use std::error::Error;
use std::fmt::{self, Display, Formatter};

use codenoesis_domain::s1_boundaries::RepositoryBoundaryReport;
use codenoesis_domain::s4::{S4_TREE_SITTER_EXTRACTOR_VERSION, WorkspaceExtractionChunk};
use codenoesis_domain::s4_r3::{
    R3_EXTRACTION_CONTRACT_VERSION, R3_ONTOLOGY_VERSION, R3_PIPELINE_VERSION,
    R3_WORKSPACE_EXTRACTOR_VERSION, R3_WORKSPACE_PROFILE, RootPackageMember,
    RootPackageWorkspaceError, RootPackageWorkspaceKnowledge, RootPackageWorkspacePlan,
};
use codenoesis_domain::storage::{
    LocalSnapshotHead, PublicationCandidate, SNAPSHOT_SCHEMA_VERSION_V6, StorageComponent,
    StorageError,
};
use codenoesis_domain::{
    AcquisitionError, LimitKind, RepositoryInventory, STANDARD_LOCAL_S1_LIMITS, limit_exceeded,
};
use serde_json::{Value, json};

use super::s1_boundaries::repository_boundary_value;
use super::s4::{
    claim_value, coverage_value, diagnostic_value, entity_value, evidence_value, relationship_value,
};
use super::{
    LimitedVecWriter, PublicationCandidateError, SnapshotEnvelopeV1, inventory_value,
    publication_candidate, semantic_hash,
};

const CONFIGURATION_V3_HASH_DOMAIN: &[u8] = b"codenoesis.configuration.semantic.v3";
const SNAPSHOT_V6_HASH_DOMAIN: &[u8] = b"codenoesis.repository-snapshot.semantic.v6";
const EXTRACTION_V3_HASH_DOMAIN: &[u8] = b"codenoesis.extraction-chunk.semantic.v3";
const GRAPH_V3_HASH_DOMAIN: &[u8] = b"codenoesis.knowledge-graph.semantic.v3";

#[derive(Clone, Debug)]
pub struct CodeNoesisErrorV10 {
    value: Value,
}

impl CodeNoesisErrorV10 {
    #[must_use]
    pub fn invalid_workspace_profile() -> Self {
        Self::new(
            "input.invalid_workspace_profile",
            "input",
            "invalid workspace profile",
            json!({}),
        )
    }

    #[must_use]
    pub fn from_workspace(error: &RootPackageWorkspaceError) -> Option<Self> {
        match error {
            RootPackageWorkspaceError::InvalidManifest { reason, path } => Some(Self::new(
                "extraction.invalid_workspace_manifest",
                "extraction",
                "invalid workspace manifest",
                json!({"reason": reason.as_str(), "path": path}),
            )),
            RootPackageWorkspaceError::MemberConflict { path } => Some(Self::new(
                "extraction.workspace_member_conflict",
                "extraction",
                "conflicting workspace member",
                json!({"path": path}),
            )),
            RootPackageWorkspaceError::TargetConflict {
                path,
                target_kind,
                target_name,
            } => Some(Self::new(
                "extraction.workspace_target_conflict",
                "extraction",
                "conflicting workspace target",
                json!({
                    "path": path,
                    "target_kind": target_kind.as_str(),
                    "target_name": target_name
                }),
            )),
            RootPackageWorkspaceError::LimitExceeded {
                limit,
                maximum,
                observed,
            } => Some(Self::new(
                "extraction.root_package_limit_exceeded",
                "extraction",
                "root package workspace limit exceeded",
                json!({
                    "limit": limit.as_str(),
                    "maximum": maximum,
                    "observed": observed
                }),
            )),
            RootPackageWorkspaceError::ContractInvalid => Some(Self::internal()),
            RootPackageWorkspaceError::Source(_) => None,
        }
    }

    #[must_use]
    pub fn internal() -> Self {
        Self::new(
            "internal.unexpected",
            "internal",
            "internal error",
            json!({}),
        )
    }

