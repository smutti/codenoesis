use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
#[cfg(windows)]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(windows)]
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;
use sha2::{Digest as _, Sha256};

use super::{git_command, read_repository_text, stdout_line, successful_output, unique_temp_root};

pub const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s4-rust-callable-semantics-v1";
pub const FIXTURE_TREE_OID: &str = "c43f0c6e91c8e3e27abbba94cdd666d6c3598414";
pub const FIXTURE_COMMIT_OID: &str = "637091858d6582fbe7f0c75b7c62d4fd9c2d87ca";
pub const R10_PROFILE: &str = "rust-cfg-declaration-alternatives-v1";
pub const R6_PROFILE: &str = "rust-framework-declarations-v1";
pub const K1_PROFILE: &str = "rust-callable-semantics-v1";
#[cfg(windows)]
static R12_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub struct MaterializedCallableCfgAlternativesRepository {
    pub root: PathBuf,
    pub worktree: PathBuf,
    pub store: PathBuf,
    pub documents: PathBuf,
    pub portable: PathBuf,
    pub explorer: PathBuf,
    pub commit_oid: String,
}

impl MaterializedCallableCfgAlternativesRepository {
    pub fn fixture() -> Self {
        let fixture = fixture_root();
        let manifest: Value = serde_json::from_slice(
            &fs::read(fixture.join("manifest.json")).expect("read R12 fixture manifest"),
        )
        .expect("parse R12 fixture manifest");
        assert_eq!(manifest["repository_identity"], REPOSITORY_ID);

        let root = r12_temp_root();
        let worktree = root.join("repository");
        let store = root.join("store");
        let documents = root.join("documents");
        let portable = root.join("portable");
        let explorer = root.join("explorer");
        let template = root.join("template");
        let global_config = root.join("global.gitconfig");
        fs::create_dir_all(&template).expect("create empty R12 Git template");
        fs::write(&global_config, []).expect("create empty R12 Git configuration");

        let mut init = git_command(&global_config);
        init.args(["init", "--quiet", "--initial-branch=main"])
            .arg(format!("--template={}", template.display()))
            .arg(&worktree);
        successful_output(init, None);

        let reviewed = manifest["files"]
            .as_array()
            .expect("reviewed R12 files")
            .iter()
            .map(|file| (file["path"].as_str().expect("reviewed R12 path"), file))
            .collect::<BTreeMap<_, _>>();
        for (fixture_path, file) in reviewed {
            let relative = fixture_path
                .strip_prefix("repository/")
                .expect("R12 repository-relative fixture path");
            let bytes = read_repository_text(fixture.join(fixture_path));
            assert_reviewed_bytes(file, &bytes, fixture_path);
            let destination = worktree.join(relative);
            fs::create_dir_all(destination.parent().expect("R12 fixture parent"))
                .expect("create R12 fixture parent");
            fs::write(&destination, &bytes).expect("materialize R12 source");

            let mut hash = git_command(&global_config);
            hash.arg("-C")
                .arg(&worktree)
                .args(["hash-object", "-w", "--stdin"]);
            let blob_oid = stdout_line(successful_output(hash, Some(&bytes)));
            assert_eq!(
                blob_oid,
                file["git_blob_oid"].as_str().expect("reviewed R12 blob"),
                "R12 fixture Git blob changed for {fixture_path}"
            );
            update_index(&worktree, &global_config, &blob_oid, relative);
        }

        let mut write_tree = git_command(&global_config);
        write_tree.arg("-C").arg(&worktree).arg("write-tree");
        let tree_oid = stdout_line(successful_output(write_tree, None));
        assert_eq!(tree_oid, FIXTURE_TREE_OID, "R12 fixture tree changed");

        let mut make_commit = git_command(&global_config);
        make_commit
            .arg("-C")
            .arg(&worktree)
            .args(["commit-tree", &tree_oid, "-F", "-"])
            .env("GIT_AUTHOR_NAME", "CodeNoesis")
            .env("GIT_AUTHOR_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_AUTHOR_DATE", "2026-08-10T06:00:00Z")
            .env("GIT_COMMITTER_NAME", "CodeNoesis")
            .env("GIT_COMMITTER_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_COMMITTER_DATE", "2026-08-10T06:00:00Z");
        let commit_oid = stdout_line(successful_output(
            make_commit,
            Some(b"R12 project-owned callable cfg alternatives fixture\n"),
        ));
        assert_eq!(commit_oid, FIXTURE_COMMIT_OID, "R12 fixture commit changed");

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
        self.scan_with_extra(&[])
    }

