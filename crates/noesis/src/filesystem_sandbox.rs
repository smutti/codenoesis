use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use landlock::{
    ABI, Access, AccessFs, CompatLevel, Compatible, PathBeneath, PathFd, Ruleset, RulesetAttr,
    RulesetCreatedAttr, RulesetStatus,
};

use crate::SecurityBoundaryError;

pub(crate) fn install(repository: &OsStr) -> Result<(), SecurityBoundaryError> {
    let abi = ABI::V3;
    let allowed = AccessFs::ReadFile | AccessFs::ReadDir;
    let status = Ruleset::default()
        .set_compatibility(CompatLevel::HardRequirement)
        .handle_access(AccessFs::from_all(abi))
        .map_err(|_| SecurityBoundaryError)?
        .create()
        .map_err(|_| SecurityBoundaryError)?
        .add_rule(PathBeneath::new(
            PathFd::new(Path::new(repository)).map_err(|_| SecurityBoundaryError)?,
            allowed,
        ))
        .map_err(|_| SecurityBoundaryError)?
        .restrict_self()
        .map_err(|_| SecurityBoundaryError)?;
    if status.ruleset != RulesetStatus::FullyEnforced || !status.no_new_privs {
        return Err(SecurityBoundaryError);
    }
    Ok(())
}

pub(crate) fn install_read_only_paths(
    workspace_manifest: &OsStr,
    repository_roots: &[PathBuf],
) -> Result<(), SecurityBoundaryError> {
    let abi = ABI::V3;
    let read_file = AccessFs::ReadFile;
    let read_tree = read_file | AccessFs::ReadDir;
    let mut ruleset = Ruleset::default()
        .set_compatibility(CompatLevel::HardRequirement)
        .handle_access(AccessFs::from_all(abi))
        .map_err(|_| SecurityBoundaryError)?
        .create()
        .map_err(|_| SecurityBoundaryError)?
        .add_rule(PathBeneath::new(
            PathFd::new(Path::new(workspace_manifest)).map_err(|_| SecurityBoundaryError)?,
            read_file,
        ))
        .map_err(|_| SecurityBoundaryError)?;
    for root in repository_roots {
        ruleset = ruleset
            .add_rule(PathBeneath::new(
                PathFd::new(root).map_err(|_| SecurityBoundaryError)?,
                read_tree,
            ))
            .map_err(|_| SecurityBoundaryError)?;
    }
    let status = ruleset.restrict_self().map_err(|_| SecurityBoundaryError)?;
    if status.ruleset != RulesetStatus::FullyEnforced || !status.no_new_privs {
        return Err(SecurityBoundaryError);
    }
    Ok(())
}

#[allow(dead_code)]
pub(crate) fn install_with_compiler_index(
    repository: &OsStr,
    binding: &Path,
    artifact: &Path,
) -> Result<(), SecurityBoundaryError> {
    let abi = ABI::V3;
    let read_file = AccessFs::ReadFile;
    let read_tree = read_file | AccessFs::ReadDir;
    let status = Ruleset::default()
        .set_compatibility(CompatLevel::HardRequirement)
        .handle_access(AccessFs::from_all(abi))
        .map_err(|_| SecurityBoundaryError)?
        .create()
        .map_err(|_| SecurityBoundaryError)?
        .add_rule(PathBeneath::new(
            PathFd::new(Path::new(repository)).map_err(|_| SecurityBoundaryError)?,
            read_tree,
        ))
        .map_err(|_| SecurityBoundaryError)?
        .add_rule(PathBeneath::new(
            PathFd::new(binding).map_err(|_| SecurityBoundaryError)?,
            read_file,
        ))
        .map_err(|_| SecurityBoundaryError)?
        .add_rule(PathBeneath::new(
            PathFd::new(artifact).map_err(|_| SecurityBoundaryError)?,
            read_file,
        ))
        .map_err(|_| SecurityBoundaryError)?
        .restrict_self()
        .map_err(|_| SecurityBoundaryError)?;
    if status.ruleset != RulesetStatus::FullyEnforced || !status.no_new_privs {
        return Err(SecurityBoundaryError);
    }
    Ok(())
}

