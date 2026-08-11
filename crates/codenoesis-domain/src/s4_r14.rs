use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::knowledge::{ClaimState, ClaimSubjectKind};
use crate::s4::{WorkspaceClaim, WorkspaceEvidence, workspace_claim_id};
use crate::s4_k1::{
    CallableSemanticEntity, CallableSemanticEntityKind, CallableSemanticProperties,
    CallableSemanticsError, CallableSemanticsExtraction, CallableSemanticsKnowledge,
};
use crate::s5::AnalysisCacheEntry;

pub const R14_CONFIGURATION_VERSION: &str = "codenoesis.configuration/v13";
pub const R14_SNAPSHOT_VERSION: &str = "codenoesis.repository-snapshot/v16";
pub const R14_EXTRACTION_CHUNK_VERSION: &str = "codenoesis.extraction-chunk/v13";
pub const R14_GRAPH_VERSION: &str = "codenoesis.knowledge-graph/v13";
pub const R14_ONTOLOGY_VERSION: &str = "codenoesis.ontology/rust/v13";
pub const R14_PIPELINE_VERSION: &str = "codenoesis.pipeline/s4-r14-v1";
pub const R14_EXTRACTION_CONTRACT_VERSION: &str = "codenoesis.extraction/v13";
pub const R14_EXTRACTOR_VERSION: &str = "codenoesis.rust-expression-bindings/s4-r14-v1";
pub const R14_INDEX_VERSION: &str = "codenoesis.expression-binding-index/v1";
pub const R14_SEMANTIC_HASH_CONTRACT_VERSION: &str = "codenoesis.semantic-hash-contract/v12";
pub const R14_ERROR_VERSION: &str = "codenoesis.error/v21";
pub const R14_QUERY_VERSION: &str = "codenoesis.local-query-result/v11";
pub const R14_PORTABLE_GRAPH_VERSION: &str = "codenoesis.portable-graph/v7";
pub const R14_LOCAL_EXPLORER_VERSION: &str = "codenoesis.local-explorer/v7";
pub const R14_PROFILE: &str = "rust-expression-bindings-v1";

pub const MAX_R14_EXPRESSIONS_PER_CALLABLE: u64 = 16_384;
pub const MAX_R14_EXPRESSION_DEPTH: u64 = 256;
pub const MAX_R14_ARGUMENTS_PER_CALL: u64 = 256;
pub const MAX_R14_BINDINGS_PER_CALLABLE: u64 = 4_096;
pub const MAX_R14_EXPRESSIONS: u64 = 400_000;
pub const MAX_R14_BINDINGS_AND_ARGUMENTS: u64 = 200_000;
pub const MAX_R14_RELATIONSHIPS: u64 = 1_000_000;
pub const MAX_R14_NORMALIZED_SPELLING_BYTES: u64 = 4_096;

const ENTITY_ID_DOMAIN: &str = "codenoesis.entity-id/rust-expression-bindings/v1";
const RELATIONSHIP_ID_DOMAIN: &str = "codenoesis.relationship-id/rust-expression-bindings/v1";
const COVERAGE_GAP_ID_DOMAIN: &str = "codenoesis.coverage-gap-id/rust-expression-bindings/v1";

pub const SELECTED_EXPRESSION_KINDS: [&str; 27] = [
    "array_expression",
    "assignment_expression",
    "await_expression",
    "binary_expression",
    "boolean_literal",
    "char_literal",
    "float_literal",
    "integer_literal",
    "raw_string_literal",
    "string_literal",
    "call_expression",
    "compound_assignment_expr",
    "field_expression",
    "generic_function",
    "identifier",
    "self",
    "scoped_identifier",
    "index_expression",
    "parenthesized_expression",
    "range_expression",
    "reference_expression",
    "struct_expression",
    "try_expression",
    "tuple_expression",
    "type_cast_expression",
    "unary_expression",
    "unit_expression",
];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ExpressionEntityKind {
    Expression,
    CallArgument,
    PatternBinding,
}

impl ExpressionEntityKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Expression => "rust.expression",
            Self::CallArgument => "rust.call_argument",
            Self::PatternBinding => "rust.pattern_binding",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ExpressionRelationshipKind {
    HasExpression,
    ContainsExpression,
    HasArgument,
    ArgumentValue,
    HasReceiver,
    RepresentsCallSite,
    DeclaresBinding,
    BindsFrom,
    Reads,
    Writes,
}

