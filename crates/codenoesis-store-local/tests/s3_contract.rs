use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use codenoesis_domain::storage::{
    ArtifactId, ArtifactReference, ArtifactRole, GRAPH_HASH_DOMAIN, LocalSnapshotHead,
    PublicationBoundary, PublicationCandidate, PublicationEvent, PublicationResult,
    SNAPSHOT_HASH_DOMAIN, SNAPSHOT_SCHEMA_VERSION, SemanticHash, SnapshotId, SnapshotRecord,
    StorageError, StoredArtifact, SweepResult,
};
use codenoesis_domain::{ObjectId, RepositoryIdentity};
use codenoesis_ports::{ArtifactStore, MetadataStore, PublicationObserver};
use codenoesis_store_local::LocalStore;
use rusqlite::Connection;

#[test]
fn ct_fr_sto_001_metadata_store_parity() {
    let candidate = candidate();
    let mut fake = FakeMetadataStore::default();
    let fake_result = metadata_vector(&mut fake, &candidate);

    let fixture = StoreFixture::new();
    let mut real = LocalStore::open(&fixture.repository, &fixture.store)
        .expect("open real SQLite metadata adapter");
    let real_result = metadata_vector(&mut real.metadata, &candidate);

    assert_eq!(real_result, fake_result);
    let evidence = real.metadata.evidence().expect("read SQLite evidence");
    assert_eq!(evidence.sqlite_version, "3.53.2");
    assert!(!evidence.compile_options.is_empty());
    assert_eq!(evidence.journal_mode, "wal");
    assert_eq!(evidence.synchronous, 2);
    assert!(evidence.foreign_keys);
    assert!(!evidence.trusted_schema);
    assert_eq!(evidence.busy_timeout_milliseconds, 0);
    assert_immutable_trigger(&fixture, "snapshots");
    assert_writer_contention(&fixture, &mut real.metadata, &candidate);
    assert_schema_drift_rejected();
    assert_foreign_key_corruption_rejected(&candidate);
}

#[test]
fn ct_fr_sto_001_cas_parity() {
    let artifact = artifact(
        ArtifactRole::KnowledgeGraph,
        b"{\"graph\":\"contract\"}",
        GRAPH_HASH_DOMAIN,
        'b',
    );
    let mut fake = FakeArtifactStore::default();
    let fake_result = cas_vector(&mut fake, &artifact);

    let fixture = StoreFixture::new();
    let mut real = LocalStore::open(&fixture.repository, &fixture.store)
        .expect("open real filesystem CAS adapter");
    let real_result = cas_vector(&mut real.artifacts, &artifact);
    assert_eq!(real_result, fake_result);

    real.artifacts
        .stage(&artifact, &mut EventRecorder::default())
        .expect("restage artifact before corruption");
    let object_path = real.artifacts.object_path(&artifact.artifact_id);
    fs::write(&object_path, b"{\"graph\":\"corrupt!\"}").expect("corrupt final object");
    let error = real
        .artifacts
        .stage(&artifact, &mut EventRecorder::default())
        .expect_err("mismatched existing object must fail closed");
    assert!(matches!(error, StorageError::CorruptObject { .. }));
    assert_eq!(
        fs::read(object_path).expect("read preserved corrupt object"),
        b"{\"graph\":\"corrupt!\"}"
    );
}

#[derive(Debug, Eq, PartialEq)]
struct MetadataVector {
    events: Vec<PublicationBoundary>,
    generation: u64,
    retry_changed: bool,
    references: BTreeSet<ArtifactId>,
    conflict: StorageError,
}

fn metadata_vector<M: MetadataStore>(
    store: &mut M,
    candidate: &PublicationCandidate,
) -> MetadataVector {
    assert_eq!(
        store
            .current_head_id(&candidate.snapshot.repository_identity)
            .expect("read empty head"),
        None
    );
    let mut events = EventRecorder::default();
    let first = store
        .publish(candidate, None, &mut events)
        .expect("publish contract candidate");
    let retry = store
        .publish(
            candidate,
            Some(&candidate.snapshot.snapshot_id),
            &mut EventRecorder::default(),
        )
        .expect("retry contract candidate");
    assert_eq!(
        store
            .load_head(&candidate.snapshot.repository_identity)
            .expect("load contract head"),
        Some(first.head.clone())
    );
    let references = store
        .referenced_artifacts()
        .expect("read stable references");
    for artifact in &candidate.artifacts {
        assert!(
            store
                .is_artifact_referenced(&artifact.artifact_id)
                .expect("recheck stable reference")
        );
    }
    let conflict = store
        .publish(candidate, None, &mut EventRecorder::default())
        .expect_err("stale expected head must conflict");
    MetadataVector {
        events: events.boundaries,
        generation: first.head.generation,
        retry_changed: retry.changed,
        references,
        conflict,
    }
}

