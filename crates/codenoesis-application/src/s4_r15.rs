use codenoesis_contracts::{RepositorySnapshotV17, RepositorySnapshotV17Error};
use codenoesis_domain::s1_boundaries::{BoundarySha256, build_boundary_report, parse_gitmodules};
use codenoesis_domain::s4_r3::ExternalWorkspaceBoundary;
use codenoesis_domain::s4_r15::{LocalFlowError, LocalFlowExtraction};
use codenoesis_domain::s5::AnalysisCacheEntry;
use codenoesis_domain::storage::LocalSnapshotHead;
use codenoesis_domain::{K1OutputCapacityProfile, RepositoryInventory};
use codenoesis_ports::{
    ArtifactStore, MetadataStore, PublicationObserver, RepositoryBoundaryAcquirer,
    SafeRepositoryAcquirer,
};

use crate::s4_r4::{map_boundary_acquisition, validate_boundary_input};
use crate::{
    BoundaryScanError, PublicationService, RepositoryBoundaryScanInput, ScanError, ScanRequest,
    ScanService, map_repository_error,
};

pub struct S4R15ScanOutput {
    pub snapshot: RepositorySnapshotV17,
    pub analysis_cache_entries: Vec<AnalysisCacheEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LocalFlowScanError {
    Scan(ScanError),
    LocalFlow(LocalFlowError),
    InvalidSnapshot,
    Boundary(BoundaryScanError),
}

impl<A> ScanService<A>
where
    A: SafeRepositoryAcquirer,
{
    /// Executes the exact R14 source-only lineage plus the R15 local-flow overlay.
    ///
    /// # Errors
    ///
    /// Returns an acquisition, extraction, validation, or snapshot failure.
    pub fn scan_s4_r15<F>(
        &self,
        request: ScanRequest,
        output_capacity_profile: K1OutputCapacityProfile,
        extractor: F,
    ) -> Result<S4R15ScanOutput, LocalFlowScanError>
    where
        F: FnOnce(&RepositoryInventory) -> Result<LocalFlowExtraction, LocalFlowError>,
    {
        let acquired = self
            .acquirer
            .acquire_inventory(&request.repository, request.identity, request.revision)
            .map_err(map_repository_error)
            .map_err(LocalFlowScanError::Scan)?;
        let inventory = RepositoryInventory::classify(acquired);
        build_output(
            request.envelope,
            &inventory,
            None,
            output_capacity_profile,
            |inventory, _| extractor(inventory),
        )
    }
}

impl<A> ScanService<A>
where
    A: RepositoryBoundaryAcquirer,
{
    /// Executes R15 over the exact R12 cfg-alternatives and repository-boundary projection.
    ///
    /// # Errors
    ///
    /// Returns inherited boundary/acquisition failures or a typed R15 composition failure.
    pub fn scan_s4_r15_boundaries<F, H>(
        &self,
        request: ScanRequest,
        input: RepositoryBoundaryScanInput,
        output_capacity_profile: K1OutputCapacityProfile,
        extractor: F,
        hasher: &H,
    ) -> Result<S4R15ScanOutput, LocalFlowScanError>
    where
        F: FnOnce(
            &RepositoryInventory,
            &[ExternalWorkspaceBoundary],
        ) -> Result<LocalFlowExtraction, LocalFlowError>,
        H: BoundarySha256,
    {
        validate_boundary_input(&request, &input).map_err(LocalFlowScanError::Boundary)?;
        let acquired = self
            .acquirer
            .acquire_inventory_with_boundaries(
                &request.repository,
                request.identity,
                request.revision,
            )
            .map_err(map_boundary_acquisition)
            .map_err(LocalFlowScanError::Boundary)?;
        let root = acquired.repository.bound_revision().clone();
        let parsed = parse_gitmodules(&root, acquired.gitmodules.as_ref(), hasher)
            .map_err(BoundaryScanError::Boundary)
            .map_err(LocalFlowScanError::Boundary)?;
        let verified = self
            .verify_r4_nested_boundaries(&acquired.gitlinks, input)
            .map_err(LocalFlowScanError::Boundary)?;
        let report = build_boundary_report(&root, acquired.gitlinks, parsed, &verified, hasher)
            .map_err(BoundaryScanError::Boundary)
            .map_err(LocalFlowScanError::Boundary)?;
        codenoesis_contracts::validate_repository_boundary_report_size(&report)
            .map_err(BoundaryScanError::Boundary)
            .map_err(LocalFlowScanError::Boundary)?;
        let inventory = RepositoryInventory::classify(acquired.repository);
        build_output(
            request.envelope,
            &inventory,
            Some(&report),
            output_capacity_profile,
            extractor,
        )
    }
}

fn build_output<F>(
    envelope: codenoesis_contracts::SnapshotEnvelopeV1,
    inventory: &RepositoryInventory,
    boundaries: Option<&codenoesis_domain::s1_boundaries::RepositoryBoundaryReport>,
    output_capacity_profile: K1OutputCapacityProfile,
    extractor: F,
) -> Result<S4R15ScanOutput, LocalFlowScanError>
where
    F: FnOnce(
        &RepositoryInventory,
        &[ExternalWorkspaceBoundary],
    ) -> Result<LocalFlowExtraction, LocalFlowError>,
{
    let external_boundaries = boundaries
        .into_iter()
        .flat_map(|report| &report.boundaries)
        .map(|boundary| ExternalWorkspaceBoundary {
            path: boundary.path.clone(),
            boundary_id: boundary.boundary_id.clone(),
        })
        .collect::<Vec<_>>();
    let extraction =
        extractor(inventory, &external_boundaries).map_err(LocalFlowScanError::LocalFlow)?;
    let snapshot = RepositorySnapshotV17::from_inventory_local_flow_and_boundaries(
        inventory,
        &extraction.knowledge,
        boundaries,
        output_capacity_profile,
        envelope,
    )
    .map_err(map_snapshot_error)?;
    Ok(S4R15ScanOutput {
        snapshot,
        analysis_cache_entries: extraction.cache_entries,
    })
}

impl PublicationService {
    /// Publishes one V17 snapshot through the immutable local-store protocol.
    ///
    /// # Errors
    ///
    /// Returns a typed storage/publication failure or internal contract error.
    pub fn publish_v17<C, M>(
        snapshot: &RepositorySnapshotV17,
        artifact_store: &mut C,
        metadata_store: &mut M,
        observer: &mut dyn PublicationObserver,
    ) -> Result<LocalSnapshotHead, ScanError>
    where
        C: ArtifactStore,
        M: MetadataStore,
    {
        let candidate = snapshot
            .publication_candidate()
            .map_err(|_| ScanError::Internal)?;
        Self::publish_candidate(&candidate, artifact_store, metadata_store, observer)
    }
}

fn map_snapshot_error(error: RepositorySnapshotV17Error) -> LocalFlowScanError {
    match error {
        RepositorySnapshotV17Error::LimitExceeded(error) => {
            LocalFlowScanError::Scan(ScanError::Acquisition(error))
        }
        RepositorySnapshotV17Error::Serialization(_)
        | RepositorySnapshotV17Error::OutputLengthOverflow => {
            LocalFlowScanError::Scan(ScanError::Internal)
        }
        RepositorySnapshotV17Error::ContractInvalid => LocalFlowScanError::InvalidSnapshot,
    }
}