impl ExpressionRelationshipKind {
    pub const ALL: [Self; 10] = [
        Self::HasExpression,
        Self::ContainsExpression,
        Self::HasArgument,
        Self::ArgumentValue,
        Self::HasReceiver,
        Self::RepresentsCallSite,
        Self::DeclaresBinding,
        Self::BindsFrom,
        Self::Reads,
        Self::Writes,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HasExpression => "HAS_EXPRESSION",
            Self::ContainsExpression => "CONTAINS_EXPRESSION",
            Self::HasArgument => "HAS_ARGUMENT",
            Self::ArgumentValue => "ARGUMENT_VALUE",
            Self::HasReceiver => "HAS_RECEIVER",
            Self::RepresentsCallSite => "REPRESENTS_CALL_SITE",
            Self::DeclaresBinding => "DECLARES_BINDING",
            Self::BindsFrom => "BINDS_FROM",
            Self::Reads => "READS",
            Self::Writes => "WRITES",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ExpressionRole {
    Argument,
    AssignmentTarget,
    AssignmentValue,
    BodyTail,
    Callee,
    Condition,
    Initializer,
    Iterator,
    Nested,
    PatternInput,
    Receiver,
    ReturnValue,
}

impl ExpressionRole {
    pub const ALL: [Self; 12] = [
        Self::Argument,
        Self::AssignmentTarget,
        Self::AssignmentValue,
        Self::BodyTail,
        Self::Callee,
        Self::Condition,
        Self::Initializer,
        Self::Iterator,
        Self::Nested,
        Self::PatternInput,
        Self::Receiver,
        Self::ReturnValue,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Argument => "argument",
            Self::AssignmentTarget => "assignment_target",
            Self::AssignmentValue => "assignment_value",
            Self::BodyTail => "body_tail",
            Self::Callee => "callee",
            Self::Condition => "condition",
            Self::Initializer => "initializer",
            Self::Iterator => "iterator",
            Self::Nested => "nested",
            Self::PatternInput => "pattern_input",
            Self::Receiver => "receiver",
            Self::ReturnValue => "return_value",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BindingOrigin {
    Parameter,
    LocalLet,
    IfLet,
    WhileLet,
    For,
    MatchArm,
}

impl BindingOrigin {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Parameter => "parameter",
            Self::LocalLet => "local_let",
            Self::IfLet => "if_let",
            Self::WhileLet => "while_let",
            Self::For => "for",
            Self::MatchArm => "match_arm",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BindingModifier {
    None,
    ExplicitMut,
    ExplicitRef,
    ExplicitRefMut,
}

impl BindingModifier {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::ExplicitMut => "explicit_mut",
            Self::ExplicitRef => "explicit_ref",
            Self::ExplicitRefMut => "explicit_ref_mut",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpressionLocator {
    pub path: String,
    pub blob_oid: String,
    pub start_byte: u64,
    pub end_byte: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpressionProperties {
    pub syntax_kind: String,
    pub token: Option<String>,
    pub operator: Option<String>,
    pub source_digest: String,
    pub source_byte_length: u64,
    pub parent_expression_id: Option<String>,
    pub lexical_depth: u64,
    pub roles: Vec<ExpressionRole>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallArgumentProperties {
    pub call_expression_id: String,
    pub ordinal: u64,
    pub expression_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatternBindingProperties {
    pub origin: BindingOrigin,
    pub scope_owner_id: String,
    pub modifier: BindingModifier,
    pub scope_start_byte: u64,
    pub scope_end_byte: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExpressionEntityProperties {
    Expression(ExpressionProperties),
    CallArgument(CallArgumentProperties),
    PatternBinding(PatternBindingProperties),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpressionBindingEntity {
    pub id: String,
    pub kind: ExpressionEntityKind,
    pub name: String,
    pub callable_id: String,
    pub source_file_id: String,
    pub evidence_id: String,
    pub locator: ExpressionLocator,
    pub properties: ExpressionEntityProperties,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpressionBindingRelationship {
    pub id: String,
    pub kind: ExpressionRelationshipKind,
    pub source: String,
    pub target: String,
    pub evidence_ids: Vec<String>,
}

impl ExpressionBindingRelationship {
    #[must_use]
    pub fn new(
        kind: ExpressionRelationshipKind,
        source: String,
        target: String,
        mut evidence_ids: Vec<String>,
    ) -> Self {
        evidence_ids.sort();
        evidence_ids.dedup();
        Self {
            id: expression_relationship_id(kind, &source, &target),
            kind,
            source,
            target,
            evidence_ids,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpressionCoverageGap {
    pub id: String,
    pub capability: String,
    pub state: String,
    pub subject_id: String,
    pub evidence_ids: Vec<String>,
}

impl ExpressionCoverageGap {
    #[must_use]
    pub fn unsupported(
        capability: &str,
        subject_id: String,
        mut evidence_ids: Vec<String>,
    ) -> Self {
        evidence_ids.sort();
        evidence_ids.dedup();
        let state = "unsupported".to_owned();
        let id = expression_coverage_gap_id(capability, &state, &subject_id, &evidence_ids);
        Self {
            id,
            capability: capability.to_owned(),
            state,
            subject_id,
            evidence_ids,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpressionBindingSourceChunk {
    pub crate_id: String,
    pub source_file_id: String,
    pub path: String,
    pub entities: Vec<ExpressionBindingEntity>,
    pub relationships: Vec<ExpressionBindingRelationship>,
    pub claims: Vec<WorkspaceClaim>,
    pub evidence: Vec<WorkspaceEvidence>,
    pub coverage: Vec<ExpressionCoverageGap>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpressionBindingIndex {
    pub expression_entity_ids: Vec<String>,
    pub argument_entity_ids: Vec<String>,
    pub binding_entity_ids: Vec<String>,
    pub read_relationship_ids: Vec<String>,
    pub write_relationship_ids: Vec<String>,
    pub call_site_relationship_ids: Vec<String>,
}

impl ExpressionBindingIndex {
    #[must_use]
    pub fn from_graph(
        entities: &[ExpressionBindingEntity],
        relationships: &[ExpressionBindingRelationship],
    ) -> Self {
        let entity_ids = |kind| {
            entities
                .iter()
                .filter(|entity| entity.kind == kind)
                .map(|entity| entity.id.clone())
                .collect()
        };
        let relationship_ids = |kind| {
            relationships
                .iter()
                .filter(|relationship| relationship.kind == kind)
                .map(|relationship| relationship.id.clone())
                .collect()
        };
        Self {
            expression_entity_ids: entity_ids(ExpressionEntityKind::Expression),
            argument_entity_ids: entity_ids(ExpressionEntityKind::CallArgument),
            binding_entity_ids: entity_ids(ExpressionEntityKind::PatternBinding),
            read_relationship_ids: relationship_ids(ExpressionRelationshipKind::Reads),
            write_relationship_ids: relationship_ids(ExpressionRelationshipKind::Writes),
            call_site_relationship_ids: relationship_ids(
                ExpressionRelationshipKind::RepresentsCallSite,
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpressionBindingGraph {
    pub entities: Vec<ExpressionBindingEntity>,
    pub relationships: Vec<ExpressionBindingRelationship>,
    pub claims: Vec<WorkspaceClaim>,
    pub evidence: Vec<WorkspaceEvidence>,
    pub coverage: Vec<ExpressionCoverageGap>,
    pub index: ExpressionBindingIndex,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpressionBindingKnowledge {
    pub callable: CallableSemanticsKnowledge,
    pub extraction_chunks: Vec<ExpressionBindingSourceChunk>,
    pub graph: ExpressionBindingGraph,
}

impl ExpressionBindingKnowledge {
    /// Validates the inherited K1 lineage and the complete additive R14 graph.
    ///
    /// # Errors
    ///
    /// Returns the first identity, ordering, limit, evidence, scope, or reference failure.
    #[allow(clippy::too_many_lines)]
    pub fn validate(&self) -> Result<(), ExpressionBindingError> {
        self.callable
            .validate()
            .map_err(ExpressionBindingError::Source)?;
        if self.extraction_chunks.is_empty() {
            return Err(ExpressionBindingError::ContractInvalid);
        }
        validate_ordered_identities(self.graph.entities.iter().map(|value| value.id.as_str()))?;
        validate_ordered_identities(
            self.graph
                .relationships
                .iter()
                .map(|value| value.id.as_str()),
        )?;
        validate_ordered_identities(self.graph.claims.iter().map(|value| value.id.as_str()))?;
        validate_ordered_identities(self.graph.evidence.iter().map(|value| value.id.as_str()))?;
        validate_ordered_identities(self.graph.coverage.iter().map(|value| value.id.as_str()))?;
        enforce_expression_limit(
            ExpressionBindingLimit::ExpressionsTotal,
            self.graph
                .entities
                .iter()
                .filter(|value| value.kind == ExpressionEntityKind::Expression)
                .count(),
        )?;
        enforce_expression_limit(
            ExpressionBindingLimit::BindingsAndArgumentsTotal,
            self.graph
                .entities
                .iter()
                .filter(|value| value.kind != ExpressionEntityKind::Expression)
                .count(),
        )?;
        enforce_expression_limit(
            ExpressionBindingLimit::RelationshipsTotal,
            self.graph.relationships.len(),
        )?;

        let repository_identity = self
            .callable
            .framework
            .semantic
            .manifest
            .workspace
            .knowledge
            .graph
            .repository_identity
            .as_str();
        let callable_ids = self
            .callable
            .graph
            .entities
            .iter()
            .filter(|entity| entity.kind == CallableSemanticEntityKind::Signature)
            .map(|entity| entity.subject_id.as_str())
            .collect::<BTreeSet<_>>();
        let callable_entities = self
            .callable
            .graph
            .entities
            .iter()
            .map(|entity| (entity.id.as_str(), entity))
            .collect::<BTreeMap<_, _>>();
        let callable_entity_ids = callable_entities.keys().copied().collect::<BTreeSet<_>>();
        let entity_ids = self
            .graph
            .entities
            .iter()
            .map(|entity| entity.id.as_str())
            .collect::<BTreeSet<_>>();
        let evidence = self
            .graph
            .evidence
            .iter()
            .map(|value| (value.id.as_str(), value))
            .collect::<BTreeMap<_, _>>();
        let mut expressions_per_callable = BTreeMap::<&str, usize>::new();
        let mut bindings_per_callable = BTreeMap::<&str, usize>::new();
        validate_argument_ordinals(&self.graph.entities)?;
        for entity in &self.graph.entities {
            if !callable_ids.contains(entity.callable_id.as_str())
                || entity.id != expected_entity_id(repository_identity, entity)
                || !valid_locator(entity, &evidence)
            {
                return Err(ExpressionBindingError::ContractInvalid);
            }
            match (entity.kind, &entity.properties) {
                (
                    ExpressionEntityKind::Expression,
                    ExpressionEntityProperties::Expression(properties),
                ) => {
                    *expressions_per_callable
                        .entry(entity.callable_id.as_str())
                        .or_default() += 1;
                    if entity.name
                        != properties
                            .token
                            .as_deref()
                            .unwrap_or(properties.syntax_kind.as_str())
                        || properties.source_byte_length
                            != entity
                                .locator
                                .end_byte
                                .saturating_sub(entity.locator.start_byte)
                    {
                        return Err(ExpressionBindingError::ContractInvalid);
                    }
                    validate_expression_properties(properties)?;
                }
                (
                    ExpressionEntityKind::CallArgument,
                    ExpressionEntityProperties::CallArgument(properties),
                ) => {
                    if !entity_ids.contains(properties.call_expression_id.as_str())
                        || !entity_ids.contains(properties.expression_id.as_str())
                        || entity.name != format!("argument:{}", properties.ordinal)
                    {
                        return Err(ExpressionBindingError::ArgumentOrdinalInvalid);
                    }
                }
                (
                    ExpressionEntityKind::PatternBinding,
                    ExpressionEntityProperties::PatternBinding(properties),
                ) => {
                    *bindings_per_callable
                        .entry(entity.callable_id.as_str())
                        .or_default() += 1;
                    if properties.scope_start_byte > properties.scope_end_byte
                        || entity.locator.end_byte > properties.scope_start_byte
                        || entity.name.is_empty()
                        || u64::try_from(entity.name.len()).unwrap_or(u64::MAX)
                            > MAX_R14_NORMALIZED_SPELLING_BYTES
                        || !valid_binding_owner(entity, properties, &callable_entities)
                    {
                        return Err(ExpressionBindingError::BindingScopeInvalid);
                    }
                }
                _ => return Err(ExpressionBindingError::ContractInvalid),
            }
        }
        for observed in expressions_per_callable.into_values() {
            enforce_expression_limit(ExpressionBindingLimit::ExpressionsPerCallable, observed)?;
        }
        for observed in bindings_per_callable.into_values() {
            enforce_expression_limit(ExpressionBindingLimit::BindingsPerCallable, observed)?;
        }

        let all_endpoints = callable_ids
            .iter()
            .copied()
            .chain(callable_entity_ids.iter().copied())
            .chain(entity_ids.iter().copied())
            .collect::<BTreeSet<_>>();
        let relationship_ids = self
            .graph
            .relationships
            .iter()
            .map(|value| value.id.as_str())
            .collect::<BTreeSet<_>>();
        for relationship in &self.graph.relationships {
            if relationship.id
                != expression_relationship_id(
                    relationship.kind,
                    &relationship.source,
                    &relationship.target,
                )
                || !all_endpoints.contains(relationship.source.as_str())
                || !all_endpoints.contains(relationship.target.as_str())
                || relationship.evidence_ids.is_empty()
                || relationship
                    .evidence_ids
                    .iter()
                    .any(|identifier| !evidence.contains_key(identifier.as_str()))
            {
                return Err(ExpressionBindingError::ContractInvalid);
            }
        }
        validate_relationship_semantics(self)?;
        for gap in &self.graph.coverage {
            if gap.id
                != expression_coverage_gap_id(
                    &gap.capability,
                    &gap.state,
                    &gap.subject_id,
                    &gap.evidence_ids,
                )
                || gap.state != "unsupported"
                || gap.capability.is_empty()
                || !all_endpoints.contains(gap.subject_id.as_str())
                || gap.evidence_ids.is_empty()
                || gap
                    .evidence_ids
                    .iter()
                    .any(|identifier| !evidence.contains_key(identifier.as_str()))
            {
                return Err(ExpressionBindingError::ContractInvalid);
            }
        }
        validate_parent_depths(&self.graph.entities, &self.graph.relationships)?;

        let subjects = entity_ids
            .iter()
            .copied()
            .chain(relationship_ids.iter().copied())
            .collect::<BTreeSet<_>>();
        if self.graph.claims.len() != subjects.len() {
            return Err(ExpressionBindingError::ContractInvalid);
        }
        for claim in &self.graph.claims {
            let expected_subject_kind = if entity_ids.contains(claim.subject_id.as_str()) {
                ClaimSubjectKind::Entity
            } else if relationship_ids.contains(claim.subject_id.as_str()) {
                ClaimSubjectKind::Relationship
            } else {
                return Err(ExpressionBindingError::ContractInvalid);
            };
            if claim.id != workspace_claim_id(claim.subject_kind, &claim.subject_id, claim.state)
                || claim.state != ClaimState::DeterministicFact
                || claim.subject_kind != expected_subject_kind
                || !subjects.contains(claim.subject_id.as_str())
                || claim.evidence_ids.is_empty()
                || claim
                    .evidence_ids
                    .iter()
                    .any(|identifier| !evidence.contains_key(identifier.as_str()))
            {
                return Err(ExpressionBindingError::ContractInvalid);
            }
        }
        if self.graph.index
            != ExpressionBindingIndex::from_graph(&self.graph.entities, &self.graph.relationships)
        {
            return Err(ExpressionBindingError::IndexMismatch);
        }
        validate_chunk_union(self)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpressionBindingExtraction {
    pub knowledge: ExpressionBindingKnowledge,
    pub cache_entries: Vec<AnalysisCacheEntry>,
    pub parser_invocation_count: u64,
}

impl ExpressionBindingExtraction {
    #[must_use]
    pub fn from_k1(
        source: CallableSemanticsExtraction,
        extraction_chunks: Vec<ExpressionBindingSourceChunk>,
        graph: ExpressionBindingGraph,
        parser_invocation_count: u64,
    ) -> Self {
        Self {
            knowledge: ExpressionBindingKnowledge {
                callable: source.knowledge,
                extraction_chunks,
                graph,
            },
            cache_entries: source.cache_entries,
            parser_invocation_count,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExpressionBindingLimit {
    ExpressionsPerCallable,
    ExpressionDepth,
    ArgumentsPerCall,
    BindingsPerCallable,
    ExpressionsTotal,
    BindingsAndArgumentsTotal,
    RelationshipsTotal,
    NormalizedSpellingBytes,
}

impl ExpressionBindingLimit {
    #[must_use]
    pub const fn maximum(self) -> u64 {
        match self {
            Self::ExpressionsPerCallable => MAX_R14_EXPRESSIONS_PER_CALLABLE,
            Self::ExpressionDepth => MAX_R14_EXPRESSION_DEPTH,
            Self::ArgumentsPerCall => MAX_R14_ARGUMENTS_PER_CALL,
            Self::BindingsPerCallable => MAX_R14_BINDINGS_PER_CALLABLE,
            Self::ExpressionsTotal => MAX_R14_EXPRESSIONS,
            Self::BindingsAndArgumentsTotal => MAX_R14_BINDINGS_AND_ARGUMENTS,
            Self::RelationshipsTotal => MAX_R14_RELATIONSHIPS,
            Self::NormalizedSpellingBytes => MAX_R14_NORMALIZED_SPELLING_BYTES,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExpressionsPerCallable => "expressions_per_callable",
            Self::ExpressionDepth => "selected_expression_depth",
            Self::ArgumentsPerCall => "arguments_per_call",
            Self::BindingsPerCallable => "bindings_per_callable",
            Self::ExpressionsTotal => "expressions_total",
            Self::BindingsAndArgumentsTotal => "bindings_and_arguments_total",
            Self::RelationshipsTotal => "relationships_total",
            Self::NormalizedSpellingBytes => "normalized_spelling_utf8_bytes",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExpressionBindingError {
    Source(CallableSemanticsError),
    InvalidSyntax {
        path: String,
        start_byte: u64,
        syntax_kind: String,
    },
    IdentityConflict,
    ParentInvalid,
    OperatorInvalid,
    RoleInvalid,
    ArgumentOrdinalInvalid,
    PatternUnsupported,
    BindingScopeInvalid,
    BindingAmbiguous,
    AccessResolutionInvalid,
    CallSiteEvidenceMismatch,
    IndexMismatch,
    LimitExceeded {
        limit: ExpressionBindingLimit,
        maximum: u64,
        observed: u64,
    },
    ContractInvalid,
}

impl Display for ExpressionBindingError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Source(_) => "R14 callable source extraction failed",
            Self::InvalidSyntax { .. } => "invalid R14 Rust expression syntax",
            Self::IdentityConflict => "R14 expression identity conflict",
            Self::ParentInvalid => "R14 expression parent is invalid",
            Self::OperatorInvalid => "R14 expression operator is invalid",
            Self::RoleInvalid => "R14 expression role is invalid",
            Self::ArgumentOrdinalInvalid => "R14 call argument ordinal is invalid",
            Self::PatternUnsupported => "R14 pattern is unsupported",
            Self::BindingScopeInvalid => "R14 binding scope is invalid",
            Self::BindingAmbiguous => "R14 binding resolution is ambiguous",
            Self::AccessResolutionInvalid => "R14 access resolution is invalid",
            Self::CallSiteEvidenceMismatch => "R14 call-site evidence does not match",
            Self::IndexMismatch => "R14 expression index does not match the graph",
            Self::LimitExceeded { .. } => "R14 expression limit exceeded",
            Self::ContractInvalid => "R14 expression contract is invalid",
        })
    }
}

impl Error for ExpressionBindingError {}

#[must_use]
pub fn expression_entity_id(
    repository_identity: &str,
    callable_id: &str,
    source_file_id: &str,
    start_byte: u64,
    end_byte: u64,
    syntax_kind: &str,
) -> String {
    stable_id(
        "urn:codenoesis:entity:blake3:",
        &[
            ENTITY_ID_DOMAIN,
            "expression",
            repository_identity,
            callable_id,
            source_file_id,
            &start_byte.to_string(),
            &end_byte.to_string(),
            syntax_kind,
        ],
    )
}

#[must_use]
pub fn call_argument_entity_id(call_expression_id: &str, ordinal: u64) -> String {
    stable_id(
        "urn:codenoesis:entity:blake3:",
        &[
            ENTITY_ID_DOMAIN,
            "argument",
            call_expression_id,
            &ordinal.to_string(),
        ],
    )
}

#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn pattern_binding_entity_id(
    repository_identity: &str,
    callable_id: &str,
    scope_owner_id: &str,
    source_file_id: &str,
    start_byte: u64,
    end_byte: u64,
    normalized_name: &str,
) -> String {
    stable_id(
        "urn:codenoesis:entity:blake3:",
        &[
            ENTITY_ID_DOMAIN,
            "binding",
            repository_identity,
            callable_id,
            scope_owner_id,
            source_file_id,
            &start_byte.to_string(),
            &end_byte.to_string(),
            normalized_name,
        ],
    )
}

#[must_use]
pub fn expression_relationship_id(
    kind: ExpressionRelationshipKind,
    source: &str,
    target: &str,
) -> String {
    stable_id(
        "urn:codenoesis:relationship:blake3:",
        &[RELATIONSHIP_ID_DOMAIN, kind.as_str(), source, target],
    )
}

fn expression_coverage_gap_id(
    capability: &str,
    state: &str,
    subject_id: &str,
    evidence_ids: &[String],
) -> String {
    stable_id(
        "urn:codenoesis:coverage-gap:blake3:",
        &[
            COVERAGE_GAP_ID_DOMAIN,
            capability,
            state,
            subject_id,
            &evidence_ids.join("\u{1f}"),
        ],
    )
}

#[must_use]
pub fn expression_claim(
    subject_kind: ClaimSubjectKind,
    subject_id: String,
    evidence_ids: Vec<String>,
) -> WorkspaceClaim {
    WorkspaceClaim::new(
        subject_kind,
        subject_id,
        ClaimState::DeterministicFact,
        evidence_ids,
    )
}

/// Enforces one fixed R14 resource limit.
///
/// # Errors
///
/// Returns the exact maximum and observed value when exceeded.
pub fn enforce_expression_limit(
    limit: ExpressionBindingLimit,
    observed: usize,
) -> Result<(), ExpressionBindingError> {
    let observed = u64::try_from(observed).unwrap_or(u64::MAX);
    let maximum = limit.maximum();
    if observed > maximum {
        Err(ExpressionBindingError::LimitExceeded {
            limit,
            maximum,
            observed,
        })
    } else {
        Ok(())
    }
}

fn expected_entity_id(repository_identity: &str, entity: &ExpressionBindingEntity) -> String {
    match &entity.properties {
        ExpressionEntityProperties::Expression(properties) => expression_entity_id(
            repository_identity,
            &entity.callable_id,
            &entity.source_file_id,
            entity.locator.start_byte,
            entity.locator.end_byte,
            &properties.syntax_kind,
        ),
        ExpressionEntityProperties::CallArgument(properties) => {
            call_argument_entity_id(&properties.call_expression_id, properties.ordinal)
        }
        ExpressionEntityProperties::PatternBinding(properties) => pattern_binding_entity_id(
            repository_identity,
            &entity.callable_id,
            &properties.scope_owner_id,
            &entity.source_file_id,
            entity.locator.start_byte,
            entity.locator.end_byte,
            &entity.name,
        ),
    }
}

fn valid_locator(
    entity: &ExpressionBindingEntity,
    evidence: &BTreeMap<&str, &WorkspaceEvidence>,
) -> bool {
    let Some(value) = evidence.get(entity.evidence_id.as_str()) else {
        return false;
    };
    value.path == entity.locator.path
        && value.blob_oid == entity.locator.blob_oid
        && value.start_byte == entity.locator.start_byte
        && value.end_byte == entity.locator.end_byte
        && value.start_byte < value.end_byte
}

fn valid_binding_owner(
    entity: &ExpressionBindingEntity,
    properties: &PatternBindingProperties,
    callable_entities: &BTreeMap<&str, &CallableSemanticEntity>,
) -> bool {
    let Some(owner) = callable_entities.get(properties.scope_owner_id.as_str()) else {
        return false;
    };
    if owner.subject_id != entity.callable_id {
        return false;
    }
    match (properties.origin, owner.kind, &owner.properties) {
        (
            BindingOrigin::Parameter,
            CallableSemanticEntityKind::Parameter,
            CallableSemanticProperties::Parameter(_),
        )
        | (
            BindingOrigin::LocalLet,
            CallableSemanticEntityKind::LocalBinding,
            CallableSemanticProperties::LocalBinding(_),
        ) => true,
        (
            origin,
            CallableSemanticEntityKind::Control,
            CallableSemanticProperties::Control(control),
        ) => {
            let expected = match origin {
                BindingOrigin::IfLet => "if_let",
                BindingOrigin::WhileLet => "while_let",
                BindingOrigin::For => "for",
                BindingOrigin::MatchArm => "match",
                BindingOrigin::Parameter | BindingOrigin::LocalLet => return false,
            };
            control.control_kind.as_str() == expected
        }
        _ => false,
    }
}

fn validate_expression_properties(
    properties: &ExpressionProperties,
) -> Result<(), ExpressionBindingError> {
    if !SELECTED_EXPRESSION_KINDS.contains(&properties.syntax_kind.as_str())
        || properties.source_byte_length == 0
        || properties.source_digest.len() != 64
        || !properties
            .source_digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(ExpressionBindingError::ContractInvalid);
    }
    if properties.lexical_depth > MAX_R14_EXPRESSION_DEPTH {
        return Err(ExpressionBindingError::LimitExceeded {
            limit: ExpressionBindingLimit::ExpressionDepth,
            maximum: MAX_R14_EXPRESSION_DEPTH,
            observed: properties.lexical_depth,
        });
    }
    if properties.roles.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ExpressionBindingError::RoleInvalid);
    }
    let token_required = matches!(
        properties.syntax_kind.as_str(),
        "identifier" | "self" | "scoped_identifier"
    );
    if token_required
        != properties
            .token
            .as_ref()
            .is_some_and(|token| !token.is_empty())
    {
        return Err(ExpressionBindingError::ContractInvalid);
    }
    let operator_allowed = match properties.syntax_kind.as_str() {
        "assignment_expression" => properties.operator.as_deref() == Some("="),
        "binary_expression" => properties.operator.as_deref().is_some_and(|value| {
            matches!(
                value,
                "+" | "-"
                    | "*"
                    | "/"
                    | "%"
                    | "&&"
                    | "||"
                    | "&"
                    | "|"
                    | "^"
                    | "<<"
                    | ">>"
                    | "=="
                    | "!="
                    | "<"
                    | "<="
                    | ">"
                    | ">="
            )
        }),
        "compound_assignment_expr" => properties.operator.as_deref().is_some_and(|value| {
            matches!(
                value,
                "+=" | "-=" | "*=" | "/=" | "%=" | "&=" | "|=" | "^=" | "<<=" | ">>="
            )
        }),
        "unary_expression" => properties
            .operator
            .as_deref()
            .is_some_and(|value| matches!(value, "-" | "!" | "*")),
        _ => properties.operator.is_none(),
    };
    if !operator_allowed {
        return Err(ExpressionBindingError::OperatorInvalid);
    }
    if let Some(token) = &properties.token
        && u64::try_from(token.len()).unwrap_or(u64::MAX) > MAX_R14_NORMALIZED_SPELLING_BYTES
    {
        return Err(ExpressionBindingError::LimitExceeded {
            limit: ExpressionBindingLimit::NormalizedSpellingBytes,
            maximum: MAX_R14_NORMALIZED_SPELLING_BYTES,
            observed: u64::try_from(token.len()).unwrap_or(u64::MAX),
        });
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn validate_relationship_semantics(
    knowledge: &ExpressionBindingKnowledge,
) -> Result<(), ExpressionBindingError> {
    let entities = knowledge
        .graph
        .entities
        .iter()
        .map(|entity| (entity.id.as_str(), entity))
        .collect::<BTreeMap<_, _>>();
    let callable_entities = knowledge
        .callable
        .graph
        .entities
        .iter()
        .map(|entity| (entity.id.as_str(), entity))
        .collect::<BTreeMap<_, _>>();
    let mut has_expression = BTreeMap::<&str, usize>::new();
    let mut contains_expression = BTreeMap::<&str, usize>::new();
    let mut has_argument = BTreeMap::<&str, usize>::new();
    let mut argument_value = BTreeMap::<&str, usize>::new();
    let mut has_receiver = BTreeMap::<&str, usize>::new();
    let mut represents_call_site = BTreeMap::<&str, usize>::new();
    let mut represented_call_sites = BTreeMap::<&str, usize>::new();
    let mut declares_binding = BTreeMap::<&str, usize>::new();
    let mut binds_from = BTreeMap::<&str, usize>::new();

    for relationship in &knowledge.graph.relationships {
        match relationship.kind {
            ExpressionRelationshipKind::HasExpression => {
                let target = expression_entity(&entities, &relationship.target)
                    .ok_or(ExpressionBindingError::ContractInvalid)?;
                if relationship.source != target.callable_id
                    || !single_evidence(relationship, &target.evidence_id)
                {
                    return Err(ExpressionBindingError::ContractInvalid);
                }
                increment(&mut has_expression, target.id.as_str());
            }
            ExpressionRelationshipKind::ContainsExpression => {
                let source = expression_entity(&entities, &relationship.source)
                    .ok_or(ExpressionBindingError::ParentInvalid)?;
                let target = expression_entity(&entities, &relationship.target)
                    .ok_or(ExpressionBindingError::ParentInvalid)?;
                let ExpressionEntityProperties::Expression(properties) = &target.properties else {
                    return Err(ExpressionBindingError::ParentInvalid);
                };
                if source.callable_id != target.callable_id
                    || properties.parent_expression_id.as_deref() != Some(source.id.as_str())
                    || source.locator.path != target.locator.path
                    || source.locator.start_byte > target.locator.start_byte
                    || source.locator.end_byte < target.locator.end_byte
                    || (source.locator.start_byte == target.locator.start_byte
                        && source.locator.end_byte == target.locator.end_byte)
                    || !single_evidence(relationship, &target.evidence_id)
                {
                    return Err(ExpressionBindingError::ParentInvalid);
                }
                increment(&mut contains_expression, target.id.as_str());
            }
            ExpressionRelationshipKind::HasArgument => {
                let source = call_expression(&entities, &relationship.source)
                    .ok_or(ExpressionBindingError::ArgumentOrdinalInvalid)?;
                let target = argument_entity(&entities, &relationship.target)
                    .ok_or(ExpressionBindingError::ArgumentOrdinalInvalid)?;
                let ExpressionEntityProperties::CallArgument(properties) = &target.properties
                else {
                    return Err(ExpressionBindingError::ArgumentOrdinalInvalid);
                };
                if properties.call_expression_id != source.id
                    || source.callable_id != target.callable_id
                    || !single_evidence(relationship, &target.evidence_id)
                {
                    return Err(ExpressionBindingError::ArgumentOrdinalInvalid);
                }
                increment(&mut has_argument, target.id.as_str());
            }
            ExpressionRelationshipKind::ArgumentValue => {
                let source = argument_entity(&entities, &relationship.source)
                    .ok_or(ExpressionBindingError::ArgumentOrdinalInvalid)?;
                let target = expression_entity(&entities, &relationship.target)
                    .ok_or(ExpressionBindingError::ArgumentOrdinalInvalid)?;
                let ExpressionEntityProperties::CallArgument(properties) = &source.properties
                else {
                    return Err(ExpressionBindingError::ArgumentOrdinalInvalid);
                };
                if properties.expression_id != target.id
                    || source.callable_id != target.callable_id
                    || source.evidence_id != target.evidence_id
                    || source.locator != target.locator
                    || !single_evidence(relationship, &target.evidence_id)
                {
                    return Err(ExpressionBindingError::ArgumentOrdinalInvalid);
                }
                increment(&mut argument_value, source.id.as_str());
            }
            ExpressionRelationshipKind::HasReceiver => {
                let source = call_expression(&entities, &relationship.source)
                    .ok_or(ExpressionBindingError::RoleInvalid)?;
                let target = expression_entity(&entities, &relationship.target)
                    .ok_or(ExpressionBindingError::RoleInvalid)?;
                let ExpressionEntityProperties::Expression(properties) = &target.properties else {
                    return Err(ExpressionBindingError::RoleInvalid);
                };
                if source.callable_id != target.callable_id
                    || !properties.roles.contains(&ExpressionRole::Receiver)
                    || !single_evidence(relationship, &target.evidence_id)
                {
                    return Err(ExpressionBindingError::RoleInvalid);
                }
                increment(&mut has_receiver, source.id.as_str());
            }
            ExpressionRelationshipKind::RepresentsCallSite => {
                let source = call_expression(&entities, &relationship.source)
                    .ok_or(ExpressionBindingError::CallSiteEvidenceMismatch)?;
                let Some(target) = callable_entities.get(relationship.target.as_str()) else {
                    return Err(ExpressionBindingError::CallSiteEvidenceMismatch);
                };
                if target.kind != CallableSemanticEntityKind::CallSite
                    || target.subject_id != source.callable_id
                    || !target.evidence_ids.contains(&source.evidence_id)
                    || !single_evidence(relationship, &source.evidence_id)
                {
                    return Err(ExpressionBindingError::CallSiteEvidenceMismatch);
                }
                increment(&mut represents_call_site, source.id.as_str());
                increment(&mut represented_call_sites, target.id.as_str());
            }
            ExpressionRelationshipKind::DeclaresBinding => {
                let target = binding_entity(&entities, &relationship.target)
                    .ok_or(ExpressionBindingError::BindingScopeInvalid)?;
                let ExpressionEntityProperties::PatternBinding(properties) = &target.properties
                else {
                    return Err(ExpressionBindingError::BindingScopeInvalid);
                };
                if relationship.source != properties.scope_owner_id
                    || !single_evidence(relationship, &target.evidence_id)
                {
                    return Err(ExpressionBindingError::BindingScopeInvalid);
                }
                increment(&mut declares_binding, target.id.as_str());
            }
            ExpressionRelationshipKind::BindsFrom => {
                let source = binding_entity(&entities, &relationship.source)
                    .ok_or(ExpressionBindingError::AccessResolutionInvalid)?;
                let target = expression_entity(&entities, &relationship.target)
                    .ok_or(ExpressionBindingError::AccessResolutionInvalid)?;
                if source.callable_id != target.callable_id
                    || relationship.evidence_ids
                        != canonical_evidence_ids(&source.evidence_id, &target.evidence_id)
                {
                    return Err(ExpressionBindingError::AccessResolutionInvalid);
                }
                increment(&mut binds_from, source.id.as_str());
            }
            ExpressionRelationshipKind::Reads | ExpressionRelationshipKind::Writes => {
                validate_access_relationship(relationship, &entities)?;
            }
        }
    }

    for entity in &knowledge.graph.entities {
        match &entity.properties {
            ExpressionEntityProperties::Expression(properties) => {
                if count(&has_expression, &entity.id) != 1
                    || count(&contains_expression, &entity.id)
                        != usize::from(properties.parent_expression_id.is_some())
                {
                    return Err(ExpressionBindingError::ParentInvalid);
                }
            }
            ExpressionEntityProperties::CallArgument(_) => {
                if count(&has_argument, &entity.id) != 1 || count(&argument_value, &entity.id) != 1
                {
                    return Err(ExpressionBindingError::ArgumentOrdinalInvalid);
                }
            }
            ExpressionEntityProperties::PatternBinding(properties) => {
                let requires_input = matches!(
                    properties.origin,
                    BindingOrigin::IfLet
                        | BindingOrigin::WhileLet
                        | BindingOrigin::For
                        | BindingOrigin::MatchArm
                );
                if count(&declares_binding, &entity.id) != 1
                    || count(&binds_from, &entity.id) > 1
                    || (requires_input && count(&binds_from, &entity.id) != 1)
                {
                    return Err(ExpressionBindingError::BindingScopeInvalid);
                }
            }
        }
    }
    if has_receiver.values().any(|count| *count > 1)
        || represents_call_site.values().any(|count| *count > 1)
        || represented_call_sites.values().any(|count| *count > 1)
    {
        return Err(ExpressionBindingError::CallSiteEvidenceMismatch);
    }
    Ok(())
}

fn validate_access_relationship(
    relationship: &ExpressionBindingRelationship,
    entities: &BTreeMap<&str, &ExpressionBindingEntity>,
) -> Result<(), ExpressionBindingError> {
    let source = expression_entity(entities, &relationship.source)
        .ok_or(ExpressionBindingError::AccessResolutionInvalid)?;
    let target = binding_entity(entities, &relationship.target)
        .ok_or(ExpressionBindingError::AccessResolutionInvalid)?;
    let ExpressionEntityProperties::Expression(expression) = &source.properties else {
        return Err(ExpressionBindingError::AccessResolutionInvalid);
    };
    let ExpressionEntityProperties::PatternBinding(binding) = &target.properties else {
        return Err(ExpressionBindingError::AccessResolutionInvalid);
    };
    if !matches!(expression.syntax_kind.as_str(), "identifier" | "self")
        || expression.token.as_deref() != Some(target.name.as_str())
        || source.callable_id != target.callable_id
        || source.locator.path != target.locator.path
        || source.locator.start_byte < binding.scope_start_byte
        || source.locator.end_byte > binding.scope_end_byte
        || !single_evidence(relationship, &source.evidence_id)
    {
        return Err(ExpressionBindingError::AccessResolutionInvalid);
    }
    let mut candidates = entities
        .values()
        .filter_map(|candidate| {
            let ExpressionEntityProperties::PatternBinding(properties) = &candidate.properties
            else {
                return None;
            };
            (candidate.callable_id == source.callable_id
                && candidate.name == target.name
                && candidate.locator.path == source.locator.path
                && source.locator.start_byte >= properties.scope_start_byte
                && source.locator.end_byte <= properties.scope_end_byte)
                .then_some((*candidate, properties))
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|(candidate, properties)| {
        (
            properties
                .scope_end_byte
                .saturating_sub(properties.scope_start_byte),
            std::cmp::Reverse(properties.scope_start_byte),
            candidate.id.as_str(),
        )
    });
    let Some((nearest, nearest_scope)) = candidates.first() else {
        return Err(ExpressionBindingError::AccessResolutionInvalid);
    };
    if candidates.get(1).is_some_and(|(_, other_scope)| {
        other_scope.scope_start_byte == nearest_scope.scope_start_byte
            && other_scope.scope_end_byte == nearest_scope.scope_end_byte
    }) || nearest.id != target.id
    {
        return Err(ExpressionBindingError::AccessResolutionInvalid);
    }
    let assignment_target = expression.roles.contains(&ExpressionRole::AssignmentTarget);
    let compound_target = expression
        .parent_expression_id
        .as_deref()
        .and_then(|identifier| entities.get(identifier).copied())
        .and_then(|parent| match &parent.properties {
            ExpressionEntityProperties::Expression(properties) => Some(properties),
            _ => None,
        })
        .is_some_and(|properties| properties.syntax_kind == "compound_assignment_expr");
    match relationship.kind {
        ExpressionRelationshipKind::Reads if assignment_target && !compound_target => {
            Err(ExpressionBindingError::AccessResolutionInvalid)
        }
        ExpressionRelationshipKind::Writes if !assignment_target => {
            Err(ExpressionBindingError::AccessResolutionInvalid)
        }
        ExpressionRelationshipKind::Reads | ExpressionRelationshipKind::Writes => Ok(()),
        _ => Err(ExpressionBindingError::AccessResolutionInvalid),
    }
}

fn expression_entity<'a>(
    entities: &'a BTreeMap<&str, &'a ExpressionBindingEntity>,
    identifier: &str,
) -> Option<&'a ExpressionBindingEntity> {
    entities.get(identifier).copied().filter(|entity| {
        entity.kind == ExpressionEntityKind::Expression
            && matches!(entity.properties, ExpressionEntityProperties::Expression(_))
    })
}

fn call_expression<'a>(
    entities: &'a BTreeMap<&str, &'a ExpressionBindingEntity>,
    identifier: &str,
) -> Option<&'a ExpressionBindingEntity> {
    expression_entity(entities, identifier).filter(|entity| {
        matches!(
            &entity.properties,
            ExpressionEntityProperties::Expression(properties)
                if properties.syntax_kind == "call_expression"
        )
    })
}

fn argument_entity<'a>(
    entities: &'a BTreeMap<&str, &'a ExpressionBindingEntity>,
    identifier: &str,
) -> Option<&'a ExpressionBindingEntity> {
    entities.get(identifier).copied().filter(|entity| {
        entity.kind == ExpressionEntityKind::CallArgument
            && matches!(
                entity.properties,
                ExpressionEntityProperties::CallArgument(_)
            )
    })
}

fn binding_entity<'a>(
    entities: &'a BTreeMap<&str, &'a ExpressionBindingEntity>,
    identifier: &str,
) -> Option<&'a ExpressionBindingEntity> {
    entities.get(identifier).copied().filter(|entity| {
        entity.kind == ExpressionEntityKind::PatternBinding
            && matches!(
                entity.properties,
                ExpressionEntityProperties::PatternBinding(_)
            )
    })
}

fn single_evidence(relationship: &ExpressionBindingRelationship, evidence_id: &str) -> bool {
    relationship.evidence_ids.as_slice() == [evidence_id]
}

fn canonical_evidence_ids(left: &str, right: &str) -> Vec<String> {
    let mut values = vec![left.to_owned(), right.to_owned()];
    values.sort();
    values.dedup();
    values
}

fn increment<'a>(counts: &mut BTreeMap<&'a str, usize>, identifier: &'a str) {
    *counts.entry(identifier).or_default() += 1;
}

fn count(counts: &BTreeMap<&str, usize>, identifier: &str) -> usize {
    counts.get(identifier).copied().unwrap_or_default()
}

fn validate_parent_depths(
    entities: &[ExpressionBindingEntity],
    relationships: &[ExpressionBindingRelationship],
) -> Result<(), ExpressionBindingError> {
    let expressions = entities
        .iter()
        .filter_map(|entity| match &entity.properties {
            ExpressionEntityProperties::Expression(properties) => {
                Some((entity.id.as_str(), properties))
            }
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let mut parents = BTreeMap::new();
    for relationship in relationships
        .iter()
        .filter(|relationship| relationship.kind == ExpressionRelationshipKind::ContainsExpression)
    {
        if parents
            .insert(relationship.target.as_str(), relationship.source.as_str())
            .is_some()
        {
            return Err(ExpressionBindingError::ParentInvalid);
        }
    }
    for (identifier, properties) in &expressions {
        if properties.parent_expression_id.as_deref() != parents.get(identifier).copied() {
            return Err(ExpressionBindingError::ParentInvalid);
        }
        let mut depth = 0_u64;
        let mut parent = properties.parent_expression_id.as_deref();
        let mut seen = BTreeSet::new();
        while let Some(identifier) = parent {
            if !seen.insert(identifier) {
                return Err(ExpressionBindingError::ParentInvalid);
            }
            depth = depth.saturating_add(1);
            if depth > MAX_R14_EXPRESSION_DEPTH {
                return Err(ExpressionBindingError::ParentInvalid);
            }
            parent = expressions
                .get(identifier)
                .ok_or(ExpressionBindingError::ParentInvalid)?
                .parent_expression_id
                .as_deref();
        }
        if depth != properties.lexical_depth {
            return Err(ExpressionBindingError::ParentInvalid);
        }
    }
    Ok(())
}

fn validate_argument_ordinals(
    entities: &[ExpressionBindingEntity],
) -> Result<(), ExpressionBindingError> {
    let mut by_call = BTreeMap::<&str, Vec<u64>>::new();
    for entity in entities {
        if let ExpressionEntityProperties::CallArgument(properties) = &entity.properties {
            by_call
                .entry(properties.call_expression_id.as_str())
                .or_default()
                .push(properties.ordinal);
        }
    }
    for ordinals in by_call.values_mut() {
        ordinals.sort_unstable();
        enforce_expression_limit(ExpressionBindingLimit::ArgumentsPerCall, ordinals.len())?;
        if ordinals
            .iter()
            .copied()
            .ne(0_u64..u64::try_from(ordinals.len()).unwrap_or(u64::MAX))
        {
            return Err(ExpressionBindingError::ArgumentOrdinalInvalid);
        }
    }
    Ok(())
}

fn validate_chunk_union(
    knowledge: &ExpressionBindingKnowledge,
) -> Result<(), ExpressionBindingError> {
    let chunk_entities = knowledge
        .extraction_chunks
        .iter()
        .flat_map(|chunk| chunk.entities.iter())
        .map(|value| (&value.id, value))
        .collect::<BTreeMap<_, _>>();
    let chunk_relationships = knowledge
        .extraction_chunks
        .iter()
        .flat_map(|chunk| chunk.relationships.iter())
        .map(|value| (&value.id, value))
        .collect::<BTreeMap<_, _>>();
    let chunk_claims = knowledge
        .extraction_chunks
        .iter()
        .flat_map(|chunk| chunk.claims.iter())
        .map(|value| (&value.id, value))
        .collect::<BTreeMap<_, _>>();
    let chunk_evidence = knowledge
        .extraction_chunks
        .iter()
        .flat_map(|chunk| chunk.evidence.iter())
        .map(|value| (&value.id, value))
        .collect::<BTreeMap<_, _>>();
    let chunk_coverage = knowledge
        .extraction_chunks
        .iter()
        .flat_map(|chunk| chunk.coverage.iter())
        .map(|value| (&value.id, value))
        .collect::<BTreeMap<_, _>>();
    if chunk_entities.len() != knowledge.graph.entities.len()
        || chunk_relationships.len() != knowledge.graph.relationships.len()
        || chunk_claims.len() != knowledge.graph.claims.len()
        || chunk_evidence.len() != knowledge.graph.evidence.len()
        || chunk_coverage.len() != knowledge.graph.coverage.len()
        || knowledge
            .graph
            .entities
            .iter()
            .any(|value| chunk_entities.get(&value.id) != Some(&value))
        || knowledge
            .graph
            .relationships
            .iter()
            .any(|value| chunk_relationships.get(&value.id) != Some(&value))
        || knowledge
            .graph
            .claims
            .iter()
            .any(|value| chunk_claims.get(&value.id) != Some(&value))
        || knowledge
            .graph
            .evidence
            .iter()
            .any(|value| chunk_evidence.get(&value.id) != Some(&value))
        || knowledge
            .graph
            .coverage
            .iter()
            .any(|value| chunk_coverage.get(&value.id) != Some(&value))
    {
        return Err(ExpressionBindingError::ContractInvalid);
    }
    Ok(())
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

fn validate_ordered_identities<'a>(
    values: impl IntoIterator<Item = &'a str>,
) -> Result<(), ExpressionBindingError> {
    let mut previous = None;
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(ExpressionBindingError::IdentityConflict);
        }
        if previous.is_some_and(|last| last > value) {
            return Err(ExpressionBindingError::ContractInvalid);
        }
        previous = Some(value);
    }
    Ok(())
}
