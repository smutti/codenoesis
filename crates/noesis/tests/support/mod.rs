#![allow(
    dead_code,
    reason = "shared integration-test support is compiled once per test target"
)]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

pub const COMMIT_A_OID: &str = "6d4152a7787ac82eedf3f9fc5df408dfdf6e412f";
pub const COMMIT_B_OID: &str = "3217e5245913eef0f8046e9d1a60a410ec0ada97";
pub const TREE_A_OID: &str = "892c4a33b5529ba6b6651fc26765957f11f7ba9e";
pub const TREE_B_OID: &str = "db1a3df9256411212c5013e6e00dcaa090bd5451";
pub const BLOB_A_OID: &str = "b367ffda48249ca59f89ebe73a63a2cc053ebf38";
pub const BLOB_B_OID: &str = "ce8d9f907ed475bcd9b859ddabb81c848da05d34";
pub const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s0-one-file-v1";
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub struct MaterializedRepository {
    pub root: PathBuf,
    pub worktree: PathBuf,
    global_config: PathBuf,
}

impl MaterializedRepository {
    pub fn commit_a() -> Self {
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

        let source = read_repository_text(fixture_root().join("commit-a/main.rs"));
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

        let source_b = read_repository_text(fixture_root().join("commit-b/main.rs"));
        let mut hash_blob_b = git_command(&global_config);
        hash_blob_b
            .arg("-C")
            .arg(&worktree)
            .args(["hash-object", "-w", "--stdin"]);
        let blob_b_oid = stdout_line(successful_output(hash_blob_b, Some(&source_b)));
        assert_eq!(blob_b_oid, BLOB_B_OID, "fixture blob-B identity changed");

        let tree_b_input = format!("100644 blob {blob_b_oid}\tmain.rs\n");
        let mut make_tree_b = git_command(&global_config);
        make_tree_b.arg("-C").arg(&worktree).arg("mktree");
        let tree_b_oid = stdout_line(successful_output(
            make_tree_b,
            Some(tree_b_input.as_bytes()),
        ));
        assert_eq!(tree_b_oid, TREE_B_OID, "fixture tree-B identity changed");

        let mut make_commit_b = git_command(&global_config);
        make_commit_b
            .arg("-C")
            .arg(&worktree)
            .args(["commit-tree", &tree_b_oid, "-p", COMMIT_A_OID, "-F", "-"])
            .env("GIT_AUTHOR_NAME", "CodeNoesis Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_AUTHOR_DATE", "946684801 +0000")
            .env("GIT_COMMITTER_NAME", "CodeNoesis Fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_COMMITTER_DATE", "946684801 +0000");
        let commit_b_oid = stdout_line(successful_output(
            make_commit_b,
            Some(b"fixture: commit B\n"),
        ));
        assert_eq!(
            commit_b_oid, COMMIT_B_OID,
            "fixture commit-B identity changed"
        );

        let mut update_ref = git_command(&global_config);
        update_ref
            .arg("-C")
            .arg(&worktree)
            .args(["update-ref", "refs/heads/main", &commit_oid]);
        successful_output(update_ref, None);

        Self {
            root,
            worktree,
            global_config,
        }
    }

    pub fn update_main(&self, object_id: &str) {
        let mut update_ref = git_command(&self.global_config);
        update_ref
            .arg("-C")
            .arg(&self.worktree)
            .args(["update-ref", "refs/heads/main", object_id]);
        successful_output(update_ref, None);
    }

    pub fn object_path(&self, object_id: &str) -> PathBuf {
        self.worktree
            .join(".git/objects")
            .join(&object_id[..2])
            .join(&object_id[2..])
    }

    pub fn apply_isolation_variant(&self, sentinel: &Path) {
        let hook = self.worktree.join(".git/hooks/post-checkout");
        fs::create_dir_all(hook.parent().expect("hook parent"))
            .expect("create isolation hook directory");
        fs::write(
            &hook,
            "#!/bin/sh\nprintf 'executed\\n' > \"$CODENOESIS_SENTINEL\"\n",
        )
        .expect("write isolation sentinel hook");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;

            fs::set_permissions(&hook, fs::Permissions::from_mode(0o755))
                .expect("make isolation sentinel executable");
        }
        let mut set_remote = git_command(&self.global_config);
        set_remote.arg("-C").arg(&self.worktree).args([
            "config",
            "remote.origin.url",
            "https://127.0.0.1:9/codenoesis-sentinel.git",
        ]);
        successful_output(set_remote, None);
        assert!(!sentinel.exists());
    }
}

impl Drop for MaterializedRepository {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/s0/one-file-v1")
}

pub fn read_repository_text(path: impl AsRef<Path>) -> Vec<u8> {
    let path = path.as_ref();
    let text = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("read repository text {}: {error}", path.display()));
    let normalized = text.replace("\r\n", "\n");
    assert!(
        !normalized.contains('\r'),
        "repository text contains a bare carriage return: {}",
        path.display()
    );
    normalized.into_bytes()
}

pub fn unique_temp_root() -> PathBuf {
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

pub fn scan(repository: &Path, revision: &str) -> Output {
    scan_command(repository, revision)
        .output()
        .expect("launch noesis scan")
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
            "--format",
            "json",
        ]);
    command
}

pub fn parse_single_document(bytes: &[u8]) -> Value {
    assert!(bytes.ends_with(b"\n"), "document must end in one LF");
    assert!(
        !bytes[..bytes.len() - 1].contains(&b'\n'),
        "document must contain exactly one LF"
    );
    serde_json::from_slice(&bytes[..bytes.len() - 1]).expect("parse strict JSON document")
}

pub fn assert_acquisition_error(output: &Output, expected: &Value) {
    assert_eq!(output.status.code(), Some(10));
    assert!(output.stdout.is_empty(), "failed stdout must be empty");
    assert_eq!(&parse_single_document(&output.stderr), expected);
}

pub fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap_or_else(|error| {
        panic!("read JSON {}: {error}", path.display());
    }))
    .unwrap_or_else(|error| panic!("parse JSON {}: {error}", path.display()))
}
