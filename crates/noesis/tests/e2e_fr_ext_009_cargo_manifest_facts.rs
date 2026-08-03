mod support;

use serde_json::Value;

use support::s4_r4::MaterializedCargoManifestRepository;

const PRE_R4_STDERR: &[u8] = b"{\"code\":\"input.invalid_revision\",\"context\":{},\"message\":\"invalid revision\",\"retryable\":false,\"schema_version\":\"codenoesis.error/v4\",\"stage\":\"input\"}\n";

#[test]
fn e2e_fr_ext_009_cargo_manifest_facts() {
    let repository = MaterializedCargoManifestRepository::fixture();
    let output = repository.scan();

    if output.status.code() == Some(2) {
        assert!(output.stdout.is_empty(), "pre-R4 stdout changed");
        assert_eq!(output.stderr, PRE_R4_STDERR, "pre-R4 stderr changed");
        assert!(!repository.store.exists(), "pre-R4 store was created");
        assert!(
            !repository.documents.exists(),
            "pre-R4 documents root was created"
        );
        panic!(
            "expected RepositorySnapshotV7 success; observed approved unknown manifest selector Red"
        );
    }

    assert!(
        output.status.success(),
        "R4 scan failed: status={:?}, stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "successful R4 stderr changed");
    let snapshot: Value =
        serde_json::from_slice(&output.stdout).expect("parse RepositorySnapshotV7");
    assert_eq!(
        snapshot["schema_version"],
        "codenoesis.repository-snapshot/v7"
    );
    assert_eq!(
        snapshot["semantic"]["configuration"]["manifest_profile"],
        "cargo-manifest-facts-v1"
    );
}
