use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use codenoesis_domain::s1_boundaries::RepositoryBoundaryReport;
use codenoesis_domain::s4_r3::R3_WORKSPACE_PROFILE;
use codenoesis_domain::s4_r4::R4_MANIFEST_PROFILE;
use codenoesis_domain::s4_r5::{R5_RUST_SEMANTIC_EXTRACTOR_VERSION, R5_RUST_SEMANTIC_PROFILE};
use codenoesis_domain::s4_r6::{
    FRAMEWORK_DECLARATION_ID_DOMAIN, FrameworkCoverageGap, FrameworkDeclaration,
    FrameworkDiagnostic, FrameworkError, FrameworkKnowledge, R6_CONFIGURATION_VERSION,
    R6_ERROR_VERSION, R6_EXTRACTION_CHUNK_VERSION, R6_EXTRACTION_CONTRACT_VERSION,
    R6_FRAMEWORK_EXTRACTOR_VERSION, R6_FRAMEWORK_INDEX_VERSION, R6_FRAMEWORK_PROFILE,
    R6_GRAPH_VERSION, R6_ONTOLOGY_VERSION, R6_PIPELINE_VERSION, R6_QUERY_VERSION,
    R6_SNAPSHOT_VERSION,
};
use codenoesis_domain::storage::{
    LocalSnapshotHead, PublicationCandidate, SNAPSHOT_SCHEMA_VERSION_V9, StorageComponent,
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
use super::s4_r5::{RepositorySnapshotV8, RepositorySnapshotV8Error, local_query_result_v3};
use super::{
    LimitedVecWriter, PublicationCandidateError, SnapshotEnvelopeV1, publication_candidate,
    semantic_hash,
};

const CONFIGURATION_V6_HASH_DOMAIN: &[u8] = b"codenoesis.configuration.semantic.v6";
const SNAPSHOT_V9_HASH_DOMAIN: &[u8] = b"codenoesis.repository-snapshot.semantic.v9";
const EXTRACTION_V6_HASH_DOMAIN: &[u8] = b"codenoesis.extraction-chunk.semantic.v6";
const GRAPH_V6_HASH_DOMAIN: &[u8] = b"codenoesis.knowledge-graph.semantic.v6";
const REQUIRED_FRAMEWORK_PROFILES: [&str; 4] = [
    "standard-local-s4",
    R3_WORKSPACE_PROFILE,
    R4_MANIFEST_PROFILE,
    R5_RUST_SEMANTIC_PROFILE,
];

#[derive(Clone, Debug)]
pub struct CodeNoesisErrorV13 {
    value: Value,
}

impl CodeNoesisErrorV13 {
    #[must_use]
    pub fn invalid_rust_framework_profile(profile: &str) -> Self {
        Self::new(
            "input.invalid_rust_framework_profile",
            "input",
            "invalid rust framework profile",
            &json!({"profile": bounded_nonempty(profile, 256, "missing")}),
        )
    }

    #[must_use]
    pub fn unsupported_composition(selected_profiles: &[String]) -> Self {
        let mut selected = selected_profiles
            .iter()
            .map(|value| bounded(value, 256))
            .filter(|value| !value.is_empty())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .take(8)
            .collect::<Vec<_>>();
        selected.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
        Self::new(
            "extraction.unsupported_framework_composition",
            "extraction",
            "unsupported framework composition",
            &json!({
                "required_profiles": REQUIRED_FRAMEWORK_PROFILES,
                "selected_profiles": selected
            }),
        )
    }

    #[must_use]
    pub fn from_framework(error: &FrameworkError) -> Option<Self> {
        match error {
            FrameworkError::InvalidDeclaration { path, reason } => Some(Self::new(
                "extraction.invalid_framework_declaration",
                "extraction",
                "invalid framework declaration",
                &json!({
                    "path": bounded_framework_path(path),
                    "reason": bounded_nonempty(reason, 256, "invalid_declaration")
                }),
            )),
            FrameworkError::IdentityConflict {
                normalized_preimage_sha256,
            } => Some(if valid_lower_hex(normalized_preimage_sha256, 64) {
                Self::new(
                    "extraction.framework_declaration_identity_conflict",
                    "extraction",
                    "framework declaration identity conflict",
                    &json!({
                        "domain": FRAMEWORK_DECLARATION_ID_DOMAIN,
                        "normalized_preimage_sha256": normalized_preimage_sha256
                    }),
                )
            } else {
                Self::internal()
            }),
            FrameworkError::LimitExceeded {
                limit,
                maximum,
                observed,
            } => Some(Self::new(
                "extraction.framework_declaration_limit_exceeded",
                "extraction",
                "framework declaration limit exceeded",
                &json!({
                    "limit": limit.as_str(),
                    "maximum": (*maximum).max(1),
                    "observed": (*observed).max(1)
                }),
            )),
            FrameworkError::UnsupportedComposition {
                selected_profiles, ..
            } => Some(Self::unsupported_composition(selected_profiles)),
            FrameworkError::AmbiguousTarget {
                target_spelling,
                candidate_count,
            } => Some(Self::new(
                "extraction.ambiguous_framework_target",
                "extraction",
                "ambiguous framework target",
                &json!({
                    "target_spelling": bounded_nonempty(
                        target_spelling,
                        1024,
                        "unresolved-target"
                    ),
                    "candidate_count": (*candidate_count).clamp(2, 4096)
                }),
            )),
            FrameworkError::UnresolvableEvidence { evidence_id } => {
                Some(if valid_evidence_id(evidence_id) {
                    Self::new(
                        "extraction.unresolvable_framework_evidence",
                        "extraction",
                        "unresolvable framework evidence",
                        &json!({"evidence_id": evidence_id}),
                    )
                } else {
                    Self::internal()
                })
            }
            FrameworkError::UnsafePath { path, reason } => Some(Self::new(
                "input.unsafe_framework_path",
                "input",
                "unsafe framework path",
                &json!({
                    "path": bounded_framework_path(path),
                    "reason": bounded_nonempty(reason, 128, "unsafe_path")
                }),
            )),
            FrameworkError::ContractInvalid => Some(Self::internal()),
            FrameworkError::Source(_) => None,
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
                "schema_version": R6_ERROR_VERSION,
                "code": code,
                "stage": stage,
                "message": message,
                "retryable": false,
                "context": context
            }),
        }
    }

    /// Serializes one strict `ErrorV13` followed by one LF.
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
pub struct LocalQueryResultV4 {
    value: Value,
}

