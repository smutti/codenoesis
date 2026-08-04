use std::collections::{BTreeMap, BTreeSet};

use codenoesis_domain::knowledge::{ClaimState, ClaimSubjectKind};
use codenoesis_domain::s4::{WorkspaceEvidence, workspace_evidence_id};
use codenoesis_domain::s4_r3::RootPackageWorkspaceExtraction;
use codenoesis_domain::s4_r4::{
    BuildScriptProperties, BuildScriptSelection, CargoCoverageGap, CargoCoverageState,
    CargoDiagnostic, CargoEntity, CargoEntityKind, CargoEntityProperties, CargoFactKind,
    CargoFactLimit, CargoFactReason, CargoManifestExtractionChunk, CargoManifestFactError,
    CargoManifestFactExtraction, CargoManifestGraph, CargoManifestKnowledge, CargoRelationship,
    CargoRelationshipKind, CargoTargetKind, DeclaredBoolean, DeclaredName, DeclaredPath,
    DeclaredString, DeclaredValue, DependencyKind, DependencyProperties, DependencyScope,
    DependencySource, DependencySourceKind, FeatureMember, FeatureMemberSyntax, FeatureProperties,
    LocatorDigest, LocatorReference, LocatorReferenceKind, ManifestIndexEntry, ManifestProperties,
    ManifestRole, MetadataFact, PackageProperties, PatchProperties, PatchSelectorKind,
    PatchSourceSelector, SourceAnalysisState, TargetNameSource, TargetOptions, TargetPathSource,
    TargetProperties, WorkspaceDefaultsProperties, cargo_claim, cargo_entity_id,
    cargo_fact_limit_exceeded,
};
use codenoesis_domain::{
    ContentKind, InventoryFile, RepositoryInventory, STANDARD_LOCAL_S1_LIMITS,
};
use unicode_normalization::UnicodeNormalization as _;

const METADATA_FIELDS: &[&str] = &[
    "version",
    "edition",
    "rust-version",
    "authors",
    "description",
    "documentation",
    "homepage",
    "repository",
    "license",
    "license-file",
    "readme",
    "keywords",
    "categories",
    "publish",
    "include",
    "exclude",
    "default-run",
    "links",
    "autolib",
    "autobins",
    "autoexamples",
    "autotests",
    "autobenches",
];

const WORKSPACE_INHERITABLE_FIELDS: &[&str] = &[
    "version",
    "edition",
    "rust-version",
    "authors",
    "description",
    "documentation",
    "homepage",
    "repository",
    "license",
    "license-file",
    "readme",
    "keywords",
    "categories",
    "publish",
    "include",
    "exclude",
];

pub(super) fn extract_manifest_facts(
    inventory: &RepositoryInventory,
    workspace_extraction: RootPackageWorkspaceExtraction,
) -> Result<CargoManifestFactExtraction, CargoManifestFactError> {
    let RootPackageWorkspaceExtraction {
        knowledge: workspace,
        cache_entries,
        source_records,
        parser_invocation_count,
    } = workspace_extraction;
    let projection = ManifestFactBuilder::new(inventory, &workspace).build()?;
    let knowledge = CargoManifestKnowledge {
        workspace,
        extraction_chunks: projection.extraction_chunks,
        graph: projection.graph,
    };
    knowledge.validate()?;
    Ok(CargoManifestFactExtraction {
        knowledge,
        cache_entries,
        source_records,
        parser_invocation_count,
    })
}

struct ManifestProjection {
    extraction_chunks: Vec<CargoManifestExtractionChunk>,
    graph: CargoManifestGraph,
}

struct ManifestFactBuilder<'a> {
    workspace: &'a codenoesis_domain::s4_r3::RootPackageWorkspaceKnowledge,
    files: BTreeMap<&'a str, &'a InventoryFile>,
    repository_identity: &'a str,
    commit_oid: &'a str,
    entities: BTreeMap<String, CargoEntity>,
    entity_states: BTreeMap<String, ClaimState>,
    relationships: BTreeMap<String, CargoRelationship>,
    relationship_states: BTreeMap<String, ClaimState>,
    evidence: BTreeMap<String, WorkspaceEvidence>,
    diagnostics: BTreeMap<String, CargoDiagnostic>,
    coverage: BTreeMap<String, CargoCoverageGap>,
    workspace_defaults_id: Option<String>,
    workspace_default_fields: BTreeSet<String>,
    workspace_dependencies: BTreeMap<String, String>,
}

