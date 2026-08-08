use std::collections::{BTreeMap, BTreeSet};

use codenoesis_domain::RepositoryInventory;
use codenoesis_domain::knowledge::{ClaimState, ClaimSubjectKind, EntityKind};
use codenoesis_domain::s4::{
    WorkspaceEvidence, workspace_declaration_id, workspace_evidence_id, workspace_module_id,
};
use codenoesis_domain::s4_k1::{
    CallForm, CallResolutionState, CallSiteProperties, CallableBodyState, CallableCoverageGap,
    CallableCoverageState, CallableDiagnostic, CallableParameterProperties, CallableReceiverState,
    CallableRelationship, CallableRelationshipKind, CallableReturnState, CallableSemanticEntity,
    CallableSemanticEntityKind, CallableSemanticProperties, CallableSemanticsError,
    CallableSemanticsExtraction, CallableSemanticsGraph, CallableSemanticsIndex,
    CallableSemanticsLimit, CallableSignatureProperties, CallableSourceChunk, ControlKind,
    ControlProperties, DeclaredValueProperties, DeclaredValueState, LocalBindingProperties,
    NormalizedScalarValue, callable_body_fact_id, callable_claim, callable_parameter_id,
    callable_signature_id, declared_value_id, enforce_limit, k1_digest,
};
use codenoesis_domain::s4_r5::{RustSemanticEntityKind, rust_semantic_member_id};
use codenoesis_ports::RustFrameworkDeclarationExtractor;
use tree_sitter::Node;
use unicode_normalization::UnicodeNormalization as _;

use crate::TreeSitterRustWorkspaceExtractor;
use crate::semantic_depth::{parse_tree, source_contexts, source_text};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct OwnerKey {
    crate_id: String,
    module_path: String,
    kind: EntityKind,
    name: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FreeFunctionKey {
    crate_id: String,
    module_path: String,
    name: String,
}

struct ExistingCatalog {
    owners: BTreeMap<OwnerKey, String>,
    free_functions: BTreeMap<FreeFunctionKey, String>,
    semantic_ids: BTreeSet<String>,
}

impl ExistingCatalog {
    fn from_extraction(framework: &codenoesis_domain::s4_r6::FrameworkExtraction) -> Self {
        let workspace = &framework
            .knowledge
            .semantic
            .manifest
            .workspace
            .knowledge
            .graph;
        let mut owners = BTreeMap::new();
        let mut free_functions = BTreeMap::new();
        for entity in workspace
            .entities
            .iter()
            .chain(framework.knowledge.semantic.graph.legacy_entities.iter())
            .chain(framework.knowledge.graph.supplemental_entities.iter())
        {
            let Some(crate_id) = entity.crate_id.as_ref() else {
                continue;
            };
            let Some(module_path) = entity.module_path.as_ref() else {
                continue;
            };
            match entity.kind {
                EntityKind::RustModule
                | EntityKind::RustStruct
                | EntityKind::RustEnum
                | EntityKind::RustTrait => {
                    owners.insert(
                        OwnerKey {
                            crate_id: crate_id.clone(),
                            module_path: module_path.clone(),
                            kind: entity.kind,
                            name: entity.name.clone(),
                        },
                        entity.id.clone(),
                    );
                }
                EntityKind::RustFunction => {
                    free_functions.insert(
                        FreeFunctionKey {
                            crate_id: crate_id.clone(),
                            module_path: module_path.clone(),
                            name: entity.name.clone(),
                        },
                        entity.id.clone(),
                    );
                }
                EntityKind::RustCrate
                | EntityKind::RustMethod
                | EntityKind::RustSymbolReference
                | EntityKind::RustTypeAlias
                | EntityKind::SourceFile => {}
            }
        }
        let semantic_ids = framework
            .knowledge
            .semantic
            .graph
            .entities
            .iter()
            .map(|value| value.id.clone())
            .collect();
        Self {
            owners,
            free_functions,
            semantic_ids,
        }
    }

    fn owner(
        &self,
        crate_id: &str,
        module_path: &str,
        kind: EntityKind,
        name: &str,
    ) -> Option<&str> {
        self.owners
            .get(&OwnerKey {
                crate_id: crate_id.to_owned(),
                module_path: module_path.to_owned(),
                kind,
                name: name.to_owned(),
            })
            .map(String::as_str)
    }

    fn resolve_owner(
        &self,
        crate_id: &str,
        current_module: &str,
        spelling: &str,
        kinds: &[EntityKind],
    ) -> Option<&str> {
        let (module_path, name) = local_path(current_module, spelling)?;
        let exact = kinds
            .iter()
            .filter_map(|kind| self.owner(crate_id, &module_path, *kind, &name))
            .collect::<Vec<_>>();
        match exact.as_slice() {
            [value] => Some(*value),
            _ => None,
        }
    }

    fn resolve_free_function(
        &self,
        crate_id: &str,
        current_module: &str,
        spelling: &str,
    ) -> Option<&str> {
        let (module_path, name) = local_path(current_module, spelling)?;
        let exact = self.free_functions.get(&FreeFunctionKey {
            crate_id: crate_id.to_owned(),
            module_path,
            name: name.clone(),
        });
        if exact.is_some() {
            return exact.map(String::as_str);
        }
        if spelling.contains("::") {
            return None;
        }
        let candidates = self
            .free_functions
            .iter()
            .filter(|(key, _)| key.crate_id == crate_id && key.name == name)
            .map(|(_, value)| value.as_str())
            .collect::<Vec<_>>();
        match candidates.as_slice() {
            [value] => Some(*value),
            _ => None,
        }
    }
}

#[derive(Clone)]
enum ScopeOwner {
    Module {
        module_id: String,
    },
    Trait {
        owner_id: String,
    },
    Implementation {
        owner_id: String,
        trait_context_id: Option<String>,
    },
}

impl ScopeOwner {
    fn method_context(&self) -> Option<(&str, Option<&str>)> {
        match self {
            Self::Trait { owner_id } => Some((owner_id, Some(owner_id))),
            Self::Implementation {
                owner_id,
                trait_context_id,
            } => Some((owner_id, trait_context_id.as_deref())),
            Self::Module { .. } => None,
        }
    }

    fn value_context<'a>(
        &'a self,
        crate_id: &'a str,
        module_path: &str,
    ) -> (&'a str, Option<&'a str>) {
        match self {
            Self::Module { .. } if module_path == "crate" => (crate_id, None),
            Self::Module { module_id } => (module_id, None),
            Self::Trait { owner_id } => (owner_id, Some(owner_id)),
            Self::Implementation {
                owner_id,
                trait_context_id,
            } => (owner_id, trait_context_id.as_deref()),
        }
    }
}

