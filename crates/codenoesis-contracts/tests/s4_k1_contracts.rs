use codenoesis_contracts::{
    CodeNoesisErrorV16, K1_ERROR_VERSION, K1_EXPLORER_SECURITY_PROFILE, K1_GRAPH_VERSION,
    K1_LOCAL_EXPLORER_VERSION, K1_ONTOLOGY_VERSION, K1_PORTABLE_GRAPH_VERSION, K1_QUERY_VERSION,
    K1_SNAPSHOT_VERSION, K1ContractError, LocalExplorerManifestV2, PortableGraphV2,
    local_query_result_v6,
};
use codenoesis_domain::storage::{
    EXTRACTION_HASH_DOMAIN_V8, GRAPH_HASH_DOMAIN_V8, SNAPSHOT_HASH_DOMAIN_V11,
    SNAPSHOT_SCHEMA_VERSION_V11, extraction_hash_domain, graph_hash_domain, snapshot_hash_domain,
};
use serde_json::{Map, Value, json};

#[test]
fn conf_fr_ext_012_snapshot_v11_graph_v8_error_v16() {
    assert_eq!(K1_SNAPSHOT_VERSION, "codenoesis.repository-snapshot/v11");
    assert_eq!(K1_GRAPH_VERSION, "codenoesis.knowledge-graph/v8");
    assert_eq!(K1_ERROR_VERSION, "codenoesis.error/v16");
    assert_eq!(SNAPSHOT_SCHEMA_VERSION_V11, K1_SNAPSHOT_VERSION);
    assert_eq!(
        snapshot_hash_domain(K1_SNAPSHOT_VERSION),
        Some(SNAPSHOT_HASH_DOMAIN_V11)
    );
    assert_eq!(
        graph_hash_domain(K1_SNAPSHOT_VERSION),
        Some(GRAPH_HASH_DOMAIN_V8)
    );
    assert_eq!(
        extraction_hash_domain(K1_SNAPSHOT_VERSION),
        Some(EXTRACTION_HASH_DOMAIN_V8)
    );
    let error: Value = serde_json::from_slice(
        &CodeNoesisErrorV16::invalid_rust_callable_profile("unknown")
            .canonical_stderr()
            .expect("serialize ErrorV16"),
    )
    .expect("parse ErrorV16");
    assert_eq!(error["schema_version"], K1_ERROR_VERSION);
    assert_eq!(error["code"], "input.invalid_rust_callable_profile");
    assert_eq!(error["retryable"], false);
}

#[test]
fn conf_fr_qry_001_v11_uses_local_query_result_v6() {
    let callable_id = entity_id('1');
    let signature_id = entity_id('2');
    let relationship_id = relationship_id('3');
    let callable_claim_id = claim_id('4');
    let signature_claim_id = claim_id('5');
    let relationship_claim_id = claim_id('6');
    let evidence_id = evidence_id('7');
    let semantic = json!({
        "repository": {"identity": "urn:codenoesis:test:k1-query"},
        "knowledge_graph": {
            "schema_version": K1_GRAPH_VERSION,
            "entities": [
                {"id": callable_id, "kind": "rust.function", "name": "callable"},
                {
                    "id": signature_id,
                    "kind": "rust.callable_signature",
                    "name": "callable",
                    "evidence_ids": [evidence_id]
                }
            ],
            "relationships": [{
                "id": relationship_id,
                "kind": "HAS_SIGNATURE",
                "source": callable_id,
                "target": signature_id,
                "evidence_ids": [evidence_id]
            }],
            "claims": [
                {
                    "id": callable_claim_id,
                    "subject_kind": "entity",
                    "subject_id": callable_id,
                    "state": "deterministic_fact",
                    "evidence_ids": [evidence_id]
                },
                {
                    "id": signature_claim_id,
                    "subject_kind": "entity",
                    "subject_id": signature_id,
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
                "path": "src/lib.rs",
                "blob_oid": "8".repeat(40),
                "start_byte": 0,
                "end_byte": 8
            }],
            "diagnostics": [],
            "coverage": []
        }
    });
    let snapshot_id = format!("urn:codenoesis:snapshot:blake3:{}", "9".repeat(64));
    let manifest = json!({
        "schema_version": "codenoesis.documentation-manifest/v1",
        "repository_identity": "urn:codenoesis:test:k1-query",
        "snapshot_id": snapshot_id,
        "renderer_version": "codenoesis.renderer/markdown-v1",
        "documents": []
    });
    let result = local_query_result_v6(
        &semantic,
        &manifest,
        manifest["snapshot_id"].as_str().expect("snapshot ID"),
        &callable_id,
    )
    .expect("build query V6");
    assert_eq!(result.value()["schema_version"], K1_QUERY_VERSION);
    assert_eq!(result.value()["linked_k1_entities"][0]["id"], signature_id);
    assert_eq!(
        result.value()["linked_k1_relationships"][0]["id"],
        relationship_id
    );
}

