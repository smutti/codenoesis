use std::collections::{BTreeMap, BTreeSet};

use codenoesis_contracts::{RepositorySnapshotV4, ValidatedS4Head};
use codenoesis_domain::s4::WorkspaceError;
use codenoesis_domain::s5::{
    AnalysisCacheEntry, ChangedPath, IncrementalError, IncrementalRuleOutcome,
    IncrementalWorkspaceExtraction, MAX_ANALYSIS_ENTRIES, RustSourceAnalysis, diff_inventory,
    select_rule,
};
use codenoesis_domain::{
    AcquisitionError, RepositoryError, RepositoryIdentity, RepositoryInventory, Revision,
};
use codenoesis_ports::{IncrementalRustWorkspaceExtractor, SafeRepositoryAcquirer};

use crate::ScanRequest;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RefreshError {
    Acquisition(AcquisitionError),
    Workspace(WorkspaceError),
    BaselineRepositoryMismatch,
    BaselineIncompatible,
    ColdEquivalenceFailed {
        expected_hash: String,
        observed_hash: String,
    },
    LimitExceeded {
        limit: &'static str,
        maximum: u64,
        observed: u64,
    },
    Internal,
}

pub struct RefreshPlan {
    pub baseline_cache_entries: Vec<AnalysisCacheEntry>,
    pub target_snapshot: RepositorySnapshotV4,
    pub target_extraction: IncrementalWorkspaceExtraction,
    pub changed_paths: Vec<ChangedPath>,
    pub rule: IncrementalRuleOutcome,
}

pub struct RefreshService<A> {
    acquirer: A,
}

