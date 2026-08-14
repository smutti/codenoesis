use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

#[cfg(all(target_arch = "x86_64", target_os = "linux", target_env = "gnu"))]
const EXPECTED_PLAN: Option<&[u8]> = Some(include_bytes!(
    "../../tests/specifications/g2/local-upgrade-safety-v1/expected-upgrade-plan-x86_64-unknown-linux-gnu.json"
));
#[cfg(all(target_arch = "x86_64", target_os = "linux", target_env = "gnu"))]
const EXPECTED_ROLLBACK: Option<&[u8]> = Some(include_bytes!(
    "../../tests/specifications/g2/local-upgrade-safety-v1/expected-rollback-report-x86_64-unknown-linux-gnu.json"
));
#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
const EXPECTED_PLAN: Option<&[u8]> = Some(include_bytes!(
    "../../tests/specifications/g2/local-upgrade-safety-v1/expected-upgrade-plan-aarch64-apple-darwin.json"
));
#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
const EXPECTED_ROLLBACK: Option<&[u8]> = Some(include_bytes!(
    "../../tests/specifications/g2/local-upgrade-safety-v1/expected-rollback-report-aarch64-apple-darwin.json"
));
#[cfg(all(target_arch = "x86_64", target_os = "windows", target_env = "msvc"))]
const EXPECTED_PLAN: Option<&[u8]> = Some(include_bytes!(
    "../../tests/specifications/g2/local-upgrade-safety-v1/expected-upgrade-plan-x86_64-pc-windows-msvc.json"
));
#[cfg(all(target_arch = "x86_64", target_os = "windows", target_env = "msvc"))]
const EXPECTED_ROLLBACK: Option<&[u8]> = Some(include_bytes!(
    "../../tests/specifications/g2/local-upgrade-safety-v1/expected-rollback-report-x86_64-pc-windows-msvc.json"
));
#[cfg(not(any(
    all(target_arch = "x86_64", target_os = "linux", target_env = "gnu"),
    all(target_arch = "aarch64", target_os = "macos"),
    all(target_arch = "x86_64", target_os = "windows", target_env = "msvc")
)))]
const EXPECTED_PLAN: Option<&[u8]> = None;
#[cfg(not(any(
    all(target_arch = "x86_64", target_os = "linux", target_env = "gnu"),
    all(target_arch = "aarch64", target_os = "macos"),
    all(target_arch = "x86_64", target_os = "windows", target_env = "msvc")
)))]
const EXPECTED_ROLLBACK: Option<&[u8]> = None;

#[test]
fn e2e_fr_cmp_001_preflights_local_upgrade_and_rollback() {
    let (Some(expected_plan), Some(expected_rollback)) = (EXPECTED_PLAN, EXPECTED_ROLLBACK) else {
        return;
    };
    let expected_plan = canonical_lf_golden(expected_plan);
    let expected_rollback = canonical_lf_golden(expected_rollback);
    let test_directory = TestDirectory::new();
    let current = package_fixture(test_directory.path(), "current", "noesis-v1.bin");
    let candidate = package_fixture(test_directory.path(), "candidate", "noesis-v2.bin");
    let current_snapshot = snapshot_tree(&current);
    let candidate_snapshot = snapshot_tree(&candidate);

    for construction in 0..50 {
        let output = run_upgrade(&current, &candidate, construction % 2 == 1);
        fail_if_expected_checkpoint_red(&output);
        assert_success(&output, &expected_plan);
    }

    let plan_path = test_directory.path().join("local-upgrade-plan-v1.json");
    fs::write(&plan_path, &expected_plan).expect("write exact upgrade plan");
    for _schedule in 0..10 {
        let output = run_rollback(&plan_path, &candidate, &current);
        assert_success(&output, &expected_rollback);
    }

    assert_eq!(snapshot_tree(&current), current_snapshot);
    assert_eq!(snapshot_tree(&candidate), candidate_snapshot);
}