    /// Serializes one strict `ErrorV10` followed by one LF.
    ///
    /// # Errors
    ///
    /// Returns a serialization error only if the internal document is invalid.
    pub fn canonical_stderr(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = serde_json::to_vec(&self.value)?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    #[allow(clippy::needless_pass_by_value)]
    fn new(code: &str, stage: &str, message: &str, context: Value) -> Self {
        Self {
            value: json!({
                "schema_version": "codenoesis.error/v10",
                "code": code,
                "stage": stage,
                "message": message,
                "retryable": false,
                "context": context
            }),
        }
    }
}

#[derive(Clone, Debug)]
pub struct RepositorySnapshotV6 {
    value: Value,
}

#[derive(Debug)]
pub enum RepositorySnapshotV6Error {
    Serialization(serde_json::Error),
    LimitExceeded(AcquisitionError),
    ContractInvalid,
    OutputLengthOverflow,
}

impl Display for RepositorySnapshotV6Error {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Serialization(_) => "R3 snapshot serialization failed",
            Self::LimitExceeded(_) => "R3 snapshot output limit exceeded",
            Self::ContractInvalid => "R3 snapshot contract is invalid",
            Self::OutputLengthOverflow => "R3 snapshot output length overflowed",
        })
    }
}

impl Error for RepositorySnapshotV6Error {}

impl RepositorySnapshotV6 {
    /// Builds the strict additive R3 snapshot over one validated immutable inventory.
    ///
    /// # Errors
    ///
    /// Returns a plan/graph contract or serialization failure.
    pub fn from_inventory_and_workspace(
        inventory: &RepositoryInventory,
        workspace: &RootPackageWorkspaceKnowledge,
        boundaries: Option<&RepositoryBoundaryReport>,
        envelope: SnapshotEnvelopeV1,
    ) -> Result<Self, RepositorySnapshotV6Error> {
        workspace
            .validate()
            .map_err(|_| RepositorySnapshotV6Error::ContractInvalid)?;
        if boundaries.is_some_and(|report| report.root_repository != *inventory.bound_revision()) {
            return Err(RepositorySnapshotV6Error::ContractInvalid);
        }
        let SnapshotEnvelopeV1 {
            created_at,
            job_id,
            correlation_id,
        } = envelope;
        let boundary_profile = boundaries.map(|_| "local-gitlinks-v1");
        let configuration_without_hash = json!({
            "schema_version": "codenoesis.configuration/v3",
            "profile": "standard-local-s4",
            "workspace_profile": R3_WORKSPACE_PROFILE,
            "repository_boundary_profile": boundary_profile
        });
        let configuration_hash =
            semantic_hash(CONFIGURATION_V3_HASH_DOMAIN, &configuration_without_hash);
        let mut configuration = configuration_without_hash;
        configuration
            .as_object_mut()
            .ok_or(RepositorySnapshotV6Error::ContractInvalid)?
            .insert(
                "semantic_hash".to_owned(),
                json!({"algorithm": "blake3-256", "value": configuration_hash}),
            );
        let extraction_chunks = workspace
            .knowledge
            .extraction_chunks
            .iter()
            .map(|chunk| extraction_chunk_v3(chunk, &workspace.plan))
            .collect::<Result<Vec<_>, _>>()?;
        let graph = knowledge_graph_v3(inventory, workspace)?;
        let bound = inventory.bound_revision();
        let mut semantic = json!({
            "repository": repository_value(inventory),
            "configuration": configuration,
            "pipeline_version": R3_PIPELINE_VERSION,
            "ontology_version": R3_ONTOLOGY_VERSION,
            "extractor_contract_version": R3_EXTRACTION_CONTRACT_VERSION,
            "extractor_versions": [
                "codenoesis.inventory-classifier/s1-v1",
                S4_TREE_SITTER_EXTRACTOR_VERSION,
                R3_WORKSPACE_EXTRACTOR_VERSION
            ],
            "evidence_lineage_version": "codenoesis.evidence-lineage/v2",
            "inventory": inventory_value(inventory),
            "extraction_chunks": extraction_chunks,
            "knowledge_graph": graph
        });
        if let Some(report) = boundaries {
            let semantic_object = semantic
                .as_object_mut()
                .ok_or(RepositorySnapshotV6Error::ContractInvalid)?;
            semantic_object.insert(
                "extractor_versions".to_owned(),
                json!([
                    "codenoesis.inventory-classifier/s1-v1",
                    S4_TREE_SITTER_EXTRACTOR_VERSION,
                    R3_WORKSPACE_EXTRACTOR_VERSION,
                    "codenoesis.git-boundary/s1-v1"
                ]),
            );
            semantic_object.insert(
                "repository_boundaries".to_owned(),
                repository_boundary_value(report),
            );
        }
        let snapshot_hash = semantic_hash(SNAPSHOT_V6_HASH_DOMAIN, &semantic);
        let snapshot = Self {
            value: json!({
                "schema_version": SNAPSHOT_SCHEMA_VERSION_V6,
                "semantic_hash": {"algorithm": "blake3-256", "value": snapshot_hash},
                "semantic": semantic,
                "envelope": {
                    "created_at": created_at,
                    "job_id": job_id,
                    "correlation_id": correlation_id
                }
            }),
        };
        if snapshot.value["semantic"]["repository"]["identity"]
            != bound.repository_identity().as_str()
        {
            return Err(RepositorySnapshotV6Error::ContractInvalid);
        }
        Ok(snapshot)
    }

