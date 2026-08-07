use std::collections::BTreeMap;
use std::fs;
use std::ops::Deref;
#[cfg(windows)]
use std::path::{Component, Prefix};
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(windows)]
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;
use serde_json::Value;

use super::s4_r7::MaterializedCompilerIndexRepository;
#[cfg(not(windows))]
use super::unique_temp_root;

#[cfg(windows)]
static R8_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub const PORTABLE_FILE_SHA256: &str =
    "389f5b211dd2a0c33af36a639145fb9867653fe3d85b56f3899f8cac799e3523";
pub const PORTABLE_CANONICAL_SHA256: &str =
    "ba3ab0fe6c4de2c1c195c5a3e4902f07e4864358669f00c8ea3fcb10944412bc";
pub const VIEWER_SHA256: &str = "1caa2c0ca5675937eab674f61681883ba3c6a428feb6b1baa744a0cb7eecd044";

pub fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/s4/portable-explorer-v1")
}

pub fn portable_bytes() -> Vec<u8> {
    reviewed_text_bytes("portable-graph.json")
}

pub fn portable_value() -> Value {
    serde_json::from_slice(&portable_bytes()).expect("parse R8 portable fixture")
}

pub fn family_digest_oracle() -> Value {
    serde_json::from_slice(
        &fs::read(fixture_root().join("source-family-digests.json"))
            .expect("read R8 family digest oracle"),
    )
    .expect("parse R8 family digest oracle")
}

pub fn viewer_bytes() -> Vec<u8> {
    reviewed_text_bytes("index.html")
}

pub fn explorer_manifest_bytes() -> Vec<u8> {
    reviewed_text_bytes("explorer-manifest.json")
}

fn reviewed_text_bytes(name: &str) -> Vec<u8> {
    reviewed_checkout_text(&fixture_root().join(name))
}

pub fn reviewed_checkout_text(path: &Path) -> Vec<u8> {
    let bytes = fs::read(path).expect("read R8 reviewed text");
    normalize_checkout_text(&bytes).expect("normalize R8 reviewed text checkout")
}

fn normalize_checkout_text(bytes: &[u8]) -> Option<Vec<u8>> {
    if !bytes.contains(&b'\r') {
        return Some(bytes.to_vec());
    }
    let mut normalized = Vec::with_capacity(bytes.len());
    let mut index = 0;
    let mut saw_crlf = false;
    let mut saw_lf = false;
    while index < bytes.len() {
        match bytes[index] {
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => {
                normalized.push(b'\n');
                saw_crlf = true;
                index += 2;
            }
            b'\r' => return None,
            b'\n' => {
                normalized.push(b'\n');
                saw_lf = true;
                index += 1;
            }
            byte => {
                normalized.push(byte);
                index += 1;
            }
        }
    }
    if saw_crlf && saw_lf {
        None
    } else {
        Some(normalized)
    }
}

pub fn invalid_case_expectations() -> BTreeMap<String, String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/specifications/s4/r8/invalid-security-cases-v1.json");
    let value: Value = serde_json::from_slice(&fs::read(path).expect("read R8 invalid matrix"))
        .expect("parse R8 invalid matrix");
    value["cases"]
        .as_array()
        .expect("R8 invalid cases")
        .iter()
        .map(|case| {
            (
                case["id"].as_str().expect("R8 invalid case ID").to_owned(),
                case["expected_code"]
                    .as_str()
                    .expect("R8 invalid expected code")
                    .to_owned(),
            )
        })
        .collect()
}

pub struct R8TestRoot {
    path: PathBuf,
    remove_on_drop: bool,
}

impl R8TestRoot {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            remove_on_drop: true,
        }
    }

    fn into_path(mut self) -> PathBuf {
        self.remove_on_drop = false;
        std::mem::take(&mut self.path)
    }
}

