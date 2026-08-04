use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use codenoesis_domain::s1_boundaries::RepositoryBoundaryReport;
use codenoesis_domain::s4_r3::R3_WORKSPACE_PROFILE;
use codenoesis_domain::s4_r4::R4_MANIFEST_PROFILE;
use codenoesis_domain::s4_r5::{
    R5_EXTRACTION_CONTRACT_VERSION, R5_ONTOLOGY_VERSION, R5_PIPELINE_VERSION,
    R5_RUST_SEMANTIC_EXTRACTOR_VERSION, R5_RUST_SEMANTIC_INDEX_VERSION, R5_RUST_SEMANTIC_PROFILE,
    RustSemanticAttribute, RustSemanticCoverageGap, RustSemanticDiagnostic, RustSemanticEntity,
    RustSemanticError, RustSemanticKnowledge, RustSemanticProperties,
};
use codenoesis_domain::storage::{
    LocalSnapshotHead, PublicationCandidate, SNAPSHOT_SCHEMA_VERSION_V8, StorageComponent,
    StorageError,
};
use codenoesis_domain::{
    AcquisitionError, LimitKind, RepositoryInventory, STANDARD_LOCAL_S1_LIMITS, limit_exceeded,
};
use serde_json::{Map, Value, json};

use super::s4::{
    MAX_QUERY_BYTES, QueryContractError, claim_value, entity_value, evidence_value,
    relationship_value,
};
use super::s4_r4::{RepositorySnapshotV7, RepositorySnapshotV7Error, local_query_result_v2};
use super::{
    LimitedVecWriter, PublicationCandidateError, SnapshotEnvelopeV1, publication_candidate,
    semantic_hash,
};

const CONFIGURATION_V5_HASH_DOMAIN: &[u8] = b"codenoesis.configuration.semantic.v5";
const SNAPSHOT_V8_HASH_DOMAIN: &[u8] = b"codenoesis.repository-snapshot.semantic.v8";
const EXTRACTION_V5_HASH_DOMAIN: &[u8] = b"codenoesis.extraction-chunk.semantic.v5";
const GRAPH_V5_HASH_DOMAIN: &[u8] = b"codenoesis.knowledge-graph.semantic.v5";
const REQUIRED_R5_COMPOSITION: &str =
    "standard-local-s4+cargo-root-package-v1+cargo-manifest-facts-v1";

#[derive(Clone, Debug)]
pub struct CodeNoesisErrorV12 {
    value: Value,
}

impl CodeNoesisErrorV12 {
    #[must_use]
    pub fn invalid_rust_semantic_profile(provided_profile: &str) -> Self {
        let provided_profile = provided_profile.chars().take(256).collect::<String>();
        Self::new(
            "input.invalid_rust_semantic_profile",
            "input",
            "invalid rust semantic profile",
            &json!({"provided_profile": provided_profile}),
        )
    }

    #[must_use]
    pub fn unsupported_composition(reason: &str) -> Self {
        Self::new(
            "extraction.unsupported_rust_semantic_composition",
            "extraction",
            "unsupported rust semantic composition",
            &json!({
                "profile": R5_RUST_SEMANTIC_PROFILE,
                "required_profile": REQUIRED_R5_COMPOSITION,
                "reason": reason
            }),
        )
    }

    #[must_use]
    pub fn from_semantic(error: &RustSemanticError) -> Option<Self> {
        match error {
            RustSemanticError::InvalidDeclaration {
                path,
                start_byte,
                declaration_kind,
            } => Some(Self::new(
                "extraction.invalid_rust_semantic_declaration",
                "extraction",
                "invalid rust semantic declaration",
                &json!({
                    "path": path,
                    "start_byte": start_byte,
                    "declaration_kind": declaration_kind
                }),
            )),
            RustSemanticError::IdentityConflict {
                owner_id,
                member_kind,
                normalized_member,
            } => Some(Self::new(
                "extraction.rust_semantic_identity_conflict",
                "extraction",
                "rust semantic identity conflict",
                &json!({
                    "owner_id": owner_id,
                    "member_kind": member_kind,
                    "normalized_member": normalized_member
                }),
            )),
            RustSemanticError::LimitExceeded {
                limit,
                maximum,
                observed,
            } => Some(Self::new(
                "extraction.rust_semantic_limit_exceeded",
                "extraction",
                "rust semantic limit exceeded",
                &json!({
                    "limit": limit.as_str(),
                    "maximum": maximum,
                    "observed": observed
                }),
            )),
            RustSemanticError::UnsupportedComposition { reason } => {
                Some(Self::unsupported_composition(reason))
            }
            RustSemanticError::ContractInvalid => Some(Self::internal()),
            RustSemanticError::Source(_) => None,
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
                "schema_version": "codenoesis.error/v12",
                "code": code,
                "stage": stage,
                "message": message,
                "retryable": false,
                "context": context
            }),
        }
    }

    /// Serializes one strict `ErrorV12` followed by one LF.
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
pub struct LocalQueryResultV3 {
    value: Value,
}

