use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::knowledge::{ClaimState, ClaimSubjectKind, EntityKind, RelationshipKind};
use crate::s4::{
    WorkspaceClaim, WorkspaceEntity, WorkspaceEvidence, WorkspaceRelationship, workspace_claim_id,
    workspace_relationship_id,
};
use crate::s4_r4::{CargoManifestFactError, CargoManifestKnowledge};
use crate::s5::{AnalysisCacheEntry, SourceAnalysisRecord};

pub const R5_ONTOLOGY_VERSION: &str = "codenoesis.ontology/rust/v5";
pub const R5_PIPELINE_VERSION: &str = "codenoesis.pipeline/s4-r5-v1";
pub const R5_RUST_SEMANTIC_EXTRACTOR_VERSION: &str = "codenoesis.rust-semantic/s4-r5-v1";
pub const R5_EXTRACTION_CONTRACT_VERSION: &str = "codenoesis.extraction/v5";
pub const R5_RUST_SEMANTIC_PROFILE: &str = "rust-semantic-depth-v1";
pub const R5_RUST_SEMANTIC_INDEX_VERSION: &str = "codenoesis.rust-semantic-index/v1";

pub const MAX_R5_FIELDS_PER_OWNER: u64 = 1_024;
pub const MAX_R5_VARIANTS_PER_ENUM: u64 = 1_024;
pub const MAX_R5_TUPLE_FIELDS_PER_OWNER: u64 = 1_024;
pub const MAX_R5_ASSOCIATED_ITEMS_PER_CONTEXT: u64 = 1_024;
pub const MAX_R5_OUTER_ATTRIBUTES_PER_DECLARATION: u64 = 128;
pub const MAX_R5_ATTRIBUTE_TOKEN_BYTES: u64 = 16_384;
pub const MAX_R5_DECLARED_TYPE_OR_HEADER_BYTES: u64 = 4_096;
pub const R5_DETERMINISM_PERMUTATIONS: u64 = 50;

const MEMBER_ENTITY_ID_DOMAIN: &str = "codenoesis.entity-id/rust-member/v1";
const DIAGNOSTIC_ID_DOMAIN: &str = "codenoesis.diagnostic-id/rust-semantic/v1";
const COVERAGE_GAP_ID_DOMAIN: &str = "codenoesis.coverage-gap-id/rust-semantic/v1";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RustSemanticEntityKind {
    Field,
    EnumVariant,
    Constant,
    Static,
    AssociatedType,
    Method,
}

