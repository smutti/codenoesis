use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use codenoesis_domain::s1_boundaries::{
    BoundaryLimit, NestedAcquisitionProfile, NestedRepositoryInput, RepositoryBoundaryError,
    RepositoryBoundaryEvidence, RepositoryBoundaryInput, RepositoryBoundaryReport,
    boundary_limit_exceeded, check_boundary_limit, validate_canonical_relative_path,
};
use codenoesis_domain::storage::{
    LocalSnapshotHead, PublicationCandidate, SNAPSHOT_SCHEMA_VERSION_V5, StorageComponent,
    StorageError,
};
use codenoesis_domain::{
    AcquisitionError, LimitKind, ObjectId, RepositoryIdentity, STANDARD_LOCAL_S1_LIMITS,
    limit_exceeded,
};
use serde_json::{Map, Value, json};

use crate::{
    LimitedVecWriter, PublicationCandidateError, RepositorySnapshotV4, publication_candidate,
    semantic_hash,
};

const CONFIGURATION_V2_HASH_DOMAIN: &[u8] = b"codenoesis.configuration.semantic.v2";
const SNAPSHOT_V5_HASH_DOMAIN: &[u8] = b"codenoesis.repository-snapshot.semantic.v5";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoundaryManifestReason {
    SchemaInvalid,
    ManifestUnavailable,
    RootMismatch,
    DuplicateBoundaryPath,
    RecursiveInput,
}

impl BoundaryManifestReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SchemaInvalid => "schema_invalid",
            Self::ManifestUnavailable => "manifest_unavailable",
            Self::RootMismatch => "root_mismatch",
            Self::DuplicateBoundaryPath => "duplicate_boundary_path",
            Self::RecursiveInput => "recursive_input",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RepositoryBoundaryInputError {
    Invalid(BoundaryManifestReason),
    Limit(RepositoryBoundaryError),
}

impl Display for RepositoryBoundaryInputError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Invalid(_) => "invalid repository boundary manifest",
            Self::Limit(_) => "repository boundary manifest limit exceeded",
        })
    }
}

impl Error for RepositoryBoundaryInputError {}

