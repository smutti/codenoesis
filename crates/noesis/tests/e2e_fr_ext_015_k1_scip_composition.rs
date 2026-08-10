mod support;

use std::collections::BTreeMap;
use std::fs;
use std::process::Output;

use serde_json::Value;
use sha2::{Digest as _, Sha256};

use support::parse_single_document;
use support::s4_r13::{MaterializedCallableScipRepository, expected_composition};

const EXPECTED_RED_STDERR_SHA256: &str =
    "2573e0f364350b300218c6d1940e6eb33f4f0bc70b7ba92dd9b2821f5bf97013";

#[test]
#[allow(clippy::too_many_lines)]
fn e2e_fr_ext_015_k1_scip_composition_complete_local_journey() {
    let repository = MaterializedCallableScipRepository::fixture();
    assert!(!repository.build_sentinel().exists());
    assert!(!repository.indexer_sentinel().exists());
    let scan = repository.scan();

    if scan.status.code() == Some(11) {
        assert!(scan.stdout.is_empty(), "R13 expected-Red stdout changed");
        assert_eq!(scan.stderr.len(), 309, "R13 expected-Red length changed");
        assert_eq!(
            hex_sha256(&scan.stderr),
            EXPECTED_RED_STDERR_SHA256,
            "R13 expected-Red digest changed"
        );
        let error = parse_single_document(&scan.stderr);
        assert_eq!(error["schema_version"], "codenoesis.error/v16");
        assert_eq!(error["code"], "input.unsupported_rust_callable_composition");
        assert_eq!(error["context"]["profile"], "rust-callable-semantics-v1");
        assert_eq!(error["context"]["required_lineage"], "r6_source_only");
        assert_eq!(error["context"]["compiler_index_composition"], false);
        assert!(
            !repository.store().exists(),
            "R13 expected Red mutated store"
        );
        assert!(!repository.build_sentinel().exists());
        assert!(!repository.indexer_sentinel().exists());
        panic!("expected RepositorySnapshotV15 success; observed frozen composition Red");
    }

    assert_success(&scan, "R13 callable/SCIP scan");
    assert!(scan.stderr.is_empty());
    let snapshot = parse_single_document(&scan.stdout);
    assert_eq!(
        snapshot["schema_version"],
        "codenoesis.repository-snapshot/v15"
    );
    assert_eq!(
        snapshot["semantic"]["configuration"]["schema_version"],
        "codenoesis.configuration/v12"
    );
    assert_eq!(
        snapshot["semantic"]["configuration"]["rust_callable_profile"],
        "rust-callable-semantics-v1"
    );
    assert_eq!(
        snapshot["semantic"]["configuration"]["compiler_index_profile"],
        "scip-rust-v0.9.0-import-v1"
    );

    let expected = expected_composition();
    let graph = &snapshot["semantic"]["knowledge_graph"];
    assert_eq!(graph["schema_version"], "codenoesis.knowledge-graph/v12");
    assert_eq!(graph["ontology_version"], "codenoesis.ontology/rust/v12");
    for (family, expected_count) in expected["family_counts"]
        .as_object()
        .expect("R13 expected family counts")
    {
        assert_eq!(
            graph[family.as_str()].as_array().map_or(0, Vec::len),
            usize::try_from(expected_count.as_u64().expect("R13 family count"))
                .expect("R13 family count fits usize"),
            "R13 graph family count {family}"
        );
    }
    assert_eq!(
        count_by(graph, "relationships", "kind").get("HAS_COMPILER_SYMBOL"),
        Some(&5)
    );

    let entities = graph["entities"].as_array().expect("R13 graph entities");
    let relationships = graph["relationships"]
        .as_array()
        .expect("R13 graph relationships");
    for join in expected["joins"].as_array().expect("R13 expected joins") {
        let callable_id = join["source_callable_id"]
            .as_str()
            .expect("R13 callable ID");
        let signature_id = join["signature_id"].as_str().expect("R13 signature ID");
        let compiler_id = join["compiler_symbol_id"]
            .as_str()
            .expect("R13 compiler ID");
        assert!(entities.iter().any(|entity| entity["id"] == callable_id));
        assert!(entities.iter().any(|entity| entity["id"] == signature_id));
        assert!(entities.iter().any(|entity| entity["id"] == compiler_id));
        assert_eq!(
            relationships
                .iter()
                .filter(|relationship| {
                    relationship["kind"] == "HAS_SIGNATURE"
                        && relationship["source"] == callable_id
                        && relationship["target"] == signature_id
                })
                .count(),
            1
        );
        let relationship = relationships
            .iter()
            .find(|relationship| relationship["id"] == join["relationship_id"])
            .expect("R13 join relationship");
        assert_eq!(relationship["kind"], "HAS_COMPILER_SYMBOL");
        assert_eq!(relationship["source"], callable_id);
        assert_eq!(relationship["target"], compiler_id);
        assert_eq!(relationship["evidence_ids"], join["evidence_ids"]);
    }
    assert_eq!(
        graph["callable_compiler_join_index"]["schema_version"],
        "codenoesis.callable-compiler-join-index/v1"
    );
    assert_eq!(
        graph["callable_compiler_join_index"]["joins"],
        expected["joins"]
    );
    assert_eq!(
        graph["callable_semantics_index"]["unresolved_call_site_ids"]
            .as_array()
            .expect("R13 unresolved call IDs")
            .len(),
        2
    );

    assert_success(&repository.docs(), "R13 documentation generation");
    let documentation = generated_markdown(repository.documents());
    assert!(documentation.contains("Compiler symbol correspondence"));
    assert!(documentation.contains("does not prove runtime behavior"));
    let first = &expected["joins"][0];
    for requested_id in [
        first["source_callable_id"]
            .as_str()
            .expect("R13 callable query ID"),
        first["signature_id"]
            .as_str()
            .expect("R13 signature query ID"),
        first["compiler_symbol_id"]
            .as_str()
            .expect("R13 compiler query ID"),
        first["relationship_id"]
            .as_str()
            .expect("R13 join query ID"),
    ] {
        let query = repository.query(requested_id);
        assert_success(&query, "R13 exact-ID query");
        assert_eq!(
            parse_single_document(&query.stdout)["schema_version"],
            "codenoesis.local-query-result/v10"
        );
    }

    let export = repository.export();
    assert_success(&export, "R13 portable export");
    let portable_bytes = fs::read(repository.portable().join("portable-graph.json"))
        .expect("read R13 portable graph");
    assert_eq!(export.stdout, portable_bytes);
    let portable = parse_single_document(&portable_bytes);
    assert_eq!(portable["schema_version"], "codenoesis.portable-graph/v6");
    assert_eq!(
        portable["source_snapshot"]["schema_version"],
        "codenoesis.repository-snapshot/v15"
    );
    assert_private(&portable);

    let explore = repository.explore();
    assert_success(&explore, "R13 local explorer");
    let manifest = parse_single_document(&explore.stdout);
    assert_eq!(manifest["schema_version"], "codenoesis.local-explorer/v6");
    assert_eq!(manifest["security"]["network"], false);
    assert_eq!(manifest["security"]["dynamic_code"], false);
    let viewer = fs::read(repository.explorer().join("index.html")).expect("read R13 viewer");
    let immutable_viewer = normalize_lf(
        &fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/s4/k1/index.html"))
            .expect("read immutable K1 viewer"),
    );
    assert_eq!(viewer, immutable_viewer);
    assert!(!repository.build_sentinel().exists());
    assert!(!repository.indexer_sentinel().exists());
}

