use std::collections::{BTreeMap, BTreeSet};

use codenoesis_domain::knowledge::{
    ByteSpan, ClaimSubjectKind, CoverageGap, EntityKind, EntityProperties, ExtractionChunk,
    ExtractionCoverage, ExtractionDiagnostic, KnowledgeClaim, KnowledgeEntity, KnowledgeError,
    KnowledgeGraph, KnowledgeRelationship, MAX_S2_COVERAGE_GAPS, MAX_S2_DIAGNOSTICS,
    MAX_S2_ENTITIES, MAX_S2_EVIDENCE, MAX_S2_RELATIONSHIPS, RelationshipKind, RustKnowledge,
    SourceEvidence, extraction_chunk_id,
};
use codenoesis_domain::{
    ContentKind, InventoryLanguage, RepositoryInventory, STANDARD_LOCAL_S1_LIMITS,
};
use codenoesis_ports::RustKnowledgeExtractor;
use tree_sitter::{Node, Parser};
use unicode_normalization::UnicodeNormalization as _;

mod framework_declarations;
mod manifest_facts;
mod root_package;
mod semantic_depth;
mod workspace;

pub use workspace::TreeSitterRustWorkspaceExtractor;

#[derive(Clone, Copy, Debug, Default)]
pub struct TreeSitterRustExtractor;

impl TreeSitterRustExtractor {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl RustKnowledgeExtractor for TreeSitterRustExtractor {
    fn extract(&self, inventory: &RepositoryInventory) -> Result<RustKnowledge, KnowledgeError> {
        let rust_files = inventory
            .files()
            .iter()
            .filter(|file| {
                file.path().as_bytes().ends_with(b".rs")
                    && file.languages().contains(&InventoryLanguage::Rust)
            })
            .collect::<Vec<_>>();
        if rust_files.len() != 1 || rust_files[0].path() != "src/lib.rs" {
            return Err(KnowledgeError::UnsupportedCrateShape);
        }
        let source_file = rust_files[0];
        if source_file.content_kind() != ContentKind::TextUtf8 {
            return Err(KnowledgeError::InvalidUtf8 {
                path: source_file.path().to_owned(),
            });
        }
        let source =
            std::str::from_utf8(source_file.bytes()).map_err(|_| KnowledgeError::InvalidUtf8 {
                path: source_file.path().to_owned(),
            })?;
        extract_source(
            inventory.bound_revision().repository_identity().as_str(),
            inventory.bound_revision().commit_oid().as_str(),
            source_file.blob_oid().as_str(),
            source_file.path(),
            source,
        )
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct EntityKey {
    kind: EntityKind,
    canonical_identity: String,
}

#[derive(Clone, Debug)]
struct EntityDraft {
    key: EntityKey,
    display_name: String,
    properties: EntityProperties,
    evidence: EvidenceKey,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct EvidenceKey {
    span: ByteSpan,
    syntax_kind: String,
}

#[derive(Clone, Debug)]
struct RelationshipDraft {
    kind: RelationshipKind,
    source: EntityKey,
    target: EntityKey,
    evidence: EvidenceKey,
}

#[derive(Clone, Debug)]
struct PendingImport {
    owner: EntityKey,
    target_paths: Vec<String>,
    evidence: EvidenceKey,
}

struct Builder<'a> {
    repository_identity: &'a str,
    commit_oid: &'a str,
    blob_oid: &'a str,
    path: &'a str,
    source: &'a str,
    malformed: Vec<ByteSpan>,
    entities: BTreeMap<EntityKey, EntityDraft>,
    relationships: Vec<RelationshipDraft>,
    pending_imports: Vec<PendingImport>,
    diagnostics: Vec<ExtractionDiagnostic>,
    extra_gaps: Vec<(String, ByteSpan, EvidenceKey)>,
}

fn extract_source(
    repository_identity: &str,
    commit_oid: &str,
    blob_oid: &str,
    path: &str,
    source: &str,
) -> Result<RustKnowledge, KnowledgeError> {
    let source_length = u64::try_from(source.len()).map_err(|_| KnowledgeError::LimitExceeded {
        limit: "single_file_bytes",
        maximum: STANDARD_LOCAL_S1_LIMITS.single_file_bytes,
        observed: STANDARD_LOCAL_S1_LIMITS.single_file_bytes + 1,
    })?;
    if source_length == 0 {
        return Err(KnowledgeError::ContractInvalid);
    }
    if source_length > STANDARD_LOCAL_S1_LIMITS.single_file_bytes {
        return Err(extraction_limit(
            "single_file_bytes",
            STANDARD_LOCAL_S1_LIMITS.single_file_bytes,
        ));
    }
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .map_err(|_| KnowledgeError::ParserCancelled {
            path: path.to_owned(),
        })?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| KnowledgeError::ParserCancelled {
            path: path.to_owned(),
        })?;
    let malformed = malformed_spans(tree.root_node(), path)?;
    let builder = Builder {
        repository_identity,
        commit_oid,
        blob_oid,
        path,
        source,
        malformed,
        entities: BTreeMap::new(),
        relationships: Vec::new(),
        pending_imports: Vec::new(),
        diagnostics: Vec::new(),
        extra_gaps: Vec::new(),
    };
    builder.extract(tree.root_node())
}

impl Builder<'_> {
    fn extract(mut self, root: Node<'_>) -> Result<RustKnowledge, KnowledgeError> {
        let root_evidence = EvidenceKey {
            span: byte_span(root)?,
            syntax_kind: "source_file".to_owned(),
        };
        let crate_key = self.add_entity(
            EntityKind::RustCrate,
            "crate".to_owned(),
            "crate".to_owned(),
            properties(&[("crate_root", self.path)]),
            root_evidence.clone(),
        )?;
        let file_key = self.add_entity(
            EntityKind::SourceFile,
            format!("file:{}", self.path),
            self.path.to_owned(),
            properties(&[("path", self.path)]),
            root_evidence.clone(),
        )?;

        self.walk_scope(root, &crate_key, "crate", 0)?;
        self.resolve_imports()?;

        for malformed in self.malformed.clone() {
            self.push_diagnostic(ExtractionDiagnostic {
                code: "extraction.malformed_syntax".to_owned(),
                severity: "warning".to_owned(),
                path: self.path.to_owned(),
                span: malformed,
            })?;
            self.push_gap((
                "malformed_syntax_excluded".to_owned(),
                malformed,
                root_evidence.clone(),
            ))?;
        }
        self.finish(&crate_key, &file_key, &root_evidence)
    }

