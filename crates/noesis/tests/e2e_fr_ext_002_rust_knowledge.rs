mod support;

use support::s2::{COMMIT_A_OID, MaterializedRepository, fixture_root, scan};
use support::{parse_single_document, read_json};

#[test]
fn e2e_fr_ext_002_rust_knowledge() {
    let repository = MaterializedRepository::revision_a();
    let output = scan(&repository.worktree, COMMIT_A_OID);

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
        snapshot["semantic"]["extraction_chunks"][0],
        read_json(&fixture_root().join("expected-extraction-a.json"))
    );
    assert_eq!(
        snapshot["semantic"]["knowledge_graph"],
        read_json(&fixture_root().join("expected-graph-a.json"))
    );
}
