mod support;

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use sha2::{Digest as _, Sha256};

use codenoesis_application::{ScanRequest, ScanService};
use codenoesis_contracts::{PortableGraphV3, R10ContractError, SnapshotEnvelopeV1};
use codenoesis_domain::{RepositoryIdentity, Revision};
use codenoesis_lang_rust::TreeSitterRustWorkspaceExtractor;
use codenoesis_repository::LocalGitRepository;
use support::parse_single_document;
use support::s4_r10::{MaterializedCfgAlternativesRepository, R10_PROFILE, REPOSITORY_ID};
use support::versioned_explorer::assert_matching_viewer_contract;

const LOGICAL_METHOD_ID: &str =
    "urn:codenoesis:entity:blake3:437b0bfcd3821ae91eabe8c395d99c80ec54cc53e6f1e6ca6e24098b20bf4b45";
const UNIX_ALTERNATIVE_ID: &str =
    "urn:codenoesis:entity:blake3:452f8e5e1fe8f0e22b43d49b7393c1b224261b8b2f47452ebaee4ca19794542d";
const WINDOWS_ALTERNATIVE_ID: &str =
    "urn:codenoesis:entity:blake3:df85456dbd86d6ded1dd8a752c159e8d838ebf954feb556f47c6200e9a2843c6";
const UNIX_DECLARATION_EVIDENCE_ID: &str = "urn:codenoesis:evidence:blake3:6390dfef60968e233c891126013da8d58faa2b38dd6573fe0729b88b040bf5a7";
const WINDOWS_DECLARATION_EVIDENCE_ID: &str = "urn:codenoesis:evidence:blake3:c7366514072cddc09890213650e2fa11aaef48645e780c9cbedaa45eae295037";
const UNIX_RELATIONSHIP_ID: &str = "urn:codenoesis:relationship:blake3:b7f0fc978d2ec344f7e28d7dbc24d357c5a53eb63588c478a746e0d507cd5408";
const WINDOWS_RELATIONSHIP_ID: &str = "urn:codenoesis:relationship:blake3:e66eb376e7e7191599ab4db84d82d52c31c9b25ee41c1edb002d0683695afcaf";
const EXPECTED_RED_STDERR_SHA256: &str =
    "dda30410fb0e9ea21d098ac38074c69d6316d777ee16340175ad7db00aa26be1";
const LEGACY_R5_STDERR_SHA256: &str =
    "b5c9f9edd1c4d38220f5f20992a3cf2bc4e693ea6bcacab1b44d3b2dcbe62663";

