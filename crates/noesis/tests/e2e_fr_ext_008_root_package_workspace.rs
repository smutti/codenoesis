mod support;

use serde_json::Value;

use support::s4_r3::MaterializedRootPackageRepository;

const EXPECTED_RED_STDERR: &[u8] = b"{\"code\":\"input.invalid_revision\",\"context\":{},\"message\":\"invalid revision\",\"retryable\":false,\"schema_version\":\"codenoesis.error/v4\",\"stage\":\"input\"}\n";

#[test]
fn e2e_fr_ext_008_root_package_workspace() {
    let repository = MaterializedRootPackageRepository::implicit();
    let output = repository.scan();

    if !output.status.success() {
        assert_eq!(output.status.code(), Some(2), "unexpected R3 Red exit");
        assert!(output.stdout.is_empty(), "R3 Red stdout must be empty");
        assert_eq!(
            output.stderr, EXPECTED_RED_STDERR,
            "unexpected R3 Red stderr"
        );
        assert!(
            !repository.store.exists(),
            "R3 Red must not create the store"
        );
        assert!(
            !repository.documents.exists(),
            "R3 Red must not create the documents root"
        );
        panic!(
            "expected R3 RepositorySnapshotV6 success; observed approved pre-R3 selector rejection"
        );
    }

    assert!(output.stderr.is_empty(), "successful R3 stderr changed");
    let snapshot: Value =
        serde_json::from_slice(&output.stdout).expect("parse RepositorySnapshotV6");
    assert_eq!(
        snapshot["schema_version"],
        "codenoesis.repository-snapshot/v6"
    );
    assert_eq!(
        snapshot["semantic"]["configuration"]["workspace_profile"],
        "cargo-root-package-v1"
    );
    assert_eq!(
        snapshot["semantic"]["knowledge_graph"]["root_shape"],
        "non_virtual_workspace"
    );
    assert_eq!(
        snapshot["semantic"]["repository_boundaries"]["summary"]["boundary_count"],
        1
    );
}
