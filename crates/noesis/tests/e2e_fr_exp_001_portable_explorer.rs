mod support;

use std::fs;
use std::process::{Command, Output};

use serde_json::Value;

use support::s4_r7::{MaterializedCompilerIndexRepository, REPOSITORY_ID};

const PRE_R8_STDERR: &[u8] = b"{\"code\":\"input.invalid_revision\",\"context\":{},\"message\":\"invalid revision\",\"retryable\":false,\"schema_version\":\"codenoesis.error/v1\",\"stage\":\"input\"}\n";

#[test]
fn e2e_fr_exp_001_export_and_explore_offline() {
    let repository = MaterializedCompilerIndexRepository::fixture();
    let scan = repository.scan();
    assert_success(&scan, "R8 source V10 scan");

    let portable_root = repository.root.join("portable-graph");
    let explorer_root = repository.root.join("local-explorer");
    let export = export(&repository, &portable_root);

    if export.status.code() == Some(2) {
        assert!(export.stdout.is_empty(), "pre-R8 stdout changed");
        assert_eq!(export.stderr, PRE_R8_STDERR, "pre-R8 stderr changed");
        assert!(!portable_root.exists(), "pre-R8 portable root was created");
        assert!(!explorer_root.exists(), "pre-R8 explorer root was created");
        assert!(!repository.build_sentinel().exists());
        assert!(!repository.indexer_sentinel().exists());
        panic!("expected PortableGraphV1 success; observed approved unknown export command Red");
    }

    assert_success(&export, "R8 portable graph export");
    assert!(export.stderr.is_empty());
    let portable_path = portable_root.join("portable-graph.json");
    let portable_bytes = fs::read(&portable_path).expect("read PortableGraphV1 output");
    assert_eq!(export.stdout, portable_bytes, "export stdout/file drift");
    let portable: Value =
        serde_json::from_slice(&portable_bytes).expect("parse PortableGraphV1 output");
    assert_eq!(portable["schema_version"], "codenoesis.portable-graph/v1");
    assert_eq!(
        portable["source_snapshot"]["schema_version"],
        "codenoesis.repository-snapshot/v10"
    );
    assert_eq!(portable["repository"]["identity"], REPOSITORY_ID);
    assert!(
        portable_root
            .join(".codenoesis-portable-graph-v1")
            .is_file()
    );

    let explore = explore(&portable_path, &explorer_root);
    assert_success(&explore, "R8 local explorer generation");
    assert!(explore.stderr.is_empty());
    let manifest: Value =
        serde_json::from_slice(&explore.stdout).expect("parse LocalExplorerV1 manifest");
    assert_eq!(manifest["schema_version"], "codenoesis.local-explorer/v1");
    assert_eq!(
        fs::read(explorer_root.join("explorer-manifest.json"))
            .expect("read LocalExplorerV1 manifest file"),
        explore
            .stdout
            .strip_suffix(b"\n")
            .unwrap_or(&explore.stdout)
    );
    assert_eq!(
        fs::read(explorer_root.join("portable-graph.json")).expect("read explorer portable graph"),
        portable_bytes
    );
    assert!(explorer_root.join("index.html").is_file());
    assert!(
        explorer_root
            .join(".codenoesis-local-explorer-v1")
            .is_file()
    );
    assert!(!repository.build_sentinel().exists());
    assert!(!repository.indexer_sentinel().exists());
}

fn export(repository: &MaterializedCompilerIndexRepository, output: &std::path::Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_noesis"))
        .args(["export", "--store"])
        .arg(&repository.store)
        .args(["--repository-id", REPOSITORY_ID, "--output"])
        .arg(output)
        .args(["--format", "json"])
        .output()
        .expect("launch R8 export subject")
}

fn explore(input: &std::path::Path, output: &std::path::Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_noesis"))
        .args(["explore", "--input"])
        .arg(input)
        .arg("--output")
        .arg(output)
        .args(["--format", "json"])
        .output()
        .expect("launch R8 explore subject")
}

fn assert_success(output: &Output, subject: &str) {
    assert!(
        output.status.success(),
        "{subject} failed: status={:?}, stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}
