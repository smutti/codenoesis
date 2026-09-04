use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::path::{Component, Path};

use codenoesis_domain::s1_boundaries::RepositoryBoundaryReport;
use codenoesis_domain::s4_k1::{
    CallSiteProperties, CallableCoverageGap, CallableDiagnostic, CallableRelationship,
    CallableSemanticEntity, CallableSemanticProperties, CallableSemanticsError,
    CallableSourceChunk, ControlProperties, DeclaredValueProperties, K1_EXTRACTOR_VERSION,
    K1_INDEX_VERSION, K1_PROFILE, LocalBindingProperties, NormalizedScalarValue,
};
use codenoesis_domain::s4_r3::R3_WORKSPACE_PROFILE;
use codenoesis_domain::s4_r4::R4_MANIFEST_PROFILE;
use codenoesis_domain::s4_r5::{R5_RUST_SEMANTIC_EXTRACTOR_VERSION, RustSemanticError};
use codenoesis_domain::s4_r6::{
    FrameworkError, R6_FRAMEWORK_EXTRACTOR_VERSION, R6_FRAMEWORK_PROFILE,
};
use codenoesis_domain::s4_r10::{
    HAS_DECLARATION_ALTERNATIVE, R10_EXTRACTOR_VERSION, R10_INDEX_VERSION, R10_PROFILE,
    RustCfgDeclarationAlternativesError, RustCfgDeclarationAlternativesSourceChunk,
    RustDeclarationAlternative, RustDeclarationAlternativeRelationship,
};
use codenoesis_domain::s4_r12::{CallableCfgAlternativesError, CallableCfgAlternativesKnowledge};
pub use codenoesis_domain::s4_r12::{
    R12_COMPOSITION_VERSION, R12_CONFIGURATION_VERSION, R12_ERROR_VERSION,
    R12_EXTRACTION_CHUNK_VERSION, R12_EXTRACTION_CONTRACT_VERSION, R12_EXTRACTOR_VERSION,
    R12_GRAPH_VERSION, R12_INDEX_VERSION, R12_LOCAL_EXPLORER_VERSION, R12_ONTOLOGY_VERSION,
    R12_PIPELINE_VERSION, R12_PORTABLE_GRAPH_VERSION, R12_QUERY_VERSION, R12_SNAPSHOT_VERSION,
};
use codenoesis_domain::storage::{
    LocalSnapshotHead, PublicationCandidate, SNAPSHOT_SCHEMA_VERSION_V14, StorageComponent,
    StorageError,
};
use codenoesis_domain::{
    AcquisitionError, K1OutputCapacityProfile, LimitKind, RepositoryInventory,
};
use serde_json::{Map, Value, json};

use super::s1_boundaries::CodeNoesisErrorV9;
use super::s4::{MAX_QUERY_BYTES, QueryContractError, claim_value, evidence_value};
use super::s4_r6::{RepositorySnapshotV9, RepositorySnapshotV9Error, local_query_result_v4};
use super::{
    LimitedVecWriter, PublicationCandidateError, SnapshotEnvelopeV1, publication_candidate,
    repository_boundary_value, semantic_hash, validate_repository_boundary_report_size,
};

const CONFIGURATION_V11_HASH_DOMAIN: &[u8] = b"codenoesis.configuration.semantic.v11";
const SNAPSHOT_V14_HASH_DOMAIN: &[u8] = b"codenoesis.repository-snapshot.semantic.v14";
const EXTRACTION_V11_HASH_DOMAIN: &[u8] = b"codenoesis.extraction-chunk.semantic.v11";
const GRAPH_V11_HASH_DOMAIN: &[u8] = b"codenoesis.knowledge-graph.semantic.v11";
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

pub type R12Sha256 = fn(&[u8]) -> String;
pub const R12_PORTABLE_MARKER: &str = ".codenoesis-portable-graph-v5";
pub const R12_EXPLORER_MARKER: &str = ".codenoesis-local-explorer-v5";
pub const R12_EXPLORER_SECURITY_PROFILE: &str = "codenoesis.local-explorer-security/v5";
pub const MAX_R12_PORTABLE_GRAPH_BYTES: u64 = 268_435_456;
pub const MAX_R12_JSON_NESTING: u64 = 64;
pub const MAX_R12_TEXT_SEARCH_RESULTS: u64 = 100;
pub const R12_TRAVERSAL_DEPTH_DEFAULT: u64 = 1;
pub const MAX_R12_TRAVERSAL_DEPTH: u64 = 2;

#[derive(Clone, Debug)]
pub struct CodeNoesisErrorV19 {
    value: Value,
}

impl CodeNoesisErrorV19 {
    #[must_use]
    pub fn invalid_profile(field: &str, profile: &str) -> Self {
        match field {
            "rust_semantic_profile" => Self::new(
                "input.invalid_rust_cfg_alternatives_profile",
                "input",
                "invalid rust cfg declaration alternatives profile",
                &json!({"provided_profile": bounded(profile, 256)}),
            ),
            "rust_callable_profile" => Self::new(
                "input.invalid_rust_callable_profile",
                "input",
                "invalid rust callable profile",
                &json!({"provided_profile": bounded(profile, 256)}),
            ),
            "repository_boundary_profile" => Self::new(
                "input.invalid_repository_boundary_profile",
                "input",
                "invalid repository boundary profile",
                &json!({"provided_profile": bounded(profile, 256)}),
            ),
            _ => Self::unsupported_composition("invalid_optional_profile"),
        }
    }

    #[must_use]
    pub fn unsupported_composition(reason: &str) -> Self {
        Self::new(
            "input.unsupported_rust_callable_cfg_alternatives_composition",
            "input",
            "unsupported R12 profile composition",
            &json!({
                "semantic_profile": R10_PROFILE,
                "framework_profile": R6_FRAMEWORK_PROFILE,
                "callable_profile": K1_PROFILE,
                "reason": bounded(reason, 128)
            }),
        )
    }

    #[must_use]
    pub fn from_extraction(error: &CallableCfgAlternativesError) -> Self {
        match error {
            CallableCfgAlternativesError::Alternatives(error) => Self::from_alternatives(error),
            CallableCfgAlternativesError::Callable(error) => Self::from_callable(error),
            CallableCfgAlternativesError::LogicalMethodHasOccurrenceShape => Self::new(
                "extraction.callable_cfg_logical_shape_forbidden",
                "extraction",
                "logical cfg-alternative method has direct callable shape",
                &json!({}),
            ),
            CallableCfgAlternativesError::AlternativeSubjectMismatch {
                alternative_id,
                observed_subject_id,
            } => Self::new(
                "extraction.callable_cfg_alternative_subject_mismatch",
                "extraction",
                "declaration alternative callable subject does not match its occurrence",
                &json!({
                    "alternative_id": bounded(alternative_id, 256),
                    "observed_subject_id": bounded(observed_subject_id, 256)
                }),
            ),
            CallableCfgAlternativesError::AlternativeSignatureCardinality {
                alternative_id,
                observed,
            } => Self::new(
                if *observed == 0 {
                    "extraction.callable_cfg_alternative_signature_missing"
                } else {
                    "extraction.callable_cfg_alternative_signature_duplicate"
                },
                "extraction",
                "declaration alternative signature cardinality is invalid",
                &json!({
                    "alternative_id": bounded(alternative_id, 256),
                    "observed": observed
                }),
            ),
            CallableCfgAlternativesError::OccurrenceEvidenceMismatch { alternative_id } => {
                Self::new(
                    "extraction.callable_cfg_evidence_invalid",
                    "extraction",
                    "callable evidence does not close over its declaration alternative",
                    &json!({"alternative_id": bounded(alternative_id, 256)}),
                )
            }
            CallableCfgAlternativesError::ContractInvalid => Self::invalid_snapshot(),
        }
    }

    #[must_use]
    pub fn internal(stage: &str) -> Self {
        Self::new(
            "internal.unexpected",
            "internal",
            "unexpected internal R12 failure",
            &json!({"stage": bounded(stage, 128)}),
        )
    }

