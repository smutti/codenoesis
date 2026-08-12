mod support;

use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use std::thread;

use support::parse_single_document;
use support::s7::{MaterializedImpactWorkspace, reviewed_golden};

#[test]
fn e2e_fr_imp_004_implementation_aware_api_diff() {
    let workspace = MaterializedImpactWorkspace::reviewed();
    let files_before = relative_files(workspace.root());
    let output = invoke(&workspace);

    if !output.status.success() {
        assert_eq!(output.status.code(), Some(2), "unexpected impact exit");
        assert!(output.stdout.is_empty(), "failed impact wrote a report");
        let error = parse_single_document(&output.stderr);
        assert_eq!(error["schema_version"], "codenoesis.error/v23");
        assert_eq!(
            error["code"], "impact.unsupported_implementation_semantics",
            "S7 product Red reached the wrong boundary"
        );
        assert_eq!(error["stage"], "impact");
        panic!("expected S7 impact success; observed the approved unsupported-semantics Red");
    }

    assert!(output.stderr.is_empty(), "successful impact wrote stderr");
    assert_eq!(
        output.stdout,
        reviewed_golden(),
        "S7 semantic compatibility report differs from the immutable golden"
    );
    assert_eq!(relative_files(workspace.root()), files_before);
    let stdout = String::from_utf8(output.stdout).expect("S7 report UTF-8");
    assert!(!stdout.contains(&workspace.root().display().to_string()));
    assert!(!stdout.contains("custom_profile_fields(user)"));
    assert!(!stdout.contains("payload.getValue"));
}

#[test]
fn pt_nfr_det_001_s7_replays_and_schedules_are_byte_identical() {
    let expected = reviewed_golden();
    let workspace = MaterializedImpactWorkspace::reviewed();
    for seed in 0..50 {
        permute_clients(&workspace, seed);
        let output = invoke(&workspace);
        assert!(output.status.success(), "sequential replay {seed} failed");
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, expected, "sequential replay {seed}");
    }

    let schedules = (0..10)
        .map(|seed| {
            let expected = expected.clone();
            thread::spawn(move || {
                let workspace = MaterializedImpactWorkspace::reviewed();
                permute_clients(&workspace, seed);
                let output = invoke(&workspace);
                assert!(output.status.success(), "parallel schedule {seed} failed");
                assert!(output.stderr.is_empty());
                assert_eq!(output.stdout, expected, "parallel schedule {seed}");
            })
        })
        .collect::<Vec<_>>();
    for schedule in schedules {
        schedule.join().expect("S7 schedule panicked");
    }
}

#[test]
fn sec_fr_cli_006_traversal_oversize_and_malformed_source_fail_closed() {
    let workspace = MaterializedImpactWorkspace::reviewed();
    let mut manifest = workspace.manifest_value();
    manifest["provider"]["baseline"]["root"] = "../provider/revision-a".into();
    workspace.write_manifest(&manifest);
    assert_failure(&invoke(&workspace), "impact.invalid_workspace", None);

    let workspace = MaterializedImpactWorkspace::reviewed();
    fs::write(
        workspace
            .root()
            .join("provider/revision-a/src/user_response.rs"),
        vec![b'x'; 2_097_153],
    )
    .expect("write oversized S7 source");
    assert_failure(
        &invoke(&workspace),
        "impact.limit_exceeded",
        Some(("source_bytes_per_file", 2_097_153)),
    );

    let workspace = MaterializedImpactWorkspace::reviewed();
    let malformed = b"fn user_response( {";
    fs::write(
        workspace
            .root()
            .join("provider/revision-a/src/user_response.rs"),
        malformed,
    )
    .expect("write malformed S7 source");
    let mut manifest = workspace.manifest_value();
    manifest["provider"]["baseline"]["source"]["sha256"] = sha256(malformed).into();
    workspace.write_manifest(&manifest);
    assert_failure(
        &invoke(&workspace),
        "impact.unsupported_implementation_semantics",
        None,
    );
}

#[cfg(unix)]
#[test]
fn sec_fr_cli_006_symlinked_source_is_rejected() {
    use std::os::unix::fs::symlink;

    let workspace = MaterializedImpactWorkspace::reviewed();
    let source = workspace
        .root()
        .join("provider/revision-a/src/user_response.rs");
    fs::remove_file(&source).expect("remove reviewed source");
    symlink("../../revision-b/src/user_response.rs", &source).expect("create source symlink");
    assert_failure(&invoke(&workspace), "impact.invalid_workspace", None);
}

fn invoke(workspace: &MaterializedImpactWorkspace) -> Output {
    Command::new(env!("CARGO_BIN_EXE_noesis"))
        .args(["impact", "--workspace"])
        .arg(&workspace.manifest)
        .args([
            "--profile",
            "implementation-aware-http-json-v1",
            "--format",
            "json",
        ])
        .output()
        .expect("launch noesis impact")
}

fn assert_failure(output: &Output, code: &str, limit: Option<(&str, u64)>) {
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error = parse_single_document(&output.stderr);
    assert_eq!(error["schema_version"], "codenoesis.error/v23");
    assert_eq!(error["code"], code);
    if let Some((limit, observed)) = limit {
        assert_eq!(error["context"]["limit"], limit);
        assert_eq!(error["context"]["observed"], observed);
    }
}

fn permute_clients(workspace: &MaterializedImpactWorkspace, seed: usize) {
    let mut manifest = workspace.manifest_value();
    let clients = manifest["clients"].as_array_mut().expect("S7 clients");
    let length = clients.len();
    clients.rotate_left(seed % length);
    if seed % 2 == 1 {
        clients.reverse();
    }
    workspace.write_manifest(&manifest);
}

fn relative_files(root: &Path) -> Vec<String> {
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).expect("read S7 fixture directory") {
            let entry = entry.expect("read S7 fixture entry");
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                files.push(
                    path.strip_prefix(root)
                        .expect("relative S7 path")
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    files.sort();
    files
}

fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(64);
    for byte in digest {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}
