use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

use super::s4_r7::{BINDING_RELATIVE_PATH, MaterializedCompilerIndexRepository};

pub const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s4-compiler-index-v1";
pub const R5_PROFILE: &str = "rust-semantic-depth-v1";
pub const R6_PROFILE: &str = "rust-framework-declarations-v1";
pub const R7_PROFILE: &str = "scip-rust-v0.9.0-import-v1";
pub const K1_PROFILE: &str = "rust-callable-semantics-v1";

pub struct MaterializedCallableScipRepository {
    inner: MaterializedCompilerIndexRepository,
    portable: PathBuf,
    explorer: PathBuf,
}

impl MaterializedCallableScipRepository {
    pub fn fixture() -> Self {
        let inner = MaterializedCompilerIndexRepository::fixture();
        let portable = inner.root.join("portable");
        let explorer = inner.root.join("explorer");
        Self {
            inner,
            portable,
            explorer,
        }
    }

    pub fn scan(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .current_dir(&self.inner.root)
            .args(["scan", "--repository"])
            .arg(&self.inner.worktree)
            .args(["--repository-id", REPOSITORY_ID, "--revision"])
            .arg(&self.inner.commit_oid)
            .args([
                "--profile",
                "standard-local-s4",
                "--workspace-profile",
                "cargo-root-package-v1",
                "--manifest-profile",
                "cargo-manifest-facts-v1",
                "--rust-semantic-profile",
                R5_PROFILE,
                "--rust-framework-profile",
                R6_PROFILE,
                "--compiler-index-profile",
                R7_PROFILE,
                "--compiler-index-binding",
                BINDING_RELATIVE_PATH,
                "--rust-callable-profile",
                K1_PROFILE,
            ])
            .arg("--store")
            .arg(&self.inner.store)
            .args(["--format", "json"])
            .output()
            .expect("launch R13 callable/SCIP scan")
    }

    pub fn docs(&self) -> Output {
        self.inner.docs()
    }

    pub fn query(&self, requested_id: &str) -> Output {
        self.inner.query(requested_id)
    }

    pub fn export(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .args(["export", "--store"])
            .arg(&self.inner.store)
            .args(["--repository-id", REPOSITORY_ID, "--output"])
            .arg(&self.portable)
            .args(["--portable-profile", K1_PROFILE, "--format", "json"])
            .output()
            .expect("launch R13 portable export")
    }

    pub fn explore(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .args(["explore", "--input"])
            .arg(self.portable.join("portable-graph.json"))
            .arg("--output")
            .arg(&self.explorer)
            .args(["--explorer-profile", K1_PROFILE, "--format", "json"])
            .output()
            .expect("launch R13 local explorer")
    }

    pub fn store(&self) -> &Path {
        &self.inner.store
    }

    pub fn documents(&self) -> &Path {
        &self.inner.documents
    }

    pub fn portable(&self) -> &Path {
        &self.portable
    }

    pub fn explorer(&self) -> &Path {
        &self.explorer
    }

    pub fn build_sentinel(&self) -> PathBuf {
        self.inner.build_sentinel()
    }

    pub fn indexer_sentinel(&self) -> PathBuf {
        self.inner.indexer_sentinel()
    }
}

pub fn expected_composition() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../tests/fixtures/s4/rust-callable-scip-composition-v1/expected-composition.json",
    );
    serde_json::from_slice(&fs::read(path).expect("read expected R13 composition"))
        .expect("parse expected R13 composition")
}
