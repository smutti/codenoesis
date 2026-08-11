use std::collections::BTreeSet;

use codenoesis_contracts::{
    CodeNoesisErrorV21, LocalExplorerManifestV7, PortableGraphV7, R14_ERROR_VERSION,
    R14_ONTOLOGY_VERSION, R14_PORTABLE_GRAPH_VERSION, R14_QUERY_VERSION, R14_SNAPSHOT_VERSION,
    R14ContractError,
};
use codenoesis_domain::s4_r14::{ExpressionBindingError, ExpressionBindingLimit};
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
fn conf_fr_exp_006_v7_reimports_one_canonical_closed_projection() {
    let bytes = canonical_file(&valid_portable_with_references());
    let portable = PortableGraphV7::from_canonical_file(&bytes, sha256)
        .expect("reimport canonical R14 portable graph");
    assert_eq!(portable.canonical_file(), bytes);
}

#[test]
fn ft_fr_exp_006_v7_rejects_unknown_schema_fields_and_noncanonical_bytes() {
    let mut schema = valid_portable_shell();
    schema["schema_version"] = Value::String("codenoesis.portable-graph/v6".to_owned());
    assert_eq!(
        PortableGraphV7::from_canonical_file(&canonical_file(&schema), sha256).unwrap_err(),
        R14ContractError::UnsupportedPortableGraphSchema("codenoesis.portable-graph/v6".to_owned())
    );

    let mut unknown = valid_portable_shell();
    unknown["unknown"] = Value::Bool(true);
    assert_eq!(
        PortableGraphV7::from_canonical_file(&canonical_file(&unknown), sha256).unwrap_err(),
        R14ContractError::InvalidProjection
    );

    let value = valid_portable_shell();
    let mut noncanonical = serde_json::to_string_pretty(&value)
        .expect("serialize noncanonical R14 portable graph")
        .into_bytes();
    noncanonical.push(b'\n');
    assert!(matches!(
        PortableGraphV7::from_canonical_file(&noncanonical, sha256),
        Err(R14ContractError::Noncanonical { .. })
    ));
}

#[test]
fn ft_nfr_prv_002_v7_rejects_private_fields_urls_and_unsafe_paths() {
    for field in ["expression_text", "literal_lexeme", "repository_root"] {
        let mut private = valid_portable_shell();
        private["repository"][field] = Value::String("secret".to_owned());
        refresh_family_digests(&mut private);
        assert_eq!(
            PortableGraphV7::from_canonical_file(&canonical_file(&private), sha256).unwrap_err(),
            R14ContractError::UnsafePayload {
                reason: "private_field"
            }
        );
    }

    let mut raw_url = valid_portable_shell();
    raw_url["repository"]["name"] = Value::String("https://credential.invalid".to_owned());
    refresh_family_digests(&mut raw_url);
    assert_eq!(
        PortableGraphV7::from_canonical_file(&canonical_file(&raw_url), sha256).unwrap_err(),
        R14ContractError::UnsafePayload { reason: "raw_url" }
    );

    let mut unsafe_path = valid_portable_with_references();
    unsafe_path["evidence"][0]["path"] = Value::String("../secret.rs".to_owned());
    refresh_family_digests(&mut unsafe_path);
    assert_eq!(
        PortableGraphV7::from_canonical_file(&canonical_file(&unsafe_path), sha256).unwrap_err(),
        R14ContractError::UnsafePayload {
            reason: "unsafe_evidence_path"
        }
    );
}

#[test]
fn ft_fr_exp_006_v7_rejects_duplicate_dangling_and_hash_mismatches() {
    let mut duplicate = valid_portable_with_references();
    let duplicate_entity = duplicate["entities"][0].clone();
    duplicate["entities"]
        .as_array_mut()
        .expect("R14 entity family")
        .push(duplicate_entity);
    refresh_family_digests(&mut duplicate);
    assert!(matches!(
        PortableGraphV7::from_canonical_file(&canonical_file(&duplicate), sha256),
        Err(R14ContractError::IdentityConflict {
            family: "entities",
            ..
        })
    ));

    let mut dangling = valid_portable_with_references();
    dangling["relationships"][0]["target"] = Value::String("entity:missing".to_owned());
    refresh_family_digests(&mut dangling);
    assert_eq!(
        PortableGraphV7::from_canonical_file(&canonical_file(&dangling), sha256).unwrap_err(),
        R14ContractError::ReferenceMismatch {
            family: "relationships",
            id: "entity:missing".to_owned()
        }
    );

    let mut hash = valid_portable_shell();
    hash["projection"]["family_sha256"]["entities"] = Value::String("0".repeat(64));
    assert_eq!(
        PortableGraphV7::from_canonical_file(&canonical_file(&hash), sha256).unwrap_err(),
        R14ContractError::InvalidProjection
    );
}

