use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use codenoesis_domain::knowledge::EntityKind;
use codenoesis_domain::s4::{
    S4_ONTOLOGY_VERSION, S4_TREE_SITTER_EXTRACTOR_VERSION, S4_WORKSPACE_EXTRACTOR_VERSION,
    WorkspaceVisibility,
};
use codenoesis_domain::s5::{
    ANALYSIS_CACHE_PAYLOAD_DOMAIN, ANALYSIS_CACHE_SCHEMA_VERSION, AnalysisCacheEntry,
    AnalysisCacheKey, ChangeKind, ChangedPath, DEPENDENCY_RULE_VERSION,
    EXTRACTION_CONTRACT_VERSION, IncrementalRuleOutcome, IncrementalWorkspaceExtraction,
    InventoryBlob, MAX_ANALYSIS_ENTRIES, MAX_CHANGED_PATHS, MAX_REPORT_BYTES,
    MAX_REPORT_SUBJECT_IDS, NORMALIZATION_VERSION, RustDeclarationObservation,
    RustModuleObservation, RustSourceAnalysis, TARGET_SEMANTIC_PROFILE,
};
use codenoesis_domain::storage::{LocalSnapshotHead, StorageComponent, StorageError};
use codenoesis_domain::{AcquisitionError, InputError};
use serde_json::{Map, Value, json};

use crate::s4::{
    GeneratedDocumentationV1, RepositorySnapshotV4, validate_stored_snapshot_semantic_v4,
};

#[derive(Clone, Debug)]
pub struct CodeNoesisErrorV7 {
    value: Value,
}

impl CodeNoesisErrorV7 {
    #[must_use]
    pub fn from_input(error: InputError) -> Self {
        let code = match error {
            InputError::InvalidRepositoryIdentity => "input.invalid_repository_identity",
            InputError::InvalidRevision => "input.invalid_revision",
            InputError::InvalidProfile => "input.invalid_profile",
            InputError::InvalidStoreRoot => "input.invalid_store_root",
        };
        Self::new(code, "input", &error.to_string(), false, json!({}))
    }

    #[must_use]
    pub fn from_acquisition(error: &AcquisitionError) -> Self {
        let (code, context) = match error {
            AcquisitionError::NotGitRepository => ("acquisition.not_git_repository", json!({})),
            AcquisitionError::RevisionNotFound { revision } => (
                "acquisition.revision_not_found",
                json!({"revision": revision.as_str()}),
            ),
            AcquisitionError::RevisionNotCommit { object_oid, .. } => (
                "acquisition.revision_not_commit",
                json!({"object_oid": object_oid.as_str()}),
            ),
            AcquisitionError::ObjectMissing { object_oid, .. } => (
                "acquisition.object_missing",
                json!({"object_oid": object_oid.as_str()}),
            ),
            AcquisitionError::RepositoryInconsistent { object_oid, .. } => (
                "acquisition.repository_inconsistent",
                json!({"object_oid": object_oid.as_str()}),
            ),
            AcquisitionError::UnsupportedRepositoryShape { feature } => (
                "acquisition.unsupported_repository_shape",
                json!({"reason": feature.as_str()}),
            ),
            AcquisitionError::PathInvalid { reason } => (
                "acquisition.path_invalid",
                json!({"reason": reason.as_str()}),
            ),
            AcquisitionError::RootPolicyViolation { policy } => (
                "acquisition.root_policy_violation",
                json!({"reason": policy.as_str()}),
            ),
            AcquisitionError::EntryPolicyViolation { path, entry } => (
                "acquisition.entry_policy_violation",
                json!({"path": path, "reason": entry.as_str()}),
            ),
            AcquisitionError::LimitExceeded { .. } => ("acquisition.limit_exceeded", json!({})),
        };
        Self::new(code, "acquisition", &error.to_string(), false, context)
    }

    #[must_use]
    pub fn from_workspace(error: &codenoesis_domain::s4::WorkspaceError) -> Self {
        let (code, context) = match error {
            codenoesis_domain::s4::WorkspaceError::InvalidUtf8 { path } => {
                ("extraction.invalid_utf8", json!({"path": path}))
            }
            codenoesis_domain::s4::WorkspaceError::UnsupportedWorkspace => {
                ("extraction.unsupported_workspace", json!({}))
            }
            codenoesis_domain::s4::WorkspaceError::AmbiguousModule => {
                ("extraction.ambiguous_module", json!({}))
            }
            codenoesis_domain::s4::WorkspaceError::ParserCancelled { path } => {
                ("extraction.parser_cancelled", json!({"path": path}))
            }
            codenoesis_domain::s4::WorkspaceError::MalformedSyntax { path } => {
                ("extraction.malformed_syntax", json!({"path": path}))
            }
            codenoesis_domain::s4::WorkspaceError::ContractInvalid => {
                ("extraction.contract_invalid", json!({}))
            }
            codenoesis_domain::s4::WorkspaceError::LimitExceeded { .. } => {
                ("extraction.limit_exceeded", json!({}))
            }
        };
        Self::new(code, "extraction", &error.to_string(), false, context)
    }

    #[must_use]
    pub fn from_storage(error: &StorageError) -> Self {
        match error {
            StorageError::UnmarkedNonemptyRoot => Self::new(
                "storage.unmarked_nonempty_root",
                "storage",
                &error.to_string(),
                false,
                json!({}),
            ),
            StorageError::IncompatibleSchema { observed_schema } => Self::new(
                "storage.incompatible_schema",
                "storage",
                &error.to_string(),
                false,
                json!({
                    "expected_version": "codenoesis.local-store/v1",
                    "observed_version": safe_version(observed_schema)
                }),
            ),
            StorageError::WriterBusy => Self::new(
                "storage.writer_busy",
                "storage",
                &error.to_string(),
                true,
                json!({}),
            ),
            StorageError::MissingObject { .. } => Self::new(
                "storage.missing_object",
                "storage",
                &error.to_string(),
                false,
                json!({}),
            ),
            StorageError::CorruptObject {
                expected_hash,
                observed_hash,
                ..
            } => Self::new(
                "storage.corrupt_object",
                "storage",
                &error.to_string(),
                false,
                json!({
                    "expected_hash": safe_digest(expected_hash),
                    "observed_hash": safe_digest(observed_hash)
                }),
            ),
            StorageError::CorruptMetadata { reason, .. } => Self::new(
                "storage.corrupt_metadata",
                "storage",
                &error.to_string(),
                false,
                json!({"reason": reason}),
            ),
            StorageError::UnsafePath { reason } => Self::new(
                "storage.unsafe_path",
                "storage",
                &error.to_string(),
                false,
                json!({"reason": reason}),
            ),
            StorageError::HeadConflict { expected, actual } => {
                let mut context = Map::new();
                if let Some(expected) = expected {
                    context.insert(
                        "expected_snapshot_id".to_owned(),
                        Value::String(expected.clone()),
                    );
                }
                if let Some(actual) = actual {
                    context.insert(
                        "actual_snapshot_id".to_owned(),
                        Value::String(actual.clone()),
                    );
                }
                Self::new(
                    "publication.head_conflict",
                    "publication",
                    &error.to_string(),
                    true,
                    Value::Object(context),
                )
            }
            StorageError::PublicationFailed => Self::new(
                "publication.failed",
                "publication",
                &error.to_string(),
                false,
                json!({}),
            ),
        }
    }

    #[must_use]
    pub fn baseline_missing(expected_repository_identity: &str) -> Self {
        Self::new(
            "incremental.baseline_missing",
            "incremental",
            "validated visible S4 baseline is missing",
            false,
            json!({
                "component": "baseline_head",
                "expected_repository_identity": safe_repository_identity(
                    expected_repository_identity
                )
            }),
        )
    }

    #[must_use]
    pub fn baseline_repository_mismatch(
        expected_repository_identity: &str,
        actual_repository_identity: &str,
    ) -> Self {
        Self::new(
            "incremental.baseline_repository_mismatch",
            "incremental",
            "baseline repository identity does not match the request",
            false,
            json!({
                "component": "baseline_head",
                "expected_repository_identity": safe_repository_identity(
                    expected_repository_identity
                ),
                "actual_repository_identity": safe_repository_identity(
                    actual_repository_identity
                )
            }),
        )
    }

    #[must_use]
    pub fn baseline_incompatible(expected_version: &str, observed_version: &str) -> Self {
        Self::new(
            "incremental.baseline_incompatible",
            "incremental",
            "visible baseline is not compatible with standard-local-s5",
            false,
            json!({
                "component": "baseline_snapshot",
                "expected_version": safe_version(expected_version),
                "observed_version": safe_version(observed_version)
            }),
        )
    }

    #[must_use]
    pub fn cache_corrupt(path: &str, expected_hash: &str, observed_hash: &str) -> Self {
        Self::new(
            "incremental.cache_corrupt",
            "incremental",
            "current analysis cache entry failed validation",
            false,
            json!({
                "component": "analysis_cache",
                "path": safe_path(path),
                "expected_hash": safe_digest(expected_hash),
                "observed_hash": safe_digest(observed_hash)
            }),
        )
    }

