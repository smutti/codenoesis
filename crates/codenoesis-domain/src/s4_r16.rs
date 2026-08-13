use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::knowledge::{ClaimState, ClaimSubjectKind};
use crate::s4::{WorkspaceClaim, workspace_claim_id};
use crate::s4_r15::{LocalFlowError, LocalFlowExtraction, LocalFlowKnowledge};
use crate::s5::AnalysisCacheEntry;

pub const R16_CONFIGURATION_VERSION: &str = "codenoesis.configuration/v15";
pub const R16_SNAPSHOT_VERSION: &str = "codenoesis.repository-snapshot/v18";
pub const R16_EXTRACTION_CHUNK_VERSION: &str = "codenoesis.extraction-chunk/v15";
pub const R16_GRAPH_VERSION: &str = "codenoesis.knowledge-graph/v15";
pub const R16_ONTOLOGY_VERSION: &str = "codenoesis.ontology/rust/v15";
pub const R16_PIPELINE_VERSION: &str = "codenoesis.pipeline/s4-r16-v1";
pub const R16_EXTRACTION_CONTRACT_VERSION: &str = "codenoesis.extraction/v15";
pub const R16_EXTRACTOR_VERSION: &str = "codenoesis.rust-constant-evaluation/s4-r16-v1";
pub const R16_INDEX_VERSION: &str = "codenoesis.constant-evaluation-index/v1";
pub const R16_RULE_VERSION: &str = "codenoesis.rule/rust-safe-constant-evaluation/v1";
pub const R16_SEMANTIC_HASH_CONTRACT_VERSION: &str = "codenoesis.semantic-hash-contract/v14";
pub const R16_ERROR_VERSION: &str = "codenoesis.error/v24";
pub const R16_QUERY_VERSION: &str = "codenoesis.local-query-result/v13";
pub const R16_PORTABLE_GRAPH_VERSION: &str = "codenoesis.portable-graph/v9";
pub const R16_LOCAL_EXPLORER_VERSION: &str = "codenoesis.local-explorer/v9";
pub const R16_PROFILE: &str = "rust-safe-constant-evaluation-v1";

pub const MAX_R16_CANDIDATES_PER_SOURCE: u64 = 4_096;
pub const MAX_R16_SYNTAX_NODES_PER_EXPRESSION: u64 = 256;
pub const MAX_R16_DIRECT_DEPENDENCIES: u64 = 256;
pub const MAX_R16_DEPENDENCY_LEVELS: u64 = 64;
pub const MAX_R16_VARIANTS_PER_ENUM: u64 = 4_096;
pub const MAX_R16_EVALUATED_ENTITIES: u64 = 200_000;
pub const MAX_R16_EVALUATION_RELATIONSHIPS: u64 = 200_000;
pub const MAX_R16_DEPENDENCY_REFERENCES: u64 = 400_000;
pub const MAX_R16_DERIVATION_INPUT_REFERENCES: u64 = 1_000_000;

const ENTITY_ID_DOMAIN: &str = "codenoesis.entity-id/rust-safe-constant-evaluation/v1";
const RELATIONSHIP_ID_DOMAIN: &str = "codenoesis.relationship-id/rust-safe-constant-evaluation/v1";
const COVERAGE_ID_DOMAIN: &str = "codenoesis.coverage-gap-id/rust-safe-constant-evaluation/v1";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ConstantValueKind {
    Boolean,
    Integer,
}

impl ConstantValueKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Boolean => "boolean",
            Self::Integer => "integer",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ConstantTypeAuthority {
    ExplicitPrimitiveAnnotation,
    FixedReprAttribute,
}

