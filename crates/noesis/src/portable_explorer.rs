use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{Read as _, Seek as _, Write as _};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use codenoesis_contracts::{
    LocalExplorerManifestV1, MAX_R8_PORTABLE_GRAPH_BYTES, PortableGraphV1, R8_EXPLORER_MARKER,
    R8_PORTABLE_MARKER, R8ContractError,
};
#[cfg(any(unix, windows))]
use same_file::Handle as FileIdentity;
use sha2::{Digest as _, Sha256};

const PORTABLE_MARKER_BYTES: &[u8] =
    b"{\"schema_version\":\"codenoesis.portable-graph-marker/v1\"}\n";
const EXPLORER_MARKER_BYTES: &[u8] =
    b"{\"schema_version\":\"codenoesis.local-explorer-marker/v1\"}\n";
const VIEWER_SHA256: &str = "1caa2c0ca5675937eab674f61681883ba3c6a428feb6b1baa744a0cb7eecd044";
const VIEWER_BYTES: &[u8] = include_bytes!("../assets/s4/r8/index.html");
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PortableExplorerError {
    UnsafeOutput {
        path_sha256: String,
        reason: &'static str,
    },
    Contract(R8ContractError),
    Internal,
}

impl From<R8ContractError> for PortableExplorerError {
    fn from(error: R8ContractError) -> Self {
        Self::Contract(error)
    }
}

/// Validates an export destination without creating or changing it.
///
/// # Errors
///
/// Returns a typed error for aliasing, symlinks, escapes, or invalid ownership.
pub fn validate_export_output_root(
    store: &Path,
    output: &Path,
) -> Result<(), PortableExplorerError> {
    validate_output_root(store, output, OutputKind::Portable)
}

/// Creates an absent export destination after complete validation.
///
/// # Errors
///
/// Returns a typed error when validation or directory creation fails.
pub fn ensure_export_output_root_for_boundary(
    store: &Path,
    output: &Path,
) -> Result<(), PortableExplorerError> {
    ensure_output_root(store, output, OutputKind::Portable)
}

/// Publishes one validated portable graph into its marker-owned destination.
///
/// # Errors
///
/// Returns a typed error for integrity, ownership, race, or I/O failures.
pub fn publish_portable_graph(
    output: &Path,
    portable: &PortableGraphV1,
) -> Result<Vec<u8>, PortableExplorerError> {
    let bytes = portable.canonical_file();
    publish_files(
        output,
        OutputKind::Portable,
        &[OwnedFile::new("portable-graph.json", bytes.clone())],
    )?;
    Ok(bytes)
}

/// Acquires and strictly validates one immutable portable graph input.
///
/// # Errors
///
/// Returns a typed error for unsafe paths, input races, limits, or contract failures.
pub fn read_portable_graph(
    input: &Path,
) -> Result<(PortableGraphV1, Vec<u8>), PortableExplorerError> {
    read_portable_graph_with(input, || {})
}