fn count_by<'a>(graph: &'a Value, family: &str, field: &str) -> BTreeMap<&'a str, u64> {
    graph[family]
        .as_array()
        .expect("R13 graph family")
        .iter()
        .filter_map(|record| record[field].as_str())
        .fold(BTreeMap::new(), |mut counts, value| {
            *counts.entry(value).or_insert(0) += 1;
            counts
        })
}

fn generated_markdown(root: &std::path::Path) -> String {
    let mut paths = fs::read_dir(root)
        .expect("read R13 documentation root")
        .map(|entry| entry.expect("read R13 documentation entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "md"))
        .collect::<Vec<_>>();
    paths.sort();
    paths
        .into_iter()
        .map(|path| fs::read_to_string(path).expect("read R13 documentation"))
        .collect::<Vec<_>>()
        .join("\n")
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
                        | "environment"
                        | "arguments"
                ));
                assert_private(nested);
            }
        }
        Value::Array(values) => values.iter().for_each(assert_private),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn normalize_lf(bytes: &[u8]) -> Vec<u8> {
    String::from_utf8_lossy(bytes)
        .replace("\r\n", "\n")
        .into_bytes()
}

fn hex_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn assert_success(output: &Output, subject: &str) {
    assert!(
        output.status.success(),
        "{subject} failed: status={:?}, stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}