struct ChunkBuilder<'a> {
    repository_identity: &'a str,
    commit_oid: &'a str,
    crate_id: &'a str,
    source_file_id: &'a str,
    path: &'a str,
    blob_oid: &'a str,
    source: &'a str,
    catalog: &'a ExistingCatalog,
    entities: BTreeMap<String, CallableSemanticEntity>,
    relationships: BTreeMap<String, CallableRelationship>,
    evidence: BTreeMap<String, WorkspaceEvidence>,
    diagnostics: BTreeMap<String, CallableDiagnostic>,
    coverage: BTreeMap<String, CallableCoverageGap>,
    callable_count: usize,
}

impl<'a> ChunkBuilder<'a> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        repository_identity: &'a str,
        commit_oid: &'a str,
        crate_id: &'a str,
        source_file_id: &'a str,
        path: &'a str,
        blob_oid: &'a str,
        source: &'a str,
        catalog: &'a ExistingCatalog,
    ) -> Self {
        Self {
            repository_identity,
            commit_oid,
            crate_id,
            source_file_id,
            path,
            blob_oid,
            source,
            catalog,
            entities: BTreeMap::new(),
            relationships: BTreeMap::new(),
            evidence: BTreeMap::new(),
            diagnostics: BTreeMap::new(),
            coverage: BTreeMap::new(),
            callable_count: 0,
        }
    }

    fn add_evidence(&mut self, node: Node<'_>) -> Result<String, CallableSemanticsError> {
        self.add_evidence_range(node.start_byte(), node.end_byte())
    }

    fn add_evidence_range(
        &mut self,
        start: usize,
        end: usize,
    ) -> Result<String, CallableSemanticsError> {
        if start >= end || end > self.source.len() {
            return Err(invalid_syntax(self.path, start, "invalid_span"));
        }
        let start_byte =
            u64::try_from(start).map_err(|_| CallableSemanticsError::ContractInvalid)?;
        let end_byte = u64::try_from(end).map_err(|_| CallableSemanticsError::ContractInvalid)?;
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

    fn insert_entity(
        &mut self,
        entity: CallableSemanticEntity,
    ) -> Result<(), CallableSemanticsError> {
        if self.entities.insert(entity.id.clone(), entity).is_some() {
            return Err(CallableSemanticsError::ContractInvalid);
        }
        Ok(())
    }

    fn add_relationship(&mut self, relationship: CallableRelationship) {
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
        state: CallableCoverageState,
        subject_id: &str,
        evidence_id: &str,
    ) {
        let gap = CallableCoverageGap::new(
            capability,
            state,
            subject_id.to_owned(),
            vec![evidence_id.to_owned()],
        );
        self.coverage.entry(gap.id.clone()).or_insert(gap);
    }

    fn add_unresolved_call(&mut self, subject_id: &str, evidence_id: &str) {
        let diagnostic = CallableDiagnostic::new(
            "rust.call_target_candidate_unresolved",
            "Call syntax does not prove one unique local free-function target",
            subject_id.to_owned(),
            vec![evidence_id.to_owned()],
        );
        self.diagnostics
            .entry(diagnostic.id.clone())
            .or_insert(diagnostic);
        self.add_gap(
            "rust.call_target_resolution",
            CallableCoverageState::NotResolved,
            subject_id,
            evidence_id,
        );
    }

    fn add_callable_gaps(&mut self, signature_id: &str, evidence_id: &str) {
        for (capability, state) in [
            (
                "rust.compiler_cfg_not_computed",
                CallableCoverageState::NotAnalyzed,
            ),
            (
                "rust.reachability_not_computed",
                CallableCoverageState::NotAnalyzed,
            ),
            (
                "rust.data_flow_not_computed",
                CallableCoverageState::NotAnalyzed,
            ),
            (
                "rust.ownership_flow_not_computed",
                CallableCoverageState::NotAnalyzed,
            ),
            (
                "rust.side_effects_not_computed",
                CallableCoverageState::NotAnalyzed,
            ),
            (
                "rust.runtime_behavior_not_observed",
                CallableCoverageState::NotObserved,
            ),
        ] {
            self.add_gap(capability, state, signature_id, evidence_id);
        }
    }

    fn finish(self) -> CallableSourceChunk {
        let entities = self.entities.into_values().collect::<Vec<_>>();
        let relationships = self.relationships.into_values().collect::<Vec<_>>();
        let mut claims = entities
            .iter()
            .map(|entity| {
                let state = match &entity.properties {
                    CallableSemanticProperties::CallSite(properties)
                        if properties.resolution_state
                            == CallResolutionState::CandidateUnresolved =>
                    {
                        ClaimState::Candidate
                    }
                    _ => ClaimState::DeterministicFact,
                };
                callable_claim(
                    ClaimSubjectKind::Entity,
                    entity.id.clone(),
                    entity.evidence_ids[0].clone(),
                    state,
                )
            })
            .chain(relationships.iter().map(|relationship| {
                callable_claim(
                    ClaimSubjectKind::Relationship,
                    relationship.id.clone(),
                    relationship.evidence_ids[0].clone(),
                    ClaimState::DeterministicFact,
                )
            }))
            .collect::<Vec<_>>();
        claims.sort_by(|left, right| left.id.cmp(&right.id));
        CallableSourceChunk {
            crate_id: self.crate_id.to_owned(),
            source_file_id: self.source_file_id.to_owned(),
            path: self.path.to_owned(),
            entities,
            relationships,
            claims,
            evidence: self.evidence.into_values().collect(),
            diagnostics: self.diagnostics.into_values().collect(),
            coverage: self.coverage.into_values().collect(),
        }
    }
}

