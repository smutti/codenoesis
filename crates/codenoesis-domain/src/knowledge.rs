use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::{RepositoryIdentity, STANDARD_LOCAL_S1_LIMITS};

pub const ONTOLOGY_VERSION: &str = "codenoesis.ontology/rust/v1";
pub const EXTRACTOR_VERSION: &str = "codenoesis.rust-tree-sitter/s2-v1";
pub const CONTAINMENT_RULE_VERSION: &str = "codenoesis.rule.rust-file-containment/s2-v1";
pub const MAX_S2_ENTITIES: u64 = 100_000;
pub const MAX_S2_RELATIONSHIPS: u64 = 300_000;
pub const MAX_S2_CLAIMS: u64 = 400_000;
pub const MAX_S2_EVIDENCE: u64 = 10_000;
pub const MAX_S2_DIAGNOSTICS: u64 = 20_000;
pub const MAX_S2_COVERAGE_GAPS: u64 = 20_000;
pub const MAX_S2_SUPPORTED_CAPABILITIES: u64 = 64;
pub const MAX_S2_EVIDENCE_IDS: usize = 64;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum EntityKind {
    RustCrate,
    SourceFile,
    RustModule,
    RustStruct,
    RustEnum,
    RustTrait,
    RustTypeAlias,
    RustFunction,
    RustMethod,
    RustSymbolReference,
}

impl EntityKind {
    pub const ALL: [Self; 10] = [
        Self::RustCrate,
        Self::RustEnum,
        Self::RustFunction,
        Self::RustMethod,
        Self::RustModule,
        Self::RustStruct,
        Self::RustSymbolReference,
        Self::RustTrait,
        Self::RustTypeAlias,
        Self::SourceFile,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RustCrate => "rust.crate",
            Self::SourceFile => "source.file",
            Self::RustModule => "rust.module",
            Self::RustStruct => "rust.struct",
            Self::RustEnum => "rust.enum",
            Self::RustTrait => "rust.trait",
            Self::RustTypeAlias => "rust.type_alias",
            Self::RustFunction => "rust.function",
            Self::RustMethod => "rust.method",
            Self::RustSymbolReference => "rust.symbol_reference",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RelationshipKind {
    Contains,
    Defines,
    Implements,
    Imports,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ClaimSubjectKind {
    Entity,
    Relationship,
}

impl ClaimSubjectKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Entity => "entity",
            Self::Relationship => "relationship",
        }
    }
}

