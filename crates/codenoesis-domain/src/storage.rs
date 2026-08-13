use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::{ObjectId, RepositoryIdentity};

pub const LOCAL_STORE_SCHEMA_VERSION: &str = "codenoesis.local-store/v1";
pub const LOCAL_STORE_MARKER_VERSION: &str = "codenoesis.local-store-marker/v1";
pub const FILESYSTEM_CAS_VERSION: &str = "codenoesis.filesystem-cas/v1";
pub const SNAPSHOT_SCHEMA_VERSION: &str = "codenoesis.repository-snapshot/v3";
pub const SNAPSHOT_HASH_DOMAIN: &str = "codenoesis.repository-snapshot.semantic.v3";
pub const GRAPH_HASH_DOMAIN: &str = "codenoesis.knowledge-graph.semantic.v1";
pub const EXTRACTION_HASH_DOMAIN: &str = "codenoesis.extraction-chunk.semantic.v1";
pub const SNAPSHOT_SCHEMA_VERSION_V4: &str = "codenoesis.repository-snapshot/v4";
pub const SNAPSHOT_HASH_DOMAIN_V4: &str = "codenoesis.repository-snapshot.semantic.v4";
pub const SNAPSHOT_SCHEMA_VERSION_V5: &str = "codenoesis.repository-snapshot/v5";
pub const SNAPSHOT_HASH_DOMAIN_V5: &str = "codenoesis.repository-snapshot.semantic.v5";
pub const SNAPSHOT_SCHEMA_VERSION_V6: &str = "codenoesis.repository-snapshot/v6";
pub const SNAPSHOT_HASH_DOMAIN_V6: &str = "codenoesis.repository-snapshot.semantic.v6";
pub const SNAPSHOT_SCHEMA_VERSION_V7: &str = "codenoesis.repository-snapshot/v7";
pub const SNAPSHOT_HASH_DOMAIN_V7: &str = "codenoesis.repository-snapshot.semantic.v7";
pub const SNAPSHOT_SCHEMA_VERSION_V8: &str = "codenoesis.repository-snapshot/v8";
pub const SNAPSHOT_HASH_DOMAIN_V8: &str = "codenoesis.repository-snapshot.semantic.v8";
pub const SNAPSHOT_SCHEMA_VERSION_V9: &str = "codenoesis.repository-snapshot/v9";
pub const SNAPSHOT_HASH_DOMAIN_V9: &str = "codenoesis.repository-snapshot.semantic.v9";
pub const SNAPSHOT_SCHEMA_VERSION_V10: &str = "codenoesis.repository-snapshot/v10";
pub const SNAPSHOT_HASH_DOMAIN_V10: &str = "codenoesis.repository-snapshot.semantic.v10";
pub const SNAPSHOT_SCHEMA_VERSION_V11: &str = "codenoesis.repository-snapshot/v11";
pub const SNAPSHOT_HASH_DOMAIN_V11: &str = "codenoesis.repository-snapshot.semantic.v11";
pub const SNAPSHOT_SCHEMA_VERSION_V12: &str = "codenoesis.repository-snapshot/v12";
pub const SNAPSHOT_HASH_DOMAIN_V12: &str = "codenoesis.repository-snapshot.semantic.v12";
pub const SNAPSHOT_SCHEMA_VERSION_V13: &str = "codenoesis.repository-snapshot/v13";
pub const SNAPSHOT_HASH_DOMAIN_V13: &str = "codenoesis.repository-snapshot.semantic.v13";
pub const SNAPSHOT_SCHEMA_VERSION_V14: &str = "codenoesis.repository-snapshot/v14";
pub const SNAPSHOT_HASH_DOMAIN_V14: &str = "codenoesis.repository-snapshot.semantic.v14";
pub const SNAPSHOT_SCHEMA_VERSION_V15: &str = "codenoesis.repository-snapshot/v15";
pub const SNAPSHOT_HASH_DOMAIN_V15: &str = "codenoesis.repository-snapshot.semantic.v15";
pub const SNAPSHOT_SCHEMA_VERSION_V16: &str = "codenoesis.repository-snapshot/v16";
pub const SNAPSHOT_HASH_DOMAIN_V16: &str = "codenoesis.repository-snapshot.semantic.v16";
pub const SNAPSHOT_SCHEMA_VERSION_V17: &str = "codenoesis.repository-snapshot/v17";
pub const SNAPSHOT_HASH_DOMAIN_V17: &str = "codenoesis.repository-snapshot.semantic.v17";
pub const SNAPSHOT_SCHEMA_VERSION_V18: &str = "codenoesis.repository-snapshot/v18";
pub const SNAPSHOT_HASH_DOMAIN_V18: &str = "codenoesis.repository-snapshot.semantic.v18";
pub const GRAPH_HASH_DOMAIN_V3: &str = "codenoesis.knowledge-graph.semantic.v3";
pub const EXTRACTION_HASH_DOMAIN_V3: &str = "codenoesis.extraction-chunk.semantic.v3";
pub const GRAPH_HASH_DOMAIN_V4: &str = "codenoesis.knowledge-graph.semantic.v4";
pub const EXTRACTION_HASH_DOMAIN_V4: &str = "codenoesis.extraction-chunk.semantic.v4";
pub const GRAPH_HASH_DOMAIN_V5: &str = "codenoesis.knowledge-graph.semantic.v5";
pub const EXTRACTION_HASH_DOMAIN_V5: &str = "codenoesis.extraction-chunk.semantic.v5";
pub const GRAPH_HASH_DOMAIN_V2: &str = "codenoesis.knowledge-graph.semantic.v2";
pub const EXTRACTION_HASH_DOMAIN_V2: &str = "codenoesis.extraction-chunk.semantic.v2";
pub const GRAPH_HASH_DOMAIN_V6: &str = "codenoesis.knowledge-graph.semantic.v6";
pub const EXTRACTION_HASH_DOMAIN_V6: &str = "codenoesis.extraction-chunk.semantic.v6";
pub const GRAPH_HASH_DOMAIN_V7: &str = "codenoesis.knowledge-graph.semantic.v7";
pub const EXTRACTION_HASH_DOMAIN_V7: &str = "codenoesis.extraction-chunk.semantic.v7";
pub const GRAPH_HASH_DOMAIN_V8: &str = "codenoesis.knowledge-graph.semantic.v8";
pub const EXTRACTION_HASH_DOMAIN_V8: &str = "codenoesis.extraction-chunk.semantic.v8";
pub const GRAPH_HASH_DOMAIN_V9: &str = "codenoesis.knowledge-graph.semantic.v9";
pub const EXTRACTION_HASH_DOMAIN_V9: &str = "codenoesis.extraction-chunk.semantic.v9";
pub const GRAPH_HASH_DOMAIN_V10: &str = "codenoesis.knowledge-graph.semantic.v10";
pub const EXTRACTION_HASH_DOMAIN_V10: &str = "codenoesis.extraction-chunk.semantic.v10";
pub const GRAPH_HASH_DOMAIN_V11: &str = "codenoesis.knowledge-graph.semantic.v11";
pub const EXTRACTION_HASH_DOMAIN_V11: &str = "codenoesis.extraction-chunk.semantic.v11";
pub const GRAPH_HASH_DOMAIN_V12: &str = "codenoesis.knowledge-graph.semantic.v12";
pub const EXTRACTION_HASH_DOMAIN_V12: &str = "codenoesis.extraction-chunk.semantic.v12";
pub const GRAPH_HASH_DOMAIN_V13: &str = "codenoesis.knowledge-graph.semantic.v13";
pub const EXTRACTION_HASH_DOMAIN_V13: &str = "codenoesis.extraction-chunk.semantic.v13";
pub const GRAPH_HASH_DOMAIN_V14: &str = "codenoesis.knowledge-graph.semantic.v14";
pub const EXTRACTION_HASH_DOMAIN_V14: &str = "codenoesis.extraction-chunk.semantic.v14";
pub const GRAPH_HASH_DOMAIN_V15: &str = "codenoesis.knowledge-graph.semantic.v15";
pub const EXTRACTION_HASH_DOMAIN_V15: &str = "codenoesis.extraction-chunk.semantic.v15";

