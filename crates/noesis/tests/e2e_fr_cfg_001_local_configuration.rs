use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const EXPECTED_EMBEDDED: &[u8] = include_bytes!(
    "../../../tests/specifications/g1/local-cli-distribution-v1/expected-config-embedded.json"
);
const EXPECTED_EXPLICIT: &[u8] = include_bytes!(
    "../../../tests/specifications/g1/local-cli-distribution-v1/expected-config-explicit.json"
);

#[test]
fn e2e_fr_cfg_001_validates_embedded_default() {
    let embedded = run_noesis(&["config", "validate", "--format", "json"]);
    fail_if_expected_checkpoint_red(&embedded);
    assert_success(&embedded, EXPECTED_EMBEDDED);

    let configuration = specification_path("default-config.json");
    let configuration_text = configuration.to_string_lossy().into_owned();
    let explicit = run_noesis(&[
        "config",
        "validate",
        "--file",
        &configuration_text,
        "--format",
        "json",
    ]);
    assert_success(&explicit, EXPECTED_EXPLICIT);

    let profiled = run_noesis(&[
        "--config",
        &configuration_text,
        "profile",
        "--id",
        "local-experimental-r17",
        "--format",
        "json",
    ]);
    assert_eq!(profiled.status.code(), Some(0));
    assert!(profiled.stderr.is_empty());
    assert!(!profiled.stdout.is_empty());

    for schedule in 0..10 {
        let replay = run_noesis(&["config", "validate", "--format", "json"]);
        assert_success(&replay, EXPECTED_EMBEDDED);
        assert_eq!(replay.stdout, embedded.stdout, "schedule {schedule}");
    }
}

fn specification_path(leaf: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/specifications/g1/local-cli-distribution-v1")
        .join(leaf)
}

fn run_noesis(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_noesis"))
        .args(arguments)
        .env("CODENOESIS_G1_PRIVATE_CANARY", "CODENOESIS_PRIVATE_CANARY")
        .env("NOESIS_CONFIG", "/private/ambient/config.json")
        .env("HOME", "/private/absolute/root")
        .env("HTTPS_PROXY", "https://private.invalid")
        .output()
        .expect("run noesis")
}

fn assert_success(output: &Output, expected: &[u8]) {
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, canonical_lf_golden(expected));
    assert!(output.stdout.len() <= 65_536);
    let text = String::from_utf8_lossy(&output.stdout);
    for canary in [
        "CODENOESIS_PRIVATE_CANARY",
        "/private/ambient/config.json",
        "/private/absolute/root",
        "https://private.invalid",
    ] {
        assert!(!text.contains(canary), "private canary leaked: {canary}");
    }
}

fn canonical_lf_golden(checked_out: &[u8]) -> Vec<u8> {
    let body = checked_out
        .strip_suffix(b"\r\n")
        .or_else(|| checked_out.strip_suffix(b"\n"))
        .expect("golden has one final LF or CRLF");
    assert!(!body.contains(&b'\r'));
    assert!(!body.contains(&b'\n'));

    let mut canonical = Vec::with_capacity(body.len() + 1);
    canonical.extend_from_slice(body);
    canonical.push(b'\n');
    canonical
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
            "expected Red: the exact base rejects the unimplemented config command as input.invalid_revision"
        );
    }
}
