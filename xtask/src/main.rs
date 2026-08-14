//! Repository-maintenance entry point.

use std::env;
use std::ffi::OsString;
use std::io::{self, Write as _};
use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments = env::args_os().collect::<Vec<_>>();
    if xtask::release::is_release_command(arguments.get(1)) {
        run_release(arguments)
    } else if xtask::upgrade::is_upgrade_command(arguments.get(1)) {
        run_upgrade(arguments)
    } else {
        run_distribution(arguments)
    }
}

fn run_release(arguments: Vec<OsString>) -> ExitCode {
    match xtask::release::run(arguments) {
        Ok(stdout) => {
            if io::stdout().lock().write_all(&stdout).is_ok() {
                ExitCode::SUCCESS
            } else {
                emit_release_failure(&xtask::release::ReleaseFailure::internal())
            }
        }
        Err(failure) => emit_release_failure(&failure),
    }
}

fn run_distribution(arguments: Vec<OsString>) -> ExitCode {
    match xtask::distribution::run(arguments) {
        Ok(stdout) => {
            if io::stdout().lock().write_all(&stdout).is_ok() {
                ExitCode::SUCCESS
            } else {
                emit_distribution_failure(&xtask::distribution::DistributionFailure::internal())
            }
        }
        Err(failure) => emit_distribution_failure(&failure),
    }
}

fn run_upgrade(arguments: Vec<OsString>) -> ExitCode {
    match xtask::upgrade::run(arguments) {
        Ok(stdout) => {
            if io::stdout().lock().write_all(&stdout).is_ok() {
                ExitCode::SUCCESS
            } else {
                emit_upgrade_failure(&xtask::upgrade::UpgradeFailure::internal())
            }
        }
        Err(failure) => emit_upgrade_failure(&failure),
    }
}

fn emit_distribution_failure(failure: &xtask::distribution::DistributionFailure) -> ExitCode {
    if let Ok(stderr) = failure.error().canonical_stderr() {
        let _ = io::stderr().lock().write_all(&stderr);
    }
    ExitCode::from(failure.exit_code())
}

fn emit_upgrade_failure(failure: &xtask::upgrade::UpgradeFailure) -> ExitCode {
    if let Ok(stderr) = failure.error().canonical_stderr() {
        let _ = io::stderr().lock().write_all(&stderr);
    }
    ExitCode::from(failure.exit_code())
}

fn emit_release_failure(failure: &xtask::release::ReleaseFailure) -> ExitCode {
    if let Ok(stderr) = failure.error().canonical_stderr() {
        let _ = io::stderr().lock().write_all(&stderr);
    }
    ExitCode::from(failure.exit_code())
}
