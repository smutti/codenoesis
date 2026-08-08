use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::path::{Component, Path};

use codenoesis_domain::s4_k1::{
    CallSiteProperties, CallableCoverageGap, CallableDiagnostic, CallableRelationship,
    CallableSemanticEntity, CallableSemanticProperties, CallableSemanticsError,
    CallableSemanticsKnowledge, ControlProperties, DeclaredValueProperties, LocalBindingProperties,
    MAX_K1_EXPRESSION_METADATA_BYTES, NormalizedScalarValue,
};
pub use codenoesis_domain::s4_k1::{
    K1_CONFIGURATION_VERSION, K1_ERROR_VERSION, K1_EXTRACTION_CHUNK_VERSION,
    K1_EXTRACTION_CONTRACT_VERSION, K1_EXTRACTOR_VERSION, K1_GRAPH_VERSION, K1_INDEX_VERSION,
    K1_LOCAL_EXPLORER_VERSION, K1_ONTOLOGY_VERSION, K1_PIPELINE_VERSION, K1_PORTABLE_GRAPH_VERSION,
    K1_PROFILE, K1_QUERY_VERSION, K1_SNAPSHOT_VERSION,
};
use codenoesis_domain::s4_r3::R3_WORKSPACE_PROFILE;
use codenoesis_domain::s4_r4::R4_MANIFEST_PROFILE;
use codenoesis_domain::s4_r5::{R5_RUST_SEMANTIC_EXTRACTOR_VERSION, R5_RUST_SEMANTIC_PROFILE};
use codenoesis_domain::s4_r6::{R6_FRAMEWORK_EXTRACTOR_VERSION, R6_FRAMEWORK_PROFILE};
use codenoesis_domain::storage::{
    LocalSnapshotHead, PublicationCandidate, SNAPSHOT_SCHEMA_VERSION_V11, StorageComponent,
    StorageError,
};
use codenoesis_domain::{
    AcquisitionError, LimitKind, RepositoryInventory, STANDARD_LOCAL_S1_LIMITS, limit_exceeded,
};
use serde_json::{Map, Value, json};

use super::s4::{MAX_QUERY_BYTES, QueryContractError, claim_value, evidence_value};
use super::s4_r6::{RepositorySnapshotV9, RepositorySnapshotV9Error, local_query_result_v4};
use super::{
    LimitedVecWriter, PublicationCandidateError, SnapshotEnvelopeV1, publication_candidate,
    semantic_hash,
};

pub type K1Sha256 = fn(&[u8]) -> String;

pub const K1_PORTABLE_MARKER: &str = ".codenoesis-portable-graph-v2";
pub const K1_EXPLORER_MARKER: &str = ".codenoesis-local-explorer-v2";
pub const K1_EXPLORER_SECURITY_PROFILE: &str = "codenoesis.local-explorer-security/v2";
pub const MAX_K1_PORTABLE_GRAPH_BYTES: u64 = 268_435_456;
pub const MAX_K1_JSON_NESTING: u64 = 64;
pub const MAX_K1_TEXT_SEARCH_RESULTS: u64 = 100;
pub const K1_TRAVERSAL_DEPTH_DEFAULT: u64 = 1;
pub const MAX_K1_TRAVERSAL_DEPTH: u64 = 2;

const CONFIGURATION_V8_HASH_DOMAIN: &[u8] = b"codenoesis.configuration.semantic.v8";
const SNAPSHOT_V11_HASH_DOMAIN: &[u8] = b"codenoesis.repository-snapshot.semantic.v11";
const EXTRACTION_V8_HASH_DOMAIN: &[u8] = b"codenoesis.extraction-chunk.semantic.v8";
const GRAPH_V8_HASH_DOMAIN: &[u8] = b"codenoesis.knowledge-graph.semantic.v8";
const PORTABLE_FAMILIES: [(&str, &str); 8] = [
    ("entities", "id"),
    ("relationships", "id"),
    ("claims", "id"),
    ("evidence", "id"),
    ("diagnostics", "id"),
    ("coverage_gaps", "id"),
    ("documents", "document_id"),
    ("document_statements", "statement_id"),
];

#[derive(Clone, Debug)]
pub struct CodeNoesisErrorV16 {
    value: Value,
}

impl CodeNoesisErrorV16 {
    #[must_use]
    pub fn invalid_rust_callable_profile(profile: &str) -> Self {
        Self::new(
            "input.invalid_rust_callable_profile",
            "input",
            "invalid rust callable profile",
            json!({"profile": bounded_nonempty(profile, 256, "missing")}),
        )
    }

    #[must_use]
    pub fn unsupported_composition() -> Self {
        Self::new(
            "input.unsupported_rust_callable_composition",
            "input",
            "unsupported rust callable profile composition",
            json!({
                "profile": K1_PROFILE,
                "required_lineage": "r6_source_only",
                "compiler_index_composition": false
            }),
        )
    }

    #[must_use]
    pub fn from_callable(error: &CallableSemanticsError) -> Option<Self> {
        match error {
            CallableSemanticsError::Source(_) => None,
            CallableSemanticsError::InvalidSyntax {
                path,
                start_byte,
                syntax_kind,
            } => Some(Self::new(
                "extraction.invalid_callable_syntax",
                "extraction",
                "invalid rust callable syntax",
                json!({
                    "path": bounded_repository_path(path),
                    "start_byte": start_byte,
                    "syntax_kind": bounded_nonempty(syntax_kind, 128, "invalid_syntax")
                }),
            )),
            CallableSemanticsError::IdentityConflict {
                kind,
                normalized_identity,
            } => Some(Self::new(
                "extraction.callable_identity_conflict",
                "extraction",
                "rust callable identity conflict",
                json!({
                    "kind": bounded_nonempty(kind, 128, "unknown"),
                    "normalized_identity": bounded_nonempty(normalized_identity, 512, "unknown")
                }),
            )),
            CallableSemanticsError::UnsupportedComposition => Some(Self::unsupported_composition()),
            CallableSemanticsError::LimitExceeded {
                limit,
                maximum,
                observed,
            } => Some(Self::new(
                "extraction.callable_limit_exceeded",
                "extraction",
                "rust callable limit exceeded",
                json!({
                    "limit": limit.as_str(),
                    "maximum": maximum,
                    "observed": observed
                }),
            )),
            CallableSemanticsError::ContractInvalid => Some(Self::internal()),
        }
    }

