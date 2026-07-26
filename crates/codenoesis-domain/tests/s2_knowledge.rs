use codenoesis_domain::knowledge::{
    ClaimState, EntityKind, ONTOLOGY_VERSION, RelationshipKind, stable_claim_id, stable_entity_id,
    stable_relationship_id, validate_claim_transition,
};

const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s2-rust-knowledge-v1";

#[test]
fn pt_dr_idn_001_stable_ids_match_reviewed_preimages() {
    let crate_id = stable_entity_id(REPOSITORY_ID, EntityKind::RustCrate, "crate");
    assert_eq!(
        crate_id,
        "urn:codenoesis:entity:blake3:df3ec7670cebe2dedf4deb17000a4dabdce69e7fd81864914d5644d1a58ad363"
    );

    let file_id = stable_entity_id(REPOSITORY_ID, EntityKind::SourceFile, "file:src/lib.rs");
    assert_eq!(
        file_id,
        "urn:codenoesis:entity:blake3:11e4fbcfea4e1daa255e129d1f90760bac6fbd06c4a274ac44116cf9fd9c43d8"
    );

    let relationship_id = stable_relationship_id(
        REPOSITORY_ID,
        RelationshipKind::Contains,
        &crate_id,
        &file_id,
    );
    assert_eq!(
        relationship_id,
        "urn:codenoesis:relationship:blake3:c13ee6387ec24e48ea6a1b379b2e7b1f6f8ae1485cbd9bb0b5cfe1a1e4ff16bb"
    );

    assert_eq!(
        stable_claim_id(REPOSITORY_ID, "relationship", &relationship_id),
        "urn:codenoesis:claim:blake3:00c76b16ac3ea21cd83e5681bc0f6d549ef8acccfa88f14e8ce7c8ca7e1c72a3"
    );
    assert_eq!(ONTOLOGY_VERSION, "codenoesis.ontology/rust/v1");
}

#[test]
fn pt_fr_knw_002_claim_state_machine_accepts_exactly_eleven_transitions() {
    let states = ClaimState::ALL;
    let mut accepted = Vec::new();
    for source in states {
        for target in states {
            if validate_claim_transition(source, target).is_ok() {
                accepted.push((source.as_str(), target.as_str()));
            }
        }
    }
    accepted.sort_unstable();

    assert_eq!(
        accepted,
        vec![
            ("candidate", "confirmed"),
            ("candidate", "rejected"),
            ("candidate", "reviewed_inference"),
            ("candidate", "superseded"),
            ("confirmed", "superseded"),
            ("derived_fact", "superseded"),
            ("deterministic_fact", "superseded"),
            ("rejected", "superseded"),
            ("reviewed_inference", "confirmed"),
            ("reviewed_inference", "rejected"),
            ("reviewed_inference", "superseded"),
        ]
    );
}
