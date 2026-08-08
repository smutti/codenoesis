use std::ffi::{OsStr, OsString};

use codenoesis_application::{CallableSemanticsScanError, ScanRequest, ScanService};
use codenoesis_contracts::SnapshotEnvelopeV1;
use codenoesis_domain::s4_k1::CallableSemanticsError;
use codenoesis_domain::{
    AcquiredFile, AcquiredRepository, BoundRevision, ObjectId, RegularFileMode, RepositoryError,
    RepositoryIdentity, Revision,
};
use codenoesis_ports::{RepositoryAcquirer, SafeRepositoryAcquirer};

#[test]
fn gt_fr_ext_012_application_preserves_typed_k1_failure() {
    let service = ScanService::new(MockAcquirer);
    let Err(error) = service.scan_s4_k1(request(), |_| {
        Err(CallableSemanticsError::UnsupportedComposition)
    }) else {
        panic!("K1 extraction failure must terminate before publication");
    };
    assert_eq!(
        error,
        CallableSemanticsScanError::Callable(CallableSemanticsError::UnsupportedComposition)
    );
}

struct MockAcquirer;

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
                    b"[package]\nname=\"k1-application\"\nversion=\"0.1.0\"\nedition=\"2024\"\n"
                        .to_vec(),
                ),
                AcquiredFile::new(
                    "src/lib.rs".to_owned(),
                    RegularFileMode::Regular,
                    oid('d'),
                    b"pub fn callable(value: u8) -> u8 { value }\n".to_vec(),
                ),
            ],
        ))
    }
}

fn request() -> ScanRequest {
    ScanRequest::new(
        OsString::from("repository"),
        RepositoryIdentity::parse("urn:codenoesis:test:k1-application")
            .expect("K1 application repository identity"),
        Revision::Commit(oid('a')),
        SnapshotEnvelopeV1::new(
            "2026-08-08T00:00:00Z".to_owned(),
            None,
            "k1-application".to_owned(),
        ),
    )
}

fn oid(value: char) -> ObjectId {
    ObjectId::parse_sha1(&value.to_string().repeat(40)).expect("synthetic SHA-1 object ID")
}