    #[must_use]
    pub fn invalid_snapshot() -> Self {
        Self::new(
            "snapshot.invalid_v14",
            "snapshot",
            "invalid R12 snapshot",
            &json!({}),
        )
    }

    #[must_use]
    pub fn invalid_query(reason: &str) -> Self {
        Self::new(
            "query.invalid_v14",
            "query",
            "invalid R12 query state",
            &json!({"reason": bounded(reason, 256)}),
        )
    }

    #[must_use]
    pub fn unsafe_output_path(path_sha256: &str, reason: &str) -> Self {
        Self::new(
            "input.unsafe_output_path",
            "input",
            "unsafe R12 output path",
            &json!({
                "path_sha256": bounded(path_sha256, 64),
                "reason": bounded(reason, 128)
            }),
        )
    }

    #[must_use]
    pub fn from_boundary_error(error: &CodeNoesisErrorV9) -> Self {
        let mut value = error.value().clone();
        value["schema_version"] = Value::String(R12_ERROR_VERSION.to_owned());
        value["retryable"] = Value::Bool(false);
        Self { value }
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
                "R12 acquisition limit exceeded",
                &json!({
                    "limit": limit.as_str(),
                    "maximum": maximum,
                    "observed": observed
                }),
            )),
            _ => None,
        }
    }

    #[must_use]
    pub fn from_contract(error: &R12ContractError, explore: bool) -> Self {
        match error {
            R12ContractError::UnsupportedSnapshotSchema(observed) => Self::new(
                "export.invalid_snapshot",
                "export",
                "invalid R12 source snapshot",
                &json!({"observed": bounded(observed, 256)}),
            ),
            R12ContractError::InvalidSnapshot => Self::new(
                "export.invalid_snapshot",
                "export",
                "invalid R12 source snapshot",
                &json!({}),
            ),
            R12ContractError::LimitExceeded {
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
                "R12 portable graph limit exceeded",
                &json!({"limit": limit, "maximum": maximum, "observed": observed}),
            ),
            R12ContractError::AssetIntegrityMismatch => Self::new(
                "explorer.asset_integrity_mismatch",
                "explorer",
                "R12 explorer asset integrity mismatch",
                &json!({}),
            ),
            R12ContractError::Internal => Self::internal("contract"),
            R12ContractError::UnsupportedPortableGraphSchema(_)
            | R12ContractError::Noncanonical { .. }
            | R12ContractError::IdentityConflict { .. }
            | R12ContractError::ReferenceMismatch { .. }
            | R12ContractError::UnsafePayload { .. }
            | R12ContractError::InvalidProjection => Self::new(
                "export.invalid_portable_graph_v5",
                "export",
                "invalid portable graph V5",
                &json!({"reason": bounded(&error.to_string(), 256)}),
            ),
        }
    }

    fn from_alternatives(error: &RustCfgDeclarationAlternativesError) -> Self {
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
            RustCfgDeclarationAlternativesError::Source(error) => Self::from_rust_semantic(error),
            RustCfgDeclarationAlternativesError::ContractInvalid => Self::invalid_snapshot(),
        }
    }

    fn from_callable(error: &CallableSemanticsError) -> Self {
        match error {
            CallableSemanticsError::Source(FrameworkError::Source(error)) => {
                Self::from_rust_semantic(error)
            }
            CallableSemanticsError::Source(_) => Self::internal("framework_extraction"),
            CallableSemanticsError::InvalidSyntax {
                path,
                start_byte,
                syntax_kind,
            } => Self::new(
                "extraction.invalid_callable_syntax",
                "extraction",
                "invalid rust callable syntax",
                &json!({
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
                &json!({
                    "kind": bounded(kind, 128),
                    "normalized_identity": bounded(normalized_identity, 512)
                }),
            ),
            CallableSemanticsError::UnsupportedComposition => {
                Self::unsupported_composition("callable_composition")
            }
            CallableSemanticsError::LimitExceeded {
                limit,
                maximum,
                observed,
            } => Self::new(
                "extraction.callable_limit_exceeded",
                "extraction",
                "rust callable limit exceeded",
                &json!({"limit": limit.as_str(), "maximum": maximum, "observed": observed}),
            ),
            CallableSemanticsError::ContractInvalid => Self::invalid_snapshot(),
        }
    }

    fn from_rust_semantic(error: &RustSemanticError) -> Self {
        match error {
            RustSemanticError::InvalidDeclaration {
                path,
                start_byte,
                declaration_kind,
            } => Self::new(
                "extraction.invalid_rust_source",
                "extraction",
                "invalid rust source",
                &json!({
                    "path": bounded(path, 1024),
                    "start_byte": start_byte,
                    "declaration_kind": bounded(declaration_kind, 128)
                }),
            ),
            RustSemanticError::IdentityConflict {
                owner_id,
                member_kind,
                normalized_member,
            } => Self::new(
                "extraction.rust_semantic_identity_conflict",
                "extraction",
                "rust semantic identity conflict",
                &json!({
                    "owner_id": owner_id,
                    "member_kind": member_kind,
                    "normalized_member": bounded(normalized_member, 512)
                }),
            ),
            RustSemanticError::LimitExceeded {
                limit,
                maximum,
                observed,
            } => Self::new(
                "extraction.rust_cfg_alternative_limit_exceeded",
                "extraction",
                "rust cfg declaration alternative limit exceeded",
                &json!({"limit": limit.as_str(), "maximum": maximum, "observed": observed}),
            ),
            RustSemanticError::UnsupportedComposition { .. }
            | RustSemanticError::Source(_)
            | RustSemanticError::ContractInvalid => Self::internal("rust_semantic"),
        }
    }

    fn new(code: &str, stage: &str, message: &str, context: &Value) -> Self {
        Self {
            value: json!({
                "schema_version": R12_ERROR_VERSION,
                "code": code,
                "stage": stage,
                "message": message,
                "retryable": false,
                "context": context
            }),
        }
    }

    /// Serializes one `ErrorV19` followed by LF.
    ///
    /// # Errors
    ///
    /// Returns an error only when the internal JSON cannot be serialized.
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

#[derive(Clone, Debug)]
pub struct RepositorySnapshotV14 {
    value: Value,
}

#[derive(Debug)]
pub enum RepositorySnapshotV14Error {
    Serialization(serde_json::Error),
    LimitExceeded(AcquisitionError),
    ContractInvalid,
    OutputLengthOverflow,
}

impl Display for RepositorySnapshotV14Error {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Serialization(_) => "R12 snapshot serialization failed",
            Self::LimitExceeded(_) => "R12 snapshot output limit exceeded",
            Self::ContractInvalid => "R12 snapshot contract is invalid",
            Self::OutputLengthOverflow => "R12 snapshot output length overflowed",
        })
    }
}

impl Error for RepositorySnapshotV14Error {}