#[test]
#[allow(clippy::too_many_lines)]
fn e2e_fr_ext_013_cfg_method_alternatives_publish_declarations() {
    let repository = MaterializedCfgAlternativesRepository::fixture();
    assert!(!repository.build_sentinel().exists());
    let output = repository.scan();

    if output.status.code() == Some(2) {
        assert!(output.stdout.is_empty(), "R10 expected-Red stdout changed");
        assert_eq!(output.stderr.len(), 233, "R10 expected-Red length changed");
        assert_eq!(
            hex_sha256(&output.stderr),
            EXPECTED_RED_STDERR_SHA256,
            "R10 expected-Red digest changed"
        );
        let error: Value = parse_single_document(&output.stderr);
        assert_eq!(error["schema_version"], "codenoesis.error/v12");
        assert_eq!(error["code"], "input.invalid_rust_semantic_profile");
        assert!(!repository.store.exists(), "R10 expected Red mutated store");
        assert!(!repository.build_sentinel().exists());
        panic!("expected RepositorySnapshotV12 success; observed frozen missing-profile Red");
    }

    assert!(
        output.status.success(),
        "R10 scan failed: status={:?}, stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "successful R10 stderr changed");
    assert!(
        !repository.build_sentinel().exists(),
        "R10 executed build.rs"
    );

    let snapshot: Value = parse_single_document(&output.stdout);
    assert_eq!(
        snapshot["schema_version"],
        "codenoesis.repository-snapshot/v12"
    );
    assert_eq!(
        snapshot["semantic"]["configuration"]["schema_version"],
        "codenoesis.configuration/v9"
    );
    assert_eq!(
        snapshot["semantic"]["configuration"]["rust_semantic_profile"],
        "rust-cfg-declaration-alternatives-v1"
    );
    let graph = &snapshot["semantic"]["knowledge_graph"];
    assert_eq!(graph["schema_version"], "codenoesis.knowledge-graph/v9");
    assert_eq!(graph["ontology_version"], "codenoesis.ontology/rust/v9");

    let entities = graph["entities"].as_array().expect("R10 graph entities");
    let logical = entities
        .iter()
        .find(|entity| entity["id"] == LOGICAL_METHOD_ID)
        .expect("R10 logical method");
    assert_eq!(logical["kind"], "rust.method");
    assert_eq!(logical["properties"]["declaration_state"], "alternatives");
    assert_eq!(
        logical["properties"]["declaration_alternative_ids"],
        serde_json::json!([UNIX_ALTERNATIVE_ID, WINDOWS_ALTERNATIVE_ID])
    );
    for forbidden in [
        "receiver_present",
        "declared_signature",
        "compilation_presence",
        "attributes",
    ] {
        assert!(
            logical["properties"].get(forbidden).is_none(),
            "logical method selected occurrence property {forbidden}"
        );
    }

    let alternatives = entities
        .iter()
        .filter(|entity| entity["kind"] == "rust.declaration_alternative")
        .collect::<Vec<_>>();
    assert_eq!(alternatives.len(), 2);
    assert_eq!(
        alternatives
            .iter()
            .map(|entity| entity["id"].as_str().expect("alternative ID"))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([UNIX_ALTERNATIVE_ID, WINDOWS_ALTERNATIVE_ID])
    );
    assert!(alternatives.iter().all(|entity| {
        entity["subject_id"] == LOGICAL_METHOD_ID
            && entity["properties"]["declaration_kind"] == "rust.method"
            && entity["properties"]["compilation_presence"] == "conditional_unknown"
            && entity["properties"]["receiver_present"] == true
    }));
    for (alternative_id, signature, declaration_evidence_id) in [
        (
            UNIX_ALTERNATIVE_ID,
            "pub fn try_start_clipboard(&self, context: Option<Context>)",
            UNIX_DECLARATION_EVIDENCE_ID,
        ),
        (
            WINDOWS_ALTERNATIVE_ID,
            "pub fn try_start_clipboard(&self, value: Option<()>)",
            WINDOWS_DECLARATION_EVIDENCE_ID,
        ),
    ] {
        let alternative = alternatives
            .iter()
            .find(|entity| entity["id"] == alternative_id)
            .expect("reviewed R10 alternative");
        assert_eq!(alternative["properties"]["declared_signature"], signature);
        assert_eq!(
            alternative["properties"]["declaration_evidence_id"],
            declaration_evidence_id
        );
    }

    let relationships = graph["relationships"]
        .as_array()
        .expect("R10 relationships");
    assert_eq!(
        relationships
            .iter()
            .filter(|relationship| {
                relationship["kind"] == "HAS_DECLARATION_ALTERNATIVE"
                    && relationship["source"] == LOGICAL_METHOD_ID
            })
            .count(),
        2
    );
    let declaration_evidence = serde_json::json!([
        UNIX_DECLARATION_EVIDENCE_ID,
        WINDOWS_DECLARATION_EVIDENCE_ID
    ]);
    let defines = relationships
        .iter()
        .find(|relationship| {
            relationship["kind"] == "DEFINES" && relationship["target"] == LOGICAL_METHOD_ID
        })
        .expect("R10 logical method DEFINES relationship");
    assert_eq!(defines["evidence_ids"], declaration_evidence);
    let logical_claim = graph["claims"]
        .as_array()
        .expect("R10 claims")
        .iter()
        .find(|claim| claim["subject_id"] == LOGICAL_METHOD_ID)
        .expect("R10 logical method claim");
    assert_eq!(logical_claim["evidence_ids"], declaration_evidence);
    assert!(
        repository.store.exists(),
        "R10 did not publish one visible head"
    );
}

fn hex_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut encoded, byte| {
            write!(&mut encoded, "{byte:02x}").expect("write SHA-256 digest");
            encoded
        })
}

#[test]
fn conf_fr_ext_013_fixture_bytes_are_reviewed() {
    let fixture = support::s4_r10::fixture_root();
    let expected: Value = serde_json::from_slice(
        &fs::read(fixture.join("expected-declaration-alternatives.json"))
            .expect("read R10 expected facts"),
    )
    .expect("parse R10 expected facts");
    assert_eq!(expected["logical_method"]["id"], LOGICAL_METHOD_ID);
    assert_eq!(expected["alternatives"].as_array().map(Vec::len), Some(2));
}

