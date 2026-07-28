use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use super::{git_command, read_repository_text, stdout_line, successful_output, unique_temp_root};

pub const COMMIT_A_OID: &str = "c09d8c24e4704036c31b4f42e2f4df6e4acd347f";
pub const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s4-workspace-docs-v1";
pub const ROOT_TREE_OID: &str = "8f9e36122bec5caac5dc0f739ea7ab4c830bd356";
pub const ENTITY_QUERY_ID: &str =
    "urn:codenoesis:entity:blake3:18bd97153a41f322136e8f93573877bfc1a2f43fabc6564b789e979a6cffcafa";
pub const DOCUMENT_QUERY_ID: &str = "urn:codenoesis:document:blake3:4d6e3c7a3442af14614b515c93197302e23744d004ec671abc4bbab4ae402a99";
pub const UNKNOWN_QUERY_ID: &str =
    "urn:codenoesis:entity:blake3:0000000000000000000000000000000000000000000000000000000000000000";

const ROOT_MANIFEST_BLOB: &str = "9a61bc964dd0b80e54880d0a40471b50324b6e15";
const APP_MANIFEST_BLOB: &str = "887b09de92b06c4003daf2ffc60a48e33aa1187f";
const APP_BUILD_BLOB: &str = "817cc01b6a06743e810217ca57f9abfbbdf34824";
const APP_MAIN_BLOB: &str = "7e6f91e233eff28c5511d686f7c46f67dd919505";
const MODEL_MANIFEST_BLOB: &str = "bd9cc68a4885e014ce4a034be8c2317425f3e3dc";
const MODEL_ITEM_BLOB: &str = "885b4746097a67e5c4fb997a2082597dc23699e6";
const MODEL_LIB_BLOB: &str = "f001952aa75e3c631153f1fe5dec0af6ca11c429";

pub struct MaterializedRepository {
    pub root: PathBuf,
    pub worktree: PathBuf,
    pub store: PathBuf,
    pub documents: PathBuf,
}

impl MaterializedRepository {
    #[allow(clippy::too_many_lines)]
    pub fn revision_a() -> Self {
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

        assert_blob(
            &worktree,
            &global_config,
            "revision-a/Cargo.toml",
            ROOT_MANIFEST_BLOB,
        );
        assert_blob(
            &worktree,
            &global_config,
            "revision-a/crates/app/Cargo.toml",
            APP_MANIFEST_BLOB,
        );
        assert_blob(
            &worktree,
            &global_config,
            "revision-a/crates/app/build.rs",
            APP_BUILD_BLOB,
        );
        assert_blob(
            &worktree,
            &global_config,
            "revision-a/crates/app/src/main.rs",
            APP_MAIN_BLOB,
        );
        assert_blob(
            &worktree,
            &global_config,
            "revision-a/crates/model/Cargo.toml",
            MODEL_MANIFEST_BLOB,
        );
        assert_blob(
            &worktree,
            &global_config,
            "revision-a/crates/model/src/item.rs",
            MODEL_ITEM_BLOB,
        );
        assert_blob(
            &worktree,
            &global_config,
            "revision-a/crates/model/src/lib.rs",
            MODEL_LIB_BLOB,
        );

        let app_source_tree = write_tree(
            &worktree,
            &global_config,
            &format!("100644 blob {APP_MAIN_BLOB}\tmain.rs\n"),
        );
        assert_eq!(
            app_source_tree, "747d120659c476bb5a0af68f3e73264a0d1936fa",
            "S4 app source tree identity changed"
        );
        let app_tree = write_tree(
            &worktree,
            &global_config,
            &format!(
                "100644 blob {APP_MANIFEST_BLOB}\tCargo.toml\n100644 blob {APP_BUILD_BLOB}\tbuild.rs\n040000 tree {app_source_tree}\tsrc\n"
            ),
        );
        assert_eq!(
            app_tree, "839c54592804c3786eaa921b3fd2741fe23f052d",
            "S4 app tree identity changed"
        );

        let model_source_tree = write_tree(
            &worktree,
            &global_config,
            &format!(
                "100644 blob {MODEL_ITEM_BLOB}\titem.rs\n100644 blob {MODEL_LIB_BLOB}\tlib.rs\n"
            ),
        );
        assert_eq!(
            model_source_tree, "8625cf0eab213b81775a83ad25be14e041d9be68",
            "S4 model source tree identity changed"
        );
        let model_tree = write_tree(
            &worktree,
            &global_config,
            &format!(
                "100644 blob {MODEL_MANIFEST_BLOB}\tCargo.toml\n040000 tree {model_source_tree}\tsrc\n"
            ),
        );
        assert_eq!(
            model_tree, "836d7e3d8b09c19bab01fdc5ba8cf46263799aad",
            "S4 model tree identity changed"
        );

        let crates_tree = write_tree(
            &worktree,
            &global_config,
            &format!("040000 tree {app_tree}\tapp\n040000 tree {model_tree}\tmodel\n"),
        );
        assert_eq!(
            crates_tree, "3e03df22f6961d79f831318ae35e06fa026ad599",
            "S4 crates tree identity changed"
        );
        let root_tree = write_tree(
            &worktree,
            &global_config,
            &format!(
                "100644 blob {ROOT_MANIFEST_BLOB}\tCargo.toml\n040000 tree {crates_tree}\tcrates\n"
            ),
        );
        assert_eq!(root_tree, ROOT_TREE_OID, "S4 root tree identity changed");

        let mut make_commit = git_command(&global_config);
        make_commit
            .arg("-C")
            .arg(&worktree)
            .args(["commit-tree", ROOT_TREE_OID, "-F", "-"])
            .env("GIT_AUTHOR_NAME", "CodeNoesis Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_AUTHOR_DATE", "1785110400 +0000")
            .env("GIT_COMMITTER_NAME", "CodeNoesis Fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_COMMITTER_DATE", "1785110400 +0000");
        let commit_oid = stdout_line(successful_output(
            make_commit,
            Some(b"CodeNoesis S4 workspace docs fixture revision A\n"),
        ));
        assert_eq!(commit_oid, COMMIT_A_OID, "S4 commit identity changed");

        let mut update_ref = git_command(&global_config);
        update_ref
            .arg("-C")
            .arg(&worktree)
            .args(["update-ref", "refs/heads/main", COMMIT_A_OID]);
        successful_output(update_ref, None);

        Self {
            root,
            worktree,
            store,
            documents,
        }
    }
}

