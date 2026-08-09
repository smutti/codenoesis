mod support;

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

use support::parse_single_document;
use support::s4_k1::{MaterializedCallableRepository, REPOSITORY_ID};

const OUTPUT_CAPACITY_PROFILE: &str = "local-snapshot-64m-v1";

#[test]
fn e2e_fr_ext_012_explicit_output_capacity_profile() {
    let standard = MaterializedCallableRepository::fixture();
    let standard_output = standard.scan();
    assert_success(&standard_output, "standard K1 scan");

    let large = MaterializedCallableRepository::fixture();
    let mut command = k1_scan_command(&large, &large.store);
    command.args(["--output-capacity-profile", OUTPUT_CAPACITY_PROFILE]);
    let large_output = command.output().expect("launch large-output K1 scan");
    assert_success(&large_output, "large-output K1 scan");

    let standard_snapshot = parse_single_document(&standard_output.stdout);
    let large_snapshot = parse_single_document(&large_output.stdout);
    assert_eq!(standard_snapshot["semantic"], large_snapshot["semantic"]);
    assert_eq!(
        standard_snapshot["semantic"]["configuration"],
        large_snapshot["semantic"]["configuration"]
    );
    assert_eq!(
        standard_snapshot["semantic_hash"],
        large_snapshot["semantic_hash"]
    );
}

#[test]
fn conf_fr_cli_001_output_capacity_composition_is_closed() {
    let repository = MaterializedCallableRepository::fixture();

    let unknown_store = repository.root.join("unknown-store");
    let mut unknown = k1_scan_command(&repository, &unknown_store);
    unknown.args(["--output-capacity-profile", "unknown"]);
    assert_unsupported(
        &unknown.output().expect("run unknown profile"),
        &unknown_store,
    );

    let duplicate_store = repository.root.join("duplicate-store");
    let mut duplicate = k1_scan_command(&repository, &duplicate_store);
    duplicate.args(["--output-capacity-profile", OUTPUT_CAPACITY_PROFILE]);
    duplicate.args(["--output-capacity-profile", OUTPUT_CAPACITY_PROFILE]);
    assert_unsupported(
        &duplicate.output().expect("run duplicate profile"),
        &duplicate_store,
    );

    let missing_store = repository.root.join("missing-store");
    let mut missing = k1_scan_command(&repository, &missing_store);
    missing.arg("--output-capacity-profile");
    assert_unsupported(
        &missing.output().expect("run missing profile"),
        &missing_store,
    );

    let non_k1_store = repository.root.join("non-k1-store");
    let mut non_k1 = base_command("scan");
    non_k1
        .arg("--repository")
        .arg(&repository.worktree)
        .args(["--repository-id", REPOSITORY_ID, "--revision"])
        .arg(&repository.commit_oid)
        .args(["--profile", "standard-local-s4"])
        .arg("--store")
        .arg(&non_k1_store)
        .args(["--format", "json"])
        .args(["--output-capacity-profile", OUTPUT_CAPACITY_PROFILE]);
    assert_unsupported(&non_k1.output().expect("run non-K1 profile"), &non_k1_store);

    for command_name in ["docs", "query", "export", "explore"] {
        let mut command = base_command(command_name);
        command.args(["--output-capacity-profile", OUTPUT_CAPACITY_PROFILE]);
        assert_error_v16(&command.output().expect("run non-scan command"));
    }

    let compiler_store = repository.root.join("compiler-store");
    let mut compiler = k1_scan_command(&repository, &compiler_store);
    compiler.args(["--compiler-index-profile", "rust-scip-import-v1"]);
    compiler.args(["--output-capacity-profile", OUTPUT_CAPACITY_PROFILE]);
    assert_unsupported(
        &compiler.output().expect("run compiler composition"),
        &compiler_store,
    );

    let boundary_store = repository.root.join("boundary-store");
    let mut boundary = k1_scan_command(&repository, &boundary_store);
    boundary.args(["--repository-boundary-profile", "local-gitlinks-v1"]);
    boundary.args(["--output-capacity-profile", OUTPUT_CAPACITY_PROFILE]);
    assert_unsupported(
        &boundary.output().expect("run boundary composition"),
        &boundary_store,
    );
}

fn k1_scan_command(repository: &MaterializedCallableRepository, store: &Path) -> Command {
    let mut command = base_command("scan");
    command
        .current_dir(&repository.root)
        .arg("--repository")
        .arg(&repository.worktree)
        .args(["--repository-id", REPOSITORY_ID, "--revision"])
        .arg(&repository.commit_oid)
        .args([
            "--profile",
            "standard-local-s4",
            "--workspace-profile",
            "cargo-root-package-v1",
            "--manifest-profile",
            "cargo-manifest-facts-v1",
            "--rust-semantic-profile",
            "rust-semantic-depth-v1",
            "--rust-framework-profile",
            "rust-framework-declarations-v1",
            "--rust-callable-profile",
            "rust-callable-semantics-v1",
        ])
        .arg("--store")
        .arg(store)
        .args(["--format", "json"]);
    command
}

fn base_command(command_name: &str) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_noesis"));
    command.arg(command_name);
    command
}

fn assert_success(output: &Output, subject: &str) {
    assert!(
        output.status.success(),
        "{subject} failed: status={:?}, stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
}

fn assert_unsupported(output: &Output, store: &Path) {
    assert_error_v16(output);
    assert!(
        fs::symlink_metadata(store).is_err(),
        "invalid composition mutated the store"
    );
}

fn assert_error_v16(output: &Output) {
    assert_eq!(output.status.code(), Some(11));
    assert!(output.stdout.is_empty());
    let error: Value = parse_single_document(&output.stderr);
    assert_eq!(error["schema_version"], "codenoesis.error/v16");
    assert_eq!(error["code"], "input.unsupported_rust_callable_composition");
}
