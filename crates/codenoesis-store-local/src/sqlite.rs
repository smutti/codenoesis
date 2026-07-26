use std::collections::BTreeSet;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::Duration;

use codenoesis_domain::storage::{
    ArtifactId, ArtifactReference, ArtifactRole, FILESYSTEM_CAS_VERSION, GRAPH_HASH_DOMAIN,
    LOCAL_STORE_SCHEMA_VERSION, LocalSnapshotHead, PublicationBoundary, PublicationCandidate,
    PublicationEvent, PublicationResult, SNAPSHOT_HASH_DOMAIN, SNAPSHOT_SCHEMA_VERSION,
    SemanticHash, SnapshotId, StorageComponent, StorageError,
};
use codenoesis_domain::{ObjectId, RepositoryIdentity};
use codenoesis_ports::{MetadataStore, PublicationObserver};
use rusqlite::{
    Connection, Error as SqliteError, ErrorCode, OpenFlags, OptionalExtension, Transaction,
    TransactionBehavior, params,
};

use crate::path::sync_directory;

const DDL: &str = include_str!("../../../tests/specifications/s3/local-store-v1.sql");
const APPLICATION_ID: i64 = 1_129_205_587;
const USER_VERSION: i64 = 1;
const OPEN_EXISTING: OpenFlags =
    OpenFlags::SQLITE_OPEN_READ_WRITE.union(OpenFlags::SQLITE_OPEN_NO_MUTEX);
const OPEN_FRESH: OpenFlags = OPEN_EXISTING.union(OpenFlags::SQLITE_OPEN_CREATE);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SqliteEvidence {
    pub sqlite_version: String,
    pub compile_options: Vec<String>,
    pub journal_mode: String,
    pub synchronous: i64,
    pub foreign_keys: bool,
    pub trusted_schema: bool,
    pub busy_timeout_milliseconds: i64,
}

pub struct SqliteMetadataStore {
    database: PathBuf,
}

impl SqliteMetadataStore {
    /// Opens an exact v1 database or creates it from the protected DDL.
    ///
    /// # Errors
    ///
    /// Returns a typed schema, integrity, contention, or durability failure.
    pub fn open(database: &Path, fresh: bool) -> Result<Self, StorageError> {
        if fresh {
            initialize(database)?;
        } else {
            let connection = open_existing(database)?;
            validate_database(&connection)?;
        }
        Ok(Self {
            database: database.to_path_buf(),
        })
    }

    #[must_use]
    pub fn database_path(&self) -> &Path {
        &self.database
    }

    /// Captures deterministic `SQLite` runtime evidence without mutating data.
    ///
    /// # Errors
    ///
    /// Returns a typed `SQLite` or schema failure.
    pub fn evidence(&self) -> Result<SqliteEvidence, StorageError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare("PRAGMA compile_options")
            .map_err(map_sqlite)?;
        let options = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(map_sqlite)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_sqlite)?;
        Ok(SqliteEvidence {
            sqlite_version: connection
                .query_row("SELECT sqlite_version()", [], |row| row.get(0))
                .map_err(map_sqlite)?,
            compile_options: options,
            journal_mode: pragma_string(&connection, "journal_mode")?,
            synchronous: pragma_i64(&connection, "synchronous")?,
            foreign_keys: pragma_i64(&connection, "foreign_keys")? == 1,
            trusted_schema: pragma_i64(&connection, "trusted_schema")? == 1,
            busy_timeout_milliseconds: pragma_i64(&connection, "busy_timeout")?,
        })
    }

    fn connection(&self) -> Result<Connection, StorageError> {
        let connection = open_existing(&self.database)?;
        validate_database(&connection)?;
        Ok(connection)
    }
}

impl MetadataStore for SqliteMetadataStore {
    fn current_head_id(
        &self,
        repository_identity: &RepositoryIdentity,
    ) -> Result<Option<SnapshotId>, StorageError> {
        let connection = self.connection()?;
        current_head(&connection, repository_identity).map(|head| head.map(|value| value.0))
    }

