use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

const SOURCE_COMMIT: &str = "c5d259d7689b8a49527f8322b606e58cc0e1e61d";
const CARGO_LOCK_SHA256: &str = "434cc5e8e38a4c57f35990431d4682974b6cae94893860e1948c8f7cc21ffbca";
const SERDE_JSON_SHA256: &str = "c841b55ecdae098c80dcae9cf767f6f8a0c2cdb3416bbef72181df4d0fe73f14";
const AUDIT_DATABASE_COMMIT: &str = "69f93e1d081d8b6fbee010e48f0b5e0d13661415";

#[cfg(all(target_arch = "x86_64", target_os = "linux", target_env = "gnu"))]
const TARGET_ORACLE: Option<TargetOracle> = Some(TargetOracle {
    target: "x86_64-unknown-linux-gnu",
    manifest: include_bytes!(
        "../../tests/specifications/g8/local-release-candidate-v1/expected-manifest-x86_64-unknown-linux-gnu.json"
    ),
    verification: include_bytes!(
        "../../tests/specifications/g8/local-release-candidate-v1/expected-verification-x86_64-unknown-linux-gnu.json"
    ),
});
#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
const TARGET_ORACLE: Option<TargetOracle> = Some(TargetOracle {
    target: "aarch64-apple-darwin",
    manifest: include_bytes!(
        "../../tests/specifications/g8/local-release-candidate-v1/expected-manifest-aarch64-apple-darwin.json"
    ),
    verification: include_bytes!(
        "../../tests/specifications/g8/local-release-candidate-v1/expected-verification-aarch64-apple-darwin.json"
    ),
});
#[cfg(all(target_arch = "x86_64", target_os = "windows", target_env = "msvc"))]
const TARGET_ORACLE: Option<TargetOracle> = Some(TargetOracle {
    target: "x86_64-pc-windows-msvc",
    manifest: include_bytes!(
        "../../tests/specifications/g8/local-release-candidate-v1/expected-manifest-x86_64-pc-windows-msvc.json"
    ),
    verification: include_bytes!(
        "../../tests/specifications/g8/local-release-candidate-v1/expected-verification-x86_64-pc-windows-msvc.json"
    ),
});
#[cfg(not(any(
    all(target_arch = "x86_64", target_os = "linux", target_env = "gnu"),
    all(target_arch = "aarch64", target_os = "macos"),
    all(target_arch = "x86_64", target_os = "windows", target_env = "msvc")
)))]
const TARGET_ORACLE: Option<TargetOracle> = None;

#[derive(Clone, Copy)]
struct TargetOracle {
    target: &'static str,
    manifest: &'static [u8],
    verification: &'static [u8],
}

#[test]
fn e2e_fr_rel_003_packages_and_verifies_exact_candidate() {
    let Some(oracle) = TARGET_ORACLE else {
        return;
    };
    let expected_manifest = canonical_lf_golden(oracle.manifest);
    let expected_verification = canonical_lf_golden(oracle.verification);
    let expected_verification_value: Value =
        serde_json::from_slice(&expected_verification).expect("parse verification golden");
    let candidate_name = expected_verification_value["candidate_name"]
        .as_str()
        .expect("candidate name")
        .to_owned();

    let test_directory = tempfile::tempdir().expect("create test directory");
    let bundle = package_g1a_fixture(test_directory.path());
    let supply = test_directory.path().join("supply-chain");
    fs::create_dir(&supply).expect("create supply-chain fixture root");
    write_supply_chain_fixture(&supply, oracle.target);
    let bundle_snapshot = snapshot_tree(&bundle);
    let supply_snapshot = snapshot_tree(&supply);

    let mut first_candidate_snapshot = None;
    let mut first_candidate = None;
    for construction in 0..50 {
        let output_root = test_directory
            .path()
            .join(format!("candidate-{construction}"));
        fs::create_dir(&output_root).expect("create candidate output root");
        let output = run_package(&bundle, &supply, &output_root, construction % 2 == 1);
        fail_if_expected_checkpoint_red(&output, &output_root);
        assert_success(&output, &expected_manifest);

        let candidate = output_root.join(&candidate_name);
        assert!(candidate.is_dir());
        assert_eq!(
            fs::read(candidate.join("manifest.json")).expect("read candidate manifest"),
            expected_manifest
        );
        let snapshot = snapshot_tree(&candidate);
        if let Some(expected) = &first_candidate_snapshot {
            assert_eq!(&snapshot, expected);
        } else {
            first_candidate_snapshot = Some(snapshot);
            first_candidate = Some(candidate);
        }
    }

    let candidate = first_candidate.expect("first candidate path");
    for _schedule in 0..10 {
        let output = run_verify(&candidate);
        assert_success(&output, &expected_verification);
    }

    assert_eq!(snapshot_tree(&bundle), bundle_snapshot);
    assert_eq!(snapshot_tree(&supply), supply_snapshot);
}