    fn walk_scope(
        &mut self,
        container: Node<'_>,
        owner: &EntityKey,
        scope: &str,
        depth: u64,
    ) -> Result<(), KnowledgeError> {
        if depth > STANDARD_LOCAL_S1_LIMITS.recursion_depth {
            return Err(extraction_limit(
                "syntax_recursion_depth",
                STANDARD_LOCAL_S1_LIMITS.recursion_depth,
            ));
        }
        let mut cursor = container.walk();
        for node in container.named_children(&mut cursor) {
            if self.node_is_malformed(node)? {
                continue;
            }
            match node.kind() {
                "use_declaration" => self.add_import(node, owner, scope)?,
                "type_item" => {
                    self.add_named_declaration(node, owner, scope, EntityKind::RustTypeAlias)?;
                }
                "trait_item" => self.add_trait(node, owner, scope)?,
                "mod_item" => self.add_module(node, owner, scope, depth)?,
                "struct_item" => {
                    self.add_named_declaration(node, owner, scope, EntityKind::RustStruct)?;
                }
                "enum_item" => {
                    self.add_named_declaration(node, owner, scope, EntityKind::RustEnum)?;
                }
                "function_item" => {
                    self.add_named_declaration(node, owner, scope, EntityKind::RustFunction)?;
                }
                "impl_item" => self.add_impl(node, scope)?,
                "line_comment" | "block_comment" => {}
                _ => self.add_unsupported(node, &root_evidence_key(self.source)?)?,
            }
        }
        Ok(())
    }

    fn add_named_declaration(
        &mut self,
        node: Node<'_>,
        owner: &EntityKey,
        scope: &str,
        kind: EntityKind,
    ) -> Result<EntityKey, KnowledgeError> {
        let name = node_name(node, self.source)?;
        let normalized_name = normalize_identifier(name);
        let canonical_identity = format!("{scope}::{normalized_name}");
        let evidence = evidence_key(node, self.source)?;
        let key = self.add_entity(
            kind,
            canonical_identity,
            name.to_owned(),
            properties(&[("visibility", visibility(node, self.source)?)]),
            evidence.clone(),
        )?;
        self.push_relationship(RelationshipDraft {
            kind: RelationshipKind::Defines,
            source: owner.clone(),
            target: key.clone(),
            evidence,
        })?;
        Ok(key)
    }

    fn add_trait(
        &mut self,
        node: Node<'_>,
        owner: &EntityKey,
        scope: &str,
    ) -> Result<(), KnowledgeError> {
        let trait_key = self.add_named_declaration(node, owner, scope, EntityKind::RustTrait)?;
        let body = node
            .child_by_field_name("body")
            .ok_or(KnowledgeError::ContractInvalid)?;
        let mut cursor = body.walk();
        for child in body.named_children(&mut cursor) {
            if child.kind() != "function_signature_item" || self.node_is_malformed(child)? {
                continue;
            }
            let name = node_name(child, self.source)?;
            let evidence = evidence_key(child, self.source)?;
            let method_key = self.add_entity(
                EntityKind::RustMethod,
                format!(
                    "{}::{}",
                    trait_key.canonical_identity,
                    normalize_identifier(name)
                ),
                name.to_owned(),
                properties(&[
                    ("owner_kind", EntityKind::RustTrait.as_str()),
                    ("visibility", "inherited_trait"),
                ]),
                evidence.clone(),
            )?;
            self.push_relationship(RelationshipDraft {
                kind: RelationshipKind::Defines,
                source: trait_key.clone(),
                target: method_key,
                evidence,
            })?;
        }
        Ok(())
    }

    fn add_module(
        &mut self,
        node: Node<'_>,
        owner: &EntityKey,
        scope: &str,
        depth: u64,
    ) -> Result<(), KnowledgeError> {
        let module_key = self.add_named_declaration(node, owner, scope, EntityKind::RustModule)?;
        let body = node
            .child_by_field_name("body")
            .ok_or(KnowledgeError::UnsupportedCrateShape)?;
        self.walk_scope(body, &module_key, &module_key.canonical_identity, depth + 1)
    }

    fn add_impl(&mut self, node: Node<'_>, scope: &str) -> Result<(), KnowledgeError> {
        let trait_node = node
            .child_by_field_name("trait")
            .ok_or(KnowledgeError::UnsupportedCrateShape)?;
        let type_node = node
            .child_by_field_name("type")
            .ok_or(KnowledgeError::UnsupportedCrateShape)?;
        let trait_name = normalize_identifier(node_text(trait_node, self.source)?);
        let type_name = normalize_identifier(node_text(type_node, self.source)?);
        let Some(trait_key) = self.resolve_lexical(
            scope,
            &trait_name,
            &[EntityKind::RustTrait, EntityKind::RustSymbolReference],
        ) else {
            return Err(KnowledgeError::UnsupportedCrateShape);
        };
        let Some(type_key) = self.resolve_lexical(
            scope,
            &type_name,
            &[EntityKind::RustStruct, EntityKind::RustEnum],
        ) else {
            return Err(KnowledgeError::UnsupportedCrateShape);
        };
        let evidence = evidence_key(node, self.source)?;
        self.push_relationship(RelationshipDraft {
            kind: RelationshipKind::Implements,
            source: type_key.clone(),
            target: trait_key,
            evidence: evidence.clone(),
        })?;

        let body = node
            .child_by_field_name("body")
            .ok_or(KnowledgeError::UnsupportedCrateShape)?;
        let mut cursor = body.walk();
        for child in body.named_children(&mut cursor) {
            if child.kind() != "function_item" || self.node_is_malformed(child)? {
                continue;
            }
            let name = node_name(child, self.source)?;
            let method_evidence = evidence_key(child, self.source)?;
            let method_key = self.add_entity(
                EntityKind::RustMethod,
                format!(
                    "{}::{}",
                    type_key.canonical_identity,
                    normalize_identifier(name)
                ),
                name.to_owned(),
                properties(&[
                    ("owner_kind", type_key.kind.as_str()),
                    ("visibility", "inherited_trait"),
                ]),
                method_evidence.clone(),
            )?;
            self.push_relationship(RelationshipDraft {
                kind: RelationshipKind::Defines,
                source: type_key.clone(),
                target: method_key,
                evidence: method_evidence,
            })?;
        }
        Ok(())
    }

    fn add_import(
        &mut self,
        node: Node<'_>,
        owner: &EntityKey,
        scope: &str,
    ) -> Result<(), KnowledgeError> {
        if u64::try_from(self.pending_imports.len()).unwrap_or(MAX_S2_RELATIONSHIPS)
            >= MAX_S2_RELATIONSHIPS
        {
            return Err(extraction_limit("relationships", MAX_S2_RELATIONSHIPS));
        }
        self.pending_imports.push(PendingImport {
            owner: owner.clone(),
            target_paths: grouped_import_targets(node_text(node, self.source)?, scope)?,
            evidence: evidence_key(node, self.source)?,
        });
        Ok(())
    }