    fn publish(
        &mut self,
        candidate: &PublicationCandidate,
        expected_head: Option<&SnapshotId>,
        observer: &mut dyn PublicationObserver,
    ) -> Result<PublicationResult, StorageError> {
        candidate.validate()?;
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite)?;
        observer.observe(&PublicationEvent::sqlite(
            PublicationBoundary::SqliteAfterBegin,
        ))?;
        insert_candidate(&transaction, candidate)?;
        observer.observe(&PublicationEvent::sqlite(
            PublicationBoundary::SqliteAfterSnapshotRows,
        ))?;
        let (generation, changed) = compare_and_set_head(&transaction, candidate, expected_head)?;
        observer.observe(&PublicationEvent::sqlite(
            PublicationBoundary::SqliteAfterHeadUpdate,
        ))?;
        let head = head_from_candidate(candidate, generation);
        transaction.commit().map_err(map_sqlite)?;
        observer.observe(&PublicationEvent::sqlite(
            PublicationBoundary::SqliteAfterCommit,
        ))?;
        Ok(PublicationResult { head, changed })
    }

    fn load_head(
        &self,
        repository_identity: &RepositoryIdentity,
    ) -> Result<Option<LocalSnapshotHead>, StorageError> {
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(map_sqlite)?;
        let Some((snapshot_id, generation)) = current_head(&transaction, repository_identity)?
        else {
            transaction.commit().map_err(map_sqlite)?;
            return Ok(None);
        };
        validate_foreign_keys(&transaction)?;
        let mut head =
            load_snapshot_head(&transaction, repository_identity, &snapshot_id, generation)?;
        head.artifacts = load_artifact_references(&transaction, &snapshot_id)?;
        validate_artifact_references(&head)?;
        validate_graph_rows(&transaction, &head)?;
        transaction.commit().map_err(map_sqlite)?;
        Ok(Some(head))
    }

    fn referenced_artifacts(&self) -> Result<BTreeSet<ArtifactId>, StorageError> {
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(map_sqlite)?;
        let mut statement = transaction
            .prepare("SELECT DISTINCT artifact_id FROM snapshot_artifacts ORDER BY artifact_id")
            .map_err(map_sqlite)?;
        let values = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(map_sqlite)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_sqlite)?;
        drop(statement);
        let artifacts = values
            .into_iter()
            .map(|value| {
                ArtifactId::parse(&value).ok_or_else(|| corrupt("invalid_artifact_id", None))
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        transaction.commit().map_err(map_sqlite)?;
        Ok(artifacts)
    }

    fn is_artifact_referenced(&self, artifact_id: &ArtifactId) -> Result<bool, StorageError> {
        let connection = self.connection()?;
        connection
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM snapshot_artifacts WHERE artifact_id = ?1
                )",
                [artifact_id.as_str()],
                |row| row.get::<_, bool>(0),
            )
            .map_err(map_sqlite)
    }
}

fn initialize(database: &Path) -> Result<(), StorageError> {
    if database.exists() {
        return Err(StorageError::PublicationFailed);
    }
    let connection = Connection::open_with_flags(database, OPEN_FRESH).map_err(map_sqlite)?;
    configure_fresh(&connection)?;
    connection.execute_batch(DDL).map_err(map_sqlite)?;
    connection
        .execute(
            "INSERT INTO store_metadata (key, value) VALUES
                ('schema_version', ?1),
                ('cas_layout_version', ?2)",
            params![LOCAL_STORE_SCHEMA_VERSION, FILESYSTEM_CAS_VERSION],
        )
        .map_err(map_sqlite)?;
    validate_database(&connection)?;
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .map_err(map_sqlite)?;
    drop(connection);
    File::open(database)
        .and_then(|file| file.sync_all())
        .map_err(|_| StorageError::PublicationFailed)?;
    sync_directory(database.parent().ok_or(StorageError::PublicationFailed)?)
}

fn open_existing(database: &Path) -> Result<Connection, StorageError> {
    let connection = Connection::open_with_flags(database, OPEN_EXISTING).map_err(map_sqlite)?;
    validate_header(&connection)?;
    configure_existing(&connection)?;
    Ok(connection)
}

fn configure_fresh(connection: &Connection) -> Result<(), StorageError> {
    connection
        .busy_timeout(Duration::ZERO)
        .map_err(map_sqlite)?;
    connection
        .execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = FULL;
             PRAGMA foreign_keys = ON;
             PRAGMA trusted_schema = OFF;
             PRAGMA busy_timeout = 0;",
        )
        .map_err(map_sqlite)
}

fn configure_existing(connection: &Connection) -> Result<(), StorageError> {
    if !pragma_string(connection, "journal_mode")?.eq_ignore_ascii_case("wal") {
        return Err(corrupt("journal_mode_drift", None));
    }
    connection
        .busy_timeout(Duration::ZERO)
        .map_err(map_sqlite)?;
    connection
        .execute_batch(
            "PRAGMA synchronous = FULL;
             PRAGMA foreign_keys = ON;
             PRAGMA trusted_schema = OFF;
             PRAGMA busy_timeout = 0;",
        )
        .map_err(map_sqlite)
}

fn validate_header(connection: &Connection) -> Result<(), StorageError> {
    let application_id = pragma_i64(connection, "application_id")?;
    let user_version = pragma_i64(connection, "user_version")?;
    if application_id != APPLICATION_ID || user_version != USER_VERSION {
        let observed_schema = if user_version == USER_VERSION {
            "codenoesis.local-store/foreign".to_owned()
        } else {
            format!("codenoesis.local-store/v{user_version}")
        };
        return Err(StorageError::IncompatibleSchema { observed_schema });
    }
    Ok(())
}

