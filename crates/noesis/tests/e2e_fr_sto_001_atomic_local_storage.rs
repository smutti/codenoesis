mod support;

use std::fs;

use support::s3::{COMMIT_A_OID, MaterializedRepository, fixture_root, scan};
use support::{parse_single_document, read_json};

#[test]
fn e2e_fr_sto_001_atomic_local_storage() {
    let repository = MaterializedRepository::revisions();
    let output = scan(&repository.worktree, &repository.store, COMMIT_A_OID);

    assert_eq!(
        output.status.code(),
        Some(0),
        "subject stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "successful stderr must be empty");

    let snapshot = parse_single_document(&output.stdout);
    assert_eq!(
        snapshot["schema_version"],
        "codenoesis.repository-snapshot/v3"
    );
    assert_eq!(
        snapshot["semantic"],
        read_json(&fixture_root().join("snapshot-semantic-a.json"))
    );
    assert_eq!(
        fs::read(repository.store.join("store.json")).expect("read durable store marker"),
        b"{\"database\":\"metadata.sqlite3\",\"objects\":\"objects\",\"schema_version\":\"codenoesis.local-store-marker/v1\",\"temporary\":\"tmp\"}\n"
    );
    assert!(
        repository.store.join("metadata.sqlite3").is_file(),
        "successful S3 publication must create the metadata database"
    );
}