    #[must_use]
    pub fn limit_exceeded(limit: &str, maximum: u64, observed: u64) -> Self {
        Self::new(
            "incremental.limit_exceeded",
            "incremental",
            "incremental refresh limit exceeded",
            false,
            json!({
                "component": limit,
                "limit": limit,
                "maximum": maximum.min(16_777_217),
                "observed": observed.clamp(1, 16_777_217)
            }),
        )
    }

    #[must_use]
    pub fn cold_equivalence_failed(
        component: &str,
        expected_hash: &str,
        observed_hash: &str,
    ) -> Self {
        Self::new(
            "incremental.cold_equivalence_failed",
            "incremental",
            "incrementally composed target differs from the cold-equivalent target",
            false,
            json!({
                "component": component,
                "expected_hash": safe_digest(expected_hash),
                "observed_hash": safe_digest(observed_hash)
            }),
        )
    }

    #[must_use]
    pub fn internal() -> Self {
        Self::new(
            "internal.unexpected",
            "internal",
            "unexpected internal failure",
            false,
            json!({}),
        )
    }

    /// Serializes one strict V7 error followed by one LF.
    ///
    /// # Errors
    ///
    /// Returns a serialization error only if the internal JSON is invalid.
    pub fn canonical_stderr(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = serde_json::to_vec(&self.value)?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    fn new(code: &str, stage: &str, message: &str, retryable: bool, context: Value) -> Self {
        let mut value = Map::new();
        value.insert(
            "schema_version".to_owned(),
            Value::String("codenoesis.error/v7".to_owned()),
        );
        value.insert("code".to_owned(), Value::String(code.to_owned()));
        value.insert("stage".to_owned(), Value::String(stage.to_owned()));
        value.insert("message".to_owned(), Value::String(message.to_owned()));
        value.insert("retryable".to_owned(), Value::Bool(retryable));
        value.insert("context".to_owned(), context);
        Self {
            value: Value::Object(value),
        }
    }
}

#[derive(Clone, Debug)]
pub struct AnalysisCacheEntryV1 {
    value: Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnalysisCacheEntryV1Error {
    InvalidJson,
    InvalidContract,
    InvalidIdentity,
    InvalidPayloadHash,
}

impl Display for AnalysisCacheEntryV1Error {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidJson => "invalid analysis cache JSON",
            Self::InvalidContract => "invalid analysis cache contract",
            Self::InvalidIdentity => "invalid analysis cache identity",
            Self::InvalidPayloadHash => "invalid analysis cache payload hash",
        })
    }
}

impl Error for AnalysisCacheEntryV1Error {}

impl AnalysisCacheEntryV1 {
    #[must_use]
    pub fn from_domain(entry: &AnalysisCacheEntry) -> Self {
        let mut observations = ObservationWriter::new(&entry.key);
        observations.write_root(&entry.analysis);
        let payload = json!({
            "schema_version": ANALYSIS_CACHE_SCHEMA_VERSION,
            "repository_identity": entry.key.repository_identity,
            "source": {
                "source_file_id": entry.key.source_file_id,
                "path": entry.key.canonical_source_path,
                "blob_oid": entry.key.source_blob_oid,
                "crate_id": entry.key.crate_id,
                "module_path": entry.key.canonical_module_path
            },
            "versions": {
                "language_extractor": entry.key.language_extractor,
                "workspace_mapper": entry.key.workspace_mapper,
                "normalization": NORMALIZATION_VERSION,
                "ontology": entry.key.ontology,
                "extraction_contract": EXTRACTION_CONTRACT_VERSION,
                "semantic_profile": TARGET_SEMANTIC_PROFILE,
                "dependency_rules": DEPENDENCY_RULE_VERSION
            },
            "observations": {
                "spans": [],
                "entities": observations.entities,
                "relationships": [],
                "coverage": [],
                "diagnostics": []
            }
        });
        let payload_hash = payload_hash(&payload);
        let mut value = payload.as_object().cloned().unwrap_or_default();
        value.insert(
            "analysis_cache_entry_id".to_owned(),
            Value::String(entry.analysis_cache_entry_id.clone()),
        );
        value.insert("payload_hash".to_owned(), Value::String(payload_hash));
        Self {
            value: Value::Object(value),
        }
    }

    /// Parses and validates one closed current-version cache entry.
    ///
    /// # Errors
    ///
    /// Returns a typed failure for malformed JSON, unknown/missing fields,
    /// identity mismatch, or payload-hash mismatch.
    pub fn parse(bytes: &[u8]) -> Result<Self, AnalysisCacheEntryV1Error> {
        let value: Value =
            serde_json::from_slice(bytes).map_err(|_| AnalysisCacheEntryV1Error::InvalidJson)?;
        let contract = Self { value };
        let domain = contract.to_domain()?;
        if !domain.is_self_consistent() {
            return Err(AnalysisCacheEntryV1Error::InvalidIdentity);
        }
        Ok(contract)
    }

    /// Converts a validated cache document into revision-neutral domain data.
    ///
    /// # Errors
    ///
    /// Returns a typed closed-contract, identity, or hash failure.
    #[allow(clippy::too_many_lines)]
    pub fn to_domain(&self) -> Result<AnalysisCacheEntry, AnalysisCacheEntryV1Error> {
        let object = exact_object(
            &self.value,
            &[
                "analysis_cache_entry_id",
                "observations",
                "payload_hash",
                "repository_identity",
                "schema_version",
                "source",
                "versions",
            ],
        )?;
        required_const(object, "schema_version", ANALYSIS_CACHE_SCHEMA_VERSION)?;
        let repository_identity = required_string(object, "repository_identity")?;
        if !valid_repository_identity(&repository_identity) {
            return Err(AnalysisCacheEntryV1Error::InvalidContract);
        }
        let source = exact_object(
            required_value(object, "source")?,
            &[
                "blob_oid",
                "crate_id",
                "module_path",
                "path",
                "source_file_id",
            ],
        )?;
        let versions = exact_object(
            required_value(object, "versions")?,
            &[
                "dependency_rules",
                "extraction_contract",
                "language_extractor",
                "normalization",
                "ontology",
                "semantic_profile",
                "workspace_mapper",
            ],
        )?;
        required_const(
            versions,
            "language_extractor",
            S4_TREE_SITTER_EXTRACTOR_VERSION,
        )?;
        required_const(versions, "workspace_mapper", S4_WORKSPACE_EXTRACTOR_VERSION)?;
        required_const(versions, "normalization", NORMALIZATION_VERSION)?;
        required_const(versions, "ontology", S4_ONTOLOGY_VERSION)?;
        required_const(versions, "extraction_contract", EXTRACTION_CONTRACT_VERSION)?;
        required_const(versions, "semantic_profile", TARGET_SEMANTIC_PROFILE)?;
        required_const(versions, "dependency_rules", DEPENDENCY_RULE_VERSION)?;
        let source_file_id = required_string(source, "source_file_id")?;
        let canonical_source_path = required_string(source, "path")?;
        let source_blob_oid = required_string(source, "blob_oid")?;
        let crate_id = required_string(source, "crate_id")?;
        let canonical_module_path = required_string(source, "module_path")?;
        if !valid_entity_id(&source_file_id)
            || !valid_safe_path(&canonical_source_path)
            || !valid_git_oid(&source_blob_oid)
            || !valid_entity_id(&crate_id)
            || !valid_module_path(&canonical_module_path)
        {
            return Err(AnalysisCacheEntryV1Error::InvalidContract);
        }
        let key = AnalysisCacheKey {
            repository_identity,
            source_file_id,
            canonical_source_path,
            source_blob_oid,
            crate_id,
            canonical_module_path,
            language_extractor: required_string(versions, "language_extractor")?,
            workspace_mapper: required_string(versions, "workspace_mapper")?,
            ontology: required_string(versions, "ontology")?,
        };
        let expected_id = key.entry_id();
        let observed_id = required_string(object, "analysis_cache_entry_id")?;
        if expected_id != observed_id {
            return Err(AnalysisCacheEntryV1Error::InvalidIdentity);
        }
        let observed_hash = required_string(object, "payload_hash")?;
        if !valid_digest(&observed_hash) {
            return Err(AnalysisCacheEntryV1Error::InvalidPayloadHash);
        }
        let observations = exact_object(
            required_value(object, "observations")?,
            &[
                "coverage",
                "diagnostics",
                "entities",
                "relationships",
                "spans",
            ],
        )?;
        for empty in ["spans", "relationships", "coverage", "diagnostics"] {
            if required_array(observations, empty)?.is_empty() {
                continue;
            }
            return Err(AnalysisCacheEntryV1Error::InvalidContract);
        }
        let entities = required_array(observations, "entities")?;
        if entities.len() > 100_000 {
            return Err(AnalysisCacheEntryV1Error::InvalidContract);
        }
        let analysis = ObservationReader::new(
            entities,
            &key.canonical_source_path,
            &key.canonical_module_path,
        )?
        .read_root()?;
        let mut payload = self.value.clone();
        let payload_object = payload
            .as_object_mut()
            .ok_or(AnalysisCacheEntryV1Error::InvalidContract)?;
        payload_object.remove("analysis_cache_entry_id");
        payload_object.remove("payload_hash");
        if payload_hash(&payload) != observed_hash {
            return Err(AnalysisCacheEntryV1Error::InvalidPayloadHash);
        }
        Ok(AnalysisCacheEntry {
            analysis_cache_entry_id: observed_id,
            key,
            analysis,
        })
    }

