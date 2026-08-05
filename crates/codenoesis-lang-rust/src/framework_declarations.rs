use std::collections::{BTreeMap, BTreeSet};

use codenoesis_domain::RepositoryInventory;
use codenoesis_domain::knowledge::{ClaimSubjectKind, EntityKind, RelationshipKind};
use codenoesis_domain::s4::{
    WorkspaceEntity, WorkspaceEvidence, WorkspaceRelationship, WorkspaceVisibility,
    workspace_declaration_id, workspace_module_id,
};
use codenoesis_domain::s4_r3::ExternalWorkspaceBoundary;
use codenoesis_domain::s4_r5::{CompilationPresence, RustSemanticError, RustSemanticLimit};
use codenoesis_domain::s4_r6::{
    FrameworkCoverageGap, FrameworkDeclaration, FrameworkDeclarationIndex, FrameworkDiagnostic,
    FrameworkEpistemicState, FrameworkError, FrameworkExtraction, FrameworkGraph, FrameworkLimit,
    FrameworkRole, FrameworkSourceChunk, FrameworkSourceProfile, FrameworkTargetBinding,
    deterministic_framework_claim, framework_capability_state,
    framework_declaration_identity_preimage, framework_diagnostic_message,
    framework_limit_exceeded,
};
use codenoesis_domain::s5::AnalysisCacheEntry;
use codenoesis_ports::{RustFrameworkDeclarationExtractor, RustSemanticDepthExtractor};
use tree_sitter::{Node, Tree};
use unicode_normalization::UnicodeNormalization as _;

use crate::TreeSitterRustWorkspaceExtractor;
use crate::semantic_depth::{SourceContext, parse_tree, source_contexts, source_text};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ByteRange {
    start: usize,
    end: usize,
}

#[derive(Clone)]
struct LocalRecord {
    entity: WorkspaceEntity,
    source_file_id: String,
    span: ByteRange,
}

#[derive(Default)]
struct LocalCatalog {
    records: BTreeMap<String, LocalRecord>,
    exact: BTreeMap<(String, String, String), Vec<String>>,
    names: BTreeMap<(String, String), Vec<String>>,
}

impl LocalCatalog {
    fn insert(&mut self, record: LocalRecord) -> Result<(), FrameworkError> {
        let id = record.entity.id.clone();
        if let Some(existing) = self.records.get(&id) {
            if existing.entity == record.entity {
                return Ok(());
            }
            return Err(FrameworkError::ContractInvalid);
        }
        let crate_id = record
            .entity
            .crate_id
            .clone()
            .ok_or(FrameworkError::ContractInvalid)?;
        let module_path = record
            .entity
            .module_path
            .clone()
            .ok_or(FrameworkError::ContractInvalid)?;
        let name = record.entity.name.clone();
        self.exact
            .entry((crate_id.clone(), module_path, name.clone()))
            .or_default()
            .push(id.clone());
        self.names
            .entry((crate_id, name))
            .or_default()
            .push(id.clone());
        self.records.insert(id, record);
        Ok(())
    }

    fn record(&self, id: &str) -> Option<&LocalRecord> {
        self.records.get(id)
    }

    fn resolve(&self, crate_id: &str, current_module: &str, spelling: &str) -> TargetResolution {
        let Some((module_path, name, qualified)) = local_target_path(current_module, spelling)
        else {
            return TargetResolution::Unresolved;
        };
        let exact = self
            .exact
            .get(&(crate_id.to_owned(), module_path, name.clone()))
            .cloned()
            .unwrap_or_default();
        match exact.as_slice() {
            [id] => return TargetResolution::Unique(id.clone()),
            values if values.len() > 1 => return TargetResolution::Ambiguous(values.len()),
            _ => {}
        }
        if qualified {
            return TargetResolution::Unresolved;
        }
        let candidates = self
            .names
            .get(&(crate_id.to_owned(), name))
            .cloned()
            .unwrap_or_default();
        match candidates.as_slice() {
            [id] => TargetResolution::Unique(id.clone()),
            [] => TargetResolution::Unresolved,
            values => TargetResolution::Ambiguous(values.len()),
        }
    }
}

enum TargetResolution {
    Unique(String),
    Unresolved,
    Ambiguous(usize),
}

struct ChunkBuilder<'a> {
    repository_identity: &'a str,
    commit_oid: &'a str,
    context: &'a SourceContext<'a>,
    source: &'a str,
    source_sha256: String,
    supplemental_entities: BTreeMap<String, WorkspaceEntity>,
    declarations: BTreeMap<String, FrameworkDeclaration>,
    relationships: BTreeMap<String, WorkspaceRelationship>,
    claims: BTreeMap<String, codenoesis_domain::s4::WorkspaceClaim>,
    evidence: BTreeMap<String, WorkspaceEvidence>,
    diagnostics: BTreeMap<String, FrameworkDiagnostic>,
    coverage: BTreeMap<String, FrameworkCoverageGap>,
}

impl<'a> ChunkBuilder<'a> {
    fn new(
        repository_identity: &'a str,
        commit_oid: &'a str,
        context: &'a SourceContext<'a>,
        source: &'a str,
    ) -> Result<Self, FrameworkError> {
        let source_sha256 = sha256_hex(source.as_bytes());
        let mut builder = Self {
            repository_identity,
            commit_oid,
            context,
            source,
            source_sha256,
            supplemental_entities: BTreeMap::new(),
            declarations: BTreeMap::new(),
            relationships: BTreeMap::new(),
            claims: BTreeMap::new(),
            evidence: BTreeMap::new(),
            diagnostics: BTreeMap::new(),
            coverage: BTreeMap::new(),
        };
        builder.add_evidence(ByteRange {
            start: 0,
            end: source.len(),
        })?;
        Ok(builder)
    }

    fn add_evidence(&mut self, span: ByteRange) -> Result<String, FrameworkError> {
        if span.start >= span.end
            || span.end > self.source.len()
            || !self.source.is_char_boundary(span.start)
            || !self.source.is_char_boundary(span.end)
        {
            return Err(FrameworkError::InvalidDeclaration {
                path: self.context.path.clone(),
                reason: "invalid_utf8_source_span".to_owned(),
            });
        }
        let start_byte = u64::try_from(span.start).map_err(|_| FrameworkError::ContractInvalid)?;
        let end_byte = u64::try_from(span.end).map_err(|_| FrameworkError::ContractInvalid)?;
        let id = framework_source_evidence_id(
            &self.context.path,
            start_byte,
            end_byte,
            &self.source_sha256,
        );
        self.evidence
            .entry(id.clone())
            .or_insert_with(|| WorkspaceEvidence {
                id: id.clone(),
                path: self.context.path.clone(),
                blob_oid: self.context.file.blob_oid().as_str().to_owned(),
                start_byte,
                end_byte,
            });
        Ok(id)
    }

    fn add_supplemental(&mut self, record: &LocalRecord) -> Result<(), FrameworkError> {
        if self.supplemental_entities.contains_key(&record.entity.id) {
            return Ok(());
        }
        let evidence_id = self.add_evidence(record.span)?;
        let claim = deterministic_framework_claim(
            ClaimSubjectKind::Entity,
            record.entity.id.clone(),
            evidence_id,
        );
        self.claims.insert(claim.id.clone(), claim);
        self.supplemental_entities
            .insert(record.entity.id.clone(), record.entity.clone());
        Ok(())
    }

