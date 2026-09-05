use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::path::{Component, Path};

use codenoesis_domain::knowledge::{ClaimState, ClaimSubjectKind};
use codenoesis_domain::s1_boundaries::RepositoryBoundaryReport;
use codenoesis_domain::s4::workspace_claim_id;
use codenoesis_domain::s4_r16::{
    ConstantEvaluationCoverageGap, ConstantEvaluationDerivation, ConstantEvaluationError,
    ConstantEvaluationIndex, ConstantEvaluationKnowledge, ConstantEvaluationRelationship,
    EvaluatedValue, evaluated_value_id, evaluation_relationship_id,
};
pub use codenoesis_domain::s4_r16::{
    MAX_R16_CANDIDATES_PER_SOURCE, MAX_R16_DEPENDENCY_LEVELS, MAX_R16_DEPENDENCY_REFERENCES,
    MAX_R16_DERIVATION_INPUT_REFERENCES, MAX_R16_DIRECT_DEPENDENCIES, MAX_R16_EVALUATED_ENTITIES,
    MAX_R16_EVALUATION_RELATIONSHIPS, MAX_R16_SYNTAX_NODES_PER_EXPRESSION,
    MAX_R16_VARIANTS_PER_ENUM, R16_CONFIGURATION_VERSION, R16_ERROR_VERSION,
    R16_EXTRACTION_CHUNK_VERSION, R16_EXTRACTION_CONTRACT_VERSION, R16_EXTRACTOR_VERSION,
    R16_GRAPH_VERSION, R16_INDEX_VERSION, R16_LOCAL_EXPLORER_VERSION, R16_ONTOLOGY_VERSION,
    R16_PIPELINE_VERSION, R16_PORTABLE_GRAPH_VERSION, R16_PROFILE, R16_QUERY_VERSION,
    R16_RULE_VERSION, R16_SEMANTIC_HASH_CONTRACT_VERSION, R16_SNAPSHOT_VERSION,
};
use codenoesis_domain::storage::{
    LocalSnapshotHead, PublicationCandidate, SNAPSHOT_SCHEMA_VERSION_V18, StorageComponent,
    StorageError,
};
use codenoesis_domain::{
    AcquisitionError, K1OutputCapacityProfile, LimitKind, RepositoryInventory,
};
use serde_json::{Map, Value, json};

use super::s1_boundaries::CodeNoesisErrorV9;
use super::s4::{MAX_QUERY_BYTES, QueryContractError, claim_value};
use super::s4_r15::{RepositorySnapshotV17, RepositorySnapshotV17Error, local_query_result_v12};
use super::{
    LimitedVecWriter, PublicationCandidateError, SnapshotEnvelopeV1, publication_candidate,
    semantic_hash,
};

const CONFIGURATION_V15_HASH_DOMAIN: &[u8] = b"codenoesis.configuration.semantic.v15";
const SNAPSHOT_V18_HASH_DOMAIN: &[u8] = b"codenoesis.repository-snapshot.semantic.v18";
const EXTRACTION_V15_HASH_DOMAIN: &[u8] = b"codenoesis.extraction-chunk.semantic.v15";
const GRAPH_V15_HASH_DOMAIN: &[u8] = b"codenoesis.knowledge-graph.semantic.v15";
const PORTABLE_FAMILIES: [&str; 10] = [
    "entities",
    "relationships",
    "claims",
    "evidence",
    "diagnostics",
    "coverage_gaps",
    "documents",
    "document_statements",
    "local_flow_index",
    "constant_evaluation_index",
];

pub type R16Sha256 = fn(&[u8]) -> String;
pub const R16_PORTABLE_MARKER: &str = ".codenoesis-portable-graph-v9";
pub const R16_EXPLORER_MARKER: &str = ".codenoesis-local-explorer-v9";
pub const R16_EXPLORER_SECURITY_PROFILE: &str = "codenoesis.local-explorer-security/v9";
pub const MAX_R16_PORTABLE_GRAPH_BYTES: u64 = 268_435_456;
pub const MAX_R16_JSON_NESTING: u64 = 64;
pub const MAX_R16_TEXT_SEARCH_RESULTS: u64 = 100;
pub const R16_TRAVERSAL_DEPTH_DEFAULT: u64 = 1;
pub const MAX_R16_TRAVERSAL_DEPTH: u64 = 2;

#[derive(Clone, Debug)]
pub struct CodeNoesisErrorV24 {
    value: Value,
}

impl CodeNoesisErrorV24 {
    #[must_use]
    pub fn from_boundary_error(error: &CodeNoesisErrorV9) -> Self {
        let mut value = error.value().clone();
        value["schema_version"] = Value::String(R16_ERROR_VERSION.to_owned());
        Self { value }
    }

    #[must_use]
    pub fn invalid_profile(profile: &str) -> Self {
        Self::new(
            "input.invalid_rust_constant_profile",
            "input",
            "invalid rust constant profile",
            json!({"profile": bounded_nonempty(profile, 256, "missing")}),
        )
    }

    #[must_use]
    pub fn unsupported_composition(reason: &str) -> Self {
        Self::new(
            "input.unsupported_rust_constant_evaluation_composition",
            "input",
            "unsupported rust constant-evaluation profile composition",
            json!({
                "rust_flow_profile": codenoesis_domain::s4_r15::R15_PROFILE,
                "rust_constant_profile": R16_PROFILE,
                "reason": bounded_nonempty(reason, 128, "unsupported_composition")
            }),
        )
    }

    #[must_use]
    pub fn from_constant_evaluation(error: &ConstantEvaluationError) -> Self {
        match error {
            ConstantEvaluationError::Source(error) => {
                let inherited = super::s4_r15::CodeNoesisErrorV22::from_local_flow(error);
                let mut value = inherited.value().clone();
                value["schema_version"] = Value::String(R16_ERROR_VERSION.to_owned());
                Self { value }
            }
            ConstantEvaluationError::IdentityConflict => Self::extraction(
                "extraction.constant_identity_conflict",
                "constant identity conflict",
            ),
            ConstantEvaluationError::ValueInvalid => Self::extraction(
                "extraction.constant_value_invalid",
                "constant value invalid",
            ),
            ConstantEvaluationError::DependencyInvalid => Self::extraction(
                "extraction.constant_dependency_invalid",
                "constant dependency invalid",
            ),
            ConstantEvaluationError::DependencyCycle => Self::extraction(
                "extraction.constant_dependency_cycle",
                "constant dependency cycle",
            ),
            ConstantEvaluationError::DerivationMismatch => Self::extraction(
                "extraction.constant_derivation_mismatch",
                "constant derivation mismatch",
            ),
            ConstantEvaluationError::IndexMismatch => Self::extraction(
                "extraction.constant_index_mismatch",
                "constant index mismatch",
            ),
            ConstantEvaluationError::LimitExceeded {
                limit,
                maximum,
                observed,
            } => Self::new(
                "extraction.constant_limit_exceeded",
                "extraction",
                "constant-evaluation limit exceeded",
                json!({
                    "limit": limit.as_str(),
                    "maximum": maximum,
                    "observed": observed
                }),
            ),
            ConstantEvaluationError::ContractInvalid => Self::extraction(
                "extraction.constant_contract_invalid",
                "constant-evaluation contract invalid",
            ),
        }
    }

    #[must_use]
    pub fn from_contract(error: &R16ContractError) -> Self {
        match error {
            R16ContractError::LimitExceeded {
                limit,
                maximum,
                observed,
            } => Self::new(
                "export.limit_exceeded",
                "export",
                "portable graph limit exceeded",
                json!({"limit": limit, "maximum": maximum, "observed": observed}),
            ),
            R16ContractError::AssetIntegrityMismatch => Self::new(
                "explorer.asset_integrity_mismatch",
                "explorer",
                "local explorer asset integrity mismatch",
                json!({}),
            ),
            R16ContractError::InvalidSnapshot => Self::new(
                "export.invalid_snapshot",
                "export",
                "invalid R16 source snapshot",
                json!({}),
            ),
            R16ContractError::Internal => Self::internal("contract"),
            _ => Self::new(
                "export.invalid_portable_graph_v9",
                "export",
                "invalid portable graph v9",
                json!({}),
            ),
        }
    }

    #[must_use]
    pub fn from_explorer_contract(error: &R16ContractError) -> Self {
        match error {
            R16ContractError::LimitExceeded {
                limit,
                maximum,
                observed,
            } => Self::new(
                "explorer.limit_exceeded",
                "explorer",
                "local explorer input limit exceeded",
                json!({"limit": limit, "maximum": maximum, "observed": observed}),
            ),
            R16ContractError::AssetIntegrityMismatch => Self::new(
                "explorer.asset_integrity_mismatch",
                "explorer",
                "local explorer asset integrity mismatch",
                json!({}),
            ),
            R16ContractError::Internal => Self::internal("explorer"),
            _ => Self::new(
                "export.invalid_portable_graph_v9",
                "export",
                "invalid portable graph v9",
                json!({}),
            ),
        }
    }

