mod support;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use sha2::{Digest as _, Sha256};

use support::parse_single_document;
use support::s4_r12::{MaterializedCallableCfgAlternativesRepository, expected_composition};

const EXPECTED_RED_STDERR_SHA256: &str =
    "dbe134dbc101765a8ebdc2ffe917f4776fddb42d10e3dfe1957e2aa819adb70c";

#[test]
#[allow(clippy::too_many_lines)]
fn e2e_fr_ext_014_k1_cfg_alternatives_complete_local_journey() {
    let repository = MaterializedCallableCfgAlternativesRepository::fixture();
    assert!(!repository.build_sentinel().exists());
    let scan = repository.scan();

    if scan.status.code() == Some(2) {
        assert!(scan.stdout.is_empty(), "R12 expected-Red stdout changed");
        assert_eq!(scan.stderr.len(), 344, "R12 expected-Red length changed");
        assert_eq!(
            hex_sha256(&scan.stderr),
            EXPECTED_RED_STDERR_SHA256,
            "R12 expected-Red digest changed"
        );
        let error = parse_single_document(&scan.stderr);
        assert_eq!(error["schema_version"], "codenoesis.error/v17");
        assert_eq!(
            error["code"],
            "input.unsupported_rust_cfg_alternatives_composition"
        );
        assert_eq!(
            error["context"]["profile"],
            "rust-cfg-declaration-alternatives-v1"
        );
        assert_eq!(error["context"]["required_lineage"], "r5_source_only");
        assert_eq!(error["context"]["reason"], "source_only_lineage_required");
        assert!(!repository.store.exists(), "R12 expected Red mutated store");
        assert!(!repository.build_sentinel().exists());
        panic!("expected RepositorySnapshotV14 success; observed frozen composition Red");
    }

    assert_success(&scan, "R12 callable cfg-alternatives scan");
    assert!(scan.stderr.is_empty());
    let snapshot = parse_single_document(&scan.stdout);
    assert_eq!(
        snapshot["schema_version"],
        "codenoesis.repository-snapshot/v14"
    );
    assert_eq!(
        snapshot["semantic"]["configuration"]["schema_version"],
        "codenoesis.configuration/v11"
    );
    assert_eq!(
        snapshot["semantic"]["configuration"]["rust_semantic_profile"],
        "rust-cfg-declaration-alternatives-v1"
    );
    assert_eq!(
        snapshot["semantic"]["configuration"]["rust_framework_profile"],
        "rust-framework-declarations-v1"
    );
    assert_eq!(
        snapshot["semantic"]["configuration"]["rust_callable_profile"],
        "rust-callable-semantics-v1"
    );
    assert_eq!(
        snapshot["semantic"]["configuration"]["repository_boundary_profile"],
        Value::Null
    );
    assert_eq!(snapshot["semantic"]["repository_boundaries"], Value::Null);

    let expected = expected_composition();
    let graph = &snapshot["semantic"]["knowledge_graph"];
    assert_eq!(graph["schema_version"], "codenoesis.knowledge-graph/v11");
    assert_eq!(graph["ontology_version"], "codenoesis.ontology/rust/v11");
    assert_eq!(
        callable_counts(graph),
        expected["callable_entity_counts"]
            .as_object()
            .expect("R12 expected entity counts")
            .iter()
            .map(|(kind, count)| (kind.as_str(), count.as_u64().expect("R12 entity count")))
            .collect()
    );
    let relationship_counts = count_by(graph, "relationships", "kind");
    for (kind, expected_count) in expected["callable_relationship_counts"]
        .as_object()
        .expect("R12 expected relationship counts")
    {
        assert_eq!(
            relationship_counts.get(kind.as_str()),
            expected_count.as_u64().as_ref(),
            "R12 relationship count {kind}"
        );
    }
    assert_eq!(
        graph["callable_semantics_index"]["unresolved_call_site_ids"]
            .as_array()
            .expect("R12 unresolved calls")
            .len(),
        usize::try_from(
            expected["unresolved_call_sites"]
                .as_u64()
                .expect("R12 expected unresolved calls"),
        )
        .expect("R12 unresolved call count fits usize")
    );

    let logical_id = expected["logical_method"]["id"]
        .as_str()
        .expect("R12 logical ID");
    let entities = graph["entities"].as_array().expect("R12 graph entities");
    let logical = entities
        .iter()
        .find(|entity| entity["id"] == logical_id)
        .expect("R12 logical method");
    assert_eq!(logical["kind"], "rust.method");
    assert_eq!(logical["properties"]["declaration_state"], "alternatives");
    for forbidden in [
        "receiver_present",
        "declared_signature",
        "compilation_presence",
        "attributes",
        "callable_signature_id",
        "parameter_ids",
        "body_fact_ids",
    ] {
        assert!(
            logical["properties"].get(forbidden).is_none(),
            "R12 logical method selected occurrence property {forbidden}"
        );
    }

    let relationships = graph["relationships"]
        .as_array()
        .expect("R12 graph relationships");
    for forbidden_kind in ["HAS_SIGNATURE", "HAS_BODY_FACT", "CALLS"] {
        assert!(relationships.iter().all(|relationship| {
            relationship["kind"] != forbidden_kind || relationship["source"] != logical_id
        }));
    }

    for alternative in expected["alternatives"]
        .as_array()
        .expect("R12 expected alternatives")
    {
        let alternative_id = alternative["id"].as_str().expect("R12 alternative ID");
        let signature_id = alternative["callable_signature_id"]
            .as_str()
            .expect("R12 signature ID");
        assert!(entities.iter().any(|entity| {
            entity["id"] == alternative_id
                && entity["kind"] == "rust.declaration_alternative"
                && entity["subject_id"] == logical_id
        }));
        assert!(entities.iter().any(|entity| {
            entity["id"] == signature_id && entity["kind"] == "rust.callable_signature"
        }));
        assert_eq!(
            relationships
                .iter()
                .filter(|relationship| {
                    relationship["kind"] == "HAS_SIGNATURE"
                        && relationship["source"] == alternative_id
                        && relationship["target"] == signature_id
                })
                .count(),
            1
        );
        assert_eq!(
            relationships
                .iter()
                .filter(|relationship| {
                    relationship["kind"] == "HAS_PARAMETER"
                        && relationship["source"] == signature_id
                })
                .count(),
            2
        );
        assert!(relationships.iter().any(|relationship| {
            relationship["id"] == alternative["calls_relationship_id"]
                && relationship["kind"] == "CALLS"
                && relationship["source"] == alternative_id
                && relationship["target"] == expected["helper_target_id"]
        }));
    }

    let join = &graph["callable_cfg_alternatives_index"];
    assert_eq!(
        join["schema_version"],
        "codenoesis.callable-cfg-alternatives-index/v1"
    );
    assert_eq!(join["logical_method_ids"], serde_json::json!([logical_id]));
    assert_eq!(
        join["alternative_callable_subject_ids"],
        expected["logical_method"]["declaration_alternative_ids"]
    );

    assert_success(&repository.docs(), "R12 documentation generation");
    let first_alternative = &expected["alternatives"][0];
    let documentation = generated_markdown(&repository.documents);
    assert!(documentation.contains("Conditional declaration alternatives"));
    assert!(documentation.contains("no active target is selected"));
    assert!(
        documentation.contains(
            first_alternative["id"]
                .as_str()
                .expect("R12 documented alternative ID")
        )
    );
    assert!(
        documentation.contains(
            first_alternative["callable_signature_id"]
                .as_str()
                .expect("R12 documented signature ID")
        )
    );
    for requested_id in [
        logical_id,
        first_alternative["id"]
            .as_str()
            .expect("R12 alternative query ID"),
        first_alternative["callable_signature_id"]
            .as_str()
            .expect("R12 signature query ID"),
    ] {
        let query = repository.query(requested_id);
        assert_success(&query, "R12 exact-ID query");
        let result = parse_single_document(&query.stdout);
        assert_eq!(result["schema_version"], "codenoesis.local-query-result/v9");
        if requested_id == logical_id {
            assert!(
                result["linked_r10_entities"]
                    .as_array()
                    .expect("R12 logical linked alternatives")
                    .iter()
                    .any(|entity| entity["id"] == first_alternative["id"])
            );
        } else if requested_id == first_alternative["id"] {
            assert!(
                result["linked_k1_entities"]
                    .as_array()
                    .expect("R12 alternative linked callable facts")
                    .iter()
                    .any(|entity| entity["id"] == first_alternative["callable_signature_id"])
            );
        } else {
            assert_eq!(
                result["linked_k1_entities"]
                    .as_array()
                    .expect("R12 signature linked parameters")
                    .iter()
                    .filter(|entity| entity["kind"] == "rust.parameter")
                    .count(),
                2
            );
        }
    }

    let export = repository.export();
    assert_success(&export, "R12 portable export");
    let portable_bytes =
        fs::read(repository.portable.join("portable-graph.json")).expect("read R12 portable");
    assert_eq!(export.stdout, portable_bytes);
    let portable = parse_single_document(&portable_bytes);
    assert_eq!(portable["schema_version"], "codenoesis.portable-graph/v5");
    assert_eq!(
        portable["source_snapshot"]["schema_version"],
        "codenoesis.repository-snapshot/v14"
    );
    assert_private(&portable);

    let explore = repository.explore();
    assert_success(&explore, "R12 offline explorer");
    let manifest = parse_single_document(&explore.stdout);
    assert_eq!(manifest["schema_version"], "codenoesis.local-explorer/v5");
    assert_eq!(manifest["security"]["network"], false);
    assert_eq!(manifest["security"]["dynamic_code"], false);
    assert_eq!(
        fs::read(repository.explorer.join("index.html")).expect("read R12 viewer"),
        fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/s4/k1/index.html"))
            .expect("read immutable K1 viewer")
    );
    assert!(!repository.build_sentinel().exists());

    let with_boundary = MaterializedCallableCfgAlternativesRepository::fixture();
    let boundary_scan =
        with_boundary.scan_with_extra(&["--repository-boundary-profile", "local-gitlinks-v1"]);
    assert_success(&boundary_scan, "R12 optional boundary scan");
    let boundary_snapshot = parse_single_document(&boundary_scan.stdout);
    assert_eq!(
        boundary_snapshot["schema_version"],
        "codenoesis.repository-snapshot/v14"
    );
    assert_eq!(
        boundary_snapshot["semantic"]["configuration"]["repository_boundary_profile"],
        "local-gitlinks-v1"
    );
    assert!(boundary_snapshot["semantic"]["repository_boundaries"].is_object());

    let with_capacity = MaterializedCallableCfgAlternativesRepository::fixture();
    let capacity_scan =
        with_capacity.scan_with_extra(&["--output-capacity-profile", "local-snapshot-64m-v1"]);
    assert_success(&capacity_scan, "R12 optional output-capacity scan");
    let capacity_snapshot = parse_single_document(&capacity_scan.stdout);
    assert_eq!(capacity_snapshot["semantic"], snapshot["semantic"]);
    assert_eq!(
        capacity_snapshot["semantic_hash"],
        snapshot["semantic_hash"]
    );
}

