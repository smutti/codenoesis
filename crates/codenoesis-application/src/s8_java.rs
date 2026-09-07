use crate::{PublicationService, ScanError, ScanRequest, ScanService, map_repository_error};
use codenoesis_contracts::s8_java::JavaSnapshot;
use codenoesis_domain::storage::LocalSnapshotHead;
use codenoesis_domain::{RepositoryInventory, s8_java::JavaError};
use codenoesis_ports::{
    ArtifactStore, JavaWorkspaceExtractor, MetadataStore, PublicationObserver,
    SafeRepositoryAcquirer,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JavaScanError {
    Scan(ScanError),
    Extraction(JavaError),
}

impl<A: SafeRepositoryAcquirer> ScanService<A> {
    /// Acquires one immutable commit and extracts bounded static Java facts.
    /// # Errors
    /// Returns typed acquisition, extraction or serialization failures.
    pub fn scan_java(
        &self,
        request: ScanRequest,
        extractor: &impl JavaWorkspaceExtractor,
    ) -> Result<JavaSnapshot, JavaScanError> {
        let acquired = self
            .acquirer
            .acquire_inventory(&request.repository, request.identity, request.revision)
            .map_err(map_repository_error)
            .map_err(JavaScanError::Scan)?;
        let inventory = RepositoryInventory::classify(acquired);
        let workspace = extractor
            .extract_java_workspace(&inventory)
            .map_err(JavaScanError::Extraction)?;
        JavaSnapshot::from_workspace(&inventory, &workspace, &request.envelope)
            .map_err(JavaScanError::Extraction)
    }
}
impl PublicationService {
    /// Publishes Java through the existing immutable CAS and metadata protocol.
    /// # Errors
    /// Returns a typed contract or storage failure without publishing a partial head.
    pub fn publish_java<C: ArtifactStore, M: MetadataStore>(
        snapshot: &JavaSnapshot,
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
