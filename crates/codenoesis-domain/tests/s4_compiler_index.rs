use codenoesis_domain::s4_r7::{
    CompilerIndexError, CompilerIndexLimit, R7_DETERMINISM_PERMUTATIONS,
    compiler_index_limit_exceeded,
};

#[test]
fn pt_fr_ext_005_limits_have_max_and_plus_one() {
    let reviewed = [
        (CompilerIndexLimit::RawIndexBytes, 67_108_864),
        (CompilerIndexLimit::BindingJsonBytes, 1_048_576),
        (CompilerIndexLimit::Documents, 20_000),
        (CompilerIndexLimit::OccurrencesTotal, 1_000_000),
        (CompilerIndexLimit::OccurrencesPerDocument, 100_000),
        (CompilerIndexLimit::SymbolInformationTotal, 250_000),
        (CompilerIndexLimit::RelationshipsTotal, 500_000),
        (CompilerIndexLimit::SymbolOrDisplayBytes, 16_384),
        (CompilerIndexLimit::UnpromotedValueBytes, 65_536),
        (CompilerIndexLimit::ToolArguments, 128),
        (CompilerIndexLimit::ToolArgumentBytes, 4_096),
        (CompilerIndexLimit::ProtobufRecursion, 64),
    ];
    assert_eq!(CompilerIndexLimit::ALL.len(), reviewed.len());
    for (limit, maximum) in reviewed {
        assert_eq!(limit.maximum(), maximum);
        assert!(matches!(
            compiler_index_limit_exceeded(limit, maximum + 1),
            CompilerIndexError::LimitExceeded {
                limit: observed_limit,
                maximum: observed_maximum,
                observed
            } if observed_limit == limit
                && observed_maximum == maximum
                && observed == maximum + 1
        ));
        assert!(matches!(
            compiler_index_limit_exceeded(limit, maximum + 2),
            CompilerIndexError::LimitExceeded { observed, .. } if observed == maximum + 1
        ));
    }
}

#[test]
fn pt_fr_ext_005_determinism_budget_is_fixed() {
    assert_eq!(R7_DETERMINISM_PERMUTATIONS, 50);
}