fn package_g1a_fixture(root: &Path) -> PathBuf {
    let output_root = root.join("g1a");
    fs::create_dir(&output_root).expect("create G1a output root");
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("package-local-cli")
        .arg("--binary")
        .arg(g1_specification_path("fixtures/noesis-v1.bin"))
        .arg("--output")
        .arg(&output_root)
        .env("HOME", "/private/absolute/root")
        .env("HTTPS_PROXY", "https://private.invalid")
        .output()
        .expect("package G1a fixture");
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.ends_with(b"\n"));
    assert!(output.stderr.is_empty());
    let entries = fs::read_dir(&output_root)
        .expect("read G1a output")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect G1a output");
    assert_eq!(entries.len(), 1);
    entries[0].path()
}

fn write_supply_chain_fixture(root: &Path, target: &str) {
    write_json(
        &root.join("advisory-report.json"),
        &json!({
            "schema_version": "codenoesis.local-advisory-report/v1",
            "cargo_lock_sha256": CARGO_LOCK_SHA256,
            "tool": {"name": "cargo-audit", "version": "0.22.2"},
            "database": {
                "commit": AUDIT_DATABASE_COMMIT,
                "updated": "2026-08-12T12:42:29+02:00"
            },
            "status": "accepted",
            "vulnerabilities": [],
            "warnings": []
        }),
    );
    write_json(
        &root.join("dependency-lock.json"),
        &json!({
            "schema_version": "codenoesis.local-dependency-lock/v1",
            "target": target,
            "root": "noesis@0.1.0",
            "cargo_lock_sha256": CARGO_LOCK_SHA256,
            "packages": [
                {
                    "id": "noesis@0.1.0",
                    "name": "noesis",
                    "version": "0.1.0",
                    "source": "workspace",
                    "checksum": null,
                    "dependencies": ["serde_json@1.0.151"]
                },
                {
                    "id": "serde_json@1.0.151",
                    "name": "serde_json",
                    "version": "1.0.151",
                    "source": "registry+https://github.com/rust-lang/crates.io-index",
                    "checksum": SERDE_JSON_SHA256,
                    "dependencies": []
                }
            ],
            "dependency_edges": 1
        }),
    );
    write_json(
        &root.join("license-report.json"),
        &json!({
            "schema_version": "codenoesis.local-license-report/v1",
            "target": target,
            "cargo_lock_sha256": CARGO_LOCK_SHA256,
            "policy": "codenoesis.local-release-policy/v1",
            "status": "accepted",
            "packages": [
                {"id": "noesis@0.1.0", "expression": "Apache-2.0", "decision": "allowed"},
                {"id": "serde_json@1.0.151", "expression": "MIT OR Apache-2.0", "decision": "allowed"}
            ],
            "exceptions": []
        }),
    );
    write_json(
        &root.join("unsafe-inventory.json"),
        &json!({
            "schema_version": "codenoesis.local-unsafe-inventory/v1",
            "target": target,
            "cargo_lock_sha256": CARGO_LOCK_SHA256,
            "method": "conservative-rust-token-scan-v1",
            "status": "accepted",
            "packages": [
                {"id": "noesis@0.1.0", "rust_files": 1, "unsafe_tokens": 0, "exception_id": null},
                {"id": "serde_json@1.0.151", "rust_files": 69, "unsafe_tokens": 20, "exception_id": "unsafe-serde-json-1-0-151"}
            ],
            "exceptions": [
                {"id": "unsafe-serde-json-1-0-151", "package": "serde_json", "version": "1.0.151", "owner": "@smutti", "expires_on": "2026-11-14"}
            ]
        }),
    );
    write_json(
        &root.join("sbom.cdx.json"),
        &json!({
            "bomFormat": "CycloneDX",
            "specVersion": "1.6",
            "serialNumber": format!("urn:uuid:{}", deterministic_uuid(target)),
            "version": 1,
            "metadata": {
                "component": {
                    "type": "application",
                    "bom-ref": "pkg:cargo/noesis@0.1.0",
                    "name": "noesis",
                    "version": "0.1.0"
                },
                "properties": [
                    {"name": "codenoesis:cargo-lock-sha256", "value": CARGO_LOCK_SHA256},
                    {"name": "codenoesis:target", "value": target}
                ]
            },
            "components": [
                {
                    "type": "library",
                    "bom-ref": "pkg:cargo/serde_json@1.0.151",
                    "name": "serde_json",
                    "version": "1.0.151",
                    "hashes": [{"alg": "SHA-256", "content": SERDE_JSON_SHA256}],
                    "licenses": [{"expression": "MIT OR Apache-2.0"}],
                    "purl": "pkg:cargo/serde_json@1.0.151"
                }
            ],
            "dependencies": [
                {"ref": "pkg:cargo/noesis@0.1.0", "dependsOn": ["pkg:cargo/serde_json@1.0.151"]},
                {"ref": "pkg:cargo/serde_json@1.0.151", "dependsOn": []}
            ]
        }),
    );
}

