use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{Read as _, Seek as _, Write as _};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use codenoesis_contracts::{
    K1_EXPLORER_MARKER, K1_PORTABLE_MARKER, K1ContractError, LocalExplorerManifestV1,
    LocalExplorerManifestV2, LocalExplorerManifestV3, LocalExplorerManifestV4,
    LocalExplorerManifestV5, LocalExplorerManifestV6, LocalExplorerManifestV7,
    LocalExplorerManifestV8, LocalExplorerManifestV9, LocalExplorerManifestV10,
    MAX_K1_PORTABLE_GRAPH_BYTES, MAX_R8_PORTABLE_GRAPH_BYTES, MAX_R10_PORTABLE_GRAPH_BYTES,
    MAX_R11_PORTABLE_GRAPH_BYTES, MAX_R12_PORTABLE_GRAPH_BYTES, MAX_R13_PORTABLE_GRAPH_BYTES,
    MAX_R14_PORTABLE_GRAPH_BYTES, MAX_R15_PORTABLE_GRAPH_BYTES, MAX_R16_PORTABLE_GRAPH_BYTES,
    PortableGraphV1, PortableGraphV2, PortableGraphV3, PortableGraphV4, PortableGraphV5,
    PortableGraphV6, PortableGraphV7, PortableGraphV8, PortableGraphV9, R8_EXPLORER_MARKER,
    R8_PORTABLE_MARKER, R8ContractError, R10_EXPLORER_MARKER, R10_PORTABLE_MARKER,
    R10ContractError, R11_EXPLORER_MARKER, R11_PORTABLE_GRAPH_VERSION, R11_PORTABLE_MARKER,
    R11ContractError, R12_EXPLORER_MARKER, R12_PORTABLE_GRAPH_VERSION, R12_PORTABLE_MARKER,
    R12ContractError, R13_EXPLORER_MARKER, R13_PORTABLE_GRAPH_VERSION, R13_PORTABLE_MARKER,
    R13ContractError, R14_EXPLORER_MARKER, R14_PORTABLE_MARKER, R14ContractError,
    R15_EXPLORER_MARKER, R15_PORTABLE_MARKER, R15ContractError, R16_EXPLORER_MARKER,
    R16_PORTABLE_MARKER, R16ContractError, R17_EXPLORER_MARKER,
};
#[cfg(any(unix, windows))]
use same_file::Handle as FileIdentity;
use sha2::{Digest as _, Sha256};

const PORTABLE_MARKER_BYTES: &[u8] =
    b"{\"schema_version\":\"codenoesis.portable-graph-marker/v1\"}\n";
const EXPLORER_MARKER_BYTES: &[u8] =
    b"{\"schema_version\":\"codenoesis.local-explorer-marker/v1\"}\n";
const K1_PORTABLE_MARKER_BYTES: &[u8] =
    b"{\"schema_version\":\"codenoesis.portable-graph-marker/v2\"}\n";
const K1_EXPLORER_MARKER_BYTES: &[u8] =
    b"{\"schema_version\":\"codenoesis.local-explorer-marker/v2\"}\n";
const R10_PORTABLE_MARKER_BYTES: &[u8] =
    b"{\"schema_version\":\"codenoesis.portable-graph-marker/v3\"}\n";
const R10_EXPLORER_MARKER_BYTES: &[u8] =
    b"{\"schema_version\":\"codenoesis.local-explorer-marker/v3\"}\n";
const R11_PORTABLE_MARKER_BYTES: &[u8] =
    b"{\"schema_version\":\"codenoesis.portable-graph-marker/v4\"}\n";
const R11_EXPLORER_MARKER_BYTES: &[u8] =
    b"{\"schema_version\":\"codenoesis.local-explorer-marker/v4\"}\n";
const R12_PORTABLE_MARKER_BYTES: &[u8] =
    b"{\"schema_version\":\"codenoesis.portable-graph-marker/v5\"}\n";
const R12_EXPLORER_MARKER_BYTES: &[u8] =
    b"{\"schema_version\":\"codenoesis.local-explorer-marker/v5\"}\n";
const R13_PORTABLE_MARKER_BYTES: &[u8] =
    b"{\"schema_version\":\"codenoesis.portable-graph-marker/v6\"}\n";
const R13_EXPLORER_MARKER_BYTES: &[u8] =
    b"{\"schema_version\":\"codenoesis.local-explorer-marker/v6\"}\n";
const R14_PORTABLE_MARKER_BYTES: &[u8] =
    b"{\"schema_version\":\"codenoesis.portable-graph-marker/v7\"}\n";
const R14_EXPLORER_MARKER_BYTES: &[u8] =
    b"{\"schema_version\":\"codenoesis.local-explorer-marker/v7\"}\n";
const R15_PORTABLE_MARKER_BYTES: &[u8] =
    b"{\"schema_version\":\"codenoesis.portable-graph-marker/v8\"}\n";
const R15_EXPLORER_MARKER_BYTES: &[u8] =
    b"{\"schema_version\":\"codenoesis.local-explorer-marker/v8\"}\n";
const R16_PORTABLE_MARKER_BYTES: &[u8] =
    b"{\"schema_version\":\"codenoesis.portable-graph-marker/v9\"}\n";
const R16_EXPLORER_MARKER_BYTES: &[u8] =
    b"{\"schema_version\":\"codenoesis.local-explorer-marker/v9\"}\n";
const R17_EXPLORER_MARKER_BYTES: &[u8] =
    b"{\"schema_version\":\"codenoesis.local-explorer-marker/v10\"}\n";
const VIEWER_SHA256: &str = "1caa2c0ca5675937eab674f61681883ba3c6a428feb6b1baa744a0cb7eecd044";
const VIEWER_SOURCE_BYTES: &[u8] = include_bytes!("../assets/s4/r8/index.html");
const K1_VIEWER_SHA256: &str = "d0b633b29e6494d6494a35b5553d72c3dd04a747eeef219ca33a9f5fe2a1f4fa";
const K1_VIEWER_SOURCE_BYTES: &[u8] = include_bytes!("../assets/s4/k1/index.html");
const K1_CONTENT_SECURITY_POLICY: &str = "default-src 'none'; script-src 'sha256-R41XZjWjTeEyibhsIBP7psaPv+NxqnUdBpe0aobqE60='; style-src 'sha256-DlrPz5j7NCFDGArFkpQGNY37x++FaVe5CWumM3tlRuw='; img-src 'none'; font-src 'none'; connect-src 'none'; object-src 'none'; frame-src 'none'; frame-ancestors 'none'; form-action 'none'; base-uri 'none'; manifest-src 'none'; media-src 'none'; worker-src 'none'";
const VERSIONED_VIEWER_TEMPLATE_BYTES: &[u8] = include_bytes!("../assets/s4/versioned/index.html");
const VERSIONED_CONTENT_SECURITY_POLICY: &str = "default-src 'none'; script-src 'sha256-cFyehXImq07b3INT0Fff+uEiQWykXxu4jkzRLNqF9EM='; style-src 'sha256-Y4n+kUJ7cKEiQDrj1EhdHicIssJjePi9deKe0UgCjvk='; img-src 'none'; font-src 'none'; connect-src 'none'; object-src 'none'; frame-src 'none'; frame-ancestors 'none'; form-action 'none'; base-uri 'none'; manifest-src 'none'; media-src 'none'; worker-src 'none'";
const R10_VERSIONED_VIEWER: VersionedViewer = VersionedViewer::new(
    "codenoesis.portable-graph/v3",
    "PortableGraphV3",
    "LocalExplorerV3",
    "2b165eb4d4e0f9c0ccff1709142c5346825370921476cf3f37522a33c49d5ae9",
);
const R11_VERSIONED_VIEWER: VersionedViewer = VersionedViewer::new(
    "codenoesis.portable-graph/v4",
    "PortableGraphV4",
    "LocalExplorerV4",
    "e8e38f88c8890b574f75874ca64b65dcf27659e96eb88d8fb3e992365832f70d",
);
const R12_VERSIONED_VIEWER: VersionedViewer = VersionedViewer::new(
    "codenoesis.portable-graph/v5",
    "PortableGraphV5",
    "LocalExplorerV5",
    "5dce1d0c0c1242bd4b8555c2aa72a6e0a4abbd39e6541f7c779f3421c1d73573",
);
const R13_VERSIONED_VIEWER: VersionedViewer = VersionedViewer::new(
    "codenoesis.portable-graph/v6",
    "PortableGraphV6",
    "LocalExplorerV6",
    "c8b7d78da8750db7055b07ec65249570f816bdd046691b4b6f7f0dd49a864dd7",
);
const R14_VERSIONED_VIEWER: VersionedViewer = VersionedViewer::new(
    "codenoesis.portable-graph/v7",
    "PortableGraphV7",
    "LocalExplorerV7",
    "fe4da97cfe466106b43608067a5ef70660056a3467b6ca3af5362868435b70c9",
);
const R15_VERSIONED_VIEWER: VersionedViewer = VersionedViewer::new(
    "codenoesis.portable-graph/v8",
    "PortableGraphV8",
    "LocalExplorerV8",
    "6b15bf9b66487a6d76c5ac7b738d2bbcf76ccb946fd9b2f9e0c611da85c99ae5",
);
const R16_VERSIONED_VIEWER: VersionedViewer = VersionedViewer::new(
    "codenoesis.portable-graph/v9",
    "PortableGraphV9",
    "LocalExplorerV9",
    "1c933411b01e5684ceeca0d92423908976b9605e12c4e8a95533200eb494c85a",
);
const R17_FUNCTION_CONTEXT_VIEWER_SHA256: &str =
    "1494f3868a4dae29d20c46d423e4f368e4a33db9eeb0b9490884bfff621c8b54";
const R17_FUNCTION_CONTEXT_VIEWER_BYTES: &[u8] =
    include_bytes!("../assets/s4/function-context/index.html");
const R17_FUNCTION_CONTEXT_CONTENT_SECURITY_POLICY: &str = "default-src 'none'; script-src 'sha256-QLT6Bh/CWnZJlwd1fpkQRnyO3aIzrkXh0QprqW71ZPU='; style-src 'sha256-05TnzuCUvQRD9X6omoYicxv6U0biuIEsVNx0tcxpeHE='; img-src 'none'; font-src 'none'; connect-src 'none'; object-src 'none'; frame-src 'none'; frame-ancestors 'none'; form-action 'none'; base-uri 'none'; manifest-src 'none'; media-src 'none'; worker-src 'none'";
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy)]
struct VersionedViewer {
    portable_schema: &'static str,
    portable_label: &'static str,
    explorer_label: &'static str,
    sha256: &'static str,
}

