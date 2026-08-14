use codenoesis_contracts::{
    CodeNoesisErrorV28, LocalReleaseCandidateVerificationV1, LocalReleaseContractError,
    parse_local_release_candidate_manifest_v1, validate_local_supply_chain_v1,
};
use serde_json::{Value, json};

const SPEC: &str = "../../tests/specifications/g8/local-release-candidate-v1";
const CARGO_LOCK_SHA256: &str = "434cc5e8e38a4c57f35990431d4682974b6cae94893860e1948c8f7cc21ffbca";
const SERDE_JSON_SHA256: &str = "c841b55ecdae098c80dcae9cf767f6f8a0c2cdb3416bbef72181df4d0fe73f14";

#[test]
fn ct_fr_rel_003_all_target_manifests_and_verifications_are_exact() {
    for target in [
        "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc",
        "x86_64-unknown-linux-gnu",
    ] {
        let manifest_bytes = fixture(&format!("expected-manifest-{target}.json"));
        let expected_verification = fixture(&format!("expected-verification-{target}.json"));
        let manifest = parse_local_release_candidate_manifest_v1(&manifest_bytes)
            .expect("parse exact target manifest");
        assert_eq!(manifest.canonical_stdout().unwrap(), manifest_bytes);

        let expected: Value =
            serde_json::from_slice(&expected_verification).expect("parse verification fixture");
        let verification = LocalReleaseCandidateVerificationV1::new(
            expected["candidate_name"].as_str().unwrap(),
            &manifest,
            expected["manifest_sha256"].as_str().unwrap(),
            expected["checksums_sha256"].as_str().unwrap(),
        )
        .expect("build target verification");
        assert_eq!(
            verification.canonical_stdout().unwrap(),
            expected_verification
        );
    }
}

#[test]
fn ct_fr_cli_010_error_v28_matches_frozen_public_bytes() {
    for (error, fixture_name) in [
        (
            CodeNoesisErrorV28::invalid_arguments(),
            "expected-error-invalid-arguments.json",
        ),
        (
            CodeNoesisErrorV28::invalid_evidence(),
            "expected-error-invalid-evidence.json",
        ),
        (
            CodeNoesisErrorV28::invalid_archive(),
            "expected-error-invalid-archive.json",
        ),
    ] {
        assert_eq!(
            error.canonical_stderr().unwrap(),
            fixture(fixture_name),
            "{fixture_name}"
        );
    }
}

#[test]
fn ct_nfr_sup_001_supply_documents_are_closed_and_cross_bound() {
    let target = "x86_64-unknown-linux-gnu";
    let documents = supply_documents(target, false);
    let inputs = supply_inputs(&documents);
    let validated = validate_local_supply_chain_v1(target, &inputs).expect("valid supply evidence");
    assert_eq!(validated.target(), target);
    assert_eq!(validated.cargo_lock_sha256(), CARGO_LOCK_SHA256);
    assert_eq!(validated.records().len(), 5);

    let vulnerable = supply_documents(target, true);
    let vulnerable_inputs = supply_inputs(&vulnerable);
    assert_eq!(
        validate_local_supply_chain_v1(target, &vulnerable_inputs).unwrap_err(),
        LocalReleaseContractError::PolicyRejected
    );
}

#[test]
fn sec_nfr_sec_004_noncanonical_and_private_evidence_fails_closed() {
    let target = "x86_64-unknown-linux-gnu";
    let mut documents = supply_documents(target, false);
    documents[0].1 = b"{\"schema_version\":\"codenoesis.local-advisory-report/v1\",\"private\":\"/Users/private\"}\n".to_vec();
    let inputs = supply_inputs(&documents);
    assert_eq!(
        validate_local_supply_chain_v1(target, &inputs).unwrap_err(),
        LocalReleaseContractError::InvalidEvidence
    );
}

fn fixture(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(SPEC)
        .join(name);
    let mut bytes = std::fs::read(path).expect("read G8 fixture");
    if bytes.ends_with(b"\r\n") {
        bytes.pop();
        bytes.pop();
        bytes.push(b'\n');
    }
    bytes
}

