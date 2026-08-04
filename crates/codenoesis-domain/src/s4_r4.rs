use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::knowledge::{ClaimState, ClaimSubjectKind};
use crate::s4::{WorkspaceClaim, WorkspaceEvidence, workspace_claim_id};
use crate::s4_r3::{RootPackageWorkspaceError, RootPackageWorkspaceKnowledge};
use crate::s5::{AnalysisCacheEntry, SourceAnalysisRecord};

pub const R4_ONTOLOGY_VERSION: &str = "codenoesis.ontology/rust/v4";
pub const R4_PIPELINE_VERSION: &str = "codenoesis.pipeline/s4-r4-v1";
pub const R4_CARGO_EXTRACTOR_VERSION: &str = "codenoesis.cargo-manifest/s4-r4-v1";
pub const R4_EXTRACTION_CONTRACT_VERSION: &str = "codenoesis.extraction/v4";
pub const R4_MANIFEST_PROFILE: &str = "cargo-manifest-facts-v1";

pub const MAX_R4_MANIFEST_FACT_ENTITIES: u64 = 10_000;
pub const MAX_R4_DEPENDENCIES_PER_MANIFEST: u64 = 256;
pub const MAX_R4_FEATURES_PER_MANIFEST: u64 = 256;
pub const MAX_R4_FEATURE_MEMBERS_PER_FEATURE: u64 = 128;
pub const MAX_R4_TARGETS_PER_PACKAGE: u64 = 128;
pub const MAX_R4_PATCHES_PER_WORKSPACE: u64 = 256;
pub const MAX_R4_METADATA_FIELDS_PER_OWNER: u64 = 32;
pub const MAX_R4_REQUESTED_FEATURES_PER_DECLARATION: u64 = 64;
pub const MAX_R4_TARGET_PREDICATES_PER_MANIFEST: u64 = 128;
pub const MAX_R4_DECLARATION_STRING_BYTES: u64 = 2_048;
pub const MAX_R4_EXTERNAL_LOCATOR_BYTES: u64 = 4_096;
pub const R4_DETERMINISM_PERMUTATIONS: u64 = 50;

const CARGO_ENTITY_ID_DOMAIN: &str = "codenoesis.entity-id/cargo-manifest/v1";
const CARGO_RELATIONSHIP_ID_DOMAIN: &str = "codenoesis.relationship-id/cargo-manifest/v1";
const DIAGNOSTIC_ID_DOMAIN: &str = "codenoesis.diagnostic-id/cargo-manifest/v1";
const COVERAGE_GAP_ID_DOMAIN: &str = "codenoesis.coverage-gap-id/v3";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CargoEntityKind {
    Manifest,
    WorkspacePackageDefaults,
    Package,
    Target,
    Dependency,
    Feature,
    Patch,
    BuildScript,
}

impl CargoEntityKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Manifest => "cargo.manifest",
            Self::WorkspacePackageDefaults => "cargo.workspace_package_defaults",
            Self::Package => "cargo.package",
            Self::Target => "cargo.target",
            Self::Dependency => "cargo.dependency",
            Self::Feature => "cargo.feature",
            Self::Patch => "cargo.patch",
            Self::BuildScript => "cargo.build_script",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CargoRelationshipKind {
    Declares,
    ReferencesDeclaration,
    Materializes,
}

impl CargoRelationshipKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Declares => "DECLARES",
            Self::ReferencesDeclaration => "REFERENCES_DECLARATION",
            Self::Materializes => "MATERIALIZES",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ManifestRole {
    WorkspaceRoot,
    WorkspaceMember,
}

