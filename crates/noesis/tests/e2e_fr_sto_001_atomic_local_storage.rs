mod support;

use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use codenoesis_application::{PublicationService, ScanRequest, ScanService};
use codenoesis_contracts::{LocalSnapshotHeadV1, RepositorySnapshotV3, SnapshotEnvelopeV1};
use codenoesis_domain::storage::{
    ArtifactRole, PublicationBoundary, PublicationEvent, SemanticHash, StorageError, StoredArtifact,
};
use codenoesis_domain::{RepositoryIdentity, Revision};
use codenoesis_lang_rust::TreeSitterRustExtractor;
use codenoesis_ports::{ArtifactStore, NoopPublicationObserver, PublicationObserver};
use codenoesis_repository::LocalGitRepository;
use codenoesis_store_local::LocalStore;
use rusqlite::Connection;
use serde_json::Value;
use support::s3::{
    COMMIT_A_OID, COMMIT_B_OID, MaterializedRepository, REPOSITORY_ID, fixture_root, scan,
    scan_command,
};
use support::{parse_single_document, read_json};

const PROBE_STORE: &str = "CODENOESIS_S3_PROBE_STORE";
const PROBE_REPOSITORY: &str = "CODENOESIS_S3_PROBE_REPOSITORY";
const PROBE_RESULT: &str = "CODENOESIS_S3_PROBE_RESULT";
const FAILPOINT_BOUNDARY: &str = "CODENOESIS_S3_FAILPOINT_BOUNDARY";
const FAILPOINT_ROLE: &str = "CODENOESIS_S3_FAILPOINT_ROLE";
const FAILPOINT_ORDINAL: &str = "CODENOESIS_S3_FAILPOINT_ORDINAL";
const FAILPOINT_REVISION: &str = "CODENOESIS_S3_FAILPOINT_REVISION";
const FAILPOINT_SIGNAL: &str = "CODENOESIS_S3_FAILPOINT_SIGNAL";

#[test]
fn e2e_fr_sto_001_atomic_local_storage() {
    let repository = MaterializedRepository::revisions();
    let output_a = scan(&repository.worktree, &repository.store, COMMIT_A_OID);
    assert_snapshot(&output_a, "snapshot-semantic-a.json");
    assert_eq!(
        probe_head(&repository),
        expected_head("expected-head-a.json")
    );

    let output_b = scan(&repository.worktree, &repository.store, COMMIT_B_OID);
    assert_snapshot(&output_b, "snapshot-semantic-b.json");
    assert_eq!(
        probe_head(&repository),
        expected_head("expected-head-b.json")
    );

    let counts_before_retry = immutable_counts(&repository.store);
    let output_retry = scan(&repository.worktree, &repository.store, COMMIT_B_OID);
    assert_snapshot(&output_retry, "snapshot-semantic-b.json");
    assert_eq!(
        probe_head(&repository),
        expected_head("expected-head-b.json")
    );
    assert_eq!(immutable_counts(&repository.store), counts_before_retry);

    assert_eq!(
        fs::read(repository.store.join("store.json")).expect("read durable store marker"),
        b"{\"database\":\"metadata.sqlite3\",\"objects\":\"objects\",\"schema_version\":\"codenoesis.local-store-marker/v1\",\"temporary\":\"tmp\"}\n"
    );
    assert!(
        repository.store.join("metadata.sqlite3").is_file(),
        "successful S3 publication must create the metadata database"
    );
}

