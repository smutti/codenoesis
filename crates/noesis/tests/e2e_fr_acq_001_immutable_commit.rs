use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const COMMIT_A_OID: &str = "6d4152a7787ac82eedf3f9fc5df408dfdf6e412f";
const TREE_A_OID: &str = "892c4a33b5529ba6b6651fc26765957f11f7ba9e";
const BLOB_A_OID: &str = "b367ffda48249ca59f89ebe73a63a2cc053ebf38";
const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s0-one-file-v1";
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct MaterializedRepository {
    root: PathBuf,
    worktree: PathBuf,
}

impl MaterializedRepository {
    fn commit_a() -> Self {
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

        let source = fs::read(fixture_root().join("commit-a/main.rs"))
            .expect("read the reviewed commit-A source fixture");
        fs::write(worktree.join("main.rs"), &source).expect("write the materialized worktree file");

        let mut hash_blob = git_command(&global_config);
        hash_blob
            .arg("-C")
            .arg(&worktree)
            .args(["hash-object", "-w", "--stdin"]);
        let blob_oid = stdout_line(successful_output(hash_blob, Some(&source)));
        assert_eq!(blob_oid, BLOB_A_OID, "fixture blob identity changed");

        let tree_input = format!("100644 blob {blob_oid}\tmain.rs\n");
        let mut make_tree = git_command(&global_config);
        make_tree.arg("-C").arg(&worktree).arg("mktree");
        let tree_oid = stdout_line(successful_output(make_tree, Some(tree_input.as_bytes())));
        assert_eq!(tree_oid, TREE_A_OID, "fixture tree identity changed");

        let mut make_commit = git_command(&global_config);
        make_commit
            .arg("-C")
            .arg(&worktree)
            .args(["commit-tree", &tree_oid, "-F", "-"])
            .env("GIT_AUTHOR_NAME", "CodeNoesis Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_AUTHOR_DATE", "946684800 +0000")
            .env("GIT_COMMITTER_NAME", "CodeNoesis Fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_COMMITTER_DATE", "946684800 +0000");
        let commit_oid = stdout_line(successful_output(make_commit, Some(b"fixture: commit A\n")));
        assert_eq!(commit_oid, COMMIT_A_OID, "fixture commit identity changed");

        let mut update_ref = git_command(&global_config);
        update_ref
            .arg("-C")
            .arg(&worktree)
            .args(["update-ref", "refs/heads/main", &commit_oid]);
        successful_output(update_ref, None);

        Self { root, worktree }
    }
}

impl Drop for MaterializedRepository {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/s0/one-file-v1")
}

fn unique_temp_root() -> PathBuf {
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after the Unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "codenoesis-s0-{}-{timestamp}-{sequence}",
        std::process::id()
    ));
    fs::create_dir(&root).expect("create isolated S0 fixture root");
    root
}

fn git_command(global_config: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", global_config)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE");
    command
}

fn successful_output(mut command: Command, input: Option<&[u8]>) -> Output {
    let invocation = format!("{command:?}");
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    if input.is_some() {
        command.stdin(Stdio::piped());
    }

    let mut child = command.spawn().expect("launch fixture Git command");
    if let Some(content) = input {
        child
            .stdin
            .take()
            .expect("fixture Git command stdin")
            .write_all(content)
            .expect("write fixture Git command stdin");
    }
    let output = child
        .wait_with_output()
        .expect("wait for fixture Git command");
    assert!(
        output.status.success(),
        "fixture command failed: {invocation}; stdout={:?}; stderr={:?}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn stdout_line(output: Output) -> String {
    String::from_utf8(output.stdout)
        .expect("fixture Git output must be UTF-8")
        .trim_end_matches(['\r', '\n'])
        .to_owned()
}

#[test]
fn e2e_fr_acq_001_immutable_commit() {
    let repository = MaterializedRepository::commit_a();
    let output = Command::new(env!("CARGO_BIN_EXE_noesis"))
        .args(["scan", "--repository"])
        .arg(&repository.worktree)
        .args([
            "--repository-id",
            REPOSITORY_ID,
            "--revision",
            COMMIT_A_OID,
            "--format",
            "json",
        ])
        .output()
        .expect("launch noesis scan");

    assert_eq!(
        output.status.code(),
        Some(0),
        "expected subject exit 0 with RepositorySnapshotV1; observed subject exit {:?}; stdout={:?}; stderr={:?}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "successful stderr must be empty");
    assert!(output.stdout.ends_with(b"\n"), "stdout must end in one LF");
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("\"schema_version\":\"codenoesis.repository-snapshot/v1\""),
        "stdout must contain RepositorySnapshotV1"
    );
}
