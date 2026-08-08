use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter, Write as _};
use std::path::Path;

use crate::RepositoryInventory;
use crate::knowledge::EntityKind;
use crate::s4::{RustWorkspaceKnowledge, WorkspaceVisibility};

pub const ANALYSIS_CACHE_SCHEMA_VERSION: &str = "codenoesis.analysis-cache-entry/v1";
pub const ANALYSIS_CACHE_ID_DOMAIN: &str = "codenoesis.analysis-cache-entry-id/rust-workspace/v1";
pub const ANALYSIS_CACHE_PAYLOAD_DOMAIN: &str =
    "codenoesis.analysis-cache-payload/rust-workspace/v1";
pub const NORMALIZATION_VERSION: &str = "codenoesis.normalization/rust-workspace/v1";
pub const EXTRACTION_CONTRACT_VERSION: &str = "codenoesis.extraction/v2";
pub const TARGET_SEMANTIC_PROFILE: &str = "standard-local-s4";
pub const DEPENDENCY_RULE_VERSION: &str = "codenoesis.incremental-rules/rust-workspace-v1";
pub const MAX_CHANGED_PATHS: usize = 100_000;
pub const MAX_ANALYSIS_ENTRIES: usize = 100_000;
pub const MAX_DEPENDENCY_EDGES: usize = 1_000_000;
pub const MAX_REPORT_SUBJECT_IDS: usize = 1_000_000;
pub const MAX_REPORT_BYTES: usize = 16_777_216;
pub const MAX_REFRESH_WALL_MILLISECONDS: u64 = 60_000;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct AnalysisCacheKey {
    pub repository_identity: String,
    pub source_file_id: String,
    pub canonical_source_path: String,
    pub source_blob_oid: String,
    pub crate_id: String,
    pub canonical_module_path: String,
    pub language_extractor: String,
    pub workspace_mapper: String,
    pub ontology: String,
}

