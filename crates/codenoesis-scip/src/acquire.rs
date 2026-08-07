use std::fs::{self, File, Metadata};
use std::io::{Read as _, Seek as _};
use std::path::{Component, Path, PathBuf};

use codenoesis_domain::s4_r7::{
    CompilerIndexError, CompilerIndexLimit, CompilerIndexMismatchSubject,
    MAX_R7_BINDING_JSON_BYTES, MAX_R7_RAW_INDEX_BYTES, compiler_index_limit_exceeded,
};
#[cfg(windows)]
use same_file::Handle as FileIdentity;
use sha2::{Digest as _, Sha256};

use crate::binding::artifact_relative_path;

const IMMUTABLE_READ_BUFFER_BYTES: usize = 8 * 1_024;

pub(crate) struct AcquiredInput {
    pub(crate) path: String,
    pub(crate) bytes: Vec<u8>,
    pub(crate) sha256: String,
}

pub(crate) struct AcquiredPair {
    pub(crate) binding: AcquiredInput,
    pub(crate) artifact: AcquiredInput,
}

pub(crate) fn acquire_pair(binding_path: &Path) -> Result<AcquiredPair, CompilerIndexError> {
    let binding_path = validate_relative_path(binding_path)?;
    let binding = read_immutable(
        &binding_path,
        MAX_R7_BINDING_JSON_BYTES,
        CompilerIndexLimit::BindingJsonBytes,
    )?;
    let relative_artifact = artifact_relative_path(&binding.path, &binding.bytes)?;
    let artifact_path = PathBuf::from(relative_artifact);
    let artifact_path = validate_relative_path(&artifact_path)?;
    let artifact = read_immutable(
        &artifact_path,
        MAX_R7_RAW_INDEX_BYTES,
        CompilerIndexLimit::RawIndexBytes,
    )?;
    Ok(AcquiredPair { binding, artifact })
}

fn validate_relative_path(path: &Path) -> Result<PathBuf, CompilerIndexError> {
    let display = path.to_string_lossy().into_owned();
    if display.is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(CompilerIndexError::UnsafePath {
            path: bounded_path(&display),
            reason: "path_must_be_safe_relative".to_owned(),
        });
    }
    let mut current = PathBuf::new();
    for component in path.components() {
        if component == Component::CurDir {
            continue;
        }
        current.push(component);
        let metadata =
            fs::symlink_metadata(&current).map_err(|_| CompilerIndexError::UnsafePath {
                path: bounded_path(&display),
                reason: "path_is_not_readable".to_owned(),
            })?;
        if metadata.file_type().is_symlink() {
            return Err(CompilerIndexError::UnsafePath {
                path: bounded_path(&display),
                reason: "symlink_not_allowed".to_owned(),
            });
        }
    }
    Ok(path.to_path_buf())
}

fn read_immutable(
    path: &Path,
    maximum: u64,
    limit: CompilerIndexLimit,
) -> Result<AcquiredInput, CompilerIndexError> {
    read_immutable_with(path, maximum, limit, || {})
}

