use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use codenoesis_domain::s7::SemanticCompatibilityReport;
use codenoesis_domain::{ObjectId, RepositoryIdentity, RepositoryInventory};
use serde_json::{Map, Value, json};

use super::SemanticCompatibilityReportV1;

pub const R19_WORKSPACE_VERSION: &str = "codenoesis.impact-git-workspace/v1";
pub const R19_REPORT_VERSION: &str = "codenoesis.semantic-compatibility-report/v2";
pub const R19_PIPELINE_VERSION: &str = "codenoesis.pipeline/s7-git-v1";
pub const R19_EVIDENCE_LINEAGE_VERSION: &str = "codenoesis.source-evidence/git-v1";
pub const R19_ANALYSIS_PROFILE: &str = "implementation-aware-http-json-git-v1";
pub const R19_SOURCE_PROFILE: &str = "trusted-local-impact-source-v1";
pub const R19_SOURCE_RESULT_VERSION: &str = "codenoesis.trusted-impact-source-excerpt/v1";
pub const R19_ERROR_VERSION: &str = "codenoesis.error/v30";
pub const MAX_R19_WORKSPACE_BYTES: u64 = 1_048_576;
pub const MAX_R19_REPORT_BYTES: u64 = 67_108_864;
pub const MAX_R19_FEDERATION_BYTES: u64 = 67_108_864;
pub const MAX_R19_SOURCE_BYTES_PER_FILE: u64 = 2_097_152;
pub const MAX_R19_TOTAL_SOURCE_BYTES: u64 = 268_435_456;
pub const MAX_R19_PATH_BYTES: usize = 1_024;
pub const MAX_R19_SYMBOL_BYTES: usize = 1_024;
pub const MAX_R19_CLIENTS: usize = 32;
pub const MAX_R19_EXCERPT_BYTES: u64 = 262_144;
pub const MAX_R19_EXCERPT_STDOUT_BYTES: u64 = 524_288;

pub type R19Sha256 = fn(&[u8]) -> String;

