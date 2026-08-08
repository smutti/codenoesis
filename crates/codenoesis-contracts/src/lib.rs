//! Versioned JSON contracts for the `CodeNoesis` S0 through S3 slices.

mod s1_boundaries;
mod s1_packed;
mod s4;
mod s4_r3;
mod s4_r4;
mod s4_r5;
mod s4_r6;
mod s4_r7;
mod s4_r8;
mod s5;
mod s6;

pub use s1_boundaries::*;
pub use s1_packed::*;
pub use s4::*;
pub use s4_r3::*;
pub use s4_r4::*;
pub use s4_r5::*;
pub use s4_r6::*;
pub use s4_r7::*;
pub use s4_r8::*;
pub use s5::*;
pub use s6::*;

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io::{self, Write};

use codenoesis_domain::knowledge::{
    ByteSpan, CONTAINMENT_RULE_VERSION, ClaimDerivation, CoverageGap, EXTRACTOR_VERSION,
    ExtractionChunk, ExtractionCoverage, ExtractionDiagnostic, KnowledgeClaim, KnowledgeEntity,
    KnowledgeError, KnowledgeGraph, KnowledgeRelationship, ONTOLOGY_VERSION, RustKnowledge,
    SourceEvidence,
};
use codenoesis_domain::storage::{
    ArtifactReference, ArtifactRole, ClaimRow, EntityRow, EvidenceRow, ExtractionRow,
    LocalSnapshotHead, OrdinalRow, PublicationCandidate, RelationshipRow,
    SemanticHash as StoredSemanticHash, SnapshotId, SnapshotRecord, StorageComponent, StorageError,
    StoredArtifact,
};
use codenoesis_domain::{
    AcquisitionError, BoundRevision, InputError, InventoryFile, InventoryLanguage, LimitKind,
    ObjectId, RecognizedInventoryKind, RepositoryIdentity, RepositoryInventory,
    STANDARD_LOCAL_S1_LIMITS, limit_exceeded,
};
use serde_json::{Value, json};

const CONFIGURATION_HASH_DOMAIN: &[u8] = b"codenoesis.configuration.semantic.v1";
const SNAPSHOT_HASH_DOMAIN: &[u8] = b"codenoesis.repository-snapshot.semantic.v1";
const SNAPSHOT_V2_HASH_DOMAIN: &[u8] = b"codenoesis.repository-snapshot.semantic.v2";
const SNAPSHOT_V3_HASH_DOMAIN: &[u8] = b"codenoesis.repository-snapshot.semantic.v3";
const EXTRACTION_HASH_DOMAIN: &[u8] = b"codenoesis.extraction-chunk.semantic.v1";
const GRAPH_HASH_DOMAIN: &[u8] = b"codenoesis.knowledge-graph.semantic.v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotEnvelopeV1 {
    created_at: String,
    job_id: Option<String>,
    correlation_id: String,
}

impl SnapshotEnvelopeV1 {
    #[must_use]
    pub const fn new(created_at: String, job_id: Option<String>, correlation_id: String) -> Self {
        Self {
            created_at,
            job_id,
            correlation_id,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RepositorySnapshotV1 {
    value: Value,
}

impl RepositorySnapshotV1 {
    #[must_use]
    pub fn from_bound_revision(bound: &BoundRevision, envelope: SnapshotEnvelopeV1) -> Self {
        let SnapshotEnvelopeV1 {
            created_at,
            job_id,
            correlation_id,
        } = envelope;
        let configuration_semantic = json!({"profile": "standard-local-s0"});
        let configuration_hash = semantic_hash(CONFIGURATION_HASH_DOMAIN, &configuration_semantic);
        let semantic = json!({
            "repository": {
                "contract_version": "codenoesis.repository/v1",
                "identity_schema_version": "codenoesis.repository-identity/v1",
                "identity": bound.repository_identity().as_str(),
                "vcs": "git",
                "object_format": "sha1",
                "commit_oid": bound.commit_oid().as_str(),
                "tree_oid": bound.tree_oid().as_str()
            },
            "configuration": {
                "schema_version": "codenoesis.configuration/v1",
                "profile": "standard-local-s0",
                "semantic_hash": {
                    "algorithm": "blake3-256",
                    "value": configuration_hash
                }
            },
            "pipeline_version": "codenoesis.pipeline/s0-v1",
            "ontology_version": "codenoesis.ontology/none-v1",
            "extractor_contract_version": "codenoesis.extraction/v1",
            "extractor_versions": [],
            "evidence_lineage_version": "codenoesis.evidence-lineage/v1"
        });
        let snapshot_hash = semantic_hash(SNAPSHOT_HASH_DOMAIN, &semantic);
        Self {
            value: json!({
                "schema_version": "codenoesis.repository-snapshot/v1",
                "semantic_hash": {
                    "algorithm": "blake3-256",
                    "value": snapshot_hash
                },
                "semantic": semantic,
                "envelope": {
                    "created_at": created_at,
                    "job_id": job_id,
                    "correlation_id": correlation_id
                }
            }),
        }
    }

    /// Serializes the complete snapshot as RFC 8785-compatible S0 bytes.
    ///
    /// # Errors
    ///
    /// Returns an error only if the internally constructed JSON value cannot be
    /// serialized.
    pub fn canonical_stdout(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = serde_json::to_vec(&self.value)?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Serializes the semantic value as RFC 8785-compatible S0 bytes.
    ///
    /// # Errors
    ///
    /// Returns an error only if the internally constructed JSON value cannot be
    /// serialized.
    pub fn canonical_semantic(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(&self.value["semantic"])
    }

    #[must_use]
    pub fn value(&self) -> &Value {
        &self.value
    }
}

#[derive(Clone, Debug)]
pub struct RepositorySnapshotV2 {
    value: Value,
}

#[derive(Debug)]
pub enum RepositorySnapshotV2Error {
    Serialization(serde_json::Error),
    LimitExceeded(AcquisitionError),
    OutputLengthOverflow,
}

impl RepositorySnapshotV2 {
    #[must_use]
    pub fn from_inventory(inventory: &RepositoryInventory, envelope: SnapshotEnvelopeV1) -> Self {
        let SnapshotEnvelopeV1 {
            created_at,
            job_id,
            correlation_id,
        } = envelope;
        let configuration_semantic = json!({"profile": "standard-local-s1"});
        let configuration_hash = semantic_hash(CONFIGURATION_HASH_DOMAIN, &configuration_semantic);
        let bound = inventory.bound_revision();
        let semantic = json!({
            "repository": {
                "contract_version": "codenoesis.repository/v1",
                "identity_schema_version": "codenoesis.repository-identity/v1",
                "identity": bound.repository_identity().as_str(),
                "vcs": "git",
                "object_format": "sha1",
                "commit_oid": bound.commit_oid().as_str(),
                "tree_oid": bound.tree_oid().as_str()
            },
            "configuration": {
                "schema_version": "codenoesis.configuration/v1",
                "profile": "standard-local-s1",
                "semantic_hash": {
                    "algorithm": "blake3-256",
                    "value": configuration_hash
                }
            },
            "pipeline_version": "codenoesis.pipeline/s1-v1",
            "ontology_version": "codenoesis.ontology/none-v1",
            "extractor_contract_version": "codenoesis.extraction/v1",
            "extractor_versions": ["codenoesis.inventory-classifier/s1-v1"],
            "evidence_lineage_version": "codenoesis.evidence-lineage/v1",
            "inventory": inventory_value(inventory)
        });
        let snapshot_hash = semantic_hash(SNAPSHOT_V2_HASH_DOMAIN, &semantic);
        Self {
            value: json!({
                "schema_version": "codenoesis.repository-snapshot/v2",
                "semantic_hash": {
                    "algorithm": "blake3-256",
                    "value": snapshot_hash
                },
                "semantic": semantic,
                "envelope": {
                    "created_at": created_at,
                    "job_id": job_id,
                    "correlation_id": correlation_id
                }
            }),
        }
    }

    /// Serializes the complete S1 snapshot and enforces the public output limit.
    ///
    /// # Errors
    ///
    /// Returns [`AcquisitionError::LimitExceeded`] when the LF-terminated
    /// canonical document would exceed the fixed S1 output limit.
    pub fn canonical_stdout(&self) -> Result<Vec<u8>, RepositorySnapshotV2Error> {
        let maximum = usize::try_from(STANDARD_LOCAL_S1_LIMITS.canonical_output_bytes)
            .map_err(|_| RepositorySnapshotV2Error::OutputLengthOverflow)?;
        let body_maximum = maximum
            .checked_sub(1)
            .ok_or(RepositorySnapshotV2Error::OutputLengthOverflow)?;
        let mut writer = LimitedVecWriter::new(body_maximum);
        let result = serde_json::to_writer(&mut writer, &self.value);
        if writer.overflowed() {
            return Err(RepositorySnapshotV2Error::LimitExceeded(limit_exceeded(
                LimitKind::CanonicalOutputBytes,
                STANDARD_LOCAL_S1_LIMITS
                    .canonical_output_bytes
                    .saturating_add(1),
            )));
        }
        result.map_err(RepositorySnapshotV2Error::Serialization)?;
        let mut bytes = writer.into_inner();
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Serializes the complete S1 semantic value.
    ///
    /// # Errors
    ///
    /// Returns a serialization error if the internal JSON value cannot be encoded.
    pub fn canonical_semantic(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(&self.value["semantic"])
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }
}

#[derive(Clone, Debug)]
pub struct RepositorySnapshotV3 {
    value: Value,
}

#[derive(Debug)]
pub enum RepositorySnapshotV3Error {
    Serialization(serde_json::Error),
    LimitExceeded(AcquisitionError),
    OutputLengthOverflow,
}

#[derive(Debug)]
pub enum PublicationCandidateError {
    InvalidContract(&'static str),
    Serialization(serde_json::Error),
    Storage(StorageError),
}

impl Display for PublicationCandidateError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidContract(field) => write!(formatter, "invalid V3 field: {field}"),
            Self::Serialization(error) => Display::fmt(error, formatter),
            Self::Storage(error) => Display::fmt(error, formatter),
        }
    }
}

impl Error for PublicationCandidateError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Serialization(error) => Some(error),
            Self::Storage(error) => Some(error),
            Self::InvalidContract(_) => None,
        }
    }
}

