use std::ffi::{OsStr, OsString};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use codenoesis_application::{
    BoundaryScanError, PreparedNestedRepositoryRoot, RepositoryBoundaryScanInput, ScanRequest,
    ScanService,
};
use codenoesis_contracts::SnapshotEnvelopeV1;
use codenoesis_domain::s1_boundaries::{
    AcquiredGitlink, AcquiredRepositoryBoundaries, BoundarySha256, NestedAcquisitionProfile,
    NestedRepositoryAcquisitionError, NestedRepositoryInput, RepositoryBoundaryAcquisitionError,
    RepositoryBoundaryInput,
};
use codenoesis_domain::s4::WorkspaceError;
use codenoesis_domain::s5::{AnalysisCacheEntry, IncrementalWorkspaceExtraction};
use codenoesis_domain::{
    AcquiredRepository, BoundRevision, ObjectId, RepositoryError, RepositoryIdentity, Revision,
};
use codenoesis_ports::{
    IncrementalRustWorkspaceExtractor, RepositoryAcquirer, RepositoryBoundaryAcquirer,
};

#[test]
fn conf_fr_acq_005_root_manifest_mismatch_precedes_root_acquisition() {
    let counters = Arc::new(Counters::default());
    let service = ScanService::new(MockAcquirer::new(counters.clone()));
    let mut manifest = matching_manifest();
    manifest.root_commit_oid = oid('9');
    let error = service
        .scan_s4_boundaries(
            request(),
            RepositoryBoundaryScanInput {
                manifest: Some(manifest),
                nested_roots: vec![PreparedNestedRepositoryRoot::Available(OsString::from(
                    "nested",
                ))],
            },
            &NeverExtractor,
            &TestHasher,
        )
        .err()
        .expect("root mismatch must fail");
    assert_eq!(error, BoundaryScanError::InvalidManifest);
    assert_eq!(counters.root.load(Ordering::Relaxed), 0);
    assert_eq!(counters.nested.load(Ordering::Relaxed), 0);
}

#[test]
fn gt_fr_acq_005_nested_oid_mismatch_precedes_nested_open() {
    let counters = Arc::new(Counters::default());
    let service = ScanService::new(MockAcquirer::new(counters.clone()));
    let mut manifest = matching_manifest();
    manifest.nested_repositories[0].revision = oid('2');
    let error = service
        .scan_s4_boundaries(
            request(),
            RepositoryBoundaryScanInput {
                manifest: Some(manifest),
                nested_roots: vec![PreparedNestedRepositoryRoot::Available(OsString::from(
                    "nested",
                ))],
            },
            &NeverExtractor,
            &TestHasher,
        )
        .err()
        .expect("nested mismatch must fail");
    assert_eq!(
        error,
        BoundaryScanError::NestedMismatch {
            path: "external/model".to_owned(),
            expected: oid('1'),
            observed: oid('2'),
        }
    );
    assert_eq!(counters.root.load(Ordering::Relaxed), 1);
    assert_eq!(counters.nested.load(Ordering::Relaxed), 0);
}

#[test]
fn gt_fr_acq_005_missing_manifest_boundary_precedes_nested_open() {
    let counters = Arc::new(Counters::default());
    let mut acquirer = MockAcquirer::new(counters.clone());
    acquirer.acquired.gitlinks.clear();
    let service = ScanService::new(acquirer);
    let error = service
        .scan_s4_boundaries(
            request(),
            RepositoryBoundaryScanInput {
                manifest: Some(matching_manifest()),
                nested_roots: vec![PreparedNestedRepositoryRoot::Available(OsString::from(
                    "nested",
                ))],
            },
            &NeverExtractor,
            &TestHasher,
        )
        .err()
        .expect("missing boundary must fail");
    assert_eq!(error, BoundaryScanError::InvalidManifest);
    assert_eq!(counters.root.load(Ordering::Relaxed), 1);
    assert_eq!(counters.nested.load(Ordering::Relaxed), 0);
}