#[derive(Debug, Eq, PartialEq)]
struct CasVector {
    events: Vec<PublicationBoundary>,
    bytes: Vec<u8>,
    sweep: SweepResult,
    missing: StorageError,
}

fn cas_vector<C: ArtifactStore>(store: &mut C, artifact: &StoredArtifact) -> CasVector {
    let mut events = EventRecorder::default();
    store
        .stage(artifact, &mut events)
        .expect("stage new contract object");
    store
        .stage(artifact, &mut EventRecorder::default())
        .expect("reuse exact contract object");
    let bytes = store
        .read(&artifact.artifact_id, artifact.byte_length())
        .expect("read exact contract object");
    let sweep = store
        .sweep(&BTreeSet::new(), &mut |_| Ok(false))
        .expect("sweep unreferenced contract object");
    let missing = store
        .read(&artifact.artifact_id, artifact.byte_length())
        .expect_err("swept object must be missing");
    CasVector {
        events: events.boundaries,
        bytes,
        sweep,
        missing,
    }
}

#[derive(Default)]
struct EventRecorder {
    boundaries: Vec<PublicationBoundary>,
}

impl PublicationObserver for EventRecorder {
    fn observe(&mut self, event: &PublicationEvent) -> Result<(), StorageError> {
        self.boundaries.push(event.boundary);
        Ok(())
    }
}

#[derive(Default)]
struct FakeMetadataStore {
    head: Option<LocalSnapshotHead>,
    references: BTreeSet<ArtifactId>,
}

impl MetadataStore for FakeMetadataStore {
    fn current_head_id(
        &self,
        repository_identity: &RepositoryIdentity,
    ) -> Result<Option<SnapshotId>, StorageError> {
        Ok(self
            .head
            .as_ref()
            .filter(|head| head.repository_identity == *repository_identity)
            .map(|head| head.snapshot_id.clone()))
    }

    fn publish(
        &mut self,
        candidate: &PublicationCandidate,
        expected_head: Option<&SnapshotId>,
        observer: &mut dyn PublicationObserver,
    ) -> Result<PublicationResult, StorageError> {
        candidate.validate()?;
        observer.observe(&PublicationEvent::sqlite(
            PublicationBoundary::SqliteAfterBegin,
        ))?;
        observer.observe(&PublicationEvent::sqlite(
            PublicationBoundary::SqliteAfterSnapshotRows,
        ))?;
        let current = self.head.as_ref().map(|head| head.snapshot_id.clone());
        if current.as_ref() != expected_head {
            return Err(StorageError::HeadConflict {
                expected: expected_head.map(ToString::to_string),
                actual: current.map(|value| value.to_string()),
            });
        }
        let changed = current.as_ref() != Some(&candidate.snapshot.snapshot_id);
        let generation = self.head.as_ref().map_or(1, |head| {
            if changed {
                head.generation + 1
            } else {
                head.generation
            }
        });
        let head = head(candidate, generation);
        self.head = Some(head.clone());
        self.references.extend(
            candidate
                .artifacts
                .iter()
                .map(|value| value.artifact_id.clone()),
        );
        observer.observe(&PublicationEvent::sqlite(
            PublicationBoundary::SqliteAfterHeadUpdate,
        ))?;
        observer.observe(&PublicationEvent::sqlite(
            PublicationBoundary::SqliteAfterCommit,
        ))?;
        Ok(PublicationResult { head, changed })
    }

    fn load_head(
        &self,
        repository_identity: &RepositoryIdentity,
    ) -> Result<Option<LocalSnapshotHead>, StorageError> {
        Ok(self
            .head
            .clone()
            .filter(|head| head.repository_identity == *repository_identity))
    }

    fn referenced_artifacts(&self) -> Result<BTreeSet<ArtifactId>, StorageError> {
        Ok(self.references.clone())
    }

    fn is_artifact_referenced(&self, artifact_id: &ArtifactId) -> Result<bool, StorageError> {
        Ok(self.references.contains(artifact_id))
    }
}

