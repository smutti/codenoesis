//! In-process local Git adapter for the approved S0 and S1 repository subsets.

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::Instant;

use codenoesis_domain::{
    AcquiredFile, AcquiredRepository, AcquisitionError, ActualObjectKind, BoundRevision,
    EntryPolicy, LimitKind, ObjectId, ObjectKind, PathInvalidReason, RegularFileMode,
    RepositoryError, RepositoryIdentity, Revision, RootPolicy, STANDARD_LOCAL_S1_LIMITS,
    UnsupportedFeature, limit_exceeded,
};
use codenoesis_ports::{RepositoryAcquirer, SafeRepositoryAcquirer};
use flate2::read::ZlibDecoder;
use sha1::{Digest, Sha1};

const S1_INTERNAL_OBJECT_BYTES: usize = 33_554_432;
const S1_INTERNAL_CONTROL_FILE_BYTES: u64 = 33_554_432;

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

impl SafeRepositoryAcquirer for LocalGitRepository {
    fn acquire_inventory(
        &self,
        repository: &OsStr,
        identity: RepositoryIdentity,
        revision: Revision,
    ) -> Result<AcquiredRepository, RepositoryError> {
        let root = Path::new(repository);
        let git_dir = validate_s1_repository_root(root)?;
        validate_s1_repository_shape(&git_dir)?;
        reject_packed_object_database(&git_dir)?;

        let commit_oid = resolve_s1_revision(&git_dir, &revision)?;
        let commit = required_s1_revision_object(&git_dir, &commit_oid, &revision)?;
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
        let tree = required_referenced_object_with_capture(
            &git_dir,
            &tree_oid,
            ObjectKind::Tree,
            &commit_oid,
            s1_tree_capture_limit(),
            None,
            s1_tree_capture_limit(),
        )?;
        if tree.kind != GitObjectKind::Tree || tree.body_size != tree.body_prefix.len() {
            return Err(AcquisitionError::RepositoryInconsistent {
                object_oid: tree_oid,
                expected_kind: ObjectKind::Tree,
            }
            .into());
        }

        let bound_revision = BoundRevision::new(identity, commit_oid.clone(), tree_oid.clone());
        let mut state = S1TraversalState::new();
        traverse_tree_object(&git_dir, &tree_oid, &tree, "", 0, &mut state)?;
        Ok(AcquiredRepository::new(
            bound_revision,
            state.directory_count,
            state.files,
        ))
    }
}

fn validate_s1_repository_root(root: &Path) -> Result<PathBuf, RepositoryError> {
    let root_metadata = match fs::symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(AcquisitionError::NotGitRepository.into());
        }
        Err(_) => return Err(RepositoryError::Unexpected),
    };
    if root_metadata.file_type().is_symlink() {
        return Err(AcquisitionError::RootPolicyViolation {
            policy: RootPolicy::RepositoryRootIsSymlink,
        }
        .into());
    }
    if !root_metadata.is_dir() {
        return Err(AcquisitionError::NotGitRepository.into());
    }

    let git_dir = root.join(".git");
    let git_metadata = match fs::symlink_metadata(&git_dir) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            if root.join("objects").is_dir() && root.join("HEAD").is_file() {
                return Err(AcquisitionError::UnsupportedRepositoryShape {
                    feature: UnsupportedFeature::BareRepository,
                }
                .into());
            }
            return Err(AcquisitionError::NotGitRepository.into());
        }
        Err(_) => return Err(RepositoryError::Unexpected),
    };
    if git_metadata.file_type().is_symlink() {
        return Err(AcquisitionError::RootPolicyViolation {
            policy: RootPolicy::GitDirectoryIsSymlink,
        }
        .into());
    }
    if !git_metadata.is_dir() {
        return Err(AcquisitionError::RootPolicyViolation {
            policy: RootPolicy::GitDirectoryNotContained,
        }
        .into());
    }

    let canonical_root = fs::canonicalize(root).map_err(|_| RepositoryError::Unexpected)?;
    let canonical_git = fs::canonicalize(&git_dir).map_err(|_| RepositoryError::Unexpected)?;
    if canonical_git.parent() != Some(canonical_root.as_path()) {
        return Err(AcquisitionError::RootPolicyViolation {
            policy: RootPolicy::GitDirectoryNotContained,
        }
        .into());
    }
    Ok(canonical_git)
}