const SNAPSHOT_ID_PREFIX: &str = "urn:codenoesis:snapshot:blake3:";
const ARTIFACT_ID_PREFIX: &str = "urn:codenoesis:artifact:blake3:";
const ARTIFACT_ID_DOMAIN: &[u8] = b"codenoesis.artifact-id/v1";

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SnapshotId(String);

impl SnapshotId {
    /// Derives the stable S3 snapshot identifier from a V3 semantic hash.
    ///
    /// # Errors
    ///
    /// Returns a metadata error when the supplied hash is not lowercase
    /// BLAKE3-256 hexadecimal.
    pub fn from_semantic_hash(semantic_hash: &str) -> Result<Self, StorageError> {
        if !is_blake3_hex(semantic_hash) {
            return Err(StorageError::CorruptMetadata {
                component: StorageComponent::Head,
                reason: "invalid_snapshot_semantic_hash",
                snapshot_id: None,
            });
        }
        let preimage = format!("[\"codenoesis.snapshot-id/v1\",\"{semantic_hash}\"]");
        let digest = blake3::hash(preimage.as_bytes()).to_hex();
        Ok(Self(format!("{SNAPSHOT_ID_PREFIX}{digest}")))
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        value
            .strip_prefix(SNAPSHOT_ID_PREFIX)
            .filter(|digest| is_blake3_hex(digest))
            .map(|_| Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for SnapshotId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ArtifactId(String);

impl ArtifactId {
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(ARTIFACT_ID_DOMAIN);
        hasher.update(&[0]);
        hasher.update(bytes);
        Self(format!(
            "{ARTIFACT_ID_PREFIX}{}",
            hasher.finalize().to_hex()
        ))
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        value
            .strip_prefix(ARTIFACT_ID_PREFIX)
            .filter(|digest| is_blake3_hex(digest))
            .map(|_| Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn digest(&self) -> &str {
        &self.0[ARTIFACT_ID_PREFIX.len()..]
    }

    #[must_use]
    pub fn verifies(&self, bytes: &[u8]) -> bool {
        Self::from_bytes(bytes) == *self
    }
}

impl Display for ArtifactId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ArtifactRole {
    SnapshotSemantic,
    KnowledgeGraph,
    ExtractionChunk,
}

impl ArtifactRole {
    pub const ALL: [Self; 3] = [
        Self::SnapshotSemantic,
        Self::KnowledgeGraph,
        Self::ExtractionChunk,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SnapshotSemantic => "snapshot_semantic",
            Self::KnowledgeGraph => "knowledge_graph",
            Self::ExtractionChunk => "extraction_chunk",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "snapshot_semantic" => Some(Self::SnapshotSemantic),
            "knowledge_graph" => Some(Self::KnowledgeGraph),
            "extraction_chunk" => Some(Self::ExtractionChunk),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationBoundary {
    CasBeforeTempCreate,
    CasAfterTempSync,
    CasAfterObjectMove,
    CasAfterParentSync,
    SqliteAfterBegin,
    SqliteAfterSnapshotRows,
    SqliteAfterHeadUpdate,
    SqliteAfterCommit,
}

impl PublicationBoundary {
    pub const ALL: [Self; 8] = [
        Self::CasBeforeTempCreate,
        Self::CasAfterTempSync,
        Self::CasAfterObjectMove,
        Self::CasAfterParentSync,
        Self::SqliteAfterBegin,
        Self::SqliteAfterSnapshotRows,
        Self::SqliteAfterHeadUpdate,
        Self::SqliteAfterCommit,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CasBeforeTempCreate => "cas_before_temp_create",
            Self::CasAfterTempSync => "cas_after_temp_sync",
            Self::CasAfterObjectMove => "cas_after_object_move",
            Self::CasAfterParentSync => "cas_after_parent_sync",
            Self::SqliteAfterBegin => "sqlite_after_begin",
            Self::SqliteAfterSnapshotRows => "sqlite_after_snapshot_rows",
            Self::SqliteAfterHeadUpdate => "sqlite_after_head_update",
            Self::SqliteAfterCommit => "sqlite_after_commit",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationEvent {
    pub boundary: PublicationBoundary,
    pub role: Option<ArtifactRole>,
    pub ordinal: Option<u32>,
}

impl PublicationEvent {
    #[must_use]
    pub const fn cas(boundary: PublicationBoundary, role: ArtifactRole, ordinal: u32) -> Self {
        Self {
            boundary,
            role: Some(role),
            ordinal: Some(ordinal),
        }
    }

    #[must_use]
    pub const fn sqlite(boundary: PublicationBoundary) -> Self {
        Self {
            boundary,
            role: None,
            ordinal: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticHash {
    pub algorithm: String,
    pub domain: String,
    pub value: String,
}

impl SemanticHash {
    #[must_use]
    pub fn blake3(domain: &str, value: &str) -> Self {
        Self {
            algorithm: "blake3-256".to_owned(),
            domain: domain.to_owned(),
            value: value.to_owned(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredArtifact {
    pub role: ArtifactRole,
    pub ordinal: u32,
    pub artifact_id: ArtifactId,
    pub bytes: Vec<u8>,
    pub semantic_hash: SemanticHash,
}

impl StoredArtifact {
    #[must_use]
    pub fn new(
        role: ArtifactRole,
        ordinal: u32,
        bytes: Vec<u8>,
        semantic_hash: SemanticHash,
    ) -> Self {
        let artifact_id = ArtifactId::from_bytes(&bytes);
        Self {
            role,
            ordinal,
            artifact_id,
            bytes,
            semantic_hash,
        }
    }

    #[must_use]
    pub fn byte_length(&self) -> u64 {
        u64::try_from(self.bytes.len()).unwrap_or(u64::MAX)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactReference {
    pub role: ArtifactRole,
    pub ordinal: u32,
    pub artifact_id: ArtifactId,
    pub byte_length: u64,
    pub semantic_hash: SemanticHash,
}

impl From<&StoredArtifact> for ArtifactReference {
    fn from(artifact: &StoredArtifact) -> Self {
        Self {
            role: artifact.role,
            ordinal: artifact.ordinal,
            artifact_id: artifact.artifact_id.clone(),
            byte_length: artifact.byte_length(),
            semantic_hash: artifact.semantic_hash.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtractionRow {
    pub ordinal: u32,
    pub chunk_id: String,
    pub artifact_id: ArtifactId,
    pub canonical_json: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityRow {
    pub entity_id: String,
    pub canonical_json: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationshipRow {
    pub relationship_id: String,
    pub source_entity_id: String,
    pub target_entity_id: String,
    pub canonical_json: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimRow {
    pub claim_id: String,
    pub subject_kind: String,
    pub subject_id: String,
    pub canonical_json: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceRow {
    pub evidence_id: String,
    pub canonical_json: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrdinalRow {
    pub ordinal: u32,
    pub canonical_json: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotRecord {
    pub snapshot_id: SnapshotId,
    pub repository_identity: RepositoryIdentity,
    pub commit_oid: ObjectId,
    pub snapshot_schema_version: String,
    pub semantic_hash: SemanticHash,
    pub graph_semantic_hash: SemanticHash,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationCandidate {
    pub snapshot: SnapshotRecord,
    pub artifacts: Vec<StoredArtifact>,
    pub extraction_chunks: Vec<ExtractionRow>,
    pub entities: Vec<EntityRow>,
    pub relationships: Vec<RelationshipRow>,
    pub claims: Vec<ClaimRow>,
    pub evidence: Vec<EvidenceRow>,
    pub diagnostics: Vec<OrdinalRow>,
    pub coverage_gaps: Vec<OrdinalRow>,
}

impl PublicationCandidate {
    /// Checks stable identities, artifact ordering, cardinality, and exact-byte
    /// integrity before any adapter receives the candidate.
    ///
    /// # Errors
    ///
    /// Returns a typed metadata or object-integrity error.
    pub fn validate(&self) -> Result<(), StorageError> {
        let Some(snapshot_hash_domain) =
            snapshot_hash_domain(&self.snapshot.snapshot_schema_version)
        else {
            return Err(corrupt_candidate("invalid_snapshot_metadata"));
        };
        let Some(graph_hash_domain) = graph_hash_domain(&self.snapshot.snapshot_schema_version)
        else {
            return Err(corrupt_candidate("invalid_snapshot_metadata"));
        };
        let Some(extraction_hash_domain) =
            extraction_hash_domain(&self.snapshot.snapshot_schema_version)
        else {
            return Err(corrupt_candidate("invalid_snapshot_metadata"));
        };
        if self.snapshot.semantic_hash.algorithm != "blake3-256"
            || self.snapshot.semantic_hash.domain != snapshot_hash_domain
            || self.snapshot.graph_semantic_hash.algorithm != "blake3-256"
            || self.snapshot.graph_semantic_hash.domain != graph_hash_domain
            || SnapshotId::from_semantic_hash(&self.snapshot.semantic_hash.value)?
                != self.snapshot.snapshot_id
        {
            return Err(corrupt_candidate("invalid_snapshot_metadata"));
        }
        if self.artifacts.len() < 2 {
            return Err(corrupt_candidate("missing_required_artifact"));
        }
        let mut identifiers = BTreeSet::new();
        let mut snapshot_count = 0_u32;
        let mut graph_count = 0_u32;
        let mut extraction_ordinal = 0_u32;
        let mut previous = None;
        for artifact in &self.artifacts {
            if artifact.bytes.len() < 2 || !artifact.artifact_id.verifies(&artifact.bytes) {
                return Err(StorageError::CorruptObject {
                    artifact_id: artifact.artifact_id.to_string(),
                    expected_hash: artifact.artifact_id.digest().to_owned(),
                    observed_hash: ArtifactId::from_bytes(&artifact.bytes).digest().to_owned(),
                });
            }
            if !identifiers.insert(artifact.artifact_id.clone()) {
                return Err(corrupt_candidate("duplicate_artifact"));
            }
            let order = (artifact.role, artifact.ordinal);
            if previous.is_some_and(|previous| previous >= order) {
                return Err(corrupt_candidate("invalid_artifact_order"));
            }
            previous = Some(order);
            match artifact.role {
                ArtifactRole::SnapshotSemantic
                    if artifact.ordinal == 0
                        && artifact.semantic_hash == self.snapshot.semantic_hash =>
                {
                    snapshot_count += 1;
                }
                ArtifactRole::KnowledgeGraph
                    if artifact.ordinal == 0
                        && artifact.semantic_hash == self.snapshot.graph_semantic_hash =>
                {
                    graph_count += 1;
                }
                ArtifactRole::ExtractionChunk
                    if artifact.ordinal == extraction_ordinal
                        && artifact.semantic_hash.algorithm == "blake3-256"
                        && artifact.semantic_hash.domain == extraction_hash_domain =>
                {
                    extraction_ordinal += 1;
                }
                _ => return Err(corrupt_candidate("invalid_artifact_ordinal")),
            }
        }
        if snapshot_count != 1
            || graph_count != 1
            || usize::try_from(extraction_ordinal).ok() != Some(self.extraction_chunks.len())
        {
            return Err(corrupt_candidate("invalid_artifact_cardinality"));
        }
        Ok(())
    }

    #[must_use]
    pub fn artifact_references(&self) -> Vec<ArtifactReference> {
        self.artifacts.iter().map(ArtifactReference::from).collect()
    }
}

#[must_use]
pub fn snapshot_hash_domain(snapshot_schema_version: &str) -> Option<&'static str> {
    match snapshot_schema_version {
        SNAPSHOT_SCHEMA_VERSION => Some(SNAPSHOT_HASH_DOMAIN),
        SNAPSHOT_SCHEMA_VERSION_V4 => Some(SNAPSHOT_HASH_DOMAIN_V4),
        SNAPSHOT_SCHEMA_VERSION_V5 => Some(SNAPSHOT_HASH_DOMAIN_V5),
        SNAPSHOT_SCHEMA_VERSION_V6 => Some(SNAPSHOT_HASH_DOMAIN_V6),
        SNAPSHOT_SCHEMA_VERSION_V7 => Some(SNAPSHOT_HASH_DOMAIN_V7),
        SNAPSHOT_SCHEMA_VERSION_V8 => Some(SNAPSHOT_HASH_DOMAIN_V8),
        SNAPSHOT_SCHEMA_VERSION_V9 => Some(SNAPSHOT_HASH_DOMAIN_V9),
        SNAPSHOT_SCHEMA_VERSION_V10 => Some(SNAPSHOT_HASH_DOMAIN_V10),
        SNAPSHOT_SCHEMA_VERSION_V11 => Some(SNAPSHOT_HASH_DOMAIN_V11),
        SNAPSHOT_SCHEMA_VERSION_V12 => Some(SNAPSHOT_HASH_DOMAIN_V12),
        SNAPSHOT_SCHEMA_VERSION_V13 => Some(SNAPSHOT_HASH_DOMAIN_V13),
        SNAPSHOT_SCHEMA_VERSION_V14 => Some(SNAPSHOT_HASH_DOMAIN_V14),
        SNAPSHOT_SCHEMA_VERSION_V15 => Some(SNAPSHOT_HASH_DOMAIN_V15),
        SNAPSHOT_SCHEMA_VERSION_V16 => Some(SNAPSHOT_HASH_DOMAIN_V16),
        SNAPSHOT_SCHEMA_VERSION_V17 => Some(SNAPSHOT_HASH_DOMAIN_V17),
        SNAPSHOT_SCHEMA_VERSION_V18 => Some(SNAPSHOT_HASH_DOMAIN_V18),
        _ => None,
    }
}

#[must_use]
pub fn graph_hash_domain(snapshot_schema_version: &str) -> Option<&'static str> {
    match snapshot_schema_version {
        SNAPSHOT_SCHEMA_VERSION => Some(GRAPH_HASH_DOMAIN),
        SNAPSHOT_SCHEMA_VERSION_V4 | SNAPSHOT_SCHEMA_VERSION_V5 => Some(GRAPH_HASH_DOMAIN_V2),
        SNAPSHOT_SCHEMA_VERSION_V6 => Some(GRAPH_HASH_DOMAIN_V3),
        SNAPSHOT_SCHEMA_VERSION_V7 => Some(GRAPH_HASH_DOMAIN_V4),
        SNAPSHOT_SCHEMA_VERSION_V8 => Some(GRAPH_HASH_DOMAIN_V5),
        SNAPSHOT_SCHEMA_VERSION_V9 => Some(GRAPH_HASH_DOMAIN_V6),
        SNAPSHOT_SCHEMA_VERSION_V10 => Some(GRAPH_HASH_DOMAIN_V7),
        SNAPSHOT_SCHEMA_VERSION_V11 => Some(GRAPH_HASH_DOMAIN_V8),
        SNAPSHOT_SCHEMA_VERSION_V12 => Some(GRAPH_HASH_DOMAIN_V9),
        SNAPSHOT_SCHEMA_VERSION_V13 => Some(GRAPH_HASH_DOMAIN_V10),
        SNAPSHOT_SCHEMA_VERSION_V14 => Some(GRAPH_HASH_DOMAIN_V11),
        SNAPSHOT_SCHEMA_VERSION_V15 => Some(GRAPH_HASH_DOMAIN_V12),
        SNAPSHOT_SCHEMA_VERSION_V16 => Some(GRAPH_HASH_DOMAIN_V13),
        SNAPSHOT_SCHEMA_VERSION_V17 => Some(GRAPH_HASH_DOMAIN_V14),
        SNAPSHOT_SCHEMA_VERSION_V18 => Some(GRAPH_HASH_DOMAIN_V15),
        _ => None,
    }
}

#[must_use]
pub fn extraction_hash_domain(snapshot_schema_version: &str) -> Option<&'static str> {
    match snapshot_schema_version {
        SNAPSHOT_SCHEMA_VERSION => Some(EXTRACTION_HASH_DOMAIN),
        SNAPSHOT_SCHEMA_VERSION_V4 | SNAPSHOT_SCHEMA_VERSION_V5 => Some(EXTRACTION_HASH_DOMAIN_V2),
        SNAPSHOT_SCHEMA_VERSION_V6 => Some(EXTRACTION_HASH_DOMAIN_V3),
        SNAPSHOT_SCHEMA_VERSION_V7 => Some(EXTRACTION_HASH_DOMAIN_V4),
        SNAPSHOT_SCHEMA_VERSION_V8 => Some(EXTRACTION_HASH_DOMAIN_V5),
        SNAPSHOT_SCHEMA_VERSION_V9 => Some(EXTRACTION_HASH_DOMAIN_V6),
        SNAPSHOT_SCHEMA_VERSION_V10 => Some(EXTRACTION_HASH_DOMAIN_V7),
        SNAPSHOT_SCHEMA_VERSION_V11 => Some(EXTRACTION_HASH_DOMAIN_V8),
        SNAPSHOT_SCHEMA_VERSION_V12 => Some(EXTRACTION_HASH_DOMAIN_V9),
        SNAPSHOT_SCHEMA_VERSION_V13 => Some(EXTRACTION_HASH_DOMAIN_V10),
        SNAPSHOT_SCHEMA_VERSION_V14 => Some(EXTRACTION_HASH_DOMAIN_V11),
        SNAPSHOT_SCHEMA_VERSION_V15 => Some(EXTRACTION_HASH_DOMAIN_V12),
        SNAPSHOT_SCHEMA_VERSION_V16 => Some(EXTRACTION_HASH_DOMAIN_V13),
        SNAPSHOT_SCHEMA_VERSION_V17 => Some(EXTRACTION_HASH_DOMAIN_V14),
        SNAPSHOT_SCHEMA_VERSION_V18 => Some(EXTRACTION_HASH_DOMAIN_V15),
        _ => None,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalSnapshotHead {
    pub repository_identity: RepositoryIdentity,
    pub snapshot_id: SnapshotId,
    pub commit_oid: ObjectId,
    pub snapshot_schema_version: String,
    pub semantic_hash: SemanticHash,
    pub graph_semantic_hash: SemanticHash,
    pub generation: u64,
    pub artifacts: Vec<ArtifactReference>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationResult {
    pub head: LocalSnapshotHead,
    pub changed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SweepResult {
    pub temporary_files_removed: u64,
    pub objects_removed: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageComponent {
    Marker,
    Sqlite,
    Cas,
    Head,
}

impl StorageComponent {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Marker => "marker",
            Self::Sqlite => "sqlite",
            Self::Cas => "cas",
            Self::Head => "head",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageError {
    UnmarkedNonemptyRoot,
    IncompatibleSchema {
        observed_schema: String,
    },
    WriterBusy,
    MissingObject {
        artifact_id: String,
    },
    CorruptObject {
        artifact_id: String,
        expected_hash: String,
        observed_hash: String,
    },
    CorruptMetadata {
        component: StorageComponent,
        reason: &'static str,
        snapshot_id: Option<String>,
    },
    UnsafePath {
        reason: &'static str,
    },
    HeadConflict {
        expected: Option<String>,
        actual: Option<String>,
    },
    PublicationFailed,
}

impl StorageError {
    #[must_use]
    pub const fn retryable(&self) -> bool {
        matches!(self, Self::WriterBusy | Self::HeadConflict { .. })
    }
}

impl Display for StorageError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnmarkedNonemptyRoot => "the local store root is not marked",
            Self::IncompatibleSchema { .. } => "the local store schema is not supported",
            Self::WriterBusy => "the local store writer is busy",
            Self::MissingObject { .. } => "a head-referenced content object is missing",
            Self::CorruptObject { .. } => {
                "a head-referenced content object failed integrity validation"
            }
            Self::CorruptMetadata { .. } => "the local store metadata is corrupt",
            Self::UnsafePath { .. } => "the store root violates the local storage path policy",
            Self::HeadConflict { .. } => "the project head changed concurrently",
            Self::PublicationFailed => "atomic snapshot publication failed",
        })
    }
}

impl Error for StorageError {}

fn corrupt_candidate(reason: &'static str) -> StorageError {
    StorageError::CorruptMetadata {
        component: StorageComponent::Head,
        reason,
        snapshot_id: None,
    }
}

fn is_blake3_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}