impl AnalysisCacheKey {
    #[must_use]
    pub fn entry_id(&self) -> String {
        let bytes = canonical_string_array(&[
            ANALYSIS_CACHE_ID_DOMAIN,
            &self.repository_identity,
            &self.source_file_id,
            &self.canonical_source_path,
            &self.source_blob_oid,
            &self.crate_id,
            &self.canonical_module_path,
            ANALYSIS_CACHE_SCHEMA_VERSION,
            &self.language_extractor,
            &self.workspace_mapper,
            NORMALIZATION_VERSION,
            &self.ontology,
            EXTRACTION_CONTRACT_VERSION,
            TARGET_SEMANTIC_PROFILE,
            DEPENDENCY_RULE_VERSION,
        ]);
        format!(
            "urn:codenoesis:analysis-cache-entry:blake3:{}",
            blake3::hash(&bytes).to_hex()
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustSourceAnalysis {
    pub declarations: Vec<RustDeclarationObservation>,
    pub modules: Vec<RustModuleObservation>,
    pub imports: Vec<String>,
    pub unsupported_construct: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustDeclarationObservation {
    pub kind: EntityKind,
    pub name: String,
    pub visibility: WorkspaceVisibility,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustModuleObservation {
    pub name: String,
    pub visibility: WorkspaceVisibility,
    pub body: Option<Box<RustSourceAnalysis>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnalysisCacheEntry {
    pub analysis_cache_entry_id: String,
    pub key: AnalysisCacheKey,
    pub analysis: RustSourceAnalysis,
}

impl AnalysisCacheEntry {
    #[must_use]
    pub fn new(key: AnalysisCacheKey, analysis: RustSourceAnalysis) -> Self {
        let analysis_cache_entry_id = key.entry_id();
        Self {
            analysis_cache_entry_id,
            key,
            analysis,
        }
    }

    #[must_use]
    pub fn is_self_consistent(&self) -> bool {
        self.analysis_cache_entry_id == self.key.entry_id()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceAnalysisRecord {
    pub path: String,
    pub source_file_id: String,
    pub analysis_cache_entry_id: String,
    pub reused: bool,
    pub root: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IncrementalWorkspaceExtraction {
    pub knowledge: RustWorkspaceKnowledge,
    pub cache_entries: Vec<AnalysisCacheEntry>,
    pub source_records: Vec<SourceAnalysisRecord>,
    pub parser_invocation_count: u64,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
}

impl ChangeKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Modified => "modified",
            Self::Deleted => "deleted",
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ChangedPath {
    pub path: String,
    pub change_kind: ChangeKind,
    pub baseline_blob_oid: Option<String>,
    pub target_blob_oid: Option<String>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct InventoryBlob {
    pub path: String,
    pub blob_oid: String,
    pub mode: String,
}

/// Derives the exact sorted path/blob delta between a validated baseline and
/// one independently acquired target inventory.
///
/// # Errors
///
/// Returns [`IncrementalError::LimitExceeded`] when the fixed changed-path
/// bound cannot be respected.
pub fn diff_inventory(
    baseline: &[InventoryBlob],
    target: &RepositoryInventory,
) -> Result<Vec<ChangedPath>, IncrementalError> {
    let baseline = baseline
        .iter()
        .map(|file| {
            (
                file.path.as_str(),
                (file.blob_oid.as_str(), file.mode.as_str()),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let target = target
        .files()
        .iter()
        .map(|file| {
            (
                file.path(),
                (file.blob_oid().as_str(), file.mode().as_str()),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let paths = baseline
        .keys()
        .copied()
        .chain(target.keys().copied())
        .collect::<BTreeSet<_>>();
    let changed_paths = paths
        .into_iter()
        .filter_map(|path| match (baseline.get(path), target.get(path)) {
            (Some(baseline), Some(target)) if baseline == target => None,
            (Some((baseline_blob, _)), Some((target_blob, _))) => Some(ChangedPath {
                path: path.to_owned(),
                change_kind: ChangeKind::Modified,
                baseline_blob_oid: Some((*baseline_blob).to_owned()),
                target_blob_oid: Some((*target_blob).to_owned()),
            }),
            (Some((baseline_blob, _)), None) => Some(ChangedPath {
                path: path.to_owned(),
                change_kind: ChangeKind::Deleted,
                baseline_blob_oid: Some((*baseline_blob).to_owned()),
                target_blob_oid: None,
            }),
            (None, Some((target_blob, _))) => Some(ChangedPath {
                path: path.to_owned(),
                change_kind: ChangeKind::Added,
                baseline_blob_oid: None,
                target_blob_oid: Some((*target_blob).to_owned()),
            }),
            (None, None) => None,
        })
        .collect::<Vec<_>>();
    if changed_paths.len() > MAX_CHANGED_PATHS {
        return Err(IncrementalError::LimitExceeded {
            limit: "changed_paths",
            maximum: MAX_CHANGED_PATHS,
            observed: changed_paths.len(),
        });
    }
    Ok(changed_paths)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IncrementalRuleOutcome {
    FullRebuild,
    FullWorkspaceAnalysis,
    PartialAnalysis,
    InventoryOnly,
    NoChange,
}

impl IncrementalRuleOutcome {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FullRebuild => "full_rebuild",
            Self::FullWorkspaceAnalysis => "full_workspace_analysis",
            Self::PartialAnalysis => "partial_analysis",
            Self::InventoryOnly => "inventory_only",
            Self::NoChange => "no_change",
        }
    }

    #[must_use]
    pub const fn rule_id(self) -> &'static str {
        match self {
            Self::FullRebuild => "INC-RULE-002",
            Self::FullWorkspaceAnalysis => "INC-RULE-003",
            Self::PartialAnalysis => "INC-RULE-004",
            Self::InventoryOnly => "INC-RULE-005",
            Self::NoChange => "INC-RULE-006",
        }
    }

    #[must_use]
    pub const fn reason(self) -> &'static str {
        match self {
            Self::FullRebuild => "version_boundary_changed",
            Self::FullWorkspaceAnalysis => "workspace_dependency_closure_not_proven",
            Self::PartialAnalysis => "bounded_source_analysis_change",
            Self::InventoryOnly => "inventory_changed_without_analysis_change",
            Self::NoChange => "exact_target_already_visible",
        }
    }
}

#[must_use]
pub fn select_rule(
    same_commit: bool,
    versions_compatible: bool,
    changed_paths: &[ChangedPath],
    stable_non_root_source_paths: &BTreeSet<String>,
) -> IncrementalRuleOutcome {
    if !versions_compatible {
        return IncrementalRuleOutcome::FullRebuild;
    }
    if same_commit && changed_paths.is_empty() {
        return IncrementalRuleOutcome::NoChange;
    }
    if changed_paths.iter().any(|change| {
        analysis_content_changed(change)
            && (is_workspace_mapping_input(&change.path)
                || is_rust_source(&change.path)
                    && (change.change_kind != ChangeKind::Modified
                        || !stable_non_root_source_paths.contains(&change.path)))
    }) {
        return IncrementalRuleOutcome::FullWorkspaceAnalysis;
    }
    if changed_paths
        .iter()
        .any(|change| is_rust_source(&change.path) && analysis_content_changed(change))
    {
        IncrementalRuleOutcome::PartialAnalysis
    } else {
        IncrementalRuleOutcome::InventoryOnly
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IncrementalError {
    BaselineMissing,
    BaselineRepositoryMismatch,
    BaselineIncompatible,
    CacheCorrupt,
    LimitExceeded {
        limit: &'static str,
        maximum: usize,
        observed: usize,
    },
    ColdEquivalenceFailed,
    HeadConflict,
    Storage,
    Acquisition,
    Workspace,
    Internal,
}

impl Display for IncrementalError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::BaselineMissing => "incremental baseline missing",
            Self::BaselineRepositoryMismatch => "incremental baseline repository mismatch",
            Self::BaselineIncompatible => "incremental baseline incompatible",
            Self::CacheCorrupt => "incremental cache corrupt",
            Self::LimitExceeded { .. } => "incremental limit exceeded",
            Self::ColdEquivalenceFailed => "incremental cold equivalence failed",
            Self::HeadConflict => "incremental publication head conflict",
            Self::Storage => "incremental storage failure",
            Self::Acquisition => "incremental acquisition failure",
            Self::Workspace => "incremental workspace failure",
            Self::Internal => "incremental internal failure",
        })
    }
}

impl Error for IncrementalError {}

fn is_workspace_mapping_input(path: &str) -> bool {
    path == "Cargo.toml"
        || path.ends_with("/Cargo.toml")
        || path.ends_with("/src/lib.rs")
        || path.ends_with("/src/main.rs")
        || path.ends_with("/mod.rs")
}

fn is_rust_source(path: &str) -> bool {
    Path::new(path).extension() == Some(std::ffi::OsStr::new("rs"))
}

fn analysis_content_changed(change: &ChangedPath) -> bool {
    change.change_kind != ChangeKind::Modified || change.baseline_blob_oid != change.target_blob_oid
}

fn canonical_string_array(values: &[&str]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.push(b'[');
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            bytes.push(b',');
        }
        bytes.push(b'"');
        write_json_string(&mut bytes, value);
        bytes.push(b'"');
    }
    bytes.push(b']');
    bytes
}

fn write_json_string(bytes: &mut Vec<u8>, value: &str) {
    for character in value.chars() {
        match character {
            '"' => bytes.extend_from_slice(br#"\""#),
            '\\' => bytes.extend_from_slice(br"\\"),
            '\u{0008}' => bytes.extend_from_slice(br"\b"),
            '\u{0009}' => bytes.extend_from_slice(br"\t"),
            '\u{000A}' => bytes.extend_from_slice(br"\n"),
            '\u{000C}' => bytes.extend_from_slice(br"\f"),
            '\u{000D}' => bytes.extend_from_slice(br"\r"),
            value if value <= '\u{001F}' => {
                let _ = write!(ByteWriter(bytes), "\\u{:04x}", u32::from(value));
            }
            value => {
                let mut encoded = [0_u8; 4];
                bytes.extend_from_slice(value.encode_utf8(&mut encoded).as_bytes());
            }
        }
    }
}

struct ByteWriter<'a>(&'a mut Vec<u8>);

impl fmt::Write for ByteWriter<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.0.extend_from_slice(value.as_bytes());
        Ok(())
    }
}
