use std::collections::BTreeSet;

use codenoesis_contracts::{
    CodeNoesisErrorV9, CodeNoesisErrorV19, PortableGraphV5, R12_ERROR_VERSION,
    R12_ONTOLOGY_VERSION, R12_PORTABLE_GRAPH_VERSION, R12_QUERY_VERSION, R12_SNAPSHOT_VERSION,
    R12ContractError,
};
use codenoesis_domain::s4_k1::{CallableSemanticsError, CallableSemanticsLimit};
use codenoesis_domain::s4_r10::RustCfgDeclarationAlternativesError;
use codenoesis_domain::s4_r12::CallableCfgAlternativesError;
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
fn ft_fr_exp_004_v5_reimport_rejects_unknown_schema_before_projection() {
    let mut value = valid_portable_shell();
    value["schema_version"] = Value::String("codenoesis.portable-graph/v4".to_owned());
    let bytes = canonical_file(&value);
    assert_eq!(
        PortableGraphV5::from_canonical_file(&bytes, sha256).unwrap_err(),
        R12ContractError::UnsupportedPortableGraphSchema("codenoesis.portable-graph/v4".to_owned())
    );
}

#[test]
fn conf_fr_exp_004_v5_reimports_one_canonical_closed_projection() {
    let value = valid_portable_shell();
    let bytes = canonical_file(&value);
    let portable = PortableGraphV5::from_canonical_file(&bytes, sha256)
        .expect("reimport canonical R12 portable shell");
    assert_eq!(portable.canonical_file(), bytes);
}

#[test]
fn conf_v5_document_statements_accept_validated_boundary_references() {
    let mut value = valid_portable_shell();
    value["repository"]["identity"] = Value::String("urn:codenoesis:pilot:rustdesk:r12".to_owned());
    value["repository_boundaries"] = rustdesk_boundary_projection();
    value["documents"] = json!([{
        "document_id": "document:a",
        "subject_id": "urn:codenoesis:pilot:rustdesk:r12"
    }]);
    value["document_statements"] = json!([{
        "statement_id": "statement:a",
        "document_id": "document:a",
        "subject_ids": [
            "urn:codenoesis:repository-boundary:sha256:4cc5105a068b6a39074ac1eea8e7f26968deac21f10aee1eb7132634e385341d",
            "urn:codenoesis:gitmodules-declaration:sha256:e379c323b66a3d9123744f33056ee176b6fdda79058d79edc5c1270a17b8e034",
            "urn:codenoesis:boundary-evidence:sha256:888b5e93f8b93ee7abfc38fe46bda260bdf19aeff46fe2146360627e454caa5d",
            "urn:codenoesis:boundary-gap:sha256:4db27bc5fadf0c9b73a2417c4107d3c3fce0caaa5d96b8cca7535b9c4d542c0b"
        ],
        "evidence_ids": [
            "urn:codenoesis:boundary-evidence:sha256:888b5e93f8b93ee7abfc38fe46bda260bdf19aeff46fe2146360627e454caa5d"
        ],
        "coverage_gap_ids": [
            "urn:codenoesis:boundary-gap:sha256:4db27bc5fadf0c9b73a2417c4107d3c3fce0caaa5d96b8cca7535b9c4d542c0b"
        ]
    }]);
    refresh_family_digests(&mut value);

    PortableGraphV5::from_canonical_file(&canonical_file(&value), sha256)
        .expect("accept validated repository-boundary document references");
}

#[test]
fn ft_nfr_prv_002_v5_reimport_rejects_private_payloads() {
    let mut value = valid_portable_shell();
    value["repository"]["raw_url"] = Value::String("https://credential.invalid".to_owned());
    refresh_family_digests(&mut value);
    let bytes = canonical_file(&value);
    assert_eq!(
        PortableGraphV5::from_canonical_file(&bytes, sha256).unwrap_err(),
        R12ContractError::UnsafePayload {
            reason: "private_field"
        }
    );
}

#[test]
fn ft_fr_exp_004_v5_reimport_rejects_unknown_and_noncanonical_payloads() {
    let mut unknown = valid_portable_shell();
    unknown["unknown"] = Value::Bool(true);
    assert_eq!(
        PortableGraphV5::from_canonical_file(&canonical_file(&unknown), sha256).unwrap_err(),
        R12ContractError::InvalidProjection
    );

    let value = valid_portable_shell();
    let mut noncanonical = serde_json::to_string_pretty(&value)
        .expect("serialize noncanonical R12 portable")
        .into_bytes();
    noncanonical.push(b'\n');
    assert!(matches!(
        PortableGraphV5::from_canonical_file(&noncanonical, sha256),
        Err(R12ContractError::Noncanonical { .. })
    ));
}

