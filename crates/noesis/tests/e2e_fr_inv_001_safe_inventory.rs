mod support;

use std::ffi::OsString;
use std::fs;
use std::process::{Command, Output};
use std::thread;

use serde_json::{Value, json};

use support::s1::{
    COMMIT_A_OID, MaterializedRepository, REPOSITORY_ID, fixture_root, scan, scan_command,
};
use support::{parse_single_document, read_repository_text};

#[test]
fn e2e_fr_inv_001_safe_inventory() {
    let repository = MaterializedRepository::revision_a();
    let snapshot = successful_snapshot(&repository, COMMIT_A_OID);

    let mut expected_snapshot: Value = serde_json::from_slice(
        &fs::read(fixture_root().join("expected-snapshot-a.json"))
            .expect("read reviewed S1 snapshot golden"),
    )
    .expect("parse reviewed S1 snapshot golden");
    expected_snapshot["envelope"] = snapshot["envelope"].clone();
    assert_eq!(snapshot, expected_snapshot);

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

#[test]
fn e2e_fr_acq_002_traversal_rejected() {
    let repository = MaterializedRepository::revision_a();
    let canary = repository.root.join("outside-sentinel");
    fs::write(&canary, b"must-not-be-read-or-changed\n").expect("write outside-root canary");
    let expected_canary = fs::read(&canary).expect("read initial outside-root canary");

    let output = scan(&repository.worktree, repository.traversal_commit());
    assert_s1_acquisition_error(
        &output,
        &json!({
            "schema_version": "codenoesis.error/v2",
            "code": "acquisition.path_invalid",
            "stage": "acquisition",
            "message": "repository path is invalid for standard-local-s1",
            "retryable": false,
            "context": {"reason": "dot_component"}
        }),
    );
    assert_eq!(
        fs::read(&canary).expect("read outside-root canary after scan"),
        expected_canary
    );
}

#[test]
fn e2e_fr_acq_002_symlink_rejected() {
    let repository = MaterializedRepository::revision_a();
    let output = scan(&repository.worktree, repository.symlink_escape_commit());
    let expected: Value = serde_json::from_slice(
        &fs::read(fixture_root().join("expected-error-symlink.json"))
            .expect("read reviewed S1 symlink error"),
    )
    .expect("parse reviewed S1 symlink error");
    assert_s1_acquisition_error(&output, &expected);
}

#[test]
fn e2e_fr_acq_002_gitlink_rejected() {
    let repository = MaterializedRepository::revision_a();
    let output = scan(&repository.worktree, repository.gitlink_commit());
    assert_s1_acquisition_error(
        &output,
        &json!({
            "schema_version": "codenoesis.error/v2",
            "code": "acquisition.entry_policy_violation",
            "stage": "acquisition",
            "message": "repository entry violates the standard-local-s1 policy",
            "retryable": false,
            "context": {
                "entry": "gitlink",
                "path": "vendor"
            }
        }),
    );
}

#[test]
fn sec_nfr_sec_001_bombs_and_parser_inputs_are_bounded() {
    let repository = MaterializedRepository::revision_a();
    let output = scan(
        &repository.worktree,
        repository.single_file_over_limit_commit(),
    );
    let expected: Value = serde_json::from_slice(
        &fs::read(fixture_root().join("expected-error-file-limit.json"))
            .expect("read reviewed S1 file-limit error"),
    )
    .expect("parse reviewed S1 file-limit error");
    assert_s1_acquisition_error(&output, &expected);

    let at_limit = scan(
        &repository.worktree,
        repository.single_file_at_limit_commit(),
    );
    assert_eq!(at_limit.status.code(), Some(0));
    assert!(at_limit.stderr.is_empty());
    let snapshot = parse_single_document(&at_limit.stdout);
    assert_eq!(
        snapshot["semantic"]["inventory"]["summary"]["total_file_bytes"],
        4_194_304
    );

    let fanout_commit = repository.tree_fanout_over_limit_commit();
    let fanout = scan(&repository.worktree, &fanout_commit);
    assert_s1_acquisition_error(
        &fanout,
        &json!({
            "schema_version": "codenoesis.error/v2",
            "code": "acquisition.limit_exceeded",
            "stage": "acquisition",
            "message": "repository exceeds a standard-local-s1 limit",
            "retryable": false,
            "context": {
                "limit": "tree_entries",
                "maximum": 25_000,
                "observed": 25_001
            }
        }),
    );

    let archive = high_ratio_zip();
    let archive_commit =
        repository.generated_single_file_commit("archive.zip", &archive, 978_307_211);
    let archive_snapshot = successful_snapshot(&repository, &archive_commit);
    let archive_inventory = &archive_snapshot["semantic"]["inventory"];
    assert_eq!(
        archive_inventory["summary"]["total_file_bytes"],
        u64::try_from(archive.len()).expect("archive length fits u64")
    );
    assert_eq!(
        archive_inventory["unsupported_content"],
        json!([{
            "path": "archive.zip",
            "reason": "unsupported_extension",
            "evidence_id": "evidence-00001"
        }])
    );

    let malformed_utf8 = b"fn main() {}\n\xff";
    let malformed_commit =
        repository.generated_single_file_commit("malformed.rs", malformed_utf8, 978_307_212);
    let malformed_snapshot = successful_snapshot(&repository, &malformed_commit);
    let malformed_file = &malformed_snapshot["semantic"]["inventory"]["files"][0];
    assert_eq!(malformed_file["content_kind"], "binary_or_unknown");
    assert_eq!(malformed_file["languages"], json!(["rust"]));
    assert_eq!(malformed_file["roles"], json!(["source"]));

    let mut openapi_stress = b"openapi: 3.1.0\n".to_vec();
    openapi_stress.extend(std::iter::repeat_n(b'[', 1_048_576));
    let openapi_commit =
        repository.generated_single_file_commit("openapi.yaml", &openapi_stress, 978_307_213);
    let openapi_snapshot = successful_snapshot(&repository, &openapi_commit);
    let openapi_inventory = &openapi_snapshot["semantic"]["inventory"];
    assert_eq!(
        openapi_inventory["contracts"],
        json!([{
            "kind": "openapi",
            "path": "openapi.yaml",
            "status": "recognized_not_interpreted",
            "evidence_id": "evidence-00001"
        }])
    );
    assert_eq!(
        openapi_inventory["extraction_capabilities"],
        json!([
            {
                "capability": "contract_extraction",
                "subject": "contract:openapi",
                "status": "not_available"
            },
            {
                "capability": "file_classification",
                "subject": "repository",
                "status": "available"
            }
        ])
    );
}

#[test]
fn conf_dr_evd_001_source_evidence_resolves() {
    let repository = MaterializedRepository::revision_a();
    let snapshot = successful_snapshot(&repository, COMMIT_A_OID);
    let inventory = &snapshot["semantic"]["inventory"];
    let files = inventory["files"].as_array().expect("S1 files array");
    let evidence = inventory["evidence"].as_array().expect("S1 evidence array");
    assert_eq!(files.len(), evidence.len());

    for (file, record) in files.iter().zip(evidence) {
        assert_eq!(file["evidence_id"], record["evidence_id"]);
        assert_eq!(file["path"], record["path"]);
        assert_eq!(file["blob_oid"], record["blob_oid"]);
        assert_eq!(record["span"]["start"], 0);
        assert_eq!(record["span"]["end"], file["byte_length"]);
        assert_eq!(
            record["repository"]["identity"],
            "urn:codenoesis:fixture:s1-safe-inventory-v1"
        );
        assert_eq!(record["repository"]["commit_oid"], COMMIT_A_OID);
    }
}

#[test]
fn gt_fr_inv_001_exact_inventory_and_coverage_gaps() {
    let repository = MaterializedRepository::revision_a();
    let snapshot = successful_snapshot(&repository, COMMIT_A_OID);
    let expected: Value = serde_json::from_slice(
        &fs::read(fixture_root().join("expected-semantic-a.json"))
            .expect("read reviewed S1 semantic golden"),
    )
    .expect("parse reviewed S1 semantic golden");
    assert_eq!(
        snapshot["semantic"]["inventory"], expected["inventory"],
        "inventory and explicit coverage gaps must match the reviewed golden"
    );
}

#[test]
fn pt_fr_inv_001_public_inventory_is_replay_and_schedule_invariant() {
    let repository = MaterializedRepository::revision_a();
    let mut expected_semantic =
        read_repository_text(fixture_root().join("expected-semantic-a.jcs"));
    assert_eq!(expected_semantic.pop(), Some(b'\n'));

    for seed in 0..50 {
        let output = scheduled_scan(&repository.worktree, COMMIT_A_OID, seed);
        assert_eq!(output.status.code(), Some(0), "replay seed {seed}");
        assert!(output.stderr.is_empty(), "replay seed {seed}");
        let snapshot = parse_single_document(&output.stdout);
        assert_eq!(
            serde_json::to_vec(&snapshot["semantic"]).expect("serialize replay semantic"),
            expected_semantic,
            "replay seed {seed}"
        );
    }

    let mut workers = Vec::new();
    for schedule in 0..10 {
        let worktree = repository.worktree.clone();
        let expected_semantic = expected_semantic.clone();
        workers.push(thread::spawn(move || {
            let output = scheduled_scan(&worktree, COMMIT_A_OID, schedule);
            assert_eq!(
                output.status.code(),
                Some(0),
                "parallel schedule {schedule}"
            );
            assert!(output.stderr.is_empty(), "parallel schedule {schedule}");
            let snapshot = parse_single_document(&output.stdout);
            assert_eq!(
                serde_json::to_vec(&snapshot["semantic"]).expect("serialize parallel semantic"),
                expected_semantic,
                "parallel schedule {schedule}"
            );
        }));
    }
    for worker in workers {
        worker.join().expect("parallel S1 replay must not panic");
    }
}

#[test]
fn sec_nfr_sec_001_sentinel_scripts_never_execute() {
    let repository = MaterializedRepository::revision_a();
    let canary = repository.root.join("outside-sentinel");
    fs::write(&canary, b"must-not-be-read-or-changed\n").expect("write sentinel canary");
    let expected_canary = fs::read(&canary).expect("read initial sentinel canary");
    repository.apply_isolation_variant(&canary);

    let output = scan_command(&repository.worktree, COMMIT_A_OID)
        .env("CODENOESIS_SENTINEL", &canary)
        .output()
        .expect("launch isolated S1 scan");
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(
        fs::read(&canary).expect("read sentinel canary after scan"),
        expected_canary
    );
    let snapshot = parse_single_document(&output.stdout);
    let diagnostics = snapshot["semantic"]["inventory"]["diagnostics"]
        .as_array()
        .expect("S1 diagnostics array");
    assert_eq!(
        diagnostics
            .iter()
            .filter(|diagnostic| { diagnostic["code"] == "inventory.target_execution_suppressed" })
            .count(),
        2
    );
}

#[test]
fn conf_fr_inv_001_repository_snapshot_v2_and_error_v2() {
    let repository = MaterializedRepository::revision_a();
    let output = Command::new(env!("CARGO_BIN_EXE_noesis"))
        .args(["scan", "--repository"])
        .arg(&repository.worktree)
        .args([
            "--repository-id",
            REPOSITORY_ID,
            "--revision",
            COMMIT_A_OID,
            "--profile",
            "standard-local-invalid",
            "--format",
            "json",
        ])
        .output()
        .expect("scan with invalid S1 profile");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        parse_single_document(&output.stderr),
        json!({
            "schema_version": "codenoesis.error/v2",
            "code": "input.invalid_profile",
            "stage": "input",
            "message": "invalid profile",
            "retryable": false,
            "context": {}
        })
    );
}

