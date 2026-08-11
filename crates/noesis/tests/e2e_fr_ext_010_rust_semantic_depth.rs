mod support;

use serde_json::Value;
use sha2::{Digest as _, Sha256};
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::thread;

use support::s4_r4::MaterializedCargoManifestRepository;
use support::s4_r5::{MaterializedEmptyRustSemanticRepository, MaterializedRustSemanticRepository};

const PRE_R5_STDERR: &[u8] = b"{\"code\":\"input.invalid_revision\",\"context\":{},\"message\":\"invalid revision\",\"retryable\":false,\"schema_version\":\"codenoesis.error/v4\",\"stage\":\"input\"}\n";
const CFG_OWNER_ALTERNATIVES_SOURCE: &[u8] = br#"#[cfg(feature = "desktop")]
pub struct ConditionalOwner;

#[cfg(not(feature = "desktop"))]
pub struct ConditionalOwner {
    pub context: String,
}

#[cfg(windows)]
const BIN_DATA: &[u8] = include_bytes!("data.bin");

#[cfg(not(windows))]
const BIN_DATA: &[u8] = &[];
"#;
const HETEROGENEOUS_CFG_METHOD_ALTERNATIVES_SOURCE: &[u8] = br"pub struct Client;
pub struct Context;

impl Client {
    #[cfg(unix)]
    fn try_start_clipboard(&self, _ctx: Option<Context>) {}

    #[cfg(windows)]
    fn try_start_clipboard(&self, _p: Option<()>) {}
}
";
const PAIRED_CONSTANT_EXTENSION_SOURCE: &[u8] = br"#![allow(dead_code)]

pub const MARKER: u8 = 1;

pub fn choose(value: i32, flag: bool) -> i32 {
    let mut total = value;
    if flag {
        total = total + 1;
    }
    let result = total;
    result
}
";

