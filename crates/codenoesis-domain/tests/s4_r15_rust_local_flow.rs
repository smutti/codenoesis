use codenoesis_domain::s4_r15::{
    LocalFlowBlockRole, LocalFlowLimit, LocalFlowRelationshipKind, R15_CONFIGURATION_VERSION,
    R15_ERROR_VERSION, R15_GRAPH_VERSION, R15_LOCAL_EXPLORER_VERSION, R15_ONTOLOGY_VERSION,
    R15_PORTABLE_GRAPH_VERSION, R15_QUERY_VERSION, R15_SNAPSHOT_VERSION, enforce_local_flow_limit,
    local_flow_evidence_id, local_flow_relationship_id, syntax_basic_block_id,
};
use codenoesis_domain::storage::SNAPSHOT_SCHEMA_VERSION_V17;

#[test]
fn fr_ext_017_r15_contract_constants_and_reviewed_identities_are_exact() {
    assert_eq!(R15_CONFIGURATION_VERSION, "codenoesis.configuration/v14");
    assert_eq!(R15_SNAPSHOT_VERSION, SNAPSHOT_SCHEMA_VERSION_V17);
    assert_eq!(R15_GRAPH_VERSION, "codenoesis.knowledge-graph/v14");
    assert_eq!(R15_ONTOLOGY_VERSION, "codenoesis.ontology/rust/v14");
    assert_eq!(R15_ERROR_VERSION, "codenoesis.error/v22");
    assert_eq!(R15_QUERY_VERSION, "codenoesis.local-query-result/v12");
    assert_eq!(R15_PORTABLE_GRAPH_VERSION, "codenoesis.portable-graph/v8");
    assert_eq!(R15_LOCAL_EXPLORER_VERSION, "codenoesis.local-explorer/v8");

    let block = syntax_basic_block_id(
        "urn:codenoesis:fixture:s4-rust-local-flow-v1",
        "urn:codenoesis:entity:blake3:2faedf3d82c8379dec454da67efa6535cda76d3eccadea1cf45df46e927cb211",
        "urn:codenoesis:entity:blake3:14a3d8d24f4cd60fd3dc13bd66cda07bc56fc08e162d72c025013fabef1f18f7",
        78,
        100,
        LocalFlowBlockRole::Entry,
        0,
    );
    assert_eq!(
        block,
        "urn:codenoesis:entity:blake3:1b9601fa1fc10bb313c69ed8ba725a841621690d666081bfa810dd4878e971cf"
    );
    assert_eq!(
        local_flow_relationship_id(
            LocalFlowRelationshipKind::HasSyntaxBlock,
            "urn:codenoesis:entity:blake3:2faedf3d82c8379dec454da67efa6535cda76d3eccadea1cf45df46e927cb211",
            &block,
        ),
        "urn:codenoesis:relationship:blake3:d3634dad074be9ce54049871a1460f8a6085857447cf5be79fe49956abfbab80"
    );
    assert_eq!(
        local_flow_evidence_id(
            "urn:codenoesis:fixture:s4-rust-local-flow-v1",
            "552a8cdc76b2dd80dc26ad1e3b381fc0de9eab24",
            "36be3c129a00aee60e6c312fdb4de133ad57b7bb",
            "src/lib.rs",
            78,
            100,
        ),
        "urn:codenoesis:evidence:blake3:f06f4ba33cf43f9899f72829da5447325031357c2f520eed39e9e8d2ca4e4e63"
    );
}

#[test]
fn pt_fr_ext_017_each_limit_accepts_maximum_and_rejects_plus_one() {
    for limit in [
        LocalFlowLimit::BlocksPerCallable,
        LocalFlowLimit::NestedBranches,
        LocalFlowLimit::FlowNodesPerBlock,
        LocalFlowLimit::ReachabilityPairsPerCallable,
        LocalFlowLimit::BlocksTotal,
        LocalFlowLimit::RelationshipsTotal,
        LocalFlowLimit::DerivationInputReferences,
    ] {
        let maximum = usize::try_from(limit.maximum()).expect("R15 maximum fits usize");
        assert_eq!(enforce_local_flow_limit(limit, maximum), Ok(()));
        assert!(enforce_local_flow_limit(limit, maximum + 1).is_err());
    }
}