#[test]
fn it_fr_sto_001_corruption_fails_closed() {
    let repository = MaterializedRepository::revisions();
    let initial = scan(&repository.worktree, &repository.store, COMMIT_B_OID);
    assert_snapshot(&initial, "snapshot-semantic-b.json");

    let graph_digest = "73fb647037047b3c48bf40745075ea7e7f4531c4b5fdb5cfde8f52b194adf7b7";
    let graph_path = repository
        .store
        .join("objects/blake3")
        .join(&graph_digest[..2])
        .join(&graph_digest[2..]);
    let mut bytes = fs::read(&graph_path).expect("read reviewed graph object");
    assert_eq!(bytes.first(), Some(&b'{'));
    bytes[0] = b'[';
    fs::write(graph_path, bytes).expect("apply reviewed corruption variant");

    let output = scan(&repository.worktree, &repository.store, COMMIT_B_OID);
    assert_failure(
        &output,
        12,
        "expected-error-corrupt-object.json",
        "corrupt object",
    );

    let missing = MaterializedRepository::revisions();
    let initial = scan(&missing.worktree, &missing.store, COMMIT_B_OID);
    assert_snapshot(&initial, "snapshot-semantic-b.json");
    let missing_path = missing
        .store
        .join("objects/blake3")
        .join(&graph_digest[..2])
        .join(&graph_digest[2..]);
    fs::remove_file(&missing_path).expect("remove reviewed graph object");
    let output = scan(&missing.worktree, &missing.store, COMMIT_B_OID);
    assert_eq!(output.status.code(), Some(12));
    assert!(output.stdout.is_empty());
    let error = parse_single_document(&output.stderr);
    assert_eq!(error["schema_version"], "codenoesis.error/v4");
    assert_eq!(error["code"], "storage.missing_object");
    assert_eq!(
        error["context"]["artifact_id"],
        format!("urn:codenoesis:artifact:blake3:{graph_digest}")
    );
    assert!(
        !missing_path.exists(),
        "idempotent retry must not repair missing reachable content"
    );
}

#[test]
fn conf_fr_sto_001_store_v1_and_error_v4() {
    let incompatible = MaterializedRepository::revisions();
    let initial = scan(&incompatible.worktree, &incompatible.store, COMMIT_B_OID);
    assert_snapshot(&initial, "snapshot-semantic-b.json");
    let connection =
        Connection::open(incompatible.store.join("metadata.sqlite3")).expect("open metadata");
    connection
        .execute_batch("PRAGMA user_version = 2;")
        .expect("apply reviewed incompatible-schema variant");
    drop(connection);
    let output = scan(&incompatible.worktree, &incompatible.store, COMMIT_B_OID);
    assert_failure(
        &output,
        12,
        "expected-error-incompatible-schema.json",
        "incompatible schema",
    );

    let unsafe_repository = MaterializedRepository::revisions();
    let unsafe_store = unsafe_repository.worktree.join(".codenoesis-store");
    let output = scan(&unsafe_repository.worktree, &unsafe_store, COMMIT_A_OID);
    assert_failure(
        &output,
        12,
        "expected-error-unsafe-path.json",
        "unsafe store path",
    );
    assert!(
        !unsafe_store.exists(),
        "unsafe store root must not be created"
    );
}

#[test]
fn ft_fr_snp_001_publication_failpoint_matrix() {
    let repository = MaterializedRepository::revisions();
    for target in failpoint_targets() {
        run_failpoint_case(&repository, target, false);
        run_failpoint_case(&repository, target, true);
    }
}

#[test]
fn pt_inv_snp_001_reader_visibility() {
    let repository = MaterializedRepository::revisions();
    let published_a = scan(&repository.worktree, &repository.store, COMMIT_A_OID);
    assert_snapshot(&published_a, "snapshot-semantic-a.json");
    let snapshot_b = build_snapshot(&repository.worktree, COMMIT_B_OID);
    let mut store =
        LocalStore::open(&repository.worktree, &repository.store).expect("open A store");
    let mut observer = VisibilityObserver {
        repository: &repository.worktree,
        store: &repository.store,
        observations: Vec::new(),
    };
    let head = PublicationService::publish(
        &snapshot_b,
        &mut store.artifacts,
        &mut store.metadata,
        &mut observer,
    )
    .expect("publish B under reader schedules");
    let expected_a = expected_head("expected-head-a.json")["snapshot_id"]
        .as_str()
        .expect("A snapshot ID")
        .to_owned();
    let expected_b = expected_head("expected-head-b.json")["snapshot_id"]
        .as_str()
        .expect("B snapshot ID")
        .to_owned();
    assert_eq!(head.snapshot_id.as_str(), expected_b);
    assert!(
        observer
            .observations
            .iter()
            .any(|(boundary, _)| *boundary == PublicationBoundary::SqliteAfterCommit)
    );
    for (boundary, observed) in observer.observations {
        if boundary == PublicationBoundary::SqliteAfterCommit {
            assert_eq!(observed.as_deref(), Some(expected_b.as_str()));
        } else {
            assert_eq!(observed.as_deref(), Some(expected_a.as_str()));
        }
    }
}

