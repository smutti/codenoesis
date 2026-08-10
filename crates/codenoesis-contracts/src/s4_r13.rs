use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::path::{Component, Path};

use codenoesis_domain::s4_k1::{
    CallableSemanticsError, K1_EXTRACTOR_VERSION, K1_GRAPH_VERSION, K1_ONTOLOGY_VERSION, K1_PROFILE,
};
use codenoesis_domain::s4_r3::R3_WORKSPACE_PROFILE;
use codenoesis_domain::s4_r4::R4_MANIFEST_PROFILE;
use codenoesis_domain::s4_r5::{R5_RUST_SEMANTIC_EXTRACTOR_VERSION, RustSemanticError};
use codenoesis_domain::s4_r6::{
    FrameworkError, R6_FRAMEWORK_EXTRACTOR_VERSION, R6_FRAMEWORK_PROFILE,
};
use codenoesis_domain::s4_r7::{
    CompilerIndexError, R7_COMPILER_EXTRACTOR_VERSION, R7_COMPILER_INDEX_PROFILE,
};
pub use codenoesis_domain::s4_r13::{
    CallableCompilerJoin, CallableCompilerJoinIndex, CallableScipCompositionError,
    CallableScipKnowledge, MAX_R13_CALLABLE_COMPILER_JOINS, R13_COMPOSITION_VERSION,
    R13_CONFIGURATION_VERSION, R13_ERROR_VERSION, R13_EXTRACTION_CHUNK_VERSION,
    R13_EXTRACTION_CONTRACT_VERSION, R13_GRAPH_VERSION, R13_INDEX_VERSION,
    R13_LOCAL_EXPLORER_VERSION, R13_ONTOLOGY_VERSION, R13_PIPELINE_VERSION,
    R13_PORTABLE_GRAPH_VERSION, R13_QUERY_VERSION, R13_RELATIONSHIP_KIND,
    R13_SEMANTIC_HASH_CONTRACT_VERSION, R13_SNAPSHOT_VERSION,
};
use codenoesis_domain::storage::{
    LocalSnapshotHead, PublicationCandidate, SNAPSHOT_SCHEMA_VERSION_V15, StorageComponent,
    StorageError,
};
use codenoesis_domain::{
    AcquisitionError, LimitKind, RepositoryInventory, STANDARD_LOCAL_S1_LIMITS,
};
use serde_json::{Map, Value, json};

use super::s4::{MAX_QUERY_BYTES, QueryContractError};
use super::s4_k1::{RepositorySnapshotV11, RepositorySnapshotV11Error, local_query_result_v6};
use super::s4_r7::{RepositorySnapshotV10, RepositorySnapshotV10Error};
use super::{
    LimitedVecWriter, PublicationCandidateError, SnapshotEnvelopeV1, publication_candidate,
    semantic_hash,
};

const CONFIGURATION_V12_HASH_DOMAIN: &[u8] = b"codenoesis.configuration.semantic.v12";
const SNAPSHOT_V15_HASH_DOMAIN: &[u8] = b"codenoesis.repository-snapshot.semantic.v15";
const EXTRACTION_V12_HASH_DOMAIN: &[u8] = b"codenoesis.extraction-chunk.semantic.v12";
const GRAPH_V12_HASH_DOMAIN: &[u8] = b"codenoesis.knowledge-graph.semantic.v12";
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
const GRAPH_FAMILIES: [&str; 6] = [
    "entities",
    "relationships",
    "claims",
    "evidence",
    "diagnostics",
    "coverage",
];

pub type R13Sha256 = fn(&[u8]) -> String;
pub const R13_PORTABLE_MARKER: &str = ".codenoesis-portable-graph-v6";
pub const R13_EXPLORER_MARKER: &str = ".codenoesis-local-explorer-v6";
pub const R13_EXPLORER_SECURITY_PROFILE: &str = "codenoesis.local-explorer-security/v6";
pub const MAX_R13_PORTABLE_GRAPH_BYTES: u64 = 268_435_456;
pub const MAX_R13_JSON_NESTING: u64 = 64;
pub const MAX_R13_TEXT_SEARCH_RESULTS: u64 = 100;
pub const R13_TRAVERSAL_DEPTH_DEFAULT: u64 = 1;
pub const MAX_R13_TRAVERSAL_DEPTH: u64 = 2;

#[derive(Clone, Debug)]
pub struct CodeNoesisErrorV20 {
    value: Value,
}

impl CodeNoesisErrorV20 {
    #[must_use]
    pub fn invalid_profile(field: &str, profile: &str) -> Self {
        match field {
            "compiler_index_profile" => Self::new(
                "input.invalid_compiler_index_profile",
                "input",
                "invalid compiler index profile",
                json!({"profile": bounded_nonempty(profile, 256, "missing")}),
            ),
            "rust_callable_profile" => Self::new(
                "input.invalid_rust_callable_profile",
                "input",
                "invalid rust callable profile",
                json!({"profile": bounded_nonempty(profile, 256, "missing")}),
            ),
            _ => Self::unsupported_composition("invalid_optional_profile"),
        }
    }

    #[must_use]
    pub fn unsupported_composition(reason: &str) -> Self {
        Self::new(
            "input.unsupported_rust_callable_scip_composition",
            "input",
            "unsupported R13 callable and compiler composition",
            json!({
                "compiler_index_profile": R7_COMPILER_INDEX_PROFILE,
                "rust_callable_profile": K1_PROFILE,
                "reason": bounded_nonempty(reason, 128, "unsupported_composition")
            }),
        )
    }

    #[must_use]
    pub fn from_composition(error: &CallableScipCompositionError) -> Self {
        match error {
            CallableScipCompositionError::Callable(error) => Self::from_callable(error),
            CallableScipCompositionError::Compiler(error) => Self::from_compiler_index(error),
            CallableScipCompositionError::SignatureCardinality {
                callable_id,
                observed,
            } => Self::new(
                if *observed == 0 {
                    "extraction.callable_scip_signature_missing"
                } else {
                    "extraction.callable_scip_signature_duplicate"
                },
                "extraction",
                "callable signature cardinality is invalid",
                json!({"callable_id": bounded(callable_id, 256), "observed": observed}),
            ),
            CallableScipCompositionError::DuplicateCompilerOwnership {
                callable_id,
                observed,
            } => Self::new(
                "extraction.callable_scip_duplicate_owner",
                "extraction",
                "callable has multiple compiler symbol owners",
                json!({"callable_id": bounded(callable_id, 256), "observed": observed}),
            ),
            CallableScipCompositionError::InvalidJoinEvidence { callable_id } => Self::new(
                "extraction.callable_scip_evidence_invalid",
                "extraction",
                "callable and compiler correspondence evidence is invalid",
                json!({"callable_id": bounded(callable_id, 256)}),
            ),
            CallableScipCompositionError::LimitExceeded { maximum, observed } => Self::new(
                "extraction.callable_scip_limit_exceeded",
                "extraction",
                "callable and compiler correspondence limit exceeded",
                json!({"limit": "callable_compiler_joins", "maximum": maximum, "observed": observed}),
            ),
            CallableScipCompositionError::ContractInvalid => Self::new(
                "extraction.callable_scip_identity_conflict",
                "extraction",
                "callable and compiler correspondence identity conflict",
                json!({}),
            ),
        }
    }

    #[must_use]
    pub fn from_callable(error: &CallableSemanticsError) -> Self {
        match error {
            CallableSemanticsError::Source(FrameworkError::Source(
                RustSemanticError::InvalidDeclaration {
                    path,
                    start_byte,
                    declaration_kind,
                },
            )) => Self::new(
                "extraction.invalid_rust_source",
                "extraction",
                "invalid rust source",
                json!({
                    "path": bounded(path, 1024),
                    "start_byte": start_byte,
                    "declaration_kind": bounded(declaration_kind, 128)
                }),
            ),
            CallableSemanticsError::InvalidSyntax {
                path,
                start_byte,
                syntax_kind,
            } => Self::new(
                "extraction.invalid_callable_syntax",
                "extraction",
                "invalid rust callable syntax",
                json!({
                    "path": bounded(path, 1024),
                    "start_byte": start_byte,
                    "syntax_kind": bounded(syntax_kind, 128)
                }),
            ),
            CallableSemanticsError::IdentityConflict {
                kind,
                normalized_identity,
            } => Self::new(
                "extraction.callable_identity_conflict",
                "extraction",
                "rust callable identity conflict",
                json!({
                    "kind": bounded(kind, 128),
                    "normalized_identity": bounded(normalized_identity, 512)
                }),
            ),
            CallableSemanticsError::LimitExceeded {
                limit,
                maximum,
                observed,
            } => Self::new(
                "extraction.callable_limit_exceeded",
                "extraction",
                "rust callable limit exceeded",
                json!({"limit": limit.as_str(), "maximum": maximum, "observed": observed}),
            ),
            CallableSemanticsError::UnsupportedComposition => {
                Self::unsupported_composition("callable_composition")
            }
            CallableSemanticsError::Source(_) | CallableSemanticsError::ContractInvalid => {
                Self::internal("callable_extraction")
            }
        }
    }

