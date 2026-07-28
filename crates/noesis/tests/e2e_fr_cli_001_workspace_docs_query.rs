mod support;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Output;

use serde_json::Value;
use support::s4::{
    DOCUMENT_QUERY_ID, ENTITY_QUERY_ID, MaterializedRepository, UNKNOWN_QUERY_ID, docs,
    fixture_root, query, scan,
};
use support::{parse_single_document, read_json};

#[test]
fn e2e_fr_cli_001_workspace_docs_query() {
    let repository = MaterializedRepository::revision_a();
    let scan_output = scan(&repository);
    assert_scan_success_or_expected_red(&repository, &scan_output);

    let snapshot = parse_single_document(&scan_output.stdout);
    assert_snapshot(&snapshot);

    let docs_output = docs(&repository);
    assert_success(&docs_output, "S4 docs");
    let expected_manifest = read_json(&fixture_root().join("expected-documentation-manifest.json"));
    assert_eq!(
        parse_single_document(&docs_output.stdout),
        expected_manifest,
        "docs stdout manifest changed"
    );
    assert_documentation_bytes(&repository.documents);

    let first_generation = owned_documentation_bytes(&repository.documents);
    let replay = docs(&repository);
    assert_success(&replay, "S4 docs replay");
    assert_eq!(
        parse_single_document(&replay.stdout),
        expected_manifest,
        "replayed docs manifest changed"
    );
    assert_eq!(
        owned_documentation_bytes(&repository.documents),
        first_generation,
        "replayed documentation bytes changed"
    );

    let entity_query = query(&repository, ENTITY_QUERY_ID);
    assert_success(&entity_query, "S4 entity query");
    assert_eq!(
        parse_single_document(&entity_query.stdout),
        read_json(&fixture_root().join("expected-query-entity.json"))
    );

    let document_query = query(&repository, DOCUMENT_QUERY_ID);
    assert_success(&document_query, "S4 document query");
    assert_eq!(
        parse_single_document(&document_query.stdout),
        read_json(&fixture_root().join("expected-query-document.json"))
    );

    let unknown_query = query(&repository, UNKNOWN_QUERY_ID);
    assert_eq!(unknown_query.status.code(), Some(14));
    assert!(unknown_query.stdout.is_empty());
    assert_eq!(
        parse_single_document(&unknown_query.stderr),
        read_json(&fixture_root().join("expected-error-unknown-id.json"))
    );
}

fn assert_scan_success_or_expected_red(repository: &MaterializedRepository, output: &Output) {
    if output.status.success() {
        assert!(output.stderr.is_empty(), "successful scan stderr changed");
        return;
    }

    assert_eq!(
        output.status.code(),
        Some(2),
        "pre-S4 scan must reject only the unsupported profile"
    );
    assert!(output.stdout.is_empty(), "failed scan stdout must be empty");
    assert!(
        !repository.store.exists(),
        "expected Red must not create the store"
    );
    assert!(
        !repository.documents.exists(),
        "expected Red must not create the documents root"
    );
    let error = parse_single_document(&output.stderr);
    assert_eq!(error["code"], "input.invalid_profile");
    assert_eq!(
        error["schema_version"], "codenoesis.error/v4",
        "merged S3 must provide the ratified inherited V4 Red"
    );
    panic!("expected S4 scan success; observed the approved unsupported-profile Red");
}

fn assert_snapshot(snapshot: &Value) {
    let expected = read_json(&fixture_root().join("expected-graph-summary.json"));
    assert_eq!(
        snapshot["schema_version"],
        expected["snapshot_schema_version"]
    );
    assert_eq!(
        snapshot["semantic"]["repository"]["identity"],
        expected["repository_identity"]
    );
    assert_eq!(
        snapshot["semantic"]["repository"]["commit_oid"],
        expected["commit_oid"]
    );
    assert_eq!(
        snapshot["semantic"]["ontology_version"],
        expected["ontology_version"]
    );
    assert_eq!(
        snapshot["semantic_hash"]["value"],
        expected["semantic_hash"]
    );
    let graph = &snapshot["semantic"]["knowledge_graph"];
    for (field, expected_count) in [
        ("entities", &expected["counts"]["entities"]),
        ("relationships", &expected["counts"]["relationships"]),
        ("claims", &expected["counts"]["claims"]),
        ("evidence", &expected["counts"]["evidence"]),
        ("diagnostics", &expected["counts"]["diagnostics"]),
        ("coverage", &expected["counts"]["coverage_gaps"]),
    ] {
        assert_eq!(
            graph[field]
                .as_array()
                .unwrap_or_else(|| panic!("graph {field} array"))
                .len(),
            expected_count
                .as_u64()
                .and_then(|value| usize::try_from(value).ok())
                .expect("reviewed graph count"),
            "graph {field} count changed"
        );
    }
}

fn assert_documentation_bytes(documents: &Path) {
    let golden = fixture_root().join("expected-docs");
    for relative in [
        "overview.md",
        "modules/app.md",
        "modules/model-item.md",
        "modules/model.md",
    ] {
        assert_eq!(
            fs::read(documents.join(relative))
                .unwrap_or_else(|error| panic!("read generated {relative}: {error}")),
            fs::read(golden.join(relative))
                .unwrap_or_else(|error| panic!("read golden {relative}: {error}")),
            "generated document changed: {relative}"
        );
    }
    assert_eq!(
        read_json(&documents.join("manifest.json")),
        read_json(&fixture_root().join("expected-documentation-manifest.json"))
    );
}

fn owned_documentation_bytes(documents: &Path) -> BTreeMap<String, Vec<u8>> {
    [
        ".codenoesis-generated.json",
        "manifest.json",
        "overview.md",
        "modules/app.md",
        "modules/model-item.md",
        "modules/model.md",
    ]
    .into_iter()
    .map(|relative| {
        (
            relative.to_owned(),
            fs::read(documents.join(relative))
                .unwrap_or_else(|error| panic!("read owned {relative}: {error}")),
        )
    })
    .collect()
}

fn assert_success(output: &Output, journey: &str) {
    assert_eq!(
        output.status.code(),
        Some(0),
        "{journey} failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "{journey} stderr changed");
}
