use std::collections::{BTreeMap, BTreeSet};

use codenoesis_domain::RepositoryInventory;
use codenoesis_domain::knowledge::{ClaimSubjectKind, RelationshipKind};
use codenoesis_domain::s4::WorkspaceEvidence;
use codenoesis_domain::s4_k1::CallableSemanticEntityKind;
use codenoesis_domain::s4_r3::ExternalWorkspaceBoundary;
use codenoesis_domain::s4_r5::{
    CompilationPresence, RustSemanticAttributeKind, RustSemanticEntity, RustSemanticEntityKind,
    RustSemanticForm, RustSemanticOwnerKind, RustSemanticProperties,
};
use codenoesis_domain::s4_r16::{
    ConstantEvaluationCoverageGap, ConstantEvaluationDerivation, ConstantEvaluationError,
    ConstantEvaluationExtraction, ConstantEvaluationGraph, ConstantEvaluationIndex,
    ConstantEvaluationLimit, ConstantEvaluationRelationship, ConstantEvaluationSourceOverlay,
    ConstantTypeAuthority, ConstantValueKind, EvaluatedValue, constant_evaluation_claim,
    enforce_constant_limit,
};
use codenoesis_ports::RustConstantEvaluationExtractor;
use tree_sitter::Node;

use crate::TreeSitterRustWorkspaceExtractor;
use crate::semantic_depth::{parse_tree, source_contexts, source_text};

const TARGET_DEPENDENT: &str = "rust.constant_target_dependent";
const EXPRESSION_NOT_EVALUATED: &str = "rust.constant_expression_not_evaluated";
const DEPENDENCY_NOT_EVALUATED: &str = "rust.constant_dependency_not_evaluated";
const ARITHMETIC_NOT_DEFINED: &str = "rust.constant_arithmetic_not_defined";
const ENUM_NOT_EVALUATED: &str = "rust.enum_discriminant_not_evaluated";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PrimitiveType {
    Bool,
    Signed(u32),
    Unsigned(u32),
}

