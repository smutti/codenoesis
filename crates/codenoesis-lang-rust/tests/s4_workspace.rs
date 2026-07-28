use std::fs;
use std::path::Path;

use codenoesis_domain::knowledge::EntityKind;
use codenoesis_domain::s4::{MAX_S4_WORKSPACE_CRATES, WorkspaceError};
use codenoesis_domain::{
    AcquiredFile, AcquiredRepository, BoundRevision, ObjectId, RegularFileMode, RepositoryIdentity,
    RepositoryInventory,
};
use codenoesis_lang_rust::TreeSitterRustWorkspaceExtractor;
use codenoesis_ports::RustWorkspaceExtractor;

const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s4-workspace-docs-v1";
const COMMIT_OID: &str = "c09d8c24e4704036c31b4f42e2f4df6e4acd347f";
const TREE_OID: &str = "8f9e36122bec5caac5dc0f739ea7ab4c830bd356";

#[test]
fn gt_fr_ext_007_workspace_graph_v2() {
    let inventory = fixture_inventory();
    let knowledge = TreeSitterRustWorkspaceExtractor::new()
        .extract_workspace(&inventory)
        .expect("extract reviewed S4 workspace");

    assert_eq!(knowledge.graph.entities.len(), 12);
    assert_eq!(knowledge.graph.relationships.len(), 12);
    assert_eq!(knowledge.graph.claims.len(), 24);
    assert_eq!(knowledge.graph.evidence.len(), 6);
    assert_eq!(knowledge.graph.diagnostics.len(), 1);
    assert_eq!(knowledge.graph.coverage.len(), 2);
    assert_eq!(knowledge.extraction_chunks.len(), 3);
    assert!(knowledge.graph.entities.iter().any(|entity| {
        entity.id
            == "urn:codenoesis:entity:blake3:18bd97153a41f322136e8f93573877bfc1a2f43fabc6564b789e979a6cffcafa"
    }));
}

#[test]
fn gt_fr_ext_007_inline_modules_and_multiple_targets_are_supported() {
    let inventory = inventory_from_files(&[
        (
            "Cargo.toml",
            b"[workspace]\nmembers = [\"member\"]\nresolver = \"3\"\n",
        ),
        (
            "member/Cargo.toml",
            b"[package]\nname = \"multi\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[lib]\nname = \"multi\"\npath = \"src/lib.rs\"\n\n[[bin]]\nname = \"multi-cli\"\npath = \"src/main.rs\"\n",
        ),
        (
            "member/src/lib.rs",
            b"pub mod inline { pub struct Thing; }\n",
        ),
        ("member/src/main.rs", b"fn main() {}\n"),
    ]);

    let knowledge = TreeSitterRustWorkspaceExtractor::new()
        .extract_workspace(&inventory)
        .expect("extract supported multi-target inline workspace");

    assert_eq!(
        knowledge
            .graph
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::RustCrate)
            .count(),
        2
    );
    assert!(knowledge.graph.entities.iter().any(|entity| {
        entity.kind == EntityKind::RustModule
            && entity.module_path.as_deref() == Some("crate::inline")
    }));
    assert_eq!(knowledge.extraction_chunks.len(), 2);
}

