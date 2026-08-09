use codenoesis_contracts::{
    K1_ONTOLOGY_VERSION, K1_PORTABLE_GRAPH_VERSION, K1_QUERY_VERSION, K1_SNAPSHOT_VERSION,
    K1ContractError, PortableGraphV2, RepositorySnapshotV11, RepositorySnapshotV11Error,
};
use codenoesis_domain::{K1OutputCapacityProfile, LOCAL_SNAPSHOT_64M_V1, STANDARD_LOCAL_S1_LIMITS};
use serde_json::{Map, Value, json};

#[test]
fn conf_fr_ext_012_k1_output_capacity_profile_is_additive() {
    assert_eq!(STANDARD_LOCAL_S1_LIMITS.canonical_output_bytes, 33_554_432);
    assert_eq!(LOCAL_SNAPSHOT_64M_V1, "local-snapshot-64m-v1");
    assert_eq!(
        K1OutputCapacityProfile::Standard.maximum_bytes(),
        33_554_432
    );
    assert_eq!(
        K1OutputCapacityProfile::LocalSnapshot64MV1.maximum_bytes(),
        67_108_864
    );

    let serializer: fn(
        &RepositorySnapshotV11,
        K1OutputCapacityProfile,
    ) -> Result<Vec<u8>, RepositorySnapshotV11Error> =
        RepositorySnapshotV11::canonical_stdout_with_output_capacity;
    let _ = serializer;
}

#[test]
fn reg_fr_exp_002_k1_script_close_source_spelling_is_inert_data() {
    let mut value = portable_value("view! { <script>{script}</script> }");
    let mut bytes = serde_json::to_vec(&value).expect("serialize portable V2 source spelling");
    bytes.push(b'\n');
    let portable = PortableGraphV2::from_canonical_file(&bytes, digest)
        .expect("source spelling remains inert portable JSON data");
    assert_eq!(
        portable.value()["entities"][0]["properties"]["target_spelling"],
        "view! { <script>{script}</script> }"
    );

    value["entities"][0]["body_text"] = Value::String("forbidden".to_owned());
    value["projection"]["family_sha256"] = family_digests(&value);
    let mut private_bytes =
        serde_json::to_vec(&value).expect("serialize private portable V2 mutation");
    private_bytes.push(b'\n');
    assert_eq!(
        PortableGraphV2::from_canonical_file(&private_bytes, digest)
            .expect_err("private source body must still fail closed"),
        K1ContractError::UnsafePayload {
            reason: "private_field"
        }
    );
}

fn portable_value(target_spelling: &str) -> Value {
    let entity_id = format!("urn:codenoesis:entity:blake3:{}", "a".repeat(64));
    let claim_id = format!("urn:codenoesis:claim:blake3:{}", "b".repeat(64));
    let evidence_id = format!("urn:codenoesis:evidence:blake3:{}", "c".repeat(64));
    let mut value = json!({
        "schema_version": K1_PORTABLE_GRAPH_VERSION,
        "repository": {
            "identity": "urn:codenoesis:test:r9-portable",
            "commit_oid": "d".repeat(40)
        },
        "source_snapshot": {
            "schema_version": K1_SNAPSHOT_VERSION,
            "snapshot_id": format!("urn:codenoesis:snapshot:blake3:{}", "e".repeat(64)),
            "semantic_hash": {"algorithm": "blake3-256", "value": "f".repeat(64)}
        },
        "ontology_version": K1_ONTOLOGY_VERSION,
        "query_contract_version": K1_QUERY_VERSION,
        "projection": {
            "profile": "codenoesis.lossless-portable-projection/v2",
            "family_sha256": {}
        },
        "entities": [{
            "id": entity_id,
            "kind": "rust.call_site",
            "name": "view",
            "evidence_ids": [evidence_id],
            "properties": {"target_spelling": target_spelling}
        }],
        "relationships": [],
        "claims": [{
            "id": claim_id,
            "subject_kind": "entity",
            "subject_id": entity_id,
            "state": "deterministic_fact",
            "evidence_ids": [evidence_id]
        }],
        "evidence": [{
            "id": evidence_id,
            "path": "src/lib.rs",
            "blob_oid": "1".repeat(40),
            "start_byte": 0,
            "end_byte": 1
        }],
        "diagnostics": [],
        "coverage_gaps": [],
        "documents": [],
        "document_statements": []
    });
    value["projection"]["family_sha256"] = family_digests(&value);
    value
}

fn family_digests(value: &Value) -> Value {
    let mut values = Map::new();
    for family in [
        "entities",
        "relationships",
        "claims",
        "evidence",
        "diagnostics",
        "coverage_gaps",
        "documents",
        "document_statements",
    ] {
        values.insert(
            family.to_owned(),
            Value::String(digest(
                &serde_json::to_vec(&value[family]).expect("serialize portable family"),
            )),
        );
    }
    Value::Object(values)
}

fn digest(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}
