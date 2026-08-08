mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::process::{Command, Output};

use codenoesis_domain::s4_r7::R7_DETERMINISM_PERMUTATIONS;
use serde_json::{Value, json};

use support::s4_r4::MaterializedCargoManifestRepository;
use support::s4_r7::{
    MaterializedCompilerIndexRepository, expected_overlay, invalid_case_expectations,
};

const PRE_R7_STDERR: &[u8] = b"{\"code\":\"input.invalid_revision\",\"context\":{},\"message\":\"invalid revision\",\"retryable\":false,\"schema_version\":\"codenoesis.error/v4\",\"stage\":\"input\"}\n";
const R7_QUERY_KINDS: [&str; 7] = [
    "entity",
    "relationship",
    "claim",
    "evidence",
    "diagnostic",
    "coverage_gap",
    "document",
];

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

#[test]
fn gt_fr_ext_005_cross_crate_symbol_resolution() {
    let repository = MaterializedCompilerIndexRepository::fixture();
    let snapshot = successful_snapshot(&repository);
    let expected = expected_overlay();
    let graph = &snapshot["semantic"]["knowledge_graph"];
    let expected_relationship = expected["relationships"]
        .as_array()
        .expect("reviewed R7 relationships")
        .iter()
        .find(|relationship| relationship["kind"] == "RESOLVES_TO")
        .expect("reviewed cross-crate resolution");
    let relationship = graph["relationships"]
        .as_array()
        .expect("R7 graph relationships")
        .iter()
        .find(|relationship| relationship["id"] == expected_relationship["id"])
        .unwrap_or_else(|| {
            panic!(
                "promoted cross-crate resolution not found; expected={} actual={:?}",
                expected_relationship["id"],
                graph["relationships"]
                    .as_array()
                    .expect("R7 graph relationships")
                    .iter()
                    .filter(|relationship| relationship["kind"] == "RESOLVES_TO")
                    .collect::<Vec<_>>()
            )
        });
    assert_eq!(relationship["kind"], "RESOLVES_TO");
    assert_eq!(relationship["source"], expected_relationship["source"]);
    assert_eq!(relationship["target"], expected_relationship["target"]);
    assert_eq!(relationship["endpoint_binding"], "unique");
    assert_eq!(relationship["provenance"], "validated_scip_v0.9.0");
    assert_eq!(
        relationship["evidence_ids"],
        expected_relationship["evidence_ids"]
    );

    let diagnostic = graph["diagnostics"]
        .as_array()
        .expect("R7 graph diagnostics")
        .iter()
        .find(|diagnostic| diagnostic["code"] == "compiler_index.syntax_uncertainty_retained")
        .expect("syntax uncertainty diagnostic");
    assert_eq!(diagnostic["subject_id"], relationship["source"]);
    assert_eq!(diagnostic["compiler_target_id"], relationship["target"]);
}

#[test]
fn gt_fr_ext_005_explicit_implementation_and_type_relations() {
    let repository = MaterializedCompilerIndexRepository::fixture();
    let snapshot = successful_snapshot(&repository);
    let expected = expected_overlay();
    let relationships = snapshot["semantic"]["knowledge_graph"]["relationships"]
        .as_array()
        .expect("R7 graph relationships");

    for reviewed in expected["relationships"]
        .as_array()
        .expect("reviewed R7 relationships")
        .iter()
        .filter(|relationship| {
            matches!(
                relationship["kind"].as_str(),
                Some("IMPLEMENTS" | "TYPE_DEFINITION")
            )
        })
    {
        let actual = relationships
            .iter()
            .find(|relationship| relationship["id"] == reviewed["id"])
            .expect("reviewed explicit compiler relationship");
        assert_eq!(actual["kind"], reviewed["kind"]);
        assert_eq!(actual["source"], reviewed["source"]);
        assert_eq!(actual["target"], reviewed["target"]);
        assert_eq!(actual["evidence_ids"], reviewed["evidence_ids"]);
        assert_eq!(actual["endpoint_binding"], "unique");
    }
}

#[test]
fn gt_fr_ext_005_external_and_generated_symbols_remain_bounded() {
    let repository = MaterializedCompilerIndexRepository::fixture();
    let snapshot = successful_snapshot(&repository);
    let graph = &snapshot["semantic"]["knowledge_graph"];
    let compiler_symbols = graph["entities"]
        .as_array()
        .expect("R7 graph entities")
        .iter()
        .filter(|entity| entity["kind"] == "compiler.symbol")
        .collect::<Vec<_>>();
    let external = compiler_symbols
        .iter()
        .filter(|entity| entity["binding_state"] == "external_unbound")
        .collect::<Vec<_>>();
    let generated = compiler_symbols
        .iter()
        .filter(|entity| entity["binding_state"] == "generated_unbound")
        .collect::<Vec<_>>();
    assert_eq!(external.len(), 1);
    assert_eq!(generated.len(), 1);
    for symbol in external.into_iter().chain(generated) {
        assert!(symbol["source_entity_id"].is_null());
        assert!(
            symbol["compiler_evidence_ids"]
                .as_array()
                .is_some_and(|evidence| !evidence.is_empty())
        );
    }
    let relationship_kinds = graph["relationships"]
        .as_array()
        .expect("R7 relationships")
        .iter()
        .filter(|relationship| relationship["provenance"] == "validated_scip_v0.9.0")
        .map(|relationship| relationship["kind"].as_str().expect("relationship kind"))
        .collect::<BTreeSet<_>>();
    assert!(relationship_kinds.is_subset(&BTreeSet::from([
        "RESOLVES_TO",
        "REFERENCES",
        "IMPLEMENTS",
        "TYPE_DEFINITION",
    ])));
    for forbidden in [
        "ACTIVATES",
        "CALLS",
        "EXECUTES",
        "REACHES",
        "SERVES",
        "STARTS",
    ] {
        assert!(!relationship_kinds.contains(forbidden));
    }
}