    #[must_use]
    pub fn from_contract(error: &K1ContractError) -> Self {
        match error {
            K1ContractError::UnsupportedSnapshotSchema(observed) => Self::new(
                "export.invalid_snapshot",
                "export",
                "invalid K1 source snapshot",
                json!({"observed": bounded_nonempty(observed, 256, "missing")}),
            ),
            K1ContractError::UnsupportedPortableGraphSchema(observed) => {
                Self::invalid_portable_graph(json!({
                    "observed": bounded_nonempty(observed, 256, "missing")
                }))
            }
            K1ContractError::Noncanonical {
                expected_sha256,
                observed_sha256,
            } => Self::invalid_portable_graph(json!({
                "expected_sha256": expected_sha256,
                "observed_sha256": observed_sha256
            })),
            K1ContractError::IdentityConflict { family, id }
            | K1ContractError::ReferenceMismatch { family, id } => {
                Self::invalid_portable_graph(json!({
                    "family": family,
                    "id": bounded_nonempty(id, 512, "invalid")
                }))
            }
            K1ContractError::LimitExceeded {
                limit,
                maximum,
                observed,
            } => Self::new(
                "export.limit_exceeded",
                "export",
                "portable graph limit exceeded",
                json!({"limit": limit, "maximum": maximum, "observed": observed}),
            ),
            K1ContractError::UnsafePayload { reason } => {
                Self::invalid_portable_graph(json!({"reason": reason}))
            }
            K1ContractError::AssetIntegrityMismatch => Self::new(
                "explorer.asset_integrity_mismatch",
                "explorer",
                "local explorer asset integrity mismatch",
                json!({}),
            ),
            K1ContractError::InvalidSnapshot => Self::new(
                "export.invalid_snapshot",
                "export",
                "invalid K1 source snapshot",
                json!({}),
            ),
            K1ContractError::InvalidProjection => Self::invalid_portable_graph(json!({})),
            K1ContractError::Internal => Self::internal(),
        }
    }

    #[must_use]
    pub fn from_explorer_contract(error: &K1ContractError) -> Self {
        match error {
            K1ContractError::LimitExceeded {
                limit,
                maximum,
                observed,
            } => Self::new(
                "explorer.limit_exceeded",
                "explorer",
                "local explorer input limit exceeded",
                json!({"limit": limit, "maximum": maximum, "observed": observed}),
            ),
            K1ContractError::AssetIntegrityMismatch => Self::new(
                "explorer.asset_integrity_mismatch",
                "explorer",
                "local explorer asset integrity mismatch",
                json!({}),
            ),
            K1ContractError::Internal => Self::internal(),
            _ => Self::new(
                "export.invalid_portable_graph_v2",
                "export",
                "invalid portable graph v2",
                json!({}),
            ),
        }
    }

    #[must_use]
    pub fn unsafe_output_path(path_sha256: &str, reason: &str) -> Self {
        Self::new(
            "input.unsafe_output_path",
            "input",
            "unsafe output path",
            json!({
                "path_sha256": bounded_nonempty(path_sha256, 64, "invalid"),
                "reason": bounded_nonempty(reason, 64, "unsafe")
            }),
        )
    }

    #[must_use]
    pub fn invalid_snapshot() -> Self {
        Self::new(
            "snapshot.invalid_v11",
            "snapshot",
            "invalid K1 snapshot",
            json!({}),
        )
    }

    #[must_use]
    pub fn invalid_query() -> Self {
        Self::new("query.invalid_v11", "query", "invalid K1 query", json!({}))
    }

    #[must_use]
    pub fn internal() -> Self {
        Self::new(
            "internal.unexpected",
            "internal",
            "unexpected internal error",
            json!({}),
        )
    }

    fn invalid_portable_graph(context: Value) -> Self {
        Self::new(
            "export.invalid_portable_graph_v2",
            "export",
            "invalid portable graph v2",
            context,
        )
    }

    fn new(code: &str, stage: &str, message: &str, context: Value) -> Self {
        let mut value = json!({
            "schema_version": K1_ERROR_VERSION,
            "code": code,
            "stage": stage,
            "message": message,
            "retryable": false
        });
        value["context"] = context;
        Self { value }
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Serializes one bounded `ErrorV16` followed by LF.
    ///
    /// # Errors
    ///
    /// Returns an error only if the internal JSON value cannot be serialized.
    pub fn canonical_stderr(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = serde_json::to_vec(&self.value)?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

#[derive(Clone, Debug)]
pub struct LocalQueryResultV6 {
    value: Value,
}

impl LocalQueryResultV6 {
    /// Serializes one bounded `LocalQueryResultV6` followed by LF.
    ///
    /// # Errors
    ///
    /// Returns a strict query limit or snapshot failure.
    pub fn canonical_stdout(&self) -> Result<Vec<u8>, QueryContractError> {
        let mut bytes =
            serde_json::to_vec(&self.value).map_err(|_| QueryContractError::InvalidSnapshot)?;
        if bytes.len() >= MAX_QUERY_BYTES {
            return Err(QueryContractError::LimitExceeded);
        }
        bytes.push(b'\n');
        Ok(bytes)
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }
}

/// Builds one exact-ID V6 result and its directly linked K1 records.
///
/// # Errors
///
/// Returns a strict snapshot, document, not-found, or result-limit failure.
pub fn local_query_result_v6(
    semantic: &Value,
    manifest: &Value,
    snapshot_id: &str,
    requested_id: &str,
) -> Result<LocalQueryResultV6, QueryContractError> {
    let mut value =
        local_query_result_v4(semantic, manifest, snapshot_id, requested_id)?.into_value();
    let graph = semantic
        .get("knowledge_graph")
        .ok_or(QueryContractError::InvalidSnapshot)?;
    if graph.get("schema_version").and_then(Value::as_str) != Some(K1_GRAPH_VERSION) {
        return Err(QueryContractError::InvalidSnapshot);
    }
    let entities = id_map(graph, "entities")?;
    let relationships = graph
        .get("relationships")
        .and_then(Value::as_array)
        .ok_or(QueryContractError::InvalidSnapshot)?;
    let mut linked_relationships = relationships
        .iter()
        .filter(|relationship| {
            relationship.get("source").and_then(Value::as_str) == Some(requested_id)
                || relationship.get("target").and_then(Value::as_str) == Some(requested_id)
                || relationship.get("id").and_then(Value::as_str) == Some(requested_id)
        })
        .filter(|relationship| {
            relationship
                .get("kind")
                .and_then(Value::as_str)
                .is_some_and(is_k1_relationship_kind)
        })
        .cloned()
        .collect::<Vec<_>>();
    linked_relationships.sort_by(|left, right| record_id(left).cmp(&record_id(right)));
    linked_relationships.dedup_by(|left, right| record_id(left) == record_id(right));

    let linked_ids = linked_relationships
        .iter()
        .flat_map(|relationship| {
            [
                relationship.get("source").and_then(Value::as_str),
                relationship.get("target").and_then(Value::as_str),
            ]
        })
        .flatten()
        .filter(|id| *id != requested_id)
        .collect::<BTreeSet<_>>();
    let mut linked_entities = linked_ids
        .into_iter()
        .filter_map(|id| entities.get(id))
        .filter(|entity| {
            entity
                .get("kind")
                .and_then(Value::as_str)
                .is_some_and(is_k1_entity_kind)
        })
        .map(|entity| (*entity).clone())
        .collect::<Vec<_>>();
    linked_entities.sort_by(|left, right| record_id(left).cmp(&record_id(right)));
    value["schema_version"] = Value::String(K1_QUERY_VERSION.to_owned());
    value["linked_k1_entities"] = Value::Array(linked_entities);
    value["linked_k1_relationships"] = Value::Array(linked_relationships);
    let result = LocalQueryResultV6 { value };
    result.canonical_stdout()?;
    Ok(result)
}

#[derive(Clone, Debug)]
pub struct RepositorySnapshotV11 {
    value: Value,
}

#[derive(Debug)]
pub enum RepositorySnapshotV11Error {
    Serialization(serde_json::Error),
    LimitExceeded(AcquisitionError),
    ContractInvalid,
    OutputLengthOverflow,
}

impl Display for RepositorySnapshotV11Error {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Serialization(_) => "K1 snapshot serialization failed",
            Self::LimitExceeded(_) => "K1 snapshot output limit exceeded",
            Self::ContractInvalid => "K1 snapshot contract is invalid",
            Self::OutputLengthOverflow => "K1 snapshot output length overflowed",
        })
    }
}