    /// Serializes the complete V6 snapshot with the inherited output bound.
    ///
    /// # Errors
    ///
    /// Returns a typed serialization or output-limit failure.
    pub fn canonical_stdout(&self) -> Result<Vec<u8>, RepositorySnapshotV6Error> {
        let maximum = usize::try_from(STANDARD_LOCAL_S1_LIMITS.canonical_output_bytes)
            .map_err(|_| RepositorySnapshotV6Error::OutputLengthOverflow)?;
        let body_maximum = maximum
            .checked_sub(1)
            .ok_or(RepositorySnapshotV6Error::OutputLengthOverflow)?;
        let mut writer = LimitedVecWriter::new(body_maximum);
        let result = serde_json::to_writer(&mut writer, &self.value);
        if writer.overflowed() {
            return Err(RepositorySnapshotV6Error::LimitExceeded(limit_exceeded(
                LimitKind::CanonicalOutputBytes,
                STANDARD_LOCAL_S1_LIMITS
                    .canonical_output_bytes
                    .saturating_add(1),
            )));
        }
        result.map_err(RepositorySnapshotV6Error::Serialization)?;
        let mut bytes = writer.into_inner();
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Serializes the exact V6 semantic payload stored by S4.
    ///
    /// # Errors
    ///
    /// Returns a serialization failure only for an invalid internal value.
    pub fn canonical_semantic(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(&self.value["semantic"])
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Converts V6 into the unchanged immutable publication model.
    ///
    /// # Errors
    ///
    /// Returns a strict contract or storage-integrity failure.
    pub fn publication_candidate(&self) -> Result<PublicationCandidate, PublicationCandidateError> {
        publication_candidate(&self.value)
    }
}

/// Validates one loaded V6 semantic payload against the complete visible head.
///
/// # Errors
///
/// Returns a typed metadata-integrity failure on any binding mismatch.
pub fn validate_stored_snapshot_semantic_v6(
    semantic: &Value,
    head: &LocalSnapshotHead,
) -> Result<(), StorageError> {
    if head.snapshot_schema_version != SNAPSHOT_SCHEMA_VERSION_V6 {
        return Err(stored_snapshot_error(
            head,
            "stored_snapshot_schema_mismatch",
        ));
    }
    let value = json!({
        "schema_version": head.snapshot_schema_version,
        "semantic_hash": {
            "algorithm": head.semantic_hash.algorithm,
            "value": head.semantic_hash.value
        },
        "semantic": semantic
    });
    let candidate = publication_candidate(&value)
        .map_err(|_| stored_snapshot_error(head, "stored_snapshot_contract_invalid"))?;
    if candidate.snapshot.repository_identity != head.repository_identity
        || candidate.snapshot.snapshot_id != head.snapshot_id
        || candidate.snapshot.commit_oid != head.commit_oid
        || candidate.snapshot.snapshot_schema_version != head.snapshot_schema_version
        || candidate.snapshot.semantic_hash != head.semantic_hash
        || candidate.snapshot.graph_semantic_hash != head.graph_semantic_hash
        || candidate.artifact_references() != head.artifacts
    {
        return Err(stored_snapshot_error(head, "stored_snapshot_head_mismatch"));
    }
    Ok(())
}

fn extraction_chunk_v3(
    chunk: &WorkspaceExtractionChunk,
    plan: &RootPackageWorkspacePlan,
) -> Result<Value, RepositorySnapshotV6Error> {
    let target = plan
        .target(&chunk.crate_id)
        .ok_or(RepositorySnapshotV6Error::ContractInvalid)?;
    let mut value = json!({
        "schema_version": "codenoesis.extraction-chunk/v3",
        "ontology_version": R3_ONTOLOGY_VERSION,
        "repository_identity": chunk.repository_identity,
        "crate_id": chunk.crate_id,
        "source_file_id": chunk.source_file_id,
        "workspace": {
            "root_shape": plan.root_shape.as_str(),
            "member_path": target.member_path,
            "member_source": target.member_source.as_str(),
            "manifest_path": target.manifest_path,
            "target_kind": target.target_kind.as_str(),
            "target_name": target.target_name
        },
        "entities": chunk.entities.iter().map(entity_value).collect::<Vec<_>>(),
        "relationships": chunk.relationships.iter().map(relationship_value).collect::<Vec<_>>(),
        "claims": chunk.claims.iter().map(claim_value).collect::<Vec<_>>(),
        "evidence": chunk.evidence.iter().map(evidence_value).collect::<Vec<_>>(),
        "diagnostics": chunk.diagnostics.iter().map(diagnostic_value).collect::<Vec<_>>(),
        "coverage": chunk.coverage.iter().map(coverage_value).collect::<Vec<_>>()
    });
    insert_semantic_hash(&mut value, EXTRACTION_V3_HASH_DOMAIN)?;
    Ok(value)
}

fn knowledge_graph_v3(
    inventory: &RepositoryInventory,
    workspace: &RootPackageWorkspaceKnowledge,
) -> Result<Value, RepositorySnapshotV6Error> {
    let graph = &workspace.knowledge.graph;
    let mut value = json!({
        "schema_version": "codenoesis.knowledge-graph/v3",
        "ontology_version": R3_ONTOLOGY_VERSION,
        "repository": repository_value(inventory),
        "extractor_versions": [
            S4_TREE_SITTER_EXTRACTOR_VERSION,
            R3_WORKSPACE_EXTRACTOR_VERSION
        ],
        "workspace": workspace_value(&workspace.plan),
        "entities": graph.entities.iter().map(entity_value).collect::<Vec<_>>(),
        "relationships": graph.relationships.iter().map(relationship_value).collect::<Vec<_>>(),
        "claims": graph.claims.iter().map(claim_value).collect::<Vec<_>>(),
        "evidence": graph.evidence.iter().map(evidence_value).collect::<Vec<_>>(),
        "diagnostics": graph.diagnostics.iter().map(diagnostic_value).collect::<Vec<_>>(),
        "coverage": graph.coverage.iter().map(coverage_value).collect::<Vec<_>>()
    });
    insert_semantic_hash(&mut value, GRAPH_V3_HASH_DOMAIN)?;
    Ok(value)
}

fn workspace_value(plan: &RootPackageWorkspacePlan) -> Value {
    json!({
        "root_shape": plan.root_shape.as_str(),
        "members": plan.members.iter().map(member_value).collect::<Vec<_>>(),
        "excluded_paths": plan.excluded_paths
    })
}

fn member_value(member: &RootPackageMember) -> Value {
    json!({
        "path": member.path,
        "manifest_path": member.manifest_path,
        "member_source": member.member_source.as_str(),
        "crate_ids": member.crate_ids,
        "external_boundary_id": member.external_boundary_id
    })
}

fn repository_value(inventory: &RepositoryInventory) -> Value {
    let bound = inventory.bound_revision();
    json!({
        "contract_version": "codenoesis.repository/v1",
        "identity_schema_version": "codenoesis.repository-identity/v1",
        "identity": bound.repository_identity().as_str(),
        "vcs": "git",
        "object_format": "sha1",
        "commit_oid": bound.commit_oid().as_str(),
        "tree_oid": bound.tree_oid().as_str()
    })
}

fn insert_semantic_hash(value: &mut Value, domain: &[u8]) -> Result<(), RepositorySnapshotV6Error> {
    let hash = semantic_hash(domain, value);
    let object = value
        .as_object_mut()
        .ok_or(RepositorySnapshotV6Error::ContractInvalid)?;
    object.insert(
        "semantic_hash".to_owned(),
        json!({"algorithm": "blake3-256", "value": hash}),
    );
    Ok(())
}

fn stored_snapshot_error(head: &LocalSnapshotHead, reason: &'static str) -> StorageError {
    StorageError::CorruptMetadata {
        component: StorageComponent::Head,
        reason,
        snapshot_id: Some(head.snapshot_id.to_string()),
    }
}