#[test]
fn conf_fr_cli_001_r12_typed_failures_emit_only_error_v19_codes() {
    let errors = [
        CodeNoesisErrorV19::invalid_profile("rust_semantic_profile", "wrong"),
        CodeNoesisErrorV19::invalid_profile("rust_callable_profile", "wrong"),
        CodeNoesisErrorV19::unsupported_composition("missing_profile"),
        CodeNoesisErrorV19::from_boundary_error(&CodeNoesisErrorV9::invalid_profile()),
        CodeNoesisErrorV19::from_extraction(
            &CallableCfgAlternativesError::LogicalMethodHasOccurrenceShape,
        ),
        CodeNoesisErrorV19::from_extraction(
            &CallableCfgAlternativesError::AlternativeSubjectMismatch {
                alternative_id: "alternative".to_owned(),
                observed_subject_id: "observed".to_owned(),
            },
        ),
        CodeNoesisErrorV19::from_extraction(
            &CallableCfgAlternativesError::AlternativeSignatureCardinality {
                alternative_id: "alternative".to_owned(),
                observed: 0,
            },
        ),
        CodeNoesisErrorV19::from_extraction(
            &CallableCfgAlternativesError::AlternativeSignatureCardinality {
                alternative_id: "alternative".to_owned(),
                observed: 2,
            },
        ),
        CodeNoesisErrorV19::from_extraction(
            &CallableCfgAlternativesError::OccurrenceEvidenceMismatch {
                alternative_id: "alternative".to_owned(),
            },
        ),
        CodeNoesisErrorV19::from_extraction(&CallableCfgAlternativesError::Alternatives(
            RustCfgDeclarationAlternativesError::Duplicate {
                logical_method_id: "logical".to_owned(),
                declaration_evidence_id: "evidence".to_owned(),
            },
        )),
        CodeNoesisErrorV19::from_extraction(&CallableCfgAlternativesError::Callable(
            CallableSemanticsError::LimitExceeded {
                limit: CallableSemanticsLimit::ParametersPerCallable,
                maximum: 256,
                observed: 257,
            },
        )),
        CodeNoesisErrorV19::invalid_snapshot(),
        CodeNoesisErrorV19::invalid_query("snapshot_invalid"),
        CodeNoesisErrorV19::from_contract(&R12ContractError::InvalidSnapshot, false),
        CodeNoesisErrorV19::from_contract(
            &R12ContractError::LimitExceeded {
                limit: "portable_graph_bytes",
                maximum: 8,
                observed: 9,
            },
            false,
        ),
        CodeNoesisErrorV19::from_contract(&R12ContractError::AssetIntegrityMismatch, true),
        CodeNoesisErrorV19::internal("test"),
    ];
    let allowed = allowed_error_codes();
    for error in errors {
        assert_eq!(error.value()["schema_version"], R12_ERROR_VERSION);
        assert_eq!(error.value()["retryable"], false);
        let code = error.value()["code"].as_str().expect("R12 error code");
        assert!(allowed.contains(code), "unexpected ErrorV19 code {code}");
        assert!(
            error
                .canonical_stderr()
                .expect("serialize ErrorV19")
                .ends_with(b"\n")
        );
    }
}

#[test]
fn ft_fr_exp_004_v5_reimport_rejects_duplicate_and_dangling_graph_records() {
    let mut duplicate = valid_portable_with_references();
    let entity = duplicate["entities"][0].clone();
    duplicate["entities"]
        .as_array_mut()
        .expect("R12 entity family")
        .push(entity);
    refresh_family_digests(&mut duplicate);
    assert_eq!(
        PortableGraphV5::from_canonical_file(&canonical_file(&duplicate), sha256).unwrap_err(),
        R12ContractError::IdentityConflict {
            family: "entities",
            id: "entity:a".to_owned()
        }
    );

    let mut dangling = valid_portable_with_references();
    dangling["relationships"] = json!([{
        "id": "relationship:a",
        "kind": "HAS_SIGNATURE",
        "source": "entity:missing",
        "target": "entity:a",
        "evidence_ids": ["evidence:a"]
    }]);
    refresh_family_digests(&mut dangling);
    assert_eq!(
        PortableGraphV5::from_canonical_file(&canonical_file(&dangling), sha256).unwrap_err(),
        R12ContractError::ReferenceMismatch {
            family: "relationships",
            id: "entity:missing".to_owned()
        }
    );
}

