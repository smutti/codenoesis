mod support;

use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use codenoesis_domain::s1_boundaries::{
    MAX_BOUNDARY_MANIFEST_BYTES, MAX_EXPLICIT_NESTED_REPOSITORIES,
};
use serde_json::{Value, json};

use support::parse_single_document;
use support::s1_boundaries::{
    GITLINK_OID, MaterializedBoundaryRepository, ROOT_REPOSITORY_ID, expected_bound_boundaries,
    expected_unbound_boundaries,
};

const EXPECTED_RED_STDERR: &[u8] = b"{\"code\":\"input.invalid_revision\",\"context\":{},\"message\":\"invalid revision\",\"retryable\":false,\"schema_version\":\"codenoesis.error/v4\",\"stage\":\"input\"}\n";

#[test]
fn e2e_fr_acq_005_gitlink_boundaries() {
    let repository = MaterializedBoundaryRepository::unbound();
    let output = repository.scan_unbound();

    if !output.status.success() {
        assert_eq!(output.status.code(), Some(2), "unexpected Red exit");
        assert!(output.stdout.is_empty(), "Red stdout must be empty");
        assert_eq!(output.stderr, EXPECTED_RED_STDERR, "unexpected Red stderr");
        assert!(
            !repository.base.store.exists(),
            "Red must not create the store"
        );
        panic!(
            "expected R2 RepositorySnapshotV5 success; observed approved pre-R2 selector rejection"
        );
    }

    assert!(output.stderr.is_empty(), "successful stderr must be empty");
    let snapshot: Value =
        serde_json::from_slice(&output.stdout).expect("parse RepositorySnapshotV5");
    assert_eq!(
        snapshot["schema_version"],
        "codenoesis.repository-snapshot/v5"
    );
    assert_eq!(
        snapshot["semantic"]["repository_boundaries"],
        expected_unbound_boundaries()
    );
}

#[test]
fn sec_fr_acq_005_no_ambient_nested_authority() {
    let repository = MaterializedBoundaryRepository::unbound();
    let absent = successful_snapshot(repository.scan_unbound());
    let ambient = repository.base.worktree.join("external/nested-model");
    fs::create_dir_all(&ambient).expect("create ambient nested worktree");
    fs::write(ambient.join("credential-canary"), b"must-not-be-read")
        .expect("write ambient canary");
    let present = successful_snapshot(
        repository.scan_to_store(&repository.base.root.join("store-present"), None),
    );
    assert_eq!(present["semantic"], absent["semantic"]);
    assert_eq!(present["semantic_hash"], absent["semantic_hash"]);
}

#[test]
fn gt_fr_acq_005_explicit_nested_binding() {
    let repository = MaterializedBoundaryRepository::unbound();
    let nested = repository.materialize_nested("generated/nested-model");
    let loose_manifest = repository.write_matching_manifest(false);
    let loose = successful_snapshot(repository.scan_with_manifest(&loose_manifest));
    assert_eq!(
        loose["semantic"]["repository_boundaries"],
        expected_bound_boundaries()
    );

    repository.repack_nested(&nested);
    let packed_manifest = repository.write_matching_manifest(true);
    let packed = successful_snapshot(repository.scan_to_store(
        &repository.base.root.join("store-packed-nested"),
        Some(&packed_manifest),
    ));
    assert_eq!(packed["semantic"], loose["semantic"]);
    assert_eq!(packed["semantic_hash"], loose["semantic_hash"]);
}

#[test]
fn conf_fr_acq_005_v5_store_docs_query() {
    let repository = MaterializedBoundaryRepository::unbound();
    let snapshot = successful_snapshot(repository.scan_unbound());
    let entity_id = snapshot["semantic"]["knowledge_graph"]["entities"][0]["id"]
        .as_str()
        .expect("V5 root entity ID");
    let docs = repository.docs();
    assert!(docs.status.success(), "docs stderr={:?}", docs.stderr);
    assert!(docs.stderr.is_empty());
    let manifest = parse_single_document(&docs.stdout);
    assert_eq!(
        manifest["snapshot_semantic_hash"]["value"],
        snapshot["semantic_hash"]["value"]
    );
    let query = repository.query(entity_id);
    assert!(query.status.success(), "query stderr={:?}", query.stderr);
    assert!(query.stderr.is_empty());
    let result = parse_single_document(&query.stdout);
    assert_eq!(result["requested_id"], entity_id);
    assert_eq!(result["result_kind"], "entity");
}