#[test]
fn e2e_fr_cli_001_r12_invalid_selector_matrix_fails_before_acquisition() {
    let cases = [
        (
            vec![
                "--rust-semantic-profile",
                "rust-cfg-declaration-alternatives-v1",
                "--rust-callable-profile",
                "rust-callable-semantics-v1",
            ],
            "codenoesis.error/v17",
            "input.unsupported_rust_cfg_alternatives_composition",
        ),
        (
            vec![
                "--rust-semantic-profile",
                "rust-cfg-declaration-alternatives-v1",
                "--rust-framework-profile",
                "rust-framework-declarations-v1",
            ],
            "codenoesis.error/v17",
            "input.unsupported_rust_cfg_alternatives_composition",
        ),
        (
            vec![
                "--rust-semantic-profile",
                "rust-cfg-declaration-alternatives-v1",
                "--rust-framework-profile",
                "rust-framework-declarations-v1",
                "--rust-callable-profile",
                "rust-callable-semantics-v1",
                "--compiler-index-profile",
                "scip-rust-v0.9.0-import-v1",
            ],
            "codenoesis.error/v19",
            "input.unsupported_rust_callable_cfg_alternatives_composition",
        ),
        (
            vec![
                "--rust-semantic-profile",
                "rust-cfg-declaration-alternatives-v1",
                "--rust-framework-profile",
                "rust-framework-declarations-v1",
                "--rust-callable-profile",
                "rust-callable-semantics-v1",
                "--repository-boundary-manifest",
                "missing.json",
            ],
            "codenoesis.error/v19",
            "input.invalid_repository_boundary_manifest",
        ),
        (
            vec![
                "--rust-semantic-profile",
                "rust-cfg-declaration-alternatives-invalid",
                "--rust-framework-profile",
                "rust-framework-declarations-v1",
                "--rust-callable-profile",
                "rust-callable-semantics-v1",
            ],
            "codenoesis.error/v19",
            "input.invalid_rust_cfg_alternatives_profile",
        ),
        (
            vec![
                "--rust-semantic-profile",
                "rust-cfg-declaration-alternatives-v1",
                "--rust-framework-profile",
                "rust-framework-declarations-v1",
                "--rust-callable-profile",
                "rust-callable-semantics-invalid",
            ],
            "codenoesis.error/v19",
            "input.invalid_rust_callable_profile",
        ),
    ];

    for (arguments, expected_schema, expected_code) in cases {
        let repository = MaterializedCallableCfgAlternativesRepository::fixture();
        let output = scan_with_rust_arguments(&repository, &arguments);
        assert_eq!(output.status.code(), Some(2), "R12 invalid selector status");
        assert!(output.stdout.is_empty(), "R12 invalid selector stdout");
        let error = parse_single_document(&output.stderr);
        assert_eq!(error["schema_version"], expected_schema);
        assert_eq!(error["code"], expected_code);
        assert_eq!(error["retryable"], false);
        assert!(
            !repository.store.exists(),
            "R12 invalid selector mutated store"
        );
        assert!(!repository.build_sentinel().exists());
    }
}