    /// Serializes one canonical cache document.
    ///
    /// # Errors
    ///
    /// Returns a JSON serialization failure only for an invalid in-memory value.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(&self.value)
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }
}

#[derive(Clone, Debug)]
pub struct ValidatedS4Head {
    semantic: Value,
    head: LocalSnapshotHead,
}

impl ValidatedS4Head {
    /// Creates one strict baseline from already loaded semantic bytes and the
    /// complete validated visible head.
    ///
    /// # Errors
    ///
    /// Returns a typed S4 storage-integrity failure when any binding differs.
    pub fn new(semantic: Value, head: LocalSnapshotHead) -> Result<Self, StorageError> {
        validate_stored_snapshot_semantic_v4(&semantic, &head)?;
        Ok(Self { semantic, head })
    }

    #[must_use]
    pub const fn semantic(&self) -> &Value {
        &self.semantic
    }

    #[must_use]
    pub const fn head(&self) -> &LocalSnapshotHead {
        &self.head
    }

    #[must_use]
    pub fn supports_current_s5_versions(&self) -> bool {
        self.semantic
            .pointer("/configuration/profile")
            .and_then(Value::as_str)
            == Some(TARGET_SEMANTIC_PROFILE)
            && self
                .semantic
                .get("pipeline_version")
                .and_then(Value::as_str)
                == Some("codenoesis.pipeline/s4-v1")
            && self
                .semantic
                .get("ontology_version")
                .and_then(Value::as_str)
                == Some(S4_ONTOLOGY_VERSION)
            && self
                .semantic
                .get("extractor_contract_version")
                .and_then(Value::as_str)
                == Some(EXTRACTION_CONTRACT_VERSION)
            && self
                .semantic
                .get("evidence_lineage_version")
                .and_then(Value::as_str)
                == Some("codenoesis.evidence-lineage/v2")
            && exact_string_array(
                self.semantic.get("extractor_versions"),
                &[
                    "codenoesis.inventory-classifier/s1-v1",
                    S4_TREE_SITTER_EXTRACTOR_VERSION,
                    S4_WORKSPACE_EXTRACTOR_VERSION,
                ],
            )
            && self
                .semantic
                .get("extraction_chunks")
                .and_then(Value::as_array)
                .is_some_and(|chunks| {
                    chunks.iter().all(|chunk| {
                        chunk.get("schema_version").and_then(Value::as_str)
                            == Some("codenoesis.extraction-chunk/v2")
                            && chunk.get("ontology_version").and_then(Value::as_str)
                                == Some(S4_ONTOLOGY_VERSION)
                    })
                })
            && self.semantic.get("knowledge_graph").is_some_and(|graph| {
                graph.get("schema_version").and_then(Value::as_str)
                    == Some("codenoesis.knowledge-graph/v2")
                    && graph.get("ontology_version").and_then(Value::as_str)
                        == Some(S4_ONTOLOGY_VERSION)
                    && exact_string_array(
                        graph.get("extractor_versions"),
                        &[
                            S4_TREE_SITTER_EXTRACTOR_VERSION,
                            S4_WORKSPACE_EXTRACTOR_VERSION,
                        ],
                    )
            })
    }

    /// Returns the exact baseline path/blob inventory needed for an in-process
    /// target-tree comparison.
    ///
    /// # Errors
    ///
    /// Returns a typed metadata-integrity failure for malformed or duplicate
    /// stored inventory records.
    pub fn inventory_blobs(&self) -> Result<Vec<InventoryBlob>, StorageError> {
        let files = self
            .semantic
            .get("inventory")
            .and_then(|inventory| inventory.get("files"))
            .and_then(Value::as_array)
            .ok_or_else(|| baseline_corrupt(&self.head, "stored_inventory_invalid"))?;
        let mut inventory = files
            .iter()
            .map(|file| {
                Ok(InventoryBlob {
                    path: file
                        .get("path")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                        .ok_or_else(|| baseline_corrupt(&self.head, "stored_inventory_invalid"))?,
                    blob_oid: file
                        .get("blob_oid")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                        .ok_or_else(|| baseline_corrupt(&self.head, "stored_inventory_invalid"))?,
                    mode: file
                        .get("mode")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                        .ok_or_else(|| baseline_corrupt(&self.head, "stored_inventory_invalid"))?,
                })
            })
            .collect::<Result<Vec<_>, StorageError>>()?;
        inventory.sort_by(|left, right| left.path.as_bytes().cmp(right.path.as_bytes()));
        if inventory
            .windows(2)
            .any(|pair| pair[0].path == pair[1].path)
        {
            return Err(baseline_corrupt(&self.head, "stored_inventory_duplicate"));
        }
        Ok(inventory)
    }
}

pub struct IncrementalRefreshReportInput<'a> {
    pub baseline: &'a ValidatedS4Head,
    pub target: &'a RepositorySnapshotV4,
    pub baseline_documentation: &'a GeneratedDocumentationV1,
    pub target_documentation: &'a GeneratedDocumentationV1,
    pub changed_paths: &'a [ChangedPath],
    pub baseline_cache_entries: &'a [AnalysisCacheEntry],
    pub target_extraction: &'a IncrementalWorkspaceExtraction,
    pub rule: IncrementalRuleOutcome,
}

#[derive(Clone, Debug)]
pub struct IncrementalRefreshReportV1 {
    value: Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IncrementalRefreshReportError {
    InvalidBaseline,
    InvalidTarget,
    InvalidDocumentation,
    InvalidAnalysis,
    LimitExceeded {
        limit: &'static str,
        maximum: usize,
        observed: usize,
    },
    Serialization,
}

impl Display for IncrementalRefreshReportError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidBaseline => "invalid incremental baseline",
            Self::InvalidTarget => "invalid incremental target",
            Self::InvalidDocumentation => "invalid incremental documentation",
            Self::InvalidAnalysis => "invalid incremental analysis",
            Self::LimitExceeded { .. } => "incremental report limit exceeded",
            Self::Serialization => "incremental report serialization failed",
        })
    }
}

impl Error for IncrementalRefreshReportError {}

