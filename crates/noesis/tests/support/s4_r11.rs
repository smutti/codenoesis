use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use sha2::{Digest as _, Sha256};

use super::{git_command, read_repository_text, stdout_line, successful_output, unique_temp_root};

pub const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s4-rust-callable-semantics-v1";
pub const NESTED_REPOSITORY_ID: &str =
    "urn:codenoesis:fixture:s4-rust-callable-semantics-v1-nested-model";
pub const FIXTURE_TREE_OID: &str = "289bc8a5abcc2f45fa7e2aa9d787a97305975b71";
pub const FIXTURE_COMMIT_OID: &str = "1c8921919b50f565db49acdbf344cc7e1e864dd1";
pub const NESTED_TREE_OID: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";
pub const NESTED_COMMIT_OID: &str = "6ecf94267842da776e35406a9ebcb85e058a3181";
pub const BOUNDARY_ID: &str = "urn:codenoesis:repository-boundary:sha256:7f8f79973410e908962009f651f418416b57a4921c6b27e539dbed2696c45fd1";
pub const BOUNDARY_EVIDENCE_ID: &str = "urn:codenoesis:boundary-evidence:sha256:b56a73f0140dcff3e8bd36d676201f018ff2cdb4c1b5fb2687d78a0e0927ede3";

pub struct MaterializedCallableBoundaryRepository {
    pub root: PathBuf,
    pub worktree: PathBuf,
    pub store: PathBuf,
    pub documents: PathBuf,
    pub portable: PathBuf,
    pub explorer: PathBuf,
}

