//! Inward-owned ports for the `CodeNoesis` S0 through S4 slices.

use std::collections::BTreeSet;
use std::ffi::OsStr;

use codenoesis_domain::knowledge::{KnowledgeError, RustKnowledge};
use codenoesis_domain::s1_boundaries::{
    AcquiredRepositoryBoundaries, NestedAcquisitionProfile, NestedRepositoryAcquisitionError,
    RepositoryBoundaryAcquisitionError,
};
use codenoesis_domain::s4::{RustWorkspaceKnowledge, WorkspaceError};
use codenoesis_domain::s5::{AnalysisCacheEntry, IncrementalWorkspaceExtraction};
use codenoesis_domain::s6::{ContractError, OpenApiContractInput, ProviderContract};
use codenoesis_domain::storage::{
    ArtifactId, LocalSnapshotHead, PublicationCandidate, PublicationEvent, PublicationResult,
    SnapshotId, StorageError, StoredArtifact, SweepResult,
};
use codenoesis_domain::{
    AcquiredRepository, BoundRevision, RepositoryError, RepositoryIdentity, RepositoryInventory,
    Revision,
};

pub trait RepositoryAcquirer {
    /// Resolves and verifies the complete supported repository closure once.
    ///
    /// # Errors
    ///
    /// Returns a typed repository failure without exposing the source locator.
    fn bind(
        &self,
        repository: &OsStr,
        identity: RepositoryIdentity,
        revision: Revision,
    ) -> Result<BoundRevision, RepositoryError>;
}

pub trait SafeRepositoryAcquirer {
    /// Resolves, verifies, and safely inventories the approved S1 repository subset.
    ///
    /// # Errors
    ///
    /// Returns a typed repository failure without exposing the source locator.
    fn acquire_inventory(
        &self,
        repository: &OsStr,
        identity: RepositoryIdentity,
        revision: Revision,
    ) -> Result<AcquiredRepository, RepositoryError>;
}

pub trait RepositoryBoundaryAcquirer {
    /// Resolves the immutable root and collects gitlinks without entering them.
    ///
    /// # Errors
    ///
    /// Returns a typed root acquisition or boundary-limit failure.
    fn acquire_inventory_with_boundaries(
        &self,
        repository: &OsStr,
        identity: RepositoryIdentity,
        revision: Revision,
    ) -> Result<AcquiredRepositoryBoundaries, RepositoryBoundaryAcquisitionError>;

    /// Binds one explicitly authorized depth-one nested repository only.
    ///
    /// # Errors
    ///
    /// Returns a typed repository or retained-root stability failure.
    fn bind_nested_repository(
        &self,
        repository: &OsStr,
        identity: RepositoryIdentity,
        revision: Revision,
        profile: NestedAcquisitionProfile,
    ) -> Result<BoundRevision, NestedRepositoryAcquisitionError>;
}

pub trait RustKnowledgeExtractor {
    /// Extracts and validates the approved S2 Rust knowledge subset.
    ///
    /// # Errors
    ///
    /// Returns a typed extraction, ontology, or graph failure without partial
    /// publication.
    fn extract(&self, inventory: &RepositoryInventory) -> Result<RustKnowledge, KnowledgeError>;
}

pub trait RustWorkspaceExtractor {
    /// Extracts the approved deterministic S4 Cargo-workspace subset.
    ///
    /// # Errors
    ///
    /// Returns a typed workspace, parser, ontology, or graph failure without
    /// executing Cargo, rustc, build scripts, or repository code.
    fn extract_workspace(
        &self,
        inventory: &RepositoryInventory,
    ) -> Result<RustWorkspaceKnowledge, WorkspaceError>;
}

pub trait IncrementalRustWorkspaceExtractor {
    /// Extracts S4 workspace knowledge while reusing only exact validated
    /// revision-neutral source analyses.
    ///
    /// # Errors
    ///
    /// Returns a typed workspace, parser, ontology, or graph failure without
    /// executing repository code or accepting an incompatible cache entry.
    fn extract_workspace_incremental(
        &self,
        inventory: &RepositoryInventory,
        cache_entries: &[AnalysisCacheEntry],
    ) -> Result<IncrementalWorkspaceExtraction, WorkspaceError>;
}

