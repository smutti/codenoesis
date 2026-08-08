use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use codenoesis_domain::storage::{LOCAL_STORE_SCHEMA_VERSION, StorageError};
use tempfile::NamedTempFile;

pub const MARKER_BYTES: &[u8] = b"{\"database\":\"metadata.sqlite3\",\"objects\":\"objects\",\"schema_version\":\"codenoesis.local-store-marker/v1\",\"temporary\":\"tmp\"}\n";

pub struct PreparedStore {
    pub root: PathBuf,
    pub database: PathBuf,
    pub objects_blake3: PathBuf,
    pub temporary: PathBuf,
    pub fresh: bool,
}

pub fn prepare(
    repository_root: &Path,
    requested_store_root: &Path,
) -> Result<PreparedStore, StorageError> {
    let root = ensure_root(repository_root, requested_store_root)?;

    let fresh = if root.exists() {
        let metadata = fs::symlink_metadata(&root).map_err(|_| unsafe_path("store_root"))?;
        if !metadata.is_dir() || is_unsafe_metadata(&metadata) {
            return Err(unsafe_path("unsafe_path_component"));
        }
        classify_existing_root(&root)?
    } else {
        create_directory_noclobber(&root).map_err(|_| StorageError::PublicationFailed)?;
        sync_directory(root.parent().ok_or_else(|| unsafe_path("missing_parent"))?)?;
        true
    };

    let objects = root.join("objects");
    let objects_blake3 = objects.join("blake3");
    let temporary = root.join("tmp");
    if fresh {
        create_directory(&objects)?;
        create_directory(&objects_blake3)?;
        create_directory(&temporary)?;
        sync_directory(&root)?;
    } else {
        verify_store_directory(&objects)?;
        verify_store_directory(&objects_blake3)?;
        verify_store_directory(&temporary)?;
    }

    Ok(PreparedStore {
        database: root.join("metadata.sqlite3"),
        root,
        objects_blake3,
        temporary,
        fresh,
    })
}

pub fn prepare_existing(requested_store_root: &Path) -> Result<PreparedStore, StorageError> {
    let absolute = absolute_without_parent_components(requested_store_root)?;
    let trusted_ancestor = filesystem_namespace_anchor(&absolute)?;
    if !absolute.exists() {
        return Err(unsafe_path("store_root"));
    }
    verify_no_unsafe_components(&absolute, &trusted_ancestor)?;
    let root = fs::canonicalize(&absolute).map_err(|_| unsafe_path("store_root"))?;
    if classify_existing_root(&root)? {
        return Err(StorageError::UnmarkedNonemptyRoot);
    }
    let objects = root.join("objects");
    let objects_blake3 = objects.join("blake3");
    let temporary = root.join("tmp");
    verify_store_directory(&objects)?;
    verify_store_directory(&objects_blake3)?;
    verify_store_directory(&temporary)?;
    Ok(PreparedStore {
        database: root.join("metadata.sqlite3"),
        root,
        objects_blake3,
        temporary,
        fresh: false,
    })
}

pub fn ensure_root(
    repository_root: &Path,
    requested_store_root: &Path,
) -> Result<PathBuf, StorageError> {
    let repository_input = absolute_without_parent_components(repository_root)?;
    let store_input = absolute_without_parent_components(requested_store_root)?;
    let trusted_ancestor = common_ancestor(&repository_input, &store_input);
    let repository =
        fs::canonicalize(repository_root).map_err(|_| unsafe_path("repository_root"))?;
    let root = verified_store_root(&store_input, &trusted_ancestor)?;
    if repository.starts_with(&root) || root.starts_with(&repository) {
        return Err(unsafe_path("repository_overlap"));
    }
    if !root.exists() {
        create_directory_noclobber(&root).map_err(|_| StorageError::PublicationFailed)?;
        sync_directory(root.parent().ok_or_else(|| unsafe_path("missing_parent"))?)?;
    }
    Ok(root)
}

pub fn write_marker(prepared: &PreparedStore) -> Result<(), StorageError> {
    let mut marker =
        NamedTempFile::new_in(&prepared.root).map_err(|_| StorageError::PublicationFailed)?;
    marker
        .write_all(MARKER_BYTES)
        .and_then(|()| marker.as_file().sync_all())
        .map_err(|_| StorageError::PublicationFailed)?;
    persist_noclobber(marker, &prepared.root.join("store.json"))
        .map_err(|_| StorageError::PublicationFailed)?;
    sync_directory(&prepared.root)
}

#[cfg(windows)]
pub fn sync_directory(_directory: &Path) -> Result<(), StorageError> {
    Ok(())
}

#[cfg(not(windows))]
pub fn sync_directory(directory: &Path) -> Result<(), StorageError> {
    std::fs::File::open(directory)
        .and_then(|file| file.sync_all())
        .map_err(|_| StorageError::PublicationFailed)
}

fn verified_store_root(absolute: &Path, trusted_ancestor: &Path) -> Result<PathBuf, StorageError> {
    let (existing, missing_leaf) = if absolute.exists() {
        (absolute, None)
    } else {
        let parent = absolute
            .parent()
            .ok_or_else(|| unsafe_path("missing_parent"))?;
        if !parent.exists() {
            return Err(unsafe_path("missing_parent"));
        }
        (
            parent,
            absolute.file_name().map(std::ffi::OsStr::to_os_string),
        )
    };
    verify_no_unsafe_components(existing, trusted_ancestor)?;
    let canonical = fs::canonicalize(existing).map_err(|_| unsafe_path("store_root"))?;
    Ok(missing_leaf.map_or(canonical.clone(), |leaf| canonical.join(leaf)))
}