impl TreeSitterRustWorkspaceExtractor {
    /// Extracts the explicit K1 source-only callable semantics profile.
    ///
    /// # Errors
    ///
    /// Returns the inherited R6 failure or a typed K1 syntax, identity, limit,
    /// composition, or graph contract failure.
    pub fn extract_rust_callable_semantics(
        &self,
        inventory: &RepositoryInventory,
    ) -> Result<CallableSemanticsExtraction, CallableSemanticsError> {
        let framework = self
            .extract_rust_framework_declarations_incremental(inventory, &[], &[])
            .map_err(CallableSemanticsError::Source)?;
        let catalog = ExistingCatalog::from_extraction(&framework);
        let contexts = source_contexts(&framework.knowledge.semantic.manifest, inventory)
            .map_err(|_| CallableSemanticsError::ContractInvalid)?;
        let repository_identity = inventory.bound_revision().repository_identity().as_str();
        let commit_oid = inventory.bound_revision().commit_oid().as_str();
        let mut chunks = Vec::with_capacity(contexts.len());
        for context in contexts {
            let text =
                source_text(&context).map_err(|_| CallableSemanticsError::ContractInvalid)?;
            let tree = parse_tree(&context.path, text)
                .map_err(|_| invalid_syntax(&context.path, 0, "source_file"))?;
            let mut builder = ChunkBuilder::new(
                repository_identity,
                commit_oid,
                &context.crate_id,
                &context.source_file_id,
                &context.path,
                context.file.blob_oid().as_str(),
                text,
                &catalog,
            );
            process_scope(
                tree.root_node(),
                &context.base_module_path,
                &ScopeOwner::Module {
                    module_id: context.base_module_id.clone(),
                },
                &mut builder,
            )?;
            enforce_limit(
                CallableSemanticsLimit::CallablesPerSource,
                builder.callable_count,
            )?;
            chunks.push(builder.finish());
        }
        let graph = aggregate_graph(&chunks)?;
        let parser_invocation_count = u64::try_from(chunks.len()).unwrap_or(u64::MAX);
        let extraction =
            CallableSemanticsExtraction::from_r6(framework, chunks, graph, parser_invocation_count);
        extraction.knowledge.validate()?;
        Ok(extraction)
    }
}

fn process_scope(
    scope: Node<'_>,
    module_path: &str,
    owner: &ScopeOwner,
    builder: &mut ChunkBuilder<'_>,
) -> Result<(), CallableSemanticsError> {
    let mut cursor = scope.walk();
    for node in scope.named_children(&mut cursor) {
        match node.kind() {
            "mod_item" => {
                let name = normalized_name(node, builder.source, builder.path)?;
                if let Some(body) = node.child_by_field_name("body") {
                    let child_path = child_module_path(module_path, &name);
                    let module_id = workspace_module_id(
                        builder.repository_identity,
                        builder.crate_id,
                        &child_path,
                    );
                    process_scope(
                        body,
                        &child_path,
                        &ScopeOwner::Module { module_id },
                        builder,
                    )?;
                }
            }
            "function_item" if matches!(owner, ScopeOwner::Module { .. }) => {
                process_callable(node, module_path, owner, builder)?;
            }
            "trait_item" => {
                let name = normalized_name(node, builder.source, builder.path)?;
                let trait_id = builder
                    .catalog
                    .owner(builder.crate_id, module_path, EntityKind::RustTrait, &name)
                    .ok_or(CallableSemanticsError::ContractInvalid)?
                    .to_owned();
                if let Some(body) = node.child_by_field_name("body") {
                    process_scope(
                        body,
                        module_path,
                        &ScopeOwner::Trait { owner_id: trait_id },
                        builder,
                    )?;
                }
            }
            "function_signature_item" | "function_item"
                if !matches!(owner, ScopeOwner::Module { .. }) =>
            {
                process_callable(node, module_path, owner, builder)?;
            }
            "impl_item" => process_implementation(node, module_path, builder)?,
            "enum_item" => process_enum_values(node, module_path, builder)?,
            "const_item" => process_declared_value(
                node,
                module_path,
                owner,
                RustSemanticEntityKind::Constant,
                builder,
            )?,
            "static_item" => process_declared_value(
                node,
                module_path,
                owner,
                RustSemanticEntityKind::Static,
                builder,
            )?,
            _ => {}
        }
    }
    Ok(())
}

