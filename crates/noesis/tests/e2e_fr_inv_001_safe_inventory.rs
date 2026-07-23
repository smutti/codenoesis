mod support;

use std::fs;

use serde_json::{Value, json};

use support::parse_single_document;
use support::s1::{COMMIT_A_OID, MaterializedRepository, fixture_root, scan};

#[test]
fn e2e_fr_inv_001_safe_inventory() {
    let repository = MaterializedRepository::revision_a();
    let output = scan(&repository.worktree, COMMIT_A_OID);

    assert_eq!(
        output.status.code(),
        Some(0),
        "expected subject exit 0 with RepositorySnapshotV2; observed subject exit {:?}; stdout={:?}; stderr={:?}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "successful stderr must be empty");
    let snapshot = parse_single_document(&output.stdout);
    assert_eq!(
        serde_json::to_vec(&snapshot).expect("serialize parsed snapshot"),
        output.stdout[..output.stdout.len() - 1],
        "stdout must use canonical key order"
    );

    let expected_semantic: Value = serde_json::from_slice(
        &fs::read(fixture_root().join("expected-semantic-a.json"))
            .expect("read reviewed S1 semantic golden"),
    )
    .expect("parse reviewed S1 semantic golden");
    assert_eq!(snapshot["semantic"], expected_semantic);
    assert_eq!(
        snapshot["semantic_hash"],
        json!({
            "algorithm": "blake3-256",
            "value": "236b231c3154f9be56130ddc8dfb39bb482af10330f7c6757597ad22c006e9e7"
        })
    );
    assert_eq!(
        snapshot["schema_version"],
        "codenoesis.repository-snapshot/v2"
    );
}