const CONTRACT_CAPABILITY: &str = "codenoesis.contract-capability/openapi-3.1-http-json/v1";
const PROVIDER_CAPABILITY: &str = "rust-direct-json-map/v1";
const CLIENT_CAPABILITY: &str = "kotlin-direct-json-access/v1";
const CONFIGURATION_HASH_DOMAIN: &[u8] = b"codenoesis.impact-git.configuration.v1";
const LIMITATIONS: [&str; 6] = [
    "exact_committed_bytes_only",
    "no_context_expansion",
    "no_retention_or_export",
    "no_working_tree_fallback",
    "single_evidence_only",
    "utf8_only",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImpactGitWorkspaceV1 {
    pub provider: ImpactGitProviderInput,
    pub clients: Vec<ImpactGitClientInput>,
    pub federation_report: ImpactGitBoundFile,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImpactGitProviderInput {
    pub repository_identity: String,
    pub root: String,
    pub baseline: ImpactGitRevisionInput,
    pub target: ImpactGitRevisionInput,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImpactGitRevisionInput {
    pub revision: String,
    pub federation_revision: String,
    pub contract_path: String,
    pub source_path: String,
    pub callable_symbol: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImpactGitClientInput {
    pub role: String,
    pub repository_identity: String,
    pub root: String,
    pub revision: String,
    pub federation_revision: String,
    pub source_path: String,
    pub decoder_symbol: String,
    pub call_symbol: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImpactGitBoundFile {
    pub path: String,
    pub sha256: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImpactGitWorkspaceError {
    Invalid,
    TooManyClients { observed: u64 },
}

/// Parses the closed Git-backed S7 workspace authority.
///
/// # Errors
///
/// Returns a closed malformed, path, identity, revision, duplicate, or
/// cardinality failure.
pub fn parse_impact_git_workspace(
    bytes: &[u8],
) -> Result<ImpactGitWorkspaceV1, ImpactGitWorkspaceError> {
    let value: Value =
        serde_json::from_slice(bytes).map_err(|_| ImpactGitWorkspaceError::Invalid)?;
    let root = exact_object(
        &value,
        &[
            "analysis_profile",
            "client_capability",
            "clients",
            "contract_capability",
            "federation_report",
            "pipeline",
            "provider",
            "provider_capability",
            "schema_version",
        ],
    )?;
    require_const(root, "schema_version", R19_WORKSPACE_VERSION)?;
    require_const(root, "analysis_profile", R19_ANALYSIS_PROFILE)?;
    require_const(root, "pipeline", R19_PIPELINE_VERSION)?;
    require_const(root, "contract_capability", CONTRACT_CAPABILITY)?;
    require_const(root, "provider_capability", PROVIDER_CAPABILITY)?;
    require_const(root, "client_capability", CLIENT_CAPABILITY)?;

    let provider = parse_provider(
        root.get("provider")
            .ok_or(ImpactGitWorkspaceError::Invalid)?,
    )?;
    let clients = root
        .get("clients")
        .and_then(Value::as_array)
        .filter(|clients| !clients.is_empty())
        .ok_or(ImpactGitWorkspaceError::Invalid)?;
    if clients.len() > MAX_R19_CLIENTS {
        return Err(ImpactGitWorkspaceError::TooManyClients {
            observed: u64::try_from(clients.len()).unwrap_or(u64::MAX),
        });
    }
    let clients = clients
        .iter()
        .map(parse_client)
        .collect::<Result<Vec<_>, _>>()?;
    let mut identities = BTreeSet::new();
    let mut roots = BTreeSet::new();
    for client in &clients {
        if client.repository_identity == provider.repository_identity
            || !identities.insert(client.repository_identity.as_str())
            || !roots.insert(client.root.as_str())
        {
            return Err(ImpactGitWorkspaceError::Invalid);
        }
    }
    if roots.contains(provider.root.as_str()) {
        return Err(ImpactGitWorkspaceError::Invalid);
    }
    let federation_report = parse_bound_file(
        root.get("federation_report")
            .ok_or(ImpactGitWorkspaceError::Invalid)?,
    )?;
    Ok(ImpactGitWorkspaceV1 {
        provider,
        clients,
        federation_report,
    })
}

fn parse_provider(value: &Value) -> Result<ImpactGitProviderInput, ImpactGitWorkspaceError> {
    let object = exact_object(
        value,
        &["baseline", "repository_identity", "root", "target"],
    )?;
    let repository_identity = required_identity(object, "repository_identity")?;
    let root = required_path(object, "root")?;
    let baseline = parse_revision(
        object
            .get("baseline")
            .ok_or(ImpactGitWorkspaceError::Invalid)?,
    )?;
    let target = parse_revision(
        object
            .get("target")
            .ok_or(ImpactGitWorkspaceError::Invalid)?,
    )?;
    if baseline.revision == target.revision
        || baseline.contract_path != target.contract_path
        || baseline.callable_symbol != target.callable_symbol
    {
        return Err(ImpactGitWorkspaceError::Invalid);
    }
    Ok(ImpactGitProviderInput {
        repository_identity,
        root,
        baseline,
        target,
    })
}

fn parse_revision(value: &Value) -> Result<ImpactGitRevisionInput, ImpactGitWorkspaceError> {
    let object = exact_object(
        value,
        &[
            "callable_symbol",
            "contract_path",
            "federation_revision",
            "revision",
            "source_path",
        ],
    )?;
    Ok(ImpactGitRevisionInput {
        revision: required_commit(object, "revision")?,
        federation_revision: required_bounded(object, "federation_revision", 255)?,
        contract_path: required_path(object, "contract_path")?,
        source_path: required_path(object, "source_path")?,
        callable_symbol: required_bounded(object, "callable_symbol", MAX_R19_SYMBOL_BYTES)?,
    })
}

fn parse_client(value: &Value) -> Result<ImpactGitClientInput, ImpactGitWorkspaceError> {
    let object = exact_object(
        value,
        &[
            "call_symbol",
            "decoder_symbol",
            "federation_revision",
            "repository_identity",
            "revision",
            "role",
            "root",
            "source_path",
        ],
    )?;
    Ok(ImpactGitClientInput {
        role: required_bounded(object, "role", 64)?,
        repository_identity: required_identity(object, "repository_identity")?,
        root: required_path(object, "root")?,
        revision: required_commit(object, "revision")?,
        federation_revision: required_bounded(object, "federation_revision", 255)?,
        source_path: required_path(object, "source_path")?,
        decoder_symbol: required_bounded(object, "decoder_symbol", MAX_R19_SYMBOL_BYTES)?,
        call_symbol: required_bounded(object, "call_symbol", MAX_R19_SYMBOL_BYTES)?,
    })
}

fn parse_bound_file(value: &Value) -> Result<ImpactGitBoundFile, ImpactGitWorkspaceError> {
    let object = exact_object(value, &["path", "sha256"])?;
    let sha256 = required_bounded(object, "sha256", 64)?;
    if !valid_sha256(&sha256) {
        return Err(ImpactGitWorkspaceError::Invalid);
    }
    Ok(ImpactGitBoundFile {
        path: required_path(object, "path")?,
        sha256,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitImpactSourceFile {
    pub repository_identity: String,
    pub commit_oid: String,
    pub tree_oid: String,
    pub path: String,
    pub blob_oid: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct SemanticCompatibilityReportV2 {
    value: Value,
    canonical: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticReportV2Error {
    Invalid,
    LimitExceeded,
    Serialization,
}

impl SemanticCompatibilityReportV2 {
    /// Enriches one already validated S7 domain report with exact Git bindings.
    ///
    /// # Errors
    ///
    /// Returns a closed semantic, source-binding, UTF-8, digest, ordering, or
    /// output-limit failure.
    pub fn from_domain(
        report: &SemanticCompatibilityReport,
        sources: &[GitImpactSourceFile],
        sha256: R19Sha256,
    ) -> Result<Self, SemanticReportV2Error> {
        let v1 = SemanticCompatibilityReportV1::from_domain(report)
            .map_err(|_| SemanticReportV2Error::Invalid)?
            .canonical_stdout()
            .map_err(|_| SemanticReportV2Error::Invalid)?;
        let mut value: Value =
            serde_json::from_slice(&v1).map_err(|_| SemanticReportV2Error::Serialization)?;
        let root = value
            .as_object_mut()
            .ok_or(SemanticReportV2Error::Invalid)?;
        root.insert(
            "schema_version".to_owned(),
            Value::String(R19_REPORT_VERSION.to_owned()),
        );
        root.insert(
            "analysis_profile".to_owned(),
            Value::String(R19_ANALYSIS_PROFILE.to_owned()),
        );
        root.insert(
            "pipeline_version".to_owned(),
            Value::String(R19_PIPELINE_VERSION.to_owned()),
        );
        root.insert(
            "evidence_lineage_version".to_owned(),
            Value::String(R19_EVIDENCE_LINEAGE_VERSION.to_owned()),
        );
        root.insert(
            "configuration_hash".to_owned(),
            Value::String(configuration_hash()),
        );

        let evidence = root
            .get_mut("evidence")
            .and_then(Value::as_array_mut)
            .ok_or(SemanticReportV2Error::Invalid)?;
        for record in evidence {
            bind_evidence(record, sources, sha256)?;
        }
        validate_report_value(&value)?;
        let canonical =
            serde_json::to_vec(&value).map_err(|_| SemanticReportV2Error::Serialization)?;
        let observed = u64::try_from(canonical.len().saturating_add(1)).unwrap_or(u64::MAX);
        if observed > MAX_R19_REPORT_BYTES {
            return Err(SemanticReportV2Error::LimitExceeded);
        }
        Ok(Self { value, canonical })
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    #[must_use]
    pub fn canonical_stdout(&self) -> Vec<u8> {
        let mut bytes = self.canonical.clone();
        bytes.push(b'\n');
        bytes
    }
}

fn bind_evidence(
    record: &mut Value,
    sources: &[GitImpactSourceFile],
    sha256: R19Sha256,
) -> Result<(), SemanticReportV2Error> {
    let object = record
        .as_object_mut()
        .ok_or(SemanticReportV2Error::Invalid)?;
    let repository_identity = string_value(object, "repository_identity")?;
    let revision = string_value(object, "revision")?;
    let path = string_value(object, "path")?;
    let start_line = object
        .get("start_line")
        .and_then(Value::as_u64)
        .ok_or(SemanticReportV2Error::Invalid)?;
    let end_line = object
        .get("end_line")
        .and_then(Value::as_u64)
        .ok_or(SemanticReportV2Error::Invalid)?;
    let expected_excerpt = string_value(object, "excerpt_sha256")?;
    let matches = sources
        .iter()
        .filter(|source| {
            source.repository_identity == repository_identity
                && source.commit_oid == revision
                && source.path == path
        })
        .collect::<Vec<_>>();
    let [source] = matches.as_slice() else {
        return Err(SemanticReportV2Error::Invalid);
    };
    if !valid_sha1(&source.commit_oid)
        || !valid_sha1(&source.tree_oid)
        || !valid_sha1(&source.blob_oid)
        || !valid_repository_path(&source.path)
        || source.bytes.len() as u64 > MAX_R19_SOURCE_BYTES_PER_FILE
    {
        return Err(SemanticReportV2Error::Invalid);
    }
    let text = std::str::from_utf8(&source.bytes).map_err(|_| SemanticReportV2Error::Invalid)?;
    let (start, end) =
        line_span(text, start_line, end_line).ok_or(SemanticReportV2Error::Invalid)?;
    if sha256(&source.bytes[start..end]) != expected_excerpt {
        return Err(SemanticReportV2Error::Invalid);
    }
    object.insert(
        "source_binding".to_owned(),
        json!({
            "commit_oid": source.commit_oid,
            "tree_oid": source.tree_oid,
            "blob_oid": source.blob_oid,
            "span": {
                "unit": "byte",
                "start": u64::try_from(start).map_err(|_| SemanticReportV2Error::Invalid)?,
                "end": u64::try_from(end).map_err(|_| SemanticReportV2Error::Invalid)?,
                "start_position": position_value(
                    &source_position(text, start).ok_or(SemanticReportV2Error::Invalid)?
                ),
                "end_position": position_value(
                    &source_position(text, end).ok_or(SemanticReportV2Error::Invalid)?
                )
            }
        }),
    );
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImpactSourceSelectionV1 {
    repository_identity: RepositoryIdentity,
    commit_oid: ObjectId,
    tree_oid: ObjectId,
    report_sha256: String,
    evidence_id: String,
    path: String,
    blob_oid: ObjectId,
    excerpt_sha256: String,
    start_byte: u64,
    end_byte: u64,
    start_position: SourcePosition,
    end_position: SourcePosition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourcePosition {
    line: u64,
    column: u64,
}

impl ImpactSourceSelectionV1 {
    /// Selects one exact Git-bound evidence locator from one canonical V2 report.
    ///
    /// # Errors
    ///
    /// Returns a strict report, evidence, repository, revision, path, binding,
    /// or canonical-input failure.
    pub fn from_report(
        bytes: &[u8],
        evidence_id: &str,
        repository_identity: &str,
        revision: &str,
        sha256: R19Sha256,
    ) -> Result<Self, ImpactSourceError> {
        let value = parse_canonical_report(bytes)?;
        if !valid_evidence_id(evidence_id) {
            return Err(ImpactSourceError::InvalidEvidence);
        }
        let records = value
            .get("evidence")
            .and_then(Value::as_array)
            .ok_or(ImpactSourceError::InvalidReport)?;
        let matches = records
            .iter()
            .filter(|record| record.get("id").and_then(Value::as_str) == Some(evidence_id))
            .collect::<Vec<_>>();
        let record = match matches.as_slice() {
            [] => return Err(ImpactSourceError::EvidenceNotFound),
            [record] => *record,
            _ => return Err(ImpactSourceError::InvalidEvidence),
        };
        let record_object = record
            .as_object()
            .ok_or(ImpactSourceError::InvalidEvidence)?;
        let selected_identity = string_value(record_object, "repository_identity")
            .map_err(|_| ImpactSourceError::InvalidEvidence)?;
        let selected_revision = string_value(record_object, "revision")
            .map_err(|_| ImpactSourceError::InvalidEvidence)?;
        if selected_identity != repository_identity || selected_revision != revision {
            return Err(ImpactSourceError::RepositoryMismatch);
        }
        let repository_identity = RepositoryIdentity::parse(repository_identity)
            .map_err(|_| ImpactSourceError::InvalidEvidence)?;
        let commit_oid =
            ObjectId::parse_sha1(revision).ok_or(ImpactSourceError::InvalidEvidence)?;
        let path =
            string_value(record_object, "path").map_err(|_| ImpactSourceError::InvalidEvidence)?;
        let excerpt_sha256 = string_value(record_object, "excerpt_sha256")
            .map_err(|_| ImpactSourceError::InvalidEvidence)?;
        if !valid_sha256(&excerpt_sha256) {
            return Err(ImpactSourceError::InvalidEvidence);
        }
        if !valid_repository_path(&path) {
            return Err(ImpactSourceError::PathRejected);
        }
        let binding = record_object
            .get("source_binding")
            .and_then(Value::as_object)
            .ok_or(ImpactSourceError::InvalidEvidence)?;
        if !has_exact_keys(binding, &["blob_oid", "commit_oid", "span", "tree_oid"])
            || binding.get("commit_oid").and_then(Value::as_str) != Some(revision)
        {
            return Err(ImpactSourceError::InvalidEvidence);
        }
        let tree_oid = binding
            .get("tree_oid")
            .and_then(Value::as_str)
            .and_then(ObjectId::parse_sha1)
            .ok_or(ImpactSourceError::InvalidEvidence)?;
        let blob_oid = binding
            .get("blob_oid")
            .and_then(Value::as_str)
            .and_then(ObjectId::parse_sha1)
            .ok_or(ImpactSourceError::InvalidEvidence)?;
        let span = binding
            .get("span")
            .and_then(Value::as_object)
            .ok_or(ImpactSourceError::InvalidEvidence)?;
        if !has_exact_keys(
            span,
            &["end", "end_position", "start", "start_position", "unit"],
        ) || span.get("unit").and_then(Value::as_str) != Some("byte")
        {
            return Err(ImpactSourceError::InvalidEvidence);
        }
        let start_byte = span
            .get("start")
            .and_then(Value::as_u64)
            .ok_or(ImpactSourceError::InvalidEvidence)?;
        let end_byte = span
            .get("end")
            .and_then(Value::as_u64)
            .ok_or(ImpactSourceError::InvalidEvidence)?;
        if start_byte >= end_byte {
            return Err(ImpactSourceError::InvalidEvidence);
        }
        Ok(Self {
            repository_identity,
            commit_oid,
            tree_oid,
            report_sha256: sha256(bytes),
            evidence_id: evidence_id.to_owned(),
            path,
            blob_oid,
            excerpt_sha256,
            start_byte,
            end_byte,
            start_position: parse_position(span.get("start_position"))?,
            end_position: parse_position(span.get("end_position"))?,
        })
    }

    #[must_use]
    pub const fn repository_identity(&self) -> &RepositoryIdentity {
        &self.repository_identity
    }

    #[must_use]
    pub const fn commit_oid(&self) -> &ObjectId {
        &self.commit_oid
    }

    #[must_use]
    pub const fn tree_oid(&self) -> &ObjectId {
        &self.tree_oid
    }

    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[must_use]
    pub const fn blob_oid(&self) -> &ObjectId {
        &self.blob_oid
    }
}

fn parse_canonical_report(bytes: &[u8]) -> Result<Value, ImpactSourceError> {
    let observed = u64::try_from(bytes.len()).map_err(|_| ImpactSourceError::Internal)?;
    if observed > MAX_R19_REPORT_BYTES {
        return Err(ImpactSourceError::LimitExceeded);
    }
    let body = bytes
        .strip_suffix(b"\n")
        .ok_or(ImpactSourceError::InvalidReport)?;
    let value: Value =
        serde_json::from_slice(body).map_err(|_| ImpactSourceError::InvalidReport)?;
    validate_report_value(&value).map_err(|_| ImpactSourceError::InvalidReport)?;
    if serde_json::to_vec(&value).map_err(|_| ImpactSourceError::Internal)? != body {
        return Err(ImpactSourceError::InvalidReport);
    }
    Ok(value)
}

#[derive(Clone, Debug)]
pub struct TrustedImpactSourceExcerptV1 {
    value: Value,
    canonical: Vec<u8>,
}

impl TrustedImpactSourceExcerptV1 {
    /// Resolves one report-selected locator against an independently acquired inventory.
    ///
    /// # Errors
    ///
    /// Returns a typed repository, path, blob, UTF-8, position, digest, or
    /// output-limit failure.
    pub fn from_inventory(
        selection: &ImpactSourceSelectionV1,
        inventory: &RepositoryInventory,
        sha256: R19Sha256,
    ) -> Result<Self, ImpactSourceError> {
        let bound = inventory.bound_revision();
        if bound.repository_identity() != selection.repository_identity()
            || bound.commit_oid() != selection.commit_oid()
            || bound.tree_oid() != selection.tree_oid()
        {
            return Err(ImpactSourceError::RepositoryMismatch);
        }
        let files = inventory
            .files()
            .iter()
            .filter(|file| file.path() == selection.path())
            .collect::<Vec<_>>();
        let [file] = files.as_slice() else {
            return Err(ImpactSourceError::PathRejected);
        };
        if file.blob_oid() != selection.blob_oid() {
            return Err(ImpactSourceError::RepositoryMismatch);
        }
        let source =
            std::str::from_utf8(file.bytes()).map_err(|_| ImpactSourceError::ContentRejected)?;
        let start = usize::try_from(selection.start_byte)
            .map_err(|_| ImpactSourceError::InvalidEvidence)?;
        let end =
            usize::try_from(selection.end_byte).map_err(|_| ImpactSourceError::InvalidEvidence)?;
        if start >= end
            || end > source.len()
            || !source.is_char_boundary(start)
            || !source.is_char_boundary(end)
        {
            return Err(ImpactSourceError::ContentRejected);
        }
        let start_position =
            source_position(source, start).ok_or(ImpactSourceError::ContentRejected)?;
        let end_position =
            source_position(source, end).ok_or(ImpactSourceError::ContentRejected)?;
        if start_position != selection.start_position || end_position != selection.end_position {
            return Err(ImpactSourceError::InvalidEvidence);
        }
        let excerpt = &source[start..end];
        let excerpt_sha256 = sha256(excerpt.as_bytes());
        if excerpt_sha256 != selection.excerpt_sha256 {
            return Err(ImpactSourceError::ContentRejected);
        }
        let excerpt_bytes =
            u64::try_from(excerpt.len()).map_err(|_| ImpactSourceError::Internal)?;
        if excerpt_bytes > MAX_R19_EXCERPT_BYTES {
            return Err(ImpactSourceError::LimitExceeded);
        }
        let value = json!({
            "schema_version": R19_SOURCE_RESULT_VERSION,
            "profile": R19_SOURCE_PROFILE,
            "authority": "explicit_local_git_object_only",
            "disclosure": "explicit_transient_stdout",
            "report": {
                "schema_version": R19_REPORT_VERSION,
                "sha256": selection.report_sha256
            },
            "source": {
                "repository_identity": selection.repository_identity.as_str(),
                "commit_oid": selection.commit_oid.as_str(),
                "tree_oid": selection.tree_oid.as_str()
            },
            "evidence": {
                "id": selection.evidence_id,
                "path": selection.path,
                "blob_oid": selection.blob_oid.as_str(),
                "span": {
                    "unit": "byte",
                    "start": selection.start_byte,
                    "end": selection.end_byte,
                    "start_position": position_value(&start_position),
                    "end_position": position_value(&end_position)
                }
            },
            "excerpt": {
                "encoding": "utf-8",
                "text": excerpt,
                "byte_length": excerpt_bytes,
                "sha256": excerpt_sha256
            },
            "limitations": LIMITATIONS
        });
        let canonical = serde_json::to_vec(&value).map_err(|_| ImpactSourceError::Internal)?;
        let observed = u64::try_from(canonical.len().saturating_add(1))
            .map_err(|_| ImpactSourceError::Internal)?;
        if observed > MAX_R19_EXCERPT_STDOUT_BYTES {
            return Err(ImpactSourceError::LimitExceeded);
        }
        Ok(Self { value, canonical })
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    #[must_use]
    pub fn canonical_stdout(&self) -> Vec<u8> {
        let mut bytes = self.canonical.clone();
        bytes.push(b'\n');
        bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImpactSourceError {
    InvalidReport,
    EvidenceNotFound,
    InvalidEvidence,
    RepositoryMismatch,
    PathRejected,
    ContentRejected,
    LimitExceeded,
    Internal,
}

impl Display for ImpactSourceError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidReport => "semantic impact report is invalid",
            Self::EvidenceNotFound => "semantic impact evidence was not found",
            Self::InvalidEvidence => "semantic impact evidence is invalid",
            Self::RepositoryMismatch => "semantic impact repository does not match",
            Self::PathRejected => "semantic impact source path was rejected",
            Self::ContentRejected => "semantic impact source content was rejected",
            Self::LimitExceeded => "semantic impact source limit exceeded",
            Self::Internal => "semantic impact source contract failed",
        })
    }
}

impl Error for ImpactSourceError {}

#[derive(Clone, Debug)]
pub struct CodeNoesisErrorV30 {
    value: Value,
}

impl CodeNoesisErrorV30 {
    #[must_use]
    pub fn impact_invalid_workspace() -> Self {
        Self::new(
            "impact_git.invalid_workspace",
            "input",
            "invalid Git impact workspace",
        )
    }

    #[must_use]
    pub fn impact_invalid_federation_report() -> Self {
        Self::new(
            "impact_git.invalid_federation_report",
            "impact",
            "invalid Git impact federation report",
        )
    }

    #[must_use]
    pub fn impact_acquisition_rejected() -> Self {
        Self::new(
            "impact_git.acquisition_rejected",
            "acquisition",
            "Git impact acquisition rejected",
        )
    }

    #[must_use]
    pub fn impact_source_rejected() -> Self {
        Self::new(
            "impact_git.source_rejected",
            "impact",
            "Git impact source rejected",
        )
    }

    #[must_use]
    pub fn impact_limit_exceeded() -> Self {
        Self::new(
            "impact_git.limit_exceeded",
            "impact",
            "Git impact limit exceeded",
        )
    }

    #[must_use]
    pub fn impact_unstable_input() -> Self {
        Self::new(
            "impact_git.unstable_input",
            "input",
            "Git impact input changed",
        )
    }

    #[must_use]
    pub fn source_invalid_arguments() -> Self {
        Self::new(
            "impact_source.invalid_arguments",
            "input",
            "invalid impact source arguments",
        )
    }

    #[must_use]
    pub fn source_invalid_report() -> Self {
        Self::new(
            "impact_source.invalid_report",
            "source",
            "invalid semantic impact report",
        )
    }

    #[must_use]
    pub fn source_acquisition_rejected() -> Self {
        Self::new(
            "impact_source.acquisition_rejected",
            "acquisition",
            "impact source acquisition rejected",
        )
    }

    #[must_use]
    pub fn source_unstable_input() -> Self {
        Self::new(
            "impact_source.unstable_input",
            "source",
            "impact source input changed",
        )
    }

    #[must_use]
    pub fn from_source(error: &ImpactSourceError) -> Self {
        match error {
            ImpactSourceError::InvalidReport => Self::source_invalid_report(),
            ImpactSourceError::EvidenceNotFound => Self::new(
                "impact_source.evidence_not_found",
                "source",
                "impact source evidence not found",
            ),
            ImpactSourceError::InvalidEvidence => Self::new(
                "impact_source.invalid_evidence",
                "source",
                "impact source evidence invalid",
            ),
            ImpactSourceError::RepositoryMismatch => Self::new(
                "impact_source.repository_mismatch",
                "source",
                "impact source repository mismatch",
            ),
            ImpactSourceError::PathRejected => Self::new(
                "impact_source.path_rejected",
                "source",
                "impact source path rejected",
            ),
            ImpactSourceError::ContentRejected => Self::new(
                "impact_source.content_rejected",
                "source",
                "impact source content rejected",
            ),
            ImpactSourceError::LimitExceeded => Self::new(
                "impact_source.limit_exceeded",
                "source",
                "impact source limit exceeded",
            ),
            ImpactSourceError::Internal => Self::source_internal(),
        }
    }

    #[must_use]
    pub fn source_internal() -> Self {
        Self::new(
            "impact_source.internal",
            "internal",
            "impact source internal failure",
        )
    }

    fn new(code: &str, stage: &str, message: &str) -> Self {
        Self {
            value: json!({
                "schema_version": R19_ERROR_VERSION,
                "code": code,
                "stage": stage,
                "message": message,
                "retryable": false,
                "context": {}
            }),
        }
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Serializes one strict `ErrorV30` plus LF.
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

fn validate_report_value(value: &Value) -> Result<(), SemanticReportV2Error> {
    let root = value.as_object().ok_or(SemanticReportV2Error::Invalid)?;
    if !has_exact_keys(
        root,
        &[
            "analysis_profile",
            "client_assessments",
            "configuration_hash",
            "coverage_gaps",
            "evidence",
            "evidence_lineage_version",
            "extractor_versions",
            "ontology_versions",
            "pipeline_version",
            "provider",
            "rejected_candidates",
            "rule_catalog_version",
            "schema_version",
            "semantic_diffs",
        ],
    ) || root.get("schema_version").and_then(Value::as_str) != Some(R19_REPORT_VERSION)
        || root.get("analysis_profile").and_then(Value::as_str) != Some(R19_ANALYSIS_PROFILE)
        || root.get("pipeline_version").and_then(Value::as_str) != Some(R19_PIPELINE_VERSION)
        || root.get("evidence_lineage_version").and_then(Value::as_str)
            != Some(R19_EVIDENCE_LINEAGE_VERSION)
        || !root
            .get("configuration_hash")
            .and_then(Value::as_str)
            .is_some_and(valid_blake3)
    {
        return Err(SemanticReportV2Error::Invalid);
    }
    for collection in [
        "client_assessments",
        "coverage_gaps",
        "extractor_versions",
        "ontology_versions",
        "rejected_candidates",
        "semantic_diffs",
    ] {
        if !root.get(collection).is_some_and(Value::is_array) {
            return Err(SemanticReportV2Error::Invalid);
        }
    }
    let evidence = root
        .get("evidence")
        .and_then(Value::as_array)
        .filter(|evidence| !evidence.is_empty())
        .ok_or(SemanticReportV2Error::Invalid)?;
    let mut ids = BTreeSet::new();
    let mut previous = None;
    for record in evidence {
        validate_git_evidence(record)?;
        let id = record
            .get("id")
            .and_then(Value::as_str)
            .ok_or(SemanticReportV2Error::Invalid)?;
        if !ids.insert(id) || previous.is_some_and(|previous| previous >= id) {
            return Err(SemanticReportV2Error::Invalid);
        }
        previous = Some(id);
    }
    validate_evidence_references(value, &ids)
}

fn validate_git_evidence(value: &Value) -> Result<(), SemanticReportV2Error> {
    let object = value.as_object().ok_or(SemanticReportV2Error::Invalid)?;
    if !has_exact_keys(
        object,
        &[
            "capability_version",
            "claim_state",
            "end_line",
            "excerpt_sha256",
            "id",
            "path",
            "repository_identity",
            "revision",
            "source_binding",
            "source_kind",
            "start_line",
        ],
    ) || !object
        .get("id")
        .and_then(Value::as_str)
        .is_some_and(valid_evidence_id)
        || object
            .get("repository_identity")
            .and_then(Value::as_str)
            .and_then(|identity| RepositoryIdentity::parse(identity).ok())
            .is_none()
        || !object
            .get("revision")
            .and_then(Value::as_str)
            .is_some_and(valid_sha1)
        || !object
            .get("path")
            .and_then(Value::as_str)
            .is_some_and(valid_repository_path)
        || !object
            .get("excerpt_sha256")
            .and_then(Value::as_str)
            .is_some_and(valid_sha256)
    {
        return Err(SemanticReportV2Error::Invalid);
    }
    let binding = object
        .get("source_binding")
        .and_then(Value::as_object)
        .ok_or(SemanticReportV2Error::Invalid)?;
    if !has_exact_keys(binding, &["blob_oid", "commit_oid", "span", "tree_oid"])
        || binding.get("commit_oid") != object.get("revision")
        || !binding
            .get("tree_oid")
            .and_then(Value::as_str)
            .is_some_and(valid_sha1)
        || !binding
            .get("blob_oid")
            .and_then(Value::as_str)
            .is_some_and(valid_sha1)
    {
        return Err(SemanticReportV2Error::Invalid);
    }
    let span = binding
        .get("span")
        .and_then(Value::as_object)
        .ok_or(SemanticReportV2Error::Invalid)?;
    if !has_exact_keys(
        span,
        &["end", "end_position", "start", "start_position", "unit"],
    ) || span.get("unit").and_then(Value::as_str) != Some("byte")
        || span.get("start").and_then(Value::as_u64) >= span.get("end").and_then(Value::as_u64)
        || parse_position(span.get("start_position")).is_err()
        || parse_position(span.get("end_position")).is_err()
    {
        return Err(SemanticReportV2Error::Invalid);
    }
    Ok(())
}

fn validate_evidence_references(
    value: &Value,
    evidence_ids: &BTreeSet<&str>,
) -> Result<(), SemanticReportV2Error> {
    match value {
        Value::Object(object) => {
            if let Some(ids) = object.get("evidence_ids") {
                let ids = ids.as_array().ok_or(SemanticReportV2Error::Invalid)?;
                if ids
                    .iter()
                    .any(|id| id.as_str().is_none_or(|id| !evidence_ids.contains(id)))
                {
                    return Err(SemanticReportV2Error::Invalid);
                }
            }
            for child in object.values() {
                validate_evidence_references(child, evidence_ids)?;
            }
        }
        Value::Array(values) => {
            for child in values {
                validate_evidence_references(child, evidence_ids)?;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    Ok(())
}

fn parse_position(value: Option<&Value>) -> Result<SourcePosition, ImpactSourceError> {
    let object = value
        .and_then(Value::as_object)
        .ok_or(ImpactSourceError::InvalidEvidence)?;
    if !has_exact_keys(object, &["column", "line", "unit"])
        || object.get("unit").and_then(Value::as_str) != Some("unicode_scalar")
    {
        return Err(ImpactSourceError::InvalidEvidence);
    }
    Ok(SourcePosition {
        line: object
            .get("line")
            .and_then(Value::as_u64)
            .filter(|line| *line > 0)
            .ok_or(ImpactSourceError::InvalidEvidence)?,
        column: object
            .get("column")
            .and_then(Value::as_u64)
            .filter(|column| *column > 0)
            .ok_or(ImpactSourceError::InvalidEvidence)?,
    })
}

fn position_value(position: &SourcePosition) -> Value {
    json!({
        "line": position.line,
        "column": position.column,
        "unit": "unicode_scalar"
    })
}

fn source_position(source: &str, offset: usize) -> Option<SourcePosition> {
    let prefix = source.get(..offset)?;
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let current_line = prefix
        .rsplit_once('\n')
        .map_or(prefix, |(_, suffix)| suffix);
    Some(SourcePosition {
        line: u64::try_from(line).ok()?,
        column: u64::try_from(current_line.chars().count().saturating_add(1)).ok()?,
    })
}

fn line_span(source: &str, start_line: u64, end_line: u64) -> Option<(usize, usize)> {
    if start_line == 0 || end_line < start_line {
        return None;
    }
    let bytes = source.as_bytes();
    let mut line = 1_u64;
    let mut start = None;
    let mut end = None;
    for (index, byte) in bytes.iter().enumerate() {
        if line == start_line && start.is_none() {
            start = Some(index);
        }
        if *byte == b'\n' {
            if line == end_line {
                end = Some(index + 1);
                break;
            }
            line = line.saturating_add(1);
        }
    }
    if start.is_none() && line == start_line {
        start = Some(bytes.len());
    }
    if end.is_none() && line == end_line {
        end = Some(bytes.len());
    }
    let start = start?;
    let end = end?;
    (start < end).then_some((start, end))
}

fn configuration_hash() -> String {
    let value = json!({
        "analysis_profile": R19_ANALYSIS_PROFILE,
        "client_capability": CLIENT_CAPABILITY,
        "contract_capability": CONTRACT_CAPABILITY,
        "pipeline": R19_PIPELINE_VERSION,
        "provider_capability": PROVIDER_CAPABILITY
    });
    let mut hasher = blake3::Hasher::new();
    hasher.update(CONFIGURATION_HASH_DOMAIN);
    hasher.update(&[0]);
    serde_json::to_writer(&mut hasher_writer(&mut hasher), &value)
        .expect("R19 configuration JSON serializes");
    format!("blake3:{}", hasher.finalize().to_hex())
}

fn hasher_writer(hasher: &mut blake3::Hasher) -> HasherWriter<'_> {
    HasherWriter(hasher)
}

struct HasherWriter<'a>(&'a mut blake3::Hasher);

impl std::io::Write for HasherWriter<'_> {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.0.update(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn exact_object<'a>(
    value: &'a Value,
    expected: &[&str],
) -> Result<&'a Map<String, Value>, ImpactGitWorkspaceError> {
    let object = value.as_object().ok_or(ImpactGitWorkspaceError::Invalid)?;
    has_exact_keys(object, expected)
        .then_some(object)
        .ok_or(ImpactGitWorkspaceError::Invalid)
}

fn has_exact_keys(object: &Map<String, Value>, expected: &[&str]) -> bool {
    object.len() == expected.len() && expected.iter().all(|key| object.contains_key(*key))
}

fn require_const(
    object: &Map<String, Value>,
    key: &str,
    expected: &str,
) -> Result<(), ImpactGitWorkspaceError> {
    (object.get(key).and_then(Value::as_str) == Some(expected))
        .then_some(())
        .ok_or(ImpactGitWorkspaceError::Invalid)
}

fn string_value(object: &Map<String, Value>, key: &str) -> Result<String, SemanticReportV2Error> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or(SemanticReportV2Error::Invalid)
}

fn required_bounded(
    object: &Map<String, Value>,
    key: &str,
    maximum: usize,
) -> Result<String, ImpactGitWorkspaceError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= maximum && !value.contains('\0'))
        .map(str::to_owned)
        .ok_or(ImpactGitWorkspaceError::Invalid)
}

fn required_identity(
    object: &Map<String, Value>,
    key: &str,
) -> Result<String, ImpactGitWorkspaceError> {
    let value = required_bounded(object, key, 271)?;
    RepositoryIdentity::parse(&value)
        .map(|_| value)
        .map_err(|_| ImpactGitWorkspaceError::Invalid)
}

fn required_commit(
    object: &Map<String, Value>,
    key: &str,
) -> Result<String, ImpactGitWorkspaceError> {
    let value = required_bounded(object, key, 40)?;
    valid_sha1(&value)
        .then_some(value)
        .ok_or(ImpactGitWorkspaceError::Invalid)
}

fn required_path(
    object: &Map<String, Value>,
    key: &str,
) -> Result<String, ImpactGitWorkspaceError> {
    let value = required_bounded(object, key, MAX_R19_PATH_BYTES)?;
    valid_repository_path(&value)
        .then_some(value)
        .ok_or(ImpactGitWorkspaceError::Invalid)
}

fn valid_repository_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_R19_PATH_BYTES
        && !value.starts_with('/')
        && !value.ends_with('/')
        && !value.contains('\0')
        && value
            .split('/')
            .all(|component| !component.is_empty() && component != "." && component != "..")
}

fn valid_sha1(value: &str) -> bool {
    ObjectId::parse_sha1(value).is_some()
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn valid_blake3(value: &str) -> bool {
    value.strip_prefix("blake3:").is_some_and(valid_sha256)
}

fn valid_evidence_id(value: &str) -> bool {
    value
        .strip_prefix("urn:codenoesis:evidence:blake3:")
        .is_some_and(valid_sha256)
}
