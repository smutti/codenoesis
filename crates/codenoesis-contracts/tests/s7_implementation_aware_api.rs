use std::fs;
use std::path::PathBuf;

use codenoesis_contracts::{
    ImpactWorkspaceError, parse_impact_workspace, parse_s7_federation_authority,
};
use serde_json::Value;

fn fixture(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

#[test]
fn conf_fr_imp_004_s6_authority_is_strictly_accepted() {
    let bytes = fs::read(fixture(
        "tests/fixtures/s6/openapi-federation-v1/expected-federation-report.json",
    ))
    .expect("read S6 report");
    let authority = parse_s7_federation_authority(&bytes).expect("validate S6 authority");
    assert_eq!(authority.operations.len(), 1);
    assert_eq!(authority.clients.len(), 4);
    assert_eq!(
        authority.operations[0].operation_id,
        "urn:codenoesis:operation:blake3:071cbb8fa33a959879d7d8a2bfbbac31e1fea4850c28fdb73227c605f5974923"
    );
}

#[test]
fn sec_fr_imp_004_s6_authority_rejects_unknown_fields_and_hash_tampering() {
    let bytes = fs::read(fixture(
        "tests/fixtures/s6/openapi-federation-v1/expected-federation-report.json",
    ))
    .expect("read S6 report");
    let mut value: Value = serde_json::from_slice(&bytes).expect("parse S6 report");
    value["operations"][0]["unexpected"] = Value::Bool(true);
    assert!(parse_s7_federation_authority(&serde_json::to_vec(&value).unwrap()).is_err());

    let mut value: Value = serde_json::from_slice(&bytes).expect("parse S6 report");
    value["semantic_hash"] = Value::String(format!("blake3:{}", "0".repeat(64)));
    assert!(parse_s7_federation_authority(&serde_json::to_vec(&value).unwrap()).is_err());
}

#[test]
fn pt_fr_cli_006_workspace_path_and_symbol_limits_are_typed() {
    let bytes = reviewed_workspace();
    let mut value: Value = serde_json::from_slice(&bytes).expect("parse workspace");
    value["provider"]["baseline"]["root"] = Value::String("a".repeat(4_097));
    assert_eq!(
        parse_impact_workspace(&serde_json::to_vec(&value).unwrap()),
        Err(ImpactWorkspaceError::LimitExceeded {
            limit: "logical_path_bytes",
            maximum: 4_096,
            observed: 4_097,
        })
    );

    let mut value: Value = serde_json::from_slice(&bytes).expect("parse workspace");
    value["provider"]["baseline"]["callable_symbol"] = Value::String("a".repeat(1_025));
    assert_eq!(
        parse_impact_workspace(&serde_json::to_vec(&value).unwrap()),
        Err(ImpactWorkspaceError::LimitExceeded {
            limit: "callable_symbol_bytes",
            maximum: 1_024,
            observed: 1_025,
        })
    );
}

fn reviewed_workspace() -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "schema_version": "codenoesis.impact-workspace/v1",
        "analysis_profile": "implementation-aware-http-json/v1",
        "pipeline": "codenoesis.pipeline/s7-v1",
        "contract_capability": "codenoesis.contract-capability/openapi-3.1-http-json/v1",
        "provider_capability": "rust-direct-json-map/v1",
        "client_capability": "kotlin-direct-json-access/v1",
        "provider": {
            "repository_identity": "urn:codenoesis:test:provider",
            "baseline": revision("a"),
            "target": revision("b")
        },
        "clients": [{
            "role": "strict",
            "repository_identity": "urn:codenoesis:test:client",
            "revision": "client-a",
            "root": "client",
            "source": file("client.kt"),
            "decoder_symbol": "decode",
            "call_symbol": "call"
        }],
        "federation_report": file("federation.json")
    }))
    .expect("serialize workspace")
}

fn revision(revision: &str) -> Value {
    serde_json::json!({
        "revision": revision,
        "root": revision,
        "contract": file("openapi.yaml"),
        "source": file("source.rs"),
        "callable_symbol": "selected"
    })
}

fn file(path: &str) -> Value {
    serde_json::json!({"path": path, "sha256": "0".repeat(64)})
}
