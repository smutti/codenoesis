use codenoesis_contracts::RepositorySnapshotV4;
use codenoesis_domain::RepositoryInventory;
use codenoesis_domain::storage::LocalSnapshotHead;
use codenoesis_ports::{
    ArtifactStore, MetadataStore, PublicationObserver, RustWorkspaceExtractor,
    SafeRepositoryAcquirer,
};

use crate::{PublicationService, ScanError, ScanRequest, ScanService};

impl<A> ScanService<A>
where
    A: SafeRepositoryAcquirer,
{
    /// Executes an S4 offline workspace scan over the approved S1 acquisition.
    ///
    /// # Errors
    ///
    /// Returns a typed acquisition, workspace, ontology, or graph failure.
    pub fn scan_s4<E>(
        &self,
        request: ScanRequest,
        extractor: &E,
    ) -> Result<RepositorySnapshotV4, ScanError>
    where
        E: RustWorkspaceExtractor,
    {
        let acquired = self
            .acquirer
            .acquire_inventory(&request.repository, request.identity, request.revision)
            .map_err(|error| match error {
                codenoesis_domain::RepositoryError::Acquisition(acquisition) => {
                    ScanError::Acquisition(acquisition)
                }
                codenoesis_domain::RepositoryError::Unexpected => ScanError::Internal,
            })?;
        let inventory = RepositoryInventory::classify(acquired);
        let knowledge = extractor
            .extract_workspace(&inventory)
            .map_err(ScanError::Workspace)?;
        knowledge.validate().map_err(ScanError::Workspace)?;
        Ok(RepositorySnapshotV4::from_inventory_and_workspace(
            &inventory,
            &knowledge,
            request.envelope,
        ))
    }
}

impl PublicationService {
    /// Publishes one V4 snapshot through the inherited immutable local store.
    ///
    /// # Errors
    ///
    /// Returns a typed storage/publication failure or internal contract error.
    pub fn publish_v4<C, M>(
        snapshot: &RepositorySnapshotV4,
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