impl IncrementalRefreshReportV1 {
    /// Builds the deterministic semantic S5 report before publication.
    ///
    /// # Errors
    ///
    /// Returns a strict baseline, target, analysis, documentation, or limit
    /// failure without creating partial report bytes.
    #[allow(clippy::too_many_lines)]
    pub fn new(
        input: &IncrementalRefreshReportInput<'_>,
    ) -> Result<Self, IncrementalRefreshReportError> {
        let baseline_semantic = input.baseline.semantic();
        let target_semantic = input
            .target
            .value()
            .get("semantic")
            .ok_or(IncrementalRefreshReportError::InvalidTarget)?;
        let target_candidate = input
            .target
            .publication_candidate()
            .map_err(|_| IncrementalRefreshReportError::InvalidTarget)?;
        let repository_identity = semantic_string(
            baseline_semantic,
            &["repository", "identity"],
            IncrementalRefreshReportError::InvalidBaseline,
        )?;
        if repository_identity
            != semantic_string(
                target_semantic,
                &["repository", "identity"],
                IncrementalRefreshReportError::InvalidTarget,
            )?
            || input.baseline.head().repository_identity.as_str() != repository_identity
        {
            return Err(IncrementalRefreshReportError::InvalidTarget);
        }
        if input.changed_paths.len() > MAX_CHANGED_PATHS {
            return Err(IncrementalRefreshReportError::LimitExceeded {
                limit: "changed_paths",
                maximum: MAX_CHANGED_PATHS,
                observed: input.changed_paths.len(),
            });
        }
        let analysis_entry_count = input
            .baseline_cache_entries
            .len()
            .max(input.target_extraction.cache_entries.len());
        if analysis_entry_count > MAX_ANALYSIS_ENTRIES {
            return Err(IncrementalRefreshReportError::LimitExceeded {
                limit: "analysis_entries",
                maximum: MAX_ANALYSIS_ENTRIES,
                observed: analysis_entry_count,
            });
        }
        validate_changed_paths(input.changed_paths)?;

        let no_change = input.rule == IncrementalRuleOutcome::NoChange;
        let validated_baseline_ids =
            validated_cache_ids(input.baseline_cache_entries, &repository_identity)?;
        let target_ids =
            validated_cache_ids(&input.target_extraction.cache_entries, &repository_identity)?;
        validate_source_records(input.target_extraction, &target_ids)?;
        let baseline_ids = if no_change {
            target_ids.clone()
        } else {
            validated_baseline_ids
        };
        let reused_ids = if no_change {
            target_ids.clone()
        } else {
            input
                .target_extraction
                .source_records
                .iter()
                .filter(|record| record.reused)
                .map(|record| record.analysis_cache_entry_id.clone())
                .collect::<BTreeSet<_>>()
        };
        let recomputed_ids = if no_change {
            BTreeSet::new()
        } else {
            input
                .target_extraction
                .source_records
                .iter()
                .filter(|record| !record.reused)
                .map(|record| record.analysis_cache_entry_id.clone())
                .collect::<BTreeSet<_>>()
        };
        let invalidated_ids = baseline_ids
            .difference(&target_ids)
            .cloned()
            .collect::<BTreeSet<_>>();
        if reused_ids.len() + recomputed_ids.len() != target_ids.len() {
            return Err(IncrementalRefreshReportError::InvalidAnalysis);
        }

        let baseline_documents = document_index(input.baseline_documentation)?;
        let target_documents = document_index(input.target_documentation)?;
        let baseline_revision = revision_artifacts(
            baseline_semantic,
            input.baseline.head().snapshot_id.as_str(),
            &input.baseline.head().semantic_hash.value,
            &input.baseline.head().graph_semantic_hash.value,
            input.baseline_documentation,
            IncrementalRefreshReportError::InvalidBaseline,
        )?;
        let target_revision = revision_artifacts(
            target_semantic,
            target_candidate.snapshot.snapshot_id.as_str(),
            &target_candidate.snapshot.semantic_hash.value,
            &target_candidate.snapshot.graph_semantic_hash.value,
            input.target_documentation,
            IncrementalRefreshReportError::InvalidTarget,
        )?;
        let head_advanced =
            input.baseline.head().snapshot_id != target_candidate.snapshot.snapshot_id;
        if no_change && (head_advanced || !input.changed_paths.is_empty()) {
            return Err(IncrementalRefreshReportError::InvalidTarget);
        }
        let chunks = if no_change {
            Vec::new()
        } else {
            chunk_rematerialization(
                baseline_semantic,
                target_semantic,
                &input.target_extraction.source_records,
            )?
        };
        let documents = if no_change {
            Vec::new()
        } else {
            document_rematerialization(&baseline_documents, &target_documents)?
        };
        let invalidation = invalidation_value(
            baseline_semantic,
            target_semantic,
            &baseline_documents,
            &target_documents,
            !no_change,
        )?;
        let changed_path_values = input
            .changed_paths
            .iter()
            .map(|change| {
                json!({
                    "path": change.path,
                    "change_kind": change.change_kind.as_str(),
                    "baseline_blob_oid": change.baseline_blob_oid,
                    "target_blob_oid": change.target_blob_oid
                })
            })
            .collect::<Vec<_>>();
        let changed_path_set = input
            .changed_paths
            .iter()
            .map(|change| change.path.as_str())
            .collect::<BTreeSet<_>>();
        let target_inventory_paths = semantic_array(
            target_semantic,
            &["inventory", "files"],
            IncrementalRefreshReportError::InvalidTarget,
        )?
        .iter()
        .map(|file| {
            file.get("path")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or(IncrementalRefreshReportError::InvalidTarget)
        })
        .collect::<Result<Vec<_>, _>>()?;
        let reused_classification_paths = target_inventory_paths
            .iter()
            .filter(|path| !changed_path_set.contains(path.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        let reclassified_paths = input
            .changed_paths
            .iter()
            .map(|change| change.path.clone())
            .collect::<Vec<_>>();
        let changed_document_count =
            changed_document_content_count(&baseline_documents, &target_documents)?;
        let invalidation_subject_count = identity_list_count(&invalidation)?;
        let analysis_subject_count = baseline_ids.len()
            + target_ids.len()
            + reused_ids.len()
            + invalidated_ids.len()
            + recomputed_ids.len();
        let subject_count = invalidation_subject_count
            .checked_add(analysis_subject_count)
            .ok_or(IncrementalRefreshReportError::LimitExceeded {
                limit: "report_subject_ids",
                maximum: MAX_REPORT_SUBJECT_IDS,
                observed: MAX_REPORT_SUBJECT_IDS.saturating_add(1),
            })?;
        if subject_count > MAX_REPORT_SUBJECT_IDS {
            return Err(IncrementalRefreshReportError::LimitExceeded {
                limit: "report_subject_ids",
                maximum: MAX_REPORT_SUBJECT_IDS,
                observed: subject_count,
            });
        }
        let target_snapshot_hash = target_candidate.snapshot.semantic_hash.value.clone();
        let target_graph_hash = target_candidate.snapshot.graph_semantic_hash.value.clone();
        let target_documentation_hash = generation_hash(input.target_documentation)?;
        let mut value = json!({
            "schema_version": "codenoesis.incremental-refresh-report/v1",
            "refresh_profile": "standard-local-s5",
            "target_semantic_profile": TARGET_SEMANTIC_PROFILE,
            "repository_identity": repository_identity,
            "target_configuration_hash": hash_value("abbf68de36374ed553f32a6b4560e9dc08b4485c709ad4eb9eaddac0f2fc2e34"),
            "versions": report_versions(),
            "rule": {
                "outcome": input.rule.as_str(),
                "rule_ids": [input.rule.rule_id()],
                "reasons": [input.rule.reason()]
            },
            "baseline": baseline_revision,
            "target": target_revision,
            "changed_paths": changed_path_values,
            "analysis": {
                "baseline_entry_ids": set_values(&baseline_ids),
                "target_entry_ids": set_values(&target_ids),
                "reused_entry_ids": set_values(&reused_ids),
                "invalidated_entry_ids": set_values(&invalidated_ids),
                "recomputed_entry_ids": set_values(&recomputed_ids)
            },
            "inventory": {
                "reused_classification_paths": reused_classification_paths,
                "reclassified_paths": reclassified_paths
            },
            "public_rematerialization": {
                "all_target_evidence_uses_target_commit": true,
                "baseline_public_chunk_copy_permitted": false,
                "chunks": chunks,
                "documents": documents
            },
            "invalidation": invalidation,
            "cold_equivalence": {
                "semantic_bytes_equal": true,
                "snapshot": hash_equivalence(&target_snapshot_hash),
                "graph": hash_equivalence(&target_graph_hash),
                "documentation": hash_equivalence(&target_documentation_hash)
            },
            "publication": {
                "previous_head_snapshot_id": input.baseline.head().snapshot_id.as_str(),
                "new_head_snapshot_id": target_candidate.snapshot.snapshot_id.as_str(),
                "head_advanced": head_advanced
            },
            "metrics": {
                "changed_path_count": input.changed_paths.len(),
                "analysis_entry_count": target_ids.len(),
                "cache_hit_count": reused_ids.len(),
                "cache_miss_count": recomputed_ids.len(),
                "cache_invalidated_count": invalidated_ids.len(),
                "parser_invocation_count": if no_change {
                    0
                } else {
                    input.target_extraction.parser_invocation_count
                },
                "dependency_edge_count": 0,
                "rematerialized_chunk_count": if no_change {
                    0
                } else {
                    input.target_extraction.knowledge.extraction_chunks.len()
                },
                "rematerialized_document_manifest_count": if no_change {
                    0
                } else {
                    target_documents.len()
                },
                "changed_document_content_count": changed_document_count,
                "report_subject_id_count": subject_count
            }
        });
        let semantic_hash = report_semantic_hash(&value)?;
        value
            .as_object_mut()
            .ok_or(IncrementalRefreshReportError::Serialization)?
            .insert("semantic_hash".to_owned(), hash_value(&semantic_hash));
        let report = Self { value };
        let report_bytes = report
            .canonical_stdout()
            .map_err(|_| IncrementalRefreshReportError::Serialization)?
            .len();
        if report_bytes > MAX_REPORT_BYTES {
            return Err(IncrementalRefreshReportError::LimitExceeded {
                limit: "report_bytes",
                maximum: MAX_REPORT_BYTES,
                observed: report_bytes,
            });
        }
        Ok(report)
    }

    /// Serializes one canonical report plus its single transport LF.
    ///
    /// # Errors
    ///
    /// Returns a JSON serialization failure for invalid in-memory data.
    pub fn canonical_stdout(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = serde_json::to_vec(&self.value)?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }
}

fn validate_changed_paths(
    changed_paths: &[ChangedPath],
) -> Result<(), IncrementalRefreshReportError> {
    if changed_paths
        .windows(2)
        .any(|pair| pair[0].path.as_bytes() >= pair[1].path.as_bytes())
        || changed_paths.iter().any(|change| {
            !valid_safe_path(&change.path)
                || match change.change_kind {
                    ChangeKind::Added => {
                        change.baseline_blob_oid.is_some()
                            || change
                                .target_blob_oid
                                .as_deref()
                                .is_none_or(|oid| !valid_git_oid(oid))
                    }
                    ChangeKind::Modified => {
                        change
                            .baseline_blob_oid
                            .as_deref()
                            .is_none_or(|oid| !valid_git_oid(oid))
                            || change
                                .target_blob_oid
                                .as_deref()
                                .is_none_or(|oid| !valid_git_oid(oid))
                    }
                    ChangeKind::Deleted => {
                        change
                            .baseline_blob_oid
                            .as_deref()
                            .is_none_or(|oid| !valid_git_oid(oid))
                            || change.target_blob_oid.is_some()
                    }
                }
        })
    {
        return Err(IncrementalRefreshReportError::InvalidAnalysis);
    }
    Ok(())
}

fn validated_cache_ids(
    entries: &[AnalysisCacheEntry],
    repository_identity: &str,
) -> Result<BTreeSet<String>, IncrementalRefreshReportError> {
    let mut identities = BTreeSet::new();
    for entry in entries {
        if entry.key.repository_identity != repository_identity
            || AnalysisCacheEntryV1::from_domain(entry)
                .to_domain()
                .as_ref()
                != Ok(entry)
            || !identities.insert(entry.analysis_cache_entry_id.clone())
        {
            return Err(IncrementalRefreshReportError::InvalidAnalysis);
        }
    }
    Ok(identities)
}

fn validate_source_records(
    extraction: &IncrementalWorkspaceExtraction,
    target_ids: &BTreeSet<String>,
) -> Result<(), IncrementalRefreshReportError> {
    let entries = extraction
        .cache_entries
        .iter()
        .map(|entry| (entry.analysis_cache_entry_id.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let mut record_ids = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut source_file_ids = BTreeSet::new();
    for record in &extraction.source_records {
        let entry = entries
            .get(record.analysis_cache_entry_id.as_str())
            .ok_or(IncrementalRefreshReportError::InvalidAnalysis)?;
        if entry.key.canonical_source_path != record.path
            || entry.key.source_file_id != record.source_file_id
            || !record_ids.insert(record.analysis_cache_entry_id.clone())
            || !paths.insert(record.path.clone())
            || !source_file_ids.insert(record.source_file_id.clone())
        {
            return Err(IncrementalRefreshReportError::InvalidAnalysis);
        }
    }
    if &record_ids != target_ids {
        return Err(IncrementalRefreshReportError::InvalidAnalysis);
    }
    Ok(())
}

fn report_versions() -> Value {
    json!({
        "cache_schema": ANALYSIS_CACHE_SCHEMA_VERSION,
        "language_extractor": S4_TREE_SITTER_EXTRACTOR_VERSION,
        "workspace_mapper": S4_WORKSPACE_EXTRACTOR_VERSION,
        "normalization": NORMALIZATION_VERSION,
        "ontology": S4_ONTOLOGY_VERSION,
        "extraction_contract": EXTRACTION_CONTRACT_VERSION,
        "chunk_schema": "codenoesis.extraction-chunk/v2",
        "graph_schema": "codenoesis.knowledge-graph/v2",
        "snapshot_schema": "codenoesis.repository-snapshot/v4",
        "pipeline": "codenoesis.pipeline/s4-v1",
        "evidence_lineage": "codenoesis.evidence-lineage/v2",
        "renderer": "codenoesis.renderer/markdown-v1",
        "dependency_rules": DEPENDENCY_RULE_VERSION
    })
}

fn revision_artifacts(
    semantic: &Value,
    snapshot_id: &str,
    snapshot_semantic_hash: &str,
    graph_semantic_hash: &str,
    documentation: &GeneratedDocumentationV1,
    error: IncrementalRefreshReportError,
) -> Result<Value, IncrementalRefreshReportError> {
    Ok(json!({
        "commit_oid": semantic_string(semantic, &["repository", "commit_oid"], error)?,
        "tree_oid": semantic_string(semantic, &["repository", "tree_oid"], error)?,
        "snapshot_id": snapshot_id,
        "snapshot_semantic_hash": hash_value(snapshot_semantic_hash),
        "graph_semantic_hash": hash_value(graph_semantic_hash),
        "documentation_generation_hash": hash_value(&generation_hash(documentation)?)
    }))
}

fn generation_hash(
    documentation: &GeneratedDocumentationV1,
) -> Result<String, IncrementalRefreshReportError> {
    semantic_string(
        documentation.manifest(),
        &["generation_hash", "value"],
        IncrementalRefreshReportError::InvalidDocumentation,
    )
}

fn document_index(
    documentation: &GeneratedDocumentationV1,
) -> Result<BTreeMap<String, Value>, IncrementalRefreshReportError> {
    value_index(
        semantic_array(
            documentation.manifest(),
            &["documents"],
            IncrementalRefreshReportError::InvalidDocumentation,
        )?,
        "document_id",
        IncrementalRefreshReportError::InvalidDocumentation,
    )
}

fn chunk_rematerialization(
    baseline: &Value,
    target: &Value,
    records: &[codenoesis_domain::s5::SourceAnalysisRecord],
) -> Result<Vec<Value>, IncrementalRefreshReportError> {
    let baseline_chunks = value_index(
        semantic_array(
            baseline,
            &["extraction_chunks"],
            IncrementalRefreshReportError::InvalidBaseline,
        )?,
        "source_file_id",
        IncrementalRefreshReportError::InvalidBaseline,
    )?;
    let target_chunks = value_index(
        semantic_array(
            target,
            &["extraction_chunks"],
            IncrementalRefreshReportError::InvalidTarget,
        )?,
        "source_file_id",
        IncrementalRefreshReportError::InvalidTarget,
    )?;
    let actions = records
        .iter()
        .map(|record| (record.source_file_id.as_str(), record.reused))
        .collect::<BTreeMap<_, _>>();
    let mut rematerialized = Vec::new();
    for (source_file_id, target_chunk) in target_chunks {
        let Some(baseline_chunk) = baseline_chunks.get(&source_file_id) else {
            continue;
        };
        let reused = actions
            .get(source_file_id.as_str())
            .copied()
            .ok_or(IncrementalRefreshReportError::InvalidAnalysis)?;
        rematerialized.push(json!({
            "source_file_id": source_file_id,
            "analysis_action": if reused { "reused" } else { "recomputed" },
            "baseline_semantic_hash": {
                "algorithm": "blake3-256",
                "value": semantic_string(
                    baseline_chunk,
                    &["semantic_hash", "value"],
                    IncrementalRefreshReportError::InvalidBaseline
                )?
            },
            "target_semantic_hash": {
                "algorithm": "blake3-256",
                "value": semantic_string(
                    &target_chunk,
                    &["semantic_hash", "value"],
                    IncrementalRefreshReportError::InvalidTarget
                )?
            }
        }));
    }
    Ok(rematerialized)
}

fn document_rematerialization(
    baseline: &BTreeMap<String, Value>,
    target: &BTreeMap<String, Value>,
) -> Result<Vec<Value>, IncrementalRefreshReportError> {
    let mut rematerialized = Vec::new();
    for (document_id, target_document) in target {
        let Some(baseline_document) = baseline.get(document_id) else {
            continue;
        };
        let baseline_blake3 = semantic_string(
            baseline_document,
            &["blake3"],
            IncrementalRefreshReportError::InvalidDocumentation,
        )?;
        let target_blake3 = semantic_string(
            target_document,
            &["blake3"],
            IncrementalRefreshReportError::InvalidDocumentation,
        )?;
        rematerialized.push(json!({
            "document_id": document_id,
            "path": semantic_string(
                target_document,
                &["path"],
                IncrementalRefreshReportError::InvalidDocumentation
            )?,
            "baseline_blake3": baseline_blake3,
            "target_blake3": target_blake3,
            "manifest_rematerialized": true,
            "content_changed": baseline_blake3 != target_blake3
        }));
    }
    Ok(rematerialized)
}

fn changed_document_content_count(
    baseline: &BTreeMap<String, Value>,
    target: &BTreeMap<String, Value>,
) -> Result<usize, IncrementalRefreshReportError> {
    target
        .iter()
        .try_fold(0_usize, |count, (identity, document)| {
            let target_hash = semantic_string(
                document,
                &["blake3"],
                IncrementalRefreshReportError::InvalidDocumentation,
            )?;
            let changed = baseline.get(identity).is_none_or(|baseline| {
                baseline.get("blake3").and_then(Value::as_str) != Some(target_hash.as_str())
            });
            Ok(count + usize::from(changed))
        })
}

fn invalidation_value(
    baseline: &Value,
    target: &Value,
    baseline_documents: &BTreeMap<String, Value>,
    target_documents: &BTreeMap<String, Value>,
    invalidate_documents: bool,
) -> Result<Value, IncrementalRefreshReportError> {
    let baseline_graph = baseline
        .get("knowledge_graph")
        .ok_or(IncrementalRefreshReportError::InvalidBaseline)?;
    let target_graph = target
        .get("knowledge_graph")
        .ok_or(IncrementalRefreshReportError::InvalidTarget)?;
    let mut value = Map::new();
    for (report_field, graph_field) in [
        ("entities", "entities"),
        ("relationships", "relationships"),
        ("claims", "claims"),
        ("evidence", "evidence"),
        ("coverage_gaps", "coverage"),
    ] {
        let baseline_values = value_index(
            semantic_array(
                baseline_graph,
                &[graph_field],
                IncrementalRefreshReportError::InvalidBaseline,
            )?,
            "id",
            IncrementalRefreshReportError::InvalidBaseline,
        )?;
        let target_values = value_index(
            semantic_array(
                target_graph,
                &[graph_field],
                IncrementalRefreshReportError::InvalidTarget,
            )?,
            "id",
            IncrementalRefreshReportError::InvalidTarget,
        )?;
        value.insert(
            report_field.to_owned(),
            identity_delta(&baseline_values, &target_values, false),
        );
    }
    value.insert(
        "documents".to_owned(),
        identity_delta(baseline_documents, target_documents, invalidate_documents),
    );
    value.insert(
        "federation_links".to_owned(),
        json!({
            "baseline_count": 0,
            "target_count": 0,
            "retained_count": 0,
            "invalidated_ids": [],
            "added_ids": [],
            "removed_ids": []
        }),
    );
    Ok(Value::Object(value))
}

fn identity_delta(
    baseline: &BTreeMap<String, Value>,
    target: &BTreeMap<String, Value>,
    invalidate_every_baseline: bool,
) -> Value {
    let baseline_ids = baseline.keys().cloned().collect::<BTreeSet<_>>();
    let target_ids = target.keys().cloned().collect::<BTreeSet<_>>();
    let invalidated_ids = baseline
        .iter()
        .filter(|(identity, value)| {
            invalidate_every_baseline || target.get(*identity).is_none_or(|target| target != *value)
        })
        .map(|(identity, _)| identity.clone())
        .collect::<Vec<_>>();
    json!({
        "baseline_count": baseline_ids.len(),
        "target_count": target_ids.len(),
        "retained_count": baseline_ids.intersection(&target_ids).count(),
        "invalidated_ids": invalidated_ids,
        "added_ids": target_ids.difference(&baseline_ids).cloned().collect::<Vec<_>>(),
        "removed_ids": baseline_ids.difference(&target_ids).cloned().collect::<Vec<_>>()
    })
}

fn identity_list_count(value: &Value) -> Result<usize, IncrementalRefreshReportError> {
    [
        "entities",
        "relationships",
        "claims",
        "evidence",
        "coverage_gaps",
        "documents",
        "federation_links",
    ]
    .into_iter()
    .try_fold(0_usize, |total, category| {
        ["invalidated_ids", "added_ids", "removed_ids"]
            .into_iter()
            .try_fold(total, |subtotal, field| {
                let count = semantic_array(
                    value,
                    &[category, field],
                    IncrementalRefreshReportError::InvalidAnalysis,
                )?
                .len();
                subtotal
                    .checked_add(count)
                    .ok_or(IncrementalRefreshReportError::LimitExceeded {
                        limit: "report_subject_ids",
                        maximum: MAX_REPORT_SUBJECT_IDS,
                        observed: MAX_REPORT_SUBJECT_IDS.saturating_add(1),
                    })
            })
    })
}

fn value_index(
    values: &[Value],
    id_field: &str,
    error: IncrementalRefreshReportError,
) -> Result<BTreeMap<String, Value>, IncrementalRefreshReportError> {
    let mut index = BTreeMap::new();
    for value in values {
        let identity = value
            .get(id_field)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or(error)?;
        if index.insert(identity, value.clone()).is_some() {
            return Err(error);
        }
    }
    Ok(index)
}

fn semantic_array<'a>(
    value: &'a Value,
    path: &[&str],
    error: IncrementalRefreshReportError,
) -> Result<&'a [Value], IncrementalRefreshReportError> {
    path.iter()
        .try_fold(value, |current, segment| current.get(*segment).ok_or(error))?
        .as_array()
        .map(Vec::as_slice)
        .ok_or(error)
}

fn semantic_string(
    value: &Value,
    path: &[&str],
    error: IncrementalRefreshReportError,
) -> Result<String, IncrementalRefreshReportError> {
    path.iter()
        .try_fold(value, |current, segment| current.get(*segment).ok_or(error))?
        .as_str()
        .map(str::to_owned)
        .ok_or(error)
}

fn set_values(values: &BTreeSet<String>) -> Vec<String> {
    values.iter().cloned().collect()
}

fn hash_value(value: &str) -> Value {
    json!({"algorithm": "blake3-256", "value": value})
}

fn hash_equivalence(value: &str) -> Value {
    json!({
        "incremental": hash_value(value),
        "cold": hash_value(value),
        "equal": true
    })
}

fn report_semantic_hash(value: &Value) -> Result<String, IncrementalRefreshReportError> {
    let bytes =
        serde_json::to_vec(value).map_err(|_| IncrementalRefreshReportError::Serialization)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codenoesis.incremental-refresh-report.semantic.v1");
    hasher.update(&[0]);
    hasher.update(&bytes);
    Ok(hasher.finalize().to_hex().to_string())
}

fn baseline_corrupt(head: &LocalSnapshotHead, reason: &'static str) -> StorageError {
    StorageError::CorruptMetadata {
        component: StorageComponent::Head,
        reason,
        snapshot_id: Some(head.snapshot_id.to_string()),
    }
}

fn safe_digest(value: &str) -> String {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        value.to_owned()
    } else {
        blake3::hash(value.as_bytes()).to_hex().to_string()
    }
}

fn safe_version(value: &str) -> String {
    if !value.is_empty()
        && value.len() <= 256
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'.' | b'_' | b'/' | b'-')
        })
    {
        value.to_owned()
    } else {
        "invalid".to_owned()
    }
}