impl MaterializedCallableBoundaryRepository {
    pub fn fixture() -> Self {
        let root = unique_temp_root();
        let worktree = root.join("repository");
        let store = root.join("store");
        let documents = root.join("documents");
        let portable = root.join("portable");
        let explorer = root.join("explorer");
        let template = root.join("template");
        let global_config = root.join("global.gitconfig");
        fs::create_dir_all(&template).expect("create empty R11 Git template");
        fs::write(&global_config, []).expect("create empty R11 Git configuration");

        let mut init = git_command(&global_config);
        init.args(["init", "--quiet", "--initial-branch=main"])
            .arg(format!("--template={}", template.display()))
            .arg(&worktree);
        successful_output(init, None);

        let source_fixture = source_fixture_root();
        let source_manifest: Value = serde_json::from_slice(
            &fs::read(source_fixture.join("manifest.json")).expect("read K1 source manifest"),
        )
        .expect("parse K1 source manifest");
        let reviewed = source_manifest["files"]
            .as_array()
            .expect("reviewed K1 source files")
            .iter()
            .map(|file| (file["path"].as_str().expect("reviewed K1 path"), file))
            .collect::<BTreeMap<_, _>>();
        for (fixture_path, file) in reviewed {
            let relative = fixture_path
                .strip_prefix("repository/")
                .expect("K1 repository-relative path");
            let bytes = read_repository_text(source_fixture.join(fixture_path));
            assert_reviewed_bytes(file, &bytes, fixture_path);
            let destination = worktree.join(relative);
            fs::create_dir_all(destination.parent().expect("R11 source parent"))
                .expect("create R11 source parent");
            fs::write(&destination, &bytes).expect("materialize R11 source");
            let blob_oid = hash_object(&worktree, &global_config, &bytes);
            assert_eq!(blob_oid, file["git_blob_oid"].as_str().unwrap());
            update_index(&worktree, &global_config, "100644", &blob_oid, relative);
        }

        let gitmodules = read_repository_text(fixture_root().join("revision-overlay/.gitmodules"));
        fs::write(worktree.join(".gitmodules"), &gitmodules).expect("materialize .gitmodules");
        let gitmodules_oid = hash_object(&worktree, &global_config, &gitmodules);
        assert_eq!(gitmodules_oid, "204d65a4ed6a58dd265349f8a1579a4522dc4f7d");
        update_index(
            &worktree,
            &global_config,
            "100644",
            &gitmodules_oid,
            ".gitmodules",
        );

        materialize_nested_objects(&worktree, &global_config);
        update_index(
            &worktree,
            &global_config,
            "160000",
            NESTED_COMMIT_OID,
            "external/nested-model",
        );
        let mut write_tree = git_command(&global_config);
        write_tree.arg("-C").arg(&worktree).arg("write-tree");
        let tree_oid = stdout_line(successful_output(write_tree, None));
        assert_eq!(tree_oid, FIXTURE_TREE_OID, "R11 fixture tree changed");

        let mut make_commit = git_command(&global_config);
        make_commit
            .arg("-C")
            .arg(&worktree)
            .args(["commit-tree", &tree_oid, "-F", "-"])
            .env("GIT_AUTHOR_NAME", "CodeNoesis")
            .env("GIT_AUTHOR_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_AUTHOR_DATE", "2026-08-09T18:00:00Z")
            .env("GIT_COMMITTER_NAME", "CodeNoesis")
            .env("GIT_COMMITTER_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_COMMITTER_DATE", "2026-08-09T18:00:00Z");
        let commit_oid = stdout_line(successful_output(
            make_commit,
            Some(b"R11 project-owned callable boundary composition fixture\n"),
        ));
        assert_eq!(commit_oid, FIXTURE_COMMIT_OID, "R11 fixture commit changed");
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
        }
    }

    pub fn scan_unbound(&self) -> Output {
        self.scan_command(&self.store, None)
            .output()
            .expect("launch unbound R11 scan")
    }

    pub fn scan_bound(&self) -> Output {
        let nested = self.materialize_nested_repository();
        assert_eq!(nested, self.root.join("generated/nested-model"));
        let manifest = self.root.join("boundary-input-matching.json");
        fs::copy(
            fixture_root().join("boundary-input-matching.json"),
            &manifest,
        )
        .expect("copy R11 boundary input");
        self.scan_command(&self.store, Some(&manifest))
            .output()
            .expect("launch bound R11 scan")
    }

    pub fn docs(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .args(["docs", "--store"])
            .arg(&self.store)
            .args(["--repository-id", REPOSITORY_ID, "--output"])
            .arg(&self.documents)
            .args(["--format", "json"])
            .output()
            .expect("launch R11 docs")
    }

    pub fn query(&self, requested_id: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .args(["query", "--store"])
            .arg(&self.store)
            .args(["--repository-id", REPOSITORY_ID, "--documents"])
            .arg(&self.documents)
            .args(["--id", requested_id, "--format", "json"])
            .output()
            .expect("launch R11 query")
    }

    pub fn export(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .args(["export", "--store"])
            .arg(&self.store)
            .args(["--repository-id", REPOSITORY_ID, "--output"])
            .arg(&self.portable)
            .args([
                "--portable-profile",
                "rust-callable-semantics-v1",
                "--format",
                "json",
            ])
            .output()
            .expect("launch R11 export")
    }

    pub fn explore(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .args(["explore", "--input"])
            .arg(self.portable.join("portable-graph.json"))
            .arg("--output")
            .arg(&self.explorer)
            .args([
                "--explorer-profile",
                "rust-callable-semantics-v1",
                "--format",
                "json",
            ])
            .output()
            .expect("launch R11 explore")
    }

    pub fn build_sentinel(&self) -> PathBuf {
        self.worktree.join("K1_BUILD_SENTINEL_EXECUTED")
    }

    fn scan_command(&self, store: &Path, manifest: Option<&Path>) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_noesis"));
        command
            .current_dir(&self.root)
            .args(["scan", "--repository"])
            .arg(&self.worktree)
            .args([
                "--repository-id",
                REPOSITORY_ID,
                "--revision",
                FIXTURE_COMMIT_OID,
            ])
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
                "--repository-boundary-profile",
                "local-gitlinks-v1",
            ])
            .arg("--store")
            .arg(store);
        if let Some(manifest) = manifest {
            command.arg("--repository-boundary-manifest").arg(manifest);
        }
        command.args(["--format", "json"]);
        command
    }

    fn materialize_nested_repository(&self) -> PathBuf {
        let nested = self.root.join("generated/nested-model");
        let template = self.root.join("nested-template");
        let global_config = self.root.join("global.gitconfig");
        fs::create_dir_all(&template).expect("create nested template");
        let mut init = git_command(&global_config);
        init.args(["init", "--quiet", "--initial-branch=main"])
            .arg(format!("--template={}", template.display()))
            .arg(&nested);
        successful_output(init, None);
        materialize_nested_objects(&nested, &global_config);
        let mut update_ref = git_command(&global_config);
        update_ref.arg("-C").arg(&nested).args([
            "update-ref",
            "refs/heads/main",
            NESTED_COMMIT_OID,
        ]);
        successful_output(update_ref, None);
        nested
    }
}

