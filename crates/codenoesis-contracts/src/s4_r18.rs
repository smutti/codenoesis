use std::error::Error;
use std::fmt::{self, Display, Formatter};

use codenoesis_domain::storage::LocalSnapshotHead;
use codenoesis_domain::{ObjectId, RepositoryIdentity, RepositoryInventory};
use serde_json::{Value, json};

use super::{
    R16_GRAPH_VERSION, R16_ONTOLOGY_VERSION, R16_SNAPSHOT_VERSION,
    validate_stored_snapshot_semantic_v18,
};

pub const R18_SOURCE_PROFILE: &str = "trusted-local-source-v1";
pub const R18_SOURCE_EXCERPT_VERSION: &str = "codenoesis.trusted-source-excerpt/v1";
pub const R18_ERROR_VERSION: &str = "codenoesis.error/v29";
pub const MAX_R18_EXCERPT_BYTES: u64 = 262_144;
pub const MAX_R18_STDOUT_BYTES: u64 = 524_288;
pub const MAX_R18_PATH_BYTES: usize = 1_024;

pub type R18Sha256 = fn(&[u8]) -> String;

const LIMITATIONS: [&str; 6] = [
    "exact_committed_bytes_only",
    "no_context_line_expansion",
    "no_retention_or_export",
    "no_working_tree_fallback",
    "single_evidence_only",
    "utf8_only",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrustedSourceSelectionV1 {
    repository_identity: RepositoryIdentity,
    commit_oid: ObjectId,
    tree_oid: ObjectId,
    snapshot_id: String,
    snapshot_semantic_hash: String,
    graph_semantic_hash: String,
    evidence_id: String,
    path: String,
    blob_oid: ObjectId,
    start_byte: u64,
    end_byte: u64,
}

impl TrustedSourceSelectionV1 {
    /// Selects one exact evidence locator from a validated visible V18 head.
    ///
    /// # Errors
    ///
    /// Returns a typed contract failure when the head, graph, evidence identity,
    /// locator, or source binding is invalid.
    pub fn from_validated_v18(
        semantic: &Value,
        head: &LocalSnapshotHead,
        evidence_id: &str,
    ) -> Result<Self, TrustedSourceError> {
        validate_stored_snapshot_semantic_v18(semantic, head)
            .map_err(|_| TrustedSourceError::InvalidSnapshot)?;
        if !valid_evidence_id(evidence_id) {
            return Err(TrustedSourceError::InvalidEvidence);
        }
        if head.snapshot_schema_version != R16_SNAPSHOT_VERSION
            || semantic
                .pointer("/repository/identity")
                .and_then(Value::as_str)
                != Some(head.repository_identity.as_str())
            || semantic
                .pointer("/repository/commit_oid")
                .and_then(Value::as_str)
                != Some(head.commit_oid.as_str())
            || semantic.get("ontology_version").and_then(Value::as_str)
                != Some(R16_ONTOLOGY_VERSION)
        {
            return Err(TrustedSourceError::InvalidSnapshot);
        }
        let tree_oid = semantic
            .pointer("/repository/tree_oid")
            .and_then(Value::as_str)
            .and_then(ObjectId::parse_sha1)
            .ok_or(TrustedSourceError::InvalidSnapshot)?;
        let graph = semantic
            .get("knowledge_graph")
            .and_then(Value::as_object)
            .ok_or(TrustedSourceError::InvalidSnapshot)?;
        if graph.get("schema_version").and_then(Value::as_str) != Some(R16_GRAPH_VERSION)
            || graph.get("ontology_version").and_then(Value::as_str) != Some(R16_ONTOLOGY_VERSION)
            || graph
                .get("semantic_hash")
                .and_then(|value| value.get("value"))
                .and_then(Value::as_str)
                != Some(head.graph_semantic_hash.value.as_str())
        {
            return Err(TrustedSourceError::InvalidSnapshot);
        }
        let evidence = graph
            .get("evidence")
            .and_then(Value::as_array)
            .ok_or(TrustedSourceError::InvalidSnapshot)?;
        let matches = evidence
            .iter()
            .filter(|record| record.get("id").and_then(Value::as_str) == Some(evidence_id))
            .collect::<Vec<_>>();
        let record = match matches.as_slice() {
            [] => return Err(TrustedSourceError::EvidenceNotFound),
            [record] => *record,
            _ => return Err(TrustedSourceError::InvalidEvidence),
        };
        if !has_exact_keys(
            record,
            &["blob_oid", "end_byte", "id", "path", "start_byte"],
        ) {
            return Err(TrustedSourceError::InvalidEvidence);
        }
        let path = required_string(record, "path")?;
        if !valid_repository_path(path) {
            return Err(TrustedSourceError::PathRejected);
        }
        let blob_oid = required_string(record, "blob_oid")
            .ok()
            .and_then(ObjectId::parse_sha1)
            .ok_or(TrustedSourceError::InvalidEvidence)?;
        let start_byte = record
            .get("start_byte")
            .and_then(Value::as_u64)
            .ok_or(TrustedSourceError::InvalidEvidence)?;
        let end_byte = record
            .get("end_byte")
            .and_then(Value::as_u64)
            .ok_or(TrustedSourceError::InvalidEvidence)?;
        if start_byte >= end_byte {
            return Err(TrustedSourceError::InvalidEvidence);
        }
        Ok(Self {
            repository_identity: head.repository_identity.clone(),
            commit_oid: head.commit_oid.clone(),
            tree_oid,
            snapshot_id: head.snapshot_id.as_str().to_owned(),
            snapshot_semantic_hash: head.semantic_hash.value.clone(),
            graph_semantic_hash: head.graph_semantic_hash.value.clone(),
            evidence_id: evidence_id.to_owned(),
            path: path.to_owned(),
            blob_oid,
            start_byte,
            end_byte,
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
    pub fn evidence_id(&self) -> &str {
        &self.evidence_id
    }

    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[must_use]
    pub const fn blob_oid(&self) -> &ObjectId {
        &self.blob_oid
    }

    #[must_use]
    pub const fn start_byte(&self) -> u64 {
        self.start_byte
    }

    #[must_use]
    pub const fn end_byte(&self) -> u64 {
        self.end_byte
    }
}

#[derive(Clone, Debug)]
pub struct TrustedSourceExcerptV1 {
    value: Value,
    canonical: Vec<u8>,
}

impl TrustedSourceExcerptV1 {
    /// Resolves one selected locator against an independently acquired inventory.
    ///
    /// # Errors
    ///
    /// Returns a typed binding, path, UTF-8, scalar-boundary, limit, or
    /// serialization failure.
    pub fn from_inventory(
        selection: &TrustedSourceSelectionV1,
        inventory: &RepositoryInventory,
        sha256: R18Sha256,
    ) -> Result<Self, TrustedSourceError> {
        let bound = inventory.bound_revision();
        if bound.repository_identity() != selection.repository_identity()
            || bound.commit_oid() != selection.commit_oid()
            || bound.tree_oid() != selection.tree_oid()
        {
            return Err(TrustedSourceError::RepositoryMismatch);
        }
        let matches = inventory
            .files()
            .iter()
            .filter(|file| file.path() == selection.path())
            .collect::<Vec<_>>();
        let file = match matches.as_slice() {
            [] => return Err(TrustedSourceError::PathRejected),
            [file] => *file,
            _ => return Err(TrustedSourceError::InvalidEvidence),
        };
        if file.blob_oid() != selection.blob_oid() {
            return Err(TrustedSourceError::RepositoryMismatch);
        }
        let source =
            std::str::from_utf8(file.bytes()).map_err(|_| TrustedSourceError::ContentRejected)?;
        let start = usize::try_from(selection.start_byte)
            .map_err(|_| TrustedSourceError::InvalidEvidence)?;
        let end =
            usize::try_from(selection.end_byte).map_err(|_| TrustedSourceError::InvalidEvidence)?;
        if start >= end || end > source.len() {
            return Err(TrustedSourceError::InvalidEvidence);
        }
        if !source.is_char_boundary(start) || !source.is_char_boundary(end) {
            return Err(TrustedSourceError::ContentRejected);
        }
        let excerpt = &source[start..end];
        let excerpt_bytes =
            u64::try_from(excerpt.len()).map_err(|_| TrustedSourceError::Internal)?;
        if excerpt_bytes > MAX_R18_EXCERPT_BYTES {
            return Err(TrustedSourceError::LimitExceeded);
        }
        let excerpt_sha256 = sha256(excerpt.as_bytes());
        if !valid_sha256(&excerpt_sha256) {
            return Err(TrustedSourceError::Internal);
        }
        let start_position = source_position(source, start)?;
        let end_position = source_position(source, end)?;
        let value = json!({
            "schema_version": R18_SOURCE_EXCERPT_VERSION,
            "profile": R18_SOURCE_PROFILE,
            "authority": "explicit_local_git_object_only",
            "disclosure": "explicit_transient_stdout",
            "source": {
                "repository_identity": selection.repository_identity.as_str(),
                "commit_oid": selection.commit_oid.as_str(),
                "tree_oid": selection.tree_oid.as_str(),
                "snapshot_id": selection.snapshot_id,
                "snapshot_schema_version": R16_SNAPSHOT_VERSION,
                "graph_schema_version": R16_GRAPH_VERSION,
                "ontology_version": R16_ONTOLOGY_VERSION,
                "snapshot_semantic_hash": selection.snapshot_semantic_hash,
                "graph_semantic_hash": selection.graph_semantic_hash
            },
            "evidence": {
                "id": selection.evidence_id,
                "path": selection.path,
                "blob_oid": selection.blob_oid.as_str(),
                "span": {
                    "unit": "byte",
                    "start": selection.start_byte,
                    "end": selection.end_byte,
                    "start_position": start_position,
                    "end_position": end_position
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
        Self::from_value(value, sha256)
    }

    /// Strictly validates one canonical LF-terminated result.
    ///
    /// # Errors
    ///
    /// Returns a typed contract failure for malformed, non-canonical, private,
    /// mismatched, or over-limit bytes.
    pub fn from_canonical_stdout(
        bytes: &[u8],
        sha256: R18Sha256,
    ) -> Result<Self, TrustedSourceError> {
        let observed = u64::try_from(bytes.len()).map_err(|_| TrustedSourceError::Internal)?;
        if observed > MAX_R18_STDOUT_BYTES {
            return Err(TrustedSourceError::LimitExceeded);
        }
        let canonical = bytes
            .strip_suffix(b"\n")
            .ok_or(TrustedSourceError::InvalidEvidence)?;
        let value = serde_json::from_slice::<Value>(canonical)
            .map_err(|_| TrustedSourceError::InvalidEvidence)?;
        validate_output_value(&value, sha256)?;
        let expected = serde_json::to_vec(&value).map_err(|_| TrustedSourceError::Internal)?;
        if expected != canonical {
            return Err(TrustedSourceError::InvalidEvidence);
        }
        Ok(Self {
            value,
            canonical: expected,
        })
    }

    fn from_value(value: Value, sha256: R18Sha256) -> Result<Self, TrustedSourceError> {
        validate_output_value(&value, sha256)?;
        let canonical = serde_json::to_vec(&value).map_err(|_| TrustedSourceError::Internal)?;
        let observed = u64::try_from(canonical.len().saturating_add(1))
            .map_err(|_| TrustedSourceError::Internal)?;
        if observed > MAX_R18_STDOUT_BYTES {
            return Err(TrustedSourceError::LimitExceeded);
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
pub enum TrustedSourceError {
    InvalidSnapshot,
    EvidenceNotFound,
    InvalidEvidence,
    RepositoryMismatch,
    PathRejected,
    ContentRejected,
    LimitExceeded,
    Internal,
}

impl Display for TrustedSourceError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidSnapshot => "trusted source snapshot is invalid",
            Self::EvidenceNotFound => "trusted source evidence was not found",
            Self::InvalidEvidence => "trusted source evidence is invalid",
            Self::RepositoryMismatch => "trusted source repository does not match",
            Self::PathRejected => "trusted source path was rejected",
            Self::ContentRejected => "trusted source content was rejected",
            Self::LimitExceeded => "trusted source limit exceeded",
            Self::Internal => "trusted source contract failed",
        })
    }
}

impl Error for TrustedSourceError {}

#[derive(Clone, Debug)]
pub struct CodeNoesisErrorV29 {
    value: Value,
}

impl CodeNoesisErrorV29 {
    #[must_use]
    pub fn invalid_arguments() -> Self {
        Self::new(
            "source.invalid_arguments",
            "input",
            "invalid trusted source arguments",
        )
    }

    #[must_use]
    pub fn acquisition_rejected() -> Self {
        Self::new(
            "source.acquisition_rejected",
            "source",
            "trusted source acquisition rejected",
        )
    }

    #[must_use]
    pub fn path_rejected() -> Self {
        Self::new(
            "source.path_rejected",
            "source",
            "trusted source path rejected",
        )
    }

    #[must_use]
    pub fn limit_exceeded() -> Self {
        Self::new(
            "source.limit_exceeded",
            "source",
            "trusted source limit exceeded",
        )
    }

    #[must_use]
    pub fn unstable_input() -> Self {
        Self::new(
            "source.unstable_input",
            "source",
            "trusted source input changed",
        )
    }

    #[must_use]
    pub fn repository_mismatch() -> Self {
        Self::new(
            "source.repository_mismatch",
            "source",
            "trusted source repository mismatch",
        )
    }

    #[must_use]
    pub fn invalid_snapshot() -> Self {
        Self::new(
            "source.invalid_snapshot",
            "source",
            "trusted source snapshot invalid",
        )
    }

    #[must_use]
    pub fn from_contract(error: &TrustedSourceError) -> Self {
        match error {
            TrustedSourceError::InvalidSnapshot => Self::invalid_snapshot(),
            TrustedSourceError::EvidenceNotFound => Self::new(
                "source.evidence_not_found",
                "source",
                "trusted source evidence not found",
            ),
            TrustedSourceError::InvalidEvidence => Self::new(
                "source.invalid_evidence",
                "source",
                "trusted source evidence invalid",
            ),
            TrustedSourceError::RepositoryMismatch => Self::repository_mismatch(),
            TrustedSourceError::PathRejected => Self::path_rejected(),
            TrustedSourceError::ContentRejected => Self::new(
                "source.content_rejected",
                "source",
                "trusted source content rejected",
            ),
            TrustedSourceError::LimitExceeded => Self::limit_exceeded(),
            TrustedSourceError::Internal => Self::internal(),
        }
    }

    #[must_use]
    pub fn internal() -> Self {
        Self::new(
            "source.internal",
            "internal",
            "trusted source internal failure",
        )
    }

    fn new(code: &str, stage: &str, message: &str) -> Self {
        Self {
            value: json!({
                "schema_version": R18_ERROR_VERSION,
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

    /// Serializes one strict `ErrorV29` plus LF.
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

fn validate_output_value(value: &Value, sha256: R18Sha256) -> Result<(), TrustedSourceError> {
    if !has_exact_keys(
        value,
        &[
            "authority",
            "disclosure",
            "evidence",
            "excerpt",
            "limitations",
            "profile",
            "schema_version",
            "source",
        ],
    ) || value.get("schema_version").and_then(Value::as_str) != Some(R18_SOURCE_EXCERPT_VERSION)
        || value.get("profile").and_then(Value::as_str) != Some(R18_SOURCE_PROFILE)
        || value.get("authority").and_then(Value::as_str) != Some("explicit_local_git_object_only")
        || value.get("disclosure").and_then(Value::as_str) != Some("explicit_transient_stdout")
    {
        return Err(TrustedSourceError::InvalidEvidence);
    }
    validate_source(value.get("source"))?;
    let (start, end) = validate_evidence(value.get("evidence"))?;
    validate_excerpt(value.get("excerpt"), start, end, sha256)?;
    validate_limitations(value.get("limitations"))
}

fn validate_source(source: Option<&Value>) -> Result<(), TrustedSourceError> {
    let source = source.ok_or(TrustedSourceError::InvalidEvidence)?;
    if !has_exact_keys(
        source,
        &[
            "commit_oid",
            "graph_schema_version",
            "graph_semantic_hash",
            "ontology_version",
            "repository_identity",
            "snapshot_id",
            "snapshot_schema_version",
            "snapshot_semantic_hash",
            "tree_oid",
        ],
    ) || source
        .get("repository_identity")
        .and_then(Value::as_str)
        .and_then(|value| RepositoryIdentity::parse(value).ok())
        .is_none()
        || source
            .get("commit_oid")
            .and_then(Value::as_str)
            .and_then(ObjectId::parse_sha1)
            .is_none()
        || source
            .get("tree_oid")
            .and_then(Value::as_str)
            .and_then(ObjectId::parse_sha1)
            .is_none()
        || source
            .get("snapshot_schema_version")
            .and_then(Value::as_str)
            != Some(R16_SNAPSHOT_VERSION)
        || source.get("graph_schema_version").and_then(Value::as_str) != Some(R16_GRAPH_VERSION)
        || source.get("ontology_version").and_then(Value::as_str) != Some(R16_ONTOLOGY_VERSION)
        || !source
            .get("snapshot_id")
            .and_then(Value::as_str)
            .is_some_and(valid_snapshot_id)
        || !source
            .get("snapshot_semantic_hash")
            .and_then(Value::as_str)
            .is_some_and(valid_sha256)
        || !source
            .get("graph_semantic_hash")
            .and_then(Value::as_str)
            .is_some_and(valid_sha256)
    {
        return Err(TrustedSourceError::InvalidSnapshot);
    }
    Ok(())
}

fn validate_evidence(evidence: Option<&Value>) -> Result<(u64, u64), TrustedSourceError> {
    let evidence = evidence.ok_or(TrustedSourceError::InvalidEvidence)?;
    if !has_exact_keys(evidence, &["blob_oid", "id", "path", "span"])
        || !evidence
            .get("id")
            .and_then(Value::as_str)
            .is_some_and(valid_evidence_id)
        || !evidence
            .get("path")
            .and_then(Value::as_str)
            .is_some_and(valid_repository_path)
        || evidence
            .get("blob_oid")
            .and_then(Value::as_str)
            .and_then(ObjectId::parse_sha1)
            .is_none()
    {
        return Err(TrustedSourceError::InvalidEvidence);
    }
    let span = evidence
        .get("span")
        .ok_or(TrustedSourceError::InvalidEvidence)?;
    if !has_exact_keys(
        span,
        &["end", "end_position", "start", "start_position", "unit"],
    ) || span.get("unit").and_then(Value::as_str) != Some("byte")
    {
        return Err(TrustedSourceError::InvalidEvidence);
    }
    let start = span
        .get("start")
        .and_then(Value::as_u64)
        .ok_or(TrustedSourceError::InvalidEvidence)?;
    let end = span
        .get("end")
        .and_then(Value::as_u64)
        .ok_or(TrustedSourceError::InvalidEvidence)?;
    if start >= end
        || !valid_position(span.get("start_position"))
        || !valid_position(span.get("end_position"))
    {
        return Err(TrustedSourceError::InvalidEvidence);
    }
    Ok((start, end))
}

fn validate_excerpt(
    excerpt: Option<&Value>,
    start: u64,
    end: u64,
    sha256: R18Sha256,
) -> Result<(), TrustedSourceError> {
    let excerpt = excerpt.ok_or(TrustedSourceError::InvalidEvidence)?;
    if !has_exact_keys(excerpt, &["byte_length", "encoding", "sha256", "text"])
        || excerpt.get("encoding").and_then(Value::as_str) != Some("utf-8")
    {
        return Err(TrustedSourceError::InvalidEvidence);
    }
    let text = excerpt
        .get("text")
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .ok_or(TrustedSourceError::ContentRejected)?;
    let byte_length = excerpt
        .get("byte_length")
        .and_then(Value::as_u64)
        .ok_or(TrustedSourceError::InvalidEvidence)?;
    if byte_length != u64::try_from(text.len()).map_err(|_| TrustedSourceError::Internal)?
        || byte_length != end - start
        || byte_length > MAX_R18_EXCERPT_BYTES
    {
        return Err(TrustedSourceError::LimitExceeded);
    }
    let observed_sha256 = excerpt
        .get("sha256")
        .and_then(Value::as_str)
        .ok_or(TrustedSourceError::InvalidEvidence)?;
    if !valid_sha256(observed_sha256) || observed_sha256 != sha256(text.as_bytes()) {
        return Err(TrustedSourceError::ContentRejected);
    }
    Ok(())
}

fn validate_limitations(limitations: Option<&Value>) -> Result<(), TrustedSourceError> {
    let limitations = limitations
        .and_then(Value::as_array)
        .ok_or(TrustedSourceError::InvalidEvidence)?;
    if limitations.len() != LIMITATIONS.len()
        || limitations
            .iter()
            .zip(LIMITATIONS)
            .any(|(observed, expected)| observed.as_str() != Some(expected))
    {
        return Err(TrustedSourceError::InvalidEvidence);
    }
    Ok(())
}

fn source_position(source: &str, offset: usize) -> Result<Value, TrustedSourceError> {
    let prefix = source
        .get(..offset)
        .ok_or(TrustedSourceError::ContentRejected)?;
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let current_line = prefix
        .rsplit_once('\n')
        .map_or(prefix, |(_, suffix)| suffix);
    let column = current_line.chars().count() + 1;
    Ok(json!({
        "line": u64::try_from(line).map_err(|_| TrustedSourceError::Internal)?,
        "column": u64::try_from(column).map_err(|_| TrustedSourceError::Internal)?,
        "unit": "unicode_scalar"
    }))
}

fn required_string<'a>(value: &'a Value, field: &str) -> Result<&'a str, TrustedSourceError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or(TrustedSourceError::InvalidEvidence)
}

fn has_exact_keys(value: &Value, expected: &[&str]) -> bool {
    value.as_object().is_some_and(|object| {
        object.len() == expected.len() && expected.iter().all(|key| object.contains_key(*key))
    })
}

fn valid_evidence_id(value: &str) -> bool {
    [
        "urn:codenoesis:evidence:blake3:",
        "urn:codenoesis:evidence:sha256:",
    ]
    .iter()
    .any(|prefix| value.strip_prefix(prefix).is_some_and(valid_sha256))
}

fn valid_snapshot_id(value: &str) -> bool {
    value
        .strip_prefix("urn:codenoesis:snapshot:blake3:")
        .is_some_and(valid_sha256)
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn valid_repository_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_R18_PATH_BYTES
        && !value.starts_with('/')
        && !value.contains('\\')
        && !value.chars().any(char::is_control)
        && value
            .split('/')
            .all(|component| !component.is_empty() && component != "." && component != "..")
}

fn valid_position(value: Option<&Value>) -> bool {
    value.is_some_and(|position| {
        has_exact_keys(position, &["column", "line", "unit"])
            && position
                .get("line")
                .and_then(Value::as_u64)
                .is_some_and(|line| line > 0)
            && position
                .get("column")
                .and_then(Value::as_u64)
                .is_some_and(|column| column > 0)
            && position.get("unit").and_then(Value::as_str) == Some("unicode_scalar")
    })
}

#[cfg(test)]
mod tests {
    use codenoesis_domain::{
        AcquiredFile, AcquiredRepository, BoundRevision, RegularFileMode, RepositoryInventory,
    };

    use super::*;

    #[test]
    fn conf_fr_ctx_002_positions_are_unicode_scalar_and_crlf_aware() {
        let source = "α\r\nbeta\n";
        let selection = selection(0, u64::try_from(source.len()).expect("source length"));
        let inventory = inventory(source.as_bytes());
        let excerpt = TrustedSourceExcerptV1::from_inventory(&selection, &inventory, test_sha256)
            .expect("valid source excerpt");
        assert_eq!(
            excerpt.value()["evidence"]["span"]["start_position"]["line"],
            1
        );
        assert_eq!(
            excerpt.value()["evidence"]["span"]["start_position"]["column"],
            1
        );
        assert_eq!(
            excerpt.value()["evidence"]["span"]["end_position"]["line"],
            3
        );
        assert_eq!(
            excerpt.value()["evidence"]["span"]["end_position"]["column"],
            1
        );
        TrustedSourceExcerptV1::from_canonical_stdout(&excerpt.canonical_stdout(), test_sha256)
            .expect("canonical reimport");
    }

    #[test]
    fn sec_fr_ctx_002_rejects_non_utf8_and_non_scalar_boundaries() {
        let invalid_utf8 = inventory(&[0xff]);
        assert_eq!(
            TrustedSourceExcerptV1::from_inventory(&selection(0, 1), &invalid_utf8, test_sha256)
                .unwrap_err(),
            TrustedSourceError::ContentRejected
        );
        let multibyte = inventory("α".as_bytes());
        assert_eq!(
            TrustedSourceExcerptV1::from_inventory(&selection(1, 2), &multibyte, test_sha256)
                .unwrap_err(),
            TrustedSourceError::ContentRejected
        );
    }

    #[test]
    fn inv_bnd_001_excerpt_maximum_plus_one_is_rejected() {
        let bytes = vec![b'a'; usize::try_from(MAX_R18_EXCERPT_BYTES + 1).expect("limit")];
        let inventory = inventory(&bytes);
        assert_eq!(
            TrustedSourceExcerptV1::from_inventory(
                &selection(0, MAX_R18_EXCERPT_BYTES + 1),
                &inventory,
                test_sha256,
            )
            .unwrap_err(),
            TrustedSourceError::LimitExceeded
        );
    }

    #[test]
    fn inv_bnd_001_exact_excerpt_and_stdout_limits_are_enforced() {
        let bytes = vec![b'a'; usize::try_from(MAX_R18_EXCERPT_BYTES).expect("limit")];
        let excerpt = TrustedSourceExcerptV1::from_inventory(
            &selection(0, MAX_R18_EXCERPT_BYTES),
            &inventory(&bytes),
            test_sha256,
        )
        .expect("exact excerpt maximum");
        assert!(
            u64::try_from(excerpt.canonical_stdout().len()).expect("stdout length")
                <= MAX_R18_STDOUT_BYTES
        );

        let exact_stdout = vec![b' '; usize::try_from(MAX_R18_STDOUT_BYTES).expect("limit")];
        assert_eq!(
            TrustedSourceExcerptV1::from_canonical_stdout(&exact_stdout, test_sha256).unwrap_err(),
            TrustedSourceError::InvalidEvidence
        );
        let oversized_stdout =
            vec![b' '; usize::try_from(MAX_R18_STDOUT_BYTES + 1).expect("limit")];
        assert_eq!(
            TrustedSourceExcerptV1::from_canonical_stdout(&oversized_stdout, test_sha256)
                .unwrap_err(),
            TrustedSourceError::LimitExceeded
        );
    }

    #[test]
    fn sec_fr_ctx_002_repository_blob_path_and_span_mismatches_fail_closed() {
        let source = inventory(b"a");

        let mut repository = selection(0, 1);
        repository.repository_identity =
            RepositoryIdentity::parse("urn:codenoesis:fixture:r18-other").expect("identity");
        assert_eq!(
            TrustedSourceExcerptV1::from_inventory(&repository, &source, test_sha256).unwrap_err(),
            TrustedSourceError::RepositoryMismatch
        );

        let mut commit = selection(0, 1);
        commit.commit_oid = object_id('2');
        assert_eq!(
            TrustedSourceExcerptV1::from_inventory(&commit, &source, test_sha256).unwrap_err(),
            TrustedSourceError::RepositoryMismatch
        );

        let mut tree = selection(0, 1);
        tree.tree_oid = object_id('3');
        assert_eq!(
            TrustedSourceExcerptV1::from_inventory(&tree, &source, test_sha256).unwrap_err(),
            TrustedSourceError::RepositoryMismatch
        );

        let mut blob = selection(0, 1);
        blob.blob_oid = object_id('4');
        assert_eq!(
            TrustedSourceExcerptV1::from_inventory(&blob, &source, test_sha256).unwrap_err(),
            TrustedSourceError::RepositoryMismatch
        );

        let mut path = selection(0, 1);
        path.path = "src/missing.rs".to_owned();
        assert_eq!(
            TrustedSourceExcerptV1::from_inventory(&path, &source, test_sha256).unwrap_err(),
            TrustedSourceError::PathRejected
        );

        for span in [selection(0, 2), selection(1, 1)] {
            assert_eq!(
                TrustedSourceExcerptV1::from_inventory(&span, &source, test_sha256).unwrap_err(),
                TrustedSourceError::InvalidEvidence
            );
        }
    }

    #[test]
    fn sec_fr_ctx_002_repository_paths_are_bounded_and_relative() {
        assert!(valid_repository_path("a"));
        assert!(valid_repository_path(&"a".repeat(MAX_R18_PATH_BYTES)));
        for path in [
            "",
            "/src/lib.rs",
            "../src/lib.rs",
            "src/../lib.rs",
            "src/./lib.rs",
            "src//lib.rs",
            "src\\lib.rs",
            "src/\u{0}lib.rs",
            &"a".repeat(MAX_R18_PATH_BYTES + 1),
        ] {
            assert!(
                !valid_repository_path(path),
                "unsafe path accepted: {path:?}"
            );
        }
    }

    #[test]
    fn sec_fr_ctx_002_lfs_pointer_is_never_resolved_as_remote_content() {
        let pointer = concat!(
            "version https://git-lfs.github.com/spec/v1\n",
            "oid sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n",
            "size 123456\n",
        );
        let excerpt = TrustedSourceExcerptV1::from_inventory(
            &selection(0, u64::try_from(pointer.len()).expect("pointer length")),
            &inventory(pointer.as_bytes()),
            test_sha256,
        )
        .expect("immutable LFS pointer bytes");
        assert_eq!(excerpt.value()["excerpt"]["text"], pointer);
        assert_eq!(
            excerpt.value()["authority"],
            "explicit_local_git_object_only"
        );
    }

    fn selection(start_byte: u64, end_byte: u64) -> TrustedSourceSelectionV1 {
        TrustedSourceSelectionV1 {
            repository_identity: identity(),
            commit_oid: object_id('a'),
            tree_oid: object_id('b'),
            snapshot_id: format!("urn:codenoesis:snapshot:blake3:{}", "c".repeat(64)),
            snapshot_semantic_hash: "d".repeat(64),
            graph_semantic_hash: "e".repeat(64),
            evidence_id: format!("urn:codenoesis:evidence:blake3:{}", "f".repeat(64)),
            path: "src/lib.rs".to_owned(),
            blob_oid: object_id('1'),
            start_byte,
            end_byte,
        }
    }

    fn inventory(bytes: &[u8]) -> RepositoryInventory {
        RepositoryInventory::classify(AcquiredRepository::new(
            BoundRevision::new(identity(), object_id('a'), object_id('b')),
            1,
            vec![AcquiredFile::new(
                "src/lib.rs".to_owned(),
                RegularFileMode::Regular,
                object_id('1'),
                bytes.to_vec(),
            )],
        ))
    }

    fn identity() -> RepositoryIdentity {
        RepositoryIdentity::parse("urn:codenoesis:fixture:r18-contract").expect("identity")
    }

    fn object_id(value: char) -> ObjectId {
        ObjectId::parse_sha1(&value.to_string().repeat(40)).expect("object ID")
    }

    fn test_sha256(bytes: &[u8]) -> String {
        format!("{:064x}", bytes.len())
    }
}
