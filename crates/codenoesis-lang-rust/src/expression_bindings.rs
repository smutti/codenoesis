use std::collections::{BTreeMap, BTreeSet};

use codenoesis_domain::RepositoryInventory;
use codenoesis_domain::knowledge::ClaimSubjectKind;
use codenoesis_domain::s4::{WorkspaceEvidence, workspace_evidence_id};
use codenoesis_domain::s4_k1::{
    CallableSemanticEntity, CallableSemanticEntityKind, CallableSemanticProperties, k1_digest,
};
use codenoesis_domain::s4_r14::{
    BindingModifier, BindingOrigin, CallArgumentProperties, ExpressionBindingEntity,
    ExpressionBindingError, ExpressionBindingExtraction, ExpressionBindingGraph,
    ExpressionBindingIndex, ExpressionBindingLimit, ExpressionBindingRelationship,
    ExpressionBindingSourceChunk, ExpressionCoverageGap, ExpressionEntityKind,
    ExpressionEntityProperties, ExpressionLocator, ExpressionProperties,
    ExpressionRelationshipKind, ExpressionRole, MAX_R14_EXPRESSION_DEPTH, PatternBindingProperties,
    SELECTED_EXPRESSION_KINDS, call_argument_entity_id, enforce_expression_limit, expression_claim,
    expression_entity_id, pattern_binding_entity_id,
};
use codenoesis_ports::RustExpressionBindingsExtractor;
use tree_sitter::Node;
use unicode_normalization::UnicodeNormalization as _;

use crate::TreeSitterRustWorkspaceExtractor;
use crate::semantic_depth::{parse_tree, source_contexts, source_text};

const PATTERN_CONTAINERS: &[&str] = &[
    "captured_pattern",
    "generic_pattern",
    "match_pattern",
    "mut_pattern",
    "or_pattern",
    "range_pattern",
    "ref_pattern",
    "reference_pattern",
    "remaining_field_pattern",
    "slice_pattern",
    "struct_pattern",
    "tuple_pattern",
    "tuple_struct_pattern",
];

#[derive(Clone)]
struct ExistingEntity {
    id: String,
    kind: CallableSemanticEntityKind,
    subject_id: String,
    evidence: Vec<WorkspaceEvidence>,
    control_kind: Option<&'static str>,
}

impl ExistingEntity {
    fn from_callable(
        entity: &CallableSemanticEntity,
        evidence: &BTreeMap<&str, &WorkspaceEvidence>,
    ) -> Result<Self, ExpressionBindingError> {
        let control_kind = match &entity.properties {
            CallableSemanticProperties::Control(properties) => {
                Some(properties.control_kind.as_str())
            }
            _ => None,
        };
        let evidence = entity
            .evidence_ids
            .iter()
            .map(|identifier| {
                evidence
                    .get(identifier.as_str())
                    .copied()
                    .cloned()
                    .ok_or(ExpressionBindingError::ContractInvalid)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            id: entity.id.clone(),
            kind: entity.kind,
            subject_id: entity.subject_id.clone(),
            evidence,
            control_kind,
        })
    }

    fn has_span(&self, path: &str, start: usize, end: usize) -> bool {
        self.evidence.iter().any(|value| {
            value.path == path
                && usize::try_from(value.start_byte).ok() == Some(start)
                && usize::try_from(value.end_byte).ok() == Some(end)
        })
    }
}

#[derive(Clone, Debug)]
struct ExpressionDraft {
    entity: ExpressionBindingEntity,
    start: usize,
    end: usize,
    parent_range: Option<(usize, usize, String)>,
    roles: BTreeSet<ExpressionRole>,
}

#[derive(Clone, Debug)]
struct BindingDraft {
    entity: ExpressionBindingEntity,
    owner_id: String,
    scope_start: usize,
    scope_end: usize,
    source_expression_range: Option<(usize, usize, String)>,
}

struct SourceBuilder<'a> {
    repository_identity: &'a str,
    commit_oid: &'a str,
    crate_id: &'a str,
    source_file_id: &'a str,
    path: &'a str,
    blob_oid: &'a str,
    source: &'a str,
    existing: &'a [ExistingEntity],
    expressions: Vec<ExpressionDraft>,
    arguments: Vec<ExpressionBindingEntity>,
    bindings: Vec<BindingDraft>,
    relationships: BTreeMap<String, ExpressionBindingRelationship>,
    evidence: BTreeMap<String, WorkspaceEvidence>,
    coverage: BTreeMap<String, ExpressionCoverageGap>,
}