impl RepositorySnapshotV14 {
    /// Builds the additive V14 R10 + R6 + K1 composition.
    ///
    /// # Errors
    ///
    /// Returns the first lineage, boundary, serialization, publication, or output failure.
    #[allow(clippy::too_many_lines)]
    pub fn from_inventory_callable_cfg_alternatives(
        inventory: &RepositoryInventory,
        knowledge: &CallableCfgAlternativesKnowledge,
        boundaries: Option<&RepositoryBoundaryReport>,
        envelope: SnapshotEnvelopeV1,
    ) -> Result<Self, RepositorySnapshotV14Error> {
        knowledge
            .validate()
            .map_err(|_| RepositorySnapshotV14Error::ContractInvalid)?;
        if let Some(report) = boundaries {
            validate_repository_boundary_report_size(report)
                .map_err(|_| RepositorySnapshotV14Error::ContractInvalid)?;
        }
        let baseline = RepositorySnapshotV9::from_inventory_and_framework_declarations(
            inventory,
            &knowledge.callable.framework,
            boundaries,
            envelope,
        )
        .map_err(map_v9_error)?;
        let mut value = baseline.value().clone();
        let semantic = value
            .get_mut("semantic")
            .and_then(Value::as_object_mut)
            .ok_or(RepositorySnapshotV14Error::ContractInvalid)?;
        let configuration_without_hash = json!({
            "schema_version": R12_CONFIGURATION_VERSION,
            "profile": "standard-local-s4",
            "workspace_profile": R3_WORKSPACE_PROFILE,
            "manifest_profile": R4_MANIFEST_PROFILE,
            "rust_semantic_profile": R10_PROFILE,
            "rust_framework_profile": R6_FRAMEWORK_PROFILE,
            "rust_callable_profile": K1_PROFILE,
            "repository_boundary_profile": boundaries.map(|_| "local-gitlinks-v1")
        });
        let configuration_hash =
            semantic_hash(CONFIGURATION_V11_HASH_DOMAIN, &configuration_without_hash);
        let mut configuration = configuration_without_hash;
        configuration["semantic_hash"] =
            json!({"algorithm": "blake3-256", "value": configuration_hash});
        semantic.insert("configuration".to_owned(), configuration);
        semantic.insert(
            "pipeline_version".to_owned(),
            Value::String(R12_PIPELINE_VERSION.to_owned()),
        );
        semantic.insert(
            "ontology_version".to_owned(),
            Value::String(R12_ONTOLOGY_VERSION.to_owned()),
        );
        semantic.insert(
            "extractor_contract_version".to_owned(),
            Value::String(R12_EXTRACTION_CONTRACT_VERSION.to_owned()),
        );
        semantic.insert(
            "extractor_versions".to_owned(),
            Value::Array(extractor_versions(boundaries.is_some(), true)),
        );
        let baseline_chunks = semantic
            .get("extraction_chunks")
            .cloned()
            .ok_or(RepositorySnapshotV14Error::ContractInvalid)?;
        semantic.insert(
            "extraction_chunks".to_owned(),
            Value::Array(extraction_chunks_v11(&baseline_chunks, knowledge)?),
        );
        let baseline_graph = semantic
            .get("knowledge_graph")
            .cloned()
            .ok_or(RepositorySnapshotV14Error::ContractInvalid)?;
        semantic.insert(
            "knowledge_graph".to_owned(),
            knowledge_graph_v11(&baseline_graph, knowledge, boundaries.is_some())?,
        );
        semantic.insert(
            "repository_boundaries".to_owned(),
            boundaries.map_or(Value::Null, repository_boundary_value),
        );
        let semantic_value = Value::Object(semantic.clone());
        let snapshot_hash = semantic_hash(SNAPSHOT_V14_HASH_DOMAIN, &semantic_value);
        let root = value
            .as_object_mut()
            .ok_or(RepositorySnapshotV14Error::ContractInvalid)?;
        root.insert(
            "schema_version".to_owned(),
            Value::String(R12_SNAPSHOT_VERSION.to_owned()),
        );
        root.insert(
            "semantic_hash".to_owned(),
            json!({"algorithm": "blake3-256", "value": snapshot_hash}),
        );
        publication_candidate(&value).map_err(|_| RepositorySnapshotV14Error::ContractInvalid)?;
        Ok(Self { value })
    }