fn read_portable_graph_with(
    input: &Path,
    after_first_read: impl FnOnce(),
) -> Result<(PortableGraphV1, Vec<u8>), PortableExplorerError> {
    let input = absolute_without_parent_components(input).map_err(|reason| {
        PortableExplorerError::Contract(R8ContractError::InvalidProjection {
            projection_sha256: sha256(input.as_os_str().as_encoded_bytes()),
            reason,
        })
    })?;
    verify_existing_components(&input)?;
    let path_metadata = fs::symlink_metadata(&input).map_err(|_| invalid_input(&input))?;
    if !path_metadata.is_file() || unsafe_metadata(&path_metadata) {
        return Err(invalid_input(&input));
    }
    let mut file = File::open(&input).map_err(|_| invalid_input(&input))?;
    let before = file.metadata().map_err(|_| invalid_input(&input))?;
    #[cfg(any(unix, windows))]
    let identity = FileIdentity::from_file(file.try_clone().map_err(|_| invalid_input(&input))?)
        .map_err(|_| invalid_input(&input))?;
    let maximum = MAX_R8_PORTABLE_GRAPH_BYTES;
    if before.len() > maximum {
        return Err(R8ContractError::LimitExceeded {
            limit: "portable_graph_bytes",
            maximum,
            observed: before.len(),
        }
        .into());
    }
    let capacity = usize::try_from(before.len()).map_err(|_| R8ContractError::LimitExceeded {
        limit: "portable_graph_bytes",
        maximum,
        observed: u64::MAX,
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    std::io::Read::by_ref(&mut file)
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| invalid_input(&input))?;
    let observed = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if observed > maximum {
        return Err(R8ContractError::LimitExceeded {
            limit: "portable_graph_bytes",
            maximum,
            observed,
        }
        .into());
    }
    after_first_read();
    verify_open_file_bytes(&mut file, &bytes).map_err(|_| invalid_input(&input))?;
    let after = file.metadata().map_err(|_| invalid_input(&input))?;
    let path_after = fs::symlink_metadata(&input).map_err(|_| invalid_input(&input))?;
    #[cfg(any(unix, windows))]
    let path_identity_matches =
        FileIdentity::from_path(&input).is_ok_and(|path_identity| path_identity == identity);
    #[cfg(not(any(unix, windows)))]
    let path_identity_matches = true;
    if before.len() != observed
        || unsafe_metadata(&path_after)
        || !same_file_metadata(&before, &after)
        || !same_file_metadata(&after, &path_after)
        || !path_identity_matches
    {
        return Err(invalid_input(&input));
    }
    let portable = PortableGraphV1::from_canonical_file(&bytes, sha256)?;
    Ok((portable, bytes))
}

fn verify_open_file_bytes(file: &mut File, expected: &[u8]) -> std::io::Result<()> {
    file.rewind()?;
    let mut buffer = [0_u8; 16 * 1024];
    let mut offset = 0;
    while offset < expected.len() {
        let requested = (expected.len() - offset).min(buffer.len());
        let read = read_retrying_interrupts(file, &mut buffer[..requested])?;
        if read == 0 || expected[offset..offset + read] != buffer[..read] {
            return Err(std::io::Error::other("portable input changed"));
        }
        offset += read;
    }
    if read_retrying_interrupts(file, &mut buffer[..1])? == 0 {
        Ok(())
    } else {
        Err(std::io::Error::other("portable input changed"))
    }
}

fn read_retrying_interrupts(file: &mut File, buffer: &mut [u8]) -> std::io::Result<usize> {
    loop {
        match file.read(buffer) {
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            result => return result,
        }
    }
}

/// Validates an explorer destination without creating or changing it.
///
/// # Errors
///
/// Returns a typed error for aliasing, symlinks, escapes, or invalid ownership.
pub fn validate_explorer_output_root(
    input: &Path,
    output: &Path,
) -> Result<(), PortableExplorerError> {
    validate_output_root(input, output, OutputKind::Explorer)
}

/// Creates an absent explorer destination after complete validation.
///
/// # Errors
///
/// Returns a typed error when validation or directory creation fails.
pub fn ensure_explorer_output_root_for_boundary(
    input: &Path,
    output: &Path,
) -> Result<(), PortableExplorerError> {
    ensure_output_root(input, output, OutputKind::Explorer)
}

/// Publishes the reviewed static explorer bound to one validated portable graph.
///
/// # Errors
///
/// Returns a typed error for asset, integrity, ownership, race, or I/O failures.
pub fn publish_local_explorer(
    output: &Path,
    portable: &PortableGraphV1,
    portable_bytes: &[u8],
) -> Result<Vec<u8>, PortableExplorerError> {
    let manifest =
        LocalExplorerManifestV1::new(portable, portable_bytes, VIEWER_BYTES, VIEWER_SHA256)?;
    let manifest_bytes = manifest
        .canonical_file()
        .map_err(|_| PortableExplorerError::Internal)?;
    publish_files(
        output,
        OutputKind::Explorer,
        &[
            OwnedFile::new("portable-graph.json", portable_bytes.to_vec()),
            OwnedFile::new("index.html", VIEWER_BYTES.to_vec()),
            OwnedFile::new("explorer-manifest.json", manifest_bytes.clone()),
        ],
    )?;
    Ok(manifest_bytes)
}

#[must_use]
pub fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing SHA-256 hex cannot fail");
    }
    output
}

