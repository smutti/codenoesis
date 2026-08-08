mod support;

use std::collections::BTreeSet;
use std::fs;

use serde_json::Value;

use support::s4_r3::MaterializedRootPackageRepository;

#[test]
fn e2e_fr_ext_008_root_package_workspace() {
    let repository = MaterializedRootPackageRepository::implicit();
    let output = repository.scan();

    assert!(
        output.status.success(),
        "R3 scan failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "successful R3 stderr changed");
    let snapshot: Value =
        serde_json::from_slice(&output.stdout).expect("parse RepositorySnapshotV6");
    assert_eq!(
        snapshot["schema_version"],
        "codenoesis.repository-snapshot/v6"
    );
    assert_eq!(
        snapshot["semantic"]["configuration"]["workspace_profile"],
        "cargo-root-package-v1"
    );
    assert_eq!(
        snapshot["semantic"]["knowledge_graph"]["workspace"]["root_shape"],
        "non_virtual_workspace"
    );
    assert_eq!(
        snapshot["semantic"]["repository_boundaries"]["summary"]["boundary_count"],
        1
    );
}

#[test]
fn e2e_fr_doc_001_r3_coverage_is_documented() {
    let repository = MaterializedRootPackageRepository::implicit();
    let snapshot = successful_json(repository.scan(), "R3 scan");
    let docs = repository.docs();
    assert!(
        docs.status.success(),
        "R3 docs failed: {}",
        String::from_utf8_lossy(&docs.stderr)
    );
    assert!(docs.stderr.is_empty());
    let manifest: Value = serde_json::from_slice(&docs.stdout).expect("R3 docs manifest");
    assert_eq!(
        manifest["snapshot_semantic_hash"]["value"],
        snapshot["semantic_hash"]["value"]
    );
    let overview = fs::read_to_string(repository.documents.join("overview.md"))
        .expect("read restarted R3 overview");
    let gaps = snapshot["semantic"]["knowledge_graph"]["coverage"]
        .as_array()
        .expect("R3 coverage gaps");
    let documented_gap_ids = manifest["documents"]
        .as_array()
        .expect("R3 document records")
        .iter()
        .flat_map(|document| {
            document["statements"]
                .as_array()
                .expect("R3 document statements")
        })
        .flat_map(|statement| {
            statement["coverage_gap_ids"]
                .as_array()
                .expect("coverage gap IDs")
        })
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    for gap in gaps {
        let capability = gap["capability"].as_str().expect("coverage capability");
        if capability.contains('.') || capability == "compiler_cross_crate_use_resolution" {
            assert!(
                overview.contains(capability),
                "missing {capability} in docs"
            );
            assert!(documented_gap_ids.contains(gap["id"].as_str().expect("coverage gap ID")));
        }
    }
}

#[test]
fn e2e_fr_qry_001_r3_exact_id_results() {
    let repository = MaterializedRootPackageRepository::implicit();
    let snapshot = successful_json(repository.scan(), "R3 scan");
    let manifest = successful_json(repository.docs(), "R3 docs");
    let graph = &snapshot["semantic"]["knowledge_graph"];
    let crate_id = graph["entities"]
        .as_array()
        .expect("R3 entities")
        .iter()
        .find(|entity| entity["kind"] == "rust.crate")
        .and_then(|entity| entity["id"].as_str())
        .expect("R3 crate ID");
    let entity = successful_json(repository.query(crate_id), "R3 entity query");
    assert_eq!(entity["requested_id"], crate_id);
    assert_eq!(entity["result_kind"], "entity");

    let evidence_id = graph["evidence"][0]["id"].as_str().expect("R3 evidence ID");
    let evidence = successful_json(repository.query(evidence_id), "R3 evidence query");
    assert_eq!(evidence["result_kind"], "evidence");

    let overview_id = manifest["documents"]
        .as_array()
        .expect("R3 document records")
        .iter()
        .find(|document| document["path"] == "overview.md")
        .and_then(|document| document["document_id"].as_str())
        .expect("R3 overview ID");
    let overview = successful_json(repository.query(overview_id), "R3 document query");
    let projected_gap_ids = overview["document_statements"]
        .as_array()
        .expect("R3 queried statements")
        .iter()
        .flat_map(|statement| {
            statement["coverage_gap_ids"]
                .as_array()
                .expect("queried coverage IDs")
        })
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    assert!(
        graph["coverage"]
            .as_array()
            .expect("R3 coverage")
            .iter()
            .filter(|gap| gap["capability"]
                .as_str()
                .is_some_and(|value| value.contains('.')))
            .all(|gap| projected_gap_ids.contains(gap["id"].as_str().expect("gap ID")))
    );
}

#[test]
fn reg_fr_cli_001_r3_selector_absence_is_byte_identical() {
    let repository = MaterializedRootPackageRepository::implicit();
    let output = repository.scan_legacy();
    assert_eq!(output.status.code(), Some(11));
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"{\"code\":\"extraction.unsupported_workspace\",\"context\":{},\"message\":\"unsupported workspace\",\"retryable\":false,\"schema_version\":\"codenoesis.error/v5\",\"stage\":\"extraction\"}\n"
    );
    assert!(!repository.store.exists());
}

