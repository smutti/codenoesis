//! Application orchestration for the `CodeNoesis` S0 through S6 slices.

mod s1_boundaries;
mod s4;
mod s4_k1;
mod s4_r3;
mod s4_r4;
mod s4_r5;
mod s4_r6;
mod s4_r7;
mod s5;
mod s6;

pub use s1_boundaries::{
    BoundaryScanError, PreparedNestedRepositoryRoot, RepositoryBoundaryScanInput,
    S4BoundaryScanOutput,
};
pub use s4::S4ScanOutput;
pub use s4_k1::{CallableSemanticsScanError, S4K1ScanOutput};
pub use s4_r3::{RootPackageScanError, S4R3ScanOutput};
pub use s4_r4::{CargoManifestScanError, S4R4ScanOutput};
pub use s4_r5::{RustSemanticScanError, S4R5ScanOutput};
pub use s4_r6::{FrameworkScanError, S4R6ScanOutput};
pub use s4_r7::{CompilerIndexScanError, S4R7ScanOutput};
pub use s5::{RefreshError, RefreshPlan, RefreshService};
pub use s6::{FederationRequest, FederationService, FederationServiceError};

use std::ffi::OsString;

use codenoesis_contracts::{
    RepositorySnapshotV1, RepositorySnapshotV2, RepositorySnapshotV3, SnapshotEnvelopeV1,
    validate_head_artifact,
};
use codenoesis_domain::knowledge::KnowledgeError;
use codenoesis_domain::s4::WorkspaceError;
use codenoesis_domain::storage::{
    LocalSnapshotHead, PublicationCandidate, StorageError, SweepResult,
};
use codenoesis_domain::{
    AcquisitionError, RepositoryError, RepositoryIdentity, RepositoryInventory, Revision,
};
use codenoesis_ports::{
    ArtifactStore, MetadataStore, PublicationObserver, RepositoryAcquirer, RustKnowledgeExtractor,
    SafeRepositoryAcquirer,
};

pub struct ScanRequest {
    repository: OsString,
    identity: RepositoryIdentity,
    revision: Revision,
    envelope: SnapshotEnvelopeV1,
}

