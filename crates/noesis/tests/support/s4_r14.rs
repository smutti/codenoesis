use std::ffi::OsString;
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
        self.scan_command(true, true)
            .output()
            .expect("launch R14 expression-binding scan")
    }

    pub fn scan_with_profiles(
        &self,
        callable_profile: bool,
        expression_profile: bool,
        extra_options: &[&str],
    ) -> Output {
        let mut command = self.scan_command(callable_profile, expression_profile);
        command.args(extra_options);
        command.output().expect("launch R14 selector matrix scan")
    }

    pub fn permuted_scan_command(&self, seed: u64) -> Command {
        let store = self.inner.root.join(format!("permuted-store-{seed}"));
        let mut options = vec![
            (
                OsString::from("--repository"),
                self.inner.worktree.clone().into_os_string(),
            ),
            (
                OsString::from("--repository-id"),
                OsString::from(REPOSITORY_ID),
            ),
            (
                OsString::from("--revision"),
                OsString::from(&self.inner.commit_oid),
            ),
            (
                OsString::from("--profile"),
                OsString::from("standard-local-s4"),
            ),
            (
                OsString::from("--workspace-profile"),
                OsString::from("cargo-root-package-v1"),
            ),
            (
                OsString::from("--manifest-profile"),
                OsString::from("cargo-manifest-facts-v1"),
            ),
            (
                OsString::from("--rust-semantic-profile"),
                OsString::from("rust-semantic-depth-v1"),
            ),
            (
                OsString::from("--rust-framework-profile"),
                OsString::from("rust-framework-declarations-v1"),
            ),
            (
                OsString::from("--rust-callable-profile"),
                OsString::from(K1_PROFILE),
            ),
            (
                OsString::from("--rust-expression-profile"),
                OsString::from(R14_PROFILE),
            ),
            (OsString::from("--store"), store.into_os_string()),
            (OsString::from("--format"), OsString::from("json")),
        ];
        let mut state = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
        for position in (1..options.len()).rev() {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let divisor = u64::try_from(position + 1).expect("R14 option count fits u64");
            let selected =
                usize::try_from(state % divisor).expect("R14 selected option index fits usize");
            options.swap(position, selected);
        }
        let mut command = Command::new(env!("CARGO_BIN_EXE_noesis"));
        command.current_dir(&self.inner.root).arg("scan");
        for (flag, value) in options {
            command.arg(flag).arg(value);
        }
        command
    }

    fn scan_command(&self, callable_profile: bool, expression_profile: bool) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_noesis"));
        command
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
            ])
            .arg("--store")
            .arg(&self.inner.store)
            .args(["--format", "json"]);
        if callable_profile {
            command.args(["--rust-callable-profile", K1_PROFILE]);
        }
        if expression_profile {
            command.args(["--rust-expression-profile", R14_PROFILE]);
        }
        command
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