impl<'a> SourceBuilder<'a> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        repository_identity: &'a str,
        commit_oid: &'a str,
        crate_id: &'a str,
        source_file_id: &'a str,
        path: &'a str,
        blob_oid: &'a str,
        source: &'a str,
        existing: &'a [ExistingEntity],
    ) -> Self {
        Self {
            repository_identity,
            commit_oid,
            crate_id,
            source_file_id,
            path,
            blob_oid,
            source,
            existing,
            expressions: Vec::new(),
            arguments: Vec::new(),
            bindings: Vec::new(),
            relationships: BTreeMap::new(),
            evidence: BTreeMap::new(),
            coverage: BTreeMap::new(),
        }
    }

    fn add_evidence(&mut self, node: Node<'_>) -> Result<String, ExpressionBindingError> {
        if node.start_byte() >= node.end_byte() || node.end_byte() > self.source.len() {
            return Err(invalid_syntax(self.path, node.start_byte(), node.kind()));
        }
        let start_byte = u64::try_from(node.start_byte())
            .map_err(|_| ExpressionBindingError::ContractInvalid)?;
        let end_byte =
            u64::try_from(node.end_byte()).map_err(|_| ExpressionBindingError::ContractInvalid)?;
        let id = workspace_evidence_id(
            self.repository_identity,
            self.commit_oid,
            self.blob_oid,
            self.path,
            start_byte,
            end_byte,
        );
        self.evidence
            .entry(id.clone())
            .or_insert_with(|| WorkspaceEvidence {
                id: id.clone(),
                path: self.path.to_owned(),
                blob_oid: self.blob_oid.to_owned(),
                start_byte,
                end_byte,
            });
        Ok(id)
    }

    fn add_relationship(&mut self, relationship: ExpressionBindingRelationship) {
        self.relationships
            .entry(relationship.id.clone())
            .and_modify(|existing| {
                existing
                    .evidence_ids
                    .extend(relationship.evidence_ids.clone());
                existing.evidence_ids.sort();
                existing.evidence_ids.dedup();
            })
            .or_insert(relationship);
    }

    fn add_gap(
        &mut self,
        capability: &str,
        subject_id: &str,
        node: Node<'_>,
    ) -> Result<(), ExpressionBindingError> {
        let evidence_id = self.add_evidence(node)?;
        self.add_gap_with_evidence(capability, subject_id, evidence_id);
        Ok(())
    }

    fn add_gap_with_evidence(&mut self, capability: &str, subject_id: &str, evidence_id: String) {
        let gap = ExpressionCoverageGap::unsupported(
            capability,
            subject_id.to_owned(),
            vec![evidence_id],
        );
        self.coverage.entry(gap.id.clone()).or_insert(gap);
    }

    #[allow(clippy::too_many_lines)]
    fn finish(mut self) -> Result<ExpressionBindingSourceChunk, ExpressionBindingError> {
        self.expressions
            .sort_by(|left, right| left.entity.id.cmp(&right.entity.id));
        self.arguments.sort_by(|left, right| left.id.cmp(&right.id));
        self.bindings
            .sort_by(|left, right| left.entity.id.cmp(&right.entity.id));
        if !unique(
            self.expressions
                .iter()
                .map(|value| value.entity.id.as_str()),
        ) || !unique(self.arguments.iter().map(|value| value.id.as_str()))
            || !unique(self.bindings.iter().map(|value| value.entity.id.as_str()))
        {
            return Err(ExpressionBindingError::IdentityConflict);
        }

        let mut pending = Vec::new();
        for expression in &self.expressions {
            pending.push(ExpressionBindingRelationship::new(
                ExpressionRelationshipKind::HasExpression,
                expression.entity.callable_id.clone(),
                expression.entity.id.clone(),
                vec![expression.entity.evidence_id.clone()],
            ));
            if let Some((start, end, kind)) = &expression.parent_range {
                let parent = self
                    .expressions
                    .iter()
                    .find(|candidate| {
                        candidate.start == *start
                            && candidate.end == *end
                            && expression_syntax_kind(candidate) == kind
                            && candidate.entity.callable_id == expression.entity.callable_id
                    })
                    .ok_or(ExpressionBindingError::ParentInvalid)?;
                pending.push(ExpressionBindingRelationship::new(
                    ExpressionRelationshipKind::ContainsExpression,
                    parent.entity.id.clone(),
                    expression.entity.id.clone(),
                    vec![expression.entity.evidence_id.clone()],
                ));
            }
        }
        for binding in &self.bindings {
            pending.push(ExpressionBindingRelationship::new(
                ExpressionRelationshipKind::DeclaresBinding,
                binding.owner_id.clone(),
                binding.entity.id.clone(),
                vec![binding.entity.evidence_id.clone()],
            ));
            if let Some((start, end, kind)) = &binding.source_expression_range {
                let expression = self
                    .expressions
                    .iter()
                    .find(|candidate| {
                        candidate.start == *start
                            && candidate.end == *end
                            && expression_syntax_kind(candidate) == kind
                            && candidate.entity.callable_id == binding.entity.callable_id
                    })
                    .ok_or(ExpressionBindingError::AccessResolutionInvalid)?;
                pending.push(ExpressionBindingRelationship::new(
                    ExpressionRelationshipKind::BindsFrom,
                    binding.entity.id.clone(),
                    expression.entity.id.clone(),
                    vec![
                        binding.entity.evidence_id.clone(),
                        expression.entity.evidence_id.clone(),
                    ],
                ));
            }
        }
        for relationship in pending {
            self.add_relationship(relationship);
        }
        self.add_access_relationships()?;

        let mut entities = self
            .expressions
            .into_iter()
            .map(|value| value.entity)
            .chain(self.arguments)
            .chain(self.bindings.into_iter().map(|value| value.entity))
            .collect::<Vec<_>>();
        entities.sort_by(|left, right| left.id.cmp(&right.id));
        if !unique(entities.iter().map(|value| value.id.as_str())) {
            return Err(ExpressionBindingError::IdentityConflict);
        }
        let relationships = self.relationships.into_values().collect::<Vec<_>>();
        let mut claims = entities
            .iter()
            .map(|entity| {
                expression_claim(
                    ClaimSubjectKind::Entity,
                    entity.id.clone(),
                    vec![entity.evidence_id.clone()],
                )
            })
            .chain(relationships.iter().map(|relationship| {
                expression_claim(
                    ClaimSubjectKind::Relationship,
                    relationship.id.clone(),
                    relationship.evidence_ids.clone(),
                )
            }))
            .collect::<Vec<_>>();
        claims.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(ExpressionBindingSourceChunk {
            crate_id: self.crate_id.to_owned(),
            source_file_id: self.source_file_id.to_owned(),
            path: self.path.to_owned(),
            entities,
            relationships,
            claims,
            evidence: self.evidence.into_values().collect(),
            coverage: self.coverage.into_values().collect(),
        })
    }

    fn add_access_relationships(&mut self) -> Result<(), ExpressionBindingError> {
        let mut coverage = Vec::new();
        let accesses = self
            .expressions
            .iter()
            .filter(|value| matches!(expression_syntax_kind(value), "identifier" | "self"))
            .map(|expression| {
                let ExpressionEntityProperties::Expression(properties) =
                    &expression.entity.properties
                else {
                    return Err(ExpressionBindingError::ContractInvalid);
                };
                let name = properties
                    .token
                    .as_deref()
                    .ok_or(ExpressionBindingError::ContractInvalid)?;
                let mut candidates = self
                    .bindings
                    .iter()
                    .filter(|binding| {
                        binding.entity.callable_id == expression.entity.callable_id
                            && binding.entity.name == name
                            && expression.start >= binding.scope_start
                            && expression.end <= binding.scope_end
                            && expression.entity.locator.path == binding.entity.locator.path
                    })
                    .collect::<Vec<_>>();
                candidates.sort_by_key(|binding| {
                    (
                        binding.scope_end.saturating_sub(binding.scope_start),
                        std::cmp::Reverse(binding.scope_start),
                        binding.entity.id.as_str(),
                    )
                });
                let Some(binding) = candidates.first() else {
                    return Ok(Vec::new());
                };
                if candidates.get(1).is_some_and(|other| {
                    other.scope_start == binding.scope_start
                        && other.scope_end == binding.scope_end
                        && other.entity.name == binding.entity.name
                }) {
                    coverage.push(ExpressionCoverageGap::unsupported(
                        "rust.lexical_binding_shadowing",
                        expression.entity.id.clone(),
                        vec![expression.entity.evidence_id.clone()],
                    ));
                    return Ok(Vec::new());
                }
                let target = properties.roles.contains(&ExpressionRole::AssignmentTarget);
                let compound = expression
                    .parent_range
                    .as_ref()
                    .is_some_and(|(_, _, kind)| kind == "compound_assignment_expr");
                let mut relationships = Vec::new();
                if !target || compound {
                    relationships.push(ExpressionBindingRelationship::new(
                        ExpressionRelationshipKind::Reads,
                        expression.entity.id.clone(),
                        binding.entity.id.clone(),
                        vec![expression.entity.evidence_id.clone()],
                    ));
                }
                if target {
                    relationships.push(ExpressionBindingRelationship::new(
                        ExpressionRelationshipKind::Writes,
                        expression.entity.id.clone(),
                        binding.entity.id.clone(),
                        vec![expression.entity.evidence_id.clone()],
                    ));
                }
                Ok(relationships)
            })
            .collect::<Result<Vec<_>, _>>()?;
        for relationship in accesses.into_iter().flatten() {
            self.add_relationship(relationship);
        }
        for gap in coverage {
            self.coverage.entry(gap.id.clone()).or_insert(gap);
        }
        Ok(())
    }
}

