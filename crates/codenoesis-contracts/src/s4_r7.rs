use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use codenoesis_domain::s1_boundaries::RepositoryBoundaryReport;
use codenoesis_domain::s4::WorkspaceClaim;
use codenoesis_domain::s4_r3::R3_WORKSPACE_PROFILE;
use codenoesis_domain::s4_r4::R4_MANIFEST_PROFILE;
use codenoesis_domain::s4_r5::{R5_RUST_SEMANTIC_EXTRACTOR_VERSION, R5_RUST_SEMANTIC_PROFILE};
use codenoesis_domain::s4_r6::{
    FrameworkKnowledge, R6_FRAMEWORK_EXTRACTOR_VERSION, R6_FRAMEWORK_PROFILE,
};
use codenoesis_domain::s4_r7::{
    COMPILER_SYMBOL_ID_DOMAIN, CompilerBindingState, CompilerCoverageGap, CompilerDiagnostic,
    CompilerEvidence, CompilerIndexError, CompilerIndexOverlay, CompilerRelationship,
    CompilerSourceEvidence, CompilerSymbol, R7_COMPILER_EXTRACTOR_VERSION,
    R7_COMPILER_INDEX_PROFILE, R7_COMPILER_INDEX_VERSION, R7_CONFIGURATION_VERSION,
    R7_ERROR_VERSION, R7_EXTRACTION_CHUNK_VERSION, R7_EXTRACTION_CONTRACT_VERSION,
    R7_GRAPH_VERSION, R7_ONTOLOGY_VERSION, R7_PIPELINE_VERSION, R7_QUERY_VERSION,
    R7_SNAPSHOT_VERSION,
};
use codenoesis_domain::storage::{
    LocalSnapshotHead, PublicationCandidate, SNAPSHOT_SCHEMA_VERSION_V10, StorageComponent,
    StorageError,
};
use codenoesis_domain::{
    AcquisitionError, LimitKind, RepositoryInventory, STANDARD_LOCAL_S1_LIMITS, limit_exceeded,
};
use serde_json::{Map, Value, json};

use super::s4::{GraphIndex, linked_statements, string_array, validate_manifest_binding};
use super::s4::{MAX_QUERY_BYTES, QueryContractError, claim_value, entity_value};
use super::s4_r6::{RepositorySnapshotV9, RepositorySnapshotV9Error, local_query_result_v4};
use super::{
    LimitedVecWriter, PublicationCandidateError, SnapshotEnvelopeV1, publication_candidate,
    semantic_hash,
};

const CONFIGURATION_V7_HASH_DOMAIN: &[u8] = b"codenoesis.configuration.semantic.v7";
const SNAPSHOT_V10_HASH_DOMAIN: &[u8] = b"codenoesis.repository-snapshot.semantic.v10";
const EXTRACTION_V7_HASH_DOMAIN: &[u8] = b"codenoesis.extraction-chunk.semantic.v7";
const GRAPH_V7_HASH_DOMAIN: &[u8] = b"codenoesis.knowledge-graph.semantic.v7";
const REQUIRED_COMPILER_PROFILES: [&str; 6] = [
    "standard-local-s4",
    R3_WORKSPACE_PROFILE,
    R4_MANIFEST_PROFILE,
    R5_RUST_SEMANTIC_PROFILE,
    R6_FRAMEWORK_PROFILE,
    R7_COMPILER_INDEX_PROFILE,
];

#[derive(Clone, Debug)]
pub struct CodeNoesisErrorV14 {
    value: Value,
}

impl CodeNoesisErrorV14 {
    #[must_use]
    pub fn invalid_compiler_index_profile(profile: &str) -> Self {
        Self::new(
            "input.invalid_compiler_index_profile",
            "input",
            "invalid compiler index profile",
            &json!({"profile": bounded_nonempty(profile, 256, "missing")}),
        )
    }