#[test]
fn reg_fr_acq_004_packed_sha1_composes_with_r2() {
    let repository = MaterializedBoundaryRepository::unbound();
    let loose = successful_snapshot(
        repository.scan_to_store(&repository.base.root.join("store-loose-root"), None),
    );
    repository.repack_root();
    let packed = successful_snapshot(
        repository.scan_packed_root(&repository.base.root.join("store-packed-root")),
    );
    assert_eq!(packed["semantic"], loose["semantic"]);
    assert_eq!(packed["semantic_hash"], loose["semantic_hash"]);
}

#[test]
fn conf_fr_acq_005_selector_profile_matrix() {
    let repository = MaterializedBoundaryRepository::unbound();
    let cases = [
        (Some("standard-local-s3"), Some("local-gitlinks-v1"), None),
        (Some("standard-local-s4"), Some("unknown"), None),
        (
            Some("standard-local-s4"),
            None,
            Some(Path::new("manifest.json")),
        ),
        (None, Some("local-gitlinks-v1"), None),
    ];
    for (index, (profile, selector, manifest)) in cases.into_iter().enumerate() {
        let store = repository.base.root.join(format!("invalid-store-{index}"));
        let output = raw_scan(&repository, &store, profile, selector, manifest);
        let error = assert_v9(output, 2, "input.invalid_repository_boundary_profile");
        assert_eq!(error["context"], json!({}));
        assert!(!store.exists());
    }

    let output = Command::new(env!("CARGO_BIN_EXE_noesis"))
        .args(["docs", "--repository-boundary-profile", "local-gitlinks-v1"])
        .output()
        .expect("launch invalid docs selector");
    assert_v9(output, 2, "input.invalid_repository_boundary_profile");
}

#[test]
fn conf_fr_acq_005_boundary_manifest_failures_are_side_effect_free() {
    let repository = MaterializedBoundaryRepository::unbound();
    let unavailable_store = repository.base.root.join("store-unavailable");
    let unavailable = repository.scan_to_store(
        &unavailable_store,
        Some(&repository.base.root.join("absent.json")),
    );
    let error = assert_v9(unavailable, 2, "input.invalid_repository_boundary_manifest");
    assert_eq!(error["context"]["reason"], "manifest_unavailable");
    assert!(!unavailable_store.exists());

    let mismatch_manifest = repository.base.root.join("root-mismatch.json");
    fs::write(
        &mismatch_manifest,
        serde_json::to_vec(&json!({
            "schema_version": "codenoesis.repository-boundary-input/v1",
            "root": {
                "repository_identity": ROOT_REPOSITORY_ID,
                "commit_oid": "1111111111111111111111111111111111111111"
            },
            "nested_repositories": []
        }))
        .unwrap(),
    )
    .unwrap();
    let mismatch_store = repository.base.root.join("store-root-mismatch");
    let mismatch = repository.scan_to_store(&mismatch_store, Some(&mismatch_manifest));
    let error = assert_v9(mismatch, 2, "input.invalid_repository_boundary_manifest");
    assert_eq!(error["context"]["reason"], "root_mismatch");
    assert!(!mismatch_store.exists());

    let duplicate_manifest = repository.base.root.join("duplicate.json");
    fs::write(
        &duplicate_manifest,
        format!(
            "{{\"schema_version\":\"codenoesis.repository-boundary-input/v1\",\"schema_version\":\"codenoesis.repository-boundary-input/v1\",\"root\":{{\"repository_identity\":\"{ROOT_REPOSITORY_ID}\",\"commit_oid\":\"{}\"}},\"nested_repositories\":[]}}",
            repository.commit_oid
        ),
    )
    .unwrap();
    let duplicate_store = repository.base.root.join("store-duplicate");
    let duplicate = repository.scan_to_store(&duplicate_store, Some(&duplicate_manifest));
    let error = assert_v9(duplicate, 2, "input.invalid_repository_boundary_manifest");
    assert_eq!(error["context"]["reason"], "schema_invalid");
    assert!(!duplicate_store.exists());
}

