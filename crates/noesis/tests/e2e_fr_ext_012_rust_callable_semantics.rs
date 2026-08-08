mod support;

use std::collections::BTreeMap;
use std::fs;
use std::process::{Command, Output};

use codenoesis_contracts::{K1ContractError, PortableGraphV2};
use serde_json::Value;

use support::parse_single_document;
use support::s4_k1::{MaterializedCallableRepository, REPOSITORY_ID};

#[test]
#[allow(clippy::too_many_lines)]
fn e2e_fr_ext_012_rust_callable_semantics() {
    let repository = MaterializedCallableRepository::fixture();
    let scan = repository.scan();
    assert_success(&scan, "K1 callable-semantics scan");
    assert!(scan.stderr.is_empty());
    let snapshot = parse_single_document(&scan.stdout);
    assert_eq!(
        snapshot["schema_version"],
        "codenoesis.repository-snapshot/v11"
    );
    assert_eq!(
        snapshot["semantic"]["ontology_version"],
        "codenoesis.ontology/rust/v8"
    );
    assert_eq!(
        snapshot["semantic"]["knowledge_graph"]["schema_version"],
        "codenoesis.knowledge-graph/v8"
    );
    assert_eq!(
        snapshot["semantic"]["repository"]["identity"],
        REPOSITORY_ID
    );

    let graph = &snapshot["semantic"]["knowledge_graph"];
    assert_eq!(
        count_by(graph, "entities", "kind"),
        BTreeMap::from([
            ("rust.call_site", 9),
            ("rust.callable_signature", 9),
            ("rust.control", 11),
            ("rust.declared_value", 10),
            ("rust.local_binding", 4),
            ("rust.parameter", 15),
        ])
    );
    let relationship_counts = count_by(graph, "relationships", "kind");
    for (kind, expected) in [
        ("HAS_SIGNATURE", 9),
        ("HAS_PARAMETER", 15),
        ("DECLARES_VALUE", 10),
        ("HAS_BODY_FACT", 24),
        ("CALLS", 4),
    ] {
        assert_eq!(relationship_counts.get(kind), Some(&expected), "{kind}");
    }
    let value_states = graph["entities"]
        .as_array()
        .expect("K1 graph entities")
        .iter()
        .filter(|entity| entity["kind"] == "rust.declared_value")
        .fold(BTreeMap::new(), |mut counts, entity| {
            let state = entity["properties"]["state"]
                .as_str()
                .expect("K1 declared-value state");
            *counts.entry(state).or_insert(0_u64) += 1;
            counts
        });
    assert_eq!(
        value_states,
        BTreeMap::from([
            ("expression_only", 2),
            ("normalized_scalar", 7),
            ("unresolved", 1),
        ])
    );
    assert_eq!(
        graph["callable_semantics_index"]["unresolved_call_site_ids"]
            .as_array()
            .expect("K1 unresolved calls")
            .len(),
        5
    );

    assert_success(&repository.docs(), "K1 documentation generation");
    let signature_id = graph["entities"]
        .as_array()
        .expect("K1 graph entities")
        .iter()
        .find(|entity| entity["kind"] == "rust.callable_signature")
        .and_then(|entity| entity["id"].as_str())
        .expect("K1 callable signature ID");
    let query = repository.query(signature_id);
    assert_success(&query, "K1 exact-ID query");
    assert_eq!(
        parse_single_document(&query.stdout)["schema_version"],
        "codenoesis.local-query-result/v6"
    );

    let export = repository.export();
    assert_success(&export, "K1 portable export");
    let portable_bytes =
        fs::read(repository.portable.join("portable-graph.json")).expect("read K1 portable graph");
    assert_eq!(export.stdout, portable_bytes);
    let portable = parse_single_document(&portable_bytes);
    assert_eq!(portable["schema_version"], "codenoesis.portable-graph/v2");
    assert_eq!(
        portable["source_snapshot"]["schema_version"],
        "codenoesis.repository-snapshot/v11"
    );
    assert_private(&portable);

    let explore = repository.explore();
    assert_success(&explore, "K1 offline explorer");
    let manifest = parse_single_document(&explore.stdout);
    assert_eq!(manifest["schema_version"], "codenoesis.local-explorer/v2");
    assert_eq!(manifest["security"]["network"], false);
    assert_eq!(manifest["security"]["dynamic_code"], false);
    assert_eq!(
        manifest["capabilities"]["bounded_traversal"],
        serde_json::json!([1, 2])
    );
    assert!(repository.explorer.join("index.html").is_file());
    assert!(!repository.build_sentinel().exists());
}