#[test]
fn pt_fr_snp_001_idempotent_retry() {
    let repository = MaterializedRepository::revisions();
    let snapshot = build_snapshot(&repository.worktree, COMMIT_B_OID);
    let mut store =
        LocalStore::open(&repository.worktree, &repository.store).expect("open empty store");
    for repetition in 0..100 {
        let head = PublicationService::publish(
            &snapshot,
            &mut store.artifacts,
            &mut store.metadata,
            &mut NoopPublicationObserver,
        )
        .expect("idempotent publication");
        assert_eq!(head.generation, 1, "repetition {repetition}");
    }
    assert_eq!(immutable_counts(&repository.store), (1, 3, 3, 3));
}

#[test]
fn it_fr_sto_001_restart_preserves_head() {
    let repository = MaterializedRepository::revisions();
    let output = scan(&repository.worktree, &repository.store, COMMIT_A_OID);
    assert_snapshot(&output, "snapshot-semantic-a.json");
    assert_eq!(
        probe_head(&repository),
        expected_head("expected-head-a.json")
    );
}

#[test]
fn ft_fr_snp_001_orphan_sweep_preserves_reachable() {
    let repository = MaterializedRepository::revisions();
    assert_snapshot(
        &scan(&repository.worktree, &repository.store, COMMIT_A_OID),
        "snapshot-semantic-a.json",
    );
    assert_snapshot(
        &scan(&repository.worktree, &repository.store, COMMIT_B_OID),
        "snapshot-semantic-b.json",
    );
    let mut store =
        LocalStore::open(&repository.worktree, &repository.store).expect("open B store");
    let orphan = StoredArtifact::new(
        ArtifactRole::ExtractionChunk,
        99,
        b"{\"orphan\":true}".to_vec(),
        SemanticHash::blake3(
            "codenoesis.extraction-chunk.semantic.v1",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ),
    );
    store
        .artifacts
        .stage(&orphan, &mut NoopPublicationObserver)
        .expect("stage reviewed orphan");
    fs::write(repository.store.join("tmp/abandoned"), b"partial")
        .expect("stage abandoned temporary file");
    let result = PublicationService::sweep(&mut store.artifacts, &store.metadata)
        .expect("sweep reviewed orphans");
    assert_eq!(result.temporary_files_removed, 1);
    assert_eq!(result.objects_removed, 1);
    assert!(!store.artifacts.object_path(&orphan.artifact_id).exists());
    assert_eq!(
        probe_head(&repository),
        expected_head("expected-head-b.json")
    );
}