#[derive(Default)]
struct FakeArtifactStore {
    objects: BTreeMap<ArtifactId, Vec<u8>>,
}

impl ArtifactStore for FakeArtifactStore {
    fn stage(
        &mut self,
        artifact: &StoredArtifact,
        observer: &mut dyn PublicationObserver,
    ) -> Result<(), StorageError> {
        if let Some(bytes) = self.objects.get(&artifact.artifact_id) {
            return verify_bytes(&artifact.artifact_id, artifact.byte_length(), bytes);
        }
        for boundary in [
            PublicationBoundary::CasBeforeTempCreate,
            PublicationBoundary::CasAfterTempSync,
            PublicationBoundary::CasAfterObjectMove,
            PublicationBoundary::CasAfterParentSync,
        ] {
            observer.observe(&PublicationEvent::cas(
                boundary,
                artifact.role,
                artifact.ordinal,
            ))?;
        }
        self.objects
            .insert(artifact.artifact_id.clone(), artifact.bytes.clone());
        Ok(())
    }

    fn read(&self, artifact_id: &ArtifactId, byte_length: u64) -> Result<Vec<u8>, StorageError> {
        let bytes = self
            .objects
            .get(artifact_id)
            .ok_or_else(|| StorageError::MissingObject {
                artifact_id: artifact_id.to_string(),
            })?;
        verify_bytes(artifact_id, byte_length, bytes)?;
        Ok(bytes.clone())
    }

    fn sweep(
        &mut self,
        reachable: &BTreeSet<ArtifactId>,
        recheck: &mut dyn FnMut(&ArtifactId) -> Result<bool, StorageError>,
    ) -> Result<SweepResult, StorageError> {
        let mut removed = 0_u64;
        let identifiers = self.objects.keys().cloned().collect::<Vec<_>>();
        for artifact_id in identifiers {
            if !reachable.contains(&artifact_id) && !recheck(&artifact_id)? {
                self.objects.remove(&artifact_id);
                removed += 1;
            }
        }
        Ok(SweepResult {
            temporary_files_removed: 0,
            objects_removed: removed,
        })
    }
}

struct StoreFixture {
    _root: tempfile::TempDir,
    repository: std::path::PathBuf,
    store: std::path::PathBuf,
}

impl StoreFixture {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("create contract fixture root");
        let repository = root.path().join("repository");
        let store = root.path().join("store");
        fs::create_dir(&repository).expect("create contract repository");
        Self {
            _root: root,
            repository,
            store,
        }
    }
}

fn candidate() -> PublicationCandidate {
    let repository_identity =
        RepositoryIdentity::parse("urn:codenoesis:test:s3-contract").expect("repository identity");
    let semantic_hash = SemanticHash::blake3(SNAPSHOT_HASH_DOMAIN, &"a".repeat(64));
    let graph_hash = SemanticHash::blake3(GRAPH_HASH_DOMAIN, &"b".repeat(64));
    let snapshot_id =
        SnapshotId::from_semantic_hash(&semantic_hash.value).expect("snapshot identity");
    PublicationCandidate {
        snapshot: SnapshotRecord {
            snapshot_id,
            repository_identity,
            commit_oid: ObjectId::parse_sha1(&"0".repeat(40)).expect("commit identity"),
            snapshot_schema_version: SNAPSHOT_SCHEMA_VERSION.to_owned(),
            semantic_hash: semantic_hash.clone(),
            graph_semantic_hash: graph_hash.clone(),
        },
        artifacts: vec![
            StoredArtifact::new(
                ArtifactRole::SnapshotSemantic,
                0,
                b"{\"snapshot\":true}".to_vec(),
                semantic_hash,
            ),
            StoredArtifact::new(
                ArtifactRole::KnowledgeGraph,
                0,
                b"{\"graph\":true}".to_vec(),
                graph_hash,
            ),
        ],
        extraction_chunks: Vec::new(),
        entities: Vec::new(),
        relationships: Vec::new(),
        claims: Vec::new(),
        evidence: Vec::new(),
        diagnostics: Vec::new(),
        coverage_gaps: Vec::new(),
    }
}

fn artifact(
    role: ArtifactRole,
    bytes: &[u8],
    domain: &str,
    hash_character: char,
) -> StoredArtifact {
    StoredArtifact::new(
        role,
        0,
        bytes.to_vec(),
        SemanticHash::blake3(domain, &hash_character.to_string().repeat(64)),
    )
}