fn validate_output_root(
    authority: &Path,
    requested: &Path,
    kind: OutputKind,
) -> Result<(), PortableExplorerError> {
    let authority = absolute_without_parent_components(authority)
        .map_err(|reason| unsafe_output(authority.as_os_str(), reason))?;
    let output = absolute_without_parent_components(requested)
        .map_err(|reason| unsafe_output(requested.as_os_str(), reason))?;
    verify_existing_components(&authority)?;
    let authority = fs::canonicalize(&authority)
        .map_err(|_| unsafe_output(authority.as_os_str(), "outside_destination"))?;
    let parent = output
        .parent()
        .ok_or_else(|| unsafe_output(output.as_os_str(), "outside_destination"))?;
    verify_existing_components(parent)?;
    let canonical_parent = fs::canonicalize(parent)
        .map_err(|_| unsafe_output(parent.as_os_str(), "outside_destination"))?;
    let canonical_output = match fs::symlink_metadata(&output) {
        Ok(metadata) => {
            if !metadata.is_dir() || unsafe_metadata(&metadata) {
                return Err(unsafe_output(output.as_os_str(), "symlink_escape"));
            }
            verify_existing_components(&output)?;
            fs::canonicalize(&output)
                .map_err(|_| unsafe_output(output.as_os_str(), "outside_destination"))?
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => canonical_parent.join(
            output
                .file_name()
                .ok_or_else(|| unsafe_output(output.as_os_str(), "outside_destination"))?,
        ),
        Err(_) => return Err(unsafe_output(output.as_os_str(), "outside_destination")),
    };
    if canonical_output == authority
        || canonical_output.starts_with(&authority)
        || authority.starts_with(&canonical_output)
    {
        return Err(unsafe_output(output.as_os_str(), "outside_destination"));
    }
    inspect_output(&output, kind)
}

fn ensure_output_root(
    authority: &Path,
    output: &Path,
    kind: OutputKind,
) -> Result<(), PortableExplorerError> {
    validate_output_root(authority, output, kind)?;
    match fs::symlink_metadata(output) {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(output).map_err(|_| PortableExplorerError::Internal)?;
            sync_directory(output.parent().ok_or(PortableExplorerError::Internal)?)
        }
        Err(_) => Err(PortableExplorerError::Internal),
    }
}

fn inspect_output(root: &Path, kind: OutputKind) -> Result<(), PortableExplorerError> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| PortableExplorerError::Internal)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(PortableExplorerError::Internal),
    };
    if entries.is_empty() {
        return Ok(());
    }
    let marker_path = root.join(kind.marker());
    let Ok(marker) = fs::read(&marker_path) else {
        return Err(unsafe_output(root.as_os_str(), "non_empty_unmarked"));
    };
    if marker != kind.marker_bytes() {
        return Err(unsafe_output(root.as_os_str(), "marker_mismatch"));
    }
    let allowed = kind.allowed_names();
    for entry in entries {
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| unsafe_output(root.as_os_str(), "outside_destination"))?;
        if !allowed.contains(name.as_str()) {
            return Err(unsafe_output(root.as_os_str(), "outside_destination"));
        }
        verify_regular_file(&entry.path())?;
    }
    Ok(())
}

fn publish_files(
    root: &Path,
    kind: OutputKind,
    files: &[OwnedFile],
) -> Result<(), PortableExplorerError> {
    publish_files_with(root, kind, files, || {})
}

