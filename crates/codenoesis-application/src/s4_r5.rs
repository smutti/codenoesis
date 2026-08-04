use codenoesis_contracts::{RepositorySnapshotV8, RepositorySnapshotV8Error};
use codenoesis_domain::RepositoryInventory;
use codenoesis_domain::s1_boundaries::{BoundarySha256, build_boundary_report, parse_gitmodules};
use codenoesis_domain::s4_r3::ExternalWorkspaceBoundary;
use codenoesis_domain::s4_r5::RustSemanticError;
use codenoesis_domain::s5::AnalysisCacheEntry;
use codenoesis_domain::storage::LocalSnapshotHead;
use codenoesis_ports::{
    ArtifactStore, MetadataStore, PublicationObserver, RepositoryBoundaryAcquirer,
    RustSemanticDepthExtractor, SafeRepositoryAcquirer,
};

use crate::s4_r4::{map_boundary_acquisition, validate_boundary_input};
use crate::{
    BoundaryScanError, PublicationService, RepositoryBoundaryScanInput, ScanError, ScanRequest,
    ScanService, map_repository_error,
};

pub struct S4R5ScanOutput {
    pub snapshot: RepositorySnapshotV8,
    pub analysis_cache_entries: Vec<AnalysisCacheEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RustSemanticScanError {
    Scan(ScanError),
    Semantic(RustSemanticError),
    Boundary(BoundaryScanError),
}

impl<A> ScanService<A>
where
    A: SafeRepositoryAcquirer,
{
    /// Executes the explicit R5 Rust semantic-depth profile over one safe root inventory.
    ///
    /// # Errors
    ///
    /// Returns inherited acquisition/R4 failures or a typed R5 declaration failure.
    pub fn scan_s4_r5<E>(
        &self,
        request: ScanRequest,
        extractor: &E,
    ) -> Result<S4R5ScanOutput, RustSemanticScanError>
    where
        E: RustSemanticDepthExtractor,
    {
        let acquired = self
            .acquirer
            .acquire_inventory(&request.repository, request.identity, request.revision)
            .map_err(map_repository_error)
            .map_err(RustSemanticScanError::Scan)?;
        let inventory = RepositoryInventory::classify(acquired);
        build_output(request.envelope, &inventory, None, extractor)
    }
}

impl<A> ScanService<A>
where
    A: RepositoryBoundaryAcquirer,
{
    /// Executes R5 with the explicit R2 boundary projection and no implicit nested authority.
    ///
    /// # Errors
    ///
    /// Returns inherited R2/R4 failures or a typed R5 declaration failure.
    pub fn scan_s4_r5_boundaries<E, H>(
        &self,
        request: ScanRequest,
        input: RepositoryBoundaryScanInput,
        extractor: &E,
        hasher: &H,
    ) -> Result<S4R5ScanOutput, RustSemanticScanError>
    where
        E: RustSemanticDepthExtractor,
        H: BoundarySha256,
    {
        validate_boundary_input(&request, &input).map_err(RustSemanticScanError::Boundary)?;
        let acquired = self
            .acquirer
            .acquire_inventory_with_boundaries(
                &request.repository,
                request.identity,
                request.revision,
            )
            .map_err(map_boundary_acquisition)
            .map_err(RustSemanticScanError::Boundary)?;
        let root = acquired.repository.bound_revision().clone();
        let parsed = parse_gitmodules(&root, acquired.gitmodules.as_ref(), hasher)
            .map_err(BoundaryScanError::Boundary)
            .map_err(RustSemanticScanError::Boundary)?;
        let verified = self
            .verify_r4_nested_boundaries(&acquired.gitlinks, input)
            .map_err(RustSemanticScanError::Boundary)?;
        let report = build_boundary_report(&root, acquired.gitlinks, parsed, &verified, hasher)
            .map_err(BoundaryScanError::Boundary)
            .map_err(RustSemanticScanError::Boundary)?;
        codenoesis_contracts::validate_repository_boundary_report_size(&report)
            .map_err(BoundaryScanError::Boundary)
            .map_err(RustSemanticScanError::Boundary)?;
        let inventory = RepositoryInventory::classify(acquired.repository);
        build_output(request.envelope, &inventory, Some(&report), extractor)
    }
}

impl PublicationService {
    /// Publishes one V8 snapshot through the unchanged immutable local-store protocol.
    ///
    /// # Errors
    ///
    /// Returns a typed storage/publication failure or internal contract error.
    pub fn publish_v8<C, M>(
        snapshot: &RepositorySnapshotV8,
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
) -> Result<S4R5ScanOutput, RustSemanticScanError>
where
    E: RustSemanticDepthExtractor,
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
        .extract_rust_semantic_depth_incremental(inventory, &external_boundaries, &[])
        .map_err(RustSemanticScanError::Semantic)?;
    extraction
        .knowledge
        .validate()
        .map_err(RustSemanticScanError::Semantic)?;
    let snapshot = RepositorySnapshotV8::from_inventory_and_rust_semantics(
        inventory,
        &extraction.knowledge,
        boundaries,
        envelope,
    )
    .map_err(map_snapshot_error)?;
    Ok(S4R5ScanOutput {
        snapshot,
        analysis_cache_entries: extraction.cache_entries,
    })
}

fn map_snapshot_error(error: RepositorySnapshotV8Error) -> RustSemanticScanError {
    match error {
        RepositorySnapshotV8Error::LimitExceeded(error) => {
            RustSemanticScanError::Scan(ScanError::Acquisition(error))
        }
        RepositorySnapshotV8Error::Serialization(_)
        | RepositorySnapshotV8Error::ContractInvalid
        | RepositorySnapshotV8Error::OutputLengthOverflow => {
            RustSemanticScanError::Scan(ScanError::Internal)
        }
    }
}
