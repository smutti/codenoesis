use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::STANDARD_LOCAL_S1_LIMITS;
use crate::s4::{RustWorkspaceKnowledge, WorkspaceError, workspace_crate_id};
use crate::s5::{AnalysisCacheEntry, SourceAnalysisRecord};

pub const R3_ONTOLOGY_VERSION: &str = "codenoesis.ontology/rust/v3";
pub const R3_PIPELINE_VERSION: &str = "codenoesis.pipeline/s4-r3-v1";
pub const R3_WORKSPACE_EXTRACTOR_VERSION: &str = "codenoesis.rust-workspace/s4-r3-v1";
pub const R3_EXTRACTION_CONTRACT_VERSION: &str = "codenoesis.extraction/v3";
pub const R3_WORKSPACE_PROFILE: &str = "cargo-root-package-v1";
pub const MAX_R3_LITERAL_MEMBERS: u64 = 200;
pub const MAX_R3_PROJECTED_MEMBERS: u64 = 201;
pub const MAX_R3_EXCLUSIONS: u64 = 200;
pub const MAX_R3_PACKAGE_MANIFESTS: u64 = 200;
pub const MAX_R3_CRATE_TARGETS: u64 = 200;
pub const MAX_R3_BINARY_ROOTS_PER_PACKAGE: u64 = 64;

pub const R3_COVERAGE_CAPABILITIES: [&str; 10] = [
    "cargo.build_script_not_executed",
    "cargo.dependencies_deferred",
    "cargo.features_deferred",
    "cargo.package_metadata_deferred",
    "cargo.patch_deferred",
    "cargo.proc_macro_not_executed",
    "cargo.required_features_deferred",
    "cargo.target_world_deferred",
    "cargo.workspace_inheritance_deferred",
    "workspace.external_gitlink_member_not_analyzed",
];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RootPackageShape {
    StandaloneRootPackage,
    VirtualWorkspace,
    NonVirtualWorkspace,
}

