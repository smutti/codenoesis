mod support;

use serde_json::Value;

use support::s4_r6::MaterializedFrameworkDeclarationsRepository;

const PRE_R6_STDERR: &[u8] = b"{\"code\":\"input.invalid_revision\",\"context\":{},\"message\":\"invalid revision\",\"retryable\":false,\"schema_version\":\"codenoesis.error/v4\",\"stage\":\"input\"}\n";

#[test]
fn e2e_fr_ext_011_framework_declarations() {
    let repository = MaterializedFrameworkDeclarationsRepository::fixture();
    assert!(!repository.build_sentinel().exists());
    let output = repository.scan();

    if output.status.code() == Some(2) {
        assert!(output.stdout.is_empty(), "pre-R6 stdout changed");
        assert_eq!(output.stderr, PRE_R6_STDERR, "pre-R6 stderr changed");
        assert!(!repository.store.exists(), "pre-R6 store was created");
        assert!(
            !repository.documents.exists(),
            "pre-R6 documents root was created"
        );
        assert!(
            !repository.build_sentinel().exists(),
            "pre-R6 subject executed build.rs"
        );
        panic!(
            "expected RepositorySnapshotV9 success; observed approved unknown Rust framework selector Red"
        );
    }

    assert!(
        output.status.success(),
        "R6 scan failed: status={:?}, stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "successful R6 stderr changed");
    assert!(
        !repository.build_sentinel().exists(),
        "R6 subject executed build.rs"
    );
    let snapshot: Value =
        serde_json::from_slice(&output.stdout).expect("parse RepositorySnapshotV9");
    assert_eq!(
        snapshot["schema_version"],
        "codenoesis.repository-snapshot/v9"
    );
    assert_eq!(
        snapshot["semantic"]["configuration"]["rust_framework_profile"],
        "rust-framework-declarations-v1"
    );
    assert_eq!(
        snapshot["semantic"]["knowledge_graph"]["ontology_version"],
        "codenoesis.ontology/rust/v6"
    );
}