fn validate_database(connection: &Connection) -> Result<(), StorageError> {
    validate_header(connection)?;
    if pragma_i64(connection, "foreign_keys")? != 1
        || pragma_i64(connection, "synchronous")? != 2
        || pragma_i64(connection, "trusted_schema")? != 0
        || pragma_i64(connection, "busy_timeout")? != 0
    {
        return Err(corrupt("connection_pragma_drift", None));
    }
    validate_schema_definition(connection)?;
    let schema_version = connection
        .query_row(
            "SELECT value FROM store_metadata WHERE key = 'schema_version'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(map_sqlite)?;
    if schema_version.as_deref() != Some(LOCAL_STORE_SCHEMA_VERSION) {
        return Err(StorageError::IncompatibleSchema {
            observed_schema: schema_version
                .unwrap_or_else(|| "codenoesis.local-store/missing".to_owned()),
        });
    }
    let layout_version = connection
        .query_row(
            "SELECT value FROM store_metadata WHERE key = 'cas_layout_version'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(map_sqlite)?;
    if layout_version.as_deref() != Some(FILESYSTEM_CAS_VERSION) {
        return Err(corrupt("cas_layout_version_drift", None));
    }
    let quick_check = connection
        .query_row("PRAGMA quick_check", [], |row| row.get::<_, String>(0))
        .map_err(map_sqlite)?;
    if quick_check != "ok" {
        return Err(corrupt("sqlite_integrity_check_failed", None));
    }
    validate_foreign_keys(connection)
}

fn validate_schema_definition(connection: &Connection) -> Result<(), StorageError> {
    let expected = Connection::open_in_memory().map_err(map_sqlite)?;
    configure_fresh(&expected)?;
    expected.execute_batch(DDL).map_err(map_sqlite)?;
    if schema_definition(connection)? != schema_definition(&expected)? {
        return Err(corrupt("schema_drift", None));
    }
    Ok(())
}

fn schema_definition(
    connection: &Connection,
) -> Result<Vec<(String, String, String, String)>, StorageError> {
    let mut statement = connection
        .prepare(
            "SELECT type, name, tbl_name, sql
             FROM sqlite_schema
             WHERE sql IS NOT NULL
             ORDER BY type, name",
        )
        .map_err(map_sqlite)?;
    statement
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .map_err(map_sqlite)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_sqlite)
}

fn insert_candidate(
    transaction: &Transaction<'_>,
    candidate: &PublicationCandidate,
) -> Result<(), StorageError> {
    insert_snapshot(transaction, candidate)?;
    for artifact in &candidate.artifacts {
        insert_artifact(transaction, artifact)?;
        insert_artifact_reference(transaction, &candidate.snapshot.snapshot_id, artifact)?;
    }
    for row in &candidate.extraction_chunks {
        insert_extraction(transaction, &candidate.snapshot.snapshot_id, row)?;
    }
    insert_graph_rows(transaction, candidate)
}

fn insert_snapshot(
    transaction: &Transaction<'_>,
    candidate: &PublicationCandidate,
) -> Result<(), StorageError> {
    let snapshot = &candidate.snapshot;
    let values = params![
        snapshot.snapshot_id.as_str(),
        snapshot.repository_identity.as_str(),
        snapshot.commit_oid.as_str(),
        snapshot.snapshot_schema_version,
        snapshot.semantic_hash.algorithm,
        snapshot.semantic_hash.domain,
        snapshot.semantic_hash.value,
        snapshot.graph_semantic_hash.value,
    ];
    transaction
        .execute(
            "INSERT OR IGNORE INTO snapshots (
                snapshot_id, repository_identity, commit_oid,
                snapshot_schema_version, semantic_hash_algorithm,
                semantic_hash_domain, semantic_hash_value,
                graph_semantic_hash_value
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            values,
        )
        .map_err(map_sqlite)?;
    let exact = transaction
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM snapshots
                WHERE snapshot_id = ?1 AND repository_identity = ?2
                  AND commit_oid = ?3 AND snapshot_schema_version = ?4
                  AND semantic_hash_algorithm = ?5
                  AND semantic_hash_domain = ?6
                  AND semantic_hash_value = ?7
                  AND graph_semantic_hash_value = ?8
            )",
            params![
                snapshot.snapshot_id.as_str(),
                snapshot.repository_identity.as_str(),
                snapshot.commit_oid.as_str(),
                snapshot.snapshot_schema_version,
                snapshot.semantic_hash.algorithm,
                snapshot.semantic_hash.domain,
                snapshot.semantic_hash.value,
                snapshot.graph_semantic_hash.value,
            ],
            |row| row.get::<_, bool>(0),
        )
        .map_err(map_sqlite)?;
    exact.then_some(()).ok_or_else(|| {
        corrupt(
            "immutable_snapshot_mismatch",
            Some(snapshot.snapshot_id.to_string()),
        )
    })
}

fn insert_artifact(
    transaction: &Transaction<'_>,
    artifact: &codenoesis_domain::storage::StoredArtifact,
) -> Result<(), StorageError> {
    let byte_length = to_i64(artifact.byte_length())?;
    transaction
        .execute(
            "INSERT OR IGNORE INTO artifacts (
                artifact_id, digest, byte_length, layout_version
             ) VALUES (?1, ?2, ?3, ?4)",
            params![
                artifact.artifact_id.as_str(),
                artifact.artifact_id.digest(),
                byte_length,
                FILESYSTEM_CAS_VERSION,
            ],
        )
        .map_err(map_sqlite)?;
    let exact = transaction
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM artifacts
                WHERE artifact_id = ?1 AND digest = ?2
                  AND byte_length = ?3 AND layout_version = ?4
            )",
            params![
                artifact.artifact_id.as_str(),
                artifact.artifact_id.digest(),
                byte_length,
                FILESYSTEM_CAS_VERSION,
            ],
            |row| row.get::<_, bool>(0),
        )
        .map_err(map_sqlite)?;
    exact
        .then_some(())
        .ok_or_else(|| corrupt("immutable_artifact_mismatch", None))
}