fn process_implementation(
    node: Node<'_>,
    module_path: &str,
    builder: &mut ChunkBuilder<'_>,
) -> Result<(), CallableSemanticsError> {
    if node.child_by_field_name("type_parameters").is_some() {
        return Ok(());
    }
    let target = node
        .child_by_field_name("type")
        .ok_or_else(|| invalid_syntax(builder.path, node.start_byte(), node.kind()))?;
    let target_id = builder
        .catalog
        .resolve_owner(
            builder.crate_id,
            module_path,
            node_text(target, builder.source),
            &[EntityKind::RustStruct, EntityKind::RustEnum],
        )
        .ok_or(CallableSemanticsError::ContractInvalid)?
        .to_owned();
    let trait_context_id = node
        .child_by_field_name("trait")
        .and_then(|trait_node| {
            builder.catalog.resolve_owner(
                builder.crate_id,
                module_path,
                node_text(trait_node, builder.source),
                &[EntityKind::RustTrait],
            )
        })
        .map(str::to_owned);
    if let Some(body) = node.child_by_field_name("body") {
        process_scope(
            body,
            module_path,
            &ScopeOwner::Implementation {
                owner_id: target_id,
                trait_context_id,
            },
            builder,
        )?;
    }
    Ok(())
}

fn process_enum_values(
    node: Node<'_>,
    module_path: &str,
    builder: &mut ChunkBuilder<'_>,
) -> Result<(), CallableSemanticsError> {
    let name = normalized_name(node, builder.source, builder.path)?;
    let owner_id = builder
        .catalog
        .owner(builder.crate_id, module_path, EntityKind::RustEnum, &name)
        .ok_or(CallableSemanticsError::ContractInvalid)?
        .to_owned();
    let body = node
        .child_by_field_name("body")
        .ok_or_else(|| invalid_syntax(builder.path, node.start_byte(), node.kind()))?;
    let mut cursor = body.walk();
    for variant in body.named_children(&mut cursor) {
        if variant.kind() != "enum_variant" {
            continue;
        }
        process_value_node(
            variant,
            module_path,
            &owner_id,
            None,
            RustSemanticEntityKind::EnumVariant,
            builder,
        )?;
    }
    Ok(())
}

fn process_declared_value(
    node: Node<'_>,
    module_path: &str,
    owner: &ScopeOwner,
    kind: RustSemanticEntityKind,
    builder: &mut ChunkBuilder<'_>,
) -> Result<(), CallableSemanticsError> {
    let (owner_id, trait_context_id) = owner.value_context(builder.crate_id, module_path);
    process_value_node(node, module_path, owner_id, trait_context_id, kind, builder)
}

