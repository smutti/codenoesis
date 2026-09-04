use std::collections::{BTreeMap, BTreeSet, VecDeque};

use codenoesis_domain::RepositoryInventory;
use codenoesis_domain::knowledge::{ClaimState, ClaimSubjectKind};
use codenoesis_domain::s4::WorkspaceEvidence;
use codenoesis_domain::s4_k1::{
    CallableSemanticEntity, CallableSemanticEntityKind, CallableSemanticProperties,
};
use codenoesis_domain::s4_r3::ExternalWorkspaceBoundary;
use codenoesis_domain::s4_r14::{
    BindingOrigin, ExpressionBindingEntity, ExpressionBindingError, ExpressionBindingKnowledge,
    ExpressionEntityKind, ExpressionEntityProperties, ExpressionRelationshipKind,
};
use codenoesis_domain::s4_r15::{
    LocalFlowBlockRole, LocalFlowCoverageGap, LocalFlowDerivation, LocalFlowError,
    LocalFlowExtraction, LocalFlowGraph, LocalFlowIndex, LocalFlowLimit, LocalFlowLocator,
    LocalFlowRelationship, LocalFlowRelationshipKind, LocalFlowSourceChunk, SyntaxBasicBlock,
    enforce_local_flow_limit, local_flow_claim, local_flow_evidence_id, syntax_basic_block_id,
};
use codenoesis_ports::RustLocalFlowExtractor;
use tree_sitter::Node;

use crate::TreeSitterRustWorkspaceExtractor;
use crate::semantic_depth::{parse_tree, source_contexts, source_text};

const FLOW_GAPS: [&str; 2] = [
    "rust.syntax_normal_flow_not_analyzed",
    "rust.lexical_reaching_definitions_not_analyzed",
];

#[derive(Clone)]
struct EntityFact {
    evidence_ids: Vec<String>,
}

#[derive(Clone)]
struct AccessFact {
    expression_id: String,
    binding_id: String,
    relationship_id: String,
    start: usize,
}

#[derive(Clone)]
struct FlowOperation {
    reads: Vec<AccessFact>,
    definitions: Vec<(String, String)>,
    writes: Vec<AccessFact>,
}

#[derive(Clone)]
struct FlowStep {
    start: usize,
    end: usize,
    flow_node_ids: Vec<String>,
    operation: FlowOperation,
}

struct BlockDraft {
    block: SyntaxBasicBlock,
    operations: Vec<FlowOperation>,
}

#[derive(Clone)]
struct Fragment {
    entry: String,
    exits: Vec<String>,
}

enum ClosedResult<T> {
    Complete(T),
    Unsupported,
}

impl<T> ClosedResult<T> {
    fn complete(self) -> Option<T> {
        match self {
            Self::Complete(value) => Some(value),
            Self::Unsupported => None,
        }
    }
}

#[derive(Clone, Default)]
struct DefinitionState {
    may: BTreeSet<String>,
    must: BTreeSet<String>,
}

type FlowState = BTreeMap<String, DefinitionState>;

struct ReadObservation {
    read: AccessFact,
    block_id: String,
    may: BTreeSet<String>,
    must: BTreeSet<String>,
}

struct CallableBuilder<'a> {
    repository_id: &'a str,
    commit_oid: &'a str,
    source_file_id: &'a str,
    path: &'a str,
    blob_oid: &'a str,
    source: &'a str,
    callable_id: &'a str,
    expression: &'a ExpressionBindingKnowledge,
    entity_facts: &'a BTreeMap<String, EntityFact>,
    blocks: Vec<BlockDraft>,
    relationships: BTreeMap<String, LocalFlowRelationship>,
    evidence: BTreeMap<String, WorkspaceEvidence>,
}