pub fn expected_unbound_boundaries() -> Value {
    read_json(fixture_root().join("expected-boundaries-unbound.json"))
}

pub fn expected_bound_boundaries() -> Value {
    read_json(fixture_root().join("expected-boundaries-bound.json"))
}

pub fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/s4/rust-callable-boundary-composition-v1")
}

fn source_fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/s4/rust-callable-semantics-v1")
}

fn read_json(path: PathBuf) -> Value {
    serde_json::from_slice(&fs::read(path).expect("read R11 JSON")).expect("parse R11 JSON")
}

fn materialize_nested_objects(repository: &Path, global_config: &Path) {
    let mut make_tree = git_command(global_config);
    make_tree.arg("-C").arg(repository).arg("mktree");
    let tree_oid = stdout_line(successful_output(make_tree, Some(b"")));
    assert_eq!(tree_oid, NESTED_TREE_OID);
    let mut make_commit = git_command(global_config);
    make_commit
        .arg("-C")
        .arg(repository)
        .args(["commit-tree", &tree_oid, "-F", "-"])
        .env("GIT_AUTHOR_NAME", "CodeNoesis Fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@codenoesis.invalid")
        .env("GIT_AUTHOR_DATE", "1785628800 +0000")
        .env("GIT_COMMITTER_NAME", "CodeNoesis Fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@codenoesis.invalid")
        .env("GIT_COMMITTER_DATE", "1785628800 +0000");
    let commit_oid = stdout_line(successful_output(
        make_commit,
        Some(b"CodeNoesis R2 empty nested fixture\n"),
    ));
    assert_eq!(commit_oid, NESTED_COMMIT_OID);
}

fn hash_object(repository: &Path, global_config: &Path, bytes: &[u8]) -> String {
    let mut hash = git_command(global_config);
    hash.arg("-C")
        .arg(repository)
        .args(["hash-object", "-w", "--stdin"]);
    stdout_line(successful_output(hash, Some(bytes)))
}

fn update_index(repository: &Path, global_config: &Path, mode: &str, oid: &str, path: &str) {
    let mut update = git_command(global_config);
    update
        .arg("-C")
        .arg(repository)
        .args(["update-index", "--add", "--cacheinfo"])
        .arg(format!("{mode},{oid},{path}"));
    successful_output(update, None);
}

fn assert_reviewed_bytes(file: &Value, bytes: &[u8], path: &str) {
    assert_eq!(
        u64::try_from(bytes.len()).expect("R11 fixture byte length"),
        file["byte_length"].as_u64().expect("reviewed K1 length"),
        "R11 source length changed for {path}"
    );
    assert_eq!(
        lower_hex(&Sha256::digest(bytes)),
        file["sha256"].as_str().expect("reviewed K1 SHA-256"),
        "R11 source digest changed for {path}"
    );
}

fn lower_hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}").expect("write R11 digest");
    }
    encoded
}
