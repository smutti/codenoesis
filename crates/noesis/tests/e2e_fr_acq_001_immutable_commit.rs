mod support;

use std::fs;
use std::process::Command;

use codenoesis_contracts::{RepositorySnapshotV1, SnapshotEnvelopeV1};
use codenoesis_domain::{RepositoryIdentity, Revision};
use codenoesis_ports::RepositoryAcquirer;
use codenoesis_repository::LocalGitRepository;
use serde_json::{Value, json};

use support::{
    BLOB_A_OID, BLOB_B_OID, COMMIT_A_OID, COMMIT_B_OID, MaterializedRepository, REPOSITORY_ID,
    TREE_A_OID, TREE_B_OID, assert_acquisition_error, fixture_root, parse_single_document, scan,
    unique_temp_root,
};

#[test]
fn e2e_fr_acq_001_immutable_commit() {
    let repository = MaterializedRepository::commit_a();
    let output = scan(&repository.worktree, COMMIT_A_OID);

    assert_eq!(
        output.status.code(),
        Some(0),
        "expected subject exit 0 with RepositorySnapshotV1; observed subject exit {:?}; stdout={:?}; stderr={:?}",
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
            .expect("read reviewed semantic golden"),
    )
    .expect("parse reviewed semantic golden");
    assert_eq!(snapshot["semantic"], expected_semantic);
    assert_eq!(
        snapshot["semantic_hash"],
        json!({
            "algorithm": "blake3-256",
            "value": "b673624a329f43fd84852bbdeefd66326a7fcb1c03fdb626e2de6bfedff11997"
        })
    );
    assert_eq!(
        snapshot["schema_version"],
        "codenoesis.repository-snapshot/v1"
    );
    assert!(
        !snapshot["semantic"].to_string().contains(
            repository
                .worktree
                .to_str()
                .expect("fixture path must be UTF-8")
        )
    );
}

#[test]
fn it_fr_acq_001_ref_move_after_binding_keeps_original_commit() {
    let repository = MaterializedRepository::commit_a();
    let adapter = LocalGitRepository::new();
    let identity = RepositoryIdentity::parse(REPOSITORY_ID).expect("approved repository identity");
    let bound_a = adapter
        .bind(
            repository.worktree.as_os_str(),
            identity.clone(),
            Revision::Main,
        )
        .expect("bind main while it names commit A");

    repository.update_main(COMMIT_B_OID);
    let snapshot_a = RepositorySnapshotV1::from_bound_revision(
        &bound_a,
        SnapshotEnvelopeV1::new(
            "2000-01-01T00:00:00Z".to_owned(),
            None,
            "s0-golden-a".to_owned(),
        ),
    );
    assert_eq!(
        snapshot_a.value()["semantic"]["repository"]["commit_oid"],
        COMMIT_A_OID
    );
    assert_eq!(
        snapshot_a.value()["semantic"]["repository"]["tree_oid"],
        TREE_A_OID
    );

    let bound_b = adapter
        .bind(repository.worktree.as_os_str(), identity, Revision::Main)
        .expect("a new binding resolves commit B");
    assert_eq!(bound_b.commit_oid().as_str(), COMMIT_B_OID);
    assert_eq!(bound_b.tree_oid().as_str(), TREE_B_OID);
}

#[test]
fn e2e_fr_acq_001_non_git_returns_typed_error() {
    let root = unique_temp_root();
    let plain_directory = root.join("plain-directory");
    fs::create_dir(&plain_directory).expect("create plain source directory");
    fs::copy(
        fixture_root().join("commit-a/main.rs"),
        plain_directory.join("main.rs"),
    )
    .expect("copy source into plain directory");

    let output = scan(&plain_directory, COMMIT_A_OID);
    let expected: Value = serde_json::from_slice(
        &fs::read(fixture_root().join("expected-error-not-git.json"))
            .expect("read reviewed non-Git error"),
    )
    .expect("parse reviewed non-Git error");
    assert_acquisition_error(&output, &expected);
    assert!(
        !String::from_utf8_lossy(&output.stderr).contains(
            plain_directory
                .to_str()
                .expect("fixture path must be UTF-8")
        )
    );
    fs::remove_dir_all(root).expect("remove plain source fixture");
}