impl<'a> CallableBuilder<'a> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        repository_id: &'a str,
        commit_oid: &'a str,
        source_file_id: &'a str,
        path: &'a str,
        blob_oid: &'a str,
        source: &'a str,
        callable_id: &'a str,
        expression: &'a ExpressionBindingKnowledge,
        entity_facts: &'a BTreeMap<String, EntityFact>,
    ) -> Self {
        Self {
            repository_id,
            commit_oid,
            source_file_id,
            path,
            blob_oid,
            source,
            callable_id,
            expression,
            entity_facts,
            blocks: Vec::new(),
            relationships: BTreeMap::new(),
            evidence: BTreeMap::new(),
        }
    }

    fn compile_body(&mut self, body: Node<'_>) -> Result<ClosedResult<Fragment>, LocalFlowError> {
        let nodes = named_children(body);
        if nodes.is_empty() || contains_forbidden(body, self.source) {
            return Ok(ClosedResult::Unsupported);
        }
        self.compile_sequence(&nodes, LocalFlowBlockRole::Entry, 0)
    }

    fn compile_sequence(
        &mut self,
        nodes: &[Node<'_>],
        initial_role: LocalFlowBlockRole,
        depth: usize,
    ) -> Result<ClosedResult<Fragment>, LocalFlowError> {
        let mut pending = Vec::new();
        let mut entry = None;
        let mut exits = Vec::new();
        let mut role = initial_role;
        for node in nodes {
            let semantic = semantic_node(*node);
            if semantic.kind() == "if_expression" {
                if let Some(block_id) = self.flush_steps(&mut pending, role)? {
                    if entry.is_none() {
                        entry = Some(block_id.clone());
                    }
                    self.connect_all(&exits, &block_id, LocalFlowRelationshipKind::SyntaxNext)?;
                    exits = vec![block_id];
                }
                let ClosedResult::Complete(branch) = self.compile_if(semantic, depth)? else {
                    return Ok(ClosedResult::Unsupported);
                };
                if entry.is_none() {
                    entry = Some(branch.entry.clone());
                }
                self.connect_all(&exits, &branch.entry, LocalFlowRelationshipKind::SyntaxNext)?;
                exits = branch.exits;
                role = LocalFlowBlockRole::Join;
            } else {
                let ClosedResult::Complete(step) = self.plain_step(*node, semantic) else {
                    return Ok(ClosedResult::Unsupported);
                };
                pending.push(step);
            }
        }
        if let Some(block_id) = self.flush_steps(&mut pending, role)? {
            if entry.is_none() {
                entry = Some(block_id.clone());
            }
            self.connect_all(&exits, &block_id, LocalFlowRelationshipKind::SyntaxNext)?;
            exits = vec![block_id];
        }
        let Some(entry) = entry else {
            return Ok(ClosedResult::Unsupported);
        };
        Ok(ClosedResult::Complete(Fragment { entry, exits }))
    }

    #[allow(clippy::too_many_lines)]
    fn compile_if(
        &mut self,
        node: Node<'_>,
        depth: usize,
    ) -> Result<ClosedResult<Fragment>, LocalFlowError> {
        enforce_local_flow_limit(LocalFlowLimit::NestedBranches, depth.saturating_add(1))?;
        let Some(condition) = node.child_by_field_name("condition") else {
            return Ok(ClosedResult::Unsupported);
        };
        let Some(consequence) = node.child_by_field_name("consequence") else {
            return Ok(ClosedResult::Unsupported);
        };
        let Some(alternative_clause) = node.child_by_field_name("alternative") else {
            return Ok(ClosedResult::Unsupported);
        };
        let alternative = if alternative_clause.kind() == "else_clause" {
            let Some(alternative) = alternative_clause.named_child(0) else {
                return Ok(ClosedResult::Unsupported);
            };
            alternative
        } else {
            alternative_clause
        };
        if condition.kind() == "let_condition" || consequence.kind() != "block" {
            return Ok(ClosedResult::Unsupported);
        }
        let Some(condition_expression) =
            expression_for_node(self.expression, self.callable_id, condition)
        else {
            return Ok(ClosedResult::Unsupported);
        };
        let Some(control) = control_for_node(
            &self.expression.callable.graph.entities,
            &self.expression.callable.graph.evidence,
            self.callable_id,
            self.path,
            node,
        ) else {
            return Err(LocalFlowError::AccessMismatch);
        };
        let condition_step = FlowStep {
            start: condition.start_byte(),
            end: condition.end_byte(),
            flow_node_ids: vec![control.id.clone(), condition_expression.id.clone()],
            operation: FlowOperation {
                reads: accesses_in_span(
                    self.expression,
                    self.callable_id,
                    condition.start_byte(),
                    condition.end_byte(),
                    ExpressionRelationshipKind::Reads,
                ),
                definitions: Vec::new(),
                writes: Vec::new(),
            },
        };
        let condition_block = self.add_block(LocalFlowBlockRole::Condition, &[condition_step])?;
        self.add_relationship(LocalFlowRelationship::new(
            LocalFlowRelationshipKind::HasCondition,
            control.id.clone(),
            condition_expression.id.clone(),
            union_evidence(
                self.entity_facts,
                &[control.id.as_str(), condition_expression.id.as_str()],
            )?,
        ));

        let consequence_nodes = named_children(consequence);
        if consequence_nodes.is_empty() {
            return Ok(ClosedResult::Unsupported);
        }
        let ClosedResult::Complete(then_fragment) = self.compile_sequence(
            &consequence_nodes,
            LocalFlowBlockRole::ThenBranch,
            depth.saturating_add(1),
        )?
        else {
            return Ok(ClosedResult::Unsupported);
        };
        let else_result = if alternative.kind() == "block" {
            let nodes = named_children(alternative);
            if nodes.is_empty() {
                ClosedResult::Unsupported
            } else {
                self.compile_sequence(
                    &nodes,
                    LocalFlowBlockRole::ElseBranch,
                    depth.saturating_add(1),
                )?
            }
        } else if alternative.kind() == "if_expression" {
            self.compile_if(alternative, depth.saturating_add(1))?
        } else {
            ClosedResult::Unsupported
        };
        let Some(else_fragment) = else_result.complete() else {
            return Ok(ClosedResult::Unsupported);
        };
        self.connect(
            &condition_block,
            &then_fragment.entry,
            LocalFlowRelationshipKind::SyntaxTrueBranch,
        )?;
        self.connect(
            &condition_block,
            &else_fragment.entry,
            LocalFlowRelationshipKind::SyntaxFalseBranch,
        )?;
        let mut exits = then_fragment.exits;
        exits.extend(else_fragment.exits);
        Ok(ClosedResult::Complete(Fragment {
            entry: condition_block,
            exits,
        }))
    }

    fn plain_step(&self, statement: Node<'_>, semantic: Node<'_>) -> ClosedResult<FlowStep> {
        if statement.kind() == "let_declaration" {
            let Some(value) = statement.child_by_field_name("value") else {
                return ClosedResult::Unsupported;
            };
            let Some(pattern) = statement.child_by_field_name("pattern") else {
                return ClosedResult::Unsupported;
            };
            let bindings = bindings_in_pattern(
                self.expression,
                self.callable_id,
                pattern.start_byte(),
                pattern.end_byte(),
            );
            if bindings.len() != 1 {
                return ClosedResult::Unsupported;
            }
            let Some(root) = expression_for_node(self.expression, self.callable_id, value) else {
                return ClosedResult::Unsupported;
            };
            let binding = bindings[0];
            return ClosedResult::Complete(FlowStep {
                start: statement.start_byte(),
                end: statement.end_byte(),
                flow_node_ids: vec![binding.id.clone(), root.id.clone()],
                operation: FlowOperation {
                    reads: accesses_in_span(
                        self.expression,
                        self.callable_id,
                        value.start_byte(),
                        value.end_byte(),
                        ExpressionRelationshipKind::Reads,
                    ),
                    definitions: vec![(binding.id.clone(), binding.id.clone())],
                    writes: Vec::new(),
                },
            });
        }
        if matches!(
            semantic.kind(),
            "return_expression"
                | "break_expression"
                | "continue_expression"
                | "try_expression"
                | "await_expression"
                | "closure_expression"
                | "match_expression"
                | "loop_expression"
                | "while_expression"
                | "for_expression"
                | "macro_invocation"
        ) {
            return ClosedResult::Unsupported;
        }
        let Some(root) = expression_for_node(self.expression, self.callable_id, semantic) else {
            return ClosedResult::Unsupported;
        };
        let reads = accesses_in_span(
            self.expression,
            self.callable_id,
            semantic.start_byte(),
            semantic.end_byte(),
            ExpressionRelationshipKind::Reads,
        );
        let writes = accesses_in_span(
            self.expression,
            self.callable_id,
            semantic.start_byte(),
            semantic.end_byte(),
            ExpressionRelationshipKind::Writes,
        );
        let assignment = matches!(
            semantic.kind(),
            "assignment_expression" | "compound_assignment_expr"
        );
        let direct_identifier_assignment = assignment
            && semantic
                .child_by_field_name("left")
                .is_some_and(|target| target.kind() == "identifier");
        if (assignment && (!direct_identifier_assignment || writes.len() != 1))
            || (!assignment && !writes.is_empty())
        {
            return ClosedResult::Unsupported;
        }
        ClosedResult::Complete(FlowStep {
            start: semantic.start_byte(),
            end: semantic.end_byte(),
            flow_node_ids: vec![root.id.clone()],
            operation: FlowOperation {
                reads,
                definitions: Vec::new(),
                writes,
            },
        })
    }

    fn flush_steps(
        &mut self,
        pending: &mut Vec<FlowStep>,
        role: LocalFlowBlockRole,
    ) -> Result<Option<String>, LocalFlowError> {
        if pending.is_empty() {
            return Ok(None);
        }
        let identifier = self.add_block(role, pending)?;
        pending.clear();
        Ok(Some(identifier))
    }

    fn add_block(
        &mut self,
        role: LocalFlowBlockRole,
        steps: &[FlowStep],
    ) -> Result<String, LocalFlowError> {
        let first = steps.first().ok_or(LocalFlowError::BlockInvalid)?;
        let last = steps.last().ok_or(LocalFlowError::BlockInvalid)?;
        let start_byte = u64::try_from(first.start).map_err(|_| LocalFlowError::BlockInvalid)?;
        let end_byte = u64::try_from(last.end).map_err(|_| LocalFlowError::BlockInvalid)?;
        let ordinal = u64::try_from(self.blocks.len()).map_err(|_| LocalFlowError::BlockInvalid)?;
        let evidence_id = local_flow_evidence_id(
            self.repository_id,
            self.commit_oid,
            self.blob_oid,
            self.path,
            start_byte,
            end_byte,
        );
        self.evidence
            .entry(evidence_id.clone())
            .or_insert_with(|| WorkspaceEvidence {
                id: evidence_id.clone(),
                path: self.path.to_owned(),
                blob_oid: self.blob_oid.to_owned(),
                start_byte,
                end_byte,
            });
        let flow_node_ids = steps
            .iter()
            .flat_map(|step| step.flow_node_ids.iter().cloned())
            .collect::<Vec<_>>();
        enforce_local_flow_limit(LocalFlowLimit::FlowNodesPerBlock, flow_node_ids.len())?;
        let identifier = syntax_basic_block_id(
            self.repository_id,
            self.callable_id,
            self.source_file_id,
            start_byte,
            end_byte,
            role,
            ordinal,
        );
        let block = SyntaxBasicBlock {
            id: identifier.clone(),
            callable_id: self.callable_id.to_owned(),
            source_file_id: self.source_file_id.to_owned(),
            evidence_id: evidence_id.clone(),
            locator: LocalFlowLocator {
                path: self.path.to_owned(),
                blob_oid: self.blob_oid.to_owned(),
                start_byte,
                end_byte,
            },
            ordinal,
            role,
            flow_node_ids: flow_node_ids.clone(),
        };
        self.add_relationship(LocalFlowRelationship::new(
            LocalFlowRelationshipKind::HasSyntaxBlock,
            self.callable_id.to_owned(),
            identifier.clone(),
            vec![evidence_id.clone()],
        ));
        for flow_node_id in &flow_node_ids {
            let mut evidence_ids = vec![evidence_id.clone()];
            evidence_ids.extend(
                self.entity_facts
                    .get(flow_node_id)
                    .ok_or(LocalFlowError::AccessMismatch)?
                    .evidence_ids
                    .clone(),
            );
            self.add_relationship(LocalFlowRelationship::new(
                LocalFlowRelationshipKind::ContainsFlowNode,
                identifier.clone(),
                flow_node_id.clone(),
                evidence_ids,
            ));
        }
        self.blocks.push(BlockDraft {
            block,
            operations: steps.iter().map(|step| step.operation.clone()).collect(),
        });
        Ok(identifier)
    }

    fn connect_all(
        &mut self,
        sources: &[String],
        target: &str,
        kind: LocalFlowRelationshipKind,
    ) -> Result<(), LocalFlowError> {
        for source in sources {
            self.connect(source, target, kind)?;
        }
        Ok(())
    }

    fn connect(
        &mut self,
        source: &str,
        target: &str,
        kind: LocalFlowRelationshipKind,
    ) -> Result<(), LocalFlowError> {
        let source_evidence = self
            .blocks
            .iter()
            .find(|block| block.block.id == source)
            .map(|block| block.block.evidence_id.clone())
            .ok_or(LocalFlowError::EdgeInvalid)?;
        let target_evidence = self
            .blocks
            .iter()
            .find(|block| block.block.id == target)
            .map(|block| block.block.evidence_id.clone())
            .ok_or(LocalFlowError::EdgeInvalid)?;
        self.add_relationship(LocalFlowRelationship::new(
            kind,
            source.to_owned(),
            target.to_owned(),
            vec![source_evidence, target_evidence],
        ));
        Ok(())
    }

    fn add_relationship(&mut self, relationship: LocalFlowRelationship) {
        self.relationships
            .entry(relationship.id.clone())
            .or_insert(relationship);
    }

    fn finish(
        mut self,
        parameter_definitions: &[(String, String)],
    ) -> Result<CallableOutput, LocalFlowError> {
        enforce_local_flow_limit(LocalFlowLimit::BlocksPerCallable, self.blocks.len())?;
        add_reachability(&self.blocks, &mut self.relationships)?;
        let lexical_derivations = add_lexical_flow(
            &self.blocks,
            &mut self.relationships,
            self.expression,
            self.entity_facts,
            parameter_definitions,
        )?;
        let mut derivations = syntax_derivations(&self.blocks, &self.relationships)?;
        derivations.extend(lexical_derivations);
        derivations.sort_by(|left, right| left.relationship_id.cmp(&right.relationship_id));
        let mut blocks = self
            .blocks
            .into_iter()
            .map(|draft| draft.block)
            .collect::<Vec<_>>();
        blocks.sort_by(|left, right| left.id.cmp(&right.id));
        let mut relationships = self.relationships.into_values().collect::<Vec<_>>();
        relationships.sort_by(|left, right| left.id.cmp(&right.id));
        let evidence = self.evidence.into_values().collect::<Vec<_>>();
        let mut claims = blocks
            .iter()
            .map(|block| {
                local_flow_claim(
                    ClaimSubjectKind::Entity,
                    block.id.clone(),
                    ClaimState::DeterministicFact,
                    vec![block.evidence_id.clone()],
                )
            })
            .chain(relationships.iter().map(|relationship| {
                local_flow_claim(
                    ClaimSubjectKind::Relationship,
                    relationship.id.clone(),
                    relationship.kind.claim_state(),
                    relationship.evidence_ids.clone(),
                )
            }))
            .collect::<Vec<_>>();
        claims.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(CallableOutput {
            blocks,
            relationships,
            claims,
            evidence,
            derivations,
        })
    }
}