impl<A> RefreshService<A>
where
    A: SafeRepositoryAcquirer,
{
    #[must_use]
    pub const fn new(acquirer: A) -> Self {
        Self { acquirer }
    }

    /// Creates one deterministic target plan against the exact validated
    /// visible S4 baseline.
    ///
    /// # Errors
    ///
    /// Returns a typed acquisition, baseline, limit, or workspace failure.
    #[allow(clippy::too_many_lines)]
    pub fn plan<E>(
        &self,
        request: ScanRequest,
        baseline: &ValidatedS4Head,
        cache_entries: &[AnalysisCacheEntry],
        versions_compatible: bool,
        extractor: &E,
    ) -> Result<RefreshPlan, RefreshError>
    where
        E: IncrementalRustWorkspaceExtractor,
    {
        if baseline.head().repository_identity != request.identity {
            return Err(RefreshError::BaselineRepositoryMismatch);
        }
        if baseline.head().snapshot_schema_version != "codenoesis.repository-snapshot/v4" {
            return Err(RefreshError::BaselineIncompatible);
        }
        if cache_entries.len() > MAX_ANALYSIS_ENTRIES {
            return Err(RefreshError::LimitExceeded {
                limit: "analysis_entries",
                maximum: MAX_ANALYSIS_ENTRIES as u64,
                observed: u64::try_from(cache_entries.len()).unwrap_or(u64::MAX),
            });
        }
        let baseline_extraction = if versions_compatible {
            let baseline_revision = Revision::parse(baseline.head().commit_oid.as_str())
                .map_err(|_| RefreshError::BaselineIncompatible)?;
            let baseline_acquired = self
                .acquirer
                .acquire_inventory(
                    &request.repository,
                    RepositoryIdentity::parse(baseline.head().repository_identity.as_str())
                        .map_err(|_| RefreshError::BaselineIncompatible)?,
                    baseline_revision,
                )
                .map_err(|error| match error {
                    RepositoryError::Acquisition(acquisition) => {
                        RefreshError::Acquisition(acquisition)
                    }
                    RepositoryError::Unexpected => RefreshError::Internal,
                })?;
            let baseline_inventory = RepositoryInventory::classify(baseline_acquired);
            let extraction = extractor
                .extract_workspace_incremental(&baseline_inventory, cache_entries)
                .map_err(RefreshError::Workspace)?;
            extraction
                .knowledge
                .validate()
                .map_err(RefreshError::Workspace)?;
            let reconstructed_baseline = RepositorySnapshotV4::from_inventory_and_workspace(
                &baseline_inventory,
                &extraction.knowledge,
                request.envelope.clone(),
            );
            if reconstructed_baseline.value().get("semantic") != Some(baseline.semantic()) {
                return Err(RefreshError::ColdEquivalenceFailed {
                    expected_hash: baseline.head().semantic_hash.value.clone(),
                    observed_hash: reconstructed_baseline
                        .value()
                        .pointer("/semantic_hash/value")
                        .and_then(|value| value.as_str())
                        .unwrap_or("invalid")
                        .to_owned(),
                });
            }
            Some(extraction)
        } else {
            None
        };
        let baseline_parser_invocation_count = baseline_extraction
            .as_ref()
            .map_or(0, |extraction| extraction.parser_invocation_count);
        let baseline_cache_entries = baseline_extraction.as_ref().map_or_else(
            || cache_entries.to_vec(),
            |extraction| extraction.cache_entries.clone(),
        );
        let acquired = self
            .acquirer
            .acquire_inventory(&request.repository, request.identity, request.revision)
            .map_err(|error| match error {
                RepositoryError::Acquisition(acquisition) => RefreshError::Acquisition(acquisition),
                RepositoryError::Unexpected => RefreshError::Internal,
            })?;
        let target_inventory = RepositoryInventory::classify(acquired);
        let baseline_inventory_blobs = baseline
            .inventory_blobs()
            .map_err(|_| RefreshError::BaselineIncompatible)?;
        let changed_paths =
            diff_inventory(&baseline_inventory_blobs, &target_inventory).map_err(|error| {
                match error {
                    IncrementalError::LimitExceeded {
                        limit,
                        maximum,
                        observed,
                    } => RefreshError::LimitExceeded {
                        limit,
                        maximum: u64::try_from(maximum).unwrap_or(u64::MAX),
                        observed: u64::try_from(observed).unwrap_or(u64::MAX),
                    },
                    _ => RefreshError::Internal,
                }
            })?;
        let same_commit =
            baseline.head().commit_oid == *target_inventory.bound_revision().commit_oid();
        let mut extraction = if same_commit && versions_compatible {
            baseline_extraction.ok_or(RefreshError::Internal)?
        } else {
            extractor
                .extract_workspace_incremental(
                    &target_inventory,
                    if versions_compatible {
                        &baseline_cache_entries
                    } else {
                        &[]
                    },
                )
                .map_err(RefreshError::Workspace)?
        };
        extraction.parser_invocation_count = extraction
            .parser_invocation_count
            .checked_add(if same_commit || !versions_compatible {
                0
            } else {
                baseline_parser_invocation_count
            })
            .ok_or(RefreshError::Internal)?;
        let baseline_by_path = baseline_cache_entries
            .iter()
            .filter(|entry| {
                entry.key.repository_identity == baseline.head().repository_identity.as_str()
            })
            .map(|entry| (entry.key.canonical_source_path.as_str(), entry))
            .collect::<BTreeMap<_, _>>();
        let target_by_id = extraction
            .cache_entries
            .iter()
            .map(|entry| (entry.analysis_cache_entry_id.as_str(), &entry.key))
            .collect::<BTreeMap<_, _>>();
        let target_by_path = extraction
            .cache_entries
            .iter()
            .map(|entry| (entry.key.canonical_source_path.as_str(), entry))
            .collect::<BTreeMap<_, _>>();
        let stable_non_root = extraction
            .source_records
            .iter()
            .filter(|record| !record.root)
            .filter_map(|record| {
                let target = target_by_id.get(record.analysis_cache_entry_id.as_str())?;
                let baseline = &baseline_by_path.get(record.path.as_str())?.key;
                (baseline.repository_identity == target.repository_identity
                    && baseline.source_file_id == target.source_file_id
                    && baseline.canonical_source_path == target.canonical_source_path
                    && baseline.crate_id == target.crate_id
                    && baseline.canonical_module_path == target.canonical_module_path
                    && baseline.language_extractor == target.language_extractor
                    && baseline.workspace_mapper == target.workspace_mapper
                    && baseline.ontology == target.ontology)
                    .then(|| record.path.clone())
            })
            .collect::<BTreeSet<_>>();
        let module_mapping_changed = changed_paths.iter().any(|change| {
            baseline_by_path
                .get(change.path.as_str())
                .zip(target_by_path.get(change.path.as_str()))
                .is_some_and(|(baseline, target)| {
                    !same_module_mapping(&baseline.analysis, &target.analysis)
                })
        });
        let rule = if !versions_compatible {
            select_rule(same_commit, false, &changed_paths, &stable_non_root)
        } else if module_mapping_changed {
            IncrementalRuleOutcome::FullWorkspaceAnalysis
        } else {
            select_rule(same_commit, true, &changed_paths, &stable_non_root)
        };
        if rule == IncrementalRuleOutcome::FullWorkspaceAnalysis
            || rule == IncrementalRuleOutcome::FullRebuild && versions_compatible
        {
            let prior_parser_invocation_count = extraction.parser_invocation_count;
            extraction = extractor
                .extract_workspace_incremental(&target_inventory, &[])
                .map_err(RefreshError::Workspace)?;
            extraction.parser_invocation_count = extraction
                .parser_invocation_count
                .checked_add(prior_parser_invocation_count)
                .ok_or(RefreshError::Internal)?;
        }
        extraction
            .knowledge
            .validate()
            .map_err(RefreshError::Workspace)?;
        if extraction.cache_entries.len() > MAX_ANALYSIS_ENTRIES {
            return Err(RefreshError::LimitExceeded {
                limit: "analysis_entries",
                maximum: MAX_ANALYSIS_ENTRIES as u64,
                observed: u64::try_from(extraction.cache_entries.len()).unwrap_or(u64::MAX),
            });
        }
        Ok(RefreshPlan {
            baseline_cache_entries,
            target_snapshot: RepositorySnapshotV4::from_inventory_and_workspace(
                &target_inventory,
                &extraction.knowledge,
                request.envelope,
            ),
            target_extraction: extraction,
            changed_paths,
            rule,
        })
    }
}

