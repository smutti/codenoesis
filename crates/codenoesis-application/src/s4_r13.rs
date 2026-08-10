use codenoesis_contracts::{RepositorySnapshotV15, RepositorySnapshotV15Error};
use codenoesis_domain::RepositoryInventory;
use codenoesis_domain::s4_k1::{CallableSemanticsError, CallableSemanticsExtraction};
use codenoesis_domain::s4_r7::CompilerIndexError;
use codenoesis_domain::s4_r13::{CallableScipCompositionError, CallableScipKnowledge};
use codenoesis_domain::s5::AnalysisCacheEntry;
use codenoesis_domain::storage::LocalSnapshotHead;
use codenoesis_ports::{
    ArtifactStore, CompilerIndexImporter, MetadataStore, PublicationObserver,
    SafeRepositoryAcquirer,
};

use crate::{PublicationService, ScanError, ScanRequest, ScanService, map_repository_error};

pub struct S4R13ScanOutput {
    pub snapshot: RepositorySnapshotV15,
    pub analysis_cache_entries: Vec<AnalysisCacheEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableScipScanError {
    Scan(ScanError),
    Callable(CallableSemanticsError),
    CompilerIndex(CompilerIndexError),
    Composition(CallableScipCompositionError),
    InvalidSnapshot,
}

impl<A> ScanService<A>
where
    A: SafeRepositoryAcquirer,
{
    /// Executes the exact source K1 plus revision-bound R7 composition.
    ///
    /// # Errors
    ///
    /// Returns an acquisition, extraction, import, composition, or snapshot failure.
    pub fn scan_s4_r13<F, I>(
        &self,
        request: ScanRequest,
        callable_extractor: F,
        importer: &I,
    ) -> Result<S4R13ScanOutput, CallableScipScanError>
    where
        F: FnOnce(
            &RepositoryInventory,
        ) -> Result<CallableSemanticsExtraction, CallableSemanticsError>,
        I: CompilerIndexImporter,
    {
        let acquired = self
            .acquirer
            .acquire_inventory(&request.repository, request.identity, request.revision)
            .map_err(map_repository_error)
            .map_err(CallableScipScanError::Scan)?;
        let inventory = RepositoryInventory::classify(acquired);
        let extraction = callable_extractor(&inventory).map_err(CallableScipScanError::Callable)?;
        extraction
            .knowledge
            .validate()
            .map_err(CallableScipScanError::Callable)?;
        let compiler = importer
            .import_compiler_index(&inventory, &extraction.knowledge.framework)
            .map_err(CallableScipScanError::CompilerIndex)?;
        let composition = CallableScipKnowledge::compose(extraction.knowledge, compiler)
            .map_err(CallableScipScanError::Composition)?;
        let snapshot = RepositorySnapshotV15::from_inventory_callable_scip(
            &inventory,
            &composition,
            request.envelope,
        )
        .map_err(map_snapshot_error)?;
        Ok(S4R13ScanOutput {
            snapshot,
            analysis_cache_entries: extraction.cache_entries,
        })
    }
}

impl PublicationService {
    /// Publishes one V15 snapshot through the immutable local-store protocol.
    ///
    /// # Errors
    ///
    /// Returns a typed storage/publication failure or internal contract error.
    pub fn publish_v15<C, M>(
        snapshot: &RepositorySnapshotV15,
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

fn map_snapshot_error(error: RepositorySnapshotV15Error) -> CallableScipScanError {
    match error {
        RepositorySnapshotV15Error::LimitExceeded(error) => {
            CallableScipScanError::Scan(ScanError::Acquisition(error))
        }
        RepositorySnapshotV15Error::Serialization(_)
        | RepositorySnapshotV15Error::OutputLengthOverflow => {
            CallableScipScanError::Scan(ScanError::Internal)
        }
        RepositorySnapshotV15Error::ContractInvalid => CallableScipScanError::InvalidSnapshot,
    }
}
