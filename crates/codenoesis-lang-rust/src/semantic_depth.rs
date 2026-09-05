use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};

use codenoesis_domain::knowledge::{ClaimSubjectKind, EntityKind, RelationshipKind};
use codenoesis_domain::s4::{
    WorkspaceEntity, WorkspaceError, WorkspaceEvidence, WorkspaceRelationship, WorkspaceVisibility,
    workspace_declaration_id, workspace_evidence_id, workspace_module_id,
};
use codenoesis_domain::s4_r3::{ExternalWorkspaceBoundary, RootPackageWorkspaceError};
use codenoesis_domain::s4_r4::CargoManifestFactError;
use codenoesis_domain::s4_r5::{
    CompilationPresence, RustMemberProperties, RustMethodContext, RustMethodProperties,
    RustSemanticAttribute, RustSemanticAttributeKind, RustSemanticCoverageGap,
    RustSemanticDepthExtraction, RustSemanticDiagnostic, RustSemanticEntity,
    RustSemanticEntityKind, RustSemanticError, RustSemanticForm, RustSemanticGraph,
    RustSemanticIndex, RustSemanticKnowledge, RustSemanticLimit, RustSemanticOwnerKind,
    RustSemanticProperties, RustSemanticSourceChunk, RustSemanticVisibility, capability_state,
    deterministic_claim, diagnostic_message, rust_semantic_limit_exceeded,
};
use codenoesis_domain::s4_r10::{
    MAX_R10_ALTERNATIVES_PER_METHOD, RustCfgDeclarationAlternativesError,
    RustCfgDeclarationAlternativesExtraction, RustCfgDeclarationAlternativesLimit,
    RustCfgDeclarationAlternativesSourceChunk, RustDeclarationAlternative,
};
use codenoesis_domain::s5::AnalysisCacheEntry;
use codenoesis_domain::{InventoryFile, RepositoryInventory};
use codenoesis_ports::{
    CargoManifestFactExtractor, RustCfgDeclarationAlternativesExtractor, RustSemanticDepthExtractor,
};
use tree_sitter::{Node, Parser, Tree};
use unicode_normalization::UnicodeNormalization as _;

use crate::workspace::parse_rust_tree_with_compatibility;

use crate::TreeSitterRustWorkspaceExtractor;

#[derive(Clone)]
pub(crate) struct SourceContext<'a> {
    pub(crate) repository_identity: String,
    pub(crate) crate_id: String,
    pub(crate) source_file_id: String,
    pub(crate) path: String,
    pub(crate) base_module_path: String,
    pub(crate) base_module_id: String,
    pub(crate) file: &'a InventoryFile,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct OwnerKey {
    crate_id: String,
    module_path: String,
    kind: EntityKind,
    name: String,
}

#[derive(Clone)]
struct OwnerRecord {
    key: OwnerKey,
    id: String,
    source_file_id: String,
    span: ByteRange,
    visibility: RustSemanticVisibility,
    module_owner_id: String,
    direct_cfg: bool,
    attributes: Vec<AttributeDraft>,
}

#[derive(Clone)]
struct AttributeDraft {
    kind: RustSemanticAttributeKind,
    token_text: String,
    span: ByteRange,
}

#[derive(Clone, Copy)]
struct ByteRange {
    start: usize,
    end: usize,
}

struct OwnerCatalog {
    records: BTreeMap<OwnerKey, OwnerRecord>,
    by_id: BTreeMap<String, OwnerKey>,
}

impl OwnerCatalog {
    fn new() -> Self {
        Self {
            records: BTreeMap::new(),
            by_id: BTreeMap::new(),
        }
    }

    fn insert(&mut self, record: OwnerRecord) -> Result<(), RustSemanticError> {
        if let Some(existing) = self.records.get(&record.key) {
            if !closed_cfg_owner_alternative(existing, &record) {
                return Err(RustSemanticError::IdentityConflict {
                    owner_id: existing.module_owner_id.clone(),
                    member_kind: record_kind(record.key.kind).to_owned(),
                    normalized_member: record.key.name.clone(),
                });
            }
            let replace =
                (record.span.start, record.span.end) < (existing.span.start, existing.span.end);
            if replace {
                self.records.insert(record.key.clone(), record);
            }
            return Ok(());
        }
        if self
            .by_id
            .insert(record.id.clone(), record.key.clone())
            .is_some()
        {
            return Err(RustSemanticError::ContractInvalid);
        }
        self.records.insert(record.key.clone(), record);
        Ok(())
    }

    fn by_key(
        &self,
        crate_id: &str,
        module_path: &str,
        kind: EntityKind,
        name: &str,
    ) -> Option<&OwnerRecord> {
        self.records.get(&OwnerKey {
            crate_id: crate_id.to_owned(),
            module_path: module_path.to_owned(),
            kind,
            name: name.to_owned(),
        })
    }

    fn resolve_named(
        &self,
        crate_id: &str,
        current_module: &str,
        text: &str,
        kinds: &[EntityKind],
    ) -> Result<Option<&OwnerRecord>, RustSemanticError> {
        let Some((module_path, name)) = local_path(current_module, text) else {
            return Ok(None);
        };
        let exact = kinds
            .iter()
            .filter_map(|kind| self.by_key(crate_id, &module_path, *kind, &name))
            .collect::<Vec<_>>();
        if exact.len() == 1 {
            return Ok(exact.into_iter().next());
        }
        if exact.len() > 1 {
            return Err(RustSemanticError::UnsupportedComposition {
                reason: "ambiguous_local_declaration",
            });
        }
        if text.contains("::") {
            return Ok(None);
        }
        let candidates = self
            .records
            .values()
            .filter(|record| {
                record.key.crate_id == crate_id
                    && record.key.name == name
                    && kinds.contains(&record.key.kind)
            })
            .collect::<Vec<_>>();
        match candidates.as_slice() {
            [] => Ok(None),
            [record] => Ok(Some(*record)),
            _ => Err(RustSemanticError::UnsupportedComposition {
                reason: "ambiguous_local_declaration",
            }),
        }
    }
}

fn closed_cfg_owner_alternative(existing: &OwnerRecord, candidate: &OwnerRecord) -> bool {
    matches!(
        existing.key.kind,
        EntityKind::RustStruct | EntityKind::RustEnum | EntityKind::RustTrait
    ) && existing.direct_cfg
        && candidate.direct_cfg
        && existing.id == candidate.id
        && existing.source_file_id == candidate.source_file_id
        && existing.visibility == candidate.visibility
        && existing.module_owner_id == candidate.module_owner_id
        && (existing.span.end <= candidate.span.start || candidate.span.end <= existing.span.start)
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum SemanticExtractionMode {
    R5,
    R10,
}

struct SemanticExtractionOutput {
    semantic: RustSemanticDepthExtraction,
    alternatives: Vec<RustCfgDeclarationAlternativesSourceChunk>,
}

struct SemanticExtractionFailure {
    source: Box<RustSemanticError>,
    alternative: Option<Box<RustCfgDeclarationAlternativesError>>,
}

impl From<RustSemanticError> for SemanticExtractionFailure {
    fn from(source: RustSemanticError) -> Self {
        Self {
            source: Box::new(source),
            alternative: None,
        }
    }
}

struct ChunkBuilder<'a> {
    repository_identity: &'a str,
    commit_oid: &'a str,
    context: &'a SourceContext<'a>,
    legacy_entities: BTreeMap<String, WorkspaceEntity>,
    entities: BTreeMap<String, RustSemanticEntity>,
    entity_evidence: BTreeMap<String, String>,
    relationships: BTreeMap<String, WorkspaceRelationship>,
    claims: BTreeMap<String, codenoesis_domain::s4::WorkspaceClaim>,
    evidence: BTreeMap<String, WorkspaceEvidence>,
    diagnostics: BTreeMap<String, RustSemanticDiagnostic>,
    coverage: BTreeMap<String, RustSemanticCoverageGap>,
    mode: SemanticExtractionMode,
    method_occurrences: BTreeMap<String, Vec<RustDeclarationAlternative>>,
    method_raw_names: BTreeMap<String, String>,
    alternative_failure: Option<RustCfgDeclarationAlternativesError>,
}

impl<'a> ChunkBuilder<'a> {
    fn new(
        repository_identity: &'a str,
        commit_oid: &'a str,
        context: &'a SourceContext<'a>,
        mode: SemanticExtractionMode,
    ) -> Result<Self, RustSemanticError> {
        let mut builder = Self {
            repository_identity,
            commit_oid,
            context,
            legacy_entities: BTreeMap::new(),
            entities: BTreeMap::new(),
            entity_evidence: BTreeMap::new(),
            relationships: BTreeMap::new(),
            claims: BTreeMap::new(),
            evidence: BTreeMap::new(),
            diagnostics: BTreeMap::new(),
            coverage: BTreeMap::new(),
            mode,
            method_occurrences: BTreeMap::new(),
            method_raw_names: BTreeMap::new(),
            alternative_failure: None,
        };
        builder.add_evidence(ByteRange {
            start: 0,
            end: context.file.bytes().len(),
        })?;
        Ok(builder)
    }

