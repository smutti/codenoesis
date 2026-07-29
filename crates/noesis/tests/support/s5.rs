use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use codenoesis_contracts::AnalysisCacheEntryV1;
use rusqlite::Connection;
use serde_json::Value;

use super::{
    git_command, parse_single_document, read_json, read_repository_text, stdout_line,
    successful_output, unique_temp_root,
};

pub const BASELINE_COMMIT_OID: &str = "2106dc8bf32867e519b89ef6d73ce2afdced170d";
pub const TARGET_COMMIT_OID: &str = "c2408412192e61249403bfa1dbc2dce7db0364cc";
pub const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s5-incremental-refresh-v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredHead {
    pub snapshot_id: String,
    pub commit_oid: String,
    pub generation: i64,
}

pub struct MaterializedRepository {
    pub root: PathBuf,
    pub worktree: PathBuf,
    pub store: PathBuf,
    pub cold_store: PathBuf,
    pub documents: PathBuf,
    pub cold_documents: PathBuf,
    pub process_sentinel: PathBuf,
    global_config: PathBuf,
}

impl MaterializedRepository {
    pub fn two_revisions() -> Self {
        let root = unique_temp_root();
        let worktree = root.join("repository");
        let store = root.join("store");
        let cold_store = root.join("cold-store");
        let documents = root.join("documents");
        let cold_documents = root.join("cold-documents");
        let process_sentinel = root.join("process-executed");
        let template = root.join("template");
        let global_config = root.join("global.gitconfig");
        fs::create_dir_all(&template).expect("create empty Git template directory");
        fs::write(&global_config, []).expect("create empty global Git configuration");

        let mut init = git_command(&global_config);
        init.args(["init", "--quiet", "--initial-branch=main"])
            .arg(format!("--template={}", template.display()))
            .arg(&worktree);
        successful_output(init, None);

        let manifest = read_json(&fixture_root().join("manifest.json"));
        for revision in manifest["revisions"]
            .as_array()
            .expect("S5 fixture revisions")
        {
            materialize_revision(&worktree, &global_config, revision);
        }

        let mut update_ref = git_command(&global_config);
        update_ref.arg("-C").arg(&worktree).args([
            "update-ref",
            "refs/heads/main",
            BASELINE_COMMIT_OID,
        ]);
        successful_output(update_ref, None);

        let hook = worktree.join(".git/hooks/post-checkout");
        fs::create_dir_all(hook.parent().expect("S5 hook parent"))
            .expect("create S5 hook directory");
        fs::write(
            &hook,
            "#!/bin/sh\nprintf 'executed\\n' > \"$CODENOESIS_SENTINEL\"\n",
        )
        .expect("write S5 Git-process sentinel");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;

            fs::set_permissions(&hook, fs::Permissions::from_mode(0o755))
                .expect("make S5 hook executable");
        }
        let mut set_remote = git_command(&global_config);
        set_remote.arg("-C").arg(&worktree).args([
            "config",
            "remote.origin.url",
            "https://127.0.0.1:9/codenoesis-s5-sentinel.git",
        ]);
        successful_output(set_remote, None);

