use std::collections::{BTreeMap, BTreeSet};

use codenoesis_domain::s4_r3::{
    ExternalWorkspaceBoundary, MAX_R3_BINARY_ROOTS_PER_PACKAGE, MAX_R3_CRATE_TARGETS,
    MAX_R3_EXCLUSIONS, MAX_R3_LITERAL_MEMBERS, MAX_R3_PACKAGE_MANIFESTS, MAX_R3_PROJECTED_MEMBERS,
    RootPackageMember, RootPackageShape, RootPackageTarget, RootPackageWorkspaceError,
    RootPackageWorkspacePlan, WorkspaceManifestReason, WorkspaceMemberSource, WorkspaceTargetKind,
    root_package_limit_exceeded,
};
use codenoesis_domain::{
    ContentKind, InventoryFile, RepositoryInventory, STANDARD_LOCAL_S1_LIMITS,
};
use unicode_normalization::UnicodeNormalization as _;

pub(super) struct PlannedRootPackageWorkspace {
    pub plan: RootPackageWorkspacePlan,
    pub coverage: Vec<PlannedCoverage>,
}

pub(super) struct PlannedCoverage {
    pub source_path: String,
    pub manifest_path: String,
    pub capability: &'static str,
}

struct PackageDraft {
    manifest_path: String,
    targets: Vec<RootPackageTarget>,
    coverage: BTreeSet<&'static str>,
}

#[derive(Clone)]
struct TargetDraft {
    kind: WorkspaceTargetKind,
    name: String,
    path: String,
}

enum WorkspaceMemberDeclaration {
    Literal(String),
    OneLevelPattern { declaration: String, prefix: String },
}

impl WorkspaceMemberDeclaration {
    fn declaration(&self) -> &str {
        match self {
            Self::Literal(path) => path,
            Self::OneLevelPattern { declaration, .. } => declaration,
        }
    }
}

pub(super) fn plan_root_package_workspace(
    inventory: &RepositoryInventory,
    external_boundaries: &[ExternalWorkspaceBoundary],
) -> Result<PlannedRootPackageWorkspace, RootPackageWorkspaceError> {
    Planner::new(inventory, external_boundaries, false)?.plan()
}

pub(super) fn plan_root_package_workspace_r4(
    inventory: &RepositoryInventory,
    external_boundaries: &[ExternalWorkspaceBoundary],
) -> Result<PlannedRootPackageWorkspace, RootPackageWorkspaceError> {
    Planner::new(inventory, external_boundaries, true)?.plan()
}

struct Planner<'a> {
    inventory: &'a RepositoryInventory,
    files: BTreeMap<&'a str, &'a InventoryFile>,
    external_boundaries: BTreeMap<&'a str, &'a str>,
    manifest_facts: bool,
}

impl<'a> Planner<'a> {
    fn new(
        inventory: &'a RepositoryInventory,
        external_boundaries: &'a [ExternalWorkspaceBoundary],
        manifest_facts: bool,
    ) -> Result<Self, RootPackageWorkspaceError> {
        let files = inventory
            .files()
            .iter()
            .map(|file| (file.path(), file))
            .collect::<BTreeMap<_, _>>();
        let external_boundary_count = external_boundaries.len();
        let external_boundaries = external_boundaries
            .iter()
            .map(|boundary| (boundary.path.as_str(), boundary.boundary_id.as_str()))
            .collect::<BTreeMap<_, _>>();
        if external_boundaries.len() != external_boundary_count
            || external_boundaries
                .keys()
                .any(|path| !valid_canonical_path(path))
        {
            return Err(RootPackageWorkspaceError::ContractInvalid);
        }
        Ok(Self {
            inventory,
            files,
            external_boundaries,
            manifest_facts,
        })
    }

