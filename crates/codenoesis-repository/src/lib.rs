//! In-process local Git adapter for the approved S0 repository subset.

use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use codenoesis_domain::{
    AcquisitionError, ActualObjectKind, BoundRevision, ObjectId, ObjectKind, RepositoryError,
    RepositoryIdentity, Revision, UnsupportedFeature,
};
use codenoesis_ports::RepositoryAcquirer;
use flate2::read::ZlibDecoder;
use sha1::{Digest, Sha1};

#[derive(Default)]
pub struct LocalGitRepository;

impl LocalGitRepository {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl RepositoryAcquirer for LocalGitRepository {
    fn bind(
        &self,
        repository: &OsStr,
        identity: RepositoryIdentity,
        revision: Revision,
    ) -> Result<BoundRevision, RepositoryError> {
        let root = Path::new(repository);
        let git_dir = root.join(".git");
        if !root.is_dir() || !git_dir.is_dir() {
            if root.join("objects").is_dir() && root.join("HEAD").is_file() {
                return Err(AcquisitionError::UnsupportedRepositoryShape {
                    feature: UnsupportedFeature::BareRepository,
                }
                .into());
            }
            return Err(AcquisitionError::NotGitRepository.into());
        }
        validate_repository_shape(&git_dir)?;

        let commit_oid = resolve_revision(&git_dir, &revision)?;
        let commit = required_revision_object(&git_dir, &commit_oid, &revision)?;
        if commit.kind != GitObjectKind::Commit {
            let actual_kind = match commit.kind {
                GitObjectKind::Tag => ActualObjectKind::Tag,
                GitObjectKind::Tree => ActualObjectKind::Tree,
                GitObjectKind::Blob => ActualObjectKind::Blob,
                GitObjectKind::Commit => unreachable!(),
            };
            return Err(AcquisitionError::RevisionNotCommit {
                object_oid: commit_oid,
                actual_kind,
            }
            .into());
        }
        let tree_oid = parse_commit_tree(&commit.body_prefix).ok_or_else(|| {
            RepositoryError::from(AcquisitionError::RepositoryInconsistent {
                object_oid: commit_oid.clone(),
                expected_kind: ObjectKind::Commit,
            })
        })?;
        let tree = required_referenced_object(&git_dir, &tree_oid, ObjectKind::Tree, &commit_oid)?;
        if tree.kind != GitObjectKind::Tree {
            return Err(AcquisitionError::RepositoryInconsistent {
                object_oid: tree_oid,
                expected_kind: ObjectKind::Tree,
            }
            .into());
        }
        let blob_oid = parse_single_regular_file(&tree)?;
        let blob = required_referenced_object(&git_dir, &blob_oid, ObjectKind::Blob, &tree_oid)?;
        if blob.kind != GitObjectKind::Blob {
            return Err(AcquisitionError::RepositoryInconsistent {
                object_oid: blob_oid,
                expected_kind: ObjectKind::Blob,
            }
            .into());
        }
        if blob
            .body_prefix
            .starts_with(b"version https://git-lfs.github.com/spec/v1\n")
        {
            return Err(AcquisitionError::UnsupportedRepositoryShape {
                feature: UnsupportedFeature::LfsMaterialization,
            }
            .into());
        }

        Ok(BoundRevision::new(identity, commit_oid, tree_oid))
    }
}

fn validate_repository_shape(git_dir: &Path) -> Result<(), RepositoryError> {
    let config =
        fs::read_to_string(git_dir.join("config")).map_err(|_| RepositoryError::Unexpected)?;
    let normalized = config.to_ascii_lowercase();
    if normalized.lines().any(|line| {
        let compact = line.split_whitespace().collect::<String>();
        compact == "bare=true"
    }) {
        return Err(AcquisitionError::UnsupportedRepositoryShape {
            feature: UnsupportedFeature::BareRepository,
        }
        .into());
    }
    if normalized.lines().any(|line| {
        let compact = line.split_whitespace().collect::<String>();
        compact == "objectformat=sha256"
    }) {
        return Err(AcquisitionError::UnsupportedRepositoryShape {
            feature: UnsupportedFeature::Sha256ObjectFormat,
        }
        .into());
    }
    if git_dir.join("shallow").exists() {
        return Err(AcquisitionError::UnsupportedRepositoryShape {
            feature: UnsupportedFeature::ShallowRepository,
        }
        .into());
    }
    let alternates = git_dir.join("objects/info/alternates");
    if alternates.exists() {
        let bytes = fs::read(alternates).map_err(|_| RepositoryError::Unexpected)?;
        if bytes.iter().any(|byte| !byte.is_ascii_whitespace()) {
            return Err(AcquisitionError::UnsupportedRepositoryShape {
                feature: UnsupportedFeature::AlternateObjectDatabase,
            }
            .into());
        }
    }
    if git_dir.join("info/grafts").exists() || git_dir.join("refs/replace").exists() {
        return Err(AcquisitionError::UnsupportedRepositoryShape {
            feature: UnsupportedFeature::ReplaceOrGraft,
        }
        .into());
    }
    Ok(())
}

fn resolve_revision(git_dir: &Path, revision: &Revision) -> Result<ObjectId, RepositoryError> {
    match revision {
        Revision::Commit(object_id) => Ok(object_id.clone()),
        Revision::Main => resolve_main_ref(git_dir).ok_or_else(|| {
            RepositoryError::from(AcquisitionError::RevisionNotFound {
                revision: revision.clone(),
            })
        }),
    }
}

fn resolve_main_ref(git_dir: &Path) -> Option<ObjectId> {
    let loose = git_dir.join("refs/heads/main");
    if let Ok(value) = fs::read_to_string(loose) {
        return ObjectId::parse_sha1(value.trim_end_matches(['\r', '\n']));
    }
    let packed = fs::read_to_string(git_dir.join("packed-refs")).ok()?;
    packed.lines().find_map(|line| {
        let (object_id, name) = line.split_once(' ')?;
        (name == "refs/heads/main")
            .then(|| ObjectId::parse_sha1(object_id))
            .flatten()
    })
}

fn required_revision_object(
    git_dir: &Path,
    object_id: &ObjectId,
    revision: &Revision,
) -> Result<GitObject, RepositoryError> {
    match read_object(git_dir, object_id) {
        Ok(Some(object)) => Ok(object),
        Ok(None) => Err(AcquisitionError::RevisionNotFound {
            revision: revision.clone(),
        }
        .into()),
        Err(ReadObjectError::Invalid) => Err(AcquisitionError::RepositoryInconsistent {
            object_oid: object_id.clone(),
            expected_kind: ObjectKind::Commit,
        }
        .into()),
        Err(ReadObjectError::Io) => Err(RepositoryError::Unexpected),
    }
}

fn required_referenced_object(
    git_dir: &Path,
    object_id: &ObjectId,
    expected_kind: ObjectKind,
    referenced_by: &ObjectId,
) -> Result<GitObject, RepositoryError> {
    match read_object(git_dir, object_id) {
        Ok(Some(object)) => Ok(object),
        Ok(None) => Err(AcquisitionError::ObjectMissing {
            object_oid: object_id.clone(),
            expected_kind,
            referenced_by: referenced_by.clone(),
        }
        .into()),
        Err(ReadObjectError::Invalid) => Err(AcquisitionError::RepositoryInconsistent {
            object_oid: object_id.clone(),
            expected_kind,
        }
        .into()),
        Err(ReadObjectError::Io) => Err(RepositoryError::Unexpected),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GitObjectKind {
    Commit,
    Tree,
    Blob,
    Tag,
}

struct GitObject {
    kind: GitObjectKind,
    body_prefix: Vec<u8>,
    body_size: usize,
}

enum ReadObjectError {
    Invalid,
    Io,
}

fn read_object(git_dir: &Path, object_id: &ObjectId) -> Result<Option<GitObject>, ReadObjectError> {
    let path = loose_object_path(git_dir, object_id);
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(ReadObjectError::Io),
    };
    let mut decoder = ZlibDecoder::new(file);
    let mut hasher = Sha1::new();
    let mut header = Vec::with_capacity(64);
    loop {
        let mut byte = [0_u8; 1];
        if decoder
            .read(&mut byte)
            .map_err(|_| ReadObjectError::Invalid)?
            == 0
        {
            return Err(ReadObjectError::Invalid);
        }
        hasher.update(byte);
        if byte[0] == 0 {
            break;
        }
        if header.len() == 64 {
            return Err(ReadObjectError::Invalid);
        }
        header.push(byte[0]);
    }
    let (kind, body_size) = parse_object_header(&header).ok_or(ReadObjectError::Invalid)?;
    let capture_limit = match kind {
        GitObjectKind::Commit | GitObjectKind::Blob => 64,
        GitObjectKind::Tree => 512,
        GitObjectKind::Tag => 0,
    };
    let mut body_prefix = Vec::with_capacity(capture_limit.min(body_size));
    let mut observed_size = 0_usize;
    let mut buffer = [0_u8; 8_192];
    loop {
        let read = decoder
            .read(&mut buffer)
            .map_err(|_| ReadObjectError::Invalid)?;
        if read == 0 {
            break;
        }
        observed_size = observed_size
            .checked_add(read)
            .ok_or(ReadObjectError::Invalid)?;
        hasher.update(&buffer[..read]);
        let remaining = capture_limit.saturating_sub(body_prefix.len());
        body_prefix.extend_from_slice(&buffer[..read.min(remaining)]);
    }
    if observed_size != body_size {
        return Err(ReadObjectError::Invalid);
    }
    let actual_oid = format!("{:x}", hasher.finalize());
    if actual_oid != object_id.as_str() {
        return Err(ReadObjectError::Invalid);
    }
    Ok(Some(GitObject {
        kind,
        body_prefix,
        body_size,
    }))
}

fn loose_object_path(git_dir: &Path, object_id: &ObjectId) -> PathBuf {
    let value = object_id.as_str();
    git_dir.join("objects").join(&value[..2]).join(&value[2..])
}

fn parse_object_header(header: &[u8]) -> Option<(GitObjectKind, usize)> {
    let header = std::str::from_utf8(header).ok()?;
    let (kind, size) = header.split_once(' ')?;
    if size.is_empty() || !size.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let size = size.parse::<usize>().ok()?;
    let kind = match kind {
        "commit" => GitObjectKind::Commit,
        "tree" => GitObjectKind::Tree,
        "blob" => GitObjectKind::Blob,
        "tag" => GitObjectKind::Tag,
        _ => return None,
    };
    Some((kind, size))
}

fn parse_commit_tree(body: &[u8]) -> Option<ObjectId> {
    let line_end = body.iter().position(|byte| *byte == b'\n')?;
    let line = std::str::from_utf8(&body[..line_end]).ok()?;
    ObjectId::parse_sha1(line.strip_prefix("tree ")?)
}

fn parse_single_regular_file(object: &GitObject) -> Result<ObjectId, RepositoryError> {
    if object.body_size != object.body_prefix.len() {
        return Err(unsupported_single_file());
    }
    let body = &object.body_prefix;
    let Some(space) = body.iter().position(|byte| *byte == b' ') else {
        return Err(unsupported_single_file());
    };
    let Some(name_end_relative) = body[space + 1..].iter().position(|byte| *byte == 0) else {
        return Err(unsupported_single_file());
    };
    let name_end = space + 1 + name_end_relative;
    let mode = &body[..space];
    let name = &body[space + 1..name_end];
    let object_bytes = body
        .get(name_end + 1..)
        .ok_or_else(unsupported_single_file)?;
    if object_bytes.len() != 20 {
        return Err(unsupported_single_file());
    }
    match mode {
        b"120000" => {
            return Err(AcquisitionError::UnsupportedRepositoryShape {
                feature: UnsupportedFeature::Symlink,
            }
            .into());
        }
        b"160000" => {
            return Err(AcquisitionError::UnsupportedRepositoryShape {
                feature: UnsupportedFeature::SubmoduleOrGitlink,
            }
            .into());
        }
        b"100644" => {}
        _ => return Err(unsupported_single_file()),
    }
    if !valid_root_file_name(name) {
        return Err(unsupported_single_file());
    }
    let mut bytes = [0_u8; 20];
    bytes.copy_from_slice(object_bytes);
    Ok(ObjectId::from_bytes(&bytes))
}

fn valid_root_file_name(name: &[u8]) -> bool {
    let Some((&first, rest)) = name.split_first() else {
        return false;
    };
    name.len() <= 128
        && first.is_ascii_alphanumeric()
        && rest
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn unsupported_single_file() -> RepositoryError {
    AcquisitionError::UnsupportedRepositoryShape {
        feature: UnsupportedFeature::NonSingleRegularRootFile,
    }
    .into()
}
