use std::fs;
use std::path::Path;

use codenoesis_contracts::{
    CodeNoesisErrorV12, QueryContractError, local_query_result_v3,
};
use codenoesis_domain::s4_r5::{RustSemanticError, RustSemanticLimit};
use serde_json::{Value, json};

#[test]
fn conf_fr_ext_010_snapshot_v8_graph_v5_error_v12() {
    let snapshot_schema = specification("repository-snapshot-v8.schema.json");
    assert_eq!(
        snapshot_schema["properties"]["schema_version"]["const"],
        "codenoesis.repository-snapshot/v8"
    );
    assert_eq!(snapshot_schema["additionalProperties"], false);

    let graph_schema = specification("knowledge-graph-v5.schema.json");
    assert_eq!(
        graph_schema["properties"]["schema_version"]["const"],
        "codenoesis.knowledge-graph/v5"
    );
    assert_eq!(
        graph_schema["properties"]["rust_semantic_index"]["$ref"],
        "#/$defs/rust_semantic_index"
    );

    assert_eq!(
        CodeNoesisErrorV12::invalid_rust_semantic_profile("unknown")
            .canonical_stderr()
            .expect("serialize invalid R5 profile"),
        b"{\"code\":\"input.invalid_rust_semantic_profile\",\"context\":{\"provided_profile\":\"unknown\"},\"message\":\"invalid rust semantic profile\",\"retryable\":false,\"schema_version\":\"codenoesis.error/v12\",\"stage\":\"input\"}\n"
    );
    let limit = RustSemanticError::LimitExceeded {
        limit: RustSemanticLimit::FieldsPerOwner,
        maximum: 1_024,
        observed: 1_025,
    };
    let error: Value = serde_json::from_slice(
        &CodeNoesisErrorV12::from_semantic(&limit)
            .expect("map closed R5 semantic error")
            .canonical_stderr()
            .expect("serialize R5 semantic limit"),
    )
    .expect("parse ErrorV12");
    assert_eq!(error["code"], "extraction.rust_semantic_limit_exceeded");
    assert_eq!(error["context"]["limit"], "fields_per_owner");
    assert_eq!(error["context"]["maximum"], 1_024);
    assert_eq!(error["context"]["observed"], 1_025);
}

#[test]
fn conf_fr_qry_001_v8_uses_local_query_result_v3() {
    let entity_id = format!("urn:codenoesis:entity:blake3:{}", "1".repeat(64));
    let claim_id = format!("urn:codenoesis:claim:blake3:{}", "2".repeat(64));
    let evidence_id = format!("urn:codenoesis:evidence:blake3:{}", "3".repeat(64));
    let semantic = json!({
        "repository": {"identity": "urn:codenoesis:test:r5-contract"},
        "knowledge_graph": {
            "entities": [{"id": entity_id, "kind": "rust.field"}],
            "relationships": [],
            "claims": [{
                "id": claim_id,
                "subject_kind": "entity",
                "subject_id": entity_id,
                "state": "deterministic_fact",
                "evidence_ids": [evidence_id]
            }],
            "evidence": [{
                "id": evidence_id,
                "path": "src/lib.rs",
                "blob_oid": "1111111111111111111111111111111111111111",
                "start_byte": 0,
                "end_byte": 1
            }],
            "diagnostics": [],
            "coverage": []
        }
    });
    let snapshot_id = format!("urn:codenoesis:snapshot:blake3:{}", "a".repeat(64));
    let manifest = json!({
        "schema_version": "codenoesis.documentation-manifest/v1",
        "repository_identity": "urn:codenoesis:test:r5-contract",
        "snapshot_id": snapshot_id,
        "renderer_version": "codenoesis.renderer/markdown-v1",
        "documents": []
    });

    let result = local_query_result_v3(
        &semantic,
        &manifest,
        manifest["snapshot_id"].as_str().expect("snapshot ID"),
        &entity_id,
    )
    .expect("build strict entity query V3")
    .canonical_stdout()
    .expect("serialize strict entity query V3");
    let value: Value = serde_json::from_slice(&result).expect("parse query V3");
    assert_eq!(value["schema_version"], "codenoesis.local-query-result/v3");
    assert_eq!(value["result_kind"], "entity");
    assert_eq!(
        local_query_result_v3(
            &semantic,
            &manifest,
            manifest["snapshot_id"].as_str().expect("snapshot ID"),
            &format!("urn:codenoesis:entity:blake3:{}", "f".repeat(64)),
        )
        .expect_err("unknown exact ID must fail"),
        QueryContractError::NotFound
    );
}

fn specification(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/specifications/s4/r5")
        .join(name);
    serde_json::from_slice(&fs::read(path).expect("read R5 specification"))
        .expect("parse R5 specification")
}