impl TreeSitterRustWorkspaceExtractor {
    /// Extracts the exact R14 source-only expression and lexical-binding profile.
    ///
    /// # Errors
    ///
    /// Returns an inherited K1 or typed expression, identity, scope, or limit failure.
    pub fn extract_rust_expression_bindings(
        &self,
        inventory: &RepositoryInventory,
    ) -> Result<ExpressionBindingExtraction, ExpressionBindingError> {
        let callable = self
            .extract_rust_callable_semantics(inventory)
            .map_err(ExpressionBindingError::Source)?;
        let evidence = callable
            .knowledge
            .graph
            .evidence
            .iter()
            .map(|value| (value.id.as_str(), value))
            .collect::<BTreeMap<_, _>>();
        let existing = callable
            .knowledge
            .graph
            .entities
            .iter()
            .map(|entity| ExistingEntity::from_callable(entity, &evidence))
            .collect::<Result<Vec<_>, _>>()?;
        let contexts = source_contexts(&callable.knowledge.framework.semantic.manifest, inventory)
            .map_err(|_| ExpressionBindingError::ContractInvalid)?;
        let repository_identity = inventory.bound_revision().repository_identity().as_str();
        let commit_oid = inventory.bound_revision().commit_oid().as_str();
        let mut chunks = Vec::with_capacity(contexts.len());
        for context in contexts {
            let source = source_text(&context)
                .map_err(|_| invalid_syntax(&context.path, 0, "source_file"))?;
            let tree = parse_tree(&context.path, source)
                .map_err(|_| invalid_syntax(&context.path, 0, "source_file"))?;
            let mut builder = SourceBuilder::new(
                repository_identity,
                commit_oid,
                &context.crate_id,
                &context.source_file_id,
                &context.path,
                context.file.blob_oid().as_str(),
                source,
                &existing,
            );
            visit_declarations(tree.root_node(), &mut builder)?;
            chunks.push(builder.finish()?);
        }
        let graph = aggregate_graph(&chunks)?;
        let parser_invocation_count = callable
            .parser_invocation_count
            .saturating_add(u64::try_from(chunks.len()).unwrap_or(u64::MAX));
        let extraction =
            ExpressionBindingExtraction::from_k1(callable, chunks, graph, parser_invocation_count);
        extraction.knowledge.validate()?;
        Ok(extraction)
    }
}

impl RustExpressionBindingsExtractor for TreeSitterRustWorkspaceExtractor {
    fn extract_rust_expression_bindings(
        &self,
        inventory: &RepositoryInventory,
    ) -> Result<ExpressionBindingExtraction, ExpressionBindingError> {
        TreeSitterRustWorkspaceExtractor::extract_rust_expression_bindings(self, inventory)
    }
}

fn visit_declarations(
    node: Node<'_>,
    builder: &mut SourceBuilder<'_>,
) -> Result<(), ExpressionBindingError> {
    if matches!(
        node.kind(),
        "macro_invocation" | "macro_definition" | "closure_expression"
    ) {
        return Ok(());
    }
    if matches!(node.kind(), "function_item" | "function_signature_item") {
        process_callable(node, builder)?;
        return Ok(());
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        visit_declarations(child, builder)?;
    }
    Ok(())
}