fn read_immutable_with(
    path: &Path,
    maximum: u64,
    limit: CompilerIndexLimit,
    after_read: impl FnOnce(),
) -> Result<AcquiredInput, CompilerIndexError> {
    let display = path.to_string_lossy().into_owned();
    let canonical_root = fs::canonicalize(".").map_err(|_| CompilerIndexError::UnsafePath {
        path: bounded_path(&display),
        reason: "working_directory_unavailable".to_owned(),
    })?;
    let canonical_path = fs::canonicalize(path).map_err(|_| CompilerIndexError::UnsafePath {
        path: bounded_path(&display),
        reason: "path_is_not_readable".to_owned(),
    })?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err(CompilerIndexError::UnsafePath {
            path: bounded_path(&display),
            reason: "path_escapes_working_directory".to_owned(),
        });
    }

    let mut file = File::open(path).map_err(|_| CompilerIndexError::UnsafePath {
        path: bounded_path(&display),
        reason: "path_is_not_readable".to_owned(),
    })?;
    let before = file
        .metadata()
        .map_err(|_| CompilerIndexError::UnsafePath {
            path: bounded_path(&display),
            reason: "metadata_unavailable".to_owned(),
        })?;
    if !before.is_file() {
        return Err(CompilerIndexError::UnsafePath {
            path: bounded_path(&display),
            reason: "regular_file_required".to_owned(),
        });
    }
    if before.len() > maximum {
        return Err(compiler_index_limit_exceeded(limit, before.len()));
    }
    #[cfg(windows)]
    let identity = capture_file_identity(&file, &display)?;

    let capacity = usize::try_from(before.len()).unwrap_or(0).saturating_add(1);
    let mut bytes = Vec::with_capacity(capacity);
    file.by_ref()
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| CompilerIndexError::UnsafePath {
            path: bounded_path(&display),
            reason: "read_failed".to_owned(),
        })?;
    let observed = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if observed > maximum {
        return Err(compiler_index_limit_exceeded(limit, observed));
    }
    let expected_sha256 = lower_hex(&Sha256::digest(&bytes));
    after_read();
    let verification = verify_file_bytes(&mut file, &bytes, &display)?;
    if !verification.matches {
        return Err(CompilerIndexError::BindingMismatch {
            subject: CompilerIndexMismatchSubject::Artifact,
            expected_sha256,
            observed_sha256: verification.observed_sha256,
        });
    }
    let after = file
        .metadata()
        .map_err(|_| CompilerIndexError::UnsafePath {
            path: bounded_path(&display),
            reason: "metadata_unavailable".to_owned(),
        })?;
    let path_after = fs::metadata(path).map_err(|_| CompilerIndexError::UnsafePath {
        path: bounded_path(&display),
        reason: "path_changed_during_read".to_owned(),
    })?;
    #[cfg(windows)]
    let path_identity_matches =
        FileIdentity::from_path(path).is_ok_and(|path_identity| path_identity == identity);
    #[cfg(not(windows))]
    let path_identity_matches = true;
    if before.len() != observed
        || !same_file_metadata(&before, &after)
        || !same_file_metadata(&after, &path_after)
        || !path_identity_matches
    {
        return Err(CompilerIndexError::BindingMismatch {
            subject: CompilerIndexMismatchSubject::Artifact,
            expected_sha256: metadata_sha256(&before),
            observed_sha256: metadata_sha256(&path_after),
        });
    }

    Ok(AcquiredInput {
        path: display,
        sha256: expected_sha256,
        bytes,
    })
}

struct ByteVerification {
    matches: bool,
    observed_sha256: String,
}

fn verify_file_bytes(
    file: &mut File,
    expected: &[u8],
    display: &str,
) -> Result<ByteVerification, CompilerIndexError> {
    file.rewind().map_err(|_| read_failure(display))?;
    let mut buffer = [0_u8; IMMUTABLE_READ_BUFFER_BYTES];
    let mut observed_sha256 = Sha256::new();
    let mut offset = 0;
    let mut matches = true;

    while offset < expected.len() {
        let remaining = expected.len() - offset;
        let requested = remaining.min(buffer.len());
        let read = read_retrying_interrupts(file, &mut buffer[..requested], display)?;
        if read == 0 {
            matches = false;
            break;
        }
        observed_sha256.update(&buffer[..read]);
        if expected[offset..offset + read] != buffer[..read] {
            matches = false;
        }
        offset += read;
    }

    let mut trailing = [0_u8; 1];
    let trailing_read = read_retrying_interrupts(file, &mut trailing, display)?;
    if trailing_read != 0 {
        observed_sha256.update(&trailing[..trailing_read]);
        matches = false;
    }

    Ok(ByteVerification {
        matches: matches && offset == expected.len(),
        observed_sha256: lower_hex(&observed_sha256.finalize()),
    })
}

fn read_retrying_interrupts(
    file: &mut File,
    buffer: &mut [u8],
    display: &str,
) -> Result<usize, CompilerIndexError> {
    loop {
        match file.read(buffer) {
            Ok(read) => return Ok(read),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return Err(read_failure(display)),
        }
    }
}

fn read_failure(display: &str) -> CompilerIndexError {
    CompilerIndexError::UnsafePath {
        path: bounded_path(display),
        reason: "read_failed".to_owned(),
    }
}

#[cfg(windows)]
fn capture_file_identity(file: &File, display: &str) -> Result<FileIdentity, CompilerIndexError> {
    let clone = file
        .try_clone()
        .map_err(|_| CompilerIndexError::UnsafePath {
            path: bounded_path(display),
            reason: "metadata_unavailable".to_owned(),
        })?;
    FileIdentity::from_file(clone).map_err(|_| CompilerIndexError::UnsafePath {
        path: bounded_path(display),
        reason: "metadata_unavailable".to_owned(),
    })
}

