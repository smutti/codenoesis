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
    let signature_id = graph["entities"]
        .as_array()
        .expect("R11 graph entities")
        .iter()
        .find(|entity| entity["kind"] == "rust.callable_signature")
        .and_then(|entity| entity["id"].as_str())
        .expect("R11 callable signature");
    for requested_id in [signature_id, BOUNDARY_ID, BOUNDARY_EVIDENCE_ID] {
        let query = repository.query(requested_id);
        assert_success(&query, "R11 exact-ID query");
        assert_eq!(
            parse_single_document(&query.stdout)["schema_version"],
            "codenoesis.local-query-result/v8"
        );
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
    assert_eq!(
        fs::read(repository.explorer.join("index.html")).expect("read R11 viewer"),
        fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/s4/k1/index.html"))
            .expect("read immutable K1 viewer")
    );
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

fn callable_counts<'a>(graph: &'a Value) -> BTreeMap<&'a str, u64> {
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
