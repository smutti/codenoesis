//! Kotlin projections use the existing marker-owned publication protocol.
use super::{OutputKind, OwnedFile, PortableExplorerError, PreparedOutputRoot};
use codenoesis_contracts::s8_kotlin::{canonical, parse_portable};
use serde_json::json;
use std::path::{Path, PathBuf};

/// Validates every input component before a bounded stable read.
/// # Errors
/// Rejects symlinks, relative escapes and unsafe components.
pub fn input_path(input: &Path) -> Result<PathBuf, PortableExplorerError> {
    let input = super::absolute_without_parent_components(input)
        .map_err(|_| PortableExplorerError::Internal)?;
    super::verify_existing_components(&input)?;
    Ok(input)
}
/// Prepares a marker-owned destination with complete overlap and path checks.
/// # Errors
/// Rejects unsafe, overlapping or unowned destinations.
pub fn prepare(
    authority: &Path,
    output: &Path,
    explorer: bool,
) -> Result<PreparedOutputRoot, PortableExplorerError> {
    super::ensure_output_root(
        authority,
        output,
        if explorer {
            OutputKind::KotlinExplorer
        } else {
            OutputKind::KotlinPortable
        },
    )
}
/// Publishes a canonical package already validated against its source snapshot.
/// # Errors
/// Rejects malformed packages and guarded publication failures.
pub fn publish(
    prepared: &PreparedOutputRoot,
    bytes: &[u8],
    explorer: bool,
) -> Result<Vec<u8>, PortableExplorerError> {
    let portable = parse_portable(bytes).map_err(|_| PortableExplorerError::Internal)?;
    if !explorer {
        super::publish_files(
            prepared,
            OutputKind::KotlinPortable,
            &[OwnedFile::new("portable-graph.json", bytes.to_vec())],
        )?;
        return Ok(bytes.to_vec());
    }
    let payload = serde_json::to_string(&portable["payload"])
        .map_err(|_| PortableExplorerError::Internal)?
        .replace('&', "\\u0026")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e");
    let html = viewer_html(
        include_bytes!("../../assets/s8/kotlin/index.html"),
        &payload,
    )?;
    let manifest = json!({"schema_version":"codenoesis.kotlin-explorer/v1","snapshot_id":portable["payload"]["snapshot_id"],"files":[
        {"path":"index.html","sha256":super::sha256(&html),"byte_length":html.len()},
        {"path":"portable-graph.json","sha256":super::sha256(bytes),"byte_length":bytes.len()}],"network":"disabled","auto_open":false});
    let manifest = canonical(&manifest).map_err(|_| PortableExplorerError::Internal)?;
    super::publish_files(
        prepared,
        OutputKind::KotlinExplorer,
        &[
            OwnedFile::new("portable-graph.json", bytes.to_vec()),
            OwnedFile::new("index.html", html),
            OwnedFile::new("explorer-manifest.json", manifest.clone()),
        ],
    )?;
    Ok(manifest)
}

fn viewer_html(template: &[u8], payload: &str) -> Result<Vec<u8>, PortableExplorerError> {
    let template =
        super::normalize_checkout_text(template).ok_or(PortableExplorerError::Internal)?;
    let template = std::str::from_utf8(&template).map_err(|_| PortableExplorerError::Internal)?;
    Ok(template.replace("__KOTLIN_PAYLOAD__", payload).into_bytes())
}

#[cfg(test)]
mod tests {
    use super::viewer_html;

    #[test]
    fn pt_fr_ext_025_viewer_bytes_are_identical_for_lf_and_crlf_checkouts() {
        let checked_out = include_bytes!("../../assets/s8/kotlin/index.html");
        let lf = super::super::normalize_checkout_text(checked_out).unwrap();
        let crlf = std::str::from_utf8(&lf).unwrap().replace('\n', "\r\n");
        let payload = r#"{"entities":[]}"#;
        let expected = viewer_html(&lf, payload).unwrap();
        assert_eq!(viewer_html(crlf.as_bytes(), payload).unwrap(), expected);
        assert!(!expected.contains(&b'\r'));
        assert!(
            !std::str::from_utf8(&expected)
                .unwrap()
                .contains("__KOTLIN_PAYLOAD__")
        );
    }
}
