mod support;

use std::collections::BTreeMap;
use std::fs;
use std::process::Output;

use serde_json::Value;

use support::parse_single_document;
use support::s4_r11::{
    BOUNDARY_EVIDENCE_ID, BOUNDARY_ID, MaterializedCallableBoundaryRepository,
    expected_bound_boundaries, expected_unbound_boundaries,
};

const HETEROGENEOUS_CFG_METHOD_ALTERNATIVES_SOURCE: &[u8] = br"pub struct Client;
pub struct Context;

impl Client {
    #[cfg(unix)]
    fn try_start_clipboard(&self, _ctx: Option<Context>) {}

    #[cfg(windows)]
    fn try_start_clipboard(&self, _value: Option<()>) {}
}
";
const EXTERNAL_WORKSPACE_MEMBER_MANIFEST: &[u8] = br#"[workspace]
members = ["external/nested-model"]

[package]
name = "rust-callable-semantics-fixture"
version = "0.1.0"
edition = "2024"
build = "build.rs"

[lib]
path = "src/lib.rs"

[features]
experimental = []
"#;
const UNSUPPORTED_KEY_GITMODULES: &[u8] = br#"[submodule "nested-model"]
	path = external/nested-model
	url = https://credential-canary.invalid/private/nested-model.git
	branch = main
"#;

#[test]
fn conf_nfr_det_001_r11_viewer_checkout_transport_is_platform_neutral() {
    assert_eq!(normalize_lf(b"first\r\nsecond\r\n"), b"first\nsecond\n");
    assert_eq!(normalize_lf(b"first\nsecond\n"), b"first\nsecond\n");
}

#[test]
#[allow(clippy::too_many_lines)]
fn e2e_fr_ext_012_k1_gitlink_boundaries_complete_local_journey() {
    let repository = MaterializedCallableBoundaryRepository::fixture();
    let scan = repository.scan_unbound();
    assert_success(&scan, "R11 unbound scan");
    assert!(scan.stderr.is_empty());
    let snapshot = parse_single_document(&scan.stdout);
    assert_eq!(
        snapshot["schema_version"],
        "codenoesis.repository-snapshot/v13"
    );
    assert_eq!(
        snapshot["semantic"]["configuration"]["schema_version"],
        "codenoesis.configuration/v10"
    );
    assert_eq!(
        snapshot["semantic"]["configuration"]["repository_boundary_profile"],
        "local-gitlinks-v1"
    );
    assert_eq!(
        snapshot["semantic"]["knowledge_graph"]["schema_version"],
        "codenoesis.knowledge-graph/v10"
    );
    assert_eq!(
        snapshot["semantic"]["repository_boundaries"],
        expected_unbound_boundaries()
    );

    let graph = &snapshot["semantic"]["knowledge_graph"];
    assert_eq!(
        callable_counts(graph),
        BTreeMap::from([
            ("rust.call_site", 9),
            ("rust.callable_signature", 9),
            ("rust.control", 11),
            ("rust.declared_value", 10),
            ("rust.local_binding", 4),
            ("rust.parameter", 15),
        ])
    );

    assert_success(&repository.docs(), "R11 docs");
    let overview = fs::read_to_string(repository.documents.join("overview.md"))
        .expect("read R11 boundary documentation");
    assert!(overview.contains("## Repository boundaries"));
    assert!(overview.contains("declared_unbound"));
    assert!(overview.contains("Nested source is not analyzed"));
    let signature_id = graph["entities"]
        .as_array()
        .expect("R11 graph entities")
        .iter()
        .find(|entity| entity["kind"] == "rust.callable_signature")
        .and_then(|entity| entity["id"].as_str())
        .expect("R11 callable signature");
    let declaration_id =
        snapshot["semantic"]["repository_boundaries"]["declarations"][0]["declaration_id"]
            .as_str()
            .expect("R11 declaration ID");
    let gap_id = snapshot["semantic"]["repository_boundaries"]["coverage_gaps"][0]["gap_id"]
        .as_str()
        .expect("R11 coverage-gap ID");
    for requested_id in [
        signature_id,
        BOUNDARY_ID,
        declaration_id,
        BOUNDARY_EVIDENCE_ID,
        gap_id,
    ] {
        let query = repository.query(requested_id);
        assert_success(&query, "R11 exact-ID query");
        let result = parse_single_document(&query.stdout);
        assert_eq!(result["schema_version"], "codenoesis.local-query-result/v8");
        if requested_id != signature_id {
            assert!(
                !result["document_statements"]
                    .as_array()
                    .expect("R11 linked boundary statements")
                    .is_empty()
            );
        }
    }

    let exported = repository.export();
    assert_success(&exported, "R11 export");
    let portable_bytes =
        fs::read(repository.portable.join("portable-graph.json")).expect("read R11 portable");
    assert_eq!(exported.stdout, portable_bytes);
    let portable = parse_single_document(&portable_bytes);
    assert_eq!(portable["schema_version"], "codenoesis.portable-graph/v4");
    assert_eq!(
        portable["source_snapshot"]["schema_version"],
        "codenoesis.repository-snapshot/v13"
    );
    assert_eq!(
        portable["repository_boundaries"],
        expected_unbound_boundaries()
    );
    assert_private(&portable);

    let explored = repository.explore();
    assert_success(&explored, "R11 explore");
    let manifest = parse_single_document(&explored.stdout);
    assert_eq!(manifest["schema_version"], "codenoesis.local-explorer/v4");
    assert_eq!(manifest["security"]["network"], false);
    assert_eq!(manifest["security"]["dynamic_code"], false);
    let viewer = fs::read(repository.explorer.join("index.html")).expect("read R11 viewer");
    let immutable_viewer = normalize_lf(
        &fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/s4/k1/index.html"))
            .expect("read immutable K1 viewer"),
    );
    assert_eq!(viewer, immutable_viewer);
    assert!(!repository.build_sentinel().exists());

    let bound = MaterializedCallableBoundaryRepository::fixture();
    let bound_scan = bound.scan_bound();
    assert_success(&bound_scan, "R11 explicitly-bound scan");
    let bound_snapshot = parse_single_document(&bound_scan.stdout);
    assert_eq!(
        bound_snapshot["semantic"]["repository_boundaries"],
        expected_bound_boundaries()
    );
    assert!(!bound.build_sentinel().exists());
}

