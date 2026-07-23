use std::ffi::OsStr;
use std::path::Path;

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