        Self {
            root,
            worktree,
            store,
            cold_store,
            documents,
            cold_documents,
            process_sentinel,
            global_config,
        }
    }

    pub fn baseline_scan(&self) -> Output {
        self.scan(BASELINE_COMMIT_OID, &self.store)
    }

    pub fn baseline_s3_scan(&self) -> Output {
        let cargo = b"[package]\nname = \"s5-incompatible-baseline\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[lib]\npath = \"src/lib.rs\"\n";
        let source = b"pub fn baseline_only() {}\n";
        let cargo_oid = self.write_git_blob(cargo);
        let source_oid = self.write_git_blob(source);
        let src_tree = self.write_git_tree(&format!("100644 blob {source_oid}\tlib.rs\n"));
        let root_tree = self.write_git_tree(&format!(
            "100644 blob {cargo_oid}\tCargo.toml\n040000 tree {src_tree}\tsrc\n"
        ));
        let mut commit = git_command(&self.global_config);
        commit
            .arg("-C")
            .arg(&self.worktree)
            .args(["commit-tree", &root_tree, "-F", "-"])
            .env("GIT_AUTHOR_NAME", "CodeNoesis Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_AUTHOR_DATE", "946684802 +0000")
            .env("GIT_COMMITTER_NAME", "CodeNoesis Fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_COMMITTER_DATE", "946684802 +0000");
        let commit_oid = stdout_line(successful_output(
            commit,
            Some(b"fixture: incompatible S3 baseline\n"),
        ));
        self.scan_with_profile(&commit_oid, &self.store, "standard-local-s3")
    }

    pub fn cold_target_scan(&self) -> Output {
        self.scan(TARGET_COMMIT_OID, &self.cold_store)
    }

    pub fn refresh(&self) -> Output {
        self.refresh_with_profile("standard-local-s5")
    }

    pub fn refresh_with_profile(&self, profile: &str) -> Output {
        let mut command = self.noesis_command();
        command
            .args(["refresh", "--repository"])
            .arg(&self.worktree)
            .args([
                "--repository-id",
                REPOSITORY_ID,
                "--revision",
                TARGET_COMMIT_OID,
                "--store",
            ])
            .arg(&self.store)
            .args(["--profile", profile, "--format", "json"]);
        command.output().expect("launch S5 noesis refresh")
    }

    pub fn docs(&self, store: &Path, output: &Path) -> Output {
        let mut command = self.noesis_command();
        command
            .args(["docs", "--store"])
            .arg(store)
            .args(["--repository-id", REPOSITORY_ID, "--output"])
            .arg(output)
            .args(["--format", "json"]);
        command.output().expect("launch S5 comparison docs")
    }

    pub fn stored_head(&self, store: &Path) -> StoredHead {
        assert!(
            store.starts_with(&self.root),
            "S5 store escaped fixture root"
        );
        let connection =
            Connection::open(store.join("metadata.sqlite3")).expect("open S5 metadata");
        connection
            .query_row(
                "SELECT s.snapshot_id, s.commit_oid, h.generation
                 FROM project_heads h
                 JOIN snapshots s ON s.snapshot_id = h.snapshot_id
                 WHERE h.repository_identity = ?1",
                [REPOSITORY_ID],
                |row| {
                    Ok(StoredHead {
                        snapshot_id: row.get(0)?,
                        commit_oid: row.get(1)?,
                        generation: row.get(2)?,
                    })
                },
            )
            .expect("load S5 visible head")
    }

    pub fn stored_snapshot_semantic(&self, store: &Path) -> Vec<u8> {
        assert!(
            store.starts_with(&self.root),
            "S5 store escaped fixture root"
        );
        let connection =
            Connection::open(store.join("metadata.sqlite3")).expect("open S5 metadata");
        let artifact_id = connection
            .query_row(
                "SELECT a.artifact_id
                 FROM project_heads h
                 JOIN snapshot_artifacts sa ON sa.snapshot_id = h.snapshot_id
                 JOIN artifacts a ON a.artifact_id = sa.artifact_id
                 WHERE h.repository_identity = ?1
                   AND sa.role = 'snapshot_semantic'",
                [REPOSITORY_ID],
                |row| row.get::<_, String>(0),
            )
            .expect("load S5 snapshot semantic artifact");
        let digest = artifact_id
            .strip_prefix("urn:codenoesis:artifact:blake3:")
            .expect("S5 artifact identity");
        fs::read(
            store
                .join("objects/blake3")
                .join(&digest[..2])
                .join(&digest[2..]),
        )
        .expect("read S5 snapshot semantic bytes")
    }

    pub fn corrupt_first_analysis_cache_entry(&self) {
        fs::write(self.first_analysis_cache_entry_path(), b"{}")
            .expect("corrupt S5 analysis-cache entry");
    }

    pub fn poison_first_analysis_cache_entry(&self) {
        let path = self.first_analysis_cache_entry_path();
        let bytes = fs::read(&path).expect("read S5 cache entry to poison");
        let mut entry = AnalysisCacheEntryV1::parse(&bytes)
            .expect("parse S5 cache entry to poison")
            .to_domain()
            .expect("convert S5 cache entry to poison");
        entry.analysis.unsupported_construct = !entry.analysis.unsupported_construct;
        let poisoned = AnalysisCacheEntryV1::from_domain(&entry)
            .canonical_bytes()
            .expect("serialize poisoned S5 cache entry");
        fs::write(path, poisoned).expect("write poisoned S5 cache entry");
    }

    pub fn remove_analysis_cache(&self) {
        fs::remove_dir_all(self.store.join("objects/analysis-cache-entry-v1"))
            .expect("remove S5 analysis cache");
    }

    pub fn add_incompatible_analysis_cache_entry(&self) {
        let digest = "d".repeat(64);
        let path = self
            .store
            .join("objects/analysis-cache-entry-v1")
            .join(&digest[..2])
            .join(&digest[2..]);
        fs::create_dir_all(path.parent().expect("S5 incompatible cache shard"))
            .expect("create S5 incompatible cache shard");
        let value = serde_json::json!({
            "schema_version": "codenoesis.analysis-cache-entry/v1",
            "repository_identity": REPOSITORY_ID,
            "source": {
                "path": "crates/app/src/main.rs",
                "blob_oid": "7e6f91e233eff28c5511d686f7c46f67dd919505"
            },
            "versions": {
                "language_extractor": "codenoesis.rust-tree-sitter/s4-v0",
                "workspace_mapper": "codenoesis.rust-workspace/s4-v1",
                "normalization": "codenoesis.normalization/rust-workspace/v1",
                "ontology": "codenoesis.ontology/rust/v2",
                "extraction_contract": "codenoesis.extraction/v2",
                "semantic_profile": "standard-local-s4",
                "dependency_rules": "codenoesis.incremental-rules/rust-workspace-v1"
            }
        });
        fs::write(
            path,
            serde_json::to_vec(&value).expect("serialize S5 incompatible cache"),
        )
        .expect("write S5 incompatible cache");
    }

    fn scan(&self, revision: &str, store: &Path) -> Output {
        self.scan_with_profile(revision, store, "standard-local-s4")
    }

    fn scan_with_profile(&self, revision: &str, store: &Path, profile: &str) -> Output {
        let mut command = self.noesis_command();
        command
            .args(["scan", "--repository"])
            .arg(&self.worktree)
            .args([
                "--repository-id",
                REPOSITORY_ID,
                "--revision",
                revision,
                "--profile",
                profile,
                "--store",
            ])
            .arg(store)
            .args(["--format", "json"]);
        command.output().expect("launch S5 fixture S4 scan")
    }

    fn first_analysis_cache_entry_path(&self) -> PathBuf {
        let cache_root = self.store.join("objects/analysis-cache-entry-v1");
        let mut shards = fs::read_dir(&cache_root)
            .expect("read S5 analysis cache")
            .map(|entry| entry.expect("read S5 analysis-cache shard"))
            .collect::<Vec<_>>();
        shards.sort_by_key(std::fs::DirEntry::file_name);
        let mut entries = fs::read_dir(shards.first().expect("S5 analysis-cache shard").path())
            .expect("read S5 analysis-cache shard")
            .map(|entry| entry.expect("read S5 analysis-cache entry"))
            .collect::<Vec<_>>();
        entries.sort_by_key(std::fs::DirEntry::file_name);
        entries.first().expect("S5 analysis-cache entry").path()
    }

    fn write_git_blob(&self, bytes: &[u8]) -> String {
        let mut command = git_command(&self.global_config);
        command
            .arg("-C")
            .arg(&self.worktree)
            .args(["hash-object", "-w", "--stdin"]);
        stdout_line(successful_output(command, Some(bytes)))
    }

    fn write_git_tree(&self, entries: &str) -> String {
        let mut command = git_command(&self.global_config);
        command.arg("-C").arg(&self.worktree).arg("mktree");
        stdout_line(successful_output(command, Some(entries.as_bytes())))
    }

    fn noesis_command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_noesis"));
        command
            .env("CODENOESIS_SENTINEL", &self.process_sentinel)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", &self.global_config)
            .env("GIT_TERMINAL_PROMPT", "0");
        command
    }
}

