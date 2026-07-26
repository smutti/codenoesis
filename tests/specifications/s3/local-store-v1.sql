PRAGMA application_id = 1129205587;
PRAGMA user_version = 1;
PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
PRAGMA synchronous = FULL;
PRAGMA trusted_schema = OFF;
PRAGMA busy_timeout = 0;

CREATE TABLE store_metadata (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
) STRICT;

CREATE TABLE snapshots (
    snapshot_id TEXT PRIMARY KEY NOT NULL,
    repository_identity TEXT NOT NULL,
    commit_oid TEXT NOT NULL,
    snapshot_schema_version TEXT NOT NULL,
    semantic_hash_algorithm TEXT NOT NULL,
    semantic_hash_domain TEXT NOT NULL,
    semantic_hash_value TEXT NOT NULL,
    graph_semantic_hash_value TEXT NOT NULL,
    UNIQUE (repository_identity, semantic_hash_value),
    UNIQUE (snapshot_id, repository_identity)
) STRICT;

CREATE TABLE artifacts (
    artifact_id TEXT PRIMARY KEY NOT NULL,
    digest TEXT NOT NULL UNIQUE,
    byte_length INTEGER NOT NULL CHECK (byte_length >= 2),
    layout_version TEXT NOT NULL
) STRICT;

CREATE TABLE snapshot_artifacts (
    snapshot_id TEXT NOT NULL,
    role TEXT NOT NULL CHECK (
        role IN ('snapshot_semantic', 'knowledge_graph', 'extraction_chunk')
    ),
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    artifact_id TEXT NOT NULL,
    semantic_hash_algorithm TEXT NOT NULL,
    semantic_hash_domain TEXT NOT NULL,
    semantic_hash_value TEXT NOT NULL,
    PRIMARY KEY (snapshot_id, role, ordinal),
    UNIQUE (snapshot_id, artifact_id),
    FOREIGN KEY (snapshot_id) REFERENCES snapshots (snapshot_id),
    FOREIGN KEY (artifact_id) REFERENCES artifacts (artifact_id)
) STRICT;

CREATE TABLE extraction_chunks (
    snapshot_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    chunk_id TEXT NOT NULL,
    artifact_id TEXT NOT NULL,
    canonical_json BLOB NOT NULL CHECK (length(canonical_json) >= 2),
    PRIMARY KEY (snapshot_id, ordinal),
    UNIQUE (snapshot_id, chunk_id),
    FOREIGN KEY (snapshot_id) REFERENCES snapshots (snapshot_id),
    FOREIGN KEY (artifact_id) REFERENCES artifacts (artifact_id)
) STRICT;

CREATE TABLE entities (
    snapshot_id TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    canonical_json BLOB NOT NULL CHECK (length(canonical_json) >= 2),
    PRIMARY KEY (snapshot_id, entity_id),
    FOREIGN KEY (snapshot_id) REFERENCES snapshots (snapshot_id)
) STRICT;

CREATE TABLE relationships (
    snapshot_id TEXT NOT NULL,
    relationship_id TEXT NOT NULL,
    source_entity_id TEXT NOT NULL,
    target_entity_id TEXT NOT NULL,
    canonical_json BLOB NOT NULL CHECK (length(canonical_json) >= 2),
    PRIMARY KEY (snapshot_id, relationship_id),
    FOREIGN KEY (snapshot_id) REFERENCES snapshots (snapshot_id),
    FOREIGN KEY (snapshot_id, source_entity_id)
        REFERENCES entities (snapshot_id, entity_id),
    FOREIGN KEY (snapshot_id, target_entity_id)
        REFERENCES entities (snapshot_id, entity_id)
) STRICT;

CREATE TABLE claims (
    snapshot_id TEXT NOT NULL,
    claim_id TEXT NOT NULL,
    subject_kind TEXT NOT NULL CHECK (
        subject_kind IN ('entity', 'relationship')
    ),
    subject_id TEXT NOT NULL,
    canonical_json BLOB NOT NULL CHECK (length(canonical_json) >= 2),
    PRIMARY KEY (snapshot_id, claim_id),
    UNIQUE (snapshot_id, subject_kind, subject_id),
    FOREIGN KEY (snapshot_id) REFERENCES snapshots (snapshot_id)
) STRICT;

CREATE TABLE evidence (
    snapshot_id TEXT NOT NULL,
    evidence_id TEXT NOT NULL,
    canonical_json BLOB NOT NULL CHECK (length(canonical_json) >= 2),
    PRIMARY KEY (snapshot_id, evidence_id),
    FOREIGN KEY (snapshot_id) REFERENCES snapshots (snapshot_id)
) STRICT;

CREATE TABLE diagnostics (
    snapshot_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    canonical_json BLOB NOT NULL CHECK (length(canonical_json) >= 2),
    PRIMARY KEY (snapshot_id, ordinal),
    FOREIGN KEY (snapshot_id) REFERENCES snapshots (snapshot_id)
) STRICT;

CREATE TABLE coverage_gaps (
    snapshot_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    canonical_json BLOB NOT NULL CHECK (length(canonical_json) >= 2),
    PRIMARY KEY (snapshot_id, ordinal),
    FOREIGN KEY (snapshot_id) REFERENCES snapshots (snapshot_id)
) STRICT;