struct CallableOutput {
    blocks: Vec<SyntaxBasicBlock>,
    relationships: Vec<LocalFlowRelationship>,
    claims: Vec<codenoesis_domain::s4::WorkspaceClaim>,
    evidence: Vec<WorkspaceEvidence>,
    derivations: Vec<LocalFlowDerivation>,
}

impl TreeSitterRustWorkspaceExtractor {
    /// Extracts the exact closed R15 syntax-normal local-flow profile.
    ///
    /// # Errors
    ///
    /// Returns an inherited R14 or typed identity, flow, access, or limit failure.
    #[allow(clippy::too_many_lines)]
    pub fn extract_rust_local_flow(
        &self,
        inventory: &RepositoryInventory,
    ) -> Result<LocalFlowExtraction, LocalFlowError> {
        let expression = self
            .extract_rust_expression_bindings(inventory)
            .map_err(LocalFlowError::Source)?;
        Self::extract_local_flow_from_expression(inventory, expression)
    }

    /// Extracts R15 over the exact R12 cfg-alternatives and repository-boundary lineage.
    ///
    /// # Errors
    ///
    /// Returns an inherited R12/R14 or typed identity, flow, access, or limit failure.
    pub fn extract_rust_local_flow_with_cfg_alternatives(
        &self,
        inventory: &RepositoryInventory,
        external_boundaries: &[ExternalWorkspaceBoundary],
    ) -> Result<LocalFlowExtraction, LocalFlowError> {
        let expression = self
            .extract_rust_expression_bindings_with_cfg_alternatives(inventory, external_boundaries)
            .map_err(LocalFlowError::Source)?;
        Self::extract_local_flow_from_expression(inventory, expression)
    }