fn safe_repository_identity(value: &str) -> String {
    if value.starts_with("urn:codenoesis:")
        && value.len() <= 2_048
        && !value
            .bytes()
            .any(|byte| matches!(byte, b'\r' | b'\n' | b'\0'))
    {
        value.to_owned()
    } else {
        "urn:codenoesis:invalid".to_owned()
    }
}

fn safe_path(value: &str) -> String {
    if !value.is_empty()
        && value.len() <= 1_024
        && !value.starts_with('/')
        && !value.contains('\\')
        && !value.split('/').any(|component| {
            component.is_empty()
                || component == ".."
                || component
                    .bytes()
                    .any(|byte| byte.is_ascii_control() || byte == 0x7f)
        })
    {
        value.to_owned()
    } else {
        "analysis-cache/invalid".to_owned()
    }
}

fn valid_repository_identity(value: &str) -> bool {
    let Some(suffix) = value.strip_prefix("urn:codenoesis:") else {
        return false;
    };
    !suffix.is_empty()
        && value.chars().count() <= 2_048
        && !value
            .bytes()
            .any(|byte| matches!(byte, b'\r' | b'\n' | b'\0'))
}

fn valid_entity_id(value: &str) -> bool {
    valid_prefixed_hex(value, "urn:codenoesis:entity:blake3:", 64)
}

