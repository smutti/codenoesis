use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use serde_json::{Map, Value, json};

use crate::local_distribution_bundle_name;

pub const LOCAL_RELEASE_CANDIDATE_MANIFEST_V1_SCHEMA: &str =
    "codenoesis.local-release-candidate-manifest/v1";
pub const LOCAL_RELEASE_CANDIDATE_VERIFICATION_V1_SCHEMA: &str =
    "codenoesis.local-release-candidate-verification/v1";
pub const LOCAL_DEPENDENCY_LOCK_V1_SCHEMA: &str = "codenoesis.local-dependency-lock/v1";
pub const LOCAL_LICENSE_REPORT_V1_SCHEMA: &str = "codenoesis.local-license-report/v1";
pub const LOCAL_ADVISORY_REPORT_V1_SCHEMA: &str = "codenoesis.local-advisory-report/v1";
pub const LOCAL_UNSAFE_INVENTORY_V1_SCHEMA: &str = "codenoesis.local-unsafe-inventory/v1";
pub const CYCLONEDX_1_6_SCHEMA: &str = "CycloneDX-1.6";
pub const CODENOESIS_ERROR_V28_SCHEMA: &str = "codenoesis.error/v28";
pub const MAX_LOCAL_RELEASE_ARCHIVE_BYTES: u64 = 285_212_672;
pub const MAX_LOCAL_RELEASE_EVIDENCE_DOCUMENT_BYTES: usize = 4_194_304;
pub const MAX_LOCAL_RELEASE_EVIDENCE_TOTAL_BYTES: usize = 33_554_432;
pub const MAX_LOCAL_RELEASE_PUBLIC_JSON_BYTES: usize = 131_072;
pub const MAX_LOCAL_RELEASE_PACKAGES: usize = 512;
pub const MAX_LOCAL_RELEASE_DEPENDENCY_EDGES: u64 = 4_096;
pub const MAX_LOCAL_RELEASE_UNSAFE_EXCEPTIONS: usize = 512;
pub const MAX_LOCAL_RELEASE_ZIP_ENTRIES: usize = 1_024;
pub const MAX_LOCAL_RELEASE_RELATIVE_PATH_BYTES: usize = 256;
pub const LOCAL_RELEASE_EVIDENCE_COUNT: usize = 5;
pub const LOCAL_RELEASE_CANDIDATE_SUBJECT_COUNT: usize = 8;

const POLICY_REVIEW_DATE: &str = "2026-08-14";
const RELEASE_PROFILE_ID: &str = "local-experimental-r17";
const SOURCE_REPOSITORY: &str = "https://github.com/smutti/codenoesis";
const SOURCE_REF: &str = "refs/heads/main";
const EXPECTED_EVIDENCE: [(&str, &str); LOCAL_RELEASE_EVIDENCE_COUNT] = [
    (
        "evidence/advisory-report.json",
        LOCAL_ADVISORY_REPORT_V1_SCHEMA,
    ),
    (
        "evidence/dependency-lock.json",
        LOCAL_DEPENDENCY_LOCK_V1_SCHEMA,
    ),
    (
        "evidence/license-report.json",
        LOCAL_LICENSE_REPORT_V1_SCHEMA,
    ),
    ("evidence/sbom.cdx.json", CYCLONEDX_1_6_SCHEMA),
    (
        "evidence/unsafe-inventory.json",
        LOCAL_UNSAFE_INVENTORY_V1_SCHEMA,
    ),
];
const ALLOWED_LICENSE_EXPRESSIONS: [&str; 18] = [
    "(MIT OR Apache-2.0) AND Unicode-3.0",
    "0BSD OR MIT OR Apache-2.0",
    "Apache-2.0",
    "Apache-2.0 OR BSD-3-Clause",
    "Apache-2.0 OR MIT",
    "Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT",
    "BSD-2-Clause",
    "CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception",
    "CC0-1.0 OR MIT-0 OR Apache-2.0",
    "MIT",
    "MIT OR Apache-2.0",
    "MIT OR Apache-2.0 OR Zlib",
    "MIT OR Zlib OR Apache-2.0",
    "MIT/Apache-2.0",
    "Unlicense OR MIT",
    "Unlicense/MIT",
    "Zlib",
    "Zlib OR Apache-2.0 OR MIT",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalReleaseContractError {
    InvalidBundle,
    InvalidEvidence,
    InvalidArchive,
    UnstableInput,
    PolicyRejected,
    LimitExceeded,
    ContractInvalid,
}

impl Display for LocalReleaseContractError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidBundle => "invalid local distribution bundle",
            Self::InvalidEvidence => "invalid release evidence",
            Self::InvalidArchive => "invalid release archive",
            Self::UnstableInput => "unstable release input",
            Self::PolicyRejected => "release policy rejected",
            Self::LimitExceeded => "release limit exceeded",
            Self::ContractInvalid => "invalid release contract",
        })
    }
}

impl Error for LocalReleaseContractError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseEvidenceRecordV1 {
    path: String,
    schema: String,
    length: u64,
    sha256: String,
}

impl ReleaseEvidenceRecordV1 {
    #[must_use]
    pub fn new(
        path: impl Into<String>,
        schema: impl Into<String>,
        length: u64,
        sha256: impl Into<String>,
    ) -> Self {
        Self {
            path: path.into(),
            schema: schema.into(),
            length,
            sha256: sha256.into(),
        }
    }

    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[must_use]
    pub fn schema(&self) -> &str {
        &self.schema
    }

    #[must_use]
    pub const fn length(&self) -> u64 {
        self.length
    }

