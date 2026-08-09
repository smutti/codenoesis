use std::ffi::{OsStr, OsString};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use codenoesis_application::{RustCfgAlternativesScanError, ScanRequest, ScanService};
use codenoesis_contracts::SnapshotEnvelopeV1;
use codenoesis_domain::s4_r3::ExternalWorkspaceBoundary;
use codenoesis_domain::s4_r10::{
    RustCfgDeclarationAlternativesError, RustCfgDeclarationAlternativesExtraction,
};
use codenoesis_domain::s5::AnalysisCacheEntry;
use codenoesis_domain::{
    AcquiredFile, AcquiredRepository, BoundRevision, ObjectId, RegularFileMode, RepositoryError,
    RepositoryIdentity, RepositoryInventory, Revision,
};
use codenoesis_ports::{
    RepositoryAcquirer, RustCfgDeclarationAlternativesExtractor, SafeRepositoryAcquirer,
};

#[test]
fn ft_fr_ext_013_application_propagates_typed_r10_failure_atomically() {
    let acquisition_calls = Arc::new(AtomicUsize::new(0));
    let extraction_calls = Arc::new(AtomicUsize::new(0));
    let service = ScanService::new(MockAcquirer {
        calls: acquisition_calls.clone(),
    });
    let expected = RustCfgDeclarationAlternativesError::IdentityMismatch {
        logical_method_id: "logical-method".to_owned(),
        reason: "direct_cfg_required",
    };
    let Err(error) = service.scan_s4_r10(
        request(),
        &RejectingExtractor {
            calls: extraction_calls.clone(),
            error: expected.clone(),
        },
    ) else {
        panic!("typed R10 failure must terminate before snapshot construction");
    };
    assert_eq!(error, RustCfgAlternativesScanError::Alternatives(expected));
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
                    b"[package]\nname=\"r10-application\"\nversion=\"0.1.0\"\nedition=\"2024\"\n"
                        .to_vec(),
                ),
                AcquiredFile::new(
                    "src/lib.rs".to_owned(),
                    RegularFileMode::Regular,
                    oid('d'),
                    b"pub struct Client;\n".to_vec(),
                ),
            ],
        ))
    }
}

struct RejectingExtractor {
    calls: Arc<AtomicUsize>,
    error: RustCfgDeclarationAlternativesError,
}

impl RustCfgDeclarationAlternativesExtractor for RejectingExtractor {
    fn extract_rust_cfg_declaration_alternatives_incremental(
        &self,
        inventory: &RepositoryInventory,
        external_boundaries: &[ExternalWorkspaceBoundary],
        cache_entries: &[AnalysisCacheEntry],
    ) -> Result<RustCfgDeclarationAlternativesExtraction, RustCfgDeclarationAlternativesError> {
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
        RepositoryIdentity::parse("urn:codenoesis:test:r10-application")
            .expect("R10 application repository identity"),
        Revision::Commit(oid('a')),
        SnapshotEnvelopeV1::new(
            "2026-08-09T00:00:00Z".to_owned(),
            None,
            "r10-application".to_owned(),
        ),
    )
}

fn oid(value: char) -> ObjectId {
    ObjectId::parse_sha1(&value.to_string().repeat(40)).expect("synthetic SHA-1 object ID")
}
