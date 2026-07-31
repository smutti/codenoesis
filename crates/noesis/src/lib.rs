//! Composition support for the `CodeNoesis` command-line interface.

use std::ffi::OsStr;
use std::path::PathBuf;

pub mod generated_docs;

#[cfg(target_os = "linux")]
mod filesystem_sandbox;

#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod sandbox;

/// Installs the approved S0 process and network capability boundary.
///
/// # Errors
///
/// Returns an opaque error when the running Linux architecture is unsupported,
/// the embedded policy no longer matches the compiled rules, or the kernel
/// rejects the filter.
#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
pub fn install_s0_security_boundary() -> Result<(), SecurityBoundaryError> {
    sandbox::install()
}

/// Rejects Linux architectures outside the ratified S0 seccomp policy.
///
/// # Errors
///
/// Always returns an error because the running architecture has no approved
/// syscall mapping.
#[cfg(all(
    target_os = "linux",
    not(any(target_arch = "x86_64", target_arch = "aarch64"))
))]
pub const fn install_s0_security_boundary() -> Result<(), SecurityBoundaryError> {
    Err(SecurityBoundaryError)
}

/// Confirms that the S0 security boundary is a Linux-only normative control.
///
/// # Errors
///
/// This portability implementation is infallible and installs no substitute
/// control on non-Linux systems.
#[cfg(not(target_os = "linux"))]
pub const fn install_s0_security_boundary() -> Result<(), SecurityBoundaryError> {
    Ok(())
}

/// Installs the S1 read-only repository-root filesystem boundary on Linux.
///
/// # Errors
///
/// Returns an opaque error when Landlock is unavailable or cannot fully enforce
/// the approved filesystem rights.
#[cfg(target_os = "linux")]
pub fn install_s1_filesystem_boundary(repository: &OsStr) -> Result<(), SecurityBoundaryError> {
    filesystem_sandbox::install(repository)
}

/// Confirms that normative S1 filesystem confinement is Linux-only.
///
/// # Errors
///
/// This portability implementation is infallible and installs no substitute
/// control on non-Linux systems.
#[cfg(not(target_os = "linux"))]
pub const fn install_s1_filesystem_boundary(
    _repository: &OsStr,
) -> Result<(), SecurityBoundaryError> {
    Ok(())
}

/// Installs the S6 read-only manifest and repository-root boundary on Linux.
///
/// # Errors
///
/// Returns an opaque error when Landlock cannot fully confine all authorized
/// input roots without granting filesystem writes.
#[cfg(target_os = "linux")]
pub fn install_s6_filesystem_boundary(
    workspace_manifest: &OsStr,
    repository_roots: &[PathBuf],
) -> Result<(), SecurityBoundaryError> {
    filesystem_sandbox::install_read_only_paths(workspace_manifest, repository_roots)
}

/// Confirms that normative S6 filesystem confinement is Linux-only.
///
/// # Errors
///
/// This portability implementation is infallible and installs no substitute
/// control on non-Linux systems.
#[cfg(not(target_os = "linux"))]
pub const fn install_s6_filesystem_boundary(
    _workspace_manifest: &OsStr,
    _repository_roots: &[PathBuf],
) -> Result<(), SecurityBoundaryError> {
    Ok(())
}

/// Installs the S3 read-only repository and writable-store boundary on Linux.
///
/// # Errors
///
/// Returns an opaque error when Landlock is unavailable or cannot fully
/// enforce the approved filesystem rights.
#[cfg(target_os = "linux")]
pub fn install_s3_filesystem_boundary(
    repository: &OsStr,
    store: &OsStr,
) -> Result<(), SecurityBoundaryError> {
    filesystem_sandbox::install_with_store(repository, store)
}

/// Confirms that normative S3 filesystem confinement is Linux-only.
///
/// # Errors
///
/// This portability implementation is infallible and installs no substitute
/// control on non-Linux systems.
#[cfg(not(target_os = "linux"))]
pub const fn install_s3_filesystem_boundary(
    _repository: &OsStr,
    _store: &OsStr,
) -> Result<(), SecurityBoundaryError> {
    Ok(())
}

/// Installs the S4 read-only store and writable generated-docs boundary.
///
/// # Errors
///
/// Returns an opaque error when Linux Landlock cannot fully enforce the
/// approved rights.
#[cfg(target_os = "linux")]
pub fn install_s4_docs_filesystem_boundary(
    store: &OsStr,
    documents: &OsStr,
) -> Result<(), SecurityBoundaryError> {
    filesystem_sandbox::install_with_documents(store, documents, true)
}

/// Confirms that normative S4 filesystem confinement is Linux-only.
///
/// # Errors
///
/// This portability implementation is infallible.
#[cfg(not(target_os = "linux"))]
pub const fn install_s4_docs_filesystem_boundary(
    _store: &OsStr,
    _documents: &OsStr,
) -> Result<(), SecurityBoundaryError> {
    Ok(())
}

/// Installs the S4 read-only store and generated-docs query boundary.
///
/// # Errors
///
/// Returns an opaque error when Linux Landlock cannot fully enforce the
/// approved rights.
#[cfg(target_os = "linux")]
pub fn install_s4_query_filesystem_boundary(
    store: &OsStr,
    documents: &OsStr,
) -> Result<(), SecurityBoundaryError> {
    filesystem_sandbox::install_with_documents(store, documents, false)
}

/// Confirms that normative S4 filesystem confinement is Linux-only.
///
/// # Errors
///
/// This portability implementation is infallible.
#[cfg(not(target_os = "linux"))]
pub const fn install_s4_query_filesystem_boundary(
    _store: &OsStr,
    _documents: &OsStr,
) -> Result<(), SecurityBoundaryError> {
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SecurityBoundaryError;
