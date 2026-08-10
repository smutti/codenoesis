use codenoesis_contracts::{RepositorySnapshotV14, RepositorySnapshotV14Error};
use codenoesis_domain::RepositoryInventory;
use codenoesis_domain::s1_boundaries::{BoundarySha256, build_boundary_report, parse_gitmodules};
use codenoesis_domain::s4_r3::ExternalWorkspaceBoundary;
use codenoesis_domain::s4_r12::CallableCfgAlternativesError;
use codenoesis_domain::s5::AnalysisCacheEntry;
use codenoesis_domain::storage::LocalSnapshotHead;
use codenoesis_ports::{
    ArtifactStore, MetadataStore, PublicationObserver, RepositoryBoundaryAcquirer,
    RustCallableCfgAlternativesCompositionExtractor, SafeRepositoryAcquirer,
};

use crate::s4_r4::{map_boundary_acquisition, validate_boundary_input};
use crate::{
    BoundaryScanError, PublicationService, RepositoryBoundaryScanInput, ScanError, ScanRequest,
    ScanService, map_repository_error,
};

pub struct S4R12ScanOutput {
    pub snapshot: RepositorySnapshotV14,
    pub analysis_cache_entries: Vec<AnalysisCacheEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableCfgAlternativesScanError {
    Scan(ScanError),
    Composition(CallableCfgAlternativesError),
    InvalidSnapshot,
    Boundary(BoundaryScanError),
}

impl<A> ScanService<A>
where
    A: SafeRepositoryAcquirer,
{
    /// Executes R12 without repository-boundary authority.
    ///
    /// # Errors
    ///
    /// Returns inherited acquisition failures or a typed R12 composition failure.
    pub fn scan_s4_r12<E>(
        &self,
        request: ScanRequest,
        extractor: &E,
    ) -> Result<S4R12ScanOutput, CallableCfgAlternativesScanError>
    where
        E: RustCallableCfgAlternativesCompositionExtractor,
    {
        let acquired = self
            .acquirer
            .acquire_inventory(&request.repository, request.identity, request.revision)
            .map_err(map_repository_error)
            .map_err(CallableCfgAlternativesScanError::Scan)?;
        let inventory = RepositoryInventory::classify(acquired);
        build_output(request.envelope, &inventory, None, extractor)
    }
}

impl<A> ScanService<A>
where
    A: RepositoryBoundaryAcquirer,
{
    /// Executes R12 over the exact R2 boundary report without traversing nested source.
    ///
    /// # Errors
    ///
    /// Returns inherited boundary/acquisition failures or a typed R12 composition failure.
    pub fn scan_s4_r12_boundaries<E, H>(
        &self,
        request: ScanRequest,
        input: RepositoryBoundaryScanInput,
        extractor: &E,
        hasher: &H,
    ) -> Result<S4R12ScanOutput, CallableCfgAlternativesScanError>
    where
        E: RustCallableCfgAlternativesCompositionExtractor,
        H: BoundarySha256,
    {
        validate_boundary_input(&request, &input)
            .map_err(CallableCfgAlternativesScanError::Boundary)?;
        let acquired = self
            .acquirer
            .acquire_inventory_with_boundaries(
                &request.repository,
                request.identity,
                request.revision,
            )
            .map_err(map_boundary_acquisition)
            .map_err(CallableCfgAlternativesScanError::Boundary)?;
        let root = acquired.repository.bound_revision().clone();
        let parsed = parse_gitmodules(&root, acquired.gitmodules.as_ref(), hasher)
            .map_err(BoundaryScanError::Boundary)
            .map_err(CallableCfgAlternativesScanError::Boundary)?;
        let verified = self
            .verify_r4_nested_boundaries(&acquired.gitlinks, input)
            .map_err(CallableCfgAlternativesScanError::Boundary)?;
        let report = build_boundary_report(&root, acquired.gitlinks, parsed, &verified, hasher)
            .map_err(BoundaryScanError::Boundary)
            .map_err(CallableCfgAlternativesScanError::Boundary)?;
        codenoesis_contracts::validate_repository_boundary_report_size(&report)
            .map_err(BoundaryScanError::Boundary)
            .map_err(CallableCfgAlternativesScanError::Boundary)?;
        let inventory = RepositoryInventory::classify(acquired.repository);
        build_output(request.envelope, &inventory, Some(&report), extractor)
    }
}

impl PublicationService {
    /// Publishes one V14 snapshot through the immutable local-store protocol.
    ///
    /// # Errors
    ///
    /// Returns a typed storage/publication failure or internal contract error.
    pub fn publish_v14<C, M>(
        snapshot: &RepositorySnapshotV14,
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

fn build_output<E>(
    envelope: codenoesis_contracts::SnapshotEnvelopeV1,
    inventory: &RepositoryInventory,
    boundaries: Option<&codenoesis_domain::s1_boundaries::RepositoryBoundaryReport>,
    extractor: &E,
) -> Result<S4R12ScanOutput, CallableCfgAlternativesScanError>
where
    E: RustCallableCfgAlternativesCompositionExtractor,
{
    let external_boundaries = boundaries
        .into_iter()
        .flat_map(|report| &report.boundaries)
        .map(|boundary| ExternalWorkspaceBoundary {
            path: boundary.path.clone(),
            boundary_id: boundary.boundary_id.clone(),
        })
        .collect::<Vec<_>>();
    let extraction = extractor
        .extract_rust_callable_cfg_alternatives_with_boundaries(inventory, &external_boundaries)
        .map_err(CallableCfgAlternativesScanError::Composition)?;
    extraction
        .knowledge
        .validate()
        .map_err(CallableCfgAlternativesScanError::Composition)?;
    let snapshot = RepositorySnapshotV14::from_inventory_callable_cfg_alternatives(
        inventory,
        &extraction.knowledge,
        boundaries,
        envelope,
    )
    .map_err(map_snapshot_error)?;
    Ok(S4R12ScanOutput {
        snapshot,
        analysis_cache_entries: extraction.cache_entries,
    })
}

fn map_snapshot_error(error: RepositorySnapshotV14Error) -> CallableCfgAlternativesScanError {
    match error {
        RepositorySnapshotV14Error::LimitExceeded(error) => {
            CallableCfgAlternativesScanError::Scan(ScanError::Acquisition(error))
        }
        RepositorySnapshotV14Error::ContractInvalid => {
            CallableCfgAlternativesScanError::InvalidSnapshot
        }
        RepositorySnapshotV14Error::Serialization(_)
        | RepositorySnapshotV14Error::OutputLengthOverflow => {
            CallableCfgAlternativesScanError::Scan(ScanError::Internal)
        }
    }
}