#[test]
fn gt_fr_acq_005_nested_failure_precedence() {
    let mismatch_repository = MaterializedBoundaryRepository::unbound();
    fs::create_dir_all(
        mismatch_repository
            .base
            .root
            .join("generated/nested-model-mismatch"),
    )
    .unwrap();
    let mismatch_manifest = mismatch_repository.copy_manifest("boundary-input-mismatch.json");
    let mismatch = assert_v9(
        mismatch_repository.scan_with_manifest(&mismatch_manifest),
        10,
        "acquisition.nested_repository_mismatch",
    );
    assert_eq!(mismatch["context"]["expected_oid"], GITLINK_OID);
    assert_eq!(
        mismatch["context"]["observed_oid"],
        "1111111111111111111111111111111111111111"
    );

    let unavailable_repository = MaterializedBoundaryRepository::unbound();
    fs::create_dir_all(
        unavailable_repository
            .base
            .root
            .join("generated/nested-model"),
    )
    .unwrap();
    let unavailable_manifest = unavailable_repository.write_matching_manifest(false);
    let unavailable = assert_v9(
        unavailable_repository.scan_with_manifest(&unavailable_manifest),
        10,
        "acquisition.nested_repository_unavailable",
    );
    assert_eq!(unavailable["context"]["reason"], "not_git_repository");
}

#[cfg(unix)]
#[test]
fn sec_fr_acq_005_nested_symlink_escape_is_rejected() {
    use std::os::unix::fs::symlink;

    let repository = MaterializedBoundaryRepository::unbound();
    fs::create_dir_all(repository.base.root.join("generated")).unwrap();
    symlink(
        std::env::temp_dir(),
        repository.base.root.join("generated/nested-model"),
    )
    .unwrap();
    let manifest = repository.write_matching_manifest(false);
    let error = assert_v9(
        repository.scan_with_manifest(&manifest),
        10,
        "acquisition.nested_repository_unavailable",
    );
    assert_eq!(error["context"]["reason"], "path_invalid");
    assert!(!repository.base.store.exists());
}

#[test]
fn pt_fr_acq_005_public_limits_use_128_gitlinks_and_256_sections() {
    let gitlinks = (0..128)
        .map(|index| {
            (
                format!("gitlink-{index:03}"),
                "1111111111111111111111111111111111111111".to_owned(),
            )
        })
        .collect::<Vec<_>>();
    let maximum = MaterializedBoundaryRepository::custom(None, &gitlinks);
    let snapshot = successful_snapshot(maximum.scan_unbound());
    assert_eq!(
        snapshot["semantic"]["repository_boundaries"]["summary"]["boundary_count"],
        128
    );

    let mut plus_one_gitlinks = gitlinks;
    plus_one_gitlinks.push((
        "gitlink-128".to_owned(),
        "1111111111111111111111111111111111111111".to_owned(),
    ));
    let plus_one = MaterializedBoundaryRepository::custom(None, &plus_one_gitlinks);
    let error = assert_v9(
        plus_one.scan_unbound(),
        10,
        "acquisition.repository_boundary_limit_exceeded",
    );
    assert_eq!(error["context"]["limit"], "gitlink_entries");
    assert_eq!(error["context"]["maximum"], 128);
    assert_eq!(error["context"]["observed"], 129);

    let maximum_sections =
        MaterializedBoundaryRepository::custom(Some(&gitmodules_sections(256)), &[]);
    let snapshot = successful_snapshot(maximum_sections.scan_unbound());
    assert_eq!(
        snapshot["semantic"]["repository_boundaries"]["summary"]["declaration_count"],
        256
    );
    let plus_one_sections =
        MaterializedBoundaryRepository::custom(Some(&gitmodules_sections(257)), &[]);
    let error = assert_v9(
        plus_one_sections.scan_unbound(),
        10,
        "acquisition.repository_boundary_limit_exceeded",
    );
    assert_eq!(error["context"]["limit"], "gitmodules_sections");
    assert_eq!(error["context"]["maximum"], 256);
    assert_eq!(error["context"]["observed"], 257);
}

