use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::path::{Component, Path};

use codenoesis_domain::s1_boundaries::RepositoryBoundaryReport;
use codenoesis_domain::s4_k1::{CallableSemanticsError, K1_GRAPH_VERSION, K1_ONTOLOGY_VERSION};
use codenoesis_domain::s4_r3::R3_WORKSPACE_PROFILE;
use codenoesis_domain::s4_r4::R4_MANIFEST_PROFILE;
use codenoesis_domain::s4_r5::{R5_RUST_SEMANTIC_PROFILE, RustSemanticError};
use codenoesis_domain::s4_r6::{FrameworkError, R6_FRAMEWORK_PROFILE};
use codenoesis_domain::s4_r14::{
    ExpressionBindingEntity, ExpressionBindingError, ExpressionBindingKnowledge,
    ExpressionBindingRelationship, ExpressionCoverageGap, ExpressionEntityProperties,
};
pub use codenoesis_domain::s4_r14::{
    MAX_R14_ARGUMENTS_PER_CALL, MAX_R14_BINDINGS_AND_ARGUMENTS, MAX_R14_BINDINGS_PER_CALLABLE,
    MAX_R14_EXPRESSION_DEPTH, MAX_R14_EXPRESSIONS, MAX_R14_EXPRESSIONS_PER_CALLABLE,
    MAX_R14_NORMALIZED_SPELLING_BYTES, MAX_R14_RELATIONSHIPS, R14_CONFIGURATION_VERSION,
    R14_ERROR_VERSION, R14_EXTRACTION_CHUNK_VERSION, R14_EXTRACTION_CONTRACT_VERSION,
    R14_EXTRACTOR_VERSION, R14_GRAPH_VERSION, R14_INDEX_VERSION, R14_LOCAL_EXPLORER_VERSION,
    R14_ONTOLOGY_VERSION, R14_PIPELINE_VERSION, R14_PORTABLE_GRAPH_VERSION, R14_PROFILE,
    R14_QUERY_VERSION, R14_SEMANTIC_HASH_CONTRACT_VERSION, R14_SNAPSHOT_VERSION,
};
use codenoesis_domain::storage::{
    LocalSnapshotHead, PublicationCandidate, SNAPSHOT_SCHEMA_VERSION_V16, StorageComponent,
    StorageError,
};
use codenoesis_domain::{
    AcquisitionError, K1OutputCapacityProfile, LimitKind, RepositoryInventory,
};
use serde_json::{Map, Value, json};

use super::s1_boundaries::CodeNoesisErrorV9;
use super::s4::{MAX_QUERY_BYTES, QueryContractError, claim_subjects, claim_value, evidence_value};
use super::s4_k1::{RepositorySnapshotV11, RepositorySnapshotV11Error, local_query_result_v6};
use super::s4_r12::{RepositorySnapshotV14, RepositorySnapshotV14Error, local_query_result_v9};
use super::{
    LimitedVecWriter, PublicationCandidateError, SnapshotEnvelopeV1, publication_candidate,
    semantic_hash,
};

const CONFIGURATION_V13_HASH_DOMAIN: &[u8] = b"codenoesis.configuration.semantic.v13";
const SNAPSHOT_V16_HASH_DOMAIN: &[u8] = b"codenoesis.repository-snapshot.semantic.v16";
const EXTRACTION_V13_HASH_DOMAIN: &[u8] = b"codenoesis.extraction-chunk.semantic.v13";
const GRAPH_V13_HASH_DOMAIN: &[u8] = b"codenoesis.knowledge-graph.semantic.v13";
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

pub type R14Sha256 = fn(&[u8]) -> String;
pub const R14_PORTABLE_MARKER: &str = ".codenoesis-portable-graph-v7";
pub const R14_EXPLORER_MARKER: &str = ".codenoesis-local-explorer-v7";
pub const R14_EXPLORER_SECURITY_PROFILE: &str = "codenoesis.local-explorer-security/v7";
pub const MAX_R14_PORTABLE_GRAPH_BYTES: u64 = 268_435_456;
pub const MAX_R14_JSON_NESTING: u64 = 64;
pub const MAX_R14_TEXT_SEARCH_RESULTS: u64 = 100;
pub const R14_TRAVERSAL_DEPTH_DEFAULT: u64 = 1;
pub const MAX_R14_TRAVERSAL_DEPTH: u64 = 2;

#[derive(Clone, Debug)]
pub struct CodeNoesisErrorV21 {
    value: Value,
}

impl CodeNoesisErrorV21 {
    #[must_use]
    pub fn from_boundary_error(error: &CodeNoesisErrorV9) -> Self {
        let mut value = error.value().clone();
        value["schema_version"] = Value::String(R14_ERROR_VERSION.to_owned());
        Self { value }
    }

    #[must_use]
    pub fn invalid_profile(profile: &str) -> Self {
        Self::new(
            "input.invalid_rust_expression_profile",
            "input",
            "invalid rust expression profile",
            json!({"profile": bounded_nonempty(profile, 256, "missing")}),
        )
    }

    #[must_use]
    pub fn unsupported_composition(reason: &str) -> Self {
        Self::new(
            "input.unsupported_rust_expression_composition",
            "input",
            "unsupported rust expression profile composition",
            json!({
                "rust_callable_profile": "rust-callable-semantics-v1",
                "rust_expression_profile": R14_PROFILE,
                "reason": bounded_nonempty(reason, 128, "unsupported_composition")
            }),
        )
    }

    #[must_use]
    pub fn from_expression(error: &ExpressionBindingError) -> Self {
        match error {
            ExpressionBindingError::Source(CallableSemanticsError::Source(
                FrameworkError::Source(RustSemanticError::InvalidDeclaration {
                    path,
                    start_byte,
                    declaration_kind,
                }),
            )) => Self::new(
                "extraction.invalid_rust_source",
                "extraction",
                "invalid rust source",
                json!({
                    "path": bounded(path, 1024),
                    "start_byte": start_byte,
                    "syntax_kind": bounded(declaration_kind, 128)
                }),
            ),
            ExpressionBindingError::InvalidSyntax {
                path,
                start_byte,
                syntax_kind,
            } => Self::new(
                "extraction.invalid_expression_syntax",
                "extraction",
                "invalid rust expression syntax",
                json!({
                    "path": bounded(path, 1024),
                    "start_byte": start_byte,
                    "syntax_kind": bounded(syntax_kind, 128)
                }),
            ),
            ExpressionBindingError::IdentityConflict => Self::extraction(
                "extraction.expression_identity_conflict",
                "expression identity conflict",
            ),
            ExpressionBindingError::ParentInvalid => Self::extraction(
                "extraction.expression_parent_invalid",
                "expression parent is invalid",
            ),
            ExpressionBindingError::OperatorInvalid => Self::extraction(
                "extraction.expression_operator_invalid",
                "expression operator is invalid",
            ),
            ExpressionBindingError::RoleInvalid => Self::extraction(
                "extraction.expression_role_invalid",
                "expression role is invalid",
            ),
            ExpressionBindingError::ArgumentOrdinalInvalid => Self::extraction(
                "extraction.argument_ordinal_invalid",
                "call argument ordinal is invalid",
            ),
            ExpressionBindingError::PatternUnsupported => {
                Self::extraction("extraction.pattern_unsupported", "pattern is unsupported")
            }
            ExpressionBindingError::BindingScopeInvalid => Self::extraction(
                "extraction.binding_scope_invalid",
                "binding scope is invalid",
            ),
            ExpressionBindingError::BindingAmbiguous => Self::extraction(
                "extraction.binding_ambiguous",
                "binding resolution is ambiguous",
            ),
            ExpressionBindingError::AccessResolutionInvalid => Self::extraction(
                "extraction.access_resolution_invalid",
                "lexical access resolution is invalid",
            ),
            ExpressionBindingError::CallSiteEvidenceMismatch => Self::extraction(
                "extraction.call_site_evidence_mismatch",
                "call-site evidence does not match",
            ),
            ExpressionBindingError::IndexMismatch => Self::extraction(
                "extraction.expression_index_mismatch",
                "expression index does not match the graph",
            ),
            ExpressionBindingError::LimitExceeded {
                limit,
                maximum,
                observed,
            } => Self::new(
                "extraction.expression_limit_exceeded",
                "extraction",
                "expression extraction limit exceeded",
                json!({"limit": limit.as_str(), "maximum": maximum, "observed": observed}),
            ),
            ExpressionBindingError::Source(_)
            | ExpressionBindingError::CfgAlternatives(_)
            | ExpressionBindingError::ContractInvalid => Self::internal("expression_extraction"),
        }
    }