    #[allow(clippy::too_many_lines)]
    fn extract_local_flow_from_expression(
        inventory: &RepositoryInventory,
        expression: codenoesis_domain::s4_r14::ExpressionBindingExtraction,
    ) -> Result<LocalFlowExtraction, LocalFlowError> {
        let contexts = source_contexts(
            &expression.knowledge.callable.framework.semantic.manifest,
            inventory,
        )
        .map_err(|error| {
            LocalFlowError::Source(ExpressionBindingError::Source(
                codenoesis_domain::s4_k1::CallableSemanticsError::Source(
                    codenoesis_domain::s4_r6::FrameworkError::Source(error),
                ),
            ))
        })?;
        let repository_id = inventory.bound_revision().repository_identity().as_str();
        let commit_oid = inventory.bound_revision().commit_oid().as_str();
        let entity_facts = inherited_entity_facts(&expression.knowledge)?;
        let mut chunks = Vec::new();
        let mut all_derivations = Vec::new();
        let mut completed_callable_ids = Vec::new();
        for context in contexts {
            let source = source_text(&context).map_err(|_| LocalFlowError::InvalidSyntax {
                path: context.path.clone(),
                start_byte: 0,
            })?;
            let tree =
                parse_tree(&context.path, source).map_err(|_| LocalFlowError::InvalidSyntax {
                    path: context.path.clone(),
                    start_byte: 0,
                })?;
            let mut chunk = LocalFlowSourceChunk {
                crate_id: context.crate_id.clone(),
                source_file_id: context.source_file_id.clone(),
                path: context.path.clone(),
                blocks: Vec::new(),
                relationships: Vec::new(),
                claims: Vec::new(),
                evidence: Vec::new(),
                coverage: Vec::new(),
            };
            let callable_nodes = declaration_nodes(tree.root_node());
            for callable_node in callable_nodes {
                let Some(body) = callable_node.child_by_field_name("body") else {
                    continue;
                };
                let header_end = body.start_byte();
                let Some(signature) = signature_for_node(
                    &expression.knowledge.callable.graph.entities,
                    &expression.knowledge.callable.graph.evidence,
                    &context.path,
                    callable_node.start_byte(),
                    header_end,
                ) else {
                    continue;
                };
                let callable_id = signature.subject_id.as_str();
                let signature_evidence = signature.evidence_ids.clone();
                let unsupported_r14 =
                    callable_has_unsupported_r14_coverage(&expression.knowledge, callable_id);
                let parameter_definitions =
                    parameter_definitions(&expression.knowledge, callable_id);
                let mut builder = CallableBuilder::new(
                    repository_id,
                    commit_oid,
                    &context.source_file_id,
                    &context.path,
                    context.file.blob_oid().as_str(),
                    source,
                    callable_id,
                    &expression.knowledge,
                    &entity_facts,
                );
                let complete = if unsupported_r14 {
                    false
                } else {
                    builder.compile_body(body)?.complete().is_some()
                };
                if !complete {
                    for capability in FLOW_GAPS {
                        chunk.coverage.push(LocalFlowCoverageGap::unsupported(
                            capability,
                            callable_id.to_owned(),
                            signature_evidence.clone(),
                        ));
                    }
                    continue;
                }
                let output = builder.finish(&parameter_definitions)?;
                completed_callable_ids.push(callable_id.to_owned());
                chunk.blocks.extend(output.blocks);
                chunk.relationships.extend(output.relationships);
                chunk.claims.extend(output.claims);
                chunk.evidence.extend(output.evidence);
                all_derivations.extend(output.derivations);
            }
            sort_chunk(&mut chunk);
            chunks.push(chunk);
        }
        let graph = aggregate_graph(&chunks, completed_callable_ids, all_derivations)?;
        let parser_invocation_count = expression
            .parser_invocation_count
            .saturating_add(u64::try_from(chunks.len()).unwrap_or(u64::MAX));
        let extraction =
            LocalFlowExtraction::from_r14(expression, chunks, graph, parser_invocation_count);
        extraction.knowledge.validate()?;
        Ok(extraction)
    }
}

impl RustLocalFlowExtractor for TreeSitterRustWorkspaceExtractor {
    fn extract_rust_local_flow(
        &self,
        inventory: &RepositoryInventory,
    ) -> Result<LocalFlowExtraction, LocalFlowError> {
        TreeSitterRustWorkspaceExtractor::extract_rust_local_flow(self, inventory)
    }
}

fn declaration_nodes(root: Node<'_>) -> Vec<Node<'_>> {
    fn collect<'tree>(node: Node<'tree>, output: &mut Vec<Node<'tree>>) {
        if matches!(
            node.kind(),
            "macro_invocation" | "macro_definition" | "closure_expression"
        ) {
            return;
        }
        if matches!(node.kind(), "function_item" | "function_signature_item") {
            output.push(node);
            return;
        }
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            collect(child, output);
        }
    }
    let mut output = Vec::new();
    collect(root, &mut output);
    output
}