fn publish_files_with(
    root: &Path,
    kind: OutputKind,
    files: &[OwnedFile],
    before_publication: impl FnOnce(),
) -> Result<(), PortableExplorerError> {
    verify_directory(root)?;
    let guard = PublicationGuard::capture(root)?;
    before_publication();
    guard.verify()?;
    let marker = root.join(kind.marker());
    let fresh = fs::read_dir(root)
        .map_err(|_| PortableExplorerError::Internal)?
        .next()
        .is_none();
    if fresh {
        guard.verify()?;
        write_exclusive(&marker, kind.marker_bytes())?;
        guard.verify()?;
        sync_directory(root)?;
    } else {
        verify_exact_file(&marker, kind.marker_bytes())?;
    }
    let result = (|| {
        for file in files {
            guard.verify()?;
            write_atomic_or_verify(root, &file.path, &file.bytes)?;
            guard.verify()?;
        }
        sync_directory(root)?;
        guard.verify()?;
        inspect_output(root, kind)?;
        for file in files {
            verify_exact_file(&root.join(&file.path), &file.bytes)?;
        }
        Ok(())
    })();
    if result.is_err() && fresh && guard.verify().is_ok() {
        for file in files.iter().rev() {
            let _ = fs::remove_file(root.join(&file.path));
        }
        let _ = fs::remove_file(marker);
        let _ = sync_directory(root);
    }
    result
}

struct PublicationGuard {
    root: PathBuf,
    parent: PathBuf,
    #[cfg(not(any(unix, windows)))]
    root_metadata: Metadata,
    #[cfg(not(any(unix, windows)))]
    parent_metadata: Metadata,
    #[cfg(any(unix, windows))]
    root_identity: FileIdentity,
    #[cfg(any(unix, windows))]
    parent_identity: FileIdentity,
}

impl PublicationGuard {
    fn capture(root: &Path) -> Result<Self, PortableExplorerError> {
        let parent = root
            .parent()
            .ok_or(PortableExplorerError::Internal)?
            .to_path_buf();
        let root_metadata = fs::symlink_metadata(root)
            .map_err(|_| unsafe_output(root.as_os_str(), "outside_destination"))?;
        let parent_metadata = fs::symlink_metadata(&parent)
            .map_err(|_| unsafe_output(parent.as_os_str(), "outside_destination"))?;
        if unsafe_metadata(&root_metadata) || unsafe_metadata(&parent_metadata) {
            return Err(unsafe_output(root.as_os_str(), "outside_destination"));
        }
        #[cfg(any(unix, windows))]
        let root_identity = FileIdentity::from_path(root)
            .map_err(|_| unsafe_output(root.as_os_str(), "outside_destination"))?;
        #[cfg(any(unix, windows))]
        let parent_identity = FileIdentity::from_path(&parent)
            .map_err(|_| unsafe_output(parent.as_os_str(), "outside_destination"))?;
        Ok(Self {
            root: root.to_path_buf(),
            parent,
            #[cfg(not(any(unix, windows)))]
            root_metadata,
            #[cfg(not(any(unix, windows)))]
            parent_metadata,
            #[cfg(any(unix, windows))]
            root_identity,
            #[cfg(any(unix, windows))]
            parent_identity,
        })
    }

    fn verify(&self) -> Result<(), PortableExplorerError> {
        verify_existing_components(&self.root)?;
        let root_metadata = fs::symlink_metadata(&self.root)
            .map_err(|_| unsafe_output(self.root.as_os_str(), "outside_destination"))?;
        let parent_metadata = fs::symlink_metadata(&self.parent)
            .map_err(|_| unsafe_output(self.parent.as_os_str(), "outside_destination"))?;
        #[cfg(any(unix, windows))]
        let identity_matches = FileIdentity::from_path(&self.root)
            .is_ok_and(|identity| identity == self.root_identity)
            && FileIdentity::from_path(&self.parent)
                .is_ok_and(|identity| identity == self.parent_identity);
        #[cfg(not(any(unix, windows)))]
        let identity_matches = same_path_identity(&self.root_metadata, &root_metadata)
            && same_path_identity(&self.parent_metadata, &parent_metadata);
        if unsafe_metadata(&root_metadata) || unsafe_metadata(&parent_metadata) || !identity_matches
        {
            return Err(unsafe_output(self.root.as_os_str(), "outside_destination"));
        }
        Ok(())
    }
}

