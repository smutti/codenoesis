use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use sha2::{Digest as _, Sha256};

use super::{git_command, read_repository_text, stdout_line, successful_output, unique_temp_root};

pub const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s4-rust-semantic-depth-v1";
pub const FIXTURE_TREE_OID: &str = "9f2229d9caf4efacfb8dc0eccf1e47e6bd738fef";
pub const FIXTURE_COMMIT_OID: &str = "4fc9e6efd37c289a347c9f642fdac0ef611c9fe8";

pub struct MaterializedRustSemanticRepository {
    pub root: PathBuf,
    pub worktree: PathBuf,
    pub store: PathBuf,
    pub documents: PathBuf,
    pub commit_oid: String,
}

impl MaterializedRustSemanticRepository {
    pub fn fixture() -> Self {
        let fixture = fixture_root();
        let fixture_manifest: Value = serde_json::from_slice(
            &fs::read(fixture.join("manifest.json")).expect("read R5 fixture manifest"),
        )
        .expect("parse R5 fixture manifest");
        assert_eq!(fixture_manifest["repository_identity"], REPOSITORY_ID);

        let root = unique_temp_root();
        let worktree = root.join("repository");
        let store = root.join("store");
        let documents = root.join("documents");
        let template = root.join("template");
        let global_config = root.join("global.gitconfig");
        fs::create_dir_all(&template).expect("create empty Git template directory");
        fs::write(&global_config, []).expect("create empty global Git configuration");

        let mut init = git_command(&global_config);
        init.args(["init", "--quiet", "--initial-branch=main"])
            .arg(format!("--template={}", template.display()))
            .arg(&worktree);
        successful_output(init, None);

        for file in fixture_manifest["files"]
            .as_array()
            .expect("reviewed R5 fixture files")
        {
            let fixture_path = file["path"].as_str().expect("reviewed fixture path");
            let destination = fixture_path
                .strip_prefix("repository/")
                .expect("R5 repository-relative fixture path");
            let bytes = read_repository_text(fixture.join(fixture_path));
            assert_eq!(
                u64::try_from(bytes.len()).expect("fixture byte length"),
                file["byte_length"].as_u64().expect("reviewed byte length"),
                "R5 fixture byte length changed for {fixture_path}"
            );
            assert_eq!(
                lower_hex(&Sha256::digest(&bytes)),
                file["sha256"].as_str().expect("reviewed SHA-256"),
                "R5 fixture SHA-256 changed for {fixture_path}"
            );
            let destination_path = worktree.join(destination);
            fs::create_dir_all(
                destination_path
                    .parent()
                    .expect("fixture destination parent"),
            )
            .expect("create fixture destination parent");
            fs::write(&destination_path, &bytes).expect("materialize R5 fixture worktree file");

            let mut hash = git_command(&global_config);
            hash.arg("-C")
                .arg(&worktree)
                .args(["hash-object", "-w", "--stdin"]);
            let blob_oid = stdout_line(successful_output(hash, Some(&bytes)));
            assert_eq!(
                blob_oid,
                file["git_blob_oid"].as_str().expect("reviewed Git blob"),
                "R5 fixture Git blob changed for {fixture_path}"
            );
            update_index(&worktree, &global_config, "100644", &blob_oid, destination);
        }

        let mut write_tree = git_command(&global_config);
        write_tree.arg("-C").arg(&worktree).arg("write-tree");
        let tree_oid = stdout_line(successful_output(write_tree, None));
        assert_eq!(tree_oid, FIXTURE_TREE_OID, "R5 fixture tree changed");
        let mut make_commit = git_command(&global_config);
        make_commit
            .arg("-C")
            .arg(&worktree)
            .args(["commit-tree", &tree_oid, "-F", "-"])
            .env("GIT_AUTHOR_NAME", "CodeNoesis")
            .env("GIT_AUTHOR_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_AUTHOR_DATE", "1785801600 +0000")
            .env("GIT_COMMITTER_NAME", "CodeNoesis")
            .env("GIT_COMMITTER_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_COMMITTER_DATE", "1785801600 +0000");
        let commit_oid = stdout_line(successful_output(
            make_commit,
            Some(b"R5 project-owned semantic-depth fixture\n"),
        ));
        assert_eq!(commit_oid, FIXTURE_COMMIT_OID, "R5 fixture commit changed");
        let mut update_ref = git_command(&global_config);
        update_ref
            .arg("-C")
            .arg(&worktree)
            .args(["update-ref", "refs/heads/main", &commit_oid]);
        successful_output(update_ref, None);

        Self {
            root,
            worktree,
            store,
            documents,
            commit_oid,
        }
    }

    pub fn scan(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .args(["scan", "--repository"])
            .arg(&self.worktree)
            .args(["--repository-id", REPOSITORY_ID, "--revision"])
            .arg(&self.commit_oid)
            .args([
                "--profile",
                "standard-local-s4",
                "--workspace-profile",
                "cargo-root-package-v1",
                "--manifest-profile",
                "cargo-manifest-facts-v1",
                "--rust-semantic-profile",
                "rust-semantic-depth-v1",
            ])
            .arg("--store")
            .arg(&self.store)
            .args(["--format", "json"])
            .output()
            .expect("launch R5 Rust semantic-depth subject")
    }

    pub fn build_sentinel(&self) -> PathBuf {
        self.worktree.join("BUILD_SENTINEL_EXECUTED")
    }
}

impl Drop for MaterializedRustSemanticRepository {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/s4/rust-semantic-depth-v1")
}

fn update_index(worktree: &Path, global_config: &Path, mode: &str, oid: &str, path: &str) {
    let mut update = git_command(global_config);
    update
        .arg("-C")
        .arg(worktree)
        .args(["update-index", "--add", "--cacheinfo"])
        .arg(format!("{mode},{oid},{path}"));
    successful_output(update, None);
}

fn lower_hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}").expect("write digest");
    }
    encoded
}