fn signature_for_node<'a>(
    entities: &'a [CallableSemanticEntity],
    evidence: &[WorkspaceEvidence],
    path: &str,
    start: usize,
    end: usize,
) -> Option<&'a CallableSemanticEntity> {
    entities.iter().find(|entity| {
        entity.kind == CallableSemanticEntityKind::Signature
            && entity.evidence_ids.iter().any(|identifier| {
                evidence.iter().any(|value| {
                    value.id == *identifier
                        && value.path == path
                        && usize::try_from(value.start_byte).ok() == Some(start)
                        && usize::try_from(value.end_byte).ok() == Some(end)
                })
            })
    })
}

fn control_for_node<'a>(
    entities: &'a [CallableSemanticEntity],
    evidence: &[WorkspaceEvidence],
    callable_id: &str,
    path: &str,
    node: Node<'_>,
) -> Option<&'a CallableSemanticEntity> {
    entities.iter().find(|entity| {
        entity.kind == CallableSemanticEntityKind::Control
            && entity.subject_id == callable_id
            && matches!(
                &entity.properties,
                CallableSemanticProperties::Control(properties) if properties.control_kind.as_str() == "if"
            )
            && entity.evidence_ids.iter().any(|identifier| {
                evidence.iter().any(|value| {
                    value.id == *identifier
                        && value.path == path
                        && usize::try_from(value.start_byte).ok() == Some(node.start_byte())
                        && usize::try_from(value.end_byte).ok() == Some(node.end_byte())
                })
            })
    })
}

fn parameter_definitions(
    knowledge: &ExpressionBindingKnowledge,
    callable_id: &str,
) -> Vec<(String, String)> {
    knowledge
        .graph
        .entities
        .iter()
        .filter_map(|entity| {
            if entity.callable_id != callable_id {
                return None;
            }
            match &entity.properties {
                ExpressionEntityProperties::PatternBinding(properties)
                    if properties.origin == BindingOrigin::Parameter =>
                {
                    Some((entity.id.clone(), entity.id.clone()))
                }
                _ => None,
            }
        })
        .collect()
}

fn bindings_in_pattern<'a>(
    knowledge: &'a ExpressionBindingKnowledge,
    callable_id: &str,
    start: usize,
    end: usize,
) -> Vec<&'a ExpressionBindingEntity> {
    knowledge
        .graph
        .entities
        .iter()
        .filter(|entity| {
            entity.callable_id == callable_id
                && entity.kind == ExpressionEntityKind::PatternBinding
                && usize::try_from(entity.locator.start_byte).is_ok_and(|value| value >= start)
                && usize::try_from(entity.locator.end_byte).is_ok_and(|value| value <= end)
        })
        .collect()
}

fn expression_for_node<'a>(
    knowledge: &'a ExpressionBindingKnowledge,
    callable_id: &str,
    node: Node<'_>,
) -> Option<&'a ExpressionBindingEntity> {
    knowledge.graph.entities.iter().find(|entity| {
        entity.callable_id == callable_id
            && entity.kind == ExpressionEntityKind::Expression
            && usize::try_from(entity.locator.start_byte).ok() == Some(node.start_byte())
            && usize::try_from(entity.locator.end_byte).ok() == Some(node.end_byte())
            && matches!(
                &entity.properties,
                ExpressionEntityProperties::Expression(properties) if properties.syntax_kind == node.kind()
            )
    })
}

fn accesses_in_span(
    knowledge: &ExpressionBindingKnowledge,
    callable_id: &str,
    start: usize,
    end: usize,
    kind: ExpressionRelationshipKind,
) -> Vec<AccessFact> {
    let entities = knowledge
        .graph
        .entities
        .iter()
        .filter(|entity| entity.callable_id == callable_id)
        .map(|entity| (entity.id.as_str(), entity))
        .collect::<BTreeMap<_, _>>();
    let mut accesses = knowledge
        .graph
        .relationships
        .iter()
        .filter(|relationship| relationship.kind == kind)
        .filter_map(|relationship| {
            let entity = entities.get(relationship.source.as_str())?;
            let entity_start = usize::try_from(entity.locator.start_byte).ok()?;
            let entity_end = usize::try_from(entity.locator.end_byte).ok()?;
            (entity_start >= start && entity_end <= end).then(|| AccessFact {
                expression_id: relationship.source.clone(),
                binding_id: relationship.target.clone(),
                relationship_id: relationship.id.clone(),
                start: entity_start,
            })
        })
        .collect::<Vec<_>>();
    accesses.sort_by(|left, right| {
        (left.start, left.expression_id.as_str()).cmp(&(right.start, right.expression_id.as_str()))
    });
    accesses
}

fn inherited_entity_facts(
    knowledge: &ExpressionBindingKnowledge,
) -> Result<BTreeMap<String, EntityFact>, LocalFlowError> {
    let mut facts = BTreeMap::new();
    for entity in &knowledge.callable.graph.entities {
        insert_entity_fact(&mut facts, &entity.id, entity.evidence_ids.clone())?;
    }
    for entity in &knowledge.graph.entities {
        insert_entity_fact(&mut facts, &entity.id, vec![entity.evidence_id.clone()])?;
    }
    Ok(facts)
}

fn insert_entity_fact(
    facts: &mut BTreeMap<String, EntityFact>,
    identifier: &str,
    mut evidence_ids: Vec<String>,
) -> Result<(), LocalFlowError> {
    evidence_ids.sort();
    evidence_ids.dedup();
    let value = EntityFact { evidence_ids };
    if let Some(existing) = facts.insert(identifier.to_owned(), value.clone())
        && existing.evidence_ids != value.evidence_ids
    {
        return Err(LocalFlowError::IdentityConflict);
    }
    Ok(())
}

fn callable_has_unsupported_r14_coverage(
    knowledge: &ExpressionBindingKnowledge,
    callable_id: &str,
) -> bool {
    let subjects = knowledge
        .graph
        .entities
        .iter()
        .filter(|entity| entity.callable_id == callable_id)
        .map(|entity| entity.id.as_str())
        .chain(std::iter::once(callable_id))
        .collect::<BTreeSet<_>>();
    knowledge.graph.coverage.iter().any(|gap| {
        subjects.contains(gap.subject_id.as_str())
            && matches!(
                gap.capability.as_str(),
                "rust.expression_macro_expansion"
                    | "rust.expression_closure_capture"
                    | "rust.expression_nested_callable"
                    | "rust.pattern_input_unexpanded"
                    | "rust.pattern_condition_chain"
                    | "rust.pattern_guard"
                    | "rust.pattern_binding"
                    | "rust.lexical_binding_shadowing"
            )
    })
}