fn write_json(path: &Path, value: &Value) {
    let mut bytes = serde_json::to_vec(value).expect("serialize fixture JSON");
    bytes.push(b'\n');
    fs::write(path, bytes).expect("write fixture JSON");
}

fn deterministic_uuid(target: &str) -> String {
    let mut digest = Sha256::digest(format!("{target}\0{CARGO_LOCK_SHA256}")).to_vec();
    digest[6] = (digest[6] & 0x0f) | 0x50;
    digest[8] = (digest[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        digest[0],
        digest[1],
        digest[2],
        digest[3],
        digest[4],
        digest[5],
        digest[6],
        digest[7],
        digest[8],
        digest[9],
        digest[10],
        digest[11],
        digest[12],
        digest[13],
        digest[14],
        digest[15]
    )
}

fn run_package(bundle: &Path, supply: &Path, output_root: &Path, reverse_pairs: bool) -> Output {
    let pairs = if reverse_pairs {
        vec![
            ("--output", output_root.as_os_str()),
            ("--supply-chain", supply.as_os_str()),
            ("--source-commit", std::ffi::OsStr::new(SOURCE_COMMIT)),
            ("--bundle", bundle.as_os_str()),
        ]
    } else {
        vec![
            ("--bundle", bundle.as_os_str()),
            ("--source-commit", std::ffi::OsStr::new(SOURCE_COMMIT)),
            ("--supply-chain", supply.as_os_str()),
            ("--output", output_root.as_os_str()),
        ]
    };
    let mut command = Command::new(env!("CARGO_BIN_EXE_xtask"));
    command.arg("package-local-release-candidate");
    for (flag, value) in pairs {
        command.arg(flag).arg(value);
    }
    command
        .env("HOME", "/private/absolute/root")
        .env("HTTPS_PROXY", "https://private.invalid")
        .env("CODE_NOESIS_PRIVATE_CANARY", "credential-private-canary")
        .output()
        .expect("package local release candidate")
}

fn run_verify(candidate: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("verify-local-release-candidate")
        .arg("--candidate")
        .arg(candidate)
        .env("HOME", "/private/absolute/root")
        .env("HTTPS_PROXY", "https://private.invalid")
        .env("CODE_NOESIS_PRIVATE_CANARY", "credential-private-canary")
        .output()
        .expect("verify local release candidate")
}

fn fail_if_expected_checkpoint_red(output: &Output, output_root: &Path) {
    const EXPECTED: &[u8] = b"{\"code\":\"distribution.invalid_arguments\",\"context\":{},\"message\":\"invalid distribution command\",\"retryable\":false,\"schema_version\":\"codenoesis.error/v26\",\"stage\":\"input\"}\n";
    if output.status.code() == Some(2) && output.stdout.is_empty() && output.stderr == EXPECTED {
        assert_eq!(
            fs::read_dir(output_root).expect("read output root").count(),
            0
        );
        panic!(
            "expected Red: exact base rejects package-local-release-candidate as distribution.invalid_arguments"
        );
    }
}

fn assert_success(output: &Output, expected: &[u8]) {
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, expected);
}

fn g1_specification_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/specifications/g1/local-cli-distribution-v1")
        .join(relative)
}

fn canonical_lf_golden(checked_out: &[u8]) -> Vec<u8> {
    let body = checked_out
        .strip_suffix(b"\r\n")
        .or_else(|| checked_out.strip_suffix(b"\n"))
        .expect("golden has one final LF or CRLF");
    assert!(!body.contains(&b'\r'));
    assert!(!body.contains(&b'\n'));
    let mut canonical = Vec::with_capacity(body.len() + 1);
    canonical.extend_from_slice(body);
    canonical.push(b'\n');
    canonical
}

fn snapshot_tree(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn collect(root: &Path, directory: &Path, snapshot: &mut Vec<(PathBuf, Vec<u8>)>) {
        let mut entries = fs::read_dir(directory)
            .expect("read directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect directory entries");
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).expect("read metadata");
            assert!(!metadata.file_type().is_symlink());
            if metadata.is_dir() {
                collect(root, &path, snapshot);
            } else {
                snapshot.push((
                    path.strip_prefix(root)
                        .expect("relative path")
                        .to_path_buf(),
                    fs::read(path).expect("read file"),
                ));
            }
        }
    }

    let mut snapshot = Vec::new();
    collect(root, root, &mut snapshot);
    snapshot
}