#[cfg(unix)]
fn same_file_metadata(left: &Metadata, right: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
}

#[cfg(unix)]
fn metadata_sha256(metadata: &Metadata) -> String {
    use std::os::unix::fs::MetadataExt as _;

    lower_hex(&Sha256::digest(
        format!(
            "{}:{}:{}:{}:{}",
            metadata.dev(),
            metadata.ino(),
            metadata.len(),
            metadata.mtime(),
            metadata.mtime_nsec()
        )
        .as_bytes(),
    ))
}

#[cfg(not(unix))]
fn same_file_metadata(left: &Metadata, right: &Metadata) -> bool {
    left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
        && left.file_type() == right.file_type()
}

#[cfg(not(unix))]
fn metadata_sha256(metadata: &Metadata) -> String {
    lower_hex(&Sha256::digest(
        format!(
            "{}:{:?}:{:?}",
            metadata.len(),
            metadata.modified().ok(),
            metadata.file_type()
        )
        .as_bytes(),
    ))
}

fn lower_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn bounded_path(path: &str) -> String {
    path.chars().take(4_096).collect()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use codenoesis_domain::s4_r7::{
        CompilerIndexError, CompilerIndexLimit, CompilerIndexMismatchSubject,
        MAX_R7_RAW_INDEX_BYTES,
    };
    use sha2::{Digest as _, Sha256};

    use super::{lower_hex, read_immutable_with};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn race_fr_ext_005_mutable_artifact_read_is_binding_mismatch() {
        for iteration in 0..64 {
            let path = unique_path("same-length-rewrite");
            fs::create_dir_all(path.parent().expect("race fixture parent"))
                .expect("create race fixture parent");
            fs::write(&path, b"original").expect("write race fixture");
            let result = read_immutable_with(
                &path,
                MAX_R7_RAW_INDEX_BYTES,
                CompilerIndexLimit::RawIndexBytes,
                || fs::write(&path, b"replaced").expect("replace race fixture"),
            );
            let _ = fs::remove_file(&path);
            assert!(
                matches!(
                    result,
                    Err(CompilerIndexError::BindingMismatch {
                        subject: CompilerIndexMismatchSubject::Artifact,
                        ..
                    })
                ),
                "same-length rewrite iteration {iteration} was accepted"
            );
        }
    }

    #[test]
    fn race_fr_ext_005_path_replacement_is_binding_mismatch() {
        let root = unique_path("path-replacement");
        fs::create_dir_all(&root).expect("create path replacement root");
        let path = root.join("artifact.scip");
        let archived = root.join("artifact.archived");
        let replacement = root.join("artifact.replacement");
        fs::write(&path, b"original").expect("write path replacement fixture");
        fs::write(&replacement, b"original").expect("write same-content replacement");
        let result = read_immutable_with(
            &path,
            MAX_R7_RAW_INDEX_BYTES,
            CompilerIndexLimit::RawIndexBytes,
            || {
                fs::rename(&path, &archived).expect("archive opened artifact");
                fs::rename(&replacement, &path).expect("install replacement artifact");
            },
        );
        let _ = fs::remove_dir_all(&root);
        assert!(matches!(
            result,
            Err(CompilerIndexError::BindingMismatch {
                subject: CompilerIndexMismatchSubject::Artifact,
                ..
            })
        ));
    }

    #[test]
    fn reg_fr_ext_005_unchanged_artifact_preserves_bytes_and_digest() {
        let path = unique_path("unchanged");
        fs::create_dir_all(path.parent().expect("unchanged fixture parent"))
            .expect("create unchanged fixture parent");
        let expected = b"unchanged";
        fs::write(&path, expected).expect("write unchanged fixture");
        let acquired = read_immutable_with(
            &path,
            MAX_R7_RAW_INDEX_BYTES,
            CompilerIndexLimit::RawIndexBytes,
            || {},
        )
        .expect("read unchanged artifact");
        let _ = fs::remove_file(&path);

        assert_eq!(acquired.bytes, expected);
        assert_eq!(acquired.sha256, lower_hex(&Sha256::digest(expected)));
    }

    fn unique_path(scenario: &str) -> PathBuf {
        PathBuf::from("target").join(format!(
            "codenoesis-r7-{scenario}-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }
}
