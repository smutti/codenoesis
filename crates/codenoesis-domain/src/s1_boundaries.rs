use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::{
    AcquiredRepository, AcquisitionError, BoundRevision, ObjectId, RepositoryError,
    RepositoryIdentity,
};

pub const LOCAL_GITLINKS_V1: &str = "local-gitlinks-v1";
pub const BOUNDARY_EXTRACTOR_VERSION: &str = "codenoesis.git-boundary/s1-v1";
pub const MAX_BOUNDARY_MANIFEST_BYTES: u64 = 262_144;
pub const MAX_GITLINK_ENTRIES: u64 = 128;
pub const MAX_GITMODULES_BYTES: u64 = 1_048_576;
pub const MAX_GITMODULES_SECTIONS: u64 = 256;
pub const MAX_GITMODULES_KEYS_PER_SECTION: u64 = 32;
pub const MAX_EXPLICIT_NESTED_REPOSITORIES: u64 = 32;
pub const MAX_EXPLICIT_NESTING_DEPTH: u64 = 1;
pub const MAX_BOUNDARY_REPORT_BYTES: u64 = 1_048_576;

pub trait BoundarySha256 {
    fn digest(&self, bytes: &[u8]) -> [u8; 32];
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoundaryLimit {
    BoundaryManifestBytes,
    GitlinkEntries,
    GitmodulesBytes,
    GitmodulesSections,
    GitmodulesKeysPerSection,
    ExplicitNestedRepositories,
    ExplicitNestingDepth,
    BoundaryReportBytes,
}

impl BoundaryLimit {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BoundaryManifestBytes => "boundary_manifest_bytes",
            Self::GitlinkEntries => "gitlink_entries",
            Self::GitmodulesBytes => "gitmodules_bytes",
            Self::GitmodulesSections => "gitmodules_sections",
            Self::GitmodulesKeysPerSection => "gitmodules_keys_per_section",
            Self::ExplicitNestedRepositories => "explicit_nested_repositories",
            Self::ExplicitNestingDepth => "explicit_nesting_depth",
            Self::BoundaryReportBytes => "boundary_report_bytes",
        }
    }

    #[must_use]
    pub const fn component(self) -> &'static str {
        match self {
            Self::BoundaryManifestBytes
            | Self::ExplicitNestedRepositories
            | Self::ExplicitNestingDepth => "boundary_manifest",
            Self::GitlinkEntries
            | Self::GitmodulesBytes
            | Self::GitmodulesSections
            | Self::GitmodulesKeysPerSection => "gitmodules",
            Self::BoundaryReportBytes => "boundary_report",
        }
    }

    #[must_use]
    pub const fn maximum(self) -> u64 {
        match self {
            Self::BoundaryManifestBytes => MAX_BOUNDARY_MANIFEST_BYTES,
            Self::GitlinkEntries => MAX_GITLINK_ENTRIES,
            Self::GitmodulesBytes => MAX_GITMODULES_BYTES,
            Self::GitmodulesSections => MAX_GITMODULES_SECTIONS,
            Self::GitmodulesKeysPerSection => MAX_GITMODULES_KEYS_PER_SECTION,
            Self::ExplicitNestedRepositories => MAX_EXPLICIT_NESTED_REPOSITORIES,
            Self::ExplicitNestingDepth => MAX_EXPLICIT_NESTING_DEPTH,
            Self::BoundaryReportBytes => MAX_BOUNDARY_REPORT_BYTES,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoundaryMetadataReason {
    InvalidEncoding,
    NulOrControl,
    BareCarriageReturn,
    MalformedSection,
    InvalidName,
    KeyOutsideSection,
    DuplicateSection,
    DuplicateKey,
    RequiredKeyMissing,
    PathInvalid,
    AmbiguousMapping,
    UnsafeEntryKind,
}

impl BoundaryMetadataReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidEncoding => "invalid_encoding",
            Self::NulOrControl => "nul_or_control",
            Self::BareCarriageReturn => "bare_carriage_return",
            Self::MalformedSection => "malformed_section",
            Self::InvalidName => "invalid_name",
            Self::KeyOutsideSection => "key_outside_section",
            Self::DuplicateSection => "duplicate_section",
            Self::DuplicateKey => "duplicate_key",
            Self::RequiredKeyMissing => "required_key_missing",
            Self::PathInvalid => "path_invalid",
            Self::AmbiguousMapping => "ambiguous_mapping",
            Self::UnsafeEntryKind => "unsafe_entry_kind",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RepositoryBoundaryError {
    MetadataInvalid {
        reason: BoundaryMetadataReason,
        path: Option<String>,
    },
    LimitExceeded {
        limit: BoundaryLimit,
        maximum: u64,
        observed: u64,
    },
    InvalidReport,
}

impl Display for RepositoryBoundaryError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MetadataInvalid { .. } => "repository boundary metadata invalid",
            Self::LimitExceeded { .. } => "repository boundary limit exceeded",
            Self::InvalidReport => "repository boundary report invalid",
        })
    }
}

impl Error for RepositoryBoundaryError {}