#[test]
fn sec_fr_exp_006_v7_explorer_rejects_changed_unbalanced_or_active_viewer() {
    let portable =
        PortableGraphV7::from_canonical_file(&canonical_file(&valid_portable_shell()), sha256)
            .expect("construct R14 portable shell");
    let viewer = b"<script>const value = '<\\/script>';</script>\n";
    let viewer_sha256 = sha256(viewer);
    let unbalanced_viewer = b"<script>const value = '</script>';</script>\n";
    let unbalanced_viewer_sha256 = sha256(unbalanced_viewer);
    LocalExplorerManifestV7::new(
        &portable,
        viewer,
        &viewer_sha256,
        "default-src 'none'; script-src 'sha256-reviewed'",
        sha256,
    )
    .expect("accept balanced reviewed viewer");
    for (candidate, digest, csp) in [
        (
            b"changed".as_slice(),
            viewer_sha256.as_str(),
            "default-src 'none'",
        ),
        (
            unbalanced_viewer.as_slice(),
            unbalanced_viewer_sha256.as_str(),
            "default-src 'none'",
        ),
        (
            viewer.as_slice(),
            viewer_sha256.as_str(),
            "default-src https:",
        ),
    ] {
        assert_eq!(
            LocalExplorerManifestV7::new(&portable, candidate, digest, csp, sha256).unwrap_err(),
            R14ContractError::AssetIntegrityMismatch
        );
    }
}

#[test]
fn conf_fr_cli_001_r14_typed_failures_emit_only_error_v21_codes() {
    let errors = [
        CodeNoesisErrorV21::invalid_profile("rust-expression-bindings-v2"),
        CodeNoesisErrorV21::unsupported_composition("boundary_profile"),
        CodeNoesisErrorV21::from_expression(&ExpressionBindingError::ParentInvalid),
        CodeNoesisErrorV21::from_expression(&ExpressionBindingError::OperatorInvalid),
        CodeNoesisErrorV21::from_expression(&ExpressionBindingError::RoleInvalid),
        CodeNoesisErrorV21::from_expression(&ExpressionBindingError::ArgumentOrdinalInvalid),
        CodeNoesisErrorV21::from_expression(&ExpressionBindingError::BindingScopeInvalid),
        CodeNoesisErrorV21::from_expression(&ExpressionBindingError::AccessResolutionInvalid),
        CodeNoesisErrorV21::from_expression(&ExpressionBindingError::CallSiteEvidenceMismatch),
        CodeNoesisErrorV21::from_expression(&ExpressionBindingError::IndexMismatch),
        CodeNoesisErrorV21::from_expression(&ExpressionBindingError::LimitExceeded {
            limit: ExpressionBindingLimit::ExpressionDepth,
            maximum: 256,
            observed: 257,
        }),
        CodeNoesisErrorV21::invalid_snapshot(),
        CodeNoesisErrorV21::invalid_query(),
        CodeNoesisErrorV21::from_contract(&R14ContractError::InvalidProjection),
        CodeNoesisErrorV21::from_explorer_contract(&R14ContractError::AssetIntegrityMismatch),
    ];
    let allowed = allowed_error_codes();
    for error in errors {
        assert_eq!(error.value()["schema_version"], R14_ERROR_VERSION);
        assert_eq!(error.value()["retryable"], false);
        let code = error.value()["code"].as_str().expect("R14 error code");
        assert!(allowed.contains(code), "unexpected ErrorV21 code {code}");
        assert!(
            error
                .canonical_stderr()
                .expect("serialize ErrorV21")
                .ends_with(b"\n")
        );
    }
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
        "ontology_version": R14_ONTOLOGY_VERSION,
        "projection": {
            "profile": "codenoesis.lossless-portable-projection/v7",
            "family_sha256": {}
        },
        "query_contract_version": R14_QUERY_VERSION,
        "relationships": [],
        "repository": {
            "identity": "urn:codenoesis:test:r14-portable"
        },
        "schema_version": R14_PORTABLE_GRAPH_VERSION,
        "source_snapshot": {
            "schema_version": R14_SNAPSHOT_VERSION,
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
            "kind": "rust.expression",
            "evidence_id": "evidence:a"
        },
        {
            "id": "entity:b",
            "kind": "rust.pattern_binding",
            "evidence_id": "evidence:b"
        }
    ]);
    value["relationships"] = json!([{
        "id": "relationship:a",
        "kind": "READS",
        "source": "entity:a",
        "target": "entity:b",
        "evidence_ids": ["evidence:a"]
    }]);
    value["claims"] = json!([{
        "id": "claim:a",
        "subject_kind": "relationship",
        "subject_id": "relationship:a",
        "state": "deterministic_fact",
        "evidence_ids": ["evidence:a"]
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
            "path": "src/lib.rs",
            "blob_oid": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "start_byte": 2,
            "end_byte": 3
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
        "evidence_ids": ["evidence:a"],
        "coverage_gap_ids": ["coverage:a"]
    }]);
    refresh_family_digests(&mut value);
    value
}

fn refresh_family_digests(value: &mut Value) {
    let mut digests = Map::new();
    for family in PORTABLE_FAMILIES {
        let bytes = serde_json::to_vec(&value[family]).expect("serialize R14 portable family");
        digests.insert(family.to_owned(), Value::String(sha256(&bytes)));
    }
    value["projection"]["family_sha256"] = Value::Object(digests);
}

fn canonical_file(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(value).expect("serialize R14 portable graph");
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
        "input.invalid_rust_expression_profile",
        "input.unsupported_rust_expression_composition",
        "extraction.expression_parent_invalid",
        "extraction.expression_operator_invalid",
        "extraction.expression_role_invalid",
        "extraction.argument_ordinal_invalid",
        "extraction.binding_scope_invalid",
        "extraction.access_resolution_invalid",
        "extraction.call_site_evidence_mismatch",
        "extraction.expression_index_mismatch",
        "extraction.expression_limit_exceeded",
        "snapshot.invalid_v16",
        "query.invalid_v16",
        "export.invalid_portable_graph_v7",
        "explorer.asset_integrity_mismatch",
    ])
}