    fn resolve_imports(&mut self) -> Result<(), KnowledgeError> {
        for pending in std::mem::take(&mut self.pending_imports) {
            for target_path in pending.target_paths {
                let target = self
                    .find_exact(
                        &target_path,
                        &[
                            EntityKind::RustModule,
                            EntityKind::RustStruct,
                            EntityKind::RustEnum,
                            EntityKind::RustTrait,
                            EntityKind::RustTypeAlias,
                            EntityKind::RustFunction,
                        ],
                    )
                    .unwrap_or_else(|| {
                        let symbol_path = target_path
                            .strip_prefix("crate::")
                            .unwrap_or(&target_path)
                            .to_owned();
                        EntityKey {
                            kind: EntityKind::RustSymbolReference,
                            canonical_identity: format!(
                                "unresolved:{}:{symbol_path}",
                                pending.owner.canonical_identity
                            ),
                        }
                    });
                let target = if target.kind == EntityKind::RustSymbolReference
                    && !self.entities.contains_key(&target)
                {
                    let symbol_path = target_path
                        .strip_prefix("crate::")
                        .unwrap_or(&target_path)
                        .to_owned();
                    let created = self.add_entity(
                        EntityKind::RustSymbolReference,
                        target.canonical_identity,
                        symbol_path.clone(),
                        properties(&[("resolution", "unresolved"), ("symbol_path", &symbol_path)]),
                        pending.evidence.clone(),
                    )?;
                    self.push_diagnostic(ExtractionDiagnostic {
                        code: "extraction.unresolved_symbol".to_owned(),
                        severity: "warning".to_owned(),
                        path: self.path.to_owned(),
                        span: pending.evidence.span,
                    })?;
                    self.push_gap((
                        "unresolved_symbol".to_owned(),
                        pending.evidence.span,
                        pending.evidence.clone(),
                    ))?;
                    created
                } else {
                    target
                };
                self.push_relationship(RelationshipDraft {
                    kind: RelationshipKind::Imports,
                    source: pending.owner.clone(),
                    target,
                    evidence: pending.evidence.clone(),
                })?;
            }
        }
        Ok(())
    }

    fn add_entity(
        &mut self,
        kind: EntityKind,
        canonical_identity: String,
        display_name: String,
        properties: EntityProperties,
        evidence: EvidenceKey,
    ) -> Result<EntityKey, KnowledgeError> {
        let key = EntityKey {
            kind,
            canonical_identity,
        };
        if let Some(existing) = self.entities.get(&key) {
            return Err(KnowledgeError::NormalizationCollision {
                kind,
                canonical_identity: key.canonical_identity,
                path: self.path.to_owned(),
                first_span: existing.evidence.span,
                second_span: evidence.span,
            });
        }
        if u64::try_from(self.entities.len()).unwrap_or(MAX_S2_ENTITIES) >= MAX_S2_ENTITIES {
            return Err(extraction_limit("entities", MAX_S2_ENTITIES));
        }
        self.entities.insert(
            key.clone(),
            EntityDraft {
                key: key.clone(),
                display_name,
                properties,
                evidence,
            },
        );
        Ok(key)
    }

    fn add_unsupported(
        &mut self,
        node: Node<'_>,
        root_evidence: &EvidenceKey,
    ) -> Result<(), KnowledgeError> {
        let span = byte_span(node)?;
        self.push_diagnostic(ExtractionDiagnostic {
            code: "extraction.unsupported_construct".to_owned(),
            severity: "warning".to_owned(),
            path: self.path.to_owned(),
            span,
        })?;
        self.push_gap((
            "unsupported_construct".to_owned(),
            span,
            root_evidence.clone(),
        ))?;
        Ok(())
    }

    fn push_relationship(&mut self, relationship: RelationshipDraft) -> Result<(), KnowledgeError> {
        if u64::try_from(self.relationships.len()).unwrap_or(MAX_S2_RELATIONSHIPS)
            >= MAX_S2_RELATIONSHIPS
        {
            return Err(extraction_limit("relationships", MAX_S2_RELATIONSHIPS));
        }
        self.relationships.push(relationship);
        Ok(())
    }

    fn push_diagnostic(&mut self, diagnostic: ExtractionDiagnostic) -> Result<(), KnowledgeError> {
        if u64::try_from(self.diagnostics.len()).unwrap_or(MAX_S2_DIAGNOSTICS) >= MAX_S2_DIAGNOSTICS
        {
            return Err(extraction_limit("diagnostics", MAX_S2_DIAGNOSTICS));
        }
        self.diagnostics.push(diagnostic);
        Ok(())
    }

    fn push_gap(&mut self, gap: (String, ByteSpan, EvidenceKey)) -> Result<(), KnowledgeError> {
        if u64::try_from(self.extra_gaps.len()).unwrap_or(MAX_S2_COVERAGE_GAPS)
            >= MAX_S2_COVERAGE_GAPS
        {
            return Err(extraction_limit("coverage_gaps", MAX_S2_COVERAGE_GAPS));
        }
        self.extra_gaps.push(gap);
        Ok(())
    }

    fn node_is_malformed(&self, node: Node<'_>) -> Result<bool, KnowledgeError> {
        let node_span = byte_span(node)?;
        Ok(self
            .malformed
            .iter()
            .any(|malformed| node_span.intersects(*malformed)))
    }

    fn find_exact(&self, canonical_identity: &str, kinds: &[EntityKind]) -> Option<EntityKey> {
        let mut found = None;
        for kind in kinds {
            let candidate = EntityKey {
                kind: *kind,
                canonical_identity: canonical_identity.to_owned(),
            };
            if self.entities.contains_key(&candidate) {
                if found.is_some() {
                    return None;
                }
                found = Some(candidate);
            }
        }
        found
    }

    fn resolve_lexical(&self, scope: &str, name: &str, kinds: &[EntityKind]) -> Option<EntityKey> {
        let mut candidate_scope = Some(scope);
        while let Some(current_scope) = candidate_scope {
            let canonical_identity = format!("{current_scope}::{name}");
            if let Some(found) = self.find_exact(&canonical_identity, kinds) {
                return Some(found);
            }
            candidate_scope = parent_scope(current_scope);
        }
        None
    }