    #[must_use]
    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    fn value(&self) -> Value {
        json!({
            "path": self.path,
            "schema": self.schema,
            "length": self.length,
            "sha256": self.sha256
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseArchiveRecordV1 {
    path: String,
    length: u64,
    sha256: String,
}

impl ReleaseArchiveRecordV1 {
    #[must_use]
    pub fn new(path: impl Into<String>, length: u64, sha256: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            length,
            sha256: sha256.into(),
        }
    }

    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[must_use]
    pub const fn length(&self) -> u64 {
        self.length
    }

    #[must_use]
    pub fn sha256(&self) -> &str {
        &self.sha256
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmbeddedLocalBundleV1 {
    name: String,
    manifest_sha256: String,
    binary_sha256: String,
}

impl EmbeddedLocalBundleV1 {
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        manifest_sha256: impl Into<String>,
        binary_sha256: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            manifest_sha256: manifest_sha256.into(),
            binary_sha256: binary_sha256.into(),
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn manifest_sha256(&self) -> &str {
        &self.manifest_sha256
    }

    #[must_use]
    pub fn binary_sha256(&self) -> &str {
        &self.binary_sha256
    }
}

#[derive(Clone, Debug)]
pub struct ValidatedLocalSupplyChainV1 {
    target: String,
    cargo_lock_sha256: String,
    records: Vec<ReleaseEvidenceRecordV1>,
}

impl ValidatedLocalSupplyChainV1 {
    #[must_use]
    pub fn target(&self) -> &str {
        &self.target
    }

    #[must_use]
    pub fn cargo_lock_sha256(&self) -> &str {
        &self.cargo_lock_sha256
    }

    #[must_use]
    pub fn records(&self) -> &[ReleaseEvidenceRecordV1] {
        &self.records
    }
}

#[derive(Clone, Debug)]
pub struct LocalReleaseCandidateManifestV1 {
    value: Value,
    target: String,
    source_commit: String,
    archive: ReleaseArchiveRecordV1,
    bundle: EmbeddedLocalBundleV1,
    evidence: Vec<ReleaseEvidenceRecordV1>,
}

impl LocalReleaseCandidateManifestV1 {
    /// Builds the closed local release-candidate manifest.
    ///
    /// # Errors
    ///
    /// Returns a typed archive, evidence, or contract error for any value that
    /// is not part of the exact G1b contract.
    pub fn new(
        target: &str,
        source_commit: &str,
        archive: ReleaseArchiveRecordV1,
        bundle: EmbeddedLocalBundleV1,
        evidence: &[ReleaseEvidenceRecordV1],
    ) -> Result<Self, LocalReleaseContractError> {
        validate_manifest_inputs(target, source_commit, &archive, &bundle, evidence)?;
        let value = manifest_value(target, source_commit, &archive, &bundle, evidence);
        Ok(Self {
            value,
            target: target.to_owned(),
            source_commit: source_commit.to_owned(),
            archive,
            bundle,
            evidence: evidence.to_vec(),
        })
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    #[must_use]
    pub fn target(&self) -> &str {
        &self.target
    }

    #[must_use]
    pub fn source_commit(&self) -> &str {
        &self.source_commit
    }

    #[must_use]
    pub const fn archive(&self) -> &ReleaseArchiveRecordV1 {
        &self.archive
    }

    #[must_use]
    pub const fn bundle(&self) -> &EmbeddedLocalBundleV1 {
        &self.bundle
    }

    #[must_use]
    pub fn evidence(&self) -> &[ReleaseEvidenceRecordV1] {
        &self.evidence
    }

    /// Serializes one canonical LF-terminated manifest.
    ///
    /// # Errors
    ///
    /// Returns a contract or output-limit error if the in-memory value no
    /// longer matches its closed representation.
    pub fn canonical_stdout(&self) -> Result<Vec<u8>, LocalReleaseContractError> {
        validate_manifest_inputs(
            &self.target,
            &self.source_commit,
            &self.archive,
            &self.bundle,
            &self.evidence,
        )?;
        if self.value
            != manifest_value(
                &self.target,
                &self.source_commit,
                &self.archive,
                &self.bundle,
                &self.evidence,
            )
        {
            return Err(LocalReleaseContractError::ContractInvalid);
        }
        canonical_bounded(&self.value)
    }
}

/// Parses and reserializes the exact closed release-candidate manifest.
///
/// # Errors
///
/// Returns an archive, evidence, or contract error for malformed,
/// non-canonical, unknown, or out-of-bound bytes.
pub fn parse_local_release_candidate_manifest_v1(
    bytes: &[u8],
) -> Result<LocalReleaseCandidateManifestV1, LocalReleaseContractError> {
    let value = parse_canonical_json(bytes, MAX_LOCAL_RELEASE_PUBLIC_JSON_BYTES)?;
    let target = required_string(&value, "target")?;
    let source = required_object(&value, "source")?;
    let source_commit = required_string_from(source, "commit")?;
    let archive_value = required_object(&value, "archive")?;
    let archive = ReleaseArchiveRecordV1::new(
        required_string_from(archive_value, "path")?,
        required_u64_from(archive_value, "length")?,
        required_string_from(archive_value, "sha256")?,
    );
    let bundle_value = required_object(&value, "embedded_bundle")?;
    let bundle = EmbeddedLocalBundleV1::new(
        required_string_from(bundle_value, "name")?,
        required_string_from(bundle_value, "manifest_sha256")?,
        required_string_from(bundle_value, "binary_sha256")?,
    );
    let evidence = evidence_records_from_value(&value)?;
    let manifest = SelfBuilder::manifest(target, source_commit, archive, bundle, &evidence)?;
    if manifest.value != value {
        return Err(LocalReleaseContractError::ContractInvalid);
    }
    Ok(manifest)
}

struct SelfBuilder;

impl SelfBuilder {
    fn manifest(
        target: &str,
        source_commit: &str,
        archive: ReleaseArchiveRecordV1,
        bundle: EmbeddedLocalBundleV1,
        evidence: &[ReleaseEvidenceRecordV1],
    ) -> Result<LocalReleaseCandidateManifestV1, LocalReleaseContractError> {
        LocalReleaseCandidateManifestV1::new(target, source_commit, archive, bundle, evidence)
    }
}

#[derive(Clone, Debug)]
pub struct LocalReleaseCandidateVerificationV1 {
    value: Value,
}

impl LocalReleaseCandidateVerificationV1 {
    /// Builds the exact offline verification report for one validated
    /// candidate.
    ///
    /// # Errors
    ///
    /// Returns a contract error for malformed digest or candidate identity.
    pub fn new(
        candidate_name: &str,
        manifest: &LocalReleaseCandidateManifestV1,
        manifest_sha256: &str,
        checksums_sha256: &str,
    ) -> Result<Self, LocalReleaseContractError> {
        let expected_name =
            local_release_candidate_name(manifest.target(), manifest.archive().sha256());
        if candidate_name != expected_name
            || !is_sha256(manifest_sha256)
            || !is_sha256(checksums_sha256)
        {
            return Err(LocalReleaseContractError::ContractInvalid);
        }
        let evidence = manifest
            .evidence()
            .iter()
            .map(ReleaseEvidenceRecordV1::value)
            .collect::<Vec<_>>();
        let value = json!({
            "schema_version": LOCAL_RELEASE_CANDIDATE_VERIFICATION_V1_SCHEMA,
            "candidate_name": candidate_name,
            "profile_id": RELEASE_PROFILE_ID,
            "target": manifest.target(),
            "source_commit": manifest.source_commit(),
            "archive_sha256": manifest.archive().sha256(),
            "manifest_sha256": manifest_sha256,
            "checksums_sha256": checksums_sha256,
            "evidence": evidence,
            "verified": true,
            "signature_verification": "external-gh-required",
            "publication": false,
            "support": "none",
            "release_status": "not-ga"
        });
        Ok(Self { value })
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Serializes one canonical LF-terminated verification report.
    ///
    /// # Errors
    ///
    /// Returns an output-limit or internal serialization error.
    pub fn canonical_stdout(&self) -> Result<Vec<u8>, LocalReleaseContractError> {
        canonical_bounded(&self.value)
    }
}

/// Validates the five canonical target-bound supply-chain documents.
///
/// Input paths are candidate-relative evidence paths. Validation is closed,
/// cross-document, privacy-safe, and independent of filesystem or network
/// access.
///
/// # Errors
///
/// Returns a typed evidence, policy, or limit error.
pub fn validate_local_supply_chain_v1(
    target: &str,
    documents: &[(&str, &[u8], &str)],
) -> Result<ValidatedLocalSupplyChainV1, LocalReleaseContractError> {
    if !is_supported_target(target) || documents.len() != LOCAL_RELEASE_EVIDENCE_COUNT {
        return Err(LocalReleaseContractError::InvalidEvidence);
    }
    let total = documents.iter().try_fold(0_usize, |sum, (_, bytes, _)| {
        sum.checked_add(bytes.len())
            .ok_or(LocalReleaseContractError::LimitExceeded)
    })?;
    if total > MAX_LOCAL_RELEASE_EVIDENCE_TOTAL_BYTES {
        return Err(LocalReleaseContractError::LimitExceeded);
    }

    let mut values = BTreeMap::new();
    let mut records = Vec::with_capacity(LOCAL_RELEASE_EVIDENCE_COUNT);
    for (index, (path, bytes, sha256)) in documents.iter().enumerate() {
        let (expected_path, expected_schema) = EXPECTED_EVIDENCE[index];
        if *path != expected_path || !is_sha256(sha256) {
            return Err(LocalReleaseContractError::InvalidEvidence);
        }
        let value = parse_canonical_json(bytes, MAX_LOCAL_RELEASE_EVIDENCE_DOCUMENT_BYTES)?;
        let actual_schema = if expected_schema == CYCLONEDX_1_6_SCHEMA {
            if value.get("bomFormat").and_then(Value::as_str) == Some("CycloneDX")
                && value.get("specVersion").and_then(Value::as_str) == Some("1.6")
            {
                CYCLONEDX_1_6_SCHEMA
            } else {
                return Err(LocalReleaseContractError::InvalidEvidence);
            }
        } else {
            value
                .get("schema_version")
                .and_then(Value::as_str)
                .ok_or(LocalReleaseContractError::InvalidEvidence)?
        };
        if actual_schema != expected_schema {
            return Err(LocalReleaseContractError::InvalidEvidence);
        }
        records.push(ReleaseEvidenceRecordV1::new(
            *path,
            expected_schema,
            u64::try_from(bytes.len()).map_err(|_| LocalReleaseContractError::LimitExceeded)?,
            *sha256,
        ));
        values.insert(*path, value);
    }

    let advisory = values
        .get("evidence/advisory-report.json")
        .ok_or(LocalReleaseContractError::InvalidEvidence)?;
    let cargo_lock_sha256 = validate_advisory(advisory)?.to_owned();
    let dependency = values
        .get("evidence/dependency-lock.json")
        .ok_or(LocalReleaseContractError::InvalidEvidence)?;
    let packages = validate_dependency_lock(dependency, target, &cargo_lock_sha256)?;
    let license = values
        .get("evidence/license-report.json")
        .ok_or(LocalReleaseContractError::InvalidEvidence)?;
    validate_license_report(license, target, &cargo_lock_sha256, &packages)?;
    let unsafe_inventory = values
        .get("evidence/unsafe-inventory.json")
        .ok_or(LocalReleaseContractError::InvalidEvidence)?;
    validate_unsafe_inventory(unsafe_inventory, target, &cargo_lock_sha256, &packages)?;
    let sbom = values
        .get("evidence/sbom.cdx.json")
        .ok_or(LocalReleaseContractError::InvalidEvidence)?;
    validate_sbom(sbom, target, &cargo_lock_sha256, &packages)?;

    Ok(ValidatedLocalSupplyChainV1 {
        target: target.to_owned(),
        cargo_lock_sha256,
        records,
    })
}

#[derive(Clone, Debug)]
pub struct CodeNoesisErrorV28 {
    value: Value,
}

impl CodeNoesisErrorV28 {
    #[must_use]
    pub fn invalid_arguments() -> Self {
        Self::new(
            "release.invalid_arguments",
            "input",
            "invalid release command",
        )
    }

    #[must_use]
    pub fn invalid_bundle() -> Self {
        Self::new(
            "release.invalid_bundle",
            "release",
            "invalid local distribution bundle",
        )
    }

    #[must_use]
    pub fn invalid_evidence() -> Self {
        Self::new(
            "release.invalid_evidence",
            "release",
            "invalid release evidence",
        )
    }

    #[must_use]
    pub fn invalid_archive() -> Self {
        Self::new(
            "release.invalid_archive",
            "release",
            "invalid release archive",
        )
    }

    #[must_use]
    pub fn unstable_input() -> Self {
        Self::new(
            "release.unstable_input",
            "release",
            "unstable release input",
        )
    }

    #[must_use]
    pub fn policy_rejected() -> Self {
        Self::new(
            "release.policy_rejected",
            "release",
            "release policy rejected",
        )
    }

    #[must_use]
    pub fn limit_exceeded() -> Self {
        Self::new(
            "release.limit_exceeded",
            "release",
            "release limit exceeded",
        )
    }

    #[must_use]
    pub fn internal() -> Self {
        Self::new("release.internal", "internal", "release operation failed")
    }

    #[must_use]
    pub fn from_contract(error: LocalReleaseContractError) -> Self {
        match error {
            LocalReleaseContractError::InvalidBundle => Self::invalid_bundle(),
            LocalReleaseContractError::InvalidEvidence => Self::invalid_evidence(),
            LocalReleaseContractError::InvalidArchive => Self::invalid_archive(),
            LocalReleaseContractError::UnstableInput => Self::unstable_input(),
            LocalReleaseContractError::PolicyRejected => Self::policy_rejected(),
            LocalReleaseContractError::LimitExceeded => Self::limit_exceeded(),
            LocalReleaseContractError::ContractInvalid => Self::internal(),
        }
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Serializes the exact public error as canonical JSON plus LF.
    ///
    /// # Errors
    ///
    /// Returns the underlying JSON serialization error.
    pub fn canonical_stderr(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = serde_json::to_vec(&self.value)?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    fn new(code: &str, stage: &str, message: &str) -> Self {
        Self {
            value: json!({
                "schema_version": CODENOESIS_ERROR_V28_SCHEMA,
                "code": code,
                "stage": stage,
                "message": message,
                "retryable": false,
                "context": {}
            }),
        }
    }
}

#[must_use]
pub fn local_release_candidate_name(target: &str, archive_sha256: &str) -> String {
    format!("codenoesis-local-release-candidate-r17-{target}-{archive_sha256}")
}

#[must_use]
pub fn local_release_archive_name(target: &str, binary_sha256: &str) -> String {
    format!(
        "{}.zip",
        local_distribution_bundle_name(target, binary_sha256)
    )
}

#[must_use]
pub const fn is_supported_local_release_target(target: &str) -> bool {
    is_supported_target(target)
}

fn validate_manifest_inputs(
    target: &str,
    source_commit: &str,
    archive: &ReleaseArchiveRecordV1,
    bundle: &EmbeddedLocalBundleV1,
    evidence: &[ReleaseEvidenceRecordV1],
) -> Result<(), LocalReleaseContractError> {
    if !is_supported_target(target)
        || !is_git_sha(source_commit)
        || archive.length == 0
        || archive.length > MAX_LOCAL_RELEASE_ARCHIVE_BYTES
        || !is_sha256(&archive.sha256)
        || !is_sha256(&bundle.manifest_sha256)
        || !is_sha256(&bundle.binary_sha256)
        || archive.path != local_release_archive_name(target, &bundle.binary_sha256)
        || bundle.name != local_distribution_bundle_name(target, &bundle.binary_sha256)
    {
        return Err(if archive.length > MAX_LOCAL_RELEASE_ARCHIVE_BYTES {
            LocalReleaseContractError::LimitExceeded
        } else {
            LocalReleaseContractError::ContractInvalid
        });
    }
    validate_evidence_records(evidence)
}

fn validate_evidence_records(
    evidence: &[ReleaseEvidenceRecordV1],
) -> Result<(), LocalReleaseContractError> {
    if evidence.len() != LOCAL_RELEASE_EVIDENCE_COUNT {
        return Err(LocalReleaseContractError::InvalidEvidence);
    }
    for (record, (path, schema)) in evidence.iter().zip(EXPECTED_EVIDENCE) {
        if record.path != path
            || record.schema != schema
            || record.length == 0
            || record.length
                > u64::try_from(MAX_LOCAL_RELEASE_EVIDENCE_DOCUMENT_BYTES).unwrap_or(u64::MAX)
            || !is_sha256(&record.sha256)
        {
            return Err(LocalReleaseContractError::InvalidEvidence);
        }
    }
    Ok(())
}

fn manifest_value(
    target: &str,
    source_commit: &str,
    archive: &ReleaseArchiveRecordV1,
    bundle: &EmbeddedLocalBundleV1,
    evidence: &[ReleaseEvidenceRecordV1],
) -> Value {
    json!({
        "schema_version": LOCAL_RELEASE_CANDIDATE_MANIFEST_V1_SCHEMA,
        "profile_id": RELEASE_PROFILE_ID,
        "carrier": "deterministic-zip-stored-v1",
        "target": target,
        "source": {
            "repository": SOURCE_REPOSITORY,
            "commit": source_commit,
            "ref": SOURCE_REF
        },
        "release_status": "not-ga",
        "support": "none",
        "publication": false,
        "runtime_profile_release_authority_unchanged": true,
        "archive": {
            "path": archive.path,
            "length": archive.length,
            "sha256": archive.sha256,
            "entries": 6
        },
        "embedded_bundle": {
            "name": bundle.name,
            "manifest_sha256": bundle.manifest_sha256,
            "binary_sha256": bundle.binary_sha256
        },
        "evidence": evidence.iter().map(ReleaseEvidenceRecordV1::value).collect::<Vec<_>>(),
        "attestation": {
            "provider": "github-artifact-attestations",
            "predicate_type": "https://slsa.dev/provenance/v1",
            "required_for_distribution": true,
            "included_in_candidate": false
        },
        "reproducibility": {
            "fixed_inputs": "byte-identical",
            "advisory_observation": "time-varying-not-claimed"
        }
    })
}

fn evidence_records_from_value(
    value: &Value,
) -> Result<Vec<ReleaseEvidenceRecordV1>, LocalReleaseContractError> {
    value
        .get("evidence")
        .and_then(Value::as_array)
        .ok_or(LocalReleaseContractError::InvalidEvidence)?
        .iter()
        .map(|record| {
            Ok(ReleaseEvidenceRecordV1::new(
                required_string(record, "path")?,
                required_string(record, "schema")?,
                required_u64(record, "length")?,
                required_string(record, "sha256")?,
            ))
        })
        .collect()
}

#[derive(Clone, Debug)]
struct LockedPackage {
    name: String,
    version: String,
    source: String,
    checksum: Option<String>,
    dependencies: Vec<String>,
}

fn validate_advisory(value: &Value) -> Result<&str, LocalReleaseContractError> {
    require_keys(
        value,
        &[
            "schema_version",
            "cargo_lock_sha256",
            "tool",
            "database",
            "status",
            "vulnerabilities",
            "warnings",
        ],
    )?;
    if required_string(value, "schema_version")? != LOCAL_ADVISORY_REPORT_V1_SCHEMA
        || required_string(value, "status")? != "accepted"
    {
        return Err(LocalReleaseContractError::InvalidEvidence);
    }
    let lock = required_string(value, "cargo_lock_sha256")?;
    if !is_sha256(lock) {
        return Err(LocalReleaseContractError::InvalidEvidence);
    }
    let tool = required_object(value, "tool")?;
    require_keys_map(tool, &["name", "version"])?;
    if required_string_from(tool, "name")? != "cargo-audit"
        || required_string_from(tool, "version")? != "0.22.2"
    {
        return Err(LocalReleaseContractError::PolicyRejected);
    }
    let database = required_object(value, "database")?;
    require_keys_map(database, &["commit", "updated"])?;
    if !is_git_sha(required_string_from(database, "commit")?)
        || required_string_from(database, "updated")?.is_empty()
        || required_string_from(database, "updated")?.len() > 64
    {
        return Err(LocalReleaseContractError::InvalidEvidence);
    }
    let vulnerabilities = required_array(value, "vulnerabilities")?;
    if !vulnerabilities.is_empty() {
        return Err(LocalReleaseContractError::PolicyRejected);
    }
    let warnings = required_array(value, "warnings")?;
    if warnings.len() > MAX_LOCAL_RELEASE_PACKAGES {
        return Err(LocalReleaseContractError::LimitExceeded);
    }
    Ok(lock)
}

fn validate_dependency_lock(
    value: &Value,
    target: &str,
    cargo_lock_sha256: &str,
) -> Result<BTreeMap<String, LockedPackage>, LocalReleaseContractError> {
    require_keys(
        value,
        &[
            "schema_version",
            "target",
            "root",
            "cargo_lock_sha256",
            "packages",
            "dependency_edges",
        ],
    )?;
    if required_string(value, "schema_version")? != LOCAL_DEPENDENCY_LOCK_V1_SCHEMA
        || required_string(value, "target")? != target
        || required_string(value, "root")? != "noesis@0.1.0"
        || required_string(value, "cargo_lock_sha256")? != cargo_lock_sha256
    {
        return Err(LocalReleaseContractError::InvalidEvidence);
    }
    let package_values = required_array(value, "packages")?;
    if package_values.is_empty() || package_values.len() > MAX_LOCAL_RELEASE_PACKAGES {
        return Err(LocalReleaseContractError::LimitExceeded);
    }
    let mut packages = BTreeMap::new();
    let mut dependency_edges = 0_u64;
    for package in package_values {
        let (id, locked) = locked_package_from_value(package)?;
        dependency_edges = dependency_edges
            .checked_add(u64::try_from(locked.dependencies.len()).unwrap_or(u64::MAX))
            .ok_or(LocalReleaseContractError::LimitExceeded)?;
        if packages.insert(id, locked).is_some() {
            return Err(LocalReleaseContractError::InvalidEvidence);
        }
    }
    if !packages.contains_key("noesis@0.1.0")
        || dependency_edges > MAX_LOCAL_RELEASE_DEPENDENCY_EDGES
        || required_u64(value, "dependency_edges")? != dependency_edges
        || packages
            .values()
            .flat_map(|package| &package.dependencies)
            .any(|dependency| !packages.contains_key(dependency))
    {
        return Err(LocalReleaseContractError::InvalidEvidence);
    }
    let listed_ids = package_values
        .iter()
        .map(|package| required_string(package, "id").map(str::to_owned))
        .collect::<Result<Vec<_>, _>>()?;
    if !is_sorted_unique(&listed_ids) || listed_ids != packages.keys().cloned().collect::<Vec<_>>()
    {
        return Err(LocalReleaseContractError::InvalidEvidence);
    }
    Ok(packages)
}

fn locked_package_from_value(
    package: &Value,
) -> Result<(String, LockedPackage), LocalReleaseContractError> {
    require_keys(
        package,
        &[
            "id",
            "name",
            "version",
            "source",
            "checksum",
            "dependencies",
        ],
    )?;
    let id = required_string(package, "id")?;
    let name = required_string(package, "name")?;
    let version = required_string(package, "version")?;
    let source = required_string(package, "source")?;
    if id != format!("{name}@{version}")
        || id.len() > MAX_LOCAL_RELEASE_RELATIVE_PATH_BYTES
        || name.is_empty()
        || name.len() > 128
        || version.is_empty()
        || version.len() > 128
        || !matches!(
            source,
            "workspace" | "registry+https://github.com/rust-lang/crates.io-index"
        )
    {
        return Err(LocalReleaseContractError::PolicyRejected);
    }
    let checksum = match package.get("checksum") {
        Some(Value::Null) if source == "workspace" => None,
        Some(Value::String(checksum)) if source != "workspace" && is_sha256(checksum) => {
            Some(checksum.clone())
        }
        _ => return Err(LocalReleaseContractError::InvalidEvidence),
    };
    let dependencies = required_array(package, "dependencies")?
        .iter()
        .map(|dependency| {
            dependency
                .as_str()
                .map(str::to_owned)
                .ok_or(LocalReleaseContractError::InvalidEvidence)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if !is_sorted_unique(&dependencies) {
        return Err(LocalReleaseContractError::InvalidEvidence);
    }
    Ok((
        id.to_owned(),
        LockedPackage {
            name: name.to_owned(),
            version: version.to_owned(),
            source: source.to_owned(),
            checksum,
            dependencies,
        },
    ))
}

fn validate_license_report(
    value: &Value,
    target: &str,
    cargo_lock_sha256: &str,
    packages: &BTreeMap<String, LockedPackage>,
) -> Result<(), LocalReleaseContractError> {
    require_keys(
        value,
        &[
            "schema_version",
            "target",
            "cargo_lock_sha256",
            "policy",
            "status",
            "packages",
            "exceptions",
        ],
    )?;
    if required_string(value, "schema_version")? != LOCAL_LICENSE_REPORT_V1_SCHEMA
        || required_string(value, "target")? != target
        || required_string(value, "cargo_lock_sha256")? != cargo_lock_sha256
        || required_string(value, "policy")? != "codenoesis.local-release-policy/v1"
        || required_string(value, "status")? != "accepted"
        || !required_array(value, "exceptions")?.is_empty()
    {
        return Err(LocalReleaseContractError::PolicyRejected);
    }
    let rows = required_array(value, "packages")?;
    if rows.len() != packages.len() {
        return Err(LocalReleaseContractError::InvalidEvidence);
    }
    let mut ids = Vec::with_capacity(rows.len());
    for row in rows {
        require_keys(row, &["id", "expression", "decision"])?;
        let id = required_string(row, "id")?;
        let expression = required_string(row, "expression")?;
        if !packages.contains_key(id)
            || required_string(row, "decision")? != "allowed"
            || !ALLOWED_LICENSE_EXPRESSIONS.contains(&expression)
        {
            return Err(LocalReleaseContractError::PolicyRejected);
        }
        ids.push(id.to_owned());
    }
    if !is_sorted_unique(&ids) || ids != packages.keys().cloned().collect::<Vec<_>>() {
        return Err(LocalReleaseContractError::InvalidEvidence);
    }
    Ok(())
}

fn validate_unsafe_inventory(
    value: &Value,
    target: &str,
    cargo_lock_sha256: &str,
    packages: &BTreeMap<String, LockedPackage>,
) -> Result<(), LocalReleaseContractError> {
    require_keys(
        value,
        &[
            "schema_version",
            "target",
            "cargo_lock_sha256",
            "method",
            "status",
            "packages",
            "exceptions",
        ],
    )?;
    if required_string(value, "schema_version")? != LOCAL_UNSAFE_INVENTORY_V1_SCHEMA
        || required_string(value, "target")? != target
        || required_string(value, "cargo_lock_sha256")? != cargo_lock_sha256
        || required_string(value, "method")? != "conservative-rust-token-scan-v1"
        || required_string(value, "status")? != "accepted"
    {
        return Err(LocalReleaseContractError::InvalidEvidence);
    }
    let exception_values = required_array(value, "exceptions")?;
    if exception_values.len() > MAX_LOCAL_RELEASE_UNSAFE_EXCEPTIONS {
        return Err(LocalReleaseContractError::LimitExceeded);
    }
    let mut exceptions = BTreeMap::new();
    let mut exception_ids = Vec::with_capacity(exception_values.len());
    for exception in exception_values {
        require_keys(
            exception,
            &["id", "package", "version", "owner", "expires_on"],
        )?;
        let id = required_string(exception, "id")?;
        let package = required_string(exception, "package")?;
        let version = required_string(exception, "version")?;
        let expires_on = required_string(exception, "expires_on")?;
        if required_string(exception, "owner")? != "@smutti"
            || expires_on < POLICY_REVIEW_DATE
            || !is_date(expires_on)
            || exceptions
                .insert(id.to_owned(), (package.to_owned(), version.to_owned()))
                .is_some()
        {
            return Err(LocalReleaseContractError::PolicyRejected);
        }
        exception_ids.push(id.to_owned());
    }
    if !is_sorted_unique(&exception_ids) {
        return Err(LocalReleaseContractError::InvalidEvidence);
    }

    let rows = required_array(value, "packages")?;
    if rows.len() != packages.len() {
        return Err(LocalReleaseContractError::InvalidEvidence);
    }
    let mut ids = Vec::with_capacity(rows.len());
    let mut used_exceptions = BTreeSet::new();
    for row in rows {
        require_keys(row, &["id", "rust_files", "unsafe_tokens", "exception_id"])?;
        let id = required_string(row, "id")?;
        let package = packages
            .get(id)
            .ok_or(LocalReleaseContractError::InvalidEvidence)?;
        let unsafe_tokens = required_u64(row, "unsafe_tokens")?;
        let _rust_files = required_u64(row, "rust_files")?;
        let exception_id = match row.get("exception_id") {
            Some(Value::Null) => None,
            Some(Value::String(value)) => Some(value.as_str()),
            _ => return Err(LocalReleaseContractError::InvalidEvidence),
        };
        if package.source == "workspace" {
            if unsafe_tokens != 0 || exception_id.is_some() {
                return Err(LocalReleaseContractError::PolicyRejected);
            }
        } else if unsafe_tokens == 0 {
            if exception_id.is_some() {
                return Err(LocalReleaseContractError::PolicyRejected);
            }
        } else {
            let exception_id = exception_id.ok_or(LocalReleaseContractError::PolicyRejected)?;
            let identity = exceptions
                .get(exception_id)
                .ok_or(LocalReleaseContractError::PolicyRejected)?;
            if identity != &(package.name.clone(), package.version.clone()) {
                return Err(LocalReleaseContractError::PolicyRejected);
            }
            used_exceptions.insert(exception_id.to_owned());
        }
        ids.push(id.to_owned());
    }
    if !is_sorted_unique(&ids)
        || ids != packages.keys().cloned().collect::<Vec<_>>()
        || used_exceptions != exceptions.keys().cloned().collect()
    {
        return Err(LocalReleaseContractError::PolicyRejected);
    }
    Ok(())
}

fn validate_sbom(
    value: &Value,
    target: &str,
    cargo_lock_sha256: &str,
    packages: &BTreeMap<String, LockedPackage>,
) -> Result<(), LocalReleaseContractError> {
    require_keys(
        value,
        &[
            "bomFormat",
            "specVersion",
            "serialNumber",
            "version",
            "metadata",
            "components",
            "dependencies",
        ],
    )?;
    if required_string(value, "bomFormat")? != "CycloneDX"
        || required_string(value, "specVersion")? != "1.6"
        || required_u64(value, "version")? != 1
        || !required_string(value, "serialNumber")?.starts_with("urn:uuid:")
    {
        return Err(LocalReleaseContractError::InvalidEvidence);
    }
    validate_sbom_metadata(
        required_object(value, "metadata")?,
        target,
        cargo_lock_sha256,
    )?;
    validate_sbom_components(required_array(value, "components")?, packages)?;
    validate_sbom_dependencies(required_array(value, "dependencies")?, packages)
}

fn validate_sbom_metadata(
    metadata: &Map<String, Value>,
    target: &str,
    cargo_lock_sha256: &str,
) -> Result<(), LocalReleaseContractError> {
    require_keys_map(metadata, &["component", "properties"])?;
    let component = required_object_from(metadata, "component")?;
    require_keys_map(component, &["type", "bom-ref", "name", "version"])?;
    if required_string_from(component, "type")? != "application"
        || required_string_from(component, "bom-ref")? != "pkg:cargo/noesis@0.1.0"
        || required_string_from(component, "name")? != "noesis"
        || required_string_from(component, "version")? != "0.1.0"
    {
        return Err(LocalReleaseContractError::InvalidEvidence);
    }
    let properties = required_array_from(metadata, "properties")?;
    if properties.as_slice()
        != [
            json!({"name": "codenoesis:cargo-lock-sha256", "value": cargo_lock_sha256}),
            json!({"name": "codenoesis:target", "value": target}),
        ]
    {
        return Err(LocalReleaseContractError::InvalidEvidence);
    }
    Ok(())
}

fn validate_sbom_components(
    components: &[Value],
    packages: &BTreeMap<String, LockedPackage>,
) -> Result<(), LocalReleaseContractError> {
    if components.len() + 1 != packages.len() || components.len() > MAX_LOCAL_RELEASE_PACKAGES {
        return Err(LocalReleaseContractError::InvalidEvidence);
    }
    let mut component_ids = Vec::with_capacity(components.len());
    for sbom_component in components {
        let name = required_string(sbom_component, "name")?;
        let version = required_string(sbom_component, "version")?;
        let id = format!("{name}@{version}");
        let package = packages
            .get(&id)
            .ok_or(LocalReleaseContractError::InvalidEvidence)?;
        if id == "noesis@0.1.0"
            || required_string(sbom_component, "type")? != "library"
            || required_string(sbom_component, "bom-ref")? != format!("pkg:cargo/{name}@{version}")
            || required_string(sbom_component, "purl")? != format!("pkg:cargo/{name}@{version}")
        {
            return Err(LocalReleaseContractError::InvalidEvidence);
        }
        match &package.checksum {
            Some(checksum) => {
                if sbom_component.get("hashes")
                    != Some(&json!([{"alg": "SHA-256", "content": checksum}]))
                {
                    return Err(LocalReleaseContractError::InvalidEvidence);
                }
            }
            None => {
                if sbom_component.get("hashes").is_some() {
                    return Err(LocalReleaseContractError::InvalidEvidence);
                }
            }
        }
        let licenses = required_array(sbom_component, "licenses")?;
        if licenses.len() != 1
            || licenses[0]
                .get("expression")
                .and_then(Value::as_str)
                .is_none_or(|expression| !ALLOWED_LICENSE_EXPRESSIONS.contains(&expression))
        {
            return Err(LocalReleaseContractError::PolicyRejected);
        }
        component_ids.push(id);
    }
    if !is_sorted_unique(&component_ids) {
        return Err(LocalReleaseContractError::InvalidEvidence);
    }
    Ok(())
}

fn validate_sbom_dependencies(
    dependency_values: &[Value],
    packages: &BTreeMap<String, LockedPackage>,
) -> Result<(), LocalReleaseContractError> {
    if dependency_values.len() != packages.len() {
        return Err(LocalReleaseContractError::InvalidEvidence);
    }
    let mut dependency_ids = Vec::with_capacity(dependency_values.len());
    for dependency in dependency_values {
        require_keys(dependency, &["ref", "dependsOn"])?;
        let reference = required_string(dependency, "ref")?;
        let id = reference
            .strip_prefix("pkg:cargo/")
            .ok_or(LocalReleaseContractError::InvalidEvidence)?;
        let package = packages
            .get(id)
            .ok_or(LocalReleaseContractError::InvalidEvidence)?;
        let depends_on = required_array(dependency, "dependsOn")?
            .iter()
            .map(|reference| {
                reference
                    .as_str()
                    .and_then(|value| value.strip_prefix("pkg:cargo/"))
                    .map(str::to_owned)
                    .ok_or(LocalReleaseContractError::InvalidEvidence)
            })
            .collect::<Result<Vec<_>, _>>()?;
        if depends_on != package.dependencies {
            return Err(LocalReleaseContractError::InvalidEvidence);
        }
        dependency_ids.push(id.to_owned());
    }
    if !is_sorted_unique(&dependency_ids)
        || dependency_ids != packages.keys().cloned().collect::<Vec<_>>()
    {
        return Err(LocalReleaseContractError::InvalidEvidence);
    }
    Ok(())
}

fn canonical_bounded(value: &Value) -> Result<Vec<u8>, LocalReleaseContractError> {
    let mut bytes =
        serde_json::to_vec(value).map_err(|_| LocalReleaseContractError::ContractInvalid)?;
    bytes.push(b'\n');
    if bytes.len() > MAX_LOCAL_RELEASE_PUBLIC_JSON_BYTES {
        return Err(LocalReleaseContractError::LimitExceeded);
    }
    Ok(bytes)
}

fn parse_canonical_json(bytes: &[u8], maximum: usize) -> Result<Value, LocalReleaseContractError> {
    if bytes.is_empty() || bytes.len() > maximum || contains_private_material(bytes) {
        return Err(if bytes.len() > maximum {
            LocalReleaseContractError::LimitExceeded
        } else {
            LocalReleaseContractError::InvalidEvidence
        });
    }
    let value: Value =
        serde_json::from_slice(bytes).map_err(|_| LocalReleaseContractError::InvalidEvidence)?;
    let mut canonical =
        serde_json::to_vec(&value).map_err(|_| LocalReleaseContractError::ContractInvalid)?;
    canonical.push(b'\n');
    if canonical != bytes {
        return Err(LocalReleaseContractError::InvalidEvidence);
    }
    Ok(value)
}

fn contains_private_material(bytes: &[u8]) -> bool {
    [
        b"/Users/".as_slice(),
        b"\\\\Users\\\\".as_slice(),
        b"credential-private-canary".as_slice(),
        b"private.invalid".as_slice(),
        b"file://".as_slice(),
    ]
    .iter()
    .any(|needle| bytes.windows(needle.len()).any(|window| window == *needle))
}

fn require_keys(value: &Value, keys: &[&str]) -> Result<(), LocalReleaseContractError> {
    let object = value
        .as_object()
        .ok_or(LocalReleaseContractError::InvalidEvidence)?;
    require_keys_map(object, keys)
}

fn require_keys_map(
    object: &Map<String, Value>,
    keys: &[&str],
) -> Result<(), LocalReleaseContractError> {
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = keys.iter().copied().collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(LocalReleaseContractError::InvalidEvidence);
    }
    Ok(())
}

fn required_object<'a>(
    value: &'a Value,
    key: &str,
) -> Result<&'a Map<String, Value>, LocalReleaseContractError> {
    value
        .get(key)
        .and_then(Value::as_object)
        .ok_or(LocalReleaseContractError::InvalidEvidence)
}

fn required_object_from<'a>(
    value: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a Map<String, Value>, LocalReleaseContractError> {
    value
        .get(key)
        .and_then(Value::as_object)
        .ok_or(LocalReleaseContractError::InvalidEvidence)
}

fn required_array<'a>(
    value: &'a Value,
    key: &str,
) -> Result<&'a Vec<Value>, LocalReleaseContractError> {
    value
        .get(key)
        .and_then(Value::as_array)
        .ok_or(LocalReleaseContractError::InvalidEvidence)
}

fn required_array_from<'a>(
    value: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a Vec<Value>, LocalReleaseContractError> {
    value
        .get(key)
        .and_then(Value::as_array)
        .ok_or(LocalReleaseContractError::InvalidEvidence)
}

fn required_string<'a>(value: &'a Value, key: &str) -> Result<&'a str, LocalReleaseContractError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or(LocalReleaseContractError::InvalidEvidence)
}

fn required_string_from<'a>(
    value: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a str, LocalReleaseContractError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or(LocalReleaseContractError::InvalidEvidence)
}

fn required_u64(value: &Value, key: &str) -> Result<u64, LocalReleaseContractError> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or(LocalReleaseContractError::InvalidEvidence)
}

fn required_u64_from(
    value: &Map<String, Value>,
    key: &str,
) -> Result<u64, LocalReleaseContractError> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or(LocalReleaseContractError::InvalidEvidence)
}

const fn is_supported_target(target: &str) -> bool {
    matches!(
        target.as_bytes(),
        b"aarch64-apple-darwin" | b"x86_64-pc-windows-msvc" | b"x86_64-unknown-linux-gnu"
    )
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_git_sha(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_date(value: &str) -> bool {
    value.len() == 10
        && value.bytes().enumerate().all(|(index, byte)| match index {
            4 | 7 => byte == b'-',
            _ => byte.is_ascii_digit(),
        })
}

fn is_sorted_unique(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
