mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::process::Command;

use serde_json::Value;

use support::s4_r4::MaterializedCargoManifestRepository;
use support::s4_r6::MaterializedFrameworkDeclarationsRepository;

const PRE_R6_STDERR: &[u8] = b"{\"code\":\"input.invalid_revision\",\"context\":{},\"message\":\"invalid revision\",\"retryable\":false,\"schema_version\":\"codenoesis.error/v4\",\"stage\":\"input\"}\n";

#[test]
fn e2e_fr_ext_011_framework_declarations() {
    let repository = MaterializedFrameworkDeclarationsRepository::fixture();
    assert!(!repository.build_sentinel().exists());
    let output = repository.scan();

    if output.status.code() == Some(2) {
        assert!(output.stdout.is_empty(), "pre-R6 stdout changed");
        assert_eq!(output.stderr, PRE_R6_STDERR, "pre-R6 stderr changed");
        assert!(!repository.store.exists(), "pre-R6 store was created");
        assert!(
            !repository.documents.exists(),
            "pre-R6 documents root was created"
        );
        assert!(
            !repository.build_sentinel().exists(),
            "pre-R6 subject executed build.rs"
        );
        panic!(
            "expected RepositorySnapshotV9 success; observed approved unknown Rust framework selector Red"
        );
    }

    assert!(
        output.status.success(),
        "R6 scan failed: status={:?}, stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "successful R6 stderr changed");
    assert!(
        !repository.build_sentinel().exists(),
        "R6 subject executed build.rs"
    );
    let snapshot: Value =
        serde_json::from_slice(&output.stdout).expect("parse RepositorySnapshotV9");
    assert_eq!(
        snapshot["schema_version"],
        "codenoesis.repository-snapshot/v9"
    );
    assert_eq!(
        snapshot["semantic"]["configuration"]["rust_framework_profile"],
        "rust-framework-declarations-v1"
    );
    assert_eq!(
        snapshot["semantic"]["knowledge_graph"]["ontology_version"],
        "codenoesis.ontology/rust/v6"
    );
    assert_framework_golden(&snapshot);
}

#[test]
fn e2e_fr_doc_001_r6_declaration_candidate_non_runtime_wording() {
    let repository = MaterializedFrameworkDeclarationsRepository::fixture();
    let scan = repository.scan();
    assert!(scan.status.success(), "R6 setup scan failed");
    let expected = expected_facts();

    let docs = repository.docs();
    assert!(
        docs.status.success(),
        "R6 docs failed: status={:?}, stderr={}",
        docs.status.code(),
        String::from_utf8_lossy(&docs.stderr)
    );
    let manifest: Value = serde_json::from_slice(&docs.stdout).expect("parse R6 docs manifest");
    let statements = manifest["documents"]
        .as_array()
        .expect("R6 documents")
        .iter()
        .flat_map(|document| {
            document["statements"]
                .as_array()
                .expect("R6 document statements")
        })
        .collect::<Vec<_>>();
    let documented_subjects = statements
        .iter()
        .flat_map(|statement| {
            statement["subject_ids"]
                .as_array()
                .expect("R6 statement subjects")
        })
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    let documented_gaps = statements
        .iter()
        .flat_map(|statement| {
            statement["coverage_gap_ids"]
                .as_array()
                .expect("R6 statement coverage")
        })
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    for declaration in expected["declarations"]
        .as_array()
        .expect("reviewed declarations")
    {
        assert!(
            documented_subjects.contains(declaration["id"].as_str().expect("declaration ID")),
            "R6 declaration is undocumented"
        );
    }
    for diagnostic in expected["diagnostics"]
        .as_array()
        .expect("reviewed diagnostics")
    {
        assert!(
            documented_subjects.contains(diagnostic["id"].as_str().expect("diagnostic ID")),
            "R6 diagnostic is undocumented"
        );
    }
    for gap in expected["coverage_gaps"]
        .as_array()
        .expect("reviewed coverage gaps")
    {
        assert!(
            documented_gaps.contains(gap["id"].as_str().expect("coverage-gap ID")),
            "R6 coverage gap is undocumented"
        );
    }

    let markdown = manifest["documents"]
        .as_array()
        .expect("R6 generated documents")
        .iter()
        .map(|document| {
            let path = document["path"].as_str().expect("R6 document path");
            fs::read_to_string(repository.documents.join(path)).expect("read R6 document")
        })
        .collect::<Vec<_>>()
        .join("\n");
    for statement in expected["document_statements"]
        .as_array()
        .expect("reviewed documentation statements")
    {
        assert!(
            markdown.contains(statement.as_str().expect("documentation statement")),
            "reviewed R6 non-runtime wording changed"
        );
    }
    assert!(markdown.contains("unresolved candidate"));
    assert!(markdown.contains("not observed runtime behavior"));
}

#[test]
fn e2e_fr_qry_001_r6_exact_id_results() {
    let repository = MaterializedFrameworkDeclarationsRepository::fixture();
    let scan = repository.scan();
    assert!(scan.status.success(), "R6 setup scan failed");
    let snapshot: Value = serde_json::from_slice(&scan.stdout).expect("parse V9 query snapshot");
    let docs = repository.docs();
    assert!(docs.status.success(), "R6 setup docs failed");
    let manifest: Value = serde_json::from_slice(&docs.stdout).expect("parse R6 query manifest");
    let expected = expected_facts();
    let graph = &snapshot["semantic"]["knowledge_graph"];
    let entity_id = expected["declarations"][0]["id"]
        .as_str()
        .expect("reviewed framework entity ID");
    let relationship_id = graph["relationships"]
        .as_array()
        .expect("V9 relationships")
        .iter()
        .find(|relationship| relationship["target"] == entity_id)
        .and_then(|relationship| relationship["id"].as_str())
        .expect("framework owner relationship");
    let claim_id = graph["claims"]
        .as_array()
        .expect("V9 claims")
        .iter()
        .find(|claim| claim["subject_id"] == entity_id)
        .and_then(|claim| claim["id"].as_str())
        .expect("framework parser claim");
    let evidence_id = expected["declarations"][0]["evidence"]["id"]
        .as_str()
        .expect("reviewed SHA-256 evidence ID");
    let diagnostic_id = expected["diagnostics"][0]["id"]
        .as_str()
        .expect("reviewed diagnostic ID");
    let coverage_id = expected["coverage_gaps"][0]["id"]
        .as_str()
        .expect("reviewed coverage-gap ID");
    let document_id = manifest["documents"]
        .as_array()
        .expect("R6 documents")
        .iter()
        .find(|document| document["path"] == "overview.md")
        .and_then(|document| document["document_id"].as_str())
        .expect("R6 overview document ID");

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
        assert!(
            first.status.success(),
            "R6 {kind} query failed: {}",
            String::from_utf8_lossy(&first.stderr)
        );
        let replay = repository.query(requested_id);
        assert_eq!(replay.status.code(), Some(0));
        assert_eq!(replay.stdout, first.stdout, "R6 {kind} replay changed");
        let result: Value = serde_json::from_slice(&first.stdout).expect("parse exact R6 query");
        assert_eq!(result["schema_version"], "codenoesis.local-query-result/v4");
        assert_eq!(result["requested_id"], requested_id);
        assert_eq!(result["result_kind"], kind);
    }
}