#[test]
fn e2e_fr_acq_002_dirty_worktree_is_ignored() {
    let repository = MaterializedRepository::revision_a();
    fs::write(repository.worktree.join("build.rs"), b"panic!();\n")
        .expect("write dirty build script decoy");
    fs::write(
        repository.worktree.join("untracked-secret.txt"),
        b"not evidence\n",
    )
    .expect("write untracked decoy");

    let snapshot = successful_snapshot(&repository, COMMIT_A_OID);
    let expected: Value = serde_json::from_slice(
        &fs::read(fixture_root().join("expected-semantic-a.json"))
            .expect("read reviewed S1 semantic golden"),
    )
    .expect("parse reviewed S1 semantic golden");
    assert_eq!(snapshot["semantic"], expected);
}

#[cfg(unix)]
#[test]
fn e2e_fr_acq_002_symlinked_repository_root_is_rejected() {
    use std::os::unix::fs::symlink;

    let repository = MaterializedRepository::revision_a();
    let linked_root = repository.root.join("linked-repository");
    symlink(&repository.worktree, &linked_root).expect("create repository-root symlink");
    let output = scan(&linked_root, COMMIT_A_OID);
    assert_s1_acquisition_error(
        &output,
        &json!({
            "schema_version": "codenoesis.error/v2",
            "code": "acquisition.root_policy_violation",
            "stage": "acquisition",
            "message": "repository root violates the standard-local-s1 policy",
            "retryable": false,
            "context": {"policy": "repository_root_is_symlink"}
        }),
    );
}