#[test]
fn pt_fr_acq_005_manifest_bytes_have_public_max_and_plus_one() {
    let repository = MaterializedBoundaryRepository::unbound();
    let base = serde_json::to_vec(&json!({
        "schema_version": "codenoesis.repository-boundary-input/v1",
        "root": {
            "repository_identity": ROOT_REPOSITORY_ID,
            "commit_oid": repository.commit_oid
        },
        "nested_repositories": []
    }))
    .unwrap();
    let maximum = usize::try_from(MAX_BOUNDARY_MANIFEST_BYTES).unwrap();
    let mut bytes = base;
    bytes.resize(maximum, b' ');
    let maximum_manifest = repository.base.root.join("manifest-at-limit.json");
    fs::write(&maximum_manifest, &bytes).unwrap();
    let snapshot = successful_snapshot(repository.scan_to_store(
        &repository.base.root.join("store-manifest-at-limit"),
        Some(&maximum_manifest),
    ));
    assert_eq!(
        snapshot["semantic"]["repository_boundaries"]["summary"]["bound_count"],
        0
    );

    bytes.push(b' ');
    let plus_one_manifest = repository.base.root.join("manifest-over-limit.json");
    fs::write(&plus_one_manifest, bytes).unwrap();
    let store = repository.base.root.join("store-manifest-over-limit");
    let error = assert_v9(
        repository.scan_to_store(&store, Some(&plus_one_manifest)),
        10,
        "acquisition.repository_boundary_limit_exceeded",
    );
    assert_eq!(error["context"]["limit"], "boundary_manifest_bytes");
    assert_eq!(error["context"]["maximum"], 262_144);
    assert_eq!(error["context"]["observed"], 262_145);
    assert!(!store.exists());
}

#[test]
fn pt_fr_acq_005_explicit_roots_have_public_max_and_plus_one() {
    let maximum = usize::try_from(MAX_EXPLICIT_NESTED_REPOSITORIES).unwrap();
    let gitlinks = (0..maximum)
        .map(|index| (format!("nested-{index:02}"), GITLINK_OID.to_owned()))
        .collect::<Vec<_>>();
    let repository = MaterializedBoundaryRepository::custom(None, &gitlinks);
    for index in 0..maximum {
        repository.materialize_nested(&format!("generated/nested-{index:02}"));
    }
    let manifest = write_nested_manifest(&repository, maximum);
    let snapshot = successful_snapshot(repository.scan_with_manifest(&manifest));
    assert_eq!(
        snapshot["semantic"]["repository_boundaries"]["summary"]["bound_count"],
        32
    );
    assert_eq!(
        snapshot["semantic"]["repository_boundaries"]["summary"]["coverage_gap_count"], 64,
        "each bound undeclared root retains missing-declaration and not-analyzed gaps"
    );

    let plus_one_repository = MaterializedBoundaryRepository::unbound();
    let plus_one_manifest = write_nested_manifest(&plus_one_repository, maximum + 1);
    let store = plus_one_repository
        .base
        .root
        .join("store-explicit-roots-over-limit");
    let error = assert_v9(
        plus_one_repository.scan_to_store(&store, Some(&plus_one_manifest)),
        10,
        "acquisition.repository_boundary_limit_exceeded",
    );
    assert_eq!(error["context"]["limit"], "explicit_nested_repositories");
    assert_eq!(error["context"]["maximum"], 32);
    assert_eq!(error["context"]["observed"], 33);
    assert!(!store.exists());
}

