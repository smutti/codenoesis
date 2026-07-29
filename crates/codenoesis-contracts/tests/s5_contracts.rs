use codenoesis_contracts::{AnalysisCacheEntryV1, AnalysisCacheEntryV1Error, CodeNoesisErrorV7};
use codenoesis_domain::knowledge::EntityKind;
use codenoesis_domain::s4::{
    S4_ONTOLOGY_VERSION, S4_TREE_SITTER_EXTRACTOR_VERSION, S4_WORKSPACE_EXTRACTOR_VERSION,
    WorkspaceVisibility,
};
use codenoesis_domain::s5::{
    AnalysisCacheEntry, AnalysisCacheKey, RustDeclarationObservation, RustSourceAnalysis,
};
use codenoesis_domain::storage::StorageError;

#[test]
fn conf_fr_inc_003_analysis_cache_v1_round_trip() {
    let entry = AnalysisCacheEntry::new(
        AnalysisCacheKey {
            repository_identity: "urn:codenoesis:fixture:s5-incremental-refresh-v1".to_owned(),
            source_file_id:
                "urn:codenoesis:entity:blake3:cc0026b78ded40638ecdd0443d1abac4128001845cf26561876e5ff68fd5ab87"
                    .to_owned(),
            canonical_source_path: "crates/model/src/item.rs".to_owned(),
            source_blob_oid: "ab58a9d6417ab6852d5311994e480fba6185002f".to_owned(),
            crate_id:
                "urn:codenoesis:entity:blake3:7409cb974bde86fa2508c2dd6eb76588b82bed0bce60c4cd1db46aa70443dbc4"
                    .to_owned(),
            canonical_module_path: "crate::item".to_owned(),
            language_extractor: S4_TREE_SITTER_EXTRACTOR_VERSION.to_owned(),
            workspace_mapper: S4_WORKSPACE_EXTRACTOR_VERSION.to_owned(),
            ontology: S4_ONTOLOGY_VERSION.to_owned(),
        },
        RustSourceAnalysis {
            declarations: vec![
                RustDeclarationObservation {
                    kind: EntityKind::RustStruct,
                    name: "Item".to_owned(),
                    visibility: WorkspaceVisibility::Public,
                },
                RustDeclarationObservation {
                    kind: EntityKind::RustFunction,
                    name: "item_id".to_owned(),
                    visibility: WorkspaceVisibility::Public,
                },
            ],
            modules: Vec::new(),
            imports: Vec::new(),
            unsupported_construct: false,
        },
    );
    assert_eq!(
        entry.analysis_cache_entry_id,
        "urn:codenoesis:analysis-cache-entry:blake3:ca46085d6bc162e2ec49c6002ae86f60014c26d38eeb115456147ff65e058ec9"
    );
    let contract = AnalysisCacheEntryV1::from_domain(&entry);
    let bytes = contract.canonical_bytes().expect("serialize S5 cache");
    assert_eq!(
        AnalysisCacheEntryV1::parse(&bytes)
            .expect("parse S5 cache")
            .to_domain()
            .expect("convert S5 cache"),
        entry
    );

    let mut corrupt: serde_json::Value =
        serde_json::from_slice(&bytes).expect("parse cache for corruption");
    corrupt["payload_hash"] = serde_json::Value::String("0".repeat(64));
    assert!(matches!(
        AnalysisCacheEntryV1::parse(
            &serde_json::to_vec(&corrupt).expect("serialize corrupt cache")
        ),
        Err(AnalysisCacheEntryV1Error::InvalidPayloadHash)
    ));

    let incompatible = AnalysisCacheEntry::new(
        AnalysisCacheKey {
            language_extractor: "codenoesis.rust-tree-sitter/s4-v0".to_owned(),
            ..entry.key.clone()
        },
        entry.analysis.clone(),
    );
    assert!(matches!(
        AnalysisCacheEntryV1::parse(
            &AnalysisCacheEntryV1::from_domain(&incompatible)
                .canonical_bytes()
                .expect("serialize incompatible cache")
        ),
        Err(AnalysisCacheEntryV1Error::InvalidContract)
    ));

    let mut invalid_analysis = entry.analysis.clone();
    invalid_analysis.declarations[0].name.clear();
    let invalid_observation = AnalysisCacheEntry::new(entry.key, invalid_analysis);
    assert!(matches!(
        AnalysisCacheEntryV1::parse(
            &AnalysisCacheEntryV1::from_domain(&invalid_observation)
                .canonical_bytes()
                .expect("serialize invalid cache observation")
        ),
        Err(AnalysisCacheEntryV1Error::InvalidContract)
    ));
}

#[test]
fn conf_fr_cli_004_error_v7_is_closed_and_typed() {
    let baseline = error_value(&CodeNoesisErrorV7::baseline_missing(
        "urn:codenoesis:fixture:s5-incremental-refresh-v1",
    ));
    assert_eq!(
        baseline,
        serde_json::json!({
            "schema_version": "codenoesis.error/v7",
            "code": "incremental.baseline_missing",
            "stage": "incremental",
            "message": "validated visible S4 baseline is missing",
            "retryable": false,
            "context": {
                "component": "baseline_head",
                "expected_repository_identity":
                    "urn:codenoesis:fixture:s5-incremental-refresh-v1"
            }
        })
    );

    let cache = error_value(&CodeNoesisErrorV7::cache_corrupt(
        "analysis-cache/entry",
        &"a".repeat(64),
        &"b".repeat(64),
    ));
    assert_eq!(cache["code"], "incremental.cache_corrupt");
    assert_eq!(
        cache["context"]
            .as_object()
            .expect("S5 cache error context")
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        ["component", "expected_hash", "observed_hash", "path"]
    );

    let writer_busy = error_value(&CodeNoesisErrorV7::from_storage(&StorageError::WriterBusy));
    assert_eq!(writer_busy["code"], "storage.writer_busy");
    assert_eq!(writer_busy["stage"], "storage");
    assert_eq!(writer_busy["retryable"], true);
    assert_eq!(writer_busy["context"], serde_json::json!({}));

    let expected_snapshot_id = format!("urn:codenoesis:snapshot:blake3:{}", "a".repeat(64));
    let actual_snapshot_id = format!("urn:codenoesis:snapshot:blake3:{}", "b".repeat(64));
    let head_conflict = error_value(&CodeNoesisErrorV7::from_storage(
        &StorageError::HeadConflict {
            expected: Some(expected_snapshot_id.clone()),
            actual: Some(actual_snapshot_id.clone()),
        },
    ));
    assert_eq!(head_conflict["code"], "publication.head_conflict");
    assert_eq!(head_conflict["stage"], "publication");
    assert_eq!(head_conflict["retryable"], true);
    assert_eq!(
        head_conflict["context"],
        serde_json::json!({
            "expected_snapshot_id": expected_snapshot_id,
            "actual_snapshot_id": actual_snapshot_id
        })
    );
}

fn error_value(error: &CodeNoesisErrorV7) -> serde_json::Value {
    let bytes = error.canonical_stderr().expect("serialize ErrorV7");
    assert_eq!(bytes.last(), Some(&b'\n'));
    serde_json::from_slice(&bytes).expect("parse ErrorV7")
}