#[test]
fn sec_fr_sto_001_store_root_confinement() {
    let repository = MaterializedRepository::revisions();
    let sentinel = repository.root.join("outside-sentinel");
    fs::write(&sentinel, b"unchanged\n").expect("write outside sentinel");
    let hook = repository.worktree.join(".git/hooks/post-checkout");
    fs::create_dir_all(hook.parent().expect("hook parent")).expect("create hook directory");
    fs::write(
        &hook,
        "#!/bin/sh\nprintf 'executed\\n' > \"$CODENOESIS_S3_SENTINEL\"\n",
    )
    .expect("write target-controlled hook");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;

        fs::set_permissions(&hook, fs::Permissions::from_mode(0o755))
            .expect("make target-controlled hook executable");
    }
    let mut config = fs::OpenOptions::new()
        .append(true)
        .open(repository.worktree.join(".git/config"))
        .expect("open target-controlled Git configuration");
    config
        .write_all(b"\n[remote \"origin\"]\n\turl = https://127.0.0.1:9/sentinel.git\n")
        .expect("write target-controlled remote");
    drop(config);
    let output = scan_command(&repository.worktree, &repository.store, COMMIT_A_OID)
        .env("CODENOESIS_S3_SENTINEL", &sentinel)
        .output()
        .expect("launch confined S3 scan");
    assert_snapshot(&output, "snapshot-semantic-a.json");
    assert_eq!(
        fs::read(&sentinel).expect("read outside sentinel"),
        b"unchanged\n"
    );
    assert!(
        object_files(&repository.store)
            .iter()
            .all(|path| path.starts_with(&repository.store))
    );

    let unmarked = repository.root.join("unmarked");
    fs::create_dir(&unmarked).expect("create unmarked root");
    fs::write(unmarked.join("sentinel"), b"preserve\n").expect("write unmarked sentinel");
    let output = scan(&repository.worktree, &unmarked, COMMIT_A_OID);
    assert_eq!(output.status.code(), Some(12));
    let error = parse_single_document(&output.stderr);
    assert_eq!(error["code"], "storage.unmarked_nonempty_root");
    assert_eq!(
        fs::read(unmarked.join("sentinel")).expect("read preserved unmarked sentinel"),
        b"preserve\n"
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let target = repository.root.join("symlink-target");
        let linked_store = repository.root.join("linked-store");
        fs::create_dir(&target).expect("create symlink target");
        symlink(&target, &linked_store).expect("create store symlink");
        let output = scan(&repository.worktree, &linked_store, COMMIT_A_OID);
        assert_eq!(output.status.code(), Some(12));
        let error = parse_single_document(&output.stderr);
        assert_eq!(error["code"], "storage.unsafe_path");
        assert_eq!(error["context"]["reason"], "unsafe_path_component");
        assert!(
            fs::read_dir(target)
                .expect("read untouched symlink target")
                .next()
                .is_none()
        );
    }
}

#[test]
fn reg_fr_sto_001_legacy_profiles_unchanged() {
    let repository = support::s2::MaterializedRepository::revision_a();
    let output = support::s2::scan(&repository.worktree, support::s2::COMMIT_A_OID);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(
        parse_single_document(&output.stdout)["schema_version"],
        "codenoesis.repository-snapshot/v3"
    );
    assert!(!repository.root.join("store").exists());
}

#[test]
fn s3_publication_probe_process() {
    let Some(boundary) = env::var_os(FAILPOINT_BOUNDARY) else {
        return;
    };
    let repository = env::var_os(PROBE_REPOSITORY).expect("failpoint repository");
    let store = env::var_os(PROBE_STORE).expect("failpoint store");
    let revision = env::var(FAILPOINT_REVISION).expect("failpoint revision");
    let signal = env::var_os(FAILPOINT_SIGNAL).expect("failpoint signal");
    let target = FailpointTarget {
        boundary: parse_boundary(
            boundary
                .to_str()
                .expect("failpoint boundary must be valid UTF-8"),
        ),
        role: env::var(FAILPOINT_ROLE)
            .ok()
            .filter(|value| !value.is_empty())
            .map(|value| ArtifactRole::parse(&value).expect("failpoint artifact role")),
        ordinal: env::var(FAILPOINT_ORDINAL)
            .ok()
            .filter(|value| !value.is_empty())
            .map(|value| value.parse().expect("failpoint artifact ordinal")),
    };
    let snapshot = build_snapshot(Path::new(&repository), &revision);
    let mut local_store = LocalStore::open(Path::new(&repository), Path::new(&store))
        .expect("failpoint opens local store");
    let mut observer = BlockingObserver {
        target,
        signal: PathBuf::from(signal),
    };
    let _head = PublicationService::publish(
        &snapshot,
        &mut local_store.artifacts,
        &mut local_store.metadata,
        &mut observer,
    )
    .expect("target boundary must externally terminate the probe");
    panic!("publication probe completed without reaching its target");
}

