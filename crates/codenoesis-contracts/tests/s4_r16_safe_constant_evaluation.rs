use std::collections::BTreeSet;

use codenoesis_contracts::{
    CodeNoesisErrorV24, LocalExplorerManifestV9, PortableGraphV9, R16_ERROR_VERSION,
    R16_INDEX_VERSION, R16_ONTOLOGY_VERSION, R16_PORTABLE_GRAPH_VERSION, R16_QUERY_VERSION,
    R16_RULE_VERSION, R16_SNAPSHOT_VERSION, R16ContractError,
};
use codenoesis_domain::knowledge::{ClaimState, ClaimSubjectKind};
use codenoesis_domain::s4::workspace_claim_id;
use codenoesis_domain::s4_r12::CallableCfgAlternativesError;
use codenoesis_domain::s4_r14::ExpressionBindingError;
use codenoesis_domain::s4_r15::LocalFlowError;
use codenoesis_domain::s4_r16::{
    ConstantEvaluationError, ConstantEvaluationLimit, evaluated_value_id,
    evaluation_relationship_id,
};
use serde_json::{Map, Value, json};

const REPOSITORY_ID: &str = "urn:codenoesis:test:r16-portable";
const SEMANTIC_ID: &str = "entity:constant";
const DECLARED_ID: &str = "entity:declared";
const EVIDENCE_ID: &str = "evidence:constant";
const PORTABLE_FAMILIES: [&str; 10] = [
    "entities",
    "relationships",
    "claims",
    "evidence",
    "diagnostics",
    "coverage_gaps",
    "documents",
    "document_statements",
    "local_flow_index",
    "constant_evaluation_index",
];