#[test]
fn gt_fr_ext_007_requires_literal_supported_manifest_metadata() {
    let missing_version = inventory_from_files(&[
        (
            "Cargo.toml",
            b"[workspace]\nmembers = [\"member\"]\nresolver = \"3\"\n",
        ),
        (
            "member/Cargo.toml",
            b"[package]\nname = \"member\"\nedition = \"2024\"\n",
        ),
        ("member/src/lib.rs", b"pub struct Item;\n"),
    ]);
    assert_eq!(
        TreeSitterRustWorkspaceExtractor::new().extract_workspace(&missing_version),
        Err(WorkspaceError::UnsupportedWorkspace)
    );

    let member_glob = inventory_from_files(&[
        (
            "Cargo.toml",
            b"[workspace]\nmembers = [\"member?\"]\nresolver = \"3\"\n",
        ),
        (
            "member?/Cargo.toml",
            b"[package]\nname = \"member\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        ),
        ("member?/src/lib.rs", b"pub struct Item;\n"),
    ]);
    assert_eq!(
        TreeSitterRustWorkspaceExtractor::new().extract_workspace(&member_glob),
        Err(WorkspaceError::UnsupportedWorkspace)
    );
}

#[test]
fn pt_dr_idn_002_normalization_collisions_fail_closed() {
    let inventory = inventory_from_files(&[
        (
            "Cargo.toml",
            b"[workspace]\nmembers = [\"member\"]\nresolver = \"3\"\n",
        ),
        (
            "member/Cargo.toml",
            b"[package]\nname = \"member\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        ),
        (
            "member/src/lib.rs",
            "pub struct Café;\npub struct Cafe\u{301};\n".as_bytes(),
        ),
    ]);

    assert_eq!(
        TreeSitterRustWorkspaceExtractor::new().extract_workspace(&inventory),
        Err(WorkspaceError::ContractInvalid)
    );
}

#[test]
fn fr_ext_007_workspace_crate_limit_plus_one_is_typed() {
    let members = (0..=MAX_S4_WORKSPACE_CRATES)
        .map(|ordinal| format!("\"member-{ordinal}\""))
        .collect::<Vec<_>>()
        .join(",");
    let manifest = format!("[workspace]\nmembers = [{members}]\nresolver = \"3\"\n");
    let inventory = inventory_from_files(&[("Cargo.toml", manifest.as_bytes())]);

    assert_eq!(
        TreeSitterRustWorkspaceExtractor::new().extract_workspace(&inventory),
        Err(WorkspaceError::LimitExceeded {
            limit: "workspace_crates",
            maximum: 200,
            observed: 201,
        })
    );
}

fn fixture_inventory() -> RepositoryInventory {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/s4/workspace-docs-v1/revision-a");
    let files = [
        ("Cargo.toml", "9a61bc964dd0b80e54880d0a40471b50324b6e15"),
        (
            "crates/app/Cargo.toml",
            "887b09de92b06c4003daf2ffc60a48e33aa1187f",
        ),
        (
            "crates/app/build.rs",
            "817cc01b6a06743e810217ca57f9abfbbdf34824",
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
            "885b4746097a67e5c4fb997a2082597dc23699e6",
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
            ObjectId::parse_sha1(oid).expect("fixture blob OID"),
            fs::read(root.join(path)).expect("fixture source"),
        )
    })
    .collect();
    RepositoryInventory::classify(AcquiredRepository::new(
        BoundRevision::new(
            RepositoryIdentity::parse(REPOSITORY_ID).expect("fixture repository ID"),
            ObjectId::parse_sha1(COMMIT_OID).expect("fixture commit OID"),
            ObjectId::parse_sha1(TREE_OID).expect("fixture tree OID"),
        ),
        5,
        files,
    ))
}

fn inventory_from_files(files: &[(&str, &[u8])]) -> RepositoryInventory {
    let files = files
        .iter()
        .enumerate()
        .map(|(index, (path, bytes))| {
            AcquiredFile::new(
                (*path).to_owned(),
                RegularFileMode::Regular,
                ObjectId::parse_sha1(&format!("{:040x}", index + 1)).expect("synthetic blob OID"),
                bytes.to_vec(),
            )
        })
        .collect();
    RepositoryInventory::classify(AcquiredRepository::new(
        BoundRevision::new(
            RepositoryIdentity::parse("urn:codenoesis:test:s4-workspace")
                .expect("synthetic repository ID"),
            ObjectId::parse_sha1(COMMIT_OID).expect("fixture commit OID"),
            ObjectId::parse_sha1(TREE_OID).expect("fixture tree OID"),
        ),
        5,
        files,
    ))
}
