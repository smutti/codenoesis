use std::path::Path;

use serde_json::Value;
use sha2::{Digest as _, Sha256};

pub fn assert_matching_viewer_contract(viewer_path: &Path, manifest: &Value, portable_version: u8) {
    let viewer = std::fs::read(viewer_path).expect("read versioned explorer viewer");
    let text = std::str::from_utf8(&viewer).expect("versioned explorer viewer UTF-8");
    let expected_schema = format!("codenoesis.portable-graph/v{portable_version}");
    let expected_meta =
        format!("<meta name=\"codenoesis-portable-schema\" content=\"{expected_schema}\">");
    assert!(
        text.contains(&expected_meta),
        "viewer is not bound to {expected_schema}"
    );
    assert!(
        text.contains("id=\"graph-view\""),
        "viewer lacks bounded graph visualization"
    );
    assert!(
        text.contains("id=\"uncertainty-button\""),
        "viewer lacks uncertainty inspection"
    );
    assert!(
        text.contains("id=\"derivation-view\""),
        "viewer lacks derivation inspection"
    );
    assert_eq!(
        manifest["entrypoint"]["byte_length"].as_u64(),
        Some(u64::try_from(viewer.len()).expect("viewer length fits u64"))
    );
    let viewer_sha256 = lower_hex(&Sha256::digest(&viewer));
    assert_eq!(
        manifest["entrypoint"]["sha256"].as_str(),
        Some(viewer_sha256.as_str())
    );
    for forbidden in [
        "http://",
        "https://",
        "fetch(",
        "XMLHttpRequest",
        "WebSocket",
        "eval(",
        "new Function(",
        ".innerHTML",
        "unsafe-inline",
        "unsafe-eval",
    ] {
        assert!(!text.contains(forbidden), "viewer contains {forbidden}");
    }
}

fn lower_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
