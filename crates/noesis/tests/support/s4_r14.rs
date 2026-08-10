use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

use super::s4_k1::{MaterializedCallableRepository, REPOSITORY_ID};

pub const K1_PROFILE: &str = "rust-callable-semantics-v1";
pub const R14_PROFILE: &str = "rust-expression-bindings-v1";

pub struct MaterializedExpressionBindingRepository {
    pub inner: MaterializedCallableRepository,
}

impl MaterializedExpressionBindingRepository {
    pub fn fixture() -> Self {
        Self {
            inner: MaterializedCallableRepository::fixture(),
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
                "rust-semantic-depth-v1",
                "--rust-framework-profile",
                "rust-framework-declarations-v1",
                "--rust-callable-profile",
                K1_PROFILE,
                "--rust-expression-profile",
                R14_PROFILE,
            ])
            .arg("--store")
            .arg(&self.inner.store)
            .args(["--format", "json"])
            .output()
            .expect("launch R14 expression-binding scan")
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
            .args(["--repository-id", REPOSITORY_ID, "--documents"])
            .arg(&self.inner.documents)
            .arg("--output")
            .arg(&self.inner.portable)
            .args(["--portable-profile", R14_PROFILE, "--format", "json"])
            .output()
            .expect("launch R14 portable export")
    }

    pub fn explore(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .args(["explore", "--input"])
            .arg(self.inner.portable.join("portable-graph.json"))
            .arg("--output")
            .arg(&self.inner.explorer)
            .args(["--explorer-profile", R14_PROFILE, "--format", "json"])
            .output()
            .expect("launch R14 local explorer")
    }

    pub fn store(&self) -> &Path {
        &self.inner.store
    }

    pub fn build_sentinel(&self) -> std::path::PathBuf {
        self.inner.build_sentinel()
    }
}

pub fn expected_expression_bindings() -> Value {
    serde_json::from_slice(
        &fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../tests/fixtures/s4/rust-expression-bindings-v1/expected-expression-bindings.json",
        ))
        .expect("read R14 expected expression bindings"),
    )
    .expect("parse R14 expected expression bindings")
}