fn package_fixture(root: &Path, label: &str, fixture: &str) -> PathBuf {
    let output_root = root.join(label);
    fs::create_dir(&output_root).expect("create empty package output root");
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("package-local-cli")
        .arg("--binary")
        .arg(g1_specification_path(&format!("fixtures/{fixture}")))
        .arg("--output")
        .arg(&output_root)
        .env("HOME", "/private/absolute/root")
        .env("HTTPS_PROXY", "https://private.invalid")
        .output()
        .expect("package G1a fixture");
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());

    let bundles = fs::read_dir(&output_root)
        .expect("read package output root")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect package output entries");
    assert_eq!(bundles.len(), 1);
    bundles[0].path()
}

fn run_upgrade(current: &Path, candidate: &Path, reversed_flags: bool) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_xtask"));
    command.arg("preflight-local-upgrade");
    if reversed_flags {
        command
            .arg("--candidate")
            .arg(candidate)
            .arg("--current")
            .arg(current);
    } else {
        command
            .arg("--current")
            .arg(current)
            .arg("--candidate")
            .arg(candidate);
    }
    command
        .env("HOME", "/private/absolute/root")
        .env("HTTPS_PROXY", "https://private.invalid")
        .output()
        .expect("run local upgrade preflight")
}

fn run_rollback(plan: &Path, current: &Path, target: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("preflight-local-rollback")
        .arg("--plan")
        .arg(plan)
        .arg("--current")
        .arg(current)
        .arg("--target")
        .arg(target)
        .env("HOME", "/private/absolute/root")
        .env("HTTPS_PROXY", "https://private.invalid")
        .output()
        .expect("run local rollback preflight")
}

fn fail_if_expected_checkpoint_red(output: &Output) {
    const EXPECTED: &[u8] = b"{\"code\":\"distribution.invalid_arguments\",\"context\":{},\"message\":\"invalid distribution command\",\"retryable\":false,\"schema_version\":\"codenoesis.error/v26\",\"stage\":\"input\"}\n";
    if output.status.code() == Some(2) && output.stdout.is_empty() && output.stderr == EXPECTED {
        panic!(
            "expected Red: exact base rejects preflight-local-upgrade as distribution.invalid_arguments"
        );
    }
}

fn assert_success(output: &Output, expected: &[u8]) {
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, expected);
}

fn g1_specification_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/specifications/g1/local-cli-distribution-v1")
        .join(relative)
}

fn canonical_lf_golden(checked_out: &[u8]) -> Vec<u8> {
    let body = checked_out
        .strip_suffix(b"\r\n")
        .or_else(|| checked_out.strip_suffix(b"\n"))
        .expect("golden has one final LF or CRLF");
    assert!(!body.contains(&b'\r'));
    assert!(!body.contains(&b'\n'));

    let mut canonical = Vec::with_capacity(body.len() + 1);
    canonical.extend_from_slice(body);
    canonical.push(b'\n');
    canonical
}

fn snapshot_tree(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn collect(root: &Path, directory: &Path, snapshot: &mut Vec<(PathBuf, Vec<u8>)>) {
        let mut entries = fs::read_dir(directory)
            .expect("read bundle directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect bundle entries");
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).expect("read bundle metadata");
            assert!(!metadata.file_type().is_symlink());
            if metadata.is_dir() {
                collect(root, &path, snapshot);
            } else {
                snapshot.push((
                    path.strip_prefix(root)
                        .expect("relative bundle path")
                        .to_path_buf(),
                    fs::read(path).expect("read bundle file"),
                ));
            }
        }
    }

    let mut snapshot = Vec::new();
    collect(root, root, &mut snapshot);
    snapshot
}

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new() -> Self {
        let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "codenoesis-g2-local-upgrade-{}-{sequence}",
            std::process::id()
        ));
        if path.exists() {
            fs::remove_dir_all(&path).expect("remove stale test directory");
        }
        fs::create_dir(&path).expect("create test directory");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        if self.path.exists() {
            fs::remove_dir_all(&self.path).expect("remove test directory");
        }
    }
}