#[cfg(unix)]
#[test]
fn e2e_fr_acq_002_symlinked_git_directory_is_rejected() {
    use std::os::unix::fs::symlink;

    let repository = MaterializedRepository::revision_a();
    let external_git = repository.root.join("external-git-directory");
    fs::rename(repository.worktree.join(".git"), &external_git)
        .expect("move Git directory outside repository root");
    symlink(&external_git, repository.worktree.join(".git"))
        .expect("create symlinked Git directory");
    let output = scan(&repository.worktree, COMMIT_A_OID);
    assert_s1_acquisition_error(
        &output,
        &json!({
            "schema_version": "codenoesis.error/v2",
            "code": "acquisition.root_policy_violation",
            "stage": "acquisition",
            "message": "repository root violates the standard-local-s1 policy",
            "retryable": false,
            "context": {"policy": "git_directory_is_symlink"}
        }),
    );
}

#[cfg(unix)]
#[test]
fn sec_nfr_sec_001_git_control_symlink_is_not_followed() {
    use std::os::unix::fs::symlink;

    let repository = MaterializedRepository::revision_a();
    let external_config = repository.root.join("external-git-config");
    fs::write(&external_config, b"[core]\n\tbare = true\n").expect("write external Git config");
    let config = repository.worktree.join(".git/config");
    fs::remove_file(&config).expect("remove in-root Git config");
    symlink(&external_config, &config).expect("create escaping Git config symlink");

    let output = scan(&repository.worktree, COMMIT_A_OID);
    assert_eq!(output.status.code(), Some(70));
    assert!(output.stdout.is_empty());
    assert_eq!(
        parse_single_document(&output.stderr),
        json!({
            "schema_version": "codenoesis.error/v2",
            "code": "internal.unexpected",
            "stage": "internal",
            "message": "unexpected internal failure",
            "retryable": false,
            "context": {}
        })
    );
}