    /// Serializes V14 under the selected unchanged K1 output envelope.
    ///
    /// # Errors
    ///
    /// Returns a serialization or selected output-limit failure.
    pub fn canonical_stdout_with_output_capacity(
        &self,
        profile: K1OutputCapacityProfile,
    ) -> Result<Vec<u8>, RepositorySnapshotV14Error> {
        let maximum = usize::try_from(profile.maximum_bytes())
            .map_err(|_| RepositorySnapshotV14Error::OutputLengthOverflow)?;
        let body_maximum = maximum
            .checked_sub(1)
            .ok_or(RepositorySnapshotV14Error::OutputLengthOverflow)?;
        let mut writer = LimitedVecWriter::new(body_maximum);
        let result = serde_json::to_writer(&mut writer, &self.value);
        if writer.overflowed() {
            return Err(RepositorySnapshotV14Error::LimitExceeded(
                AcquisitionError::LimitExceeded {
                    limit: LimitKind::CanonicalOutputBytes,
                    maximum: profile.maximum_bytes(),
                    observed: profile.maximum_bytes().saturating_add(1),
                },
            ));
        }
        result.map_err(RepositorySnapshotV14Error::Serialization)?;
        let mut bytes = writer.into_inner();
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Serializes the exact V14 semantic payload.
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

    /// Converts V14 into the immutable publication model.
    ///
    /// # Errors
    ///
    /// Returns a strict contract or storage-integrity failure.
    pub fn publication_candidate(&self) -> Result<PublicationCandidate, PublicationCandidateError> {
        publication_candidate(&self.value)
    }
}

/// Validates one loaded V14 semantic payload against its visible head.
///
/// # Errors
///
/// Returns a typed storage-integrity failure on any mismatch.
pub fn validate_stored_snapshot_semantic_v14(
    semantic: &Value,
    head: &LocalSnapshotHead,
) -> Result<(), StorageError> {
    if head.snapshot_schema_version != SNAPSHOT_SCHEMA_VERSION_V14 {
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
pub struct LocalQueryResultV9 {
    value: Value,
}

impl LocalQueryResultV9 {
    /// Serializes one bounded exact-ID V9 result followed by LF.
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

/// Builds one exact-ID V9 result over the R10, K1, and optional R2 projections.
///
/// # Errors
///
/// Returns a strict snapshot, document, not-found, or result-limit failure.
pub fn local_query_result_v9(
    semantic: &Value,
    manifest: &Value,
    snapshot_id: &str,
    requested_id: &str,
) -> Result<LocalQueryResultV9, QueryContractError> {
    if semantic.get("ontology_version").and_then(Value::as_str) != Some(R12_ONTOLOGY_VERSION)
        || semantic
            .pointer("/knowledge_graph/schema_version")
            .and_then(Value::as_str)
            != Some(R12_GRAPH_VERSION)
    {
        return Err(QueryContractError::InvalidSnapshot);
    }
    let mut value = match local_query_result_v4(semantic, manifest, snapshot_id, requested_id) {
        Ok(result) => result.into_value(),
        Err(QueryContractError::NotFound)
            if semantic
                .get("repository_boundaries")
                .is_some_and(|value| !value.is_null()) =>
        {
            let mut compatible = semantic.clone();
            compatible["ontology_version"] =
                Value::String(codenoesis_domain::s4_r11::R11_ONTOLOGY_VERSION.to_owned());
            compatible["knowledge_graph"]["schema_version"] =
                Value::String(codenoesis_domain::s4_r11::R11_GRAPH_VERSION.to_owned());
            super::s4_r11::local_query_result_v8(&compatible, manifest, snapshot_id, requested_id)?
                .value()
                .clone()
        }
        Err(error) => return Err(error),
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
    let linked_r10_relationships = linked_relationships(relationships, requested_id, |kind| {
        kind == HAS_DECLARATION_ALTERNATIVE
    });
    let linked_k1_relationships = linked_relationships(relationships, requested_id, is_k1_kind);
    let linked_r10_entities = linked_entities(&entities, requested_id, &linked_r10_relationships);
    let linked_k1_entities = linked_entities(&entities, requested_id, &linked_k1_relationships)
        .into_iter()
        .filter(|entity| {
            entity
                .get("kind")
                .and_then(Value::as_str)
                .is_some_and(is_k1_entity_kind)
        })
        .collect::<Vec<_>>();
    let linked_subject_ids = linked_r10_entities
        .iter()
        .chain(linked_k1_entities.iter())
        .filter_map(record_id)
        .chain(
            linked_r10_relationships
                .iter()
                .chain(linked_k1_relationships.iter())
                .filter_map(record_id),
        )
        .chain(std::iter::once(requested_id))
        .collect::<BTreeSet<_>>();
    union_linked_claims_and_evidence(&mut value, graph, &linked_subject_ids)?;
    value["schema_version"] = Value::String(R12_QUERY_VERSION.to_owned());
    value["linked_r10_entities"] = Value::Array(linked_r10_entities);
    value["linked_r10_relationships"] = Value::Array(linked_r10_relationships);
    value["linked_k1_entities"] = Value::Array(linked_k1_entities);
    value["linked_k1_relationships"] = Value::Array(linked_k1_relationships);
    for field in [
        "repository_boundary",
        "boundary_declaration",
        "boundary_evidence",
        "boundary_coverage_gap",
    ] {
        if value.get(field).is_none() {
            value[field] = Value::Null;
        }
    }
    for field in [
        "linked_repository_boundaries",
        "linked_boundary_declarations",
        "linked_boundary_evidence",
        "linked_boundary_coverage_gaps",
    ] {
        if value.get(field).is_none() {
            value[field] = Value::Array(Vec::new());
        }
    }
    let result = LocalQueryResultV9 { value };
    result.canonical_stdout()?;
    Ok(result)
}

#[derive(Clone, Debug)]
pub struct PortableGraphV5 {
    value: Value,
    canonical: Vec<u8>,
    sha256: R12Sha256,
}

impl PortableGraphV5 {
    /// Projects one validated V14 head and deterministic documentation manifest.
    ///
    /// # Errors
    ///
    /// Returns a strict binding, privacy, reference, or limit failure.
    pub fn from_validated_v14(
        semantic: &Value,
        head: &LocalSnapshotHead,
        documentation_manifest: &Value,
        sha256: R12Sha256,
    ) -> Result<Self, R12ContractError> {
        validate_stored_snapshot_semantic_v14(semantic, head)
            .map_err(|_| R12ContractError::InvalidSnapshot)?;
        if semantic.get("ontology_version").and_then(Value::as_str) != Some(R12_ONTOLOGY_VERSION) {
            return Err(R12ContractError::InvalidSnapshot);
        }
        validate_documentation_binding(documentation_manifest, head)?;
        let graph = semantic
            .get("knowledge_graph")
            .and_then(Value::as_object)
            .ok_or(R12ContractError::InvalidSnapshot)?;
        let boundaries = semantic
            .get("repository_boundaries")
            .cloned()
            .ok_or(R12ContractError::InvalidSnapshot)?;
        validate_boundary_projection(&boundaries)?;
        let (documents, document_statements) = portable_documents(documentation_manifest)?;
        let mut value = json!({
            "schema_version": R12_PORTABLE_GRAPH_VERSION,
            "repository": semantic.get("repository").cloned().ok_or(R12ContractError::InvalidSnapshot)?,
            "source_snapshot": {
                "schema_version": R12_SNAPSHOT_VERSION,
                "snapshot_id": head.snapshot_id.as_str(),
                "semantic_hash": {
                    "algorithm": head.semantic_hash.algorithm,
                    "value": head.semantic_hash.value
                }
            },
            "ontology_version": R12_ONTOLOGY_VERSION,
            "query_contract_version": R12_QUERY_VERSION,
            "projection": {
                "profile": "codenoesis.lossless-portable-projection/v5",
                "family_sha256": {}
            },
            "repository_boundaries": boundaries,
            "entities": graph.get("entities").cloned().ok_or(R12ContractError::InvalidSnapshot)?,
            "relationships": graph.get("relationships").cloned().ok_or(R12ContractError::InvalidSnapshot)?,
            "claims": graph.get("claims").cloned().ok_or(R12ContractError::InvalidSnapshot)?,
            "evidence": graph.get("evidence").cloned().ok_or(R12ContractError::InvalidSnapshot)?,
            "diagnostics": graph.get("diagnostics").cloned().ok_or(R12ContractError::InvalidSnapshot)?,
            "coverage_gaps": graph.get("coverage").cloned().ok_or(R12ContractError::InvalidSnapshot)?,
            "documents": documents,
            "document_statements": document_statements
        });
        value["projection"]["family_sha256"] = family_digests(&value, sha256)?;
        Self::from_generated_value(value, sha256)
    }

    /// Strictly reimports one canonical LF-terminated `PortableGraphV5`.
    ///
    /// # Errors
    ///
    /// Returns the first decode, schema, identity, reference, privacy, or limit failure.
    pub fn from_canonical_file(bytes: &[u8], sha256: R12Sha256) -> Result<Self, R12ContractError> {
        enforce_portable_size(bytes.len())?;
        let value = serde_json::from_slice::<Value>(bytes)
            .map_err(|_| R12ContractError::InvalidProjection)?;
        validate_portable_value(&value, sha256)?;
        let canonical = serde_json::to_vec(&value).map_err(|_| R12ContractError::Internal)?;
        let mut expected = canonical.clone();
        expected.push(b'\n');
        if expected != bytes {
            return Err(R12ContractError::Noncanonical {
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

    fn from_generated_value(value: Value, sha256: R12Sha256) -> Result<Self, R12ContractError> {
        validate_portable_value(&value, sha256)?;
        let canonical = serde_json::to_vec(&value).map_err(|_| R12ContractError::Internal)?;
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
pub struct LocalExplorerManifestV5 {
    value: Value,
}

impl LocalExplorerManifestV5 {
    /// Builds the offline V5 explorer manifest over immutable K1 viewer bytes.
    ///
    /// # Errors
    ///
    /// Returns an integrity or unsafe-CSP failure.
    pub fn new(
        portable: &PortableGraphV5,
        viewer_bytes: &[u8],
        expected_viewer_sha256: &str,
        content_security_policy: &str,
        sha256: R12Sha256,
    ) -> Result<Self, R12ContractError> {
        if sha256(viewer_bytes) != expected_viewer_sha256
            || content_security_policy.contains("http:")
            || content_security_policy.contains("https:")
            || content_security_policy.contains("unsafe-inline")
            || content_security_policy.contains("unsafe-eval")
        {
            return Err(R12ContractError::AssetIntegrityMismatch);
        }
        Ok(Self {
            value: json!({
                "schema_version": R12_LOCAL_EXPLORER_VERSION,
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
                    "profile": R12_EXPLORER_SECURITY_PROFILE,
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
                    "repository_boundaries": true,
                    "cfg_declaration_alternatives": true,
                    "callable_occurrences": true,
                    "bounded_traversal": [1, 2]
                },
                "limits": {
                    "text_search_results": MAX_R12_TEXT_SEARCH_RESULTS,
                    "traversal_depth_default": R12_TRAVERSAL_DEPTH_DEFAULT,
                    "traversal_depth_maximum": MAX_R12_TRAVERSAL_DEPTH
                }
            }),
        })
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Serializes one `LocalExplorerManifestV5` followed by LF.
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
pub enum R12ContractError {
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

impl Display for R12ContractError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidSnapshot => "invalid R12 snapshot",
            Self::UnsupportedSnapshotSchema(_) => "unsupported R12 snapshot schema",
            Self::UnsupportedPortableGraphSchema(_) => "unsupported portable graph schema",
            Self::Noncanonical { .. } => "noncanonical portable graph",
            Self::IdentityConflict { .. } => "portable identity conflict",
            Self::ReferenceMismatch { .. } => "portable reference mismatch",
            Self::LimitExceeded { .. } => "portable graph limit exceeded",
            Self::UnsafePayload { .. } => "unsafe portable payload",
            Self::AssetIntegrityMismatch => "explorer asset integrity mismatch",
            Self::InvalidProjection => "invalid portable graph projection",
            Self::Internal => "internal R12 contract error",
        })
    }
}

impl Error for R12ContractError {}

fn linked_relationships(
    relationships: &[Value],
    requested_id: &str,
    kind_filter: impl Fn(&str) -> bool,
) -> Vec<Value> {
    let mut linked = relationships
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
                .is_some_and(&kind_filter)
        })
        .cloned()
        .collect::<Vec<_>>();
    linked.sort_by(|left, right| record_id(left).cmp(&record_id(right)));
    linked.dedup_by(|left, right| record_id(left) == record_id(right));
    linked
}

fn linked_entities(
    entities: &BTreeMap<&str, &Value>,
    requested_id: &str,
    relationships: &[Value],
) -> Vec<Value> {
    let mut linked = relationships
        .iter()
        .flat_map(|relationship| {
            [
                relationship.get("source").and_then(Value::as_str),
                relationship.get("target").and_then(Value::as_str),
            ]
        })
        .flatten()
        .filter(|identifier| *identifier != requested_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter_map(|identifier| entities.get(identifier).copied().cloned())
        .collect::<Vec<_>>();
    linked.sort_by(|left, right| record_id(left).cmp(&record_id(right)));
    linked
}

fn union_linked_claims_and_evidence(
    value: &mut Value,
    graph: &Map<String, Value>,
    subject_ids: &BTreeSet<&str>,
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

fn is_k1_kind(kind: &str) -> bool {
    matches!(
        kind,
        "HAS_SIGNATURE" | "HAS_PARAMETER" | "DECLARES_VALUE" | "HAS_BODY_FACT" | "CALLS"
    )
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

fn validate_documentation_binding(
    manifest: &Value,
    head: &LocalSnapshotHead,
) -> Result<(), R12ContractError> {
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
        return Err(R12ContractError::InvalidSnapshot);
    }
    Ok(())
}

fn portable_documents(manifest: &Value) -> Result<(Vec<Value>, Vec<Value>), R12ContractError> {
    let source = manifest
        .get("documents")
        .and_then(Value::as_array)
        .ok_or(R12ContractError::InvalidSnapshot)?;
    let mut documents = Vec::with_capacity(source.len());
    let mut statements = Vec::new();
    for document in source {
        let mut record = document
            .as_object()
            .cloned()
            .ok_or(R12ContractError::InvalidSnapshot)?;
        let document_id = record
            .get("document_id")
            .and_then(Value::as_str)
            .ok_or(R12ContractError::InvalidSnapshot)?
            .to_owned();
        let document_statements = record
            .remove("statements")
            .and_then(|value| value.as_array().cloned())
            .ok_or(R12ContractError::InvalidSnapshot)?;
        documents.push(Value::Object(record));
        for statement in document_statements {
            let mut statement = statement
                .as_object()
                .cloned()
                .ok_or(R12ContractError::InvalidSnapshot)?;
            if statement
                .insert("document_id".to_owned(), Value::String(document_id.clone()))
                .is_some()
            {
                return Err(R12ContractError::InvalidSnapshot);
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

fn family_digests(value: &Value, sha256: R12Sha256) -> Result<Value, R12ContractError> {
    let mut digests = Map::new();
    for (family, _) in PORTABLE_FAMILIES {
        let bytes = serde_json::to_vec(
            value
                .get(family)
                .ok_or(R12ContractError::InvalidProjection)?,
        )
        .map_err(|_| R12ContractError::Internal)?;
        digests.insert(family.to_owned(), Value::String(sha256(&bytes)));
    }
    Ok(Value::Object(digests))
}

#[allow(clippy::too_many_lines)]
fn validate_portable_value(value: &Value, sha256: R12Sha256) -> Result<(), R12ContractError> {
    ensure_nesting(value, 0)?;
    validate_private_fields(value)?;
    let object = value
        .as_object()
        .ok_or(R12ContractError::InvalidProjection)?;
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
        "repository_boundaries",
        "schema_version",
        "source_snapshot",
    ]);
    if object.keys().map(String::as_str).collect::<BTreeSet<_>>() != expected_keys {
        return Err(R12ContractError::InvalidProjection);
    }
    let schema = object
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or(R12ContractError::InvalidProjection)?;
    if schema != R12_PORTABLE_GRAPH_VERSION {
        return Err(R12ContractError::UnsupportedPortableGraphSchema(bounded(
            schema, 256,
        )));
    }
    if object.get("ontology_version").and_then(Value::as_str) != Some(R12_ONTOLOGY_VERSION)
        || object.get("query_contract_version").and_then(Value::as_str) != Some(R12_QUERY_VERSION)
        || value
            .pointer("/source_snapshot/schema_version")
            .and_then(Value::as_str)
            != Some(R12_SNAPSHOT_VERSION)
        || value.pointer("/projection/profile").and_then(Value::as_str)
            != Some("codenoesis.lossless-portable-projection/v5")
    {
        return Err(R12ContractError::InvalidProjection);
    }
    validate_boundary_projection(&object["repository_boundaries"])?;
    let repository_identity = value
        .pointer("/repository/identity")
        .and_then(Value::as_str)
        .filter(|value| value.starts_with("urn:codenoesis:"))
        .ok_or(R12ContractError::InvalidProjection)?;
    let entity_ids = validate_family(object, "entities", "id")?;
    let relationship_ids = validate_family(object, "relationships", "id")?;
    let claim_ids = validate_family(object, "claims", "id")?;
    let evidence_ids = validate_family(object, "evidence", "id")?;
    let diagnostic_ids = validate_family(object, "diagnostics", "id")?;
    let coverage_ids = validate_family(object, "coverage_gaps", "id")?;
    let document_ids = validate_family(object, "documents", "document_id")?;
    let statement_ids = validate_family(object, "document_statements", "statement_id")?;
    let boundary_ids = boundary_document_reference_ids(&object["repository_boundaries"])?;
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
        .ok_or(R12ContractError::InvalidProjection)?
    {
        for field in ["source", "target"] {
            validate_reference(relationship, field, &entity_ids, "relationships")?;
        }
        validate_evidence_references(relationship, &evidence_ids, "relationships")?;
    }
    for claim in object["claims"]
        .as_array()
        .ok_or(R12ContractError::InvalidProjection)?
    {
        let subject_id = claim
            .get("subject_id")
            .and_then(Value::as_str)
            .ok_or(R12ContractError::InvalidProjection)?;
        let subject_kind = claim
            .get("subject_kind")
            .and_then(Value::as_str)
            .ok_or(R12ContractError::InvalidProjection)?;
        let valid = match subject_kind {
            "entity" => entity_ids.contains(subject_id),
            "relationship" => relationship_ids.contains(subject_id),
            _ => false,
        };
        if !valid {
            return Err(R12ContractError::ReferenceMismatch {
                family: "claims",
                id: subject_id.to_owned(),
            });
        }
        validate_evidence_references(claim, &evidence_ids, "claims")?;
    }
    for entity in object["entities"]
        .as_array()
        .ok_or(R12ContractError::InvalidProjection)?
    {
        validate_evidence_references(entity, &evidence_ids, "entities")?;
        validate_entity_references(entity, &entity_ids, &evidence_ids)?;
    }
    for evidence in object["evidence"]
        .as_array()
        .ok_or(R12ContractError::InvalidProjection)?
    {
        let blob_oid = evidence
            .get("blob_oid")
            .and_then(Value::as_str)
            .ok_or(R12ContractError::InvalidProjection)?;
        let start_byte = evidence
            .get("start_byte")
            .and_then(Value::as_u64)
            .ok_or(R12ContractError::InvalidProjection)?;
        let end_byte = evidence
            .get("end_byte")
            .and_then(Value::as_u64)
            .ok_or(R12ContractError::InvalidProjection)?;
        if blob_oid.len() != 40
            || !blob_oid
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || start_byte >= end_byte
        {
            return Err(R12ContractError::InvalidProjection);
        }
    }
    for (family, ids) in [
        ("diagnostics", &diagnostic_ids),
        ("coverage_gaps", &coverage_ids),
    ] {
        for record in object[family]
            .as_array()
            .ok_or(R12ContractError::InvalidProjection)?
        {
            let identifier = record_id(record).ok_or(R12ContractError::InvalidProjection)?;
            if !ids.contains(identifier) {
                return Err(R12ContractError::InvalidProjection);
            }
            validate_evidence_references(record, &evidence_ids, family)?;
            if let Some(subject) = record
                .get("subject_id")
                .or_else(|| record.get("declaration_id"))
                .and_then(Value::as_str)
                && !entity_ids.contains(subject)
            {
                return Err(R12ContractError::ReferenceMismatch {
                    family,
                    id: subject.to_owned(),
                });
            }
        }
    }
    for document in object["documents"]
        .as_array()
        .ok_or(R12ContractError::InvalidProjection)?
    {
        validate_reference(document, "subject_id", &subject_ids, "documents")?;
    }
    for statement in object["document_statements"]
        .as_array()
        .ok_or(R12ContractError::InvalidProjection)?
    {
        let document_id = statement
            .get("document_id")
            .and_then(Value::as_str)
            .ok_or(R12ContractError::InvalidProjection)?;
        if !document_ids.contains(document_id) {
            return Err(R12ContractError::ReferenceMismatch {
                family: "document_statements",
                id: document_id.to_owned(),
            });
        }
        validate_array_subset(
            statement,
            "subject_ids",
            &subject_ids,
            "document_statements",
        )?;
        validate_array_subset(
            statement,
            "evidence_ids",
            &statement_evidence_ids,
            "document_statements",
        )?;
        validate_array_subset(
            statement,
            "coverage_gap_ids",
            &statement_coverage_ids,
            "document_statements",
        )?;
    }
    validate_portable_paths(value)?;
    if repository_identity.is_empty() {
        return Err(R12ContractError::InvalidProjection);
    }
    let observed_digests = value
        .pointer("/projection/family_sha256")
        .ok_or(R12ContractError::InvalidProjection)?;
    let expected_digests = family_digests(value, sha256)?;
    if observed_digests != &expected_digests {
        return Err(R12ContractError::InvalidProjection);
    }
    Ok(())
}

#[derive(Default)]
struct BoundaryDocumentReferenceIds {
    subjects: BTreeSet<String>,
    evidence: BTreeSet<String>,
    coverage: BTreeSet<String>,
}

fn boundary_document_reference_ids(
    value: &Value,
) -> Result<BoundaryDocumentReferenceIds, R12ContractError> {
    if value.is_null() {
        return Ok(BoundaryDocumentReferenceIds::default());
    }
    let object = value
        .as_object()
        .ok_or(R12ContractError::InvalidProjection)?;
    let mut identifiers = BoundaryDocumentReferenceIds::default();
    for (family, field) in [
        ("boundaries", "boundary_id"),
        ("declarations", "declaration_id"),
        ("evidence", "evidence_id"),
        ("coverage_gaps", "gap_id"),
    ] {
        for record in object
            .get(family)
            .and_then(Value::as_array)
            .ok_or(R12ContractError::InvalidProjection)?
        {
            let identifier = record
                .get(field)
                .and_then(Value::as_str)
                .ok_or(R12ContractError::InvalidProjection)?
                .to_owned();
            identifiers.subjects.insert(identifier.clone());
            match family {
                "evidence" => {
                    identifiers.evidence.insert(identifier);
                }
                "coverage_gaps" => {
                    identifiers.coverage.insert(identifier);
                }
                _ => {}
            }
        }
    }
    Ok(identifiers)
}

fn validate_family(
    object: &Map<String, Value>,
    family: &'static str,
    id_field: &'static str,
) -> Result<BTreeSet<String>, R12ContractError> {
    let values = object
        .get(family)
        .and_then(Value::as_array)
        .ok_or(R12ContractError::InvalidProjection)?;
    let mut ids = BTreeSet::new();
    let mut previous = None;
    for value in values {
        let identifier = value
            .get(id_field)
            .and_then(Value::as_str)
            .filter(|identifier| !identifier.is_empty())
            .ok_or(R12ContractError::InvalidProjection)?;
        if previous.is_some_and(|previous| previous >= identifier) {
            return Err(if ids.contains(identifier) {
                R12ContractError::IdentityConflict {
                    family,
                    id: identifier.to_owned(),
                }
            } else {
                R12ContractError::InvalidProjection
            });
        }
        ids.insert(identifier.to_owned());
        previous = Some(identifier);
    }
    Ok(ids)
}

fn validate_reference(
    record: &Value,
    field: &'static str,
    ids: &BTreeSet<String>,
    family: &'static str,
) -> Result<(), R12ContractError> {
    let identifier = record
        .get(field)
        .and_then(Value::as_str)
        .ok_or(R12ContractError::InvalidProjection)?;
    if !ids.contains(identifier) {
        return Err(R12ContractError::ReferenceMismatch {
            family,
            id: identifier.to_owned(),
        });
    }
    Ok(())
}

fn validate_evidence_references(
    record: &Value,
    evidence_ids: &BTreeSet<String>,
    family: &'static str,
) -> Result<(), R12ContractError> {
    if record.get("evidence_ids").is_none() {
        return Ok(());
    }
    validate_array_subset(record, "evidence_ids", evidence_ids, family)
}

fn validate_entity_references(
    entity: &Value,
    entity_ids: &BTreeSet<String>,
    evidence_ids: &BTreeSet<String>,
) -> Result<(), R12ContractError> {
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
        .ok_or(R12ContractError::InvalidProjection)?;
    for field in [
        "manifest_id",
        "materialized_crate_id",
        "owner_id",
        "package_id",
        "parent_fact_id",
        "resolved_target_id",
        "source_entity_id",
        "source_file_id",
        "trait_context_id",
        "workspace_reference_id",
    ] {
        validate_optional_reference(properties.get(field), entity_ids, "entities")?;
    }
    for field in ["body_evidence_id", "declaration_evidence_id", "evidence_id"] {
        validate_optional_reference(properties.get(field), evidence_ids, "entities")?;
    }
    if let Some(alternatives) = properties.get("declaration_alternative_ids") {
        validate_reference_array(alternatives, entity_ids, "entities")?;
    }
    if let Some(attributes) = properties.get("attributes") {
        for attribute in attributes
            .as_array()
            .ok_or(R12ContractError::InvalidProjection)?
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
) -> Result<(), R12ContractError> {
    let Some(value) = value else {
        return Ok(());
    };
    if value.is_null() {
        return Ok(());
    }
    let identifier = value.as_str().ok_or(R12ContractError::InvalidProjection)?;
    if !identifiers.contains(identifier) {
        return Err(R12ContractError::ReferenceMismatch {
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
) -> Result<(), R12ContractError> {
    let mut previous = None;
    for value in value
        .as_array()
        .ok_or(R12ContractError::InvalidProjection)?
    {
        let identifier = value.as_str().ok_or(R12ContractError::InvalidProjection)?;
        if previous.is_some_and(|previous: &str| previous >= identifier)
            || !identifiers.contains(identifier)
        {
            return Err(R12ContractError::ReferenceMismatch {
                family,
                id: identifier.to_owned(),
            });
        }
        previous = Some(identifier);
    }
    Ok(())
}

fn validate_array_subset(
    record: &Value,
    field: &'static str,
    ids: &BTreeSet<String>,
    family: &'static str,
) -> Result<(), R12ContractError> {
    let Some(values) = record.get(field) else {
        return Ok(());
    };
    let values = values
        .as_array()
        .ok_or(R12ContractError::InvalidProjection)?;
    for identifier in values {
        let identifier = identifier
            .as_str()
            .ok_or(R12ContractError::InvalidProjection)?;
        if !ids.contains(identifier) {
            return Err(R12ContractError::ReferenceMismatch {
                family,
                id: identifier.to_owned(),
            });
        }
    }
    Ok(())
}

fn validate_boundary_projection(value: &Value) -> Result<(), R12ContractError> {
    if value.is_null() {
        return Ok(());
    }
    let object = value
        .as_object()
        .ok_or(R12ContractError::InvalidProjection)?;
    if object.get("schema_version").and_then(Value::as_str)
        != Some("codenoesis.repository-boundaries/v1")
        || object.get("profile").and_then(Value::as_str) != Some("local-gitlinks-v1")
    {
        return Err(R12ContractError::InvalidProjection);
    }
    for family in ["boundaries", "declarations", "coverage_gaps", "evidence"] {
        let records = object
            .get(family)
            .and_then(Value::as_array)
            .ok_or(R12ContractError::InvalidProjection)?;
        for record in records {
            if let Some(path) = record.get("path").and_then(Value::as_str)
                && !safe_relative_path(path)
            {
                return Err(R12ContractError::UnsafePayload {
                    reason: "unsafe_boundary_path",
                });
            }
        }
    }
    Ok(())
}

fn validate_portable_paths(value: &Value) -> Result<(), R12ContractError> {
    for evidence in value["evidence"]
        .as_array()
        .ok_or(R12ContractError::InvalidProjection)?
    {
        let path = evidence
            .get("path")
            .and_then(Value::as_str)
            .ok_or(R12ContractError::InvalidProjection)?;
        if !safe_relative_path(path) {
            return Err(R12ContractError::UnsafePayload {
                reason: "unsafe_evidence_path",
            });
        }
    }
    for document in value["documents"]
        .as_array()
        .ok_or(R12ContractError::InvalidProjection)?
    {
        if let Some(path) = document.get("path").and_then(Value::as_str)
            && !safe_relative_path(path)
        {
            return Err(R12ContractError::UnsafePayload {
                reason: "unsafe_document_path",
            });
        }
    }
    Ok(())
}

fn validate_private_fields(value: &Value) -> Result<(), R12ContractError> {
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
                        | "telemetry"
                ) {
                    return Err(R12ContractError::UnsafePayload {
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
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    Ok(())
}

fn ensure_nesting(value: &Value, depth: u64) -> Result<(), R12ContractError> {
    if depth > MAX_R12_JSON_NESTING {
        return Err(R12ContractError::LimitExceeded {
            limit: "json_nesting",
            maximum: MAX_R12_JSON_NESTING,
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

fn enforce_portable_size(length: usize) -> Result<(), R12ContractError> {
    let observed = u64::try_from(length).unwrap_or(u64::MAX);
    if observed > MAX_R12_PORTABLE_GRAPH_BYTES {
        return Err(R12ContractError::LimitExceeded {
            limit: "portable_graph_bytes",
            maximum: MAX_R12_PORTABLE_GRAPH_BYTES,
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

fn extraction_chunks_v11(
    baseline: &Value,
    knowledge: &CallableCfgAlternativesKnowledge,
) -> Result<Vec<Value>, RepositorySnapshotV14Error> {
    let alternatives = knowledge
        .alternatives
        .extraction_chunks
        .iter()
        .map(|chunk| (chunk.source_file_id.as_str(), chunk))
        .collect::<BTreeMap<_, _>>();
    let callables = knowledge
        .callable
        .extraction_chunks
        .iter()
        .map(|chunk| (chunk.source_file_id.as_str(), chunk))
        .collect::<BTreeMap<_, _>>();
    if alternatives.len() != callables.len() {
        return Err(RepositorySnapshotV14Error::ContractInvalid);
    }
    let mut transformed = Vec::with_capacity(alternatives.len());
    for chunk in baseline
        .as_array()
        .ok_or(RepositorySnapshotV14Error::ContractInvalid)?
    {
        if chunk.pointer("/subject/kind").and_then(Value::as_str) != Some("rust_source") {
            continue;
        }
        let source_file_id = chunk
            .pointer("/subject/source_file_id")
            .and_then(Value::as_str)
            .ok_or(RepositorySnapshotV14Error::ContractInvalid)?;
        let alternative = alternatives
            .get(source_file_id)
            .ok_or(RepositorySnapshotV14Error::ContractInvalid)?;
        let callable = callables
            .get(source_file_id)
            .ok_or(RepositorySnapshotV14Error::ContractInvalid)?;
        let mut value = json!({
            "schema_version": R12_EXTRACTION_CHUNK_VERSION,
            "source_file_id": source_file_id,
            "entities": required_family(chunk, "entities")?,
            "relationships": required_family(chunk, "relationships")?,
            "claims": required_family(chunk, "claims")?,
            "evidence": required_family(chunk, "evidence")?,
            "diagnostics": required_family(chunk, "diagnostics")?,
            "coverage": required_family(chunk, "coverage")?,
            "declaration_alternatives_profile": R10_PROFILE,
            "callable_semantics_profile": K1_PROFILE,
            "callable_cfg_alternatives_profile": R12_EXTRACTOR_VERSION
        });
        apply_r10_additions(&mut value, alternative)?;
        apply_callable_additions(&mut value, callable)?;
        canonicalize_evidence_references(&mut value)?;
        insert_semantic_hash(&mut value, EXTRACTION_V11_HASH_DOMAIN)?;
        transformed.push(value);
    }
    transformed.sort_by(|left, right| {
        left.get("source_file_id")
            .and_then(Value::as_str)
            .cmp(&right.get("source_file_id").and_then(Value::as_str))
    });
    if transformed.len() != alternatives.len() {
        return Err(RepositorySnapshotV14Error::ContractInvalid);
    }
    Ok(transformed)
}

fn knowledge_graph_v11(
    baseline: &Value,
    knowledge: &CallableCfgAlternativesKnowledge,
    has_boundaries: bool,
) -> Result<Value, RepositorySnapshotV14Error> {
    let mut value = baseline.clone();
    let object = value
        .as_object_mut()
        .ok_or(RepositorySnapshotV14Error::ContractInvalid)?;
    object.insert(
        "schema_version".to_owned(),
        Value::String(R12_GRAPH_VERSION.to_owned()),
    );
    object.insert(
        "ontology_version".to_owned(),
        Value::String(R12_ONTOLOGY_VERSION.to_owned()),
    );
    object.insert(
        "extractor_versions".to_owned(),
        Value::Array(extractor_versions(has_boundaries, false)),
    );
    object.insert(
        "declaration_alternative_index".to_owned(),
        json!({
            "schema_version": R10_INDEX_VERSION,
            "profile": R10_PROFILE,
            "logical_method_ids": knowledge.alternatives.graph.index.logical_method_ids,
            "alternative_entity_ids": knowledge.alternatives.graph.index.alternative_entity_ids,
            "alternative_relationship_ids": knowledge.alternatives.graph.index.alternative_relationship_ids
        }),
    );
    object.insert(
        "callable_semantics_index".to_owned(),
        json!({
            "schema_version": K1_INDEX_VERSION,
            "profile": K1_PROFILE,
            "signature_ids": knowledge.callable.graph.index.signature_ids,
            "parameter_ids": knowledge.callable.graph.index.parameter_ids,
            "declared_value_ids": knowledge.callable.graph.index.declared_value_ids,
            "body_fact_ids": knowledge.callable.graph.index.body_fact_ids,
            "resolved_call_relationship_ids": knowledge.callable.graph.index.resolved_call_relationship_ids,
            "unresolved_call_site_ids": knowledge.callable.graph.index.unresolved_call_site_ids
        }),
    );
    object.insert(
        "callable_cfg_alternatives_index".to_owned(),
        json!({
            "schema_version": R12_INDEX_VERSION,
            "logical_method_ids": knowledge.index.logical_method_ids,
            "alternative_callable_subject_ids": knowledge.index.alternative_callable_subject_ids,
            "signature_ids": knowledge.index.signature_ids
        }),
    );
    object.remove("semantic_hash");
    let aggregate = RustCfgDeclarationAlternativesSourceChunk {
        source_file_id: String::new(),
        logical_method_ids: knowledge
            .alternatives
            .graph
            .index
            .logical_method_ids
            .clone(),
        alternatives: knowledge.alternatives.graph.alternatives.clone(),
        relationships: knowledge.alternatives.graph.relationships.clone(),
        claims: knowledge.alternatives.graph.claims.clone(),
    };
    apply_r10_additions(&mut value, &aggregate)?;
    apply_callable_graph_additions(&mut value, knowledge)?;
    canonicalize_evidence_references(&mut value)?;
    insert_semantic_hash(&mut value, GRAPH_V11_HASH_DOMAIN)?;
    Ok(value)
}

fn apply_r10_additions(
    value: &mut Value,
    additions: &RustCfgDeclarationAlternativesSourceChunk,
) -> Result<(), RepositorySnapshotV14Error> {
    let object = value
        .as_object_mut()
        .ok_or(RepositorySnapshotV14Error::ContractInvalid)?;
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
    merge_id_array(object, "claims", additions.claims.iter().map(claim_value))
}

fn project_logical_method(
    object: &mut Map<String, Value>,
    logical_method_id: &str,
    alternatives: &[&RustDeclarationAlternative],
) -> Result<(), RepositorySnapshotV14Error> {
    if alternatives.len() < 2 {
        return Err(RepositorySnapshotV14Error::ContractInvalid);
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
        return Err(RepositorySnapshotV14Error::ContractInvalid);
    }
    let logical = find_record_mut(family_mut(object, "entities")?, logical_method_id)?;
    let owner_id = logical
        .get("owner_id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or(RepositorySnapshotV14Error::ContractInvalid)?
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
        return Err(RepositorySnapshotV14Error::ContractInvalid);
    }
    defines[0]["evidence_ids"] = json!(declaration_evidence_ids);
    let relationship_id = defines[0]
        .get("id")
        .and_then(Value::as_str)
        .ok_or(RepositorySnapshotV14Error::ContractInvalid)?
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
) -> Result<(), RepositorySnapshotV14Error> {
    let mut matching = family_mut(object, "claims")?
        .iter_mut()
        .filter(|claim| {
            claim.get("subject_kind").and_then(Value::as_str) == Some(subject_kind)
                && claim.get("subject_id").and_then(Value::as_str) == Some(subject_id)
        })
        .collect::<Vec<_>>();
    if matching.len() != 1 {
        return Err(RepositorySnapshotV14Error::ContractInvalid);
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

fn apply_callable_additions(
    value: &mut Value,
    callable: &CallableSourceChunk,
) -> Result<(), RepositorySnapshotV14Error> {
    let object = value
        .as_object_mut()
        .ok_or(RepositorySnapshotV14Error::ContractInvalid)?;
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
    )
}

fn apply_callable_graph_additions(
    value: &mut Value,
    knowledge: &CallableCfgAlternativesKnowledge,
) -> Result<(), RepositorySnapshotV14Error> {
    let object = value
        .as_object_mut()
        .ok_or(RepositorySnapshotV14Error::ContractInvalid)?;
    merge_id_array(
        object,
        "entities",
        knowledge
            .callable
            .graph
            .entities
            .iter()
            .map(callable_entity_value),
    )?;
    merge_id_array(
        object,
        "relationships",
        knowledge
            .callable
            .graph
            .relationships
            .iter()
            .map(callable_relationship_value),
    )?;
    merge_id_array(
        object,
        "claims",
        knowledge.callable.graph.claims.iter().map(claim_value),
    )?;
    merge_id_array(
        object,
        "evidence",
        knowledge.callable.graph.evidence.iter().map(evidence_value),
    )?;
    merge_id_array(
        object,
        "diagnostics",
        knowledge
            .callable
            .graph
            .diagnostics
            .iter()
            .map(callable_diagnostic_value),
    )?;
    merge_id_array(
        object,
        "coverage",
        knowledge
            .callable
            .graph
            .coverage
            .iter()
            .map(callable_coverage_value),
    )
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
        NormalizedScalarValue::Boolean(value) => json!({"kind": "boolean", "value": value}),
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
        NormalizedScalarValue::Character(value) => {
            json!({"kind": "character", "value": value})
        }
        NormalizedScalarValue::String(value) => json!({"kind": "string", "value": value}),
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

fn extractor_versions(has_boundaries: bool, include_inventory: bool) -> Vec<Value> {
    let mut versions = Vec::new();
    if include_inventory {
        versions.push(Value::String(
            "codenoesis.inventory-classifier/s1-v1".to_owned(),
        ));
    }
    versions.extend([
        Value::String("codenoesis.rust-tree-sitter/s4-v1".to_owned()),
        Value::String("codenoesis.rust-workspace/s4-r3-v1".to_owned()),
        Value::String("codenoesis.cargo-manifest/s4-r4-v1".to_owned()),
        Value::String(R5_RUST_SEMANTIC_EXTRACTOR_VERSION.to_owned()),
        Value::String(R6_FRAMEWORK_EXTRACTOR_VERSION.to_owned()),
        Value::String(R10_EXTRACTOR_VERSION.to_owned()),
        Value::String(K1_EXTRACTOR_VERSION.to_owned()),
        Value::String(R12_EXTRACTOR_VERSION.to_owned()),
        Value::String(R12_COMPOSITION_VERSION.to_owned()),
    ]);
    if has_boundaries {
        versions.push(Value::String("codenoesis.git-boundary/s1-v1".to_owned()));
    }
    versions
}

fn merge_id_array(
    object: &mut Map<String, Value>,
    field: &'static str,
    additions: impl IntoIterator<Item = Value>,
) -> Result<(), RepositorySnapshotV14Error> {
    let values = family_mut(object, field)?;
    values.extend(additions);
    values.sort_by(|left, right| record_id(left).cmp(&record_id(right)));
    let mut retained = Vec::with_capacity(values.len());
    for value in values.drain(..) {
        let identifier = record_id(&value).ok_or(RepositorySnapshotV14Error::ContractInvalid)?;
        if let Some(previous) = retained.last()
            && record_id(previous) == Some(identifier)
        {
            if previous != &value {
                return Err(RepositorySnapshotV14Error::ContractInvalid);
            }
            continue;
        }
        retained.push(value);
    }
    *values = retained;
    Ok(())
}

fn canonicalize_evidence_references(value: &mut Value) -> Result<(), RepositorySnapshotV14Error> {
    let object = value
        .as_object_mut()
        .ok_or(RepositorySnapshotV14Error::ContractInvalid)?;
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
                .ok_or(RepositorySnapshotV14Error::ContractInvalid)?;
            if references
                .iter()
                .any(|reference| reference.as_str().is_none())
            {
                return Err(RepositorySnapshotV14Error::ContractInvalid);
            }
            references.sort_by(|left, right| left.as_str().cmp(&right.as_str()));
            if !strictly_ordered(references.iter().filter_map(Value::as_str)) {
                return Err(RepositorySnapshotV14Error::ContractInvalid);
            }
        }
    }
    Ok(())
}

fn required_family(
    value: &Value,
    field: &'static str,
) -> Result<Value, RepositorySnapshotV14Error> {
    value
        .get(field)
        .filter(|value| value.is_array())
        .cloned()
        .ok_or(RepositorySnapshotV14Error::ContractInvalid)
}

fn family_mut<'a>(
    object: &'a mut Map<String, Value>,
    field: &'static str,
) -> Result<&'a mut Vec<Value>, RepositorySnapshotV14Error> {
    object
        .get_mut(field)
        .and_then(Value::as_array_mut)
        .ok_or(RepositorySnapshotV14Error::ContractInvalid)
}

fn find_record_mut<'a>(
    values: &'a mut [Value],
    identifier: &str,
) -> Result<&'a mut Value, RepositorySnapshotV14Error> {
    let mut matching = values
        .iter_mut()
        .filter(|value| record_id(value) == Some(identifier));
    let value = matching
        .next()
        .ok_or(RepositorySnapshotV14Error::ContractInvalid)?;
    if matching.next().is_some() {
        return Err(RepositorySnapshotV14Error::ContractInvalid);
    }
    Ok(value)
}

fn insert_semantic_hash(
    value: &mut Value,
    domain: &[u8],
) -> Result<(), RepositorySnapshotV14Error> {
    let hash = semantic_hash(domain, value);
    value
        .as_object_mut()
        .ok_or(RepositorySnapshotV14Error::ContractInvalid)?
        .insert(
            "semantic_hash".to_owned(),
            json!({"algorithm": "blake3-256", "value": hash}),
        );
    Ok(())
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

fn map_v9_error(error: RepositorySnapshotV9Error) -> RepositorySnapshotV14Error {
    match error {
        RepositorySnapshotV9Error::Serialization(error) => {
            RepositorySnapshotV14Error::Serialization(error)
        }
        RepositorySnapshotV9Error::LimitExceeded(error) => {
            RepositorySnapshotV14Error::LimitExceeded(error)
        }
        RepositorySnapshotV9Error::ContractInvalid => RepositorySnapshotV14Error::ContractInvalid,
        RepositorySnapshotV9Error::OutputLengthOverflow => {
            RepositorySnapshotV14Error::OutputLengthOverflow
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
