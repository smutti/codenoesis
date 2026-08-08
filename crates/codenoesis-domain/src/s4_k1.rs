use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::knowledge::{ClaimState, ClaimSubjectKind};
use crate::s4::{WorkspaceClaim, WorkspaceEvidence, workspace_claim_id};
use crate::s4_r6::{FrameworkError, FrameworkExtraction, FrameworkKnowledge};
use crate::s5::AnalysisCacheEntry;

pub const K1_CONFIGURATION_VERSION: &str = "codenoesis.configuration/v8";
pub const K1_ONTOLOGY_VERSION: &str = "codenoesis.ontology/rust/v8";
pub const K1_PIPELINE_VERSION: &str = "codenoesis.pipeline/s4-k1-v1";
pub const K1_EXTRACTOR_VERSION: &str = "codenoesis.rust-callable/s4-k1-v1";
pub const K1_EXTRACTION_CONTRACT_VERSION: &str = "codenoesis.extraction/v8";
pub const K1_EXTRACTION_CHUNK_VERSION: &str = "codenoesis.extraction-chunk/v8";
pub const K1_GRAPH_VERSION: &str = "codenoesis.knowledge-graph/v8";
pub const K1_SNAPSHOT_VERSION: &str = "codenoesis.repository-snapshot/v11";
pub const K1_QUERY_VERSION: &str = "codenoesis.local-query-result/v6";
pub const K1_PORTABLE_GRAPH_VERSION: &str = "codenoesis.portable-graph/v2";
pub const K1_LOCAL_EXPLORER_VERSION: &str = "codenoesis.local-explorer/v2";
pub const K1_ERROR_VERSION: &str = "codenoesis.error/v16";
pub const K1_PROFILE: &str = "rust-callable-semantics-v1";
pub const K1_INDEX_VERSION: &str = "codenoesis.callable-semantics-index/v1";
pub const K1_SEMANTIC_HASH_CONTRACT_VERSION: &str = "codenoesis.semantic-hash-contract/v7";

pub const MAX_K1_CALLABLES_PER_SOURCE: u64 = 4_096;
pub const MAX_K1_PARAMETERS_PER_CALLABLE: u64 = 256;
pub const MAX_K1_BODY_FACTS_PER_CALLABLE: u64 = 8_192;
pub const MAX_K1_BODY_FACT_LEXICAL_DEPTH: u64 = 256;
pub const MAX_K1_SIGNATURE_COMPONENT_BYTES: u64 = 4_096;
pub const MAX_K1_EXPRESSION_METADATA_BYTES: u64 = 4_096;
pub const MAX_K1_ENTITIES: u64 = 200_000;
pub const MAX_K1_RELATIONSHIPS: u64 = 400_000;
pub const MAX_K1_DIAGNOSTICS: u64 = 50_000;
pub const MAX_K1_COVERAGE_GAPS: u64 = 50_000;
pub const K1_DETERMINISM_PERMUTATIONS: u64 = 50;
pub const K1_DETERMINISM_SCHEDULES: u64 = 10;

const ENTITY_ID_DOMAIN: &str = "codenoesis.entity-id/rust-callable-semantics/v1";
const RELATIONSHIP_ID_DOMAIN: &str = "codenoesis.relationship-id/rust-callable-semantics/v1";
const DIAGNOSTIC_ID_DOMAIN: &str = "codenoesis.diagnostic-id/rust-callable-semantics/v1";
const COVERAGE_GAP_ID_DOMAIN: &str = "codenoesis.coverage-gap-id/rust-callable-semantics/v1";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CallableSemanticEntityKind {
    Signature,
    Parameter,
    DeclaredValue,
    LocalBinding,
    CallSite,
    Control,
}