impl ConstantTypeAuthority {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExplicitPrimitiveAnnotation => "explicit_primitive_annotation",
            Self::FixedReprAttribute => "fixed_repr_attribute",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvaluatedValue {
    pub id: String,
    pub declared_value_id: String,
    pub value_kind: ConstantValueKind,
    pub canonical_value: String,
    pub rust_type: String,
    pub type_authority: ConstantTypeAuthority,
    pub evidence_ids: Vec<String>,
}

impl EvaluatedValue {
    #[must_use]
    pub fn new(
        repository_identity: &str,
        declared_value_id: String,
        value_kind: ConstantValueKind,
        canonical_value: String,
        rust_type: String,
        type_authority: ConstantTypeAuthority,
        mut evidence_ids: Vec<String>,
    ) -> Self {
        evidence_ids.sort();
        evidence_ids.dedup();
        Self {
            id: evaluated_value_id(repository_identity, &declared_value_id),
            declared_value_id,
            value_kind,
            canonical_value,
            rust_type,
            type_authority,
            evidence_ids,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstantEvaluationRelationship {
    pub id: String,
    pub source: String,
    pub target: String,
    pub evidence_ids: Vec<String>,
}

impl ConstantEvaluationRelationship {
    #[must_use]
    pub fn new(source: String, target: String, mut evidence_ids: Vec<String>) -> Self {
        evidence_ids.sort();
        evidence_ids.dedup();
        Self {
            id: evaluation_relationship_id(&source, &target),
            source,
            target,
            evidence_ids,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstantEvaluationDerivation {
    pub entity_id: String,
    pub relationship_id: String,
    pub input_claim_ids: Vec<String>,
    pub input_evidence_ids: Vec<String>,
    pub dependency_entity_ids: Vec<String>,
}

impl ConstantEvaluationDerivation {
    #[must_use]
    pub fn new(
        entity_id: String,
        relationship_id: String,
        mut input_claim_ids: Vec<String>,
        mut input_evidence_ids: Vec<String>,
        mut dependency_entity_ids: Vec<String>,
    ) -> Self {
        input_claim_ids.sort();
        input_claim_ids.dedup();
        input_evidence_ids.sort();
        input_evidence_ids.dedup();
        dependency_entity_ids.sort();
        dependency_entity_ids.dedup();
        Self {
            entity_id,
            relationship_id,
            input_claim_ids,
            input_evidence_ids,
            dependency_entity_ids,
        }
    }

    #[must_use]
    pub fn input_count(&self) -> usize {
        self.input_claim_ids
            .len()
            .saturating_add(self.input_evidence_ids.len())
            .saturating_add(self.dependency_entity_ids.len())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstantEvaluationCoverageGap {
    pub id: String,
    pub capability: String,
    pub state: String,
    pub subject_id: String,
    pub evidence_ids: Vec<String>,
}

impl ConstantEvaluationCoverageGap {
    #[must_use]
    pub fn not_evaluated(
        capability: &str,
        subject_id: String,
        mut evidence_ids: Vec<String>,
    ) -> Self {
        evidence_ids.sort();
        evidence_ids.dedup();
        let state = "not_evaluated".to_owned();
        let id = stable_id(
            "urn:codenoesis:coverage-gap:blake3:",
            &[
                COVERAGE_ID_DOMAIN,
                capability,
                &state,
                &subject_id,
                &evidence_ids.join("\u{1f}"),
            ],
        );
        Self {
            id,
            capability: capability.to_owned(),
            state,
            subject_id,
            evidence_ids,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ConstantEvaluationSourceOverlay {
    pub source_file_id: String,
    pub entities: Vec<EvaluatedValue>,
    pub relationships: Vec<ConstantEvaluationRelationship>,
    pub claims: Vec<WorkspaceClaim>,
    pub coverage: Vec<ConstantEvaluationCoverageGap>,
    pub removed_coverage_ids: Vec<String>,
    pub removed_diagnostic_ids: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ConstantEvaluationIndex {
    pub evaluated_entity_ids: Vec<String>,
    pub evaluation_relationship_ids: Vec<String>,
    pub derivations: Vec<ConstantEvaluationDerivation>,
}

impl ConstantEvaluationIndex {
    #[must_use]
    pub fn from_graph(
        entities: &[EvaluatedValue],
        relationships: &[ConstantEvaluationRelationship],
        mut derivations: Vec<ConstantEvaluationDerivation>,
    ) -> Self {
        derivations.sort_by(|left, right| left.entity_id.cmp(&right.entity_id));
        Self {
            evaluated_entity_ids: entities.iter().map(|value| value.id.clone()).collect(),
            evaluation_relationship_ids: relationships
                .iter()
                .map(|value| value.id.clone())
                .collect(),
            derivations,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ConstantEvaluationGraph {
    pub entities: Vec<EvaluatedValue>,
    pub relationships: Vec<ConstantEvaluationRelationship>,
    pub claims: Vec<WorkspaceClaim>,
    pub coverage: Vec<ConstantEvaluationCoverageGap>,
    pub removed_coverage_ids: Vec<String>,
    pub removed_diagnostic_ids: Vec<String>,
    pub index: ConstantEvaluationIndex,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstantEvaluationKnowledge {
    pub local_flow: LocalFlowKnowledge,
    pub source_overlays: Vec<ConstantEvaluationSourceOverlay>,
    pub graph: ConstantEvaluationGraph,
}

impl ConstantEvaluationKnowledge {
    /// Validates the exact R15 lineage and additive R16 value graph.
    ///
    /// # Errors
    ///
    /// Returns the first identity, value, reference, derivation, cycle, ordering, or limit
    /// failure.
    #[allow(clippy::too_many_lines)]
    pub fn validate(&self) -> Result<(), ConstantEvaluationError> {
        self.local_flow
            .validate()
            .map_err(ConstantEvaluationError::Source)?;
        ordered_unique(self.graph.entities.iter().map(|value| value.id.as_str()))?;
        ordered_unique(
            self.graph
                .relationships
                .iter()
                .map(|value| value.id.as_str()),
        )?;
        ordered_unique(self.graph.claims.iter().map(|value| value.id.as_str()))?;
        ordered_unique(self.graph.coverage.iter().map(|value| value.id.as_str()))?;
        ordered_unique(self.graph.removed_coverage_ids.iter().map(String::as_str))?;
        ordered_unique(self.graph.removed_diagnostic_ids.iter().map(String::as_str))?;
        enforce_constant_limit(
            ConstantEvaluationLimit::EvaluatedEntities,
            self.graph.entities.len(),
        )?;
        enforce_constant_limit(
            ConstantEvaluationLimit::EvaluationRelationships,
            self.graph.relationships.len(),
        )?;

        let repository_identity = self
            .local_flow
            .expression
            .callable
            .framework
            .semantic
            .manifest
            .workspace
            .knowledge
            .graph
            .repository_identity
            .as_str();
        let declared_values = self
            .local_flow
            .expression
            .callable
            .graph
            .entities
            .iter()
            .filter(|entity| entity.kind == crate::s4_k1::CallableSemanticEntityKind::DeclaredValue)
            .map(|entity| (entity.id.as_str(), entity))
            .collect::<BTreeMap<_, _>>();
        let semantic_entities = self
            .local_flow
            .expression
            .callable
            .framework
            .semantic
            .graph
            .entities
            .iter()
            .map(|entity| (entity.id.as_str(), entity))
            .collect::<BTreeMap<_, _>>();
        let inherited_claims = self
            .local_flow
            .expression
            .callable
            .framework
            .semantic
            .graph
            .claims
            .iter()
            .chain(&self.local_flow.expression.callable.graph.claims)
            .chain(&self.local_flow.expression.graph.claims)
            .chain(&self.local_flow.graph.claims)
            .map(|claim| (claim.id.as_str(), claim))
            .collect::<BTreeMap<_, _>>();
        let inherited_evidence = self
            .local_flow
            .expression
            .callable
            .framework
            .semantic
            .graph
            .evidence
            .iter()
            .chain(&self.local_flow.expression.callable.graph.evidence)
            .chain(&self.local_flow.expression.graph.evidence)
            .chain(&self.local_flow.graph.evidence)
            .map(|evidence| evidence.id.as_str())
            .collect::<BTreeSet<_>>();
        let entity_ids = self
            .graph
            .entities
            .iter()
            .map(|entity| entity.id.as_str())
            .collect::<BTreeSet<_>>();
        let relationship_ids = self
            .graph
            .relationships
            .iter()
            .map(|relationship| relationship.id.as_str())
            .collect::<BTreeSet<_>>();
        let entity_by_id = self
            .graph
            .entities
            .iter()
            .map(|entity| (entity.id.as_str(), entity))
            .collect::<BTreeMap<_, _>>();
        let relationship_by_id = self
            .graph
            .relationships
            .iter()
            .map(|relationship| (relationship.id.as_str(), relationship))
            .collect::<BTreeMap<_, _>>();
        let mut declared_seen = BTreeSet::new();
        for entity in &self.graph.entities {
            let declared = declared_values
                .get(entity.declared_value_id.as_str())
                .copied();
            let semantic = declared
                .and_then(|declared| semantic_entities.get(declared.subject_id.as_str()).copied());
            if !declared_seen.insert(entity.declared_value_id.as_str())
                || declared.is_none()
                || semantic.is_none_or(|semantic| !valid_type_authority(entity, semantic))
                || entity.id != evaluated_value_id(repository_identity, &entity.declared_value_id)
                || entity.evidence_ids.is_empty()
                || !strictly_ordered(&entity.evidence_ids)
                || entity
                    .evidence_ids
                    .iter()
                    .any(|identifier| !inherited_evidence.contains(identifier.as_str()))
                || !valid_value(entity)
            {
                return Err(ConstantEvaluationError::ValueInvalid);
            }
        }
        if self.graph.relationships.len() != self.graph.entities.len() {
            return Err(ConstantEvaluationError::ValueInvalid);
        }
        let mut relationship_targets = BTreeSet::new();
        for relationship in &self.graph.relationships {
            if relationship.id
                != evaluation_relationship_id(&relationship.source, &relationship.target)
                || !declared_values.contains_key(relationship.source.as_str())
                || !entity_ids.contains(relationship.target.as_str())
                || !relationship_targets.insert(relationship.target.as_str())
                || relationship.evidence_ids.is_empty()
                || !strictly_ordered(&relationship.evidence_ids)
                || relationship
                    .evidence_ids
                    .iter()
                    .any(|identifier| !inherited_evidence.contains(identifier.as_str()))
                || entity_by_id
                    .get(relationship.target.as_str())
                    .is_none_or(|entity| {
                        entity.declared_value_id != relationship.source
                            || entity.evidence_ids != relationship.evidence_ids
                    })
            {
                return Err(ConstantEvaluationError::ValueInvalid);
            }
        }
        let subjects = entity_ids
            .iter()
            .map(|identifier| (ClaimSubjectKind::Entity, *identifier))
            .chain(
                relationship_ids
                    .iter()
                    .map(|identifier| (ClaimSubjectKind::Relationship, *identifier)),
            )
            .collect::<BTreeSet<_>>();
        if self.graph.claims.len() != subjects.len() {
            return Err(ConstantEvaluationError::DerivationMismatch);
        }
        for claim in &self.graph.claims {
            let evidence = match claim.subject_kind {
                ClaimSubjectKind::Entity => entity_by_id
                    .get(claim.subject_id.as_str())
                    .map(|entity| &entity.evidence_ids),
                ClaimSubjectKind::Relationship => relationship_by_id
                    .get(claim.subject_id.as_str())
                    .map(|relationship| &relationship.evidence_ids),
            };
            if claim.state != ClaimState::DerivedFact
                || claim.id
                    != workspace_claim_id(claim.subject_kind, &claim.subject_id, claim.state)
                || evidence != Some(&claim.evidence_ids)
            {
                return Err(ConstantEvaluationError::DerivationMismatch);
            }
        }

        if self.graph.index.derivations.len() != self.graph.entities.len() {
            return Err(ConstantEvaluationError::DerivationMismatch);
        }
        let inherited_claim_by_subject = inherited_claims
            .values()
            .map(|claim| (claim.subject_id.as_str(), *claim))
            .collect::<BTreeMap<_, _>>();
        let mut dependency_references = 0_usize;
        let mut derivation_inputs = 0_usize;
        let mut adjacency = BTreeMap::<&str, Vec<&str>>::new();
        let mut previous_derivation = None;
        let mut represented_relationships = BTreeSet::new();
        for derivation in &self.graph.index.derivations {
            if previous_derivation.is_some_and(|previous| previous >= derivation.entity_id.as_str())
            {
                return Err(ConstantEvaluationError::DerivationMismatch);
            }
            previous_derivation = Some(derivation.entity_id.as_str());
            dependency_references =
                dependency_references.saturating_add(derivation.dependency_entity_ids.len());
            derivation_inputs = derivation_inputs.saturating_add(derivation.input_count());
            let entity = entity_by_id
                .get(derivation.entity_id.as_str())
                .copied()
                .ok_or(ConstantEvaluationError::DerivationMismatch)?;
            let relationship = relationship_by_id
                .get(derivation.relationship_id.as_str())
                .copied()
                .ok_or(ConstantEvaluationError::DerivationMismatch)?;
            if derivation
                .dependency_entity_ids
                .iter()
                .any(|identifier| !entity_ids.contains(identifier.as_str()))
            {
                return Err(ConstantEvaluationError::DependencyInvalid);
            }
            enforce_constant_limit(
                ConstantEvaluationLimit::DirectDependencies,
                derivation.dependency_entity_ids.len(),
            )?;
            let declared_claim = inherited_claim_by_subject
                .get(entity.declared_value_id.as_str())
                .copied()
                .ok_or(ConstantEvaluationError::DerivationMismatch)?;
            let mut expected_input_claim_ids = vec![declared_claim.id.clone()];
            expected_input_claim_ids.extend(derivation.dependency_entity_ids.iter().map(
                |identifier| {
                    workspace_claim_id(
                        ClaimSubjectKind::Entity,
                        identifier,
                        ClaimState::DerivedFact,
                    )
                },
            ));
            expected_input_claim_ids.sort();
            expected_input_claim_ids.dedup();
            if relationship.target != derivation.entity_id
                || relationship.source != entity.declared_value_id
                || !represented_relationships.insert(relationship.id.as_str())
                || derivation.input_claim_ids.is_empty()
                || derivation.input_claim_ids != expected_input_claim_ids
                || derivation.input_evidence_ids != entity.evidence_ids
                || derivation.input_claim_ids.iter().any(|identifier| {
                    !inherited_claims.contains_key(identifier.as_str())
                        && !self
                            .graph
                            .claims
                            .iter()
                            .any(|claim| claim.id == *identifier)
                })
                || !strictly_ordered(&derivation.input_claim_ids)
                || !strictly_ordered(&derivation.input_evidence_ids)
                || !strictly_ordered(&derivation.dependency_entity_ids)
            {
                return Err(ConstantEvaluationError::DerivationMismatch);
            }
            adjacency.insert(
                derivation.entity_id.as_str(),
                derivation
                    .dependency_entity_ids
                    .iter()
                    .map(String::as_str)
                    .collect(),
            );
        }
        enforce_constant_limit(
            ConstantEvaluationLimit::DependencyReferences,
            dependency_references,
        )?;
        enforce_constant_limit(
            ConstantEvaluationLimit::DerivationInputReferences,
            derivation_inputs,
        )?;
        validate_acyclic(&adjacency)?;
        if self.graph.index
            != ConstantEvaluationIndex::from_graph(
                &self.graph.entities,
                &self.graph.relationships,
                self.graph.index.derivations.clone(),
            )
        {
            return Err(ConstantEvaluationError::IndexMismatch);
        }
        validate_overlay_union(self)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstantEvaluationExtraction {
    pub knowledge: ConstantEvaluationKnowledge,
    pub cache_entries: Vec<AnalysisCacheEntry>,
    pub parser_invocation_count: u64,
}

impl ConstantEvaluationExtraction {
    #[must_use]
    pub fn from_r15(
        source: LocalFlowExtraction,
        source_overlays: Vec<ConstantEvaluationSourceOverlay>,
        graph: ConstantEvaluationGraph,
        parser_invocation_count: u64,
    ) -> Self {
        Self {
            knowledge: ConstantEvaluationKnowledge {
                local_flow: source.knowledge,
                source_overlays,
                graph,
            },
            cache_entries: source.cache_entries,
            parser_invocation_count,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstantEvaluationLimit {
    CandidatesPerSource,
    SyntaxNodesPerExpression,
    DirectDependencies,
    DependencyLevels,
    VariantsPerEnum,
    EvaluatedEntities,
    EvaluationRelationships,
    DependencyReferences,
    DerivationInputReferences,
}

impl ConstantEvaluationLimit {
    #[must_use]
    pub const fn maximum(self) -> u64 {
        match self {
            Self::CandidatesPerSource => MAX_R16_CANDIDATES_PER_SOURCE,
            Self::SyntaxNodesPerExpression => MAX_R16_SYNTAX_NODES_PER_EXPRESSION,
            Self::DirectDependencies => MAX_R16_DIRECT_DEPENDENCIES,
            Self::DependencyLevels => MAX_R16_DEPENDENCY_LEVELS,
            Self::VariantsPerEnum => MAX_R16_VARIANTS_PER_ENUM,
            Self::EvaluatedEntities => MAX_R16_EVALUATED_ENTITIES,
            Self::EvaluationRelationships => MAX_R16_EVALUATION_RELATIONSHIPS,
            Self::DependencyReferences => MAX_R16_DEPENDENCY_REFERENCES,
            Self::DerivationInputReferences => MAX_R16_DERIVATION_INPUT_REFERENCES,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CandidatesPerSource => "candidate_declared_values_per_source",
            Self::SyntaxNodesPerExpression => "syntax_nodes_per_expression",
            Self::DirectDependencies => "direct_dependencies_per_subject",
            Self::DependencyLevels => "dependency_levels",
            Self::VariantsPerEnum => "variants_per_enum",
            Self::EvaluatedEntities => "evaluated_entities_total",
            Self::EvaluationRelationships => "evaluation_relationships_total",
            Self::DependencyReferences => "evaluation_dependency_references",
            Self::DerivationInputReferences => "derivation_input_references",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConstantEvaluationError {
    Source(LocalFlowError),
    IdentityConflict,
    ValueInvalid,
    DependencyInvalid,
    DependencyCycle,
    DerivationMismatch,
    IndexMismatch,
    LimitExceeded {
        limit: ConstantEvaluationLimit,
        maximum: u64,
        observed: u64,
    },
    ContractInvalid,
}

impl Display for ConstantEvaluationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Source(_) => "R16 local-flow source extraction failed",
            Self::IdentityConflict => "R16 constant identity conflict",
            Self::ValueInvalid => "R16 evaluated value is invalid",
            Self::DependencyInvalid => "R16 constant dependency is invalid",
            Self::DependencyCycle => "R16 constant dependency cycle",
            Self::DerivationMismatch => "R16 constant derivation mismatch",
            Self::IndexMismatch => "R16 constant index mismatch",
            Self::LimitExceeded { .. } => "R16 constant-evaluation limit exceeded",
            Self::ContractInvalid => "R16 constant-evaluation contract is invalid",
        })
    }
}

impl Error for ConstantEvaluationError {}

#[must_use]
pub fn evaluated_value_id(repository_identity: &str, declared_value_id: &str) -> String {
    stable_id(
        "urn:codenoesis:entity:blake3:",
        &[
            ENTITY_ID_DOMAIN,
            "evaluated_value",
            repository_identity,
            declared_value_id,
        ],
    )
}

#[must_use]
pub fn evaluation_relationship_id(declared_value_id: &str, evaluated_value_id: &str) -> String {
    stable_id(
        "urn:codenoesis:relationship:blake3:",
        &[
            RELATIONSHIP_ID_DOMAIN,
            "EVALUATES_TO",
            declared_value_id,
            evaluated_value_id,
        ],
    )
}

#[must_use]
pub fn constant_evaluation_claim(
    subject_kind: ClaimSubjectKind,
    subject_id: String,
    evidence_ids: Vec<String>,
) -> WorkspaceClaim {
    WorkspaceClaim::new(
        subject_kind,
        subject_id,
        ClaimState::DerivedFact,
        evidence_ids,
    )
}

/// Enforces one additive R16 limit.
///
/// # Errors
///
/// Returns a typed limit failure when `observed` exceeds the frozen maximum.
pub fn enforce_constant_limit(
    limit: ConstantEvaluationLimit,
    observed: usize,
) -> Result<(), ConstantEvaluationError> {
    let observed = u64::try_from(observed).unwrap_or(u64::MAX);
    let maximum = limit.maximum();
    if observed > maximum {
        return Err(ConstantEvaluationError::LimitExceeded {
            limit,
            maximum,
            observed,
        });
    }
    Ok(())
}

fn valid_value(value: &EvaluatedValue) -> bool {
    let valid_type = matches!(
        value.rust_type.as_str(),
        "bool" | "i8" | "i16" | "i32" | "i64" | "i128" | "u8" | "u16" | "u32" | "u64" | "u128"
    );
    valid_type
        && match value.type_authority {
            ConstantTypeAuthority::ExplicitPrimitiveAnnotation => true,
            ConstantTypeAuthority::FixedReprAttribute => {
                value.value_kind == ConstantValueKind::Integer
            }
        }
        && match value.value_kind {
            ConstantValueKind::Boolean => {
                value.rust_type == "bool"
                    && matches!(value.canonical_value.as_str(), "true" | "false")
            }
            ConstantValueKind::Integer => {
                value.rust_type != "bool"
                    && canonical_integer(&value.canonical_value)
                    && integer_in_range(&value.canonical_value, &value.rust_type)
            }
        }
}

fn valid_type_authority(
    value: &EvaluatedValue,
    semantic: &crate::s4_r5::RustSemanticEntity,
) -> bool {
    match value.type_authority {
        ConstantTypeAuthority::ExplicitPrimitiveAnnotation => matches!(
            semantic.kind,
            crate::s4_r5::RustSemanticEntityKind::Constant
                | crate::s4_r5::RustSemanticEntityKind::Static
        ),
        ConstantTypeAuthority::FixedReprAttribute => {
            semantic.kind == crate::s4_r5::RustSemanticEntityKind::EnumVariant
        }
    }
}

fn canonical_integer(value: &str) -> bool {
    let digits = value.strip_prefix('-').unwrap_or(value);
    !digits.is_empty()
        && digits.bytes().all(|byte| byte.is_ascii_digit())
        && (digits == "0" || !digits.starts_with('0'))
        && value != "-0"
}

fn integer_in_range(value: &str, rust_type: &str) -> bool {
    match rust_type.as_bytes().first() {
        Some(b'i') => value.parse::<i128>().ok().is_some_and(|parsed| {
            rust_type[1..].parse::<u32>().ok().is_some_and(|bits| {
                if bits == 128 {
                    true
                } else {
                    let maximum = (1_i128 << (bits - 1)) - 1;
                    parsed >= -maximum - 1 && parsed <= maximum
                }
            })
        }),
        Some(b'u') => value.parse::<u128>().ok().is_some_and(|parsed| {
            rust_type[1..]
                .parse::<u32>()
                .ok()
                .is_some_and(|bits| bits == 128 || parsed < (1_u128 << bits))
        }),
        _ => false,
    }
}

fn validate_acyclic(adjacency: &BTreeMap<&str, Vec<&str>>) -> Result<(), ConstantEvaluationError> {
    fn visit<'a>(
        identifier: &'a str,
        depth: u64,
        adjacency: &BTreeMap<&'a str, Vec<&'a str>>,
        active: &mut BTreeSet<&'a str>,
        complete: &mut BTreeSet<&'a str>,
    ) -> Result<(), ConstantEvaluationError> {
        if depth > MAX_R16_DEPENDENCY_LEVELS {
            return Err(ConstantEvaluationError::LimitExceeded {
                limit: ConstantEvaluationLimit::DependencyLevels,
                maximum: MAX_R16_DEPENDENCY_LEVELS,
                observed: depth,
            });
        }
        if complete.contains(identifier) {
            return Ok(());
        }
        if !active.insert(identifier) {
            return Err(ConstantEvaluationError::DependencyCycle);
        }
        for dependency in adjacency.get(identifier).into_iter().flatten() {
            visit(
                dependency,
                depth.saturating_add(1),
                adjacency,
                active,
                complete,
            )?;
        }
        active.remove(identifier);
        complete.insert(identifier);
        Ok(())
    }

    let mut complete = BTreeSet::new();
    for identifier in adjacency.keys().copied() {
        visit(
            identifier,
            0,
            adjacency,
            &mut BTreeSet::new(),
            &mut complete,
        )?;
    }
    Ok(())
}

fn validate_overlay_union(
    knowledge: &ConstantEvaluationKnowledge,
) -> Result<(), ConstantEvaluationError> {
    ordered_unique(
        knowledge
            .source_overlays
            .iter()
            .map(|overlay| overlay.source_file_id.as_str()),
    )?;
    for overlay in &knowledge.source_overlays {
        ordered_unique(overlay.entities.iter().map(|value| value.id.as_str()))?;
        ordered_unique(overlay.relationships.iter().map(|value| value.id.as_str()))?;
        ordered_unique(overlay.claims.iter().map(|value| value.id.as_str()))?;
        ordered_unique(overlay.coverage.iter().map(|value| value.id.as_str()))?;
        ordered_unique(overlay.removed_coverage_ids.iter().map(String::as_str))?;
        ordered_unique(overlay.removed_diagnostic_ids.iter().map(String::as_str))?;
    }
    let mut entities = knowledge
        .source_overlays
        .iter()
        .flat_map(|overlay| overlay.entities.clone())
        .collect::<Vec<_>>();
    let mut relationships = knowledge
        .source_overlays
        .iter()
        .flat_map(|overlay| overlay.relationships.clone())
        .collect::<Vec<_>>();
    let mut claims = knowledge
        .source_overlays
        .iter()
        .flat_map(|overlay| overlay.claims.clone())
        .collect::<Vec<_>>();
    let mut coverage = knowledge
        .source_overlays
        .iter()
        .flat_map(|overlay| overlay.coverage.clone())
        .collect::<Vec<_>>();
    let mut removed_coverage_ids = knowledge
        .source_overlays
        .iter()
        .flat_map(|overlay| overlay.removed_coverage_ids.clone())
        .collect::<Vec<_>>();
    let mut removed_diagnostic_ids = knowledge
        .source_overlays
        .iter()
        .flat_map(|overlay| overlay.removed_diagnostic_ids.clone())
        .collect::<Vec<_>>();
    entities.sort_by(|left, right| left.id.cmp(&right.id));
    relationships.sort_by(|left, right| left.id.cmp(&right.id));
    claims.sort_by(|left, right| left.id.cmp(&right.id));
    coverage.sort_by(|left, right| left.id.cmp(&right.id));
    removed_coverage_ids.sort();
    removed_diagnostic_ids.sort();
    if entities != knowledge.graph.entities
        || relationships != knowledge.graph.relationships
        || claims != knowledge.graph.claims
        || coverage != knowledge.graph.coverage
        || removed_coverage_ids != knowledge.graph.removed_coverage_ids
        || removed_diagnostic_ids != knowledge.graph.removed_diagnostic_ids
    {
        return Err(ConstantEvaluationError::ContractInvalid);
    }
    Ok(())
}

fn ordered_unique<'a>(
    values: impl IntoIterator<Item = &'a str>,
) -> Result<(), ConstantEvaluationError> {
    let mut previous = None;
    for value in values {
        if previous.is_some_and(|previous| previous >= value) {
            return Err(ConstantEvaluationError::IdentityConflict);
        }
        previous = Some(value);
    }
    Ok(())
}

fn strictly_ordered(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
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
                let mut buffer = [0_u8; 4];
                bytes.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
            }
        }
    }
    bytes.push(b'"');
}