impl Error for RepositorySnapshotV11Error {}

impl RepositorySnapshotV11 {
    /// Builds selector-bound V11 over the complete immutable R6 lineage.
    ///
    /// # Errors
    ///
    /// Returns a K1 validation, serialization, publication, or bound failure.
    pub fn from_inventory_and_callable_semantics(
        inventory: &RepositoryInventory,
        knowledge: &CallableSemanticsKnowledge,
        envelope: SnapshotEnvelopeV1,
    ) -> Result<Self, RepositorySnapshotV11Error> {
        knowledge
            .validate()
            .map_err(|_| RepositorySnapshotV11Error::ContractInvalid)?;
        let baseline = RepositorySnapshotV9::from_inventory_and_framework_declarations(
            inventory,
            &knowledge.framework,
            None,
            envelope,
        )
        .map_err(map_v9_error)?;
        let mut value = baseline.value().clone();
        let semantic = value
            .get_mut("semantic")
            .and_then(Value::as_object_mut)
            .ok_or(RepositorySnapshotV11Error::ContractInvalid)?;
        let configuration_without_hash = json!({
            "schema_version": K1_CONFIGURATION_VERSION,
            "profile": "standard-local-s4",
            "workspace_profile": R3_WORKSPACE_PROFILE,
            "manifest_profile": R4_MANIFEST_PROFILE,
            "rust_semantic_profile": R5_RUST_SEMANTIC_PROFILE,
            "rust_framework_profile": R6_FRAMEWORK_PROFILE,
            "rust_callable_profile": K1_PROFILE,
            "repository_boundary_profile": null
        });
        let configuration_hash =
            semantic_hash(CONFIGURATION_V8_HASH_DOMAIN, &configuration_without_hash);
        let mut configuration = configuration_without_hash;
        configuration
            .as_object_mut()
            .ok_or(RepositorySnapshotV11Error::ContractInvalid)?
            .insert(
                "semantic_hash".to_owned(),
                json!({"algorithm": "blake3-256", "value": configuration_hash}),
            );
        semantic.insert("configuration".to_owned(), configuration);
        semantic.insert(
            "pipeline_version".to_owned(),
            Value::String(K1_PIPELINE_VERSION.to_owned()),
        );
        semantic.insert(
            "ontology_version".to_owned(),
            Value::String(K1_ONTOLOGY_VERSION.to_owned()),
        );
        semantic.insert(
            "extractor_contract_version".to_owned(),
            Value::String(K1_EXTRACTION_CONTRACT_VERSION.to_owned()),
        );
        semantic.insert(
            "extractor_versions".to_owned(),
            json!([
                "codenoesis.inventory-classifier/s1-v1",
                "codenoesis.rust-tree-sitter/s4-v1",
                "codenoesis.rust-workspace/s4-r3-v1",
                "codenoesis.cargo-manifest/s4-r4-v1",
                R5_RUST_SEMANTIC_EXTRACTOR_VERSION,
                R6_FRAMEWORK_EXTRACTOR_VERSION,
                K1_EXTRACTOR_VERSION
            ]),
        );
        let baseline_chunks = semantic
            .get("extraction_chunks")
            .cloned()
            .ok_or(RepositorySnapshotV11Error::ContractInvalid)?;
        semantic.insert(
            "extraction_chunks".to_owned(),
            Value::Array(extraction_chunks_v8(&baseline_chunks, knowledge)?),
        );
        let baseline_graph = semantic
            .get("knowledge_graph")
            .cloned()
            .ok_or(RepositorySnapshotV11Error::ContractInvalid)?;
        semantic.insert(
            "knowledge_graph".to_owned(),
            knowledge_graph_v8(&baseline_graph, knowledge)?,
        );
        let semantic_value = Value::Object(semantic.clone());
        let snapshot_hash = semantic_hash(SNAPSHOT_V11_HASH_DOMAIN, &semantic_value);
        let root = value
            .as_object_mut()
            .ok_or(RepositorySnapshotV11Error::ContractInvalid)?;
        root.insert(
            "schema_version".to_owned(),
            Value::String(K1_SNAPSHOT_VERSION.to_owned()),
        );
        root.insert(
            "semantic_hash".to_owned(),
            json!({"algorithm": "blake3-256", "value": snapshot_hash}),
        );
        publication_candidate(&value).map_err(|_| RepositorySnapshotV11Error::ContractInvalid)?;
        Ok(Self { value })
    }