impl LocalQueryResultV4 {
    /// Serializes one bounded exact-ID V4 result followed by one LF.
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

    pub(crate) fn into_value(self) -> Value {
        self.value
    }
}

/// Builds one strict V9 exact-ID query result without adding read authority.
///
/// # Errors
///
/// Returns a strict snapshot, document, not-found, or output-limit failure.
pub fn local_query_result_v4(
    semantic: &Value,
    manifest: &Value,
    snapshot_id: &str,
    requested_id: &str,
) -> Result<LocalQueryResultV4, QueryContractError> {
    let mut value: Value =
        local_query_result_v3(semantic, manifest, snapshot_id, requested_id)?.into_value();
    value["schema_version"] = Value::String(R6_QUERY_VERSION.to_owned());
    let result = LocalQueryResultV4 { value };
    result.canonical_stdout()?;
    Ok(result)
}

#[derive(Clone, Debug)]
pub struct RepositorySnapshotV9 {
    value: Value,
}

#[derive(Debug)]
pub enum RepositorySnapshotV9Error {
    Serialization(serde_json::Error),
    LimitExceeded(AcquisitionError),
    ContractInvalid,
    OutputLengthOverflow,
}

impl Display for RepositorySnapshotV9Error {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Serialization(_) => "R6 snapshot serialization failed",
            Self::LimitExceeded(_) => "R6 snapshot output limit exceeded",
            Self::ContractInvalid => "R6 snapshot contract is invalid",
            Self::OutputLengthOverflow => "R6 snapshot output length overflowed",
        })
    }
}

impl Error for RepositorySnapshotV9Error {}