    #[allow(clippy::too_many_lines)]
    fn plan(self) -> Result<PlannedRootPackageWorkspace, RootPackageWorkspaceError> {
        let root_file =
            self.files.get("Cargo.toml").copied().ok_or_else(|| {
                invalid_manifest(WorkspaceManifestReason::MissingRootManifest, None)
            })?;
        let root_value = parse_manifest(root_file, "Cargo.toml")?;
        let root_table = root_value
            .as_table()
            .ok_or_else(|| invalid_manifest(WorkspaceManifestReason::UnsupportedRootShape, None))?;
        validate_top_level(root_table, "Cargo.toml")?;
        let package = optional_table(root_table, "package", "Cargo.toml")?;
        let workspace = optional_table(root_table, "workspace", "Cargo.toml")?;
        let root_workspace_inheritance = workspace.is_some_and(|workspace| {
            ["dependencies", "package", "lints"]
                .iter()
                .any(|key| workspace.contains_key(*key))
        });
        let root_shape = match (package.is_some(), workspace.is_some()) {
            (true, false) => RootPackageShape::StandaloneRootPackage,
            (false, true) => RootPackageShape::VirtualWorkspace,
            (true, true) => RootPackageShape::NonVirtualWorkspace,
            (false, false) => {
                return Err(invalid_manifest(
                    WorkspaceManifestReason::UnsupportedRootShape,
                    None,
                ));
            }
        };

        let (member_declarations, excluded_paths, explicit_root) =
            if let Some(workspace) = workspace {
                parse_workspace(workspace)?
            } else {
                (Vec::new(), Vec::new(), false)
            };
        if root_shape == RootPackageShape::VirtualWorkspace && explicit_root {
            return Err(invalid_manifest(
                WorkspaceManifestReason::InvalidMemberPath,
                Some("Cargo.toml".to_owned()),
            ));
        }
        let excluded = excluded_paths
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let mut planned_members = self.expand_workspace_members(&member_declarations, &excluded)?;
        if package.is_some() {
            planned_members.push((
                ".".to_owned(),
                if explicit_root {
                    WorkspaceMemberSource::ExplicitRootMember
                } else {
                    WorkspaceMemberSource::ImplicitRootPackage
                },
            ));
        }
        planned_members.sort_by(|left, right| left.0.as_bytes().cmp(right.0.as_bytes()));
        if u64::try_from(planned_members.len()).unwrap_or(u64::MAX) > MAX_R3_PROJECTED_MEMBERS {
            return Err(root_package_limit_exceeded(
                codenoesis_domain::s4_r3::RootPackageLimit::WorkspaceMembers,
                u64::try_from(planned_members.len()).unwrap_or(u64::MAX),
            ));
        }
        if planned_members.is_empty() {
            return Err(invalid_manifest(
                WorkspaceManifestReason::UnsupportedRootShape,
                None,
            ));
        }

        let mut packages = Vec::new();
        let mut external_members = Vec::new();
        for (member_path, member_source) in &planned_members {
            if let Some(boundary_id) = self.external_boundaries.get(member_path.as_str()) {
                if member_path == "." {
                    return Err(RootPackageWorkspaceError::ContractInvalid);
                }
                external_members.push((member_path.clone(), (*boundary_id).to_owned()));
                continue;
            }
            let manifest_path = manifest_path(member_path);
            let file = self
                .files
                .get(manifest_path.as_str())
                .copied()
                .ok_or_else(|| {
                    invalid_manifest(
                        WorkspaceManifestReason::MissingMemberManifest,
                        Some(manifest_path.clone()),
                    )
                })?;
            let value = if member_path == "." {
                root_value.clone()
            } else {
                parse_manifest(file, &manifest_path)?
            };
            let table = value.as_table().ok_or_else(|| {
                invalid_manifest(
                    WorkspaceManifestReason::InvalidPackageManifest,
                    Some(manifest_path.clone()),
                )
            })?;
            validate_top_level(table, &manifest_path)?;
            if member_path != "." && table.contains_key("workspace") {
                return Err(invalid_manifest(
                    WorkspaceManifestReason::UnsupportedStructuralKey,
                    Some(manifest_path),
                ));
            }
            packages.push(self.parse_package(
                member_path,
                *member_source,
                manifest_path,
                table,
                workspace,
            )?);
        }
        if u64::try_from(packages.len()).unwrap_or(u64::MAX) > MAX_R3_PACKAGE_MANIFESTS {
            return Err(root_package_limit_exceeded(
                codenoesis_domain::s4_r3::RootPackageLimit::PackageManifests,
                u64::try_from(packages.len()).unwrap_or(u64::MAX),
            ));
        }

        let mut targets = packages
            .iter()
            .flat_map(|package| package.targets.iter().cloned())
            .collect::<Vec<_>>();
        targets.sort_by(target_order);
        if u64::try_from(targets.len()).unwrap_or(u64::MAX) > MAX_R3_CRATE_TARGETS {
            return Err(root_package_limit_exceeded(
                codenoesis_domain::s4_r3::RootPackageLimit::WorkspaceCrates,
                u64::try_from(targets.len()).unwrap_or(u64::MAX),
            ));
        }
        if targets
            .windows(2)
            .any(|pair| pair[0].crate_id == pair[1].crate_id)
        {
            return Err(RootPackageWorkspaceError::ContractInvalid);
        }

        let mut members = Vec::with_capacity(planned_members.len());
        for (path, member_source) in planned_members {
            if let Some((_, boundary_id)) = external_members
                .iter()
                .find(|(external_path, _)| external_path == &path)
            {
                members.push(RootPackageMember {
                    path,
                    manifest_path: None,
                    member_source,
                    crate_ids: Vec::new(),
                    external_boundary_id: Some(boundary_id.clone()),
                });
                continue;
            }
            let manifest_path = manifest_path(&path);
            let mut crate_ids = targets
                .iter()
                .filter(|target| target.member_path == path)
                .map(|target| target.crate_id.clone())
                .collect::<Vec<_>>();
            crate_ids.sort();
            members.push(RootPackageMember {
                path,
                manifest_path: Some(manifest_path),
                member_source,
                crate_ids,
                external_boundary_id: None,
            });
        }

        let first_source = targets
            .first()
            .map(|target| target.source_path.clone())
            .ok_or(RootPackageWorkspaceError::ContractInvalid)?;
        let mut coverage = Vec::new();
        let mut coverage_keys = BTreeSet::new();
        for package in &packages {
            let source_path = package
                .targets
                .first()
                .map_or_else(|| first_source.clone(), |target| target.source_path.clone());
            for capability in &package.coverage {
                if coverage_keys.insert((package.manifest_path.clone(), *capability)) {
                    coverage.push(PlannedCoverage {
                        source_path: source_path.clone(),
                        manifest_path: package.manifest_path.clone(),
                        capability,
                    });
                }
            }
        }
        if root_workspace_inheritance
            && coverage_keys.insert((
                "Cargo.toml".to_owned(),
                "cargo.workspace_inheritance_deferred",
            ))
        {
            coverage.push(PlannedCoverage {
                source_path: first_source.clone(),
                manifest_path: "Cargo.toml".to_owned(),
                capability: "cargo.workspace_inheritance_deferred",
            });
        }
        if !external_members.is_empty() {
            coverage.push(PlannedCoverage {
                source_path: first_source,
                manifest_path: "Cargo.toml".to_owned(),
                capability: "workspace.external_gitlink_member_not_analyzed",
            });
        }
        coverage.sort_by(|left, right| {
            (left.capability, left.manifest_path.as_bytes())
                .cmp(&(right.capability, right.manifest_path.as_bytes()))
        });

        Ok(PlannedRootPackageWorkspace {
            plan: RootPackageWorkspacePlan {
                root_shape,
                members,
                excluded_paths,
                targets,
            },
            coverage,
        })
    }