#[test]
fn ft_fr_cli_001_r11_forbidden_composition_fails_before_store_creation() {
    let repository = MaterializedCallableBoundaryRepository::fixture();
    let output = repository.scan_with_extra_options(&[
        "--compiler-index-profile",
        "scip-index-v1",
        "--compiler-index-binding",
        "missing.scip",
    ]);
    assert_eq!(output.status.code(), Some(11));
    assert!(output.stdout.is_empty());
    let error = parse_single_document(&output.stderr);
    assert_eq!(error["schema_version"], "codenoesis.error/v18");
    assert_eq!(error["code"], "input.unsupported_rust_callable_composition");
    assert!(!repository.store.exists());
}

#[test]
fn reg_fr_ext_012_r11_output_capacity_is_non_semantic() {
    let standard_repository = MaterializedCallableBoundaryRepository::fixture();
    let standard = standard_repository.scan_unbound();
    assert_success(&standard, "R11 standard-capacity scan");
    let large_repository = MaterializedCallableBoundaryRepository::fixture();
    let large = large_repository.scan_unbound_with_large_output_capacity();
    assert_success(&large, "R11 large-capacity scan");
    let standard = parse_single_document(&standard.stdout);
    let large = parse_single_document(&large.stdout);
    assert_eq!(standard["semantic"], large["semantic"]);
    assert_eq!(standard["semantic_hash"], large["semantic_hash"]);
}

#[test]
fn sec_nfr_sec_001_r11_unbound_nested_worktree_has_no_semantic_authority() {
    let absent_repository = MaterializedCallableBoundaryRepository::fixture();
    let absent = absent_repository.scan_unbound();
    assert_success(&absent, "R11 absent nested worktree scan");
    let present_repository = MaterializedCallableBoundaryRepository::fixture();
    present_repository.materialize_unbound_nested_worktree_canary();
    let present = present_repository.scan_unbound();
    assert_success(&present, "R11 present unbound nested worktree scan");
    let absent_snapshot = parse_single_document(&absent.stdout);
    let present_snapshot = parse_single_document(&present.stdout);
    assert_eq!(absent_snapshot["semantic"], present_snapshot["semantic"]);
    assert_eq!(
        absent_snapshot["semantic_hash"],
        present_snapshot["semantic_hash"]
    );
    assert!(
        !present
            .stdout
            .windows(b"nested-source-canary".len())
            .any(|window| window == b"nested-source-canary")
    );
    assert!(!present_repository.build_sentinel().exists());
}