impl<'a> ManifestFactBuilder<'a> {
    fn new(
        inventory: &'a RepositoryInventory,
        workspace: &'a codenoesis_domain::s4_r3::RootPackageWorkspaceKnowledge,
    ) -> Self {
        let files = inventory
            .files()
            .iter()
            .map(|file| (file.path(), file))
            .collect::<BTreeMap<_, _>>();
        Self {
            workspace,
            files,
            repository_identity: inventory.bound_revision().repository_identity().as_str(),
            commit_oid: inventory.bound_revision().commit_oid().as_str(),
            entities: BTreeMap::new(),
            entity_states: BTreeMap::new(),
            relationships: BTreeMap::new(),
            relationship_states: BTreeMap::new(),
            evidence: BTreeMap::new(),
            diagnostics: BTreeMap::new(),
            coverage: BTreeMap::new(),
            workspace_defaults_id: None,
            workspace_default_fields: BTreeSet::new(),
            workspace_dependencies: BTreeMap::new(),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn build(mut self) -> Result<ManifestProjection, CargoManifestFactError> {
        let mut manifest_paths = self
            .workspace
            .plan
            .members
            .iter()
            .filter_map(|member| member.manifest_path.clone())
            .collect::<Vec<_>>();
        if !manifest_paths.iter().any(|path| path == "Cargo.toml") {
            manifest_paths.push("Cargo.toml".to_owned());
        }
        manifest_paths.sort_by(|left, right| {
            let left_root = left != "Cargo.toml";
            let right_root = right != "Cargo.toml";
            (left_root, left.as_bytes()).cmp(&(right_root, right.as_bytes()))
        });
        manifest_paths.dedup();

        for manifest_path in &manifest_paths {
            self.process_manifest(manifest_path)?;
        }
        if u64::try_from(self.entities.len()).unwrap_or(u64::MAX)
            > codenoesis_domain::s4_r4::MAX_R4_MANIFEST_FACT_ENTITIES
        {
            return Err(cargo_fact_limit_exceeded(
                CargoFactLimit::ManifestFactEntities,
                u64::try_from(self.entities.len()).unwrap_or(u64::MAX),
            ));
        }

        let claims = self.build_claims()?;
        let manifest_index = self.build_manifest_index();
        let extraction_chunks = self.build_chunks(&claims, &manifest_index)?;
        Ok(ManifestProjection {
            extraction_chunks,
            graph: CargoManifestGraph {
                entities: self.entities.into_values().collect(),
                relationships: self.relationships.into_values().collect(),
                claims,
                evidence: self.evidence.into_values().collect(),
                diagnostics: self.diagnostics.into_values().collect(),
                coverage: self.coverage.into_values().collect(),
                manifest_index,
            },
        })
    }

    #[allow(clippy::too_many_lines)]
    fn process_manifest(&mut self, manifest_path: &str) -> Result<(), CargoManifestFactError> {
        let file = self.files.get(manifest_path).copied().ok_or_else(|| {
            invalid_fact(
                CargoFactReason::MalformedValue,
                manifest_path,
                CargoFactKind::Manifest,
                None,
            )
        })?;
        if file.content_kind() != ContentKind::TextUtf8 || file.bytes().is_empty() {
            return Err(invalid_fact(
                CargoFactReason::MalformedValue,
                manifest_path,
                CargoFactKind::Manifest,
                None,
            ));
        }
        let source = std::str::from_utf8(file.bytes()).map_err(|_| {
            invalid_fact(
                CargoFactReason::MalformedValue,
                manifest_path,
                CargoFactKind::Manifest,
                None,
            )
        })?;
        let map = ManifestMap::parse(source, manifest_path)?;
        let package_section = map.single_section(&["package"])?;
        let workspace_section = map.single_section(&["workspace"])?;
        let manifest_header = if manifest_path == "Cargo.toml" {
            workspace_section
                .or(package_section)
                .map(|section| section.header)
        } else {
            package_section.map(|section| section.header)
        }
        .ok_or_else(|| {
            invalid_fact(
                CargoFactReason::MalformedValue,
                manifest_path,
                CargoFactKind::Manifest,
                None,
            )
        })?;
        let manifest_evidence_id = self.add_evidence(file, manifest_header);
        let root_shape = self.workspace.plan.root_shape.as_str().to_owned();
        let manifest_id = cargo_entity_id(
            self.repository_identity,
            CargoEntityKind::Manifest,
            &[manifest_path],
        );
        self.add_entity(
            CargoEntity {
                id: manifest_id.clone(),
                name: manifest_path.to_owned(),
                properties: CargoEntityProperties::Manifest(ManifestProperties {
                    manifest_path: manifest_path.to_owned(),
                    manifest_role: if manifest_path == "Cargo.toml" {
                        ManifestRole::WorkspaceRoot
                    } else {
                        ManifestRole::WorkspaceMember
                    },
                    root_shape,
                    package_table_present: package_section.is_some(),
                    workspace_table_present: workspace_section.is_some(),
                    evidence_id: manifest_evidence_id,
                }),
            },
            ClaimState::DeterministicFact,
        )?;

        if manifest_path == "Cargo.toml" {
            self.process_workspace_defaults(file, &map, &manifest_id)?;
        }
        let package_id = if let Some(section) = package_section {
            Some(self.process_package(file, &map, section, &manifest_id, manifest_path)?)
        } else {
            None
        };
        self.process_targets(
            file,
            &map,
            &manifest_id,
            package_id.as_deref(),
            manifest_path,
        )?;
        self.process_dependencies(
            file,
            &map,
            &manifest_id,
            package_id.as_deref(),
            manifest_path,
        )?;
        self.process_features(
            file,
            &map,
            &manifest_id,
            package_id.as_deref(),
            manifest_path,
        )?;
        self.process_patches(file, &map, &manifest_id, manifest_path)?;
        if let Some(package_id) = package_id.as_deref() {
            self.process_build_script(file, &map, &manifest_id, package_id, manifest_path)?;
        }
        self.process_typed_unsupported(file, &map, manifest_path)?;
        Self::reject_unrecognized_sections(&map, manifest_path)?;
        Ok(())
    }

    fn process_workspace_defaults(
        &mut self,
        file: &InventoryFile,
        map: &ManifestMap,
        manifest_id: &str,
    ) -> Result<(), CargoManifestFactError> {
        let Some(section) = map.single_section(&["workspace", "package"])? else {
            return Ok(());
        };
        let evidence_id = self.add_evidence(file, section.header);
        let defaults_id = cargo_entity_id(
            self.repository_identity,
            CargoEntityKind::WorkspacePackageDefaults,
            &[file.path()],
        );
        let metadata = self.parse_direct_metadata(file, section, file.path(), "")?;
        self.workspace_default_fields = metadata.iter().map(|fact| fact.field.clone()).collect();
        self.workspace_defaults_id = Some(defaults_id.clone());
        self.add_entity(
            CargoEntity {
                id: defaults_id.clone(),
                name: "workspace.package".to_owned(),
                properties: CargoEntityProperties::WorkspacePackageDefaults(
                    WorkspaceDefaultsProperties {
                        manifest_id: manifest_id.to_owned(),
                        manifest_path: file.path().to_owned(),
                        metadata,
                        evidence_id: evidence_id.clone(),
                    },
                ),
            },
            ClaimState::DeterministicFact,
        )?;
        self.add_relationship(
            CargoRelationshipKind::Declares,
            manifest_id.to_owned(),
            defaults_id,
            vec![evidence_id],
            ClaimState::DeterministicFact,
        )
    }

    #[allow(clippy::too_many_lines)]
    fn process_package(
        &mut self,
        file: &InventoryFile,
        map: &ManifestMap,
        section: &Section,
        manifest_id: &str,
        manifest_path: &str,
    ) -> Result<String, CargoManifestFactError> {
        let name_entry = section.single_entry(&["name"], manifest_path, CargoFactKind::Package)?;
        let package_name =
            required_name(name_entry, manifest_path, CargoFactKind::Package, "name")?;
        let entity_span = ByteRange {
            start: section.header.start,
            end: name_entry.span.end,
        };
        let evidence_id = self.add_evidence(file, entity_span);
        let package_id = cargo_entity_id(
            self.repository_identity,
            CargoEntityKind::Package,
            &[manifest_path, &package_name],
        );
        let mut metadata = Vec::new();
        let package_base = parent_path(manifest_path);
        let defaults_id = self.workspace_defaults_id.clone();
        let mut inherited = false;
        for entry in &section.entries {
            let field = entry
                .key_path
                .first()
                .map(String::as_str)
                .unwrap_or_default();
            if field == "name" || field == "build" {
                continue;
            }
            if entry.key_path.len() == 2 && entry.key_path[1] == "workspace" {
                if !WORKSPACE_INHERITABLE_FIELDS.contains(&field)
                    || entry.value.as_bool() != Some(true)
                {
                    return Err(invalid_fact(
                        CargoFactReason::MalformedValue,
                        manifest_path,
                        CargoFactKind::Package,
                        static_field(field),
                    ));
                }
                let defaults_id = defaults_id.as_ref().ok_or_else(|| {
                    invalid_fact(
                        CargoFactReason::MissingWorkspaceDeclaration,
                        manifest_path,
                        CargoFactKind::Package,
                        static_field(field),
                    )
                })?;
                if !self.workspace_default_fields.contains(field) {
                    return Err(invalid_fact(
                        CargoFactReason::MissingWorkspaceDeclaration,
                        manifest_path,
                        CargoFactKind::Package,
                        static_field(field),
                    ));
                }
                let field_evidence = self.add_evidence(file, entry.span);
                metadata.push(MetadataFact {
                    field: field.to_owned(),
                    value: DeclaredValue::WorkspaceReference {
                        source_entity_id: defaults_id.clone(),
                        source_field: field.to_owned(),
                    },
                    inherited_from: Some(defaults_id.clone()),
                    evidence_id: field_evidence,
                });
                inherited = true;
                continue;
            }
            if entry.key_path.len() != 1 || !METADATA_FIELDS.contains(&field) {
                return Err(invalid_fact(
                    CargoFactReason::UnsupportedKey,
                    manifest_path,
                    CargoFactKind::Package,
                    static_field(field),
                ));
            }
            let field_evidence = self.add_evidence(file, entry.span);
            let value = self.metadata_value(
                field,
                &entry.value,
                &field_evidence,
                package_base.as_deref().unwrap_or(""),
                manifest_path,
                CargoFactKind::Package,
            )?;
            metadata.push(MetadataFact {
                field: field.to_owned(),
                value,
                inherited_from: None,
                evidence_id: field_evidence,
            });
        }
        check_limit(CargoFactLimit::MetadataFieldsPerOwner, metadata.len())?;
        metadata.sort_by(|left, right| left.field.as_bytes().cmp(right.field.as_bytes()));
        self.add_entity(
            CargoEntity {
                id: package_id.clone(),
                name: package_name,
                properties: CargoEntityProperties::Package(PackageProperties {
                    manifest_id: manifest_id.to_owned(),
                    manifest_path: manifest_path.to_owned(),
                    package_name: package_name_from_id_input(section, manifest_path)?,
                    metadata,
                    evidence_id: evidence_id.clone(),
                }),
            },
            ClaimState::DeterministicFact,
        )?;
        self.add_relationship(
            CargoRelationshipKind::Declares,
            manifest_id.to_owned(),
            package_id.clone(),
            vec![evidence_id.clone()],
            ClaimState::DeterministicFact,
        )?;
        if inherited {
            let defaults_id = defaults_id.ok_or(CargoManifestFactError::ContractInvalid)?;
            self.add_relationship(
                CargoRelationshipKind::ReferencesDeclaration,
                package_id.clone(),
                defaults_id,
                vec![evidence_id.clone()],
                ClaimState::DeterministicFact,
            )?;
            self.add_gap(
                "cargo.workspace_inheritance_not_materialized",
                CargoCoverageState::NotResolved,
                vec![evidence_id],
            );
        }
        if section.entries.iter().any(|entry| {
            matches!(
                entry.key_path.first().map(String::as_str),
                Some("include" | "exclude")
            )
        }) {
            let package_evidence = self
                .entities
                .get(&package_id)
                .map(|entity| entity.properties.evidence_id().to_owned())
                .ok_or(CargoManifestFactError::ContractInvalid)?;
            self.add_gap(
                "cargo.package_file_selection_not_applied",
                CargoCoverageState::NotApplied,
                vec![package_evidence],
            );
        }
        let _ = map;
        Ok(package_id)
    }

    fn parse_direct_metadata(
        &mut self,
        file: &InventoryFile,
        section: &Section,
        manifest_path: &str,
        base: &str,
    ) -> Result<Vec<MetadataFact>, CargoManifestFactError> {
        let mut metadata = Vec::new();
        for entry in &section.entries {
            if entry.key_path.len() != 1 {
                return Err(invalid_fact(
                    CargoFactReason::UnsupportedKey,
                    manifest_path,
                    CargoFactKind::WorkspacePackageDefaults,
                    None,
                ));
            }
            let field = &entry.key_path[0];
            if !WORKSPACE_INHERITABLE_FIELDS.contains(&field.as_str()) {
                return Err(invalid_fact(
                    CargoFactReason::UnsupportedKey,
                    manifest_path,
                    CargoFactKind::WorkspacePackageDefaults,
                    static_field(field),
                ));
            }
            let evidence_id = self.add_evidence(file, entry.span);
            let value = self.metadata_value(
                field,
                &entry.value,
                &evidence_id,
                base,
                manifest_path,
                CargoFactKind::WorkspacePackageDefaults,
            )?;
            metadata.push(MetadataFact {
                field: field.clone(),
                value,
                inherited_from: None,
                evidence_id,
            });
        }
        check_limit(CargoFactLimit::MetadataFieldsPerOwner, metadata.len())?;
        metadata.sort_by(|left, right| left.field.as_bytes().cmp(right.field.as_bytes()));
        Ok(metadata)
    }

    #[allow(clippy::too_many_arguments)]
    fn metadata_value(
        &mut self,
        field: &str,
        value: &toml::Value,
        evidence_id: &str,
        base: &str,
        manifest_path: &str,
        fact_kind: CargoFactKind,
    ) -> Result<DeclaredValue, CargoManifestFactError> {
        match field {
            "version" | "edition" | "rust-version" | "description" | "license" | "default-run"
            | "links" => value
                .as_str()
                .map(|value| checked_string(value, CargoFactLimit::DeclarationStringBytes))
                .transpose()?
                .map(DeclaredValue::String)
                .ok_or_else(|| {
                    invalid_fact(
                        CargoFactReason::MalformedValue,
                        manifest_path,
                        fact_kind,
                        static_field(field),
                    )
                }),
            "documentation" | "homepage" | "repository" => {
                let locator = value.as_str().ok_or_else(|| {
                    invalid_fact(
                        CargoFactReason::MalformedValue,
                        manifest_path,
                        fact_kind,
                        static_field(field),
                    )
                })?;
                check_locator(locator)?;
                let digest = sha256_hex(locator.as_bytes());
                self.add_external_locator_projection(evidence_id)?;
                Ok(DeclaredValue::LocatorSha256(digest))
            }
            "license-file" | "readme" => {
                let declared = value.as_str().ok_or_else(|| {
                    invalid_fact(
                        CargoFactReason::MalformedValue,
                        manifest_path,
                        fact_kind,
                        static_field(field),
                    )
                })?;
                let normalized = normalize_relative_path(base, declared, manifest_path, fact_kind)?;
                Ok(DeclaredValue::Path {
                    declared: declared.to_owned(),
                    normalized,
                })
            }
            "authors" | "keywords" | "categories" | "include" | "exclude" => {
                let values = string_array(value, manifest_path, fact_kind, static_field(field))?;
                Ok(DeclaredValue::StringArray(values))
            }
            "publish" => {
                if value.as_bool() == Some(false) {
                    Ok(DeclaredValue::Publish {
                        enabled: false,
                        registries: Vec::new(),
                    })
                } else if value.is_array() {
                    Ok(DeclaredValue::Publish {
                        enabled: true,
                        registries: string_array(
                            value,
                            manifest_path,
                            fact_kind,
                            static_field(field),
                        )?,
                    })
                } else {
                    Err(invalid_fact(
                        CargoFactReason::MalformedValue,
                        manifest_path,
                        fact_kind,
                        static_field(field),
                    ))
                }
            }
            "autolib" | "autobins" | "autoexamples" | "autotests" | "autobenches" => {
                value.as_bool().map(DeclaredValue::Boolean).ok_or_else(|| {
                    invalid_fact(
                        CargoFactReason::MalformedValue,
                        manifest_path,
                        fact_kind,
                        static_field(field),
                    )
                })
            }
            _ => Err(invalid_fact(
                CargoFactReason::UnsupportedKey,
                manifest_path,
                fact_kind,
                static_field(field),
            )),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn process_targets(
        &mut self,
        file: &InventoryFile,
        map: &ManifestMap,
        manifest_id: &str,
        package_id: Option<&str>,
        manifest_path: &str,
    ) -> Result<(), CargoManifestFactError> {
        let target_sections = map
            .sections
            .iter()
            .filter_map(|section| {
                let kind = match section.path.as_slice() {
                    [value] if value == "lib" => CargoTargetKind::Library,
                    [value] if value == "bin" => CargoTargetKind::Binary,
                    [value] if value == "example" => CargoTargetKind::Example,
                    [value] if value == "test" => CargoTargetKind::Test,
                    [value] if value == "bench" => CargoTargetKind::Bench,
                    _ => return None,
                };
                Some((kind, section))
            })
            .collect::<Vec<_>>();
        check_limit(CargoFactLimit::TargetsPerPackage, target_sections.len())?;
        if target_sections.is_empty() {
            return Ok(());
        }
        let package_id = package_id.ok_or_else(|| {
            invalid_fact(
                CargoFactReason::InvalidTargetDeclaration,
                manifest_path,
                CargoFactKind::Target,
                None,
            )
        })?;
        let package_name = self
            .entities
            .get(package_id)
            .and_then(|entity| match &entity.properties {
                CargoEntityProperties::Package(properties) => Some(properties.package_name.clone()),
                _ => None,
            })
            .ok_or(CargoManifestFactError::ContractInvalid)?;
        let package_base = parent_path(manifest_path).unwrap_or_default();
        let mut seen = BTreeSet::new();
        for (kind, section) in target_sections {
            let fields = section.entry_map(manifest_path, CargoFactKind::Target)?;
            for field in fields.keys() {
                if !matches!(
                    field.as_str(),
                    "name"
                        | "path"
                        | "required-features"
                        | "crate-type"
                        | "proc-macro"
                        | "bench"
                        | "doc"
                        | "doctest"
                        | "test"
                        | "harness"
                        | "edition"
                ) {
                    return Err(invalid_fact(
                        CargoFactReason::UnsupportedKey,
                        manifest_path,
                        CargoFactKind::Target,
                        static_field(field),
                    ));
                }
            }
            let declared_path = fields
                .get("path")
                .map(|entry| required_string(entry, manifest_path, CargoFactKind::Target, "path"))
                .transpose()?;
            let literal_name = fields
                .get("name")
                .map(|entry| required_name(entry, manifest_path, CargoFactKind::Target, "name"))
                .transpose()?;
            let (target_name, name_source) = if let Some(name) = literal_name {
                (name, TargetNameSource::Literal)
            } else if kind == CargoTargetKind::Library {
                (
                    package_name.replace('-', "_"),
                    TargetNameSource::PackageDefault,
                )
            } else if let Some(path) = declared_path.as_deref() {
                (
                    path_stem(path).ok_or_else(|| {
                        invalid_fact(
                            CargoFactReason::InvalidTargetDeclaration,
                            manifest_path,
                            CargoFactKind::Target,
                            Some("name"),
                        )
                    })?,
                    TargetNameSource::PathStem,
                )
            } else if kind == CargoTargetKind::Binary {
                (package_name.clone(), TargetNameSource::PackageDefault)
            } else {
                return Err(invalid_fact(
                    CargoFactReason::InvalidTargetDeclaration,
                    manifest_path,
                    CargoFactKind::Target,
                    Some("name"),
                ));
            };
            if !seen.insert((kind, target_name.clone())) {
                return Err(conflict(manifest_path, CargoFactKind::Target, &target_name));
            }
            let conventional = conventional_target_path(kind, &target_name);
            let declared_path_value = declared_path.unwrap_or(conventional);
            let normalized_path = normalize_relative_path(
                &package_base,
                &declared_path_value,
                manifest_path,
                CargoFactKind::Target,
            )?;
            let path_evidence = fields
                .get("path")
                .map_or(section.header, |entry| entry.span);
            let path_evidence_id = self.add_evidence(file, path_evidence);
            let name_span = fields
                .get("name")
                .map_or(section.header, |entry| ByteRange {
                    start: section.header.start,
                    end: entry.span.end,
                });
            let entity_evidence_id = self.add_evidence(file, name_span);
            let required_features = fields
                .get("required-features")
                .map(|entry| {
                    let evidence_id = self.add_evidence(file, entry.span);
                    string_array(
                        &entry.value,
                        manifest_path,
                        CargoFactKind::Target,
                        Some("required-features"),
                    )
                    .map(|values| {
                        values
                            .into_iter()
                            .map(|value| DeclaredName {
                                value,
                                evidence_id: evidence_id.clone(),
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .transpose()?
                .unwrap_or_default();
            check_limit(
                CargoFactLimit::RequestedFeaturesPerDeclaration,
                required_features.len(),
            )?;
            let options = self.target_options(file, &fields, manifest_path)?;
            let materialized = self
                .workspace
                .plan
                .targets
                .iter()
                .find(|target| {
                    target.manifest_path == manifest_path
                        && target.target_kind.as_str() == kind.as_str()
                        && target.target_name == target_name
                        && target.source_path == normalized_path
                })
                .map(|target| target.crate_id.clone());
            let source_analysis_state = if materialized.is_some() {
                SourceAnalysisState::AnalyzedR3
            } else {
                SourceAnalysisState::NotAnalyzed
            };
            let target_id = cargo_entity_id(
                self.repository_identity,
                CargoEntityKind::Target,
                &[manifest_path, &package_name, kind.as_str(), &target_name],
            );
            self.add_entity(
                CargoEntity {
                    id: target_id.clone(),
                    name: target_name.clone(),
                    properties: CargoEntityProperties::Target(TargetProperties {
                        manifest_id: manifest_id.to_owned(),
                        package_id: package_id.to_owned(),
                        manifest_path: manifest_path.to_owned(),
                        target_kind: kind,
                        target_name,
                        name_source,
                        source_path: DeclaredPath {
                            declared: declared_path_value,
                            normalized: normalized_path,
                            evidence_id: path_evidence_id,
                        },
                        path_source: if fields.contains_key("path") {
                            TargetPathSource::Literal
                        } else {
                            TargetPathSource::Conventional
                        },
                        required_features,
                        options: options.clone(),
                        source_analysis_state,
                        materialized_crate_id: materialized.clone(),
                        evidence_id: entity_evidence_id.clone(),
                    }),
                },
                ClaimState::DeterministicFact,
            )?;
            self.add_relationship(
                CargoRelationshipKind::Declares,
                package_id.to_owned(),
                target_id.clone(),
                vec![entity_evidence_id.clone()],
                ClaimState::DeterministicFact,
            )?;
            if let Some(crate_id) = materialized {
                self.add_relationship(
                    CargoRelationshipKind::Materializes,
                    target_id,
                    crate_id,
                    vec![entity_evidence_id.clone()],
                    ClaimState::DerivedFact,
                )?;
            } else {
                self.add_diagnostic(
                    "cargo.target_source_not_analyzed",
                    "declared Cargo target source is not analyzed by R4",
                    vec![entity_evidence_id.clone()],
                )?;
                self.add_gap(
                    "cargo.target_source_not_analyzed",
                    CargoCoverageState::NotAnalyzed,
                    vec![entity_evidence_id.clone()],
                );
            }
            if options.proc_macro.as_ref().is_some_and(|value| value.value) {
                self.add_gap(
                    "cargo.proc_macro_not_executed",
                    CargoCoverageState::NotExecuted,
                    vec![entity_evidence_id.clone()],
                );
            }
            self.add_gap(
                "cargo.active_target_not_resolved",
                CargoCoverageState::NotResolved,
                vec![entity_evidence_id],
            );
        }
        Ok(())
    }

    fn target_options(
        &mut self,
        file: &InventoryFile,
        fields: &BTreeMap<String, &Entry>,
        manifest_path: &str,
    ) -> Result<TargetOptions, CargoManifestFactError> {
        let crate_types = fields
            .get("crate-type")
            .map(|entry| {
                let evidence_id = self.add_evidence(file, entry.span);
                string_array(
                    &entry.value,
                    manifest_path,
                    CargoFactKind::Target,
                    Some("crate-type"),
                )
                .map(|values| {
                    values
                        .into_iter()
                        .map(|value| DeclaredName {
                            value,
                            evidence_id: evidence_id.clone(),
                        })
                        .collect()
                })
            })
            .transpose()?
            .unwrap_or_default();
        Ok(TargetOptions {
            crate_types,
            proc_macro: self.optional_declared_bool(file, fields, "proc-macro", manifest_path)?,
            bench: self.optional_declared_bool(file, fields, "bench", manifest_path)?,
            doc: self.optional_declared_bool(file, fields, "doc", manifest_path)?,
            doctest: self.optional_declared_bool(file, fields, "doctest", manifest_path)?,
            test: self.optional_declared_bool(file, fields, "test", manifest_path)?,
            harness: self.optional_declared_bool(file, fields, "harness", manifest_path)?,
            edition: fields
                .get("edition")
                .map(|entry| {
                    let value =
                        required_string(entry, manifest_path, CargoFactKind::Target, "edition")?;
                    Ok(DeclaredString {
                        value,
                        evidence_id: self.add_evidence(file, entry.span),
                    })
                })
                .transpose()?,
        })
    }

    fn optional_declared_bool(
        &mut self,
        file: &InventoryFile,
        fields: &BTreeMap<String, &Entry>,
        field: &'static str,
        manifest_path: &str,
    ) -> Result<Option<DeclaredBoolean>, CargoManifestFactError> {
        fields
            .get(field)
            .map(|entry| {
                entry
                    .value
                    .as_bool()
                    .map(|value| DeclaredBoolean {
                        value,
                        evidence_id: self.add_evidence(file, entry.span),
                    })
                    .ok_or_else(|| {
                        invalid_fact(
                            CargoFactReason::MalformedValue,
                            manifest_path,
                            CargoFactKind::Target,
                            Some(field),
                        )
                    })
            })
            .transpose()
    }

    #[allow(clippy::too_many_lines)]
    fn process_dependencies(
        &mut self,
        file: &InventoryFile,
        map: &ManifestMap,
        manifest_id: &str,
        package_id: Option<&str>,
        manifest_path: &str,
    ) -> Result<(), CargoManifestFactError> {
        let mut dependency_count = 0_usize;
        let mut predicates = BTreeSet::new();
        for section in &map.sections {
            let Some((scope, kind, predicate, standard_table_name)) =
                dependency_section(&section.path)
            else {
                continue;
            };
            let declarations =
                dependency_declarations(section, standard_table_name.as_deref(), manifest_path)?;
            let mut declaration_names = BTreeMap::new();
            dependency_count = dependency_count.saturating_add(declarations.len());
            check_limit(CargoFactLimit::DependenciesPerManifest, dependency_count)?;
            if let Some(predicate) = predicate.as_deref() {
                predicates.insert(predicate.to_owned());
                check_limit(
                    CargoFactLimit::TargetPredicatesPerManifest,
                    predicates.len(),
                )?;
            }
            let owner_id = match scope {
                DependencyScope::Workspace => manifest_id,
                DependencyScope::Package => package_id.ok_or_else(|| {
                    invalid_fact(
                        CargoFactReason::MalformedValue,
                        manifest_path,
                        CargoFactKind::Dependency,
                        None,
                    )
                })?,
            };
            for declaration in declarations {
                let declared_name =
                    normalized_name(&declaration.name, manifest_path, CargoFactKind::Dependency)?;
                if let Some(previous) =
                    declaration_names.insert(declared_name.clone(), declaration.name.clone())
                {
                    let reason = if previous == declaration.name {
                        CargoFactReason::DuplicateDeclaration
                    } else {
                        CargoFactReason::UnicodeNormalizationCollision
                    };
                    return Err(invalid_fact(
                        reason,
                        manifest_path,
                        CargoFactKind::Dependency,
                        None,
                    ));
                }
                let evidence_id = self.add_evidence(file, declaration.span);
                let target_predicate = predicate
                    .as_ref()
                    .map(|value| {
                        checked_string(value, CargoFactLimit::DeclarationStringBytes).map(|value| {
                            DeclaredString {
                                value,
                                evidence_id: self.add_evidence(file, section.header),
                            }
                        })
                    })
                    .transpose()?;
                let parsed = self.parse_dependency_value(
                    file,
                    &declaration.value,
                    &evidence_id,
                    manifest_path,
                    scope,
                    &declared_name,
                )?;
                let dependency_id = cargo_entity_id(
                    self.repository_identity,
                    CargoEntityKind::Dependency,
                    &[
                        manifest_path,
                        scope.as_str(),
                        kind.as_str(),
                        predicate.as_deref().unwrap_or(""),
                        &declared_name,
                    ],
                );
                self.add_entity(
                    CargoEntity {
                        id: dependency_id.clone(),
                        name: declared_name.clone(),
                        properties: CargoEntityProperties::Dependency(DependencyProperties {
                            manifest_id: manifest_id.to_owned(),
                            owner_id: owner_id.to_owned(),
                            manifest_path: manifest_path.to_owned(),
                            scope,
                            dependency_kind: kind,
                            target_predicate,
                            declared_name: declared_name.clone(),
                            package_name: parsed.package_name,
                            source: parsed.source.clone(),
                            optional: parsed.optional,
                            default_features: parsed.default_features,
                            requested_features: parsed.requested_features,
                            evidence_id: evidence_id.clone(),
                        }),
                    },
                    ClaimState::DeterministicFact,
                )?;
                self.add_relationship(
                    CargoRelationshipKind::Declares,
                    owner_id.to_owned(),
                    dependency_id.clone(),
                    vec![evidence_id.clone()],
                    ClaimState::DeterministicFact,
                )?;
                if scope == DependencyScope::Workspace
                    && self
                        .workspace_dependencies
                        .insert(declared_name.clone(), dependency_id.clone())
                        .is_some()
                {
                    return Err(conflict(
                        manifest_path,
                        CargoFactKind::Dependency,
                        &declared_name,
                    ));
                }
                if parsed.source.kind == DependencySourceKind::WorkspaceInherited {
                    let target = parsed
                        .source
                        .workspace_reference_id
                        .clone()
                        .ok_or(CargoManifestFactError::ContractInvalid)?;
                    self.add_relationship(
                        CargoRelationshipKind::ReferencesDeclaration,
                        dependency_id,
                        target,
                        vec![evidence_id.clone()],
                        ClaimState::DeterministicFact,
                    )?;
                    self.add_gap(
                        "cargo.workspace_inheritance_not_materialized",
                        CargoCoverageState::NotResolved,
                        vec![evidence_id.clone()],
                    );
                }
                if matches!(
                    parsed.source.kind,
                    DependencySourceKind::Git | DependencySourceKind::Path
                ) {
                    self.add_gap(
                        "cargo.dependency_source_not_fetched",
                        CargoCoverageState::NotFetched,
                        vec![evidence_id.clone()],
                    );
                }
                self.add_gap(
                    "cargo.dependency_graph_not_resolved",
                    CargoCoverageState::NotResolved,
                    vec![evidence_id],
                );
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_lines)]
    fn parse_dependency_value(
        &mut self,
        file: &InventoryFile,
        value: &toml::Value,
        evidence_id: &str,
        manifest_path: &str,
        scope: DependencyScope,
        declared_name: &str,
    ) -> Result<ParsedDependency, CargoManifestFactError> {
        if let Some(version) = value.as_str() {
            return Ok(ParsedDependency {
                package_name: None,
                source: DependencySource {
                    kind: DependencySourceKind::RegistryDefault,
                    version_requirement: Some(DeclaredString {
                        value: checked_string(version, CargoFactLimit::DeclarationStringBytes)?,
                        evidence_id: evidence_id.to_owned(),
                    }),
                    registry_name: None,
                    path: None,
                    git_locator: None,
                    git_reference: None,
                    workspace_reference_id: None,
                },
                optional: None,
                default_features: None,
                requested_features: Vec::new(),
            });
        }
        let table = value.as_table().ok_or_else(|| {
            invalid_fact(
                CargoFactReason::MalformedValue,
                manifest_path,
                CargoFactKind::Dependency,
                None,
            )
        })?;
        let allowed = [
            "version",
            "registry",
            "path",
            "git",
            "branch",
            "tag",
            "rev",
            "package",
            "optional",
            "default-features",
            "features",
            "workspace",
            "artifact",
            "lib",
            "target",
            "public",
        ];
        if let Some(field) = table
            .keys()
            .find(|field| !allowed.contains(&field.as_str()))
        {
            return Err(invalid_fact(
                CargoFactReason::UnsupportedKey,
                manifest_path,
                CargoFactKind::Dependency,
                static_field(field),
            ));
        }
        if table
            .keys()
            .any(|field| matches!(field.as_str(), "artifact" | "lib" | "target" | "public"))
        {
            self.add_diagnostic(
                "cargo.unsupported_manifest_field",
                "Cargo manifest field is outside the selected declaration subset",
                vec![evidence_id.to_owned()],
            )?;
            self.add_gap(
                "cargo.dependency_advanced_fields_unsupported",
                CargoCoverageState::Unsupported,
                vec![evidence_id.to_owned()],
            );
        }
        let version_requirement =
            declared_string_field(table, "version", evidence_id, manifest_path)?;
        let registry_name = declared_string_field(table, "registry", evidence_id, manifest_path)?;
        let package_name = declared_string_field(table, "package", evidence_id, manifest_path)?;
        let optional = declared_bool_field(table, "optional", evidence_id, manifest_path)?;
        let default_features =
            declared_bool_field(table, "default-features", evidence_id, manifest_path)?;
        let requested_features = table
            .get("features")
            .map(|value| {
                string_array(
                    value,
                    manifest_path,
                    CargoFactKind::Dependency,
                    Some("features"),
                )
                .map(|values| {
                    values
                        .into_iter()
                        .map(|value| DeclaredName {
                            value,
                            evidence_id: evidence_id.to_owned(),
                        })
                        .collect::<Vec<_>>()
                })
            })
            .transpose()?
            .unwrap_or_default();
        check_limit(
            CargoFactLimit::RequestedFeaturesPerDeclaration,
            requested_features.len(),
        )?;
        let workspace_inherited = table.get("workspace").and_then(toml::Value::as_bool);
        if table.contains_key("workspace") && workspace_inherited != Some(true) {
            return Err(invalid_fact(
                CargoFactReason::MalformedValue,
                manifest_path,
                CargoFactKind::Dependency,
                Some("workspace"),
            ));
        }
        let has_git = table.contains_key("git");
        let has_path = table.contains_key("path");
        let has_registry = table.contains_key("registry");
        if usize::from(has_git) + usize::from(has_path) + usize::from(has_registry) > 1 {
            return Err(invalid_fact(
                CargoFactReason::ConflictingSourceSelectors,
                manifest_path,
                CargoFactKind::Dependency,
                None,
            ));
        }
        let package_base = parent_path(manifest_path).unwrap_or_default();
        let source = if workspace_inherited == Some(true) {
            if scope != DependencyScope::Package
                || has_git
                || has_path
                || has_registry
                || version_requirement.is_some()
                || package_name.is_some()
            {
                return Err(invalid_fact(
                    CargoFactReason::ConflictingSourceSelectors,
                    manifest_path,
                    CargoFactKind::Dependency,
                    Some("workspace"),
                ));
            }
            let reference = self
                .workspace_dependencies
                .get(declared_name)
                .cloned()
                .ok_or_else(|| {
                    invalid_fact(
                        CargoFactReason::MissingWorkspaceDeclaration,
                        manifest_path,
                        CargoFactKind::Dependency,
                        Some("workspace"),
                    )
                })?;
            DependencySource {
                kind: DependencySourceKind::WorkspaceInherited,
                version_requirement: None,
                registry_name: None,
                path: None,
                git_locator: None,
                git_reference: None,
                workspace_reference_id: Some(reference),
            }
        } else if has_git {
            let locator = table
                .get("git")
                .and_then(toml::Value::as_str)
                .ok_or_else(|| {
                    invalid_fact(
                        CargoFactReason::MalformedValue,
                        manifest_path,
                        CargoFactKind::Dependency,
                        Some("git"),
                    )
                })?;
            check_locator(locator)?;
            let references = ["branch", "tag", "rev"]
                .into_iter()
                .filter(|field| table.contains_key(*field))
                .collect::<Vec<_>>();
            if references.len() > 1 {
                return Err(invalid_fact(
                    CargoFactReason::ConflictingSourceSelectors,
                    manifest_path,
                    CargoFactKind::Dependency,
                    None,
                ));
            }
            let git_reference = references
                .first()
                .map(|field| {
                    let raw = table
                        .get(*field)
                        .and_then(toml::Value::as_str)
                        .ok_or_else(|| {
                            invalid_fact(
                                CargoFactReason::MalformedValue,
                                manifest_path,
                                CargoFactKind::Dependency,
                                static_field(field),
                            )
                        })?;
                    check_locator(raw)?;
                    Ok(LocatorReference {
                        kind: match *field {
                            "branch" => LocatorReferenceKind::Branch,
                            "tag" => LocatorReferenceKind::Tag,
                            _ => LocatorReferenceKind::Revision,
                        },
                        sha256: sha256_hex(raw.as_bytes()),
                        evidence_id: evidence_id.to_owned(),
                    })
                })
                .transpose()?;
            self.add_external_locator_projection(evidence_id)?;
            DependencySource {
                kind: DependencySourceKind::Git,
                version_requirement,
                registry_name: None,
                path: None,
                git_locator: Some(LocatorDigest {
                    sha256: sha256_hex(locator.as_bytes()),
                    evidence_id: evidence_id.to_owned(),
                }),
                git_reference,
                workspace_reference_id: None,
            }
        } else if has_path {
            let declared = table
                .get("path")
                .and_then(toml::Value::as_str)
                .ok_or_else(|| {
                    invalid_fact(
                        CargoFactReason::MalformedValue,
                        manifest_path,
                        CargoFactKind::Dependency,
                        Some("path"),
                    )
                })?;
            DependencySource {
                kind: DependencySourceKind::Path,
                version_requirement,
                registry_name: None,
                path: Some(DeclaredPath {
                    declared: declared.to_owned(),
                    normalized: normalize_relative_path(
                        &package_base,
                        declared,
                        manifest_path,
                        CargoFactKind::Dependency,
                    )?,
                    evidence_id: evidence_id.to_owned(),
                }),
                git_locator: None,
                git_reference: None,
                workspace_reference_id: None,
            }
        } else if has_registry {
            DependencySource {
                kind: DependencySourceKind::RegistryNamed,
                version_requirement,
                registry_name,
                path: None,
                git_locator: None,
                git_reference: None,
                workspace_reference_id: None,
            }
        } else {
            if version_requirement.is_none() {
                return Err(invalid_fact(
                    CargoFactReason::MalformedValue,
                    manifest_path,
                    CargoFactKind::Dependency,
                    Some("version"),
                ));
            }
            DependencySource {
                kind: DependencySourceKind::RegistryDefault,
                version_requirement,
                registry_name: None,
                path: None,
                git_locator: None,
                git_reference: None,
                workspace_reference_id: None,
            }
        };
        let _ = file;
        Ok(ParsedDependency {
            package_name,
            source,
            optional,
            default_features,
            requested_features,
        })
    }

    fn process_features(
        &mut self,
        file: &InventoryFile,
        map: &ManifestMap,
        manifest_id: &str,
        package_id: Option<&str>,
        manifest_path: &str,
    ) -> Result<(), CargoManifestFactError> {
        let Some(section) = map.single_section(&["features"])? else {
            return Ok(());
        };
        check_limit(CargoFactLimit::FeaturesPerManifest, section.entries.len())?;
        let package_id = package_id.ok_or_else(|| {
            invalid_fact(
                CargoFactReason::MalformedValue,
                manifest_path,
                CargoFactKind::Feature,
                None,
            )
        })?;
        let package_name = self.package_name(package_id)?;
        let mut names = BTreeSet::new();
        for entry in &section.entries {
            if entry.key_path.len() != 1 {
                return Err(invalid_fact(
                    CargoFactReason::InvalidDeclarationName,
                    manifest_path,
                    CargoFactKind::Feature,
                    None,
                ));
            }
            let feature_name =
                normalized_name(&entry.key_path[0], manifest_path, CargoFactKind::Feature)?;
            if !names.insert(feature_name.clone()) {
                return Err(conflict(
                    manifest_path,
                    CargoFactKind::Feature,
                    &feature_name,
                ));
            }
            let evidence_id = self.add_evidence(file, entry.span);
            let lexemes = string_array(
                &entry.value,
                manifest_path,
                CargoFactKind::Feature,
                Some("members"),
            )?;
            check_limit(CargoFactLimit::FeatureMembersPerFeature, lexemes.len())?;
            let mut members = lexemes
                .into_iter()
                .map(|lexeme| parse_feature_member(lexeme, &evidence_id, manifest_path))
                .collect::<Result<Vec<_>, _>>()?;
            members.sort_by(|left, right| {
                (left.syntax, left.lexeme.as_bytes()).cmp(&(right.syntax, right.lexeme.as_bytes()))
            });
            let feature_id = cargo_entity_id(
                self.repository_identity,
                CargoEntityKind::Feature,
                &[manifest_path, &package_name, &feature_name],
            );
            self.add_entity(
                CargoEntity {
                    id: feature_id.clone(),
                    name: feature_name.clone(),
                    properties: CargoEntityProperties::Feature(FeatureProperties {
                        manifest_id: manifest_id.to_owned(),
                        package_id: package_id.to_owned(),
                        manifest_path: manifest_path.to_owned(),
                        feature_name,
                        members,
                        evidence_id: evidence_id.clone(),
                    }),
                },
                ClaimState::DeterministicFact,
            )?;
            self.add_relationship(
                CargoRelationshipKind::Declares,
                package_id.to_owned(),
                feature_id,
                vec![evidence_id.clone()],
                ClaimState::DeterministicFact,
            )?;
            self.add_gap(
                "cargo.active_features_not_resolved",
                CargoCoverageState::NotResolved,
                vec![evidence_id],
            );
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn process_patches(
        &mut self,
        file: &InventoryFile,
        map: &ManifestMap,
        manifest_id: &str,
        manifest_path: &str,
    ) -> Result<(), CargoManifestFactError> {
        let sections = map
            .sections
            .iter()
            .filter(|section| section.path.first().is_some_and(|value| value == "patch"))
            .collect::<Vec<_>>();
        let patch_count = sections
            .iter()
            .map(|section| section.entries.len())
            .sum::<usize>();
        check_limit(CargoFactLimit::PatchesPerWorkspace, patch_count)?;
        for section in sections {
            if section.path.len() != 2 {
                return Err(invalid_fact(
                    CargoFactReason::UnsupportedKey,
                    manifest_path,
                    CargoFactKind::Patch,
                    None,
                ));
            }
            let selector_evidence_id = self.add_evidence(file, section.header);
            let raw_selector = &section.path[1];
            let (selector_kind, selector_identity, selector_name, selector_sha256) =
                if raw_selector == "crates-io" {
                    (
                        PatchSelectorKind::CratesIo,
                        "crates-io".to_owned(),
                        Some("crates-io".to_owned()),
                        None,
                    )
                } else if raw_selector.contains("://") {
                    check_locator(raw_selector)?;
                    let digest = sha256_hex(raw_selector.as_bytes());
                    self.add_external_locator_projection(&selector_evidence_id)?;
                    (
                        PatchSelectorKind::SourceLocatorSha256,
                        digest.clone(),
                        None,
                        Some(digest),
                    )
                } else {
                    let name = normalized_name(raw_selector, manifest_path, CargoFactKind::Patch)?;
                    (
                        PatchSelectorKind::NamedRegistry,
                        name.clone(),
                        Some(name.clone()),
                        None,
                    )
                };
            for entry in &section.entries {
                if entry.key_path.len() != 1 {
                    return Err(invalid_fact(
                        CargoFactReason::InvalidDeclarationName,
                        manifest_path,
                        CargoFactKind::Patch,
                        None,
                    ));
                }
                let declared_name =
                    normalized_name(&entry.key_path[0], manifest_path, CargoFactKind::Patch)?;
                let evidence_id = self.add_evidence(file, entry.span);
                let parsed = self.parse_patch_value(&entry.value, &evidence_id, manifest_path)?;
                let patch_id = cargo_entity_id(
                    self.repository_identity,
                    CargoEntityKind::Patch,
                    &[
                        manifest_path,
                        selector_kind.as_str(),
                        &selector_identity,
                        &declared_name,
                    ],
                );
                self.add_entity(
                    CargoEntity {
                        id: patch_id.clone(),
                        name: declared_name.clone(),
                        properties: CargoEntityProperties::Patch(PatchProperties {
                            manifest_id: manifest_id.to_owned(),
                            manifest_path: manifest_path.to_owned(),
                            source_selector: PatchSourceSelector {
                                kind: selector_kind,
                                name: selector_name.clone(),
                                sha256: selector_sha256.clone(),
                                evidence_id: selector_evidence_id.clone(),
                            },
                            declared_name,
                            package_name: parsed.package_name,
                            source: parsed.source,
                            evidence_id: evidence_id.clone(),
                        }),
                    },
                    ClaimState::DeterministicFact,
                )?;
                self.add_relationship(
                    CargoRelationshipKind::Declares,
                    manifest_id.to_owned(),
                    patch_id,
                    vec![evidence_id.clone(), selector_evidence_id.clone()],
                    ClaimState::DeterministicFact,
                )?;
                self.add_gap(
                    "cargo.patch_not_applied",
                    CargoCoverageState::NotApplied,
                    vec![evidence_id],
                );
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn parse_patch_value(
        &mut self,
        value: &toml::Value,
        evidence_id: &str,
        manifest_path: &str,
    ) -> Result<ParsedPatch, CargoManifestFactError> {
        let table = value.as_table().ok_or_else(|| {
            invalid_fact(
                CargoFactReason::MalformedValue,
                manifest_path,
                CargoFactKind::Patch,
                None,
            )
        })?;
        let allowed = [
            "version", "registry", "path", "git", "branch", "tag", "rev", "package",
        ];
        if let Some(field) = table
            .keys()
            .find(|field| !allowed.contains(&field.as_str()))
        {
            return Err(invalid_fact(
                CargoFactReason::UnsupportedKey,
                manifest_path,
                CargoFactKind::Patch,
                static_field(field),
            ));
        }
        let version_requirement =
            declared_string_field(table, "version", evidence_id, manifest_path)?;
        let registry_name = declared_string_field(table, "registry", evidence_id, manifest_path)?;
        let package_name = declared_string_field(table, "package", evidence_id, manifest_path)?;
        let has_git = table.contains_key("git");
        let has_path = table.contains_key("path");
        let has_registry = table.contains_key("registry");
        if usize::from(has_git) + usize::from(has_path) + usize::from(has_registry) > 1 {
            return Err(invalid_fact(
                CargoFactReason::ConflictingSourceSelectors,
                manifest_path,
                CargoFactKind::Patch,
                None,
            ));
        }
        let base = parent_path(manifest_path).unwrap_or_default();
        let source = if has_git {
            let locator = table
                .get("git")
                .and_then(toml::Value::as_str)
                .ok_or_else(|| {
                    invalid_fact(
                        CargoFactReason::MalformedValue,
                        manifest_path,
                        CargoFactKind::Patch,
                        Some("git"),
                    )
                })?;
            check_locator(locator)?;
            let references = ["branch", "tag", "rev"]
                .into_iter()
                .filter(|field| table.contains_key(*field))
                .collect::<Vec<_>>();
            if references.len() > 1 {
                return Err(invalid_fact(
                    CargoFactReason::ConflictingSourceSelectors,
                    manifest_path,
                    CargoFactKind::Patch,
                    None,
                ));
            }
            let git_reference = references
                .first()
                .map(|field| {
                    let raw = table
                        .get(*field)
                        .and_then(toml::Value::as_str)
                        .ok_or_else(|| {
                            invalid_fact(
                                CargoFactReason::MalformedValue,
                                manifest_path,
                                CargoFactKind::Patch,
                                static_field(field),
                            )
                        })?;
                    check_locator(raw)?;
                    Ok(LocatorReference {
                        kind: match *field {
                            "branch" => LocatorReferenceKind::Branch,
                            "tag" => LocatorReferenceKind::Tag,
                            _ => LocatorReferenceKind::Revision,
                        },
                        sha256: sha256_hex(raw.as_bytes()),
                        evidence_id: evidence_id.to_owned(),
                    })
                })
                .transpose()?;
            self.add_external_locator_projection(evidence_id)?;
            DependencySource {
                kind: DependencySourceKind::Git,
                version_requirement,
                registry_name: None,
                path: None,
                git_locator: Some(LocatorDigest {
                    sha256: sha256_hex(locator.as_bytes()),
                    evidence_id: evidence_id.to_owned(),
                }),
                git_reference,
                workspace_reference_id: None,
            }
        } else if has_path {
            let declared = table
                .get("path")
                .and_then(toml::Value::as_str)
                .ok_or_else(|| {
                    invalid_fact(
                        CargoFactReason::MalformedValue,
                        manifest_path,
                        CargoFactKind::Patch,
                        Some("path"),
                    )
                })?;
            DependencySource {
                kind: DependencySourceKind::Path,
                version_requirement,
                registry_name: None,
                path: Some(DeclaredPath {
                    declared: declared.to_owned(),
                    normalized: normalize_relative_path(
                        &base,
                        declared,
                        manifest_path,
                        CargoFactKind::Patch,
                    )?,
                    evidence_id: evidence_id.to_owned(),
                }),
                git_locator: None,
                git_reference: None,
                workspace_reference_id: None,
            }
        } else if has_registry {
            DependencySource {
                kind: DependencySourceKind::RegistryNamed,
                version_requirement,
                registry_name,
                path: None,
                git_locator: None,
                git_reference: None,
                workspace_reference_id: None,
            }
        } else if version_requirement.is_some() {
            DependencySource {
                kind: DependencySourceKind::RegistryDefault,
                version_requirement,
                registry_name: None,
                path: None,
                git_locator: None,
                git_reference: None,
                workspace_reference_id: None,
            }
        } else {
            return Err(invalid_fact(
                CargoFactReason::MalformedValue,
                manifest_path,
                CargoFactKind::Patch,
                None,
            ));
        };
        Ok(ParsedPatch {
            package_name,
            source,
        })
    }

    #[allow(clippy::too_many_lines)]
    fn process_build_script(
        &mut self,
        file: &InventoryFile,
        map: &ManifestMap,
        manifest_id: &str,
        package_id: &str,
        manifest_path: &str,
    ) -> Result<(), CargoManifestFactError> {
        let package = map
            .single_section(&["package"])?
            .ok_or(CargoManifestFactError::ContractInvalid)?;
        let build_entry =
            package.optional_entry(&["build"], manifest_path, CargoFactKind::BuildScript)?;
        let package_name = self.package_name(package_id)?;
        let package_base = parent_path(manifest_path).unwrap_or_default();
        let conventional_path = join_path(&package_base, "build.rs");
        let conventional_present = self.files.contains_key(conventional_path.as_str());
        let (selection, path, committed_present, evidence_range, claim_state) =
            if let Some(entry) = build_entry {
                if let Some(declared) = entry.value.as_str() {
                    let normalized = normalize_relative_path(
                        &package_base,
                        declared,
                        manifest_path,
                        CargoFactKind::BuildScript,
                    )?;
                    let present = self.files.contains_key(normalized.as_str());
                    let evidence_id = self.add_evidence(file, entry.span);
                    (
                        BuildScriptSelection::ExplicitPath,
                        Some(DeclaredPath {
                            declared: declared.to_owned(),
                            normalized,
                            evidence_id,
                        }),
                        present,
                        entry.span,
                        ClaimState::DeterministicFact,
                    )
                } else if entry.value.as_bool() == Some(false) {
                    (
                        BuildScriptSelection::ExplicitDisabled,
                        None,
                        false,
                        entry.span,
                        ClaimState::DeterministicFact,
                    )
                } else {
                    return Err(invalid_fact(
                        CargoFactReason::MalformedValue,
                        manifest_path,
                        CargoFactKind::BuildScript,
                        Some("build"),
                    ));
                }
            } else if conventional_present {
                let evidence_id = self.add_evidence(file, package.header);
                (
                    BuildScriptSelection::ConventionalPresent,
                    Some(DeclaredPath {
                        declared: "build.rs".to_owned(),
                        normalized: conventional_path,
                        evidence_id,
                    }),
                    true,
                    package.header,
                    ClaimState::DerivedFact,
                )
            } else {
                (
                    BuildScriptSelection::Absent,
                    None,
                    false,
                    package.header,
                    ClaimState::DerivedFact,
                )
            };
        let evidence_id = self.add_evidence(file, evidence_range);
        let build_id = cargo_entity_id(
            self.repository_identity,
            CargoEntityKind::BuildScript,
            &[manifest_path, &package_name],
        );
        self.add_entity(
            CargoEntity {
                id: build_id.clone(),
                name: "build-script".to_owned(),
                properties: CargoEntityProperties::BuildScript(BuildScriptProperties {
                    manifest_id: manifest_id.to_owned(),
                    package_id: package_id.to_owned(),
                    manifest_path: manifest_path.to_owned(),
                    selection,
                    path,
                    committed_present,
                    evidence_id: evidence_id.clone(),
                }),
            },
            claim_state,
        )?;
        self.add_relationship(
            CargoRelationshipKind::Declares,
            package_id.to_owned(),
            build_id,
            vec![evidence_id.clone()],
            claim_state,
        )?;
        if committed_present && selection != BuildScriptSelection::ExplicitDisabled {
            self.add_gap(
                "cargo.build_script_not_executed",
                CargoCoverageState::NotExecuted,
                vec![evidence_id.clone()],
            );
            self.add_gap(
                "cargo.generated_source_not_analyzed",
                CargoCoverageState::NotAnalyzed,
                vec![evidence_id],
            );
        }
        Ok(())
    }

    fn process_typed_unsupported(
        &mut self,
        file: &InventoryFile,
        map: &ManifestMap,
        _manifest_path: &str,
    ) -> Result<(), CargoManifestFactError> {
        for section in &map.sections {
            let capability = match section.path.as_slice() {
                [value] if value == "badges" => Some("cargo.legacy_badges_unsupported"),
                [first, ..] if first == "profile" => Some("cargo.profile_tables_unsupported"),
                [first, ..] if first == "lints" => Some("cargo.lint_configuration_unsupported"),
                [first, second, ..] if first == "workspace" && second == "lints" => {
                    Some("cargo.lint_configuration_unsupported")
                }
                [first, second, ..]
                    if (first == "package" || first == "workspace") && second == "metadata" =>
                {
                    Some("cargo.package_metadata_table_unsupported")
                }
                [first, ..] if first == "replace" => Some("cargo.replace_table_unsupported"),
                _ => None,
            };
            if let Some(capability) = capability {
                let evidence_id = self.add_evidence(file, section.header);
                self.add_diagnostic(
                    "cargo.unsupported_manifest_family",
                    "Cargo manifest family is outside the selected declaration subset",
                    vec![evidence_id.clone()],
                )?;
                self.add_gap(
                    capability,
                    CargoCoverageState::Unsupported,
                    vec![evidence_id],
                );
            }
        }
        Ok(())
    }

    fn reject_unrecognized_sections(
        map: &ManifestMap,
        manifest_path: &str,
    ) -> Result<(), CargoManifestFactError> {
        for section in &map.sections {
            let recognized = dependency_section(&section.path).is_some()
                || match section.path.as_slice() {
                    [value]
                        if matches!(
                            value.as_str(),
                            "workspace"
                                | "package"
                                | "lib"
                                | "bin"
                                | "example"
                                | "test"
                                | "bench"
                                | "dependencies"
                                | "dev-dependencies"
                                | "build-dependencies"
                                | "features"
                                | "badges"
                                | "profile"
                                | "lints"
                                | "replace"
                        ) =>
                    {
                        true
                    }
                    [first, second]
                        if first == "workspace"
                            && matches!(
                                second.as_str(),
                                "package" | "dependencies" | "metadata" | "lints"
                            ) =>
                    {
                        true
                    }
                    [first, ..] if matches!(first.as_str(), "profile" | "lints" | "replace") => {
                        true
                    }
                    [first, second, ..]
                        if (first == "package" || first == "workspace")
                            && matches!(second.as_str(), "metadata" | "lints") =>
                    {
                        true
                    }
                    [first, _] if first == "patch" => true,
                    [first, _, third]
                        if first == "target"
                            && matches!(
                                third.as_str(),
                                "dependencies" | "dev-dependencies" | "build-dependencies"
                            ) =>
                    {
                        true
                    }
                    _ => false,
                };
            if !recognized {
                return Err(invalid_fact(
                    CargoFactReason::UnsupportedKey,
                    manifest_path,
                    CargoFactKind::Manifest,
                    None,
                ));
            }
        }
        Ok(())
    }

    fn package_name(&self, package_id: &str) -> Result<String, CargoManifestFactError> {
        self.entities
            .get(package_id)
            .and_then(|entity| match &entity.properties {
                CargoEntityProperties::Package(properties) => Some(properties.package_name.clone()),
                _ => None,
            })
            .ok_or(CargoManifestFactError::ContractInvalid)
    }

    fn add_entity(
        &mut self,
        entity: CargoEntity,
        state: ClaimState,
    ) -> Result<(), CargoManifestFactError> {
        if self
            .entities
            .insert(entity.id.clone(), entity.clone())
            .is_some()
            || self.entity_states.insert(entity.id, state).is_some()
        {
            return Err(CargoManifestFactError::ContractInvalid);
        }
        Ok(())
    }

    fn add_relationship(
        &mut self,
        kind: CargoRelationshipKind,
        source: String,
        target: String,
        evidence_ids: Vec<String>,
        state: ClaimState,
    ) -> Result<(), CargoManifestFactError> {
        let relationship = CargoRelationship::new(kind, source, target, evidence_ids);
        if self
            .relationships
            .insert(relationship.id.clone(), relationship.clone())
            .is_some()
            || self
                .relationship_states
                .insert(relationship.id, state)
                .is_some()
        {
            return Err(CargoManifestFactError::ContractInvalid);
        }
        Ok(())
    }

    fn add_evidence(&mut self, file: &InventoryFile, range: ByteRange) -> String {
        let identifier = workspace_evidence_id(
            self.repository_identity,
            self.commit_oid,
            file.blob_oid().as_str(),
            file.path(),
            u64::try_from(range.start).unwrap_or(u64::MAX),
            u64::try_from(range.end).unwrap_or(u64::MAX),
        );
        self.evidence
            .entry(identifier.clone())
            .or_insert_with(|| WorkspaceEvidence {
                id: identifier.clone(),
                path: file.path().to_owned(),
                blob_oid: file.blob_oid().as_str().to_owned(),
                start_byte: u64::try_from(range.start).unwrap_or(u64::MAX),
                end_byte: u64::try_from(range.end).unwrap_or(u64::MAX),
            });
        identifier
    }

    fn add_diagnostic(
        &mut self,
        code: &str,
        message: &str,
        evidence_ids: Vec<String>,
    ) -> Result<(), CargoManifestFactError> {
        let diagnostic =
            CargoDiagnostic::new(self.repository_identity, code, message, evidence_ids);
        if self
            .diagnostics
            .insert(diagnostic.id.clone(), diagnostic)
            .is_some()
        {
            return Err(CargoManifestFactError::ContractInvalid);
        }
        Ok(())
    }

    fn add_gap(&mut self, capability: &str, state: CargoCoverageState, evidence_ids: Vec<String>) {
        let gap = CargoCoverageGap::new(
            self.repository_identity,
            self.commit_oid,
            capability,
            state,
            evidence_ids,
        );
        self.coverage.entry(gap.id.clone()).or_insert(gap);
    }

    fn add_external_locator_projection(
        &mut self,
        evidence_id: &str,
    ) -> Result<(), CargoManifestFactError> {
        self.add_diagnostic(
            "cargo.external_locator_redacted",
            "external Cargo locator is represented only by digest",
            vec![evidence_id.to_owned()],
        )?;
        self.add_gap(
            "cargo.external_locator_redacted",
            CargoCoverageState::Redacted,
            vec![evidence_id.to_owned()],
        );
        Ok(())
    }

    fn build_claims(
        &self,
    ) -> Result<Vec<codenoesis_domain::s4::WorkspaceClaim>, CargoManifestFactError> {
        let mut claims = BTreeMap::new();
        for (identifier, entity) in &self.entities {
            let state = *self
                .entity_states
                .get(identifier)
                .ok_or(CargoManifestFactError::ContractInvalid)?;
            let claim = cargo_claim(
                ClaimSubjectKind::Entity,
                identifier.clone(),
                state,
                vec![entity.properties.evidence_id().to_owned()],
            );
            claims.insert(claim.id.clone(), claim);
        }
        for (identifier, relationship) in &self.relationships {
            let state = *self
                .relationship_states
                .get(identifier)
                .ok_or(CargoManifestFactError::ContractInvalid)?;
            let claim = cargo_claim(
                ClaimSubjectKind::Relationship,
                identifier.clone(),
                state,
                relationship.evidence_ids.clone(),
            );
            claims.insert(claim.id.clone(), claim);
        }
        Ok(claims.into_values().collect())
    }

    fn build_manifest_index(&self) -> Vec<ManifestIndexEntry> {
        let manifests = self
            .entities
            .values()
            .filter_map(|entity| match &entity.properties {
                CargoEntityProperties::Manifest(properties) => {
                    Some((entity.id.clone(), properties.manifest_path.clone()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let mut index = manifests
            .into_iter()
            .map(|(manifest_id, manifest_path)| {
                let mut fact_entity_ids = self
                    .entities
                    .values()
                    .filter(|entity| entity.properties.manifest_path() == manifest_path)
                    .map(|entity| entity.id.clone())
                    .collect::<Vec<_>>();
                fact_entity_ids.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
                let package_id =
                    self.entities
                        .values()
                        .find_map(|entity| match &entity.properties {
                            CargoEntityProperties::Package(properties)
                                if properties.manifest_path == manifest_path =>
                            {
                                Some(entity.id.clone())
                            }
                            _ => None,
                        });
                ManifestIndexEntry {
                    manifest_id,
                    manifest_path,
                    package_id,
                    fact_entity_ids,
                }
            })
            .collect::<Vec<_>>();
        index.sort_by(|left, right| {
            left.manifest_path
                .as_bytes()
                .cmp(right.manifest_path.as_bytes())
        });
        index
    }

    fn build_chunks(
        &self,
        claims: &[codenoesis_domain::s4::WorkspaceClaim],
        index: &[ManifestIndexEntry],
    ) -> Result<Vec<CargoManifestExtractionChunk>, CargoManifestFactError> {
        let claims_by_subject = claims
            .iter()
            .map(|claim| (claim.subject_id.as_str(), claim))
            .collect::<BTreeMap<_, _>>();
        index
            .iter()
            .map(|entry| {
                let entities = entry
                    .fact_entity_ids
                    .iter()
                    .map(|identifier| {
                        self.entities
                            .get(identifier)
                            .cloned()
                            .ok_or(CargoManifestFactError::ContractInvalid)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let entity_ids = entities
                    .iter()
                    .map(|entity| entity.id.as_str())
                    .collect::<BTreeSet<_>>();
                let relationships = self
                    .relationships
                    .values()
                    .filter(|relationship| entity_ids.contains(relationship.source.as_str()))
                    .cloned()
                    .collect::<Vec<_>>();
                let mut chunk_claims = entities
                    .iter()
                    .map(|entity| claims_by_subject.get(entity.id.as_str()).copied())
                    .chain(relationships.iter().map(|relationship| {
                        claims_by_subject.get(relationship.id.as_str()).copied()
                    }))
                    .collect::<Option<Vec<_>>>()
                    .ok_or(CargoManifestFactError::ContractInvalid)?
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>();
                chunk_claims.sort_by(|left, right| left.id.as_bytes().cmp(right.id.as_bytes()));
                let evidence = self
                    .evidence
                    .values()
                    .filter(|evidence| evidence.path == entry.manifest_path)
                    .cloned()
                    .collect::<Vec<_>>();
                let diagnostics = self
                    .diagnostics
                    .values()
                    .filter(|diagnostic| {
                        diagnostic.evidence_ids.iter().any(|identifier| {
                            self.evidence
                                .get(identifier)
                                .is_some_and(|evidence| evidence.path == entry.manifest_path)
                        })
                    })
                    .cloned()
                    .collect();
                let coverage = self
                    .coverage
                    .values()
                    .filter(|gap| {
                        gap.evidence_ids.iter().any(|identifier| {
                            self.evidence
                                .get(identifier)
                                .is_some_and(|evidence| evidence.path == entry.manifest_path)
                        })
                    })
                    .cloned()
                    .collect();
                Ok(CargoManifestExtractionChunk {
                    manifest_id: entry.manifest_id.clone(),
                    manifest_path: entry.manifest_path.clone(),
                    entities,
                    relationships,
                    claims: chunk_claims,
                    evidence,
                    diagnostics,
                    coverage,
                })
            })
            .collect::<Result<Vec<_>, _>>()
    }
}

struct ParsedDependency {
    package_name: Option<DeclaredString>,
    source: DependencySource,
    optional: Option<DeclaredBoolean>,
    default_features: Option<DeclaredBoolean>,
    requested_features: Vec<DeclaredName>,
}

struct DependencyDeclaration {
    name: String,
    value: toml::Value,
    span: ByteRange,
}

struct ParsedPatch {
    package_name: Option<DeclaredString>,
    source: DependencySource,
}

#[derive(Clone, Copy)]
struct ByteRange {
    start: usize,
    end: usize,
}

struct Entry {
    key_path: Vec<String>,
    value: toml::Value,
    span: ByteRange,
}

struct Section {
    path: Vec<String>,
    header: ByteRange,
    entries: Vec<Entry>,
}

impl Section {
    fn single_entry(
        &self,
        key: &[&str],
        manifest_path: &str,
        fact_kind: CargoFactKind,
    ) -> Result<&Entry, CargoManifestFactError> {
        self.optional_entry(key, manifest_path, fact_kind)?
            .ok_or_else(|| {
                invalid_fact(
                    CargoFactReason::MalformedValue,
                    manifest_path,
                    fact_kind,
                    key.first().and_then(|field| static_field(field)),
                )
            })
    }

    fn optional_entry(
        &self,
        key: &[&str],
        manifest_path: &str,
        fact_kind: CargoFactKind,
    ) -> Result<Option<&Entry>, CargoManifestFactError> {
        let matches = self
            .entries
            .iter()
            .filter(|entry| {
                entry.key_path.len() == key.len()
                    && entry
                        .key_path
                        .iter()
                        .zip(key)
                        .all(|(left, right)| left == right)
            })
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => Ok(None),
            [entry] => Ok(Some(entry)),
            _ => Err(conflict(
                manifest_path,
                fact_kind,
                key.first().copied().unwrap_or("declaration"),
            )),
        }
    }

    fn entry_map<'a>(
        &'a self,
        manifest_path: &str,
        fact_kind: CargoFactKind,
    ) -> Result<BTreeMap<String, &'a Entry>, CargoManifestFactError> {
        let mut fields = BTreeMap::new();
        for entry in &self.entries {
            if entry.key_path.len() != 1 {
                return Err(invalid_fact(
                    CargoFactReason::UnsupportedKey,
                    manifest_path,
                    fact_kind,
                    None,
                ));
            }
            let field = entry.key_path[0].clone();
            if fields.insert(field.clone(), entry).is_some() {
                return Err(conflict(manifest_path, fact_kind, &field));
            }
        }
        Ok(fields)
    }
}

struct ManifestMap {
    sections: Vec<Section>,
}

impl ManifestMap {
    fn parse(source: &str, manifest_path: &str) -> Result<Self, CargoManifestFactError> {
        let _: toml::Value = toml::from_str(source).map_err(|_| {
            invalid_fact(
                CargoFactReason::MalformedValue,
                manifest_path,
                CargoFactKind::Manifest,
                None,
            )
        })?;
        let lines = source_lines(source);
        let mut sections = Vec::new();
        let mut current = Section {
            path: Vec::new(),
            header: ByteRange { start: 0, end: 0 },
            entries: Vec::new(),
        };
        let mut line_index = 0_usize;
        while line_index < lines.len() {
            let line = &lines[line_index];
            let trimmed = source[line.start..line.end].trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                line_index += 1;
                continue;
            }
            if trimmed.starts_with('[') {
                if !current.path.is_empty() || !current.entries.is_empty() {
                    sections.push(current);
                }
                let (path, array) = parse_header(trimmed, manifest_path)?;
                if array
                    && !matches!(
                        path.first().map(String::as_str),
                        Some("bin" | "example" | "test" | "bench")
                    )
                {
                    return Err(invalid_fact(
                        CargoFactReason::UnsupportedKey,
                        manifest_path,
                        CargoFactKind::Manifest,
                        None,
                    ));
                }
                current = Section {
                    path,
                    header: *line,
                    entries: Vec::new(),
                };
                line_index += 1;
                continue;
            }
            let start = line.start;
            let mut end_index = line_index;
            let mut parsed = None;
            while end_index < lines.len() {
                let end = lines[end_index].end;
                let candidate = &source[start..end];
                if let Ok(entry) = parse_entry(candidate, ByteRange { start, end }, manifest_path) {
                    parsed = Some(entry);
                    break;
                }
                if end_index + 1 >= lines.len() {
                    break;
                }
                let next = source[lines[end_index + 1].start..lines[end_index + 1].end].trim();
                if next.starts_with('[') {
                    break;
                }
                end_index += 1;
            }
            let entry = parsed.ok_or_else(|| {
                invalid_fact(
                    CargoFactReason::MalformedValue,
                    manifest_path,
                    CargoFactKind::Manifest,
                    None,
                )
            })?;
            current.entries.push(entry);
            line_index = end_index + 1;
        }
        if !current.path.is_empty() || !current.entries.is_empty() {
            sections.push(current);
        }
        Ok(Self { sections })
    }

    fn single_section(&self, path: &[&str]) -> Result<Option<&Section>, CargoManifestFactError> {
        let matches = self
            .sections
            .iter()
            .filter(|section| {
                section.path.len() == path.len()
                    && section
                        .path
                        .iter()
                        .zip(path)
                        .all(|(left, right)| left == right)
            })
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => Ok(None),
            [section] => Ok(Some(section)),
            _ => Err(CargoManifestFactError::ContractInvalid),
        }
    }
}

fn source_lines(source: &str) -> Vec<ByteRange> {
    let mut lines = Vec::new();
    let mut start = 0_usize;
    for segment in source.split_inclusive('\n') {
        let end = start + segment.strip_suffix('\n').map_or(segment.len(), str::len);
        lines.push(ByteRange { start, end });
        start += segment.len();
    }
    if source.is_empty() {
        lines.push(ByteRange { start: 0, end: 0 });
    } else if !source.ends_with('\n') && lines.last().is_none_or(|line| line.end != source.len()) {
        lines.push(ByteRange {
            start,
            end: source.len(),
        });
    }
    lines
}

fn parse_header(
    header: &str,
    manifest_path: &str,
) -> Result<(Vec<String>, bool), CargoManifestFactError> {
    let array = header.starts_with("[[");
    let inner = if array {
        header
            .strip_prefix("[[")
            .and_then(|value| value.strip_suffix("]]"))
    } else {
        header
            .strip_prefix('[')
            .and_then(|value| value.strip_suffix(']'))
    }
    .ok_or_else(|| {
        invalid_fact(
            CargoFactReason::MalformedValue,
            manifest_path,
            CargoFactKind::Manifest,
            None,
        )
    })?;
    Ok((parse_key_path(inner, manifest_path)?, array))
}

fn parse_entry(
    source: &str,
    span: ByteRange,
    manifest_path: &str,
) -> Result<Entry, CargoManifestFactError> {
    let equals = find_unquoted_equals(source).ok_or_else(|| {
        invalid_fact(
            CargoFactReason::MalformedValue,
            manifest_path,
            CargoFactKind::Manifest,
            None,
        )
    })?;
    let key = source[..equals].trim();
    let value_source = source[equals + 1..].trim();
    let wrapped = format!("value = {value_source}");
    let value = toml::from_str::<toml::Value>(&wrapped)
        .ok()
        .and_then(|value| value.get("value").cloned())
        .ok_or_else(|| {
            invalid_fact(
                CargoFactReason::MalformedValue,
                manifest_path,
                CargoFactKind::Manifest,
                None,
            )
        })?;
    Ok(Entry {
        key_path: parse_key_path(key, manifest_path)?,
        value,
        span,
    })
}

fn find_unquoted_equals(value: &str) -> Option<usize> {
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if quote == Some('"') && character == '\\' {
            escaped = true;
            continue;
        }
        if matches!(character, '\'' | '"') {
            if quote == Some(character) {
                quote = None;
            } else if quote.is_none() {
                quote = Some(character);
            }
            continue;
        }
        if character == '=' && quote.is_none() {
            return Some(index);
        }
    }
    None
}

fn parse_key_path(value: &str, manifest_path: &str) -> Result<Vec<String>, CargoManifestFactError> {
    let mut segments = Vec::new();
    let mut start = 0_usize;
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if quote == Some('"') && character == '\\' {
            escaped = true;
            continue;
        }
        if matches!(character, '\'' | '"') {
            if quote == Some(character) {
                quote = None;
            } else if quote.is_none() {
                quote = Some(character);
            }
        } else if character == '.' && quote.is_none() {
            segments.push(parse_key_segment(&value[start..index], manifest_path)?);
            start = index + 1;
        }
    }
    if quote.is_some() {
        return Err(invalid_fact(
            CargoFactReason::MalformedValue,
            manifest_path,
            CargoFactKind::Manifest,
            None,
        ));
    }
    segments.push(parse_key_segment(&value[start..], manifest_path)?);
    Ok(segments)
}

fn parse_key_segment(segment: &str, manifest_path: &str) -> Result<String, CargoManifestFactError> {
    let segment = segment.trim();
    let wrapped = format!("{segment} = 0");
    let parsed = toml::from_str::<toml::Value>(&wrapped).map_err(|_| {
        invalid_fact(
            CargoFactReason::MalformedValue,
            manifest_path,
            CargoFactKind::Manifest,
            None,
        )
    })?;
    parsed
        .as_table()
        .and_then(|table| table.keys().next())
        .cloned()
        .ok_or_else(|| {
            invalid_fact(
                CargoFactReason::MalformedValue,
                manifest_path,
                CargoFactKind::Manifest,
                None,
            )
        })
}

fn dependency_section(
    path: &[String],
) -> Option<(
    DependencyScope,
    DependencyKind,
    Option<String>,
    Option<String>,
)> {
    match path {
        [first] if first == "dependencies" => {
            Some((DependencyScope::Package, DependencyKind::Normal, None, None))
        }
        [first] if first == "dev-dependencies" => Some((
            DependencyScope::Package,
            DependencyKind::Development,
            None,
            None,
        )),
        [first] if first == "build-dependencies" => {
            Some((DependencyScope::Package, DependencyKind::Build, None, None))
        }
        [first, name] if first == "dependencies" => Some((
            DependencyScope::Package,
            DependencyKind::Normal,
            None,
            Some(name.clone()),
        )),
        [first, name] if first == "dev-dependencies" => Some((
            DependencyScope::Package,
            DependencyKind::Development,
            None,
            Some(name.clone()),
        )),
        [first, name] if first == "build-dependencies" => Some((
            DependencyScope::Package,
            DependencyKind::Build,
            None,
            Some(name.clone()),
        )),
        [first, second] if first == "workspace" && second == "dependencies" => Some((
            DependencyScope::Workspace,
            DependencyKind::Normal,
            None,
            None,
        )),
        [first, second, name] if first == "workspace" && second == "dependencies" => Some((
            DependencyScope::Workspace,
            DependencyKind::Normal,
            None,
            Some(name.clone()),
        )),
        [first, predicate, third] if first == "target" && third == "dependencies" => Some((
            DependencyScope::Package,
            DependencyKind::Normal,
            Some(predicate.nfc().collect()),
            None,
        )),
        [first, predicate, third] if first == "target" && third == "dev-dependencies" => Some((
            DependencyScope::Package,
            DependencyKind::Development,
            Some(predicate.nfc().collect()),
            None,
        )),
        [first, predicate, third] if first == "target" && third == "build-dependencies" => Some((
            DependencyScope::Package,
            DependencyKind::Build,
            Some(predicate.nfc().collect()),
            None,
        )),
        [first, predicate, third, name] if first == "target" && third == "dependencies" => Some((
            DependencyScope::Package,
            DependencyKind::Normal,
            Some(predicate.nfc().collect()),
            Some(name.clone()),
        )),
        [first, predicate, third, name] if first == "target" && third == "dev-dependencies" => {
            Some((
                DependencyScope::Package,
                DependencyKind::Development,
                Some(predicate.nfc().collect()),
                Some(name.clone()),
            ))
        }
        [first, predicate, third, name] if first == "target" && third == "build-dependencies" => {
            Some((
                DependencyScope::Package,
                DependencyKind::Build,
                Some(predicate.nfc().collect()),
                Some(name.clone()),
            ))
        }
        _ => None,
    }
}

fn dependency_declarations(
    section: &Section,
    standard_table_name: Option<&str>,
    manifest_path: &str,
) -> Result<Vec<DependencyDeclaration>, CargoManifestFactError> {
    if let Some(name) = standard_table_name {
        let mut table = toml::Table::new();
        for entry in &section.entries {
            if entry.key_path.len() != 1 {
                return Err(invalid_fact(
                    CargoFactReason::UnsupportedKey,
                    manifest_path,
                    CargoFactKind::Dependency,
                    None,
                ));
            }
            let field = entry.key_path[0].clone();
            if table.insert(field.clone(), entry.value.clone()).is_some() {
                return Err(conflict(manifest_path, CargoFactKind::Dependency, &field));
            }
        }
        let end = section
            .entries
            .iter()
            .map(|entry| entry.span.end)
            .max()
            .unwrap_or(section.header.end);
        return Ok(vec![DependencyDeclaration {
            name: name.to_owned(),
            value: toml::Value::Table(table),
            span: ByteRange {
                start: section.header.start,
                end,
            },
        }]);
    }

    section
        .entries
        .iter()
        .map(|entry| {
            if entry.key_path.len() != 1 {
                return Err(invalid_fact(
                    CargoFactReason::InvalidDeclarationName,
                    manifest_path,
                    CargoFactKind::Dependency,
                    None,
                ));
            }
            Ok(DependencyDeclaration {
                name: entry.key_path[0].clone(),
                value: entry.value.clone(),
                span: entry.span,
            })
        })
        .collect()
}

fn declared_string_field(
    table: &toml::Table,
    field: &'static str,
    evidence_id: &str,
    manifest_path: &str,
) -> Result<Option<DeclaredString>, CargoManifestFactError> {
    table
        .get(field)
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| {
                    invalid_fact(
                        CargoFactReason::MalformedValue,
                        manifest_path,
                        CargoFactKind::Dependency,
                        Some(field),
                    )
                })
                .and_then(|value| checked_string(value, CargoFactLimit::DeclarationStringBytes))
                .map(|value| DeclaredString {
                    value,
                    evidence_id: evidence_id.to_owned(),
                })
        })
        .transpose()
}

fn declared_bool_field(
    table: &toml::Table,
    field: &'static str,
    evidence_id: &str,
    manifest_path: &str,
) -> Result<Option<DeclaredBoolean>, CargoManifestFactError> {
    table
        .get(field)
        .map(|value| {
            value
                .as_bool()
                .map(|value| DeclaredBoolean {
                    value,
                    evidence_id: evidence_id.to_owned(),
                })
                .ok_or_else(|| {
                    invalid_fact(
                        CargoFactReason::MalformedValue,
                        manifest_path,
                        CargoFactKind::Dependency,
                        Some(field),
                    )
                })
        })
        .transpose()
}

fn parse_feature_member(
    lexeme: String,
    evidence_id: &str,
    manifest_path: &str,
) -> Result<FeatureMember, CargoManifestFactError> {
    let (syntax, dependency_name, feature_name) = if let Some(name) = lexeme.strip_prefix("dep:") {
        if name.is_empty() || name.contains('/') {
            return Err(invalid_fact(
                CargoFactReason::InvalidFeatureMember,
                manifest_path,
                CargoFactKind::Feature,
                Some("members"),
            ));
        }
        (
            FeatureMemberSyntax::ExplicitDependency,
            Some(name.to_owned()),
            None,
        )
    } else if let Some((dependency, feature)) = lexeme.split_once("?/") {
        if dependency.is_empty() || feature.is_empty() || feature.contains('/') {
            return Err(invalid_fact(
                CargoFactReason::InvalidFeatureMember,
                manifest_path,
                CargoFactKind::Feature,
                Some("members"),
            ));
        }
        (
            FeatureMemberSyntax::WeakDependencyFeature,
            Some(dependency.to_owned()),
            Some(feature.to_owned()),
        )
    } else if let Some((dependency, feature)) = lexeme.split_once('/') {
        if dependency.is_empty() || feature.is_empty() || feature.contains('/') {
            return Err(invalid_fact(
                CargoFactReason::InvalidFeatureMember,
                manifest_path,
                CargoFactKind::Feature,
                Some("members"),
            ));
        }
        (
            FeatureMemberSyntax::DependencyFeature,
            Some(dependency.to_owned()),
            Some(feature.to_owned()),
        )
    } else if !lexeme.is_empty() {
        (FeatureMemberSyntax::Bare, None, Some(lexeme.clone()))
    } else {
        return Err(invalid_fact(
            CargoFactReason::InvalidFeatureMember,
            manifest_path,
            CargoFactKind::Feature,
            Some("members"),
        ));
    };
    Ok(FeatureMember {
        lexeme,
        syntax,
        dependency_name,
        feature_name,
        evidence_id: evidence_id.to_owned(),
    })
}

fn required_name(
    entry: &Entry,
    manifest_path: &str,
    fact_kind: CargoFactKind,
    field: &'static str,
) -> Result<String, CargoManifestFactError> {
    let value = required_string(entry, manifest_path, fact_kind, field)?;
    normalized_name(&value, manifest_path, fact_kind)
}

fn required_string(
    entry: &Entry,
    manifest_path: &str,
    fact_kind: CargoFactKind,
    field: &'static str,
) -> Result<String, CargoManifestFactError> {
    entry
        .value
        .as_str()
        .ok_or_else(|| {
            invalid_fact(
                CargoFactReason::MalformedValue,
                manifest_path,
                fact_kind,
                Some(field),
            )
        })
        .and_then(|value| checked_string(value, CargoFactLimit::DeclarationStringBytes))
}

fn package_name_from_id_input(
    section: &Section,
    manifest_path: &str,
) -> Result<String, CargoManifestFactError> {
    required_name(
        section.single_entry(&["name"], manifest_path, CargoFactKind::Package)?,
        manifest_path,
        CargoFactKind::Package,
        "name",
    )
}

fn string_array(
    value: &toml::Value,
    manifest_path: &str,
    fact_kind: CargoFactKind,
    field: Option<&'static str>,
) -> Result<Vec<String>, CargoManifestFactError> {
    let array = value.as_array().ok_or_else(|| {
        invalid_fact(
            CargoFactReason::MalformedValue,
            manifest_path,
            fact_kind,
            field,
        )
    })?;
    let mut values = array
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    invalid_fact(
                        CargoFactReason::MalformedValue,
                        manifest_path,
                        fact_kind,
                        field,
                    )
                })
                .and_then(|value| checked_string(value, CargoFactLimit::DeclarationStringBytes))
        })
        .collect::<Result<Vec<_>, _>>()?;
    values.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    values.dedup();
    Ok(values)
}

fn normalized_name(
    value: &str,
    manifest_path: &str,
    fact_kind: CargoFactKind,
) -> Result<String, CargoManifestFactError> {
    let normalized = value.nfc().collect::<String>();
    if normalized.is_empty() || normalized.len() > 255 || normalized.chars().any(char::is_control) {
        return Err(invalid_fact(
            CargoFactReason::InvalidDeclarationName,
            manifest_path,
            fact_kind,
            None,
        ));
    }
    Ok(normalized)
}

fn checked_string(value: &str, limit: CargoFactLimit) -> Result<String, CargoManifestFactError> {
    let observed = u64::try_from(value.len()).unwrap_or(u64::MAX);
    if observed > limit.maximum() {
        return Err(cargo_fact_limit_exceeded(limit, observed));
    }
    Ok(value.nfc().collect())
}

fn check_locator(value: &str) -> Result<(), CargoManifestFactError> {
    let observed = u64::try_from(value.len()).unwrap_or(u64::MAX);
    if observed > CargoFactLimit::ExternalLocatorBytes.maximum() {
        return Err(cargo_fact_limit_exceeded(
            CargoFactLimit::ExternalLocatorBytes,
            observed,
        ));
    }
    Ok(())
}

fn check_limit(limit: CargoFactLimit, observed: usize) -> Result<(), CargoManifestFactError> {
    let observed = u64::try_from(observed).unwrap_or(u64::MAX);
    if observed > limit.maximum() {
        Err(cargo_fact_limit_exceeded(limit, observed))
    } else {
        Ok(())
    }
}

fn normalize_relative_path(
    base: &str,
    declared: &str,
    manifest_path: &str,
    fact_kind: CargoFactKind,
) -> Result<String, CargoManifestFactError> {
    if declared.is_empty()
        || declared.starts_with('/')
        || declared.contains('\\')
        || declared.chars().any(char::is_control)
    {
        return Err(invalid_fact(
            CargoFactReason::InvalidRelativePath,
            manifest_path,
            fact_kind,
            Some("path"),
        ));
    }
    let normalized_declared = declared.nfc().collect::<String>();
    let mut components = base
        .split('/')
        .filter(|component| !component.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    for component in normalized_declared.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                if components.pop().is_none() {
                    return Err(invalid_fact(
                        CargoFactReason::InvalidRelativePath,
                        manifest_path,
                        fact_kind,
                        Some("path"),
                    ));
                }
            }
            value
                if value.len()
                    <= usize::try_from(STANDARD_LOCAL_S1_LIMITS.path_component_bytes)
                        .unwrap_or(usize::MAX) =>
            {
                components.push(value.to_owned());
            }
            _ => {
                return Err(invalid_fact(
                    CargoFactReason::InvalidRelativePath,
                    manifest_path,
                    fact_kind,
                    Some("path"),
                ));
            }
        }
    }
    let normalized = components.join("/");
    if normalized.is_empty()
        || normalized.len()
            > usize::try_from(STANDARD_LOCAL_S1_LIMITS.path_bytes).unwrap_or(usize::MAX)
        || components.len()
            > usize::try_from(STANDARD_LOCAL_S1_LIMITS.recursion_depth).unwrap_or(usize::MAX)
    {
        return Err(invalid_fact(
            CargoFactReason::InvalidRelativePath,
            manifest_path,
            fact_kind,
            Some("path"),
        ));
    }
    Ok(normalized)
}

fn parent_path(path: &str) -> Option<String> {
    path.rsplit_once('/').map(|(parent, _)| parent.to_owned())
}

fn join_path(base: &str, relative: &str) -> String {
    if base.is_empty() {
        relative.to_owned()
    } else {
        format!("{base}/{relative}")
    }
}

fn conventional_target_path(kind: CargoTargetKind, name: &str) -> String {
    match kind {
        CargoTargetKind::Library => "src/lib.rs".to_owned(),
        CargoTargetKind::Binary => "src/main.rs".to_owned(),
        CargoTargetKind::Example => format!("examples/{name}.rs"),
        CargoTargetKind::Test => format!("tests/{name}.rs"),
        CargoTargetKind::Bench => format!("benches/{name}.rs"),
    }
}

fn path_stem(path: &str) -> Option<String> {
    path.rsplit('/')
        .next()
        .and_then(|component| component.strip_suffix(".rs"))
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn invalid_fact(
    reason: CargoFactReason,
    path: &str,
    fact_kind: CargoFactKind,
    field: Option<&'static str>,
) -> CargoManifestFactError {
    CargoManifestFactError::InvalidFact {
        reason,
        path: path.to_owned(),
        fact_kind,
        field,
    }
}

fn conflict(
    path: &str,
    fact_kind: CargoFactKind,
    declaration_name: &str,
) -> CargoManifestFactError {
    CargoManifestFactError::Conflict {
        path: path.to_owned(),
        fact_kind,
        declaration_name_sha256: sha256_hex(declaration_name.as_bytes()),
    }
}

fn static_field(field: &str) -> Option<&'static str> {
    match field {
        "artifact" => Some("artifact"),
        "authors" => Some("authors"),
        "autobenches" => Some("autobenches"),
        "autobins" => Some("autobins"),
        "autoexamples" => Some("autoexamples"),
        "autolib" => Some("autolib"),
        "autotests" => Some("autotests"),
        "branch" => Some("branch"),
        "build" => Some("build"),
        "categories" => Some("categories"),
        "default-features" => Some("default-features"),
        "default-run" => Some("default-run"),
        "description" => Some("description"),
        "documentation" => Some("documentation"),
        "edition" => Some("edition"),
        "exclude" => Some("exclude"),
        "features" => Some("features"),
        "git" => Some("git"),
        "homepage" => Some("homepage"),
        "include" => Some("include"),
        "keywords" => Some("keywords"),
        "lib" => Some("lib"),
        "license" => Some("license"),
        "license-file" => Some("license-file"),
        "links" => Some("links"),
        "name" => Some("name"),
        "optional" => Some("optional"),
        "package" => Some("package"),
        "path" => Some("path"),
        "public" => Some("public"),
        "publish" => Some("publish"),
        "readme" => Some("readme"),
        "registry" => Some("registry"),
        "repository" => Some("repository"),
        "rev" => Some("rev"),
        "rust-version" => Some("rust-version"),
        "tag" => Some("tag"),
        "target" => Some("target"),
        "version" => Some("version"),
        "workspace" => Some("workspace"),
        _ => None,
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut state = [
        0x6a09_e667_u32,
        0xbb67_ae85,
        0x3c6e_f372,
        0xa54f_f53a,
        0x510e_527f,
        0x9b05_688c,
        0x1f83_d9ab,
        0x5be0_cd19,
    ];
    let bit_length = u64::try_from(bytes.len())
        .unwrap_or(u64::MAX)
        .saturating_mul(8);
    let mut padded = bytes.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_length.to_be_bytes());
    for block in padded.chunks_exact(64) {
        sha256_compress(&mut state, block);
    }
    let mut digest = String::with_capacity(64);
    for word in state {
        for byte in word.to_be_bytes() {
            digest.push(char::from(HEX[usize::from(byte >> 4)]));
            digest.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
    }
    digest
}

#[allow(clippy::too_many_lines)]
fn sha256_compress(state: &mut [u32; 8], block: &[u8]) {
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
    let mut words = [0_u32; 64];
    for (index, chunk) in block.chunks_exact(4).enumerate() {
        words[index] = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
    }
    for index in 16..64 {
        let sigma0 = words[index - 15].rotate_right(7)
            ^ words[index - 15].rotate_right(18)
            ^ (words[index - 15] >> 3);
        let sigma1 = words[index - 2].rotate_right(17)
            ^ words[index - 2].rotate_right(19)
            ^ (words[index - 2] >> 10);
        words[index] = words[index - 16]
            .wrapping_add(sigma0)
            .wrapping_add(words[index - 7])
            .wrapping_add(sigma1);
    }
    let mut working = *state;
    for index in 0..64 {
        let choice = (working[4] & working[5]) ^ (!working[4] & working[6]);
        let majority =
            (working[0] & working[1]) ^ (working[0] & working[2]) ^ (working[1] & working[2]);
        let sum0 =
            working[0].rotate_right(2) ^ working[0].rotate_right(13) ^ working[0].rotate_right(22);
        let sum1 =
            working[4].rotate_right(6) ^ working[4].rotate_right(11) ^ working[4].rotate_right(25);
        let temporary1 = working[7]
            .wrapping_add(sum1)
            .wrapping_add(choice)
            .wrapping_add(ROUND[index])
            .wrapping_add(words[index]);
        let temporary2 = sum0.wrapping_add(majority);
        working.copy_within(0..7, 1);
        working[4] = working[4].wrapping_add(temporary1);
        working[0] = temporary1.wrapping_add(temporary2);
    }
    for (value, addition) in state.iter_mut().zip(working) {
        *value = value.wrapping_add(addition);
    }
}