impl ScanRequest {
    #[must_use]
    pub const fn new(
        repository: OsString,
        identity: RepositoryIdentity,
        revision: Revision,
        envelope: SnapshotEnvelopeV1,
    ) -> Self {
        Self {
            repository,
            identity,
            revision,
            envelope,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScanError {
    Acquisition(AcquisitionError),
    Knowledge(KnowledgeError),
    Workspace(WorkspaceError),
    Storage(StorageError),
    Internal,
}

pub struct ScanService<A> {
    acquirer: A,
}

pub struct PublicationService;

impl PublicationService {
    /// Stages every exact-byte artifact, atomically compare/sets metadata, and
    /// validates the complete visible head before returning.
    ///
    /// # Errors
    ///
    /// Returns a typed storage/publication failure or a redacted internal
    /// contract failure.
    pub fn publish<C, M>(
        snapshot: &RepositorySnapshotV3,
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

    fn publish_candidate<C, M>(
        candidate: &PublicationCandidate,
        artifact_store: &mut C,
        metadata_store: &mut M,
        observer: &mut dyn PublicationObserver,
    ) -> Result<LocalSnapshotHead, ScanError>
    where
        C: ArtifactStore,
        M: MetadataStore,
    {
        let expected_head = metadata_store
            .current_head_id(&candidate.snapshot.repository_identity)
            .map_err(ScanError::Storage)?;
        Self::publish_candidate_with_expected(
            candidate,
            expected_head.as_ref(),
            artifact_store,
            metadata_store,
            observer,
        )
    }

    fn publish_candidate_with_expected<C, M>(
        candidate: &PublicationCandidate,
        expected_head: Option<&codenoesis_domain::storage::SnapshotId>,
        artifact_store: &mut C,
        metadata_store: &mut M,
        observer: &mut dyn PublicationObserver,
    ) -> Result<LocalSnapshotHead, ScanError>
    where
        C: ArtifactStore,
        M: MetadataStore,
    {
        if expected_head == Some(&candidate.snapshot.snapshot_id) {
            let current = Self::load_head(
                &candidate.snapshot.repository_identity,
                artifact_store,
                metadata_store,
            )?
            .ok_or_else(|| {
                ScanError::Storage(StorageError::CorruptMetadata {
                    component: codenoesis_domain::storage::StorageComponent::Head,
                    reason: "current_head_missing",
                    snapshot_id: Some(candidate.snapshot.snapshot_id.to_string()),
                })
            })?;
            if current.snapshot_id != candidate.snapshot.snapshot_id {
                return Err(ScanError::Storage(StorageError::CorruptMetadata {
                    component: codenoesis_domain::storage::StorageComponent::Head,
                    reason: "current_head_mismatch",
                    snapshot_id: Some(candidate.snapshot.snapshot_id.to_string()),
                }));
            }
        }
        for artifact in &candidate.artifacts {
            artifact_store
                .stage(artifact, observer)
                .map_err(ScanError::Storage)?;
        }
        let published = metadata_store
            .publish(candidate, expected_head, observer)
            .map_err(ScanError::Storage)?;
        let head = Self::load_head(
            &candidate.snapshot.repository_identity,
            artifact_store,
            metadata_store,
        )?
        .ok_or_else(|| {
            ScanError::Storage(StorageError::CorruptMetadata {
                component: codenoesis_domain::storage::StorageComponent::Head,
                reason: "published_head_missing",
                snapshot_id: Some(candidate.snapshot.snapshot_id.to_string()),
            })
        })?;
        if head != published.head || head.snapshot_id != candidate.snapshot.snapshot_id {
            return Err(ScanError::Storage(StorageError::CorruptMetadata {
                component: codenoesis_domain::storage::StorageComponent::Head,
                reason: "published_head_mismatch",
                snapshot_id: Some(candidate.snapshot.snapshot_id.to_string()),
            }));
        }
        Ok(head)
    }

    /// Loads and verifies one complete visible head and every referenced
    /// immutable artifact.
    ///
    /// # Errors
    ///
    /// Returns a typed metadata or content-integrity failure.
    pub fn load_head<C, M>(
        repository_identity: &RepositoryIdentity,
        artifact_store: &C,
        metadata_store: &M,
    ) -> Result<Option<LocalSnapshotHead>, ScanError>
    where
        C: ArtifactStore,
        M: MetadataStore,
    {
        let Some(head) = metadata_store
            .load_head(repository_identity)
            .map_err(ScanError::Storage)?
        else {
            return Ok(None);
        };
        for reference in &head.artifacts {
            let bytes = artifact_store
                .read(&reference.artifact_id, reference.byte_length)
                .map_err(ScanError::Storage)?;
            validate_head_artifact(reference, &bytes).map_err(ScanError::Storage)?;
        }
        Ok(Some(head))
    }

    /// Sweeps only temporary and final objects proven unreachable by a stable
    /// metadata view and a per-object recheck.
    ///
    /// # Errors
    ///
    /// Returns a typed metadata or filesystem failure.
    pub fn sweep<C, M>(artifact_store: &mut C, metadata_store: &M) -> Result<SweepResult, ScanError>
    where
        C: ArtifactStore,
        M: MetadataStore,
    {
        let reachable = metadata_store
            .referenced_artifacts()
            .map_err(ScanError::Storage)?;
        artifact_store
            .sweep(&reachable, &mut |artifact_id| {
                metadata_store.is_artifact_referenced(artifact_id)
            })
            .map_err(ScanError::Storage)
    }
}

impl<A> ScanService<A>
where
    A: RepositoryAcquirer,
{
    #[must_use]
    pub const fn new(acquirer: A) -> Self {
        Self { acquirer }
    }

    /// Executes an S0 scan from one immutable repository binding.
    ///
    /// # Errors
    ///
    /// Returns a typed acquisition failure or a redacted internal failure.
    pub fn scan(&self, request: ScanRequest) -> Result<RepositorySnapshotV1, ScanError> {
        let bound = self
            .acquirer
            .bind(&request.repository, request.identity, request.revision)
            .map_err(map_repository_error)?;
        Ok(RepositorySnapshotV1::from_bound_revision(
            &bound,
            request.envelope,
        ))
    }
}

impl<A> ScanService<A>
where
    A: SafeRepositoryAcquirer,
{
    /// Executes an S1 safe-inventory scan from one immutable repository binding.
    ///
    /// # Errors
    ///
    /// Returns a typed acquisition or policy failure, or a redacted internal failure.
    pub fn scan_s1(&self, request: ScanRequest) -> Result<RepositorySnapshotV2, ScanError> {
        let acquired = self
            .acquirer
            .acquire_inventory(&request.repository, request.identity, request.revision)
            .map_err(map_repository_error)?;
        let inventory = RepositoryInventory::classify(acquired);
        Ok(RepositorySnapshotV2::from_inventory(
            &inventory,
            request.envelope,
        ))
    }

    /// Executes an S2 Rust-knowledge scan over the approved S1 acquisition.
    ///
    /// # Errors
    ///
    /// Returns a typed acquisition, extraction, ontology, or graph failure, or
    /// a redacted internal failure.
    pub fn scan_s2<E>(
        &self,
        request: ScanRequest,
        extractor: &E,
    ) -> Result<RepositorySnapshotV3, ScanError>
    where
        E: RustKnowledgeExtractor,
    {
        let acquired = self
            .acquirer
            .acquire_inventory(&request.repository, request.identity, request.revision)
            .map_err(map_repository_error)?;
        let inventory = RepositoryInventory::classify(acquired);
        let knowledge = extractor
            .extract(&inventory)
            .map_err(ScanError::Knowledge)?;
        knowledge.validate().map_err(ScanError::Knowledge)?;
        Ok(RepositorySnapshotV3::from_inventory_and_knowledge(
            &inventory,
            &knowledge,
            request.envelope,
        ))
    }
}

pub(crate) fn map_repository_error(error: RepositoryError) -> ScanError {
    match error {
        RepositoryError::Acquisition(acquisition) => ScanError::Acquisition(acquisition),
        RepositoryError::Unexpected => ScanError::Internal,
    }
}
