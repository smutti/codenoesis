use std::fs;
use std::path::{Path, PathBuf};

use codenoesis_contracts::{
    CodeNoesisErrorV15, PortableGraphV1, R8_DETERMINISM_PERMUTATIONS, R8_PORTABLE_GRAPH_VERSION,
    R8ContractError,
};
use codenoesis_domain::storage::{
    LocalSnapshotHead, SNAPSHOT_SCHEMA_VERSION_V10, SemanticHash, SnapshotId,
};
use codenoesis_domain::{ObjectId, RepositoryIdentity};
use serde_json::{Value, json};

#[test]
fn conf_fr_exp_001_portable_graph_v1_lossless_reimport() {
    let bytes = fs::read(fixture_root().join("portable-graph.json"))
        .expect("read canonical PortableGraphV1 fixture");
    let portable = PortableGraphV1::from_canonical_file(&bytes, test_digest)
        .expect("validate canonical PortableGraphV1 fixture");
    assert_eq!(
        portable.value()["schema_version"],
        R8_PORTABLE_GRAPH_VERSION
    );
    assert_eq!(portable.canonical_file(), bytes);

    let digests = portable.family_digests().expect("reimport family digests");
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
        assert_eq!(
            digests[family]["count"],
            portable.value()[family]
                .as_array()
                .expect("portable family")
                .len()
        );
    }
}

#[test]
fn gt_fr_exp_001_duplicate_loss_reorder_and_hash_rejection() {
    let fixture = canonical_fixture();

    let mut duplicate = fixture.clone();
    let entity = duplicate["entities"][0].clone();
    duplicate["entities"]
        .as_array_mut()
        .expect("entity family")
        .push(entity);
    assert!(matches!(
        reimport(&duplicate),
        Err(R8ContractError::IdentityConflict {
            family: "entity",
            ..
        })
    ));

    let mut missing = fixture.clone();
    missing["entities"]
        .as_array_mut()
        .expect("entity family")
        .pop();
    assert!(matches!(
        reimport(&missing),
        Err(R8ContractError::ReferenceMismatch { .. })
    ));

    let mut reordered = fixture.clone();
    reordered["entities"]
        .as_array_mut()
        .expect("entity family")
        .reverse();
    assert!(matches!(
        reimport(&reordered),
        Err(R8ContractError::Noncanonical { .. })
    ));

    let mut unsupported = fixture;
    unsupported["schema_version"] = Value::String("codenoesis.portable-graph/v2".to_owned());
    assert!(matches!(
        reimport(&unsupported),
        Err(R8ContractError::UnsupportedPortableGraphSchema(_))
    ));

    let mut unknown_nested = canonical_fixture();
    unknown_nested["entities"][0]
        .as_object_mut()
        .expect("entity object")
        .insert("unknown".to_owned(), Value::Bool(true));
    assert!(matches!(
        reimport(&unknown_nested),
        Err(R8ContractError::InvalidProjection {
            reason: "unknown_field",
            ..
        })
    ));

    let mut mismatched_claim_kind = canonical_fixture();
    mismatched_claim_kind["claims"][0]["subject_kind"] = Value::String("entity".to_owned());
    assert!(matches!(
        reimport(&mismatched_claim_kind),
        Err(R8ContractError::ReferenceMismatch {
            family: "claim_subject",
            ..
        })
    ));

    let duplicate_member = String::from_utf8(canonical_fixture_bytes())
        .expect("UTF-8 portable fixture")
        .replacen("{\"claims\":", "{\"claims\":[],\"claims\":", 1)
        .into_bytes();
    assert!(matches!(
        PortableGraphV1::from_canonical_file(&duplicate_member, test_digest),
        Err(R8ContractError::InvalidProjection {
            reason: "duplicate_member",
            ..
        })
    ));
}

#[test]
fn conf_fr_exp_001_error_v15_is_strict_and_lf_terminated() {
    let error = CodeNoesisErrorV15::from_contract(&R8ContractError::LimitExceeded {
        limit: "portable_graph_bytes",
        maximum: 268_435_456,
        observed: 268_435_457,
    });
    let bytes = error.canonical_stderr().expect("serialize ErrorV15");
    assert_eq!(bytes.last(), Some(&b'\n'));
    let value: Value = serde_json::from_slice(&bytes).expect("parse ErrorV15");
    assert_eq!(value["schema_version"], "codenoesis.error/v15");
    assert_eq!(value["code"], "export.limit_exceeded");
    assert_eq!(value["stage"], "export");
    assert_eq!(value["retryable"], false);
}