#[allow(dead_code)]
pub(crate) fn install_with_compiler_index_and_roots(
    repository: &OsStr,
    binding: &Path,
    artifact: &Path,
    manifest: Option<&OsStr>,
    nested_roots: &[PathBuf],
) -> Result<(), SecurityBoundaryError> {
    let abi = ABI::V3;
    let read_file = AccessFs::ReadFile;
    let read_tree = read_file | AccessFs::ReadDir;
    let mut ruleset = Ruleset::default()
        .set_compatibility(CompatLevel::HardRequirement)
        .handle_access(AccessFs::from_all(abi))
        .map_err(|_| SecurityBoundaryError)?
        .create()
        .map_err(|_| SecurityBoundaryError)?
        .add_rule(PathBeneath::new(
            PathFd::new(Path::new(repository)).map_err(|_| SecurityBoundaryError)?,
            read_tree,
        ))
        .map_err(|_| SecurityBoundaryError)?
        .add_rule(PathBeneath::new(
            PathFd::new(binding).map_err(|_| SecurityBoundaryError)?,
            read_file,
        ))
        .map_err(|_| SecurityBoundaryError)?
        .add_rule(PathBeneath::new(
            PathFd::new(artifact).map_err(|_| SecurityBoundaryError)?,
            read_file,
        ))
        .map_err(|_| SecurityBoundaryError)?;
    if let Some(manifest) = manifest {
        ruleset = ruleset
            .add_rule(PathBeneath::new(
                PathFd::new(Path::new(manifest)).map_err(|_| SecurityBoundaryError)?,
                read_file,
            ))
            .map_err(|_| SecurityBoundaryError)?;
    }
    for root in nested_roots {
        ruleset = ruleset
            .add_rule(PathBeneath::new(
                PathFd::new(root).map_err(|_| SecurityBoundaryError)?,
                read_tree,
            ))
            .map_err(|_| SecurityBoundaryError)?;
    }
    let status = ruleset.restrict_self().map_err(|_| SecurityBoundaryError)?;
    if status.ruleset != RulesetStatus::FullyEnforced || !status.no_new_privs {
        return Err(SecurityBoundaryError);
    }
    Ok(())
}

pub(crate) fn install_with_store(
    repository: &OsStr,
    store: &OsStr,
) -> Result<(), SecurityBoundaryError> {
    let abi = ABI::V3;
    let repository_allowed = AccessFs::ReadFile | AccessFs::ReadDir;
    let store_allowed = repository_allowed
        | AccessFs::WriteFile
        | AccessFs::RemoveDir
        | AccessFs::RemoveFile
        | AccessFs::MakeDir
        | AccessFs::MakeReg
        | AccessFs::Refer
        | AccessFs::Truncate;
    let status = Ruleset::default()
        .set_compatibility(CompatLevel::HardRequirement)
        .handle_access(AccessFs::from_all(abi))
        .map_err(|_| SecurityBoundaryError)?
        .create()
        .map_err(|_| SecurityBoundaryError)?
        .add_rule(PathBeneath::new(
            PathFd::new(Path::new(repository)).map_err(|_| SecurityBoundaryError)?,
            repository_allowed,
        ))
        .map_err(|_| SecurityBoundaryError)?
        .add_rule(PathBeneath::new(
            PathFd::new(Path::new(store)).map_err(|_| SecurityBoundaryError)?,
            store_allowed,
        ))
        .map_err(|_| SecurityBoundaryError)?
        .restrict_self()
        .map_err(|_| SecurityBoundaryError)?;
    if status.ruleset != RulesetStatus::FullyEnforced || !status.no_new_privs {
        return Err(SecurityBoundaryError);
    }
    Ok(())
}

