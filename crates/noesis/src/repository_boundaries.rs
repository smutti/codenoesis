use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs;
use std::io::Read as _;
use std::path::{Component, Path, PathBuf};

use codenoesis_application::{PreparedNestedRepositoryRoot, RepositoryBoundaryScanInput};
use codenoesis_contracts::{
    BoundaryManifestReason, CodeNoesisErrorV9, NestedRepositoryUnavailableReason,
    RepositoryBoundaryInputError, parse_repository_boundary_input,
};
use codenoesis_domain::s1_boundaries::{
    BoundaryLimit, BoundarySha256, MAX_BOUNDARY_MANIFEST_BYTES,
};
use codenoesis_domain::{
    AcquisitionError, RepositoryError, RepositoryIdentity, Revision, UnsupportedFeature,
};
use sha2::{Digest, Sha256};

pub(crate) struct Sha256BoundaryHasher;

impl BoundarySha256 for Sha256BoundaryHasher {
    fn digest(&self, bytes: &[u8]) -> [u8; 32] {
        Sha256::digest(bytes).into()
    }
}

pub(crate) struct PreparedRepositoryBoundaries {
    pub(crate) scan_input: RepositoryBoundaryScanInput,
    pub(crate) manifest_path: Option<PathBuf>,
    pub(crate) nested_roots: Vec<PathBuf>,
}

impl PreparedRepositoryBoundaries {
    pub(crate) fn reject_overlaps(&mut self, protected_root: &Path) {
        for nested in &mut self.scan_input.nested_roots {
            let PreparedNestedRepositoryRoot::Available(root) = nested else {
                continue;
            };
            let root = Path::new(root);
            if root.starts_with(protected_root) || protected_root.starts_with(root) {
                *nested = PreparedNestedRepositoryRoot::Unavailable;
            }
        }
        self.nested_roots
            .retain(|root| !root.starts_with(protected_root) && !protected_root.starts_with(root));
    }
}

pub(crate) struct RepositoryBoundaryFailure {
    pub(crate) error: CodeNoesisErrorV9,
    pub(crate) exit_code: u8,
}

pub(crate) fn prepare(
    manifest: Option<&OsStr>,
    identity: &RepositoryIdentity,
    revision: &Revision,
) -> Result<PreparedRepositoryBoundaries, RepositoryBoundaryFailure> {
    let Some(manifest) = manifest else {
        return Ok(PreparedRepositoryBoundaries {
            scan_input: RepositoryBoundaryScanInput {
                manifest: None,
                nested_roots: Vec::new(),
            },
            manifest_path: None,
            nested_roots: Vec::new(),
        });
    };
    let manifest_path = canonical_manifest(Path::new(manifest))?;
    let bytes = read_manifest(&manifest_path)?;
    let input = parse_repository_boundary_input(&bytes).map_err(input_failure)?;
    if input.root_repository_identity != *identity
        || !matches!(revision, Revision::Commit(commit) if commit == &input.root_commit_oid)
    {
        return Err(invalid_manifest(BoundaryManifestReason::RootMismatch));
    }

    let base = manifest_path
        .parent()
        .ok_or_else(|| invalid_manifest(BoundaryManifestReason::ManifestUnavailable))?;
    let mut canonical_roots = BTreeSet::new();
    let mut nested_roots = Vec::with_capacity(input.nested_repositories.len());
    let mut prepared_roots = Vec::with_capacity(input.nested_repositories.len());
    for nested in &input.nested_repositories {
        match resolve_root(base, &nested.repository_root, &nested.boundary_path) {
            Ok(root) => {
                if !canonical_roots.insert(root.clone()) {
                    return Err(invalid_manifest(BoundaryManifestReason::SchemaInvalid));
                }
                prepared_roots.push(PreparedNestedRepositoryRoot::Available(
                    root.as_os_str().to_owned(),
                ));
                nested_roots.push(root);
            }
            Err(_) => prepared_roots.push(PreparedNestedRepositoryRoot::Unavailable),
        }
    }
    Ok(PreparedRepositoryBoundaries {
        scan_input: RepositoryBoundaryScanInput {
            manifest: Some(input),
            nested_roots: prepared_roots,
        },
        manifest_path: Some(manifest_path),
        nested_roots,
    })
}

