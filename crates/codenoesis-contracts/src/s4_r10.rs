use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::path::{Component, Path};

use codenoesis_domain::s1_boundaries::RepositoryBoundaryReport;
use codenoesis_domain::s4_r3::R3_WORKSPACE_PROFILE;
use codenoesis_domain::s4_r4::R4_MANIFEST_PROFILE;
use codenoesis_domain::s4_r5::{R5_RUST_SEMANTIC_EXTRACTOR_VERSION, RustSemanticError};
pub use codenoesis_domain::s4_r10::{
    HAS_DECLARATION_ALTERNATIVE, MAX_R10_ALTERNATIVES_PER_METHOD,
    MAX_R10_ALTERNATIVES_PER_SNAPSHOT, MAX_R10_ALTERNATIVES_PER_SOURCE, R10_CONFIGURATION_VERSION,
    R10_ERROR_VERSION, R10_EXTRACTION_CHUNK_VERSION, R10_EXTRACTION_CONTRACT_VERSION,
    R10_EXTRACTOR_VERSION, R10_GRAPH_VERSION, R10_INDEX_VERSION, R10_LOCAL_EXPLORER_VERSION,
    R10_ONTOLOGY_VERSION, R10_PIPELINE_VERSION, R10_PORTABLE_GRAPH_VERSION, R10_PROFILE,
    R10_QUERY_VERSION, R10_SNAPSHOT_VERSION,
};
use codenoesis_domain::s4_r10::{
    RustCfgDeclarationAlternativesError, RustCfgDeclarationAlternativesKnowledge,
    RustCfgDeclarationAlternativesSourceChunk, RustDeclarationAlternative,
    RustDeclarationAlternativeRelationship, declaration_alternative_id,
    declaration_alternative_relationship_id,
};
use codenoesis_domain::storage::{
    LocalSnapshotHead, PublicationCandidate, SNAPSHOT_SCHEMA_VERSION_V12, SnapshotId,
    StorageComponent, StorageError,
};
use codenoesis_domain::{
    AcquisitionError, LimitKind, RepositoryIdentity, RepositoryInventory, STANDARD_LOCAL_S1_LIMITS,
    limit_exceeded,
};
use serde_json::{Map, Value, json};

use super::s4::{MAX_QUERY_BYTES, QueryContractError, claim_value};
use super::s4_r5::{RepositorySnapshotV8, RepositorySnapshotV8Error, local_query_result_v3};
use super::{
    LimitedVecWriter, PublicationCandidateError, SnapshotEnvelopeV1, publication_candidate,
    semantic_hash,
};

const CONFIGURATION_V9_HASH_DOMAIN: &[u8] = b"codenoesis.configuration.semantic.v9";
const SNAPSHOT_V12_HASH_DOMAIN: &[u8] = b"codenoesis.repository-snapshot.semantic.v12";
const EXTRACTION_V9_HASH_DOMAIN: &[u8] = b"codenoesis.extraction-chunk.semantic.v9";
const GRAPH_V9_HASH_DOMAIN: &[u8] = b"codenoesis.knowledge-graph.semantic.v9";
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

pub type R10Sha256 = fn(&[u8]) -> String;
pub const R10_PORTABLE_MARKER: &str = ".codenoesis-portable-graph-v3";
pub const R10_EXPLORER_MARKER: &str = ".codenoesis-local-explorer-v3";
pub const R10_EXPLORER_SECURITY_PROFILE: &str = "codenoesis.local-explorer-security/v3";
pub const MAX_R10_PORTABLE_GRAPH_BYTES: u64 = 268_435_456;
pub const MAX_R10_JSON_NESTING: u64 = 64;
pub const MAX_R10_TEXT_SEARCH_RESULTS: u64 = 100;
pub const R10_TRAVERSAL_DEPTH_DEFAULT: u64 = 1;
pub const MAX_R10_TRAVERSAL_DEPTH: u64 = 2;

#[derive(Clone, Debug)]
pub struct CodeNoesisErrorV17 {
    value: Value,
}

impl CodeNoesisErrorV17 {
    #[must_use]
    pub fn invalid_profile(profile: &str) -> Self {
        Self::new(
            "input.invalid_rust_cfg_alternatives_profile",
            "input",
            "invalid rust cfg declaration alternatives profile",
            &json!({"provided_profile": bounded(profile, 256)}),
        )
    }

    #[must_use]
    pub fn unsupported_composition(reason: &str) -> Self {
        Self::new(
            "input.unsupported_rust_cfg_alternatives_composition",
            "input",
            "unsupported rust cfg declaration alternatives composition",
            &json!({
                "profile": R10_PROFILE,
                "required_lineage": "r5_source_only",
                "reason": bounded(reason, 128)
            }),
        )
    }

    #[must_use]
    pub fn from_extraction(error: &RustCfgDeclarationAlternativesError) -> Self {
        match error {
            RustCfgDeclarationAlternativesError::IdentityMismatch {
                logical_method_id,
                reason,
            } => Self::new(
                "extraction.rust_cfg_alternative_identity_mismatch",
                "extraction",
                "rust cfg declaration alternative identity mismatch",
                &json!({"logical_method_id": logical_method_id, "reason": reason}),
            ),
            RustCfgDeclarationAlternativesError::Duplicate {
                logical_method_id,
                declaration_evidence_id,
            } => Self::new(
                "extraction.rust_cfg_alternative_duplicate",
                "extraction",
                "duplicate rust cfg declaration alternative",
                &json!({
                    "logical_method_id": logical_method_id,
                    "declaration_evidence_id": declaration_evidence_id
                }),
            ),
            RustCfgDeclarationAlternativesError::Overlap {
                logical_method_id,
                first_evidence_id,
                second_evidence_id,
            } => Self::new(
                "extraction.rust_cfg_alternative_overlap",
                "extraction",
                "overlapping rust cfg declaration alternatives",
                &json!({
                    "logical_method_id": logical_method_id,
                    "first_evidence_id": first_evidence_id,
                    "second_evidence_id": second_evidence_id
                }),
            ),
            RustCfgDeclarationAlternativesError::CrossSource { logical_method_id } => Self::new(
                "extraction.rust_cfg_alternative_cross_source",
                "extraction",
                "cross-source rust cfg declaration alternatives",
                &json!({"logical_method_id": logical_method_id}),
            ),
            RustCfgDeclarationAlternativesError::LimitExceeded {
                limit,
                maximum,
                observed,
            } => Self::new(
                "extraction.rust_cfg_alternative_limit_exceeded",
                "extraction",
                "rust cfg declaration alternative limit exceeded",
                &json!({"limit": limit.as_str(), "maximum": maximum, "observed": observed}),
            ),
            RustCfgDeclarationAlternativesError::Source(
                RustSemanticError::InvalidDeclaration {
                    path,
                    start_byte,
                    declaration_kind,
                },
            ) => Self::new(
                "extraction.invalid_rust_source",
                "extraction",
                "invalid rust source",
                &json!({
                    "path": bounded(path, 1024),
                    "start_byte": start_byte,
                    "declaration_kind": bounded(declaration_kind, 128)
                }),
            ),
            RustCfgDeclarationAlternativesError::Source(RustSemanticError::LimitExceeded {
                limit,
                maximum,
                observed,
            }) => Self::new(
                "extraction.rust_cfg_alternative_limit_exceeded",
                "extraction",
                "rust cfg declaration alternative limit exceeded",
                &json!({"limit": limit.as_str(), "maximum": maximum, "observed": observed}),
            ),
            RustCfgDeclarationAlternativesError::Source(_)
            | RustCfgDeclarationAlternativesError::ContractInvalid => Self::internal(),
        }
    }

    #[must_use]
    pub fn invalid_snapshot() -> Self {
        Self::new(
            "snapshot.invalid_v12",
            "snapshot",
            "invalid R10 snapshot",
            &json!({}),
        )
    }

    #[must_use]
    pub fn invalid_query() -> Self {
        Self::new(
            "query.invalid_v12",
            "query",
            "invalid R10 query",
            &json!({}),
        )
    }

    #[must_use]
    pub fn unsafe_output_path(path_sha256: &str, reason: &str) -> Self {
        Self::new(
            "input.unsafe_output_path",
            "input",
            "unsafe output path",
            &json!({"path_sha256": bounded(path_sha256, 64), "reason": bounded(reason, 64)}),
        )
    }

    #[must_use]
    pub fn from_contract(error: &R10ContractError, explorer: bool) -> Self {
        match error {
            R10ContractError::UnsupportedSnapshotSchema(observed) => Self::new(
                "export.invalid_snapshot",
                "export",
                "invalid R10 source snapshot",
                &json!({"observed": bounded(observed, 256)}),
            ),
            R10ContractError::LimitExceeded {
                limit,
                maximum,
                observed,
            } => Self::new(
                if explorer {
                    "explorer.limit_exceeded"
                } else {
                    "export.limit_exceeded"
                },
                if explorer { "explorer" } else { "export" },
                if explorer {
                    "local explorer input limit exceeded"
                } else {
                    "portable graph limit exceeded"
                },
                &json!({"limit": limit, "maximum": maximum, "observed": observed}),
            ),
            R10ContractError::AssetIntegrityMismatch => Self::new(
                "explorer.asset_integrity_mismatch",
                "explorer",
                "local explorer asset integrity mismatch",
                &json!({}),
            ),
            R10ContractError::Internal => Self::internal(),
            R10ContractError::InvalidSnapshot => Self::new(
                "export.invalid_snapshot",
                "export",
                "invalid R10 source snapshot",
                &json!({}),
            ),
            R10ContractError::UnsupportedPortableGraphSchema(observed) => {
                Self::invalid_portable_graph(&json!({"observed": bounded(observed, 256)}))
            }
            R10ContractError::Noncanonical {
                expected_sha256,
                observed_sha256,
            } => Self::invalid_portable_graph(&json!({
                "expected_sha256": expected_sha256,
                "observed_sha256": observed_sha256
            })),
            R10ContractError::IdentityConflict { family, id }
            | R10ContractError::ReferenceMismatch { family, id } => {
                Self::invalid_portable_graph(&json!({"family": family, "id": bounded(id, 512)}))
            }
            R10ContractError::UnsafePayload { reason } => {
                Self::invalid_portable_graph(&json!({"reason": reason}))
            }
            R10ContractError::InvalidProjection => Self::invalid_portable_graph(&json!({})),
        }
    }

    #[must_use]
    pub fn internal() -> Self {
        Self::new(
            "internal.unexpected",
            "internal",
            "unexpected internal error",
            &json!({}),
        )
    }