fn valid_git_oid(value: &str) -> bool {
    valid_prefixed_hex(value, "", 40)
}

fn valid_digest(value: &str) -> bool {
    valid_prefixed_hex(value, "", 64)
}

fn valid_prefixed_hex(value: &str, prefix: &str, digits: usize) -> bool {
    value.strip_prefix(prefix).is_some_and(|suffix| {
        suffix.len() == digits
            && suffix
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn valid_safe_path(value: &str) -> bool {
    !value.is_empty()
        && value.chars().count() <= 1_024
        && !value.starts_with('/')
        && !value.contains('\\')
        && !value.split('/').any(|component| {
            component.is_empty()
                || component == ".."
                || component
                    .chars()
                    .any(|character| character.is_ascii_control())
        })
}

fn valid_module_path(value: &str) -> bool {
    let mut segments = value.split("::");
    segments.next() == Some("crate")
        && value.chars().count() <= 1_024
        && segments.all(valid_rust_identifier)
}

fn valid_rust_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn valid_observation_key(value: &str) -> bool {
    let Some(suffix) = value.strip_prefix("obs:") else {
        return false;
    };
    let mut bytes = suffix.bytes();
    value.chars().count() <= 512
        && bytes
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && bytes.all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'.' | b'_' | b':' | b'/' | b'#' | b'-')
        })
}

fn valid_observation_name(value: &str) -> bool {
    !value.is_empty()
        && value.chars().count() <= 1_024
        && !value
            .bytes()
            .any(|byte| matches!(byte, b'\r' | b'\n' | b'\0'))
}

fn valid_node_path(value: &str) -> bool {
    value == "root"
        || value.len() <= 512
            && value.split('/').all(|component| {
                component == "0"
                    || component
                        .bytes()
                        .next()
                        .is_some_and(|byte| (b'1'..=b'9').contains(&byte))
                        && component.bytes().all(|byte| byte.is_ascii_digit())
            })
}

fn has_exact_properties(properties: &BTreeMap<String, Value>, expected: &[&str]) -> bool {
    properties.len() == expected.len()
        && expected
            .iter()
            .all(|property| properties.contains_key(*property))
}

fn exact_string_array(value: Option<&Value>, expected: &[&str]) -> bool {
    value.and_then(Value::as_array).is_some_and(|observed| {
        observed.len() == expected.len()
            && observed
                .iter()
                .zip(expected)
                .all(|(observed, expected)| observed.as_str() == Some(*expected))
    })
}