impl LocalQueryResultV3 {
    /// Serializes one bounded exact-ID V3 result followed by one LF.
    ///
    /// # Errors
    ///
    /// Returns a serialization or fixed query-result limit failure.
    pub fn canonical_stdout(&self) -> Result<Vec<u8>, QueryContractError> {
        let mut bytes =
            serde_json::to_vec(&self.value).map_err(|_| QueryContractError::InvalidSnapshot)?;
        if bytes.len() >= MAX_QUERY_BYTES {
            return Err(QueryContractError::LimitExceeded);
        }
        bytes.push(b'\n');
        Ok(bytes)
    }
}

/// Builds one strict V8 exact-ID query result without adding read authority.
///
/// # Errors
///
/// Returns a strict snapshot, document, not-found, or output-limit failure.
pub fn local_query_result_v3(
    semantic: &Value,
    manifest: &Value,
    snapshot_id: &str,
    requested_id: &str,
) -> Result<LocalQueryResultV3, QueryContractError> {
    let mut value =
        local_query_result_v2(semantic, manifest, snapshot_id, requested_id)?.into_value();
    value["schema_version"] = Value::String("codenoesis.local-query-result/v3".to_owned());
    let result = LocalQueryResultV3 { value };
    result.canonical_stdout()?;
    Ok(result)
}

#[derive(Clone, Debug)]
pub struct RepositorySnapshotV8 {
    value: Value,
}

#[derive(Debug)]
pub enum RepositorySnapshotV8Error {
    Serialization(serde_json::Error),
    LimitExceeded(AcquisitionError),
    ContractInvalid,
    OutputLengthOverflow,
}

impl Display for RepositorySnapshotV8Error {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Serialization(_) => "R5 snapshot serialization failed",
            Self::LimitExceeded(_) => "R5 snapshot output limit exceeded",
            Self::ContractInvalid => "R5 snapshot contract is invalid",
            Self::OutputLengthOverflow => "R5 snapshot output length overflowed",
        })
    }
}

impl Error for RepositorySnapshotV8Error {}