#[test]
fn e2e_fr_acq_001_missing_object_returns_typed_error() {
    let repository = MaterializedRepository::commit_a();
    fs::remove_file(repository.object_path(BLOB_A_OID)).expect("remove referenced blob object");

    let output = scan(&repository.worktree, COMMIT_A_OID);
    assert_acquisition_error(
        &output,
        &json!({
            "schema_version": "codenoesis.error/v1",
            "code": "acquisition.object_missing",
            "stage": "acquisition",
            "message": "referenced Git object is missing",
            "retryable": false,
            "context": {
                "object_oid": BLOB_A_OID,
                "expected_kind": "blob",
                "referenced_by": TREE_A_OID
            }
        }),
    );
}

#[test]
fn e2e_fr_acq_001_inconsistent_object_returns_typed_error() {
    let repository = MaterializedRepository::commit_a();
    fs::remove_file(repository.object_path(BLOB_A_OID))
        .expect("remove original blob A before corruption");
    fs::copy(
        repository.object_path(BLOB_B_OID),
        repository.object_path(BLOB_A_OID),
    )
    .expect("replace blob A with validly framed blob B bytes");

    let output = scan(&repository.worktree, COMMIT_A_OID);
    assert_acquisition_error(
        &output,
        &json!({
            "schema_version": "codenoesis.error/v1",
            "code": "acquisition.repository_inconsistent",
            "stage": "acquisition",
            "message": "Git repository is inconsistent",
            "retryable": false,
            "context": {
                "object_oid": BLOB_A_OID,
                "expected_kind": "blob"
            }
        }),
    );
}

#[test]
fn e2e_fr_cli_003_invalid_identity_returns_strict_error() {
    let repository = MaterializedRepository::commit_a();
    let output = Command::new(env!("CARGO_BIN_EXE_noesis"))
        .args(["scan", "--repository"])
        .arg(&repository.worktree)
        .args([
            "--repository-id",
            "https://example.invalid/repository",
            "--revision",
            COMMIT_A_OID,
            "--format",
            "json",
        ])
        .output()
        .expect("scan with invalid repository identity");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        parse_single_document(&output.stderr),
        json!({
            "schema_version": "codenoesis.error/v1",
            "code": "input.invalid_repository_identity",
            "stage": "input",
            "message": "invalid repository identity",
            "retryable": false,
            "context": {}
        })
    );
}

#[test]
fn e2e_fr_cli_003_invalid_revision_returns_strict_error() {
    let repository = MaterializedRepository::commit_a();
    let output = scan(&repository.worktree, "HEAD");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        parse_single_document(&output.stderr),
        json!({
            "schema_version": "codenoesis.error/v1",
            "code": "input.invalid_revision",
            "stage": "input",
            "message": "invalid revision",
            "retryable": false,
            "context": {}
        })
    );
}

#[test]
fn e2e_fr_acq_001_revision_not_found_returns_typed_error() {
    let repository = MaterializedRepository::commit_a();
    let missing = "0000000000000000000000000000000000000000";
    let output = scan(&repository.worktree, missing);
    assert_acquisition_error(
        &output,
        &json!({
            "schema_version": "codenoesis.error/v1",
            "code": "acquisition.revision_not_found",
            "stage": "acquisition",
            "message": "revision not found",
            "retryable": false,
            "context": {"revision": missing}
        }),
    );
}

#[test]
fn e2e_fr_acq_001_revision_not_commit_returns_typed_error() {
    let repository = MaterializedRepository::commit_a();
    let output = scan(&repository.worktree, TREE_A_OID);
    assert_acquisition_error(
        &output,
        &json!({
            "schema_version": "codenoesis.error/v1",
            "code": "acquisition.revision_not_commit",
            "stage": "acquisition",
            "message": "revision does not name a commit",
            "retryable": false,
            "context": {
                "object_oid": TREE_A_OID,
                "actual_kind": "tree"
            }
        }),
    );
}

#[test]
fn e2e_fr_acq_001_unexpected_failure_is_strict_internal() {
    let repository = MaterializedRepository::commit_a();
    fs::write(repository.worktree.join(".git/config"), [0xff])
        .expect("install invalid repository configuration bytes");
    let output = scan(&repository.worktree, COMMIT_A_OID);
    assert_eq!(output.status.code(), Some(70));
    assert!(output.stdout.is_empty());
    assert_eq!(
        parse_single_document(&output.stderr),
        json!({
            "schema_version": "codenoesis.error/v1",
            "code": "internal.unexpected",
            "stage": "internal",
            "message": "unexpected internal failure",
            "retryable": false,
            "context": {}
        })
    );
    assert!(
        !String::from_utf8_lossy(&output.stderr).contains(
            repository
                .worktree
                .to_str()
                .expect("fixture path must be UTF-8")
        )
    );
}
