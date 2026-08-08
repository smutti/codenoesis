use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use sha2::{Digest as _, Sha256};

use super::{git_command, read_repository_text, stdout_line, successful_output, unique_temp_root};

pub const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s4-compiler-index-v1";
pub const FIXTURE_TREE_OID: &str = "d117f2f0924cbef9e7396b97ee46c76bd5261e00";
pub const FIXTURE_COMMIT_OID: &str = "2203600cce0f0904aefc66dcb49dd0dbc7fd5fd3";
pub const BINDING_RELATIVE_PATH: &str = "compiler-index/compiler-index-binding.json";

pub struct MaterializedCompilerIndexRepository {
    pub root: PathBuf,
    pub worktree: PathBuf,
    pub store: PathBuf,
    pub documents: PathBuf,
    pub commit_oid: String,
}

impl MaterializedCompilerIndexRepository {
    pub fn fixture() -> Self {
        Self::fixture_in(unique_temp_root())
    }

    pub fn fixture_in(root: PathBuf) -> Self {
        let fixture = fixture_root();
        let fixture_manifest: Value = serde_json::from_slice(
            &fs::read(fixture.join("manifest.json")).expect("read R7 fixture manifest"),
        )
        .expect("parse R7 fixture manifest");
        assert_eq!(fixture_manifest["repository_identity"], REPOSITORY_ID);

        let worktree = root.join("repository");
        let store = root.join("store");
        let documents = root.join("documents");
        let compiler_index = root.join("compiler-index");
        let template = root.join("template");
        let global_config = root.join("global.gitconfig");
        fs::create_dir_all(&template).expect("create empty Git template directory");
        fs::create_dir_all(&compiler_index).expect("create compiler-index input directory");
        fs::write(&global_config, []).expect("create empty global Git configuration");

        let mut init = git_command(&global_config);
        init.args(["init", "--quiet", "--initial-branch=main"])
            .arg(format!("--template={}", template.display()))
            .arg(&worktree);
        successful_output(init, None);

        let files = fixture_manifest["files"]
            .as_array()
            .expect("reviewed R7 fixture files");
        let reviewed = files
            .iter()
            .map(|file| (file["path"].as_str().expect("reviewed fixture path"), file))
            .collect::<BTreeMap<_, _>>();

        for (fixture_path, file) in reviewed
            .iter()
            .filter(|(path, _)| path.starts_with("repository/"))
        {
            let destination = fixture_path
                .strip_prefix("repository/")
                .expect("R7 repository-relative fixture path");
            let bytes = read_repository_text(fixture.join(fixture_path));
            assert_reviewed_bytes(file, &bytes, fixture_path);
            let destination_path = worktree.join(destination);
            fs::create_dir_all(
                destination_path
                    .parent()
                    .expect("fixture destination parent"),
            )
            .expect("create fixture destination parent");
            fs::write(&destination_path, &bytes).expect("materialize R7 fixture worktree file");

            let mut hash = git_command(&global_config);
            hash.arg("-C")
                .arg(&worktree)
                .args(["hash-object", "-w", "--stdin"]);
            let blob_oid = stdout_line(successful_output(hash, Some(&bytes)));
            assert_eq!(
                blob_oid,
                file["git_blob_oid"].as_str().expect("reviewed Git blob"),
                "R7 fixture Git blob changed for {fixture_path}"
            );
            update_index(&worktree, &global_config, "100644", &blob_oid, destination);
        }

        for (source, destination) in [
            ("compiler-index-binding.json", "compiler-index-binding.json"),
            ("index.scip", "index.scip"),
        ] {
            let file = reviewed.get(source).expect("reviewed R7 sidecar entry");
            let bytes = read_reviewed_sidecar(&fixture, source);
            assert_reviewed_bytes(file, &bytes, source);
            fs::write(compiler_index.join(destination), bytes)
                .expect("materialize explicit R7 sidecar input");
        }

        let mut write_tree = git_command(&global_config);
        write_tree.arg("-C").arg(&worktree).arg("write-tree");
        let tree_oid = stdout_line(successful_output(write_tree, None));
        assert_eq!(tree_oid, FIXTURE_TREE_OID, "R7 fixture tree changed");

        let mut make_commit = git_command(&global_config);
        make_commit
            .arg("-C")
            .arg(&worktree)
            .args(["commit-tree", &tree_oid, "-F", "-"])
            .env("GIT_AUTHOR_NAME", "CodeNoesis Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_AUTHOR_DATE", "@0 +0000")
            .env("GIT_COMMITTER_NAME", "CodeNoesis Fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_COMMITTER_DATE", "@0 +0000");
        let commit_oid = stdout_line(successful_output(make_commit, Some(b"compiler-index-v1\n")));
        assert_eq!(commit_oid, FIXTURE_COMMIT_OID, "R7 fixture commit changed");

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
                "--compiler-index-profile",
                "scip-rust-v0.9.0-import-v1",
                "--compiler-index-binding",
                BINDING_RELATIVE_PATH,
            ])
            .arg("--store")
            .arg(&self.store)
            .args(["--format", "json"])
            .output()
            .expect("launch R7 compiler-index subject")
    }

    pub fn docs(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .args(["docs", "--store"])
            .arg(&self.store)
            .args(["--repository-id", REPOSITORY_ID, "--output"])
            .arg(&self.documents)
            .args(["--format", "json"])
            .output()
            .expect("launch R7 documentation subject")
    }

    pub fn query(&self, requested_id: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .args(["query", "--store"])
            .arg(&self.store)
            .args(["--repository-id", REPOSITORY_ID, "--documents"])
            .arg(&self.documents)
            .args(["--id", requested_id, "--format", "json"])
            .output()
            .expect("launch R7 exact-ID query subject")
    }

    pub fn binding_path(&self) -> PathBuf {
        self.root.join(BINDING_RELATIVE_PATH)
    }

    pub fn artifact_path(&self) -> PathBuf {
        self.root.join("compiler-index/index.scip")
    }

    pub fn artifact_bytes(&self) -> Vec<u8> {
        fs::read(self.artifact_path()).expect("read materialized R7 artifact")
    }

    pub fn binding(&self) -> Value {
        serde_json::from_slice(
            &fs::read(self.binding_path()).expect("read materialized R7 binding"),
        )
        .expect("parse materialized R7 binding")
    }

    pub fn write_binding(&self, binding: &Value) {
        fs::write(
            self.binding_path(),
            serde_json::to_vec(binding).expect("serialize materialized R7 binding"),
        )
        .expect("write materialized R7 binding");
    }

    pub fn mutate_binding(&self, mutate: impl FnOnce(&mut Value)) {
        let mut binding = self.binding();
        mutate(&mut binding);
        self.write_binding(&binding);
    }

    pub fn write_bound_artifact(&self, bytes: &[u8]) {
        fs::write(self.artifact_path(), bytes).expect("write materialized R7 artifact");
        self.mutate_binding(|binding| {
            binding["artifact"]["byte_length"] = Value::Number(
                u64::try_from(bytes.len())
                    .expect("materialized R7 artifact length")
                    .into(),
            );
            binding["artifact"]["sha256"] = Value::String(lower_hex(&Sha256::digest(bytes)));
        });
    }

    pub fn refresh_source_manifest(&self) {
        self.mutate_binding(|binding| {
            let indexed = binding["documents"]["indexed"]
                .as_array()
                .expect("materialized indexed documents")
                .clone();
            let bytes = serde_json::to_vec(&Value::Array(indexed))
                .expect("serialize materialized source manifest");
            binding["repository"]["source_manifest_sha256"] =
                Value::String(lower_hex(&Sha256::digest(bytes)));
        });
    }

    pub fn bind_arguments(&self, arguments: &[&str]) {
        let value = Value::Array(
            arguments
                .iter()
                .map(|argument| Value::String((*argument).to_owned()))
                .collect(),
        );
        let bytes = serde_json::to_vec(&value).expect("serialize materialized R7 arguments");
        self.mutate_binding(|binding| {
            binding["producer"]["arguments_sha256"] =
                Value::String(lower_hex(&Sha256::digest(bytes)));
        });
    }

    pub fn build_sentinel(&self) -> PathBuf {
        self.worktree.join("BUILD_SENTINEL_EXECUTED")
    }

    pub fn indexer_sentinel(&self) -> PathBuf {
        self.root.join("INDEXER_SENTINEL_EXECUTED")
    }
}