#[test]
fn conf_fr_ext_008_error_v10_and_r2_precedence() {
    let invalid = MaterializedRootPackageRepository::standalone();
    let output = invalid.scan_with_workspace_profile("unknown-root-profile");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"{\"code\":\"input.invalid_workspace_profile\",\"context\":{},\"message\":\"invalid workspace profile\",\"retryable\":false,\"schema_version\":\"codenoesis.error/v10\",\"stage\":\"input\"}\n"
    );
    assert!(!invalid.store.exists());

    let conflict = MaterializedRootPackageRepository::member_exclude_conflict();
    let output = conflict.scan();
    assert_eq!(output.status.code(), Some(11));
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).expect("R3 conflict ErrorV10");
    assert_eq!(error["schema_version"], "codenoesis.error/v10");
    assert_eq!(error["code"], "extraction.workspace_member_conflict");
    assert_eq!(error["context"]["path"], "crates/cli");
    assert!(!conflict.store.exists());

    let gitlink = MaterializedRootPackageRepository::implicit();
    let output = gitlink.scan_without_boundary_profile();
    assert_eq!(output.status.code(), Some(10));
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).expect("inherited R2 precedence");
    assert_eq!(error["schema_version"], "codenoesis.error/v4");
    assert_eq!(error["code"], "acquisition.entry_policy_violation");
    assert!(!gitlink.store.exists());
}

#[test]
fn gt_fr_ext_008_public_root_variants() {
    let implicit_repository = MaterializedRootPackageRepository::implicit();
    let implicit = successful_json(implicit_repository.scan(), "implicit R3 scan");
    let explicit_repository = MaterializedRootPackageRepository::explicit_dot();
    let explicit = successful_json(explicit_repository.scan(), "explicit-dot R3 scan");
    assert_eq!(
        crate_ids(&implicit),
        crate_ids(&explicit),
        "explicit root provenance changed crate IDs"
    );
    assert_eq!(
        explicit["semantic"]["knowledge_graph"]["workspace"]["members"][0]["member_source"],
        "explicit_root_member"
    );

    let standalone_repository = MaterializedRootPackageRepository::standalone();
    let standalone = successful_json(standalone_repository.scan(), "standalone R3 scan");
    assert_eq!(
        standalone["semantic"]["knowledge_graph"]["workspace"]["root_shape"],
        "standalone_root_package"
    );
    let virtual_repository = MaterializedRootPackageRepository::virtual_workspace();
    let virtual_workspace = successful_json(virtual_repository.scan(), "virtual R3 scan");
    assert_eq!(
        virtual_workspace["semantic"]["knowledge_graph"]["workspace"]["root_shape"],
        "virtual_workspace"
    );
}

#[allow(clippy::needless_pass_by_value)]
fn successful_json(output: std::process::Output, operation: &str) -> Value {
    assert!(
        output.status.success(),
        "{operation} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "{operation} stderr changed");
    serde_json::from_slice(&output.stdout).expect("single JSON output")
}

fn crate_ids(snapshot: &Value) -> BTreeSet<&str> {
    snapshot["semantic"]["knowledge_graph"]["entities"]
        .as_array()
        .expect("R3 entities")
        .iter()
        .filter(|entity| entity["kind"] == "rust.crate")
        .filter_map(|entity| entity["id"].as_str())
        .collect()
}