    fn add_declaration(
        &mut self,
        draft: DeclarationDraft,
        span: ByteRange,
    ) -> Result<FrameworkDeclaration, FrameworkError> {
        enforce_count(
            FrameworkLimit::FrameworkDeclarationsPerSource,
            self.declarations.len().saturating_add(1),
        )?;
        let evidence_id = self.add_evidence(span)?;
        let normalized_preimage_sha256 = sha256_hex(&framework_declaration_identity_preimage(
            self.repository_identity,
            &self.context.crate_id,
            &draft.lexical_owner_id,
            draft.role,
            draft.source_profile,
            &draft.source_form_identity,
            &draft.declared_key_or_target,
        ));
        let declaration = FrameworkDeclaration::new(
            self.repository_identity,
            draft.role,
            self.context.crate_id.clone(),
            draft.lexical_owner_id,
            draft.source_profile,
            draft.source_form_identity,
            draft.declared_key_or_target,
            draft.compilation_presence,
            draft.method,
            draft.path,
            draft.configuration_key,
            draft.target_spelling,
            draft.local_target_id,
            draft.target_binding,
            vec![evidence_id.clone()],
        );
        if self.declarations.contains_key(&declaration.id) {
            return Err(FrameworkError::IdentityConflict {
                normalized_preimage_sha256,
            });
        }
        let relationship = WorkspaceRelationship::new(
            RelationshipKind::Defines,
            declaration.lexical_owner_id.clone(),
            declaration.id.clone(),
            vec![evidence_id.clone()],
        );
        for (subject_kind, subject_id) in [
            (ClaimSubjectKind::Entity, declaration.id.clone()),
            (ClaimSubjectKind::Relationship, relationship.id.clone()),
        ] {
            let claim =
                deterministic_framework_claim(subject_kind, subject_id, evidence_id.clone());
            self.claims.insert(claim.id.clone(), claim);
        }
        self.relationships
            .insert(relationship.id.clone(), relationship);
        self.add_capability(
            &declaration,
            "rust.framework_runtime_not_observed",
            evidence_id.clone(),
            false,
        )?;
        for capability in draft.capabilities {
            self.add_capability(
                &declaration,
                capability.code,
                evidence_id.clone(),
                capability.diagnostic,
            )?;
        }
        self.declarations
            .insert(declaration.id.clone(), declaration.clone());
        Ok(declaration)
    }

    fn add_capability(
        &mut self,
        declaration: &FrameworkDeclaration,
        capability: &str,
        evidence_id: String,
        diagnostic: bool,
    ) -> Result<(), FrameworkError> {
        let state =
            framework_capability_state(capability).ok_or(FrameworkError::ContractInvalid)?;
        let gap = FrameworkCoverageGap::new(
            self.repository_identity,
            self.commit_oid,
            declaration.id.clone(),
            capability,
            state,
            vec![evidence_id.clone()],
        );
        self.coverage.insert(gap.id.clone(), gap);
        if diagnostic {
            let diagnostic = FrameworkDiagnostic::new(
                self.repository_identity,
                declaration.id.clone(),
                capability,
                framework_diagnostic_message(capability),
                vec![evidence_id],
            );
            self.diagnostics.insert(diagnostic.id.clone(), diagnostic);
        }
        Ok(())
    }

    fn finish(self) -> FrameworkSourceChunk {
        FrameworkSourceChunk {
            crate_id: self.context.crate_id.clone(),
            source_file_id: self.context.source_file_id.clone(),
            supplemental_entities: self.supplemental_entities.into_values().collect(),
            declarations: self.declarations.into_values().collect(),
            relationships: self.relationships.into_values().collect(),
            claims: self.claims.into_values().collect(),
            evidence: self.evidence.into_values().collect(),
            diagnostics: self.diagnostics.into_values().collect(),
            coverage: self.coverage.into_values().collect(),
        }
    }
}

struct DeclarationDraft {
    role: FrameworkRole,
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
    capabilities: Vec<CapabilityDraft>,
}

struct CapabilityDraft {
    code: &'static str,
    diagnostic: bool,
}

#[derive(Clone)]
struct AttributeDraft {
    text: String,
    span: ByteRange,
}

struct AttributedNode<'tree> {
    node: Node<'tree>,
    attributes: Vec<AttributeDraft>,
}

#[derive(Clone, Copy)]
struct CandidateRule {
    role: FrameworkRole,
    source_form_identity: &'static str,
    key_label: &'static str,
    compilation_presence: CompilationPresence,
    capability: &'static str,
    cfg_gap: bool,
}

struct BuilderSegment<'tree> {
    method: String,
    arguments: Vec<Node<'tree>>,
    span: ByteRange,
}

impl RustFrameworkDeclarationExtractor for TreeSitterRustWorkspaceExtractor {
    #[allow(clippy::too_many_lines)]
    fn extract_rust_framework_declarations_incremental(
        &self,
        inventory: &RepositoryInventory,
        external_boundaries: &[ExternalWorkspaceBoundary],
        cache_entries: &[AnalysisCacheEntry],
    ) -> Result<FrameworkExtraction, FrameworkError> {
        validate_framework_inventory_paths(inventory)?;
        let r5 = <Self as RustSemanticDepthExtractor>::extract_rust_semantic_depth_incremental(
            self,
            inventory,
            external_boundaries,
            cache_entries,
        )
        .map_err(map_semantic_error)?;
        let repository_identity = inventory.bound_revision().repository_identity().as_str();
        let commit_oid = inventory.bound_revision().commit_oid().as_str();
        let contexts = source_contexts(r5.knowledge.semantic_manifest(), inventory)
            .map_err(FrameworkError::Source)?;
        let mut trees = BTreeMap::<String, Tree>::new();
        let mut catalog = LocalCatalog::default();
        for context in &contexts {
            let source = source_text(context).map_err(FrameworkError::Source)?;
            let tree = parse_tree(&context.path, source).map_err(FrameworkError::Source)?;
            collect_context_catalog(context, tree.root_node(), source, &mut catalog)?;
            trees.insert(context.source_file_id.clone(), tree);
        }
        let base_entity_ids = r5_entity_ids(&r5.knowledge);
        let mut builders = BTreeMap::new();
        for context in &contexts {
            let source = source_text(context).map_err(FrameworkError::Source)?;
            builders.insert(
                context.source_file_id.clone(),
                ChunkBuilder::new(repository_identity, commit_oid, context, source)?,
            );
        }
        let mut needed_entities = BTreeSet::new();
        for context in &contexts {
            let source = source_text(context).map_err(FrameworkError::Source)?;
            let tree = trees
                .get(&context.source_file_id)
                .ok_or(FrameworkError::ContractInvalid)?;
            let lexical_owner_id = context_lexical_owner(context, &catalog)?;
            process_scope(
                tree.root_node(),
                source,
                context,
                &context.base_module_path,
                &lexical_owner_id,
                &catalog,
                builders
                    .get_mut(&context.source_file_id)
                    .ok_or(FrameworkError::ContractInvalid)?,
                &mut needed_entities,
            )?;
        }
        for entity_id in needed_entities {
            if base_entity_ids.contains(&entity_id) {
                continue;
            }
            let record = catalog
                .record(&entity_id)
                .ok_or(FrameworkError::ContractInvalid)?;
            builders
                .get_mut(&record.source_file_id)
                .ok_or(FrameworkError::ContractInvalid)?
                .add_supplemental(record)?;
        }
        let extraction_chunks = builders
            .into_values()
            .map(ChunkBuilder::finish)
            .collect::<Vec<_>>();
        let graph = aggregate_graph(repository_identity, &extraction_chunks)?;
        let parser_invocation_count = r5
            .parser_invocation_count
            .saturating_add(u64::try_from(contexts.len()).unwrap_or(u64::MAX));
        let extraction =
            FrameworkExtraction::from_r5(r5, extraction_chunks, graph, parser_invocation_count);
        extraction.knowledge.validate()?;
        Ok(extraction)
    }
}

