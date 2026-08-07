use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde_json::Value;

use super::unique_temp_root;

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

pub fn canonical_temp_root() -> PathBuf {
    let root = unique_temp_root();
    fs::create_dir_all(&root).expect("create R8 temporary root");
    fs::canonicalize(root).expect("canonicalize R8 temporary root")
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
