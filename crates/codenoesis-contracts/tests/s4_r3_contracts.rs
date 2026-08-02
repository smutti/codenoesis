use std::collections::{BTreeMap, BTreeSet};

use codenoesis_contracts::{
    CodeNoesisErrorV10, RepositorySnapshotV6, SnapshotEnvelopeV1, generate_documentation_v1,
    local_query_result_v1, validate_stored_snapshot_semantic_v6,
};
use codenoesis_domain::knowledge::{ClaimState, ClaimSubjectKind};
use codenoesis_domain::s4::{
    RustWorkspaceKnowledge, WorkspaceClaim, WorkspaceCoverageGap, WorkspaceEntity,
    WorkspaceEvidence, WorkspaceExtractionChunk, WorkspaceKnowledgeGraph,
};
use codenoesis_domain::s4_r3::{
    R3_EXTRACTION_CONTRACT_VERSION, R3_ONTOLOGY_VERSION, R3_PIPELINE_VERSION,
    R3_WORKSPACE_EXTRACTOR_VERSION, R3_WORKSPACE_PROFILE, RootPackageMember, RootPackageShape,
    RootPackageTarget, RootPackageWorkspaceError, RootPackageWorkspaceKnowledge,
    RootPackageWorkspacePlan, WorkspaceManifestReason, WorkspaceMemberSource, WorkspaceTargetKind,
};
use codenoesis_domain::storage::{ArtifactRole, LocalSnapshotHead, SNAPSHOT_SCHEMA_VERSION_V6};
use codenoesis_domain::{
    AcquiredFile, AcquiredRepository, BoundRevision, ObjectId, RegularFileMode, RepositoryIdentity,
    RepositoryInventory,
};
use serde_json::Value;

const REPOSITORY_ID: &str = "urn:codenoesis:test:r3-contract";
const COMMIT_OID: &str = "1111111111111111111111111111111111111111";
const TREE_OID: &str = "2222222222222222222222222222222222222222";
const MANIFEST_OID: &str = "3333333333333333333333333333333333333333";
const SOURCE_OID: &str = "4444444444444444444444444444444444444444";

