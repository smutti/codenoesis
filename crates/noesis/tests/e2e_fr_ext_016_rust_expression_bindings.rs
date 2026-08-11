mod support;

use std::collections::BTreeMap;
use std::fs;
use std::process::Output;

use serde_json::Value;
use sha2::{Digest as _, Sha256};

use support::parse_single_document;
use support::s4_r14::{MaterializedExpressionBindingRepository, expected_expression_bindings};

const EXPECTED_RED_STDERR_SHA256: &str =
    "7f75f7a91f6af0328795f3fbd2729e69756beba2ebd642cc1f6401265662a2fe";

#[test]
fn conf_nfr_det_001_r14_viewer_checkout_transport_is_platform_neutral() {
    assert_eq!(normalize_lf(b"first\r\nsecond\r\n"), b"first\nsecond\n");
    assert_eq!(normalize_lf(b"first\nsecond\n"), b"first\nsecond\n");
}

#[test]
#[allow(clippy::too_many_lines)]
fn e2e_fr_ext_016_rust_expression_bindings_complete_local_journey() {
    let repository = MaterializedExpressionBindingRepository::fixture();
    assert!(!repository.build_sentinel().exists());
    let scan = repository.scan();

    if scan.status.code() == Some(2) {
        assert!(scan.stdout.is_empty(), "R14 expected-Red stdout changed");
        assert_eq!(scan.stderr.len(), 149, "R14 expected-Red length changed");
        assert_eq!(
            hex_sha256(&scan.stderr),
            EXPECTED_RED_STDERR_SHA256,
            "R14 expected-Red digest changed"
        );
        let error = parse_single_document(&scan.stderr);
        assert_eq!(error["schema_version"], "codenoesis.error/v4");
        assert_eq!(error["code"], "input.invalid_revision");
        assert_eq!(error["stage"], "input");
        assert!(
            !repository.store().exists(),
            "R14 expected Red mutated store"
        );
        assert!(!repository.build_sentinel().exists());
        panic!("expected RepositorySnapshotV16 success; observed frozen selector Red");
    }

    assert_success(&scan, "R14 expression-binding scan");
    assert!(scan.stderr.is_empty());
    let snapshot = parse_single_document(&scan.stdout);
    assert_eq!(
        snapshot["schema_version"],
        "codenoesis.repository-snapshot/v16"
    );
    assert_eq!(
        snapshot["semantic"]["configuration"]["schema_version"],
        "codenoesis.configuration/v13"
    );
    assert_eq!(
        snapshot["semantic"]["configuration"]["rust_expression_profile"],
        "rust-expression-bindings-v1"
    );
    assert_eq!(
        snapshot["semantic"]["pipeline_version"],
        "codenoesis.pipeline/s4-r14-v1"
    );

    let expected = expected_expression_bindings();
    let graph = &snapshot["semantic"]["knowledge_graph"];
    assert_eq!(graph["schema_version"], "codenoesis.knowledge-graph/v13");
    assert_eq!(graph["ontology_version"], "codenoesis.ontology/rust/v13");
    for (family, count) in expected["complete_counts"]
        .as_object()
        .expect("R14 complete counts")
    {
        assert_eq!(
            graph[family].as_array().map_or(0, Vec::len),
            usize::try_from(count.as_u64().expect("R14 family count"))
                .expect("R14 count fits usize"),
            "R14 graph family {family}"
        );
    }

    let actual_entities = graph["entities"].as_array().expect("R14 graph entities");
    let mut expected_entities = expected["expressions"]
        .as_array()
        .expect("R14 expressions")
        .iter()
        .chain(expected["arguments"].as_array().expect("R14 arguments"))
        .chain(expected["bindings"].as_array().expect("R14 bindings"))
        .cloned()
        .collect::<Vec<_>>();
    expected_entities.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
    let mut actual_additive = actual_entities
        .iter()
        .filter(|entity| {
            matches!(
                entity["kind"].as_str(),
                Some("rust.expression" | "rust.call_argument" | "rust.pattern_binding")
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    actual_additive.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
    assert_eq!(
        actual_additive, expected_entities,
        "R14 exact additive entities"
    );

    let kinds = expected["relationship_kind_counts"]
        .as_object()
        .expect("R14 relationship counts");
    let mut actual_relationships = graph["relationships"]
        .as_array()
        .expect("R14 graph relationships")
        .iter()
        .filter(|relationship| {
            relationship["kind"]
                .as_str()
                .is_some_and(|kind| kinds.contains_key(kind))
        })
        .cloned()
        .collect::<Vec<_>>();
    actual_relationships.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
    assert_eq!(
        Value::Array(actual_relationships.clone()),
        expected["relationships"],
        "R14 exact additive relationships"
    );
    assert_eq!(
        count_by(&actual_relationships, "kind").get("READS"),
        Some(&29)
    );
    assert_eq!(
        count_by(&actual_relationships, "kind").get("WRITES"),
        Some(&7)
    );
    assert_eq!(
        count_by(&actual_relationships, "kind").get("REPRESENTS_CALL_SITE"),
        Some(&9)
    );
    assert_eq!(
        count_by(
            graph["relationships"]
                .as_array()
                .expect("R14 all relationships"),
            "kind"
        )
        .get("CALLS"),
        Some(&4)
    );

    assert_eq!(
        graph["expression_binding_index"]["schema_version"],
        "codenoesis.expression-binding-index/v1"
    );
    assert_eq!(
        graph["expression_binding_index"]["expression_entity_ids"]
            .as_array()
            .expect("R14 expression index")
            .len(),
        73
    );
    assert_eq!(
        graph["expression_binding_index"]["argument_entity_ids"]
            .as_array()
            .expect("R14 argument index")
            .len(),
        8
    );
    assert_eq!(
        graph["expression_binding_index"]["binding_entity_ids"]
            .as_array()
            .expect("R14 binding index")
            .len(),
        23
    );

    assert_success(&repository.docs(), "R14 documentation generation");
    let documentation = generated_markdown(&repository.inner.documents);
    assert!(documentation.contains("Lexical expression and binding facts"));
    assert!(documentation.contains("syntax occurrence; not data flow or runtime behavior"));

    for requested_id in [
        expected["expressions"][0]["id"]
            .as_str()
            .expect("R14 expression ID"),
        expected["arguments"][0]["id"]
            .as_str()
            .expect("R14 argument ID"),
        expected["bindings"][0]["id"]
            .as_str()
            .expect("R14 binding ID"),
        expected["relationships"]
            .as_array()
            .expect("R14 relationships")
            .iter()
            .find(|value| value["kind"] == "READS")
            .and_then(|value| value["id"].as_str())
            .expect("R14 READS ID"),
    ] {
        let query = repository.query(requested_id);
        assert_success(&query, "R14 exact-ID query");
        assert_eq!(
            parse_single_document(&query.stdout)["schema_version"],
            "codenoesis.local-query-result/v11"
        );
    }

    let export = repository.export();
    assert_success(&export, "R14 portable export");
    let portable_bytes = fs::read(repository.inner.portable.join("portable-graph.json"))
        .expect("read R14 portable graph");
    assert_eq!(export.stdout, portable_bytes);
    let portable = parse_single_document(&portable_bytes);
    assert_eq!(portable["schema_version"], "codenoesis.portable-graph/v7");
    assert_eq!(
        portable["source_snapshot"]["schema_version"],
        "codenoesis.repository-snapshot/v16"
    );
    assert_eq!(portable["entities"], graph["entities"]);
    assert_eq!(portable["relationships"], graph["relationships"]);
    assert_private(&portable);

    let explore = repository.explore();
    assert_success(&explore, "R14 local explorer");
    let manifest = parse_single_document(&explore.stdout);
    assert_eq!(manifest["schema_version"], "codenoesis.local-explorer/v7");
    assert_eq!(manifest["security"]["network"], false);
    assert_eq!(manifest["security"]["dynamic_code"], false);
    let viewer = fs::read(repository.inner.explorer.join("index.html")).expect("read R14 viewer");
    let immutable = normalize_lf(
        fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/s4/k1/index.html"))
            .expect("read immutable K1 viewer")
            .as_slice(),
    );
    assert_eq!(viewer, immutable, "R14 viewer bytes changed");
    assert!(!repository.build_sentinel().exists());
}

#[test]
fn conf_fr_cli_001_r14_selector_dispatch_is_closed_and_side_effect_free() {
    let repository = MaterializedExpressionBindingRepository::fixture();
    let missing_callable = repository.scan_with_profiles(false, true, &[]);
    assert_typed_error(
        &missing_callable,
        2,
        "codenoesis.error/v21",
        "input.unsupported_rust_expression_composition",
    );
    for extra_options in [
        ["--repository-boundary-profile", "local-gitlinks-v1"],
        [
            "--rust-semantic-profile",
            "rust-cfg-declaration-alternatives-v1",
        ],
        ["--compiler-index-profile", "scip-rust-v0.9.0-import-v1"],
    ] {
        let output = repository.scan_with_profiles(true, true, &extra_options);
        assert_typed_error(
            &output,
            2,
            "codenoesis.error/v21",
            "input.unsupported_rust_expression_composition",
        );
    }
    assert!(
        !repository.store().exists(),
        "invalid R14 selectors mutated the store"
    );

    let legacy = MaterializedExpressionBindingRepository::fixture();
    let k1 = legacy.scan_with_profiles(true, false, &[]);
    assert_success(&k1, "legacy K1 selector dispatch");
    assert_eq!(
        parse_single_document(&k1.stdout)["schema_version"],
        "codenoesis.repository-snapshot/v11"
    );
    assert!(!legacy.build_sentinel().exists());
}

#[test]
fn pt_nfr_det_001_r14_fifty_permutations_and_ten_schedules_are_identical() {
    let repository = MaterializedExpressionBindingRepository::fixture();
    let mut expected_semantic = None;
    for seed in 0..50 {
        let output = repository
            .permuted_scan_command(seed)
            .output()
            .expect("run R14 argument permutation");
        let semantic = semantic_projection(&output);
        if let Some(expected) = &expected_semantic {
            assert_eq!(&semantic, expected, "R14 semantic permutation {seed}");
        } else {
            expected_semantic = Some(semantic);
        }
    }

    let schedules = (100..110)
        .map(|seed| {
            let mut command = repository.permuted_scan_command(seed);
            std::thread::spawn(move || command.output().expect("run R14 parallel schedule"))
        })
        .collect::<Vec<_>>();
    let expected_semantic = expected_semantic.expect("R14 semantic oracle");
    for (schedule, handle) in schedules.into_iter().enumerate() {
        let output = handle.join().expect("join R14 parallel schedule");
        assert_eq!(
            semantic_projection(&output),
            expected_semantic,
            "R14 semantic schedule {schedule}"
        );
    }
    assert!(!repository.build_sentinel().exists());
}

fn count_by<'a>(values: &'a [Value], field: &str) -> BTreeMap<&'a str, u64> {
    let mut counts = BTreeMap::new();
    for value in values {
        if let Some(key) = value[field].as_str() {
            *counts.entry(key).or_insert(0) += 1;
        }
    }
    counts
}

fn generated_markdown(root: &std::path::Path) -> String {
    let mut paths = Vec::new();
    collect_markdown_paths(root, &mut paths);
    paths.sort();
    paths.into_iter().fold(String::new(), |mut output, path| {
        output.push_str(&fs::read_to_string(path).expect("read R14 documentation"));
        output
    })
}

fn collect_markdown_paths(root: &std::path::Path, paths: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(root).expect("read R14 documentation root") {
        let path = entry.expect("R14 documentation entry").path();
        if path.is_dir() {
            collect_markdown_paths(&path, paths);
        } else if path.extension().and_then(std::ffi::OsStr::to_str) == Some("md") {
            paths.push(path);
        }
    }
}

fn assert_private(value: &Value) {
    let text = serde_json::to_string(value).expect("serialize R14 privacy projection");
    for forbidden in [
        "body_text",
        "initializer_text",
        "expression_text",
        "literal_lexeme",
        "source_snippet",
        "file://",
        "http://",
        "https://",
        "BEGIN PRIVATE KEY",
    ] {
        assert!(
            !text.contains(forbidden),
            "R14 private field leaked: {forbidden}"
        );
    }
}

fn assert_success(output: &Output, label: &str) {
    assert!(
        output.status.success(),
        "{label} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_typed_error(output: &Output, exit_code: i32, schema: &str, code: &str) {
    assert_eq!(output.status.code(), Some(exit_code));
    assert!(output.stdout.is_empty());
    let error = parse_single_document(&output.stderr);
    assert_eq!(error["schema_version"], schema);
    assert_eq!(error["code"], code);
}

fn semantic_projection(output: &Output) -> Vec<u8> {
    assert_success(output, "R14 deterministic scan");
    serde_json::to_vec(&parse_single_document(&output.stdout)["semantic"])
        .expect("serialize R14 semantic projection")
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

fn hex_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut output, byte| {
            use std::fmt::Write as _;
            write!(&mut output, "{byte:02x}").expect("write R14 SHA-256 hex");
            output
        })
}