    fn parse_package(
        &self,
        member_path: &str,
        member_source: WorkspaceMemberSource,
        manifest_path: String,
        table: &toml::Table,
        workspace: Option<&toml::Table>,
    ) -> Result<PackageDraft, RootPackageWorkspaceError> {
        let package = optional_table(table, "package", &manifest_path)?.ok_or_else(|| {
            invalid_manifest(
                WorkspaceManifestReason::InvalidPackageManifest,
                Some(manifest_path.clone()),
            )
        })?;
        if !self.manifest_facts {
            for automatic in [
                "autolib",
                "autobins",
                "autoexamples",
                "autotests",
                "autobenches",
            ] {
                if package.contains_key(automatic) {
                    return Err(invalid_manifest(
                        WorkspaceManifestReason::UnsupportedStructuralKey,
                        Some(manifest_path),
                    ));
                }
            }
        }
        let package_name = required_nonempty_string(package, "name", &manifest_path)?;
        if self.manifest_facts {
            validate_r4_edition(package, &manifest_path)?;
        } else {
            validate_r3_edition(package, workspace, &manifest_path)?;
        }
        if package_name.len() > 255 {
            return Err(invalid_manifest(
                WorkspaceManifestReason::InvalidPackageManifest,
                Some(manifest_path),
            ));
        }

        let mut coverage = BTreeSet::new();
        if package
            .keys()
            .any(|key| !matches!(key.as_str(), "name" | "edition"))
        {
            coverage.insert("cargo.package_metadata_deferred");
        }
        if package.get("build").is_some()
            || self
                .files
                .contains_key(join_package_path(member_path, "build.rs").as_str())
        {
            coverage.insert("cargo.build_script_not_executed");
        }
        validate_deferred_tables(table, &manifest_path, &mut coverage)?;

        let targets = self.package_targets(
            member_path,
            member_source,
            &manifest_path,
            package_name,
            table,
            &mut coverage,
        )?;
        Ok(PackageDraft {
            manifest_path,
            targets,
            coverage,
        })
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_lines)]
    fn package_targets(
        &self,
        member_path: &str,
        member_source: WorkspaceMemberSource,
        manifest_path: &str,
        package_name: &str,
        table: &toml::Table,
        coverage: &mut BTreeSet<&'static str>,
    ) -> Result<Vec<RootPackageTarget>, RootPackageWorkspaceError> {
        let mut conventional = self.conventional_targets(member_path, package_name)?;
        let mut selected = BTreeMap::<(WorkspaceTargetKind, String), TargetDraft>::new();

        if let Some(library) = table.get("lib") {
            conventional.retain(|target| target.kind != WorkspaceTargetKind::Library);
            let library = library.as_table().ok_or_else(|| {
                invalid_manifest(
                    WorkspaceManifestReason::InvalidPackageManifest,
                    Some(manifest_path.to_owned()),
                )
            })?;
            if library.contains_key("proc-macro") {
                let proc_macro = library
                    .get("proc-macro")
                    .and_then(toml::Value::as_bool)
                    .ok_or_else(|| {
                        invalid_manifest(
                            WorkspaceManifestReason::InvalidPackageManifest,
                            Some(manifest_path.to_owned()),
                        )
                    })?;
                if proc_macro {
                    coverage.insert("cargo.proc_macro_not_executed");
                }
            }
            if library
                .keys()
                .any(|key| !matches!(key.as_str(), "name" | "path"))
            {
                coverage.insert("cargo.target_world_deferred");
            }
            let name = optional_nonempty_string(library, "name", manifest_path)?
                .unwrap_or_else(|| package_name.replace('-', "_"));
            let relative = optional_nonempty_string(library, "path", manifest_path)?
                .unwrap_or_else(|| "src/lib.rs".to_owned());
            selected.insert(
                (WorkspaceTargetKind::Library, name.clone()),
                TargetDraft {
                    kind: WorkspaceTargetKind::Library,
                    name,
                    path: checked_target_path(member_path, &relative, manifest_path)?,
                },
            );
        }

        if let Some(binaries) = table.get("bin") {
            let binaries = binaries.as_array().ok_or_else(|| {
                invalid_manifest(
                    WorkspaceManifestReason::InvalidPackageManifest,
                    Some(manifest_path.to_owned()),
                )
            })?;
            if u64::try_from(binaries.len()).unwrap_or(u64::MAX) > MAX_R3_BINARY_ROOTS_PER_PACKAGE {
                return Err(root_package_limit_exceeded(
                    codenoesis_domain::s4_r3::RootPackageLimit::BinaryRootsPerPackage,
                    u64::try_from(binaries.len()).unwrap_or(u64::MAX),
                ));
            }
            let mut explicit_binaries = Vec::with_capacity(binaries.len());
            for binary in binaries {
                let binary = binary.as_table().ok_or_else(|| {
                    invalid_manifest(
                        WorkspaceManifestReason::InvalidPackageManifest,
                        Some(manifest_path.to_owned()),
                    )
                })?;
                if let Some(required_features) = binary.get("required-features") {
                    let values = required_features.as_array().ok_or_else(|| {
                        invalid_manifest(
                            WorkspaceManifestReason::InvalidPackageManifest,
                            Some(manifest_path.to_owned()),
                        )
                    })?;
                    if values
                        .iter()
                        .any(|value| value.as_str().is_none_or(str::is_empty))
                    {
                        return Err(invalid_manifest(
                            WorkspaceManifestReason::InvalidPackageManifest,
                            Some(manifest_path.to_owned()),
                        ));
                    }
                    coverage.insert("cargo.required_features_deferred");
                }
                if binary
                    .keys()
                    .any(|key| !matches!(key.as_str(), "name" | "path"))
                {
                    coverage.insert("cargo.target_world_deferred");
                }
                let declared_name = optional_nonempty_string(binary, "name", manifest_path)?;
                let declared_path = optional_nonempty_string(binary, "path", manifest_path)?;
                let (name, path) = resolve_explicit_binary(
                    member_path,
                    package_name,
                    declared_name,
                    declared_path,
                    conventional.iter(),
                    manifest_path,
                )?;
                explicit_binaries.push(TargetDraft {
                    kind: WorkspaceTargetKind::Binary,
                    name,
                    path,
                });
            }
            explicit_binaries.sort_by(|left, right| {
                (left.name.as_bytes(), left.path.as_bytes())
                    .cmp(&(right.name.as_bytes(), right.path.as_bytes()))
            });
            for binary in explicit_binaries {
                conventional.retain(|target| {
                    target.kind != WorkspaceTargetKind::Binary
                        || target.name != binary.name && target.path != binary.path
                });
                insert_target(&mut selected, binary)?;
            }
        }

        for target in conventional {
            insert_target(&mut selected, target)?;
        }

        let binary_count = selected
            .values()
            .filter(|target| target.kind == WorkspaceTargetKind::Binary)
            .count();
        if u64::try_from(binary_count).unwrap_or(u64::MAX) > MAX_R3_BINARY_ROOTS_PER_PACKAGE {
            return Err(root_package_limit_exceeded(
                codenoesis_domain::s4_r3::RootPackageLimit::BinaryRootsPerPackage,
                u64::try_from(binary_count).unwrap_or(u64::MAX),
            ));
        }
        let has_deferred_targets = ["example", "test", "bench"]
            .iter()
            .any(|key| table.contains_key(*key));
        if selected.is_empty() && has_deferred_targets {
            coverage.insert("cargo.target_world_deferred");
            return Ok(Vec::new());
        }
        if selected.is_empty() {
            return Err(invalid_manifest(
                WorkspaceManifestReason::MissingTargetRoot,
                Some(manifest_path.to_owned()),
            ));
        }
        let mut source_paths = BTreeMap::<String, (WorkspaceTargetKind, String)>::new();
        for target in selected.values() {
            if target.name.is_empty() || target.name.len() > 255 {
                return Err(invalid_manifest(
                    WorkspaceManifestReason::InvalidPackageManifest,
                    Some(manifest_path.to_owned()),
                ));
            }
            let file = self
                .files
                .get(target.path.as_str())
                .copied()
                .ok_or_else(|| {
                    invalid_manifest(
                        WorkspaceManifestReason::MissingTargetRoot,
                        Some(target.path.clone()),
                    )
                })?;
            if file.content_kind() != ContentKind::TextUtf8 || file.bytes().is_empty() {
                return Err(invalid_manifest(
                    WorkspaceManifestReason::MissingTargetRoot,
                    Some(target.path.clone()),
                ));
            }
            if let Some((kind, name)) =
                source_paths.insert(target.path.clone(), (target.kind, target.name.clone()))
            {
                return Err(RootPackageWorkspaceError::TargetConflict {
                    path: target.path.clone(),
                    target_kind: kind,
                    target_name: name,
                });
            }
        }
        let repository_identity = self
            .inventory
            .bound_revision()
            .repository_identity()
            .as_str();
        let mut targets = selected
            .into_values()
            .map(|target| {
                RootPackageTarget::new(
                    repository_identity,
                    member_path.to_owned(),
                    member_source,
                    manifest_path.to_owned(),
                    package_name.to_owned(),
                    target.kind,
                    target.name,
                    target.path,
                )
            })
            .collect::<Vec<_>>();
        targets.sort_by(target_order);
        Ok(targets)
    }

    fn expand_workspace_members(
        &self,
        declarations: &[WorkspaceMemberDeclaration],
        excluded: &BTreeSet<&str>,
    ) -> Result<Vec<(String, WorkspaceMemberSource)>, RootPackageWorkspaceError> {
        let mut members = BTreeMap::<String, WorkspaceMemberSource>::new();
        for declaration in declarations {
            match declaration {
                WorkspaceMemberDeclaration::Literal(path) if path == "." => {}
                WorkspaceMemberDeclaration::Literal(path) => {
                    if excluded.contains(path.as_str()) {
                        return Err(RootPackageWorkspaceError::MemberConflict {
                            path: path.clone(),
                        });
                    }
                    members.insert(path.clone(), WorkspaceMemberSource::LiteralMember);
                }
                WorkspaceMemberDeclaration::OneLevelPattern { prefix, .. } => {
                    let path_prefix = format!("{prefix}/");
                    let mut matched = false;
                    for manifest in self.files.keys() {
                        let Some(relative) = manifest.strip_prefix(path_prefix.as_str()) else {
                            continue;
                        };
                        let Some(child) = relative.strip_suffix("/Cargo.toml") else {
                            continue;
                        };
                        if child.is_empty() || child.contains('/') {
                            continue;
                        }
                        matched = true;
                        let path = format!("{prefix}/{child}");
                        if excluded.contains(path.as_str()) {
                            continue;
                        }
                        members
                            .entry(path)
                            .or_insert(WorkspaceMemberSource::LiteralMember);
                    }
                    if !matched {
                        return Err(invalid_manifest(
                            WorkspaceManifestReason::InvalidMemberPath,
                            Some("Cargo.toml".to_owned()),
                        ));
                    }
                }
            }
        }
        let observed = u64::try_from(members.len()).unwrap_or(u64::MAX);
        if observed > MAX_R3_LITERAL_MEMBERS {
            return Err(root_package_limit_exceeded(
                codenoesis_domain::s4_r3::RootPackageLimit::WorkspaceMembers,
                observed,
            ));
        }
        Ok(members.into_iter().collect())
    }

    fn conventional_targets(
        &self,
        member_path: &str,
        package_name: &str,
    ) -> Result<Vec<TargetDraft>, RootPackageWorkspaceError> {
        let mut targets = Vec::new();
        let lib = join_package_path(member_path, "src/lib.rs");
        if self.files.contains_key(lib.as_str()) {
            targets.push(TargetDraft {
                kind: WorkspaceTargetKind::Library,
                name: package_name.replace('-', "_"),
                path: lib,
            });
        }
        let main = join_package_path(member_path, "src/main.rs");
        if self.files.contains_key(main.as_str()) {
            targets.push(TargetDraft {
                kind: WorkspaceTargetKind::Binary,
                name: package_name.to_owned(),
                path: main,
            });
        }
        let bin_prefix = join_package_path(member_path, "src/bin/");
        let mut conventional_bins = BTreeMap::<String, String>::new();
        for path in self.files.keys().copied() {
            let Some(relative) = path.strip_prefix(&bin_prefix) else {
                continue;
            };
            let name = if let Some(directory) = relative.strip_suffix("/main.rs") {
                (!directory.is_empty() && !directory.contains('/')).then_some(directory)
            } else if let Some(file) = relative.strip_suffix(".rs") {
                (!file.is_empty() && !file.contains('/')).then_some(file)
            } else {
                None
            };
            let Some(name) = name else {
                continue;
            };
            if let Some(existing) = conventional_bins.insert(name.to_owned(), path.to_owned())
                && existing != path
            {
                return Err(invalid_manifest(
                    WorkspaceManifestReason::AmbiguousConventionalTarget,
                    Some(path.to_owned()),
                ));
            }
        }
        targets.extend(
            conventional_bins
                .into_iter()
                .map(|(name, path)| TargetDraft {
                    kind: WorkspaceTargetKind::Binary,
                    name,
                    path,
                }),
        );
        targets.sort_by(|left, right| {
            (left.kind, left.name.as_bytes(), left.path.as_bytes()).cmp(&(
                right.kind,
                right.name.as_bytes(),
                right.path.as_bytes(),
            ))
        });
        Ok(targets)
    }
}