#[test]
fn conf_fr_ext_008_snapshot_v6_graph_v3_error_v10() {
    let (snapshot, crate_id, _) = reviewed_snapshot();
    let value = snapshot.value();
    assert_eq!(value["schema_version"], SNAPSHOT_SCHEMA_VERSION_V6);
    assert_eq!(
        value["semantic"]["configuration"]["workspace_profile"],
        R3_WORKSPACE_PROFILE
    );
    assert_eq!(
        value["semantic"]["configuration"]["repository_boundary_profile"],
        Value::Null
    );
    assert_eq!(value["semantic"]["pipeline_version"], R3_PIPELINE_VERSION);
    assert_eq!(value["semantic"]["ontology_version"], R3_ONTOLOGY_VERSION);
    assert_eq!(
        value["semantic"]["extractor_contract_version"],
        R3_EXTRACTION_CONTRACT_VERSION
    );
    assert_eq!(
        value["semantic"]["knowledge_graph"]["schema_version"],
        "codenoesis.knowledge-graph/v3"
    );
    assert_eq!(
        value["semantic"]["knowledge_graph"]["workspace"]["root_shape"],
        "standalone_root_package"
    );
    assert_eq!(
        value["semantic"]["extraction_chunks"][0]["schema_version"],
        "codenoesis.extraction-chunk/v3"
    );
    assert_eq!(
        value["semantic"]["extraction_chunks"][0]["crate_id"],
        crate_id
    );
    assert_eq!(
        object_keys(value),
        set(["envelope", "schema_version", "semantic", "semantic_hash"])
    );
    assert_eq!(
        object_keys(&value["semantic"]["configuration"]),
        set([
            "profile",
            "repository_boundary_profile",
            "schema_version",
            "semantic_hash",
            "workspace_profile",
        ])
    );
    assert_eq!(
        value["semantic"]["extractor_versions"],
        serde_json::json!([
            "codenoesis.inventory-classifier/s1-v1",
            "codenoesis.rust-tree-sitter/s4-v1",
            R3_WORKSPACE_EXTRACTOR_VERSION
        ])
    );
    let stdout = snapshot.canonical_stdout().expect("serialize strict V6");
    assert_eq!(stdout.last(), Some(&b'\n'));

    let candidate = snapshot
        .publication_candidate()
        .expect("build V6 publication candidate");
    candidate
        .validate()
        .expect("validate V6 existing-role candidate");
    assert_eq!(
        candidate
            .artifacts
            .iter()
            .map(|artifact| artifact.role)
            .collect::<Vec<_>>(),
        [
            ArtifactRole::SnapshotSemantic,
            ArtifactRole::KnowledgeGraph,
            ArtifactRole::ExtractionChunk,
        ]
    );
    let head = LocalSnapshotHead {
        repository_identity: candidate.snapshot.repository_identity.clone(),
        snapshot_id: candidate.snapshot.snapshot_id.clone(),
        commit_oid: candidate.snapshot.commit_oid.clone(),
        snapshot_schema_version: candidate.snapshot.snapshot_schema_version.clone(),
        semantic_hash: candidate.snapshot.semantic_hash.clone(),
        graph_semantic_hash: candidate.snapshot.graph_semantic_hash.clone(),
        generation: 1,
        artifacts: candidate.artifact_references(),
    };
    validate_stored_snapshot_semantic_v6(&value["semantic"], &head)
        .expect("validate complete V6 stored head");
    let mut corrupted = value["semantic"].clone();
    corrupted["knowledge_graph"]["semantic_hash"]["value"] = Value::String("0".repeat(64));
    assert!(validate_stored_snapshot_semantic_v6(&corrupted, &head).is_err());

    let invalid = CodeNoesisErrorV10::invalid_workspace_profile()
        .canonical_stderr()
        .expect("serialize ErrorV10 input");
    assert_eq!(
        invalid,
        b"{\"code\":\"input.invalid_workspace_profile\",\"context\":{},\"message\":\"invalid workspace profile\",\"retryable\":false,\"schema_version\":\"codenoesis.error/v10\",\"stage\":\"input\"}\n"
    );
    let manifest_error =
        CodeNoesisErrorV10::from_workspace(&RootPackageWorkspaceError::InvalidManifest {
            reason: WorkspaceManifestReason::MissingMemberManifest,
            path: Some("member/Cargo.toml".to_owned()),
        })
        .expect("map reviewed R3 failure")
        .canonical_stderr()
        .expect("serialize ErrorV10 extraction");
    assert_eq!(
        serde_json::from_slice::<Value>(&manifest_error).expect("ErrorV10 JSON"),
        serde_json::json!({
            "schema_version": "codenoesis.error/v10",
            "code": "extraction.invalid_workspace_manifest",
            "stage": "extraction",
            "message": "invalid workspace manifest",
            "retryable": false,
            "context": {
                "reason": "missing_member_manifest",
                "path": "member/Cargo.toml"
            }
        })
    );
}

#[test]
fn e2e_fr_doc_001_r3_coverage_is_documented() {
    let (snapshot, _, gap_id) = reviewed_snapshot();
    let candidate = snapshot
        .publication_candidate()
        .expect("derive reviewed snapshot ID");
    let generated = generate_documentation_v1(
        &snapshot.value()["semantic"],
        candidate.snapshot.snapshot_id.as_str(),
        &candidate.snapshot.semantic_hash.value,
    )
    .expect("generate R3 documentation through v1 contract");
    let overview = generated
        .documents()
        .iter()
        .find(|document| document.path == "overview.md")
        .expect("R3 overview");
    assert!(
        std::str::from_utf8(&overview.bytes)
            .expect("UTF-8 Markdown")
            .contains("cargo.dependencies_deferred")
    );
    assert!(
        generated.manifest()["documents"]
            .as_array()
            .expect("documentation records")
            .iter()
            .flat_map(|document| {
                document["statements"]
                    .as_array()
                    .expect("documentation statements")
            })
            .any(|statement| statement["coverage_gap_ids"]
                .as_array()
                .is_some_and(|ids| ids.iter().any(|id| id.as_str() == Some(&gap_id))))
    );
}