fn canonical(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(&value).expect("serialize supply fixture");
    bytes.push(b'\n');
    bytes
}

fn supply_inputs(documents: &[(String, Vec<u8>)]) -> Vec<(&str, &[u8], &str)> {
    documents
        .iter()
        .map(|(path, bytes)| {
            (
                path.as_str(),
                bytes.as_slice(),
                "0000000000000000000000000000000000000000000000000000000000000000",
            )
        })
        .collect()
}

fn supply_documents(target: &str, vulnerable: bool) -> Vec<(String, Vec<u8>)> {
    let vulnerabilities = if vulnerable {
        vec![json!({"id": "RUSTSEC-TEST"})]
    } else {
        Vec::new()
    };
    vec![
        (
            "evidence/advisory-report.json".to_owned(),
            canonical(&json!({
                "schema_version": "codenoesis.local-advisory-report/v1",
                "cargo_lock_sha256": CARGO_LOCK_SHA256,
                "tool": {"name": "cargo-audit", "version": "0.22.2"},
                "database": {
                    "commit": "69f93e1d081d8b6fbee010e48f0b5e0d13661415",
                    "updated": "2026-08-12T12:42:29+02:00"
                },
                "status": "accepted",
                "vulnerabilities": vulnerabilities,
                "warnings": []
            })),
        ),
        (
            "evidence/dependency-lock.json".to_owned(),
            canonical(&dependency_value(target)),
        ),
        (
            "evidence/license-report.json".to_owned(),
            canonical(&license_value(target)),
        ),
        (
            "evidence/sbom.cdx.json".to_owned(),
            canonical(&sbom_value(target)),
        ),
        (
            "evidence/unsafe-inventory.json".to_owned(),
            canonical(&unsafe_value(target)),
        ),
    ]
}

fn dependency_value(target: &str) -> Value {
    json!({
        "schema_version": "codenoesis.local-dependency-lock/v1",
        "target": target,
        "root": "noesis@0.1.0",
        "cargo_lock_sha256": CARGO_LOCK_SHA256,
        "packages": [
            {"id": "noesis@0.1.0", "name": "noesis", "version": "0.1.0", "source": "workspace", "checksum": null, "dependencies": ["serde_json@1.0.151"]},
            {"id": "serde_json@1.0.151", "name": "serde_json", "version": "1.0.151", "source": "registry+https://github.com/rust-lang/crates.io-index", "checksum": SERDE_JSON_SHA256, "dependencies": []}
        ],
        "dependency_edges": 1
    })
}

fn license_value(target: &str) -> Value {
    json!({
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
    })
}

fn unsafe_value(target: &str) -> Value {
    json!({
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
    })
}

fn sbom_value(target: &str) -> Value {
    json!({
        "bomFormat": "CycloneDX",
        "specVersion": "1.6",
        "serialNumber": "urn:uuid:00000000-0000-5000-8000-000000000000",
        "version": 1,
        "metadata": {
            "component": {"type": "application", "bom-ref": "pkg:cargo/noesis@0.1.0", "name": "noesis", "version": "0.1.0"},
            "properties": [
                {"name": "codenoesis:cargo-lock-sha256", "value": CARGO_LOCK_SHA256},
                {"name": "codenoesis:target", "value": target}
            ]
        },
        "components": [
            {"type": "library", "bom-ref": "pkg:cargo/serde_json@1.0.151", "name": "serde_json", "version": "1.0.151", "hashes": [{"alg": "SHA-256", "content": SERDE_JSON_SHA256}], "licenses": [{"expression": "MIT OR Apache-2.0"}], "purl": "pkg:cargo/serde_json@1.0.151"}
        ],
        "dependencies": [
            {"ref": "pkg:cargo/noesis@0.1.0", "dependsOn": ["pkg:cargo/serde_json@1.0.151"]},
            {"ref": "pkg:cargo/serde_json@1.0.151", "dependsOn": []}
        ]
    })
}
