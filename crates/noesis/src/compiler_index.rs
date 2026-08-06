use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::Read as _;
use std::path::{Component, Path, PathBuf};

use codenoesis_domain::s4_r7::{
    CompilerIndexError, CompilerIndexLimit, MAX_R7_BINDING_JSON_BYTES,
    compiler_index_limit_exceeded,
};
#[cfg(target_os = "linux")]
use landlock::{
    ABI, Access, AccessFs, CompatLevel, Compatible, PathBeneath, PathFd, Ruleset, RulesetAttr,
    RulesetCreatedAttr, RulesetStatus,
};

#[derive(Clone)]
pub(crate) struct PreparedCompilerIndex {
    pub(crate) binding_path: PathBuf,
    pub(crate) artifact_path: PathBuf,
}

pub(crate) fn prepare(binding: &OsStr) -> Result<PreparedCompilerIndex, CompilerIndexError> {
    let binding_path = safe_relative_path(Path::new(binding))?;
    reject_symlinks(&binding_path)?;
    let bytes = read_bounded_binding(&binding_path)?;
    let artifact = serde_json::from_slice::<serde_json::Value>(&bytes)
        .ok()
        .and_then(|value| {
            value
                .pointer("/artifact/path")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .ok_or_else(|| CompilerIndexError::InvalidBinding {
            path: bounded_path(&binding_path),
            reason: "artifact_path_missing".to_owned(),
        })?;
    let artifact_relative = safe_relative_path(Path::new(&artifact))?;
    if artifact_relative.components().count() != 1 {
        return Err(CompilerIndexError::UnsafePath {
            path: bounded_path(&artifact_relative),
            reason: "artifact_must_be_adjacent".to_owned(),
        });
    }
    let artifact_path = binding_path
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .join(artifact_relative);
    reject_symlinks(&artifact_path)?;
    let metadata = fs::metadata(&artifact_path).map_err(|_| CompilerIndexError::UnsafePath {
        path: bounded_path(&artifact_path),
        reason: "path_is_not_readable".to_owned(),
    })?;
    if !metadata.is_file() {
        return Err(CompilerIndexError::UnsafePath {
            path: bounded_path(&artifact_path),
            reason: "regular_file_required".to_owned(),
        });
    }
    Ok(PreparedCompilerIndex {
        binding_path,
        artifact_path,
    })
}

#[cfg(target_os = "linux")]
pub(crate) fn install_scan_boundary(
    repository: &OsStr,
    input: &PreparedCompilerIndex,
) -> Result<(), ()> {
    install_linux_boundary(
        repository,
        &input.binding_path,
        &input.artifact_path,
        None,
        &[],
    )
}

#[cfg(target_os = "linux")]
pub(crate) fn install_boundary_scan_boundary(
    repository: &OsStr,
    manifest: Option<&OsStr>,
    nested_roots: &[PathBuf],
    input: &PreparedCompilerIndex,
) -> Result<(), ()> {
    install_linux_boundary(
        repository,
        &input.binding_path,
        &input.artifact_path,
        manifest,
        nested_roots,
    )
}

#[cfg(target_os = "linux")]
fn install_linux_boundary(
    repository: &OsStr,
    binding: &Path,
    artifact: &Path,
    manifest: Option<&OsStr>,
    nested_roots: &[PathBuf],
) -> Result<(), ()> {
    let abi = ABI::V3;
    let read_file = AccessFs::ReadFile;
    let read_tree = read_file | AccessFs::ReadDir;
    let mut ruleset = Ruleset::default()
        .set_compatibility(CompatLevel::HardRequirement)
        .handle_access(AccessFs::from_all(abi))
        .map_err(|_| ())?
        .create()
        .map_err(|_| ())?
        .add_rule(PathBeneath::new(
            PathFd::new(Path::new(repository)).map_err(|_| ())?,
            read_tree,
        ))
        .map_err(|_| ())?
        .add_rule(PathBeneath::new(
            PathFd::new(binding).map_err(|_| ())?,
            read_file,
        ))
        .map_err(|_| ())?
        .add_rule(PathBeneath::new(
            PathFd::new(artifact).map_err(|_| ())?,
            read_file,
        ))
        .map_err(|_| ())?;
    if let Some(manifest) = manifest {
        ruleset = ruleset
            .add_rule(PathBeneath::new(
                PathFd::new(Path::new(manifest)).map_err(|_| ())?,
                read_file,
            ))
            .map_err(|_| ())?;
    }
    for root in nested_roots {
        ruleset = ruleset
            .add_rule(PathBeneath::new(
                PathFd::new(root).map_err(|_| ())?,
                read_tree,
            ))
            .map_err(|_| ())?;
    }
    let status = ruleset.restrict_self().map_err(|_| ())?;
    if status.ruleset != RulesetStatus::FullyEnforced || !status.no_new_privs {
        return Err(());
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
#[allow(clippy::unnecessary_wraps)]
pub(crate) fn install_scan_boundary(
    _repository: &OsStr,
    input: &PreparedCompilerIndex,
) -> Result<(), ()> {
    let _ = (&input.binding_path, &input.artifact_path);
    Ok(())
}

#[cfg(not(target_os = "linux"))]
#[allow(clippy::unnecessary_wraps)]
pub(crate) fn install_boundary_scan_boundary(
    _repository: &OsStr,
    _manifest: Option<&OsStr>,
    _nested_roots: &[PathBuf],
    input: &PreparedCompilerIndex,
) -> Result<(), ()> {
    let _ = (&input.binding_path, &input.artifact_path);
    Ok(())
}

fn safe_relative_path(path: &Path) -> Result<PathBuf, CompilerIndexError> {
    let display = bounded_path(path);
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(CompilerIndexError::UnsafePath {
            path: display,
            reason: "path_must_be_safe_relative".to_owned(),
        });
    }
    let normalized = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value),
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => None,
        })
        .collect::<PathBuf>();
    if normalized.as_os_str().is_empty() {
        return Err(CompilerIndexError::UnsafePath {
            path: display,
            reason: "path_must_be_safe_relative".to_owned(),
        });
    }
    Ok(normalized)
}

