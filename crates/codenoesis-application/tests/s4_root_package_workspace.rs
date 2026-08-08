use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use codenoesis_application::{RepositoryBoundaryScanInput, ScanRequest, ScanService};
use codenoesis_contracts::SnapshotEnvelopeV1;
use codenoesis_domain::knowledge::{ClaimState, ClaimSubjectKind};
use codenoesis_domain::s1_boundaries::{
    AcquiredGitlink, AcquiredRepositoryBoundaries, BoundarySha256, NestedAcquisitionProfile,
    NestedRepositoryAcquisitionError, RepositoryBoundaryAcquisitionError,
};
use codenoesis_domain::s4::{
    RustWorkspaceKnowledge, WorkspaceClaim, WorkspaceEntity, WorkspaceEvidence,
    WorkspaceExtractionChunk, WorkspaceKnowledgeGraph,
};
use codenoesis_domain::s4_r3::{
    ExternalWorkspaceBoundary, RootPackageMember, RootPackageShape, RootPackageTarget,
    RootPackageWorkspaceError, RootPackageWorkspaceExtraction, RootPackageWorkspaceKnowledge,
    RootPackageWorkspacePlan, WorkspaceMemberSource, WorkspaceTargetKind,
};
use codenoesis_domain::{
    AcquiredFile, AcquiredRepository, BoundRevision, ObjectId, RegularFileMode, RepositoryError,
    RepositoryIdentity, RepositoryInventory, Revision,
};
use codenoesis_ports::{
    RepositoryAcquirer, RepositoryBoundaryAcquirer, RootPackageWorkspaceExtractor,
};

const REPOSITORY_ID: &str = "urn:codenoesis:test:r3-application";

#[test]
fn sec_fr_ext_008_gitlink_member_stays_external_in_application() {
    let counters = Arc::new(Counters::default());
    let service = ScanService::new(MockBoundaryAcquirer {
        counters: counters.clone(),
    });
    let scan = service
        .scan_s4_r3_boundaries(
            request(),
            RepositoryBoundaryScanInput {
                manifest: None,
                nested_roots: Vec::new(),
            },
            &RecordingExtractor {
                calls: counters.extractor.clone(),
            },
            &TestHasher,
        )
        .expect("compose R2 boundaries with R3 extraction");
    assert_eq!(counters.root.load(Ordering::Relaxed), 1);
    assert_eq!(counters.nested.load(Ordering::Relaxed), 0);
    assert_eq!(counters.extractor.load(Ordering::Relaxed), 1);
    assert_eq!(
        scan.snapshot.value()["semantic"]["configuration"]["repository_boundary_profile"],
        "local-gitlinks-v1"
    );
    assert_eq!(
        scan.snapshot.value()["semantic"]["repository_boundaries"]["summary"]["boundary_count"],
        1
    );
    let external_member =
        scan.snapshot.value()["semantic"]["knowledge_graph"]["workspace"]["members"]
            .as_array()
            .expect("workspace members")
            .iter()
            .find(|member| member["path"] == "external/model")
            .expect("external member projection");
    assert!(external_member["manifest_path"].is_null());
    assert!(
        external_member["crate_ids"]
            .as_array()
            .expect("external crate IDs")
            .is_empty()
    );
    assert!(
        external_member["external_boundary_id"]
            .as_str()
            .is_some_and(|value| value.starts_with("urn:codenoesis:repository-boundary:sha256:"))
    );
}

#[derive(Default)]
struct Counters {
    root: AtomicUsize,
    nested: AtomicUsize,
    extractor: Arc<AtomicUsize>,
}

struct MockBoundaryAcquirer {
    counters: Arc<Counters>,
}

impl RepositoryBoundaryAcquirer for MockBoundaryAcquirer {
    fn acquire_inventory_with_boundaries(
        &self,
        _repository: &OsStr,
        _identity: RepositoryIdentity,
        _revision: Revision,
    ) -> Result<AcquiredRepositoryBoundaries, RepositoryBoundaryAcquisitionError> {
        self.counters.root.fetch_add(1, Ordering::Relaxed);
        let root = bound_revision();
        Ok(AcquiredRepositoryBoundaries {
            repository: AcquiredRepository::new(
                root.clone(),
                1,
                vec![
                    AcquiredFile::new(
                        "Cargo.toml".to_owned(),
                        RegularFileMode::Regular,
                        oid('c'),
                        b"[package]\nname=\"root\"\nedition=\"2024\"\n[workspace]\nmembers=[\"external/model\"]\n[lib]\npath=\"src/lib.rs\"\n".to_vec(),
                    ),
                    AcquiredFile::new(
                        "src/lib.rs".to_owned(),
                        RegularFileMode::Regular,
                        oid('d'),
                        b"pub struct Root;\n".to_vec(),
                    ),
                ],
            ),
            gitlinks: vec![AcquiredGitlink {
                path: "external/model".to_owned(),
                containing_tree_oid: root.tree_oid().clone(),
                gitlink_oid: oid('e'),
            }],
            gitmodules: None,
        })
    }

    fn bind_nested_repository(
        &self,
        _repository: &OsStr,
        _identity: RepositoryIdentity,
        _revision: Revision,
        _profile: NestedAcquisitionProfile,
    ) -> Result<BoundRevision, NestedRepositoryAcquisitionError> {
        self.counters.nested.fetch_add(1, Ordering::Relaxed);
        Err(NestedRepositoryAcquisitionError::Repository(
            RepositoryError::Unexpected,
        ))
    }
}

impl RepositoryAcquirer for MockBoundaryAcquirer {
    fn bind(
        &self,
        _repository: &OsStr,
        _identity: RepositoryIdentity,
        _revision: Revision,
    ) -> Result<BoundRevision, RepositoryError> {
        Err(RepositoryError::Unexpected)
    }
}