fn parse_workspace(
    workspace: &toml::Table,
) -> Result<(Vec<WorkspaceMemberDeclaration>, Vec<String>, bool), RootPackageWorkspaceError> {
    const ALLOWED: &[&str] = &[
        "members",
        "exclude",
        "resolver",
        "dependencies",
        "package",
        "lints",
        "metadata",
        "default-members",
    ];
    if workspace.keys().any(|key| !ALLOWED.contains(&key.as_str())) {
        return Err(invalid_manifest(
            WorkspaceManifestReason::UnsupportedStructuralKey,
            Some("Cargo.toml".to_owned()),
        ));
    }
    let members = string_array(
        workspace.get("members"),
        WorkspaceManifestReason::InvalidMemberPath,
    )?;
    if u64::try_from(members.len()).unwrap_or(u64::MAX) > MAX_R3_LITERAL_MEMBERS {
        return Err(root_package_limit_exceeded(
            codenoesis_domain::s4_r3::RootPackageLimit::WorkspaceMembers,
            u64::try_from(members.len()).unwrap_or(u64::MAX),
        ));
    }
    let exclusions = string_array(
        workspace.get("exclude"),
        WorkspaceManifestReason::InvalidExclusionPath,
    )?;
    if u64::try_from(exclusions.len()).unwrap_or(u64::MAX) > MAX_R3_EXCLUSIONS {
        return Err(root_package_limit_exceeded(
            codenoesis_domain::s4_r3::RootPackageLimit::WorkspaceExclusions,
            u64::try_from(exclusions.len()).unwrap_or(u64::MAX),
        ));
    }
    let mut normalized_members = normalize_member_declarations(members)?;
    let explicit_root = normalized_members
        .iter()
        .any(|member| matches!(member, WorkspaceMemberDeclaration::Literal(path) if path == "."));
    normalized_members.sort_by(|left, right| {
        left.declaration()
            .as_bytes()
            .cmp(right.declaration().as_bytes())
    });
    if normalized_members
        .windows(2)
        .any(|pair| pair[0].declaration() == pair[1].declaration())
    {
        return Err(invalid_manifest(
            WorkspaceManifestReason::InvalidMemberPath,
            Some("Cargo.toml".to_owned()),
        ));
    }
    let mut normalized_exclusions = normalize_paths(
        exclusions,
        false,
        WorkspaceManifestReason::InvalidExclusionPath,
    )?;
    normalized_exclusions.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    if normalized_exclusions
        .windows(2)
        .any(|pair| pair[0] == pair[1])
    {
        return Err(invalid_manifest(
            WorkspaceManifestReason::InvalidExclusionPath,
            Some("Cargo.toml".to_owned()),
        ));
    }
    normalized_exclusions.dedup();
    Ok((normalized_members, normalized_exclusions, explicit_root))
}