impl RepositorySnapshotV3 {
    #[must_use]
    pub fn from_inventory_and_knowledge(
        inventory: &RepositoryInventory,
        knowledge: &RustKnowledge,
        envelope: SnapshotEnvelopeV1,
    ) -> Self {
        let SnapshotEnvelopeV1 {
            created_at,
            job_id,
            correlation_id,
        } = envelope;
        let configuration_semantic = json!({"profile": "standard-local-s2"});
        let configuration_hash = semantic_hash(CONFIGURATION_HASH_DOMAIN, &configuration_semantic);
        let bound = inventory.bound_revision();
        let extraction_chunks = knowledge
            .extraction_chunks
            .iter()
            .map(extraction_chunk_value)
            .collect::<Vec<_>>();
        let graph = knowledge_graph_value(&knowledge.graph);
        let semantic = json!({
            "repository": {
                "contract_version": "codenoesis.repository/v1",
                "identity_schema_version": "codenoesis.repository-identity/v1",
                "identity": bound.repository_identity().as_str(),
                "vcs": "git",
                "object_format": "sha1",
                "commit_oid": bound.commit_oid().as_str(),
                "tree_oid": bound.tree_oid().as_str()
            },
            "configuration": {
                "schema_version": "codenoesis.configuration/v1",
                "profile": "standard-local-s2",
                "semantic_hash": {
                    "algorithm": "blake3-256",
                    "value": configuration_hash
                }
            },
            "pipeline_version": "codenoesis.pipeline/s2-v1",
            "ontology_version": ONTOLOGY_VERSION,
            "extractor_contract_version": "codenoesis.extraction-chunk/v1",
            "extractor_versions": [
                "codenoesis.inventory-classifier/s1-v1",
                EXTRACTOR_VERSION
            ],
            "evidence_lineage_version": "codenoesis.evidence-lineage/v2",
            "inventory": inventory_value(inventory),
            "extraction_chunks": extraction_chunks,
            "knowledge_graph": graph
        });
        let snapshot_hash = semantic_hash(SNAPSHOT_V3_HASH_DOMAIN, &semantic);
        Self {
            value: json!({
                "schema_version": "codenoesis.repository-snapshot/v3",
                "semantic_hash": {
                    "algorithm": "blake3-256",
                    "value": snapshot_hash
                },
                "semantic": semantic,
                "envelope": {
                    "created_at": created_at,
                    "job_id": job_id,
                    "correlation_id": correlation_id
                }
            }),
        }
    }

    /// Serializes the complete S2 snapshot and enforces the inherited output
    /// limit.
    ///
    /// # Errors
    ///
    /// Returns a typed output-limit or serialization failure.
    pub fn canonical_stdout(&self) -> Result<Vec<u8>, RepositorySnapshotV3Error> {
        let maximum = usize::try_from(STANDARD_LOCAL_S1_LIMITS.canonical_output_bytes)
            .map_err(|_| RepositorySnapshotV3Error::OutputLengthOverflow)?;
        let body_maximum = maximum
            .checked_sub(1)
            .ok_or(RepositorySnapshotV3Error::OutputLengthOverflow)?;
        let mut writer = LimitedVecWriter::new(body_maximum);
        let result = serde_json::to_writer(&mut writer, &self.value);
        if writer.overflowed() {
            return Err(RepositorySnapshotV3Error::LimitExceeded(limit_exceeded(
                LimitKind::CanonicalOutputBytes,
                STANDARD_LOCAL_S1_LIMITS
                    .canonical_output_bytes
                    .saturating_add(1),
            )));
        }
        result.map_err(RepositorySnapshotV3Error::Serialization)?;
        let mut bytes = writer.into_inner();
        bytes.push(b'\n');
        Ok(bytes)
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Converts the validated V3 document into the exact immutable S3
    /// publication candidate.
    ///
    /// # Errors
    ///
    /// Returns a strict contract, serialization, or storage-identity failure.
    pub fn publication_candidate(&self) -> Result<PublicationCandidate, PublicationCandidateError> {
        publication_candidate(&self.value)
    }
}

#[derive(Clone, Debug)]
pub struct LocalSnapshotHeadV1 {
    value: Value,
}

impl LocalSnapshotHeadV1 {
    #[must_use]
    pub fn from_head(head: &LocalSnapshotHead) -> Self {
        let artifacts = head
            .artifacts
            .iter()
            .map(artifact_reference_value)
            .collect::<Vec<_>>();
        Self {
            value: json!({
                "schema_version": "codenoesis.local-snapshot-head/v1",
                "repository_identity": head.repository_identity.as_str(),
                "snapshot_id": head.snapshot_id.as_str(),
                "commit_oid": head.commit_oid.as_str(),
                "snapshot_schema_version": head.snapshot_schema_version,
                "semantic_hash": semantic_hash_value(&head.semantic_hash),
                "graph_semantic_hash": semantic_hash_value(&head.graph_semantic_hash),
                "generation": head.generation,
                "artifacts": artifacts
            }),
        }
    }