#[test]
fn e2e_fr_qry_001_r3_exact_id_results() {
    let (snapshot, crate_id, gap_id) = reviewed_snapshot();
    let candidate = snapshot
        .publication_candidate()
        .expect("derive reviewed snapshot ID");
    let snapshot_id = candidate.snapshot.snapshot_id.as_str();
    let generated = generate_documentation_v1(
        &snapshot.value()["semantic"],
        snapshot_id,
        &candidate.snapshot.semantic_hash.value,
    )
    .expect("generate query documents");
    let entity = local_query_result_v1(
        &snapshot.value()["semantic"],
        generated.manifest(),
        snapshot_id,
        &crate_id,
    )
    .expect("query R3 crate ID")
    .canonical_stdout()
    .expect("serialize R3 entity query");
    assert_eq!(
        serde_json::from_slice::<Value>(&entity).expect("entity query JSON")["result_kind"],
        "entity"
    );
    let overview_id = generated.manifest()["documents"]
        .as_array()
        .expect("documentation records")
        .iter()
        .find(|document| document["path"] == "overview.md")
        .and_then(|document| document["document_id"].as_str())
        .expect("overview document ID");
    let document = local_query_result_v1(
        &snapshot.value()["semantic"],
        generated.manifest(),
        snapshot_id,
        overview_id,
    )
    .expect("query exact R3 document ID")
    .canonical_stdout()
    .expect("serialize R3 document query");
    let document: Value = serde_json::from_slice(&document).expect("document query JSON");
    assert!(
        document["document_statements"]
            .as_array()
            .expect("document query statements")
            .iter()
            .any(|statement| statement["coverage_gap_ids"]
                .as_array()
                .is_some_and(|ids| ids.iter().any(|id| id.as_str() == Some(&gap_id))))
    );
}