    fn new(code: &str, stage: &str, message: &str, context: &Value) -> Self {
        Self {
            value: json!({
                "schema_version": R10_ERROR_VERSION,
                "code": code,
                "stage": stage,
                "message": message,
                "retryable": false,
                "context": context
            }),
        }
    }

    fn invalid_portable_graph(context: &Value) -> Self {
        Self::new(
            "export.invalid_portable_graph_v3",
            "export",
            "invalid portable graph v3",
            context,
        )
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Serializes one bounded `ErrorV17` followed by LF.
    ///
    /// # Errors
    ///
    /// Returns an error only if the internal JSON cannot be serialized.
    pub fn canonical_stderr(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = serde_json::to_vec(&self.value)?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

#[derive(Clone, Debug)]
pub struct LocalQueryResultV7 {
    value: Value,
}

impl LocalQueryResultV7 {
    /// Serializes one bounded exact-ID V7 result followed by LF.
    ///
    /// # Errors
    ///
    /// Returns a strict query result or output-limit failure.
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

/// Builds one exact-ID V7 result and its directly linked R10 records.
///
/// # Errors
///
/// Returns a strict snapshot, document, not-found, or result-limit failure.
pub fn local_query_result_v7(
    semantic: &Value,
    manifest: &Value,
    snapshot_id: &str,
    requested_id: &str,
) -> Result<LocalQueryResultV7, QueryContractError> {
    let mut value =
        local_query_result_v3(semantic, manifest, snapshot_id, requested_id)?.into_value();
    let graph = semantic
        .get("knowledge_graph")
        .ok_or(QueryContractError::InvalidSnapshot)?;
    if graph.get("schema_version").and_then(Value::as_str) != Some(R10_GRAPH_VERSION) {
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
            relationship.get("kind").and_then(Value::as_str) == Some(HAS_DECLARATION_ALTERNATIVE)
                && (relationship.get("source").and_then(Value::as_str) == Some(requested_id)
                    || relationship.get("target").and_then(Value::as_str) == Some(requested_id)
                    || relationship.get("id").and_then(Value::as_str) == Some(requested_id))
        })
        .cloned()
        .collect::<Vec<_>>();
    linked_relationships.sort_by(|left, right| record_id(left).cmp(&record_id(right)));
    let linked_ids = linked_relationships
        .iter()
        .flat_map(|relationship| {
            [
                relationship.get("source").and_then(Value::as_str),
                relationship.get("target").and_then(Value::as_str),
            ]
        })
        .flatten()
        .filter(|identifier| *identifier != requested_id)
        .collect::<BTreeSet<_>>();
    let mut linked_entities = linked_ids
        .into_iter()
        .filter_map(|identifier| entities.get(identifier).copied())
        .filter(|entity| {
            entity.get("kind").and_then(Value::as_str) == Some("rust.declaration_alternative")
                || entity.get("id").and_then(Value::as_str) == Some(requested_id)
                || linked_relationships.iter().any(|relationship| {
                    relationship.get("source").and_then(Value::as_str)
                        == entity.get("id").and_then(Value::as_str)
                })
        })
        .cloned()
        .collect::<Vec<_>>();
    linked_entities.sort_by(|left, right| record_id(left).cmp(&record_id(right)));
    let mut linked_subject_ids = BTreeSet::from([requested_id]);
    linked_subject_ids.extend(linked_entities.iter().filter_map(record_id));
    linked_subject_ids.extend(linked_relationships.iter().filter_map(record_id));
    let linked_claims = graph
        .get("claims")
        .and_then(Value::as_array)
        .ok_or(QueryContractError::InvalidSnapshot)?
        .iter()
        .filter(|claim| {
            claim
                .get("subject_id")
                .and_then(Value::as_str)
                .is_some_and(|identifier| linked_subject_ids.contains(identifier))
        })
        .cloned()
        .collect::<Vec<_>>();
    union_query_records(&mut value, "claims", &linked_claims)?;
    let linked_evidence_ids = value["claims"]
        .as_array()
        .ok_or(QueryContractError::InvalidSnapshot)?
        .iter()
        .filter_map(|claim| claim.get("evidence_ids").and_then(Value::as_array))
        .flatten()
        .map(|identifier| {
            identifier
                .as_str()
                .ok_or(QueryContractError::InvalidSnapshot)
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let evidence = id_map(graph, "evidence")?;
    let linked_evidence = linked_evidence_ids
        .into_iter()
        .map(|identifier| {
            evidence
                .get(identifier)
                .copied()
                .cloned()
                .ok_or(QueryContractError::InvalidSnapshot)
        })
        .collect::<Result<Vec<_>, _>>()?;
    union_query_records(&mut value, "evidence", &linked_evidence)?;
    value["schema_version"] = Value::String(R10_QUERY_VERSION.to_owned());
    value["linked_r10_entities"] = Value::Array(linked_entities);
    value["linked_r10_relationships"] = Value::Array(linked_relationships);
    let result = LocalQueryResultV7 { value };
    result.canonical_stdout()?;
    Ok(result)
}

fn union_query_records(
    value: &mut Value,
    field: &'static str,
    additions: &[Value],
) -> Result<(), QueryContractError> {
    let records = value
        .get_mut(field)
        .and_then(Value::as_array_mut)
        .ok_or(QueryContractError::InvalidSnapshot)?;
    let mut union = BTreeMap::new();
    for record in records.iter().chain(additions.iter()) {
        let identifier = record_id(record).ok_or(QueryContractError::InvalidSnapshot)?;
        if let Some(existing) = union.insert(identifier.to_owned(), record.clone())
            && existing != *record
        {
            return Err(QueryContractError::InvalidSnapshot);
        }
    }
    *records = union.into_values().collect();
    Ok(())
}

#[derive(Clone, Debug)]
pub struct RepositorySnapshotV12 {
    value: Value,
}

#[derive(Debug)]
pub enum RepositorySnapshotV12Error {
    Serialization(serde_json::Error),
    LimitExceeded(AcquisitionError),
    ContractInvalid,
    OutputLengthOverflow,
}

impl Display for RepositorySnapshotV12Error {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Serialization(_) => "R10 snapshot serialization failed",
            Self::LimitExceeded(_) => "R10 snapshot output limit exceeded",
            Self::ContractInvalid => "R10 snapshot contract is invalid",
            Self::OutputLengthOverflow => "R10 snapshot output length overflowed",
        })
    }
}

impl Error for RepositorySnapshotV12Error {}

impl RepositorySnapshotV12 {
    /// Builds selector-bound V12 over the immutable R5 lineage and R10 additions.
    ///
    /// # Errors
    ///
    /// Returns a validation, serialization, publication, or output-bound failure.
    pub fn from_inventory_and_cfg_declaration_alternatives(
        inventory: &RepositoryInventory,
        knowledge: &RustCfgDeclarationAlternativesKnowledge,
        boundaries: Option<&RepositoryBoundaryReport>,
        envelope: SnapshotEnvelopeV1,
    ) -> Result<Self, RepositorySnapshotV12Error> {
        knowledge
            .validate()
            .map_err(|_| RepositorySnapshotV12Error::ContractInvalid)?;
        let baseline = RepositorySnapshotV8::from_inventory_and_rust_semantics(
            inventory,
            &knowledge.semantic,
            boundaries,
            envelope,
        )
        .map_err(map_v8_error)?;
        let mut value = baseline.value().clone();
        let semantic = value
            .get_mut("semantic")
            .and_then(Value::as_object_mut)
            .ok_or(RepositorySnapshotV12Error::ContractInvalid)?;
        let configuration_without_hash = json!({
            "schema_version": R10_CONFIGURATION_VERSION,
            "profile": "standard-local-s4",
            "workspace_profile": R3_WORKSPACE_PROFILE,
            "manifest_profile": R4_MANIFEST_PROFILE,
            "rust_semantic_profile": R10_PROFILE,
            "repository_boundary_profile": boundaries.map(|_| "local-gitlinks-v1")
        });
        let configuration_hash =
            semantic_hash(CONFIGURATION_V9_HASH_DOMAIN, &configuration_without_hash);
        let mut configuration = configuration_without_hash;
        configuration["semantic_hash"] =
            json!({"algorithm": "blake3-256", "value": configuration_hash});
        semantic.insert("configuration".to_owned(), configuration);
        semantic.insert(
            "pipeline_version".to_owned(),
            Value::String(R10_PIPELINE_VERSION.to_owned()),
        );
        semantic.insert(
            "ontology_version".to_owned(),
            Value::String(R10_ONTOLOGY_VERSION.to_owned()),
        );
        semantic.insert(
            "extractor_contract_version".to_owned(),
            Value::String(R10_EXTRACTION_CONTRACT_VERSION.to_owned()),
        );
        let mut extractor_versions = vec![
            Value::String("codenoesis.inventory-classifier/s1-v1".to_owned()),
            Value::String("codenoesis.rust-tree-sitter/s4-v1".to_owned()),
            Value::String("codenoesis.rust-workspace/s4-r3-v1".to_owned()),
            Value::String("codenoesis.cargo-manifest/s4-r4-v1".to_owned()),
            Value::String(R5_RUST_SEMANTIC_EXTRACTOR_VERSION.to_owned()),
            Value::String(R10_EXTRACTOR_VERSION.to_owned()),
        ];
        if boundaries.is_some() {
            extractor_versions.push(Value::String("codenoesis.git-boundary/s1-v1".to_owned()));
        }
        semantic.insert(
            "extractor_versions".to_owned(),
            Value::Array(extractor_versions),
        );
        let baseline_chunks = semantic
            .get("extraction_chunks")
            .cloned()
            .ok_or(RepositorySnapshotV12Error::ContractInvalid)?;
        semantic.insert(
            "extraction_chunks".to_owned(),
            Value::Array(extraction_chunks_v9(&baseline_chunks, knowledge)?),
        );
        let baseline_graph = semantic
            .get("knowledge_graph")
            .cloned()
            .ok_or(RepositorySnapshotV12Error::ContractInvalid)?;
        let repository = semantic
            .get("repository")
            .cloned()
            .ok_or(RepositorySnapshotV12Error::ContractInvalid)?;
        semantic.insert(
            "knowledge_graph".to_owned(),
            knowledge_graph_v9(&baseline_graph, &repository, knowledge)?,
        );
        let semantic_value = Value::Object(semantic.clone());
        let snapshot_hash = semantic_hash(SNAPSHOT_V12_HASH_DOMAIN, &semantic_value);
        let root = value
            .as_object_mut()
            .ok_or(RepositorySnapshotV12Error::ContractInvalid)?;
        root.insert(
            "schema_version".to_owned(),
            Value::String(R10_SNAPSHOT_VERSION.to_owned()),
        );
        root.insert(
            "semantic_hash".to_owned(),
            json!({"algorithm": "blake3-256", "value": snapshot_hash}),
        );
        publication_candidate(&value).map_err(|_| RepositorySnapshotV12Error::ContractInvalid)?;
        Ok(Self { value })
    }

    /// Serializes the complete V12 snapshot with the inherited standard bound.
    ///
    /// # Errors
    ///
    /// Returns a serialization or output-limit failure.
    pub fn canonical_stdout(&self) -> Result<Vec<u8>, RepositorySnapshotV12Error> {
        let maximum = usize::try_from(STANDARD_LOCAL_S1_LIMITS.canonical_output_bytes)
            .map_err(|_| RepositorySnapshotV12Error::OutputLengthOverflow)?;
        let body_maximum = maximum
            .checked_sub(1)
            .ok_or(RepositorySnapshotV12Error::OutputLengthOverflow)?;
        let mut writer = LimitedVecWriter::new(body_maximum);
        let result = serde_json::to_writer(&mut writer, &self.value);
        if writer.overflowed() {
            return Err(RepositorySnapshotV12Error::LimitExceeded(limit_exceeded(
                LimitKind::CanonicalOutputBytes,
                STANDARD_LOCAL_S1_LIMITS
                    .canonical_output_bytes
                    .saturating_add(1),
            )));
        }
        result.map_err(RepositorySnapshotV12Error::Serialization)?;
        let mut bytes = writer.into_inner();
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Serializes the exact V12 semantic payload.
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

    /// Converts V12 into the unchanged immutable publication model.
    ///
    /// # Errors
    ///
    /// Returns a strict contract or storage-integrity failure.
    pub fn publication_candidate(&self) -> Result<PublicationCandidate, PublicationCandidateError> {
        publication_candidate(&self.value)
    }
}

/// Validates one loaded V12 semantic payload against the visible head.
///
/// # Errors
///
/// Returns a typed storage-integrity failure on any mismatch.
pub fn validate_stored_snapshot_semantic_v12(
    semantic: &Value,
    head: &LocalSnapshotHead,
) -> Result<(), StorageError> {
    if head.snapshot_schema_version != SNAPSHOT_SCHEMA_VERSION_V12 {
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
pub struct PortableGraphV3 {
    value: Value,
    canonical: Vec<u8>,
    sha256: R10Sha256,
}

impl PortableGraphV3 {
    /// Projects one validated V12 head and deterministic documentation manifest.
    ///
    /// # Errors
    ///
    /// Returns a strict binding, privacy, reference, or limit failure.
    pub fn from_validated_v12(
        semantic: &Value,
        head: &LocalSnapshotHead,
        documentation_manifest: &Value,
        sha256: R10Sha256,
    ) -> Result<Self, R10ContractError> {
        validate_stored_snapshot_semantic_v12(semantic, head)
            .map_err(|_| R10ContractError::InvalidSnapshot)?;
        if semantic.get("ontology_version").and_then(Value::as_str) != Some(R10_ONTOLOGY_VERSION) {
            return Err(R10ContractError::InvalidSnapshot);
        }
        validate_documentation_binding(documentation_manifest, head)?;
        let graph = semantic
            .get("knowledge_graph")
            .and_then(Value::as_object)
            .ok_or(R10ContractError::InvalidSnapshot)?;
        let (documents, document_statements) = portable_documents(documentation_manifest)?;
        let mut value = json!({
            "schema_version": R10_PORTABLE_GRAPH_VERSION,
            "repository": semantic.get("repository").cloned().ok_or(R10ContractError::InvalidSnapshot)?,
            "source_snapshot": {
                "schema_version": R10_SNAPSHOT_VERSION,
                "snapshot_id": head.snapshot_id.as_str(),
                "semantic_hash": {
                    "algorithm": head.semantic_hash.algorithm,
                    "value": head.semantic_hash.value
                }
            },
            "ontology_version": R10_ONTOLOGY_VERSION,
            "query_contract_version": R10_QUERY_VERSION,
            "projection": {
                "profile": "codenoesis.lossless-portable-projection/v3",
                "family_sha256": {}
            },
            "entities": graph.get("entities").cloned().ok_or(R10ContractError::InvalidSnapshot)?,
            "relationships": graph.get("relationships").cloned().ok_or(R10ContractError::InvalidSnapshot)?,
            "claims": graph.get("claims").cloned().ok_or(R10ContractError::InvalidSnapshot)?,
            "evidence": graph.get("evidence").cloned().ok_or(R10ContractError::InvalidSnapshot)?,
            "diagnostics": graph.get("diagnostics").cloned().ok_or(R10ContractError::InvalidSnapshot)?,
            "coverage_gaps": graph.get("coverage").cloned().ok_or(R10ContractError::InvalidSnapshot)?,
            "documents": documents,
            "document_statements": document_statements
        });
        value["projection"]["family_sha256"] = family_digests(&value, sha256)?;
        Self::from_generated_value(value, sha256)
    }

    /// Strictly reimports one canonical LF-terminated `PortableGraphV3`.
    ///
    /// # Errors
    ///
    /// Returns the first decode, schema, identity, reference, privacy, or limit failure.
    pub fn from_canonical_file(bytes: &[u8], sha256: R10Sha256) -> Result<Self, R10ContractError> {
        enforce_portable_size(bytes.len())?;
        let value = serde_json::from_slice::<Value>(bytes)
            .map_err(|_| R10ContractError::InvalidProjection)?;
        validate_portable_value(&value, sha256)?;
        let canonical = serde_json::to_vec(&value).map_err(|_| R10ContractError::Internal)?;
        let mut expected = canonical.clone();
        expected.push(b'\n');
        if expected != bytes {
            return Err(R10ContractError::Noncanonical {
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

    fn from_generated_value(value: Value, sha256: R10Sha256) -> Result<Self, R10ContractError> {
        validate_portable_value(&value, sha256)?;
        let canonical = serde_json::to_vec(&value).map_err(|_| R10ContractError::Internal)?;
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
pub struct LocalExplorerManifestV3 {
    value: Value,
}

impl LocalExplorerManifestV3 {
    /// Builds the offline V3 explorer manifest bound to exact graph and viewer bytes.
    ///
    /// # Errors
    ///
    /// Returns an integrity or unsafe-CSP failure.
    pub fn new(
        portable: &PortableGraphV3,
        viewer_bytes: &[u8],
        expected_viewer_sha256: &str,
        content_security_policy: &str,
        sha256: R10Sha256,
    ) -> Result<Self, R10ContractError> {
        if sha256(viewer_bytes) != expected_viewer_sha256
            || content_security_policy.contains("http:")
            || content_security_policy.contains("https:")
            || content_security_policy.contains("unsafe-inline")
            || content_security_policy.contains("unsafe-eval")
        {
            return Err(R10ContractError::AssetIntegrityMismatch);
        }
        Ok(Self {
            value: json!({
                "schema_version": R10_LOCAL_EXPLORER_VERSION,
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
                    "profile": R10_EXPLORER_SECURITY_PROFILE,
                    "content_security_policy": content_security_policy,
                    "network": false,
                    "dynamic_code": false,
                    "storage": false,
                    "telemetry": false,
                    "browser_launch": false
                },
                "capabilities": {
                    "exact_id_search": true,
                    "text_search": true,
                    "typed_filters": true,
                    "evidence_inspection": true,
                    "bounded_traversal": [1, 2]
                },
                "limits": {
                    "text_search_results": MAX_R10_TEXT_SEARCH_RESULTS,
                    "traversal_depth_default": R10_TRAVERSAL_DEPTH_DEFAULT,
                    "traversal_depth_maximum": MAX_R10_TRAVERSAL_DEPTH
                }
            }),
        })
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Serializes one `LocalExplorerManifestV3` followed by LF.
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
pub enum R10ContractError {
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

impl Display for R10ContractError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidSnapshot => "invalid R10 snapshot",
            Self::UnsupportedSnapshotSchema(_) => "unsupported R10 snapshot schema",
            Self::UnsupportedPortableGraphSchema(_) => "unsupported portable graph schema",
            Self::Noncanonical { .. } => "noncanonical portable graph",
            Self::IdentityConflict { .. } => "portable identity conflict",
            Self::ReferenceMismatch { .. } => "portable reference mismatch",
            Self::LimitExceeded { .. } => "portable graph limit exceeded",
            Self::UnsafePayload { .. } => "unsafe portable payload",
            Self::AssetIntegrityMismatch => "explorer asset integrity mismatch",
            Self::InvalidProjection => "invalid portable graph projection",
            Self::Internal => "internal R10 contract error",
        })
    }
}

impl Error for R10ContractError {}

fn extraction_chunks_v9(
    baseline: &Value,
    knowledge: &RustCfgDeclarationAlternativesKnowledge,
) -> Result<Vec<Value>, RepositorySnapshotV12Error> {
    let additions = knowledge
        .extraction_chunks
        .iter()
        .map(|chunk| (chunk.source_file_id.as_str(), chunk))
        .collect::<BTreeMap<_, _>>();
    let mut transformed = Vec::with_capacity(additions.len());
    for chunk in baseline
        .as_array()
        .ok_or(RepositorySnapshotV12Error::ContractInvalid)?
    {
        if chunk.pointer("/subject/kind").and_then(Value::as_str) != Some("rust_source") {
            continue;
        }
        let source_file_id = chunk
            .pointer("/subject/source_file_id")
            .and_then(Value::as_str)
            .ok_or(RepositorySnapshotV12Error::ContractInvalid)?;
        let addition = additions
            .get(source_file_id)
            .ok_or(RepositorySnapshotV12Error::ContractInvalid)?;
        let mut value = json!({
            "schema_version": R10_EXTRACTION_CHUNK_VERSION,
            "source_file_id": source_file_id,
            "entities": chunk.get("entities").cloned().ok_or(RepositorySnapshotV12Error::ContractInvalid)?,
            "relationships": chunk.get("relationships").cloned().ok_or(RepositorySnapshotV12Error::ContractInvalid)?,
            "claims": chunk.get("claims").cloned().ok_or(RepositorySnapshotV12Error::ContractInvalid)?,
            "evidence": chunk.get("evidence").cloned().ok_or(RepositorySnapshotV12Error::ContractInvalid)?,
            "diagnostics": chunk.get("diagnostics").cloned().ok_or(RepositorySnapshotV12Error::ContractInvalid)?,
            "coverage": chunk.get("coverage").cloned().ok_or(RepositorySnapshotV12Error::ContractInvalid)?,
            "declaration_alternatives_profile": R10_PROFILE
        });
        apply_r10_additions(&mut value, addition)?;
        canonicalize_evidence_references(&mut value)?;
        insert_semantic_hash(&mut value, EXTRACTION_V9_HASH_DOMAIN)?;
        transformed.push(value);
    }
    transformed.sort_by(|left, right| {
        left.get("source_file_id")
            .and_then(Value::as_str)
            .cmp(&right.get("source_file_id").and_then(Value::as_str))
    });
    if transformed.len() != additions.len() {
        return Err(RepositorySnapshotV12Error::ContractInvalid);
    }
    Ok(transformed)
}

fn knowledge_graph_v9(
    baseline: &Value,
    repository: &Value,
    knowledge: &RustCfgDeclarationAlternativesKnowledge,
) -> Result<Value, RepositorySnapshotV12Error> {
    let mut value = json!({
        "schema_version": R10_GRAPH_VERSION,
        "ontology_version": R10_ONTOLOGY_VERSION,
        "extractor_versions": [
            "codenoesis.rust-tree-sitter/s4-v1",
            "codenoesis.rust-workspace/s4-r3-v1",
            "codenoesis.cargo-manifest/s4-r4-v1",
            R5_RUST_SEMANTIC_EXTRACTOR_VERSION,
            R10_EXTRACTOR_VERSION
        ],
        "repository_identity": repository.get("identity").cloned().ok_or(RepositorySnapshotV12Error::ContractInvalid)?,
        "commit_oid": repository.get("commit_oid").cloned().ok_or(RepositorySnapshotV12Error::ContractInvalid)?,
        "entities": baseline.get("entities").cloned().ok_or(RepositorySnapshotV12Error::ContractInvalid)?,
        "relationships": baseline.get("relationships").cloned().ok_or(RepositorySnapshotV12Error::ContractInvalid)?,
        "claims": baseline.get("claims").cloned().ok_or(RepositorySnapshotV12Error::ContractInvalid)?,
        "evidence": baseline.get("evidence").cloned().ok_or(RepositorySnapshotV12Error::ContractInvalid)?,
        "diagnostics": baseline.get("diagnostics").cloned().ok_or(RepositorySnapshotV12Error::ContractInvalid)?,
        "coverage": baseline.get("coverage").cloned().ok_or(RepositorySnapshotV12Error::ContractInvalid)?,
        "declaration_alternative_index": {
            "schema_version": R10_INDEX_VERSION,
            "profile": R10_PROFILE,
            "logical_method_ids": knowledge.graph.index.logical_method_ids,
            "alternative_entity_ids": knowledge.graph.index.alternative_entity_ids,
            "alternative_relationship_ids": knowledge.graph.index.alternative_relationship_ids
        }
    });
    let aggregate = RustCfgDeclarationAlternativesSourceChunk {
        source_file_id: String::new(),
        logical_method_ids: knowledge.graph.index.logical_method_ids.clone(),
        alternatives: knowledge.graph.alternatives.clone(),
        relationships: knowledge.graph.relationships.clone(),
        claims: knowledge.graph.claims.clone(),
    };
    apply_r10_additions(&mut value, &aggregate)?;
    canonicalize_evidence_references(&mut value)?;
    insert_semantic_hash(&mut value, GRAPH_V9_HASH_DOMAIN)?;
    Ok(value)
}

fn apply_r10_additions(
    value: &mut Value,
    additions: &RustCfgDeclarationAlternativesSourceChunk,
) -> Result<(), RepositorySnapshotV12Error> {
    let object = value
        .as_object_mut()
        .ok_or(RepositorySnapshotV12Error::ContractInvalid)?;
    for logical_method_id in &additions.logical_method_ids {
        let alternatives = additions
            .alternatives
            .iter()
            .filter(|alternative| &alternative.subject_id == logical_method_id)
            .collect::<Vec<_>>();
        project_logical_method(object, logical_method_id, &alternatives)?;
    }
    merge_id_array(
        object,
        "entities",
        additions.alternatives.iter().map(alternative_value),
    )?;
    merge_id_array(
        object,
        "relationships",
        additions
            .relationships
            .iter()
            .map(alternative_relationship_value),
    )?;
    merge_id_array(object, "claims", additions.claims.iter().map(claim_value))?;
    Ok(())
}

fn project_logical_method(
    object: &mut Map<String, Value>,
    logical_method_id: &str,
    alternatives: &[&RustDeclarationAlternative],
) -> Result<(), RepositorySnapshotV12Error> {
    if alternatives.len() < 2 {
        return Err(RepositorySnapshotV12Error::ContractInvalid);
    }
    let first = alternatives[0];
    let mut alternative_ids = alternatives
        .iter()
        .map(|alternative| alternative.id.clone())
        .collect::<Vec<_>>();
    alternative_ids.sort();
    let mut declaration_evidence_ids = alternatives
        .iter()
        .map(|alternative| alternative.properties.declaration_evidence_id.clone())
        .collect::<Vec<_>>();
    declaration_evidence_ids.sort();
    if !strictly_ordered(alternative_ids.iter().map(String::as_str))
        || !strictly_ordered(declaration_evidence_ids.iter().map(String::as_str))
    {
        return Err(RepositorySnapshotV12Error::ContractInvalid);
    }
    let entities = family_mut(object, "entities")?;
    let logical = find_record_mut(entities, logical_method_id)?;
    let owner_id = logical
        .get("owner_id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or(RepositorySnapshotV12Error::ContractInvalid)?
        .to_owned();
    *logical = json!({
        "id": logical_method_id,
        "kind": "rust.method",
        "crate_id": first.crate_id,
        "module_path": first.module_path,
        "name": first.name,
        "visibility": first.properties.visibility.as_str(),
        "owner_id": owner_id,
        "properties": {
            "implementation_context": first.properties.implementation_context.as_str(),
            "trait_context_id": first.properties.trait_context_id,
            "declaration_state": "alternatives",
            "declaration_alternative_ids": alternative_ids
        }
    });
    aggregate_subject_evidence(
        object,
        "entity",
        logical_method_id,
        &declaration_evidence_ids,
    )?;
    let relationships = family_mut(object, "relationships")?;
    let mut defines = relationships
        .iter_mut()
        .filter(|relationship| {
            relationship.get("kind").and_then(Value::as_str) == Some("DEFINES")
                && relationship.get("target").and_then(Value::as_str) == Some(logical_method_id)
        })
        .collect::<Vec<_>>();
    if defines.len() != 1 {
        return Err(RepositorySnapshotV12Error::ContractInvalid);
    }
    defines[0]["evidence_ids"] = json!(declaration_evidence_ids);
    let relationship_id = defines[0]
        .get("id")
        .and_then(Value::as_str)
        .ok_or(RepositorySnapshotV12Error::ContractInvalid)?
        .to_owned();
    aggregate_subject_evidence(
        object,
        "relationship",
        &relationship_id,
        &declaration_evidence_ids,
    )
}

fn aggregate_subject_evidence(
    object: &mut Map<String, Value>,
    subject_kind: &str,
    subject_id: &str,
    evidence_ids: &[String],
) -> Result<(), RepositorySnapshotV12Error> {
    let claims = family_mut(object, "claims")?;
    let mut matching = claims
        .iter_mut()
        .filter(|claim| {
            claim.get("subject_kind").and_then(Value::as_str) == Some(subject_kind)
                && claim.get("subject_id").and_then(Value::as_str) == Some(subject_id)
        })
        .collect::<Vec<_>>();
    if matching.len() != 1 {
        return Err(RepositorySnapshotV12Error::ContractInvalid);
    }
    matching[0]["evidence_ids"] = json!(evidence_ids);
    Ok(())
}

fn alternative_value(alternative: &RustDeclarationAlternative) -> Value {
    json!({
        "id": alternative.id,
        "kind": "rust.declaration_alternative",
        "crate_id": alternative.crate_id,
        "module_path": alternative.module_path,
        "name": alternative.name,
        "subject_id": alternative.subject_id,
        "source_file_id": alternative.source_file_id,
        "properties": {
            "declaration_kind": alternative.properties.declaration_kind,
            "implementation_context": alternative.properties.implementation_context.as_str(),
            "trait_context_id": alternative.properties.trait_context_id,
            "visibility": alternative.properties.visibility.as_str(),
            "receiver_present": alternative.properties.receiver_present,
            "declared_signature": alternative.properties.declared_signature,
            "compilation_presence": alternative.properties.compilation_presence.as_str(),
            "declaration_evidence_id": alternative.properties.declaration_evidence_id,
            "attributes": alternative.properties.attributes.iter().map(|attribute| json!({
                "kind": attribute.kind.as_str(),
                "token_text": attribute.token_text,
                "evidence_id": attribute.evidence_id
            })).collect::<Vec<_>>()
        }
    })
}

fn alternative_relationship_value(relationship: &RustDeclarationAlternativeRelationship) -> Value {
    json!({
        "id": relationship.id,
        "kind": HAS_DECLARATION_ALTERNATIVE,
        "source": relationship.source,
        "target": relationship.target,
        "evidence_ids": relationship.evidence_ids
    })
}

fn merge_id_array(
    object: &mut Map<String, Value>,
    field: &'static str,
    additions: impl IntoIterator<Item = Value>,
) -> Result<(), RepositorySnapshotV12Error> {
    let values = family_mut(object, field)?;
    values.extend(additions);
    values.sort_by(|left, right| record_id(left).cmp(&record_id(right)));
    if !strictly_ordered(values.iter().filter_map(record_id))
        || values.iter().any(|value| record_id(value).is_none())
    {
        return Err(RepositorySnapshotV12Error::ContractInvalid);
    }
    Ok(())
}

fn canonicalize_evidence_references(value: &mut Value) -> Result<(), RepositorySnapshotV12Error> {
    let object = value
        .as_object_mut()
        .ok_or(RepositorySnapshotV12Error::ContractInvalid)?;
    for family in [
        "entities",
        "relationships",
        "claims",
        "diagnostics",
        "coverage",
    ] {
        for record in family_mut(object, family)? {
            let Some(references) = record.get_mut("evidence_ids") else {
                continue;
            };
            let references = references
                .as_array_mut()
                .ok_or(RepositorySnapshotV12Error::ContractInvalid)?;
            if references
                .iter()
                .any(|reference| reference.as_str().is_none())
            {
                return Err(RepositorySnapshotV12Error::ContractInvalid);
            }
            references.sort_by(|left, right| left.as_str().cmp(&right.as_str()));
            if !strictly_ordered(references.iter().filter_map(Value::as_str)) {
                return Err(RepositorySnapshotV12Error::ContractInvalid);
            }
        }
    }
    Ok(())
}

fn family_mut<'a>(
    object: &'a mut Map<String, Value>,
    field: &'static str,
) -> Result<&'a mut Vec<Value>, RepositorySnapshotV12Error> {
    object
        .get_mut(field)
        .and_then(Value::as_array_mut)
        .ok_or(RepositorySnapshotV12Error::ContractInvalid)
}

fn find_record_mut<'a>(
    values: &'a mut [Value],
    identifier: &str,
) -> Result<&'a mut Value, RepositorySnapshotV12Error> {
    let mut matching = values
        .iter_mut()
        .filter(|value| record_id(value) == Some(identifier))
        .collect::<Vec<_>>();
    if matching.len() != 1 {
        return Err(RepositorySnapshotV12Error::ContractInvalid);
    }
    Ok(matching.remove(0))
}

fn insert_semantic_hash(
    value: &mut Value,
    domain: &[u8],
) -> Result<(), RepositorySnapshotV12Error> {
    let hash = semantic_hash(domain, value);
    value
        .as_object_mut()
        .ok_or(RepositorySnapshotV12Error::ContractInvalid)?
        .insert(
            "semantic_hash".to_owned(),
            json!({"algorithm": "blake3-256", "value": hash}),
        );
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn validate_portable_shapes(value: &Value) -> Result<(), R10ContractError> {
    let repository_fields = [
        "commit_oid",
        "contract_version",
        "identity",
        "identity_schema_version",
        "object_format",
        "tree_oid",
        "vcs",
    ];
    let repository = object_with_shape(
        value
            .get("repository")
            .ok_or(R10ContractError::InvalidProjection)?,
        &repository_fields,
        &repository_fields,
    )?;
    if repository.get("contract_version").and_then(Value::as_str)
        != Some("codenoesis.repository/v1")
        || repository
            .get("identity_schema_version")
            .and_then(Value::as_str)
            != Some("codenoesis.repository-identity/v1")
        || repository.get("object_format").and_then(Value::as_str) != Some("sha1")
        || repository.get("vcs").and_then(Value::as_str) != Some("git")
        || !repository
            .get("commit_oid")
            .and_then(Value::as_str)
            .is_some_and(|value| lower_hex_with_length(value, 40))
        || !repository
            .get("tree_oid")
            .and_then(Value::as_str)
            .is_some_and(|value| lower_hex_with_length(value, 40))
        || repository
            .get("identity")
            .and_then(Value::as_str)
            .is_none_or(|value| RepositoryIdentity::parse(value).is_err())
    {
        return Err(R10ContractError::InvalidProjection);
    }

    let source_snapshot_fields = ["schema_version", "semantic_hash", "snapshot_id"];
    let source_snapshot = object_with_shape(
        value
            .get("source_snapshot")
            .ok_or(R10ContractError::InvalidProjection)?,
        &source_snapshot_fields,
        &source_snapshot_fields,
    )?;
    validate_blake3_hash(
        source_snapshot
            .get("semantic_hash")
            .ok_or(R10ContractError::InvalidProjection)?,
    )?;
    let semantic_hash = source_snapshot
        .get("semantic_hash")
        .and_then(Value::as_object)
        .and_then(|hash| hash.get("value"))
        .and_then(Value::as_str)
        .ok_or(R10ContractError::InvalidProjection)?;
    let snapshot_id = source_snapshot
        .get("snapshot_id")
        .and_then(Value::as_str)
        .ok_or(R10ContractError::InvalidProjection)?;
    if SnapshotId::from_semantic_hash(semantic_hash)
        .map_err(|_| R10ContractError::InvalidProjection)?
        .as_str()
        != snapshot_id
    {
        return Err(R10ContractError::InvalidProjection);
    }

    let projection_fields = ["family_sha256", "profile"];
    let projection = object_with_shape(
        value
            .get("projection")
            .ok_or(R10ContractError::InvalidProjection)?,
        &projection_fields,
        &projection_fields,
    )?;
    if projection.get("profile").and_then(Value::as_str)
        != Some("codenoesis.lossless-portable-projection/v3")
    {
        return Err(R10ContractError::InvalidProjection);
    }
    let family_hashes = projection
        .get("family_sha256")
        .and_then(Value::as_object)
        .ok_or(R10ContractError::InvalidProjection)?;
    let expected_families = PORTABLE_FAMILIES
        .into_iter()
        .map(|(family, _)| family)
        .collect::<BTreeSet<_>>();
    if family_hashes
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>()
        != expected_families
        || family_hashes.values().any(|value| {
            !value
                .as_str()
                .is_some_and(|value| lower_hex_with_length(value, 64))
        })
    {
        return Err(R10ContractError::InvalidProjection);
    }

    let entity_required = [
        "crate_id",
        "id",
        "kind",
        "module_path",
        "name",
        "properties",
    ];
    let entity_allowed = [
        "compilation_presence",
        "crate_id",
        "id",
        "kind",
        "module_path",
        "name",
        "owner_id",
        "properties",
        "source_file_id",
        "subject_id",
        "trait_context_id",
        "visibility",
    ];
    let property_allowed = [
        "applied",
        "attributes",
        "bench",
        "blob_oid",
        "bounds_present",
        "committed_present",
        "compilation_presence",
        "crate_types",
        "declaration_alternative_ids",
        "declaration_evidence_id",
        "declaration_kind",
        "declaration_state",
        "declared",
        "declared_name",
        "declared_signature",
        "declared_type_or_header",
        "default_features",
        "default_present",
        "discriminant_present",
        "dependency_kind",
        "dependency_name",
        "doc",
        "doctest",
        "edition",
        "enabled",
        "evidence_id",
        "executed",
        "feature_name",
        "form",
        "git_locator",
        "git_reference",
        "harness",
        "implementation_context",
        "inherited_from",
        "initializer_present",
        "lexeme",
        "manifest_id",
        "manifest_path",
        "manifest_role",
        "materialized_crate_id",
        "member_path",
        "member_source",
        "members",
        "metadata",
        "mutable",
        "name_source",
        "normalized",
        "optional",
        "options",
        "owner_id",
        "owner_kind",
        "package_id",
        "package_name",
        "package_table_present",
        "path",
        "path_source",
        "proc_macro",
        "receiver_present",
        "redacted",
        "registries",
        "registry_name",
        "requested_features",
        "required_features",
        "root_shape",
        "scope",
        "selection",
        "sha256",
        "source",
        "source_analysis_state",
        "source_entity_id",
        "source_field",
        "source_file_id",
        "source_path",
        "source_selector",
        "syntax",
        "target",
        "target_kind",
        "target_name",
        "target_predicate",
        "test",
        "token_text",
        "trait_context_id",
        "tuple_index",
        "value",
        "values",
        "version_requirement",
        "visibility",
        "workspace",
        "workspace_member_source",
        "workspace_reference_id",
        "workspace_root_shape",
        "workspace_table_present",
    ];
    for entity in value["entities"]
        .as_array()
        .ok_or(R10ContractError::InvalidProjection)?
    {
        let entity = object_with_shape(entity, &entity_required, &entity_allowed)?;
        let properties = object_with_shape(
            entity
                .get("properties")
                .ok_or(R10ContractError::InvalidProjection)?,
            &[],
            &property_allowed,
        )?;
        if entity.get("kind").and_then(Value::as_str) == Some("rust.declaration_alternative") {
            let fields = [
                "attributes",
                "compilation_presence",
                "declaration_evidence_id",
                "declaration_kind",
                "declared_signature",
                "implementation_context",
                "receiver_present",
                "trait_context_id",
                "visibility",
            ];
            object_with_shape(
                entity
                    .get("properties")
                    .ok_or(R10ContractError::InvalidProjection)?,
                &fields,
                &fields,
            )?;
        } else if properties.get("declaration_state").and_then(Value::as_str)
            == Some("alternatives")
        {
            let fields = [
                "declaration_alternative_ids",
                "declaration_state",
                "implementation_context",
                "trait_context_id",
            ];
            object_with_shape(
                entity
                    .get("properties")
                    .ok_or(R10ContractError::InvalidProjection)?,
                &fields,
                &fields,
            )?;
        }
        if let Some(attributes) = properties.get("attributes") {
            let fields = ["evidence_id", "kind", "token_text"];
            for attribute in attributes
                .as_array()
                .ok_or(R10ContractError::InvalidProjection)?
            {
                object_with_shape(attribute, &fields, &fields)?;
            }
        }
    }

    validate_family_shapes(
        value,
        "relationships",
        &["evidence_ids", "id", "kind", "source", "target"],
    )?;
    validate_family_shapes(
        value,
        "claims",
        &["evidence_ids", "id", "state", "subject_id", "subject_kind"],
    )?;
    validate_family_shapes(
        value,
        "evidence",
        &["blob_oid", "end_byte", "id", "path", "start_byte"],
    )?;
    validate_family_shapes(
        value,
        "diagnostics",
        &["code", "evidence_ids", "id", "message"],
    )?;
    validate_family_shapes(
        value,
        "coverage_gaps",
        &["capability", "evidence_ids", "id", "state"],
    )?;
    validate_family_shapes(
        value,
        "documents",
        &[
            "blake3",
            "byte_length",
            "document_id",
            "kind",
            "path",
            "subject_id",
        ],
    )?;
    validate_family_shapes(
        value,
        "document_statements",
        &[
            "coverage_gap_ids",
            "document_id",
            "evidence_ids",
            "statement_id",
            "subject_ids",
            "truth_state",
        ],
    )?;
    Ok(())
}

fn validate_family_shapes(
    value: &Value,
    family: &'static str,
    fields: &[&str],
) -> Result<(), R10ContractError> {
    for record in value[family]
        .as_array()
        .ok_or(R10ContractError::InvalidProjection)?
    {
        object_with_shape(record, fields, fields)?;
    }
    Ok(())
}

fn object_with_shape<'a>(
    value: &'a Value,
    required: &[&str],
    allowed: &[&str],
) -> Result<&'a Map<String, Value>, R10ContractError> {
    let object = value
        .as_object()
        .ok_or(R10ContractError::InvalidProjection)?;
    if required.iter().any(|field| !object.contains_key(*field))
        || object
            .keys()
            .any(|field| !allowed.contains(&field.as_str()))
    {
        return Err(R10ContractError::InvalidProjection);
    }
    Ok(object)
}

fn validate_blake3_hash(value: &Value) -> Result<(), R10ContractError> {
    let fields = ["algorithm", "value"];
    let hash = object_with_shape(value, &fields, &fields)?;
    if hash.get("algorithm").and_then(Value::as_str) != Some("blake3-256")
        || !hash
            .get("value")
            .and_then(Value::as_str)
            .is_some_and(|value| lower_hex_with_length(value, 64))
    {
        return Err(R10ContractError::InvalidProjection);
    }
    Ok(())
}

fn lower_hex_with_length(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_family_id(family: &str, identifier: &str) -> bool {
    let prefix = match family {
        "entities" => "urn:codenoesis:entity:blake3:",
        "relationships" => "urn:codenoesis:relationship:blake3:",
        "claims" => "urn:codenoesis:claim:blake3:",
        "evidence" => {
            return [
                "urn:codenoesis:evidence:blake3:",
                "urn:codenoesis:evidence:sha256:",
            ]
            .iter()
            .any(|prefix| valid_prefixed_digest(identifier, prefix));
        }
        "diagnostics" => "urn:codenoesis:diagnostic:blake3:",
        "coverage_gaps" => "urn:codenoesis:coverage-gap:blake3:",
        "documents" => "urn:codenoesis:document:blake3:",
        "document_statements" => "urn:codenoesis:statement:blake3:",
        _ => return false,
    };
    valid_prefixed_digest(identifier, prefix)
}

fn valid_prefixed_digest(value: &str, prefix: &str) -> bool {
    value
        .strip_prefix(prefix)
        .is_some_and(|digest| lower_hex_with_length(digest, 64))
}

fn enforce_portable_alternative_limit(
    limit: &'static str,
    maximum: u64,
    observed: u64,
) -> Result<(), R10ContractError> {
    if observed > maximum {
        return Err(R10ContractError::LimitExceeded {
            limit,
            maximum,
            observed,
        });
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn validate_portable_value(value: &Value, sha256: R10Sha256) -> Result<(), R10ContractError> {
    let object = value
        .as_object()
        .ok_or(R10ContractError::InvalidProjection)?;
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
        return Err(R10ContractError::InvalidProjection);
    }
    let schema = object
        .get("schema_version")
        .and_then(Value::as_str)
        .unwrap_or("missing");
    if schema != R10_PORTABLE_GRAPH_VERSION {
        return Err(R10ContractError::UnsupportedPortableGraphSchema(bounded(
            schema, 256,
        )));
    }
    if value
        .pointer("/source_snapshot/schema_version")
        .and_then(Value::as_str)
        != Some(R10_SNAPSHOT_VERSION)
        || object.get("ontology_version").and_then(Value::as_str) != Some(R10_ONTOLOGY_VERSION)
        || object.get("query_contract_version").and_then(Value::as_str) != Some(R10_QUERY_VERSION)
    {
        return Err(R10ContractError::InvalidProjection);
    }
    ensure_nesting(value, 0)?;
    validate_private_fields(value)?;
    validate_portable_shapes(value)?;
    let mut family_ids = BTreeMap::new();
    for (family, key) in PORTABLE_FAMILIES {
        let values = object
            .get(family)
            .and_then(Value::as_array)
            .ok_or(R10ContractError::InvalidProjection)?;
        let mut previous = None;
        let mut current = BTreeSet::new();
        for record in values {
            let identifier = record
                .get(key)
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .ok_or(R10ContractError::InvalidProjection)?;
            if !valid_family_id(family, identifier) {
                return Err(R10ContractError::InvalidProjection);
            }
            if previous.is_some_and(|value: &str| value >= identifier)
                || !current.insert(identifier.to_owned())
            {
                return Err(R10ContractError::IdentityConflict {
                    family,
                    id: identifier.to_owned(),
                });
            }
            previous = Some(identifier);
        }
        family_ids.insert(family, current);
    }
    let entity_ids = &family_ids["entities"];
    let relationship_ids = &family_ids["relationships"];
    let evidence_ids = &family_ids["evidence"];
    let coverage_ids = &family_ids["coverage_gaps"];
    let document_ids = &family_ids["documents"];
    let mut subject_ids = BTreeSet::new();
    for family in [
        "entities",
        "relationships",
        "claims",
        "evidence",
        "diagnostics",
        "coverage_gaps",
        "documents",
        "document_statements",
    ] {
        subject_ids.extend(family_ids[family].iter().cloned());
    }
    let repository_identity = value
        .pointer("/repository/identity")
        .and_then(Value::as_str)
        .ok_or(R10ContractError::InvalidProjection)?;
    for relationship in object["relationships"]
        .as_array()
        .ok_or(R10ContractError::InvalidProjection)?
    {
        for field in ["source", "target"] {
            let identifier = relationship
                .get(field)
                .and_then(Value::as_str)
                .ok_or(R10ContractError::InvalidProjection)?;
            if !entity_ids.contains(identifier) {
                return Err(R10ContractError::ReferenceMismatch {
                    family: "relationships",
                    id: identifier.to_owned(),
                });
            }
        }
        validate_evidence_ids(relationship, evidence_ids, "relationships")?;
    }
    for claim in object["claims"]
        .as_array()
        .ok_or(R10ContractError::InvalidProjection)?
    {
        let subject_id = claim
            .get("subject_id")
            .and_then(Value::as_str)
            .ok_or(R10ContractError::InvalidProjection)?;
        let valid = match claim.get("subject_kind").and_then(Value::as_str) {
            Some("entity") => entity_ids.contains(subject_id),
            Some("relationship") => relationship_ids.contains(subject_id),
            _ => false,
        };
        if !valid {
            return Err(R10ContractError::ReferenceMismatch {
                family: "claims",
                id: subject_id.to_owned(),
            });
        }
        validate_evidence_ids(claim, evidence_ids, "claims")?;
    }
    for entity in object["entities"]
        .as_array()
        .ok_or(R10ContractError::InvalidProjection)?
    {
        if entity.get("kind").and_then(Value::as_str) == Some("rust.declaration_alternative") {
            for field in ["subject_id", "source_file_id"] {
                let identifier = entity
                    .get(field)
                    .and_then(Value::as_str)
                    .ok_or(R10ContractError::InvalidProjection)?;
                if !entity_ids.contains(identifier) {
                    return Err(R10ContractError::ReferenceMismatch {
                        family: "entities",
                        id: identifier.to_owned(),
                    });
                }
            }
            let declaration_evidence = entity
                .pointer("/properties/declaration_evidence_id")
                .and_then(Value::as_str)
                .ok_or(R10ContractError::InvalidProjection)?;
            if !evidence_ids.contains(declaration_evidence) {
                return Err(R10ContractError::ReferenceMismatch {
                    family: "entities",
                    id: declaration_evidence.to_owned(),
                });
            }
            for attribute in entity
                .pointer("/properties/attributes")
                .and_then(Value::as_array)
                .ok_or(R10ContractError::InvalidProjection)?
            {
                let identifier = attribute
                    .get("evidence_id")
                    .and_then(Value::as_str)
                    .ok_or(R10ContractError::InvalidProjection)?;
                if !evidence_ids.contains(identifier) {
                    return Err(R10ContractError::ReferenceMismatch {
                        family: "entities",
                        id: identifier.to_owned(),
                    });
                }
            }
        }
        if entity.get("evidence_ids").is_some() {
            validate_evidence_ids(entity, evidence_ids, "entities")?;
        }
    }
    validate_r10_portable_graph(value, repository_identity, entity_ids, evidence_ids)?;
    for family in ["diagnostics", "coverage_gaps"] {
        for record in object[family]
            .as_array()
            .ok_or(R10ContractError::InvalidProjection)?
        {
            if record.get("evidence_ids").is_some() {
                validate_evidence_ids(record, evidence_ids, family)?;
            }
        }
    }
    for document in object["documents"]
        .as_array()
        .ok_or(R10ContractError::InvalidProjection)?
    {
        let subject_id = document
            .get("subject_id")
            .and_then(Value::as_str)
            .ok_or(R10ContractError::InvalidProjection)?;
        if subject_id != repository_identity && !subject_ids.contains(subject_id) {
            return Err(R10ContractError::ReferenceMismatch {
                family: "documents",
                id: subject_id.to_owned(),
            });
        }
    }
    for statement in object["document_statements"]
        .as_array()
        .ok_or(R10ContractError::InvalidProjection)?
    {
        let document_id = statement
            .get("document_id")
            .and_then(Value::as_str)
            .ok_or(R10ContractError::InvalidProjection)?;
        if !document_ids.contains(document_id) {
            return Err(R10ContractError::ReferenceMismatch {
                family: "document_statements",
                id: document_id.to_owned(),
            });
        }
        validate_subject_ids(statement, &subject_ids, repository_identity)?;
        validate_evidence_references(statement, evidence_ids, "document_statements")?;
        validate_coverage_gap_ids(statement, coverage_ids)?;
    }
    validate_portable_paths(value)?;
    let observed = value
        .pointer("/projection/family_sha256")
        .ok_or(R10ContractError::InvalidProjection)?;
    if observed != &family_digests(value, sha256)? || entity_ids.is_empty() {
        return Err(R10ContractError::InvalidProjection);
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn validate_r10_portable_graph(
    value: &Value,
    repository_identity: &str,
    entity_ids: &BTreeSet<String>,
    evidence_ids: &BTreeSet<String>,
) -> Result<(), R10ContractError> {
    let entities = value["entities"]
        .as_array()
        .ok_or(R10ContractError::InvalidProjection)?;
    let relationships = value["relationships"]
        .as_array()
        .ok_or(R10ContractError::InvalidProjection)?;
    let claims = value["claims"]
        .as_array()
        .ok_or(R10ContractError::InvalidProjection)?;
    let evidence = value["evidence"]
        .as_array()
        .ok_or(R10ContractError::InvalidProjection)?;
    let entity_map = entities
        .iter()
        .map(|entity| {
            record_id(entity)
                .map(|identifier| (identifier, entity))
                .ok_or(R10ContractError::InvalidProjection)
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let evidence_map = evidence
        .iter()
        .map(|record| {
            record_id(record)
                .map(|identifier| (identifier, record))
                .ok_or(R10ContractError::InvalidProjection)
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;

    for record in evidence {
        let identifier = record_id(record).ok_or(R10ContractError::InvalidProjection)?;
        let blob_oid = record
            .get("blob_oid")
            .and_then(Value::as_str)
            .ok_or(R10ContractError::InvalidProjection)?;
        let start = record
            .get("start_byte")
            .and_then(Value::as_u64)
            .ok_or(R10ContractError::InvalidProjection)?;
        let end = record
            .get("end_byte")
            .and_then(Value::as_u64)
            .ok_or(R10ContractError::InvalidProjection)?;
        if !lower_hex_with_length(blob_oid, 40) || start >= end {
            return Err(R10ContractError::ReferenceMismatch {
                family: "evidence",
                id: identifier.to_owned(),
            });
        }
    }

    let mut alternatives = BTreeMap::<String, Vec<(String, String)>>::new();
    let mut alternatives_per_source = BTreeMap::<String, u64>::new();
    for entity in entities {
        validate_entity_references(entity, entity_ids, evidence_ids)?;
        if entity.get("kind").and_then(Value::as_str) != Some("rust.declaration_alternative") {
            continue;
        }
        let identifier = record_id(entity).ok_or(R10ContractError::InvalidProjection)?;
        let subject_id = entity
            .get("subject_id")
            .and_then(Value::as_str)
            .ok_or(R10ContractError::InvalidProjection)?;
        let source_file_id = entity
            .get("source_file_id")
            .and_then(Value::as_str)
            .ok_or(R10ContractError::InvalidProjection)?;
        let source_count = alternatives_per_source
            .entry(source_file_id.to_owned())
            .or_default();
        *source_count = source_count.saturating_add(1);
        enforce_portable_alternative_limit(
            "alternatives_per_source",
            MAX_R10_ALTERNATIVES_PER_SOURCE,
            *source_count,
        )?;
        let declaration_evidence_id = entity
            .pointer("/properties/declaration_evidence_id")
            .and_then(Value::as_str)
            .ok_or(R10ContractError::InvalidProjection)?;
        if identifier
            != declaration_alternative_id(repository_identity, subject_id, declaration_evidence_id)
        {
            return Err(R10ContractError::IdentityConflict {
                family: "entities",
                id: identifier.to_owned(),
            });
        }
        let source_file = entity_map.get(source_file_id).copied().ok_or_else(|| {
            R10ContractError::ReferenceMismatch {
                family: "entities",
                id: source_file_id.to_owned(),
            }
        })?;
        let declaration_evidence = evidence_map
            .get(declaration_evidence_id)
            .copied()
            .ok_or_else(|| R10ContractError::ReferenceMismatch {
                family: "entities",
                id: declaration_evidence_id.to_owned(),
            })?;
        if source_file.get("kind").and_then(Value::as_str) != Some("source.file")
            || source_file
                .pointer("/properties/path")
                .and_then(Value::as_str)
                != declaration_evidence.get("path").and_then(Value::as_str)
            || source_file
                .pointer("/properties/blob_oid")
                .and_then(Value::as_str)
                != declaration_evidence.get("blob_oid").and_then(Value::as_str)
        {
            return Err(R10ContractError::ReferenceMismatch {
                family: "entities",
                id: identifier.to_owned(),
            });
        }
        alternatives
            .entry(subject_id.to_owned())
            .or_default()
            .push((identifier.to_owned(), declaration_evidence_id.to_owned()));
    }

    let alternative_count = alternatives.values().map(Vec::len).sum::<usize>();
    enforce_portable_alternative_limit(
        "alternatives_per_snapshot",
        MAX_R10_ALTERNATIVES_PER_SNAPSHOT,
        u64::try_from(alternative_count).unwrap_or(u64::MAX),
    )?;
    if relationships
        .iter()
        .filter(|relationship| {
            relationship.get("kind").and_then(Value::as_str) == Some(HAS_DECLARATION_ALTERNATIVE)
        })
        .count()
        != alternative_count
    {
        return Err(R10ContractError::InvalidProjection);
    }
    for (logical_method_id, occurrences) in alternatives {
        enforce_portable_alternative_limit(
            "alternatives_per_logical_method",
            MAX_R10_ALTERNATIVES_PER_METHOD,
            u64::try_from(occurrences.len()).unwrap_or(u64::MAX),
        )?;
        if occurrences.len() < 2 {
            return Err(R10ContractError::InvalidProjection);
        }
        let logical_method = entity_map
            .get(logical_method_id.as_str())
            .copied()
            .ok_or_else(|| R10ContractError::ReferenceMismatch {
                family: "entities",
                id: logical_method_id.clone(),
            })?;
        if logical_method.get("kind").and_then(Value::as_str) != Some("rust.method") {
            return Err(R10ContractError::ReferenceMismatch {
                family: "entities",
                id: logical_method_id,
            });
        }
        let mut alternative_ids = occurrences
            .iter()
            .map(|(identifier, _)| identifier.clone())
            .collect::<Vec<_>>();
        alternative_ids.sort();
        let mut declaration_evidence_ids = occurrences
            .iter()
            .map(|(_, identifier)| identifier.clone())
            .collect::<Vec<_>>();
        declaration_evidence_ids.sort();
        declaration_evidence_ids.dedup();
        if declaration_evidence_ids.len() != occurrences.len()
            || !string_array_equals(
                &logical_method["properties"],
                "declaration_alternative_ids",
                &alternative_ids,
            )
        {
            return Err(R10ContractError::IdentityConflict {
                family: "entities",
                id: logical_method_id,
            });
        }
        let logical_claim = unique_record(claims, |claim| {
            claim.get("subject_id").and_then(Value::as_str) == Some(logical_method_id.as_str())
        })?;
        if logical_claim.get("subject_kind").and_then(Value::as_str) != Some("entity")
            || logical_claim.get("state").and_then(Value::as_str) != Some("deterministic_fact")
            || !string_array_equals(logical_claim, "evidence_ids", &declaration_evidence_ids)
        {
            return Err(R10ContractError::InvalidProjection);
        }
        let defines = unique_record(relationships, |relationship| {
            relationship.get("kind").and_then(Value::as_str) == Some("DEFINES")
                && relationship.get("target").and_then(Value::as_str)
                    == Some(logical_method_id.as_str())
        })?;
        if !string_array_equals(defines, "evidence_ids", &declaration_evidence_ids) {
            return Err(R10ContractError::InvalidProjection);
        }
        for (alternative_id, declaration_evidence_id) in occurrences {
            let relationship = unique_record(relationships, |relationship| {
                relationship.get("kind").and_then(Value::as_str)
                    == Some(HAS_DECLARATION_ALTERNATIVE)
                    && relationship.get("target").and_then(Value::as_str)
                        == Some(alternative_id.as_str())
            })?;
            let relationship_id =
                record_id(relationship).ok_or(R10ContractError::InvalidProjection)?;
            if relationship.get("source").and_then(Value::as_str)
                != Some(logical_method_id.as_str())
                || relationship_id
                    != declaration_alternative_relationship_id(&logical_method_id, &alternative_id)
                || !string_array_equals(
                    relationship,
                    "evidence_ids",
                    std::slice::from_ref(&declaration_evidence_id),
                )
            {
                return Err(R10ContractError::IdentityConflict {
                    family: "relationships",
                    id: relationship_id.to_owned(),
                });
            }
            for subject_id in [alternative_id.as_str(), relationship_id] {
                let claim = unique_record(claims, |claim| {
                    claim.get("subject_id").and_then(Value::as_str) == Some(subject_id)
                })?;
                if claim.get("state").and_then(Value::as_str) != Some("deterministic_fact")
                    || !string_array_equals(
                        claim,
                        "evidence_ids",
                        std::slice::from_ref(&declaration_evidence_id),
                    )
                {
                    return Err(R10ContractError::InvalidProjection);
                }
            }
        }
    }
    Ok(())
}

fn validate_entity_references(
    entity: &Value,
    entity_ids: &BTreeSet<String>,
    evidence_ids: &BTreeSet<String>,
) -> Result<(), R10ContractError> {
    for field in [
        "crate_id",
        "owner_id",
        "source_file_id",
        "subject_id",
        "trait_context_id",
    ] {
        validate_optional_reference(entity.get(field), entity_ids, "entities")?;
    }
    let properties = entity
        .get("properties")
        .ok_or(R10ContractError::InvalidProjection)?;
    for field in [
        "manifest_id",
        "materialized_crate_id",
        "owner_id",
        "package_id",
        "source_entity_id",
        "source_file_id",
        "trait_context_id",
        "workspace_reference_id",
    ] {
        validate_optional_reference(properties.get(field), entity_ids, "entities")?;
    }
    for field in ["declaration_evidence_id", "evidence_id"] {
        validate_optional_reference(properties.get(field), evidence_ids, "entities")?;
    }
    if let Some(alternatives) = properties.get("declaration_alternative_ids") {
        validate_reference_array(alternatives, entity_ids, "entities")?;
    }
    if let Some(attributes) = properties.get("attributes") {
        for attribute in attributes
            .as_array()
            .ok_or(R10ContractError::InvalidProjection)?
        {
            validate_optional_reference(attribute.get("evidence_id"), evidence_ids, "entities")?;
        }
    }
    Ok(())
}

fn validate_optional_reference(
    value: Option<&Value>,
    identifiers: &BTreeSet<String>,
    family: &'static str,
) -> Result<(), R10ContractError> {
    let Some(value) = value else {
        return Ok(());
    };
    if value.is_null() {
        return Ok(());
    }
    let identifier = value.as_str().ok_or(R10ContractError::InvalidProjection)?;
    if !identifiers.contains(identifier) {
        return Err(R10ContractError::ReferenceMismatch {
            family,
            id: identifier.to_owned(),
        });
    }
    Ok(())
}

fn validate_reference_array(
    value: &Value,
    identifiers: &BTreeSet<String>,
    family: &'static str,
) -> Result<(), R10ContractError> {
    let mut previous = None;
    for value in value
        .as_array()
        .ok_or(R10ContractError::InvalidProjection)?
    {
        let identifier = value.as_str().ok_or(R10ContractError::InvalidProjection)?;
        if previous.is_some_and(|previous: &str| previous >= identifier)
            || !identifiers.contains(identifier)
        {
            return Err(R10ContractError::ReferenceMismatch {
                family,
                id: identifier.to_owned(),
            });
        }
        previous = Some(identifier);
    }
    Ok(())
}

fn string_array_equals(record: &Value, field: &str, expected: &[String]) -> bool {
    record
        .get(field)
        .and_then(Value::as_array)
        .is_some_and(|values| {
            values.len() == expected.len()
                && values
                    .iter()
                    .zip(expected)
                    .all(|(value, expected)| value.as_str() == Some(expected))
        })
}

fn unique_record<F>(values: &[Value], mut predicate: F) -> Result<&Value, R10ContractError>
where
    F: FnMut(&Value) -> bool,
{
    let mut matching = values.iter().filter(|value| predicate(value));
    let record = matching.next().ok_or(R10ContractError::InvalidProjection)?;
    if matching.next().is_some() {
        return Err(R10ContractError::InvalidProjection);
    }
    Ok(record)
}

fn family_digests(value: &Value, sha256: R10Sha256) -> Result<Value, R10ContractError> {
    let mut digests = Map::new();
    for (family, _) in PORTABLE_FAMILIES {
        let bytes = serde_json::to_vec(
            value
                .get(family)
                .ok_or(R10ContractError::InvalidProjection)?,
        )
        .map_err(|_| R10ContractError::Internal)?;
        digests.insert(family.to_owned(), Value::String(sha256(&bytes)));
    }
    Ok(Value::Object(digests))
}

fn portable_documents(manifest: &Value) -> Result<(Vec<Value>, Vec<Value>), R10ContractError> {
    let source = manifest
        .get("documents")
        .and_then(Value::as_array)
        .ok_or(R10ContractError::InvalidSnapshot)?;
    let mut documents = Vec::with_capacity(source.len());
    let mut statements = Vec::new();
    for document in source {
        let mut record = document
            .as_object()
            .cloned()
            .ok_or(R10ContractError::InvalidSnapshot)?;
        let document_id = record
            .get("document_id")
            .and_then(Value::as_str)
            .ok_or(R10ContractError::InvalidSnapshot)?
            .to_owned();
        let document_statements = record
            .remove("statements")
            .and_then(|value| value.as_array().cloned())
            .ok_or(R10ContractError::InvalidSnapshot)?;
        documents.push(Value::Object(record));
        for statement in document_statements {
            let mut statement = statement
                .as_object()
                .cloned()
                .ok_or(R10ContractError::InvalidSnapshot)?;
            if statement
                .insert("document_id".to_owned(), Value::String(document_id.clone()))
                .is_some()
            {
                return Err(R10ContractError::InvalidSnapshot);
            }
            statements.push(Value::Object(statement));
        }
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
) -> Result<(), R10ContractError> {
    if manifest.get("schema_version").and_then(Value::as_str)
        != Some("codenoesis.documentation-manifest/v1")
        || manifest.get("repository_identity").and_then(Value::as_str)
            != Some(head.repository_identity.as_str())
        || manifest.get("snapshot_id").and_then(Value::as_str) != Some(head.snapshot_id.as_str())
    {
        return Err(R10ContractError::InvalidSnapshot);
    }
    Ok(())
}

fn validate_evidence_ids(
    record: &Value,
    evidence_ids: &BTreeSet<String>,
    family: &'static str,
) -> Result<(), R10ContractError> {
    let references = record
        .get("evidence_ids")
        .and_then(Value::as_array)
        .ok_or(R10ContractError::InvalidProjection)?;
    let mut previous = None;
    for reference in references {
        let identifier = reference
            .as_str()
            .ok_or(R10ContractError::InvalidProjection)?;
        if previous.is_some_and(|value: &str| value >= identifier)
            || !evidence_ids.contains(identifier)
        {
            return Err(R10ContractError::ReferenceMismatch {
                family,
                id: identifier.to_owned(),
            });
        }
        previous = Some(identifier);
    }
    Ok(())
}

fn validate_evidence_references(
    record: &Value,
    evidence_ids: &BTreeSet<String>,
    family: &'static str,
) -> Result<(), R10ContractError> {
    let references = record
        .get("evidence_ids")
        .and_then(Value::as_array)
        .ok_or(R10ContractError::InvalidProjection)?;
    for reference in references {
        let identifier = reference
            .as_str()
            .ok_or(R10ContractError::InvalidProjection)?;
        if !evidence_ids.contains(identifier) {
            return Err(R10ContractError::ReferenceMismatch {
                family,
                id: identifier.to_owned(),
            });
        }
    }
    Ok(())
}

fn validate_subject_ids(
    record: &Value,
    subject_ids: &BTreeSet<String>,
    repository_identity: &str,
) -> Result<(), R10ContractError> {
    let references = record
        .get("subject_ids")
        .and_then(Value::as_array)
        .ok_or(R10ContractError::InvalidProjection)?;
    for reference in references {
        let identifier = reference
            .as_str()
            .ok_or(R10ContractError::InvalidProjection)?;
        if identifier != repository_identity && !subject_ids.contains(identifier) {
            return Err(R10ContractError::ReferenceMismatch {
                family: "document_statements",
                id: identifier.to_owned(),
            });
        }
    }
    Ok(())
}

fn validate_coverage_gap_ids(
    record: &Value,
    coverage_ids: &BTreeSet<String>,
) -> Result<(), R10ContractError> {
    let references = record
        .get("coverage_gap_ids")
        .and_then(Value::as_array)
        .ok_or(R10ContractError::InvalidProjection)?;
    for reference in references {
        let identifier = reference
            .as_str()
            .ok_or(R10ContractError::InvalidProjection)?;
        if !coverage_ids.contains(identifier) {
            return Err(R10ContractError::ReferenceMismatch {
                family: "document_statements",
                id: identifier.to_owned(),
            });
        }
    }
    Ok(())
}

fn validate_portable_paths(value: &Value) -> Result<(), R10ContractError> {
    for evidence in value["evidence"]
        .as_array()
        .ok_or(R10ContractError::InvalidProjection)?
    {
        let path = evidence
            .get("path")
            .and_then(Value::as_str)
            .ok_or(R10ContractError::InvalidProjection)?;
        if !safe_relative_path(path) {
            return Err(R10ContractError::UnsafePayload {
                reason: "unsafe_evidence_path",
            });
        }
    }
    for document in value["documents"]
        .as_array()
        .ok_or(R10ContractError::InvalidProjection)?
    {
        if let Some(path) = document.get("path").and_then(Value::as_str)
            && !safe_relative_path(path)
        {
            return Err(R10ContractError::UnsafePayload {
                reason: "unsafe_document_path",
            });
        }
    }
    Ok(())
}

fn validate_private_fields(value: &Value) -> Result<(), R10ContractError> {
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
                    return Err(R10ContractError::UnsafePayload {
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
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    Ok(())
}

fn ensure_nesting(value: &Value, depth: u64) -> Result<(), R10ContractError> {
    if depth > MAX_R10_JSON_NESTING {
        return Err(R10ContractError::LimitExceeded {
            limit: "json_nesting",
            maximum: MAX_R10_JSON_NESTING,
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

fn enforce_portable_size(length: usize) -> Result<(), R10ContractError> {
    let observed = u64::try_from(length).unwrap_or(u64::MAX);
    if observed > MAX_R10_PORTABLE_GRAPH_BYTES {
        return Err(R10ContractError::LimitExceeded {
            limit: "portable_graph_bytes",
            maximum: MAX_R10_PORTABLE_GRAPH_BYTES,
            observed,
        });
    }
    Ok(())
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
        let identifier = record_id(value).ok_or(QueryContractError::InvalidSnapshot)?;
        if result.insert(identifier, value).is_some() {
            return Err(QueryContractError::InvalidSnapshot);
        }
    }
    Ok(result)
}

fn record_id(value: &Value) -> Option<&str> {
    value.get("id").and_then(Value::as_str)
}

fn strictly_ordered<'a>(values: impl IntoIterator<Item = &'a str>) -> bool {
    let mut previous = None;
    for value in values {
        if previous.is_some_and(|previous| previous >= value) {
            return false;
        }
        previous = Some(value);
    }
    true
}

fn map_v8_error(error: RepositorySnapshotV8Error) -> RepositorySnapshotV12Error {
    match error {
        RepositorySnapshotV8Error::Serialization(error) => {
            RepositorySnapshotV12Error::Serialization(error)
        }
        RepositorySnapshotV8Error::LimitExceeded(error) => {
            RepositorySnapshotV12Error::LimitExceeded(error)
        }
        RepositorySnapshotV8Error::ContractInvalid => RepositorySnapshotV12Error::ContractInvalid,
        RepositorySnapshotV8Error::OutputLengthOverflow => {
            RepositorySnapshotV12Error::OutputLengthOverflow
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pt_fr_exp_003_portable_size_maximum_and_plus_one_are_exact() {
        let maximum =
            usize::try_from(MAX_R10_PORTABLE_GRAPH_BYTES).expect("R10 portable maximum fits usize");
        assert_eq!(enforce_portable_size(maximum), Ok(()));
        assert_eq!(
            enforce_portable_size(maximum + 1),
            Err(R10ContractError::LimitExceeded {
                limit: "portable_graph_bytes",
                maximum: MAX_R10_PORTABLE_GRAPH_BYTES,
                observed: MAX_R10_PORTABLE_GRAPH_BYTES + 1,
            })
        );
    }

    #[test]
    fn pt_fr_exp_003_json_nesting_maximum_and_plus_one_are_exact() {
        assert_eq!(ensure_nesting(&nested_array(64), 0), Ok(()));
        assert_eq!(
            ensure_nesting(&nested_array(65), 0),
            Err(R10ContractError::LimitExceeded {
                limit: "json_nesting",
                maximum: MAX_R10_JSON_NESTING,
                observed: MAX_R10_JSON_NESTING + 1,
            })
        );
    }

    #[test]
    fn pt_fr_exp_003_portable_alternative_limits_are_exact() {
        for (limit, maximum) in [
            (
                "alternatives_per_logical_method",
                MAX_R10_ALTERNATIVES_PER_METHOD,
            ),
            ("alternatives_per_source", MAX_R10_ALTERNATIVES_PER_SOURCE),
            (
                "alternatives_per_snapshot",
                MAX_R10_ALTERNATIVES_PER_SNAPSHOT,
            ),
        ] {
            assert_eq!(
                enforce_portable_alternative_limit(limit, maximum, maximum),
                Ok(())
            );
            assert_eq!(
                enforce_portable_alternative_limit(limit, maximum, maximum + 1),
                Err(R10ContractError::LimitExceeded {
                    limit,
                    maximum,
                    observed: maximum + 1,
                })
            );
        }
    }

    #[test]
    fn sec_fr_exp_003_portable_paths_are_closed_and_relative() {
        for accepted in ["src/lib.rs", "Cargo.toml", "modules/crate.md"] {
            assert!(safe_relative_path(accepted));
        }
        for rejected in [
            "",
            "../secret",
            "/absolute",
            "nested/../escape",
            "windows\\escape",
            "line\nbreak",
        ] {
            assert!(!safe_relative_path(rejected), "accepted {rejected:?}");
        }
    }

    fn nested_array(depth: u64) -> Value {
        let mut value = Value::Null;
        for _ in 0..depth {
            value = Value::Array(vec![value]);
        }
        value
    }
}
