use codenoesis_contracts::{CodeNoesisErrorV17, R10ContractError};
use codenoesis_domain::s4_r5::{RustSemanticError, RustSemanticLimit};
use codenoesis_domain::s4_r10::{
    RustCfgDeclarationAlternativesError, RustCfgDeclarationAlternativesLimit,
};

#[test]
fn conf_fr_ext_013_error_v17_extraction_matrix_is_typed() {
    let cases = [
        (
            RustCfgDeclarationAlternativesError::IdentityMismatch {
                logical_method_id: "logical".to_owned(),
                reason: "logical_properties",
            },
            "extraction.rust_cfg_alternative_identity_mismatch",
        ),
        (
            RustCfgDeclarationAlternativesError::Duplicate {
                logical_method_id: "logical".to_owned(),
                declaration_evidence_id: "evidence".to_owned(),
            },
            "extraction.rust_cfg_alternative_duplicate",
        ),
        (
            RustCfgDeclarationAlternativesError::Overlap {
                logical_method_id: "logical".to_owned(),
                first_evidence_id: "first".to_owned(),
                second_evidence_id: "second".to_owned(),
            },
            "extraction.rust_cfg_alternative_overlap",
        ),
        (
            RustCfgDeclarationAlternativesError::CrossSource {
                logical_method_id: "logical".to_owned(),
            },
            "extraction.rust_cfg_alternative_cross_source",
        ),
        (
            RustCfgDeclarationAlternativesError::LimitExceeded {
                limit: RustCfgDeclarationAlternativesLimit::AlternativesPerLogicalMethod,
                maximum: 32,
                observed: 33,
            },
            "extraction.rust_cfg_alternative_limit_exceeded",
        ),
        (
            RustCfgDeclarationAlternativesError::Source(RustSemanticError::InvalidDeclaration {
                path: "src/lib.rs".to_owned(),
                start_byte: 7,
                declaration_kind: "ERROR".to_owned(),
            }),
            "extraction.invalid_rust_source",
        ),
        (
            RustCfgDeclarationAlternativesError::Source(RustSemanticError::LimitExceeded {
                limit: RustSemanticLimit::DeclaredTypeOrHeaderBytes,
                maximum: 4_096,
                observed: 4_097,
            }),
            "extraction.rust_cfg_alternative_limit_exceeded",
        ),
    ];
    for (error, expected_code) in cases {
        let boundary = CodeNoesisErrorV17::from_extraction(&error);
        assert_eq!(boundary.value()["schema_version"], "codenoesis.error/v17");
        assert_eq!(boundary.value()["code"], expected_code);
        assert_eq!(boundary.value()["retryable"], false);
        assert_eq!(
            boundary
                .canonical_stderr()
                .expect("serialize ErrorV17")
                .last(),
            Some(&b'\n')
        );
    }
}

#[test]
fn conf_fr_exp_003_error_v17_contract_matrix_is_typed() {
    for (error, explorer, expected_code) in [
        (
            R10ContractError::InvalidSnapshot,
            false,
            "export.invalid_snapshot",
        ),
        (
            R10ContractError::InvalidProjection,
            false,
            "export.invalid_portable_graph_v3",
        ),
        (
            R10ContractError::AssetIntegrityMismatch,
            true,
            "explorer.asset_integrity_mismatch",
        ),
        (
            R10ContractError::LimitExceeded {
                limit: "portable_graph_bytes",
                maximum: 10,
                observed: 11,
            },
            false,
            "export.limit_exceeded",
        ),
        (
            R10ContractError::LimitExceeded {
                limit: "portable_graph_bytes",
                maximum: 10,
                observed: 11,
            },
            true,
            "explorer.limit_exceeded",
        ),
    ] {
        let boundary = CodeNoesisErrorV17::from_contract(&error, explorer);
        assert_eq!(boundary.value()["schema_version"], "codenoesis.error/v17");
        assert_eq!(boundary.value()["code"], expected_code);
    }

    for (boundary, expected_code, expected_stage) in [
        (
            CodeNoesisErrorV17::invalid_snapshot(),
            "snapshot.invalid_v12",
            "snapshot",
        ),
        (
            CodeNoesisErrorV17::invalid_query(),
            "query.invalid_v12",
            "query",
        ),
    ] {
        assert_eq!(boundary.value()["code"], expected_code);
        assert_eq!(boundary.value()["stage"], expected_stage);
    }
}