#[test]
fn ft_fr_exp_004_v5_reimport_rejects_nested_entity_reference_mismatches() {
    for pointer in [
        "/entities/0/subject_id",
        "/entities/0/properties/parent_fact_id",
        "/entities/0/properties/resolved_target_id",
        "/entities/0/properties/declaration_alternative_ids/0",
    ] {
        let mut value = valid_portable_with_references();
        *value
            .pointer_mut(pointer)
            .expect("reviewed R12 entity reference") = Value::String("entity:missing".to_owned());
        refresh_family_digests(&mut value);
        assert_eq!(
            PortableGraphV5::from_canonical_file(&canonical_file(&value), sha256).unwrap_err(),
            R12ContractError::ReferenceMismatch {
                family: "entities",
                id: "entity:missing".to_owned()
            },
            "pointer {pointer}"
        );
    }

    for pointer in [
        "/entities/0/properties/body_evidence_id",
        "/entities/0/properties/declaration_evidence_id",
        "/entities/0/properties/evidence_id",
        "/entities/0/properties/attributes/0/evidence_id",
    ] {
        let mut value = valid_portable_with_references();
        *value
            .pointer_mut(pointer)
            .expect("reviewed R12 evidence reference") =
            Value::String("evidence:missing".to_owned());
        refresh_family_digests(&mut value);
        assert_eq!(
            PortableGraphV5::from_canonical_file(&canonical_file(&value), sha256).unwrap_err(),
            R12ContractError::ReferenceMismatch {
                family: "entities",
                id: "evidence:missing".to_owned()
            },
            "pointer {pointer}"
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
        "ontology_version": R12_ONTOLOGY_VERSION,
        "projection": {
            "profile": "codenoesis.lossless-portable-projection/v5",
            "family_sha256": {}
        },
        "query_contract_version": R12_QUERY_VERSION,
        "relationships": [],
        "repository": {
            "identity": "urn:codenoesis:test:r12-portable"
        },
        "repository_boundaries": null,
        "schema_version": R12_PORTABLE_GRAPH_VERSION,
        "source_snapshot": {
            "schema_version": R12_SNAPSHOT_VERSION,
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
    value["entities"] = json!([{
        "id": "entity:a",
        "kind": "rust.callable_signature",
        "crate_id": "entity:a",
        "module_path": "crate",
        "name": "run",
        "subject_id": "entity:a",
        "ordinal": null,
        "evidence_ids": ["evidence:a"],
        "properties": {
            "body_evidence_id": "evidence:a",
            "declaration_evidence_id": "evidence:a",
            "evidence_id": "evidence:a",
            "parent_fact_id": "entity:a",
            "resolved_target_id": "entity:a",
            "declaration_alternative_ids": ["entity:a"],
            "attributes": [{
                "kind": "cfg",
                "token_text": "#[cfg(test)]",
                "evidence_id": "evidence:a"
            }]
        }
    }]);
    value["evidence"] = json!([{
        "id": "evidence:a",
        "path": "src/lib.rs",
        "blob_oid": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "start_byte": 0,
        "end_byte": 1
    }]);
    refresh_family_digests(&mut value);
    value
}

fn rustdesk_boundary_projection() -> Value {
    json!({
        "schema_version": "codenoesis.repository-boundaries/v1",
        "profile": "local-gitlinks-v1",
        "root_repository": {
            "identity": "urn:codenoesis:pilot:rustdesk:r12",
            "vcs": "git",
            "object_format": "sha1",
            "commit_oid": "d412d198720aa56f6cfed2dfad262e8fb1322fb7",
            "tree_oid": "df8d4c292c9d256a445480eb878e507df3de1dc4"
        },
        "summary": {
            "boundary_count": 1,
            "declaration_count": 1,
            "coverage_gap_count": 1,
            "bound_count": 0,
            "unbound_count": 1
        },
        "boundaries": [{
            "boundary_id": "urn:codenoesis:repository-boundary:sha256:4cc5105a068b6a39074ac1eea8e7f26968deac21f10aee1eb7132634e385341d",
            "path": "libs/hbb_common",
            "gitlink_oid": "69cea8dafee147848ae88702029f4bf7df7224c3",
            "state": "declared_unbound",
            "declaration_id": "urn:codenoesis:gitmodules-declaration:sha256:e379c323b66a3d9123744f33056ee176b6fdda79058d79edc5c1270a17b8e034",
            "nested_repository": null,
            "evidence_ids": [
                "urn:codenoesis:boundary-evidence:sha256:888b5e93f8b93ee7abfc38fe46bda260bdf19aeff46fe2146360627e454caa5d",
                "urn:codenoesis:boundary-evidence:sha256:ad564882d85c15bcd1b13f62d74208268d2b636a98e5202a2141191faa66a863"
            ],
            "coverage_gap_ids": [
                "urn:codenoesis:boundary-gap:sha256:4db27bc5fadf0c9b73a2417c4107d3c3fce0caaa5d96b8cca7535b9c4d542c0b"
            ]
        }],
        "declarations": [{
            "declaration_id": "urn:codenoesis:gitmodules-declaration:sha256:e379c323b66a3d9123744f33056ee176b6fdda79058d79edc5c1270a17b8e034",
            "name_sha256": "d6fa95f1476bac33248cc3ba17950ae514fe1e3bd559d22457480d7fab9eec1d",
            "path": "libs/hbb_common",
            "url_kind": "https",
            "url_sha256": "a368509517a1eb9faf1a7ab9d26e57b27dc16d6d8e910aecd97ef29f7051c9b5",
            "unsupported_keys": [],
            "boundary_id": "urn:codenoesis:repository-boundary:sha256:4cc5105a068b6a39074ac1eea8e7f26968deac21f10aee1eb7132634e385341d",
            "evidence_id": "urn:codenoesis:boundary-evidence:sha256:ad564882d85c15bcd1b13f62d74208268d2b636a98e5202a2141191faa66a863"
        }],
        "coverage_gaps": [{
            "gap_id": "urn:codenoesis:boundary-gap:sha256:4db27bc5fadf0c9b73a2417c4107d3c3fce0caaa5d96b8cca7535b9c4d542c0b",
            "code": "boundary.nested_repository_unbound",
            "subject_id": "urn:codenoesis:repository-boundary:sha256:4cc5105a068b6a39074ac1eea8e7f26968deac21f10aee1eb7132634e385341d",
            "path": "libs/hbb_common",
            "evidence_ids": [
                "urn:codenoesis:boundary-evidence:sha256:888b5e93f8b93ee7abfc38fe46bda260bdf19aeff46fe2146360627e454caa5d",
                "urn:codenoesis:boundary-evidence:sha256:ad564882d85c15bcd1b13f62d74208268d2b636a98e5202a2141191faa66a863"
            ]
        }],
        "evidence": [{
            "evidence_id": "urn:codenoesis:boundary-evidence:sha256:888b5e93f8b93ee7abfc38fe46bda260bdf19aeff46fe2146360627e454caa5d",
            "kind": "git_tree_entry",
            "repository": {
                "identity": "urn:codenoesis:pilot:rustdesk:r12",
                "commit_oid": "d412d198720aa56f6cfed2dfad262e8fb1322fb7"
            },
            "tree_oid": "c7041390104b9d59276e1e5c3388f2d281eb91b4",
            "path": "libs/hbb_common",
            "mode": "160000",
            "object_oid": "69cea8dafee147848ae88702029f4bf7df7224c3"
        }, {
            "evidence_id": "urn:codenoesis:boundary-evidence:sha256:ad564882d85c15bcd1b13f62d74208268d2b636a98e5202a2141191faa66a863",
            "kind": "gitmodules_declaration",
            "repository": {
                "identity": "urn:codenoesis:pilot:rustdesk:r12",
                "commit_oid": "d412d198720aa56f6cfed2dfad262e8fb1322fb7"
            },
            "path": ".gitmodules",
            "blob_oid": "d80e69aa84a4c7f764eea191622a386878cf852d",
            "span": {"unit": "byte", "start": 0, "end": 100}
        }]
    })
}

fn refresh_family_digests(value: &mut Value) {
    let mut digests = Map::new();
    for family in PORTABLE_FAMILIES {
        let bytes = serde_json::to_vec(&value[family]).expect("serialize R12 portable family");
        digests.insert(family.to_owned(), Value::String(sha256(&bytes)));
    }
    value["projection"]["family_sha256"] = Value::Object(digests);
}

fn canonical_file(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(value).expect("serialize R12 portable test value");
    bytes.push(b'\n');
    bytes
}

fn sha256(bytes: &[u8]) -> String {
    let mut digest = [0_u8; 32];
    for (index, byte) in bytes.iter().copied().enumerate() {
        digest[index % digest.len()] ^= byte;
    }
    digest
        .iter()
        .fold(String::with_capacity(64), |mut output, byte| {
            use std::fmt::Write as _;
            write!(&mut output, "{byte:02x}").expect("write SHA-256 test hex");
            output
        })
}

fn allowed_error_codes() -> BTreeSet<&'static str> {
    BTreeSet::from([
        "input.invalid_rust_cfg_alternatives_profile",
        "input.invalid_rust_callable_profile",
        "input.unsupported_rust_callable_cfg_alternatives_composition",
        "input.invalid_repository_boundary_profile",
        "extraction.rust_cfg_alternative_duplicate",
        "extraction.callable_cfg_logical_shape_forbidden",
        "extraction.callable_cfg_alternative_subject_mismatch",
        "extraction.callable_cfg_alternative_signature_missing",
        "extraction.callable_cfg_alternative_signature_duplicate",
        "extraction.callable_cfg_evidence_invalid",
        "extraction.callable_limit_exceeded",
        "snapshot.invalid_v14",
        "query.invalid_v14",
        "export.invalid_snapshot",
        "export.limit_exceeded",
        "explorer.asset_integrity_mismatch",
        "internal.unexpected",
    ])
}
