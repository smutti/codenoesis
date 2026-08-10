use std::collections::BTreeSet;

use codenoesis_contracts::{
    CodeNoesisErrorV20, LocalExplorerManifestV6, PortableGraphV6, R13_ERROR_VERSION,
    R13_ONTOLOGY_VERSION, R13_PORTABLE_GRAPH_VERSION, R13_QUERY_VERSION, R13_SNAPSHOT_VERSION,
    R13ContractError,
};
use codenoesis_domain::s4_r13::CallableScipCompositionError;
use serde_json::{Map, Value, json};

const PORTABLE_FAMILIES: [&str; 8] = [
    "entities",
    "relationships",
    "claims",
    "evidence",
    "diagnostics",
    "coverage_gaps",
    "documents",
    "document_statements",
];

#[test]
fn ft_fr_exp_005_v6_rejects_unknown_schema_before_projection() {
    let mut value = valid_portable_shell();
    value["schema_version"] = Value::String("codenoesis.portable-graph/v5".to_owned());
    assert_eq!(
        PortableGraphV6::from_canonical_file(&canonical_file(&value), sha256).unwrap_err(),
        R13ContractError::UnsupportedPortableGraphSchema("codenoesis.portable-graph/v5".to_owned())
    );
}

#[test]
fn conf_fr_exp_005_v6_reimports_one_canonical_closed_projection() {
    let value = valid_portable_with_references();
    let bytes = canonical_file(&value);
    let portable = PortableGraphV6::from_canonical_file(&bytes, sha256)
        .expect("reimport canonical R13 portable graph");
    assert_eq!(portable.canonical_file(), bytes);
}

#[test]
fn ft_fr_exp_005_v6_rejects_unknown_and_noncanonical_payloads() {
    let mut unknown = valid_portable_shell();
    unknown["unknown"] = Value::Bool(true);
    assert_eq!(
        PortableGraphV6::from_canonical_file(&canonical_file(&unknown), sha256).unwrap_err(),
        R13ContractError::InvalidProjection
    );

    let value = valid_portable_shell();
    let mut noncanonical = serde_json::to_string_pretty(&value)
        .expect("serialize noncanonical R13 portable graph")
        .into_bytes();
    noncanonical.push(b'\n');
    assert!(matches!(
        PortableGraphV6::from_canonical_file(&noncanonical, sha256),
        Err(R13ContractError::Noncanonical { .. })
    ));
}

#[test]
fn ft_nfr_prv_002_v6_rejects_private_and_unsafe_path_payloads() {
    let mut private = valid_portable_shell();
    private["repository"]["raw_url"] = Value::String("https://credential.invalid".to_owned());
    refresh_family_digests(&mut private);
    assert_eq!(
        PortableGraphV6::from_canonical_file(&canonical_file(&private), sha256).unwrap_err(),
        R13ContractError::UnsafePayload {
            reason: "private_field"
        }
    );

    let mut unsafe_path = valid_portable_with_references();
    unsafe_path["evidence"][0]["path"] = Value::String("../secret.rs".to_owned());
    refresh_family_digests(&mut unsafe_path);
    assert_eq!(
        PortableGraphV6::from_canonical_file(&canonical_file(&unsafe_path), sha256).unwrap_err(),
        R13ContractError::UnsafePayload {
            reason: "unsafe_evidence_path"
        }
    );
}

#[test]
fn ft_fr_exp_005_v6_rejects_duplicate_and_dangling_records() {
    let mut duplicate = valid_portable_with_references();
    let entity = duplicate["entities"][0].clone();
    duplicate["entities"]
        .as_array_mut()
        .expect("R13 entity family")
        .push(entity);
    refresh_family_digests(&mut duplicate);
    assert_eq!(
        PortableGraphV6::from_canonical_file(&canonical_file(&duplicate), sha256).unwrap_err(),
        R13ContractError::IdentityConflict {
            family: "entities",
            id: "entity:a".to_owned()
        }
    );

    let mut dangling = valid_portable_with_references();
    dangling["relationships"][0]["source"] = Value::String("entity:missing".to_owned());
    refresh_family_digests(&mut dangling);
    assert_eq!(
        PortableGraphV6::from_canonical_file(&canonical_file(&dangling), sha256).unwrap_err(),
        R13ContractError::ReferenceMismatch {
            family: "relationships",
            id: "entity:missing".to_owned()
        }
    );
}

