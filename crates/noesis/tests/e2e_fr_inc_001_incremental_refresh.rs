mod support;

use std::process::Output;

use serde_json::Value;
use support::parse_single_document;
use support::read_json;
use support::s5::{
    BASELINE_COMMIT_OID, MaterializedRepository, REPOSITORY_ID, TARGET_COMMIT_OID,
    canonical_semantic_from_scan, expected_report_bytes, fixture_root, owned_document_bytes,
};

#[test]
fn e2e_fr_inc_001_incremental_refresh() {
    let repository = MaterializedRepository::two_revisions();

    let baseline = repository.baseline_scan();
    assert_success(&baseline, "S5 baseline S4 scan");
    assert_revision_summary(&baseline, "revision-a");
    let baseline_head = repository.stored_head(&repository.store);
    assert_eq!(baseline_head.commit_oid, BASELINE_COMMIT_OID);
    assert_eq!(baseline_head.generation, 1);

    let refresh = repository.refresh();
    assert_refresh_success_or_expected_red(&repository, &baseline_head, &refresh);
    assert_eq!(
        refresh.stdout,
        expected_report_bytes(),
        "S5 canonical refresh report changed"
    );
    let target_head = repository.stored_head(&repository.store);
    assert_eq!(target_head.commit_oid, TARGET_COMMIT_OID);
    assert_eq!(target_head.generation, 2);
    assert_ne!(target_head.snapshot_id, baseline_head.snapshot_id);

    let cold_target = repository.cold_target_scan();
    assert_success(&cold_target, "S5 cold target S4 scan");
    assert_revision_summary(&cold_target, "revision-b");
    assert_eq!(
        repository.stored_snapshot_semantic(&repository.store),
        canonical_semantic_from_scan(&cold_target),
        "incremental target semantic bytes differ from cold S4"
    );

    let incremental_docs = repository.docs(&repository.store, &repository.documents);
    assert_success(&incremental_docs, "S5 incremental target docs");
    let cold_docs = repository.docs(&repository.cold_store, &repository.cold_documents);
    assert_success(&cold_docs, "S5 cold target docs");
    assert_eq!(
        owned_document_bytes(&repository.documents),
        owned_document_bytes(&repository.cold_documents),
        "incremental target documentation differs from cold S4"
    );

    let replay = repository.refresh();
    assert_success(&replay, "S5 no-change replay");
    let replay_report = parse_single_document(&replay.stdout);
    assert_eq!(
        replay_report["schema_version"],
        "codenoesis.incremental-refresh-report/v1"
    );
    assert_eq!(replay_report["rule"]["outcome"], "no_change");
    assert_eq!(replay_report["changed_paths"], Value::Array(Vec::new()));
    assert_eq!(
        repository.stored_head(&repository.store),
        target_head,
        "S5 no-change replay advanced the visible head"
    );
    assert!(
        !repository.process_sentinel.exists(),
        "S5 runtime executed a forbidden process or hook"
    );
}

fn assert_refresh_success_or_expected_red(
    repository: &MaterializedRepository,
    baseline_head: &support::s5::StoredHead,
    output: &Output,
) {
    if output.status.success() {
        assert!(
            output.stderr.is_empty(),
            "successful S5 refresh stderr changed"
        );
        return;
    }

    assert_eq!(
        output.status.code(),
        Some(2),
        "pre-S5 refresh must reject only the unrecognized command boundary"
    );
    assert!(
        output.stdout.is_empty(),
        "failed S5 refresh stdout must be empty"
    );
    assert_eq!(
        repository.stored_head(&repository.store),
        *baseline_head,
        "expected S5 Red advanced or replaced the baseline head"
    );
    let error = parse_single_document(&output.stderr);
    assert_eq!(error["schema_version"], "codenoesis.error/v2");
    assert_eq!(error["code"], "input.invalid_revision");
    assert!(
        !repository.process_sentinel.exists(),
        "expected S5 Red executed a forbidden process or hook"
    );
    panic!("expected S5 refresh success; observed the approved unrecognized-command Red");
}

fn assert_revision_summary(output: &Output, revision_name: &str) {
    let snapshot = parse_single_document(&output.stdout);
    let summary = read_json(&fixture_root().join("expected-cold-artifacts.json"));
    let revision = summary["revisions"]
        .as_array()
        .expect("S5 cold revisions")
        .iter()
        .find(|revision| revision["name"] == revision_name)
        .expect("S5 reviewed cold revision");
    assert_eq!(
        snapshot["semantic"]["repository"]["identity"],
        REPOSITORY_ID
    );
    assert_eq!(
        snapshot["semantic"]["repository"]["commit_oid"],
        revision["commit_oid"]
    );
    assert_eq!(
        snapshot["semantic_hash"],
        revision["snapshot_semantic_hash"]
    );
}

fn assert_success(output: &Output, journey: &str) {
    assert_eq!(
        output.status.code(),
        Some(0),
        "{journey} failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "{journey} stderr changed");
}