    fn add_evidence(&mut self, range: ByteRange) -> Result<String, RustSemanticError> {
        if range.start >= range.end || range.end > self.context.file.bytes().len() {
            return Err(invalid_declaration(
                &self.context.path,
                range.start,
                "invalid_span",
            ));
        }
        let start_byte =
            u64::try_from(range.start).map_err(|_| RustSemanticError::ContractInvalid)?;
        let end_byte = u64::try_from(range.end).map_err(|_| RustSemanticError::ContractInvalid)?;
        let id = workspace_evidence_id(
            self.repository_identity,
            self.commit_oid,
            self.context.file.blob_oid().as_str(),
            &self.context.path,
            start_byte,
            end_byte,
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

    fn materialize_attributes(
        &mut self,
        attributes: &[AttributeDraft],
    ) -> Result<Vec<RustSemanticAttribute>, RustSemanticError> {
        attributes
            .iter()
            .map(|attribute| {
                let evidence_id = self.add_evidence(attribute.span)?;
                self.add_capability(
                    match attribute.kind {
                        RustSemanticAttributeKind::Cfg | RustSemanticAttributeKind::CfgAttr => {
                            "rust.cfg_presence_unresolved"
                        }
                        RustSemanticAttributeKind::Other => {
                            "rust.attribute_semantics_not_interpreted"
                        }
                    },
                    evidence_id.clone(),
                    true,
                )?;
                Ok(RustSemanticAttribute {
                    kind: attribute.kind,
                    token_text: attribute.token_text.clone(),
                    evidence_id,
                })
            })
            .collect()
    }

    fn add_capability(
        &mut self,
        capability: &str,
        evidence_id: String,
        diagnostic: bool,
    ) -> Result<(), RustSemanticError> {
        let state = capability_state(capability).ok_or(RustSemanticError::ContractInvalid)?;
        let gap = RustSemanticCoverageGap::new(
            self.repository_identity,
            self.commit_oid,
            capability,
            state,
            vec![evidence_id.clone()],
        );
        self.coverage.entry(gap.id.clone()).or_insert(gap);
        if diagnostic {
            let diagnostic = RustSemanticDiagnostic::new(
                self.repository_identity,
                capability,
                diagnostic_message(capability),
                vec![evidence_id],
            );
            self.diagnostics
                .entry(diagnostic.id.clone())
                .or_insert(diagnostic);
        }
        Ok(())
    }

    fn add_legacy_entity(
        &mut self,
        entity: WorkspaceEntity,
        owner_id: String,
        evidence_id: String,
    ) -> Result<(), RustSemanticError> {
        if self
            .legacy_entities
            .insert(entity.id.clone(), entity.clone())
            .is_some()
        {
            return Err(RustSemanticError::ContractInvalid);
        }
        self.add_claim(
            ClaimSubjectKind::Entity,
            entity.id.clone(),
            evidence_id.clone(),
        )?;
        self.add_relationship(RelationshipKind::Defines, owner_id, entity.id, evidence_id)
    }

    fn add_entity(
        &mut self,
        entity: RustSemanticEntity,
        evidence_id: String,
    ) -> Result<(), RustSemanticError> {
        if let Some(existing) = self.entities.get(&entity.id).cloned() {
            if self.mode == SemanticExtractionMode::R10
                && entity.kind == RustSemanticEntityKind::Method
            {
                return self.add_r10_method_occurrence(&existing, entity, evidence_id);
            }
            let normalized_member = entity.identity_member();
            let existing_evidence = self
                .entity_evidence
                .get(&entity.id)
                .and_then(|identifier| self.evidence.get(identifier))
                .ok_or(RustSemanticError::ContractInvalid)?;
            let candidate_evidence = self
                .evidence
                .get(&evidence_id)
                .ok_or(RustSemanticError::ContractInvalid)?;
            if !closed_cfg_member_alternative(
                &existing,
                &entity,
                existing_evidence,
                candidate_evidence,
            ) {
                return Err(RustSemanticError::IdentityConflict {
                    owner_id: entity.owner_id.clone(),
                    member_kind: entity.kind.as_str().to_owned(),
                    normalized_member,
                });
            }
            let mut attributes = existing.attributes().to_vec();
            attributes.extend_from_slice(entity.attributes());
            attributes.sort_by(|left, right| {
                (left.kind, &left.token_text, &left.evidence_id).cmp(&(
                    right.kind,
                    &right.token_text,
                    &right.evidence_id,
                ))
            });
            attributes.dedup();
            enforce_count(
                RustSemanticLimit::OuterAttributesPerDeclaration,
                attributes.len(),
            )?;
            let existing = self
                .entities
                .get_mut(&entity.id)
                .ok_or(RustSemanticError::ContractInvalid)?;
            *member_attributes_mut(existing) = attributes;
            return Ok(());
        }
        if self.mode == SemanticExtractionMode::R10
            && entity.kind == RustSemanticEntityKind::Method
            && entity.compilation_presence == CompilationPresence::ConditionalUnknown
            && entity
                .attributes()
                .iter()
                .any(|attribute| attribute.kind == RustSemanticAttributeKind::Cfg)
        {
            let occurrence = self.r10_occurrence(&entity, &evidence_id)?;
            self.method_occurrences
                .entry(entity.id.clone())
                .or_default()
                .push(occurrence);
        }
        self.add_claim(
            ClaimSubjectKind::Entity,
            entity.id.clone(),
            evidence_id.clone(),
        )?;
        self.add_relationship(
            RelationshipKind::Defines,
            entity.owner_id.clone(),
            entity.id.clone(),
            evidence_id.clone(),
        )?;
        if self
            .entity_evidence
            .insert(entity.id.clone(), evidence_id)
            .is_some()
        {
            return Err(RustSemanticError::ContractInvalid);
        }
        self.entities.insert(entity.id.clone(), entity);
        Ok(())
    }

    fn add_method_entity(
        &mut self,
        entity: RustSemanticEntity,
        evidence_id: String,
        raw_name: &str,
    ) -> Result<(), RustSemanticError> {
        if self.mode == SemanticExtractionMode::R10 {
            if let Some(existing) = self.method_raw_names.get(&entity.id) {
                if existing != raw_name {
                    return self.fail_alternative(
                        RustCfgDeclarationAlternativesError::IdentityMismatch {
                            logical_method_id: entity.id,
                            reason: "unicode_nfc_collision",
                        },
                    );
                }
            } else {
                self.method_raw_names
                    .insert(entity.id.clone(), raw_name.to_owned());
            }
        }
        self.add_entity(entity, evidence_id)
    }

    fn add_r10_method_occurrence(
        &mut self,
        existing: &RustSemanticEntity,
        candidate: RustSemanticEntity,
        evidence_id: String,
    ) -> Result<(), RustSemanticError> {
        if !same_r10_logical_method(existing, &candidate) {
            return self.fail_alternative(RustCfgDeclarationAlternativesError::IdentityMismatch {
                logical_method_id: candidate.id,
                reason: "logical_properties",
            });
        }
        let occurrence = self.r10_occurrence(&candidate, &evidence_id)?;
        let existing_occurrences = self.method_occurrences.get(&candidate.id).ok_or_else(|| {
            self.alternative_failure =
                Some(RustCfgDeclarationAlternativesError::IdentityMismatch {
                    logical_method_id: candidate.id.clone(),
                    reason: "direct_cfg_required",
                });
            RustSemanticError::ContractInvalid
        })?;
        let candidate_evidence = self
            .evidence
            .get(&evidence_id)
            .ok_or(RustSemanticError::ContractInvalid)?;
        for existing_occurrence in existing_occurrences {
            let existing_id = &existing_occurrence.properties.declaration_evidence_id;
            if existing_id == &evidence_id {
                return self.fail_alternative(RustCfgDeclarationAlternativesError::Duplicate {
                    logical_method_id: candidate.id,
                    declaration_evidence_id: evidence_id,
                });
            }
            let existing_evidence = self
                .evidence
                .get(existing_id)
                .ok_or(RustSemanticError::ContractInvalid)?;
            if existing_evidence.path != candidate_evidence.path
                || existing_evidence.blob_oid != candidate_evidence.blob_oid
            {
                return self.fail_alternative(RustCfgDeclarationAlternativesError::CrossSource {
                    logical_method_id: candidate.id,
                });
            }
            if existing_evidence.end_byte > candidate_evidence.start_byte
                && candidate_evidence.end_byte > existing_evidence.start_byte
            {
                return self.fail_alternative(RustCfgDeclarationAlternativesError::Overlap {
                    logical_method_id: candidate.id,
                    first_evidence_id: existing_id.clone(),
                    second_evidence_id: evidence_id,
                });
            }
        }
        let observed = existing_occurrences.len().saturating_add(1);
        if u64::try_from(observed).unwrap_or(u64::MAX) > MAX_R10_ALTERNATIVES_PER_METHOD {
            return self.fail_alternative(RustCfgDeclarationAlternativesError::LimitExceeded {
                limit: RustCfgDeclarationAlternativesLimit::AlternativesPerLogicalMethod,
                maximum: MAX_R10_ALTERNATIVES_PER_METHOD,
                observed: MAX_R10_ALTERNATIVES_PER_METHOD.saturating_add(1),
            });
        }
        self.method_occurrences
            .get_mut(&candidate.id)
            .ok_or(RustSemanticError::ContractInvalid)?
            .push(occurrence);
        Ok(())
    }

    fn r10_occurrence(
        &mut self,
        method: &RustSemanticEntity,
        declaration_evidence_id: &str,
    ) -> Result<RustDeclarationAlternative, RustSemanticError> {
        let direct_cfg_evidence_ids = method
            .attributes()
            .iter()
            .filter(|attribute| attribute.kind == RustSemanticAttributeKind::Cfg)
            .map(|attribute| attribute.evidence_id.clone())
            .collect::<Vec<_>>();
        RustDeclarationAlternative::from_method(
            self.repository_identity,
            method,
            self.context.source_file_id.clone(),
            declaration_evidence_id.to_owned(),
            direct_cfg_evidence_ids,
        )
        .map_err(|error| {
            self.alternative_failure = Some(error);
            RustSemanticError::ContractInvalid
        })
    }

    fn fail_alternative<T>(
        &mut self,
        error: RustCfgDeclarationAlternativesError,
    ) -> Result<T, RustSemanticError> {
        self.alternative_failure = Some(error);
        Err(RustSemanticError::ContractInvalid)
    }

    fn add_relationship(
        &mut self,
        kind: RelationshipKind,
        source: String,
        target: String,
        evidence_id: String,
    ) -> Result<(), RustSemanticError> {
        let relationship =
            WorkspaceRelationship::new(kind, source, target, vec![evidence_id.clone()]);
        if self.relationships.contains_key(&relationship.id) {
            return Ok(());
        }
        self.add_claim(
            ClaimSubjectKind::Relationship,
            relationship.id.clone(),
            evidence_id,
        )?;
        self.relationships
            .insert(relationship.id.clone(), relationship);
        Ok(())
    }

    fn add_claim(
        &mut self,
        subject_kind: ClaimSubjectKind,
        subject_id: String,
        evidence_id: String,
    ) -> Result<(), RustSemanticError> {
        let claim = deterministic_claim(subject_kind, subject_id, evidence_id);
        if self.claims.insert(claim.id.clone(), claim).is_some() {
            return Err(RustSemanticError::ContractInvalid);
        }
        Ok(())
    }

    fn finish(self) -> RustSemanticSourceChunk {
        RustSemanticSourceChunk {
            crate_id: self.context.crate_id.clone(),
            source_file_id: self.context.source_file_id.clone(),
            legacy_entities: self.legacy_entities.into_values().collect(),
            entities: self.entities.into_values().collect(),
            relationships: self.relationships.into_values().collect(),
            claims: self.claims.into_values().collect(),
            evidence: self.evidence.into_values().collect(),
            diagnostics: self.diagnostics.into_values().collect(),
            coverage: self.coverage.into_values().collect(),
        }
    }

    fn alternative_chunk(
        &self,
    ) -> Result<RustCfgDeclarationAlternativesSourceChunk, RustCfgDeclarationAlternativesError>
    {
        let alternatives = self
            .method_occurrences
            .values()
            .filter(|occurrences| occurrences.len() >= 2)
            .flatten()
            .cloned()
            .collect();
        RustCfgDeclarationAlternativesSourceChunk::new(
            self.context.source_file_id.clone(),
            alternatives,
        )
    }
}

fn same_r10_logical_method(existing: &RustSemanticEntity, candidate: &RustSemanticEntity) -> bool {
    let (
        RustSemanticProperties::Method(existing_properties),
        RustSemanticProperties::Method(candidate_properties),
    ) = (&existing.properties, &candidate.properties)
    else {
        return false;
    };
    existing.id == candidate.id
        && existing.kind == RustSemanticEntityKind::Method
        && candidate.kind == RustSemanticEntityKind::Method
        && existing.crate_id == candidate.crate_id
        && existing.module_path == candidate.module_path
        && existing.name == candidate.name
        && existing.visibility == candidate.visibility
        && existing.owner_id == candidate.owner_id
        && existing.trait_context_id == candidate.trait_context_id
        && existing_properties.implementation_context == candidate_properties.implementation_context
        && existing_properties.trait_context_id == candidate_properties.trait_context_id
        && existing.compilation_presence == CompilationPresence::ConditionalUnknown
        && candidate.compilation_presence == CompilationPresence::ConditionalUnknown
        && existing
            .attributes()
            .iter()
            .any(|attribute| attribute.kind == RustSemanticAttributeKind::Cfg)
        && candidate
            .attributes()
            .iter()
            .any(|attribute| attribute.kind == RustSemanticAttributeKind::Cfg)
}

fn closed_cfg_member_alternative(
    existing: &RustSemanticEntity,
    candidate: &RustSemanticEntity,
    existing_evidence: &WorkspaceEvidence,
    candidate_evidence: &WorkspaceEvidence,
) -> bool {
    existing.compilation_presence == CompilationPresence::ConditionalUnknown
        && candidate.compilation_presence == CompilationPresence::ConditionalUnknown
        && existing
            .attributes()
            .iter()
            .any(|attribute| attribute.kind == RustSemanticAttributeKind::Cfg)
        && candidate
            .attributes()
            .iter()
            .any(|attribute| attribute.kind == RustSemanticAttributeKind::Cfg)
        && member_without_attributes(existing) == member_without_attributes(candidate)
        && existing_evidence.path == candidate_evidence.path
        && existing_evidence.blob_oid == candidate_evidence.blob_oid
        && (existing_evidence.end_byte <= candidate_evidence.start_byte
            || candidate_evidence.end_byte <= existing_evidence.start_byte)
}

fn member_without_attributes(entity: &RustSemanticEntity) -> RustSemanticEntity {
    let mut entity = entity.clone();
    member_attributes_mut(&mut entity).clear();
    entity
}

fn member_attributes_mut(entity: &mut RustSemanticEntity) -> &mut Vec<RustSemanticAttribute> {
    match &mut entity.properties {
        codenoesis_domain::s4_r5::RustSemanticProperties::Member(properties) => {
            &mut properties.attributes
        }
        codenoesis_domain::s4_r5::RustSemanticProperties::Method(properties) => {
            &mut properties.attributes
        }
    }
}

impl RustSemanticDepthExtractor for TreeSitterRustWorkspaceExtractor {
    fn extract_rust_semantic_depth_incremental(
        &self,
        inventory: &RepositoryInventory,
        external_boundaries: &[ExternalWorkspaceBoundary],
        cache_entries: &[AnalysisCacheEntry],
    ) -> Result<RustSemanticDepthExtraction, RustSemanticError> {
        extract_semantic_depth(
            *self,
            inventory,
            external_boundaries,
            cache_entries,
            SemanticExtractionMode::R5,
        )
        .map(|output| output.semantic)
        .map_err(|failure| *failure.source)
    }
}

impl RustCfgDeclarationAlternativesExtractor for TreeSitterRustWorkspaceExtractor {
    fn extract_rust_cfg_declaration_alternatives_incremental(
        &self,
        inventory: &RepositoryInventory,
        external_boundaries: &[ExternalWorkspaceBoundary],
        cache_entries: &[AnalysisCacheEntry],
    ) -> Result<RustCfgDeclarationAlternativesExtraction, RustCfgDeclarationAlternativesError> {
        let output = extract_semantic_depth(
            *self,
            inventory,
            external_boundaries,
            cache_entries,
            SemanticExtractionMode::R10,
        )
        .map_err(|failure| match failure.alternative {
            Some(alternative) => *alternative,
            None => RustCfgDeclarationAlternativesError::Source(*failure.source),
        })?;
        RustCfgDeclarationAlternativesExtraction::from_r5(output.semantic, output.alternatives)
    }
}

#[allow(clippy::too_many_lines)]
fn extract_semantic_depth(
    extractor: TreeSitterRustWorkspaceExtractor,
    inventory: &RepositoryInventory,
    external_boundaries: &[ExternalWorkspaceBoundary],
    cache_entries: &[AnalysisCacheEntry],
    mode: SemanticExtractionMode,
) -> Result<SemanticExtractionOutput, SemanticExtractionFailure> {
    let manifest = <TreeSitterRustWorkspaceExtractor as CargoManifestFactExtractor>::extract_cargo_manifest_facts_incremental(
        &extractor,
        inventory,
        external_boundaries,
        cache_entries,
    )
    .map_err(map_manifest_source_error)?;
    let repository_identity = inventory.bound_revision().repository_identity().as_str();
    let commit_oid = inventory.bound_revision().commit_oid().as_str();
    let contexts = source_contexts(&manifest.knowledge, inventory)?;
    let base_entity_ids = manifest
        .knowledge
        .workspace
        .knowledge
        .graph
        .entities
        .iter()
        .map(|entity| entity.id.clone())
        .collect::<BTreeSet<_>>();
    let mut catalog = OwnerCatalog::new();
    for context in &contexts {
        let source = source_text(context)?;
        let tree = parse_tree(&context.path, source)?;
        collect_owners(
            tree.root_node(),
            source,
            context,
            &context.base_module_path,
            &context.base_module_id,
            CompilationPresence::Unconditional,
            &mut catalog,
        )?;
    }

    let mut builders = contexts
        .iter()
        .map(|context| {
            Ok((
                context.source_file_id.clone(),
                ChunkBuilder::new(repository_identity, commit_oid, context, mode)?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>, RustSemanticError>>()?;

    for record in catalog.records.values() {
        let builder = builders
            .get_mut(&record.source_file_id)
            .ok_or(RustSemanticError::ContractInvalid)?;
        builder.materialize_attributes(&record.attributes)?;
        if base_entity_ids.contains(&record.id) {
            continue;
        }
        let evidence_id = builder.add_evidence(record.span)?;
        let entity = match record.key.kind {
            EntityKind::RustModule => WorkspaceEntity::module(
                repository_identity,
                &record.key.crate_id,
                &record.key.module_path,
                &record.key.name,
                workspace_visibility(record.visibility),
                &record.source_file_id,
            ),
            EntityKind::RustStruct | EntityKind::RustEnum | EntityKind::RustTrait => {
                WorkspaceEntity::declaration(
                    repository_identity,
                    record.key.kind,
                    &record.key.crate_id,
                    &record.key.module_path,
                    &record.key.name,
                    workspace_visibility(record.visibility),
                )
            }
            _ => return Err(RustSemanticError::ContractInvalid.into()),
        };
        if entity.id != record.id {
            return Err(RustSemanticError::ContractInvalid.into());
        }
        builder.add_legacy_entity(entity, record.module_owner_id.clone(), evidence_id)?;
    }

    for context in &contexts {
        let source = source_text(context)?;
        let tree = parse_tree(&context.path, source)?;
        let builder = builders
            .get_mut(&context.source_file_id)
            .ok_or(RustSemanticError::ContractInvalid)?;
        if let Err(source) = process_scope(
            tree.root_node(),
            source,
            context,
            &context.base_module_path,
            &context.base_module_id,
            CompilationPresence::Unconditional,
            &catalog,
            builder,
        ) {
            return Err(SemanticExtractionFailure {
                source: Box::new(source),
                alternative: builder.alternative_failure.take().map(Box::new),
            });
        }
    }

    let first_source = builders
        .keys()
        .next()
        .cloned()
        .ok_or(RustSemanticError::ContractInvalid)?;
    let root_evidence = builders
        .get(&first_source)
        .ok_or(RustSemanticError::ContractInvalid)?
        .evidence
        .keys()
        .next()
        .cloned()
        .ok_or(RustSemanticError::ContractInvalid)?;
    let present_capabilities = builders
        .values()
        .flat_map(|builder| builder.coverage.values())
        .map(|gap| gap.capability.clone())
        .collect::<BTreeSet<_>>();
    for capability in [
        "rust.attribute_semantics_not_interpreted",
        "rust.cfg_presence_unresolved",
        "rust.macro_generated_items_not_analyzed",
        "rust.type_resolution_not_performed",
        "rust.value_not_evaluated",
        "rust.union_unsupported",
        "rust.foreign_block_unsupported",
        "rust.unsupported_impl_header",
    ] {
        if !present_capabilities.contains(capability) {
            builders
                .get_mut(&first_source)
                .ok_or(RustSemanticError::ContractInvalid)?
                .add_capability(capability, root_evidence.clone(), false)?;
        }
    }

    if mode == SemanticExtractionMode::R10 {
        let mut method_sources = BTreeMap::new();
        for builder in builders.values() {
            for logical_method_id in builder.method_occurrences.keys() {
                if method_sources
                    .insert(logical_method_id, builder.context.source_file_id.as_str())
                    .is_some_and(|source| source != builder.context.source_file_id)
                {
                    return Err(SemanticExtractionFailure {
                        source: Box::new(RustSemanticError::ContractInvalid),
                        alternative: Some(Box::new(
                            RustCfgDeclarationAlternativesError::CrossSource {
                                logical_method_id: logical_method_id.clone(),
                            },
                        )),
                    });
                }
            }
        }
    }
    let alternatives = if mode == SemanticExtractionMode::R10 {
        builders
            .values()
            .map(ChunkBuilder::alternative_chunk)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|alternative| SemanticExtractionFailure {
                source: Box::new(RustSemanticError::ContractInvalid),
                alternative: Some(Box::new(alternative)),
            })?
    } else {
        Vec::new()
    };
    let extraction_chunks = builders
        .into_values()
        .map(ChunkBuilder::finish)
        .collect::<Vec<_>>();
    let graph = aggregate_graph(&extraction_chunks)?;
    let parser_invocation_count = manifest
        .parser_invocation_count
        .saturating_add(u64::try_from(contexts.len()).unwrap_or(u64::MAX));
    let knowledge = RustSemanticKnowledge {
        manifest: manifest.knowledge,
        extraction_chunks,
        graph,
    };
    knowledge.validate()?;
    Ok(SemanticExtractionOutput {
        semantic: RustSemanticDepthExtraction {
            knowledge,
            cache_entries: manifest.cache_entries,
            source_records: manifest.source_records,
            parser_invocation_count,
        },
        alternatives,
    })
}

fn map_manifest_source_error(error: CargoManifestFactError) -> RustSemanticError {
    match error {
        CargoManifestFactError::Source(RootPackageWorkspaceError::Source(
            WorkspaceError::MalformedSyntax { path },
        )) => RustSemanticError::InvalidDeclaration {
            path,
            start_byte: 0,
            declaration_kind: "syntax_error".to_owned(),
        },
        other => RustSemanticError::Source(other),
    }
}

pub(crate) fn source_contexts<'a>(
    knowledge: &codenoesis_domain::s4_r4::CargoManifestKnowledge,
    inventory: &'a RepositoryInventory,
) -> Result<Vec<SourceContext<'a>>, RustSemanticError> {
    let files = inventory
        .files()
        .iter()
        .map(|file| (file.path(), file))
        .collect::<BTreeMap<_, _>>();
    let mut contexts = knowledge
        .workspace
        .knowledge
        .extraction_chunks
        .iter()
        .map(|chunk| {
            let source_entity = chunk
                .entities
                .iter()
                .find(|entity| entity.id == chunk.source_file_id)
                .ok_or(RustSemanticError::ContractInvalid)?;
            let path = source_entity
                .properties
                .get("path")
                .cloned()
                .ok_or(RustSemanticError::ContractInvalid)?;
            let base_module = chunk
                .entities
                .iter()
                .filter(|entity| {
                    entity.kind == EntityKind::RustModule
                        && entity.crate_id.as_deref() == Some(chunk.crate_id.as_str())
                        && entity.properties.get("source_file_id") == Some(&chunk.source_file_id)
                })
                .min_by(|left, right| {
                    let left_path = left.module_path.as_deref().unwrap_or("");
                    let right_path = right.module_path.as_deref().unwrap_or("");
                    (left_path.matches("::").count(), left_path)
                        .cmp(&(right_path.matches("::").count(), right_path))
                })
                .ok_or(RustSemanticError::ContractInvalid)?;
            Ok(SourceContext {
                repository_identity: knowledge
                    .workspace
                    .knowledge
                    .graph
                    .repository_identity
                    .clone(),
                crate_id: chunk.crate_id.clone(),
                source_file_id: chunk.source_file_id.clone(),
                path: path.clone(),
                base_module_path: base_module
                    .module_path
                    .clone()
                    .ok_or(RustSemanticError::ContractInvalid)?,
                base_module_id: base_module.id.clone(),
                file: files
                    .get(path.as_str())
                    .copied()
                    .ok_or(RustSemanticError::ContractInvalid)?,
            })
        })
        .collect::<Result<Vec<_>, RustSemanticError>>()?;
    contexts.sort_by(|left, right| left.path.as_bytes().cmp(right.path.as_bytes()));
    Ok(contexts)
}

pub(crate) fn source_text<'a>(
    context: &'a SourceContext<'a>,
) -> Result<&'a str, RustSemanticError> {
    std::str::from_utf8(context.file.bytes())
        .map_err(|_| invalid_declaration(&context.path, 0, "utf8"))
}

pub(crate) fn parse_tree(path: &str, source: &str) -> Result<Tree, RustSemanticError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .map_err(|_| RustSemanticError::ContractInvalid)?;
    let (tree, _) = parse_rust_tree_with_compatibility(&mut parser, source, true)
        .ok_or_else(|| invalid_declaration(path, 0, "source_file"))?;
    if tree.root_node().has_error() {
        let malformed = first_malformed(tree.root_node()).unwrap_or(tree.root_node());
        return Err(invalid_declaration(
            path,
            malformed.start_byte(),
            malformed.kind(),
        ));
    }
    Ok(tree)
}