fn process_callable(
    node: Node<'_>,
    builder: &mut SourceBuilder<'_>,
) -> Result<(), ExpressionBindingError> {
    let body = node.child_by_field_name("body");
    let header_end = body.map_or(node.end_byte(), |value| value.start_byte());
    let callable_id = builder
        .existing
        .iter()
        .find(|entity| {
            entity.kind == CallableSemanticEntityKind::Signature
                && entity.has_span(builder.path, node.start_byte(), header_end)
        })
        .map(|entity| entity.subject_id.clone())
        .ok_or(ExpressionBindingError::CallSiteEvidenceMismatch)?;
    let mut bindings = Vec::new();
    if let Some(parameters) = node.child_by_field_name("parameters") {
        let mut cursor = parameters.walk();
        for parameter in parameters
            .named_children(&mut cursor)
            .filter(|value| matches!(value.kind(), "parameter" | "self_parameter"))
        {
            let owner_id = builder
                .existing
                .iter()
                .find(|entity| {
                    entity.kind == CallableSemanticEntityKind::Parameter
                        && entity.subject_id == callable_id
                        && entity.has_span(
                            builder.path,
                            parameter.start_byte(),
                            parameter.end_byte(),
                        )
                })
                .map(|entity| entity.id.clone())
                .ok_or(ExpressionBindingError::CallSiteEvidenceMismatch)?;
            let modifier = if node_text(parameter, builder.source)
                .trim_start()
                .starts_with("mut ")
            {
                BindingModifier::ExplicitMut
            } else {
                BindingModifier::None
            };
            let scope = body.map_or((node.end_byte(), node.end_byte()), |value| {
                (value.start_byte(), value.end_byte())
            });
            let pattern = if parameter.kind() == "self_parameter" {
                find_named_kind(parameter, "self")
                    .ok_or(ExpressionBindingError::PatternUnsupported)?
            } else {
                parameter
                    .child_by_field_name("pattern")
                    .or_else(|| parameter.named_child(0))
                    .ok_or(ExpressionBindingError::PatternUnsupported)?
            };
            collect_pattern_bindings(
                pattern,
                &callable_id,
                &owner_id,
                BindingOrigin::Parameter,
                modifier,
                scope,
                None,
                builder,
                &mut bindings,
            )?;
        }
    }
    if let Some(body) = body {
        let mut expressions = Vec::new();
        collect_expressions(body, &callable_id, builder, &mut expressions)?;
        assign_roles(body, &mut expressions)?;
        add_call_facts(body, &callable_id, &expressions, builder)?;
        collect_body_bindings(body, &callable_id, &expressions, builder, &mut bindings)?;
        enforce_expression_limit(
            ExpressionBindingLimit::ExpressionsPerCallable,
            expressions.len(),
        )?;
        builder.expressions.extend(expressions);
    }
    enforce_expression_limit(ExpressionBindingLimit::BindingsPerCallable, bindings.len())?;
    builder.bindings.extend(bindings);
    Ok(())
}

fn collect_expressions(
    node: Node<'_>,
    callable_id: &str,
    builder: &mut SourceBuilder<'_>,
    output: &mut Vec<ExpressionDraft>,
) -> Result<(), ExpressionBindingError> {
    let unsupported = match node.kind() {
        "macro_invocation" | "macro_definition" => Some("rust.expression_macro_expansion"),
        "closure_expression" => Some("rust.expression_closure_capture"),
        "function_item" | "function_signature_item" => Some("rust.expression_nested_callable"),
        _ => None,
    };
    if let Some(capability) = unsupported {
        builder.add_gap(capability, callable_id, node)?;
        return Ok(());
    }
    if selected_expression(node) && !excluded_identifier_context(node) {
        let evidence_id = builder.add_evidence(node)?;
        let start_byte = u64::try_from(node.start_byte())
            .map_err(|_| ExpressionBindingError::ContractInvalid)?;
        let end_byte =
            u64::try_from(node.end_byte()).map_err(|_| ExpressionBindingError::ContractInvalid)?;
        let syntax_kind = node.kind().to_owned();
        let token = matches!(node.kind(), "identifier" | "self" | "scoped_identifier")
            .then(|| normalize(node_text(node, builder.source)));
        let operator = expression_operator(node, builder.source);
        let source_bytes = &builder.source.as_bytes()[node.start_byte()..node.end_byte()];
        output.push(ExpressionDraft {
            entity: ExpressionBindingEntity {
                id: expression_entity_id(
                    builder.repository_identity,
                    callable_id,
                    builder.source_file_id,
                    start_byte,
                    end_byte,
                    &syntax_kind,
                ),
                kind: ExpressionEntityKind::Expression,
                name: token.clone().unwrap_or_else(|| syntax_kind.clone()),
                callable_id: callable_id.to_owned(),
                source_file_id: builder.source_file_id.to_owned(),
                evidence_id,
                locator: locator(builder, node)?,
                properties: ExpressionEntityProperties::Expression(ExpressionProperties {
                    syntax_kind,
                    token,
                    operator,
                    source_digest: k1_digest(source_bytes),
                    source_byte_length: u64::try_from(source_bytes.len())
                        .map_err(|_| ExpressionBindingError::ContractInvalid)?,
                    parent_expression_id: None,
                    lexical_depth: 0,
                    roles: Vec::new(),
                }),
            },
            start: node.start_byte(),
            end: node.end_byte(),
            parent_range: node
                .parent()
                .filter(|parent| {
                    selected_expression(*parent) && !excluded_identifier_context(*parent)
                })
                .map(|parent| {
                    (
                        parent.start_byte(),
                        parent.end_byte(),
                        parent.kind().to_owned(),
                    )
                }),
            roles: BTreeSet::new(),
        });
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_expressions(child, callable_id, builder, output)?;
    }
    Ok(())
}

