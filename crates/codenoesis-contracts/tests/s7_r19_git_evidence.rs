use std::collections::BTreeSet;

use codenoesis_contracts::{
    CodeNoesisErrorV30, ImpactGitWorkspaceError, ImpactSourceError, ImpactSourceSelectionV1,
    MAX_R19_CLIENTS, MAX_R19_EXCERPT_BYTES, MAX_R19_EXCERPT_STDOUT_BYTES, MAX_R19_FEDERATION_BYTES,
    MAX_R19_PATH_BYTES, MAX_R19_REPORT_BYTES, MAX_R19_SOURCE_BYTES_PER_FILE, MAX_R19_SYMBOL_BYTES,
    MAX_R19_TOTAL_SOURCE_BYTES, MAX_R19_WORKSPACE_BYTES, R19_ANALYSIS_PROFILE, R19_ERROR_VERSION,
    R19_EVIDENCE_LINEAGE_VERSION, R19_PIPELINE_VERSION, R19_REPORT_VERSION, R19_SOURCE_PROFILE,
    R19_SOURCE_RESULT_VERSION, R19_WORKSPACE_VERSION, TrustedImpactSourceExcerptV1,
    parse_impact_git_workspace,
};
use codenoesis_domain::{
    AcquiredFile, AcquiredRepository, BoundRevision, ObjectId, RegularFileMode, RepositoryIdentity,
    RepositoryInventory,
};
use serde_json::{Value, json};

const EVIDENCE_ID: &str = "urn:codenoesis:evidence:blake3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const REPOSITORY_ID: &str = "urn:codenoesis:test:r19-contract";
const COMMIT_OID: &str = "1111111111111111111111111111111111111111";
const TREE_OID: &str = "2222222222222222222222222222222222222222";
const BLOB_OID: &str = "3333333333333333333333333333333333333333";
const SOURCE_PATH: &str = "src/lib.rs";

#[test]
fn ct_fr_cli_012_r19_contract_constants_are_exact() {
    assert_eq!(R19_WORKSPACE_VERSION, "codenoesis.impact-git-workspace/v1");
    assert_eq!(
        R19_REPORT_VERSION,
        "codenoesis.semantic-compatibility-report/v2"
    );
    assert_eq!(R19_PIPELINE_VERSION, "codenoesis.pipeline/s7-git-v1");
    assert_eq!(
        R19_EVIDENCE_LINEAGE_VERSION,
        "codenoesis.source-evidence/git-v1"
    );
    assert_eq!(
        R19_ANALYSIS_PROFILE,
        "implementation-aware-http-json-git-v1"
    );
    assert_eq!(R19_SOURCE_PROFILE, "trusted-local-impact-source-v1");
    assert_eq!(
        R19_SOURCE_RESULT_VERSION,
        "codenoesis.trusted-impact-source-excerpt/v1"
    );
    assert_eq!(R19_ERROR_VERSION, "codenoesis.error/v30");
    assert_eq!(MAX_R19_WORKSPACE_BYTES, 1_048_576);
    assert_eq!(MAX_R19_FEDERATION_BYTES, 67_108_864);
    assert_eq!(MAX_R19_REPORT_BYTES, 67_108_864);
    assert_eq!(MAX_R19_SOURCE_BYTES_PER_FILE, 2_097_152);
    assert_eq!(MAX_R19_TOTAL_SOURCE_BYTES, 268_435_456);
    assert_eq!(MAX_R19_PATH_BYTES, 1_024);
    assert_eq!(MAX_R19_SYMBOL_BYTES, 1_024);
    assert_eq!(MAX_R19_CLIENTS, 32);
    assert_eq!(MAX_R19_EXCERPT_BYTES, 262_144);
    assert_eq!(MAX_R19_EXCERPT_STDOUT_BYTES, 524_288);
}

#[test]
fn ct_fr_imp_006_workspace_boundaries_have_maximum_and_plus_one() {
    let maximum_path = "p".repeat(MAX_R19_PATH_BYTES);
    let maximum_symbol = "s".repeat(MAX_R19_SYMBOL_BYTES);
    let maximum = workspace_value(MAX_R19_CLIENTS, &maximum_path, &maximum_symbol);
    let parsed = parse_impact_git_workspace(&serde_json::to_vec(&maximum).unwrap())
        .expect("R19 exact workspace boundaries");
    assert_eq!(parsed.clients.len(), MAX_R19_CLIENTS);
    assert_eq!(parsed.provider.baseline.contract_path, maximum_path);
    assert_eq!(parsed.provider.baseline.callable_symbol, maximum_symbol);

    let too_many = workspace_value(MAX_R19_CLIENTS + 1, "openapi.yaml", "callable");
    assert_eq!(
        parse_impact_git_workspace(&serde_json::to_vec(&too_many).unwrap()),
        Err(ImpactGitWorkspaceError::TooManyClients { observed: 33 })
    );

    let path_plus_one = workspace_value(1, &"p".repeat(MAX_R19_PATH_BYTES + 1), "callable");
    assert_eq!(
        parse_impact_git_workspace(&serde_json::to_vec(&path_plus_one).unwrap()),
        Err(ImpactGitWorkspaceError::Invalid)
    );

    let symbol_plus_one = workspace_value(1, "openapi.yaml", &"s".repeat(MAX_R19_SYMBOL_BYTES + 1));
    assert_eq!(
        parse_impact_git_workspace(&serde_json::to_vec(&symbol_plus_one).unwrap()),
        Err(ImpactGitWorkspaceError::Invalid)
    );
}

