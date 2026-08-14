use codenoesis_contracts::{
    FunctionContextCounts, FunctionContextError, MAX_R16_PORTABLE_GRAPH_BYTES,
    MAX_R17_CONTEXT_OUTPUT_BYTES, MAX_R17_FUNCTION_PARAMETERS, MAX_R17_FUNCTION_SEARCH_RESULTS,
    MAX_R17_LINKED_CLAIMS, MAX_R17_LINKED_EVIDENCE, MAX_R17_LINKED_RELATIONSHIPS,
    MAX_R17_LINKED_SUBJECTS, MAX_R17_NAVIGATION_HISTORY, MAX_R17_UNCERTAINTY_RECORDS,
    R17_CONTEXT_PROFILE, R17_EXPLORER_SECURITY_PROFILE, R17_FUNCTION_CONTEXT_VERSION,
    R17_LOCAL_EXPLORER_VERSION, validate_function_context_limits,
    validate_function_context_output_bytes,
};

#[test]
fn pt_fr_ctx_001_exact_maxima_are_accepted() {
    validate_function_context_limits(FunctionContextCounts {
        parameters: MAX_R17_FUNCTION_PARAMETERS,
        linked_subjects: MAX_R17_LINKED_SUBJECTS,
        linked_relationships: MAX_R17_LINKED_RELATIONSHIPS,
        linked_claims: MAX_R17_LINKED_CLAIMS,
        linked_evidence: MAX_R17_LINKED_EVIDENCE,
        uncertainty_records: MAX_R17_UNCERTAINTY_RECORDS,
    })
    .expect("every exact R17 maximum must remain accepted");
    validate_function_context_output_bytes(MAX_R17_CONTEXT_OUTPUT_BYTES)
        .expect("exact R17 output maximum must remain accepted");
}

#[test]
fn pt_fr_ctx_001_each_maximum_plus_one_fails_closed() {
    let cases = [
        (
            "function_parameters",
            FunctionContextCounts {
                parameters: MAX_R17_FUNCTION_PARAMETERS + 1,
                ..FunctionContextCounts::default()
            },
        ),
        (
            "linked_subjects",
            FunctionContextCounts {
                linked_subjects: MAX_R17_LINKED_SUBJECTS + 1,
                ..FunctionContextCounts::default()
            },
        ),
        (
            "linked_relationships",
            FunctionContextCounts {
                linked_relationships: MAX_R17_LINKED_RELATIONSHIPS + 1,
                ..FunctionContextCounts::default()
            },
        ),
        (
            "linked_claims",
            FunctionContextCounts {
                linked_claims: MAX_R17_LINKED_CLAIMS + 1,
                ..FunctionContextCounts::default()
            },
        ),
        (
            "linked_evidence",
            FunctionContextCounts {
                linked_evidence: MAX_R17_LINKED_EVIDENCE + 1,
                ..FunctionContextCounts::default()
            },
        ),
        (
            "uncertainty_records",
            FunctionContextCounts {
                uncertainty_records: MAX_R17_UNCERTAINTY_RECORDS + 1,
                ..FunctionContextCounts::default()
            },
        ),
    ];
    for (expected_limit, counts) in cases {
        assert!(matches!(
            validate_function_context_limits(counts),
            Err(FunctionContextError::LimitExceeded {
                limit,
                maximum: _,
                observed: _
            }) if limit == expected_limit
        ));
    }
    assert!(matches!(
        validate_function_context_output_bytes(MAX_R17_CONTEXT_OUTPUT_BYTES + 1),
        Err(FunctionContextError::LimitExceeded {
            limit: "context_output_bytes_including_lf",
            maximum: MAX_R17_CONTEXT_OUTPUT_BYTES,
            observed
        }) if observed == MAX_R17_CONTEXT_OUTPUT_BYTES + 1
    ));
}

#[test]
fn ct_fr_exp_009_additive_contract_values_are_exact() {
    assert_eq!(R17_CONTEXT_PROFILE, "rust-function-context-v1");
    assert_eq!(
        R17_FUNCTION_CONTEXT_VERSION,
        "codenoesis.function-context/v1"
    );
    assert_eq!(R17_LOCAL_EXPLORER_VERSION, "codenoesis.local-explorer/v10");
    assert_eq!(
        R17_EXPLORER_SECURITY_PROFILE,
        "codenoesis.local-explorer-security/v10"
    );
    assert_eq!(MAX_R16_PORTABLE_GRAPH_BYTES, 268_435_456);
    assert_eq!(MAX_R17_FUNCTION_SEARCH_RESULTS, 100);
    assert_eq!(MAX_R17_NAVIGATION_HISTORY, 128);
}