impl RootPackageShape {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StandaloneRootPackage => "standalone_root_package",
            Self::VirtualWorkspace => "virtual_workspace",
            Self::NonVirtualWorkspace => "non_virtual_workspace",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum WorkspaceMemberSource {
    LiteralMember,
    ImplicitRootPackage,
    ExplicitRootMember,
}

impl WorkspaceMemberSource {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LiteralMember => "literal_member",
            Self::ImplicitRootPackage => "implicit_root_package",
            Self::ExplicitRootMember => "explicit_root_member",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum WorkspaceTargetKind {
    Library,
    Binary,
}

impl WorkspaceTargetKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Library => "lib",
            Self::Binary => "bin",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalWorkspaceBoundary {
    pub path: String,
    pub boundary_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RootPackageTarget {
    pub crate_id: String,
    pub member_path: String,
    pub member_source: WorkspaceMemberSource,
    pub manifest_path: String,
    pub package_name: String,
    pub target_kind: WorkspaceTargetKind,
    pub target_name: String,
    pub source_path: String,
}

impl RootPackageTarget {
    #[must_use]
    pub fn new(
        repository_identity: &str,
        member_path: String,
        member_source: WorkspaceMemberSource,
        manifest_path: String,
        package_name: String,
        target_kind: WorkspaceTargetKind,
        target_name: String,
        source_path: String,
    ) -> Self {
        let crate_id = workspace_crate_id(
            repository_identity,
            &manifest_path,
            &package_name,
            target_kind.as_str(),
            &target_name,
        );
        Self {
            crate_id,
            member_path,
            member_source,
            manifest_path,
            package_name,
            target_kind,
            target_name,
            source_path,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RootPackageMember {
    pub path: String,
    pub manifest_path: Option<String>,
    pub member_source: WorkspaceMemberSource,
    pub crate_ids: Vec<String>,
    pub external_boundary_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RootPackageWorkspacePlan {
    pub root_shape: RootPackageShape,
    pub members: Vec<RootPackageMember>,
    pub excluded_paths: Vec<String>,
    pub targets: Vec<RootPackageTarget>,
}

impl RootPackageWorkspacePlan {
    /// Validates canonical ordering, identity compatibility, and R3 cardinalities.
    ///
    /// # Errors
    ///
    /// Returns a contract error when the plan does not satisfy the reviewed R3 model.
    pub fn validate(
        &self,
        knowledge: &RustWorkspaceKnowledge,
    ) -> Result<(), RootPackageWorkspaceError> {
        knowledge
            .validate()
            .map_err(RootPackageWorkspaceError::Source)?;
        if self.members.is_empty()
            || u64::try_from(self.members.len()).unwrap_or(u64::MAX) > MAX_R3_PROJECTED_MEMBERS
            || u64::try_from(self.excluded_paths.len()).unwrap_or(u64::MAX) > MAX_R3_EXCLUSIONS
            || u64::try_from(self.targets.len()).unwrap_or(u64::MAX) > MAX_R3_CRATE_TARGETS
            || !ordered_unique(self.members.iter().map(|member| member.path.as_str()))
            || !ordered_unique(self.excluded_paths.iter().map(String::as_str))
        {
            return Err(RootPackageWorkspaceError::ContractInvalid);
        }
        let member_paths = self
            .members
            .iter()
            .map(|member| member.path.as_str())
            .collect::<BTreeSet<_>>();
        if self.members.iter().any(|member| {
            (member.path != "." && !valid_canonical_path(&member.path))
                || member.manifest_path.as_ref().is_some_and(|path| {
                    !valid_canonical_path(path)
                        || if member.path == "." {
                            path != "Cargo.toml"
                        } else {
                            path != &format!("{}/Cargo.toml", member.path)
                        }
                })
                || member
                    .external_boundary_id
                    .as_ref()
                    .is_some_and(|identifier| !valid_boundary_id(identifier))
        }) || self
            .excluded_paths
            .iter()
            .any(|path| !valid_canonical_path(path) || member_paths.contains(path.as_str()))
        {
            return Err(RootPackageWorkspaceError::ContractInvalid);
        }
        let targets_by_member = self.targets.iter().fold(
            BTreeMap::<&str, Vec<&RootPackageTarget>>::new(),
            |mut grouped, target| {
                grouped.entry(&target.member_path).or_default().push(target);
                grouped
            },
        );
        let graph_entities = knowledge
            .graph
            .entities
            .iter()
            .map(|entity| (entity.id.as_str(), entity))
            .collect::<BTreeMap<_, _>>();
        let mut target_ids = BTreeSet::new();
        let mut previous_target = None;
        for target in &self.targets {
            let order = (
                target.manifest_path.as_bytes(),
                target.target_kind,
                target.target_name.as_bytes(),
                target.source_path.as_bytes(),
            );
            if previous_target.is_some_and(|previous| previous >= order)
                || target.crate_id
                    != workspace_crate_id(
                        knowledge.graph.repository_identity.as_str(),
                        &target.manifest_path,
                        &target.package_name,
                        target.target_kind.as_str(),
                        &target.target_name,
                    )
                || !target_ids.insert(target.crate_id.as_str())
                || !member_paths.contains(target.member_path.as_str())
                || !valid_canonical_path(&target.manifest_path)
                || !valid_canonical_path(&target.source_path)
                || target.package_name.is_empty()
                || target.package_name.len() > 255
                || target.target_name.is_empty()
                || target.target_name.len() > 255
            {
                return Err(RootPackageWorkspaceError::ContractInvalid);
            }
            previous_target = Some(order);
            let Some(entity) = graph_entities.get(target.crate_id.as_str()) else {
                return Err(RootPackageWorkspaceError::ContractInvalid);
            };
            if entity.properties.get("manifest_path") != Some(&target.manifest_path)
                || entity.properties.get("package_name") != Some(&target.package_name)
                || entity.properties.get("target_kind").map(String::as_str)
                    != Some(target.target_kind.as_str())
                || entity.properties.get("target_name") != Some(&target.target_name)
                || entity
                    .properties
                    .get("workspace_member_source")
                    .map(String::as_str)
                    != Some(target.member_source.as_str())
                || entity
                    .properties
                    .get("workspace_root_shape")
                    .map(String::as_str)
                    != Some(self.root_shape.as_str())
            {
                return Err(RootPackageWorkspaceError::ContractInvalid);
            }
        }
        for member in &self.members {
            let planned_targets = targets_by_member
                .get(member.path.as_str())
                .cloned()
                .unwrap_or_default();
            let mut expected_ids = planned_targets
                .iter()
                .map(|target| target.crate_id.clone())
                .collect::<Vec<_>>();
            expected_ids.sort();
            if member.crate_ids != expected_ids
                || !ordered_unique(member.crate_ids.iter().map(String::as_str))
            {
                return Err(RootPackageWorkspaceError::ContractInvalid);
            }
            let external = member.external_boundary_id.is_some();
            if external
                != (member.manifest_path.is_none()
                    && member.crate_ids.is_empty()
                    && member.member_source == WorkspaceMemberSource::LiteralMember)
                || !external && (member.manifest_path.is_none() || member.crate_ids.is_empty())
                || planned_targets.iter().any(|target| {
                    target.member_source != member.member_source
                        || member.manifest_path.as_deref() != Some(target.manifest_path.as_str())
                })
            {
                return Err(RootPackageWorkspaceError::ContractInvalid);
            }
        }
        let root_members = self
            .members
            .iter()
            .filter(|member| member.path == ".")
            .collect::<Vec<_>>();
        match self.root_shape {
            RootPackageShape::VirtualWorkspace if !root_members.is_empty() => {
                return Err(RootPackageWorkspaceError::ContractInvalid);
            }
            RootPackageShape::StandaloneRootPackage | RootPackageShape::NonVirtualWorkspace
                if root_members.len() != 1 =>
            {
                return Err(RootPackageWorkspaceError::ContractInvalid);
            }
            _ => {}
        }
        if knowledge
            .extraction_chunks
            .iter()
            .any(|chunk| !target_ids.contains(chunk.crate_id.as_str()))
            || target_ids.iter().any(|target_id| {
                !knowledge
                    .extraction_chunks
                    .iter()
                    .any(|chunk| chunk.crate_id == *target_id)
            })
            || knowledge
                .graph
                .coverage
                .iter()
                .chain(
                    knowledge
                        .extraction_chunks
                        .iter()
                        .flat_map(|chunk| &chunk.coverage),
                )
                .any(|gap| !valid_coverage_capability(&gap.capability))
        {
            return Err(RootPackageWorkspaceError::ContractInvalid);
        }
        Ok(())
    }

    #[must_use]
    pub fn target(&self, crate_id: &str) -> Option<&RootPackageTarget> {
        self.targets
            .iter()
            .find(|target| target.crate_id == crate_id)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RootPackageWorkspaceKnowledge {
    pub plan: RootPackageWorkspacePlan,
    pub knowledge: RustWorkspaceKnowledge,
}

impl RootPackageWorkspaceKnowledge {
    /// Validates the R3 plan and inherited graph closure.
    ///
    /// # Errors
    ///
    /// Returns a typed R3 contract failure.
    pub fn validate(&self) -> Result<(), RootPackageWorkspaceError> {
        self.plan.validate(&self.knowledge)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RootPackageWorkspaceExtraction {
    pub knowledge: RootPackageWorkspaceKnowledge,
    pub cache_entries: Vec<AnalysisCacheEntry>,
    pub source_records: Vec<SourceAnalysisRecord>,
    pub parser_invocation_count: u64,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum WorkspaceManifestReason {
    MalformedToml,
    MissingRootManifest,
    UnsupportedRootShape,
    UnsupportedStructuralKey,
    InvalidMemberPath,
    InvalidExclusionPath,
    MissingMemberManifest,
    InvalidPackageManifest,
    InvalidTargetPath,
    MissingTargetRoot,
    AmbiguousConventionalTarget,
    UnicodeNormalizationCollision,
}

impl WorkspaceManifestReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MalformedToml => "malformed_toml",
            Self::MissingRootManifest => "missing_root_manifest",
            Self::UnsupportedRootShape => "unsupported_root_shape",
            Self::UnsupportedStructuralKey => "unsupported_structural_key",
            Self::InvalidMemberPath => "invalid_member_path",
            Self::InvalidExclusionPath => "invalid_exclusion_path",
            Self::MissingMemberManifest => "missing_member_manifest",
            Self::InvalidPackageManifest => "invalid_package_manifest",
            Self::InvalidTargetPath => "invalid_target_path",
            Self::MissingTargetRoot => "missing_target_root",
            Self::AmbiguousConventionalTarget => "ambiguous_conventional_target",
            Self::UnicodeNormalizationCollision => "unicode_normalization_collision",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RootPackageLimit {
    WorkspaceMembers,
    WorkspaceExclusions,
    PackageManifests,
    WorkspaceCrates,
    BinaryRootsPerPackage,
    SingleManifestBytes,
}

impl RootPackageLimit {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WorkspaceMembers => "workspace_members",
            Self::WorkspaceExclusions => "workspace_exclusions",
            Self::PackageManifests => "package_manifests",
            Self::WorkspaceCrates => "workspace_crates",
            Self::BinaryRootsPerPackage => "binary_roots_per_package",
            Self::SingleManifestBytes => "single_manifest_bytes",
        }
    }

    #[must_use]
    pub const fn maximum(self) -> u64 {
        match self {
            Self::WorkspaceMembers => MAX_R3_LITERAL_MEMBERS,
            Self::WorkspaceExclusions => MAX_R3_EXCLUSIONS,
            Self::PackageManifests => MAX_R3_PACKAGE_MANIFESTS,
            Self::WorkspaceCrates => MAX_R3_CRATE_TARGETS,
            Self::BinaryRootsPerPackage => MAX_R3_BINARY_ROOTS_PER_PACKAGE,
            Self::SingleManifestBytes => STANDARD_LOCAL_S1_LIMITS.single_file_bytes,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RootPackageWorkspaceError {
    InvalidManifest {
        reason: WorkspaceManifestReason,
        path: Option<String>,
    },
    MemberConflict {
        path: String,
    },
    TargetConflict {
        path: String,
        target_kind: WorkspaceTargetKind,
        target_name: String,
    },
    LimitExceeded {
        limit: RootPackageLimit,
        maximum: u64,
        observed: u64,
    },
    Source(WorkspaceError),
    ContractInvalid,
}

impl Display for RootPackageWorkspaceError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidManifest { .. } => "invalid root package workspace manifest",
            Self::MemberConflict { .. } => "conflicting root package workspace member",
            Self::TargetConflict { .. } => "conflicting root package workspace target",
            Self::LimitExceeded { .. } => "root package workspace limit exceeded",
            Self::Source(error) => return Display::fmt(error, formatter),
            Self::ContractInvalid => "invalid root package workspace contract",
        })
    }
}

impl Error for RootPackageWorkspaceError {}

#[must_use]
pub const fn root_package_limit_exceeded(
    limit: RootPackageLimit,
    observed: u64,
) -> RootPackageWorkspaceError {
    let maximum = limit.maximum();
    RootPackageWorkspaceError::LimitExceeded {
        limit,
        maximum,
        observed: if observed > maximum + 1 {
            maximum + 1
        } else {
            observed
        },
    }
}

fn ordered_unique<'a>(values: impl IntoIterator<Item = &'a str>) -> bool {
    let mut previous = None;
    for value in values {
        if previous.is_some_and(|previous| previous >= value) {
            return false;
        }
        previous = Some(value);
    }
    true
}

fn valid_canonical_path(path: &str) -> bool {
    !path.is_empty()
        && u64::try_from(path.len()).unwrap_or(u64::MAX) <= STANDARD_LOCAL_S1_LIMITS.path_bytes
        && !path.starts_with('/')
        && !path.ends_with('/')
        && !path.contains('\\')
        && !path
            .bytes()
            .any(|byte| byte == 0 || byte.is_ascii_control())
        && u64::try_from(path.split('/').count()).unwrap_or(u64::MAX)
            <= STANDARD_LOCAL_S1_LIMITS.recursion_depth
        && path.split('/').all(|component| {
            !component.is_empty()
                && !matches!(component, "." | "..")
                && u64::try_from(component.len()).unwrap_or(u64::MAX)
                    <= STANDARD_LOCAL_S1_LIMITS.path_component_bytes
        })
}

fn valid_boundary_id(identifier: &str) -> bool {
    identifier
        .strip_prefix("urn:codenoesis:repository-boundary:sha256:")
        .is_some_and(|digest| {
            digest.len() == 64
                && digest
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        })
}

fn valid_coverage_capability(capability: &str) -> bool {
    R3_COVERAGE_CAPABILITIES.contains(&capability)
        || matches!(
            capability,
            "build_script_execution_forbidden"
                | "compiler_cross_crate_use_resolution"
                | "rust_unsupported_construct"
        )
}
