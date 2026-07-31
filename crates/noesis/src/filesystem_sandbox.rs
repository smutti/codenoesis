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