impl Drop for MaterializedCompilerIndexRepository {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/s4/compiler-index-v1")
}

pub fn expected_overlay() -> Value {
    serde_json::from_slice(
        &fs::read(fixture_root().join("expected-compiler-overlay.json"))
            .expect("read expected R7 compiler overlay"),
    )
    .expect("parse expected R7 compiler overlay")
}

pub fn invalid_case_expectations() -> BTreeMap<String, String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/specifications/s4/r7/invalid-cases-v1.json");
    let value: Value = serde_json::from_slice(&fs::read(path).expect("read R7 invalid matrix"))
        .expect("parse R7 invalid matrix");
    value["cases"]
        .as_array()
        .expect("R7 invalid cases")
        .iter()
        .map(|case| {
            (
                case["id"].as_str().expect("R7 invalid case ID").to_owned(),
                case["expected_error"]
                    .as_str()
                    .expect("R7 invalid expected error")
                    .to_owned(),
            )
        })
        .collect()
}

fn assert_reviewed_bytes(file: &Value, bytes: &[u8], fixture_path: &str) {
    assert_eq!(
        u64::try_from(bytes.len()).expect("fixture byte length"),
        file["byte_length"].as_u64().expect("reviewed byte length"),
        "R7 fixture byte length changed for {fixture_path}"
    );
    assert_eq!(
        lower_hex(&Sha256::digest(bytes)),
        file["sha256"].as_str().expect("reviewed SHA-256"),
        "R7 fixture SHA-256 changed for {fixture_path}"
    );
}

fn read_reviewed_sidecar(fixture: &Path, source: &str) -> Vec<u8> {
    if source == "compiler-index-binding.json" {
        read_repository_text(fixture.join(source))
    } else {
        fs::read(fixture.join(source)).expect("read R7 binary sidecar byte")
    }
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
        write!(&mut encoded, "{byte:02x}").expect("write digest");
    }
    encoded
}