    /// Serializes one strict local head followed by one LF.
    ///
    /// # Errors
    ///
    /// Returns an error if the internally constructed JSON cannot be
    /// serialized.
    pub fn canonical_stdout(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = serde_json::to_vec(&self.value)?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }
}

/// Validates one head-referenced artifact against exact bytes and its public
/// semantic hash.
///
/// # Errors
///
/// Returns a typed object or metadata-integrity failure.
pub fn validate_head_artifact(
    reference: &ArtifactReference,
    bytes: &[u8],
) -> Result<(), StorageError> {
    let observed = codenoesis_domain::storage::ArtifactId::from_bytes(bytes);
    if u64::try_from(bytes.len()).ok() != Some(reference.byte_length)
        || observed != reference.artifact_id
    {
        return Err(StorageError::CorruptObject {
            artifact_id: reference.artifact_id.to_string(),
            expected_hash: reference.artifact_id.digest().to_owned(),
            observed_hash: observed.digest().to_owned(),
        });
    }
    let mut value =
        serde_json::from_slice::<Value>(bytes).map_err(|_| StorageError::CorruptMetadata {
            component: StorageComponent::Head,
            reason: "artifact_json_invalid",
            snapshot_id: None,
        })?;
    if serde_json::to_vec(&value).map_err(|_| StorageError::CorruptMetadata {
        component: StorageComponent::Head,
        reason: "artifact_json_invalid",
        snapshot_id: None,
    })? != bytes
    {
        return Err(StorageError::CorruptMetadata {
            component: StorageComponent::Head,
            reason: "artifact_json_not_canonical",
            snapshot_id: None,
        });
    }
    if reference.role != ArtifactRole::SnapshotSemantic {
        value
            .as_object_mut()
            .and_then(|object| object.remove("semantic_hash"))
            .ok_or(StorageError::CorruptMetadata {
                component: StorageComponent::Head,
                reason: "artifact_semantic_hash_missing",
                snapshot_id: None,
            })?;
    }
    let observed_semantic_hash = semantic_hash(reference.semantic_hash.domain.as_bytes(), &value);
    if reference.semantic_hash.algorithm != "blake3-256"
        || observed_semantic_hash != reference.semantic_hash.value
    {
        return Err(StorageError::CorruptMetadata {
            component: StorageComponent::Head,
            reason: "artifact_semantic_hash_mismatch",
            snapshot_id: None,
        });
    }
    Ok(())
}

fn publication_candidate(value: &Value) -> Result<PublicationCandidate, PublicationCandidateError> {
    let snapshot_schema_version = required_str(value, "schema_version", "snapshot.schema_version")?;
    let snapshot_hash_domain =
        codenoesis_domain::storage::snapshot_hash_domain(snapshot_schema_version).ok_or(
            PublicationCandidateError::InvalidContract("snapshot.schema_version"),
        )?;
    let graph_hash_domain = codenoesis_domain::storage::graph_hash_domain(snapshot_schema_version)
        .ok_or(PublicationCandidateError::InvalidContract(
            "snapshot.schema_version",
        ))?;
    let extraction_hash_domain =
        codenoesis_domain::storage::extraction_hash_domain(snapshot_schema_version).ok_or(
            PublicationCandidateError::InvalidContract("snapshot.schema_version"),
        )?;
    let embedded_domains_required =
        snapshot_schema_version == codenoesis_domain::storage::SNAPSHOT_SCHEMA_VERSION;
    let v4_contract = matches!(
        snapshot_schema_version,
        codenoesis_domain::storage::SNAPSHOT_SCHEMA_VERSION_V4
            | codenoesis_domain::storage::SNAPSHOT_SCHEMA_VERSION_V5
            | codenoesis_domain::storage::SNAPSHOT_SCHEMA_VERSION_V6
            | codenoesis_domain::storage::SNAPSHOT_SCHEMA_VERSION_V7
            | codenoesis_domain::storage::SNAPSHOT_SCHEMA_VERSION_V8
            | codenoesis_domain::storage::SNAPSHOT_SCHEMA_VERSION_V9
            | codenoesis_domain::storage::SNAPSHOT_SCHEMA_VERSION_V10
    );
    let semantic = required_field(value, "semantic", "semantic")?;
    let repository = required_field(semantic, "repository", "semantic.repository")?;
    let repository_identity =
        RepositoryIdentity::parse(required_str(repository, "identity", "repository.identity")?)
            .map_err(|_| PublicationCandidateError::InvalidContract("repository.identity"))?;
    let commit_oid = ObjectId::parse_sha1(required_str(
        repository,
        "commit_oid",
        "repository.commit_oid",
    )?)
    .ok_or(PublicationCandidateError::InvalidContract(
        "repository.commit_oid",
    ))?;
    let snapshot_hash = required_str(
        required_field(value, "semantic_hash", "semantic_hash")?,
        "value",
        "semantic_hash.value",
    )?;
    let snapshot_id = SnapshotId::from_semantic_hash(snapshot_hash)
        .map_err(PublicationCandidateError::Storage)?;
    let graph = required_field(semantic, "knowledge_graph", "semantic.knowledge_graph")?;
    let graph_hash = stored_semantic_hash(
        graph,
        "knowledge_graph.semantic_hash",
        graph_hash_domain,
        embedded_domains_required,
    )?;
    let (artifacts, extraction_chunks) = publication_artifacts(
        semantic,
        graph,
        snapshot_hash,
        snapshot_hash_domain,
        &graph_hash,
        extraction_hash_domain,
        embedded_domains_required,
    )?;
    let rows = publication_graph_rows(graph, v4_contract)?;
    let candidate = PublicationCandidate {
        snapshot: SnapshotRecord {
            snapshot_id,
            repository_identity,
            commit_oid,
            snapshot_schema_version: snapshot_schema_version.to_owned(),
            semantic_hash: StoredSemanticHash::blake3(snapshot_hash_domain, snapshot_hash),
            graph_semantic_hash: graph_hash,
        },
        artifacts,
        extraction_chunks,
        entities: rows.entities,
        relationships: rows.relationships,
        claims: rows.claims,
        evidence: rows.evidence,
        diagnostics: rows.diagnostics,
        coverage_gaps: rows.coverage_gaps,
    };
    candidate
        .validate()
        .map_err(PublicationCandidateError::Storage)?;
    Ok(candidate)
}

fn publication_artifacts(
    semantic: &Value,
    graph: &Value,
    snapshot_hash: &str,
    snapshot_hash_domain: &str,
    graph_hash: &StoredSemanticHash,
    extraction_hash_domain: &str,
    embedded_domains_required: bool,
) -> Result<(Vec<StoredArtifact>, Vec<ExtractionRow>), PublicationCandidateError> {
    let snapshot_bytes =
        serde_json::to_vec(semantic).map_err(PublicationCandidateError::Serialization)?;
    let graph_bytes =
        serde_json::to_vec(graph).map_err(PublicationCandidateError::Serialization)?;
    let mut artifacts = vec![
        StoredArtifact::new(
            ArtifactRole::SnapshotSemantic,
            0,
            snapshot_bytes,
            StoredSemanticHash::blake3(snapshot_hash_domain, snapshot_hash),
        ),
        StoredArtifact::new(
            ArtifactRole::KnowledgeGraph,
            0,
            graph_bytes,
            graph_hash.clone(),
        ),
    ];
    let mut extraction_rows = Vec::new();
    for (ordinal, chunk) in
        required_array(semantic, "extraction_chunks", "semantic.extraction_chunks")?
            .iter()
            .enumerate()
    {
        let ordinal = u32::try_from(ordinal)
            .map_err(|_| PublicationCandidateError::InvalidContract("extraction ordinal"))?;
        let bytes = serde_json::to_vec(chunk).map_err(PublicationCandidateError::Serialization)?;
        let artifact = StoredArtifact::new(
            ArtifactRole::ExtractionChunk,
            ordinal,
            bytes.clone(),
            stored_semantic_hash(
                chunk,
                "extraction_chunk.semantic_hash",
                extraction_hash_domain,
                embedded_domains_required,
            )?,
        );
        let chunk_id = if embedded_domains_required {
            required_str(chunk, "chunk_id", "extraction_chunk.chunk_id")?
        } else if matches!(
            chunk["schema_version"].as_str(),
            Some(
                "codenoesis.extraction-chunk/v4"
                    | "codenoesis.extraction-chunk/v5"
                    | "codenoesis.extraction-chunk/v6"
                    | "codenoesis.extraction-chunk/v7"
            )
        ) {
            let subject = required_field(chunk, "subject", "extraction_chunk.subject")?;
            match required_str(subject, "kind", "extraction_chunk.subject.kind")? {
                "cargo_manifest" => required_str(
                    subject,
                    "manifest_id",
                    "extraction_chunk.subject.manifest_id",
                )?,
                "rust_source" => required_str(
                    subject,
                    "source_file_id",
                    "extraction_chunk.subject.source_file_id",
                )?,
                _ => {
                    return Err(PublicationCandidateError::InvalidContract(
                        "extraction_chunk.subject.kind",
                    ));
                }
            }
        } else {
            required_str(chunk, "source_file_id", "extraction_chunk.source_file_id")?
        };
        extraction_rows.push(ExtractionRow {
            ordinal,
            chunk_id: chunk_id.to_owned(),
            artifact_id: artifact.artifact_id.clone(),
            canonical_json: bytes,
        });
        artifacts.push(artifact);
    }
    Ok((artifacts, extraction_rows))
}

struct GraphRows {
    entities: Vec<EntityRow>,
    relationships: Vec<RelationshipRow>,
    claims: Vec<ClaimRow>,
    evidence: Vec<EvidenceRow>,
    diagnostics: Vec<OrdinalRow>,
    coverage_gaps: Vec<OrdinalRow>,
}

fn publication_graph_rows(
    graph: &Value,
    v4_contract: bool,
) -> Result<GraphRows, PublicationCandidateError> {
    let entity_id_field = if v4_contract { "id" } else { "entity_id" };
    let relationship_id_field = if v4_contract { "id" } else { "relationship_id" };
    let relationship_source_field = if v4_contract {
        "source"
    } else {
        "source_entity_id"
    };
    let relationship_target_field = if v4_contract {
        "target"
    } else {
        "target_entity_id"
    };
    let claim_id_field = if v4_contract { "id" } else { "claim_id" };
    let evidence_id_field = if v4_contract { "id" } else { "evidence_id" };
    let entities = canonical_rows(
        required_array(graph, "entities", "knowledge_graph.entities")?,
        |entity, canonical_json| {
            Ok(EntityRow {
                entity_id: required_str(entity, entity_id_field, "entity.entity_id")?.to_owned(),
                canonical_json,
            })
        },
    )?;
    let relationships = canonical_rows(
        required_array(graph, "relationships", "knowledge_graph.relationships")?,
        |relationship, canonical_json| {
            Ok(RelationshipRow {
                relationship_id: required_str(
                    relationship,
                    relationship_id_field,
                    "relationship.relationship_id",
                )?
                .to_owned(),
                source_entity_id: required_str(
                    relationship,
                    relationship_source_field,
                    "relationship.source_entity_id",
                )?
                .to_owned(),
                target_entity_id: required_str(
                    relationship,
                    relationship_target_field,
                    "relationship.target_entity_id",
                )?
                .to_owned(),
                canonical_json,
            })
        },
    )?;
    let claims = canonical_rows(
        required_array(graph, "claims", "knowledge_graph.claims")?,
        |claim, canonical_json| {
            Ok(ClaimRow {
                claim_id: required_str(claim, claim_id_field, "claim.claim_id")?.to_owned(),
                subject_kind: required_str(claim, "subject_kind", "claim.subject_kind")?.to_owned(),
                subject_id: required_str(claim, "subject_id", "claim.subject_id")?.to_owned(),
                canonical_json,
            })
        },
    )?;
    let evidence = canonical_rows(
        required_array(graph, "evidence", "knowledge_graph.evidence")?,
        |evidence, canonical_json| {
            Ok(EvidenceRow {
                evidence_id: required_str(evidence, evidence_id_field, "evidence.evidence_id")?
                    .to_owned(),
                canonical_json,
            })
        },
    )?;
    let diagnostics = ordinal_rows(required_array(
        graph,
        "diagnostics",
        "knowledge_graph.diagnostics",
    )?)?;
    let coverage_gaps = if v4_contract {
        ordinal_rows(required_array(
            graph,
            "coverage",
            "knowledge_graph.coverage",
        )?)?
    } else {
        let coverage = required_field(graph, "coverage", "knowledge_graph.coverage")?;
        ordinal_rows(required_array(
            coverage,
            "gaps",
            "knowledge_graph.coverage.gaps",
        )?)?
    };
    Ok(GraphRows {
        entities,
        relationships,
        claims,
        evidence,
        diagnostics,
        coverage_gaps,
    })
}

fn required_field<'a>(
    value: &'a Value,
    key: &str,
    field: &'static str,
) -> Result<&'a Value, PublicationCandidateError> {
    value
        .get(key)
        .ok_or(PublicationCandidateError::InvalidContract(field))
}

fn required_str<'a>(
    value: &'a Value,
    key: &str,
    field: &'static str,
) -> Result<&'a str, PublicationCandidateError> {
    required_field(value, key, field)?
        .as_str()
        .ok_or(PublicationCandidateError::InvalidContract(field))
}

fn required_array<'a>(
    value: &'a Value,
    key: &str,
    field: &'static str,
) -> Result<&'a [Value], PublicationCandidateError> {
    required_field(value, key, field)?
        .as_array()
        .map(Vec::as_slice)
        .ok_or(PublicationCandidateError::InvalidContract(field))
}

fn stored_semantic_hash(
    value: &Value,
    field: &'static str,
    expected_domain: &str,
    embedded_domain_required: bool,
) -> Result<StoredSemanticHash, PublicationCandidateError> {
    let hash = required_field(value, "semantic_hash", field)?;
    let domain = match hash.get("domain") {
        Some(domain) => domain
            .as_str()
            .filter(|domain| *domain == expected_domain)
            .ok_or(PublicationCandidateError::InvalidContract(field))?,
        None if !embedded_domain_required => expected_domain,
        None => return Err(PublicationCandidateError::InvalidContract(field)),
    };
    Ok(StoredSemanticHash {
        algorithm: required_str(hash, "algorithm", field)?.to_owned(),
        domain: domain.to_owned(),
        value: required_str(hash, "value", field)?.to_owned(),
    })
}