#[test]
fn ft_fr_exp_005_v6_rejects_document_and_statement_reference_mismatches() {
    let mut document = valid_portable_with_references();
    document["documents"][0]["subject_id"] = Value::String("entity:missing".to_owned());
    refresh_family_digests(&mut document);
    assert_eq!(
        PortableGraphV6::from_canonical_file(&canonical_file(&document), sha256).unwrap_err(),
        R13ContractError::ReferenceMismatch {
            family: "documents",
            id: "entity:missing".to_owned()
        }
    );

    let mut statement = valid_portable_with_references();
    statement["document_statements"][0]["document_id"] =
        Value::String("document:missing".to_owned());
    refresh_family_digests(&mut statement);
    assert_eq!(
        PortableGraphV6::from_canonical_file(&canonical_file(&statement), sha256).unwrap_err(),
        R13ContractError::ReferenceMismatch {
            family: "document_statements",
            id: "document:missing".to_owned()
        }
    );
}

#[test]
fn ft_fr_exp_005_v6_rejects_incomplete_callable_compiler_join_lineage() {
    let mut missing_evidence = valid_portable_with_references();
    missing_evidence["relationships"][0]["evidence_ids"] = json!([]);
    refresh_family_digests(&mut missing_evidence);
    assert_eq!(
        PortableGraphV6::from_canonical_file(&canonical_file(&missing_evidence), sha256)
            .unwrap_err(),
        R13ContractError::InvalidProjection
    );

    let mut missing_claim = valid_portable_with_references();
    missing_claim["claims"] = json!([]);
    refresh_family_digests(&mut missing_claim);
    assert_eq!(
        PortableGraphV6::from_canonical_file(&canonical_file(&missing_claim), sha256).unwrap_err(),
        R13ContractError::InvalidProjection
    );

    let mut missing_signature = valid_portable_with_references();
    missing_signature["relationships"]
        .as_array_mut()
        .expect("R13 relationship family")
        .pop();
    refresh_family_digests(&mut missing_signature);
    assert_eq!(
        PortableGraphV6::from_canonical_file(&canonical_file(&missing_signature), sha256)
            .unwrap_err(),
        R13ContractError::InvalidProjection
    );

    let mut missing_statement = valid_portable_with_references();
    missing_statement["document_statements"] = json!([]);
    refresh_family_digests(&mut missing_statement);
    assert_eq!(
        PortableGraphV6::from_canonical_file(&canonical_file(&missing_statement), sha256)
            .unwrap_err(),
        R13ContractError::InvalidProjection
    );
}

#[test]
fn conf_fr_cli_001_r13_typed_failures_emit_only_error_v20_codes() {
    let errors = [
        CodeNoesisErrorV20::unsupported_composition("missing_binding"),
        CodeNoesisErrorV20::from_composition(&CallableScipCompositionError::SignatureCardinality {
            callable_id: "callable".to_owned(),
            observed: 0,
        }),
        CodeNoesisErrorV20::from_composition(
            &CallableScipCompositionError::DuplicateCompilerOwnership {
                callable_id: "callable".to_owned(),
                observed: 2,
            },
        ),
        CodeNoesisErrorV20::from_composition(&CallableScipCompositionError::InvalidJoinEvidence {
            callable_id: "callable".to_owned(),
        }),
        CodeNoesisErrorV20::from_composition(&CallableScipCompositionError::LimitExceeded {
            maximum: 200_000,
            observed: 200_001,
        }),
        CodeNoesisErrorV20::invalid_snapshot(),
        CodeNoesisErrorV20::invalid_query("snapshot_invalid"),
        CodeNoesisErrorV20::from_contract(&R13ContractError::InvalidSnapshot, false),
        CodeNoesisErrorV20::from_contract(&R13ContractError::InvalidProjection, false),
        CodeNoesisErrorV20::from_contract(&R13ContractError::AssetIntegrityMismatch, true),
    ];
    let allowed = allowed_error_codes();
    for error in errors {
        assert_eq!(error.value()["schema_version"], R13_ERROR_VERSION);
        assert_eq!(error.value()["retryable"], false);
        let code = error.value()["code"].as_str().expect("R13 error code");
        assert!(allowed.contains(code), "unexpected ErrorV20 code {code}");
        assert!(
            error
                .canonical_stderr()
                .expect("serialize ErrorV20")
                .ends_with(b"\n")
        );
    }
}