fn assign_roles(
    body: Node<'_>,
    expressions: &mut [ExpressionDraft],
) -> Result<(), ExpressionBindingError> {
    let ranges = expressions
        .iter()
        .map(|value| ((value.start, value.end), value.entity.id.clone()))
        .collect::<BTreeMap<_, _>>();
    let parents = expressions
        .iter()
        .map(|value| {
            let parent = value
                .parent_range
                .as_ref()
                .and_then(|(start, end, _)| ranges.get(&(*start, *end)).cloned());
            (value.entity.id.clone(), parent)
        })
        .collect::<BTreeMap<_, _>>();
    for expression in expressions {
        let node = find_node(
            body,
            expression.start,
            expression.end,
            expression_syntax_kind(expression),
        )
        .ok_or(ExpressionBindingError::ParentInvalid)?;
        if expression.parent_range.is_some() {
            expression.roles.insert(ExpressionRole::Nested);
        }
        if let Some(parent) = node.parent() {
            match parent.kind() {
                "arguments" => {
                    expression.roles.insert(ExpressionRole::Argument);
                }
                "let_declaration" if same_field(parent, "value", node) => {
                    expression.roles.insert(ExpressionRole::Initializer);
                }
                "let_condition" | "match_expression" if same_field(parent, "value", node) => {
                    expression.roles.insert(ExpressionRole::PatternInput);
                }
                "for_expression" if same_field(parent, "value", node) => {
                    expression.roles.insert(ExpressionRole::Iterator);
                }
                "if_expression" | "while_expression" if same_field(parent, "condition", node) => {
                    expression.roles.insert(ExpressionRole::Condition);
                }
                "return_expression" => {
                    expression.roles.insert(ExpressionRole::ReturnValue);
                }
                "block" if last_named_child(parent).is_some_and(|last| same_node(last, node)) => {
                    expression.roles.insert(ExpressionRole::BodyTail);
                }
                _ => {}
            }
            if parent.kind() == "call_expression" && same_field(parent, "function", node) {
                expression.roles.insert(ExpressionRole::Callee);
            }
            if matches!(
                parent.kind(),
                "assignment_expression" | "compound_assignment_expr"
            ) {
                if same_field(parent, "left", node) {
                    expression.roles.insert(ExpressionRole::AssignmentTarget);
                }
                if same_field(parent, "right", node) {
                    expression.roles.insert(ExpressionRole::AssignmentValue);
                }
            }
            if parent.kind() == "field_expression" && same_field(parent, "value", node) {
                expression.roles.insert(ExpressionRole::Receiver);
            }
        }
        let parent_id = expression
            .parent_range
            .as_ref()
            .and_then(|(start, end, _)| ranges.get(&(*start, *end)).cloned());
        let mut depth = 0_u64;
        let mut ancestor = parent_id.clone();
        while let Some(identifier) = ancestor {
            depth = depth.saturating_add(1);
            if depth > MAX_R14_EXPRESSION_DEPTH {
                return Err(ExpressionBindingError::LimitExceeded {
                    limit: ExpressionBindingLimit::ExpressionDepth,
                    maximum: MAX_R14_EXPRESSION_DEPTH,
                    observed: depth,
                });
            }
            ancestor = parents.get(&identifier).cloned().flatten();
        }
        let ExpressionEntityProperties::Expression(properties) = &mut expression.entity.properties
        else {
            return Err(ExpressionBindingError::ContractInvalid);
        };
        properties.parent_expression_id = parent_id;
        properties.lexical_depth = depth;
        properties.roles = expression.roles.iter().copied().collect();
    }
    Ok(())
}