impl RustSemanticEntityKind {
    pub const ALL: [Self; 6] = [
        Self::AssociatedType,
        Self::Constant,
        Self::EnumVariant,
        Self::Field,
        Self::Method,
        Self::Static,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Field => "rust.field",
            Self::EnumVariant => "rust.enum_variant",
            Self::Constant => "rust.constant",
            Self::Static => "rust.static",
            Self::AssociatedType => "rust.associated_type",
            Self::Method => "rust.method",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RustSemanticVisibility {
    Public,
    Crate,
    Restricted,
    Private,
    InheritedTrait,
    NotApplicable,
}

impl RustSemanticVisibility {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Crate => "crate",
            Self::Restricted => "restricted",
            Self::Private => "private",
            Self::InheritedTrait => "inherited_trait",
            Self::NotApplicable => "not_applicable",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RustSemanticOwnerKind {
    Module,
    Struct,
    Enum,
    EnumVariant,
    Trait,
    InherentImplementation,
    NamedLocalTraitImplementation,
}

impl RustSemanticOwnerKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Module => "rust.module",
            Self::Struct => "rust.struct",
            Self::Enum => "rust.enum",
            Self::EnumVariant => "rust.enum_variant",
            Self::Trait => "rust.trait",
            Self::InherentImplementation => "inherent_implementation",
            Self::NamedLocalTraitImplementation => "named_local_trait_implementation",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RustSemanticForm {
    Named,
    Tuple,
    Unit,
    Struct,
    Constant,
    Static,
    AssociatedType,
}

impl RustSemanticForm {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Named => "named",
            Self::Tuple => "tuple",
            Self::Unit => "unit",
            Self::Struct => "struct",
            Self::Constant => "constant",
            Self::Static => "static",
            Self::AssociatedType => "associated_type",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CompilationPresence {
    Unconditional,
    ConditionalUnknown,
    AttributeTransformUnknown,
}

impl CompilationPresence {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unconditional => "unconditional",
            Self::ConditionalUnknown => "conditional_unknown",
            Self::AttributeTransformUnknown => "attribute_transform_unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RustSemanticAttributeKind {
    Cfg,
    CfgAttr,
    Other,
}

impl RustSemanticAttributeKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cfg => "cfg",
            Self::CfgAttr => "cfg_attr",
            Self::Other => "other",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustSemanticAttribute {
    pub kind: RustSemanticAttributeKind,
    pub token_text: String,
    pub evidence_id: String,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RustMethodContext {
    TraitDeclaration,
    InherentImplementation,
    NamedLocalTraitImplementation,
}

impl RustMethodContext {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TraitDeclaration => "trait_declaration",
            Self::InherentImplementation => "inherent_implementation",
            Self::NamedLocalTraitImplementation => "named_local_trait_implementation",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustMemberProperties {
    pub owner_kind: RustSemanticOwnerKind,
    pub form: RustSemanticForm,
    pub declared_name: Option<String>,
    pub tuple_index: Option<u64>,
    pub declared_type_or_header: Option<String>,
    pub mutable: Option<bool>,
    pub initializer_present: Option<bool>,
    pub discriminant_present: Option<bool>,
    pub bounds_present: Option<bool>,
    pub default_present: Option<bool>,
    pub attributes: Vec<RustSemanticAttribute>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustMethodProperties {
    pub implementation_context: RustMethodContext,
    pub trait_context_id: Option<String>,
    pub receiver_present: bool,
    pub declared_signature: String,
    pub compilation_presence: CompilationPresence,
    pub attributes: Vec<RustSemanticAttribute>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RustSemanticProperties {
    Member(RustMemberProperties),
    Method(RustMethodProperties),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustSemanticEntity {
    pub id: String,
    pub kind: RustSemanticEntityKind,
    pub crate_id: String,
    pub module_path: String,
    pub name: String,
    pub visibility: RustSemanticVisibility,
    pub owner_id: String,
    pub trait_context_id: Option<String>,
    pub compilation_presence: CompilationPresence,
    pub properties: RustSemanticProperties,
}

impl RustSemanticEntity {
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new_member(
        repository_identity: &str,
        kind: RustSemanticEntityKind,
        crate_id: String,
        module_path: String,
        name: String,
        identity_member: &str,
        visibility: RustSemanticVisibility,
        owner_id: String,
        trait_context_id: Option<String>,
        compilation_presence: CompilationPresence,
        properties: RustMemberProperties,
    ) -> Self {
        let id = rust_semantic_member_id(
            repository_identity,
            &crate_id,
            &owner_id,
            kind,
            identity_member,
            trait_context_id.as_deref(),
        );
        Self {
            id,
            kind,
            crate_id,
            module_path,
            name,
            visibility,
            owner_id,
            trait_context_id,
            compilation_presence,
            properties: RustSemanticProperties::Member(properties),
        }
    }

    #[must_use]
    pub fn new_method(
        repository_identity: &str,
        crate_id: String,
        module_path: String,
        name: String,
        visibility: RustSemanticVisibility,
        owner_id: String,
        properties: RustMethodProperties,
    ) -> Self {
        let id = rust_semantic_member_id(
            repository_identity,
            &crate_id,
            &owner_id,
            RustSemanticEntityKind::Method,
            &name,
            properties.trait_context_id.as_deref(),
        );
        Self {
            id,
            kind: RustSemanticEntityKind::Method,
            crate_id,
            module_path,
            name,
            visibility,
            owner_id,
            trait_context_id: properties.trait_context_id.clone(),
            compilation_presence: properties.compilation_presence,
            properties: RustSemanticProperties::Method(properties),
        }
    }

    #[must_use]
    pub fn identity_member(&self) -> String {
        match &self.properties {
            RustSemanticProperties::Member(properties) => properties
                .declared_name
                .clone()
                .or_else(|| properties.tuple_index.map(|index| index.to_string()))
                .unwrap_or_else(|| self.name.clone()),
            RustSemanticProperties::Method(_) => self.name.clone(),
        }
    }

    #[must_use]
    pub fn attributes(&self) -> &[RustSemanticAttribute] {
        match &self.properties {
            RustSemanticProperties::Member(properties) => &properties.attributes,
            RustSemanticProperties::Method(properties) => &properties.attributes,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustSemanticDiagnostic {
    pub id: String,
    pub code: String,
    pub message: String,
    pub evidence_ids: Vec<String>,
}

impl RustSemanticDiagnostic {
    #[must_use]
    pub fn new(
        repository_identity: &str,
        code: &str,
        message: &str,
        evidence_ids: Vec<String>,
    ) -> Self {
        let evidence_ids = stable_dedup(evidence_ids);
        let joined = evidence_ids.join("\u{1f}");
        Self {
            id: stable_id(
                "urn:codenoesis:diagnostic:blake3:",
                &[DIAGNOSTIC_ID_DOMAIN, repository_identity, code, &joined],
            ),
            code: code.to_owned(),
            message: message.to_owned(),
            evidence_ids,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RustSemanticCoverageState {
    Unsupported,
    NotResolved,
    NotAnalyzed,
    NotEvaluated,
}

impl RustSemanticCoverageState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unsupported => "unsupported",
            Self::NotResolved => "not_resolved",
            Self::NotAnalyzed => "not_analyzed",
            Self::NotEvaluated => "not_evaluated",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustSemanticCoverageGap {
    pub id: String,
    pub capability: String,
    pub state: RustSemanticCoverageState,
    pub evidence_ids: Vec<String>,
}

impl RustSemanticCoverageGap {
    #[must_use]
    pub fn new(
        repository_identity: &str,
        commit_oid: &str,
        capability: &str,
        state: RustSemanticCoverageState,
        evidence_ids: Vec<String>,
    ) -> Self {
        let evidence_ids = stable_dedup(evidence_ids);
        let joined = evidence_ids.join("\u{1f}");
        Self {
            id: stable_id(
                "urn:codenoesis:coverage-gap:blake3:",
                &[
                    COVERAGE_GAP_ID_DOMAIN,
                    repository_identity,
                    commit_oid,
                    capability,
                    state.as_str(),
                    &joined,
                ],
            ),
            capability: capability.to_owned(),
            state,
            evidence_ids,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustSemanticSourceChunk {
    pub crate_id: String,
    pub source_file_id: String,
    pub legacy_entities: Vec<WorkspaceEntity>,
    pub entities: Vec<RustSemanticEntity>,
    pub relationships: Vec<WorkspaceRelationship>,
    pub claims: Vec<WorkspaceClaim>,
    pub evidence: Vec<WorkspaceEvidence>,
    pub diagnostics: Vec<RustSemanticDiagnostic>,
    pub coverage: Vec<RustSemanticCoverageGap>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustSemanticIndex {
    pub member_entity_ids: Vec<String>,
    pub implementation_context_method_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustSemanticGraph {
    pub legacy_entities: Vec<WorkspaceEntity>,
    pub entities: Vec<RustSemanticEntity>,
    pub relationships: Vec<WorkspaceRelationship>,
    pub claims: Vec<WorkspaceClaim>,
    pub evidence: Vec<WorkspaceEvidence>,
    pub diagnostics: Vec<RustSemanticDiagnostic>,
    pub coverage: Vec<RustSemanticCoverageGap>,
    pub index: RustSemanticIndex,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustSemanticKnowledge {
    pub manifest: CargoManifestKnowledge,
    pub extraction_chunks: Vec<RustSemanticSourceChunk>,
    pub graph: RustSemanticGraph,
}

impl RustSemanticKnowledge {
    /// Validates the selected R5 additions and the complete inherited R4 knowledge.
    ///
    /// # Errors
    ///
    /// Returns a closed source or R5 contract failure.
    #[allow(clippy::too_many_lines)]
    pub fn validate(&self) -> Result<(), RustSemanticError> {
        self.manifest
            .validate()
            .map_err(RustSemanticError::Source)?;
        let repository_identity = &self.manifest.workspace.knowledge.graph.repository_identity;
        let commit_oid = &self.manifest.workspace.knowledge.graph.commit_oid;
        let base_entity_ids = self
            .manifest
            .workspace
            .knowledge
            .graph
            .entities
            .iter()
            .map(|entity| entity.id.as_str())
            .chain(
                self.manifest
                    .graph
                    .entities
                    .iter()
                    .map(|entity| entity.id.as_str()),
            )
            .collect::<BTreeSet<_>>();
        let legacy_entity_ids = self
            .graph
            .legacy_entities
            .iter()
            .map(|entity| entity.id.as_str())
            .collect::<BTreeSet<_>>();
        let semantic_entity_ids = self
            .graph
            .entities
            .iter()
            .map(|entity| entity.id.as_str())
            .collect::<BTreeSet<_>>();
        let all_entity_ids = base_entity_ids
            .iter()
            .copied()
            .chain(legacy_entity_ids.iter().copied())
            .chain(semantic_entity_ids.iter().copied())
            .collect::<BTreeSet<_>>();
        let evidence_ids = self
            .manifest
            .workspace
            .knowledge
            .graph
            .evidence
            .iter()
            .map(|evidence| evidence.id.as_str())
            .chain(
                self.manifest
                    .graph
                    .evidence
                    .iter()
                    .map(|evidence| evidence.id.as_str()),
            )
            .chain(
                self.graph
                    .evidence
                    .iter()
                    .map(|evidence| evidence.id.as_str()),
            )
            .collect::<BTreeSet<_>>();

        if !additive_collection_shape_is_valid(
            self.graph.entities.len(),
            self.graph.relationships.len(),
            self.graph.claims.len(),
        ) || !ordered_unique(
            self.graph
                .legacy_entities
                .iter()
                .map(|entity| entity.id.as_str()),
        ) || !ordered_unique(self.graph.entities.iter().map(|entity| entity.id.as_str()))
            || !ordered_unique(
                self.graph
                    .relationships
                    .iter()
                    .map(|relationship| relationship.id.as_str()),
            )
            || !ordered_unique(self.graph.claims.iter().map(|claim| claim.id.as_str()))
            || !ordered_unique(
                self.graph
                    .evidence
                    .iter()
                    .map(|evidence| evidence.id.as_str()),
            )
            || !ordered_unique(self.graph.diagnostics.iter().map(|value| value.id.as_str()))
            || !ordered_unique(self.graph.coverage.iter().map(|value| value.id.as_str()))
            || legacy_entity_ids
                .iter()
                .any(|identifier| base_entity_ids.contains(identifier))
        {
            return Err(RustSemanticError::ContractInvalid);
        }

        for entity in &self.graph.entities {
            if entity.id
                != rust_semantic_member_id(
                    repository_identity,
                    &entity.crate_id,
                    &entity.owner_id,
                    entity.kind,
                    &entity.identity_member(),
                    entity.trait_context_id.as_deref(),
                )
                || !all_entity_ids.contains(entity.crate_id.as_str())
                || !all_entity_ids.contains(entity.owner_id.as_str())
                || entity
                    .trait_context_id
                    .as_ref()
                    .is_some_and(|identifier| !all_entity_ids.contains(identifier.as_str()))
                || entity.attributes().len()
                    > usize::try_from(MAX_R5_OUTER_ATTRIBUTES_PER_DECLARATION).unwrap_or(usize::MAX)
                || entity.attributes().iter().any(|attribute| {
                    attribute.token_text.len()
                        > usize::try_from(MAX_R5_ATTRIBUTE_TOKEN_BYTES).unwrap_or(usize::MAX)
                        || !evidence_ids.contains(attribute.evidence_id.as_str())
                })
            {
                return Err(RustSemanticError::ContractInvalid);
            }
        }

        if self.graph.relationships.iter().any(|relationship| {
            !matches!(
                relationship.kind,
                RelationshipKind::Defines | RelationshipKind::Implements
            ) || relationship.id
                != workspace_relationship_id(
                    relationship.kind,
                    &relationship.source,
                    &relationship.target,
                )
                || !all_entity_ids.contains(relationship.source.as_str())
                || !all_entity_ids.contains(relationship.target.as_str())
                || relationship.evidence_ids.len() != 1
                || !evidence_ids.contains(relationship.evidence_ids[0].as_str())
        }) {
            return Err(RustSemanticError::ContractInvalid);
        }

        let subject_ids = semantic_entity_ids
            .iter()
            .copied()
            .chain(
                self.graph
                    .relationships
                    .iter()
                    .map(|relationship| relationship.id.as_str()),
            )
            .chain(legacy_entity_ids.iter().copied())
            .collect::<BTreeSet<_>>();
        if self.graph.claims.iter().any(|claim| {
            claim.state != ClaimState::DeterministicFact
                || claim.id
                    != workspace_claim_id(claim.subject_kind, &claim.subject_id, claim.state)
                || !subject_ids.contains(claim.subject_id.as_str())
                || claim.evidence_ids.len() != 1
                || !evidence_ids.contains(claim.evidence_ids[0].as_str())
        }) || self.graph.diagnostics.iter().any(|diagnostic| {
            !valid_capability(&diagnostic.code)
                || diagnostic.message != diagnostic_message(&diagnostic.code)
                || diagnostic.evidence_ids.is_empty()
                || diagnostic
                    .evidence_ids
                    .iter()
                    .any(|identifier| !evidence_ids.contains(identifier.as_str()))
        }) || self.graph.coverage.iter().any(|gap| {
            capability_state(&gap.capability) != Some(gap.state)
                || gap.evidence_ids.is_empty()
                || gap
                    .evidence_ids
                    .iter()
                    .any(|identifier| !evidence_ids.contains(identifier.as_str()))
        }) {
            return Err(RustSemanticError::ContractInvalid);
        }

        let expected_members = self
            .graph
            .entities
            .iter()
            .map(|entity| entity.id.clone())
            .collect::<Vec<_>>();
        let expected_methods = self
            .graph
            .entities
            .iter()
            .filter(|entity| entity.kind == RustSemanticEntityKind::Method)
            .map(|entity| entity.id.clone())
            .collect::<Vec<_>>();
        if self.graph.index.member_entity_ids != expected_members
            || self.graph.index.implementation_context_method_ids != expected_methods
            || self.extraction_chunks.is_empty()
            || self.extraction_chunks.iter().any(|chunk| {
                !all_entity_ids.contains(chunk.crate_id.as_str())
                    || !all_entity_ids.contains(chunk.source_file_id.as_str())
            })
            || repository_identity.is_empty()
            || commit_oid.is_empty()
        {
            return Err(RustSemanticError::ContractInvalid);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustSemanticDepthExtraction {
    pub knowledge: RustSemanticKnowledge,
    pub cache_entries: Vec<AnalysisCacheEntry>,
    pub source_records: Vec<SourceAnalysisRecord>,
    pub parser_invocation_count: u64,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RustSemanticLimit {
    FieldsPerOwner,
    VariantsPerEnum,
    TupleFieldsPerOwner,
    AssociatedItemsPerContext,
    OuterAttributesPerDeclaration,
    AttributeTokenBytes,
    DeclaredTypeOrHeaderBytes,
}

impl RustSemanticLimit {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FieldsPerOwner => "fields_per_owner",
            Self::VariantsPerEnum => "variants_per_enum",
            Self::TupleFieldsPerOwner => "tuple_fields_per_owner",
            Self::AssociatedItemsPerContext => "associated_items_per_context",
            Self::OuterAttributesPerDeclaration => "outer_attributes_per_declaration",
            Self::AttributeTokenBytes => "attribute_token_bytes",
            Self::DeclaredTypeOrHeaderBytes => "declared_type_or_header_bytes",
        }
    }

    #[must_use]
    pub const fn maximum(self) -> u64 {
        match self {
            Self::FieldsPerOwner => MAX_R5_FIELDS_PER_OWNER,
            Self::VariantsPerEnum => MAX_R5_VARIANTS_PER_ENUM,
            Self::TupleFieldsPerOwner => MAX_R5_TUPLE_FIELDS_PER_OWNER,
            Self::AssociatedItemsPerContext => MAX_R5_ASSOCIATED_ITEMS_PER_CONTEXT,
            Self::OuterAttributesPerDeclaration => MAX_R5_OUTER_ATTRIBUTES_PER_DECLARATION,
            Self::AttributeTokenBytes => MAX_R5_ATTRIBUTE_TOKEN_BYTES,
            Self::DeclaredTypeOrHeaderBytes => MAX_R5_DECLARED_TYPE_OR_HEADER_BYTES,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RustSemanticError {
    InvalidDeclaration {
        path: String,
        start_byte: u64,
        declaration_kind: String,
    },
    IdentityConflict {
        owner_id: String,
        member_kind: String,
        normalized_member: String,
    },
    LimitExceeded {
        limit: RustSemanticLimit,
        maximum: u64,
        observed: u64,
    },
    UnsupportedComposition {
        reason: &'static str,
    },
    Source(CargoManifestFactError),
    ContractInvalid,
}

impl Display for RustSemanticError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidDeclaration { .. } => "invalid Rust semantic declaration",
            Self::IdentityConflict { .. } => "Rust semantic identity conflict",
            Self::LimitExceeded { .. } => "Rust semantic limit exceeded",
            Self::UnsupportedComposition { .. } => "unsupported Rust semantic composition",
            Self::Source(error) => return Display::fmt(error, formatter),
            Self::ContractInvalid => "invalid Rust semantic contract",
        })
    }
}

impl Error for RustSemanticError {}

#[must_use]
pub const fn rust_semantic_limit_exceeded(
    limit: RustSemanticLimit,
    observed: u64,
) -> RustSemanticError {
    let maximum = limit.maximum();
    RustSemanticError::LimitExceeded {
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
pub fn rust_semantic_member_id(
    repository_identity: &str,
    crate_id: &str,
    owner_id: &str,
    kind: RustSemanticEntityKind,
    normalized_member: &str,
    trait_context_id: Option<&str>,
) -> String {
    stable_id(
        "urn:codenoesis:entity:blake3:",
        &[
            MEMBER_ENTITY_ID_DOMAIN,
            repository_identity,
            crate_id,
            owner_id,
            kind.as_str(),
            normalized_member,
            trait_context_id.unwrap_or(""),
        ],
    )
}

#[must_use]
pub const fn capability_state(capability: &str) -> Option<RustSemanticCoverageState> {
    match capability.as_bytes() {
        b"rust.attribute_semantics_not_interpreted"
        | b"rust.union_unsupported"
        | b"rust.foreign_block_unsupported"
        | b"rust.unsupported_impl_header" => Some(RustSemanticCoverageState::Unsupported),
        b"rust.cfg_presence_unresolved" | b"rust.type_resolution_not_performed" => {
            Some(RustSemanticCoverageState::NotResolved)
        }
        b"rust.macro_generated_items_not_analyzed" => Some(RustSemanticCoverageState::NotAnalyzed),
        b"rust.value_not_evaluated" => Some(RustSemanticCoverageState::NotEvaluated),
        _ => None,
    }
}

#[must_use]
pub const fn diagnostic_message(code: &str) -> &'static str {
    match code.as_bytes() {
        b"rust.attribute_semantics_not_interpreted" => {
            "Rust attribute semantics are not interpreted"
        }
        b"rust.cfg_presence_unresolved" => "Rust cfg presence is not resolved",
        b"rust.macro_generated_items_not_analyzed" => "Rust macro-generated items are not analyzed",
        b"rust.type_resolution_not_performed" => "Rust declared types are not resolved",
        b"rust.value_not_evaluated" => "Rust declared values are not evaluated",
        b"rust.union_unsupported" => "Rust union declarations are unsupported",
        b"rust.foreign_block_unsupported" => "Rust foreign blocks are unsupported",
        b"rust.unsupported_impl_header" => "Rust implementation header is unsupported",
        _ => "unsupported Rust semantic capability",
    }
}

fn valid_capability(capability: &str) -> bool {
    capability_state(capability).is_some()
}

const fn additive_collection_shape_is_valid(
    entity_count: usize,
    relationship_count: usize,
    claim_count: usize,
) -> bool {
    entity_count != 0 || (relationship_count == 0 && claim_count == 0)
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
pub fn r5_entity_counts(
    entities: &[RustSemanticEntity],
) -> BTreeMap<RustSemanticEntityKind, usize> {
    let mut counts = BTreeMap::new();
    for entity in entities {
        *counts.entry(entity.kind).or_insert(0) += 1;
    }
    counts
}

#[must_use]
pub const fn legacy_owner_kind(kind: EntityKind) -> Option<RustSemanticOwnerKind> {
    match kind {
        EntityKind::RustModule => Some(RustSemanticOwnerKind::Module),
        EntityKind::RustStruct => Some(RustSemanticOwnerKind::Struct),
        EntityKind::RustEnum => Some(RustSemanticOwnerKind::Enum),
        EntityKind::RustTrait => Some(RustSemanticOwnerKind::Trait),
        _ => None,
    }
}

#[must_use]
pub fn deterministic_claim(
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

#[cfg(test)]
mod tests {
    use super::additive_collection_shape_is_valid;

    #[test]
    fn ut_fr_ext_010_empty_additive_collections_are_neutral_only_together() {
        assert!(additive_collection_shape_is_valid(0, 0, 0));
        assert!(additive_collection_shape_is_valid(1, 1, 1));
        assert!(!additive_collection_shape_is_valid(0, 1, 0));
        assert!(!additive_collection_shape_is_valid(0, 0, 1));
        assert!(!additive_collection_shape_is_valid(0, 1, 1));
    }
}