impl Drop for MaterializedRepository {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/s5/incremental-refresh-v1")
}

pub fn expected_report_bytes() -> Vec<u8> {
    let report = read_json(&fixture_root().join("expected-incremental-refresh-report.json"));
    let mut bytes = serde_json::to_vec(&report).expect("serialize canonical S5 report");
    bytes.push(b'\n');
    bytes
}

pub fn canonical_semantic_from_scan(output: &Output) -> Vec<u8> {
    let snapshot = parse_single_document(&output.stdout);
    serde_json::to_vec(&snapshot["semantic"]).expect("serialize cold S5 semantic")
}

pub fn owned_document_bytes(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();
    collect_files(root, root, &mut files);
    files
}

fn materialize_revision(worktree: &Path, global_config: &Path, revision: &Value) {
    let name = revision["name"].as_str().expect("S5 revision name");
    for file in revision["files"].as_array().expect("S5 revision files") {
        let relative = file["path"].as_str().expect("S5 file path");
        let bytes = read_repository_text(fixture_root().join(name).join(relative));
        let mut hash_blob = git_command(global_config);
        hash_blob
            .arg("-C")
            .arg(worktree)
            .args(["hash-object", "-w", "--stdin"]);
        let observed = stdout_line(successful_output(hash_blob, Some(&bytes)));
        assert_eq!(
            observed,
            file["git_blob_oid"].as_str().expect("S5 blob identity"),
            "S5 blob identity changed for {name}/{relative}"
        );
    }

    for tree in revision["trees"]
        .as_array()
        .expect("S5 revision trees")
        .iter()
        .rev()
    {
        let entries = tree["entries"]
            .as_array()
            .expect("S5 tree entries")
            .iter()
            .fold(String::new(), |mut entries, entry| {
                let mode = entry["mode"].as_str().expect("S5 tree mode");
                let kind = if mode == "040000" { "tree" } else { "blob" };
                writeln!(
                    entries,
                    "{mode} {kind} {}\t{}",
                    entry["oid"].as_str().expect("S5 tree entry oid"),
                    entry["name"].as_str().expect("S5 tree entry name")
                )
                .expect("write S5 tree entry");
                entries
            });
        let mut make_tree = git_command(global_config);
        make_tree.arg("-C").arg(worktree).arg("mktree");
        let observed = stdout_line(successful_output(make_tree, Some(entries.as_bytes())));
        assert_eq!(
            observed,
            tree["oid"].as_str().expect("S5 tree identity"),
            "S5 tree identity changed for {name}/{}",
            tree["path"].as_str().expect("S5 tree path")
        );
    }

    let commit = &revision["commit"];
    let mut hash_commit = git_command(global_config);
    hash_commit
        .arg("-C")
        .arg(worktree)
        .args(["hash-object", "-t", "commit", "-w", "--stdin"]);
    let observed = stdout_line(successful_output(
        hash_commit,
        Some(
            commit["payload_utf8"]
                .as_str()
                .expect("S5 commit payload")
                .as_bytes(),
        ),
    ));
    assert_eq!(
        observed,
        commit["oid"].as_str().expect("S5 commit identity"),
        "S5 commit identity changed for {name}"
    );
}

fn collect_files(root: &Path, current: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
    let mut entries = fs::read_dir(current)
        .unwrap_or_else(|error| panic!("read generated directory {}: {error}", current.display()))
        .map(|entry| entry.expect("read generated entry"))
        .collect::<Vec<_>>();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let metadata = entry.metadata().expect("read generated metadata");
        if metadata.is_dir() {
            collect_files(root, &path, files);
        } else {
            let relative = path
                .strip_prefix(root)
                .expect("generated relative path")
                .to_string_lossy()
                .replace('\\', "/");
            files.insert(relative, fs::read(path).expect("read generated file"));
        }
    }
}