fn insert_artifact_reference(
    transaction: &Transaction<'_>,
    snapshot_id: &SnapshotId,
    artifact: &codenoesis_domain::storage::StoredArtifact,
) -> Result<(), StorageError> {
    let ordinal = i64::from(artifact.ordinal);
    transaction
        .execute(
            "INSERT OR IGNORE INTO snapshot_artifacts (
                snapshot_id, role, ordinal, artifact_id,
                semantic_hash_algorithm, semantic_hash_domain,
                semantic_hash_value
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                snapshot_id.as_str(),
                artifact.role.as_str(),
                ordinal,
                artifact.artifact_id.as_str(),
                artifact.semantic_hash.algorithm,
                artifact.semantic_hash.domain,
                artifact.semantic_hash.value,
            ],
        )
        .map_err(map_sqlite)?;
    let exact = transaction
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM snapshot_artifacts
                WHERE snapshot_id = ?1 AND role = ?2 AND ordinal = ?3
                  AND artifact_id = ?4 AND semantic_hash_algorithm = ?5
                  AND semantic_hash_domain = ?6 AND semantic_hash_value = ?7
            )",
            params![
                snapshot_id.as_str(),
                artifact.role.as_str(),
                ordinal,
                artifact.artifact_id.as_str(),
                artifact.semantic_hash.algorithm,
                artifact.semantic_hash.domain,
                artifact.semantic_hash.value,
            ],
            |row| row.get::<_, bool>(0),
        )
        .map_err(map_sqlite)?;
    exact
        .then_some(())
        .ok_or_else(|| corrupt("immutable_artifact_reference_mismatch", None))
}

fn insert_extraction(
    transaction: &Transaction<'_>,
    snapshot_id: &SnapshotId,
    row: &codenoesis_domain::storage::ExtractionRow,
) -> Result<(), StorageError> {
    transaction
        .execute(
            "INSERT OR IGNORE INTO extraction_chunks (
                snapshot_id, ordinal, chunk_id, artifact_id, canonical_json
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                snapshot_id.as_str(),
                i64::from(row.ordinal),
                row.chunk_id,
                row.artifact_id.as_str(),
                row.canonical_json,
            ],
        )
        .map_err(map_sqlite)?;
    let exact = transaction
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM extraction_chunks
                WHERE snapshot_id = ?1 AND ordinal = ?2 AND chunk_id = ?3
                  AND artifact_id = ?4 AND canonical_json = ?5
            )",
            params![
                snapshot_id.as_str(),
                i64::from(row.ordinal),
                row.chunk_id,
                row.artifact_id.as_str(),
                row.canonical_json,
            ],
            |result| result.get::<_, bool>(0),
        )
        .map_err(map_sqlite)?;
    exact
        .then_some(())
        .ok_or_else(|| corrupt("immutable_extraction_mismatch", None))
}

fn insert_graph_rows(
    transaction: &Transaction<'_>,
    candidate: &PublicationCandidate,
) -> Result<(), StorageError> {
    let snapshot_id = &candidate.snapshot.snapshot_id;
    for row in &candidate.entities {
        insert_entity(transaction, snapshot_id, row)?;
    }
    for row in &candidate.relationships {
        insert_relationship(transaction, snapshot_id, row)?;
    }
    for row in &candidate.claims {
        insert_claim(transaction, snapshot_id, row)?;
    }
    for row in &candidate.evidence {
        insert_evidence(transaction, snapshot_id, row)?;
    }
    for row in &candidate.diagnostics {
        insert_ordinal_row(transaction, "diagnostics", snapshot_id, row)?;
    }
    for row in &candidate.coverage_gaps {
        insert_ordinal_row(transaction, "coverage_gaps", snapshot_id, row)?;
    }
    Ok(())
}

fn insert_entity(
    transaction: &Transaction<'_>,
    snapshot_id: &SnapshotId,
    row: &codenoesis_domain::storage::EntityRow,
) -> Result<(), StorageError> {
    transaction
        .execute(
            "INSERT OR IGNORE INTO entities (
                snapshot_id, entity_id, canonical_json
             ) VALUES (?1, ?2, ?3)",
            params![snapshot_id.as_str(), row.entity_id, row.canonical_json],
        )
        .map_err(map_sqlite)?;
    verify_blob_row(
        transaction,
        "SELECT EXISTS(
            SELECT 1 FROM entities
            WHERE snapshot_id = ?1 AND entity_id = ?2 AND canonical_json = ?3
         )",
        snapshot_id,
        &row.entity_id,
        &row.canonical_json,
        "immutable_entity_mismatch",
    )
}

