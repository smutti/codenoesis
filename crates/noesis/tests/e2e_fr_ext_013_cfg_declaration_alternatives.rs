mod support;

use std::collections::BTreeSet;
use std::fs;

use serde_json::Value;
use sha2::{Digest as _, Sha256};

use support::parse_single_document;
use support::s4_r10::MaterializedCfgAlternativesRepository;

const LOGICAL_METHOD_ID: &str =
    "urn:codenoesis:entity:blake3:437b0bfcd3821ae91eabe8c395d99c80ec54cc53e6f1e6ca6e24098b20bf4b45";
const UNIX_ALTERNATIVE_ID: &str =
    "urn:codenoesis:entity:blake3:452f8e5e1fe8f0e22b43d49b7393c1b224261b8b2f47452ebaee4ca19794542d";
const WINDOWS_ALTERNATIVE_ID: &str =
    "urn:codenoesis:entity:blake3:df85456dbd86d6ded1dd8a752c159e8d838ebf954feb556f47c6200e9a2843c6";
const EXPECTED_RED_STDERR_SHA256: &str =
    "dda30410fb0e9ea21d098ac38074c69d6316d777ee16340175ad7db00aa26be1";

#[test]
fn e2e_fr_ext_013_cfg_method_alternatives_publish_declarations() {
    let repository = MaterializedCfgAlternativesRepository::fixture();
    assert!(!repository.build_sentinel().exists());
    let output = repository.scan();

    if output.status.code() == Some(2) {
        assert!(output.stdout.is_empty(), "R10 expected-Red stdout changed");
        assert_eq!(output.stderr.len(), 233, "R10 expected-Red length changed");
        assert_eq!(
            hex_sha256(&output.stderr),
            EXPECTED_RED_STDERR_SHA256,
            "R10 expected-Red digest changed"
        );
        let error: Value = parse_single_document(&output.stderr);
        assert_eq!(error["schema_version"], "codenoesis.error/v12");
        assert_eq!(error["code"], "input.invalid_rust_semantic_profile");
        assert!(!repository.store.exists(), "R10 expected Red mutated store");
        assert!(!repository.build_sentinel().exists());
        panic!("expected RepositorySnapshotV12 success; observed frozen missing-profile Red");
    }

    assert!(
        output.status.success(),
        "R10 scan failed: status={:?}, stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "successful R10 stderr changed");
    assert!(
        !repository.build_sentinel().exists(),
        "R10 executed build.rs"
    );

    let snapshot: Value = parse_single_document(&output.stdout);
    assert_eq!(
        snapshot["schema_version"],
        "codenoesis.repository-snapshot/v12"
    );
    assert_eq!(
        snapshot["semantic"]["configuration"]["schema_version"],
        "codenoesis.configuration/v9"
    );
    assert_eq!(
        snapshot["semantic"]["configuration"]["rust_semantic_profile"],
        "rust-cfg-declaration-alternatives-v1"
    );
    let graph = &snapshot["semantic"]["knowledge_graph"];
    assert_eq!(graph["schema_version"], "codenoesis.knowledge-graph/v9");
    assert_eq!(graph["ontology_version"], "codenoesis.ontology/rust/v9");

    let entities = graph["entities"].as_array().expect("R10 graph entities");
    let logical = entities
        .iter()
        .find(|entity| entity["id"] == LOGICAL_METHOD_ID)
        .expect("R10 logical method");
    assert_eq!(logical["kind"], "rust.method");
    assert_eq!(logical["properties"]["declaration_state"], "alternatives");
    assert_eq!(
        logical["properties"]["declaration_alternative_ids"],
        serde_json::json!([UNIX_ALTERNATIVE_ID, WINDOWS_ALTERNATIVE_ID])
    );
    for forbidden in [
        "receiver_present",
        "declared_signature",
        "compilation_presence",
        "attributes",
    ] {
        assert!(
            logical["properties"].get(forbidden).is_none(),
            "logical method selected occurrence property {forbidden}"
        );
    }

    let alternatives = entities
        .iter()
        .filter(|entity| entity["kind"] == "rust.declaration_alternative")
        .collect::<Vec<_>>();
    assert_eq!(alternatives.len(), 2);
    assert_eq!(
        alternatives
            .iter()
            .map(|entity| entity["id"].as_str().expect("alternative ID"))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([UNIX_ALTERNATIVE_ID, WINDOWS_ALTERNATIVE_ID])
    );
    assert!(alternatives.iter().all(|entity| {
        entity["subject_id"] == LOGICAL_METHOD_ID
            && entity["properties"]["declaration_kind"] == "rust.method"
            && entity["properties"]["compilation_presence"] == "conditional_unknown"
            && entity["properties"]["receiver_present"] == true
    }));

    let relationships = graph["relationships"]
        .as_array()
        .expect("R10 relationships");
    assert_eq!(
        relationships
            .iter()
            .filter(|relationship| {
                relationship["kind"] == "HAS_DECLARATION_ALTERNATIVE"
                    && relationship["source"] == LOGICAL_METHOD_ID
            })
            .count(),
        2
    );
    assert!(
        repository.store.exists(),
        "R10 did not publish one visible head"
    );
}

fn hex_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn conf_fr_ext_013_fixture_bytes_are_reviewed() {
    let fixture = support::s4_r10::fixture_root();
    let expected: Value = serde_json::from_slice(
        &fs::read(fixture.join("expected-declaration-alternatives.json"))
            .expect("read R10 expected facts"),
    )
    .expect("parse R10 expected facts");
    assert_eq!(expected["logical_method"]["id"], LOGICAL_METHOD_ID);
    assert_eq!(expected["alternatives"].as_array().map(Vec::len), Some(2));
}