fn validate_framework_inventory_paths(
    inventory: &RepositoryInventory,
) -> Result<(), FrameworkError> {
    for file in inventory.files() {
        let path = file.path();
        let first = path.as_bytes().first();
        let windows_prefix =
            path.as_bytes().get(1) == Some(&b':') && first.is_some_and(u8::is_ascii_alphabetic);
        let reason = if path.is_empty() {
            Some("empty_path")
        } else if path.starts_with('/') || windows_prefix {
            Some("absolute_path")
        } else if path.contains('\\') {
            Some("invalid_path_separator")
        } else if path.chars().any(char::is_control) {
            Some("control_character")
        } else if path
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
        {
            Some("unsafe_path_component")
        } else {
            None
        };
        if let Some(reason) = reason {
            return Err(FrameworkError::UnsafePath {
                path: "repository-path".to_owned(),
                reason: reason.to_owned(),
            });
        }
    }
    Ok(())
}

fn map_semantic_error(error: RustSemanticError) -> FrameworkError {
    match error {
        RustSemanticError::LimitExceeded {
            limit: RustSemanticLimit::OuterAttributesPerDeclaration,
            observed,
            ..
        } => framework_limit_exceeded(FrameworkLimit::OuterAttributesPerDeclaration, observed),
        RustSemanticError::LimitExceeded {
            limit: RustSemanticLimit::AttributeTokenBytes,
            observed,
            ..
        } => framework_limit_exceeded(FrameworkLimit::AttributeTokenBytes, observed),
        other => FrameworkError::Source(other),
    }
}

trait SemanticManifest {
    fn semantic_manifest(&self) -> &codenoesis_domain::s4_r4::CargoManifestKnowledge;
}

impl SemanticManifest for codenoesis_domain::s4_r5::RustSemanticKnowledge {
    fn semantic_manifest(&self) -> &codenoesis_domain::s4_r4::CargoManifestKnowledge {
        &self.manifest
    }
}

fn r5_entity_ids(knowledge: &codenoesis_domain::s4_r5::RustSemanticKnowledge) -> BTreeSet<String> {
    knowledge
        .manifest
        .workspace
        .knowledge
        .graph
        .entities
        .iter()
        .map(|value| value.id.clone())
        .chain(
            knowledge
                .manifest
                .graph
                .entities
                .iter()
                .map(|value| value.id.clone()),
        )
        .chain(
            knowledge
                .graph
                .legacy_entities
                .iter()
                .map(|value| value.id.clone()),
        )
        .chain(
            knowledge
                .graph
                .entities
                .iter()
                .map(|value| value.id.clone()),
        )
        .collect()
}

fn collect_context_catalog(
    context: &SourceContext<'_>,
    root: Node<'_>,
    source: &str,
    catalog: &mut LocalCatalog,
) -> Result<(), FrameworkError> {
    if context.base_module_path != "crate" {
        let (parent, name) =
            split_module_path(&context.base_module_path).ok_or(FrameworkError::ContractInvalid)?;
        catalog.insert(LocalRecord {
            entity: WorkspaceEntity::declaration(
                &context.repository_identity,
                EntityKind::RustModule,
                &context.crate_id,
                parent,
                name,
                WorkspaceVisibility::Public,
            ),
            source_file_id: context.source_file_id.clone(),
            span: ByteRange {
                start: 0,
                end: source.len(),
            },
        })?;
    }
    collect_local_declarations(root, source, context, &context.base_module_path, catalog)
}