fn absolute_without_parent_components(path: &Path) -> Result<PathBuf, StorageError> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(unsafe_path("invalid_store_root"));
    }
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|_| unsafe_path("current_directory"))?
            .join(path)
    };
    Ok(absolute)
}

fn verify_no_unsafe_components(path: &Path, trusted_ancestor: &Path) -> Result<(), StorageError> {
    let relative = path
        .strip_prefix(trusted_ancestor)
        .map_err(|_| unsafe_path("unsafe_path_component"))?;
    let mut current = trusted_ancestor.to_path_buf();
    for component in relative.components() {
        current.push(component.as_os_str());
        let metadata =
            fs::symlink_metadata(&current).map_err(|_| unsafe_path("unsafe_path_component"))?;
        if is_unsafe_metadata(&metadata) {
            return Err(unsafe_path("unsafe_path_component"));
        }
    }
    Ok(())
}

fn common_ancestor(left: &Path, right: &Path) -> PathBuf {
    let mut common = PathBuf::new();
    for (left, right) in left.components().zip(right.components()) {
        if left != right {
            break;
        }
        common.push(left.as_os_str());
    }
    common
}

fn filesystem_namespace_anchor(path: &Path) -> Result<PathBuf, StorageError> {
    let mut anchor = PathBuf::new();
    let mut rooted = false;
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => {
                anchor.push(component.as_os_str());
                rooted = true;
            }
            Component::Normal(_) if rooted => {
                anchor.push(component.as_os_str());
                return Ok(anchor);
            }
            _ => return Err(unsafe_path("invalid_store_root")),
        }
    }
    Err(unsafe_path("invalid_store_root"))
}

fn classify_existing_root(root: &Path) -> Result<bool, StorageError> {
    let mut entries = fs::read_dir(root).map_err(|_| StorageError::PublicationFailed)?;
    if entries.next().is_none() {
        return Ok(true);
    }
    let marker = root.join("store.json");
    let metadata = fs::symlink_metadata(&marker).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            StorageError::UnmarkedNonemptyRoot
        } else {
            StorageError::PublicationFailed
        }
    })?;
    if !metadata.is_file() || is_unsafe_metadata(&metadata) {
        return Err(unsafe_path("unsafe_path_component"));
    }
    let bytes = fs::read(marker).map_err(|_| StorageError::PublicationFailed)?;
    if bytes != MARKER_BYTES {
        return Err(StorageError::IncompatibleSchema {
            observed_schema: format!("{LOCAL_STORE_SCHEMA_VERSION}-unknown-marker"),
        });
    }
    Ok(false)
}

fn create_directory(path: &Path) -> Result<(), StorageError> {
    create_directory_noclobber(path).map_err(|_| StorageError::PublicationFailed)?;
    let parent = path.parent().ok_or_else(|| unsafe_path("missing_parent"))?;
    sync_directory(parent)
}

pub(crate) fn create_directory_noclobber(path: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        let parent = path
            .parent()
            .ok_or_else(|| std::io::Error::other("directory has no parent"))?;
        let temporary = tempfile::Builder::new()
            .prefix(".codenoesis-directory-")
            .tempdir_in(parent)?;
        atomicwrites::move_atomic(temporary.path(), path)
    }
    #[cfg(not(windows))]
    {
        fs::create_dir(path)
    }
}

pub(crate) fn persist_noclobber(
    temporary: NamedTempFile,
    destination: &Path,
) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        atomicwrites::move_atomic(temporary.path(), destination)
    }
    #[cfg(not(windows))]
    {
        temporary
            .persist_noclobber(destination)
            .map(drop)
            .map_err(|error| error.error)
    }
}

fn verify_store_directory(path: &Path) -> Result<(), StorageError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| StorageError::PublicationFailed)?;
    if !metadata.is_dir() || is_unsafe_metadata(&metadata) {
        return Err(unsafe_path("unsafe_path_component"));
    }
    Ok(())
}

fn unsafe_path(reason: &'static str) -> StorageError {
    StorageError::UnsafePath { reason }
}

#[cfg(unix)]
pub(crate) fn is_unsafe_metadata(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::symlink;

    use super::*;

    #[test]
    fn sec_fr_doc_003_existing_store_ancestor_symlink_is_rejected() {
        let temporary = tempfile::tempdir().expect("temporary root");
        let actual = temporary.path().join("actual");
        let linked = temporary.path().join("linked");
        fs::create_dir(&actual).expect("actual ancestor");
        fs::create_dir(actual.join("store")).expect("store root");
        symlink(&actual, &linked).expect("linked ancestor");

        assert!(matches!(
            prepare_existing(&linked.join("store")),
            Err(StorageError::UnsafePath {
                reason: "unsafe_path_component"
            })
        ));
    }
}

#[cfg(windows)]
pub(crate) fn is_unsafe_metadata(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn is_unsafe_metadata(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}
