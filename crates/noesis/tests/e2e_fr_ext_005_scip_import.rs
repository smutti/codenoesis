mod support;

use std::collections::BTreeMap;

use serde_json::Value;

use support::s4_r7::{MaterializedCompilerIndexRepository, expected_overlay};

const PRE_R7_STDERR: &[u8] = b"{\"code\":\"input.invalid_revision\",\"context\":{},\"message\":\"invalid revision\",\"retryable\":false,\"schema_version\":\"codenoesis.error/v4\",\"stage\":\"input\"}\n";

#[test]
fn e2e_fr_ext_005_revision_bound_scip_import() {
    let repository = MaterializedCompilerIndexRepository::fixture();
    assert!(!repository.build_sentinel().exists());
    assert!(!repository.indexer_sentinel().exists());
    let output = repository.scan();

    if output.status.code() == Some(2) {
        assert!(output.stdout.is_empty(), "pre-R7 stdout changed");
        assert_eq!(output.stderr, PRE_R7_STDERR, "pre-R7 stderr changed");
        assert!(!repository.store.exists(), "pre-R7 store was created");
        assert!(
            !repository.documents.exists(),
            "pre-R7 documents root was created"
        );
        assert!(
            !repository.build_sentinel().exists(),
            "pre-R7 subject executed build.rs"
        );
        assert!(
            !repository.indexer_sentinel().exists(),
            "pre-R7 subject executed an indexer"
        );
        panic!(
            "expected RepositorySnapshotV10 success; observed approved unknown compiler-index selector Red"
        );
    }

    assert!(
        output.status.success(),
        "R7 scan failed: status={:?}, stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "successful R7 stderr changed");
    assert!(
        !repository.build_sentinel().exists(),
        "R7 subject executed build.rs"
    );
    assert!(
        !repository.indexer_sentinel().exists(),
        "R7 subject executed an indexer"
    );

    let snapshot: Value =
        serde_json::from_slice(&output.stdout).expect("parse RepositorySnapshotV10");
    assert_eq!(
        snapshot["schema_version"],
        "codenoesis.repository-snapshot/v10"
    );
    assert_eq!(
        snapshot["semantic"]["configuration"]["compiler_index_profile"],
        "scip-rust-v0.9.0-import-v1"
    );
    assert_eq!(
        snapshot["semantic"]["knowledge_graph"]["ontology_version"],
        "codenoesis.ontology/rust/v7"
    );
    assert_reviewed_overlay(&snapshot);
}

fn assert_reviewed_overlay(snapshot: &Value) {
    let expected = expected_overlay();
    let graph = &snapshot["semantic"]["knowledge_graph"];
    let compiler_index = &graph["compiler_index"];
    assert_eq!(
        compiler_index["artifact_sha256"],
        expected["artifact_sha256"]
    );

    let symbols = graph["entities"]
        .as_array()
        .expect("R7 graph entities")
        .iter()
        .filter(|entity| entity["kind"] == "compiler.symbol")
        .collect::<Vec<_>>();
    assert_eq!(symbols.len(), 15, "reviewed compiler-symbol count changed");
    let mut states = BTreeMap::<&str, usize>::new();
    for symbol in symbols {
        let state = symbol["properties"]["binding_state"]
            .as_str()
            .expect("compiler binding state");
        *states.entry(state).or_default() += 1;
    }
    for (state, count) in expected["binding_state_counts"]
        .as_object()
        .expect("reviewed binding-state counts")
    {
        assert_eq!(
            states.get(state.as_str()).copied(),
            count
                .as_u64()
                .map(|value| usize::try_from(value).expect("reviewed count fits usize"))
        );
    }

    let mut relationships = BTreeMap::<&str, usize>::new();
    for relationship in graph["relationships"]
        .as_array()
        .expect("R7 graph relationships")
    {
        let kind = relationship["kind"].as_str().expect("relationship kind");
        if matches!(
            kind,
            "RESOLVES_TO" | "REFERENCES" | "IMPLEMENTS" | "TYPE_DEFINITION"
        ) {
            *relationships.entry(kind).or_default() += 1;
        }
    }
    assert_eq!(relationships.get("RESOLVES_TO"), Some(&1));
    assert_eq!(relationships.get("REFERENCES"), Some(&1));
    assert_eq!(relationships.get("IMPLEMENTS"), Some(&2));
    assert_eq!(relationships.get("TYPE_DEFINITION"), Some(&1));
}