#[test]
fn diag_fr_ext_012_r11_semantic_identity_conflict_is_deterministic_error_v18() {
    let mut repository = MaterializedCallableBoundaryRepository::fixture();
    repository.replace_source_and_commit(HETEROGENEOUS_CFG_METHOD_ALTERNATIVES_SOURCE);
    let first = repository.scan_unbound();
    let second = repository.scan_unbound();
    assert_eq!(first.status.code(), Some(11));
    assert_eq!(second.status.code(), Some(11));
    assert!(first.stdout.is_empty());
    assert!(second.stdout.is_empty());
    assert_eq!(first.stderr, second.stderr);
    let error = parse_single_document(&first.stderr);
    assert_eq!(error["schema_version"], "codenoesis.error/v18");
    assert_eq!(error["code"], "extraction.rust_semantic_identity_conflict");
    assert_eq!(error["context"]["member_kind"], "rust.method");
    assert_eq!(error["context"]["normalized_member"], "try_start_clipboard");
    assert!(!repository.store.exists());
    assert!(!repository.build_sentinel().exists());
}

#[test]
fn e2e_fr_qry_001_r11_links_external_workspace_member_and_declaration_gap() {
    let mut repository = MaterializedCallableBoundaryRepository::fixture();
    repository.replace_root_manifest_and_commit(EXTERNAL_WORKSPACE_MEMBER_MANIFEST);
    repository.replace_gitmodules_and_commit(UNSUPPORTED_KEY_GITMODULES);
    let scan = repository.scan_unbound();
    assert_success(&scan, "R11 external-member scan");
    let snapshot = parse_single_document(&scan.stdout);
    let report = &snapshot["semantic"]["repository_boundaries"];
    let boundary_id = report["boundaries"][0]["boundary_id"]
        .as_str()
        .expect("R11 dynamic boundary ID");
    let declaration_id = report["declarations"][0]["declaration_id"]
        .as_str()
        .expect("R11 dynamic declaration ID");
    assert!(
        report["coverage_gaps"]
            .as_array()
            .expect("R11 dynamic gaps")
            .iter()
            .any(|gap| {
                gap["code"] == "boundary.gitmodules_key_unsupported"
                    && gap["subject_id"] == declaration_id
            })
    );
    assert_success(&repository.docs(), "R11 external-member docs");
    let overview = fs::read_to_string(repository.documents.join("overview.md"))
        .expect("read R11 external-member docs");
    assert!(overview.contains("boundary.gitmodules_key_unsupported"));
    let query = repository.query(declaration_id);
    assert_success(&query, "R11 declaration query");
    let result = parse_single_document(&query.stdout);
    assert!(
        result["linked_k1_entities"]
            .as_array()
            .expect("R11 linked workspace members")
            .iter()
            .any(|member| {
                member["path"] == "external/nested-model"
                    && member["external_boundary_id"] == boundary_id
            })
    );
    assert!(
        result["linked_boundary_coverage_gaps"]
            .as_array()
            .expect("R11 linked declaration gaps")
            .iter()
            .any(|gap| gap["code"] == "boundary.gitmodules_key_unsupported")
    );
    let exported = repository.export();
    assert_success(&exported, "R11 external-member export");
    let portable = parse_single_document(&exported.stdout);
    assert_private(&portable);
    assert!(
        !exported
            .stdout
            .windows(b"credential-canary".len())
            .any(|window| window == b"credential-canary")
    );
    assert!(!repository.build_sentinel().exists());
}

fn callable_counts(graph: &Value) -> BTreeMap<&str, u64> {
    graph["entities"]
        .as_array()
        .expect("R11 graph entities")
        .iter()
        .filter_map(|entity| entity["kind"].as_str())
        .filter(|kind| {
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
        .fold(BTreeMap::new(), |mut counts, kind| {
            *counts.entry(kind).or_insert(0) += 1;
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

fn normalize_lf(bytes: &[u8]) -> Vec<u8> {
    let mut normalized = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\r' && bytes.get(index + 1) == Some(&b'\n') {
            normalized.push(b'\n');
            index += 2;
        } else {
            normalized.push(bytes[index]);
            index += 1;
        }
    }
    normalized
}
