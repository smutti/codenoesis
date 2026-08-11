use std::ffi::OsString;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use sha2::{Digest as _, Sha256};

use super::{git_command, read_repository_text, stdout_line, successful_output, unique_temp_root};

pub const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s4-rust-local-flow-v1";
pub const FIXTURE_TREE_OID: &str = "6a97e7b14d29db6aa50416fd9ac76b9022298104";
pub const FIXTURE_COMMIT_OID: &str = "552a8cdc76b2dd80dc26ad1e3b381fc0de9eab24";
pub const FLOW_PROFILE: &str = "rust-local-flow-v1";

pub struct MaterializedLocalFlowRepository {
    pub root: PathBuf,
    pub worktree: PathBuf,
    pub store: PathBuf,
    pub documents: PathBuf,
    pub portable: PathBuf,
    pub explorer: PathBuf,
    pub commit_oid: String,
}

impl MaterializedLocalFlowRepository {
    #[allow(clippy::too_many_lines)]
    pub fn fixture() -> Self {
        let fixture = fixture_root();
        let manifest: Value = serde_json::from_slice(
            &fs::read(fixture.join("manifest.json")).expect("read R15 fixture manifest"),
        )
        .expect("parse R15 fixture manifest");
        assert_eq!(manifest["repository_identity"], REPOSITORY_ID);

        let root = unique_temp_root();
        #[cfg(not(windows))]
        let root = fs::canonicalize(root).expect("canonicalize R15 fixture root");
        let worktree = root.join("repository");
        let store = root.join("store");
        let documents = root.join("documents");
        let portable = root.join("portable");
        let explorer = root.join("explorer");
        let template = root.join("template");
        let global_config = root.join("global.gitconfig");
        fs::create_dir_all(&template).expect("create R15 Git template directory");
        fs::write(&global_config, []).expect("create R15 Git configuration");

        let mut init = git_command(&global_config);
        init.args(["init", "--quiet", "--initial-branch=main"])
            .arg(format!("--template={}", template.display()))
            .arg(&worktree);
        successful_output(init, None);

        for file in manifest["files"].as_array().expect("reviewed R15 files") {
            let fixture_path = file["path"].as_str().expect("reviewed R15 fixture path");
            let relative = fixture_path
                .strip_prefix("repository/")
                .expect("R15 repository-relative path");
            let bytes = read_repository_text(fixture.join(fixture_path));
            assert_eq!(
                u64::try_from(bytes.len()).expect("R15 fixture byte length"),
                file["byte_length"].as_u64().expect("reviewed byte length")
            );
            assert_eq!(
                lower_hex(&Sha256::digest(&bytes)),
                file["sha256"].as_str().expect("reviewed SHA-256")
            );
            let destination = worktree.join(relative);
            fs::create_dir_all(destination.parent().expect("R15 fixture parent"))
                .expect("create R15 fixture parent");
            fs::write(&destination, &bytes).expect("materialize R15 fixture");

            let mut hash = git_command(&global_config);
            hash.arg("-C")
                .arg(&worktree)
                .args(["hash-object", "-w", "--stdin"]);
            let blob_oid = stdout_line(successful_output(hash, Some(&bytes)));
            assert_eq!(blob_oid, file["blob_oid"].as_str().expect("reviewed blob"));
            update_index(
                &worktree,
                &global_config,
                file["mode"].as_str().expect("reviewed mode"),
                &blob_oid,
                relative,
            );
        }

        let mut write_tree = git_command(&global_config);
        write_tree.arg("-C").arg(&worktree).arg("write-tree");
        let tree_oid = stdout_line(successful_output(write_tree, None));
        assert_eq!(tree_oid, FIXTURE_TREE_OID);

        let materialization = &manifest["materialization"];
        let mut make_commit = git_command(&global_config);
        make_commit
            .arg("-C")
            .arg(&worktree)
            .args(["commit-tree", &tree_oid, "-F", "-"])
            .env(
                "GIT_AUTHOR_NAME",
                materialization["author_name"].as_str().expect("R15 author"),
            )
            .env(
                "GIT_AUTHOR_EMAIL",
                materialization["author_email"].as_str().expect("R15 email"),
            )
            .env(
                "GIT_AUTHOR_DATE",
                materialization["author_date"].as_str().expect("R15 date"),
            )
            .env(
                "GIT_COMMITTER_NAME",
                materialization["author_name"]
                    .as_str()
                    .expect("R15 committer"),
            )
            .env(
                "GIT_COMMITTER_EMAIL",
                materialization["author_email"]
                    .as_str()
                    .expect("R15 committer email"),
            )
            .env(
                "GIT_COMMITTER_DATE",
                materialization["author_date"]
                    .as_str()
                    .expect("R15 committer date"),
            );
        let message = materialization["commit_message"]
            .as_str()
            .expect("R15 commit message");
        let commit_oid = stdout_line(successful_output(make_commit, Some(message.as_bytes())));
        assert_eq!(commit_oid, FIXTURE_COMMIT_OID);
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
        self.scan_command(true)
            .output()
            .expect("launch R15 local-flow scan")
    }