    fn finish(
        mut self,
        crate_key: &EntityKey,
        file_key: &EntityKey,
        root_evidence: &EvidenceKey,
    ) -> Result<RustKnowledge, KnowledgeError> {
        let evidence_ids = evidence_ids(
            root_evidence.clone(),
            self.entities.values().map(|entity| entity.evidence.clone()),
            self.relationships
                .iter()
                .map(|relationship| relationship.evidence.clone()),
        );
        if u64::try_from(evidence_ids.len()).unwrap_or(MAX_S2_EVIDENCE) > MAX_S2_EVIDENCE {
            return Err(extraction_limit("evidence_records", MAX_S2_EVIDENCE));
        }
        let root_evidence_id = evidence_ids
            .get(root_evidence)
            .ok_or(KnowledgeError::ContractInvalid)?
            .clone();
        let evidence = materialize_evidence(&self, &evidence_ids);
        let entities = materialize_entities(self.repository_identity, self.entities, &evidence_ids);
        let entity_ids = entity_id_index(&entities);
        let mut relationships = materialize_relationships(
            self.repository_identity,
            self.relationships,
            &entity_ids,
            &evidence_ids,
        )?;
        let mut claims = parser_claims(self.repository_identity, &entities, &relationships);
        claims.sort_by(|left, right| left.claim_id.cmp(&right.claim_id));

        sort_diagnostics(&mut self.diagnostics);
        let source_length =
            u64::try_from(self.source.len()).map_err(|_| KnowledgeError::LimitExceeded {
                limit: "single_file_bytes",
                maximum: u64::MAX,
                observed: u64::MAX,
            })?;
        let coverage = materialize_coverage(
            self.path,
            source_length,
            self.extra_gaps,
            &evidence_ids,
            &root_evidence_id,
        );
        let chunk = ExtractionChunk {
            chunk_id: extraction_chunk_id(
                self.repository_identity,
                self.commit_oid,
                self.blob_oid,
                self.path,
            ),
            repository_identity: self.repository_identity.to_owned(),
            commit_oid: self.commit_oid.to_owned(),
            blob_oid: self.blob_oid.to_owned(),
            path: self.path.to_owned(),
            byte_length: source_length,
            entities: entities.clone(),
            relationships: relationships.clone(),
            claims: claims.clone(),
            evidence: evidence.clone(),
            diagnostics: self.diagnostics.clone(),
            coverage: coverage.clone(),
        };
        let (contains, derived_claim) = derive_containment(
            self.repository_identity,
            crate_key,
            file_key,
            &entity_ids,
            &claims,
            root_evidence_id,
        )?;
        relationships.push(contains);
        relationships.sort_by(|left, right| left.relationship_id.cmp(&right.relationship_id));
        claims.push(derived_claim);
        claims.sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
        let graph = KnowledgeGraph {
            repository_identity: self.repository_identity.to_owned(),
            commit_oid: self.commit_oid.to_owned(),
            entities,
            relationships,
            claims,
            evidence,
            diagnostics: self.diagnostics,
            coverage,
        };
        let knowledge = RustKnowledge {
            extraction_chunks: vec![chunk],
            graph,
        };
        knowledge.validate()?;
        Ok(knowledge)
    }
}

fn materialize_evidence(
    builder: &Builder<'_>,
    evidence_ids: &BTreeMap<EvidenceKey, String>,
) -> Vec<SourceEvidence> {
    let mut evidence = evidence_ids
        .iter()
        .map(|(key, evidence_id)| SourceEvidence {
            evidence_id: evidence_id.clone(),
            repository_identity: builder.repository_identity.to_owned(),
            commit_oid: builder.commit_oid.to_owned(),
            blob_oid: builder.blob_oid.to_owned(),
            path: builder.path.to_owned(),
            span: key.span,
            syntax_kind: key.syntax_kind.clone(),
        })
        .collect::<Vec<_>>();
    evidence.sort_by(|left, right| {
        (
            left.path.as_bytes(),
            left.span.start,
            left.span.end,
            left.evidence_id.as_str(),
        )
            .cmp(&(
                right.path.as_bytes(),
                right.span.start,
                right.span.end,
                right.evidence_id.as_str(),
            ))
    });
    evidence
}

fn materialize_entities(
    repository_identity: &str,
    drafts: BTreeMap<EntityKey, EntityDraft>,
    evidence_ids: &BTreeMap<EvidenceKey, String>,
) -> Vec<KnowledgeEntity> {
    let mut entities = drafts
        .into_values()
        .map(|draft| {
            KnowledgeEntity::new(
                repository_identity,
                draft.key.kind,
                draft.key.canonical_identity,
                draft.display_name,
                draft.properties,
                vec![
                    evidence_ids
                        .get(&draft.evidence)
                        .expect("every entity evidence is indexed")
                        .clone(),
                ],
            )
        })
        .collect::<Vec<_>>();
    entities.sort_by(|left, right| left.entity_id.cmp(&right.entity_id));
    entities
}

fn entity_id_index(entities: &[KnowledgeEntity]) -> BTreeMap<EntityKey, String> {
    entities
        .iter()
        .map(|entity| {
            (
                EntityKey {
                    kind: entity.kind,
                    canonical_identity: entity.canonical_identity.clone(),
                },
                entity.entity_id.clone(),
            )
        })
        .collect()
}

fn materialize_relationships(
    repository_identity: &str,
    drafts: Vec<RelationshipDraft>,
    entity_ids: &BTreeMap<EntityKey, String>,
    evidence_ids: &BTreeMap<EvidenceKey, String>,
) -> Result<Vec<KnowledgeRelationship>, KnowledgeError> {
    let mut grouped = BTreeMap::<(RelationshipKind, String, String), BTreeSet<String>>::new();
    for draft in drafts {
        let source_id = entity_ids
            .get(&draft.source)
            .ok_or(KnowledgeError::ContractInvalid)?
            .clone();
        let target_id = entity_ids
            .get(&draft.target)
            .ok_or(KnowledgeError::ContractInvalid)?
            .clone();
        grouped
            .entry((draft.kind, source_id, target_id))
            .or_default()
            .insert(
                evidence_ids
                    .get(&draft.evidence)
                    .expect("every relationship evidence is indexed")
                    .clone(),
            );
    }
    let mut relationships = grouped
        .into_iter()
        .map(|((kind, source_id, target_id), evidence_ids)| {
            KnowledgeRelationship::new(
                repository_identity,
                kind,
                source_id,
                target_id,
                evidence_ids.into_iter().collect(),
            )
        })
        .collect::<Vec<_>>();
    relationships.sort_by(|left, right| left.relationship_id.cmp(&right.relationship_id));
    Ok(relationships)
}

fn sort_diagnostics(diagnostics: &mut Vec<ExtractionDiagnostic>) {
    diagnostics.sort_by(|left, right| {
        (
            left.path.as_bytes(),
            left.span.start,
            left.span.end,
            left.code.as_str(),
        )
            .cmp(&(
                right.path.as_bytes(),
                right.span.start,
                right.span.end,
                right.code.as_str(),
            ))
    });
    diagnostics.dedup();
}

fn materialize_coverage(
    path: &str,
    source_length: u64,
    extra_gaps: Vec<(String, ByteSpan, EvidenceKey)>,
    evidence_ids: &BTreeMap<EvidenceKey, String>,
    root_evidence_id: &str,
) -> ExtractionCoverage {
    let mut gaps = [
        "calls_not_extracted",
        "fields_not_extracted",
        "variants_not_extracted",
    ]
    .into_iter()
    .map(|code| CoverageGap {
        code: code.to_owned(),
        path: path.to_owned(),
        span: ByteSpan::new(0, source_length),
        evidence_id: root_evidence_id.to_owned(),
    })
    .chain(extra_gaps.into_iter().map(|(code, span, evidence_key)| {
        CoverageGap {
            code,
            path: path.to_owned(),
            span,
            evidence_id: evidence_ids
                .get(&evidence_key)
                .map_or_else(|| root_evidence_id.to_owned(), Clone::clone),
        }
    }))
    .collect::<Vec<_>>();
    gaps.sort_by(|left, right| {
        (
            left.path.as_bytes(),
            left.span.start,
            left.span.end,
            left.code.as_str(),
        )
            .cmp(&(
                right.path.as_bytes(),
                right.span.start,
                right.span.end,
                right.code.as_str(),
            ))
    });
    gaps.dedup();
    ExtractionCoverage {
        supported_capabilities: [
            "grouped_imports",
            "inline_modules",
            "named_trait_implementations",
            "named_types",
            "rust_library_root",
            "unicode_identifiers",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        gaps,
    }
}

fn derive_containment(
    repository_identity: &str,
    crate_key: &EntityKey,
    file_key: &EntityKey,
    entity_ids: &BTreeMap<EntityKey, String>,
    claims: &[KnowledgeClaim],
    root_evidence_id: String,
) -> Result<(KnowledgeRelationship, KnowledgeClaim), KnowledgeError> {
    let crate_id = entity_ids
        .get(crate_key)
        .ok_or(KnowledgeError::ContractInvalid)?
        .clone();
    let file_id = entity_ids
        .get(file_key)
        .ok_or(KnowledgeError::ContractInvalid)?
        .clone();
    let contains = KnowledgeRelationship::new(
        repository_identity,
        RelationshipKind::Contains,
        crate_id,
        file_id,
        vec![root_evidence_id],
    );
    let entity_claims = claims
        .iter()
        .filter(|claim| claim.subject_kind == ClaimSubjectKind::Entity)
        .map(|claim| (claim.subject_id.as_str(), claim.claim_id.clone()))
        .collect::<BTreeMap<_, _>>();
    let input_claim_ids = vec![
        entity_claims
            .get(contains.source_entity_id.as_str())
            .ok_or(KnowledgeError::ContractInvalid)?
            .clone(),
        entity_claims
            .get(contains.target_entity_id.as_str())
            .ok_or(KnowledgeError::ContractInvalid)?
            .clone(),
    ];
    let derived_claim = KnowledgeClaim::containment_rule(
        repository_identity,
        contains.relationship_id.clone(),
        input_claim_ids,
        contains.evidence_ids.clone(),
    );
    Ok((contains, derived_claim))
}

fn parser_claims(
    repository_identity: &str,
    entities: &[KnowledgeEntity],
    relationships: &[KnowledgeRelationship],
) -> Vec<KnowledgeClaim> {
    entities
        .iter()
        .map(|entity| {
            KnowledgeClaim::parser(
                repository_identity,
                ClaimSubjectKind::Entity,
                entity.entity_id.clone(),
                entity.evidence_ids.clone(),
            )
        })
        .chain(relationships.iter().map(|relationship| {
            KnowledgeClaim::parser(
                repository_identity,
                ClaimSubjectKind::Relationship,
                relationship.relationship_id.clone(),
                relationship.evidence_ids.clone(),
            )
        }))
        .collect()
}

fn evidence_ids(
    root: EvidenceKey,
    entity_evidence: impl IntoIterator<Item = EvidenceKey>,
    relationship_evidence: impl IntoIterator<Item = EvidenceKey>,
) -> BTreeMap<EvidenceKey, String> {
    let mut keys = entity_evidence
        .into_iter()
        .chain(relationship_evidence)
        .filter(|key| *key != root)
        .collect::<BTreeSet<_>>();
    let mut result = BTreeMap::from([(root, "evidence-s2-0000".to_owned())]);
    for (index, key) in std::mem::take(&mut keys).into_iter().enumerate() {
        result.insert(key, format!("evidence-s2-{:04}", index + 1));
    }
    result
}

fn grouped_import_targets(text: &str, scope: &str) -> Result<Vec<String>, KnowledgeError> {
    let value = text
        .trim()
        .strip_prefix("use ")
        .and_then(|value| value.strip_suffix(';'))
        .ok_or(KnowledgeError::ContractInvalid)?;
    let (prefix, names) = value
        .split_once("::{")
        .and_then(|(prefix, names)| names.strip_suffix('}').map(|names| (prefix, names)))
        .ok_or(KnowledgeError::UnsupportedCrateShape)?;
    let base = use_base(prefix, scope)?;
    let mut targets = Vec::new();
    for name in names
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        if u64::try_from(targets.len()).unwrap_or(MAX_S2_RELATIONSHIPS) >= MAX_S2_RELATIONSHIPS {
            return Err(extraction_limit("relationships", MAX_S2_RELATIONSHIPS));
        }
        targets.push(format!("{base}::{}", normalize_identifier(name)));
    }
    if targets.is_empty() {
        Err(KnowledgeError::UnsupportedCrateShape)
    } else {
        Ok(targets)
    }
}

fn use_base(prefix: &str, scope: &str) -> Result<String, KnowledgeError> {
    let normalized = prefix
        .split("::")
        .map(normalize_identifier)
        .collect::<Vec<_>>();
    match normalized.as_slice() {
        [first, rest @ ..] if first == "crate" => Ok(std::iter::once("crate".to_owned())
            .chain(rest.iter().cloned())
            .collect::<Vec<_>>()
            .join("::")),
        [first, rest @ ..] if first == "self" => Ok(std::iter::once(scope.to_owned())
            .chain(rest.iter().cloned())
            .collect::<Vec<_>>()
            .join("::")),
        [first, rest @ ..] if first == "super" => {
            let parent = parent_scope(scope).ok_or(KnowledgeError::UnsupportedCrateShape)?;
            Ok(std::iter::once(parent.to_owned())
                .chain(rest.iter().cloned())
                .collect::<Vec<_>>()
                .join("::"))
        }
        _ => Err(KnowledgeError::UnsupportedCrateShape),
    }
}

fn parent_scope(scope: &str) -> Option<&str> {
    if scope == "crate" {
        None
    } else {
        scope.rsplit_once("::").map(|(parent, _)| parent)
    }
}

fn malformed_spans(root: Node<'_>, path: &str) -> Result<Vec<ByteSpan>, KnowledgeError> {
    let mut spans = Vec::new();
    let mut frontier = vec![root];
    while let Some(node) = frontier.pop() {
        if node.is_missing() {
            return Err(KnowledgeError::MalformedSyntax {
                path: path.to_owned(),
                span: byte_span(node)?,
            });
        }
        if node.is_error() {
            spans.push(byte_span(node)?);
            continue;
        }
        let mut cursor = node.walk();
        frontier.extend(node.children(&mut cursor));
    }
    spans.sort_unstable();
    spans.dedup();
    Ok(spans)
}

fn evidence_key(node: Node<'_>, _source: &str) -> Result<EvidenceKey, KnowledgeError> {
    Ok(EvidenceKey {
        span: declaration_span(node)?,
        syntax_kind: node.kind().to_owned(),
    })
}

fn root_evidence_key(source: &str) -> Result<EvidenceKey, KnowledgeError> {
    Ok(EvidenceKey {
        span: ByteSpan::new(
            0,
            u64::try_from(source.len()).map_err(|_| KnowledgeError::ContractInvalid)?,
        ),
        syntax_kind: "source_file".to_owned(),
    })
}

fn declaration_span(node: Node<'_>) -> Result<ByteSpan, KnowledgeError> {
    let start = u64::try_from(node.start_byte()).map_err(|_| KnowledgeError::ContractInvalid)?;
    let end = if let Some(body) = node.child_by_field_name("body") {
        if node.kind() == "function_item" && body.named_child_count() == 0 {
            node.end_byte()
        } else {
            body.start_byte()
                .checked_add(1)
                .ok_or(KnowledgeError::ContractInvalid)?
        }
    } else {
        node.end_byte()
    };
    Ok(ByteSpan::new(
        start,
        u64::try_from(end).map_err(|_| KnowledgeError::ContractInvalid)?,
    ))
}

fn byte_span(node: Node<'_>) -> Result<ByteSpan, KnowledgeError> {
    Ok(ByteSpan::new(
        u64::try_from(node.start_byte()).map_err(|_| KnowledgeError::ContractInvalid)?,
        u64::try_from(node.end_byte()).map_err(|_| KnowledgeError::ContractInvalid)?,
    ))
}

fn node_name<'a>(node: Node<'_>, source: &'a str) -> Result<&'a str, KnowledgeError> {
    let name = node
        .child_by_field_name("name")
        .ok_or(KnowledgeError::ContractInvalid)?;
    node_text(name, source)
}

fn node_text<'a>(node: Node<'_>, source: &'a str) -> Result<&'a str, KnowledgeError> {
    node.utf8_text(source.as_bytes())
        .map_err(|_| KnowledgeError::InvalidUtf8 {
            path: String::new(),
        })
}

fn visibility<'a>(node: Node<'_>, source: &'a str) -> Result<&'a str, KnowledgeError> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "visibility_modifier" {
            let value = node_text(child, source)?;
            return Ok(if value.starts_with("pub") {
                "public"
            } else {
                "private"
            });
        }
    }
    Ok("private")
}