#[test]
fn sec_fr_exp_002_xss_csp_path_and_race_fail_closed() {
    let repository = MaterializedCallableRepository::fixture();
    assert_success(&repository.scan(), "K1 security source scan");
    assert_success(&repository.export(), "K1 security export");
    let bytes = fs::read(repository.portable.join("portable-graph.json"))
        .expect("read K1 security portable graph");
    let portable = PortableGraphV2::from_canonical_file(&bytes, noesis::portable_explorer::sha256)
        .expect("reimport generated K1 portable graph");

    let mut xss = portable.value().clone();
    xss["entities"][0]["name"] = Value::String("script-close payload".to_owned());
    xss["entities"][0]["body_text"] = Value::String("forbidden".to_owned());
    let mut xss_bytes = serde_json::to_vec(&xss).expect("serialize K1 XSS mutation");
    xss_bytes.push(b'\n');
    assert!(matches!(
        PortableGraphV2::from_canonical_file(&xss_bytes, noesis::portable_explorer::sha256),
        Err(K1ContractError::UnsafePayload { .. })
    ));

    let viewer = fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/s4/k1/index.html"),
    )
    .expect("read shipped K1 viewer");
    for required in [
        "default-src 'none'",
        "connect-src 'none'",
        "worker-src 'none'",
        ".textContent",
        "const MAX_DEPTH = 2;",
        "linked_evidence",
    ] {
        assert!(
            viewer.contains(required),
            "missing K1 viewer control: {required}"
        );
    }
    for forbidden in [
        "http://",
        "https://",
        "fetch(",
        "XMLHttpRequest",
        "WebSocket",
        "eval(",
        "new Function(",
        ".innerHTML",
        "document.write",
    ] {
        assert!(
            !viewer.contains(forbidden),
            "active K1 viewer token: {forbidden}"
        );
    }

    let unsafe_output = repository.root.join("selected").join("..").join("escaped");
    let rejected = Command::new(env!("CARGO_BIN_EXE_noesis"))
        .args(["explore", "--input"])
        .arg(repository.portable.join("portable-graph.json"))
        .arg("--output")
        .arg(&unsafe_output)
        .args([
            "--explorer-profile",
            "rust-callable-semantics-v1",
            "--format",
            "json",
        ])
        .output()
        .expect("launch K1 unsafe-path explorer");
    assert_error_v16(&rejected, 2, "input.unsafe_output_path");

    let output = repository.root.join("race-output");
    let attacker = repository.root.join("race-attacker");
    let displaced = repository.root.join("race-displaced");
    fs::create_dir(&attacker).expect("create K1 race attacker");
    let prepared = noesis::portable_explorer::ensure_k1_export_output_root_for_boundary(
        &repository.store,
        &output,
    )
    .expect("prepare K1 race output");
    let replaced =
        fs::rename(&output, &displaced).is_ok() && fs::rename(&attacker, &output).is_ok();
    let result = noesis::portable_explorer::publish_portable_graph_v2(&prepared, &portable);
    if replaced {
        assert!(matches!(
            result,
            Err(noesis::portable_explorer::PortableExplorerError::UnsafeOutput { .. })
        ));
        assert!(
            fs::read_dir(&output)
                .expect("read K1 attacker output")
                .next()
                .is_none()
        );
    } else {
        assert!(
            result.is_ok(),
            "blocked race must preserve the selected K1 root"
        );
    }
}

#[test]
fn reg_fr_cli_001_selector_absence_preserves_r0_r8_bytes() {
    let root = support::s4_r8::canonical_temp_root();
    let input = support::s4_r8::write_portable_input(&root);
    let output = root.join("legacy-explorer");
    let explored = Command::new(env!("CARGO_BIN_EXE_noesis"))
        .args(["explore", "--input"])
        .arg(&input)
        .arg("--output")
        .arg(&output)
        .args(["--format", "json"])
        .output()
        .expect("launch selector-absent R8 explorer");
    assert_success(&explored, "selector-absent R8 explorer");
    assert_eq!(
        explored.stdout,
        support::s4_r8::explorer_manifest_bytes(),
        "K1 changed selector-absent R8 manifest bytes"
    );
    assert_eq!(
        fs::read(output.join("index.html")).expect("read selector-absent R8 viewer"),
        support::s4_r8::viewer_bytes(),
        "K1 changed selector-absent R8 viewer bytes"
    );
}

fn count_by<'a>(graph: &'a Value, family: &str, field: &str) -> BTreeMap<&'a str, u64> {
    graph[family]
        .as_array()
        .expect("K1 graph family")
        .iter()
        .filter_map(|record| record[field].as_str())
        .filter(|value| {
            family != "entities"
                || matches!(
                    *value,
                    "rust.callable_signature"
                        | "rust.parameter"
                        | "rust.declared_value"
                        | "rust.local_binding"
                        | "rust.call_site"
                        | "rust.control"
                )
        })
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
                        | "source_contents"
                        | "source_snippet"
                        | "environment"
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

fn assert_error_v16(output: &Output, exit_code: i32, code: &str) {
    assert_eq!(output.status.code(), Some(exit_code));
    assert!(output.stdout.is_empty());
    let error = parse_single_document(&output.stderr);
    assert_eq!(error["schema_version"], "codenoesis.error/v16");
    assert_eq!(error["code"], code);
}
