mod support;

use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;

use support::s4_r4::MaterializedCargoManifestRepository;
use support::s4_r5::MaterializedRustSemanticRepository;

const PRE_R5_STDERR: &[u8] = b"{\"code\":\"input.invalid_revision\",\"context\":{},\"message\":\"invalid revision\",\"retryable\":false,\"schema_version\":\"codenoesis.error/v4\",\"stage\":\"input\"}\n";

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
