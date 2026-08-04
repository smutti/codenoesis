use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use codenoesis_contracts::{
    CodeNoesisErrorV11, DocumentationContractError, QueryContractError, generate_documentation_v1,
    local_query_result_v2, validate_documentation_bundle_v1,
};
use codenoesis_domain::s4_r4::{
    CargoFactKind, CargoFactLimit, CargoFactReason, CargoManifestFactError,
};
use serde_json::{Value, json};

#[test]
#[allow(clippy::too_many_lines)]
fn conf_fr_ext_009_snapshot_v7_graph_v4_error_v11() {
    let snapshot_schema = specification("repository-snapshot-v7.schema.json");
    assert_eq!(
        snapshot_schema["properties"]["schema_version"]["const"],
        "codenoesis.repository-snapshot/v7"
    );
    assert_eq!(snapshot_schema["additionalProperties"], false);

    let graph_schema = specification("knowledge-graph-v4.schema.json");
    assert_eq!(
        graph_schema["properties"]["schema_version"]["const"],
        "codenoesis.knowledge-graph/v4"
    );
    assert_eq!(
        graph_schema["properties"]["diagnostics"]["items"]["$ref"],
        "extraction-chunk-v4.schema.json#/$defs/diagnostic"
    );

    let query_schema = specification("local-query-result-v2.schema.json");
    assert_eq!(
        query_schema["properties"]["schema_version"]["const"],
        "codenoesis.local-query-result/v2"
    );
    assert_eq!(query_schema["allOf"].as_array().map(Vec::len), Some(7));

    assert_eq!(
        CodeNoesisErrorV11::invalid_manifest_profile()
            .canonical_stderr()
            .expect("serialize invalid manifest profile"),
        b"{\"code\":\"input.invalid_manifest_profile\",\"context\":{},\"message\":\"invalid manifest profile\",\"retryable\":false,\"schema_version\":\"codenoesis.error/v11\",\"stage\":\"input\"}\n"
    );
    let invalid = CargoManifestFactError::InvalidFact {
        reason: CargoFactReason::MalformedValue,
        path: "Cargo.toml".to_owned(),
        fact_kind: CargoFactKind::Manifest,
        field: Some("package"),
    };
    let invalid_error = CodeNoesisErrorV11::from_manifest(&invalid)
        .expect("map closed invalid manifest fact")
        .canonical_stderr()
        .expect("serialize invalid manifest fact");
    let invalid_value: Value =
        serde_json::from_slice(&invalid_error).expect("parse invalid manifest ErrorV11");
    assert_eq!(
        invalid_value["code"],
        "extraction.invalid_cargo_manifest_fact"
    );
    assert_eq!(invalid_value["context"]["reason"], "malformed_value");
    assert_eq!(invalid_value["context"]["path"], "Cargo.toml");
    assert_eq!(invalid_value["context"]["fact_kind"], "manifest");
    assert_eq!(invalid_value["context"]["field"], "package");

    let limit = CargoManifestFactError::LimitExceeded {
        limit: CargoFactLimit::DependenciesPerManifest,
        maximum: 256,
        observed: 257,
    };
    let limit_value: Value = serde_json::from_slice(
        &CodeNoesisErrorV11::from_manifest(&limit)
            .expect("map closed limit")
            .canonical_stderr()
            .expect("serialize limit ErrorV11"),
    )
    .expect("parse limit ErrorV11");
    assert_eq!(
        limit_value["code"],
        "extraction.cargo_manifest_fact_limit_exceeded"
    );
    assert_eq!(limit_value["context"]["limit"], "dependencies_per_manifest");
    assert_eq!(limit_value["context"]["maximum"], 256);
    assert_eq!(limit_value["context"]["observed"], 257);

    let entity_id = format!("urn:codenoesis:entity:blake3:{}", "1".repeat(64));
    let claim_id = format!("urn:codenoesis:claim:blake3:{}", "2".repeat(64));
    let evidence_id = format!("urn:codenoesis:evidence:blake3:{}", "3".repeat(64));
    let semantic = json!({
        "repository": {"identity": "urn:codenoesis:test:r4-contract"},
        "knowledge_graph": {
            "entities": [{"id": entity_id, "kind": "cargo.manifest"}],
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
                "path": "Cargo.toml",
                "blob_oid": "1111111111111111111111111111111111111111",
                "start_byte": 0,
                "end_byte": 1
            }],
            "diagnostics": [],
            "coverage": []
        }
    });
    let manifest = json!({
        "schema_version": "codenoesis.documentation-manifest/v1",
        "repository_identity": "urn:codenoesis:test:r4-contract",
        "snapshot_id": "urn:codenoesis:snapshot:blake3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "renderer_version": "codenoesis.renderer/markdown-v1",
        "documents": []
    });
    let result = local_query_result_v2(
        &semantic,
        &manifest,
        manifest["snapshot_id"].as_str().expect("snapshot ID"),
        &entity_id,
    )
    .expect("build strict entity query V2")
    .canonical_stdout()
    .expect("serialize strict entity query V2");
    let value: Value = serde_json::from_slice(&result).expect("parse query V2");
    assert_eq!(value["schema_version"], "codenoesis.local-query-result/v2");
    assert_eq!(value["result_kind"], "entity");
    assert_eq!(value["relationship"], Value::Null);
    assert_eq!(value["diagnostic"], Value::Null);
    assert_eq!(value["coverage_gap"], Value::Null);
    assert_eq!(
        local_query_result_v2(
            &semantic,
            &manifest,
            manifest["snapshot_id"].as_str().expect("snapshot ID"),
            &format!("urn:codenoesis:entity:blake3:{}", "f".repeat(64)),
        )
        .expect_err("unknown exact ID must fail"),
        QueryContractError::NotFound
    );
}