#[test]
fn sec_fr_ext_011_invalid_composition_precedes_acquisition() {
    let invalid = MaterializedFrameworkDeclarationsRepository::fixture();
    let output =
        invalid.scan_with_profiles(true, true, Some("rust-semantic-depth-v1"), Some("unknown"));
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"{\"code\":\"input.invalid_rust_framework_profile\",\"context\":{\"profile\":\"unknown\"},\"message\":\"invalid rust framework profile\",\"retryable\":false,\"schema_version\":\"codenoesis.error/v13\",\"stage\":\"input\"}\n"
    );
    assert!(!invalid.store.exists());
    assert!(!invalid.build_sentinel().exists());

    let incomplete = MaterializedFrameworkDeclarationsRepository::fixture();
    let output = incomplete.scan_with_profiles(
        true,
        false,
        Some("rust-semantic-depth-v1"),
        Some("rust-framework-declarations-v1"),
    );
    assert_eq!(output.status.code(), Some(11));
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).expect("parse ErrorV13 composition");
    assert_eq!(error["schema_version"], "codenoesis.error/v13");
    assert_eq!(
        error["code"],
        "extraction.unsupported_framework_composition"
    );
    assert_eq!(
        error["context"]["required_profiles"],
        serde_json::json!([
            "standard-local-s4",
            "cargo-root-package-v1",
            "cargo-manifest-facts-v1",
            "rust-semantic-depth-v1"
        ])
    );
    assert!(!incomplete.store.exists());
    assert!(!incomplete.build_sentinel().exists());
}

#[test]
fn sec_fr_ext_011_r6_prerequisite_parse_failures_use_v13() {
    let duplicate = MaterializedFrameworkDeclarationsRepository::fixture();
    let output = base_r6_command(&duplicate)
        .args(["--workspace-profile", "cargo-root-package-v1"])
        .output()
        .expect("launch duplicate R6 prerequisite subject");
    assert_r6_composition_failure(&duplicate, &output);

    let missing = MaterializedFrameworkDeclarationsRepository::fixture();
    let output = base_r6_command(&missing)
        .arg("--manifest-profile")
        .output()
        .expect("launch missing R6 prerequisite value subject");
    assert_r6_composition_failure(&missing, &output);
}

#[test]
fn reg_fr_cli_001_r6_selector_absence_is_byte_identical() {
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

fn base_r6_command(repository: &MaterializedFrameworkDeclarationsRepository) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_noesis"));
    command
        .args(["scan", "--repository"])
        .arg(&repository.worktree)
        .args([
            "--repository-id",
            support::s4_r6::REPOSITORY_ID,
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
            "--store",
        ])
        .arg(&repository.store)
        .args(["--format", "json"]);
    command
}