#[test]
fn conf_fr_ext_005_snapshot_v10_graph_v7_error_v14() {
    let repository = MaterializedCompilerIndexRepository::fixture();
    let snapshot = successful_snapshot(&repository);
    assert_eq!(
        snapshot["schema_version"],
        "codenoesis.repository-snapshot/v10"
    );
    assert_eq!(
        snapshot["semantic"]["configuration"]["schema_version"],
        "codenoesis.configuration/v7"
    );
    assert_eq!(
        snapshot["semantic"]["knowledge_graph"]["schema_version"],
        "codenoesis.knowledge-graph/v7"
    );
    assert_eq!(
        snapshot["semantic"]["extraction_chunks"][0]["schema_version"],
        "codenoesis.extraction-chunk/v7"
    );

    let invalid = MaterializedCompilerIndexRepository::fixture();
    let output = base_r7_command(&invalid)
        .args(["--compiler-index-profile", "unsupported-scip-profile"])
        .args([
            "--compiler-index-binding",
            support::s4_r7::BINDING_RELATIVE_PATH,
        ])
        .output()
        .expect("launch invalid R7 profile subject");
    assert_r7_failure(&invalid, &output, "input.invalid_compiler_index_profile");
}

#[test]
fn conf_fr_ext_005_invalid_security_matrix() {
    let expected = invalid_case_expectations();
    assert_eq!(expected.len(), 29);
    let observed = expected
        .iter()
        .map(|(case_id, expected_code)| {
            let repository = MaterializedCompilerIndexRepository::fixture();
            let output = exercise_invalid_case(case_id, &repository);
            assert_eq!(
                output.status.code(),
                Some(10),
                "R7 invalid case {case_id} did not fail: stdout_len={} stderr={}",
                output.stdout.len(),
                String::from_utf8_lossy(&output.stderr)
            );
            assert_r7_failure(&repository, &output, expected_code);
            (case_id.clone(), expected_code.clone())
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(observed, expected);
}

#[test]
fn pt_nfr_det_001_r7_permutation_and_replay_invariant() {
    let baseline_repository = MaterializedCompilerIndexRepository::fixture();
    let baseline = replay_observation(&baseline_repository, &R7_QUERY_KINDS);

    for permutation in 0..R7_DETERMINISM_PERMUTATIONS {
        let repository = MaterializedCompilerIndexRepository::fixture();
        let query_kind = R7_QUERY_KINDS
            [usize::try_from(permutation).expect("R7 permutation index") % R7_QUERY_KINDS.len()];
        let observed = replay_observation(&repository, &[query_kind]);
        assert_eq!(
            observed.semantic, baseline.semantic,
            "R7 public replay permutation {permutation} changed semantic bytes"
        );
        assert_eq!(
            observed.documentation_manifest, baseline.documentation_manifest,
            "R7 public replay permutation {permutation} changed documentation manifest bytes"
        );
        assert_eq!(
            observed.documents, baseline.documents,
            "R7 public replay permutation {permutation} changed document bytes"
        );
        assert_eq!(
            observed.queries.get(query_kind),
            baseline.queries.get(query_kind),
            "R7 public replay permutation {permutation} changed {query_kind} query bytes"
        );
    }

    let isolated_repository = MaterializedCompilerIndexRepository::fixture();
    assert_eq!(
        replay_observation(&isolated_repository, &R7_QUERY_KINDS),
        baseline,
        "isolated R7 replay changed semantic/docs/query bytes"
    );
}

#[test]
fn e2e_fr_doc_001_r7_provenance_conflict_and_gap_wording() {
    let repository = MaterializedCompilerIndexRepository::fixture();
    let snapshot = successful_snapshot(&repository);
    let docs = repository.docs();
    assert_success(&docs, "R7 documentation");
    let manifest: Value = serde_json::from_slice(&docs.stdout).expect("parse R7 docs manifest");
    let markdown = manifest["documents"]
        .as_array()
        .expect("R7 generated documents")
        .iter()
        .map(|document| {
            let path = document["path"].as_str().expect("R7 document path");
            fs::read_to_string(repository.documents.join(path)).expect("read R7 document")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let reviewed = expected_overlay();
    for statement in reviewed["documentation_statements"]
        .as_array()
        .expect("reviewed R7 documentation statements")
    {
        let statement = statement.as_str().expect("reviewed documentation text");
        assert_eq!(
            markdown.matches(statement).count(),
            1,
            "reviewed R7 documentation statement changed"
        );
    }
    assert!(markdown.contains("compiler_index.syntax_uncertainty_retained"));
    for gap in reviewed["coverage_gaps"]
        .as_array()
        .expect("reviewed R7 coverage gaps")
    {
        assert!(
            markdown.contains(gap["capability"].as_str().expect("coverage capability")),
            "R7 coverage gap is undocumented"
        );
    }
    assert_public_bytes_are_redacted(&docs.stdout);
    assert_public_bytes_are_redacted(markdown.as_bytes());
    assert_eq!(
        snapshot["semantic"]["knowledge_graph"]["compiler_index"]["producer"]["attested"],
        false
    );
}

#[test]
fn conf_fr_qry_001_v10_uses_local_query_result_v5() {
    let repository = MaterializedCompilerIndexRepository::fixture();
    let snapshot = successful_snapshot(&repository);
    let docs = repository.docs();
    assert_success(&docs, "R7 query documentation setup");
    let manifest: Value = serde_json::from_slice(&docs.stdout).expect("parse R7 docs manifest");
    let graph = &snapshot["semantic"]["knowledge_graph"];
    let entity_id = graph["entities"]
        .as_array()
        .expect("R7 entities")
        .iter()
        .find(|entity| entity["kind"] == "compiler.symbol")
        .and_then(|entity| entity["id"].as_str())
        .expect("compiler symbol ID");
    let relationship_id = graph["relationships"]
        .as_array()
        .expect("R7 relationships")
        .iter()
        .find(|relationship| relationship["provenance"] == "validated_scip_v0.9.0")
        .and_then(|relationship| relationship["id"].as_str())
        .expect("compiler relationship ID");
    let claim_id = graph["claims"]
        .as_array()
        .expect("R7 claims")
        .iter()
        .find(|claim| claim["subject_id"] == entity_id)
        .and_then(|claim| claim["id"].as_str())
        .expect("compiler claim ID");
    let evidence_id = graph["evidence"]
        .as_array()
        .expect("R7 evidence")
        .iter()
        .find(|evidence| {
            evidence["artifact_sha256"]
                == "e1d3b4ca3c55b1a2779f7bea644fddc9557ddd30417fe8e4cf589e4089153a92"
        })
        .and_then(|evidence| evidence["id"].as_str())
        .expect("compiler evidence ID");
    let diagnostic_id = graph["diagnostics"]
        .as_array()
        .expect("R7 diagnostics")
        .iter()
        .find(|diagnostic| diagnostic["code"] == "compiler_index.syntax_uncertainty_retained")
        .and_then(|diagnostic| diagnostic["id"].as_str())
        .expect("compiler diagnostic ID");
    let coverage_id = graph["coverage"]
        .as_array()
        .expect("R7 coverage")
        .iter()
        .find(|gap| gap["capability"] == "compiler_index.arguments_redacted")
        .and_then(|gap| gap["id"].as_str())
        .expect("empty-evidence compiler coverage ID");
    let document_id = manifest["documents"]
        .as_array()
        .expect("R7 documents")
        .iter()
        .find(|document| document["path"] == "overview.md")
        .and_then(|document| document["document_id"].as_str())
        .expect("R7 overview document ID");

    for (kind, requested_id) in [
        ("entity", entity_id),
        ("relationship", relationship_id),
        ("claim", claim_id),
        ("evidence", evidence_id),
        ("diagnostic", diagnostic_id),
        ("coverage_gap", coverage_id),
        ("document", document_id),
    ] {
        let first = repository.query(requested_id);
        assert_success(&first, &format!("R7 {kind} query"));
        let replay = repository.query(requested_id);
        assert_eq!(replay.status.code(), Some(0));
        assert_eq!(replay.stdout, first.stdout, "R7 {kind} replay changed");
        let result: Value = serde_json::from_slice(&first.stdout).expect("parse R7 exact query");
        assert_eq!(result["schema_version"], "codenoesis.local-query-result/v5");
        assert_eq!(result["requested_id"], requested_id);
        assert_eq!(result["result_kind"], kind);
        assert_public_bytes_are_redacted(&first.stdout);
    }
}

#[test]
fn sec_fr_ext_005_never_executes_indexer_or_target() {
    let repository = MaterializedCompilerIndexRepository::fixture();
    assert!(!repository.worktree.join("target").exists());
    let _ = successful_snapshot(&repository);
    assert!(!repository.build_sentinel().exists());
    assert!(!repository.indexer_sentinel().exists());
    assert!(
        !repository.worktree.join("target").exists(),
        "R7 product created or executed a repository target"
    );
}

#[test]
fn sec_fr_ext_005_binding_path_race_and_privacy() {
    let repository = MaterializedCompilerIndexRepository::fixture();
    let snapshot = successful_snapshot(&repository);
    let docs = repository.docs();
    assert_success(&docs, "R7 privacy documentation");
    let public_snapshot = serde_json::to_vec(&snapshot).expect("serialize R7 public snapshot");
    assert_public_bytes_are_redacted(&public_snapshot);
    assert_public_bytes_are_redacted(&docs.stdout);
    for document in fs::read_dir(&repository.documents).expect("read R7 documents root") {
        let document = document.expect("read R7 document entry");
        if document.file_type().expect("R7 document type").is_file() {
            assert_public_bytes_are_redacted(
                &fs::read(document.path()).expect("read R7 public document"),
            );
        }
    }

    let changed = MaterializedCompilerIndexRepository::fixture();
    fs::write(
        changed.artifact_path(),
        b"changed between explicit binding and import\n",
    )
    .expect("replace R7 artifact before import");
    let output = changed.scan();
    assert_r7_failure(
        &changed,
        &output,
        "extraction.compiler_index_binding_mismatch",
    );

    let unsafe_parent = MaterializedCompilerIndexRepository::fixture();
    let mut binding: Value = serde_json::from_slice(
        &fs::read(unsafe_parent.binding_path()).expect("read R7 binding for path mutation"),
    )
    .expect("parse R7 binding for path mutation");
    binding["artifact"]["path"] = Value::String("../index.scip".to_owned());
    fs::write(
        unsafe_parent.binding_path(),
        serde_json::to_vec(&binding).expect("serialize unsafe R7 binding"),
    )
    .expect("write unsafe R7 binding");
    let output = unsafe_parent.scan();
    assert_r7_failure(&unsafe_parent, &output, "input.unsafe_compiler_index_path");
}

#[test]
fn reg_fr_cli_001_r7_selector_absence_is_byte_identical() {
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

#[allow(clippy::too_many_lines)]
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
    let reviewed_symbols = expected["compiler_symbols"]
        .as_array()
        .expect("reviewed compiler symbols");
    assert_eq!(
        symbols.len(),
        reviewed_symbols.len(),
        "reviewed compiler-symbol count changed"
    );
    let mut states = BTreeMap::<&str, usize>::new();
    for symbol in &symbols {
        let state = symbol["binding_state"]
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

    let graph_evidence = graph["evidence"].as_array().expect("R7 graph evidence");
    let expected_compiler_evidence = reviewed_symbols
        .iter()
        .map(|symbol| symbol["compiler_evidence"].clone())
        .chain(
            expected["relationship_evidence"]
                .as_array()
                .expect("reviewed relationship evidence")
                .iter()
                .cloned(),
        )
        .map(|evidence| {
            (
                evidence["id"]
                    .as_str()
                    .expect("reviewed compiler evidence ID")
                    .to_owned(),
                evidence,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let actual_compiler_evidence = graph_evidence
        .iter()
        .filter(|evidence| evidence["artifact_sha256"] == expected["artifact_sha256"])
        .map(|evidence| {
            let logical = logical_compiler_evidence(evidence);
            (
                logical["id"]
                    .as_str()
                    .expect("R7 compiler evidence ID")
                    .to_owned(),
                logical,
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(actual_compiler_evidence, expected_compiler_evidence);

    let expected_source_evidence = reviewed_symbols
        .iter()
        .filter_map(|symbol| symbol.get("source_evidence"))
        .filter(|evidence| !evidence.is_null())
        .map(|evidence| {
            let mut public = evidence.clone();
            public
                .as_object_mut()
                .expect("reviewed source evidence object")
                .remove("source_sha256");
            (
                public["id"]
                    .as_str()
                    .expect("reviewed source evidence ID")
                    .to_owned(),
                public,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let actual_source_evidence = graph_evidence
        .iter()
        .filter(|evidence| {
            evidence["id"]
                .as_str()
                .is_some_and(|id| expected_source_evidence.contains_key(id))
        })
        .map(|evidence| {
            (
                evidence["id"]
                    .as_str()
                    .expect("R7 source evidence ID")
                    .to_owned(),
                evidence.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(actual_source_evidence, expected_source_evidence);

    for reviewed in reviewed_symbols {
        let actual = symbols
            .iter()
            .find(|symbol| symbol["id"] == reviewed["id"])
            .expect("reviewed compiler symbol");
        for field in [
            "id",
            "symbol",
            "display_name",
            "binding_state",
            "identity_preimage",
        ] {
            assert_eq!(actual[field], reviewed[field], "compiler symbol {field}");
        }
        assert_eq!(actual["kind"], "compiler.symbol");
        assert_eq!(actual["scope"], reviewed["identity_preimage"][0]);
        assert_eq!(
            actual["compiler_evidence_ids"],
            json!([reviewed["compiler_evidence"]["id"].clone()])
        );
        let expected_source_ids = reviewed["source_evidence"]
            .as_object()
            .map(|evidence| vec![Value::String(evidence["id"].as_str().unwrap().to_owned())])
            .unwrap_or_default();
        assert_eq!(
            actual["source_evidence_ids"],
            Value::Array(expected_source_ids)
        );
    }

    let expected_symbol_ids = reviewed_symbols
        .iter()
        .map(|symbol| symbol["id"].as_str().expect("reviewed symbol ID"))
        .collect::<BTreeSet<_>>();
    let indexed_symbol_ids = compiler_index["compiler_symbol_ids"]
        .as_array()
        .expect("R7 compiler symbol index")
        .iter()
        .map(|id| id.as_str().expect("R7 indexed compiler symbol ID"))
        .collect::<BTreeSet<_>>();
    assert_eq!(indexed_symbol_ids, expected_symbol_ids);

    let reviewed_relationships = expected["relationships"]
        .as_array()
        .expect("reviewed R7 relationships");
    let actual_relationships = graph["relationships"]
        .as_array()
        .expect("R7 graph relationships")
        .iter()
        .filter(|relationship| relationship["provenance"] == "validated_scip_v0.9.0")
        .collect::<Vec<_>>();
    assert_eq!(actual_relationships.len(), reviewed_relationships.len());
    let mut relationship_counts = BTreeMap::<&str, usize>::new();
    for reviewed in reviewed_relationships {
        let actual = actual_relationships
            .iter()
            .find(|relationship| relationship["id"] == reviewed["id"])
            .expect("reviewed compiler relationship");
        for field in ["id", "kind", "source", "target", "evidence_ids"] {
            assert_eq!(
                actual[field], reviewed[field],
                "compiler relationship {field}"
            );
        }
        assert_eq!(actual["provenance"], "validated_scip_v0.9.0");
        assert_eq!(actual["endpoint_binding"], "unique");
        let kind = actual["kind"].as_str().expect("relationship kind");
        *relationship_counts.entry(kind).or_default() += 1;
    }
    for (kind, count) in expected["relationship_counts"]
        .as_object()
        .expect("reviewed relationship counts")
    {
        assert_eq!(
            relationship_counts.get(kind.as_str()).copied(),
            count
                .as_u64()
                .map(|value| usize::try_from(value).expect("relationship count fits usize"))
        );
    }
    let expected_relationship_ids = reviewed_relationships
        .iter()
        .map(|relationship| {
            relationship["id"]
                .as_str()
                .expect("reviewed relationship ID")
        })
        .collect::<BTreeSet<_>>();
    let indexed_relationship_ids = compiler_index["compiler_relationship_ids"]
        .as_array()
        .expect("R7 compiler relationship index")
        .iter()
        .map(|id| id.as_str().expect("R7 indexed relationship ID"))
        .collect::<BTreeSet<_>>();
    assert_eq!(indexed_relationship_ids, expected_relationship_ids);

    for forbidden in expected["forbidden_relationship_kinds"]
        .as_array()
        .expect("reviewed forbidden relationship kinds")
    {
        assert!(
            graph["relationships"]
                .as_array()
                .expect("R7 relationships")
                .iter()
                .all(|relationship| relationship["kind"] != *forbidden),
            "forbidden R7 relationship kind {forbidden}"
        );
    }

    let syntax_bindings = expected["syntax_reference_bindings"]
        .as_array()
        .expect("reviewed syntax-reference bindings");
    for binding in syntax_bindings {
        let actual = graph["entities"]
            .as_array()
            .expect("R7 graph entities")
            .iter()
            .find(|entity| entity["id"] == binding["id"])
            .expect("reviewed syntax-reference entity");
        assert_eq!(actual["kind"], "rust.symbol_reference");
        assert_eq!(actual["name"], binding["spelling"]);
        assert_eq!(
            actual["properties"]["source_range"],
            binding["range"]
                .as_array()
                .expect("reviewed syntax range")
                .iter()
                .map(|part| part.as_u64().expect("syntax range part").to_string())
                .collect::<Vec<_>>()
                .join(":")
        );
    }

    let compiler_subjects = expected_symbol_ids
        .iter()
        .copied()
        .chain(expected_relationship_ids.iter().copied())
        .chain(syntax_bindings.iter().map(|binding| {
            binding["id"]
                .as_str()
                .expect("reviewed syntax-reference ID")
        }))
        .collect::<BTreeSet<_>>();
    let compiler_claims = graph["claims"]
        .as_array()
        .expect("R7 claims")
        .iter()
        .filter(|claim| {
            claim["subject_id"]
                .as_str()
                .is_some_and(|subject| compiler_subjects.contains(subject))
        })
        .collect::<Vec<_>>();
    assert_eq!(compiler_claims.len(), compiler_subjects.len());
    assert!(compiler_claims.iter().all(|claim| {
        claim["state"] == expected["claims"]["compiler_symbol_state"]
            && claim["evidence_ids"]
                .as_array()
                .is_some_and(|evidence| !evidence.is_empty())
    }));

    let diagnostics = graph["diagnostics"]
        .as_array()
        .expect("R7 diagnostics")
        .iter()
        .filter(|diagnostic| {
            diagnostic["code"]
                .as_str()
                .is_some_and(|code| code.starts_with("compiler_index."))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        diagnostics.len(),
        expected["diagnostics"].as_array().unwrap().len()
    );
    for reviewed in expected["diagnostics"].as_array().unwrap() {
        let actual = diagnostics
            .iter()
            .find(|diagnostic| diagnostic["code"] == reviewed["code"])
            .expect("reviewed R7 diagnostic");
        for field in ["code", "subject_id", "compiler_target_id"] {
            assert_eq!(
                actual[field], reviewed[field],
                "compiler diagnostic {field}"
            );
        }
    }

    let coverage = graph["coverage"]
        .as_array()
        .expect("R7 coverage")
        .iter()
        .filter(|gap| {
            gap["capability"]
                .as_str()
                .is_some_and(|capability| capability.starts_with("compiler_index."))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        coverage.len(),
        expected["coverage_gaps"].as_array().unwrap().len()
    );
    for reviewed in expected["coverage_gaps"].as_array().unwrap() {
        let actual = coverage
            .iter()
            .find(|gap| gap["capability"] == reviewed["capability"])
            .expect("reviewed R7 coverage gap");
        for field in ["subject", "capability", "state"] {
            assert_eq!(actual[field], reviewed[field], "compiler coverage {field}");
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct ReplayObservation {
    semantic: Vec<u8>,
    documentation_manifest: Vec<u8>,
    documents: BTreeMap<String, Vec<u8>>,
    queries: BTreeMap<String, Vec<u8>>,
}

fn replay_observation(
    repository: &MaterializedCompilerIndexRepository,
    query_kinds: &[&str],
) -> ReplayObservation {
    let scan = repository.scan();
    assert_success(&scan, "R7 deterministic scan");
    let snapshot: Value =
        serde_json::from_slice(&scan.stdout).expect("parse deterministic R7 scan");
    let semantic =
        serde_json::to_vec(&snapshot["semantic"]).expect("serialize R7 semantic payload");

    let docs = repository.docs();
    assert_success(&docs, "R7 deterministic documentation");
    let manifest: Value =
        serde_json::from_slice(&docs.stdout).expect("parse deterministic R7 docs manifest");
    let documents = manifest["documents"]
        .as_array()
        .expect("deterministic R7 documents")
        .iter()
        .map(|document| {
            let path = document["path"].as_str().expect("R7 document path");
            (
                path.to_owned(),
                fs::read(repository.documents.join(path)).expect("read deterministic R7 document"),
            )
        })
        .collect();

    let query_ids = reviewed_query_ids(&snapshot, &manifest);
    let queries = query_kinds
        .iter()
        .map(|kind| {
            let requested_id = query_ids
                .get(*kind)
                .unwrap_or_else(|| panic!("unknown deterministic R7 query kind: {kind}"));
            let output = repository.query(requested_id);
            assert_success(&output, &format!("R7 deterministic {kind} query"));
            assert_public_bytes_are_redacted(&output.stdout);
            ((*kind).to_owned(), output.stdout)
        })
        .collect();

    ReplayObservation {
        semantic,
        documentation_manifest: docs.stdout,
        documents,
        queries,
    }
}

fn reviewed_query_ids(snapshot: &Value, manifest: &Value) -> BTreeMap<String, String> {
    let expected = expected_overlay();
    let graph = &snapshot["semantic"]["knowledge_graph"];
    let entity = expected["compiler_symbols"][0]["id"]
        .as_str()
        .expect("reviewed compiler entity ID");
    let relationship = expected["relationships"][0]["id"]
        .as_str()
        .expect("reviewed compiler relationship ID");
    let claim = graph["claims"]
        .as_array()
        .expect("R7 claims")
        .iter()
        .find(|claim| claim["subject_id"] == entity)
        .and_then(|claim| claim["id"].as_str())
        .expect("R7 compiler entity claim ID");
    let evidence = expected["compiler_symbols"][0]["compiler_evidence"]["id"]
        .as_str()
        .expect("reviewed compiler evidence ID");
    let diagnostic = graph["diagnostics"]
        .as_array()
        .expect("R7 diagnostics")
        .iter()
        .find(|diagnostic| diagnostic["code"] == "compiler_index.syntax_uncertainty_retained")
        .and_then(|diagnostic| diagnostic["id"].as_str())
        .expect("R7 compiler diagnostic ID");
    let coverage = graph["coverage"]
        .as_array()
        .expect("R7 coverage")
        .iter()
        .find(|gap| gap["capability"] == "compiler_index.arguments_redacted")
        .and_then(|gap| gap["id"].as_str())
        .expect("R7 compiler coverage ID");
    let document = manifest["documents"]
        .as_array()
        .expect("R7 documents")
        .iter()
        .find(|document| document["path"] == "overview.md")
        .and_then(|document| document["document_id"].as_str())
        .expect("R7 overview document ID");

    [
        ("entity", entity),
        ("relationship", relationship),
        ("claim", claim),
        ("evidence", evidence),
        ("diagnostic", diagnostic),
        ("coverage_gap", coverage),
        ("document", document),
    ]
    .into_iter()
    .map(|(kind, id)| (kind.to_owned(), id.to_owned()))
    .collect()
}

fn logical_compiler_evidence(evidence: &Value) -> Value {
    let record_kind = evidence["record_kind"]
        .as_str()
        .expect("R7 compiler evidence record kind");
    let locator = match record_kind {
        "occurrence" => json!({
            "record_kind": record_kind,
            "document_path": evidence["document_path"],
            "range": evidence["range"],
            "symbol": evidence["symbol"],
            "symbol_roles": evidence["symbol_roles"]
        }),
        "external_symbol" => json!({
            "record_kind": record_kind,
            "symbol": evidence["symbol"]
        }),
        "occurrence_resolution" | "occurrence_reference" => json!({
            "record_kind": record_kind,
            "source_symbol": evidence["symbol"],
            "target_symbol": evidence["relationship_target"],
            "document_path": evidence["document_path"],
            "range": evidence["range"]
        }),
        "symbol_relationship" => json!({
            "record_kind": record_kind,
            "source_symbol": evidence["symbol"],
            "target_symbol": evidence["relationship_target"],
            "flags": evidence["relationship_flags"]
        }),
        other => panic!("unsupported R7 compiler evidence kind: {other}"),
    };
    json!({
        "id": evidence["id"],
        "artifact_sha256": evidence["artifact_sha256"],
        "locator": locator
    })
}

fn successful_snapshot(repository: &MaterializedCompilerIndexRepository) -> Value {
    let output = repository.scan();
    assert_success(&output, "R7 scan");
    serde_json::from_slice(&output.stdout).expect("parse RepositorySnapshotV10")
}

fn base_r7_command(repository: &MaterializedCompilerIndexRepository) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_noesis"));
    command
        .current_dir(&repository.root)
        .args(["scan", "--repository"])
        .arg(&repository.worktree)
        .args([
            "--repository-id",
            support::s4_r7::REPOSITORY_ID,
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
            "--rust-semantic-profile",
            "rust-semantic-depth-v1",
            "--rust-framework-profile",
            "rust-framework-declarations-v1",
        ])
        .arg("--store")
        .arg(&repository.store)
        .args(["--format", "json"]);
    command
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

#[allow(clippy::naive_bytecount)]
fn assert_r7_failure(
    repository: &MaterializedCompilerIndexRepository,
    output: &Output,
    expected_code: &str,
) {
    assert_eq!(output.status.code(), Some(10));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.ends_with(b"\n"));
    assert_eq!(
        output.stderr.iter().filter(|byte| **byte == b'\n').count(),
        1
    );
    let error: Value = serde_json::from_slice(&output.stderr).expect("parse strict ErrorV14");
    assert_eq!(error["schema_version"], "codenoesis.error/v14");
    assert_eq!(
        error["code"],
        expected_code,
        "unexpected R7 failure: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(error["retryable"], false);
    assert!(!repository.store.exists(), "R7 failure published a store");
    assert!(
        !repository.documents.exists(),
        "R7 failure published documents"
    );
}

fn assert_public_bytes_are_redacted(bytes: &[u8]) {
    let text = String::from_utf8_lossy(bytes);
    for private in [
        "R7_SECRET_ARGUMENT_CANARY",
        "R7_DOCUMENTATION_CANARY",
        "R7_EXTERNAL_DOCUMENTATION_CANARY",
        "file:///private/compiler-index-fixture",
        "--config=",
    ] {
        assert!(
            !text.contains(private),
            "R7 private value leaked: {private}"
        );
    }
}

#[allow(clippy::too_many_lines)]
fn exercise_invalid_case(
    case_id: &str,
    repository: &MaterializedCompilerIndexRepository,
) -> Output {
    match case_id {
        "artifact-digest-mismatch" | "mutable-artifact-race" => {
            let mut bytes = repository.artifact_bytes();
            let last = bytes.last_mut().expect("nonempty R7 artifact");
            *last ^= 1;
            fs::write(repository.artifact_path(), bytes).expect("write mismatched R7 artifact");
        }
        "binding-revision-mismatch" => repository.mutate_binding(|binding| {
            binding["repository"]["commit_oid"] = Value::String("0".repeat(40));
        }),
        "duplicate-metadata" => repository.write_bound_artifact(&[0x0a, 0x00, 0x0a, 0x00]),
        "forbidden-indexer-execution" => {
            return base_r7_command(repository)
                .args(["--compiler-index-profile", "scip-rust-v0.9.0-import-v1"])
                .output()
                .expect("launch incomplete R7 composition");
        }
        "incomplete-declared-document" => {
            repository.mutate_binding(|binding| {
                binding["documents"]["indexed"]
                    .as_array_mut()
                    .expect("materialized indexed documents")
                    .pop();
            });
            repository.refresh_source_manifest();
        }
        "invalid-scip-symbol" => {
            append_external_symbol(repository, "not a valid scip symbol", "invalid", &[]);
        }
        "limit-plus-one-binding-json-bytes" => {
            fs::write(repository.binding_path(), vec![b' '; 1_048_577])
                .expect("write oversized R7 binding");
        }
        "limit-plus-one-documents" => {
            repository.write_bound_artifact(&index_with_repeated(2, &[], 20_001));
        }
        "limit-plus-one-occurrences-per-document" => {
            let document = repeated_message_fields(2, &[], 100_001);
            repository.write_bound_artifact(&index_with_message(2, &document));
        }
        "limit-plus-one-occurrences-total" => {
            let mut bytes = metadata_only_index();
            let full = repeated_message_fields(2, &[], 100_000);
            for _ in 0..10 {
                append_message_field(&mut bytes, 2, &full);
            }
            append_message_field(&mut bytes, 2, &message_field(2, &[]));
            repository.write_bound_artifact(&bytes);
        }
        "limit-plus-one-protobuf-recursion" => {
            let mut bytes = metadata_only_index();
            bytes.extend(std::iter::repeat_n(0x23, 65));
            bytes.extend(std::iter::repeat_n(0x24, 65));
            repository.write_bound_artifact(&bytes);
        }
        "limit-plus-one-raw-index-bytes" => {
            File::create(repository.artifact_path())
                .expect("create oversized R7 artifact")
                .set_len(67_108_865)
                .expect("size oversized R7 artifact");
        }
        "limit-plus-one-relationships-total" => {
            let symbol = repeated_message_fields(4, &[], 500_001);
            repository.write_bound_artifact(&index_with_message(3, &symbol));
        }
        "limit-plus-one-symbol-information-total" => {
            repository.write_bound_artifact(&index_with_repeated(3, &[], 250_001));
        }
        "limit-plus-one-symbol-or-display-bytes" => {
            let symbol = string_field(1, &"s".repeat(16_385));
            repository.write_bound_artifact(&index_with_message(3, &symbol));
        }
        "limit-plus-one-tool-argument-bytes" => {
            let tool = string_field(3, &"a".repeat(4_097));
            let metadata = message_field(2, &tool);
            repository.write_bound_artifact(&message_field(1, &metadata));
        }
        "limit-plus-one-tool-arguments" => {
            let tool = repeated_message_fields(3, &[], 129);
            let metadata = message_field(2, &tool);
            repository.write_bound_artifact(&message_field(1, &metadata));
        }
        "limit-plus-one-unpromoted-value-bytes" => {
            let metadata = string_field(3, &"p".repeat(65_537));
            repository.write_bound_artifact(&message_field(1, &metadata));
        }
        "malformed-truncated-varint" => {
            let mut bytes = metadata_only_index();
            bytes.extend_from_slice(&[0x12, 0x80]);
            repository.write_bound_artifact(&bytes);
        }
        "nfc-symbol-collision" => append_external_symbol(
            repository,
            "rust-analyzer cargo api 0.1.0 api/`Cafe\u{301}`#",
            "Cafe\u{301}",
            &[],
        ),
        "noncanonical-varint" => repository.write_bound_artifact(&[0x8a, 0x00, 0x00]),
        "privacy-argument-canary" => {
            let mut bytes = repository.artifact_bytes();
            replace_once(&mut bytes, b"\x1a\x04scip", b"\x1a\x04User");
            repository.write_bound_artifact(&bytes);
            repository.bind_arguments(&["User", ".", "--config=R7_SECRET_ARGUMENT_CANARY"]);
        }
        "relation-authority-conflict" => {
            let target = "rust-analyzer cargo core 1.97.1 core/option/Option#";
            append_external_symbol(
                repository,
                "rust-analyzer cargo conflict 1.0.0 conflict/Source#",
                "Source",
                &[(target, 3), (target, 4)],
            );
        }
        "required-endpoint-ambiguous" => append_external_symbol(
            repository,
            "rust-analyzer cargo api 0.1.0 get().",
            "get",
            &[],
        ),
        "symlink-artifact-escape" => replace_artifact_with_symlink(repository),
        "unknown-nested-field" => {
            repository.write_bound_artifact(&message_field(1, &message_field(5, &[])));
        }
        "unsafe-artifact-parent" => repository.mutate_binding(|binding| {
            binding["artifact"]["path"] = Value::String("../index.scip".to_owned());
        }),
        "unsupported-position-encoding" => {
            let mut bytes = repository.artifact_bytes();
            replace_once(&mut bytes, &[0x20, 0x01], &[0x20, 0x02]);
            repository.write_bound_artifact(&bytes);
        }
        other => panic!("unimplemented R7 invalid case: {other}"),
    }
    repository.scan()
}

fn metadata_only_index() -> Vec<u8> {
    message_field(1, &[])
}

fn index_with_message(field: u32, value: &[u8]) -> Vec<u8> {
    let mut bytes = metadata_only_index();
    append_message_field(&mut bytes, field, value);
    bytes
}

fn index_with_repeated(field: u32, value: &[u8], count: usize) -> Vec<u8> {
    let mut bytes = metadata_only_index();
    bytes.extend(repeated_message_fields(field, value, count));
    bytes
}

fn repeated_message_fields(field: u32, value: &[u8], count: usize) -> Vec<u8> {
    let encoded = message_field(field, value);
    let mut bytes = Vec::with_capacity(encoded.len().saturating_mul(count));
    for _ in 0..count {
        bytes.extend_from_slice(&encoded);
    }
    bytes
}

fn message_field(field: u32, value: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    append_varint(&mut bytes, u64::from(field) << 3 | 2);
    append_varint(
        &mut bytes,
        u64::try_from(value.len()).expect("protobuf field length"),
    );
    bytes.extend_from_slice(value);
    bytes
}

fn string_field(field: u32, value: &str) -> Vec<u8> {
    message_field(field, value.as_bytes())
}

fn bool_field(field: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    append_varint(&mut bytes, u64::from(field) << 3);
    bytes.push(1);
    bytes
}

fn append_message_field(bytes: &mut Vec<u8>, field: u32, value: &[u8]) {
    bytes.extend(message_field(field, value));
}

fn append_varint(bytes: &mut Vec<u8>, mut value: u64) {
    loop {
        let mut byte = u8::try_from(value & 0x7f).expect("protobuf varint byte");
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        bytes.push(byte);
        if value == 0 {
            return;
        }
    }
}

fn append_external_symbol(
    repository: &MaterializedCompilerIndexRepository,
    symbol: &str,
    display_name: &str,
    relationships: &[(&str, u32)],
) {
    let mut information = string_field(1, symbol);
    for (target, flag) in relationships {
        let mut relationship = string_field(1, target);
        relationship.extend(bool_field(*flag));
        append_message_field(&mut information, 4, &relationship);
    }
    information.extend(string_field(6, display_name));
    let mut bytes = repository.artifact_bytes();
    append_message_field(&mut bytes, 3, &information);
    repository.write_bound_artifact(&bytes);
}

fn replace_once(bytes: &mut [u8], source: &[u8], replacement: &[u8]) {
    assert_eq!(source.len(), replacement.len());
    let offset = bytes
        .windows(source.len())
        .position(|window| window == source)
        .expect("reviewed protobuf sequence");
    bytes[offset..offset + source.len()].copy_from_slice(replacement);
}

#[cfg(unix)]
fn replace_artifact_with_symlink(repository: &MaterializedCompilerIndexRepository) {
    use std::os::unix::fs::symlink;

    let outside = repository.root.join("outside-index.scip");
    fs::write(&outside, repository.artifact_bytes()).expect("write outside R7 artifact");
    fs::remove_file(repository.artifact_path()).expect("remove materialized R7 artifact");
    symlink(&outside, repository.artifact_path()).expect("create R7 artifact symlink");
}

#[cfg(not(unix))]
fn replace_artifact_with_symlink(repository: &MaterializedCompilerIndexRepository) {
    fs::remove_file(repository.artifact_path()).expect("remove materialized R7 artifact");
    fs::create_dir(repository.artifact_path()).expect("create unsafe R7 artifact replacement");
}