impl Drop for MaterializedRepository {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/s4/workspace-docs-v1")
}

pub fn scan(repository: &MaterializedRepository) -> Output {
    Command::new(env!("CARGO_BIN_EXE_noesis"))
        .args(["scan", "--repository"])
        .arg(&repository.worktree)
        .args([
            "--repository-id",
            REPOSITORY_ID,
            "--revision",
            COMMIT_A_OID,
            "--profile",
            "standard-local-s4",
            "--store",
        ])
        .arg(&repository.store)
        .args(["--format", "json"])
        .output()
        .expect("launch S4 noesis scan")
}

pub fn docs(repository: &MaterializedRepository) -> Output {
    Command::new(env!("CARGO_BIN_EXE_noesis"))
        .args(["docs", "--store"])
        .arg(&repository.store)
        .args(["--repository-id", REPOSITORY_ID, "--output"])
        .arg(&repository.documents)
        .args(["--format", "json"])
        .output()
        .expect("launch S4 noesis docs")
}

pub fn query(repository: &MaterializedRepository, stable_id: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_noesis"))
        .args(["query", "--store"])
        .arg(&repository.store)
        .args(["--repository-id", REPOSITORY_ID, "--documents"])
        .arg(&repository.documents)
        .args(["--id", stable_id, "--format", "json"])
        .output()
        .expect("launch S4 noesis query")
}

fn assert_blob(worktree: &Path, global_config: &Path, relative: &str, expected: &str) {
    let bytes = read_repository_text(fixture_root().join(relative));
    let mut command = git_command(global_config);
    command
        .arg("-C")
        .arg(worktree)
        .args(["hash-object", "-w", "--stdin"]);
    let observed = stdout_line(successful_output(command, Some(&bytes)));
    assert_eq!(
        observed, expected,
        "S4 blob identity changed for {relative}"
    );
}

fn write_tree(worktree: &Path, global_config: &Path, entries: &str) -> String {
    let mut command = git_command(global_config);
    command.arg("-C").arg(worktree).arg("mktree");
    stdout_line(successful_output(command, Some(entries.as_bytes())))
}
