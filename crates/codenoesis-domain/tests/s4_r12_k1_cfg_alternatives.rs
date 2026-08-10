use codenoesis_domain::s4_r12::{
    R12_COMPOSITION_VERSION, R12_CONFIGURATION_VERSION, R12_ERROR_VERSION,
    R12_EXTRACTION_CHUNK_VERSION, R12_EXTRACTION_CONTRACT_VERSION, R12_EXTRACTOR_VERSION,
    R12_GRAPH_VERSION, R12_INDEX_VERSION, R12_LOCAL_EXPLORER_VERSION, R12_ONTOLOGY_VERSION,
    R12_PIPELINE_VERSION, R12_PORTABLE_GRAPH_VERSION, R12_QUERY_VERSION,
    R12_SEMANTIC_HASH_CONTRACT_VERSION, R12_SNAPSHOT_VERSION,
};
use codenoesis_domain::storage::{
    EXTRACTION_HASH_DOMAIN_V11, GRAPH_HASH_DOMAIN_V11, SNAPSHOT_HASH_DOMAIN_V14,
    SNAPSHOT_SCHEMA_VERSION_V14, extraction_hash_domain, graph_hash_domain, snapshot_hash_domain,
};

#[test]
fn conf_fr_ext_014_r12_contract_versions_are_exact() {
    assert_eq!(R12_CONFIGURATION_VERSION, "codenoesis.configuration/v11");
    assert_eq!(R12_SNAPSHOT_VERSION, "codenoesis.repository-snapshot/v14");
    assert_eq!(R12_EXTRACTION_CONTRACT_VERSION, "codenoesis.extraction/v11");
    assert_eq!(
        R12_EXTRACTION_CHUNK_VERSION,
        "codenoesis.extraction-chunk/v11"
    );
    assert_eq!(R12_GRAPH_VERSION, "codenoesis.knowledge-graph/v11");
    assert_eq!(R12_ONTOLOGY_VERSION, "codenoesis.ontology/rust/v11");
    assert_eq!(R12_ERROR_VERSION, "codenoesis.error/v19");
    assert_eq!(R12_QUERY_VERSION, "codenoesis.local-query-result/v9");
    assert_eq!(R12_PORTABLE_GRAPH_VERSION, "codenoesis.portable-graph/v5");
    assert_eq!(R12_LOCAL_EXPLORER_VERSION, "codenoesis.local-explorer/v5");
    assert_eq!(R12_PIPELINE_VERSION, "codenoesis.pipeline/s4-r12-v1");
    assert_eq!(
        R12_COMPOSITION_VERSION,
        "codenoesis.rust-callable-cfg-alternatives-composition/s4-r12-v1"
    );
    assert_eq!(
        R12_EXTRACTOR_VERSION,
        "codenoesis.rust-callable-cfg-alternatives/s4-r12-v1"
    );
    assert_eq!(
        R12_INDEX_VERSION,
        "codenoesis.callable-cfg-alternatives-index/v1"
    );
    assert_eq!(
        R12_SEMANTIC_HASH_CONTRACT_VERSION,
        "codenoesis.semantic-hash-contract/v10"
    );
}

#[test]
fn conf_fr_ext_014_r12_hash_domains_are_additive() {
    assert_eq!(R12_SNAPSHOT_VERSION, SNAPSHOT_SCHEMA_VERSION_V14);
    assert_eq!(
        snapshot_hash_domain(SNAPSHOT_SCHEMA_VERSION_V14),
        Some(SNAPSHOT_HASH_DOMAIN_V14)
    );
    assert_eq!(
        graph_hash_domain(SNAPSHOT_SCHEMA_VERSION_V14),
        Some(GRAPH_HASH_DOMAIN_V11)
    );
    assert_eq!(
        extraction_hash_domain(SNAPSHOT_SCHEMA_VERSION_V14),
        Some(EXTRACTION_HASH_DOMAIN_V11)
    );
}