impl PrimitiveType {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "bool" => Some(Self::Bool),
            "i8" => Some(Self::Signed(8)),
            "i16" => Some(Self::Signed(16)),
            "i32" => Some(Self::Signed(32)),
            "i64" => Some(Self::Signed(64)),
            "i128" => Some(Self::Signed(128)),
            "u8" => Some(Self::Unsigned(8)),
            "u16" => Some(Self::Unsigned(16)),
            "u32" => Some(Self::Unsigned(32)),
            "u64" => Some(Self::Unsigned(64)),
            "u128" => Some(Self::Unsigned(128)),
            _ => None,
        }
    }

    const fn kind(self) -> ConstantValueKind {
        match self {
            Self::Bool => ConstantValueKind::Boolean,
            Self::Signed(_) | Self::Unsigned(_) => ConstantValueKind::Integer,
        }
    }

    const fn signed_bounds(self) -> Option<(i128, i128)> {
        match self {
            Self::Signed(128) => Some((i128::MIN, i128::MAX)),
            Self::Signed(bits) => {
                let maximum = (1_i128 << (bits - 1)) - 1;
                Some((-maximum - 1, maximum))
            }
            Self::Bool | Self::Unsigned(_) => None,
        }
    }

    const fn unsigned_maximum(self) -> Option<u128> {
        match self {
            Self::Unsigned(128) => Some(u128::MAX),
            Self::Unsigned(bits) => Some((1_u128 << bits) - 1),
            Self::Bool | Self::Signed(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConstantValue {
    Bool(bool),
    Signed(i128),
    Unsigned(u128),
}

impl ConstantValue {
    fn canonical(&self) -> String {
        match self {
            Self::Bool(value) => value.to_string(),
            Self::Signed(value) => value.to_string(),
            Self::Unsigned(value) => value.to_string(),
        }
    }
}

#[derive(Clone, Debug)]
enum Expression {
    Parsed(Vec<Token>),
    EnumImplicit(Option<String>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Token {
    Bool(bool),
    Integer(String),
    Identifier(String),
    Operator(&'static str),
    LeftParenthesis,
    RightParenthesis,
}

#[derive(Clone, Debug)]
struct Candidate {
    declared_value_id: String,
    declared_value_claim_id: String,
    owner_id: String,
    name: String,
    rust_type: String,
    primitive: PrimitiveType,
    type_authority: ConstantTypeAuthority,
    evidence_ids: Vec<String>,
    declaration_evidence_ids: Vec<String>,
    source_file_id: String,
    expression: Expression,
    explicit_value: bool,
    dependency_source: bool,
    enum_id: Option<String>,
    repr_evidence_id: Option<String>,
}

#[derive(Clone, Debug)]
struct UnsupportedSubject {
    capability: &'static str,
    subject_id: String,
    evidence_ids: Vec<String>,
    legacy_evidence_ids: Vec<String>,
    source_file_id: String,
}

#[derive(Clone, Debug)]
struct Evaluation {
    value: ConstantValue,
    dependency_ids: Vec<String>,
    dependency_levels: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EvaluationFailure {
    Expression,
    Dependency,
    Arithmetic,
    Limit {
        limit: ConstantEvaluationLimit,
        maximum: u64,
        observed: u64,
    },
}

impl EvaluationFailure {
    const fn capability(self) -> &'static str {
        match self {
            Self::Expression => EXPRESSION_NOT_EVALUATED,
            Self::Dependency | Self::Limit { .. } => DEPENDENCY_NOT_EVALUATED,
            Self::Arithmetic => ARITHMETIC_NOT_DEFINED,
        }
    }

    fn from_limit(error: &ConstantEvaluationError) -> Self {
        match error {
            ConstantEvaluationError::LimitExceeded {
                limit,
                maximum,
                observed,
            } => Self::Limit {
                limit: *limit,
                maximum: *maximum,
                observed: *observed,
            },
            _ => Self::Dependency,
        }
    }

    const fn constant_error(self) -> Option<ConstantEvaluationError> {
        match self {
            Self::Limit {
                limit,
                maximum,
                observed,
            } => Some(ConstantEvaluationError::LimitExceeded {
                limit,
                maximum,
                observed,
            }),
            Self::Expression | Self::Dependency | Self::Arithmetic => None,
        }
    }
}

struct ExtractionCatalog<'a> {
    semantic_entities: BTreeMap<&'a str, &'a RustSemanticEntity>,
    declared_values: BTreeMap<&'a str, &'a codenoesis_domain::s4_k1::CallableSemanticEntity>,
    declared_by_span: BTreeMap<(&'a str, u64, u64), &'a str>,
    ambiguous_declared_values: BTreeSet<&'a str>,
    claims_by_subject: BTreeMap<&'a str, &'a codenoesis_domain::s4::WorkspaceClaim>,
    declaration_evidence_by_subject: BTreeMap<&'a str, &'a Vec<String>>,
    evidence: BTreeMap<&'a str, &'a WorkspaceEvidence>,
    semantic_coverage: Vec<&'a codenoesis_domain::s4_r5::RustSemanticCoverageGap>,
    semantic_diagnostics: Vec<&'a codenoesis_domain::s4_r5::RustSemanticDiagnostic>,
}

impl TreeSitterRustWorkspaceExtractor {
    /// Extracts the exact closed R16 target-independent constant-evaluation profile.
    ///
    /// # Errors
    ///
    /// Returns an inherited R15 or typed source, identity, dependency, arithmetic, or limit
    /// failure.
    #[allow(clippy::too_many_lines)]
    pub fn extract_rust_constant_evaluation(
        &self,
        inventory: &RepositoryInventory,
    ) -> Result<ConstantEvaluationExtraction, ConstantEvaluationError> {
        let local_flow = self
            .extract_rust_local_flow(inventory)
            .map_err(ConstantEvaluationError::Source)?;
        Self::extract_constant_evaluation_from_local_flow(inventory, local_flow)
    }

    /// Extracts R16 over the exact R12 cfg-alternatives and repository-boundary lineage.
    ///
    /// # Errors
    ///
    /// Returns an inherited R12-R15 or typed source, dependency, arithmetic, or limit failure.
    pub fn extract_rust_constant_evaluation_with_cfg_alternatives(
        &self,
        inventory: &RepositoryInventory,
        external_boundaries: &[ExternalWorkspaceBoundary],
    ) -> Result<ConstantEvaluationExtraction, ConstantEvaluationError> {
        let local_flow = self
            .extract_rust_local_flow_with_cfg_alternatives(inventory, external_boundaries)
            .map_err(ConstantEvaluationError::Source)?;
        Self::extract_constant_evaluation_from_local_flow(inventory, local_flow)
    }

    #[allow(clippy::too_many_lines)]
    fn extract_constant_evaluation_from_local_flow(
        inventory: &RepositoryInventory,
        local_flow: codenoesis_domain::s4_r15::LocalFlowExtraction,
    ) -> Result<ConstantEvaluationExtraction, ConstantEvaluationError> {
        let contexts = source_contexts(
            &local_flow
                .knowledge
                .expression
                .callable
                .framework
                .semantic
                .manifest,
            inventory,
        )
        .map_err(|error| {
            ConstantEvaluationError::Source(codenoesis_domain::s4_r15::LocalFlowError::Source(
                codenoesis_domain::s4_r14::ExpressionBindingError::Source(
                    codenoesis_domain::s4_k1::CallableSemanticsError::Source(
                        codenoesis_domain::s4_r6::FrameworkError::Source(error),
                    ),
                ),
            ))
        })?;
        let catalog = extraction_catalog(&local_flow.knowledge)?;
        let mut candidates = BTreeMap::new();
        let mut unsupported = Vec::new();
        let mut enum_evidence = BTreeMap::new();
        for context in &contexts {
            let source = source_text(context).map_err(|error| {
                ConstantEvaluationError::Source(codenoesis_domain::s4_r15::LocalFlowError::Source(
                    codenoesis_domain::s4_r14::ExpressionBindingError::Source(
                        codenoesis_domain::s4_k1::CallableSemanticsError::Source(
                            codenoesis_domain::s4_r6::FrameworkError::Source(error),
                        ),
                    ),
                ))
            })?;
            let tree = parse_tree(&context.path, source).map_err(|error| {
                ConstantEvaluationError::Source(codenoesis_domain::s4_r15::LocalFlowError::Source(
                    codenoesis_domain::s4_r14::ExpressionBindingError::Source(
                        codenoesis_domain::s4_k1::CallableSemanticsError::Source(
                            codenoesis_domain::s4_r6::FrameworkError::Source(error),
                        ),
                    ),
                ))
            })?;
            collect_source_candidates(
                tree.root_node(),
                source,
                &context.path,
                &context.source_file_id,
                &catalog,
                &mut candidates,
                &mut unsupported,
                &mut enum_evidence,
            )?;
        }
        let mut states = BTreeMap::new();
        let mut evaluated = BTreeMap::new();
        let candidate_ids = candidates.keys().cloned().collect::<Vec<_>>();
        for identifier in candidate_ids {
            if let Err(failure) =
                evaluate_candidate(&identifier, &candidates, &mut states, &mut evaluated)
                && let Some(error) = failure.constant_error()
            {
                return Err(error);
            }
        }
        let failed_enums = candidates
            .values()
            .filter_map(|candidate| {
                candidate
                    .enum_id
                    .as_ref()
                    .filter(|_| !evaluated.contains_key(&candidate.declared_value_id))
            })
            .cloned()
            .collect::<BTreeSet<_>>();
        for enum_id in &failed_enums {
            evaluated.retain(|identifier, _| {
                candidates
                    .get(identifier)
                    .is_none_or(|candidate| candidate.enum_id.as_ref() != Some(enum_id))
            });
            if let Some((source_file_id, evidence_ids)) = enum_evidence.get(enum_id) {
                unsupported.push(UnsupportedSubject {
                    capability: ENUM_NOT_EVALUATED,
                    subject_id: enum_id.clone(),
                    evidence_ids: evidence_ids.clone(),
                    legacy_evidence_ids: Vec::new(),
                    source_file_id: source_file_id.clone(),
                });
            }
        }
        for candidate in candidates.values() {
            if candidate.enum_id.is_none() && !evaluated.contains_key(&candidate.declared_value_id)
            {
                let failure = states
                    .get(&candidate.declared_value_id)
                    .and_then(|state| match state {
                        EvaluationState::Failed(failure) => Some(*failure),
                        EvaluationState::Visiting | EvaluationState::Complete => None,
                    })
                    .unwrap_or(EvaluationFailure::Dependency);
                unsupported.push(UnsupportedSubject {
                    capability: failure.capability(),
                    subject_id: candidate.declared_value_id.clone(),
                    evidence_ids: candidate.evidence_ids.clone(),
                    legacy_evidence_ids: candidate.declaration_evidence_ids.clone(),
                    source_file_id: candidate.source_file_id.clone(),
                });
            }
        }
        unsupported.sort_by(|left, right| {
            (&left.source_file_id, &left.subject_id, left.capability).cmp(&(
                &right.source_file_id,
                &right.subject_id,
                right.capability,
            ))
        });
        unsupported.dedup_by(|left, right| {
            left.capability == right.capability && left.subject_id == right.subject_id
        });
        let graph = build_graph(
            inventory.bound_revision().repository_identity().as_str(),
            &candidates,
            &evaluated,
            &unsupported,
            &catalog,
            &failed_enums,
            &enum_evidence,
        )?;
        let source_overlays = source_overlays(&contexts, &graph, &catalog);
        let parser_invocation_count = local_flow
            .parser_invocation_count
            .saturating_add(u64::try_from(contexts.len()).unwrap_or(u64::MAX));
        let extraction = ConstantEvaluationExtraction::from_r15(
            local_flow,
            source_overlays,
            graph,
            parser_invocation_count,
        );
        extraction.knowledge.validate()?;
        Ok(extraction)
    }
}

impl RustConstantEvaluationExtractor for TreeSitterRustWorkspaceExtractor {
    fn extract_rust_constant_evaluation(
        &self,
        inventory: &RepositoryInventory,
    ) -> Result<ConstantEvaluationExtraction, ConstantEvaluationError> {
        TreeSitterRustWorkspaceExtractor::extract_rust_constant_evaluation(self, inventory)
    }
}

#[allow(clippy::too_many_lines)]
fn extraction_catalog(
    knowledge: &codenoesis_domain::s4_r15::LocalFlowKnowledge,
) -> Result<ExtractionCatalog<'_>, ConstantEvaluationError> {
    let semantic = &knowledge.expression.callable.framework.semantic.graph;
    let callable = &knowledge.expression.callable.graph;
    let semantic_entities = semantic
        .entities
        .iter()
        .map(|entity| (entity.id.as_str(), entity))
        .collect();
    let declared_values = callable
        .entities
        .iter()
        .filter(|entity| entity.kind == CallableSemanticEntityKind::DeclaredValue)
        .map(|entity| (entity.id.as_str(), entity))
        .collect::<BTreeMap<_, _>>();
    let mut evidence = semantic
        .evidence
        .iter()
        .chain(&callable.evidence)
        .chain(&knowledge.expression.graph.evidence)
        .chain(&knowledge.graph.evidence)
        .map(|value| (value.id.as_str(), value))
        .collect::<BTreeMap<_, _>>();
    for source in &knowledge
        .expression
        .callable
        .framework
        .semantic
        .extraction_chunks
    {
        for value in &source.evidence {
            evidence.entry(value.id.as_str()).or_insert(value);
        }
    }
    for source in &knowledge.expression.callable.extraction_chunks {
        for value in &source.evidence {
            evidence.entry(value.id.as_str()).or_insert(value);
        }
    }
    let mut declared_by_span = BTreeMap::new();
    let mut ambiguous_declared_values = BTreeSet::new();
    for (identifier, entity) in &declared_values {
        if entity.evidence_ids.is_empty() {
            return Err(ConstantEvaluationError::ContractInvalid);
        }
        if entity.evidence_ids.len() > 1 {
            ambiguous_declared_values.insert(*identifier);
        }
        for evidence_id in &entity.evidence_ids {
            let value = evidence
                .get(evidence_id.as_str())
                .copied()
                .ok_or(ConstantEvaluationError::ContractInvalid)?;
            if declared_by_span
                .insert(
                    (value.path.as_str(), value.start_byte, value.end_byte),
                    *identifier,
                )
                .is_some_and(|existing| existing != *identifier)
            {
                return Err(ConstantEvaluationError::IdentityConflict);
            }
        }
    }
    let mut claims_by_subject = semantic
        .claims
        .iter()
        .map(|claim| (claim.subject_id.as_str(), claim))
        .collect::<BTreeMap<_, _>>();
    for claim in knowledge
        .expression
        .callable
        .framework
        .semantic
        .extraction_chunks
        .iter()
        .flat_map(|source| &source.claims)
        .chain(&callable.claims)
        .chain(&knowledge.expression.graph.claims)
        .chain(&knowledge.graph.claims)
    {
        claims_by_subject
            .entry(claim.subject_id.as_str())
            .or_insert(claim);
    }
    let mut declaration_evidence_by_subject = semantic
        .relationships
        .iter()
        .filter(|relationship| relationship.kind == RelationshipKind::Defines)
        .map(|relationship| (relationship.target.as_str(), &relationship.evidence_ids))
        .collect::<BTreeMap<_, _>>();
    for relationship in knowledge
        .expression
        .callable
        .framework
        .semantic
        .extraction_chunks
        .iter()
        .flat_map(|source| &source.relationships)
        .filter(|relationship| relationship.kind == RelationshipKind::Defines)
    {
        declaration_evidence_by_subject
            .entry(relationship.target.as_str())
            .or_insert(&relationship.evidence_ids);
    }
    Ok(ExtractionCatalog {
        semantic_entities,
        declared_values,
        declared_by_span,
        ambiguous_declared_values,
        claims_by_subject,
        declaration_evidence_by_subject,
        evidence,
        semantic_coverage: semantic.coverage.iter().collect(),
        semantic_diagnostics: semantic.diagnostics.iter().collect(),
    })
}

#[allow(clippy::too_many_arguments)]
fn collect_source_candidates(
    root: Node<'_>,
    source: &str,
    path: &str,
    source_file_id: &str,
    catalog: &ExtractionCatalog<'_>,
    candidates: &mut BTreeMap<String, Candidate>,
    unsupported: &mut Vec<UnsupportedSubject>,
    enum_evidence: &mut BTreeMap<String, (String, Vec<String>)>,
) -> Result<(), ConstantEvaluationError> {
    let mut declaration_count = 0_usize;
    let mut cursor = root.walk();
    for node in root.named_children(&mut cursor) {
        collect_node_candidates(
            node,
            source,
            path,
            source_file_id,
            catalog,
            candidates,
            unsupported,
            enum_evidence,
            &mut declaration_count,
        )?;
    }
    enforce_constant_limit(
        ConstantEvaluationLimit::CandidatesPerSource,
        declaration_count,
    )
}

#[allow(clippy::too_many_arguments)]
fn collect_node_candidates(
    node: Node<'_>,
    source: &str,
    path: &str,
    source_file_id: &str,
    catalog: &ExtractionCatalog<'_>,
    candidates: &mut BTreeMap<String, Candidate>,
    unsupported: &mut Vec<UnsupportedSubject>,
    enum_evidence: &mut BTreeMap<String, (String, Vec<String>)>,
    declaration_count: &mut usize,
) -> Result<(), ConstantEvaluationError> {
    if matches!(
        node.kind(),
        "macro_invocation" | "macro_definition" | "function_item" | "impl_item" | "trait_item"
    ) {
        return Ok(());
    }
    match node.kind() {
        "const_item" | "static_item" => {
            *declaration_count = declaration_count.saturating_add(1);
            collect_typed_value(
                node,
                source,
                path,
                source_file_id,
                catalog,
                candidates,
                unsupported,
            )?;
        }
        "enum_item" => {
            collect_enum(
                node,
                source,
                path,
                source_file_id,
                catalog,
                candidates,
                unsupported,
                enum_evidence,
                declaration_count,
            )?;
            return Ok(());
        }
        _ => {}
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_node_candidates(
            child,
            source,
            path,
            source_file_id,
            catalog,
            candidates,
            unsupported,
            enum_evidence,
            declaration_count,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_typed_value(
    node: Node<'_>,
    source: &str,
    path: &str,
    source_file_id: &str,
    catalog: &ExtractionCatalog<'_>,
    candidates: &mut BTreeMap<String, Candidate>,
    unsupported: &mut Vec<UnsupportedSubject>,
) -> Result<(), ConstantEvaluationError> {
    let Some(value_node) = node.child_by_field_name("value") else {
        return Ok(());
    };
    let Some(declared_id) = declared_id_for_node(catalog, path, value_node) else {
        return Ok(());
    };
    let declared = catalog
        .declared_values
        .get(declared_id)
        .copied()
        .ok_or(ConstantEvaluationError::ContractInvalid)?;
    let semantic = catalog
        .semantic_entities
        .get(declared.subject_id.as_str())
        .copied()
        .ok_or(ConstantEvaluationError::ContractInvalid)?;
    let RustSemanticProperties::Member(properties) = &semantic.properties else {
        return Ok(());
    };
    let eligible_kind = match semantic.kind {
        RustSemanticEntityKind::Constant => properties.owner_kind == RustSemanticOwnerKind::Module,
        RustSemanticEntityKind::Static => {
            properties.owner_kind == RustSemanticOwnerKind::Module
                && properties.mutable == Some(false)
        }
        _ => false,
    };
    let supported_presence = semantic.compilation_presence
        != CompilationPresence::AttributeTransformUnknown
        && properties
            .attributes
            .iter()
            .all(|attribute| attribute.kind == RustSemanticAttributeKind::Cfg);
    if !eligible_kind || !supported_presence {
        return Ok(());
    }
    let Some(rust_type) = properties.declared_type_or_header.as_deref() else {
        return Ok(());
    };
    let evidence_ids = declared.evidence_ids.clone();
    let declaration_evidence_ids = catalog
        .claims_by_subject
        .get(semantic.id.as_str())
        .map(|claim| claim.evidence_ids.clone())
        .ok_or(ConstantEvaluationError::ContractInvalid)?;
    if catalog.ambiguous_declared_values.contains(declared_id) {
        unsupported.push(UnsupportedSubject {
            capability: EXPRESSION_NOT_EVALUATED,
            subject_id: declared.id.clone(),
            evidence_ids,
            legacy_evidence_ids: declaration_evidence_ids,
            source_file_id: source_file_id.to_owned(),
        });
        return Ok(());
    }
    if matches!(rust_type, "usize" | "isize") {
        unsupported.push(UnsupportedSubject {
            capability: TARGET_DEPENDENT,
            subject_id: declared.id.clone(),
            evidence_ids,
            legacy_evidence_ids: declaration_evidence_ids,
            source_file_id: source_file_id.to_owned(),
        });
        return Ok(());
    }
    let Some(primitive) = PrimitiveType::parse(rust_type) else {
        unsupported.push(UnsupportedSubject {
            capability: EXPRESSION_NOT_EVALUATED,
            subject_id: declared.id.clone(),
            evidence_ids,
            legacy_evidence_ids: declaration_evidence_ids,
            source_file_id: source_file_id.to_owned(),
        });
        return Ok(());
    };
    let expression = if let Ok(tokens) = tokenize(node_text(value_node, source)) {
        Expression::Parsed(tokens)
    } else {
        unsupported.push(UnsupportedSubject {
            capability: EXPRESSION_NOT_EVALUATED,
            subject_id: declared.id.clone(),
            evidence_ids,
            legacy_evidence_ids: declaration_evidence_ids,
            source_file_id: source_file_id.to_owned(),
        });
        return Ok(());
    };
    let candidate = candidate(
        declared,
        semantic,
        rust_type,
        primitive,
        ConstantTypeAuthority::ExplicitPrimitiveAnnotation,
        expression,
        source_file_id,
        true,
        None,
        catalog,
    )?;
    insert_candidate(candidates, candidate)
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn collect_enum(
    node: Node<'_>,
    source: &str,
    path: &str,
    source_file_id: &str,
    catalog: &ExtractionCatalog<'_>,
    candidates: &mut BTreeMap<String, Candidate>,
    unsupported: &mut Vec<UnsupportedSubject>,
    enum_evidence: &mut BTreeMap<String, (String, Vec<String>)>,
    declaration_count: &mut usize,
) -> Result<(), ConstantEvaluationError> {
    let Some(body) = node.child_by_field_name("body") else {
        return Ok(());
    };
    let mut variants = Vec::new();
    let mut cursor = body.walk();
    for variant in body.named_children(&mut cursor) {
        if variant.kind() == "enum_variant" {
            variants.push(variant);
        }
    }
    enforce_constant_limit(ConstantEvaluationLimit::VariantsPerEnum, variants.len())?;
    *declaration_count = declaration_count.saturating_add(variants.len());
    if variants.is_empty() {
        return Ok(());
    }
    let first_declared_id = declared_id_for_variant(catalog, path, variants[0]);
    let Some(first_declared_id) = first_declared_id else {
        return Ok(());
    };
    let first_declared = catalog
        .declared_values
        .get(first_declared_id)
        .copied()
        .ok_or(ConstantEvaluationError::ContractInvalid)?;
    let first_semantic = catalog
        .semantic_entities
        .get(first_declared.subject_id.as_str())
        .copied()
        .ok_or(ConstantEvaluationError::ContractInvalid)?;
    let enum_id = first_semantic.owner_id.clone();
    let enum_node_evidence = catalog
        .claims_by_subject
        .get(enum_id.as_str())
        .map(|claim| claim.evidence_ids.clone())
        .or_else(|| {
            catalog
                .declaration_evidence_by_subject
                .get(enum_id.as_str())
                .map(|evidence_ids| (*evidence_ids).clone())
        })
        .or_else(|| {
            evidence_for_span(catalog, path, node.start_byte(), node.end_byte())
                .map(|evidence| vec![evidence.id.clone()])
        })
        .or_else(|| {
            evidence_for_span(catalog, path, 0, source.len())
                .map(|evidence| vec![evidence.id.clone()])
        })
        .unwrap_or_else(|| first_declared.evidence_ids.clone());
    enum_evidence.insert(
        enum_id.clone(),
        (source_file_id.to_owned(), enum_node_evidence.clone()),
    );
    if variants.iter().any(|variant| {
        declared_id_for_variant(catalog, path, *variant)
            .is_some_and(|identifier| catalog.ambiguous_declared_values.contains(identifier))
    }) {
        push_enum_gap(unsupported, &enum_id, &enum_node_evidence, source_file_id);
        return Ok(());
    }
    let Some((rust_type, repr_evidence_id)) = direct_repr(node, source, path, catalog) else {
        push_enum_gap(unsupported, &enum_id, &enum_node_evidence, source_file_id);
        return Ok(());
    };
    let Some(primitive) = PrimitiveType::parse(&rust_type) else {
        push_enum_gap(unsupported, &enum_id, &enum_node_evidence, source_file_id);
        return Ok(());
    };
    if primitive == PrimitiveType::Bool {
        push_enum_gap(unsupported, &enum_id, &enum_node_evidence, source_file_id);
        return Ok(());
    }
    let mut staged = Vec::new();
    let mut previous = None;
    for variant in variants {
        if variant.child_by_field_name("body").is_some() {
            push_enum_gap(unsupported, &enum_id, &enum_node_evidence, source_file_id);
            return Ok(());
        }
        let Some(declared_id) = declared_id_for_variant(catalog, path, variant) else {
            push_enum_gap(unsupported, &enum_id, &enum_node_evidence, source_file_id);
            return Ok(());
        };
        let declared = catalog
            .declared_values
            .get(declared_id)
            .copied()
            .ok_or(ConstantEvaluationError::ContractInvalid)?;
        let semantic = catalog
            .semantic_entities
            .get(declared.subject_id.as_str())
            .copied()
            .ok_or(ConstantEvaluationError::ContractInvalid)?;
        let RustSemanticProperties::Member(properties) = &semantic.properties else {
            push_enum_gap(unsupported, &enum_id, &enum_node_evidence, source_file_id);
            return Ok(());
        };
        if semantic.kind != RustSemanticEntityKind::EnumVariant
            || semantic.owner_id != enum_id
            || semantic.compilation_presence != CompilationPresence::Unconditional
            || properties.form != RustSemanticForm::Unit
            || !properties.attributes.is_empty()
        {
            push_enum_gap(unsupported, &enum_id, &enum_node_evidence, source_file_id);
            return Ok(());
        }
        let expression = if let Some(value) = variant.child_by_field_name("value") {
            if let Ok(tokens) = tokenize(node_text(value, source)) {
                Expression::Parsed(tokens)
            } else {
                push_enum_gap(unsupported, &enum_id, &enum_node_evidence, source_file_id);
                return Ok(());
            }
        } else {
            Expression::EnumImplicit(previous.clone())
        };
        let mut candidate = candidate(
            declared,
            semantic,
            &rust_type,
            primitive,
            ConstantTypeAuthority::FixedReprAttribute,
            expression,
            source_file_id,
            variant.child_by_field_name("value").is_some(),
            Some(enum_id.clone()),
            catalog,
        )?;
        candidate.evidence_ids.push(repr_evidence_id.clone());
        candidate.evidence_ids.sort();
        candidate.evidence_ids.dedup();
        candidate.repr_evidence_id = Some(repr_evidence_id.clone());
        previous = Some(candidate.declared_value_id.clone());
        staged.push(candidate);
    }
    for candidate in staged {
        insert_candidate(candidates, candidate)?;
    }
    Ok(())
}

fn push_enum_gap(
    unsupported: &mut Vec<UnsupportedSubject>,
    enum_id: &str,
    evidence_ids: &[String],
    source_file_id: &str,
) {
    unsupported.push(UnsupportedSubject {
        capability: ENUM_NOT_EVALUATED,
        subject_id: enum_id.to_owned(),
        evidence_ids: evidence_ids.to_vec(),
        legacy_evidence_ids: Vec::new(),
        source_file_id: source_file_id.to_owned(),
    });
}

#[allow(clippy::too_many_arguments)]
fn candidate(
    declared: &codenoesis_domain::s4_k1::CallableSemanticEntity,
    semantic: &RustSemanticEntity,
    rust_type: &str,
    primitive: PrimitiveType,
    type_authority: ConstantTypeAuthority,
    expression: Expression,
    source_file_id: &str,
    explicit_value: bool,
    enum_id: Option<String>,
    catalog: &ExtractionCatalog<'_>,
) -> Result<Candidate, ConstantEvaluationError> {
    let claim = catalog
        .claims_by_subject
        .get(declared.id.as_str())
        .copied()
        .ok_or(ConstantEvaluationError::ContractInvalid)?;
    let declaration_claim = catalog
        .claims_by_subject
        .get(semantic.id.as_str())
        .copied()
        .ok_or(ConstantEvaluationError::ContractInvalid)?;
    Ok(Candidate {
        declared_value_id: declared.id.clone(),
        declared_value_claim_id: claim.id.clone(),
        owner_id: semantic.owner_id.clone(),
        name: declared.name.clone(),
        rust_type: rust_type.to_owned(),
        primitive,
        type_authority,
        evidence_ids: declared.evidence_ids.clone(),
        declaration_evidence_ids: declaration_claim.evidence_ids.clone(),
        source_file_id: source_file_id.to_owned(),
        expression,
        explicit_value,
        dependency_source: semantic.kind == RustSemanticEntityKind::Constant
            && semantic.compilation_presence == CompilationPresence::Unconditional
            && matches!(
                &semantic.properties,
                RustSemanticProperties::Member(properties) if properties.attributes.is_empty()
            ),
        enum_id,
        repr_evidence_id: None,
    })
}

fn insert_candidate(
    candidates: &mut BTreeMap<String, Candidate>,
    candidate: Candidate,
) -> Result<(), ConstantEvaluationError> {
    if candidates
        .insert(candidate.declared_value_id.clone(), candidate)
        .is_some()
    {
        return Err(ConstantEvaluationError::IdentityConflict);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
enum EvaluationState {
    Visiting,
    Complete,
    Failed(EvaluationFailure),
}

fn evaluate_candidate(
    identifier: &str,
    candidates: &BTreeMap<String, Candidate>,
    states: &mut BTreeMap<String, EvaluationState>,
    evaluated: &mut BTreeMap<String, Evaluation>,
) -> Result<Evaluation, EvaluationFailure> {
    match states.get(identifier) {
        Some(EvaluationState::Complete) => {
            return evaluated
                .get(identifier)
                .cloned()
                .ok_or(EvaluationFailure::Dependency);
        }
        Some(EvaluationState::Visiting | EvaluationState::Failed(_)) => {
            return Err(EvaluationFailure::Dependency);
        }
        None => {}
    }
    let candidate = candidates
        .get(identifier)
        .ok_or(EvaluationFailure::Dependency)?;
    states.insert(identifier.to_owned(), EvaluationState::Visiting);
    let result = (|| match &candidate.expression {
        Expression::EnumImplicit(previous) => {
            if let Some(previous) = previous {
                let dependency = evaluate_candidate(previous, candidates, states, evaluated)?;
                let dependency_levels = dependency.dependency_levels.saturating_add(1);
                enforce_constant_limit(
                    ConstantEvaluationLimit::DependencyLevels,
                    usize::try_from(dependency_levels).unwrap_or(usize::MAX),
                )
                .map_err(|error| EvaluationFailure::from_limit(&error))?;
                Ok(Evaluation {
                    value: checked_increment(&dependency.value, candidate.primitive)?,
                    dependency_ids: vec![previous.clone()],
                    dependency_levels,
                })
            } else {
                Ok(Evaluation {
                    value: zero(candidate.primitive)?,
                    dependency_ids: Vec::new(),
                    dependency_levels: 0,
                })
            }
        }
        Expression::Parsed(tokens) => {
            let mut parser = ExpressionParser {
                tokens,
                position: 0,
                syntax_nodes: 0,
                primitive: candidate.primitive,
                owner_id: &candidate.owner_id,
                candidates,
                states,
                evaluated,
                dependencies: BTreeSet::new(),
                dependency_levels: 0,
            };
            let value = parser.parse_expression(0)?;
            if parser.position != tokens.len() || !value_matches(&value, candidate.primitive) {
                return fail_state(states, identifier, EvaluationFailure::Expression);
            }
            Ok(Evaluation {
                value,
                dependency_ids: parser.dependencies.into_iter().collect(),
                dependency_levels: parser.dependency_levels,
            })
        }
    })();
    match result {
        Ok(value) => {
            states.insert(identifier.to_owned(), EvaluationState::Complete);
            evaluated.insert(identifier.to_owned(), value.clone());
            Ok(value)
        }
        Err(failure) => fail_state(states, identifier, failure),
    }
}

fn fail_state<T>(
    states: &mut BTreeMap<String, EvaluationState>,
    identifier: &str,
    failure: EvaluationFailure,
) -> Result<T, EvaluationFailure> {
    states.insert(identifier.to_owned(), EvaluationState::Failed(failure));
    Err(failure)
}

struct ExpressionParser<'a> {
    tokens: &'a [Token],
    position: usize,
    syntax_nodes: usize,
    primitive: PrimitiveType,
    owner_id: &'a str,
    candidates: &'a BTreeMap<String, Candidate>,
    states: &'a mut BTreeMap<String, EvaluationState>,
    evaluated: &'a mut BTreeMap<String, Evaluation>,
    dependencies: BTreeSet<String>,
    dependency_levels: u64,
}

impl ExpressionParser<'_> {
    fn parse_expression(
        &mut self,
        minimum_precedence: u8,
    ) -> Result<ConstantValue, EvaluationFailure> {
        self.bump_node()?;
        let mut left = self.parse_prefix()?;
        while let Some(Token::Operator(operator)) = self.tokens.get(self.position) {
            let Some(precedence) = binary_precedence(operator) else {
                break;
            };
            if precedence < minimum_precedence {
                break;
            }
            let operator = *operator;
            self.position = self.position.saturating_add(1);
            let right = self.parse_expression(precedence.saturating_add(1))?;
            left = apply_binary(operator, left, right, self.primitive)?;
        }
        Ok(left)
    }

    fn parse_prefix(&mut self) -> Result<ConstantValue, EvaluationFailure> {
        let token = self
            .tokens
            .get(self.position)
            .cloned()
            .ok_or(EvaluationFailure::Expression)?;
        self.position = self.position.saturating_add(1);
        match token {
            Token::Bool(value) if self.primitive == PrimitiveType::Bool => {
                Ok(ConstantValue::Bool(value))
            }
            Token::Integer(literal) => parse_integer(&literal, self.primitive, false),
            Token::Identifier(name) => self.resolve_dependency(&name),
            Token::Operator("!") => {
                self.bump_node()?;
                checked_not(self.parse_prefix()?, self.primitive)
            }
            Token::Operator("-") => {
                self.bump_node()?;
                if let Some(Token::Integer(literal)) = self.tokens.get(self.position) {
                    let literal = literal.clone();
                    self.position = self.position.saturating_add(1);
                    parse_integer(&literal, self.primitive, true)
                } else {
                    checked_negate(self.parse_prefix()?, self.primitive)
                }
            }
            Token::LeftParenthesis => {
                let value = self.parse_expression(0)?;
                if self.tokens.get(self.position) != Some(&Token::RightParenthesis) {
                    return Err(EvaluationFailure::Expression);
                }
                self.position = self.position.saturating_add(1);
                Ok(value)
            }
            Token::Bool(_) | Token::Operator(_) | Token::RightParenthesis => {
                Err(EvaluationFailure::Expression)
            }
        }
    }

    fn resolve_dependency(&mut self, name: &str) -> Result<ConstantValue, EvaluationFailure> {
        let mut matching = self
            .candidates
            .values()
            .filter(|candidate| {
                candidate.dependency_source
                    && candidate.owner_id == self.owner_id
                    && candidate.name == name
                    && candidate.primitive == self.primitive
            })
            .map(|candidate| candidate.declared_value_id.as_str());
        let identifier = matching.next().ok_or(EvaluationFailure::Dependency)?;
        if matching.next().is_some() {
            return Err(EvaluationFailure::Dependency);
        }
        let identifier = identifier.to_owned();
        if !self.dependencies.contains(&identifier) {
            enforce_constant_limit(
                ConstantEvaluationLimit::DirectDependencies,
                self.dependencies.len().saturating_add(1),
            )
            .map_err(|error| EvaluationFailure::from_limit(&error))?;
        }
        let value = evaluate_candidate(&identifier, self.candidates, self.states, self.evaluated)?;
        let dependency_levels = value.dependency_levels.saturating_add(1);
        enforce_constant_limit(
            ConstantEvaluationLimit::DependencyLevels,
            usize::try_from(dependency_levels).unwrap_or(usize::MAX),
        )
        .map_err(|error| EvaluationFailure::from_limit(&error))?;
        self.dependency_levels = self.dependency_levels.max(dependency_levels);
        self.dependencies.insert(identifier);
        Ok(value.value)
    }

    fn bump_node(&mut self) -> Result<(), EvaluationFailure> {
        self.syntax_nodes = self.syntax_nodes.saturating_add(1);
        enforce_constant_limit(
            ConstantEvaluationLimit::SyntaxNodesPerExpression,
            self.syntax_nodes,
        )
        .map_err(|error| EvaluationFailure::from_limit(&error))
    }
}

fn binary_precedence(operator: &str) -> Option<u8> {
    match operator {
        "||" => Some(1),
        "&&" => Some(2),
        "==" | "!=" | "<" | "<=" | ">" | ">=" => Some(3),
        "|" => Some(4),
        "^" => Some(5),
        "&" => Some(6),
        "+" | "-" => Some(7),
        "*" | "/" | "%" => Some(8),
        _ => None,
    }
}

fn tokenize(source: &str) -> Result<Vec<Token>, EvaluationFailure> {
    let characters = source.char_indices().collect::<Vec<_>>();
    let mut tokens = Vec::new();
    let mut position = 0;
    while position < characters.len() {
        let (start, character) = characters[position];
        if character.is_whitespace() {
            position += 1;
            continue;
        }
        if character == '(' {
            tokens.push(Token::LeftParenthesis);
            position += 1;
            continue;
        }
        if character == ')' {
            tokens.push(Token::RightParenthesis);
            position += 1;
            continue;
        }
        let tail = &source[start..];
        if let Some((operator, length)) = [
            ("&&", 2),
            ("||", 2),
            ("==", 2),
            ("!=", 2),
            ("<=", 2),
            (">=", 2),
        ]
        .into_iter()
        .find(|(operator, _)| tail.starts_with(operator))
        {
            tokens.push(Token::Operator(operator));
            position += length;
            continue;
        }
        if let Some(operator) = match character {
            '+' => Some("+"),
            '-' => Some("-"),
            '*' => Some("*"),
            '/' => Some("/"),
            '%' => Some("%"),
            '&' => Some("&"),
            '|' => Some("|"),
            '^' => Some("^"),
            '!' => Some("!"),
            '<' => Some("<"),
            '>' => Some(">"),
            _ => None,
        } {
            tokens.push(Token::Operator(operator));
            position += 1;
            continue;
        }
        if character.is_ascii_digit() {
            let end = token_end(&characters, position, |value| {
                value.is_ascii_alphanumeric() || value == '_'
            });
            tokens.push(Token::Integer(source[start..end].to_owned()));
            position = characters.partition_point(|(offset, _)| *offset < end);
            continue;
        }
        if character == '_' || character.is_alphabetic() {
            let end = token_end(&characters, position, |value| {
                value == '_' || value.is_alphanumeric()
            });
            let value = &source[start..end];
            tokens.push(match value {
                "true" => Token::Bool(true),
                "false" => Token::Bool(false),
                _ => Token::Identifier(value.to_owned()),
            });
            position = characters.partition_point(|(offset, _)| *offset < end);
            continue;
        }
        if character == ':' {
            return Err(EvaluationFailure::Dependency);
        }
        return Err(EvaluationFailure::Expression);
    }
    if tokens.is_empty() {
        return Err(EvaluationFailure::Expression);
    }
    if tokens
        .windows(2)
        .any(|pair| matches!(pair, [Token::Identifier(_), Token::LeftParenthesis]))
    {
        return Err(EvaluationFailure::Expression);
    }
    Ok(tokens)
}

fn token_end(
    characters: &[(usize, char)],
    start: usize,
    predicate: impl Fn(char) -> bool,
) -> usize {
    let mut position = start;
    while position < characters.len() && predicate(characters[position].1) {
        position += 1;
    }
    characters.get(position).map_or_else(
        || {
            characters
                .last()
                .map_or(0, |(offset, value)| offset + value.len_utf8())
        },
        |(offset, _)| *offset,
    )
}

fn parse_integer(
    literal: &str,
    primitive: PrimitiveType,
    negative: bool,
) -> Result<ConstantValue, EvaluationFailure> {
    let (radix, digits_start) = if literal.starts_with("0x") {
        (16, 2)
    } else if literal.starts_with("0o") {
        (8, 2)
    } else if literal.starts_with("0b") {
        (2, 2)
    } else {
        (10, 0)
    };
    let suffix_start = literal[digits_start..]
        .char_indices()
        .find_map(|(offset, character)| {
            (!character.is_digit(radix) && character != '_').then_some(digits_start + offset)
        })
        .unwrap_or(literal.len());
    let digits = literal[digits_start..suffix_start].replace('_', "");
    let suffix = &literal[suffix_start..];
    let expected_suffix = match primitive {
        PrimitiveType::Bool => return Err(EvaluationFailure::Expression),
        PrimitiveType::Signed(bits) => format!("i{bits}"),
        PrimitiveType::Unsigned(bits) => format!("u{bits}"),
    };
    if digits.is_empty() || (!suffix.is_empty() && suffix != expected_suffix) {
        return Err(EvaluationFailure::Expression);
    }
    let magnitude =
        u128::from_str_radix(&digits, radix).map_err(|_| EvaluationFailure::Expression)?;
    match primitive {
        PrimitiveType::Bool => Err(EvaluationFailure::Expression),
        PrimitiveType::Unsigned(_) => {
            if negative || magnitude > primitive.unsigned_maximum().unwrap_or(0) {
                Err(EvaluationFailure::Arithmetic)
            } else {
                Ok(ConstantValue::Unsigned(magnitude))
            }
        }
        PrimitiveType::Signed(_) => {
            let (minimum, maximum) = primitive.signed_bounds().unwrap_or((0, 0));
            if negative {
                let minimum_magnitude = minimum.unsigned_abs();
                if magnitude > minimum_magnitude {
                    return Err(EvaluationFailure::Arithmetic);
                }
                if magnitude == minimum_magnitude {
                    Ok(ConstantValue::Signed(minimum))
                } else {
                    let value =
                        i128::try_from(magnitude).map_err(|_| EvaluationFailure::Arithmetic)?;
                    Ok(ConstantValue::Signed(-value))
                }
            } else if magnitude > u128::try_from(maximum).unwrap_or(0) {
                Err(EvaluationFailure::Arithmetic)
            } else {
                i128::try_from(magnitude)
                    .map(ConstantValue::Signed)
                    .map_err(|_| EvaluationFailure::Arithmetic)
            }
        }
    }
}

fn apply_binary(
    operator: &str,
    left: ConstantValue,
    right: ConstantValue,
    primitive: PrimitiveType,
) -> Result<ConstantValue, EvaluationFailure> {
    match (left, right) {
        (ConstantValue::Bool(left), ConstantValue::Bool(right)) => match operator {
            "&&" => Ok(ConstantValue::Bool(left && right)),
            "||" => Ok(ConstantValue::Bool(left || right)),
            "==" => Ok(ConstantValue::Bool(left == right)),
            "!=" => Ok(ConstantValue::Bool(left != right)),
            "<" => Ok(ConstantValue::Bool(!left && right)),
            "<=" => Ok(ConstantValue::Bool(!left || right)),
            ">" => Ok(ConstantValue::Bool(left && !right)),
            ">=" => Ok(ConstantValue::Bool(left || !right)),
            _ => Err(EvaluationFailure::Expression),
        },
        (ConstantValue::Signed(left), ConstantValue::Signed(right)) => {
            let value = match operator {
                "+" => left.checked_add(right).map(ConstantValue::Signed),
                "-" => left.checked_sub(right).map(ConstantValue::Signed),
                "*" => left.checked_mul(right).map(ConstantValue::Signed),
                "/" => left.checked_div(right).map(ConstantValue::Signed),
                "%" => left.checked_rem(right).map(ConstantValue::Signed),
                "&" => Some(ConstantValue::Signed(left & right)),
                "|" => Some(ConstantValue::Signed(left | right)),
                "^" => Some(ConstantValue::Signed(left ^ right)),
                "==" => return Ok(ConstantValue::Bool(left == right)),
                "!=" => return Ok(ConstantValue::Bool(left != right)),
                "<" => return Ok(ConstantValue::Bool(left < right)),
                "<=" => return Ok(ConstantValue::Bool(left <= right)),
                ">" => return Ok(ConstantValue::Bool(left > right)),
                ">=" => return Ok(ConstantValue::Bool(left >= right)),
                _ => return Err(EvaluationFailure::Expression),
            }
            .ok_or(EvaluationFailure::Arithmetic)?;
            check_range(value, primitive)
        }
        (ConstantValue::Unsigned(left), ConstantValue::Unsigned(right)) => {
            let value = match operator {
                "+" => left.checked_add(right).map(ConstantValue::Unsigned),
                "-" => left.checked_sub(right).map(ConstantValue::Unsigned),
                "*" => left.checked_mul(right).map(ConstantValue::Unsigned),
                "/" => left.checked_div(right).map(ConstantValue::Unsigned),
                "%" => left.checked_rem(right).map(ConstantValue::Unsigned),
                "&" => Some(ConstantValue::Unsigned(left & right)),
                "|" => Some(ConstantValue::Unsigned(left | right)),
                "^" => Some(ConstantValue::Unsigned(left ^ right)),
                "==" => return Ok(ConstantValue::Bool(left == right)),
                "!=" => return Ok(ConstantValue::Bool(left != right)),
                "<" => return Ok(ConstantValue::Bool(left < right)),
                "<=" => return Ok(ConstantValue::Bool(left <= right)),
                ">" => return Ok(ConstantValue::Bool(left > right)),
                ">=" => return Ok(ConstantValue::Bool(left >= right)),
                _ => return Err(EvaluationFailure::Expression),
            }
            .ok_or(EvaluationFailure::Arithmetic)?;
            check_range(value, primitive)
        }
        _ => Err(EvaluationFailure::Expression),
    }
}

fn checked_not(
    value: ConstantValue,
    primitive: PrimitiveType,
) -> Result<ConstantValue, EvaluationFailure> {
    match value {
        ConstantValue::Bool(value) => Ok(ConstantValue::Bool(!value)),
        ConstantValue::Signed(value) => check_range(ConstantValue::Signed(!value), primitive),
        ConstantValue::Unsigned(value) => {
            let maximum = primitive
                .unsigned_maximum()
                .ok_or(EvaluationFailure::Expression)?;
            Ok(ConstantValue::Unsigned((!value) & maximum))
        }
    }
}

fn checked_negate(
    value: ConstantValue,
    primitive: PrimitiveType,
) -> Result<ConstantValue, EvaluationFailure> {
    match value {
        ConstantValue::Signed(value) => value
            .checked_neg()
            .map(ConstantValue::Signed)
            .ok_or(EvaluationFailure::Arithmetic)
            .and_then(|value| check_range(value, primitive)),
        ConstantValue::Bool(_) | ConstantValue::Unsigned(_) => Err(EvaluationFailure::Expression),
    }
}

fn checked_increment(
    value: &ConstantValue,
    primitive: PrimitiveType,
) -> Result<ConstantValue, EvaluationFailure> {
    match value {
        ConstantValue::Signed(value) => value
            .checked_add(1)
            .map(ConstantValue::Signed)
            .ok_or(EvaluationFailure::Arithmetic)
            .and_then(|value| check_range(value, primitive)),
        ConstantValue::Unsigned(value) => value
            .checked_add(1)
            .map(ConstantValue::Unsigned)
            .ok_or(EvaluationFailure::Arithmetic)
            .and_then(|value| check_range(value, primitive)),
        ConstantValue::Bool(_) => Err(EvaluationFailure::Expression),
    }
}

fn zero(primitive: PrimitiveType) -> Result<ConstantValue, EvaluationFailure> {
    match primitive {
        PrimitiveType::Signed(_) => Ok(ConstantValue::Signed(0)),
        PrimitiveType::Unsigned(_) => Ok(ConstantValue::Unsigned(0)),
        PrimitiveType::Bool => Err(EvaluationFailure::Expression),
    }
}

fn check_range(
    value: ConstantValue,
    primitive: PrimitiveType,
) -> Result<ConstantValue, EvaluationFailure> {
    let valid = match (&value, primitive) {
        (ConstantValue::Bool(_), PrimitiveType::Bool) => true,
        (ConstantValue::Signed(value), PrimitiveType::Signed(_)) => primitive
            .signed_bounds()
            .is_some_and(|(minimum, maximum)| *value >= minimum && *value <= maximum),
        (ConstantValue::Unsigned(value), PrimitiveType::Unsigned(_)) => primitive
            .unsigned_maximum()
            .is_some_and(|maximum| *value <= maximum),
        _ => false,
    };
    valid.then_some(value).ok_or(EvaluationFailure::Arithmetic)
}

fn value_matches(value: &ConstantValue, primitive: PrimitiveType) -> bool {
    matches!(
        (value, primitive),
        (ConstantValue::Bool(_), PrimitiveType::Bool)
            | (ConstantValue::Signed(_), PrimitiveType::Signed(_))
            | (ConstantValue::Unsigned(_), PrimitiveType::Unsigned(_))
    )
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn build_graph(
    repository_identity: &str,
    candidates: &BTreeMap<String, Candidate>,
    evaluated: &BTreeMap<String, Evaluation>,
    unsupported: &[UnsupportedSubject],
    catalog: &ExtractionCatalog<'_>,
    failed_enums: &BTreeSet<String>,
    enum_evidence: &BTreeMap<String, (String, Vec<String>)>,
) -> Result<ConstantEvaluationGraph, ConstantEvaluationError> {
    let mut entities = Vec::new();
    let mut relationships = Vec::new();
    let mut claims = Vec::new();
    let mut derivations = Vec::new();
    let mut evaluated_entity_by_declared = BTreeMap::new();
    for (identifier, evaluation) in evaluated {
        let candidate = candidates
            .get(identifier)
            .ok_or(ConstantEvaluationError::DependencyInvalid)?;
        let entity = EvaluatedValue::new(
            repository_identity,
            candidate.declared_value_id.clone(),
            candidate.primitive.kind(),
            evaluation.value.canonical(),
            candidate.rust_type.clone(),
            candidate.type_authority,
            candidate.evidence_ids.clone(),
        );
        evaluated_entity_by_declared.insert(identifier.as_str(), entity.id.clone());
        entities.push(entity);
    }
    entities.sort_by(|left, right| left.id.cmp(&right.id));
    for entity in &entities {
        let candidate = candidates
            .get(&entity.declared_value_id)
            .ok_or(ConstantEvaluationError::DependencyInvalid)?;
        let evaluation = evaluated
            .get(&entity.declared_value_id)
            .ok_or(ConstantEvaluationError::DependencyInvalid)?;
        let relationship = ConstantEvaluationRelationship::new(
            entity.declared_value_id.clone(),
            entity.id.clone(),
            entity.evidence_ids.clone(),
        );
        let mut input_claim_ids = vec![candidate.declared_value_claim_id.clone()];
        let mut dependency_entity_ids = Vec::new();
        for dependency in &evaluation.dependency_ids {
            let entity_id = evaluated_entity_by_declared
                .get(dependency.as_str())
                .cloned()
                .ok_or(ConstantEvaluationError::DependencyInvalid)?;
            input_claim_ids.push(codenoesis_domain::s4::workspace_claim_id(
                ClaimSubjectKind::Entity,
                &entity_id,
                codenoesis_domain::knowledge::ClaimState::DerivedFact,
            ));
            dependency_entity_ids.push(entity_id);
        }
        claims.push(constant_evaluation_claim(
            ClaimSubjectKind::Entity,
            entity.id.clone(),
            entity.evidence_ids.clone(),
        ));
        claims.push(constant_evaluation_claim(
            ClaimSubjectKind::Relationship,
            relationship.id.clone(),
            relationship.evidence_ids.clone(),
        ));
        derivations.push(ConstantEvaluationDerivation::new(
            entity.id.clone(),
            relationship.id.clone(),
            input_claim_ids,
            entity.evidence_ids.clone(),
            dependency_entity_ids,
        ));
        relationships.push(relationship);
    }
    relationships.sort_by(|left, right| left.id.cmp(&right.id));
    claims.sort_by(|left, right| left.id.cmp(&right.id));
    let mut coverage = unsupported
        .iter()
        .map(|subject| {
            ConstantEvaluationCoverageGap::not_evaluated(
                subject.capability,
                subject.subject_id.clone(),
                subject.evidence_ids.clone(),
            )
        })
        .collect::<Vec<_>>();
    coverage.sort_by(|left, right| left.id.cmp(&right.id));
    coverage.dedup_by(|left, right| left.id == right.id);
    let successful = evaluated
        .iter()
        .filter_map(|(identifier, evaluation)| {
            candidates
                .get(identifier)
                .map(|candidate| (candidate, evaluation))
        })
        .filter(|(candidate, evaluation)| {
            candidate.explicit_value
                && candidate.primitive != PrimitiveType::Bool
                && evaluation.dependency_ids.is_empty()
        })
        .map(|(candidate, _)| candidate)
        .collect::<Vec<_>>();
    let mut removed_coverage_ids = catalog
        .semantic_coverage
        .iter()
        .filter(|gap| {
            gap.capability == "rust.value_not_evaluated"
                && successful
                    .iter()
                    .any(|candidate| gap.evidence_ids == candidate.declaration_evidence_ids)
        })
        .map(|gap| gap.id.clone())
        .collect::<Vec<_>>();
    removed_coverage_ids.extend(
        catalog
            .semantic_coverage
            .iter()
            .filter(|gap| {
                gap.capability == "rust.value_not_evaluated"
                    && unsupported.iter().any(|subject| {
                        !subject.legacy_evidence_ids.is_empty()
                            && gap.evidence_ids == subject.legacy_evidence_ids
                    })
            })
            .map(|gap| gap.id.clone()),
    );
    let completed_enum_repr_evidence = enum_evidence
        .keys()
        .filter(|identifier| !failed_enums.contains(*identifier))
        .filter_map(|identifier| {
            candidates
                .values()
                .find(|candidate| candidate.enum_id.as_ref() == Some(identifier))
                .and_then(|candidate| candidate.repr_evidence_id.clone())
        })
        .collect::<BTreeSet<_>>();
    removed_coverage_ids.extend(
        catalog
            .semantic_coverage
            .iter()
            .filter(|gap| {
                gap.capability == "rust.attribute_semantics_not_interpreted"
                    && gap
                        .evidence_ids
                        .iter()
                        .any(|identifier| completed_enum_repr_evidence.contains(identifier))
            })
            .map(|gap| gap.id.clone()),
    );
    removed_coverage_ids.sort();
    removed_coverage_ids.dedup();
    let mut removed_diagnostic_ids = catalog
        .semantic_diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic.code == "rust.attribute_semantics_not_interpreted"
                && diagnostic
                    .evidence_ids
                    .iter()
                    .any(|identifier| completed_enum_repr_evidence.contains(identifier))
        })
        .map(|diagnostic| diagnostic.id.clone())
        .collect::<Vec<_>>();
    removed_diagnostic_ids.sort();
    let index = ConstantEvaluationIndex::from_graph(&entities, &relationships, derivations);
    Ok(ConstantEvaluationGraph {
        entities,
        relationships,
        claims,
        coverage,
        removed_coverage_ids,
        removed_diagnostic_ids,
        index,
    })
}

fn source_overlays(
    contexts: &[crate::semantic_depth::SourceContext<'_>],
    graph: &ConstantEvaluationGraph,
    catalog: &ExtractionCatalog<'_>,
) -> Vec<ConstantEvaluationSourceOverlay> {
    let mut values = contexts
        .iter()
        .map(|context| {
            let evidence_in_source = |identifiers: &[String]| {
                identifiers.iter().any(|identifier| {
                    catalog
                        .evidence
                        .get(identifier.as_str())
                        .is_some_and(|evidence| evidence.path == context.path)
                })
            };
            ConstantEvaluationSourceOverlay {
                source_file_id: context.source_file_id.clone(),
                entities: graph
                    .entities
                    .iter()
                    .filter(|value| evidence_in_source(&value.evidence_ids))
                    .cloned()
                    .collect(),
                relationships: graph
                    .relationships
                    .iter()
                    .filter(|value| evidence_in_source(&value.evidence_ids))
                    .cloned()
                    .collect(),
                claims: graph
                    .claims
                    .iter()
                    .filter(|value| evidence_in_source(&value.evidence_ids))
                    .cloned()
                    .collect(),
                coverage: graph
                    .coverage
                    .iter()
                    .filter(|value| evidence_in_source(&value.evidence_ids))
                    .cloned()
                    .collect(),
                removed_coverage_ids: graph
                    .removed_coverage_ids
                    .iter()
                    .filter(|identifier| {
                        catalog.semantic_coverage.iter().any(|gap| {
                            gap.id == **identifier && evidence_in_source(&gap.evidence_ids)
                        })
                    })
                    .cloned()
                    .collect(),
                removed_diagnostic_ids: graph
                    .removed_diagnostic_ids
                    .iter()
                    .filter(|identifier| {
                        catalog.semantic_diagnostics.iter().any(|diagnostic| {
                            diagnostic.id == **identifier
                                && evidence_in_source(&diagnostic.evidence_ids)
                        })
                    })
                    .cloned()
                    .collect(),
            }
        })
        .collect::<Vec<_>>();
    values.sort_by(|left, right| left.source_file_id.cmp(&right.source_file_id));
    values
}

fn declared_id_for_node<'a>(
    catalog: &'a ExtractionCatalog<'a>,
    path: &'a str,
    node: Node<'_>,
) -> Option<&'a str> {
    catalog
        .declared_by_span
        .get(&(
            path,
            u64::try_from(node.start_byte()).ok()?,
            u64::try_from(node.end_byte()).ok()?,
        ))
        .copied()
}

fn declared_id_for_variant<'a>(
    catalog: &'a ExtractionCatalog<'a>,
    path: &'a str,
    variant: Node<'_>,
) -> Option<&'a str> {
    variant
        .child_by_field_name("value")
        .and_then(|value| declared_id_for_node(catalog, path, value))
        .or_else(|| declared_id_for_node(catalog, path, variant))
}

fn evidence_for_span<'a>(
    catalog: &'a ExtractionCatalog<'a>,
    path: &str,
    start: usize,
    end: usize,
) -> Option<&'a WorkspaceEvidence> {
    catalog.evidence.values().copied().find(|evidence| {
        evidence.path == path
            && usize::try_from(evidence.start_byte).ok() == Some(start)
            && usize::try_from(evidence.end_byte).ok() == Some(end)
    })
}

fn direct_repr(
    node: Node<'_>,
    source: &str,
    path: &str,
    catalog: &ExtractionCatalog<'_>,
) -> Option<(String, String)> {
    let mut values = Vec::new();
    let mut attributes = Vec::new();
    let mut sibling = node.prev_named_sibling();
    while let Some(value) = sibling {
        if value.kind() != "attribute_item" {
            break;
        }
        attributes.push(value);
        sibling = value.prev_named_sibling();
    }
    attributes.reverse();
    if attributes.len() != 1 {
        return None;
    }
    for child in attributes {
        if child.kind() != "attribute_item" {
            continue;
        }
        let compact = node_text(child, source)
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        let Some(value) = compact
            .strip_prefix("#[repr(")
            .and_then(|value| value.strip_suffix(")]"))
        else {
            continue;
        };
        if PrimitiveType::parse(value).is_none() || matches!(value, "bool" | "usize" | "isize") {
            continue;
        }
        let evidence = evidence_for_span(catalog, path, child.start_byte(), child.end_byte())?;
        values.push((value.to_owned(), evidence.id.clone()));
    }
    (values.len() == 1).then(|| values.remove(0))
}

fn node_text<'a>(node: Node<'_>, source: &'a str) -> &'a str {
    &source[node.start_byte()..node.end_byte()]
}