    #[must_use]
    pub fn invalid_snapshot() -> Self {
        Self::new(
            "store.invalid_snapshot_v18",
            "store",
            "invalid stored R16 snapshot",
            json!({}),
        )
    }

    #[must_use]
    pub fn invalid_query() -> Self {
        Self::new(
            "query.invalid_local_query_v13",
            "query",
            "invalid R16 local query",
            json!({}),
        )
    }

    #[must_use]
    pub fn unsafe_output_path(path_sha256: &str, reason: &str) -> Self {
        Self::new(
            "input.unsafe_output_path",
            "input",
            "unsafe output path",
            json!({
                "path_sha256": bounded_nonempty(path_sha256, 64, "missing"),
                "reason": bounded_nonempty(reason, 128, "unsafe_output")
            }),
        )
    }

    #[must_use]
    pub fn acquisition_limit(error: &AcquisitionError) -> Self {
        let (limit, maximum, observed) = match error {
            AcquisitionError::LimitExceeded {
                limit,
                maximum,
                observed,
            } => (limit.as_str(), *maximum, *observed),
            _ => ("repository", 0, 0),
        };
        Self::new(
            "input.repository_limit_exceeded",
            "input",
            "repository input limit exceeded",
            json!({"limit": limit, "maximum": maximum, "observed": observed}),
        )
    }

    #[must_use]
    pub fn internal(stage: &str) -> Self {
        Self::new("internal.failure", stage, "internal failure", json!({}))
    }

    fn extraction(code: &str, message: &str) -> Self {
        Self::new(code, "extraction", message, json!({}))
    }

    fn new(code: &str, stage: &str, message: &str, context: Value) -> Self {
        let mut value = json!({
            "schema_version": R16_ERROR_VERSION,
            "code": code,
            "stage": stage,
            "message": message,
            "retryable": false,
            "context": null
        });
        value["context"] = context;
        Self { value }
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Serializes one `ErrorV24` followed by LF.
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
pub struct RepositorySnapshotV18 {
    value: Value,
    output_capacity_profile: K1OutputCapacityProfile,
}

#[derive(Debug)]
pub enum RepositorySnapshotV18Error {
    Serialization(serde_json::Error),
    LimitExceeded(AcquisitionError),
    ContractInvalid,
    OutputLengthOverflow,
}

impl Display for RepositorySnapshotV18Error {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Serialization(_) => "R16 snapshot serialization failed",
            Self::LimitExceeded(_) => "R16 snapshot output limit exceeded",
            Self::ContractInvalid => "R16 snapshot contract is invalid",
            Self::OutputLengthOverflow => "R16 snapshot output length overflowed",
        })
    }
}

impl Error for RepositorySnapshotV18Error {}

impl RepositorySnapshotV18 {
    /// Builds the additive R16 constant-value overlay over the exact R15 lineage.
    ///
    /// # Errors
    ///
    /// Returns the first R15, constant, identity, serialization, or publication failure.
    pub fn from_inventory_and_constant_evaluation(
        inventory: &RepositoryInventory,
        knowledge: &ConstantEvaluationKnowledge,
        output_capacity_profile: K1OutputCapacityProfile,
        envelope: SnapshotEnvelopeV1,
    ) -> Result<Self, RepositorySnapshotV18Error> {
        Self::from_inventory_constant_evaluation_and_boundaries(
            inventory,
            knowledge,
            None,
            output_capacity_profile,
            envelope,
        )
    }

    /// Builds R16 over the historical R15 lineage or its additive R12 boundary composition.
    ///
    /// # Errors
    ///
    /// Returns the first R15, constant, boundary, serialization, or publication failure.
    pub fn from_inventory_constant_evaluation_and_boundaries(
        inventory: &RepositoryInventory,
        knowledge: &ConstantEvaluationKnowledge,
        boundaries: Option<&RepositoryBoundaryReport>,
        output_capacity_profile: K1OutputCapacityProfile,
        envelope: SnapshotEnvelopeV1,
    ) -> Result<Self, RepositorySnapshotV18Error> {
        knowledge
            .validate()
            .map_err(|_| RepositorySnapshotV18Error::ContractInvalid)?;
        let mut value = RepositorySnapshotV17::value_for_successor(
            inventory,
            &knowledge.local_flow,
            boundaries,
            output_capacity_profile,
            envelope,
        )
        .map_err(map_r15_snapshot_error)?;
        let semantic = value
            .get_mut("semantic")
            .and_then(Value::as_object_mut)
            .ok_or(RepositorySnapshotV18Error::ContractInvalid)?;
        let mut configuration = semantic
            .get("configuration")
            .and_then(Value::as_object)
            .cloned()
            .ok_or(RepositorySnapshotV18Error::ContractInvalid)?;
        configuration.insert(
            "schema_version".to_owned(),
            Value::String(R16_CONFIGURATION_VERSION.to_owned()),
        );
        configuration.insert(
            "rust_constant_profile".to_owned(),
            Value::String(R16_PROFILE.to_owned()),
        );
        configuration.remove("semantic_hash");
        let configuration_value = Value::Object(configuration.clone());
        configuration.insert(
            "semantic_hash".to_owned(),
            json!({
                "algorithm": "blake3-256",
                "value": semantic_hash(CONFIGURATION_V15_HASH_DOMAIN, &configuration_value)
            }),
        );
        semantic.insert("configuration".to_owned(), Value::Object(configuration));
        semantic.insert(
            "pipeline_version".to_owned(),
            Value::String(R16_PIPELINE_VERSION.to_owned()),
        );
        semantic.insert(
            "ontology_version".to_owned(),
            Value::String(R16_ONTOLOGY_VERSION.to_owned()),
        );
        semantic.insert(
            "extractor_contract_version".to_owned(),
            Value::String(R16_EXTRACTION_CONTRACT_VERSION.to_owned()),
        );
        append_extractor_version(semantic, R16_EXTRACTOR_VERSION)?;
        let baseline_chunks = semantic
            .get("extraction_chunks")
            .cloned()
            .ok_or(RepositorySnapshotV18Error::ContractInvalid)?;
        semantic.insert(
            "extraction_chunks".to_owned(),
            Value::Array(extraction_chunks_v15(&baseline_chunks, knowledge)?),
        );
        let baseline_graph = semantic
            .get("knowledge_graph")
            .cloned()
            .ok_or(RepositorySnapshotV18Error::ContractInvalid)?;
        semantic.insert(
            "knowledge_graph".to_owned(),
            knowledge_graph_v15(&baseline_graph, knowledge)?,
        );
        let semantic_value = Value::Object(semantic.clone());
        let root = value
            .as_object_mut()
            .ok_or(RepositorySnapshotV18Error::ContractInvalid)?;
        root.insert(
            "schema_version".to_owned(),
            Value::String(R16_SNAPSHOT_VERSION.to_owned()),
        );
        root.insert(
            "semantic_hash".to_owned(),
            json!({
                "algorithm": "blake3-256",
                "value": semantic_hash(SNAPSHOT_V18_HASH_DOMAIN, &semantic_value)
            }),
        );
        publication_candidate(&value).map_err(|_| RepositorySnapshotV18Error::ContractInvalid)?;
        Ok(Self {
            value,
            output_capacity_profile,
        })
    }