pub(crate) fn nested_reason(error: &RepositoryError) -> NestedRepositoryUnavailableReason {
    match error {
        RepositoryError::Acquisition(error) => acquisition_reason(error),
        RepositoryError::Unexpected => NestedRepositoryUnavailableReason::RepositoryInconsistent,
    }
}

fn acquisition_reason(error: &AcquisitionError) -> NestedRepositoryUnavailableReason {
    match error {
        AcquisitionError::NotGitRepository => NestedRepositoryUnavailableReason::NotGitRepository,
        AcquisitionError::RevisionNotFound { .. } => {
            NestedRepositoryUnavailableReason::RevisionNotFound
        }
        AcquisitionError::RevisionNotCommit { .. } => {
            NestedRepositoryUnavailableReason::RevisionNotCommit
        }
        AcquisitionError::ObjectMissing { .. } => NestedRepositoryUnavailableReason::ObjectMissing,
        AcquisitionError::RootPolicyViolation { .. } => {
            NestedRepositoryUnavailableReason::RootPolicyViolation
        }
        AcquisitionError::RepositoryInconsistent { .. } => {
            NestedRepositoryUnavailableReason::RepositoryInconsistent
        }
        AcquisitionError::UnsupportedRepositoryShape {
            feature: UnsupportedFeature::PackedAcquisition(error),
        } => match error {
            codenoesis_domain::s1_packed::PackedAcquisitionError::Invalid(_) => {
                NestedRepositoryUnavailableReason::ObjectDatabaseInvalid
            }
            codenoesis_domain::s1_packed::PackedAcquisitionError::Unavailable(_) => {
                NestedRepositoryUnavailableReason::ObjectDatabaseUnavailable
            }
            codenoesis_domain::s1_packed::PackedAcquisitionError::Changed(_) => {
                NestedRepositoryUnavailableReason::RepositoryInconsistent
            }
        },
        AcquisitionError::UnsupportedRepositoryShape { .. } => {
            NestedRepositoryUnavailableReason::UnsupportedRepositoryShape
        }
        AcquisitionError::LimitExceeded { .. } => {
            NestedRepositoryUnavailableReason::ObjectLimitExceeded
        }
        AcquisitionError::PathInvalid { .. } => NestedRepositoryUnavailableReason::PathInvalid,
        AcquisitionError::EntryPolicyViolation { .. } => {
            NestedRepositoryUnavailableReason::EntryPolicyViolation
        }
    }
}

fn canonical_manifest(path: &Path) -> Result<PathBuf, RepositoryBoundaryFailure> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| invalid_manifest(BoundaryManifestReason::ManifestUnavailable))?;
    if metadata_is_unsafe(&metadata) || !metadata.is_file() {
        return Err(invalid_manifest(
            BoundaryManifestReason::ManifestUnavailable,
        ));
    }
    if metadata.len() > MAX_BOUNDARY_MANIFEST_BYTES {
        return Err(limit_failure(metadata.len()));
    }
    fs::canonicalize(path)
        .map_err(|_| invalid_manifest(BoundaryManifestReason::ManifestUnavailable))
}

fn read_manifest(path: &Path) -> Result<Vec<u8>, RepositoryBoundaryFailure> {
    let path_before = fs::symlink_metadata(path)
        .map_err(|_| invalid_manifest(BoundaryManifestReason::ManifestUnavailable))?;
    if metadata_is_unsafe(&path_before) || !path_before.is_file() {
        return Err(invalid_manifest(
            BoundaryManifestReason::ManifestUnavailable,
        ));
    }
    let expected_identity = file_identity(&path_before);
    let file = fs::File::open(path)
        .map_err(|_| invalid_manifest(BoundaryManifestReason::ManifestUnavailable))?;
    let opened = file
        .metadata()
        .map_err(|_| invalid_manifest(BoundaryManifestReason::ManifestUnavailable))?;
    if file_identity(&opened) != expected_identity {
        return Err(invalid_manifest(
            BoundaryManifestReason::ManifestUnavailable,
        ));
    }
    let mut bytes = Vec::new();
    file.take(MAX_BOUNDARY_MANIFEST_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| invalid_manifest(BoundaryManifestReason::ManifestUnavailable))?;
    let observed = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if observed > MAX_BOUNDARY_MANIFEST_BYTES {
        return Err(limit_failure(observed));
    }
    let path_after = fs::symlink_metadata(path)
        .map_err(|_| invalid_manifest(BoundaryManifestReason::ManifestUnavailable))?;
    if metadata_is_unsafe(&path_after) || file_identity(&path_after) != expected_identity {
        return Err(invalid_manifest(
            BoundaryManifestReason::ManifestUnavailable,
        ));
    }
    Ok(bytes)
}

