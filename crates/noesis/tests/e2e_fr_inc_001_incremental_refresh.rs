mod support;

use std::process::Output;

use codenoesis_application::{PublicationService, ScanError, ScanRequest, ScanService};
use codenoesis_contracts::{CodeNoesisErrorV7, SnapshotEnvelopeV1};
use codenoesis_domain::storage::{SnapshotId, StorageError};
use codenoesis_domain::{RepositoryIdentity, Revision};
use codenoesis_lang_rust::TreeSitterRustWorkspaceExtractor;
use codenoesis_ports::NoopPublicationObserver;
use codenoesis_repository::LocalGitRepository;
use codenoesis_store_local::LocalStore;
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
    assert_eq!(replay_report["publication"]["head_advanced"], false);
    assert_eq!(
        replay_report["public_rematerialization"]["chunks"],
        Value::Array(Vec::new())
    );
    assert_eq!(
        replay_report["public_rematerialization"]["documents"],
        Value::Array(Vec::new())
    );
    assert_eq!(replay_report["metrics"]["parser_invocation_count"], 0);
    assert_eq!(replay_report["metrics"]["rematerialized_chunk_count"], 0);
    assert_eq!(
        replay_report["metrics"]["rematerialized_document_manifest_count"],
        0
    );
    assert_eq!(
        repository.stored_head(&repository.store),
        target_head,
        "S5 no-change replay advanced the visible head"
    );
    assert!(
        !repository.process_sentinel.exists(),
        "S5 runtime executed a forbidden process or hook"
    );
    let repeated_replay = repository.refresh();
    assert_success(&repeated_replay, "S5 repeated no-change replay");
    assert_eq!(
        repeated_replay.stdout, replay.stdout,
        "S5 no-change report is not deterministic"
    );
}

#[test]
fn e2e_fr_cli_004_missing_baseline_is_error_v7() {
    let repository = MaterializedRepository::two_revisions();

    let refresh = repository.refresh();

    assert_eq!(refresh.status.code(), Some(15));
    assert!(refresh.stdout.is_empty());
    let error = parse_single_document(&refresh.stderr);
    assert_eq!(error["schema_version"], "codenoesis.error/v7");
    assert_eq!(error["code"], "incremental.baseline_missing");
    assert_eq!(error["stage"], "incremental");
    assert_eq!(error["retryable"], false);
    assert_eq!(error["context"]["component"], "baseline_head");
    assert_eq!(
        error["context"]["expected_repository_identity"],
        REPOSITORY_ID
    );
    assert!(!repository.store.exists());
    assert!(!repository.process_sentinel.exists());
}

#[test]
fn e2e_fr_cli_004_recognized_refresh_uses_error_v7_for_input() {
    let repository = MaterializedRepository::two_revisions();

    let refresh = repository.refresh_with_profile("standard-local-s4");

    assert_eq!(refresh.status.code(), Some(2));
    assert!(refresh.stdout.is_empty());
    let error = parse_single_document(&refresh.stderr);
    assert_eq!(error["schema_version"], "codenoesis.error/v7");
    assert_eq!(error["code"], "input.invalid_profile");
    assert_eq!(error["stage"], "input");
    assert_eq!(error["retryable"], false);
    assert_eq!(error["context"], serde_json::json!({}));
    assert!(!repository.store.exists());
    assert!(!repository.process_sentinel.exists());
}