    pub fn scan_r14(&self) -> Output {
        self.scan_command(false)
            .output()
            .expect("launch R14 compatibility scan")
    }

    pub fn scan_with_options(&self, extra_options: &[&str]) -> Output {
        let mut command = self.scan_command(true);
        command.args(extra_options);
        command.output().expect("launch R15 selector matrix scan")
    }

    pub fn permuted_scan_command(&self, seed: u64) -> Command {
        let store_slot = if seed >= 100 {
            10_u64.saturating_add(seed % 10)
        } else {
            seed % 10
        };
        let store = self.root.join(format!("permuted-store-{store_slot}"));
        let mut options = vec![
            (
                OsString::from("--repository"),
                self.worktree.clone().into_os_string(),
            ),
            (
                OsString::from("--repository-id"),
                OsString::from(REPOSITORY_ID),
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
            (
                OsString::from("--rust-flow-profile"),
                OsString::from(FLOW_PROFILE),
            ),
            (OsString::from("--store"), store.into_os_string()),
            (OsString::from("--format"), OsString::from("json")),
        ];
        let mut state = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
        for position in (1..options.len()).rev() {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let divisor = u64::try_from(position + 1).expect("R15 option count fits u64");
            let selected = usize::try_from(state % divisor).expect("R15 option index fits usize");
            options.swap(position, selected);
        }
        let mut command = Command::new(env!("CARGO_BIN_EXE_noesis"));
        command.current_dir(&self.root).arg("scan");
        for (flag, value) in options {
            command.arg(flag).arg(value);
        }
        command
    }

    pub fn docs(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .args(["docs", "--store"])
            .arg(&self.store)
            .args(["--repository-id", REPOSITORY_ID, "--output"])
            .arg(&self.documents)
            .args(["--format", "json"])
            .output()
            .expect("launch R15 documentation")
    }

    pub fn query(&self, requested_id: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .args(["query", "--store"])
            .arg(&self.store)
            .args(["--repository-id", REPOSITORY_ID, "--documents"])
            .arg(&self.documents)
            .args(["--id", requested_id, "--format", "json"])
            .output()
            .expect("launch R15 exact-ID query")
    }

    pub fn export(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .args(["export", "--store"])
            .arg(&self.store)
            .args(["--repository-id", REPOSITORY_ID, "--documents"])
            .arg(&self.documents)
            .arg("--output")
            .arg(&self.portable)
            .args(["--portable-profile", FLOW_PROFILE, "--format", "json"])
            .output()
            .expect("launch R15 portable export")
    }

    pub fn explore(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .args(["explore", "--input"])
            .arg(self.portable.join("portable-graph.json"))
            .arg("--output")
            .arg(&self.explorer)
            .args(["--explorer-profile", FLOW_PROFILE, "--format", "json"])
            .output()
            .expect("launch R15 local explorer")
    }

    pub fn build_sentinel(&self) -> PathBuf {
        self.worktree.join("R15_BUILD_SENTINEL_EXECUTED")
    }

    fn scan_command(&self, include_flow: bool) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_noesis"));
        command
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
            .args(["--format", "json"]);
        if include_flow {
            command.args(["--rust-flow-profile", FLOW_PROFILE]);
        }
        command
    }
}

impl Drop for MaterializedLocalFlowRepository {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub fn expected_local_flow() -> Value {
    serde_json::from_slice(
        &fs::read(fixture_root().join("expected-local-flow.json"))
            .expect("read R15 expected local flow"),
    )
    .expect("parse R15 expected local flow")
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/s4/rust-local-flow-v1")
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
        write!(&mut encoded, "{byte:02x}").expect("write R15 digest");
    }
    encoded
}