#[test]
fn reg_fr_ext_010_r10_fixture_preserves_legacy_r5_failure_bytes() {
    let repository = MaterializedCfgAlternativesRepository::fixture();
    let output = repository.scan_r5();
    assert_eq!(output.status.code(), Some(11));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr.len(), 366);
    assert_eq!(hex_sha256(&output.stderr), LEGACY_R5_STDERR_SHA256);
    let error: Value = parse_single_document(&output.stderr);
    assert_eq!(error["schema_version"], "codenoesis.error/v12");
    assert_eq!(error["code"], "extraction.rust_semantic_identity_conflict");
    assert!(!repository.store.exists());
    assert!(!repository.build_sentinel().exists());
}

#[test]
fn conf_fr_cli_001_r10_forbidden_compositions_fail_before_acquisition() {
    for extra in [
        &["--rust-framework-profile", "rust-framework-declarations-v1"][..],
        &[
            "--compiler-index-profile",
            "scip-rust-v0.9.0-import-v1",
            "--compiler-index-binding",
            "missing-binding.json",
        ],
        &["--rust-callable-profile", "rust-callable-semantics-v1"],
        &["--output-capacity-profile", "local-snapshot-64m-v1"],
    ] {
        let repository = MaterializedCfgAlternativesRepository::fixture();
        let output = repository.scan_with_extra(extra);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let error: Value = parse_single_document(&output.stderr);
        assert_eq!(error["schema_version"], "codenoesis.error/v17");
        assert_eq!(
            error["code"],
            "input.unsupported_rust_cfg_alternatives_composition"
        );
        assert_eq!(error["context"]["reason"], "source_only_lineage_required");
        assert!(!repository.store.exists());
        assert!(!repository.build_sentinel().exists());
    }
}

#[test]
fn conf_fr_cli_001_r10_invalid_profile_matrix_is_pre_acquisition() {
    let unknown = MaterializedCfgAlternativesRepository::fixture();
    let output = unknown.scan_with_rust_profile("rust-cfg-declaration-alternatives-v2");
    assert_error_v17(&output, 2, "input.invalid_rust_cfg_alternatives_profile");
    assert!(!unknown.store.exists());

    let duplicate = MaterializedCfgAlternativesRepository::fixture();
    let output = duplicate.scan_with_extra(&["--rust-semantic-profile", R10_PROFILE]);
    assert_error_v17(
        &output,
        2,
        "input.unsupported_rust_cfg_alternatives_composition",
    );
    assert!(!duplicate.store.exists());

    for omitted in ["workspace", "manifest"] {
        let repository = MaterializedCfgAlternativesRepository::fixture();
        let mut command = Command::new(env!("CARGO_BIN_EXE_noesis"));
        command
            .current_dir(&repository.root)
            .args(["scan", "--repository"])
            .arg(&repository.worktree)
            .args(["--repository-id", REPOSITORY_ID, "--revision"])
            .arg(&repository.commit_oid)
            .args(["--profile", "standard-local-s4"]);
        if omitted != "workspace" {
            command.args(["--workspace-profile", "cargo-root-package-v1"]);
        }
        if omitted != "manifest" {
            command.args(["--manifest-profile", "cargo-manifest-facts-v1"]);
        }
        let output = command
            .args(["--rust-semantic-profile", R10_PROFILE])
            .arg("--store")
            .arg(&repository.store)
            .args(["--format", "json"])
            .output()
            .expect("launch incomplete R10 profile");
        assert_error_v17(
            &output,
            2,
            "input.unsupported_rust_cfg_alternatives_composition",
        );
        assert!(!repository.store.exists());
        assert!(!repository.build_sentinel().exists());
    }
}

#[test]
fn conf_fr_cli_001_r10_optional_gitlink_boundary_profile_composes() {
    let repository = MaterializedCfgAlternativesRepository::fixture();
    let output =
        repository.scan_with_extra(&["--repository-boundary-profile", "local-gitlinks-v1"]);
    assert_success(&output, "R10 optional gitlink-boundary scan");
    let snapshot: Value = parse_single_document(&output.stdout);
    assert_eq!(
        snapshot["schema_version"],
        "codenoesis.repository-snapshot/v12"
    );
    assert_eq!(
        snapshot["semantic"]["configuration"]["repository_boundary_profile"],
        "local-gitlinks-v1"
    );
    assert!(!repository.build_sentinel().exists());
}