fn insert_relationship(
    transaction: &Transaction<'_>,
    snapshot_id: &SnapshotId,
    row: &codenoesis_domain::storage::RelationshipRow,
) -> Result<(), StorageError> {
    transaction
        .execute(
            "INSERT OR IGNORE INTO relationships (
                snapshot_id, relationship_id, source_entity_id,
                target_entity_id, canonical_json
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                snapshot_id.as_str(),
                row.relationship_id,
                row.source_entity_id,
                row.target_entity_id,
                row.canonical_json,
            ],
        )
        .map_err(map_sqlite)?;
    let exact = transaction
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM relationships
                WHERE snapshot_id = ?1 AND relationship_id = ?2
                  AND source_entity_id = ?3 AND target_entity_id = ?4
                  AND canonical_json = ?5
            )",
            params![
                snapshot_id.as_str(),
                row.relationship_id,
                row.source_entity_id,
                row.target_entity_id,
                row.canonical_json,
            ],
            |result| result.get::<_, bool>(0),
        )
        .map_err(map_sqlite)?;
    exact
        .then_some(())
        .ok_or_else(|| corrupt("immutable_relationship_mismatch", None))
}

fn insert_claim(
    transaction: &Transaction<'_>,
    snapshot_id: &SnapshotId,
    row: &codenoesis_domain::storage::ClaimRow,
) -> Result<(), StorageError> {
    transaction
        .execute(
            "INSERT OR IGNORE INTO claims (
                snapshot_id, claim_id, subject_kind, subject_id, canonical_json
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                snapshot_id.as_str(),
                row.claim_id,
                row.subject_kind,
                row.subject_id,
                row.canonical_json,
            ],
        )
        .map_err(map_sqlite)?;
    let exact = transaction
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM claims
                WHERE snapshot_id = ?1 AND claim_id = ?2
                  AND subject_kind = ?3 AND subject_id = ?4
                  AND canonical_json = ?5
            )",
            params![
                snapshot_id.as_str(),
                row.claim_id,
                row.subject_kind,
                row.subject_id,
                row.canonical_json,
            ],
            |result| result.get::<_, bool>(0),
        )
        .map_err(map_sqlite)?;
    exact
        .then_some(())
        .ok_or_else(|| corrupt("immutable_claim_mismatch", None))
}

fn insert_evidence(
    transaction: &Transaction<'_>,
    snapshot_id: &SnapshotId,
    row: &codenoesis_domain::storage::EvidenceRow,
) -> Result<(), StorageError> {
    transaction
        .execute(
            "INSERT OR IGNORE INTO evidence (
                snapshot_id, evidence_id, canonical_json
             ) VALUES (?1, ?2, ?3)",
            params![snapshot_id.as_str(), row.evidence_id, row.canonical_json],
        )
        .map_err(map_sqlite)?;
    verify_blob_row(
        transaction,
        "SELECT EXISTS(
            SELECT 1 FROM evidence
            WHERE snapshot_id = ?1 AND evidence_id = ?2 AND canonical_json = ?3
         )",
        snapshot_id,
        &row.evidence_id,
        &row.canonical_json,
        "immutable_evidence_mismatch",
    )
}

fn insert_ordinal_row(
    transaction: &Transaction<'_>,
    table: &'static str,
    snapshot_id: &SnapshotId,
    row: &codenoesis_domain::storage::OrdinalRow,
) -> Result<(), StorageError> {
    let (insert, verify, reason) = match table {
        "diagnostics" => (
            "INSERT OR IGNORE INTO diagnostics (
                snapshot_id, ordinal, canonical_json
             ) VALUES (?1, ?2, ?3)",
            "SELECT EXISTS(
                SELECT 1 FROM diagnostics
                WHERE snapshot_id = ?1 AND ordinal = ?2 AND canonical_json = ?3
             )",
            "immutable_diagnostic_mismatch",
        ),
        "coverage_gaps" => (
            "INSERT OR IGNORE INTO coverage_gaps (
                snapshot_id, ordinal, canonical_json
             ) VALUES (?1, ?2, ?3)",
            "SELECT EXISTS(
                SELECT 1 FROM coverage_gaps
                WHERE snapshot_id = ?1 AND ordinal = ?2 AND canonical_json = ?3
             )",
            "immutable_coverage_gap_mismatch",
        ),
        _ => return Err(StorageError::PublicationFailed),
    };
    let ordinal = i64::from(row.ordinal);
    transaction
        .execute(
            insert,
            params![snapshot_id.as_str(), ordinal, row.canonical_json],
        )
        .map_err(map_sqlite)?;
    let exact = transaction
        .query_row(
            verify,
            params![snapshot_id.as_str(), ordinal, row.canonical_json],
            |result| result.get::<_, bool>(0),
        )
        .map_err(map_sqlite)?;
    exact.then_some(()).ok_or_else(|| corrupt(reason, None))
}

fn verify_blob_row(
    transaction: &Transaction<'_>,
    query: &str,
    snapshot_id: &SnapshotId,
    identifier: &str,
    canonical_json: &[u8],
    reason: &'static str,
) -> Result<(), StorageError> {
    let exact = transaction
        .query_row(
            query,
            params![snapshot_id.as_str(), identifier, canonical_json],
            |row| row.get::<_, bool>(0),
        )
        .map_err(map_sqlite)?;
    exact.then_some(()).ok_or_else(|| corrupt(reason, None))
}