fn assert_r6_composition_failure(
    repository: &MaterializedFrameworkDeclarationsRepository,
    output: &std::process::Output,
) {
    assert_eq!(output.status.code(), Some(11));
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).expect("parse ErrorV13 composition");
    assert_eq!(error["schema_version"], "codenoesis.error/v13");
    assert_eq!(
        error["code"],
        "extraction.unsupported_framework_composition"
    );
    assert!(!repository.store.exists());
    assert!(!repository.build_sentinel().exists());
}

#[allow(clippy::too_many_lines)]
fn assert_framework_golden(snapshot: &Value) {
    let expected = expected_facts();
    let graph = &snapshot["semantic"]["knowledge_graph"];
    let declarations = expected["declarations"]
        .as_array()
        .expect("reviewed framework declarations");
    let entities = graph["entities"].as_array().expect("V9 graph entities");
    assert_eq!(declarations.len(), 24);
    let mut counts = BTreeMap::new();
    for expected_declaration in declarations {
        let declaration_id = expected_declaration["id"].as_str().expect("declaration ID");
        let actual = entities
            .iter()
            .find(|entity| entity["id"] == declaration_id)
            .expect("reviewed framework declaration");
        for (actual_key, expected_key) in [
            ("kind", "entity_kind"),
            ("crate_id", "crate_identity"),
            ("lexical_owner_id", "lexical_owner_id"),
            ("role", "role"),
            ("source_profile", "source_profile"),
            ("source_form_identity", "source_form_identity"),
            ("declared_key_or_target", "declared_key_or_target"),
            ("epistemic_state", "epistemic_state"),
            ("compilation_presence", "compilation_presence"),
            ("method", "method"),
            ("path", "path"),
            ("configuration_key", "configuration_key"),
            ("target_spelling", "target_spelling"),
            ("local_target_id", "local_target_id"),
            ("target_binding", "target_binding"),
        ] {
            assert_eq!(
                actual[actual_key], expected_declaration[expected_key],
                "reviewed R6 field changed for {declaration_id}: {actual_key}"
            );
        }
        assert_eq!(
            actual["evidence_ids"],
            serde_json::json!([expected_declaration["evidence"]["id"]])
        );
        *counts
            .entry(actual["kind"].as_str().expect("framework kind"))
            .or_insert(0_usize) += 1;

        let expected_evidence = &expected_declaration["evidence"];
        let evidence = graph["evidence"]
            .as_array()
            .expect("V9 evidence")
            .iter()
            .find(|value| value["id"] == expected_evidence["id"])
            .expect("reviewed framework evidence");
        for key in ["path", "start_byte", "end_byte"] {
            assert_eq!(evidence[key], expected_evidence[key]);
        }
    }
    assert_eq!(
        counts,
        BTreeMap::from([
            ("framework.component_declaration", 4),
            ("framework.configuration_declaration", 2),
            ("framework.endpoint_declaration", 2),
            ("framework.handler_declaration", 3),
            ("framework.route_declaration", 10),
            ("framework.service_declaration", 3),
        ])
    );

    let declaration_ids = declarations
        .iter()
        .map(|value| value["id"].as_str().expect("declaration ID"))
        .collect::<BTreeSet<_>>();
    let framework_relationships = graph["relationships"]
        .as_array()
        .expect("V9 relationships")
        .iter()
        .filter(|relationship| {
            relationship["target"]
                .as_str()
                .is_some_and(|target| declaration_ids.contains(target))
        })
        .collect::<Vec<_>>();
    assert_eq!(framework_relationships.len(), 24);
    assert!(
        framework_relationships
            .iter()
            .all(|relationship| relationship["kind"] == "DEFINES")
    );
    for forbidden in expected["forbidden_relationship_kinds"]
        .as_array()
        .expect("forbidden relationships")
    {
        assert!(
            graph["relationships"]
                .as_array()
                .expect("V9 relationships")
                .iter()
                .all(|relationship| relationship["kind"] != *forbidden)
        );
    }

    assert_exact_artifacts(
        graph["diagnostics"].as_array().expect("V9 diagnostics"),
        expected["diagnostics"]
            .as_array()
            .expect("reviewed diagnostics"),
        "code",
        "message",
    );
    assert_exact_artifacts(
        graph["coverage"].as_array().expect("V9 coverage"),
        expected["coverage_gaps"]
            .as_array()
            .expect("reviewed coverage gaps"),
        "capability",
        "state",
    );
}

fn assert_exact_artifacts(
    actual: &[Value],
    expected: &[Value],
    first_field: &str,
    second_field: &str,
) {
    for reviewed in expected {
        let id = reviewed["id"].as_str().expect("reviewed artifact ID");
        let artifact = actual
            .iter()
            .find(|value| value["id"] == id)
            .expect("reviewed R6 artifact");
        assert_eq!(artifact[first_field], reviewed[first_field]);
        assert_eq!(artifact[second_field], reviewed[second_field]);
        assert_eq!(artifact["evidence_ids"], reviewed["evidence_ids"]);
    }
}

fn expected_facts() -> Value {
    support::read_json(&support::s4_r6::fixture_root().join("expected-framework-declarations.json"))
}
