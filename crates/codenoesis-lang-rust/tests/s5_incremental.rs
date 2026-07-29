use std::fs;
use std::path::Path;

use codenoesis_domain::{
    AcquiredFile, AcquiredRepository, BoundRevision, ObjectId, RegularFileMode, RepositoryIdentity,
    RepositoryInventory,
};
use codenoesis_lang_rust::TreeSitterRustWorkspaceExtractor;
use codenoesis_ports::{IncrementalRustWorkspaceExtractor, RustWorkspaceExtractor};

const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s5-incremental-refresh-v1";
const BASELINE_COMMIT: &str = "2106dc8bf32867e519b89ef6d73ce2afdced170d";
const BASELINE_TREE: &str = "36d30c925a7bb512acd0795c97e50f5b46388cb7";
const TARGET_COMMIT: &str = "c2408412192e61249403bfa1dbc2dce7db0364cc";
const TARGET_TREE: &str = "7e5c687082b25274ab003121b656bafbfeb0fbd0";

#[test]
fn pt_fr_inc_003_only_changed_source_is_reparsed() {
    let extractor = TreeSitterRustWorkspaceExtractor::new();
    let baseline = extractor
        .extract_workspace_incremental(&fixture_inventory("revision-a"), &[])
        .expect("extract S5 baseline analyses");
    assert_eq!(baseline.parser_invocation_count, 3);
    assert_eq!(baseline.cache_entries.len(), 3);

    let target_inventory = fixture_inventory("revision-b");
    let incremental = extractor
        .extract_workspace_incremental(&target_inventory, &baseline.cache_entries)
        .expect("extract S5 target incrementally");
    let cold = extractor
        .extract_workspace(&target_inventory)
        .expect("extract S5 target cold");

    assert_eq!(incremental.parser_invocation_count, 1);
    assert_eq!(
        incremental
            .source_records
            .iter()
            .filter(|record| record.reused)
            .count(),
        2
    );
    assert_eq!(incremental.knowledge, cold);
    assert_eq!(
        incremental
            .cache_entries
            .iter()
            .map(|entry| entry.analysis_cache_entry_id.as_str())
            .collect::<Vec<_>>(),
        [
            "urn:codenoesis:analysis-cache-entry:blake3:4f36ea35e62ff1e1547354d63e37f1dc842986498267ce92f7fa254b6f6b41ab",
            "urn:codenoesis:analysis-cache-entry:blake3:5b73dc45043df2725e5cee134272a4e4da33ec099a361fd49d8e5ff8676effd0",
            "urn:codenoesis:analysis-cache-entry:blake3:ca46085d6bc162e2ec49c6002ae86f60014c26d38eeb115456147ff65e058ec9",
        ]
    );
}

#[test]
fn pt_fr_inc_003_cache_order_does_not_change_target() {
    let extractor = TreeSitterRustWorkspaceExtractor::new();
    let baseline = extractor
        .extract_workspace_incremental(&fixture_inventory("revision-a"), &[])
        .expect("extract ordered S5 baseline");
    let target = fixture_inventory("revision-b");
    let cold = extractor
        .extract_workspace(&target)
        .expect("extract ordered S5 cold target");
    let mut cache_entries = baseline.cache_entries;
    let entry_count = cache_entries.len();

    for schedule in 0..20 {
        cache_entries.rotate_left(schedule % entry_count);
        let incremental = extractor
            .extract_workspace_incremental(&target, &cache_entries)
            .expect("extract shuffled S5 target");
        assert_eq!(incremental.parser_invocation_count, 1);
        assert_eq!(incremental.knowledge, cold);
        assert_eq!(
            incremental
                .source_records
                .iter()
                .filter(|record| record.reused)
                .count(),
            2
        );
    }
}

fn fixture_inventory(revision: &str) -> RepositoryInventory {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/s5/incremental-refresh-v1")
        .join(revision);
    let changed_item = revision == "revision-b";
    let files = [
        ("Cargo.toml", "9a61bc964dd0b80e54880d0a40471b50324b6e15"),
        (
            "crates/app/Cargo.toml",
            "887b09de92b06c4003daf2ffc60a48e33aa1187f",
        ),
        (
            "crates/app/build.rs",
            "23984539c6aaa35586c06b4b01e14e20eed817a9",
        ),
        (
            "crates/app/src/main.rs",
            "7e6f91e233eff28c5511d686f7c46f67dd919505",
        ),
        (
            "crates/model/Cargo.toml",
            "bd9cc68a4885e014ce4a034be8c2317425f3e3dc",
        ),
        (
            "crates/model/src/item.rs",
            if changed_item {
                "ab58a9d6417ab6852d5311994e480fba6185002f"
            } else {
                "885b4746097a67e5c4fb997a2082597dc23699e6"
            },
        ),
        (
            "crates/model/src/lib.rs",
            "f001952aa75e3c631153f1fe5dec0af6ca11c429",
        ),
    ]
    .into_iter()
    .map(|(path, oid)| {
        AcquiredFile::new(
            path.to_owned(),
            RegularFileMode::Regular,
            ObjectId::parse_sha1(oid).expect("S5 fixture blob OID"),
            fs::read(root.join(path)).expect("S5 fixture source"),
        )
    })
    .collect();
    let (commit, tree) = if changed_item {
        (TARGET_COMMIT, TARGET_TREE)
    } else {
        (BASELINE_COMMIT, BASELINE_TREE)
    };
    RepositoryInventory::classify(AcquiredRepository::new(
        BoundRevision::new(
            RepositoryIdentity::parse(REPOSITORY_ID).expect("S5 fixture repository ID"),
            ObjectId::parse_sha1(commit).expect("S5 fixture commit OID"),
            ObjectId::parse_sha1(tree).expect("S5 fixture tree OID"),
        ),
        5,
        files,
    ))
}