impl RepositorySnapshotV9 {
    /// Builds selector-bound V9 over the complete immutable V8 lineage.
    ///
    /// # Errors
    ///
    /// Returns a framework, serialization, publication, or output-bound failure.
    pub fn from_inventory_and_framework_declarations(
        inventory: &RepositoryInventory,
        knowledge: &FrameworkKnowledge,
        boundaries: Option<&RepositoryBoundaryReport>,
        envelope: SnapshotEnvelopeV1,
    ) -> Result<Self, RepositorySnapshotV9Error> {
        knowledge
            .validate()
            .map_err(|_| RepositorySnapshotV9Error::ContractInvalid)?;
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
            .ok_or(RepositorySnapshotV9Error::ContractInvalid)?;
        let boundary_profile = boundaries.map(|_| "local-gitlinks-v1");
        let configuration_without_hash = json!({
            "schema_version": R6_CONFIGURATION_VERSION,
            "profile": "standard-local-s4",
            "workspace_profile": R3_WORKSPACE_PROFILE,
            "manifest_profile": R4_MANIFEST_PROFILE,
            "rust_semantic_profile": R5_RUST_SEMANTIC_PROFILE,
            "rust_framework_profile": R6_FRAMEWORK_PROFILE,
            "repository_boundary_profile": boundary_profile
        });
        let configuration_hash =
            semantic_hash(CONFIGURATION_V6_HASH_DOMAIN, &configuration_without_hash);
        let mut configuration = configuration_without_hash;
        configuration
            .as_object_mut()
            .ok_or(RepositorySnapshotV9Error::ContractInvalid)?
            .insert(
                "semantic_hash".to_owned(),
                json!({"algorithm": "blake3-256", "value": configuration_hash}),
            );
        semantic.insert("configuration".to_owned(), configuration);
        semantic.insert(
            "pipeline_version".to_owned(),
            Value::String(R6_PIPELINE_VERSION.to_owned()),
        );
        semantic.insert(
            "ontology_version".to_owned(),
            Value::String(R6_ONTOLOGY_VERSION.to_owned()),
        );
        semantic.insert(
            "extractor_contract_version".to_owned(),
            Value::String(R6_EXTRACTION_CONTRACT_VERSION.to_owned()),
        );
        let mut extractor_versions = vec![
            Value::String("codenoesis.inventory-classifier/s1-v1".to_owned()),
            Value::String("codenoesis.rust-tree-sitter/s4-v1".to_owned()),
            Value::String("codenoesis.rust-workspace/s4-r3-v1".to_owned()),
            Value::String("codenoesis.cargo-manifest/s4-r4-v1".to_owned()),
            Value::String(R5_RUST_SEMANTIC_EXTRACTOR_VERSION.to_owned()),
            Value::String(R6_FRAMEWORK_EXTRACTOR_VERSION.to_owned()),
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
            .ok_or(RepositorySnapshotV9Error::ContractInvalid)?;
        semantic.insert(
            "extraction_chunks".to_owned(),
            Value::Array(extraction_chunks_v6(&baseline_chunks, knowledge)?),
        );
        let baseline_graph = semantic
            .get("knowledge_graph")
            .cloned()
            .ok_or(RepositorySnapshotV9Error::ContractInvalid)?;
        semantic.insert(
            "knowledge_graph".to_owned(),
            knowledge_graph_v6(&baseline_graph, knowledge)?,
        );
        let semantic_value = Value::Object(semantic.clone());
        let snapshot_hash = semantic_hash(SNAPSHOT_V9_HASH_DOMAIN, &semantic_value);
        let root = value
            .as_object_mut()
            .ok_or(RepositorySnapshotV9Error::ContractInvalid)?;
        root.insert(
            "schema_version".to_owned(),
            Value::String(R6_SNAPSHOT_VERSION.to_owned()),
        );
        root.insert(
            "semantic_hash".to_owned(),
            json!({"algorithm": "blake3-256", "value": snapshot_hash}),
        );
        publication_candidate(&value).map_err(|_| RepositorySnapshotV9Error::ContractInvalid)?;
        Ok(Self { value })
    }

    /// Serializes the complete V9 snapshot with the inherited output bound.
    ///
    /// # Errors
    ///
    /// Returns a typed serialization or output-limit failure.
    pub fn canonical_stdout(&self) -> Result<Vec<u8>, RepositorySnapshotV9Error> {
        let maximum = usize::try_from(STANDARD_LOCAL_S1_LIMITS.canonical_output_bytes)
            .map_err(|_| RepositorySnapshotV9Error::OutputLengthOverflow)?;
        let body_maximum = maximum
            .checked_sub(1)
            .ok_or(RepositorySnapshotV9Error::OutputLengthOverflow)?;
        let mut writer = LimitedVecWriter::new(body_maximum);
        let result = serde_json::to_writer(&mut writer, &self.value);
        if writer.overflowed() {
            return Err(RepositorySnapshotV9Error::LimitExceeded(limit_exceeded(
                LimitKind::CanonicalOutputBytes,
                STANDARD_LOCAL_S1_LIMITS
                    .canonical_output_bytes
                    .saturating_add(1),
            )));
        }
        result.map_err(RepositorySnapshotV9Error::Serialization)?;
        let mut bytes = writer.into_inner();
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Serializes the exact V9 semantic payload stored by S4.
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

    /// Converts V9 into the unchanged immutable publication model.
    ///
    /// # Errors
    ///
    /// Returns a strict contract or storage-integrity failure.
    pub fn publication_candidate(&self) -> Result<PublicationCandidate, PublicationCandidateError> {
        publication_candidate(&self.value)
    }
}

/// Validates one loaded V9 semantic payload against the complete visible head.
///
/// # Errors
///
/// Returns a typed metadata-integrity failure on any binding mismatch.
pub fn validate_stored_snapshot_semantic_v9(
    semantic: &Value,
    head: &LocalSnapshotHead,
) -> Result<(), StorageError> {
    if head.snapshot_schema_version != SNAPSHOT_SCHEMA_VERSION_V9 {
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

fn extraction_chunks_v6(
    baseline: &Value,
    knowledge: &FrameworkKnowledge,
) -> Result<Vec<Value>, RepositorySnapshotV9Error> {
    let mut framework_chunks = knowledge
        .extraction_chunks
        .iter()
        .map(|chunk| (chunk.source_file_id.as_str(), chunk))
        .collect::<BTreeMap<_, _>>();
    let chunks = baseline
        .as_array()
        .ok_or(RepositorySnapshotV9Error::ContractInvalid)?;
    let mut transformed = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        let mut value = chunk.clone();
        let object = value
            .as_object_mut()
            .ok_or(RepositorySnapshotV9Error::ContractInvalid)?;
        object.insert(
            "schema_version".to_owned(),
            Value::String(R6_EXTRACTION_CHUNK_VERSION.to_owned()),
        );
        object.insert(
            "ontology_version".to_owned(),
            Value::String(R6_ONTOLOGY_VERSION.to_owned()),
        );
        object.remove("semantic_hash");
        let subject = object
            .get("subject")
            .ok_or(RepositorySnapshotV9Error::ContractInvalid)?;
        if subject.get("kind").and_then(Value::as_str) == Some("rust_source") {
            let source_file_id = subject
                .get("source_file_id")
                .and_then(Value::as_str)
                .ok_or(RepositorySnapshotV9Error::ContractInvalid)?;
            let framework = framework_chunks
                .remove(source_file_id)
                .ok_or(RepositorySnapshotV9Error::ContractInvalid)?;
            merge_id_array(
                object,
                "entities",
                framework
                    .supplemental_entities
                    .iter()
                    .map(entity_value)
                    .chain(framework.declarations.iter().map(framework_entity_value)),
            )?;
            merge_id_array(
                object,
                "relationships",
                framework.relationships.iter().map(relationship_value),
            )?;
            merge_id_array(object, "claims", framework.claims.iter().map(claim_value))?;
            merge_id_array(
                object,
                "evidence",
                framework.evidence.iter().map(evidence_value),
            )?;
            merge_id_array(
                object,
                "diagnostics",
                framework.diagnostics.iter().map(framework_diagnostic_value),
            )?;
            merge_id_array(
                object,
                "coverage",
                framework.coverage.iter().map(framework_coverage_value),
            )?;
        }
        insert_semantic_hash(&mut value, EXTRACTION_V6_HASH_DOMAIN)?;
        transformed.push(value);
    }
    if !framework_chunks.is_empty() {
        return Err(RepositorySnapshotV9Error::ContractInvalid);
    }
    Ok(transformed)
}

fn knowledge_graph_v6(
    baseline: &Value,
    knowledge: &FrameworkKnowledge,
) -> Result<Value, RepositorySnapshotV9Error> {
    let mut value = baseline.clone();
    let object = value
        .as_object_mut()
        .ok_or(RepositorySnapshotV9Error::ContractInvalid)?;
    object.insert(
        "schema_version".to_owned(),
        Value::String(R6_GRAPH_VERSION.to_owned()),
    );
    object.insert(
        "ontology_version".to_owned(),
        Value::String(R6_ONTOLOGY_VERSION.to_owned()),
    );
    object.insert(
        "extractor_versions".to_owned(),
        json!([
            "codenoesis.rust-tree-sitter/s4-v1",
            "codenoesis.rust-workspace/s4-r3-v1",
            "codenoesis.cargo-manifest/s4-r4-v1",
            R5_RUST_SEMANTIC_EXTRACTOR_VERSION,
            R6_FRAMEWORK_EXTRACTOR_VERSION
        ]),
    );
    object.insert(
        "framework_declaration_index".to_owned(),
        json!({
            "schema_version": R6_FRAMEWORK_INDEX_VERSION,
            "profile": R6_FRAMEWORK_PROFILE,
            "entity_ids": knowledge.graph.index.entity_ids,
            "declared_registration_ids": knowledge.graph.index.declared_registration_ids,
            "candidate_unresolved_ids": knowledge.graph.index.candidate_unresolved_ids
        }),
    );
    object.remove("semantic_hash");
    merge_id_array(
        object,
        "entities",
        knowledge
            .graph
            .supplemental_entities
            .iter()
            .map(entity_value)
            .chain(
                knowledge
                    .graph
                    .declarations
                    .iter()
                    .map(framework_entity_value),
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
            .map(framework_diagnostic_value),
    )?;
    merge_id_array(
        object,
        "coverage",
        knowledge
            .graph
            .coverage
            .iter()
            .map(framework_coverage_value),
    )?;
    insert_semantic_hash(&mut value, GRAPH_V6_HASH_DOMAIN)?;
    Ok(value)
}

fn framework_entity_value(declaration: &FrameworkDeclaration) -> Value {
    json!({
        "id": declaration.id,
        "kind": declaration.role.entity_kind(),
        "crate_id": declaration.crate_id,
        "lexical_owner_id": declaration.lexical_owner_id,
        "role": declaration.role.as_str(),
        "source_profile": declaration.source_profile.as_str(),
        "source_form_identity": declaration.source_form_identity,
        "declared_key_or_target": declaration.declared_key_or_target,
        "epistemic_state": declaration.epistemic_state.as_str(),
        "compilation_presence": declaration.compilation_presence.as_str(),
        "method": declaration.method,
        "path": declaration.path,
        "configuration_key": declaration.configuration_key,
        "target_spelling": declaration.target_spelling,
        "local_target_id": declaration.local_target_id,
        "target_binding": declaration.target_binding.as_str(),
        "evidence_ids": declaration.evidence_ids
    })
}

fn framework_diagnostic_value(diagnostic: &FrameworkDiagnostic) -> Value {
    json!({
        "id": diagnostic.id,
        "code": diagnostic.code,
        "message": diagnostic.message,
        "evidence_ids": diagnostic.evidence_ids
    })
}

fn framework_coverage_value(gap: &FrameworkCoverageGap) -> Value {
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
) -> Result<(), RepositorySnapshotV9Error> {
    let values = object
        .get_mut(field)
        .and_then(Value::as_array_mut)
        .ok_or(RepositorySnapshotV9Error::ContractInvalid)?;
    values.extend(additions);
    values.sort_by(|left, right| identifier(left).cmp(&identifier(right)));
    let mut seen = BTreeSet::new();
    let mut retained = Vec::with_capacity(values.len());
    for value in values.drain(..) {
        let id = identifier(&value)
            .ok_or(RepositorySnapshotV9Error::ContractInvalid)?
            .to_owned();
        if seen.insert(id) {
            retained.push(value);
        } else if retained.last() != Some(&value) {
            return Err(RepositorySnapshotV9Error::ContractInvalid);
        }
    }
    *values = retained;
    Ok(())
}

fn identifier(value: &Value) -> Option<&str> {
    value.get("id").and_then(Value::as_str)
}

fn insert_semantic_hash(value: &mut Value, domain: &[u8]) -> Result<(), RepositorySnapshotV9Error> {
    let hash = semantic_hash(domain, value);
    value
        .as_object_mut()
        .ok_or(RepositorySnapshotV9Error::ContractInvalid)?
        .insert(
            "semantic_hash".to_owned(),
            json!({"algorithm": "blake3-256", "value": hash}),
        );
    Ok(())
}

fn map_v8_error(error: RepositorySnapshotV8Error) -> RepositorySnapshotV9Error {
    match error {
        RepositorySnapshotV8Error::Serialization(error) => {
            RepositorySnapshotV9Error::Serialization(error)
        }
        RepositorySnapshotV8Error::LimitExceeded(error) => {
            RepositorySnapshotV9Error::LimitExceeded(error)
        }
        RepositorySnapshotV8Error::ContractInvalid => RepositorySnapshotV9Error::ContractInvalid,
        RepositorySnapshotV8Error::OutputLengthOverflow => {
            RepositorySnapshotV9Error::OutputLengthOverflow
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

fn bounded_framework_path(value: &str) -> String {
    let value = bounded_nonempty(value, 4096, "repository-path");
    let windows_prefix = value.as_bytes().get(1) == Some(&b':')
        && value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic);
    if value.starts_with('/')
        || windows_prefix
        || value.contains('\\')
        || value.chars().any(char::is_control)
        || value
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
    {
        "repository-path".to_owned()
    } else {
        value
    }
}

fn valid_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn valid_evidence_id(value: &str) -> bool {
    [
        "urn:codenoesis:evidence:blake3:",
        "urn:codenoesis:evidence:sha256:",
    ]
    .into_iter()
    .find_map(|prefix| value.strip_prefix(prefix))
    .is_some_and(|digest| valid_lower_hex(digest, 64))
}