fn normalize_identifier(value: &str) -> String {
    value.trim().trim_start_matches("r#").nfc().collect()
}

fn properties(values: &[(&str, &str)]) -> EntityProperties {
    values
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect()
}

const fn extraction_limit(limit: &'static str, maximum: u64) -> KnowledgeError {
    KnowledgeError::LimitExceeded {
        limit,
        maximum,
        observed: maximum + 1,
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use super::*;
    use codenoesis_domain::knowledge::{
        CONTAINMENT_RULE_VERSION, ClaimDerivation, ClaimState, KnowledgeGraph, stable_entity_id,
    };

    const SOURCE_FIXTURE: &str =
        include_str!("../../../tests/fixtures/s2/rust-knowledge-v1/revision-a/src/lib.rs");
    const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s2-rust-knowledge-v1";
    const COMMIT_OID: &str = "d77c36ec27d878cee6d5d85d761de2b70284cd55";
    const BLOB_OID: &str = "615f6db198ab9ebb96fbdfbbfab8d7e4e7c0c242";

    #[test]
    fn gt_fr_ext_002_reviewed_rust_graph() {
        let knowledge = reviewed_knowledge();

        assert_eq!(knowledge.graph.entities.len(), 11);
        assert_eq!(knowledge.graph.relationships.len(), 15);
        assert_eq!(knowledge.graph.claims.len(), 26);
        assert_eq!(knowledge.graph.evidence.len(), 13);
        assert_eq!(
            knowledge
                .graph
                .claims
                .iter()
                .filter(|claim| claim.state == ClaimState::DerivedFact)
                .count(),
            1
        );
        assert_eq!(
            knowledge.extraction_chunks[0].chunk_id,
            "urn:codenoesis:extraction-chunk:blake3:bede00dd7856de4d2f45d762ef2541317559b1d7babf967d3e7be8974e171d89"
        );
    }

    #[test]
    fn gt_dr_idn_001_unicode_normalization_collision() {
        let source = canonical_source();
        let collision = format!("{source}\npub fn cafe\u{301}_label() {{}}\n");
        let error = extract_source(
            REPOSITORY_ID,
            COMMIT_OID,
            BLOB_OID,
            "src/lib.rs",
            &collision,
        )
        .expect_err("NFC-equivalent declarations must collide");

        assert!(matches!(
            error,
            KnowledgeError::NormalizationCollision {
                kind: EntityKind::RustFunction,
                ref canonical_identity,
                first_span: ByteSpan {
                    start: 555,
                    end: 607
                },
                second_span: ByteSpan {
                    start: 631,
                    end: 655
                },
                ..
            } if canonical_identity == "crate::café_label"
        ));
    }

    #[test]
    fn gt_fr_ext_002_malformed_syntax_is_explicit() {
        let malformed =
            canonical_source().replacen("pub fn café_label", "\n§\npub fn café_label", 1);
        let knowledge = extract_source(
            REPOSITORY_ID,
            COMMIT_OID,
            BLOB_OID,
            "src/lib.rs",
            &malformed,
        )
        .expect("bounded malformed syntax must retain disjoint subjects");

        assert!(knowledge.graph.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "extraction.malformed_syntax"
                && diagnostic.span == ByteSpan::new(556, 558)
        }));
        assert!(
            knowledge
                .graph
                .coverage
                .gaps
                .iter()
                .any(|gap| gap.code == "malformed_syntax_excluded")
        );
    }

    #[test]
    fn conf_fr_ext_001_extraction_chunk_v1() {
        let knowledge = reviewed_knowledge();
        knowledge.extraction_chunks[0]
            .validate()
            .expect("reviewed extraction chunk");

        let mut forbidden_relationship = knowledge.extraction_chunks[0].clone();
        forbidden_relationship.relationships.push(
            knowledge
                .graph
                .relationships
                .iter()
                .find(|relationship| relationship.kind == RelationshipKind::Contains)
                .expect("reviewed containment relationship")
                .clone(),
        );
        forbidden_relationship
            .relationships
            .sort_by(|left, right| left.relationship_id.cmp(&right.relationship_id));
        assert_eq!(
            forbidden_relationship.validate(),
            Err(KnowledgeError::ContractInvalid)
        );

        let mut derived_state = knowledge.extraction_chunks[0].clone();
        derived_state.claims[0].state = ClaimState::DerivedFact;
        assert_eq!(
            derived_state.validate(),
            Err(KnowledgeError::ContractInvalid)
        );

        let mut wrong_parser_version = knowledge.extraction_chunks[0].clone();
        let ClaimDerivation::Parser {
            extractor_version, ..
        } = &mut wrong_parser_version.claims[0].derivation
        else {
            panic!("reviewed extraction claim must be parser-derived");
        };
        extractor_version.push_str("-changed");
        assert_eq!(
            wrong_parser_version.validate(),
            Err(KnowledgeError::ContractInvalid)
        );

        let mut dangling_evidence = knowledge.extraction_chunks[0].clone();
        dangling_evidence.entities[0].evidence_ids = vec!["evidence-s2-9999".to_owned()];
        assert_eq!(
            dangling_evidence.validate(),
            Err(KnowledgeError::ContractInvalid)
        );

        let mut out_of_range = knowledge.extraction_chunks[0].clone();
        out_of_range.evidence[0].span.end = out_of_range.byte_length + 1;
        assert_eq!(
            out_of_range.validate(),
            Err(KnowledgeError::ContractInvalid)
        );

        let mut duplicate = knowledge.extraction_chunks[0].clone();
        duplicate.entities.push(duplicate.entities[0].clone());
        duplicate
            .entities
            .sort_by(|left, right| left.entity_id.cmp(&right.entity_id));
        assert_eq!(duplicate.validate(), Err(KnowledgeError::ContractInvalid));

        let mut unordered = knowledge.extraction_chunks[0].clone();
        unordered.entities.reverse();
        assert_eq!(unordered.validate(), Err(KnowledgeError::ContractInvalid));

        let mut oversized_source = knowledge.extraction_chunks[0].clone();
        oversized_source.byte_length = 4_194_305;
        assert_eq!(
            oversized_source.validate(),
            Err(KnowledgeError::LimitExceeded {
                limit: "single_file_bytes",
                maximum: 4_194_304,
                observed: 4_194_305,
            })
        );
    }

    #[test]
    fn pt_fr_knw_001_graph_invariants() {
        let knowledge = reviewed_knowledge();
        knowledge.graph.validate().expect("reviewed graph");

        for (seed, graph) in invalid_graph_seeds(&knowledge.graph) {
            assert!(
                graph.validate().is_err(),
                "invalid graph seed {seed} must fail"
            );
        }

        let mut endpoint = knowledge.graph.clone();
        let relationship_index = endpoint
            .relationships
            .iter()
            .position(|relationship| relationship.kind == RelationshipKind::Implements)
            .expect("reviewed implementation relationship");
        let crate_id = entity_id(&endpoint, EntityKind::RustCrate);
        let trait_id = endpoint.relationships[relationship_index]
            .target_entity_id
            .clone();
        replace_parser_relationship(
            &mut endpoint,
            relationship_index,
            RelationshipKind::Implements,
            crate_id,
            trait_id,
        );
        assert!(matches!(
            endpoint.validate(),
            Err(KnowledgeError::InvalidRelationship { .. })
        ));

        let mut dangling = knowledge.graph.clone();
        let relationship_index = dangling
            .relationships
            .iter()
            .position(|relationship| relationship.kind == RelationshipKind::Implements)
            .expect("reviewed implementation relationship");
        let missing_id = stable_entity_id(REPOSITORY_ID, EntityKind::RustStruct, "crate::Missing");
        let target_id = dangling.relationships[relationship_index]
            .target_entity_id
            .clone();
        replace_parser_relationship(
            &mut dangling,
            relationship_index,
            RelationshipKind::Implements,
            missing_id.clone(),
            target_id,
        );
        assert_eq!(
            dangling.validate(),
            Err(KnowledgeError::DanglingReference {
                reference_id: missing_id,
            })
        );

        let mut without_container = knowledge.graph.clone();
        let contains = without_container
            .relationships
            .iter()
            .find(|relationship| relationship.kind == RelationshipKind::Contains)
            .expect("reviewed containment")
            .clone();
        without_container
            .relationships
            .retain(|relationship| relationship.relationship_id != contains.relationship_id);
        without_container
            .claims
            .retain(|claim| claim.claim_id != contains.claim_id);
        assert!(matches!(
            without_container.validate(),
            Err(KnowledgeError::CardinalityViolation { .. })
        ));

        let mut cycle = knowledge.graph.clone();
        let module_id = entity_id(&cycle, EntityKind::RustModule);
        let relationship_index = cycle
            .relationships
            .iter()
            .position(|relationship| {
                relationship.kind == RelationshipKind::Defines
                    && relationship.target_entity_id == module_id
            })
            .expect("reviewed module definition");
        replace_parser_relationship(
            &mut cycle,
            relationship_index,
            RelationshipKind::Defines,
            module_id.clone(),
            module_id,
        );
        assert!(matches!(
            cycle.validate(),
            Err(KnowledgeError::CardinalityViolation { .. })
        ));
    }

    #[test]
    fn pt_fr_knw_003_rule_provenance_replays() {
        let knowledge = reviewed_knowledge();
        let contains = knowledge
            .graph
            .relationships
            .iter()
            .find(|relationship| relationship.kind == RelationshipKind::Contains)
            .expect("reviewed containment");
        let derived = knowledge
            .graph
            .claims
            .iter()
            .find(|claim| claim.claim_id == contains.claim_id)
            .expect("reviewed derived claim");
        let ClaimDerivation::DeterministicRule {
            rule_version,
            input_claim_ids,
            evidence_ids,
        } = &derived.derivation
        else {
            panic!("containment must retain deterministic-rule provenance");
        };
        assert_eq!(rule_version, CONTAINMENT_RULE_VERSION);
        assert_eq!(input_claim_ids.len(), 2);
        assert_eq!(evidence_ids, &contains.evidence_ids);
        assert_eq!(
            KnowledgeClaim::containment_rule(
                REPOSITORY_ID,
                contains.relationship_id.clone(),
                input_claim_ids.clone(),
                evidence_ids.clone(),
            ),
            *derived
        );

        for mutation in [
            RuleMutation::Reversed,
            RuleMutation::Missing,
            RuleMutation::Duplicate,
            RuleMutation::Dangling,
            RuleMutation::Cyclic,
            RuleMutation::WrongVersion,
            RuleMutation::WrongEvidence,
            RuleMutation::IncompatibleInputState,
        ] {
            let graph = mutate_rule(&knowledge.graph, mutation);
            assert!(
                matches!(
                    graph.validate(),
                    Err(KnowledgeError::InvalidDerivation { .. })
                ),
                "rule mutation {mutation:?} must fail as an invalid derivation"
            );
        }
    }

    #[test]
    fn fz_fr_ext_001_extraction_contract_seed_corpus() {
        let knowledge = reviewed_knowledge();
        for (seed, graph) in invalid_graph_seeds(&knowledge.graph) {
            let first = graph.validate();
            let second = graph.validate();
            assert_eq!(first, second, "seed {seed} must be deterministic");
            assert!(first.is_err(), "seed {seed} must remain rejected");
        }
    }

    #[test]
    fn fz_fr_ext_002_rust_parser_seed_corpus() {
        let source = canonical_source();
        let malformed = source.replacen("pub fn café_label", "\n§\npub fn café_label", 1);
        let collision = format!("{source}\npub fn cafe\u{301}_label() {{}}\n");
        let unsupported = "pub const ANSWER: u8 = 42;\npub fn supported() {}\n".to_owned();
        let grouped_unresolved = "use crate::missing::{One, Two};\n".to_owned();
        let amplified = format!(
            "{}pub fn supported() {{}}\n",
            "// bounded token\n".repeat(10_000)
        );
        let graph_amplified = (0..1_024).fold(String::new(), |mut source, index| {
            writeln!(source, "pub type Amplified{index} = u8;")
                .expect("writing to a String cannot fail");
            source
        });
        let mut nested = "pub fn leaf() {}\n".to_owned();
        for depth in 0..34 {
            nested = format!("pub mod level_{depth} {{ {nested} }}\n");
        }

        let seeds = [
            ("valid", source),
            ("malformed", malformed),
            ("nfc_collision", collision),
            ("unsupported", unsupported),
            ("grouped_unresolved", grouped_unresolved),
            ("token_amplification", amplified),
            ("graph_amplification", graph_amplified),
            ("nesting", nested),
        ];
        for (seed, source) in seeds {
            let first = extract_source(REPOSITORY_ID, COMMIT_OID, BLOB_OID, "src/lib.rs", &source);
            let second = extract_source(REPOSITORY_ID, COMMIT_OID, BLOB_OID, "src/lib.rs", &source);
            assert_eq!(first, second, "parser seed {seed} must be deterministic");
            match seed {
                "valid"
                | "malformed"
                | "unsupported"
                | "grouped_unresolved"
                | "token_amplification"
                | "graph_amplification" => {
                    assert!(first.is_ok(), "parser seed {seed} must be covered");
                }
                "nfc_collision" => assert!(matches!(
                    first,
                    Err(KnowledgeError::NormalizationCollision { .. })
                )),
                "nesting" => assert!(matches!(
                    first,
                    Err(KnowledgeError::LimitExceeded {
                        limit: "syntax_recursion_depth",
                        ..
                    })
                )),
                _ => unreachable!("closed seed corpus"),
            }
        }
    }

    fn reviewed_knowledge() -> RustKnowledge {
        let source = canonical_source();
        extract_source(REPOSITORY_ID, COMMIT_OID, BLOB_OID, "src/lib.rs", &source)
            .expect("extract reviewed Rust fixture")
    }

    fn canonical_source() -> String {
        let normalized = SOURCE_FIXTURE.replace("\r\n", "\n");
        assert!(
            !normalized.contains('\r'),
            "reviewed Rust fixture contains a bare carriage return"
        );
        normalized
    }

    fn entity_id(graph: &KnowledgeGraph, kind: EntityKind) -> String {
        graph
            .entities
            .iter()
            .find(|entity| entity.kind == kind)
            .unwrap_or_else(|| panic!("reviewed graph must contain {}", kind.as_str()))
            .entity_id
            .clone()
    }

    fn replace_parser_relationship(
        graph: &mut KnowledgeGraph,
        relationship_index: usize,
        kind: RelationshipKind,
        source_entity_id: String,
        target_entity_id: String,
    ) {
        let previous_claim_id = graph.relationships[relationship_index].claim_id.clone();
        let evidence_ids = graph.relationships[relationship_index].evidence_ids.clone();
        let replacement = KnowledgeRelationship::new(
            REPOSITORY_ID,
            kind,
            source_entity_id,
            target_entity_id,
            evidence_ids.clone(),
        );
        let claim_index = graph
            .claims
            .iter()
            .position(|claim| claim.claim_id == previous_claim_id)
            .expect("relationship claim");
        graph.claims[claim_index] = KnowledgeClaim::parser(
            REPOSITORY_ID,
            ClaimSubjectKind::Relationship,
            replacement.relationship_id.clone(),
            evidence_ids,
        );
        graph.relationships[relationship_index] = replacement;
        graph
            .relationships
            .sort_by(|left, right| left.relationship_id.cmp(&right.relationship_id));
        graph
            .claims
            .sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
    }

    fn invalid_graph_seeds(graph: &KnowledgeGraph) -> Vec<(&'static str, KnowledgeGraph)> {
        let mut invalid_property = graph.clone();
        invalid_property.entities[0].properties.clear();

        let mut duplicate_relationship = graph.clone();
        duplicate_relationship
            .relationships
            .push(duplicate_relationship.relationships[0].clone());
        duplicate_relationship
            .relationships
            .sort_by(|left, right| left.relationship_id.cmp(&right.relationship_id));

        let mut invalid_evidence = graph.clone();
        invalid_evidence.entities[0].evidence_ids = vec!["evidence-s2-9999".to_owned()];

        let mut unordered = graph.clone();
        unordered.entities.reverse();

        let mut invalid_claim = graph.clone();
        invalid_claim.claims[0].state = ClaimState::Candidate;

        vec![
            ("invalid_property", invalid_property),
            ("duplicate_relationship", duplicate_relationship),
            ("invalid_evidence", invalid_evidence),
            ("unordered", unordered),
            ("invalid_claim", invalid_claim),
        ]
    }

    #[derive(Clone, Copy, Debug)]
    enum RuleMutation {
        Reversed,
        Missing,
        Duplicate,
        Dangling,
        Cyclic,
        WrongVersion,
        WrongEvidence,
        IncompatibleInputState,
    }

    fn mutate_rule(graph: &KnowledgeGraph, mutation: RuleMutation) -> KnowledgeGraph {
        let mut graph = graph.clone();
        let derived_index = graph
            .claims
            .iter()
            .position(|claim| claim.state == ClaimState::DerivedFact)
            .expect("reviewed derived claim");
        let derived_claim_id = graph.claims[derived_index].claim_id.clone();
        let ClaimDerivation::DeterministicRule {
            rule_version,
            input_claim_ids,
            evidence_ids,
        } = &mut graph.claims[derived_index].derivation
        else {
            panic!("reviewed deterministic rule");
        };
        match mutation {
            RuleMutation::Reversed => input_claim_ids.reverse(),
            RuleMutation::Missing => {
                input_claim_ids.pop();
            }
            RuleMutation::Duplicate => input_claim_ids[1] = input_claim_ids[0].clone(),
            RuleMutation::Dangling => {
                input_claim_ids[1] =
                    "urn:codenoesis:claim:blake3:0000000000000000000000000000000000000000000000000000000000000000"
                        .to_owned();
            }
            RuleMutation::Cyclic => input_claim_ids[1] = derived_claim_id,
            RuleMutation::WrongVersion => rule_version.push_str("-changed"),
            RuleMutation::WrongEvidence => {
                evidence_ids[0] = "evidence-s2-9999".to_owned();
            }
            RuleMutation::IncompatibleInputState => {
                let input_id = input_claim_ids[0].clone();
                graph
                    .claims
                    .iter_mut()
                    .find(|claim| claim.claim_id == input_id)
                    .expect("reviewed rule input")
                    .state = ClaimState::Candidate;
            }
        }
        graph
    }
}