#[test]
fn e2e_fr_inc_003_corrupt_cache_preserves_baseline_head() {
    let repository = MaterializedRepository::two_revisions();
    let baseline = repository.baseline_scan();
    assert_success(&baseline, "S5 corrupt-cache baseline scan");
    let baseline_head = repository.stored_head(&repository.store);
    repository.corrupt_first_analysis_cache_entry();

    let refresh = repository.refresh();

    assert_eq!(refresh.status.code(), Some(15));
    assert!(refresh.stdout.is_empty());
    let error = parse_single_document(&refresh.stderr);
    assert_eq!(error["schema_version"], "codenoesis.error/v7");
    assert_eq!(error["code"], "incremental.cache_corrupt");
    assert_eq!(error["stage"], "incremental");
    assert_eq!(error["retryable"], false);
    assert_eq!(error["context"]["component"], "analysis_cache");
    assert_eq!(
        repository.stored_head(&repository.store),
        baseline_head,
        "corrupt S5 cache advanced the visible head"
    );
    assert!(!repository.process_sentinel.exists());
}

#[test]
fn e2e_fr_inc_003_valid_but_false_cache_fails_equivalence() {
    let repository = MaterializedRepository::two_revisions();
    let baseline = repository.baseline_scan();
    assert_success(&baseline, "S5 poisoned-cache baseline scan");
    let baseline_head = repository.stored_head(&repository.store);
    repository.poison_first_analysis_cache_entry();

    let refresh = repository.refresh();

    assert_eq!(refresh.status.code(), Some(15));
    assert!(refresh.stdout.is_empty());
    let error = parse_single_document(&refresh.stderr);
    assert_eq!(error["schema_version"], "codenoesis.error/v7");
    assert_eq!(error["code"], "incremental.cold_equivalence_failed");
    assert_eq!(error["context"]["component"], "snapshot");
    assert_ne!(
        error["context"]["expected_hash"],
        error["context"]["observed_hash"]
    );
    assert_eq!(
        repository.stored_head(&repository.store),
        baseline_head,
        "false S5 cache analysis advanced the visible head"
    );
    assert!(!repository.process_sentinel.exists());
}

#[test]
fn e2e_fr_inc_003_missing_cache_recomputes_without_cold_substitution() {
    let repository = MaterializedRepository::two_revisions();
    let baseline = repository.baseline_scan();
    assert_success(&baseline, "S5 missing-cache baseline scan");
    repository.remove_analysis_cache();

    let refresh = repository.refresh();

    assert_success(&refresh, "S5 missing-cache refresh");
    let report = parse_single_document(&refresh.stdout);
    assert_eq!(report["rule"]["outcome"], "partial_analysis");
    assert_eq!(report["metrics"]["parser_invocation_count"], 4);
    assert_eq!(report["metrics"]["cache_hit_count"], 2);
    assert_eq!(report["metrics"]["cache_miss_count"], 1);
    assert_eq!(
        repository.stored_head(&repository.store).commit_oid,
        TARGET_COMMIT_OID
    );
    assert!(!repository.process_sentinel.exists());
}

#[test]
fn e2e_fr_inc_002_incompatible_cache_selects_full_rebuild() {
    let repository = MaterializedRepository::two_revisions();
    let baseline = repository.baseline_scan();
    assert_success(&baseline, "S5 version-rebuild baseline scan");
    repository.add_incompatible_analysis_cache_entry();

    let refresh = repository.refresh();

    assert_success(&refresh, "S5 incompatible-cache full rebuild");
    let report = parse_single_document(&refresh.stdout);
    assert_eq!(report["rule"]["outcome"], "full_rebuild");
    assert_eq!(
        report["rule"]["rule_ids"],
        serde_json::json!(["INC-RULE-002"])
    );
    assert_eq!(
        report["rule"]["reasons"],
        serde_json::json!(["version_boundary_changed"])
    );
    assert_eq!(report["metrics"]["cache_hit_count"], 0);
    assert_eq!(report["metrics"]["parser_invocation_count"], 3);
    assert_eq!(
        repository.stored_head(&repository.store).commit_oid,
        TARGET_COMMIT_OID
    );
    assert!(!repository.process_sentinel.exists());
}