fn contains_forbidden(node: Node<'_>, source: &str) -> bool {
    if matches!(
        node.kind(),
        "loop_expression"
            | "while_expression"
            | "for_expression"
            | "match_expression"
            | "break_expression"
            | "continue_expression"
            | "return_expression"
            | "try_expression"
            | "await_expression"
            | "closure_expression"
            | "function_item"
            | "function_signature_item"
            | "async_block"
            | "unsafe_block"
            | "const_block"
            | "macro_invocation"
            | "macro_definition"
            | "label"
    ) {
        return true;
    }
    if matches!(node.kind(), "attribute_item" | "inner_attribute_item")
        && node_text(node, source).contains("cfg")
    {
        return true;
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .any(|child| contains_forbidden(child, source))
}

fn semantic_node(node: Node<'_>) -> Node<'_> {
    if node.kind() == "expression_statement" {
        node.named_child(0).unwrap_or(node)
    } else {
        node
    }
}

fn named_children(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).collect()
}

fn node_text<'a>(node: Node<'_>, source: &'a str) -> &'a str {
    &source[node.start_byte()..node.end_byte()]
}

fn union_evidence(
    facts: &BTreeMap<String, EntityFact>,
    identifiers: &[&str],
) -> Result<Vec<String>, LocalFlowError> {
    let mut evidence = identifiers
        .iter()
        .map(|identifier| facts.get(*identifier).ok_or(LocalFlowError::AccessMismatch))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flat_map(|fact| fact.evidence_ids.iter().cloned())
        .collect::<Vec<_>>();
    evidence.sort();
    evidence.dedup();
    Ok(evidence)
}

fn add_reachability(
    blocks: &[BlockDraft],
    relationships: &mut BTreeMap<String, LocalFlowRelationship>,
) -> Result<(), LocalFlowError> {
    let block_evidence = blocks
        .iter()
        .map(|block| (block.block.id.as_str(), block.block.evidence_id.as_str()))
        .collect::<BTreeMap<_, _>>();
    let adjacency = direct_adjacency(relationships.values());
    let mut additions = Vec::new();
    for source in block_evidence.keys() {
        let mut pending = VecDeque::from([*source]);
        let mut seen = BTreeSet::new();
        while let Some(current) = pending.pop_front() {
            for target in adjacency.get(current).into_iter().flatten() {
                if seen.insert(*target) {
                    additions.push(LocalFlowRelationship::new(
                        LocalFlowRelationshipKind::SyntaxReaches,
                        (*source).to_owned(),
                        (*target).to_owned(),
                        vec![
                            block_evidence
                                .get(source)
                                .ok_or(LocalFlowError::ReachabilityMismatch)?
                                .to_string(),
                            block_evidence
                                .get(target)
                                .ok_or(LocalFlowError::ReachabilityMismatch)?
                                .to_string(),
                        ],
                    ));
                    pending.push_back(*target);
                }
            }
        }
    }
    enforce_local_flow_limit(
        LocalFlowLimit::ReachabilityPairsPerCallable,
        additions.len(),
    )?;
    for relationship in additions {
        relationships.insert(relationship.id.clone(), relationship);
    }
    Ok(())
}

fn syntax_derivations(
    blocks: &[BlockDraft],
    relationships: &BTreeMap<String, LocalFlowRelationship>,
) -> Result<Vec<LocalFlowDerivation>, LocalFlowError> {
    let adjacency = direct_adjacency(relationships.values());
    let block_map = blocks
        .iter()
        .map(|block| (block.block.id.as_str(), &block.block))
        .collect::<BTreeMap<_, _>>();
    let direct = relationships
        .values()
        .filter(|relationship| relationship.kind.is_direct_syntax())
        .collect::<Vec<_>>();
    relationships
        .values()
        .filter(|relationship| relationship.kind == LocalFlowRelationshipKind::SyntaxReaches)
        .map(|relationship| {
            let (vertices, edges) = path_slice(
                &relationship.source,
                &relationship.target,
                &adjacency,
                &direct,
            );
            let evidence = vertices
                .iter()
                .map(|identifier| {
                    block_map
                        .get(identifier.as_str())
                        .map(|block| block.evidence_id.clone())
                        .ok_or(LocalFlowError::DerivationMismatch)
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(LocalFlowDerivation::new(
                relationship.id.clone(),
                vertices,
                edges,
                evidence,
            ))
        })
        .collect()
}

#[allow(clippy::too_many_lines)]
fn add_lexical_flow(
    blocks: &[BlockDraft],
    relationships: &mut BTreeMap<String, LocalFlowRelationship>,
    expression: &ExpressionBindingKnowledge,
    entity_facts: &BTreeMap<String, EntityFact>,
    parameter_definitions: &[(String, String)],
) -> Result<Vec<LocalFlowDerivation>, LocalFlowError> {
    let direct_relationships = relationships
        .values()
        .filter(|relationship| relationship.kind.is_direct_syntax())
        .cloned()
        .collect::<Vec<_>>();
    let adjacency = direct_adjacency(direct_relationships.iter());
    let predecessors = reverse_adjacency(&adjacency);
    let block_ordinals = blocks
        .iter()
        .map(|block| (block.block.id.clone(), block.block.ordinal))
        .collect::<BTreeMap<_, _>>();
    let mut initial_state = FlowState::new();
    let mut lineage = BTreeMap::<String, BTreeSet<String>>::new();
    let mut definition_blocks = BTreeMap::<String, String>::new();
    let mut definition_writes = BTreeMap::<String, String>::new();
    for (definition, binding) in parameter_definitions {
        initial_state.insert(
            binding.clone(),
            DefinitionState {
                may: BTreeSet::from([definition.clone()]),
                must: BTreeSet::from([definition.clone()]),
            },
        );
        lineage.insert(definition.clone(), BTreeSet::from([definition.clone()]));
    }
    let mut outputs = BTreeMap::<String, FlowState>::new();
    let mut observations = Vec::new();
    let mut ordered_blocks = blocks.iter().collect::<Vec<_>>();
    ordered_blocks.sort_by_key(|block| block.block.ordinal);
    for block in ordered_blocks {
        let predecessor_ids = predecessors
            .get(block.block.id.as_str())
            .cloned()
            .unwrap_or_default();
        let mut state = if predecessor_ids.is_empty() {
            initial_state.clone()
        } else {
            merge_states(
                predecessor_ids
                    .iter()
                    .map(|identifier| outputs.get(*identifier))
                    .collect::<Option<Vec<_>>>()
                    .ok_or(LocalFlowError::EdgeInvalid)?
                    .as_slice(),
            )
        };
        for operation in &block.operations {
            for read in &operation.reads {
                let reaching = state
                    .get(&read.binding_id)
                    .cloned()
                    .ok_or(LocalFlowError::AccessMismatch)?;
                if reaching.may.is_empty() {
                    return Err(LocalFlowError::AccessMismatch);
                }
                observations.push(ReadObservation {
                    read: read.clone(),
                    block_id: block.block.id.clone(),
                    may: reaching.may,
                    must: reaching.must,
                });
            }
            for (definition, binding) in &operation.definitions {
                state.insert(
                    binding.clone(),
                    DefinitionState {
                        may: BTreeSet::from([definition.clone()]),
                        must: BTreeSet::from([definition.clone()]),
                    },
                );
                lineage.insert(definition.clone(), BTreeSet::from([definition.clone()]));
                definition_blocks.insert(definition.clone(), block.block.id.clone());
            }
            for write in &operation.writes {
                let previous = state
                    .get(&write.binding_id)
                    .cloned()
                    .ok_or(LocalFlowError::AccessMismatch)?;
                let mut write_lineage = BTreeSet::from([write.expression_id.clone()]);
                for definition in previous.may {
                    write_lineage.extend(
                        lineage
                            .get(&definition)
                            .cloned()
                            .unwrap_or_else(|| BTreeSet::from([definition])),
                    );
                }
                lineage.insert(write.expression_id.clone(), write_lineage);
                definition_blocks.insert(write.expression_id.clone(), block.block.id.clone());
                definition_writes
                    .insert(write.expression_id.clone(), write.relationship_id.clone());
                state.insert(
                    write.binding_id.clone(),
                    DefinitionState {
                        may: BTreeSet::from([write.expression_id.clone()]),
                        must: BTreeSet::from([write.expression_id.clone()]),
                    },
                );
            }
        }
        outputs.insert(block.block.id.clone(), state);
    }

    let contains = relationships
        .values()
        .filter(|relationship| relationship.kind == LocalFlowRelationshipKind::ContainsFlowNode)
        .map(|relationship| {
            (
                (relationship.source.clone(), relationship.target.clone()),
                relationship.id.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let direct = direct_relationships.iter().collect::<Vec<_>>();
    let block_map = blocks
        .iter()
        .map(|block| (block.block.id.as_str(), &block.block))
        .collect::<BTreeMap<_, _>>();
    let mut derivations = Vec::new();
    for observation in observations {
        let write_based = observation
            .may
            .iter()
            .any(|definition| definition_writes.contains_key(definition));
        let start_block = lexical_slice_start(
            &observation.may,
            &definition_blocks,
            &observation.block_id,
            &predecessors,
            &block_ordinals,
        );
        let (mut block_entities, mut direct_inputs) =
            path_slice(&start_block, &observation.block_id, &adjacency, &direct);
        if start_block == observation.block_id {
            block_entities = vec![observation.block_id.clone()];
            direct_inputs.clear();
        }
        let mut lineage_entities = observation
            .may
            .iter()
            .flat_map(|definition| {
                lineage
                    .get(definition)
                    .cloned()
                    .unwrap_or_else(|| BTreeSet::from([definition.clone()]))
            })
            .collect::<BTreeSet<_>>();
        lineage_entities.insert(observation.read.expression_id.clone());
        let mut input_entities = block_entities.clone();
        input_entities.extend(lineage_entities.iter().cloned());
        let mut input_relationships = direct_inputs;
        input_relationships.push(observation.read.relationship_id.clone());
        if write_based {
            input_relationships.extend(
                observation
                    .may
                    .iter()
                    .filter_map(|definition| definition_writes.get(definition).cloned()),
            );
        } else {
            for identifier in observation
                .may
                .iter()
                .chain(std::iter::once(&observation.read.expression_id))
            {
                if let Some(block_id) = definition_blocks
                    .get(identifier)
                    .or_else(|| find_containing_block(blocks, identifier))
                    && let Some(relationship_id) =
                        contains.get(&(block_id.clone(), identifier.clone()))
                {
                    input_relationships.push(relationship_id.clone());
                }
            }
        }
        let mut input_evidence = input_entities
            .iter()
            .flat_map(|identifier| {
                block_map
                    .get(identifier.as_str())
                    .map(|block| vec![block.evidence_id.clone()])
                    .or_else(|| {
                        entity_facts
                            .get(identifier)
                            .map(|fact| fact.evidence_ids.clone())
                    })
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>();
        input_evidence.sort();
        input_evidence.dedup();

        for definition in &observation.must {
            let relationship = lexical_relationship(
                LocalFlowRelationshipKind::LexicalMustReachesRead,
                definition,
                &observation.read.expression_id,
                entity_facts,
            )?;
            let identifier = relationship.id.clone();
            relationships.insert(identifier.clone(), relationship);
            derivations.push(LocalFlowDerivation::new(
                identifier,
                input_entities.clone(),
                input_relationships.clone(),
                input_evidence.clone(),
            ));
        }
        for definition in observation.may.difference(&observation.must) {
            let relationship = lexical_relationship(
                LocalFlowRelationshipKind::LexicalMayReachesRead,
                definition,
                &observation.read.expression_id,
                entity_facts,
            )?;
            let identifier = relationship.id.clone();
            relationships.insert(identifier.clone(), relationship);
            derivations.push(LocalFlowDerivation::new(
                identifier,
                input_entities.clone(),
                input_relationships.clone(),
                input_evidence.clone(),
            ));
        }
    }
    let _ = expression;
    Ok(derivations)
}

fn lexical_relationship(
    kind: LocalFlowRelationshipKind,
    definition: &str,
    read: &str,
    entity_facts: &BTreeMap<String, EntityFact>,
) -> Result<LocalFlowRelationship, LocalFlowError> {
    Ok(LocalFlowRelationship::new(
        kind,
        definition.to_owned(),
        read.to_owned(),
        union_evidence(entity_facts, &[definition, read])?,
    ))
}

fn merge_states(states: &[&FlowState]) -> FlowState {
    let bindings = states
        .iter()
        .flat_map(|state| state.keys().cloned())
        .collect::<BTreeSet<_>>();
    bindings
        .into_iter()
        .map(|binding| {
            let may = states
                .iter()
                .flat_map(|state| {
                    state
                        .get(&binding)
                        .into_iter()
                        .flat_map(|value| value.may.iter().cloned())
                })
                .collect();
            let must = states
                .iter()
                .map(|state| {
                    state
                        .get(&binding)
                        .map(|value| value.must.clone())
                        .unwrap_or_default()
                })
                .reduce(|left, right| left.intersection(&right).cloned().collect())
                .unwrap_or_default();
            (binding, DefinitionState { may, must })
        })
        .collect()
}

fn lexical_slice_start(
    definitions: &BTreeSet<String>,
    definition_blocks: &BTreeMap<String, String>,
    read_block: &str,
    predecessors: &BTreeMap<&str, Vec<&str>>,
    ordinals: &BTreeMap<String, u64>,
) -> String {
    let blocks = definitions
        .iter()
        .filter_map(|definition| definition_blocks.get(definition).cloned())
        .collect::<BTreeSet<_>>();
    if blocks.is_empty() {
        return read_block.to_owned();
    }
    if blocks.len() == 1 {
        return blocks
            .into_iter()
            .next()
            .unwrap_or_else(|| read_block.to_owned());
    }
    let common = blocks
        .iter()
        .map(|block| ancestors(block, predecessors))
        .reduce(|left, right| left.intersection(&right).copied().collect())
        .unwrap_or_default();
    common
        .into_iter()
        .max_by_key(|identifier| ordinals.get(*identifier).copied().unwrap_or_default())
        .unwrap_or(read_block)
        .to_owned()
}

fn ancestors<'a>(
    block: &'a str,
    predecessors: &BTreeMap<&'a str, Vec<&'a str>>,
) -> BTreeSet<&'a str> {
    let mut values = BTreeSet::from([block]);
    let mut pending = VecDeque::from([block]);
    while let Some(current) = pending.pop_front() {
        for predecessor in predecessors.get(current).into_iter().flatten() {
            if values.insert(*predecessor) {
                pending.push_back(*predecessor);
            }
        }
    }
    values
}

fn find_containing_block<'a>(blocks: &'a [BlockDraft], entity_id: &str) -> Option<&'a String> {
    blocks
        .iter()
        .find(|block| {
            block
                .block
                .flow_node_ids
                .iter()
                .any(|value| value == entity_id)
        })
        .map(|block| &block.block.id)
}

fn path_slice(
    source: &str,
    target: &str,
    adjacency: &BTreeMap<&str, Vec<&str>>,
    direct: &[&LocalFlowRelationship],
) -> (Vec<String>, Vec<String>) {
    let forward = reachable_from(source, adjacency);
    let reverse = reverse_adjacency(adjacency);
    let backward = ancestors(target, &reverse);
    let vertices = forward
        .intersection(&backward)
        .copied()
        .chain(std::iter::once(source))
        .chain(std::iter::once(target))
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let edges = direct
        .iter()
        .filter(|relationship| {
            vertices.contains(&relationship.source) && vertices.contains(&relationship.target)
        })
        .map(|relationship| relationship.id.clone())
        .collect::<Vec<_>>();
    (vertices.into_iter().collect(), edges)
}

fn reachable_from<'a>(
    source: &'a str,
    adjacency: &BTreeMap<&'a str, Vec<&'a str>>,
) -> BTreeSet<&'a str> {
    let mut values = BTreeSet::new();
    let mut pending = VecDeque::from([source]);
    while let Some(current) = pending.pop_front() {
        for target in adjacency.get(current).into_iter().flatten() {
            if values.insert(*target) {
                pending.push_back(*target);
            }
        }
    }
    values
}

fn direct_adjacency<'a>(
    relationships: impl IntoIterator<Item = &'a LocalFlowRelationship>,
) -> BTreeMap<&'a str, Vec<&'a str>> {
    let mut adjacency = BTreeMap::<&str, Vec<&str>>::new();
    for relationship in relationships {
        if relationship.kind.is_direct_syntax() {
            adjacency
                .entry(&relationship.source)
                .or_default()
                .push(&relationship.target);
        }
    }
    for targets in adjacency.values_mut() {
        targets.sort_unstable();
        targets.dedup();
    }
    adjacency
}

fn reverse_adjacency<'a>(
    adjacency: &BTreeMap<&'a str, Vec<&'a str>>,
) -> BTreeMap<&'a str, Vec<&'a str>> {
    let mut reverse = BTreeMap::<&str, Vec<&str>>::new();
    for (source, targets) in adjacency {
        for target in targets {
            reverse.entry(*target).or_default().push(*source);
        }
    }
    reverse
}

