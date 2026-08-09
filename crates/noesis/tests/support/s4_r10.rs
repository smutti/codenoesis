use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use sha2::{Digest as _, Sha256};

use super::{git_command, read_repository_text, stdout_line, successful_output, unique_temp_root};

pub const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s4-rust-cfg-declaration-alternatives-v1";
pub const FIXTURE_TREE_OID: &str = "6aa31d889f4c87b2b7dfbff3fef3b32ee7fa0363";
pub const FIXTURE_COMMIT_OID: &str = "d5a44bb5bb12ddb6f71ea4dd0c88944dc41eefec";
pub const R10_PROFILE: &str = "rust-cfg-declaration-alternatives-v1";

pub struct MaterializedCfgAlternativesRepository {
    pub root: PathBuf,
    pub worktree: PathBuf,
    pub store: PathBuf,
    pub documents: PathBuf,
    pub portable: PathBuf,
    pub explorer: PathBuf,
    pub commit_oid: String,
}

impl MaterializedCfgAlternativesRepository {
    pub fn fixture() -> Self {
        let fixture = fixture_root();
        let manifest: Value = serde_json::from_slice(
            &fs::read(fixture.join("manifest.json")).expect("read R10 fixture manifest"),
        )
        .expect("parse R10 fixture manifest");
        assert_eq!(manifest["repository_identity"], REPOSITORY_ID);

        let root = fs::canonicalize(unique_temp_root()).expect("canonicalize R10 fixture root");
        let worktree = root.join("repository");
        let store = root.join("store");
        let documents = root.join("documents");
        let portable = root.join("portable");
        let explorer = root.join("explorer");
        let template = root.join("template");
        let global_config = root.join("global.gitconfig");
        fs::create_dir_all(&template).expect("create empty R10 Git template");
        fs::write(&global_config, []).expect("create empty R10 Git configuration");

        let mut init = git_command(&global_config);
        init.args(["init", "--quiet", "--initial-branch=main"])
            .arg(format!("--template={}", template.display()))
            .arg(&worktree);
        successful_output(init, None);

        let reviewed = manifest["files"]
            .as_array()
            .expect("reviewed R10 files")
            .iter()
            .map(|file| (file["path"].as_str().expect("reviewed R10 path"), file))
            .collect::<BTreeMap<_, _>>();
        for (fixture_path, file) in reviewed {
            let relative = fixture_path
                .strip_prefix("repository/")
                .expect("R10 repository-relative fixture path");
            let bytes = read_repository_text(fixture.join(fixture_path));
            assert_reviewed_bytes(file, &bytes, fixture_path);
            let destination = worktree.join(relative);
            fs::create_dir_all(destination.parent().expect("R10 fixture parent"))
                .expect("create R10 fixture parent");
            fs::write(&destination, &bytes).expect("materialize R10 source");

            let mut hash = git_command(&global_config);
            hash.arg("-C")
                .arg(&worktree)
                .args(["hash-object", "-w", "--stdin"]);
            let blob_oid = stdout_line(successful_output(hash, Some(&bytes)));
            assert_eq!(
                blob_oid,
                file["git_blob_oid"].as_str().expect("reviewed R10 blob"),
                "R10 fixture Git blob changed for {fixture_path}"
            );
            update_index(&worktree, &global_config, &blob_oid, relative);
        }

        let mut write_tree = git_command(&global_config);
        write_tree.arg("-C").arg(&worktree).arg("write-tree");
        let tree_oid = stdout_line(successful_output(write_tree, None));
        assert_eq!(tree_oid, FIXTURE_TREE_OID, "R10 fixture tree changed");

        let mut make_commit = git_command(&global_config);
        make_commit
            .arg("-C")
            .arg(&worktree)
            .args(["commit-tree", &tree_oid, "-F", "-"])
            .env("GIT_AUTHOR_NAME", "CodeNoesis")
            .env("GIT_AUTHOR_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_AUTHOR_DATE", "2026-08-09T12:00:00Z")
            .env("GIT_COMMITTER_NAME", "CodeNoesis")
            .env("GIT_COMMITTER_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_COMMITTER_DATE", "2026-08-09T12:00:00Z");
        let commit_oid = stdout_line(successful_output(
            make_commit,
            Some(b"R10 project-owned cfg declaration alternatives fixture\n"),
        ));
        assert_eq!(commit_oid, FIXTURE_COMMIT_OID, "R10 fixture commit changed");

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
            portable,
            explorer,
            commit_oid,
        }
    }

    pub fn scan(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .current_dir(&self.root)
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
                R10_PROFILE,
            ])
            .arg("--store")
            .arg(&self.store)
            .args(["--format", "json"])
            .output()
            .expect("launch R10 scan")
    }

    pub fn docs(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .args(["docs", "--store"])
            .arg(&self.store)
            .args(["--repository-id", REPOSITORY_ID, "--output"])
            .arg(&self.documents)
            .args(["--format", "json"])
            .output()
            .expect("launch R10 docs")
    }

    pub fn query(&self, requested_id: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .args(["query", "--store"])
            .arg(&self.store)
            .args(["--repository-id", REPOSITORY_ID, "--documents"])
            .arg(&self.documents)
            .args(["--id", requested_id, "--format", "json"])
            .output()
            .expect("launch R10 query")
    }

    pub fn export(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .args(["export", "--store"])
            .arg(&self.store)
            .args(["--repository-id", REPOSITORY_ID, "--output"])
            .arg(&self.portable)
            .args(["--portable-profile", R10_PROFILE, "--format", "json"])
            .output()
            .expect("launch R10 export")
    }

    pub fn explore(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .args(["explore", "--input"])
            .arg(self.portable.join("portable-graph.json"))
            .arg("--output")
            .arg(&self.explorer)
            .args(["--explorer-profile", R10_PROFILE, "--format", "json"])
            .output()
            .expect("launch R10 explore")
    }

    pub fn build_sentinel(&self) -> PathBuf {
        self.worktree.join("R10_BUILD_SENTINEL_EXECUTED")
    }
}

impl Drop for MaterializedCfgAlternativesRepository {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/s4/rust-cfg-declaration-alternatives-v1")
}

fn assert_reviewed_bytes(file: &Value, bytes: &[u8], path: &str) {
    assert_eq!(
        u64::try_from(bytes.len()).expect("R10 fixture byte length"),
        file["byte_length"].as_u64().expect("reviewed R10 length"),
        "R10 fixture length changed for {path}"
    );
    assert_eq!(
        lower_hex(&Sha256::digest(bytes)),
        file["sha256"].as_str().expect("reviewed R10 SHA-256"),
        "R10 fixture digest changed for {path}"
    );
}

fn update_index(worktree: &Path, global_config: &Path, oid: &str, path: &str) {
    let mut update = git_command(global_config);
    update
        .arg("-C")
        .arg(worktree)
        .args(["update-index", "--add", "--cacheinfo"])
        .arg(format!("100644,{oid},{path}"));
    successful_output(update, None);
}

fn lower_hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}").expect("write R10 digest");
    }
    encoded
}
