use codenoesis_contracts::{RepositorySnapshotV10, RepositorySnapshotV10Error};
use codenoesis_domain::RepositoryInventory;
use codenoesis_domain::s1_boundaries::{BoundarySha256, build_boundary_report, parse_gitmodules};
use codenoesis_domain::s4_r3::ExternalWorkspaceBoundary;
use codenoesis_domain::s4_r6::FrameworkError;
use codenoesis_domain::s4_r7::CompilerIndexError;
use codenoesis_domain::s5::AnalysisCacheEntry;
use codenoesis_domain::storage::LocalSnapshotHead;
use codenoesis_ports::{
    ArtifactStore, CompilerIndexImporter, MetadataStore, PublicationObserver,
    RepositoryBoundaryAcquirer, RustFrameworkDeclarationExtractor, SafeRepositoryAcquirer,
};

use crate::s4_r4::{map_boundary_acquisition, validate_boundary_input};
use crate::{
    BoundaryScanError, PublicationService, RepositoryBoundaryScanInput, ScanError, ScanRequest,
    ScanService, map_repository_error,
};

pub struct S4R7ScanOutput {
    pub snapshot: RepositorySnapshotV10,
    pub analysis_cache_entries: Vec<AnalysisCacheEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompilerIndexScanError {
    Scan(ScanError),
    Framework(FrameworkError),
    CompilerIndex(CompilerIndexError),
    Boundary(BoundaryScanError),
}

impl<A> ScanService<A>
where
    A: SafeRepositoryAcquirer,
{
    /// Executes the explicit R7 compiler-index profile over one safe root inventory.
    ///
    /// # Errors
    ///
    /// Returns inherited acquisition failures or a typed R6/R7 extraction failure.
    pub fn scan_s4_r7<E, I>(
        &self,
        request: ScanRequest,
        extractor: &E,
        importer: &I,
    ) -> Result<S4R7ScanOutput, CompilerIndexScanError>
    where
        E: RustFrameworkDeclarationExtractor,
        I: CompilerIndexImporter,
    {
        let acquired = self
            .acquirer
            .acquire_inventory(&request.repository, request.identity, request.revision)
            .map_err(map_repository_error)
            .map_err(CompilerIndexScanError::Scan)?;
        let inventory = RepositoryInventory::classify(acquired);
        build_output(request.envelope, &inventory, None, extractor, importer)
    }
}

impl<A> ScanService<A>
where
    A: RepositoryBoundaryAcquirer,
{
    /// Executes R7 with the explicit R2 boundary projection and compiler-index pair.
    ///
    /// # Errors
    ///
    /// Returns inherited boundary failures or a typed R6/R7 extraction failure.
    pub fn scan_s4_r7_boundaries<E, I, H>(
        &self,
        request: ScanRequest,
        input: RepositoryBoundaryScanInput,
        extractor: &E,
        importer: &I,
        hasher: &H,
    ) -> Result<S4R7ScanOutput, CompilerIndexScanError>
    where
        E: RustFrameworkDeclarationExtractor,
        I: CompilerIndexImporter,
        H: BoundarySha256,
    {
        validate_boundary_input(&request, &input).map_err(CompilerIndexScanError::Boundary)?;
        let acquired = self
            .acquirer
            .acquire_inventory_with_boundaries(
                &request.repository,
                request.identity,
                request.revision,
            )
            .map_err(map_boundary_acquisition)
            .map_err(CompilerIndexScanError::Boundary)?;
        let root = acquired.repository.bound_revision().clone();
        let parsed = parse_gitmodules(&root, acquired.gitmodules.as_ref(), hasher)
            .map_err(BoundaryScanError::Boundary)
            .map_err(CompilerIndexScanError::Boundary)?;
        let verified = self
            .verify_r4_nested_boundaries(&acquired.gitlinks, input)
            .map_err(CompilerIndexScanError::Boundary)?;
        let report = build_boundary_report(&root, acquired.gitlinks, parsed, &verified, hasher)
            .map_err(BoundaryScanError::Boundary)
            .map_err(CompilerIndexScanError::Boundary)?;
        codenoesis_contracts::validate_repository_boundary_report_size(&report)
            .map_err(BoundaryScanError::Boundary)
            .map_err(CompilerIndexScanError::Boundary)?;
        let inventory = RepositoryInventory::classify(acquired.repository);
        build_output(
            request.envelope,
            &inventory,
            Some(&report),
            extractor,
            importer,
        )
    }
}

impl PublicationService {
    /// Publishes one V10 snapshot through the unchanged immutable local-store protocol.
    ///
    /// # Errors
    ///
    /// Returns a typed storage/publication failure or internal contract error.
    pub fn publish_v10<C, M>(
        snapshot: &RepositorySnapshotV10,
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

fn build_output<E, I>(
    envelope: codenoesis_contracts::SnapshotEnvelopeV1,
    inventory: &RepositoryInventory,
    boundaries: Option<&codenoesis_domain::s1_boundaries::RepositoryBoundaryReport>,
    extractor: &E,
    importer: &I,
) -> Result<S4R7ScanOutput, CompilerIndexScanError>
where
    E: RustFrameworkDeclarationExtractor,
    I: CompilerIndexImporter,
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
        .extract_rust_framework_declarations_incremental(inventory, &external_boundaries, &[])
        .map_err(CompilerIndexScanError::Framework)?;
    extraction
        .knowledge
        .validate()
        .map_err(CompilerIndexScanError::Framework)?;
    let overlay = importer
        .import_compiler_index(inventory, &extraction.knowledge)
        .map_err(CompilerIndexScanError::CompilerIndex)?;
    let snapshot = RepositorySnapshotV10::from_inventory_and_compiler_index(
        inventory,
        &extraction.knowledge,
        &overlay,
        boundaries,
        envelope,
    )
    .map_err(map_snapshot_error)?;
    Ok(S4R7ScanOutput {
        snapshot,
        analysis_cache_entries: extraction.cache_entries,
    })
}

fn map_snapshot_error(error: RepositorySnapshotV10Error) -> CompilerIndexScanError {
    match error {
        RepositorySnapshotV10Error::LimitExceeded(error) => {
            CompilerIndexScanError::Scan(ScanError::Acquisition(error))
        }
        RepositorySnapshotV10Error::Serialization(_)
        | RepositorySnapshotV10Error::ContractInvalid
        | RepositorySnapshotV10Error::OutputLengthOverflow => {
            CompilerIndexScanError::Scan(ScanError::Internal)
        }
    }
}
