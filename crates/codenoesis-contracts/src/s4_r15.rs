use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::path::{Component, Path};

use codenoesis_domain::s4_k1::CallableSemanticsError;
use codenoesis_domain::s4_r5::RustSemanticError;
use codenoesis_domain::s4_r6::FrameworkError;
use codenoesis_domain::s4_r14::ExpressionBindingError;
use codenoesis_domain::s4_r15::{
    LocalFlowCoverageGap, LocalFlowDerivation, LocalFlowError, LocalFlowIndex, LocalFlowKnowledge,
    LocalFlowRelationship, SyntaxBasicBlock,
};
pub use codenoesis_domain::s4_r15::{
    MAX_R15_BLOCKS, MAX_R15_BLOCKS_PER_CALLABLE, MAX_R15_DERIVATION_INPUT_REFERENCES,
    MAX_R15_FLOW_NODES_PER_BLOCK, MAX_R15_NESTED_BRANCHES, MAX_R15_REACHABILITY_PAIRS_PER_CALLABLE,
    MAX_R15_RELATIONSHIPS, R15_CONFIGURATION_VERSION, R15_ERROR_VERSION,
    R15_EXTRACTION_CHUNK_VERSION, R15_EXTRACTION_CONTRACT_VERSION, R15_EXTRACTOR_VERSION,
    R15_GRAPH_VERSION, R15_INDEX_VERSION, R15_LOCAL_EXPLORER_VERSION, R15_ONTOLOGY_VERSION,
    R15_PIPELINE_VERSION, R15_PORTABLE_GRAPH_VERSION, R15_PROFILE, R15_QUERY_VERSION,
    R15_RULE_VERSION, R15_SEMANTIC_HASH_CONTRACT_VERSION, R15_SNAPSHOT_VERSION,
};
use codenoesis_domain::storage::{
    LocalSnapshotHead, PublicationCandidate, SNAPSHOT_SCHEMA_VERSION_V17, StorageComponent,
    StorageError,
};
use codenoesis_domain::{
    AcquisitionError, K1OutputCapacityProfile, LimitKind, RepositoryInventory,
};
use serde_json::{Map, Value, json};

use super::s4::{MAX_QUERY_BYTES, QueryContractError, claim_value, evidence_value};
use super::s4_r14::{RepositorySnapshotV16, RepositorySnapshotV16Error, local_query_result_v11};
use super::{
    LimitedVecWriter, PublicationCandidateError, SnapshotEnvelopeV1, publication_candidate,
    semantic_hash,
};

const CONFIGURATION_V14_HASH_DOMAIN: &[u8] = b"codenoesis.configuration.semantic.v14";
const SNAPSHOT_V17_HASH_DOMAIN: &[u8] = b"codenoesis.repository-snapshot.semantic.v17";
const EXTRACTION_V14_HASH_DOMAIN: &[u8] = b"codenoesis.extraction-chunk.semantic.v14";
const GRAPH_V14_HASH_DOMAIN: &[u8] = b"codenoesis.knowledge-graph.semantic.v14";
const PORTABLE_FAMILIES: [&str; 9] = [
    "entities",
    "relationships",
    "claims",
    "evidence",
    "diagnostics",
    "coverage_gaps",
    "documents",
    "document_statements",
    "local_flow_index",
];

pub type R15Sha256 = fn(&[u8]) -> String;
pub const R15_PORTABLE_MARKER: &str = ".codenoesis-portable-graph-v8";
pub const R15_EXPLORER_MARKER: &str = ".codenoesis-local-explorer-v8";
pub const R15_EXPLORER_SECURITY_PROFILE: &str = "codenoesis.local-explorer-security/v8";
pub const MAX_R15_PORTABLE_GRAPH_BYTES: u64 = 268_435_456;
pub const MAX_R15_JSON_NESTING: u64 = 64;
pub const MAX_R15_TEXT_SEARCH_RESULTS: u64 = 100;
pub const R15_TRAVERSAL_DEPTH_DEFAULT: u64 = 1;
pub const MAX_R15_TRAVERSAL_DEPTH: u64 = 2;

#[derive(Clone, Debug)]
pub struct CodeNoesisErrorV22 {
    value: Value,
}

impl CodeNoesisErrorV22 {
    #[must_use]
    pub fn invalid_profile(profile: &str) -> Self {
        Self::new(
            "input.invalid_rust_flow_profile",
            "input",
            "invalid rust flow profile",
            json!({"profile": bounded_nonempty(profile, 256, "missing")}),
        )
    }

    #[must_use]
    pub fn unsupported_composition(reason: &str) -> Self {
        Self::new(
            "input.unsupported_rust_flow_composition",
            "input",
            "unsupported rust flow profile composition",
            json!({
                "rust_expression_profile": "rust-expression-bindings-v1",
                "rust_flow_profile": R15_PROFILE,
                "reason": bounded_nonempty(reason, 128, "unsupported_composition")
            }),
        )
    }

