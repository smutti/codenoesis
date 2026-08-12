use codenoesis_domain::s7::{S7Limit, coverage_gap_id, diff_id, evidence_id};

#[test]
fn conf_fr_imp_004_stable_identity_preimages_match_golden() {
    assert_eq!(
        diff_id(
            "urn:codenoesis:fixture:s7-provider",
            "fixture-provider-a",
            "fixture-provider-b",
            "urn:codenoesis:field:blake3:a2b6cafe0db73cb016ffd790941c18fbab768923a5b8b4b671eb22459ec66301",
            "presence",
        ),
        "urn:codenoesis:diff:blake3:0ae7aad43c4c2dcf639e0e8929e9904118f02f9a35a1f370eb5d2cd1b541b251"
    );
    assert_eq!(
        evidence_id(
            "urn:codenoesis:fixture:s7-provider",
            "fixture-provider-a",
            "src/user_response.rs",
            6,
            6,
            "cb67bab3a4deab11c69a7d2f890a462bc464c059ddd813712179331a0bcf14d7",
        ),
        "urn:codenoesis:evidence:blake3:074cd404e50be00ae5994034b76986b555548f89730fbb0b8fdf2143427043fe"
    );
    assert_eq!(
        coverage_gap_id(
            "urn:codenoesis:field:blake3:a4bef896d066ecb3441a6d9638cea00947075d62c66bad642455840a7ce4f0e7",
            "unsupported_custom_provider_mapping",
            "fixture-provider-a",
            "fixture-provider-b",
        ),
        "urn:codenoesis:coverage-gap:blake3:d50c035847baacf04cdb30d284fa010e934182ac1295d5018dc1ba574488a3cb"
    );
}

#[test]
fn pt_od_lim_001_s7_every_limit_accepts_maximum_and_rejects_plus_one() {
    for limit in [
        S7Limit::WorkspaceBytes,
        S7Limit::FederationReportBytes,
        S7Limit::SourceBytesPerFile,
        S7Limit::TotalSourceBytes,
        S7Limit::LogicalPathBytes,
        S7Limit::CallableSymbolBytes,
        S7Limit::Operations,
        S7Limit::FieldsPerOperation,
        S7Limit::LinkedClients,
        S7Limit::CallSites,
        S7Limit::SemanticDiffs,
        S7Limit::EvidenceItems,
        S7Limit::CoverageGaps,
        S7Limit::ReportBytes,
        S7Limit::SourceFiles,
        S7Limit::SyntaxNodesPerSource,
        S7Limit::SourceNestingDepth,
        S7Limit::StringLiteralBytes,
    ] {
        let maximum = limit.maximum();
        assert_eq!(limit.check(maximum), Ok(()), "{} maximum", limit.as_str());
        let error = limit
            .check(maximum + 1)
            .expect_err("maximum plus one must fail");
        assert_eq!(error.limit, limit);
        assert_eq!(error.maximum, maximum);
        assert_eq!(error.observed, maximum + 1);
    }
}
