use std::fs;
use std::path::Path;

use codenoesis_contracts::{CodeNoesisErrorV14, QueryContractError, local_query_result_v5};
use codenoesis_domain::s4_r7::{
    CompilerIndexError, CompilerIndexLimit, compiler_index_limit_exceeded,
};
use serde_json::{Value, json};

#[test]
fn conf_fr_ext_005_snapshot_v10_graph_v7_error_v14() {
    for (schema, expected_version) in [
        (
            "repository-snapshot-v10.schema.json",
            "codenoesis.repository-snapshot/v10",
        ),
        (
            "configuration-v7.schema.json",
            "codenoesis.configuration/v7",
        ),
        (
            "extraction-chunk-v7.schema.json",
            "codenoesis.extraction-chunk/v7",
        ),
        (
            "knowledge-graph-v7.schema.json",
            "codenoesis.knowledge-graph/v7",
        ),
        (
            "local-query-result-v5.schema.json",
            "codenoesis.local-query-result/v5",
        ),
    ] {
        let value = specification(schema);
        assert_eq!(
            value["properties"]["schema_version"]["const"],
            expected_version
        );
        assert_eq!(value["additionalProperties"], false);
    }

    assert_eq!(
        CodeNoesisErrorV14::invalid_compiler_index_profile("unsupported")
            .canonical_stderr()
            .expect("serialize invalid R7 profile"),
        b"{\"code\":\"input.invalid_compiler_index_profile\",\"context\":{\"profile\":\"unsupported\"},\"message\":\"invalid compiler index profile\",\"retryable\":false,\"schema_version\":\"codenoesis.error/v14\",\"stage\":\"input\"}\n"
    );
    let limit = compiler_index_limit_exceeded(
        CompilerIndexLimit::RawIndexBytes,
        CompilerIndexLimit::RawIndexBytes.maximum() + 1,
    );
    let error = parse_error(&CodeNoesisErrorV14::from_compiler_index(&limit));
    assert_eq!(error["code"], "extraction.compiler_index_limit_exceeded");
    assert_eq!(error["context"]["limit"], "raw_index_bytes");
    assert_eq!(error["context"]["maximum"], 67_108_864);
    assert_eq!(error["context"]["observed"], 67_108_865);

    let internal = parse_error(&CodeNoesisErrorV14::from_compiler_index(
        &CompilerIndexError::ContractInvalid,
    ));
    assert_eq!(internal["schema_version"], "codenoesis.error/v14");
    assert_eq!(internal["code"], "internal.unexpected");
    assert_eq!(internal["context"], json!({}));
    assert_eq!(internal["retryable"], false);
}

#[test]
#[allow(clippy::too_many_lines)]
fn conf_fr_qry_001_v10_uses_local_query_result_v5() {
    let entity_id = id("entity", '1');
    let relationship_id = id("relationship", '2');
    let entity_claim_id = id("claim", '3');
    let relationship_claim_id = id("claim", '4');
    let evidence_id = evidence_id('5');
    let diagnostic_id = id("diagnostic", '6');
    let coverage_id = id("coverage-gap", '7');
    let document_id = id("document", '8');
    let snapshot_id = id("snapshot", '9');
    let repository_identity = "urn:codenoesis:test:r7-contract";
    let semantic = json!({
        "repository": {"identity": repository_identity},
        "knowledge_graph": {
            "schema_version": "codenoesis.knowledge-graph/v7",
            "entities": [{
                "id": entity_id,
                "kind": "compiler.symbol",
                "binding_state": "in_repository_bound"
            }],
            "relationships": [{
                "id": relationship_id,
                "kind": "REFERENCES",
                "source": entity_id,
                "target": entity_id,
                "evidence_ids": [evidence_id]
            }],
            "claims": [
                {
                    "id": entity_claim_id,
                    "subject_kind": "entity",
                    "subject_id": entity_id,
                    "state": "deterministic_fact",
                    "evidence_ids": [evidence_id]
                },
                {
                    "id": relationship_claim_id,
                    "subject_kind": "relationship",
                    "subject_id": relationship_id,
                    "state": "deterministic_fact",
                    "evidence_ids": [evidence_id]
                }
            ],
            "evidence": [{
                "id": evidence_id,
                "artifact_sha256": "a".repeat(64),
                "record_kind": "occurrence_reference"
            }],
            "diagnostics": [{
                "id": diagnostic_id,
                "code": "compiler_index.syntax_uncertainty_retained",
                "subject_id": entity_id,
                "compiler_target_id": entity_id,
                "evidence_ids": [evidence_id]
            }],
            "coverage": [{
                "id": coverage_id,
                "subject": "artifact",
                "capability": "compiler_index.arguments_redacted",
                "state": "redacted",
                "evidence_ids": []
            }]
        }
    });
    let manifest = json!({
        "schema_version": "codenoesis.documentation-manifest/v1",
        "repository_identity": repository_identity,
        "snapshot_id": snapshot_id,
        "renderer_version": "codenoesis.renderer/markdown-v1",
        "documents": [{
            "document_id": document_id,
            "kind": "overview",
            "subject_id": repository_identity,
            "path": "overview.md",
            "byte_length": 1,
            "blake3": "b".repeat(64),
            "statements": []
        }]
    });

    for (kind, requested_id) in [
        ("entity", entity_id.as_str()),
        ("relationship", relationship_id.as_str()),
        ("claim", entity_claim_id.as_str()),
        ("evidence", evidence_id.as_str()),
        ("diagnostic", diagnostic_id.as_str()),
        ("coverage_gap", coverage_id.as_str()),
        ("document", document_id.as_str()),
    ] {
        let bytes = local_query_result_v5(
            &semantic,
            &manifest,
            manifest["snapshot_id"].as_str().expect("snapshot ID"),
            requested_id,
        )
        .unwrap_or_else(|error| panic!("build R7 {kind} query: {error}"))
        .canonical_stdout()
        .unwrap_or_else(|error| panic!("serialize R7 {kind} query: {error}"));
        let value: Value = serde_json::from_slice(&bytes).expect("parse LocalQueryResultV5");
        assert_eq!(value["schema_version"], "codenoesis.local-query-result/v5");
        assert_eq!(value["requested_id"], requested_id);
        assert_eq!(value["result_kind"], kind);
    }

    assert_eq!(
        local_query_result_v5(
            &semantic,
            &manifest,
            manifest["snapshot_id"].as_str().expect("snapshot ID"),
            &id("entity", 'f'),
        )
        .expect_err("unknown exact R7 ID must fail"),
        QueryContractError::NotFound
    );
}

fn specification(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/specifications/s4/r7")
        .join(name);
    serde_json::from_slice(&fs::read(path).expect("read R7 specification"))
        .expect("parse R7 specification")
}

fn parse_error(error: &CodeNoesisErrorV14) -> Value {
    serde_json::from_slice(
        &error
            .canonical_stderr()
            .expect("serialize strict CodeNoesisErrorV14"),
    )
    .expect("parse strict CodeNoesisErrorV14")
}

fn id(kind: &str, digit: char) -> String {
    format!(
        "urn:codenoesis:{kind}:blake3:{}",
        digit.to_string().repeat(64)
    )
}

fn evidence_id(digit: char) -> String {
    format!(
        "urn:codenoesis:evidence:sha256:{}",
        digit.to_string().repeat(64)
    )
}