fn normalize_member_declarations(
    paths: Vec<&str>,
) -> Result<Vec<WorkspaceMemberDeclaration>, RootPackageWorkspaceError> {
    let mut normalized = Vec::with_capacity(paths.len());
    let mut raw_by_normalized = BTreeMap::<String, &str>::new();
    for path in paths {
        let declaration = if path == "." {
            WorkspaceMemberDeclaration::Literal(".".to_owned())
        } else if valid_canonical_path(path) && !contains_glob(path) {
            WorkspaceMemberDeclaration::Literal(path.to_owned())
        } else if let Some(prefix) = one_level_member_pattern_prefix(path) {
            WorkspaceMemberDeclaration::OneLevelPattern {
                declaration: path.to_owned(),
                prefix: prefix.to_owned(),
            }
        } else {
            return Err(invalid_manifest(
                WorkspaceManifestReason::InvalidMemberPath,
                Some("Cargo.toml".to_owned()),
            ));
        };
        let nfc = path.nfc().collect::<String>();
        if let Some(previous) = raw_by_normalized.insert(nfc.clone(), path)
            && previous != path
        {
            return Err(invalid_manifest(
                WorkspaceManifestReason::UnicodeNormalizationCollision,
                Some("Cargo.toml".to_owned()),
            ));
        }
        if nfc != path {
            return Err(invalid_manifest(
                WorkspaceManifestReason::UnicodeNormalizationCollision,
                Some("Cargo.toml".to_owned()),
            ));
        }
        normalized.push(declaration);
    }
    Ok(normalized)
}