#[test]
fn sec_fr_exp_005_v6_explorer_rejects_changed_viewer_and_active_csp() {
    let portable =
        PortableGraphV6::from_canonical_file(&canonical_file(&valid_portable_shell()), sha256)
            .expect("construct R13 portable shell");
    let viewer = b"<script>const value = '<\\/script>';</script>\n";
    let viewer_sha256 = sha256(viewer);
    LocalExplorerManifestV6::new(
        &portable,
        viewer,
        &viewer_sha256,
        "default-src 'none'; script-src 'sha256-reviewed'",
        sha256,
    )
    .expect("accept reviewed immutable viewer bytes");
    assert_eq!(
        LocalExplorerManifestV6::new(
            &portable,
            b"changed",
            &viewer_sha256,
            "default-src 'none'",
            sha256,
        )
        .unwrap_err(),
        R13ContractError::AssetIntegrityMismatch
    );
    assert_eq!(
        LocalExplorerManifestV6::new(
            &portable,
            viewer,
            &viewer_sha256,
            "default-src https:",
            sha256,
        )
        .unwrap_err(),
        R13ContractError::AssetIntegrityMismatch
    );
}

fn valid_portable_shell() -> Value {
    let mut value = json!({
        "claims": [],
        "coverage_gaps": [],
        "diagnostics": [],
        "document_statements": [],
        "documents": [],
        "entities": [],
        "evidence": [],
        "ontology_version": R13_ONTOLOGY_VERSION,
        "projection": {
            "profile": "codenoesis.lossless-portable-projection/v6",
            "family_sha256": {}
        },
        "query_contract_version": R13_QUERY_VERSION,
        "relationships": [],
        "repository": {
            "identity": "urn:codenoesis:test:r13-portable"
        },
        "schema_version": R13_PORTABLE_GRAPH_VERSION,
        "source_snapshot": {
            "schema_version": R13_SNAPSHOT_VERSION,
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

fn valid_portable_with_references() -> Value {
    let mut value = valid_portable_shell();
    value["entities"] = json!([
        {
            "id": "entity:a",
            "kind": "rust.function",
            "evidence_ids": ["evidence:a"]
        },
        {
            "id": "entity:b",
            "kind": "compiler.symbol",
            "evidence_ids": ["evidence:b"]
        },
        {
            "id": "entity:signature",
            "kind": "rust.callable_signature",
            "evidence_ids": ["evidence:a"]
        }
    ]);
    value["relationships"] = json!([
        {
            "id": "relationship:a",
            "kind": "HAS_COMPILER_SYMBOL",
            "source": "entity:a",
            "target": "entity:b",
            "evidence_ids": ["evidence:a", "evidence:b"]
        },
        {
            "id": "relationship:signature",
            "kind": "HAS_SIGNATURE",
            "source": "entity:a",
            "target": "entity:signature",
            "evidence_ids": ["evidence:a"]
        }
    ]);
    value["claims"] = json!([{
        "id": "claim:a",
        "subject_kind": "relationship",
        "subject_id": "relationship:a",
        "state": "deterministic_fact",
        "evidence_ids": ["evidence:a", "evidence:b"]
    }]);
    value["evidence"] = json!([
        {
            "id": "evidence:a",
            "path": "src/lib.rs",
            "blob_oid": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "start_byte": 0,
            "end_byte": 1
        },
        {
            "id": "evidence:b",
            "artifact_sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "document_path": "src/lib.rs"
        }
    ]);
    value["coverage_gaps"] = json!([{
        "id": "coverage:a",
        "evidence_ids": ["evidence:a"]
    }]);
    value["documents"] = json!([{
        "document_id": "document:a",
        "subject_id": "entity:a",
        "path": "entities/a.md"
    }]);
    value["document_statements"] = json!([{
        "statement_id": "statement:a",
        "document_id": "document:a",
        "subject_ids": ["relationship:a"],
        "truth_state": "deterministic_fact",
        "evidence_ids": ["evidence:a", "evidence:b"],
        "coverage_gap_ids": ["coverage:a"]
    }]);
    refresh_family_digests(&mut value);
    value
}

fn refresh_family_digests(value: &mut Value) {
    let mut digests = Map::new();
    for family in PORTABLE_FAMILIES {
        let bytes = serde_json::to_vec(&value[family]).expect("serialize R13 portable family");
        digests.insert(family.to_owned(), Value::String(sha256(&bytes)));
    }
    value["projection"]["family_sha256"] = Value::Object(digests);
}

fn canonical_file(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(value).expect("serialize R13 portable graph");
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

fn allowed_error_codes() -> BTreeSet<&'static str> {
    BTreeSet::from([
        "input.unsupported_rust_callable_scip_composition",
        "extraction.callable_scip_signature_missing",
        "extraction.callable_scip_duplicate_owner",
        "extraction.callable_scip_evidence_invalid",
        "extraction.callable_scip_limit_exceeded",
        "snapshot.invalid_v15",
        "query.invalid_v15",
        "export.invalid_snapshot",
        "export.invalid_portable_graph_v6",
        "explorer.asset_integrity_mismatch",
    ])
}