fn sort_chunk(chunk: &mut LocalFlowSourceChunk) {
    chunk.blocks.sort_by(|left, right| left.id.cmp(&right.id));
    chunk
        .relationships
        .sort_by(|left, right| left.id.cmp(&right.id));
    chunk.claims.sort_by(|left, right| left.id.cmp(&right.id));
    chunk.evidence.sort_by(|left, right| left.id.cmp(&right.id));
    chunk.coverage.sort_by(|left, right| left.id.cmp(&right.id));
}

fn aggregate_graph(
    chunks: &[LocalFlowSourceChunk],
    completed_callable_ids: Vec<String>,
    derivations: Vec<LocalFlowDerivation>,
) -> Result<LocalFlowGraph, LocalFlowError> {
    let mut blocks = merge_values(
        chunks.iter().flat_map(|chunk| chunk.blocks.clone()),
        |value| value.id.clone(),
    )?;
    let mut relationships = merge_values(
        chunks.iter().flat_map(|chunk| chunk.relationships.clone()),
        |value| value.id.clone(),
    )?;
    let mut claims = merge_values(
        chunks.iter().flat_map(|chunk| chunk.claims.clone()),
        |value| value.id.clone(),
    )?;
    let mut evidence = merge_values(
        chunks.iter().flat_map(|chunk| chunk.evidence.clone()),
        |value| value.id.clone(),
    )?;
    let mut coverage = merge_values(
        chunks.iter().flat_map(|chunk| chunk.coverage.clone()),
        |value| value.id.clone(),
    )?;
    blocks.sort_by(|left, right| left.id.cmp(&right.id));
    relationships.sort_by(|left, right| left.id.cmp(&right.id));
    claims.sort_by(|left, right| left.id.cmp(&right.id));
    evidence.sort_by(|left, right| left.id.cmp(&right.id));
    coverage.sort_by(|left, right| left.id.cmp(&right.id));
    let index =
        LocalFlowIndex::from_graph(completed_callable_ids, &blocks, &relationships, derivations);
    Ok(LocalFlowGraph {
        blocks,
        relationships,
        claims,
        evidence,
        coverage,
        index,
    })
}

fn merge_values<T: Clone + Eq>(
    values: impl IntoIterator<Item = T>,
    identifier: impl Fn(&T) -> String,
) -> Result<Vec<T>, LocalFlowError> {
    let mut merged = BTreeMap::new();
    for value in values {
        let id = identifier(&value);
        if let Some(existing) = merged.insert(id, value.clone())
            && existing != value
        {
            return Err(LocalFlowError::IdentityConflict);
        }
    }
    Ok(merged.into_values().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fr_ext_017_block_roles_and_relationship_kinds_are_canonical() {
        assert_eq!(
            LocalFlowBlockRole::ALL.map(LocalFlowBlockRole::as_str),
            ["entry", "condition", "then_branch", "else_branch", "join"]
        );
        assert_eq!(LocalFlowRelationshipKind::ALL.len(), 9);
    }
}
