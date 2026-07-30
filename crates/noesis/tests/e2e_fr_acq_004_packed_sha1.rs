mod support;

use std::fs;

use serde_json::{Value, json};

use support::s1::{COMMIT_A_OID, MaterializedRepository, fixture_root};
use support::s1_packed::{materialize_base_only_pack, scan_packed_command};

#[test]
fn e2e_fr_acq_004_packed_sha1_equivalence() {
    let repository = MaterializedRepository::revision_a();
    let packed = materialize_base_only_pack(&repository);
    let sentinel = repository.root.join("outside-sentinel");
    let sentinel_bytes = b"must-not-be-read-or-changed\n";
    fs::write(&sentinel, sentinel_bytes).expect("write packed acquisition sentinel");
    repository.apply_isolation_variant(&sentinel);

    let output = scan_packed_command(&repository.worktree, COMMIT_A_OID)
        .env("CODENOESIS_SENTINEL", &sentinel)
        .output()
        .expect("launch packed acquisition subject");

    packed.assert_unchanged();
    assert_eq!(
        fs::read(&sentinel).expect("read packed acquisition sentinel after subject"),
        sentinel_bytes,
        "packed acquisition executed a hook or changed the outside sentinel"
    );
    assert!(
        output.status.success(),
        "expected selected packed S1 scan success; status={:?}; stdout={}; stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let snapshot: Value =
        serde_json::from_slice(&output.stdout).expect("parse packed RepositorySnapshotV2");
    let mut expected_semantic =
        fs::read(fixture_root().join("expected-semantic-a.jcs")).expect("read S1 semantic golden");
    assert_eq!(expected_semantic.pop(), Some(b'\n'));
    assert_eq!(
        serde_json::to_vec(&snapshot["semantic"]).expect("serialize packed semantic value"),
        expected_semantic
    );
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
