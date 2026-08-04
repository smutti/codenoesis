mod support;

use std::fs;
use std::path::Path;

use serde_json::Value;
use sha2::{Digest as _, Sha256};

use support::s4_r4::{MaterializedCargoManifestRepository, fixture_root};

const PRE_R4_STDERR: &[u8] = b"{\"code\":\"input.invalid_revision\",\"context\":{},\"message\":\"invalid revision\",\"retryable\":false,\"schema_version\":\"codenoesis.error/v4\",\"stage\":\"input\"}\n";

#[test]
fn e2e_fr_ext_009_cargo_manifest_facts() {
    let repository = MaterializedCargoManifestRepository::fixture();
    let output = repository.scan();

    if output.status.code() == Some(2) {
        assert!(output.stdout.is_empty(), "pre-R4 stdout changed");
        assert_eq!(output.stderr, PRE_R4_STDERR, "pre-R4 stderr changed");
        assert!(!repository.store.exists(), "pre-R4 store was created");
        assert!(
            !repository.documents.exists(),
            "pre-R4 documents root was created"
        );
        panic!(
            "expected RepositorySnapshotV7 success; observed approved unknown manifest selector Red"
        );
    }

    assert!(
        output.status.success(),
        "R4 scan failed: status={:?}, stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "successful R4 stderr changed");
    let snapshot: Value =
        serde_json::from_slice(&output.stdout).expect("parse RepositorySnapshotV7");
    assert_eq!(
        snapshot["schema_version"],
        "codenoesis.repository-snapshot/v7"
    );
    assert_eq!(
        snapshot["semantic"]["configuration"]["manifest_profile"],
        "cargo-manifest-facts-v1"
    );
}

#[test]
fn e2e_fr_qry_001_r4_exact_id_results() {
    let repository = MaterializedCargoManifestRepository::fixture();
    let scan = repository.scan();
    assert!(
        scan.status.success(),
        "R4 scan failed: status={:?}, stderr={}",
        scan.status.code(),
        String::from_utf8_lossy(&scan.stderr)
    );
    let docs = repository.docs();
    assert!(
        docs.status.success(),
        "R4 docs failed: status={:?}, stderr={}",
        docs.status.code(),
        String::from_utf8_lossy(&docs.stderr)
    );

    let expected: Value = serde_json::from_slice(
        &fs::read(fixture_root().join("expected-manifest-facts.json"))
            .expect("read reviewed R4 manifest facts"),
    )
    .expect("parse reviewed R4 manifest facts");
    let examples = expected["exact_query_examples"]
        .as_object()
        .expect("reviewed exact query examples");
    let before_store = tree_fingerprint(&repository.store);
    let before_documents = tree_fingerprint(&repository.documents);

    for kind in [
        "entity",
        "relationship",
        "claim",
        "evidence",
        "diagnostic",
        "coverage_gap",
        "document",
    ] {
        let requested_id = examples[kind]["id"]
            .as_str()
            .expect("reviewed exact query ID");
        let first = repository.query(requested_id);
        assert!(
            first.status.success(),
            "{kind} query failed: status={:?}, stderr={}",
            first.status.code(),
            String::from_utf8_lossy(&first.stderr)
        );
        assert!(first.stderr.is_empty(), "successful {kind} stderr changed");
        let replay = repository.query(requested_id);
        assert_eq!(replay.status.code(), Some(0), "{kind} replay failed");
        assert_eq!(replay.stdout, first.stdout, "{kind} replay bytes changed");
        assert!(first.stdout.ends_with(b"\n"), "{kind} result lacks LF");
        assert!(
            !first.stdout.ends_with(b"\n\n"),
            "{kind} result has extra LF"
        );

        let result: Value =
            serde_json::from_slice(&first.stdout).expect("parse exact query result");
        assert_eq!(
            result["schema_version"], "codenoesis.local-query-result/v2",
            "V7 must dispatch to LocalQueryResultV2"
        );
        assert_eq!(result["requested_id"], requested_id);
        assert_eq!(result["result_kind"], kind);
        assert_query_cardinality(&result, kind, requested_id);
    }

    let unknown = repository.query(
        "urn:codenoesis:entity:blake3:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
    );
    assert_eq!(unknown.status.code(), Some(14));
    assert!(unknown.stdout.is_empty());
    let error: Value =
        serde_json::from_slice(&unknown.stderr).expect("parse query not-found error");
    assert_eq!(error["schema_version"], "codenoesis.error/v5");
    assert_eq!(error["code"], "query.not_found");
    assert_eq!(error["stage"], "query");
    assert_eq!(error["retryable"], false);
    assert_eq!(error["context"], serde_json::json!({}));

    assert_eq!(tree_fingerprint(&repository.store), before_store);
    assert_eq!(tree_fingerprint(&repository.documents), before_documents);
}

fn assert_query_cardinality(result: &Value, kind: &str, requested_id: &str) {
    let claims = result["claims"].as_array().expect("query claims");
    let evidence = result["evidence"].as_array().expect("query evidence");
    let statements = result["document_statements"]
        .as_array()
        .expect("query document statements");
    match kind {
        "entity" => {
            assert_eq!(result["entity"]["id"], requested_id);
            assert!(result["relationship"].is_null());
            assert_eq!(claims.len(), 1);
            assert!(!evidence.is_empty());
        }
        "relationship" => {
            assert!(result["entity"].is_null());
            assert_eq!(result["relationship"]["id"], requested_id);
            assert_eq!(claims.len(), 1);
            assert!(!evidence.is_empty());
        }
        "claim" => {
            assert_eq!(claims.len(), 1);
            assert_eq!(claims[0]["id"], requested_id);
            assert!(result["entity"].is_object() ^ result["relationship"].is_object());
            assert!(!evidence.is_empty());
        }
        "evidence" => {
            assert_eq!(evidence.len(), 1);
            assert_eq!(evidence[0]["id"], requested_id);
        }
        "diagnostic" => {
            assert_eq!(result["diagnostic"]["id"], requested_id);
            assert!(!evidence.is_empty());
            assert!(evidence.len() <= 64);
        }
        "coverage_gap" => {
            assert_eq!(result["coverage_gap"]["id"], requested_id);
            assert!(!evidence.is_empty());
            assert!(evidence.len() <= 64);
        }
        "document" => {
            assert_eq!(result["document"]["document_id"], requested_id);
            assert!(!statements.is_empty());
        }
        _ => panic!("unexpected query kind {kind}"),
    }
}

fn tree_fingerprint(root: &Path) -> Vec<(String, u64, String)> {
    fn visit(root: &Path, current: &Path, files: &mut Vec<(String, u64, String)>) {
        let mut entries = fs::read_dir(current)
            .unwrap_or_else(|error| panic!("read {}: {error}", current.display()))
            .collect::<Result<Vec<_>, _>>()
            .expect("read fingerprint entries");
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let metadata = entry.metadata().expect("read fingerprint metadata");
            if metadata.is_dir() {
                visit(root, &path, files);
            } else {
                let bytes = fs::read(&path).expect("read fingerprint file");
                files.push((
                    path.strip_prefix(root)
                        .expect("fingerprint relative path")
                        .to_string_lossy()
                        .replace('\\', "/"),
                    u64::try_from(bytes.len()).expect("fingerprint byte length"),
                    Sha256::digest(&bytes)
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect(),
                ));
            }
        }
    }

    let mut files = Vec::new();
    visit(root, root, &mut files);
    files
}