impl VersionedViewer {
    const fn new(
        portable_schema: &'static str,
        portable_label: &'static str,
        explorer_label: &'static str,
        sha256: &'static str,
    ) -> Self {
        Self {
            portable_schema,
            portable_label,
            explorer_label,
            sha256,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PortableExplorerError {
    UnsafeOutput {
        path_sha256: String,
        reason: &'static str,
    },
    Contract(R8ContractError),
    K1Contract(K1ContractError),
    R10Contract(R10ContractError),
    R11Contract(R11ContractError),
    R12Contract(R12ContractError),
    R13Contract(R13ContractError),
    R14Contract(R14ContractError),
    R15Contract(R15ContractError),
    R16Contract(R16ContractError),
    Internal,
}

impl From<R8ContractError> for PortableExplorerError {
    fn from(error: R8ContractError) -> Self {
        Self::Contract(error)
    }
}

impl From<K1ContractError> for PortableExplorerError {
    fn from(error: K1ContractError) -> Self {
        Self::K1Contract(error)
    }
}

impl From<R10ContractError> for PortableExplorerError {
    fn from(error: R10ContractError) -> Self {
        Self::R10Contract(error)
    }
}

impl From<R11ContractError> for PortableExplorerError {
    fn from(error: R11ContractError) -> Self {
        Self::R11Contract(error)
    }
}

impl From<R12ContractError> for PortableExplorerError {
    fn from(error: R12ContractError) -> Self {
        Self::R12Contract(error)
    }
}

impl From<R13ContractError> for PortableExplorerError {
    fn from(error: R13ContractError) -> Self {
        Self::R13Contract(error)
    }
}

impl From<R14ContractError> for PortableExplorerError {
    fn from(error: R14ContractError) -> Self {
        Self::R14Contract(error)
    }
}

impl From<R15ContractError> for PortableExplorerError {
    fn from(error: R15ContractError) -> Self {
        Self::R15Contract(error)
    }
}

impl From<R16ContractError> for PortableExplorerError {
    fn from(error: R16ContractError) -> Self {
        Self::R16Contract(error)
    }
}

pub struct PreparedOutputRoot {
    kind: OutputKind,
    guard: PublicationGuard,
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
) -> Result<PreparedOutputRoot, PortableExplorerError> {
    ensure_output_root(store, output, OutputKind::Portable)
}

/// Publishes one validated portable graph into its marker-owned destination.
///
/// # Errors
///
/// Returns a typed error for integrity, ownership, race, or I/O failures.
pub fn publish_portable_graph(
    prepared: &PreparedOutputRoot,
    portable: &PortableGraphV1,
) -> Result<Vec<u8>, PortableExplorerError> {
    let bytes = portable.canonical_file();
    publish_files(
        prepared,
        OutputKind::Portable,
        &[OwnedFile::new("portable-graph.json", bytes.clone())],
    )?;
    Ok(bytes)
}

/// Validates a K1 export destination without creating or changing it.
///
/// # Errors
///
/// Returns a typed error for aliasing, symlinks, escapes, or invalid ownership.
pub fn validate_k1_export_output_root(
    store: &Path,
    output: &Path,
) -> Result<(), PortableExplorerError> {
    validate_output_root(store, output, OutputKind::K1Portable)
}

/// Creates an absent K1 export destination after complete validation.
///
/// # Errors
///
/// Returns a typed error when validation or directory creation fails.
pub fn ensure_k1_export_output_root_for_boundary(
    store: &Path,
    output: &Path,
) -> Result<PreparedOutputRoot, PortableExplorerError> {
    ensure_output_root(store, output, OutputKind::K1Portable)
}

/// Publishes one validated K1 portable graph into its marker-owned destination.
///
/// # Errors
///
/// Returns a typed error for integrity, ownership, race, or I/O failures.
pub fn publish_portable_graph_v2(
    prepared: &PreparedOutputRoot,
    portable: &PortableGraphV2,
) -> Result<Vec<u8>, PortableExplorerError> {
    let bytes = portable.canonical_file();
    publish_files(
        prepared,
        OutputKind::K1Portable,
        &[OwnedFile::new("portable-graph.json", bytes.clone())],
    )?;
    Ok(bytes)
}

/// Validates an R10 export destination without creating or changing it.
///
/// # Errors
///
/// Returns a typed error for aliasing, symlinks, escapes, or invalid ownership.
pub fn validate_r10_export_output_root(
    store: &Path,
    output: &Path,
) -> Result<(), PortableExplorerError> {
    validate_output_root(store, output, OutputKind::R10Portable)
}

/// Creates an absent R10 export destination after complete validation.
///
/// # Errors
///
/// Returns a typed error when validation or directory creation fails.
pub fn ensure_r10_export_output_root_for_boundary(
    store: &Path,
    output: &Path,
) -> Result<PreparedOutputRoot, PortableExplorerError> {
    ensure_output_root(store, output, OutputKind::R10Portable)
}

/// Publishes one validated R10 portable graph into its marker-owned destination.
///
/// # Errors
///
/// Returns a typed error for integrity, ownership, race, or I/O failures.
pub fn publish_portable_graph_v3(
    prepared: &PreparedOutputRoot,
    portable: &PortableGraphV3,
) -> Result<Vec<u8>, PortableExplorerError> {
    let bytes = portable.canonical_file();
    publish_files(
        prepared,
        OutputKind::R10Portable,
        &[OwnedFile::new("portable-graph.json", bytes.clone())],
    )?;
    Ok(bytes)
}

/// Validates an R11 export destination without creating or changing it.
///
/// # Errors
///
/// Returns a typed error for aliasing, symlinks, escapes, or invalid ownership.
pub fn validate_r11_export_output_root(
    store: &Path,
    output: &Path,
) -> Result<(), PortableExplorerError> {
    validate_output_root(store, output, OutputKind::R11Portable)
}

/// Creates an absent R11 export destination after complete validation.
///
/// # Errors
///
/// Returns a typed error when validation or directory creation fails.
pub fn ensure_r11_export_output_root_for_boundary(
    store: &Path,
    output: &Path,
) -> Result<PreparedOutputRoot, PortableExplorerError> {
    ensure_output_root(store, output, OutputKind::R11Portable)
}

/// Publishes one validated R11 portable graph into its marker-owned destination.
///
/// # Errors
///
/// Returns a typed error for integrity, ownership, race, or I/O failures.
pub fn publish_portable_graph_v4(
    prepared: &PreparedOutputRoot,
    portable: &PortableGraphV4,
) -> Result<Vec<u8>, PortableExplorerError> {
    let bytes = portable.canonical_file();
    publish_files(
        prepared,
        OutputKind::R11Portable,
        &[OwnedFile::new("portable-graph.json", bytes.clone())],
    )?;
    Ok(bytes)
}

/// Validates an R12 export destination without creating or changing it.
///
/// # Errors
///
/// Returns a typed error for aliasing, symlinks, escapes, or invalid ownership.
pub fn validate_r12_export_output_root(
    store: &Path,
    output: &Path,
) -> Result<(), PortableExplorerError> {
    validate_output_root(store, output, OutputKind::R12Portable)
}

/// Creates an absent R12 export destination after complete validation.
///
/// # Errors
///
/// Returns a typed error when validation or directory creation fails.
pub fn ensure_r12_export_output_root_for_boundary(
    store: &Path,
    output: &Path,
) -> Result<PreparedOutputRoot, PortableExplorerError> {
    ensure_output_root(store, output, OutputKind::R12Portable)
}

/// Publishes one validated R12 portable graph into its marker-owned destination.
///
/// # Errors
///
/// Returns a typed error for integrity, ownership, race, or I/O failures.
pub fn publish_portable_graph_v5(
    prepared: &PreparedOutputRoot,
    portable: &PortableGraphV5,
) -> Result<Vec<u8>, PortableExplorerError> {
    let bytes = portable.canonical_file();
    publish_files(
        prepared,
        OutputKind::R12Portable,
        &[OwnedFile::new("portable-graph.json", bytes.clone())],
    )?;
    Ok(bytes)
}

/// Validates an R13 export destination without creating or changing it.
///
/// # Errors
///
/// Returns a typed error for aliasing, symlinks, escapes, or invalid ownership.
pub fn validate_r13_export_output_root(
    store: &Path,
    output: &Path,
) -> Result<(), PortableExplorerError> {
    validate_output_root(store, output, OutputKind::R13Portable)
}

/// Creates an absent R13 export destination after complete validation.
///
/// # Errors
///
/// Returns a typed error when validation or directory creation fails.
pub fn ensure_r13_export_output_root_for_boundary(
    store: &Path,
    output: &Path,
) -> Result<PreparedOutputRoot, PortableExplorerError> {
    ensure_output_root(store, output, OutputKind::R13Portable)
}

/// Publishes one validated R13 portable graph into its marker-owned destination.
///
/// # Errors
///
/// Returns a typed error for integrity, ownership, race, or I/O failures.
pub fn publish_portable_graph_v6(
    prepared: &PreparedOutputRoot,
    portable: &PortableGraphV6,
) -> Result<Vec<u8>, PortableExplorerError> {
    let bytes = portable.canonical_file();
    publish_files(
        prepared,
        OutputKind::R13Portable,
        &[OwnedFile::new("portable-graph.json", bytes.clone())],
    )?;
    Ok(bytes)
}

/// Validates an R14 export destination without creating or changing it.
///
/// # Errors
///
/// Returns a typed error for aliasing, symlinks, escapes, or invalid ownership.
pub fn validate_r14_export_output_root(
    store: &Path,
    output: &Path,
) -> Result<(), PortableExplorerError> {
    validate_output_root(store, output, OutputKind::R14Portable)
}

/// Creates an absent R14 export destination after complete validation.
///
/// # Errors
///
/// Returns a typed error when validation or directory creation fails.
pub fn ensure_r14_export_output_root_for_boundary(
    store: &Path,
    output: &Path,
) -> Result<PreparedOutputRoot, PortableExplorerError> {
    ensure_output_root(store, output, OutputKind::R14Portable)
}

/// Publishes one validated R14 portable graph into its marker-owned destination.
///
/// # Errors
///
/// Returns a typed error for integrity, ownership, race, or I/O failures.
pub fn publish_portable_graph_v7(
    prepared: &PreparedOutputRoot,
    portable: &PortableGraphV7,
) -> Result<Vec<u8>, PortableExplorerError> {
    let bytes = portable.canonical_file();
    publish_files(
        prepared,
        OutputKind::R14Portable,
        &[OwnedFile::new("portable-graph.json", bytes.clone())],
    )?;
    Ok(bytes)
}

/// Validates an R15 export destination without creating or changing it.
///
/// # Errors
///
/// Returns a typed error for aliasing, symlinks, escapes, or invalid ownership.
pub fn validate_r15_export_output_root(
    store: &Path,
    output: &Path,
) -> Result<(), PortableExplorerError> {
    validate_output_root(store, output, OutputKind::R15Portable)
}

/// Creates an absent R15 export destination after complete validation.
///
/// # Errors
///
/// Returns a typed error when validation or directory creation fails.
pub fn ensure_r15_export_output_root_for_boundary(
    store: &Path,
    output: &Path,
) -> Result<PreparedOutputRoot, PortableExplorerError> {
    ensure_output_root(store, output, OutputKind::R15Portable)
}

/// Publishes one validated R15 portable graph into its marker-owned destination.
///
/// # Errors
///
/// Returns a typed error for integrity, ownership, race, or I/O failures.
pub fn publish_portable_graph_v8(
    prepared: &PreparedOutputRoot,
    portable: &PortableGraphV8,
) -> Result<Vec<u8>, PortableExplorerError> {
    let bytes = portable.canonical_file();
    publish_files(
        prepared,
        OutputKind::R15Portable,
        &[OwnedFile::new("portable-graph.json", bytes.clone())],
    )?;
    Ok(bytes)
}

/// Validates an R16 export destination without creating or changing it.
///
/// # Errors
///
/// Returns a typed error for aliasing, symlinks, escapes, or invalid ownership.
pub fn validate_r16_export_output_root(
    store: &Path,
    output: &Path,
) -> Result<(), PortableExplorerError> {
    validate_output_root(store, output, OutputKind::R16Portable)
}

/// Creates an absent R16 export destination after complete validation.
///
/// # Errors
///
/// Returns a typed error when validation or directory creation fails.
pub fn ensure_r16_export_output_root_for_boundary(
    store: &Path,
    output: &Path,
) -> Result<PreparedOutputRoot, PortableExplorerError> {
    ensure_output_root(store, output, OutputKind::R16Portable)
}

/// Publishes one validated R16 portable graph into its marker-owned destination.
///
/// # Errors
///
/// Returns a typed error for integrity, ownership, race, or I/O failures.
pub fn publish_portable_graph_v9(
    prepared: &PreparedOutputRoot,
    portable: &PortableGraphV9,
) -> Result<Vec<u8>, PortableExplorerError> {
    let bytes = portable.canonical_file();
    publish_files(
        prepared,
        OutputKind::R16Portable,
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
    let path_before_reopen = fs::symlink_metadata(&input).map_err(|_| invalid_input(&input))?;
    if !path_before_reopen.is_file() || unsafe_metadata(&path_before_reopen) {
        return Err(invalid_input(&input));
    }
    let mut reopened = File::open(&input).map_err(|_| invalid_input(&input))?;
    let reopened_before = reopened.metadata().map_err(|_| invalid_input(&input))?;
    if !reopened_before.is_file() {
        return Err(invalid_input(&input));
    }
    #[cfg(any(unix, windows))]
    let reopened_identity =
        FileIdentity::from_file(reopened.try_clone().map_err(|_| invalid_input(&input))?)
            .map_err(|_| invalid_input(&input))?;
    verify_open_file_bytes(&mut reopened, &bytes).map_err(|_| invalid_input(&input))?;
    let reopened_after = reopened.metadata().map_err(|_| invalid_input(&input))?;
    let path_after = fs::symlink_metadata(&input).map_err(|_| invalid_input(&input))?;
    #[cfg(any(unix, windows))]
    let path_identity_matches =
        FileIdentity::from_path(&input).is_ok_and(|path_identity| path_identity == identity);
    #[cfg(not(any(unix, windows)))]
    let path_identity_matches = true;
    #[cfg(any(unix, windows))]
    let reopened_identity_matches = reopened_identity == identity;
    #[cfg(not(any(unix, windows)))]
    let reopened_identity_matches = true;
    if before.len() != observed
        || unsafe_metadata(&path_after)
        || !same_file_metadata(&before, &after)
        || !same_file_metadata(&after, &path_before_reopen)
        || !same_file_metadata(&path_before_reopen, &reopened_before)
        || !same_file_metadata(&reopened_before, &reopened_after)
        || !same_file_metadata(&reopened_after, &path_after)
        || !reopened_identity_matches
        || !path_identity_matches
    {
        return Err(invalid_input(&input));
    }
    let portable = PortableGraphV1::from_canonical_file(&bytes, sha256)?;
    Ok((portable, bytes))
}

/// Acquires and strictly validates one immutable K1 portable graph input.
///
/// # Errors
///
/// Returns a typed error for unsafe paths, input races, limits, or contract failures.
pub fn read_portable_graph_v2(
    input: &Path,
) -> Result<(PortableGraphV2, Vec<u8>), PortableExplorerError> {
    read_portable_graph_v2_with(input, || {})
}

fn read_portable_graph_v2_with(
    input: &Path,
    after_first_read: impl FnOnce(),
) -> Result<(PortableGraphV2, Vec<u8>), PortableExplorerError> {
    let input = absolute_without_parent_components(input)
        .map_err(|_| PortableExplorerError::K1Contract(K1ContractError::InvalidProjection))?;
    verify_existing_components(&input)?;
    let path_metadata = fs::symlink_metadata(&input).map_err(|_| invalid_k1_input())?;
    if !path_metadata.is_file() || unsafe_metadata(&path_metadata) {
        return Err(invalid_k1_input());
    }
    let mut file = File::open(&input).map_err(|_| invalid_k1_input())?;
    let before = file.metadata().map_err(|_| invalid_k1_input())?;
    #[cfg(any(unix, windows))]
    let identity = FileIdentity::from_file(file.try_clone().map_err(|_| invalid_k1_input())?)
        .map_err(|_| invalid_k1_input())?;
    let maximum = MAX_K1_PORTABLE_GRAPH_BYTES;
    if before.len() > maximum {
        return Err(K1ContractError::LimitExceeded {
            limit: "portable_graph_bytes",
            maximum,
            observed: before.len(),
        }
        .into());
    }
    let capacity = usize::try_from(before.len()).map_err(|_| K1ContractError::LimitExceeded {
        limit: "portable_graph_bytes",
        maximum,
        observed: u64::MAX,
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    std::io::Read::by_ref(&mut file)
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| invalid_k1_input())?;
    let observed = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if observed > maximum {
        return Err(K1ContractError::LimitExceeded {
            limit: "portable_graph_bytes",
            maximum,
            observed,
        }
        .into());
    }
    after_first_read();
    verify_open_file_bytes(&mut file, &bytes).map_err(|_| invalid_k1_input())?;
    let after = file.metadata().map_err(|_| invalid_k1_input())?;
    let path_before_reopen = fs::symlink_metadata(&input).map_err(|_| invalid_k1_input())?;
    if !path_before_reopen.is_file() || unsafe_metadata(&path_before_reopen) {
        return Err(invalid_k1_input());
    }
    let mut reopened = File::open(&input).map_err(|_| invalid_k1_input())?;
    let reopened_before = reopened.metadata().map_err(|_| invalid_k1_input())?;
    if !reopened_before.is_file() {
        return Err(invalid_k1_input());
    }
    #[cfg(any(unix, windows))]
    let reopened_identity =
        FileIdentity::from_file(reopened.try_clone().map_err(|_| invalid_k1_input())?)
            .map_err(|_| invalid_k1_input())?;
    verify_open_file_bytes(&mut reopened, &bytes).map_err(|_| invalid_k1_input())?;
    let reopened_after = reopened.metadata().map_err(|_| invalid_k1_input())?;
    let path_after = fs::symlink_metadata(&input).map_err(|_| invalid_k1_input())?;
    #[cfg(any(unix, windows))]
    let path_identity_matches =
        FileIdentity::from_path(&input).is_ok_and(|path_identity| path_identity == identity);
    #[cfg(not(any(unix, windows)))]
    let path_identity_matches = true;
    #[cfg(any(unix, windows))]
    let reopened_identity_matches = reopened_identity == identity;
    #[cfg(not(any(unix, windows)))]
    let reopened_identity_matches = true;
    if before.len() != observed
        || unsafe_metadata(&path_after)
        || !same_file_metadata(&before, &after)
        || !same_file_metadata(&after, &path_before_reopen)
        || !same_file_metadata(&path_before_reopen, &reopened_before)
        || !same_file_metadata(&reopened_before, &reopened_after)
        || !same_file_metadata(&reopened_after, &path_after)
        || !reopened_identity_matches
        || !path_identity_matches
    {
        return Err(invalid_k1_input());
    }
    let observed_schema = serde_json::from_slice::<serde_json::Value>(&bytes)
        .ok()
        .and_then(|value| {
            value
                .get("schema_version")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        });
    if matches!(
        observed_schema.as_deref(),
        Some(R11_PORTABLE_GRAPH_VERSION | R12_PORTABLE_GRAPH_VERSION)
    ) {
        return Err(K1ContractError::UnsupportedPortableGraphSchema(
            observed_schema.unwrap_or_default(),
        )
        .into());
    }
    let portable = PortableGraphV2::from_canonical_file(&bytes, sha256)?;
    Ok((portable, bytes))
}

/// Acquires and strictly validates one immutable R10 portable graph input.
///
/// # Errors
///
/// Returns a typed error for unsafe paths, input races, limits, or contract failures.
pub fn read_portable_graph_v3(
    input: &Path,
) -> Result<(PortableGraphV3, Vec<u8>), PortableExplorerError> {
    read_portable_graph_v3_with(input, || {})
}

fn read_portable_graph_v3_with(
    input: &Path,
    after_first_read: impl FnOnce(),
) -> Result<(PortableGraphV3, Vec<u8>), PortableExplorerError> {
    let input = absolute_without_parent_components(input)
        .map_err(|_| PortableExplorerError::R10Contract(R10ContractError::InvalidProjection))?;
    verify_existing_components(&input)?;
    let path_metadata = fs::symlink_metadata(&input).map_err(|_| invalid_r10_input())?;
    if !path_metadata.is_file() || unsafe_metadata(&path_metadata) {
        return Err(invalid_r10_input());
    }
    let mut file = File::open(&input).map_err(|_| invalid_r10_input())?;
    let before = file.metadata().map_err(|_| invalid_r10_input())?;
    #[cfg(any(unix, windows))]
    let identity = FileIdentity::from_file(file.try_clone().map_err(|_| invalid_r10_input())?)
        .map_err(|_| invalid_r10_input())?;
    let maximum = MAX_R10_PORTABLE_GRAPH_BYTES;
    if before.len() > maximum {
        return Err(R10ContractError::LimitExceeded {
            limit: "portable_graph_bytes",
            maximum,
            observed: before.len(),
        }
        .into());
    }
    let capacity = usize::try_from(before.len()).map_err(|_| R10ContractError::LimitExceeded {
        limit: "portable_graph_bytes",
        maximum,
        observed: u64::MAX,
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    std::io::Read::by_ref(&mut file)
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| invalid_r10_input())?;
    let observed = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if observed > maximum {
        return Err(R10ContractError::LimitExceeded {
            limit: "portable_graph_bytes",
            maximum,
            observed,
        }
        .into());
    }
    after_first_read();
    verify_open_file_bytes(&mut file, &bytes).map_err(|_| invalid_r10_input())?;
    let after = file.metadata().map_err(|_| invalid_r10_input())?;
    let path_before_reopen = fs::symlink_metadata(&input).map_err(|_| invalid_r10_input())?;
    if !path_before_reopen.is_file() || unsafe_metadata(&path_before_reopen) {
        return Err(invalid_r10_input());
    }
    let mut reopened = File::open(&input).map_err(|_| invalid_r10_input())?;
    let reopened_before = reopened.metadata().map_err(|_| invalid_r10_input())?;
    if !reopened_before.is_file() {
        return Err(invalid_r10_input());
    }
    #[cfg(any(unix, windows))]
    let reopened_identity =
        FileIdentity::from_file(reopened.try_clone().map_err(|_| invalid_r10_input())?)
            .map_err(|_| invalid_r10_input())?;
    verify_open_file_bytes(&mut reopened, &bytes).map_err(|_| invalid_r10_input())?;
    let reopened_after = reopened.metadata().map_err(|_| invalid_r10_input())?;
    let path_after = fs::symlink_metadata(&input).map_err(|_| invalid_r10_input())?;
    #[cfg(any(unix, windows))]
    let path_identity_matches =
        FileIdentity::from_path(&input).is_ok_and(|path_identity| path_identity == identity);
    #[cfg(not(any(unix, windows)))]
    let path_identity_matches = true;
    #[cfg(any(unix, windows))]
    let reopened_identity_matches = reopened_identity == identity;
    #[cfg(not(any(unix, windows)))]
    let reopened_identity_matches = true;
    if before.len() != observed
        || unsafe_metadata(&path_after)
        || !same_file_metadata(&before, &after)
        || !same_file_metadata(&after, &path_before_reopen)
        || !same_file_metadata(&path_before_reopen, &reopened_before)
        || !same_file_metadata(&reopened_before, &reopened_after)
        || !same_file_metadata(&reopened_after, &path_after)
        || !reopened_identity_matches
        || !path_identity_matches
    {
        return Err(invalid_r10_input());
    }
    let portable = PortableGraphV3::from_canonical_file(&bytes, sha256)?;
    Ok((portable, bytes))
}

/// Acquires and strictly validates one immutable R11 portable graph input.
///
/// # Errors
///
/// Returns a typed error for unsafe paths, input races, limits, or contract failures.
pub fn read_portable_graph_v4(
    input: &Path,
) -> Result<(PortableGraphV4, Vec<u8>), PortableExplorerError> {
    read_portable_graph_v4_with(input, || {})
}

fn read_portable_graph_v4_with(
    input: &Path,
    after_first_read: impl FnOnce(),
) -> Result<(PortableGraphV4, Vec<u8>), PortableExplorerError> {
    let input = absolute_without_parent_components(input)
        .map_err(|_| PortableExplorerError::R11Contract(R11ContractError::InvalidProjection))?;
    verify_existing_components(&input)?;
    let path_metadata = fs::symlink_metadata(&input).map_err(|_| invalid_r11_input())?;
    if !path_metadata.is_file() || unsafe_metadata(&path_metadata) {
        return Err(invalid_r11_input());
    }
    let mut file = File::open(&input).map_err(|_| invalid_r11_input())?;
    let before = file.metadata().map_err(|_| invalid_r11_input())?;
    #[cfg(any(unix, windows))]
    let identity = FileIdentity::from_file(file.try_clone().map_err(|_| invalid_r11_input())?)
        .map_err(|_| invalid_r11_input())?;
    let maximum = MAX_R11_PORTABLE_GRAPH_BYTES;
    if before.len() > maximum {
        return Err(R11ContractError::LimitExceeded {
            limit: "portable_graph_bytes",
            maximum,
            observed: before.len(),
        }
        .into());
    }
    let capacity = usize::try_from(before.len()).map_err(|_| R11ContractError::LimitExceeded {
        limit: "portable_graph_bytes",
        maximum,
        observed: u64::MAX,
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    std::io::Read::by_ref(&mut file)
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| invalid_r11_input())?;
    let observed = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if observed > maximum {
        return Err(R11ContractError::LimitExceeded {
            limit: "portable_graph_bytes",
            maximum,
            observed,
        }
        .into());
    }
    after_first_read();
    verify_open_file_bytes(&mut file, &bytes).map_err(|_| invalid_r11_input())?;
    let after = file.metadata().map_err(|_| invalid_r11_input())?;
    let path_before_reopen = fs::symlink_metadata(&input).map_err(|_| invalid_r11_input())?;
    if !path_before_reopen.is_file() || unsafe_metadata(&path_before_reopen) {
        return Err(invalid_r11_input());
    }
    let mut reopened = File::open(&input).map_err(|_| invalid_r11_input())?;
    let reopened_before = reopened.metadata().map_err(|_| invalid_r11_input())?;
    if !reopened_before.is_file() {
        return Err(invalid_r11_input());
    }
    #[cfg(any(unix, windows))]
    let reopened_identity =
        FileIdentity::from_file(reopened.try_clone().map_err(|_| invalid_r11_input())?)
            .map_err(|_| invalid_r11_input())?;
    verify_open_file_bytes(&mut reopened, &bytes).map_err(|_| invalid_r11_input())?;
    let reopened_after = reopened.metadata().map_err(|_| invalid_r11_input())?;
    let path_after = fs::symlink_metadata(&input).map_err(|_| invalid_r11_input())?;
    #[cfg(any(unix, windows))]
    let path_identity_matches =
        FileIdentity::from_path(&input).is_ok_and(|path_identity| path_identity == identity);
    #[cfg(not(any(unix, windows)))]
    let path_identity_matches = true;
    #[cfg(any(unix, windows))]
    let reopened_identity_matches = reopened_identity == identity;
    #[cfg(not(any(unix, windows)))]
    let reopened_identity_matches = true;
    if before.len() != observed
        || unsafe_metadata(&path_after)
        || !same_file_metadata(&before, &after)
        || !same_file_metadata(&after, &path_before_reopen)
        || !same_file_metadata(&path_before_reopen, &reopened_before)
        || !same_file_metadata(&reopened_before, &reopened_after)
        || !same_file_metadata(&reopened_after, &path_after)
        || !reopened_identity_matches
        || !path_identity_matches
    {
        return Err(invalid_r11_input());
    }
    if let Some(observed) = serde_json::from_slice::<serde_json::Value>(&bytes)
        .ok()
        .and_then(|value| {
            value
                .get("schema_version")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .filter(|observed| {
            matches!(
                observed.as_str(),
                R12_PORTABLE_GRAPH_VERSION | R13_PORTABLE_GRAPH_VERSION
            )
        })
    {
        return Err(R11ContractError::UnsupportedPortableGraphSchema(observed).into());
    }
    let portable = PortableGraphV4::from_canonical_file(&bytes, sha256)?;
    Ok((portable, bytes))
}

/// Acquires and strictly validates one immutable R12 portable graph input.
///
/// # Errors
///
/// Returns a typed error for unsafe paths, input races, limits, or contract failures.
pub fn read_portable_graph_v5(
    input: &Path,
) -> Result<(PortableGraphV5, Vec<u8>), PortableExplorerError> {
    read_portable_graph_v5_with(input, || {})
}

fn read_portable_graph_v5_with(
    input: &Path,
    after_first_read: impl FnOnce(),
) -> Result<(PortableGraphV5, Vec<u8>), PortableExplorerError> {
    let input = absolute_without_parent_components(input)
        .map_err(|_| PortableExplorerError::R12Contract(R12ContractError::InvalidProjection))?;
    verify_existing_components(&input)?;
    let path_metadata = fs::symlink_metadata(&input).map_err(|_| invalid_r12_input())?;
    if !path_metadata.is_file() || unsafe_metadata(&path_metadata) {
        return Err(invalid_r12_input());
    }
    let mut file = File::open(&input).map_err(|_| invalid_r12_input())?;
    let before = file.metadata().map_err(|_| invalid_r12_input())?;
    #[cfg(any(unix, windows))]
    let identity = FileIdentity::from_file(file.try_clone().map_err(|_| invalid_r12_input())?)
        .map_err(|_| invalid_r12_input())?;
    let maximum = MAX_R12_PORTABLE_GRAPH_BYTES;
    if before.len() > maximum {
        return Err(R12ContractError::LimitExceeded {
            limit: "portable_graph_bytes",
            maximum,
            observed: before.len(),
        }
        .into());
    }
    let capacity = usize::try_from(before.len()).map_err(|_| R12ContractError::LimitExceeded {
        limit: "portable_graph_bytes",
        maximum,
        observed: u64::MAX,
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    std::io::Read::by_ref(&mut file)
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| invalid_r12_input())?;
    let observed = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if observed > maximum {
        return Err(R12ContractError::LimitExceeded {
            limit: "portable_graph_bytes",
            maximum,
            observed,
        }
        .into());
    }
    after_first_read();
    verify_open_file_bytes(&mut file, &bytes).map_err(|_| invalid_r12_input())?;
    let after = file.metadata().map_err(|_| invalid_r12_input())?;
    let path_before_reopen = fs::symlink_metadata(&input).map_err(|_| invalid_r12_input())?;
    if !path_before_reopen.is_file() || unsafe_metadata(&path_before_reopen) {
        return Err(invalid_r12_input());
    }
    let mut reopened = File::open(&input).map_err(|_| invalid_r12_input())?;
    let reopened_before = reopened.metadata().map_err(|_| invalid_r12_input())?;
    if !reopened_before.is_file() {
        return Err(invalid_r12_input());
    }
    #[cfg(any(unix, windows))]
    let reopened_identity =
        FileIdentity::from_file(reopened.try_clone().map_err(|_| invalid_r12_input())?)
            .map_err(|_| invalid_r12_input())?;
    verify_open_file_bytes(&mut reopened, &bytes).map_err(|_| invalid_r12_input())?;
    let reopened_after = reopened.metadata().map_err(|_| invalid_r12_input())?;
    let path_after = fs::symlink_metadata(&input).map_err(|_| invalid_r12_input())?;
    #[cfg(any(unix, windows))]
    let path_identity_matches =
        FileIdentity::from_path(&input).is_ok_and(|path_identity| path_identity == identity);
    #[cfg(not(any(unix, windows)))]
    let path_identity_matches = true;
    #[cfg(any(unix, windows))]
    let reopened_identity_matches = reopened_identity == identity;
    #[cfg(not(any(unix, windows)))]
    let reopened_identity_matches = true;
    if before.len() != observed
        || unsafe_metadata(&path_after)
        || !same_file_metadata(&before, &after)
        || !same_file_metadata(&after, &path_before_reopen)
        || !same_file_metadata(&path_before_reopen, &reopened_before)
        || !same_file_metadata(&reopened_before, &reopened_after)
        || !same_file_metadata(&reopened_after, &path_after)
        || !reopened_identity_matches
        || !path_identity_matches
    {
        return Err(invalid_r12_input());
    }
    if serde_json::from_slice::<serde_json::Value>(&bytes)
        .ok()
        .and_then(|value| {
            value
                .get("schema_version")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .as_deref()
        == Some(R13_PORTABLE_GRAPH_VERSION)
    {
        return Err(R12ContractError::UnsupportedPortableGraphSchema(
            R13_PORTABLE_GRAPH_VERSION.to_owned(),
        )
        .into());
    }
    let portable = PortableGraphV5::from_canonical_file(&bytes, sha256)?;
    Ok((portable, bytes))
}

/// Acquires and strictly validates one immutable R13 portable graph input.
///
/// # Errors
///
/// Returns a typed error for unsafe paths, input races, limits, or contract failures.
pub fn read_portable_graph_v6(
    input: &Path,
) -> Result<(PortableGraphV6, Vec<u8>), PortableExplorerError> {
    read_portable_graph_v6_with(input, || {})
}

fn read_portable_graph_v6_with(
    input: &Path,
    after_first_read: impl FnOnce(),
) -> Result<(PortableGraphV6, Vec<u8>), PortableExplorerError> {
    let input = absolute_without_parent_components(input)
        .map_err(|_| PortableExplorerError::R13Contract(R13ContractError::InvalidProjection))?;
    verify_existing_components(&input)?;
    let path_metadata = fs::symlink_metadata(&input).map_err(|_| invalid_r13_input())?;
    if !path_metadata.is_file() || unsafe_metadata(&path_metadata) {
        return Err(invalid_r13_input());
    }
    let mut file = File::open(&input).map_err(|_| invalid_r13_input())?;
    let before = file.metadata().map_err(|_| invalid_r13_input())?;
    #[cfg(any(unix, windows))]
    let identity = FileIdentity::from_file(file.try_clone().map_err(|_| invalid_r13_input())?)
        .map_err(|_| invalid_r13_input())?;
    let maximum = MAX_R13_PORTABLE_GRAPH_BYTES;
    if before.len() > maximum {
        return Err(R13ContractError::LimitExceeded {
            limit: "portable_graph_bytes",
            maximum,
            observed: before.len(),
        }
        .into());
    }
    let capacity = usize::try_from(before.len()).map_err(|_| R13ContractError::LimitExceeded {
        limit: "portable_graph_bytes",
        maximum,
        observed: u64::MAX,
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    std::io::Read::by_ref(&mut file)
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| invalid_r13_input())?;
    let observed = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if observed > maximum {
        return Err(R13ContractError::LimitExceeded {
            limit: "portable_graph_bytes",
            maximum,
            observed,
        }
        .into());
    }
    after_first_read();
    verify_open_file_bytes(&mut file, &bytes).map_err(|_| invalid_r13_input())?;
    let after = file.metadata().map_err(|_| invalid_r13_input())?;
    let path_before_reopen = fs::symlink_metadata(&input).map_err(|_| invalid_r13_input())?;
    if !path_before_reopen.is_file() || unsafe_metadata(&path_before_reopen) {
        return Err(invalid_r13_input());
    }
    let mut reopened = File::open(&input).map_err(|_| invalid_r13_input())?;
    let reopened_before = reopened.metadata().map_err(|_| invalid_r13_input())?;
    if !reopened_before.is_file() {
        return Err(invalid_r13_input());
    }
    #[cfg(any(unix, windows))]
    let reopened_identity =
        FileIdentity::from_file(reopened.try_clone().map_err(|_| invalid_r13_input())?)
            .map_err(|_| invalid_r13_input())?;
    verify_open_file_bytes(&mut reopened, &bytes).map_err(|_| invalid_r13_input())?;
    let reopened_after = reopened.metadata().map_err(|_| invalid_r13_input())?;
    let path_after = fs::symlink_metadata(&input).map_err(|_| invalid_r13_input())?;
    #[cfg(any(unix, windows))]
    let path_identity_matches =
        FileIdentity::from_path(&input).is_ok_and(|path_identity| path_identity == identity);
    #[cfg(not(any(unix, windows)))]
    let path_identity_matches = true;
    #[cfg(any(unix, windows))]
    let reopened_identity_matches = reopened_identity == identity;
    #[cfg(not(any(unix, windows)))]
    let reopened_identity_matches = true;
    if before.len() != observed
        || unsafe_metadata(&path_after)
        || !same_file_metadata(&before, &after)
        || !same_file_metadata(&after, &path_before_reopen)
        || !same_file_metadata(&path_before_reopen, &reopened_before)
        || !same_file_metadata(&reopened_before, &reopened_after)
        || !same_file_metadata(&reopened_after, &path_after)
        || !reopened_identity_matches
        || !path_identity_matches
    {
        return Err(invalid_r13_input());
    }
    let portable = PortableGraphV6::from_canonical_file(&bytes, sha256)?;
    Ok((portable, bytes))
}

/// Acquires and strictly validates one immutable R14 portable graph input.
///
/// # Errors
///
/// Returns a typed error for unsafe paths, input races, limits, or contract failures.
pub fn read_portable_graph_v7(
    input: &Path,
) -> Result<(PortableGraphV7, Vec<u8>), PortableExplorerError> {
    read_portable_graph_v7_with(input, || {})
}

fn read_portable_graph_v7_with(
    input: &Path,
    after_first_read: impl FnOnce(),
) -> Result<(PortableGraphV7, Vec<u8>), PortableExplorerError> {
    let input = absolute_without_parent_components(input)
        .map_err(|_| PortableExplorerError::R14Contract(R14ContractError::InvalidProjection))?;
    verify_existing_components(&input)?;
    let path_metadata = fs::symlink_metadata(&input).map_err(|_| invalid_r14_input())?;
    if !path_metadata.is_file() || unsafe_metadata(&path_metadata) {
        return Err(invalid_r14_input());
    }
    let mut file = File::open(&input).map_err(|_| invalid_r14_input())?;
    let before = file.metadata().map_err(|_| invalid_r14_input())?;
    #[cfg(any(unix, windows))]
    let identity = FileIdentity::from_file(file.try_clone().map_err(|_| invalid_r14_input())?)
        .map_err(|_| invalid_r14_input())?;
    let maximum = MAX_R14_PORTABLE_GRAPH_BYTES;
    if before.len() > maximum {
        return Err(R14ContractError::LimitExceeded {
            limit: "portable_graph_bytes",
            maximum,
            observed: before.len(),
        }
        .into());
    }
    let capacity = usize::try_from(before.len()).map_err(|_| R14ContractError::LimitExceeded {
        limit: "portable_graph_bytes",
        maximum,
        observed: u64::MAX,
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    std::io::Read::by_ref(&mut file)
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| invalid_r14_input())?;
    let observed = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if observed > maximum {
        return Err(R14ContractError::LimitExceeded {
            limit: "portable_graph_bytes",
            maximum,
            observed,
        }
        .into());
    }
    after_first_read();
    verify_open_file_bytes(&mut file, &bytes).map_err(|_| invalid_r14_input())?;
    let after = file.metadata().map_err(|_| invalid_r14_input())?;
    let path_before_reopen = fs::symlink_metadata(&input).map_err(|_| invalid_r14_input())?;
    if !path_before_reopen.is_file() || unsafe_metadata(&path_before_reopen) {
        return Err(invalid_r14_input());
    }
    let mut reopened = File::open(&input).map_err(|_| invalid_r14_input())?;
    let reopened_before = reopened.metadata().map_err(|_| invalid_r14_input())?;
    if !reopened_before.is_file() {
        return Err(invalid_r14_input());
    }
    #[cfg(any(unix, windows))]
    let reopened_identity =
        FileIdentity::from_file(reopened.try_clone().map_err(|_| invalid_r14_input())?)
            .map_err(|_| invalid_r14_input())?;
    verify_open_file_bytes(&mut reopened, &bytes).map_err(|_| invalid_r14_input())?;
    let reopened_after = reopened.metadata().map_err(|_| invalid_r14_input())?;
    let path_after = fs::symlink_metadata(&input).map_err(|_| invalid_r14_input())?;
    #[cfg(any(unix, windows))]
    let path_identity_matches =
        FileIdentity::from_path(&input).is_ok_and(|path_identity| path_identity == identity);
    #[cfg(not(any(unix, windows)))]
    let path_identity_matches = true;
    #[cfg(any(unix, windows))]
    let reopened_identity_matches = reopened_identity == identity;
    #[cfg(not(any(unix, windows)))]
    let reopened_identity_matches = true;
    if before.len() != observed
        || unsafe_metadata(&path_after)
        || !same_file_metadata(&before, &after)
        || !same_file_metadata(&after, &path_before_reopen)
        || !same_file_metadata(&path_before_reopen, &reopened_before)
        || !same_file_metadata(&reopened_before, &reopened_after)
        || !same_file_metadata(&reopened_after, &path_after)
        || !reopened_identity_matches
        || !path_identity_matches
    {
        return Err(invalid_r14_input());
    }
    let portable = PortableGraphV7::from_canonical_file(&bytes, sha256)?;
    Ok((portable, bytes))
}

/// Acquires and strictly validates one immutable R15 portable graph input.
///
/// # Errors
///
/// Returns a typed error for unsafe paths, input races, limits, or contract failures.
pub fn read_portable_graph_v8(
    input: &Path,
) -> Result<(PortableGraphV8, Vec<u8>), PortableExplorerError> {
    read_portable_graph_v8_with(input, || {})
}

fn read_portable_graph_v8_with(
    input: &Path,
    after_first_read: impl FnOnce(),
) -> Result<(PortableGraphV8, Vec<u8>), PortableExplorerError> {
    let input = absolute_without_parent_components(input)
        .map_err(|_| PortableExplorerError::R15Contract(R15ContractError::InvalidProjection))?;
    verify_existing_components(&input)?;
    let path_metadata = fs::symlink_metadata(&input).map_err(|_| invalid_r15_input())?;
    if !path_metadata.is_file() || unsafe_metadata(&path_metadata) {
        return Err(invalid_r15_input());
    }
    let mut file = File::open(&input).map_err(|_| invalid_r15_input())?;
    let before = file.metadata().map_err(|_| invalid_r15_input())?;
    #[cfg(any(unix, windows))]
    let identity = FileIdentity::from_file(file.try_clone().map_err(|_| invalid_r15_input())?)
        .map_err(|_| invalid_r15_input())?;
    let maximum = MAX_R15_PORTABLE_GRAPH_BYTES;
    if before.len() > maximum {
        return Err(R15ContractError::LimitExceeded {
            limit: "portable_graph_bytes",
            maximum,
            observed: before.len(),
        }
        .into());
    }
    let capacity = usize::try_from(before.len()).map_err(|_| R15ContractError::LimitExceeded {
        limit: "portable_graph_bytes",
        maximum,
        observed: u64::MAX,
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    std::io::Read::by_ref(&mut file)
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| invalid_r15_input())?;
    let observed = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if observed > maximum {
        return Err(R15ContractError::LimitExceeded {
            limit: "portable_graph_bytes",
            maximum,
            observed,
        }
        .into());
    }
    after_first_read();
    verify_open_file_bytes(&mut file, &bytes).map_err(|_| invalid_r15_input())?;
    let after = file.metadata().map_err(|_| invalid_r15_input())?;
    let path_before_reopen = fs::symlink_metadata(&input).map_err(|_| invalid_r15_input())?;
    if !path_before_reopen.is_file() || unsafe_metadata(&path_before_reopen) {
        return Err(invalid_r15_input());
    }
    let mut reopened = File::open(&input).map_err(|_| invalid_r15_input())?;
    let reopened_before = reopened.metadata().map_err(|_| invalid_r15_input())?;
    if !reopened_before.is_file() {
        return Err(invalid_r15_input());
    }
    #[cfg(any(unix, windows))]
    let reopened_identity =
        FileIdentity::from_file(reopened.try_clone().map_err(|_| invalid_r15_input())?)
            .map_err(|_| invalid_r15_input())?;
    verify_open_file_bytes(&mut reopened, &bytes).map_err(|_| invalid_r15_input())?;
    let reopened_after = reopened.metadata().map_err(|_| invalid_r15_input())?;
    let path_after = fs::symlink_metadata(&input).map_err(|_| invalid_r15_input())?;
    #[cfg(any(unix, windows))]
    let path_identity_matches =
        FileIdentity::from_path(&input).is_ok_and(|path_identity| path_identity == identity);
    #[cfg(not(any(unix, windows)))]
    let path_identity_matches = true;
    #[cfg(any(unix, windows))]
    let reopened_identity_matches = reopened_identity == identity;
    #[cfg(not(any(unix, windows)))]
    let reopened_identity_matches = true;
    if before.len() != observed
        || unsafe_metadata(&path_after)
        || !same_file_metadata(&before, &after)
        || !same_file_metadata(&after, &path_before_reopen)
        || !same_file_metadata(&path_before_reopen, &reopened_before)
        || !same_file_metadata(&reopened_before, &reopened_after)
        || !same_file_metadata(&reopened_after, &path_after)
        || !reopened_identity_matches
        || !path_identity_matches
    {
        return Err(invalid_r15_input());
    }
    let portable = PortableGraphV8::from_canonical_file(&bytes, sha256)?;
    Ok((portable, bytes))
}

/// Acquires and strictly validates one immutable R16 portable graph input.
///
/// # Errors
///
/// Returns a typed error for unsafe paths, input races, limits, or contract failures.
pub fn read_portable_graph_v9(
    input: &Path,
) -> Result<(PortableGraphV9, Vec<u8>), PortableExplorerError> {
    read_portable_graph_v9_with(input, || {})
}

fn read_portable_graph_v9_with(
    input: &Path,
    after_first_read: impl FnOnce(),
) -> Result<(PortableGraphV9, Vec<u8>), PortableExplorerError> {
    let input = absolute_without_parent_components(input)
        .map_err(|_| PortableExplorerError::R16Contract(R16ContractError::InvalidProjection))?;
    verify_existing_components(&input)?;
    let path_metadata = fs::symlink_metadata(&input).map_err(|_| invalid_r16_input())?;
    if !path_metadata.is_file() || unsafe_metadata(&path_metadata) {
        return Err(invalid_r16_input());
    }
    let mut file = File::open(&input).map_err(|_| invalid_r16_input())?;
    let before = file.metadata().map_err(|_| invalid_r16_input())?;
    #[cfg(any(unix, windows))]
    let identity = FileIdentity::from_file(file.try_clone().map_err(|_| invalid_r16_input())?)
        .map_err(|_| invalid_r16_input())?;
    let maximum = MAX_R16_PORTABLE_GRAPH_BYTES;
    if before.len() > maximum {
        return Err(R16ContractError::LimitExceeded {
            limit: "portable_graph_bytes",
            maximum,
            observed: before.len(),
        }
        .into());
    }
    let capacity = usize::try_from(before.len()).map_err(|_| R16ContractError::LimitExceeded {
        limit: "portable_graph_bytes",
        maximum,
        observed: u64::MAX,
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    std::io::Read::by_ref(&mut file)
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| invalid_r16_input())?;
    let observed = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if observed > maximum {
        return Err(R16ContractError::LimitExceeded {
            limit: "portable_graph_bytes",
            maximum,
            observed,
        }
        .into());
    }
    after_first_read();
    verify_open_file_bytes(&mut file, &bytes).map_err(|_| invalid_r16_input())?;
    let after = file.metadata().map_err(|_| invalid_r16_input())?;
    let path_before_reopen = fs::symlink_metadata(&input).map_err(|_| invalid_r16_input())?;
    if !path_before_reopen.is_file() || unsafe_metadata(&path_before_reopen) {
        return Err(invalid_r16_input());
    }
    let mut reopened = File::open(&input).map_err(|_| invalid_r16_input())?;
    let reopened_before = reopened.metadata().map_err(|_| invalid_r16_input())?;
    if !reopened_before.is_file() {
        return Err(invalid_r16_input());
    }
    #[cfg(any(unix, windows))]
    let reopened_identity =
        FileIdentity::from_file(reopened.try_clone().map_err(|_| invalid_r16_input())?)
            .map_err(|_| invalid_r16_input())?;
    verify_open_file_bytes(&mut reopened, &bytes).map_err(|_| invalid_r16_input())?;
    let reopened_after = reopened.metadata().map_err(|_| invalid_r16_input())?;
    let path_after = fs::symlink_metadata(&input).map_err(|_| invalid_r16_input())?;
    #[cfg(any(unix, windows))]
    let path_identity_matches =
        FileIdentity::from_path(&input).is_ok_and(|path_identity| path_identity == identity);
    #[cfg(not(any(unix, windows)))]
    let path_identity_matches = true;
    #[cfg(any(unix, windows))]
    let reopened_identity_matches = reopened_identity == identity;
    #[cfg(not(any(unix, windows)))]
    let reopened_identity_matches = true;
    if before.len() != observed
        || unsafe_metadata(&path_after)
        || !same_file_metadata(&before, &after)
        || !same_file_metadata(&after, &path_before_reopen)
        || !same_file_metadata(&path_before_reopen, &reopened_before)
        || !same_file_metadata(&reopened_before, &reopened_after)
        || !same_file_metadata(&reopened_after, &path_after)
        || !reopened_identity_matches
        || !path_identity_matches
    {
        return Err(invalid_r16_input());
    }
    let portable = PortableGraphV9::from_canonical_file(&bytes, sha256)?;
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

fn normalize_checkout_text(bytes: &[u8]) -> Option<Vec<u8>> {
    if !bytes.contains(&b'\r') {
        return Some(bytes.to_vec());
    }
    let mut normalized = Vec::with_capacity(bytes.len());
    let mut index = 0;
    let mut saw_crlf = false;
    let mut saw_lf = false;
    while index < bytes.len() {
        match bytes[index] {
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => {
                normalized.push(b'\n');
                saw_crlf = true;
                index += 2;
            }
            b'\r' => return None,
            b'\n' => {
                normalized.push(b'\n');
                saw_lf = true;
                index += 1;
            }
            byte => {
                normalized.push(byte);
                index += 1;
            }
        }
    }
    if saw_crlf && saw_lf {
        None
    } else {
        Some(normalized)
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
) -> Result<PreparedOutputRoot, PortableExplorerError> {
    ensure_output_root(input, output, OutputKind::Explorer)
}

/// Publishes the reviewed static explorer bound to one validated portable graph.
///
/// # Errors
///
/// Returns a typed error for asset, integrity, ownership, race, or I/O failures.
pub fn publish_local_explorer(
    prepared: &PreparedOutputRoot,
    portable: &PortableGraphV1,
    portable_bytes: &[u8],
) -> Result<Vec<u8>, PortableExplorerError> {
    let viewer_bytes =
        normalize_checkout_text(VIEWER_SOURCE_BYTES).ok_or(PortableExplorerError::Internal)?;
    let manifest =
        LocalExplorerManifestV1::new(portable, portable_bytes, &viewer_bytes, VIEWER_SHA256)?;
    let manifest_bytes = manifest
        .canonical_file()
        .map_err(|_| PortableExplorerError::Internal)?;
    publish_files(
        prepared,
        OutputKind::Explorer,
        &[
            OwnedFile::new("portable-graph.json", portable_bytes.to_vec()),
            OwnedFile::new("index.html", viewer_bytes),
            OwnedFile::new("explorer-manifest.json", manifest_bytes.clone()),
        ],
    )?;
    Ok(manifest_bytes)
}

/// Validates a K1 explorer destination without creating or changing it.
///
/// # Errors
///
/// Returns a typed error for aliasing, symlinks, escapes, or invalid ownership.
pub fn validate_k1_explorer_output_root(
    input: &Path,
    output: &Path,
) -> Result<(), PortableExplorerError> {
    validate_output_root(input, output, OutputKind::K1Explorer)
}

/// Creates an absent K1 explorer destination after complete validation.
///
/// # Errors
///
/// Returns a typed error when validation or directory creation fails.
pub fn ensure_k1_explorer_output_root_for_boundary(
    input: &Path,
    output: &Path,
) -> Result<PreparedOutputRoot, PortableExplorerError> {
    ensure_output_root(input, output, OutputKind::K1Explorer)
}

/// Publishes the reviewed K1 static explorer bound to one validated graph.
///
/// # Errors
///
/// Returns a typed error for asset, integrity, ownership, race, or I/O failures.
pub fn publish_local_explorer_v2(
    prepared: &PreparedOutputRoot,
    portable: &PortableGraphV2,
    portable_bytes: &[u8],
) -> Result<Vec<u8>, PortableExplorerError> {
    if portable.canonical_file() != portable_bytes {
        return Err(K1ContractError::InvalidProjection.into());
    }
    let viewer_bytes =
        normalize_checkout_text(K1_VIEWER_SOURCE_BYTES).ok_or(PortableExplorerError::Internal)?;
    let manifest = LocalExplorerManifestV2::new(
        portable,
        &viewer_bytes,
        K1_VIEWER_SHA256,
        K1_CONTENT_SECURITY_POLICY,
        sha256,
    )?;
    let manifest_bytes = manifest
        .canonical_file()
        .map_err(|_| PortableExplorerError::Internal)?;
    publish_files(
        prepared,
        OutputKind::K1Explorer,
        &[
            OwnedFile::new("portable-graph.json", portable_bytes.to_vec()),
            OwnedFile::new("index.html", viewer_bytes),
            OwnedFile::new("explorer-manifest.json", manifest_bytes.clone()),
        ],
    )?;
    Ok(manifest_bytes)
}

/// Validates an R10 explorer destination without creating or changing it.
///
/// # Errors
///
/// Returns a typed error for aliasing, symlinks, escapes, or invalid ownership.
pub fn validate_r10_explorer_output_root(
    input: &Path,
    output: &Path,
) -> Result<(), PortableExplorerError> {
    validate_output_root(input, output, OutputKind::R10Explorer)
}

/// Creates an absent R10 explorer destination after complete validation.
///
/// # Errors
///
/// Returns a typed error when validation or directory creation fails.
pub fn ensure_r10_explorer_output_root_for_boundary(
    input: &Path,
    output: &Path,
) -> Result<PreparedOutputRoot, PortableExplorerError> {
    ensure_output_root(input, output, OutputKind::R10Explorer)
}

/// Publishes the exact-schema versioned viewer bound to one validated R10 graph.
///
/// # Errors
///
/// Returns a typed error for asset, integrity, ownership, race, or I/O failures.
pub fn publish_local_explorer_v3(
    prepared: &PreparedOutputRoot,
    portable: &PortableGraphV3,
    portable_bytes: &[u8],
) -> Result<Vec<u8>, PortableExplorerError> {
    if portable.canonical_file() != portable_bytes {
        return Err(R10ContractError::InvalidProjection.into());
    }
    let viewer_bytes = render_versioned_viewer(R10_VERSIONED_VIEWER)?;
    let manifest = LocalExplorerManifestV3::new(
        portable,
        &viewer_bytes,
        R10_VERSIONED_VIEWER.sha256,
        VERSIONED_CONTENT_SECURITY_POLICY,
        sha256,
    )?;
    let manifest_bytes = manifest
        .canonical_file()
        .map_err(|_| PortableExplorerError::Internal)?;
    publish_files(
        prepared,
        OutputKind::R10Explorer,
        &[
            OwnedFile::new("portable-graph.json", portable_bytes.to_vec()),
            OwnedFile::new("index.html", viewer_bytes),
            OwnedFile::new("explorer-manifest.json", manifest_bytes.clone()),
        ],
    )?;
    Ok(manifest_bytes)
}

/// Validates an R11 explorer destination without creating or changing it.
///
/// # Errors
///
/// Returns a typed error for aliasing, symlinks, escapes, or invalid ownership.
pub fn validate_r11_explorer_output_root(
    input: &Path,
    output: &Path,
) -> Result<(), PortableExplorerError> {
    validate_output_root(input, output, OutputKind::R11Explorer)
}

/// Creates an absent R11 explorer destination after complete validation.
///
/// # Errors
///
/// Returns a typed error when validation or directory creation fails.
pub fn ensure_r11_explorer_output_root_for_boundary(
    input: &Path,
    output: &Path,
) -> Result<PreparedOutputRoot, PortableExplorerError> {
    ensure_output_root(input, output, OutputKind::R11Explorer)
}

/// Publishes the exact-schema versioned viewer bound to one validated R11 graph.
///
/// # Errors
///
/// Returns a typed error for asset, integrity, ownership, race, or I/O failures.
pub fn publish_local_explorer_v4(
    prepared: &PreparedOutputRoot,
    portable: &PortableGraphV4,
    portable_bytes: &[u8],
) -> Result<Vec<u8>, PortableExplorerError> {
    if portable.canonical_file() != portable_bytes {
        return Err(R11ContractError::InvalidProjection.into());
    }
    let viewer_bytes = render_versioned_viewer(R11_VERSIONED_VIEWER)?;
    let manifest = LocalExplorerManifestV4::new(
        portable,
        &viewer_bytes,
        R11_VERSIONED_VIEWER.sha256,
        VERSIONED_CONTENT_SECURITY_POLICY,
        sha256,
    )?;
    let manifest_bytes = manifest
        .canonical_file()
        .map_err(|_| PortableExplorerError::Internal)?;
    publish_files(
        prepared,
        OutputKind::R11Explorer,
        &[
            OwnedFile::new("portable-graph.json", portable_bytes.to_vec()),
            OwnedFile::new("index.html", viewer_bytes),
            OwnedFile::new("explorer-manifest.json", manifest_bytes.clone()),
        ],
    )?;
    Ok(manifest_bytes)
}

/// Validates an R12 explorer destination without creating or changing it.
///
/// # Errors
///
/// Returns a typed error for aliasing, symlinks, escapes, or invalid ownership.
pub fn validate_r12_explorer_output_root(
    input: &Path,
    output: &Path,
) -> Result<(), PortableExplorerError> {
    validate_output_root(input, output, OutputKind::R12Explorer)
}

/// Creates an absent R12 explorer destination after complete validation.
///
/// # Errors
///
/// Returns a typed error when validation or directory creation fails.
pub fn ensure_r12_explorer_output_root_for_boundary(
    input: &Path,
    output: &Path,
) -> Result<PreparedOutputRoot, PortableExplorerError> {
    ensure_output_root(input, output, OutputKind::R12Explorer)
}

/// Publishes the exact-schema versioned viewer bound to one validated R12 graph.
///
/// # Errors
///
/// Returns a typed error for asset, integrity, ownership, race, or I/O failures.
pub fn publish_local_explorer_v5(
    prepared: &PreparedOutputRoot,
    portable: &PortableGraphV5,
    portable_bytes: &[u8],
) -> Result<Vec<u8>, PortableExplorerError> {
    if portable.canonical_file() != portable_bytes {
        return Err(R12ContractError::InvalidProjection.into());
    }
    let viewer_bytes = render_versioned_viewer(R12_VERSIONED_VIEWER)?;
    let manifest = LocalExplorerManifestV5::new(
        portable,
        &viewer_bytes,
        R12_VERSIONED_VIEWER.sha256,
        VERSIONED_CONTENT_SECURITY_POLICY,
        sha256,
    )?;
    let manifest_bytes = manifest
        .canonical_file()
        .map_err(|_| PortableExplorerError::Internal)?;
    publish_files(
        prepared,
        OutputKind::R12Explorer,
        &[
            OwnedFile::new("portable-graph.json", portable_bytes.to_vec()),
            OwnedFile::new("index.html", viewer_bytes),
            OwnedFile::new("explorer-manifest.json", manifest_bytes.clone()),
        ],
    )?;
    Ok(manifest_bytes)
}

/// Validates an R13 explorer destination without creating or changing it.
///
/// # Errors
///
/// Returns a typed error for aliasing, symlinks, escapes, or invalid ownership.
pub fn validate_r13_explorer_output_root(
    input: &Path,
    output: &Path,
) -> Result<(), PortableExplorerError> {
    validate_output_root(input, output, OutputKind::R13Explorer)
}

/// Creates an absent R13 explorer destination after complete validation.
///
/// # Errors
///
/// Returns a typed error when validation or directory creation fails.
pub fn ensure_r13_explorer_output_root_for_boundary(
    input: &Path,
    output: &Path,
) -> Result<PreparedOutputRoot, PortableExplorerError> {
    ensure_output_root(input, output, OutputKind::R13Explorer)
}

/// Publishes the exact-schema versioned viewer bound to one validated R13 graph.
///
/// # Errors
///
/// Returns a typed error for asset, integrity, ownership, race, or I/O failures.
pub fn publish_local_explorer_v6(
    prepared: &PreparedOutputRoot,
    portable: &PortableGraphV6,
    portable_bytes: &[u8],
) -> Result<Vec<u8>, PortableExplorerError> {
    if portable.canonical_file() != portable_bytes {
        return Err(R13ContractError::InvalidProjection.into());
    }
    let viewer_bytes = render_versioned_viewer(R13_VERSIONED_VIEWER)?;
    let manifest = LocalExplorerManifestV6::new(
        portable,
        &viewer_bytes,
        R13_VERSIONED_VIEWER.sha256,
        VERSIONED_CONTENT_SECURITY_POLICY,
        sha256,
    )?;
    let manifest_bytes = manifest
        .canonical_file()
        .map_err(|_| PortableExplorerError::Internal)?;
    publish_files(
        prepared,
        OutputKind::R13Explorer,
        &[
            OwnedFile::new("portable-graph.json", portable_bytes.to_vec()),
            OwnedFile::new("index.html", viewer_bytes),
            OwnedFile::new("explorer-manifest.json", manifest_bytes.clone()),
        ],
    )?;
    Ok(manifest_bytes)
}

/// Validates an R14 explorer destination without creating or changing it.
///
/// # Errors
///
/// Returns a typed error for aliasing, symlinks, escapes, or invalid ownership.
pub fn validate_r14_explorer_output_root(
    input: &Path,
    output: &Path,
) -> Result<(), PortableExplorerError> {
    validate_output_root(input, output, OutputKind::R14Explorer)
}

/// Creates an absent R14 explorer destination after complete validation.
///
/// # Errors
///
/// Returns a typed error when validation or directory creation fails.
pub fn ensure_r14_explorer_output_root_for_boundary(
    input: &Path,
    output: &Path,
) -> Result<PreparedOutputRoot, PortableExplorerError> {
    ensure_output_root(input, output, OutputKind::R14Explorer)
}

/// Publishes the exact-schema versioned viewer bound to one validated R14 graph.
///
/// # Errors
///
/// Returns a typed error for asset, integrity, ownership, race, or I/O failures.
pub fn publish_local_explorer_v7(
    prepared: &PreparedOutputRoot,
    portable: &PortableGraphV7,
    portable_bytes: &[u8],
) -> Result<Vec<u8>, PortableExplorerError> {
    if portable.canonical_file() != portable_bytes {
        return Err(R14ContractError::InvalidProjection.into());
    }
    let viewer_bytes = render_versioned_viewer(R14_VERSIONED_VIEWER)?;
    let manifest = LocalExplorerManifestV7::new(
        portable,
        &viewer_bytes,
        R14_VERSIONED_VIEWER.sha256,
        VERSIONED_CONTENT_SECURITY_POLICY,
        sha256,
    )?;
    let manifest_bytes = manifest
        .canonical_file()
        .map_err(|_| PortableExplorerError::Internal)?;
    publish_files(
        prepared,
        OutputKind::R14Explorer,
        &[
            OwnedFile::new("portable-graph.json", portable_bytes.to_vec()),
            OwnedFile::new("index.html", viewer_bytes),
            OwnedFile::new("explorer-manifest.json", manifest_bytes.clone()),
        ],
    )?;
    Ok(manifest_bytes)
}

/// Validates an R15 explorer destination without creating or changing it.
///
/// # Errors
///
/// Returns a typed error for aliasing, symlinks, escapes, or invalid ownership.
pub fn validate_r15_explorer_output_root(
    input: &Path,
    output: &Path,
) -> Result<(), PortableExplorerError> {
    validate_output_root(input, output, OutputKind::R15Explorer)
}

/// Creates an absent R15 explorer destination after complete validation.
///
/// # Errors
///
/// Returns a typed error when validation or directory creation fails.
pub fn ensure_r15_explorer_output_root_for_boundary(
    input: &Path,
    output: &Path,
) -> Result<PreparedOutputRoot, PortableExplorerError> {
    ensure_output_root(input, output, OutputKind::R15Explorer)
}

/// Publishes the exact-schema versioned viewer bound to one validated R15 graph.
///
/// # Errors
///
/// Returns a typed error for asset, integrity, ownership, race, or I/O failures.
pub fn publish_local_explorer_v8(
    prepared: &PreparedOutputRoot,
    portable: &PortableGraphV8,
    portable_bytes: &[u8],
) -> Result<Vec<u8>, PortableExplorerError> {
    if portable.canonical_file() != portable_bytes {
        return Err(R15ContractError::InvalidProjection.into());
    }
    let viewer_bytes = render_versioned_viewer(R15_VERSIONED_VIEWER)?;
    let manifest = LocalExplorerManifestV8::new(
        portable,
        &viewer_bytes,
        R15_VERSIONED_VIEWER.sha256,
        VERSIONED_CONTENT_SECURITY_POLICY,
        sha256,
    )?;
    let manifest_bytes = manifest
        .canonical_file()
        .map_err(|_| PortableExplorerError::Internal)?;
    publish_files(
        prepared,
        OutputKind::R15Explorer,
        &[
            OwnedFile::new("portable-graph.json", portable_bytes.to_vec()),
            OwnedFile::new("index.html", viewer_bytes),
            OwnedFile::new("explorer-manifest.json", manifest_bytes.clone()),
        ],
    )?;
    Ok(manifest_bytes)
}

/// Validates an R16 explorer destination without creating or changing it.
///
/// # Errors
///
/// Returns a typed error for aliasing, symlinks, escapes, or invalid ownership.
pub fn validate_r16_explorer_output_root(
    input: &Path,
    output: &Path,
) -> Result<(), PortableExplorerError> {
    validate_output_root(input, output, OutputKind::R16Explorer)
}

/// Creates an absent R16 explorer destination after complete validation.
///
/// # Errors
///
/// Returns a typed error when validation or directory creation fails.
pub fn ensure_r16_explorer_output_root_for_boundary(
    input: &Path,
    output: &Path,
) -> Result<PreparedOutputRoot, PortableExplorerError> {
    ensure_output_root(input, output, OutputKind::R16Explorer)
}

/// Publishes the exact-schema versioned viewer bound to one validated R16 graph.
///
/// # Errors
///
/// Returns a typed error for asset, integrity, ownership, race, or I/O failures.
pub fn publish_local_explorer_v9(
    prepared: &PreparedOutputRoot,
    portable: &PortableGraphV9,
    portable_bytes: &[u8],
) -> Result<Vec<u8>, PortableExplorerError> {
    if portable.canonical_file() != portable_bytes {
        return Err(R16ContractError::InvalidProjection.into());
    }
    let viewer_bytes = render_versioned_viewer(R16_VERSIONED_VIEWER)?;
    let manifest = LocalExplorerManifestV9::new(
        portable,
        &viewer_bytes,
        R16_VERSIONED_VIEWER.sha256,
        VERSIONED_CONTENT_SECURITY_POLICY,
        sha256,
    )?;
    let manifest_bytes = manifest
        .canonical_file()
        .map_err(|_| PortableExplorerError::Internal)?;
    publish_files(
        prepared,
        OutputKind::R16Explorer,
        &[
            OwnedFile::new("portable-graph.json", portable_bytes.to_vec()),
            OwnedFile::new("index.html", viewer_bytes),
            OwnedFile::new("explorer-manifest.json", manifest_bytes.clone()),
        ],
    )?;
    Ok(manifest_bytes)
}

/// Validates an R17 explorer destination without creating or changing it.
///
/// # Errors
///
/// Returns a typed error for aliasing, symlinks, escapes, or invalid ownership.
pub fn validate_r17_explorer_output_root(
    input: &Path,
    output: &Path,
) -> Result<(), PortableExplorerError> {
    validate_output_root(input, output, OutputKind::R17Explorer)
}

/// Creates an absent R17 explorer destination after complete validation.
///
/// # Errors
///
/// Returns a typed error when validation or directory creation fails.
pub fn ensure_r17_explorer_output_root_for_boundary(
    input: &Path,
    output: &Path,
) -> Result<PreparedOutputRoot, PortableExplorerError> {
    ensure_output_root(input, output, OutputKind::R17Explorer)
}

/// Publishes the function-centered viewer bound to one validated R16 graph.
///
/// # Errors
///
/// Returns a typed error for asset, integrity, ownership, race, or I/O failures.
pub fn publish_local_explorer_v10(
    prepared: &PreparedOutputRoot,
    portable: &PortableGraphV9,
    portable_bytes: &[u8],
) -> Result<Vec<u8>, PortableExplorerError> {
    if portable.canonical_file() != portable_bytes {
        return Err(R16ContractError::InvalidProjection.into());
    }
    let viewer_bytes = normalize_checkout_text(R17_FUNCTION_CONTEXT_VIEWER_BYTES)
        .ok_or(PortableExplorerError::Internal)?;
    let manifest = LocalExplorerManifestV10::new(
        portable,
        &viewer_bytes,
        R17_FUNCTION_CONTEXT_VIEWER_SHA256,
        R17_FUNCTION_CONTEXT_CONTENT_SECURITY_POLICY,
        sha256,
    )?;
    let manifest_bytes = manifest
        .canonical_file()
        .map_err(|_| PortableExplorerError::Internal)?;
    publish_files(
        prepared,
        OutputKind::R17Explorer,
        &[
            OwnedFile::new("portable-graph.json", portable_bytes.to_vec()),
            OwnedFile::new("index.html", viewer_bytes),
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

fn render_versioned_viewer(contract: VersionedViewer) -> Result<Vec<u8>, PortableExplorerError> {
    let template = normalize_checkout_text(VERSIONED_VIEWER_TEMPLATE_BYTES)
        .ok_or(PortableExplorerError::Internal)?;
    let template = std::str::from_utf8(&template).map_err(|_| PortableExplorerError::Internal)?;
    if !template.contains("@@PORTABLE_SCHEMA@@")
        || !template.contains("@@PORTABLE_LABEL@@")
        || !template.contains("@@EXPLORER_LABEL@@")
    {
        return Err(PortableExplorerError::Internal);
    }
    let viewer = template
        .replace("@@PORTABLE_SCHEMA@@", contract.portable_schema)
        .replace("@@PORTABLE_LABEL@@", contract.portable_label)
        .replace("@@EXPLORER_LABEL@@", contract.explorer_label);
    if viewer.contains("@@")
        || viewer.len() > 1_048_576
        || sha256(viewer.as_bytes()) != contract.sha256
    {
        return Err(PortableExplorerError::Internal);
    }
    Ok(viewer.into_bytes())
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
) -> Result<PreparedOutputRoot, PortableExplorerError> {
    validate_output_root(authority, output, kind)?;
    match fs::symlink_metadata(output) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(output).map_err(|_| PortableExplorerError::Internal)?;
            sync_directory(output.parent().ok_or(PortableExplorerError::Internal)?)?;
        }
        Err(_) => return Err(PortableExplorerError::Internal),
    }
    let guard = PublicationGuard::capture(output)?;
    validate_output_root(authority, output, kind)?;
    guard.verify()?;
    Ok(PreparedOutputRoot { kind, guard })
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
    prepared: &PreparedOutputRoot,
    kind: OutputKind,
    files: &[OwnedFile],
) -> Result<(), PortableExplorerError> {
    if prepared.kind != kind {
        return Err(PortableExplorerError::Internal);
    }
    publish_files_with_guard(&prepared.guard, kind, files, || {})
}

#[cfg(test)]
fn publish_files_with(
    root: &Path,
    kind: OutputKind,
    files: &[OwnedFile],
    before_publication: impl FnOnce(),
) -> Result<(), PortableExplorerError> {
    verify_directory(root)?;
    let guard = PublicationGuard::capture(root)?;
    publish_files_with_guard(&guard, kind, files, before_publication)
}

fn publish_files_with_guard(
    guard: &PublicationGuard,
    kind: OutputKind,
    files: &[OwnedFile],
    before_publication: impl FnOnce(),
) -> Result<(), PortableExplorerError> {
    let root = guard.root.clone();
    verify_directory(&root)?;
    before_publication();
    guard.verify()?;
    let marker = root.join(kind.marker());
    let fresh = fs::read_dir(&root)
        .map_err(|_| PortableExplorerError::Internal)?
        .next()
        .is_none();
    if fresh {
        guard.verify()?;
        write_exclusive(&marker, kind.marker_bytes())?;
        guard.verify()?;
        sync_directory(&root)?;
    } else {
        verify_exact_file(&marker, kind.marker_bytes())?;
    }
    let result = (|| {
        for file in files {
            guard.verify()?;
            write_atomic_or_verify(&root, &file.path, &file.bytes)?;
            guard.verify()?;
        }
        sync_directory(&root)?;
        guard.verify()?;
        inspect_output(&root, kind)?;
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
        let _ = sync_directory(&root);
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
        #[cfg(unix)]
        let identity_matches = retained_unix_identity_matches(&self.root_identity, &root_metadata)
            && retained_unix_identity_matches(&self.parent_identity, &parent_metadata);
        #[cfg(windows)]
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

fn invalid_k1_input() -> PortableExplorerError {
    PortableExplorerError::K1Contract(K1ContractError::InvalidProjection)
}

fn invalid_r10_input() -> PortableExplorerError {
    PortableExplorerError::R10Contract(R10ContractError::InvalidProjection)
}

fn invalid_r11_input() -> PortableExplorerError {
    PortableExplorerError::R11Contract(R11ContractError::InvalidProjection)
}

fn invalid_r12_input() -> PortableExplorerError {
    PortableExplorerError::R12Contract(R12ContractError::InvalidProjection)
}

fn invalid_r13_input() -> PortableExplorerError {
    PortableExplorerError::R13Contract(R13ContractError::InvalidProjection)
}

fn invalid_r14_input() -> PortableExplorerError {
    PortableExplorerError::R14Contract(R14ContractError::InvalidProjection)
}

fn invalid_r15_input() -> PortableExplorerError {
    PortableExplorerError::R15Contract(R15ContractError::InvalidProjection)
}

fn invalid_r16_input() -> PortableExplorerError {
    PortableExplorerError::R16Contract(R16ContractError::InvalidProjection)
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
fn retained_unix_identity_matches(identity: &FileIdentity, metadata: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    identity.dev() == metadata.dev() && identity.ino() == metadata.ino()
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

#[derive(Clone, Copy, Eq, PartialEq)]
enum OutputKind {
    Portable,
    Explorer,
    K1Portable,
    K1Explorer,
    R10Portable,
    R10Explorer,
    R11Portable,
    R11Explorer,
    R12Portable,
    R12Explorer,
    R13Portable,
    R13Explorer,
    R14Portable,
    R14Explorer,
    R15Portable,
    R15Explorer,
    R16Portable,
    R16Explorer,
    R17Explorer,
}

impl OutputKind {
    const fn marker(self) -> &'static str {
        match self {
            Self::Portable => R8_PORTABLE_MARKER,
            Self::Explorer => R8_EXPLORER_MARKER,
            Self::K1Portable => K1_PORTABLE_MARKER,
            Self::K1Explorer => K1_EXPLORER_MARKER,
            Self::R10Portable => R10_PORTABLE_MARKER,
            Self::R10Explorer => R10_EXPLORER_MARKER,
            Self::R11Portable => R11_PORTABLE_MARKER,
            Self::R11Explorer => R11_EXPLORER_MARKER,
            Self::R12Portable => R12_PORTABLE_MARKER,
            Self::R12Explorer => R12_EXPLORER_MARKER,
            Self::R13Portable => R13_PORTABLE_MARKER,
            Self::R13Explorer => R13_EXPLORER_MARKER,
            Self::R14Portable => R14_PORTABLE_MARKER,
            Self::R14Explorer => R14_EXPLORER_MARKER,
            Self::R15Portable => R15_PORTABLE_MARKER,
            Self::R15Explorer => R15_EXPLORER_MARKER,
            Self::R16Portable => R16_PORTABLE_MARKER,
            Self::R16Explorer => R16_EXPLORER_MARKER,
            Self::R17Explorer => R17_EXPLORER_MARKER,
        }
    }

    const fn marker_bytes(self) -> &'static [u8] {
        match self {
            Self::Portable => PORTABLE_MARKER_BYTES,
            Self::Explorer => EXPLORER_MARKER_BYTES,
            Self::K1Portable => K1_PORTABLE_MARKER_BYTES,
            Self::K1Explorer => K1_EXPLORER_MARKER_BYTES,
            Self::R10Portable => R10_PORTABLE_MARKER_BYTES,
            Self::R10Explorer => R10_EXPLORER_MARKER_BYTES,
            Self::R11Portable => R11_PORTABLE_MARKER_BYTES,
            Self::R11Explorer => R11_EXPLORER_MARKER_BYTES,
            Self::R12Portable => R12_PORTABLE_MARKER_BYTES,
            Self::R12Explorer => R12_EXPLORER_MARKER_BYTES,
            Self::R13Portable => R13_PORTABLE_MARKER_BYTES,
            Self::R13Explorer => R13_EXPLORER_MARKER_BYTES,
            Self::R14Portable => R14_PORTABLE_MARKER_BYTES,
            Self::R14Explorer => R14_EXPLORER_MARKER_BYTES,
            Self::R15Portable => R15_PORTABLE_MARKER_BYTES,
            Self::R15Explorer => R15_EXPLORER_MARKER_BYTES,
            Self::R16Portable => R16_PORTABLE_MARKER_BYTES,
            Self::R16Explorer => R16_EXPLORER_MARKER_BYTES,
            Self::R17Explorer => R17_EXPLORER_MARKER_BYTES,
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
            Self::K1Portable => BTreeSet::from([K1_PORTABLE_MARKER, "portable-graph.json"]),
            Self::K1Explorer => BTreeSet::from([
                K1_EXPLORER_MARKER,
                "portable-graph.json",
                "index.html",
                "explorer-manifest.json",
            ]),
            Self::R10Portable => BTreeSet::from([R10_PORTABLE_MARKER, "portable-graph.json"]),
            Self::R10Explorer => BTreeSet::from([
                R10_EXPLORER_MARKER,
                "portable-graph.json",
                "index.html",
                "explorer-manifest.json",
            ]),
            Self::R11Portable => BTreeSet::from([R11_PORTABLE_MARKER, "portable-graph.json"]),
            Self::R11Explorer => BTreeSet::from([
                R11_EXPLORER_MARKER,
                "portable-graph.json",
                "index.html",
                "explorer-manifest.json",
            ]),
            Self::R12Portable => BTreeSet::from([R12_PORTABLE_MARKER, "portable-graph.json"]),
            Self::R12Explorer => BTreeSet::from([
                R12_EXPLORER_MARKER,
                "portable-graph.json",
                "index.html",
                "explorer-manifest.json",
            ]),
            Self::R13Portable => BTreeSet::from([R13_PORTABLE_MARKER, "portable-graph.json"]),
            Self::R13Explorer => BTreeSet::from([
                R13_EXPLORER_MARKER,
                "portable-graph.json",
                "index.html",
                "explorer-manifest.json",
            ]),
            Self::R14Portable => BTreeSet::from([R14_PORTABLE_MARKER, "portable-graph.json"]),
            Self::R14Explorer => BTreeSet::from([
                R14_EXPLORER_MARKER,
                "portable-graph.json",
                "index.html",
                "explorer-manifest.json",
            ]),
            Self::R15Portable => BTreeSet::from([R15_PORTABLE_MARKER, "portable-graph.json"]),
            Self::R15Explorer => BTreeSet::from([
                R15_EXPLORER_MARKER,
                "portable-graph.json",
                "index.html",
                "explorer-manifest.json",
            ]),
            Self::R16Portable => BTreeSet::from([R16_PORTABLE_MARKER, "portable-graph.json"]),
            Self::R16Explorer => BTreeSet::from([
                R16_EXPLORER_MARKER,
                "portable-graph.json",
                "index.html",
                "explorer-manifest.json",
            ]),
            Self::R17Explorer => BTreeSet::from([
                R17_EXPLORER_MARKER,
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
        OutputKind, OwnedFile, PortableExplorerError, publish_files_with,
        read_portable_graph_v3_with, read_portable_graph_v4_with, read_portable_graph_v5_with,
        read_portable_graph_v6_with, read_portable_graph_v7_with, read_portable_graph_v8_with,
        read_portable_graph_with,
    };

    const PORTABLE_FIXTURE_SOURCE: &[u8] =
        include_bytes!("../../../tests/fixtures/s4/portable-explorer-v1/portable-graph.json");

    #[test]
    fn pt_nfr_det_001_versioned_viewers_are_exact_and_deterministic() {
        let contracts = [
            super::R10_VERSIONED_VIEWER,
            super::R11_VERSIONED_VIEWER,
            super::R12_VERSIONED_VIEWER,
            super::R13_VERSIONED_VIEWER,
            super::R14_VERSIONED_VIEWER,
            super::R15_VERSIONED_VIEWER,
            super::R16_VERSIONED_VIEWER,
        ];
        for contract in contracts {
            let expected = super::render_versioned_viewer(contract).expect("render viewer");
            assert_eq!(super::sha256(&expected), contract.sha256);
            let expected_meta = format!(
                "<meta name=\"codenoesis-portable-schema\" content=\"{}\">",
                contract.portable_schema
            );
            let text = std::str::from_utf8(&expected).expect("viewer UTF-8");
            assert!(text.contains(&expected_meta));
            assert!(!text.contains("@@"));
            for _ in 0..50 {
                assert_eq!(
                    super::render_versioned_viewer(contract).expect("repeat viewer"),
                    expected
                );
            }
        }
    }

    #[test]
    fn sec_fr_exp_009_function_context_viewer_is_exact_and_offline() {
        let bytes = super::normalize_checkout_text(super::R17_FUNCTION_CONTEXT_VIEWER_BYTES)
            .expect("reviewed LF-only R17 viewer");
        assert_eq!(
            super::sha256(&bytes),
            super::R17_FUNCTION_CONTEXT_VIEWER_SHA256
        );
        let text = std::str::from_utf8(&bytes).expect("R17 viewer UTF-8");
        for required in [
            "codenoesis.portable-graph/v9",
            "codenoesis.function-context/v1",
            "rust.function",
            "rust.method",
            "declared_signature",
            "createElementNS",
            "textContent",
            "MAX_FILE_BYTES=268435456",
            "MAX_HISTORY=128",
            "MAX_SUBJECTS=256",
            "MAX_RELATIONSHIPS=512",
            "window.codenoesisR17",
            "#id=",
        ] {
            assert!(
                text.contains(required),
                "missing R17 viewer control: {required}"
            );
        }
        for forbidden in [
            "http://",
            "https://",
            "fetch(",
            "XMLHttpRequest",
            "WebSocket",
            "eval(",
            "new Function(",
            ".innerHTML",
            "document.write",
            "localStorage",
            "sessionStorage",
            "indexedDB",
            "document.cookie",
            "navigator.clipboard",
        ] {
            assert!(
                !text.contains(forbidden),
                "active R17 viewer token: {forbidden}"
            );
        }
        assert_eq!(
            OutputKind::R17Explorer.marker(),
            codenoesis_contracts::R17_EXPLORER_MARKER
        );
        assert_eq!(
            OutputKind::R17Explorer.marker_bytes(),
            b"{\"schema_version\":\"codenoesis.local-explorer-marker/v10\"}\n"
        );
        for _ in 0..50 {
            assert_eq!(
                super::sha256(&bytes),
                super::R17_FUNCTION_CONTEXT_VIEWER_SHA256
            );
        }
    }

    #[test]
    fn sec_nfr_sec_001_checkout_text_normalization_is_closed() {
        assert_eq!(
            super::normalize_checkout_text(b"first\r\nsecond\r\n"),
            Some(b"first\nsecond\n".to_vec())
        );
        assert_eq!(
            super::normalize_checkout_text(b"first\nsecond\n"),
            Some(b"first\nsecond\n".to_vec())
        );
        assert_eq!(super::normalize_checkout_text(b"first\rsecond"), None);
        assert_eq!(super::normalize_checkout_text(b"first\r\nsecond\n"), None);
    }

    #[test]
    fn race_nfr_sec_005_mutable_input_is_rejected() {
        let root = temporary_root("mutable-input");
        let input = root.join("portable-graph.json");
        let portable_fixture = super::normalize_checkout_text(PORTABLE_FIXTURE_SOURCE)
            .expect("normalize R8 race fixture checkout");
        fs::write(&input, &portable_fixture).expect("write R8 race input");
        let mutation_ran = Cell::new(false);
        let result = read_portable_graph_with(&input, || {
            let mut changed = portable_fixture;
            changed[0] ^= 1;
            fs::write(&input, changed).expect("rewrite R8 race input");
            mutation_ran.set(true);
        });
        assert!(mutation_ran.get(), "R8 mutable-input schedule did not run");
        match result {
            Err(PortableExplorerError::Contract(_)) => {}
            Err(error) => panic!("R8 mutable input returned the wrong error: {error:?}"),
            Ok(_) => panic!("R8 mutable input was accepted"),
        }
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

    #[test]
    fn race_fr_exp_003_r10_mutable_input_is_rejected() {
        let root = temporary_root("r10-mutable-input");
        let input = root.join("portable-graph.json");
        let initial = b"{}\n".to_vec();
        fs::write(&input, &initial).expect("write R10 race input");
        let mutation_ran = Cell::new(false);
        let result = read_portable_graph_v3_with(&input, || {
            fs::write(&input, b"[]\n").expect("rewrite R10 race input");
            mutation_ran.set(true);
        });
        assert!(mutation_ran.get(), "R10 mutable-input schedule did not run");
        assert!(matches!(result, Err(PortableExplorerError::R10Contract(_))));
        fs::remove_dir_all(root).expect("remove R10 race fixture");
    }

    #[test]
    fn race_fr_exp_003_r10_output_replacement_is_rejected_or_blocked() {
        let parent = temporary_root("r10-output-replacement");
        let output = parent.join("output");
        let attacker = parent.join("attacker");
        let displaced = parent.join("displaced");
        fs::create_dir(&output).expect("create R10 selected output");
        fs::create_dir(&attacker).expect("create R10 attacker output");
        let replaced = Cell::new(false);
        let result = publish_files_with(
            &output,
            OutputKind::R10Portable,
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
                            .expect("restore blocked R10 output replacement");
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
        fs::remove_dir_all(parent).expect("remove R10 output race fixture");
    }

    #[test]
    fn race_nfr_sec_005_r11_mutable_input_is_rejected() {
        let root = temporary_root("r11-mutable-input");
        let input = root.join("portable-graph.json");
        let initial = b"{}\n".to_vec();
        fs::write(&input, &initial).expect("write R11 race input");
        let mutation_ran = Cell::new(false);
        let result = read_portable_graph_v4_with(&input, || {
            fs::write(&input, b"[]\n").expect("rewrite R11 race input");
            mutation_ran.set(true);
        });
        assert!(mutation_ran.get(), "R11 mutable-input schedule did not run");
        assert!(matches!(result, Err(PortableExplorerError::R11Contract(_))));
        fs::remove_dir_all(root).expect("remove R11 race fixture");
    }

    #[test]
    fn race_nfr_sec_005_r11_output_replacement_is_rejected_or_blocked() {
        let parent = temporary_root("r11-output-replacement");
        let output = parent.join("output");
        let attacker = parent.join("attacker");
        let displaced = parent.join("displaced");
        fs::create_dir(&output).expect("create R11 selected output");
        fs::create_dir(&attacker).expect("create R11 attacker output");
        let replaced = Cell::new(false);
        let result = publish_files_with(
            &output,
            OutputKind::R11Portable,
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
                            .expect("restore blocked R11 output replacement");
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
        fs::remove_dir_all(parent).expect("remove R11 output race fixture");
    }

    #[test]
    fn race_nfr_sec_005_r12_mutable_input_is_rejected() {
        let root = temporary_root("r12-mutable-input");
        let input = root.join("portable-graph.json");
        let initial = b"{}\n".to_vec();
        fs::write(&input, &initial).expect("write R12 race input");
        let mutation_ran = Cell::new(false);
        let result = read_portable_graph_v5_with(&input, || {
            fs::write(&input, b"[]\n").expect("rewrite R12 race input");
            mutation_ran.set(true);
        });
        assert!(mutation_ran.get(), "R12 mutable-input schedule did not run");
        assert!(matches!(result, Err(PortableExplorerError::R12Contract(_))));
        fs::remove_dir_all(root).expect("remove R12 race fixture");
    }

    #[test]
    fn race_nfr_sec_005_r12_output_replacement_is_rejected_or_blocked() {
        let parent = temporary_root("r12-output-replacement");
        let output = parent.join("output");
        let attacker = parent.join("attacker");
        let displaced = parent.join("displaced");
        fs::create_dir(&output).expect("create R12 selected output");
        fs::create_dir(&attacker).expect("create R12 attacker output");
        let replaced = Cell::new(false);
        let result = publish_files_with(
            &output,
            OutputKind::R12Portable,
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
                            .expect("restore blocked R12 output replacement");
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
        fs::remove_dir_all(parent).expect("remove R12 output race fixture");
    }

    #[test]
    fn race_nfr_sec_005_r13_mutable_input_is_rejected() {
        let root = temporary_root("r13-mutable-input");
        let input = root.join("portable-graph.json");
        let initial = b"{}\n".to_vec();
        fs::write(&input, &initial).expect("write R13 race input");
        let mutation_ran = Cell::new(false);
        let result = read_portable_graph_v6_with(&input, || {
            fs::write(&input, b"[]\n").expect("rewrite R13 race input");
            mutation_ran.set(true);
        });
        assert!(mutation_ran.get(), "R13 mutable-input schedule did not run");
        assert!(matches!(result, Err(PortableExplorerError::R13Contract(_))));
        fs::remove_dir_all(root).expect("remove R13 race fixture");
    }

    #[test]
    fn race_nfr_sec_005_r13_output_replacement_is_rejected_or_blocked() {
        let parent = temporary_root("r13-output-replacement");
        let output = parent.join("output");
        let attacker = parent.join("attacker");
        let displaced = parent.join("displaced");
        fs::create_dir(&output).expect("create R13 selected output");
        fs::create_dir(&attacker).expect("create R13 attacker output");
        let replaced = Cell::new(false);
        let result = publish_files_with(
            &output,
            OutputKind::R13Portable,
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
                            .expect("restore blocked R13 output replacement");
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
        fs::remove_dir_all(parent).expect("remove R13 output race fixture");
    }

    #[test]
    fn race_nfr_sec_005_r14_mutable_input_is_rejected() {
        let root = temporary_root("r14-mutable-input");
        let input = root.join("portable-graph.json");
        let initial = b"{}\n".to_vec();
        fs::write(&input, &initial).expect("write R14 race input");
        let mutation_ran = Cell::new(false);
        let result = read_portable_graph_v7_with(&input, || {
            fs::write(&input, b"[]\n").expect("rewrite R14 race input");
            mutation_ran.set(true);
        });
        assert!(mutation_ran.get(), "R14 mutable-input schedule did not run");
        assert!(matches!(result, Err(PortableExplorerError::R14Contract(_))));
        fs::remove_dir_all(root).expect("remove R14 race fixture");
    }

    #[test]
    fn race_nfr_sec_005_r14_output_replacement_is_rejected_or_blocked() {
        let parent = temporary_root("r14-output-replacement");
        let output = parent.join("output");
        let attacker = parent.join("attacker");
        let displaced = parent.join("displaced");
        fs::create_dir(&output).expect("create R14 selected output");
        fs::create_dir(&attacker).expect("create R14 attacker output");
        let replaced = Cell::new(false);
        let result = publish_files_with(
            &output,
            OutputKind::R14Portable,
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
                            .expect("restore blocked R14 output replacement");
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
        fs::remove_dir_all(parent).expect("remove R14 output race fixture");
    }

    #[test]
    fn race_nfr_sec_005_r15_mutable_input_is_rejected() {
        let root = temporary_root("r15-mutable-input");
        let input = root.join("portable-graph.json");
        let initial = b"{}\n".to_vec();
        fs::write(&input, &initial).expect("write R15 race input");
        let mutation_ran = Cell::new(false);
        let result = read_portable_graph_v8_with(&input, || {
            fs::write(&input, b"[]\n").expect("rewrite R15 race input");
            mutation_ran.set(true);
        });
        assert!(mutation_ran.get(), "R15 mutable-input schedule did not run");
        assert!(matches!(result, Err(PortableExplorerError::R15Contract(_))));
        fs::remove_dir_all(root).expect("remove R15 race fixture");
    }

    #[test]
    fn race_nfr_sec_005_r15_output_replacement_is_rejected_or_blocked() {
        let parent = temporary_root("r15-output-replacement");
        let output = parent.join("output");
        let attacker = parent.join("attacker");
        let displaced = parent.join("displaced");
        fs::create_dir(&output).expect("create R15 selected output");
        fs::create_dir(&attacker).expect("create R15 attacker output");
        let replaced = Cell::new(false);
        let result = publish_files_with(
            &output,
            OutputKind::R15Portable,
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
                            .expect("restore blocked R15 output replacement");
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
        fs::remove_dir_all(parent).expect("remove R15 output race fixture");
    }

    fn temporary_root(label: &str) -> PathBuf {
        #[cfg(not(windows))]
        {
            let root = std::env::temp_dir().join(format!(
                "codenoesis-r8-unit-{label}-{}-{}",
                std::process::id(),
                super::TEMP_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            fs::create_dir(&root).expect("create R8 unit root");
            fs::canonicalize(root).expect("canonicalize R8 unit root")
        }
        #[cfg(windows)]
        {
            let workspace = std::env::current_dir().expect("resolve R8 unit workspace");
            let mut candidates = vec![("workspace", workspace.join("target"))];
            if let Some(volume_root) = workspace.ancestors().last() {
                if candidates
                    .iter()
                    .all(|(_, candidate)| candidate != volume_root)
                {
                    candidates.push(("windows-volume", volume_root.to_path_buf()));
                }
            }
            let sequence = super::TEMP_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let candidate_count = candidates.len();
            for (authority, candidate) in candidates {
                if super::verify_existing_components(&candidate).is_err() {
                    continue;
                }
                let root = candidate.join(format!(
                    "codenoesis-r8-unit-{authority}-{label}-{}-{sequence}",
                    std::process::id()
                ));
                if fs::create_dir(&root).is_err() {
                    continue;
                }
                if super::verify_existing_components(&root).is_ok() {
                    return root;
                }
                let _ = fs::remove_dir(&root);
            }
            panic!(
                "no validated R8 unit authority across {candidate_count} bounded candidates for {label}"
            );
        }
    }

    fn directory_is_empty(path: &Path) -> bool {
        fs::read_dir(path)
            .expect("read R8 race directory")
            .next()
            .is_none()
    }
}