impl RepositorySnapshotV8 {
    /// Builds the selector-bound V8 snapshot over the immutable R4 lineage.
    ///
    /// # Errors
    ///
    /// Returns a semantic, serialization, publication, or output-bound failure.
    pub fn from_inventory_and_rust_semantics(
        inventory: &RepositoryInventory,
        knowledge: &RustSemanticKnowledge,
        boundaries: Option<&RepositoryBoundaryReport>,
        envelope: SnapshotEnvelopeV1,
    ) -> Result<Self, RepositorySnapshotV8Error> {
        knowledge
            .validate()
            .map_err(|_| RepositorySnapshotV8Error::ContractInvalid)?;
        let baseline = RepositorySnapshotV7::from_inventory_and_manifest_facts(
            inventory,
            &knowledge.manifest,
            boundaries,
            envelope,
        )
        .map_err(map_v7_error)?;
        let mut value = baseline.value().clone();
        let semantic = value
            .get_mut("semantic")
            .and_then(Value::as_object_mut)
            .ok_or(RepositorySnapshotV8Error::ContractInvalid)?;
        let boundary_profile = boundaries.map(|_| "local-gitlinks-v1");
        let configuration_without_hash = json!({
            "schema_version": "codenoesis.configuration/v5",
            "profile": "standard-local-s4",
            "workspace_profile": R3_WORKSPACE_PROFILE,
            "manifest_profile": R4_MANIFEST_PROFILE,
            "rust_semantic_profile": R5_RUST_SEMANTIC_PROFILE,
            "repository_boundary_profile": boundary_profile
        });
        let configuration_hash =
            semantic_hash(CONFIGURATION_V5_HASH_DOMAIN, &configuration_without_hash);
        let mut configuration = configuration_without_hash;
        configuration
            .as_object_mut()
            .ok_or(RepositorySnapshotV8Error::ContractInvalid)?
            .insert(
                "semantic_hash".to_owned(),
                json!({"algorithm": "blake3-256", "value": configuration_hash}),
            );
        semantic.insert("configuration".to_owned(), configuration);
        semantic.insert(
            "pipeline_version".to_owned(),
            Value::String(R5_PIPELINE_VERSION.to_owned()),
        );
        semantic.insert(
            "ontology_version".to_owned(),
            Value::String(R5_ONTOLOGY_VERSION.to_owned()),
        );
        semantic.insert(
            "extractor_contract_version".to_owned(),
            Value::String(R5_EXTRACTION_CONTRACT_VERSION.to_owned()),
        );
        let mut extractor_versions = vec![
            Value::String("codenoesis.inventory-classifier/s1-v1".to_owned()),
            Value::String("codenoesis.rust-tree-sitter/s4-v1".to_owned()),
            Value::String("codenoesis.rust-workspace/s4-r3-v1".to_owned()),
            Value::String("codenoesis.cargo-manifest/s4-r4-v1".to_owned()),
            Value::String(R5_RUST_SEMANTIC_EXTRACTOR_VERSION.to_owned()),
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
            .ok_or(RepositorySnapshotV8Error::ContractInvalid)?;
        semantic.insert(
            "extraction_chunks".to_owned(),
            Value::Array(extraction_chunks_v5(&baseline_chunks, knowledge)?),
        );
        let baseline_graph = semantic
            .get("knowledge_graph")
            .cloned()
            .ok_or(RepositorySnapshotV8Error::ContractInvalid)?;
        semantic.insert(
            "knowledge_graph".to_owned(),
            knowledge_graph_v5(&baseline_graph, knowledge)?,
        );
        let semantic_value = Value::Object(semantic.clone());
        let snapshot_hash = semantic_hash(SNAPSHOT_V8_HASH_DOMAIN, &semantic_value);
        let root = value
            .as_object_mut()
            .ok_or(RepositorySnapshotV8Error::ContractInvalid)?;
        root.insert(
            "schema_version".to_owned(),
            Value::String(SNAPSHOT_SCHEMA_VERSION_V8.to_owned()),
        );
        root.insert(
            "semantic_hash".to_owned(),
            json!({"algorithm": "blake3-256", "value": snapshot_hash}),
        );
        publication_candidate(&value).map_err(|_| RepositorySnapshotV8Error::ContractInvalid)?;
        Ok(Self { value })
    }

    /// Serializes the complete V8 snapshot with the inherited output bound.
    ///
    /// # Errors
    ///
    /// Returns a typed serialization or output-limit failure.
    pub fn canonical_stdout(&self) -> Result<Vec<u8>, RepositorySnapshotV8Error> {
        let maximum = usize::try_from(STANDARD_LOCAL_S1_LIMITS.canonical_output_bytes)
            .map_err(|_| RepositorySnapshotV8Error::OutputLengthOverflow)?;
        let body_maximum = maximum
            .checked_sub(1)
            .ok_or(RepositorySnapshotV8Error::OutputLengthOverflow)?;
        let mut writer = LimitedVecWriter::new(body_maximum);
        let result = serde_json::to_writer(&mut writer, &self.value);
        if writer.overflowed() {
            return Err(RepositorySnapshotV8Error::LimitExceeded(limit_exceeded(
                LimitKind::CanonicalOutputBytes,
                STANDARD_LOCAL_S1_LIMITS
                    .canonical_output_bytes
                    .saturating_add(1),
            )));
        }
        result.map_err(RepositorySnapshotV8Error::Serialization)?;
        let mut bytes = writer.into_inner();
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Serializes the exact V8 semantic payload stored by S4.
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

    /// Converts V8 into the unchanged immutable publication model.
    ///
    /// # Errors
    ///
    /// Returns a strict contract or storage-integrity failure.
    pub fn publication_candidate(&self) -> Result<PublicationCandidate, PublicationCandidateError> {
        publication_candidate(&self.value)
    }
}

/// Validates one loaded V8 semantic payload against the complete visible head.
///
/// # Errors
///
/// Returns a typed metadata-integrity failure on any binding mismatch.
pub fn validate_stored_snapshot_semantic_v8(
    semantic: &Value,
    head: &LocalSnapshotHead,
) -> Result<(), StorageError> {
    if head.snapshot_schema_version != SNAPSHOT_SCHEMA_VERSION_V8 {
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

fn extraction_chunks_v5(
    baseline: &Value,
    knowledge: &RustSemanticKnowledge,
) -> Result<Vec<Value>, RepositorySnapshotV8Error> {
    let mut semantic_chunks = knowledge
        .extraction_chunks
        .iter()
        .map(|chunk| (chunk.source_file_id.as_str(), chunk))
        .collect::<BTreeMap<_, _>>();
    let chunks = baseline
        .as_array()
        .ok_or(RepositorySnapshotV8Error::ContractInvalid)?;
    let mut transformed = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        let mut value = chunk.clone();
        let object = value
            .as_object_mut()
            .ok_or(RepositorySnapshotV8Error::ContractInvalid)?;
        object.insert(
            "schema_version".to_owned(),
            Value::String("codenoesis.extraction-chunk/v5".to_owned()),
        );
        object.insert(
            "ontology_version".to_owned(),
            Value::String(R5_ONTOLOGY_VERSION.to_owned()),
        );
        object.remove("semantic_hash");
        let subject = object
            .get("subject")
            .ok_or(RepositorySnapshotV8Error::ContractInvalid)?;
        if subject.get("kind").and_then(Value::as_str) == Some("rust_source") {
            let source_file_id = subject
                .get("source_file_id")
                .and_then(Value::as_str)
                .ok_or(RepositorySnapshotV8Error::ContractInvalid)?;
            let semantic = semantic_chunks
                .remove(source_file_id)
                .ok_or(RepositorySnapshotV8Error::ContractInvalid)?;
            merge_id_array(
                object,
                "entities",
                semantic
                    .legacy_entities
                    .iter()
                    .map(entity_value)
                    .chain(semantic.entities.iter().map(rust_semantic_entity_value)),
            )?;
            merge_id_array(
                object,
                "relationships",
                semantic.relationships.iter().map(relationship_value),
            )?;
            merge_id_array(object, "claims", semantic.claims.iter().map(claim_value))?;
            merge_id_array(
                object,
                "evidence",
                semantic.evidence.iter().map(evidence_value),
            )?;
            merge_id_array(
                object,
                "diagnostics",
                semantic
                    .diagnostics
                    .iter()
                    .map(rust_semantic_diagnostic_value),
            )?;
            merge_id_array(
                object,
                "coverage",
                semantic.coverage.iter().map(rust_semantic_coverage_value),
            )?;
        }
        insert_semantic_hash(&mut value, EXTRACTION_V5_HASH_DOMAIN)?;
        transformed.push(value);
    }
    if !semantic_chunks.is_empty() {
        return Err(RepositorySnapshotV8Error::ContractInvalid);
    }
    Ok(transformed)
}

fn knowledge_graph_v5(
    baseline: &Value,
    knowledge: &RustSemanticKnowledge,
) -> Result<Value, RepositorySnapshotV8Error> {
    let mut value = baseline.clone();
    let object = value
        .as_object_mut()
        .ok_or(RepositorySnapshotV8Error::ContractInvalid)?;
    object.insert(
        "schema_version".to_owned(),
        Value::String("codenoesis.knowledge-graph/v5".to_owned()),
    );
    object.insert(
        "ontology_version".to_owned(),
        Value::String(R5_ONTOLOGY_VERSION.to_owned()),
    );
    object.insert(
        "extractor_versions".to_owned(),
        json!([
            "codenoesis.rust-tree-sitter/s4-v1",
            "codenoesis.rust-workspace/s4-r3-v1",
            "codenoesis.cargo-manifest/s4-r4-v1",
            R5_RUST_SEMANTIC_EXTRACTOR_VERSION
        ]),
    );
    object.insert(
        "rust_semantic_index".to_owned(),
        json!({
            "schema_version": R5_RUST_SEMANTIC_INDEX_VERSION,
            "profile": R5_RUST_SEMANTIC_PROFILE,
            "member_entity_ids": knowledge.graph.index.member_entity_ids,
            "implementation_context_method_ids": knowledge.graph.index.implementation_context_method_ids
        }),
    );
    object.remove("semantic_hash");
    merge_id_array(
        object,
        "entities",
        knowledge
            .graph
            .legacy_entities
            .iter()
            .map(entity_value)
            .chain(
                knowledge
                    .graph
                    .entities
                    .iter()
                    .map(rust_semantic_entity_value),
            ),
    )?;
    merge_id_array(
        object,
        "relationships",
        knowledge.graph.relationships.iter().map(relationship_value),
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
            .map(rust_semantic_diagnostic_value),
    )?;
    merge_id_array(
        object,
        "coverage",
        knowledge
            .graph
            .coverage
            .iter()
            .map(rust_semantic_coverage_value),
    )?;
    insert_semantic_hash(&mut value, GRAPH_V5_HASH_DOMAIN)?;
    Ok(value)
}

fn rust_semantic_entity_value(entity: &RustSemanticEntity) -> Value {
    match &entity.properties {
        RustSemanticProperties::Member(properties) => json!({
            "id": entity.id,
            "kind": entity.kind.as_str(),
            "crate_id": entity.crate_id,
            "module_path": entity.module_path,
            "name": entity.name,
            "visibility": entity.visibility.as_str(),
            "owner_id": entity.owner_id,
            "trait_context_id": entity.trait_context_id,
            "compilation_presence": entity.compilation_presence.as_str(),
            "properties": {
                "owner_kind": properties.owner_kind.as_str(),
                "form": properties.form.as_str(),
                "declared_name": properties.declared_name,
                "tuple_index": properties.tuple_index,
                "declared_type_or_header": properties.declared_type_or_header,
                "mutable": properties.mutable,
                "initializer_present": properties.initializer_present,
                "discriminant_present": properties.discriminant_present,
                "bounds_present": properties.bounds_present,
                "default_present": properties.default_present,
                "attributes": properties.attributes.iter().map(attribute_value).collect::<Vec<_>>()
            }
        }),
        RustSemanticProperties::Method(properties) => json!({
            "id": entity.id,
            "kind": entity.kind.as_str(),
            "crate_id": entity.crate_id,
            "module_path": entity.module_path,
            "name": entity.name,
            "visibility": entity.visibility.as_str(),
            "owner_id": entity.owner_id,
            "properties": {
                "implementation_context": properties.implementation_context.as_str(),
                "trait_context_id": properties.trait_context_id,
                "receiver_present": properties.receiver_present,
                "declared_signature": properties.declared_signature,
                "compilation_presence": properties.compilation_presence.as_str(),
                "attributes": properties.attributes.iter().map(attribute_value).collect::<Vec<_>>()
            }
        }),
    }
}

fn attribute_value(attribute: &RustSemanticAttribute) -> Value {
    json!({
        "kind": attribute.kind.as_str(),
        "token_text": attribute.token_text,
        "evidence_id": attribute.evidence_id
    })
}

fn rust_semantic_diagnostic_value(diagnostic: &RustSemanticDiagnostic) -> Value {
    json!({
        "id": diagnostic.id,
        "code": diagnostic.code,
        "message": diagnostic.message,
        "evidence_ids": diagnostic.evidence_ids
    })
}

fn rust_semantic_coverage_value(gap: &RustSemanticCoverageGap) -> Value {
    json!({
        "id": gap.id,
        "capability": gap.capability,
        "state": gap.state.as_str(),
        "evidence_ids": gap.evidence_ids
    })
}

fn merge_id_array(
    object: &mut Map<String, Value>,
    field: &'static str,
    additions: impl IntoIterator<Item = Value>,
) -> Result<(), RepositorySnapshotV8Error> {
    let values = object
        .get_mut(field)
        .and_then(Value::as_array_mut)
        .ok_or(RepositorySnapshotV8Error::ContractInvalid)?;
    values.extend(additions);
    values.sort_by(|left, right| identifier(left).cmp(&identifier(right)));
    let mut seen = BTreeSet::new();
    let mut retained = Vec::with_capacity(values.len());
    for value in values.drain(..) {
        let id = identifier(&value)
            .ok_or(RepositorySnapshotV8Error::ContractInvalid)?
            .to_owned();
        if seen.insert(id) {
            retained.push(value);
        } else if retained.last() != Some(&value) {
            return Err(RepositorySnapshotV8Error::ContractInvalid);
        }
    }
    *values = retained;
    Ok(())
}

fn identifier(value: &Value) -> Option<&str> {
    value.get("id").and_then(Value::as_str)
}

fn insert_semantic_hash(value: &mut Value, domain: &[u8]) -> Result<(), RepositorySnapshotV8Error> {
    let hash = semantic_hash(domain, value);
    value
        .as_object_mut()
        .ok_or(RepositorySnapshotV8Error::ContractInvalid)?
        .insert(
            "semantic_hash".to_owned(),
            json!({"algorithm": "blake3-256", "value": hash}),
        );
    Ok(())
}

fn map_v7_error(error: RepositorySnapshotV7Error) -> RepositorySnapshotV8Error {
    match error {
        RepositorySnapshotV7Error::Serialization(error) => {
            RepositorySnapshotV8Error::Serialization(error)
        }
        RepositorySnapshotV7Error::LimitExceeded(error) => {
            RepositorySnapshotV8Error::LimitExceeded(error)
        }
        RepositorySnapshotV7Error::ContractInvalid => RepositorySnapshotV8Error::ContractInvalid,
        RepositorySnapshotV7Error::OutputLengthOverflow => {
            RepositorySnapshotV8Error::OutputLengthOverflow
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
