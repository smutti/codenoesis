use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use super::{git_command, stdout_line, successful_output, unique_temp_root};

pub const COMMIT_A_OID: &str = "a72b34a03936b70511bd72bd4fa0d37a5a593386";
pub const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s1-safe-inventory-v1";
pub const TREE_A_OID: &str = "2a77b46a6a4a7645b7ec63831ce7e46bd7921904";

const FILES: [(&str, &str, &str); 9] = [
    (
        ".github/CODEOWNERS",
        "100644",
        "e2d63062cf5b70d85dfa800ba4acadc7b3a78262",
    ),
    (
        "Cargo.toml",
        "100644",
        "f2c75744ba1623ebd2bf64e8dda36fdb0fa86ab6",
    ),
    (
        "README.md",
        "100644",
        "bea61f8462cb66bab5eb5dbdc74b8054d0c0f116",
    ),
    (
        "api/openapi.yaml",
        "100644",
        "c55d01ab6f19f6d6d6ee6e81d40a65d73733c995",
    ),
    (
        "assets/payload.bin",
        "100644",
        "de75dc54dee262791dc95eca71814a74aae4bbce",
    ),
    (
        "build.rs",
        "100644",
        "0ef4746c8ff5e7e129364c7c88be39742ac195e2",
    ),
    (
        "rustfmt.toml",
        "100644",
        "ed49ca7b3d9a10e8bc0c7337d85ff497e83cc3bf",
    ),
    (
        "src/lib.rs",
        "100644",
        "432593368f9028b66c60c5f011dd54949c10ca81",
    ),
    (
        "tools/sentinel.sh",
        "100755",
        "9e02753a1d6a2f3d6825bb6e7107cecb1ad7e1c6",
    ),
];

pub struct MaterializedRepository {
    pub worktree: PathBuf,
    root: PathBuf,
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

        for (path, _, expected_oid) in FILES {
            let bytes = fs::read(fixture_root().join("revision-a").join(path))
                .unwrap_or_else(|error| panic!("read S1 fixture source {path}: {error}"));
            let mut hash_blob = git_command(&global_config);
            hash_blob
                .arg("-C")
                .arg(&worktree)
                .args(["hash-object", "-w", "--stdin"]);
            let observed_oid = stdout_line(successful_output(hash_blob, Some(&bytes)));
            assert_eq!(
                observed_oid, expected_oid,
                "S1 fixture blob identity changed for {path}"
            );
        }

        let github_tree = write_tree(
            &worktree,
            &global_config,
            &[tree_entry(".github/CODEOWNERS")],
        );
        assert_eq!(
            github_tree, "5f093732bf441a06ff1af692b108674a9e585f31",
            "S1 .github tree identity changed"
        );
        let api_tree = write_tree(&worktree, &global_config, &[tree_entry("api/openapi.yaml")]);
        assert_eq!(
            api_tree, "b6aef728b9c98d60001c3c02ed8b18a907a0e266",
            "S1 api tree identity changed"
        );
        let assets_tree = write_tree(
            &worktree,
            &global_config,
            &[tree_entry("assets/payload.bin")],
        );
        assert_eq!(
            assets_tree, "5507e3f2d1cb90e4751ab11bb1896d46c9ec1840",
            "S1 assets tree identity changed"
        );
        let source_tree = write_tree(&worktree, &global_config, &[tree_entry("src/lib.rs")]);
        assert_eq!(
            source_tree, "f41cdbd8214aeb85cd9b1eb07d45308fed184f9d",
            "S1 src tree identity changed"
        );
        let tools_tree = write_tree(
            &worktree,
            &global_config,
            &[tree_entry("tools/sentinel.sh")],
        );
        assert_eq!(
            tools_tree, "197d47856ba9946649aa3999766a77ea8537703a",
            "S1 tools tree identity changed"
        );

        let root_tree = write_tree(
            &worktree,
            &global_config,
            &[
                format!("040000 tree {github_tree}\t.github\n"),
                tree_entry("Cargo.toml"),
                tree_entry("README.md"),
                format!("040000 tree {api_tree}\tapi\n"),
                format!("040000 tree {assets_tree}\tassets\n"),
                tree_entry("build.rs"),
                tree_entry("rustfmt.toml"),
                format!("040000 tree {source_tree}\tsrc\n"),
                format!("040000 tree {tools_tree}\ttools\n"),
            ],
        );
        assert_eq!(root_tree, TREE_A_OID, "S1 root tree identity changed");

        let mut make_commit = git_command(&global_config);
        make_commit
            .arg("-C")
            .arg(&worktree)
            .args(["commit-tree", TREE_A_OID, "-F", "-"])
            .env("GIT_AUTHOR_NAME", "CodeNoesis Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_AUTHOR_DATE", "978307200 +0000")
            .env("GIT_COMMITTER_NAME", "CodeNoesis Fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_COMMITTER_DATE", "978307200 +0000");
        let commit_oid = stdout_line(successful_output(
            make_commit,
            Some(b"fixture: S1 revision A\n"),
        ));
        assert_eq!(commit_oid, COMMIT_A_OID, "S1 commit identity changed");

        let mut update_ref = git_command(&global_config);
        update_ref
            .arg("-C")
            .arg(&worktree)
            .args(["update-ref", "refs/heads/main", COMMIT_A_OID]);
        successful_output(update_ref, None);

        Self { worktree, root }
    }
}

impl Drop for MaterializedRepository {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/s1/safe-inventory-v1")
}

pub fn scan(repository: &Path, revision: &str) -> Output {
    scan_command(repository, revision)
        .output()
        .expect("launch S1 noesis scan")
}

pub fn scan_command(repository: &Path, revision: &str) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_noesis"));
    command
        .args(["scan", "--repository"])
        .arg(repository)
        .args([
            "--repository-id",
            REPOSITORY_ID,
            "--revision",
            revision,
            "--profile",
            "standard-local-s1",
            "--format",
            "json",
        ]);
    command
}

fn tree_entry(path: &str) -> String {
    let (_, mode, oid) = FILES
        .iter()
        .find(|(candidate, _, _)| *candidate == path)
        .unwrap_or_else(|| panic!("missing S1 fixture tree entry for {path}"));
    let name = path.rsplit('/').next().expect("fixture path basename");
    format!("{mode} blob {oid}\t{name}\n")
}

fn write_tree(worktree: &Path, global_config: &Path, entries: &[String]) -> String {
    let input = entries.concat();
    let mut make_tree = git_command(global_config);
    make_tree.arg("-C").arg(worktree).arg("mktree");
    stdout_line(successful_output(make_tree, Some(input.as_bytes())))
}
