use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::knowledge::{ClaimState, ClaimSubjectKind};
use crate::s4::{WorkspaceClaim, WorkspaceEvidence};
use crate::s4_r5::{
    CompilationPresence, RustMethodContext, RustSemanticAttribute, RustSemanticDepthExtraction,
    RustSemanticEntity, RustSemanticEntityKind, RustSemanticError, RustSemanticKnowledge,
    RustSemanticProperties, RustSemanticVisibility, deterministic_claim,
};
use crate::s5::{AnalysisCacheEntry, SourceAnalysisRecord};

pub const R10_PROFILE: &str = "rust-cfg-declaration-alternatives-v1";
pub const R10_CONFIGURATION_VERSION: &str = "codenoesis.configuration/v9";
pub const R10_SNAPSHOT_VERSION: &str = "codenoesis.repository-snapshot/v12";
pub const R10_EXTRACTION_CHUNK_VERSION: &str = "codenoesis.extraction-chunk/v9";
pub const R10_GRAPH_VERSION: &str = "codenoesis.knowledge-graph/v9";
pub const R10_ONTOLOGY_VERSION: &str = "codenoesis.ontology/rust/v9";
pub const R10_PIPELINE_VERSION: &str = "codenoesis.pipeline/s4-r10-v1";
pub const R10_EXTRACTION_CONTRACT_VERSION: &str = "codenoesis.extraction/v9";
pub const R10_EXTRACTOR_VERSION: &str = "codenoesis.rust-cfg-alternatives/s4-r10-v1";
pub const R10_INDEX_VERSION: &str = "codenoesis.rust-cfg-alternative-index/v1";
pub const R10_ERROR_VERSION: &str = "codenoesis.error/v17";
pub const R10_QUERY_VERSION: &str = "codenoesis.local-query-result/v7";
pub const R10_PORTABLE_GRAPH_VERSION: &str = "codenoesis.portable-graph/v3";
pub const R10_LOCAL_EXPLORER_VERSION: &str = "codenoesis.local-explorer/v3";

pub const MAX_R10_ALTERNATIVES_PER_METHOD: u64 = 32;
pub const MAX_R10_ALTERNATIVES_PER_SOURCE: u64 = 4_096;
pub const MAX_R10_ALTERNATIVES_PER_SNAPSHOT: u64 = 200_000;
pub const R10_DETERMINISM_PERMUTATIONS: u64 = 50;
pub const R10_PARALLEL_SCHEDULES: u64 = 10;

const ALTERNATIVE_ENTITY_ID_DOMAIN: &str = "codenoesis.entity-id/rust-declaration-alternative/v1";
const ALTERNATIVE_RELATIONSHIP_ID_DOMAIN: &str =
    "codenoesis.relationship-id/rust-declaration-alternative/v1";
