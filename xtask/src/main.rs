//! Repository-maintenance entry point.

use std::env;
use std::io::{self, Write as _};
use std::process::ExitCode;

fn main() -> ExitCode {
    match xtask::distribution::run(env::args_os()) {
        Ok(stdout) => {
            if io::stdout().lock().write_all(&stdout).is_ok() {
                ExitCode::SUCCESS
            } else {
                emit_failure(&xtask::distribution::DistributionFailure::internal())
            }
        }
        Err(failure) => emit_failure(&failure),
    }
}

fn emit_failure(failure: &xtask::distribution::DistributionFailure) -> ExitCode {
    if let Ok(stderr) = failure.error().canonical_stderr() {
        let _ = io::stderr().lock().write_all(&stderr);
    }
    ExitCode::from(failure.exit_code())
}