#[must_use]
pub const fn boundary_limit_exceeded(
    limit: BoundaryLimit,
    observed: u64,
) -> RepositoryBoundaryError {
    let maximum = limit.maximum();
    RepositoryBoundaryError::LimitExceeded {
        limit,
        maximum,
        observed: if observed > maximum + 1 {
            maximum + 1
        } else {
            observed
        },
    }
}

/// Checks one fixed R2 resource limit.
///
/// # Errors
///
/// Returns a capped maximum-plus-one boundary error.
pub fn check_boundary_limit(
    limit: BoundaryLimit,
    observed: u64,
) -> Result<(), RepositoryBoundaryError> {
    if observed > limit.maximum() {
        Err(boundary_limit_exceeded(limit, observed))
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcquiredGitlink {
    pub path: String,
    pub containing_tree_oid: ObjectId,
    pub gitlink_oid: ObjectId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcquiredGitmodules {
    pub mode: String,
    pub blob_oid: ObjectId,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcquiredRepositoryBoundaries {
    pub repository: AcquiredRepository,
    pub gitlinks: Vec<AcquiredGitlink>,
    pub gitmodules: Option<AcquiredGitmodules>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RepositoryBoundaryAcquisitionError {
    Repository(RepositoryError),
    Boundary(RepositoryBoundaryError),
}

impl From<RepositoryError> for RepositoryBoundaryAcquisitionError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}

impl From<AcquisitionError> for RepositoryBoundaryAcquisitionError {
    fn from(error: AcquisitionError) -> Self {
        Self::Repository(RepositoryError::Acquisition(error))
    }
}

impl From<RepositoryBoundaryError> for RepositoryBoundaryAcquisitionError {
    fn from(error: RepositoryBoundaryError) -> Self {
        Self::Boundary(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NestedAcquisitionProfile {
    VerifiedLooseSha1V1,
    LocalGitSha1PackedV1,
}

impl NestedAcquisitionProfile {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::VerifiedLooseSha1V1 => "verified-loose-sha1-v1",
            Self::LocalGitSha1PackedV1 => "local-git-sha1-packed-v1",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "verified-loose-sha1-v1" => Some(Self::VerifiedLooseSha1V1),
            "local-git-sha1-packed-v1" => Some(Self::LocalGitSha1PackedV1),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryBoundaryInput {
    pub root_repository_identity: RepositoryIdentity,
    pub root_commit_oid: ObjectId,
    pub nested_repositories: Vec<NestedRepositoryInput>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NestedRepositoryInput {
    pub boundary_path: String,
    pub repository_identity: RepositoryIdentity,
    pub repository_root: String,
    pub revision: ObjectId,
    pub acquisition_profile: NestedAcquisitionProfile,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedNestedRepository {
    pub boundary_path: String,
    pub bound_revision: BoundRevision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NestedRepositoryAcquisitionError {
    Repository(RepositoryError),
    Changed,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BoundaryUrlKind {
    Relative,
    AbsolutePath,
    File,
    Ssh,
    Https,
    Http,
    Git,
    ScpLike,
    Other,
}

impl BoundaryUrlKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Relative => "relative",
            Self::AbsolutePath => "absolute_path",
            Self::File => "file",
            Self::Ssh => "ssh",
            Self::Https => "https",
            Self::Http => "http",
            Self::Git => "git",
            Self::ScpLike => "scp_like",
            Self::Other => "other",
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct UnsupportedGitmodulesKey {
    pub key: String,
    pub value_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitmodulesDeclaration {
    pub declaration_id: String,
    pub name_sha256: String,
    pub path: String,
    pub url_kind: BoundaryUrlKind,
    pub url_sha256: String,
    pub unsupported_keys: Vec<UnsupportedGitmodulesKey>,
    pub evidence_id: String,
    pub blob_oid: ObjectId,
    pub start_byte: u64,
    pub end_byte: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ParsedGitmodules {
    pub declarations: Vec<GitmodulesDeclaration>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryBoundaryReport {
    pub root_repository: BoundRevision,
    pub boundaries: Vec<RepositoryBoundary>,
    pub declarations: Vec<RepositoryBoundaryDeclaration>,
    pub coverage_gaps: Vec<RepositoryBoundaryGap>,
    pub evidence: Vec<RepositoryBoundaryEvidence>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryBoundary {
    pub boundary_id: String,
    pub path: String,
    pub gitlink_oid: ObjectId,
    pub state: RepositoryBoundaryState,
    pub declaration_id: Option<String>,
    pub nested_repository: Option<BoundRevision>,
    pub evidence_ids: Vec<String>,
    pub coverage_gap_ids: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepositoryBoundaryState {
    DeclaredUnbound,
    UndeclaredUnbound,
    ExplicitlyBound,
}

impl RepositoryBoundaryState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DeclaredUnbound => "declared_unbound",
            Self::UndeclaredUnbound => "undeclared_unbound",
            Self::ExplicitlyBound => "explicitly_bound",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryBoundaryDeclaration {
    pub declaration_id: String,
    pub name_sha256: String,
    pub path: String,
    pub url_kind: BoundaryUrlKind,
    pub url_sha256: String,
    pub unsupported_keys: Vec<UnsupportedGitmodulesKey>,
    pub boundary_id: Option<String>,
    pub evidence_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryBoundaryGap {
    pub gap_id: String,
    pub code: &'static str,
    pub path: String,
    pub subject_id: String,
    pub evidence_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RepositoryBoundaryEvidence {
    GitTreeEntry {
        evidence_id: String,
        tree_oid: ObjectId,
        path: String,
        object_oid: ObjectId,
    },
    GitmodulesDeclaration {
        evidence_id: String,
        blob_oid: ObjectId,
        start_byte: u64,
        end_byte: u64,
    },
}

struct PendingSection {
    name_sha256: String,
    start_byte: u64,
    end_byte: u64,
    values: BTreeMap<String, String>,
}

/// Parses the complete approved committed `.gitmodules` subset.
///
/// # Errors
///
/// Returns a typed bounded metadata failure without retaining raw URLs.
#[allow(clippy::too_many_lines)]
pub fn parse_gitmodules(
    root: &BoundRevision,
    source: Option<&AcquiredGitmodules>,
    hasher: &impl BoundarySha256,
) -> Result<ParsedGitmodules, RepositoryBoundaryError> {
    let Some(source) = source else {
        return Ok(ParsedGitmodules::default());
    };
    if source.mode != "100644" {
        return Err(metadata_error(BoundaryMetadataReason::UnsafeEntryKind));
    }
    check_boundary_limit(
        BoundaryLimit::GitmodulesBytes,
        u64::try_from(source.bytes.len()).unwrap_or(u64::MAX),
    )?;
    validate_gitmodules_bytes(&source.bytes)?;
    let text = std::str::from_utf8(&source.bytes)
        .map_err(|_| metadata_error(BoundaryMetadataReason::InvalidEncoding))?;
    let lines = gitmodules_lines(text.as_bytes());
    let mut sections = Vec::<PendingSection>::new();
    let mut section_names = BTreeSet::new();
    let mut current = None::<PendingSection>;

    for line in lines {
        let trimmed = trim_horizontal(&text.as_bytes()[line.start..line.content_end]);
        if trimmed.is_empty() || matches!(trimmed.first(), Some(b'#' | b';')) {
            continue;
        }
        if trimmed.starts_with(b"[") {
            if let Some(mut previous) = current.take() {
                validate_required_gitmodules_keys(&previous)?;
                previous.end_byte = u64::try_from(line.start).unwrap_or(u64::MAX);
                sections.push(previous);
            }
            let name = parse_section_name(trimmed)?;
            if section_names.contains(name) {
                return Err(metadata_error(BoundaryMetadataReason::DuplicateSection));
            }
            check_boundary_limit(
                BoundaryLimit::GitmodulesSections,
                u64::try_from(section_names.len() + 1).unwrap_or(u64::MAX),
            )?;
            section_names.insert(name.to_owned());
            current = Some(PendingSection {
                name_sha256: sha256_hex(name.as_bytes(), hasher),
                start_byte: u64::try_from(line.start).unwrap_or(u64::MAX),
                end_byte: u64::try_from(source.bytes.len()).unwrap_or(u64::MAX),
                values: BTreeMap::new(),
            });
            continue;
        }
        let section = current
            .as_mut()
            .ok_or_else(|| metadata_error(BoundaryMetadataReason::KeyOutsideSection))?;
        let Some(equals) = trimmed.iter().position(|byte| *byte == b'=') else {
            return Err(metadata_error(BoundaryMetadataReason::MalformedSection));
        };
        let key = trim_horizontal(&trimmed[..equals]);
        let value = trim_horizontal(&trimmed[equals + 1..]);
        if !valid_key(key) {
            return Err(metadata_error(BoundaryMetadataReason::InvalidName));
        }
        if value.is_empty() || unsupported_value_syntax(value) {
            return Err(metadata_error(BoundaryMetadataReason::MalformedSection));
        }
        let key = std::str::from_utf8(key)
            .map_err(|_| metadata_error(BoundaryMetadataReason::InvalidName))?;
        let value = std::str::from_utf8(value)
            .map_err(|_| metadata_error(BoundaryMetadataReason::InvalidEncoding))?;
        if key == "include" {
            return Err(metadata_error(BoundaryMetadataReason::MalformedSection));
        }
        if section.values.contains_key(key) {
            return Err(metadata_error(BoundaryMetadataReason::DuplicateKey));
        }
        check_boundary_limit(
            BoundaryLimit::GitmodulesKeysPerSection,
            u64::try_from(section.values.len() + 1).unwrap_or(u64::MAX),
        )?;
        section.values.insert(key.to_owned(), value.to_owned());
    }
    if let Some(section) = current {
        validate_required_gitmodules_keys(&section)?;
        sections.push(section);
    }

    let mut declarations = Vec::with_capacity(sections.len());
    let mut paths = BTreeSet::new();
    for mut section in sections {
        let path = section
            .values
            .remove("path")
            .ok_or_else(|| metadata_error(BoundaryMetadataReason::RequiredKeyMissing))?;
        let url = section
            .values
            .remove("url")
            .ok_or_else(|| metadata_error(BoundaryMetadataReason::RequiredKeyMissing))?;
        validate_canonical_relative_path(&path).map_err(|_| {
            RepositoryBoundaryError::MetadataInvalid {
                reason: BoundaryMetadataReason::PathInvalid,
                path: safe_error_path(&path),
            }
        })?;
        if !paths.insert(path.clone()) {
            return Err(RepositoryBoundaryError::MetadataInvalid {
                reason: BoundaryMetadataReason::AmbiguousMapping,
                path: Some(path),
            });
        }
        let unsupported_keys = section
            .values
            .into_iter()
            .map(|(key, value)| UnsupportedGitmodulesKey {
                key,
                value_sha256: sha256_hex(value.as_bytes(), hasher),
            })
            .collect::<Vec<_>>();
        let declaration_id = prefixed_domain_id(
            "urn:codenoesis:gitmodules-declaration:sha256:",
            "codenoesis.gitmodules-declaration/v1",
            &[
                root.repository_identity().as_str(),
                root.commit_oid().as_str(),
                &path,
                &section.name_sha256,
            ],
            hasher,
        );
        let start = section.start_byte.to_string();
        let end = section.end_byte.to_string();
        let evidence_id = prefixed_domain_id(
            "urn:codenoesis:boundary-evidence:sha256:",
            "codenoesis.boundary-evidence.gitmodules/v1",
            &[
                root.repository_identity().as_str(),
                root.commit_oid().as_str(),
                source.blob_oid.as_str(),
                &start,
                &end,
            ],
            hasher,
        );
        declarations.push(GitmodulesDeclaration {
            declaration_id,
            name_sha256: section.name_sha256,
            path,
            url_kind: classify_url(&url),
            url_sha256: sha256_hex(url.as_bytes(), hasher),
            unsupported_keys,
            evidence_id,
            blob_oid: source.blob_oid.clone(),
            start_byte: section.start_byte,
            end_byte: section.end_byte,
        });
    }
    declarations.sort_by(|left, right| {
        left.path
            .as_bytes()
            .cmp(right.path.as_bytes())
            .then_with(|| left.declaration_id.cmp(&right.declaration_id))
    });
    Ok(ParsedGitmodules { declarations })
}

/// Builds the complete deterministic semantic boundary report.
///
/// # Errors
///
/// Returns an invalid-report error for duplicate or inconsistent verified input.
#[allow(clippy::too_many_lines)]
pub fn build_boundary_report(
    root: &BoundRevision,
    mut gitlinks: Vec<AcquiredGitlink>,
    mut parsed: ParsedGitmodules,
    verified_nested: &[VerifiedNestedRepository],
    hasher: &impl BoundarySha256,
) -> Result<RepositoryBoundaryReport, RepositoryBoundaryError> {
    check_boundary_limit(
        BoundaryLimit::GitlinkEntries,
        u64::try_from(gitlinks.len()).unwrap_or(u64::MAX),
    )?;
    check_boundary_limit(
        BoundaryLimit::ExplicitNestedRepositories,
        u64::try_from(verified_nested.len()).unwrap_or(u64::MAX),
    )?;
    if gitlinks
        .iter()
        .any(|gitlink| validate_canonical_relative_path(&gitlink.path).is_err())
    {
        return Err(RepositoryBoundaryError::InvalidReport);
    }
    gitlinks.sort_by(|left, right| left.path.as_bytes().cmp(right.path.as_bytes()));
    if gitlinks.windows(2).any(|pair| pair[0].path == pair[1].path) {
        return Err(RepositoryBoundaryError::InvalidReport);
    }
    for declaration in &mut parsed.declarations {
        declaration.unsupported_keys.sort();
    }
    parsed.declarations.sort_by(|left, right| {
        left.path
            .as_bytes()
            .cmp(right.path.as_bytes())
            .then_with(|| left.declaration_id.cmp(&right.declaration_id))
    });
    let declaration_by_path = parsed
        .declarations
        .iter()
        .map(|declaration| (declaration.path.as_str(), declaration))
        .collect::<BTreeMap<_, _>>();
    let nested_by_path = verified_nested
        .iter()
        .map(|nested| (nested.boundary_path.as_str(), &nested.bound_revision))
        .collect::<BTreeMap<_, _>>();
    if declaration_by_path.len() != parsed.declarations.len()
        || nested_by_path.len() != verified_nested.len()
        || nested_by_path.keys().any(|path| {
            !gitlinks
                .iter()
                .any(|gitlink| &gitlink.path.as_str() == path)
        })
    {
        return Err(RepositoryBoundaryError::InvalidReport);
    }
    let mut evidence = Vec::new();
    let mut tree_evidence_by_path = BTreeMap::new();
    let mut boundary_id_by_path = BTreeMap::new();
    for gitlink in &gitlinks {
        let boundary_id = boundary_id(root, gitlink, hasher);
        boundary_id_by_path.insert(gitlink.path.as_str(), boundary_id);
        let evidence_id = prefixed_domain_id(
            "urn:codenoesis:boundary-evidence:sha256:",
            "codenoesis.boundary-evidence.git-tree-entry/v1",
            &[
                root.repository_identity().as_str(),
                root.commit_oid().as_str(),
                gitlink.containing_tree_oid.as_str(),
                &gitlink.path,
                "160000",
                gitlink.gitlink_oid.as_str(),
            ],
            hasher,
        );
        tree_evidence_by_path.insert(gitlink.path.as_str(), evidence_id.clone());
        evidence.push(RepositoryBoundaryEvidence::GitTreeEntry {
            evidence_id,
            tree_oid: gitlink.containing_tree_oid.clone(),
            path: gitlink.path.clone(),
            object_oid: gitlink.gitlink_oid.clone(),
        });
    }
    let mut declaration_evidence = parsed.declarations.iter().collect::<Vec<_>>();
    declaration_evidence.sort_by(|left, right| {
        left.start_byte
            .cmp(&right.start_byte)
            .then_with(|| left.end_byte.cmp(&right.end_byte))
            .then_with(|| left.evidence_id.cmp(&right.evidence_id))
    });
    for declaration in declaration_evidence {
        evidence.push(RepositoryBoundaryEvidence::GitmodulesDeclaration {
            evidence_id: declaration.evidence_id.clone(),
            blob_oid: declaration.blob_oid.clone(),
            start_byte: declaration.start_byte,
            end_byte: declaration.end_byte,
        });
    }

    let mut gaps = Vec::new();
    let mut boundaries = Vec::new();
    for gitlink in &gitlinks {
        let boundary_id = boundary_id_by_path
            .get(gitlink.path.as_str())
            .cloned()
            .ok_or(RepositoryBoundaryError::InvalidReport)?;
        let tree_evidence = tree_evidence_by_path
            .get(gitlink.path.as_str())
            .cloned()
            .ok_or(RepositoryBoundaryError::InvalidReport)?;
        let declaration = declaration_by_path.get(gitlink.path.as_str()).copied();
        let nested = nested_by_path.get(gitlink.path.as_str()).copied();
        if let Some(nested) = nested
            && nested.commit_oid() != &gitlink.gitlink_oid
        {
            return Err(RepositoryBoundaryError::InvalidReport);
        }
        let mut evidence_ids = vec![tree_evidence.clone()];
        if let Some(declaration) = declaration {
            evidence_ids.push(declaration.evidence_id.clone());
        }
        let (state, mut gap_specs) = if nested.is_some() {
            (
                RepositoryBoundaryState::ExplicitlyBound,
                vec![(
                    "boundary.nested_repository_not_analyzed",
                    boundary_id.clone(),
                    vec![tree_evidence],
                )],
            )
        } else if declaration.is_some() {
            (
                RepositoryBoundaryState::DeclaredUnbound,
                vec![(
                    "boundary.nested_repository_unbound",
                    boundary_id.clone(),
                    evidence_ids.clone(),
                )],
            )
        } else {
            (
                RepositoryBoundaryState::UndeclaredUnbound,
                vec![
                    (
                        "boundary.gitmodules_declaration_missing",
                        boundary_id.clone(),
                        vec![tree_evidence.clone()],
                    ),
                    (
                        "boundary.nested_repository_unbound",
                        boundary_id.clone(),
                        vec![tree_evidence],
                    ),
                ],
            )
        };
        if declaration.is_none() && nested.is_some() {
            gap_specs.push((
                "boundary.gitmodules_declaration_missing",
                boundary_id.clone(),
                evidence_ids.clone(),
            ));
        }
        for (code, subject, gap_evidence) in gap_specs {
            gaps.push(gap_record(
                code,
                &gitlink.path,
                &subject,
                gap_evidence,
                hasher,
            ));
        }
        if declaration.is_some_and(|value| !value.unsupported_keys.is_empty()) {
            let declaration = declaration.ok_or(RepositoryBoundaryError::InvalidReport)?;
            gaps.push(gap_record(
                "boundary.gitmodules_key_unsupported",
                &gitlink.path,
                &declaration.declaration_id,
                vec![declaration.evidence_id.clone()],
                hasher,
            ));
        }
        boundaries.push(RepositoryBoundary {
            boundary_id,
            path: gitlink.path.clone(),
            gitlink_oid: gitlink.gitlink_oid.clone(),
            state,
            declaration_id: declaration.map(|value| value.declaration_id.clone()),
            nested_repository: nested.cloned(),
            evidence_ids,
            coverage_gap_ids: Vec::new(),
        });
    }

    let mut declarations = Vec::new();
    for declaration in parsed.declarations {
        let boundary_id = boundary_id_by_path.get(declaration.path.as_str()).cloned();
        if boundary_id.is_none() {
            gaps.push(gap_record(
                "boundary.gitmodules_declaration_orphan",
                &declaration.path,
                &declaration.declaration_id,
                vec![declaration.evidence_id.clone()],
                hasher,
            ));
        }
        if !declaration.unsupported_keys.is_empty() && boundary_id.is_none() {
            gaps.push(gap_record(
                "boundary.gitmodules_key_unsupported",
                &declaration.path,
                &declaration.declaration_id,
                vec![declaration.evidence_id.clone()],
                hasher,
            ));
        }
        declarations.push(RepositoryBoundaryDeclaration {
            declaration_id: declaration.declaration_id,
            name_sha256: declaration.name_sha256,
            path: declaration.path,
            url_kind: declaration.url_kind,
            url_sha256: declaration.url_sha256,
            unsupported_keys: declaration.unsupported_keys,
            boundary_id,
            evidence_id: declaration.evidence_id,
        });
    }
    gaps.sort_by(|left, right| {
        left.code
            .cmp(right.code)
            .then_with(|| left.path.as_bytes().cmp(right.path.as_bytes()))
            .then_with(|| left.subject_id.cmp(&right.subject_id))
    });
    for boundary in &mut boundaries {
        boundary.coverage_gap_ids = gaps
            .iter()
            .filter(|gap| {
                gap.path == boundary.path
                    && (gap.subject_id == boundary.boundary_id
                        || boundary
                            .declaration_id
                            .as_ref()
                            .is_some_and(|id| id == &gap.subject_id))
            })
            .map(|gap| gap.gap_id.clone())
            .collect();
    }
    if boundaries
        .iter()
        .any(|boundary| boundary.coverage_gap_ids.is_empty())
    {
        return Err(RepositoryBoundaryError::InvalidReport);
    }
    let report = RepositoryBoundaryReport {
        root_repository: root.clone(),
        boundaries,
        declarations,
        coverage_gaps: gaps,
        evidence,
    };
    validate_boundary_report(&report)?;
    Ok(report)
}

fn validate_boundary_report(
    report: &RepositoryBoundaryReport,
) -> Result<(), RepositoryBoundaryError> {
    let boundary_ids = report
        .boundaries
        .iter()
        .map(|boundary| boundary.boundary_id.as_str())
        .collect::<BTreeSet<_>>();
    let declaration_ids = report
        .declarations
        .iter()
        .map(|declaration| declaration.declaration_id.as_str())
        .collect::<BTreeSet<_>>();
    let gap_ids = report
        .coverage_gaps
        .iter()
        .map(|gap| gap.gap_id.as_str())
        .collect::<BTreeSet<_>>();
    let evidence_ids = report
        .evidence
        .iter()
        .map(boundary_evidence_id)
        .collect::<BTreeSet<_>>();
    if boundary_ids.len() != report.boundaries.len()
        || declaration_ids.len() != report.declarations.len()
        || gap_ids.len() != report.coverage_gaps.len()
        || evidence_ids.len() != report.evidence.len()
    {
        return Err(RepositoryBoundaryError::InvalidReport);
    }

    let boundary_by_id = report
        .boundaries
        .iter()
        .map(|boundary| (boundary.boundary_id.as_str(), boundary))
        .collect::<BTreeMap<_, _>>();
    for boundary in &report.boundaries {
        if boundary.evidence_ids.is_empty()
            || boundary.coverage_gap_ids.is_empty()
            || !all_unique(boundary.evidence_ids.iter().map(String::as_str))
            || !all_unique(boundary.coverage_gap_ids.iter().map(String::as_str))
            || boundary
                .evidence_ids
                .iter()
                .any(|identifier| !evidence_ids.contains(identifier.as_str()))
            || boundary
                .coverage_gap_ids
                .iter()
                .any(|identifier| !gap_ids.contains(identifier.as_str()))
            || boundary
                .declaration_id
                .as_ref()
                .is_some_and(|identifier| !declaration_ids.contains(identifier.as_str()))
        {
            return Err(RepositoryBoundaryError::InvalidReport);
        }
    }
    for declaration in &report.declarations {
        if !evidence_ids.contains(declaration.evidence_id.as_str())
            || declaration
                .unsupported_keys
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || declaration.boundary_id.as_ref().is_some_and(|identifier| {
                boundary_by_id
                    .get(identifier.as_str())
                    .is_none_or(|boundary| {
                        boundary.declaration_id.as_deref()
                            != Some(declaration.declaration_id.as_str())
                    })
            })
        {
            return Err(RepositoryBoundaryError::InvalidReport);
        }
    }
    for gap in &report.coverage_gaps {
        if gap.evidence_ids.is_empty()
            || !all_unique(gap.evidence_ids.iter().map(String::as_str))
            || gap
                .evidence_ids
                .iter()
                .any(|identifier| !evidence_ids.contains(identifier.as_str()))
        {
            return Err(RepositoryBoundaryError::InvalidReport);
        }
    }
    Ok(())
}

fn boundary_evidence_id(evidence: &RepositoryBoundaryEvidence) -> &str {
    match evidence {
        RepositoryBoundaryEvidence::GitTreeEntry { evidence_id, .. }
        | RepositoryBoundaryEvidence::GitmodulesDeclaration { evidence_id, .. } => evidence_id,
    }
}

fn all_unique<'a>(values: impl IntoIterator<Item = &'a str>) -> bool {
    let mut observed = BTreeSet::new();
    values.into_iter().all(|value| observed.insert(value))
}

/// Validates one canonical relative path shared by metadata and manifest input.
///
/// # Errors
///
/// Returns `PathInvalid` for absolute, escaping, ambiguous, or over-limit input.
pub fn validate_canonical_relative_path(path: &str) -> Result<(), BoundaryMetadataReason> {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path
            .as_bytes()
            .iter()
            .any(|byte| byte.is_ascii_control() || *byte == 0x7f)
        || path.len() > 1_024
    {
        return Err(BoundaryMetadataReason::PathInvalid);
    }
    for component in path.split('/') {
        if component.is_empty() || matches!(component, "." | "..") || component.len() > 255 {
            return Err(BoundaryMetadataReason::PathInvalid);
        }
    }
    Ok(())
}

fn validate_gitmodules_bytes(bytes: &[u8]) -> Result<(), RepositoryBoundaryError> {
    let mut failure = std::str::from_utf8(bytes)
        .err()
        .map(|error| (error.valid_up_to(), BoundaryMetadataReason::InvalidEncoding));
    for (index, byte) in bytes.iter().copied().enumerate() {
        if byte == b'\r' && bytes.get(index + 1) != Some(&b'\n') {
            select_earlier_metadata_failure(
                &mut failure,
                index,
                BoundaryMetadataReason::BareCarriageReturn,
            );
        }
        if byte == 0x7f || byte < 0x20 && !matches!(byte, b'\t' | b'\n' | b'\r') {
            select_earlier_metadata_failure(
                &mut failure,
                index,
                BoundaryMetadataReason::NulOrControl,
            );
        }
    }
    failure.map_or(Ok(()), |(_, reason)| Err(metadata_error(reason)))
}

fn select_earlier_metadata_failure(
    selected: &mut Option<(usize, BoundaryMetadataReason)>,
    offset: usize,
    reason: BoundaryMetadataReason,
) {
    if selected
        .as_ref()
        .is_none_or(|(selected_offset, selected_reason)| {
            offset < *selected_offset
                || offset == *selected_offset
                    && metadata_reason_order(reason) < metadata_reason_order(*selected_reason)
        })
    {
        *selected = Some((offset, reason));
    }
}

const fn metadata_reason_order(reason: BoundaryMetadataReason) -> u8 {
    match reason {
        BoundaryMetadataReason::InvalidEncoding => 0,
        BoundaryMetadataReason::NulOrControl => 1,
        BoundaryMetadataReason::BareCarriageReturn => 2,
        BoundaryMetadataReason::MalformedSection => 3,
        BoundaryMetadataReason::InvalidName => 4,
        BoundaryMetadataReason::KeyOutsideSection => 5,
        BoundaryMetadataReason::DuplicateSection => 6,
        BoundaryMetadataReason::DuplicateKey => 7,
        BoundaryMetadataReason::RequiredKeyMissing => 8,
        BoundaryMetadataReason::PathInvalid => 9,
        BoundaryMetadataReason::AmbiguousMapping => 10,
        BoundaryMetadataReason::UnsafeEntryKind => 11,
    }
}

fn validate_required_gitmodules_keys(
    section: &PendingSection,
) -> Result<(), RepositoryBoundaryError> {
    if section.values.contains_key("path") && section.values.contains_key("url") {
        Ok(())
    } else {
        Err(metadata_error(BoundaryMetadataReason::RequiredKeyMissing))
    }
}

struct GitmodulesLine {
    start: usize,
    content_end: usize,
}

fn gitmodules_lines(bytes: &[u8]) -> Vec<GitmodulesLine> {
    let mut lines = Vec::new();
    let mut start = 0;
    while start < bytes.len() {
        let newline = bytes[start..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(bytes.len(), |offset| start + offset);
        let content_end = if newline > start && bytes[newline - 1] == b'\r' {
            newline - 1
        } else {
            newline
        };
        lines.push(GitmodulesLine { start, content_end });
        start = newline.saturating_add(1);
    }
    lines
}

fn trim_horizontal(mut value: &[u8]) -> &[u8] {
    while value
        .first()
        .is_some_and(|byte| matches!(byte, b' ' | b'\t'))
    {
        value = &value[1..];
    }
    while value
        .last()
        .is_some_and(|byte| matches!(byte, b' ' | b'\t'))
    {
        value = &value[..value.len() - 1];
    }
    value
}

fn parse_section_name(value: &[u8]) -> Result<&str, RepositoryBoundaryError> {
    const PREFIX: &[u8] = b"[submodule \"";
    if !value.starts_with(PREFIX) || !value.ends_with(b"\"]") {
        return Err(metadata_error(BoundaryMetadataReason::MalformedSection));
    }
    let name = &value[PREFIX.len()..value.len() - 2];
    if name.is_empty()
        || name.len() > 255
        || !name
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'/' | b'-'))
    {
        return Err(metadata_error(BoundaryMetadataReason::InvalidName));
    }
    std::str::from_utf8(name).map_err(|_| metadata_error(BoundaryMetadataReason::InvalidName))
}

fn valid_key(key: &[u8]) -> bool {
    let Some((&first, rest)) = key.split_first() else {
        return false;
    };
    key.len() <= 64
        && first.is_ascii_lowercase()
        && rest
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

fn unsupported_value_syntax(value: &[u8]) -> bool {
    value.contains(&b'"')
        || value.contains(&b'\\')
        || value.windows(2).any(|pair| pair == b"$(")
        || value.windows(2).any(|pair| pair == b"${")
        || value
            .windows(2)
            .any(|pair| matches!(pair[0], b' ' | b'\t') && matches!(pair[1], b'#' | b';'))
}

fn classify_url(value: &str) -> BoundaryUrlKind {
    if value.starts_with("./") || value.starts_with("../") {
        BoundaryUrlKind::Relative
    } else if value.starts_with('/') {
        BoundaryUrlKind::AbsolutePath
    } else if value.starts_with("file:") {
        BoundaryUrlKind::File
    } else if value.starts_with("ssh:") {
        BoundaryUrlKind::Ssh
    } else if value.starts_with("https:") {
        BoundaryUrlKind::Https
    } else if value.starts_with("http:") {
        BoundaryUrlKind::Http
    } else if value.starts_with("git:") {
        BoundaryUrlKind::Git
    } else if is_scp_like(value.as_bytes()) {
        BoundaryUrlKind::ScpLike
    } else {
        BoundaryUrlKind::Other
    }
}

fn is_scp_like(value: &[u8]) -> bool {
    let Some(at) = value.iter().position(|byte| *byte == b'@') else {
        return false;
    };
    let Some(colon_offset) = value[at + 1..].iter().position(|byte| *byte == b':') else {
        return false;
    };
    let colon = at + 1 + colon_offset;
    let user = &value[..at];
    let host = &value[at + 1..colon];
    let path = &value[colon + 1..];
    !user.is_empty()
        && !host.is_empty()
        && !path.is_empty()
        && user.iter().all(|byte| safe_scp_byte(*byte, b"/@:"))
        && host.iter().all(|byte| safe_scp_byte(*byte, b"/:"))
}

fn safe_scp_byte(byte: u8, forbidden: &[u8]) -> bool {
    !forbidden.contains(&byte) && byte > 0x20 && byte != 0x7f
}

fn metadata_error(reason: BoundaryMetadataReason) -> RepositoryBoundaryError {
    RepositoryBoundaryError::MetadataInvalid { reason, path: None }
}

fn safe_error_path(path: &str) -> Option<String> {
    validate_canonical_relative_path(path)
        .is_ok()
        .then(|| path.to_owned())
}

fn boundary_id(
    root: &BoundRevision,
    gitlink: &AcquiredGitlink,
    hasher: &impl BoundarySha256,
) -> String {
    prefixed_domain_id(
        "urn:codenoesis:repository-boundary:sha256:",
        "codenoesis.repository-boundary/v1",
        &[
            root.repository_identity().as_str(),
            root.commit_oid().as_str(),
            &gitlink.path,
            gitlink.gitlink_oid.as_str(),
        ],
        hasher,
    )
}

fn gap_record(
    code: &'static str,
    path: &str,
    subject_id: &str,
    evidence_ids: Vec<String>,
    hasher: &impl BoundarySha256,
) -> RepositoryBoundaryGap {
    RepositoryBoundaryGap {
        gap_id: prefixed_domain_id(
            "urn:codenoesis:boundary-gap:sha256:",
            "codenoesis.boundary-gap/v1",
            &[code, subject_id],
            hasher,
        ),
        code,
        path: path.to_owned(),
        subject_id: subject_id.to_owned(),
        evidence_ids,
    }
}

fn prefixed_domain_id(
    prefix: &str,
    domain: &str,
    fields: &[&str],
    hasher: &impl BoundarySha256,
) -> String {
    format!("{prefix}{}", domain_sha256(domain, fields, hasher))
}

fn domain_sha256(domain: &str, fields: &[&str], hasher: &impl BoundarySha256) -> String {
    let required = domain.len()
        + 1
        + fields.iter().map(|field| field.len()).sum::<usize>()
        + fields.len().saturating_sub(1);
    let mut input = Vec::with_capacity(required);
    input.extend_from_slice(domain.as_bytes());
    input.push(0);
    for (index, field) in fields.iter().enumerate() {
        if index > 0 {
            input.push(0);
        }
        input.extend_from_slice(field.as_bytes());
    }
    hex_digest(&hasher.digest(&input))
}

fn sha256_hex(value: &[u8], hasher: &impl BoundarySha256) -> String {
    hex_digest(&hasher.digest(value))
}

fn hex_digest(bytes: &[u8]) -> String {
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(value, "{byte:02x}").expect("writing to String cannot fail");
    }
    value
}
