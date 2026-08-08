use codenoesis_domain::knowledge::{ClaimState, ClaimSubjectKind, EntityKind, RelationshipKind};
use codenoesis_domain::s4::{
    workspace_claim_id, workspace_crate_id, workspace_evidence_id, workspace_relationship_id,
};

const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s4-workspace-docs-v1";

#[test]
fn pt_dr_idn_002_workspace_identity_stability() {
    let crate_id = workspace_crate_id(
        REPOSITORY_ID,
        "crates/model/Cargo.toml",
        "workspace-model",
        "lib",
        "workspace_model",
    );
    assert_eq!(
        crate_id,
        "urn:codenoesis:entity:blake3:eaa0d43cb6168c2102dfbcd6c70f5a655a6415f92af26ef2c3a3a5937cf3c749"
    );

    let entity_id = codenoesis_domain::s4::workspace_declaration_id(
        REPOSITORY_ID,
        EntityKind::RustStruct,
        &crate_id,
        "crate::item",
        "Item",
    );
    assert_eq!(
        entity_id,
        "urn:codenoesis:entity:blake3:18bd97153a41f322136e8f93573877bfc1a2f43fabc6564b789e979a6cffcafa"
    );
    assert_eq!(
        workspace_claim_id(
            ClaimSubjectKind::Entity,
            &entity_id,
            ClaimState::DeterministicFact,
        ),
        "urn:codenoesis:claim:blake3:539a66db31096c678ae5b74aebf89a1237c609bcdd7ad1f54284e8ff158e1cb6"
    );
}

#[test]
fn pt_dr_idn_002_evidence_and_relationship_identity() {
    let evidence_id = workspace_evidence_id(
        REPOSITORY_ID,
        "c09d8c24e4704036c31b4f42e2f4df6e4acd347f",
        "885b4746097a67e5c4fb997a2082597dc23699e6",
        "crates/model/src/item.rs",
        0,
        37,
    );
    assert_eq!(
        evidence_id,
        "urn:codenoesis:evidence:blake3:61b8a6522478347afc01fe4ec2fa0576fdc7a8bb1383a59e7c5bf70bd8715bd4"
    );

    assert_eq!(
        workspace_relationship_id(
            RelationshipKind::Defines,
            "urn:codenoesis:entity:blake3:5f37308937c6af04439ee35bea02e1a2db12b08da4c3633bf49ee5dc767d11fb",
            "urn:codenoesis:entity:blake3:18bd97153a41f322136e8f93573877bfc1a2f43fabc6564b789e979a6cffcafa",
        ),
        "urn:codenoesis:relationship:blake3:de15b2640647ce1c2f4385e4b7751d105637c37a6f285025768c790ef56be02c"
    );
}