#[test]
fn reg_fr_doc_001_r4_declared_import_survives_omitted_r3_coverage() {
    let repository_identity = "urn:codenoesis:test:r4-docs";
    let snapshot_id = format!("urn:codenoesis:snapshot:blake3:{}", "a".repeat(64));
    let crate_id = format!("urn:codenoesis:entity:blake3:{}", "1".repeat(64));
    let source_id = format!("urn:codenoesis:entity:blake3:{}", "2".repeat(64));
    let module_id = format!("urn:codenoesis:entity:blake3:{}", "3".repeat(64));
    let symbol_id = format!("urn:codenoesis:entity:blake3:{}", "4".repeat(64));
    let relationship_id = format!("urn:codenoesis:relationship:blake3:{}", "5".repeat(64));
    let module_claim_id = format!("urn:codenoesis:claim:blake3:{}", "6".repeat(64));
    let relationship_claim_id = format!("urn:codenoesis:claim:blake3:{}", "7".repeat(64));
    let evidence_id = format!("urn:codenoesis:evidence:blake3:{}", "8".repeat(64));
    let mut semantic = json!({
        "repository": {"identity": repository_identity},
        "knowledge_graph": {
            "schema_version": "codenoesis.knowledge-graph/v4",
            "entities": [
                {
                    "id": crate_id,
                    "kind": "rust.crate",
                    "crate_id": crate_id,
                    "module_path": "crate",
                    "name": "fixture",
                    "visibility": "public",
                    "properties": {
                        "manifest_path": "Cargo.toml",
                        "package_name": "fixture",
                        "target_kind": "lib",
                        "target_name": "fixture"
                    }
                },
                {
                    "id": source_id,
                    "kind": "source.file",
                    "crate_id": crate_id,
                    "module_path": null,
                    "name": "src/client.rs",
                    "visibility": "not_applicable",
                    "properties": {
                        "blob_oid": "1111111111111111111111111111111111111111",
                        "path": "src/client.rs"
                    }
                },
                {
                    "id": module_id,
                    "kind": "rust.module",
                    "crate_id": crate_id,
                    "module_path": "crate::client",
                    "name": "fixture::client",
                    "visibility": "private",
                    "properties": {"source_file_id": source_id}
                },
                {
                    "id": symbol_id,
                    "kind": "rust.symbol_reference",
                    "crate_id": crate_id,
                    "module_path": "crate::client",
                    "name": "remote::Api",
                    "visibility": "not_applicable",
                    "properties": {
                        "resolution_state": "unresolved",
                        "spelling": "remote::Api"
                    }
                }
            ],
            "relationships": [{
                "id": relationship_id,
                "kind": "IMPORTS",
                "source": module_id,
                "target": symbol_id,
                "evidence_ids": [evidence_id]
            }],
            "claims": [
                {
                    "id": module_claim_id,
                    "subject_kind": "entity",
                    "subject_id": module_id,
                    "state": "deterministic_fact",
                    "evidence_ids": [evidence_id]
                },
                {
                    "id": relationship_claim_id,
                    "subject_kind": "relationship",
                    "subject_id": relationship_id,
                    "state": "derived_fact",
                    "evidence_ids": [evidence_id]
                }
            ],
            "evidence": [{
                "id": evidence_id,
                "path": "Cargo.toml",
                "blob_oid": "1111111111111111111111111111111111111111",
                "start_byte": 0,
                "end_byte": 1
            }],
            "diagnostics": [],
            "coverage": []
        }
    });

    let generated = generate_documentation_v1(&semantic, &snapshot_id, &"b".repeat(64))
        .expect("R4 docs retain declaration-only unresolved imports");
    let module = generated
        .documents()
        .iter()
        .find(|document| document.path.starts_with("modules/"))
        .expect("generated module document");
    let markdown = std::str::from_utf8(&module.bytes).expect("UTF-8 module Markdown");
    assert!(markdown.contains(
        "Declared import reference: `remote::Api`; cross-crate resolution is unavailable."
    ));
    let document_bytes = generated
        .documents()
        .iter()
        .map(|document| (document.path.clone(), document.bytes.clone()))
        .collect::<BTreeMap<_, _>>();
    validate_documentation_bundle_v1(
        generated.manifest(),
        &document_bytes,
        repository_identity,
        &snapshot_id,
        &"b".repeat(64),
    )
    .expect("validate R4 declaration-only documentation");

    semantic["knowledge_graph"]["schema_version"] =
        Value::String("codenoesis.knowledge-graph/v3".to_owned());
    assert_eq!(
        generate_documentation_v1(&semantic, &snapshot_id, &"b".repeat(64))
            .expect_err("legacy graph without required coverage must remain invalid"),
        DocumentationContractError::InvalidSnapshot
    );
}

fn specification(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/specifications/s4/r4")
        .join(name);
    serde_json::from_slice(&fs::read(path).expect("read reviewed R4 schema"))
        .expect("parse reviewed R4 schema")
}
