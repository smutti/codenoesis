use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use super::{git_command, read_repository_text, stdout_line, successful_output, unique_temp_root};

pub const COMMIT_A_OID: &str = "d77c36ec27d878cee6d5d85d761de2b70284cd55";
pub const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s2-rust-knowledge-v1";
pub const TREE_A_OID: &str = "262d3054a4d054b2c90fbbb5f1d0f347f680b635";

const CARGO_BLOB_OID: &str = "9f33cdaae97d9c660c90f299e54d4522d4b3d5cc";
const LIB_BLOB_OID: &str = "615f6db198ab9ebb96fbdfbbfab8d7e4e7c0c242";
const SRC_TREE_OID: &str = "3e711cceb17eaaa7707805aaf79fc342d9026c83";

pub struct MaterializedRepository {
    pub root: PathBuf,
    pub worktree: PathBuf,
}

impl MaterializedRepository {
    pub fn revision_a() -> Self {
        let root = unique_temp_root();
        let worktree = root.join("repository");
        let template = root.join("template");
        let global_config = root.join("global.gitconfig");
        fs::create_dir_all(&template).expect("create empty Git template directory");
        fs::write(&global_config, []).expect("create empty global Git configuration");

        let mut init = git_command(&global_config);
        init.args(["init", "--quiet", "--initial-branch=main"])
            .arg(format!("--template={}", template.display()))
            .arg(&worktree);
        successful_output(init, None);

        let cargo_bytes = read_repository_text(fixture_root().join("revision-a/Cargo.toml"));
        let cargo_oid = hash_blob(&worktree, &global_config, &cargo_bytes);
        assert_eq!(cargo_oid, CARGO_BLOB_OID, "S2 Cargo blob identity changed");

        let source_bytes = read_repository_text(fixture_root().join("revision-a/src/lib.rs"));
        let source_oid = hash_blob(&worktree, &global_config, &source_bytes);
        assert_eq!(source_oid, LIB_BLOB_OID, "S2 Rust blob identity changed");

        let source_tree = write_tree(
            &worktree,
            &global_config,
            &format!("100644 blob {LIB_BLOB_OID}\tlib.rs\n"),
        );
        assert_eq!(source_tree, SRC_TREE_OID, "S2 source tree identity changed");

        let root_tree = write_tree(
            &worktree,
            &global_config,
            &format!("100644 blob {CARGO_BLOB_OID}\tCargo.toml\n040000 tree {SRC_TREE_OID}\tsrc\n"),
        );
        assert_eq!(root_tree, TREE_A_OID, "S2 root tree identity changed");

        let mut make_commit = git_command(&global_config);
        make_commit
            .arg("-C")
            .arg(&worktree)
            .args(["commit-tree", TREE_A_OID, "-F", "-"])
            .env("GIT_AUTHOR_NAME", "CodeNoesis Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_AUTHOR_DATE", "1784851200 +0000")
            .env("GIT_COMMITTER_NAME", "CodeNoesis Fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_COMMITTER_DATE", "1784851200 +0000");
        let commit_oid = stdout_line(successful_output(
            make_commit,
            Some(b"CodeNoesis S2 Rust knowledge fixture revision A\n"),
        ));
        assert_eq!(commit_oid, COMMIT_A_OID, "S2 commit identity changed");

        let mut update_ref = git_command(&global_config);
        update_ref
            .arg("-C")
            .arg(&worktree)
            .args(["update-ref", "refs/heads/main", COMMIT_A_OID]);
        successful_output(update_ref, None);

        Self { root, worktree }
    }
}

impl Drop for MaterializedRepository {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/s2/rust-knowledge-v1")
}

pub fn scan(repository: &Path, revision: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_noesis"))
        .args(["scan", "--repository"])
        .arg(repository)
        .args([
            "--repository-id",
            REPOSITORY_ID,
            "--revision",
            revision,
            "--profile",
            "standard-local-s2",
            "--format",
            "json",
        ])
        .output()
        .expect("launch S2 noesis scan")
}

fn hash_blob(worktree: &Path, global_config: &Path, bytes: &[u8]) -> String {
    let mut command = git_command(global_config);
    command
        .arg("-C")
        .arg(worktree)
        .args(["hash-object", "-w", "--stdin"]);
    stdout_line(successful_output(command, Some(bytes)))
}

fn write_tree(worktree: &Path, global_config: &Path, entries: &str) -> String {
    let mut command = git_command(global_config);
    command.arg("-C").arg(worktree).arg("mktree");
    stdout_line(successful_output(command, Some(entries.as_bytes())))
}
