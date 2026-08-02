use std::collections::{BTreeMap, BTreeSet};

use codenoesis_domain::knowledge::{ClaimState, ClaimSubjectKind, EntityKind, RelationshipKind};
use codenoesis_domain::s4::{
    MAX_S4_WORKSPACE_CRATES, RustWorkspaceKnowledge, S4_ONTOLOGY_VERSION,
    S4_TREE_SITTER_EXTRACTOR_VERSION, S4_WORKSPACE_EXTRACTOR_VERSION, WorkspaceClaim,
    WorkspaceCoverageGap, WorkspaceDiagnostic, WorkspaceEntity, WorkspaceError, WorkspaceEvidence,
    WorkspaceExtractionChunk, WorkspaceKnowledgeGraph, WorkspaceRelationship, WorkspaceVisibility,
    workspace_source_file_id,
};
use codenoesis_domain::s4_r3::{
    ExternalWorkspaceBoundary, RootPackageWorkspaceError, RootPackageWorkspaceExtraction,
    RootPackageWorkspaceKnowledge, RootPackageWorkspacePlan,
};
use codenoesis_domain::s5::{
    AnalysisCacheEntry, AnalysisCacheKey, IncrementalWorkspaceExtraction,
    RustDeclarationObservation, RustModuleObservation, RustSourceAnalysis, SourceAnalysisRecord,
};
use codenoesis_domain::{
    ContentKind, InventoryFile, RepositoryInventory, STANDARD_LOCAL_S1_LIMITS,
};
use codenoesis_ports::{
    IncrementalRustWorkspaceExtractor, RootPackageWorkspaceExtractor, RustWorkspaceExtractor,
};
use tree_sitter::{Node, Parser};
use unicode_normalization::UnicodeNormalization as _;

use crate::root_package::{
    PlannedCoverage, PlannedRootPackageWorkspace, plan_root_package_workspace,
};

#[derive(Clone, Copy, Debug, Default)]
pub struct TreeSitterRustWorkspaceExtractor;

impl TreeSitterRustWorkspaceExtractor {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl RustWorkspaceExtractor for TreeSitterRustWorkspaceExtractor {
    fn extract_workspace(
        &self,
        inventory: &RepositoryInventory,
    ) -> Result<RustWorkspaceKnowledge, WorkspaceError> {
        WorkspaceBuilder::new(inventory)?.extract()
    }
}

impl IncrementalRustWorkspaceExtractor for TreeSitterRustWorkspaceExtractor {
    fn extract_workspace_incremental(
        &self,
        inventory: &RepositoryInventory,
        cache_entries: &[AnalysisCacheEntry],
    ) -> Result<IncrementalWorkspaceExtraction, WorkspaceError> {
        WorkspaceBuilder::new(inventory)?.extract_incremental(cache_entries)
    }
}

impl RootPackageWorkspaceExtractor for TreeSitterRustWorkspaceExtractor {
    fn extract_root_package_workspace_incremental(
        &self,
        inventory: &RepositoryInventory,
        external_boundaries: &[ExternalWorkspaceBoundary],
        cache_entries: &[AnalysisCacheEntry],
    ) -> Result<RootPackageWorkspaceExtraction, RootPackageWorkspaceError> {
        let planned = plan_root_package_workspace(inventory, external_boundaries)?;
        WorkspaceBuilder::new(inventory)
            .map_err(RootPackageWorkspaceError::Source)?
            .extract_root_package_incremental(cache_entries, planned)
    }
}

struct WorkspaceBuilder<'a> {
    inventory: &'a RepositoryInventory,
    files: BTreeMap<&'a str, &'a InventoryFile>,
    repository_identity: &'a str,
    commit_oid: &'a str,
    root_evidence: WorkspaceEvidence,
}

#[derive(Clone)]
struct CrateDraft {
    entity: WorkspaceEntity,
    manifest_evidence: WorkspaceEvidence,
    root_source_path: String,
    root_module_name: String,
    build_script: bool,
}

struct TargetDraft {
    kind: &'static str,
    name: String,
    path: String,
}

#[derive(Clone)]
struct SourceDraft {
    crate_id: String,
    crate_manifest_evidence_id: String,
    path: String,
    source: String,
    evidence: WorkspaceEvidence,
    parent_evidence_id: Option<String>,
    module_path: String,
    module_name: String,
    module_visibility: WorkspaceVisibility,
    root: bool,
}

#[derive(Clone)]
struct ParsedSource {
    declarations: Vec<DeclarationDraft>,
    modules: Vec<ModuleDeclaration>,
    imports: Vec<ImportDraft>,
    unsupported_construct: bool,
}

struct ResolvedAnalysis {
    key: AnalysisCacheKey,
    analysis: RustSourceAnalysis,
    reused: bool,
}

#[derive(Clone)]
struct DeclarationDraft {
    kind: EntityKind,
    name: String,
    visibility: WorkspaceVisibility,
}

#[derive(Clone)]
struct ModuleDeclaration {
    name: String,
    visibility: WorkspaceVisibility,
    body: Option<Box<ParsedSource>>,
}

#[derive(Clone)]
struct ImportDraft {
    spelling: String,
}

#[derive(Clone)]
struct RelationshipDraft {
    relationship: WorkspaceRelationship,
    state: ClaimState,
    source_path: String,
}

struct ModuleView<'a> {
    source: &'a SourceDraft,
    module_path: String,
    module_name: String,
    visibility: WorkspaceVisibility,
    parsed: &'a ParsedSource,
    inline: bool,
}

impl<'a> WorkspaceBuilder<'a> {
    fn new(inventory: &'a RepositoryInventory) -> Result<Self, WorkspaceError> {
        let files = inventory
            .files()
            .iter()
            .map(|file| (file.path(), file))
            .collect::<BTreeMap<_, _>>();
        let root_manifest = files
            .get("Cargo.toml")
            .copied()
            .ok_or(WorkspaceError::UnsupportedWorkspace)?;
        let root_evidence = complete_evidence(inventory, root_manifest)?;
        Ok(Self {
            inventory,
            files,
            repository_identity: inventory.bound_revision().repository_identity().as_str(),
            commit_oid: inventory.bound_revision().commit_oid().as_str(),
            root_evidence,
        })
    }

    fn extract(self) -> Result<RustWorkspaceKnowledge, WorkspaceError> {
        self.extract_incremental(&[])
            .map(|extraction| extraction.knowledge)
    }

    #[allow(clippy::too_many_lines)]
    fn extract_incremental(
        self,
        cache_entries: &[AnalysisCacheEntry],
    ) -> Result<IncrementalWorkspaceExtraction, WorkspaceError> {
        let crates = self.parse_crates()?;
        self.extract_prepared(cache_entries, &crates, None)
    }

    fn extract_root_package_incremental(
        self,
        cache_entries: &[AnalysisCacheEntry],
        planned: PlannedRootPackageWorkspace,
    ) -> Result<RootPackageWorkspaceExtraction, RootPackageWorkspaceError> {
        let crates = self
            .root_package_crates(&planned.plan)
            .map_err(RootPackageWorkspaceError::Source)?;
        let extraction = self
            .extract_prepared(cache_entries, &crates, Some(&planned.coverage))
            .map_err(RootPackageWorkspaceError::Source)?;
        let knowledge = RootPackageWorkspaceKnowledge {
            plan: planned.plan,
            knowledge: extraction.knowledge,
        };
        knowledge.validate()?;
        Ok(RootPackageWorkspaceExtraction {
            knowledge,
            cache_entries: Vec::new(),
            source_records: extraction.source_records,
            parser_invocation_count: extraction.parser_invocation_count,
        })
    }