fn reject_packed_object_database(git_dir: &Path) -> Result<(), RepositoryError> {
    let pack_dir = git_dir.join("objects/pack");
    let Some(metadata) = s1_relative_metadata(git_dir, "objects/pack")? else {
        return Ok(());
    };
    if !metadata.is_dir() {
        return Err(RepositoryError::Unexpected);
    }
    let entries = fs::read_dir(pack_dir).map_err(|_| RepositoryError::Unexpected)?;
    for entry in entries {
        let entry = entry.map_err(|_| RepositoryError::Unexpected)?;
        if matches!(
            entry.path().extension().and_then(OsStr::to_str),
            Some("pack" | "idx")
        ) {
            return Err(AcquisitionError::UnsupportedRepositoryShape {
                feature: UnsupportedFeature::PackedObjectDatabase,
            }
            .into());
        }
    }
    Ok(())
}

struct S1TraversalState {
    started_at: Instant,
    tree_entries: u64,
    regular_files: u64,
    cumulative_file_bytes: u64,
    directory_count: u64,
    canonical_paths: BTreeSet<String>,
    files: Vec<AcquiredFile>,
}

impl S1TraversalState {
    fn new() -> Self {
        Self {
            started_at: Instant::now(),
            tree_entries: 0,
            regular_files: 0,
            cumulative_file_bytes: 0,
            directory_count: 0,
            canonical_paths: BTreeSet::new(),
            files: Vec::new(),
        }
    }

    fn check_time(&self) -> Result<(), RepositoryError> {
        let observed = u64::try_from(self.started_at.elapsed().as_millis()).unwrap_or(u64::MAX);
        if observed > STANDARD_LOCAL_S1_LIMITS.scan_wall_milliseconds {
            return Err(limit_exceeded(LimitKind::ScanWallMilliseconds, observed).into());
        }
        Ok(())
    }

    fn observe_entry(
        &mut self,
        path: &str,
        parent_tree_oid: &ObjectId,
    ) -> Result<(), RepositoryError> {
        self.tree_entries = self
            .tree_entries
            .checked_add(1)
            .ok_or(RepositoryError::Unexpected)?;
        if self.tree_entries > STANDARD_LOCAL_S1_LIMITS.tree_entries {
            return Err(limit_exceeded(LimitKind::TreeEntries, self.tree_entries).into());
        }
        if !self.canonical_paths.insert(path.to_owned()) {
            return Err(AcquisitionError::RepositoryInconsistent {
                object_oid: parent_tree_oid.clone(),
                expected_kind: ObjectKind::Tree,
            }
            .into());
        }
        Ok(())
    }

    fn observe_regular_path(&mut self) -> Result<(), RepositoryError> {
        self.regular_files = self
            .regular_files
            .checked_add(1)
            .ok_or(RepositoryError::Unexpected)?;
        if self.regular_files > STANDARD_LOCAL_S1_LIMITS.regular_files {
            return Err(limit_exceeded(LimitKind::RegularFiles, self.regular_files).into());
        }
        Ok(())
    }

    fn observe_file_bytes(&mut self, byte_length: u64) -> Result<(), RepositoryError> {
        self.cumulative_file_bytes = self
            .cumulative_file_bytes
            .checked_add(byte_length)
            .ok_or(RepositoryError::Unexpected)?;
        if self.cumulative_file_bytes > STANDARD_LOCAL_S1_LIMITS.cumulative_file_bytes {
            return Err(
                limit_exceeded(LimitKind::CumulativeFileBytes, self.cumulative_file_bytes).into(),
            );
        }
        Ok(())
    }
}