    #[must_use]
    pub fn from_compiler_index(error: &CompilerIndexError) -> Self {
        match error {
            CompilerIndexError::UnsafePath { path, reason } => Self::new(
                "input.unsafe_compiler_index_path",
                "input",
                "unsafe compiler index path",
                json!({"path": bounded(path, 1024), "reason": bounded(reason, 128)}),
            ),
            CompilerIndexError::InvalidBinding { path, reason } => Self::new(
                "input.invalid_compiler_index_binding",
                "input",
                "invalid compiler index binding",
                json!({"path": bounded(path, 1024), "reason": bounded(reason, 256)}),
            ),
            CompilerIndexError::BindingMismatch {
                subject,
                expected_sha256,
                observed_sha256,
            } => Self::new(
                "extraction.compiler_index_binding_mismatch",
                "extraction",
                "compiler index binding mismatch",
                json!({
                    "subject": subject.as_str(),
                    "expected_sha256": valid_sha256_or_zero(expected_sha256),
                    "observed_sha256": valid_sha256_or_zero(observed_sha256)
                }),
            ),
            CompilerIndexError::IdentityConflict {
                normalized_preimage_sha256,
            } => Self::new(
                "extraction.compiler_index_identity_conflict",
                "extraction",
                "compiler index identity conflict",
                json!({
                    "normalized_preimage_sha256": valid_sha256_or_zero(
                        normalized_preimage_sha256
                    )
                }),
            ),
            CompilerIndexError::LimitExceeded {
                limit,
                maximum,
                observed,
            } => Self::new(
                "acquisition.limit_exceeded",
                "acquisition",
                "compiler index limit exceeded",
                json!({"limit": limit.as_str(), "maximum": maximum, "observed": observed}),
            ),
            CompilerIndexError::UnsupportedSchema { .. }
            | CompilerIndexError::UnsupportedProducer { .. }
            | CompilerIndexError::MalformedArtifact { .. }
            | CompilerIndexError::NoncanonicalArtifact { .. }
            | CompilerIndexError::AmbiguousEndpoint { .. }
            | CompilerIndexError::RelationConflict { .. }
            | CompilerIndexError::UnresolvableEvidence { .. }
            | CompilerIndexError::ContractInvalid => Self::new(
                "extraction.invalid_compiler_index",
                "extraction",
                "invalid compiler index",
                json!({"reason": bounded(&error.to_string(), 256)}),
            ),
        }
    }

    #[must_use]
    pub fn from_acquisition_limit(error: &AcquisitionError) -> Option<Self> {
        match error {
            AcquisitionError::LimitExceeded {
                limit,
                maximum,
                observed,
            } => Some(Self::new(
                "acquisition.limit_exceeded",
                "acquisition",
                "R13 acquisition limit exceeded",
                json!({"limit": limit.as_str(), "maximum": maximum, "observed": observed}),
            )),
            _ => None,
        }
    }

    #[must_use]
    pub fn from_contract(error: &R13ContractError, explore: bool) -> Self {
        match error {
            R13ContractError::UnsupportedSnapshotSchema(observed) => Self::new(
                "export.invalid_snapshot",
                "export",
                "invalid R13 source snapshot",
                json!({"observed": bounded(observed, 256)}),
            ),
            R13ContractError::InvalidSnapshot => Self::new(
                "export.invalid_snapshot",
                "export",
                "invalid R13 source snapshot",
                json!({}),
            ),
            R13ContractError::LimitExceeded {
                limit,
                maximum,
                observed,
            } => Self::new(
                if explore {
                    "explorer.limit_exceeded"
                } else {
                    "export.limit_exceeded"
                },
                if explore { "explorer" } else { "export" },
                "R13 portable graph limit exceeded",
                json!({"limit": limit, "maximum": maximum, "observed": observed}),
            ),
            R13ContractError::AssetIntegrityMismatch => Self::new(
                "explorer.asset_integrity_mismatch",
                "explorer",
                "R13 explorer asset integrity mismatch",
                json!({}),
            ),
            R13ContractError::Internal => Self::internal("contract"),
            R13ContractError::UnsupportedPortableGraphSchema(_)
            | R13ContractError::Noncanonical { .. }
            | R13ContractError::IdentityConflict { .. }
            | R13ContractError::ReferenceMismatch { .. }
            | R13ContractError::UnsafePayload { .. }
            | R13ContractError::InvalidProjection => Self::new(
                "export.invalid_portable_graph_v6",
                "export",
                "invalid portable graph V6",
                json!({"reason": bounded(&error.to_string(), 256)}),
            ),
        }
    }

    #[must_use]
    pub fn invalid_snapshot() -> Self {
        Self::new(
            "snapshot.invalid_v15",
            "snapshot",
            "invalid R13 snapshot",
            json!({}),
        )
    }

    #[must_use]
    pub fn invalid_query(reason: &str) -> Self {
        Self::new(
            "query.invalid_v15",
            "query",
            "invalid R13 query state",
            json!({"reason": bounded(reason, 256)}),
        )
    }

    #[must_use]
    pub fn unsafe_output_path(path_sha256: &str, reason: &str) -> Self {
        Self::new(
            "input.unsafe_output_path",
            "input",
            "unsafe R13 output path",
            json!({
                "path_sha256": bounded(path_sha256, 64),
                "reason": bounded(reason, 128)
            }),
        )
    }