CREATE TABLE project_heads (
    repository_identity TEXT PRIMARY KEY NOT NULL,
    snapshot_id TEXT NOT NULL,
    generation INTEGER NOT NULL CHECK (generation >= 1),
    FOREIGN KEY (snapshot_id, repository_identity)
        REFERENCES snapshots (snapshot_id, repository_identity)
) STRICT;

CREATE TRIGGER project_heads_validate_insert
BEFORE INSERT ON project_heads
WHEN NEW.generation <> 1
BEGIN
    SELECT RAISE(ABORT, 'invalid_head_generation');
END;

CREATE TRIGGER project_heads_validate_update
BEFORE UPDATE ON project_heads
WHEN
    NEW.repository_identity <> OLD.repository_identity
    OR (
        NEW.snapshot_id = OLD.snapshot_id
        AND NEW.generation <> OLD.generation
    )
    OR (
        NEW.snapshot_id <> OLD.snapshot_id
        AND NEW.generation <> OLD.generation + 1
    )
BEGIN
    SELECT RAISE(ABORT, 'invalid_head_transition');
END;

CREATE TRIGGER project_heads_forbid_delete
BEFORE DELETE ON project_heads
BEGIN
    SELECT RAISE(ABORT, 'head_deletion_not_supported');
END;

CREATE TRIGGER store_metadata_forbid_update
BEFORE UPDATE ON store_metadata
BEGIN
    SELECT RAISE(ABORT, 'immutable_table');
END;

CREATE TRIGGER store_metadata_forbid_delete
BEFORE DELETE ON store_metadata
BEGIN
    SELECT RAISE(ABORT, 'immutable_table');
END;

CREATE TRIGGER snapshots_forbid_update
BEFORE UPDATE ON snapshots
BEGIN
    SELECT RAISE(ABORT, 'immutable_table');
END;

CREATE TRIGGER snapshots_forbid_delete
BEFORE DELETE ON snapshots
BEGIN
    SELECT RAISE(ABORT, 'immutable_table');
END;

CREATE TRIGGER artifacts_forbid_update
BEFORE UPDATE ON artifacts
BEGIN
    SELECT RAISE(ABORT, 'immutable_table');
END;

CREATE TRIGGER artifacts_forbid_delete
BEFORE DELETE ON artifacts
BEGIN
    SELECT RAISE(ABORT, 'immutable_table');
END;

CREATE TRIGGER snapshot_artifacts_forbid_update
BEFORE UPDATE ON snapshot_artifacts
BEGIN
    SELECT RAISE(ABORT, 'immutable_table');
END;

CREATE TRIGGER snapshot_artifacts_forbid_delete
BEFORE DELETE ON snapshot_artifacts
BEGIN
    SELECT RAISE(ABORT, 'immutable_table');
END;

CREATE TRIGGER extraction_chunks_forbid_update
BEFORE UPDATE ON extraction_chunks
BEGIN
    SELECT RAISE(ABORT, 'immutable_table');
END;

CREATE TRIGGER extraction_chunks_forbid_delete
BEFORE DELETE ON extraction_chunks
BEGIN
    SELECT RAISE(ABORT, 'immutable_table');
END;

CREATE TRIGGER entities_forbid_update
BEFORE UPDATE ON entities
BEGIN
    SELECT RAISE(ABORT, 'immutable_table');
END;

CREATE TRIGGER entities_forbid_delete
BEFORE DELETE ON entities
BEGIN
    SELECT RAISE(ABORT, 'immutable_table');
END;

CREATE TRIGGER relationships_forbid_update
BEFORE UPDATE ON relationships
BEGIN
    SELECT RAISE(ABORT, 'immutable_table');
END;

CREATE TRIGGER relationships_forbid_delete
BEFORE DELETE ON relationships
BEGIN
    SELECT RAISE(ABORT, 'immutable_table');
END;

CREATE TRIGGER claims_forbid_update
BEFORE UPDATE ON claims
BEGIN
    SELECT RAISE(ABORT, 'immutable_table');
END;

CREATE TRIGGER claims_forbid_delete
BEFORE DELETE ON claims
BEGIN
    SELECT RAISE(ABORT, 'immutable_table');
END;

CREATE TRIGGER evidence_forbid_update
BEFORE UPDATE ON evidence
BEGIN
    SELECT RAISE(ABORT, 'immutable_table');
END;

CREATE TRIGGER evidence_forbid_delete
BEFORE DELETE ON evidence
BEGIN
    SELECT RAISE(ABORT, 'immutable_table');
END;

CREATE TRIGGER diagnostics_forbid_update
BEFORE UPDATE ON diagnostics
BEGIN
    SELECT RAISE(ABORT, 'immutable_table');
END;

CREATE TRIGGER diagnostics_forbid_delete
BEFORE DELETE ON diagnostics
BEGIN
    SELECT RAISE(ABORT, 'immutable_table');
END;

CREATE TRIGGER coverage_gaps_forbid_update
BEFORE UPDATE ON coverage_gaps
BEGIN
    SELECT RAISE(ABORT, 'immutable_table');
END;

CREATE TRIGGER coverage_gaps_forbid_delete
BEFORE DELETE ON coverage_gaps
BEGIN
    SELECT RAISE(ABORT, 'immutable_table');
END;