impl CallableSemanticEntityKind {
    pub const ALL: [Self; 6] = [
        Self::Signature,
        Self::Parameter,
        Self::DeclaredValue,
        Self::LocalBinding,
        Self::CallSite,
        Self::Control,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Signature => "rust.callable_signature",
            Self::Parameter => "rust.parameter",
            Self::DeclaredValue => "rust.declared_value",
            Self::LocalBinding => "rust.local_binding",
            Self::CallSite => "rust.call_site",
            Self::Control => "rust.control",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CallableRelationshipKind {
    HasSignature,
    HasParameter,
    DeclaresValue,
    HasBodyFact,
    Calls,
}

impl CallableRelationshipKind {
    pub const ALL: [Self; 5] = [
        Self::HasSignature,
        Self::HasParameter,
        Self::DeclaresValue,
        Self::HasBodyFact,
        Self::Calls,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HasSignature => "HAS_SIGNATURE",
            Self::HasParameter => "HAS_PARAMETER",
            Self::DeclaresValue => "DECLARES_VALUE",
            Self::HasBodyFact => "HAS_BODY_FACT",
            Self::Calls => "CALLS",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CallableReceiverState {
    None,
    Value,
    Ref,
    RefMut,
    TypedSelf,
    Explicit,
}

impl CallableReceiverState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Value => "value",
            Self::Ref => "ref",
            Self::RefMut => "ref_mut",
            Self::TypedSelf => "typed_self",
            Self::Explicit => "explicit",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CallableBodyState {
    Present,
    Absent,
}

impl CallableBodyState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Present => "present",
            Self::Absent => "absent",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CallableReturnState {
    Declared,
    UnitDefault,
}

impl CallableReturnState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Declared => "declared",
            Self::UnitDefault => "unit_default",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DeclaredValueState {
    NormalizedScalar,
    ExpressionOnly,
    Unresolved,
}

impl DeclaredValueState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NormalizedScalar => "normalized_scalar",
            Self::ExpressionOnly => "expression_only",
            Self::Unresolved => "unresolved",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NormalizedScalarValue {
    Boolean(bool),
    Integer {
        sign: String,
        radix: u32,
        digits: String,
        suffix: Option<String>,
    },
    Character(String),
    String(String),
}

impl NormalizedScalarValue {
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Boolean(_) => "boolean",
            Self::Integer { .. } => "integer",
            Self::Character(_) => "character",
            Self::String(_) => "string",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CallForm {
    Direct,
    Method,
}

impl CallForm {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Method => "method",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CallResolutionState {
    ResolvedUniqueLocal,
    CandidateUnresolved,
}

impl CallResolutionState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ResolvedUniqueLocal => "resolved_unique_local",
            Self::CandidateUnresolved => "candidate_unresolved",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ControlKind {
    If,
    IfLet,
    Match,
    Loop,
    While,
    WhileLet,
    For,
    Return,
    Break,
    Continue,
    Try,
}

impl ControlKind {
    pub const ALL: [Self; 11] = [
        Self::If,
        Self::IfLet,
        Self::Match,
        Self::Loop,
        Self::While,
        Self::WhileLet,
        Self::For,
        Self::Return,
        Self::Break,
        Self::Continue,
        Self::Try,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::If => "if",
            Self::IfLet => "if_let",
            Self::Match => "match",
            Self::Loop => "loop",
            Self::While => "while",
            Self::WhileLet => "while_let",
            Self::For => "for",
            Self::Return => "return",
            Self::Break => "break",
            Self::Continue => "continue",
            Self::Try => "try",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableSignatureProperties {
    pub visibility: String,
    pub is_async: bool,
    pub is_const: bool,
    pub is_unsafe: bool,
    pub abi: Option<String>,
    pub generic_parameters: Option<String>,
    pub where_clause: Option<String>,
    pub return_state: CallableReturnState,
    pub return_type: Option<String>,
    pub body_state: CallableBodyState,
    pub body_digest: Option<String>,
    pub body_evidence_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableParameterProperties {
    pub pattern: String,
    pub declared_type: Option<String>,
    pub receiver_state: CallableReceiverState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclaredValueProperties {
    pub state: DeclaredValueState,
    pub syntax_kind: Option<String>,
    pub expression_digest: Option<String>,
    pub expression_byte_length: u64,
    pub normalized: Option<NormalizedScalarValue>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalBindingProperties {
    pub pattern: String,
    pub declared_type: Option<String>,
    pub initializer_present: bool,
    pub lexical_depth: u64,
    pub parent_fact_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallSiteProperties {
    pub form: CallForm,
    pub target_spelling: String,
    pub resolution_state: CallResolutionState,
    pub resolved_target_id: Option<String>,
    pub lexical_depth: u64,
    pub parent_fact_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlProperties {
    pub control_kind: ControlKind,
    pub lexical_depth: u64,
    pub parent_fact_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableSemanticProperties {
    Signature(CallableSignatureProperties),
    Parameter(CallableParameterProperties),
    DeclaredValue(DeclaredValueProperties),
    LocalBinding(LocalBindingProperties),
    CallSite(CallSiteProperties),
    Control(ControlProperties),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableSemanticEntity {
    pub id: String,
    pub kind: CallableSemanticEntityKind,
    pub crate_id: String,
    pub module_path: String,
    pub name: String,
    pub subject_id: String,
    pub ordinal: Option<u64>,
    pub evidence_ids: Vec<String>,
    pub properties: CallableSemanticProperties,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableRelationship {
    pub id: String,
    pub kind: CallableRelationshipKind,
    pub source: String,
    pub target: String,
    pub evidence_ids: Vec<String>,
}

impl CallableRelationship {
    #[must_use]
    pub fn new(
        kind: CallableRelationshipKind,
        source: String,
        target: String,
        evidence_ids: Vec<String>,
    ) -> Self {
        let id = callable_relationship_id(kind, &source, &target);
        Self {
            id,
            kind,
            source,
            target,
            evidence_ids: stable_dedup(evidence_ids),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableDiagnostic {
    pub id: String,
    pub code: String,
    pub message: String,
    pub subject_id: String,
    pub evidence_ids: Vec<String>,
}

impl CallableDiagnostic {
    #[must_use]
    pub fn new(code: &str, message: &str, subject_id: String, evidence_ids: Vec<String>) -> Self {
        let evidence_ids = stable_dedup(evidence_ids);
        let id = callable_diagnostic_id(code, &subject_id, &evidence_ids);
        Self {
            id,
            code: code.to_owned(),
            message: message.to_owned(),
            subject_id,
            evidence_ids,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CallableCoverageState {
    Unsupported,
    NotResolved,
    NotAnalyzed,
    NotObserved,
}

impl CallableCoverageState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unsupported => "unsupported",
            Self::NotResolved => "not_resolved",
            Self::NotAnalyzed => "not_analyzed",
            Self::NotObserved => "not_observed",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableCoverageGap {
    pub id: String,
    pub capability: String,
    pub state: CallableCoverageState,
    pub subject_id: String,
    pub evidence_ids: Vec<String>,
}

impl CallableCoverageGap {
    #[must_use]
    pub fn new(
        capability: &str,
        state: CallableCoverageState,
        subject_id: String,
        evidence_ids: Vec<String>,
    ) -> Self {
        let evidence_ids = stable_dedup(evidence_ids);
        let id = callable_coverage_gap_id(capability, state, &subject_id, &evidence_ids);
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
pub struct CallableSourceChunk {
    pub crate_id: String,
    pub source_file_id: String,
    pub path: String,
    pub entities: Vec<CallableSemanticEntity>,
    pub relationships: Vec<CallableRelationship>,
    pub claims: Vec<WorkspaceClaim>,
    pub evidence: Vec<WorkspaceEvidence>,
    pub diagnostics: Vec<CallableDiagnostic>,
    pub coverage: Vec<CallableCoverageGap>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableSemanticsIndex {
    pub signature_ids: Vec<String>,
    pub parameter_ids: Vec<String>,
    pub declared_value_ids: Vec<String>,
    pub body_fact_ids: Vec<String>,
    pub resolved_call_relationship_ids: Vec<String>,
    pub unresolved_call_site_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableSemanticsGraph {
    pub entities: Vec<CallableSemanticEntity>,
    pub relationships: Vec<CallableRelationship>,
    pub claims: Vec<WorkspaceClaim>,
    pub evidence: Vec<WorkspaceEvidence>,
    pub diagnostics: Vec<CallableDiagnostic>,
    pub coverage: Vec<CallableCoverageGap>,
    pub index: CallableSemanticsIndex,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableSemanticsKnowledge {
    pub framework: FrameworkKnowledge,
    pub extraction_chunks: Vec<CallableSourceChunk>,
    pub graph: CallableSemanticsGraph,
}

impl CallableSemanticsKnowledge {
    /// Validates the complete inherited R6 lineage and additive K1 graph.
    ///
    /// # Errors
    ///
    /// Returns the first source, identity, ordering, evidence, reference, or
    /// resource contract failure.
    #[allow(clippy::too_many_lines)]
    pub fn validate(&self) -> Result<(), CallableSemanticsError> {
        self.framework
            .validate()
            .map_err(CallableSemanticsError::Source)?;
        if self.extraction_chunks.is_empty()
            || !ordered_unique(self.graph.entities.iter().map(|value| value.id.as_str()))
            || !ordered_unique(
                self.graph
                    .relationships
                    .iter()
                    .map(|value| value.id.as_str()),
            )
            || !ordered_unique(self.graph.claims.iter().map(|value| value.id.as_str()))
            || !ordered_unique(self.graph.evidence.iter().map(|value| value.id.as_str()))
            || !ordered_unique(self.graph.diagnostics.iter().map(|value| value.id.as_str()))
            || !ordered_unique(self.graph.coverage.iter().map(|value| value.id.as_str()))
        {
            return Err(CallableSemanticsError::ContractInvalid);
        }
        enforce_limit(
            CallableSemanticsLimit::EntitiesTotal,
            self.graph.entities.len(),
        )?;
        enforce_limit(
            CallableSemanticsLimit::RelationshipsTotal,
            self.graph.relationships.len(),
        )?;
        enforce_limit(
            CallableSemanticsLimit::DiagnosticsTotal,
            self.graph.diagnostics.len(),
        )?;
        enforce_limit(
            CallableSemanticsLimit::CoverageGapsTotal,
            self.graph.coverage.len(),
        )?;

        let repository_identity = &self
            .framework
            .semantic
            .manifest
            .workspace
            .knowledge
            .graph
            .repository_identity;
        let mut base_ids = self
            .framework
            .semantic
            .manifest
            .workspace
            .knowledge
            .graph
            .entities
            .iter()
            .map(|value| value.id.as_str())
            .chain(
                self.framework
                    .semantic
                    .manifest
                    .graph
                    .entities
                    .iter()
                    .map(|value| value.id.as_str()),
            )
            .chain(
                self.framework
                    .semantic
                    .graph
                    .legacy_entities
                    .iter()
                    .map(|value| value.id.as_str()),
            )
            .chain(
                self.framework
                    .semantic
                    .graph
                    .entities
                    .iter()
                    .map(|value| value.id.as_str()),
            )
            .chain(
                self.framework
                    .graph
                    .supplemental_entities
                    .iter()
                    .map(|value| value.id.as_str()),
            )
            .chain(
                self.framework
                    .graph
                    .declarations
                    .iter()
                    .map(|value| value.id.as_str()),
            )
            .collect::<BTreeSet<_>>();
        let entity_ids = self
            .graph
            .entities
            .iter()
            .map(|value| value.id.as_str())
            .collect::<BTreeSet<_>>();
        base_ids.extend(entity_ids.iter().copied());
        let evidence_ids = self
            .graph
            .evidence
            .iter()
            .map(|value| value.id.as_str())
            .collect::<BTreeSet<_>>();

        for entity in &self.graph.entities {
            if !base_ids.contains(entity.subject_id.as_str())
                || entity.evidence_ids.is_empty()
                || entity
                    .evidence_ids
                    .iter()
                    .any(|value| !evidence_ids.contains(value.as_str()))
                || entity.id != expected_entity_id(repository_identity, entity)
                || !properties_match_kind(entity)
            {
                return Err(CallableSemanticsError::ContractInvalid);
            }
            if let Some(parent) = parent_fact_id(entity)
                && !entity_ids.contains(parent)
            {
                return Err(CallableSemanticsError::ContractInvalid);
            }
        }

        let relationship_ids = self
            .graph
            .relationships
            .iter()
            .map(|value| value.id.as_str())
            .collect::<BTreeSet<_>>();
        for relationship in &self.graph.relationships {
            if relationship.id
                != callable_relationship_id(
                    relationship.kind,
                    &relationship.source,
                    &relationship.target,
                )
                || !base_ids.contains(relationship.source.as_str())
                || !base_ids.contains(relationship.target.as_str())
                || relationship.evidence_ids.is_empty()
                || relationship
                    .evidence_ids
                    .iter()
                    .any(|value| !evidence_ids.contains(value.as_str()))
            {
                return Err(CallableSemanticsError::ContractInvalid);
            }
        }
        let subjects = entity_ids
            .iter()
            .copied()
            .chain(relationship_ids.iter().copied())
            .collect::<BTreeSet<_>>();
        for claim in &self.graph.claims {
            if claim.id != workspace_claim_id(claim.subject_kind, &claim.subject_id, claim.state)
                || !subjects.contains(claim.subject_id.as_str())
                || claim.evidence_ids.is_empty()
                || claim
                    .evidence_ids
                    .iter()
                    .any(|value| !evidence_ids.contains(value.as_str()))
            {
                return Err(CallableSemanticsError::ContractInvalid);
            }
        }
        for diagnostic in &self.graph.diagnostics {
            if diagnostic.id
                != callable_diagnostic_id(
                    &diagnostic.code,
                    &diagnostic.subject_id,
                    &diagnostic.evidence_ids,
                )
                || !entity_ids.contains(diagnostic.subject_id.as_str())
            {
                return Err(CallableSemanticsError::ContractInvalid);
            }
        }
        for gap in &self.graph.coverage {
            if gap.id
                != callable_coverage_gap_id(
                    &gap.capability,
                    gap.state,
                    &gap.subject_id,
                    &gap.evidence_ids,
                )
                || !entity_ids.contains(gap.subject_id.as_str())
            {
                return Err(CallableSemanticsError::ContractInvalid);
            }
        }
        let expected_index =
            CallableSemanticsIndex::from_graph(&self.graph.entities, &self.graph.relationships);
        if self.graph.index != expected_index {
            return Err(CallableSemanticsError::ContractInvalid);
        }
        Ok(())
    }
}

impl CallableSemanticsIndex {
    #[must_use]
    pub fn from_graph(
        entities: &[CallableSemanticEntity],
        relationships: &[CallableRelationship],
    ) -> Self {
        let ids = |kind| {
            entities
                .iter()
                .filter(|value| value.kind == kind)
                .map(|value| value.id.clone())
                .collect::<Vec<_>>()
        };
        Self {
            signature_ids: ids(CallableSemanticEntityKind::Signature),
            parameter_ids: ids(CallableSemanticEntityKind::Parameter),
            declared_value_ids: ids(CallableSemanticEntityKind::DeclaredValue),
            body_fact_ids: entities
                .iter()
                .filter(|value| {
                    matches!(
                        value.kind,
                        CallableSemanticEntityKind::LocalBinding
                            | CallableSemanticEntityKind::CallSite
                            | CallableSemanticEntityKind::Control
                    )
                })
                .map(|value| value.id.clone())
                .collect(),
            resolved_call_relationship_ids: relationships
                .iter()
                .filter(|value| value.kind == CallableRelationshipKind::Calls)
                .map(|value| value.id.clone())
                .collect(),
            unresolved_call_site_ids: entities
                .iter()
                .filter(|value| {
                    matches!(
                        &value.properties,
                        CallableSemanticProperties::CallSite(CallSiteProperties {
                            resolution_state: CallResolutionState::CandidateUnresolved,
                            ..
                        })
                    )
                })
                .map(|value| value.id.clone())
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableSemanticsExtraction {
    pub knowledge: CallableSemanticsKnowledge,
    pub cache_entries: Vec<AnalysisCacheEntry>,
    pub parser_invocation_count: u64,
}

impl CallableSemanticsExtraction {
    #[must_use]
    pub fn from_r6(
        source: FrameworkExtraction,
        extraction_chunks: Vec<CallableSourceChunk>,
        graph: CallableSemanticsGraph,
        parser_invocation_count: u64,
    ) -> Self {
        Self {
            knowledge: CallableSemanticsKnowledge {
                framework: source.knowledge,
                extraction_chunks,
                graph,
            },
            cache_entries: source.cache_entries,
            parser_invocation_count,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableSemanticsLimit {
    CallablesPerSource,
    ParametersPerCallable,
    BodyFactsPerCallable,
    BodyFactLexicalDepth,
    SignatureComponentBytes,
    ExpressionMetadataBytes,
    EntitiesTotal,
    RelationshipsTotal,
    DiagnosticsTotal,
    CoverageGapsTotal,
}

impl CallableSemanticsLimit {
    pub const ALL: [Self; 10] = [
        Self::CallablesPerSource,
        Self::ParametersPerCallable,
        Self::BodyFactsPerCallable,
        Self::BodyFactLexicalDepth,
        Self::SignatureComponentBytes,
        Self::ExpressionMetadataBytes,
        Self::EntitiesTotal,
        Self::RelationshipsTotal,
        Self::DiagnosticsTotal,
        Self::CoverageGapsTotal,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CallablesPerSource => "callables_per_source",
            Self::ParametersPerCallable => "parameters_per_callable",
            Self::BodyFactsPerCallable => "body_facts_per_callable",
            Self::BodyFactLexicalDepth => "body_fact_lexical_depth",
            Self::SignatureComponentBytes => "signature_component_bytes",
            Self::ExpressionMetadataBytes => "expression_metadata_bytes",
            Self::EntitiesTotal => "entities_total",
            Self::RelationshipsTotal => "relationships_total",
            Self::DiagnosticsTotal => "diagnostics_total",
            Self::CoverageGapsTotal => "coverage_gaps_total",
        }
    }

    #[must_use]
    pub const fn maximum(self) -> u64 {
        match self {
            Self::CallablesPerSource => MAX_K1_CALLABLES_PER_SOURCE,
            Self::ParametersPerCallable => MAX_K1_PARAMETERS_PER_CALLABLE,
            Self::BodyFactsPerCallable => MAX_K1_BODY_FACTS_PER_CALLABLE,
            Self::BodyFactLexicalDepth => MAX_K1_BODY_FACT_LEXICAL_DEPTH,
            Self::SignatureComponentBytes => MAX_K1_SIGNATURE_COMPONENT_BYTES,
            Self::ExpressionMetadataBytes => MAX_K1_EXPRESSION_METADATA_BYTES,
            Self::EntitiesTotal => MAX_K1_ENTITIES,
            Self::RelationshipsTotal => MAX_K1_RELATIONSHIPS,
            Self::DiagnosticsTotal => MAX_K1_DIAGNOSTICS,
            Self::CoverageGapsTotal => MAX_K1_COVERAGE_GAPS,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableSemanticsError {
    Source(FrameworkError),
    InvalidSyntax {
        path: String,
        start_byte: u64,
        syntax_kind: String,
    },
    IdentityConflict {
        kind: String,
        normalized_identity: String,
    },
    UnsupportedComposition,
    LimitExceeded {
        limit: CallableSemanticsLimit,
        maximum: u64,
        observed: u64,
    },
    ContractInvalid,
}

impl Display for CallableSemanticsError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Source(_) => "K1 source extraction failed",
            Self::InvalidSyntax { .. } => "invalid K1 callable syntax",
            Self::IdentityConflict { .. } => "K1 callable identity conflict",
            Self::UnsupportedComposition => "unsupported K1 profile composition",
            Self::LimitExceeded { .. } => "K1 callable limit exceeded",
            Self::ContractInvalid => "K1 callable contract is invalid",
        })
    }
}

impl Error for CallableSemanticsError {}

#[must_use]
pub fn callable_signature_id(repository_identity: &str, callable_id: &str) -> String {
    stable_id(
        "urn:codenoesis:entity:blake3:",
        &[
            ENTITY_ID_DOMAIN,
            repository_identity,
            callable_id,
            CallableSemanticEntityKind::Signature.as_str(),
        ],
    )
}

#[must_use]
pub fn callable_parameter_id(
    repository_identity: &str,
    callable_id: &str,
    ordinal: u64,
    normalized_pattern: &str,
) -> String {
    stable_id(
        "urn:codenoesis:entity:blake3:",
        &[
            ENTITY_ID_DOMAIN,
            repository_identity,
            callable_id,
            CallableSemanticEntityKind::Parameter.as_str(),
            &ordinal.to_string(),
            normalized_pattern,
        ],
    )
}

#[must_use]
pub fn declared_value_id(repository_identity: &str, declaration_id: &str) -> String {
    stable_id(
        "urn:codenoesis:entity:blake3:",
        &[
            ENTITY_ID_DOMAIN,
            repository_identity,
            declaration_id,
            CallableSemanticEntityKind::DeclaredValue.as_str(),
        ],
    )
}

#[must_use]
pub fn callable_body_fact_id(
    repository_identity: &str,
    callable_id: &str,
    kind: CallableSemanticEntityKind,
    evidence_id: &str,
) -> String {
    stable_id(
        "urn:codenoesis:entity:blake3:",
        &[
            ENTITY_ID_DOMAIN,
            repository_identity,
            callable_id,
            kind.as_str(),
            evidence_id,
        ],
    )
}

#[must_use]
pub fn callable_relationship_id(
    kind: CallableRelationshipKind,
    source: &str,
    target: &str,
) -> String {
    stable_id(
        "urn:codenoesis:relationship:blake3:",
        &[RELATIONSHIP_ID_DOMAIN, kind.as_str(), source, target],
    )
}

#[must_use]
pub fn k1_digest(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

#[must_use]
pub fn callable_claim(
    subject_kind: ClaimSubjectKind,
    subject_id: String,
    evidence_id: String,
    state: ClaimState,
) -> WorkspaceClaim {
    WorkspaceClaim::new(subject_kind, subject_id, state, vec![evidence_id])
}

/// Enforces one reviewed K1-specific fixed bound.
///
/// # Errors
///
/// Returns the exact limit, maximum, and observed maximum-plus-one value.
pub fn enforce_limit(
    limit: CallableSemanticsLimit,
    observed: usize,
) -> Result<(), CallableSemanticsError> {
    let observed = u64::try_from(observed).unwrap_or(u64::MAX);
    let maximum = limit.maximum();
    if observed > maximum {
        Err(CallableSemanticsError::LimitExceeded {
            limit,
            maximum,
            observed,
        })
    } else {
        Ok(())
    }
}

fn expected_entity_id(repository_identity: &str, entity: &CallableSemanticEntity) -> String {
    match &entity.properties {
        CallableSemanticProperties::Signature(_) => {
            callable_signature_id(repository_identity, &entity.subject_id)
        }
        CallableSemanticProperties::Parameter(properties) => callable_parameter_id(
            repository_identity,
            &entity.subject_id,
            entity.ordinal.unwrap_or(u64::MAX),
            &properties.pattern,
        ),
        CallableSemanticProperties::DeclaredValue(_) => {
            declared_value_id(repository_identity, &entity.subject_id)
        }
        CallableSemanticProperties::LocalBinding(_)
        | CallableSemanticProperties::CallSite(_)
        | CallableSemanticProperties::Control(_) => callable_body_fact_id(
            repository_identity,
            &entity.subject_id,
            entity.kind,
            entity.evidence_ids.first().map_or("", String::as_str),
        ),
    }
}

fn properties_match_kind(entity: &CallableSemanticEntity) -> bool {
    matches!(
        (entity.kind, &entity.properties),
        (
            CallableSemanticEntityKind::Signature,
            CallableSemanticProperties::Signature(_)
        ) | (
            CallableSemanticEntityKind::Parameter,
            CallableSemanticProperties::Parameter(_)
        ) | (
            CallableSemanticEntityKind::DeclaredValue,
            CallableSemanticProperties::DeclaredValue(_)
        ) | (
            CallableSemanticEntityKind::LocalBinding,
            CallableSemanticProperties::LocalBinding(_)
        ) | (
            CallableSemanticEntityKind::CallSite,
            CallableSemanticProperties::CallSite(_)
        ) | (
            CallableSemanticEntityKind::Control,
            CallableSemanticProperties::Control(_)
        )
    )
}

fn parent_fact_id(entity: &CallableSemanticEntity) -> Option<&str> {
    match &entity.properties {
        CallableSemanticProperties::LocalBinding(value) => value.parent_fact_id.as_deref(),
        CallableSemanticProperties::CallSite(value) => value.parent_fact_id.as_deref(),
        CallableSemanticProperties::Control(value) => value.parent_fact_id.as_deref(),
        CallableSemanticProperties::Signature(_)
        | CallableSemanticProperties::Parameter(_)
        | CallableSemanticProperties::DeclaredValue(_) => None,
    }
}

fn callable_diagnostic_id(code: &str, subject_id: &str, evidence_ids: &[String]) -> String {
    stable_id(
        "urn:codenoesis:diagnostic:blake3:",
        &[
            DIAGNOSTIC_ID_DOMAIN,
            code,
            subject_id,
            &evidence_ids.join("\u{1f}"),
        ],
    )
}

fn callable_coverage_gap_id(
    capability: &str,
    state: CallableCoverageState,
    subject_id: &str,
    evidence_ids: &[String],
) -> String {
    stable_id(
        "urn:codenoesis:coverage-gap:blake3:",
        &[
            COVERAGE_GAP_ID_DOMAIN,
            capability,
            state.as_str(),
            subject_id,
            &evidence_ids.join("\u{1f}"),
        ],
    )
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

fn stable_dedup(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter(|value| seen.insert(value.clone()))
        .collect()
}

#[must_use]
pub fn callable_entity_counts(
    entities: &[CallableSemanticEntity],
) -> BTreeMap<CallableSemanticEntityKind, usize> {
    let mut counts = BTreeMap::new();
    for entity in entities {
        *counts.entry(entity.kind).or_default() += 1;
    }
    counts
}

#[must_use]
pub fn callable_relationship_counts(
    relationships: &[CallableRelationship],
) -> BTreeMap<CallableRelationshipKind, usize> {
    let mut counts = BTreeMap::new();
    for relationship in relationships {
        *counts.entry(relationship.kind).or_default() += 1;
    }
    counts
}