fn normalize_paths(
    paths: Vec<&str>,
    allow_root: bool,
    reason: WorkspaceManifestReason,
) -> Result<Vec<String>, RootPackageWorkspaceError> {
    let mut normalized = Vec::with_capacity(paths.len());
    let mut raw_by_normalized = BTreeMap::<String, &str>::new();
    for path in paths {
        if path == "." && allow_root {
            normalized.push(".".to_owned());
            continue;
        }
        if !valid_canonical_path(path) || contains_glob(path) {
            return Err(invalid_manifest(reason, Some("Cargo.toml".to_owned())));
        }
        let nfc = path.nfc().collect::<String>();
        if let Some(previous) = raw_by_normalized.insert(nfc.clone(), path)
            && previous != path
        {
            return Err(invalid_manifest(
                WorkspaceManifestReason::UnicodeNormalizationCollision,
                Some("Cargo.toml".to_owned()),
            ));
        }
        if nfc != path {
            return Err(invalid_manifest(
                WorkspaceManifestReason::UnicodeNormalizationCollision,
                Some("Cargo.toml".to_owned()),
            ));
        }
        normalized.push(nfc);
    }
    Ok(normalized)
}

fn validate_top_level(
    table: &toml::Table,
    manifest_path: &str,
) -> Result<(), RootPackageWorkspaceError> {
    const ALLOWED: &[&str] = &[
        "package",
        "workspace",
        "lib",
        "bin",
        "dependencies",
        "dev-dependencies",
        "build-dependencies",
        "target",
        "features",
        "patch",
        "replace",
        "example",
        "test",
        "bench",
        "profile",
        "lints",
        "badges",
    ];
    if table.keys().any(|key| !ALLOWED.contains(&key.as_str())) {
        return Err(invalid_manifest(
            WorkspaceManifestReason::UnsupportedStructuralKey,
            Some(manifest_path.to_owned()),
        ));
    }
    Ok(())
}

fn validate_deferred_tables(
    table: &toml::Table,
    manifest_path: &str,
    coverage: &mut BTreeSet<&'static str>,
) -> Result<(), RootPackageWorkspaceError> {
    if ["dependencies", "dev-dependencies", "build-dependencies"]
        .iter()
        .any(|key| table.contains_key(*key))
    {
        for key in ["dependencies", "dev-dependencies", "build-dependencies"] {
            if let Some(value) = table.get(key)
                && value.as_table().is_none()
            {
                return Err(invalid_manifest(
                    WorkspaceManifestReason::InvalidPackageManifest,
                    Some(manifest_path.to_owned()),
                ));
            }
        }
        coverage.insert("cargo.dependencies_deferred");
    }
    if let Some(value) = table.get("features") {
        if value.as_table().is_none() {
            return Err(invalid_manifest(
                WorkspaceManifestReason::InvalidPackageManifest,
                Some(manifest_path.to_owned()),
            ));
        }
        coverage.insert("cargo.features_deferred");
    }
    if ["patch", "replace"]
        .iter()
        .any(|key| table.contains_key(*key))
    {
        coverage.insert("cargo.patch_deferred");
    }
    if let Some(value) = table.get("target") {
        if value.as_table().is_none() {
            return Err(invalid_manifest(
                WorkspaceManifestReason::InvalidPackageManifest,
                Some(manifest_path.to_owned()),
            ));
        }
        coverage.insert("cargo.target_world_deferred");
        coverage.insert("cargo.dependencies_deferred");
    }
    if ["example", "test", "bench", "profile"]
        .iter()
        .any(|key| table.contains_key(*key))
    {
        coverage.insert("cargo.target_world_deferred");
    }
    if let Some(workspace) = table.get("workspace").and_then(toml::Value::as_table)
        && ["dependencies", "package", "lints"]
            .iter()
            .any(|key| workspace.contains_key(*key))
    {
        coverage.insert("cargo.workspace_inheritance_deferred");
    }
    Ok(())
}

