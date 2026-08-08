use codenoesis_contracts::{RepositorySnapshotV11, RepositorySnapshotV11Error};
use codenoesis_domain::RepositoryInventory;
use codenoesis_domain::s4_k1::{CallableSemanticsError, CallableSemanticsExtraction};
use codenoesis_domain::s5::AnalysisCacheEntry;
use codenoesis_domain::storage::LocalSnapshotHead;
use codenoesis_ports::{ArtifactStore, MetadataStore, PublicationObserver, SafeRepositoryAcquirer};

use crate::{PublicationService, ScanError, ScanRequest, ScanService, map_repository_error};

pub struct S4K1ScanOutput {
    pub snapshot: RepositorySnapshotV11,
    pub analysis_cache_entries: Vec<AnalysisCacheEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableSemanticsScanError {
    Scan(ScanError),
    Callable(CallableSemanticsError),
}

impl<A> ScanService<A>
where
    A: SafeRepositoryAcquirer,
{
    /// Executes the explicit source-only K1 callable-semantics profile.
    ///
    /// # Errors
    ///
    /// Returns an inherited acquisition failure or typed K1 extraction failure.
    pub fn scan_s4_k1<F>(
        &self,
        request: ScanRequest,
        extractor: F,
    ) -> Result<S4K1ScanOutput, CallableSemanticsScanError>
    where
        F: FnOnce(
            &RepositoryInventory,
        ) -> Result<CallableSemanticsExtraction, CallableSemanticsError>,
    {
        let acquired = self
            .acquirer
            .acquire_inventory(&request.repository, request.identity, request.revision)
            .map_err(map_repository_error)
            .map_err(CallableSemanticsScanError::Scan)?;
        let inventory = RepositoryInventory::classify(acquired);
        let extraction = extractor(&inventory).map_err(CallableSemanticsScanError::Callable)?;
        extraction
            .knowledge
            .validate()
            .map_err(CallableSemanticsScanError::Callable)?;
        let snapshot = RepositorySnapshotV11::from_inventory_and_callable_semantics(
            &inventory,
            &extraction.knowledge,
            request.envelope,
        )
        .map_err(map_snapshot_error)?;
        Ok(S4K1ScanOutput {
            snapshot,
            analysis_cache_entries: extraction.cache_entries,
        })
    }
}

impl PublicationService {
    /// Publishes one V11 snapshot through the immutable local-store protocol.
    ///
    /// # Errors
    ///
    /// Returns a typed storage/publication failure or internal contract error.
    pub fn publish_v11<C, M>(
        snapshot: &RepositorySnapshotV11,
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

fn map_snapshot_error(error: RepositorySnapshotV11Error) -> CallableSemanticsScanError {
    match error {
        RepositorySnapshotV11Error::LimitExceeded(error) => {
            CallableSemanticsScanError::Scan(ScanError::Acquisition(error))
        }
        RepositorySnapshotV11Error::Serialization(_)
        | RepositorySnapshotV11Error::ContractInvalid
        | RepositorySnapshotV11Error::OutputLengthOverflow => {
            CallableSemanticsScanError::Scan(ScanError::Internal)
        }
    }
}