pub trait OpenApiContractExtractor {
    /// Parses and normalizes the approved bounded `OpenAPI` 3.1 HTTP/JSON subset.
    ///
    /// # Errors
    ///
    /// Returns a typed encoding, YAML, `OpenAPI`, reference, or contract-limit
    /// failure without network access or partial output.
    fn extract(&self, input: OpenApiContractInput<'_>) -> Result<ProviderContract, ContractError>;
}

pub trait PublicationObserver {
    /// Observes one exact S3 publication boundary occurrence.
    ///
    /// # Errors
    ///
    /// Returns a publication error when the observer cannot record the
    /// boundary. Production uses [`NoopPublicationObserver`].
    fn observe(&mut self, event: &PublicationEvent) -> Result<(), StorageError>;
}

pub struct NoopPublicationObserver;

impl PublicationObserver for NoopPublicationObserver {
    fn observe(&mut self, _event: &PublicationEvent) -> Result<(), StorageError> {
        Ok(())
    }
}

pub trait ArtifactStore {
    /// Stages one immutable exact-byte artifact.
    ///
    /// # Errors
    ///
    /// Returns a typed content, path, or durability failure.
    fn stage(
        &mut self,
        artifact: &StoredArtifact,
        observer: &mut dyn PublicationObserver,
    ) -> Result<(), StorageError>;

    /// Reads and verifies one immutable exact-byte artifact.
    ///
    /// # Errors
    ///
    /// Returns a typed missing or corrupt object failure.
    fn read(&self, artifact_id: &ArtifactId, byte_length: u64) -> Result<Vec<u8>, StorageError>;

    /// Removes only abandoned temporary files and objects absent from the
    /// supplied stable reachability view and the per-object recheck.
    ///
    /// # Errors
    ///
    /// Returns a typed storage failure without deleting a rechecked reachable
    /// object.
    fn sweep(
        &mut self,
        reachable: &BTreeSet<ArtifactId>,
        recheck: &mut dyn FnMut(&ArtifactId) -> Result<bool, StorageError>,
    ) -> Result<SweepResult, StorageError>;
}

pub trait AnalysisCacheStore {
    /// Stages one immutable current-schema analysis-cache entry by its exact
    /// deterministic identity.
    ///
    /// # Errors
    ///
    /// Returns a typed path, durability, or conflicting-byte failure.
    fn stage_entry(
        &mut self,
        analysis_cache_entry_id: &str,
        bytes: &[u8],
    ) -> Result<(), StorageError>;

    /// Loads every immutable cache document in deterministic identity order.
    ///
    /// # Errors
    ///
    /// Returns a typed path or corrupt-layout failure.
    fn load_entries(&self) -> Result<Vec<(String, Vec<u8>)>, StorageError>;
}

pub trait MetadataStore {
    /// Returns the current snapshot identifier without changing store state.
    ///
    /// # Errors
    ///
    /// Returns a typed schema, contention, or metadata-integrity failure.
    fn current_head_id(
        &self,
        repository_identity: &RepositoryIdentity,
    ) -> Result<Option<SnapshotId>, StorageError>;

    /// Atomically inserts immutable rows and compare/sets the visible head.
    ///
    /// # Errors
    ///
    /// Returns a typed contention, conflict, schema, metadata, or publication
    /// failure.
    fn publish(
        &mut self,
        candidate: &PublicationCandidate,
        expected_head: Option<&SnapshotId>,
        observer: &mut dyn PublicationObserver,
    ) -> Result<PublicationResult, StorageError>;

    /// Loads one complete metadata head in a stable read transaction.
    ///
    /// # Errors
    ///
    /// Returns a typed schema or metadata-integrity failure.
    fn load_head(
        &self,
        repository_identity: &RepositoryIdentity,
    ) -> Result<Option<LocalSnapshotHead>, StorageError>;

    /// Returns one stable committed set of every referenced artifact.
    ///
    /// # Errors
    ///
    /// Returns a typed metadata-integrity failure.
    fn referenced_artifacts(&self) -> Result<BTreeSet<ArtifactId>, StorageError>;

    /// Rechecks whether one artifact is currently referenced.
    ///
    /// # Errors
    ///
    /// Returns a typed metadata-integrity failure.
    fn is_artifact_referenced(&self, artifact_id: &ArtifactId) -> Result<bool, StorageError>;
}