#[test]
fn e2e_fr_cli_004_non_s4_baseline_is_incompatible() {
    let repository = MaterializedRepository::two_revisions();
    let baseline = repository.baseline_s3_scan();
    assert_success(&baseline, "S5 incompatible S3 baseline scan");
    let baseline_head = repository.stored_head(&repository.store);

    let refresh = repository.refresh();

    assert_eq!(refresh.status.code(), Some(15));
    assert!(refresh.stdout.is_empty());
    let error = parse_single_document(&refresh.stderr);
    assert_eq!(error["schema_version"], "codenoesis.error/v7");
    assert_eq!(error["code"], "incremental.baseline_incompatible");
    assert_eq!(error["context"]["component"], "baseline_snapshot");
    assert_eq!(
        repository.stored_head(&repository.store),
        baseline_head,
        "incompatible baseline advanced the visible head"
    );
    assert!(!repository.process_sentinel.exists());
}

#[test]
fn ft_fr_cli_004_concurrent_head_movement_is_retryable() {
    let repository = MaterializedRepository::two_revisions();
    let baseline = repository.baseline_scan();
    assert_success(&baseline, "S5 concurrent baseline scan");
    let baseline_head = repository.stored_head(&repository.store);
    let request = ScanRequest::new(
        repository.worktree.as_os_str().to_os_string(),
        RepositoryIdentity::parse(REPOSITORY_ID).expect("S5 concurrent repository identity"),
        Revision::parse(TARGET_COMMIT_OID).expect("S5 concurrent target revision"),
        SnapshotEnvelopeV1::new(
            "2026-07-29T00:00:00Z".to_owned(),
            None,
            "s5-concurrent-target".to_owned(),
        ),
    );
    let target = ScanService::new(LocalGitRepository::new())
        .scan_s4(request, &TreeSitterRustWorkspaceExtractor::new())
        .expect("build S5 concurrent target");
    let expected_baseline =
        SnapshotId::parse(&baseline_head.snapshot_id).expect("S5 baseline snapshot ID");
    let mut store = LocalStore::open_existing(&repository.store).expect("open S5 concurrent store");
    PublicationService::publish_v4(
        &target,
        &mut store.artifacts,
        &mut store.metadata,
        &mut NoopPublicationObserver,
    )
    .expect("publish concurrent S5 movement");
    let moved_head = repository.stored_head(&repository.store);

    let storage_error = match PublicationService::publish_v4_expected(
        &target,
        &expected_baseline,
        &mut store.artifacts,
        &mut store.metadata,
        &mut NoopPublicationObserver,
    ) {
        Err(ScanError::Storage(error @ StorageError::HeadConflict { .. })) => error,
        Err(ScanError::Storage(_)) => {
            panic!("S5 concurrent movement returned the wrong storage failure")
        }
        Err(ScanError::Acquisition(_)) => {
            panic!("S5 concurrent movement returned an acquisition failure")
        }
        Err(ScanError::Knowledge(_)) => {
            panic!("S5 concurrent movement returned a knowledge failure")
        }
        Err(ScanError::Workspace(_)) => {
            panic!("S5 concurrent movement returned a workspace failure")
        }
        Err(ScanError::Internal) => {
            panic!("S5 concurrent movement returned an internal failure")
        }
        Ok(_) => panic!("S5 concurrent movement replaced the publication precondition"),
    };
    let error = parse_single_document(
        &CodeNoesisErrorV7::from_storage(&storage_error)
            .canonical_stderr()
            .expect("serialize S5 concurrent ErrorV7"),
    );
    assert_eq!(error["code"], "publication.head_conflict");
    assert_eq!(error["stage"], "publication");
    assert_eq!(error["retryable"], true);
    assert_eq!(
        error["context"]["expected_snapshot_id"],
        baseline_head.snapshot_id
    );
    assert_eq!(
        error["context"]["actual_snapshot_id"],
        moved_head.snapshot_id
    );
    assert_eq!(
        repository.stored_head(&repository.store),
        moved_head,
        "S5 head conflict changed the visible target"
    );
    assert!(!repository.process_sentinel.exists());
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
