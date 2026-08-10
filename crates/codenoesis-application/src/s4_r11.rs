use codenoesis_contracts::{RepositorySnapshotV13, RepositorySnapshotV13Error};
use codenoesis_domain::RepositoryInventory;
use codenoesis_domain::s1_boundaries::{BoundarySha256, build_boundary_report, parse_gitmodules};
use codenoesis_domain::s4_k1::CallableSemanticsError;
use codenoesis_domain::s4_r3::ExternalWorkspaceBoundary;
use codenoesis_domain::s5::AnalysisCacheEntry;
use codenoesis_domain::storage::LocalSnapshotHead;
use codenoesis_ports::{
    ArtifactStore, MetadataStore, PublicationObserver, RepositoryBoundaryAcquirer,
    RustCallableBoundaryCompositionExtractor,
};

use crate::s4_r4::{map_boundary_acquisition, validate_boundary_input};
use crate::{
    BoundaryScanError, PublicationService, RepositoryBoundaryScanInput, ScanError, ScanRequest,
    ScanService,
};

pub struct S4R11ScanOutput {
    pub snapshot: RepositorySnapshotV13,
    pub analysis_cache_entries: Vec<AnalysisCacheEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableBoundaryCompositionScanError {
    Scan(ScanError),
    Callable(CallableSemanticsError),
    InvalidSnapshot,
    Boundary(BoundaryScanError),
}

impl<A> ScanService<A>
where
    A: RepositoryBoundaryAcquirer,
{
    /// Executes K1 over the explicit R2 repository-boundary projection.
    ///
    /// # Errors
    ///
    /// Returns inherited boundary/acquisition failures or a typed K1 failure.
    pub fn scan_s4_r11_boundaries<E, H>(
        &self,
        request: ScanRequest,
        input: RepositoryBoundaryScanInput,
        extractor: &E,
        hasher: &H,
    ) -> Result<S4R11ScanOutput, CallableBoundaryCompositionScanError>
    where
        E: RustCallableBoundaryCompositionExtractor,
        H: BoundarySha256,
    {
        validate_boundary_input(&request, &input)
            .map_err(CallableBoundaryCompositionScanError::Boundary)?;
        let acquired = self
            .acquirer
            .acquire_inventory_with_boundaries(
                &request.repository,
                request.identity,
                request.revision,
            )
            .map_err(map_boundary_acquisition)
            .map_err(CallableBoundaryCompositionScanError::Boundary)?;
        let root = acquired.repository.bound_revision().clone();
        let parsed = parse_gitmodules(&root, acquired.gitmodules.as_ref(), hasher)
            .map_err(BoundaryScanError::Boundary)
            .map_err(CallableBoundaryCompositionScanError::Boundary)?;
        let verified = self
            .verify_r4_nested_boundaries(&acquired.gitlinks, input)
            .map_err(CallableBoundaryCompositionScanError::Boundary)?;
        let report = build_boundary_report(&root, acquired.gitlinks, parsed, &verified, hasher)
            .map_err(BoundaryScanError::Boundary)
            .map_err(CallableBoundaryCompositionScanError::Boundary)?;
        codenoesis_contracts::validate_repository_boundary_report_size(&report)
            .map_err(BoundaryScanError::Boundary)
            .map_err(CallableBoundaryCompositionScanError::Boundary)?;
        let inventory = RepositoryInventory::classify(acquired.repository);
        let external_boundaries = report
            .boundaries
            .iter()
            .map(|boundary| ExternalWorkspaceBoundary {
                path: boundary.path.clone(),
                boundary_id: boundary.boundary_id.clone(),
            })
            .collect::<Vec<_>>();
        let extraction = extractor
            .extract_rust_callable_semantics_with_boundaries(&inventory, &external_boundaries)
            .map_err(CallableBoundaryCompositionScanError::Callable)?;
        extraction
            .knowledge
            .validate()
            .map_err(CallableBoundaryCompositionScanError::Callable)?;
        let snapshot = RepositorySnapshotV13::from_inventory_callable_and_boundaries(
            &inventory,
            &extraction.knowledge,
            &report,
            request.envelope,
        )
        .map_err(map_snapshot_error)?;
        Ok(S4R11ScanOutput {
            snapshot,
            analysis_cache_entries: extraction.cache_entries,
        })
    }
}

impl PublicationService {
    /// Publishes one V13 snapshot through the immutable local-store protocol.
    ///
    /// # Errors
    ///
    /// Returns a typed storage/publication failure or internal contract error.
    pub fn publish_v13<C, M>(
        snapshot: &RepositorySnapshotV13,
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

fn map_snapshot_error(error: RepositorySnapshotV13Error) -> CallableBoundaryCompositionScanError {
    match error {
        RepositorySnapshotV13Error::LimitExceeded(error) => {
            CallableBoundaryCompositionScanError::Scan(ScanError::Acquisition(error))
        }
        RepositorySnapshotV13Error::ContractInvalid => {
            CallableBoundaryCompositionScanError::InvalidSnapshot
        }
        RepositorySnapshotV13Error::Serialization(_)
        | RepositorySnapshotV13Error::OutputLengthOverflow => {
            CallableBoundaryCompositionScanError::Scan(ScanError::Internal)
        }
    }
}