    /// Serializes the complete V11 snapshot with the inherited output bound.
    ///
    /// # Errors
    ///
    /// Returns a serialization or output-limit failure.
    pub fn canonical_stdout(&self) -> Result<Vec<u8>, RepositorySnapshotV11Error> {
        let maximum = usize::try_from(STANDARD_LOCAL_S1_LIMITS.canonical_output_bytes)
            .map_err(|_| RepositorySnapshotV11Error::OutputLengthOverflow)?;
        let body_maximum = maximum
            .checked_sub(1)
            .ok_or(RepositorySnapshotV11Error::OutputLengthOverflow)?;
        let mut writer = LimitedVecWriter::new(body_maximum);
        let result = serde_json::to_writer(&mut writer, &self.value);
        if writer.overflowed() {
            return Err(RepositorySnapshotV11Error::LimitExceeded(limit_exceeded(
                LimitKind::CanonicalOutputBytes,
                STANDARD_LOCAL_S1_LIMITS
                    .canonical_output_bytes
                    .saturating_add(1),
            )));
        }
        result.map_err(RepositorySnapshotV11Error::Serialization)?;
        let mut bytes = writer.into_inner();
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Serializes the exact V11 semantic payload.
    ///
    /// # Errors
    ///
    /// Returns an error only for an invalid internal value.
    pub fn canonical_semantic(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(&self.value["semantic"])
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Converts V11 into the immutable publication model.
    ///
    /// # Errors
    ///
    /// Returns a strict contract or storage-integrity failure.
    pub fn publication_candidate(&self) -> Result<PublicationCandidate, PublicationCandidateError> {
        publication_candidate(&self.value)
    }
}

/// Validates one loaded V11 semantic payload against the visible head.
///
/// # Errors
///
/// Returns a typed storage-integrity failure on any mismatch.
pub fn validate_stored_snapshot_semantic_v11(
    semantic: &Value,
    head: &LocalSnapshotHead,
) -> Result<(), StorageError> {
    if head.snapshot_schema_version != SNAPSHOT_SCHEMA_VERSION_V11 {
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

#[derive(Clone, Debug)]
pub struct PortableGraphV2 {
    value: Value,
    canonical: Vec<u8>,
    sha256: K1Sha256,
}

impl PortableGraphV2 {
    /// Projects one validated V11 head and deterministic documentation manifest.
    ///
    /// # Errors
    ///
    /// Returns a strict binding, privacy, reference, or limit failure.
    pub fn from_validated_v11(
        semantic: &Value,
        head: &LocalSnapshotHead,
        documentation_manifest: &Value,
        sha256: K1Sha256,
    ) -> Result<Self, K1ContractError> {
        validate_stored_snapshot_semantic_v11(semantic, head)
            .map_err(|_| K1ContractError::InvalidSnapshot)?;
        if semantic.get("ontology_version").and_then(Value::as_str) != Some(K1_ONTOLOGY_VERSION) {
            return Err(K1ContractError::InvalidSnapshot);
        }
        validate_documentation_binding(documentation_manifest, head)?;
        let graph = semantic
            .get("knowledge_graph")
            .and_then(Value::as_object)
            .ok_or(K1ContractError::InvalidSnapshot)?;
        let (documents, document_statements) = portable_documents(documentation_manifest)?;
        let mut value = json!({
            "schema_version": K1_PORTABLE_GRAPH_VERSION,
            "repository": semantic.get("repository").cloned().ok_or(K1ContractError::InvalidSnapshot)?,
            "source_snapshot": {
                "schema_version": SNAPSHOT_SCHEMA_VERSION_V11,
                "snapshot_id": head.snapshot_id.as_str(),
                "semantic_hash": {
                    "algorithm": head.semantic_hash.algorithm,
                    "value": head.semantic_hash.value
                }
            },
            "ontology_version": K1_ONTOLOGY_VERSION,
            "query_contract_version": K1_QUERY_VERSION,
            "projection": {
                "profile": "codenoesis.lossless-portable-projection/v2",
                "family_sha256": {}
            },
            "entities": graph.get("entities").cloned().ok_or(K1ContractError::InvalidSnapshot)?,
            "relationships": graph.get("relationships").cloned().ok_or(K1ContractError::InvalidSnapshot)?,
            "claims": graph.get("claims").cloned().ok_or(K1ContractError::InvalidSnapshot)?,
            "evidence": graph.get("evidence").cloned().ok_or(K1ContractError::InvalidSnapshot)?,
            "diagnostics": graph.get("diagnostics").cloned().ok_or(K1ContractError::InvalidSnapshot)?,
            "coverage_gaps": graph.get("coverage").cloned().ok_or(K1ContractError::InvalidSnapshot)?,
            "documents": documents,
            "document_statements": document_statements
        });
        let digests = family_digests(&value, sha256)?;
        value["projection"]["family_sha256"] = digests;
        Self::from_generated_value(value, sha256)
    }

    /// Strictly reimports one canonical LF-terminated `PortableGraphV2`.
    ///
    /// # Errors
    ///
    /// Returns the first decode, schema, identity, reference, privacy, or limit failure.
    pub fn from_canonical_file(bytes: &[u8], sha256: K1Sha256) -> Result<Self, K1ContractError> {
        enforce_portable_size(bytes.len())?;
        let value = serde_json::from_slice::<Value>(bytes)
            .map_err(|_| K1ContractError::InvalidProjection)?;
        validate_portable_value(&value, sha256)?;
        let canonical = serde_json::to_vec(&value).map_err(|_| K1ContractError::Internal)?;
        let mut expected = canonical.clone();
        expected.push(b'\n');
        if expected != bytes {
            return Err(K1ContractError::Noncanonical {
                expected_sha256: sha256(&expected),
                observed_sha256: sha256(bytes),
            });
        }
        Ok(Self {
            value,
            canonical,
            sha256,
        })
    }

    fn from_generated_value(value: Value, sha256: K1Sha256) -> Result<Self, K1ContractError> {
        validate_portable_value(&value, sha256)?;
        let canonical = serde_json::to_vec(&value).map_err(|_| K1ContractError::Internal)?;
        enforce_portable_size(canonical.len().saturating_add(1))?;
        Ok(Self {
            value,
            canonical,
            sha256,
        })
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    #[must_use]
    pub fn canonical_file(&self) -> Vec<u8> {
        let mut bytes = self.canonical.clone();
        bytes.push(b'\n');
        bytes
    }

    #[must_use]
    pub fn canonical_sha256(&self) -> String {
        (self.sha256)(&self.canonical_file())
    }
}

#[derive(Clone, Debug)]
pub struct LocalExplorerManifestV2 {
    value: Value,
}

impl LocalExplorerManifestV2 {
    /// Builds the offline V2 explorer manifest bound to exact graph and viewer bytes.
    ///
    /// # Errors
    ///
    /// Returns an integrity or unsafe-CSP failure.
    pub fn new(
        portable: &PortableGraphV2,
        viewer_bytes: &[u8],
        expected_viewer_sha256: &str,
        content_security_policy: &str,
        sha256: K1Sha256,
    ) -> Result<Self, K1ContractError> {
        if sha256(viewer_bytes) != expected_viewer_sha256
            || content_security_policy.contains("http:")
            || content_security_policy.contains("https:")
            || content_security_policy.contains("unsafe-inline")
            || content_security_policy.contains("unsafe-eval")
        {
            return Err(K1ContractError::AssetIntegrityMismatch);
        }
        Ok(Self {
            value: json!({
                "schema_version": K1_LOCAL_EXPLORER_VERSION,
                "portable_graph": {
                    "path": "portable-graph.json",
                    "sha256": portable.canonical_sha256(),
                    "byte_length": portable.canonical_file().len()
                },
                "entrypoint": {
                    "path": "index.html",
                    "sha256": expected_viewer_sha256,
                    "byte_length": viewer_bytes.len()
                },
                "security": {
                    "profile": K1_EXPLORER_SECURITY_PROFILE,
                    "content_security_policy": content_security_policy,
                    "network": false,
                    "dynamic_code": false,
                    "storage": false,
                    "telemetry": false
                },
                "capabilities": {
                    "exact_id_search": true,
                    "text_search": true,
                    "typed_filters": true,
                    "evidence_inspection": true,
                    "bounded_traversal": [1, 2]
                },
                "limits": {
                    "text_search_results": MAX_K1_TEXT_SEARCH_RESULTS,
                    "traversal_depth_default": K1_TRAVERSAL_DEPTH_DEFAULT,
                    "traversal_depth_maximum": MAX_K1_TRAVERSAL_DEPTH
                }
            }),
        })
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Serializes one `LocalExplorerManifestV2` followed by LF.
    ///
    /// # Errors
    ///
    /// Returns an error only if the internal JSON cannot be serialized.
    pub fn canonical_file(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = serde_json::to_vec(&self.value)?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum K1ContractError {
    InvalidSnapshot,
    UnsupportedSnapshotSchema(String),
    UnsupportedPortableGraphSchema(String),
    Noncanonical {
        expected_sha256: String,
        observed_sha256: String,
    },
    IdentityConflict {
        family: &'static str,
        id: String,
    },
    ReferenceMismatch {
        family: &'static str,
        id: String,
    },
    LimitExceeded {
        limit: &'static str,
        maximum: u64,
        observed: u64,
    },
    UnsafePayload {
        reason: &'static str,
    },
    AssetIntegrityMismatch,
    InvalidProjection,
    Internal,
}

impl Display for K1ContractError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidSnapshot => "invalid K1 snapshot",
            Self::UnsupportedSnapshotSchema(_) => "unsupported K1 snapshot schema",
            Self::UnsupportedPortableGraphSchema(_) => "unsupported portable graph schema",
            Self::Noncanonical { .. } => "noncanonical portable graph",
            Self::IdentityConflict { .. } => "portable identity conflict",
            Self::ReferenceMismatch { .. } => "portable reference mismatch",
            Self::LimitExceeded { .. } => "portable graph limit exceeded",
            Self::UnsafePayload { .. } => "unsafe portable payload",
            Self::AssetIntegrityMismatch => "explorer asset integrity mismatch",
            Self::InvalidProjection => "invalid portable graph projection",
            Self::Internal => "internal K1 contract error",
        })
    }
}

impl Error for K1ContractError {}

fn extraction_chunks_v8(
    baseline: &Value,
    knowledge: &CallableSemanticsKnowledge,
) -> Result<Vec<Value>, RepositorySnapshotV11Error> {
    let mut additions = knowledge
        .extraction_chunks
        .iter()
        .map(|chunk| (chunk.source_file_id.as_str(), chunk))
        .collect::<BTreeMap<_, _>>();
    let chunks = baseline
        .as_array()
        .ok_or(RepositorySnapshotV11Error::ContractInvalid)?;
    let mut transformed = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        let mut value = chunk.clone();
        let object = value
            .as_object_mut()
            .ok_or(RepositorySnapshotV11Error::ContractInvalid)?;
        object.insert(
            "schema_version".to_owned(),
            Value::String(K1_EXTRACTION_CHUNK_VERSION.to_owned()),
        );
        object.insert(
            "ontology_version".to_owned(),
            Value::String(K1_ONTOLOGY_VERSION.to_owned()),
        );
        object.remove("semantic_hash");
        if object
            .get("subject")
            .and_then(|subject| subject.get("kind"))
            .and_then(Value::as_str)
            == Some("rust_source")
        {
            let source_file_id = object
                .get("subject")
                .and_then(|subject| subject.get("source_file_id"))
                .and_then(Value::as_str)
                .ok_or(RepositorySnapshotV11Error::ContractInvalid)?
                .to_owned();
            let callable = additions
                .remove(source_file_id.as_str())
                .ok_or(RepositorySnapshotV11Error::ContractInvalid)?;
            object.insert("source_file_id".to_owned(), Value::String(source_file_id));
            object.insert(
                "callable_semantics_profile".to_owned(),
                Value::String(K1_PROFILE.to_owned()),
            );
            merge_id_array(
                object,
                "entities",
                callable.entities.iter().map(callable_entity_value),
            )?;
            merge_id_array(
                object,
                "relationships",
                callable
                    .relationships
                    .iter()
                    .map(callable_relationship_value),
            )?;
            merge_id_array(object, "claims", callable.claims.iter().map(claim_value))?;
            merge_id_array(
                object,
                "evidence",
                callable.evidence.iter().map(evidence_value),
            )?;
            merge_id_array(
                object,
                "diagnostics",
                callable.diagnostics.iter().map(callable_diagnostic_value),
            )?;
            merge_id_array(
                object,
                "coverage",
                callable.coverage.iter().map(callable_coverage_value),
            )?;
        }
        insert_semantic_hash(&mut value, EXTRACTION_V8_HASH_DOMAIN)?;
        transformed.push(value);
    }
    if !additions.is_empty() {
        return Err(RepositorySnapshotV11Error::ContractInvalid);
    }
    Ok(transformed)
}

fn knowledge_graph_v8(
    baseline: &Value,
    knowledge: &CallableSemanticsKnowledge,
) -> Result<Value, RepositorySnapshotV11Error> {
    let mut value = baseline.clone();
    let object = value
        .as_object_mut()
        .ok_or(RepositorySnapshotV11Error::ContractInvalid)?;
    object.insert(
        "schema_version".to_owned(),
        Value::String(K1_GRAPH_VERSION.to_owned()),
    );
    object.insert(
        "ontology_version".to_owned(),
        Value::String(K1_ONTOLOGY_VERSION.to_owned()),
    );
    object.insert(
        "extractor_versions".to_owned(),
        json!([
            "codenoesis.rust-tree-sitter/s4-v1",
            "codenoesis.rust-workspace/s4-r3-v1",
            "codenoesis.cargo-manifest/s4-r4-v1",
            R5_RUST_SEMANTIC_EXTRACTOR_VERSION,
            R6_FRAMEWORK_EXTRACTOR_VERSION,
            K1_EXTRACTOR_VERSION
        ]),
    );
    object.insert(
        "callable_semantics_index".to_owned(),
        json!({
            "schema_version": K1_INDEX_VERSION,
            "profile": K1_PROFILE,
            "signature_ids": knowledge.graph.index.signature_ids,
            "parameter_ids": knowledge.graph.index.parameter_ids,
            "declared_value_ids": knowledge.graph.index.declared_value_ids,
            "body_fact_ids": knowledge.graph.index.body_fact_ids,
            "resolved_call_relationship_ids": knowledge.graph.index.resolved_call_relationship_ids,
            "unresolved_call_site_ids": knowledge.graph.index.unresolved_call_site_ids
        }),
    );
    object.remove("semantic_hash");
    merge_id_array(
        object,
        "entities",
        knowledge.graph.entities.iter().map(callable_entity_value),
    )?;
    merge_id_array(
        object,
        "relationships",
        knowledge
            .graph
            .relationships
            .iter()
            .map(callable_relationship_value),
    )?;
    merge_id_array(
        object,
        "claims",
        knowledge.graph.claims.iter().map(claim_value),
    )?;
    merge_id_array(
        object,
        "evidence",
        knowledge.graph.evidence.iter().map(evidence_value),
    )?;
    merge_id_array(
        object,
        "diagnostics",
        knowledge
            .graph
            .diagnostics
            .iter()
            .map(callable_diagnostic_value),
    )?;
    merge_id_array(
        object,
        "coverage",
        knowledge.graph.coverage.iter().map(callable_coverage_value),
    )?;
    insert_semantic_hash(&mut value, GRAPH_V8_HASH_DOMAIN)?;
    Ok(value)
}

fn callable_entity_value(entity: &CallableSemanticEntity) -> Value {
    json!({
        "id": entity.id,
        "kind": entity.kind.as_str(),
        "crate_id": entity.crate_id,
        "module_path": entity.module_path,
        "name": entity.name,
        "subject_id": entity.subject_id,
        "ordinal": entity.ordinal,
        "evidence_ids": entity.evidence_ids,
        "properties": callable_properties_value(&entity.properties)
    })
}

fn callable_properties_value(properties: &CallableSemanticProperties) -> Value {
    match properties {
        CallableSemanticProperties::Signature(value) => json!({
            "visibility": value.visibility,
            "async": value.is_async,
            "const": value.is_const,
            "unsafe": value.is_unsafe,
            "abi": value.abi,
            "generic_parameters": value.generic_parameters,
            "where_clause": value.where_clause,
            "return_state": value.return_state.as_str(),
            "return_type": value.return_type,
            "body_state": value.body_state.as_str(),
            "body_digest": value.body_digest,
            "body_evidence_id": value.body_evidence_id
        }),
        CallableSemanticProperties::Parameter(value) => json!({
            "pattern": value.pattern,
            "declared_type": value.declared_type,
            "receiver_state": value.receiver_state.as_str()
        }),
        CallableSemanticProperties::DeclaredValue(value) => declared_value_properties(value),
        CallableSemanticProperties::LocalBinding(value) => local_binding_properties(value),
        CallableSemanticProperties::CallSite(value) => call_site_properties(value),
        CallableSemanticProperties::Control(value) => control_properties(value),
    }
}

fn declared_value_properties(value: &DeclaredValueProperties) -> Value {
    json!({
        "state": value.state.as_str(),
        "syntax_kind": value.syntax_kind,
        "expression_digest": value.expression_digest,
        "expression_byte_length": value.expression_byte_length,
        "normalized": value.normalized.as_ref().map(normalized_scalar_value)
    })
}

fn normalized_scalar_value(value: &NormalizedScalarValue) -> Value {
    match value {
        NormalizedScalarValue::Boolean(value) => json!({
            "kind": "boolean",
            "value": value
        }),
        NormalizedScalarValue::Integer {
            sign,
            radix,
            digits,
            suffix,
        } => json!({
            "kind": "integer",
            "sign": sign,
            "radix": radix,
            "digits": digits,
            "suffix": suffix
        }),
        NormalizedScalarValue::Character(value) => json!({
            "kind": "character",
            "value": value
        }),
        NormalizedScalarValue::String(value) => json!({
            "kind": "string",
            "value": value
        }),
    }
}

fn local_binding_properties(value: &LocalBindingProperties) -> Value {
    json!({
        "pattern": value.pattern,
        "declared_type": value.declared_type,
        "initializer_present": value.initializer_present,
        "lexical_depth": value.lexical_depth,
        "parent_fact_id": value.parent_fact_id
    })
}

fn call_site_properties(value: &CallSiteProperties) -> Value {
    json!({
        "form": value.form.as_str(),
        "target_spelling": value.target_spelling,
        "resolution_state": value.resolution_state.as_str(),
        "resolved_target_id": value.resolved_target_id,
        "lexical_depth": value.lexical_depth,
        "parent_fact_id": value.parent_fact_id
    })
}

fn control_properties(value: &ControlProperties) -> Value {
    json!({
        "control_kind": value.control_kind.as_str(),
        "lexical_depth": value.lexical_depth,
        "parent_fact_id": value.parent_fact_id
    })
}

fn callable_relationship_value(relationship: &CallableRelationship) -> Value {
    json!({
        "id": relationship.id,
        "kind": relationship.kind.as_str(),
        "source": relationship.source,
        "target": relationship.target,
        "evidence_ids": relationship.evidence_ids
    })
}

fn callable_diagnostic_value(diagnostic: &CallableDiagnostic) -> Value {
    json!({
        "id": diagnostic.id,
        "code": diagnostic.code,
        "message": diagnostic.message,
        "subject_id": diagnostic.subject_id,
        "evidence_ids": diagnostic.evidence_ids
    })
}

fn callable_coverage_value(gap: &CallableCoverageGap) -> Value {
    json!({
        "id": gap.id,
        "capability": gap.capability,
        "state": gap.state.as_str(),
        "subject_id": gap.subject_id,
        "evidence_ids": gap.evidence_ids
    })
}

fn merge_id_array(
    object: &mut Map<String, Value>,
    field: &'static str,
    additions: impl IntoIterator<Item = Value>,
) -> Result<(), RepositorySnapshotV11Error> {
    let values = object
        .get_mut(field)
        .and_then(Value::as_array_mut)
        .ok_or(RepositorySnapshotV11Error::ContractInvalid)?;
    values.extend(additions);
    values.sort_by(|left, right| record_id(left).cmp(&record_id(right)));
    let mut retained = Vec::with_capacity(values.len());
    for value in values.drain(..) {
        if let Some(previous) = retained.last()
            && record_id(previous) == record_id(&value)
        {
            if previous != &value {
                return Err(RepositorySnapshotV11Error::ContractInvalid);
            }
            continue;
        }
        retained.push(value);
    }
    *values = retained;
    Ok(())
}

fn insert_semantic_hash(
    value: &mut Value,
    domain: &[u8],
) -> Result<(), RepositorySnapshotV11Error> {
    let hash = semantic_hash(domain, value);
    value
        .as_object_mut()
        .ok_or(RepositorySnapshotV11Error::ContractInvalid)?
        .insert(
            "semantic_hash".to_owned(),
            json!({"algorithm": "blake3-256", "value": hash}),
        );
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn validate_portable_value(value: &Value, sha256: K1Sha256) -> Result<(), K1ContractError> {
    let object = value
        .as_object()
        .ok_or(K1ContractError::InvalidProjection)?;
    let expected_keys = BTreeSet::from([
        "claims",
        "coverage_gaps",
        "diagnostics",
        "document_statements",
        "documents",
        "entities",
        "evidence",
        "ontology_version",
        "projection",
        "query_contract_version",
        "relationships",
        "repository",
        "schema_version",
        "source_snapshot",
    ]);
    if object.keys().map(String::as_str).collect::<BTreeSet<_>>() != expected_keys {
        return Err(K1ContractError::InvalidProjection);
    }
    let schema = object
        .get("schema_version")
        .and_then(Value::as_str)
        .unwrap_or("missing");
    if schema != K1_PORTABLE_GRAPH_VERSION {
        return Err(K1ContractError::UnsupportedPortableGraphSchema(
            bounded_nonempty(schema, 256, "missing"),
        ));
    }
    if value
        .pointer("/source_snapshot/schema_version")
        .and_then(Value::as_str)
        != Some(SNAPSHOT_SCHEMA_VERSION_V11)
        || object.get("ontology_version").and_then(Value::as_str) != Some(K1_ONTOLOGY_VERSION)
        || object.get("query_contract_version").and_then(Value::as_str) != Some(K1_QUERY_VERSION)
    {
        return Err(K1ContractError::InvalidProjection);
    }
    ensure_nesting(value, 0)?;
    let mut ids = BTreeSet::new();
    let mut family_ids = BTreeMap::new();
    for (family, key) in PORTABLE_FAMILIES {
        let values = object
            .get(family)
            .and_then(Value::as_array)
            .ok_or(K1ContractError::InvalidProjection)?;
        let mut previous = None;
        let mut current = BTreeSet::new();
        for record in values {
            let id = record
                .get(key)
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
                .ok_or(K1ContractError::InvalidProjection)?;
            if previous.is_some_and(|value: &str| value >= id) || !current.insert(id.to_owned()) {
                return Err(K1ContractError::IdentityConflict {
                    family,
                    id: id.to_owned(),
                });
            }
            previous = Some(id);
            if matches!(family, "entities" | "relationships" | "claims" | "evidence") {
                ids.insert(id.to_owned());
            }
        }
        family_ids.insert(family, current);
    }
    let entity_ids = &family_ids["entities"];
    let relationship_ids = &family_ids["relationships"];
    let evidence_ids = &family_ids["evidence"];
    for relationship in object["relationships"]
        .as_array()
        .ok_or(K1ContractError::InvalidProjection)?
    {
        for field in ["source", "target"] {
            let id = relationship
                .get(field)
                .and_then(Value::as_str)
                .ok_or(K1ContractError::InvalidProjection)?;
            if !entity_ids.contains(id) {
                return Err(K1ContractError::ReferenceMismatch {
                    family: "relationships",
                    id: id.to_owned(),
                });
            }
        }
        validate_evidence_ids(relationship, evidence_ids, "relationships")?;
    }
    for claim in object["claims"]
        .as_array()
        .ok_or(K1ContractError::InvalidProjection)?
    {
        let subject_id = claim
            .get("subject_id")
            .and_then(Value::as_str)
            .ok_or(K1ContractError::InvalidProjection)?;
        let subject_kind = claim
            .get("subject_kind")
            .and_then(Value::as_str)
            .ok_or(K1ContractError::InvalidProjection)?;
        let valid = match subject_kind {
            "entity" => entity_ids.contains(subject_id),
            "relationship" => relationship_ids.contains(subject_id),
            _ => false,
        };
        if !valid {
            return Err(K1ContractError::ReferenceMismatch {
                family: "claims",
                id: subject_id.to_owned(),
            });
        }
        validate_evidence_ids(claim, evidence_ids, "claims")?;
    }
    for family in ["entities", "diagnostics", "coverage_gaps"] {
        for record in object[family]
            .as_array()
            .ok_or(K1ContractError::InvalidProjection)?
        {
            if record.get("evidence_ids").is_some() {
                validate_evidence_ids(record, evidence_ids, family)?;
            }
        }
    }
    validate_portable_paths(value)?;
    validate_private_fields(value)?;
    let observed = value
        .pointer("/projection/family_sha256")
        .ok_or(K1ContractError::InvalidProjection)?;
    if observed != &family_digests(value, sha256)? {
        return Err(K1ContractError::InvalidProjection);
    }
    if ids.is_empty() {
        return Err(K1ContractError::InvalidProjection);
    }
    Ok(())
}

fn family_digests(value: &Value, sha256: K1Sha256) -> Result<Value, K1ContractError> {
    let mut digests = Map::new();
    for (family, _) in PORTABLE_FAMILIES {
        let bytes = serde_json::to_vec(
            value
                .get(family)
                .ok_or(K1ContractError::InvalidProjection)?,
        )
        .map_err(|_| K1ContractError::Internal)?;
        digests.insert(family.to_owned(), Value::String(sha256(&bytes)));
    }
    Ok(Value::Object(digests))
}

fn portable_documents(manifest: &Value) -> Result<(Vec<Value>, Vec<Value>), K1ContractError> {
    let source = manifest
        .get("documents")
        .and_then(Value::as_array)
        .ok_or(K1ContractError::InvalidSnapshot)?;
    let mut documents = Vec::with_capacity(source.len());
    let mut statements = Vec::new();
    for document in source {
        let mut record = document
            .as_object()
            .cloned()
            .ok_or(K1ContractError::InvalidSnapshot)?;
        let mut document_statements = record
            .remove("statements")
            .and_then(|value| value.as_array().cloned())
            .ok_or(K1ContractError::InvalidSnapshot)?;
        documents.push(Value::Object(record));
        statements.append(&mut document_statements);
    }
    documents.sort_by(|left, right| {
        left.get("document_id")
            .and_then(Value::as_str)
            .cmp(&right.get("document_id").and_then(Value::as_str))
    });
    statements.sort_by(|left, right| {
        left.get("statement_id")
            .and_then(Value::as_str)
            .cmp(&right.get("statement_id").and_then(Value::as_str))
    });
    Ok((documents, statements))
}

fn validate_documentation_binding(
    manifest: &Value,
    head: &LocalSnapshotHead,
) -> Result<(), K1ContractError> {
    if manifest.get("schema_version").and_then(Value::as_str)
        != Some("codenoesis.documentation-manifest/v1")
        || manifest.get("repository_identity").and_then(Value::as_str)
            != Some(head.repository_identity.as_str())
        || manifest.get("snapshot_id").and_then(Value::as_str) != Some(head.snapshot_id.as_str())
    {
        return Err(K1ContractError::InvalidSnapshot);
    }
    Ok(())
}

fn validate_evidence_ids(
    record: &Value,
    evidence_ids: &BTreeSet<String>,
    family: &'static str,
) -> Result<(), K1ContractError> {
    let references = record
        .get("evidence_ids")
        .and_then(Value::as_array)
        .ok_or(K1ContractError::InvalidProjection)?;
    for reference in references {
        let id = reference
            .as_str()
            .ok_or(K1ContractError::InvalidProjection)?;
        if !evidence_ids.contains(id) {
            return Err(K1ContractError::ReferenceMismatch {
                family,
                id: id.to_owned(),
            });
        }
    }
    Ok(())
}

fn validate_portable_paths(value: &Value) -> Result<(), K1ContractError> {
    for evidence in value["evidence"]
        .as_array()
        .ok_or(K1ContractError::InvalidProjection)?
    {
        let path = evidence
            .get("path")
            .and_then(Value::as_str)
            .ok_or(K1ContractError::InvalidProjection)?;
        if !safe_relative_path(path) {
            return Err(K1ContractError::UnsafePayload {
                reason: "unsafe_evidence_path",
            });
        }
    }
    for document in value["documents"]
        .as_array()
        .ok_or(K1ContractError::InvalidProjection)?
    {
        if let Some(path) = document.get("path").and_then(Value::as_str)
            && !safe_relative_path(path)
        {
            return Err(K1ContractError::UnsafePayload {
                reason: "unsafe_document_path",
            });
        }
    }
    Ok(())
}

fn validate_private_fields(value: &Value) -> Result<(), K1ContractError> {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                if matches!(
                    key.as_str(),
                    "body_text"
                        | "expression_text"
                        | "source_contents"
                        | "source_snippet"
                        | "absolute_path"
                        | "environment"
                ) {
                    return Err(K1ContractError::UnsafePayload {
                        reason: "private_field",
                    });
                }
                validate_private_fields(value)?;
            }
        }
        Value::Array(values) => {
            for value in values {
                validate_private_fields(value)?;
            }
        }
        Value::String(value) if value.to_ascii_lowercase().contains("</script") => {
            return Err(K1ContractError::UnsafePayload {
                reason: "script_close_sequence",
            });
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    Ok(())
}

fn ensure_nesting(value: &Value, depth: u64) -> Result<(), K1ContractError> {
    if depth > MAX_K1_JSON_NESTING {
        return Err(K1ContractError::LimitExceeded {
            limit: "json_nesting",
            maximum: MAX_K1_JSON_NESTING,
            observed: depth,
        });
    }
    match value {
        Value::Array(values) => {
            for value in values {
                ensure_nesting(value, depth.saturating_add(1))?;
            }
        }
        Value::Object(object) => {
            for value in object.values() {
                ensure_nesting(value, depth.saturating_add(1))?;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    Ok(())
}

fn enforce_portable_size(length: usize) -> Result<(), K1ContractError> {
    let observed = u64::try_from(length).unwrap_or(u64::MAX);
    if observed > MAX_K1_PORTABLE_GRAPH_BYTES {
        return Err(K1ContractError::LimitExceeded {
            limit: "portable_graph_bytes",
            maximum: MAX_K1_PORTABLE_GRAPH_BYTES,
            observed,
        });
    }
    Ok(())
}

fn id_map<'a>(
    value: &'a Value,
    field: &'static str,
) -> Result<BTreeMap<&'a str, &'a Value>, QueryContractError> {
    let values = value
        .get(field)
        .and_then(Value::as_array)
        .ok_or(QueryContractError::InvalidSnapshot)?;
    let mut result = BTreeMap::new();
    for value in values {
        let id = record_id(value).ok_or(QueryContractError::InvalidSnapshot)?;
        if result.insert(id, value).is_some() {
            return Err(QueryContractError::InvalidSnapshot);
        }
    }
    Ok(result)
}

fn is_k1_entity_kind(kind: &str) -> bool {
    matches!(
        kind,
        "rust.callable_signature"
            | "rust.parameter"
            | "rust.declared_value"
            | "rust.local_binding"
            | "rust.call_site"
            | "rust.control"
    )
}

fn is_k1_relationship_kind(kind: &str) -> bool {
    matches!(
        kind,
        "HAS_SIGNATURE" | "HAS_PARAMETER" | "DECLARES_VALUE" | "HAS_BODY_FACT" | "CALLS"
    )
}

fn safe_relative_path(value: &str) -> bool {
    let path = Path::new(value);
    !value.is_empty()
        && !path.is_absolute()
        && !value.contains('\\')
        && !value.chars().any(char::is_control)
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn record_id(value: &Value) -> Option<&str> {
    value.get("id").and_then(Value::as_str)
}

fn map_v9_error(error: RepositorySnapshotV9Error) -> RepositorySnapshotV11Error {
    match error {
        RepositorySnapshotV9Error::Serialization(error) => {
            RepositorySnapshotV11Error::Serialization(error)
        }
        RepositorySnapshotV9Error::LimitExceeded(error) => {
            RepositorySnapshotV11Error::LimitExceeded(error)
        }
        RepositorySnapshotV9Error::ContractInvalid => RepositorySnapshotV11Error::ContractInvalid,
        RepositorySnapshotV9Error::OutputLengthOverflow => {
            RepositorySnapshotV11Error::OutputLengthOverflow
        }
    }
}

fn stored_snapshot_error(head: &LocalSnapshotHead, reason: &'static str) -> StorageError {
    StorageError::CorruptMetadata {
        component: StorageComponent::Head,
        reason,
        snapshot_id: Some(head.snapshot_id.to_string()),
    }
}

fn bounded(value: &str, maximum: usize) -> String {
    value.chars().take(maximum).collect()
}

fn bounded_nonempty(value: &str, maximum: usize, fallback: &str) -> String {
    let value = bounded(value, maximum);
    if value.is_empty() {
        fallback.to_owned()
    } else {
        value
    }
}

fn bounded_repository_path(value: &str) -> String {
    if safe_relative_path(value) {
        bounded(
            value,
            usize::try_from(MAX_K1_EXPRESSION_METADATA_BYTES).unwrap_or(4096),
        )
    } else {
        "repository-path".to_owned()
    }
}
