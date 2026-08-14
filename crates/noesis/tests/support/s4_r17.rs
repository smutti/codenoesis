use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use sha2::{Digest as _, Sha256};

use super::{git_command, read_repository_text, stdout_line, successful_output, unique_temp_root};

pub const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s4-rust-function-context-v1";
pub const FIXTURE_TREE_OID: &str = "aed06ed2ea7447c85a25b03fa48dd9d591b7d825";
pub const FIXTURE_COMMIT_OID: &str = "09093916dfc9b925fa22c7de660b67103a4def01";
pub const CONTEXT_PROFILE: &str = "rust-function-context-v1";
pub const ROOT_CALLABLE_ID: &str =
    "urn:codenoesis:entity:blake3:08acb2c94e0a5448751cac2901892967ce6489d47cbe88ffd1354dca8ad5a3ce";

pub struct MaterializedFunctionContextRepository {
    pub root: PathBuf,
    pub worktree: PathBuf,
    pub store: PathBuf,
    pub documents: PathBuf,
    pub portable: PathBuf,
    pub explorer: PathBuf,
    pub commit_oid: String,
}

impl MaterializedFunctionContextRepository {
    pub fn fixture() -> Self {
        let fixture = fixture_root();
        let manifest: Value = serde_json::from_slice(
            &fs::read(fixture.join("manifest.json")).expect("read R17 fixture manifest"),
        )
        .expect("parse R17 fixture manifest");
        assert_eq!(manifest["repository_identity"], REPOSITORY_ID);

        let root = unique_temp_root();
        #[cfg(not(windows))]
        let root = fs::canonicalize(root).expect("canonicalize R17 fixture root");
        let worktree = root.join("repository");
        let store = root.join("store");
        let documents = root.join("documents");
        let portable = root.join("portable");
        let explorer = root.join("explorer");
        let template = root.join("template");
        let global_config = root.join("global.gitconfig");
        fs::create_dir_all(&template).expect("create R17 Git template directory");
        fs::write(&global_config, []).expect("create R17 Git configuration");

        let mut init = git_command(&global_config);
        init.args(["init", "--quiet", "--initial-branch=main"])
            .arg(format!("--template={}", template.display()))
            .arg(&worktree);
        successful_output(init, None);

        for file in manifest["files"].as_array().expect("reviewed R17 files") {
            let fixture_path = file["path"].as_str().expect("reviewed R17 path");
            let relative = fixture_path
                .strip_prefix("repository/")
                .expect("R17 repository-relative path");
            let bytes = read_repository_text(fixture.join(fixture_path));
            assert_eq!(
                u64::try_from(bytes.len()).expect("R17 fixture byte length"),
                file["byte_length"].as_u64().expect("reviewed byte length")
            );
            assert_eq!(
                lower_hex(&Sha256::digest(&bytes)),
                file["sha256"].as_str().expect("reviewed SHA-256")
            );
            let destination = worktree.join(relative);
            fs::create_dir_all(destination.parent().expect("R17 fixture parent"))
                .expect("create R17 fixture parent");
            fs::write(&destination, &bytes).expect("materialize R17 fixture");

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
                materialization["author_name"].as_str().expect("R17 author"),
            )
            .env(
                "GIT_AUTHOR_EMAIL",
                materialization["author_email"].as_str().expect("R17 email"),
            )
            .env(
                "GIT_AUTHOR_DATE",
                materialization["author_date"].as_str().expect("R17 date"),
            )
            .env(
                "GIT_COMMITTER_NAME",
                materialization["author_name"]
                    .as_str()
                    .expect("R17 committer"),
            )
            .env(
                "GIT_COMMITTER_EMAIL",
                materialization["author_email"]
                    .as_str()
                    .expect("R17 committer email"),
            )
            .env(
                "GIT_COMMITTER_DATE",
                materialization["author_date"]
                    .as_str()
                    .expect("R17 committer date"),
            );
        let message = materialization["commit_message"]
            .as_str()
            .expect("R17 commit message");
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
                "rust-semantic-depth-v1",
                "--rust-framework-profile",
                "rust-framework-declarations-v1",
                "--rust-callable-profile",
                "rust-callable-semantics-v1",
                "--rust-expression-profile",
                "rust-expression-bindings-v1",
                "--rust-flow-profile",
                "rust-local-flow-v1",
                "--rust-constant-profile",
                "rust-safe-constant-evaluation-v1",
            ])
            .arg("--store")
            .arg(&self.store)
            .args(["--format", "json"])
            .output()
            .expect("launch R17 fixture scan")
    }

    pub fn docs(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .args(["docs", "--store"])
            .arg(&self.store)
            .args(["--repository-id", REPOSITORY_ID, "--output"])
            .arg(&self.documents)
            .args(["--format", "json"])
            .output()
            .expect("launch R17 documentation")
    }

    pub fn query(&self, requested_id: &str) -> Output {
        let mut command = self.query_command(requested_id);
        command.output().expect("launch R17 default query")
    }

    pub fn query_context(&self, requested_id: &str) -> Output {
        let mut command = self.query_command(requested_id);
        command.args(["--context-profile", CONTEXT_PROFILE]);
        command.output().expect("launch R17 function-context query")
    }

    pub fn export(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .args(["export", "--store"])
            .arg(&self.store)
            .args(["--repository-id", REPOSITORY_ID, "--documents"])
            .arg(&self.documents)
            .arg("--output")
            .arg(&self.portable)
            .args([
                "--portable-profile",
                "rust-safe-constant-evaluation-v1",
                "--format",
                "json",
            ])
            .output()
            .expect("launch R17 PortableGraphV9 export")
    }

    pub fn explore_context(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .args(["explore", "--input"])
            .arg(self.portable.join("portable-graph.json"))
            .arg("--output")
            .arg(&self.explorer)
            .args(["--explorer-profile", CONTEXT_PROFILE, "--format", "json"])
            .output()
            .expect("launch R17 LocalExplorerV10")
    }

    pub fn build_sentinel(&self) -> PathBuf {
        self.worktree.join("R17_BUILD_SENTINEL_EXECUTED")
    }

    fn query_command(&self, requested_id: &str) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_noesis"));
        command
            .args(["query", "--store"])
            .arg(&self.store)
            .args(["--repository-id", REPOSITORY_ID, "--documents"])
            .arg(&self.documents)
            .args(["--id", requested_id, "--format", "json"]);
        command
    }
}

impl Drop for MaterializedFunctionContextRepository {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub fn expected_function_context() -> Value {
    serde_json::from_slice(
        &fs::read(fixture_root().join("expected-function-context.json"))
            .expect("read R17 expected function context"),
    )
    .expect("parse R17 expected function context")
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/s4/rust-function-context-v1")
}

fn update_index(worktree: &Path, global_config: &Path, mode: &str, oid: &str, path: &str) {
    let mut update = git_command(global_config);
    update
        .arg("-C")
        .arg(worktree)
        .args(["update-index", "--add", "--cacheinfo", mode, oid, path]);
    successful_output(update, None);
}

fn lower_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("write SHA-256 hex");
    }
    output
}
