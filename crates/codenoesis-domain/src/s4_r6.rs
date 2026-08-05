use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::knowledge::{ClaimState, ClaimSubjectKind, RelationshipKind};
use crate::s4::{
    WorkspaceClaim, WorkspaceEntity, WorkspaceEvidence, WorkspaceRelationship, workspace_claim_id,
    workspace_relationship_id,
};
use crate::s4_r5::{
    CompilationPresence, RustSemanticDepthExtraction, RustSemanticError, RustSemanticKnowledge,
};
use crate::s5::{AnalysisCacheEntry, SourceAnalysisRecord};

pub const R6_ONTOLOGY_VERSION: &str = "codenoesis.ontology/rust/v6";
pub const R6_PIPELINE_VERSION: &str = "codenoesis.pipeline/s4-r6-v1";
pub const R6_FRAMEWORK_EXTRACTOR_VERSION: &str = "codenoesis.rust-framework/s4-r6-v1";
pub const R6_EXTRACTION_CONTRACT_VERSION: &str = "codenoesis.extraction/v6";
pub const R6_FRAMEWORK_PROFILE: &str = "rust-framework-declarations-v1";
pub const R6_FRAMEWORK_INDEX_VERSION: &str = "codenoesis.framework-declaration-index/v1";
pub const R6_SEMANTIC_HASH_CONTRACT_VERSION: &str = "codenoesis.semantic-hash-contract/v5";
pub const R6_CONFIGURATION_VERSION: &str = "codenoesis.configuration/v6";
pub const R6_SNAPSHOT_VERSION: &str = "codenoesis.repository-snapshot/v9";
pub const R6_EXTRACTION_CHUNK_VERSION: &str = "codenoesis.extraction-chunk/v6";
pub const R6_GRAPH_VERSION: &str = "codenoesis.knowledge-graph/v6";
pub const R6_ERROR_VERSION: &str = "codenoesis.error/v13";
pub const R6_QUERY_VERSION: &str = "codenoesis.local-query-result/v4";

pub const MAX_R6_FRAMEWORK_DECLARATIONS_PER_SOURCE: u64 = 4_096;
pub const MAX_R6_EXPLICIT_REGISTRATION_CHAIN_SEGMENTS: u64 = 256;
pub const MAX_R6_REGISTRATION_EXPRESSION_DEPTH: u64 = 64;
pub const MAX_R6_LITERAL_ROUTE_PATH_BYTES: u64 = 2_048;
pub const MAX_R6_LITERAL_METHOD_OR_CONFIGURATION_KEY_BYTES: u64 = 1_024;
pub const MAX_R6_TARGET_SPELLING_BYTES: u64 = 1_024;
pub const MAX_R6_OUTER_ATTRIBUTES_PER_DECLARATION: u64 = 128;
pub const MAX_R6_ATTRIBUTE_TOKEN_BYTES: u64 = 16_384;
pub const R6_DETERMINISM_PERMUTATIONS: u64 = 50;

pub const FRAMEWORK_DECLARATION_ID_DOMAIN: &str = "codenoesis.entity-id/framework-declaration/v1";
const FRAMEWORK_DIAGNOSTIC_ID_DOMAIN: &str = "codenoesis.diagnostic-id/v1";
const FRAMEWORK_COVERAGE_GAP_ID_DOMAIN: &str = "codenoesis.coverage-gap-id/v1";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FrameworkRole {
    Component,
    Configuration,
    Endpoint,
    Handler,
    Route,
    Service,
}