fn write_atomic_or_verify(
    root: &Path,
    relative: &str,
    bytes: &[u8],
) -> Result<(), PortableExplorerError> {
    let destination = root.join(relative);
    match fs::symlink_metadata(&destination) {
        Ok(_) => return verify_exact_file(&destination, bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err(PortableExplorerError::Internal),
    }
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temporary = root.join(format!(
        ".codenoesis-r8-{}-{sequence}.tmp",
        std::process::id()
    ));
    let write = (|| {
        write_exclusive(&temporary, bytes)?;
        verify_exact_file(&temporary, bytes)?;
        fs::rename(&temporary, &destination).map_err(|_| PortableExplorerError::Internal)?;
        verify_exact_file(&destination, bytes)
    })();
    if write.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    write
}

fn write_exclusive(path: &Path, bytes: &[u8]) -> Result<(), PortableExplorerError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| PortableExplorerError::Internal)?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| PortableExplorerError::Internal)
}

fn verify_exact_file(path: &Path, expected: &[u8]) -> Result<(), PortableExplorerError> {
    verify_regular_file(path)?;
    let observed = fs::read(path).map_err(|_| PortableExplorerError::Internal)?;
    if observed == expected {
        Ok(())
    } else {
        Err(unsafe_output(path.as_os_str(), "marker_mismatch"))
    }
}

fn verify_regular_file(path: &Path) -> Result<(), PortableExplorerError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| unsafe_output(path.as_os_str(), "outside_destination"))?;
    if metadata.is_file() && !unsafe_metadata(&metadata) {
        Ok(())
    } else {
        Err(unsafe_output(path.as_os_str(), "symlink_escape"))
    }
}

fn verify_directory(path: &Path) -> Result<(), PortableExplorerError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| unsafe_output(path.as_os_str(), "outside_destination"))?;
    if metadata.is_dir() && !unsafe_metadata(&metadata) {
        Ok(())
    } else {
        Err(unsafe_output(path.as_os_str(), "symlink_escape"))
    }
}

fn verify_existing_components(path: &Path) -> Result<(), PortableExplorerError> {
    let absolute = absolute_without_parent_components(path)
        .map_err(|reason| unsafe_output(path.as_os_str(), reason))?;
    let mut current = PathBuf::new();
    for component in absolute.components() {
        current.push(component.as_os_str());
        let metadata = fs::symlink_metadata(&current)
            .map_err(|_| unsafe_output(path.as_os_str(), "outside_destination"))?;
        if unsafe_metadata(&metadata) {
            return Err(unsafe_output(path.as_os_str(), "symlink_escape"));
        }
    }
    Ok(())
}

fn absolute_without_parent_components(path: &Path) -> Result<PathBuf, &'static str> {
    if path.as_os_str().is_empty() {
        return Err("outside_destination");
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err("parent_escape");
    }
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::env::current_dir()
            .map(|current| current.join(path))
            .map_err(|_| "outside_destination")
    }
}

fn unsafe_output(path: &OsStr, reason: &'static str) -> PortableExplorerError {
    PortableExplorerError::UnsafeOutput {
        path_sha256: sha256(path.as_encoded_bytes()),
        reason,
    }
}