impl RelationshipKind {
    pub const ALL: [Self; 4] = [
        Self::Contains,
        Self::Defines,
        Self::Implements,
        Self::Imports,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Contains => "CONTAINS",
            Self::Defines => "DEFINES",
            Self::Implements => "IMPLEMENTS",
            Self::Imports => "IMPORTS",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ClaimState {
    Candidate,
    Confirmed,
    DerivedFact,
    DeterministicFact,
    Rejected,
    ReviewedInference,
    Superseded,
}

impl ClaimState {
    pub const ALL: [Self; 7] = [
        Self::Candidate,
        Self::Confirmed,
        Self::DerivedFact,
        Self::DeterministicFact,
        Self::Rejected,
        Self::ReviewedInference,
        Self::Superseded,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Candidate => "candidate",
            Self::Confirmed => "confirmed",
            Self::DerivedFact => "derived_fact",
            Self::DeterministicFact => "deterministic_fact",
            Self::Rejected => "rejected",
            Self::ReviewedInference => "reviewed_inference",
            Self::Superseded => "superseded",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidClaimTransition {
    pub source: ClaimState,
    pub target: ClaimState,
}

impl Display for InvalidClaimTransition {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid claim transition from {} to {}",
            self.source.as_str(),
            self.target.as_str()
        )
    }
}

impl Error for InvalidClaimTransition {}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ByteSpan {
    pub start: u64,
    pub end: u64,
}

impl ByteSpan {
    #[must_use]
    pub const fn new(start: u64, end: u64) -> Self {
        Self { start, end }
    }

    #[must_use]
    pub const fn is_valid_for(self, byte_length: u64) -> bool {
        self.start < self.end && self.end <= byte_length
    }

    #[must_use]
    pub const fn intersects(self, other: Self) -> bool {
        self.start < other.end && other.start < self.end
    }
}

pub type EntityProperties = BTreeMap<String, String>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnowledgeEntity {
    pub entity_id: String,
    pub kind: EntityKind,
    pub canonical_identity: String,
    pub display_name: String,
    pub properties: EntityProperties,
    pub evidence_ids: Vec<String>,
    pub claim_id: String,
}

impl KnowledgeEntity {
    #[must_use]
    pub fn new(
        repository_identity: &str,
        kind: EntityKind,
        canonical_identity: String,
        display_name: String,
        properties: EntityProperties,
        evidence_ids: Vec<String>,
    ) -> Self {
        let entity_id = stable_entity_id(repository_identity, kind, &canonical_identity);
        let claim_id = stable_claim_id(
            repository_identity,
            ClaimSubjectKind::Entity.as_str(),
            &entity_id,
        );
        Self {
            entity_id,
            kind,
            canonical_identity,
            display_name,
            properties,
            evidence_ids,
            claim_id,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnowledgeRelationship {
    pub relationship_id: String,
    pub kind: RelationshipKind,
    pub source_entity_id: String,
    pub target_entity_id: String,
    pub evidence_ids: Vec<String>,
    pub claim_id: String,
}

impl KnowledgeRelationship {
    #[must_use]
    pub fn new(
        repository_identity: &str,
        kind: RelationshipKind,
        source_entity_id: String,
        target_entity_id: String,
        evidence_ids: Vec<String>,
    ) -> Self {
        let relationship_id = stable_relationship_id(
            repository_identity,
            kind,
            &source_entity_id,
            &target_entity_id,
        );
        let claim_id = stable_claim_id(
            repository_identity,
            ClaimSubjectKind::Relationship.as_str(),
            &relationship_id,
        );
        Self {
            relationship_id,
            kind,
            source_entity_id,
            target_entity_id,
            evidence_ids,
            claim_id,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClaimDerivation {
    Parser {
        extractor_version: String,
        evidence_ids: Vec<String>,
    },
    DeterministicRule {
        rule_version: String,
        input_claim_ids: Vec<String>,
        evidence_ids: Vec<String>,
    },
}

impl ClaimDerivation {
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Parser { .. } => "parser",
            Self::DeterministicRule { .. } => "deterministic_rule",
        }
    }

    #[must_use]
    pub fn evidence_ids(&self) -> &[String] {
        match self {
            Self::Parser { evidence_ids, .. } | Self::DeterministicRule { evidence_ids, .. } => {
                evidence_ids
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnowledgeClaim {
    pub claim_id: String,
    pub subject_kind: ClaimSubjectKind,
    pub subject_id: String,
    pub state: ClaimState,
    pub derivation: ClaimDerivation,
}

impl KnowledgeClaim {
    #[must_use]
    pub fn parser(
        repository_identity: &str,
        subject_kind: ClaimSubjectKind,
        subject_id: String,
        evidence_ids: Vec<String>,
    ) -> Self {
        Self {
            claim_id: stable_claim_id(repository_identity, subject_kind.as_str(), &subject_id),
            subject_kind,
            subject_id,
            state: ClaimState::DeterministicFact,
            derivation: ClaimDerivation::Parser {
                extractor_version: EXTRACTOR_VERSION.to_owned(),
                evidence_ids,
            },
        }
    }

    #[must_use]
    pub fn containment_rule(
        repository_identity: &str,
        relationship_id: String,
        input_claim_ids: Vec<String>,
        evidence_ids: Vec<String>,
    ) -> Self {
        Self {
            claim_id: stable_claim_id(
                repository_identity,
                ClaimSubjectKind::Relationship.as_str(),
                &relationship_id,
            ),
            subject_kind: ClaimSubjectKind::Relationship,
            subject_id: relationship_id,
            state: ClaimState::DerivedFact,
            derivation: ClaimDerivation::DeterministicRule {
                rule_version: CONTAINMENT_RULE_VERSION.to_owned(),
                input_claim_ids,
                evidence_ids,
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceEvidence {
    pub evidence_id: String,
    pub repository_identity: String,
    pub commit_oid: String,
    pub blob_oid: String,
    pub path: String,
    pub span: ByteSpan,
    pub syntax_kind: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtractionDiagnostic {
    pub code: String,
    pub severity: String,
    pub path: String,
    pub span: ByteSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoverageGap {
    pub code: String,
    pub path: String,
    pub span: ByteSpan,
    pub evidence_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtractionCoverage {
    pub supported_capabilities: Vec<String>,
    pub gaps: Vec<CoverageGap>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtractionChunk {
    pub chunk_id: String,
    pub repository_identity: String,
    pub commit_oid: String,
    pub blob_oid: String,
    pub path: String,
    pub byte_length: u64,
    pub entities: Vec<KnowledgeEntity>,
    pub relationships: Vec<KnowledgeRelationship>,
    pub claims: Vec<KnowledgeClaim>,
    pub evidence: Vec<SourceEvidence>,
    pub diagnostics: Vec<ExtractionDiagnostic>,
    pub coverage: ExtractionCoverage,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnowledgeGraph {
    pub repository_identity: String,
    pub commit_oid: String,
    pub entities: Vec<KnowledgeEntity>,
    pub relationships: Vec<KnowledgeRelationship>,
    pub claims: Vec<KnowledgeClaim>,
    pub evidence: Vec<SourceEvidence>,
    pub diagnostics: Vec<ExtractionDiagnostic>,
    pub coverage: ExtractionCoverage,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustKnowledge {
    pub extraction_chunks: Vec<ExtractionChunk>,
    pub graph: KnowledgeGraph,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KnowledgeError {
    InvalidUtf8 {
        path: String,
    },
    UnsupportedCrateShape,
    ParserCancelled {
        path: String,
    },
    MalformedSyntax {
        path: String,
        span: ByteSpan,
    },
    NormalizationCollision {
        kind: EntityKind,
        canonical_identity: String,
        path: String,
        first_span: ByteSpan,
        second_span: ByteSpan,
    },
    ContractInvalid,
    InvalidEntity {
        entity_id: String,
    },
    InvalidRelationship {
        relationship_id: String,
    },
    DanglingReference {
        reference_id: String,
    },
    CardinalityViolation {
        subject_id: String,
    },
    InvalidClaimState {
        claim_id: String,
    },
    InvalidDerivation {
        claim_id: String,
    },
    LimitExceeded {
        limit: &'static str,
        maximum: u64,
        observed: u64,
    },
    GraphLimitExceeded {
        limit: &'static str,
        maximum: u64,
        observed: u64,
    },
}

impl Display for KnowledgeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidUtf8 { .. } => "invalid UTF-8 source",
            Self::UnsupportedCrateShape => "unsupported Rust crate shape",
            Self::ParserCancelled { .. } => "Rust parser cancelled",
            Self::MalformedSyntax { .. } => "malformed Rust syntax",
            Self::NormalizationCollision { .. } => "Rust identifier normalization collision",
            Self::ContractInvalid => "invalid extraction contract",
            Self::InvalidEntity { .. } => "invalid graph entity",
            Self::InvalidRelationship { .. } => "invalid graph relationship",
            Self::DanglingReference { .. } => "dangling graph reference",
            Self::CardinalityViolation { .. } => "graph cardinality violation",
            Self::InvalidClaimState { .. } => "invalid claim state",
            Self::InvalidDerivation { .. } => "invalid claim derivation",
            Self::LimitExceeded { .. } => "extraction limit exceeded",
            Self::GraphLimitExceeded { .. } => "graph limit exceeded",
        })
    }
}

impl Error for KnowledgeError {}

impl ExtractionChunk {
    /// Validates the strict parser output before graph ingestion.
    ///
    /// # Errors
    ///
    /// Returns a typed S2 extraction or graph error for the first invalid
    /// contract element in deterministic order.
    pub fn validate(&self) -> Result<(), KnowledgeError> {
        validate_extraction_limits(self)?;
        if self.chunk_id
            != extraction_chunk_id(
                &self.repository_identity,
                &self.commit_oid,
                &self.blob_oid,
                &self.path,
            )
        {
            return Err(KnowledgeError::ContractInvalid);
        }
        if self
            .relationships
            .iter()
            .any(|relationship| relationship.kind == RelationshipKind::Contains)
        {
            return Err(KnowledgeError::ContractInvalid);
        }
        if self
            .claims
            .iter()
            .any(|claim| claim.state != ClaimState::DeterministicFact)
        {
            return Err(KnowledgeError::ContractInvalid);
        }
        let validation = ValidationInput {
            repository_identity: &self.repository_identity,
            commit_oid: &self.commit_oid,
            entities: &self.entities,
            relationships: &self.relationships,
            claims: &self.claims,
            evidence: &self.evidence,
            diagnostics: &self.diagnostics,
            coverage: &self.coverage,
            byte_length: self.byte_length,
            source: Some((&self.blob_oid, &self.path)),
        };
        validate_common(validation)
            .and_then(|()| {
                validate_claims(
                    &self.repository_identity,
                    &self.entities,
                    &self.relationships,
                    &self.claims,
                    &self.evidence,
                )
            })
            .and_then(|()| validate_parser_derivations(&self.claims))
            .map_err(|error| match error {
                KnowledgeError::LimitExceeded { .. } => error,
                _ => KnowledgeError::ContractInvalid,
            })
    }
}

impl KnowledgeGraph {
    /// Validates ontology, identity, endpoint, cardinality, claim, and
    /// derivation invariants before publication.
    ///
    /// # Errors
    ///
    /// Returns the first stable graph error in canonical subject order.
    pub fn validate(&self) -> Result<(), KnowledgeError> {
        validate_graph_limits(self)?;
        validate_common(ValidationInput {
            repository_identity: &self.repository_identity,
            commit_oid: &self.commit_oid,
            entities: &self.entities,
            relationships: &self.relationships,
            claims: &self.claims,
            evidence: &self.evidence,
            diagnostics: &self.diagnostics,
            coverage: &self.coverage,
            byte_length: STANDARD_LOCAL_S1_LIMITS.single_file_bytes,
            source: None,
        })?;
        validate_cardinalities(&self.entities, &self.relationships)?;
        validate_claims(
            &self.repository_identity,
            &self.entities,
            &self.relationships,
            &self.claims,
            &self.evidence,
        )?;
        validate_derivations(
            &self.entities,
            &self.relationships,
            &self.claims,
            &self.evidence,
        )
    }
}

impl RustKnowledge {
    /// Validates all chunks and the all-or-nothing graph publication.
    ///
    /// # Errors
    ///
    /// Returns the first deterministic S2 contract or graph error.
    pub fn validate(&self) -> Result<(), KnowledgeError> {
        if self.extraction_chunks.len() != 1 {
            return Err(KnowledgeError::ContractInvalid);
        }
        for chunk in &self.extraction_chunks {
            chunk.validate()?;
        }
        self.graph.validate()?;
        let chunk = &self.extraction_chunks[0];
        if chunk.repository_identity != self.graph.repository_identity
            || chunk.commit_oid != self.graph.commit_oid
            || chunk.entities != self.graph.entities
            || chunk.evidence != self.graph.evidence
            || chunk.diagnostics != self.graph.diagnostics
            || chunk.coverage != self.graph.coverage
        {
            return Err(KnowledgeError::ContractInvalid);
        }
        let graph_parser_relationships = self
            .graph
            .relationships
            .iter()
            .filter(|relationship| relationship.kind != RelationshipKind::Contains)
            .collect::<Vec<_>>();
        if chunk.relationships.iter().collect::<Vec<_>>() != graph_parser_relationships {
            return Err(KnowledgeError::ContractInvalid);
        }
        let graph_parser_claims = self
            .graph
            .claims
            .iter()
            .filter(|claim| claim.state == ClaimState::DeterministicFact)
            .collect::<Vec<_>>();
        if chunk.claims.iter().collect::<Vec<_>>() != graph_parser_claims {
            return Err(KnowledgeError::ContractInvalid);
        }
        Ok(())
    }
}

/// Validates one transition in the closed S2 claim-state machine.
///
/// # Errors
///
/// Returns [`InvalidClaimTransition`] for same-state requests and every
/// transition outside the eleven approved edges.
pub const fn validate_claim_transition(
    source: ClaimState,
    target: ClaimState,
) -> Result<(), InvalidClaimTransition> {
    let allowed = matches!(
        (source, target),
        (
            ClaimState::DeterministicFact
                | ClaimState::DerivedFact
                | ClaimState::Confirmed
                | ClaimState::Rejected,
            ClaimState::Superseded
        ) | (
            ClaimState::Candidate,
            ClaimState::ReviewedInference
                | ClaimState::Confirmed
                | ClaimState::Rejected
                | ClaimState::Superseded
        ) | (
            ClaimState::ReviewedInference,
            ClaimState::Confirmed | ClaimState::Rejected | ClaimState::Superseded
        )
    );
    if allowed {
        Ok(())
    } else {
        Err(InvalidClaimTransition { source, target })
    }
}

#[must_use]
pub fn stable_entity_id(
    repository_identity: &str,
    kind: EntityKind,
    canonical_identity: &str,
) -> String {
    stable_id(
        "urn:codenoesis:entity:blake3:",
        &[
            "codenoesis.entity-id/v1",
            repository_identity,
            "rust",
            kind.as_str(),
            canonical_identity,
        ],
    )
}

#[must_use]
pub fn stable_relationship_id(
    repository_identity: &str,
    kind: RelationshipKind,
    source_entity_id: &str,
    target_entity_id: &str,
) -> String {
    stable_id(
        "urn:codenoesis:relationship:blake3:",
        &[
            "codenoesis.relationship-id/v1",
            repository_identity,
            ONTOLOGY_VERSION,
            kind.as_str(),
            source_entity_id,
            target_entity_id,
        ],
    )
}

#[must_use]
pub fn stable_claim_id(repository_identity: &str, subject_kind: &str, subject_id: &str) -> String {
    stable_id(
        "urn:codenoesis:claim:blake3:",
        &[
            "codenoesis.claim-id/v1",
            repository_identity,
            ONTOLOGY_VERSION,
            subject_kind,
            subject_id,
        ],
    )
}

#[must_use]
pub fn extraction_chunk_id(
    repository_identity: &str,
    commit_oid: &str,
    blob_oid: &str,
    path: &str,
) -> String {
    stable_id(
        "urn:codenoesis:extraction-chunk:blake3:",
        &[
            "codenoesis.extraction-chunk-id/v1",
            repository_identity,
            commit_oid,
            blob_oid,
            path,
            EXTRACTOR_VERSION,
        ],
    )
}

#[derive(Clone, Copy)]
struct ValidationInput<'a> {
    repository_identity: &'a str,
    commit_oid: &'a str,
    entities: &'a [KnowledgeEntity],
    relationships: &'a [KnowledgeRelationship],
    claims: &'a [KnowledgeClaim],
    evidence: &'a [SourceEvidence],
    diagnostics: &'a [ExtractionDiagnostic],
    coverage: &'a ExtractionCoverage,
    byte_length: u64,
    source: Option<(&'a str, &'a str)>,
}

fn validate_common(input: ValidationInput<'_>) -> Result<(), KnowledgeError> {
    if RepositoryIdentity::parse(input.repository_identity).is_err()
        || !is_git_sha1(input.commit_oid)
    {
        return Err(KnowledgeError::ContractInvalid);
    }
    if !ordered_unique(
        input
            .entities
            .iter()
            .map(|entity| entity.entity_id.as_str()),
    ) || !ordered_unique(
        input
            .relationships
            .iter()
            .map(|relationship| relationship.relationship_id.as_str()),
    ) || !ordered_unique(input.claims.iter().map(|claim| claim.claim_id.as_str()))
    {
        return Err(KnowledgeError::ContractInvalid);
    }

    let evidence_by_id = validate_evidence(
        input.repository_identity,
        input.commit_oid,
        input.evidence,
        input.byte_length,
        input.source,
    )?;
    validate_observations(
        input.diagnostics,
        input.coverage,
        &evidence_by_id,
        input.byte_length,
        input.source.map(|(_, path)| path),
    )?;
    let entity_by_id = input
        .entities
        .iter()
        .map(|entity| (entity.entity_id.as_str(), entity))
        .collect::<BTreeMap<_, _>>();
    if entity_by_id.len() != input.entities.len() {
        return Err(KnowledgeError::ContractInvalid);
    }
    if input.claims.len() != input.entities.len() + input.relationships.len() {
        return Err(KnowledgeError::ContractInvalid);
    }

    validate_entities(input.repository_identity, input.entities, &evidence_by_id)?;
    validate_relationships(
        input.repository_identity,
        input.relationships,
        &entity_by_id,
        &evidence_by_id,
    )
}

fn validate_entities(
    repository_identity: &str,
    entities: &[KnowledgeEntity],
    evidence_by_id: &BTreeMap<&str, &SourceEvidence>,
) -> Result<(), KnowledgeError> {
    for entity in entities {
        if entity.entity_id
            != stable_entity_id(repository_identity, entity.kind, &entity.canonical_identity)
            || entity.claim_id
                != stable_claim_id(
                    repository_identity,
                    ClaimSubjectKind::Entity.as_str(),
                    &entity.entity_id,
                )
            || entity.display_name.is_empty()
            || !valid_entity_properties(entity)
            || !valid_evidence_references(&entity.evidence_ids, evidence_by_id)
        {
            return Err(KnowledgeError::InvalidEntity {
                entity_id: entity.entity_id.clone(),
            });
        }
    }
    Ok(())
}

fn validate_relationships(
    repository_identity: &str,
    relationships: &[KnowledgeRelationship],
    entity_by_id: &BTreeMap<&str, &KnowledgeEntity>,
    evidence_by_id: &BTreeMap<&str, &SourceEvidence>,
) -> Result<(), KnowledgeError> {
    let mut relationship_tuples = BTreeSet::new();
    for relationship in relationships {
        if relationship.relationship_id
            != stable_relationship_id(
                repository_identity,
                relationship.kind,
                &relationship.source_entity_id,
                &relationship.target_entity_id,
            )
            || relationship.claim_id
                != stable_claim_id(
                    repository_identity,
                    ClaimSubjectKind::Relationship.as_str(),
                    &relationship.relationship_id,
                )
            || !valid_evidence_references(&relationship.evidence_ids, evidence_by_id)
        {
            return Err(KnowledgeError::InvalidRelationship {
                relationship_id: relationship.relationship_id.clone(),
            });
        }
        let Some(source_entity) = entity_by_id.get(relationship.source_entity_id.as_str()) else {
            return Err(KnowledgeError::DanglingReference {
                reference_id: relationship.source_entity_id.clone(),
            });
        };
        let Some(target_entity) = entity_by_id.get(relationship.target_entity_id.as_str()) else {
            return Err(KnowledgeError::DanglingReference {
                reference_id: relationship.target_entity_id.clone(),
            });
        };
        if !relationship_endpoint_allowed(relationship.kind, source_entity.kind, target_entity.kind)
            || !relationship_tuples.insert((
                relationship.kind,
                relationship.source_entity_id.as_str(),
                relationship.target_entity_id.as_str(),
            ))
        {
            return Err(KnowledgeError::InvalidRelationship {
                relationship_id: relationship.relationship_id.clone(),
            });
        }
    }
    Ok(())
}

fn validate_claims(
    repository_identity: &str,
    entities: &[KnowledgeEntity],
    relationships: &[KnowledgeRelationship],
    claims: &[KnowledgeClaim],
    evidence: &[SourceEvidence],
) -> Result<(), KnowledgeError> {
    let entity_by_id = entities
        .iter()
        .map(|entity| (entity.entity_id.as_str(), entity))
        .collect::<BTreeMap<_, _>>();
    let relationship_by_id = relationships
        .iter()
        .map(|relationship| (relationship.relationship_id.as_str(), relationship))
        .collect::<BTreeMap<_, _>>();
    let evidence_by_id = evidence
        .iter()
        .map(|item| (item.evidence_id.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    for claim in claims {
        if claim.claim_id
            != stable_claim_id(
                repository_identity,
                claim.subject_kind.as_str(),
                &claim.subject_id,
            )
        {
            return Err(KnowledgeError::InvalidClaimState {
                claim_id: claim.claim_id.clone(),
            });
        }
        if !valid_evidence_references(claim.derivation.evidence_ids(), &evidence_by_id) {
            return Err(KnowledgeError::InvalidDerivation {
                claim_id: claim.claim_id.clone(),
            });
        }
        let subject = match claim.subject_kind {
            ClaimSubjectKind::Entity => entity_by_id
                .get(claim.subject_id.as_str())
                .map(|entity| (entity.claim_id.as_str(), entity.evidence_ids.as_slice())),
            ClaimSubjectKind::Relationship => relationship_by_id
                .get(claim.subject_id.as_str())
                .map(|relationship| {
                    (
                        relationship.claim_id.as_str(),
                        relationship.evidence_ids.as_slice(),
                    )
                }),
        };
        let Some((subject_claim_id, subject_evidence_ids)) = subject else {
            return Err(KnowledgeError::DanglingReference {
                reference_id: claim.subject_id.clone(),
            });
        };
        if subject_claim_id != claim.claim_id {
            return Err(KnowledgeError::InvalidClaimState {
                claim_id: claim.claim_id.clone(),
            });
        }
        if subject_evidence_ids != claim.derivation.evidence_ids() {
            return Err(KnowledgeError::InvalidDerivation {
                claim_id: claim.claim_id.clone(),
            });
        }
    }
    Ok(())
}

fn validate_parser_derivations(claims: &[KnowledgeClaim]) -> Result<(), KnowledgeError> {
    for claim in claims {
        if !matches!(
            (&claim.state, &claim.derivation),
            (
                ClaimState::DeterministicFact,
                ClaimDerivation::Parser {
                    extractor_version,
                    ..
                }
            ) if extractor_version == EXTRACTOR_VERSION
        ) {
            return Err(KnowledgeError::InvalidDerivation {
                claim_id: claim.claim_id.clone(),
            });
        }
    }
    Ok(())
}

fn validate_evidence<'a>(
    repository_identity: &str,
    commit_oid: &str,
    evidence: &'a [SourceEvidence],
    byte_length: u64,
    source: Option<(&str, &str)>,
) -> Result<BTreeMap<&'a str, &'a SourceEvidence>, KnowledgeError> {
    for pair in evidence.windows(2) {
        let left = &pair[0];
        let right = &pair[1];
        let left_key = (
            left.path.as_bytes(),
            left.span.start,
            left.span.end,
            left.evidence_id.as_str(),
        );
        let right_key = (
            right.path.as_bytes(),
            right.span.start,
            right.span.end,
            right.evidence_id.as_str(),
        );
        if left_key >= right_key {
            return Err(KnowledgeError::ContractInvalid);
        }
    }
    let evidence_by_id = evidence
        .iter()
        .map(|item| (item.evidence_id.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    if evidence_by_id.len() != evidence.len() {
        return Err(KnowledgeError::ContractInvalid);
    }
    for item in evidence {
        if !valid_evidence_id(&item.evidence_id)
            || item.repository_identity != repository_identity
            || item.commit_oid != commit_oid
            || !is_git_sha1(&item.commit_oid)
            || !is_git_sha1(&item.blob_oid)
            || !valid_canonical_path(&item.path)
            || !valid_syntax_kind(&item.syntax_kind)
            || !item.span.is_valid_for(byte_length)
        {
            return Err(KnowledgeError::ContractInvalid);
        }
        if let Some((blob_oid, path)) = source
            && (item.blob_oid != blob_oid || item.path != path)
        {
            return Err(KnowledgeError::ContractInvalid);
        }
    }
    Ok(evidence_by_id)
}

fn valid_entity_properties(entity: &KnowledgeEntity) -> bool {
    if !valid_bounded_text(&entity.canonical_identity, 2_048)
        || !valid_bounded_text(&entity.display_name, 1_024)
    {
        return false;
    }
    let required = match entity.kind {
        EntityKind::RustCrate => &["crate_root"][..],
        EntityKind::SourceFile => &["path"][..],
        EntityKind::RustModule
        | EntityKind::RustStruct
        | EntityKind::RustEnum
        | EntityKind::RustTrait
        | EntityKind::RustTypeAlias
        | EntityKind::RustFunction => &["visibility"][..],
        EntityKind::RustMethod => &["owner_kind", "visibility"][..],
        EntityKind::RustSymbolReference => &["resolution", "symbol_path"][..],
    };
    if entity.properties.len() != required.len()
        || !required
            .iter()
            .all(|key| entity.properties.contains_key(*key))
    {
        return false;
    }
    if entity.kind == EntityKind::RustSymbolReference {
        return entity
            .properties
            .get("resolution")
            .is_some_and(|value| value == "unresolved")
            && entity
                .properties
                .get("symbol_path")
                .is_some_and(|value| valid_bounded_text(value, 2_048));
    }
    match entity.kind {
        EntityKind::RustCrate => entity
            .properties
            .get("crate_root")
            .is_some_and(|value| valid_canonical_path(value)),
        EntityKind::SourceFile => entity
            .properties
            .get("path")
            .is_some_and(|value| valid_canonical_path(value)),
        EntityKind::RustModule
        | EntityKind::RustStruct
        | EntityKind::RustEnum
        | EntityKind::RustTrait
        | EntityKind::RustTypeAlias
        | EntityKind::RustFunction => entity
            .properties
            .get("visibility")
            .is_some_and(|value| valid_visibility(value)),
        EntityKind::RustMethod => {
            entity
                .properties
                .get("visibility")
                .is_some_and(|value| valid_visibility(value))
                && entity.properties.get("owner_kind").is_some_and(|value| {
                    matches!(value.as_str(), "rust.enum" | "rust.struct" | "rust.trait")
                })
        }
        EntityKind::RustSymbolReference => true,
    }
}

fn valid_evidence_references(
    evidence_ids: &[String],
    evidence_by_id: &BTreeMap<&str, &SourceEvidence>,
) -> bool {
    !evidence_ids.is_empty()
        && evidence_ids.len() <= MAX_S2_EVIDENCE_IDS
        && ordered_unique(evidence_ids.iter().map(String::as_str))
        && evidence_ids
            .iter()
            .all(|evidence_id| evidence_by_id.contains_key(evidence_id.as_str()))
}

const fn relationship_endpoint_allowed(
    relationship: RelationshipKind,
    source: EntityKind,
    target: EntityKind,
) -> bool {
    match relationship {
        RelationshipKind::Contains => {
            matches!(
                (source, target),
                (EntityKind::RustCrate, EntityKind::SourceFile)
            )
        }
        RelationshipKind::Defines => {
            matches!(
                (source, target),
                (
                    EntityKind::RustCrate | EntityKind::RustModule,
                    EntityKind::RustModule
                        | EntityKind::RustStruct
                        | EntityKind::RustEnum
                        | EntityKind::RustTrait
                        | EntityKind::RustTypeAlias
                        | EntityKind::RustFunction
                ) | (
                    EntityKind::RustStruct | EntityKind::RustEnum | EntityKind::RustTrait,
                    EntityKind::RustMethod
                )
            )
        }
        RelationshipKind::Imports => {
            matches!(
                (source, target),
                (
                    EntityKind::RustCrate | EntityKind::RustModule,
                    EntityKind::RustModule
                        | EntityKind::RustStruct
                        | EntityKind::RustEnum
                        | EntityKind::RustTrait
                        | EntityKind::RustTypeAlias
                        | EntityKind::RustFunction
                        | EntityKind::RustSymbolReference
                )
            )
        }
        RelationshipKind::Implements => {
            matches!(
                (source, target),
                (
                    EntityKind::RustStruct | EntityKind::RustEnum,
                    EntityKind::RustTrait | EntityKind::RustSymbolReference
                )
            )
        }
    }
}

fn validate_cardinalities(
    entities: &[KnowledgeEntity],
    relationships: &[KnowledgeRelationship],
) -> Result<(), KnowledgeError> {
    let crate_ids = entities
        .iter()
        .filter(|entity| entity.kind == EntityKind::RustCrate)
        .map(|entity| entity.entity_id.as_str())
        .collect::<Vec<_>>();
    if crate_ids.len() != 1 {
        return Err(KnowledgeError::CardinalityViolation {
            subject_id: "rust.crate".to_owned(),
        });
    }

    let mut incoming = BTreeMap::<(&str, RelationshipKind), usize>::new();
    let mut outgoing = BTreeMap::<&str, usize>::new();
    for relationship in relationships {
        *incoming
            .entry((relationship.target_entity_id.as_str(), relationship.kind))
            .or_default() += 1;
        *outgoing
            .entry(relationship.source_entity_id.as_str())
            .or_default() += 1;
    }

    for entity in entities {
        if !valid_entity_cardinality(entity, &incoming, &outgoing) {
            return Err(KnowledgeError::CardinalityViolation {
                subject_id: entity.entity_id.clone(),
            });
        }
    }
    validate_defines_reachability(entities, relationships, crate_ids[0])
}

fn valid_entity_cardinality(
    entity: &KnowledgeEntity,
    incoming: &BTreeMap<(&str, RelationshipKind), usize>,
    outgoing: &BTreeMap<&str, usize>,
) -> bool {
    let incoming_count = |kind| {
        incoming
            .get(&(entity.entity_id.as_str(), kind))
            .copied()
            .unwrap_or_default()
    };
    match entity.kind {
        EntityKind::SourceFile => incoming_count(RelationshipKind::Contains) == 1,
        EntityKind::RustModule
        | EntityKind::RustStruct
        | EntityKind::RustEnum
        | EntityKind::RustTrait
        | EntityKind::RustTypeAlias
        | EntityKind::RustFunction
        | EntityKind::RustMethod => incoming_count(RelationshipKind::Defines) == 1,
        EntityKind::RustSymbolReference => {
            outgoing
                .get(entity.entity_id.as_str())
                .copied()
                .unwrap_or_default()
                == 0
                && incoming_count(RelationshipKind::Imports)
                    + incoming_count(RelationshipKind::Implements)
                    >= 1
        }
        EntityKind::RustCrate => true,
    }
}

fn validate_defines_reachability(
    entities: &[KnowledgeEntity],
    relationships: &[KnowledgeRelationship],
    crate_id: &str,
) -> Result<(), KnowledgeError> {
    let mut defines = BTreeMap::<&str, Vec<&str>>::new();
    for relationship in relationships {
        if relationship.kind == RelationshipKind::Defines {
            defines
                .entry(relationship.source_entity_id.as_str())
                .or_default()
                .push(relationship.target_entity_id.as_str());
        }
    }
    let mut reachable = BTreeSet::new();
    let mut frontier = vec![crate_id];
    while let Some(parent) = frontier.pop() {
        if !reachable.insert(parent) {
            return Err(KnowledgeError::CardinalityViolation {
                subject_id: parent.to_owned(),
            });
        }
        if let Some(children) = defines.get(parent) {
            frontier.extend(children.iter().copied());
        }
    }
    for entity in entities {
        if matches!(
            entity.kind,
            EntityKind::RustModule
                | EntityKind::RustStruct
                | EntityKind::RustEnum
                | EntityKind::RustTrait
                | EntityKind::RustTypeAlias
                | EntityKind::RustFunction
                | EntityKind::RustMethod
        ) && !reachable.contains(entity.entity_id.as_str())
        {
            return Err(KnowledgeError::CardinalityViolation {
                subject_id: entity.entity_id.clone(),
            });
        }
    }
    Ok(())
}

fn validate_derivations(
    entities: &[KnowledgeEntity],
    relationships: &[KnowledgeRelationship],
    claims: &[KnowledgeClaim],
    evidence: &[SourceEvidence],
) -> Result<(), KnowledgeError> {
    let evidence_ids = evidence
        .iter()
        .map(|item| item.evidence_id.as_str())
        .collect::<BTreeSet<_>>();
    let claims_by_id = claims
        .iter()
        .map(|claim| (claim.claim_id.as_str(), claim))
        .collect::<BTreeMap<_, _>>();
    let entity_claims = entities
        .iter()
        .map(|entity| (entity.entity_id.as_str(), entity.claim_id.as_str()))
        .collect::<BTreeMap<_, _>>();
    let relationships_by_id = relationships
        .iter()
        .map(|relationship| (relationship.relationship_id.as_str(), relationship))
        .collect::<BTreeMap<_, _>>();
    let contains_count = relationships
        .iter()
        .filter(|relationship| relationship.kind == RelationshipKind::Contains)
        .count();
    let mut derived_count = 0;
    for claim in claims {
        match (&claim.state, &claim.derivation) {
            (
                ClaimState::DeterministicFact,
                ClaimDerivation::Parser {
                    extractor_version,
                    evidence_ids: parser_evidence,
                },
            ) if extractor_version == EXTRACTOR_VERSION
                && parser_evidence
                    .iter()
                    .all(|evidence_id| evidence_ids.contains(evidence_id.as_str())) => {}
            (
                ClaimState::DerivedFact,
                ClaimDerivation::DeterministicRule {
                    rule_version,
                    input_claim_ids,
                    evidence_ids: rule_evidence,
                },
            ) => {
                derived_count += 1;
                let expected_inputs = relationships_by_id
                    .get(claim.subject_id.as_str())
                    .filter(|relationship| relationship.kind == RelationshipKind::Contains)
                    .and_then(|relationship| {
                        Some(vec![
                            entity_claims
                                .get(relationship.source_entity_id.as_str())?
                                .to_string(),
                            entity_claims
                                .get(relationship.target_entity_id.as_str())?
                                .to_string(),
                        ])
                    });
                if input_claim_ids.len() != 2
                    || input_claim_ids[0] == input_claim_ids[1]
                    || rule_version != CONTAINMENT_RULE_VERSION
                    || expected_inputs.as_ref() != Some(input_claim_ids)
                    || input_claim_ids.iter().any(|input_id| {
                        claims_by_id
                            .get(input_id.as_str())
                            .is_none_or(|input| input.state != ClaimState::DeterministicFact)
                    })
                    || rule_evidence
                        .iter()
                        .any(|evidence_id| !evidence_ids.contains(evidence_id.as_str()))
                {
                    return Err(KnowledgeError::InvalidDerivation {
                        claim_id: claim.claim_id.clone(),
                    });
                }
            }
            _ => {
                return Err(KnowledgeError::InvalidDerivation {
                    claim_id: claim.claim_id.clone(),
                });
            }
        }
    }
    if derived_count != contains_count || derived_count != 1 {
        return Err(KnowledgeError::InvalidDerivation {
            claim_id: String::new(),
        });
    }
    Ok(())
}

fn validate_extraction_limits(chunk: &ExtractionChunk) -> Result<(), KnowledgeError> {
    if chunk.byte_length == 0 {
        return Err(KnowledgeError::ContractInvalid);
    }
    if chunk.byte_length > STANDARD_LOCAL_S1_LIMITS.single_file_bytes {
        return Err(KnowledgeError::LimitExceeded {
            limit: "single_file_bytes",
            maximum: STANDARD_LOCAL_S1_LIMITS.single_file_bytes,
            observed: capped_observed(
                chunk.byte_length,
                STANDARD_LOCAL_S1_LIMITS.single_file_bytes,
            ),
        });
    }
    check_extraction_count("entities", chunk.entities.len(), MAX_S2_ENTITIES)?;
    check_extraction_count(
        "relationships",
        chunk.relationships.len(),
        MAX_S2_RELATIONSHIPS,
    )?;
    check_extraction_count("claims", chunk.claims.len(), MAX_S2_CLAIMS)?;
    check_extraction_count("evidence_records", chunk.evidence.len(), MAX_S2_EVIDENCE)?;
    check_extraction_count("diagnostics", chunk.diagnostics.len(), MAX_S2_DIAGNOSTICS)?;
    check_extraction_count(
        "coverage_gaps",
        chunk.coverage.gaps.len(),
        MAX_S2_COVERAGE_GAPS,
    )?;
    check_extraction_count(
        "supported_capabilities",
        chunk.coverage.supported_capabilities.len(),
        MAX_S2_SUPPORTED_CAPABILITIES,
    )
}

fn validate_graph_limits(graph: &KnowledgeGraph) -> Result<(), KnowledgeError> {
    check_graph_count("entities", graph.entities.len(), MAX_S2_ENTITIES)?;
    check_graph_count(
        "relationships",
        graph.relationships.len(),
        MAX_S2_RELATIONSHIPS,
    )?;
    check_graph_count("claims", graph.claims.len(), MAX_S2_CLAIMS)?;
    check_graph_count("evidence_records", graph.evidence.len(), MAX_S2_EVIDENCE)?;
    check_graph_count("diagnostics", graph.diagnostics.len(), MAX_S2_DIAGNOSTICS)?;
    check_graph_count(
        "coverage_gaps",
        graph.coverage.gaps.len(),
        MAX_S2_COVERAGE_GAPS,
    )?;
    check_graph_count(
        "supported_capabilities",
        graph.coverage.supported_capabilities.len(),
        MAX_S2_SUPPORTED_CAPABILITIES,
    )
}

fn check_extraction_count(
    limit: &'static str,
    observed: usize,
    maximum: u64,
) -> Result<(), KnowledgeError> {
    let observed = u64::try_from(observed).unwrap_or(maximum + 1);
    if observed > maximum {
        Err(KnowledgeError::LimitExceeded {
            limit,
            maximum,
            observed: capped_observed(observed, maximum),
        })
    } else {
        Ok(())
    }
}

fn check_graph_count(
    limit: &'static str,
    observed: usize,
    maximum: u64,
) -> Result<(), KnowledgeError> {
    let observed = u64::try_from(observed).unwrap_or(maximum + 1);
    if observed > maximum {
        Err(KnowledgeError::GraphLimitExceeded {
            limit,
            maximum,
            observed: capped_observed(observed, maximum),
        })
    } else {
        Ok(())
    }
}

const fn capped_observed(observed: u64, maximum: u64) -> u64 {
    if observed > maximum + 1 {
        maximum + 1
    } else {
        observed
    }
}

fn validate_observations(
    diagnostics: &[ExtractionDiagnostic],
    coverage: &ExtractionCoverage,
    evidence_by_id: &BTreeMap<&str, &SourceEvidence>,
    byte_length: u64,
    source_path: Option<&str>,
) -> Result<(), KnowledgeError> {
    for pair in diagnostics.windows(2) {
        if diagnostic_key(&pair[0]) >= diagnostic_key(&pair[1]) {
            return Err(KnowledgeError::ContractInvalid);
        }
    }
    for diagnostic in diagnostics {
        if !matches!(
            diagnostic.code.as_str(),
            "extraction.malformed_syntax"
                | "extraction.unsupported_construct"
                | "extraction.unresolved_symbol"
        ) || !matches!(diagnostic.severity.as_str(), "warning" | "error")
            || !valid_canonical_path(&diagnostic.path)
            || source_path.is_some_and(|path| diagnostic.path != path)
            || !diagnostic.span.is_valid_for(byte_length)
        {
            return Err(KnowledgeError::ContractInvalid);
        }
    }

    if !ordered_unique(coverage.supported_capabilities.iter().map(String::as_str))
        || coverage
            .supported_capabilities
            .iter()
            .any(|capability| !valid_capability(capability))
    {
        return Err(KnowledgeError::ContractInvalid);
    }
    for pair in coverage.gaps.windows(2) {
        if gap_key(&pair[0]) >= gap_key(&pair[1]) {
            return Err(KnowledgeError::ContractInvalid);
        }
    }
    for gap in &coverage.gaps {
        if !matches!(
            gap.code.as_str(),
            "calls_not_extracted"
                | "fields_not_extracted"
                | "malformed_syntax_excluded"
                | "unsupported_construct"
                | "unresolved_symbol"
                | "variants_not_extracted"
        ) || !valid_canonical_path(&gap.path)
            || source_path.is_some_and(|path| gap.path != path)
            || !gap.span.is_valid_for(byte_length)
            || !evidence_by_id.contains_key(gap.evidence_id.as_str())
        {
            return Err(KnowledgeError::ContractInvalid);
        }
    }

    for diagnostic in diagnostics {
        let expected_gap = match diagnostic.code.as_str() {
            "extraction.malformed_syntax" => "malformed_syntax_excluded",
            "extraction.unsupported_construct" => "unsupported_construct",
            "extraction.unresolved_symbol" => "unresolved_symbol",
            _ => return Err(KnowledgeError::ContractInvalid),
        };
        if coverage
            .gaps
            .iter()
            .filter(|gap| {
                gap.code == expected_gap
                    && gap.path == diagnostic.path
                    && gap.span == diagnostic.span
            })
            .count()
            != 1
        {
            return Err(KnowledgeError::ContractInvalid);
        }
    }
    Ok(())
}

fn diagnostic_key(diagnostic: &ExtractionDiagnostic) -> (&[u8], u64, u64, &str) {
    (
        diagnostic.path.as_bytes(),
        diagnostic.span.start,
        diagnostic.span.end,
        diagnostic.code.as_str(),
    )
}

fn gap_key(gap: &CoverageGap) -> (&[u8], u64, u64, &str) {
    (
        gap.path.as_bytes(),
        gap.span.start,
        gap.span.end,
        gap.code.as_str(),
    )
}

fn valid_evidence_id(value: &str) -> bool {
    value
        .strip_prefix("evidence-s2-")
        .is_some_and(|suffix| suffix.len() == 4 && suffix.bytes().all(|byte| byte.is_ascii_digit()))
}

fn is_git_sha1(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn valid_canonical_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 1_024
        && value.split('/').all(|component| {
            !component.is_empty()
                && component
                    .chars()
                    .all(|character| character != '\\' && !character.is_control())
        })
}

fn valid_bounded_text(value: &str, maximum: usize) -> bool {
    !value.is_empty() && value.chars().count() <= maximum && !value.contains(['\r', '\n', '\0'])
}

fn valid_visibility(value: &str) -> bool {
    matches!(value, "inherited_trait" | "private" | "public")
}

fn valid_syntax_kind(value: &str) -> bool {
    value.len() <= 128
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte == b'_')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn valid_capability(value: &str) -> bool {
    value
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_lowercase())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn ordered_unique<'a>(values: impl IntoIterator<Item = &'a str>) -> bool {
    let mut previous = None;
    for value in values {
        if previous.is_some_and(|previous_value| previous_value >= value) {
            return false;
        }
        previous = Some(value);
    }
    true
}

fn stable_id(prefix: &str, components: &[&str]) -> String {
    let preimage = canonical_string_array(components);
    let digest = blake3::hash(&preimage);
    format!("{prefix}{}", digest.to_hex())
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
            character if character <= '\u{001f}' => {
                use std::fmt::Write as _;

                write!(StringByteWriter(bytes), "\\u{:04x}", u32::from(character))
                    .expect("writing to a byte vector cannot fail");
            }
            character => {
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