fn collect_local_declarations(
    scope: Node<'_>,
    source: &str,
    context: &SourceContext<'_>,
    module_path: &str,
    catalog: &mut LocalCatalog,
) -> Result<(), FrameworkError> {
    for attributed in attributed_children(scope, source, &context.path)? {
        let node = attributed.node;
        match node.kind() {
            "mod_item" => {
                let name = node_name(node, source, &context.path)?;
                let child_path = child_module_path(module_path, &name);
                if let Some(body) = node.child_by_field_name("body") {
                    collect_local_declarations(body, source, context, &child_path, catalog)?;
                }
            }
            "function_item" | "struct_item" | "enum_item" | "trait_item" | "type_item" => {
                let kind = match node.kind() {
                    "function_item" => EntityKind::RustFunction,
                    "struct_item" => EntityKind::RustStruct,
                    "enum_item" => EntityKind::RustEnum,
                    "trait_item" => EntityKind::RustTrait,
                    "type_item" => EntityKind::RustTypeAlias,
                    _ => unreachable!(),
                };
                let name = node_name(node, source, &context.path)?;
                catalog.insert(LocalRecord {
                    entity: WorkspaceEntity::declaration(
                        &context.repository_identity,
                        kind,
                        &context.crate_id,
                        module_path,
                        &name,
                        workspace_visibility(node, source),
                    ),
                    source_file_id: context.source_file_id.clone(),
                    span: attributed_span(node, &attributed.attributes),
                })?;
            }
            _ => {}
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn process_scope(
    scope: Node<'_>,
    source: &str,
    context: &SourceContext<'_>,
    module_path: &str,
    module_owner_id: &str,
    catalog: &LocalCatalog,
    builder: &mut ChunkBuilder<'_>,
    needed_entities: &mut BTreeSet<String>,
) -> Result<(), FrameworkError> {
    for attributed in attributed_children(scope, source, &context.path)? {
        enforce_attributes(&attributed.attributes)?;
        let node = attributed.node;
        match node.kind() {
            "mod_item" => {
                if let Some(body) = node.child_by_field_name("body") {
                    let name = node_name(node, source, &context.path)?;
                    let child_path = child_module_path(module_path, &name);
                    let child_owner = workspace_module_id(
                        &context.repository_identity,
                        &context.crate_id,
                        &child_path,
                    );
                    process_scope(
                        body,
                        source,
                        context,
                        &child_path,
                        &child_owner,
                        catalog,
                        builder,
                        needed_entities,
                    )?;
                }
            }
            "function_item" | "struct_item" => {
                if let Some(rule) = candidate_rule(node, source, &attributed.attributes) {
                    emit_candidate(
                        node,
                        source,
                        context,
                        module_path,
                        module_owner_id,
                        &attributed.attributes,
                        rule,
                        catalog,
                        builder,
                        needed_entities,
                    )?;
                }
                if node.kind() == "function_item" {
                    emit_builder_tail(
                        node,
                        source,
                        context,
                        module_path,
                        catalog,
                        builder,
                        needed_entities,
                    )?;
                }
            }
            "macro_invocation" if macro_name(node, source).as_deref() == Some("declare_routes") => {
                needed_entities.insert(module_owner_id.to_owned());
                builder.add_declaration(
                    DeclarationDraft {
                        role: FrameworkRole::Route,
                        lexical_owner_id: module_owner_id.to_owned(),
                        source_profile: FrameworkSourceProfile::AttributeMacroCandidate,
                        source_form_identity: "candidate/declarative-route-macro/v1".to_owned(),
                        declared_key_or_target: "declare_routes!".to_owned(),
                        compilation_presence: CompilationPresence::Unconditional,
                        method: None,
                        path: None,
                        configuration_key: None,
                        target_spelling: None,
                        local_target_id: None,
                        target_binding: FrameworkTargetBinding::NotApplicable,
                        capabilities: vec![CapabilityDraft {
                            code: "rust.macro_generated_items_not_analyzed",
                            diagnostic: true,
                        }],
                    },
                    node_range(node),
                )?;
            }
            _ => {}
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn emit_candidate(
    node: Node<'_>,
    source: &str,
    context: &SourceContext<'_>,
    module_path: &str,
    module_owner_id: &str,
    attributes: &[AttributeDraft],
    rule: CandidateRule,
    catalog: &LocalCatalog,
    builder: &mut ChunkBuilder<'_>,
    needed_entities: &mut BTreeSet<String>,
) -> Result<(), FrameworkError> {
    let target_spelling = bounded_node_name(
        node,
        source,
        &context.path,
        FrameworkLimit::TargetSpellingBytes,
    )?;
    let resolution = catalog.resolve(&context.crate_id, module_path, &target_spelling);
    let (local_target_id, target_binding, mut capabilities) =
        target_binding(resolution, &target_spelling, needed_entities);
    capabilities.push(CapabilityDraft {
        code: rule.capability,
        diagnostic: true,
    });
    if rule.cfg_gap {
        capabilities.push(CapabilityDraft {
            code: "rust.cfg_presence_unresolved",
            diagnostic: false,
        });
    }
    needed_entities.insert(module_owner_id.to_owned());
    let declared_key_or_target = format!("{} -> {target_spelling}", rule.key_label)
        .nfc()
        .collect();
    builder.add_declaration(
        DeclarationDraft {
            role: rule.role,
            lexical_owner_id: module_owner_id.to_owned(),
            source_profile: FrameworkSourceProfile::AttributeMacroCandidate,
            source_form_identity: rule.source_form_identity.to_owned(),
            declared_key_or_target,
            compilation_presence: rule.compilation_presence,
            method: None,
            path: None,
            configuration_key: None,
            target_spelling: Some(target_spelling),
            local_target_id,
            target_binding,
            capabilities,
        },
        attributed_span(node, attributes),
    )?;
    Ok(())
}

fn emit_builder_tail(
    function: Node<'_>,
    source: &str,
    context: &SourceContext<'_>,
    module_path: &str,
    catalog: &LocalCatalog,
    builder: &mut ChunkBuilder<'_>,
    needed_entities: &mut BTreeSet<String>,
) -> Result<(), FrameworkError> {
    let Some(body) = function.child_by_field_name("body") else {
        return Ok(());
    };
    let Some(tail_index) = body.named_child_count().checked_sub(1) else {
        return Ok(());
    };
    let tail_index = u32::try_from(tail_index)
        .map_err(|_| invalid_framework_declaration(&context.path, "function_body_child_count"))?;
    let Some(tail) = body.named_child(tail_index) else {
        return Ok(());
    };
    if tail.kind() != "call_expression" {
        return Ok(());
    }
    let Some(segments) = builder_chain(tail, source, &context.path)? else {
        return Ok(());
    };
    enforce_count(
        FrameworkLimit::ExplicitRegistrationChainSegments,
        segments.len(),
    )?;
    let function_name = node_name(function, source, &context.path)?;
    let lexical_owner_id = workspace_declaration_id(
        &context.repository_identity,
        EntityKind::RustFunction,
        &context.crate_id,
        module_path,
        &function_name,
    );
    needed_entities.insert(lexical_owner_id.clone());
    for segment in segments {
        let Some(mut draft) = builder_declaration(
            &segment,
            source,
            context,
            module_path,
            &lexical_owner_id,
            catalog,
            needed_entities,
        )?
        else {
            continue;
        };
        draft.compilation_presence = CompilationPresence::Unconditional;
        builder.add_declaration(draft, segment.span)?;
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn builder_declaration(
    segment: &BuilderSegment<'_>,
    source: &str,
    context: &SourceContext<'_>,
    module_path: &str,
    lexical_owner_id: &str,
    catalog: &LocalCatalog,
    needed_entities: &mut BTreeSet<String>,
) -> Result<Option<DeclarationDraft>, FrameworkError> {
    if !reviewed_builder_method(&segment.method) {
        return Ok(None);
    }
    for argument in &segment.arguments {
        enforce_expression_depth(*argument, 1)?;
    }
    let mut role = FrameworkRole::Route;
    let mut source_form_identity = format!("builder-tail/{}/v1", segment.method);
    let mut method = None;
    let mut path = None;
    let mut configuration_key = None;
    let target;
    match (segment.method.as_str(), segment.arguments.as_slice()) {
        ("component", [value]) => {
            role = FrameworkRole::Component;
            target = direct_target(*value, source, &context.path, false)?;
        }
        ("service" | "layer" | "route_layer", [value]) => {
            role = FrameworkRole::Service;
            target = direct_target(*value, source, &context.path, false)?;
        }
        ("handler", [value]) => {
            role = FrameworkRole::Handler;
            target = direct_target(*value, source, &context.path, false)?;
        }
        ("with_state", [value]) => {
            role = FrameworkRole::Configuration;
            target = direct_target(*value, source, &context.path, false)?;
        }
        ("configuration", [key, value]) => {
            role = FrameworkRole::Configuration;
            configuration_key = Some(literal_string(
                *key,
                source,
                &context.path,
                FrameworkLimit::LiteralMethodOrConfigurationKeyBytes,
            )?);
            target = direct_target(*value, source, &context.path, false)?;
        }
        ("endpoint", [route_path, value]) => {
            role = FrameworkRole::Endpoint;
            path = Some(literal_string(
                *route_path,
                source,
                &context.path,
                FrameworkLimit::LiteralRoutePathBytes,
            )?);
            target = direct_target(*value, source, &context.path, false)?;
        }
        ("route", [route_method, route_path, value]) => {
            method = Some(literal_string(
                *route_method,
                source,
                &context.path,
                FrameworkLimit::LiteralMethodOrConfigurationKeyBytes,
            )?);
            path = Some(literal_string(
                *route_path,
                source,
                &context.path,
                FrameworkLimit::LiteralRoutePathBytes,
            )?);
            target = direct_target(*value, source, &context.path, false)?;
        }
        ("route", [route_path, wrapper]) => {
            path = Some(literal_string(
                *route_path,
                source,
                &context.path,
                FrameworkLimit::LiteralRoutePathBytes,
            )?);
            let Some((wrapper_method, wrapper_target)) =
                method_wrapper(*wrapper, source, &context.path)?
            else {
                return Ok(None);
            };
            method = Some(wrapper_method);
            target = Some(wrapper_target);
        }
        ("group" | "nest", [route_path, value]) => {
            path = Some(literal_string(
                *route_path,
                source,
                &context.path,
                FrameworkLimit::LiteralRoutePathBytes,
            )?);
            target = direct_target(*value, source, &context.path, true)?;
        }
        _ => {
            return Err(invalid_framework_declaration(
                &context.path,
                "malformed_reviewed_builder_method",
            ));
        }
    }
    let Some(target_spelling) = target else {
        return Ok(None);
    };
    enforce_bytes(FrameworkLimit::TargetSpellingBytes, target_spelling.len())?;
    let resolution = catalog.resolve(&context.crate_id, module_path, &target_spelling);
    let (local_target_id, target_binding, capabilities) =
        target_binding(resolution, &target_spelling, needed_entities);
    let declared_key_or_target = match role {
        FrameworkRole::Component | FrameworkRole::Handler | FrameworkRole::Service => {
            target_spelling.clone()
        }
        FrameworkRole::Configuration => configuration_key.as_ref().map_or_else(
            || target_spelling.clone(),
            |key| format!("{key} -> {target_spelling}"),
        ),
        FrameworkRole::Endpoint => {
            format!(
                "{} -> {target_spelling}",
                path.as_deref().unwrap_or_default()
            )
        }
        FrameworkRole::Route => {
            let route_path = path.as_deref().unwrap_or_default();
            method.as_ref().map_or_else(
                || format!("{route_path} -> {target_spelling}"),
                |value| format!("{value} {route_path} -> {target_spelling}"),
            )
        }
    }
    .nfc()
    .collect();
    if segment.method == "group" || segment.method == "nest" {
        source_form_identity = format!("builder-tail/{}/v1", segment.method);
    }
    Ok(Some(DeclarationDraft {
        role,
        lexical_owner_id: lexical_owner_id.to_owned(),
        source_profile: FrameworkSourceProfile::ExplicitBuilderRegistration,
        source_form_identity,
        declared_key_or_target,
        compilation_presence: CompilationPresence::Unconditional,
        method,
        path,
        configuration_key,
        target_spelling: Some(target_spelling),
        local_target_id,
        target_binding,
        capabilities,
    }))
}

fn target_binding(
    resolution: TargetResolution,
    target_spelling: &str,
    needed_entities: &mut BTreeSet<String>,
) -> (Option<String>, FrameworkTargetBinding, Vec<CapabilityDraft>) {
    match resolution {
        TargetResolution::Unique(id) => {
            needed_entities.insert(id.clone());
            (Some(id), FrameworkTargetBinding::ResolvedUnique, Vec::new())
        }
        TargetResolution::Unresolved => (
            None,
            FrameworkTargetBinding::UnresolvedExternal,
            vec![CapabilityDraft {
                code: "rust.framework_target_resolution_unresolved",
                diagnostic: true,
            }],
        ),
        TargetResolution::Ambiguous(candidate_count) => {
            let _ = (target_spelling, candidate_count);
            (
                None,
                FrameworkTargetBinding::AmbiguousLocal,
                vec![CapabilityDraft {
                    code: "rust.framework_target_resolution_ambiguous",
                    diagnostic: true,
                }],
            )
        }
    }
}

#[allow(clippy::too_many_lines)]
fn candidate_rule(
    node: Node<'_>,
    source: &str,
    attributes: &[AttributeDraft],
) -> Option<CandidateRule> {
    let compact = attributes
        .iter()
        .map(|attribute| compact_attribute(&attribute.text))
        .collect::<Vec<_>>();
    let has_cfg = compact.iter().any(|value| value.starts_with("#[cfg("));
    if compact
        .iter()
        .any(|value| value.starts_with("#[cfg_attr(") && value.contains("route("))
    {
        return Some(CandidateRule {
            role: FrameworkRole::Route,
            source_form_identity: "candidate/cfg-attr-route/v1",
            key_label: "cfg-attr-route",
            compilation_presence: CompilationPresence::AttributeTransformUnknown,
            capability: "rust.attribute_semantics_not_interpreted",
            cfg_gap: true,
        });
    }
    if has_cfg && compact.iter().any(|value| route_attribute(value)) {
        return Some(CandidateRule {
            role: FrameworkRole::Route,
            source_form_identity: "candidate/cfg-route-attributes/v1",
            key_label: "cfg-route-attributes",
            compilation_presence: CompilationPresence::ConditionalUnknown,
            capability: "rust.attribute_semantics_not_interpreted",
            cfg_gap: true,
        });
    }
    for value in compact.iter().rev() {
        let rule = if route_attribute(value) {
            Some((
                FrameworkRole::Route,
                "candidate/route-attribute/v1",
                "route-attribute",
            ))
        } else if value == "#[component]" {
            Some((
                FrameworkRole::Component,
                "candidate/component-attribute/v1",
                "component-attribute",
            ))
        } else if value == "#[service]" {
            Some((
                FrameworkRole::Service,
                "candidate/service-attribute/v1",
                "service-attribute",
            ))
        } else if value == "#[configuration]" {
            Some((
                FrameworkRole::Configuration,
                "candidate/configuration-attribute/v1",
                "configuration-attribute",
            ))
        } else if value == "#[command]" {
            Some((
                FrameworkRole::Handler,
                "candidate/command-attribute/v1",
                "command-attribute",
            ))
        } else if value == "#[runtime::entry]" {
            Some((
                FrameworkRole::Component,
                "candidate/runtime-entry-attribute/v1",
                "runtime-entry-attribute",
            ))
        } else if value == "#[bridge]" {
            Some((
                FrameworkRole::Service,
                "candidate/bridge-attribute/v1",
                "bridge-attribute",
            ))
        } else if value.starts_with("#[framework::endpoint(") {
            Some((
                FrameworkRole::Endpoint,
                "candidate/qualified-endpoint-attribute/v1",
                "qualified-endpoint-attribute",
            ))
        } else if node.kind() == "struct_item"
            && value.starts_with("#[derive(")
            && value
                .trim_start_matches("#[derive(")
                .trim_end_matches(")]")
                .split(',')
                .any(|item| item.rsplit("::").next() == Some("Component"))
        {
            Some((
                FrameworkRole::Component,
                "candidate/derive-component/v1",
                "derive-component",
            ))
        } else {
            None
        };
        if let Some((role, source_form_identity, key_label)) = rule {
            return Some(CandidateRule {
                role,
                source_form_identity,
                key_label,
                compilation_presence: CompilationPresence::Unconditional,
                capability: "rust.attribute_semantics_not_interpreted",
                cfg_gap: false,
            });
        }
    }
    let _ = source;
    None
}

fn route_attribute(value: &str) -> bool {
    value.starts_with("#[route(") || value.starts_with("#[framework::route(")
}

fn compact_attribute(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn attributed_children<'tree>(
    parent: Node<'tree>,
    source: &str,
    path: &str,
) -> Result<Vec<AttributedNode<'tree>>, FrameworkError> {
    let mut cursor = parent.walk();
    let mut pending = Vec::new();
    let mut values = Vec::new();
    for child in parent.named_children(&mut cursor) {
        match child.kind() {
            "attribute_item" => {
                parse_and_push_attribute(&mut pending, child, source, path)?;
            }
            "line_comment" | "block_comment" | "inner_attribute_item" => {}
            _ => {
                let mut attributes = std::mem::take(&mut pending);
                let mut child_cursor = child.walk();
                for direct in child.named_children(&mut child_cursor) {
                    if direct.kind() == "attribute_item"
                        && !attributes
                            .iter()
                            .any(|value| value.span == node_range(direct))
                    {
                        parse_and_push_attribute(&mut attributes, direct, source, path)?;
                    }
                }
                values.push(AttributedNode {
                    node: child,
                    attributes,
                });
            }
        }
    }
    Ok(values)
}

fn attribute_draft(
    node: Node<'_>,
    source: &str,
    path: &str,
) -> Result<AttributeDraft, FrameworkError> {
    let text = node_text(node, source);
    if text.is_empty() {
        return Err(invalid_framework_declaration(path, "empty_attribute"));
    }
    enforce_bytes(FrameworkLimit::AttributeTokenBytes, text.len())?;
    Ok(AttributeDraft {
        text: text.to_owned(),
        span: node_range(node),
    })
}

fn parse_and_push_attribute(
    attributes: &mut Vec<AttributeDraft>,
    node: Node<'_>,
    source: &str,
    path: &str,
) -> Result<(), FrameworkError> {
    enforce_count(
        FrameworkLimit::OuterAttributesPerDeclaration,
        attributes.len().saturating_add(1),
    )?;
    attributes.push(attribute_draft(node, source, path)?);
    Ok(())
}

fn enforce_attributes(attributes: &[AttributeDraft]) -> Result<(), FrameworkError> {
    enforce_count(
        FrameworkLimit::OuterAttributesPerDeclaration,
        attributes.len(),
    )
}

fn context_lexical_owner(
    context: &SourceContext<'_>,
    catalog: &LocalCatalog,
) -> Result<String, FrameworkError> {
    if context.base_module_path == "crate" {
        return Ok(context.crate_id.clone());
    }
    let (parent, name) =
        split_module_path(&context.base_module_path).ok_or(FrameworkError::ContractInvalid)?;
    let id = workspace_declaration_id(
        &context.repository_identity,
        EntityKind::RustModule,
        &context.crate_id,
        parent,
        name,
    );
    catalog
        .record(&id)
        .map(|_| id)
        .ok_or(FrameworkError::ContractInvalid)
}

fn builder_chain<'tree>(
    tail: Node<'tree>,
    source: &str,
    path: &str,
) -> Result<Option<Vec<BuilderSegment<'tree>>>, FrameworkError> {
    let Some(segment_count) = reviewed_builder_segment_count(tail, source, path)? else {
        return Ok(None);
    };
    enforce_count(
        FrameworkLimit::ExplicitRegistrationChainSegments,
        segment_count,
    )?;
    let mut segments = Vec::with_capacity(segment_count);
    let mut call = tail;
    while segments.len() < segment_count {
        let function = call
            .child_by_field_name("function")
            .ok_or_else(|| invalid_framework_declaration(path, "malformed_builder_call"))?;
        let field = function
            .child_by_field_name("field")
            .ok_or_else(|| invalid_framework_declaration(path, "malformed_builder_method"))?;
        let method_text = node_text(field, source);
        let method = if method_text.len() <= 64 {
            normalize_identifier(method_text)
        } else {
            String::new()
        };
        let arguments_node = call
            .child_by_field_name("arguments")
            .ok_or_else(|| invalid_framework_declaration(path, "malformed_builder_arguments"))?;
        let mut cursor = arguments_node.walk();
        let arguments = arguments_node
            .named_children(&mut cursor)
            .take(4)
            .collect::<Vec<_>>();
        let start = field
            .start_byte()
            .checked_sub(1)
            .ok_or_else(|| invalid_framework_declaration(path, "malformed_builder_span"))?;
        segments.push(BuilderSegment {
            method,
            arguments,
            span: ByteRange {
                start,
                end: call.end_byte(),
            },
        });
        call = function
            .child_by_field_name("value")
            .ok_or_else(|| invalid_framework_declaration(path, "malformed_builder_receiver"))?;
    }
    segments.reverse();
    Ok(Some(segments))
}

fn reviewed_builder_segment_count(
    mut call: Node<'_>,
    source: &str,
    path: &str,
) -> Result<Option<usize>, FrameworkError> {
    let mut count = 0_usize;
    loop {
        if call.kind() != "call_expression" {
            return Ok(None);
        }
        let function = call
            .child_by_field_name("function")
            .ok_or_else(|| invalid_framework_declaration(path, "malformed_builder_call"))?;
        if function.kind() != "field_expression" {
            let terminal = node_text(function, source).trim();
            let arguments = call
                .child_by_field_name("arguments")
                .ok_or_else(|| invalid_framework_declaration(path, "malformed_builder_root"))?;
            return Ok((matches!(terminal, "RegistrationSet::new" | "Router::new")
                && arguments.named_child_count() == 0)
                .then_some(count));
        }
        count = count.saturating_add(1);
        call = function
            .child_by_field_name("value")
            .ok_or_else(|| invalid_framework_declaration(path, "malformed_builder_receiver"))?;
    }
}

fn direct_target(
    node: Node<'_>,
    source: &str,
    path: &str,
    permit_zero_argument_call: bool,
) -> Result<Option<String>, FrameworkError> {
    match node.kind() {
        "identifier" | "scoped_identifier" => {
            let raw = node_text(node, source).trim();
            enforce_bytes(FrameworkLimit::TargetSpellingBytes, raw.len())?;
            let value = raw.nfc().collect::<String>();
            enforce_bytes(FrameworkLimit::TargetSpellingBytes, value.len())?;
            Ok((!value.is_empty()).then_some(value))
        }
        "call_expression" if permit_zero_argument_call => {
            let function = node
                .child_by_field_name("function")
                .ok_or_else(|| invalid_framework_declaration(path, "malformed_direct_target"))?;
            let arguments = node
                .child_by_field_name("arguments")
                .ok_or_else(|| invalid_framework_declaration(path, "malformed_direct_target"))?;
            if arguments.named_child_count() != 0 {
                return Ok(None);
            }
            direct_target(function, source, path, false)
        }
        _ => Ok(None),
    }
}

fn method_wrapper(
    node: Node<'_>,
    source: &str,
    path: &str,
) -> Result<Option<(String, String)>, FrameworkError> {
    if node.kind() != "call_expression" {
        return Ok(None);
    }
    let function = node
        .child_by_field_name("function")
        .ok_or_else(|| invalid_framework_declaration(path, "malformed_method_wrapper"))?;
    if !matches!(function.kind(), "identifier" | "scoped_identifier") {
        return Ok(None);
    }
    let wrapper_text = node_text(function, source);
    enforce_bytes(FrameworkLimit::TargetSpellingBytes, wrapper_text.len())?;
    let wrapper = normalize_identifier(wrapper_text);
    let method = match wrapper.rsplit("::").next().unwrap_or_default() {
        "get" => "GET",
        "post" => "POST",
        "put" => "PUT",
        "delete" => "DELETE",
        "patch" => "PATCH",
        "head" => "HEAD",
        "options" => "OPTIONS",
        "trace" => "TRACE",
        _ => return Ok(None),
    }
    .to_owned();
    let arguments = node
        .child_by_field_name("arguments")
        .ok_or_else(|| invalid_framework_declaration(path, "malformed_method_wrapper"))?;
    let mut cursor = arguments.walk();
    let values = arguments
        .named_children(&mut cursor)
        .take(2)
        .collect::<Vec<_>>();
    let [target] = values.as_slice() else {
        return Err(invalid_framework_declaration(
            path,
            "malformed_method_wrapper",
        ));
    };
    let Some(target) = direct_target(*target, source, path, false)? else {
        return Ok(None);
    };
    Ok(Some((method, target)))
}

fn literal_string(
    node: Node<'_>,
    source: &str,
    path: &str,
    limit: FrameworkLimit,
) -> Result<String, FrameworkError> {
    if node.kind() != "string_literal" {
        return Err(invalid_framework_declaration(
            path,
            "reviewed_literal_required",
        ));
    }
    let text = node_text(node, source).trim();
    let raw = text
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .filter(|value| !value.contains('\\'))
        .ok_or_else(|| invalid_framework_declaration(path, "literal_escape_not_interpreted"))?;
    enforce_bytes(limit, raw.len())?;
    let value = raw.nfc().collect::<String>();
    enforce_bytes(limit, value.len())?;
    validate_derived_literal(&value, path)?;
    Ok(value)
}

fn reviewed_builder_method(method: &str) -> bool {
    matches!(
        method,
        "component"
            | "service"
            | "configuration"
            | "endpoint"
            | "route"
            | "group"
            | "nest"
            | "layer"
            | "route_layer"
            | "with_state"
            | "handler"
    )
}

fn validate_derived_literal(value: &str, path: &str) -> Result<(), FrameworkError> {
    let lower = value.to_ascii_lowercase();
    let user_info = lower
        .split_once('@')
        .is_some_and(|(authority, _)| authority.contains(':'));
    if lower.contains("://")
        || lower.starts_with("file:")
        || lower.starts_with("git@")
        || lower.contains("?token=")
        || lower.contains("&token=")
        || lower.contains("password=")
        || lower.contains("secret=")
        || lower.contains("credential=")
        || lower.contains("authorization:")
        || lower.contains("bearer ")
        || user_info
    {
        return Err(invalid_framework_declaration(
            path,
            "private_locator_or_credential",
        ));
    }
    Ok(())
}

fn enforce_expression_depth(node: Node<'_>, depth: usize) -> Result<(), FrameworkError> {
    enforce_count(FrameworkLimit::RegistrationExpressionDepth, depth)?;
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        enforce_expression_depth(child, depth.saturating_add(1))?;
    }
    Ok(())
}

fn node_name(node: Node<'_>, source: &str, path: &str) -> Result<String, FrameworkError> {
    node.child_by_field_name("name")
        .map(|name| normalize_identifier(node_text(name, source)))
        .filter(|name| !name.is_empty())
        .ok_or_else(|| FrameworkError::InvalidDeclaration {
            path: path.to_owned(),
            reason: node.kind().to_owned(),
        })
}

fn bounded_node_name(
    node: Node<'_>,
    source: &str,
    path: &str,
    limit: FrameworkLimit,
) -> Result<String, FrameworkError> {
    let name = node
        .child_by_field_name("name")
        .ok_or_else(|| invalid_framework_declaration(path, node.kind()))?;
    let raw = node_text(name, source);
    enforce_bytes(limit, raw.len())?;
    let normalized = normalize_identifier(raw);
    enforce_bytes(limit, normalized.len())?;
    if normalized.is_empty() {
        return Err(invalid_framework_declaration(path, node.kind()));
    }
    Ok(normalized)
}

fn invalid_framework_declaration(path: &str, reason: &str) -> FrameworkError {
    FrameworkError::InvalidDeclaration {
        path: path.to_owned(),
        reason: reason.to_owned(),
    }
}

fn macro_name(node: Node<'_>, source: &str) -> Option<String> {
    node.child_by_field_name("macro")
        .or_else(|| node.named_child(0))
        .map(|value| normalize_identifier(node_text(value, source).trim_end_matches('!')))
        .filter(|value| !value.is_empty())
}

fn normalize_identifier(value: &str) -> String {
    value.strip_prefix("r#").unwrap_or(value).nfc().collect()
}

fn workspace_visibility(node: Node<'_>, source: &str) -> WorkspaceVisibility {
    let mut cursor = node.walk();
    if node
        .named_children(&mut cursor)
        .find(|child| child.kind() == "visibility_modifier")
        .is_some_and(|value| node_text(value, source).trim() == "pub")
    {
        WorkspaceVisibility::Public
    } else {
        WorkspaceVisibility::Private
    }
}

fn local_target_path(current_module: &str, spelling: &str) -> Option<(String, String, bool)> {
    let normalized = spelling
        .split("::")
        .map(str::trim)
        .map(normalize_identifier)
        .collect::<Vec<_>>();
    if normalized.is_empty()
        || normalized.iter().any(|segment| {
            segment.is_empty()
                || segment.contains(['<', '>', '&', '(', ')', '[', ']', '{', '}', ' '])
        })
    {
        return None;
    }
    let name = normalized.last()?.clone();
    let prefix = &normalized[..normalized.len().saturating_sub(1)];
    let qualified = !prefix.is_empty();
    let module = if normalized.first().is_some_and(|value| value == "crate") {
        if prefix.len() == 1 {
            "crate".to_owned()
        } else {
            prefix.join("::")
        }
    } else if prefix.is_empty() {
        current_module.to_owned()
    } else if normalized.first().is_some_and(|value| value == "self") {
        std::iter::once(current_module)
            .chain(prefix.iter().skip(1).map(String::as_str))
            .collect::<Vec<_>>()
            .join("::")
    } else if normalized.first().is_some_and(|value| value == "super") {
        let parent = current_module
            .rsplit_once("::")
            .map_or("crate", |(parent, _)| parent);
        std::iter::once(parent)
            .chain(prefix.iter().skip(1).map(String::as_str))
            .collect::<Vec<_>>()
            .join("::")
    } else {
        std::iter::once(current_module)
            .chain(prefix.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join("::")
    };
    Some((module, name, qualified))
}

fn child_module_path(parent: &str, name: &str) -> String {
    if parent == "crate" {
        format!("crate::{name}")
    } else {
        format!("{parent}::{name}")
    }
}

fn split_module_path(module_path: &str) -> Option<(&str, &str)> {
    module_path.rsplit_once("::")
}

fn attributed_span(node: Node<'_>, attributes: &[AttributeDraft]) -> ByteRange {
    ByteRange {
        start: attributes
            .first()
            .map_or(node.start_byte(), |attribute| attribute.span.start),
        end: node.end_byte(),
    }
}

fn node_range(node: Node<'_>) -> ByteRange {
    ByteRange {
        start: node.start_byte(),
        end: node.end_byte(),
    }
}

fn node_text<'a>(node: Node<'_>, source: &'a str) -> &'a str {
    node.utf8_text(source.as_bytes()).unwrap_or("")
}

fn enforce_count(limit: FrameworkLimit, observed: usize) -> Result<(), FrameworkError> {
    let observed = u64::try_from(observed).unwrap_or(u64::MAX);
    if observed > limit.maximum() {
        return Err(framework_limit_exceeded(limit, observed));
    }
    Ok(())
}

fn enforce_bytes(limit: FrameworkLimit, observed: usize) -> Result<(), FrameworkError> {
    enforce_count(limit, observed)
}

fn aggregate_graph(
    repository_identity: &str,
    chunks: &[FrameworkSourceChunk],
) -> Result<FrameworkGraph, FrameworkError> {
    let mut supplemental_entities = BTreeMap::new();
    let mut declarations = BTreeMap::new();
    let mut relationships = BTreeMap::new();
    let mut claims = BTreeMap::new();
    let mut evidence = BTreeMap::new();
    let mut diagnostics = BTreeMap::new();
    let mut coverage = BTreeMap::new();
    for chunk in chunks {
        extend_unique(
            &mut supplemental_entities,
            &chunk.supplemental_entities,
            |value| &value.id,
        )?;
        extend_declarations(repository_identity, &mut declarations, &chunk.declarations)?;
        extend_unique(&mut relationships, &chunk.relationships, |value| &value.id)?;
        extend_unique(&mut claims, &chunk.claims, |value| &value.id)?;
        extend_equal(&mut evidence, &chunk.evidence, |value| &value.id)?;
        extend_unique(&mut diagnostics, &chunk.diagnostics, |value| &value.id)?;
        extend_unique(&mut coverage, &chunk.coverage, |value| &value.id)?;
    }
    let declarations = declarations.into_values().collect::<Vec<_>>();
    let index = FrameworkDeclarationIndex {
        entity_ids: declarations.iter().map(|value| value.id.clone()).collect(),
        declared_registration_ids: declarations
            .iter()
            .filter(|value| {
                value.epistemic_state == FrameworkEpistemicState::DeclaredRegistrationSyntax
            })
            .map(|value| value.id.clone())
            .collect(),
        candidate_unresolved_ids: declarations
            .iter()
            .filter(|value| value.epistemic_state == FrameworkEpistemicState::CandidateUnresolved)
            .map(|value| value.id.clone())
            .collect(),
    };
    Ok(FrameworkGraph {
        supplemental_entities: supplemental_entities.into_values().collect(),
        declarations,
        relationships: relationships.into_values().collect(),
        claims: claims.into_values().collect(),
        evidence: evidence.into_values().collect(),
        diagnostics: diagnostics.into_values().collect(),
        coverage: coverage.into_values().collect(),
        index,
    })
}

fn extend_declarations(
    repository_identity: &str,
    target: &mut BTreeMap<String, FrameworkDeclaration>,
    values: &[FrameworkDeclaration],
) -> Result<(), FrameworkError> {
    for value in values {
        if target.insert(value.id.clone(), value.clone()).is_some() {
            return Err(FrameworkError::IdentityConflict {
                normalized_preimage_sha256: sha256_hex(&framework_declaration_identity_preimage(
                    repository_identity,
                    &value.crate_id,
                    &value.lexical_owner_id,
                    value.role,
                    value.source_profile,
                    &value.source_form_identity,
                    &value.declared_key_or_target,
                )),
            });
        }
    }
    Ok(())
}

fn extend_unique<T: Clone>(
    target: &mut BTreeMap<String, T>,
    values: &[T],
    identifier: impl Fn(&T) -> &String,
) -> Result<(), FrameworkError> {
    for value in values {
        if target
            .insert(identifier(value).clone(), value.clone())
            .is_some()
        {
            return Err(FrameworkError::ContractInvalid);
        }
    }
    Ok(())
}

fn extend_equal<T: Clone + Eq>(
    target: &mut BTreeMap<String, T>,
    values: &[T],
    identifier: impl Fn(&T) -> &String,
) -> Result<(), FrameworkError> {
    for value in values {
        if let Some(existing) = target.insert(identifier(value).clone(), value.clone())
            && existing != *value
        {
            return Err(FrameworkError::ContractInvalid);
        }
    }
    Ok(())
}

pub(crate) fn framework_source_evidence_id(
    path: &str,
    start_byte: u64,
    end_byte: u64,
    source_sha256: &str,
) -> String {
    let mut canonical = Vec::new();
    canonical.push(b'[');
    write_json_string(&mut canonical, "codenoesis.evidence-id/source-span/v1");
    canonical.push(b',');
    write_json_string(&mut canonical, path);
    canonical.push(b',');
    canonical.extend_from_slice(start_byte.to_string().as_bytes());
    canonical.push(b',');
    canonical.extend_from_slice(end_byte.to_string().as_bytes());
    canonical.push(b',');
    write_json_string(&mut canonical, source_sha256);
    canonical.push(b']');
    format!("urn:codenoesis:evidence:sha256:{}", sha256_hex(&canonical))
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

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = sha256(bytes);
    let mut value = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut value, "{byte:02x}").expect("writing to a String cannot fail");
    }
    value
}

#[allow(clippy::many_single_char_names)]
#[allow(clippy::too_many_lines)]
fn sha256(bytes: &[u8]) -> [u8; 32] {
    const INITIAL: [u32; 8] = [
        0x6a09_e667,
        0xbb67_ae85,
        0x3c6e_f372,
        0xa54f_f53a,
        0x510e_527f,
        0x9b05_688c,
        0x1f83_d9ab,
        0x5be0_cd19,
    ];
    const ROUND: [u32; 64] = [
        0x428a_2f98,
        0x7137_4491,
        0xb5c0_fbcf,
        0xe9b5_dba5,
        0x3956_c25b,
        0x59f1_11f1,
        0x923f_82a4,
        0xab1c_5ed5,
        0xd807_aa98,
        0x1283_5b01,
        0x2431_85be,
        0x550c_7dc3,
        0x72be_5d74,
        0x80de_b1fe,
        0x9bdc_06a7,
        0xc19b_f174,
        0xe49b_69c1,
        0xefbe_4786,
        0x0fc1_9dc6,
        0x240c_a1cc,
        0x2de9_2c6f,
        0x4a74_84aa,
        0x5cb0_a9dc,
        0x76f9_88da,
        0x983e_5152,
        0xa831_c66d,
        0xb003_27c8,
        0xbf59_7fc7,
        0xc6e0_0bf3,
        0xd5a7_9147,
        0x06ca_6351,
        0x1429_2967,
        0x27b7_0a85,
        0x2e1b_2138,
        0x4d2c_6dfc,
        0x5338_0d13,
        0x650a_7354,
        0x766a_0abb,
        0x81c2_c92e,
        0x9272_2c85,
        0xa2bf_e8a1,
        0xa81a_664b,
        0xc24b_8b70,
        0xc76c_51a3,
        0xd192_e819,
        0xd699_0624,
        0xf40e_3585,
        0x106a_a070,
        0x19a4_c116,
        0x1e37_6c08,
        0x2748_774c,
        0x34b0_bcb5,
        0x391c_0cb3,
        0x4ed8_aa4a,
        0x5b9c_ca4f,
        0x682e_6ff3,
        0x748f_82ee,
        0x78a5_636f,
        0x84c8_7814,
        0x8cc7_0208,
        0x90be_fffa,
        0xa450_6ceb,
        0xbef9_a3f7,
        0xc671_78f2,
    ];
    let bit_length = u64::try_from(bytes.len())
        .unwrap_or(u64::MAX)
        .saturating_mul(8);
    let mut padded = Vec::with_capacity(bytes.len().saturating_add(72));
    padded.extend_from_slice(bytes);
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_length.to_be_bytes());
    let mut state = INITIAL;
    for chunk in padded.chunks_exact(64) {
        let mut schedule = [0_u32; 64];
        for (index, word) in chunk.chunks_exact(4).enumerate() {
            schedule[index] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for index in 16..64 {
            let s0 = schedule[index - 15].rotate_right(7)
                ^ schedule[index - 15].rotate_right(18)
                ^ (schedule[index - 15] >> 3);
            let s1 = schedule[index - 2].rotate_right(17)
                ^ schedule[index - 2].rotate_right(19)
                ^ (schedule[index - 2] >> 10);
            schedule[index] = schedule[index - 16]
                .wrapping_add(s0)
                .wrapping_add(schedule[index - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ ((!e) & g);
            let temporary_one = h
                .wrapping_add(s1)
                .wrapping_add(choice)
                .wrapping_add(ROUND[index])
                .wrapping_add(schedule[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temporary_two = s0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temporary_one);
            d = c;
            c = b;
            b = a;
            a = temporary_one.wrapping_add(temporary_two);
        }
        for (value, addition) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *value = value.wrapping_add(addition);
        }
    }
    let mut digest = [0_u8; 32];
    for (chunk, value) in digest.chunks_exact_mut(4).zip(state) {
        chunk.copy_from_slice(&value.to_be_bytes());
    }
    digest
}