fn compare_and_set_head(
    transaction: &Transaction<'_>,
    candidate: &PublicationCandidate,
    expected_head: Option<&SnapshotId>,
) -> Result<(u64, bool), StorageError> {
    let repository = &candidate.snapshot.repository_identity;
    let current = current_head(transaction, repository)?;
    if current.as_ref().map(|value| &value.0) != expected_head {
        return Err(StorageError::HeadConflict {
            expected: expected_head.map(ToString::to_string),
            actual: current.as_ref().map(|value| value.0.to_string()),
        });
    }
    match current {
        Some((snapshot_id, generation)) if snapshot_id == candidate.snapshot.snapshot_id => {
            Ok((generation, false))
        }
        Some((snapshot_id, generation)) => {
            let next = generation.checked_add(1).ok_or_else(|| {
                corrupt("head_generation_overflow", Some(snapshot_id.to_string()))
            })?;
            let changed = transaction
                .execute(
                    "UPDATE project_heads
                     SET snapshot_id = ?1, generation = ?2
                     WHERE repository_identity = ?3 AND snapshot_id = ?4
                       AND generation = ?5",
                    params![
                        candidate.snapshot.snapshot_id.as_str(),
                        to_i64(next)?,
                        repository.as_str(),
                        snapshot_id.as_str(),
                        to_i64(generation)?,
                    ],
                )
                .map_err(map_sqlite)?;
            if changed != 1 {
                return Err(StorageError::HeadConflict {
                    expected: Some(snapshot_id.to_string()),
                    actual: current_head(transaction, repository)?.map(|value| value.0.to_string()),
                });
            }
            Ok((next, true))
        }
        None => {
            transaction
                .execute(
                    "INSERT INTO project_heads (
                        repository_identity, snapshot_id, generation
                     ) VALUES (?1, ?2, 1)",
                    params![repository.as_str(), candidate.snapshot.snapshot_id.as_str()],
                )
                .map_err(map_sqlite)?;
            Ok((1, true))
        }
    }
}

fn current_head(
    connection: &Connection,
    repository_identity: &RepositoryIdentity,
) -> Result<Option<(SnapshotId, u64)>, StorageError> {
    let value = connection
        .query_row(
            "SELECT snapshot_id, generation
             FROM project_heads
             WHERE repository_identity = ?1",
            [repository_identity.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(map_sqlite)?;
    value
        .map(|(snapshot_id, generation)| {
            let snapshot_id = SnapshotId::parse(&snapshot_id)
                .ok_or_else(|| corrupt("invalid_head_snapshot_id", None))?;
            let generation = u64::try_from(generation)
                .ok()
                .filter(|value| *value >= 1)
                .ok_or_else(|| corrupt("invalid_head_generation", Some(snapshot_id.to_string())))?;
            Ok((snapshot_id, generation))
        })
        .transpose()
}

fn head_from_candidate(candidate: &PublicationCandidate, generation: u64) -> LocalSnapshotHead {
    LocalSnapshotHead {
        repository_identity: candidate.snapshot.repository_identity.clone(),
        snapshot_id: candidate.snapshot.snapshot_id.clone(),
        commit_oid: candidate.snapshot.commit_oid.clone(),
        snapshot_schema_version: candidate.snapshot.snapshot_schema_version.clone(),
        semantic_hash: candidate.snapshot.semantic_hash.clone(),
        graph_semantic_hash: candidate.snapshot.graph_semantic_hash.clone(),
        generation,
        artifacts: candidate.artifact_references(),
    }
}

fn load_snapshot_head(
    connection: &Connection,
    repository_identity: &RepositoryIdentity,
    snapshot_id: &SnapshotId,
    generation: u64,
) -> Result<LocalSnapshotHead, StorageError> {
    let row = connection
        .query_row(
            "SELECT repository_identity, commit_oid, snapshot_schema_version,
                    semantic_hash_algorithm, semantic_hash_domain,
                    semantic_hash_value, graph_semantic_hash_value
             FROM snapshots
             WHERE snapshot_id = ?1",
            [snapshot_id.as_str()],
            |result| {
                Ok((
                    result.get::<_, String>(0)?,
                    result.get::<_, String>(1)?,
                    result.get::<_, String>(2)?,
                    result.get::<_, String>(3)?,
                    result.get::<_, String>(4)?,
                    result.get::<_, String>(5)?,
                    result.get::<_, String>(6)?,
                ))
            },
        )
        .optional()
        .map_err(map_sqlite)?
        .ok_or_else(|| corrupt("head_snapshot_missing", Some(snapshot_id.to_string())))?;
    if row.0 != repository_identity.as_str()
        || row.2 != SNAPSHOT_SCHEMA_VERSION
        || row.3 != "blake3-256"
        || row.4 != SNAPSHOT_HASH_DOMAIN
        || !is_blake3_hex(&row.5)
        || !is_blake3_hex(&row.6)
        || SnapshotId::from_semantic_hash(&row.5)? != *snapshot_id
    {
        return Err(corrupt(
            "invalid_snapshot_metadata",
            Some(snapshot_id.to_string()),
        ));
    }
    let commit_oid = ObjectId::parse_sha1(&row.1)
        .ok_or_else(|| corrupt("invalid_commit_oid", Some(snapshot_id.to_string())))?;
    Ok(LocalSnapshotHead {
        repository_identity: repository_identity.clone(),
        snapshot_id: snapshot_id.clone(),
        commit_oid,
        snapshot_schema_version: row.2,
        semantic_hash: SemanticHash::blake3(SNAPSHOT_HASH_DOMAIN, &row.5),
        graph_semantic_hash: SemanticHash::blake3(GRAPH_HASH_DOMAIN, &row.6),
        generation,
        artifacts: Vec::new(),
    })
}

fn load_artifact_references(
    connection: &Connection,
    snapshot_id: &SnapshotId,
) -> Result<Vec<ArtifactReference>, StorageError> {
    let mut statement = connection
        .prepare(
            "SELECT sa.role, sa.ordinal, sa.artifact_id, a.byte_length,
                    sa.semantic_hash_algorithm, sa.semantic_hash_domain,
                    sa.semantic_hash_value
             FROM snapshot_artifacts AS sa
             JOIN artifacts AS a ON a.artifact_id = sa.artifact_id
             WHERE sa.snapshot_id = ?1
             ORDER BY CASE sa.role
                 WHEN 'snapshot_semantic' THEN 0
                 WHEN 'knowledge_graph' THEN 1
                 WHEN 'extraction_chunk' THEN 2
                 ELSE 3 END,
                 sa.ordinal",
        )
        .map_err(map_sqlite)?;
    let raw = statement
        .query_map([snapshot_id.as_str()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
            ))
        })
        .map_err(map_sqlite)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_sqlite)?;
    raw.into_iter()
        .map(|row| {
            let role = ArtifactRole::parse(&row.0)
                .ok_or_else(|| corrupt("invalid_artifact_role", Some(snapshot_id.to_string())))?;
            let ordinal = u32::try_from(row.1)
                .map_err(|_| corrupt("invalid_artifact_ordinal", Some(snapshot_id.to_string())))?;
            let artifact_id = ArtifactId::parse(&row.2)
                .ok_or_else(|| corrupt("invalid_artifact_id", Some(snapshot_id.to_string())))?;
            let byte_length = u64::try_from(row.3)
                .ok()
                .filter(|value| *value >= 2)
                .ok_or_else(|| corrupt("invalid_artifact_length", Some(snapshot_id.to_string())))?;
            if row.4 != "blake3-256" || !is_blake3_hex(&row.6) {
                return Err(corrupt(
                    "invalid_artifact_semantic_hash",
                    Some(snapshot_id.to_string()),
                ));
            }
            Ok(ArtifactReference {
                role,
                ordinal,
                artifact_id,
                byte_length,
                semantic_hash: SemanticHash {
                    algorithm: row.4,
                    domain: row.5,
                    value: row.6,
                },
            })
        })
        .collect()
}