#[test]
fn sec_fr_exp_003_r10_parent_outputs_fail_before_read_or_mutation() {
    let repository = MaterializedCfgAlternativesRepository::fixture();
    let unsafe_output = repository.root.join("selected").join("..").join("escaped");
    let missing_store = repository.root.join("missing-store");
    let export = Command::new(env!("CARGO_BIN_EXE_noesis"))
        .args(["export", "--store"])
        .arg(&missing_store)
        .args(["--repository-id", REPOSITORY_ID, "--output"])
        .arg(&unsafe_output)
        .args(["--portable-profile", R10_PROFILE, "--format", "json"])
        .output()
        .expect("launch unsafe R10 export");
    assert_error_v17(&export, 2, "input.unsafe_output_path");
    assert!(!missing_store.exists());
    assert!(!repository.root.join("escaped").exists());

    let missing_input = repository.root.join("missing-portable.json");
    let explore = Command::new(env!("CARGO_BIN_EXE_noesis"))
        .args(["explore", "--input"])
        .arg(&missing_input)
        .arg("--output")
        .arg(&unsafe_output)
        .args(["--explorer-profile", R10_PROFILE, "--format", "json"])
        .output()
        .expect("launch unsafe R10 explorer");
    assert_error_v17(&explore, 2, "input.unsafe_output_path");
    assert!(!missing_input.exists());
    assert!(!repository.root.join("escaped").exists());
}

