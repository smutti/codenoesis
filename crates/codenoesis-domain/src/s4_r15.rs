use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::knowledge::{ClaimState, ClaimSubjectKind};
use crate::s4::{WorkspaceClaim, WorkspaceEvidence, workspace_claim_id};
use crate::s4_k1::CallableSemanticEntityKind;
use crate::s4_r14::{
    ExpressionBindingError, ExpressionBindingExtraction, ExpressionBindingKnowledge,
    ExpressionEntityKind, ExpressionRelationshipKind,
};
use crate::s5::AnalysisCacheEntry;

pub const R15_CONFIGURATION_VERSION: &str = "codenoesis.configuration/v14";
pub const R15_SNAPSHOT_VERSION: &str = "codenoesis.repository-snapshot/v17";
pub const R15_EXTRACTION_CHUNK_VERSION: &str = "codenoesis.extraction-chunk/v14";
pub const R15_GRAPH_VERSION: &str = "codenoesis.knowledge-graph/v14";
pub const R15_ONTOLOGY_VERSION: &str = "codenoesis.ontology/rust/v14";
pub const R15_PIPELINE_VERSION: &str = "codenoesis.pipeline/s4-r15-v1";
pub const R15_EXTRACTION_CONTRACT_VERSION: &str = "codenoesis.extraction/v14";
pub const R15_EXTRACTOR_VERSION: &str = "codenoesis.rust-local-flow/s4-r15-v1";
pub const R15_INDEX_VERSION: &str = "codenoesis.local-flow-index/v1";
pub const R15_RULE_VERSION: &str = "codenoesis.rule/rust-local-flow/v1";
pub const R15_SEMANTIC_HASH_CONTRACT_VERSION: &str = "codenoesis.semantic-hash-contract/v13";
pub const R15_ERROR_VERSION: &str = "codenoesis.error/v22";
pub const R15_QUERY_VERSION: &str = "codenoesis.local-query-result/v12";
pub const R15_PORTABLE_GRAPH_VERSION: &str = "codenoesis.portable-graph/v8";
pub const R15_LOCAL_EXPLORER_VERSION: &str = "codenoesis.local-explorer/v8";
pub const R15_PROFILE: &str = "rust-local-flow-v1";

pub const MAX_R15_BLOCKS_PER_CALLABLE: u64 = 4_096;
pub const MAX_R15_NESTED_BRANCHES: u64 = 64;
pub const MAX_R15_FLOW_NODES_PER_BLOCK: u64 = 4_096;
pub const MAX_R15_REACHABILITY_PAIRS_PER_CALLABLE: u64 = 262_144;
pub const MAX_R15_BLOCKS: u64 = 200_000;
pub const MAX_R15_RELATIONSHIPS: u64 = 1_000_000;
pub const MAX_R15_DERIVATION_INPUT_REFERENCES: u64 = 1_000_000;

const ENTITY_ID_DOMAIN: &str = "codenoesis.entity-id/rust-local-flow/v1";
const RELATIONSHIP_ID_DOMAIN: &str = "codenoesis.relationship-id/rust-local-flow/v1";
const EVIDENCE_ID_DOMAIN: &str = "codenoesis.evidence-id/rust-local-flow/v1";
const COVERAGE_ID_DOMAIN: &str = "codenoesis.coverage-gap-id/rust-local-flow/v1";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LocalFlowBlockRole {
    Entry,
    Condition,
    ThenBranch,
    ElseBranch,
    Join,
}