#[test]
fn conf_fr_exp_008_v9_reimports_canonical_lossless_constant_projection() {
    let value = valid_portable();
    let bytes = canonical_file(&value);
    let portable = PortableGraphV9::from_canonical_file(&bytes, sha256)
        .expect("reimport canonical R16 portable graph");
    assert_eq!(portable.canonical_file(), bytes);
    assert_eq!(
        portable.value()["constant_evaluation_index"]["derivations"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
}

#[test]
fn ft_fr_exp_008_v9_rejects_unknown_order_derivation_privacy_and_path_failures() {
    let mut unknown = valid_portable();
    unknown["unknown"] = Value::Bool(true);
    assert_eq!(
        reimport_error(&unknown),
        R16ContractError::InvalidProjection
    );

    let mut order = valid_portable();
    order["entities"]
        .as_array_mut()
        .expect("R16 entity family")
        .swap(0, 1);
    refresh_family_digests(&mut order);
    assert!(matches!(
        reimport_error(&order),
        R16ContractError::IdentityConflict(_)
    ));

    let mut missing_derivation = valid_portable();
    missing_derivation["constant_evaluation_index"]["derivations"] = json!([]);
    refresh_family_digests(&mut missing_derivation);
    assert_eq!(
        reimport_error(&missing_derivation),
        R16ContractError::InvalidProjection
    );

    let mut raw_expression = valid_portable();
    evaluated_entity_mut(&mut raw_expression)["initializer_text"] =
        Value::String("1 + secret".to_owned());
    refresh_family_digests(&mut raw_expression);
    assert_eq!(
        reimport_error(&raw_expression),
        R16ContractError::UnsafePayload("private_field")
    );

    let mut absolute_path = valid_portable();
    absolute_path["evidence"][0]["path"] = Value::String("/private/source.rs".to_owned());
    refresh_family_digests(&mut absolute_path);
    assert_eq!(
        reimport_error(&absolute_path),
        R16ContractError::UnsafePayload("unsafe_evidence_path")
    );

    let mut noncanonical = serde_json::to_string_pretty(&valid_portable())
        .expect("serialize noncanonical R16 graph")
        .into_bytes();
    noncanonical.push(b'\n');
    assert!(matches!(
        PortableGraphV9::from_canonical_file(&noncanonical, sha256),
        Err(R16ContractError::Noncanonical { .. })
    ));
}

#[test]
fn ft_fr_exp_008_v9_rejects_value_derivation_index_and_cycle_mismatches() {
    for (case, mutate) in [
        (
            "noncanonical_value",
            mutate_noncanonical_value as fn(&mut Value),
        ),
        ("invalid_type_authority", mutate_type_authority),
        ("missing_input_claim", mutate_missing_input_claim),
        ("missing_input_evidence", mutate_missing_input_evidence),
        ("index_graph_disagreement", mutate_index_disagreement),
    ] {
        let mut value = valid_portable();
        mutate(&mut value);
        refresh_family_digests(&mut value);
        assert_eq!(
            reimport_error(&value),
            R16ContractError::InvalidProjection,
            "{case}"
        );
    }

    let mut dangling = valid_portable();
    dangling["constant_evaluation_index"]["derivations"][0]["dependency_entity_ids"] =
        json!(["entity:missing"]);
    refresh_family_digests(&mut dangling);
    assert_eq!(
        reimport_error(&dangling),
        R16ContractError::ReferenceMismatch("entity:missing".to_owned())
    );

    let evaluated_id = evaluated_value_id(REPOSITORY_ID, DECLARED_ID);
    let evaluated_claim_id = workspace_claim_id(
        ClaimSubjectKind::Entity,
        &evaluated_id,
        ClaimState::DerivedFact,
    );
    let declared_claim_id = workspace_claim_id(
        ClaimSubjectKind::Entity,
        DECLARED_ID,
        ClaimState::DeterministicFact,
    );
    let mut cycle = valid_portable();
    cycle["constant_evaluation_index"]["derivations"][0]["dependency_entity_ids"] =
        json!([evaluated_id]);
    let mut input_claims = vec![declared_claim_id, evaluated_claim_id];
    input_claims.sort();
    cycle["constant_evaluation_index"]["derivations"][0]["input_claim_ids"] = json!(input_claims);
    refresh_family_digests(&mut cycle);
    assert_eq!(reimport_error(&cycle), R16ContractError::InvalidProjection);
}

#[test]
fn sec_fr_exp_008_v9_explorer_rejects_remote_dynamic_and_script_close_content() {
    let portable = PortableGraphV9::from_canonical_file(&canonical_file(&valid_portable()), sha256)
        .expect("construct R16 portable graph");
    let viewer = b"<script>const value = 1;</script>\n";
    let viewer_sha256 = sha256(viewer);
    let manifest = LocalExplorerManifestV9::new(
        &portable,
        viewer,
        &viewer_sha256,
        "default-src 'none'; script-src 'sha256-reviewed'",
        sha256,
    )
    .expect("accept reviewed immutable R16 viewer");
    assert_eq!(
        manifest.value()["capabilities"]["constant_evaluation_derivations"],
        true
    );

    for candidate in [
        b"<script>fetch('https://example.invalid');</script>".as_slice(),
        b"<script>eval('1');</script>".as_slice(),
        b"<script>const value = '</script>';</script>".as_slice(),
    ] {
        assert_eq!(
            LocalExplorerManifestV9::new(
                &portable,
                candidate,
                &sha256(candidate),
                "default-src 'none'",
                sha256,
            )
            .unwrap_err(),
            R16ContractError::AssetIntegrityMismatch
        );
    }
}

#[test]
fn conf_fr_cli_001_r16_typed_failures_emit_only_error_v24_codes() {
    let errors = [
        CodeNoesisErrorV24::invalid_profile("rust-safe-constant-evaluation-v2"),
        CodeNoesisErrorV24::unsupported_composition("repository_boundary_not_supported"),
        CodeNoesisErrorV24::from_constant_evaluation(&ConstantEvaluationError::IdentityConflict),
        CodeNoesisErrorV24::from_constant_evaluation(&ConstantEvaluationError::Source(
            LocalFlowError::Source(ExpressionBindingError::CfgAlternatives(
                CallableCfgAlternativesError::ContractInvalid,
            )),
        )),
        CodeNoesisErrorV24::from_constant_evaluation(&ConstantEvaluationError::Source(
            LocalFlowError::Source(ExpressionBindingError::ContractInvalid),
        )),
        CodeNoesisErrorV24::from_constant_evaluation(&ConstantEvaluationError::ValueInvalid),
        CodeNoesisErrorV24::from_constant_evaluation(&ConstantEvaluationError::DependencyInvalid),
        CodeNoesisErrorV24::from_constant_evaluation(&ConstantEvaluationError::DependencyCycle),
        CodeNoesisErrorV24::from_constant_evaluation(&ConstantEvaluationError::DerivationMismatch),
        CodeNoesisErrorV24::from_constant_evaluation(&ConstantEvaluationError::IndexMismatch),
        CodeNoesisErrorV24::from_constant_evaluation(&ConstantEvaluationError::LimitExceeded {
            limit: ConstantEvaluationLimit::DependencyLevels,
            maximum: 64,
            observed: 65,
        }),
        CodeNoesisErrorV24::from_contract(&R16ContractError::InvalidProjection),
        CodeNoesisErrorV24::from_explorer_contract(&R16ContractError::AssetIntegrityMismatch),
        CodeNoesisErrorV24::invalid_snapshot(),
        CodeNoesisErrorV24::invalid_query(),
    ];
    let allowed = BTreeSet::from([
        "input.invalid_rust_constant_profile",
        "input.unsupported_rust_constant_evaluation_composition",
        "extraction.constant_identity_conflict",
        "extraction.callable_cfg_alternatives_contract_invalid",
        "extraction.expression_contract_invalid",
        "extraction.constant_value_invalid",
        "extraction.constant_dependency_invalid",
        "extraction.constant_dependency_cycle",
        "extraction.constant_derivation_mismatch",
        "extraction.constant_index_mismatch",
        "extraction.constant_limit_exceeded",
        "export.invalid_portable_graph_v9",
        "explorer.asset_integrity_mismatch",
        "store.invalid_snapshot_v18",
        "query.invalid_local_query_v13",
    ]);
    for error in errors {
        assert_eq!(error.value()["schema_version"], R16_ERROR_VERSION);
        assert_eq!(error.value()["retryable"], false);
        let code = error.value()["code"].as_str().expect("R16 error code");
        assert!(allowed.contains(code), "unexpected ErrorV24 code {code}");
        assert!(
            error
                .canonical_stderr()
                .expect("serialize ErrorV24")
                .ends_with(b"\n")
        );
    }
}

#[allow(clippy::too_many_lines)]
fn valid_portable() -> Value {
    let evaluated_id = evaluated_value_id(REPOSITORY_ID, DECLARED_ID);
    let relationship_id = evaluation_relationship_id(DECLARED_ID, &evaluated_id);
    let declared_claim_id = workspace_claim_id(
        ClaimSubjectKind::Entity,
        DECLARED_ID,
        ClaimState::DeterministicFact,
    );
    let evaluated_claim_id = workspace_claim_id(
        ClaimSubjectKind::Entity,
        &evaluated_id,
        ClaimState::DerivedFact,
    );
    let relationship_claim_id = workspace_claim_id(
        ClaimSubjectKind::Relationship,
        &relationship_id,
        ClaimState::DerivedFact,
    );
    let mut entities = vec![
        json!({"id": SEMANTIC_ID, "kind": "rust.constant"}),
        json!({
            "id": DECLARED_ID,
            "kind": "rust.declared_value",
            "subject_id": SEMANTIC_ID
        }),
        json!({
            "id": evaluated_id,
            "kind": "rust.evaluated_value",
            "declared_value_id": DECLARED_ID,
            "properties": {
                "canonical_value": "42",
                "rule_version": R16_RULE_VERSION,
                "rust_type": "i32",
                "type_authority": "explicit_primitive_annotation",
                "value_kind": "integer"
            }
        }),
    ];
    sort_by(&mut entities, "id");
    let mut claims = vec![
        json!({
            "id": declared_claim_id,
            "subject_kind": "entity",
            "subject_id": DECLARED_ID,
            "state": "deterministic_fact",
            "evidence_ids": [EVIDENCE_ID]
        }),
        json!({
            "id": evaluated_claim_id,
            "subject_kind": "entity",
            "subject_id": evaluated_id,
            "state": "derived_fact",
            "evidence_ids": [EVIDENCE_ID]
        }),
        json!({
            "id": relationship_claim_id,
            "subject_kind": "relationship",
            "subject_id": relationship_id,
            "state": "derived_fact",
            "evidence_ids": [EVIDENCE_ID]
        }),
    ];
    sort_by(&mut claims, "id");
    let mut value = json!({
        "claims": claims,
        "constant_evaluation_index": {
            "schema_version": R16_INDEX_VERSION,
            "rule_version": R16_RULE_VERSION,
            "evaluated_entity_ids": [evaluated_id],
            "evaluation_relationship_ids": [relationship_id],
            "derivations": [{
                "entity_id": evaluated_id,
                "relationship_id": relationship_id,
                "rule_version": R16_RULE_VERSION,
                "input_claim_ids": [declared_claim_id],
                "input_evidence_ids": [EVIDENCE_ID],
                "dependency_entity_ids": []
            }]
        },
        "coverage_gaps": [],
        "diagnostics": [],
        "document_statements": [],
        "documents": [],
        "entities": entities,
        "evidence": [{
            "id": EVIDENCE_ID,
            "path": "src/lib.rs",
            "blob_oid": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "start_byte": 0,
            "end_byte": 25
        }],
        "local_flow_index": {},
        "ontology_version": R16_ONTOLOGY_VERSION,
        "projection": {
            "profile": "codenoesis.lossless-portable-projection/v9",
            "family_sha256": {}
        },
        "query_contract_version": R16_QUERY_VERSION,
        "relationships": [{
            "id": relationship_id,
            "kind": "EVALUATES_TO",
            "source": DECLARED_ID,
            "target": evaluated_id,
            "evidence_ids": [EVIDENCE_ID]
        }],
        "repository": {"identity": REPOSITORY_ID},
        "schema_version": R16_PORTABLE_GRAPH_VERSION,
        "source_snapshot": {
            "schema_version": R16_SNAPSHOT_VERSION,
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

fn evaluated_entity_mut(value: &mut Value) -> &mut Value {
    value["entities"]
        .as_array_mut()
        .expect("R16 entity family")
        .iter_mut()
        .find(|entity| entity["kind"] == "rust.evaluated_value")
        .expect("R16 evaluated entity")
}

fn mutate_noncanonical_value(value: &mut Value) {
    evaluated_entity_mut(value)["properties"]["canonical_value"] = Value::String("01".to_owned());
}

fn mutate_type_authority(value: &mut Value) {
    evaluated_entity_mut(value)["properties"]["type_authority"] =
        Value::String("fixed_repr_attribute".to_owned());
}

fn mutate_missing_input_claim(value: &mut Value) {
    value["constant_evaluation_index"]["derivations"][0]["input_claim_ids"] = json!([]);
}

fn mutate_missing_input_evidence(value: &mut Value) {
    value["constant_evaluation_index"]["derivations"][0]["input_evidence_ids"] = json!([]);
}

fn mutate_index_disagreement(value: &mut Value) {
    value["constant_evaluation_index"]["evaluated_entity_ids"] = json!([]);
}

fn reimport_error(value: &Value) -> R16ContractError {
    PortableGraphV9::from_canonical_file(&canonical_file(value), sha256)
        .expect_err("reject invalid R16 portable graph")
}

fn sort_by(values: &mut [Value], field: &str) {
    values.sort_by(|left, right| {
        left[field]
            .as_str()
            .expect("R16 sortable left ID")
            .cmp(right[field].as_str().expect("R16 sortable right ID"))
    });
}

fn refresh_family_digests(value: &mut Value) {
    let mut digests = Map::new();
    for family in PORTABLE_FAMILIES {
        let bytes = serde_json::to_vec(&value[family]).expect("serialize R16 portable family");
        digests.insert(family.to_owned(), Value::String(sha256(&bytes)));
    }
    value["projection"]["family_sha256"] = Value::Object(digests);
}

fn canonical_file(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(value).expect("serialize R16 portable graph");
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