fn traverse_tree_object(
    git_dir: &Path,
    tree_oid: &ObjectId,
    tree: &GitObject,
    prefix: &str,
    parent_depth: u64,
    state: &mut S1TraversalState,
) -> Result<(), RepositoryError> {
    state.check_time()?;
    let mut entries = parse_tree_entries(&tree.body_prefix).ok_or_else(|| {
        RepositoryError::from(AcquisitionError::RepositoryInconsistent {
            object_oid: tree_oid.clone(),
            expected_kind: ObjectKind::Tree,
        })
    })?;
    entries.sort_by(|left, right| left.name.cmp(&right.name));

    for entry in entries {
        state.check_time()?;
        let component = validate_s1_path_component(&entry.name)?;
        let path = if prefix.is_empty() {
            component
        } else {
            format!("{prefix}/{component}")
        };
        let depth = parent_depth
            .checked_add(1)
            .ok_or(RepositoryError::Unexpected)?;
        check_path_limits(&path, &entry.name, depth)?;

        match entry.mode.as_slice() {
            b"040000" | b"40000" => {
                state.observe_entry(&path, tree_oid)?;
                traverse_directory(git_dir, tree_oid, &entry, &path, depth, state)?;
            }
            b"100644" | b"100755" => {
                state.observe_entry(&path, tree_oid)?;
                state.observe_regular_path()?;
                acquire_regular_file(git_dir, tree_oid, &entry, path, state)?;
            }
            b"120000" => return Err(entry_policy_error(path, EntryPolicy::Symlink)),
            b"160000" => return Err(entry_policy_error(path, EntryPolicy::Gitlink)),
            _ => return Err(entry_policy_error(path, EntryPolicy::SpecialFileMode)),
        }
    }
    Ok(())
}

fn traverse_directory(
    git_dir: &Path,
    parent_tree_oid: &ObjectId,
    entry: &RawTreeEntry,
    path: &str,
    depth: u64,
    state: &mut S1TraversalState,
) -> Result<(), RepositoryError> {
    let child = required_referenced_object_with_capture(
        git_dir,
        &entry.object_id,
        ObjectKind::Tree,
        parent_tree_oid,
        s1_tree_capture_limit(),
        None,
        s1_tree_capture_limit(),
    )?;
    if child.kind != GitObjectKind::Tree || child.body_size != child.body_prefix.len() {
        return Err(AcquisitionError::RepositoryInconsistent {
            object_oid: entry.object_id.clone(),
            expected_kind: ObjectKind::Tree,
        }
        .into());
    }
    state.directory_count = state
        .directory_count
        .checked_add(1)
        .ok_or(RepositoryError::Unexpected)?;
    traverse_tree_object(git_dir, &entry.object_id, &child, path, depth, state)
}