fn validate_artifact_references(head: &LocalSnapshotHead) -> Result<(), StorageError> {
    let mut extraction_ordinal = 0_u32;
    for (position, reference) in head.artifacts.iter().enumerate() {
        let valid = match (position, reference.role, reference.ordinal) {
            (0, ArtifactRole::SnapshotSemantic, 0) => reference.semantic_hash == head.semantic_hash,
            (1, ArtifactRole::KnowledgeGraph, 0) => {
                reference.semantic_hash == head.graph_semantic_hash
            }
            (_, ArtifactRole::ExtractionChunk, ordinal) if ordinal == extraction_ordinal => {
                extraction_ordinal = extraction_ordinal.saturating_add(1);
                reference.semantic_hash.algorithm == "blake3-256"
                    && reference.semantic_hash.domain
                        == codenoesis_domain::storage::EXTRACTION_HASH_DOMAIN
            }
            _ => false,
        };
        if !valid {
            return Err(corrupt(
                "invalid_artifact_manifest",
                Some(head.snapshot_id.to_string()),
            ));
        }
    }
    if head.artifacts.len() < 2 {
        return Err(corrupt(
            "missing_required_artifact",
            Some(head.snapshot_id.to_string()),
        ));
    }
    Ok(())
}

fn validate_graph_rows(
    connection: &Connection,
    head: &LocalSnapshotHead,
) -> Result<(), StorageError> {
    for table in [
        "entities",
        "relationships",
        "claims",
        "evidence",
        "diagnostics",
        "coverage_gaps",
    ] {
        validate_canonical_blobs(connection, table, &head.snapshot_id)?;
    }
    validate_extraction_rows(connection, head)?;
    let dangling_claims = connection
        .query_row(
            "SELECT COUNT(*)
             FROM claims AS c
             WHERE c.snapshot_id = ?1
               AND (
                 (c.subject_kind = 'entity' AND NOT EXISTS (
                    SELECT 1 FROM entities AS e
                    WHERE e.snapshot_id = c.snapshot_id
                      AND e.entity_id = c.subject_id
                 ))
                 OR
                 (c.subject_kind = 'relationship' AND NOT EXISTS (
                    SELECT 1 FROM relationships AS r
                    WHERE r.snapshot_id = c.snapshot_id
                      AND r.relationship_id = c.subject_id
                 ))
               )",
            [head.snapshot_id.as_str()],
            |row| row.get::<_, i64>(0),
        )
        .map_err(map_sqlite)?;
    if dangling_claims != 0 {
        return Err(corrupt(
            "dangling_claim_subject",
            Some(head.snapshot_id.to_string()),
        ));
    }
    Ok(())
}

