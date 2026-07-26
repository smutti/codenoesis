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

#[test]
fn gt_fr_ext_002_malformed_syntax_is_explicit() {
    let repository = MaterializedRepository::revision_a();
    let revision = repository.malformed_commit();
    let output = scan(&repository.worktree, &revision);

    assert_eq!(
        output.status.code(),
        Some(0),
        "subject stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "successful stderr must be empty");
    let snapshot = parse_single_document(&output.stdout);
    let graph = &snapshot["semantic"]["knowledge_graph"];
    assert!(graph["diagnostics"].as_array().is_some_and(|diagnostics| {
        diagnostics.iter().any(|diagnostic| {
            diagnostic["code"] == "extraction.malformed_syntax"
                && diagnostic["span"]
                    == serde_json::json!({"unit": "byte", "start": 556, "end": 558})
        })
    }));
    assert!(graph["coverage"]["gaps"].as_array().is_some_and(|gaps| {
        gaps.iter().any(|gap| {
            gap["code"] == "malformed_syntax_excluded"
                && gap["span"] == serde_json::json!({"unit": "byte", "start": 556, "end": 558})
        })
    }));
}

#[test]
fn gt_dr_idn_001_unicode_normalization_collision() {
    let repository = MaterializedRepository::revision_a();
    let revision = repository.nfc_collision_commit();
    let output = scan(&repository.worktree, &revision);

    assert_eq!(output.status.code(), Some(11));
    assert!(output.stdout.is_empty(), "failed stdout must be empty");
    assert_eq!(
        parse_single_document(&output.stderr),
        read_json(&fixture_root().join("expected-error-nfc-collision.json"))
    );
}

#[test]
fn sec_fr_ext_002_target_never_executes() {
    let repository = MaterializedRepository::revision_a();
    repository.write_dirty_worktree_decoy();
    let output = scan(&repository.worktree, COMMIT_A_OID);

    assert_eq!(
        output.status.code(),
        Some(0),
        "subject stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let snapshot = parse_single_document(&output.stdout);
    assert_eq!(
        snapshot["semantic"]["knowledge_graph"],
        read_json(&fixture_root().join("expected-graph-a.json"))
    );
}

#[test]
fn conf_fr_ext_001_snapshot_v3_and_error_v3() {
    let repository = MaterializedRepository::revision_a();
    let success = scan(&repository.worktree, COMMIT_A_OID);
    let snapshot = parse_single_document(&success.stdout);
    let top_level = snapshot
        .as_object()
        .expect("RepositorySnapshotV3 must be an object");
    assert_eq!(
        top_level.keys().map(String::as_str).collect::<Vec<_>>(),
        ["envelope", "schema_version", "semantic", "semantic_hash"]
    );
    let semantic = snapshot["semantic"]
        .as_object()
        .expect("RepositorySnapshotV3 semantic must be an object");
    assert_eq!(
        semantic.keys().map(String::as_str).collect::<Vec<_>>(),
        [
            "configuration",
            "evidence_lineage_version",
            "extraction_chunks",
            "extractor_contract_version",
            "extractor_versions",
            "inventory",
            "knowledge_graph",
            "ontology_version",
            "pipeline_version",
            "repository",
        ]
    );

    let collision = repository.nfc_collision_commit();
    let failure = scan(&repository.worktree, &collision);
    let error = parse_single_document(&failure.stderr);
    assert_eq!(
        error
            .as_object()
            .expect("CodeNoesisErrorV3 must be an object")
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        [
            "code",
            "context",
            "message",
            "retryable",
            "schema_version",
            "stage",
        ]
    );
}
