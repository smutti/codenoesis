use std::collections::BTreeSet;

use codenoesis_domain::s4::{
    S4_ONTOLOGY_VERSION, S4_TREE_SITTER_EXTRACTOR_VERSION, S4_WORKSPACE_EXTRACTOR_VERSION,
};
use codenoesis_domain::s5::{
    AnalysisCacheKey, ChangeKind, ChangedPath, IncrementalError, IncrementalRuleOutcome,
    InventoryBlob, MAX_CHANGED_PATHS, diff_inventory, select_rule,
};
use codenoesis_domain::{
    AcquiredFile, AcquiredRepository, BoundRevision, ObjectId, RegularFileMode, RepositoryIdentity,
    RepositoryInventory,
};

#[test]
fn pt_fr_inc_003_revision_neutral_cache_identity() {
    let key = AnalysisCacheKey {
        repository_identity: "urn:codenoesis:fixture:s5-incremental-refresh-v1".to_owned(),
        source_file_id:
            "urn:codenoesis:entity:blake3:06ca422bf17dbd703230b379e9a58fc14d963c2bcba7aa6b32931fc3ee02a863"
                .to_owned(),
        canonical_source_path: "crates/app/src/main.rs".to_owned(),
        source_blob_oid: "7e6f91e233eff28c5511d686f7c46f67dd919505".to_owned(),
        crate_id:
            "urn:codenoesis:entity:blake3:77bddda9b2183bca8f81217a8bc807d7cdf98274214a55492087df074a60ce21"
                .to_owned(),
        canonical_module_path: "crate".to_owned(),
        language_extractor: S4_TREE_SITTER_EXTRACTOR_VERSION.to_owned(),
        workspace_mapper: S4_WORKSPACE_EXTRACTOR_VERSION.to_owned(),
        ontology: S4_ONTOLOGY_VERSION.to_owned(),
    };
    assert_eq!(
        key.entry_id(),
        "urn:codenoesis:analysis-cache-entry:blake3:4f36ea35e62ff1e1547354d63e37f1dc842986498267ce92f7fa254b6f6b41ab"
    );
}

#[test]
fn pt_fr_inc_002_versioned_rule_precedence() {
    let changed = vec![ChangedPath {
        path: "crates/model/src/item.rs".to_owned(),
        change_kind: ChangeKind::Modified,
        baseline_blob_oid: Some("885b4746097a67e5c4fb997a2082597dc23699e6".to_owned()),
        target_blob_oid: Some("ab58a9d6417ab6852d5311994e480fba6185002f".to_owned()),
    }];
    let stable = BTreeSet::from(["crates/model/src/item.rs".to_owned()]);
    assert_eq!(
        select_rule(false, true, &changed, &stable),
        IncrementalRuleOutcome::PartialAnalysis
    );
    assert_eq!(
        select_rule(false, false, &changed, &stable),
        IncrementalRuleOutcome::FullRebuild
    );
    assert_eq!(
        select_rule(true, true, &[], &BTreeSet::new()),
        IncrementalRuleOutcome::NoChange
    );
}

#[test]
fn pt_fr_inc_002_inventory_only_excludes_source_and_mapping_inputs() {
    for change_kind in [ChangeKind::Added, ChangeKind::Modified, ChangeKind::Deleted] {
        let inventory_change = vec![ChangedPath {
            path: "README.md".to_owned(),
            change_kind,
            baseline_blob_oid: Some("a".repeat(40)),
            target_blob_oid: Some("b".repeat(40)),
        }];
        assert_eq!(
            select_rule(false, true, &inventory_change, &BTreeSet::new()),
            IncrementalRuleOutcome::InventoryOnly
        );
    }

    for (path, change_kind) in [
        ("crates/model/src/new.rs", ChangeKind::Added),
        ("crates/model/src/item.rs", ChangeKind::Deleted),
        ("crates/model/src/lib.rs", ChangeKind::Modified),
        ("crates/model/Cargo.toml", ChangeKind::Modified),
    ] {
        let workspace_change = vec![ChangedPath {
            path: path.to_owned(),
            change_kind,
            baseline_blob_oid: Some("a".repeat(40)),
            target_blob_oid: Some("b".repeat(40)),
        }];
        assert_eq!(
            select_rule(false, true, &workspace_change, &BTreeSet::new()),
            IncrementalRuleOutcome::FullWorkspaceAnalysis,
            "{path} did not select the conservative workspace rule"
        );
    }
}

