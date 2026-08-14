use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

#[cfg(all(target_arch = "x86_64", target_os = "linux", target_env = "gnu"))]
const EXPECTED_MANIFEST: Option<&[u8]> = Some(include_bytes!(
    "../../tests/specifications/g1/local-cli-distribution-v1/expected-manifest-x86_64-unknown-linux-gnu.json"
));
#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
const EXPECTED_MANIFEST: Option<&[u8]> = Some(include_bytes!(
    "../../tests/specifications/g1/local-cli-distribution-v1/expected-manifest-aarch64-apple-darwin.json"
));
#[cfg(all(target_arch = "x86_64", target_os = "windows", target_env = "msvc"))]
const EXPECTED_MANIFEST: Option<&[u8]> = Some(include_bytes!(
    "../../tests/specifications/g1/local-cli-distribution-v1/expected-manifest-x86_64-pc-windows-msvc.json"
));
#[cfg(not(any(
    all(target_arch = "x86_64", target_os = "linux", target_env = "gnu"),
    all(target_arch = "aarch64", target_os = "macos"),
    all(target_arch = "x86_64", target_os = "windows", target_env = "msvc")
)))]
const EXPECTED_MANIFEST: Option<&[u8]> = None;

#[test]
fn e2e_fr_rel_002_packages_local_cli() {
    let test_directory = TestDirectory::new();
    let output_root = test_directory.path().join("output");
    fs::create_dir(&output_root).expect("create empty output root");
    let binary = specification_path("fixtures/noesis-v1.bin");
    let output = run_xtask(&binary, &output_root);
    fail_if_expected_checkpoint_red(&output, &output_root);

    let Some(expected) = EXPECTED_MANIFEST else {
        assert_error(&output, "distribution.invalid_binary");
        return;
    };
    let expected = canonical_lf_golden(expected);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, expected);

    let directories = fs::read_dir(&output_root)
        .expect("read output root")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect output entries");
    assert_eq!(directories.len(), 1);
    let bundle = directories[0].path();
    assert_eq!(fs::read(bundle.join("manifest.json")).unwrap(), expected);
    assert_eq!(
        fs::read(bundle.join(binary_leaf())).unwrap(),
        fs::read(binary).unwrap()
    );
    assert_eq!(
        fs::read(bundle.join("etc/codenoesis/config.json")).unwrap(),
        fs::read(specification_path("default-config.json")).unwrap()
    );
    assert_eq!(
        fs::read(bundle.join(
            "share/codenoesis/schemas/local-cli-config-v1.schema.json"
        ))
        .unwrap(),
        fs::read(specification_path("local-cli-config-v1.schema.json")).unwrap()
    );
    assert_eq!(
        fs::read(bundle.join("share/doc/codenoesis/INSTALL.md")).unwrap(),
        fs::read(specification_path("install-v1.md")).unwrap()
    );
    assert_eq!(
        fs::read(bundle.join("share/doc/codenoesis/LICENSE")).unwrap(),
        fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../LICENSE")).unwrap()
    );
}

fn specification_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/specifications/g1/local-cli-distribution-v1")
        .join(relative)
}

fn binary_leaf() -> &'static str {
    if cfg!(windows) {
        "bin/noesis.exe"
    } else {
        "bin/noesis"
    }
}

fn run_xtask(binary: &Path, output_root: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("package-local-cli")
        .arg("--binary")
        .arg(binary)
        .arg("--output")
        .arg(output_root)
        .env("HOME", "/private/absolute/root")
        .env("HTTPS_PROXY", "https://private.invalid")
        .output()
        .expect("run xtask package-local-cli")
}

fn fail_if_expected_checkpoint_red(output: &Output, output_root: &Path) {
    const BOOTSTRAP: &[u8] = b"CodeNoesis workspace bootstrap: no product slice is implemented. See docs/software/software-requirements-specification.md.\n";
    if output.status.code() == Some(0) && output.stdout == BOOTSTRAP {
        assert!(output.stderr.is_empty());
        assert_eq!(fs::read_dir(output_root).unwrap().count(), 0);
        panic!("expected Red: the exact base xtask emits only its bootstrap sentence");
    }
}

fn assert_error(output: &Output, code: &str) {
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error = String::from_utf8(output.stderr.clone()).expect("error is UTF-8");
    assert!(error.contains("\"schema_version\":\"codenoesis.error/v26\""));
    assert!(error.contains(&format!("\"code\":\"{code}\"")));
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

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new() -> Self {
        let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "codenoesis-g1a-{}-{sequence}",
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