fn reviewed_snapshot() -> (RepositorySnapshotV6, String, String) {
    let inventory = reviewed_inventory();
    let mut crate_entity =
        WorkspaceEntity::rust_crate(REPOSITORY_ID, "Cargo.toml", "root", "lib", "root");
    crate_entity.properties.extend(BTreeMap::from([
        (
            "workspace_member_source".to_owned(),
            "implicit_root_package".to_owned(),
        ),
        (
            "workspace_root_shape".to_owned(),
            "standalone_root_package".to_owned(),
        ),
    ]));
    let crate_id = crate_entity.id.clone();
    let source_entity =
        WorkspaceEntity::source_file(REPOSITORY_ID, &crate_id, "src/lib.rs", SOURCE_OID);
    let manifest_evidence =
        WorkspaceEvidence::complete_file(REPOSITORY_ID, COMMIT_OID, "Cargo.toml", MANIFEST_OID, 83);
    let source_evidence =
        WorkspaceEvidence::complete_file(REPOSITORY_ID, COMMIT_OID, "src/lib.rs", SOURCE_OID, 18);
    let coverage = WorkspaceCoverageGap::unsupported(
        REPOSITORY_ID,
        COMMIT_OID,
        "cargo.dependencies_deferred",
        &manifest_evidence.id,
    );
    let gap_id = coverage.id.clone();
    let mut entities = vec![crate_entity, source_entity.clone()];
    entities.sort_by(|left, right| left.id.cmp(&right.id));
    let mut claims = entities
        .iter()
        .map(|entity| {
            WorkspaceClaim::new(
                ClaimSubjectKind::Entity,
                entity.id.clone(),
                ClaimState::DeterministicFact,
                if entity.id == crate_id {
                    vec![manifest_evidence.id.clone()]
                } else {
                    vec![source_evidence.id.clone()]
                },
            )
        })
        .collect::<Vec<_>>();
    claims.sort_by(|left, right| left.id.cmp(&right.id));
    let mut evidence = vec![manifest_evidence, source_evidence];
    evidence.sort_by(|left, right| left.id.cmp(&right.id));
    let chunk = WorkspaceExtractionChunk {
        repository_identity: REPOSITORY_ID.to_owned(),
        crate_id: crate_id.clone(),
        source_file_id: source_entity.id,
        entities: entities.clone(),
        relationships: Vec::new(),
        claims: claims.clone(),
        evidence: evidence.clone(),
        diagnostics: Vec::new(),
        coverage: vec![coverage.clone()],
    };
    let knowledge = RustWorkspaceKnowledge {
        extraction_chunks: vec![chunk],
        graph: WorkspaceKnowledgeGraph {
            repository_identity: REPOSITORY_ID.to_owned(),
            commit_oid: COMMIT_OID.to_owned(),
            entities,
            relationships: Vec::new(),
            claims,
            evidence,
            diagnostics: Vec::new(),
            coverage: vec![coverage],
        },
    };
    let target = RootPackageTarget::new(
        REPOSITORY_ID,
        ".".to_owned(),
        WorkspaceMemberSource::ImplicitRootPackage,
        "Cargo.toml".to_owned(),
        "root".to_owned(),
        WorkspaceTargetKind::Library,
        "root".to_owned(),
        "src/lib.rs".to_owned(),
    );
    assert_eq!(target.crate_id, crate_id);
    let workspace = RootPackageWorkspaceKnowledge {
        plan: RootPackageWorkspacePlan {
            root_shape: RootPackageShape::StandaloneRootPackage,
            members: vec![RootPackageMember {
                path: ".".to_owned(),
                manifest_path: Some("Cargo.toml".to_owned()),
                member_source: WorkspaceMemberSource::ImplicitRootPackage,
                crate_ids: vec![crate_id.clone()],
                external_boundary_id: None,
            }],
            excluded_paths: Vec::new(),
            targets: vec![target],
        },
        knowledge,
    };
    let snapshot = RepositorySnapshotV6::from_inventory_and_workspace(
        &inventory,
        &workspace,
        None,
        SnapshotEnvelopeV1::new(
            "2026-08-02T00:00:00Z".to_owned(),
            None,
            "r3-contract".to_owned(),
        ),
    )
    .expect("build reviewed V6 snapshot");
    (snapshot, crate_id, gap_id)
}

fn reviewed_inventory() -> RepositoryInventory {
    RepositoryInventory::classify(AcquiredRepository::new(
        BoundRevision::new(
            RepositoryIdentity::parse(REPOSITORY_ID).expect("repository identity"),
            oid(COMMIT_OID),
            oid(TREE_OID),
        ),
        1,
        vec![
            AcquiredFile::new(
                "Cargo.toml".to_owned(),
                RegularFileMode::Regular,
                oid(MANIFEST_OID),
                b"[package]\nname=\"root\"\nedition=\"2024\"\n[lib]\npath=\"src/lib.rs\"\n[dependencies]\n".to_vec(),
            ),
            AcquiredFile::new(
                "src/lib.rs".to_owned(),
                RegularFileMode::Regular,
                oid(SOURCE_OID),
                b"pub struct Root;\n".to_vec(),
            ),
        ],
    ))
}

fn object_keys(value: &Value) -> BTreeSet<&str> {
    value
        .as_object()
        .expect("JSON object")
        .keys()
        .map(String::as_str)
        .collect()
}

fn set<const LENGTH: usize>(values: [&'static str; LENGTH]) -> BTreeSet<&'static str> {
    values.into_iter().collect()
}

fn oid(value: &str) -> ObjectId {
    ObjectId::parse_sha1(value).expect("SHA-1 object ID")
}