fn resolve_root(
    base: &Path,
    relative: &str,
    boundary_path: &str,
) -> Result<PathBuf, RepositoryBoundaryFailure> {
    let mut resolved = base.to_path_buf();
    let mut identities = Vec::new();
    for component in Path::new(relative).components() {
        let Component::Normal(component) = component else {
            return Err(nested_path_failure(boundary_path));
        };
        resolved.push(component);
        let metadata =
            fs::symlink_metadata(&resolved).map_err(|_| nested_path_failure(boundary_path))?;
        if metadata_is_unsafe(&metadata) || !metadata.is_dir() {
            return Err(nested_path_failure(boundary_path));
        }
        identities.push((resolved.clone(), file_identity(&metadata)));
    }
    let canonical = fs::canonicalize(&resolved).map_err(|_| nested_path_failure(boundary_path))?;
    if !canonical.starts_with(base) {
        return Err(nested_path_failure(boundary_path));
    }
    for (path, expected_identity) in identities {
        let metadata =
            fs::symlink_metadata(path).map_err(|_| nested_path_failure(boundary_path))?;
        if metadata_is_unsafe(&metadata)
            || !metadata.is_dir()
            || file_identity(&metadata) != expected_identity
        {
            return Err(nested_path_failure(boundary_path));
        }
    }
    Ok(canonical)
}

#[cfg(unix)]
type FileIdentity = (u64, u64);

#[cfg(unix)]
fn file_identity(metadata: &fs::Metadata) -> FileIdentity {
    use std::os::unix::fs::MetadataExt as _;

    (metadata.dev(), metadata.ino())
}

#[cfg(windows)]
type FileIdentity = (u64, u64, u64, u32);

#[cfg(windows)]
fn file_identity(metadata: &fs::Metadata) -> FileIdentity {
    use std::os::windows::fs::MetadataExt as _;

    (
        metadata.creation_time(),
        metadata.last_write_time(),
        metadata.file_size(),
        metadata.file_attributes(),
    )
}

#[cfg(not(any(unix, windows)))]
type FileIdentity = (u64, Option<std::time::SystemTime>);

#[cfg(not(any(unix, windows)))]
fn file_identity(metadata: &fs::Metadata) -> FileIdentity {
    (metadata.len(), metadata.modified().ok())
}

#[cfg(windows)]
fn metadata_is_unsafe(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn metadata_is_unsafe(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn input_failure(error: RepositoryBoundaryInputError) -> RepositoryBoundaryFailure {
    match error {
        RepositoryBoundaryInputError::Invalid(reason) => invalid_manifest(reason),
        RepositoryBoundaryInputError::Limit(error) => RepositoryBoundaryFailure {
            error: CodeNoesisErrorV9::from_boundary(&error),
            exit_code: 10,
        },
    }
}

fn invalid_manifest(reason: BoundaryManifestReason) -> RepositoryBoundaryFailure {
    RepositoryBoundaryFailure {
        error: CodeNoesisErrorV9::invalid_manifest(reason),
        exit_code: 2,
    }
}

fn limit_failure(observed: u64) -> RepositoryBoundaryFailure {
    RepositoryBoundaryFailure {
        error: codenoesis_contracts::manifest_limit_error(
            BoundaryLimit::BoundaryManifestBytes,
            observed,
        ),
        exit_code: 10,
    }
}

fn nested_path_failure(path: &str) -> RepositoryBoundaryFailure {
    RepositoryBoundaryFailure {
        error: CodeNoesisErrorV9::nested_unavailable(
            path,
            NestedRepositoryUnavailableReason::PathInvalid,
        ),
        exit_code: 10,
    }
}