    #[allow(clippy::too_many_lines)]
    fn extract_prepared(
        self,
        cache_entries: &[AnalysisCacheEntry],
        crates: &[CrateDraft],
        r3_coverage: Option<&[PlannedCoverage]>,
    ) -> Result<IncrementalWorkspaceExtraction, WorkspaceError> {
        if cache_entries
            .iter()
            .any(|entry| !entry.is_self_consistent())
        {
            return Err(WorkspaceError::ContractInvalid);
        }
        let cache_by_id = cache_entries
            .iter()
            .map(|entry| (entry.analysis_cache_entry_id.as_str(), entry))
            .collect::<BTreeMap<_, _>>();
        if cache_by_id.len() != cache_entries.len() {
            return Err(WorkspaceError::ContractInvalid);
        }
        let mut sources = Vec::new();
        let mut analyses = BTreeMap::new();
        let mut parser_invocation_count = 0_u64;
        let tolerate_unsupported_rust = r3_coverage.is_some();
        for crate_draft in crates {
            self.collect_crate_sources(
                crate_draft,
                &cache_by_id,
                &mut analyses,
                &mut parser_invocation_count,
                &mut sources,
                tolerate_unsupported_rust,
            )?;
        }
        sources.sort_by(|left, right| left.path.as_bytes().cmp(right.path.as_bytes()));

        let parsed = sources
            .iter()
            .map(|source| {
                analyses
                    .get(&source.path)
                    .map(|resolved: &ResolvedAnalysis| parsed_from_analysis(&resolved.analysis))
                    .ok_or(WorkspaceError::ContractInvalid)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut modules = Vec::new();
        for (source, parsed_source) in sources.iter().zip(&parsed) {
            collect_module_views(source, parsed_source, &mut modules);
        }
        let mut entities = BTreeMap::<String, WorkspaceEntity>::new();
        let mut entity_evidence = BTreeMap::<String, Vec<String>>::new();
        let mut source_entity_ids = BTreeMap::<String, String>::new();
        let mut module_entity_ids = BTreeMap::<(String, String), String>::new();
        let mut declaration_ids = BTreeMap::<(String, String, EntityKind, String), String>::new();
        let mut declarations_by_name = BTreeMap::<(String, String, String), Vec<String>>::new();
        let mut entity_source_paths = BTreeMap::<String, String>::new();

        for crate_draft in crates {
            let crate_id = crate_draft.entity.id.clone();
            insert_unique(
                &mut entity_evidence,
                crate_id.clone(),
                vec![
                    self.root_evidence.id.clone(),
                    crate_draft.manifest_evidence.id.clone(),
                ],
            )?;
            insert_unique(&mut entities, crate_id, crate_draft.entity.clone())?;
        }

        for source in &sources {
            let file = self
                .files
                .get(source.path.as_str())
                .copied()
                .ok_or(WorkspaceError::ContractInvalid)?;
            let source_entity = WorkspaceEntity::source_file(
                self.repository_identity,
                &source.crate_id,
                &source.path,
                file.blob_oid().as_str(),
            );
            insert_unique(
                &mut source_entity_ids,
                source.path.clone(),
                source_entity.id.clone(),
            )?;
            insert_unique(
                &mut entity_source_paths,
                source_entity.id.clone(),
                source.path.clone(),
            )?;
            insert_unique(
                &mut entity_evidence,
                source_entity.id.clone(),
                source_entity_evidence(source)?,
            )?;
            insert_unique(&mut entities, source_entity.id.clone(), source_entity)?;
        }

        for module in &modules {
            let source_entity_id = source_entity_ids
                .get(&module.source.path)
                .ok_or(WorkspaceError::ContractInvalid)?;
            let module_entity = WorkspaceEntity::module(
                self.repository_identity,
                &module.source.crate_id,
                &module.module_path,
                &module.module_name,
                module.visibility,
                source_entity_id,
            );
            insert_unique(
                &mut module_entity_ids,
                (module.source.crate_id.clone(), module.module_path.clone()),
                module_entity.id.clone(),
            )?;
            insert_unique(
                &mut entity_source_paths,
                module_entity.id.clone(),
                module.source.path.clone(),
            )?;
            insert_unique(
                &mut entity_evidence,
                module_entity.id.clone(),
                if module.inline {
                    vec![module.source.evidence.id.clone()]
                } else {
                    source_entity_evidence(module.source)?
                },
            )?;
            insert_unique(&mut entities, module_entity.id.clone(), module_entity)?;

            for declaration in &module.parsed.declarations {
                let entity = WorkspaceEntity::declaration(
                    self.repository_identity,
                    declaration.kind,
                    &module.source.crate_id,
                    &module.module_path,
                    &declaration.name,
                    declaration.visibility,
                );
                insert_unique(
                    &mut declaration_ids,
                    (
                        module.source.crate_id.clone(),
                        module.module_path.clone(),
                        declaration.kind,
                        declaration.name.clone(),
                    ),
                    entity.id.clone(),
                )?;
                declarations_by_name
                    .entry((
                        module.source.crate_id.clone(),
                        module.module_path.clone(),
                        declaration.name.clone(),
                    ))
                    .or_default()
                    .push(entity.id.clone());
                insert_unique(
                    &mut entity_source_paths,
                    entity.id.clone(),
                    module.source.path.clone(),
                )?;
                insert_unique(
                    &mut entity_evidence,
                    entity.id.clone(),
                    vec![module.source.evidence.id.clone()],
                )?;
                insert_unique(&mut entities, entity.id.clone(), entity)?;
            }
        }

        let mut relationship_drafts = Vec::new();
        let mut diagnostics = Vec::new();
        let mut coverage = Vec::new();
        for source in &sources {
            let source_entity_id = source_entity_ids
                .get(&source.path)
                .ok_or(WorkspaceError::ContractInvalid)?;
            let module_entity_id = module_entity_ids
                .get(&(source.crate_id.clone(), source.module_path.clone()))
                .ok_or(WorkspaceError::ContractInvalid)?;
            relationship_drafts.push(RelationshipDraft {
                relationship: WorkspaceRelationship::new(
                    RelationshipKind::Defines,
                    source_entity_id.clone(),
                    module_entity_id.clone(),
                    if source.root {
                        vec![source.evidence.id.clone()]
                    } else {
                        vec![
                            source
                                .parent_evidence_id
                                .clone()
                                .ok_or(WorkspaceError::ContractInvalid)?,
                            source.evidence.id.clone(),
                        ]
                    },
                ),
                state: ClaimState::DerivedFact,
                source_path: source.path.clone(),
            });
            relationship_drafts.push(RelationshipDraft {
                relationship: WorkspaceRelationship::new(
                    RelationshipKind::Contains,
                    source.crate_id.clone(),
                    source_entity_id.clone(),
                    if source.root {
                        vec![
                            source.crate_manifest_evidence_id.clone(),
                            source.evidence.id.clone(),
                        ]
                    } else {
                        vec![
                            source.crate_manifest_evidence_id.clone(),
                            source
                                .parent_evidence_id
                                .clone()
                                .ok_or(WorkspaceError::ContractInvalid)?,
                            source.evidence.id.clone(),
                        ]
                    },
                ),
                state: ClaimState::DerivedFact,
                source_path: source.path.clone(),
            });
        }

        for module in &modules {
            let module_entity_id = module_entity_ids
                .get(&(module.source.crate_id.clone(), module.module_path.clone()))
                .ok_or(WorkspaceError::ContractInvalid)?;
            if module.inline {
                let source_entity_id = source_entity_ids
                    .get(&module.source.path)
                    .ok_or(WorkspaceError::ContractInvalid)?;
                relationship_drafts.push(RelationshipDraft {
                    relationship: WorkspaceRelationship::new(
                        RelationshipKind::Defines,
                        source_entity_id.clone(),
                        module_entity_id.clone(),
                        vec![module.source.evidence.id.clone()],
                    ),
                    state: ClaimState::DerivedFact,
                    source_path: module.source.path.clone(),
                });
            }

            for declaration in &module.parsed.declarations {
                let declaration_id = declaration_ids
                    .get(&(
                        module.source.crate_id.clone(),
                        module.module_path.clone(),
                        declaration.kind,
                        declaration.name.clone(),
                    ))
                    .ok_or(WorkspaceError::ContractInvalid)?;
                relationship_drafts.push(RelationshipDraft {
                    relationship: WorkspaceRelationship::new(
                        RelationshipKind::Defines,
                        module_entity_id.clone(),
                        declaration_id.clone(),
                        vec![module.source.evidence.id.clone()],
                    ),
                    state: ClaimState::DeterministicFact,
                    source_path: module.source.path.clone(),
                });
            }

            for declaration in &module.parsed.modules {
                let child_path = child_module_path(&module.module_path, &declaration.name);
                let child_module_id = module_entity_ids
                    .get(&(module.source.crate_id.clone(), child_path.clone()))
                    .ok_or(WorkspaceError::UnsupportedWorkspace)?;
                let child_module = modules
                    .iter()
                    .find(|candidate| {
                        candidate.source.crate_id == module.source.crate_id
                            && candidate.module_path == child_path
                    })
                    .ok_or(WorkspaceError::ContractInvalid)?;
                relationship_drafts.push(RelationshipDraft {
                    relationship: WorkspaceRelationship::new(
                        RelationshipKind::Defines,
                        module_entity_id.clone(),
                        child_module_id.clone(),
                        vec![
                            module.source.evidence.id.clone(),
                            child_module.source.evidence.id.clone(),
                        ],
                    ),
                    state: ClaimState::DeterministicFact,
                    source_path: module.source.path.clone(),
                });
            }

            for import in &module.parsed.imports {
                let segments = import.spelling.split("::").collect::<Vec<_>>();
                let local_target = if segments.len() == 2 {
                    let target_module = child_module_path(&module.module_path, segments[0]);
                    declarations_by_name
                        .get(&(
                            module.source.crate_id.clone(),
                            target_module,
                            segments[1].to_owned(),
                        ))
                        .filter(|candidates| candidates.len() == 1)
                        .and_then(|candidates| candidates.first())
                        .cloned()
                } else {
                    None
                };
                if let Some(target) = local_target {
                    let target_source = entity_source_paths
                        .get(&target)
                        .and_then(|path| sources.iter().find(|candidate| &candidate.path == path))
                        .ok_or(WorkspaceError::ContractInvalid)?;
                    relationship_drafts.push(RelationshipDraft {
                        relationship: WorkspaceRelationship::new(
                            RelationshipKind::Imports,
                            module_entity_id.clone(),
                            target,
                            vec![
                                module.source.evidence.id.clone(),
                                target_source.evidence.id.clone(),
                            ],
                        ),
                        state: ClaimState::DerivedFact,
                        source_path: module.source.path.clone(),
                    });
                } else {
                    let symbol = WorkspaceEntity::unresolved_symbol(
                        self.repository_identity,
                        &module.source.crate_id,
                        &module.module_path,
                        &import.spelling,
                    );
                    insert_unique(
                        &mut entity_source_paths,
                        symbol.id.clone(),
                        module.source.path.clone(),
                    )?;
                    insert_unique(
                        &mut entity_evidence,
                        symbol.id.clone(),
                        vec![module.source.evidence.id.clone()],
                    )?;
                    relationship_drafts.push(RelationshipDraft {
                        relationship: WorkspaceRelationship::new(
                            RelationshipKind::Imports,
                            module_entity_id.clone(),
                            symbol.id.clone(),
                            vec![module.source.evidence.id.clone()],
                        ),
                        state: ClaimState::DeterministicFact,
                        source_path: module.source.path.clone(),
                    });
                    insert_unique(&mut entities, symbol.id.clone(), symbol)?;
                    diagnostics.push((
                        module.source.path.clone(),
                        WorkspaceDiagnostic {
                            code: "rust.unresolved_cross_crate_use".to_owned(),
                            message:
                                "compiler-grade cross-crate Rust use resolution is unavailable"
                                    .to_owned(),
                            evidence_ids: vec![module.source.evidence.id.clone()],
                        },
                    ));
                    coverage.push((
                        module.source.path.clone(),
                        WorkspaceCoverageGap::unsupported(
                            self.repository_identity,
                            self.commit_oid,
                            "compiler_cross_crate_use_resolution",
                            &module.source.evidence.id,
                        ),
                    ));
                }
            }
        }

        let mut unsupported_paths = BTreeSet::new();
        for module in &modules {
            if module.parsed.unsupported_construct
                && unsupported_paths.insert(module.source.path.clone())
            {
                diagnostics.push((
                    module.source.path.clone(),
                    WorkspaceDiagnostic {
                        code: "rust.unsupported_construct".to_owned(),
                        message: "unsupported Rust syntax was excluded from extraction".to_owned(),
                        evidence_ids: vec![module.source.evidence.id.clone()],
                    },
                ));
                coverage.push((
                    module.source.path.clone(),
                    WorkspaceCoverageGap::unsupported(
                        self.repository_identity,
                        self.commit_oid,
                        "rust_unsupported_construct",
                        &module.source.evidence.id,
                    ),
                ));
            }
        }
        if let Some(planned_coverage) = r3_coverage {
            for planned in planned_coverage {
                let manifest = self
                    .files
                    .get(planned.manifest_path.as_str())
                    .copied()
                    .ok_or(WorkspaceError::ContractInvalid)?;
                let evidence = complete_evidence(self.inventory, manifest)?;
                coverage.push((
                    planned.source_path.clone(),
                    WorkspaceCoverageGap::unsupported(
                        self.repository_identity,
                        self.commit_oid,
                        planned.capability,
                        &evidence.id,
                    ),
                ));
            }
        } else {
            let mut build_scripts = BTreeSet::new();
            for crate_draft in crates {
                if crate_draft.build_script
                    && build_scripts.insert(crate_draft.manifest_evidence.id.clone())
                {
                    coverage.push((
                        crate_draft.root_source_path.clone(),
                        WorkspaceCoverageGap::unsupported(
                            self.repository_identity,
                            self.commit_oid,
                            "build_script_execution_forbidden",
                            &crate_draft.manifest_evidence.id,
                        ),
                    ));
                }
            }
        }
        if r3_coverage.is_some() {
            coverage = deduplicate_coverage(coverage)?;
        }

        relationship_drafts.sort_by(|left, right| {
            left.relationship
                .id
                .as_bytes()
                .cmp(right.relationship.id.as_bytes())
        });
        if relationship_drafts
            .windows(2)
            .any(|pair| pair[0].relationship.id == pair[1].relationship.id)
        {
            return Err(WorkspaceError::ContractInvalid);
        }
        let mut claims = BTreeMap::<String, WorkspaceClaim>::new();
        for entity in entities.values() {
            let evidence_ids = entity_evidence
                .get(&entity.id)
                .cloned()
                .ok_or(WorkspaceError::ContractInvalid)?;
            let claim = WorkspaceClaim::new(
                ClaimSubjectKind::Entity,
                entity.id.clone(),
                ClaimState::DeterministicFact,
                evidence_ids,
            );
            claims.insert(claim.id.clone(), claim);
        }
        for draft in &relationship_drafts {
            let claim = WorkspaceClaim::new(
                ClaimSubjectKind::Relationship,
                draft.relationship.id.clone(),
                draft.state,
                draft.relationship.evidence_ids.clone(),
            );
            claims.insert(claim.id.clone(), claim);
        }

        let all_evidence = self.collect_evidence(crates, &sources);
        let graph = WorkspaceKnowledgeGraph {
            repository_identity: self.repository_identity.to_owned(),
            commit_oid: self.commit_oid.to_owned(),
            entities: entities.values().cloned().collect(),
            relationships: relationship_drafts
                .iter()
                .map(|draft| draft.relationship.clone())
                .collect(),
            claims: claims.values().cloned().collect(),
            evidence: all_evidence.values().cloned().collect(),
            diagnostics: sorted_diagnostics(&diagnostics),
            coverage: sorted_coverage(&coverage),
        };
        let extraction_chunks = sources
            .iter()
            .map(|source| {
                build_chunk(
                    self.repository_identity,
                    source,
                    &entities,
                    &entity_source_paths,
                    &relationship_drafts,
                    &claims,
                    &all_evidence,
                    &diagnostics,
                    &coverage,
                    &self.root_evidence.id,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let knowledge = RustWorkspaceKnowledge {
            extraction_chunks,
            graph,
        };
        knowledge.validate()?;
        let mut cache_entries = analyses
            .values()
            .map(|resolved| {
                AnalysisCacheEntry::new(resolved.key.clone(), resolved.analysis.clone())
            })
            .collect::<Vec<_>>();
        cache_entries.sort_by(|left, right| {
            left.analysis_cache_entry_id
                .as_bytes()
                .cmp(right.analysis_cache_entry_id.as_bytes())
        });
        let source_records = sources
            .iter()
            .map(|source| {
                let resolved = analyses
                    .get(&source.path)
                    .ok_or(WorkspaceError::ContractInvalid)?;
                Ok(SourceAnalysisRecord {
                    path: source.path.clone(),
                    source_file_id: workspace_source_file_id(
                        self.repository_identity,
                        &source.crate_id,
                        &source.path,
                    ),
                    analysis_cache_entry_id: resolved.key.entry_id(),
                    reused: resolved.reused,
                    root: source.root,
                })
            })
            .collect::<Result<Vec<_>, WorkspaceError>>()?;
        Ok(IncrementalWorkspaceExtraction {
            knowledge,
            cache_entries,
            source_records,
            parser_invocation_count,
        })
    }

    fn root_package_crates(
        &self,
        plan: &RootPackageWorkspacePlan,
    ) -> Result<Vec<CrateDraft>, WorkspaceError> {
        plan.targets
            .iter()
            .map(|target| {
                let manifest = self
                    .files
                    .get(target.manifest_path.as_str())
                    .copied()
                    .ok_or(WorkspaceError::ContractInvalid)?;
                let mut entity = WorkspaceEntity::rust_crate(
                    self.repository_identity,
                    &target.manifest_path,
                    &target.package_name,
                    target.target_kind.as_str(),
                    &target.target_name,
                );
                entity.properties.insert(
                    "workspace_member_source".to_owned(),
                    target.member_source.as_str().to_owned(),
                );
                entity.properties.insert(
                    "workspace_root_shape".to_owned(),
                    plan.root_shape.as_str().to_owned(),
                );
                Ok(CrateDraft {
                    entity,
                    manifest_evidence: complete_evidence(self.inventory, manifest)?,
                    root_source_path: target.source_path.clone(),
                    root_module_name: normalize_identifier(&target.target_name.replace('-', "_")),
                    build_script: false,
                })
            })
            .collect()
    }

    fn parse_crates(&self) -> Result<Vec<CrateDraft>, WorkspaceError> {
        let root = self
            .files
            .get("Cargo.toml")
            .copied()
            .ok_or(WorkspaceError::UnsupportedWorkspace)?;
        let root_value = parse_manifest(root)?;
        let workspace = root_value
            .get("workspace")
            .and_then(toml::Value::as_table)
            .ok_or(WorkspaceError::UnsupportedWorkspace)?;
        if !only_keys(
            root_value
                .as_table()
                .ok_or(WorkspaceError::UnsupportedWorkspace)?,
            &["workspace"],
        ) || !only_keys(workspace, &["members", "resolver"])
            || workspace
                .get("resolver")
                .and_then(toml::Value::as_str)
                .is_none_or(str::is_empty)
        {
            return Err(WorkspaceError::UnsupportedWorkspace);
        }
        let members = workspace
            .get("members")
            .and_then(toml::Value::as_array)
            .ok_or(WorkspaceError::UnsupportedWorkspace)?;
        if members.is_empty() {
            return Err(WorkspaceError::UnsupportedWorkspace);
        }
        if members.len() > MAX_S4_WORKSPACE_CRATES {
            return Err(WorkspaceError::LimitExceeded {
                limit: "workspace_crates",
                maximum: MAX_S4_WORKSPACE_CRATES as u64,
                observed: u64::try_from(members.len()).unwrap_or(u64::MAX),
            });
        }
        let mut member_paths = members
            .iter()
            .map(|member| {
                member
                    .as_str()
                    .filter(|path| {
                        valid_relative_path(path) && !path.contains(['*', '?', '[', ']'])
                    })
                    .map(str::to_owned)
                    .ok_or(WorkspaceError::UnsupportedWorkspace)
            })
            .collect::<Result<Vec<_>, _>>()?;
        member_paths.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
        if member_paths.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(WorkspaceError::UnsupportedWorkspace);
        }
        let mut crates = Vec::new();
        for member_path in &member_paths {
            crates.extend(self.parse_member(member_path)?);
        }
        if crates.len() > MAX_S4_WORKSPACE_CRATES {
            return Err(WorkspaceError::LimitExceeded {
                limit: "workspace_crates",
                maximum: MAX_S4_WORKSPACE_CRATES as u64,
                observed: u64::try_from(crates.len()).unwrap_or(u64::MAX),
            });
        }
        let mut crate_ids = BTreeSet::new();
        let mut source_paths = BTreeSet::new();
        let mut package_manifests = BTreeMap::new();
        let mut target_names = BTreeSet::new();
        if crates.iter().any(|crate_draft| {
            let package_name = crate_draft
                .entity
                .properties
                .get("package_name")
                .cloned()
                .unwrap_or_default();
            let manifest_path = crate_draft
                .entity
                .properties
                .get("manifest_path")
                .cloned()
                .unwrap_or_default();
            let target_name = crate_draft
                .entity
                .properties
                .get("target_name")
                .cloned()
                .unwrap_or_default();
            let package_collision = package_manifests
                .insert(package_name, manifest_path.clone())
                .is_some_and(|existing| existing != manifest_path);
            !crate_ids.insert(crate_draft.entity.id.clone())
                || !source_paths.insert(crate_draft.root_source_path.clone())
                || !target_names.insert(target_name)
                || package_collision
        }) {
            return Err(WorkspaceError::ContractInvalid);
        }
        Ok(crates)
    }

    fn parse_member(&self, member_path: &str) -> Result<Vec<CrateDraft>, WorkspaceError> {
        let manifest_path = format!("{member_path}/Cargo.toml");
        let manifest_file = self
            .files
            .get(manifest_path.as_str())
            .copied()
            .ok_or(WorkspaceError::UnsupportedWorkspace)?;
        let manifest = parse_manifest(manifest_file)?;
        let manifest_table = manifest
            .as_table()
            .ok_or(WorkspaceError::UnsupportedWorkspace)?;
        if !only_keys(
            manifest_table,
            &[
                "package",
                "lib",
                "bin",
                "dependencies",
                "dev-dependencies",
                "build-dependencies",
            ],
        ) {
            return Err(WorkspaceError::UnsupportedWorkspace);
        }
        validate_dependencies(&manifest)?;
        let package = manifest
            .get("package")
            .and_then(toml::Value::as_table)
            .ok_or(WorkspaceError::UnsupportedWorkspace)?;
        if !only_keys(package, &["name", "version", "edition", "build"])
            || package
                .get("version")
                .and_then(toml::Value::as_str)
                .is_none_or(str::is_empty)
            || package
                .get("edition")
                .and_then(toml::Value::as_str)
                .is_none_or(str::is_empty)
        {
            return Err(WorkspaceError::UnsupportedWorkspace);
        }
        let package_name = package
            .get("name")
            .and_then(toml::Value::as_str)
            .filter(|name| !name.is_empty())
            .ok_or(WorkspaceError::UnsupportedWorkspace)?;
        let declared_build_script = match package.get("build") {
            None | Some(toml::Value::Boolean(false)) => false,
            Some(toml::Value::String(path)) if valid_relative_path(path) => true,
            _ => return Err(WorkspaceError::UnsupportedWorkspace),
        };
        let targets = self.targets(&manifest, member_path, package_name)?;
        let manifest_evidence = complete_evidence(self.inventory, manifest_file)?;
        let conventional_build_script = self
            .files
            .contains_key(format!("{member_path}/build.rs").as_str());
        Ok(targets
            .into_iter()
            .map(|target| CrateDraft {
                entity: WorkspaceEntity::rust_crate(
                    self.repository_identity,
                    &manifest_path,
                    package_name,
                    target.kind,
                    &target.name,
                ),
                manifest_evidence: manifest_evidence.clone(),
                root_source_path: target.path,
                root_module_name: normalize_identifier(&target.name.replace('-', "_")),
                build_script: declared_build_script || conventional_build_script,
            })
            .collect())
    }

    fn targets(
        &self,
        manifest: &toml::Value,
        member_path: &str,
        package_name: &str,
    ) -> Result<Vec<TargetDraft>, WorkspaceError> {
        let mut targets = Vec::new();
        if let Some(library) = manifest.get("lib") {
            let library = library
                .as_table()
                .ok_or(WorkspaceError::UnsupportedWorkspace)?;
            if !only_keys(library, &["name", "path"]) {
                return Err(WorkspaceError::UnsupportedWorkspace);
            }
            targets.push(explicit_target("lib", library, member_path)?);
        } else {
            let path = format!("{member_path}/src/lib.rs");
            if self.files.contains_key(path.as_str()) {
                targets.push(TargetDraft {
                    kind: "lib",
                    name: package_name.replace('-', "_"),
                    path,
                });
            }
        }
        if let Some(binaries) = manifest.get("bin") {
            let binaries = binaries
                .as_array()
                .ok_or(WorkspaceError::UnsupportedWorkspace)?;
            for binary in binaries {
                let binary = binary
                    .as_table()
                    .ok_or(WorkspaceError::UnsupportedWorkspace)?;
                if !only_keys(binary, &["name", "path"]) {
                    return Err(WorkspaceError::UnsupportedWorkspace);
                }
                targets.push(explicit_target("bin", binary, member_path)?);
            }
        } else {
            let path = format!("{member_path}/src/main.rs");
            if self.files.contains_key(path.as_str()) {
                targets.push(TargetDraft {
                    kind: "bin",
                    name: package_name.to_owned(),
                    path,
                });
            }
        }
        targets.sort_by(|left, right| {
            (left.kind, left.name.as_bytes(), left.path.as_bytes()).cmp(&(
                right.kind,
                right.name.as_bytes(),
                right.path.as_bytes(),
            ))
        });
        let mut identities = BTreeSet::new();
        let mut paths = BTreeSet::new();
        if targets.is_empty()
            || targets.iter().any(|target| {
                !identities.insert((target.kind, target.name.clone()))
                    || !paths.insert(target.path.clone())
            })
        {
            return Err(WorkspaceError::UnsupportedWorkspace);
        }
        Ok(targets)
    }

    fn collect_crate_sources(
        &self,
        crate_draft: &CrateDraft,
        cache_entries: &BTreeMap<&str, &AnalysisCacheEntry>,
        analyses: &mut BTreeMap<String, ResolvedAnalysis>,
        parser_invocation_count: &mut u64,
        sources: &mut Vec<SourceDraft>,
        tolerate_unsupported_rust: bool,
    ) -> Result<(), WorkspaceError> {
        let root = self.source_draft(
            crate_draft,
            &crate_draft.root_source_path,
            "crate",
            &crate_draft.root_module_name,
            WorkspaceVisibility::Public,
            None,
            true,
        )?;
        let mut pending = vec![root];
        let mut seen = BTreeSet::new();
        while let Some(source) = pending.pop() {
            if !seen.insert(source.path.clone()) {
                return Err(WorkspaceError::AmbiguousModule);
            }
            let resolved = self.resolve_analysis(
                &source,
                cache_entries,
                parser_invocation_count,
                tolerate_unsupported_rust,
            )?;
            let parsed = parsed_from_analysis(&resolved.analysis);
            self.collect_out_of_line_sources(
                crate_draft,
                &source,
                &parsed,
                &source.module_path,
                &source.module_name,
                &module_children_directory(&source.path)?,
                &mut pending,
            )?;
            if analyses.insert(source.path.clone(), resolved).is_some() {
                return Err(WorkspaceError::ContractInvalid);
            }
            sources.push(source);
        }
        Ok(())
    }

    fn resolve_analysis(
        &self,
        source: &SourceDraft,
        cache_entries: &BTreeMap<&str, &AnalysisCacheEntry>,
        parser_invocation_count: &mut u64,
        tolerate_unsupported_rust: bool,
    ) -> Result<ResolvedAnalysis, WorkspaceError> {
        let key = AnalysisCacheKey {
            repository_identity: self.repository_identity.to_owned(),
            source_file_id: workspace_source_file_id(
                self.repository_identity,
                &source.crate_id,
                &source.path,
            ),
            canonical_source_path: source.path.clone(),
            source_blob_oid: source.evidence.blob_oid.clone(),
            crate_id: source.crate_id.clone(),
            canonical_module_path: source.module_path.clone(),
            language_extractor: S4_TREE_SITTER_EXTRACTOR_VERSION.to_owned(),
            workspace_mapper: S4_WORKSPACE_EXTRACTOR_VERSION.to_owned(),
            ontology: S4_ONTOLOGY_VERSION.to_owned(),
        };
        let entry_id = key.entry_id();
        if let Some(entry) = cache_entries.get(entry_id.as_str()) {
            if entry.key != key || !entry.is_self_consistent() {
                return Err(WorkspaceError::ContractInvalid);
            }
            if !tolerate_unsupported_rust || !entry.analysis.unsupported_construct {
                let mut parsed = parsed_from_analysis(&entry.analysis);
                if tolerate_unsupported_rust {
                    self.defer_missing_modules(
                        &mut parsed,
                        &module_children_directory(&source.path)?,
                    )?;
                }
                return Ok(ResolvedAnalysis {
                    key,
                    analysis: analysis_from_parsed(&parsed),
                    reused: true,
                });
            }
        }
        let mut parsed =
            parse_rust_source(&source.path, &source.source, tolerate_unsupported_rust)?;
        if tolerate_unsupported_rust {
            self.defer_missing_modules(&mut parsed, &module_children_directory(&source.path)?)?;
        }
        *parser_invocation_count = parser_invocation_count.saturating_add(1);
        Ok(ResolvedAnalysis {
            key,
            analysis: analysis_from_parsed(&parsed),
            reused: false,
        })
    }

    fn defer_missing_modules(
        &self,
        parsed: &mut ParsedSource,
        children_directory: &str,
    ) -> Result<(), WorkspaceError> {
        let mut retained = Vec::with_capacity(parsed.modules.len());
        for mut declaration in std::mem::take(&mut parsed.modules) {
            if let Some(body) = declaration.body.as_mut() {
                self.defer_missing_modules(
                    body,
                    &join_directory(children_directory, &declaration.name),
                )?;
                retained.push(declaration);
                continue;
            }
            match self.resolve_module_source(children_directory, &declaration.name) {
                Ok(_) => retained.push(declaration),
                Err(WorkspaceError::UnsupportedWorkspace) => {
                    parsed.unsupported_construct = true;
                }
                Err(error) => return Err(error),
            }
        }
        parsed.modules = retained;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn collect_out_of_line_sources(
        &self,
        crate_draft: &CrateDraft,
        declaring_source: &SourceDraft,
        parsed: &ParsedSource,
        module_path: &str,
        module_name: &str,
        children_directory: &str,
        pending: &mut Vec<SourceDraft>,
    ) -> Result<(), WorkspaceError> {
        for declaration in parsed.modules.iter().rev() {
            let child_path = child_module_path(module_path, &declaration.name);
            let child_name = format!("{module_name}::{}", declaration.name);
            if let Some(body) = &declaration.body {
                let nested_directory = join_directory(children_directory, &declaration.name);
                self.collect_out_of_line_sources(
                    crate_draft,
                    declaring_source,
                    body,
                    &child_path,
                    &child_name,
                    &nested_directory,
                    pending,
                )?;
            } else {
                let child_source_path =
                    self.resolve_module_source(children_directory, &declaration.name)?;
                pending.push(self.source_draft(
                    crate_draft,
                    &child_source_path,
                    &child_path,
                    &child_name,
                    declaration.visibility,
                    Some(declaring_source.evidence.id.clone()),
                    false,
                )?);
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn source_draft(
        &self,
        crate_draft: &CrateDraft,
        path: &str,
        module_path: &str,
        module_name: &str,
        module_visibility: WorkspaceVisibility,
        parent_evidence_id: Option<String>,
        root: bool,
    ) -> Result<SourceDraft, WorkspaceError> {
        let file = self
            .files
            .get(path)
            .copied()
            .ok_or(WorkspaceError::UnsupportedWorkspace)?;
        if file.content_kind() != ContentKind::TextUtf8 {
            return Err(WorkspaceError::InvalidUtf8 {
                path: path.to_owned(),
            });
        }
        let source =
            std::str::from_utf8(file.bytes()).map_err(|_| WorkspaceError::InvalidUtf8 {
                path: path.to_owned(),
            })?;
        Ok(SourceDraft {
            crate_id: crate_draft.entity.id.clone(),
            crate_manifest_evidence_id: crate_draft.manifest_evidence.id.clone(),
            path: path.to_owned(),
            source: source.to_owned(),
            evidence: complete_evidence(self.inventory, file)?,
            parent_evidence_id,
            module_path: module_path.to_owned(),
            module_name: module_name.to_owned(),
            module_visibility,
            root,
        })
    }

    fn resolve_module_source(
        &self,
        module_directory: &str,
        module_name: &str,
    ) -> Result<String, WorkspaceError> {
        let module_base = join_directory(module_directory, module_name);
        let flat = format!("{module_base}.rs");
        let nested = format!("{module_base}/mod.rs");
        match (
            self.files.contains_key(flat.as_str()),
            self.files.contains_key(nested.as_str()),
        ) {
            (true, false) => Ok(flat),
            (false, true) => Ok(nested),
            (true, true) => Err(WorkspaceError::AmbiguousModule),
            (false, false) => Err(WorkspaceError::UnsupportedWorkspace),
        }
    }

    fn collect_evidence(
        &self,
        crates: &[CrateDraft],
        sources: &[SourceDraft],
    ) -> BTreeMap<String, WorkspaceEvidence> {
        std::iter::once(self.root_evidence.clone())
            .chain(
                crates
                    .iter()
                    .map(|crate_draft| crate_draft.manifest_evidence.clone()),
            )
            .chain(sources.iter().map(|source| source.evidence.clone()))
            .map(|evidence| (evidence.id.clone(), evidence))
            .collect()
    }
}

fn collect_module_views<'a>(
    source: &'a SourceDraft,
    parsed: &'a ParsedSource,
    modules: &mut Vec<ModuleView<'a>>,
) {
    modules.push(ModuleView {
        source,
        module_path: source.module_path.clone(),
        module_name: source.module_name.clone(),
        visibility: source.module_visibility,
        parsed,
        inline: false,
    });
    collect_inline_module_views(
        source,
        parsed,
        &source.module_path,
        &source.module_name,
        modules,
    );
}

fn collect_inline_module_views<'a>(
    source: &'a SourceDraft,
    parsed: &'a ParsedSource,
    parent_path: &str,
    parent_name: &str,
    modules: &mut Vec<ModuleView<'a>>,
) {
    for declaration in &parsed.modules {
        let Some(body) = &declaration.body else {
            continue;
        };
        let module_path = child_module_path(parent_path, &declaration.name);
        let module_name = format!("{parent_name}::{}", declaration.name);
        modules.push(ModuleView {
            source,
            module_path: module_path.clone(),
            module_name: module_name.clone(),
            visibility: declaration.visibility,
            parsed: body,
            inline: true,
        });
        collect_inline_module_views(source, body, &module_path, &module_name, modules);
    }
}

fn source_entity_evidence(source: &SourceDraft) -> Result<Vec<String>, WorkspaceError> {
    if source.root {
        Ok(vec![
            source.crate_manifest_evidence_id.clone(),
            source.evidence.id.clone(),
        ])
    } else {
        Ok(vec![
            source
                .parent_evidence_id
                .clone()
                .ok_or(WorkspaceError::ContractInvalid)?,
            source.evidence.id.clone(),
        ])
    }
}

fn module_children_directory(path: &str) -> Result<String, WorkspaceError> {
    let directory = path.rsplit_once('/').map_or("", |(directory, _)| directory);
    let stem = path
        .rsplit('/')
        .next()
        .and_then(|name| name.strip_suffix(".rs"))
        .ok_or(WorkspaceError::UnsupportedWorkspace)?;
    Ok(if matches!(stem, "lib" | "main" | "mod") {
        directory.to_owned()
    } else {
        join_directory(directory, stem)
    })
}

fn join_directory(directory: &str, name: &str) -> String {
    if directory.is_empty() {
        name.to_owned()
    } else {
        format!("{directory}/{name}")
    }
}

fn normalize_identifier(value: &str) -> String {
    value.nfc().collect()
}

fn insert_unique<K: Ord, V>(
    values: &mut BTreeMap<K, V>,
    key: K,
    value: V,
) -> Result<(), WorkspaceError> {
    if values.insert(key, value).is_none() {
        Ok(())
    } else {
        Err(WorkspaceError::ContractInvalid)
    }
}

fn deduplicate_coverage(
    values: Vec<(String, WorkspaceCoverageGap)>,
) -> Result<Vec<(String, WorkspaceCoverageGap)>, WorkspaceError> {
    let mut by_id = BTreeMap::<String, (String, WorkspaceCoverageGap)>::new();
    for (path, gap) in values {
        match by_id.entry(gap.id.clone()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert((path, gap));
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                if entry.get().1 != gap {
                    return Err(WorkspaceError::ContractInvalid);
                }
                if path.as_bytes() < entry.get().0.as_bytes() {
                    entry.get_mut().0 = path;
                }
            }
        }
    }
    Ok(by_id.into_values().collect())
}

#[allow(clippy::too_many_arguments)]
fn build_chunk(
    repository_identity: &str,
    source: &SourceDraft,
    entities: &BTreeMap<String, WorkspaceEntity>,
    entity_source_paths: &BTreeMap<String, String>,
    relationship_drafts: &[RelationshipDraft],
    claims: &BTreeMap<String, WorkspaceClaim>,
    evidence: &BTreeMap<String, WorkspaceEvidence>,
    diagnostics: &[(String, WorkspaceDiagnostic)],
    coverage: &[(String, WorkspaceCoverageGap)],
    root_evidence_id: &str,
) -> Result<WorkspaceExtractionChunk, WorkspaceError> {
    let mut local_entities = entities
        .values()
        .filter(|entity| {
            entity_source_paths.get(&entity.id) == Some(&source.path)
                || source.root && entity.id == source.crate_id
        })
        .cloned()
        .collect::<Vec<_>>();
    local_entities.sort_by(|left, right| left.id.as_bytes().cmp(right.id.as_bytes()));
    let mut local_relationships = relationship_drafts
        .iter()
        .filter(|draft| draft.source_path == source.path)
        .map(|draft| draft.relationship.clone())
        .collect::<Vec<_>>();
    local_relationships.sort_by(|left, right| left.id.as_bytes().cmp(right.id.as_bytes()));
    let subject_ids = local_entities
        .iter()
        .map(|entity| entity.id.as_str())
        .chain(
            local_relationships
                .iter()
                .map(|relationship| relationship.id.as_str()),
        )
        .collect::<BTreeSet<_>>();
    let local_claims = claims
        .values()
        .filter(|claim| subject_ids.contains(claim.subject_id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let referenced_evidence = local_claims
        .iter()
        .flat_map(|claim| claim.evidence_ids.iter().map(String::as_str))
        .filter(|id| *id != root_evidence_id)
        .collect::<BTreeSet<_>>();
    let local_evidence = referenced_evidence
        .iter()
        .map(|id| {
            evidence
                .get(*id)
                .cloned()
                .ok_or(WorkspaceError::ContractInvalid)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let source_entity_id = entities
        .values()
        .find(|entity| {
            entity.kind == EntityKind::SourceFile
                && entity_source_paths.get(&entity.id) == Some(&source.path)
        })
        .map(|entity| entity.id.clone())
        .ok_or(WorkspaceError::ContractInvalid)?;
    let mut local_coverage = coverage
        .iter()
        .filter(|(path, _)| path == &source.path)
        .map(|(_, gap)| gap.clone())
        .collect::<Vec<_>>();
    local_coverage.sort_by(|left, right| left.id.as_bytes().cmp(right.id.as_bytes()));
    Ok(WorkspaceExtractionChunk {
        repository_identity: repository_identity.to_owned(),
        crate_id: source.crate_id.clone(),
        source_file_id: source_entity_id,
        entities: local_entities,
        relationships: local_relationships,
        claims: local_claims,
        evidence: local_evidence,
        diagnostics: diagnostics
            .iter()
            .filter(|(path, _)| path == &source.path)
            .map(|(_, diagnostic)| diagnostic.clone())
            .collect(),
        coverage: local_coverage,
    })
}

fn parse_manifest(file: &InventoryFile) -> Result<toml::Value, WorkspaceError> {
    if file.content_kind() != ContentKind::TextUtf8 {
        return Err(WorkspaceError::InvalidUtf8 {
            path: file.path().to_owned(),
        });
    }
    let source = std::str::from_utf8(file.bytes()).map_err(|_| WorkspaceError::InvalidUtf8 {
        path: file.path().to_owned(),
    })?;
    toml::from_str(source).map_err(|_| WorkspaceError::UnsupportedWorkspace)
}

fn validate_dependencies(manifest: &toml::Value) -> Result<(), WorkspaceError> {
    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        let Some(dependencies) = manifest.get(section) else {
            continue;
        };
        let dependencies = dependencies
            .as_table()
            .ok_or(WorkspaceError::UnsupportedWorkspace)?;
        for dependency in dependencies.values() {
            let table = dependency
                .as_table()
                .ok_or(WorkspaceError::UnsupportedWorkspace)?;
            if table.len() != 1
                || table
                    .get("path")
                    .and_then(toml::Value::as_str)
                    .is_none_or(|path| !valid_relative_dependency_path(path))
            {
                return Err(WorkspaceError::UnsupportedWorkspace);
            }
        }
    }
    Ok(())
}

fn explicit_target(
    kind: &'static str,
    target: &toml::Table,
    member_path: &str,
) -> Result<TargetDraft, WorkspaceError> {
    let name = target
        .get("name")
        .and_then(toml::Value::as_str)
        .filter(|name| !name.is_empty())
        .ok_or(WorkspaceError::UnsupportedWorkspace)?;
    let relative = target
        .get("path")
        .and_then(toml::Value::as_str)
        .ok_or(WorkspaceError::UnsupportedWorkspace)?;
    Ok(TargetDraft {
        kind,
        name: name.to_owned(),
        path: join_member_path(member_path, relative)?,
    })
}

fn only_keys(table: &toml::Table, allowed: &[&str]) -> bool {
    table.keys().all(|key| allowed.contains(&key.as_str()))
}

fn join_member_path(member_path: &str, relative: &str) -> Result<String, WorkspaceError> {
    if !valid_relative_path(relative) {
        return Err(WorkspaceError::UnsupportedWorkspace);
    }
    Ok(format!("{member_path}/{relative}"))
}

fn valid_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.ends_with('/')
        && !path.contains('\\')
        && path
            .split('/')
            .all(|component| !component.is_empty() && !matches!(component, "." | ".."))
}

fn valid_relative_dependency_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && path
            .split('/')
            .all(|component| !component.is_empty() && component != ".")
}

fn complete_evidence(
    inventory: &RepositoryInventory,
    file: &InventoryFile,
) -> Result<WorkspaceEvidence, WorkspaceError> {
    let byte_length =
        u64::try_from(file.bytes().len()).map_err(|_| WorkspaceError::LimitExceeded {
            limit: "single_file_bytes",
            maximum: u64::MAX - 1,
            observed: u64::MAX,
        })?;
    if byte_length == 0 {
        return Err(WorkspaceError::UnsupportedWorkspace);
    }
    Ok(WorkspaceEvidence::complete_file(
        inventory.bound_revision().repository_identity().as_str(),
        inventory.bound_revision().commit_oid().as_str(),
        file.path(),
        file.blob_oid().as_str(),
        byte_length,
    ))
}

fn analysis_from_parsed(parsed: &ParsedSource) -> RustSourceAnalysis {
    RustSourceAnalysis {
        declarations: parsed
            .declarations
            .iter()
            .map(|declaration| RustDeclarationObservation {
                kind: declaration.kind,
                name: declaration.name.clone(),
                visibility: declaration.visibility,
            })
            .collect(),
        modules: parsed
            .modules
            .iter()
            .map(|module| RustModuleObservation {
                name: module.name.clone(),
                visibility: module.visibility,
                body: module
                    .body
                    .as_deref()
                    .map(analysis_from_parsed)
                    .map(Box::new),
            })
            .collect(),
        imports: parsed
            .imports
            .iter()
            .map(|import| import.spelling.clone())
            .collect(),
        unsupported_construct: parsed.unsupported_construct,
    }
}

fn parsed_from_analysis(analysis: &RustSourceAnalysis) -> ParsedSource {
    ParsedSource {
        declarations: analysis
            .declarations
            .iter()
            .map(|declaration| DeclarationDraft {
                kind: declaration.kind,
                name: declaration.name.clone(),
                visibility: declaration.visibility,
            })
            .collect(),
        modules: analysis
            .modules
            .iter()
            .map(|module| ModuleDeclaration {
                name: module.name.clone(),
                visibility: module.visibility,
                body: module
                    .body
                    .as_deref()
                    .map(parsed_from_analysis)
                    .map(Box::new),
            })
            .collect(),
        imports: analysis
            .imports
            .iter()
            .map(|spelling| ImportDraft {
                spelling: spelling.clone(),
            })
            .collect(),
        unsupported_construct: analysis.unsupported_construct,
    }
}

fn parse_rust_source(
    path: &str,
    source: &str,
    tolerate_unsupported_rust: bool,
) -> Result<ParsedSource, WorkspaceError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .map_err(|_| WorkspaceError::ParserCancelled {
            path: path.to_owned(),
        })?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| WorkspaceError::ParserCancelled {
            path: path.to_owned(),
        })?;
    if tree.root_node().has_error() {
        return Err(WorkspaceError::MalformedSyntax {
            path: path.to_owned(),
        });
    }
    parse_rust_scope(tree.root_node(), source, path, 0, tolerate_unsupported_rust)
}

#[allow(clippy::too_many_lines)]
fn parse_rust_scope(
    scope: Node<'_>,
    source: &str,
    path: &str,
    depth: u64,
    tolerate_unsupported_rust: bool,
) -> Result<ParsedSource, WorkspaceError> {
    if depth > STANDARD_LOCAL_S1_LIMITS.recursion_depth {
        return Err(WorkspaceError::LimitExceeded {
            limit: "syntax_recursion_depth",
            maximum: STANDARD_LOCAL_S1_LIMITS.recursion_depth,
            observed: depth,
        });
    }
    let mut parsed = ParsedSource {
        declarations: Vec::new(),
        modules: Vec::new(),
        imports: Vec::new(),
        unsupported_construct: false,
    };
    let mut cursor = scope.walk();
    let mut pending_outer_attribute = false;
    for node in scope.named_children(&mut cursor) {
        if tolerate_unsupported_rust && node.kind() == "attribute_item" {
            parsed.unsupported_construct = true;
            pending_outer_attribute |= node
                .utf8_text(source.as_bytes())
                .is_ok_and(|text| text.trim_start().starts_with("#["));
            continue;
        }
        if matches!(node.kind(), "line_comment" | "block_comment") {
            continue;
        }
        let attached_outer_attribute = tolerate_unsupported_rust
            && node
                .utf8_text(source.as_bytes())
                .is_ok_and(|text| text.trim_start().starts_with("#["));
        if tolerate_unsupported_rust && (pending_outer_attribute || attached_outer_attribute) {
            parsed.unsupported_construct = true;
            pending_outer_attribute = false;
            continue;
        }
        pending_outer_attribute = false;
        match node.kind() {
            "struct_item" => {
                parsed
                    .declarations
                    .push(declaration(node, source, EntityKind::RustStruct, path)?);
            }
            "enum_item" => {
                parsed
                    .declarations
                    .push(declaration(node, source, EntityKind::RustEnum, path)?);
            }
            "trait_item" => {
                parsed
                    .declarations
                    .push(declaration(node, source, EntityKind::RustTrait, path)?);
                parsed.unsupported_construct |= node
                    .child_by_field_name("body")
                    .is_some_and(|body| body.named_child_count() > 0);
            }
            "type_item" => parsed.declarations.push(declaration(
                node,
                source,
                EntityKind::RustTypeAlias,
                path,
            )?),
            "function_item" => {
                parsed.declarations.push(declaration(
                    node,
                    source,
                    EntityKind::RustFunction,
                    path,
                )?);
            }
            "mod_item" => {
                parsed.modules.push(ModuleDeclaration {
                    name: node_name(node, source, path)?,
                    visibility: visibility(node, source),
                    body: node
                        .child_by_field_name("body")
                        .map(|body| {
                            parse_rust_scope(
                                body,
                                source,
                                path,
                                depth + 1,
                                tolerate_unsupported_rust,
                            )
                            .map(Box::new)
                        })
                        .transpose()?,
                });
            }
            "use_declaration" => match parse_import(node, source, path) {
                Ok(spelling) => parsed.imports.push(ImportDraft { spelling }),
                Err(WorkspaceError::UnsupportedWorkspace) if tolerate_unsupported_rust => {
                    parsed.unsupported_construct = true;
                }
                Err(error) => return Err(error),
            },
            _ => parsed.unsupported_construct = true,
        }
    }
    parsed.declarations.sort_by(|left, right| {
        (left.kind, left.name.as_bytes()).cmp(&(right.kind, right.name.as_bytes()))
    });
    parsed
        .modules
        .sort_by(|left, right| left.name.as_bytes().cmp(right.name.as_bytes()));
    parsed
        .imports
        .sort_by(|left, right| left.spelling.as_bytes().cmp(right.spelling.as_bytes()));
    if tolerate_unsupported_rust {
        remove_ambiguous_observations(&mut parsed);
    }
    Ok(parsed)
}

fn remove_ambiguous_observations(parsed: &mut ParsedSource) {
    let mut declaration_counts = BTreeMap::new();
    for declaration in &parsed.declarations {
        *declaration_counts
            .entry((declaration.kind, declaration.name.clone()))
            .or_insert(0_usize) += 1;
    }
    let declaration_count = parsed.declarations.len();
    parsed.declarations.retain(|declaration| {
        declaration_counts.get(&(declaration.kind, declaration.name.clone())) == Some(&1)
    });

    let mut module_counts = BTreeMap::new();
    for module in &parsed.modules {
        *module_counts.entry(module.name.clone()).or_insert(0_usize) += 1;
    }
    let module_count = parsed.modules.len();
    parsed
        .modules
        .retain(|module| module_counts.get(&module.name) == Some(&1));

    let mut import_counts = BTreeMap::new();
    for import in &parsed.imports {
        *import_counts
            .entry(import.spelling.clone())
            .or_insert(0_usize) += 1;
    }
    let import_count = parsed.imports.len();
    parsed
        .imports
        .retain(|import| import_counts.get(&import.spelling) == Some(&1));

    parsed.unsupported_construct |= declaration_count != parsed.declarations.len()
        || module_count != parsed.modules.len()
        || import_count != parsed.imports.len();
}

fn declaration(
    node: Node<'_>,
    source: &str,
    kind: EntityKind,
    path: &str,
) -> Result<DeclarationDraft, WorkspaceError> {
    Ok(DeclarationDraft {
        kind,
        name: node_name(node, source, path)?,
        visibility: visibility(node, source),
    })
}

fn node_name(node: Node<'_>, source: &str, path: &str) -> Result<String, WorkspaceError> {
    node.child_by_field_name("name")
        .and_then(|name| name.utf8_text(source.as_bytes()).ok())
        .filter(|name| !name.is_empty())
        .map(normalize_identifier)
        .ok_or_else(|| WorkspaceError::MalformedSyntax {
            path: path.to_owned(),
        })
}

fn visibility(node: Node<'_>, source: &str) -> WorkspaceVisibility {
    node.utf8_text(source.as_bytes())
        .map_or(WorkspaceVisibility::Private, |text| {
            if text.trim_start().starts_with("pub ") {
                WorkspaceVisibility::Public
            } else {
                WorkspaceVisibility::Private
            }
        })
}

fn parse_import(node: Node<'_>, source: &str, path: &str) -> Result<String, WorkspaceError> {
    let text = node
        .utf8_text(source.as_bytes())
        .map_err(|_| WorkspaceError::InvalidUtf8 {
            path: path.to_owned(),
        })?
        .trim();
    let text = text
        .strip_prefix("pub ")
        .unwrap_or(text)
        .strip_prefix("use ")
        .and_then(|text| text.strip_suffix(';'))
        .map(str::trim)
        .filter(|text| {
            !text.is_empty()
                && !text.contains(['{', '}', '*'])
                && text
                    .split("::")
                    .all(|segment| !segment.is_empty() && segment != "self")
        })
        .ok_or(WorkspaceError::UnsupportedWorkspace)?;
    Ok(normalize_identifier(text))
}

fn child_module_path(parent: &str, name: &str) -> String {
    if parent == "crate" {
        format!("crate::{name}")
    } else {
        format!("{parent}::{name}")
    }
}

fn sorted_diagnostics(diagnostics: &[(String, WorkspaceDiagnostic)]) -> Vec<WorkspaceDiagnostic> {
    let mut values = diagnostics
        .iter()
        .map(|(_, diagnostic)| diagnostic.clone())
        .collect::<Vec<_>>();
    values.sort_by(|left, right| {
        (&left.code, &left.message, &left.evidence_ids).cmp(&(
            &right.code,
            &right.message,
            &right.evidence_ids,
        ))
    });
    values
}

fn sorted_coverage(coverage: &[(String, WorkspaceCoverageGap)]) -> Vec<WorkspaceCoverageGap> {
    let mut values = coverage
        .iter()
        .map(|(_, gap)| gap.clone())
        .collect::<Vec<_>>();
    values.sort_by(|left, right| left.id.as_bytes().cmp(right.id.as_bytes()));
    values
}
