use std::ffi::{OsStr, OsString};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use codenoesis_application::{FrameworkScanError, ScanRequest, ScanService};
use codenoesis_contracts::SnapshotEnvelopeV1;
use codenoesis_domain::s4_r3::ExternalWorkspaceBoundary;
use codenoesis_domain::s4_r6::{FrameworkError, FrameworkExtraction};
use codenoesis_domain::s5::AnalysisCacheEntry;
use codenoesis_domain::{
    AcquiredFile, AcquiredRepository, BoundRevision, ObjectId, RegularFileMode, RepositoryError,
    RepositoryIdentity, RepositoryInventory, Revision,
};
use codenoesis_ports::{
    RepositoryAcquirer, RustFrameworkDeclarationExtractor, SafeRepositoryAcquirer,
};

#[test]
fn gt_fr_ext_011_application_propagates_typed_r6_failure() {
    let acquisition_calls = Arc::new(AtomicUsize::new(0));
    let extraction_calls = Arc::new(AtomicUsize::new(0));
    let service = ScanService::new(MockAcquirer {
        calls: acquisition_calls.clone(),
    });
    let expected = FrameworkError::InvalidDeclaration {
        path: "src/lib.rs".to_owned(),
        reason: "reviewed_literal_required".to_owned(),
    };

    let Err(error) = service.scan_s4_r6(
        request(),
        &RejectingExtractor {
            calls: extraction_calls.clone(),
            error: expected.clone(),
        },
    ) else {
        panic!("typed R6 extraction failure must terminate the application journey");
    };

    assert_eq!(error, FrameworkScanError::Framework(expected));
    assert_eq!(acquisition_calls.load(Ordering::Relaxed), 1);
    assert_eq!(extraction_calls.load(Ordering::Relaxed), 1);
}

struct MockAcquirer {
    calls: Arc<AtomicUsize>,
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

impl SafeRepositoryAcquirer for MockAcquirer {
    fn acquire_inventory(
        &self,
        _repository: &OsStr,
        identity: RepositoryIdentity,
        revision: Revision,
    ) -> Result<AcquiredRepository, RepositoryError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        let Revision::Commit(commit_oid) = revision else {
            return Err(RepositoryError::Unexpected);
        };
        Ok(AcquiredRepository::new(
            BoundRevision::new(identity, commit_oid, oid('b')),
            2,
            vec![
                AcquiredFile::new(
                    "Cargo.toml".to_owned(),
                    RegularFileMode::Regular,
                    oid('c'),
                    b"[package]\nname=\"r6-application\"\nversion=\"0.1.0\"\nedition=\"2024\"\n"
                        .to_vec(),
                ),
                AcquiredFile::new(
                    "src/lib.rs".to_owned(),
                    RegularFileMode::Regular,
                    oid('d'),
                    b"pub struct Anchor { pub value: u8 }\n".to_vec(),
                ),
            ],
        ))
    }
}

struct RejectingExtractor {
    calls: Arc<AtomicUsize>,
    error: FrameworkError,
}

impl RustFrameworkDeclarationExtractor for RejectingExtractor {
    fn extract_rust_framework_declarations_incremental(
        &self,
        inventory: &RepositoryInventory,
        external_boundaries: &[ExternalWorkspaceBoundary],
        cache_entries: &[AnalysisCacheEntry],
    ) -> Result<FrameworkExtraction, FrameworkError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        assert_eq!(inventory.files().len(), 2);
        assert!(external_boundaries.is_empty());
        assert!(cache_entries.is_empty());
        Err(self.error.clone())
    }
}

fn request() -> ScanRequest {
    ScanRequest::new(
        OsString::from("repository"),
        RepositoryIdentity::parse("urn:codenoesis:test:r6-application")
            .expect("R6 application repository identity"),
        Revision::Commit(oid('a')),
        SnapshotEnvelopeV1::new(
            "2026-08-05T00:00:00Z".to_owned(),
            None,
            "r6-application".to_owned(),
        ),
    )
}

fn oid(value: char) -> ObjectId {
    ObjectId::parse_sha1(&value.to_string().repeat(40)).expect("synthetic SHA-1 object ID")
}
