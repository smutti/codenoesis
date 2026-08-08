use std::fs;
use std::path::Path;

use codenoesis_contracts::{CodeNoesisErrorV13, QueryContractError, local_query_result_v4};
use codenoesis_domain::s4_r6::{FrameworkError, FrameworkLimit};
use serde_json::{Value, json};

#[test]
fn conf_fr_ext_011_snapshot_v9_graph_v6_error_v13() {
    let snapshot_schema = specification("repository-snapshot-v9.schema.json");
    assert_eq!(
        snapshot_schema["properties"]["schema_version"]["const"],
        "codenoesis.repository-snapshot/v9"
    );
    assert_eq!(snapshot_schema["additionalProperties"], false);

    let graph_schema = specification("knowledge-graph-v6.schema.json");
    assert_eq!(
        graph_schema["properties"]["schema_version"]["const"],
        "codenoesis.knowledge-graph/v6"
    );
    assert_eq!(
        graph_schema["properties"]["framework_declaration_index"]["$ref"],
        "#/$defs/framework_declaration_index"
    );

    assert_eq!(
        CodeNoesisErrorV13::invalid_rust_framework_profile("unknown")
            .canonical_stderr()
            .expect("serialize invalid R6 profile"),
        b"{\"code\":\"input.invalid_rust_framework_profile\",\"context\":{\"profile\":\"unknown\"},\"message\":\"invalid rust framework profile\",\"retryable\":false,\"schema_version\":\"codenoesis.error/v13\",\"stage\":\"input\"}\n"
    );
    let limit = FrameworkError::LimitExceeded {
        limit: FrameworkLimit::ExplicitRegistrationChainSegments,
        maximum: 256,
        observed: 257,
    };
    let error: Value = serde_json::from_slice(
        &CodeNoesisErrorV13::from_framework(&limit)
            .expect("map closed R6 framework error")
            .canonical_stderr()
            .expect("serialize R6 framework limit"),
    )
    .expect("parse ErrorV13");
    assert_eq!(
        error["code"],
        "extraction.framework_declaration_limit_exceeded"
    );
    assert_eq!(
        error["context"]["limit"],
        "explicit_registration_chain_segments"
    );
    assert_eq!(error["context"]["maximum"], 256);
    assert_eq!(error["context"]["observed"], 257);
}

#[test]
fn conf_fr_ext_011_error_v13_invalid_matrix_is_strict() {
    let strict_cases = [
        (
            FrameworkError::InvalidDeclaration {
                path: "src/lib.rs".to_owned(),
                reason: "malformed_reviewed_builder_method".to_owned(),
            },
            "extraction.invalid_framework_declaration",
        ),
        (
            FrameworkError::IdentityConflict {
                normalized_preimage_sha256: "a".repeat(64),
            },
            "extraction.framework_declaration_identity_conflict",
        ),
        (
            FrameworkError::UnsupportedComposition {
                required_profiles: Vec::new(),
                selected_profiles: vec!["rust-framework-declarations-v1".to_owned()],
            },
            "extraction.unsupported_framework_composition",
        ),
        (
            FrameworkError::AmbiguousTarget {
                target_spelling: "handler".to_owned(),
                candidate_count: 2,
            },
            "extraction.ambiguous_framework_target",
        ),
        (
            FrameworkError::UnresolvableEvidence {
                evidence_id: format!("urn:codenoesis:evidence:sha256:{}", "b".repeat(64)),
            },
            "extraction.unresolvable_framework_evidence",
        ),
        (
            FrameworkError::UnsafePath {
                path: "/private/repository.rs".to_owned(),
                reason: "absolute_path".to_owned(),
            },
            "input.unsafe_framework_path",
        ),
    ];
    for (framework_error, expected_code) in strict_cases {
        let error: Value = serde_json::from_slice(
            &CodeNoesisErrorV13::from_framework(&framework_error)
                .expect("map reviewed R6 failure")
                .canonical_stderr()
                .expect("serialize reviewed R6 failure"),
        )
        .expect("parse reviewed ErrorV13");
        assert_eq!(error["schema_version"], "codenoesis.error/v13");
        assert_eq!(error["code"], expected_code);
        assert_eq!(error["retryable"], false);
        if expected_code == "input.unsafe_framework_path" {
            assert_eq!(error["context"]["path"], "repository-path");
        }
    }
    let missing_profile: Value = serde_json::from_slice(
        &CodeNoesisErrorV13::invalid_rust_framework_profile("")
            .canonical_stderr()
            .expect("serialize missing R6 profile"),
    )
    .expect("parse missing-profile ErrorV13");
    assert_eq!(missing_profile["context"]["profile"], "missing");
    let invalid_identity = FrameworkError::IdentityConflict {
        normalized_preimage_sha256: "not-a-digest".to_owned(),
    };
    let internal: Value = serde_json::from_slice(
        &CodeNoesisErrorV13::from_framework(&invalid_identity)
            .expect("map invalid internal identity context")
            .canonical_stderr()
            .expect("serialize internal ErrorV13"),
    )
    .expect("parse internal ErrorV13");
    assert_eq!(internal["code"], "internal.unexpected");
    assert_eq!(internal["context"], json!({}));
}

#[test]
fn conf_fr_qry_001_v9_uses_local_query_result_v4() {
    let entity_id = format!("urn:codenoesis:entity:blake3:{}", "1".repeat(64));
    let claim_id = format!("urn:codenoesis:claim:blake3:{}", "2".repeat(64));
    let evidence_id = format!("urn:codenoesis:evidence:sha256:{}", "3".repeat(64));
    let semantic = json!({
        "repository": {"identity": "urn:codenoesis:test:r6-contract"},
        "knowledge_graph": {
            "schema_version": "codenoesis.knowledge-graph/v6",
            "entities": [{"id": entity_id, "kind": "framework.route_declaration"}],
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
        "repository_identity": "urn:codenoesis:test:r6-contract",
        "snapshot_id": snapshot_id,
        "renderer_version": "codenoesis.renderer/markdown-v1",
        "documents": []
    });

    let result = local_query_result_v4(
        &semantic,
        &manifest,
        manifest["snapshot_id"].as_str().expect("snapshot ID"),
        &evidence_id,
    )
    .expect("build strict SHA-256 evidence query V4")
    .canonical_stdout()
    .expect("serialize strict evidence query V4");
    let value: Value = serde_json::from_slice(&result).expect("parse query V4");
    assert_eq!(value["schema_version"], "codenoesis.local-query-result/v4");
    assert_eq!(value["result_kind"], "evidence");
    assert_eq!(value["evidence"][0]["id"], evidence_id);
    assert_eq!(
        local_query_result_v4(
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
        .join("../../tests/specifications/s4/r6")
        .join(name);
    serde_json::from_slice(&fs::read(path).expect("read R6 specification"))
        .expect("parse R6 specification")
}