#[test]
fn s3_head_probe_process() {
    let Some(store) = env::var_os(PROBE_STORE) else {
        return;
    };
    let repository = env::var_os(PROBE_REPOSITORY).expect("probe repository path");
    let result = env::var_os(PROBE_RESULT).expect("probe result path");
    let local_store = LocalStore::open(Path::new(&repository), Path::new(&store))
        .expect("probe opens exact local store");
    let identity = RepositoryIdentity::parse(REPOSITORY_ID).expect("reviewed repository identity");
    let head =
        PublicationService::load_head(&identity, &local_store.artifacts, &local_store.metadata)
            .expect("probe loads complete head");
    let bytes = head.map_or_else(
        || b"null\n".to_vec(),
        |head| {
            LocalSnapshotHeadV1::from_head(&head)
                .canonical_stdout()
                .expect("serialize probed head")
        },
    );
    fs::write(result, bytes).expect("write isolated probe result");
}

#[derive(Clone, Copy, Debug)]
struct FailpointTarget {
    boundary: PublicationBoundary,
    role: Option<ArtifactRole>,
    ordinal: Option<u32>,
}

impl FailpointTarget {
    fn matches(self, event: &PublicationEvent) -> bool {
        self.boundary == event.boundary && self.role == event.role && self.ordinal == event.ordinal
    }
}

struct BlockingObserver {
    target: FailpointTarget,
    signal: PathBuf,
}

impl PublicationObserver for BlockingObserver {
    fn observe(&mut self, event: &PublicationEvent) -> Result<(), StorageError> {
        if !self.target.matches(event) {
            return Ok(());
        }
        let mut signal = fs::File::create(&self.signal).expect("create failpoint signal");
        signal
            .write_all(event.boundary.as_str().as_bytes())
            .and_then(|()| signal.sync_all())
            .expect("durably emit failpoint signal");
        loop {
            thread::sleep(Duration::from_mins(1));
        }
    }
}

struct VisibilityObserver<'a> {
    repository: &'a Path,
    store: &'a Path,
    observations: Vec<(PublicationBoundary, Option<String>)>,
}

impl PublicationObserver for VisibilityObserver<'_> {
    fn observe(&mut self, event: &PublicationEvent) -> Result<(), StorageError> {
        let local_store =
            LocalStore::open(self.repository, self.store).expect("reader opens stable store");
        let identity =
            RepositoryIdentity::parse(REPOSITORY_ID).expect("reviewed repository identity");
        let head =
            PublicationService::load_head(&identity, &local_store.artifacts, &local_store.metadata)
                .expect("reader loads only a complete head");
        self.observations.push((
            event.boundary,
            head.map(|value| value.snapshot_id.to_string()),
        ));
        Ok(())
    }
}

fn failpoint_targets() -> Vec<FailpointTarget> {
    let mut targets = Vec::new();
    for boundary in [
        PublicationBoundary::CasBeforeTempCreate,
        PublicationBoundary::CasAfterTempSync,
        PublicationBoundary::CasAfterObjectMove,
        PublicationBoundary::CasAfterParentSync,
    ] {
        for role in [
            ArtifactRole::SnapshotSemantic,
            ArtifactRole::KnowledgeGraph,
            ArtifactRole::ExtractionChunk,
        ] {
            targets.push(FailpointTarget {
                boundary,
                role: Some(role),
                ordinal: Some(0),
            });
        }
    }
    for boundary in [
        PublicationBoundary::SqliteAfterBegin,
        PublicationBoundary::SqliteAfterSnapshotRows,
        PublicationBoundary::SqliteAfterHeadUpdate,
        PublicationBoundary::SqliteAfterCommit,
    ] {
        targets.push(FailpointTarget {
            boundary,
            role: None,
            ordinal: None,
        });
    }
    targets
}