#[test]
fn int_fr_ext_013_r10_application_builds_snapshot_v12() {
    let repository = MaterializedCfgAlternativesRepository::fixture();
    let request = ScanRequest::new(
        repository.worktree.clone().into_os_string(),
        RepositoryIdentity::parse(REPOSITORY_ID).expect("R10 repository identity"),
        Revision::parse(&repository.commit_oid).expect("R10 revision"),
        SnapshotEnvelopeV1::new(
            "2026-08-09T12:00:00Z".to_owned(),
            None,
            "r10-application-test".to_owned(),
        ),
    );
    let output = ScanService::new(LocalGitRepository::new())
        .scan_s4_r10(request, &TreeSitterRustWorkspaceExtractor::new())
        .expect("R10 application scan");
    assert_eq!(
        output.snapshot.value()["schema_version"],
        "codenoesis.repository-snapshot/v12"
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn e2e_fr_exp_003_r10_lossless_local_journey() {
    let repository = MaterializedCfgAlternativesRepository::fixture();
    let scan = repository.scan();
    assert_success(&scan, "R10 source scan");
    let snapshot: Value = parse_single_document(&scan.stdout);
    assert_graph_evidence_references(&snapshot["semantic"]["knowledge_graph"]);

    let docs = repository.docs();
    assert_success(&docs, "R10 documentation generation");
    let manifest: Value = parse_single_document(&docs.stdout);
    assert_eq!(
        manifest["schema_version"],
        "codenoesis.documentation-manifest/v1"
    );
    let snapshot_id = manifest["snapshot_id"]
        .as_str()
        .expect("R10 snapshot identity");
    let documentation = read_text_tree(&repository.documents);
    for required in [
        "Conditional declaration alternatives",
        LOGICAL_METHOD_ID,
        UNIX_ALTERNATIVE_ID,
        WINDOWS_ALTERNATIVE_ID,
        "pub fn try_start_clipboard(&amp;self, context: Option&lt;Context&gt;)",
        "pub fn try_start_clipboard(&amp;self, value: Option&lt;()&gt;)",
    ] {
        assert!(
            documentation.contains(required),
            "R10 documentation omitted {required}"
        );
    }

    let logical_query = repository.query(LOGICAL_METHOD_ID);
    assert_success(&logical_query, "R10 logical-method query");
    let logical_result: Value = parse_single_document(&logical_query.stdout);
    assert_eq!(
        logical_result["schema_version"],
        "codenoesis.local-query-result/v7"
    );
    assert_eq!(
        record_ids(&logical_result["linked_r10_entities"]),
        BTreeSet::from([UNIX_ALTERNATIVE_ID, WINDOWS_ALTERNATIVE_ID])
    );
    assert_eq!(
        logical_result["linked_r10_relationships"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(
        subject_ids(&logical_result["claims"]),
        BTreeSet::from([
            LOGICAL_METHOD_ID,
            UNIX_ALTERNATIVE_ID,
            WINDOWS_ALTERNATIVE_ID,
            UNIX_RELATIONSHIP_ID,
            WINDOWS_RELATIONSHIP_ID,
        ])
    );
    assert_eq!(logical_result["evidence"].as_array().map(Vec::len), Some(2));

    let alternative_query = repository.query(UNIX_ALTERNATIVE_ID);
    assert_success(&alternative_query, "R10 declaration-alternative query");
    let alternative_result: Value = parse_single_document(&alternative_query.stdout);
    assert_eq!(
        alternative_result["schema_version"],
        "codenoesis.local-query-result/v7"
    );
    assert_eq!(
        record_ids(&alternative_result["linked_r10_entities"]),
        BTreeSet::from([LOGICAL_METHOD_ID])
    );
    assert_eq!(
        subject_ids(&alternative_result["claims"]),
        BTreeSet::from([LOGICAL_METHOD_ID, UNIX_ALTERNATIVE_ID, UNIX_RELATIONSHIP_ID])
    );
    assert_eq!(
        alternative_result["evidence"].as_array().map(Vec::len),
        Some(2)
    );

    let export = repository.export();
    assert_success(&export, "R10 portable export");
    let portable_bytes =
        fs::read(repository.portable.join("portable-graph.json")).expect("read R10 portable graph");
    assert_eq!(
        export.stdout, portable_bytes,
        "R10 export stdout/file drift"
    );
    let portable =
        PortableGraphV3::from_canonical_file(&portable_bytes, noesis::portable_explorer::sha256)
            .expect("strictly reimport generated R10 portable graph");
    assert_eq!(
        portable.value()["schema_version"],
        "codenoesis.portable-graph/v3"
    );
    assert_eq!(
        portable.value()["source_snapshot"]["schema_version"],
        "codenoesis.repository-snapshot/v12"
    );
    assert_eq!(
        portable.value()["source_snapshot"]["snapshot_id"],
        snapshot_id
    );
    for (portable_family, graph_family) in [
        ("entities", "entities"),
        ("relationships", "relationships"),
        ("claims", "claims"),
        ("evidence", "evidence"),
        ("diagnostics", "diagnostics"),
        ("coverage_gaps", "coverage"),
    ] {
        assert_eq!(
            portable.value()[portable_family],
            snapshot["semantic"]["knowledge_graph"][graph_family],
            "R10 portable family {portable_family} is lossy"
        );
    }

    let explore = repository.explore();
    assert_success(&explore, "R10 offline explorer");
    let explorer_manifest: Value = parse_single_document(&explore.stdout);
    assert_eq!(
        explorer_manifest["schema_version"],
        "codenoesis.local-explorer/v3"
    );
    for disabled in [
        "network",
        "dynamic_code",
        "storage",
        "telemetry",
        "browser_launch",
    ] {
        assert_eq!(
            explorer_manifest["security"][disabled], false,
            "R10 explorer enabled {disabled}"
        );
    }
    assert_matching_viewer_contract(
        &repository.explorer.join("index.html"),
        &explorer_manifest,
        3,
    );
    assert!(!repository.build_sentinel().exists());
}

#[test]
fn ft_fr_qry_001_r10_corrupt_documents_fail_with_error_v17() {
    let repository = MaterializedCfgAlternativesRepository::fixture();
    assert_success(&repository.scan(), "R10 corrupt-query source scan");
    assert_success(&repository.docs(), "R10 corrupt-query documents");
    fs::write(repository.documents.join("manifest.json"), b"{}\n")
        .expect("corrupt R10 documentation manifest");

    let query = repository.query(LOGICAL_METHOD_ID);
    assert_error_v17(&query, 14, "query.invalid_v12");
    assert!(!repository.build_sentinel().exists());
}

#[test]
#[allow(clippy::too_many_lines)]
fn sec_fr_exp_003_r10_portable_reimport_rejects_invalid_or_private_data() {
    let repository = MaterializedCfgAlternativesRepository::fixture();
    assert_success(&repository.scan(), "R10 strict-reimport source scan");
    assert_success(&repository.docs(), "R10 strict-reimport documents");
    assert_success(&repository.export(), "R10 strict-reimport export");
    let bytes = fs::read(repository.portable.join("portable-graph.json"))
        .expect("read R10 strict-reimport graph");
    let portable = PortableGraphV3::from_canonical_file(&bytes, noesis::portable_explorer::sha256)
        .expect("reimport canonical R10 graph");
    let value = portable.value();
    let serialized = String::from_utf8(bytes.clone()).expect("R10 portable UTF-8");
    for forbidden in [
        "source_contents",
        "source_snippet",
        "body_text",
        "R10_BUILD_SENTINEL_EXECUTED",
        "macro_rules!",
    ] {
        assert!(
            !serialized.contains(forbidden),
            "R10 portable leaked {forbidden}"
        );
    }

    let mut unsupported = value.clone();
    unsupported["schema_version"] = Value::String("codenoesis.portable-graph/v4".to_owned());
    assert!(matches!(
        PortableGraphV3::from_canonical_file(
            &canonical_json_file(&unsupported),
            noesis::portable_explorer::sha256,
        ),
        Err(R10ContractError::UnsupportedPortableGraphSchema(_))
    ));

    let mut unknown = value.clone();
    unknown["unknown"] = Value::Bool(true);
    assert!(matches!(
        PortableGraphV3::from_canonical_file(
            &canonical_json_file(&unknown),
            noesis::portable_explorer::sha256,
        ),
        Err(R10ContractError::InvalidProjection)
    ));

    let mut invalid_snapshot_id = value.clone();
    invalid_snapshot_id["source_snapshot"]["snapshot_id"] =
        Value::String(format!("urn:codenoesis:snapshot:blake3:{}", "0".repeat(64)));
    assert!(matches!(
        PortableGraphV3::from_canonical_file(
            &canonical_json_file(&invalid_snapshot_id),
            noesis::portable_explorer::sha256,
        ),
        Err(R10ContractError::InvalidProjection)
    ));

    let mut invalid_entity_id = value.clone();
    invalid_entity_id["entities"][0]["id"] = Value::String("invalid".to_owned());
    refresh_family_sha256(&mut invalid_entity_id, "entities");
    assert!(matches!(
        PortableGraphV3::from_canonical_file(
            &canonical_json_file(&invalid_entity_id),
            noesis::portable_explorer::sha256,
        ),
        Err(R10ContractError::InvalidProjection)
    ));

    let mut unknown_entity_field = value.clone();
    unknown_entity_field["entities"][0]["unknown"] = Value::Bool(true);
    refresh_family_sha256(&mut unknown_entity_field, "entities");
    assert!(matches!(
        PortableGraphV3::from_canonical_file(
            &canonical_json_file(&unknown_entity_field),
            noesis::portable_explorer::sha256,
        ),
        Err(R10ContractError::InvalidProjection)
    ));

    let mut unknown_property_field = value.clone();
    unknown_property_field["entities"][0]["properties"]["unknown"] = Value::Bool(true);
    refresh_family_sha256(&mut unknown_property_field, "entities");
    assert!(matches!(
        PortableGraphV3::from_canonical_file(
            &canonical_json_file(&unknown_property_field),
            noesis::portable_explorer::sha256,
        ),
        Err(R10ContractError::InvalidProjection)
    ));

    let mut duplicate = value.clone();
    let entity = duplicate["entities"][0].clone();
    duplicate["entities"]
        .as_array_mut()
        .expect("R10 entities")
        .insert(0, entity);
    assert!(matches!(
        PortableGraphV3::from_canonical_file(
            &canonical_json_file(&duplicate),
            noesis::portable_explorer::sha256,
        ),
        Err(R10ContractError::IdentityConflict {
            family: "entities",
            ..
        })
    ));

    let mut unordered = value.clone();
    unordered["entities"]
        .as_array_mut()
        .expect("R10 entities")
        .swap(0, 1);
    assert!(matches!(
        PortableGraphV3::from_canonical_file(
            &canonical_json_file(&unordered),
            noesis::portable_explorer::sha256,
        ),
        Err(R10ContractError::IdentityConflict {
            family: "entities",
            ..
        })
    ));

    let mut dangling = value.clone();
    r10_relationship_mut(&mut dangling)["target"] =
        Value::String("urn:codenoesis:entity:blake3:missing".to_owned());
    assert!(matches!(
        PortableGraphV3::from_canonical_file(
            &canonical_json_file(&dangling),
            noesis::portable_explorer::sha256,
        ),
        Err(R10ContractError::ReferenceMismatch {
            family: "relationships",
            ..
        })
    ));

    let mut dangling_owner = value.clone();
    let entity_with_owner = dangling_owner["entities"]
        .as_array_mut()
        .expect("R10 entities")
        .iter_mut()
        .find(|entity| entity.get("owner_id").is_some())
        .expect("R10 entity with owner");
    entity_with_owner["owner_id"] =
        Value::String("urn:codenoesis:entity:blake3:missing".to_owned());
    refresh_family_sha256(&mut dangling_owner, "entities");
    assert!(matches!(
        PortableGraphV3::from_canonical_file(
            &canonical_json_file(&dangling_owner),
            noesis::portable_explorer::sha256,
        ),
        Err(R10ContractError::ReferenceMismatch {
            family: "entities",
            ..
        })
    ));

    let mut unresolved_evidence = value.clone();
    r10_relationship_mut(&mut unresolved_evidence)["evidence_ids"][0] =
        Value::String("urn:codenoesis:evidence:blake3:missing".to_owned());
    assert!(matches!(
        PortableGraphV3::from_canonical_file(
            &canonical_json_file(&unresolved_evidence),
            noesis::portable_explorer::sha256,
        ),
        Err(R10ContractError::ReferenceMismatch {
            family: "relationships",
            ..
        })
    ));

    let mut dangling_document = value.clone();
    dangling_document["documents"][0]["subject_id"] =
        Value::String("urn:codenoesis:entity:blake3:missing".to_owned());
    assert!(matches!(
        PortableGraphV3::from_canonical_file(
            &canonical_json_file(&dangling_document),
            noesis::portable_explorer::sha256,
        ),
        Err(R10ContractError::ReferenceMismatch {
            family: "documents",
            ..
        })
    ));

    let mut dangling_statement_document = value.clone();
    dangling_statement_document["document_statements"][0]["document_id"] =
        Value::String("urn:codenoesis:document:blake3:missing".to_owned());
    assert!(matches!(
        PortableGraphV3::from_canonical_file(
            &canonical_json_file(&dangling_statement_document),
            noesis::portable_explorer::sha256,
        ),
        Err(R10ContractError::ReferenceMismatch {
            family: "document_statements",
            ..
        })
    ));

    let mut dangling_statement_subject = value.clone();
    dangling_statement_subject["document_statements"][0]["subject_ids"][0] =
        Value::String("urn:codenoesis:entity:blake3:missing".to_owned());
    assert!(matches!(
        PortableGraphV3::from_canonical_file(
            &canonical_json_file(&dangling_statement_subject),
            noesis::portable_explorer::sha256,
        ),
        Err(R10ContractError::ReferenceMismatch {
            family: "document_statements",
            ..
        })
    ));

    let mut dangling_statement_evidence = value.clone();
    let evidence_statement = dangling_statement_evidence["document_statements"]
        .as_array_mut()
        .expect("R10 document statements")
        .iter_mut()
        .find(|statement| {
            statement["evidence_ids"]
                .as_array()
                .is_some_and(|identifiers| !identifiers.is_empty())
        })
        .expect("R10 evidence-backed statement");
    evidence_statement["evidence_ids"][0] =
        Value::String("urn:codenoesis:evidence:blake3:missing".to_owned());
    assert!(matches!(
        PortableGraphV3::from_canonical_file(
            &canonical_json_file(&dangling_statement_evidence),
            noesis::portable_explorer::sha256,
        ),
        Err(R10ContractError::ReferenceMismatch {
            family: "document_statements",
            ..
        })
    ));

    let mut private = value.clone();
    private["entities"][0]["body_text"] = Value::String("secret".to_owned());
    assert!(matches!(
        PortableGraphV3::from_canonical_file(
            &canonical_json_file(&private),
            noesis::portable_explorer::sha256,
        ),
        Err(R10ContractError::UnsafePayload {
            reason: "private_field"
        })
    ));

    let mut unsafe_path = value.clone();
    unsafe_path["evidence"][0]["path"] = Value::String("../secret.rs".to_owned());
    assert!(matches!(
        PortableGraphV3::from_canonical_file(
            &canonical_json_file(&unsafe_path),
            noesis::portable_explorer::sha256,
        ),
        Err(R10ContractError::UnsafePayload {
            reason: "unsafe_evidence_path"
        })
    ));

    let mut hash_mismatch = value.clone();
    hash_mismatch["projection"]["family_sha256"]["entities"] = Value::String("0".repeat(64));
    assert!(matches!(
        PortableGraphV3::from_canonical_file(
            &canonical_json_file(&hash_mismatch),
            noesis::portable_explorer::sha256,
        ),
        Err(R10ContractError::InvalidProjection)
    ));

    let mut noncanonical = serde_json::to_vec_pretty(value).expect("pretty R10 portable graph");
    noncanonical.push(b'\n');
    assert!(matches!(
        PortableGraphV3::from_canonical_file(&noncanonical, noesis::portable_explorer::sha256,),
        Err(R10ContractError::Noncanonical { .. })
    ));
    assert!(!repository.build_sentinel().exists());
}

fn assert_success(output: &Output, journey: &str) {
    assert!(
        output.status.success(),
        "{journey} failed: status={:?}, stdout={} stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "{journey} stderr changed");
}

fn assert_error_v17(output: &Output, exit_code: i32, code: &str) {
    assert_eq!(output.status.code(), Some(exit_code));
    assert!(output.stdout.is_empty());
    let error: Value = parse_single_document(&output.stderr);
    assert_eq!(error["schema_version"], "codenoesis.error/v17");
    assert_eq!(error["code"], code);
}

fn record_ids(value: &Value) -> BTreeSet<&str> {
    value
        .as_array()
        .expect("R10 linked records")
        .iter()
        .map(|record| record["id"].as_str().expect("R10 linked record ID"))
        .collect()
}

fn subject_ids(value: &Value) -> BTreeSet<&str> {
    value
        .as_array()
        .expect("R10 claims")
        .iter()
        .map(|record| record["subject_id"].as_str().expect("R10 claim subject"))
        .collect()
}

fn canonical_json_file(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(value).expect("serialize R10 mutation");
    bytes.push(b'\n');
    bytes
}

fn refresh_family_sha256(value: &mut Value, family: &str) {
    let bytes = serde_json::to_vec(&value[family]).expect("serialize R10 portable family");
    value["projection"]["family_sha256"][family] = Value::String(hex_sha256(&bytes));
}

fn r10_relationship_mut(value: &mut Value) -> &mut Value {
    value["relationships"]
        .as_array_mut()
        .expect("R10 relationships")
        .iter_mut()
        .find(|relationship| relationship["kind"] == "HAS_DECLARATION_ALTERNATIVE")
        .expect("R10 alternative relationship")
}

fn assert_graph_evidence_references(graph: &Value) {
    let entity_ids = record_ids(&graph["entities"]);
    let evidence_ids = record_ids(&graph["evidence"]);
    for family in ["entities", "relationships", "claims"] {
        for record in graph[family].as_array().expect("R10 graph family") {
            if family == "relationships" {
                for endpoint in ["source", "target"] {
                    let endpoint_id = record[endpoint]
                        .as_str()
                        .expect("R10 relationship endpoint");
                    assert!(
                        entity_ids.contains(endpoint_id),
                        "R10 relationship has non-entity {endpoint} {endpoint_id}: {record}"
                    );
                }
            }
            let mut previous = None;
            for evidence_id in record["evidence_ids"].as_array().into_iter().flatten() {
                let evidence_id = evidence_id.as_str().expect("R10 evidence reference");
                assert!(
                    evidence_ids.contains(evidence_id),
                    "R10 {family} record has unresolved evidence {evidence_id}: {record}"
                );
                assert!(
                    previous.is_none_or(|value| value < evidence_id),
                    "R10 {family} record has unordered evidence {evidence_id}: {record}"
                );
                previous = Some(evidence_id);
            }
        }
    }
}

fn read_text_tree(root: &Path) -> String {
    let mut paths = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(path) = paths.pop() {
        if path.is_dir() {
            paths.extend(
                fs::read_dir(&path)
                    .expect("read R10 documentation directory")
                    .map(|entry| entry.expect("read R10 documentation entry").path()),
            );
        } else {
            files.push(path);
        }
    }
    let mut paths = files;
    paths.sort();
    paths
        .into_iter()
        .map(|path| fs::read_to_string(path).expect("read R10 documentation file"))
        .collect::<Vec<_>>()
        .join("\n")
}