#[cfg(unix)]
#[test]
fn sec_nfr_sec_001_loose_object_symlink_is_not_followed() {
    use std::os::unix::fs::symlink;

    const BLOB_OID: &str = "f2c75744ba1623ebd2bf64e8dda36fdb0fa86ab6";

    let repository = MaterializedRepository::revision_a();
    let object = repository
        .worktree
        .join(".git/objects")
        .join(&BLOB_OID[..2])
        .join(&BLOB_OID[2..]);
    let external_object = repository.root.join("external-loose-object");
    fs::rename(&object, &external_object).expect("move loose object outside repository root");
    symlink(&external_object, &object).expect("create escaping loose-object symlink");

    let output = scan(&repository.worktree, COMMIT_A_OID);
    assert_s1_acquisition_error(
        &output,
        &json!({
            "schema_version": "codenoesis.error/v2",
            "code": "acquisition.repository_inconsistent",
            "stage": "acquisition",
            "message": "Git repository is inconsistent",
            "retryable": false,
            "context": {
                "object_oid": BLOB_OID,
                "expected_kind": "blob"
            }
        }),
    );
}

#[test]
fn e2e_fr_acq_002_packed_object_database_is_rejected() {
    let repository = MaterializedRepository::revision_a();
    let pack = repository
        .worktree
        .join(".git/objects/pack/pack-sentinel.pack");
    fs::write(pack, b"not a pack\n").expect("write packed-object sentinel");
    let output = scan(&repository.worktree, COMMIT_A_OID);
    assert_s1_acquisition_error(
        &output,
        &json!({
            "schema_version": "codenoesis.error/v2",
            "code": "acquisition.unsupported_repository_shape",
            "stage": "acquisition",
            "message": "unsupported Git repository shape",
            "retryable": false,
            "context": {"feature": "packed_object_database"}
        }),
    );
}