#[test]
fn e2e_fr_ext_010_rust_semantic_depth() {
    let repository = MaterializedRustSemanticRepository::fixture();
    assert!(!repository.build_sentinel().exists());
    let output = repository.scan();

    if output.status.code() == Some(2) {
        assert!(output.stdout.is_empty(), "pre-R5 stdout changed");
        assert_eq!(output.stderr, PRE_R5_STDERR, "pre-R5 stderr changed");
        assert!(!repository.store.exists(), "pre-R5 store was created");
        assert!(
            !repository.documents.exists(),
            "pre-R5 documents root was created"
        );
        assert!(
            !repository.build_sentinel().exists(),
            "pre-R5 subject executed build.rs"
        );
        panic!(
            "expected RepositorySnapshotV8 success; observed approved unknown Rust semantic selector Red"
        );
    }

    assert!(
        output.status.success(),
        "R5 scan failed: status={:?}, stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "successful R5 stderr changed");
    assert!(
        !repository.build_sentinel().exists(),
        "R5 subject executed build.rs"
    );
    let snapshot: Value =
        serde_json::from_slice(&output.stdout).expect("parse RepositorySnapshotV8");
    assert_eq!(
        snapshot["schema_version"],
        "codenoesis.repository-snapshot/v8"
    );
    assert_eq!(
        snapshot["semantic"]["configuration"]["rust_semantic_profile"],
        "rust-semantic-depth-v1"
    );
    assert_eq!(
        snapshot["semantic"]["knowledge_graph"]["schema_version"],
        "codenoesis.knowledge-graph/v5"
    );
    assert_eq!(
        snapshot["semantic"]["knowledge_graph"]["ontology_version"],
        "codenoesis.ontology/rust/v5"
    );
    assert_eq!(
        snapshot["semantic"]["knowledge_graph"]["rust_semantic_index"]["profile"],
        "rust-semantic-depth-v1"
    );
    assert!(
        snapshot["semantic"]["extraction_chunks"]
            .as_array()
            .expect("V8 extraction chunks")
            .iter()
            .all(|chunk| {
                chunk["schema_version"] == "codenoesis.extraction-chunk/v5"
                    && chunk["ontology_version"] == "codenoesis.ontology/rust/v5"
            })
    );
    let entities = snapshot["semantic"]["knowledge_graph"]["entities"]
        .as_array()
        .expect("V8 graph entities");
    for (kind, expected) in [
        ("rust.field", 16),
        ("rust.enum_variant", 5),
        ("rust.constant", 6),
        ("rust.static", 2),
        ("rust.associated_type", 2),
        ("rust.method", 9),
    ] {
        assert_eq!(
            entities
                .iter()
                .filter(|entity| entity["kind"] == kind)
                .count(),
            expected,
            "reviewed R5 count changed for {kind}"
        );
    }
    assert!(entities.iter().any(|entity| {
        entity["id"]
            == "urn:codenoesis:entity:blake3:ab24c82375e533b482ef71fe657a00b190ecf49712498cdc772c8d9db946b9d4"
    }));
}

#[test]
fn conf_fr_ext_010_empty_semantic_extension_checkpoint_is_bound() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let oracle: Value = serde_json::from_slice(
        &fs::read(root.join("tests/specifications/s4/r5/empty-semantic-extension-v1.json"))
            .expect("read empty R5 oracle"),
    )
    .expect("parse empty R5 oracle");
    assert_eq!(oracle["issue"], 164);
    assert_eq!(
        oracle["base_sha"],
        "408e4021044cf6b3628b6a8873787148de719341"
    );
    assert_eq!(oracle["slice"], "S4");
    assert_eq!(oracle["risk"], "high");
    assert_eq!(
        oracle["expected_red"]["stderr_sha256"],
        "9b284f4bb7368bb0d11c5b33725c109ee469845aac00081e51175413adec4e3c"
    );
    assert_eq!(oracle["acceptance"]["r5"]["additive_entities"], 0);
    assert_eq!(oracle["acceptance"]["r14_complete_counts"]["entities"], 27);
    assert_eq!(oracle["acceptance"]["permutations"], 50);
    assert_eq!(oracle["acceptance"]["schedules"], 10);

    for (path, marker) in [
        ("README.md", "Implemented local Rust analysis through R14"),
        (
            "docs/software/software-requirements-specification.md",
            "### 2.26 S4 empty additive R5 correction register",
        ),
        (
            "docs/software/architecture.md",
            "### Empty R5 additive neutral element",
        ),
        ("docs/software/roadmap.md", "R0-R14 and K1 are implemented"),
        (
            "docs/software/decisions/0026-s4-empty-semantic-extension-neutrality.md",
            "c780476957a29db6ede1cefb408140763990e829",
        ),
    ] {
        let text = fs::read_to_string(root.join(path)).expect("read issue #164 governance");
        assert!(text.contains(marker), "missing issue #164 marker in {path}");
    }

    let decision_0025 = fs::read(
        root.join("docs/software/decisions/0025-s4-r14-rust-expression-bindings-contract.md"),
    )
    .expect("read immutable Decision 0025");
    assert_eq!(
        sha256_hex(&decision_0025),
        "da0da4a3d9ace0a0e58dee5d747e8c5557250f712040fd52f7c1e57f1fd699ad"
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn e2e_fr_ext_010_empty_semantic_extension_reaches_r14() {
    let repository = MaterializedEmptyRustSemanticRepository::fixture();
    assert!(!repository.build_sentinel().exists());
    let scan = repository.scan_r14();

    if scan.status.code() == Some(11) {
        assert!(scan.stdout.is_empty(), "empty R5 Red stdout changed");
        assert_eq!(scan.stderr.len(), 196, "empty R5 Red length changed");
        assert_eq!(
            sha256_hex(&scan.stderr),
            "9b284f4bb7368bb0d11c5b33725c109ee469845aac00081e51175413adec4e3c",
            "empty R5 Red digest changed"
        );
        let error: Value = serde_json::from_slice(&scan.stderr).expect("parse ErrorV21 Red");
        assert_eq!(error["schema_version"], "codenoesis.error/v21");
        assert_eq!(error["code"], "internal.unexpected");
        assert_eq!(error["stage"], "internal");
        assert_eq!(error["context"]["stage"], "expression_extraction");
        assert!(!repository.store.exists(), "empty R5 Red created a store");
        assert!(!repository.build_sentinel().exists());
        panic!(
            "expected RepositorySnapshotV16 success; observed invalid empty additive R5 rejection"
        );
    }

    assert!(
        scan.status.success(),
        "empty R5 R14 scan failed: status={:?}, stderr={}",
        scan.status.code(),
        String::from_utf8_lossy(&scan.stderr)
    );
    assert!(scan.stderr.is_empty());
    let snapshot: Value = serde_json::from_slice(&scan.stdout).expect("parse empty R5 V16");
    assert_eq!(
        snapshot["schema_version"],
        "codenoesis.repository-snapshot/v16"
    );
    let graph = &snapshot["semantic"]["knowledge_graph"];
    assert_eq!(graph["schema_version"], "codenoesis.knowledge-graph/v13");
    for (family, expected) in [
        ("entities", 27),
        ("relationships", 38),
        ("evidence", 29),
        ("claims", 65),
        ("diagnostics", 0),
        ("coverage", 15),
    ] {
        assert_eq!(
            graph[family].as_array().map_or(0, Vec::len),
            expected,
            "empty R5 complete family {family}"
        );
    }

    let entities = graph["entities"].as_array().expect("empty R5 entities");
    for (kind, expected) in [
        ("rust.function", 1),
        ("rust.callable_signature", 1),
        ("rust.parameter", 2),
        ("rust.local_binding", 2),
        ("rust.control", 1),
        ("rust.expression", 9),
        ("rust.pattern_binding", 4),
        ("rust.constant", 0),
        ("rust.declared_value", 0),
    ] {
        assert_eq!(
            entities
                .iter()
                .filter(|entity| entity["kind"] == kind)
                .count(),
            expected,
            "empty R5 entity kind {kind}"
        );
    }
    for kind in [
        "rust.field",
        "rust.enum_variant",
        "rust.static",
        "rust.associated_type",
        "rust.method",
    ] {
        assert!(entities.iter().all(|entity| entity["kind"] != kind));
    }
    assert_eq!(
        graph["rust_semantic_index"]["member_entity_ids"]
            .as_array()
            .expect("R5 member index")
            .len(),
        0
    );
    assert_eq!(
        graph["rust_semantic_index"]["implementation_context_method_ids"]
            .as_array()
            .expect("R5 method index")
            .len(),
        0
    );

    let relationships = graph["relationships"]
        .as_array()
        .expect("empty R5 relationships");
    for (kind, expected) in [
        ("HAS_EXPRESSION", 9),
        ("CONTAINS_EXPRESSION", 4),
        ("DECLARES_BINDING", 4),
        ("BINDS_FROM", 2),
        ("READS", 5),
        ("WRITES", 1),
        ("DECLARES_VALUE", 0),
    ] {
        assert_eq!(
            relationships
                .iter()
                .filter(|relationship| relationship["kind"] == kind)
                .count(),
            expected,
            "empty R5 relationship kind {kind}"
        );
    }
    let coverage = graph["coverage"]
        .as_array()
        .expect("empty R5 coverage")
        .iter()
        .filter_map(|gap| gap["capability"].as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        coverage,
        BTreeSet::from([
            "cargo.active_target_not_resolved",
            "rust.attribute_semantics_not_interpreted",
            "rust.cfg_presence_unresolved",
            "rust.compiler_cfg_not_computed",
            "rust.data_flow_not_computed",
            "rust.foreign_block_unsupported",
            "rust.macro_generated_items_not_analyzed",
            "rust.ownership_flow_not_computed",
            "rust.reachability_not_computed",
            "rust.runtime_behavior_not_observed",
            "rust.side_effects_not_computed",
            "rust.type_resolution_not_performed",
            "rust.union_unsupported",
            "rust.unsupported_impl_header",
            "rust.value_not_evaluated",
        ])
    );

    let docs = repository.docs();
    assert!(
        docs.status.success(),
        "empty R5 R14 docs failed: {}",
        String::from_utf8_lossy(&docs.stderr)
    );
    let function_id = entities
        .iter()
        .find(|entity| entity["kind"] == "rust.function")
        .and_then(|entity| entity["id"].as_str())
        .expect("empty R5 function ID");
    let query = repository.query(function_id);
    assert!(
        query.status.success(),
        "empty R5 R14 query failed: {}",
        String::from_utf8_lossy(&query.stderr)
    );
    let query_result: Value = serde_json::from_slice(&query.stdout).expect("parse V11 query");
    assert_eq!(
        query_result["schema_version"],
        "codenoesis.local-query-result/v11"
    );
    assert_eq!(query_result["requested_id"], function_id);

    let export = repository.export();
    assert!(
        export.status.success(),
        "empty R5 R14 export failed: {}",
        String::from_utf8_lossy(&export.stderr)
    );
    let portable_path = repository.portable.join("portable-graph.json");
    let (_, portable_bytes) = noesis::portable_explorer::read_portable_graph_v7(&portable_path)
        .expect("strictly reimport empty R5 R14 portable graph");
    assert_eq!(export.stdout, portable_bytes);

    let explore = repository.explore();
    assert!(
        explore.status.success(),
        "empty R5 R14 explore failed: {}",
        String::from_utf8_lossy(&explore.stderr)
    );
    let explorer: Value = serde_json::from_slice(&explore.stdout).expect("parse V7 explorer");
    assert_eq!(explorer["schema_version"], "codenoesis.local-explorer/v7");
    assert!(!repository.build_sentinel().exists());
}

#[test]
fn reg_fr_ext_010_paired_constant_reaches_r14_without_silent_drop() {
    let mut repository = MaterializedEmptyRustSemanticRepository::fixture();
    repository.replace_source_and_commit(PAIRED_CONSTANT_EXTENSION_SOURCE);
    let scan = repository.scan_r14();
    assert!(
        scan.status.success(),
        "paired R5 constant scan failed: {}",
        String::from_utf8_lossy(&scan.stderr)
    );
    let snapshot: Value = serde_json::from_slice(&scan.stdout).expect("parse paired R14 V16");
    let graph = &snapshot["semantic"]["knowledge_graph"];
    let entities = graph["entities"].as_array().expect("paired graph entities");
    assert_eq!(
        entities
            .iter()
            .filter(|entity| entity["kind"] == "rust.constant" && entity["name"] == "MARKER")
            .count(),
        1
    );
    assert_eq!(
        entities
            .iter()
            .filter(|entity| entity["kind"] == "rust.declared_value")
            .count(),
        1
    );
    assert_eq!(
        graph["relationships"]
            .as_array()
            .expect("paired graph relationships")
            .iter()
            .filter(|relationship| relationship["kind"] == "DECLARES_VALUE")
            .count(),
        1
    );
    assert_eq!(
        graph["rust_semantic_index"]["member_entity_ids"]
            .as_array()
            .expect("paired R5 index")
            .len(),
        1
    );
    assert!(!repository.build_sentinel().exists());
}

#[test]
fn pt_nfr_det_001_empty_semantic_extension_has_fifty_permutations_and_ten_schedules() {
    let repository = MaterializedEmptyRustSemanticRepository::fixture();
    let baseline_output = repository
        .permuted_scan_command(0)
        .output()
        .expect("run empty R5 permutation zero");
    let baseline = deterministic_semantic(&baseline_output);
    for seed in 1..50 {
        let output = repository
            .permuted_scan_command(seed)
            .output()
            .expect("run empty R5 input permutation");
        let semantic = deterministic_semantic(&output);
        assert_eq!(semantic, baseline, "empty R5 permutation {seed}");
    }

    thread::scope(|scope| {
        let handles = (0..10)
            .map(|schedule| {
                let mut command = repository.permuted_scan_command(50 + schedule);
                scope.spawn(move || command.output().expect("run empty R5 schedule"))
            })
            .collect::<Vec<_>>();
        for (schedule, handle) in handles.into_iter().enumerate() {
            let output = handle.join().expect("join empty R5 schedule");
            let semantic = deterministic_semantic(&output);
            assert_eq!(semantic, baseline, "empty R5 schedule {schedule}");
        }
    });
    assert!(!repository.build_sentinel().exists());
}

#[test]
fn e2e_fr_ext_010_cfg_owner_alternatives_publish_one_logical_owner() {
    let mut repository = MaterializedRustSemanticRepository::fixture();
    repository.replace_source_and_commit(CFG_OWNER_ALTERNATIVES_SOURCE);
    let output = repository.scan();
    assert!(
        output.status.success(),
        "cfg-alternative R5 scan failed: status={:?}, stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert!(!repository.build_sentinel().exists());

    let snapshot: Value =
        serde_json::from_slice(&output.stdout).expect("parse cfg-alternative V8 snapshot");
    let graph = &snapshot["semantic"]["knowledge_graph"];
    let entities = graph["entities"]
        .as_array()
        .expect("cfg-alternative graph entities");
    assert_eq!(
        entities
            .iter()
            .filter(|entity| {
                entity["kind"] == "rust.struct" && entity["name"] == "ConditionalOwner"
            })
            .count(),
        1
    );
    assert!(entities.iter().any(|entity| {
        entity["kind"] == "rust.field"
            && entity["name"] == "context"
            && entity["compilation_presence"] == "conditional_unknown"
    }));
    let constants = entities
        .iter()
        .filter(|entity| entity["kind"] == "rust.constant" && entity["name"] == "BIN_DATA")
        .collect::<Vec<_>>();
    assert_eq!(constants.len(), 1);
    assert_eq!(constants[0]["compilation_presence"], "conditional_unknown");
    let constant_attributes = constants[0]["properties"]["attributes"]
        .as_array()
        .expect("BIN_DATA attributes");
    assert_eq!(constant_attributes.len(), 2);
    assert!(
        constant_attributes
            .iter()
            .all(|attribute| attribute["kind"] == "cfg")
    );

    let cfg_diagnostics = graph["diagnostics"]
        .as_array()
        .expect("cfg-alternative diagnostics")
        .iter()
        .filter(|diagnostic| diagnostic["code"] == "rust.cfg_presence_unresolved")
        .collect::<Vec<_>>();
    assert_eq!(cfg_diagnostics.len(), 4);
    let evidence_ids = graph["evidence"]
        .as_array()
        .expect("cfg-alternative evidence")
        .iter()
        .filter_map(|evidence| evidence["id"].as_str())
        .collect::<BTreeSet<_>>();
    assert!(cfg_diagnostics.iter().all(|diagnostic| {
        diagnostic["evidence_ids"]
            .as_array()
            .is_some_and(|identifiers| {
                identifiers.iter().all(|identifier| {
                    identifier
                        .as_str()
                        .is_some_and(|identifier| evidence_ids.contains(identifier))
                })
            })
    }));
    assert!(constant_attributes.iter().all(|attribute| {
        attribute["evidence_id"]
            .as_str()
            .is_some_and(|identifier| evidence_ids.contains(identifier))
    }));
    assert!(repository.store.exists());
}

#[test]
fn e2e_fr_doc_001_r5_declared_and_unresolved_are_documented() {
    let repository = MaterializedRustSemanticRepository::fixture();
    let scan = repository.scan();
    assert!(scan.status.success(), "R5 setup scan failed");
    let snapshot: Value = serde_json::from_slice(&scan.stdout).expect("parse V8 setup snapshot");

    let docs = repository.docs();
    assert!(
        docs.status.success(),
        "R5 docs failed: status={:?}, stderr={}",
        docs.status.code(),
        String::from_utf8_lossy(&docs.stderr)
    );
    let manifest: Value = serde_json::from_slice(&docs.stdout).expect("parse R5 docs manifest");
    assert_r5_documentation_coverage(&snapshot, &manifest);
    let markdown = manifest["documents"]
        .as_array()
        .expect("R5 generated documents")
        .iter()
        .map(|document| {
            let path = document["path"].as_str().expect("generated document path");
            fs::read_to_string(repository.documents.join(path)).expect("read R5 generated document")
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(markdown.contains("Rust semantic declarations"));
    assert!(markdown.contains("declared syntax only"));
    assert!(markdown.contains("conditional_unknown"));
    assert!(markdown.contains("rust.cfg_presence_unresolved"));
    assert!(markdown.contains("not observed runtime behavior"));

    let field_id = "urn:codenoesis:entity:blake3:ab24c82375e533b482ef71fe657a00b190ecf49712498cdc772c8d9db946b9d4";
    let query = repository.query(field_id);
    assert!(
        query.status.success(),
        "R5 query failed: status={:?}, stderr={}",
        query.status.code(),
        String::from_utf8_lossy(&query.stderr)
    );
    let result: Value = serde_json::from_slice(&query.stdout).expect("parse LocalQueryResultV3");
    assert_eq!(result["schema_version"], "codenoesis.local-query-result/v3");
    assert_eq!(result["requested_id"], field_id);
    assert_eq!(result["result_kind"], "entity");
    assert_eq!(result["entity"]["kind"], "rust.field");
    assert_eq!(
        result["snapshot_id"], manifest["snapshot_id"],
        "stored V8 head must be the sole query-version authority"
    );
    assert_eq!(
        snapshot["semantic_hash"]["value"],
        manifest["snapshot_semantic_hash"]["value"]
    );
}

fn assert_r5_documentation_coverage(snapshot: &Value, manifest: &Value) {
    let graph = &snapshot["semantic"]["knowledge_graph"];
    let r5_entity_ids = graph["entities"]
        .as_array()
        .expect("R5 graph entities")
        .iter()
        .filter(|entity| {
            matches!(
                entity["kind"].as_str(),
                Some(
                    "rust.field"
                        | "rust.enum_variant"
                        | "rust.constant"
                        | "rust.static"
                        | "rust.associated_type"
                        | "rust.method"
                )
            )
        })
        .map(|entity| entity["id"].as_str().expect("R5 entity ID"))
        .collect::<BTreeSet<_>>();
    let diagnostic_ids = graph["diagnostics"]
        .as_array()
        .expect("R5 graph diagnostics")
        .iter()
        .map(|diagnostic| diagnostic["id"].as_str().expect("R5 diagnostic ID"))
        .collect::<BTreeSet<_>>();
    let coverage_ids = graph["coverage"]
        .as_array()
        .expect("R5 graph coverage")
        .iter()
        .filter(|gap| {
            gap["capability"]
                .as_str()
                .is_some_and(|capability| capability.starts_with("rust."))
        })
        .map(|gap| gap["id"].as_str().expect("R5 coverage ID"))
        .collect::<BTreeSet<_>>();
    let statements = manifest["documents"]
        .as_array()
        .expect("R5 generated documents")
        .iter()
        .flat_map(|document| {
            document["statements"]
                .as_array()
                .expect("R5 document statements")
        })
        .collect::<Vec<_>>();
    let documented_subjects = statements
        .iter()
        .flat_map(|statement| {
            statement["subject_ids"]
                .as_array()
                .expect("R5 statement subjects")
        })
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    let documented_gaps = statements
        .iter()
        .flat_map(|statement| {
            statement["coverage_gap_ids"]
                .as_array()
                .expect("R5 statement coverage")
        })
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    assert_eq!(r5_entity_ids.len(), 40);
    assert!(r5_entity_ids.is_subset(&documented_subjects));
    assert!(diagnostic_ids.is_subset(&documented_subjects));
    assert!(coverage_ids.is_subset(&documented_gaps));
    assert!(
        manifest["documents"]
            .as_array()
            .expect("R5 document records")
            .iter()
            .all(|document| document["byte_length"]
                .as_u64()
                .is_some_and(|bytes| bytes <= 1_048_576))
    );
}

#[test]
fn e2e_fr_qry_001_r5_exact_id_results() {
    let repository = MaterializedRustSemanticRepository::fixture();
    let scan = repository.scan();
    assert!(scan.status.success(), "R5 setup scan failed");
    let snapshot: Value = serde_json::from_slice(&scan.stdout).expect("parse V8 query snapshot");
    let docs = repository.docs();
    assert!(docs.status.success(), "R5 setup docs failed");
    let manifest: Value = serde_json::from_slice(&docs.stdout).expect("parse R5 query manifest");
    let graph = &snapshot["semantic"]["knowledge_graph"];
    let field_id = "urn:codenoesis:entity:blake3:ab24c82375e533b482ef71fe657a00b190ecf49712498cdc772c8d9db946b9d4";
    let relationship_id = graph["relationships"]
        .as_array()
        .expect("V8 relationships")
        .iter()
        .find(|relationship| relationship["target"] == field_id)
        .and_then(|relationship| relationship["id"].as_str())
        .expect("field owner relationship");
    let claim = graph["claims"]
        .as_array()
        .expect("V8 claims")
        .iter()
        .find(|claim| claim["subject_id"] == field_id)
        .expect("field claim");
    let claim_id = claim["id"].as_str().expect("field claim ID");
    let evidence_id = claim["evidence_ids"][0]
        .as_str()
        .expect("field evidence ID");
    let diagnostic_id = graph["diagnostics"]
        .as_array()
        .expect("V8 diagnostics")
        .iter()
        .find(|diagnostic| {
            diagnostic["code"]
                .as_str()
                .is_some_and(|code| code.starts_with("rust."))
        })
        .and_then(|diagnostic| diagnostic["id"].as_str())
        .expect("R5 diagnostic ID");
    let coverage_id = graph["coverage"]
        .as_array()
        .expect("V8 coverage")
        .iter()
        .find(|gap| {
            gap["capability"]
                .as_str()
                .is_some_and(|capability| capability.starts_with("rust."))
        })
        .and_then(|gap| gap["id"].as_str())
        .expect("R5 coverage ID");
    let document_id = manifest["documents"]
        .as_array()
        .expect("R5 documents")
        .iter()
        .find(|document| document["path"] == "overview.md")
        .and_then(|document| document["document_id"].as_str())
        .expect("R5 overview document ID");

    for (kind, requested_id) in [
        ("entity", field_id),
        ("relationship", relationship_id),
        ("claim", claim_id),
        ("evidence", evidence_id),
        ("diagnostic", diagnostic_id),
        ("coverage_gap", coverage_id),
        ("document", document_id),
    ] {
        let first = repository.query(requested_id);
        assert!(
            first.status.success(),
            "R5 {kind} query failed: {}",
            String::from_utf8_lossy(&first.stderr)
        );
        let replay = repository.query(requested_id);
        assert_eq!(replay.status.code(), Some(0));
        assert_eq!(replay.stdout, first.stdout, "R5 {kind} replay changed");
        let result: Value =
            serde_json::from_slice(&first.stdout).expect("parse exact R5 query result");
        assert_eq!(result["schema_version"], "codenoesis.local-query-result/v3");
        assert_eq!(result["requested_id"], requested_id);
        assert_eq!(result["result_kind"], kind);
    }
}

#[test]
fn reg_fr_cli_001_r5_selector_absence_is_byte_identical() {
    let repository = MaterializedCargoManifestRepository::fixture();
    let output = repository.scan_r3();
    assert_eq!(output.status.code(), Some(11));
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"{\"code\":\"extraction.invalid_workspace_manifest\",\"context\":{\"path\":\"crates/app/Cargo.toml\",\"reason\":\"unsupported_structural_key\"},\"message\":\"invalid workspace manifest\",\"retryable\":false,\"schema_version\":\"codenoesis.error/v10\",\"stage\":\"extraction\"}\n"
    );
    assert!(!repository.store.exists());
    assert!(!repository.documents.exists());
}

#[test]
fn sec_fr_ext_010_invalid_composition_precedes_acquisition() {
    let invalid = MaterializedRustSemanticRepository::fixture();
    let output = invalid.scan_with_profiles(true, true, Some("unknown"));
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"{\"code\":\"input.invalid_rust_semantic_profile\",\"context\":{\"provided_profile\":\"unknown\"},\"message\":\"invalid rust semantic profile\",\"retryable\":false,\"schema_version\":\"codenoesis.error/v12\",\"stage\":\"input\"}\n"
    );
    assert!(!invalid.store.exists());
    assert!(!invalid.build_sentinel().exists());

    let incomplete = MaterializedRustSemanticRepository::fixture();
    let output = incomplete.scan_with_profiles(true, false, Some("rust-semantic-depth-v1"));
    assert_eq!(output.status.code(), Some(11));
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"{\"code\":\"extraction.unsupported_rust_semantic_composition\",\"context\":{\"profile\":\"rust-semantic-depth-v1\",\"reason\":\"cargo_manifest_facts_profile_required\",\"required_profile\":\"standard-local-s4+cargo-root-package-v1+cargo-manifest-facts-v1\"},\"message\":\"unsupported rust semantic composition\",\"retryable\":false,\"schema_version\":\"codenoesis.error/v12\",\"stage\":\"extraction\"}\n"
    );
    assert!(!incomplete.store.exists());
    assert!(!incomplete.build_sentinel().exists());
}

#[test]
fn sec_fr_ext_010_malformed_has_error_v12_no_publication() {
    let mut repository = MaterializedRustSemanticRepository::fixture();
    repository.replace_source_and_commit(b"pub struct Broken { value: }\n");
    let output = repository.scan();
    assert_eq!(output.status.code(), Some(11));
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"{\"code\":\"extraction.invalid_rust_semantic_declaration\",\"context\":{\"declaration_kind\":\"syntax_error\",\"path\":\"src/lib.rs\",\"start_byte\":0},\"message\":\"invalid rust semantic declaration\",\"retryable\":false,\"schema_version\":\"codenoesis.error/v12\",\"stage\":\"extraction\"}\n"
    );
    assert!(!repository.store.exists());
    assert!(!repository.documents.exists());
    assert!(!repository.build_sentinel().exists());
}

#[test]
fn sec_fr_ext_010_unconditional_owner_duplicates_keep_error_v12() {
    let mut repository = MaterializedRustSemanticRepository::fixture();
    repository.replace_source_and_commit(b"pub struct Repeated;\npub struct Repeated;\n");
    let output = repository.scan();
    assert_eq!(output.status.code(), Some(11));
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).expect("parse ErrorV12");
    assert_eq!(error["schema_version"], "codenoesis.error/v12");
    assert_eq!(error["code"], "extraction.rust_semantic_identity_conflict");
    assert_eq!(error["context"]["member_kind"], "rust.struct");
    assert_eq!(error["context"]["normalized_member"], "Repeated");
    assert!(!repository.store.exists());
    assert!(!repository.documents.exists());
    assert!(!repository.build_sentinel().exists());
}

