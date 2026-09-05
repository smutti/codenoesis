use codenoesis_contracts::{RepositorySnapshotV18, RepositorySnapshotV18Error};
use codenoesis_domain::s1_boundaries::{BoundarySha256, build_boundary_report, parse_gitmodules};
use codenoesis_domain::s4_r3::ExternalWorkspaceBoundary;
use codenoesis_domain::s4_r16::{ConstantEvaluationError, ConstantEvaluationExtraction};
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

pub struct S4R16ScanOutput {
    pub snapshot: RepositorySnapshotV18,
    pub analysis_cache_entries: Vec<AnalysisCacheEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConstantEvaluationScanError {
    Scan(ScanError),
    ConstantEvaluation(ConstantEvaluationError),
    InvalidSnapshot,
    Boundary(BoundaryScanError),
}

impl<A> ScanService<A>
where
    A: SafeRepositoryAcquirer,
{
    /// Executes the exact R15 source-only lineage plus the R16 constant-evaluation overlay.
    ///
    /// # Errors
    ///
    /// Returns an acquisition, extraction, validation, or snapshot failure.
    pub fn scan_s4_r16<F>(
        &self,
        request: ScanRequest,
        output_capacity_profile: K1OutputCapacityProfile,
        extractor: F,
    ) -> Result<S4R16ScanOutput, ConstantEvaluationScanError>
    where
        F: FnOnce(
            &RepositoryInventory,
        ) -> Result<ConstantEvaluationExtraction, ConstantEvaluationError>,
    {
        let acquired = self
            .acquirer
            .acquire_inventory(&request.repository, request.identity, request.revision)
            .map_err(map_repository_error)
            .map_err(ConstantEvaluationScanError::Scan)?;
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
    /// Executes R16 over the exact R12 cfg-alternatives and repository-boundary projection.
    ///
    /// # Errors
    ///
    /// Returns inherited boundary/acquisition failures or a typed R16 composition failure.
    pub fn scan_s4_r16_boundaries<F, H>(
        &self,
        request: ScanRequest,
        input: RepositoryBoundaryScanInput,
        output_capacity_profile: K1OutputCapacityProfile,
        extractor: F,
        hasher: &H,
    ) -> Result<S4R16ScanOutput, ConstantEvaluationScanError>
    where
        F: FnOnce(
            &RepositoryInventory,
            &[ExternalWorkspaceBoundary],
        ) -> Result<ConstantEvaluationExtraction, ConstantEvaluationError>,
        H: BoundarySha256,
    {
        validate_boundary_input(&request, &input).map_err(ConstantEvaluationScanError::Boundary)?;
        let acquired = self
            .acquirer
            .acquire_inventory_with_boundaries(
                &request.repository,
                request.identity,
                request.revision,
            )
            .map_err(map_boundary_acquisition)
            .map_err(ConstantEvaluationScanError::Boundary)?;
        let root = acquired.repository.bound_revision().clone();
        let parsed = parse_gitmodules(&root, acquired.gitmodules.as_ref(), hasher)
            .map_err(BoundaryScanError::Boundary)
            .map_err(ConstantEvaluationScanError::Boundary)?;
        let verified = self
            .verify_r4_nested_boundaries(&acquired.gitlinks, input)
            .map_err(ConstantEvaluationScanError::Boundary)?;
        let report = build_boundary_report(&root, acquired.gitlinks, parsed, &verified, hasher)
            .map_err(BoundaryScanError::Boundary)
            .map_err(ConstantEvaluationScanError::Boundary)?;
        codenoesis_contracts::validate_repository_boundary_report_size(&report)
            .map_err(BoundaryScanError::Boundary)
            .map_err(ConstantEvaluationScanError::Boundary)?;
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
) -> Result<S4R16ScanOutput, ConstantEvaluationScanError>
where
    F: FnOnce(
        &RepositoryInventory,
        &[ExternalWorkspaceBoundary],
    ) -> Result<ConstantEvaluationExtraction, ConstantEvaluationError>,
{
    let external_boundaries = boundaries
        .into_iter()
        .flat_map(|report| &report.boundaries)
        .map(|boundary| ExternalWorkspaceBoundary {
            path: boundary.path.clone(),
            boundary_id: boundary.boundary_id.clone(),
        })
        .collect::<Vec<_>>();
    let extraction = extractor(inventory, &external_boundaries)
        .map_err(ConstantEvaluationScanError::ConstantEvaluation)?;
    let snapshot = RepositorySnapshotV18::from_inventory_constant_evaluation_and_boundaries(
        inventory,
        &extraction.knowledge,
        boundaries,
        output_capacity_profile,
        envelope,
    )
    .map_err(map_snapshot_error)?;
    Ok(S4R16ScanOutput {
        snapshot,
        analysis_cache_entries: extraction.cache_entries,
    })
}

impl PublicationService {
    /// Publishes one V18 snapshot through the immutable local-store protocol.
    ///
    /// # Errors
    ///
    /// Returns a typed storage/publication failure or internal contract error.
    pub fn publish_v18<C, M>(
        snapshot: &RepositorySnapshotV18,
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

fn map_snapshot_error(error: RepositorySnapshotV18Error) -> ConstantEvaluationScanError {
    match error {
        RepositorySnapshotV18Error::LimitExceeded(error) => {
            ConstantEvaluationScanError::Scan(ScanError::Acquisition(error))
        }
        RepositorySnapshotV18Error::Serialization(_)
        | RepositorySnapshotV18Error::OutputLengthOverflow => {
            ConstantEvaluationScanError::Scan(ScanError::Internal)
        }
        RepositorySnapshotV18Error::ContractInvalid => ConstantEvaluationScanError::InvalidSnapshot,
    }
}