fn add_call_facts(
    node: Node<'_>,
    callable_id: &str,
    expressions: &[ExpressionDraft],
    builder: &mut SourceBuilder<'_>,
) -> Result<(), ExpressionBindingError> {
    if matches!(
        node.kind(),
        "macro_invocation"
            | "macro_definition"
            | "closure_expression"
            | "function_item"
            | "function_signature_item"
    ) {
        return Ok(());
    }
    if node.kind() == "call_expression" {
        let call = expression_for_node(expressions, node)?;
        let arguments = node
            .child_by_field_name("arguments")
            .ok_or(ExpressionBindingError::ArgumentOrdinalInvalid)?;
        let mut cursor = arguments.walk();
        let values = arguments.named_children(&mut cursor).collect::<Vec<_>>();
        enforce_expression_limit(ExpressionBindingLimit::ArgumentsPerCall, values.len())?;
        for (index, argument) in values.into_iter().enumerate() {
            let value = expression_for_node(expressions, argument)?;
            let ordinal =
                u64::try_from(index).map_err(|_| ExpressionBindingError::ArgumentOrdinalInvalid)?;
            let id = call_argument_entity_id(&call.entity.id, ordinal);
            builder.arguments.push(ExpressionBindingEntity {
                id: id.clone(),
                kind: ExpressionEntityKind::CallArgument,
                name: format!("argument:{ordinal}"),
                callable_id: callable_id.to_owned(),
                source_file_id: builder.source_file_id.to_owned(),
                evidence_id: value.entity.evidence_id.clone(),
                locator: value.entity.locator.clone(),
                properties: ExpressionEntityProperties::CallArgument(CallArgumentProperties {
                    call_expression_id: call.entity.id.clone(),
                    ordinal,
                    expression_id: value.entity.id.clone(),
                }),
            });
            builder.add_relationship(ExpressionBindingRelationship::new(
                ExpressionRelationshipKind::HasArgument,
                call.entity.id.clone(),
                id.clone(),
                vec![value.entity.evidence_id.clone()],
            ));
            builder.add_relationship(ExpressionBindingRelationship::new(
                ExpressionRelationshipKind::ArgumentValue,
                id,
                value.entity.id.clone(),
                vec![value.entity.evidence_id.clone()],
            ));
        }
        if let Some(function) = node.child_by_field_name("function")
            && function.kind() == "field_expression"
        {
            let receiver_node = function
                .child_by_field_name("value")
                .ok_or(ExpressionBindingError::ContractInvalid)?;
            let receiver = expression_for_node(expressions, receiver_node)?;
            builder.add_relationship(ExpressionBindingRelationship::new(
                ExpressionRelationshipKind::HasReceiver,
                call.entity.id.clone(),
                receiver.entity.id.clone(),
                vec![receiver.entity.evidence_id.clone()],
            ));
        }
        if let Some(call_site) = builder.existing.iter().find(|entity| {
            entity.kind == CallableSemanticEntityKind::CallSite
                && entity.subject_id == callable_id
                && entity.has_span(builder.path, node.start_byte(), node.end_byte())
        }) {
            builder.add_relationship(ExpressionBindingRelationship::new(
                ExpressionRelationshipKind::RepresentsCallSite,
                call.entity.id.clone(),
                call_site.id.clone(),
                vec![call.entity.evidence_id.clone()],
            ));
        }
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        add_call_facts(child, callable_id, expressions, builder)?;
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn collect_body_bindings(
    node: Node<'_>,
    callable_id: &str,
    expressions: &[ExpressionDraft],
    builder: &mut SourceBuilder<'_>,
    output: &mut Vec<BindingDraft>,
) -> Result<(), ExpressionBindingError> {
    let unsupported = match node.kind() {
        "macro_invocation" | "macro_definition" => Some("rust.expression_macro_expansion"),
        "closure_expression" => Some("rust.expression_closure_capture"),
        "function_item" | "function_signature_item" => Some("rust.expression_nested_callable"),
        _ => None,
    };
    if let Some(capability) = unsupported {
        builder.add_gap(capability, callable_id, node)?;
        return Ok(());
    }
    match node.kind() {
        "let_declaration" => {
            let owner_id = builder
                .existing
                .iter()
                .find(|entity| {
                    entity.kind == CallableSemanticEntityKind::LocalBinding
                        && entity.subject_id == callable_id
                        && entity.has_span(builder.path, node.start_byte(), node.end_byte())
                })
                .map(|entity| entity.id.clone())
                .ok_or(ExpressionBindingError::CallSiteEvidenceMismatch)?;
            let pattern = node
                .child_by_field_name("pattern")
                .ok_or(ExpressionBindingError::PatternUnsupported)?;
            let block = nearest_ancestor(node, "block")
                .ok_or(ExpressionBindingError::BindingScopeInvalid)?;
            let modifier = if node_text(node, builder.source)
                .trim_start()
                .starts_with("let mut ")
            {
                BindingModifier::ExplicitMut
            } else {
                BindingModifier::None
            };
            let value = match node.child_by_field_name("value") {
                Some(value)
                    if matches!(
                        value.kind(),
                        "closure_expression" | "macro_invocation" | "macro_definition"
                    ) =>
                {
                    builder.add_gap("rust.pattern_input_unexpanded", &owner_id, value)?;
                    None
                }
                Some(value) => Some(expression_key(expressions, value)?),
                None => None,
            };
            collect_pattern_bindings(
                pattern,
                callable_id,
                &owner_id,
                BindingOrigin::LocalLet,
                modifier,
                (node.end_byte(), block.end_byte()),
                value,
                builder,
                output,
            )?;
        }
        "if_expression" => {
            if let Some(condition) = node.child_by_field_name("condition")
                && condition.kind() == "let_condition"
            {
                let owner = control_owner(builder, callable_id, node, "if_let")?;
                let pattern = condition
                    .child_by_field_name("pattern")
                    .ok_or(ExpressionBindingError::PatternUnsupported)?;
                let value = expression_key(
                    expressions,
                    condition
                        .child_by_field_name("value")
                        .ok_or(ExpressionBindingError::PatternUnsupported)?,
                )?;
                let body = node
                    .child_by_field_name("consequence")
                    .ok_or(ExpressionBindingError::BindingScopeInvalid)?;
                collect_pattern_bindings(
                    pattern,
                    callable_id,
                    &owner.id,
                    BindingOrigin::IfLet,
                    BindingModifier::None,
                    (body.start_byte(), body.end_byte()),
                    Some(value),
                    builder,
                    output,
                )?;
            } else if let Some(condition) = node.child_by_field_name("condition")
                && find_named_kind(condition, "let_condition").is_some()
            {
                builder.add_gap("rust.pattern_condition_chain", callable_id, condition)?;
            }
        }
        "while_expression" => {
            if let Some(condition) = node.child_by_field_name("condition")
                && condition.kind() == "let_condition"
            {
                let owner = control_owner(builder, callable_id, node, "while_let")?;
                let pattern = condition
                    .child_by_field_name("pattern")
                    .ok_or(ExpressionBindingError::PatternUnsupported)?;
                let value = expression_key(
                    expressions,
                    condition
                        .child_by_field_name("value")
                        .ok_or(ExpressionBindingError::PatternUnsupported)?,
                )?;
                let body = node
                    .child_by_field_name("body")
                    .ok_or(ExpressionBindingError::BindingScopeInvalid)?;
                collect_pattern_bindings(
                    pattern,
                    callable_id,
                    &owner.id,
                    BindingOrigin::WhileLet,
                    BindingModifier::None,
                    (body.start_byte(), body.end_byte()),
                    Some(value),
                    builder,
                    output,
                )?;
            } else if let Some(condition) = node.child_by_field_name("condition")
                && find_named_kind(condition, "let_condition").is_some()
            {
                builder.add_gap("rust.pattern_condition_chain", callable_id, condition)?;
            }
        }
        "for_expression" => {
            let owner = control_owner(builder, callable_id, node, "for")?;
            let pattern = node
                .child_by_field_name("pattern")
                .ok_or(ExpressionBindingError::PatternUnsupported)?;
            let value = expression_key(
                expressions,
                node.child_by_field_name("value")
                    .ok_or(ExpressionBindingError::PatternUnsupported)?,
            )?;
            let body = node
                .child_by_field_name("body")
                .ok_or(ExpressionBindingError::BindingScopeInvalid)?;
            collect_pattern_bindings(
                pattern,
                callable_id,
                &owner.id,
                BindingOrigin::For,
                BindingModifier::None,
                (body.start_byte(), body.end_byte()),
                Some(value),
                builder,
                output,
            )?;
        }
        "match_expression" => {
            let owner = control_owner(builder, callable_id, node, "match")?;
            let scrutinee = expression_key(
                expressions,
                node.child_by_field_name("value")
                    .or_else(|| first_expression_child(node))
                    .ok_or(ExpressionBindingError::PatternUnsupported)?,
            )?;
            let body = node
                .child_by_field_name("body")
                .ok_or(ExpressionBindingError::BindingScopeInvalid)?;
            let mut cursor = body.walk();
            for arm in body
                .named_children(&mut cursor)
                .filter(|child| child.kind() == "match_arm")
            {
                if arm.child_by_field_name("condition").is_some() {
                    builder.add_gap("rust.pattern_guard", &owner.id, arm)?;
                    continue;
                }
                let pattern = arm
                    .child_by_field_name("pattern")
                    .ok_or(ExpressionBindingError::PatternUnsupported)?;
                let value = arm
                    .child_by_field_name("value")
                    .ok_or(ExpressionBindingError::BindingScopeInvalid)?;
                collect_pattern_bindings(
                    pattern,
                    callable_id,
                    &owner.id,
                    BindingOrigin::MatchArm,
                    BindingModifier::None,
                    (value.start_byte(), value.end_byte()),
                    Some(scrutinee.clone()),
                    builder,
                    output,
                )?;
            }
        }
        _ => {}
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_body_bindings(child, callable_id, expressions, builder, output)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_lines)]
fn collect_pattern_bindings(
    pattern: Node<'_>,
    callable_id: &str,
    owner_id: &str,
    origin: BindingOrigin,
    inherited_modifier: BindingModifier,
    scope: (usize, usize),
    source_expression_range: Option<(usize, usize, String)>,
    builder: &mut SourceBuilder<'_>,
    output: &mut Vec<BindingDraft>,
) -> Result<(), ExpressionBindingError> {
    match pattern.kind() {
        "identifier" | "self" => {
            let name = normalize(node_text(pattern, builder.source));
            if origin == BindingOrigin::MatchArm && matches!(name.as_str(), "None" | "Some") {
                return Ok(());
            }
            enforce_expression_limit(ExpressionBindingLimit::NormalizedSpellingBytes, name.len())?;
            let evidence_id = builder.add_evidence(pattern)?;
            let start_byte = u64::try_from(pattern.start_byte())
                .map_err(|_| ExpressionBindingError::ContractInvalid)?;
            let end_byte = u64::try_from(pattern.end_byte())
                .map_err(|_| ExpressionBindingError::ContractInvalid)?;
            output.push(BindingDraft {
                entity: ExpressionBindingEntity {
                    id: pattern_binding_entity_id(
                        builder.repository_identity,
                        callable_id,
                        owner_id,
                        builder.source_file_id,
                        start_byte,
                        end_byte,
                        &name,
                    ),
                    kind: ExpressionEntityKind::PatternBinding,
                    name,
                    callable_id: callable_id.to_owned(),
                    source_file_id: builder.source_file_id.to_owned(),
                    evidence_id,
                    locator: locator(builder, pattern)?,
                    properties: ExpressionEntityProperties::PatternBinding(
                        PatternBindingProperties {
                            origin,
                            scope_owner_id: owner_id.to_owned(),
                            modifier: inherited_modifier,
                            scope_start_byte: u64::try_from(scope.0)
                                .map_err(|_| ExpressionBindingError::BindingScopeInvalid)?,
                            scope_end_byte: u64::try_from(scope.1)
                                .map_err(|_| ExpressionBindingError::BindingScopeInvalid)?,
                        },
                    ),
                },
                owner_id: owner_id.to_owned(),
                scope_start: scope.0,
                scope_end: scope.1,
                source_expression_range,
            });
        }
        "tuple_struct_pattern" => {
            let mut cursor = pattern.walk();
            let children = pattern.named_children(&mut cursor).collect::<Vec<_>>();
            for child in children.into_iter().skip(1) {
                collect_pattern_bindings(
                    child,
                    callable_id,
                    owner_id,
                    origin,
                    inherited_modifier,
                    scope,
                    source_expression_range.clone(),
                    builder,
                    output,
                )?;
            }
        }
        "mut_pattern" => {
            if let Some(child) = pattern.named_child(0) {
                collect_pattern_bindings(
                    child,
                    callable_id,
                    owner_id,
                    origin,
                    BindingModifier::ExplicitMut,
                    scope,
                    source_expression_range,
                    builder,
                    output,
                )?;
            }
        }
        "ref_pattern" => {
            let index = u32::try_from(pattern.named_child_count().saturating_sub(1))
                .map_err(|_| ExpressionBindingError::PatternUnsupported)?;
            if let Some(child) = pattern.named_child(index) {
                let modifier = if node_text(pattern, builder.source).contains("mut") {
                    BindingModifier::ExplicitRefMut
                } else {
                    BindingModifier::ExplicitRef
                };
                collect_pattern_bindings(
                    child,
                    callable_id,
                    owner_id,
                    origin,
                    modifier,
                    scope,
                    source_expression_range,
                    builder,
                    output,
                )?;
            }
        }
        "reference_pattern" | "tuple_pattern" | "match_pattern" => {
            let mut cursor = pattern.walk();
            for child in pattern.named_children(&mut cursor) {
                collect_pattern_bindings(
                    child,
                    callable_id,
                    owner_id,
                    origin,
                    inherited_modifier,
                    scope,
                    source_expression_range.clone(),
                    builder,
                    output,
                )?;
            }
        }
        _ => builder.add_gap("rust.pattern_binding", owner_id, pattern)?,
    }
    Ok(())
}

fn aggregate_graph(
    chunks: &[ExpressionBindingSourceChunk],
) -> Result<ExpressionBindingGraph, ExpressionBindingError> {
    let mut entities = BTreeMap::new();
    let mut relationships = BTreeMap::new();
    let mut claims = BTreeMap::new();
    let mut evidence = BTreeMap::new();
    let mut coverage = BTreeMap::new();
    for chunk in chunks {
        extend_unique(&mut entities, &chunk.entities, |value| &value.id)?;
        extend_unique(&mut relationships, &chunk.relationships, |value| &value.id)?;
        extend_unique(&mut claims, &chunk.claims, |value| &value.id)?;
        extend_equal(&mut evidence, &chunk.evidence, |value| &value.id)?;
        extend_equal(&mut coverage, &chunk.coverage, |value| &value.id)?;
    }
    let entities = entities.into_values().collect::<Vec<_>>();
    let relationships = relationships.into_values().collect::<Vec<_>>();
    let index = ExpressionBindingIndex::from_graph(&entities, &relationships);
    Ok(ExpressionBindingGraph {
        entities,
        relationships,
        claims: claims.into_values().collect(),
        evidence: evidence.into_values().collect(),
        coverage: coverage.into_values().collect(),
        index,
    })
}

fn extend_unique<T: Clone>(
    target: &mut BTreeMap<String, T>,
    values: &[T],
    identifier: impl Fn(&T) -> &String,
) -> Result<(), ExpressionBindingError> {
    for value in values {
        if target
            .insert(identifier(value).clone(), value.clone())
            .is_some()
        {
            return Err(ExpressionBindingError::IdentityConflict);
        }
    }
    Ok(())
}

fn extend_equal<T: Clone + Eq>(
    target: &mut BTreeMap<String, T>,
    values: &[T],
    identifier: impl Fn(&T) -> &String,
) -> Result<(), ExpressionBindingError> {
    for value in values {
        if let Some(existing) = target.insert(identifier(value).clone(), value.clone())
            && existing != *value
        {
            return Err(ExpressionBindingError::IdentityConflict);
        }
    }
    Ok(())
}

fn control_owner(
    builder: &SourceBuilder<'_>,
    callable_id: &str,
    node: Node<'_>,
    kind: &str,
) -> Result<ExistingEntity, ExpressionBindingError> {
    builder
        .existing
        .iter()
        .find(|entity| {
            entity.kind == CallableSemanticEntityKind::Control
                && entity.subject_id == callable_id
                && entity.control_kind == Some(kind)
                && entity.has_span(builder.path, node.start_byte(), node.end_byte())
        })
        .cloned()
        .ok_or(ExpressionBindingError::CallSiteEvidenceMismatch)
}

fn expression_for_node<'a>(
    expressions: &'a [ExpressionDraft],
    node: Node<'_>,
) -> Result<&'a ExpressionDraft, ExpressionBindingError> {
    expressions
        .iter()
        .find(|candidate| {
            candidate.start == node.start_byte()
                && candidate.end == node.end_byte()
                && expression_syntax_kind(candidate) == node.kind()
        })
        .ok_or(ExpressionBindingError::ContractInvalid)
}