fn same_module_mapping(left: &RustSourceAnalysis, right: &RustSourceAnalysis) -> bool {
    left.modules.len() == right.modules.len()
        && left
            .modules
            .iter()
            .zip(&right.modules)
            .all(|(left, right)| {
                left.name == right.name
                    && left.visibility == right.visibility
                    && match (&left.body, &right.body) {
                        (Some(left), Some(right)) => same_module_mapping(left, right),
                        (None, None) => true,
                        (Some(_), None) | (None, Some(_)) => false,
                    }
            })
}

#[cfg(test)]
mod tests {
    use codenoesis_domain::knowledge::EntityKind;
    use codenoesis_domain::s4::WorkspaceVisibility;
    use codenoesis_domain::s5::{
        RustDeclarationObservation, RustModuleObservation, RustSourceAnalysis,
    };

    use super::same_module_mapping;

    #[test]
    fn pt_fr_inc_002_module_mapping_changes_force_stronger_scope() {
        let baseline = source_with_module("child");
        let mut declaration_only = baseline.clone();
        declaration_only.modules[0]
            .body
            .as_mut()
            .expect("inline S5 module")
            .declarations
            .push(RustDeclarationObservation {
                kind: EntityKind::RustFunction,
                name: "added".to_owned(),
                visibility: WorkspaceVisibility::Private,
            });
        assert!(same_module_mapping(&baseline, &declaration_only));

        let renamed = source_with_module("renamed");
        assert!(!same_module_mapping(&baseline, &renamed));
    }

    fn source_with_module(name: &str) -> RustSourceAnalysis {
        RustSourceAnalysis {
            declarations: Vec::new(),
            modules: vec![RustModuleObservation {
                name: name.to_owned(),
                visibility: WorkspaceVisibility::Private,
                body: Some(Box::new(RustSourceAnalysis {
                    declarations: Vec::new(),
                    modules: Vec::new(),
                    imports: Vec::new(),
                    unsupported_construct: false,
                })),
            }],
            imports: Vec::new(),
            unsupported_construct: false,
        }
    }
}
