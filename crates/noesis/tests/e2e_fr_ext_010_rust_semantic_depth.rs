mod support;

use serde_json::Value;

use support::s4_r5::MaterializedRustSemanticRepository;

const PRE_R5_STDERR: &[u8] = b"{\"code\":\"input.invalid_revision\",\"context\":{},\"message\":\"invalid revision\",\"retryable\":false,\"schema_version\":\"codenoesis.error/v4\",\"stage\":\"input\"}\n";

#[test]
fn e2e_fr_ext_010_rust_semantic_depth() {
    let repository = MaterializedRustSemanticRepository::fixture();
    assert!(!repository.build_sentinel().exists());
    let output = repository.scan();

    if output.status.code() == Some(2) {
        assert!(output.stdout.is_empty(), "pre-R5 stdout changed");
        assert_eq!(output.stderr, PRE_R5_STDERR, "pre-R5 stderr changed");
        assert!(!repository.store.exists(), "pre-R5 store was created");
        assert!(
            !repository.documents.exists(),
            "pre-R5 documents root was created"
        );
        assert!(
            !repository.build_sentinel().exists(),
            "pre-R5 subject executed build.rs"
        );
        panic!(
            "expected RepositorySnapshotV8 success; observed approved unknown Rust semantic selector Red"
        );
    }

    assert!(
        output.status.success(),
        "R5 scan failed: status={:?}, stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "successful R5 stderr changed");
    assert!(
        !repository.build_sentinel().exists(),
        "R5 subject executed build.rs"
    );
    let snapshot: Value =
        serde_json::from_slice(&output.stdout).expect("parse RepositorySnapshotV8");
    assert_eq!(
        snapshot["schema_version"],
        "codenoesis.repository-snapshot/v8"
    );
    assert_eq!(
        snapshot["semantic"]["configuration"]["rust_semantic_profile"],
        "rust-semantic-depth-v1"
    );
}