#[test]
fn ct_fr_imp_006_workspace_rejects_ambiguous_or_non_commit_authority() {
    let mut workspace = workspace_value(2, "openapi.yaml", "callable");
    workspace["clients"][1]["repository_identity"] =
        workspace["clients"][0]["repository_identity"].clone();
    assert_eq!(
        parse_impact_git_workspace(&serde_json::to_vec(&workspace).unwrap()),
        Err(ImpactGitWorkspaceError::Invalid)
    );

    let mut workspace = workspace_value(1, "openapi.yaml", "callable");
    workspace["provider"]["baseline"]["revision"] = Value::String("A".repeat(40));
    assert_eq!(
        parse_impact_git_workspace(&serde_json::to_vec(&workspace).unwrap()),
        Err(ImpactGitWorkspaceError::Invalid)
    );
}

#[test]
fn pt_fr_cli_012_excerpt_limits_have_maximum_and_plus_one() {
    let maximum = vec![b'a'; usize::try_from(MAX_R19_EXCERPT_BYTES).unwrap()];
    let excerpt = retrieve(&maximum, 0, MAX_R19_EXCERPT_BYTES)
        .expect("R19 maximum excerpt remains representable");
    assert_eq!(
        excerpt.value()["excerpt"]["byte_length"],
        MAX_R19_EXCERPT_BYTES
    );

    let plus_one = vec![b'a'; usize::try_from(MAX_R19_EXCERPT_BYTES + 1).unwrap()];
    assert_eq!(
        retrieve(&plus_one, 0, MAX_R19_EXCERPT_BYTES + 1).unwrap_err(),
        ImpactSourceError::LimitExceeded
    );

    let escaped = vec![1_u8; 100_000];
    assert_eq!(
        retrieve(&escaped, 0, 100_000).unwrap_err(),
        ImpactSourceError::LimitExceeded
    );
}

#[test]
fn ct_fr_cli_012_source_binding_rejects_digest_utf8_and_scalar_mismatch() {
    let source = b"reviewed source";
    let mut report = report_value(source, 0, u64::try_from(source.len()).unwrap());
    report["evidence"][0]["excerpt_sha256"] = Value::String("0".repeat(64));
    let selected = selection(&report);
    assert_eq!(
        TrustedImpactSourceExcerptV1::from_inventory(
            &selected,
            &inventory(source.to_vec()),
            fixture_sha256,
        )
        .unwrap_err(),
        ImpactSourceError::ContentRejected
    );

    let malformed = [0xff_u8];
    let selected = selection(&report_value(&malformed, 0, 1));
    assert_eq!(
        TrustedImpactSourceExcerptV1::from_inventory(
            &selected,
            &inventory(malformed.to_vec()),
            fixture_sha256,
        )
        .unwrap_err(),
        ImpactSourceError::ContentRejected
    );

    let unicode = "é".as_bytes();
    let selected = selection(&report_value(unicode, 0, 1));
    assert_eq!(
        TrustedImpactSourceExcerptV1::from_inventory(
            &selected,
            &inventory(unicode.to_vec()),
            fixture_sha256,
        )
        .unwrap_err(),
        ImpactSourceError::ContentRejected
    );
}