fn process_value_node(
    node: Node<'_>,
    module_path: &str,
    owner_id: &str,
    trait_context_id: Option<&str>,
    kind: RustSemanticEntityKind,
    builder: &mut ChunkBuilder<'_>,
) -> Result<(), CallableSemanticsError> {
    let name = normalized_name(node, builder.source, builder.path)?;
    let declaration_id = rust_semantic_member_id(
        builder.repository_identity,
        builder.crate_id,
        owner_id,
        kind,
        &name,
        trait_context_id,
    );
    if !builder.catalog.semantic_ids.contains(&declaration_id) {
        return Err(CallableSemanticsError::ContractInvalid);
    }
    let value = node.child_by_field_name("value");
    let evidence_id = match value {
        Some(value_node) => builder.add_evidence(value_node)?,
        None => builder.add_evidence(node)?,
    };
    let (state, syntax_kind, expression_digest, expression_byte_length, normalized) =
        if let Some(value_node) = value {
            let text = node_text(value_node, builder.source).trim();
            enforce_limit(CallableSemanticsLimit::ExpressionMetadataBytes, text.len())?;
            let normalized = normalized_scalar(text);
            (
                if normalized.is_some() {
                    DeclaredValueState::NormalizedScalar
                } else {
                    DeclaredValueState::ExpressionOnly
                },
                Some(value_node.kind().to_owned()),
                Some(k1_digest(text.as_bytes())),
                u64::try_from(text.len()).unwrap_or(u64::MAX),
                normalized,
            )
        } else {
            (DeclaredValueState::Unresolved, None, None, 0, None)
        };
    let id = declared_value_id(builder.repository_identity, &declaration_id);
    let entity = CallableSemanticEntity {
        id: id.clone(),
        kind: CallableSemanticEntityKind::DeclaredValue,
        crate_id: builder.crate_id.to_owned(),
        module_path: module_path.to_owned(),
        name,
        subject_id: declaration_id.clone(),
        ordinal: None,
        evidence_ids: vec![evidence_id.clone()],
        properties: CallableSemanticProperties::DeclaredValue(DeclaredValueProperties {
            state,
            syntax_kind,
            expression_digest,
            expression_byte_length,
            normalized,
        }),
    };
    builder.insert_entity(entity)?;
    builder.add_relationship(CallableRelationship::new(
        CallableRelationshipKind::DeclaresValue,
        declaration_id,
        id.clone(),
        vec![evidence_id.clone()],
    ));
    match state {
        DeclaredValueState::NormalizedScalar => {}
        DeclaredValueState::ExpressionOnly => builder.add_gap(
            "rust.scalar_value_not_normalized",
            CallableCoverageState::NotResolved,
            &id,
            &evidence_id,
        ),
        DeclaredValueState::Unresolved => builder.add_gap(
            "rust.declared_value_not_explicit",
            CallableCoverageState::NotResolved,
            &id,
            &evidence_id,
        ),
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn process_callable(
    node: Node<'_>,
    module_path: &str,
    owner: &ScopeOwner,
    builder: &mut ChunkBuilder<'_>,
) -> Result<(), CallableSemanticsError> {
    builder.callable_count = builder
        .callable_count
        .checked_add(1)
        .ok_or(CallableSemanticsError::ContractInvalid)?;
    let name = normalized_name(node, builder.source, builder.path)?;
    let callable_id = if let Some((owner_id, trait_context_id)) = owner.method_context() {
        rust_semantic_member_id(
            builder.repository_identity,
            builder.crate_id,
            owner_id,
            RustSemanticEntityKind::Method,
            &name,
            trait_context_id,
        )
    } else {
        workspace_declaration_id(
            builder.repository_identity,
            EntityKind::RustFunction,
            builder.crate_id,
            module_path,
            &name,
        )
    };
    let callable_known = builder.catalog.semantic_ids.contains(&callable_id)
        || builder
            .catalog
            .free_functions
            .values()
            .any(|value| value == &callable_id);
    if !callable_known {
        return Err(CallableSemanticsError::ContractInvalid);
    }
    let body = node.child_by_field_name("body");
    let header_end = body.map_or(node.end_byte(), |value| value.start_byte());
    let header_evidence_id = builder.add_evidence_range(node.start_byte(), header_end)?;
    let header = builder
        .source
        .get(node.start_byte()..header_end)
        .ok_or(CallableSemanticsError::ContractInvalid)?
        .trim();
    let body_evidence_id = body.map(|value| builder.add_evidence(value)).transpose()?;
    let body_digest = body.map(|value| k1_digest(node_text(value, builder.source).as_bytes()));
    let (return_state, return_type) = node.child_by_field_name("return_type").map_or(
        (CallableReturnState::UnitDefault, None),
        |return_node| {
            let text = node_text(return_node, builder.source)
                .trim()
                .trim_start_matches("->")
                .trim();
            (CallableReturnState::Declared, Some(text.to_owned()))
        },
    );
    let generic_parameters = optional_bounded_field(node, "type_parameters", builder)?;
    let where_clause = optional_named_child(node, "where_clause")
        .map(|value| bounded_text(value, builder))
        .transpose()?;
    if let Some(value) = return_type.as_ref() {
        enforce_limit(CallableSemanticsLimit::SignatureComponentBytes, value.len())?;
    }
    let signature_id = callable_signature_id(builder.repository_identity, &callable_id);
    let mut signature_evidence = vec![header_evidence_id.clone()];
    if let Some(value) = body_evidence_id.as_ref() {
        signature_evidence.push(value.clone());
    }
    builder.insert_entity(CallableSemanticEntity {
        id: signature_id.clone(),
        kind: CallableSemanticEntityKind::Signature,
        crate_id: builder.crate_id.to_owned(),
        module_path: module_path.to_owned(),
        name: name.clone(),
        subject_id: callable_id.clone(),
        ordinal: None,
        evidence_ids: signature_evidence,
        properties: CallableSemanticProperties::Signature(CallableSignatureProperties {
            visibility: callable_visibility(header, owner),
            is_async: contains_keyword(header, "async"),
            is_const: contains_keyword(header, "const"),
            is_unsafe: contains_keyword(header, "unsafe"),
            abi: literal_abi(header),
            generic_parameters,
            where_clause,
            return_state,
            return_type,
            body_state: if body.is_some() {
                CallableBodyState::Present
            } else {
                CallableBodyState::Absent
            },
            body_digest,
            body_evidence_id,
        }),
    })?;
    builder.add_relationship(CallableRelationship::new(
        CallableRelationshipKind::HasSignature,
        callable_id.clone(),
        signature_id.clone(),
        vec![header_evidence_id.clone()],
    ));
    process_parameters(node, module_path, &callable_id, &signature_id, builder)?;
    builder.add_callable_gaps(&signature_id, &header_evidence_id);
    if let Some(body) = body {
        let mut ordinal = 0_u64;
        walk_body(
            body,
            module_path,
            &callable_id,
            0,
            None,
            &mut ordinal,
            builder,
        )?;
        let observed = usize::try_from(ordinal).unwrap_or(usize::MAX);
        enforce_limit(CallableSemanticsLimit::BodyFactsPerCallable, observed)?;
    }
    Ok(())
}

fn process_parameters(
    node: Node<'_>,
    module_path: &str,
    callable_id: &str,
    signature_id: &str,
    builder: &mut ChunkBuilder<'_>,
) -> Result<(), CallableSemanticsError> {
    let parameters = node
        .child_by_field_name("parameters")
        .ok_or_else(|| invalid_syntax(builder.path, node.start_byte(), node.kind()))?;
    let mut values = Vec::new();
    let mut cursor = parameters.walk();
    for parameter in parameters.named_children(&mut cursor) {
        if !matches!(parameter.kind(), "parameter" | "self_parameter") {
            continue;
        }
        let text = node_text(parameter, builder.source).trim();
        let (pattern, declared_type, receiver_state) = if parameter.kind() == "self_parameter" {
            let (pattern, declared_type) =
                text.split_once(':')
                    .map_or((text.to_owned(), None), |(pattern, declared_type)| {
                        (
                            pattern.trim().to_owned(),
                            Some(declared_type.trim().to_owned()),
                        )
                    });
            (
                normalize_syntax(&pattern),
                declared_type.map(|value| normalize_syntax(&value)),
                receiver_state(text),
            )
        } else {
            let pattern_node = parameter
                .child_by_field_name("pattern")
                .or_else(|| parameter.named_child(0))
                .ok_or_else(|| {
                    invalid_syntax(builder.path, parameter.start_byte(), parameter.kind())
                })?;
            let type_node = parameter.child_by_field_name("type").ok_or_else(|| {
                invalid_syntax(builder.path, parameter.start_byte(), parameter.kind())
            })?;
            (
                normalize_syntax(node_text(pattern_node, builder.source).trim()),
                Some(normalize_syntax(
                    node_text(type_node, builder.source).trim(),
                )),
                CallableReceiverState::None,
            )
        };
        enforce_limit(
            CallableSemanticsLimit::SignatureComponentBytes,
            pattern.len(),
        )?;
        if let Some(value) = declared_type.as_ref() {
            enforce_limit(CallableSemanticsLimit::SignatureComponentBytes, value.len())?;
        }
        values.push((parameter, pattern, declared_type, receiver_state));
    }
    enforce_limit(CallableSemanticsLimit::ParametersPerCallable, values.len())?;
    for (index, (parameter, pattern, declared_type, receiver_state)) in
        values.into_iter().enumerate()
    {
        let ordinal = u64::try_from(index).map_err(|_| CallableSemanticsError::ContractInvalid)?;
        let evidence_id = builder.add_evidence(parameter)?;
        let id = callable_parameter_id(builder.repository_identity, callable_id, ordinal, &pattern);
        builder.insert_entity(CallableSemanticEntity {
            id: id.clone(),
            kind: CallableSemanticEntityKind::Parameter,
            crate_id: builder.crate_id.to_owned(),
            module_path: module_path.to_owned(),
            name: pattern.clone(),
            subject_id: callable_id.to_owned(),
            ordinal: Some(ordinal),
            evidence_ids: vec![evidence_id.clone()],
            properties: CallableSemanticProperties::Parameter(CallableParameterProperties {
                pattern,
                declared_type,
                receiver_state,
            }),
        })?;
        builder.add_relationship(CallableRelationship::new(
            CallableRelationshipKind::HasParameter,
            signature_id.to_owned(),
            id,
            vec![evidence_id],
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn walk_body(
    node: Node<'_>,
    module_path: &str,
    callable_id: &str,
    lexical_depth: u64,
    parent_fact_id: Option<&str>,
    ordinal: &mut u64,
    builder: &mut ChunkBuilder<'_>,
) -> Result<(), CallableSemanticsError> {
    if lexical_depth > codenoesis_domain::s4_k1::MAX_K1_BODY_FACT_LEXICAL_DEPTH {
        return Err(CallableSemanticsError::LimitExceeded {
            limit: CallableSemanticsLimit::BodyFactLexicalDepth,
            maximum: codenoesis_domain::s4_k1::MAX_K1_BODY_FACT_LEXICAL_DEPTH,
            observed: lexical_depth,
        });
    }
    if matches!(
        node.kind(),
        "macro_invocation" | "macro_definition" | "closure_expression" | "function_item"
    ) {
        return Ok(());
    }
    let mut next_parent = parent_fact_id.map(str::to_owned);
    let mut next_depth = lexical_depth;
    let fact = match node.kind() {
        "let_declaration" => {
            let pattern_node = node
                .child_by_field_name("pattern")
                .or_else(|| node.named_child(0))
                .ok_or_else(|| invalid_syntax(builder.path, node.start_byte(), node.kind()))?;
            let pattern = normalize_syntax(node_text(pattern_node, builder.source).trim());
            let declared_type = node
                .child_by_field_name("type")
                .map(|value| normalize_syntax(node_text(value, builder.source).trim()));
            Some((
                CallableSemanticEntityKind::LocalBinding,
                pattern.clone(),
                CallableSemanticProperties::LocalBinding(LocalBindingProperties {
                    pattern,
                    declared_type,
                    initializer_present: node.child_by_field_name("value").is_some(),
                    lexical_depth,
                    parent_fact_id: parent_fact_id.map(str::to_owned),
                }),
            ))
        }
        "call_expression" => {
            let function = node
                .child_by_field_name("function")
                .or_else(|| node.named_child(0))
                .ok_or_else(|| invalid_syntax(builder.path, node.start_byte(), node.kind()))?;
            let spelling = normalize_syntax(node_text(function, builder.source).trim());
            enforce_limit(
                CallableSemanticsLimit::SignatureComponentBytes,
                spelling.len(),
            )?;
            let form = if function.kind() == "field_expression" {
                CallForm::Method
            } else {
                CallForm::Direct
            };
            if form == CallForm::Direct && is_constructor_spelling(&spelling) {
                None
            } else {
                let target = (form == CallForm::Direct)
                    .then(|| {
                        builder.catalog.resolve_free_function(
                            builder.crate_id,
                            module_path,
                            &spelling,
                        )
                    })
                    .flatten()
                    .map(str::to_owned);
                let state = if target.is_some() {
                    CallResolutionState::ResolvedUniqueLocal
                } else {
                    CallResolutionState::CandidateUnresolved
                };
                Some((
                    CallableSemanticEntityKind::CallSite,
                    spelling.clone(),
                    CallableSemanticProperties::CallSite(CallSiteProperties {
                        form,
                        target_spelling: spelling,
                        resolution_state: state,
                        resolved_target_id: target,
                        lexical_depth,
                        parent_fact_id: parent_fact_id.map(str::to_owned),
                    }),
                ))
            }
        }
        kind if control_kind(kind, node, builder.source).is_some() => {
            let control_kind = control_kind(kind, node, builder.source)
                .ok_or(CallableSemanticsError::ContractInvalid)?;
            Some((
                CallableSemanticEntityKind::Control,
                control_kind.as_str().to_owned(),
                CallableSemanticProperties::Control(ControlProperties {
                    control_kind,
                    lexical_depth,
                    parent_fact_id: parent_fact_id.map(str::to_owned),
                }),
            ))
        }
        _ => None,
    };
    if let Some((kind, name, properties)) = fact {
        let evidence_id = builder.add_evidence(node)?;
        let id =
            callable_body_fact_id(builder.repository_identity, callable_id, kind, &evidence_id);
        let current_ordinal = *ordinal;
        *ordinal = ordinal
            .checked_add(1)
            .ok_or(CallableSemanticsError::ContractInvalid)?;
        builder.insert_entity(CallableSemanticEntity {
            id: id.clone(),
            kind,
            crate_id: builder.crate_id.to_owned(),
            module_path: module_path.to_owned(),
            name,
            subject_id: callable_id.to_owned(),
            ordinal: Some(current_ordinal),
            evidence_ids: vec![evidence_id.clone()],
            properties,
        })?;
        builder.add_relationship(CallableRelationship::new(
            CallableRelationshipKind::HasBodyFact,
            callable_id.to_owned(),
            id.clone(),
            vec![evidence_id.clone()],
        ));
        if let Some(CallableSemanticProperties::CallSite(properties)) =
            builder.entities.get(&id).map(|value| &value.properties)
        {
            if let Some(target) = properties.resolved_target_id.as_ref() {
                builder.add_relationship(CallableRelationship::new(
                    CallableRelationshipKind::Calls,
                    callable_id.to_owned(),
                    target.clone(),
                    vec![evidence_id.clone()],
                ));
            } else {
                builder.add_unresolved_call(&id, &evidence_id);
            }
        }
        next_parent = Some(id);
        next_depth = lexical_depth.saturating_add(1);
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        walk_body(
            child,
            module_path,
            callable_id,
            next_depth,
            next_parent.as_deref(),
            ordinal,
            builder,
        )?;
    }
    Ok(())
}

fn aggregate_graph(
    chunks: &[CallableSourceChunk],
) -> Result<CallableSemanticsGraph, CallableSemanticsError> {
    let mut entities = BTreeMap::new();
    let mut relationships = BTreeMap::new();
    let mut claims = BTreeMap::new();
    let mut evidence = BTreeMap::new();
    let mut diagnostics = BTreeMap::new();
    let mut coverage = BTreeMap::new();
    for chunk in chunks {
        extend_unique(&mut entities, &chunk.entities, |value| &value.id)?;
        for relationship in &chunk.relationships {
            relationships
                .entry(relationship.id.clone())
                .and_modify(|existing: &mut CallableRelationship| {
                    existing
                        .evidence_ids
                        .extend(relationship.evidence_ids.clone());
                    existing.evidence_ids.sort();
                    existing.evidence_ids.dedup();
                })
                .or_insert_with(|| relationship.clone());
        }
        extend_unique(&mut claims, &chunk.claims, |value| &value.id)?;
        extend_equal(&mut evidence, &chunk.evidence, |value| &value.id)?;
        extend_equal(&mut diagnostics, &chunk.diagnostics, |value| &value.id)?;
        extend_equal(&mut coverage, &chunk.coverage, |value| &value.id)?;
    }
    let entities = entities.into_values().collect::<Vec<_>>();
    let relationships = relationships.into_values().collect::<Vec<_>>();
    let index = CallableSemanticsIndex::from_graph(&entities, &relationships);
    Ok(CallableSemanticsGraph {
        entities,
        relationships,
        claims: claims.into_values().collect(),
        evidence: evidence.into_values().collect(),
        diagnostics: diagnostics.into_values().collect(),
        coverage: coverage.into_values().collect(),
        index,
    })
}

fn extend_unique<T: Clone>(
    target: &mut BTreeMap<String, T>,
    values: &[T],
    identifier: impl Fn(&T) -> &String,
) -> Result<(), CallableSemanticsError> {
    for value in values {
        if target
            .insert(identifier(value).clone(), value.clone())
            .is_some()
        {
            return Err(CallableSemanticsError::ContractInvalid);
        }
    }
    Ok(())
}

fn extend_equal<T: Clone + Eq>(
    target: &mut BTreeMap<String, T>,
    values: &[T],
    identifier: impl Fn(&T) -> &String,
) -> Result<(), CallableSemanticsError> {
    for value in values {
        if let Some(existing) = target.insert(identifier(value).clone(), value.clone())
            && existing != *value
        {
            return Err(CallableSemanticsError::ContractInvalid);
        }
    }
    Ok(())
}

fn normalized_scalar(text: &str) -> Option<NormalizedScalarValue> {
    match text {
        "true" => return Some(NormalizedScalarValue::Boolean(true)),
        "false" => return Some(NormalizedScalarValue::Boolean(false)),
        _ => {}
    }
    if let Some(value) = decode_quoted(text, '\'') {
        return (value.chars().count() == 1).then_some(NormalizedScalarValue::Character(value));
    }
    if let Some(value) = decode_quoted(text, '"') {
        return Some(NormalizedScalarValue::String(value));
    }
    normalized_integer(text)
}

fn is_constructor_spelling(spelling: &str) -> bool {
    spelling
        .rsplit("::")
        .next()
        .and_then(|segment| segment.chars().next())
        .is_some_and(char::is_uppercase)
}

fn normalized_integer(text: &str) -> Option<NormalizedScalarValue> {
    let (sign, unsigned) = text
        .strip_prefix('-')
        .map_or(("positive", text), |value| ("negative", value));
    let suffixes = [
        "usize", "isize", "u128", "i128", "u64", "i64", "u32", "i32", "u16", "i16", "u8", "i8",
    ];
    let (literal, suffix) = suffixes
        .iter()
        .find_map(|suffix| {
            unsigned
                .strip_suffix(suffix)
                .map(|literal| (literal.trim_end_matches('_'), Some((*suffix).to_owned())))
        })
        .unwrap_or((unsigned, None));
    let (radix, digits) = if let Some(value) = literal.strip_prefix("0x") {
        (16, value)
    } else if let Some(value) = literal.strip_prefix("0o") {
        (8, value)
    } else if let Some(value) = literal.strip_prefix("0b") {
        (2, value)
    } else {
        (10, literal)
    };
    let digits = digits.replace('_', "");
    if digits.is_empty() || !digits.chars().all(|character| character.is_digit(radix)) {
        return None;
    }
    Some(NormalizedScalarValue::Integer {
        sign: sign.to_owned(),
        radix,
        digits: digits.to_ascii_lowercase(),
        suffix,
    })
}

fn decode_quoted(text: &str, quote: char) -> Option<String> {
    let inner = text.strip_prefix(quote)?.strip_suffix(quote)?;
    let mut result = String::new();
    let mut characters = inner.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            result.push(character);
            continue;
        }
        result.push(match characters.next()? {
            '\\' => '\\',
            '"' => '"',
            '\'' => '\'',
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            '0' => '\0',
            _ => return None,
        });
    }
    Some(result)
}

fn control_kind(kind: &str, node: Node<'_>, source: &str) -> Option<ControlKind> {
    match kind {
        "if_expression" => Some(if has_named_descendant(node, "let_condition") {
            ControlKind::IfLet
        } else {
            ControlKind::If
        }),
        "match_expression" => Some(ControlKind::Match),
        "loop_expression" => Some(ControlKind::Loop),
        "while_expression" => Some(
            if has_named_descendant(node, "let_condition")
                || node_text(node, source)
                    .trim_start()
                    .starts_with("while let ")
            {
                ControlKind::WhileLet
            } else {
                ControlKind::While
            },
        ),
        "for_expression" => Some(ControlKind::For),
        "return_expression" => Some(ControlKind::Return),
        "break_expression" => Some(ControlKind::Break),
        "continue_expression" => Some(ControlKind::Continue),
        "try_expression" => Some(ControlKind::Try),
        _ => None,
    }
}

fn has_named_descendant(node: Node<'_>, kind: &str) -> bool {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .any(|child| child.kind() == kind || has_named_descendant(child, kind))
}

fn callable_visibility(header: &str, owner: &ScopeOwner) -> String {
    if matches!(owner, ScopeOwner::Trait { .. })
        || matches!(
            owner,
            ScopeOwner::Implementation {
                trait_context_id: Some(_),
                ..
            }
        )
    {
        return "inherited_trait".to_owned();
    }
    let value = header.trim_start();
    if value.starts_with("pub(crate)") {
        "crate".to_owned()
    } else if value.starts_with("pub(") {
        "restricted".to_owned()
    } else if value.starts_with("pub ") {
        "public".to_owned()
    } else {
        "private".to_owned()
    }
}

fn receiver_state(text: &str) -> CallableReceiverState {
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.contains("self:") || compact.contains("self :") {
        CallableReceiverState::TypedSelf
    } else if compact.contains("&mut self") {
        CallableReceiverState::RefMut
    } else if compact.contains("&self") {
        CallableReceiverState::Ref
    } else if compact == "self" || compact == "mut self" {
        CallableReceiverState::Value
    } else {
        CallableReceiverState::Explicit
    }
}

fn literal_abi(header: &str) -> Option<String> {
    let (_, remainder) = header.split_once("extern")?;
    let remainder = remainder.trim_start();
    if !remainder.starts_with('"') {
        return Some("C".to_owned());
    }
    let end = remainder.get(1..)?.find('"')?;
    Some(remainder.get(1..end + 1)?.to_owned())
}

fn contains_keyword(value: &str, keyword: &str) -> bool {
    value
        .split(|character: char| !character.is_alphanumeric() && character != '_')
        .any(|token| token == keyword)
}

fn optional_bounded_field(
    node: Node<'_>,
    field: &str,
    builder: &ChunkBuilder<'_>,
) -> Result<Option<String>, CallableSemanticsError> {
    node.child_by_field_name(field)
        .map(|value| bounded_text(value, builder))
        .transpose()
}

fn optional_named_child<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() == kind)
}

fn bounded_text(
    node: Node<'_>,
    builder: &ChunkBuilder<'_>,
) -> Result<String, CallableSemanticsError> {
    let text = normalize_syntax(node_text(node, builder.source).trim());
    enforce_limit(CallableSemanticsLimit::SignatureComponentBytes, text.len())?;
    Ok(text)
}

fn normalized_name(
    node: Node<'_>,
    source: &str,
    path: &str,
) -> Result<String, CallableSemanticsError> {
    node.child_by_field_name("name")
        .map(|value| normalize_identifier(node_text(value, source)))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid_syntax(path, node.start_byte(), node.kind()))
}

fn normalize_identifier(value: &str) -> String {
    value.strip_prefix("r#").unwrap_or(value).nfc().collect()
}

fn normalize_syntax(value: &str) -> String {
    value.nfc().collect()
}

fn node_text<'a>(node: Node<'_>, source: &'a str) -> &'a str {
    node.utf8_text(source.as_bytes()).unwrap_or("")
}

fn child_module_path(parent: &str, name: &str) -> String {
    if parent == "crate" {
        format!("crate::{name}")
    } else {
        format!("{parent}::{name}")
    }
}

fn local_path(current_module: &str, text: &str) -> Option<(String, String)> {
    let normalized = text
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
    let prefix = &normalized[..normalized.len() - 1];
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
    } else {
        std::iter::once(current_module)
            .chain(prefix.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join("::")
    };
    Some((module, name))
}

fn invalid_syntax(path: &str, start: usize, syntax_kind: &str) -> CallableSemanticsError {
    CallableSemanticsError::InvalidSyntax {
        path: path.to_owned(),
        start_byte: u64::try_from(start).unwrap_or(u64::MAX),
        syntax_kind: syntax_kind.to_owned(),
    }
}
