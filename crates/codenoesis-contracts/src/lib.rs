//! Versioned JSON contracts for the `CodeNoesis` S0 and S1 slices.

use std::collections::BTreeMap;
use std::io::{self, Write};

use codenoesis_domain::{
    AcquisitionError, BoundRevision, InputError, InventoryFile, InventoryLanguage, LimitKind,
    RecognizedInventoryKind, RepositoryInventory, STANDARD_LOCAL_S1_LIMITS, limit_exceeded,
};
use serde_json::{Value, json};

const CONFIGURATION_HASH_DOMAIN: &[u8] = b"codenoesis.configuration.semantic.v1";
const SNAPSHOT_HASH_DOMAIN: &[u8] = b"codenoesis.repository-snapshot.semantic.v1";
const SNAPSHOT_V2_HASH_DOMAIN: &[u8] = b"codenoesis.repository-snapshot.semantic.v2";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotEnvelopeV1 {
    created_at: String,
    job_id: Option<String>,
    correlation_id: String,
}

impl SnapshotEnvelopeV1 {
    #[must_use]
    pub const fn new(created_at: String, job_id: Option<String>, correlation_id: String) -> Self {
        Self {
            created_at,
            job_id,
            correlation_id,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RepositorySnapshotV1 {
    value: Value,
}

impl RepositorySnapshotV1 {
    #[must_use]
    pub fn from_bound_revision(bound: &BoundRevision, envelope: SnapshotEnvelopeV1) -> Self {
        let SnapshotEnvelopeV1 {
            created_at,
            job_id,
            correlation_id,
        } = envelope;
        let configuration_semantic = json!({"profile": "standard-local-s0"});
        let configuration_hash = semantic_hash(CONFIGURATION_HASH_DOMAIN, &configuration_semantic);
        let semantic = json!({
            "repository": {
                "contract_version": "codenoesis.repository/v1",
                "identity_schema_version": "codenoesis.repository-identity/v1",
                "identity": bound.repository_identity().as_str(),
                "vcs": "git",
                "object_format": "sha1",
                "commit_oid": bound.commit_oid().as_str(),
                "tree_oid": bound.tree_oid().as_str()
            },
            "configuration": {
                "schema_version": "codenoesis.configuration/v1",
                "profile": "standard-local-s0",
                "semantic_hash": {
                    "algorithm": "blake3-256",
                    "value": configuration_hash
                }
            },
            "pipeline_version": "codenoesis.pipeline/s0-v1",
            "ontology_version": "codenoesis.ontology/none-v1",
            "extractor_contract_version": "codenoesis.extraction/v1",
            "extractor_versions": [],
            "evidence_lineage_version": "codenoesis.evidence-lineage/v1"
        });
        let snapshot_hash = semantic_hash(SNAPSHOT_HASH_DOMAIN, &semantic);
        Self {
            value: json!({
                "schema_version": "codenoesis.repository-snapshot/v1",
                "semantic_hash": {
                    "algorithm": "blake3-256",
                    "value": snapshot_hash
                },
                "semantic": semantic,
                "envelope": {
                    "created_at": created_at,
                    "job_id": job_id,
                    "correlation_id": correlation_id
                }
            }),
        }
    }

    /// Serializes the complete snapshot as RFC 8785-compatible S0 bytes.
    ///
    /// # Errors
    ///
    /// Returns an error only if the internally constructed JSON value cannot be
    /// serialized.
    pub fn canonical_stdout(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = serde_json::to_vec(&self.value)?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Serializes the semantic value as RFC 8785-compatible S0 bytes.
    ///
    /// # Errors
    ///
    /// Returns an error only if the internally constructed JSON value cannot be
    /// serialized.
    pub fn canonical_semantic(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(&self.value["semantic"])
    }

    #[must_use]
    pub fn value(&self) -> &Value {
        &self.value
    }
}

#[derive(Clone, Debug)]
pub struct RepositorySnapshotV2 {
    value: Value,
}

#[derive(Debug)]
pub enum RepositorySnapshotV2Error {
    Serialization(serde_json::Error),
    LimitExceeded(AcquisitionError),
    OutputLengthOverflow,
}

impl RepositorySnapshotV2 {
    #[must_use]
    pub fn from_inventory(inventory: &RepositoryInventory, envelope: SnapshotEnvelopeV1) -> Self {
        let SnapshotEnvelopeV1 {
            created_at,
            job_id,
            correlation_id,
        } = envelope;
        let configuration_semantic = json!({"profile": "standard-local-s1"});
        let configuration_hash = semantic_hash(CONFIGURATION_HASH_DOMAIN, &configuration_semantic);
        let bound = inventory.bound_revision();
        let semantic = json!({
            "repository": {
                "contract_version": "codenoesis.repository/v1",
                "identity_schema_version": "codenoesis.repository-identity/v1",
                "identity": bound.repository_identity().as_str(),
                "vcs": "git",
                "object_format": "sha1",
                "commit_oid": bound.commit_oid().as_str(),
                "tree_oid": bound.tree_oid().as_str()
            },
            "configuration": {
                "schema_version": "codenoesis.configuration/v1",
                "profile": "standard-local-s1",
                "semantic_hash": {
                    "algorithm": "blake3-256",
                    "value": configuration_hash
                }
            },
            "pipeline_version": "codenoesis.pipeline/s1-v1",
            "ontology_version": "codenoesis.ontology/none-v1",
            "extractor_contract_version": "codenoesis.extraction/v1",
            "extractor_versions": ["codenoesis.inventory-classifier/s1-v1"],
            "evidence_lineage_version": "codenoesis.evidence-lineage/v1",
            "inventory": inventory_value(inventory)
        });
        let snapshot_hash = semantic_hash(SNAPSHOT_V2_HASH_DOMAIN, &semantic);
        Self {
            value: json!({
                "schema_version": "codenoesis.repository-snapshot/v2",
                "semantic_hash": {
                    "algorithm": "blake3-256",
                    "value": snapshot_hash
                },
                "semantic": semantic,
                "envelope": {
                    "created_at": created_at,
                    "job_id": job_id,
                    "correlation_id": correlation_id
                }
            }),
        }
    }

    /// Serializes the complete S1 snapshot and enforces the public output limit.
    ///
    /// # Errors
    ///
    /// Returns [`AcquisitionError::LimitExceeded`] when the LF-terminated
    /// canonical document would exceed the fixed S1 output limit.
    pub fn canonical_stdout(&self) -> Result<Vec<u8>, RepositorySnapshotV2Error> {
        let maximum = usize::try_from(STANDARD_LOCAL_S1_LIMITS.canonical_output_bytes)
            .map_err(|_| RepositorySnapshotV2Error::OutputLengthOverflow)?;
        let body_maximum = maximum
            .checked_sub(1)
            .ok_or(RepositorySnapshotV2Error::OutputLengthOverflow)?;
        let mut writer = LimitedVecWriter::new(body_maximum);
        let result = serde_json::to_writer(&mut writer, &self.value);
        if writer.overflowed() {
            return Err(RepositorySnapshotV2Error::LimitExceeded(limit_exceeded(
                LimitKind::CanonicalOutputBytes,
                STANDARD_LOCAL_S1_LIMITS
                    .canonical_output_bytes
                    .saturating_add(1),
            )));
        }
        result.map_err(RepositorySnapshotV2Error::Serialization)?;
        let mut bytes = writer.into_inner();
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Serializes the complete S1 semantic value.
    ///
    /// # Errors
    ///
    /// Returns a serialization error if the internal JSON value cannot be encoded.
    pub fn canonical_semantic(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(&self.value["semantic"])
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }
}

#[derive(Clone, Debug)]
pub struct CodeNoesisErrorV1 {
    value: Value,
}

impl CodeNoesisErrorV1 {
    #[must_use]
    pub fn from_input(error: InputError) -> Self {
        let code = match error {
            InputError::InvalidRepositoryIdentity => "input.invalid_repository_identity",
            InputError::InvalidRevision | InputError::InvalidProfile => "input.invalid_revision",
        };
        Self::new(code, "input", &error.to_string(), &json!({}))
    }

    #[must_use]
    pub fn from_acquisition(error: &AcquisitionError) -> Self {
        match error {
            AcquisitionError::NotGitRepository => Self::new(
                "acquisition.not_git_repository",
                "acquisition",
                &error.to_string(),
                &json!({}),
            ),
            AcquisitionError::RevisionNotFound { revision } => Self::new(
                "acquisition.revision_not_found",
                "acquisition",
                &error.to_string(),
                &json!({"revision": revision.as_str()}),
            ),
            AcquisitionError::RevisionNotCommit {
                object_oid,
                actual_kind,
            } => Self::new(
                "acquisition.revision_not_commit",
                "acquisition",
                &error.to_string(),
                &json!({
                    "object_oid": object_oid.as_str(),
                    "actual_kind": actual_kind.as_str()
                }),
            ),
            AcquisitionError::ObjectMissing {
                object_oid,
                expected_kind,
                referenced_by,
            } => Self::new(
                "acquisition.object_missing",
                "acquisition",
                &error.to_string(),
                &json!({
                    "object_oid": object_oid.as_str(),
                    "expected_kind": expected_kind.as_str(),
                    "referenced_by": referenced_by.as_str()
                }),
            ),
            AcquisitionError::RepositoryInconsistent {
                object_oid,
                expected_kind,
            } => Self::new(
                "acquisition.repository_inconsistent",
                "acquisition",
                &error.to_string(),
                &json!({
                    "object_oid": object_oid.as_str(),
                    "expected_kind": expected_kind.as_str()
                }),
            ),
            AcquisitionError::UnsupportedRepositoryShape { feature } => Self::new(
                "acquisition.unsupported_repository_shape",
                "acquisition",
                &error.to_string(),
                &json!({"feature": feature.as_str()}),
            ),
            AcquisitionError::PathInvalid { .. }
            | AcquisitionError::RootPolicyViolation { .. }
            | AcquisitionError::EntryPolicyViolation { .. }
            | AcquisitionError::LimitExceeded { .. } => Self::internal(),
        }
    }

    #[must_use]
    pub fn internal() -> Self {
        Self::new(
            "internal.unexpected",
            "internal",
            "unexpected internal failure",
            &json!({}),
        )
    }

    fn new(code: &str, stage: &str, message: &str, context: &Value) -> Self {
        Self {
            value: json!({
                "schema_version": "codenoesis.error/v1",
                "code": code,
                "stage": stage,
                "message": message,
                "retryable": false,
                "context": context
            }),
        }
    }

    /// Serializes one strict error document followed by one LF.
    ///
    /// # Errors
    ///
    /// Returns an error only if the internally constructed JSON value cannot be
    /// serialized.
    pub fn canonical_stderr(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = serde_json::to_vec(&self.value)?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

#[derive(Clone, Debug)]
pub struct CodeNoesisErrorV2 {
    value: Value,
}

impl CodeNoesisErrorV2 {
    #[must_use]
    pub fn from_input(error: InputError) -> Self {
        let code = match error {
            InputError::InvalidRepositoryIdentity => "input.invalid_repository_identity",
            InputError::InvalidRevision => "input.invalid_revision",
            InputError::InvalidProfile => "input.invalid_profile",
        };
        Self::new(code, "input", &error.to_string(), &json!({}))
    }

    #[must_use]
    pub fn from_acquisition(error: &AcquisitionError) -> Self {
        match error {
            AcquisitionError::NotGitRepository => Self::new(
                "acquisition.not_git_repository",
                "acquisition",
                &error.to_string(),
                &json!({}),
            ),
            AcquisitionError::RevisionNotFound { revision } => Self::new(
                "acquisition.revision_not_found",
                "acquisition",
                &error.to_string(),
                &json!({"revision": revision.as_str()}),
            ),
            AcquisitionError::RevisionNotCommit {
                object_oid,
                actual_kind,
            } => Self::new(
                "acquisition.revision_not_commit",
                "acquisition",
                &error.to_string(),
                &json!({
                    "object_oid": object_oid.as_str(),
                    "actual_kind": actual_kind.as_str()
                }),
            ),
            AcquisitionError::ObjectMissing {
                object_oid,
                expected_kind,
                referenced_by,
            } => Self::new(
                "acquisition.object_missing",
                "acquisition",
                &error.to_string(),
                &json!({
                    "object_oid": object_oid.as_str(),
                    "expected_kind": expected_kind.as_str(),
                    "referenced_by": referenced_by.as_str()
                }),
            ),
            AcquisitionError::RepositoryInconsistent {
                object_oid,
                expected_kind,
            } => Self::new(
                "acquisition.repository_inconsistent",
                "acquisition",
                &error.to_string(),
                &json!({
                    "object_oid": object_oid.as_str(),
                    "expected_kind": expected_kind.as_str()
                }),
            ),
            AcquisitionError::UnsupportedRepositoryShape { feature } => Self::new(
                "acquisition.unsupported_repository_shape",
                "acquisition",
                &error.to_string(),
                &json!({"feature": feature.as_str()}),
            ),
            AcquisitionError::PathInvalid { reason } => Self::new(
                "acquisition.path_invalid",
                "acquisition",
                &error.to_string(),
                &json!({"reason": reason.as_str()}),
            ),
            AcquisitionError::RootPolicyViolation { policy } => Self::new(
                "acquisition.root_policy_violation",
                "acquisition",
                &error.to_string(),
                &json!({"policy": policy.as_str()}),
            ),
            AcquisitionError::EntryPolicyViolation { path, entry } => Self::new(
                "acquisition.entry_policy_violation",
                "acquisition",
                &error.to_string(),
                &json!({"entry": entry.as_str(), "path": path}),
            ),
            AcquisitionError::LimitExceeded {
                limit,
                maximum,
                observed,
            } => Self::new(
                "acquisition.limit_exceeded",
                "acquisition",
                &error.to_string(),
                &json!({
                    "limit": limit.as_str(),
                    "maximum": maximum,
                    "observed": observed
                }),
            ),
        }
    }

    #[must_use]
    pub fn internal() -> Self {
        Self::new(
            "internal.unexpected",
            "internal",
            "unexpected internal failure",
            &json!({}),
        )
    }

    fn new(code: &str, stage: &str, message: &str, context: &Value) -> Self {
        Self {
            value: json!({
                "schema_version": "codenoesis.error/v2",
                "code": code,
                "stage": stage,
                "message": message,
                "retryable": false,
                "context": context
            }),
        }
    }

    /// Serializes one strict S1 error document followed by one LF.
    ///
    /// # Errors
    ///
    /// Returns an error only if the internally constructed JSON value cannot be serialized.
    pub fn canonical_stderr(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = serde_json::to_vec(&self.value)?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

fn inventory_value(inventory: &RepositoryInventory) -> Value {
    let files = inventory.files();
    let language_groups = language_groups(files);
    let manifests = recognized_files(files, RecognizedInventoryKind::CargoManifest);
    let contracts = recognized_files(files, RecognizedInventoryKind::OpenApiContract);
    let configurations = recognized_files(files, RecognizedInventoryKind::RustfmtConfiguration);
    let ownership = recognized_files(files, RecognizedInventoryKind::GitHubCodeowners);
    let unsupported = files
        .iter()
        .filter(|file| file.is_unsupported())
        .collect::<Vec<_>>();
    let sentinels = files
        .iter()
        .filter(|file| file.is_sentinel())
        .collect::<Vec<_>>();
    let diagnostics = diagnostics_value(&sentinels, &unsupported);
    let capabilities = capabilities_value(
        &language_groups,
        &manifests,
        &contracts,
        &configurations,
        &ownership,
    );
    let coverage_gaps = coverage_gaps_value(
        &language_groups,
        &manifests,
        &contracts,
        &configurations,
        &ownership,
        &unsupported,
    );
    let supported_file_count = files
        .len()
        .checked_sub(unsupported.len())
        .expect("unsupported files are a subset");

    json!({
        "schema_version": "codenoesis.inventory/v1",
        "classifier_version": "codenoesis.inventory-classifier/s1-v1",
        "summary": {
            "directory_count": inventory.directory_count(),
            "regular_file_count": files.len(),
            "total_file_bytes": files.iter().map(InventoryFile::byte_length).sum::<u64>(),
            "supported_file_count": supported_file_count,
            "unsupported_file_count": unsupported.len(),
            "language_count": language_groups.len(),
            "manifest_count": manifests.len(),
            "contract_count": contracts.len(),
            "configuration_count": configurations.len(),
            "ownership_count": ownership.len(),
            "diagnostic_count": diagnostics.len(),
            "coverage_gap_count": coverage_gaps.len()
        },
        "files": files.iter().map(file_value).collect::<Vec<_>>(),
        "languages": language_groups.iter().map(language_value).collect::<Vec<_>>(),
        "manifests": manifests.iter().map(|file| recognized_value("cargo", file)).collect::<Vec<_>>(),
        "contracts": contracts.iter().map(|file| recognized_value("openapi", file)).collect::<Vec<_>>(),
        "configurations": configurations.iter().map(|file| recognized_value("rustfmt", file)).collect::<Vec<_>>(),
        "ownership": ownership.iter().map(|file| recognized_value("github-codeowners", file)).collect::<Vec<_>>(),
        "extraction_capabilities": capabilities,
        "unsupported_content": unsupported.iter().map(|file| json!({
            "path": file.path(),
            "reason": "unsupported_extension",
            "evidence_id": file.evidence_id()
        })).collect::<Vec<_>>(),
        "diagnostics": diagnostics,
        "coverage_gaps": coverage_gaps,
        "evidence": files.iter().map(|file| evidence_value(inventory, file)).collect::<Vec<_>>()
    })
}

fn file_value(file: &InventoryFile) -> Value {
    json!({
        "path": file.path(),
        "mode": file.mode().as_str(),
        "blob_oid": file.blob_oid().as_str(),
        "byte_length": file.byte_length(),
        "content_kind": file.content_kind().as_str(),
        "roles": file.roles().iter().map(|role| role.as_str()).collect::<Vec<_>>(),
        "languages": file.languages().iter().map(|language| language.as_str()).collect::<Vec<_>>(),
        "evidence_id": file.evidence_id()
    })
}

fn language_groups(files: &[InventoryFile]) -> Vec<(InventoryLanguage, Vec<&InventoryFile>)> {
    let mut groups = BTreeMap::<InventoryLanguage, Vec<&InventoryFile>>::new();
    for file in files {
        for language in file.languages() {
            groups.entry(*language).or_default().push(file);
        }
    }
    groups.into_iter().collect()
}

fn language_value((language, files): &(InventoryLanguage, Vec<&InventoryFile>)) -> Value {
    json!({
        "id": language.as_str(),
        "display_name": language.display_name(),
        "paths": files.iter().map(|file| file.path()).collect::<Vec<_>>(),
        "evidence_ids": files.iter().map(|file| file.evidence_id()).collect::<Vec<_>>(),
        "detection_status": "supported",
        "extraction_status": "not_available"
    })
}

fn recognized_files(files: &[InventoryFile], kind: RecognizedInventoryKind) -> Vec<&InventoryFile> {
    files
        .iter()
        .filter(|file| file.recognized_kind() == Some(kind))
        .collect()
}

fn recognized_value(kind: &str, file: &InventoryFile) -> Value {
    json!({
        "kind": kind,
        "path": file.path(),
        "status": "recognized_not_interpreted",
        "evidence_id": file.evidence_id()
    })
}

fn capabilities_value(
    languages: &[(InventoryLanguage, Vec<&InventoryFile>)],
    manifests: &[&InventoryFile],
    contracts: &[&InventoryFile],
    configurations: &[&InventoryFile],
    ownership: &[&InventoryFile],
) -> Vec<Value> {
    let mut capabilities = Vec::new();
    if !configurations.is_empty() {
        capabilities.push(("configuration_interpretation", "configuration:rustfmt"));
    }
    if !contracts.is_empty() {
        capabilities.push(("contract_extraction", "contract:openapi"));
    }
    capabilities.push(("file_classification", "repository"));
    if !manifests.is_empty() {
        capabilities.push(("manifest_interpretation", "manifest:cargo"));
    }
    if !ownership.is_empty() {
        capabilities.push(("ownership_resolution", "ownership:github-codeowners"));
    }
    for (language, _) in languages {
        capabilities.push((
            "symbol_extraction",
            match language {
                InventoryLanguage::Rust => "language:rust",
                InventoryLanguage::Shell => "language:shell",
            },
        ));
    }
    capabilities.sort_unstable();
    capabilities
        .into_iter()
        .map(|(capability, subject)| {
            json!({
                "capability": capability,
                "subject": subject,
                "status": if capability == "file_classification" {
                    "available"
                } else {
                    "not_available"
                }
            })
        })
        .collect()
}

fn diagnostics_value(sentinels: &[&InventoryFile], unsupported: &[&InventoryFile]) -> Vec<Value> {
    let mut diagnostics = sentinels
        .iter()
        .map(|file| {
            (
                "inventory.target_execution_suppressed",
                file.path(),
                "info",
                file.evidence_id(),
            )
        })
        .chain(unsupported.iter().map(|file| {
            (
                "inventory.unsupported_content",
                file.path(),
                "warning",
                file.evidence_id(),
            )
        }))
        .collect::<Vec<_>>();
    diagnostics.sort_unstable();
    diagnostics
        .into_iter()
        .map(|(code, path, severity, evidence_id)| {
            json!({
                "code": code,
                "severity": severity,
                "path": path,
                "evidence_id": evidence_id
            })
        })
        .collect()
}

fn coverage_gaps_value(
    languages: &[(InventoryLanguage, Vec<&InventoryFile>)],
    manifests: &[&InventoryFile],
    contracts: &[&InventoryFile],
    configurations: &[&InventoryFile],
    ownership: &[&InventoryFile],
    unsupported: &[&InventoryFile],
) -> Vec<Value> {
    let mut gaps = Vec::<(&str, &str, Vec<&str>, Vec<&str>)>::new();
    if !configurations.is_empty() {
        gaps.push(coverage_gap(
            "coverage.configuration_not_interpreted",
            "configuration:rustfmt",
            configurations,
        ));
    }
    if !contracts.is_empty() {
        gaps.push(coverage_gap(
            "coverage.contract_not_extracted",
            "contract:openapi",
            contracts,
        ));
    }
    for (language, files) in languages {
        gaps.push(coverage_gap(
            "coverage.entities_not_extracted",
            match language {
                InventoryLanguage::Rust => "language:rust",
                InventoryLanguage::Shell => "language:shell",
            },
            files,
        ));
    }
    if !manifests.is_empty() {
        gaps.push(coverage_gap(
            "coverage.manifest_not_interpreted",
            "manifest:cargo",
            manifests,
        ));
    }
    if !ownership.is_empty() {
        gaps.push(coverage_gap(
            "coverage.ownership_not_resolved",
            "ownership:github-codeowners",
            ownership,
        ));
    }
    if !unsupported.is_empty() {
        gaps.push(coverage_gap(
            "coverage.unsupported_content",
            "repository",
            unsupported,
        ));
    }
    gaps.sort_by(|left, right| (left.0, left.1).cmp(&(right.0, right.1)));
    gaps.into_iter()
        .map(|(code, scope, paths, evidence_ids)| {
            json!({
                "code": code,
                "scope": scope,
                "paths": paths,
                "evidence_ids": evidence_ids
            })
        })
        .collect()
}

fn coverage_gap<'a>(
    code: &'a str,
    scope: &'a str,
    files: &'a [&InventoryFile],
) -> (&'a str, &'a str, Vec<&'a str>, Vec<&'a str>) {
    (
        code,
        scope,
        files.iter().map(|file| file.path()).collect(),
        files.iter().map(|file| file.evidence_id()).collect(),
    )
}

fn evidence_value(inventory: &RepositoryInventory, file: &InventoryFile) -> Value {
    let bound = inventory.bound_revision();
    json!({
        "schema_version": "codenoesis.source-evidence/v1",
        "evidence_id": file.evidence_id(),
        "repository": {
            "identity": bound.repository_identity().as_str(),
            "vcs": "git",
            "object_format": "sha1",
            "commit_oid": bound.commit_oid().as_str()
        },
        "blob_oid": file.blob_oid().as_str(),
        "path": file.path(),
        "span": {
            "unit": "byte",
            "start": 0,
            "end": file.byte_length()
        },
        "extractor": {
            "id": "codenoesis.inventory.static-classifier",
            "version": "codenoesis.inventory-classifier/s1-v1"
        },
        "derivation": {
            "kind": "deterministic_static_classification",
            "rules": file.rules().iter().map(|rule| rule.as_str()).collect::<Vec<_>>()
        }
    })
}

fn semantic_hash(domain: &[u8], value: &Value) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(&[0]);
    serde_json::to_writer(Blake3Writer(&mut hasher), value)
        .expect("JSON values constructed by CodeNoesis serialize");
    hasher.finalize().to_hex().to_string()
}

struct LimitedVecWriter {
    bytes: Vec<u8>,
    maximum: usize,
    overflowed: bool,
}

impl LimitedVecWriter {
    fn new(maximum: usize) -> Self {
        Self {
            bytes: Vec::new(),
            maximum,
            overflowed: false,
        }
    }

    const fn overflowed(&self) -> bool {
        self.overflowed
    }

    fn into_inner(self) -> Vec<u8> {
        self.bytes
    }
}

impl Write for LimitedVecWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let remaining = self.maximum.saturating_sub(self.bytes.len());
        if buffer.len() > remaining {
            self.bytes.extend_from_slice(&buffer[..remaining]);
            self.overflowed = true;
            return Err(io::Error::other("canonical output limit exceeded"));
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct Blake3Writer<'a>(&'a mut blake3::Hasher);

impl Write for Blake3Writer<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.0.update(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;

    use codenoesis_domain::{BoundRevision, ObjectId, RepositoryIdentity};
    use serde_json::{Map, Value, json};

    use super::{
        RepositorySnapshotV1, RepositorySnapshotV2, RepositorySnapshotV2Error,
        SNAPSHOT_HASH_DOMAIN, SnapshotEnvelopeV1, semantic_hash,
    };

    const COMMIT_A_OID: &str = "6d4152a7787ac82eedf3f9fc5df408dfdf6e412f";
    const TREE_A_OID: &str = "892c4a33b5529ba6b6651fc26765957f11f7ba9e";
    const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s0-one-file-v1";
    const SNAPSHOT_A_HASH: &str =
        "b673624a329f43fd84852bbdeefd66326a7fcb1c03fdb626e2de6bfedff11997";

    fn bound_revision() -> BoundRevision {
        BoundRevision::new(
            RepositoryIdentity::parse(REPOSITORY_ID).expect("approved fixture identity"),
            ObjectId::parse_sha1(COMMIT_A_OID).expect("approved commit OID"),
            ObjectId::parse_sha1(TREE_A_OID).expect("approved tree OID"),
        )
    }

    fn snapshot(envelope: SnapshotEnvelopeV1) -> RepositorySnapshotV1 {
        RepositorySnapshotV1::from_bound_revision(&bound_revision(), envelope)
    }

    fn fixed_envelope() -> SnapshotEnvelopeV1 {
        SnapshotEnvelopeV1::new(
            "2000-01-01T00:00:00Z".to_owned(),
            None,
            "s0-golden-a".to_owned(),
        )
    }

    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/s0/one-file-v1")
            .join(name)
    }

    fn reviewed_jcs_body(name: &str) -> Vec<u8> {
        let mut bytes = fs::read(fixture_path(name)).expect("read reviewed JCS golden");
        if bytes.ends_with(b"\r\n") {
            bytes.truncate(bytes.len() - 2);
        } else {
            assert_eq!(bytes.pop(), Some(b'\n'), "golden must end in one newline");
        }
        assert!(
            !bytes.contains(&b'\r') && !bytes.contains(&b'\n'),
            "golden body must be one canonical JSON line"
        );
        bytes
    }

    #[test]
    fn conf_dr_art_001_repository_snapshot_v1() {
        let actual = snapshot(fixed_envelope())
            .canonical_stdout()
            .expect("serialize fixed snapshot");
        let mut expected = reviewed_jcs_body("expected-snapshot-a.jcs");
        expected.push(b'\n');
        assert_eq!(actual, expected);

        let value: Value = serde_json::from_slice(&actual).expect("parse generated snapshot");
        assert_exact_keys(
            &value,
            &["envelope", "schema_version", "semantic", "semantic_hash"],
        );
        assert_exact_keys(
            &value["semantic"],
            &[
                "configuration",
                "evidence_lineage_version",
                "extractor_contract_version",
                "extractor_versions",
                "ontology_version",
                "pipeline_version",
                "repository",
            ],
        );
        assert_exact_keys(
            &value["semantic"]["repository"],
            &[
                "commit_oid",
                "contract_version",
                "identity",
                "identity_schema_version",
                "object_format",
                "tree_oid",
                "vcs",
            ],
        );
        assert_exact_keys(
            &value["semantic"]["configuration"],
            &["profile", "schema_version", "semantic_hash"],
        );
        assert_exact_keys(
            &value["envelope"],
            &["correlation_id", "created_at", "job_id"],
        );
    }

    #[test]
    fn pt_dr_art_002_volatile_envelope_preserves_semantic_hash() {
        let baseline = snapshot(fixed_envelope());
        let baseline_semantic = baseline
            .canonical_semantic()
            .expect("serialize baseline semantic");
        let baseline_hash = baseline.value()["semantic_hash"].clone();
        let mut baseline_without_envelope = baseline.value().clone();
        baseline_without_envelope
            .as_object_mut()
            .expect("snapshot object")
            .remove("envelope");
        let fixed_stdout = baseline
            .canonical_stdout()
            .expect("serialize fixed snapshot");

        for index in 0..50 {
            let candidate = snapshot(SnapshotEnvelopeV1::new(
                format!("2000-01-01T00:00:{index:02}Z"),
                (index % 2 == 0).then(|| format!("job-{index}")),
                format!("correlation-{index}"),
            ));
            assert_eq!(
                candidate
                    .canonical_semantic()
                    .expect("serialize candidate semantic"),
                baseline_semantic,
                "semantic bytes changed for envelope {index}"
            );
            assert_eq!(candidate.value()["semantic_hash"], baseline_hash);

            let mut candidate_without_envelope = candidate.value().clone();
            candidate_without_envelope
                .as_object_mut()
                .expect("snapshot object")
                .remove("envelope");
            assert_eq!(candidate_without_envelope, baseline_without_envelope);
            assert_eq!(
                snapshot(fixed_envelope())
                    .canonical_stdout()
                    .expect("serialize replayed fixed snapshot"),
                fixed_stdout
            );
        }
    }

    #[test]
    fn pt_nfr_det_001_permutation_and_schedule_invariant() {
        let expected = reviewed_jcs_body("expected-semantic-a.jcs");

        for seed in 0..50 {
            let semantic = permuted_semantic(seed);
            let canonical = serde_json::to_vec(&semantic).expect("serialize permuted semantic");
            assert_eq!(
                canonical, expected,
                "canonical bytes differ for seed {seed}"
            );
            assert_eq!(
                semantic_hash(SNAPSHOT_HASH_DOMAIN, &semantic),
                SNAPSHOT_A_HASH,
                "semantic hash differs for seed {seed}"
            );
        }
    }

    #[test]
    fn pt_fr_acq_002_canonical_output_has_max_and_plus_one() {
        let maximum =
            usize::try_from(codenoesis_domain::STANDARD_LOCAL_S1_LIMITS.canonical_output_bytes)
                .expect("canonical output maximum fits usize");
        let max_snapshot = RepositorySnapshotV2 {
            value: Value::String("a".repeat(maximum - 3)),
        };
        let output = max_snapshot
            .canonical_stdout()
            .expect("maximum canonical output must succeed");
        assert_eq!(output.len(), maximum);
        drop(output);
        drop(max_snapshot);

        let over_snapshot = RepositorySnapshotV2 {
            value: Value::String("a".repeat(maximum - 2)),
        };
        assert!(matches!(
            over_snapshot.canonical_stdout(),
            Err(RepositorySnapshotV2Error::LimitExceeded(
                codenoesis_domain::AcquisitionError::LimitExceeded {
                    limit: codenoesis_domain::LimitKind::CanonicalOutputBytes,
                    maximum: 33_554_432,
                    observed: 33_554_433
                }
            ))
        ));
    }

    fn assert_exact_keys(value: &Value, expected: &[&str]) {
        let actual = value
            .as_object()
            .expect("contract node must be an object")
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let expected = expected.iter().copied().collect::<BTreeSet<_>>();
        assert_eq!(actual, expected);
    }

    fn permuted_semantic(seed: usize) -> Value {
        let repository = permuted_object(
            vec![
                ("contract_version", json!("codenoesis.repository/v1")),
                (
                    "identity_schema_version",
                    json!("codenoesis.repository-identity/v1"),
                ),
                ("identity", json!(REPOSITORY_ID)),
                ("vcs", json!("git")),
                ("object_format", json!("sha1")),
                ("commit_oid", json!(COMMIT_A_OID)),
                ("tree_oid", json!(TREE_A_OID)),
            ],
            seed,
        );
        let configuration = permuted_object(
            vec![
                ("schema_version", json!("codenoesis.configuration/v1")),
                ("profile", json!("standard-local-s0")),
                (
                    "semantic_hash",
                    json!({
                        "algorithm": "blake3-256",
                        "value": "4811a917bebed264f49382d65825686ad5ca506ce39bc51385e547b0c7ced1c0"
                    }),
                ),
            ],
            seed.wrapping_mul(3).wrapping_add(1),
        );
        permuted_object(
            vec![
                ("repository", repository),
                ("configuration", configuration),
                ("pipeline_version", json!("codenoesis.pipeline/s0-v1")),
                ("ontology_version", json!("codenoesis.ontology/none-v1")),
                (
                    "extractor_contract_version",
                    json!("codenoesis.extraction/v1"),
                ),
                ("extractor_versions", json!([])),
                (
                    "evidence_lineage_version",
                    json!("codenoesis.evidence-lineage/v1"),
                ),
            ],
            seed.wrapping_mul(7).wrapping_add(2),
        )
    }

    fn permuted_object(mut entries: Vec<(&'static str, Value)>, seed: usize) -> Value {
        let length = entries.len();
        entries.rotate_left(seed % length);
        if seed % 2 == 1 {
            entries.reverse();
        }
        let mut object = Map::new();
        for (key, value) in entries {
            object.insert(key.to_owned(), value);
        }
        Value::Object(object)
    }
}
