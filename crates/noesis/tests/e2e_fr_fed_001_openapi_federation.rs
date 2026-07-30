use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use sha2::{Digest, Sha256};

const PRE_S6_ERROR_SHA256: &str =
    "6441e0037f864d2fae4a60e6355e4a85b26b00d5e4e24c59ffeb5fe9c6f3859f";

#[test]
fn e2e_fr_fed_001_openapi_federation() {
    let fixture = fixture_root();
    let output = federate(&fixture.join("workspace.json"));

    assert_success_or_expected_red(&output);
    assert!(
        output.stderr.is_empty(),
        "successful federation wrote stderr"
    );
    assert_eq!(
        output.stdout,
        fs::read(fixture.join("expected-federation-report.json"))
            .expect("read reviewed S6 federation report"),
        "S6 federation report differs from the reviewed artifact"
    );
}

fn federate(workspace_manifest: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_noesis"))
        .args(["federate", "--workspace-manifest"])
        .arg(workspace_manifest)
        .args(["--profile", "standard-local-s6", "--format", "json"])
        .output()
        .expect("launch S6 federation subject")
}

fn assert_success_or_expected_red(output: &Output) {
    if output.status.success() {
        return;
    }

    assert_eq!(
        output.status.code(),
        Some(2),
        "pre-S6 subject must fail only at the unrecognized command boundary"
    );
    assert!(output.stdout.is_empty(), "pre-S6 subject wrote stdout");
    assert_eq!(output.stderr.len(), 149, "pre-S6 ErrorV2 length changed");
    let stderr_sha256 = Sha256::digest(&output.stderr)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(
        stderr_sha256, PRE_S6_ERROR_SHA256,
        "pre-S6 ErrorV2 bytes changed"
    );
    assert_eq!(
        output.stderr,
        b"{\"code\":\"input.invalid_revision\",\"context\":{},\"message\":\"invalid revision\",\"retryable\":false,\"schema_version\":\"codenoesis.error/v2\",\"stage\":\"input\"}\n"
    );
    panic!("expected S6 federation success; observed the approved pre-S6 command-boundary Red");
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/s6/openapi-federation-v1")
}