fn run_failpoint_case(
    repository: &MaterializedRepository,
    target: FailpointTarget,
    replacement: bool,
) {
    let role = target.role.map_or("metadata", ArtifactRole::as_str);
    let mode = if replacement { "replacement" } else { "first" };
    let store = repository
        .root
        .join(format!("store-{}-{role}-{mode}", target.boundary.as_str()));
    if replacement {
        assert_snapshot(
            &scan(&repository.worktree, &store, COMMIT_A_OID),
            "snapshot-semantic-a.json",
        );
    }
    let revision = if replacement {
        COMMIT_B_OID
    } else {
        COMMIT_A_OID
    };
    terminate_at_boundary(repository, &store, revision, target);

    let restarted = probe_head_at(repository, &store);
    let committed = target.boundary == PublicationBoundary::SqliteAfterCommit;
    let expected_restart = if committed {
        expected_head(if replacement {
            "expected-head-b.json"
        } else {
            "expected-head-a.json"
        })
    } else if replacement {
        expected_head("expected-head-a.json")
    } else {
        Value::Null
    };
    assert_eq!(
        restarted, expected_restart,
        "restart mismatch at {target:?} ({mode})"
    );

    assert_snapshot(
        &scan(&repository.worktree, &store, revision),
        if replacement {
            "snapshot-semantic-b.json"
        } else {
            "snapshot-semantic-a.json"
        },
    );
    let mut local_store =
        LocalStore::open(&repository.worktree, &store).expect("open recovered store");
    PublicationService::sweep(&mut local_store.artifacts, &local_store.metadata)
        .expect("sweep recovered store");
    drop(local_store);
    assert_eq!(
        probe_head_at(repository, &store),
        expected_head(if replacement {
            "expected-head-b.json"
        } else {
            "expected-head-a.json"
        }),
        "retry or sweep mismatch at {target:?} ({mode})"
    );
}

fn terminate_at_boundary(
    repository: &MaterializedRepository,
    store: &Path,
    revision: &str,
    target: FailpointTarget,
) {
    let role = target.role.map_or("", ArtifactRole::as_str);
    let ordinal = target
        .ordinal
        .map_or_else(String::new, |value| value.to_string());
    let signal = repository.root.join(format!(
        "signal-{}-{role}-{revision}",
        target.boundary.as_str()
    ));
    let mut child = Command::new(env::current_exe().expect("current test executable"))
        .args(["--exact", "s3_publication_probe_process", "--nocapture"])
        .env(PROBE_STORE, store)
        .env(PROBE_REPOSITORY, &repository.worktree)
        .env(FAILPOINT_REVISION, revision)
        .env(FAILPOINT_BOUNDARY, target.boundary.as_str())
        .env(FAILPOINT_ROLE, role)
        .env(FAILPOINT_ORDINAL, ordinal)
        .env(FAILPOINT_SIGNAL, &signal)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("launch publication failpoint probe");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !signal.exists() {
        assert!(
            child.try_wait().expect("poll failpoint probe").is_none(),
            "probe exited before {target:?}"
        );
        assert!(
            Instant::now() < deadline,
            "probe timed out before {target:?}"
        );
        thread::sleep(Duration::from_millis(10));
    }
    child
        .kill()
        .expect("externally terminate publication probe");
    let status = child.wait().expect("reap publication probe");
    assert!(
        !status.success(),
        "terminated probe must not report success"
    );
}

fn parse_boundary(value: &str) -> PublicationBoundary {
    PublicationBoundary::ALL
        .into_iter()
        .find(|boundary| boundary.as_str() == value)
        .unwrap_or_else(|| panic!("unknown failpoint boundary {value}"))
}