impl LocalFlowBlockRole {
    pub const ALL: [Self; 5] = [
        Self::Entry,
        Self::Condition,
        Self::ThenBranch,
        Self::ElseBranch,
        Self::Join,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Entry => "entry",
            Self::Condition => "condition",
            Self::ThenBranch => "then_branch",
            Self::ElseBranch => "else_branch",
            Self::Join => "join",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LocalFlowRelationshipKind {
    HasSyntaxBlock,
    ContainsFlowNode,
    HasCondition,
    SyntaxNext,
    SyntaxTrueBranch,
    SyntaxFalseBranch,
    SyntaxReaches,
    LexicalMustReachesRead,
    LexicalMayReachesRead,
}

impl LocalFlowRelationshipKind {
    pub const ALL: [Self; 9] = [
        Self::HasSyntaxBlock,
        Self::ContainsFlowNode,
        Self::HasCondition,
        Self::SyntaxNext,
        Self::SyntaxTrueBranch,
        Self::SyntaxFalseBranch,
        Self::SyntaxReaches,
        Self::LexicalMustReachesRead,
        Self::LexicalMayReachesRead,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HasSyntaxBlock => "HAS_SYNTAX_BLOCK",
            Self::ContainsFlowNode => "CONTAINS_FLOW_NODE",
            Self::HasCondition => "HAS_CONDITION",
            Self::SyntaxNext => "SYNTAX_NEXT",
            Self::SyntaxTrueBranch => "SYNTAX_TRUE_BRANCH",
            Self::SyntaxFalseBranch => "SYNTAX_FALSE_BRANCH",
            Self::SyntaxReaches => "SYNTAX_REACHES",
            Self::LexicalMustReachesRead => "LEXICAL_MUST_REACHES_READ",
            Self::LexicalMayReachesRead => "LEXICAL_MAY_REACHES_READ",
        }
    }

    #[must_use]
    pub const fn claim_state(self) -> ClaimState {
        match self {
            Self::SyntaxReaches | Self::LexicalMustReachesRead | Self::LexicalMayReachesRead => {
                ClaimState::DerivedFact
            }
            Self::HasSyntaxBlock
            | Self::ContainsFlowNode
            | Self::HasCondition
            | Self::SyntaxNext
            | Self::SyntaxTrueBranch
            | Self::SyntaxFalseBranch => ClaimState::DeterministicFact,
        }
    }

    #[must_use]
    pub const fn is_direct_syntax(self) -> bool {
        matches!(
            self,
            Self::SyntaxNext | Self::SyntaxTrueBranch | Self::SyntaxFalseBranch
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalFlowLocator {
    pub path: String,
    pub blob_oid: String,
    pub start_byte: u64,
    pub end_byte: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxBasicBlock {
    pub id: String,
    pub callable_id: String,
    pub source_file_id: String,
    pub evidence_id: String,
    pub locator: LocalFlowLocator,
    pub ordinal: u64,
    pub role: LocalFlowBlockRole,
    pub flow_node_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalFlowRelationship {
    pub id: String,
    pub kind: LocalFlowRelationshipKind,
    pub source: String,
    pub target: String,
    pub evidence_ids: Vec<String>,
}

impl LocalFlowRelationship {
    #[must_use]
    pub fn new(
        kind: LocalFlowRelationshipKind,
        source: String,
        target: String,
        mut evidence_ids: Vec<String>,
    ) -> Self {
        evidence_ids.sort();
        evidence_ids.dedup();
        Self {
            id: local_flow_relationship_id(kind, &source, &target),
            kind,
            source,
            target,
            evidence_ids,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalFlowDerivation {
    pub relationship_id: String,
    pub input_entity_ids: Vec<String>,
    pub input_relationship_ids: Vec<String>,
    pub input_evidence_ids: Vec<String>,
}

impl LocalFlowDerivation {
    #[must_use]
    pub fn new(
        relationship_id: String,
        mut input_entity_ids: Vec<String>,
        mut input_relationship_ids: Vec<String>,
        mut input_evidence_ids: Vec<String>,
    ) -> Self {
        input_entity_ids.sort();
        input_entity_ids.dedup();
        input_relationship_ids.sort();
        input_relationship_ids.dedup();
        input_evidence_ids.sort();
        input_evidence_ids.dedup();
        Self {
            relationship_id,
            input_entity_ids,
            input_relationship_ids,
            input_evidence_ids,
        }
    }

    #[must_use]
    pub fn input_count(&self) -> usize {
        self.input_entity_ids
            .len()
            .saturating_add(self.input_relationship_ids.len())
            .saturating_add(self.input_evidence_ids.len())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalFlowCoverageGap {
    pub id: String,
    pub capability: String,
    pub state: String,
    pub subject_id: String,
    pub evidence_ids: Vec<String>,
}

impl LocalFlowCoverageGap {
    #[must_use]
    pub fn unsupported(
        capability: &str,
        subject_id: String,
        mut evidence_ids: Vec<String>,
    ) -> Self {
        evidence_ids.sort();
        evidence_ids.dedup();
        let state = "unsupported".to_owned();
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalFlowSourceChunk {
    pub crate_id: String,
    pub source_file_id: String,
    pub path: String,
    pub blocks: Vec<SyntaxBasicBlock>,
    pub relationships: Vec<LocalFlowRelationship>,
    pub claims: Vec<WorkspaceClaim>,
    pub evidence: Vec<WorkspaceEvidence>,
    pub coverage: Vec<LocalFlowCoverageGap>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LocalFlowIndex {
    pub completed_callable_ids: Vec<String>,
    pub block_entity_ids: Vec<String>,
    pub flow_node_relationship_ids: Vec<String>,
    pub condition_relationship_ids: Vec<String>,
    pub direct_syntax_relationship_ids: Vec<String>,
    pub reachability_relationship_ids: Vec<String>,
    pub must_reach_relationship_ids: Vec<String>,
    pub may_reach_relationship_ids: Vec<String>,
    pub derivations: Vec<LocalFlowDerivation>,
}

impl LocalFlowIndex {
    #[must_use]
    pub fn from_graph(
        completed_callable_ids: impl IntoIterator<Item = String>,
        blocks: &[SyntaxBasicBlock],
        relationships: &[LocalFlowRelationship],
        mut derivations: Vec<LocalFlowDerivation>,
    ) -> Self {
        let mut completed_callable_ids = completed_callable_ids.into_iter().collect::<Vec<_>>();
        completed_callable_ids.sort();
        completed_callable_ids.dedup();
        let relationship_ids = |predicate: fn(LocalFlowRelationshipKind) -> bool| {
            relationships
                .iter()
                .filter(|relationship| predicate(relationship.kind))
                .map(|relationship| relationship.id.clone())
                .collect::<Vec<_>>()
        };
        derivations.sort_by(|left, right| left.relationship_id.cmp(&right.relationship_id));
        Self {
            completed_callable_ids,
            block_entity_ids: blocks.iter().map(|block| block.id.clone()).collect(),
            flow_node_relationship_ids: relationship_ids(|kind| {
                kind == LocalFlowRelationshipKind::ContainsFlowNode
            }),
            condition_relationship_ids: relationship_ids(|kind| {
                kind == LocalFlowRelationshipKind::HasCondition
            }),
            direct_syntax_relationship_ids: relationship_ids(
                LocalFlowRelationshipKind::is_direct_syntax,
            ),
            reachability_relationship_ids: relationship_ids(|kind| {
                kind == LocalFlowRelationshipKind::SyntaxReaches
            }),
            must_reach_relationship_ids: relationship_ids(|kind| {
                kind == LocalFlowRelationshipKind::LexicalMustReachesRead
            }),
            may_reach_relationship_ids: relationship_ids(|kind| {
                kind == LocalFlowRelationshipKind::LexicalMayReachesRead
            }),
            derivations,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalFlowGraph {
    pub blocks: Vec<SyntaxBasicBlock>,
    pub relationships: Vec<LocalFlowRelationship>,
    pub claims: Vec<WorkspaceClaim>,
    pub evidence: Vec<WorkspaceEvidence>,
    pub coverage: Vec<LocalFlowCoverageGap>,
    pub index: LocalFlowIndex,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalFlowKnowledge {
    pub expression: ExpressionBindingKnowledge,
    pub extraction_chunks: Vec<LocalFlowSourceChunk>,
    pub graph: LocalFlowGraph,
}

impl LocalFlowKnowledge {
    /// Validates the exact R14 lineage and the additive closed R15 graph.
    ///
    /// # Errors
    ///
    /// Returns the first identity, reference, closure, derivation, ordering, or limit failure.
    #[allow(clippy::too_many_lines)]
    pub fn validate(&self) -> Result<(), LocalFlowError> {
        self.expression.validate().map_err(LocalFlowError::Source)?;
        ordered_unique(self.graph.blocks.iter().map(|value| value.id.as_str()))?;
        ordered_unique(
            self.graph
                .relationships
                .iter()
                .map(|value| value.id.as_str()),
        )?;
        ordered_unique(self.graph.claims.iter().map(|value| value.id.as_str()))?;
        ordered_unique(self.graph.evidence.iter().map(|value| value.id.as_str()))?;
        ordered_unique(self.graph.coverage.iter().map(|value| value.id.as_str()))?;
        enforce_local_flow_limit(LocalFlowLimit::BlocksTotal, self.graph.blocks.len())?;
        enforce_local_flow_limit(
            LocalFlowLimit::RelationshipsTotal,
            self.graph.relationships.len(),
        )?;

        let repository_id = self
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
        let callable_ids = self
            .expression
            .callable
            .graph
            .entities
            .iter()
            .filter(|entity| entity.kind == CallableSemanticEntityKind::Signature)
            .map(|entity| entity.subject_id.as_str())
            .collect::<BTreeSet<_>>();
        let inherited_entity_ids = self
            .expression
            .callable
            .graph
            .entities
            .iter()
            .map(|entity| entity.id.as_str())
            .chain(
                self.expression
                    .graph
                    .entities
                    .iter()
                    .map(|entity| entity.id.as_str()),
            )
            .collect::<BTreeSet<_>>();
        let mut evidence = self
            .expression
            .callable
            .graph
            .evidence
            .iter()
            .chain(&self.expression.graph.evidence)
            .map(|value| (value.id.as_str(), value))
            .collect::<BTreeMap<_, _>>();
        for value in &self.graph.evidence {
            if evidence.insert(value.id.as_str(), value).is_some() {
                return Err(LocalFlowError::IdentityConflict);
            }
        }
        let mut blocks_per_callable = BTreeMap::<&str, usize>::new();
        let mut ordinals = BTreeSet::new();
        let mut spans = BTreeSet::new();
        let block_ids = self
            .graph
            .blocks
            .iter()
            .map(|block| block.id.as_str())
            .collect::<BTreeSet<_>>();
        for block in &self.graph.blocks {
            if !callable_ids.contains(block.callable_id.as_str())
                || block.id
                    != syntax_basic_block_id(
                        repository_id,
                        &block.callable_id,
                        &block.source_file_id,
                        block.locator.start_byte,
                        block.locator.end_byte,
                        block.role,
                        block.ordinal,
                    )
                || block.flow_node_ids.is_empty()
                || block.flow_node_ids.len()
                    > usize::try_from(MAX_R15_FLOW_NODES_PER_BLOCK).unwrap_or(usize::MAX)
                || block
                    .flow_node_ids
                    .iter()
                    .any(|identifier| !inherited_entity_ids.contains(identifier.as_str()))
                || !valid_block_evidence(block, &evidence)
                || !ordinals.insert((block.callable_id.as_str(), block.ordinal))
                || !spans.insert((
                    block.callable_id.as_str(),
                    block.locator.path.as_str(),
                    block.locator.start_byte,
                    block.locator.end_byte,
                ))
            {
                return Err(LocalFlowError::BlockInvalid);
            }
            *blocks_per_callable
                .entry(block.callable_id.as_str())
                .or_default() += 1;
        }
        for observed in blocks_per_callable.values().copied() {
            enforce_local_flow_limit(LocalFlowLimit::BlocksPerCallable, observed)?;
        }

        let inherited_relationship_ids = self
            .expression
            .graph
            .relationships
            .iter()
            .map(|relationship| relationship.id.as_str())
            .collect::<BTreeSet<_>>();
        let relationship_ids = self
            .graph
            .relationships
            .iter()
            .map(|relationship| relationship.id.as_str())
            .collect::<BTreeSet<_>>();
        let all_endpoints = callable_ids
            .iter()
            .copied()
            .chain(inherited_entity_ids.iter().copied())
            .chain(block_ids.iter().copied())
            .collect::<BTreeSet<_>>();
        for relationship in &self.graph.relationships {
            if relationship.id
                != local_flow_relationship_id(
                    relationship.kind,
                    &relationship.source,
                    &relationship.target,
                )
                || relationship.evidence_ids.is_empty()
                || relationship
                    .evidence_ids
                    .iter()
                    .any(|identifier| !evidence.contains_key(identifier.as_str()))
                || !all_endpoints.contains(relationship.source.as_str())
                || !all_endpoints.contains(relationship.target.as_str())
            {
                return Err(LocalFlowError::EdgeInvalid);
            }
            validate_edge_shape(
                relationship,
                &callable_ids,
                &block_ids,
                &inherited_entity_ids,
            )?;
        }
        let entity_evidence = local_flow_entity_evidence(self)?;
        validate_relationship_evidence(
            &self.graph.blocks,
            &self.graph.relationships,
            &entity_evidence,
        )?;
        validate_block_membership(&self.graph.blocks, &self.graph.relationships)?;
        validate_condition_and_branch_edges(
            &self.expression,
            &self.graph.blocks,
            &self.graph.relationships,
        )?;
        validate_syntax_closure(&self.graph.blocks, &self.graph.relationships)?;
        validate_lexical_relationships(&self.expression, &self.graph.relationships)?;
        validate_local_flow_coverage(
            &self.graph.coverage,
            &self.graph.index.completed_callable_ids,
            &callable_ids,
            &evidence,
        )?;

        let subjects = block_ids
            .iter()
            .map(|identifier| (ClaimSubjectKind::Entity, *identifier))
            .chain(
                relationship_ids
                    .iter()
                    .map(|identifier| (ClaimSubjectKind::Relationship, *identifier)),
            )
            .collect::<BTreeSet<_>>();
        if self.graph.claims.len() != subjects.len() {
            return Err(LocalFlowError::ContractInvalid);
        }
        let relationship_states = self
            .graph
            .relationships
            .iter()
            .map(|relationship| (relationship.id.as_str(), relationship.kind.claim_state()))
            .collect::<BTreeMap<_, _>>();
        for claim in &self.graph.claims {
            let expected_state = if block_ids.contains(claim.subject_id.as_str()) {
                ClaimState::DeterministicFact
            } else {
                relationship_states
                    .get(claim.subject_id.as_str())
                    .copied()
                    .ok_or(LocalFlowError::ContractInvalid)?
            };
            if !subjects.contains(&(claim.subject_kind, claim.subject_id.as_str()))
                || claim.state != expected_state
                || claim.id
                    != workspace_claim_id(claim.subject_kind, &claim.subject_id, claim.state)
                || claim.evidence_ids
                    != expected_claim_evidence(
                        claim.subject_kind,
                        &claim.subject_id,
                        &self.graph.blocks,
                        &self.graph.relationships,
                    )?
            {
                return Err(LocalFlowError::ContractInvalid);
            }
        }

        let derived_ids = self
            .graph
            .relationships
            .iter()
            .filter(|relationship| relationship.kind.claim_state() == ClaimState::DerivedFact)
            .map(|relationship| relationship.id.as_str())
            .collect::<BTreeSet<_>>();
        let derivation_ids = self
            .graph
            .index
            .derivations
            .iter()
            .map(|derivation| derivation.relationship_id.as_str())
            .collect::<BTreeSet<_>>();
        if derived_ids != derivation_ids {
            return Err(LocalFlowError::DerivationMismatch);
        }
        let mut derivation_input_count = 0_usize;
        for derivation in &self.graph.index.derivations {
            derivation_input_count =
                derivation_input_count.saturating_add(derivation.input_count());
            if derivation.input_entity_ids.is_empty()
                || derivation.input_evidence_ids.is_empty()
                || derivation
                    .input_entity_ids
                    .iter()
                    .any(|identifier| !all_endpoints.contains(identifier.as_str()))
                || derivation.input_relationship_ids.iter().any(|identifier| {
                    !relationship_ids.contains(identifier.as_str())
                        && !inherited_relationship_ids.contains(identifier.as_str())
                })
                || derivation
                    .input_evidence_ids
                    .iter()
                    .any(|identifier| !evidence.contains_key(identifier.as_str()))
                || !strictly_ordered(&derivation.input_entity_ids)
                || !strictly_ordered(&derivation.input_relationship_ids)
                || !strictly_ordered(&derivation.input_evidence_ids)
            {
                return Err(LocalFlowError::DerivationMismatch);
            }
            validate_derivation_semantics(
                derivation,
                &self.expression,
                &self.graph.blocks,
                &self.graph.relationships,
                &entity_evidence,
            )?;
        }
        enforce_local_flow_limit(
            LocalFlowLimit::DerivationInputReferences,
            derivation_input_count,
        )?;
        let expected_index = LocalFlowIndex::from_graph(
            self.graph.index.completed_callable_ids.clone(),
            &self.graph.blocks,
            &self.graph.relationships,
            self.graph.index.derivations.clone(),
        );
        if self.graph.index != expected_index
            || self
                .graph
                .index
                .completed_callable_ids
                .iter()
                .any(|identifier| !callable_ids.contains(identifier.as_str()))
            || self
                .graph
                .blocks
                .iter()
                .map(|block| block.callable_id.as_str())
                .collect::<BTreeSet<_>>()
                != self
                    .graph
                    .index
                    .completed_callable_ids
                    .iter()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>()
        {
            return Err(LocalFlowError::IndexMismatch);
        }
        validate_chunk_union(self)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalFlowExtraction {
    pub knowledge: LocalFlowKnowledge,
    pub cache_entries: Vec<AnalysisCacheEntry>,
    pub parser_invocation_count: u64,
}

impl LocalFlowExtraction {
    #[must_use]
    pub fn from_r14(
        source: ExpressionBindingExtraction,
        extraction_chunks: Vec<LocalFlowSourceChunk>,
        graph: LocalFlowGraph,
        parser_invocation_count: u64,
    ) -> Self {
        Self {
            knowledge: LocalFlowKnowledge {
                expression: source.knowledge,
                extraction_chunks,
                graph,
            },
            cache_entries: source.cache_entries,
            parser_invocation_count,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalFlowLimit {
    BlocksPerCallable,
    NestedBranches,
    FlowNodesPerBlock,
    ReachabilityPairsPerCallable,
    BlocksTotal,
    RelationshipsTotal,
    DerivationInputReferences,
}

impl LocalFlowLimit {
    #[must_use]
    pub const fn maximum(self) -> u64 {
        match self {
            Self::BlocksPerCallable => MAX_R15_BLOCKS_PER_CALLABLE,
            Self::NestedBranches => MAX_R15_NESTED_BRANCHES,
            Self::FlowNodesPerBlock => MAX_R15_FLOW_NODES_PER_BLOCK,
            Self::ReachabilityPairsPerCallable => MAX_R15_REACHABILITY_PAIRS_PER_CALLABLE,
            Self::BlocksTotal => MAX_R15_BLOCKS,
            Self::RelationshipsTotal => MAX_R15_RELATIONSHIPS,
            Self::DerivationInputReferences => MAX_R15_DERIVATION_INPUT_REFERENCES,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BlocksPerCallable => "syntax_blocks_per_callable",
            Self::NestedBranches => "nested_explicit_branches",
            Self::FlowNodesPerBlock => "direct_flow_nodes_per_block",
            Self::ReachabilityPairsPerCallable => "strict_reachability_pairs_per_callable",
            Self::BlocksTotal => "syntax_blocks_total",
            Self::RelationshipsTotal => "local_flow_relationships_total",
            Self::DerivationInputReferences => "derivation_input_references",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LocalFlowError {
    Source(ExpressionBindingError),
    InvalidSyntax {
        path: String,
        start_byte: u64,
    },
    IdentityConflict,
    BlockInvalid,
    EdgeInvalid,
    Cycle,
    ReachabilityMismatch,
    AccessMismatch,
    DerivationMismatch,
    IndexMismatch,
    LimitExceeded {
        limit: LocalFlowLimit,
        maximum: u64,
        observed: u64,
    },
    ContractInvalid,
}

impl Display for LocalFlowError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Source(_) => "R15 expression source extraction failed",
            Self::InvalidSyntax { .. } => "invalid R15 Rust source",
            Self::IdentityConflict => "R15 local-flow identity conflict",
            Self::BlockInvalid => "R15 syntax block is invalid",
            Self::EdgeInvalid => "R15 local-flow edge is invalid",
            Self::Cycle => "R15 syntax flow contains a cycle",
            Self::ReachabilityMismatch => "R15 strict reachability does not match direct flow",
            Self::AccessMismatch => "R15 lexical access does not match R14",
            Self::DerivationMismatch => "R15 derivation does not match its inputs",
            Self::IndexMismatch => "R15 local-flow index does not match the graph",
            Self::LimitExceeded { .. } => "R15 local-flow limit exceeded",
            Self::ContractInvalid => "R15 local-flow contract is invalid",
        })
    }
}

impl Error for LocalFlowError {}

#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn syntax_basic_block_id(
    repository_id: &str,
    callable_id: &str,
    source_file_id: &str,
    start_byte: u64,
    end_byte: u64,
    role: LocalFlowBlockRole,
    ordinal: u64,
) -> String {
    stable_id(
        "urn:codenoesis:entity:blake3:",
        &[
            ENTITY_ID_DOMAIN,
            "syntax_basic_block",
            repository_id,
            callable_id,
            source_file_id,
            &start_byte.to_string(),
            &end_byte.to_string(),
            role.as_str(),
            &ordinal.to_string(),
        ],
    )
}

#[must_use]
pub fn local_flow_relationship_id(
    kind: LocalFlowRelationshipKind,
    source: &str,
    target: &str,
) -> String {
    stable_id(
        "urn:codenoesis:relationship:blake3:",
        &[RELATIONSHIP_ID_DOMAIN, kind.as_str(), source, target],
    )
}

#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn local_flow_evidence_id(
    repository_id: &str,
    commit_oid: &str,
    blob_oid: &str,
    path: &str,
    start_byte: u64,
    end_byte: u64,
) -> String {
    stable_id(
        "urn:codenoesis:evidence:blake3:",
        &[
            EVIDENCE_ID_DOMAIN,
            repository_id,
            commit_oid,
            blob_oid,
            path,
            &start_byte.to_string(),
            &end_byte.to_string(),
        ],
    )
}

#[must_use]
pub fn local_flow_claim(
    subject_kind: ClaimSubjectKind,
    subject_id: String,
    state: ClaimState,
    evidence_ids: Vec<String>,
) -> WorkspaceClaim {
    WorkspaceClaim::new(subject_kind, subject_id, state, evidence_ids)
}

/// Enforces one fixed R15 resource limit.
///
/// # Errors
///
/// Returns the exact maximum and capped maximum-plus-one observation when exceeded.
pub fn enforce_local_flow_limit(
    limit: LocalFlowLimit,
    observed: usize,
) -> Result<(), LocalFlowError> {
    let observed = u64::try_from(observed).unwrap_or(u64::MAX);
    let maximum = limit.maximum();
    if observed > maximum {
        Err(LocalFlowError::LimitExceeded {
            limit,
            maximum,
            observed: observed.min(maximum.saturating_add(1)),
        })
    } else {
        Ok(())
    }
}

fn valid_block_evidence(
    block: &SyntaxBasicBlock,
    evidence: &BTreeMap<&str, &WorkspaceEvidence>,
) -> bool {
    evidence
        .get(block.evidence_id.as_str())
        .is_some_and(|value| {
            value.path == block.locator.path
                && value.blob_oid == block.locator.blob_oid
                && value.start_byte == block.locator.start_byte
                && value.end_byte == block.locator.end_byte
                && value.start_byte < value.end_byte
        })
}

fn local_flow_entity_evidence(
    knowledge: &LocalFlowKnowledge,
) -> Result<BTreeMap<String, Vec<String>>, LocalFlowError> {
    let mut values = BTreeMap::new();
    for entity in &knowledge.expression.callable.graph.entities {
        insert_local_flow_entity_evidence(&mut values, &entity.id, entity.evidence_ids.clone())?;
    }
    for entity in &knowledge.expression.graph.entities {
        insert_local_flow_entity_evidence(
            &mut values,
            &entity.id,
            vec![entity.evidence_id.clone()],
        )?;
    }
    for block in &knowledge.graph.blocks {
        insert_local_flow_entity_evidence(&mut values, &block.id, vec![block.evidence_id.clone()])?;
    }
    Ok(values)
}

fn insert_local_flow_entity_evidence(
    values: &mut BTreeMap<String, Vec<String>>,
    identifier: &str,
    mut evidence_ids: Vec<String>,
) -> Result<(), LocalFlowError> {
    evidence_ids.sort();
    evidence_ids.dedup();
    if evidence_ids.is_empty() {
        return Err(LocalFlowError::ContractInvalid);
    }
    if let Some(existing) = values.insert(identifier.to_owned(), evidence_ids.clone())
        && existing != evidence_ids
    {
        return Err(LocalFlowError::IdentityConflict);
    }
    Ok(())
}

fn validate_relationship_evidence(
    blocks: &[SyntaxBasicBlock],
    relationships: &[LocalFlowRelationship],
    entity_evidence: &BTreeMap<String, Vec<String>>,
) -> Result<(), LocalFlowError> {
    let block_evidence = blocks
        .iter()
        .map(|block| (block.id.as_str(), block.evidence_id.as_str()))
        .collect::<BTreeMap<_, _>>();
    for relationship in relationships {
        let mut expected = match relationship.kind {
            LocalFlowRelationshipKind::HasSyntaxBlock => vec![
                block_evidence
                    .get(relationship.target.as_str())
                    .ok_or(LocalFlowError::EdgeInvalid)?
                    .to_string(),
            ],
            _ => [relationship.source.as_str(), relationship.target.as_str()]
                .into_iter()
                .flat_map(|identifier| {
                    entity_evidence
                        .get(identifier)
                        .into_iter()
                        .flatten()
                        .cloned()
                })
                .collect::<Vec<_>>(),
        };
        expected.sort();
        expected.dedup();
        if expected.is_empty() || relationship.evidence_ids != expected {
            return Err(LocalFlowError::EdgeInvalid);
        }
    }
    Ok(())
}

fn expected_claim_evidence(
    subject_kind: ClaimSubjectKind,
    subject_id: &str,
    blocks: &[SyntaxBasicBlock],
    relationships: &[LocalFlowRelationship],
) -> Result<Vec<String>, LocalFlowError> {
    match subject_kind {
        ClaimSubjectKind::Entity => blocks
            .iter()
            .find(|block| block.id == subject_id)
            .map(|block| vec![block.evidence_id.clone()])
            .ok_or(LocalFlowError::ContractInvalid),
        ClaimSubjectKind::Relationship => relationships
            .iter()
            .find(|relationship| relationship.id == subject_id)
            .map(|relationship| relationship.evidence_ids.clone())
            .ok_or(LocalFlowError::ContractInvalid),
    }
}

#[allow(clippy::too_many_lines)]
fn validate_condition_and_branch_edges(
    expression: &ExpressionBindingKnowledge,
    blocks: &[SyntaxBasicBlock],
    relationships: &[LocalFlowRelationship],
) -> Result<(), LocalFlowError> {
    let blocks_by_id = blocks
        .iter()
        .map(|block| (block.id.as_str(), block))
        .collect::<BTreeMap<_, _>>();
    let controls = expression
        .callable
        .graph
        .entities
        .iter()
        .filter(|entity| entity.kind == CallableSemanticEntityKind::Control)
        .map(|entity| (entity.id.as_str(), entity.subject_id.as_str()))
        .collect::<BTreeMap<_, _>>();
    let expressions = expression
        .graph
        .entities
        .iter()
        .filter(|entity| entity.kind == ExpressionEntityKind::Expression)
        .map(|entity| (entity.id.as_str(), entity.callable_id.as_str()))
        .collect::<BTreeMap<_, _>>();
    let direct = relationships
        .iter()
        .filter(|relationship| relationship.kind.is_direct_syntax())
        .collect::<Vec<_>>();
    for relationship in &direct {
        let source = blocks_by_id
            .get(relationship.source.as_str())
            .ok_or(LocalFlowError::EdgeInvalid)?;
        let target = blocks_by_id
            .get(relationship.target.as_str())
            .ok_or(LocalFlowError::EdgeInvalid)?;
        if source.callable_id != target.callable_id {
            return Err(LocalFlowError::EdgeInvalid);
        }
    }

    let condition_links = relationships
        .iter()
        .filter(|relationship| relationship.kind == LocalFlowRelationshipKind::HasCondition)
        .collect::<Vec<_>>();
    let mut linked_condition_blocks = BTreeMap::<&str, usize>::new();
    for relationship in condition_links {
        let callable_id = controls
            .get(relationship.source.as_str())
            .copied()
            .ok_or(LocalFlowError::EdgeInvalid)?;
        if expressions.get(relationship.target.as_str()).copied() != Some(callable_id) {
            return Err(LocalFlowError::EdgeInvalid);
        }
        let matching = blocks
            .iter()
            .filter(|block| {
                block.callable_id == callable_id
                    && block.role == LocalFlowBlockRole::Condition
                    && block.flow_node_ids.len() == 2
                    && block
                        .flow_node_ids
                        .iter()
                        .any(|identifier| identifier == &relationship.source)
                    && block
                        .flow_node_ids
                        .iter()
                        .any(|identifier| identifier == &relationship.target)
            })
            .collect::<Vec<_>>();
        if matching.len() != 1 {
            return Err(LocalFlowError::EdgeInvalid);
        }
        *linked_condition_blocks
            .entry(matching[0].id.as_str())
            .or_default() += 1;
    }

    for block in blocks {
        let incoming = direct
            .iter()
            .filter(|relationship| relationship.target == block.id)
            .count();
        let next = direct
            .iter()
            .filter(|relationship| {
                relationship.source == block.id
                    && relationship.kind == LocalFlowRelationshipKind::SyntaxNext
            })
            .count();
        let true_branches = direct
            .iter()
            .filter(|relationship| {
                relationship.source == block.id
                    && relationship.kind == LocalFlowRelationshipKind::SyntaxTrueBranch
            })
            .collect::<Vec<_>>();
        let false_branches = direct
            .iter()
            .filter(|relationship| {
                relationship.source == block.id
                    && relationship.kind == LocalFlowRelationshipKind::SyntaxFalseBranch
            })
            .collect::<Vec<_>>();
        let has_condition_count = linked_condition_blocks
            .get(block.id.as_str())
            .copied()
            .unwrap_or_default();
        if block.role == LocalFlowBlockRole::Condition {
            if next != 0
                || true_branches.len() != 1
                || false_branches.len() != 1
                || true_branches[0].target == false_branches[0].target
                || has_condition_count != 1
            {
                return Err(LocalFlowError::EdgeInvalid);
            }
        } else if next > 1
            || !true_branches.is_empty()
            || !false_branches.is_empty()
            || has_condition_count != 0
        {
            return Err(LocalFlowError::EdgeInvalid);
        }
        let is_first = blocks
            .iter()
            .filter(|candidate| candidate.callable_id == block.callable_id)
            .min_by_key(|candidate| candidate.ordinal)
            .is_some_and(|candidate| candidate.id == block.id);
        if (is_first && incoming != 0) || (!is_first && incoming == 0) {
            return Err(LocalFlowError::EdgeInvalid);
        }
    }
    Ok(())
}

fn validate_lexical_relationships(
    expression: &ExpressionBindingKnowledge,
    relationships: &[LocalFlowRelationship],
) -> Result<(), LocalFlowError> {
    let entities = expression
        .graph
        .entities
        .iter()
        .map(|entity| (entity.id.as_str(), entity))
        .collect::<BTreeMap<_, _>>();
    let reads = expression
        .graph
        .relationships
        .iter()
        .filter(|relationship| relationship.kind == ExpressionRelationshipKind::Reads)
        .map(|relationship| (relationship.source.as_str(), relationship))
        .collect::<BTreeMap<_, _>>();
    let writes = expression
        .graph
        .relationships
        .iter()
        .filter(|relationship| relationship.kind == ExpressionRelationshipKind::Writes)
        .map(|relationship| (relationship.source.as_str(), relationship))
        .collect::<BTreeMap<_, _>>();
    let mut pairs = BTreeSet::new();
    for relationship in relationships.iter().filter(|relationship| {
        matches!(
            relationship.kind,
            LocalFlowRelationshipKind::LexicalMustReachesRead
                | LocalFlowRelationshipKind::LexicalMayReachesRead
        )
    }) {
        let source = entities
            .get(relationship.source.as_str())
            .copied()
            .ok_or(LocalFlowError::AccessMismatch)?;
        let target = entities
            .get(relationship.target.as_str())
            .copied()
            .ok_or(LocalFlowError::AccessMismatch)?;
        let read = reads
            .get(relationship.target.as_str())
            .copied()
            .ok_or(LocalFlowError::AccessMismatch)?;
        let binding_id = if source.kind == ExpressionEntityKind::PatternBinding {
            source.id.as_str()
        } else {
            writes
                .get(source.id.as_str())
                .map(|write| write.target.as_str())
                .ok_or(LocalFlowError::AccessMismatch)?
        };
        if binding_id != read.target
            || source.callable_id != target.callable_id
            || source.locator.start_byte > target.locator.start_byte
            || !pairs.insert((relationship.source.as_str(), relationship.target.as_str()))
        {
            return Err(LocalFlowError::AccessMismatch);
        }
    }
    Ok(())
}

fn validate_local_flow_coverage(
    coverage: &[LocalFlowCoverageGap],
    completed_callable_ids: &[String],
    callable_ids: &BTreeSet<&str>,
    evidence: &BTreeMap<&str, &WorkspaceEvidence>,
) -> Result<(), LocalFlowError> {
    let allowed = BTreeSet::from([
        "rust.lexical_reaching_definitions_not_analyzed",
        "rust.syntax_normal_flow_not_analyzed",
    ]);
    let completed = completed_callable_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut grouped = BTreeMap::<&str, BTreeSet<&str>>::new();
    for gap in coverage {
        if !allowed.contains(gap.capability.as_str())
            || gap.state != "unsupported"
            || !callable_ids.contains(gap.subject_id.as_str())
            || completed.contains(gap.subject_id.as_str())
            || gap.evidence_ids.is_empty()
            || gap
                .evidence_ids
                .iter()
                .any(|identifier| !evidence.contains_key(identifier.as_str()))
            || gap
                != &LocalFlowCoverageGap::unsupported(
                    &gap.capability,
                    gap.subject_id.clone(),
                    gap.evidence_ids.clone(),
                )
        {
            return Err(LocalFlowError::ContractInvalid);
        }
        grouped
            .entry(gap.subject_id.as_str())
            .or_default()
            .insert(gap.capability.as_str());
    }
    if grouped
        .values()
        .any(|capabilities| capabilities != &allowed)
    {
        return Err(LocalFlowError::ContractInvalid);
    }
    Ok(())
}

fn validate_derivation_semantics(
    derivation: &LocalFlowDerivation,
    expression: &ExpressionBindingKnowledge,
    blocks: &[SyntaxBasicBlock],
    relationships: &[LocalFlowRelationship],
    entity_evidence: &BTreeMap<String, Vec<String>>,
) -> Result<(), LocalFlowError> {
    let relationship = relationships
        .iter()
        .find(|relationship| relationship.id == derivation.relationship_id)
        .ok_or(LocalFlowError::DerivationMismatch)?;
    if !derivation
        .input_entity_ids
        .iter()
        .any(|identifier| identifier == &relationship.source)
        || !derivation
            .input_entity_ids
            .iter()
            .any(|identifier| identifier == &relationship.target)
    {
        return Err(LocalFlowError::DerivationMismatch);
    }
    let mut expected_evidence = derivation
        .input_entity_ids
        .iter()
        .flat_map(|identifier| {
            entity_evidence
                .get(identifier)
                .into_iter()
                .flatten()
                .cloned()
        })
        .collect::<Vec<_>>();
    expected_evidence.sort();
    expected_evidence.dedup();
    if expected_evidence.is_empty() || derivation.input_evidence_ids != expected_evidence {
        return Err(LocalFlowError::DerivationMismatch);
    }
    match relationship.kind {
        LocalFlowRelationshipKind::SyntaxReaches => {
            validate_syntax_derivation(derivation, relationship, blocks, relationships)
        }
        LocalFlowRelationshipKind::LexicalMustReachesRead
        | LocalFlowRelationshipKind::LexicalMayReachesRead => {
            validate_lexical_derivation(derivation, relationship, expression, relationships)
        }
        _ => Err(LocalFlowError::DerivationMismatch),
    }
}

fn validate_syntax_derivation(
    derivation: &LocalFlowDerivation,
    relationship: &LocalFlowRelationship,
    blocks: &[SyntaxBasicBlock],
    relationships: &[LocalFlowRelationship],
) -> Result<(), LocalFlowError> {
    let direct = relationships
        .iter()
        .filter(|candidate| candidate.kind.is_direct_syntax())
        .collect::<Vec<_>>();
    let adjacency = direct.iter().fold(
        BTreeMap::<&str, Vec<&str>>::new(),
        |mut values, candidate| {
            values
                .entry(candidate.source.as_str())
                .or_default()
                .push(candidate.target.as_str());
            values
        },
    );
    let reverse = direct.iter().fold(
        BTreeMap::<&str, Vec<&str>>::new(),
        |mut values, candidate| {
            values
                .entry(candidate.target.as_str())
                .or_default()
                .push(candidate.source.as_str());
            values
        },
    );
    let forward = reachable_identifiers(relationship.source.as_str(), &adjacency);
    let backward = reachable_identifiers(relationship.target.as_str(), &reverse);
    let vertices = forward
        .intersection(&backward)
        .copied()
        .chain(std::iter::once(relationship.source.as_str()))
        .chain(std::iter::once(relationship.target.as_str()))
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let expected_entities = vertices.iter().cloned().collect::<Vec<_>>();
    let expected_relationships = direct
        .iter()
        .filter(|candidate| {
            vertices.contains(&candidate.source) && vertices.contains(&candidate.target)
        })
        .map(|candidate| candidate.id.clone())
        .collect::<Vec<_>>();
    let block_ids = blocks
        .iter()
        .map(|block| block.id.as_str())
        .collect::<BTreeSet<_>>();
    if vertices
        .iter()
        .any(|identifier| !block_ids.contains(identifier.as_str()))
        || derivation.input_entity_ids != expected_entities
        || derivation.input_relationship_ids != expected_relationships
    {
        return Err(LocalFlowError::DerivationMismatch);
    }
    Ok(())
}

fn validate_lexical_derivation(
    derivation: &LocalFlowDerivation,
    relationship: &LocalFlowRelationship,
    expression: &ExpressionBindingKnowledge,
    relationships: &[LocalFlowRelationship],
) -> Result<(), LocalFlowError> {
    let inherited = expression
        .graph
        .relationships
        .iter()
        .map(|candidate| (candidate.id.as_str(), candidate))
        .collect::<BTreeMap<_, _>>();
    let local = relationships
        .iter()
        .map(|candidate| (candidate.id.as_str(), candidate))
        .collect::<BTreeMap<_, _>>();
    let read = expression
        .graph
        .relationships
        .iter()
        .find(|candidate| {
            candidate.kind == ExpressionRelationshipKind::Reads
                && candidate.source == relationship.target
        })
        .ok_or(LocalFlowError::AccessMismatch)?;
    if !derivation
        .input_relationship_ids
        .iter()
        .any(|identifier| identifier == &read.id)
    {
        return Err(LocalFlowError::AccessMismatch);
    }
    let write = expression.graph.relationships.iter().find(|candidate| {
        candidate.kind == ExpressionRelationshipKind::Writes
            && candidate.source == relationship.source
    });
    if write.is_some_and(|write| {
        !derivation
            .input_relationship_ids
            .iter()
            .any(|identifier| identifier == &write.id)
    }) {
        return Err(LocalFlowError::AccessMismatch);
    }
    let input_entities = derivation
        .input_entity_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    for identifier in &derivation.input_relationship_ids {
        if let Some(candidate) = inherited.get(identifier.as_str()) {
            let valid = match candidate.kind {
                ExpressionRelationshipKind::Reads => {
                    candidate.source == relationship.target && candidate.target == read.target
                }
                ExpressionRelationshipKind::Writes => {
                    candidate.target == read.target
                        && input_entities.contains(candidate.source.as_str())
                }
                _ => false,
            };
            if !valid {
                return Err(LocalFlowError::AccessMismatch);
            }
        } else if let Some(candidate) = local.get(identifier.as_str()) {
            let valid_kind = candidate.kind.is_direct_syntax()
                || candidate.kind == LocalFlowRelationshipKind::ContainsFlowNode;
            if !valid_kind
                || !input_entities.contains(candidate.source.as_str())
                || !input_entities.contains(candidate.target.as_str())
            {
                return Err(LocalFlowError::DerivationMismatch);
            }
        } else {
            return Err(LocalFlowError::DerivationMismatch);
        }
    }
    Ok(())
}

fn reachable_identifiers<'a>(
    start: &'a str,
    adjacency: &BTreeMap<&'a str, Vec<&'a str>>,
) -> BTreeSet<&'a str> {
    let mut values = BTreeSet::new();
    let mut pending = VecDeque::from([start]);
    while let Some(current) = pending.pop_front() {
        for target in adjacency.get(current).into_iter().flatten() {
            if values.insert(*target) {
                pending.push_back(*target);
            }
        }
    }
    values
}

fn validate_edge_shape(
    relationship: &LocalFlowRelationship,
    callable_ids: &BTreeSet<&str>,
    block_ids: &BTreeSet<&str>,
    inherited_entity_ids: &BTreeSet<&str>,
) -> Result<(), LocalFlowError> {
    let valid = match relationship.kind {
        LocalFlowRelationshipKind::HasSyntaxBlock => {
            callable_ids.contains(relationship.source.as_str())
                && block_ids.contains(relationship.target.as_str())
        }
        LocalFlowRelationshipKind::ContainsFlowNode => {
            block_ids.contains(relationship.source.as_str())
                && inherited_entity_ids.contains(relationship.target.as_str())
        }
        LocalFlowRelationshipKind::HasCondition => {
            inherited_entity_ids.contains(relationship.source.as_str())
                && inherited_entity_ids.contains(relationship.target.as_str())
        }
        LocalFlowRelationshipKind::SyntaxNext
        | LocalFlowRelationshipKind::SyntaxTrueBranch
        | LocalFlowRelationshipKind::SyntaxFalseBranch
        | LocalFlowRelationshipKind::SyntaxReaches => {
            relationship.source != relationship.target
                && block_ids.contains(relationship.source.as_str())
                && block_ids.contains(relationship.target.as_str())
        }
        LocalFlowRelationshipKind::LexicalMustReachesRead
        | LocalFlowRelationshipKind::LexicalMayReachesRead => {
            relationship.source != relationship.target
                && inherited_entity_ids.contains(relationship.source.as_str())
                && inherited_entity_ids.contains(relationship.target.as_str())
        }
    };
    if valid {
        Ok(())
    } else {
        Err(LocalFlowError::EdgeInvalid)
    }
}

fn validate_block_membership(
    blocks: &[SyntaxBasicBlock],
    relationships: &[LocalFlowRelationship],
) -> Result<(), LocalFlowError> {
    for block in blocks {
        let membership = relationships
            .iter()
            .filter(|relationship| {
                relationship.kind == LocalFlowRelationshipKind::ContainsFlowNode
                    && relationship.source == block.id
            })
            .map(|relationship| relationship.target.as_str())
            .collect::<BTreeSet<_>>();
        let expected = block
            .flow_node_ids
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        if membership != expected
            || relationships
                .iter()
                .filter(|relationship| {
                    relationship.kind == LocalFlowRelationshipKind::ContainsFlowNode
                        && relationship.source == block.id
                })
                .count()
                != block.flow_node_ids.len()
        {
            return Err(LocalFlowError::BlockInvalid);
        }
        let owners = relationships
            .iter()
            .filter(|relationship| {
                relationship.kind == LocalFlowRelationshipKind::HasSyntaxBlock
                    && relationship.target == block.id
                    && relationship.source == block.callable_id
            })
            .count();
        if owners != 1 {
            return Err(LocalFlowError::BlockInvalid);
        }
    }
    Ok(())
}

fn validate_syntax_closure(
    blocks: &[SyntaxBasicBlock],
    relationships: &[LocalFlowRelationship],
) -> Result<(), LocalFlowError> {
    let blocks_by_callable = blocks.iter().fold(
        BTreeMap::<&str, Vec<&SyntaxBasicBlock>>::new(),
        |mut grouped, block| {
            grouped.entry(&block.callable_id).or_default().push(block);
            grouped
        },
    );
    for callable_blocks in blocks_by_callable.values() {
        let identifiers = callable_blocks
            .iter()
            .map(|block| block.id.as_str())
            .collect::<BTreeSet<_>>();
        let mut adjacency = BTreeMap::<&str, Vec<&str>>::new();
        for relationship in relationships.iter().filter(|relationship| {
            relationship.kind.is_direct_syntax()
                && identifiers.contains(relationship.source.as_str())
        }) {
            adjacency
                .entry(relationship.source.as_str())
                .or_default()
                .push(relationship.target.as_str());
        }
        let mut closure = BTreeSet::new();
        for source in &identifiers {
            let mut pending = VecDeque::from([*source]);
            let mut seen = BTreeSet::new();
            while let Some(current) = pending.pop_front() {
                for target in adjacency.get(current).into_iter().flatten() {
                    if *target == *source {
                        return Err(LocalFlowError::Cycle);
                    }
                    if seen.insert(*target) {
                        closure.insert((*source, *target));
                        pending.push_back(*target);
                    }
                }
            }
        }
        enforce_local_flow_limit(LocalFlowLimit::ReachabilityPairsPerCallable, closure.len())?;
        let observed = relationships
            .iter()
            .filter(|relationship| {
                relationship.kind == LocalFlowRelationshipKind::SyntaxReaches
                    && identifiers.contains(relationship.source.as_str())
            })
            .map(|relationship| (relationship.source.as_str(), relationship.target.as_str()))
            .collect::<BTreeSet<_>>();
        if closure != observed {
            return Err(LocalFlowError::ReachabilityMismatch);
        }
        let Some(entry) = callable_blocks.iter().min_by_key(|block| block.ordinal) else {
            continue;
        };
        let reachable = closure
            .iter()
            .filter(|(source, _)| *source == entry.id)
            .map(|(_, target)| *target)
            .chain(std::iter::once(entry.id.as_str()))
            .collect::<BTreeSet<_>>();
        if reachable != identifiers {
            return Err(LocalFlowError::ReachabilityMismatch);
        }
    }
    Ok(())
}

fn validate_chunk_union(knowledge: &LocalFlowKnowledge) -> Result<(), LocalFlowError> {
    let union = |values: Vec<(&str, String)>| -> Result<BTreeMap<String, String>, LocalFlowError> {
        let mut result = BTreeMap::new();
        for (identifier, debug) in values {
            if let Some(existing) = result.insert(identifier.to_owned(), debug.clone())
                && existing != debug
            {
                return Err(LocalFlowError::IdentityConflict);
            }
        }
        Ok(result)
    };
    let chunk_blocks = union(
        knowledge
            .extraction_chunks
            .iter()
            .flat_map(|chunk| &chunk.blocks)
            .map(|value| (value.id.as_str(), format!("{value:?}")))
            .collect(),
    )?;
    let graph_blocks = union(
        knowledge
            .graph
            .blocks
            .iter()
            .map(|value| (value.id.as_str(), format!("{value:?}")))
            .collect(),
    )?;
    let chunk_relationships = union(
        knowledge
            .extraction_chunks
            .iter()
            .flat_map(|chunk| &chunk.relationships)
            .map(|value| (value.id.as_str(), format!("{value:?}")))
            .collect(),
    )?;
    let graph_relationships = union(
        knowledge
            .graph
            .relationships
            .iter()
            .map(|value| (value.id.as_str(), format!("{value:?}")))
            .collect(),
    )?;
    if chunk_blocks != graph_blocks || chunk_relationships != graph_relationships {
        return Err(LocalFlowError::ContractInvalid);
    }
    Ok(())
}

fn ordered_unique<'a>(values: impl IntoIterator<Item = &'a str>) -> Result<(), LocalFlowError> {
    let mut previous = None;
    for value in values {
        if previous.is_some_and(|previous| previous >= value) {
            return Err(LocalFlowError::IdentityConflict);
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
                let mut encoded = [0_u8; 4];
                bytes.extend_from_slice(character.encode_utf8(&mut encoded).as_bytes());
            }
        }
    }
    bytes.push(b'"');
}

#[must_use]
pub fn r14_read_relationships(knowledge: &ExpressionBindingKnowledge) -> BTreeMap<&str, &str> {
    knowledge
        .graph
        .relationships
        .iter()
        .filter(|relationship| relationship.kind == ExpressionRelationshipKind::Reads)
        .map(|relationship| (relationship.source.as_str(), relationship.target.as_str()))
        .collect()
}