fn callable_counts(graph: &Value) -> BTreeMap<&str, u64> {
    count_by(graph, "entities", "kind")
        .into_iter()
        .filter(|(kind, _)| {
            matches!(
                *kind,
                "rust.callable_signature"
                    | "rust.parameter"
                    | "rust.declared_value"
                    | "rust.local_binding"
                    | "rust.call_site"
                    | "rust.control"
            )
        })
        .collect()
}

fn count_by<'a>(graph: &'a Value, family: &str, field: &str) -> BTreeMap<&'a str, u64> {
    graph[family]
        .as_array()
        .expect("R12 graph family")
        .iter()
        .filter_map(|record| record[field].as_str())
        .fold(BTreeMap::new(), |mut counts, value| {
            *counts.entry(value).or_insert(0) += 1;
            counts
        })
}

fn assert_private(value: &Value) {
    match value {
        Value::Object(fields) => {
            for (name, nested) in fields {
                assert!(!matches!(
                    name.as_str(),
                    "body_text"
                        | "expression_text"
                        | "initializer_text"
                        | "source_contents"
                        | "source_snippet"
                        | "repository_root"
                        | "raw_url"
                        | "environment"
                        | "telemetry"
                ));
                assert_private(nested);
            }
        }
        Value::Array(values) => values.iter().for_each(assert_private),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn assert_success(output: &Output, subject: &str) {
    assert!(
        output.status.success(),
        "{subject} failed: status={:?}, stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn scan_with_rust_arguments(
    repository: &MaterializedCallableCfgAlternativesRepository,
    rust_arguments: &[&str],
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_noesis"));
    command
        .current_dir(&repository.root)
        .args(["scan", "--repository"])
        .arg(&repository.worktree)
        .args([
            "--repository-id",
            support::s4_r12::REPOSITORY_ID,
            "--revision",
        ])
        .arg(&repository.commit_oid)
        .args([
            "--profile",
            "standard-local-s4",
            "--workspace-profile",
            "cargo-root-package-v1",
            "--manifest-profile",
            "cargo-manifest-facts-v1",
        ])
        .args(rust_arguments)
        .arg("--store")
        .arg(&repository.store)
        .args(["--format", "json"])
        .output()
        .expect("launch invalid R12 selector matrix")
}

fn generated_markdown(root: &Path) -> String {
    let mut paths = Vec::new();
    collect_markdown_paths(root, &mut paths);
    paths.sort();
    paths.into_iter().fold(String::new(), |mut content, path| {
        content.push_str(&fs::read_to_string(path).expect("read R12 generated Markdown"));
        content
    })
}

fn collect_markdown_paths(root: &Path, paths: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(root).expect("read R12 documentation root") {
        let path = entry.expect("read R12 documentation entry").path();
        if path.is_dir() {
            collect_markdown_paths(&path, paths);
        } else if path.extension().and_then(std::ffi::OsStr::to_str) == Some("md") {
            paths.push(path);
        }
    }
}

fn hex_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut encoded, byte| {
            write!(&mut encoded, "{byte:02x}").expect("write SHA-256 digest");
            encoded
        })
}