#[test]
fn ct_fr_cli_012_error_v30_is_strict_private_and_lf_terminated() {
    let errors = [
        CodeNoesisErrorV30::impact_invalid_workspace(),
        CodeNoesisErrorV30::impact_invalid_federation_report(),
        CodeNoesisErrorV30::impact_acquisition_rejected(),
        CodeNoesisErrorV30::impact_source_rejected(),
        CodeNoesisErrorV30::impact_limit_exceeded(),
        CodeNoesisErrorV30::impact_unstable_input(),
        CodeNoesisErrorV30::source_invalid_arguments(),
        CodeNoesisErrorV30::source_invalid_report(),
        CodeNoesisErrorV30::source_acquisition_rejected(),
        CodeNoesisErrorV30::source_unstable_input(),
        CodeNoesisErrorV30::source_internal(),
    ];
    for error in errors {
        let bytes = error.canonical_stderr().expect("canonical ErrorV30");
        assert!(bytes.ends_with(b"\n"));
        assert!(!bytes[..bytes.len() - 1].contains(&b'\n'));
        let value: Value = serde_json::from_slice(&bytes).expect("strict ErrorV30 JSON");
        let keys = value
            .as_object()
            .expect("ErrorV30 object")
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            keys,
            [
                "code",
                "context",
                "message",
                "retryable",
                "schema_version",
                "stage"
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(value["schema_version"], R19_ERROR_VERSION);
        assert_eq!(value["retryable"], false);
        assert_eq!(value["context"], json!({}));
        let serialized = serde_json::to_string(&value).unwrap();
        assert!(!serialized.contains("PRIVATE"));
        assert!(!serialized.contains("/Users/"));
    }
}

fn workspace_value(client_count: usize, contract_path: &str, symbol: &str) -> Value {
    let clients = (0..client_count)
        .map(|index| {
            json!({
                "role": format!("client-{index}"),
                "repository_identity": format!("urn:codenoesis:test:r19-client-{index}"),
                "root": format!("client-{index}"),
                "revision": format!("{index:040x}"),
                "federation_revision": "federation-client",
                "source_path": "src/Client.kt",
                "decoder_symbol": "decode",
                "call_symbol": "call"
            })
        })
        .collect::<Vec<_>>();
    json!({
        "schema_version": R19_WORKSPACE_VERSION,
        "analysis_profile": R19_ANALYSIS_PROFILE,
        "pipeline": R19_PIPELINE_VERSION,
        "contract_capability": "codenoesis.contract-capability/openapi-3.1-http-json/v1",
        "provider_capability": "rust-direct-json-map/v1",
        "client_capability": "kotlin-direct-json-access/v1",
        "provider": {
            "repository_identity": "urn:codenoesis:test:r19-provider",
            "root": "provider",
            "baseline": {
                "revision": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "federation_revision": "provider-a",
                "contract_path": contract_path,
                "source_path": "src/lib.rs",
                "callable_symbol": symbol
            },
            "target": {
                "revision": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "federation_revision": "provider-b",
                "contract_path": contract_path,
                "source_path": "src/lib.rs",
                "callable_symbol": symbol
            }
        },
        "clients": clients,
        "federation_report": {
            "path": "federation-report.json",
            "sha256": "c".repeat(64)
        }
    })
}

fn retrieve(
    source: &[u8],
    start: u64,
    end: u64,
) -> Result<TrustedImpactSourceExcerptV1, ImpactSourceError> {
    let selection = selection(&report_value(source, start, end));
    TrustedImpactSourceExcerptV1::from_inventory(
        &selection,
        &inventory(source.to_vec()),
        fixture_sha256,
    )
}

fn selection(report: &Value) -> ImpactSourceSelectionV1 {
    let mut bytes = serde_json::to_vec(report).expect("serialize R19 test report");
    bytes.push(b'\n');
    ImpactSourceSelectionV1::from_report(
        &bytes,
        EVIDENCE_ID,
        REPOSITORY_ID,
        COMMIT_OID,
        fixture_sha256,
    )
    .expect("select R19 test evidence")
}

fn report_value(source: &[u8], start: u64, end: u64) -> Value {
    json!({
        "schema_version": R19_REPORT_VERSION,
        "analysis_profile": R19_ANALYSIS_PROFILE,
        "pipeline_version": R19_PIPELINE_VERSION,
        "evidence_lineage_version": R19_EVIDENCE_LINEAGE_VERSION,
        "configuration_hash": format!("blake3:{}", "d".repeat(64)),
        "provider": {},
        "semantic_diffs": [],
        "client_assessments": [],
        "rejected_candidates": [],
        "coverage_gaps": [],
        "evidence": [{
            "id": EVIDENCE_ID,
            "repository_identity": REPOSITORY_ID,
            "revision": COMMIT_OID,
            "path": SOURCE_PATH,
            "start_line": 1,
            "end_line": 1,
            "excerpt_sha256": fixture_sha256(source.get(usize::try_from(start).unwrap_or(usize::MAX)..usize::try_from(end).unwrap_or(usize::MAX)).unwrap_or_default()),
            "source_kind": "provider_implementation",
            "claim_state": "derived_fact",
            "capability_version": "rust-direct-json-map/v1",
            "source_binding": {
                "commit_oid": COMMIT_OID,
                "tree_oid": TREE_OID,
                "blob_oid": BLOB_OID,
                "span": {
                    "unit": "byte",
                    "start": start,
                    "end": end,
                    "start_position": {
                        "line": 1,
                        "column": start + 1,
                        "unit": "unicode_scalar"
                    },
                    "end_position": {
                        "line": 1,
                        "column": end + 1,
                        "unit": "unicode_scalar"
                    }
                }
            }
        }],
        "extractor_versions": [],
        "ontology_versions": [],
        "rule_catalog_version": "implementation-aware-impact-rules/v1"
    })
}

fn inventory(source: Vec<u8>) -> RepositoryInventory {
    RepositoryInventory::classify(AcquiredRepository::new(
        BoundRevision::new(
            RepositoryIdentity::parse(REPOSITORY_ID).expect("R19 test identity"),
            oid(COMMIT_OID),
            oid(TREE_OID),
        ),
        1,
        vec![AcquiredFile::new(
            SOURCE_PATH.to_owned(),
            RegularFileMode::Regular,
            oid(BLOB_OID),
            source,
        )],
    ))
}

fn oid(value: &str) -> ObjectId {
    ObjectId::parse_sha1(value).expect("R19 synthetic object ID")
}

fn fixture_sha256(bytes: &[u8]) -> String {
    format!("{:064x}", bytes.len())
}