fn invalid_input(path: &Path) -> PortableExplorerError {
    PortableExplorerError::Contract(R8ContractError::InvalidProjection {
        projection_sha256: sha256(path.as_os_str().as_encoded_bytes()),
        reason: "unsafe_path",
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

#[cfg(windows)]
fn same_file_metadata(left: &Metadata, right: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    left.file_size() == right.file_size()
        && left.last_write_time() == right.last_write_time()
        && left.creation_time() == right.creation_time()
        && left.file_attributes() == right.file_attributes()
}

#[cfg(not(any(unix, windows)))]
fn same_file_metadata(left: &Metadata, right: &Metadata) -> bool {
    left.len() == right.len() && left.modified().ok() == right.modified().ok()
}

#[cfg(not(any(unix, windows)))]
fn same_path_identity(left: &Metadata, right: &Metadata) -> bool {
    left.file_type() == right.file_type()
}

#[cfg(windows)]
fn unsafe_metadata(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn unsafe_metadata(metadata: &Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(windows)]
fn sync_directory(_directory: &Path) -> Result<(), PortableExplorerError> {
    Ok(())
}

#[cfg(not(windows))]
fn sync_directory(directory: &Path) -> Result<(), PortableExplorerError> {
    File::open(directory)
        .and_then(|file| file.sync_all())
        .map_err(|_| PortableExplorerError::Internal)
}

struct OwnedFile {
    path: String,
    bytes: Vec<u8>,
}

impl OwnedFile {
    fn new(path: &str, bytes: Vec<u8>) -> Self {
        Self {
            path: path.to_owned(),
            bytes,
        }
    }
}

#[derive(Clone, Copy)]
enum OutputKind {
    Portable,
    Explorer,
}

impl OutputKind {
    const fn marker(self) -> &'static str {
        match self {
            Self::Portable => R8_PORTABLE_MARKER,
            Self::Explorer => R8_EXPLORER_MARKER,
        }
    }

    const fn marker_bytes(self) -> &'static [u8] {
        match self {
            Self::Portable => PORTABLE_MARKER_BYTES,
            Self::Explorer => EXPLORER_MARKER_BYTES,
        }
    }

    fn allowed_names(self) -> BTreeSet<&'static str> {
        match self {
            Self::Portable => BTreeSet::from([R8_PORTABLE_MARKER, "portable-graph.json"]),
            Self::Explorer => BTreeSet::from([
                R8_EXPLORER_MARKER,
                "portable-graph.json",
                "index.html",
                "explorer-manifest.json",
            ]),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::{
        OutputKind, OwnedFile, PortableExplorerError, publish_files_with, read_portable_graph_with,
    };

    const PORTABLE_FIXTURE: &[u8] =
        include_bytes!("../../../tests/fixtures/s4/portable-explorer-v1/portable-graph.json");

    #[test]
    fn race_nfr_sec_005_mutable_input_is_rejected() {
        let root = temporary_root("mutable-input");
        let input = root.join("portable-graph.json");
        fs::write(&input, PORTABLE_FIXTURE).expect("write R8 race input");
        let result = read_portable_graph_with(&input, || {
            let mut changed = PORTABLE_FIXTURE.to_vec();
            changed[0] ^= 1;
            fs::write(&input, changed).expect("rewrite R8 race input");
        });
        assert!(matches!(result, Err(PortableExplorerError::Contract(_))));
        fs::remove_dir_all(root).expect("remove R8 race fixture");
    }

    #[test]
    fn race_nfr_sec_005_output_replacement_is_rejected_or_blocked() {
        let parent = temporary_root("output-replacement");
        let output = parent.join("output");
        let attacker = parent.join("attacker");
        let displaced = parent.join("displaced");
        fs::create_dir(&output).expect("create R8 selected output");
        fs::create_dir(&attacker).expect("create R8 attacker output");
        let replaced = Cell::new(false);
        let result = publish_files_with(
            &output,
            OutputKind::Portable,
            &[OwnedFile::new(
                "portable-graph.json",
                b"reviewed\n".to_vec(),
            )],
            || {
                if fs::rename(&output, &displaced).is_ok() {
                    if fs::rename(&attacker, &output).is_ok() {
                        replaced.set(true);
                    } else {
                        fs::rename(&displaced, &output)
                            .expect("restore blocked R8 output replacement");
                    }
                }
            },
        );
        if replaced.get() {
            assert!(matches!(
                result,
                Err(PortableExplorerError::UnsafeOutput { .. })
            ));
            assert!(directory_is_empty(&output));
            assert!(directory_is_empty(&displaced));
        } else {
            assert!(result.is_ok());
            assert!(output.join("portable-graph.json").is_file());
        }
        fs::remove_dir_all(parent).expect("remove R8 output race fixture");
    }

    fn temporary_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "codenoesis-r8-unit-{label}-{}-{}",
            std::process::id(),
            super::TEMP_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("create R8 unit root");
        fs::canonicalize(root).expect("canonicalize R8 unit root")
    }

    fn directory_is_empty(path: &Path) -> bool {
        fs::read_dir(path)
            .expect("read R8 race directory")
            .next()
            .is_none()
    }
}
