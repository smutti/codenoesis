use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::knowledge::{ClaimState, ClaimSubjectKind};
use crate::s4::{WorkspaceClaim, WorkspaceEntity, workspace_claim_id};
use crate::s4_r6::FrameworkKnowledge;

pub const R7_ONTOLOGY_VERSION: &str = "codenoesis.ontology/rust/v7";
pub const R7_PIPELINE_VERSION: &str = "codenoesis.pipeline/s4-r7-v1";
pub const R7_COMPILER_EXTRACTOR_VERSION: &str = "codenoesis.scip-import/s4-r7-v1";
pub const R7_EXTRACTION_CONTRACT_VERSION: &str = "codenoesis.extraction/v7";
pub const R7_COMPILER_INDEX_PROFILE: &str = "scip-rust-v0.9.0-import-v1";
pub const R7_COMPILER_INDEX_VERSION: &str = "codenoesis.compiler-index/v1";
pub const R7_SEMANTIC_HASH_CONTRACT_VERSION: &str = "codenoesis.semantic-hash-contract/v6";
pub const R7_CONFIGURATION_VERSION: &str = "codenoesis.configuration/v7";
pub const R7_SNAPSHOT_VERSION: &str = "codenoesis.repository-snapshot/v10";
pub const R7_EXTRACTION_CHUNK_VERSION: &str = "codenoesis.extraction-chunk/v7";
pub const R7_GRAPH_VERSION: &str = "codenoesis.knowledge-graph/v7";
pub const R7_ERROR_VERSION: &str = "codenoesis.error/v14";
pub const R7_QUERY_VERSION: &str = "codenoesis.local-query-result/v5";

pub const MAX_R7_RAW_INDEX_BYTES: u64 = 67_108_864;
pub const MAX_R7_BINDING_JSON_BYTES: u64 = 1_048_576;
pub const MAX_R7_DOCUMENTS: u64 = 20_000;
pub const MAX_R7_OCCURRENCES_TOTAL: u64 = 1_000_000;
pub const MAX_R7_OCCURRENCES_PER_DOCUMENT: u64 = 100_000;
pub const MAX_R7_SYMBOL_INFORMATION_TOTAL: u64 = 250_000;
pub const MAX_R7_RELATIONSHIPS_TOTAL: u64 = 500_000;
pub const MAX_R7_SYMBOL_OR_DISPLAY_BYTES: u64 = 16_384;
pub const MAX_R7_UNPROMOTED_VALUE_BYTES: u64 = 65_536;
pub const MAX_R7_TOOL_ARGUMENTS: u64 = 128;
pub const MAX_R7_TOOL_ARGUMENT_BYTES: u64 = 4_096;
pub const MAX_R7_PROTOBUF_RECURSION: u64 = 64;
pub const R7_DETERMINISM_PERMUTATIONS: u64 = 50;

pub const COMPILER_SYMBOL_ID_DOMAIN: &str = "codenoesis.entity-id/compiler-symbol/v1";
pub const COMPILER_RELATIONSHIP_ID_DOMAIN: &str = "codenoesis.relationship-id/compiler-index/v1";
const COMPILER_DIAGNOSTIC_ID_DOMAIN: &str = "codenoesis.diagnostic-id/compiler-index/v1";
const COMPILER_COVERAGE_ID_DOMAIN: &str = "codenoesis.coverage-gap-id/compiler-index/v1";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CompilerBindingState {
    InRepositoryBound,
    ExternalUnbound,
    GeneratedUnbound,
}

impl CompilerBindingState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InRepositoryBound => "in_repository_bound",
            Self::ExternalUnbound => "external_unbound",
            Self::GeneratedUnbound => "generated_unbound",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CompilerRelationshipKind {
    ResolvesTo,
    References,
    Implements,
    TypeDefinition,
}

impl CompilerRelationshipKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ResolvesTo => "RESOLVES_TO",
            Self::References => "REFERENCES",
            Self::Implements => "IMPLEMENTS",
            Self::TypeDefinition => "TYPE_DEFINITION",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CompilerEvidenceRecordKind {
    Occurrence,
    ExternalSymbol,
    OccurrenceResolution,
    OccurrenceReference,
    SymbolRelationship,
}

impl CompilerEvidenceRecordKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Occurrence => "occurrence",
            Self::ExternalSymbol => "external_symbol",
            Self::OccurrenceResolution => "occurrence_resolution",
            Self::OccurrenceReference => "occurrence_reference",
            Self::SymbolRelationship => "symbol_relationship",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CompilerCoverageState {
    Unsupported,
    Unbound,
    NotIndexed,
    Redacted,
}

impl CompilerCoverageState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unsupported => "unsupported",
            Self::Unbound => "unbound",
            Self::NotIndexed => "not_indexed",
            Self::Redacted => "redacted",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerProducer {
    pub family: String,
    pub name: String,
    pub version: String,
    pub commit: String,
    pub arguments_sha256: String,
    pub project_root_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerToolchain {
    pub channel: String,
    pub rustc_release: String,
    pub rustc_commit: String,
    pub target_triple: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerEvidenceLocator {
    pub record_kind: CompilerEvidenceRecordKind,
    pub document_path: Option<String>,
    pub range: Option<Vec<u32>>,
    pub symbol: String,
    pub symbol_roles: Option<u32>,
    pub relationship_target: Option<String>,
    pub relationship_flags: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerEvidence {
    pub id: String,
    pub artifact_sha256: String,
    pub locator: CompilerEvidenceLocator,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerSourceEvidence {
    pub id: String,
    pub path: String,
    pub blob_oid: String,
    pub start_byte: u64,
    pub end_byte: u64,
    pub source_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerSymbol {
    pub id: String,
    pub symbol: String,
    pub display_name: String,
    pub scope: String,
    pub binding_state: CompilerBindingState,
    pub identity_preimage: Vec<String>,
    pub source_entity_id: Option<String>,
    pub compiler_evidence_ids: Vec<String>,
    pub source_evidence_ids: Vec<String>,
    pub document_path: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerSyntaxReference {
    pub entity: WorkspaceEntity,
    pub document_path: String,
    pub evidence_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerRelationship {
    pub id: String,
    pub kind: CompilerRelationshipKind,
    pub source: String,
    pub target: String,
    pub evidence_ids: Vec<String>,
    pub document_path: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerDiagnostic {
    pub id: String,
    pub code: String,
    pub subject_id: String,
    pub compiler_target_id: Option<String>,
    pub evidence_ids: Vec<String>,
    pub document_path: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerCoverageGap {
    pub id: String,
    pub subject: String,
    pub capability: String,
    pub state: CompilerCoverageState,
    pub evidence_ids: Vec<String>,
    pub document_path: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerIndexOverlay {
    pub repository_identity: String,
    pub binding_sha256: String,
    pub artifact_sha256: String,
    pub producer: CompilerProducer,
    pub toolchain: CompilerToolchain,
    pub coverage_mode: String,
    pub primary_document_path: String,
    pub symbols: Vec<CompilerSymbol>,
    pub syntax_references: Vec<CompilerSyntaxReference>,
    pub relationships: Vec<CompilerRelationship>,
    pub claims: Vec<WorkspaceClaim>,
    pub compiler_evidence: Vec<CompilerEvidence>,
    pub source_evidence: Vec<CompilerSourceEvidence>,
    pub diagnostics: Vec<CompilerDiagnostic>,
    pub coverage: Vec<CompilerCoverageGap>,
}

impl CompilerIndexOverlay {
    /// Validates one normalized compiler overlay against its immutable R6 lineage.
    ///
    /// # Errors
    ///
    /// Returns a closed compiler-index contract failure on identity, ordering,
    /// endpoint, evidence, or provenance inconsistency.
    #[allow(clippy::too_many_lines)]
    pub fn validate(&self, source: &FrameworkKnowledge) -> Result<(), CompilerIndexError> {
        source
            .validate()
            .map_err(|_| CompilerIndexError::ContractInvalid)?;
        let repository_identity = &source
            .semantic
            .manifest
            .workspace
            .knowledge
            .graph
            .repository_identity;
        if self.repository_identity != *repository_identity
            || !is_lower_hex(&self.binding_sha256, 64)
            || !is_lower_hex(&self.artifact_sha256, 64)
            || self.coverage_mode != "declared_partial"
            || self.primary_document_path.is_empty()
            || !is_lower_hex(&self.producer.commit, 40)
            || !is_lower_hex(&self.producer.arguments_sha256, 64)
            || !is_lower_hex(&self.producer.project_root_sha256, 64)
            || !is_lower_hex(&self.toolchain.rustc_commit, 40)
            || self.producer.family != "rust-analyzer-scip"
            || self.producer.name != "rust-analyzer"
        {
            return Err(CompilerIndexError::ContractInvalid);
        }

        if !ordered_unique(self.symbols.iter().map(|value| value.id.as_str()))
            || !ordered_unique(
                self.syntax_references
                    .iter()
                    .map(|value| value.entity.id.as_str()),
            )
            || !ordered_unique(self.relationships.iter().map(|value| value.id.as_str()))
            || !ordered_unique(self.claims.iter().map(|value| value.id.as_str()))
            || !ordered_unique(self.compiler_evidence.iter().map(|value| value.id.as_str()))
            || !ordered_unique(self.source_evidence.iter().map(|value| value.id.as_str()))
            || !ordered_unique(self.diagnostics.iter().map(|value| value.id.as_str()))
            || !ordered_unique(self.coverage.iter().map(|value| value.id.as_str()))
        {
            return Err(CompilerIndexError::ContractInvalid);
        }

        let compiler_evidence_ids = self
            .compiler_evidence
            .iter()
            .map(|value| value.id.as_str())
            .collect::<BTreeSet<_>>();
        let source_evidence_ids = self
            .source_evidence
            .iter()
            .map(|value| value.id.as_str())
            .collect::<BTreeSet<_>>();
        let all_new_evidence = compiler_evidence_ids
            .iter()
            .copied()
            .chain(source_evidence_ids.iter().copied())
            .collect::<BTreeSet<_>>();
        let mut endpoints = source_entity_ids(source);
        endpoints.extend(self.symbols.iter().map(|value| value.id.as_str()));
        endpoints.extend(
            self.syntax_references
                .iter()
                .map(|value| value.entity.id.as_str()),
        );

        for evidence in &self.compiler_evidence {
            if !valid_compiler_evidence_id(&evidence.id)
                || evidence.artifact_sha256 != self.artifact_sha256
                || evidence.locator.symbol.is_empty()
                || evidence.locator.symbol.len()
                    > usize::try_from(MAX_R7_SYMBOL_OR_DISPLAY_BYTES).unwrap_or(usize::MAX)
                || evidence
                    .locator
                    .symbol_roles
                    .is_some_and(|roles| roles > 127)
                || evidence.locator.range.as_ref().is_some_and(|range| {
                    !matches!(range.len(), 3 | 4)
                        || range.len() == 3 && range[1] > range[2]
                        || range.len() == 4 && (range[0], range[1]) > (range[2], range[3])
                })
            {
                return Err(CompilerIndexError::ContractInvalid);
            }
        }
        for evidence in &self.source_evidence {
            if !valid_compiler_evidence_id(&evidence.id)
                || evidence.path.is_empty()
                || !is_lower_hex(&evidence.blob_oid, 40)
                || !is_lower_hex(&evidence.source_sha256, 64)
                || evidence.start_byte >= evidence.end_byte
            {
                return Err(CompilerIndexError::ContractInvalid);
            }
        }
        for symbol in &self.symbols {
            if symbol.id != compiler_symbol_id(&symbol.identity_preimage)
                || symbol.symbol.is_empty()
                || symbol.display_name.len()
                    > usize::try_from(MAX_R7_SYMBOL_OR_DISPLAY_BYTES).unwrap_or(usize::MAX)
                || !matches!(symbol.scope.as_str(), "global" | "local")
                || symbol.compiler_evidence_ids.is_empty()
                || symbol
                    .compiler_evidence_ids
                    .iter()
                    .any(|value| !compiler_evidence_ids.contains(value.as_str()))
                || symbol
                    .source_evidence_ids
                    .iter()
                    .any(|value| !source_evidence_ids.contains(value.as_str()))
                || symbol.binding_state == CompilerBindingState::ExternalUnbound
                    && (!symbol.source_evidence_ids.is_empty()
                        || symbol.source_entity_id.is_some()
                        || symbol.document_path.is_some())
                || symbol.binding_state == CompilerBindingState::InRepositoryBound
                    && (symbol.source_evidence_ids.is_empty() || symbol.document_path.is_none())
                || symbol
                    .source_entity_id
                    .as_ref()
                    .is_some_and(|value| !endpoints.contains(value.as_str()))
            {
                return Err(CompilerIndexError::ContractInvalid);
            }
        }
        for relationship in &self.relationships {
            if relationship.id
                != compiler_relationship_id(
                    relationship.kind,
                    &relationship.source,
                    &relationship.target,
                )
                || !endpoints.contains(relationship.source.as_str())
                || !endpoints.contains(relationship.target.as_str())
                || relationship.evidence_ids.is_empty()
                || relationship
                    .evidence_ids
                    .iter()
                    .any(|value| !compiler_evidence_ids.contains(value.as_str()))
            {
                return Err(CompilerIndexError::ContractInvalid);
            }
        }
        for claim in &self.claims {
            if claim.state != ClaimState::DeterministicFact
                || claim.id
                    != workspace_claim_id(claim.subject_kind, &claim.subject_id, claim.state)
                || claim.evidence_ids.is_empty()
                || claim
                    .evidence_ids
                    .iter()
                    .any(|value| !all_new_evidence.contains(value.as_str()))
                || match claim.subject_kind {
                    ClaimSubjectKind::Entity => !endpoints.contains(claim.subject_id.as_str()),
                    ClaimSubjectKind::Relationship => !self
                        .relationships
                        .iter()
                        .any(|value| value.id == claim.subject_id),
                }
            {
                return Err(CompilerIndexError::ContractInvalid);
            }
        }
        for diagnostic in &self.diagnostics {
            if diagnostic.id
                != compiler_diagnostic_id(
                    &diagnostic.code,
                    &diagnostic.subject_id,
                    diagnostic.compiler_target_id.as_deref(),
                    &diagnostic.evidence_ids,
                )
                || !endpoints.contains(diagnostic.subject_id.as_str())
                || diagnostic
                    .compiler_target_id
                    .as_ref()
                    .is_some_and(|value| !endpoints.contains(value.as_str()))
                || diagnostic
                    .evidence_ids
                    .iter()
                    .any(|value| !all_new_evidence.contains(value.as_str()))
            {
                return Err(CompilerIndexError::ContractInvalid);
            }
        }
        for gap in &self.coverage {
            if gap.id
                != compiler_coverage_gap_id(
                    &gap.subject,
                    &gap.capability,
                    gap.state,
                    &gap.evidence_ids,
                )
                || gap.subject.is_empty()
                || gap
                    .evidence_ids
                    .iter()
                    .any(|value| !all_new_evidence.contains(value.as_str()))
            {
                return Err(CompilerIndexError::ContractInvalid);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CompilerIndexLimit {
    RawIndexBytes,
    BindingJsonBytes,
    Documents,
    OccurrencesTotal,
    OccurrencesPerDocument,
    SymbolInformationTotal,
    RelationshipsTotal,
    SymbolOrDisplayBytes,
    UnpromotedValueBytes,
    ToolArguments,
    ToolArgumentBytes,
    ProtobufRecursion,
}

impl CompilerIndexLimit {
    pub const ALL: [Self; 12] = [
        Self::RawIndexBytes,
        Self::BindingJsonBytes,
        Self::Documents,
        Self::OccurrencesTotal,
        Self::OccurrencesPerDocument,
        Self::SymbolInformationTotal,
        Self::RelationshipsTotal,
        Self::SymbolOrDisplayBytes,
        Self::UnpromotedValueBytes,
        Self::ToolArguments,
        Self::ToolArgumentBytes,
        Self::ProtobufRecursion,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RawIndexBytes => "raw_index_bytes",
            Self::BindingJsonBytes => "binding_json_bytes",
            Self::Documents => "documents",
            Self::OccurrencesTotal => "occurrences_total",
            Self::OccurrencesPerDocument => "occurrences_per_document",
            Self::SymbolInformationTotal => "symbol_information_total",
            Self::RelationshipsTotal => "relationships_total",
            Self::SymbolOrDisplayBytes => "symbol_or_display_bytes",
            Self::UnpromotedValueBytes => "unpromoted_value_bytes",
            Self::ToolArguments => "tool_arguments",
            Self::ToolArgumentBytes => "tool_argument_bytes",
            Self::ProtobufRecursion => "protobuf_recursion",
        }
    }

    #[must_use]
    pub const fn maximum(self) -> u64 {
        match self {
            Self::RawIndexBytes => MAX_R7_RAW_INDEX_BYTES,
            Self::BindingJsonBytes => MAX_R7_BINDING_JSON_BYTES,
            Self::Documents => MAX_R7_DOCUMENTS,
            Self::OccurrencesTotal => MAX_R7_OCCURRENCES_TOTAL,
            Self::OccurrencesPerDocument => MAX_R7_OCCURRENCES_PER_DOCUMENT,
            Self::SymbolInformationTotal => MAX_R7_SYMBOL_INFORMATION_TOTAL,
            Self::RelationshipsTotal => MAX_R7_RELATIONSHIPS_TOTAL,
            Self::SymbolOrDisplayBytes => MAX_R7_SYMBOL_OR_DISPLAY_BYTES,
            Self::UnpromotedValueBytes => MAX_R7_UNPROMOTED_VALUE_BYTES,
            Self::ToolArguments => MAX_R7_TOOL_ARGUMENTS,
            Self::ToolArgumentBytes => MAX_R7_TOOL_ARGUMENT_BYTES,
            Self::ProtobufRecursion => MAX_R7_PROTOBUF_RECURSION,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilerIndexMismatchSubject {
    Artifact,
    Repository,
    Revision,
    Tree,
    SourceManifest,
    Document,
    Producer,
    Toolchain,
}

impl CompilerIndexMismatchSubject {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Artifact => "artifact",
            Self::Repository => "repository",
            Self::Revision => "revision",
            Self::Tree => "tree",
            Self::SourceManifest => "source_manifest",
            Self::Document => "document",
            Self::Producer => "producer",
            Self::Toolchain => "toolchain",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompilerIndexError {
    UnsafePath {
        path: String,
        reason: String,
    },
    InvalidBinding {
        path: String,
        reason: String,
    },
    UnsupportedSchema {
        commit: String,
        scip_proto_sha256: String,
    },
    UnsupportedProducer {
        name: String,
        version_sha256: String,
        commit_sha256: String,
    },
    BindingMismatch {
        subject: CompilerIndexMismatchSubject,
        expected_sha256: String,
        observed_sha256: String,
    },
    MalformedArtifact {
        artifact_sha256: String,
        reason: String,
    },
    NoncanonicalArtifact {
        artifact_sha256: String,
        reason: String,
    },
    IdentityConflict {
        normalized_preimage_sha256: String,
    },
    AmbiguousEndpoint {
        symbol_sha256: String,
        candidate_count: u64,
    },
    RelationConflict {
        kind: CompilerRelationshipKind,
        source_id: String,
        target_id: String,
        reason: String,
    },
    LimitExceeded {
        limit: CompilerIndexLimit,
        maximum: u64,
        observed: u64,
    },
    UnresolvableEvidence {
        evidence_id: String,
    },
    ContractInvalid,
}

impl Display for CompilerIndexError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsafePath { .. } => "unsafe compiler-index path",
            Self::InvalidBinding { .. } => "invalid compiler-index binding",
            Self::UnsupportedSchema { .. } => "unsupported compiler-index schema",
            Self::UnsupportedProducer { .. } => "unsupported compiler-index producer",
            Self::BindingMismatch { .. } => "compiler-index binding mismatch",
            Self::MalformedArtifact { .. } => "malformed compiler index",
            Self::NoncanonicalArtifact { .. } => "noncanonical compiler index",
            Self::IdentityConflict { .. } => "compiler-index identity conflict",
            Self::AmbiguousEndpoint { .. } => "ambiguous compiler-index endpoint",
            Self::RelationConflict { .. } => "compiler-index relationship conflict",
            Self::LimitExceeded { .. } => "compiler-index limit exceeded",
            Self::UnresolvableEvidence { .. } => "unresolvable compiler-index evidence",
            Self::ContractInvalid => "invalid compiler-index contract",
        })
    }
}

impl Error for CompilerIndexError {}

#[must_use]
pub const fn compiler_index_limit_exceeded(
    limit: CompilerIndexLimit,
    observed: u64,
) -> CompilerIndexError {
    let maximum = limit.maximum();
    CompilerIndexError::LimitExceeded {
        limit,
        maximum,
        observed: if observed > maximum + 1 {
            maximum + 1
        } else {
            observed
        },
    }
}

#[must_use]
pub fn compiler_symbol_id(normalized_preimage: &[String]) -> String {
    let mut components = Vec::with_capacity(normalized_preimage.len() + 1);
    components.push(COMPILER_SYMBOL_ID_DOMAIN);
    components.extend(normalized_preimage.iter().map(String::as_str));
    stable_id("urn:codenoesis:entity:blake3:", &components)
}

#[must_use]
pub fn compiler_relationship_id(
    kind: CompilerRelationshipKind,
    source: &str,
    target: &str,
) -> String {
    stable_id(
        "urn:codenoesis:relationship:blake3:",
        &[
            COMPILER_RELATIONSHIP_ID_DOMAIN,
            kind.as_str(),
            source,
            target,
        ],
    )
}

#[must_use]
pub fn compiler_diagnostic_id(
    code: &str,
    subject_id: &str,
    compiler_target_id: Option<&str>,
    evidence_ids: &[String],
) -> String {
    let mut components = vec![
        COMPILER_DIAGNOSTIC_ID_DOMAIN,
        code,
        subject_id,
        compiler_target_id.unwrap_or("none"),
    ];
    components.extend(evidence_ids.iter().map(String::as_str));
    stable_id("urn:codenoesis:diagnostic:blake3:", &components)
}

#[must_use]
pub fn compiler_coverage_gap_id(
    subject: &str,
    capability: &str,
    state: CompilerCoverageState,
    evidence_ids: &[String],
) -> String {
    let mut components = vec![
        COMPILER_COVERAGE_ID_DOMAIN,
        subject,
        capability,
        state.as_str(),
    ];
    components.extend(evidence_ids.iter().map(String::as_str));
    stable_id("urn:codenoesis:coverage-gap:blake3:", &components)
}

fn source_entity_ids(source: &FrameworkKnowledge) -> BTreeSet<&str> {
    source
        .semantic
        .manifest
        .workspace
        .knowledge
        .graph
        .entities
        .iter()
        .map(|value| value.id.as_str())
        .chain(
            source
                .semantic
                .manifest
                .graph
                .entities
                .iter()
                .map(|value| value.id.as_str()),
        )
        .chain(
            source
                .semantic
                .graph
                .legacy_entities
                .iter()
                .map(|value| value.id.as_str()),
        )
        .chain(
            source
                .semantic
                .graph
                .entities
                .iter()
                .map(|value| value.id.as_str()),
        )
        .chain(
            source
                .graph
                .supplemental_entities
                .iter()
                .map(|value| value.id.as_str()),
        )
        .chain(
            source
                .graph
                .declarations
                .iter()
                .map(|value| value.id.as_str()),
        )
        .collect()
}

fn ordered_unique<'a>(values: impl IntoIterator<Item = &'a str>) -> bool {
    let mut previous = None;
    for value in values {
        if previous.is_some_and(|previous| previous >= value) {
            return false;
        }
        previous = Some(value);
    }
    true
}

fn valid_compiler_evidence_id(value: &str) -> bool {
    value
        .strip_prefix("urn:codenoesis:evidence:sha256:")
        .is_some_and(|digest| is_lower_hex(digest, 64))
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn stable_id(prefix: &str, components: &[&str]) -> String {
    format!(
        "{prefix}{}",
        blake3::hash(&canonical_string_array(components)).to_hex()
    )
}

fn canonical_string_array(components: &[&str]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.push(b'[');
    for (index, component) in components.iter().enumerate() {
        if index > 0 {
            bytes.push(b',');
        }
        write_json_string(&mut bytes, component);
    }
    bytes.push(b']');
    bytes
}

fn write_json_string(bytes: &mut Vec<u8>, value: &str) {
    bytes.push(b'"');
    for character in value.chars() {
        match character {
            '"' => bytes.extend_from_slice(br#"\""#),
            '\\' => bytes.extend_from_slice(br"\\"),
            '\u{0008}' => bytes.extend_from_slice(br"\b"),
            '\u{000c}' => bytes.extend_from_slice(br"\f"),
            '\n' => bytes.extend_from_slice(br"\n"),
            '\r' => bytes.extend_from_slice(br"\r"),
            '\t' => bytes.extend_from_slice(br"\t"),
            '\u{0000}'..='\u{001f}' => {
                use std::fmt::Write as _;
                write!(StringByteWriter(bytes), "\\u{:04x}", u32::from(character))
                    .expect("writing to a byte vector cannot fail");
            }
            _ => {
                let mut encoded = [0; 4];
                bytes.extend_from_slice(character.encode_utf8(&mut encoded).as_bytes());
            }
        }
    }
    bytes.push(b'"');
}

struct StringByteWriter<'a>(&'a mut Vec<u8>);

impl fmt::Write for StringByteWriter<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.0.extend_from_slice(value.as_bytes());
        Ok(())
    }
}
