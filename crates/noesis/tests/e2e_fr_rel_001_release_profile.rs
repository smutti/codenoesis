use std::process::{Command, Output};

#[cfg(all(target_arch = "x86_64", target_os = "linux", target_env = "gnu"))]
const EXPECTED_REPORT: Option<&[u8]> = Some(include_bytes!(
    "../../../tests/specifications/g0/release-profile-v1/expected-x86_64-unknown-linux-gnu.json"
));
#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
const EXPECTED_REPORT: Option<&[u8]> = Some(include_bytes!(
    "../../../tests/specifications/g0/release-profile-v1/expected-aarch64-apple-darwin.json"
));
#[cfg(all(target_arch = "x86_64", target_os = "windows", target_env = "msvc"))]
const EXPECTED_REPORT: Option<&[u8]> = Some(include_bytes!(
    "../../../tests/specifications/g0/release-profile-v1/expected-x86_64-pc-windows-msvc.json"
));
#[cfg(not(any(
    all(target_arch = "x86_64", target_os = "linux", target_env = "gnu"),
    all(target_arch = "aarch64", target_os = "macos"),
    all(target_arch = "x86_64", target_os = "windows", target_env = "msvc")
)))]
const EXPECTED_REPORT: Option<&[u8]> = None;

#[test]
fn e2e_fr_rel_001_reports_bound_profile() {
    let first = run_profile(&["--id", "local-experimental-r17", "--format", "json"]);
    fail_if_expected_checkpoint_red(&first);
    let Some(expected) = EXPECTED_REPORT else {
        assert_error(&first, "profile.unsupported_platform");
        return;
    };

    assert_success(&first, expected);
    let reordered = run_profile(&["--format", "json", "--id", "local-experimental-r17"]);
    assert_success(&reordered, expected);
    assert_eq!(reordered.stdout, first.stdout);

    for schedule in 0..10 {
        let output = run_profile(&["--id", "local-experimental-r17", "--format", "json"]);
        assert_success(&output, expected);
        assert_eq!(output.stdout, first.stdout, "schedule {schedule}");
    }

    let text = String::from_utf8(first.stdout).expect("profile stdout is UTF-8");
    for canary in [
        "CODENOESIS_G0_PRIVATE_CANARY",
        "https://private.invalid",
        "/private/absolute/root",
    ] {
        assert!(!text.contains(canary), "private canary leaked: {canary}");
    }

    assert_error(
        &run_profile(&["--id", "unknown-profile", "--format", "json"]),
        "profile.unknown",
    );
    assert_error(
        &run_profile(&["--id", "local-experimental-r17", "--format", "text"]),
        "input.invalid_format",
    );
    assert_error(
        &run_profile(&[
            "--id",
            "local-experimental-r17",
            "--id",
            "local-experimental-r17",
            "--format",
            "json",
        ]),
        "input.invalid_profile_command",
    );
}

fn run_profile(arguments: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_noesis"));
    command.arg("profile").args(arguments);
    command
        .env("CODENOESIS_G0_CANARY", "CODENOESIS_G0_PRIVATE_CANARY")
        .env("HOME", "/private/absolute/root")
        .env("HTTPS_PROXY", "https://private.invalid")
        .output()
        .expect("run noesis profile")
}

fn assert_success(output: &Output, expected: &[u8]) {
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, expected);
    assert!(output.stdout.len() <= 65_536);
}

fn fail_if_expected_checkpoint_red(output: &Output) {
    if output.status.code() != Some(2) || output.stderr.len() != 149 {
        return;
    }
    let Ok(error) = serde_json::from_slice::<serde_json::Value>(&output.stderr) else {
        return;
    };
    if error["schema_version"] == "codenoesis.error/v1" && error["code"] == "input.invalid_revision"
    {
        assert!(output.stdout.is_empty());
        panic!(
            "expected Red: the exact base rejects the unimplemented profile command as input.invalid_revision"
        );
    }
}

fn assert_error(output: &Output, code: &str) {
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error: serde_json::Value = serde_json::from_slice(&output.stderr).expect("parse ErrorV25");
    assert_eq!(error["schema_version"], "codenoesis.error/v25");
    assert_eq!(error["code"], code);
    assert_eq!(error["retryable"], false);
}