#[test]
fn sec_nfr_sec_001_alternate_object_database_is_not_opened() {
    let repository = MaterializedRepository::revision_a();
    let outside_objects = repository.root.join("outside-objects");
    fs::create_dir(&outside_objects).expect("create alternate-object canary");
    let alternates = repository.worktree.join(".git/objects/info/alternates");
    fs::write(&alternates, format!("{}\n", outside_objects.display()))
        .expect("write alternate-object declaration");

    let output = scan(&repository.worktree, COMMIT_A_OID);
    assert_s1_acquisition_error(
        &output,
        &json!({
            "schema_version": "codenoesis.error/v2",
            "code": "acquisition.unsupported_repository_shape",
            "stage": "acquisition",
            "message": "unsupported Git repository shape",
            "retryable": false,
            "context": {"feature": "alternate_object_database"}
        }),
    );
}

#[test]
fn e2e_fr_acq_002_non_git_uses_error_v2() {
    let repository = MaterializedRepository::revision_a();
    let plain = repository.root.join("plain-directory");
    fs::create_dir(&plain).expect("create non-Git S1 source");
    let output = scan(&plain, COMMIT_A_OID);
    assert_s1_acquisition_error(
        &output,
        &json!({
            "schema_version": "codenoesis.error/v2",
            "code": "acquisition.not_git_repository",
            "stage": "acquisition",
            "message": "not a supported Git worktree",
            "retryable": false,
            "context": {}
        }),
    );
}

fn successful_snapshot(repository: &MaterializedRepository, revision: &str) -> Value {
    let output = scan(&repository.worktree, revision);
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
    snapshot
}

fn assert_s1_acquisition_error(output: &Output, expected: &Value) {
    assert_eq!(output.status.code(), Some(10));
    assert!(output.stdout.is_empty(), "failed S1 stdout must be empty");
    assert_eq!(&parse_single_document(&output.stderr), expected);
}

fn high_ratio_zip() -> Vec<u8> {
    let script = r#"
import io
import sys
import zipfile

buffer = io.BytesIO()
entry = zipfile.ZipInfo("payload.txt", (2000, 1, 1, 0, 0, 0))
entry.compress_type = zipfile.ZIP_DEFLATED
entry.external_attr = 0o100644 << 16
with zipfile.ZipFile(buffer, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
    archive.writestr(entry, b"a" * 1048576)
sys.stdout.buffer.write(buffer.getvalue())
"#;
    let output = Command::new("python3")
        .args(["-c", script])
        .output()
        .expect("generate deterministic high-ratio ZIP");
    assert!(
        output.status.success(),
        "ZIP generator failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn scheduled_scan(repository: &std::path::Path, revision: &str, seed: usize) -> Output {
    let mut arguments = vec![
        (
            OsString::from("--repository"),
            repository.as_os_str().to_owned(),
        ),
        (
            OsString::from("--repository-id"),
            OsString::from(REPOSITORY_ID),
        ),
        (OsString::from("--revision"), OsString::from(revision)),
        (
            OsString::from("--profile"),
            OsString::from("standard-local-s1"),
        ),
        (OsString::from("--format"), OsString::from("json")),
    ];
    let length = arguments.len();
    arguments.rotate_left(seed % length);
    if seed % 2 == 1 {
        arguments.reverse();
    }

    let mut command = Command::new(env!("CARGO_BIN_EXE_noesis"));
    command.arg("scan");
    for (flag, value) in arguments {
        command.arg(flag).arg(value);
    }
    command.output().expect("launch scheduled S1 scan")
}
