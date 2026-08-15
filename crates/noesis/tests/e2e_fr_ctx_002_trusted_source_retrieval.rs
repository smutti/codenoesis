#[path = "support/s4_r18.rs"]
mod s4_r18;
mod support;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};

use s4_r18::{
    MaterializedTrustedSourceRepository, SIGNATURE_EVIDENCE_ID, expected_source_excerpt,
    expected_source_stdout,
};
use support::parse_single_document;
use support::s4_r17::ROOT_CALLABLE_ID;

fn assert_success(output: &Output, label: &str) {
    assert!(
        output.status.success(),
        "{label} failed: status={:?}; stdout={}; stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "{label} stderr must be empty");
}

fn assert_error(output: &Output, exit: i32, code: &str) -> serde_json::Value {
    assert_eq!(
        output.status.code(),
        Some(exit),
        "unexpected status: stdout={}; stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty(), "R18 failure stdout must be empty");
    let error = parse_single_document(&output.stderr);
    assert_eq!(error["schema_version"], "codenoesis.error/v29");
    assert_eq!(error["code"], code);
    assert_eq!(error["retryable"], false);
    assert_eq!(error["context"], serde_json::json!({}));
    error
}

#[test]
fn e2e_fr_ctx_002_retrieves_exact_committed_excerpt() {
    let repository = MaterializedTrustedSourceRepository::fixture();
    let scan = repository.inherited.scan();
    assert_success(&scan, "R18 fixture scan");
    let docs = repository.inherited.docs();
    assert_success(&docs, "R18 documentation");

    let context_output = repository.inherited.query_context(ROOT_CALLABLE_ID);
    assert_success(&context_output, "R18 inherited function context");
    let context = parse_single_document(&context_output.stdout);
    assert!(
        context["evidence"]
            .as_array()
            .expect("R18 context evidence")
            .iter()
            .any(|record| record["id"] == SIGNATURE_EVIDENCE_ID),
        "reviewed signature evidence must be navigable from FunctionContextV1"
    );

    let source = repository.source(SIGNATURE_EVIDENCE_ID);
    assert_success(&source, "R18 trusted source excerpt");
    assert_eq!(source.stdout, expected_source_stdout());
    assert_eq!(
        parse_single_document(&source.stdout),
        expected_source_excerpt()
    );
}

#[test]
fn e2e_fr_ctx_002_loose_packed_boundary_and_ten_schedules_are_identical() {
    let repository = MaterializedTrustedSourceRepository::fixture();
    assert_success(&repository.inherited.scan(), "R18 fixture scan");
    let expected = expected_source_stdout();
    let store_before = directory_snapshot(&repository.inherited.store);

    let loose = repository.source(SIGNATURE_EVIDENCE_ID);
    assert_success(&loose, "R18 loose retrieval");
    assert_eq!(loose.stdout, expected);

    fs::write(
        repository.inherited.worktree.join("src/lib.rs"),
        b"R18_WORKING_TREE_PRIVACY_CANARY\n",
    )
    .expect("replace mutable working-tree bytes");
    let immutable = repository.source(SIGNATURE_EVIDENCE_ID);
    assert_success(&immutable, "R18 immutable loose retrieval");
    assert_eq!(immutable.stdout, expected);
    assert!(!contains_subslice(
        &immutable.stdout,
        b"R18_WORKING_TREE_PRIVACY_CANARY"
    ));

    let boundary = repository.source_with_boundaries(SIGNATURE_EVIDENCE_ID);
    assert_success(&boundary, "R18 boundary-aware retrieval");
    assert_eq!(boundary.stdout, expected);

    let packed = support::s1_packed::materialize_base_only_pack_at(
        &repository.inherited.root,
        &repository.inherited.worktree,
    );
    let packed_output = repository.source_packed(SIGNATURE_EVIDENCE_ID);
    assert_success(&packed_output, "R18 packed retrieval");
    assert_eq!(packed_output.stdout, expected);

    let schedules = std::thread::scope(|scope| {
        (0..10)
            .map(|_| {
                scope.spawn(|| {
                    let output = repository.source_packed(SIGNATURE_EVIDENCE_ID);
                    assert_success(&output, "R18 scheduled packed retrieval");
                    output.stdout
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|handle| handle.join().expect("R18 source schedule"))
            .collect::<Vec<_>>()
    });
    for (schedule, observed) in schedules.into_iter().enumerate() {
        assert_eq!(observed, expected, "R18 process schedule {schedule}");
    }
    packed.assert_unchanged();

    let store_after = directory_snapshot(&repository.inherited.store);
    assert_eq!(
        store_after, store_before,
        "source command mutated the store"
    );
    let expected_value = expected_source_excerpt();
    let excerpt = expected_value["excerpt"]["text"]
        .as_str()
        .expect("reviewed excerpt")
        .as_bytes();
    assert!(
        store_after
            .values()
            .all(|bytes| !contains_subslice(bytes, excerpt)),
        "source excerpt was retained in the store"
    );
}

#[test]
fn conf_fr_cli_011_invalid_bindings_fail_closed_without_private_context() {
    let repository = MaterializedTrustedSourceRepository::fixture();
    assert_success(&repository.inherited.scan(), "R18 fixture scan");

    let unknown = format!("urn:codenoesis:evidence:blake3:{}", "f".repeat(64));
    assert_error(&repository.source(&unknown), 2, "source.evidence_not_found");

    let wrong_revision = repository
        .source_command_at(
            &repository.inherited.worktree,
            &"0".repeat(40),
            support::s4_r17::REPOSITORY_ID,
            &repository.inherited.store,
            SIGNATURE_EVIDENCE_ID,
        )
        .output()
        .expect("wrong-revision source command");
    assert_error(&wrong_revision, 2, "source.repository_mismatch");

    let wrong_identity = repository
        .source_command_at(
            &repository.inherited.worktree,
            &repository.inherited.commit_oid,
            "urn:codenoesis:fixture:r18-wrong",
            &repository.inherited.store,
            SIGNATURE_EVIDENCE_ID,
        )
        .output()
        .expect("wrong-identity source command");
    assert_error(&wrong_identity, 2, "source.invalid_snapshot");

    let missing_store = repository.inherited.root.join("missing-store");
    let stale = repository
        .source_command_at(
            &repository.inherited.worktree,
            &repository.inherited.commit_oid,
            support::s4_r17::REPOSITORY_ID,
            &missing_store,
            SIGNATURE_EVIDENCE_ID,
        )
        .output()
        .expect("missing-store source command");
    assert_error(&stale, 2, "source.invalid_snapshot");

    let missing_repository = repository.inherited.root.join("private-missing-repository");
    let privacy_canary = "R18_PRIVATE_ENVIRONMENT_CANARY";
    let rejected = repository
        .source_command_at(
            &missing_repository,
            &repository.inherited.commit_oid,
            support::s4_r17::REPOSITORY_ID,
            &repository.inherited.store,
            SIGNATURE_EVIDENCE_ID,
        )
        .env("R18_PRIVATE_CANARY", privacy_canary)
        .output()
        .expect("missing-repository source command");
    let error = assert_error(&rejected, 2, "source.acquisition_rejected");
    let stderr = serde_json::to_string(&error).expect("R18 private error");
    assert!(!stderr.contains(privacy_canary));
    assert!(!stderr.contains(&missing_repository.to_string_lossy().into_owned()));

    let invalid_revision = repository
        .source_command_at(
            &repository.inherited.worktree,
            &repository.inherited.commit_oid.to_ascii_uppercase(),
            support::s4_r17::REPOSITORY_ID,
            &repository.inherited.store,
            SIGNATURE_EVIDENCE_ID,
        )
        .output()
        .expect("invalid-revision source command");
    assert_error(&invalid_revision, 2, "source.invalid_arguments");

    let independent = repository
        .source_command(SIGNATURE_EVIDENCE_ID)
        .current_dir(std::env::temp_dir())
        .env("R18_PRIVATE_CANARY", privacy_canary)
        .output()
        .expect("CWD-independent source command");
    assert_success(&independent, "R18 CWD-independent retrieval");
    assert_eq!(independent.stdout, expected_source_stdout());
    assert!(
        !independent
            .stdout
            .windows(privacy_canary.len())
            .any(|bytes| { bytes == privacy_canary.as_bytes() })
    );
}

#[test]
fn sec_fr_ctx_002_packed_corruption_is_a_typed_private_failure() {
    let repository = MaterializedTrustedSourceRepository::fixture();
    assert_success(&repository.inherited.scan(), "R18 fixture scan");
    let mut packed = support::s1_packed::materialize_base_only_pack_at(
        &repository.inherited.root,
        &repository.inherited.worktree,
    );
    packed.mutate(support::s1_packed::PackedMutation::PackChecksum);
    let output = repository.source_packed(SIGNATURE_EVIDENCE_ID);
    assert_error(&output, 2, "source.acquisition_rejected");
}

#[cfg(unix)]
#[test]
fn race_fr_ctx_002_packed_replacement_is_success_or_typed_failure() {
    let repository = MaterializedTrustedSourceRepository::fixture();
    assert_success(&repository.inherited.scan(), "R18 fixture scan");
    let packed = support::s1_packed::materialize_base_only_pack_at(
        &repository.inherited.root,
        &repository.inherited.worktree,
    );
    let replacement = repository.inherited.root.join("r18-pack-replacement");
    let mut command = repository.source_command(SIGNATURE_EVIDENCE_ID);
    command
        .args([
            "--acquisition-profile",
            codenoesis_domain::s1_packed::LOCAL_GIT_SHA1_PACKED_V1,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let child = command.spawn().expect("launch R18 packed race subject");
    fs::rename(&packed.pack_path, &replacement).expect("schedule R18 pack replacement");
    let output = child
        .wait_with_output()
        .expect("wait for R18 packed race subject");
    fs::rename(&replacement, &packed.pack_path).expect("restore R18 raced pack");

    match output.status.code() {
        Some(0) => {
            assert!(output.stderr.is_empty());
            assert_eq!(output.stdout, expected_source_stdout());
        }
        Some(2) => {
            assert!(output.stdout.is_empty());
            let error = parse_single_document(&output.stderr);
            assert_eq!(error["schema_version"], "codenoesis.error/v29");
            assert!(matches!(
                error["code"].as_str(),
                Some("source.acquisition_rejected" | "source.unstable_input")
            ));
            assert_eq!(error["context"], serde_json::json!({}));
        }
        status => panic!(
            "R18 packed race produced forbidden status {status:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        ),
    }
    packed.assert_unchanged();
}

#[cfg(unix)]
#[test]
fn sec_fr_cli_011_symlink_repository_root_is_rejected() {
    use std::os::unix::fs::symlink;

    let repository = MaterializedTrustedSourceRepository::fixture();
    assert_success(&repository.inherited.scan(), "R18 fixture scan");
    let link = repository.inherited.root.join("repository-link");
    symlink(&repository.inherited.worktree, &link).expect("R18 repository symlink");
    let output = repository
        .source_command_at(
            &link,
            &repository.inherited.commit_oid,
            support::s4_r17::REPOSITORY_ID,
            &repository.inherited.store,
            SIGNATURE_EVIDENCE_ID,
        )
        .output()
        .expect("symlink-root source command");
    assert_error(&output, 2, "source.path_rejected");
}

#[test]
fn conf_fr_cli_011_stdout_failure_uses_internal_error_v29() {
    let repository = MaterializedTrustedSourceRepository::fixture();
    assert_success(&repository.inherited.scan(), "R18 fixture scan");
    let (stdout_reader, stdout_writer) = std::io::pipe().expect("R18 stdout pipe");
    drop(stdout_reader);
    let output = repository
        .source_command(SIGNATURE_EVIDENCE_ID)
        .stdout(Stdio::from(stdout_writer))
        .stderr(Stdio::piped())
        .output()
        .expect("R18 stdout-failure source command");
    assert_error(&output, 1, "source.internal");
}

fn directory_snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, current: &Path, output: &mut BTreeMap<PathBuf, Vec<u8>>) {
        let mut entries = fs::read_dir(current)
            .unwrap_or_else(|error| panic!("read directory {}: {error}", current.display()))
            .map(|entry| entry.expect("read directory entry").path())
            .collect::<Vec<_>>();
        entries.sort();
        for path in entries {
            let metadata = fs::symlink_metadata(&path).expect("R18 store metadata");
            if metadata.is_dir() {
                visit(root, &path, output);
            } else if metadata.is_file() {
                output.insert(
                    path.strip_prefix(root)
                        .expect("R18 relative store path")
                        .to_owned(),
                    fs::read(&path).expect("R18 store bytes"),
                );
            } else {
                panic!("unexpected R18 store entry: {}", path.display());
            }
        }
    }

    let mut output = BTreeMap::new();
    visit(root, root, &mut output);
    output
}

fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}