fn reject_symlinks(path: &Path) -> Result<(), CompilerIndexError> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component);
        let metadata =
            fs::symlink_metadata(&current).map_err(|_| CompilerIndexError::UnsafePath {
                path: bounded_path(path),
                reason: "path_is_not_readable".to_owned(),
            })?;
        if metadata.file_type().is_symlink() {
            return Err(CompilerIndexError::UnsafePath {
                path: bounded_path(path),
                reason: "symlink_not_allowed".to_owned(),
            });
        }
    }
    Ok(())
}

fn read_bounded_binding(path: &Path) -> Result<Vec<u8>, CompilerIndexError> {
    let mut file = File::open(path).map_err(|_| CompilerIndexError::UnsafePath {
        path: bounded_path(path),
        reason: "path_is_not_readable".to_owned(),
    })?;
    let metadata = file
        .metadata()
        .map_err(|_| CompilerIndexError::UnsafePath {
            path: bounded_path(path),
            reason: "metadata_unavailable".to_owned(),
        })?;
    if !metadata.is_file() {
        return Err(CompilerIndexError::UnsafePath {
            path: bounded_path(path),
            reason: "regular_file_required".to_owned(),
        });
    }
    if metadata.len() > MAX_R7_BINDING_JSON_BYTES {
        return Err(compiler_index_limit_exceeded(
            CompilerIndexLimit::BindingJsonBytes,
            metadata.len(),
        ));
    }
    let mut bytes = Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or(0));
    file.by_ref()
        .take(MAX_R7_BINDING_JSON_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| CompilerIndexError::UnsafePath {
            path: bounded_path(path),
            reason: "read_failed".to_owned(),
        })?;
    let observed = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if observed > MAX_R7_BINDING_JSON_BYTES {
        return Err(compiler_index_limit_exceeded(
            CompilerIndexLimit::BindingJsonBytes,
            observed,
        ));
    }
    Ok(bytes)
}

fn bounded_path(path: &Path) -> String {
    path.to_string_lossy().chars().take(4_096).collect()
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::install_linux_boundary;

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn sec_nfr_sec_001_r7_reads_only_repository_and_explicit_pair() {
        let root = unique_root();
        let repository = root.join("repository");
        let sidecars = root.join("sidecars");
        let binding = sidecars.join("compiler-index-binding.json");
        let artifact = sidecars.join("index.scip");
        let sibling = sidecars.join("unselected.scip");
        let outside = root.join("outside");
        fs::create_dir(&repository).expect("create R7 Landlock repository");
        fs::create_dir(&sidecars).expect("create R7 Landlock sidecar root");
        fs::write(repository.join("inside.rs"), b"pub struct Inside;\n")
            .expect("write R7 repository sentinel");
        fs::write(&binding, b"{}\n").expect("write R7 binding sentinel");
        fs::write(&artifact, b"scip\n").expect("write R7 artifact sentinel");
        fs::write(&sibling, b"unselected\n").expect("write R7 sibling sentinel");
        fs::write(&outside, b"outside\n").expect("write R7 outside sentinel");

        install_linux_boundary(repository.as_os_str(), &binding, &artifact, None, &[])
            .expect("Landlock must enforce the R7 explicit-pair boundary");

        assert_eq!(
            fs::read(repository.join("inside.rs")).expect("read R7 repository input"),
            b"pub struct Inside;\n"
        );
        assert_eq!(
            fs::read(&binding).expect("read explicit R7 binding"),
            b"{}\n"
        );
        assert_eq!(
            fs::read(&artifact).expect("read explicit R7 artifact"),
            b"scip\n"
        );
        assert_permission_denied(fs::read(&sibling));
        assert_permission_denied(fs::read(&outside));
        assert_permission_denied(fs::write(repository.join("denied"), b"denied\n"));
        assert_permission_denied(fs::write(&binding, b"denied\n"));
        assert_permission_denied(fs::write(&artifact, b"denied\n"));
    }

    fn assert_permission_denied<T>(result: std::io::Result<T>) {
        let error = result
            .map(|_| ())
            .expect_err("R7 Landlock operation must be denied");
        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    }

    fn unique_root() -> std::path::PathBuf {
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "codenoesis-r7-landlock-{}-{timestamp}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("create R7 Landlock self-test root");
        root
    }
}
