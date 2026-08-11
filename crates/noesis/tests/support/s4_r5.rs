use std::ffi::OsString;
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
pub const EMPTY_REPOSITORY_ID: &str = "urn:codenoesis:fixture:s4-rust-empty-semantic-extension-v1";
pub const EMPTY_FIXTURE_TREE_OID: &str = "d13008ae7c7dbf9807b9599eb9c1b1213b4b94f4";
pub const EMPTY_FIXTURE_COMMIT_OID: &str = "c780476957a29db6ede1cefb408140763990e829";

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
        self.scan_with_profiles(true, true, Some("rust-semantic-depth-v1"))
    }

    pub fn scan_with_profiles(
        &self,
        workspace_profile: bool,
        manifest_profile: bool,
        rust_semantic_profile: Option<&str>,
    ) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_noesis"));
        command
            .args(["scan", "--repository"])
            .arg(&self.worktree)
            .args(["--repository-id", REPOSITORY_ID, "--revision"])
            .arg(&self.commit_oid)
            .args(["--profile", "standard-local-s4"]);
        if workspace_profile {
            command.args(["--workspace-profile", "cargo-root-package-v1"]);
        }
        if manifest_profile {
            command.args(["--manifest-profile", "cargo-manifest-facts-v1"]);
        }
        if let Some(profile) = rust_semantic_profile {
            command.args(["--rust-semantic-profile", profile]);
        }
        command
            .arg("--store")
            .arg(&self.store)
            .args(["--format", "json"])
            .output()
            .expect("launch R5 Rust semantic-depth subject")
    }

    pub fn docs(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .args(["docs", "--store"])
            .arg(&self.store)
            .args(["--repository-id", REPOSITORY_ID, "--output"])
            .arg(&self.documents)
            .args(["--format", "json"])
            .output()
            .expect("launch R5 documentation subject")
    }

    pub fn query(&self, requested_id: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .args(["query", "--store"])
            .arg(&self.store)
            .args(["--repository-id", REPOSITORY_ID, "--documents"])
            .arg(&self.documents)
            .args(["--id", requested_id, "--format", "json"])
            .output()
            .expect("launch R5 exact-ID query subject")
    }

    pub fn replace_source_and_commit(&mut self, source: &[u8]) {
        fs::write(self.worktree.join("src/lib.rs"), source).expect("replace R5 source fixture");
        let global_config = self.root.join("global.gitconfig");
        let mut hash = git_command(&global_config);
        hash.arg("-C")
            .arg(&self.worktree)
            .args(["hash-object", "-w", "--stdin"]);
        let blob_oid = stdout_line(successful_output(hash, Some(source)));
        update_index(
            &self.worktree,
            &global_config,
            "100644",
            &blob_oid,
            "src/lib.rs",
        );
        let mut write_tree = git_command(&global_config);
        write_tree.arg("-C").arg(&self.worktree).arg("write-tree");
        let tree_oid = stdout_line(successful_output(write_tree, None));
        let mut make_commit = git_command(&global_config);
        make_commit
            .arg("-C")
            .arg(&self.worktree)
            .args(["commit-tree", &tree_oid, "-F", "-"])
            .env("GIT_AUTHOR_NAME", "CodeNoesis")
            .env("GIT_AUTHOR_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_AUTHOR_DATE", "1785888000 +0000")
            .env("GIT_COMMITTER_NAME", "CodeNoesis")
            .env("GIT_COMMITTER_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_COMMITTER_DATE", "1785888000 +0000");
        self.commit_oid = stdout_line(successful_output(
            make_commit,
            Some(b"R5 malformed semantic-depth fixture\n"),
        ));
        let mut update_ref = git_command(&global_config);
        update_ref.arg("-C").arg(&self.worktree).args([
            "update-ref",
            "refs/heads/main",
            &self.commit_oid,
        ]);
        successful_output(update_ref, None);
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

pub struct MaterializedEmptyRustSemanticRepository {
    pub root: PathBuf,
    pub worktree: PathBuf,
    pub store: PathBuf,
    pub documents: PathBuf,
    pub portable: PathBuf,
    pub explorer: PathBuf,
    pub commit_oid: String,
}

impl MaterializedEmptyRustSemanticRepository {
    pub fn fixture() -> Self {
        let fixture = empty_fixture_root();
        let manifest: Value = serde_json::from_slice(
            &fs::read(fixture.join("manifest.json")).expect("read empty R5 fixture manifest"),
        )
        .expect("parse empty R5 fixture manifest");
        assert_eq!(manifest["repository_identity"], EMPTY_REPOSITORY_ID);

        let root =
            fs::canonicalize(unique_temp_root()).expect("canonicalize empty semantic fixture root");
        let worktree = root.join("repository");
        let store = root.join("store");
        let documents = root.join("documents");
        let portable = root.join("portable");
        let explorer = root.join("explorer");
        let template = root.join("template");
        let global_config = root.join("global.gitconfig");
        fs::create_dir_all(&template).expect("create empty semantic fixture Git template");
        fs::write(&global_config, []).expect("create empty semantic fixture Git configuration");

        let mut init = git_command(&global_config);
        init.args(["init", "--quiet", "--initial-branch=main"])
            .arg(format!("--template={}", template.display()))
            .arg(&worktree);
        successful_output(init, None);

        for file in manifest["files"]
            .as_array()
            .expect("reviewed empty R5 fixture files")
        {
            let fixture_path = file["path"].as_str().expect("reviewed fixture path");
            let relative = fixture_path
                .strip_prefix("repository/")
                .expect("empty R5 repository-relative fixture path");
            let bytes = read_repository_text(fixture.join(fixture_path));
            assert_eq!(
                u64::try_from(bytes.len()).expect("empty fixture byte length"),
                file["byte_length"].as_u64().expect("reviewed byte length")
            );
            assert_eq!(
                lower_hex(&Sha256::digest(&bytes)),
                file["sha256"].as_str().expect("reviewed SHA-256")
            );
            let destination = worktree.join(relative);
            fs::create_dir_all(destination.parent().expect("empty fixture parent"))
                .expect("create empty fixture parent");
            fs::write(&destination, &bytes).expect("materialize empty semantic fixture");

            let mut hash = git_command(&global_config);
            hash.arg("-C")
                .arg(&worktree)
                .args(["hash-object", "-w", "--stdin"]);
            let blob_oid = stdout_line(successful_output(hash, Some(&bytes)));
            assert_eq!(
                blob_oid,
                file["git_blob_oid"].as_str().expect("reviewed Git blob")
            );
            update_index(&worktree, &global_config, "100644", &blob_oid, relative);
        }

        let mut write_tree = git_command(&global_config);
        write_tree.arg("-C").arg(&worktree).arg("write-tree");
        let tree_oid = stdout_line(successful_output(write_tree, None));
        assert_eq!(tree_oid, EMPTY_FIXTURE_TREE_OID);

        let materialization = &manifest["materialization"];
        let mut make_commit = git_command(&global_config);
        make_commit
            .arg("-C")
            .arg(&worktree)
            .args(["commit-tree", &tree_oid, "-F", "-"])
            .env(
                "GIT_AUTHOR_NAME",
                materialization["author_name"]
                    .as_str()
                    .expect("reviewed author name"),
            )
            .env(
                "GIT_AUTHOR_EMAIL",
                materialization["author_email"]
                    .as_str()
                    .expect("reviewed author email"),
            )
            .env(
                "GIT_AUTHOR_DATE",
                materialization["git_timestamp"]
                    .as_str()
                    .expect("reviewed Git timestamp"),
            )
            .env(
                "GIT_COMMITTER_NAME",
                materialization["author_name"]
                    .as_str()
                    .expect("reviewed committer name"),
            )
            .env(
                "GIT_COMMITTER_EMAIL",
                materialization["author_email"]
                    .as_str()
                    .expect("reviewed committer email"),
            )
            .env(
                "GIT_COMMITTER_DATE",
                materialization["git_timestamp"]
                    .as_str()
                    .expect("reviewed committer timestamp"),
            );
        let message = format!(
            "{}\n",
            materialization["message"]
                .as_str()
                .expect("reviewed commit message")
        );
        let commit_oid = stdout_line(successful_output(make_commit, Some(message.as_bytes())));
        assert_eq!(commit_oid, EMPTY_FIXTURE_COMMIT_OID);
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

    pub fn scan_r14(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .current_dir(&self.root)
            .args(["scan", "--repository"])
            .arg(&self.worktree)
            .args(["--repository-id", EMPTY_REPOSITORY_ID, "--revision"])
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
                "--rust-framework-profile",
                "rust-framework-declarations-v1",
                "--rust-callable-profile",
                "rust-callable-semantics-v1",
                "--rust-expression-profile",
                "rust-expression-bindings-v1",
            ])
            .arg("--store")
            .arg(&self.store)
            .args(["--format", "json"])
            .output()
            .expect("launch empty R5 through R14 scan")
    }

    pub fn permuted_scan_command(&self, seed: u64) -> Command {
        let store = self.root.join(format!("permuted-store-{seed}"));
        let mut options = vec![
            (
                OsString::from("--repository"),
                self.worktree.clone().into_os_string(),
            ),
            (
                OsString::from("--repository-id"),
                OsString::from(EMPTY_REPOSITORY_ID),
            ),
            (
                OsString::from("--revision"),
                OsString::from(&self.commit_oid),
            ),
            (
                OsString::from("--profile"),
                OsString::from("standard-local-s4"),
            ),
            (
                OsString::from("--workspace-profile"),
                OsString::from("cargo-root-package-v1"),
            ),
            (
                OsString::from("--manifest-profile"),
                OsString::from("cargo-manifest-facts-v1"),
            ),
            (
                OsString::from("--rust-semantic-profile"),
                OsString::from("rust-semantic-depth-v1"),
            ),
            (
                OsString::from("--rust-framework-profile"),
                OsString::from("rust-framework-declarations-v1"),
            ),
            (
                OsString::from("--rust-callable-profile"),
                OsString::from("rust-callable-semantics-v1"),
            ),
            (
                OsString::from("--rust-expression-profile"),
                OsString::from("rust-expression-bindings-v1"),
            ),
            (OsString::from("--store"), store.into_os_string()),
            (OsString::from("--format"), OsString::from("json")),
        ];
        let mut state = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
        for position in (1..options.len()).rev() {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let divisor = u64::try_from(position + 1).expect("option count fits u64");
            let selected =
                usize::try_from(state % divisor).expect("selected option index fits usize");
            options.swap(position, selected);
        }
        let mut command = Command::new(env!("CARGO_BIN_EXE_noesis"));
        command.current_dir(&self.root).arg("scan");
        for (flag, value) in options {
            command.arg(flag).arg(value);
        }
        command
    }

    pub fn replace_source_and_commit(&mut self, source: &[u8]) {
        fs::write(self.worktree.join("src/lib.rs"), source)
            .expect("replace empty-extension source fixture");
        let global_config = self.root.join("global.gitconfig");
        let mut hash = git_command(&global_config);
        hash.arg("-C")
            .arg(&self.worktree)
            .args(["hash-object", "-w", "--stdin"]);
        let blob_oid = stdout_line(successful_output(hash, Some(source)));
        update_index(
            &self.worktree,
            &global_config,
            "100644",
            &blob_oid,
            "src/lib.rs",
        );
        let mut write_tree = git_command(&global_config);
        write_tree.arg("-C").arg(&self.worktree).arg("write-tree");
        let tree_oid = stdout_line(successful_output(write_tree, None));
        let mut make_commit = git_command(&global_config);
        make_commit
            .arg("-C")
            .arg(&self.worktree)
            .args(["commit-tree", &tree_oid, "-F", "-"])
            .env("GIT_AUTHOR_NAME", "CodeNoesis Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_AUTHOR_DATE", "1786432215 +0200")
            .env("GIT_COMMITTER_NAME", "CodeNoesis Fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_COMMITTER_DATE", "1786432215 +0200");
        self.commit_oid = stdout_line(successful_output(
            make_commit,
            Some(b"paired supported constant fixture\n"),
        ));
        let mut update_ref = git_command(&global_config);
        update_ref.arg("-C").arg(&self.worktree).args([
            "update-ref",
            "refs/heads/main",
            &self.commit_oid,
        ]);
        successful_output(update_ref, None);
    }

    pub fn docs(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .args(["docs", "--store"])
            .arg(&self.store)
            .args(["--repository-id", EMPTY_REPOSITORY_ID, "--output"])
            .arg(&self.documents)
            .args(["--format", "json"])
            .output()
            .expect("launch empty R5 R14 documentation")
    }

    pub fn query(&self, requested_id: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .args(["query", "--store"])
            .arg(&self.store)
            .args(["--repository-id", EMPTY_REPOSITORY_ID, "--documents"])
            .arg(&self.documents)
            .args(["--id", requested_id, "--format", "json"])
            .output()
            .expect("launch empty R5 R14 exact-ID query")
    }

    pub fn export(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .args(["export", "--store"])
            .arg(&self.store)
            .args(["--repository-id", EMPTY_REPOSITORY_ID, "--documents"])
            .arg(&self.documents)
            .arg("--output")
            .arg(&self.portable)
            .args([
                "--portable-profile",
                "rust-expression-bindings-v1",
                "--format",
                "json",
            ])
            .output()
            .expect("launch empty R5 R14 export")
    }

    pub fn explore(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .args(["explore", "--input"])
            .arg(self.portable.join("portable-graph.json"))
            .arg("--output")
            .arg(&self.explorer)
            .args([
                "--explorer-profile",
                "rust-expression-bindings-v1",
                "--format",
                "json",
            ])
            .output()
            .expect("launch empty R5 R14 explorer")
    }

    pub fn build_sentinel(&self) -> PathBuf {
        self.worktree.join("EMPTY_R5_BUILD_SENTINEL_EXECUTED")
    }
}

impl Drop for MaterializedEmptyRustSemanticRepository {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/s4/rust-semantic-depth-v1")
}

pub fn empty_fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/s4/rust-empty-semantic-extension-v1")
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