fn validate_canonical_blobs(
    connection: &Connection,
    table: &str,
    snapshot_id: &SnapshotId,
) -> Result<(), StorageError> {
    let query = match table {
        "entities" => {
            "SELECT canonical_json FROM entities WHERE snapshot_id = ?1 ORDER BY entity_id"
        }
        "relationships" => {
            "SELECT canonical_json FROM relationships
             WHERE snapshot_id = ?1 ORDER BY relationship_id"
        }
        "claims" => "SELECT canonical_json FROM claims WHERE snapshot_id = ?1 ORDER BY claim_id",
        "evidence" => {
            "SELECT canonical_json FROM evidence WHERE snapshot_id = ?1 ORDER BY evidence_id"
        }
        "diagnostics" => {
            "SELECT canonical_json FROM diagnostics WHERE snapshot_id = ?1 ORDER BY ordinal"
        }
        "coverage_gaps" => {
            "SELECT canonical_json FROM coverage_gaps WHERE snapshot_id = ?1 ORDER BY ordinal"
        }
        _ => return Err(StorageError::PublicationFailed),
    };
    let mut statement = connection.prepare(query).map_err(map_sqlite)?;
    let values = statement
        .query_map([snapshot_id.as_str()], |row| row.get::<_, Vec<u8>>(0))
        .map_err(map_sqlite)?;
    for value in values {
        validate_canonical_json(&value.map_err(map_sqlite)?, snapshot_id)?;
    }
    Ok(())
}

fn validate_extraction_rows(
    connection: &Connection,
    head: &LocalSnapshotHead,
) -> Result<(), StorageError> {
    let expected = head
        .artifacts
        .iter()
        .filter(|reference| reference.role == ArtifactRole::ExtractionChunk)
        .collect::<Vec<_>>();
    let mut statement = connection
        .prepare(
            "SELECT ordinal, artifact_id, canonical_json
             FROM extraction_chunks
             WHERE snapshot_id = ?1
             ORDER BY ordinal",
        )
        .map_err(map_sqlite)?;
    let rows = statement
        .query_map([head.snapshot_id.as_str()], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Vec<u8>>(2)?,
            ))
        })
        .map_err(map_sqlite)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_sqlite)?;
    if rows.len() != expected.len() {
        return Err(corrupt(
            "extraction_manifest_mismatch",
            Some(head.snapshot_id.to_string()),
        ));
    }
    for (row, reference) in rows.into_iter().zip(expected) {
        validate_canonical_json(&row.2, &head.snapshot_id)?;
        if u32::try_from(row.0).ok() != Some(reference.ordinal)
            || row.1 != reference.artifact_id.as_str()
            || ArtifactId::from_bytes(&row.2) != reference.artifact_id
        {
            return Err(corrupt(
                "extraction_manifest_mismatch",
                Some(head.snapshot_id.to_string()),
            ));
        }
    }
    Ok(())
}

fn validate_canonical_json(bytes: &[u8], snapshot_id: &SnapshotId) -> Result<(), StorageError> {
    let value = serde_json::from_slice::<serde_json::Value>(bytes)
        .map_err(|_| corrupt("invalid_canonical_json", Some(snapshot_id.to_string())))?;
    let canonical = serde_json::to_vec(&value)
        .map_err(|_| corrupt("invalid_canonical_json", Some(snapshot_id.to_string())))?;
    if canonical != bytes {
        return Err(corrupt("noncanonical_json", Some(snapshot_id.to_string())));
    }
    Ok(())
}

fn validate_foreign_keys(connection: &Connection) -> Result<(), StorageError> {
    let mut statement = connection
        .prepare("PRAGMA foreign_key_check")
        .map_err(map_sqlite)?;
    if statement.exists([]).map_err(map_sqlite)? {
        return Err(corrupt("foreign_key_check_failed", None));
    }
    Ok(())
}

fn pragma_i64(connection: &Connection, name: &str) -> Result<i64, StorageError> {
    connection
        .query_row(&format!("PRAGMA {name}"), [], |row| row.get(0))
        .map_err(map_sqlite)
}

fn pragma_string(connection: &Connection, name: &str) -> Result<String, StorageError> {
    connection
        .query_row(&format!("PRAGMA {name}"), [], |row| row.get(0))
        .map_err(map_sqlite)
}

fn to_i64(value: u64) -> Result<i64, StorageError> {
    i64::try_from(value).map_err(|_| StorageError::PublicationFailed)
}

fn is_blake3_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn corrupt(reason: &'static str, snapshot_id: Option<String>) -> StorageError {
    StorageError::CorruptMetadata {
        component: StorageComponent::Sqlite,
        reason,
        snapshot_id,
    }
}

fn map_sqlite(error: SqliteError) -> StorageError {
    match error {
        SqliteError::SqliteFailure(details, message)
            if matches!(
                details.code,
                ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked
            ) =>
        {
            drop(message);
            StorageError::WriterBusy
        }
        other => {
            drop(other);
            StorageError::PublicationFailed
        }
    }
}
