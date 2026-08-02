use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use super::{git_command, read_repository_text, stdout_line, successful_output};

pub const ROOT_REPOSITORY_ID: &str = "urn:codenoesis:fixture:s1-gitlink-boundary-v1";
pub const ROOT_COMMIT_OID: &str = "e3a117e190a92585bcae4c49c775d310243107e7";
pub const ROOT_TREE_OID: &str = "81f40228aa5c1e9dab33d6cc3d5d90f14aeb7d2e";
pub const GITMODULES_BLOB_OID: &str = "204d65a4ed6a58dd265349f8a1579a4522dc4f7d";
pub const GITLINK_OID: &str = "6ecf94267842da776e35406a9ebcb85e058a3181";

const ROOT_MANIFEST_BLOB_OID: &str = "9a61bc964dd0b80e54880d0a40471b50324b6e15";
const CRATES_TREE_OID: &str = "3e03df22f6961d79f831318ae35e06fa026ad599";
const EXTERNAL_TREE_OID: &str = "3ee7026beba4578478d73788368adc793e975ca0";

pub struct MaterializedBoundaryRepository {
    pub base: super::s4::MaterializedRepository,
}

impl MaterializedBoundaryRepository {
    pub fn unbound() -> Self {
        let base = super::s4::MaterializedRepository::revision_a();
        let global_config = base.root.join("global.gitconfig");
        let gitmodules = read_repository_text(fixture_root().join("revision-overlay/.gitmodules"));

        let mut hash_gitmodules = git_command(&global_config);
        hash_gitmodules
            .arg("-C")
            .arg(&base.worktree)
            .args(["hash-object", "-w", "--stdin"]);
        let gitmodules_oid = stdout_line(successful_output(hash_gitmodules, Some(&gitmodules)));
        assert_eq!(
            gitmodules_oid, GITMODULES_BLOB_OID,
            "R2 .gitmodules blob identity changed"
        );

        let external_tree = write_tree(
            &base.worktree,
            &global_config,
            &format!("160000 commit {GITLINK_OID}\tnested-model\n"),
            true,
        );
        assert_eq!(
            external_tree, EXTERNAL_TREE_OID,
            "R2 external tree identity changed"
        );

        let root_tree = write_tree(
            &base.worktree,
            &global_config,
            &format!(
                "100644 blob {GITMODULES_BLOB_OID}\t.gitmodules\n\
                 100644 blob {ROOT_MANIFEST_BLOB_OID}\tCargo.toml\n\
                 040000 tree {CRATES_TREE_OID}\tcrates\n\
                 040000 tree {EXTERNAL_TREE_OID}\texternal\n"
            ),
            false,
        );
        assert_eq!(root_tree, ROOT_TREE_OID, "R2 root tree identity changed");

        let mut make_commit = git_command(&global_config);
        make_commit
            .arg("-C")
            .arg(&base.worktree)
            .args(["commit-tree", ROOT_TREE_OID, "-F", "-"])
            .env("GIT_AUTHOR_NAME", "CodeNoesis Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_AUTHOR_DATE", "1785628800 +0000")
            .env("GIT_COMMITTER_NAME", "CodeNoesis Fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_COMMITTER_DATE", "1785628800 +0000");
        let commit_oid = stdout_line(successful_output(
            make_commit,
            Some(b"CodeNoesis R2 gitlink boundary fixture\n"),
        ));
        assert_eq!(
            commit_oid, ROOT_COMMIT_OID,
            "R2 root commit identity changed"
        );

        let mut update_ref = git_command(&global_config);
        update_ref.arg("-C").arg(&base.worktree).args([
            "update-ref",
            "refs/heads/main",
            ROOT_COMMIT_OID,
        ]);
        successful_output(update_ref, None);

        Self { base }
    }

    pub fn scan_unbound(&self) -> Output {
        scan_command(&self.base.worktree, &self.base.store)
            .output()
            .expect("launch R2 boundary subject")
    }
}

pub fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/s1/gitlink-boundary-v1")
}

pub fn scan_command(repository: &Path, store: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_noesis"));
    command
        .args(["scan", "--repository"])
        .arg(repository)
        .args([
            "--repository-id",
            ROOT_REPOSITORY_ID,
            "--revision",
            ROOT_COMMIT_OID,
            "--profile",
            "standard-local-s4",
            "--repository-boundary-profile",
            "local-gitlinks-v1",
            "--store",
        ])
        .arg(store)
        .args(["--format", "json"]);
    command
}

pub fn expected_unbound_boundaries() -> serde_json::Value {
    serde_json::from_slice(
        &fs::read(fixture_root().join("expected-boundaries-unbound.json"))
            .expect("read approved unbound boundary golden"),
    )
    .expect("parse approved unbound boundary golden")
}

fn write_tree(worktree: &Path, global_config: &Path, entries: &str, allow_missing: bool) -> String {
    let mut command = git_command(global_config);
    command.arg("-C").arg(worktree).arg("mktree");
    if allow_missing {
        command.arg("--missing");
    }
    stdout_line(successful_output(command, Some(entries.as_bytes())))
}
