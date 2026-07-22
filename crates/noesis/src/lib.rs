//! Composition support for the `CodeNoesis` command-line interface.

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SecurityBoundaryError;
