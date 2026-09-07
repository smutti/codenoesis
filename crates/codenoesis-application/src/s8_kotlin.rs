use crate::{PublicationService, ScanError, ScanRequest, ScanService, map_repository_error};
use codenoesis_contracts::s8_kotlin::KotlinSnapshot;
use codenoesis_domain::storage::LocalSnapshotHead;
use codenoesis_domain::{RepositoryInventory, s8_kotlin::KotlinError};
use codenoesis_ports::{
    ArtifactStore, KotlinWorkspaceExtractor, MetadataStore, PublicationObserver,
    SafeRepositoryAcquirer,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KotlinScanError {
    Scan(ScanError),
    Extraction(KotlinError),
}

impl<A: SafeRepositoryAcquirer> ScanService<A> {
    /// Acquires one immutable commit and extracts bounded static Kotlin facts.
    /// # Errors
    /// Returns typed acquisition, extraction or serialization failures.
    pub fn scan_kotlin(
        &self,
        request: ScanRequest,
        extractor: &impl KotlinWorkspaceExtractor,
    ) -> Result<KotlinSnapshot, KotlinScanError> {
        let acquired = self
            .acquirer
            .acquire_inventory(&request.repository, request.identity, request.revision)
            .map_err(map_repository_error)
            .map_err(KotlinScanError::Scan)?;
        let inventory = RepositoryInventory::classify(acquired);
        let workspace = extractor
            .extract_kotlin_workspace(&inventory)
            .map_err(KotlinScanError::Extraction)?;
        KotlinSnapshot::from_workspace(&inventory, &workspace, &request.envelope)
            .map_err(KotlinScanError::Extraction)
    }
}
impl PublicationService {
    /// Publishes Kotlin through the existing immutable CAS and metadata protocol.
    /// # Errors
    /// Returns a typed contract or storage failure without publishing a partial head.
    pub fn publish_kotlin<C: ArtifactStore, M: MetadataStore>(
        snapshot: &KotlinSnapshot,
        artifacts: &mut C,
        metadata: &mut M,
        observer: &mut dyn PublicationObserver,
    ) -> Result<LocalSnapshotHead, ScanError> {
        let candidate = snapshot
            .publication_candidate()
            .map_err(|_| ScanError::Internal)?;
        Self::publish_candidate(&candidate, artifacts, metadata, observer)
    }
}