fn canonical_rows<T>(
    values: &[Value],
    mut row: impl FnMut(&Value, Vec<u8>) -> Result<T, PublicationCandidateError>,
) -> Result<Vec<T>, PublicationCandidateError> {
    values
        .iter()
        .map(|value| {
            let bytes =
                serde_json::to_vec(value).map_err(PublicationCandidateError::Serialization)?;
            row(value, bytes)
        })
        .collect()
}

fn ordinal_rows(values: &[Value]) -> Result<Vec<OrdinalRow>, PublicationCandidateError> {
    values
        .iter()
        .enumerate()
        .map(|(ordinal, value)| {
            Ok(OrdinalRow {
                ordinal: u32::try_from(ordinal)
                    .map_err(|_| PublicationCandidateError::InvalidContract("ordinal row index"))?,
                canonical_json: serde_json::to_vec(value)
                    .map_err(PublicationCandidateError::Serialization)?,
            })
        })
        .collect()
}

fn artifact_reference_value(reference: &ArtifactReference) -> Value {
    json!({
        "role": reference.role.as_str(),
        "ordinal": reference.ordinal,
        "artifact_id": reference.artifact_id.as_str(),
        "byte_length": reference.byte_length,
        "semantic_hash": semantic_hash_value(&reference.semantic_hash)
    })
}

fn semantic_hash_value(hash: &StoredSemanticHash) -> Value {
    json!({
        "algorithm": hash.algorithm,
        "domain": hash.domain,
        "value": hash.value
    })
}

#[derive(Clone, Debug)]
pub struct CodeNoesisErrorV1 {
    value: Value,
}

impl CodeNoesisErrorV1 {
    #[must_use]
    pub fn from_input(error: InputError) -> Self {
        let code = match error {
            InputError::InvalidRepositoryIdentity => "input.invalid_repository_identity",
            InputError::InvalidRevision
            | InputError::InvalidProfile
            | InputError::InvalidStoreRoot => "input.invalid_revision",
        };
        Self::new(code, "input", &error.to_string(), &json!({}))
    }

    #[must_use]
    pub fn from_acquisition(error: &AcquisitionError) -> Self {
        match error {
            AcquisitionError::NotGitRepository => Self::new(
                "acquisition.not_git_repository",
                "acquisition",
                &error.to_string(),
                &json!({}),
            ),
            AcquisitionError::RevisionNotFound { revision } => Self::new(
                "acquisition.revision_not_found",
                "acquisition",
                &error.to_string(),
                &json!({"revision": revision.as_str()}),
            ),
            AcquisitionError::RevisionNotCommit {
                object_oid,
                actual_kind,
            } => Self::new(
                "acquisition.revision_not_commit",
                "acquisition",
                &error.to_string(),
                &json!({
                    "object_oid": object_oid.as_str(),
                    "actual_kind": actual_kind.as_str()
                }),
            ),
            AcquisitionError::ObjectMissing {
                object_oid,
                expected_kind,
                referenced_by,
            } => Self::new(
                "acquisition.object_missing",
                "acquisition",
                &error.to_string(),
                &json!({
                    "object_oid": object_oid.as_str(),
                    "expected_kind": expected_kind.as_str(),
                    "referenced_by": referenced_by.as_str()
                }),
            ),
            AcquisitionError::RepositoryInconsistent {
                object_oid,
                expected_kind,
            } => Self::new(
                "acquisition.repository_inconsistent",
                "acquisition",
                &error.to_string(),
                &json!({
                    "object_oid": object_oid.as_str(),
                    "expected_kind": expected_kind.as_str()
                }),
            ),
            AcquisitionError::UnsupportedRepositoryShape { feature } => Self::new(
                "acquisition.unsupported_repository_shape",
                "acquisition",
                &error.to_string(),
                &json!({"feature": feature.as_str()}),
            ),
            AcquisitionError::PathInvalid { .. }
            | AcquisitionError::RootPolicyViolation { .. }
            | AcquisitionError::EntryPolicyViolation { .. }
            | AcquisitionError::LimitExceeded { .. } => Self::internal(),
        }
    }

    #[must_use]
    pub fn internal() -> Self {
        Self::new(
            "internal.unexpected",
            "internal",
            "unexpected internal failure",
            &json!({}),
        )
    }

    fn new(code: &str, stage: &str, message: &str, context: &Value) -> Self {
        Self {
            value: json!({
                "schema_version": "codenoesis.error/v1",
                "code": code,
                "stage": stage,
                "message": message,
                "retryable": false,
                "context": context
            }),
        }
    }

