mod support;

use std::collections::BTreeMap;
use std::process::Output;

use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use support::parse_single_document;
use support::s4_r14_r15_correction::{MaterializedR14R15CorrectionRepository, expected_correction};

const R14_RED_SHA256: &str = "9b284f4bb7368bb0d11c5b33725c109ee469845aac00081e51175413adec4e3c";
const R15_RED_SHA256: &str = "01f7b883892dc357c177c072ce2cca62b3abbf3cbb22f40d173120a283181564";
const PROFILE_RED_SHA256: &str = "e84e29861457502d4d5643259fbd0669ad8ad2dece27a7d8bdd6734a812e819c";

#[test]
fn e2e_fr_ext_016_real_repository_shapes_are_fail_closed() {
    let repository = MaterializedR14R15CorrectionRepository::fixture();
    let output = repository.scan_r14();
    retain_expected_red(
        &output,
        11,
        196,
        R14_RED_SHA256,
        "codenoesis.error/v21",
        "expression_extraction",
        "R14",
    );
    assert_success(&output, "corrected R14 fixture scan");
    verify_snapshot(&output.stdout, &expected_correction()["r14"], false);
}

#[test]
fn e2e_fr_ext_017_real_repository_shapes_are_fail_closed() {
    let repository = MaterializedR14R15CorrectionRepository::fixture();
    let output = repository.scan_r15();
    retain_expected_red(
        &output,
        11,
        196,
        R15_RED_SHA256,
        "codenoesis.error/v22",
        "local_flow_extraction",
        "R15",
    );
    assert_success(&output, "corrected R15 fixture scan");
    verify_snapshot(&output.stdout, &expected_correction()["r15"], true);
}

#[test]
fn conf_fr_cli_001_r14_r15_256m_profile_is_explicit() {
    let repository = MaterializedR14R15CorrectionRepository::fixture();
    let output = repository.scan_r14_with_256m_profile();
    if output.status.code() == Some(2) {
        assert!(output.stdout.is_empty(), "profile Red wrote stdout");
        assert_eq!(output.stderr.len(), 357, "profile Red length changed");
        assert_eq!(hex_sha256(&output.stderr), PROFILE_RED_SHA256);
        let error = parse_single_document(&output.stderr);
        assert_eq!(error["schema_version"], "codenoesis.error/v21");
        assert_eq!(
            error["code"],
            "input.unsupported_rust_expression_composition"
        );
        assert!(
            !repository.profile_store.exists(),
            "profile Red created store"
        );
        panic!("expected registered local-snapshot-256m-v1; observed frozen profile Red");
    }
    assert_success(&output, "R14 256 MiB profile scan");
    let snapshot = parse_single_document(&output.stdout);
    assert_eq!(
        snapshot["semantic"]["configuration"]["output_capacity_profile"],
        Value::Null
    );
    verify_snapshot(&output.stdout, &expected_correction()["r14"], false);
}

#[test]
fn sec_inv_bnd_001_r14_gitlink_is_typed_before_publication() {
    let repository = MaterializedR14R15CorrectionRepository::fixture_with_gitlink();
    let output = repository.scan_r14();
    retain_boundary_red(&output, "codenoesis.error/v21", "R14");
    assert_typed_boundary_failure(
        &output,
        "codenoesis.error/v21",
        "input.unsupported_rust_expression_composition",
        &repository.r14_store,
    );
}

#[test]
fn sec_inv_bnd_001_r15_gitlink_is_typed_before_publication() {
    let repository = MaterializedR14R15CorrectionRepository::fixture_with_gitlink();
    let output = repository.scan_r15();
    retain_boundary_red(&output, "codenoesis.error/v22", "R15");
    assert_typed_boundary_failure(
        &output,
        "codenoesis.error/v22",
        "input.unsupported_rust_flow_composition",
        &repository.r15_store,
    );
}

fn retain_expected_red(
    output: &Output,
    exit: i32,
    stderr_bytes: usize,
    stderr_sha256: &str,
    schema: &str,
    stage: &str,
    profile: &str,
) {
    if output.status.code() != Some(exit) {
        return;
    }
    assert!(output.stdout.is_empty(), "{profile} Red wrote stdout");
    assert_eq!(
        output.stderr.len(),
        stderr_bytes,
        "{profile} Red length changed"
    );
    assert_eq!(hex_sha256(&output.stderr), stderr_sha256);
    let error = parse_single_document(&output.stderr);
    assert_eq!(error["schema_version"], schema);
    assert_eq!(error["code"], "internal.unexpected");
    assert_eq!(error["context"]["stage"], stage);
    panic!("expected corrected {profile} success; observed frozen internal extraction Red");
}

fn retain_boundary_red(output: &Output, schema: &str, profile: &str) {
    if output.status.code() != Some(10) {
        return;
    }
    assert!(
        output.stdout.is_empty(),
        "{profile} boundary Red wrote stdout"
    );
    let error = parse_single_document(&output.stderr);
    assert_eq!(error["schema_version"], schema);
    assert_eq!(error["code"], "internal.unexpected");
    assert_eq!(error["context"]["stage"], "acquisition");
    panic!("expected typed {profile} gitlink rejection; observed frozen acquisition Red");
}

