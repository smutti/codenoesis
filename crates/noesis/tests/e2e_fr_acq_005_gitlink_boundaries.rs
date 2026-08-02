mod support;

use serde_json::Value;

use support::s1_boundaries::{MaterializedBoundaryRepository, expected_unbound_boundaries};

const EXPECTED_RED_STDERR: &[u8] = b"{\"code\":\"input.invalid_revision\",\"context\":{},\"message\":\"invalid revision\",\"retryable\":false,\"schema_version\":\"codenoesis.error/v4\",\"stage\":\"input\"}\n";

#[test]
fn e2e_fr_acq_005_gitlink_boundaries() {
    let repository = MaterializedBoundaryRepository::unbound();
    let output = repository.scan_unbound();

    if !output.status.success() {
        assert_eq!(output.status.code(), Some(2), "unexpected Red exit");
        assert!(output.stdout.is_empty(), "Red stdout must be empty");
        assert_eq!(output.stderr, EXPECTED_RED_STDERR, "unexpected Red stderr");
        assert!(
            !repository.base.store.exists(),
            "Red must not create the store"
        );
        panic!(
            "expected R2 RepositorySnapshotV5 success; observed approved pre-R2 selector rejection"
        );
    }

    assert!(output.stderr.is_empty(), "successful stderr must be empty");
    let snapshot: Value =
        serde_json::from_slice(&output.stdout).expect("parse RepositorySnapshotV5");
    assert_eq!(
        snapshot["schema_version"],
        "codenoesis.repository-snapshot/v5"
    );
    assert_eq!(
        snapshot["semantic"]["repository_boundaries"],
        expected_unbound_boundaries()
    );
}