    /// Serializes one strict error document followed by one LF.
    ///
    /// # Errors
    ///
    /// Returns an error only if the internally constructed JSON value cannot be
    /// serialized.
    pub fn canonical_stderr(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = serde_json::to_vec(&self.value)?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

#[derive(Clone, Debug)]
pub struct CodeNoesisErrorV2 {
    value: Value,
}

impl CodeNoesisErrorV2 {
    #[must_use]
    pub fn from_input(error: InputError) -> Self {
        let code = match error {
            InputError::InvalidRepositoryIdentity => "input.invalid_repository_identity",
            InputError::InvalidRevision | InputError::InvalidStoreRoot => "input.invalid_revision",
            InputError::InvalidProfile => "input.invalid_profile",
        };
        Self::new(code, "input", &error.to_string(), &json!({}))
    }

    #[must_use]
    pub fn from_acquisition(error: &AcquisitionError) -> Self {
        match error {
            AcquisitionError::NotGitRepository => Self::new(
                "acquisition.not_git_repository",
                "acquisition",
                &error.to_string(),
                &json!({}),
            ),
            AcquisitionError::RevisionNotFound { revision } => Self::new(
                "acquisition.revision_not_found",
                "acquisition",
                &error.to_string(),
                &json!({"revision": revision.as_str()}),
            ),
            AcquisitionError::RevisionNotCommit {
                object_oid,
                actual_kind,
            } => Self::new(
                "acquisition.revision_not_commit",
                "acquisition",
                &error.to_string(),
                &json!({
                    "object_oid": object_oid.as_str(),
                    "actual_kind": actual_kind.as_str()
                }),
            ),
            AcquisitionError::ObjectMissing {
                object_oid,
                expected_kind,
                referenced_by,
            } => Self::new(
                "acquisition.object_missing",
                "acquisition",
                &error.to_string(),
                &json!({
                    "object_oid": object_oid.as_str(),
                    "expected_kind": expected_kind.as_str(),
                    "referenced_by": referenced_by.as_str()
                }),
            ),
            AcquisitionError::RepositoryInconsistent {
                object_oid,
                expected_kind,
            } => Self::new(
                "acquisition.repository_inconsistent",
                "acquisition",
                &error.to_string(),
                &json!({
                    "object_oid": object_oid.as_str(),
                    "expected_kind": expected_kind.as_str()
                }),
            ),
            AcquisitionError::UnsupportedRepositoryShape { feature } => Self::new(
                "acquisition.unsupported_repository_shape",
                "acquisition",
                &error.to_string(),
                &json!({"feature": feature.as_str()}),
            ),
            AcquisitionError::PathInvalid { reason } => Self::new(
                "acquisition.path_invalid",
                "acquisition",
                &error.to_string(),
                &json!({"reason": reason.as_str()}),
            ),
            AcquisitionError::RootPolicyViolation { policy } => Self::new(
                "acquisition.root_policy_violation",
                "acquisition",
                &error.to_string(),
                &json!({"policy": policy.as_str()}),
            ),
            AcquisitionError::EntryPolicyViolation { path, entry } => Self::new(
                "acquisition.entry_policy_violation",
                "acquisition",
                &error.to_string(),
                &json!({"entry": entry.as_str(), "path": path}),
            ),
            AcquisitionError::LimitExceeded {
                limit,
                maximum,
                observed,
            } => Self::new(
                "acquisition.limit_exceeded",
                "acquisition",
                &error.to_string(),
                &json!({
                    "limit": limit.as_str(),
                    "maximum": maximum,
                    "observed": observed
                }),
            ),
        }
    }

    #[must_use]
    pub fn internal() -> Self {
        Self::new(
            "internal.unexpected",
            "internal",
            "unexpected internal failure",
            &json!({}),
        )
    }

    fn new(code: &str, stage: &str, message: &str, context: &Value) -> Self {
        Self {
            value: json!({
                "schema_version": "codenoesis.error/v2",
                "code": code,
                "stage": stage,
                "message": message,
                "retryable": false,
                "context": context
            }),
        }
    }

    /// Serializes one strict S1 error document followed by one LF.
    ///
    /// # Errors
    ///
    /// Returns an error only if the internally constructed JSON value cannot be serialized.
    pub fn canonical_stderr(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = serde_json::to_vec(&self.value)?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

#[derive(Clone, Debug)]
pub struct CodeNoesisErrorV3 {
    value: Value,
}

impl CodeNoesisErrorV3 {
    #[must_use]
    pub fn from_input(error: InputError) -> Self {
        let code = match error {
            InputError::InvalidRepositoryIdentity => "input.invalid_repository_identity",
            InputError::InvalidRevision | InputError::InvalidStoreRoot => "input.invalid_revision",
            InputError::InvalidProfile => "input.invalid_profile",
        };
        Self::new(code, "input", &error.to_string(), &json!({}))
    }

    #[must_use]
    pub fn from_acquisition(error: &AcquisitionError) -> Self {
        match error {
            AcquisitionError::NotGitRepository => Self::new(
                "acquisition.not_git_repository",
                "acquisition",
                &error.to_string(),
                &json!({}),
            ),
            AcquisitionError::RevisionNotFound { revision } => Self::new(
                "acquisition.revision_not_found",
                "acquisition",
                &error.to_string(),
                &json!({"revision": revision.as_str()}),
            ),
            AcquisitionError::RevisionNotCommit {
                object_oid,
                actual_kind,
            } => Self::new(
                "acquisition.revision_not_commit",
                "acquisition",
                &error.to_string(),
                &json!({
                    "object_oid": object_oid.as_str(),
                    "actual_kind": actual_kind.as_str()
                }),
            ),
            AcquisitionError::ObjectMissing {
                object_oid,
                expected_kind,
                referenced_by,
            } => Self::new(
                "acquisition.object_missing",
                "acquisition",
                &error.to_string(),
                &json!({
                    "object_oid": object_oid.as_str(),
                    "expected_kind": expected_kind.as_str(),
                    "referenced_by": referenced_by.as_str()
                }),
            ),
            AcquisitionError::RepositoryInconsistent {
                object_oid,
                expected_kind,
            } => Self::new(
                "acquisition.repository_inconsistent",
                "acquisition",
                &error.to_string(),
                &json!({
                    "object_oid": object_oid.as_str(),
                    "expected_kind": expected_kind.as_str()
                }),
            ),
            AcquisitionError::UnsupportedRepositoryShape { feature } => Self::new(
                "acquisition.unsupported_repository_shape",
                "acquisition",
                &error.to_string(),
                &json!({"feature": feature.as_str()}),
            ),
            AcquisitionError::PathInvalid { reason } => Self::new(
                "acquisition.path_invalid",
                "acquisition",
                &error.to_string(),
                &json!({"reason": reason.as_str()}),
            ),
            AcquisitionError::RootPolicyViolation { policy } => Self::new(
                "acquisition.root_policy_violation",
                "acquisition",
                &error.to_string(),
                &json!({"policy": policy.as_str()}),
            ),
            AcquisitionError::EntryPolicyViolation { path, entry } => Self::new(
                "acquisition.entry_policy_violation",
                "acquisition",
                &error.to_string(),
                &json!({"entry": entry.as_str(), "path": path}),
            ),
            AcquisitionError::LimitExceeded {
                limit,
                maximum,
                observed,
            } => Self::new(
                "acquisition.limit_exceeded",
                "acquisition",
                &error.to_string(),
                &json!({
                    "limit": limit.as_str(),
                    "maximum": maximum,
                    "observed": observed
                }),
            ),
        }
    }

    #[must_use]
    pub fn from_knowledge(error: &KnowledgeError) -> Self {
        extraction_knowledge_error(error).unwrap_or_else(|| graph_knowledge_error(error))
    }

    #[must_use]
    pub fn internal() -> Self {
        Self::new(
            "internal.unexpected",
            "internal",
            "unexpected internal failure",
            &json!({}),
        )
    }

    fn new(code: &str, stage: &str, message: &str, context: &Value) -> Self {
        Self {
            value: json!({
                "schema_version": "codenoesis.error/v3",
                "code": code,
                "stage": stage,
                "message": message,
                "retryable": false,
                "context": context
            }),
        }
    }

    /// Serializes one strict S2 error document followed by one LF.
    ///
    /// # Errors
    ///
    /// Returns an error only if the internally constructed JSON cannot be serialized.
    pub fn canonical_stderr(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = serde_json::to_vec(&self.value)?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

#[derive(Clone, Debug)]
pub struct CodeNoesisErrorV4 {
    value: Value,
}

impl CodeNoesisErrorV4 {
    #[must_use]
    pub fn from_input(error: InputError) -> Self {
        let code = match error {
            InputError::InvalidRepositoryIdentity => "input.invalid_repository_identity",
            InputError::InvalidRevision => "input.invalid_revision",
            InputError::InvalidProfile => "input.invalid_profile",
            InputError::InvalidStoreRoot => "input.invalid_store_root",
        };
        Self::new(code, "input", &error.to_string(), false, &json!({}))
    }

    #[must_use]
    pub fn from_acquisition(error: &AcquisitionError) -> Self {
        Self::promote(CodeNoesisErrorV3::from_acquisition(error).value)
    }

    #[must_use]
    pub fn from_knowledge(error: &KnowledgeError) -> Self {
        Self::promote(CodeNoesisErrorV3::from_knowledge(error).value)
    }

    #[must_use]
    pub fn from_storage(error: &StorageError) -> Self {
        match error {
            StorageError::UnmarkedNonemptyRoot => Self::new(
                "storage.unmarked_nonempty_root",
                "storage",
                "The store root is non-empty and has no CodeNoesis marker.",
                false,
                &json!({"component": "marker"}),
            ),
            StorageError::IncompatibleSchema { observed_schema } => Self::new(
                "storage.incompatible_schema",
                "storage",
                "The local store schema is not supported.",
                false,
                &json!({
                    "component": "sqlite",
                    "expected_schema": "codenoesis.local-store/v1",
                    "observed_schema": observed_schema
                }),
            ),
            StorageError::WriterBusy => Self::new(
                "storage.writer_busy",
                "storage",
                "The local store writer is busy.",
                true,
                &json!({"component": "sqlite"}),
            ),
            StorageError::MissingObject { artifact_id } => Self::new(
                "storage.missing_object",
                "storage",
                "A head-referenced content object is missing.",
                false,
                &json!({"component": "cas", "artifact_id": artifact_id}),
            ),
            StorageError::CorruptObject {
                artifact_id,
                expected_hash,
                observed_hash,
            } => Self::new(
                "storage.corrupt_object",
                "storage",
                "A head-referenced content object failed integrity validation.",
                false,
                &json!({
                    "component": "cas",
                    "artifact_id": artifact_id,
                    "expected_hash": expected_hash,
                    "observed_hash": observed_hash
                }),
            ),
            StorageError::CorruptMetadata {
                component,
                reason,
                snapshot_id,
            } => {
                let context = snapshot_id.as_ref().map_or_else(
                    || json!({"component": component.as_str(), "reason": reason}),
                    |snapshot_id| {
                        json!({
                            "component": component.as_str(),
                            "reason": reason,
                            "snapshot_id": snapshot_id
                        })
                    },
                );
                Self::new(
                    "storage.corrupt_metadata",
                    "storage",
                    "The local store metadata failed integrity validation.",
                    false,
                    &context,
                )
            }
            StorageError::UnsafePath { reason } => Self::new(
                "storage.unsafe_path",
                "storage",
                "The store root violates the local storage path policy.",
                false,
                &json!({"component": "marker", "reason": reason}),
            ),
            StorageError::HeadConflict { expected, actual } => Self::new(
                "publication.head_conflict",
                "publication",
                "The project head changed before publication committed.",
                true,
                &json!({"expected_head": expected, "actual_head": actual}),
            ),
            StorageError::PublicationFailed => Self::new(
                "publication.failed",
                "publication",
                "Atomic snapshot publication failed.",
                false,
                &json!({}),
            ),
        }
    }

    #[must_use]
    pub fn internal() -> Self {
        Self::new(
            "internal.unexpected",
            "internal",
            "unexpected internal failure",
            false,
            &json!({}),
        )
    }

    fn promote(mut value: Value) -> Self {
        value["schema_version"] = json!("codenoesis.error/v4");
        Self { value }
    }

    fn new(code: &str, stage: &str, message: &str, retryable: bool, context: &Value) -> Self {
        Self {
            value: json!({
                "schema_version": "codenoesis.error/v4",
                "code": code,
                "stage": stage,
                "message": message,
                "retryable": retryable,
                "context": context
            }),
        }
    }

    /// Serializes one strict S3 error document followed by one LF.
    ///
    /// # Errors
    ///
    /// Returns an error only if the internally constructed JSON cannot be
    /// serialized.
    pub fn canonical_stderr(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = serde_json::to_vec(&self.value)?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }
}

fn extraction_knowledge_error(error: &KnowledgeError) -> Option<CodeNoesisErrorV3> {
    let value = match error {
        KnowledgeError::InvalidUtf8 { path } => CodeNoesisErrorV3::new(
            "extraction.invalid_utf8",
            "extraction",
            &error.to_string(),
            &json!({"path": path}),
        ),
        KnowledgeError::UnsupportedCrateShape => CodeNoesisErrorV3::new(
            "extraction.unsupported_crate_shape",
            "extraction",
            &error.to_string(),
            &json!({}),
        ),
        KnowledgeError::ParserCancelled { path } => CodeNoesisErrorV3::new(
            "extraction.parser_cancelled",
            "extraction",
            &error.to_string(),
            &json!({"path": path}),
        ),
        KnowledgeError::MalformedSyntax { path, span } => CodeNoesisErrorV3::new(
            "extraction.malformed_syntax",
            "extraction",
            &error.to_string(),
            &json!({"path": path, "span": span_value(*span)}),
        ),
        KnowledgeError::NormalizationCollision {
            kind,
            canonical_identity,
            path,
            first_span,
            second_span,
        } => CodeNoesisErrorV3::new(
            "extraction.normalization_collision",
            "extraction",
            &error.to_string(),
            &json!({
                "kind": kind.as_str(),
                "canonical_identity": canonical_identity,
                "path": path,
                "first_span": span_value(*first_span),
                "second_span": span_value(*second_span)
            }),
        ),
        KnowledgeError::ContractInvalid => CodeNoesisErrorV3::new(
            "extraction.contract_invalid",
            "extraction",
            &error.to_string(),
            &json!({}),
        ),
        KnowledgeError::LimitExceeded {
            limit,
            maximum,
            observed,
        } => CodeNoesisErrorV3::new(
            "extraction.limit_exceeded",
            "extraction",
            &error.to_string(),
            &json!({
                "limit": limit,
                "maximum": maximum,
                "observed": observed
            }),
        ),
        KnowledgeError::InvalidEntity { .. }
        | KnowledgeError::InvalidRelationship { .. }
        | KnowledgeError::DanglingReference { .. }
        | KnowledgeError::CardinalityViolation { .. }
        | KnowledgeError::InvalidClaimState { .. }
        | KnowledgeError::InvalidDerivation { .. }
        | KnowledgeError::GraphLimitExceeded { .. } => return None,
    };
    Some(value)
}

fn graph_knowledge_error(error: &KnowledgeError) -> CodeNoesisErrorV3 {
    match error {
        KnowledgeError::InvalidEntity { entity_id } => CodeNoesisErrorV3::new(
            "graph.invalid_entity",
            "graph",
            &error.to_string(),
            &json!({"entity_id": entity_id}),
        ),
        KnowledgeError::InvalidRelationship { relationship_id } => CodeNoesisErrorV3::new(
            "graph.invalid_relationship",
            "graph",
            &error.to_string(),
            &json!({"reason": "invalid_relationship_id", "canonical_identity": relationship_id}),
        ),
        KnowledgeError::DanglingReference { reference_id } => CodeNoesisErrorV3::new(
            "graph.dangling_reference",
            "graph",
            &error.to_string(),
            &json!({"canonical_identity": reference_id}),
        ),
        KnowledgeError::CardinalityViolation { subject_id } => CodeNoesisErrorV3::new(
            "graph.cardinality_violation",
            "graph",
            &error.to_string(),
            &json!({"cardinality": "invalid_count", "canonical_identity": subject_id}),
        ),
        KnowledgeError::InvalidClaimState { claim_id } => CodeNoesisErrorV3::new(
            "graph.invalid_claim_state",
            "graph",
            &error.to_string(),
            &json!({"claim_id": claim_id}),
        ),
        KnowledgeError::InvalidDerivation { claim_id } => CodeNoesisErrorV3::new(
            "graph.invalid_derivation",
            "graph",
            &error.to_string(),
            &json!({"claim_id": claim_id, "rule_version": CONTAINMENT_RULE_VERSION}),
        ),
        KnowledgeError::GraphLimitExceeded {
            limit,
            maximum,
            observed,
        } => CodeNoesisErrorV3::new(
            "graph.limit_exceeded",
            "graph",
            &error.to_string(),
            &json!({
                "limit": limit,
                "maximum": maximum,
                "observed": observed
            }),
        ),
        KnowledgeError::InvalidUtf8 { .. }
        | KnowledgeError::UnsupportedCrateShape
        | KnowledgeError::ParserCancelled { .. }
        | KnowledgeError::MalformedSyntax { .. }
        | KnowledgeError::NormalizationCollision { .. }
        | KnowledgeError::ContractInvalid
        | KnowledgeError::LimitExceeded { .. } => {
            unreachable!("extraction errors are dispatched before graph errors")
        }
    }
}

fn extraction_chunk_value(chunk: &ExtractionChunk) -> Value {
    let mut value = json!({
        "schema_version": "codenoesis.extraction-chunk/v1",
        "chunk_id": chunk.chunk_id,
        "ontology_version": ONTOLOGY_VERSION,
        "extractor_version": EXTRACTOR_VERSION,
        "repository": {
            "identity": chunk.repository_identity,
            "object_format": "sha1",
            "commit_oid": chunk.commit_oid
        },
        "source": {
            "blob_oid": chunk.blob_oid,
            "path": chunk.path,
            "byte_length": chunk.byte_length
        },
        "entities": chunk.entities.iter().map(knowledge_entity_value).collect::<Vec<_>>(),
        "relationships": chunk.relationships.iter().map(knowledge_relationship_value).collect::<Vec<_>>(),
        "claims": chunk.claims.iter().map(knowledge_claim_value).collect::<Vec<_>>(),
        "evidence": chunk.evidence.iter().map(source_evidence_value).collect::<Vec<_>>(),
        "diagnostics": chunk.diagnostics.iter().map(diagnostic_value).collect::<Vec<_>>(),
        "coverage": coverage_value(&chunk.coverage)
    });
    let hash = semantic_hash(EXTRACTION_HASH_DOMAIN, &value);
    value
        .as_object_mut()
        .expect("extraction value is an object")
        .insert(
            "semantic_hash".to_owned(),
            json!({
                "algorithm": "blake3-256",
                "domain": "codenoesis.extraction-chunk.semantic.v1",
                "value": hash
            }),
        );
    value
}

fn knowledge_graph_value(graph: &KnowledgeGraph) -> Value {
    let mut value = json!({
        "schema_version": "codenoesis.knowledge-graph/v1",
        "ontology_version": ONTOLOGY_VERSION,
        "repository": {
            "identity": graph.repository_identity,
            "object_format": "sha1",
            "commit_oid": graph.commit_oid
        },
        "extractor_versions": [EXTRACTOR_VERSION],
        "entities": graph.entities.iter().map(knowledge_entity_value).collect::<Vec<_>>(),
        "relationships": graph.relationships.iter().map(knowledge_relationship_value).collect::<Vec<_>>(),
        "claims": graph.claims.iter().map(knowledge_claim_value).collect::<Vec<_>>(),
        "evidence": graph.evidence.iter().map(source_evidence_value).collect::<Vec<_>>(),
        "diagnostics": graph.diagnostics.iter().map(diagnostic_value).collect::<Vec<_>>(),
        "coverage": coverage_value(&graph.coverage)
    });
    let hash = semantic_hash(GRAPH_HASH_DOMAIN, &value);
    value
        .as_object_mut()
        .expect("graph value is an object")
        .insert(
            "semantic_hash".to_owned(),
            json!({
                "algorithm": "blake3-256",
                "domain": "codenoesis.knowledge-graph.semantic.v1",
                "value": hash
            }),
        );
    value
}

fn knowledge_entity_value(entity: &KnowledgeEntity) -> Value {
    json!({
        "entity_id": entity.entity_id,
        "kind": entity.kind.as_str(),
        "canonical_identity": entity.canonical_identity,
        "display_name": entity.display_name,
        "language": "rust",
        "properties": entity.properties,
        "evidence_ids": entity.evidence_ids,
        "claim_id": entity.claim_id
    })
}

fn knowledge_relationship_value(relationship: &KnowledgeRelationship) -> Value {
    json!({
        "relationship_id": relationship.relationship_id,
        "kind": relationship.kind.as_str(),
        "source_entity_id": relationship.source_entity_id,
        "target_entity_id": relationship.target_entity_id,
        "evidence_ids": relationship.evidence_ids,
        "claim_id": relationship.claim_id
    })
}

fn knowledge_claim_value(claim: &KnowledgeClaim) -> Value {
    let derivation = match &claim.derivation {
        ClaimDerivation::Parser {
            extractor_version,
            evidence_ids,
        } => json!({
            "kind": "parser",
            "extractor_version": extractor_version,
            "evidence_ids": evidence_ids
        }),
        ClaimDerivation::DeterministicRule {
            rule_version,
            input_claim_ids,
            evidence_ids,
        } => json!({
            "kind": "deterministic_rule",
            "rule_version": rule_version,
            "input_claim_ids": input_claim_ids,
            "evidence_ids": evidence_ids
        }),
    };
    json!({
        "claim_id": claim.claim_id,
        "subject_kind": claim.subject_kind.as_str(),
        "subject_id": claim.subject_id,
        "state": claim.state.as_str(),
        "derivation": derivation
    })
}

fn source_evidence_value(evidence: &SourceEvidence) -> Value {
    json!({
        "schema_version": "codenoesis.source-evidence/v2",
        "evidence_id": evidence.evidence_id,
        "repository": {
            "identity": evidence.repository_identity,
            "object_format": "sha1",
            "commit_oid": evidence.commit_oid
        },
        "blob_oid": evidence.blob_oid,
        "path": evidence.path,
        "span": span_value(evidence.span),
        "extractor": {
            "id": "codenoesis.rust-tree-sitter",
            "version": EXTRACTOR_VERSION
        },
        "syntax_kind": evidence.syntax_kind,
        "derivation": "tree_sitter_concrete_syntax"
    })
}

fn diagnostic_value(diagnostic: &ExtractionDiagnostic) -> Value {
    json!({
        "code": diagnostic.code,
        "severity": diagnostic.severity,
        "path": diagnostic.path,
        "span": span_value(diagnostic.span)
    })
}

fn coverage_value(coverage: &ExtractionCoverage) -> Value {
    json!({
        "status": if coverage.gaps.is_empty() {
            "complete"
        } else {
            "partial"
        },
        "supported_capabilities": coverage.supported_capabilities,
        "gaps": coverage.gaps.iter().map(coverage_gap_value).collect::<Vec<_>>()
    })
}

fn coverage_gap_value(gap: &CoverageGap) -> Value {
    json!({
        "code": gap.code,
        "path": gap.path,
        "span": span_value(gap.span),
        "evidence_id": gap.evidence_id
    })
}

fn span_value(span: ByteSpan) -> Value {
    json!({
        "unit": "byte",
        "start": span.start,
        "end": span.end
    })
}

fn inventory_value(inventory: &RepositoryInventory) -> Value {
    let files = inventory.files();
    let language_groups = language_groups(files);
    let manifests = recognized_files(files, RecognizedInventoryKind::CargoManifest);
    let contracts = recognized_files(files, RecognizedInventoryKind::OpenApiContract);
    let configurations = recognized_files(files, RecognizedInventoryKind::RustfmtConfiguration);
    let ownership = recognized_files(files, RecognizedInventoryKind::GitHubCodeowners);
    let unsupported = files
        .iter()
        .filter(|file| file.is_unsupported())
        .collect::<Vec<_>>();
    let sentinels = files
        .iter()
        .filter(|file| file.is_sentinel())
        .collect::<Vec<_>>();
    let diagnostics = diagnostics_value(&sentinels, &unsupported);
    let capabilities = capabilities_value(
        &language_groups,
        &manifests,
        &contracts,
        &configurations,
        &ownership,
    );
    let coverage_gaps = coverage_gaps_value(
        &language_groups,
        &manifests,
        &contracts,
        &configurations,
        &ownership,
        &unsupported,
    );
    let supported_file_count = files
        .len()
        .checked_sub(unsupported.len())
        .expect("unsupported files are a subset");

    json!({
        "schema_version": "codenoesis.inventory/v1",
        "classifier_version": "codenoesis.inventory-classifier/s1-v1",
        "summary": {
            "directory_count": inventory.directory_count(),
            "regular_file_count": files.len(),
            "total_file_bytes": files.iter().map(InventoryFile::byte_length).sum::<u64>(),
            "supported_file_count": supported_file_count,
            "unsupported_file_count": unsupported.len(),
            "language_count": language_groups.len(),
            "manifest_count": manifests.len(),
            "contract_count": contracts.len(),
            "configuration_count": configurations.len(),
            "ownership_count": ownership.len(),
            "diagnostic_count": diagnostics.len(),
            "coverage_gap_count": coverage_gaps.len()
        },
        "files": files.iter().map(file_value).collect::<Vec<_>>(),
        "languages": language_groups.iter().map(language_value).collect::<Vec<_>>(),
        "manifests": manifests.iter().map(|file| recognized_value("cargo", file)).collect::<Vec<_>>(),
        "contracts": contracts.iter().map(|file| recognized_value("openapi", file)).collect::<Vec<_>>(),
        "configurations": configurations.iter().map(|file| recognized_value("rustfmt", file)).collect::<Vec<_>>(),
        "ownership": ownership.iter().map(|file| recognized_value("github-codeowners", file)).collect::<Vec<_>>(),
        "extraction_capabilities": capabilities,
        "unsupported_content": unsupported.iter().map(|file| json!({
            "path": file.path(),
            "reason": "unsupported_extension",
            "evidence_id": file.evidence_id()
        })).collect::<Vec<_>>(),
        "diagnostics": diagnostics,
        "coverage_gaps": coverage_gaps,
        "evidence": files.iter().map(|file| evidence_value(inventory, file)).collect::<Vec<_>>()
    })
}

fn file_value(file: &InventoryFile) -> Value {
    json!({
        "path": file.path(),
        "mode": file.mode().as_str(),
        "blob_oid": file.blob_oid().as_str(),
        "byte_length": file.byte_length(),
        "content_kind": file.content_kind().as_str(),
        "roles": file.roles().iter().map(|role| role.as_str()).collect::<Vec<_>>(),
        "languages": file.languages().iter().map(|language| language.as_str()).collect::<Vec<_>>(),
        "evidence_id": file.evidence_id()
    })
}

fn language_groups(files: &[InventoryFile]) -> Vec<(InventoryLanguage, Vec<&InventoryFile>)> {
    let mut groups = BTreeMap::<InventoryLanguage, Vec<&InventoryFile>>::new();
    for file in files {
        for language in file.languages() {
            groups.entry(*language).or_default().push(file);
        }
    }
    groups.into_iter().collect()
}

fn language_value((language, files): &(InventoryLanguage, Vec<&InventoryFile>)) -> Value {
    json!({
        "id": language.as_str(),
        "display_name": language.display_name(),
        "paths": files.iter().map(|file| file.path()).collect::<Vec<_>>(),
        "evidence_ids": files.iter().map(|file| file.evidence_id()).collect::<Vec<_>>(),
        "detection_status": "supported",
        "extraction_status": "not_available"
    })
}

fn recognized_files(files: &[InventoryFile], kind: RecognizedInventoryKind) -> Vec<&InventoryFile> {
    files
        .iter()
        .filter(|file| file.recognized_kind() == Some(kind))
        .collect()
}

fn recognized_value(kind: &str, file: &InventoryFile) -> Value {
    json!({
        "kind": kind,
        "path": file.path(),
        "status": "recognized_not_interpreted",
        "evidence_id": file.evidence_id()
    })
}

fn capabilities_value(
    languages: &[(InventoryLanguage, Vec<&InventoryFile>)],
    manifests: &[&InventoryFile],
    contracts: &[&InventoryFile],
    configurations: &[&InventoryFile],
    ownership: &[&InventoryFile],
) -> Vec<Value> {
    let mut capabilities = Vec::new();
    if !configurations.is_empty() {
        capabilities.push(("configuration_interpretation", "configuration:rustfmt"));
    }
    if !contracts.is_empty() {
        capabilities.push(("contract_extraction", "contract:openapi"));
    }
    capabilities.push(("file_classification", "repository"));
    if !manifests.is_empty() {
        capabilities.push(("manifest_interpretation", "manifest:cargo"));
    }
    if !ownership.is_empty() {
        capabilities.push(("ownership_resolution", "ownership:github-codeowners"));
    }
    for (language, _) in languages {
        capabilities.push((
            "symbol_extraction",
            match language {
                InventoryLanguage::Rust => "language:rust",
                InventoryLanguage::Shell => "language:shell",
            },
        ));
    }
    capabilities.sort_unstable();
    capabilities
        .into_iter()
        .map(|(capability, subject)| {
            json!({
                "capability": capability,
                "subject": subject,
                "status": if capability == "file_classification" {
                    "available"
                } else {
                    "not_available"
                }
            })
        })
        .collect()
}

fn diagnostics_value(sentinels: &[&InventoryFile], unsupported: &[&InventoryFile]) -> Vec<Value> {
    let mut diagnostics = sentinels
        .iter()
        .map(|file| {
            (
                "inventory.target_execution_suppressed",
                file.path(),
                "info",
                file.evidence_id(),
            )
        })
        .chain(unsupported.iter().map(|file| {
            (
                "inventory.unsupported_content",
                file.path(),
                "warning",
                file.evidence_id(),
            )
        }))
        .collect::<Vec<_>>();
    diagnostics.sort_unstable();
    diagnostics
        .into_iter()
        .map(|(code, path, severity, evidence_id)| {
            json!({
                "code": code,
                "severity": severity,
                "path": path,
                "evidence_id": evidence_id
            })
        })
        .collect()
}

fn coverage_gaps_value(
    languages: &[(InventoryLanguage, Vec<&InventoryFile>)],
    manifests: &[&InventoryFile],
    contracts: &[&InventoryFile],
    configurations: &[&InventoryFile],
    ownership: &[&InventoryFile],
    unsupported: &[&InventoryFile],
) -> Vec<Value> {
    let mut gaps = Vec::<(&str, &str, Vec<&str>, Vec<&str>)>::new();
    if !configurations.is_empty() {
        gaps.push(coverage_gap(
            "coverage.configuration_not_interpreted",
            "configuration:rustfmt",
            configurations,
        ));
    }
    if !contracts.is_empty() {
        gaps.push(coverage_gap(
            "coverage.contract_not_extracted",
            "contract:openapi",
            contracts,
        ));
    }
    for (language, files) in languages {
        gaps.push(coverage_gap(
            "coverage.entities_not_extracted",
            match language {
                InventoryLanguage::Rust => "language:rust",
                InventoryLanguage::Shell => "language:shell",
            },
            files,
        ));
    }
    if !manifests.is_empty() {
        gaps.push(coverage_gap(
            "coverage.manifest_not_interpreted",
            "manifest:cargo",
            manifests,
        ));
    }
    if !ownership.is_empty() {
        gaps.push(coverage_gap(
            "coverage.ownership_not_resolved",
            "ownership:github-codeowners",
            ownership,
        ));
    }
    if !unsupported.is_empty() {
        gaps.push(coverage_gap(
            "coverage.unsupported_content",
            "repository",
            unsupported,
        ));
    }
    gaps.sort_by(|left, right| (left.0, left.1).cmp(&(right.0, right.1)));
    gaps.into_iter()
        .map(|(code, scope, paths, evidence_ids)| {
            json!({
                "code": code,
                "scope": scope,
                "paths": paths,
                "evidence_ids": evidence_ids
            })
        })
        .collect()
}

fn coverage_gap<'a>(
    code: &'a str,
    scope: &'a str,
    files: &'a [&InventoryFile],
) -> (&'a str, &'a str, Vec<&'a str>, Vec<&'a str>) {
    (
        code,
        scope,
        files.iter().map(|file| file.path()).collect(),
        files.iter().map(|file| file.evidence_id()).collect(),
    )
}

fn evidence_value(inventory: &RepositoryInventory, file: &InventoryFile) -> Value {
    let bound = inventory.bound_revision();
    json!({
        "schema_version": "codenoesis.source-evidence/v1",
        "evidence_id": file.evidence_id(),
        "repository": {
            "identity": bound.repository_identity().as_str(),
            "vcs": "git",
            "object_format": "sha1",
            "commit_oid": bound.commit_oid().as_str()
        },
        "blob_oid": file.blob_oid().as_str(),
        "path": file.path(),
        "span": {
            "unit": "byte",
            "start": 0,
            "end": file.byte_length()
        },
        "extractor": {
            "id": "codenoesis.inventory.static-classifier",
            "version": "codenoesis.inventory-classifier/s1-v1"
        },
        "derivation": {
            "kind": "deterministic_static_classification",
            "rules": file.rules().iter().map(|rule| rule.as_str()).collect::<Vec<_>>()
        }
    })
}

fn semantic_hash(domain: &[u8], value: &Value) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(&[0]);
    serde_json::to_writer(Blake3Writer(&mut hasher), value)
        .expect("JSON values constructed by CodeNoesis serialize");
    hasher.finalize().to_hex().to_string()
}

struct LimitedVecWriter {
    bytes: Vec<u8>,
    maximum: usize,
    overflowed: bool,
}

impl LimitedVecWriter {
    fn new(maximum: usize) -> Self {
        Self {
            bytes: Vec::new(),
            maximum,
            overflowed: false,
        }
    }

    const fn overflowed(&self) -> bool {
        self.overflowed
    }

    fn into_inner(self) -> Vec<u8> {
        self.bytes
    }
}

impl Write for LimitedVecWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let remaining = self.maximum.saturating_sub(self.bytes.len());
        if buffer.len() > remaining {
            self.bytes.extend_from_slice(&buffer[..remaining]);
            self.overflowed = true;
            return Err(io::Error::other("canonical output limit exceeded"));
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct Blake3Writer<'a>(&'a mut blake3::Hasher);

impl Write for Blake3Writer<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.0.update(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;

    use codenoesis_domain::{BoundRevision, ObjectId, RepositoryIdentity};
    use serde_json::{Map, Value, json};

    use super::{
        RepositorySnapshotV1, RepositorySnapshotV2, RepositorySnapshotV2Error,
        SNAPSHOT_HASH_DOMAIN, SnapshotEnvelopeV1, semantic_hash,
    };

    const COMMIT_A_OID: &str = "6d4152a7787ac82eedf3f9fc5df408dfdf6e412f";
    const TREE_A_OID: &str = "892c4a33b5529ba6b6651fc26765957f11f7ba9e";
    const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s0-one-file-v1";
    const SNAPSHOT_A_HASH: &str =
        "b673624a329f43fd84852bbdeefd66326a7fcb1c03fdb626e2de6bfedff11997";

    fn bound_revision() -> BoundRevision {
        BoundRevision::new(
            RepositoryIdentity::parse(REPOSITORY_ID).expect("approved fixture identity"),
            ObjectId::parse_sha1(COMMIT_A_OID).expect("approved commit OID"),
            ObjectId::parse_sha1(TREE_A_OID).expect("approved tree OID"),
        )
    }

    fn snapshot(envelope: SnapshotEnvelopeV1) -> RepositorySnapshotV1 {
        RepositorySnapshotV1::from_bound_revision(&bound_revision(), envelope)
    }

    fn fixed_envelope() -> SnapshotEnvelopeV1 {
        SnapshotEnvelopeV1::new(
            "2000-01-01T00:00:00Z".to_owned(),
            None,
            "s0-golden-a".to_owned(),
        )
    }

    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/s0/one-file-v1")
            .join(name)
    }

    fn reviewed_jcs_body(name: &str) -> Vec<u8> {
        let mut bytes = fs::read(fixture_path(name)).expect("read reviewed JCS golden");
        if bytes.ends_with(b"\r\n") {
            bytes.truncate(bytes.len() - 2);
        } else {
            assert_eq!(bytes.pop(), Some(b'\n'), "golden must end in one newline");
        }
        assert!(
            !bytes.contains(&b'\r') && !bytes.contains(&b'\n'),
            "golden body must be one canonical JSON line"
        );
        bytes
    }

    #[test]
    fn conf_dr_art_001_repository_snapshot_v1() {
        let actual = snapshot(fixed_envelope())
            .canonical_stdout()
            .expect("serialize fixed snapshot");
        let mut expected = reviewed_jcs_body("expected-snapshot-a.jcs");
        expected.push(b'\n');
        assert_eq!(actual, expected);

        let value: Value = serde_json::from_slice(&actual).expect("parse generated snapshot");
        assert_exact_keys(
            &value,
            &["envelope", "schema_version", "semantic", "semantic_hash"],
        );
        assert_exact_keys(
            &value["semantic"],
            &[
                "configuration",
                "evidence_lineage_version",
                "extractor_contract_version",
                "extractor_versions",
                "ontology_version",
                "pipeline_version",
                "repository",
            ],
        );
        assert_exact_keys(
            &value["semantic"]["repository"],
            &[
                "commit_oid",
                "contract_version",
                "identity",
                "identity_schema_version",
                "object_format",
                "tree_oid",
                "vcs",
            ],
        );
        assert_exact_keys(
            &value["semantic"]["configuration"],
            &["profile", "schema_version", "semantic_hash"],
        );
        assert_exact_keys(
            &value["envelope"],
            &["correlation_id", "created_at", "job_id"],
        );
    }

    #[test]
    fn pt_dr_art_002_volatile_envelope_preserves_semantic_hash() {
        let baseline = snapshot(fixed_envelope());
        let baseline_semantic = baseline
            .canonical_semantic()
            .expect("serialize baseline semantic");
        let baseline_hash = baseline.value()["semantic_hash"].clone();
        let mut baseline_without_envelope = baseline.value().clone();
        baseline_without_envelope
            .as_object_mut()
            .expect("snapshot object")
            .remove("envelope");
        let fixed_stdout = baseline
            .canonical_stdout()
            .expect("serialize fixed snapshot");

        for index in 0..50 {
            let candidate = snapshot(SnapshotEnvelopeV1::new(
                format!("2000-01-01T00:00:{index:02}Z"),
                (index % 2 == 0).then(|| format!("job-{index}")),
                format!("correlation-{index}"),
            ));
            assert_eq!(
                candidate
                    .canonical_semantic()
                    .expect("serialize candidate semantic"),
                baseline_semantic,
                "semantic bytes changed for envelope {index}"
            );
            assert_eq!(candidate.value()["semantic_hash"], baseline_hash);

            let mut candidate_without_envelope = candidate.value().clone();
            candidate_without_envelope
                .as_object_mut()
                .expect("snapshot object")
                .remove("envelope");
            assert_eq!(candidate_without_envelope, baseline_without_envelope);
            assert_eq!(
                snapshot(fixed_envelope())
                    .canonical_stdout()
                    .expect("serialize replayed fixed snapshot"),
                fixed_stdout
            );
        }
    }

    #[test]
    fn pt_nfr_det_001_permutation_and_schedule_invariant() {
        let expected = reviewed_jcs_body("expected-semantic-a.jcs");

        for seed in 0..50 {
            let semantic = permuted_semantic(seed);
            let canonical = serde_json::to_vec(&semantic).expect("serialize permuted semantic");
            assert_eq!(
                canonical, expected,
                "canonical bytes differ for seed {seed}"
            );
            assert_eq!(
                semantic_hash(SNAPSHOT_HASH_DOMAIN, &semantic),
                SNAPSHOT_A_HASH,
                "semantic hash differs for seed {seed}"
            );
        }
    }

    #[test]
    fn pt_fr_acq_002_canonical_output_has_max_and_plus_one() {
        let maximum =
            usize::try_from(codenoesis_domain::STANDARD_LOCAL_S1_LIMITS.canonical_output_bytes)
                .expect("canonical output maximum fits usize");
        let max_snapshot = RepositorySnapshotV2 {
            value: Value::String("a".repeat(maximum - 3)),
        };
        let output = max_snapshot
            .canonical_stdout()
            .expect("maximum canonical output must succeed");
        assert_eq!(output.len(), maximum);
        drop(output);
        drop(max_snapshot);

        let over_snapshot = RepositorySnapshotV2 {
            value: Value::String("a".repeat(maximum - 2)),
        };
        assert!(matches!(
            over_snapshot.canonical_stdout(),
            Err(RepositorySnapshotV2Error::LimitExceeded(
                codenoesis_domain::AcquisitionError::LimitExceeded {
                    limit: codenoesis_domain::LimitKind::CanonicalOutputBytes,
                    maximum: 33_554_432,
                    observed: 33_554_433
                }
            ))
        ));
    }

    fn assert_exact_keys(value: &Value, expected: &[&str]) {
        let actual = value
            .as_object()
            .expect("contract node must be an object")
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let expected = expected.iter().copied().collect::<BTreeSet<_>>();
        assert_eq!(actual, expected);
    }

    fn permuted_semantic(seed: usize) -> Value {
        let repository = permuted_object(
            vec![
                ("contract_version", json!("codenoesis.repository/v1")),
                (
                    "identity_schema_version",
                    json!("codenoesis.repository-identity/v1"),
                ),
                ("identity", json!(REPOSITORY_ID)),
                ("vcs", json!("git")),
                ("object_format", json!("sha1")),
                ("commit_oid", json!(COMMIT_A_OID)),
                ("tree_oid", json!(TREE_A_OID)),
            ],
            seed,
        );
        let configuration = permuted_object(
            vec![
                ("schema_version", json!("codenoesis.configuration/v1")),
                ("profile", json!("standard-local-s0")),
                (
                    "semantic_hash",
                    json!({
                        "algorithm": "blake3-256",
                        "value": "4811a917bebed264f49382d65825686ad5ca506ce39bc51385e547b0c7ced1c0"
                    }),
                ),
            ],
            seed.wrapping_mul(3).wrapping_add(1),
        );
        permuted_object(
            vec![
                ("repository", repository),
                ("configuration", configuration),
                ("pipeline_version", json!("codenoesis.pipeline/s0-v1")),
                ("ontology_version", json!("codenoesis.ontology/none-v1")),
                (
                    "extractor_contract_version",
                    json!("codenoesis.extraction/v1"),
                ),
                ("extractor_versions", json!([])),
                (
                    "evidence_lineage_version",
                    json!("codenoesis.evidence-lineage/v1"),
                ),
            ],
            seed.wrapping_mul(7).wrapping_add(2),
        )
    }

    fn permuted_object(mut entries: Vec<(&'static str, Value)>, seed: usize) -> Value {
        let length = entries.len();
        entries.rotate_left(seed % length);
        if seed % 2 == 1 {
            entries.reverse();
        }
        let mut object = Map::new();
        for (key, value) in entries {
            object.insert(key.to_owned(), value);
        }
        Value::Object(object)
    }
}