    /// Serializes V18 under its selected bounded output envelope.
    ///
    /// # Errors
    ///
    /// Returns a serialization or selected output-limit failure.
    pub fn canonical_stdout(&self) -> Result<Vec<u8>, RepositorySnapshotV18Error> {
        let maximum = usize::try_from(self.output_capacity_profile.maximum_bytes())
            .map_err(|_| RepositorySnapshotV18Error::OutputLengthOverflow)?;
        let body_maximum = maximum
            .checked_sub(1)
            .ok_or(RepositorySnapshotV18Error::OutputLengthOverflow)?;
        let mut writer = LimitedVecWriter::new(body_maximum);
        let result = serde_json::to_writer(&mut writer, &self.value);
        if writer.overflowed() {
            return Err(RepositorySnapshotV18Error::LimitExceeded(
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
        result.map_err(RepositorySnapshotV18Error::Serialization)?;
        let mut bytes = writer.into_inner();
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Serializes the exact V18 semantic payload.
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

    /// Converts V18 into the immutable publication model.
    ///
    /// # Errors
    ///
    /// Returns a strict contract or storage-integrity failure.
    pub fn publication_candidate(&self) -> Result<PublicationCandidate, PublicationCandidateError> {
        publication_candidate(&self.value)
    }
}

/// Validates one loaded V18 semantic payload against the visible head.
///
/// # Errors
///
/// Returns a typed storage-integrity failure on any mismatch.
pub fn validate_stored_snapshot_semantic_v18(
    semantic: &Value,
    head: &LocalSnapshotHead,
) -> Result<(), StorageError> {
    if head.snapshot_schema_version != SNAPSHOT_SCHEMA_VERSION_V18 {
        return Err(stored_snapshot_error(
            head,
            "stored_snapshot_schema_mismatch",
        ));
    }
    validate_stored_r16_semantic(semantic)
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

fn validate_stored_r16_semantic(semantic: &Value) -> Result<(), R16ContractError> {
    if semantic.get("pipeline_version").and_then(Value::as_str) != Some(R16_PIPELINE_VERSION)
        || semantic.get("ontology_version").and_then(Value::as_str) != Some(R16_ONTOLOGY_VERSION)
        || semantic
            .pointer("/configuration/schema_version")
            .and_then(Value::as_str)
            != Some(R16_CONFIGURATION_VERSION)
        || semantic
            .pointer("/configuration/rust_constant_profile")
            .and_then(Value::as_str)
            != Some(R16_PROFILE)
        || !semantic
            .get("extractor_versions")
            .and_then(Value::as_array)
            .is_some_and(|versions| {
                versions
                    .iter()
                    .any(|version| version.as_str() == Some(R16_EXTRACTOR_VERSION))
            })
    {
        return Err(R16ContractError::InvalidSnapshot);
    }
    let graph = semantic
        .get("knowledge_graph")
        .and_then(Value::as_object)
        .ok_or(R16ContractError::InvalidSnapshot)?;
    validate_graph_v15(graph)
}

#[derive(Clone, Debug)]
pub struct LocalQueryResultV13 {
    value: Value,
}

impl LocalQueryResultV13 {
    /// Serializes one bounded exact-ID V13 result followed by LF.
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

/// Builds one exact-ID result with linked R15 and constant-evaluation derivations.
///
/// # Errors
///
/// Returns a strict snapshot, document, not-found, identity, or result-limit failure.
pub fn local_query_result_v13(
    semantic: &Value,
    manifest: &Value,
    snapshot_id: &str,
    requested_id: &str,
) -> Result<LocalQueryResultV13, QueryContractError> {
    if semantic.get("ontology_version").and_then(Value::as_str) != Some(R16_ONTOLOGY_VERSION)
        || semantic
            .pointer("/knowledge_graph/schema_version")
            .and_then(Value::as_str)
            != Some(R16_GRAPH_VERSION)
    {
        return Err(QueryContractError::InvalidSnapshot);
    }
    let mut compatible = semantic.clone();
    compatible["ontology_version"] =
        Value::String(codenoesis_domain::s4_r15::R15_ONTOLOGY_VERSION.to_owned());
    compatible["knowledge_graph"]["schema_version"] =
        Value::String(codenoesis_domain::s4_r15::R15_GRAPH_VERSION.to_owned());
    compatible["knowledge_graph"]["ontology_version"] =
        Value::String(codenoesis_domain::s4_r15::R15_ONTOLOGY_VERSION.to_owned());
    let mut value = local_query_result_v12(&compatible, manifest, snapshot_id, requested_id)?
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
    let mut linked_constant_relationships = relationships
        .iter()
        .filter(|relationship| {
            relationship.get("kind").and_then(Value::as_str) == Some("EVALUATES_TO")
        })
        .filter(|relationship| {
            ["id", "source", "target"]
                .iter()
                .any(|field| relationship.get(*field).and_then(Value::as_str) == Some(requested_id))
        })
        .cloned()
        .collect::<Vec<_>>();
    sort_records(&mut linked_constant_relationships)?;
    let mut endpoint_ids = BTreeSet::new();
    for relationship in &linked_constant_relationships {
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
    let mut linked_constant_entities = endpoint_ids
        .iter()
        .filter_map(|identifier| entities.get(identifier.as_str()).copied())
        .filter(|entity| entity.get("kind").and_then(Value::as_str) == Some("rust.evaluated_value"))
        .cloned()
        .collect::<Vec<_>>();
    sort_records(&mut linked_constant_entities)?;
    let related_ids = linked_constant_relationships
        .iter()
        .filter_map(record_id)
        .chain(std::iter::once(requested_id))
        .collect::<BTreeSet<_>>();
    let mut linked_constant_derivations = graph
        .get("constant_evaluation_index")
        .and_then(|index| index.get("derivations"))
        .and_then(Value::as_array)
        .ok_or(QueryContractError::InvalidSnapshot)?
        .iter()
        .filter(|derivation| {
            ["entity_id", "relationship_id"].iter().any(|field| {
                derivation
                    .get(*field)
                    .and_then(Value::as_str)
                    .is_some_and(|identifier| related_ids.contains(identifier))
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    linked_constant_derivations.sort_by(|left, right| {
        left.get("entity_id")
            .and_then(Value::as_str)
            .cmp(&right.get("entity_id").and_then(Value::as_str))
    });
    value["schema_version"] = Value::String(R16_QUERY_VERSION.to_owned());
    value["linked_constant_entities"] = Value::Array(linked_constant_entities);
    value["linked_constant_relationships"] = Value::Array(linked_constant_relationships);
    value["linked_constant_derivations"] = Value::Array(linked_constant_derivations);
    let result = LocalQueryResultV13 { value };
    result.canonical_stdout()?;
    Ok(result)
}

fn extraction_chunks_v15(
    baseline: &Value,
    knowledge: &ConstantEvaluationKnowledge,
) -> Result<Vec<Value>, RepositorySnapshotV18Error> {
    let mut additions = knowledge
        .source_overlays
        .iter()
        .map(|overlay| (overlay.source_file_id.as_str(), overlay))
        .collect::<BTreeMap<_, _>>();
    let chunks = baseline
        .as_array()
        .ok_or(RepositorySnapshotV18Error::ContractInvalid)?;
    let mut transformed = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        let mut value = chunk.clone();
        let object = value
            .as_object_mut()
            .ok_or(RepositorySnapshotV18Error::ContractInvalid)?;
        object.insert(
            "schema_version".to_owned(),
            Value::String(R16_EXTRACTION_CHUNK_VERSION.to_owned()),
        );
        object.insert(
            "ontology_version".to_owned(),
            Value::String(R16_ONTOLOGY_VERSION.to_owned()),
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
            let overlay = additions
                .remove(source_file_id.as_str())
                .ok_or(RepositorySnapshotV18Error::ContractInvalid)?;
            object.insert(
                "constant_evaluation_profile".to_owned(),
                Value::String(R16_PROFILE.to_owned()),
            );
            merge_id_array(
                object,
                "entities",
                overlay.entities.iter().map(evaluated_value),
            )?;
            merge_id_array(
                object,
                "relationships",
                overlay.relationships.iter().map(evaluation_relationship),
            )?;
            merge_id_array(object, "claims", overlay.claims.iter().map(claim_value))?;
            remove_ids(object, "coverage", &overlay.removed_coverage_ids)?;
            remove_ids(object, "diagnostics", &overlay.removed_diagnostic_ids)?;
            merge_id_array(
                object,
                "coverage",
                overlay.coverage.iter().map(constant_coverage),
            )?;
        }
        insert_semantic_hash(&mut value, EXTRACTION_V15_HASH_DOMAIN)?;
        transformed.push(value);
    }
    if !additions.is_empty() {
        return Err(RepositorySnapshotV18Error::ContractInvalid);
    }
    Ok(transformed)
}

fn knowledge_graph_v15(
    baseline: &Value,
    knowledge: &ConstantEvaluationKnowledge,
) -> Result<Value, RepositorySnapshotV18Error> {
    let mut value = baseline.clone();
    let object = value
        .as_object_mut()
        .ok_or(RepositorySnapshotV18Error::ContractInvalid)?;
    object.insert(
        "schema_version".to_owned(),
        Value::String(R16_GRAPH_VERSION.to_owned()),
    );
    object.insert(
        "ontology_version".to_owned(),
        Value::String(R16_ONTOLOGY_VERSION.to_owned()),
    );
    append_extractor_version(object, R16_EXTRACTOR_VERSION)?;
    object.insert(
        "constant_evaluation_index".to_owned(),
        constant_index(&knowledge.graph.index),
    );
    object.remove("semantic_hash");
    remove_ids(object, "coverage", &knowledge.graph.removed_coverage_ids)?;
    remove_ids(
        object,
        "diagnostics",
        &knowledge.graph.removed_diagnostic_ids,
    )?;
    merge_id_array(
        object,
        "entities",
        knowledge.graph.entities.iter().map(evaluated_value),
    )?;
    merge_id_array(
        object,
        "relationships",
        knowledge
            .graph
            .relationships
            .iter()
            .map(evaluation_relationship),
    )?;
    merge_id_array(
        object,
        "claims",
        knowledge.graph.claims.iter().map(claim_value),
    )?;
    merge_id_array(
        object,
        "coverage",
        knowledge.graph.coverage.iter().map(constant_coverage),
    )?;
    validate_graph_v15(object).map_err(|_| RepositorySnapshotV18Error::ContractInvalid)?;
    insert_semantic_hash(&mut value, GRAPH_V15_HASH_DOMAIN)?;
    Ok(value)
}

fn evaluated_value(value: &EvaluatedValue) -> Value {
    json!({
        "id": value.id,
        "kind": "rust.evaluated_value",
        "declared_value_id": value.declared_value_id,
        "properties": {
            "canonical_value": value.canonical_value,
            "rule_version": R16_RULE_VERSION,
            "rust_type": value.rust_type,
            "type_authority": value.type_authority.as_str(),
            "value_kind": value.value_kind.as_str()
        }
    })
}

fn evaluation_relationship(value: &ConstantEvaluationRelationship) -> Value {
    json!({
        "id": value.id,
        "kind": "EVALUATES_TO",
        "source": value.source,
        "target": value.target,
        "evidence_ids": value.evidence_ids
    })
}

fn constant_coverage(value: &ConstantEvaluationCoverageGap) -> Value {
    json!({
        "id": value.id,
        "capability": value.capability,
        "state": value.state,
        "subject_id": value.subject_id,
        "evidence_ids": value.evidence_ids
    })
}

fn constant_index(value: &ConstantEvaluationIndex) -> Value {
    json!({
        "schema_version": R16_INDEX_VERSION,
        "rule_version": R16_RULE_VERSION,
        "evaluated_entity_ids": value.evaluated_entity_ids,
        "evaluation_relationship_ids": value.evaluation_relationship_ids,
        "derivations": value.derivations.iter().map(constant_derivation).collect::<Vec<_>>()
    })
}

fn constant_derivation(value: &ConstantEvaluationDerivation) -> Value {
    json!({
        "entity_id": value.entity_id,
        "relationship_id": value.relationship_id,
        "rule_version": R16_RULE_VERSION,
        "input_claim_ids": value.input_claim_ids,
        "input_evidence_ids": value.input_evidence_ids,
        "dependency_entity_ids": value.dependency_entity_ids
    })
}

#[allow(clippy::too_many_lines)]
fn validate_graph_v15(graph: &Map<String, Value>) -> Result<(), R16ContractError> {
    if graph.get("schema_version").and_then(Value::as_str) != Some(R16_GRAPH_VERSION)
        || graph.get("ontology_version").and_then(Value::as_str) != Some(R16_ONTOLOGY_VERSION)
    {
        return Err(R16ContractError::InvalidSnapshot);
    }
    let entity_ids = validate_family(graph, "entities", "id")?;
    let relationship_ids = validate_family(graph, "relationships", "id")?;
    let claim_ids = validate_family(graph, "claims", "id")?;
    let evidence_ids = validate_family(graph, "evidence", "id")?;
    validate_family(graph, "diagnostics", "id")?;
    validate_family(graph, "coverage", "id")?;
    for relationship in graph
        .get("relationships")
        .and_then(Value::as_array)
        .ok_or(R16ContractError::InvalidSnapshot)?
    {
        validate_reference(relationship, "source", &entity_ids)?;
        validate_reference(relationship, "target", &entity_ids)?;
        validate_reference_array(relationship, "evidence_ids", &evidence_ids)?;
    }
    for claim in graph
        .get("claims")
        .and_then(Value::as_array)
        .ok_or(R16ContractError::InvalidSnapshot)?
    {
        let subject = claim
            .get("subject_id")
            .and_then(Value::as_str)
            .ok_or(R16ContractError::InvalidSnapshot)?;
        let valid = match claim.get("subject_kind").and_then(Value::as_str) {
            Some("entity") => entity_ids.contains(subject),
            Some("relationship") => relationship_ids.contains(subject),
            _ => false,
        };
        if !valid {
            return Err(R16ContractError::ReferenceMismatch(subject.to_owned()));
        }
        validate_reference_array(claim, "evidence_ids", &evidence_ids)?;
    }
    validate_evidence_paths(graph, "evidence")?;
    let index = graph
        .get("constant_evaluation_index")
        .and_then(Value::as_object)
        .ok_or(R16ContractError::InvalidSnapshot)?;
    if index.get("schema_version").and_then(Value::as_str) != Some(R16_INDEX_VERSION)
        || index.get("rule_version").and_then(Value::as_str) != Some(R16_RULE_VERSION)
    {
        return Err(R16ContractError::InvalidProjection);
    }
    let evaluated_ids = string_array(index, "evaluated_entity_ids")?;
    let evaluation_ids = string_array(index, "evaluation_relationship_ids")?;
    if !strictly_ordered(&evaluated_ids)
        || !strictly_ordered(&evaluation_ids)
        || evaluated_ids.iter().any(|identifier| {
            !entity_ids.contains(identifier)
                || graph
                    .get("entities")
                    .and_then(Value::as_array)
                    .is_none_or(|values| {
                        !values.iter().any(|value| {
                            record_id(value) == Some(identifier)
                                && value.get("kind").and_then(Value::as_str)
                                    == Some("rust.evaluated_value")
                        })
                    })
        })
        || evaluation_ids.iter().any(|identifier| {
            !relationship_ids.contains(identifier)
                || graph
                    .get("relationships")
                    .and_then(Value::as_array)
                    .is_none_or(|values| {
                        !values.iter().any(|value| {
                            record_id(value) == Some(identifier)
                                && value.get("kind").and_then(Value::as_str) == Some("EVALUATES_TO")
                        })
                    })
        })
    {
        return Err(R16ContractError::InvalidProjection);
    }
    let derivations = index
        .get("derivations")
        .and_then(Value::as_array)
        .ok_or(R16ContractError::InvalidProjection)?;
    let mut previous = None;
    for derivation in derivations {
        let identifier = derivation
            .get("entity_id")
            .and_then(Value::as_str)
            .ok_or(R16ContractError::InvalidProjection)?;
        if previous.is_some_and(|previous| previous >= identifier)
            || !evaluated_ids.iter().any(|value| value == identifier)
        {
            return Err(R16ContractError::InvalidProjection);
        }
        previous = Some(identifier);
        validate_reference(derivation, "relationship_id", &relationship_ids)?;
        validate_reference_array(derivation, "input_claim_ids", &claim_ids)?;
        validate_reference_array(derivation, "input_evidence_ids", &evidence_ids)?;
        validate_reference_array(derivation, "dependency_entity_ids", &entity_ids)?;
    }
    validate_constant_projection(
        graph,
        &entity_ids,
        &relationship_ids,
        &claim_ids,
        &evidence_ids,
    )?;
    Ok(())
}

#[derive(Clone, Debug)]
pub struct PortableGraphV9 {
    value: Value,
    canonical: Vec<u8>,
    sha256: R16Sha256,
}

impl PortableGraphV9 {
    /// Projects one validated V18 head and deterministic documentation manifest.
    ///
    /// # Errors
    ///
    /// Returns a strict binding, privacy, derivation, reference, canonicality, or limit failure.
    pub fn from_validated_v18(
        semantic: &Value,
        head: &LocalSnapshotHead,
        documentation_manifest: &Value,
        sha256: R16Sha256,
    ) -> Result<Self, R16ContractError> {
        validate_stored_snapshot_semantic_v18(semantic, head)
            .map_err(|_| R16ContractError::InvalidSnapshot)?;
        validate_documentation_binding(documentation_manifest, head)?;
        let graph = semantic
            .get("knowledge_graph")
            .and_then(Value::as_object)
            .ok_or(R16ContractError::InvalidSnapshot)?;
        let (documents, document_statements) = portable_documents(documentation_manifest)?;
        let mut value = json!({
            "schema_version": R16_PORTABLE_GRAPH_VERSION,
            "repository": semantic.get("repository").cloned().ok_or(R16ContractError::InvalidSnapshot)?,
            "source_snapshot": {
                "schema_version": R16_SNAPSHOT_VERSION,
                "snapshot_id": head.snapshot_id.as_str(),
                "semantic_hash": {
                    "algorithm": head.semantic_hash.algorithm,
                    "value": head.semantic_hash.value
                }
            },
            "ontology_version": R16_ONTOLOGY_VERSION,
            "query_contract_version": R16_QUERY_VERSION,
            "projection": {
                "profile": "codenoesis.lossless-portable-projection/v9",
                "family_sha256": {}
            },
            "local_flow_index": graph.get("local_flow_index").cloned().ok_or(R16ContractError::InvalidSnapshot)?,
            "constant_evaluation_index": graph.get("constant_evaluation_index").cloned().ok_or(R16ContractError::InvalidSnapshot)?,
            "entities": graph.get("entities").cloned().ok_or(R16ContractError::InvalidSnapshot)?,
            "relationships": graph.get("relationships").cloned().ok_or(R16ContractError::InvalidSnapshot)?,
            "claims": graph.get("claims").cloned().ok_or(R16ContractError::InvalidSnapshot)?,
            "evidence": graph.get("evidence").cloned().ok_or(R16ContractError::InvalidSnapshot)?,
            "diagnostics": graph.get("diagnostics").cloned().ok_or(R16ContractError::InvalidSnapshot)?,
            "coverage_gaps": graph.get("coverage").cloned().ok_or(R16ContractError::InvalidSnapshot)?,
            "documents": documents,
            "document_statements": document_statements
        });
        if let Some(boundaries) = semantic.get("repository_boundaries") {
            super::s4_r12::validate_boundary_projection(boundaries)
                .map_err(|_| R16ContractError::InvalidSnapshot)?;
            value["repository_boundaries"] = boundaries.clone();
        }
        value["projection"]["family_sha256"] = family_digests(&value, sha256)?;
        Self::from_generated_value(value, sha256)
    }

    /// Strictly reimports one canonical LF-terminated `PortableGraphV9`.
    ///
    /// # Errors
    ///
    /// Returns the first decode, schema, identity, derivation, privacy, or limit failure.
    pub fn from_canonical_file(bytes: &[u8], sha256: R16Sha256) -> Result<Self, R16ContractError> {
        enforce_portable_size(bytes.len())?;
        let value = serde_json::from_slice::<Value>(bytes)
            .map_err(|_| R16ContractError::InvalidProjection)?;
        validate_portable_value(&value, sha256)?;
        let canonical = serde_json::to_vec(&value).map_err(|_| R16ContractError::Internal)?;
        let mut expected = canonical.clone();
        expected.push(b'\n');
        if expected != bytes {
            return Err(R16ContractError::Noncanonical {
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

    fn from_generated_value(value: Value, sha256: R16Sha256) -> Result<Self, R16ContractError> {
        validate_portable_value(&value, sha256)?;
        let canonical = serde_json::to_vec(&value).map_err(|_| R16ContractError::Internal)?;
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
pub struct LocalExplorerManifestV9 {
    value: Value,
}

impl LocalExplorerManifestV9 {
    /// Builds the offline V9 explorer manifest bound to exact graph and K1 viewer bytes.
    ///
    /// # Errors
    ///
    /// Returns an integrity or unsafe-CSP failure.
    pub fn new(
        portable: &PortableGraphV9,
        viewer_bytes: &[u8],
        expected_viewer_sha256: &str,
        content_security_policy: &str,
        sha256: R16Sha256,
    ) -> Result<Self, R16ContractError> {
        if sha256(viewer_bytes) != expected_viewer_sha256
            || content_security_policy.contains("http:")
            || content_security_policy.contains("https:")
            || content_security_policy.contains("unsafe-inline")
            || content_security_policy.contains("unsafe-eval")
            || !balanced_script_elements(viewer_bytes)
            || !safe_viewer_bytes(viewer_bytes)
        {
            return Err(R16ContractError::AssetIntegrityMismatch);
        }
        Ok(Self {
            value: json!({
                "schema_version": R16_LOCAL_EXPLORER_VERSION,
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
                    "profile": R16_EXPLORER_SECURITY_PROFILE,
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
                    "local_flow_derivations": true,
                    "constant_evaluation_derivations": true
                },
                "limits": {
                    "text_search_results": MAX_R16_TEXT_SEARCH_RESULTS,
                    "traversal_depth_default": R16_TRAVERSAL_DEPTH_DEFAULT,
                    "traversal_depth_maximum": MAX_R16_TRAVERSAL_DEPTH
                }
            }),
        })
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Serializes one `LocalExplorerManifestV9` followed by LF.
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
pub enum R16ContractError {
    InvalidSnapshot,
    UnsupportedPortableGraphSchema(String),
    Noncanonical {
        expected_sha256: String,
        observed_sha256: String,
    },
    IdentityConflict(String),
    ReferenceMismatch(String),
    LimitExceeded {
        limit: &'static str,
        maximum: u64,
        observed: u64,
    },
    UnsafePayload(&'static str),
    AssetIntegrityMismatch,
    InvalidProjection,
    Internal,
}

impl Display for R16ContractError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidSnapshot => "invalid R16 snapshot",
            Self::UnsupportedPortableGraphSchema(_) => "unsupported R16 portable graph schema",
            Self::Noncanonical { .. } => "noncanonical R16 portable graph",
            Self::IdentityConflict(_) => "R16 portable graph identity conflict",
            Self::ReferenceMismatch(_) => "R16 portable graph reference mismatch",
            Self::LimitExceeded { .. } => "R16 portable graph limit exceeded",
            Self::UnsafePayload(_) => "unsafe R16 portable payload",
            Self::AssetIntegrityMismatch => "R16 explorer asset integrity mismatch",
            Self::InvalidProjection => "invalid R16 portable graph projection",
            Self::Internal => "internal R16 contract failure",
        })
    }
}

impl Error for R16ContractError {}

#[allow(clippy::too_many_lines)]
fn validate_portable_value(value: &Value, sha256: R16Sha256) -> Result<(), R16ContractError> {
    ensure_nesting(value, 0)?;
    validate_private_fields(value)?;
    let object = value
        .as_object()
        .ok_or(R16ContractError::InvalidProjection)?;
    let mut expected_keys = BTreeSet::from([
        "claims",
        "constant_evaluation_index",
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
    if object.contains_key("repository_boundaries") {
        expected_keys.insert("repository_boundaries");
    }
    if object.keys().map(String::as_str).collect::<BTreeSet<_>>() != expected_keys {
        return Err(R16ContractError::InvalidProjection);
    }
    let schema = object
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or(R16ContractError::InvalidProjection)?;
    if schema != R16_PORTABLE_GRAPH_VERSION {
        return Err(R16ContractError::UnsupportedPortableGraphSchema(bounded(
            schema, 256,
        )));
    }
    if object.get("ontology_version").and_then(Value::as_str) != Some(R16_ONTOLOGY_VERSION)
        || object.get("query_contract_version").and_then(Value::as_str) != Some(R16_QUERY_VERSION)
        || value
            .pointer("/source_snapshot/schema_version")
            .and_then(Value::as_str)
            != Some(R16_SNAPSHOT_VERSION)
        || value.pointer("/projection/profile").and_then(Value::as_str)
            != Some("codenoesis.lossless-portable-projection/v9")
    {
        return Err(R16ContractError::InvalidProjection);
    }
    if let Some(boundaries) = object.get("repository_boundaries") {
        super::s4_r12::validate_boundary_projection(boundaries)
            .map_err(|_| R16ContractError::InvalidProjection)?;
    }
    let entity_ids = validate_family(object, "entities", "id")?;
    let relationship_ids = validate_family(object, "relationships", "id")?;
    let claim_ids = validate_family(object, "claims", "id")?;
    let evidence_ids = validate_family(object, "evidence", "id")?;
    validate_family(object, "diagnostics", "id")?;
    validate_family(object, "coverage_gaps", "id")?;
    validate_family(object, "documents", "document_id")?;
    validate_family(object, "document_statements", "statement_id")?;
    for relationship in object
        .get("relationships")
        .and_then(Value::as_array)
        .ok_or(R16ContractError::InvalidProjection)?
    {
        validate_reference(relationship, "source", &entity_ids)?;
        validate_reference(relationship, "target", &entity_ids)?;
        validate_reference_array(relationship, "evidence_ids", &evidence_ids)?;
    }
    for claim in object
        .get("claims")
        .and_then(Value::as_array)
        .ok_or(R16ContractError::InvalidProjection)?
    {
        let subject_id = claim
            .get("subject_id")
            .and_then(Value::as_str)
            .ok_or(R16ContractError::InvalidProjection)?;
        let valid = match claim.get("subject_kind").and_then(Value::as_str) {
            Some("entity") => entity_ids.contains(subject_id),
            Some("relationship") => relationship_ids.contains(subject_id),
            _ => false,
        };
        if !valid {
            return Err(R16ContractError::ReferenceMismatch(subject_id.to_owned()));
        }
        validate_reference_array(claim, "evidence_ids", &evidence_ids)?;
    }
    validate_evidence_paths(object, "evidence")?;
    validate_constant_projection(
        object,
        &entity_ids,
        &relationship_ids,
        &claim_ids,
        &evidence_ids,
    )?;
    if value.pointer("/projection/family_sha256") != Some(&family_digests(value, sha256)?) {
        return Err(R16ContractError::InvalidProjection);
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn validate_constant_projection(
    object: &Map<String, Value>,
    entity_ids: &BTreeSet<String>,
    relationship_ids: &BTreeSet<String>,
    claim_ids: &BTreeSet<String>,
    evidence_ids: &BTreeSet<String>,
) -> Result<(), R16ContractError> {
    let repository_identity = object
        .get("repository")
        .and_then(|repository| repository.get("identity"))
        .and_then(Value::as_str)
        .ok_or(R16ContractError::InvalidProjection)?;
    let entities = object
        .get("entities")
        .and_then(Value::as_array)
        .ok_or(R16ContractError::InvalidProjection)?;
    let relationships = object
        .get("relationships")
        .and_then(Value::as_array)
        .ok_or(R16ContractError::InvalidProjection)?;
    let claims = object
        .get("claims")
        .and_then(Value::as_array)
        .ok_or(R16ContractError::InvalidProjection)?;
    let entity_by_id = entities
        .iter()
        .map(|value| {
            record_id(value)
                .map(|identifier| (identifier, value))
                .ok_or(R16ContractError::InvalidProjection)
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let relationship_by_id = relationships
        .iter()
        .map(|value| {
            record_id(value)
                .map(|identifier| (identifier, value))
                .ok_or(R16ContractError::InvalidProjection)
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let mut claim_by_subject = BTreeMap::new();
    for claim in claims {
        let subject_kind = claim
            .get("subject_kind")
            .and_then(Value::as_str)
            .ok_or(R16ContractError::InvalidProjection)?;
        let subject_id = claim
            .get("subject_id")
            .and_then(Value::as_str)
            .ok_or(R16ContractError::InvalidProjection)?;
        if claim_by_subject
            .insert((subject_kind, subject_id), claim)
            .is_some()
        {
            return Err(R16ContractError::InvalidProjection);
        }
    }

    let evaluated = entities
        .iter()
        .filter(|value| value.get("kind").and_then(Value::as_str) == Some("rust.evaluated_value"))
        .collect::<Vec<_>>();
    let evaluations = relationships
        .iter()
        .filter(|value| value.get("kind").and_then(Value::as_str) == Some("EVALUATES_TO"))
        .collect::<Vec<_>>();
    enforce_contract_limit(
        "evaluated_entities_total",
        evaluated.len(),
        MAX_R16_EVALUATED_ENTITIES,
    )?;
    enforce_contract_limit(
        "evaluation_relationships_total",
        evaluations.len(),
        MAX_R16_EVALUATION_RELATIONSHIPS,
    )?;
    if evaluated.len() != evaluations.len() {
        return Err(R16ContractError::InvalidProjection);
    }

    let mut expected_entity_ids = Vec::with_capacity(evaluated.len());
    for entity in &evaluated {
        if !has_exact_fields(entity, &["declared_value_id", "id", "kind", "properties"]) {
            return Err(R16ContractError::InvalidProjection);
        }
        let identifier = record_id(entity).ok_or(R16ContractError::InvalidProjection)?;
        let declared_id = entity
            .get("declared_value_id")
            .and_then(Value::as_str)
            .ok_or(R16ContractError::InvalidProjection)?;
        let declared = entity_by_id
            .get(declared_id)
            .copied()
            .ok_or_else(|| R16ContractError::ReferenceMismatch(declared_id.to_owned()))?;
        let semantic_id = declared
            .get("subject_id")
            .and_then(Value::as_str)
            .ok_or(R16ContractError::InvalidProjection)?;
        let semantic = entity_by_id
            .get(semantic_id)
            .copied()
            .ok_or_else(|| R16ContractError::ReferenceMismatch(semantic_id.to_owned()))?;
        let properties = entity
            .get("properties")
            .and_then(Value::as_object)
            .ok_or(R16ContractError::InvalidProjection)?;
        if identifier != evaluated_value_id(repository_identity, declared_id)
            || declared.get("kind").and_then(Value::as_str) != Some("rust.declared_value")
            || !has_exact_map_fields(
                properties,
                &[
                    "canonical_value",
                    "rule_version",
                    "rust_type",
                    "type_authority",
                    "value_kind",
                ],
            )
            || properties.get("rule_version").and_then(Value::as_str) != Some(R16_RULE_VERSION)
            || !valid_constant_properties(properties, semantic)
        {
            return Err(R16ContractError::InvalidProjection);
        }
        validate_derived_claim(
            identifier,
            ClaimSubjectKind::Entity,
            &claim_by_subject,
            evidence_ids,
        )?;
        expected_entity_ids.push(identifier.to_owned());
    }
    if !strictly_ordered(&expected_entity_ids) {
        return Err(R16ContractError::InvalidProjection);
    }

    let mut expected_relationship_ids = Vec::with_capacity(evaluations.len());
    let mut relationship_by_target = BTreeMap::new();
    for relationship in &evaluations {
        if !has_exact_fields(
            relationship,
            &["evidence_ids", "id", "kind", "source", "target"],
        ) {
            return Err(R16ContractError::InvalidProjection);
        }
        let identifier = record_id(relationship).ok_or(R16ContractError::InvalidProjection)?;
        let source = relationship
            .get("source")
            .and_then(Value::as_str)
            .ok_or(R16ContractError::InvalidProjection)?;
        let target = relationship
            .get("target")
            .and_then(Value::as_str)
            .ok_or(R16ContractError::InvalidProjection)?;
        let target_entity = entity_by_id
            .get(target)
            .copied()
            .ok_or_else(|| R16ContractError::ReferenceMismatch(target.to_owned()))?;
        let relationship_evidence = ordered_string_array(relationship, "evidence_ids", false)?;
        let entity_claim = claim_by_subject
            .get(&("entity", target))
            .copied()
            .ok_or(R16ContractError::InvalidProjection)?;
        if identifier != evaluation_relationship_id(source, target)
            || target_entity
                .get("declared_value_id")
                .and_then(Value::as_str)
                != Some(source)
            || string_array_value(entity_claim, "evidence_ids")? != relationship_evidence
            || relationship_by_target
                .insert(target, *relationship)
                .is_some()
        {
            return Err(R16ContractError::InvalidProjection);
        }
        validate_derived_claim(
            identifier,
            ClaimSubjectKind::Relationship,
            &claim_by_subject,
            evidence_ids,
        )?;
        expected_relationship_ids.push(identifier.to_owned());
    }
    if !strictly_ordered(&expected_relationship_ids) {
        return Err(R16ContractError::InvalidProjection);
    }

    let index = object
        .get("constant_evaluation_index")
        .and_then(Value::as_object)
        .ok_or(R16ContractError::InvalidProjection)?;
    if !has_exact_map_fields(
        index,
        &[
            "derivations",
            "evaluated_entity_ids",
            "evaluation_relationship_ids",
            "rule_version",
            "schema_version",
        ],
    ) || index.get("schema_version").and_then(Value::as_str) != Some(R16_INDEX_VERSION)
        || index.get("rule_version").and_then(Value::as_str) != Some(R16_RULE_VERSION)
    {
        return Err(R16ContractError::InvalidProjection);
    }
    let indexed_entities = ordered_string_array_map(index, "evaluated_entity_ids", true)?;
    let indexed_relationships =
        ordered_string_array_map(index, "evaluation_relationship_ids", true)?;
    if indexed_entities != expected_entity_ids || indexed_relationships != expected_relationship_ids
    {
        return Err(R16ContractError::InvalidProjection);
    }
    let derivations = index
        .get("derivations")
        .and_then(Value::as_array)
        .ok_or(R16ContractError::InvalidProjection)?;
    if derivations.len() != expected_entity_ids.len() {
        return Err(R16ContractError::InvalidProjection);
    }
    let mut adjacency = BTreeMap::<String, Vec<String>>::new();
    let mut dependency_references = 0_usize;
    let mut derivation_inputs = 0_usize;
    let mut previous = None;
    let mut represented_relationships = BTreeSet::new();
    for derivation in derivations {
        if !has_exact_fields(
            derivation,
            &[
                "dependency_entity_ids",
                "entity_id",
                "input_claim_ids",
                "input_evidence_ids",
                "relationship_id",
                "rule_version",
            ],
        ) || derivation.get("rule_version").and_then(Value::as_str) != Some(R16_RULE_VERSION)
        {
            return Err(R16ContractError::InvalidProjection);
        }
        let entity_id = derivation
            .get("entity_id")
            .and_then(Value::as_str)
            .ok_or(R16ContractError::InvalidProjection)?;
        if previous.is_some_and(|previous| previous >= entity_id)
            || !expected_entity_ids.iter().any(|value| value == entity_id)
        {
            return Err(R16ContractError::InvalidProjection);
        }
        previous = Some(entity_id);
        let relationship_id = derivation
            .get("relationship_id")
            .and_then(Value::as_str)
            .ok_or(R16ContractError::InvalidProjection)?;
        let relationship = relationship_by_id
            .get(relationship_id)
            .copied()
            .ok_or_else(|| R16ContractError::ReferenceMismatch(relationship_id.to_owned()))?;
        if relationship.get("target").and_then(Value::as_str) != Some(entity_id)
            || !represented_relationships.insert(relationship_id)
        {
            return Err(R16ContractError::InvalidProjection);
        }
        let dependencies = ordered_string_array(derivation, "dependency_entity_ids", true)?;
        enforce_contract_limit(
            "direct_dependencies_per_subject",
            dependencies.len(),
            MAX_R16_DIRECT_DEPENDENCIES,
        )?;
        if dependencies
            .iter()
            .any(|identifier| !expected_entity_ids.iter().any(|value| value == identifier))
        {
            return Err(R16ContractError::ReferenceMismatch(
                dependencies
                    .iter()
                    .find(|identifier| {
                        !expected_entity_ids.iter().any(|value| value == *identifier)
                    })
                    .cloned()
                    .unwrap_or_default(),
            ));
        }
        let input_claims = ordered_string_array(derivation, "input_claim_ids", false)?;
        let input_evidence = ordered_string_array(derivation, "input_evidence_ids", false)?;
        if input_claims
            .iter()
            .any(|identifier| !claim_ids.contains(identifier))
            || input_evidence
                .iter()
                .any(|identifier| !evidence_ids.contains(identifier))
        {
            return Err(R16ContractError::InvalidProjection);
        }
        let entity = entity_by_id
            .get(entity_id)
            .copied()
            .ok_or_else(|| R16ContractError::ReferenceMismatch(entity_id.to_owned()))?;
        let declared_id = entity
            .get("declared_value_id")
            .and_then(Value::as_str)
            .ok_or(R16ContractError::InvalidProjection)?;
        let declared_claim = claim_by_subject
            .get(&("entity", declared_id))
            .copied()
            .and_then(record_id)
            .ok_or(R16ContractError::InvalidProjection)?;
        let mut expected_claims = vec![declared_claim.to_owned()];
        expected_claims.extend(dependencies.iter().map(|identifier| {
            workspace_claim_id(
                ClaimSubjectKind::Entity,
                identifier,
                ClaimState::DerivedFact,
            )
        }));
        expected_claims.sort();
        expected_claims.dedup();
        let entity_claim = claim_by_subject
            .get(&("entity", entity_id))
            .copied()
            .ok_or(R16ContractError::InvalidProjection)?;
        if input_claims != expected_claims
            || input_evidence != string_array_value(entity_claim, "evidence_ids")?
        {
            return Err(R16ContractError::InvalidProjection);
        }
        dependency_references = dependency_references.saturating_add(dependencies.len());
        derivation_inputs = derivation_inputs
            .saturating_add(input_claims.len())
            .saturating_add(input_evidence.len())
            .saturating_add(dependencies.len());
        adjacency.insert(entity_id.to_owned(), dependencies);
    }
    enforce_contract_limit(
        "evaluation_dependency_references",
        dependency_references,
        MAX_R16_DEPENDENCY_REFERENCES,
    )?;
    enforce_contract_limit(
        "derivation_input_references",
        derivation_inputs,
        MAX_R16_DERIVATION_INPUT_REFERENCES,
    )?;
    validate_constant_acyclic(&adjacency)?;
    if !expected_entity_ids
        .iter()
        .all(|identifier| entity_ids.contains(identifier))
        || !expected_relationship_ids
            .iter()
            .all(|identifier| relationship_ids.contains(identifier))
    {
        return Err(R16ContractError::InvalidProjection);
    }
    Ok(())
}

fn validate_derived_claim<'a>(
    subject_id: &str,
    subject_kind: ClaimSubjectKind,
    claims: &BTreeMap<(&'a str, &'a str), &'a Value>,
    evidence_ids: &BTreeSet<String>,
) -> Result<(), R16ContractError> {
    let claim = claims
        .get(&(subject_kind.as_str(), subject_id))
        .copied()
        .ok_or(R16ContractError::InvalidProjection)?;
    let evidence = ordered_string_array(claim, "evidence_ids", false)?;
    if !has_exact_fields(
        claim,
        &["evidence_ids", "id", "state", "subject_id", "subject_kind"],
    ) || record_id(claim)
        != Some(workspace_claim_id(subject_kind, subject_id, ClaimState::DerivedFact).as_str())
        || claim.get("state").and_then(Value::as_str) != Some("derived_fact")
        || evidence
            .iter()
            .any(|identifier| !evidence_ids.contains(identifier))
    {
        return Err(R16ContractError::InvalidProjection);
    }
    Ok(())
}

fn validate_evidence_paths(
    object: &Map<String, Value>,
    field: &'static str,
) -> Result<(), R16ContractError> {
    for evidence in object
        .get(field)
        .and_then(Value::as_array)
        .ok_or(R16ContractError::InvalidProjection)?
    {
        let path = evidence
            .get("path")
            .and_then(Value::as_str)
            .ok_or(R16ContractError::InvalidProjection)?;
        if !safe_relative_path(path) {
            return Err(R16ContractError::UnsafePayload("unsafe_evidence_path"));
        }
    }
    Ok(())
}

fn valid_constant_properties(properties: &Map<String, Value>, semantic: &Value) -> bool {
    let Some(value_kind) = properties.get("value_kind").and_then(Value::as_str) else {
        return false;
    };
    let Some(canonical_value) = properties.get("canonical_value").and_then(Value::as_str) else {
        return false;
    };
    let Some(rust_type) = properties.get("rust_type").and_then(Value::as_str) else {
        return false;
    };
    let Some(authority) = properties.get("type_authority").and_then(Value::as_str) else {
        return false;
    };
    let semantic_kind = semantic.get("kind").and_then(Value::as_str);
    let authority_valid = match authority {
        "explicit_primitive_annotation" => {
            matches!(semantic_kind, Some("rust.constant" | "rust.static"))
        }
        "fixed_repr_attribute" => semantic_kind == Some("rust.enum_variant"),
        _ => false,
    };
    authority_valid
        && match value_kind {
            "boolean" => {
                rust_type == "bool"
                    && authority == "explicit_primitive_annotation"
                    && matches!(canonical_value, "true" | "false")
            }
            "integer" => {
                matches!(
                    rust_type,
                    "i8" | "i16" | "i32" | "i64" | "i128" | "u8" | "u16" | "u32" | "u64" | "u128"
                ) && canonical_integer_value(canonical_value)
                    && constant_integer_in_range(canonical_value, rust_type)
            }
            _ => false,
        }
}

fn canonical_integer_value(value: &str) -> bool {
    let digits = value.strip_prefix('-').unwrap_or(value);
    !digits.is_empty()
        && digits.bytes().all(|byte| byte.is_ascii_digit())
        && (digits == "0" || !digits.starts_with('0'))
        && value != "-0"
}

fn constant_integer_in_range(value: &str, rust_type: &str) -> bool {
    match rust_type.as_bytes().first() {
        Some(b'i') => value.parse::<i128>().ok().is_some_and(|parsed| {
            rust_type[1..].parse::<u32>().ok().is_some_and(|bits| {
                if bits == 128 {
                    true
                } else {
                    let maximum = (1_i128 << (bits - 1)) - 1;
                    parsed >= -maximum - 1 && parsed <= maximum
                }
            })
        }),
        Some(b'u') => value.parse::<u128>().ok().is_some_and(|parsed| {
            rust_type[1..]
                .parse::<u32>()
                .ok()
                .is_some_and(|bits| bits == 128 || parsed < (1_u128 << bits))
        }),
        _ => false,
    }
}

fn has_exact_fields(value: &Value, expected: &[&str]) -> bool {
    value
        .as_object()
        .is_some_and(|object| has_exact_map_fields(object, expected))
}

fn has_exact_map_fields(object: &Map<String, Value>, expected: &[&str]) -> bool {
    object.keys().map(String::as_str).collect::<BTreeSet<_>>()
        == expected.iter().copied().collect::<BTreeSet<_>>()
}

fn ordered_string_array(
    value: &Value,
    field: &'static str,
    allow_empty: bool,
) -> Result<Vec<String>, R16ContractError> {
    let object = value
        .as_object()
        .ok_or(R16ContractError::InvalidProjection)?;
    ordered_string_array_map(object, field, allow_empty)
}

fn ordered_string_array_map(
    object: &Map<String, Value>,
    field: &'static str,
    allow_empty: bool,
) -> Result<Vec<String>, R16ContractError> {
    let values = string_array(object, field)?;
    if (!allow_empty && values.is_empty()) || !strictly_ordered(&values) {
        return Err(R16ContractError::InvalidProjection);
    }
    Ok(values)
}

fn string_array_value(value: &Value, field: &'static str) -> Result<Vec<String>, R16ContractError> {
    let object = value
        .as_object()
        .ok_or(R16ContractError::InvalidProjection)?;
    string_array(object, field)
}

fn enforce_contract_limit(
    limit: &'static str,
    observed: usize,
    maximum: u64,
) -> Result<(), R16ContractError> {
    let observed = u64::try_from(observed).unwrap_or(u64::MAX);
    if observed > maximum {
        return Err(R16ContractError::LimitExceeded {
            limit,
            maximum,
            observed,
        });
    }
    Ok(())
}

fn validate_constant_acyclic(
    adjacency: &BTreeMap<String, Vec<String>>,
) -> Result<(), R16ContractError> {
    fn visit<'a>(
        identifier: &'a str,
        depth: u64,
        adjacency: &'a BTreeMap<String, Vec<String>>,
        active: &mut BTreeSet<&'a str>,
        complete: &mut BTreeSet<&'a str>,
    ) -> Result<(), R16ContractError> {
        if depth > MAX_R16_DEPENDENCY_LEVELS {
            return Err(R16ContractError::LimitExceeded {
                limit: "dependency_levels",
                maximum: MAX_R16_DEPENDENCY_LEVELS,
                observed: depth,
            });
        }
        if complete.contains(identifier) {
            return Ok(());
        }
        if !active.insert(identifier) {
            return Err(R16ContractError::InvalidProjection);
        }
        for dependency in adjacency.get(identifier).into_iter().flatten() {
            visit(
                dependency,
                depth.saturating_add(1),
                adjacency,
                active,
                complete,
            )?;
        }
        active.remove(identifier);
        complete.insert(identifier);
        Ok(())
    }

    let mut complete = BTreeSet::new();
    for identifier in adjacency.keys() {
        visit(
            identifier,
            0,
            adjacency,
            &mut BTreeSet::new(),
            &mut complete,
        )?;
    }
    Ok(())
}

fn validate_documentation_binding(
    manifest: &Value,
    head: &LocalSnapshotHead,
) -> Result<(), R16ContractError> {
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
        return Err(R16ContractError::InvalidSnapshot);
    }
    Ok(())
}

fn portable_documents(manifest: &Value) -> Result<(Vec<Value>, Vec<Value>), R16ContractError> {
    let source = manifest
        .get("documents")
        .and_then(Value::as_array)
        .ok_or(R16ContractError::InvalidSnapshot)?;
    let mut documents = Vec::with_capacity(source.len());
    let mut statements = Vec::new();
    for document in source {
        let mut record = document
            .as_object()
            .cloned()
            .ok_or(R16ContractError::InvalidSnapshot)?;
        let document_id = record
            .get("document_id")
            .and_then(Value::as_str)
            .ok_or(R16ContractError::InvalidSnapshot)?
            .to_owned();
        let document_statements = record
            .remove("statements")
            .and_then(|value| value.as_array().cloned())
            .ok_or(R16ContractError::InvalidSnapshot)?;
        documents.push(Value::Object(record));
        for statement in document_statements {
            let mut statement = statement
                .as_object()
                .cloned()
                .ok_or(R16ContractError::InvalidSnapshot)?;
            statement.insert("document_id".to_owned(), Value::String(document_id.clone()));
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

fn family_digests(value: &Value, sha256: R16Sha256) -> Result<Value, R16ContractError> {
    let mut digests = Map::new();
    for family in PORTABLE_FAMILIES {
        let bytes = serde_json::to_vec(
            value
                .get(family)
                .ok_or(R16ContractError::InvalidProjection)?,
        )
        .map_err(|_| R16ContractError::Internal)?;
        digests.insert(family.to_owned(), Value::String(sha256(&bytes)));
    }
    Ok(Value::Object(digests))
}

fn append_extractor_version(
    object: &mut Map<String, Value>,
    version: &str,
) -> Result<(), RepositorySnapshotV18Error> {
    let versions = object
        .get_mut("extractor_versions")
        .and_then(Value::as_array_mut)
        .ok_or(RepositorySnapshotV18Error::ContractInvalid)?;
    if versions.iter().any(|value| value.as_str() == Some(version)) {
        return Err(RepositorySnapshotV18Error::ContractInvalid);
    }
    versions.push(Value::String(version.to_owned()));
    Ok(())
}

fn merge_id_array(
    object: &mut Map<String, Value>,
    field: &'static str,
    additions: impl IntoIterator<Item = Value>,
) -> Result<(), RepositorySnapshotV18Error> {
    let values = object
        .get_mut(field)
        .and_then(Value::as_array_mut)
        .ok_or(RepositorySnapshotV18Error::ContractInvalid)?;
    values.extend(additions);
    values.sort_by(|left, right| record_id(left).cmp(&record_id(right)));
    for pair in values.windows(2) {
        if record_id(&pair[0]) == record_id(&pair[1]) {
            return Err(RepositorySnapshotV18Error::ContractInvalid);
        }
    }
    Ok(())
}

fn remove_ids(
    object: &mut Map<String, Value>,
    field: &'static str,
    identifiers: &[String],
) -> Result<(), RepositorySnapshotV18Error> {
    let values = object
        .get_mut(field)
        .and_then(Value::as_array_mut)
        .ok_or(RepositorySnapshotV18Error::ContractInvalid)?;
    let before = values.len();
    values.retain(|value| {
        record_id(value).is_none_or(|id| !identifiers.iter().any(|value| value == id))
    });
    if before.saturating_sub(values.len()) != identifiers.len() {
        return Err(RepositorySnapshotV18Error::ContractInvalid);
    }
    Ok(())
}

fn insert_semantic_hash(
    value: &mut Value,
    domain: &[u8],
) -> Result<(), RepositorySnapshotV18Error> {
    value
        .as_object_mut()
        .ok_or(RepositorySnapshotV18Error::ContractInvalid)?
        .remove("semantic_hash");
    let hash = semantic_hash(domain, value);
    value
        .as_object_mut()
        .ok_or(RepositorySnapshotV18Error::ContractInvalid)?
        .insert(
            "semantic_hash".to_owned(),
            json!({"algorithm": "blake3-256", "value": hash}),
        );
    Ok(())
}

fn validate_family(
    object: &Map<String, Value>,
    family: &'static str,
    id_field: &'static str,
) -> Result<BTreeSet<String>, R16ContractError> {
    let values = object
        .get(family)
        .and_then(Value::as_array)
        .ok_or(R16ContractError::InvalidProjection)?;
    let mut identifiers = BTreeSet::new();
    let mut previous = None;
    for value in values {
        let identifier = value
            .get(id_field)
            .and_then(Value::as_str)
            .filter(|identifier| !identifier.is_empty())
            .ok_or(R16ContractError::InvalidProjection)?;
        if previous.is_some_and(|previous| previous >= identifier)
            || !identifiers.insert(identifier.to_owned())
        {
            return Err(R16ContractError::IdentityConflict(identifier.to_owned()));
        }
        previous = Some(identifier);
    }
    Ok(identifiers)
}

fn validate_reference(
    record: &Value,
    field: &'static str,
    identifiers: &BTreeSet<String>,
) -> Result<(), R16ContractError> {
    let identifier = record
        .get(field)
        .and_then(Value::as_str)
        .ok_or(R16ContractError::InvalidProjection)?;
    identifiers
        .contains(identifier)
        .then_some(())
        .ok_or_else(|| R16ContractError::ReferenceMismatch(identifier.to_owned()))
}

fn validate_reference_array(
    record: &Value,
    field: &'static str,
    identifiers: &BTreeSet<String>,
) -> Result<(), R16ContractError> {
    let Some(values) = record.get(field) else {
        return Ok(());
    };
    for identifier in values
        .as_array()
        .ok_or(R16ContractError::InvalidProjection)?
    {
        let identifier = identifier
            .as_str()
            .ok_or(R16ContractError::InvalidProjection)?;
        if !identifiers.contains(identifier) {
            return Err(R16ContractError::ReferenceMismatch(identifier.to_owned()));
        }
    }
    Ok(())
}

fn string_array(
    object: &Map<String, Value>,
    field: &'static str,
) -> Result<Vec<String>, R16ContractError> {
    object
        .get(field)
        .and_then(Value::as_array)
        .ok_or(R16ContractError::InvalidProjection)?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or(R16ContractError::InvalidProjection)
        })
        .collect()
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

fn sort_records(values: &mut [Value]) -> Result<(), QueryContractError> {
    values.sort_by(|left, right| record_id(left).cmp(&record_id(right)));
    for pair in values.windows(2) {
        if record_id(&pair[0]).is_none() || record_id(&pair[0]) == record_id(&pair[1]) {
            return Err(QueryContractError::InvalidSnapshot);
        }
    }
    Ok(())
}

fn map_r15_snapshot_error(error: RepositorySnapshotV17Error) -> RepositorySnapshotV18Error {
    match error {
        RepositorySnapshotV17Error::Serialization(error) => {
            RepositorySnapshotV18Error::Serialization(error)
        }
        RepositorySnapshotV17Error::LimitExceeded(error) => {
            RepositorySnapshotV18Error::LimitExceeded(error)
        }
        RepositorySnapshotV17Error::ContractInvalid => RepositorySnapshotV18Error::ContractInvalid,
        RepositorySnapshotV17Error::OutputLengthOverflow => {
            RepositorySnapshotV18Error::OutputLengthOverflow
        }
    }
}

fn validate_private_fields(value: &Value) -> Result<(), R16ContractError> {
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
                    return Err(R16ContractError::UnsafePayload("private_field"));
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
            return Err(R16ContractError::UnsafePayload("raw_url"));
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    Ok(())
}

fn ensure_nesting(value: &Value, depth: u64) -> Result<(), R16ContractError> {
    if depth > MAX_R16_JSON_NESTING {
        return Err(R16ContractError::LimitExceeded {
            limit: "json_nesting",
            maximum: MAX_R16_JSON_NESTING,
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

fn enforce_portable_size(length: usize) -> Result<(), R16ContractError> {
    let observed = u64::try_from(length).unwrap_or(u64::MAX);
    if observed > MAX_R16_PORTABLE_GRAPH_BYTES {
        return Err(R16ContractError::LimitExceeded {
            limit: "portable_graph_bytes",
            maximum: MAX_R16_PORTABLE_GRAPH_BYTES,
            observed,
        });
    }
    Ok(())
}

fn balanced_script_elements(bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let text = text.to_ascii_lowercase();
    let openings = text.match_indices("<script").count();
    openings > 0 && openings == text.match_indices("</script>").count()
}

fn safe_viewer_bytes(bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let text = text.to_ascii_lowercase();
    ![
        "http://",
        "https://",
        "<script src",
        "eval(",
        "new function",
        "import(",
        "javascript:",
    ]
    .iter()
    .any(|forbidden| text.contains(forbidden))
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

fn strictly_ordered(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
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