fn parse_manifest(
    file: &InventoryFile,
    path: &str,
) -> Result<toml::Value, RootPackageWorkspaceError> {
    let byte_length = u64::try_from(file.bytes().len()).unwrap_or(u64::MAX);
    if byte_length > STANDARD_LOCAL_S1_LIMITS.single_file_bytes {
        return Err(root_package_limit_exceeded(
            codenoesis_domain::s4_r3::RootPackageLimit::SingleManifestBytes,
            byte_length,
        ));
    }
    if file.content_kind() != ContentKind::TextUtf8 || file.bytes().is_empty() {
        return Err(invalid_manifest(
            WorkspaceManifestReason::MalformedToml,
            Some(path.to_owned()),
        ));
    }
    let source = std::str::from_utf8(file.bytes()).map_err(|_| {
        invalid_manifest(
            WorkspaceManifestReason::MalformedToml,
            Some(path.to_owned()),
        )
    })?;
    toml::from_str(source).map_err(|_| {
        invalid_manifest(
            WorkspaceManifestReason::MalformedToml,
            Some(path.to_owned()),
        )
    })
}

fn optional_table<'a>(
    table: &'a toml::Table,
    key: &str,
    manifest_path: &str,
) -> Result<Option<&'a toml::Table>, RootPackageWorkspaceError> {
    table
        .get(key)
        .map(|value| {
            value.as_table().ok_or_else(|| {
                invalid_manifest(
                    WorkspaceManifestReason::InvalidPackageManifest,
                    Some(manifest_path.to_owned()),
                )
            })
        })
        .transpose()
}

fn required_nonempty_string<'a>(
    table: &'a toml::Table,
    key: &str,
    manifest_path: &str,
) -> Result<&'a str, RootPackageWorkspaceError> {
    table
        .get(key)
        .and_then(toml::Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            invalid_manifest(
                WorkspaceManifestReason::InvalidPackageManifest,
                Some(manifest_path.to_owned()),
            )
        })
}

fn validate_r4_edition(
    package: &toml::Table,
    manifest_path: &str,
) -> Result<(), RootPackageWorkspaceError> {
    let valid = package.get("edition").is_some_and(|value| {
        value.as_str().is_some_and(|edition| !edition.is_empty())
            || value.as_table().is_some_and(|table| {
                table.len() == 1
                    && table.get("workspace").and_then(toml::Value::as_bool) == Some(true)
            })
    });
    if valid {
        Ok(())
    } else {
        Err(invalid_manifest(
            WorkspaceManifestReason::InvalidPackageManifest,
            Some(manifest_path.to_owned()),
        ))
    }
}

fn validate_r3_edition(
    package: &toml::Table,
    workspace: Option<&toml::Table>,
    manifest_path: &str,
) -> Result<(), RootPackageWorkspaceError> {
    let valid = package.get("edition").is_some_and(|value| {
        value.as_str().is_some_and(|edition| !edition.is_empty())
            || value.as_table().is_some_and(|table| {
                table.len() == 1
                    && table.get("workspace").and_then(toml::Value::as_bool) == Some(true)
                    && workspace
                        .and_then(|table| table.get("package"))
                        .and_then(toml::Value::as_table)
                        .and_then(|table| table.get("edition"))
                        .and_then(toml::Value::as_str)
                        .is_some_and(|edition| !edition.is_empty())
            })
    });
    if valid {
        Ok(())
    } else {
        Err(invalid_manifest(
            WorkspaceManifestReason::InvalidPackageManifest,
            Some(manifest_path.to_owned()),
        ))
    }
}

fn optional_nonempty_string(
    table: &toml::Table,
    key: &str,
    manifest_path: &str,
) -> Result<Option<String>, RootPackageWorkspaceError> {
    table
        .get(key)
        .map(|value| {
            value
                .as_str()
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| {
                    invalid_manifest(
                        WorkspaceManifestReason::InvalidPackageManifest,
                        Some(manifest_path.to_owned()),
                    )
                })
        })
        .transpose()
}

fn string_array(
    value: Option<&toml::Value>,
    reason: WorkspaceManifestReason,
) -> Result<Vec<&str>, RootPackageWorkspaceError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    value
        .as_array()
        .ok_or_else(|| invalid_manifest(reason, Some("Cargo.toml".to_owned())))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| invalid_manifest(reason, Some("Cargo.toml".to_owned())))
        })
        .collect()
}