    #[must_use]
    pub fn from_contract(error: &R14ContractError) -> Self {
        match error {
            R14ContractError::LimitExceeded {
                limit,
                maximum,
                observed,
            } => Self::new(
                "export.limit_exceeded",
                "export",
                "portable graph limit exceeded",
                json!({"limit": limit, "maximum": maximum, "observed": observed}),
            ),
            R14ContractError::AssetIntegrityMismatch => Self::new(
                "explorer.asset_integrity_mismatch",
                "explorer",
                "local explorer asset integrity mismatch",
                json!({}),
            ),
            R14ContractError::InvalidSnapshot | R14ContractError::UnsupportedSnapshotSchema(_) => {
                Self::new(
                    "export.invalid_snapshot",
                    "export",
                    "invalid R14 source snapshot",
                    json!({}),
                )
            }
            R14ContractError::Internal => Self::internal("contract"),
            _ => Self::new(
                "export.invalid_portable_graph_v7",
                "export",
                "invalid portable graph v7",
                json!({}),
            ),
        }
    }

    #[must_use]
    pub fn from_explorer_contract(error: &R14ContractError) -> Self {
        match error {
            R14ContractError::LimitExceeded {
                limit,
                maximum,
                observed,
            } => Self::new(
                "explorer.limit_exceeded",
                "explorer",
                "local explorer input limit exceeded",
                json!({"limit": limit, "maximum": maximum, "observed": observed}),
            ),
            R14ContractError::AssetIntegrityMismatch => Self::new(
                "explorer.asset_integrity_mismatch",
                "explorer",
                "local explorer asset integrity mismatch",
                json!({}),
            ),
            R14ContractError::Internal => Self::internal("explorer"),
            _ => Self::new(
                "export.invalid_portable_graph_v7",
                "export",
                "invalid portable graph v7",
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
            "snapshot.invalid_v16",
            "snapshot",
            "invalid R14 snapshot",
            json!({}),
        )
    }

    #[must_use]
    pub fn invalid_query() -> Self {
        Self::new("query.invalid_v16", "query", "invalid R14 query", json!({}))
    }

    #[must_use]
    pub fn acquisition_limit(limit: &AcquisitionError) -> Self {
        match limit {
            AcquisitionError::LimitExceeded {
                limit,
                maximum,
                observed,
            } => Self::new(
                "acquisition.limit_exceeded",
                "acquisition",
                "repository acquisition limit exceeded",
                json!({"limit": limit.as_str(), "maximum": maximum, "observed": observed}),
            ),
            _ => Self::internal("acquisition"),
        }
    }

    #[must_use]
    pub fn internal(stage: &str) -> Self {
        Self::new(
            "internal.unexpected",
            "internal",
            "unexpected internal R14 failure",
            json!({"stage": bounded(stage, 128)}),
        )
    }

    fn extraction(code: &str, message: &str) -> Self {
        Self::new(code, "extraction", message, json!({}))
    }

    fn new(code: &str, stage: &str, message: &str, context: impl Into<Value>) -> Self {
        let context = context.into();
        Self {
            value: json!({
                "schema_version": R14_ERROR_VERSION,
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

    /// Serializes one `ErrorV21` followed by LF.
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
pub struct RepositorySnapshotV16 {
    value: Value,
    output_capacity_profile: K1OutputCapacityProfile,
}

#[derive(Debug)]
pub enum RepositorySnapshotV16Error {
    Serialization(serde_json::Error),
    LimitExceeded(AcquisitionError),
    ContractInvalid,
    OutputLengthOverflow,
}

impl Display for RepositorySnapshotV16Error {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Serialization(_) => "R14 snapshot serialization failed",
            Self::LimitExceeded(_) => "R14 snapshot output limit exceeded",
            Self::ContractInvalid => "R14 snapshot contract is invalid",
            Self::OutputLengthOverflow => "R14 snapshot output length overflowed",
        })
    }
}

impl Error for RepositorySnapshotV16Error {}

impl RepositorySnapshotV16 {
    /// Builds the additive R14 expression overlay over the exact K1 source-only lineage.
    ///
    /// # Errors
    ///
    /// Returns the first K1, expression, identity, serialization, or publication failure.
    pub fn from_inventory_and_expression_bindings(
        inventory: &RepositoryInventory,
        knowledge: &ExpressionBindingKnowledge,
        output_capacity_profile: K1OutputCapacityProfile,
        envelope: SnapshotEnvelopeV1,
    ) -> Result<Self, RepositorySnapshotV16Error> {
        Self::from_inventory_expression_bindings_and_boundaries(
            inventory,
            knowledge,
            None,
            output_capacity_profile,
            envelope,
        )
    }

    /// Builds R14 over either the historical K1 lineage or the additive R12 boundary lineage.
    ///
    /// # Errors
    ///
    /// Returns the first R12, expression, boundary, serialization, or publication failure.
    #[allow(clippy::too_many_lines)]
    pub fn from_inventory_expression_bindings_and_boundaries(
        inventory: &RepositoryInventory,
        knowledge: &ExpressionBindingKnowledge,
        boundaries: Option<&RepositoryBoundaryReport>,
        output_capacity_profile: K1OutputCapacityProfile,
        envelope: SnapshotEnvelopeV1,
    ) -> Result<Self, RepositorySnapshotV16Error> {
        knowledge
            .validate()
            .map_err(|_| RepositorySnapshotV16Error::ContractInvalid)?;
        if knowledge.callable_cfg_alternatives.is_none() && boundaries.is_some() {
            return Err(RepositorySnapshotV16Error::ContractInvalid);
        }
        let mut value = if let Some(composition) = &knowledge.callable_cfg_alternatives {
            RepositorySnapshotV14::from_inventory_callable_cfg_alternatives(
                inventory,
                composition,
                boundaries,
                envelope,
            )
            .map_err(map_r12_snapshot_error)?
            .value()
            .clone()
        } else {
            RepositorySnapshotV11::from_inventory_and_callable_semantics(
                inventory,
                &knowledge.callable,
                envelope,
            )
            .map_err(map_k1_snapshot_error)?
            .value()
            .clone()
        };
        let semantic = value
            .get_mut("semantic")
            .and_then(Value::as_object_mut)
            .ok_or(RepositorySnapshotV16Error::ContractInvalid)?;
        let capacity = match output_capacity_profile {
            K1OutputCapacityProfile::Standard | K1OutputCapacityProfile::LocalSnapshot256MV1 => {
                Value::Null
            }
            K1OutputCapacityProfile::LocalSnapshot64MV1 => {
                Value::String("local-snapshot-64m-v1".to_owned())
            }
        };
        let configuration_without_hash = if knowledge.callable_cfg_alternatives.is_some() {
            let mut configuration = semantic
                .get("configuration")
                .and_then(Value::as_object)
                .cloned()
                .ok_or(RepositorySnapshotV16Error::ContractInvalid)?;
            configuration.remove("semantic_hash");
            configuration.insert(
                "schema_version".to_owned(),
                Value::String(R14_CONFIGURATION_VERSION.to_owned()),
            );
            configuration.insert(
                "rust_expression_profile".to_owned(),
                Value::String(R14_PROFILE.to_owned()),
            );
            configuration.insert("output_capacity_profile".to_owned(), capacity);
            Value::Object(configuration)
        } else {
            json!({
                "schema_version": R14_CONFIGURATION_VERSION,
                "profile": "standard-local-s4",
                "workspace_profile": R3_WORKSPACE_PROFILE,
                "manifest_profile": R4_MANIFEST_PROFILE,
                "rust_semantic_profile": R5_RUST_SEMANTIC_PROFILE,
                "rust_framework_profile": R6_FRAMEWORK_PROFILE,
                "rust_callable_profile": "rust-callable-semantics-v1",
                "rust_expression_profile": R14_PROFILE,
                "output_capacity_profile": capacity
            })
        };
        let configuration_hash =
            semantic_hash(CONFIGURATION_V13_HASH_DOMAIN, &configuration_without_hash);
        let mut configuration = configuration_without_hash;
        configuration["semantic_hash"] =
            json!({"algorithm": "blake3-256", "value": configuration_hash});
        semantic.insert("configuration".to_owned(), configuration);
        semantic.insert(
            "pipeline_version".to_owned(),
            Value::String(R14_PIPELINE_VERSION.to_owned()),
        );
        semantic.insert(
            "ontology_version".to_owned(),
            Value::String(R14_ONTOLOGY_VERSION.to_owned()),
        );
        semantic.insert(
            "extractor_contract_version".to_owned(),
            Value::String(R14_EXTRACTION_CONTRACT_VERSION.to_owned()),
        );
        append_extractor_version(semantic, R14_EXTRACTOR_VERSION)?;
        let baseline_chunks = semantic
            .get("extraction_chunks")
            .cloned()
            .ok_or(RepositorySnapshotV16Error::ContractInvalid)?;
        semantic.insert(
            "extraction_chunks".to_owned(),
            Value::Array(extraction_chunks_v13(&baseline_chunks, knowledge)?),
        );
        let baseline_graph = semantic
            .get("knowledge_graph")
            .cloned()
            .ok_or(RepositorySnapshotV16Error::ContractInvalid)?;
        semantic.insert(
            "knowledge_graph".to_owned(),
            knowledge_graph_v13(&baseline_graph, knowledge)?,
        );
        let semantic_value = Value::Object(semantic.clone());
        let snapshot_hash = semantic_hash(SNAPSHOT_V16_HASH_DOMAIN, &semantic_value);
        let root = value
            .as_object_mut()
            .ok_or(RepositorySnapshotV16Error::ContractInvalid)?;
        root.insert(
            "schema_version".to_owned(),
            Value::String(R14_SNAPSHOT_VERSION.to_owned()),
        );
        root.insert(
            "semantic_hash".to_owned(),
            json!({"algorithm": "blake3-256", "value": snapshot_hash}),
        );
        publication_candidate(&value).map_err(|_| RepositorySnapshotV16Error::ContractInvalid)?;
        Ok(Self {
            value,
            output_capacity_profile,
        })
    }

    /// Serializes V16 under its selected bounded output envelope.
    ///
    /// # Errors
    ///
    /// Returns a serialization or selected output-limit failure.
    pub fn canonical_stdout(&self) -> Result<Vec<u8>, RepositorySnapshotV16Error> {
        let maximum = usize::try_from(self.output_capacity_profile.maximum_bytes())
            .map_err(|_| RepositorySnapshotV16Error::OutputLengthOverflow)?;
        let body_maximum = maximum
            .checked_sub(1)
            .ok_or(RepositorySnapshotV16Error::OutputLengthOverflow)?;
        let mut writer = LimitedVecWriter::new(body_maximum);
        let result = serde_json::to_writer(&mut writer, &self.value);
        if writer.overflowed() {
            return Err(RepositorySnapshotV16Error::LimitExceeded(
                AcquisitionError::LimitExceeded {
                    limit: LimitKind::CanonicalOutputBytes,
                    maximum: self.output_capacity_profile.maximum_bytes(),
                    observed: self
                        .output_capacity_profile
                        .maximum_bytes()
                        .saturating_add(1),
                },
            ));
        }
        result.map_err(RepositorySnapshotV16Error::Serialization)?;
        let mut bytes = writer.into_inner();
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Serializes the exact V16 semantic payload.
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

    /// Converts V16 into the immutable publication model.
    ///
    /// # Errors
    ///
    /// Returns a strict contract or storage-integrity failure.
    pub fn publication_candidate(&self) -> Result<PublicationCandidate, PublicationCandidateError> {
        publication_candidate(&self.value)
    }
}

/// Validates one loaded V16 semantic payload against its visible head.
///
/// # Errors
///
/// Returns a typed storage-integrity failure on any mismatch.
pub fn validate_stored_snapshot_semantic_v16(
    semantic: &Value,
    head: &LocalSnapshotHead,
) -> Result<(), StorageError> {
    if head.snapshot_schema_version != SNAPSHOT_SCHEMA_VERSION_V16 {
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
pub struct LocalQueryResultV11 {
    value: Value,
}

impl LocalQueryResultV11 {
    /// Serializes one bounded exact-ID V11 result followed by LF.
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

/// Builds one exact-ID result with the stable K1 and directly linked R14 neighborhood.
///
/// # Errors
///
/// Returns a strict snapshot, document, not-found, identity, or result-limit failure.
#[allow(clippy::too_many_lines)]
pub fn local_query_result_v11(
    semantic: &Value,
    manifest: &Value,
    snapshot_id: &str,
    requested_id: &str,
) -> Result<LocalQueryResultV11, QueryContractError> {
    if semantic.get("ontology_version").and_then(Value::as_str) != Some(R14_ONTOLOGY_VERSION)
        || semantic
            .pointer("/knowledge_graph/schema_version")
            .and_then(Value::as_str)
            != Some(R14_GRAPH_VERSION)
    {
        return Err(QueryContractError::InvalidSnapshot);
    }
    let mut compatible = semantic.clone();
    let mut value = if semantic.get("repository_boundaries").is_some()
        || semantic
            .pointer("/configuration/rust_semantic_profile")
            .and_then(Value::as_str)
            == Some(codenoesis_domain::s4_r10::R10_PROFILE)
    {
        compatible["ontology_version"] =
            Value::String(codenoesis_domain::s4_r12::R12_ONTOLOGY_VERSION.to_owned());
        compatible["knowledge_graph"]["schema_version"] =
            Value::String(codenoesis_domain::s4_r12::R12_GRAPH_VERSION.to_owned());
        compatible["knowledge_graph"]["ontology_version"] =
            Value::String(codenoesis_domain::s4_r12::R12_ONTOLOGY_VERSION.to_owned());
        local_query_result_v9(&compatible, manifest, snapshot_id, requested_id)?
            .value()
            .clone()
    } else {
        compatible["ontology_version"] = Value::String(K1_ONTOLOGY_VERSION.to_owned());
        compatible["knowledge_graph"]["schema_version"] =
            Value::String(K1_GRAPH_VERSION.to_owned());
        compatible["knowledge_graph"]["ontology_version"] =
            Value::String(K1_ONTOLOGY_VERSION.to_owned());
        local_query_result_v6(&compatible, manifest, snapshot_id, requested_id)?
            .value()
            .clone()
    };
    let graph = semantic
        .get("knowledge_graph")
        .and_then(Value::as_object)
        .ok_or(QueryContractError::InvalidSnapshot)?;
    let entities = id_value_map(graph, "entities")?;
    let relationships = graph
        .get("relationships")
        .and_then(Value::as_array)
        .ok_or(QueryContractError::InvalidSnapshot)?;
    let mut linked_relationships = relationships
        .iter()
        .filter(|relationship| {
            ["id", "source", "target"]
                .iter()
                .any(|field| relationship.get(*field).and_then(Value::as_str) == Some(requested_id))
        })
        .filter(|relationship| {
            relationship
                .get("kind")
                .and_then(Value::as_str)
                .is_some_and(is_expression_relationship_kind)
        })
        .cloned()
        .collect::<Vec<_>>();
    sort_dedup_records(&mut linked_relationships)?;
    let mut endpoint_ids = BTreeSet::new();
    for relationship in &linked_relationships {
        for field in ["source", "target"] {
            endpoint_ids.insert(
                relationship
                    .get(field)
                    .and_then(Value::as_str)
                    .ok_or(QueryContractError::InvalidSnapshot)?
                    .to_owned(),
            );
        }
    }
    endpoint_ids.remove(requested_id);
    let mut linked_expression_entities = endpoint_ids
        .iter()
        .filter_map(|identifier| entities.get(identifier.as_str()).copied())
        .filter(|entity| {
            entity
                .get("kind")
                .and_then(Value::as_str)
                .is_some_and(is_expression_entity_kind)
        })
        .cloned()
        .collect::<Vec<_>>();
    sort_dedup_records(&mut linked_expression_entities)?;
    let linked_k1_additions = endpoint_ids
        .iter()
        .filter_map(|identifier| entities.get(identifier.as_str()).copied())
        .filter(|entity| {
            entity
                .get("kind")
                .and_then(Value::as_str)
                .is_some_and(|kind| !is_expression_entity_kind(kind))
        })
        .cloned()
        .collect::<Vec<_>>();
    union_query_family(&mut value, "linked_k1_entities", &linked_k1_additions)?;
    value["schema_version"] = Value::String(R14_QUERY_VERSION.to_owned());
    value["linked_expression_entities"] = Value::Array(linked_expression_entities);
    value["linked_expression_relationships"] = Value::Array(linked_relationships.clone());
    let related_subject_ids = linked_relationships
        .iter()
        .filter_map(record_id)
        .map(str::to_owned)
        .chain(
            value["linked_expression_entities"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(record_id)
                .map(str::to_owned),
        )
        .chain(std::iter::once(requested_id.to_owned()))
        .collect::<BTreeSet<_>>();
    union_linked_claims_and_evidence(&mut value, graph, &related_subject_ids)?;
    let result = LocalQueryResultV11 { value };
    result.canonical_stdout()?;
    Ok(result)
}

fn extraction_chunks_v13(
    baseline: &Value,
    knowledge: &ExpressionBindingKnowledge,
) -> Result<Vec<Value>, RepositorySnapshotV16Error> {
    let mut additions = knowledge
        .extraction_chunks
        .iter()
        .map(|chunk| (chunk.source_file_id.as_str(), chunk))
        .collect::<BTreeMap<_, _>>();
    let chunks = baseline
        .as_array()
        .ok_or(RepositorySnapshotV16Error::ContractInvalid)?;
    let mut transformed = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        let mut value = chunk.clone();
        let object = value
            .as_object_mut()
            .ok_or(RepositorySnapshotV16Error::ContractInvalid)?;
        object.insert(
            "schema_version".to_owned(),
            Value::String(R14_EXTRACTION_CHUNK_VERSION.to_owned()),
        );
        object.insert(
            "ontology_version".to_owned(),
            Value::String(R14_ONTOLOGY_VERSION.to_owned()),
        );
        object.remove("semantic_hash");
        let source_file_id = object
            .get("subject")
            .and_then(|subject| {
                (subject.get("kind").and_then(Value::as_str) == Some("rust_source"))
                    .then(|| subject.get("source_file_id").and_then(Value::as_str))
                    .flatten()
            })
            .or_else(|| object.get("source_file_id").and_then(Value::as_str))
            .map(str::to_owned);
        if let Some(source_file_id) = source_file_id {
            let expression = additions
                .remove(source_file_id.as_str())
                .ok_or(RepositorySnapshotV16Error::ContractInvalid)?;
            object.insert(
                "expression_bindings_profile".to_owned(),
                Value::String(R14_PROFILE.to_owned()),
            );
            merge_id_array(
                object,
                "entities",
                expression.entities.iter().map(expression_entity_value),
            )?;
            merge_id_array(
                object,
                "relationships",
                expression
                    .relationships
                    .iter()
                    .map(expression_relationship_value),
            )?;
            merge_id_array(object, "claims", expression.claims.iter().map(claim_value))?;
            merge_id_array(
                object,
                "evidence",
                expression.evidence.iter().map(evidence_value),
            )?;
            merge_id_array(
                object,
                "coverage",
                expression.coverage.iter().map(expression_coverage_value),
            )?;
        }
        insert_semantic_hash(&mut value, EXTRACTION_V13_HASH_DOMAIN)?;
        transformed.push(value);
    }
    if !additions.is_empty() {
        return Err(RepositorySnapshotV16Error::ContractInvalid);
    }
    Ok(transformed)
}

fn knowledge_graph_v13(
    baseline: &Value,
    knowledge: &ExpressionBindingKnowledge,
) -> Result<Value, RepositorySnapshotV16Error> {
    let mut value = baseline.clone();
    let object = value
        .as_object_mut()
        .ok_or(RepositorySnapshotV16Error::ContractInvalid)?;
    object.insert(
        "schema_version".to_owned(),
        Value::String(R14_GRAPH_VERSION.to_owned()),
    );
    object.insert(
        "ontology_version".to_owned(),
        Value::String(R14_ONTOLOGY_VERSION.to_owned()),
    );
    append_extractor_version(object, R14_EXTRACTOR_VERSION)?;
    object.insert(
        "expression_binding_index".to_owned(),
        json!({
            "schema_version": R14_INDEX_VERSION,
            "expression_entity_ids": knowledge.graph.index.expression_entity_ids,
            "argument_entity_ids": knowledge.graph.index.argument_entity_ids,
            "binding_entity_ids": knowledge.graph.index.binding_entity_ids,
            "read_relationship_ids": knowledge.graph.index.read_relationship_ids,
            "write_relationship_ids": knowledge.graph.index.write_relationship_ids,
            "call_site_relationship_ids": knowledge.graph.index.call_site_relationship_ids
        }),
    );
    object.remove("semantic_hash");
    merge_id_array(
        object,
        "entities",
        knowledge.graph.entities.iter().map(expression_entity_value),
    )?;
    merge_id_array(
        object,
        "relationships",
        knowledge
            .graph
            .relationships
            .iter()
            .map(expression_relationship_value),
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
        "coverage",
        knowledge
            .graph
            .coverage
            .iter()
            .map(expression_coverage_value),
    )?;
    validate_graph_overlay(object, knowledge)?;
    insert_semantic_hash(&mut value, GRAPH_V13_HASH_DOMAIN)?;
    Ok(value)
}

fn validate_graph_overlay(
    graph: &Map<String, Value>,
    knowledge: &ExpressionBindingKnowledge,
) -> Result<(), RepositorySnapshotV16Error> {
    let entities = family_id_set(graph, "entities")?;
    let relationships = family_id_set(graph, "relationships")?;
    let claims = family_id_map(graph, "claims")?;
    let claim_subjects = claim_subjects(claims.values().copied());
    let evidence = family_id_set(graph, "evidence")?;
    for entity in &knowledge.graph.entities {
        if !entities.contains(&entity.id) || !evidence.contains(&entity.evidence_id) {
            return Err(RepositorySnapshotV16Error::ContractInvalid);
        }
    }
    for relationship in &knowledge.graph.relationships {
        if !relationships.contains(&relationship.id)
            || relationship
                .evidence_ids
                .iter()
                .any(|identifier| !evidence.contains(identifier))
            || !claim_subjects.contains(relationship.id.as_str())
        {
            return Err(RepositorySnapshotV16Error::ContractInvalid);
        }
    }
    Ok(())
}

fn expression_entity_value(entity: &ExpressionBindingEntity) -> Value {
    json!({
        "id": entity.id,
        "kind": entity.kind.as_str(),
        "name": entity.name,
        "callable_id": entity.callable_id,
        "evidence_id": entity.evidence_id,
        "locator": {
            "path": entity.locator.path,
            "blob_oid": entity.locator.blob_oid,
            "start_byte": entity.locator.start_byte,
            "end_byte": entity.locator.end_byte
        },
        "properties": expression_properties_value(&entity.properties)
    })
}

fn expression_properties_value(properties: &ExpressionEntityProperties) -> Value {
    match properties {
        ExpressionEntityProperties::Expression(value) => json!({
            "syntax_kind": value.syntax_kind,
            "token": value.token,
            "operator": value.operator,
            "source_digest": value.source_digest,
            "source_byte_length": value.source_byte_length,
            "parent_expression_id": value.parent_expression_id,
            "lexical_depth": value.lexical_depth,
            "roles": value.roles.iter().map(|role| role.as_str()).collect::<Vec<_>>(),
            "state": "syntax_fact"
        }),
        ExpressionEntityProperties::CallArgument(value) => json!({
            "call_expression_id": value.call_expression_id,
            "ordinal": value.ordinal,
            "expression_id": value.expression_id
        }),
        ExpressionEntityProperties::PatternBinding(value) => json!({
            "origin": value.origin.as_str(),
            "scope_owner_id": value.scope_owner_id,
            "modifier": value.modifier.as_str(),
            "scope_start_byte": value.scope_start_byte,
            "scope_end_byte": value.scope_end_byte,
            "binding_state": "lexically_bound"
        }),
    }
}

fn expression_relationship_value(relationship: &ExpressionBindingRelationship) -> Value {
    json!({
        "id": relationship.id,
        "kind": relationship.kind.as_str(),
        "source": relationship.source,
        "target": relationship.target,
        "evidence_ids": relationship.evidence_ids
    })
}

fn expression_coverage_value(gap: &ExpressionCoverageGap) -> Value {
    json!({
        "id": gap.id,
        "capability": gap.capability,
        "state": gap.state,
        "subject_id": gap.subject_id,
        "evidence_ids": gap.evidence_ids
    })
}

fn append_extractor_version(
    object: &mut Map<String, Value>,
    version: &str,
) -> Result<(), RepositorySnapshotV16Error> {
    let versions = object
        .get_mut("extractor_versions")
        .and_then(Value::as_array_mut)
        .ok_or(RepositorySnapshotV16Error::ContractInvalid)?;
    if versions.iter().any(|value| value.as_str() == Some(version)) {
        return Err(RepositorySnapshotV16Error::ContractInvalid);
    }
    versions.push(Value::String(version.to_owned()));
    Ok(())
}

fn merge_id_array(
    object: &mut Map<String, Value>,
    field: &'static str,
    additions: impl IntoIterator<Item = Value>,
) -> Result<(), RepositorySnapshotV16Error> {
    let values = object
        .get_mut(field)
        .and_then(Value::as_array_mut)
        .ok_or(RepositorySnapshotV16Error::ContractInvalid)?;
    values.extend(additions);
    values.sort_by(|left, right| record_id(left).cmp(&record_id(right)));
    let mut retained = Vec::with_capacity(values.len());
    for value in values.drain(..) {
        if let Some(previous) = retained.last()
            && record_id(previous) == record_id(&value)
        {
            if previous != &value {
                return Err(RepositorySnapshotV16Error::ContractInvalid);
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
) -> Result<(), RepositorySnapshotV16Error> {
    let object = value
        .as_object_mut()
        .ok_or(RepositorySnapshotV16Error::ContractInvalid)?;
    object.remove("semantic_hash");
    let hash = semantic_hash(domain, value);
    value
        .as_object_mut()
        .ok_or(RepositorySnapshotV16Error::ContractInvalid)?
        .insert(
            "semantic_hash".to_owned(),
            json!({"algorithm": "blake3-256", "value": hash}),
        );
    Ok(())
}

fn family_id_set(
    graph: &Map<String, Value>,
    family: &'static str,
) -> Result<BTreeSet<String>, RepositorySnapshotV16Error> {
    Ok(family_id_map(graph, family)?
        .into_keys()
        .map(str::to_owned)
        .collect())
}

fn family_id_map<'a>(
    graph: &'a Map<String, Value>,
    family: &'static str,
) -> Result<BTreeMap<&'a str, &'a Value>, RepositorySnapshotV16Error> {
    let mut values = BTreeMap::new();
    let mut previous = None;
    for value in graph
        .get(family)
        .and_then(Value::as_array)
        .ok_or(RepositorySnapshotV16Error::ContractInvalid)?
    {
        let identifier = record_id(value).ok_or(RepositorySnapshotV16Error::ContractInvalid)?;
        if previous.is_some_and(|previous| previous >= identifier)
            || values.insert(identifier, value).is_some()
        {
            return Err(RepositorySnapshotV16Error::ContractInvalid);
        }
        previous = Some(identifier);
    }
    Ok(values)
}

fn map_k1_snapshot_error(error: RepositorySnapshotV11Error) -> RepositorySnapshotV16Error {
    match error {
        RepositorySnapshotV11Error::Serialization(error) => {
            RepositorySnapshotV16Error::Serialization(error)
        }
        RepositorySnapshotV11Error::LimitExceeded(error) => {
            RepositorySnapshotV16Error::LimitExceeded(error)
        }
        RepositorySnapshotV11Error::ContractInvalid => RepositorySnapshotV16Error::ContractInvalid,
        RepositorySnapshotV11Error::OutputLengthOverflow => {
            RepositorySnapshotV16Error::OutputLengthOverflow
        }
    }
}

fn map_r12_snapshot_error(error: RepositorySnapshotV14Error) -> RepositorySnapshotV16Error {
    match error {
        RepositorySnapshotV14Error::Serialization(error) => {
            RepositorySnapshotV16Error::Serialization(error)
        }
        RepositorySnapshotV14Error::LimitExceeded(error) => {
            RepositorySnapshotV16Error::LimitExceeded(error)
        }
        RepositorySnapshotV14Error::ContractInvalid => RepositorySnapshotV16Error::ContractInvalid,
        RepositorySnapshotV14Error::OutputLengthOverflow => {
            RepositorySnapshotV16Error::OutputLengthOverflow
        }
    }
}

#[derive(Clone, Debug)]
pub struct PortableGraphV7 {
    value: Value,
    canonical: Vec<u8>,
    sha256: R14Sha256,
}

impl PortableGraphV7 {
    /// Projects one validated V16 head and deterministic documentation manifest.
    ///
    /// # Errors
    ///
    /// Returns a strict binding, privacy, reference, canonicality, or limit failure.
    pub fn from_validated_v16(
        semantic: &Value,
        head: &LocalSnapshotHead,
        documentation_manifest: &Value,
        sha256: R14Sha256,
    ) -> Result<Self, R14ContractError> {
        validate_stored_snapshot_semantic_v16(semantic, head)
            .map_err(|_| R14ContractError::InvalidSnapshot)?;
        if semantic.get("ontology_version").and_then(Value::as_str) != Some(R14_ONTOLOGY_VERSION) {
            return Err(R14ContractError::InvalidSnapshot);
        }
        validate_documentation_binding(documentation_manifest, head)?;
        let graph = semantic
            .get("knowledge_graph")
            .and_then(Value::as_object)
            .ok_or(R14ContractError::InvalidSnapshot)?;
        let (documents, document_statements) = portable_documents(documentation_manifest)?;
        let mut value = json!({
            "schema_version": R14_PORTABLE_GRAPH_VERSION,
            "repository": semantic.get("repository").cloned().ok_or(R14ContractError::InvalidSnapshot)?,
            "source_snapshot": {
                "schema_version": R14_SNAPSHOT_VERSION,
                "snapshot_id": head.snapshot_id.as_str(),
                "semantic_hash": {
                    "algorithm": head.semantic_hash.algorithm,
                    "value": head.semantic_hash.value
                }
            },
            "ontology_version": R14_ONTOLOGY_VERSION,
            "query_contract_version": R14_QUERY_VERSION,
            "projection": {
                "profile": "codenoesis.lossless-portable-projection/v7",
                "family_sha256": {}
            },
            "entities": graph.get("entities").cloned().ok_or(R14ContractError::InvalidSnapshot)?,
            "relationships": graph.get("relationships").cloned().ok_or(R14ContractError::InvalidSnapshot)?,
            "claims": graph.get("claims").cloned().ok_or(R14ContractError::InvalidSnapshot)?,
            "evidence": graph.get("evidence").cloned().ok_or(R14ContractError::InvalidSnapshot)?,
            "diagnostics": graph.get("diagnostics").cloned().ok_or(R14ContractError::InvalidSnapshot)?,
            "coverage_gaps": graph.get("coverage").cloned().ok_or(R14ContractError::InvalidSnapshot)?,
            "documents": documents,
            "document_statements": document_statements
        });
        if let Some(boundaries) = semantic.get("repository_boundaries") {
            super::s4_r12::validate_boundary_projection(boundaries)
                .map_err(|_| R14ContractError::InvalidSnapshot)?;
            value["repository_boundaries"] = boundaries.clone();
        }
        value["projection"]["family_sha256"] = family_digests(&value, sha256)?;
        Self::from_generated_value(value, sha256)
    }

    /// Strictly reimports one canonical LF-terminated `PortableGraphV7`.
    ///
    /// # Errors
    ///
    /// Returns the first decode, schema, identity, reference, privacy, or limit failure.
    pub fn from_canonical_file(bytes: &[u8], sha256: R14Sha256) -> Result<Self, R14ContractError> {
        enforce_portable_size(bytes.len())?;
        let value = serde_json::from_slice::<Value>(bytes)
            .map_err(|_| R14ContractError::InvalidProjection)?;
        validate_portable_value(&value, sha256)?;
        let canonical = serde_json::to_vec(&value).map_err(|_| R14ContractError::Internal)?;
        let mut expected = canonical.clone();
        expected.push(b'\n');
        if expected != bytes {
            return Err(R14ContractError::Noncanonical {
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

    fn from_generated_value(value: Value, sha256: R14Sha256) -> Result<Self, R14ContractError> {
        validate_portable_value(&value, sha256)?;
        let canonical = serde_json::to_vec(&value).map_err(|_| R14ContractError::Internal)?;
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
pub struct LocalExplorerManifestV7 {
    value: Value,
}

impl LocalExplorerManifestV7 {
    /// Builds the offline V7 explorer manifest bound to exact graph and K1 viewer bytes.
    ///
    /// # Errors
    ///
    /// Returns an integrity or unsafe-CSP failure.
    pub fn new(
        portable: &PortableGraphV7,
        viewer_bytes: &[u8],
        expected_viewer_sha256: &str,
        content_security_policy: &str,
        sha256: R14Sha256,
    ) -> Result<Self, R14ContractError> {
        if sha256(viewer_bytes) != expected_viewer_sha256
            || content_security_policy.contains("http:")
            || content_security_policy.contains("https:")
            || content_security_policy.contains("unsafe-inline")
            || content_security_policy.contains("unsafe-eval")
            || !balanced_script_elements(viewer_bytes)
        {
            return Err(R14ContractError::AssetIntegrityMismatch);
        }
        Ok(Self {
            value: json!({
                "schema_version": R14_LOCAL_EXPLORER_VERSION,
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
                    "profile": R14_EXPLORER_SECURITY_PROFILE,
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
                    "text_search_results": MAX_R14_TEXT_SEARCH_RESULTS,
                    "traversal_depth_default": R14_TRAVERSAL_DEPTH_DEFAULT,
                    "traversal_depth_maximum": MAX_R14_TRAVERSAL_DEPTH
                }
            }),
        })
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Serializes one `LocalExplorerManifestV7` followed by LF.
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
pub enum R14ContractError {
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

impl Display for R14ContractError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidSnapshot => "invalid R14 snapshot",
            Self::UnsupportedSnapshotSchema(_) => "unsupported R14 snapshot schema",
            Self::UnsupportedPortableGraphSchema(_) => "unsupported portable graph schema",
            Self::Noncanonical { .. } => "noncanonical portable graph",
            Self::IdentityConflict { .. } => "portable graph identity conflict",
            Self::ReferenceMismatch { .. } => "portable graph reference mismatch",
            Self::LimitExceeded { .. } => "portable graph limit exceeded",
            Self::UnsafePayload { .. } => "unsafe portable payload",
            Self::AssetIntegrityMismatch => "explorer asset integrity mismatch",
            Self::InvalidProjection => "invalid portable graph projection",
            Self::Internal => "internal R14 contract failure",
        })
    }
}

impl Error for R14ContractError {}

fn validate_documentation_binding(
    manifest: &Value,
    head: &LocalSnapshotHead,
) -> Result<(), R14ContractError> {
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
        return Err(R14ContractError::InvalidSnapshot);
    }
    Ok(())
}

fn portable_documents(manifest: &Value) -> Result<(Vec<Value>, Vec<Value>), R14ContractError> {
    let source = manifest
        .get("documents")
        .and_then(Value::as_array)
        .ok_or(R14ContractError::InvalidSnapshot)?;
    let mut documents = Vec::with_capacity(source.len());
    let mut statements = Vec::new();
    for document in source {
        let mut record = document
            .as_object()
            .cloned()
            .ok_or(R14ContractError::InvalidSnapshot)?;
        let document_id = record
            .get("document_id")
            .and_then(Value::as_str)
            .ok_or(R14ContractError::InvalidSnapshot)?
            .to_owned();
        let document_statements = record
            .remove("statements")
            .and_then(|value| value.as_array().cloned())
            .ok_or(R14ContractError::InvalidSnapshot)?;
        documents.push(Value::Object(record));
        for statement in document_statements {
            let mut statement = statement
                .as_object()
                .cloned()
                .ok_or(R14ContractError::InvalidSnapshot)?;
            if statement
                .insert("document_id".to_owned(), Value::String(document_id.clone()))
                .is_some()
            {
                return Err(R14ContractError::InvalidSnapshot);
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

fn family_digests(value: &Value, sha256: R14Sha256) -> Result<Value, R14ContractError> {
    let mut digests = Map::new();
    for (family, _) in PORTABLE_FAMILIES {
        let bytes = serde_json::to_vec(
            value
                .get(family)
                .ok_or(R14ContractError::InvalidProjection)?,
        )
        .map_err(|_| R14ContractError::Internal)?;
        digests.insert(family.to_owned(), Value::String(sha256(&bytes)));
    }
    Ok(Value::Object(digests))
}

#[allow(clippy::too_many_lines)]
fn validate_portable_value(value: &Value, sha256: R14Sha256) -> Result<(), R14ContractError> {
    ensure_nesting(value, 0)?;
    validate_private_fields(value)?;
    let object = value
        .as_object()
        .ok_or(R14ContractError::InvalidProjection)?;
    let mut expected_keys = BTreeSet::from([
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
    if object.contains_key("repository_boundaries") {
        expected_keys.insert("repository_boundaries");
    }
    if object.keys().map(String::as_str).collect::<BTreeSet<_>>() != expected_keys {
        return Err(R14ContractError::InvalidProjection);
    }
    let schema = object
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or(R14ContractError::InvalidProjection)?;
    if schema != R14_PORTABLE_GRAPH_VERSION {
        return Err(R14ContractError::UnsupportedPortableGraphSchema(bounded(
            schema, 256,
        )));
    }
    if object.get("ontology_version").and_then(Value::as_str) != Some(R14_ONTOLOGY_VERSION)
        || object.get("query_contract_version").and_then(Value::as_str) != Some(R14_QUERY_VERSION)
        || value
            .pointer("/source_snapshot/schema_version")
            .and_then(Value::as_str)
            != Some(R14_SNAPSHOT_VERSION)
        || value.pointer("/projection/profile").and_then(Value::as_str)
            != Some("codenoesis.lossless-portable-projection/v7")
    {
        return Err(R14ContractError::InvalidProjection);
    }
    let repository_identity = value
        .pointer("/repository/identity")
        .and_then(Value::as_str)
        .filter(|value| value.starts_with("urn:codenoesis:"))
        .ok_or(R14ContractError::InvalidProjection)?;
    let entity_ids = validate_family(object, "entities", "id")?;
    let relationship_ids = validate_family(object, "relationships", "id")?;
    let claim_ids = validate_family(object, "claims", "id")?;
    let evidence_ids = validate_family(object, "evidence", "id")?;
    let diagnostic_ids = validate_family(object, "diagnostics", "id")?;
    let coverage_ids = validate_family(object, "coverage_gaps", "id")?;
    let document_ids = validate_family(object, "documents", "document_id")?;
    let statement_ids = validate_family(object, "document_statements", "statement_id")?;
    let boundary_ids = object.get("repository_boundaries").map_or_else(
        || Ok(super::s4_r12::BoundaryDocumentReferenceIds::default()),
        |boundaries| {
            super::s4_r12::validate_boundary_projection(boundaries)
                .map_err(|_| R14ContractError::InvalidProjection)?;
            super::s4_r12::boundary_document_reference_ids(boundaries)
                .map_err(|_| R14ContractError::InvalidProjection)
        },
    )?;
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
    subject_ids.extend(boundary_ids.subjects);
    let mut statement_evidence_ids = evidence_ids.clone();
    statement_evidence_ids.extend(boundary_ids.evidence);
    let mut statement_coverage_ids = coverage_ids.clone();
    statement_coverage_ids.extend(boundary_ids.coverage);
    for relationship in object["relationships"]
        .as_array()
        .ok_or(R14ContractError::InvalidProjection)?
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
        .ok_or(R14ContractError::InvalidProjection)?
    {
        let subject_id = claim
            .get("subject_id")
            .and_then(Value::as_str)
            .ok_or(R14ContractError::InvalidProjection)?;
        let valid = match claim.get("subject_kind").and_then(Value::as_str) {
            Some("entity") => entity_ids.contains(subject_id),
            Some("relationship") => relationship_ids.contains(subject_id),
            _ => false,
        };
        if !valid {
            return Err(R14ContractError::ReferenceMismatch {
                family: "claims",
                id: subject_id.to_owned(),
            });
        }
        validate_reference_array_if_present(claim, "evidence_ids", &evidence_ids, "claims")?;
    }
    for family in ["entities", "diagnostics", "coverage_gaps"] {
        for record in object[family]
            .as_array()
            .ok_or(R14ContractError::InvalidProjection)?
        {
            validate_reference_array_if_present(record, "evidence_ids", &evidence_ids, family)?;
            if let Some(identifier) = record.get("evidence_id").and_then(Value::as_str)
                && !evidence_ids.contains(identifier)
            {
                return Err(R14ContractError::ReferenceMismatch {
                    family,
                    id: identifier.to_owned(),
                });
            }
        }
    }
    for evidence in object["evidence"]
        .as_array()
        .ok_or(R14ContractError::InvalidProjection)?
    {
        validate_evidence_path(evidence)?;
    }
    for document in object["documents"]
        .as_array()
        .ok_or(R14ContractError::InvalidProjection)?
    {
        validate_reference(document, "subject_id", &subject_ids, "documents")?;
        if let Some(path) = document.get("path").and_then(Value::as_str)
            && !safe_relative_path(path)
        {
            return Err(R14ContractError::UnsafePayload {
                reason: "unsafe_document_path",
            });
        }
    }
    for statement in object["document_statements"]
        .as_array()
        .ok_or(R14ContractError::InvalidProjection)?
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
            &statement_evidence_ids,
            "document_statements",
        )?;
        validate_reference_array_if_present(
            statement,
            "coverage_gap_ids",
            &statement_coverage_ids,
            "document_statements",
        )?;
    }
    if value.pointer("/projection/family_sha256") != Some(&family_digests(value, sha256)?) {
        return Err(R14ContractError::InvalidProjection);
    }
    Ok(())
}

fn validate_family(
    object: &Map<String, Value>,
    family: &'static str,
    id_field: &'static str,
) -> Result<BTreeSet<String>, R14ContractError> {
    let values = object
        .get(family)
        .and_then(Value::as_array)
        .ok_or(R14ContractError::InvalidProjection)?;
    let mut identifiers = BTreeSet::new();
    let mut previous = None;
    for value in values {
        let identifier = value
            .get(id_field)
            .and_then(Value::as_str)
            .filter(|identifier| !identifier.is_empty())
            .ok_or(R14ContractError::InvalidProjection)?;
        if previous.is_some_and(|previous| previous >= identifier) {
            return Err(R14ContractError::IdentityConflict {
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
) -> Result<(), R14ContractError> {
    let identifier = record
        .get(field)
        .and_then(Value::as_str)
        .ok_or(R14ContractError::InvalidProjection)?;
    if !identifiers.contains(identifier) {
        return Err(R14ContractError::ReferenceMismatch {
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
) -> Result<(), R14ContractError> {
    let Some(values) = record.get(field) else {
        return Ok(());
    };
    let values = values
        .as_array()
        .ok_or(R14ContractError::InvalidProjection)?;
    for identifier in values {
        let identifier = identifier
            .as_str()
            .ok_or(R14ContractError::InvalidProjection)?;
        if !identifiers.contains(identifier) {
            return Err(R14ContractError::ReferenceMismatch {
                family,
                id: identifier.to_owned(),
            });
        }
    }
    Ok(())
}

fn validate_evidence_path(evidence: &Value) -> Result<(), R14ContractError> {
    if let Some(path) = evidence.get("path").and_then(Value::as_str) {
        return safe_relative_path(path)
            .then_some(())
            .ok_or(R14ContractError::UnsafePayload {
                reason: "unsafe_evidence_path",
            });
    }
    if evidence
        .get("artifact_sha256")
        .and_then(Value::as_str)
        .is_some_and(valid_sha256)
    {
        if let Some(path) = evidence.get("document_path").and_then(Value::as_str)
            && !safe_relative_path(path)
        {
            return Err(R14ContractError::UnsafePayload {
                reason: "unsafe_document_path",
            });
        }
        return Ok(());
    }
    Err(R14ContractError::InvalidProjection)
}

fn validate_private_fields(value: &Value) -> Result<(), R14ContractError> {
    match value {
        Value::Object(fields) => {
            for (field, nested) in fields {
                if matches!(
                    field.as_str(),
                    "body_text"
                        | "expression_text"
                        | "initializer_text"
                        | "literal_lexeme"
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
                    return Err(R14ContractError::UnsafePayload {
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
            return Err(R14ContractError::UnsafePayload { reason: "raw_url" });
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    Ok(())
}

fn ensure_nesting(value: &Value, depth: u64) -> Result<(), R14ContractError> {
    if depth > MAX_R14_JSON_NESTING {
        return Err(R14ContractError::LimitExceeded {
            limit: "json_nesting",
            maximum: MAX_R14_JSON_NESTING,
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

fn enforce_portable_size(length: usize) -> Result<(), R14ContractError> {
    let observed = u64::try_from(length).unwrap_or(u64::MAX);
    if observed > MAX_R14_PORTABLE_GRAPH_BYTES {
        return Err(R14ContractError::LimitExceeded {
            limit: "portable_graph_bytes",
            maximum: MAX_R14_PORTABLE_GRAPH_BYTES,
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

fn balanced_script_elements(bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let text = text.to_ascii_lowercase();
    let openings = text.match_indices("<script").count();
    openings > 0 && openings == text.match_indices("</script>").count()
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

fn is_expression_entity_kind(kind: &str) -> bool {
    matches!(
        kind,
        "rust.expression" | "rust.call_argument" | "rust.pattern_binding"
    )
}

fn is_expression_relationship_kind(kind: &str) -> bool {
    matches!(
        kind,
        "HAS_EXPRESSION"
            | "CONTAINS_EXPRESSION"
            | "HAS_ARGUMENT"
            | "ARGUMENT_VALUE"
            | "HAS_RECEIVER"
            | "REPRESENTS_CALL_SITE"
            | "DECLARES_BINDING"
            | "BINDS_FROM"
            | "READS"
            | "WRITES"
    )
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
mod output_capacity_tests {
    use super::*;
    use codenoesis_domain::LOCAL_SNAPSHOT_256M_CANONICAL_OUTPUT_BYTES;

    #[test]
    fn pt_nfr_per_001_r14_256m_output_capacity_maximum_plus_one() {
        let profile = K1OutputCapacityProfile::LocalSnapshot256MV1;
        let maximum = usize::try_from(LOCAL_SNAPSHOT_256M_CANONICAL_OUTPUT_BYTES)
            .expect("256 MiB maximum fits usize");

        let exact = RepositorySnapshotV16 {
            value: Value::String("x".repeat(maximum - 3)),
            output_capacity_profile: profile,
        };
        let bytes = exact
            .canonical_stdout()
            .expect("exact 256 MiB output must serialize");
        assert_eq!(bytes.len(), maximum);
        drop(bytes);
        drop(exact);

        let plus_one = RepositorySnapshotV16 {
            value: Value::String("x".repeat(maximum - 2)),
            output_capacity_profile: profile,
        };
        assert!(matches!(
            plus_one.canonical_stdout(),
            Err(RepositorySnapshotV16Error::LimitExceeded(
                AcquisitionError::LimitExceeded {
                    limit: LimitKind::CanonicalOutputBytes,
                    maximum: LOCAL_SNAPSHOT_256M_CANONICAL_OUTPUT_BYTES,
                    observed: 268_435_457,
                }
            ))
        ));
    }
}
