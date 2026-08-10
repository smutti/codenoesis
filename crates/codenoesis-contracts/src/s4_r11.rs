use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::path::{Component, Path};

use codenoesis_domain::s1_boundaries::{
    LOCAL_GITLINKS_V1, RepositoryBoundaryError, RepositoryBoundaryReport,
};
use codenoesis_domain::s4_k1::{
    CallableSemanticsError, CallableSemanticsKnowledge, K1_EXTRACTOR_VERSION, K1_GRAPH_VERSION,
    K1_ONTOLOGY_VERSION, K1_PROFILE, K1_QUERY_VERSION,
};
use codenoesis_domain::s4_r5::RustSemanticError;
pub use codenoesis_domain::s4_r11::{
    R11_COMPOSITION_VERSION, R11_CONFIGURATION_VERSION, R11_ERROR_VERSION,
    R11_EXTRACTION_CHUNK_VERSION, R11_EXTRACTION_CONTRACT_VERSION, R11_GRAPH_VERSION,
    R11_LOCAL_EXPLORER_VERSION, R11_ONTOLOGY_VERSION, R11_PIPELINE_VERSION,
    R11_PORTABLE_GRAPH_VERSION, R11_QUERY_VERSION, R11_SNAPSHOT_VERSION,
};
use codenoesis_domain::storage::{
    LocalSnapshotHead, PublicationCandidate, SNAPSHOT_SCHEMA_VERSION_V13, StorageComponent,
    StorageError,
};
use codenoesis_domain::{
    AcquisitionError, K1OutputCapacityProfile, LimitKind, RepositoryInventory,
};
use serde_json::{Map, Value, json};

use super::s4::{MAX_QUERY_BYTES, QueryContractError};
use super::s4_k1::{
    CodeNoesisErrorV16, K1ContractError, PortableGraphV2, RepositorySnapshotV11,
    RepositorySnapshotV11Error, local_query_result_v6,
};
use super::s4_r5::CodeNoesisErrorV12;
use super::{
    CodeNoesisErrorV9, LimitedVecWriter, PublicationCandidateError, SnapshotEnvelopeV1,
    publication_candidate, repository_boundary_value, semantic_hash,
};

const CONFIGURATION_V10_HASH_DOMAIN: &[u8] = b"codenoesis.configuration.semantic.v10";
const SNAPSHOT_V13_HASH_DOMAIN: &[u8] = b"codenoesis.repository-snapshot.semantic.v13";
const EXTRACTION_V10_HASH_DOMAIN: &[u8] = b"codenoesis.extraction-chunk.semantic.v10";
const GRAPH_V10_HASH_DOMAIN: &[u8] = b"codenoesis.knowledge-graph.semantic.v10";
const PORTABLE_FAMILIES: [&str; 8] = [
    "entities",
    "relationships",
    "claims",
    "evidence",
    "diagnostics",
    "coverage_gaps",
    "documents",
    "document_statements",
];

pub type R11Sha256 = fn(&[u8]) -> String;
pub const R11_PORTABLE_MARKER: &str = ".codenoesis-portable-graph-v4";
pub const R11_EXPLORER_MARKER: &str = ".codenoesis-local-explorer-v4";
pub const R11_EXPLORER_SECURITY_PROFILE: &str = "codenoesis.local-explorer-security/v4";
pub const MAX_R11_PORTABLE_GRAPH_BYTES: u64 = 268_435_456;
pub const MAX_R11_JSON_NESTING: u64 = 64;
pub const MAX_R11_TEXT_SEARCH_RESULTS: u64 = 100;
pub const R11_TRAVERSAL_DEPTH_DEFAULT: u64 = 1;
pub const MAX_R11_TRAVERSAL_DEPTH: u64 = 2;

#[derive(Clone, Debug)]
pub struct CodeNoesisErrorV18 {
    value: Value,
}

impl CodeNoesisErrorV18 {
    #[must_use]
    pub fn invalid_rust_callable_profile(profile: &str) -> Self {
        Self::upgrade(CodeNoesisErrorV16::invalid_rust_callable_profile(profile).value())
    }

    #[must_use]
    pub fn unsupported_composition() -> Self {
        let mut error = Self::upgrade(CodeNoesisErrorV16::unsupported_composition().value());
        error.value["context"] = json!({
            "profile": K1_PROFILE,
            "required_lineage": "r6_with_local_gitlinks_v1",
            "compiler_index_composition": false,
            "cfg_alternatives_composition": false
        });
        error
    }

    #[must_use]
    pub fn from_callable(error: &CallableSemanticsError) -> Option<Self> {
        CodeNoesisErrorV16::from_callable(error).map(|value| Self::upgrade(value.value()))
    }

