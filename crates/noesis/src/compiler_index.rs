use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::Read as _;
use std::path::{Component, Path, PathBuf};

use codenoesis_domain::s4_r7::{
    CompilerIndexError, CompilerIndexLimit, MAX_R7_BINDING_JSON_BYTES,
    compiler_index_limit_exceeded,
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
    crate::filesystem_sandbox::install_with_compiler_index(
        repository,
        &input.binding_path,
        &input.artifact_path,
    )
    .map_err(|_| ())
}

#[cfg(target_os = "linux")]
pub(crate) fn install_boundary_scan_boundary(
    repository: &OsStr,
    manifest: Option<&OsStr>,
    nested_roots: &[PathBuf],
    input: &PreparedCompilerIndex,
) -> Result<(), ()> {
    crate::filesystem_sandbox::install_with_compiler_index_and_roots(
        repository,
        &input.binding_path,
        &input.artifact_path,
        manifest,
        nested_roots,
    )
    .map_err(|_| ())
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