fn assert_typed_boundary_failure(
    output: &Output,
    schema: &str,
    code: &str,
    store: &std::path::Path,
) {
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error = parse_single_document(&output.stderr);
    assert_eq!(error["schema_version"], schema);
    assert_eq!(error["code"], code);
    assert_eq!(
        error["context"]["reason"],
        "repository_boundary_not_supported"
    );
    assert!(!store.exists(), "typed boundary failure created store");
}

fn verify_snapshot(bytes: &[u8], expected: &Value, include_flow: bool) {
    assert_eq!(
        bytes.len(),
        usize::try_from(
            expected["canonical_stdout_bytes"]
                .as_u64()
                .expect("stdout bytes")
        )
        .expect("stdout bytes fit usize")
    );
    let snapshot = parse_single_document(bytes);
    assert_eq!(
        snapshot["semantic_hash"]["value"],
        expected["semantic_hash"]
    );
    assert_eq!(
        snapshot["semantic"]["configuration"]["semantic_hash"]["value"],
        expected["configuration_semantic_hash"]
    );
    assert_eq!(
        snapshot["semantic"]["configuration"]["output_capacity_profile"],
        Value::Null
    );
    let graph = &snapshot["semantic"]["knowledge_graph"];
    for graph_family in [
        "entities",
        "relationships",
        "claims",
        "evidence",
        "diagnostics",
        "coverage",
    ] {
        let family = graph[graph_family]
            .as_array()
            .unwrap_or_else(|| panic!("{graph_family} array"));
        assert_eq!(
            family.len(),
            usize::try_from(
                expected["counts"][graph_family]
                    .as_u64()
                    .expect("family count")
            )
            .expect("family count fits usize"),
            "{graph_family} count"
        );
        assert_eq!(
            hex_sha256(&serde_json::to_vec(&graph[graph_family]).expect("serialize family")),
            expected["family_canonical_sha256"][graph_family]
                .as_str()
                .expect("canonical family digest"),
            "{graph_family} canonical digest"
        );
        let mut identifiers = family
            .iter()
            .map(|value| value["id"].as_str().expect("family ID"))
            .collect::<Vec<_>>();
        identifiers.sort_unstable();
        let id_payload = format!("{}\n", identifiers.join("\n"));
        assert_eq!(
            hex_sha256(id_payload.as_bytes()),
            expected["family_id_sha256"][graph_family]
                .as_str()
                .expect("family ID digest"),
            "{graph_family} ID digest"
        );
    }

    let call_sites = graph["entities"]
        .as_array()
        .expect("entities")
        .iter()
        .filter(|entity| entity["kind"] == "rust.call_site")
        .map(|entity| {
            json!({
                "id": entity["id"],
                "name": entity["name"],
                "target_spelling": entity["properties"]["target_spelling"]
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        Value::Array(call_sites),
        expected_correction()["r14"]["call_sites"]
    );
    for entity in graph["entities"].as_array().expect("entities") {
        if let Some(name) = entity["name"].as_str() {
            assert!(!name.contains("://"), "raw URL escaped into entity name");
        }
        if entity["kind"] == "rust.call_site" {
            assert!(
                !entity["properties"]["target_spelling"]
                    .as_str()
                    .expect("target spelling")
                    .contains("://"),
                "raw URL escaped into call target"
            );
        }
    }

    let omissions = &expected_correction()["r14"]["omissions"];
    let relationships = graph["relationships"].as_array().expect("relationships");
    assert!(!relationships.iter().any(|relationship| {
        relationship["kind"] == "HAS_ARGUMENT"
            && relationship["source"] == omissions["all_arguments_for_call_expression_id"]
    }));
    assert!(!relationships.iter().any(|relationship| {
        relationship["kind"] == "HAS_RECEIVER"
            && relationship["source"] == omissions["receiver_for_call_expression_id"]
    }));
    assert!(!relationships.iter().any(|relationship| {
        relationship["kind"] == "BINDS_FROM"
            && relationship["source"] == omissions["binds_from_for_binding_id"]
    }));

    if include_flow {
        assert_eq!(
            graph["local_flow_index"]["completed_callable_ids"],
            expected["completed_callable_ids"]
        );
        assert_eq!(
            graph["local_flow_index"]["derivations"]
                .as_array()
                .expect("derivations")
                .len(),
            usize::try_from(
                expected["retained_derivation_count"]
                    .as_u64()
                    .expect("derivations")
            )
            .expect("derivations fit usize")
        );
        let actual_blocks = graph["entities"]
            .as_array()
            .expect("entities")
            .iter()
            .filter(|entity| entity["kind"] == "rust.syntax_basic_block")
            .map(|entity| {
                (
                    entity["properties"]["ordinal"]
                        .as_u64()
                        .expect("actual block ordinal"),
                    json!({
                        "id": entity["id"],
                        "ordinal": entity["properties"]["ordinal"],
                        "role": entity["properties"]["role"],
                        "start_byte": entity["locator"]["start_byte"],
                        "end_byte": entity["locator"]["end_byte"]
                    }),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let expected_blocks = expected["blocks"]
            .as_array()
            .expect("expected blocks")
            .iter()
            .cloned()
            .map(|value| (value["ordinal"].as_u64().expect("block ordinal"), value))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(actual_blocks, expected_blocks);
    }
}

fn assert_success(output: &Output, label: &str) {
    assert!(
        output.status.success(),
        "{label} failed: status={:?}, stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "{label} wrote stderr");
}

fn hex_sha256(value: &[u8]) -> String {
    Sha256::digest(value)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