fn expression_key(
    expressions: &[ExpressionDraft],
    node: Node<'_>,
) -> Result<(usize, usize, String), ExpressionBindingError> {
    let value = expression_for_node(expressions, node)?;
    Ok((
        value.start,
        value.end,
        expression_syntax_kind(value).to_owned(),
    ))
}

fn expression_syntax_kind(expression: &ExpressionDraft) -> &str {
    match &expression.entity.properties {
        ExpressionEntityProperties::Expression(properties) => &properties.syntax_kind,
        _ => "",
    }
}

fn selected_expression(node: Node<'_>) -> bool {
    SELECTED_EXPRESSION_KINDS.contains(&node.kind())
}

fn excluded_identifier_context(node: Node<'_>) -> bool {
    if !matches!(node.kind(), "identifier" | "self") {
        return false;
    }
    let mut current = node;
    while let Some(parent) = current.parent() {
        if selected_expression(parent) {
            return parent.kind() == "scoped_identifier";
        }
        if PATTERN_CONTAINERS.contains(&parent.kind()) {
            return true;
        }
        if matches!(
            parent.kind(),
            "parameter"
                | "self_parameter"
                | "let_declaration"
                | "let_condition"
                | "for_expression"
                | "match_arm"
        ) && parent
            .child_by_field_name("pattern")
            .is_some_and(|pattern| {
                node.start_byte() >= pattern.start_byte() && node.end_byte() <= pattern.end_byte()
            })
        {
            return true;
        }
        if parent.kind().contains("type") {
            return true;
        }
        if matches!(
            parent.kind(),
            "attribute_item"
                | "inner_attribute_item"
                | "meta_item"
                | "use_declaration"
                | "use_as_clause"
                | "use_list"
        ) {
            return true;
        }
        if matches!(parent.kind(), "block" | "expression_statement") {
            break;
        }
        current = parent;
    }
    false
}