struct ObservationWriter<'a> {
    key: &'a AnalysisCacheKey,
    entities: Vec<Value>,
}

impl<'a> ObservationWriter<'a> {
    fn new(key: &'a AnalysisCacheKey) -> Self {
        Self {
            key,
            entities: Vec::new(),
        }
    }

    fn write_root(&mut self, analysis: &RustSourceAnalysis) {
        self.entities.push(entity_observation(
            "obs:source",
            "source.file",
            &self.key.canonical_source_path,
            &self.key.canonical_module_path,
            "not_applicable",
            vec![
                property("node_path", Value::String("root".to_owned())),
                property("unsupported", Value::Bool(analysis.unsupported_construct)),
            ],
        ));
        self.write_node(analysis, "root", &self.key.canonical_module_path);
        self.entities.sort_by(|left, right| {
            left["observation_key"]
                .as_str()
                .unwrap_or_default()
                .as_bytes()
                .cmp(
                    right["observation_key"]
                        .as_str()
                        .unwrap_or_default()
                        .as_bytes(),
                )
        });
    }

    fn write_node(&mut self, analysis: &RustSourceAnalysis, node_path: &str, module_path: &str) {
        for (ordinal, declaration) in analysis.declarations.iter().enumerate() {
            self.entities.push(entity_observation(
                &format!("obs:declaration:{node_path}:{ordinal}"),
                declaration.kind.as_str(),
                &declaration.name,
                module_path,
                cache_visibility(declaration.visibility),
                vec![
                    property("node_path", Value::String(node_path.to_owned())),
                    property(
                        "ordinal",
                        Value::Number(u64::try_from(ordinal).unwrap_or(u64::MAX).into()),
                    ),
                ],
            ));
        }
        for (ordinal, import) in analysis.imports.iter().enumerate() {
            self.entities.push(entity_observation(
                &format!("obs:import:{node_path}:{ordinal}"),
                "rust.symbol_reference",
                import,
                module_path,
                "not_applicable",
                vec![
                    property("node_path", Value::String(node_path.to_owned())),
                    property(
                        "ordinal",
                        Value::Number(u64::try_from(ordinal).unwrap_or(u64::MAX).into()),
                    ),
                ],
            ));
        }
        for (ordinal, module) in analysis.modules.iter().enumerate() {
            let child_node = if node_path == "root" {
                ordinal.to_string()
            } else {
                format!("{node_path}/{ordinal}")
            };
            let child_module = format!("{module_path}::{}", module.name);
            self.entities.push(entity_observation(
                &format!("obs:module:{child_node}"),
                "rust.module",
                &module.name,
                &child_module,
                cache_visibility(module.visibility),
                vec![
                    property("inline", Value::Bool(module.body.is_some())),
                    property("node_path", Value::String(child_node.clone())),
                    property(
                        "ordinal",
                        Value::Number(u64::try_from(ordinal).unwrap_or(u64::MAX).into()),
                    ),
                    property("parent_path", Value::String(node_path.to_owned())),
                    property(
                        "unsupported",
                        Value::Bool(
                            module
                                .body
                                .as_deref()
                                .is_some_and(|body| body.unsupported_construct),
                        ),
                    ),
                ],
            ));
            if let Some(body) = &module.body {
                self.write_node(body, &child_node, &child_module);
            }
        }
    }
}

struct ObservationReader {
    source: SourceNode,
    nodes: BTreeMap<String, SourceNode>,
    modules: BTreeMap<String, Vec<ModuleNode>>,
}

#[derive(Default)]
struct SourceNode {
    declarations: Vec<(u64, RustDeclarationObservation)>,
    imports: Vec<(u64, String)>,
    unsupported_construct: bool,
    module_path: Option<String>,
}

struct ModuleNode {
    ordinal: u64,
    name: String,
    visibility: WorkspaceVisibility,
    node_path: String,
    module_path: String,
    inline: bool,
}

impl ObservationReader {
    #[allow(clippy::too_many_lines)]
    fn new(
        values: &[Value],
        source_path: &str,
        root_module_path: &str,
    ) -> Result<Self, AnalysisCacheEntryV1Error> {
        let mut reader = Self {
            source: SourceNode::default(),
            nodes: BTreeMap::new(),
            modules: BTreeMap::new(),
        };
        let mut observed_keys = BTreeSet::new();
        for value in values {
            let entity = exact_object(
                value,
                &[
                    "kind",
                    "module_path",
                    "name",
                    "observation_key",
                    "properties",
                    "span_keys",
                    "visibility",
                ],
            )?;
            let observation_key = required_string(entity, "observation_key")?;
            let kind = required_string(entity, "kind")?;
            let name = required_string(entity, "name")?;
            let module_path = required_string(entity, "module_path")?;
            let visibility = parse_visibility(&required_string(entity, "visibility")?)?;
            if !valid_observation_key(&observation_key)
                || !valid_observation_name(&name)
                || !valid_module_path(&module_path)
                || !observed_keys.insert(observation_key.clone())
                || !required_array(entity, "span_keys")?.is_empty()
            {
                return Err(AnalysisCacheEntryV1Error::InvalidContract);
            }
            let properties = properties_map(required_array(entity, "properties")?)?;
            if kind == "source.file" {
                if observation_key != "obs:source"
                    || name != source_path
                    || module_path != root_module_path
                    || visibility != WorkspaceVisibility::NotApplicable
                    || string_property(&properties, "node_path")? != "root"
                    || !has_exact_properties(&properties, &["node_path", "unsupported"])
                {
                    return Err(AnalysisCacheEntryV1Error::InvalidContract);
                }
                set_module_path(&mut reader.source, module_path)?;
                reader.source.unsupported_construct = bool_property(&properties, "unsupported")?;
                continue;
            }
            let node_path = string_property(&properties, "node_path")?;
            if !valid_node_path(&node_path) {
                return Err(AnalysisCacheEntryV1Error::InvalidContract);
            }
            if kind == "rust.module" {
                let parent_path = string_property(&properties, "parent_path")?;
                let ordinal = integer_property(&properties, "ordinal")?;
                let inline = bool_property(&properties, "inline")?;
                let unsupported = bool_property(&properties, "unsupported")?;
                if !has_exact_properties(
                    &properties,
                    &[
                        "inline",
                        "node_path",
                        "ordinal",
                        "parent_path",
                        "unsupported",
                    ],
                ) || !valid_node_path(&parent_path)
                    || node_path != child_node_path(&parent_path, ordinal)
                    || observation_key != format!("obs:module:{node_path}")
                    || !inline && unsupported
                {
                    return Err(AnalysisCacheEntryV1Error::InvalidContract);
                }
                if inline {
                    let child = reader.nodes.entry(node_path.clone()).or_default();
                    set_module_path(child, module_path.clone())?;
                    child.unsupported_construct = unsupported;
                }
                reader
                    .modules
                    .entry(parent_path)
                    .or_default()
                    .push(ModuleNode {
                        ordinal,
                        name,
                        visibility,
                        node_path,
                        module_path,
                        inline,
                    });
                continue;
            }
            if !has_exact_properties(&properties, &["node_path", "ordinal"]) {
                return Err(AnalysisCacheEntryV1Error::InvalidContract);
            }
            let node = if node_path == "root" {
                &mut reader.source
            } else {
                reader.nodes.entry(node_path.clone()).or_default()
            };
            set_module_path(node, module_path)?;
            let ordinal = integer_property(&properties, "ordinal")?;
            if kind == "rust.symbol_reference" {
                if observation_key != format!("obs:import:{node_path}:{ordinal}")
                    || visibility != WorkspaceVisibility::NotApplicable
                {
                    return Err(AnalysisCacheEntryV1Error::InvalidContract);
                }
                node.imports.push((ordinal, name));
            } else {
                if observation_key != format!("obs:declaration:{node_path}:{ordinal}") {
                    return Err(AnalysisCacheEntryV1Error::InvalidContract);
                }
                node.declarations.push((
                    ordinal,
                    RustDeclarationObservation {
                        kind: parse_entity_kind(&kind)?,
                        name,
                        visibility,
                    },
                ));
            }
        }
        if !observed_keys.contains("obs:source") {
            return Err(AnalysisCacheEntryV1Error::InvalidContract);
        }
        Ok(reader)
    }

    fn read_root(mut self) -> Result<RustSourceAnalysis, AnalysisCacheEntryV1Error> {
        let source = std::mem::take(&mut self.source);
        let root_module_path = source
            .module_path
            .clone()
            .ok_or(AnalysisCacheEntryV1Error::InvalidContract)?;
        let analysis = self.read_node("root", &root_module_path, source)?;
        if !self.nodes.is_empty() || !self.modules.is_empty() {
            return Err(AnalysisCacheEntryV1Error::InvalidContract);
        }
        Ok(analysis)
    }