/// Parses strict duplicate-free `RepositoryBoundaryInputV1` bytes.
///
/// # Errors
///
/// Returns one closed manifest reason or a capped fixed-limit error.
pub fn parse_repository_boundary_input(
    bytes: &[u8],
) -> Result<RepositoryBoundaryInput, RepositoryBoundaryInputError> {
    check_boundary_limit(
        BoundaryLimit::BoundaryManifestBytes,
        u64::try_from(bytes.len()).unwrap_or(u64::MAX),
    )
    .map_err(RepositoryBoundaryInputError::Limit)?;
    if std::str::from_utf8(bytes).is_err() || JsonMemberChecker::check(bytes).is_err() {
        return Err(invalid_manifest(BoundaryManifestReason::SchemaInvalid));
    }
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| invalid_manifest(BoundaryManifestReason::SchemaInvalid))?;
    let root = exact_object(&value, &["nested_repositories", "root", "schema_version"])?;
    if root.get("schema_version").and_then(Value::as_str)
        != Some("codenoesis.repository-boundary-input/v1")
    {
        return Err(invalid_manifest(BoundaryManifestReason::SchemaInvalid));
    }
    let root_binding = exact_object(
        required(root, "root")?,
        &["commit_oid", "repository_identity"],
    )?;
    let root_repository_identity =
        parse_repository_identity(required_str(root_binding, "repository_identity")?)?;
    let root_commit_oid = parse_oid(required_str(root_binding, "commit_oid")?)?;
    let nested = required(root, "nested_repositories")?
        .as_array()
        .ok_or_else(|| invalid_manifest(BoundaryManifestReason::SchemaInvalid))?;
    check_boundary_limit(
        BoundaryLimit::ExplicitNestedRepositories,
        u64::try_from(nested.len()).unwrap_or(u64::MAX),
    )
    .map_err(RepositoryBoundaryInputError::Limit)?;

    let mut nested_repositories = Vec::with_capacity(nested.len());
    let mut paths = BTreeSet::new();
    let mut identities = BTreeSet::new();
    let mut roots = BTreeSet::new();
    for entry in nested {
        let Some(object) = entry.as_object() else {
            return Err(invalid_manifest(BoundaryManifestReason::SchemaInvalid));
        };
        if object.contains_key("nested_repositories") {
            return Err(invalid_manifest(BoundaryManifestReason::RecursiveInput));
        }
        let object = exact_object(
            entry,
            &[
                "acquisition_profile",
                "boundary_path",
                "repository_identity",
                "repository_root",
                "revision",
            ],
        )?;
        let boundary_path = required_str(object, "boundary_path")?.to_owned();
        validate_canonical_relative_path(&boundary_path)
            .map_err(|_| invalid_manifest(BoundaryManifestReason::SchemaInvalid))?;
        if !paths.insert(boundary_path.clone()) {
            return Err(invalid_manifest(
                BoundaryManifestReason::DuplicateBoundaryPath,
            ));
        }
        let repository_identity =
            parse_repository_identity(required_str(object, "repository_identity")?)?;
        if repository_identity == root_repository_identity {
            return Err(invalid_manifest(BoundaryManifestReason::SchemaInvalid));
        }
        if !identities.insert(repository_identity.as_str().to_owned()) {
            return Err(invalid_manifest(BoundaryManifestReason::SchemaInvalid));
        }
        let repository_root = required_str(object, "repository_root")?.to_owned();
        validate_canonical_relative_path(&repository_root)
            .map_err(|_| invalid_manifest(BoundaryManifestReason::SchemaInvalid))?;
        if !roots.insert(repository_root.clone()) {
            return Err(invalid_manifest(BoundaryManifestReason::SchemaInvalid));
        }
        let revision = parse_oid(required_str(object, "revision")?)?;
        let acquisition_profile =
            NestedAcquisitionProfile::parse(required_str(object, "acquisition_profile")?)
                .ok_or_else(|| invalid_manifest(BoundaryManifestReason::SchemaInvalid))?;
        nested_repositories.push(NestedRepositoryInput {
            boundary_path,
            repository_identity,
            repository_root,
            revision,
            acquisition_profile,
        });
    }
    nested_repositories.sort_by(|left, right| {
        left.boundary_path
            .as_bytes()
            .cmp(right.boundary_path.as_bytes())
    });
    Ok(RepositoryBoundaryInput {
        root_repository_identity,
        root_commit_oid,
        nested_repositories,
    })
}

fn parse_oid(value: &str) -> Result<ObjectId, RepositoryBoundaryInputError> {
    ObjectId::parse_sha1(value)
        .ok_or_else(|| invalid_manifest(BoundaryManifestReason::SchemaInvalid))
}

fn parse_repository_identity(
    value: &str,
) -> Result<RepositoryIdentity, RepositoryBoundaryInputError> {
    if value.len() > 255 {
        return Err(invalid_manifest(BoundaryManifestReason::SchemaInvalid));
    }
    RepositoryIdentity::parse(value)
        .map_err(|_| invalid_manifest(BoundaryManifestReason::SchemaInvalid))
}

fn exact_object<'a>(
    value: &'a Value,
    expected: &[&str],
) -> Result<&'a Map<String, Value>, RepositoryBoundaryInputError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid_manifest(BoundaryManifestReason::SchemaInvalid))?;
    if object.len() != expected.len() || !expected.iter().all(|key| object.contains_key(*key)) {
        return Err(invalid_manifest(BoundaryManifestReason::SchemaInvalid));
    }
    Ok(object)
}

fn required<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a Value, RepositoryBoundaryInputError> {
    object
        .get(key)
        .ok_or_else(|| invalid_manifest(BoundaryManifestReason::SchemaInvalid))
}

fn required_str<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a str, RepositoryBoundaryInputError> {
    required(object, key)?
        .as_str()
        .ok_or_else(|| invalid_manifest(BoundaryManifestReason::SchemaInvalid))
}

fn invalid_manifest(reason: BoundaryManifestReason) -> RepositoryBoundaryInputError {
    RepositoryBoundaryInputError::Invalid(reason)
}

