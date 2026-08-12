use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use sha2::{Digest as _, Sha256};

use super::{git_command, read_repository_text, stdout_line, successful_output, unique_temp_root};

pub const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s4-rust-real-repository-shapes-v1";
pub const FIXTURE_TREE_OID: &str = "389e22d33ed887e32da70c2d60ddd72893bf9c27";
pub const FIXTURE_COMMIT_OID: &str = "accc966f2c2729dddc95fe7caf7036312a2a01e0";
pub const OUTPUT_CAPACITY_PROFILE: &str = "local-snapshot-256m-v1";

pub struct MaterializedR14R15CorrectionRepository {
    pub root: PathBuf,
    pub worktree: PathBuf,
    pub r14_store: PathBuf,
    pub r15_store: PathBuf,
    pub profile_store: PathBuf,
    pub commit_oid: String,
}

impl MaterializedR14R15CorrectionRepository {
    pub fn fixture() -> Self {
        let fixture = fixture_root();
        let manifest: Value = serde_json::from_slice(
            &fs::read(fixture.join("manifest.json")).expect("read correction fixture manifest"),
        )
        .expect("parse correction fixture manifest");
        assert_eq!(manifest["repository_identity"], REPOSITORY_ID);

        let root = unique_temp_root();
        #[cfg(not(windows))]
        let root = fs::canonicalize(root).expect("canonicalize correction fixture root");
        let worktree = root.join("repository");
        let template = root.join("template");
        let global_config = root.join("global.gitconfig");
        fs::create_dir_all(&template).expect("create correction Git template directory");
        fs::write(&global_config, []).expect("create correction Git configuration");

        let mut init = git_command(&global_config);
        init.args(["init", "--quiet", "--initial-branch=main"])
            .arg(format!("--template={}", template.display()))
            .arg(&worktree);
        successful_output(init, None);

        for file in manifest["files"]
            .as_array()
            .expect("reviewed correction files")
        {
            let fixture_path = file["path"]
                .as_str()
                .expect("reviewed correction fixture path");
            let relative = fixture_path
                .strip_prefix("repository/")
                .expect("correction repository-relative path");
            let bytes = read_repository_text(fixture.join(fixture_path));
            assert_eq!(
                u64::try_from(bytes.len()).expect("correction fixture byte length"),
                file["byte_length"].as_u64().expect("reviewed byte length")
            );
            assert_eq!(
                lower_hex(&Sha256::digest(&bytes)),
                file["sha256"].as_str().expect("reviewed SHA-256")
            );
            let destination = worktree.join(relative);
            fs::create_dir_all(destination.parent().expect("correction fixture parent"))
                .expect("create correction fixture parent");
            fs::write(&destination, &bytes).expect("materialize correction fixture");

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
                materialization["author_name"]
                    .as_str()
                    .expect("correction author"),
            )
            .env(
                "GIT_AUTHOR_EMAIL",
                materialization["author_email"]
                    .as_str()
                    .expect("correction email"),
            )
            .env(
                "GIT_AUTHOR_DATE",
                materialization["author_date"]
                    .as_str()
                    .expect("correction date"),
            )
            .env(
                "GIT_COMMITTER_NAME",
                materialization["author_name"]
                    .as_str()
                    .expect("correction committer"),
            )
            .env(
                "GIT_COMMITTER_EMAIL",
                materialization["author_email"]
                    .as_str()
                    .expect("correction committer email"),
            )
            .env(
                "GIT_COMMITTER_DATE",
                materialization["author_date"]
                    .as_str()
                    .expect("correction committer date"),
            );
        let message = materialization["commit_message"]
            .as_str()
            .expect("correction commit message");
        let commit_oid = stdout_line(successful_output(make_commit, Some(message.as_bytes())));
        assert_eq!(commit_oid, FIXTURE_COMMIT_OID);

        let mut update_ref = git_command(&global_config);
        update_ref
            .arg("-C")
            .arg(&worktree)
            .args(["update-ref", "refs/heads/main", &commit_oid]);
        successful_output(update_ref, None);

        Self {
            r14_store: root.join("store-r14"),
            r15_store: root.join("store-r15"),
            profile_store: root.join("store-profile"),
            root,
            worktree,
            commit_oid,
        }
    }

    pub fn scan_r14(&self) -> Output {
        self.scan_command(false, &self.r14_store)
            .output()
            .expect("launch corrected R14 fixture scan")
    }

    pub fn scan_r15(&self) -> Output {
        self.scan_command(true, &self.r15_store)
            .output()
            .expect("launch corrected R15 fixture scan")
    }

    pub fn scan_r14_with_256m_profile(&self) -> Output {
        let mut command = self.scan_command(false, &self.profile_store);
        command.args(["--output-capacity-profile", OUTPUT_CAPACITY_PROFILE]);
        command
            .output()
            .expect("launch corrected R14 256 MiB profile scan")
    }

    fn scan_command(&self, include_flow: bool, store: &Path) -> Command {
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
            .arg(store)
            .args(["--format", "json"]);
        if include_flow {
            command.args(["--rust-flow-profile", "rust-local-flow-v1"]);
        }
        command
    }
}

impl Drop for MaterializedR14R15CorrectionRepository {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub fn expected_correction() -> Value {
    serde_json::from_slice(
        &fs::read(fixture_root().join("expected-r14-r15-correction.json"))
            .expect("read correction oracle"),
    )
    .expect("parse correction oracle")
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/s4/rust-real-repository-shapes-v1")
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
        write!(&mut encoded, "{byte:02x}").expect("write correction digest");
    }
    encoded
}