pub const HAS_DECLARATION_ALTERNATIVE: &str = "HAS_DECLARATION_ALTERNATIVE";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustDeclarationAlternativeProperties {
    pub declaration_kind: &'static str,
    pub implementation_context: RustMethodContext,
    pub trait_context_id: Option<String>,
    pub visibility: RustSemanticVisibility,
    pub receiver_present: bool,
    pub declared_signature: String,
    pub compilation_presence: CompilationPresence,
    pub declaration_evidence_id: String,
    pub attributes: Vec<RustSemanticAttribute>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustDeclarationAlternative {
    pub id: String,
    pub crate_id: String,
    pub module_path: String,
    pub name: String,
    pub subject_id: String,
    pub source_file_id: String,
    pub properties: RustDeclarationAlternativeProperties,
    pub direct_cfg_evidence_ids: Vec<String>,
}

impl RustDeclarationAlternative {
    /// Materializes one evidence-bound method occurrence without changing its logical R5 ID.
    ///
    /// # Errors
    ///
    /// Returns an identity mismatch when the supplied R5 entity is not a direct-cfg method.
    pub fn from_method(
        repository_identity: &str,
        method: &RustSemanticEntity,
        source_file_id: String,
        declaration_evidence_id: String,
        direct_cfg_evidence_ids: Vec<String>,
    ) -> Result<Self, RustCfgDeclarationAlternativesError> {
        let RustSemanticProperties::Method(properties) = &method.properties else {
            return Err(identity_mismatch(method));
        };
        if method.kind != RustSemanticEntityKind::Method
            || method.compilation_presence != CompilationPresence::ConditionalUnknown
            || direct_cfg_evidence_ids.is_empty()
        {
            return Err(identity_mismatch(method));
        }
        let mut direct_cfg_evidence_ids = direct_cfg_evidence_ids;
        direct_cfg_evidence_ids.sort();
        if !ordered_unique(direct_cfg_evidence_ids.iter().map(String::as_str)) {
            return Err(RustCfgDeclarationAlternativesError::Duplicate {
                logical_method_id: method.id.clone(),
                declaration_evidence_id,
            });
        }
        let attributes = properties
            .attributes
            .iter()
            .filter(|attribute| {
                !direct_cfg_evidence_ids
                    .iter()
                    .any(|identifier| identifier == &attribute.evidence_id)
            })
            .cloned()
            .collect();
        Ok(Self {
            id: declaration_alternative_id(
                repository_identity,
                &method.id,
                &declaration_evidence_id,
            ),
            crate_id: method.crate_id.clone(),
            module_path: method.module_path.clone(),
            name: method.name.clone(),
            subject_id: method.id.clone(),
            source_file_id,
            properties: RustDeclarationAlternativeProperties {
                declaration_kind: RustSemanticEntityKind::Method.as_str(),
                implementation_context: properties.implementation_context,
                trait_context_id: properties.trait_context_id.clone(),
                visibility: method.visibility,
                receiver_present: properties.receiver_present,
                declared_signature: properties.declared_signature.clone(),
                compilation_presence: properties.compilation_presence,
                declaration_evidence_id,
                attributes,
            },
            direct_cfg_evidence_ids,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustDeclarationAlternativeRelationship {
    pub id: String,
    pub source: String,
    pub target: String,
    pub evidence_ids: Vec<String>,
}

impl RustDeclarationAlternativeRelationship {
    #[must_use]
    pub fn new(alternative: &RustDeclarationAlternative) -> Self {
        Self {
            id: declaration_alternative_relationship_id(&alternative.subject_id, &alternative.id),
            source: alternative.subject_id.clone(),
            target: alternative.id.clone(),
            evidence_ids: vec![alternative.properties.declaration_evidence_id.clone()],
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustCfgDeclarationAlternativesSourceChunk {
    pub source_file_id: String,
    pub logical_method_ids: Vec<String>,
    pub alternatives: Vec<RustDeclarationAlternative>,
    pub relationships: Vec<RustDeclarationAlternativeRelationship>,
    pub claims: Vec<WorkspaceClaim>,
}

impl RustCfgDeclarationAlternativesSourceChunk {
    /// Builds the canonical R10 additions for one source.
    ///
    /// # Errors
    ///
    /// Returns a duplicate or limit failure instead of repairing the input.
    pub fn new(
        source_file_id: String,
        mut alternatives: Vec<RustDeclarationAlternative>,
    ) -> Result<Self, RustCfgDeclarationAlternativesError> {
        alternatives.sort_by(|left, right| left.id.cmp(&right.id));
        enforce_limit(
            RustCfgDeclarationAlternativesLimit::AlternativesPerSource,
            alternatives.len(),
        )?;
        if !ordered_unique(alternatives.iter().map(|value| value.id.as_str())) {
            let duplicate = first_duplicate(alternatives.iter().map(|value| value.id.as_str()));
            return Err(RustCfgDeclarationAlternativesError::Duplicate {
                logical_method_id: duplicate.unwrap_or("duplicate").to_owned(),
                declaration_evidence_id: "duplicate".to_owned(),
            });
        }
        let logical_method_ids = alternatives
            .iter()
            .map(|value| value.subject_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let mut relationships = alternatives
            .iter()
            .map(RustDeclarationAlternativeRelationship::new)
            .collect::<Vec<_>>();
        relationships.sort_by(|left, right| left.id.cmp(&right.id));
        let mut claims = alternatives
            .iter()
            .map(|alternative| {
                deterministic_claim(
                    ClaimSubjectKind::Entity,
                    alternative.id.clone(),
                    alternative.properties.declaration_evidence_id.clone(),
                )
            })
            .chain(relationships.iter().map(|relationship| {
                deterministic_claim(
                    ClaimSubjectKind::Relationship,
                    relationship.id.clone(),
                    relationship.evidence_ids[0].clone(),
                )
            }))
            .collect::<Vec<_>>();
        claims.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(Self {
            source_file_id,
            logical_method_ids,
            alternatives,
            relationships,
            claims,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustCfgDeclarationAlternativesIndex {
    pub logical_method_ids: Vec<String>,
    pub alternative_entity_ids: Vec<String>,
    pub alternative_relationship_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustCfgDeclarationAlternativesGraph {
    pub alternatives: Vec<RustDeclarationAlternative>,
    pub relationships: Vec<RustDeclarationAlternativeRelationship>,
    pub claims: Vec<WorkspaceClaim>,
    pub index: RustCfgDeclarationAlternativesIndex,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustCfgDeclarationAlternativesKnowledge {
    pub semantic: RustSemanticKnowledge,
    pub extraction_chunks: Vec<RustCfgDeclarationAlternativesSourceChunk>,
    pub graph: RustCfgDeclarationAlternativesGraph,
}

impl RustCfgDeclarationAlternativesKnowledge {
    /// Aggregates and validates canonical R10 additions over one R5 knowledge graph.
    ///
    /// # Errors
    ///
    /// Returns the first inherited or additive contract failure.
    pub fn new(
        semantic: RustSemanticKnowledge,
        mut extraction_chunks: Vec<RustCfgDeclarationAlternativesSourceChunk>,
    ) -> Result<Self, RustCfgDeclarationAlternativesError> {
        extraction_chunks.sort_by(|left, right| left.source_file_id.cmp(&right.source_file_id));
        let mut alternatives = extraction_chunks
            .iter()
            .flat_map(|chunk| chunk.alternatives.iter().cloned())
            .collect::<Vec<_>>();
        alternatives.sort_by(|left, right| left.id.cmp(&right.id));
        enforce_limit(
            RustCfgDeclarationAlternativesLimit::AlternativesPerSnapshot,
            alternatives.len(),
        )?;
        let mut relationships = extraction_chunks
            .iter()
            .flat_map(|chunk| chunk.relationships.iter().cloned())
            .collect::<Vec<_>>();
        relationships.sort_by(|left, right| left.id.cmp(&right.id));
        let mut claims = extraction_chunks
            .iter()
            .flat_map(|chunk| chunk.claims.iter().cloned())
            .collect::<Vec<_>>();
        claims.sort_by(|left, right| left.id.cmp(&right.id));
        let index = RustCfgDeclarationAlternativesIndex {
            logical_method_ids: alternatives
                .iter()
                .map(|value| value.subject_id.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            alternative_entity_ids: alternatives.iter().map(|value| value.id.clone()).collect(),
            alternative_relationship_ids: relationships
                .iter()
                .map(|value| value.id.clone())
                .collect(),
        };
        let knowledge = Self {
            semantic,
            extraction_chunks,
            graph: RustCfgDeclarationAlternativesGraph {
                alternatives,
                relationships,
                claims,
                index,
            },
        };
        knowledge.validate()?;
        Ok(knowledge)
    }

    /// Validates the complete R10 ontology additions and their R5 source lineage.
    ///
    /// # Errors
    ///
    /// Returns the first inherited, identity, evidence, ordering, or limit failure.
    #[allow(clippy::too_many_lines)]
    pub fn validate(&self) -> Result<(), RustCfgDeclarationAlternativesError> {
        self.semantic
            .validate()
            .map_err(RustCfgDeclarationAlternativesError::Source)?;
        if !ordered_unique(
            self.extraction_chunks
                .iter()
                .map(|chunk| chunk.source_file_id.as_str()),
        ) || !ordered_unique(
            self.graph
                .alternatives
                .iter()
                .map(|value| value.id.as_str()),
        ) || !ordered_unique(
            self.graph
                .relationships
                .iter()
                .map(|value| value.id.as_str()),
        ) || !ordered_unique(self.graph.claims.iter().map(|value| value.id.as_str()))
        {
            return Err(RustCfgDeclarationAlternativesError::ContractInvalid);
        }
        enforce_limit(
            RustCfgDeclarationAlternativesLimit::AlternativesPerSnapshot,
            self.graph.alternatives.len(),
        )?;
        let repository_identity = self
            .semantic
            .manifest
            .workspace
            .knowledge
            .graph
            .repository_identity
            .as_str();
        let methods = self
            .semantic
            .graph
            .entities
            .iter()
            .filter(|entity| entity.kind == RustSemanticEntityKind::Method)
            .map(|entity| (entity.id.as_str(), entity))
            .collect::<BTreeMap<_, _>>();
        let evidence = self
            .semantic
            .graph
            .evidence
            .iter()
            .map(|value| (value.id.as_str(), value))
            .collect::<BTreeMap<_, _>>();
        let mut grouped = BTreeMap::<&str, Vec<&RustDeclarationAlternative>>::new();
        for alternative in &self.graph.alternatives {
            let method = methods
                .get(alternative.subject_id.as_str())
                .ok_or(RustCfgDeclarationAlternativesError::ContractInvalid)?;
            validate_alternative(repository_identity, alternative, method, &evidence)?;
            grouped
                .entry(alternative.subject_id.as_str())
                .or_default()
                .push(alternative);
        }
        for (logical_method_id, alternatives) in &grouped {
            enforce_limit(
                RustCfgDeclarationAlternativesLimit::AlternativesPerLogicalMethod,
                alternatives.len(),
            )?;
            if alternatives.len() < 2 {
                return Err(RustCfgDeclarationAlternativesError::IdentityMismatch {
                    logical_method_id: (*logical_method_id).to_owned(),
                    reason: "single_occurrence",
                });
            }
            validate_group(logical_method_id, alternatives, &evidence)?;
        }
        let alternative_ids = self
            .graph
            .alternatives
            .iter()
            .map(|value| value.id.as_str())
            .collect::<BTreeSet<_>>();
        let relationship_ids = self
            .graph
            .relationships
            .iter()
            .map(|value| value.id.as_str())
            .collect::<BTreeSet<_>>();
        for relationship in &self.graph.relationships {
            if relationship.id
                != declaration_alternative_relationship_id(
                    &relationship.source,
                    &relationship.target,
                )
                || !methods.contains_key(relationship.source.as_str())
                || !alternative_ids.contains(relationship.target.as_str())
                || relationship.evidence_ids.len() != 1
                || !evidence.contains_key(relationship.evidence_ids[0].as_str())
            {
                return Err(RustCfgDeclarationAlternativesError::ContractInvalid);
            }
        }
        for claim in &self.graph.claims {
            let valid_subject = match claim.subject_kind {
                ClaimSubjectKind::Entity => alternative_ids.contains(claim.subject_id.as_str()),
                ClaimSubjectKind::Relationship => {
                    relationship_ids.contains(claim.subject_id.as_str())
                }
            };
            if claim.state != ClaimState::DeterministicFact
                || !valid_subject
                || claim.evidence_ids.len() != 1
                || !evidence.contains_key(claim.evidence_ids[0].as_str())
            {
                return Err(RustCfgDeclarationAlternativesError::ContractInvalid);
            }
        }
        let expected_logical = grouped
            .keys()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>();
        let expected_alternatives = self
            .graph
            .alternatives
            .iter()
            .map(|value| value.id.clone())
            .collect::<Vec<_>>();
        let expected_relationships = self
            .graph
            .relationships
            .iter()
            .map(|value| value.id.clone())
            .collect::<Vec<_>>();
        if self.graph.index.logical_method_ids != expected_logical
            || self.graph.index.alternative_entity_ids != expected_alternatives
            || self.graph.index.alternative_relationship_ids != expected_relationships
        {
            return Err(RustCfgDeclarationAlternativesError::ContractInvalid);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustCfgDeclarationAlternativesExtraction {
    pub knowledge: RustCfgDeclarationAlternativesKnowledge,
    pub cache_entries: Vec<AnalysisCacheEntry>,
    pub source_records: Vec<SourceAnalysisRecord>,
    pub parser_invocation_count: u64,
}

impl RustCfgDeclarationAlternativesExtraction {
    /// Separates the unchanged R5 extraction transport from the additive R10 knowledge.
    ///
    /// # Errors
    ///
    /// Returns an additive contract failure when occurrence groups are invalid.
    pub fn from_r5(
        extraction: RustSemanticDepthExtraction,
        chunks: Vec<RustCfgDeclarationAlternativesSourceChunk>,
    ) -> Result<Self, RustCfgDeclarationAlternativesError> {
        let knowledge = RustCfgDeclarationAlternativesKnowledge::new(extraction.knowledge, chunks)?;
        Ok(Self {
            knowledge,
            cache_entries: extraction.cache_entries,
            source_records: extraction.source_records,
            parser_invocation_count: extraction.parser_invocation_count,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RustCfgDeclarationAlternativesLimit {
    AlternativesPerLogicalMethod,
    AlternativesPerSource,
    AlternativesPerSnapshot,
}

impl RustCfgDeclarationAlternativesLimit {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AlternativesPerLogicalMethod => "alternatives_per_logical_method",
            Self::AlternativesPerSource => "alternatives_per_source",
            Self::AlternativesPerSnapshot => "alternatives_per_snapshot",
        }
    }

    #[must_use]
    pub const fn maximum(self) -> u64 {
        match self {
            Self::AlternativesPerLogicalMethod => MAX_R10_ALTERNATIVES_PER_METHOD,
            Self::AlternativesPerSource => MAX_R10_ALTERNATIVES_PER_SOURCE,
            Self::AlternativesPerSnapshot => MAX_R10_ALTERNATIVES_PER_SNAPSHOT,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RustCfgDeclarationAlternativesError {
    IdentityMismatch {
        logical_method_id: String,
        reason: &'static str,
    },
    Duplicate {
        logical_method_id: String,
        declaration_evidence_id: String,
    },
    Overlap {
        logical_method_id: String,
        first_evidence_id: String,
        second_evidence_id: String,
    },
    CrossSource {
        logical_method_id: String,
    },
    LimitExceeded {
        limit: RustCfgDeclarationAlternativesLimit,
        maximum: u64,
        observed: u64,
    },
    Source(RustSemanticError),
    ContractInvalid,
}

impl Display for RustCfgDeclarationAlternativesError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::IdentityMismatch { .. } => "Rust cfg alternative identity mismatch",
            Self::Duplicate { .. } => "duplicate Rust cfg declaration alternative",
            Self::Overlap { .. } => "overlapping Rust cfg declaration alternatives",
            Self::CrossSource { .. } => "cross-source Rust cfg declaration alternatives",
            Self::LimitExceeded { .. } => "Rust cfg declaration alternative limit exceeded",
            Self::Source(error) => return Display::fmt(error, formatter),
            Self::ContractInvalid => "invalid Rust cfg declaration alternatives contract",
        })
    }
}

impl Error for RustCfgDeclarationAlternativesError {}

#[must_use]
pub fn declaration_alternative_id(
    repository_identity: &str,
    logical_method_id: &str,
    declaration_evidence_id: &str,
) -> String {
    stable_id(
        "urn:codenoesis:entity:blake3:",
        &[
            ALTERNATIVE_ENTITY_ID_DOMAIN,
            repository_identity,
            logical_method_id,
            declaration_evidence_id,
        ],
    )
}

#[must_use]
pub fn declaration_alternative_relationship_id(
    logical_method_id: &str,
    alternative_entity_id: &str,
) -> String {
    stable_id(
        "urn:codenoesis:relationship:blake3:",
        &[
            ALTERNATIVE_RELATIONSHIP_ID_DOMAIN,
            HAS_DECLARATION_ALTERNATIVE,
            logical_method_id,
            alternative_entity_id,
        ],
    )
}

fn validate_alternative(
    repository_identity: &str,
    alternative: &RustDeclarationAlternative,
    method: &RustSemanticEntity,
    evidence: &BTreeMap<&str, &WorkspaceEvidence>,
) -> Result<(), RustCfgDeclarationAlternativesError> {
    let RustSemanticProperties::Method(method_properties) = &method.properties else {
        return Err(identity_mismatch(method));
    };
    if alternative.id
        != declaration_alternative_id(
            repository_identity,
            &alternative.subject_id,
            &alternative.properties.declaration_evidence_id,
        )
        || alternative.properties.declaration_kind != RustSemanticEntityKind::Method.as_str()
        || alternative.crate_id != method.crate_id
        || alternative.module_path != method.module_path
        || alternative.name != method.name
        || alternative.properties.implementation_context != method_properties.implementation_context
        || alternative.properties.trait_context_id != method_properties.trait_context_id
        || alternative.properties.visibility != method.visibility
        || alternative.properties.compilation_presence != CompilationPresence::ConditionalUnknown
        || alternative.direct_cfg_evidence_ids.is_empty()
    {
        return Err(identity_mismatch(method));
    }
    let declaration = evidence
        .get(alternative.properties.declaration_evidence_id.as_str())
        .ok_or(RustCfgDeclarationAlternativesError::ContractInvalid)?;
    if alternative
        .direct_cfg_evidence_ids
        .iter()
        .chain(
            alternative
                .properties
                .attributes
                .iter()
                .map(|value| &value.evidence_id),
        )
        .any(|identifier| {
            evidence.get(identifier.as_str()).is_none_or(|value| {
                value.path != declaration.path || value.blob_oid != declaration.blob_oid
            })
        })
    {
        return Err(RustCfgDeclarationAlternativesError::CrossSource {
            logical_method_id: alternative.subject_id.clone(),
        });
    }
    Ok(())
}

fn validate_group(
    logical_method_id: &str,
    alternatives: &[&RustDeclarationAlternative],
    evidence: &BTreeMap<&str, &WorkspaceEvidence>,
) -> Result<(), RustCfgDeclarationAlternativesError> {
    let first = alternatives[0];
    let first_evidence = evidence
        .get(first.properties.declaration_evidence_id.as_str())
        .ok_or(RustCfgDeclarationAlternativesError::ContractInvalid)?;
    for (index, alternative) in alternatives.iter().enumerate() {
        if alternative.source_file_id != first.source_file_id {
            return Err(RustCfgDeclarationAlternativesError::CrossSource {
                logical_method_id: logical_method_id.to_owned(),
            });
        }
        let current = evidence
            .get(alternative.properties.declaration_evidence_id.as_str())
            .ok_or(RustCfgDeclarationAlternativesError::ContractInvalid)?;
        if current.path != first_evidence.path || current.blob_oid != first_evidence.blob_oid {
            return Err(RustCfgDeclarationAlternativesError::CrossSource {
                logical_method_id: logical_method_id.to_owned(),
            });
        }
        for previous in &alternatives[..index] {
            let previous_evidence = evidence
                .get(previous.properties.declaration_evidence_id.as_str())
                .ok_or(RustCfgDeclarationAlternativesError::ContractInvalid)?;
            if previous.properties.declaration_evidence_id
                == alternative.properties.declaration_evidence_id
            {
                return Err(RustCfgDeclarationAlternativesError::Duplicate {
                    logical_method_id: logical_method_id.to_owned(),
                    declaration_evidence_id: alternative.properties.declaration_evidence_id.clone(),
                });
            }
            if previous_evidence.end_byte > current.start_byte
                && current.end_byte > previous_evidence.start_byte
            {
                return Err(RustCfgDeclarationAlternativesError::Overlap {
                    logical_method_id: logical_method_id.to_owned(),
                    first_evidence_id: previous.properties.declaration_evidence_id.clone(),
                    second_evidence_id: alternative.properties.declaration_evidence_id.clone(),
                });
            }
        }
    }
    Ok(())
}

fn identity_mismatch(method: &RustSemanticEntity) -> RustCfgDeclarationAlternativesError {
    RustCfgDeclarationAlternativesError::IdentityMismatch {
        logical_method_id: method.id.clone(),
        reason: "logical_properties",
    }
}

fn enforce_limit(
    limit: RustCfgDeclarationAlternativesLimit,
    observed: usize,
) -> Result<(), RustCfgDeclarationAlternativesError> {
    let observed = u64::try_from(observed).unwrap_or(u64::MAX);
    if observed > limit.maximum() {
        return Err(RustCfgDeclarationAlternativesError::LimitExceeded {
            limit,
            maximum: limit.maximum(),
            observed: observed.min(limit.maximum().saturating_add(1)),
        });
    }
    Ok(())
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

fn first_duplicate<'a>(values: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    let mut previous = None;
    for value in values {
        if previous == Some(value) {
            return Some(value);
        }
        previous = Some(value);
    }
    None
}

fn stable_id(prefix: &str, components: &[&str]) -> String {
    format!(
        "{prefix}{}",
        blake3::hash(&canonical_string_array(components)).to_hex()
    )
}

fn canonical_string_array(components: &[&str]) -> Vec<u8> {
    let mut bytes = vec![b'['];
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
            '\u{08}' => bytes.extend_from_slice(br"\b"),
            '\u{0c}' => bytes.extend_from_slice(br"\f"),
            '\n' => bytes.extend_from_slice(br"\n"),
            '\r' => bytes.extend_from_slice(br"\r"),
            '\t' => bytes.extend_from_slice(br"\t"),
            character if character <= '\u{1f}' => {
                bytes.extend_from_slice(format!("\\u{:04x}", character as u32).as_bytes());
            }
            character => {
                let mut encoded = [0_u8; 4];
                bytes.extend_from_slice(character.encode_utf8(&mut encoded).as_bytes());
            }
        }
    }
    bytes.push(b'"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pt_fr_ext_013_source_and_snapshot_limit_boundaries_are_exact() {
        for limit in [
            RustCfgDeclarationAlternativesLimit::AlternativesPerSource,
            RustCfgDeclarationAlternativesLimit::AlternativesPerSnapshot,
        ] {
            let maximum = usize::try_from(limit.maximum()).expect("R10 limit fits usize");
            assert_eq!(enforce_limit(limit, maximum), Ok(()));
            assert_eq!(
                enforce_limit(limit, maximum + 1),
                Err(RustCfgDeclarationAlternativesError::LimitExceeded {
                    limit,
                    maximum: limit.maximum(),
                    observed: limit.maximum() + 1,
                })
            );
        }
    }
}