fn build_snapshot(repository: &Path, revision: &str) -> RepositorySnapshotV3 {
    let request = ScanRequest::new(
        repository.as_os_str().to_os_string(),
        RepositoryIdentity::parse(REPOSITORY_ID).expect("reviewed repository identity"),
        Revision::parse(revision).expect("reviewed revision"),
        SnapshotEnvelopeV1::new(
            "2026-07-26T00:00:00Z".to_owned(),
            None,
            "s3-test-probe".to_owned(),
        ),
    );
    ScanService::new(LocalGitRepository::new())
        .scan_s2(request, &TreeSitterRustExtractor::new())
        .expect("build reviewed S3 snapshot")
}

fn assert_snapshot(output: &Output, semantic_golden: &str) {
    assert_eq!(
        output.status.code(),
        Some(0),
        "subject stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "successful stderr must be empty");
    let snapshot = parse_single_document(&output.stdout);
    assert_eq!(
        snapshot["schema_version"],
        "codenoesis.repository-snapshot/v3"
    );
    assert_eq!(
        snapshot["semantic"],
        read_json(&fixture_root().join(semantic_golden))
    );
}

fn assert_failure(output: &Output, exit_code: i32, golden: &str, scenario: &str) {
    assert_eq!(
        output.status.code(),
        Some(exit_code),
        "{scenario} stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(output.stdout.is_empty(), "{scenario} stdout must be empty");
    assert_eq!(
        parse_single_document(&output.stderr),
        read_json(&fixture_root().join(golden))
    );
}

fn probe_head(repository: &MaterializedRepository) -> Value {
    probe_head_at(repository, &repository.store)
}

fn probe_head_at(repository: &MaterializedRepository, store: &Path) -> Value {
    let suffix = store
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("store");
    let result = repository.root.join(format!("head-probe-{suffix}.json"));
    let output = Command::new(env::current_exe().expect("current test executable"))
        .args(["--exact", "s3_head_probe_process", "--nocapture"])
        .env(PROBE_STORE, store)
        .env(PROBE_REPOSITORY, &repository.worktree)
        .env(PROBE_RESULT, &result)
        .output()
        .expect("launch isolated head probe");
    assert!(
        output.status.success(),
        "head probe failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    parse_single_document(&fs::read(result).expect("read isolated head result"))
}

fn expected_head(name: &str) -> Value {
    read_json(&fixture_root().join(name))
}

fn immutable_counts(store: &Path) -> (i64, i64, i64, usize) {
    let connection = Connection::open(store.join("metadata.sqlite3")).expect("open metadata");
    let snapshots = count_rows(&connection, "snapshots");
    let artifacts = count_rows(&connection, "artifacts");
    let references = count_rows(&connection, "snapshot_artifacts");
    let objects = fs::read_dir(store.join("objects/blake3"))
        .expect("read object root")
        .map(|entry| {
            fs::read_dir(entry.expect("read shard entry").path())
                .expect("read object shard")
                .count()
        })
        .sum();
    (snapshots, artifacts, references, objects)
}

fn object_files(store: &Path) -> Vec<PathBuf> {
    fs::read_dir(store.join("objects/blake3"))
        .expect("read object root")
        .flat_map(|entry| {
            fs::read_dir(entry.expect("read shard entry").path())
                .expect("read object shard")
                .map(|object| object.expect("read object entry").path())
        })
        .collect()
}

fn count_rows(connection: &Connection, table: &str) -> i64 {
    let query = match table {
        "snapshots" => "SELECT COUNT(*) FROM snapshots",
        "artifacts" => "SELECT COUNT(*) FROM artifacts",
        "snapshot_artifacts" => "SELECT COUNT(*) FROM snapshot_artifacts",
        _ => panic!("unsupported reviewed table"),
    };
    connection
        .query_row(query, [], |row| row.get(0))
        .expect("count immutable rows")
}