#[test]
fn gt_fr_acq_005_nested_path_failure_follows_oid_match() {
    let counters = Arc::new(Counters::default());
    let service = ScanService::new(MockAcquirer::new(counters.clone()));
    let error = service
        .scan_s4_boundaries(
            request(),
            RepositoryBoundaryScanInput {
                manifest: Some(matching_manifest()),
                nested_roots: vec![PreparedNestedRepositoryRoot::Unavailable],
            },
            &NeverExtractor,
            &TestHasher,
        )
        .err()
        .expect("unavailable confined path must fail");
    assert_eq!(
        error,
        BoundaryScanError::NestedPathUnavailable {
            path: "external/model".to_owned(),
        }
    );
    assert_eq!(counters.root.load(Ordering::Relaxed), 1);
    assert_eq!(counters.nested.load(Ordering::Relaxed), 0);
}

#[derive(Default)]
struct Counters {
    root: AtomicUsize,
    nested: AtomicUsize,
}

struct MockAcquirer {
    counters: Arc<Counters>,
    acquired: AcquiredRepositoryBoundaries,
}

impl MockAcquirer {
    fn new(counters: Arc<Counters>) -> Self {
        let root = bound("urn:codenoesis:repository:root", 'a', 'b');
        Self {
            counters,
            acquired: AcquiredRepositoryBoundaries {
                repository: AcquiredRepository::new(root, 0, Vec::new()),
                gitlinks: vec![AcquiredGitlink {
                    path: "external/model".to_owned(),
                    containing_tree_oid: oid('b'),
                    gitlink_oid: oid('1'),
                }],
                gitmodules: None,
            },
        }
    }
}

impl RepositoryBoundaryAcquirer for MockAcquirer {
    fn acquire_inventory_with_boundaries(
        &self,
        _repository: &OsStr,
        _identity: RepositoryIdentity,
        _revision: Revision,
    ) -> Result<AcquiredRepositoryBoundaries, RepositoryBoundaryAcquisitionError> {
        self.counters.root.fetch_add(1, Ordering::Relaxed);
        Ok(self.acquired.clone())
    }

    fn bind_nested_repository(
        &self,
        _repository: &OsStr,
        identity: RepositoryIdentity,
        revision: Revision,
        _profile: NestedAcquisitionProfile,
    ) -> Result<BoundRevision, NestedRepositoryAcquisitionError> {
        self.counters.nested.fetch_add(1, Ordering::Relaxed);
        let Revision::Commit(commit) = revision else {
            return Err(NestedRepositoryAcquisitionError::Repository(
                RepositoryError::Unexpected,
            ));
        };
        Ok(BoundRevision::new(identity, commit, oid('3')))
    }
}

impl RepositoryAcquirer for MockAcquirer {
    fn bind(
        &self,
        _repository: &OsStr,
        _identity: RepositoryIdentity,
        _revision: Revision,
    ) -> Result<BoundRevision, RepositoryError> {
        Err(RepositoryError::Unexpected)
    }
}

struct NeverExtractor;

impl IncrementalRustWorkspaceExtractor for NeverExtractor {
    fn extract_workspace_incremental(
        &self,
        _inventory: &codenoesis_domain::RepositoryInventory,
        _cache_entries: &[AnalysisCacheEntry],
    ) -> Result<IncrementalWorkspaceExtraction, WorkspaceError> {
        panic!("failure precedence must stop before extraction")
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
        identity("urn:codenoesis:repository:root"),
        Revision::Commit(oid('a')),
        SnapshotEnvelopeV1::new(
            "2026-08-02T00:00:00Z".to_owned(),
            None,
            "r2-test".to_owned(),
        ),
    )
}

fn matching_manifest() -> RepositoryBoundaryInput {
    RepositoryBoundaryInput {
        root_repository_identity: identity("urn:codenoesis:repository:root"),
        root_commit_oid: oid('a'),
        nested_repositories: vec![NestedRepositoryInput {
            boundary_path: "external/model".to_owned(),
            repository_identity: identity("urn:codenoesis:repository:nested"),
            repository_root: "nested".to_owned(),
            revision: oid('1'),
            acquisition_profile: NestedAcquisitionProfile::VerifiedLooseSha1V1,
        }],
    }
}

fn bound(identity_value: &str, commit: char, tree: char) -> BoundRevision {
    BoundRevision::new(identity(identity_value), oid(commit), oid(tree))
}

fn identity(value: &str) -> RepositoryIdentity {
    RepositoryIdentity::parse(value).unwrap()
}

fn oid(value: char) -> ObjectId {
    ObjectId::parse_sha1(&value.to_string().repeat(40)).unwrap()
}