    #[must_use]
    pub fn from_rust_semantic_identity_conflict(error: &RustSemanticError) -> Option<Self> {
        if matches!(error, RustSemanticError::IdentityConflict { .. }) {
            CodeNoesisErrorV12::from_semantic(error)
                .map(|value| Self::upgrade_canonical(value.canonical_stderr()))
        } else {
            None
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
                &error.to_string(),
                false,
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
    pub fn from_boundary(error: &RepositoryBoundaryError) -> Self {
        Self::upgrade(CodeNoesisErrorV9::from_boundary(error).value())
    }

    #[must_use]
    pub fn from_boundary_error(error: &CodeNoesisErrorV9) -> Self {
        Self::upgrade(error.value())
    }

    #[must_use]
    pub fn invalid_snapshot() -> Self {
        Self::new(
            "snapshot.invalid_v13",
            "snapshot",
            "invalid R11 snapshot",
            false,
            &json!({}),
        )
    }

    #[must_use]
    pub fn invalid_query() -> Self {
        Self::new(
            "query.invalid_v13",
            "query",
            "invalid R11 query",
            false,
            &json!({}),
        )
    }

    #[must_use]
    pub fn unsafe_output_path(path_sha256: &str, reason: &str) -> Self {
        Self::new(
            "input.unsafe_output_path",
            "input",
            "unsafe output path",
            false,
            &json!({"path_sha256": bounded(path_sha256, 64), "reason": bounded(reason, 64)}),
        )
    }

    #[must_use]
    pub fn from_contract(error: &R11ContractError, explorer: bool) -> Self {
        match error {
            R11ContractError::UnsupportedSnapshotSchema(observed) => Self::new(
                "export.invalid_snapshot",
                "export",
                "invalid R11 source snapshot",
                false,
                &json!({"observed": bounded(observed, 256)}),
            ),
            R11ContractError::LimitExceeded {
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
                false,
                &json!({"limit": limit, "maximum": maximum, "observed": observed}),
            ),
            R11ContractError::AssetIntegrityMismatch => Self::new(
                "explorer.asset_integrity_mismatch",
                "explorer",
                "local explorer asset integrity mismatch",
                false,
                &json!({}),
            ),
            R11ContractError::Internal => Self::internal(),
            R11ContractError::InvalidSnapshot => Self::new(
                "export.invalid_snapshot",
                "export",
                "invalid R11 source snapshot",
                false,
                &json!({}),
            ),
            R11ContractError::UnsupportedPortableGraphSchema(observed) => {
                Self::invalid_portable(&json!({"observed": bounded(observed, 256)}))
            }
            R11ContractError::Noncanonical {
                expected_sha256,
                observed_sha256,
            } => Self::invalid_portable(&json!({
                "expected_sha256": expected_sha256,
                "observed_sha256": observed_sha256
            })),
            R11ContractError::IdentityConflict { family, id }
            | R11ContractError::ReferenceMismatch { family, id } => {
                Self::invalid_portable(&json!({"family": family, "id": bounded(id, 512)}))
            }
            R11ContractError::UnsafePayload { reason } => {
                Self::invalid_portable(&json!({"reason": reason}))
            }
            R11ContractError::InvalidProjection => Self::invalid_portable(&json!({})),
        }
    }

    #[must_use]
    pub fn internal() -> Self {
        Self::new(
            "internal.unexpected",
            "internal",
            "unexpected internal error",
            false,
            &json!({}),
        )
    }

    fn invalid_portable(context: &Value) -> Self {
        Self::new(
            "export.invalid_portable_graph_v4",
            "export",
            "invalid portable graph v4",
            false,
            context,
        )
    }

    fn upgrade(value: &Value) -> Self {
        let mut value = value.clone();
        value["schema_version"] = Value::String(R11_ERROR_VERSION.to_owned());
        Self { value }
    }

    fn upgrade_canonical(bytes: Result<Vec<u8>, serde_json::Error>) -> Self {
        bytes
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .map_or_else(Self::internal, |value| Self::upgrade(&value))
    }

    fn new(code: &str, stage: &str, message: &str, retryable: bool, context: &Value) -> Self {
        Self {
            value: json!({
                "schema_version": R11_ERROR_VERSION,
                "code": code,
                "stage": stage,
                "message": message,
                "retryable": retryable,
                "context": context
            }),
        }
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Serializes one bounded `ErrorV18` followed by LF.
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
pub struct RepositorySnapshotV13 {
    value: Value,
}

#[derive(Debug)]
pub enum RepositorySnapshotV13Error {
    Serialization(serde_json::Error),
    LimitExceeded(AcquisitionError),
    ContractInvalid,
    OutputLengthOverflow,
}

impl Display for RepositorySnapshotV13Error {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Serialization(_) => "R11 snapshot serialization failed",
            Self::LimitExceeded(_) => "R11 snapshot output limit exceeded",
            Self::ContractInvalid => "R11 snapshot contract is invalid",
            Self::OutputLengthOverflow => "R11 snapshot output length overflowed",
        })
    }
}

impl Error for RepositorySnapshotV13Error {}

impl RepositorySnapshotV13 {
    /// Builds V13 from the immutable K1 projection and exact R2 boundary report.
    ///
    /// # Errors
    ///
    /// Returns a source, boundary, serialization, publication, or output-bound failure.
    #[allow(clippy::too_many_lines)]
    pub fn from_inventory_callable_and_boundaries(
        inventory: &RepositoryInventory,
        knowledge: &CallableSemanticsKnowledge,
        boundaries: &RepositoryBoundaryReport,
        envelope: SnapshotEnvelopeV1,
    ) -> Result<Self, RepositorySnapshotV13Error> {
        knowledge
            .validate()
            .map_err(|_| RepositorySnapshotV13Error::ContractInvalid)?;
        super::validate_repository_boundary_report_size(boundaries)
            .map_err(|_| RepositorySnapshotV13Error::ContractInvalid)?;
        let baseline = RepositorySnapshotV11::from_inventory_and_callable_semantics(
            inventory, knowledge, envelope,
        )
        .map_err(map_v11_error)?;
        let mut value = baseline.value().clone();
        let semantic = value
            .get_mut("semantic")
            .and_then(Value::as_object_mut)
            .ok_or(RepositorySnapshotV13Error::ContractInvalid)?;
        let configuration_without_hash = json!({
            "schema_version": R11_CONFIGURATION_VERSION,
            "profile": "standard-local-s4",
            "workspace_profile": "cargo-root-package-v1",
            "manifest_profile": "cargo-manifest-facts-v1",
            "rust_semantic_profile": "rust-semantic-depth-v1",
            "rust_framework_profile": "rust-framework-declarations-v1",
            "rust_callable_profile": K1_PROFILE,
            "repository_boundary_profile": LOCAL_GITLINKS_V1
        });
        let configuration_hash =
            semantic_hash(CONFIGURATION_V10_HASH_DOMAIN, &configuration_without_hash);
        let mut configuration = configuration_without_hash;
        configuration["semantic_hash"] =
            json!({"algorithm": "blake3-256", "value": configuration_hash});
        semantic.insert("configuration".to_owned(), configuration);
        semantic.insert(
            "pipeline_version".to_owned(),
            Value::String(R11_PIPELINE_VERSION.to_owned()),
        );
        semantic.insert(
            "ontology_version".to_owned(),
            Value::String(R11_ONTOLOGY_VERSION.to_owned()),
        );
        semantic.insert(
            "extractor_contract_version".to_owned(),
            Value::String(R11_EXTRACTION_CONTRACT_VERSION.to_owned()),
        );
        semantic.insert(
            "extractor_versions".to_owned(),
            json!([
                "codenoesis.inventory-classifier/s1-v1",
                "codenoesis.rust-tree-sitter/s4-v1",
                "codenoesis.rust-workspace/s4-r3-v1",
                "codenoesis.cargo-manifest/s4-r4-v1",
                "codenoesis.rust-semantic/s4-r5-v1",
                "codenoesis.rust-framework/s4-r6-v1",
                K1_EXTRACTOR_VERSION,
                "codenoesis.git-boundary/s1-v1",
                R11_COMPOSITION_VERSION
            ]),
        );
        let chunks = semantic
            .get_mut("extraction_chunks")
            .and_then(Value::as_array_mut)
            .ok_or(RepositorySnapshotV13Error::ContractInvalid)?;
        for chunk in chunks {
            replace_version_and_hash(
                chunk,
                R11_EXTRACTION_CHUNK_VERSION,
                EXTRACTION_V10_HASH_DOMAIN,
            )?;
        }
        let graph = semantic
            .get_mut("knowledge_graph")
            .ok_or(RepositorySnapshotV13Error::ContractInvalid)?;
        graph["schema_version"] = Value::String(R11_GRAPH_VERSION.to_owned());
        graph["ontology_version"] = Value::String(R11_ONTOLOGY_VERSION.to_owned());
        graph["extractor_versions"] = json!([
            "codenoesis.rust-tree-sitter/s4-v1",
            "codenoesis.rust-workspace/s4-r3-v1",
            "codenoesis.cargo-manifest/s4-r4-v1",
            "codenoesis.rust-semantic/s4-r5-v1",
            "codenoesis.rust-framework/s4-r6-v1",
            K1_EXTRACTOR_VERSION,
            "codenoesis.git-boundary/s1-v1",
            R11_COMPOSITION_VERSION
        ]);
        replace_hash(graph, GRAPH_V10_HASH_DOMAIN)?;
        semantic.insert(
            "repository_boundaries".to_owned(),
            repository_boundary_value(boundaries),
        );
        let semantic_value = Value::Object(semantic.clone());
        let snapshot_hash = semantic_hash(SNAPSHOT_V13_HASH_DOMAIN, &semantic_value);
        let root = value
            .as_object_mut()
            .ok_or(RepositorySnapshotV13Error::ContractInvalid)?;
        root.insert(
            "schema_version".to_owned(),
            Value::String(R11_SNAPSHOT_VERSION.to_owned()),
        );
        root.insert(
            "semantic_hash".to_owned(),
            json!({"algorithm": "blake3-256", "value": snapshot_hash}),
        );
        publication_candidate(&value).map_err(|_| RepositorySnapshotV13Error::ContractInvalid)?;
        Ok(Self { value })
    }

