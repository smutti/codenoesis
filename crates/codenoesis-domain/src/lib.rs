//! Domain values for the `CodeNoesis` S0 acquisition slice.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

const REPOSITORY_ID_PREFIX: &str = "urn:codenoesis:";

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputError {
    InvalidRepositoryIdentity,
    InvalidRevision,
}

impl Display for InputError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidRepositoryIdentity => "invalid repository identity",
            Self::InvalidRevision => "invalid revision",
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
        })
    }
}

impl Error for AcquisitionError {}

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
