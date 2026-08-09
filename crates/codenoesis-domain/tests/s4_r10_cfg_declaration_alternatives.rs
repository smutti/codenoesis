use codenoesis_domain::s4_r10::{
    R10_CONFIGURATION_VERSION, R10_ERROR_VERSION, R10_EXTRACTION_CHUNK_VERSION,
    R10_EXTRACTION_CONTRACT_VERSION, R10_EXTRACTOR_VERSION, R10_GRAPH_VERSION, R10_INDEX_VERSION,
    R10_LOCAL_EXPLORER_VERSION, R10_ONTOLOGY_VERSION, R10_PIPELINE_VERSION,
    R10_PORTABLE_GRAPH_VERSION, R10_PROFILE, R10_QUERY_VERSION, R10_SNAPSHOT_VERSION,
    declaration_alternative_id, declaration_alternative_relationship_id,
};
use codenoesis_domain::storage::{
    EXTRACTION_HASH_DOMAIN_V9, GRAPH_HASH_DOMAIN_V9, SNAPSHOT_HASH_DOMAIN_V12,
    SNAPSHOT_SCHEMA_VERSION_V12, extraction_hash_domain, graph_hash_domain, snapshot_hash_domain,
};

const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s4-rust-cfg-declaration-alternatives-v1";
const LOGICAL_METHOD_ID: &str =
    "urn:codenoesis:entity:blake3:437b0bfcd3821ae91eabe8c395d99c80ec54cc53e6f1e6ca6e24098b20bf4b45";
const UNIX_DECLARATION_EVIDENCE_ID: &str = "urn:codenoesis:evidence:blake3:6390dfef60968e233c891126013da8d58faa2b38dd6573fe0729b88b040bf5a7";
const UNIX_ALTERNATIVE_ID: &str =
    "urn:codenoesis:entity:blake3:452f8e5e1fe8f0e22b43d49b7393c1b224261b8b2f47452ebaee4ca19794542d";
const UNIX_RELATIONSHIP_ID: &str = "urn:codenoesis:relationship:blake3:b7f0fc978d2ec344f7e28d7dbc24d357c5a53eb63588c478a746e0d507cd5408";

#[test]
fn gt_fr_ext_013_reviewed_identity_preimages_are_frozen() {
    assert_eq!(
        declaration_alternative_id(
            REPOSITORY_ID,
            LOGICAL_METHOD_ID,
            UNIX_DECLARATION_EVIDENCE_ID,
        ),
        UNIX_ALTERNATIVE_ID
    );
    assert_eq!(
        declaration_alternative_relationship_id(LOGICAL_METHOD_ID, UNIX_ALTERNATIVE_ID),
        UNIX_RELATIONSHIP_ID
    );
    assert_ne!(
        declaration_alternative_id(REPOSITORY_ID, LOGICAL_METHOD_ID, "different-evidence"),
        UNIX_ALTERNATIVE_ID
    );
}

#[test]
fn conf_fr_ext_013_r10_contract_versions_are_exact() {
    assert_eq!(R10_PROFILE, "rust-cfg-declaration-alternatives-v1");
    assert_eq!(R10_CONFIGURATION_VERSION, "codenoesis.configuration/v9");
    assert_eq!(R10_SNAPSHOT_VERSION, "codenoesis.repository-snapshot/v12");
    assert_eq!(R10_EXTRACTION_CONTRACT_VERSION, "codenoesis.extraction/v9");
    assert_eq!(
        R10_EXTRACTION_CHUNK_VERSION,
        "codenoesis.extraction-chunk/v9"
    );
    assert_eq!(R10_GRAPH_VERSION, "codenoesis.knowledge-graph/v9");
    assert_eq!(R10_ONTOLOGY_VERSION, "codenoesis.ontology/rust/v9");
    assert_eq!(R10_ERROR_VERSION, "codenoesis.error/v17");
    assert_eq!(R10_QUERY_VERSION, "codenoesis.local-query-result/v7");
    assert_eq!(R10_PORTABLE_GRAPH_VERSION, "codenoesis.portable-graph/v3");
    assert_eq!(R10_LOCAL_EXPLORER_VERSION, "codenoesis.local-explorer/v3");
    assert_eq!(R10_PIPELINE_VERSION, "codenoesis.pipeline/s4-r10-v1");
    assert_eq!(
        R10_EXTRACTOR_VERSION,
        "codenoesis.rust-cfg-alternatives/s4-r10-v1"
    );
    assert_eq!(
        R10_INDEX_VERSION,
        "codenoesis.rust-cfg-alternative-index/v1"
    );
    assert_eq!(
        snapshot_hash_domain(SNAPSHOT_SCHEMA_VERSION_V12),
        Some(SNAPSHOT_HASH_DOMAIN_V12)
    );
    assert_eq!(
        graph_hash_domain(SNAPSHOT_SCHEMA_VERSION_V12),
        Some(GRAPH_HASH_DOMAIN_V9)
    );
    assert_eq!(
        extraction_hash_domain(SNAPSHOT_SCHEMA_VERSION_V12),
        Some(EXTRACTION_HASH_DOMAIN_V9)
    );
}