fn resolve_explicit_binary<'a>(
    member_path: &str,
    package_name: &str,
    declared_name: Option<String>,
    declared_path: Option<String>,
    selected: impl Iterator<Item = &'a TargetDraft>,
    manifest_path: &str,
) -> Result<(String, String), RootPackageWorkspaceError> {
    let selected = selected.collect::<Vec<_>>();
    match (declared_name, declared_path) {
        (Some(name), Some(path)) => Ok((
            name,
            checked_target_path(member_path, &path, manifest_path)?,
        )),
        (Some(name), None) => {
            let candidates = selected
                .iter()
                .filter(|target| target.kind == WorkspaceTargetKind::Binary && target.name == name)
                .map(|target| target.path.clone())
                .collect::<Vec<_>>();
            match candidates.as_slice() {
                [path] => Ok((name, path.clone())),
                [] => Err(invalid_manifest(
                    WorkspaceManifestReason::MissingTargetRoot,
                    Some(manifest_path.to_owned()),
                )),
                _ => Err(invalid_manifest(
                    WorkspaceManifestReason::AmbiguousConventionalTarget,
                    Some(manifest_path.to_owned()),
                )),
            }
        }
        (None, Some(path)) => {
            let checked = checked_target_path(member_path, &path, manifest_path)?;
            let name = default_binary_name(package_name, &path).ok_or_else(|| {
                invalid_manifest(
                    WorkspaceManifestReason::InvalidTargetPath,
                    Some(checked.clone()),
                )
            })?;
            Ok((name, checked))
        }
        (None, None) => Err(invalid_manifest(
            WorkspaceManifestReason::InvalidTargetPath,
            Some(manifest_path.to_owned()),
        )),
    }
}

fn default_binary_name(package_name: &str, relative: &str) -> Option<String> {
    if relative == "src/main.rs" {
        return Some(package_name.to_owned());
    }
    if let Some(name) = relative
        .strip_prefix("src/bin/")
        .and_then(|path| path.strip_suffix("/main.rs"))
        .filter(|path| !path.is_empty() && !path.contains('/'))
    {
        return Some(name.to_owned());
    }
    if let Some(name) = relative
        .strip_prefix("src/bin/")
        .and_then(|path| path.strip_suffix(".rs"))
        .filter(|path| !path.is_empty() && !path.contains('/'))
    {
        return Some(name.to_owned());
    }
    relative
        .rsplit('/')
        .next()
        .and_then(|name| name.strip_suffix(".rs"))
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
}

fn insert_target(
    selected: &mut BTreeMap<(WorkspaceTargetKind, String), TargetDraft>,
    target: TargetDraft,
) -> Result<(), RootPackageWorkspaceError> {
    let key = (target.kind, target.name.clone());
    if let Some(existing) = selected.get(&key) {
        if existing.path == target.path {
            return Ok(());
        }
        return Err(RootPackageWorkspaceError::TargetConflict {
            path: existing.path.clone(),
            target_kind: target.kind,
            target_name: target.name,
        });
    }
    if let Some(existing) = selected
        .values()
        .find(|existing| existing.path == target.path)
    {
        return Err(RootPackageWorkspaceError::TargetConflict {
            path: target.path,
            target_kind: existing.kind,
            target_name: existing.name.clone(),
        });
    }
    selected.insert(key, target);
    Ok(())
}

fn checked_target_path(
    member_path: &str,
    relative: &str,
    manifest_path: &str,
) -> Result<String, RootPackageWorkspaceError> {
    if !valid_canonical_path(relative)
        || contains_glob(relative)
        || relative.nfc().collect::<String>() != relative
    {
        return Err(invalid_manifest(
            WorkspaceManifestReason::InvalidTargetPath,
            Some(manifest_path.to_owned()),
        ));
    }
    let path = join_package_path(member_path, relative);
    if path.len() > usize::try_from(STANDARD_LOCAL_S1_LIMITS.path_bytes).unwrap_or(usize::MAX) {
        return Err(invalid_manifest(
            WorkspaceManifestReason::InvalidTargetPath,
            Some(manifest_path.to_owned()),
        ));
    }
    Ok(path)
}

fn manifest_path(member_path: &str) -> String {
    if member_path == "." {
        "Cargo.toml".to_owned()
    } else {
        format!("{member_path}/Cargo.toml")
    }
}

fn join_package_path(member_path: &str, relative: &str) -> String {
    if member_path == "." {
        relative.to_owned()
    } else {
        format!("{member_path}/{relative}")
    }
}

fn valid_canonical_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.ends_with('/')
        && !path.contains('\\')
        && path.len() <= usize::try_from(STANDARD_LOCAL_S1_LIMITS.path_bytes).unwrap_or(usize::MAX)
        && path.split('/').all(|component| {
            !component.is_empty()
                && !matches!(component, "." | "..")
                && component.len()
                    <= usize::try_from(STANDARD_LOCAL_S1_LIMITS.path_component_bytes)
                        .unwrap_or(usize::MAX)
                && !component.chars().any(char::is_control)
        })
}

fn contains_glob(path: &str) -> bool {
    path.contains(['*', '?', '[', ']', '{', '}'])
}

fn one_level_member_pattern_prefix(path: &str) -> Option<&str> {
    let prefix = path.strip_suffix("/*")?;
    (!prefix.is_empty()
        && path.len() <= usize::try_from(STANDARD_LOCAL_S1_LIMITS.path_bytes).unwrap_or(usize::MAX)
        && valid_canonical_path(prefix)
        && !contains_glob(prefix))
    .then_some(prefix)
}

fn target_order(left: &RootPackageTarget, right: &RootPackageTarget) -> std::cmp::Ordering {
    (
        left.manifest_path.as_bytes(),
        left.target_kind,
        left.target_name.as_bytes(),
        left.source_path.as_bytes(),
    )
        .cmp(&(
            right.manifest_path.as_bytes(),
            right.target_kind,
            right.target_name.as_bytes(),
            right.source_path.as_bytes(),
        ))
}

fn invalid_manifest(
    reason: WorkspaceManifestReason,
    path: Option<String>,
) -> RootPackageWorkspaceError {
    RootPackageWorkspaceError::InvalidManifest { reason, path }
}
