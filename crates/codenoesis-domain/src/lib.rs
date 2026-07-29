//! Domain values for the `CodeNoesis` S0 through S4 slices.

pub mod knowledge;
pub mod s4;
pub mod s5;
pub mod storage;

use std::error::Error;
use std::fmt::{self, Display, Formatter};

const REPOSITORY_ID_PREFIX: &str = "urn:codenoesis:";

pub const STANDARD_LOCAL_S1_LIMITS: StandardLocalS1Limits = StandardLocalS1Limits {
    regular_files: 20_000,
    tree_entries: 25_000,
    cumulative_file_bytes: 268_435_456,
    single_file_bytes: 4_194_304,
    path_bytes: 1_024,
    path_component_bytes: 255,
    recursion_depth: 32,
    canonical_output_bytes: 33_554_432,
    scan_wall_milliseconds: 60_000,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StandardLocalS1Limits {
    pub regular_files: u64,
    pub tree_entries: u64,
    pub cumulative_file_bytes: u64,
    pub single_file_bytes: u64,
    pub path_bytes: u64,
    pub path_component_bytes: u64,
    pub recursion_depth: u64,
    pub canonical_output_bytes: u64,
    pub scan_wall_milliseconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryIdentity(String);

impl RepositoryIdentity {
    /// Parses the canonical S0 logical repository identity.
    ///
    /// # Errors
    ///
    /// Returns [`InputError::InvalidRepositoryIdentity`] when the value is not
    /// in the approved S0 identity subset.
    pub fn parse(value: &str) -> Result<Self, InputError> {
        let Some(suffix) = value.strip_prefix(REPOSITORY_ID_PREFIX) else {
            return Err(InputError::InvalidRepositoryIdentity);
        };
        let mut bytes = suffix.bytes();
        let Some(first) = bytes.next() else {
            return Err(InputError::InvalidRepositoryIdentity);
        };
        if suffix.len() > 255
            || !first.is_ascii_lowercase() && !first.is_ascii_digit()
            || !bytes.all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'_' | b':' | b'-')
            })
        {
            return Err(InputError::InvalidRepositoryIdentity);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ObjectId(String);

impl ObjectId {
    #[must_use]
    pub fn parse_sha1(value: &str) -> Option<Self> {
        (value.len() == 40
            && value
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()))
        .then(|| Self(value.to_owned()))
    }

    #[must_use]
    pub fn from_bytes(bytes: &[u8; 20]) -> Self {
        let mut value = String::with_capacity(40);
        for byte in bytes {
            use std::fmt::Write as _;
            write!(value, "{byte:02x}").expect("writing to String cannot fail");
        }
        Self(value)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for ObjectId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Revision {
    Commit(ObjectId),
    Main,
}

impl Revision {
    /// Parses a full lowercase SHA-1 OID or the literal S0 main ref.
    ///
    /// # Errors
    ///
    /// Returns [`InputError::InvalidRevision`] for every unsupported spelling.
    pub fn parse(value: &str) -> Result<Self, InputError> {
        if value == "refs/heads/main" {
            return Ok(Self::Main);
        }
        ObjectId::parse_sha1(value)
            .map(Self::Commit)
            .ok_or(InputError::InvalidRevision)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Commit(object_id) => object_id.as_str(),
            Self::Main => "refs/heads/main",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectKind {
    Commit,
    Tree,
    Blob,
}

impl ObjectKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Commit => "commit",
            Self::Tree => "tree",
            Self::Blob => "blob",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActualObjectKind {
    Tag,
    Tree,
    Blob,
}

impl ActualObjectKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tag => "tag",
            Self::Tree => "tree",
            Self::Blob => "blob",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnsupportedFeature {
    BareRepository,
    ShallowRepository,
    Sha256ObjectFormat,
    AlternateObjectDatabase,
    ReplaceOrGraft,
    NonSingleRegularRootFile,
    LfsMaterialization,
    PackedObjectDatabase,
    SubmoduleOrGitlink,
    Symlink,
}

impl UnsupportedFeature {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BareRepository => "bare_repository",
            Self::ShallowRepository => "shallow_repository",
            Self::Sha256ObjectFormat => "sha256_object_format",
            Self::AlternateObjectDatabase => "alternate_object_database",
            Self::ReplaceOrGraft => "replace_or_graft",
            Self::NonSingleRegularRootFile => "non_single_regular_root_file",
            Self::LfsMaterialization => "lfs_materialization",
            Self::PackedObjectDatabase => "packed_object_database",
            Self::SubmoduleOrGitlink => "submodule_or_gitlink",
            Self::Symlink => "symlink",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundRevision {
    repository_identity: RepositoryIdentity,
    commit_oid: ObjectId,
    tree_oid: ObjectId,
}

impl BoundRevision {
    #[must_use]
    pub const fn new(
        repository_identity: RepositoryIdentity,
        commit_oid: ObjectId,
        tree_oid: ObjectId,
    ) -> Self {
        Self {
            repository_identity,
            commit_oid,
            tree_oid,
        }
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
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RegularFileMode {
    Regular,
    Executable,
}

impl RegularFileMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Regular => "100644",
            Self::Executable => "100755",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcquiredFile {
    path: String,
    mode: RegularFileMode,
    blob_oid: ObjectId,
    bytes: Vec<u8>,
}

impl AcquiredFile {
    #[must_use]
    pub const fn new(
        path: String,
        mode: RegularFileMode,
        blob_oid: ObjectId,
        bytes: Vec<u8>,
    ) -> Self {
        Self {
            path,
            mode,
            blob_oid,
            bytes,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcquiredRepository {
    bound_revision: BoundRevision,
    directory_count: u64,
    files: Vec<AcquiredFile>,
}

impl AcquiredRepository {
    #[must_use]
    pub const fn new(
        bound_revision: BoundRevision,
        directory_count: u64,
        files: Vec<AcquiredFile>,
    ) -> Self {
        Self {
            bound_revision,
            directory_count,
            files,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ContentKind {
    TextUtf8,
    BinaryOrUnknown,
}

impl ContentKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TextUtf8 => "text_utf8",
            Self::BinaryOrUnknown => "binary_or_unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum InventoryRole {
    Configuration,
    Contract,
    Documentation,
    Manifest,
    Ownership,
    Sentinel,
    Source,
    Unsupported,
}

impl InventoryRole {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Configuration => "configuration",
            Self::Contract => "contract",
            Self::Documentation => "documentation",
            Self::Manifest => "manifest",
            Self::Ownership => "ownership",
            Self::Sentinel => "sentinel",
            Self::Source => "source",
            Self::Unsupported => "unsupported",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum InventoryLanguage {
    Rust,
    Shell,
}

impl InventoryLanguage {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Shell => "shell",
        }
    }

    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Rust => "Rust",
            Self::Shell => "Shell",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RecognizedInventoryKind {
    CargoManifest,
    GitHubCodeowners,
    OpenApiContract,
    ReadmeDocumentation,
    RustfmtConfiguration,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ClassificationRule {
    CargoManifest,
    GitHubCodeowners,
    GitTreeEntry,
    OpenApiContract,
    ReadmeDocumentation,
    RustLanguage,
    RustfmtConfiguration,
    BuildScriptSentinel,
    ExecutableScriptSentinel,
    ShellLanguage,
    UnsupportedFallback,
}

impl ClassificationRule {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CargoManifest => "manifest:cargo",
            Self::GitHubCodeowners => "ownership:github-codeowners",
            Self::GitTreeEntry => "git-tree-entry",
            Self::OpenApiContract => "contract:openapi",
            Self::ReadmeDocumentation => "documentation:readme",
            Self::RustLanguage => "language:rust",
            Self::RustfmtConfiguration => "configuration:rustfmt",
            Self::BuildScriptSentinel => "sentinel:build-script",
            Self::ExecutableScriptSentinel => "sentinel:executable-script",
            Self::ShellLanguage => "language:shell",
            Self::UnsupportedFallback => "unsupported:fallback",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InventoryFile {
    evidence_id: String,
    path: String,
    mode: RegularFileMode,
    blob_oid: ObjectId,
    byte_length: u64,
    content_kind: ContentKind,
    roles: Vec<InventoryRole>,
    languages: Vec<InventoryLanguage>,
    rules: Vec<ClassificationRule>,
    recognized_kind: Option<RecognizedInventoryKind>,
    sentinel: bool,
    unsupported: bool,
    bytes: Vec<u8>,
}

impl InventoryFile {
    #[must_use]
    pub fn evidence_id(&self) -> &str {
        &self.evidence_id
    }

    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[must_use]
    pub const fn mode(&self) -> RegularFileMode {
        self.mode
    }

    #[must_use]
    pub const fn blob_oid(&self) -> &ObjectId {
        &self.blob_oid
    }

    #[must_use]
    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }

    #[must_use]
    pub const fn content_kind(&self) -> ContentKind {
        self.content_kind
    }

    #[must_use]
    pub fn roles(&self) -> &[InventoryRole] {
        &self.roles
    }

    #[must_use]
    pub fn languages(&self) -> &[InventoryLanguage] {
        &self.languages
    }

    #[must_use]
    pub fn rules(&self) -> &[ClassificationRule] {
        &self.rules
    }

    #[must_use]
    pub const fn recognized_kind(&self) -> Option<RecognizedInventoryKind> {
        self.recognized_kind
    }

    #[must_use]
    pub const fn is_sentinel(&self) -> bool {
        self.sentinel
    }

    #[must_use]
    pub const fn is_unsupported(&self) -> bool {
        self.unsupported
    }

    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryInventory {
    bound_revision: BoundRevision,
    directory_count: u64,
    files: Vec<InventoryFile>,
}

impl RepositoryInventory {
    #[must_use]
    pub fn classify(mut acquired: AcquiredRepository) -> Self {
        acquired
            .files
            .sort_by(|left, right| left.path.as_bytes().cmp(right.path.as_bytes()));
        let files = acquired
            .files
            .into_iter()
            .enumerate()
            .map(|(index, file)| classify_file(index, file))
            .collect();
        Self {
            bound_revision: acquired.bound_revision,
            directory_count: acquired.directory_count,
            files,
        }
    }

    #[must_use]
    pub const fn bound_revision(&self) -> &BoundRevision {
        &self.bound_revision
    }

    #[must_use]
    pub const fn directory_count(&self) -> u64 {
        self.directory_count
    }

    #[must_use]
    pub fn files(&self) -> &[InventoryFile] {
        &self.files
    }
}

fn classify_file(index: usize, file: AcquiredFile) -> InventoryFile {
    let basename = file.path.rsplit('/').next().unwrap_or(&file.path);
    let mut roles = Vec::new();
    let mut languages = Vec::new();
    let mut derivation_rules = vec![ClassificationRule::GitTreeEntry];
    let mut recognized_kind = None;
    let mut sentinel = false;

    if basename == "build.rs" || has_suffix(&file.path, b".rs") {
        roles.push(InventoryRole::Source);
        languages.push(InventoryLanguage::Rust);
        derivation_rules.push(ClassificationRule::RustLanguage);
    }
    if has_suffix(&file.path, b".sh") {
        roles.push(InventoryRole::Source);
        languages.push(InventoryLanguage::Shell);
        derivation_rules.push(ClassificationRule::ShellLanguage);
    }
    if basename == "Cargo.toml" {
        roles.push(InventoryRole::Manifest);
        derivation_rules.push(ClassificationRule::CargoManifest);
        recognized_kind = Some(RecognizedInventoryKind::CargoManifest);
    } else if matches!(basename, "openapi.yaml" | "openapi.yml")
        && file.bytes.starts_with(b"openapi:")
    {
        roles.push(InventoryRole::Contract);
        derivation_rules.push(ClassificationRule::OpenApiContract);
        recognized_kind = Some(RecognizedInventoryKind::OpenApiContract);
    } else if basename == "rustfmt.toml" {
        roles.push(InventoryRole::Configuration);
        derivation_rules.push(ClassificationRule::RustfmtConfiguration);
        recognized_kind = Some(RecognizedInventoryKind::RustfmtConfiguration);
    } else if file.path == ".github/CODEOWNERS" {
        roles.push(InventoryRole::Ownership);
        derivation_rules.push(ClassificationRule::GitHubCodeowners);
        recognized_kind = Some(RecognizedInventoryKind::GitHubCodeowners);
    } else if basename == "README.md" {
        roles.push(InventoryRole::Documentation);
        derivation_rules.push(ClassificationRule::ReadmeDocumentation);
        recognized_kind = Some(RecognizedInventoryKind::ReadmeDocumentation);
    }
    if file.path == "build.rs" {
        roles.push(InventoryRole::Sentinel);
        derivation_rules.push(ClassificationRule::BuildScriptSentinel);
        sentinel = true;
    }
    if file.mode == RegularFileMode::Executable && has_suffix(&file.path, b".sh") {
        roles.push(InventoryRole::Sentinel);
        derivation_rules.push(ClassificationRule::ExecutableScriptSentinel);
        sentinel = true;
    }

    let unsupported = roles.is_empty();
    if unsupported {
        roles.push(InventoryRole::Unsupported);
        derivation_rules.push(ClassificationRule::UnsupportedFallback);
    }
    roles.sort_unstable();
    roles.dedup();
    languages.sort_unstable();
    languages.dedup();
    derivation_rules.sort_by_key(|rule| rule.as_str());
    derivation_rules.dedup();
    let content_kind = if !unsupported && std::str::from_utf8(&file.bytes).is_ok() {
        ContentKind::TextUtf8
    } else {
        ContentKind::BinaryOrUnknown
    };

    InventoryFile {
        evidence_id: format!("evidence-{:05}", index + 1),
        path: file.path,
        mode: file.mode,
        blob_oid: file.blob_oid,
        byte_length: u64::try_from(file.bytes.len()).expect("bounded S1 blob length fits u64"),
        content_kind,
        roles,
        languages,
        rules: derivation_rules,
        recognized_kind,
        sentinel,
        unsupported,
        bytes: file.bytes,
    }
}

fn has_suffix(path: &str, suffix: &[u8]) -> bool {
    path.as_bytes().ends_with(suffix)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputError {
    InvalidRepositoryIdentity,
    InvalidRevision,
    InvalidProfile,
    InvalidStoreRoot,
}

impl Display for InputError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidRepositoryIdentity => "invalid repository identity",
            Self::InvalidRevision => "invalid revision",
            Self::InvalidProfile => "invalid profile",
            Self::InvalidStoreRoot => "invalid store root",
        })
    }
}

impl Error for InputError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcquisitionError {
    NotGitRepository,
    RevisionNotFound {
        revision: Revision,
    },
    RevisionNotCommit {
        object_oid: ObjectId,
        actual_kind: ActualObjectKind,
    },
    ObjectMissing {
        object_oid: ObjectId,
        expected_kind: ObjectKind,
        referenced_by: ObjectId,
    },
    RepositoryInconsistent {
        object_oid: ObjectId,
        expected_kind: ObjectKind,
    },
    UnsupportedRepositoryShape {
        feature: UnsupportedFeature,
    },
    PathInvalid {
        reason: PathInvalidReason,
    },
    RootPolicyViolation {
        policy: RootPolicy,
    },
    EntryPolicyViolation {
        path: String,
        entry: EntryPolicy,
    },
    LimitExceeded {
        limit: LimitKind,
        maximum: u64,
        observed: u64,
    },
}

impl Display for AcquisitionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NotGitRepository => "not a supported Git worktree",
            Self::RevisionNotFound { .. } => "revision not found",
            Self::RevisionNotCommit { .. } => "revision does not name a commit",
            Self::ObjectMissing { .. } => "referenced Git object is missing",
            Self::RepositoryInconsistent { .. } => "Git repository is inconsistent",
            Self::UnsupportedRepositoryShape { .. } => "unsupported Git repository shape",
            Self::PathInvalid { .. } => "repository path is invalid for standard-local-s1",
            Self::RootPolicyViolation { .. } => {
                "repository root violates the standard-local-s1 policy"
            }
            Self::EntryPolicyViolation { .. } => {
                "repository entry violates the standard-local-s1 policy"
            }
            Self::LimitExceeded { .. } => "repository exceeds a standard-local-s1 limit",
        })
    }
}

impl Error for AcquisitionError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PathInvalidReason {
    NonUtf8,
    EmptyComponent,
    DotComponent,
    Backslash,
    ControlCharacter,
}

impl PathInvalidReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NonUtf8 => "non_utf8",
            Self::EmptyComponent => "empty_component",
            Self::DotComponent => "dot_component",
            Self::Backslash => "backslash",
            Self::ControlCharacter => "control_character",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RootPolicy {
    RepositoryRootIsSymlink,
    GitDirectoryNotContained,
    GitDirectoryIsSymlink,
}

impl RootPolicy {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RepositoryRootIsSymlink => "repository_root_is_symlink",
            Self::GitDirectoryNotContained => "git_directory_not_contained",
            Self::GitDirectoryIsSymlink => "git_directory_is_symlink",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntryPolicy {
    Symlink,
    Gitlink,
    SpecialFileMode,
}

impl EntryPolicy {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Symlink => "symlink",
            Self::Gitlink => "gitlink",
            Self::SpecialFileMode => "special_file_mode",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LimitKind {
    RegularFiles,
    TreeEntries,
    CumulativeFileBytes,
    SingleFileBytes,
    PathBytes,
    PathComponentBytes,
    RecursionDepth,
    CanonicalOutputBytes,
    ScanWallMilliseconds,
}

impl LimitKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RegularFiles => "regular_files",
            Self::TreeEntries => "tree_entries",
            Self::CumulativeFileBytes => "cumulative_file_bytes",
            Self::SingleFileBytes => "single_file_bytes",
            Self::PathBytes => "path_bytes",
            Self::PathComponentBytes => "path_component_bytes",
            Self::RecursionDepth => "recursion_depth",
            Self::CanonicalOutputBytes => "canonical_output_bytes",
            Self::ScanWallMilliseconds => "scan_wall_milliseconds",
        }
    }

    #[must_use]
    pub const fn maximum(self) -> u64 {
        match self {
            Self::RegularFiles => STANDARD_LOCAL_S1_LIMITS.regular_files,
            Self::TreeEntries => STANDARD_LOCAL_S1_LIMITS.tree_entries,
            Self::CumulativeFileBytes => STANDARD_LOCAL_S1_LIMITS.cumulative_file_bytes,
            Self::SingleFileBytes => STANDARD_LOCAL_S1_LIMITS.single_file_bytes,
            Self::PathBytes => STANDARD_LOCAL_S1_LIMITS.path_bytes,
            Self::PathComponentBytes => STANDARD_LOCAL_S1_LIMITS.path_component_bytes,
            Self::RecursionDepth => STANDARD_LOCAL_S1_LIMITS.recursion_depth,
            Self::CanonicalOutputBytes => STANDARD_LOCAL_S1_LIMITS.canonical_output_bytes,
            Self::ScanWallMilliseconds => STANDARD_LOCAL_S1_LIMITS.scan_wall_milliseconds,
        }
    }
}

#[must_use]
pub const fn limit_exceeded(limit: LimitKind, observed: u64) -> AcquisitionError {
    let maximum = limit.maximum();
    AcquisitionError::LimitExceeded {
        limit,
        maximum,
        observed: if observed > maximum + 1 {
            maximum + 1
        } else {
            observed
        },
    }
}

/// Checks one observed S1 resource value against its fixed public maximum.
///
/// # Errors
///
/// Returns a capped [`AcquisitionError::LimitExceeded`] for `maximum + 1`.
pub fn check_limit(limit: LimitKind, observed: u64) -> Result<(), AcquisitionError> {
    if observed > limit.maximum() {
        Err(limit_exceeded(limit, observed))
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RepositoryError {
    Acquisition(AcquisitionError),
    Unexpected,
}

impl From<AcquisitionError> for RepositoryError {
    fn from(error: AcquisitionError) -> Self {
        Self::Acquisition(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AcquiredFile, AcquiredRepository, AcquisitionError, BoundRevision, LimitKind, ObjectId,
        RegularFileMode, RepositoryIdentity, RepositoryInventory, check_limit,
    };

    #[test]
    fn pt_fr_acq_002_limits_have_max_and_plus_one() {
        let limits = [
            LimitKind::RegularFiles,
            LimitKind::TreeEntries,
            LimitKind::CumulativeFileBytes,
            LimitKind::SingleFileBytes,
            LimitKind::PathBytes,
            LimitKind::PathComponentBytes,
            LimitKind::RecursionDepth,
            LimitKind::CanonicalOutputBytes,
            LimitKind::ScanWallMilliseconds,
        ];
        for limit in limits {
            let maximum = limit.maximum();
            assert_eq!(check_limit(limit, maximum), Ok(()));
            assert_eq!(
                check_limit(limit, maximum + 1),
                Err(AcquisitionError::LimitExceeded {
                    limit,
                    maximum,
                    observed: maximum + 1
                })
            );
            assert_eq!(
                check_limit(limit, u64::MAX),
                Err(AcquisitionError::LimitExceeded {
                    limit,
                    maximum,
                    observed: maximum + 1
                })
            );
        }
    }

    #[test]
    fn pt_fr_inv_001_inventory_is_order_invariant() {
        let files = vec![
            acquired_file(
                "tools/sentinel.sh",
                RegularFileMode::Executable,
                "1111111111111111111111111111111111111111",
                b"#!/bin/sh\n",
            ),
            acquired_file(
                "Cargo.toml",
                RegularFileMode::Regular,
                "2222222222222222222222222222222222222222",
                b"[package]\n",
            ),
            acquired_file(
                "src/lib.rs",
                RegularFileMode::Regular,
                "3333333333333333333333333333333333333333",
                b"pub fn value() {}\n",
            ),
        ];
        let expected = classify(files.clone());
        for seed in 0..50 {
            let mut permutation = files.clone();
            let length = permutation.len();
            permutation.rotate_left(seed % length);
            if seed % 2 == 1 {
                permutation.reverse();
            }
            assert_eq!(classify(permutation), expected, "seed {seed}");
        }
    }

    fn classify(files: Vec<AcquiredFile>) -> RepositoryInventory {
        let identity = RepositoryIdentity::parse("urn:codenoesis:fixture:domain-order")
            .expect("valid fixture identity");
        let commit =
            ObjectId::parse_sha1("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").expect("commit OID");
        let tree =
            ObjectId::parse_sha1("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").expect("tree OID");
        RepositoryInventory::classify(AcquiredRepository::new(
            BoundRevision::new(identity, commit, tree),
            2,
            files,
        ))
    }

    fn acquired_file(
        path: &str,
        mode: RegularFileMode,
        object_id: &str,
        bytes: &[u8],
    ) -> AcquiredFile {
        AcquiredFile::new(
            path.to_owned(),
            mode,
            ObjectId::parse_sha1(object_id).expect("fixture blob OID"),
            bytes.to_vec(),
        )
    }
}