#[test]
fn sec_fr_ext_010_heterogeneous_cfg_method_alternatives_keep_error_v12() {
    let mut repository = MaterializedRustSemanticRepository::fixture();
    repository.replace_source_and_commit(HETEROGENEOUS_CFG_METHOD_ALTERNATIVES_SOURCE);
    let output = repository.scan();
    assert_eq!(output.status.code(), Some(11));
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).expect("parse ErrorV12");
    assert_eq!(error["schema_version"], "codenoesis.error/v12");
    assert_eq!(error["code"], "extraction.rust_semantic_identity_conflict");
    assert_eq!(error["context"]["member_kind"], "rust.method");
    assert_eq!(error["context"]["normalized_member"], "try_start_clipboard");
    assert!(!repository.store.exists());
    assert!(!repository.documents.exists());
    assert!(!repository.build_sentinel().exists());
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(&mut output, "{byte:02x}").expect("write SHA-256");
    }
    output
}

fn deterministic_semantic(output: &std::process::Output) -> Vec<u8> {
    assert!(
        output.status.success(),
        "deterministic empty R5 scan failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let snapshot: Value = serde_json::from_slice(&output.stdout).expect("parse deterministic V16");
    serde_json::to_vec(&snapshot["semantic"]).expect("serialize deterministic V16 semantic")
}