fn head(candidate: &PublicationCandidate, generation: u64) -> LocalSnapshotHead {
    LocalSnapshotHead {
        repository_identity: candidate.snapshot.repository_identity.clone(),
        snapshot_id: candidate.snapshot.snapshot_id.clone(),
        commit_oid: candidate.snapshot.commit_oid.clone(),
        snapshot_schema_version: candidate.snapshot.snapshot_schema_version.clone(),
        semantic_hash: candidate.snapshot.semantic_hash.clone(),
        graph_semantic_hash: candidate.snapshot.graph_semantic_hash.clone(),
        generation,
        artifacts: candidate
            .artifacts
            .iter()
            .map(ArtifactReference::from)
            .collect(),
    }
}

fn verify_bytes(
    artifact_id: &ArtifactId,
    byte_length: u64,
    bytes: &[u8],
) -> Result<(), StorageError> {
    let observed = ArtifactId::from_bytes(bytes);
    if u64::try_from(bytes.len()).ok() != Some(byte_length) || observed != *artifact_id {
        return Err(StorageError::CorruptObject {
            artifact_id: artifact_id.to_string(),
            expected_hash: artifact_id.digest().to_owned(),
            observed_hash: observed.digest().to_owned(),
        });
    }
    Ok(())
}

fn assert_immutable_trigger(fixture: &StoreFixture, table: &str) {
    let connection =
        Connection::open(fixture.store.join("metadata.sqlite3")).expect("open metadata database");
    let statement = match table {
        "snapshots" => "UPDATE snapshots SET commit_oid = commit_oid",
        _ => panic!("unsupported immutable table"),
    };
    assert!(
        connection.execute(statement, []).is_err(),
        "immutable trigger must reject updates"
    );
}

fn assert_writer_contention(
    fixture: &StoreFixture,
    store: &mut impl MetadataStore,
    candidate: &PublicationCandidate,
) {
    let lock =
        Connection::open(fixture.store.join("metadata.sqlite3")).expect("open competing writer");
    lock.execute_batch("PRAGMA busy_timeout = 0; BEGIN IMMEDIATE;")
        .expect("hold competing immediate writer");
    let error = store
        .publish(
            candidate,
            Some(&candidate.snapshot.snapshot_id),
            &mut EventRecorder::default(),
        )
        .expect_err("competing writer must fail without retry");
    assert_eq!(error, StorageError::WriterBusy);
    lock.execute_batch("ROLLBACK;")
        .expect("release writer lock");
}

fn assert_schema_drift_rejected() {
    let fixture = StoreFixture::new();
    drop(
        LocalStore::open(&fixture.repository, &fixture.store)
            .expect("initialize schema-drift fixture"),
    );
    let connection =
        Connection::open(fixture.store.join("metadata.sqlite3")).expect("open metadata database");
    connection
        .execute_batch("DROP TRIGGER snapshots_forbid_update;")
        .expect("apply schema drift");
    drop(connection);
    let Err(error) = LocalStore::open(&fixture.repository, &fixture.store) else {
        panic!("schema drift must fail closed");
    };
    assert!(matches!(
        error,
        StorageError::CorruptMetadata {
            reason: "schema_drift",
            ..
        }
    ));
}

fn assert_foreign_key_corruption_rejected(candidate: &PublicationCandidate) {
    let fixture = StoreFixture::new();
    let mut store = LocalStore::open(&fixture.repository, &fixture.store)
        .expect("initialize foreign-key fixture");
    store
        .metadata
        .publish(candidate, None, &mut EventRecorder::default())
        .expect("publish foreign-key fixture head");
    drop(store);
    let connection =
        Connection::open(fixture.store.join("metadata.sqlite3")).expect("open metadata database");
    connection
        .execute_batch(
            "PRAGMA foreign_keys = OFF;
             UPDATE project_heads
             SET snapshot_id =
               'urn:codenoesis:snapshot:blake3:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff',
                 generation = 2;",
        )
        .expect("apply foreign-key corruption");
    drop(connection);
    let Err(error) = LocalStore::open(&fixture.repository, &fixture.store) else {
        panic!("foreign-key corruption must fail closed");
    };
    assert!(matches!(
        error,
        StorageError::CorruptMetadata {
            reason: "foreign_key_check_failed",
            ..
        }
    ));
}
