use codenoesis_domain::s4_r16::{
    ConstantEvaluationLimit, R16_CONFIGURATION_VERSION, R16_ERROR_VERSION, R16_GRAPH_VERSION,
    R16_INDEX_VERSION, R16_LOCAL_EXPLORER_VERSION, R16_ONTOLOGY_VERSION,
    R16_PORTABLE_GRAPH_VERSION, R16_QUERY_VERSION, R16_RULE_VERSION, R16_SNAPSHOT_VERSION,
    enforce_constant_limit, evaluated_value_id, evaluation_relationship_id,
};
use codenoesis_domain::storage::SNAPSHOT_SCHEMA_VERSION_V18;

const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s4-rust-safe-constant-evaluation-v1";
const BASE_DECLARED_VALUE_ID: &str =
    "urn:codenoesis:entity:blake3:ea203e69efa16a506f80011872387b88acd1e8e417e54f8fed6af71664689f3f";

#[test]
fn fr_ext_020_r16_contract_constants_and_reviewed_identities_are_exact() {
    assert_eq!(R16_CONFIGURATION_VERSION, "codenoesis.configuration/v15");
    assert_eq!(R16_SNAPSHOT_VERSION, SNAPSHOT_SCHEMA_VERSION_V18);
    assert_eq!(R16_GRAPH_VERSION, "codenoesis.knowledge-graph/v15");
    assert_eq!(R16_ONTOLOGY_VERSION, "codenoesis.ontology/rust/v15");
    assert_eq!(R16_ERROR_VERSION, "codenoesis.error/v24");
    assert_eq!(R16_QUERY_VERSION, "codenoesis.local-query-result/v13");
    assert_eq!(R16_PORTABLE_GRAPH_VERSION, "codenoesis.portable-graph/v9");
    assert_eq!(R16_LOCAL_EXPLORER_VERSION, "codenoesis.local-explorer/v9");
    assert_eq!(R16_INDEX_VERSION, "codenoesis.constant-evaluation-index/v1");
    assert_eq!(
        R16_RULE_VERSION,
        "codenoesis.rule/rust-safe-constant-evaluation/v1"
    );

    let evaluated = evaluated_value_id(REPOSITORY_ID, BASE_DECLARED_VALUE_ID);
    assert_eq!(
        evaluated,
        "urn:codenoesis:entity:blake3:84e293ed3d9aa98201e827f45484cb1bdb8e3f52df6ff189689edfc8959d3e8e"
    );
    assert_eq!(
        evaluation_relationship_id(BASE_DECLARED_VALUE_ID, &evaluated),
        "urn:codenoesis:relationship:blake3:91cb4854fab2fce746103ac84fca6601b4f595c39bc1ada6a804f66ab451f0af"
    );
}

#[test]
fn pt_fr_ext_020_each_r16_limit_accepts_maximum_and_rejects_plus_one() {
    for limit in [
        ConstantEvaluationLimit::CandidatesPerSource,
        ConstantEvaluationLimit::SyntaxNodesPerExpression,
        ConstantEvaluationLimit::DirectDependencies,
        ConstantEvaluationLimit::DependencyLevels,
        ConstantEvaluationLimit::VariantsPerEnum,
        ConstantEvaluationLimit::EvaluatedEntities,
        ConstantEvaluationLimit::EvaluationRelationships,
        ConstantEvaluationLimit::DependencyReferences,
        ConstantEvaluationLimit::DerivationInputReferences,
    ] {
        let maximum = usize::try_from(limit.maximum()).expect("R16 maximum fits usize");
        assert_eq!(enforce_constant_limit(limit, maximum), Ok(()));
        assert!(matches!(
            enforce_constant_limit(limit, maximum + 1),
            Err(codenoesis_domain::s4_r16::ConstantEvaluationError::LimitExceeded {
                limit: observed_limit,
                maximum: observed_maximum,
                observed,
            }) if observed_limit == limit
                && observed_maximum == limit.maximum()
                && observed == limit.maximum() + 1
        ));
    }
}
