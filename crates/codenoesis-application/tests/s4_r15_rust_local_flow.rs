use std::ffi::{OsStr, OsString};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use codenoesis_application::{LocalFlowScanError, ScanRequest, ScanService};
use codenoesis_contracts::SnapshotEnvelopeV1;
use codenoesis_domain::s4_r15::LocalFlowError;
use codenoesis_domain::{
    AcquiredFile, AcquiredRepository, BoundRevision, K1OutputCapacityProfile, ObjectId,
    RegularFileMode, RepositoryError, RepositoryIdentity, Revision,
};
use codenoesis_ports::{RepositoryAcquirer, SafeRepositoryAcquirer};

#[test]
fn gt_fr_ext_017_application_acquires_once_and_preserves_typed_r15_failure() {
    let acquisitions = Arc::new(AtomicUsize::new(0));
    let extractions = Arc::new(AtomicUsize::new(0));
    let service = ScanService::new(MockAcquirer {
        acquisitions: acquisitions.clone(),
    });
    let extraction_calls = extractions.clone();
    let error = service
        .scan_s4_r15(
            request(),
            K1OutputCapacityProfile::Standard,
            move |inventory| {
                extraction_calls.fetch_add(1, Ordering::Relaxed);
                assert_eq!(inventory.files().len(), 2);
                assert_eq!(
                    inventory.bound_revision().repository_identity().as_str(),
                    "urn:codenoesis:test:r15-application"
                );
                Err(LocalFlowError::Cycle)
            },
        )
        .err()
        .expect("typed R15 extraction failure");
    assert_eq!(error, LocalFlowScanError::LocalFlow(LocalFlowError::Cycle));
    assert_eq!(acquisitions.load(Ordering::Relaxed), 1);
    assert_eq!(extractions.load(Ordering::Relaxed), 1);
}

struct MockAcquirer {
    acquisitions: Arc<AtomicUsize>,
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
        self.acquisitions.fetch_add(1, Ordering::Relaxed);
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
                    b"[package]\nname=\"r15-application\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[lib]\npath=\"src/lib.rs\"\n"
                        .to_vec(),
                ),
                AcquiredFile::new(
                    "src/lib.rs".to_owned(),
                    RegularFileMode::Regular,
                    oid('d'),
                    b"pub fn local_flow(value: i32) -> i32 { value }\n".to_vec(),
                ),
            ],
        ))
    }
}

fn request() -> ScanRequest {
    ScanRequest::new(
        OsString::from("repository"),
        RepositoryIdentity::parse("urn:codenoesis:test:r15-application")
            .expect("R15 application repository identity"),
        Revision::Commit(oid('a')),
        SnapshotEnvelopeV1::new(
            "2026-08-11T00:00:00Z".to_owned(),
            None,
            "r15-application".to_owned(),
        ),
    )
}

fn oid(value: char) -> ObjectId {
    ObjectId::parse_sha1(&value.to_string().repeat(40)).expect("synthetic SHA-1 object ID")
}