pub(crate) fn install_with_store_and_roots(
    repository: &OsStr,
    store: &OsStr,
    manifest: Option<&OsStr>,
    nested_roots: &[PathBuf],
) -> Result<(), SecurityBoundaryError> {
    let abi = ABI::V3;
    let read_file = AccessFs::ReadFile;
    let read_tree = read_file | AccessFs::ReadDir;
    let store_allowed = read_tree
        | AccessFs::WriteFile
        | AccessFs::RemoveDir
        | AccessFs::RemoveFile
        | AccessFs::MakeDir
        | AccessFs::MakeReg
        | AccessFs::Refer
        | AccessFs::Truncate;
    let mut ruleset = Ruleset::default()
        .set_compatibility(CompatLevel::HardRequirement)
        .handle_access(AccessFs::from_all(abi))
        .map_err(|_| SecurityBoundaryError)?
        .create()
        .map_err(|_| SecurityBoundaryError)?
        .add_rule(PathBeneath::new(
            PathFd::new(Path::new(repository)).map_err(|_| SecurityBoundaryError)?,
            read_tree,
        ))
        .map_err(|_| SecurityBoundaryError)?
        .add_rule(PathBeneath::new(
            PathFd::new(Path::new(store)).map_err(|_| SecurityBoundaryError)?,
            store_allowed,
        ))
        .map_err(|_| SecurityBoundaryError)?;
    if let Some(manifest) = manifest {
        ruleset = ruleset
            .add_rule(PathBeneath::new(
                PathFd::new(Path::new(manifest)).map_err(|_| SecurityBoundaryError)?,
                read_file,
            ))
            .map_err(|_| SecurityBoundaryError)?;
    }
    for root in nested_roots {
        ruleset = ruleset
            .add_rule(PathBeneath::new(
                PathFd::new(root).map_err(|_| SecurityBoundaryError)?,
                read_tree,
            ))
            .map_err(|_| SecurityBoundaryError)?;
    }
    let status = ruleset.restrict_self().map_err(|_| SecurityBoundaryError)?;
    if status.ruleset != RulesetStatus::FullyEnforced || !status.no_new_privs {
        return Err(SecurityBoundaryError);
    }
    Ok(())
}

pub(crate) fn install_with_documents(
    store: &OsStr,
    documents: &OsStr,
    writable: bool,
) -> Result<(), SecurityBoundaryError> {
    let abi = ABI::V3;
    let read_only = AccessFs::ReadFile | AccessFs::ReadDir;
    let document_access = if writable {
        read_only
            | AccessFs::WriteFile
            | AccessFs::RemoveDir
            | AccessFs::RemoveFile
            | AccessFs::MakeDir
            | AccessFs::MakeReg
            | AccessFs::Refer
            | AccessFs::Truncate
    } else {
        read_only
    };
    let status = Ruleset::default()
        .set_compatibility(CompatLevel::HardRequirement)
        .handle_access(AccessFs::from_all(abi))
        .map_err(|_| SecurityBoundaryError)?
        .create()
        .map_err(|_| SecurityBoundaryError)?
        .add_rule(PathBeneath::new(
            PathFd::new(Path::new(store)).map_err(|_| SecurityBoundaryError)?,
            read_only,
        ))
        .map_err(|_| SecurityBoundaryError)?
        .add_rule(PathBeneath::new(
            PathFd::new(Path::new(documents)).map_err(|_| SecurityBoundaryError)?,
            document_access,
        ))
        .map_err(|_| SecurityBoundaryError)?
        .restrict_self()
        .map_err(|_| SecurityBoundaryError)?;
    if status.ruleset != RulesetStatus::FullyEnforced || !status.no_new_privs {
        return Err(SecurityBoundaryError);
    }
    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::install_with_compiler_index;

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

        install_with_compiler_index(repository.as_os_str(), &binding, &artifact)
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
