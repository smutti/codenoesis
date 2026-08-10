use codenoesis_contracts::{
    CodeNoesisErrorV18, PortableGraphV4, R11_ERROR_VERSION, R11_ONTOLOGY_VERSION,
    R11_PORTABLE_GRAPH_VERSION, R11_QUERY_VERSION, R11_SNAPSHOT_VERSION, R11ContractError,
};
use serde_json::{Value, json};

#[test]
fn ft_fr_exp_002_v4_reimport_rejects_unknown_schema_before_projection() {
    let value = portable_shell("codenoesis.portable-graph/v3");
    let bytes = canonical_file(&value);
    assert_eq!(
        PortableGraphV4::from_canonical_file(&bytes, sha256).unwrap_err(),
        R11ContractError::UnsupportedPortableGraphSchema("codenoesis.portable-graph/v3".to_owned())
    );
}

#[test]
fn ft_nfr_prv_002_v4_reimport_rejects_private_boundary_payloads() {
    let mut value = portable_shell(R11_PORTABLE_GRAPH_VERSION);
    value["repository"]["raw_url"] = Value::String("https://credential.invalid".to_owned());
    let bytes = canonical_file(&value);
    assert_eq!(
        PortableGraphV4::from_canonical_file(&bytes, sha256).unwrap_err(),
        R11ContractError::UnsafePayload {
            reason: "private_field"
        }
    );
}

#[test]
fn ft_fr_exp_002_v4_reimport_rejects_unknown_top_level_fields() {
    let mut value = portable_shell(R11_PORTABLE_GRAPH_VERSION);
    value["unknown"] = Value::Bool(true);
    let bytes = canonical_file(&value);
    assert_eq!(
        PortableGraphV4::from_canonical_file(&bytes, sha256).unwrap_err(),
        R11ContractError::InvalidProjection
    );
}

#[test]
fn conf_fr_cli_001_r11_contract_failures_emit_error_v18() {
    let error = CodeNoesisErrorV18::from_contract(&R11ContractError::InvalidProjection, false);
    assert_eq!(error.value()["schema_version"], R11_ERROR_VERSION);
    assert_eq!(error.value()["code"], "export.invalid_portable_graph_v4");
    assert!(
        error
            .canonical_stderr()
            .expect("serialize ErrorV18")
            .ends_with(b"\n")
    );
}

fn portable_shell(schema_version: &str) -> Value {
    json!({
        "claims": [],
        "coverage_gaps": [],
        "diagnostics": [],
        "document_statements": [],
        "documents": [],
        "entities": [],
        "evidence": [],
        "ontology_version": R11_ONTOLOGY_VERSION,
        "projection": {
            "profile": "codenoesis.lossless-portable-projection/v4",
            "family_sha256": {}
        },
        "query_contract_version": R11_QUERY_VERSION,
        "relationships": [],
        "repository": {},
        "repository_boundaries": {},
        "schema_version": schema_version,
        "source_snapshot": {
            "schema_version": R11_SNAPSHOT_VERSION
        }
    })
}

fn canonical_file(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(value).expect("serialize R11 portable test value");
    bytes.push(b'\n');
    bytes
}

fn sha256(bytes: &[u8]) -> String {
    let mut digest = [0_u8; 32];
    for (index, byte) in bytes.iter().copied().enumerate() {
        digest[index % digest.len()] ^= byte;
    }
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("write SHA-256 hex");
    }
    output
}