    #[must_use]
    pub fn unsupported_composition(selected_profiles: &[String]) -> Self {
        let selected_profiles = selected_profiles
            .iter()
            .map(|value| bounded(value, 256))
            .filter(|value| !value.is_empty())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .take(8)
            .collect::<Vec<_>>();
        Self::new(
            "extraction.unsupported_compiler_index_composition",
            "extraction",
            "unsupported compiler index composition",
            &json!({
                "required_profiles": REQUIRED_COMPILER_PROFILES,
                "selected_profiles": selected_profiles
            }),
        )
    }

    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn from_compiler_index(error: &CompilerIndexError) -> Self {
        match error {
            CompilerIndexError::UnsafePath { path, reason } => Self::new(
                "input.unsafe_compiler_index_path",
                "input",
                "unsafe compiler index path",
                &json!({
                    "path": bounded_safe_path(path),
                    "reason": bounded_nonempty(reason, 128, "unsafe_path")
                }),
            ),
            CompilerIndexError::InvalidBinding { path, reason } => Self::new(
                "extraction.invalid_compiler_index_binding",
                "extraction",
                "invalid compiler index binding",
                &json!({
                    "path": bounded_safe_path(path),
                    "reason": bounded_nonempty(reason, 256, "invalid_binding")
                }),
            ),
            CompilerIndexError::UnsupportedSchema {
                commit,
                scip_proto_sha256,
            } => Self::new(
                "extraction.unsupported_compiler_index_schema",
                "extraction",
                "unsupported compiler index schema",
                &json!({
                    "tag": "v0.9.0",
                    "commit": bounded_nonempty(commit, 128, "unknown"),
                    "scip_proto_sha256": valid_sha256_or_zero(scip_proto_sha256)
                }),
            ),
            CompilerIndexError::UnsupportedProducer {
                name,
                version_sha256,
                commit_sha256,
            } => Self::new(
                "extraction.unsupported_compiler_index_producer",
                "extraction",
                "unsupported compiler index producer",
                &json!({
                    "name": bounded_nonempty(name, 128, "unknown"),
                    "version_sha256": valid_sha256_or_zero(version_sha256),
                    "commit_sha256": valid_sha256_or_zero(commit_sha256)
                }),
            ),
            CompilerIndexError::BindingMismatch {
                subject,
                expected_sha256,
                observed_sha256,
            } => Self::new(
                "extraction.compiler_index_binding_mismatch",
                "extraction",
                "compiler index binding mismatch",
                &json!({
                    "subject": subject.as_str(),
                    "expected_sha256": valid_sha256_or_zero(expected_sha256),
                    "observed_sha256": valid_sha256_or_zero(observed_sha256)
                }),
            ),
            CompilerIndexError::MalformedArtifact {
                artifact_sha256,
                reason,
            } => Self::artifact_error(
                "extraction.malformed_compiler_index",
                "malformed compiler index",
                artifact_sha256,
                reason,
            ),
            CompilerIndexError::NoncanonicalArtifact {
                artifact_sha256,
                reason,
            } => Self::artifact_error(
                "extraction.noncanonical_compiler_index",
                "noncanonical compiler index",
                artifact_sha256,
                reason,
            ),
            CompilerIndexError::IdentityConflict {
                normalized_preimage_sha256,
            } => Self::new(
                "extraction.compiler_index_identity_conflict",
                "extraction",
                "compiler index identity conflict",
                &json!({
                    "domain": COMPILER_SYMBOL_ID_DOMAIN,
                    "normalized_preimage_sha256": valid_sha256_or_zero(
                        normalized_preimage_sha256
                    )
                }),
            ),
            CompilerIndexError::AmbiguousEndpoint {
                symbol_sha256,
                candidate_count,
            } => Self::new(
                "extraction.ambiguous_compiler_index_endpoint",
                "extraction",
                "ambiguous compiler index endpoint",
                &json!({
                    "symbol_sha256": valid_sha256_or_zero(symbol_sha256),
                    "candidate_count": (*candidate_count).clamp(2, 4096)
                }),
            ),
            CompilerIndexError::RelationConflict {
                kind,
                source_id,
                target_id,
                reason,
            } => Self::new(
                "extraction.compiler_index_relation_conflict",
                "extraction",
                "compiler index relation conflict",
                &json!({
                    "kind": kind.as_str(),
                    "source_id": bounded_nonempty(source_id, 256, "unknown"),
                    "target_id": bounded_nonempty(target_id, 256, "unknown"),
                    "reason": bounded_nonempty(reason, 256, "relation_conflict")
                }),
            ),
            CompilerIndexError::LimitExceeded {
                limit,
                maximum,
                observed,
            } => Self::new(
                "extraction.compiler_index_limit_exceeded",
                "extraction",
                "compiler index limit exceeded",
                &json!({
                    "limit": limit.as_str(),
                    "maximum": (*maximum).max(1),
                    "observed": (*observed).max(1)
                }),
            ),
            CompilerIndexError::UnresolvableEvidence { evidence_id } => Self::new(
                "extraction.unresolvable_compiler_index_evidence",
                "extraction",
                "unresolvable compiler index evidence",
                &json!({"evidence_id": valid_evidence_id_or_zero(evidence_id)}),
            ),
            CompilerIndexError::ContractInvalid => Self::internal(),
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

    fn artifact_error(code: &str, message: &str, artifact_sha256: &str, reason: &str) -> Self {
        Self::new(
            code,
            "extraction",
            message,
            &json!({
                "artifact_sha256": valid_sha256_or_zero(artifact_sha256),
                "reason": bounded_nonempty(reason, 256, "invalid_artifact")
            }),
        )
    }

    fn new(code: &str, stage: &str, message: &str, context: &Value) -> Self {
        Self {
            value: json!({
                "schema_version": R7_ERROR_VERSION,
                "code": code,
                "stage": stage,
                "message": message,
                "retryable": false,
                "context": context
            }),
        }
    }

    /// Serializes one strict `ErrorV14` followed by one LF.
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
pub struct LocalQueryResultV5 {
    value: Value,
}

impl LocalQueryResultV5 {
    /// Serializes one bounded exact-ID V5 result followed by one LF.
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

/// Builds one strict V10 exact-ID query result without adding read authority.
///
/// # Errors
///
/// Returns a strict snapshot, document, not-found, or output-limit failure.
pub fn local_query_result_v5(
    semantic: &Value,
    manifest: &Value,
    snapshot_id: &str,
    requested_id: &str,
) -> Result<LocalQueryResultV5, QueryContractError> {
    let mut value = match local_query_result_v4(semantic, manifest, snapshot_id, requested_id) {
        Ok(result) => result.into_value(),
        Err(QueryContractError::InvalidSnapshot) => {
            empty_evidence_coverage_result(semantic, manifest, snapshot_id, requested_id)?
        }
        Err(error) => return Err(error),
    };
    value["schema_version"] = Value::String(R7_QUERY_VERSION.to_owned());
    let result = LocalQueryResultV5 { value };
    result.canonical_stdout()?;
    Ok(result)
}

fn empty_evidence_coverage_result(
    semantic: &Value,
    manifest: &Value,
    snapshot_id: &str,
    requested_id: &str,
) -> Result<Value, QueryContractError> {
    let index = GraphIndex::new(semantic).map_err(|_| QueryContractError::InvalidSnapshot)?;
    validate_manifest_binding(manifest, &index.repository_identity, snapshot_id)?;
    let gap = index
        .coverage
        .get(requested_id)
        .ok_or(QueryContractError::InvalidSnapshot)?;
    if !string_array(gap, "evidence_ids")
        .map_err(|_| QueryContractError::InvalidSnapshot)?
        .is_empty()
    {
        return Err(QueryContractError::InvalidSnapshot);
    }
    let documents = manifest
        .get("documents")
        .and_then(Value::as_array)
        .ok_or(QueryContractError::InvalidDocuments)?;
    Ok(json!({
        "schema_version": R7_QUERY_VERSION,
        "repository_identity": index.repository_identity,
        "snapshot_id": snapshot_id,
        "requested_id": requested_id,
        "result_kind": "coverage_gap",
        "entity": null,
        "relationship": null,
        "claims": [],
        "evidence": [],
        "diagnostic": null,
        "coverage_gap": gap,
        "document": null,
        "document_statements": linked_statements(documents, requested_id)?
    }))
}

#[derive(Clone, Debug)]
pub struct RepositorySnapshotV10 {
    value: Value,
}

#[derive(Debug)]
pub enum RepositorySnapshotV10Error {
    Serialization(serde_json::Error),
    LimitExceeded(AcquisitionError),
    ContractInvalid,
    OutputLengthOverflow,
}

impl Display for RepositorySnapshotV10Error {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Serialization(_) => "R7 snapshot serialization failed",
            Self::LimitExceeded(_) => "R7 snapshot output limit exceeded",
            Self::ContractInvalid => "R7 snapshot contract is invalid",
            Self::OutputLengthOverflow => "R7 snapshot output length overflowed",
        })
    }
}

impl Error for RepositorySnapshotV10Error {}

impl RepositorySnapshotV10 {
    /// Builds selector-bound V10 over the complete immutable V9 lineage.
    ///
    /// # Errors
    ///
    /// Returns a compiler-overlay, serialization, publication, or output-bound failure.
    pub fn from_inventory_and_compiler_index(
        inventory: &RepositoryInventory,
        source: &FrameworkKnowledge,
        overlay: &CompilerIndexOverlay,
        boundaries: Option<&RepositoryBoundaryReport>,
        envelope: SnapshotEnvelopeV1,
    ) -> Result<Self, RepositorySnapshotV10Error> {
        overlay
            .validate(source)
            .map_err(|_| RepositorySnapshotV10Error::ContractInvalid)?;
        let baseline = RepositorySnapshotV9::from_inventory_and_framework_declarations(
            inventory, source, boundaries, envelope,
        )
        .map_err(map_v9_error)?;
        let mut value = baseline.value().clone();
        let semantic = value
            .get_mut("semantic")
            .and_then(Value::as_object_mut)
            .ok_or(RepositorySnapshotV10Error::ContractInvalid)?;
        let boundary_profile = boundaries.map(|_| "local-gitlinks-v1");
        let configuration_without_hash = json!({
            "schema_version": R7_CONFIGURATION_VERSION,
            "profile": "standard-local-s4",
            "workspace_profile": R3_WORKSPACE_PROFILE,
            "manifest_profile": R4_MANIFEST_PROFILE,
            "rust_semantic_profile": R5_RUST_SEMANTIC_PROFILE,
            "rust_framework_profile": R6_FRAMEWORK_PROFILE,
            "compiler_index_profile": R7_COMPILER_INDEX_PROFILE,
            "compiler_index_binding_sha256": overlay.binding_sha256,
            "repository_boundary_profile": boundary_profile
        });
        let configuration_hash =
            semantic_hash(CONFIGURATION_V7_HASH_DOMAIN, &configuration_without_hash);
        let mut configuration = configuration_without_hash;
        configuration
            .as_object_mut()
            .ok_or(RepositorySnapshotV10Error::ContractInvalid)?
            .insert(
                "semantic_hash".to_owned(),
                json!({"algorithm": "blake3-256", "value": configuration_hash}),
            );
        semantic.insert("configuration".to_owned(), configuration);
        semantic.insert(
            "pipeline_version".to_owned(),
            Value::String(R7_PIPELINE_VERSION.to_owned()),
        );
        semantic.insert(
            "ontology_version".to_owned(),
            Value::String(R7_ONTOLOGY_VERSION.to_owned()),
        );
        semantic.insert(
            "extractor_contract_version".to_owned(),
            Value::String(R7_EXTRACTION_CONTRACT_VERSION.to_owned()),
        );
        let mut extractor_versions = vec![
            Value::String("codenoesis.inventory-classifier/s1-v1".to_owned()),
            Value::String("codenoesis.rust-tree-sitter/s4-v1".to_owned()),
            Value::String("codenoesis.rust-workspace/s4-r3-v1".to_owned()),
            Value::String("codenoesis.cargo-manifest/s4-r4-v1".to_owned()),
            Value::String(R5_RUST_SEMANTIC_EXTRACTOR_VERSION.to_owned()),
            Value::String(R6_FRAMEWORK_EXTRACTOR_VERSION.to_owned()),
            Value::String(R7_COMPILER_EXTRACTOR_VERSION.to_owned()),
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
            .ok_or(RepositorySnapshotV10Error::ContractInvalid)?;
        semantic.insert(
            "extraction_chunks".to_owned(),
            Value::Array(extraction_chunks_v7(&baseline_chunks, overlay)?),
        );
        let baseline_graph = semantic
            .get("knowledge_graph")
            .cloned()
            .ok_or(RepositorySnapshotV10Error::ContractInvalid)?;
        semantic.insert(
            "knowledge_graph".to_owned(),
            knowledge_graph_v7(&baseline_graph, overlay)?,
        );
        let semantic_value = Value::Object(semantic.clone());
        let snapshot_hash = semantic_hash(SNAPSHOT_V10_HASH_DOMAIN, &semantic_value);
        let root = value
            .as_object_mut()
            .ok_or(RepositorySnapshotV10Error::ContractInvalid)?;
        root.insert(
            "schema_version".to_owned(),
            Value::String(R7_SNAPSHOT_VERSION.to_owned()),
        );
        root.insert(
            "semantic_hash".to_owned(),
            json!({"algorithm": "blake3-256", "value": snapshot_hash}),
        );
        publication_candidate(&value).map_err(|_| RepositorySnapshotV10Error::ContractInvalid)?;
        Ok(Self { value })
    }

    /// Serializes the complete V10 snapshot with the inherited output bound.
    ///
    /// # Errors
    ///
    /// Returns a typed serialization or output-limit failure.
    pub fn canonical_stdout(&self) -> Result<Vec<u8>, RepositorySnapshotV10Error> {
        let maximum = usize::try_from(STANDARD_LOCAL_S1_LIMITS.canonical_output_bytes)
            .map_err(|_| RepositorySnapshotV10Error::OutputLengthOverflow)?;
        let body_maximum = maximum
            .checked_sub(1)
            .ok_or(RepositorySnapshotV10Error::OutputLengthOverflow)?;
        let mut writer = LimitedVecWriter::new(body_maximum);
        let result = serde_json::to_writer(&mut writer, &self.value);
        if writer.overflowed() {
            return Err(RepositorySnapshotV10Error::LimitExceeded(limit_exceeded(
                LimitKind::CanonicalOutputBytes,
                STANDARD_LOCAL_S1_LIMITS
                    .canonical_output_bytes
                    .saturating_add(1),
            )));
        }
        result.map_err(RepositorySnapshotV10Error::Serialization)?;
        let mut bytes = writer.into_inner();
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Serializes the exact V10 semantic payload stored by S4.
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

    /// Converts V10 into the unchanged immutable publication model.
    ///
    /// # Errors
    ///
    /// Returns a strict contract or storage-integrity failure.
    pub fn publication_candidate(&self) -> Result<PublicationCandidate, PublicationCandidateError> {
        publication_candidate(&self.value)
    }
}

/// Validates one loaded V10 semantic payload against the complete visible head.
///
/// # Errors
///
/// Returns a typed metadata-integrity failure on any binding mismatch.
pub fn validate_stored_snapshot_semantic_v10(
    semantic: &Value,
    head: &LocalSnapshotHead,
) -> Result<(), StorageError> {
    if head.snapshot_schema_version != SNAPSHOT_SCHEMA_VERSION_V10 {
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

fn extraction_chunks_v7(
    baseline: &Value,
    overlay: &CompilerIndexOverlay,
) -> Result<Vec<Value>, RepositorySnapshotV10Error> {
    let chunks = baseline
        .as_array()
        .ok_or(RepositorySnapshotV10Error::ContractInvalid)?;
    let mut transformed = chunks.clone();
    for chunk in &mut transformed {
        let object = chunk
            .as_object_mut()
            .ok_or(RepositorySnapshotV10Error::ContractInvalid)?;
        object.insert(
            "schema_version".to_owned(),
            Value::String(R7_EXTRACTION_CHUNK_VERSION.to_owned()),
        );
        object.insert(
            "ontology_version".to_owned(),
            Value::String(R7_ONTOLOGY_VERSION.to_owned()),
        );
        object.remove("semantic_hash");
    }

    for symbol in &overlay.symbols {
        merge_chunk_value(
            &mut transformed,
            symbol.document_path.as_deref(),
            &overlay.primary_document_path,
            "entities",
            compiler_symbol_value(symbol),
        )?;
    }
    for reference in &overlay.syntax_references {
        merge_chunk_value(
            &mut transformed,
            Some(&reference.document_path),
            &overlay.primary_document_path,
            "entities",
            entity_value(&reference.entity),
        )?;
    }
    for relationship in &overlay.relationships {
        merge_chunk_value(
            &mut transformed,
            relationship.document_path.as_deref(),
            &overlay.primary_document_path,
            "relationships",
            compiler_relationship_value(relationship),
        )?;
    }
    for claim in &overlay.claims {
        let document_path = claim_document_path(claim, overlay)?;
        merge_chunk_value(
            &mut transformed,
            document_path,
            &overlay.primary_document_path,
            "claims",
            claim_value(claim),
        )?;
    }
    for evidence in &overlay.compiler_evidence {
        merge_chunk_value(
            &mut transformed,
            evidence.locator.document_path.as_deref(),
            &overlay.primary_document_path,
            "evidence",
            compiler_evidence_value(evidence),
        )?;
    }
    for evidence in &overlay.source_evidence {
        merge_chunk_value(
            &mut transformed,
            Some(&evidence.path),
            &overlay.primary_document_path,
            "evidence",
            source_evidence_value(evidence),
        )?;
    }
    for diagnostic in &overlay.diagnostics {
        merge_chunk_value(
            &mut transformed,
            diagnostic.document_path.as_deref(),
            &overlay.primary_document_path,
            "diagnostics",
            compiler_diagnostic_value(diagnostic),
        )?;
    }
    for gap in &overlay.coverage {
        let document_path = (gap.capability != "compiler_index.document_not_indexed")
            .then_some(gap.document_path.as_deref())
            .flatten();
        merge_chunk_value(
            &mut transformed,
            document_path,
            &overlay.primary_document_path,
            "coverage",
            compiler_coverage_value(gap),
        )?;
    }
    for chunk in &mut transformed {
        insert_semantic_hash(chunk, EXTRACTION_V7_HASH_DOMAIN)?;
    }
    Ok(transformed)
}

fn knowledge_graph_v7(
    baseline: &Value,
    overlay: &CompilerIndexOverlay,
) -> Result<Value, RepositorySnapshotV10Error> {
    let mut value = baseline.clone();
    let object = value
        .as_object_mut()
        .ok_or(RepositorySnapshotV10Error::ContractInvalid)?;
    object.insert(
        "schema_version".to_owned(),
        Value::String(R7_GRAPH_VERSION.to_owned()),
    );
    object.insert(
        "ontology_version".to_owned(),
        Value::String(R7_ONTOLOGY_VERSION.to_owned()),
    );
    object.insert(
        "extractor_versions".to_owned(),
        json!([
            "codenoesis.rust-tree-sitter/s4-v1",
            "codenoesis.rust-workspace/s4-r3-v1",
            "codenoesis.cargo-manifest/s4-r4-v1",
            R5_RUST_SEMANTIC_EXTRACTOR_VERSION,
            R6_FRAMEWORK_EXTRACTOR_VERSION,
            R7_COMPILER_EXTRACTOR_VERSION
        ]),
    );
    object.insert("compiler_index".to_owned(), compiler_index_value(overlay));
    object.remove("semantic_hash");
    merge_id_array(
        object,
        "entities",
        overlay.symbols.iter().map(compiler_symbol_value).chain(
            overlay
                .syntax_references
                .iter()
                .map(|value| entity_value(&value.entity)),
        ),
    )?;
    merge_id_array(
        object,
        "relationships",
        overlay
            .relationships
            .iter()
            .map(compiler_relationship_value),
    )?;
    merge_id_array(object, "claims", overlay.claims.iter().map(claim_value))?;
    merge_id_array(
        object,
        "evidence",
        overlay
            .compiler_evidence
            .iter()
            .map(compiler_evidence_value)
            .chain(overlay.source_evidence.iter().map(source_evidence_value)),
    )?;
    merge_id_array(
        object,
        "diagnostics",
        overlay.diagnostics.iter().map(compiler_diagnostic_value),
    )?;
    merge_id_array(
        object,
        "coverage",
        overlay.coverage.iter().map(compiler_coverage_value),
    )?;
    insert_semantic_hash(&mut value, GRAPH_V7_HASH_DOMAIN)?;
    Ok(value)
}

fn compiler_index_value(overlay: &CompilerIndexOverlay) -> Value {
    let compiler_symbol_ids = overlay
        .symbols
        .iter()
        .map(|value| value.id.clone())
        .collect::<Vec<_>>();
    let state_ids = |state| {
        overlay
            .symbols
            .iter()
            .filter(|value| value.binding_state == state)
            .map(|value| value.id.clone())
            .collect::<Vec<_>>()
    };
    json!({
        "schema_version": R7_COMPILER_INDEX_VERSION,
        "profile": R7_COMPILER_INDEX_PROFILE,
        "binding_sha256": overlay.binding_sha256,
        "artifact_sha256": overlay.artifact_sha256,
        "producer": {
            "family": overlay.producer.family,
            "name": overlay.producer.name,
            "version": overlay.producer.version,
            "commit": overlay.producer.commit,
            "arguments_sha256": overlay.producer.arguments_sha256,
            "project_root_sha256": overlay.producer.project_root_sha256,
            "attested": false
        },
        "toolchain": {
            "channel": overlay.toolchain.channel,
            "rustc_release": overlay.toolchain.rustc_release,
            "rustc_commit": overlay.toolchain.rustc_commit,
            "target_triple": overlay.toolchain.target_triple
        },
        "coverage_mode": overlay.coverage_mode,
        "compiler_symbol_ids": compiler_symbol_ids,
        "in_repository_symbol_ids": state_ids(CompilerBindingState::InRepositoryBound),
        "external_symbol_ids": state_ids(CompilerBindingState::ExternalUnbound),
        "generated_symbol_ids": state_ids(CompilerBindingState::GeneratedUnbound),
        "compiler_relationship_ids": overlay
            .relationships
            .iter()
            .map(|value| value.id.clone())
            .collect::<Vec<_>>()
    })
}

fn compiler_symbol_value(symbol: &CompilerSymbol) -> Value {
    json!({
        "id": symbol.id,
        "kind": "compiler.symbol",
        "symbol": symbol.symbol,
        "display_name": symbol.display_name,
        "scope": symbol.scope,
        "binding_state": symbol.binding_state.as_str(),
        "identity_preimage": symbol.identity_preimage,
        "source_entity_id": symbol.source_entity_id,
        "compiler_evidence_ids": symbol.compiler_evidence_ids,
        "source_evidence_ids": symbol.source_evidence_ids
    })
}

fn compiler_relationship_value(relationship: &CompilerRelationship) -> Value {
    json!({
        "id": relationship.id,
        "kind": relationship.kind.as_str(),
        "source": relationship.source,
        "target": relationship.target,
        "evidence_ids": relationship.evidence_ids,
        "provenance": "validated_scip_v0.9.0",
        "endpoint_binding": "unique"
    })
}

fn compiler_evidence_value(evidence: &CompilerEvidence) -> Value {
    json!({
        "id": evidence.id,
        "artifact_sha256": evidence.artifact_sha256,
        "record_kind": evidence.locator.record_kind.as_str(),
        "document_path": evidence.locator.document_path,
        "range": evidence.locator.range,
        "symbol": evidence.locator.symbol,
        "symbol_roles": evidence.locator.symbol_roles,
        "relationship_target": evidence.locator.relationship_target,
        "relationship_flags": evidence.locator.relationship_flags
    })
}

fn source_evidence_value(evidence: &CompilerSourceEvidence) -> Value {
    json!({
        "id": evidence.id,
        "path": evidence.path,
        "blob_oid": evidence.blob_oid,
        "start_byte": evidence.start_byte,
        "end_byte": evidence.end_byte
    })
}

fn compiler_diagnostic_value(diagnostic: &CompilerDiagnostic) -> Value {
    json!({
        "id": diagnostic.id,
        "code": diagnostic.code,
        "subject_id": diagnostic.subject_id,
        "compiler_target_id": diagnostic.compiler_target_id,
        "evidence_ids": diagnostic.evidence_ids
    })
}

fn compiler_coverage_value(gap: &CompilerCoverageGap) -> Value {
    json!({
        "id": gap.id,
        "subject": gap.subject,
        "capability": gap.capability,
        "state": gap.state.as_str(),
        "evidence_ids": gap.evidence_ids
    })
}

fn claim_document_path<'a>(
    claim: &WorkspaceClaim,
    overlay: &'a CompilerIndexOverlay,
) -> Result<Option<&'a str>, RepositorySnapshotV10Error> {
    if let Some(symbol) = overlay
        .symbols
        .iter()
        .find(|value| value.id == claim.subject_id)
    {
        return Ok(symbol.document_path.as_deref());
    }
    if let Some(reference) = overlay
        .syntax_references
        .iter()
        .find(|value| value.entity.id == claim.subject_id)
    {
        return Ok(Some(&reference.document_path));
    }
    if let Some(relationship) = overlay
        .relationships
        .iter()
        .find(|value| value.id == claim.subject_id)
    {
        return Ok(relationship.document_path.as_deref());
    }
    Err(RepositorySnapshotV10Error::ContractInvalid)
}

fn merge_chunk_value(
    chunks: &mut [Value],
    document_path: Option<&str>,
    primary_document_path: &str,
    field: &'static str,
    addition: Value,
) -> Result<(), RepositorySnapshotV10Error> {
    let document_path = document_path.unwrap_or(primary_document_path);
    let chunk = chunks
        .iter_mut()
        .find(|chunk| chunk_contains_path(chunk, document_path))
        .ok_or(RepositorySnapshotV10Error::ContractInvalid)?;
    let object = chunk
        .as_object_mut()
        .ok_or(RepositorySnapshotV10Error::ContractInvalid)?;
    merge_id_array(object, field, [addition])
}

fn chunk_contains_path(chunk: &Value, path: &str) -> bool {
    chunk
        .get("evidence")
        .and_then(Value::as_array)
        .is_some_and(|evidence| {
            evidence
                .iter()
                .any(|value| value.get("path").and_then(Value::as_str) == Some(path))
        })
}

fn merge_id_array(
    object: &mut Map<String, Value>,
    field: &'static str,
    additions: impl IntoIterator<Item = Value>,
) -> Result<(), RepositorySnapshotV10Error> {
    let values = object
        .get_mut(field)
        .and_then(Value::as_array_mut)
        .ok_or(RepositorySnapshotV10Error::ContractInvalid)?;
    values.extend(additions);
    values.sort_by(|left, right| identifier(left).cmp(&identifier(right)));
    let mut seen = BTreeSet::new();
    let mut retained = Vec::with_capacity(values.len());
    for value in values.drain(..) {
        let id = identifier(&value)
            .ok_or(RepositorySnapshotV10Error::ContractInvalid)?
            .to_owned();
        if seen.insert(id) {
            retained.push(value);
        } else if retained.last() != Some(&value) {
            return Err(RepositorySnapshotV10Error::ContractInvalid);
        }
    }
    *values = retained;
    Ok(())
}

fn identifier(value: &Value) -> Option<&str> {
    value.get("id").and_then(Value::as_str)
}

fn insert_semantic_hash(
    value: &mut Value,
    domain: &[u8],
) -> Result<(), RepositorySnapshotV10Error> {
    let hash = semantic_hash(domain, value);
    value
        .as_object_mut()
        .ok_or(RepositorySnapshotV10Error::ContractInvalid)?
        .insert(
            "semantic_hash".to_owned(),
            json!({"algorithm": "blake3-256", "value": hash}),
        );
    Ok(())
}

fn map_v9_error(error: RepositorySnapshotV9Error) -> RepositorySnapshotV10Error {
    match error {
        RepositorySnapshotV9Error::Serialization(error) => {
            RepositorySnapshotV10Error::Serialization(error)
        }
        RepositorySnapshotV9Error::LimitExceeded(error) => {
            RepositorySnapshotV10Error::LimitExceeded(error)
        }
        RepositorySnapshotV9Error::ContractInvalid => RepositorySnapshotV10Error::ContractInvalid,
        RepositorySnapshotV9Error::OutputLengthOverflow => {
            RepositorySnapshotV10Error::OutputLengthOverflow
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

fn bounded_safe_path(value: &str) -> String {
    let value = bounded_nonempty(value, 4096, "compiler-index-input");
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
            .any(|component| component.is_empty() || component == "..")
    {
        "compiler-index-input".to_owned()
    } else {
        value
    }
}

fn valid_sha256_or_zero(value: &str) -> String {
    if valid_lower_hex(value, 64) {
        value.to_owned()
    } else {
        "0".repeat(64)
    }
}

fn valid_evidence_id_or_zero(value: &str) -> String {
    if value
        .strip_prefix("urn:codenoesis:evidence:sha256:")
        .is_some_and(|digest| valid_lower_hex(digest, 64))
    {
        value.to_owned()
    } else {
        format!("urn:codenoesis:evidence:sha256:{}", "0".repeat(64))
    }
}

fn valid_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}