impl ManifestRole {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WorkspaceRoot => "workspace_root",
            Self::WorkspaceMember => "workspace_member",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CargoTargetKind {
    Library,
    Binary,
    Example,
    Test,
    Bench,
}

impl CargoTargetKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Library => "lib",
            Self::Binary => "bin",
            Self::Example => "example",
            Self::Test => "test",
            Self::Bench => "bench",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TargetNameSource {
    Literal,
    PackageDefault,
    PathStem,
}

impl TargetNameSource {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Literal => "literal",
            Self::PackageDefault => "package_default",
            Self::PathStem => "path_stem",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TargetPathSource {
    Literal,
    Conventional,
}

impl TargetPathSource {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Literal => "literal",
            Self::Conventional => "conventional",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SourceAnalysisState {
    AnalyzedR3,
    NotAnalyzed,
}

impl SourceAnalysisState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AnalyzedR3 => "analyzed_r3",
            Self::NotAnalyzed => "not_analyzed",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DependencyScope {
    Workspace,
    Package,
}

impl DependencyScope {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Workspace => "workspace",
            Self::Package => "package",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DependencyKind {
    Normal,
    Development,
    Build,
}

impl DependencyKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Development => "dev",
            Self::Build => "build",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DependencySourceKind {
    RegistryDefault,
    RegistryNamed,
    Path,
    Git,
    WorkspaceInherited,
}

impl DependencySourceKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RegistryDefault => "registry_default",
            Self::RegistryNamed => "registry_named",
            Self::Path => "path",
            Self::Git => "git",
            Self::WorkspaceInherited => "workspace_inherited",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LocatorReferenceKind {
    Branch,
    Tag,
    Revision,
}

impl LocatorReferenceKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Branch => "branch",
            Self::Tag => "tag",
            Self::Revision => "rev",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FeatureMemberSyntax {
    Bare,
    ExplicitDependency,
    DependencyFeature,
    WeakDependencyFeature,
}

impl FeatureMemberSyntax {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bare => "bare",
            Self::ExplicitDependency => "explicit_dependency",
            Self::DependencyFeature => "dependency_feature",
            Self::WeakDependencyFeature => "weak_dependency_feature",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PatchSelectorKind {
    CratesIo,
    NamedRegistry,
    SourceLocatorSha256,
}

impl PatchSelectorKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CratesIo => "crates_io",
            Self::NamedRegistry => "named_registry",
            Self::SourceLocatorSha256 => "source_locator_sha256",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BuildScriptSelection {
    ExplicitPath,
    ExplicitDisabled,
    ConventionalPresent,
    Absent,
}

impl BuildScriptSelection {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExplicitPath => "explicit_path",
            Self::ExplicitDisabled => "explicit_disabled",
            Self::ConventionalPresent => "conventional_present",
            Self::Absent => "absent",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CargoCoverageState {
    NotResolved,
    NotFetched,
    Redacted,
    NotApplied,
    NotExecuted,
    NotAnalyzed,
    Unsupported,
}

impl CargoCoverageState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotResolved => "not_resolved",
            Self::NotFetched => "not_fetched",
            Self::Redacted => "redacted",
            Self::NotApplied => "not_applied",
            Self::NotExecuted => "not_executed",
            Self::NotAnalyzed => "not_analyzed",
            Self::Unsupported => "unsupported",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclaredString {
    pub value: String,
    pub evidence_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclaredName {
    pub value: String,
    pub evidence_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclaredBoolean {
    pub value: bool,
    pub evidence_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclaredPath {
    pub declared: String,
    pub normalized: String,
    pub evidence_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeclaredValue {
    String(String),
    Boolean(bool),
    StringArray(Vec<String>),
    Publish {
        enabled: bool,
        registries: Vec<String>,
    },
    LocatorSha256(String),
    Path {
        declared: String,
        normalized: String,
    },
    WorkspaceReference {
        source_entity_id: String,
        source_field: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetadataFact {
    pub field: String,
    pub value: DeclaredValue,
    pub inherited_from: Option<String>,
    pub evidence_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocatorDigest {
    pub sha256: String,
    pub evidence_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocatorReference {
    pub kind: LocatorReferenceKind,
    pub sha256: String,
    pub evidence_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencySource {
    pub kind: DependencySourceKind,
    pub version_requirement: Option<DeclaredString>,
    pub registry_name: Option<DeclaredString>,
    pub path: Option<DeclaredPath>,
    pub git_locator: Option<LocatorDigest>,
    pub git_reference: Option<LocatorReference>,
    pub workspace_reference_id: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TargetOptions {
    pub crate_types: Vec<DeclaredName>,
    pub proc_macro: Option<DeclaredBoolean>,
    pub bench: Option<DeclaredBoolean>,
    pub doc: Option<DeclaredBoolean>,
    pub doctest: Option<DeclaredBoolean>,
    pub test: Option<DeclaredBoolean>,
    pub harness: Option<DeclaredBoolean>,
    pub edition: Option<DeclaredString>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeatureMember {
    pub lexeme: String,
    pub syntax: FeatureMemberSyntax,
    pub dependency_name: Option<String>,
    pub feature_name: Option<String>,
    pub evidence_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatchSourceSelector {
    pub kind: PatchSelectorKind,
    pub name: Option<String>,
    pub sha256: Option<String>,
    pub evidence_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestProperties {
    pub manifest_path: String,
    pub manifest_role: ManifestRole,
    pub root_shape: String,
    pub package_table_present: bool,
    pub workspace_table_present: bool,
    pub evidence_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceDefaultsProperties {
    pub manifest_id: String,
    pub manifest_path: String,
    pub metadata: Vec<MetadataFact>,
    pub evidence_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageProperties {
    pub manifest_id: String,
    pub manifest_path: String,
    pub package_name: String,
    pub metadata: Vec<MetadataFact>,
    pub evidence_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetProperties {
    pub manifest_id: String,
    pub package_id: String,
    pub manifest_path: String,
    pub target_kind: CargoTargetKind,
    pub target_name: String,
    pub name_source: TargetNameSource,
    pub source_path: DeclaredPath,
    pub path_source: TargetPathSource,
    pub required_features: Vec<DeclaredName>,
    pub options: TargetOptions,
    pub source_analysis_state: SourceAnalysisState,
    pub materialized_crate_id: Option<String>,
    pub evidence_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyProperties {
    pub manifest_id: String,
    pub owner_id: String,
    pub manifest_path: String,
    pub scope: DependencyScope,
    pub dependency_kind: DependencyKind,
    pub target_predicate: Option<DeclaredString>,
    pub declared_name: String,
    pub package_name: Option<DeclaredString>,
    pub source: DependencySource,
    pub optional: Option<DeclaredBoolean>,
    pub default_features: Option<DeclaredBoolean>,
    pub requested_features: Vec<DeclaredName>,
    pub evidence_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeatureProperties {
    pub manifest_id: String,
    pub package_id: String,
    pub manifest_path: String,
    pub feature_name: String,
    pub members: Vec<FeatureMember>,
    pub evidence_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatchProperties {
    pub manifest_id: String,
    pub manifest_path: String,
    pub source_selector: PatchSourceSelector,
    pub declared_name: String,
    pub package_name: Option<DeclaredString>,
    pub source: DependencySource,
    pub evidence_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildScriptProperties {
    pub manifest_id: String,
    pub package_id: String,
    pub manifest_path: String,
    pub selection: BuildScriptSelection,
    pub path: Option<DeclaredPath>,
    pub committed_present: bool,
    pub evidence_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CargoEntityProperties {
    Manifest(ManifestProperties),
    WorkspacePackageDefaults(WorkspaceDefaultsProperties),
    Package(PackageProperties),
    Target(TargetProperties),
    Dependency(DependencyProperties),
    Feature(FeatureProperties),
    Patch(PatchProperties),
    BuildScript(BuildScriptProperties),
}

impl CargoEntityProperties {
    #[must_use]
    pub const fn kind(&self) -> CargoEntityKind {
        match self {
            Self::Manifest(_) => CargoEntityKind::Manifest,
            Self::WorkspacePackageDefaults(_) => CargoEntityKind::WorkspacePackageDefaults,
            Self::Package(_) => CargoEntityKind::Package,
            Self::Target(_) => CargoEntityKind::Target,
            Self::Dependency(_) => CargoEntityKind::Dependency,
            Self::Feature(_) => CargoEntityKind::Feature,
            Self::Patch(_) => CargoEntityKind::Patch,
            Self::BuildScript(_) => CargoEntityKind::BuildScript,
        }
    }

    #[must_use]
    pub fn manifest_path(&self) -> &str {
        match self {
            Self::Manifest(properties) => &properties.manifest_path,
            Self::WorkspacePackageDefaults(properties) => &properties.manifest_path,
            Self::Package(properties) => &properties.manifest_path,
            Self::Target(properties) => &properties.manifest_path,
            Self::Dependency(properties) => &properties.manifest_path,
            Self::Feature(properties) => &properties.manifest_path,
            Self::Patch(properties) => &properties.manifest_path,
            Self::BuildScript(properties) => &properties.manifest_path,
        }
    }

    #[must_use]
    pub fn evidence_id(&self) -> &str {
        match self {
            Self::Manifest(properties) => &properties.evidence_id,
            Self::WorkspacePackageDefaults(properties) => &properties.evidence_id,
            Self::Package(properties) => &properties.evidence_id,
            Self::Target(properties) => &properties.evidence_id,
            Self::Dependency(properties) => &properties.evidence_id,
            Self::Feature(properties) => &properties.evidence_id,
            Self::Patch(properties) => &properties.evidence_id,
            Self::BuildScript(properties) => &properties.evidence_id,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CargoEntity {
    pub id: String,
    pub name: String,
    pub properties: CargoEntityProperties,
}

impl CargoEntity {
    #[must_use]
    pub const fn kind(&self) -> CargoEntityKind {
        self.properties.kind()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CargoRelationship {
    pub id: String,
    pub kind: CargoRelationshipKind,
    pub source: String,
    pub target: String,
    pub evidence_ids: Vec<String>,
}

impl CargoRelationship {
    #[must_use]
    pub fn new(
        kind: CargoRelationshipKind,
        source: String,
        target: String,
        mut evidence_ids: Vec<String>,
    ) -> Self {
        evidence_ids.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
        evidence_ids.dedup();
        Self {
            id: cargo_relationship_id(kind, &source, &target),
            kind,
            source,
            target,
            evidence_ids,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CargoDiagnostic {
    pub id: String,
    pub code: String,
    pub message: String,
    pub evidence_ids: Vec<String>,
}

impl CargoDiagnostic {
    #[must_use]
    pub fn new(
        repository_identity: &str,
        code: &str,
        message: &str,
        mut evidence_ids: Vec<String>,
    ) -> Self {
        evidence_ids.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
        evidence_ids.dedup();
        let mut components = Vec::with_capacity(evidence_ids.len() + 3);
        components.push(DIAGNOSTIC_ID_DOMAIN);
        components.push(repository_identity);
        components.push(code);
        components.extend(evidence_ids.iter().map(String::as_str));
        Self {
            id: stable_id("urn:codenoesis:diagnostic:blake3:", &components),
            code: code.to_owned(),
            message: message.to_owned(),
            evidence_ids,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CargoCoverageGap {
    pub id: String,
    pub capability: String,
    pub state: CargoCoverageState,
    pub evidence_ids: Vec<String>,
}

impl CargoCoverageGap {
    #[must_use]
    pub fn new(
        repository_identity: &str,
        commit_oid: &str,
        capability: &str,
        state: CargoCoverageState,
        mut evidence_ids: Vec<String>,
    ) -> Self {
        evidence_ids.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
        evidence_ids.dedup();
        let mut components = Vec::with_capacity(evidence_ids.len() + 5);
        components.push(COVERAGE_GAP_ID_DOMAIN);
        components.push(repository_identity);
        components.push(commit_oid);
        components.push(capability);
        components.push(state.as_str());
        components.extend(evidence_ids.iter().map(String::as_str));
        Self {
            id: stable_id("urn:codenoesis:coverage-gap:blake3:", &components),
            capability: capability.to_owned(),
            state,
            evidence_ids,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CargoManifestExtractionChunk {
    pub manifest_id: String,
    pub manifest_path: String,
    pub entities: Vec<CargoEntity>,
    pub relationships: Vec<CargoRelationship>,
    pub claims: Vec<WorkspaceClaim>,
    pub evidence: Vec<WorkspaceEvidence>,
    pub diagnostics: Vec<CargoDiagnostic>,
    pub coverage: Vec<CargoCoverageGap>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestIndexEntry {
    pub manifest_id: String,
    pub manifest_path: String,
    pub package_id: Option<String>,
    pub fact_entity_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CargoManifestGraph {
    pub entities: Vec<CargoEntity>,
    pub relationships: Vec<CargoRelationship>,
    pub claims: Vec<WorkspaceClaim>,
    pub evidence: Vec<WorkspaceEvidence>,
    pub diagnostics: Vec<CargoDiagnostic>,
    pub coverage: Vec<CargoCoverageGap>,
    pub manifest_index: Vec<ManifestIndexEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CargoManifestKnowledge {
    pub workspace: RootPackageWorkspaceKnowledge,
    pub extraction_chunks: Vec<CargoManifestExtractionChunk>,
    pub graph: CargoManifestGraph,
}

impl CargoManifestKnowledge {
    /// Validates R4 identities, ordering, evidence closure, and inherited R3 knowledge.
    ///
    /// # Errors
    ///
    /// Returns a closed R4 contract failure on the first invalid invariant.
    pub fn validate(&self) -> Result<(), CargoManifestFactError> {
        self.workspace
            .validate()
            .map_err(CargoManifestFactError::Source)?;
        self.validate_graph_shape()?;
        self.validate_entity_relationship_evidence()?;
        self.validate_claim_diagnostic_coverage()?;
        self.validate_manifest_index()
    }

    fn validate_graph_shape(&self) -> Result<(), CargoManifestFactError> {
        let graph = &self.graph;
        if graph.entities.is_empty()
            || graph.evidence.is_empty()
            || u64::try_from(graph.entities.len()).unwrap_or(u64::MAX)
                > MAX_R4_MANIFEST_FACT_ENTITIES
            || !ordered_unique(graph.entities.iter().map(|entity| entity.id.as_str()))
            || !ordered_unique(
                graph
                    .relationships
                    .iter()
                    .map(|relationship| relationship.id.as_str()),
            )
            || !ordered_unique(graph.claims.iter().map(|claim| claim.id.as_str()))
            || !ordered_unique(graph.evidence.iter().map(|evidence| evidence.id.as_str()))
            || !ordered_unique(
                graph
                    .diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.id.as_str()),
            )
            || !ordered_unique(graph.coverage.iter().map(|gap| gap.id.as_str()))
            || !ordered_unique(
                graph
                    .manifest_index
                    .iter()
                    .map(|entry| entry.manifest_path.as_str()),
            )
        {
            return Err(CargoManifestFactError::ContractInvalid);
        }
        Ok(())
    }

    fn validate_entity_relationship_evidence(&self) -> Result<(), CargoManifestFactError> {
        let graph = &self.graph;
        let rust_entity_ids = self
            .workspace
            .knowledge
            .graph
            .entities
            .iter()
            .map(|entity| entity.id.as_str());
        let cargo_entity_ids = graph
            .entities
            .iter()
            .map(|entity| entity.id.as_str())
            .collect::<BTreeSet<_>>();
        let all_entity_ids = rust_entity_ids
            .chain(cargo_entity_ids.iter().copied())
            .collect::<BTreeSet<_>>();
        let evidence_ids = graph
            .evidence
            .iter()
            .map(|evidence| evidence.id.as_str())
            .collect::<BTreeSet<_>>();
        if graph.entities.iter().any(|entity| {
            entity.name.is_empty()
                || entity.name.len() > 2_048
                || !evidence_ids.contains(entity.properties.evidence_id())
        }) || graph.relationships.iter().any(|relationship| {
            relationship.id
                != cargo_relationship_id(
                    relationship.kind,
                    &relationship.source,
                    &relationship.target,
                )
                || !all_entity_ids.contains(relationship.source.as_str())
                || !all_entity_ids.contains(relationship.target.as_str())
                || relationship.evidence_ids.is_empty()
                || relationship
                    .evidence_ids
                    .iter()
                    .any(|identifier| !evidence_ids.contains(identifier.as_str()))
        }) {
            return Err(CargoManifestFactError::ContractInvalid);
        }
        Ok(())
    }

    fn validate_claim_diagnostic_coverage(&self) -> Result<(), CargoManifestFactError> {
        let graph = &self.graph;
        let cargo_entity_ids = graph
            .entities
            .iter()
            .map(|entity| entity.id.as_str())
            .collect::<BTreeSet<_>>();
        let relationship_ids = graph
            .relationships
            .iter()
            .map(|relationship| relationship.id.as_str())
            .collect::<BTreeSet<_>>();
        let evidence_ids = graph
            .evidence
            .iter()
            .map(|evidence| evidence.id.as_str())
            .collect::<BTreeSet<_>>();
        let cargo_subjects = cargo_entity_ids
            .iter()
            .map(|identifier| (ClaimSubjectKind::Entity, *identifier))
            .chain(
                relationship_ids
                    .iter()
                    .map(|identifier| (ClaimSubjectKind::Relationship, *identifier)),
            )
            .collect::<BTreeSet<_>>();
        let claimed_subjects = graph
            .claims
            .iter()
            .map(|claim| (claim.subject_kind, claim.subject_id.as_str()))
            .collect::<BTreeSet<_>>();
        if cargo_subjects != claimed_subjects
            || cargo_subjects.len() != graph.claims.len()
            || graph.claims.iter().any(|claim| {
                !matches!(
                    claim.state,
                    ClaimState::DeterministicFact | ClaimState::DerivedFact
                ) || claim.id
                    != workspace_claim_id(claim.subject_kind, &claim.subject_id, claim.state)
                    || claim.evidence_ids.is_empty()
                    || claim
                        .evidence_ids
                        .iter()
                        .any(|identifier| !evidence_ids.contains(identifier.as_str()))
            })
            || graph.diagnostics.iter().any(|diagnostic| {
                diagnostic.evidence_ids.is_empty()
                    || diagnostic.evidence_ids.len() > 64
                    || !ordered_unique(diagnostic.evidence_ids.iter().map(String::as_str))
                    || !valid_diagnostic(&diagnostic.code, &diagnostic.message)
                    || *diagnostic
                        != CargoDiagnostic::new(
                            &self.workspace.knowledge.graph.repository_identity,
                            &diagnostic.code,
                            &diagnostic.message,
                            diagnostic.evidence_ids.clone(),
                        )
                    || diagnostic
                        .evidence_ids
                        .iter()
                        .any(|identifier| !evidence_ids.contains(identifier.as_str()))
            })
            || graph.coverage.iter().any(|gap| {
                gap.evidence_ids.is_empty()
                    || gap.evidence_ids.len() > 64
                    || !ordered_unique(gap.evidence_ids.iter().map(String::as_str))
                    || !valid_coverage(&gap.capability, gap.state)
                    || *gap
                        != CargoCoverageGap::new(
                            &self.workspace.knowledge.graph.repository_identity,
                            &self.workspace.knowledge.graph.commit_oid,
                            &gap.capability,
                            gap.state,
                            gap.evidence_ids.clone(),
                        )
                    || gap
                        .evidence_ids
                        .iter()
                        .any(|identifier| !evidence_ids.contains(identifier.as_str()))
            })
        {
            return Err(CargoManifestFactError::ContractInvalid);
        }
        Ok(())
    }

    fn validate_manifest_index(&self) -> Result<(), CargoManifestFactError> {
        let graph = &self.graph;
        let entities_by_id = graph
            .entities
            .iter()
            .map(|entity| (entity.id.as_str(), entity))
            .collect::<BTreeMap<_, _>>();
        if graph.manifest_index.len() != self.extraction_chunks.len()
            || graph.manifest_index.iter().any(|entry| {
                entities_by_id
                    .get(entry.manifest_id.as_str())
                    .is_none_or(|entity| entity.kind() != CargoEntityKind::Manifest)
                    || !ordered_unique(entry.fact_entity_ids.iter().map(String::as_str))
                    || entry.fact_entity_ids.iter().any(|identifier| {
                        entities_by_id
                            .get(identifier.as_str())
                            .is_none_or(|entity| {
                                entity.properties.manifest_path() != entry.manifest_path
                            })
                    })
            })
        {
            return Err(CargoManifestFactError::ContractInvalid);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CargoManifestFactExtraction {
    pub knowledge: CargoManifestKnowledge,
    pub cache_entries: Vec<AnalysisCacheEntry>,
    pub source_records: Vec<SourceAnalysisRecord>,
    pub parser_invocation_count: u64,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CargoFactKind {
    Manifest,
    WorkspacePackageDefaults,
    Package,
    Target,
    Dependency,
    Feature,
    Patch,
    BuildScript,
}

impl CargoFactKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Manifest => "manifest",
            Self::WorkspacePackageDefaults => "workspace_package_defaults",
            Self::Package => "package",
            Self::Target => "target",
            Self::Dependency => "dependency",
            Self::Feature => "feature",
            Self::Patch => "patch",
            Self::BuildScript => "build_script",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CargoFactReason {
    MalformedValue,
    UnsupportedKey,
    InvalidDeclarationName,
    InvalidRelativePath,
    ConflictingSourceSelectors,
    DuplicateDeclaration,
    MissingWorkspaceDeclaration,
    InvalidFeatureMember,
    InvalidTargetDeclaration,
    UnsupportedStructuralInteraction,
    UnicodeNormalizationCollision,
}

impl CargoFactReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MalformedValue => "malformed_value",
            Self::UnsupportedKey => "unsupported_key",
            Self::InvalidDeclarationName => "invalid_declaration_name",
            Self::InvalidRelativePath => "invalid_relative_path",
            Self::ConflictingSourceSelectors => "conflicting_source_selectors",
            Self::DuplicateDeclaration => "duplicate_declaration",
            Self::MissingWorkspaceDeclaration => "missing_workspace_declaration",
            Self::InvalidFeatureMember => "invalid_feature_member",
            Self::InvalidTargetDeclaration => "invalid_target_declaration",
            Self::UnsupportedStructuralInteraction => "unsupported_structural_interaction",
            Self::UnicodeNormalizationCollision => "unicode_normalization_collision",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CargoFactLimit {
    ManifestFactEntities,
    DependenciesPerManifest,
    FeaturesPerManifest,
    FeatureMembersPerFeature,
    TargetsPerPackage,
    PatchesPerWorkspace,
    MetadataFieldsPerOwner,
    RequestedFeaturesPerDeclaration,
    TargetPredicatesPerManifest,
    DeclarationStringBytes,
    ExternalLocatorBytes,
}

impl CargoFactLimit {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ManifestFactEntities => "manifest_fact_entities",
            Self::DependenciesPerManifest => "dependencies_per_manifest",
            Self::FeaturesPerManifest => "features_per_manifest",
            Self::FeatureMembersPerFeature => "feature_members_per_feature",
            Self::TargetsPerPackage => "targets_per_package",
            Self::PatchesPerWorkspace => "patches_per_workspace",
            Self::MetadataFieldsPerOwner => "metadata_fields_per_owner",
            Self::RequestedFeaturesPerDeclaration => "requested_features_per_declaration",
            Self::TargetPredicatesPerManifest => "target_predicates_per_manifest",
            Self::DeclarationStringBytes => "declaration_string_bytes",
            Self::ExternalLocatorBytes => "external_locator_bytes",
        }
    }

    #[must_use]
    pub const fn maximum(self) -> u64 {
        match self {
            Self::ManifestFactEntities => MAX_R4_MANIFEST_FACT_ENTITIES,
            Self::DependenciesPerManifest => MAX_R4_DEPENDENCIES_PER_MANIFEST,
            Self::FeaturesPerManifest => MAX_R4_FEATURES_PER_MANIFEST,
            Self::FeatureMembersPerFeature => MAX_R4_FEATURE_MEMBERS_PER_FEATURE,
            Self::TargetsPerPackage => MAX_R4_TARGETS_PER_PACKAGE,
            Self::PatchesPerWorkspace => MAX_R4_PATCHES_PER_WORKSPACE,
            Self::MetadataFieldsPerOwner => MAX_R4_METADATA_FIELDS_PER_OWNER,
            Self::RequestedFeaturesPerDeclaration => MAX_R4_REQUESTED_FEATURES_PER_DECLARATION,
            Self::TargetPredicatesPerManifest => MAX_R4_TARGET_PREDICATES_PER_MANIFEST,
            Self::DeclarationStringBytes => MAX_R4_DECLARATION_STRING_BYTES,
            Self::ExternalLocatorBytes => MAX_R4_EXTERNAL_LOCATOR_BYTES,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CargoManifestFactError {
    InvalidFact {
        reason: CargoFactReason,
        path: String,
        fact_kind: CargoFactKind,
        field: Option<&'static str>,
    },
    Conflict {
        path: String,
        fact_kind: CargoFactKind,
        declaration_name_sha256: String,
    },
    LimitExceeded {
        limit: CargoFactLimit,
        maximum: u64,
        observed: u64,
    },
    Source(RootPackageWorkspaceError),
    ContractInvalid,
}

impl Display for CargoManifestFactError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidFact { .. } => "invalid Cargo manifest fact",
            Self::Conflict { .. } => "conflicting Cargo manifest fact",
            Self::LimitExceeded { .. } => "Cargo manifest fact limit exceeded",
            Self::Source(error) => return Display::fmt(error, formatter),
            Self::ContractInvalid => "invalid Cargo manifest fact contract",
        })
    }
}

impl Error for CargoManifestFactError {}

#[must_use]
pub const fn cargo_fact_limit_exceeded(
    limit: CargoFactLimit,
    observed: u64,
) -> CargoManifestFactError {
    let maximum = limit.maximum();
    CargoManifestFactError::LimitExceeded {
        limit,
        maximum,
        observed: if observed > maximum + 1 {
            maximum + 1
        } else {
            observed
        },
    }
}

#[must_use]
pub fn cargo_entity_id(
    repository_identity: &str,
    kind: CargoEntityKind,
    identity_tail: &[&str],
) -> String {
    let mut components = Vec::with_capacity(identity_tail.len() + 3);
    components.push(CARGO_ENTITY_ID_DOMAIN);
    components.push(repository_identity);
    components.push(kind.as_str());
    components.extend_from_slice(identity_tail);
    stable_id("urn:codenoesis:entity:blake3:", &components)
}

#[must_use]
pub fn cargo_relationship_id(kind: CargoRelationshipKind, source: &str, target: &str) -> String {
    stable_id(
        "urn:codenoesis:relationship:blake3:",
        &[CARGO_RELATIONSHIP_ID_DOMAIN, kind.as_str(), source, target],
    )
}

#[must_use]
pub fn cargo_claim(
    subject_kind: ClaimSubjectKind,
    subject_id: String,
    state: ClaimState,
    evidence_ids: Vec<String>,
) -> WorkspaceClaim {
    WorkspaceClaim::new(subject_kind, subject_id, state, evidence_ids)
}

fn valid_coverage(capability: &str, state: CargoCoverageState) -> bool {
    matches!(
        (capability, state),
        (
            "cargo.active_features_not_resolved"
                | "cargo.active_target_not_resolved"
                | "cargo.dependency_graph_not_resolved"
                | "cargo.workspace_inheritance_not_materialized",
            CargoCoverageState::NotResolved
        ) | (
            "cargo.dependency_source_not_fetched",
            CargoCoverageState::NotFetched
        ) | (
            "cargo.external_locator_redacted",
            CargoCoverageState::Redacted
        ) | (
            "cargo.package_file_selection_not_applied" | "cargo.patch_not_applied",
            CargoCoverageState::NotApplied
        ) | (
            "cargo.build_script_not_executed" | "cargo.proc_macro_not_executed",
            CargoCoverageState::NotExecuted
        ) | (
            "cargo.generated_source_not_analyzed" | "cargo.target_source_not_analyzed",
            CargoCoverageState::NotAnalyzed
        ) | (
            "cargo.dependency_advanced_fields_unsupported"
                | "cargo.legacy_badges_unsupported"
                | "cargo.lint_configuration_unsupported"
                | "cargo.package_metadata_table_unsupported"
                | "cargo.profile_tables_unsupported"
                | "cargo.replace_table_unsupported",
            CargoCoverageState::Unsupported
        )
    )
}

fn valid_diagnostic(code: &str, message: &str) -> bool {
    matches!(
        (code, message),
        (
            "cargo.unsupported_manifest_family",
            "Cargo manifest family is outside the selected declaration subset"
        ) | (
            "cargo.unsupported_manifest_field",
            "Cargo manifest field is outside the selected declaration subset"
        ) | (
            "cargo.external_locator_redacted",
            "external Cargo locator is represented only by digest"
        ) | (
            "cargo.target_source_not_analyzed",
            "declared Cargo target source is not analyzed by R4"
        )
    )
}

fn ordered_unique<'a>(values: impl IntoIterator<Item = &'a str>) -> bool {
    let mut previous = None;
    for value in values {
        if previous.is_some_and(|previous_value| previous_value >= value) {
            return false;
        }
        previous = Some(value);
    }
    true
}

fn stable_id(prefix: &str, components: &[&str]) -> String {
    format!(
        "{prefix}{}",
        blake3::hash(&canonical_string_array(components)).to_hex()
    )
}

fn canonical_string_array(components: &[&str]) -> Vec<u8> {
    let mut bytes = vec![b'['];
    for (index, component) in components.iter().enumerate() {
        if index > 0 {
            bytes.push(b',');
        }
        write_json_string(&mut bytes, component);
    }
    bytes.push(b']');
    bytes
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