    #[must_use]
    pub fn from_local_flow(error: &LocalFlowError) -> Self {
        match error {
            LocalFlowError::Source(ExpressionBindingError::Source(
                CallableSemanticsError::Source(FrameworkError::Source(
                    RustSemanticError::InvalidDeclaration {
                        path,
                        start_byte,
                        declaration_kind,
                    },
                )),
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
            LocalFlowError::InvalidSyntax { path, start_byte } => Self::new(
                "extraction.local_flow_syntax_unsupported",
                "extraction",
                "rust local-flow syntax is unsupported",
                json!({"path": bounded(path, 1024), "start_byte": start_byte}),
            ),
            LocalFlowError::IdentityConflict => Self::extraction(
                "extraction.local_flow_identity_conflict",
                "local-flow identity conflict",
            ),
            LocalFlowError::BlockInvalid => Self::extraction(
                "extraction.local_flow_block_invalid",
                "syntax basic block is invalid",
            ),
            LocalFlowError::EdgeInvalid => Self::extraction(
                "extraction.local_flow_edge_invalid",
                "local-flow edge is invalid",
            ),
            LocalFlowError::Cycle => Self::extraction(
                "extraction.local_flow_cycle",
                "local-flow graph contains a cycle",
            ),
            LocalFlowError::ReachabilityMismatch => Self::extraction(
                "extraction.local_flow_reachability_mismatch",
                "local-flow reachability does not match direct flow",
            ),
            LocalFlowError::AccessMismatch => Self::extraction(
                "extraction.local_flow_access_mismatch",
                "local-flow access does not match R14",
            ),
            LocalFlowError::DerivationMismatch => Self::extraction(
                "extraction.local_flow_derivation_mismatch",
                "local-flow derivation does not match its inputs",
            ),
            LocalFlowError::IndexMismatch => Self::extraction(
                "extraction.local_flow_index_mismatch",
                "local-flow index does not match the graph",
            ),
            LocalFlowError::LimitExceeded {
                limit,
                maximum,
                observed,
            } => Self::new(
                "extraction.local_flow_limit_exceeded",
                "extraction",
                "local-flow extraction limit exceeded",
                json!({"limit": limit.as_str(), "maximum": maximum, "observed": observed}),
            ),
            LocalFlowError::Source(_) | LocalFlowError::ContractInvalid => {
                Self::internal("local_flow_extraction")
            }
        }
    }

    #[must_use]
    pub fn from_contract(error: &R15ContractError) -> Self {
        match error {
            R15ContractError::LimitExceeded {
                limit,
                maximum,
                observed,
            } => Self::new(
                "export.limit_exceeded",
                "export",
                "portable graph limit exceeded",
                json!({"limit": limit, "maximum": maximum, "observed": observed}),
            ),
            R15ContractError::AssetIntegrityMismatch => Self::new(
                "explorer.asset_integrity_mismatch",
                "explorer",
                "local explorer asset integrity mismatch",
                json!({}),
            ),
            R15ContractError::InvalidSnapshot | R15ContractError::UnsupportedSnapshotSchema(_) => {
                Self::new(
                    "export.invalid_snapshot",
                    "export",
                    "invalid R15 source snapshot",
                    json!({}),
                )
            }
            R15ContractError::Internal => Self::internal("contract"),
            _ => Self::new(
                "export.invalid_portable_graph_v8",
                "export",
                "invalid portable graph v8",
                json!({}),
            ),
        }
    }

    #[must_use]
    pub fn from_explorer_contract(error: &R15ContractError) -> Self {
        match error {
            R15ContractError::LimitExceeded {
                limit,
                maximum,
                observed,
            } => Self::new(
                "explorer.limit_exceeded",
                "explorer",
                "local explorer input limit exceeded",
                json!({"limit": limit, "maximum": maximum, "observed": observed}),
            ),
            R15ContractError::AssetIntegrityMismatch => Self::new(
                "explorer.asset_integrity_mismatch",
                "explorer",
                "local explorer asset integrity mismatch",
                json!({}),
            ),
            R15ContractError::Internal => Self::internal("explorer"),
            _ => Self::new(
                "export.invalid_portable_graph_v8",
                "export",
                "invalid portable graph v8",
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
            "snapshot.invalid_v17",
            "snapshot",
            "invalid R15 snapshot",
            json!({}),
        )
    }

    #[must_use]
    pub fn invalid_query() -> Self {
        Self::new("query.invalid_v17", "query", "invalid R15 query", json!({}))
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
            "unexpected internal R15 failure",
            json!({"stage": bounded(stage, 128)}),
        )
    }

    fn extraction(code: &str, message: &str) -> Self {
        Self::new(code, "extraction", message, json!({}))
    }

    fn new(code: &str, stage: &str, message: &str, context: impl Into<Value>) -> Self {
        Self {
            value: json!({
                "schema_version": R15_ERROR_VERSION,
                "code": code,
                "stage": stage,
                "message": message,
                "retryable": false,
                "context": context.into()
            }),
        }
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Serializes one `ErrorV22` followed by LF.
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
pub struct RepositorySnapshotV17 {
    value: Value,
    output_capacity_profile: K1OutputCapacityProfile,
}

#[derive(Debug)]
pub enum RepositorySnapshotV17Error {
    Serialization(serde_json::Error),
    LimitExceeded(AcquisitionError),
    ContractInvalid,
    OutputLengthOverflow,
}

impl Display for RepositorySnapshotV17Error {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Serialization(_) => "R15 snapshot serialization failed",
            Self::LimitExceeded(_) => "R15 snapshot output limit exceeded",
            Self::ContractInvalid => "R15 snapshot contract is invalid",
            Self::OutputLengthOverflow => "R15 snapshot output length overflowed",
        })
    }
}

impl Error for RepositorySnapshotV17Error {}

impl RepositorySnapshotV17 {
    /// Builds the additive R15 local-flow overlay over the exact R14 source-only lineage.
    ///
    /// # Errors
    ///
    /// Returns the first R14, local-flow, identity, serialization, or publication failure.
    pub fn from_inventory_and_local_flow(
        inventory: &RepositoryInventory,
        knowledge: &LocalFlowKnowledge,
        output_capacity_profile: K1OutputCapacityProfile,
        envelope: SnapshotEnvelopeV1,
    ) -> Result<Self, RepositorySnapshotV17Error> {
        knowledge
            .validate()
            .map_err(|_| RepositorySnapshotV17Error::ContractInvalid)?;
        let baseline = RepositorySnapshotV16::from_inventory_and_expression_bindings(
            inventory,
            &knowledge.expression,
            output_capacity_profile,
            envelope,
        )
        .map_err(map_r14_snapshot_error)?;
        let mut value = baseline.value().clone();
        let semantic = value
            .get_mut("semantic")
            .and_then(Value::as_object_mut)
            .ok_or(RepositorySnapshotV17Error::ContractInvalid)?;
        let mut configuration = semantic
            .get("configuration")
            .and_then(Value::as_object)
            .cloned()
            .ok_or(RepositorySnapshotV17Error::ContractInvalid)?;
        configuration.insert(
            "schema_version".to_owned(),
            Value::String(R15_CONFIGURATION_VERSION.to_owned()),
        );
        configuration.insert(
            "rust_flow_profile".to_owned(),
            Value::String(R15_PROFILE.to_owned()),
        );
        configuration.remove("semantic_hash");
        let configuration_value = Value::Object(configuration.clone());
        configuration.insert(
            "semantic_hash".to_owned(),
            json!({
                "algorithm": "blake3-256",
                "value": semantic_hash(CONFIGURATION_V14_HASH_DOMAIN, &configuration_value)
            }),
        );
        semantic.insert("configuration".to_owned(), Value::Object(configuration));
        semantic.insert(
            "pipeline_version".to_owned(),
            Value::String(R15_PIPELINE_VERSION.to_owned()),
        );
        semantic.insert(
            "ontology_version".to_owned(),
            Value::String(R15_ONTOLOGY_VERSION.to_owned()),
        );
        semantic.insert(
            "extractor_contract_version".to_owned(),
            Value::String(R15_EXTRACTION_CONTRACT_VERSION.to_owned()),
        );
        append_extractor_version(semantic, R15_EXTRACTOR_VERSION)?;
        let baseline_chunks = semantic
            .get("extraction_chunks")
            .cloned()
            .ok_or(RepositorySnapshotV17Error::ContractInvalid)?;
        semantic.insert(
            "extraction_chunks".to_owned(),
            Value::Array(extraction_chunks_v14(&baseline_chunks, knowledge)?),
        );
        let baseline_graph = semantic
            .get("knowledge_graph")
            .cloned()
            .ok_or(RepositorySnapshotV17Error::ContractInvalid)?;
        semantic.insert(
            "knowledge_graph".to_owned(),
            knowledge_graph_v14(&baseline_graph, knowledge)?,
        );
        let semantic_value = Value::Object(semantic.clone());
        let root = value
            .as_object_mut()
            .ok_or(RepositorySnapshotV17Error::ContractInvalid)?;
        root.insert(
            "schema_version".to_owned(),
            Value::String(R15_SNAPSHOT_VERSION.to_owned()),
        );
        root.insert(
            "semantic_hash".to_owned(),
            json!({
                "algorithm": "blake3-256",
                "value": semantic_hash(SNAPSHOT_V17_HASH_DOMAIN, &semantic_value)
            }),
        );
        publication_candidate(&value).map_err(|_| RepositorySnapshotV17Error::ContractInvalid)?;
        Ok(Self {
            value,
            output_capacity_profile,
        })
    }