    #[must_use]
    pub fn internal(stage: &str) -> Self {
        Self::new(
            "internal.unexpected",
            "internal",
            "unexpected internal R13 failure",
            json!({"stage": bounded(stage, 128)}),
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    fn new(code: &str, stage: &str, message: &str, context: Value) -> Self {
        Self {
            value: json!({
                "schema_version": R13_ERROR_VERSION,
                "code": code,
                "stage": stage,
                "message": message,
                "retryable": false,
                "context": context
            }),
        }
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Serializes one `ErrorV20` followed by LF.
    ///
    /// # Errors
    ///
    /// Returns an error only when the internal JSON cannot be serialized.
    pub fn canonical_stderr(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = serde_json::to_vec(&self.value)?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

#[derive(Clone, Debug)]
pub struct RepositorySnapshotV15 {
    value: Value,
}

#[derive(Debug)]
pub enum RepositorySnapshotV15Error {
    Serialization(serde_json::Error),
    LimitExceeded(AcquisitionError),
    ContractInvalid,
    OutputLengthOverflow,
}

impl Display for RepositorySnapshotV15Error {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Serialization(_) => "R13 snapshot serialization failed",
            Self::LimitExceeded(_) => "R13 snapshot output limit exceeded",
            Self::ContractInvalid => "R13 snapshot contract is invalid",
            Self::OutputLengthOverflow => "R13 snapshot output length overflowed",
        })
    }
}

impl Error for RepositorySnapshotV15Error {}

impl RepositorySnapshotV15 {
    /// Builds the strict identity union of unchanged K1 and R7 lineages plus R13 joins.
    ///
    /// # Errors
    ///
    /// Returns the first inherited, union, join, serialization, or publication failure.
    #[allow(clippy::too_many_lines)]
    pub fn from_inventory_callable_scip(
        inventory: &RepositoryInventory,
        knowledge: &CallableScipKnowledge,
        envelope: SnapshotEnvelopeV1,
    ) -> Result<Self, RepositorySnapshotV15Error> {
        knowledge
            .validate()
            .map_err(|_| RepositorySnapshotV15Error::ContractInvalid)?;
        let callable = RepositorySnapshotV11::from_inventory_and_callable_semantics(
            inventory,
            &knowledge.callable,
            envelope.clone(),
        )
        .map_err(map_k1_snapshot_error)?;
        let compiler = RepositorySnapshotV10::from_inventory_and_compiler_index(
            inventory,
            &knowledge.callable.framework,
            &knowledge.compiler,
            None,
            envelope,
        )
        .map_err(map_r7_snapshot_error)?;
        let mut value = callable.value().clone();
        if callable.value().get("envelope") != compiler.value().get("envelope")
            || callable.value().pointer("/semantic/repository")
                != compiler.value().pointer("/semantic/repository")
            || callable.value().pointer("/semantic/inventory")
                != compiler.value().pointer("/semantic/inventory")
        {
            return Err(RepositorySnapshotV15Error::ContractInvalid);
        }
        let semantic = value
            .get_mut("semantic")
            .and_then(Value::as_object_mut)
            .ok_or(RepositorySnapshotV15Error::ContractInvalid)?;
        let configuration_without_hash = json!({
            "schema_version": R13_CONFIGURATION_VERSION,
            "profile": "standard-local-s4",
            "workspace_profile": R3_WORKSPACE_PROFILE,
            "manifest_profile": R4_MANIFEST_PROFILE,
            "rust_semantic_profile": "rust-semantic-depth-v1",
            "rust_framework_profile": R6_FRAMEWORK_PROFILE,
            "compiler_index_profile": R7_COMPILER_INDEX_PROFILE,
            "compiler_index_binding_sha256": knowledge.compiler.binding_sha256,
            "rust_callable_profile": K1_PROFILE
        });
        let configuration_hash =
            semantic_hash(CONFIGURATION_V12_HASH_DOMAIN, &configuration_without_hash);
        let mut configuration = configuration_without_hash;
        configuration["semantic_hash"] =
            json!({"algorithm": "blake3-256", "value": configuration_hash});
        semantic.insert("configuration".to_owned(), configuration);
        semantic.insert(
            "pipeline_version".to_owned(),
            Value::String(R13_PIPELINE_VERSION.to_owned()),
        );
        semantic.insert(
            "ontology_version".to_owned(),
            Value::String(R13_ONTOLOGY_VERSION.to_owned()),
        );
        semantic.insert(
            "extractor_contract_version".to_owned(),
            Value::String(R13_EXTRACTION_CONTRACT_VERSION.to_owned()),
        );
        semantic.insert(
            "extractor_versions".to_owned(),
            Value::Array(snapshot_extractor_versions()),
        );
        let callable_chunks = semantic
            .get("extraction_chunks")
            .cloned()
            .ok_or(RepositorySnapshotV15Error::ContractInvalid)?;
        let compiler_chunks = compiler
            .value()
            .pointer("/semantic/extraction_chunks")
            .ok_or(RepositorySnapshotV15Error::ContractInvalid)?;
        semantic.insert(
            "extraction_chunks".to_owned(),
            Value::Array(extraction_chunks_v12(
                &callable_chunks,
                compiler_chunks,
                knowledge,
            )?),
        );
        let callable_graph = semantic
            .get("knowledge_graph")
            .cloned()
            .ok_or(RepositorySnapshotV15Error::ContractInvalid)?;
        let compiler_graph = compiler
            .value()
            .pointer("/semantic/knowledge_graph")
            .ok_or(RepositorySnapshotV15Error::ContractInvalid)?;
        semantic.insert(
            "knowledge_graph".to_owned(),
            knowledge_graph_v12(&callable_graph, compiler_graph, knowledge)?,
        );
        let semantic_value = Value::Object(semantic.clone());
        let snapshot_hash = semantic_hash(SNAPSHOT_V15_HASH_DOMAIN, &semantic_value);
        let root = value
            .as_object_mut()
            .ok_or(RepositorySnapshotV15Error::ContractInvalid)?;
        root.insert(
            "schema_version".to_owned(),
            Value::String(R13_SNAPSHOT_VERSION.to_owned()),
        );
        root.insert(
            "semantic_hash".to_owned(),
            json!({"algorithm": "blake3-256", "value": snapshot_hash}),
        );
        publication_candidate(&value).map_err(|_| RepositorySnapshotV15Error::ContractInvalid)?;
        Ok(Self { value })
    }

    /// Serializes V15 under the unchanged standard local bound.
    ///
    /// # Errors
    ///
    /// Returns a serialization or output-limit failure.
    pub fn canonical_stdout(&self) -> Result<Vec<u8>, RepositorySnapshotV15Error> {
        let maximum = usize::try_from(STANDARD_LOCAL_S1_LIMITS.canonical_output_bytes)
            .map_err(|_| RepositorySnapshotV15Error::OutputLengthOverflow)?;
        let body_maximum = maximum
            .checked_sub(1)
            .ok_or(RepositorySnapshotV15Error::OutputLengthOverflow)?;
        let mut writer = LimitedVecWriter::new(body_maximum);
        let result = serde_json::to_writer(&mut writer, &self.value);
        if writer.overflowed() {
            return Err(RepositorySnapshotV15Error::LimitExceeded(
                AcquisitionError::LimitExceeded {
                    limit: LimitKind::CanonicalOutputBytes,
                    maximum: STANDARD_LOCAL_S1_LIMITS.canonical_output_bytes,
                    observed: STANDARD_LOCAL_S1_LIMITS
                        .canonical_output_bytes
                        .saturating_add(1),
                },
            ));
        }
        result.map_err(RepositorySnapshotV15Error::Serialization)?;
        let mut bytes = writer.into_inner();
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Serializes the exact V15 semantic payload.
    ///
    /// # Errors
    ///
    /// Returns an error only for an invalid internal JSON value.
    pub fn canonical_semantic(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(&self.value["semantic"])
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Converts V15 into the immutable publication model.
    ///
    /// # Errors
    ///
    /// Returns a strict contract or storage-integrity failure.
    pub fn publication_candidate(&self) -> Result<PublicationCandidate, PublicationCandidateError> {
        publication_candidate(&self.value)
    }
}

/// Validates one loaded V15 semantic payload against its visible head.
///
/// # Errors
///
/// Returns a typed storage-integrity failure on any mismatch.
pub fn validate_stored_snapshot_semantic_v15(
    semantic: &Value,
    head: &LocalSnapshotHead,
) -> Result<(), StorageError> {
    if head.snapshot_schema_version != SNAPSHOT_SCHEMA_VERSION_V15 {
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
pub struct LocalQueryResultV10 {
    value: Value,
}

impl LocalQueryResultV10 {
    /// Serializes one bounded exact-ID V10 result followed by LF.
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

/// Builds one exact-ID result with the stable K1, compiler, and join neighborhood.
///
/// # Errors
///
/// Returns a strict snapshot, document, not-found, identity, or result-limit failure.
#[allow(clippy::too_many_lines)]
pub fn local_query_result_v10(
    semantic: &Value,
    manifest: &Value,
    snapshot_id: &str,
    requested_id: &str,
) -> Result<LocalQueryResultV10, QueryContractError> {
    if semantic.get("ontology_version").and_then(Value::as_str) != Some(R13_ONTOLOGY_VERSION)
        || semantic
            .pointer("/knowledge_graph/schema_version")
            .and_then(Value::as_str)
            != Some(R13_GRAPH_VERSION)
    {
        return Err(QueryContractError::InvalidSnapshot);
    }
    let mut compatible = semantic.clone();
    compatible["ontology_version"] = Value::String(K1_ONTOLOGY_VERSION.to_owned());
    compatible["knowledge_graph"]["schema_version"] = Value::String(K1_GRAPH_VERSION.to_owned());
    compatible["knowledge_graph"]["ontology_version"] =
        Value::String(K1_ONTOLOGY_VERSION.to_owned());
    let mut value = local_query_result_v6(&compatible, manifest, snapshot_id, requested_id)?
        .value()
        .clone();
    let graph = semantic
        .get("knowledge_graph")
        .and_then(Value::as_object)
        .ok_or(QueryContractError::InvalidSnapshot)?;
    let entities = id_value_map(graph, "entities")?;
    let relationships = graph
        .get("relationships")
        .and_then(Value::as_array)
        .ok_or(QueryContractError::InvalidSnapshot)?;
    let neighborhood_ids = join_neighborhood_ids(graph, requested_id)?;
    let linked_k1_relationships = linked_relationships(relationships, &neighborhood_ids, |kind| {
        matches!(
            kind,
            "HAS_SIGNATURE" | "HAS_PARAMETER" | "DECLARES_VALUE" | "HAS_BODY_FACT" | "CALLS"
        )
    });
    let linked_compiler_relationships =
        linked_relationships(relationships, &neighborhood_ids, |kind| {
            matches!(
                kind,
                "RESOLVES_TO" | "REFERENCES" | "IMPLEMENTS" | "TYPE_DEFINITION"
            )
        });
    let linked_composition_relationships =
        linked_relationships(relationships, &neighborhood_ids, |kind| {
            kind == R13_RELATIONSHIP_KIND
        });
    let all_relationships = linked_k1_relationships
        .iter()
        .chain(&linked_compiler_relationships)
        .chain(&linked_composition_relationships)
        .collect::<Vec<_>>();
    let mut endpoint_ids = neighborhood_ids;
    for relationship in &all_relationships {
        for field in ["source", "target"] {
            let identifier = relationship
                .get(field)
                .and_then(Value::as_str)
                .ok_or(QueryContractError::InvalidSnapshot)?;
            endpoint_ids.insert(identifier.to_owned());
        }
    }
    let mut linked_k1_entities = endpoint_ids
        .iter()
        .filter(|identifier| identifier.as_str() != requested_id)
        .filter_map(|identifier| entities.get(identifier.as_str()).copied())
        .filter(|entity| entity.get("kind").and_then(Value::as_str) != Some("compiler.symbol"))
        .cloned()
        .collect::<Vec<_>>();
    let mut linked_compiler_entities = endpoint_ids
        .iter()
        .filter(|identifier| identifier.as_str() != requested_id)
        .filter_map(|identifier| entities.get(identifier.as_str()).copied())
        .filter(|entity| entity.get("kind").and_then(Value::as_str) == Some("compiler.symbol"))
        .cloned()
        .collect::<Vec<_>>();
    sort_dedup_records(&mut linked_k1_entities)?;
    sort_dedup_records(&mut linked_compiler_entities)?;
    value["schema_version"] = Value::String(R13_QUERY_VERSION.to_owned());
    value["linked_k1_entities"] = Value::Array(linked_k1_entities);
    value["linked_k1_relationships"] = Value::Array(linked_k1_relationships);
    value["linked_compiler_entities"] = Value::Array(linked_compiler_entities);
    value["linked_compiler_relationships"] = Value::Array(linked_compiler_relationships);
    value["linked_composition_relationships"] = Value::Array(linked_composition_relationships);
    let related_subject_ids = value
        .get("linked_k1_entities")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .chain(
            value
                .get("linked_compiler_entities")
                .and_then(Value::as_array)
                .into_iter()
                .flatten(),
        )
        .chain(
            value
                .get("linked_k1_relationships")
                .and_then(Value::as_array)
                .into_iter()
                .flatten(),
        )
        .chain(
            value
                .get("linked_compiler_relationships")
                .and_then(Value::as_array)
                .into_iter()
                .flatten(),
        )
        .chain(
            value
                .get("linked_composition_relationships")
                .and_then(Value::as_array)
                .into_iter()
                .flatten(),
        )
        .filter_map(record_id)
        .map(str::to_owned)
        .chain(std::iter::once(requested_id.to_owned()))
        .collect::<BTreeSet<_>>();
    union_linked_claims_and_evidence(&mut value, graph, &related_subject_ids)?;
    let result = LocalQueryResultV10 { value };
    result.canonical_stdout()?;
    Ok(result)
}

#[derive(Clone, Debug)]
pub struct PortableGraphV6 {
    value: Value,
    canonical: Vec<u8>,
    sha256: R13Sha256,
}

impl PortableGraphV6 {
    /// Projects one validated V15 head and deterministic documentation manifest.
    ///
    /// # Errors
    ///
    /// Returns a strict binding, privacy, reference, or limit failure.
    pub fn from_validated_v15(
        semantic: &Value,
        head: &LocalSnapshotHead,
        documentation_manifest: &Value,
        sha256: R13Sha256,
    ) -> Result<Self, R13ContractError> {
        validate_stored_snapshot_semantic_v15(semantic, head)
            .map_err(|_| R13ContractError::InvalidSnapshot)?;
        if semantic.get("ontology_version").and_then(Value::as_str) != Some(R13_ONTOLOGY_VERSION) {
            return Err(R13ContractError::InvalidSnapshot);
        }
        validate_documentation_binding(documentation_manifest, head)?;
        let graph = semantic
            .get("knowledge_graph")
            .and_then(Value::as_object)
            .ok_or(R13ContractError::InvalidSnapshot)?;
        let (documents, document_statements) = portable_documents(documentation_manifest)?;
        let mut value = json!({
            "schema_version": R13_PORTABLE_GRAPH_VERSION,
            "repository": semantic.get("repository").cloned().ok_or(R13ContractError::InvalidSnapshot)?,
            "source_snapshot": {
                "schema_version": R13_SNAPSHOT_VERSION,
                "snapshot_id": head.snapshot_id.as_str(),
                "semantic_hash": {
                    "algorithm": head.semantic_hash.algorithm,
                    "value": head.semantic_hash.value
                }
            },
            "ontology_version": R13_ONTOLOGY_VERSION,
            "query_contract_version": R13_QUERY_VERSION,
            "projection": {
                "profile": "codenoesis.lossless-portable-projection/v6",
                "family_sha256": {}
            },
            "entities": graph.get("entities").cloned().ok_or(R13ContractError::InvalidSnapshot)?,
            "relationships": graph.get("relationships").cloned().ok_or(R13ContractError::InvalidSnapshot)?,
            "claims": graph.get("claims").cloned().ok_or(R13ContractError::InvalidSnapshot)?,
            "evidence": graph.get("evidence").cloned().ok_or(R13ContractError::InvalidSnapshot)?,
            "diagnostics": graph.get("diagnostics").cloned().ok_or(R13ContractError::InvalidSnapshot)?,
            "coverage_gaps": graph.get("coverage").cloned().ok_or(R13ContractError::InvalidSnapshot)?,
            "documents": documents,
            "document_statements": document_statements
        });
        value["projection"]["family_sha256"] = family_digests(&value, sha256)?;
        Self::from_generated_value(value, sha256)
    }

    /// Strictly reimports one canonical LF-terminated `PortableGraphV6`.
    ///
    /// # Errors
    ///
    /// Returns the first decode, schema, identity, reference, privacy, or limit failure.
    pub fn from_canonical_file(bytes: &[u8], sha256: R13Sha256) -> Result<Self, R13ContractError> {
        enforce_portable_size(bytes.len())?;
        let value = serde_json::from_slice::<Value>(bytes)
            .map_err(|_| R13ContractError::InvalidProjection)?;
        validate_portable_value(&value, sha256)?;
        let canonical = serde_json::to_vec(&value).map_err(|_| R13ContractError::Internal)?;
        let mut expected = canonical.clone();
        expected.push(b'\n');
        if expected != bytes {
            return Err(R13ContractError::Noncanonical {
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

    fn from_generated_value(value: Value, sha256: R13Sha256) -> Result<Self, R13ContractError> {
        validate_portable_value(&value, sha256)?;
        let canonical = serde_json::to_vec(&value).map_err(|_| R13ContractError::Internal)?;
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
pub struct LocalExplorerManifestV6 {
    value: Value,
}

impl LocalExplorerManifestV6 {
    /// Builds the offline V6 explorer manifest bound to exact graph and viewer bytes.
    ///
    /// # Errors
    ///
    /// Returns an integrity or unsafe-CSP failure.
    pub fn new(
        portable: &PortableGraphV6,
        viewer_bytes: &[u8],
        expected_viewer_sha256: &str,
        content_security_policy: &str,
        sha256: R13Sha256,
    ) -> Result<Self, R13ContractError> {
        if sha256(viewer_bytes) != expected_viewer_sha256
            || content_security_policy.contains("http:")
            || content_security_policy.contains("https:")
            || content_security_policy.contains("unsafe-inline")
            || content_security_policy.contains("unsafe-eval")
        {
            return Err(R13ContractError::AssetIntegrityMismatch);
        }
        Ok(Self {
            value: json!({
                "schema_version": R13_LOCAL_EXPLORER_VERSION,
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
                    "profile": R13_EXPLORER_SECURITY_PROFILE,
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
                    "text_search_results": MAX_R13_TEXT_SEARCH_RESULTS,
                    "traversal_depth_default": R13_TRAVERSAL_DEPTH_DEFAULT,
                    "traversal_depth_maximum": MAX_R13_TRAVERSAL_DEPTH
                }
            }),
        })
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Serializes one `LocalExplorerManifestV6` followed by LF.
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
pub enum R13ContractError {
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

impl Display for R13ContractError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidSnapshot => "invalid R13 snapshot",
            Self::UnsupportedSnapshotSchema(_) => "unsupported R13 snapshot schema",
            Self::UnsupportedPortableGraphSchema(_) => "unsupported portable graph schema",
            Self::Noncanonical { .. } => "noncanonical portable graph",
            Self::IdentityConflict { .. } => "portable graph identity conflict",
            Self::ReferenceMismatch { .. } => "portable graph reference mismatch",
            Self::LimitExceeded { .. } => "portable graph limit exceeded",
            Self::UnsafePayload { .. } => "unsafe portable payload",
            Self::AssetIntegrityMismatch => "explorer asset integrity mismatch",
            Self::InvalidProjection => "invalid portable graph projection",
            Self::Internal => "internal R13 contract failure",
        })
    }
}

impl Error for R13ContractError {}

fn extraction_chunks_v12(
    callable: &Value,
    compiler: &Value,
    knowledge: &CallableScipKnowledge,
) -> Result<Vec<Value>, RepositorySnapshotV15Error> {
    let mut callable_chunks = callable
        .as_array()
        .cloned()
        .ok_or(RepositorySnapshotV15Error::ContractInvalid)?;
    let mut compiler_chunks = compiler
        .as_array()
        .cloned()
        .ok_or(RepositorySnapshotV15Error::ContractInvalid)?;
    if callable_chunks.len() != compiler_chunks.len() {
        return Err(RepositorySnapshotV15Error::ContractInvalid);
    }
    for chunk in &mut callable_chunks {
        let subject = chunk
            .get("subject")
            .cloned()
            .ok_or(RepositorySnapshotV15Error::ContractInvalid)?;
        let compiler_index = compiler_chunks
            .iter()
            .position(|candidate| candidate.get("subject") == Some(&subject))
            .ok_or(RepositorySnapshotV15Error::ContractInvalid)?;
        let compiler_chunk = compiler_chunks.remove(compiler_index);
        if chunk.get("repository_identity") != compiler_chunk.get("repository_identity") {
            return Err(RepositorySnapshotV15Error::ContractInvalid);
        }
        let object = chunk
            .as_object_mut()
            .ok_or(RepositorySnapshotV15Error::ContractInvalid)?;
        object.insert(
            "schema_version".to_owned(),
            Value::String(R13_EXTRACTION_CHUNK_VERSION.to_owned()),
        );
        object.insert(
            "ontology_version".to_owned(),
            Value::String(R13_ONTOLOGY_VERSION.to_owned()),
        );
        object.remove("semantic_hash");
        let compiler_object = compiler_chunk
            .as_object()
            .ok_or(RepositorySnapshotV15Error::ContractInvalid)?;
        for family in GRAPH_FAMILIES {
            strict_union_family(object, compiler_object, family)?;
        }
        let source_chunk = subject.get("kind").and_then(Value::as_str) == Some("rust_source");
        if source_chunk {
            object.insert(
                "compiler_index_profile".to_owned(),
                Value::String(R7_COMPILER_INDEX_PROFILE.to_owned()),
            );
        }
        let source_ids = object
            .get("entities")
            .and_then(Value::as_array)
            .ok_or(RepositorySnapshotV15Error::ContractInvalid)?
            .iter()
            .filter_map(record_id)
            .collect::<BTreeSet<_>>();
        let joins = knowledge
            .index
            .joins
            .iter()
            .filter(|join| source_ids.contains(join.source_callable_id.as_str()))
            .collect::<Vec<_>>();
        if !joins.is_empty() {
            object.insert(
                "callable_scip_composition_profile".to_owned(),
                Value::String(R13_COMPOSITION_VERSION.to_owned()),
            );
            merge_values(
                object,
                "relationships",
                joins.iter().map(|join| join_relationship_value(join)),
            )?;
            merge_values(
                object,
                "claims",
                joins.iter().map(|join| join_claim_value(join, knowledge)),
            )?;
        }
        insert_semantic_hash(chunk, EXTRACTION_V12_HASH_DOMAIN)?;
    }
    if !compiler_chunks.is_empty() {
        return Err(RepositorySnapshotV15Error::ContractInvalid);
    }
    Ok(callable_chunks)
}

fn knowledge_graph_v12(
    callable: &Value,
    compiler: &Value,
    knowledge: &CallableScipKnowledge,
) -> Result<Value, RepositorySnapshotV15Error> {
    let mut value = callable.clone();
    let object = value
        .as_object_mut()
        .ok_or(RepositorySnapshotV15Error::ContractInvalid)?;
    let compiler_object = compiler
        .as_object()
        .ok_or(RepositorySnapshotV15Error::ContractInvalid)?;
    for field in [
        "repository",
        "workspace",
        "manifest_index",
        "rust_semantic_index",
        "framework_declaration_index",
    ] {
        if object.get(field) != compiler_object.get(field) {
            return Err(RepositorySnapshotV15Error::ContractInvalid);
        }
    }
    for family in GRAPH_FAMILIES {
        strict_union_family(object, compiler_object, family)?;
    }
    object.insert(
        "schema_version".to_owned(),
        Value::String(R13_GRAPH_VERSION.to_owned()),
    );
    object.insert(
        "ontology_version".to_owned(),
        Value::String(R13_ONTOLOGY_VERSION.to_owned()),
    );
    object.insert(
        "extractor_versions".to_owned(),
        Value::Array(graph_extractor_versions()),
    );
    object.insert(
        "compiler_index".to_owned(),
        compiler_object
            .get("compiler_index")
            .cloned()
            .ok_or(RepositorySnapshotV15Error::ContractInvalid)?,
    );
    object.insert(
        "callable_compiler_join_index".to_owned(),
        json!({
            "schema_version": R13_INDEX_VERSION,
            "relationship_kind": R13_RELATIONSHIP_KIND,
            "joins": knowledge.index.joins.iter().map(join_index_value).collect::<Vec<_>>()
        }),
    );
    merge_values(
        object,
        "relationships",
        knowledge.index.joins.iter().map(join_relationship_value),
    )?;
    merge_values(
        object,
        "claims",
        knowledge
            .index
            .joins
            .iter()
            .map(|join| join_claim_value(join, knowledge)),
    )?;
    object.remove("semantic_hash");
    validate_composed_graph(object, knowledge)?;
    insert_semantic_hash(&mut value, GRAPH_V12_HASH_DOMAIN)?;
    Ok(value)
}

fn validate_composed_graph(
    graph: &Map<String, Value>,
    knowledge: &CallableScipKnowledge,
) -> Result<(), RepositorySnapshotV15Error> {
    let entities = family_id_set(graph, "entities")?;
    let relationships = family_id_map(graph, "relationships")?;
    let claims = family_id_map(graph, "claims")?;
    let evidence = family_id_set(graph, "evidence")?;
    for join in &knowledge.index.joins {
        if !entities.contains(&join.source_callable_id)
            || !entities.contains(&join.signature_id)
            || !entities.contains(&join.compiler_symbol_id)
        {
            return Err(RepositorySnapshotV15Error::ContractInvalid);
        }
        let relationship = relationships
            .get(join.relationship_id.as_str())
            .ok_or(RepositorySnapshotV15Error::ContractInvalid)?;
        if relationship.get("kind").and_then(Value::as_str) != Some(R13_RELATIONSHIP_KIND)
            || relationship.get("source").and_then(Value::as_str)
                != Some(join.source_callable_id.as_str())
            || relationship.get("target").and_then(Value::as_str)
                != Some(join.compiler_symbol_id.as_str())
            || relationship.get("evidence_ids") != Some(&json!(join.evidence_ids))
            || join
                .evidence_ids
                .iter()
                .any(|identifier| !evidence.contains(identifier))
            || !claims.values().any(|claim| {
                claim.get("subject_id").and_then(Value::as_str)
                    == Some(join.relationship_id.as_str())
                    && claim.get("evidence_ids") == Some(&json!(join.evidence_ids))
            })
        {
            return Err(RepositorySnapshotV15Error::ContractInvalid);
        }
    }
    Ok(())
}

fn strict_union_family(
    target: &mut Map<String, Value>,
    source: &Map<String, Value>,
    family: &'static str,
) -> Result<(), RepositorySnapshotV15Error> {
    let additions = source
        .get(family)
        .and_then(Value::as_array)
        .ok_or(RepositorySnapshotV15Error::ContractInvalid)?
        .iter()
        .cloned();
    merge_values(target, family, additions)
}

fn merge_values(
    object: &mut Map<String, Value>,
    family: &'static str,
    additions: impl IntoIterator<Item = Value>,
) -> Result<(), RepositorySnapshotV15Error> {
    let values = object
        .get_mut(family)
        .and_then(Value::as_array_mut)
        .ok_or(RepositorySnapshotV15Error::ContractInvalid)?;
    let mut union = BTreeMap::<String, Value>::new();
    for value in values.drain(..).chain(additions) {
        let identifier = record_id(&value)
            .ok_or(RepositorySnapshotV15Error::ContractInvalid)?
            .to_owned();
        if let Some(existing) = union.insert(identifier, value.clone())
            && existing != value
        {
            return Err(RepositorySnapshotV15Error::ContractInvalid);
        }
    }
    *values = union.into_values().collect();
    Ok(())
}

fn join_relationship_value(join: &CallableCompilerJoin) -> Value {
    json!({
        "id": join.relationship_id,
        "kind": R13_RELATIONSHIP_KIND,
        "source": join.source_callable_id,
        "target": join.compiler_symbol_id,
        "evidence_ids": join.evidence_ids,
        "provenance": "validated_scip_v0.9.0",
        "endpoint_binding": "unique"
    })
}

fn join_claim_value(join: &CallableCompilerJoin, knowledge: &CallableScipKnowledge) -> Value {
    let claim = knowledge
        .claims
        .iter()
        .find(|claim| claim.subject_id == join.relationship_id)
        .expect("validated R13 join has one claim");
    super::s4::claim_value(claim)
}

fn join_index_value(join: &CallableCompilerJoin) -> Value {
    json!({
        "source_callable_id": join.source_callable_id,
        "signature_id": join.signature_id,
        "compiler_symbol_id": join.compiler_symbol_id,
        "relationship_id": join.relationship_id
    })
}

fn snapshot_extractor_versions() -> Vec<Value> {
    std::iter::once("codenoesis.inventory-classifier/s1-v1")
        .chain(graph_extractor_version_strings())
        .map(|value| Value::String(value.to_owned()))
        .collect()
}

fn graph_extractor_versions() -> Vec<Value> {
    graph_extractor_version_strings()
        .map(|value| Value::String(value.to_owned()))
        .collect()
}

fn graph_extractor_version_strings() -> impl Iterator<Item = &'static str> {
    [
        "codenoesis.rust-tree-sitter/s4-v1",
        "codenoesis.rust-workspace/s4-r3-v1",
        "codenoesis.cargo-manifest/s4-r4-v1",
        R5_RUST_SEMANTIC_EXTRACTOR_VERSION,
        R6_FRAMEWORK_EXTRACTOR_VERSION,
        R7_COMPILER_EXTRACTOR_VERSION,
        K1_EXTRACTOR_VERSION,
        R13_COMPOSITION_VERSION,
    ]
    .into_iter()
}

fn insert_semantic_hash(
    value: &mut Value,
    domain: &[u8],
) -> Result<(), RepositorySnapshotV15Error> {
    let object = value
        .as_object_mut()
        .ok_or(RepositorySnapshotV15Error::ContractInvalid)?;
    object.remove("semantic_hash");
    let hash = semantic_hash(domain, value);
    value
        .as_object_mut()
        .ok_or(RepositorySnapshotV15Error::ContractInvalid)?
        .insert(
            "semantic_hash".to_owned(),
            json!({"algorithm": "blake3-256", "value": hash}),
        );
    Ok(())
}

fn family_id_set(
    graph: &Map<String, Value>,
    family: &'static str,
) -> Result<BTreeSet<String>, RepositorySnapshotV15Error> {
    Ok(family_id_map(graph, family)?
        .into_keys()
        .map(str::to_owned)
        .collect())
}

fn family_id_map<'a>(
    graph: &'a Map<String, Value>,
    family: &'static str,
) -> Result<BTreeMap<&'a str, &'a Value>, RepositorySnapshotV15Error> {
    let mut values = BTreeMap::new();
    let mut previous = None;
    for value in graph
        .get(family)
        .and_then(Value::as_array)
        .ok_or(RepositorySnapshotV15Error::ContractInvalid)?
    {
        let identifier = record_id(value).ok_or(RepositorySnapshotV15Error::ContractInvalid)?;
        if previous.is_some_and(|previous| previous >= identifier)
            || values.insert(identifier, value).is_some()
        {
            return Err(RepositorySnapshotV15Error::ContractInvalid);
        }
        previous = Some(identifier);
    }
    Ok(values)
}

fn map_k1_snapshot_error(error: RepositorySnapshotV11Error) -> RepositorySnapshotV15Error {
    match error {
        RepositorySnapshotV11Error::Serialization(error) => {
            RepositorySnapshotV15Error::Serialization(error)
        }
        RepositorySnapshotV11Error::LimitExceeded(error) => {
            RepositorySnapshotV15Error::LimitExceeded(error)
        }
        RepositorySnapshotV11Error::ContractInvalid => RepositorySnapshotV15Error::ContractInvalid,
        RepositorySnapshotV11Error::OutputLengthOverflow => {
            RepositorySnapshotV15Error::OutputLengthOverflow
        }
    }
}

fn map_r7_snapshot_error(error: RepositorySnapshotV10Error) -> RepositorySnapshotV15Error {
    match error {
        RepositorySnapshotV10Error::Serialization(error) => {
            RepositorySnapshotV15Error::Serialization(error)
        }
        RepositorySnapshotV10Error::LimitExceeded(error) => {
            RepositorySnapshotV15Error::LimitExceeded(error)
        }
        RepositorySnapshotV10Error::ContractInvalid => RepositorySnapshotV15Error::ContractInvalid,
        RepositorySnapshotV10Error::OutputLengthOverflow => {
            RepositorySnapshotV15Error::OutputLengthOverflow
        }
    }
}

fn linked_relationships(
    relationships: &[Value],
    requested_ids: &BTreeSet<String>,
    kind_filter: impl Fn(&str) -> bool,
) -> Vec<Value> {
    let mut linked = relationships
        .iter()
        .filter(|relationship| {
            ["id", "source", "target"].iter().any(|field| {
                relationship
                    .get(*field)
                    .and_then(Value::as_str)
                    .is_some_and(|identifier| requested_ids.contains(identifier))
            })
        })
        .filter(|relationship| {
            relationship
                .get("kind")
                .and_then(Value::as_str)
                .is_some_and(&kind_filter)
        })
        .cloned()
        .collect::<Vec<_>>();
    linked.sort_by(|left, right| record_id(left).cmp(&record_id(right)));
    linked.dedup_by(|left, right| record_id(left) == record_id(right));
    linked
}

fn join_neighborhood_ids(
    graph: &Map<String, Value>,
    requested_id: &str,
) -> Result<BTreeSet<String>, QueryContractError> {
    let joins = graph
        .get("callable_compiler_join_index")
        .and_then(|value| value.get("joins"))
        .and_then(Value::as_array)
        .ok_or(QueryContractError::InvalidSnapshot)?;
    let mut identifiers = BTreeSet::from([requested_id.to_owned()]);
    for join in joins {
        let values = [
            "source_callable_id",
            "signature_id",
            "compiler_symbol_id",
            "relationship_id",
        ]
        .map(|field| {
            join.get(field)
                .and_then(Value::as_str)
                .ok_or(QueryContractError::InvalidSnapshot)
        });
        let values = values.into_iter().collect::<Result<Vec<_>, _>>()?;
        if values.contains(&requested_id) {
            identifiers.extend(values.into_iter().map(str::to_owned));
        }
    }
    Ok(identifiers)
}

fn union_linked_claims_and_evidence(
    value: &mut Value,
    graph: &Map<String, Value>,
    subject_ids: &BTreeSet<String>,
) -> Result<(), QueryContractError> {
    let additions = graph
        .get("claims")
        .and_then(Value::as_array)
        .ok_or(QueryContractError::InvalidSnapshot)?
        .iter()
        .filter(|claim| {
            claim
                .get("subject_id")
                .and_then(Value::as_str)
                .is_some_and(|identifier| subject_ids.contains(identifier))
        })
        .cloned()
        .collect::<Vec<_>>();
    union_query_family(value, "claims", &additions)?;
    let evidence_ids = value
        .get("claims")
        .and_then(Value::as_array)
        .ok_or(QueryContractError::InvalidSnapshot)?
        .iter()
        .flat_map(|claim| {
            claim
                .get("evidence_ids")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .map(|identifier| {
            identifier
                .as_str()
                .ok_or(QueryContractError::InvalidSnapshot)
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let evidence = id_value_map(graph, "evidence")?;
    let additions = evidence_ids
        .into_iter()
        .map(|identifier| {
            evidence
                .get(identifier)
                .copied()
                .cloned()
                .ok_or(QueryContractError::InvalidSnapshot)
        })
        .collect::<Result<Vec<_>, _>>()?;
    union_query_family(value, "evidence", &additions)
}

fn union_query_family(
    value: &mut Value,
    field: &'static str,
    additions: &[Value],
) -> Result<(), QueryContractError> {
    let records = value
        .get_mut(field)
        .and_then(Value::as_array_mut)
        .ok_or(QueryContractError::InvalidSnapshot)?;
    let mut union = BTreeMap::new();
    for record in records.iter().chain(additions) {
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

fn id_value_map<'a>(
    graph: &'a Map<String, Value>,
    family: &'static str,
) -> Result<BTreeMap<&'a str, &'a Value>, QueryContractError> {
    graph
        .get(family)
        .and_then(Value::as_array)
        .ok_or(QueryContractError::InvalidSnapshot)?
        .iter()
        .map(|record| {
            record_id(record)
                .map(|identifier| (identifier, record))
                .ok_or(QueryContractError::InvalidSnapshot)
        })
        .collect()
}

fn sort_dedup_records(values: &mut [Value]) -> Result<(), QueryContractError> {
    values.sort_by(|left, right| record_id(left).cmp(&record_id(right)));
    for pair in values.windows(2) {
        if record_id(&pair[0]).is_none() || record_id(&pair[0]) == record_id(&pair[1]) {
            return Err(QueryContractError::InvalidSnapshot);
        }
    }
    Ok(())
}

fn validate_documentation_binding(
    manifest: &Value,
    head: &LocalSnapshotHead,
) -> Result<(), R13ContractError> {
    if manifest.get("schema_version").and_then(Value::as_str)
        != Some("codenoesis.documentation-manifest/v1")
        || manifest.get("repository_identity").and_then(Value::as_str)
            != Some(head.repository_identity.as_str())
        || manifest.get("snapshot_id").and_then(Value::as_str) != Some(head.snapshot_id.as_str())
        || manifest
            .pointer("/snapshot_semantic_hash/value")
            .and_then(Value::as_str)
            != Some(head.semantic_hash.value.as_str())
    {
        return Err(R13ContractError::InvalidSnapshot);
    }
    Ok(())
}

fn portable_documents(manifest: &Value) -> Result<(Vec<Value>, Vec<Value>), R13ContractError> {
    let source = manifest
        .get("documents")
        .and_then(Value::as_array)
        .ok_or(R13ContractError::InvalidSnapshot)?;
    let mut documents = Vec::with_capacity(source.len());
    let mut statements = Vec::new();
    for document in source {
        let mut record = document
            .as_object()
            .cloned()
            .ok_or(R13ContractError::InvalidSnapshot)?;
        let document_id = record
            .get("document_id")
            .and_then(Value::as_str)
            .ok_or(R13ContractError::InvalidSnapshot)?
            .to_owned();
        let document_statements = record
            .remove("statements")
            .and_then(|value| value.as_array().cloned())
            .ok_or(R13ContractError::InvalidSnapshot)?;
        documents.push(Value::Object(record));
        for statement in document_statements {
            let mut statement = statement
                .as_object()
                .cloned()
                .ok_or(R13ContractError::InvalidSnapshot)?;
            if statement
                .insert("document_id".to_owned(), Value::String(document_id.clone()))
                .is_some()
            {
                return Err(R13ContractError::InvalidSnapshot);
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

fn family_digests(value: &Value, sha256: R13Sha256) -> Result<Value, R13ContractError> {
    let mut digests = Map::new();
    for (family, _) in PORTABLE_FAMILIES {
        let bytes = serde_json::to_vec(
            value
                .get(family)
                .ok_or(R13ContractError::InvalidProjection)?,
        )
        .map_err(|_| R13ContractError::Internal)?;
        digests.insert(family.to_owned(), Value::String(sha256(&bytes)));
    }
    Ok(Value::Object(digests))
}

#[allow(clippy::too_many_lines)]
fn validate_portable_value(value: &Value, sha256: R13Sha256) -> Result<(), R13ContractError> {
    ensure_nesting(value, 0)?;
    validate_private_fields(value)?;
    let object = value
        .as_object()
        .ok_or(R13ContractError::InvalidProjection)?;
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
        return Err(R13ContractError::InvalidProjection);
    }
    let schema = object
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or(R13ContractError::InvalidProjection)?;
    if schema != R13_PORTABLE_GRAPH_VERSION {
        return Err(R13ContractError::UnsupportedPortableGraphSchema(bounded(
            schema, 256,
        )));
    }
    if object.get("ontology_version").and_then(Value::as_str) != Some(R13_ONTOLOGY_VERSION)
        || object.get("query_contract_version").and_then(Value::as_str) != Some(R13_QUERY_VERSION)
        || value
            .pointer("/source_snapshot/schema_version")
            .and_then(Value::as_str)
            != Some(R13_SNAPSHOT_VERSION)
        || value.pointer("/projection/profile").and_then(Value::as_str)
            != Some("codenoesis.lossless-portable-projection/v6")
    {
        return Err(R13ContractError::InvalidProjection);
    }
    let repository_identity = value
        .pointer("/repository/identity")
        .and_then(Value::as_str)
        .filter(|value| value.starts_with("urn:codenoesis:"))
        .ok_or(R13ContractError::InvalidProjection)?;
    let entity_ids = validate_family(object, "entities", "id")?;
    let relationship_ids = validate_family(object, "relationships", "id")?;
    let claim_ids = validate_family(object, "claims", "id")?;
    let evidence_ids = validate_family(object, "evidence", "id")?;
    let diagnostic_ids = validate_family(object, "diagnostics", "id")?;
    let coverage_ids = validate_family(object, "coverage_gaps", "id")?;
    let document_ids = validate_family(object, "documents", "document_id")?;
    let statement_ids = validate_family(object, "document_statements", "statement_id")?;
    let mut subject_ids = BTreeSet::from([repository_identity.to_owned()]);
    for ids in [
        &entity_ids,
        &relationship_ids,
        &claim_ids,
        &evidence_ids,
        &diagnostic_ids,
        &coverage_ids,
        &document_ids,
        &statement_ids,
    ] {
        subject_ids.extend(ids.iter().cloned());
    }
    for relationship in object["relationships"]
        .as_array()
        .ok_or(R13ContractError::InvalidProjection)?
    {
        validate_reference(relationship, "source", &entity_ids, "relationships")?;
        validate_reference(relationship, "target", &entity_ids, "relationships")?;
        validate_reference_array_if_present(
            relationship,
            "evidence_ids",
            &evidence_ids,
            "relationships",
        )?;
    }
    for claim in object["claims"]
        .as_array()
        .ok_or(R13ContractError::InvalidProjection)?
    {
        let subject_id = claim
            .get("subject_id")
            .and_then(Value::as_str)
            .ok_or(R13ContractError::InvalidProjection)?;
        let valid = match claim.get("subject_kind").and_then(Value::as_str) {
            Some("entity") => entity_ids.contains(subject_id),
            Some("relationship") => relationship_ids.contains(subject_id),
            _ => false,
        };
        if !valid {
            return Err(R13ContractError::ReferenceMismatch {
                family: "claims",
                id: subject_id.to_owned(),
            });
        }
        validate_reference_array_if_present(claim, "evidence_ids", &evidence_ids, "claims")?;
    }
    for family in ["entities", "diagnostics", "coverage_gaps"] {
        for record in object[family]
            .as_array()
            .ok_or(R13ContractError::InvalidProjection)?
        {
            validate_reference_array_if_present(record, "evidence_ids", &evidence_ids, family)?;
        }
    }
    for evidence in object["evidence"]
        .as_array()
        .ok_or(R13ContractError::InvalidProjection)?
    {
        validate_evidence_path(evidence)?;
    }
    for document in object["documents"]
        .as_array()
        .ok_or(R13ContractError::InvalidProjection)?
    {
        validate_reference(document, "subject_id", &subject_ids, "documents")?;
        if let Some(path) = document.get("path").and_then(Value::as_str)
            && !safe_relative_path(path)
        {
            return Err(R13ContractError::UnsafePayload {
                reason: "unsafe_document_path",
            });
        }
    }
    for statement in object["document_statements"]
        .as_array()
        .ok_or(R13ContractError::InvalidProjection)?
    {
        validate_reference(
            statement,
            "document_id",
            &document_ids,
            "document_statements",
        )?;
        validate_reference_array_if_present(
            statement,
            "subject_ids",
            &subject_ids,
            "document_statements",
        )?;
        validate_reference_array_if_present(
            statement,
            "evidence_ids",
            &evidence_ids,
            "document_statements",
        )?;
        validate_reference_array_if_present(
            statement,
            "coverage_gap_ids",
            &coverage_ids,
            "document_statements",
        )?;
    }
    validate_callable_scip_portable_links(object)?;
    if value.pointer("/projection/family_sha256") != Some(&family_digests(value, sha256)?) {
        return Err(R13ContractError::InvalidProjection);
    }
    Ok(())
}

fn validate_callable_scip_portable_links(
    object: &Map<String, Value>,
) -> Result<(), R13ContractError> {
    let entities = object["entities"]
        .as_array()
        .ok_or(R13ContractError::InvalidProjection)?;
    let relationships = object["relationships"]
        .as_array()
        .ok_or(R13ContractError::InvalidProjection)?;
    let claims = object["claims"]
        .as_array()
        .ok_or(R13ContractError::InvalidProjection)?;
    let statements = object["document_statements"]
        .as_array()
        .ok_or(R13ContractError::InvalidProjection)?;
    for relationship in relationships.iter().filter(|relationship| {
        relationship.get("kind").and_then(Value::as_str) == Some(R13_RELATIONSHIP_KIND)
    }) {
        let relationship_id = relationship
            .get("id")
            .and_then(Value::as_str)
            .ok_or(R13ContractError::InvalidProjection)?;
        let source_id = relationship
            .get("source")
            .and_then(Value::as_str)
            .ok_or(R13ContractError::InvalidProjection)?;
        let target_id = relationship
            .get("target")
            .and_then(Value::as_str)
            .ok_or(R13ContractError::InvalidProjection)?;
        let source_kind = entity_kind(entities, source_id)?;
        let target_kind = entity_kind(entities, target_id)?;
        let evidence_ids = required_string_array(relationship, "evidence_ids")?;
        if !matches!(source_kind, "rust.function" | "rust.method")
            || target_kind != "compiler.symbol"
            || evidence_ids.len() != 2
            || evidence_ids[0] >= evidence_ids[1]
            || relationships
                .iter()
                .filter(|candidate| {
                    candidate.get("kind").and_then(Value::as_str) == Some(R13_RELATIONSHIP_KIND)
                        && candidate.get("source").and_then(Value::as_str) == Some(source_id)
                })
                .count()
                != 1
        {
            return Err(R13ContractError::InvalidProjection);
        }
        let signatures = relationships
            .iter()
            .filter(|candidate| {
                candidate.get("kind").and_then(Value::as_str) == Some("HAS_SIGNATURE")
                    && candidate.get("source").and_then(Value::as_str) == Some(source_id)
            })
            .collect::<Vec<_>>();
        if signatures.len() != 1 {
            return Err(R13ContractError::InvalidProjection);
        }
        let signature_id = signatures[0]
            .get("target")
            .and_then(Value::as_str)
            .ok_or(R13ContractError::InvalidProjection)?;
        if entity_kind(entities, signature_id)? != "rust.callable_signature" {
            return Err(R13ContractError::InvalidProjection);
        }
        let matching_claims = claims
            .iter()
            .filter(|claim| {
                claim.get("subject_kind").and_then(Value::as_str) == Some("relationship")
                    && claim.get("subject_id").and_then(Value::as_str) == Some(relationship_id)
            })
            .collect::<Vec<_>>();
        if matching_claims.len() != 1
            || matching_claims[0].get("state").and_then(Value::as_str) != Some("deterministic_fact")
            || matching_claims[0].get("evidence_ids") != relationship.get("evidence_ids")
        {
            return Err(R13ContractError::InvalidProjection);
        }
        let matching_statements = statements
            .iter()
            .filter(|statement| {
                statement
                    .get("subject_ids")
                    .and_then(Value::as_array)
                    .is_some_and(|subjects| {
                        subjects
                            .iter()
                            .any(|subject| subject.as_str() == Some(relationship_id))
                    })
            })
            .collect::<Vec<_>>();
        if matching_statements.len() != 1
            || matching_statements[0]
                .get("truth_state")
                .and_then(Value::as_str)
                != Some("deterministic_fact")
            || matching_statements[0].get("evidence_ids") != relationship.get("evidence_ids")
        {
            return Err(R13ContractError::InvalidProjection);
        }
    }
    Ok(())
}

fn entity_kind<'a>(entities: &'a [Value], identifier: &str) -> Result<&'a str, R13ContractError> {
    entities
        .iter()
        .find(|entity| entity.get("id").and_then(Value::as_str) == Some(identifier))
        .and_then(|entity| entity.get("kind"))
        .and_then(Value::as_str)
        .ok_or(R13ContractError::InvalidProjection)
}

fn required_string_array<'a>(
    value: &'a Value,
    field: &str,
) -> Result<Vec<&'a str>, R13ContractError> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or(R13ContractError::InvalidProjection)?
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|value| !value.is_empty())
                .ok_or(R13ContractError::InvalidProjection)
        })
        .collect()
}

fn validate_family(
    object: &Map<String, Value>,
    family: &'static str,
    id_field: &'static str,
) -> Result<BTreeSet<String>, R13ContractError> {
    let values = object
        .get(family)
        .and_then(Value::as_array)
        .ok_or(R13ContractError::InvalidProjection)?;
    let mut identifiers = BTreeSet::new();
    let mut previous = None;
    for value in values {
        let identifier = value
            .get(id_field)
            .and_then(Value::as_str)
            .filter(|identifier| !identifier.is_empty())
            .ok_or(R13ContractError::InvalidProjection)?;
        if previous.is_some_and(|previous| previous >= identifier) {
            return Err(R13ContractError::IdentityConflict {
                family,
                id: identifier.to_owned(),
            });
        }
        identifiers.insert(identifier.to_owned());
        previous = Some(identifier);
    }
    Ok(identifiers)
}

fn validate_reference(
    record: &Value,
    field: &'static str,
    identifiers: &BTreeSet<String>,
    family: &'static str,
) -> Result<(), R13ContractError> {
    let identifier = record
        .get(field)
        .and_then(Value::as_str)
        .ok_or(R13ContractError::InvalidProjection)?;
    if !identifiers.contains(identifier) {
        return Err(R13ContractError::ReferenceMismatch {
            family,
            id: identifier.to_owned(),
        });
    }
    Ok(())
}

fn validate_reference_array_if_present(
    record: &Value,
    field: &'static str,
    identifiers: &BTreeSet<String>,
    family: &'static str,
) -> Result<(), R13ContractError> {
    let Some(values) = record.get(field) else {
        return Ok(());
    };
    let values = values
        .as_array()
        .ok_or(R13ContractError::InvalidProjection)?;
    for identifier in values {
        let identifier = identifier
            .as_str()
            .ok_or(R13ContractError::InvalidProjection)?;
        if !identifiers.contains(identifier) {
            return Err(R13ContractError::ReferenceMismatch {
                family,
                id: identifier.to_owned(),
            });
        }
    }
    Ok(())
}

fn validate_evidence_path(evidence: &Value) -> Result<(), R13ContractError> {
    if let Some(path) = evidence.get("path").and_then(Value::as_str) {
        if !safe_relative_path(path) {
            return Err(R13ContractError::UnsafePayload {
                reason: "unsafe_evidence_path",
            });
        }
        return Ok(());
    }
    if evidence
        .get("artifact_sha256")
        .and_then(Value::as_str)
        .is_some_and(valid_sha256)
    {
        if let Some(path) = evidence.get("document_path").and_then(Value::as_str)
            && !safe_relative_path(path)
        {
            return Err(R13ContractError::UnsafePayload {
                reason: "unsafe_compiler_document_path",
            });
        }
        return Ok(());
    }
    Err(R13ContractError::InvalidProjection)
}

fn validate_private_fields(value: &Value) -> Result<(), R13ContractError> {
    match value {
        Value::Object(fields) => {
            for (field, nested) in fields {
                if matches!(
                    field.as_str(),
                    "body_text"
                        | "expression_text"
                        | "initializer_text"
                        | "source_contents"
                        | "source_snippet"
                        | "repository_root"
                        | "absolute_path"
                        | "raw_url"
                        | "credentials"
                        | "environment"
                        | "arguments"
                        | "telemetry"
                ) {
                    return Err(R13ContractError::UnsafePayload {
                        reason: "private_field",
                    });
                }
                validate_private_fields(nested)?;
            }
        }
        Value::Array(values) => {
            for nested in values {
                validate_private_fields(nested)?;
            }
        }
        Value::String(value) if value.contains("://") => {
            return Err(R13ContractError::UnsafePayload { reason: "raw_url" });
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    Ok(())
}

fn ensure_nesting(value: &Value, depth: u64) -> Result<(), R13ContractError> {
    if depth > MAX_R13_JSON_NESTING {
        return Err(R13ContractError::LimitExceeded {
            limit: "json_nesting",
            maximum: MAX_R13_JSON_NESTING,
            observed: depth,
        });
    }
    match value {
        Value::Array(values) => {
            for nested in values {
                ensure_nesting(nested, depth.saturating_add(1))?;
            }
        }
        Value::Object(values) => {
            for nested in values.values() {
                ensure_nesting(nested, depth.saturating_add(1))?;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    Ok(())
}

fn enforce_portable_size(length: usize) -> Result<(), R13ContractError> {
    let observed = u64::try_from(length).unwrap_or(u64::MAX);
    if observed > MAX_R13_PORTABLE_GRAPH_BYTES {
        return Err(R13ContractError::LimitExceeded {
            limit: "portable_graph_bytes",
            maximum: MAX_R13_PORTABLE_GRAPH_BYTES,
            observed,
        });
    }
    Ok(())
}

fn safe_relative_path(value: &str) -> bool {
    let path = Path::new(value);
    !value.is_empty()
        && value.len() <= 1_024
        && !path.is_absolute()
        && !value.contains('\\')
        && !value.chars().any(char::is_control)
        && value
            .split('/')
            .all(|component| !component.is_empty() && component.len() <= 255)
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn stored_snapshot_error(head: &LocalSnapshotHead, reason: &'static str) -> StorageError {
    StorageError::CorruptMetadata {
        component: StorageComponent::Head,
        reason,
        snapshot_id: Some(head.snapshot_id.as_str().to_owned()),
    }
}

fn record_id(value: &Value) -> Option<&str> {
    value.get("id").and_then(Value::as_str)
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn valid_sha256_or_zero(value: &str) -> String {
    if valid_sha256(value) {
        value.to_owned()
    } else {
        "0".repeat(64)
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

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::{
        MAX_R13_JSON_NESTING, MAX_R13_PORTABLE_GRAPH_BYTES, R13ContractError,
        enforce_portable_size, ensure_nesting,
    };

    #[test]
    fn pt_fr_exp_005_portable_size_has_exact_maximum_and_plus_one() {
        let maximum =
            usize::try_from(MAX_R13_PORTABLE_GRAPH_BYTES).expect("R13 portable limit fits usize");
        assert_eq!(enforce_portable_size(maximum), Ok(()));
        assert_eq!(
            enforce_portable_size(maximum + 1),
            Err(R13ContractError::LimitExceeded {
                limit: "portable_graph_bytes",
                maximum: MAX_R13_PORTABLE_GRAPH_BYTES,
                observed: MAX_R13_PORTABLE_GRAPH_BYTES + 1,
            })
        );
    }

    #[test]
    fn pt_fr_exp_005_json_nesting_has_exact_maximum_and_plus_one() {
        let mut maximum = Value::Null;
        for _ in 0..MAX_R13_JSON_NESTING {
            maximum = Value::Array(vec![maximum]);
        }
        assert_eq!(ensure_nesting(&maximum, 0), Ok(()));

        let plus_one = Value::Array(vec![maximum]);
        assert_eq!(
            ensure_nesting(&plus_one, 0),
            Err(R13ContractError::LimitExceeded {
                limit: "json_nesting",
                maximum: MAX_R13_JSON_NESTING,
                observed: MAX_R13_JSON_NESTING + 1,
            })
        );
    }
}
