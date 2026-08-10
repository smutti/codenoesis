use codenoesis_domain::s4_r13::{
    R13_COMPOSITION_VERSION, R13_CONFIGURATION_VERSION, R13_ERROR_VERSION,
    R13_EXTRACTION_CHUNK_VERSION, R13_EXTRACTION_CONTRACT_VERSION, R13_GRAPH_VERSION,
    R13_INDEX_VERSION, R13_LOCAL_EXPLORER_VERSION, R13_ONTOLOGY_VERSION, R13_PIPELINE_VERSION,
    R13_PORTABLE_GRAPH_VERSION, R13_QUERY_VERSION, R13_SEMANTIC_HASH_CONTRACT_VERSION,
    R13_SNAPSHOT_VERSION, callable_compiler_relationship_id,
};
use codenoesis_domain::storage::{
    EXTRACTION_HASH_DOMAIN_V12, GRAPH_HASH_DOMAIN_V12, SNAPSHOT_HASH_DOMAIN_V15,
    SNAPSHOT_SCHEMA_VERSION_V15, extraction_hash_domain, graph_hash_domain, snapshot_hash_domain,
};

#[test]
fn conf_fr_ext_015_r13_contract_versions_are_exact() {
    assert_eq!(R13_CONFIGURATION_VERSION, "codenoesis.configuration/v12");
    assert_eq!(R13_SNAPSHOT_VERSION, "codenoesis.repository-snapshot/v15");
    assert_eq!(R13_EXTRACTION_CONTRACT_VERSION, "codenoesis.extraction/v12");
    assert_eq!(
        R13_EXTRACTION_CHUNK_VERSION,
        "codenoesis.extraction-chunk/v12"
    );
    assert_eq!(R13_GRAPH_VERSION, "codenoesis.knowledge-graph/v12");
    assert_eq!(R13_ONTOLOGY_VERSION, "codenoesis.ontology/rust/v12");
    assert_eq!(R13_ERROR_VERSION, "codenoesis.error/v20");
    assert_eq!(R13_QUERY_VERSION, "codenoesis.local-query-result/v10");
    assert_eq!(R13_PORTABLE_GRAPH_VERSION, "codenoesis.portable-graph/v6");
    assert_eq!(R13_LOCAL_EXPLORER_VERSION, "codenoesis.local-explorer/v6");
    assert_eq!(R13_PIPELINE_VERSION, "codenoesis.pipeline/s4-r13-v1");
    assert_eq!(
        R13_COMPOSITION_VERSION,
        "codenoesis.rust-callable-scip-composition/s4-r13-v1"
    );
    assert_eq!(
        R13_INDEX_VERSION,
        "codenoesis.callable-compiler-join-index/v1"
    );
    assert_eq!(
        R13_SEMANTIC_HASH_CONTRACT_VERSION,
        "codenoesis.semantic-hash-contract/v11"
    );
}

#[test]
fn conf_fr_ext_015_r13_hash_domains_are_additive() {
    assert_eq!(R13_SNAPSHOT_VERSION, SNAPSHOT_SCHEMA_VERSION_V15);
    assert_eq!(
        snapshot_hash_domain(SNAPSHOT_SCHEMA_VERSION_V15),
        Some(SNAPSHOT_HASH_DOMAIN_V15)
    );
    assert_eq!(
        graph_hash_domain(SNAPSHOT_SCHEMA_VERSION_V15),
        Some(GRAPH_HASH_DOMAIN_V12)
    );
    assert_eq!(
        extraction_hash_domain(SNAPSHOT_SCHEMA_VERSION_V15),
        Some(EXTRACTION_HASH_DOMAIN_V12)
    );
}

#[test]
fn gt_fr_ext_015_reviewed_join_identity_preimage_is_frozen() {
    assert_eq!(
        callable_compiler_relationship_id(
            "urn:codenoesis:entity:blake3:3681c2723fc170c8bb288bc052f3eefca07e3c386d340c4e099b20bc07825b0d",
            "urn:codenoesis:entity:blake3:e76696388a0810768639dd858c172a9f0d32273564a6c9456641bb3d11c31dfd",
        ),
        "urn:codenoesis:relationship:blake3:d53564518fd9312e7e582655604eb7c8a48e956ddbcfd27a336598287e8f3f03"
    );
}