    /// Serializes V13 under the selected unchanged K1 output envelope.
    ///
    /// # Errors
    ///
    /// Returns a serialization or output-limit failure.
    pub fn canonical_stdout_with_output_capacity(
        &self,
        profile: K1OutputCapacityProfile,
    ) -> Result<Vec<u8>, RepositorySnapshotV13Error> {
        let maximum = usize::try_from(profile.maximum_bytes())
            .map_err(|_| RepositorySnapshotV13Error::OutputLengthOverflow)?;
        let body_maximum = maximum
            .checked_sub(1)
            .ok_or(RepositorySnapshotV13Error::OutputLengthOverflow)?;
        let mut writer = LimitedVecWriter::new(body_maximum);
        let result = serde_json::to_writer(&mut writer, &self.value);
        if writer.overflowed() {
            return Err(RepositorySnapshotV13Error::LimitExceeded(
                AcquisitionError::LimitExceeded {
                    limit: LimitKind::CanonicalOutputBytes,
                    maximum: profile.maximum_bytes(),
                    observed: profile.maximum_bytes().saturating_add(1),
                },
            ));
        }
        result.map_err(RepositorySnapshotV13Error::Serialization)?;
        let mut bytes = writer.into_inner();
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Serializes the exact V13 semantic payload.
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

    /// Converts V13 into the immutable publication model.
    ///
    /// # Errors
    ///
    /// Returns a strict contract or storage-integrity failure.
    pub fn publication_candidate(&self) -> Result<PublicationCandidate, PublicationCandidateError> {
        publication_candidate(&self.value)
    }
}

/// Validates one loaded V13 semantic payload against its visible head.
///
/// # Errors
///
/// Returns a typed storage-integrity failure on any mismatch.
pub fn validate_stored_snapshot_semantic_v13(
    semantic: &Value,
    head: &LocalSnapshotHead,
) -> Result<(), StorageError> {
    if head.snapshot_schema_version != SNAPSHOT_SCHEMA_VERSION_V13 {
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
pub struct LocalQueryResultV8 {
    value: Value,
}

impl LocalQueryResultV8 {
    /// Serializes one bounded exact-ID V8 result followed by LF.
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

/// Builds one exact-ID V8 result over K1 and R2 boundary subjects.
///
/// # Errors
///
/// Returns a strict snapshot, document, not-found, or result-limit failure.
pub fn local_query_result_v8(
    semantic: &Value,
    manifest: &Value,
    snapshot_id: &str,
    requested_id: &str,
) -> Result<LocalQueryResultV8, QueryContractError> {
    if semantic.get("ontology_version").and_then(Value::as_str) != Some(R11_ONTOLOGY_VERSION)
        || semantic
            .pointer("/knowledge_graph/schema_version")
            .and_then(Value::as_str)
            != Some(R11_GRAPH_VERSION)
    {
        return Err(QueryContractError::InvalidSnapshot);
    }
    let mut k1_semantic = semantic.clone();
    k1_semantic["knowledge_graph"]["schema_version"] = Value::String(K1_GRAPH_VERSION.to_owned());
    let boundaries = semantic
        .get("repository_boundaries")
        .ok_or(QueryContractError::InvalidSnapshot)?;
    validate_boundary_value(boundaries).map_err(|_| QueryContractError::InvalidSnapshot)?;
    let boundary_match = boundary_subject(boundaries, requested_id)?;
    let mut value = if let Some((kind, record)) = boundary_match {
        let seed_id = k1_semantic
            .pointer("/knowledge_graph/entities/0/id")
            .and_then(Value::as_str)
            .ok_or(QueryContractError::InvalidSnapshot)?;
        local_query_result_v6(&k1_semantic, manifest, snapshot_id, seed_id)?;
        boundary_query_value(semantic, manifest, snapshot_id, requested_id, kind, record)?
    } else {
        local_query_result_v6(&k1_semantic, manifest, snapshot_id, requested_id)?
            .value()
            .clone()
    };
    value["schema_version"] = Value::String(R11_QUERY_VERSION.to_owned());
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
    let result = LocalQueryResultV8 { value };
    result.canonical_stdout()?;
    Ok(result)
}

#[derive(Clone, Debug)]
pub struct PortableGraphV4 {
    value: Value,
    canonical: Vec<u8>,
    sha256: R11Sha256,
}

impl PortableGraphV4 {
    /// Projects one validated V13 head and deterministic documentation manifest.
    ///
    /// # Errors
    ///
    /// Returns a strict binding, privacy, reference, or limit failure.
    pub fn from_validated_v13(
        semantic: &Value,
        head: &LocalSnapshotHead,
        documentation_manifest: &Value,
        sha256: R11Sha256,
    ) -> Result<Self, R11ContractError> {
        validate_stored_snapshot_semantic_v13(semantic, head)
            .map_err(|_| R11ContractError::InvalidSnapshot)?;
        if semantic.get("ontology_version").and_then(Value::as_str) != Some(R11_ONTOLOGY_VERSION) {
            return Err(R11ContractError::InvalidSnapshot);
        }
        validate_documentation_binding(documentation_manifest, head)?;
        let graph = semantic
            .get("knowledge_graph")
            .and_then(Value::as_object)
            .ok_or(R11ContractError::InvalidSnapshot)?;
        let boundaries = semantic
            .get("repository_boundaries")
            .cloned()
            .ok_or(R11ContractError::InvalidSnapshot)?;
        validate_boundary_value(&boundaries)?;
        let (documents, document_statements) = portable_documents(documentation_manifest)?;
        let mut value = json!({
            "schema_version": R11_PORTABLE_GRAPH_VERSION,
            "repository": semantic.get("repository").cloned().ok_or(R11ContractError::InvalidSnapshot)?,
            "source_snapshot": {
                "schema_version": R11_SNAPSHOT_VERSION,
                "snapshot_id": head.snapshot_id.as_str(),
                "semantic_hash": {
                    "algorithm": head.semantic_hash.algorithm,
                    "value": head.semantic_hash.value
                }
            },
            "ontology_version": R11_ONTOLOGY_VERSION,
            "query_contract_version": R11_QUERY_VERSION,
            "projection": {
                "profile": "codenoesis.lossless-portable-projection/v4",
                "family_sha256": {}
            },
            "repository_boundaries": boundaries,
            "entities": graph.get("entities").cloned().ok_or(R11ContractError::InvalidSnapshot)?,
            "relationships": graph.get("relationships").cloned().ok_or(R11ContractError::InvalidSnapshot)?,
            "claims": graph.get("claims").cloned().ok_or(R11ContractError::InvalidSnapshot)?,
            "evidence": graph.get("evidence").cloned().ok_or(R11ContractError::InvalidSnapshot)?,
            "diagnostics": graph.get("diagnostics").cloned().ok_or(R11ContractError::InvalidSnapshot)?,
            "coverage_gaps": graph.get("coverage").cloned().ok_or(R11ContractError::InvalidSnapshot)?,
            "documents": documents,
            "document_statements": document_statements
        });
        value["projection"]["family_sha256"] = family_digests(&value, sha256)?;
        Self::from_generated_value(value, sha256)
    }

    /// Strictly reimports one canonical LF-terminated `PortableGraphV4`.
    ///
    /// # Errors
    ///
    /// Returns the first decode, schema, identity, reference, privacy, or limit failure.
    pub fn from_canonical_file(bytes: &[u8], sha256: R11Sha256) -> Result<Self, R11ContractError> {
        enforce_portable_size(bytes.len())?;
        let value = serde_json::from_slice::<Value>(bytes)
            .map_err(|_| R11ContractError::InvalidProjection)?;
        validate_portable_value(&value, sha256)?;
        let canonical = serde_json::to_vec(&value).map_err(|_| R11ContractError::Internal)?;
        let mut expected = canonical.clone();
        expected.push(b'\n');
        if expected != bytes {
            return Err(R11ContractError::Noncanonical {
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

    fn from_generated_value(value: Value, sha256: R11Sha256) -> Result<Self, R11ContractError> {
        validate_portable_value(&value, sha256)?;
        let canonical = serde_json::to_vec(&value).map_err(|_| R11ContractError::Internal)?;
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
pub struct LocalExplorerManifestV4 {
    value: Value,
}

impl LocalExplorerManifestV4 {
    /// Builds the offline V4 explorer manifest over immutable K1 viewer bytes.
    ///
    /// # Errors
    ///
    /// Returns an integrity or unsafe-CSP failure.
    pub fn new(
        portable: &PortableGraphV4,
        viewer_bytes: &[u8],
        expected_viewer_sha256: &str,
        content_security_policy: &str,
        sha256: R11Sha256,
    ) -> Result<Self, R11ContractError> {
        if sha256(viewer_bytes) != expected_viewer_sha256
            || content_security_policy.contains("http:")
            || content_security_policy.contains("https:")
            || content_security_policy.contains("unsafe-inline")
            || content_security_policy.contains("unsafe-eval")
        {
            return Err(R11ContractError::AssetIntegrityMismatch);
        }
        Ok(Self {
            value: json!({
                "schema_version": R11_LOCAL_EXPLORER_VERSION,
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
                    "profile": R11_EXPLORER_SECURITY_PROFILE,
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
                    "bounded_traversal": [1, 2]
                },
                "limits": {
                    "text_search_results": MAX_R11_TEXT_SEARCH_RESULTS,
                    "traversal_depth_default": R11_TRAVERSAL_DEPTH_DEFAULT,
                    "traversal_depth_maximum": MAX_R11_TRAVERSAL_DEPTH
                }
            }),
        })
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Serializes one `LocalExplorerManifestV4` followed by LF.
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
pub enum R11ContractError {
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

impl Display for R11ContractError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidSnapshot => "invalid R11 snapshot",
            Self::UnsupportedSnapshotSchema(_) => "unsupported R11 snapshot schema",
            Self::UnsupportedPortableGraphSchema(_) => "unsupported portable graph schema",
            Self::Noncanonical { .. } => "noncanonical portable graph",
            Self::IdentityConflict { .. } => "portable identity conflict",
            Self::ReferenceMismatch { .. } => "portable reference mismatch",
            Self::LimitExceeded { .. } => "portable graph limit exceeded",
            Self::UnsafePayload { .. } => "unsafe portable payload",
            Self::AssetIntegrityMismatch => "explorer asset integrity mismatch",
            Self::InvalidProjection => "invalid portable graph projection",
            Self::Internal => "internal R11 contract error",
        })
    }
}

impl Error for R11ContractError {}

fn replace_version_and_hash(
    value: &mut Value,
    version: &str,
    domain: &[u8],
) -> Result<(), RepositorySnapshotV13Error> {
    value["schema_version"] = Value::String(version.to_owned());
    replace_hash(value, domain)
}

fn replace_hash(value: &mut Value, domain: &[u8]) -> Result<(), RepositorySnapshotV13Error> {
    value
        .as_object_mut()
        .ok_or(RepositorySnapshotV13Error::ContractInvalid)?
        .remove("semantic_hash")
        .ok_or(RepositorySnapshotV13Error::ContractInvalid)?;
    let hash = semantic_hash(domain, value);
    value["semantic_hash"] = json!({"algorithm": "blake3-256", "value": hash});
    Ok(())
}

fn map_v11_error(error: RepositorySnapshotV11Error) -> RepositorySnapshotV13Error {
    match error {
        RepositorySnapshotV11Error::Serialization(error) => {
            RepositorySnapshotV13Error::Serialization(error)
        }
        RepositorySnapshotV11Error::LimitExceeded(error) => {
            RepositorySnapshotV13Error::LimitExceeded(error)
        }
        RepositorySnapshotV11Error::ContractInvalid => RepositorySnapshotV13Error::ContractInvalid,
        RepositorySnapshotV11Error::OutputLengthOverflow => {
            RepositorySnapshotV13Error::OutputLengthOverflow
        }
    }
}

fn boundary_subject<'a>(
    boundaries: &'a Value,
    requested_id: &str,
) -> Result<Option<(&'static str, &'a Value)>, QueryContractError> {
    for (family, key, kind) in [
        ("boundaries", "boundary_id", "repository_boundary"),
        ("declarations", "declaration_id", "boundary_declaration"),
        ("evidence", "evidence_id", "boundary_evidence"),
        ("coverage_gaps", "gap_id", "boundary_coverage_gap"),
    ] {
        let values = boundaries
            .get(family)
            .and_then(Value::as_array)
            .ok_or(QueryContractError::InvalidSnapshot)?;
        if let Some(record) = values
            .iter()
            .find(|record| record.get(key).and_then(Value::as_str) == Some(requested_id))
        {
            return Ok(Some((kind, record)));
        }
    }
    Ok(None)
}

#[allow(clippy::too_many_lines)]
fn boundary_query_value(
    semantic: &Value,
    manifest: &Value,
    snapshot_id: &str,
    requested_id: &str,
    kind: &str,
    record: &Value,
) -> Result<Value, QueryContractError> {
    let boundaries = semantic
        .get("repository_boundaries")
        .ok_or(QueryContractError::InvalidSnapshot)?;
    let mut linked_boundaries = linked_boundary_records(boundaries, "boundaries", |value| {
        value.get("boundary_id").and_then(Value::as_str) == Some(requested_id)
            || value.get("declaration_id").and_then(Value::as_str) == Some(requested_id)
            || value
                .get("evidence_ids")
                .and_then(Value::as_array)
                .is_some_and(|ids| contains_string(ids, requested_id))
            || value
                .get("coverage_gap_ids")
                .and_then(Value::as_array)
                .is_some_and(|ids| contains_string(ids, requested_id))
    })?;
    if kind == "repository_boundary" && linked_boundaries.is_empty() {
        linked_boundaries.push(record.clone());
    }
    let boundary_ids = linked_boundaries
        .iter()
        .filter_map(|value| value.get("boundary_id").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    let linked_declarations = linked_boundary_records(boundaries, "declarations", |value| {
        value.get("declaration_id").and_then(Value::as_str) == Some(requested_id)
            || value.get("evidence_id").and_then(Value::as_str) == Some(requested_id)
            || value
                .get("boundary_id")
                .and_then(Value::as_str)
                .is_some_and(|id| boundary_ids.contains(id))
    })?;
    let declaration_ids = linked_declarations
        .iter()
        .filter_map(|value| value.get("declaration_id").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    let mut evidence_ids = linked_boundaries
        .iter()
        .chain(linked_declarations.iter())
        .flat_map(|value| {
            value
                .get("evidence_ids")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .chain(value.get("evidence_id"))
        })
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    if kind == "boundary_evidence" {
        evidence_ids.insert(requested_id);
    }
    let linked_evidence = linked_boundary_records(boundaries, "evidence", |value| {
        value
            .get("evidence_id")
            .and_then(Value::as_str)
            .is_some_and(|id| evidence_ids.contains(id))
    })?;
    let linked_gaps = linked_boundary_records(boundaries, "coverage_gaps", |value| {
        value.get("gap_id").and_then(Value::as_str) == Some(requested_id)
            || value
                .get("subject_id")
                .and_then(Value::as_str)
                .is_some_and(|id| boundary_ids.contains(id) || declaration_ids.contains(id))
            || value
                .get("evidence_ids")
                .and_then(Value::as_array)
                .is_some_and(|ids| contains_string(ids, requested_id))
    })?;
    let linked_workspace_members = semantic
        .pointer("/knowledge_graph/workspace/members")
        .and_then(Value::as_array)
        .ok_or(QueryContractError::InvalidSnapshot)?
        .iter()
        .filter(|member| {
            member
                .get("external_boundary_id")
                .and_then(Value::as_str)
                .is_some_and(|id| boundary_ids.contains(id))
        })
        .cloned()
        .collect::<Vec<_>>();
    Ok(json!({
        "schema_version": R11_QUERY_VERSION,
        "repository_identity": semantic.pointer("/repository/identity").and_then(Value::as_str).ok_or(QueryContractError::InvalidSnapshot)?,
        "snapshot_id": snapshot_id,
        "requested_id": requested_id,
        "result_kind": kind,
        "entity": null,
        "relationship": null,
        "claims": [],
        "evidence": [],
        "diagnostic": null,
        "coverage_gap": null,
        "document": null,
        "document_statements": linked_document_statements(manifest, requested_id)?,
        "linked_k1_entities": linked_workspace_members,
        "linked_k1_relationships": [],
        "repository_boundary": (kind == "repository_boundary").then(|| record.clone()),
        "boundary_declaration": (kind == "boundary_declaration").then(|| record.clone()),
        "boundary_evidence": (kind == "boundary_evidence").then(|| record.clone()),
        "boundary_coverage_gap": (kind == "boundary_coverage_gap").then(|| record.clone()),
        "linked_repository_boundaries": linked_boundaries,
        "linked_boundary_declarations": linked_declarations,
        "linked_boundary_evidence": linked_evidence,
        "linked_boundary_coverage_gaps": linked_gaps
    }))
}

fn linked_boundary_records(
    boundaries: &Value,
    family: &str,
    predicate: impl Fn(&Value) -> bool,
) -> Result<Vec<Value>, QueryContractError> {
    Ok(boundaries
        .get(family)
        .and_then(Value::as_array)
        .ok_or(QueryContractError::InvalidSnapshot)?
        .iter()
        .filter(|record| predicate(record))
        .cloned()
        .collect())
}

fn linked_document_statements(
    manifest: &Value,
    requested_id: &str,
) -> Result<Vec<Value>, QueryContractError> {
    Ok(manifest
        .get("documents")
        .and_then(Value::as_array)
        .ok_or(QueryContractError::InvalidDocuments)?
        .iter()
        .flat_map(|document| {
            document
                .get("statements")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .filter(|statement| {
            statement
                .get("subject_ids")
                .and_then(Value::as_array)
                .is_some_and(|ids| contains_string(ids, requested_id))
        })
        .cloned()
        .collect())
}

fn contains_string(values: &[Value], expected: &str) -> bool {
    values.iter().any(|value| value.as_str() == Some(expected))
}

fn validate_documentation_binding(
    manifest: &Value,
    head: &LocalSnapshotHead,
) -> Result<(), R11ContractError> {
    if manifest.get("repository_identity").and_then(Value::as_str)
        != Some(head.repository_identity.as_str())
        || manifest.get("snapshot_id").and_then(Value::as_str) != Some(head.snapshot_id.as_str())
        || manifest
            .pointer("/snapshot_semantic_hash/value")
            .and_then(Value::as_str)
            != Some(head.semantic_hash.value.as_str())
    {
        return Err(R11ContractError::InvalidSnapshot);
    }
    Ok(())
}

fn portable_documents(manifest: &Value) -> Result<(Vec<Value>, Vec<Value>), R11ContractError> {
    let source = manifest
        .get("documents")
        .and_then(Value::as_array)
        .ok_or(R11ContractError::InvalidSnapshot)?;
    let mut documents = Vec::with_capacity(source.len());
    let mut statements = Vec::new();
    for document in source {
        let mut record = document
            .as_object()
            .cloned()
            .ok_or(R11ContractError::InvalidSnapshot)?;
        let mut document_statements = record
            .remove("statements")
            .and_then(|value| value.as_array().cloned())
            .ok_or(R11ContractError::InvalidSnapshot)?;
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

fn family_digests(value: &Value, sha256: R11Sha256) -> Result<Value, R11ContractError> {
    let mut digests = Map::new();
    for family in PORTABLE_FAMILIES {
        let bytes = serde_json::to_vec(
            value
                .get(family)
                .ok_or(R11ContractError::InvalidProjection)?,
        )
        .map_err(|_| R11ContractError::Internal)?;
        digests.insert(family.to_owned(), Value::String(sha256(&bytes)));
    }
    Ok(Value::Object(digests))
}

fn validate_portable_value(value: &Value, sha256: R11Sha256) -> Result<(), R11ContractError> {
    let object = value
        .as_object()
        .ok_or(R11ContractError::InvalidProjection)?;
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
        return Err(R11ContractError::InvalidProjection);
    }
    let schema = object
        .get("schema_version")
        .and_then(Value::as_str)
        .unwrap_or("missing");
    if schema != R11_PORTABLE_GRAPH_VERSION {
        return Err(R11ContractError::UnsupportedPortableGraphSchema(bounded(
            schema, 256,
        )));
    }
    if value
        .pointer("/source_snapshot/schema_version")
        .and_then(Value::as_str)
        != Some(R11_SNAPSHOT_VERSION)
        || object.get("ontology_version").and_then(Value::as_str) != Some(R11_ONTOLOGY_VERSION)
        || object.get("query_contract_version").and_then(Value::as_str) != Some(R11_QUERY_VERSION)
        || value.pointer("/projection/profile").and_then(Value::as_str)
            != Some("codenoesis.lossless-portable-projection/v4")
    {
        return Err(R11ContractError::InvalidProjection);
    }
    ensure_nesting(value, 0)?;
    validate_private_fields(value)?;
    validate_boundary_value(&object["repository_boundaries"])?;
    if value.pointer("/projection/family_sha256") != Some(&family_digests(value, sha256)?) {
        return Err(R11ContractError::InvalidProjection);
    }
    let mut k1 = value.clone();
    let k1_object = k1
        .as_object_mut()
        .ok_or(R11ContractError::InvalidProjection)?;
    k1_object.remove("repository_boundaries");
    k1_object.insert(
        "schema_version".to_owned(),
        Value::String("codenoesis.portable-graph/v2".to_owned()),
    );
    k1["source_snapshot"]["schema_version"] =
        Value::String("codenoesis.repository-snapshot/v11".to_owned());
    k1["ontology_version"] = Value::String(K1_ONTOLOGY_VERSION.to_owned());
    k1["query_contract_version"] = Value::String(K1_QUERY_VERSION.to_owned());
    k1["projection"]["profile"] =
        Value::String("codenoesis.lossless-portable-projection/v2".to_owned());
    let mut bytes = serde_json::to_vec(&k1).map_err(|_| R11ContractError::Internal)?;
    bytes.push(b'\n');
    PortableGraphV2::from_canonical_file(&bytes, sha256).map_err(map_k1_contract)?;
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn validate_boundary_value(value: &Value) -> Result<(), R11ContractError> {
    let object = exact_boundary_object(
        value,
        &[
            "schema_version",
            "profile",
            "root_repository",
            "summary",
            "boundaries",
            "declarations",
            "coverage_gaps",
            "evidence",
        ],
    )?;
    if object.get("schema_version").and_then(Value::as_str)
        != Some("codenoesis.repository-boundaries/v1")
        || object.get("profile").and_then(Value::as_str) != Some(LOCAL_GITLINKS_V1)
    {
        return Err(R11ContractError::InvalidProjection);
    }
    validate_boundary_shapes(object)?;
    let mut family_ids = BTreeMap::<&str, BTreeSet<&str>>::new();
    for (family, key, prefix) in [
        (
            "boundaries",
            "boundary_id",
            "urn:codenoesis:repository-boundary:sha256:",
        ),
        (
            "declarations",
            "declaration_id",
            "urn:codenoesis:gitmodules-declaration:sha256:",
        ),
        (
            "evidence",
            "evidence_id",
            "urn:codenoesis:boundary-evidence:sha256:",
        ),
        (
            "coverage_gaps",
            "gap_id",
            "urn:codenoesis:boundary-gap:sha256:",
        ),
    ] {
        let records = object
            .get(family)
            .and_then(Value::as_array)
            .ok_or(R11ContractError::InvalidProjection)?;
        let mut ids = BTreeSet::new();
        let mut previous_key = None;
        for record in records {
            let id = record
                .get(key)
                .and_then(Value::as_str)
                .filter(|id| valid_sha256_urn(id, prefix))
                .ok_or(R11ContractError::InvalidProjection)?;
            if !ids.insert(id) {
                return Err(R11ContractError::IdentityConflict {
                    family,
                    id: id.to_owned(),
                });
            }
            let path = record
                .get("path")
                .and_then(Value::as_str)
                .ok_or(R11ContractError::InvalidProjection)?;
            if !safe_relative_path(path) {
                return Err(R11ContractError::UnsafePayload {
                    reason: "unsafe_boundary_path",
                });
            }
            let key = boundary_record_order_key(family, record, path, id)?;
            if previous_key
                .as_ref()
                .is_some_and(|previous| previous >= &key)
            {
                return Err(R11ContractError::IdentityConflict {
                    family,
                    id: id.to_owned(),
                });
            }
            previous_key = Some(key);
        }
        family_ids.insert(family, ids);
    }
    for boundary in object["boundaries"]
        .as_array()
        .ok_or(R11ContractError::InvalidProjection)?
    {
        validate_id_references(
            boundary,
            "evidence_ids",
            &family_ids["evidence"],
            "boundaries",
        )?;
        validate_id_references(
            boundary,
            "coverage_gap_ids",
            &family_ids["coverage_gaps"],
            "boundaries",
        )?;
        if let Some(id) = boundary.get("declaration_id").and_then(Value::as_str)
            && !family_ids["declarations"].contains(id)
        {
            return Err(R11ContractError::ReferenceMismatch {
                family: "boundaries",
                id: id.to_owned(),
            });
        }
        if let Some(declaration_id) = boundary.get("declaration_id").and_then(Value::as_str) {
            let declaration =
                record_by_id(&object["declarations"], "declaration_id", declaration_id)?;
            if declaration.get("boundary_id").and_then(Value::as_str)
                != boundary.get("boundary_id").and_then(Value::as_str)
                || declaration.get("path") != boundary.get("path")
            {
                return Err(R11ContractError::ReferenceMismatch {
                    family: "boundaries",
                    id: declaration_id.to_owned(),
                });
            }
        }
        let has_matching_tree_evidence = boundary["evidence_ids"]
            .as_array()
            .ok_or(R11ContractError::InvalidProjection)?
            .iter()
            .filter_map(Value::as_str)
            .filter_map(|id| record_by_id(&object["evidence"], "evidence_id", id).ok())
            .any(|evidence| {
                evidence.get("kind").and_then(Value::as_str) == Some("git_tree_entry")
                    && evidence.get("path") == boundary.get("path")
                    && evidence.get("object_oid") == boundary.get("gitlink_oid")
            });
        if !has_matching_tree_evidence {
            return Err(R11ContractError::ReferenceMismatch {
                family: "boundaries",
                id: boundary["boundary_id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
            });
        }
    }
    for declaration in object["declarations"]
        .as_array()
        .ok_or(R11ContractError::InvalidProjection)?
    {
        for (field, family) in [("boundary_id", "boundaries"), ("evidence_id", "evidence")] {
            if let Some(id) = declaration.get(field).and_then(Value::as_str)
                && !family_ids[family].contains(id)
            {
                return Err(R11ContractError::ReferenceMismatch {
                    family: "declarations",
                    id: id.to_owned(),
                });
            }
        }
        let evidence_id = declaration["evidence_id"]
            .as_str()
            .ok_or(R11ContractError::InvalidProjection)?;
        let evidence = record_by_id(&object["evidence"], "evidence_id", evidence_id)?;
        if evidence.get("kind").and_then(Value::as_str) != Some("gitmodules_declaration") {
            return Err(R11ContractError::ReferenceMismatch {
                family: "declarations",
                id: evidence_id.to_owned(),
            });
        }
        if let Some(boundary_id) = declaration.get("boundary_id").and_then(Value::as_str) {
            let boundary = record_by_id(&object["boundaries"], "boundary_id", boundary_id)?;
            if boundary.get("declaration_id").and_then(Value::as_str)
                != declaration.get("declaration_id").and_then(Value::as_str)
                || boundary.get("path") != declaration.get("path")
            {
                return Err(R11ContractError::ReferenceMismatch {
                    family: "declarations",
                    id: boundary_id.to_owned(),
                });
            }
        }
    }
    for gap in object["coverage_gaps"]
        .as_array()
        .ok_or(R11ContractError::InvalidProjection)?
    {
        let subject_id = gap
            .get("subject_id")
            .and_then(Value::as_str)
            .ok_or(R11ContractError::InvalidProjection)?;
        if !family_ids["boundaries"].contains(subject_id)
            && !family_ids["declarations"].contains(subject_id)
        {
            return Err(R11ContractError::ReferenceMismatch {
                family: "coverage_gaps",
                id: subject_id.to_owned(),
            });
        }
        validate_id_references(
            gap,
            "evidence_ids",
            &family_ids["evidence"],
            "coverage_gaps",
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn validate_boundary_shapes(object: &Map<String, Value>) -> Result<(), R11ContractError> {
    let root = validate_repository_value(&object["root_repository"])?;
    let root_identity = required_boundary_string(root, "identity")?;
    let root_commit = required_boundary_string(root, "commit_oid")?;
    let boundaries = bounded_boundary_array(object, "boundaries", 128)?;
    let declarations = bounded_boundary_array(object, "declarations", 256)?;
    let gaps = bounded_boundary_array(object, "coverage_gaps", 512)?;
    let evidence = bounded_boundary_array(object, "evidence", 384)?;

    for boundary in boundaries {
        let boundary = exact_boundary_object(
            boundary,
            &[
                "boundary_id",
                "path",
                "gitlink_oid",
                "state",
                "declaration_id",
                "nested_repository",
                "evidence_ids",
                "coverage_gap_ids",
            ],
        )?;
        require_sha256_urn(
            boundary,
            "boundary_id",
            "urn:codenoesis:repository-boundary:sha256:",
        )?;
        require_boundary_path(boundary, "path")?;
        require_sha1(boundary, "gitlink_oid")?;
        validate_nullable_id(
            boundary,
            "declaration_id",
            "urn:codenoesis:gitmodules-declaration:sha256:",
        )?;
        validate_id_array(
            boundary,
            "evidence_ids",
            "urn:codenoesis:boundary-evidence:sha256:",
            1,
            2,
        )?;
        validate_id_array(
            boundary,
            "coverage_gap_ids",
            "urn:codenoesis:boundary-gap:sha256:",
            1,
            4,
        )?;
        let state = required_boundary_string(boundary, "state")?;
        let declaration_present = boundary["declaration_id"].is_string();
        let nested = match &boundary["nested_repository"] {
            Value::Null => None,
            value => Some(validate_repository_value(value)?),
        };
        let state_valid = match state {
            "declared_unbound" => declaration_present && nested.is_none(),
            "undeclared_unbound" => !declaration_present && nested.is_none(),
            "explicitly_bound" => nested.is_some(),
            _ => false,
        };
        if !state_valid
            || nested.is_some_and(|repository| {
                repository.get("commit_oid") != boundary.get("gitlink_oid")
            })
        {
            return Err(R11ContractError::InvalidProjection);
        }
    }

    for declaration in declarations {
        let declaration = exact_boundary_object(
            declaration,
            &[
                "declaration_id",
                "name_sha256",
                "path",
                "url_kind",
                "url_sha256",
                "unsupported_keys",
                "boundary_id",
                "evidence_id",
            ],
        )?;
        require_sha256_urn(
            declaration,
            "declaration_id",
            "urn:codenoesis:gitmodules-declaration:sha256:",
        )?;
        require_sha256(declaration, "name_sha256")?;
        require_boundary_path(declaration, "path")?;
        if !matches!(
            required_boundary_string(declaration, "url_kind")?,
            "relative"
                | "absolute_path"
                | "file"
                | "ssh"
                | "https"
                | "http"
                | "git"
                | "scp_like"
                | "other"
        ) {
            return Err(R11ContractError::InvalidProjection);
        }
        require_sha256(declaration, "url_sha256")?;
        validate_unsupported_keys(&declaration["unsupported_keys"])?;
        validate_nullable_id(
            declaration,
            "boundary_id",
            "urn:codenoesis:repository-boundary:sha256:",
        )?;
        require_sha256_urn(
            declaration,
            "evidence_id",
            "urn:codenoesis:boundary-evidence:sha256:",
        )?;
    }

    for gap in gaps {
        let gap = exact_boundary_object(
            gap,
            &["gap_id", "code", "path", "subject_id", "evidence_ids"],
        )?;
        require_sha256_urn(gap, "gap_id", "urn:codenoesis:boundary-gap:sha256:")?;
        if !matches!(
            required_boundary_string(gap, "code")?,
            "boundary.nested_repository_unbound"
                | "boundary.gitmodules_declaration_missing"
                | "boundary.gitmodules_declaration_orphan"
                | "boundary.gitmodules_key_unsupported"
                | "boundary.nested_repository_not_analyzed"
        ) {
            return Err(R11ContractError::InvalidProjection);
        }
        require_boundary_path(gap, "path")?;
        let subject_id = required_boundary_string(gap, "subject_id")?;
        if !valid_sha256_urn(subject_id, "urn:codenoesis:repository-boundary:sha256:")
            && !valid_sha256_urn(subject_id, "urn:codenoesis:gitmodules-declaration:sha256:")
        {
            return Err(R11ContractError::InvalidProjection);
        }
        validate_id_array(
            gap,
            "evidence_ids",
            "urn:codenoesis:boundary-evidence:sha256:",
            1,
            2,
        )?;
    }

    for evidence in evidence {
        validate_boundary_evidence(evidence, root_identity, root_commit)?;
    }
    validate_boundary_summary(object, boundaries, declarations, gaps)?;
    Ok(())
}

fn validate_boundary_summary(
    object: &Map<String, Value>,
    boundaries: &[Value],
    declarations: &[Value],
    gaps: &[Value],
) -> Result<(), R11ContractError> {
    let summary = exact_boundary_object(
        &object["summary"],
        &[
            "boundary_count",
            "declaration_count",
            "bound_count",
            "unbound_count",
            "coverage_gap_count",
        ],
    )?;
    let bound_count = boundaries
        .iter()
        .filter(|boundary| {
            boundary.get("state").and_then(Value::as_str) == Some("explicitly_bound")
        })
        .count();
    let expected = [
        ("boundary_count", boundaries.len(), 128),
        ("declaration_count", declarations.len(), 256),
        ("bound_count", bound_count, 32),
        (
            "unbound_count",
            boundaries.len().saturating_sub(bound_count),
            128,
        ),
        ("coverage_gap_count", gaps.len(), 512),
    ];
    for (field, count, maximum) in expected {
        let observed = summary
            .get(field)
            .and_then(Value::as_u64)
            .ok_or(R11ContractError::InvalidProjection)?;
        if observed != u64::try_from(count).unwrap_or(u64::MAX) || observed > maximum {
            return Err(R11ContractError::InvalidProjection);
        }
    }
    Ok(())
}

fn validate_boundary_evidence(
    value: &Value,
    root_identity: &str,
    root_commit: &str,
) -> Result<(), R11ContractError> {
    let kind = value
        .get("kind")
        .and_then(Value::as_str)
        .ok_or(R11ContractError::InvalidProjection)?;
    let evidence = match kind {
        "git_tree_entry" => exact_boundary_object(
            value,
            &[
                "evidence_id",
                "kind",
                "repository",
                "tree_oid",
                "path",
                "mode",
                "object_oid",
            ],
        )?,
        "gitmodules_declaration" => exact_boundary_object(
            value,
            &[
                "evidence_id",
                "kind",
                "repository",
                "blob_oid",
                "path",
                "span",
            ],
        )?,
        _ => return Err(R11ContractError::InvalidProjection),
    };
    require_sha256_urn(
        evidence,
        "evidence_id",
        "urn:codenoesis:boundary-evidence:sha256:",
    )?;
    validate_repository_reference(&evidence["repository"], root_identity, root_commit)?;
    match kind {
        "git_tree_entry" => {
            require_sha1(evidence, "tree_oid")?;
            require_boundary_path(evidence, "path")?;
            if required_boundary_string(evidence, "mode")? != "160000" {
                return Err(R11ContractError::InvalidProjection);
            }
            require_sha1(evidence, "object_oid")?;
        }
        "gitmodules_declaration" => {
            require_sha1(evidence, "blob_oid")?;
            if required_boundary_string(evidence, "path")? != ".gitmodules" {
                return Err(R11ContractError::InvalidProjection);
            }
            let span = exact_boundary_object(&evidence["span"], &["unit", "start", "end"])?;
            let start = span
                .get("start")
                .and_then(Value::as_u64)
                .ok_or(R11ContractError::InvalidProjection)?;
            let end = span
                .get("end")
                .and_then(Value::as_u64)
                .ok_or(R11ContractError::InvalidProjection)?;
            if required_boundary_string(span, "unit")? != "byte"
                || start > 1_048_576
                || end == 0
                || end > 1_048_576
                || start >= end
            {
                return Err(R11ContractError::InvalidProjection);
            }
        }
        _ => unreachable!(),
    }
    Ok(())
}

fn validate_repository_value(value: &Value) -> Result<&Map<String, Value>, R11ContractError> {
    let repository = exact_boundary_object(
        value,
        &["identity", "vcs", "object_format", "commit_oid", "tree_oid"],
    )?;
    if !valid_repository_identity(required_boundary_string(repository, "identity")?)
        || required_boundary_string(repository, "vcs")? != "git"
        || required_boundary_string(repository, "object_format")? != "sha1"
    {
        return Err(R11ContractError::InvalidProjection);
    }
    require_sha1(repository, "commit_oid")?;
    require_sha1(repository, "tree_oid")?;
    Ok(repository)
}

fn validate_repository_reference(
    value: &Value,
    root_identity: &str,
    root_commit: &str,
) -> Result<(), R11ContractError> {
    let repository = exact_boundary_object(value, &["identity", "commit_oid"])?;
    if required_boundary_string(repository, "identity")? != root_identity
        || required_boundary_string(repository, "commit_oid")? != root_commit
    {
        return Err(R11ContractError::InvalidProjection);
    }
    Ok(())
}

fn validate_unsupported_keys(value: &Value) -> Result<(), R11ContractError> {
    let values = value
        .as_array()
        .filter(|values| values.len() <= 30)
        .ok_or(R11ContractError::InvalidProjection)?;
    let mut previous = None::<String>;
    for value in values {
        let key = exact_boundary_object(value, &["key", "value_sha256"])?;
        let name = required_boundary_string(key, "key")?;
        if !valid_gitmodules_key(name) {
            return Err(R11ContractError::InvalidProjection);
        }
        require_sha256(key, "value_sha256")?;
        let order = format!(
            "{name}\0{}",
            key["value_sha256"].as_str().unwrap_or_default()
        );
        if previous.as_ref().is_some_and(|previous| previous >= &order) {
            return Err(R11ContractError::InvalidProjection);
        }
        previous = Some(order);
    }
    Ok(())
}

fn bounded_boundary_array<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    maximum: usize,
) -> Result<&'a [Value], R11ContractError> {
    object
        .get(field)
        .and_then(Value::as_array)
        .filter(|values| values.len() <= maximum)
        .map(Vec::as_slice)
        .ok_or(R11ContractError::InvalidProjection)
}

fn exact_boundary_object<'a>(
    value: &'a Value,
    expected: &[&str],
) -> Result<&'a Map<String, Value>, R11ContractError> {
    value
        .as_object()
        .filter(|object| {
            object.len() == expected.len()
                && expected.iter().all(|field| object.contains_key(*field))
        })
        .ok_or(R11ContractError::InvalidProjection)
}

fn required_boundary_string<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, R11ContractError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or(R11ContractError::InvalidProjection)
}

fn require_boundary_path(object: &Map<String, Value>, field: &str) -> Result<(), R11ContractError> {
    if safe_relative_path(required_boundary_string(object, field)?) {
        Ok(())
    } else {
        Err(R11ContractError::UnsafePayload {
            reason: "unsafe_boundary_path",
        })
    }
}

fn require_sha1(object: &Map<String, Value>, field: &str) -> Result<(), R11ContractError> {
    if valid_lower_hex(required_boundary_string(object, field)?, 40) {
        Ok(())
    } else {
        Err(R11ContractError::InvalidProjection)
    }
}

fn require_sha256(object: &Map<String, Value>, field: &str) -> Result<(), R11ContractError> {
    if valid_lower_hex(required_boundary_string(object, field)?, 64) {
        Ok(())
    } else {
        Err(R11ContractError::InvalidProjection)
    }
}

fn require_sha256_urn(
    object: &Map<String, Value>,
    field: &str,
    prefix: &str,
) -> Result<(), R11ContractError> {
    if valid_sha256_urn(required_boundary_string(object, field)?, prefix) {
        Ok(())
    } else {
        Err(R11ContractError::InvalidProjection)
    }
}

fn validate_nullable_id(
    object: &Map<String, Value>,
    field: &str,
    prefix: &str,
) -> Result<(), R11ContractError> {
    match object.get(field) {
        Some(Value::Null) => Ok(()),
        Some(Value::String(value)) if valid_sha256_urn(value, prefix) => Ok(()),
        _ => Err(R11ContractError::InvalidProjection),
    }
}

fn validate_id_array(
    object: &Map<String, Value>,
    field: &str,
    prefix: &str,
    minimum: usize,
    maximum: usize,
) -> Result<(), R11ContractError> {
    let values = object
        .get(field)
        .and_then(Value::as_array)
        .filter(|values| values.len() >= minimum && values.len() <= maximum)
        .ok_or(R11ContractError::InvalidProjection)?;
    let mut ids = BTreeSet::new();
    for value in values {
        let id = value
            .as_str()
            .filter(|id| valid_sha256_urn(id, prefix))
            .ok_or(R11ContractError::InvalidProjection)?;
        if !ids.insert(id) {
            return Err(R11ContractError::InvalidProjection);
        }
    }
    Ok(())
}

fn boundary_record_order_key(
    family: &str,
    record: &Value,
    path: &str,
    id: &str,
) -> Result<String, R11ContractError> {
    match family {
        "coverage_gaps" => Ok(format!(
            "{}\0{path}\0{}\0{id}",
            record
                .get("code")
                .and_then(Value::as_str)
                .ok_or(R11ContractError::InvalidProjection)?,
            record
                .get("subject_id")
                .and_then(Value::as_str)
                .ok_or(R11ContractError::InvalidProjection)?
        )),
        "evidence" => match record.get("kind").and_then(Value::as_str) {
            Some("git_tree_entry") => Ok(format!("0\0{path}\0{id}")),
            Some("gitmodules_declaration") => {
                let start = record
                    .pointer("/span/start")
                    .and_then(Value::as_u64)
                    .ok_or(R11ContractError::InvalidProjection)?;
                let end = record
                    .pointer("/span/end")
                    .and_then(Value::as_u64)
                    .ok_or(R11ContractError::InvalidProjection)?;
                Ok(format!("1\0{start:020}\0{end:020}\0{id}"))
            }
            _ => Err(R11ContractError::InvalidProjection),
        },
        _ => Ok(format!("{path}\0{id}")),
    }
}

fn record_by_id<'a>(
    family: &'a Value,
    field: &str,
    id: &str,
) -> Result<&'a Value, R11ContractError> {
    family
        .as_array()
        .and_then(|records| {
            records
                .iter()
                .find(|record| record.get(field).and_then(Value::as_str) == Some(id))
        })
        .ok_or_else(|| R11ContractError::ReferenceMismatch {
            family: "repository_boundaries",
            id: id.to_owned(),
        })
}

fn valid_repository_identity(value: &str) -> bool {
    value.len() <= 255
        && value.strip_prefix("urn:codenoesis:").is_some_and(|suffix| {
            let mut bytes = suffix.bytes();
            bytes
                .next()
                .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
                && bytes.all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || matches!(byte, b'.' | b'_' | b':' | b'-')
                })
        })
}

fn valid_gitmodules_key(value: &str) -> bool {
    value.len() <= 64
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn valid_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn validate_id_references(
    value: &Value,
    field: &str,
    valid: &BTreeSet<&str>,
    family: &'static str,
) -> Result<(), R11ContractError> {
    for id in value
        .get(field)
        .and_then(Value::as_array)
        .ok_or(R11ContractError::InvalidProjection)?
    {
        let id = id.as_str().ok_or(R11ContractError::InvalidProjection)?;
        if !valid.contains(id) {
            return Err(R11ContractError::ReferenceMismatch {
                family,
                id: id.to_owned(),
            });
        }
    }
    Ok(())
}

fn map_k1_contract(error: K1ContractError) -> R11ContractError {
    match error {
        K1ContractError::InvalidSnapshot => R11ContractError::InvalidSnapshot,
        K1ContractError::UnsupportedSnapshotSchema(value) => {
            R11ContractError::UnsupportedSnapshotSchema(value)
        }
        K1ContractError::UnsupportedPortableGraphSchema(value) => {
            R11ContractError::UnsupportedPortableGraphSchema(value)
        }
        K1ContractError::Noncanonical {
            expected_sha256,
            observed_sha256,
        } => R11ContractError::Noncanonical {
            expected_sha256,
            observed_sha256,
        },
        K1ContractError::IdentityConflict { family, id } => {
            R11ContractError::IdentityConflict { family, id }
        }
        K1ContractError::ReferenceMismatch { family, id } => {
            R11ContractError::ReferenceMismatch { family, id }
        }
        K1ContractError::LimitExceeded {
            limit,
            maximum,
            observed,
        } => R11ContractError::LimitExceeded {
            limit,
            maximum,
            observed,
        },
        K1ContractError::UnsafePayload { reason } => R11ContractError::UnsafePayload { reason },
        K1ContractError::AssetIntegrityMismatch => R11ContractError::AssetIntegrityMismatch,
        K1ContractError::InvalidProjection => R11ContractError::InvalidProjection,
        K1ContractError::Internal => R11ContractError::Internal,
    }
}

fn validate_private_fields(value: &Value) -> Result<(), R11ContractError> {
    match value {
        Value::Object(fields) => {
            for (field, nested) in fields {
                if matches!(
                    field.as_str(),
                    "body_text"
                        | "expression_text"
                        | "source_contents"
                        | "source_snippet"
                        | "repository_root"
                        | "raw_url"
                        | "environment"
                        | "telemetry"
                        | "credentials"
                ) {
                    return Err(R11ContractError::UnsafePayload {
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

fn ensure_nesting(value: &Value, depth: u64) -> Result<(), R11ContractError> {
    if depth > MAX_R11_JSON_NESTING {
        return Err(R11ContractError::LimitExceeded {
            limit: "json_nesting",
            maximum: MAX_R11_JSON_NESTING,
            observed: MAX_R11_JSON_NESTING.saturating_add(1),
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

fn enforce_portable_size(length: usize) -> Result<(), R11ContractError> {
    let observed = u64::try_from(length).unwrap_or(u64::MAX);
    if observed > MAX_R11_PORTABLE_GRAPH_BYTES {
        return Err(R11ContractError::LimitExceeded {
            limit: "portable_graph_bytes",
            maximum: MAX_R11_PORTABLE_GRAPH_BYTES,
            observed: MAX_R11_PORTABLE_GRAPH_BYTES.saturating_add(1),
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

fn valid_sha256_urn(value: &str, prefix: &str) -> bool {
    value.strip_prefix(prefix).is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    })
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
    fn pt_fr_ext_012_v13_hash_domains_are_additive() {
        assert_eq!(R11_SNAPSHOT_VERSION, SNAPSHOT_SCHEMA_VERSION_V13);
        assert_eq!(
            codenoesis_domain::s4_k1::K1_INDEX_VERSION,
            "codenoesis.callable-semantics-index/v1"
        );
        assert_eq!(K1_PROFILE, "rust-callable-semantics-v1");
    }

    #[test]
    fn ct_fr_cli_001_acquisition_limit_is_promoted_to_error_v18() {
        let error = AcquisitionError::LimitExceeded {
            limit: LimitKind::CanonicalOutputBytes,
            maximum: 32,
            observed: 33,
        };
        let promoted = CodeNoesisErrorV18::from_acquisition_limit(&error)
            .expect("R11 acquisition limit is public");
        assert_eq!(promoted.value()["schema_version"], R11_ERROR_VERSION);
        assert_eq!(promoted.value()["code"], "acquisition.limit_exceeded");
        assert_eq!(promoted.value()["context"]["maximum"], 32);
        assert_eq!(promoted.value()["context"]["observed"], 33);
    }

    #[test]
    fn ct_fr_exp_002_boundary_projection_rejects_unknown_shape_and_summary_drift() {
        let mut boundaries = valid_boundary_value();
        boundaries["unexpected"] = Value::Bool(true);
        assert_eq!(
            validate_boundary_value(&boundaries),
            Err(R11ContractError::InvalidProjection)
        );

        let mut boundaries = valid_boundary_value();
        boundaries["summary"]["boundary_count"] = json!(2);
        assert_eq!(
            validate_boundary_value(&boundaries),
            Err(R11ContractError::InvalidProjection)
        );
    }

    #[test]
    fn ct_fr_exp_002_boundary_projection_accepts_declaration_subject_gaps() {
        let mut boundaries = valid_boundary_value();
        let declaration_id = boundaries["declarations"][0]["declaration_id"].clone();
        let declaration_evidence = boundaries["declarations"][0]["evidence_id"].clone();
        boundaries["coverage_gaps"][0]["code"] =
            Value::String("boundary.gitmodules_key_unsupported".to_owned());
        boundaries["coverage_gaps"][0]["subject_id"] = declaration_id;
        boundaries["coverage_gaps"][0]["evidence_ids"] = json!([declaration_evidence]);
        assert_eq!(validate_boundary_value(&boundaries), Ok(()));
    }

    fn valid_boundary_value() -> Value {
        let boundary_id = format!(
            "urn:codenoesis:repository-boundary:sha256:{}",
            "1".repeat(64)
        );
        let declaration_id = format!(
            "urn:codenoesis:gitmodules-declaration:sha256:{}",
            "2".repeat(64)
        );
        let tree_evidence_id =
            format!("urn:codenoesis:boundary-evidence:sha256:{}", "3".repeat(64));
        let declaration_evidence_id =
            format!("urn:codenoesis:boundary-evidence:sha256:{}", "4".repeat(64));
        let gap_id = format!("urn:codenoesis:boundary-gap:sha256:{}", "5".repeat(64));
        let commit_oid = "a".repeat(40);
        let tree_oid = "b".repeat(40);
        let gitlink_oid = "c".repeat(40);
        json!({
            "schema_version": "codenoesis.repository-boundaries/v1",
            "profile": LOCAL_GITLINKS_V1,
            "root_repository": {
                "identity": "urn:codenoesis:fixture:r11",
                "vcs": "git",
                "object_format": "sha1",
                "commit_oid": commit_oid,
                "tree_oid": tree_oid
            },
            "summary": {
                "boundary_count": 1,
                "declaration_count": 1,
                "bound_count": 0,
                "unbound_count": 1,
                "coverage_gap_count": 1
            },
            "boundaries": [{
                "boundary_id": boundary_id,
                "path": "vendor/example",
                "gitlink_oid": gitlink_oid,
                "state": "declared_unbound",
                "declaration_id": declaration_id,
                "nested_repository": null,
                "evidence_ids": [tree_evidence_id, declaration_evidence_id],
                "coverage_gap_ids": [gap_id]
            }],
            "declarations": [{
                "declaration_id": declaration_id,
                "name_sha256": "6".repeat(64),
                "path": "vendor/example",
                "url_kind": "https",
                "url_sha256": "7".repeat(64),
                "unsupported_keys": [],
                "boundary_id": boundary_id,
                "evidence_id": declaration_evidence_id
            }],
            "coverage_gaps": [{
                "gap_id": gap_id,
                "code": "boundary.nested_repository_unbound",
                "path": "vendor/example",
                "subject_id": boundary_id,
                "evidence_ids": [tree_evidence_id, declaration_evidence_id]
            }],
            "evidence": [
                {
                    "evidence_id": tree_evidence_id,
                    "kind": "git_tree_entry",
                    "repository": {
                        "identity": "urn:codenoesis:fixture:r11",
                        "commit_oid": commit_oid
                    },
                    "tree_oid": tree_oid,
                    "path": "vendor/example",
                    "mode": "160000",
                    "object_oid": gitlink_oid
                },
                {
                    "evidence_id": declaration_evidence_id,
                    "kind": "gitmodules_declaration",
                    "repository": {
                        "identity": "urn:codenoesis:fixture:r11",
                        "commit_oid": commit_oid
                    },
                    "blob_oid": "d".repeat(40),
                    "path": ".gitmodules",
                    "span": {"unit": "byte", "start": 0, "end": 64}
                }
            ]
        })
    }
}
