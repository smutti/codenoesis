mod support;

use std::fs;
use std::process::Output;

use serde_json::Value;

use support::parse_single_document;
use support::s4_r17::{
    MaterializedFunctionContextRepository, ROOT_CALLABLE_ID, expected_function_context,
};

fn assert_success(output: &Output, label: &str) {
    assert!(
        output.status.success(),
        "{label} failed: status={:?}; stdout={}; stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "{label} stderr must be empty");
}

fn ids(records: &Value) -> Vec<&str> {
    records
        .as_array()
        .expect("reviewed context family")
        .iter()
        .map(|record| record["id"].as_str().expect("reviewed context ID"))
        .collect()
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the public R17 journey keeps its reviewed scan-through-explorer oracle in one test"
)]
fn e2e_fr_ctx_001_function_context_and_navigation() {
    let repository = MaterializedFunctionContextRepository::fixture();
    let scan = repository.scan();
    assert_success(&scan, "R17 fixture scan");
    let snapshot = parse_single_document(&scan.stdout);
    assert_eq!(
        snapshot["schema_version"],
        "codenoesis.repository-snapshot/v18"
    );
    assert_eq!(
        snapshot["semantic_hash"]["value"],
        "8d7a7ee0b187a8e66024ef5e7e80b9395f53cee4816f00cfb3168acf5788661f"
    );

    let docs = repository.docs();
    assert_success(&docs, "R17 documentation");

    let default_before = repository.query(ROOT_CALLABLE_ID);
    assert_success(&default_before, "R17 selector-absent query before context");
    assert_eq!(
        parse_single_document(&default_before.stdout)["schema_version"],
        "codenoesis.local-query-result/v13"
    );

    let context_output = repository.query_context(ROOT_CALLABLE_ID);
    assert_success(&context_output, "R17 function-context query");
    let context = parse_single_document(&context_output.stdout);
    let oracle = expected_function_context();
    assert_eq!(context["schema_version"], oracle["context_schema_version"]);
    assert_eq!(context["authority"], oracle["authority"]);
    assert_eq!(context["source"], oracle["source"]);
    assert_eq!(context["display_signature"], oracle["display_signature"]);
    assert_eq!(context["callable"]["id"], oracle["callable"]["id"]);
    assert_eq!(context["callable"]["kind"], oracle["callable"]["kind"]);
    assert_eq!(context["callable"]["name"], oracle["callable"]["name"]);
    assert_eq!(context["owner"]["id"], oracle["owner"]["id"]);
    assert_eq!(context["signature"]["id"], oracle["signature"]["id"]);
    assert_eq!(
        context["signature"]["properties"]["return_type"],
        oracle["signature"]["return_type"]
    );
    assert_eq!(
        context["signature"]["properties"]["where_clause"],
        oracle["signature"]["where_clause"]
    );
    for (actual, expected) in context["parameters"]
        .as_array()
        .expect("context parameters")
        .iter()
        .zip(oracle["parameters"].as_array().expect("oracle parameters"))
    {
        assert_eq!(actual["id"], expected["id"]);
        assert_eq!(actual["name"], expected["name"]);
        assert_eq!(actual["ordinal"], expected["ordinal"]);
        assert_eq!(actual["properties"]["pattern"], expected["pattern"]);
        assert_eq!(
            actual["properties"]["declared_type"],
            expected["declared_type"]
        );
        assert_eq!(
            actual["properties"]["receiver_state"],
            expected["receiver_state"]
        );
    }
    assert_eq!(
        ids(&context["relationships"]),
        oracle["relationship_ids"]
            .as_array()
            .expect("oracle relationship IDs")
            .iter()
            .map(|value| value.as_str().expect("oracle relationship ID"))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        ids(&context["claims"]),
        oracle["claim_ids"]
            .as_array()
            .expect("oracle claim IDs")
            .iter()
            .map(|value| value.as_str().expect("oracle claim ID"))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        ids(&context["evidence"]),
        oracle["evidence_ids"]
            .as_array()
            .expect("oracle evidence IDs")
            .iter()
            .map(|value| value.as_str().expect("oracle evidence ID"))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        ids(&context["diagnostics"]),
        oracle["diagnostic_ids"]
            .as_array()
            .expect("oracle diagnostic IDs")
            .iter()
            .map(|value| value.as_str().expect("oracle diagnostic ID"))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        ids(&context["coverage_gaps"]),
        oracle["coverage_gap_ids"]
            .as_array()
            .expect("oracle coverage IDs")
            .iter()
            .map(|value| value.as_str().expect("oracle coverage ID"))
            .collect::<Vec<_>>()
    );
    assert_eq!(context["derivations"], Value::Array(Vec::new()));
    assert_eq!(
        context["navigation"]
            .as_array()
            .expect("context navigation")
            .iter()
            .map(|entry| entry["id"].as_str().expect("navigation ID"))
            .collect::<Vec<_>>(),
        oracle["navigation_ids"]
            .as_array()
            .expect("oracle navigation IDs")
            .iter()
            .map(|value| value.as_str().expect("oracle navigation ID"))
            .collect::<Vec<_>>()
    );
    assert_eq!(context["limitations"], oracle["limitations"]);

    let default_after = repository.query(ROOT_CALLABLE_ID);
    assert_success(&default_after, "R17 selector-absent query after context");
    assert_eq!(default_after.stdout, default_before.stdout);

    let export = repository.export();
    assert_success(&export, "R17 PortableGraphV9 export");
    let portable_bytes = fs::read(repository.portable.join("portable-graph.json"))
        .expect("read R17 PortableGraphV9");
    assert_eq!(
        parse_single_document(&portable_bytes)["schema_version"],
        "codenoesis.portable-graph/v9"
    );

    let explore = repository.explore_context();
    assert_success(&explore, "R17 LocalExplorerV10");
    let manifest = parse_single_document(&explore.stdout);
    assert_eq!(manifest["schema_version"], "codenoesis.local-explorer/v10");
    assert_eq!(manifest["profile"], "rust-function-context-v1");
    assert_eq!(
        manifest["portable_graph"]["schema_version"],
        "codenoesis.portable-graph/v9"
    );
    assert!(repository.explorer.join("index.html").is_file());
    assert!(!repository.build_sentinel().exists());
}