#[test]
fn sec_fr_acq_005_url_canary_never_reaches_output_or_store() {
    let canary = "https://user:secret@example.invalid/model?token=value#fragment";
    let gitmodules = format!(
        "[submodule \"model\"]\npath = external-model\nurl = {canary}\nbranch = secret-branch\n"
    );
    let repository = MaterializedBoundaryRepository::custom(
        Some(gitmodules.as_bytes()),
        &[("external-model".to_owned(), GITLINK_OID.to_owned())],
    );
    let output = repository.scan_unbound();
    assert!(output.status.success(), "stderr={:?}", output.stderr);
    assert!(!contains(&output.stdout, canary.as_bytes()));
    assert!(!contains(&output.stdout, b"secret-branch"));
    assert!(!contains(&output.stderr, canary.as_bytes()));
    assert_directory_redacted(&repository.base.store, canary.as_bytes());
    assert_directory_redacted(&repository.base.store, b"secret-branch");
}

fn successful_snapshot(output: Output) -> Value {
    let Output {
        status,
        stdout,
        stderr,
    } = output;
    assert!(status.success(), "stderr={stderr:?}");
    assert!(stderr.is_empty());
    parse_single_document(&stdout)
}

fn assert_v9(output: Output, exit_code: i32, code: &str) -> Value {
    let Output {
        status,
        stdout,
        stderr,
    } = output;
    assert_eq!(status.code(), Some(exit_code));
    assert!(stdout.is_empty());
    let error = parse_single_document(&stderr);
    assert_eq!(error["schema_version"], "codenoesis.error/v9");
    assert_eq!(error["code"], code);
    error
}

fn raw_scan(
    repository: &MaterializedBoundaryRepository,
    store: &Path,
    profile: Option<&str>,
    selector: Option<&str>,
    manifest: Option<&Path>,
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_noesis"));
    command
        .args(["scan", "--repository"])
        .arg(&repository.base.worktree)
        .args([
            "--repository-id",
            ROOT_REPOSITORY_ID,
            "--revision",
            &repository.commit_oid,
        ]);
    if let Some(profile) = profile {
        command.args(["--profile", profile]);
    }
    if let Some(selector) = selector {
        command.args(["--repository-boundary-profile", selector]);
    }
    if let Some(manifest) = manifest {
        command.arg("--repository-boundary-manifest").arg(manifest);
    }
    command.arg("--store").arg(store).args(["--format", "json"]);
    command.output().expect("launch raw R2 scan")
}

fn gitmodules_sections(count: usize) -> Vec<u8> {
    let mut contents = String::new();
    for index in 0..count {
        writeln!(
            contents,
            "[submodule \"s{index}\"]\npath = orphan-{index:03}\nurl = https:orphan-{index:03}"
        )
        .expect("write gitmodules section fixture");
    }
    contents.into_bytes()
}

fn write_nested_manifest(
    repository: &MaterializedBoundaryRepository,
    count: usize,
) -> std::path::PathBuf {
    let nested_repositories = (0..count)
        .map(|index| {
            json!({
                "boundary_path": format!("nested-{index:02}"),
                "repository_identity": format!(
                    "urn:codenoesis:repository:nested-{index:02}"
                ),
                "repository_root": format!("generated/nested-{index:02}"),
                "revision": GITLINK_OID,
                "acquisition_profile": "verified-loose-sha1-v1"
            })
        })
        .collect::<Vec<_>>();
    let path = repository
        .base
        .root
        .join(format!("boundary-input-{count}.json"));
    fs::write(
        &path,
        serde_json::to_vec(&json!({
            "schema_version": "codenoesis.repository-boundary-input/v1",
            "root": {
                "repository_identity": ROOT_REPOSITORY_ID,
                "commit_oid": repository.commit_oid
            },
            "nested_repositories": nested_repositories
        }))
        .unwrap(),
    )
    .unwrap();
    path
}

fn assert_directory_redacted(root: &Path, canary: &[u8]) {
    for entry in fs::read_dir(root).expect("read R2 store") {
        let path = entry.expect("read R2 store entry").path();
        if path.is_dir() {
            assert_directory_redacted(&path, canary);
        } else {
            let bytes = fs::read(&path).expect("read R2 store file");
            assert!(
                !contains(&bytes, canary),
                "canary retained in {}",
                path.display()
            );
        }
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}
