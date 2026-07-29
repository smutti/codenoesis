use codenoesis_contracts::RepositorySnapshotV4;
use codenoesis_domain::RepositoryInventory;
use codenoesis_domain::s5::AnalysisCacheEntry;
use codenoesis_domain::storage::LocalSnapshotHead;
use codenoesis_ports::{
    ArtifactStore, IncrementalRustWorkspaceExtractor, MetadataStore, PublicationObserver,
    RustWorkspaceExtractor, SafeRepositoryAcquirer,
};

use crate::{PublicationService, ScanError, ScanRequest, ScanService};

pub struct S4ScanOutput {
    pub snapshot: RepositorySnapshotV4,
    pub analysis_cache_entries: Vec<AnalysisCacheEntry>,
}

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

    /// Executes the unchanged S4 semantic scan while retaining internal
    /// revision-neutral source analyses for a future S5 refresh.
    ///
    /// # Errors
    ///
    /// Returns the same typed S4 acquisition and workspace failures.
    pub fn scan_s4_with_analysis<E>(
        &self,
        request: ScanRequest,
        extractor: &E,
    ) -> Result<S4ScanOutput, ScanError>
    where
        E: IncrementalRustWorkspaceExtractor,
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
        let extraction = extractor
            .extract_workspace_incremental(&inventory, &[])
            .map_err(ScanError::Workspace)?;
        extraction
            .knowledge
            .validate()
            .map_err(ScanError::Workspace)?;
        Ok(S4ScanOutput {
            snapshot: RepositorySnapshotV4::from_inventory_and_workspace(
                &inventory,
                &extraction.knowledge,
                request.envelope,
            ),
            analysis_cache_entries: extraction.cache_entries,
        })
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

    /// Publishes one V4 snapshot only if the exact previously observed S4
    /// baseline head remains visible.
    ///
    /// # Errors
    ///
    /// Returns a typed compare-and-set, storage, or contract failure.
    pub fn publish_v4_expected<C, M>(
        snapshot: &RepositorySnapshotV4,
        expected_head: &codenoesis_domain::storage::SnapshotId,
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
        Self::publish_candidate_with_expected(
            &candidate,
            Some(expected_head),
            artifact_store,
            metadata_store,
            observer,
        )
    }
}