impl Deref for R8TestRoot {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

impl Drop for R8TestRoot {
    fn drop(&mut self) {
        if self.remove_on_drop {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

pub fn canonical_temp_root() -> R8TestRoot {
    #[cfg(not(windows))]
    {
        let root = unique_temp_root();
        fs::create_dir_all(&root).expect("create R8 temporary root");
        R8TestRoot::new(fs::canonicalize(root).expect("canonicalize R8 temporary root"))
    }
    #[cfg(windows)]
    {
        validated_windows_temp_root()
    }
}

pub fn materialized_repository() -> MaterializedCompilerIndexRepository {
    MaterializedCompilerIndexRepository::fixture_in(canonical_temp_root().into_path())
}

pub fn existing_test_path(path: &Path) -> PathBuf {
    #[cfg(not(windows))]
    {
        fs::canonicalize(path).expect("canonicalize existing R8 test path")
    }
    #[cfg(windows)]
    {
        assert!(path.is_absolute(), "R8 Windows test path must be absolute");
        assert!(
            !windows_verbatim_path(path),
            "R8 Windows test path must not use verbatim spelling"
        );
        assert!(
            !path
                .components()
                .any(|component| matches!(component, Component::ParentDir)),
            "R8 Windows test path must not contain a parent component"
        );
        fs::symlink_metadata(path).expect("inspect existing R8 Windows test path");
        path.to_path_buf()
    }
}

#[cfg(windows)]
fn validated_windows_temp_root() -> R8TestRoot {
    let workspace = std::env::current_dir().expect("resolve R8 E2E workspace");
    let mut candidates = vec![("workspace", workspace.join("target"))];
    if let Some(volume_root) = workspace.ancestors().last() {
        if candidates
            .iter()
            .all(|(_, candidate)| candidate != volume_root)
        {
            candidates.push(("windows-volume", volume_root.to_path_buf()));
        }
    }
    let sequence = R8_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("R8 E2E clock must follow the Unix epoch")
        .as_nanos();
    let candidate_count = candidates.len();
    for (authority, candidate) in candidates {
        if windows_verbatim_path(&candidate) {
            continue;
        }
        let root = candidate.join(format!(
            "codenoesis-r8-e2e-{authority}-{}-{timestamp}-{sequence}",
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
        let validated =
            noesis::portable_explorer::validate_export_output_root(&probe_authority, &probe_output)
                .is_ok();
        let probe_removed = fs::remove_dir(&probe_authority).is_ok();
        if validated && probe_removed {
            return R8TestRoot::new(root);
        }
        remove_rejected_windows_root(&root);
    }
    panic!("no validated R8 E2E authority across {candidate_count} bounded candidates");
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
    fs::remove_dir_all(root).expect("remove rejected R8 E2E authority candidate");
}

pub fn write_portable_input(root: &Path) -> PathBuf {
    let input = root.join("portable-graph.json");
    fs::write(&input, portable_bytes()).expect("write R8 portable input");
    input
}

pub fn corrupt_visible_snapshot_semantic(store: &Path, repository_identity: &str) {
    let connection =
        Connection::open(store.join("metadata.sqlite3")).expect("open R8 fixture metadata");
    let artifact_id = connection
        .query_row(
            "SELECT sa.artifact_id
             FROM project_heads h
             JOIN snapshot_artifacts sa ON sa.snapshot_id = h.snapshot_id
             WHERE h.repository_identity = ?1
               AND sa.role = 'snapshot_semantic'",
            [repository_identity],
            |row| row.get::<_, String>(0),
        )
        .expect("load R8 visible semantic artifact");
    let digest = artifact_id
        .strip_prefix("urn:codenoesis:artifact:blake3:")
        .expect("R8 semantic artifact ID");
    let path = store
        .join("objects/blake3")
        .join(&digest[..2])
        .join(&digest[2..]);
    let mut bytes = fs::read(&path).expect("read R8 visible semantic artifact");
    let byte = bytes.first_mut().expect("non-empty R8 semantic artifact");
    *byte ^= 1;
    fs::write(path, bytes).expect("corrupt R8 visible semantic artifact");
}
