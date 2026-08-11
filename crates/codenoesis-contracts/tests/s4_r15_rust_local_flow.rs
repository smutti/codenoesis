use std::collections::BTreeSet;

use codenoesis_contracts::{
    CodeNoesisErrorV22, LocalExplorerManifestV8, PortableGraphV8, R15_ERROR_VERSION,
    R15_LOCAL_EXPLORER_VERSION, R15_ONTOLOGY_VERSION, R15_PORTABLE_GRAPH_VERSION,
    R15_QUERY_VERSION, R15_RULE_VERSION, R15_SNAPSHOT_VERSION, R15ContractError,
};
use codenoesis_domain::s4_r15::{LocalFlowError, LocalFlowLimit};
use serde_json::{Map, Value, json};

const PORTABLE_FAMILIES: [&str; 9] = [
    "entities",
    "relationships",
    "claims",
    "evidence",
    "diagnostics",
    "coverage_gaps",
    "documents",
    "document_statements",
    "local_flow_index",
];

#[test]
fn conf_fr_exp_007_v8_reimports_canonical_lossless_local_flow_projection() {
    let bytes = canonical_file(&valid_portable());
    let portable = PortableGraphV8::from_canonical_file(&bytes, sha256)
        .expect("reimport canonical R15 portable graph");
    assert_eq!(portable.canonical_file(), bytes);
    assert_eq!(
        portable.value()["local_flow_index"]["derivations"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
}

#[test]
fn ft_fr_exp_007_v8_rejects_unknown_private_and_missing_derivation_inputs() {
    let mut unknown = valid_portable();
    unknown["unknown"] = Value::Bool(true);
    assert_eq!(
        PortableGraphV8::from_canonical_file(&canonical_file(&unknown), sha256).unwrap_err(),
        R15ContractError::InvalidProjection
    );

    let mut private = valid_portable();
    private["repository"]["condition_text"] = Value::String("secret".to_owned());
    refresh_family_digests(&mut private);
    assert_eq!(
        PortableGraphV8::from_canonical_file(&canonical_file(&private), sha256).unwrap_err(),
        R15ContractError::UnsafePayload {
            reason: "private_field"
        }
    );

    let mut missing = valid_portable();
    missing["local_flow_index"]["derivations"] = json!([]);
    refresh_family_digests(&mut missing);
    assert_eq!(
        PortableGraphV8::from_canonical_file(&canonical_file(&missing), sha256).unwrap_err(),
        R15ContractError::InvalidProjection
    );

    let mut wrong_evidence = valid_portable();
    wrong_evidence["evidence"]
        .as_array_mut()
        .expect("R15 evidence family")
        .push(json!({
            "id": "evidence:b",
            "path": "src/lib.rs",
            "blob_oid": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "start_byte": 2,
            "end_byte": 3
        }));
    wrong_evidence["local_flow_index"]["derivations"][0]["input_evidence_ids"] =
        json!(["evidence:b"]);
    refresh_family_digests(&mut wrong_evidence);
    assert_eq!(
        PortableGraphV8::from_canonical_file(&canonical_file(&wrong_evidence), sha256).unwrap_err(),
        R15ContractError::InvalidProjection
    );
}

#[test]
fn ft_fr_exp_007_v8_rejects_order_reference_hash_path_and_noncanonical_bytes() {
    let mut duplicate = valid_portable();
    let duplicate_entity = duplicate["entities"][2].clone();
    duplicate["entities"]
        .as_array_mut()
        .expect("R15 entity family")
        .push(duplicate_entity);
    refresh_family_digests(&mut duplicate);
    assert!(matches!(
        PortableGraphV8::from_canonical_file(&canonical_file(&duplicate), sha256),
        Err(R15ContractError::IdentityConflict {
            family: "entities",
            ..
        })
    ));

    let mut dangling = valid_portable();
    dangling["relationships"][0]["target"] = Value::String("entity:missing".to_owned());
    refresh_family_digests(&mut dangling);
    assert_eq!(
        PortableGraphV8::from_canonical_file(&canonical_file(&dangling), sha256).unwrap_err(),
        R15ContractError::ReferenceMismatch {
            family: "relationships",
            id: "entity:missing".to_owned()
        }
    );

    let mut order = valid_portable();
    order["relationships"]
        .as_array_mut()
        .expect("R15 relationship family")
        .swap(0, 1);
    refresh_family_digests(&mut order);
    assert!(matches!(
        PortableGraphV8::from_canonical_file(&canonical_file(&order), sha256),
        Err(R15ContractError::IdentityConflict {
            family: "relationships",
            ..
        })
    ));

    let mut unsafe_path = valid_portable();
    unsafe_path["evidence"][0]["path"] = Value::String("../secret.rs".to_owned());
    refresh_family_digests(&mut unsafe_path);
    assert_eq!(
        PortableGraphV8::from_canonical_file(&canonical_file(&unsafe_path), sha256).unwrap_err(),
        R15ContractError::UnsafePayload {
            reason: "unsafe_evidence_path"
        }
    );

    let mut hash = valid_portable();
    hash["projection"]["family_sha256"]["entities"] = Value::String("0".repeat(64));
    assert_eq!(
        PortableGraphV8::from_canonical_file(&canonical_file(&hash), sha256).unwrap_err(),
        R15ContractError::InvalidProjection
    );

    let mut noncanonical = serde_json::to_string_pretty(&valid_portable())
        .expect("serialize noncanonical R15 graph")
        .into_bytes();
    noncanonical.push(b'\n');
    assert!(matches!(
        PortableGraphV8::from_canonical_file(&noncanonical, sha256),
        Err(R15ContractError::Noncanonical { .. })
    ));
}

#[test]
fn sec_fr_exp_007_v8_explorer_preserves_reviewed_asset_authority() {
    let portable = PortableGraphV8::from_canonical_file(&canonical_file(&valid_portable()), sha256)
        .expect("construct R15 portable graph");
    let viewer = b"<script>const value = '<\\/script>';</script>\n";
    let viewer_sha256 = sha256(viewer);
    let manifest = LocalExplorerManifestV8::new(
        &portable,
        viewer,
        &viewer_sha256,
        "default-src 'none'; script-src 'sha256-reviewed'",
        sha256,
    )
    .expect("accept reviewed immutable viewer");
    assert_eq!(
        manifest.value()["schema_version"],
        R15_LOCAL_EXPLORER_VERSION
    );
    assert_eq!(
        manifest.value()["capabilities"]["local_flow_derivations"],
        true
    );

    for (candidate, digest, policy) in [
        (
            b"changed".as_slice(),
            viewer_sha256.as_str(),
            "default-src 'none'",
        ),
        (
            viewer.as_slice(),
            viewer_sha256.as_str(),
            "default-src https:",
        ),
    ] {
        assert_eq!(
            LocalExplorerManifestV8::new(&portable, candidate, digest, policy, sha256).unwrap_err(),
            R15ContractError::AssetIntegrityMismatch
        );
    }
}

#[test]
fn conf_fr_cli_001_r15_typed_failures_emit_only_error_v22_codes() {
    let errors = [
        CodeNoesisErrorV22::invalid_profile("rust-local-flow-v2"),
        CodeNoesisErrorV22::unsupported_composition("boundary_profile"),
        CodeNoesisErrorV22::from_local_flow(&LocalFlowError::Cycle),
        CodeNoesisErrorV22::from_local_flow(&LocalFlowError::LimitExceeded {
            limit: LocalFlowLimit::NestedBranches,
            maximum: 64,
            observed: 65,
        }),
        CodeNoesisErrorV22::invalid_snapshot(),
        CodeNoesisErrorV22::invalid_query(),
        CodeNoesisErrorV22::from_contract(&R15ContractError::InvalidProjection),
        CodeNoesisErrorV22::from_explorer_contract(&R15ContractError::AssetIntegrityMismatch),
        CodeNoesisErrorV22::unsafe_output_path(&"0".repeat(64), "symlink"),
    ];
    let allowed = BTreeSet::from([
        "input.invalid_rust_flow_profile",
        "input.unsupported_rust_flow_composition",
        "extraction.local_flow_cycle",
        "extraction.local_flow_limit_exceeded",
        "snapshot.invalid_v17",
        "query.invalid_v17",
        "export.invalid_portable_graph_v8",
        "explorer.asset_integrity_mismatch",
        "input.unsafe_output_path",
    ]);
    for error in errors {
        assert_eq!(error.value()["schema_version"], R15_ERROR_VERSION);
        assert_eq!(error.value()["retryable"], false);
        let code = error.value()["code"].as_str().expect("R15 error code");
        assert!(allowed.contains(code), "unexpected ErrorV22 code {code}");
        assert!(
            error
                .canonical_stderr()
                .expect("serialize ErrorV22")
                .ends_with(b"\n")
        );
    }
}

fn valid_portable() -> Value {
    let mut value = json!({
        "claims": [],
        "coverage_gaps": [],
        "diagnostics": [],
        "document_statements": [],
        "documents": [],
        "entities": [
            {
                "id": "entity:block:a",
                "kind": "rust.syntax_basic_block",
                "evidence_id": "evidence:a"
            },
            {
                "id": "entity:block:b",
                "kind": "rust.syntax_basic_block",
                "evidence_id": "evidence:a"
            },
            {
                "id": "entity:callable",
                "kind": "rust.callable",
                "evidence_id": "evidence:a"
            }
        ],
        "evidence": [{
            "id": "evidence:a",
            "path": "src/lib.rs",
            "blob_oid": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "start_byte": 0,
            "end_byte": 1
        }],
        "local_flow_index": {
            "schema_version": "codenoesis.local-flow-index/v1",
            "rule_version": R15_RULE_VERSION,
            "completed_callable_ids": ["entity:callable"],
            "block_entity_ids": ["entity:block:a", "entity:block:b"],
            "flow_node_relationship_ids": [],
            "condition_relationship_ids": [],
            "direct_syntax_relationship_ids": ["relationship:direct"],
            "reachability_relationship_ids": ["relationship:reach"],
            "must_reach_relationship_ids": [],
            "may_reach_relationship_ids": [],
            "derivations": [{
                "relationship_id": "relationship:reach",
                "rule_version": R15_RULE_VERSION,
                "input_entity_ids": ["entity:block:a", "entity:block:b"],
                "input_relationship_ids": ["relationship:direct"],
                "input_evidence_ids": ["evidence:a"]
            }]
        },
        "ontology_version": R15_ONTOLOGY_VERSION,
        "projection": {
            "profile": "codenoesis.lossless-portable-projection/v8",
            "family_sha256": {}
        },
        "query_contract_version": R15_QUERY_VERSION,
        "relationships": [
            {
                "id": "relationship:direct",
                "kind": "SYNTAX_NEXT",
                "source": "entity:block:a",
                "target": "entity:block:b",
                "evidence_ids": ["evidence:a"]
            },
            {
                "id": "relationship:reach",
                "kind": "SYNTAX_REACHES",
                "source": "entity:block:a",
                "target": "entity:block:b",
                "evidence_ids": ["evidence:a"]
            }
        ],
        "repository": {
            "identity": "urn:codenoesis:test:r15-portable"
        },
        "schema_version": R15_PORTABLE_GRAPH_VERSION,
        "source_snapshot": {
            "schema_version": R15_SNAPSHOT_VERSION,
            "snapshot_id": "urn:codenoesis:snapshot:blake3:0000000000000000000000000000000000000000000000000000000000000000",
            "semantic_hash": {
                "algorithm": "blake3-256",
                "value": "0000000000000000000000000000000000000000000000000000000000000000"
            }
        }
    });
    refresh_family_digests(&mut value);
    value
}

fn refresh_family_digests(value: &mut Value) {
    let mut digests = Map::new();
    for family in PORTABLE_FAMILIES {
        let bytes = serde_json::to_vec(&value[family]).expect("serialize R15 portable family");
        digests.insert(family.to_owned(), Value::String(sha256(&bytes)));
    }
    value["projection"]["family_sha256"] = Value::Object(digests);
}

fn canonical_file(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(value).expect("serialize R15 portable graph");
    bytes.push(b'\n');
    bytes
}

fn sha256(bytes: &[u8]) -> String {
    let mut digest = [0_u8; 32];
    for (index, byte) in bytes.iter().copied().enumerate() {
        digest[index % 32] ^= byte;
    }
    digest
        .iter()
        .fold(String::with_capacity(64), |mut output, byte| {
            use std::fmt::Write as _;
            write!(&mut output, "{byte:02x}").expect("write test digest");
            output
        })
}
