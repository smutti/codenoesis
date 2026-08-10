use std::ffi::{OsStr, OsString};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use codenoesis_application::{
    CallableCfgAlternativesScanError, RepositoryBoundaryScanInput, ScanRequest, ScanService,
};
use codenoesis_contracts::SnapshotEnvelopeV1;
use codenoesis_domain::s1_boundaries::{
    AcquiredGitlink, AcquiredRepositoryBoundaries, BoundarySha256, NestedAcquisitionProfile,
    NestedRepositoryAcquisitionError, RepositoryBoundaryAcquisitionError,
};
use codenoesis_domain::s4_k1::CallableSemanticsError;
use codenoesis_domain::s4_r3::ExternalWorkspaceBoundary;
use codenoesis_domain::s4_r12::{CallableCfgAlternativesError, CallableCfgAlternativesExtraction};
use codenoesis_domain::{
    AcquiredFile, AcquiredRepository, BoundRevision, ObjectId, RegularFileMode, RepositoryError,
    RepositoryIdentity, RepositoryInventory, Revision,
};
use codenoesis_ports::{
    RepositoryAcquirer, RepositoryBoundaryAcquirer, RustCallableCfgAlternativesCompositionExtractor,
};

#[test]
fn gt_fr_ext_014_application_passes_only_canonical_boundary_pairs_to_r12() {
    let counters = Arc::new(Counters::default());
    let service = ScanService::new(MockBoundaryAcquirer {
        counters: counters.clone(),
    });
    let error = service
        .scan_s4_r12_boundaries(
            request(),
            RepositoryBoundaryScanInput {
                manifest: None,
                nested_roots: Vec::new(),
            },
            &RejectingExtractor {
                calls: counters.extractor.clone(),
            },
            &TestHasher,
        )
        .err()
        .expect("typed extraction failure terminates R12 composition");
    assert_eq!(
        error,
        CallableCfgAlternativesScanError::Composition(CallableCfgAlternativesError::Callable(
            CallableSemanticsError::UnsupportedComposition
        ))
    );
    assert_eq!(counters.root.load(Ordering::Relaxed), 1);
    assert_eq!(counters.nested.load(Ordering::Relaxed), 0);
    assert_eq!(counters.extractor.load(Ordering::Relaxed), 1);
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
                2,
                vec![
                    AcquiredFile::new(
                        "Cargo.toml".to_owned(),
                        RegularFileMode::Regular,
                        oid('c'),
                        b"[package]\nname=\"r12-application\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[lib]\npath=\"src/lib.rs\"\n"
                            .to_vec(),
                    ),
                    AcquiredFile::new(
                        "src/lib.rs".to_owned(),
                        RegularFileMode::Regular,
                        oid('d'),
                        b"pub fn root_callable() {}\n".to_vec(),
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

struct RejectingExtractor {
    calls: Arc<AtomicUsize>,
}

impl RustCallableCfgAlternativesCompositionExtractor for RejectingExtractor {
    fn extract_rust_callable_cfg_alternatives_with_boundaries(
        &self,
        inventory: &RepositoryInventory,
        external_boundaries: &[ExternalWorkspaceBoundary],
    ) -> Result<CallableCfgAlternativesExtraction, CallableCfgAlternativesError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        assert_eq!(inventory.files().len(), 2);
        assert_eq!(external_boundaries.len(), 1);
        assert_eq!(external_boundaries[0].path, "external/model");
        assert!(
            external_boundaries[0]
                .boundary_id
                .starts_with("urn:codenoesis:repository-boundary:sha256:")
        );
        Err(CallableCfgAlternativesError::Callable(
            CallableSemanticsError::UnsupportedComposition,
        ))
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
        RepositoryIdentity::parse("urn:codenoesis:test:r12-application")
            .expect("R12 application repository identity"),
        Revision::Commit(oid('a')),
        SnapshotEnvelopeV1::new(
            "2026-08-10T00:00:00Z".to_owned(),
            None,
            "r12-application".to_owned(),
        ),
    )
}

fn bound_revision() -> BoundRevision {
    BoundRevision::new(
        RepositoryIdentity::parse("urn:codenoesis:test:r12-application")
            .expect("R12 application repository identity"),
        oid('a'),
        oid('b'),
    )
}

fn oid(value: char) -> ObjectId {
    ObjectId::parse_sha1(&value.to_string().repeat(40)).expect("synthetic SHA-1 object ID")
}