#[test]
fn conf_fr_exp_002_portable_graph_v2_lossless_reimport() {
    let value = portable_value();
    let mut bytes = serde_json::to_vec(&value).expect("serialize portable V2");
    bytes.push(b'\n');
    let portable = PortableGraphV2::from_canonical_file(&bytes, digest)
        .expect("strictly reimport portable V2");
    assert_eq!(portable.value(), &value);
    assert_eq!(portable.canonical_file(), bytes);
}

#[test]
fn conf_fr_exp_002_local_explorer_v2_is_offline() {
    let value = portable_value();
    let mut bytes = serde_json::to_vec(&value).expect("serialize portable V2");
    bytes.push(b'\n');
    let portable = PortableGraphV2::from_canonical_file(&bytes, digest)
        .expect("strictly reimport portable V2");
    let viewer = b"<!doctype html><title>K1</title>";
    let viewer_digest = digest(viewer);
    let manifest = LocalExplorerManifestV2::new(
        &portable,
        viewer,
        &viewer_digest,
        "default-src 'none'; script-src 'sha256-test'; connect-src 'none'",
        digest,
    )
    .expect("build offline explorer V2 manifest");
    assert_eq!(
        manifest.value()["schema_version"],
        K1_LOCAL_EXPLORER_VERSION
    );
    assert_eq!(
        manifest.value()["security"]["profile"],
        K1_EXPLORER_SECURITY_PROFILE
    );
    assert_eq!(manifest.value()["security"]["network"], false);
    assert_eq!(manifest.value()["security"]["dynamic_code"], false);
    assert_eq!(manifest.value()["security"]["storage"], false);
    assert_eq!(manifest.value()["security"]["telemetry"], false);
}

#[test]
fn sec_fr_exp_002_excludes_body_expression_and_source_text() {
    let mut value = portable_value();
    value["entities"][0]["body_text"] = Value::String("private body".to_owned());
    value["projection"]["family_sha256"] = family_digests(&value);
    let mut bytes = serde_json::to_vec(&value).expect("serialize private portable V2");
    bytes.push(b'\n');
    assert_eq!(
        PortableGraphV2::from_canonical_file(&bytes, digest)
            .expect_err("private source body must fail closed"),
        K1ContractError::UnsafePayload {
            reason: "private_field"
        }
    );
}

fn portable_value() -> Value {
    let entity_id = entity_id('a');
    let claim_id = claim_id('b');
    let evidence_id = evidence_id('c');
    let mut value = json!({
        "schema_version": K1_PORTABLE_GRAPH_VERSION,
        "repository": {
            "identity": "urn:codenoesis:test:k1-portable",
            "commit_oid": "d".repeat(40)
        },
        "source_snapshot": {
            "schema_version": K1_SNAPSHOT_VERSION,
            "snapshot_id": format!("urn:codenoesis:snapshot:blake3:{}", "e".repeat(64)),
            "semantic_hash": {"algorithm": "blake3-256", "value": "f".repeat(64)}
        },
        "ontology_version": K1_ONTOLOGY_VERSION,
        "query_contract_version": K1_QUERY_VERSION,
        "projection": {
            "profile": "codenoesis.lossless-portable-projection/v2",
            "family_sha256": {}
        },
        "entities": [{
            "id": entity_id,
            "kind": "rust.callable_signature",
            "name": "callable",
            "evidence_ids": [evidence_id]
        }],
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
            "blob_oid": "1".repeat(40),
            "start_byte": 0,
            "end_byte": 1
        }],
        "diagnostics": [],
        "coverage_gaps": [],
        "documents": [],
        "document_statements": []
    });
    value["projection"]["family_sha256"] = family_digests(&value);
    value
}

fn family_digests(value: &Value) -> Value {
    let mut values = Map::new();
    for family in [
        "entities",
        "relationships",
        "claims",
        "evidence",
        "diagnostics",
        "coverage_gaps",
        "documents",
        "document_statements",
    ] {
        values.insert(
            family.to_owned(),
            Value::String(digest(
                &serde_json::to_vec(&value[family]).expect("serialize portable family"),
            )),
        );
    }
    Value::Object(values)
}

fn digest(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

fn entity_id(value: char) -> String {
    format!(
        "urn:codenoesis:entity:blake3:{}",
        value.to_string().repeat(64)
    )
}

fn relationship_id(value: char) -> String {
    format!(
        "urn:codenoesis:relationship:blake3:{}",
        value.to_string().repeat(64)
    )
}

fn claim_id(value: char) -> String {
    format!(
        "urn:codenoesis:claim:blake3:{}",
        value.to_string().repeat(64)
    )
}

fn evidence_id(value: char) -> String {
    format!(
        "urn:codenoesis:evidence:blake3:{}",
        value.to_string().repeat(64)
    )
}
