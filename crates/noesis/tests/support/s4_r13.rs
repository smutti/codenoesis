use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
#[cfg(windows)]
use std::{
    path::{Component, Prefix},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::Value;

use super::s4_r7::{BINDING_RELATIVE_PATH, MaterializedCompilerIndexRepository};
#[cfg(not(windows))]
use super::unique_temp_root;

#[cfg(windows)]
static R13_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

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
        let root = r13_temp_root();
        let inner = MaterializedCompilerIndexRepository::fixture_in(root);
        let portable = inner.root.join("portable");
        let explorer = inner.root.join("explorer");
        Self {
            inner,
            portable,
            explorer,
        }
    }

    pub fn scan(&self) -> Output {
        self.scan_command(true, true)
            .output()
            .expect("launch R13 callable/SCIP scan")
    }

    pub fn scan_with_compiler_selector(
        &self,
        compiler_profile: bool,
        compiler_binding: bool,
        extra_options: &[&str],
    ) -> Output {
        let mut command = self.scan_command(compiler_profile, compiler_binding);
        command.args(extra_options);
        command.output().expect("launch R13 selector matrix scan")
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
                OsString::from(R5_PROFILE),
            ),
            (
                OsString::from("--rust-framework-profile"),
                OsString::from(R6_PROFILE),
            ),
            (
                OsString::from("--compiler-index-profile"),
                OsString::from(R7_PROFILE),
            ),
            (
                OsString::from("--compiler-index-binding"),
                OsString::from(BINDING_RELATIVE_PATH),
            ),
            (
                OsString::from("--rust-callable-profile"),
                OsString::from(K1_PROFILE),
            ),
            (OsString::from("--store"), store.into_os_string()),
            (OsString::from("--format"), OsString::from("json")),
        ];
        let mut state = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
        for position in (1..options.len()).rev() {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let divisor = u64::try_from(position + 1).expect("R13 option count fits u64");
            let selected =
                usize::try_from(state % divisor).expect("R13 selected option index fits usize");
            options.swap(position, selected);
        }
        let mut command = Command::new(env!("CARGO_BIN_EXE_noesis"));
        command.current_dir(&self.inner.root).arg("scan");
        for (flag, value) in options {
            command.arg(flag).arg(value);
        }
        command
    }

    fn scan_command(&self, compiler_profile: bool, compiler_binding: bool) -> Command {
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
                R5_PROFILE,
                "--rust-framework-profile",
                R6_PROFILE,
                "--rust-callable-profile",
                K1_PROFILE,
            ])
            .arg("--store")
            .arg(&self.inner.store)
            .args(["--format", "json"]);
        if compiler_profile {
            command.args(["--compiler-index-profile", R7_PROFILE]);
        }
        if compiler_binding {
            command.args(["--compiler-index-binding", BINDING_RELATIVE_PATH]);
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

#[cfg(not(windows))]
fn r13_temp_root() -> PathBuf {
    fs::canonicalize(unique_temp_root()).expect("canonicalize R13 temporary root")
}

#[cfg(windows)]
fn r13_temp_root() -> PathBuf {
    let workspace = std::env::current_dir().expect("resolve R13 E2E workspace");
    let mut candidates = vec![("workspace", workspace.join("target"))];
    if let Some(volume_root) = workspace.ancestors().last()
        && candidates
            .iter()
            .all(|(_, candidate)| candidate != volume_root)
    {
        candidates.push(("windows-volume", volume_root.to_path_buf()));
    }
    let sequence = R13_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("R13 E2E clock must follow the Unix epoch")
        .as_nanos();
    let candidate_count = candidates.len();
    for (authority, candidate) in candidates {
        if !candidate.is_absolute()
            || windows_verbatim_path(&candidate)
            || candidate
                .components()
                .any(|component| matches!(component, Component::ParentDir))
        {
            continue;
        }
        let root = candidate.join(format!(
            "codenoesis-r13-e2e-{authority}-{}-{timestamp}-{sequence}",
            std::process::id()
        ));
        if fs::create_dir(&root).is_err() {
            continue;
        }
        let probe_authority = root.join("authority-probe");
        if fs::create_dir(&probe_authority).is_err() {
            remove_rejected_windows_root(&root);
            continue;
        }
        let probe_output = root.join("output-probe");
        let validated = noesis::portable_explorer::validate_r13_export_output_root(
            &probe_authority,
            &probe_output,
        )
        .is_ok();
        let probe_removed = fs::remove_dir(&probe_authority).is_ok();
        if validated && probe_removed {
            return root;
        }
        remove_rejected_windows_root(&root);
    }
    panic!("no validated non-verbatim R13 E2E authority across {candidate_count} candidates");
}

#[cfg(windows)]
fn windows_verbatim_path(path: &Path) -> bool {
    matches!(
        path.components().next(),
        Some(Component::Prefix(prefix))
            if matches!(
                prefix.kind(),
                Prefix::Verbatim(_)
                    | Prefix::VerbatimUNC(..)
                    | Prefix::VerbatimDisk(_)
                    | Prefix::DeviceNS(_)
            )
    )
}

#[cfg(windows)]
fn remove_rejected_windows_root(root: &Path) {
    fs::remove_dir_all(root).expect("remove rejected R13 E2E authority candidate");
}

pub fn expected_composition() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../tests/fixtures/s4/rust-callable-scip-composition-v1/expected-composition.json",
    );
    serde_json::from_slice(&fs::read(path).expect("read expected R13 composition"))
        .expect("parse expected R13 composition")
}