    /// Serializes V17 under its selected 32 MiB or 64 MiB output envelope.
    ///
    /// # Errors
    ///
    /// Returns a serialization or selected output-limit failure.
    pub fn canonical_stdout(&self) -> Result<Vec<u8>, RepositorySnapshotV17Error> {
        let maximum = usize::try_from(self.output_capacity_profile.maximum_bytes())
            .map_err(|_| RepositorySnapshotV17Error::OutputLengthOverflow)?;
        let body_maximum = maximum
            .checked_sub(1)
            .ok_or(RepositorySnapshotV17Error::OutputLengthOverflow)?;
        let mut writer = LimitedVecWriter::new(body_maximum);
        let result = serde_json::to_writer(&mut writer, &self.value);
        if writer.overflowed() {
            return Err(RepositorySnapshotV17Error::LimitExceeded(
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
        result.map_err(RepositorySnapshotV17Error::Serialization)?;
        let mut bytes = writer.into_inner();
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Serializes the exact V17 semantic payload.
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

    /// Converts V17 into the immutable publication model.
    ///
    /// # Errors
    ///
    /// Returns a strict contract or storage-integrity failure.
    pub fn publication_candidate(&self) -> Result<PublicationCandidate, PublicationCandidateError> {
        publication_candidate(&self.value)
    }
}

/// Validates one loaded V17 semantic payload and its R15 derivations against the visible head.
///
/// # Errors
///
/// Returns a typed storage-integrity failure on any mismatch.
pub fn validate_stored_snapshot_semantic_v17(
    semantic: &Value,
    head: &LocalSnapshotHead,
) -> Result<(), StorageError> {
    if head.snapshot_schema_version != SNAPSHOT_SCHEMA_VERSION_V17 {
        return Err(stored_snapshot_error(
            head,
            "stored_snapshot_schema_mismatch",
        ));
    }
    validate_stored_r15_semantic(semantic)
        .map_err(|_| stored_snapshot_error(head, "stored_snapshot_contract_invalid"))?;
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

fn validate_stored_r15_semantic(semantic: &Value) -> Result<(), R15ContractError> {
    if semantic.get("pipeline_version").and_then(Value::as_str) != Some(R15_PIPELINE_VERSION)
        || semantic.get("ontology_version").and_then(Value::as_str) != Some(R15_ONTOLOGY_VERSION)
        || semantic
            .pointer("/configuration/schema_version")
            .and_then(Value::as_str)
            != Some(R15_CONFIGURATION_VERSION)
        || semantic
            .pointer("/configuration/rust_flow_profile")
            .and_then(Value::as_str)
            != Some(R15_PROFILE)
        || !semantic
            .get("extractor_versions")
            .and_then(Value::as_array)
            .is_some_and(|versions| {
                versions
                    .iter()
                    .any(|version| version.as_str() == Some(R15_EXTRACTOR_VERSION))
            })
    {
        return Err(R15ContractError::InvalidSnapshot);
    }
    let graph = semantic
        .get("knowledge_graph")
        .and_then(Value::as_object)
        .ok_or(R15ContractError::InvalidSnapshot)?;
    if graph.get("schema_version").and_then(Value::as_str) != Some(R15_GRAPH_VERSION)
        || graph.get("ontology_version").and_then(Value::as_str) != Some(R15_ONTOLOGY_VERSION)
    {
        return Err(R15ContractError::InvalidSnapshot);
    }
    let entity_ids = validate_family(graph, "entities", "id")?;
    let relationship_ids = validate_family(graph, "relationships", "id")?;
    let evidence_ids = validate_family(graph, "evidence", "id")?;
    validate_family(graph, "claims", "id")?;
    validate_family(graph, "diagnostics", "id")?;
    validate_family(graph, "coverage", "id")?;
    for relationship in graph
        .get("relationships")
        .and_then(Value::as_array)
        .ok_or(R15ContractError::InvalidSnapshot)?
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
    for entity in graph
        .get("entities")
        .and_then(Value::as_array)
        .ok_or(R15ContractError::InvalidSnapshot)?
    {
        validate_reference_array_if_present(entity, "evidence_ids", &evidence_ids, "entities")?;
        if let Some(identifier) = entity.get("evidence_id").and_then(Value::as_str)
            && !evidence_ids.contains(identifier)
        {
            return Err(R15ContractError::ReferenceMismatch {
                family: "entities",
                id: identifier.to_owned(),
            });
        }
    }
    for claim in graph
        .get("claims")
        .and_then(Value::as_array)
        .ok_or(R15ContractError::InvalidSnapshot)?
    {
        let subject_id = claim
            .get("subject_id")
            .and_then(Value::as_str)
            .ok_or(R15ContractError::InvalidSnapshot)?;
        let valid_subject = match claim.get("subject_kind").and_then(Value::as_str) {
            Some("entity") => entity_ids.contains(subject_id),
            Some("relationship") => relationship_ids.contains(subject_id),
            _ => false,
        };
        if !valid_subject {
            return Err(R15ContractError::ReferenceMismatch {
                family: "claims",
                id: subject_id.to_owned(),
            });
        }
        validate_reference_array_if_present(claim, "evidence_ids", &evidence_ids, "claims")?;
    }
    for family in ["diagnostics", "coverage"] {
        for record in graph
            .get(family)
            .and_then(Value::as_array)
            .ok_or(R15ContractError::InvalidSnapshot)?
        {
            validate_reference_array_if_present(record, "evidence_ids", &evidence_ids, family)?;
        }
    }
    validate_portable_local_flow_index(graph, &entity_ids, &relationship_ids, &evidence_ids)
}

#[derive(Clone, Debug)]
pub struct LocalQueryResultV12 {
    value: Value,
}

impl LocalQueryResultV12 {
    /// Serializes one bounded exact-ID V12 result followed by LF.
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

/// Builds one exact-ID result with linked R14 and local-flow neighborhoods and derivations.
///
/// # Errors
///
/// Returns a strict snapshot, document, not-found, identity, or result-limit failure.
pub fn local_query_result_v12(
    semantic: &Value,
    manifest: &Value,
    snapshot_id: &str,
    requested_id: &str,
) -> Result<LocalQueryResultV12, QueryContractError> {
    if semantic.get("ontology_version").and_then(Value::as_str) != Some(R15_ONTOLOGY_VERSION)
        || semantic
            .pointer("/knowledge_graph/schema_version")
            .and_then(Value::as_str)
            != Some(R15_GRAPH_VERSION)
    {
        return Err(QueryContractError::InvalidSnapshot);
    }
    let mut compatible = semantic.clone();
    compatible["ontology_version"] =
        Value::String(codenoesis_domain::s4_r14::R14_ONTOLOGY_VERSION.to_owned());
    compatible["knowledge_graph"]["schema_version"] =
        Value::String(codenoesis_domain::s4_r14::R14_GRAPH_VERSION.to_owned());
    compatible["knowledge_graph"]["ontology_version"] =
        Value::String(codenoesis_domain::s4_r14::R14_ONTOLOGY_VERSION.to_owned());
    let mut value = local_query_result_v11(&compatible, manifest, snapshot_id, requested_id)?
        .value()
        .clone();
    let graph = semantic
        .get("knowledge_graph")
        .and_then(Value::as_object)
        .ok_or(QueryContractError::InvalidSnapshot)?;
    let entities = id_value_map_query(graph, "entities")?;
    let relationships = graph
        .get("relationships")
        .and_then(Value::as_array)
        .ok_or(QueryContractError::InvalidSnapshot)?;
    let mut linked_flow_relationships = relationships
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
                .is_some_and(is_local_flow_relationship_kind)
        })
        .cloned()
        .collect::<Vec<_>>();
    sort_dedup_records_query(&mut linked_flow_relationships)?;
    let mut endpoint_ids = BTreeSet::new();
    for relationship in &linked_flow_relationships {
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
    let mut linked_flow_entities = endpoint_ids
        .iter()
        .filter_map(|identifier| entities.get(identifier.as_str()).copied())
        .filter(|entity| {
            entity.get("kind").and_then(Value::as_str) == Some("rust.syntax_basic_block")
        })
        .cloned()
        .collect::<Vec<_>>();
    sort_dedup_records_query(&mut linked_flow_entities)?;
    let related_relationship_ids = linked_flow_relationships
        .iter()
        .filter_map(record_id)
        .chain(std::iter::once(requested_id))
        .collect::<BTreeSet<_>>();
    let mut linked_derivations = graph
        .get("local_flow_index")
        .and_then(|index| index.get("derivations"))
        .and_then(Value::as_array)
        .ok_or(QueryContractError::InvalidSnapshot)?
        .iter()
        .filter(|derivation| {
            derivation
                .get("relationship_id")
                .and_then(Value::as_str)
                .is_some_and(|identifier| related_relationship_ids.contains(identifier))
        })
        .cloned()
        .collect::<Vec<_>>();
    linked_derivations.sort_by(|left, right| {
        left.get("relationship_id")
            .and_then(Value::as_str)
            .cmp(&right.get("relationship_id").and_then(Value::as_str))
    });
    value["schema_version"] = Value::String(R15_QUERY_VERSION.to_owned());
    value["linked_flow_entities"] = Value::Array(linked_flow_entities);
    value["linked_flow_relationships"] = Value::Array(linked_flow_relationships);
    value["linked_derivations"] = Value::Array(linked_derivations);
    let result = LocalQueryResultV12 { value };
    result.canonical_stdout()?;
    Ok(result)
}

fn extraction_chunks_v14(
    baseline: &Value,
    knowledge: &LocalFlowKnowledge,
) -> Result<Vec<Value>, RepositorySnapshotV17Error> {
    let mut additions = knowledge
        .extraction_chunks
        .iter()
        .map(|chunk| (chunk.source_file_id.as_str(), chunk))
        .collect::<BTreeMap<_, _>>();
    let chunks = baseline
        .as_array()
        .ok_or(RepositorySnapshotV17Error::ContractInvalid)?;
    let mut transformed = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        let mut value = chunk.clone();
        let object = value
            .as_object_mut()
            .ok_or(RepositorySnapshotV17Error::ContractInvalid)?;
        object.insert(
            "schema_version".to_owned(),
            Value::String(R15_EXTRACTION_CHUNK_VERSION.to_owned()),
        );
        object.insert(
            "ontology_version".to_owned(),
            Value::String(R15_ONTOLOGY_VERSION.to_owned()),
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
                .ok_or(RepositorySnapshotV17Error::ContractInvalid)?
                .to_owned();
            let flow = additions
                .remove(source_file_id.as_str())
                .ok_or(RepositorySnapshotV17Error::ContractInvalid)?;
            object.insert(
                "local_flow_profile".to_owned(),
                Value::String(R15_PROFILE.to_owned()),
            );
            merge_id_array(
                object,
                "entities",
                flow.blocks.iter().map(syntax_basic_block_value),
            )?;
            merge_id_array(
                object,
                "relationships",
                flow.relationships.iter().map(local_flow_relationship_value),
            )?;
            merge_id_array(object, "claims", flow.claims.iter().map(claim_value))?;
            merge_id_array(object, "evidence", flow.evidence.iter().map(evidence_value))?;
            merge_id_array(
                object,
                "coverage",
                flow.coverage.iter().map(local_flow_coverage_value),
            )?;
        }
        insert_semantic_hash(&mut value, EXTRACTION_V14_HASH_DOMAIN)?;
        transformed.push(value);
    }
    if !additions.is_empty() {
        return Err(RepositorySnapshotV17Error::ContractInvalid);
    }
    Ok(transformed)
}

fn knowledge_graph_v14(
    baseline: &Value,
    knowledge: &LocalFlowKnowledge,
) -> Result<Value, RepositorySnapshotV17Error> {
    let mut value = baseline.clone();
    let object = value
        .as_object_mut()
        .ok_or(RepositorySnapshotV17Error::ContractInvalid)?;
    object.insert(
        "schema_version".to_owned(),
        Value::String(R15_GRAPH_VERSION.to_owned()),
    );
    object.insert(
        "ontology_version".to_owned(),
        Value::String(R15_ONTOLOGY_VERSION.to_owned()),
    );
    append_extractor_version(object, R15_EXTRACTOR_VERSION)?;
    object.insert(
        "local_flow_index".to_owned(),
        local_flow_index_value(&knowledge.graph.index),
    );
    object.remove("semantic_hash");
    merge_id_array(
        object,
        "entities",
        knowledge.graph.blocks.iter().map(syntax_basic_block_value),
    )?;
    merge_id_array(
        object,
        "relationships",
        knowledge
            .graph
            .relationships
            .iter()
            .map(local_flow_relationship_value),
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
            .map(local_flow_coverage_value),
    )?;
    validate_graph_overlay(object, knowledge)?;
    insert_semantic_hash(&mut value, GRAPH_V14_HASH_DOMAIN)?;
    Ok(value)
}

fn syntax_basic_block_value(block: &SyntaxBasicBlock) -> Value {
    json!({
        "id": block.id,
        "kind": "rust.syntax_basic_block",
        "callable_id": block.callable_id,
        "source_file_id": block.source_file_id,
        "evidence_id": block.evidence_id,
        "locator": {
            "path": block.locator.path,
            "blob_oid": block.locator.blob_oid,
            "start_byte": block.locator.start_byte,
            "end_byte": block.locator.end_byte
        },
        "properties": {
            "ordinal": block.ordinal,
            "role": block.role.as_str(),
            "flow_node_ids": block.flow_node_ids,
            "flow_world": "syntax_normal_completion"
        }
    })
}

fn local_flow_relationship_value(relationship: &LocalFlowRelationship) -> Value {
    json!({
        "id": relationship.id,
        "kind": relationship.kind.as_str(),
        "source": relationship.source,
        "target": relationship.target,
        "evidence_ids": relationship.evidence_ids
    })
}

fn local_flow_coverage_value(gap: &LocalFlowCoverageGap) -> Value {
    json!({
        "id": gap.id,
        "capability": gap.capability,
        "state": gap.state,
        "subject_id": gap.subject_id,
        "evidence_ids": gap.evidence_ids
    })
}

fn local_flow_index_value(index: &LocalFlowIndex) -> Value {
    json!({
        "schema_version": R15_INDEX_VERSION,
        "rule_version": R15_RULE_VERSION,
        "completed_callable_ids": index.completed_callable_ids,
        "block_entity_ids": index.block_entity_ids,
        "flow_node_relationship_ids": index.flow_node_relationship_ids,
        "condition_relationship_ids": index.condition_relationship_ids,
        "direct_syntax_relationship_ids": index.direct_syntax_relationship_ids,
        "reachability_relationship_ids": index.reachability_relationship_ids,
        "must_reach_relationship_ids": index.must_reach_relationship_ids,
        "may_reach_relationship_ids": index.may_reach_relationship_ids,
        "derivations": index.derivations.iter().map(local_flow_derivation_value).collect::<Vec<_>>()
    })
}

fn local_flow_derivation_value(derivation: &LocalFlowDerivation) -> Value {
    json!({
        "relationship_id": derivation.relationship_id,
        "rule_version": R15_RULE_VERSION,
        "input_entity_ids": derivation.input_entity_ids,
        "input_relationship_ids": derivation.input_relationship_ids,
        "input_evidence_ids": derivation.input_evidence_ids
    })
}

fn validate_graph_overlay(
    graph: &Map<String, Value>,
    knowledge: &LocalFlowKnowledge,
) -> Result<(), RepositorySnapshotV17Error> {
    let entities = family_id_set(graph, "entities")?;
    let relationships = family_id_set(graph, "relationships")?;
    let claims = family_id_map(graph, "claims")?;
    let evidence = family_id_set(graph, "evidence")?;
    for block in &knowledge.graph.blocks {
        if !entities.contains(&block.id) || !evidence.contains(&block.evidence_id) {
            return Err(RepositorySnapshotV17Error::ContractInvalid);
        }
    }
    for relationship in &knowledge.graph.relationships {
        if !relationships.contains(&relationship.id)
            || relationship
                .evidence_ids
                .iter()
                .any(|identifier| !evidence.contains(identifier))
            || !claims.values().any(|claim| {
                claim.get("subject_id").and_then(Value::as_str) == Some(relationship.id.as_str())
            })
        {
            return Err(RepositorySnapshotV17Error::ContractInvalid);
        }
    }
    Ok(())
}

fn append_extractor_version(
    object: &mut Map<String, Value>,
    version: &str,
) -> Result<(), RepositorySnapshotV17Error> {
    let versions = object
        .get_mut("extractor_versions")
        .and_then(Value::as_array_mut)
        .ok_or(RepositorySnapshotV17Error::ContractInvalid)?;
    if versions.iter().any(|value| value.as_str() == Some(version)) {
        return Err(RepositorySnapshotV17Error::ContractInvalid);
    }
    versions.push(Value::String(version.to_owned()));
    Ok(())
}

fn merge_id_array(
    object: &mut Map<String, Value>,
    field: &'static str,
    additions: impl IntoIterator<Item = Value>,
) -> Result<(), RepositorySnapshotV17Error> {
    let values = object
        .get_mut(field)
        .and_then(Value::as_array_mut)
        .ok_or(RepositorySnapshotV17Error::ContractInvalid)?;
    values.extend(additions);
    values.sort_by(|left, right| record_id(left).cmp(&record_id(right)));
    let mut retained = Vec::with_capacity(values.len());
    for value in values.drain(..) {
        if let Some(previous) = retained.last()
            && record_id(previous) == record_id(&value)
        {
            if previous != &value {
                return Err(RepositorySnapshotV17Error::ContractInvalid);
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
) -> Result<(), RepositorySnapshotV17Error> {
    value
        .as_object_mut()
        .ok_or(RepositorySnapshotV17Error::ContractInvalid)?
        .remove("semantic_hash");
    let hash = semantic_hash(domain, value);
    value
        .as_object_mut()
        .ok_or(RepositorySnapshotV17Error::ContractInvalid)?
        .insert(
            "semantic_hash".to_owned(),
            json!({"algorithm": "blake3-256", "value": hash}),
        );
    Ok(())
}

fn family_id_set(
    graph: &Map<String, Value>,
    family: &'static str,
) -> Result<BTreeSet<String>, RepositorySnapshotV17Error> {
    Ok(family_id_map(graph, family)?
        .into_keys()
        .map(str::to_owned)
        .collect())
}

fn family_id_map<'a>(
    graph: &'a Map<String, Value>,
    family: &'static str,
) -> Result<BTreeMap<&'a str, &'a Value>, RepositorySnapshotV17Error> {
    let mut values = BTreeMap::new();
    let mut previous = None;
    for value in graph
        .get(family)
        .and_then(Value::as_array)
        .ok_or(RepositorySnapshotV17Error::ContractInvalid)?
    {
        let identifier = record_id(value).ok_or(RepositorySnapshotV17Error::ContractInvalid)?;
        if previous.is_some_and(|previous| previous >= identifier)
            || values.insert(identifier, value).is_some()
        {
            return Err(RepositorySnapshotV17Error::ContractInvalid);
        }
        previous = Some(identifier);
    }
    Ok(values)
}

fn map_r14_snapshot_error(error: RepositorySnapshotV16Error) -> RepositorySnapshotV17Error {
    match error {
        RepositorySnapshotV16Error::Serialization(error) => {
            RepositorySnapshotV17Error::Serialization(error)
        }
        RepositorySnapshotV16Error::LimitExceeded(error) => {
            RepositorySnapshotV17Error::LimitExceeded(error)
        }
        RepositorySnapshotV16Error::ContractInvalid => RepositorySnapshotV17Error::ContractInvalid,
        RepositorySnapshotV16Error::OutputLengthOverflow => {
            RepositorySnapshotV17Error::OutputLengthOverflow
        }
    }
}

#[derive(Clone, Debug)]
pub struct PortableGraphV8 {
    value: Value,
    canonical: Vec<u8>,
    sha256: R15Sha256,
}

impl PortableGraphV8 {
    /// Projects one validated V17 head and deterministic documentation manifest.
    ///
    /// # Errors
    ///
    /// Returns a strict binding, privacy, derivation, reference, canonicality, or limit failure.
    pub fn from_validated_v17(
        semantic: &Value,
        head: &LocalSnapshotHead,
        documentation_manifest: &Value,
        sha256: R15Sha256,
    ) -> Result<Self, R15ContractError> {
        validate_stored_snapshot_semantic_v17(semantic, head)
            .map_err(|_| R15ContractError::InvalidSnapshot)?;
        if semantic.get("ontology_version").and_then(Value::as_str) != Some(R15_ONTOLOGY_VERSION) {
            return Err(R15ContractError::InvalidSnapshot);
        }
        validate_documentation_binding(documentation_manifest, head)?;
        let graph = semantic
            .get("knowledge_graph")
            .and_then(Value::as_object)
            .ok_or(R15ContractError::InvalidSnapshot)?;
        let (documents, document_statements) = portable_documents(documentation_manifest)?;
        let mut value = json!({
            "schema_version": R15_PORTABLE_GRAPH_VERSION,
            "repository": semantic.get("repository").cloned().ok_or(R15ContractError::InvalidSnapshot)?,
            "source_snapshot": {
                "schema_version": R15_SNAPSHOT_VERSION,
                "snapshot_id": head.snapshot_id.as_str(),
                "semantic_hash": {
                    "algorithm": head.semantic_hash.algorithm,
                    "value": head.semantic_hash.value
                }
            },
            "ontology_version": R15_ONTOLOGY_VERSION,
            "query_contract_version": R15_QUERY_VERSION,
            "projection": {
                "profile": "codenoesis.lossless-portable-projection/v8",
                "family_sha256": {}
            },
            "local_flow_index": graph.get("local_flow_index").cloned().ok_or(R15ContractError::InvalidSnapshot)?,
            "entities": graph.get("entities").cloned().ok_or(R15ContractError::InvalidSnapshot)?,
            "relationships": graph.get("relationships").cloned().ok_or(R15ContractError::InvalidSnapshot)?,
            "claims": graph.get("claims").cloned().ok_or(R15ContractError::InvalidSnapshot)?,
            "evidence": graph.get("evidence").cloned().ok_or(R15ContractError::InvalidSnapshot)?,
            "diagnostics": graph.get("diagnostics").cloned().ok_or(R15ContractError::InvalidSnapshot)?,
            "coverage_gaps": graph.get("coverage").cloned().ok_or(R15ContractError::InvalidSnapshot)?,
            "documents": documents,
            "document_statements": document_statements
        });
        value["projection"]["family_sha256"] = family_digests(&value, sha256)?;
        Self::from_generated_value(value, sha256)
    }

    /// Strictly reimports one canonical LF-terminated `PortableGraphV8`.
    ///
    /// # Errors
    ///
    /// Returns the first decode, schema, identity, derivation, privacy, or limit failure.
    pub fn from_canonical_file(bytes: &[u8], sha256: R15Sha256) -> Result<Self, R15ContractError> {
        enforce_portable_size(bytes.len())?;
        let value = serde_json::from_slice::<Value>(bytes)
            .map_err(|_| R15ContractError::InvalidProjection)?;
        validate_portable_value(&value, sha256)?;
        let canonical = serde_json::to_vec(&value).map_err(|_| R15ContractError::Internal)?;
        let mut expected = canonical.clone();
        expected.push(b'\n');
        if expected != bytes {
            return Err(R15ContractError::Noncanonical {
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

    fn from_generated_value(value: Value, sha256: R15Sha256) -> Result<Self, R15ContractError> {
        validate_portable_value(&value, sha256)?;
        let canonical = serde_json::to_vec(&value).map_err(|_| R15ContractError::Internal)?;
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
pub struct LocalExplorerManifestV8 {
    value: Value,
}

impl LocalExplorerManifestV8 {
    /// Builds the offline V8 explorer manifest bound to exact graph and K1 viewer bytes.
    ///
    /// # Errors
    ///
    /// Returns an integrity or unsafe-CSP failure.
    pub fn new(
        portable: &PortableGraphV8,
        viewer_bytes: &[u8],
        expected_viewer_sha256: &str,
        content_security_policy: &str,
        sha256: R15Sha256,
    ) -> Result<Self, R15ContractError> {
        if sha256(viewer_bytes) != expected_viewer_sha256
            || content_security_policy.contains("http:")
            || content_security_policy.contains("https:")
            || content_security_policy.contains("unsafe-inline")
            || content_security_policy.contains("unsafe-eval")
            || !balanced_script_elements(viewer_bytes)
        {
            return Err(R15ContractError::AssetIntegrityMismatch);
        }
        Ok(Self {
            value: json!({
                "schema_version": R15_LOCAL_EXPLORER_VERSION,
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
                    "profile": R15_EXPLORER_SECURITY_PROFILE,
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
                    "bounded_traversal": [1, 2],
                    "local_flow_derivations": true
                },
                "limits": {
                    "text_search_results": MAX_R15_TEXT_SEARCH_RESULTS,
                    "traversal_depth_default": R15_TRAVERSAL_DEPTH_DEFAULT,
                    "traversal_depth_maximum": MAX_R15_TRAVERSAL_DEPTH
                }
            }),
        })
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Serializes one `LocalExplorerManifestV8` followed by LF.
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
pub enum R15ContractError {
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

impl Display for R15ContractError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidSnapshot => "invalid R15 snapshot",
            Self::UnsupportedSnapshotSchema(_) => "unsupported R15 snapshot schema",
            Self::UnsupportedPortableGraphSchema(_) => "unsupported portable graph schema",
            Self::Noncanonical { .. } => "noncanonical portable graph",
            Self::IdentityConflict { .. } => "portable graph identity conflict",
            Self::ReferenceMismatch { .. } => "portable graph reference mismatch",
            Self::LimitExceeded { .. } => "portable graph limit exceeded",
            Self::UnsafePayload { .. } => "unsafe portable payload",
            Self::AssetIntegrityMismatch => "explorer asset integrity mismatch",
            Self::InvalidProjection => "invalid portable graph projection",
            Self::Internal => "internal R15 contract failure",
        })
    }
}

impl Error for R15ContractError {}

fn validate_documentation_binding(
    manifest: &Value,
    head: &LocalSnapshotHead,
) -> Result<(), R15ContractError> {
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
        return Err(R15ContractError::InvalidSnapshot);
    }
    Ok(())
}

fn portable_documents(manifest: &Value) -> Result<(Vec<Value>, Vec<Value>), R15ContractError> {
    let source = manifest
        .get("documents")
        .and_then(Value::as_array)
        .ok_or(R15ContractError::InvalidSnapshot)?;
    let mut documents = Vec::with_capacity(source.len());
    let mut statements = Vec::new();
    for document in source {
        let mut record = document
            .as_object()
            .cloned()
            .ok_or(R15ContractError::InvalidSnapshot)?;
        let document_id = record
            .get("document_id")
            .and_then(Value::as_str)
            .ok_or(R15ContractError::InvalidSnapshot)?
            .to_owned();
        let document_statements = record
            .remove("statements")
            .and_then(|value| value.as_array().cloned())
            .ok_or(R15ContractError::InvalidSnapshot)?;
        documents.push(Value::Object(record));
        for statement in document_statements {
            let mut statement = statement
                .as_object()
                .cloned()
                .ok_or(R15ContractError::InvalidSnapshot)?;
            if statement
                .insert("document_id".to_owned(), Value::String(document_id.clone()))
                .is_some()
            {
                return Err(R15ContractError::InvalidSnapshot);
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

fn family_digests(value: &Value, sha256: R15Sha256) -> Result<Value, R15ContractError> {
    let mut digests = Map::new();
    for family in PORTABLE_FAMILIES {
        let bytes = serde_json::to_vec(
            value
                .get(family)
                .ok_or(R15ContractError::InvalidProjection)?,
        )
        .map_err(|_| R15ContractError::Internal)?;
        digests.insert(family.to_owned(), Value::String(sha256(&bytes)));
    }
    Ok(Value::Object(digests))
}

#[allow(clippy::too_many_lines)]
fn validate_portable_value(value: &Value, sha256: R15Sha256) -> Result<(), R15ContractError> {
    ensure_nesting(value, 0)?;
    validate_private_fields(value)?;
    let object = value
        .as_object()
        .ok_or(R15ContractError::InvalidProjection)?;
    let expected_keys = BTreeSet::from([
        "claims",
        "coverage_gaps",
        "diagnostics",
        "document_statements",
        "documents",
        "entities",
        "evidence",
        "local_flow_index",
        "ontology_version",
        "projection",
        "query_contract_version",
        "relationships",
        "repository",
        "schema_version",
        "source_snapshot",
    ]);
    if object.keys().map(String::as_str).collect::<BTreeSet<_>>() != expected_keys {
        return Err(R15ContractError::InvalidProjection);
    }
    let schema = object
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or(R15ContractError::InvalidProjection)?;
    if schema != R15_PORTABLE_GRAPH_VERSION {
        return Err(R15ContractError::UnsupportedPortableGraphSchema(bounded(
            schema, 256,
        )));
    }
    if object.get("ontology_version").and_then(Value::as_str) != Some(R15_ONTOLOGY_VERSION)
        || object.get("query_contract_version").and_then(Value::as_str) != Some(R15_QUERY_VERSION)
        || value
            .pointer("/source_snapshot/schema_version")
            .and_then(Value::as_str)
            != Some(R15_SNAPSHOT_VERSION)
        || value.pointer("/projection/profile").and_then(Value::as_str)
            != Some("codenoesis.lossless-portable-projection/v8")
    {
        return Err(R15ContractError::InvalidProjection);
    }
    let repository_identity = value
        .pointer("/repository/identity")
        .and_then(Value::as_str)
        .filter(|value| value.starts_with("urn:codenoesis:"))
        .ok_or(R15ContractError::InvalidProjection)?;
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
        .ok_or(R15ContractError::InvalidProjection)?
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
        .ok_or(R15ContractError::InvalidProjection)?
    {
        let subject_id = claim
            .get("subject_id")
            .and_then(Value::as_str)
            .ok_or(R15ContractError::InvalidProjection)?;
        let valid = match claim.get("subject_kind").and_then(Value::as_str) {
            Some("entity") => entity_ids.contains(subject_id),
            Some("relationship") => relationship_ids.contains(subject_id),
            _ => false,
        };
        if !valid {
            return Err(R15ContractError::ReferenceMismatch {
                family: "claims",
                id: subject_id.to_owned(),
            });
        }
        validate_reference_array_if_present(claim, "evidence_ids", &evidence_ids, "claims")?;
    }
    for family in ["entities", "diagnostics", "coverage_gaps"] {
        for record in object[family]
            .as_array()
            .ok_or(R15ContractError::InvalidProjection)?
        {
            validate_reference_array_if_present(record, "evidence_ids", &evidence_ids, family)?;
            if let Some(identifier) = record.get("evidence_id").and_then(Value::as_str)
                && !evidence_ids.contains(identifier)
            {
                return Err(R15ContractError::ReferenceMismatch {
                    family,
                    id: identifier.to_owned(),
                });
            }
        }
    }
    for evidence in object["evidence"]
        .as_array()
        .ok_or(R15ContractError::InvalidProjection)?
    {
        validate_evidence_path(evidence)?;
    }
    for document in object["documents"]
        .as_array()
        .ok_or(R15ContractError::InvalidProjection)?
    {
        validate_reference(document, "subject_id", &subject_ids, "documents")?;
        if let Some(path) = document.get("path").and_then(Value::as_str)
            && !safe_relative_path(path)
        {
            return Err(R15ContractError::UnsafePayload {
                reason: "unsafe_document_path",
            });
        }
    }
    for statement in object["document_statements"]
        .as_array()
        .ok_or(R15ContractError::InvalidProjection)?
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
    validate_portable_local_flow_index(object, &entity_ids, &relationship_ids, &evidence_ids)?;
    if value.pointer("/projection/family_sha256") != Some(&family_digests(value, sha256)?) {
        return Err(R15ContractError::InvalidProjection);
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn validate_portable_local_flow_index(
    object: &Map<String, Value>,
    entity_ids: &BTreeSet<String>,
    relationship_ids: &BTreeSet<String>,
    evidence_ids: &BTreeSet<String>,
) -> Result<(), R15ContractError> {
    let index = object
        .get("local_flow_index")
        .and_then(Value::as_object)
        .ok_or(R15ContractError::InvalidProjection)?;
    let expected_keys = BTreeSet::from([
        "block_entity_ids",
        "completed_callable_ids",
        "condition_relationship_ids",
        "derivations",
        "direct_syntax_relationship_ids",
        "flow_node_relationship_ids",
        "may_reach_relationship_ids",
        "must_reach_relationship_ids",
        "reachability_relationship_ids",
        "rule_version",
        "schema_version",
    ]);
    if index.keys().map(String::as_str).collect::<BTreeSet<_>>() != expected_keys
        || index.get("schema_version").and_then(Value::as_str) != Some(R15_INDEX_VERSION)
        || index.get("rule_version").and_then(Value::as_str) != Some(R15_RULE_VERSION)
    {
        return Err(R15ContractError::InvalidProjection);
    }
    let entities = object
        .get("entities")
        .and_then(Value::as_array)
        .ok_or(R15ContractError::InvalidProjection)?;
    let relationships = object
        .get("relationships")
        .and_then(Value::as_array)
        .ok_or(R15ContractError::InvalidProjection)?;
    let expected_blocks = entities
        .iter()
        .filter(|entity| {
            entity.get("kind").and_then(Value::as_str) == Some("rust.syntax_basic_block")
        })
        .filter_map(record_id)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let expected_relationships = |kinds: &[&str]| {
        relationships
            .iter()
            .filter(|relationship| {
                relationship
                    .get("kind")
                    .and_then(Value::as_str)
                    .is_some_and(|kind| kinds.contains(&kind))
            })
            .filter_map(record_id)
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    if string_array(index, "block_entity_ids")? != expected_blocks
        || string_array(index, "flow_node_relationship_ids")?
            != expected_relationships(&["CONTAINS_FLOW_NODE"])
        || string_array(index, "condition_relationship_ids")?
            != expected_relationships(&["HAS_CONDITION"])
        || string_array(index, "direct_syntax_relationship_ids")?
            != expected_relationships(&["SYNTAX_NEXT", "SYNTAX_TRUE_BRANCH", "SYNTAX_FALSE_BRANCH"])
        || string_array(index, "reachability_relationship_ids")?
            != expected_relationships(&["SYNTAX_REACHES"])
        || string_array(index, "must_reach_relationship_ids")?
            != expected_relationships(&["LEXICAL_MUST_REACHES_READ"])
        || string_array(index, "may_reach_relationship_ids")?
            != expected_relationships(&["LEXICAL_MAY_REACHES_READ"])
    {
        return Err(R15ContractError::InvalidProjection);
    }
    let completed = string_array(index, "completed_callable_ids")?;
    if !strictly_ordered(&completed)
        || completed
            .iter()
            .any(|identifier| !entity_ids.contains(identifier))
    {
        return Err(R15ContractError::InvalidProjection);
    }
    let expected_derived = expected_relationships(&[
        "SYNTAX_REACHES",
        "LEXICAL_MUST_REACHES_READ",
        "LEXICAL_MAY_REACHES_READ",
    ]);
    let derivations = index
        .get("derivations")
        .and_then(Value::as_array)
        .ok_or(R15ContractError::InvalidProjection)?;
    if derivations.len() > usize::try_from(MAX_R15_RELATIONSHIPS).unwrap_or(usize::MAX) {
        return Err(R15ContractError::LimitExceeded {
            limit: "local_flow_derivations",
            maximum: MAX_R15_RELATIONSHIPS,
            observed: MAX_R15_RELATIONSHIPS.saturating_add(1),
        });
    }
    let mut observed_derived = Vec::with_capacity(derivations.len());
    let mut input_count = 0_u64;
    for derivation in derivations {
        if derivation.get("rule_version").and_then(Value::as_str) != Some(R15_RULE_VERSION) {
            return Err(R15ContractError::InvalidProjection);
        }
        let relationship_id = derivation
            .get("relationship_id")
            .and_then(Value::as_str)
            .ok_or(R15ContractError::InvalidProjection)?;
        observed_derived.push(relationship_id.to_owned());
        for (field, identifiers) in [
            ("input_entity_ids", entity_ids),
            ("input_relationship_ids", relationship_ids),
            ("input_evidence_ids", evidence_ids),
        ] {
            let values = string_array(
                derivation
                    .as_object()
                    .ok_or(R15ContractError::InvalidProjection)?,
                field,
            )?;
            if values.is_empty()
                || !strictly_ordered(&values)
                || values
                    .iter()
                    .any(|identifier| !identifiers.contains(identifier))
            {
                return Err(R15ContractError::InvalidProjection);
            }
            input_count =
                input_count.saturating_add(u64::try_from(values.len()).unwrap_or(u64::MAX));
        }
        validate_portable_derivation_semantics(derivation, entities, relationships)?;
    }
    if observed_derived != expected_derived {
        return Err(R15ContractError::InvalidProjection);
    }
    if input_count > MAX_R15_DERIVATION_INPUT_REFERENCES {
        return Err(R15ContractError::LimitExceeded {
            limit: "derivation_input_references",
            maximum: MAX_R15_DERIVATION_INPUT_REFERENCES,
            observed: input_count.min(MAX_R15_DERIVATION_INPUT_REFERENCES.saturating_add(1)),
        });
    }
    Ok(())
}

fn validate_portable_derivation_semantics(
    derivation: &Value,
    entities: &[Value],
    relationships: &[Value],
) -> Result<(), R15ContractError> {
    let entity_map = entities
        .iter()
        .map(|entity| {
            Ok((
                record_id(entity).ok_or(R15ContractError::InvalidProjection)?,
                entity,
            ))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let relationship_map = relationships
        .iter()
        .map(|relationship| {
            Ok((
                record_id(relationship).ok_or(R15ContractError::InvalidProjection)?,
                relationship,
            ))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let derivation = derivation
        .as_object()
        .ok_or(R15ContractError::InvalidProjection)?;
    let relationship_id = derivation
        .get("relationship_id")
        .and_then(Value::as_str)
        .ok_or(R15ContractError::InvalidProjection)?;
    let relationship = relationship_map
        .get(relationship_id)
        .copied()
        .ok_or(R15ContractError::InvalidProjection)?;
    let input_entities = string_array(derivation, "input_entity_ids")?;
    let input_relationships = string_array(derivation, "input_relationship_ids")?;
    let input_evidence = string_array(derivation, "input_evidence_ids")?;
    let source = relationship
        .get("source")
        .and_then(Value::as_str)
        .ok_or(R15ContractError::InvalidProjection)?;
    let target = relationship
        .get("target")
        .and_then(Value::as_str)
        .ok_or(R15ContractError::InvalidProjection)?;
    if !input_entities.iter().any(|identifier| identifier == source)
        || !input_entities.iter().any(|identifier| identifier == target)
    {
        return Err(R15ContractError::InvalidProjection);
    }
    let mut expected_evidence = input_entities
        .iter()
        .map(|identifier| {
            entity_map
                .get(identifier.as_str())
                .copied()
                .ok_or(R15ContractError::InvalidProjection)
                .and_then(portable_entity_evidence)
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    expected_evidence.sort();
    expected_evidence.dedup();
    if expected_evidence.is_empty() || input_evidence != expected_evidence {
        return Err(R15ContractError::InvalidProjection);
    }
    match relationship.get("kind").and_then(Value::as_str) {
        Some("SYNTAX_REACHES") => validate_portable_syntax_derivation(
            source,
            target,
            &input_entities,
            &input_relationships,
            relationships,
        ),
        Some("LEXICAL_MUST_REACHES_READ" | "LEXICAL_MAY_REACHES_READ") => {
            validate_portable_lexical_derivation(
                source,
                target,
                &input_entities,
                &input_relationships,
                &entity_map,
                &relationship_map,
            )
        }
        _ => Err(R15ContractError::InvalidProjection),
    }
}

fn portable_entity_evidence(entity: &Value) -> Result<Vec<String>, R15ContractError> {
    let mut evidence = Vec::new();
    if let Some(identifier) = entity.get("evidence_id").and_then(Value::as_str) {
        evidence.push(identifier.to_owned());
    }
    if let Some(identifiers) = entity.get("evidence_ids") {
        evidence.extend(
            identifiers
                .as_array()
                .ok_or(R15ContractError::InvalidProjection)?
                .iter()
                .map(|identifier| {
                    identifier
                        .as_str()
                        .map(str::to_owned)
                        .ok_or(R15ContractError::InvalidProjection)
                })
                .collect::<Result<Vec<_>, _>>()?,
        );
    }
    evidence.sort();
    evidence.dedup();
    if evidence.is_empty() {
        return Err(R15ContractError::InvalidProjection);
    }
    Ok(evidence)
}

fn validate_portable_syntax_derivation(
    source: &str,
    target: &str,
    input_entities: &[String],
    input_relationships: &[String],
    relationships: &[Value],
) -> Result<(), R15ContractError> {
    let direct = relationships
        .iter()
        .filter(|relationship| {
            relationship
                .get("kind")
                .and_then(Value::as_str)
                .is_some_and(|kind| {
                    matches!(
                        kind,
                        "SYNTAX_NEXT" | "SYNTAX_TRUE_BRANCH" | "SYNTAX_FALSE_BRANCH"
                    )
                })
        })
        .collect::<Vec<_>>();
    let mut adjacency = BTreeMap::<&str, Vec<&str>>::new();
    let mut reverse = BTreeMap::<&str, Vec<&str>>::new();
    for relationship in &direct {
        let edge_source = relationship
            .get("source")
            .and_then(Value::as_str)
            .ok_or(R15ContractError::InvalidProjection)?;
        let edge_target = relationship
            .get("target")
            .and_then(Value::as_str)
            .ok_or(R15ContractError::InvalidProjection)?;
        adjacency.entry(edge_source).or_default().push(edge_target);
        reverse.entry(edge_target).or_default().push(edge_source);
    }
    let forward = portable_reachable(source, &adjacency);
    let backward = portable_reachable(target, &reverse);
    let vertices = forward
        .intersection(&backward)
        .copied()
        .chain(std::iter::once(source))
        .chain(std::iter::once(target))
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let expected_entities = vertices.iter().cloned().collect::<Vec<_>>();
    let expected_relationships = direct
        .iter()
        .filter(|relationship| {
            relationship
                .get("source")
                .and_then(Value::as_str)
                .is_some_and(|identifier| vertices.contains(identifier))
                && relationship
                    .get("target")
                    .and_then(Value::as_str)
                    .is_some_and(|identifier| vertices.contains(identifier))
        })
        .map(|relationship| {
            record_id(relationship)
                .map(str::to_owned)
                .ok_or(R15ContractError::InvalidProjection)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if input_entities != expected_entities || input_relationships != expected_relationships {
        return Err(R15ContractError::InvalidProjection);
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn validate_portable_lexical_derivation(
    source: &str,
    target: &str,
    input_entities: &[String],
    input_relationships: &[String],
    entities: &BTreeMap<&str, &Value>,
    relationships: &BTreeMap<&str, &Value>,
) -> Result<(), R15ContractError> {
    let source_entity = entities
        .get(source)
        .copied()
        .ok_or(R15ContractError::InvalidProjection)?;
    let target_entity = entities
        .get(target)
        .copied()
        .ok_or(R15ContractError::InvalidProjection)?;
    let reads = relationships
        .values()
        .copied()
        .filter(|relationship| {
            relationship.get("kind").and_then(Value::as_str) == Some("READS")
                && relationship.get("source").and_then(Value::as_str) == Some(target)
        })
        .collect::<Vec<_>>();
    if reads.len() != 1 {
        return Err(R15ContractError::InvalidProjection);
    }
    let read = reads[0];
    let binding_id = read
        .get("target")
        .and_then(Value::as_str)
        .ok_or(R15ContractError::InvalidProjection)?;
    let writes = relationships
        .values()
        .copied()
        .filter(|relationship| {
            relationship.get("kind").and_then(Value::as_str) == Some("WRITES")
                && relationship.get("source").and_then(Value::as_str) == Some(source)
        })
        .collect::<Vec<_>>();
    if source_entity.get("kind").and_then(Value::as_str) == Some("rust.pattern_binding") {
        if source != binding_id || !writes.is_empty() {
            return Err(R15ContractError::InvalidProjection);
        }
    } else if writes.len() != 1
        || writes[0].get("target").and_then(Value::as_str) != Some(binding_id)
    {
        return Err(R15ContractError::InvalidProjection);
    }
    if source_entity.get("callable_id").and_then(Value::as_str)
        != target_entity.get("callable_id").and_then(Value::as_str)
        || source_entity
            .pointer("/locator/start_byte")
            .and_then(Value::as_u64)
            .zip(
                target_entity
                    .pointer("/locator/start_byte")
                    .and_then(Value::as_u64),
            )
            .is_none_or(|(source_start, target_start)| source_start > target_start)
    {
        return Err(R15ContractError::InvalidProjection);
    }
    let read_id = record_id(read).ok_or(R15ContractError::InvalidProjection)?;
    if !input_relationships
        .iter()
        .any(|identifier| identifier == read_id)
        || writes.first().is_some_and(|write| {
            record_id(write).is_none_or(|write_id| {
                !input_relationships
                    .iter()
                    .any(|identifier| identifier == write_id)
            })
        })
    {
        return Err(R15ContractError::InvalidProjection);
    }
    let input_entities = input_entities
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    for identifier in input_relationships {
        let candidate = relationships
            .get(identifier.as_str())
            .copied()
            .ok_or(R15ContractError::InvalidProjection)?;
        let candidate_source = candidate
            .get("source")
            .and_then(Value::as_str)
            .ok_or(R15ContractError::InvalidProjection)?;
        let candidate_target = candidate
            .get("target")
            .and_then(Value::as_str)
            .ok_or(R15ContractError::InvalidProjection)?;
        let valid = match candidate.get("kind").and_then(Value::as_str) {
            Some("READS") => candidate_source == target && candidate_target == binding_id,
            Some("WRITES") => {
                candidate_target == binding_id && input_entities.contains(candidate_source)
            }
            Some(
                "CONTAINS_FLOW_NODE" | "SYNTAX_NEXT" | "SYNTAX_TRUE_BRANCH" | "SYNTAX_FALSE_BRANCH",
            ) => {
                input_entities.contains(candidate_source)
                    && input_entities.contains(candidate_target)
            }
            _ => false,
        };
        if !valid {
            return Err(R15ContractError::InvalidProjection);
        }
    }
    Ok(())
}

fn portable_reachable<'a>(
    start: &'a str,
    adjacency: &BTreeMap<&'a str, Vec<&'a str>>,
) -> BTreeSet<&'a str> {
    let mut values = BTreeSet::new();
    let mut pending = std::collections::VecDeque::from([start]);
    while let Some(current) = pending.pop_front() {
        for target in adjacency.get(current).into_iter().flatten() {
            if values.insert(*target) {
                pending.push_back(*target);
            }
        }
    }
    values
}

fn validate_family(
    object: &Map<String, Value>,
    family: &'static str,
    id_field: &'static str,
) -> Result<BTreeSet<String>, R15ContractError> {
    let values = object
        .get(family)
        .and_then(Value::as_array)
        .ok_or(R15ContractError::InvalidProjection)?;
    let mut identifiers = BTreeSet::new();
    let mut previous = None;
    for value in values {
        let identifier = value
            .get(id_field)
            .and_then(Value::as_str)
            .filter(|identifier| !identifier.is_empty())
            .ok_or(R15ContractError::InvalidProjection)?;
        if previous.is_some_and(|previous| previous >= identifier) {
            return Err(R15ContractError::IdentityConflict {
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
) -> Result<(), R15ContractError> {
    let identifier = record
        .get(field)
        .and_then(Value::as_str)
        .ok_or(R15ContractError::InvalidProjection)?;
    if !identifiers.contains(identifier) {
        return Err(R15ContractError::ReferenceMismatch {
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
) -> Result<(), R15ContractError> {
    let Some(values) = record.get(field) else {
        return Ok(());
    };
    let values = values
        .as_array()
        .ok_or(R15ContractError::InvalidProjection)?;
    for identifier in values {
        let identifier = identifier
            .as_str()
            .ok_or(R15ContractError::InvalidProjection)?;
        if !identifiers.contains(identifier) {
            return Err(R15ContractError::ReferenceMismatch {
                family,
                id: identifier.to_owned(),
            });
        }
    }
    Ok(())
}

fn validate_evidence_path(evidence: &Value) -> Result<(), R15ContractError> {
    if let Some(path) = evidence.get("path").and_then(Value::as_str) {
        return safe_relative_path(path)
            .then_some(())
            .ok_or(R15ContractError::UnsafePayload {
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
            return Err(R15ContractError::UnsafePayload {
                reason: "unsafe_document_path",
            });
        }
        return Ok(());
    }
    Err(R15ContractError::InvalidProjection)
}

fn validate_private_fields(value: &Value) -> Result<(), R15ContractError> {
    match value {
        Value::Object(fields) => {
            for (field, nested) in fields {
                if matches!(
                    field.as_str(),
                    "body_text"
                        | "condition_text"
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
                    return Err(R15ContractError::UnsafePayload {
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
            return Err(R15ContractError::UnsafePayload { reason: "raw_url" });
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    Ok(())
}

fn ensure_nesting(value: &Value, depth: u64) -> Result<(), R15ContractError> {
    if depth > MAX_R15_JSON_NESTING {
        return Err(R15ContractError::LimitExceeded {
            limit: "json_nesting",
            maximum: MAX_R15_JSON_NESTING,
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

fn enforce_portable_size(length: usize) -> Result<(), R15ContractError> {
    let observed = u64::try_from(length).unwrap_or(u64::MAX);
    if observed > MAX_R15_PORTABLE_GRAPH_BYTES {
        return Err(R15ContractError::LimitExceeded {
            limit: "portable_graph_bytes",
            maximum: MAX_R15_PORTABLE_GRAPH_BYTES,
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

fn id_value_map_query<'a>(
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

fn sort_dedup_records_query(values: &mut [Value]) -> Result<(), QueryContractError> {
    values.sort_by(|left, right| record_id(left).cmp(&record_id(right)));
    for pair in values.windows(2) {
        if record_id(&pair[0]).is_none() || record_id(&pair[0]) == record_id(&pair[1]) {
            return Err(QueryContractError::InvalidSnapshot);
        }
    }
    Ok(())
}

fn is_local_flow_relationship_kind(kind: &str) -> bool {
    matches!(
        kind,
        "HAS_SYNTAX_BLOCK"
            | "CONTAINS_FLOW_NODE"
            | "HAS_CONDITION"
            | "SYNTAX_NEXT"
            | "SYNTAX_TRUE_BRANCH"
            | "SYNTAX_FALSE_BRANCH"
            | "SYNTAX_REACHES"
            | "LEXICAL_MUST_REACHES_READ"
            | "LEXICAL_MAY_REACHES_READ"
    )
}

fn string_array(
    object: &Map<String, Value>,
    field: &'static str,
) -> Result<Vec<String>, R15ContractError> {
    object
        .get(field)
        .and_then(Value::as_array)
        .ok_or(R15ContractError::InvalidProjection)?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or(R15ContractError::InvalidProjection)
        })
        .collect()
}

fn strictly_ordered(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
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