fn first_malformed(node: Node<'_>) -> Option<Node<'_>> {
    if node.is_error() || node.is_missing() {
        return Some(node);
    }
    let mut cursor = node.walk();
    node.children(&mut cursor).find_map(first_malformed)
}

#[allow(clippy::too_many_arguments)]
fn collect_owners(
    scope: Node<'_>,
    source: &str,
    context: &SourceContext<'_>,
    module_path: &str,
    module_owner_id: &str,
    inherited_presence: CompilationPresence,
    catalog: &mut OwnerCatalog,
) -> Result<(), RustSemanticError> {
    for (node, attributes) in attributed_children(scope, source, &context.path)? {
        let presence = compilation_presence(inherited_presence, &attributes);
        let direct_cfg = attributes
            .iter()
            .any(|attribute| attribute.kind == RustSemanticAttributeKind::Cfg);
        match node.kind() {
            "mod_item" => {
                let name = normalized_node_name(node, source, &context.path)?;
                let child_path = child_module_path(module_path, &name);
                let id = workspace_module_id(
                    &context.repository_identity,
                    &context.crate_id,
                    &child_path,
                );
                catalog.insert(OwnerRecord {
                    key: OwnerKey {
                        crate_id: context.crate_id.clone(),
                        module_path: child_path.clone(),
                        kind: EntityKind::RustModule,
                        name,
                    },
                    id: id.clone(),
                    source_file_id: context.source_file_id.clone(),
                    span: node_range(node),
                    visibility: visibility(node, source),
                    module_owner_id: module_owner_id.to_owned(),
                    direct_cfg,
                    attributes,
                })?;
                if let Some(body) = node.child_by_field_name("body") {
                    collect_owners(body, source, context, &child_path, &id, presence, catalog)?;
                }
            }
            "struct_item" | "enum_item" | "trait_item" => {
                let kind = match node.kind() {
                    "struct_item" => EntityKind::RustStruct,
                    "enum_item" => EntityKind::RustEnum,
                    "trait_item" => EntityKind::RustTrait,
                    _ => unreachable!(),
                };
                let name = normalized_node_name(node, source, &context.path)?;
                let id = workspace_declaration_id(
                    &context.repository_identity,
                    kind,
                    &context.crate_id,
                    module_path,
                    &name,
                );
                catalog.insert(OwnerRecord {
                    key: OwnerKey {
                        crate_id: context.crate_id.clone(),
                        module_path: module_path.to_owned(),
                        kind,
                        name,
                    },
                    id,
                    source_file_id: context.source_file_id.clone(),
                    span: node_range(node),
                    visibility: visibility(node, source),
                    module_owner_id: module_owner_id.to_owned(),
                    direct_cfg,
                    attributes,
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
    module_id: &str,
    inherited_presence: CompilationPresence,
    catalog: &OwnerCatalog,
    builder: &mut ChunkBuilder<'_>,
) -> Result<(), RustSemanticError> {
    for (node, attributes) in attributed_children(scope, source, &context.path)? {
        let presence = compilation_presence(inherited_presence, &attributes);
        match node.kind() {
            "mod_item" => {
                builder.materialize_attributes(&attributes)?;
                if let Some(body) = node.child_by_field_name("body") {
                    let name = normalized_node_name(node, source, &context.path)?;
                    let child_path = child_module_path(module_path, &name);
                    let record = catalog
                        .by_key(
                            &context.crate_id,
                            &child_path,
                            EntityKind::RustModule,
                            &name,
                        )
                        .ok_or(RustSemanticError::ContractInvalid)?;
                    process_scope(
                        body,
                        source,
                        context,
                        &child_path,
                        &record.id,
                        presence,
                        catalog,
                        builder,
                    )?;
                }
            }
            "struct_item" => process_struct(
                node,
                source,
                context,
                module_path,
                presence,
                &attributes,
                catalog,
                builder,
            )?,
            "enum_item" => process_enum(
                node,
                source,
                context,
                module_path,
                presence,
                &attributes,
                catalog,
                builder,
            )?,
            "trait_item" => process_trait(
                node,
                source,
                context,
                module_path,
                presence,
                &attributes,
                catalog,
                builder,
            )?,
            "impl_item" => process_impl(
                node,
                source,
                context,
                module_path,
                presence,
                &attributes,
                catalog,
                builder,
            )?,
            "const_item" => process_constant(
                node,
                source,
                context,
                module_path,
                module_semantic_owner(module_path, module_id, &context.crate_id),
                RustSemanticOwnerKind::Module,
                None,
                presence,
                &attributes,
                None,
                builder,
            )?,
            "static_item" => process_static(
                node,
                source,
                context,
                module_path,
                module_semantic_owner(module_path, module_id, &context.crate_id),
                presence,
                &attributes,
                builder,
            )?,
            "macro_invocation" | "macro_definition" => {
                let evidence = builder.add_evidence(node_range(node))?;
                builder.add_capability(
                    "rust.macro_generated_items_not_analyzed",
                    evidence,
                    true,
                )?;
            }
            "union_item" => {
                let evidence = builder.add_evidence(node_range(node))?;
                builder.add_capability("rust.union_unsupported", evidence, true)?;
            }
            "foreign_mod_item" => {
                let evidence = builder.add_evidence(node_range(node))?;
                builder.add_capability("rust.foreign_block_unsupported", evidence, true)?;
            }
            _ => {
                builder.materialize_attributes(&attributes)?;
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn process_struct(
    node: Node<'_>,
    source: &str,
    context: &SourceContext<'_>,
    module_path: &str,
    presence: CompilationPresence,
    attributes: &[AttributeDraft],
    catalog: &OwnerCatalog,
    builder: &mut ChunkBuilder<'_>,
) -> Result<(), RustSemanticError> {
    builder.materialize_attributes(attributes)?;
    let name = normalized_node_name(node, source, &context.path)?;
    let owner = catalog
        .by_key(
            &context.crate_id,
            module_path,
            EntityKind::RustStruct,
            &name,
        )
        .ok_or(RustSemanticError::ContractInvalid)?;
    let Some(body) = node.child_by_field_name("body") else {
        return Ok(());
    };
    match body.kind() {
        "field_declaration_list" => process_named_fields(
            body,
            source,
            context,
            module_path,
            &owner.id,
            RustSemanticOwnerKind::Struct,
            presence,
            builder,
        ),
        "ordered_field_declaration_list" => process_tuple_fields(
            body,
            source,
            context,
            module_path,
            &owner.id,
            RustSemanticOwnerKind::Struct,
            presence,
            builder,
        ),
        _ => Err(invalid_declaration(
            &context.path,
            body.start_byte(),
            body.kind(),
        )),
    }
}

#[allow(clippy::too_many_arguments)]
fn process_enum(
    node: Node<'_>,
    source: &str,
    context: &SourceContext<'_>,
    module_path: &str,
    presence: CompilationPresence,
    attributes: &[AttributeDraft],
    catalog: &OwnerCatalog,
    builder: &mut ChunkBuilder<'_>,
) -> Result<(), RustSemanticError> {
    builder.materialize_attributes(attributes)?;
    let name = normalized_node_name(node, source, &context.path)?;
    let owner = catalog
        .by_key(&context.crate_id, module_path, EntityKind::RustEnum, &name)
        .ok_or(RustSemanticError::ContractInvalid)?;
    let body = node
        .child_by_field_name("body")
        .ok_or_else(|| invalid_declaration(&context.path, node.start_byte(), "enum_item"))?;
    let variants = attributed_children(body, source, &context.path)?;
    enforce_count(RustSemanticLimit::VariantsPerEnum, variants.len())?;
    for (variant, variant_attributes) in variants {
        if variant.kind() != "enum_variant" {
            continue;
        }
        let variant_presence = compilation_presence(presence, &variant_attributes);
        let name = normalized_node_name(variant, source, &context.path)?;
        let evidence_id = builder.add_evidence(node_range(variant))?;
        let attributes = builder.materialize_attributes(&variant_attributes)?;
        let variant_body = variant.child_by_field_name("body");
        let form = match variant_body.map(|body| body.kind()) {
            None => RustSemanticForm::Unit,
            Some("ordered_field_declaration_list") => RustSemanticForm::Tuple,
            Some("field_declaration_list") => RustSemanticForm::Struct,
            Some(kind) => {
                return Err(invalid_declaration(
                    &context.path,
                    variant.start_byte(),
                    kind,
                ));
            }
        };
        let discriminant_present = variant.child_by_field_name("value").is_some();
        let entity = RustSemanticEntity::new_member(
            builder.repository_identity,
            RustSemanticEntityKind::EnumVariant,
            context.crate_id.clone(),
            module_path.to_owned(),
            name.clone(),
            &name,
            RustSemanticVisibility::NotApplicable,
            owner.id.clone(),
            None,
            variant_presence,
            RustMemberProperties {
                owner_kind: RustSemanticOwnerKind::Enum,
                form,
                declared_name: Some(name.clone()),
                tuple_index: None,
                declared_type_or_header: None,
                mutable: None,
                initializer_present: None,
                discriminant_present: Some(discriminant_present),
                bounds_present: None,
                default_present: None,
                attributes,
            },
        );
        let variant_id = entity.id.clone();
        builder.add_entity(entity, evidence_id.clone())?;
        if discriminant_present {
            builder.add_capability("rust.value_not_evaluated", evidence_id, false)?;
        }
        if let Some(variant_body) = variant_body {
            match variant_body.kind() {
                "field_declaration_list" => process_named_fields(
                    variant_body,
                    source,
                    context,
                    module_path,
                    &variant_id,
                    RustSemanticOwnerKind::EnumVariant,
                    variant_presence,
                    builder,
                )?,
                "ordered_field_declaration_list" => process_tuple_fields(
                    variant_body,
                    source,
                    context,
                    module_path,
                    &variant_id,
                    RustSemanticOwnerKind::EnumVariant,
                    variant_presence,
                    builder,
                )?,
                _ => unreachable!(),
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn process_named_fields(
    body: Node<'_>,
    source: &str,
    context: &SourceContext<'_>,
    module_path: &str,
    owner_id: &str,
    owner_kind: RustSemanticOwnerKind,
    inherited_presence: CompilationPresence,
    builder: &mut ChunkBuilder<'_>,
) -> Result<(), RustSemanticError> {
    let fields = attributed_children(body, source, &context.path)?;
    let field_count = fields
        .iter()
        .filter(|(node, _)| node.kind() == "field_declaration")
        .count();
    enforce_count(RustSemanticLimit::FieldsPerOwner, field_count)?;
    for (field, attributes) in fields {
        if field.kind() != "field_declaration" {
            continue;
        }
        let name = normalized_node_name(field, source, &context.path)?;
        let type_node = field.child_by_field_name("type").ok_or_else(|| {
            invalid_declaration(&context.path, field.start_byte(), "field_declaration")
        })?;
        let declared_type = bounded_node_text(
            type_node,
            source,
            &context.path,
            RustSemanticLimit::DeclaredTypeOrHeaderBytes,
        )?;
        let evidence_id = builder.add_evidence(node_range(field))?;
        let materialized_attributes = builder.materialize_attributes(&attributes)?;
        let entity = RustSemanticEntity::new_member(
            builder.repository_identity,
            RustSemanticEntityKind::Field,
            context.crate_id.clone(),
            module_path.to_owned(),
            name.clone(),
            &name,
            visibility(field, source),
            owner_id.to_owned(),
            None,
            compilation_presence(inherited_presence, &attributes),
            RustMemberProperties {
                owner_kind,
                form: RustSemanticForm::Named,
                declared_name: Some(name.clone()),
                tuple_index: None,
                declared_type_or_header: Some(declared_type),
                mutable: None,
                initializer_present: None,
                discriminant_present: None,
                bounds_present: None,
                default_present: None,
                attributes: materialized_attributes,
            },
        );
        builder.add_entity(entity, evidence_id.clone())?;
        builder.add_capability("rust.type_resolution_not_performed", evidence_id, false)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn process_tuple_fields(
    body: Node<'_>,
    source: &str,
    context: &SourceContext<'_>,
    module_path: &str,
    owner_id: &str,
    owner_kind: RustSemanticOwnerKind,
    inherited_presence: CompilationPresence,
    builder: &mut ChunkBuilder<'_>,
) -> Result<(), RustSemanticError> {
    let mut cursor = body.walk();
    let mut attributes = Vec::new();
    let mut pending_visibility = RustSemanticVisibility::Private;
    let mut fields = Vec::new();
    for child in body.named_children(&mut cursor) {
        match child.kind() {
            "attribute_item" => attributes.push(parse_attribute(child, source, &context.path)?),
            "visibility_modifier" => pending_visibility = visibility_text(child, source),
            _ => {
                fields.push((child, std::mem::take(&mut attributes), pending_visibility));
                pending_visibility = RustSemanticVisibility::Private;
            }
        }
    }
    enforce_count(RustSemanticLimit::FieldsPerOwner, fields.len())?;
    enforce_count(RustSemanticLimit::TupleFieldsPerOwner, fields.len())?;
    for (index, (field_type, attributes, field_visibility)) in fields.into_iter().enumerate() {
        let declared_type = bounded_node_text(
            field_type,
            source,
            &context.path,
            RustSemanticLimit::DeclaredTypeOrHeaderBytes,
        )?;
        let evidence_id = builder.add_evidence(node_range(field_type))?;
        let materialized_attributes = builder.materialize_attributes(&attributes)?;
        let tuple_index = u64::try_from(index).map_err(|_| RustSemanticError::ContractInvalid)?;
        let name = tuple_index.to_string();
        let entity = RustSemanticEntity::new_member(
            builder.repository_identity,
            RustSemanticEntityKind::Field,
            context.crate_id.clone(),
            module_path.to_owned(),
            name.clone(),
            &name,
            field_visibility,
            owner_id.to_owned(),
            None,
            compilation_presence(inherited_presence, &attributes),
            RustMemberProperties {
                owner_kind,
                form: RustSemanticForm::Tuple,
                declared_name: None,
                tuple_index: Some(tuple_index),
                declared_type_or_header: Some(declared_type),
                mutable: None,
                initializer_present: None,
                discriminant_present: None,
                bounds_present: None,
                default_present: None,
                attributes: materialized_attributes,
            },
        );
        builder.add_entity(entity, evidence_id.clone())?;
        builder.add_capability("rust.type_resolution_not_performed", evidence_id, false)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn process_trait(
    node: Node<'_>,
    source: &str,
    context: &SourceContext<'_>,
    module_path: &str,
    presence: CompilationPresence,
    attributes: &[AttributeDraft],
    catalog: &OwnerCatalog,
    builder: &mut ChunkBuilder<'_>,
) -> Result<(), RustSemanticError> {
    builder.materialize_attributes(attributes)?;
    let name = normalized_node_name(node, source, &context.path)?;
    let owner = catalog
        .by_key(&context.crate_id, module_path, EntityKind::RustTrait, &name)
        .ok_or(RustSemanticError::ContractInvalid)?;
    let body = node
        .child_by_field_name("body")
        .ok_or_else(|| invalid_declaration(&context.path, node.start_byte(), "trait_item"))?;
    let items = attributed_children(body, source, &context.path)?;
    enforce_count(RustSemanticLimit::AssociatedItemsPerContext, items.len())?;
    for (item, item_attributes) in items {
        let item_presence = compilation_presence(presence, &item_attributes);
        match item.kind() {
            "const_item" => process_constant(
                item,
                source,
                context,
                module_path,
                owner.id.clone(),
                RustSemanticOwnerKind::Trait,
                Some(owner.id.clone()),
                item_presence,
                &item_attributes,
                Some(RustSemanticVisibility::InheritedTrait),
                builder,
            )?,
            "associated_type" => process_associated_type(
                item,
                source,
                context,
                module_path,
                owner.id.clone(),
                RustSemanticOwnerKind::Trait,
                Some(owner.id.clone()),
                item_presence,
                &item_attributes,
                false,
                builder,
            )?,
            "function_signature_item" | "function_item" => process_method(
                item,
                source,
                context,
                module_path,
                owner.id.clone(),
                RustMethodContext::TraitDeclaration,
                Some(owner.id.clone()),
                item_presence,
                &item_attributes,
                RustSemanticVisibility::InheritedTrait,
                builder,
            )?,
            _ => {
                builder.materialize_attributes(&item_attributes)?;
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn process_impl(
    node: Node<'_>,
    source: &str,
    context: &SourceContext<'_>,
    module_path: &str,
    presence: CompilationPresence,
    attributes: &[AttributeDraft],
    catalog: &OwnerCatalog,
    builder: &mut ChunkBuilder<'_>,
) -> Result<(), RustSemanticError> {
    builder.materialize_attributes(attributes)?;
    if node.child_by_field_name("type_parameters").is_some()
        || node_text(node, source).trim_start().starts_with("impl !")
    {
        let evidence = builder.add_evidence(node_range(node))?;
        return builder.add_capability("rust.unsupported_impl_header", evidence, true);
    }
    let target_node = node
        .child_by_field_name("type")
        .ok_or_else(|| invalid_declaration(&context.path, node.start_byte(), "impl_item"))?;
    let target_text = node_text(target_node, source);
    let Some(target) = catalog.resolve_named(
        &context.crate_id,
        module_path,
        target_text,
        &[EntityKind::RustStruct, EntityKind::RustEnum],
    )?
    else {
        let evidence = builder.add_evidence(node_range(node))?;
        return builder.add_capability("rust.unsupported_impl_header", evidence, true);
    };
    let trait_record = if let Some(trait_node) = node.child_by_field_name("trait") {
        let trait_text = node_text(trait_node, source);
        let Some(resolved) = catalog.resolve_named(
            &context.crate_id,
            module_path,
            trait_text,
            &[EntityKind::RustTrait],
        )?
        else {
            let evidence = builder.add_evidence(node_range(node))?;
            return builder.add_capability("rust.unsupported_impl_header", evidence, true);
        };
        Some(resolved)
    } else {
        None
    };
    let impl_evidence = builder.add_evidence(node_range(node))?;
    if let Some(trait_record) = trait_record {
        builder.add_relationship(
            RelationshipKind::Implements,
            target.id.clone(),
            trait_record.id.clone(),
            impl_evidence,
        )?;
    }
    let Some(body) = node.child_by_field_name("body") else {
        return Ok(());
    };
    let items = attributed_children(body, source, &context.path)?;
    enforce_count(RustSemanticLimit::AssociatedItemsPerContext, items.len())?;
    for (item, item_attributes) in items {
        let item_presence = compilation_presence(presence, &item_attributes);
        let (owner_kind, method_context, visibility_override) = if trait_record.is_some() {
            (
                RustSemanticOwnerKind::NamedLocalTraitImplementation,
                RustMethodContext::NamedLocalTraitImplementation,
                Some(RustSemanticVisibility::InheritedTrait),
            )
        } else {
            (
                RustSemanticOwnerKind::InherentImplementation,
                RustMethodContext::InherentImplementation,
                None,
            )
        };
        let trait_context_id = trait_record.map(|record| record.id.clone());
        match item.kind() {
            "const_item" => process_constant(
                item,
                source,
                context,
                module_path,
                target.id.clone(),
                owner_kind,
                trait_context_id.clone(),
                item_presence,
                &item_attributes,
                visibility_override,
                builder,
            )?,
            "type_item" if trait_record.is_some() => process_associated_type(
                item,
                source,
                context,
                module_path,
                target.id.clone(),
                owner_kind,
                trait_context_id.clone(),
                item_presence,
                &item_attributes,
                true,
                builder,
            )?,
            "function_item" => process_method(
                item,
                source,
                context,
                module_path,
                target.id.clone(),
                method_context,
                trait_context_id.clone(),
                item_presence,
                &item_attributes,
                visibility_override.unwrap_or_else(|| visibility(item, source)),
                builder,
            )?,
            _ => {
                builder.materialize_attributes(&item_attributes)?;
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn process_constant(
    node: Node<'_>,
    source: &str,
    context: &SourceContext<'_>,
    module_path: &str,
    owner_id: String,
    owner_kind: RustSemanticOwnerKind,
    trait_context_id: Option<String>,
    presence: CompilationPresence,
    attributes: &[AttributeDraft],
    visibility_override: Option<RustSemanticVisibility>,
    builder: &mut ChunkBuilder<'_>,
) -> Result<(), RustSemanticError> {
    let name = normalized_node_name(node, source, &context.path)?;
    let type_node = node
        .child_by_field_name("type")
        .ok_or_else(|| invalid_declaration(&context.path, node.start_byte(), "const_item"))?;
    let declared_type = bounded_node_text(
        type_node,
        source,
        &context.path,
        RustSemanticLimit::DeclaredTypeOrHeaderBytes,
    )?;
    let evidence_id = builder.add_evidence(node_range(node))?;
    let materialized_attributes = builder.materialize_attributes(attributes)?;
    let initializer_present = node.child_by_field_name("value").is_some();
    let entity = RustSemanticEntity::new_member(
        builder.repository_identity,
        RustSemanticEntityKind::Constant,
        context.crate_id.clone(),
        module_path.to_owned(),
        name.clone(),
        &name,
        visibility_override.unwrap_or_else(|| visibility(node, source)),
        owner_id,
        trait_context_id,
        presence,
        RustMemberProperties {
            owner_kind,
            form: RustSemanticForm::Constant,
            declared_name: Some(name.clone()),
            tuple_index: None,
            declared_type_or_header: Some(declared_type),
            mutable: None,
            initializer_present: Some(initializer_present),
            discriminant_present: None,
            bounds_present: None,
            default_present: None,
            attributes: materialized_attributes,
        },
    );
    builder.add_entity(entity, evidence_id.clone())?;
    builder.add_capability(
        "rust.type_resolution_not_performed",
        evidence_id.clone(),
        false,
    )?;
    if initializer_present {
        builder.add_capability("rust.value_not_evaluated", evidence_id, false)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn process_static(
    node: Node<'_>,
    source: &str,
    context: &SourceContext<'_>,
    module_path: &str,
    owner_id: String,
    presence: CompilationPresence,
    attributes: &[AttributeDraft],
    builder: &mut ChunkBuilder<'_>,
) -> Result<(), RustSemanticError> {
    let name = normalized_node_name(node, source, &context.path)?;
    let type_node = node
        .child_by_field_name("type")
        .ok_or_else(|| invalid_declaration(&context.path, node.start_byte(), "static_item"))?;
    let declared_type = bounded_node_text(
        type_node,
        source,
        &context.path,
        RustSemanticLimit::DeclaredTypeOrHeaderBytes,
    )?;
    let evidence_id = builder.add_evidence(node_range(node))?;
    let materialized_attributes = builder.materialize_attributes(attributes)?;
    let initializer_present = node.child_by_field_name("value").is_some();
    let mutable = has_named_child(node, "mutable_specifier");
    let entity = RustSemanticEntity::new_member(
        builder.repository_identity,
        RustSemanticEntityKind::Static,
        context.crate_id.clone(),
        module_path.to_owned(),
        name.clone(),
        &name,
        visibility(node, source),
        owner_id,
        None,
        presence,
        RustMemberProperties {
            owner_kind: RustSemanticOwnerKind::Module,
            form: RustSemanticForm::Static,
            declared_name: Some(name.clone()),
            tuple_index: None,
            declared_type_or_header: Some(declared_type),
            mutable: Some(mutable),
            initializer_present: Some(initializer_present),
            discriminant_present: None,
            bounds_present: None,
            default_present: None,
            attributes: materialized_attributes,
        },
    );
    builder.add_entity(entity, evidence_id.clone())?;
    builder.add_capability(
        "rust.type_resolution_not_performed",
        evidence_id.clone(),
        false,
    )?;
    if initializer_present {
        builder.add_capability("rust.value_not_evaluated", evidence_id, false)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn process_associated_type(
    node: Node<'_>,
    source: &str,
    context: &SourceContext<'_>,
    module_path: &str,
    owner_id: String,
    owner_kind: RustSemanticOwnerKind,
    trait_context_id: Option<String>,
    presence: CompilationPresence,
    attributes: &[AttributeDraft],
    default_present: bool,
    builder: &mut ChunkBuilder<'_>,
) -> Result<(), RustSemanticError> {
    let name = normalized_node_name(node, source, &context.path)?;
    let header = bounded_node_text(
        node,
        source,
        &context.path,
        RustSemanticLimit::DeclaredTypeOrHeaderBytes,
    )?;
    let evidence_id = builder.add_evidence(node_range(node))?;
    let materialized_attributes = builder.materialize_attributes(attributes)?;
    let entity = RustSemanticEntity::new_member(
        builder.repository_identity,
        RustSemanticEntityKind::AssociatedType,
        context.crate_id.clone(),
        module_path.to_owned(),
        name.clone(),
        &name,
        RustSemanticVisibility::InheritedTrait,
        owner_id,
        trait_context_id,
        presence,
        RustMemberProperties {
            owner_kind,
            form: RustSemanticForm::AssociatedType,
            declared_name: Some(name.clone()),
            tuple_index: None,
            declared_type_or_header: Some(header),
            mutable: None,
            initializer_present: None,
            discriminant_present: None,
            bounds_present: Some(node.child_by_field_name("bounds").is_some()),
            default_present: Some(default_present),
            attributes: materialized_attributes,
        },
    );
    builder.add_entity(entity, evidence_id.clone())?;
    builder.add_capability("rust.type_resolution_not_performed", evidence_id, false)
}

#[allow(clippy::too_many_arguments)]
fn process_method(
    node: Node<'_>,
    source: &str,
    context: &SourceContext<'_>,
    module_path: &str,
    owner_id: String,
    implementation_context: RustMethodContext,
    trait_context_id: Option<String>,
    presence: CompilationPresence,
    attributes: &[AttributeDraft],
    method_visibility: RustSemanticVisibility,
    builder: &mut ChunkBuilder<'_>,
) -> Result<(), RustSemanticError> {
    let name_node = node
        .child_by_field_name("name")
        .ok_or_else(|| invalid_declaration(&context.path, node.start_byte(), node.kind()))?;
    let raw_name = node_text(name_node, source);
    let name = normalize_identifier(raw_name);
    if name.is_empty() {
        return Err(invalid_declaration(
            &context.path,
            node.start_byte(),
            node.kind(),
        ));
    }
    let signature = method_signature(node, source, &context.path)?;
    let evidence_id = builder.add_evidence(node_range(node))?;
    let materialized_attributes = builder.materialize_attributes(attributes)?;
    let parameters = node
        .child_by_field_name("parameters")
        .ok_or_else(|| invalid_declaration(&context.path, node.start_byte(), node.kind()))?;
    let entity = RustSemanticEntity::new_method(
        builder.repository_identity,
        context.crate_id.clone(),
        module_path.to_owned(),
        name,
        method_visibility,
        owner_id,
        RustMethodProperties {
            implementation_context,
            trait_context_id,
            receiver_present: has_named_descendant(parameters, "self_parameter"),
            declared_signature: signature,
            compilation_presence: presence,
            attributes: materialized_attributes,
        },
    );
    builder.add_method_entity(entity, evidence_id.clone(), raw_name)?;
    builder.add_capability("rust.type_resolution_not_performed", evidence_id, false)
}

fn aggregate_graph(
    chunks: &[RustSemanticSourceChunk],
) -> Result<RustSemanticGraph, RustSemanticError> {
    let mut legacy_entities = BTreeMap::new();
    let mut entities = BTreeMap::new();
    let mut relationships = BTreeMap::new();
    let mut claims = BTreeMap::new();
    let mut evidence = BTreeMap::new();
    let mut diagnostics = BTreeMap::new();
    let mut coverage = BTreeMap::new();
    for chunk in chunks {
        extend_unique(&mut legacy_entities, &chunk.legacy_entities, |value| {
            &value.id
        })?;
        extend_semantic_entities(&mut entities, &chunk.entities)?;
        extend_unique(&mut relationships, &chunk.relationships, |value| &value.id)?;
        extend_unique(&mut claims, &chunk.claims, |value| &value.id)?;
        for value in &chunk.evidence {
            evidence
                .entry(value.id.clone())
                .or_insert_with(|| value.clone());
        }
        for value in &chunk.diagnostics {
            diagnostics
                .entry(value.id.clone())
                .or_insert_with(|| value.clone());
        }
        for value in &chunk.coverage {
            coverage
                .entry(value.id.clone())
                .or_insert_with(|| value.clone());
        }
    }
    let entities = entities.into_values().collect::<Vec<_>>();
    let index = RustSemanticIndex {
        member_entity_ids: entities.iter().map(|entity| entity.id.clone()).collect(),
        implementation_context_method_ids: entities
            .iter()
            .filter(|entity| entity.kind == RustSemanticEntityKind::Method)
            .map(|entity| entity.id.clone())
            .collect(),
    };
    Ok(RustSemanticGraph {
        legacy_entities: legacy_entities.into_values().collect(),
        entities,
        relationships: relationships.into_values().collect(),
        claims: claims.into_values().collect(),
        evidence: evidence.into_values().collect(),
        diagnostics: diagnostics.into_values().collect(),
        coverage: coverage.into_values().collect(),
        index,
    })
}

fn extend_semantic_entities(
    target: &mut BTreeMap<String, RustSemanticEntity>,
    values: &[RustSemanticEntity],
) -> Result<(), RustSemanticError> {
    for value in values {
        match target.entry(value.id.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(value.clone());
            }
            Entry::Occupied(_) => {
                return Err(RustSemanticError::IdentityConflict {
                    owner_id: value.owner_id.clone(),
                    member_kind: value.kind.as_str().to_owned(),
                    normalized_member: value.identity_member(),
                });
            }
        }
    }
    Ok(())
}

fn extend_unique<T: Clone>(
    target: &mut BTreeMap<String, T>,
    values: &[T],
    identifier: impl Fn(&T) -> &String,
) -> Result<(), RustSemanticError> {
    for value in values {
        if target
            .insert(identifier(value).clone(), value.clone())
            .is_some()
        {
            return Err(RustSemanticError::ContractInvalid);
        }
    }
    Ok(())
}

fn attributed_children<'tree>(
    parent: Node<'tree>,
    source: &str,
    path: &str,
) -> Result<Vec<(Node<'tree>, Vec<AttributeDraft>)>, RustSemanticError> {
    let mut cursor = parent.walk();
    let mut pending = Vec::new();
    let mut values = Vec::new();
    for child in parent.named_children(&mut cursor) {
        match child.kind() {
            "attribute_item" => {
                enforce_count(
                    RustSemanticLimit::OuterAttributesPerDeclaration,
                    pending.len().saturating_add(1),
                )?;
                pending.push(parse_attribute(child, source, path)?);
            }
            "line_comment" | "block_comment" | "inner_attribute_item" => {}
            _ => {
                let mut attributes = std::mem::take(&mut pending);
                let mut child_cursor = child.walk();
                for direct in child.named_children(&mut child_cursor) {
                    if direct.kind() == "attribute_item" {
                        enforce_count(
                            RustSemanticLimit::OuterAttributesPerDeclaration,
                            attributes.len().saturating_add(1),
                        )?;
                        attributes.push(parse_attribute(direct, source, path)?);
                    }
                }
                enforce_count(
                    RustSemanticLimit::OuterAttributesPerDeclaration,
                    attributes.len(),
                )?;
                values.push((child, attributes));
            }
        }
    }
    Ok(values)
}

fn parse_attribute(
    node: Node<'_>,
    source: &str,
    path: &str,
) -> Result<AttributeDraft, RustSemanticError> {
    let token_text = node_text(node, source);
    enforce_bytes(RustSemanticLimit::AttributeTokenBytes, token_text.len())?;
    let compact = token_text
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    let kind = if compact.starts_with("#[cfg(") {
        RustSemanticAttributeKind::Cfg
    } else if compact.starts_with("#[cfg_attr(") {
        RustSemanticAttributeKind::CfgAttr
    } else {
        RustSemanticAttributeKind::Other
    };
    if token_text.is_empty() {
        return Err(invalid_declaration(
            path,
            node.start_byte(),
            "attribute_item",
        ));
    }
    Ok(AttributeDraft {
        kind,
        token_text: token_text.to_owned(),
        span: node_range(node),
    })
}

fn compilation_presence(
    inherited: CompilationPresence,
    attributes: &[AttributeDraft],
) -> CompilationPresence {
    if attributes
        .iter()
        .any(|attribute| attribute.kind == RustSemanticAttributeKind::CfgAttr)
    {
        CompilationPresence::AttributeTransformUnknown
    } else if attributes
        .iter()
        .any(|attribute| attribute.kind == RustSemanticAttributeKind::Cfg)
    {
        CompilationPresence::ConditionalUnknown
    } else {
        inherited
    }
}

fn normalized_node_name(
    node: Node<'_>,
    source: &str,
    path: &str,
) -> Result<String, RustSemanticError> {
    node.child_by_field_name("name")
        .map(|name| normalize_identifier(node_text(name, source)))
        .filter(|name| !name.is_empty())
        .ok_or_else(|| invalid_declaration(path, node.start_byte(), node.kind()))
}

fn normalize_identifier(value: &str) -> String {
    value.strip_prefix("r#").unwrap_or(value).nfc().collect()
}

fn node_text<'a>(node: Node<'_>, source: &'a str) -> &'a str {
    node.utf8_text(source.as_bytes()).unwrap_or("")
}

fn bounded_node_text(
    node: Node<'_>,
    source: &str,
    path: &str,
    limit: RustSemanticLimit,
) -> Result<String, RustSemanticError> {
    let text = node_text(node, source).trim().to_owned();
    if text.is_empty() {
        return Err(invalid_declaration(path, node.start_byte(), node.kind()));
    }
    enforce_bytes(limit, text.len())?;
    Ok(text)
}

fn method_signature(node: Node<'_>, source: &str, path: &str) -> Result<String, RustSemanticError> {
    let end = node
        .child_by_field_name("body")
        .map_or(node.end_byte(), |body| body.start_byte());
    let text = source
        .get(node.start_byte()..end)
        .ok_or_else(|| invalid_declaration(path, node.start_byte(), node.kind()))?
        .trim()
        .trim_end_matches(';')
        .trim()
        .to_owned();
    if text.is_empty() {
        return Err(invalid_declaration(path, node.start_byte(), node.kind()));
    }
    enforce_bytes(RustSemanticLimit::DeclaredTypeOrHeaderBytes, text.len())?;
    Ok(text)
}

fn visibility(node: Node<'_>, source: &str) -> RustSemanticVisibility {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() == "visibility_modifier")
        .map_or(RustSemanticVisibility::Private, |child| {
            visibility_text(child, source)
        })
}

fn visibility_text(node: Node<'_>, source: &str) -> RustSemanticVisibility {
    match node_text(node, source).trim() {
        "pub" => RustSemanticVisibility::Public,
        "pub(crate)" => RustSemanticVisibility::Crate,
        value if value.starts_with("pub(") => RustSemanticVisibility::Restricted,
        _ => RustSemanticVisibility::Private,
    }
}

fn workspace_visibility(visibility: RustSemanticVisibility) -> WorkspaceVisibility {
    match visibility {
        RustSemanticVisibility::Public => WorkspaceVisibility::Public,
        RustSemanticVisibility::InheritedTrait => WorkspaceVisibility::InheritedTrait,
        RustSemanticVisibility::NotApplicable => WorkspaceVisibility::NotApplicable,
        RustSemanticVisibility::Crate
        | RustSemanticVisibility::Restricted
        | RustSemanticVisibility::Private => WorkspaceVisibility::Private,
    }
}

fn module_semantic_owner(module_path: &str, module_id: &str, crate_id: &str) -> String {
    if module_path == "crate" {
        crate_id.to_owned()
    } else {
        module_id.to_owned()
    }
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
    Some((module, name))
}

fn has_named_child(node: Node<'_>, kind: &str) -> bool {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .any(|child| child.kind() == kind)
}

fn has_named_descendant(node: Node<'_>, kind: &str) -> bool {
    if node.kind() == kind {
        return true;
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .any(|child| has_named_descendant(child, kind))
}

fn enforce_count(limit: RustSemanticLimit, observed: usize) -> Result<(), RustSemanticError> {
    let observed = u64::try_from(observed).unwrap_or(u64::MAX);
    if observed > limit.maximum() {
        return Err(rust_semantic_limit_exceeded(limit, observed));
    }
    Ok(())
}

fn enforce_bytes(limit: RustSemanticLimit, observed: usize) -> Result<(), RustSemanticError> {
    enforce_count(limit, observed)
}

fn node_range(node: Node<'_>) -> ByteRange {
    ByteRange {
        start: node.start_byte(),
        end: node.end_byte(),
    }
}

fn invalid_declaration(path: &str, start_byte: usize, kind: &str) -> RustSemanticError {
    RustSemanticError::InvalidDeclaration {
        path: path.to_owned(),
        start_byte: u64::try_from(start_byte).unwrap_or(u64::MAX),
        declaration_kind: kind.to_owned(),
    }
}

fn record_kind(kind: EntityKind) -> &'static str {
    match kind {
        EntityKind::RustModule => "rust.module",
        EntityKind::RustStruct => "rust.struct",
        EntityKind::RustEnum => "rust.enum",
        EntityKind::RustTrait => "rust.trait",
        _ => "rust.declaration",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sec_fr_ext_010_cross_file_cfg_owner_alternatives_remain_conflicts() {
        let mut catalog = OwnerCatalog::new();
        catalog
            .insert(cfg_owner_record("source-a", 10, 40))
            .expect("first cfg owner is valid");
        let error = catalog
            .insert(cfg_owner_record("source-b", 50, 80))
            .expect_err("cross-file cfg owner must remain a conflict");
        assert!(matches!(
            error,
            RustSemanticError::IdentityConflict {
                member_kind,
                normalized_member,
                ..
            } if member_kind == "rust.struct" && normalized_member == "ConditionalOwner"
        ));
    }

    #[test]
    fn sec_fr_ext_010_cross_chunk_cfg_member_alternatives_remain_conflicts() {
        let entity = RustSemanticEntity::new_member(
            "urn:codenoesis:test:cross-chunk-cfg-member",
            RustSemanticEntityKind::Constant,
            "crate-id".to_owned(),
            "crate".to_owned(),
            "BIN_DATA".to_owned(),
            "BIN_DATA",
            RustSemanticVisibility::Private,
            "module-owner-id".to_owned(),
            None,
            CompilationPresence::ConditionalUnknown,
            RustMemberProperties {
                owner_kind: RustSemanticOwnerKind::Module,
                form: RustSemanticForm::Constant,
                declared_name: Some("BIN_DATA".to_owned()),
                tuple_index: None,
                declared_type_or_header: Some("&[u8]".to_owned()),
                mutable: None,
                initializer_present: Some(true),
                discriminant_present: None,
                bounds_present: None,
                default_present: None,
                attributes: Vec::new(),
            },
        );
        let mut entities = BTreeMap::new();
        extend_semantic_entities(&mut entities, std::slice::from_ref(&entity))
            .expect("first cfg member chunk is valid");
        let error = extend_semantic_entities(&mut entities, &[entity])
            .expect_err("cross-chunk cfg member must remain a conflict");
        assert!(matches!(
            error,
            RustSemanticError::IdentityConflict {
                member_kind,
                normalized_member,
                ..
            } if member_kind == "rust.constant" && normalized_member == "BIN_DATA"
        ));
    }

    fn cfg_owner_record(source_file_id: &str, start: usize, end: usize) -> OwnerRecord {
        OwnerRecord {
            key: OwnerKey {
                crate_id: "crate-id".to_owned(),
                module_path: "crate".to_owned(),
                kind: EntityKind::RustStruct,
                name: "ConditionalOwner".to_owned(),
            },
            id: "owner-id".to_owned(),
            source_file_id: source_file_id.to_owned(),
            span: ByteRange { start, end },
            visibility: RustSemanticVisibility::Public,
            module_owner_id: "module-owner-id".to_owned(),
            direct_cfg: true,
            attributes: vec![AttributeDraft {
                kind: RustSemanticAttributeKind::Cfg,
                token_text: "#[cfg(feature = \"desktop\")]".to_owned(),
                span: ByteRange {
                    start: start.saturating_sub(5),
                    end: start,
                },
            }],
        }
    }
}
