use codenoesis_domain::s4_r11::{
    R11_COMPOSITION_VERSION, R11_CONFIGURATION_VERSION, R11_ERROR_VERSION,
    R11_EXTRACTION_CHUNK_VERSION, R11_EXTRACTION_CONTRACT_VERSION, R11_GRAPH_VERSION,
    R11_LOCAL_EXPLORER_VERSION, R11_ONTOLOGY_VERSION, R11_PIPELINE_VERSION,
    R11_PORTABLE_GRAPH_VERSION, R11_QUERY_VERSION, R11_SNAPSHOT_VERSION,
};
use codenoesis_domain::storage::{
    EXTRACTION_HASH_DOMAIN_V10, GRAPH_HASH_DOMAIN_V10, SNAPSHOT_HASH_DOMAIN_V13,
    SNAPSHOT_SCHEMA_VERSION_V13, extraction_hash_domain, graph_hash_domain, snapshot_hash_domain,
};

#[test]
fn conf_fr_ext_012_r11_contract_versions_are_exact() {
    assert_eq!(R11_CONFIGURATION_VERSION, "codenoesis.configuration/v10");
    assert_eq!(R11_SNAPSHOT_VERSION, "codenoesis.repository-snapshot/v13");
    assert_eq!(R11_EXTRACTION_CONTRACT_VERSION, "codenoesis.extraction/v10");
    assert_eq!(
        R11_EXTRACTION_CHUNK_VERSION,
        "codenoesis.extraction-chunk/v10"
    );
    assert_eq!(R11_GRAPH_VERSION, "codenoesis.knowledge-graph/v10");
    assert_eq!(R11_ONTOLOGY_VERSION, "codenoesis.ontology/rust/v10");
    assert_eq!(R11_ERROR_VERSION, "codenoesis.error/v18");
    assert_eq!(R11_QUERY_VERSION, "codenoesis.local-query-result/v8");
    assert_eq!(R11_PORTABLE_GRAPH_VERSION, "codenoesis.portable-graph/v4");
    assert_eq!(R11_LOCAL_EXPLORER_VERSION, "codenoesis.local-explorer/v4");
    assert_eq!(R11_PIPELINE_VERSION, "codenoesis.pipeline/s4-r11-v1");
    assert_eq!(
        R11_COMPOSITION_VERSION,
        "codenoesis.rust-callable-boundary-composition/s4-r11-v1"
    );
}

#[test]
fn conf_fr_ext_012_r11_hash_domains_are_additive() {
    assert_eq!(R11_SNAPSHOT_VERSION, SNAPSHOT_SCHEMA_VERSION_V13);
    assert_eq!(
        snapshot_hash_domain(SNAPSHOT_SCHEMA_VERSION_V13),
        Some(SNAPSHOT_HASH_DOMAIN_V13)
    );
    assert_eq!(
        graph_hash_domain(SNAPSHOT_SCHEMA_VERSION_V13),
        Some(GRAPH_HASH_DOMAIN_V10)
    );
    assert_eq!(
        extraction_hash_domain(SNAPSHOT_SCHEMA_VERSION_V13),
        Some(EXTRACTION_HASH_DOMAIN_V10)
    );
}