fn acquire_regular_file(
    git_dir: &Path,
    parent_tree_oid: &ObjectId,
    entry: &RawTreeEntry,
    path: String,
    state: &mut S1TraversalState,
) -> Result<(), RepositoryError> {
    let blob = required_referenced_object_with_capture(
        git_dir,
        &entry.object_id,
        ObjectKind::Blob,
        parent_tree_oid,
        s1_blob_capture_limit(),
        Some(s1_blob_body_limit(state.cumulative_file_bytes)),
        s1_blob_capture_limit(),
    )?;
    if blob.kind != GitObjectKind::Blob {
        return Err(AcquisitionError::RepositoryInconsistent {
            object_oid: entry.object_id.clone(),
            expected_kind: ObjectKind::Blob,
        }
        .into());
    }
    let byte_length = u64::try_from(blob.body_size).map_err(|_| RepositoryError::Unexpected)?;
    if byte_length > STANDARD_LOCAL_S1_LIMITS.single_file_bytes {
        return Err(limit_exceeded(LimitKind::SingleFileBytes, byte_length).into());
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
    state.observe_file_bytes(byte_length)?;
    let mode = if entry.mode == b"100755" {
        RegularFileMode::Executable
    } else {
        RegularFileMode::Regular
    };
    state.files.push(AcquiredFile::new(
        path,
        mode,
        entry.object_id.clone(),
        blob.body_prefix,
    ));
    Ok(())
}

fn entry_policy_error(path: String, entry: EntryPolicy) -> RepositoryError {
    AcquisitionError::EntryPolicyViolation { path, entry }.into()
}

fn validate_s1_path_component(name: &[u8]) -> Result<String, RepositoryError> {
    if name.is_empty() {
        return Err(AcquisitionError::PathInvalid {
            reason: PathInvalidReason::EmptyComponent,
        }
        .into());
    }
    if name.contains(&b'/') {
        return Err(RepositoryError::Unexpected);
    }
    let component = std::str::from_utf8(name).map_err(|_| {
        RepositoryError::from(AcquisitionError::PathInvalid {
            reason: PathInvalidReason::NonUtf8,
        })
    })?;
    if matches!(component, "." | "..") {
        return Err(AcquisitionError::PathInvalid {
            reason: PathInvalidReason::DotComponent,
        }
        .into());
    }
    if name.contains(&b'\\') {
        return Err(AcquisitionError::PathInvalid {
            reason: PathInvalidReason::Backslash,
        }
        .into());
    }
    if name
        .iter()
        .any(|byte| byte.is_ascii_control() || *byte == 0x7f)
    {
        return Err(AcquisitionError::PathInvalid {
            reason: PathInvalidReason::ControlCharacter,
        }
        .into());
    }
    Ok(component.to_owned())
}

fn check_path_limits(path: &str, component: &[u8], depth: u64) -> Result<(), RepositoryError> {
    let component_length =
        u64::try_from(component.len()).map_err(|_| RepositoryError::Unexpected)?;
    if component_length > STANDARD_LOCAL_S1_LIMITS.path_component_bytes {
        return Err(limit_exceeded(LimitKind::PathComponentBytes, component_length).into());
    }
    let path_length = u64::try_from(path.len()).map_err(|_| RepositoryError::Unexpected)?;
    if path_length > STANDARD_LOCAL_S1_LIMITS.path_bytes {
        return Err(limit_exceeded(LimitKind::PathBytes, path_length).into());
    }
    if depth > STANDARD_LOCAL_S1_LIMITS.recursion_depth {
        return Err(limit_exceeded(LimitKind::RecursionDepth, depth).into());
    }
    Ok(())
}

fn s1_tree_capture_limit() -> usize {
    usize::try_from(STANDARD_LOCAL_S1_LIMITS.canonical_output_bytes)
        .expect("S1 canonical-output limit fits usize")
}

fn s1_blob_capture_limit() -> usize {
    usize::try_from(STANDARD_LOCAL_S1_LIMITS.single_file_bytes)
        .expect("S1 single-file limit fits usize")
}

#[derive(Clone, Copy)]
struct DeclaredBodyLimit {
    limit: LimitKind,
    body_maximum: usize,
    observed_offset: u64,
}

fn s1_blob_body_limit(cumulative_file_bytes: u64) -> DeclaredBodyLimit {
    let cumulative_remaining = STANDARD_LOCAL_S1_LIMITS
        .cumulative_file_bytes
        .saturating_sub(cumulative_file_bytes);
    if cumulative_remaining < STANDARD_LOCAL_S1_LIMITS.single_file_bytes {
        DeclaredBodyLimit {
            limit: LimitKind::CumulativeFileBytes,
            body_maximum: usize::try_from(cumulative_remaining)
                .expect("S1 cumulative-byte remainder fits usize"),
            observed_offset: cumulative_file_bytes,
        }
    } else {
        DeclaredBodyLimit {
            limit: LimitKind::SingleFileBytes,
            body_maximum: s1_blob_capture_limit(),
            observed_offset: 0,
        }
    }
}

struct RawTreeEntry {
    mode: Vec<u8>,
    name: Vec<u8>,
    object_id: ObjectId,
}

fn parse_tree_entries(body: &[u8]) -> Option<Vec<RawTreeEntry>> {
    let mut entries = Vec::new();
    let mut offset = 0;
    while offset < body.len() {
        let space = body[offset..].iter().position(|byte| *byte == b' ')? + offset;
        let name_start = space.checked_add(1)?;
        let name_end = body[name_start..].iter().position(|byte| *byte == 0)? + name_start;
        let object_start = name_end.checked_add(1)?;
        let object_end = object_start.checked_add(20)?;
        let object_bytes: [u8; 20] = body.get(object_start..object_end)?.try_into().ok()?;
        entries.push(RawTreeEntry {
            mode: body[offset..space].to_vec(),
            name: body[name_start..name_end].to_vec(),
            object_id: ObjectId::from_bytes(&object_bytes),
        });
        offset = object_end;
    }
    Some(entries)
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

fn validate_s1_repository_shape(git_dir: &Path) -> Result<(), RepositoryError> {
    let config = read_s1_control_file(git_dir, "config")?.ok_or(RepositoryError::Unexpected)?;
    let config = std::str::from_utf8(&config).map_err(|_| RepositoryError::Unexpected)?;
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
    if s1_relative_metadata(git_dir, "shallow")?.is_some() {
        return Err(AcquisitionError::UnsupportedRepositoryShape {
            feature: UnsupportedFeature::ShallowRepository,
        }
        .into());
    }
    if let Some(alternates) = read_s1_control_file(git_dir, "objects/info/alternates")?
        && alternates.iter().any(|byte| !byte.is_ascii_whitespace())
    {
        return Err(AcquisitionError::UnsupportedRepositoryShape {
            feature: UnsupportedFeature::AlternateObjectDatabase,
        }
        .into());
    }
    if s1_relative_metadata(git_dir, "info/grafts")?.is_some()
        || s1_relative_metadata(git_dir, "refs/replace")?.is_some()
    {
        return Err(AcquisitionError::UnsupportedRepositoryShape {
            feature: UnsupportedFeature::ReplaceOrGraft,
        }
        .into());
    }
    let objects = s1_relative_metadata(git_dir, "objects")?.ok_or(RepositoryError::Unexpected)?;
    if !objects.is_dir() {
        return Err(RepositoryError::Unexpected);
    }
    Ok(())
}

fn read_s1_control_file(
    git_dir: &Path,
    relative: &str,
) -> Result<Option<Vec<u8>>, RepositoryError> {
    let Some(metadata) = s1_relative_metadata(git_dir, relative)? else {
        return Ok(None);
    };
    if !metadata.is_file() || metadata.len() > S1_INTERNAL_CONTROL_FILE_BYTES {
        return Err(RepositoryError::Unexpected);
    }
    let file = File::open(git_dir.join(relative)).map_err(|_| RepositoryError::Unexpected)?;
    let mut bytes = Vec::with_capacity(
        usize::try_from(metadata.len()).map_err(|_| RepositoryError::Unexpected)?,
    );
    file.take(S1_INTERNAL_CONTROL_FILE_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| RepositoryError::Unexpected)?;
    if u64::try_from(bytes.len()).map_err(|_| RepositoryError::Unexpected)?
        > S1_INTERNAL_CONTROL_FILE_BYTES
    {
        return Err(RepositoryError::Unexpected);
    }
    Ok(Some(bytes))
}

fn s1_relative_metadata(
    git_dir: &Path,
    relative: &str,
) -> Result<Option<fs::Metadata>, RepositoryError> {
    let mut current = git_dir.to_path_buf();
    let mut components = Path::new(relative).components().peekable();
    while let Some(component) = components.next() {
        let std::path::Component::Normal(component) = component else {
            return Err(RepositoryError::Unexpected);
        };
        current.push(component);
        let metadata = match fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(RepositoryError::Unexpected),
        };
        if metadata.file_type().is_symlink() || components.peek().is_some() && !metadata.is_dir() {
            return Err(RepositoryError::Unexpected);
        }
        if components.peek().is_none() {
            return Ok(Some(metadata));
        }
    }
    Err(RepositoryError::Unexpected)
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

fn resolve_s1_revision(git_dir: &Path, revision: &Revision) -> Result<ObjectId, RepositoryError> {
    match revision {
        Revision::Commit(object_id) => Ok(object_id.clone()),
        Revision::Main => resolve_s1_main_ref(git_dir)?.ok_or_else(|| {
            RepositoryError::from(AcquisitionError::RevisionNotFound {
                revision: revision.clone(),
            })
        }),
    }
}

fn resolve_s1_main_ref(git_dir: &Path) -> Result<Option<ObjectId>, RepositoryError> {
    if let Some(value) = read_s1_control_file(git_dir, "refs/heads/main")? {
        let value = std::str::from_utf8(&value).map_err(|_| RepositoryError::Unexpected)?;
        return Ok(ObjectId::parse_sha1(value.trim_end_matches(['\r', '\n'])));
    }
    let Some(packed) = read_s1_control_file(git_dir, "packed-refs")? else {
        return Ok(None);
    };
    let packed = std::str::from_utf8(&packed).map_err(|_| RepositoryError::Unexpected)?;
    Ok(packed.lines().find_map(|line| {
        let (object_id, name) = line.split_once(' ')?;
        (name == "refs/heads/main")
            .then(|| ObjectId::parse_sha1(object_id))
            .flatten()
    }))
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
        Err(ReadObjectError::Io | ReadObjectError::LimitExceeded { .. }) => {
            Err(RepositoryError::Unexpected)
        }
    }
}

fn required_s1_revision_object(
    git_dir: &Path,
    object_id: &ObjectId,
    revision: &Revision,
) -> Result<GitObject, RepositoryError> {
    match read_object_capped(
        git_dir,
        object_id,
        Some(64),
        None,
        Some(S1_INTERNAL_OBJECT_BYTES),
    ) {
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
        Err(ReadObjectError::Io | ReadObjectError::LimitExceeded { .. }) => {
            Err(RepositoryError::Unexpected)
        }
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
        Err(ReadObjectError::Io | ReadObjectError::LimitExceeded { .. }) => {
            Err(RepositoryError::Unexpected)
        }
    }
}

fn required_referenced_object_with_capture(
    git_dir: &Path,
    object_id: &ObjectId,
    expected_kind: ObjectKind,
    referenced_by: &ObjectId,
    capture_limit: usize,
    declared_body_limit: Option<DeclaredBodyLimit>,
    body_ceiling: usize,
) -> Result<GitObject, RepositoryError> {
    match read_object_with_capture(
        git_dir,
        object_id,
        capture_limit,
        declared_body_limit,
        body_ceiling,
    ) {
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
        Err(ReadObjectError::LimitExceeded { limit, observed }) => {
            Err(limit_exceeded(limit, observed).into())
        }
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
    LimitExceeded { limit: LimitKind, observed: u64 },
}

fn read_object(git_dir: &Path, object_id: &ObjectId) -> Result<Option<GitObject>, ReadObjectError> {
    read_object_capped(git_dir, object_id, None, None, None)
}

fn read_object_with_capture(
    git_dir: &Path,
    object_id: &ObjectId,
    capture_limit: usize,
    declared_body_limit: Option<DeclaredBodyLimit>,
    body_ceiling: usize,
) -> Result<Option<GitObject>, ReadObjectError> {
    read_object_capped(
        git_dir,
        object_id,
        Some(capture_limit),
        declared_body_limit,
        Some(body_ceiling),
    )
}

fn read_object_capped(
    git_dir: &Path,
    object_id: &ObjectId,
    requested_capture_limit: Option<usize>,
    declared_body_limit: Option<DeclaredBodyLimit>,
    body_ceiling: Option<usize>,
) -> Result<Option<GitObject>, ReadObjectError> {
    let path = loose_object_path(git_dir, object_id);
    if let Some(body_ceiling) = body_ceiling {
        let compressed_ceiling = body_ceiling
            .saturating_add(body_ceiling / 100)
            .saturating_add(1_024);
        if !validate_s1_loose_object_path(git_dir, &path, compressed_ceiling)? {
            return Ok(None);
        }
    }
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
    if kind == GitObjectKind::Blob
        && let Some(limit) = declared_body_limit
        && body_size > limit.body_maximum
    {
        let body_size = u64::try_from(body_size).unwrap_or(u64::MAX);
        return Err(ReadObjectError::LimitExceeded {
            limit: limit.limit,
            observed: limit.observed_offset.saturating_add(body_size),
        });
    }
    if body_ceiling.is_some_and(|maximum| body_size > maximum) {
        return Err(ReadObjectError::Invalid);
    }
    let capture_limit = requested_capture_limit.unwrap_or(match kind {
        GitObjectKind::Commit | GitObjectKind::Blob => 64,
        GitObjectKind::Tree => 512,
        GitObjectKind::Tag => 0,
    });
    let mut body_prefix = Vec::with_capacity(capture_limit.min(body_size));
    let mut observed_size = 0_usize;
    let mut buffer = [0_u8; 8_192];
    loop {
        let framing_remaining = body_size.saturating_sub(observed_size);
        let read_capacity = framing_remaining.saturating_add(1).min(buffer.len());
        let read = decoder
            .read(&mut buffer[..read_capacity])
            .map_err(|_| ReadObjectError::Invalid)?;
        if read == 0 {
            break;
        }
        observed_size = observed_size
            .checked_add(read)
            .ok_or(ReadObjectError::Invalid)?;
        if observed_size > body_size {
            return Err(ReadObjectError::Invalid);
        }
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

fn validate_s1_loose_object_path(
    git_dir: &Path,
    object_path: &Path,
    compressed_ceiling: usize,
) -> Result<bool, ReadObjectError> {
    let objects = git_dir.join("objects");
    let objects_root = fs::symlink_metadata(&objects).map_err(|_| ReadObjectError::Io)?;
    if objects_root.file_type().is_symlink() || !objects_root.is_dir() {
        return Err(ReadObjectError::Invalid);
    }

    let object_directory = object_path.parent().ok_or(ReadObjectError::Invalid)?;
    let fanout_directory = match fs::symlink_metadata(object_directory) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(_) => return Err(ReadObjectError::Io),
    };
    if fanout_directory.file_type().is_symlink() || !fanout_directory.is_dir() {
        return Err(ReadObjectError::Invalid);
    }

    let loose_file = match fs::symlink_metadata(object_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(_) => return Err(ReadObjectError::Io),
    };
    if loose_file.file_type().is_symlink()
        || !loose_file.is_file()
        || loose_file.len() > u64::try_from(compressed_ceiling).unwrap_or(u64::MAX)
    {
        return Err(ReadObjectError::Invalid);
    }
    Ok(true)
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write as _;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use codenoesis_domain::{AcquisitionError, LimitKind, PathInvalidReason, RepositoryError};
    use flate2::Compression;
    use flate2::write::ZlibEncoder;

    use super::{
        ObjectId, ReadObjectError, S1TraversalState, STANDARD_LOCAL_S1_LIMITS, check_path_limits,
        read_object_with_capture, s1_blob_body_limit, s1_blob_capture_limit,
        validate_s1_path_component,
    };

    #[test]
    fn pt_fr_acq_002_path_boundaries_and_invalid_components_are_typed() {
        let component_at_limit = vec![b'a'; 255];
        assert!(validate_s1_path_component(&component_at_limit).is_ok());
        assert_eq!(
            check_path_limits(
                std::str::from_utf8(&component_at_limit).expect("ASCII component"),
                &component_at_limit,
                1,
            ),
            Ok(())
        );

        let component_over_limit = vec![b'a'; 256];
        let component =
            validate_s1_path_component(&component_over_limit).expect("valid UTF-8 component");
        assert_limit(
            &check_path_limits(&component, &component_over_limit, 1),
            LimitKind::PathComponentBytes,
        );
        assert_eq!(check_path_limits(&"a".repeat(1_024), b"a", 32), Ok(()));
        assert_limit(
            &check_path_limits(&"a".repeat(1_025), b"a", 32),
            LimitKind::PathBytes,
        );
        assert_limit(&check_path_limits("a", b"a", 33), LimitKind::RecursionDepth);

        assert_path_reason(b"", PathInvalidReason::EmptyComponent);
        assert_path_reason(b"..", PathInvalidReason::DotComponent);
        assert_path_reason(b"a\\b", PathInvalidReason::Backslash);
        assert_path_reason(b"a\x7f", PathInvalidReason::ControlCharacter);
        assert_path_reason(b"\xff", PathInvalidReason::NonUtf8);
    }

    #[test]
    fn pt_fr_acq_002_adapter_counters_have_max_and_plus_one() {
        let parent_tree_oid = ObjectId::parse_sha1("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .expect("test tree OID");
        let mut state = S1TraversalState::new();

        state.tree_entries = STANDARD_LOCAL_S1_LIMITS.tree_entries - 1;
        assert_eq!(state.observe_entry("at-max", &parent_tree_oid), Ok(()));
        assert_limit(
            &state.observe_entry("over-max", &parent_tree_oid),
            LimitKind::TreeEntries,
        );

        state.regular_files = STANDARD_LOCAL_S1_LIMITS.regular_files - 1;
        assert_eq!(state.observe_regular_path(), Ok(()));
        assert_limit(&state.observe_regular_path(), LimitKind::RegularFiles);

        state.cumulative_file_bytes = STANDARD_LOCAL_S1_LIMITS.cumulative_file_bytes - 1;
        assert_eq!(state.observe_file_bytes(1), Ok(()));
        assert_limit(&state.observe_file_bytes(1), LimitKind::CumulativeFileBytes);
    }

    #[test]
    fn sec_nfr_sec_001_declared_blob_bomb_stops_before_body() {
        let root = unique_test_root();
        let git_dir = root.join(".git");
        let object_id = ObjectId::parse_sha1("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .expect("test blob OID");
        let object_path = git_dir
            .join("objects")
            .join("aa")
            .join("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        fs::create_dir_all(object_path.parent().expect("object parent"))
            .expect("create object directory");

        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(b"blob 4194305\0")
            .expect("encode declared blob header");
        fs::write(
            &object_path,
            encoder.finish().expect("finish declared blob header"),
        )
        .expect("write header-only loose object");

        let result = read_object_with_capture(
            &git_dir,
            &object_id,
            s1_blob_capture_limit(),
            Some(s1_blob_body_limit(0)),
            s1_blob_capture_limit(),
        );
        assert!(matches!(
            result,
            Err(ReadObjectError::LimitExceeded {
                limit: LimitKind::SingleFileBytes,
                observed: 4_194_305
            })
        ));
        fs::remove_dir_all(root).expect("remove object test root");
    }

    fn assert_limit(result: &Result<(), RepositoryError>, limit: LimitKind) {
        assert_eq!(
            result,
            &Err(RepositoryError::Acquisition(
                AcquisitionError::LimitExceeded {
                    limit,
                    maximum: limit.maximum(),
                    observed: limit.maximum() + 1,
                }
            ))
        );
    }

    fn assert_path_reason(value: &[u8], reason: PathInvalidReason) {
        assert_eq!(
            validate_s1_path_component(value),
            Err(RepositoryError::Acquisition(
                AcquisitionError::PathInvalid { reason }
            ))
        );
    }

    fn unique_test_root() -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "codenoesis-repository-test-{}-{timestamp}",
            std::process::id()
        ))
    }
}
