#[path = "support/s7_r19.rs"]
mod s7_r19;
mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;

use s7_r19::MaterializedGitImpactWorkspace;
use serde_json::Value;

#[test]
fn conf_fr_imp_006_all_git_evidence_is_independently_navigable() {
    let workspace = MaterializedGitImpactWorkspace::reviewed();
    let output = workspace.impact();
    if !output.status.success() {
        assert_eq!(output.status.code(), Some(2), "unexpected R19 Red exit");
        assert!(output.stdout.is_empty(), "R19 Red wrote a partial report");
        let error: serde_json::Value =
            serde_json::from_slice(&output.stderr).expect("parse R19 Red error");
        if error["schema_version"] == "codenoesis.error/v23" {
            assert_eq!(error["code"], "impact.invalid_workspace");
            assert_eq!(error["context"]["reason"], "invalid_profile");
            panic!(
                "expected R19 Git-backed impact success; observed the approved absent-profile Red"
            );
        }
        panic!("R19 Git-backed impact failed after registration: {error}");
    }

    assert!(
        output.stderr.is_empty(),
        "successful R19 impact wrote stderr"
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse R19 report V2");
    assert_eq!(
        report["schema_version"],
        "codenoesis.semantic-compatibility-report/v2"
    );
    assert_eq!(report["semantic_diffs"].as_array().map(Vec::len), Some(2));
    assert_eq!(
        report["client_assessments"].as_array().map(Vec::len),
        Some(2)
    );
    assert_eq!(
        report["rejected_candidates"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(report["evidence"].as_array().map(Vec::len), Some(9));
    assert_eq!(report["coverage_gaps"].as_array().map(Vec::len), Some(1));
    assert_semantic_oracle(&report);

    let mutable_canary = "R19_MUTABLE_WORKING_TREE_PRIVACY_CANARY";
    fs::write(
        workspace
            .root()
            .join("strict/src/commonMain/kotlin/dev/codenoesis/fixture/StrictUsersClient.kt"),
        mutable_canary,
    )
    .expect("replace R19 mutable working-tree source");
    let immutable = workspace.impact();
    assert_success(&immutable, "R19 immutable loose replay");
    assert_eq!(immutable.stdout, output.stdout);
    assert!(!contains_subslice(
        &immutable.stdout,
        mutable_canary.as_bytes()
    ));

    let report_path = workspace.write_report(&output.stdout);
    let evidence = report["evidence"].as_array().expect("R19 evidence");
    let loose_excerpts = evidence
        .iter()
        .map(|record| {
            let source = workspace.impact_source(&report_path, record);
            assert_success(&source, "R19 loose source navigation");
            assert_excerpt(record, &source.stdout);
            source.stdout
        })
        .collect::<Vec<_>>();

    let packs = pack_repositories(&workspace);
    let packed = impact_packed(&workspace);
    assert_success(&packed, "R19 packed impact");
    assert_eq!(packed.stdout, output.stdout);
    for (record, expected) in evidence.iter().zip(loose_excerpts) {
        let source = impact_source_packed(&workspace, &report_path, record);
        assert_success(&source, "R19 packed source navigation");
        assert_eq!(source.stdout, expected);
    }
    for pack in packs {
        pack.assert_unchanged();
    }
}

#[test]
fn pt_nfr_det_001_r19_fifty_permutations_and_ten_schedules_are_identical() {
    let workspace = MaterializedGitImpactWorkspace::reviewed();
    let expected = workspace.impact();
    assert_success(&expected, "R19 loose determinism baseline");
    let packs = pack_repositories(&workspace);

    for seed in 0..50 {
        permute_clients(&workspace, seed);
        let output = impact_packed(&workspace);
        assert_success(&output, "R19 packed permutation");
        assert_eq!(output.stdout, expected.stdout, "R19 permutation {seed}");
    }

    let schedules = thread::scope(|scope| {
        (0..10)
            .map(|schedule| {
                let workspace = &workspace;
                scope.spawn(move || {
                    let output = impact_packed(workspace);
                    assert_success(&output, "R19 packed schedule");
                    (schedule, output.stdout)
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|handle| handle.join().expect("R19 schedule panicked"))
            .collect::<Vec<_>>()
    });
    for (schedule, observed) in schedules {
        assert_eq!(observed, expected.stdout, "R19 process schedule {schedule}");
    }
    for pack in packs {
        pack.assert_unchanged();
    }
}

#[test]
fn conf_fr_cli_012_invalid_authority_and_limits_fail_closed() {
    let workspace = MaterializedGitImpactWorkspace::reviewed();
    let mut manifest = manifest_value(&workspace);
    manifest["federation_report"]["sha256"] = Value::String("0".repeat(64));
    write_manifest(&workspace, &manifest);
    assert_error(&workspace.impact(), "impact_git.invalid_federation_report");

    let workspace = MaterializedGitImpactWorkspace::reviewed();
    let mut manifest = manifest_value(&workspace);
    manifest["provider"]["baseline"]["revision"] = Value::String("0".repeat(40));
    write_manifest(&workspace, &manifest);
    assert_error(&workspace.impact(), "impact_git.acquisition_rejected");

    let workspace = MaterializedGitImpactWorkspace::reviewed();
    fs::write(&workspace.manifest, vec![b'x'; 1_048_577])
        .expect("write R19 maximum-plus-one workspace");
    assert_error(&workspace.impact(), "impact_git.limit_exceeded");
}

#[test]
fn conf_fr_cli_012_report_binding_mismatches_are_private() {
    let workspace = MaterializedGitImpactWorkspace::reviewed();
    let impact = workspace.impact();
    assert_success(&impact, "R19 binding fixture impact");
    let report: Value = serde_json::from_slice(&impact.stdout).expect("parse R19 binding report");
    let evidence = report["evidence"]
        .as_array()
        .and_then(|records| records.first())
        .expect("R19 binding evidence")
        .clone();
    let privacy_canary = "R19_PRIVATE_ENVIRONMENT_CANARY";

    for (field, replacement, expected) in [
        (
            "tree_oid",
            "0".repeat(40),
            "impact_source.repository_mismatch",
        ),
        (
            "blob_oid",
            "0".repeat(40),
            "impact_source.repository_mismatch",
        ),
    ] {
        let mut changed = report.clone();
        changed["evidence"][0]["source_binding"][field] = Value::String(replacement);
        let path = write_canonical_report(&workspace, field, &changed);
        let output = impact_source_command(&workspace, &path, &evidence)
            .env("R19_PRIVATE_CANARY", privacy_canary)
            .output()
            .expect("launch R19 mismatched binding");
        let error = assert_error(&output, expected);
        let stderr = serde_json::to_string(&error).expect("serialize R19 private error");
        assert!(!stderr.contains(privacy_canary));
        assert!(!stderr.contains(&workspace.root().to_string_lossy().into_owned()));
    }

    let mut changed = report;
    let start = changed["evidence"][0]["source_binding"]["span"]["start"]
        .as_u64()
        .expect("R19 span start");
    changed["evidence"][0]["source_binding"]["span"]["start"] = Value::from(start + 1);
    let path = write_canonical_report(&workspace, "span", &changed);
    assert_error(
        &workspace.impact_source(&path, &evidence),
        "impact_source.invalid_evidence",
    );

    let impact = workspace.impact();
    assert_success(&impact, "R19 excerpt-digest fixture impact");
    let mut changed: Value =
        serde_json::from_slice(&impact.stdout).expect("parse R19 excerpt-digest report");
    changed["evidence"][0]["excerpt_sha256"] = Value::String("0".repeat(64));
    let path = write_canonical_report(&workspace, "excerpt-digest", &changed);
    assert_error(
        &workspace.impact_source(&path, &evidence),
        "impact_source.content_rejected",
    );
}

#[cfg(unix)]
#[test]
fn sec_fr_cli_012_symlink_repository_and_packed_race_fail_closed() {
    use std::os::unix::fs::symlink;

    let workspace = MaterializedGitImpactWorkspace::reviewed();
    let impact = workspace.impact();
    assert_success(&impact, "R19 race fixture impact");
    let report: Value = serde_json::from_slice(&impact.stdout).expect("parse R19 race report");
    let evidence = report["evidence"]
        .as_array()
        .expect("R19 race evidence")
        .iter()
        .find(|record| record["repository_identity"] == s7_r19::PROVIDER_ID)
        .expect("R19 provider race evidence");
    let report_path = workspace.write_report(&impact.stdout);

    let repository = workspace.root().join("provider");
    let repository_link = workspace.root().join("provider-link");
    symlink(&repository, &repository_link).expect("create R19 repository symlink");
    let output = impact_source_command_at(&workspace, repository_link, &report_path, evidence)
        .output()
        .expect("launch R19 symlink source");
    assert_error(&output, "impact_source.invalid_arguments");

    let mut packs = pack_repositories(&workspace);
    let provider_pack = packs.remove(0);
    let replacement = workspace.root().join("r19-pack-replacement");
    let mut command = impact_source_command(&workspace, &report_path, evidence);
    command
        .args([
            "--acquisition-profile",
            codenoesis_domain::s1_packed::LOCAL_GIT_SHA1_PACKED_V1,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let child = command.spawn().expect("launch R19 packed race subject");
    fs::rename(&provider_pack.pack_path, &replacement).expect("schedule R19 pack replacement");
    let output = child.wait_with_output().expect("wait for R19 packed race");
    fs::rename(&replacement, &provider_pack.pack_path).expect("restore R19 raced pack");
    match output.status.code() {
        Some(0) => assert_success(&output, "R19 packed race success"),
        Some(2) => {
            let error: Value = assert_error(&output, "impact_source.acquisition_rejected");
            assert_eq!(error["context"], serde_json::json!({}));
        }
        status => panic!("R19 packed race produced forbidden status {status:?}"),
    }
    provider_pack.assert_unchanged();
    for pack in packs {
        pack.assert_unchanged();
    }
}

#[test]
fn conf_fr_cli_012_stdout_failure_uses_internal_error_v30() {
    let workspace = MaterializedGitImpactWorkspace::reviewed();
    let (stdout_reader, stdout_writer) = std::io::pipe().expect("R19 stdout pipe");
    drop(stdout_reader);
    let output = workspace
        .impact_command()
        .stdout(Stdio::from(stdout_writer))
        .stderr(Stdio::piped())
        .output()
        .expect("R19 stdout-failure impact command");
    assert_error(&output, "impact_source.internal");
}

fn assert_semantic_oracle(report: &Value) {
    let assessments = report["client_assessments"]
        .as_array()
        .expect("R19 client assessments");
    let strict = assessments
        .iter()
        .find(|assessment| assessment["repository_identity"] == s7_r19::STRICT_ID)
        .expect("R19 strict assessment");
    assert_eq!(strict["presence_assumption"], "requires_present");
    assert_eq!(strict["target_impact"], "breaking");
    let safe = assessments
        .iter()
        .find(|assessment| assessment["repository_identity"] == s7_r19::SAFE_ID)
        .expect("R19 safe assessment");
    assert_eq!(safe["presence_assumption"], "handles_absent");
    assert_eq!(safe["target_impact"], "compatible");
    assert_eq!(
        report["rejected_candidates"][0]["repository_identity"],
        s7_r19::DECOY_ID
    );
    assert_eq!(
        report["rejected_candidates"][0]["reason_code"],
        "operation_identity_mismatch"
    );
    assert_eq!(
        report["coverage_gaps"][0]["reason_code"],
        "unsupported_custom_provider_mapping"
    );
    let nickname = report["semantic_diffs"]
        .as_array()
        .expect("R19 semantic diffs")
        .iter()
        .find(|diff| diff["field_pointer"] == "/nickname")
        .expect("R19 nickname diff");
    assert_eq!(nickname["contract"]["delta"], "unchanged");
    assert_eq!(nickname["implementation"]["before"], "guaranteed_present");
    assert_eq!(nickname["implementation"]["after"], "may_be_absent");
    assert_eq!(nickname["classification"], "breaking");
}

fn assert_excerpt(evidence: &Value, stdout: &[u8]) {
    let excerpt: Value = serde_json::from_slice(stdout).expect("parse R19 source excerpt");
    assert_eq!(
        excerpt["schema_version"],
        "codenoesis.trusted-impact-source-excerpt/v1"
    );
    assert_eq!(excerpt["evidence"]["id"], evidence["id"]);
    assert_eq!(excerpt["evidence"]["path"], evidence["path"]);
    assert_eq!(
        excerpt["evidence"]["blob_oid"],
        evidence["source_binding"]["blob_oid"]
    );
    assert_eq!(
        excerpt["evidence"]["span"],
        evidence["source_binding"]["span"]
    );
    assert!(
        excerpt["excerpt"]["byte_length"]
            .as_u64()
            .is_some_and(|length| length > 0)
    );
}

fn assert_success(output: &Output, label: &str) {
    assert!(
        output.status.success(),
        "{label} failed: status={:?}; stdout={}; stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "{label} wrote stderr");
}

fn assert_error(output: &Output, code: &str) -> Value {
    assert_eq!(
        output.status.code(),
        Some(if code == "impact_source.internal" {
            1
        } else {
            2
        }),
        "unexpected R19 failure: stdout={}; stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty(), "R19 failure wrote stdout");
    let error: Value = serde_json::from_slice(&output.stderr).expect("parse R19 ErrorV30");
    assert_eq!(error["schema_version"], "codenoesis.error/v30");
    assert_eq!(error["code"], code);
    assert_eq!(error["retryable"], false);
    assert_eq!(error["context"], serde_json::json!({}));
    error
}

fn pack_repositories(
    workspace: &MaterializedGitImpactWorkspace,
) -> Vec<support::s1_packed::PackedMaterialization> {
    ["provider", "decoy", "safe", "strict"]
        .into_iter()
        .map(|name| {
            support::s1_packed::materialize_base_only_pack_at(
                workspace.root(),
                &workspace.root().join(name),
            )
        })
        .collect()
}

fn permute_clients(workspace: &MaterializedGitImpactWorkspace, seed: usize) {
    let mut manifest = manifest_value(workspace);
    let clients = manifest["clients"].as_array_mut().expect("R19 clients");
    let length = clients.len();
    clients.rotate_left(seed % length);
    if seed % 2 == 1 {
        clients.reverse();
    }
    write_manifest(workspace, &manifest);
}

fn write_canonical_report(
    workspace: &MaterializedGitImpactWorkspace,
    name: &str,
    value: &Value,
) -> std::path::PathBuf {
    let path = workspace.root().join(format!("changed-{name}-report.json"));
    let mut bytes = serde_json::to_vec(value).expect("serialize changed R19 report");
    bytes.push(b'\n');
    fs::write(&path, bytes).expect("write changed R19 report");
    path
}

fn impact_packed(workspace: &MaterializedGitImpactWorkspace) -> Output {
    let mut command = workspace.impact_command();
    command.args([
        "--acquisition-profile",
        codenoesis_domain::s1_packed::LOCAL_GIT_SHA1_PACKED_V1,
    ]);
    command.output().expect("launch packed R19 impact")
}

fn impact_source_packed(
    workspace: &MaterializedGitImpactWorkspace,
    report: &Path,
    evidence: &Value,
) -> Output {
    let mut command = impact_source_command(workspace, report, evidence);
    command.args([
        "--acquisition-profile",
        codenoesis_domain::s1_packed::LOCAL_GIT_SHA1_PACKED_V1,
    ]);
    command.output().expect("launch packed R19 impact source")
}

fn impact_source_command(
    workspace: &MaterializedGitImpactWorkspace,
    report: &Path,
    evidence: &Value,
) -> Command {
    let repository_id = evidence["repository_identity"]
        .as_str()
        .expect("R19 evidence repository identity");
    impact_source_command_at(
        workspace,
        repository_for(workspace, repository_id),
        report,
        evidence,
    )
}

fn impact_source_command_at(
    workspace: &MaterializedGitImpactWorkspace,
    repository: PathBuf,
    report: &Path,
    evidence: &Value,
) -> Command {
    let repository_id = evidence["repository_identity"]
        .as_str()
        .expect("R19 evidence repository identity");
    let revision = evidence["revision"]
        .as_str()
        .expect("R19 evidence revision");
    let evidence_id = evidence["id"].as_str().expect("R19 evidence ID");
    let mut command = Command::new(env!("CARGO_BIN_EXE_noesis"));
    command
        .current_dir(workspace.root())
        .args(["impact-source", "--repository"])
        .arg(repository)
        .args(["--repository-id", repository_id, "--revision", revision])
        .arg("--report")
        .arg(report)
        .args([
            "--evidence-id",
            evidence_id,
            "--source-profile",
            s7_r19::SOURCE_PROFILE,
            "--format",
            "json",
        ]);
    command
}

fn repository_for(workspace: &MaterializedGitImpactWorkspace, identity: &str) -> PathBuf {
    let name = match identity {
        s7_r19::PROVIDER_ID => "provider",
        s7_r19::DECOY_ID => "decoy",
        s7_r19::SAFE_ID => "safe",
        s7_r19::STRICT_ID => "strict",
        _ => panic!("unknown R19 fixture repository identity"),
    };
    workspace.root().join(name)
}

fn manifest_value(workspace: &MaterializedGitImpactWorkspace) -> Value {
    serde_json::from_slice(&fs::read(&workspace.manifest).expect("read R19 workspace"))
        .expect("parse R19 workspace")
}

fn write_manifest(workspace: &MaterializedGitImpactWorkspace, value: &Value) {
    fs::write(
        &workspace.manifest,
        serde_json::to_vec(value).expect("serialize R19 workspace"),
    )
    .expect("write R19 workspace");
}

fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}
