use codenoesis_domain::s4_r14::{
    ExpressionBindingError, ExpressionBindingLimit, ExpressionRelationshipKind,
    R14_CONFIGURATION_VERSION, R14_ERROR_VERSION, R14_EXTRACTION_CHUNK_VERSION,
    R14_EXTRACTION_CONTRACT_VERSION, R14_EXTRACTOR_VERSION, R14_GRAPH_VERSION, R14_INDEX_VERSION,
    R14_LOCAL_EXPLORER_VERSION, R14_ONTOLOGY_VERSION, R14_PIPELINE_VERSION,
    R14_PORTABLE_GRAPH_VERSION, R14_QUERY_VERSION, R14_SEMANTIC_HASH_CONTRACT_VERSION,
    R14_SNAPSHOT_VERSION, call_argument_entity_id, enforce_expression_limit,
    expression_relationship_id,
};
use codenoesis_domain::storage::{
    EXTRACTION_HASH_DOMAIN_V13, GRAPH_HASH_DOMAIN_V13, SNAPSHOT_HASH_DOMAIN_V16,
    SNAPSHOT_SCHEMA_VERSION_V16, extraction_hash_domain, graph_hash_domain, snapshot_hash_domain,
};

#[test]
fn conf_fr_ext_016_r14_contract_versions_are_exact() {
    assert_eq!(R14_CONFIGURATION_VERSION, "codenoesis.configuration/v13");
    assert_eq!(R14_SNAPSHOT_VERSION, "codenoesis.repository-snapshot/v16");
    assert_eq!(R14_EXTRACTION_CONTRACT_VERSION, "codenoesis.extraction/v13");
    assert_eq!(
        R14_EXTRACTION_CHUNK_VERSION,
        "codenoesis.extraction-chunk/v13"
    );
    assert_eq!(R14_GRAPH_VERSION, "codenoesis.knowledge-graph/v13");
    assert_eq!(R14_ONTOLOGY_VERSION, "codenoesis.ontology/rust/v13");
    assert_eq!(R14_ERROR_VERSION, "codenoesis.error/v21");
    assert_eq!(R14_QUERY_VERSION, "codenoesis.local-query-result/v11");
    assert_eq!(R14_PORTABLE_GRAPH_VERSION, "codenoesis.portable-graph/v7");
    assert_eq!(R14_LOCAL_EXPLORER_VERSION, "codenoesis.local-explorer/v7");
    assert_eq!(R14_PIPELINE_VERSION, "codenoesis.pipeline/s4-r14-v1");
    assert_eq!(
        R14_EXTRACTOR_VERSION,
        "codenoesis.rust-expression-bindings/s4-r14-v1"
    );
    assert_eq!(R14_INDEX_VERSION, "codenoesis.expression-binding-index/v1");
    assert_eq!(
        R14_SEMANTIC_HASH_CONTRACT_VERSION,
        "codenoesis.semantic-hash-contract/v12"
    );
}

#[test]
fn conf_fr_ext_016_r14_hash_domains_are_additive() {
    assert_eq!(R14_SNAPSHOT_VERSION, SNAPSHOT_SCHEMA_VERSION_V16);
    assert_eq!(
        snapshot_hash_domain(SNAPSHOT_SCHEMA_VERSION_V16),
        Some(SNAPSHOT_HASH_DOMAIN_V16)
    );
    assert_eq!(
        graph_hash_domain(SNAPSHOT_SCHEMA_VERSION_V16),
        Some(GRAPH_HASH_DOMAIN_V13)
    );
    assert_eq!(
        extraction_hash_domain(SNAPSHOT_SCHEMA_VERSION_V16),
        Some(EXTRACTION_HASH_DOMAIN_V13)
    );
}

#[test]
fn gt_fr_ext_016_reviewed_argument_and_relationship_preimages_are_frozen() {
    let call_expression = "urn:codenoesis:entity:blake3:e371b44adf32711897c465a97c26c9191078f42ed24e15f3241492828ee7bb8e";
    assert_eq!(
        call_argument_entity_id(call_expression, 0),
        "urn:codenoesis:entity:blake3:3a64f199cfcbd2f966941f639145f840afbbf2c15d4ba1c60355faee685d5767"
    );
    assert_eq!(
        expression_relationship_id(
            ExpressionRelationshipKind::HasArgument,
            "urn:codenoesis:entity:blake3:41d6d825f2bce195fbd4a20369879514333c2772bf0f409da4edcb3450df45bb",
            "urn:codenoesis:entity:blake3:b42c25322f6cf1c71a043ecc23c9690de6b0584da94752050f002c2a40e22fa7",
        ),
        "urn:codenoesis:relationship:blake3:008dc10f033f3afe1b7940f1525dea21a4b2fadf8bc161a3f6d8ab054e66e445"
    );
}

#[test]
fn ft_fr_ext_016_every_r14_limit_accepts_maximum_and_rejects_maximum_plus_one() {
    for limit in [
        ExpressionBindingLimit::ExpressionsPerCallable,
        ExpressionBindingLimit::ExpressionDepth,
        ExpressionBindingLimit::ArgumentsPerCall,
        ExpressionBindingLimit::BindingsPerCallable,
        ExpressionBindingLimit::ExpressionsTotal,
        ExpressionBindingLimit::BindingsAndArgumentsTotal,
        ExpressionBindingLimit::RelationshipsTotal,
        ExpressionBindingLimit::NormalizedSpellingBytes,
    ] {
        let maximum = usize::try_from(limit.maximum()).expect("R14 limit fits usize");
        assert_eq!(enforce_expression_limit(limit, maximum), Ok(()));
        assert_eq!(
            enforce_expression_limit(limit, maximum + 1),
            Err(ExpressionBindingError::LimitExceeded {
                limit,
                maximum: limit.maximum(),
                observed: limit.maximum() + 1,
            })
        );
    }
}