struct RecordingExtractor {
    calls: Arc<AtomicUsize>,
}

impl RootPackageWorkspaceExtractor for RecordingExtractor {
    fn extract_root_package_workspace_incremental(
        &self,
        inventory: &RepositoryInventory,
        external_boundaries: &[ExternalWorkspaceBoundary],
        _cache_entries: &[codenoesis_domain::s5::AnalysisCacheEntry],
    ) -> Result<RootPackageWorkspaceExtraction, RootPackageWorkspaceError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        assert_eq!(external_boundaries.len(), 1);
        assert_eq!(external_boundaries[0].path, "external/model");
        Ok(minimal_extraction(inventory, &external_boundaries[0]))
    }
}

#[allow(clippy::too_many_lines)]
fn minimal_extraction(
    inventory: &RepositoryInventory,
    external: &ExternalWorkspaceBoundary,
) -> RootPackageWorkspaceExtraction {
    let repository_identity = inventory.bound_revision().repository_identity().as_str();
    let commit_oid = inventory.bound_revision().commit_oid().as_str();
    let target = RootPackageTarget::new(
        repository_identity,
        ".".to_owned(),
        WorkspaceMemberSource::ImplicitRootPackage,
        "Cargo.toml".to_owned(),
        "root".to_owned(),
        WorkspaceTargetKind::Library,
        "root".to_owned(),
        "src/lib.rs".to_owned(),
    );
    let mut crate_entity =
        WorkspaceEntity::rust_crate(repository_identity, "Cargo.toml", "root", "lib", "root");
    crate_entity.properties.extend(BTreeMap::from([
        (
            "workspace_member_source".to_owned(),
            "implicit_root_package".to_owned(),
        ),
        (
            "workspace_root_shape".to_owned(),
            "non_virtual_workspace".to_owned(),
        ),
    ]));
    let source_entity = WorkspaceEntity::source_file(
        repository_identity,
        &target.crate_id,
        "src/lib.rs",
        &"d".repeat(40),
    );
    let manifest_evidence = WorkspaceEvidence::complete_file(
        repository_identity,
        commit_oid,
        "Cargo.toml",
        &"c".repeat(40),
        112,
    );
    let source_evidence = WorkspaceEvidence::complete_file(
        repository_identity,
        commit_oid,
        "src/lib.rs",
        &"d".repeat(40),
        18,
    );
    let mut entities = vec![crate_entity, source_entity.clone()];
    entities.sort_by(|left, right| left.id.cmp(&right.id));
    let mut claims = entities
        .iter()
        .map(|entity| {
            WorkspaceClaim::new(
                ClaimSubjectKind::Entity,
                entity.id.clone(),
                ClaimState::DeterministicFact,
                if entity.id == target.crate_id {
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
        repository_identity: repository_identity.to_owned(),
        crate_id: target.crate_id.clone(),
        source_file_id: source_entity.id,
        entities: entities.clone(),
        relationships: Vec::new(),
        claims: claims.clone(),
        evidence: evidence.clone(),
        diagnostics: Vec::new(),
        coverage: Vec::new(),
    };
    RootPackageWorkspaceExtraction {
        knowledge: RootPackageWorkspaceKnowledge {
            plan: RootPackageWorkspacePlan {
                root_shape: RootPackageShape::NonVirtualWorkspace,
                members: vec![
                    RootPackageMember {
                        path: ".".to_owned(),
                        manifest_path: Some("Cargo.toml".to_owned()),
                        member_source: WorkspaceMemberSource::ImplicitRootPackage,
                        crate_ids: vec![target.crate_id.clone()],
                        external_boundary_id: None,
                    },
                    RootPackageMember {
                        path: external.path.clone(),
                        manifest_path: None,
                        member_source: WorkspaceMemberSource::LiteralMember,
                        crate_ids: Vec::new(),
                        external_boundary_id: Some(external.boundary_id.clone()),
                    },
                ],
                excluded_paths: Vec::new(),
                targets: vec![target],
            },
            knowledge: RustWorkspaceKnowledge {
                extraction_chunks: vec![chunk],
                graph: WorkspaceKnowledgeGraph {
                    repository_identity: repository_identity.to_owned(),
                    commit_oid: commit_oid.to_owned(),
                    entities,
                    relationships: Vec::new(),
                    claims,
                    evidence,
                    diagnostics: Vec::new(),
                    coverage: Vec::new(),
                },
            },
        },
        cache_entries: Vec::new(),
        source_records: Vec::new(),
        parser_invocation_count: 0,
    }
}

struct TestHasher;

impl BoundarySha256 for TestHasher {
    fn digest(&self, bytes: &[u8]) -> [u8; 32] {
        let mut digest = [0_u8; 32];
        for (index, byte) in bytes.iter().copied().enumerate() {
            digest[index % 32] ^= byte;
        }
        digest
    }
}

fn request() -> ScanRequest {
    ScanRequest::new(
        OsString::from("root"),
        RepositoryIdentity::parse(REPOSITORY_ID).expect("repository identity"),
        Revision::Commit(oid('a')),
        SnapshotEnvelopeV1::new(
            "2026-08-02T00:00:00Z".to_owned(),
            None,
            "r3-application".to_owned(),
        ),
    )
}

fn bound_revision() -> BoundRevision {
    BoundRevision::new(
        RepositoryIdentity::parse(REPOSITORY_ID).expect("repository identity"),
        oid('a'),
        oid('b'),
    )
}

fn oid(value: char) -> ObjectId {
    ObjectId::parse_sha1(&value.to_string().repeat(40)).expect("SHA-1 object ID")
}