impl FrameworkRole {
    pub const ALL: [Self; 6] = [
        Self::Component,
        Self::Configuration,
        Self::Endpoint,
        Self::Handler,
        Self::Route,
        Self::Service,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Component => "component",
            Self::Configuration => "configuration",
            Self::Endpoint => "endpoint",
            Self::Handler => "handler",
            Self::Route => "route",
            Self::Service => "service",
        }
    }

    #[must_use]
    pub const fn entity_kind(self) -> &'static str {
        match self {
            Self::Component => "framework.component_declaration",
            Self::Configuration => "framework.configuration_declaration",
            Self::Endpoint => "framework.endpoint_declaration",
            Self::Handler => "framework.handler_declaration",
            Self::Route => "framework.route_declaration",
            Self::Service => "framework.service_declaration",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FrameworkSourceProfile {
    ExplicitBuilderRegistration,
    AttributeMacroCandidate,
}

impl FrameworkSourceProfile {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExplicitBuilderRegistration => "explicit-builder-registration-v1",
            Self::AttributeMacroCandidate => "attribute-macro-candidate-v1",
        }
    }

    #[must_use]
    pub const fn epistemic_state(self) -> FrameworkEpistemicState {
        match self {
            Self::ExplicitBuilderRegistration => {
                FrameworkEpistemicState::DeclaredRegistrationSyntax
            }
            Self::AttributeMacroCandidate => FrameworkEpistemicState::CandidateUnresolved,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FrameworkEpistemicState {
    DeclaredRegistrationSyntax,
    CandidateUnresolved,
}

impl FrameworkEpistemicState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DeclaredRegistrationSyntax => "declared_registration_syntax",
            Self::CandidateUnresolved => "candidate_unresolved",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FrameworkTargetBinding {
    ResolvedUnique,
    UnresolvedExternal,
    AmbiguousLocal,
    NotApplicable,
}

impl FrameworkTargetBinding {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ResolvedUnique => "resolved_unique",
            Self::UnresolvedExternal => "unresolved_external",
            Self::AmbiguousLocal => "ambiguous_local",
            Self::NotApplicable => "not_applicable",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameworkDeclaration {
    pub id: String,
    pub role: FrameworkRole,
    pub crate_id: String,
    pub lexical_owner_id: String,
    pub source_profile: FrameworkSourceProfile,
    pub source_form_identity: String,
    pub declared_key_or_target: String,
    pub epistemic_state: FrameworkEpistemicState,
    pub compilation_presence: CompilationPresence,
    pub method: Option<String>,
    pub path: Option<String>,
    pub configuration_key: Option<String>,
    pub target_spelling: Option<String>,
    pub local_target_id: Option<String>,
    pub target_binding: FrameworkTargetBinding,
    pub evidence_ids: Vec<String>,
}

impl FrameworkDeclaration {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        repository_identity: &str,
        role: FrameworkRole,
        crate_id: String,
        lexical_owner_id: String,
        source_profile: FrameworkSourceProfile,
        source_form_identity: String,
        declared_key_or_target: String,
        compilation_presence: CompilationPresence,
        method: Option<String>,
        path: Option<String>,
        configuration_key: Option<String>,
        target_spelling: Option<String>,
        local_target_id: Option<String>,
        target_binding: FrameworkTargetBinding,
        evidence_ids: Vec<String>,
    ) -> Self {
        let epistemic_state = source_profile.epistemic_state();
        let id = framework_declaration_id(
            repository_identity,
            &crate_id,
            &lexical_owner_id,
            role,
            source_profile,
            &source_form_identity,
            &declared_key_or_target,
        );
        Self {
            id,
            role,
            crate_id,
            lexical_owner_id,
            source_profile,
            source_form_identity,
            declared_key_or_target,
            epistemic_state,
            compilation_presence,
            method,
            path,
            configuration_key,
            target_spelling,
            local_target_id,
            target_binding,
            evidence_ids: stable_dedup(evidence_ids),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameworkDiagnostic {
    pub id: String,
    pub declaration_id: String,
    pub code: String,
    pub message: String,
    pub evidence_ids: Vec<String>,
}

impl FrameworkDiagnostic {
    #[must_use]
    pub fn new(
        _repository_identity: &str,
        declaration_id: String,
        code: &str,
        message: &str,
        evidence_ids: Vec<String>,
    ) -> Self {
        let evidence_ids = stable_dedup(evidence_ids);
        let joined = evidence_ids.join("\u{1f}");
        let id = stable_artifact_id(
            "urn:codenoesis:diagnostic:blake3:",
            FRAMEWORK_DIAGNOSTIC_ID_DOMAIN,
            &[&declaration_id, code, &joined],
        );
        Self {
            id,
            declaration_id,
            code: code.to_owned(),
            message: message.to_owned(),
            evidence_ids,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FrameworkCoverageState {
    Ambiguous,
    NotAnalyzed,
    NotObserved,
    NotResolved,
    Unsupported,
}

impl FrameworkCoverageState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ambiguous => "ambiguous",
            Self::NotAnalyzed => "not_analyzed",
            Self::NotObserved => "not_observed",
            Self::NotResolved => "not_resolved",
            Self::Unsupported => "unsupported",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameworkCoverageGap {
    pub id: String,
    pub declaration_id: String,
    pub capability: String,
    pub state: FrameworkCoverageState,
    pub evidence_ids: Vec<String>,
}

impl FrameworkCoverageGap {
    #[must_use]
    pub fn new(
        _repository_identity: &str,
        _commit_oid: &str,
        declaration_id: String,
        capability: &str,
        state: FrameworkCoverageState,
        evidence_ids: Vec<String>,
    ) -> Self {
        let evidence_ids = stable_dedup(evidence_ids);
        let joined = evidence_ids.join("\u{1f}");
        let id = stable_artifact_id(
            "urn:codenoesis:coverage-gap:blake3:",
            FRAMEWORK_COVERAGE_GAP_ID_DOMAIN,
            &[&declaration_id, capability, &joined],
        );
        Self {
            id,
            declaration_id,
            capability: capability.to_owned(),
            state,
            evidence_ids,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameworkSourceChunk {
    pub crate_id: String,
    pub source_file_id: String,
    pub supplemental_entities: Vec<WorkspaceEntity>,
    pub declarations: Vec<FrameworkDeclaration>,
    pub relationships: Vec<WorkspaceRelationship>,
    pub claims: Vec<WorkspaceClaim>,
    pub evidence: Vec<WorkspaceEvidence>,
    pub diagnostics: Vec<FrameworkDiagnostic>,
    pub coverage: Vec<FrameworkCoverageGap>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameworkDeclarationIndex {
    pub entity_ids: Vec<String>,
    pub declared_registration_ids: Vec<String>,
    pub candidate_unresolved_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameworkGraph {
    pub supplemental_entities: Vec<WorkspaceEntity>,
    pub declarations: Vec<FrameworkDeclaration>,
    pub relationships: Vec<WorkspaceRelationship>,
    pub claims: Vec<WorkspaceClaim>,
    pub evidence: Vec<WorkspaceEvidence>,
    pub diagnostics: Vec<FrameworkDiagnostic>,
    pub coverage: Vec<FrameworkCoverageGap>,
    pub index: FrameworkDeclarationIndex,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameworkKnowledge {
    pub semantic: RustSemanticKnowledge,
    pub extraction_chunks: Vec<FrameworkSourceChunk>,
    pub graph: FrameworkGraph,
}

impl FrameworkKnowledge {
    /// Validates the additive R6 graph and complete immutable R5 lineage.
    ///
    /// # Errors
    ///
    /// Returns the first deterministic source or framework contract failure.
    #[allow(clippy::too_many_lines)]
    pub fn validate(&self) -> Result<(), FrameworkError> {
        self.semantic.validate().map_err(FrameworkError::Source)?;
        let repository_identity = &self
            .semantic
            .manifest
            .workspace
            .knowledge
            .graph
            .repository_identity;
        let base_entities = self
            .semantic
            .manifest
            .workspace
            .knowledge
            .graph
            .entities
            .iter()
            .map(|value| value.id.as_str())
            .chain(
                self.semantic
                    .manifest
                    .graph
                    .entities
                    .iter()
                    .map(|value| value.id.as_str()),
            )
            .chain(
                self.semantic
                    .graph
                    .legacy_entities
                    .iter()
                    .map(|value| value.id.as_str()),
            )
            .chain(
                self.semantic
                    .graph
                    .entities
                    .iter()
                    .map(|value| value.id.as_str()),
            )
            .collect::<BTreeSet<_>>();
        let supplemental_ids = self
            .graph
            .supplemental_entities
            .iter()
            .map(|value| value.id.as_str())
            .collect::<BTreeSet<_>>();
        let declaration_ids = self
            .graph
            .declarations
            .iter()
            .map(|value| value.id.as_str())
            .collect::<BTreeSet<_>>();
        let all_entities = base_entities
            .iter()
            .copied()
            .chain(supplemental_ids.iter().copied())
            .chain(declaration_ids.iter().copied())
            .collect::<BTreeSet<_>>();
        let evidence_ids = self
            .graph
            .evidence
            .iter()
            .map(|value| value.id.as_str())
            .collect::<BTreeSet<_>>();

        if self.extraction_chunks.is_empty()
            || !ordered_unique(
                self.graph
                    .supplemental_entities
                    .iter()
                    .map(|value| value.id.as_str()),
            )
            || !ordered_unique(
                self.graph
                    .declarations
                    .iter()
                    .map(|value| value.id.as_str()),
            )
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
            || supplemental_ids
                .iter()
                .any(|value| base_entities.contains(value))
        {
            return Err(FrameworkError::ContractInvalid);
        }

        for declaration in &self.graph.declarations {
            if declaration.id
                != framework_declaration_id(
                    repository_identity,
                    &declaration.crate_id,
                    &declaration.lexical_owner_id,
                    declaration.role,
                    declaration.source_profile,
                    &declaration.source_form_identity,
                    &declaration.declared_key_or_target,
                )
                || declaration.epistemic_state != declaration.source_profile.epistemic_state()
                || declaration.evidence_ids.is_empty()
                || !all_entities.contains(declaration.crate_id.as_str())
                || !all_entities.contains(declaration.lexical_owner_id.as_str())
                || declaration
                    .evidence_ids
                    .iter()
                    .any(|value| !evidence_ids.contains(value.as_str()))
                || declaration.target_binding == FrameworkTargetBinding::ResolvedUnique
                    && declaration
                        .local_target_id
                        .as_ref()
                        .is_none_or(|value| !all_entities.contains(value.as_str()))
                || declaration.target_binding != FrameworkTargetBinding::ResolvedUnique
                    && declaration.local_target_id.is_some()
                || declaration.source_profile == FrameworkSourceProfile::AttributeMacroCandidate
                    && (declaration.method.is_some()
                        || declaration.path.is_some()
                        || declaration.configuration_key.is_some())
            {
                return Err(FrameworkError::ContractInvalid);
            }
        }

        if self.graph.relationships.len() != self.graph.declarations.len()
            || self.graph.relationships.iter().any(|relationship| {
                relationship.kind != RelationshipKind::Defines
                    || relationship.id
                        != workspace_relationship_id(
                            relationship.kind,
                            &relationship.source,
                            &relationship.target,
                        )
                    || !declaration_ids.contains(relationship.target.as_str())
                    || !all_entities.contains(relationship.source.as_str())
                    || relationship.evidence_ids.len() != 1
                    || !evidence_ids.contains(relationship.evidence_ids[0].as_str())
            })
        {
            return Err(FrameworkError::ContractInvalid);
        }

        let subjects = supplemental_ids
            .iter()
            .copied()
            .chain(declaration_ids.iter().copied())
            .chain(
                self.graph
                    .relationships
                    .iter()
                    .map(|value| value.id.as_str()),
            )
            .collect::<BTreeSet<_>>();
        if self.graph.claims.iter().any(|claim| {
            claim.state != ClaimState::DeterministicFact
                || claim.id
                    != workspace_claim_id(claim.subject_kind, &claim.subject_id, claim.state)
                || !subjects.contains(claim.subject_id.as_str())
                || claim.evidence_ids.is_empty()
                || claim
                    .evidence_ids
                    .iter()
                    .any(|value| !evidence_ids.contains(value.as_str()))
        }) {
            return Err(FrameworkError::ContractInvalid);
        }

        let expected_entity_ids = self
            .graph
            .declarations
            .iter()
            .map(|value| value.id.clone())
            .collect::<Vec<_>>();
        let expected_declared = self
            .graph
            .declarations
            .iter()
            .filter(|value| {
                value.epistemic_state == FrameworkEpistemicState::DeclaredRegistrationSyntax
            })
            .map(|value| value.id.clone())
            .collect::<Vec<_>>();
        let expected_candidates = self
            .graph
            .declarations
            .iter()
            .filter(|value| value.epistemic_state == FrameworkEpistemicState::CandidateUnresolved)
            .map(|value| value.id.clone())
            .collect::<Vec<_>>();
        if self.graph.index.entity_ids != expected_entity_ids
            || self.graph.index.declared_registration_ids != expected_declared
            || self.graph.index.candidate_unresolved_ids != expected_candidates
            || self.graph.diagnostics.iter().any(|value| {
                !declaration_ids.contains(value.declaration_id.as_str())
                    || value.id
                        != framework_diagnostic_id(
                            &value.declaration_id,
                            &value.code,
                            &value.evidence_ids,
                        )
                    || value.evidence_ids.is_empty()
                    || value
                        .evidence_ids
                        .iter()
                        .any(|id| !evidence_ids.contains(id.as_str()))
            })
            || self.graph.coverage.iter().any(|value| {
                framework_capability_state(&value.capability) != Some(value.state)
                    || value.id
                        != framework_coverage_gap_id(
                            &value.declaration_id,
                            &value.capability,
                            &value.evidence_ids,
                        )
                    || !declaration_ids.contains(value.declaration_id.as_str())
                    || value.evidence_ids.is_empty()
                    || value
                        .evidence_ids
                        .iter()
                        .any(|id| !evidence_ids.contains(id.as_str()))
            })
        {
            return Err(FrameworkError::ContractInvalid);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameworkExtraction {
    pub knowledge: FrameworkKnowledge,
    pub cache_entries: Vec<AnalysisCacheEntry>,
    pub source_records: Vec<SourceAnalysisRecord>,
    pub parser_invocation_count: u64,
}

impl FrameworkExtraction {
    #[must_use]
    pub fn from_r5(
        extraction: RustSemanticDepthExtraction,
        extraction_chunks: Vec<FrameworkSourceChunk>,
        graph: FrameworkGraph,
        parser_invocation_count: u64,
    ) -> Self {
        Self {
            knowledge: FrameworkKnowledge {
                semantic: extraction.knowledge,
                extraction_chunks,
                graph,
            },
            cache_entries: extraction.cache_entries,
            source_records: extraction.source_records,
            parser_invocation_count,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FrameworkLimit {
    FrameworkDeclarationsPerSource,
    ExplicitRegistrationChainSegments,
    RegistrationExpressionDepth,
    LiteralRoutePathBytes,
    LiteralMethodOrConfigurationKeyBytes,
    TargetSpellingBytes,
    OuterAttributesPerDeclaration,
    AttributeTokenBytes,
}

impl FrameworkLimit {
    pub const ALL: [Self; 8] = [
        Self::FrameworkDeclarationsPerSource,
        Self::ExplicitRegistrationChainSegments,
        Self::RegistrationExpressionDepth,
        Self::LiteralRoutePathBytes,
        Self::LiteralMethodOrConfigurationKeyBytes,
        Self::TargetSpellingBytes,
        Self::OuterAttributesPerDeclaration,
        Self::AttributeTokenBytes,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FrameworkDeclarationsPerSource => "framework_declarations_per_source",
            Self::ExplicitRegistrationChainSegments => "explicit_registration_chain_segments",
            Self::RegistrationExpressionDepth => "registration_expression_depth",
            Self::LiteralRoutePathBytes => "literal_route_path_bytes",
            Self::LiteralMethodOrConfigurationKeyBytes => {
                "literal_method_or_configuration_key_bytes"
            }
            Self::TargetSpellingBytes => "target_spelling_bytes",
            Self::OuterAttributesPerDeclaration => "outer_attributes_per_declaration",
            Self::AttributeTokenBytes => "attribute_token_bytes",
        }
    }

    #[must_use]
    pub const fn maximum(self) -> u64 {
        match self {
            Self::FrameworkDeclarationsPerSource => MAX_R6_FRAMEWORK_DECLARATIONS_PER_SOURCE,
            Self::ExplicitRegistrationChainSegments => MAX_R6_EXPLICIT_REGISTRATION_CHAIN_SEGMENTS,
            Self::RegistrationExpressionDepth => MAX_R6_REGISTRATION_EXPRESSION_DEPTH,
            Self::LiteralRoutePathBytes => MAX_R6_LITERAL_ROUTE_PATH_BYTES,
            Self::LiteralMethodOrConfigurationKeyBytes => {
                MAX_R6_LITERAL_METHOD_OR_CONFIGURATION_KEY_BYTES
            }
            Self::TargetSpellingBytes => MAX_R6_TARGET_SPELLING_BYTES,
            Self::OuterAttributesPerDeclaration => MAX_R6_OUTER_ATTRIBUTES_PER_DECLARATION,
            Self::AttributeTokenBytes => MAX_R6_ATTRIBUTE_TOKEN_BYTES,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FrameworkError {
    InvalidDeclaration {
        path: String,
        reason: String,
    },
    IdentityConflict {
        normalized_preimage_sha256: String,
    },
    LimitExceeded {
        limit: FrameworkLimit,
        maximum: u64,
        observed: u64,
    },
    UnsupportedComposition {
        required_profiles: Vec<String>,
        selected_profiles: Vec<String>,
    },
    AmbiguousTarget {
        target_spelling: String,
        candidate_count: u64,
    },
    UnresolvableEvidence {
        evidence_id: String,
    },
    UnsafePath {
        path: String,
        reason: String,
    },
    Source(RustSemanticError),
    ContractInvalid,
}

impl Display for FrameworkError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidDeclaration { .. } => "invalid framework declaration",
            Self::IdentityConflict { .. } => "framework declaration identity conflict",
            Self::LimitExceeded { .. } => "framework declaration limit exceeded",
            Self::UnsupportedComposition { .. } => "unsupported framework composition",
            Self::AmbiguousTarget { .. } => "ambiguous framework target",
            Self::UnresolvableEvidence { .. } => "unresolvable framework evidence",
            Self::UnsafePath { .. } => "unsafe framework path",
            Self::Source(error) => return Display::fmt(error, formatter),
            Self::ContractInvalid => "invalid framework declaration contract",
        })
    }
}

impl Error for FrameworkError {}

#[must_use]
pub const fn framework_limit_exceeded(limit: FrameworkLimit, observed: u64) -> FrameworkError {
    let maximum = limit.maximum();
    FrameworkError::LimitExceeded {
        limit,
        maximum,
        observed: if observed > maximum + 1 {
            maximum + 1
        } else {
            observed
        },
    }
}

#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn framework_declaration_id(
    repository_identity: &str,
    crate_id: &str,
    lexical_owner_id: &str,
    role: FrameworkRole,
    source_profile: FrameworkSourceProfile,
    source_form_identity: &str,
    normalized_declared_key_or_target: &str,
) -> String {
    let preimage = framework_declaration_identity_preimage(
        repository_identity,
        crate_id,
        lexical_owner_id,
        role,
        source_profile,
        source_form_identity,
        normalized_declared_key_or_target,
    );
    format!(
        "urn:codenoesis:entity:blake3:{}",
        blake3::hash(&preimage).to_hex()
    )
}

#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn framework_declaration_identity_preimage(
    repository_identity: &str,
    crate_id: &str,
    lexical_owner_id: &str,
    role: FrameworkRole,
    source_profile: FrameworkSourceProfile,
    source_form_identity: &str,
    normalized_declared_key_or_target: &str,
) -> Vec<u8> {
    canonical_string_array(&[
        FRAMEWORK_DECLARATION_ID_DOMAIN,
        repository_identity,
        crate_id,
        lexical_owner_id,
        role.as_str(),
        source_profile.as_str(),
        source_form_identity,
        normalized_declared_key_or_target,
    ])
}

#[must_use]
pub const fn framework_capability_state(capability: &str) -> Option<FrameworkCoverageState> {
    match capability.as_bytes() {
        b"rust.attribute_semantics_not_interpreted" | b"rust.framework_source_form_unsupported" => {
            Some(FrameworkCoverageState::Unsupported)
        }
        b"rust.cfg_presence_unresolved" | b"rust.framework_target_resolution_unresolved" => {
            Some(FrameworkCoverageState::NotResolved)
        }
        b"rust.macro_generated_items_not_analyzed" => Some(FrameworkCoverageState::NotAnalyzed),
        b"rust.framework_runtime_not_observed" => Some(FrameworkCoverageState::NotObserved),
        b"rust.framework_target_resolution_ambiguous" => Some(FrameworkCoverageState::Ambiguous),
        _ => None,
    }
}

#[must_use]
pub const fn framework_diagnostic_message(code: &str) -> &'static str {
    match code.as_bytes() {
        b"rust.attribute_semantics_not_interpreted"
        | b"rust.macro_generated_items_not_analyzed" => {
            "Candidate meaning remains unresolved; raw syntax is evidence only."
        }
        b"rust.framework_target_resolution_unresolved" => {
            "Target spelling does not resolve to a committed local R5 declaration."
        }
        b"rust.framework_target_resolution_ambiguous" => {
            "Target spelling resolves to multiple committed local R5 declarations."
        }
        b"rust.framework_source_form_unsupported" => {
            "Source form is outside the closed framework declaration profile."
        }
        _ => "Framework declaration capability remains unresolved.",
    }
}

#[must_use]
pub fn deterministic_framework_claim(
    subject_kind: ClaimSubjectKind,
    subject_id: String,
    evidence_id: String,
) -> WorkspaceClaim {
    WorkspaceClaim::new(
        subject_kind,
        subject_id,
        ClaimState::DeterministicFact,
        vec![evidence_id],
    )
}

#[must_use]
pub fn framework_role_counts(
    declarations: &[FrameworkDeclaration],
) -> BTreeMap<FrameworkRole, usize> {
    let mut counts = BTreeMap::new();
    for declaration in declarations {
        *counts.entry(declaration.role).or_insert(0) += 1;
    }
    counts
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

fn framework_diagnostic_id(declaration_id: &str, code: &str, evidence_ids: &[String]) -> String {
    let joined = evidence_ids.join("\u{1f}");
    stable_artifact_id(
        "urn:codenoesis:diagnostic:blake3:",
        FRAMEWORK_DIAGNOSTIC_ID_DOMAIN,
        &[declaration_id, code, &joined],
    )
}

fn framework_coverage_gap_id(
    declaration_id: &str,
    capability: &str,
    evidence_ids: &[String],
) -> String {
    let joined = evidence_ids.join("\u{1f}");
    stable_artifact_id(
        "urn:codenoesis:coverage-gap:blake3:",
        FRAMEWORK_COVERAGE_GAP_ID_DOMAIN,
        &[declaration_id, capability, &joined],
    )
}

fn stable_artifact_id(prefix: &str, domain: &str, payload: &[&str]) -> String {
    let mut bytes = vec![b'['];
    write_json_string(&mut bytes, domain);
    bytes.push(b',');
    bytes.extend_from_slice(&canonical_string_array(payload));
    bytes.push(b']');
    format!("{prefix}{}", blake3::hash(&bytes).to_hex())
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