fn expression_operator(node: Node<'_>, source: &str) -> Option<String> {
    match node.kind() {
        "assignment_expression" => Some("=".to_owned()),
        "binary_expression" | "compound_assignment_expr" => node
            .child_by_field_name("operator")
            .map(|value| node_text(value, source).to_owned()),
        "unary_expression" => node
            .child(0)
            .filter(|value| !value.is_named())
            .map(|value| node_text(value, source).to_owned()),
        _ => None,
    }
}

fn locator(
    builder: &SourceBuilder<'_>,
    node: Node<'_>,
) -> Result<ExpressionLocator, ExpressionBindingError> {
    Ok(ExpressionLocator {
        path: builder.path.to_owned(),
        blob_oid: builder.blob_oid.to_owned(),
        start_byte: u64::try_from(node.start_byte())
            .map_err(|_| ExpressionBindingError::ContractInvalid)?,
        end_byte: u64::try_from(node.end_byte())
            .map_err(|_| ExpressionBindingError::ContractInvalid)?,
    })
}

fn first_expression_child(node: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| selected_expression(*child) && !excluded_identifier_context(*child))
}

fn same_field(parent: Node<'_>, field: &str, child: Node<'_>) -> bool {
    parent
        .child_by_field_name(field)
        .is_some_and(|value| same_node(value, child))
}

fn same_node(left: Node<'_>, right: Node<'_>) -> bool {
    left.start_byte() == right.start_byte()
        && left.end_byte() == right.end_byte()
        && left.kind() == right.kind()
}

fn node_text<'a>(node: Node<'_>, source: &'a str) -> &'a str {
    &source[node.start_byte()..node.end_byte()]
}

fn normalize(value: &str) -> String {
    value.nfc().collect()
}

fn nearest_ancestor<'tree>(mut node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    while let Some(parent) = node.parent() {
        if parent.kind() == kind {
            return Some(parent);
        }
        node = parent;
    }
    None
}

fn find_named_kind<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    if node.kind() == kind {
        return Some(node);
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if let Some(found) = find_named_kind(child, kind) {
            return Some(found);
        }
    }
    None
}

fn last_named_child(node: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).last()
}

fn find_node<'tree>(
    node: Node<'tree>,
    start: usize,
    end: usize,
    kind: &str,
) -> Option<Node<'tree>> {
    if node.start_byte() == start && node.end_byte() == end && node.kind() == kind {
        return Some(node);
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.start_byte() <= start
            && child.end_byte() >= end
            && let Some(found) = find_node(child, start, end, kind)
        {
            return Some(found);
        }
    }
    None
}

fn unique<'a>(values: impl IntoIterator<Item = &'a str>) -> bool {
    let mut seen = BTreeSet::new();
    values.into_iter().all(|value| seen.insert(value))
}

fn invalid_syntax(path: &str, start: usize, syntax_kind: &str) -> ExpressionBindingError {
    ExpressionBindingError::InvalidSyntax {
        path: path.to_owned(),
        start_byte: u64::try_from(start).unwrap_or(u64::MAX),
        syntax_kind: syntax_kind.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fr_ext_016_roles_are_canonical() {
        assert!(
            ExpressionRole::ALL
                .windows(2)
                .all(|pair| pair[0].as_str() < pair[1].as_str())
        );
    }
}