    pub fn scan_with_extra(&self, extra: &[&str]) -> Output {
        let mut command = self.scan_command();
        command.args(extra);
        command.output().expect("launch R12 scan")
    }

    fn scan_command(&self) -> Command {
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
                R10_PROFILE,
                "--rust-framework-profile",
                R6_PROFILE,
                "--rust-callable-profile",
                K1_PROFILE,
            ])
            .arg("--store")
            .arg(&self.store)
            .args(["--format", "json"]);
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
            .expect("launch R12 docs")
    }

    pub fn query(&self, requested_id: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .args(["query", "--store"])
            .arg(&self.store)
            .args(["--repository-id", REPOSITORY_ID, "--documents"])
            .arg(&self.documents)
            .args(["--id", requested_id, "--format", "json"])
            .output()
            .expect("launch R12 query")
    }

    pub fn export(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .args(["export", "--store"])
            .arg(&self.store)
            .args(["--repository-id", REPOSITORY_ID, "--output"])
            .arg(&self.portable)
            .args(["--portable-profile", K1_PROFILE, "--format", "json"])
            .output()
            .expect("launch R12 export")
    }

    pub fn explore(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .args(["explore", "--input"])
            .arg(self.portable.join("portable-graph.json"))
            .arg("--output")
            .arg(&self.explorer)
            .args(["--explorer-profile", K1_PROFILE, "--format", "json"])
            .output()
            .expect("launch R12 explore")
    }

    pub fn build_sentinel(&self) -> PathBuf {
        self.worktree.join("K1_BUILD_SENTINEL_EXECUTED")
    }
}

impl Drop for MaterializedCallableCfgAlternativesRepository {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub fn expected_composition() -> Value {
    serde_json::from_slice(
        &fs::read(fixture_root().join("expected-composition.json"))
            .expect("read R12 expected composition"),
    )
    .expect("parse R12 expected composition")
}

pub fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/s4/rust-callable-cfg-alternatives-v1")
}

#[cfg(not(windows))]
fn r12_temp_root() -> PathBuf {
    fs::canonicalize(unique_temp_root()).expect("canonicalize R12 temporary root")
}

#[cfg(windows)]
fn r12_temp_root() -> PathBuf {
    let workspace = std::env::current_dir().expect("resolve R12 E2E workspace");
    let mut candidates = vec![workspace.join("target")];
    if let Some(volume_root) = workspace.ancestors().last()
        && candidates.iter().all(|candidate| candidate != volume_root)
    {
        candidates.push(volume_root.to_path_buf());
    }
    let sequence = R12_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("R12 E2E clock follows Unix epoch")
        .as_nanos();
    for candidate in candidates {
        let root = candidate.join(format!(
            "codenoesis-r12-e2e-{}-{timestamp}-{sequence}",
            std::process::id()
        ));
        if fs::create_dir(&root).is_err() {
            continue;
        }
        let authority = root.join("authority-probe");
        if fs::create_dir(&authority).is_ok()
            && noesis::portable_explorer::validate_r11_export_output_root(
                &authority,
                &root.join("output-probe"),
            )
            .is_ok()
            && fs::remove_dir(&authority).is_ok()
        {
            return root;
        }
        let _ = fs::remove_dir_all(&root);
    }
    panic!("no validated R12 E2E test authority")
}

fn update_index(repository: &Path, global_config: &Path, oid: &str, path: &str) {
    let mut update = git_command(global_config);
    update
        .arg("-C")
        .arg(repository)
        .args(["update-index", "--add", "--cacheinfo"])
        .arg(format!("100644,{oid},{path}"));
    successful_output(update, None);
}

fn assert_reviewed_bytes(file: &Value, bytes: &[u8], path: &str) {
    assert_eq!(
        u64::try_from(bytes.len()).expect("R12 fixture byte length"),
        file["byte_length"].as_u64().expect("reviewed R12 length"),
        "R12 source length changed for {path}"
    );
    assert_eq!(
        lower_hex(&Sha256::digest(bytes)),
        file["sha256"].as_str().expect("reviewed R12 SHA-256"),
        "R12 source digest changed for {path}"
    );
}

fn lower_hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}").expect("write R12 digest");
    }
    encoded
}