#[test]
fn conf_fr_exp_001_fifty_source_permutations_are_byte_identical() {
    let fixture = canonical_fixture();
    let expected = canonical_fixture_bytes();
    let head = fixture_head(&fixture);
    let documentation = fixture_documentation(&fixture);

    for seed in 0..R8_DETERMINISM_PERMUTATIONS {
        let semantic = fixture_semantic(&fixture, seed);
        let portable =
            PortableGraphV1::from_validated_v10(&semantic, &head, &documentation, test_digest)
                .expect("project permuted R8 source");
        assert_eq!(portable.canonical_file(), expected, "source seed {seed}");
    }
}

fn canonical_fixture() -> Value {
    serde_json::from_slice(&canonical_fixture_bytes()).expect("parse PortableGraphV1 fixture")
}

fn canonical_fixture_bytes() -> Vec<u8> {
    fs::read(fixture_root().join("portable-graph.json")).expect("read PortableGraphV1 fixture")
}

fn fixture_head(fixture: &Value) -> LocalSnapshotHead {
    LocalSnapshotHead {
        repository_identity: RepositoryIdentity::parse(
            fixture["repository"]["identity"]
                .as_str()
                .expect("fixture repository identity"),
        )
        .expect("parse fixture repository identity"),
        snapshot_id: SnapshotId::parse(
            fixture["source_snapshot"]["snapshot_id"]
                .as_str()
                .expect("fixture snapshot ID"),
        )
        .expect("parse fixture snapshot ID"),
        commit_oid: ObjectId::parse_sha1(
            fixture["repository"]["commit_oid"]
                .as_str()
                .expect("fixture commit OID"),
        )
        .expect("parse fixture commit OID"),
        snapshot_schema_version: SNAPSHOT_SCHEMA_VERSION_V10.to_owned(),
        semantic_hash: SemanticHash::blake3(
            "codenoesis.repository-snapshot.semantic.v10",
            fixture["source_snapshot"]["semantic_hash"]["value"]
                .as_str()
                .expect("fixture semantic hash"),
        ),
        graph_semantic_hash: SemanticHash::blake3(
            "codenoesis.knowledge-graph.semantic.v7",
            "0000000000000000000000000000000000000000000000000000000000000000",
        ),
        generation: 1,
        artifacts: Vec::new(),
    }
}

fn fixture_semantic(fixture: &Value, seed: u64) -> Value {
    json!({
        "repository": fixture["repository"],
        "ontology_version": fixture["ontology_version"],
        "knowledge_graph": {
            "entities": permuted(fixture, "entities", seed),
            "relationships": permuted(fixture, "relationships", seed + 1),
            "claims": permuted(fixture, "claims", seed + 2),
            "evidence": permuted(fixture, "evidence", seed + 3),
            "diagnostics": permuted(fixture, "diagnostics", seed + 4),
            "coverage": permuted(fixture, "coverage_gaps", seed + 5)
        }
    })
}

fn fixture_documentation(fixture: &Value) -> Value {
    let mut documents = fixture["documents"]
        .as_array()
        .expect("fixture documents")
        .clone();
    for document in &mut documents {
        let document_id = document["document_id"]
            .as_str()
            .expect("fixture document ID");
        let statements = fixture["document_statements"]
            .as_array()
            .expect("fixture statements")
            .iter()
            .filter(|statement| statement["document_id"] == document_id)
            .cloned()
            .map(|mut statement| {
                statement
                    .as_object_mut()
                    .expect("fixture statement object")
                    .remove("document_id");
                statement
            })
            .collect::<Vec<_>>();
        document
            .as_object_mut()
            .expect("fixture document object")
            .insert("statements".to_owned(), Value::Array(statements));
    }
    json!({
        "schema_version": "codenoesis.documentation-manifest/v1",
        "repository_identity": fixture["repository"]["identity"],
        "snapshot_id": fixture["source_snapshot"]["snapshot_id"],
        "snapshot_semantic_hash": fixture["source_snapshot"]["semantic_hash"],
        "documents": documents
    })
}

fn permuted(fixture: &Value, family: &str, seed: u64) -> Vec<Value> {
    let mut values = fixture[family].as_array().expect("fixture family").clone();
    if !values.is_empty() {
        let length = values.len();
        values.rotate_left(usize::try_from(seed).expect("seed usize") % length);
        if seed % 2 == 1 {
            values.reverse();
        }
    }
    values
}

fn reimport(value: &Value) -> Result<PortableGraphV1, R8ContractError> {
    let mut bytes = serde_json::to_vec(&value).expect("serialize PortableGraphV1 mutation");
    bytes.push(b'\n');
    PortableGraphV1::from_canonical_file(&bytes, test_digest)
}

fn test_digest(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/s4/portable-explorer-v1")
}