struct JsonMemberChecker<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl JsonMemberChecker<'_> {
    fn check(bytes: &[u8]) -> Result<(), ()> {
        let mut checker = JsonMemberChecker { bytes, offset: 0 };
        checker.value(0)?;
        checker.whitespace();
        (checker.offset == bytes.len()).then_some(()).ok_or(())
    }

    fn value(&mut self, depth: usize) -> Result<(), ()> {
        if depth > 16 {
            return Err(());
        }
        self.whitespace();
        match self.bytes.get(self.offset) {
            Some(b'{') => self.object(depth + 1),
            Some(b'[') => self.array(depth + 1),
            Some(b'"') => self.string().map(|_| ()),
            Some(_) => self.primitive(),
            None => Err(()),
        }
    }

    fn object(&mut self, depth: usize) -> Result<(), ()> {
        self.offset += 1;
        self.whitespace();
        if self.take(b'}') {
            return Ok(());
        }
        let mut members = BTreeSet::new();
        loop {
            self.whitespace();
            let range = self.string()?;
            let key: String = serde_json::from_slice(&self.bytes[range]).map_err(|_| ())?;
            if !members.insert(key) {
                return Err(());
            }
            self.whitespace();
            if !self.take(b':') {
                return Err(());
            }
            self.value(depth)?;
            self.whitespace();
            if self.take(b'}') {
                return Ok(());
            }
            if !self.take(b',') {
                return Err(());
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<(), ()> {
        self.offset += 1;
        self.whitespace();
        if self.take(b']') {
            return Ok(());
        }
        loop {
            self.value(depth)?;
            self.whitespace();
            if self.take(b']') {
                return Ok(());
            }
            if !self.take(b',') {
                return Err(());
            }
        }
    }

    fn string(&mut self) -> Result<std::ops::Range<usize>, ()> {
        let start = self.offset;
        if !self.take(b'"') {
            return Err(());
        }
        while let Some(byte) = self.bytes.get(self.offset).copied() {
            self.offset += 1;
            match byte {
                b'"' => return Ok(start..self.offset),
                b'\\' => {
                    let escaped = self.bytes.get(self.offset).copied().ok_or(())?;
                    self.offset += 1;
                    if escaped == b'u' {
                        let end = self.offset.checked_add(4).ok_or(())?;
                        if end > self.bytes.len()
                            || !self.bytes[self.offset..end]
                                .iter()
                                .all(u8::is_ascii_hexdigit)
                        {
                            return Err(());
                        }
                        self.offset = end;
                    } else if !matches!(
                        escaped,
                        b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't'
                    ) {
                        return Err(());
                    }
                }
                0..=0x1f => return Err(()),
                _ => {}
            }
        }
        Err(())
    }

    fn primitive(&mut self) -> Result<(), ()> {
        let start = self.offset;
        while self
            .bytes
            .get(self.offset)
            .is_some_and(|byte| !byte.is_ascii_whitespace() && !matches!(byte, b',' | b']' | b'}'))
        {
            self.offset += 1;
        }
        (self.offset > start).then_some(()).ok_or(())
    }

    fn whitespace(&mut self) {
        while self
            .bytes
            .get(self.offset)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.offset += 1;
        }
    }

    fn take(&mut self, expected: u8) -> bool {
        if self.bytes.get(self.offset) == Some(&expected) {
            self.offset += 1;
            true
        } else {
            false
        }
    }
}

#[derive(Clone, Debug)]
pub struct RepositorySnapshotV5 {
    value: Value,
}

#[derive(Debug)]
pub enum RepositorySnapshotV5Error {
    Serialization(serde_json::Error),
    Boundary(RepositoryBoundaryError),
    LimitExceeded(AcquisitionError),
    OutputLengthOverflow,
}

impl RepositorySnapshotV5 {
    /// Extends an already constructed V4 root analysis with the R2 boundary projection.
    ///
    /// # Errors
    ///
    /// Returns a serialization or fixed boundary-report limit error.
    pub fn from_v4_and_boundaries(
        v4: &RepositorySnapshotV4,
        boundaries: &RepositoryBoundaryReport,
    ) -> Result<Self, RepositorySnapshotV5Error> {
        validate_repository_boundary_report_size(boundaries)
            .map_err(RepositorySnapshotV5Error::Boundary)?;
        let boundary_value = repository_boundary_value(boundaries);
        let mut semantic = v4.value()["semantic"].clone();
        let semantic_object = semantic
            .as_object_mut()
            .ok_or(RepositorySnapshotV5Error::OutputLengthOverflow)?;
        let configuration_semantic = json!({
            "profile": "standard-local-s4",
            "repository_boundary_profile": "local-gitlinks-v1"
        });
        let configuration_hash =
            semantic_hash(CONFIGURATION_V2_HASH_DOMAIN, &configuration_semantic);
        semantic_object.insert(
            "configuration".to_owned(),
            json!({
                "schema_version": "codenoesis.configuration/v2",
                "profile": "standard-local-s4",
                "repository_boundary_profile": "local-gitlinks-v1",
                "semantic_hash": {
                    "algorithm": "blake3-256",
                    "value": configuration_hash
                }
            }),
        );
        semantic_object.insert(
            "pipeline_version".to_owned(),
            Value::String("codenoesis.pipeline/s4-r2-v1".to_owned()),
        );
        semantic_object.insert(
            "extractor_versions".to_owned(),
            json!([
                "codenoesis.inventory-classifier/s1-v1",
                "codenoesis.rust-tree-sitter/s4-v1",
                "codenoesis.rust-workspace/s4-v1",
                "codenoesis.git-boundary/s1-v1"
            ]),
        );
        semantic_object.insert("repository_boundaries".to_owned(), boundary_value);
        let snapshot_hash = semantic_hash(SNAPSHOT_V5_HASH_DOMAIN, &semantic);
        Ok(Self {
            value: json!({
                "schema_version": "codenoesis.repository-snapshot/v5",
                "semantic_hash": {
                    "algorithm": "blake3-256",
                    "value": snapshot_hash
                },
                "semantic": semantic,
                "envelope": v4.value()["envelope"].clone()
            }),
        })
    }

    /// # Errors
    ///
    /// Returns an error when serialization fails or the canonical output limit is exceeded.
    pub fn canonical_stdout(&self) -> Result<Vec<u8>, RepositorySnapshotV5Error> {
        let maximum = usize::try_from(STANDARD_LOCAL_S1_LIMITS.canonical_output_bytes)
            .map_err(|_| RepositorySnapshotV5Error::OutputLengthOverflow)?;
        let body_maximum = maximum
            .checked_sub(1)
            .ok_or(RepositorySnapshotV5Error::OutputLengthOverflow)?;
        let mut writer = LimitedVecWriter::new(body_maximum);
        let result = serde_json::to_writer(&mut writer, &self.value);
        if writer.overflowed() {
            return Err(RepositorySnapshotV5Error::LimitExceeded(limit_exceeded(
                LimitKind::CanonicalOutputBytes,
                STANDARD_LOCAL_S1_LIMITS
                    .canonical_output_bytes
                    .saturating_add(1),
            )));
        }
        result.map_err(RepositorySnapshotV5Error::Serialization)?;
        let mut bytes = writer.into_inner();
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// # Errors
    ///
    /// Returns an error when the semantic value cannot be serialized.
    pub fn canonical_semantic(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(&self.value["semantic"])
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// # Errors
    ///
    /// Returns an error when the snapshot does not satisfy the publication contract.
    pub fn publication_candidate(&self) -> Result<PublicationCandidate, PublicationCandidateError> {
        publication_candidate(&self.value)
    }
}

/// Validates the canonical one-MiB R2 boundary-report limit.
///
/// # Errors
///
/// Returns a capped boundary-report limit error or an invalid-report error.
pub fn validate_repository_boundary_report_size(
    report: &RepositoryBoundaryReport,
) -> Result<(), RepositoryBoundaryError> {
    let boundary_value = repository_boundary_value(report);
    let boundary_bytes =
        serde_json::to_vec(&boundary_value).map_err(|_| RepositoryBoundaryError::InvalidReport)?;
    check_boundary_limit(
        BoundaryLimit::BoundaryReportBytes,
        u64::try_from(boundary_bytes.len()).unwrap_or(u64::MAX),
    )
}

#[must_use]
pub fn repository_boundary_value(report: &RepositoryBoundaryReport) -> Value {
    let bound_count = report
        .boundaries
        .iter()
        .filter(|boundary| boundary.nested_repository.is_some())
        .count();
    let unbound_count = report.boundaries.len().saturating_sub(bound_count);
    json!({
        "schema_version": "codenoesis.repository-boundaries/v1",
        "profile": "local-gitlinks-v1",
        "root_repository": repository_value(&report.root_repository),
        "summary": {
            "boundary_count": report.boundaries.len(),
            "declaration_count": report.declarations.len(),
            "bound_count": bound_count,
            "unbound_count": unbound_count,
            "coverage_gap_count": report.coverage_gaps.len()
        },
        "boundaries": report.boundaries.iter().map(|boundary| json!({
            "boundary_id": boundary.boundary_id,
            "path": boundary.path,
            "gitlink_oid": boundary.gitlink_oid.as_str(),
            "state": boundary.state.as_str(),
            "declaration_id": boundary.declaration_id,
            "nested_repository": boundary.nested_repository.as_ref().map(repository_value),
            "evidence_ids": boundary.evidence_ids,
            "coverage_gap_ids": boundary.coverage_gap_ids
        })).collect::<Vec<_>>(),
        "declarations": report.declarations.iter().map(|declaration| json!({
            "declaration_id": declaration.declaration_id,
            "name_sha256": declaration.name_sha256,
            "path": declaration.path,
            "url_kind": declaration.url_kind.as_str(),
            "url_sha256": declaration.url_sha256,
            "unsupported_keys": declaration.unsupported_keys.iter().map(|key| json!({
                "key": key.key,
                "value_sha256": key.value_sha256
            })).collect::<Vec<_>>(),
            "boundary_id": declaration.boundary_id,
            "evidence_id": declaration.evidence_id
        })).collect::<Vec<_>>(),
        "coverage_gaps": report.coverage_gaps.iter().map(|gap| json!({
            "gap_id": gap.gap_id,
            "code": gap.code,
            "path": gap.path,
            "subject_id": gap.subject_id,
            "evidence_ids": gap.evidence_ids
        })).collect::<Vec<_>>(),
        "evidence": report.evidence.iter().map(|evidence| match evidence {
            RepositoryBoundaryEvidence::GitTreeEntry {
                evidence_id,
                tree_oid,
                path,
                object_oid,
            } => json!({
                "evidence_id": evidence_id,
                "kind": "git_tree_entry",
                "repository": repository_reference_value(&report.root_repository),
                "tree_oid": tree_oid.as_str(),
                "path": path,
                "mode": "160000",
                "object_oid": object_oid.as_str()
            }),
            RepositoryBoundaryEvidence::GitmodulesDeclaration {
                evidence_id,
                blob_oid,
                start_byte,
                end_byte,
            } => json!({
                "evidence_id": evidence_id,
                "kind": "gitmodules_declaration",
                "repository": repository_reference_value(&report.root_repository),
                "blob_oid": blob_oid.as_str(),
                "path": ".gitmodules",
                "span": {
                    "unit": "byte",
                    "start": start_byte,
                    "end": end_byte
                }
            })
        }).collect::<Vec<_>>()
    })
}

fn repository_value(bound: &codenoesis_domain::BoundRevision) -> Value {
    json!({
        "identity": bound.repository_identity().as_str(),
        "vcs": "git",
        "object_format": "sha1",
        "commit_oid": bound.commit_oid().as_str(),
        "tree_oid": bound.tree_oid().as_str()
    })
}

fn repository_reference_value(bound: &codenoesis_domain::BoundRevision) -> Value {
    json!({
        "identity": bound.repository_identity().as_str(),
        "commit_oid": bound.commit_oid().as_str()
    })
}

/// # Errors
///
/// Returns an error when stored semantic content does not match its immutable head.
pub fn validate_stored_snapshot_semantic_v5(
    semantic: &Value,
    head: &LocalSnapshotHead,
) -> Result<(), StorageError> {
    if head.snapshot_schema_version != SNAPSHOT_SCHEMA_VERSION_V5 {
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

fn stored_snapshot_error(head: &LocalSnapshotHead, reason: &'static str) -> StorageError {
    StorageError::CorruptMetadata {
        component: StorageComponent::Head,
        reason,
        snapshot_id: Some(head.snapshot_id.to_string()),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NestedRepositoryUnavailableReason {
    NotGitRepository,
    RevisionNotFound,
    RevisionNotCommit,
    ObjectMissing,
    RootPolicyViolation,
    RepositoryInconsistent,
    UnsupportedRepositoryShape,
    ObjectDatabaseInvalid,
    ObjectDatabaseUnavailable,
    ObjectLimitExceeded,
    PathInvalid,
    EntryPolicyViolation,
}

impl NestedRepositoryUnavailableReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotGitRepository => "not_git_repository",
            Self::RevisionNotFound => "revision_not_found",
            Self::RevisionNotCommit => "revision_not_commit",
            Self::ObjectMissing => "object_missing",
            Self::RootPolicyViolation => "root_policy_violation",
            Self::RepositoryInconsistent => "repository_inconsistent",
            Self::UnsupportedRepositoryShape => "unsupported_repository_shape",
            Self::ObjectDatabaseInvalid => "object_database_invalid",
            Self::ObjectDatabaseUnavailable => "object_database_unavailable",
            Self::ObjectLimitExceeded => "object_limit_exceeded",
            Self::PathInvalid => "path_invalid",
            Self::EntryPolicyViolation => "entry_policy_violation",
        }
    }
}

#[derive(Clone, Debug)]
pub struct CodeNoesisErrorV9 {
    value: Value,
}

impl CodeNoesisErrorV9 {
    #[must_use]
    pub fn invalid_profile() -> Self {
        Self::new(
            "input.invalid_repository_boundary_profile",
            "input",
            "invalid repository boundary profile",
            false,
            &json!({}),
        )
    }

    #[must_use]
    pub fn invalid_manifest(reason: BoundaryManifestReason) -> Self {
        Self::new(
            "input.invalid_repository_boundary_manifest",
            "input",
            "invalid repository boundary manifest",
            false,
            &json!({"component": "boundary_manifest", "reason": reason.as_str()}),
        )
    }

    #[must_use]
    pub fn from_boundary(error: &RepositoryBoundaryError) -> Self {
        match error {
            RepositoryBoundaryError::MetadataInvalid { reason, path } => {
                let mut context = Map::new();
                context.insert(
                    "component".to_owned(),
                    Value::String("gitmodules".to_owned()),
                );
                context.insert(
                    "reason".to_owned(),
                    Value::String(reason.as_str().to_owned()),
                );
                if let Some(path) = path {
                    context.insert("path".to_owned(), Value::String(path.clone()));
                }
                Self::new(
                    "acquisition.repository_boundary_metadata_invalid",
                    "acquisition",
                    "repository boundary metadata invalid",
                    false,
                    &Value::Object(context),
                )
            }
            RepositoryBoundaryError::LimitExceeded {
                limit,
                maximum,
                observed,
            } => Self::new(
                "acquisition.repository_boundary_limit_exceeded",
                "acquisition",
                "repository boundary limit exceeded",
                false,
                &json!({
                    "component": limit.component(),
                    "limit": limit.as_str(),
                    "maximum": maximum,
                    "observed": observed
                }),
            ),
            RepositoryBoundaryError::InvalidReport => Self::internal(),
        }
    }

    #[must_use]
    pub fn nested_mismatch(path: &str, expected: &ObjectId, observed: &ObjectId) -> Self {
        Self::new(
            "acquisition.nested_repository_mismatch",
            "acquisition",
            "nested repository mismatch",
            false,
            &json!({
                "component": "nested_repository",
                "path": path,
                "expected_oid": expected.as_str(),
                "observed_oid": observed.as_str()
            }),
        )
    }

    #[must_use]
    pub fn nested_unavailable(path: &str, reason: NestedRepositoryUnavailableReason) -> Self {
        Self::new(
            "acquisition.nested_repository_unavailable",
            "acquisition",
            "nested repository unavailable",
            false,
            &json!({
                "component": "nested_repository",
                "path": path,
                "reason": reason.as_str()
            }),
        )
    }

    #[must_use]
    pub fn nested_changed(path: &str) -> Self {
        Self::new(
            "acquisition.nested_repository_changed",
            "acquisition",
            "nested repository changed",
            true,
            &json!({
                "component": "nested_repository",
                "path": path,
                "reason": "changed_during_binding"
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
            &json!({}),
        )
    }

    fn new(code: &str, stage: &str, message: &str, retryable: bool, context: &Value) -> Self {
        Self {
            value: json!({
                "schema_version": "codenoesis.error/v9",
                "code": code,
                "stage": stage,
                "message": message,
                "retryable": retryable,
                "context": context
            }),
        }
    }

    /// # Errors
    ///
    /// Returns an error when the canonical error value cannot be serialized.
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

#[must_use]
pub fn manifest_limit_error(limit: BoundaryLimit, observed: u64) -> CodeNoesisErrorV9 {
    CodeNoesisErrorV9::from_boundary(&boundary_limit_exceeded(limit, observed))
}