    fn read_node(
        &mut self,
        node_path: &str,
        expected_module_path: &str,
        mut source: SourceNode,
    ) -> Result<RustSourceAnalysis, AnalysisCacheEntryV1Error> {
        if source.module_path.as_deref() != Some(expected_module_path) {
            return Err(AnalysisCacheEntryV1Error::InvalidContract);
        }
        source.declarations.sort_by_key(|(ordinal, _)| *ordinal);
        source.imports.sort_by_key(|(ordinal, _)| *ordinal);
        let mut modules = self.modules.remove(node_path).unwrap_or_default();
        modules.sort_by_key(|module| module.ordinal);
        if !dense_ordinals(&source.declarations)
            || !dense_ordinals(&source.imports)
            || !dense_module_ordinals(&modules)
        {
            return Err(AnalysisCacheEntryV1Error::InvalidContract);
        }
        let mut module_names = BTreeSet::new();
        let modules = modules
            .into_iter()
            .map(|module| {
                let expected_child_module = format!("{expected_module_path}::{}", module.name);
                if module.module_path != expected_child_module
                    || !module_names.insert(module.name.clone())
                {
                    return Err(AnalysisCacheEntryV1Error::InvalidContract);
                }
                let body = if module.inline {
                    let child = self
                        .nodes
                        .remove(&module.node_path)
                        .ok_or(AnalysisCacheEntryV1Error::InvalidContract)?;
                    Some(Box::new(self.read_node(
                        &module.node_path,
                        &module.module_path,
                        child,
                    )?))
                } else {
                    None
                };
                Ok(RustModuleObservation {
                    name: module.name,
                    visibility: module.visibility,
                    body,
                })
            })
            .collect::<Result<Vec<_>, AnalysisCacheEntryV1Error>>()?;
        Ok(RustSourceAnalysis {
            declarations: source
                .declarations
                .into_iter()
                .map(|(_, declaration)| declaration)
                .collect(),
            modules,
            imports: source
                .imports
                .into_iter()
                .map(|(_, import)| import)
                .collect(),
            unsupported_construct: source.unsupported_construct,
        })
    }
}

fn set_module_path(
    node: &mut SourceNode,
    module_path: String,
) -> Result<(), AnalysisCacheEntryV1Error> {
    if node
        .module_path
        .as_ref()
        .is_some_and(|observed| observed != &module_path)
    {
        return Err(AnalysisCacheEntryV1Error::InvalidContract);
    }
    node.module_path = Some(module_path);
    Ok(())
}

fn child_node_path(parent_path: &str, ordinal: u64) -> String {
    if parent_path == "root" {
        ordinal.to_string()
    } else {
        format!("{parent_path}/{ordinal}")
    }
}

fn dense_ordinals<T>(values: &[(u64, T)]) -> bool {
    values
        .iter()
        .enumerate()
        .all(|(expected, (observed, _))| u64::try_from(expected) == Ok(*observed))
}

fn dense_module_ordinals(values: &[ModuleNode]) -> bool {
    values
        .iter()
        .enumerate()
        .all(|(expected, module)| u64::try_from(expected) == Ok(module.ordinal))
}

fn entity_observation(
    observation_key: &str,
    kind: &str,
    name: &str,
    module_path: &str,
    visibility: &str,
    mut properties: Vec<Value>,
) -> Value {
    properties.sort_by(|left, right| {
        left["name"]
            .as_str()
            .unwrap_or_default()
            .as_bytes()
            .cmp(right["name"].as_str().unwrap_or_default().as_bytes())
    });
    json!({
        "observation_key": observation_key,
        "kind": kind,
        "name": name,
        "module_path": module_path,
        "visibility": visibility,
        "properties": properties,
        "span_keys": []
    })
}

fn property(name: &str, value: Value) -> Value {
    let mut property = Map::new();
    property.insert("name".to_owned(), Value::String(name.to_owned()));
    property.insert("value".to_owned(), value);
    Value::Object(property)
}

fn cache_visibility(visibility: WorkspaceVisibility) -> &'static str {
    match visibility {
        WorkspaceVisibility::Public => "public",
        WorkspaceVisibility::Private => "private",
        WorkspaceVisibility::InheritedTrait => "restricted",
        WorkspaceVisibility::NotApplicable => "not_applicable",
    }
}

fn parse_visibility(value: &str) -> Result<WorkspaceVisibility, AnalysisCacheEntryV1Error> {
    match value {
        "public" => Ok(WorkspaceVisibility::Public),
        "private" => Ok(WorkspaceVisibility::Private),
        "restricted" => Ok(WorkspaceVisibility::InheritedTrait),
        "not_applicable" => Ok(WorkspaceVisibility::NotApplicable),
        _ => Err(AnalysisCacheEntryV1Error::InvalidContract),
    }
}

fn parse_entity_kind(value: &str) -> Result<EntityKind, AnalysisCacheEntryV1Error> {
    EntityKind::ALL
        .into_iter()
        .find(|kind| kind.as_str() == value && *kind != EntityKind::RustCrate)
        .ok_or(AnalysisCacheEntryV1Error::InvalidContract)
}

fn properties_map(values: &[Value]) -> Result<BTreeMap<String, Value>, AnalysisCacheEntryV1Error> {
    if values.len() > 128 {
        return Err(AnalysisCacheEntryV1Error::InvalidContract);
    }
    let mut properties = BTreeMap::new();
    for value in values {
        let property = exact_object(value, &["name", "value"])?;
        let name = required_string(property, "name")?;
        let value = required_value(property, "value")?.clone();
        if !valid_property_name(&name)
            || !valid_property_value(&value)
            || properties.insert(name, value).is_some()
        {
            return Err(AnalysisCacheEntryV1Error::InvalidContract);
        }
    }
    Ok(properties)
}

fn valid_property_name(value: &str) -> bool {
    let mut bytes = value.bytes();
    value.len() <= 128
        && bytes.next().is_some_and(|byte| byte.is_ascii_lowercase())
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn valid_property_value(value: &Value) -> bool {
    match value {
        Value::String(value) => {
            value.chars().count() <= 4_096
                && !value
                    .bytes()
                    .any(|byte| matches!(byte, b'\r' | b'\n' | b'\0'))
        }
        Value::Number(value) => value
            .as_i64()
            .is_some_and(|value| (-9_007_199_254_740_991..=9_007_199_254_740_991).contains(&value)),
        Value::Bool(_) | Value::Null => true,
        Value::Array(_) | Value::Object(_) => false,
    }
}

fn string_property(
    properties: &BTreeMap<String, Value>,
    name: &str,
) -> Result<String, AnalysisCacheEntryV1Error> {
    properties
        .get(name)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or(AnalysisCacheEntryV1Error::InvalidContract)
}

fn bool_property(
    properties: &BTreeMap<String, Value>,
    name: &str,
) -> Result<bool, AnalysisCacheEntryV1Error> {
    properties
        .get(name)
        .and_then(Value::as_bool)
        .ok_or(AnalysisCacheEntryV1Error::InvalidContract)
}

fn integer_property(
    properties: &BTreeMap<String, Value>,
    name: &str,
) -> Result<u64, AnalysisCacheEntryV1Error> {
    properties
        .get(name)
        .and_then(Value::as_u64)
        .ok_or(AnalysisCacheEntryV1Error::InvalidContract)
}

fn payload_hash(payload: &Value) -> String {
    let bytes = serde_json::to_vec(payload).expect("cache payload serialization");
    let mut hasher = blake3::Hasher::new();
    hasher.update(ANALYSIS_CACHE_PAYLOAD_DOMAIN.as_bytes());
    hasher.update(&[0]);
    hasher.update(&bytes);
    hasher.finalize().to_hex().to_string()
}

fn exact_object<'a>(
    value: &'a Value,
    fields: &[&str],
) -> Result<&'a Map<String, Value>, AnalysisCacheEntryV1Error> {
    let object = value
        .as_object()
        .ok_or(AnalysisCacheEntryV1Error::InvalidContract)?;
    let expected = fields.iter().copied().collect::<BTreeSet<_>>();
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(AnalysisCacheEntryV1Error::InvalidContract);
    }
    Ok(object)
}

fn required_value<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a Value, AnalysisCacheEntryV1Error> {
    object
        .get(field)
        .ok_or(AnalysisCacheEntryV1Error::InvalidContract)
}

fn required_string(
    object: &Map<String, Value>,
    field: &str,
) -> Result<String, AnalysisCacheEntryV1Error> {
    required_value(object, field)?
        .as_str()
        .map(str::to_owned)
        .ok_or(AnalysisCacheEntryV1Error::InvalidContract)
}

fn required_array<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a [Value], AnalysisCacheEntryV1Error> {
    required_value(object, field)?
        .as_array()
        .map(Vec::as_slice)
        .ok_or(AnalysisCacheEntryV1Error::InvalidContract)
}

fn required_const(
    object: &Map<String, Value>,
    field: &str,
    expected: &str,
) -> Result<(), AnalysisCacheEntryV1Error> {
    if required_string(object, field)? == expected {
        Ok(())
    } else {
        Err(AnalysisCacheEntryV1Error::InvalidContract)
    }
}