#[test]
fn pt_fr_inc_002_version_rebuild_matrix() {
    let changed = vec![ChangedPath {
        path: "crates/model/src/item.rs".to_owned(),
        change_kind: ChangeKind::Modified,
        baseline_blob_oid: Some("885b4746097a67e5c4fb997a2082597dc23699e6".to_owned()),
        target_blob_oid: Some("ab58a9d6417ab6852d5311994e480fba6185002f".to_owned()),
    }];
    let stable = BTreeSet::from(["crates/model/src/item.rs".to_owned()]);
    for boundary in [
        "language_extractor",
        "workspace_mapper",
        "normalization",
        "ontology",
        "extraction_contract",
        "semantic_profile",
        "dependency_rules",
        "cache_schema",
        "snapshot_or_public_schema",
    ] {
        assert_eq!(
            select_rule(false, false, &changed, &stable),
            IncrementalRuleOutcome::FullRebuild,
            "version boundary {boundary} did not force a full rebuild"
        );
    }
}

#[test]
fn pt_fr_inc_001_changed_path_limit_boundaries() {
    let mut baseline = (0..=MAX_CHANGED_PATHS)
        .map(|ordinal| InventoryBlob {
            path: format!("inventory/file-{ordinal:06}.txt"),
            blob_oid: "a".repeat(40),
            mode: "100644".to_owned(),
        })
        .collect::<Vec<_>>();
    let target = empty_inventory();

    assert_eq!(
        diff_inventory(&baseline, &target),
        Err(IncrementalError::LimitExceeded {
            limit: "changed_paths",
            maximum: MAX_CHANGED_PATHS,
            observed: MAX_CHANGED_PATHS + 1,
        })
    );
    baseline.pop();
    let maximum = diff_inventory(&baseline, &target).expect("maximum changed-path set");
    assert_eq!(maximum.len(), MAX_CHANGED_PATHS);
}

#[test]
fn pt_fr_inc_001_mode_only_change_is_modified() {
    let blob_oid = "a".repeat(40);
    let baseline = vec![InventoryBlob {
        path: "script.sh".to_owned(),
        blob_oid: blob_oid.clone(),
        mode: "100644".to_owned(),
    }];
    let target = RepositoryInventory::classify(AcquiredRepository::new(
        BoundRevision::new(
            RepositoryIdentity::parse("urn:codenoesis:fixture:s5-mode-v1")
                .expect("S5 mode repository identity"),
            ObjectId::parse_sha1("1111111111111111111111111111111111111111")
                .expect("S5 mode commit"),
            ObjectId::parse_sha1("2222222222222222222222222222222222222222").expect("S5 mode tree"),
        ),
        0,
        vec![AcquiredFile::new(
            "script.sh".to_owned(),
            RegularFileMode::Executable,
            ObjectId::parse_sha1(&blob_oid).expect("S5 mode blob"),
            b"#!/bin/sh\n".to_vec(),
        )],
    ));

    assert_eq!(
        diff_inventory(&baseline, &target).expect("S5 mode-only diff"),
        vec![ChangedPath {
            path: "script.sh".to_owned(),
            change_kind: ChangeKind::Modified,
            baseline_blob_oid: Some(blob_oid.clone()),
            target_blob_oid: Some(blob_oid.clone()),
        }]
    );
    assert_eq!(
        select_rule(
            false,
            true,
            &[ChangedPath {
                path: "crates/model/src/item.rs".to_owned(),
                change_kind: ChangeKind::Modified,
                baseline_blob_oid: Some(blob_oid.clone()),
                target_blob_oid: Some(blob_oid),
            }],
            &BTreeSet::from(["crates/model/src/item.rs".to_owned()]),
        ),
        IncrementalRuleOutcome::InventoryOnly
    );
}

fn empty_inventory() -> RepositoryInventory {
    RepositoryInventory::classify(AcquiredRepository::new(
        BoundRevision::new(
            RepositoryIdentity::parse("urn:codenoesis:fixture:s5-limit-v1")
                .expect("S5 limit repository identity"),
            ObjectId::parse_sha1("1111111111111111111111111111111111111111")
                .expect("S5 limit commit"),
            ObjectId::parse_sha1("2222222222222222222222222222222222222222")
                .expect("S5 limit tree"),
        ),
        0,
        Vec::new(),
    ))
}
