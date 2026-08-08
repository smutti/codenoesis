use std::collections::{BTreeMap, BTreeSet};

use seccompiler::{
    BpfProgram, SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition, SeccompFilter,
    SeccompRule, TargetArch,
};
use serde_json::Value;

use crate::SecurityBoundaryError;

const CLONE_THREAD: u64 = 65_536;
const POLICY: &str =
    include_str!("../../../tests/specifications/s0/seccomp-capability-deny-v1.json");

struct ArchitectureRules {
    target_arch: TargetArch,
    syscalls: Vec<(&'static str, i64)>,
    not_exposed: Vec<&'static str>,
}

pub(crate) fn install() -> Result<(), SecurityBoundaryError> {
    let architecture = architecture_rules();
    validate_policy(&architecture.syscalls, &architecture.not_exposed)?;

    let mut rules = architecture
        .syscalls
        .iter()
        .filter(|(name, _)| *name != "clone")
        .map(|(_, number)| (*number, Vec::new()))
        .collect::<BTreeMap<_, _>>();
    let clone_number = architecture
        .syscalls
        .iter()
        .find_map(|(name, number)| (*name == "clone").then_some(*number))
        .ok_or(SecurityBoundaryError)?;
    let clone_rule = SeccompRule::new(vec![
        SeccompCondition::new(
            0,
            SeccompCmpArgLen::Qword,
            SeccompCmpOp::MaskedEq(CLONE_THREAD),
            0,
        )
        .map_err(|_| SecurityBoundaryError)?,
    ])
    .map_err(|_| SecurityBoundaryError)?;
    rules.insert(clone_number, vec![clone_rule]);

    let filter = SeccompFilter::new(
        rules,
        SeccompAction::Allow,
        SeccompAction::Errno(1),
        architecture.target_arch,
    )
    .map_err(|_| SecurityBoundaryError)?;
    let program = BpfProgram::try_from(filter).map_err(|_| SecurityBoundaryError)?;
    seccompiler::apply_filter_all_threads(&program).map_err(|_| SecurityBoundaryError)
}

fn validate_policy(
    syscalls: &[(&'static str, i64)],
    not_exposed: &[&'static str],
) -> Result<(), SecurityBoundaryError> {
    let policy: Value = serde_json::from_str(POLICY).map_err(|_| SecurityBoundaryError)?;
    let policy_names = policy["rules"]
        .as_array()
        .ok_or(SecurityBoundaryError)?
        .iter()
        .flat_map(|rule| {
            rule["syscalls"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
        })
        .collect::<BTreeSet<_>>();
    let compiled_names = syscalls
        .iter()
        .map(|(name, _)| *name)
        .chain(not_exposed.iter().copied())
        .collect::<BTreeSet<_>>();
    if policy_names != compiled_names
        || policy["deny_action"]["errno"].as_u64() != Some(1)
        || policy["no_new_privileges"].as_bool() != Some(true)
    {
        return Err(SecurityBoundaryError);
    }
    Ok(())
}

#[cfg(target_arch = "x86_64")]
fn architecture_rules() -> ArchitectureRules {
    ArchitectureRules {
        target_arch: TargetArch::x86_64,
        syscalls: vec![
            ("execve", libc::SYS_execve),
            ("execveat", libc::SYS_execveat),
            ("fork", libc::SYS_fork),
            ("vfork", libc::SYS_vfork),
            ("clone", libc::SYS_clone),
            ("clone3", libc::SYS_clone3),
            ("socket", libc::SYS_socket),
            ("socketpair", libc::SYS_socketpair),
            ("connect", libc::SYS_connect),
            ("bind", libc::SYS_bind),
            ("listen", libc::SYS_listen),
            ("accept", libc::SYS_accept),
            ("accept4", libc::SYS_accept4),
            ("sendto", libc::SYS_sendto),
            ("sendmsg", libc::SYS_sendmsg),
            ("sendmmsg", libc::SYS_sendmmsg),
            ("recvfrom", libc::SYS_recvfrom),
            ("recvmsg", libc::SYS_recvmsg),
            ("recvmmsg", libc::SYS_recvmmsg),
            ("shutdown", libc::SYS_shutdown),
            ("io_uring_setup", libc::SYS_io_uring_setup),
            ("io_uring_enter", libc::SYS_io_uring_enter),
            ("io_uring_register", libc::SYS_io_uring_register),
        ],
        not_exposed: vec!["socketcall"],
    }
}

#[cfg(target_arch = "aarch64")]
fn architecture_rules() -> ArchitectureRules {
    ArchitectureRules {
        target_arch: TargetArch::aarch64,
        syscalls: vec![
            ("execve", libc::SYS_execve),
            ("execveat", libc::SYS_execveat),
            ("clone", libc::SYS_clone),
            ("clone3", libc::SYS_clone3),
            ("socket", libc::SYS_socket),
            ("socketpair", libc::SYS_socketpair),
            ("connect", libc::SYS_connect),
            ("bind", libc::SYS_bind),
            ("listen", libc::SYS_listen),
            ("accept", libc::SYS_accept),
            ("accept4", libc::SYS_accept4),
            ("sendto", libc::SYS_sendto),
            ("sendmsg", libc::SYS_sendmsg),
            ("sendmmsg", libc::SYS_sendmmsg),
            ("recvfrom", libc::SYS_recvfrom),
            ("recvmsg", libc::SYS_recvmsg),
            ("recvmmsg", libc::SYS_recvmmsg),
            ("shutdown", libc::SYS_shutdown),
            ("io_uring_setup", libc::SYS_io_uring_setup),
            ("io_uring_enter", libc::SYS_io_uring_enter),
            ("io_uring_register", libc::SYS_io_uring_register),
        ],
        not_exposed: vec!["fork", "vfork", "socketcall"],
    }
}
