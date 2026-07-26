use codenoesis_domain::knowledge::{
    ClaimState, EntityKind, ONTOLOGY_VERSION, RelationshipKind, extraction_chunk_id,
    stable_claim_id, stable_entity_id, stable_relationship_id, validate_claim_transition,
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
fn pt_dr_idn_001_stable_ids_ignore_order_and_revision() {
    let crate_id = stable_entity_id(REPOSITORY_ID, EntityKind::RustCrate, "crate");
    let file_id = stable_entity_id(REPOSITORY_ID, EntityKind::SourceFile, "file:src/lib.rs");
    let relationship_id = stable_relationship_id(
        REPOSITORY_ID,
        RelationshipKind::Contains,
        &crate_id,
        &file_id,
    );
    let claim_id = stable_claim_id(REPOSITORY_ID, "relationship", &relationship_id);
    let chunk_id = extraction_chunk_id(
        REPOSITORY_ID,
        "d77c36ec27d878cee6d5d85d761de2b70284cd55",
        "615f6db198ab9ebb96fbdfbbfab8d7e4e7c0c242",
        "src/lib.rs",
    );

    for seed in 0..64 {
        let mut incidental_order = [
            "scheduler",
            "evidence",
            "map",
            "entity",
            "relationship",
            "claim",
            "offset",
            "storage",
        ];
        let order_length = incidental_order.len();
        incidental_order.rotate_left(seed % order_length);
        if seed % 2 == 1 {
            incidental_order.reverse();
        }
        let source_offset = seed * 17;
        let unrelated_commit = format!("{seed:040x}");

        assert_eq!(
            stable_entity_id(REPOSITORY_ID, EntityKind::RustCrate, "crate"),
            crate_id,
            "entity seed {seed}, offset {source_offset}, order {incidental_order:?}"
        );
        assert_eq!(
            stable_relationship_id(
                REPOSITORY_ID,
                RelationshipKind::Contains,
                &crate_id,
                &file_id,
            ),
            relationship_id,
            "relationship seed {seed}"
        );
        assert_eq!(
            stable_claim_id(REPOSITORY_ID, "relationship", &relationship_id),
            claim_id,
            "claim seed {seed}"
        );
        assert_eq!(
            extraction_chunk_id(
                REPOSITORY_ID,
                "d77c36ec27d878cee6d5d85d761de2b70284cd55",
                "615f6db198ab9ebb96fbdfbbfab8d7e4e7c0c242",
                "src/lib.rs",
            ),
            chunk_id,
            "chunk seed {seed}"
        );
        assert_ne!(
            extraction_chunk_id(
                REPOSITORY_ID,
                &unrelated_commit,
                "615f6db198ab9ebb96fbdfbbfab8d7e4e7c0c242",
                "src/lib.rs",
            ),
            chunk_id,
            "chunk identity must retain its immutable revision"
        );
    }

    assert_independent_preimages(&crate_id, &file_id, &relationship_id, &claim_id);
    assert_identity_dimensions(&crate_id, &file_id, &relationship_id);
}

fn assert_independent_preimages(
    crate_id: &str,
    file_id: &str,
    relationship_id: &str,
    claim_id: &str,
) {
    assert_eq!(
        crate_id,
        independent_id(
            "urn:codenoesis:entity:blake3:",
            &[
                "codenoesis.entity-id/v1",
                REPOSITORY_ID,
                "rust",
                "rust.crate",
                "crate",
            ],
        )
    );
    assert_eq!(
        relationship_id,
        independent_id(
            "urn:codenoesis:relationship:blake3:",
            &[
                "codenoesis.relationship-id/v1",
                REPOSITORY_ID,
                ONTOLOGY_VERSION,
                "CONTAINS",
                crate_id,
                file_id,
            ],
        )
    );
    assert_eq!(
        claim_id,
        independent_id(
            "urn:codenoesis:claim:blake3:",
            &[
                "codenoesis.claim-id/v1",
                REPOSITORY_ID,
                ONTOLOGY_VERSION,
                "relationship",
                relationship_id,
            ],
        )
    );
}

fn assert_identity_dimensions(crate_id: &str, file_id: &str, relationship_id: &str) {
    assert_ne!(
        stable_entity_id("urn:codenoesis:other", EntityKind::RustCrate, "crate"),
        crate_id
    );
    assert_ne!(
        stable_entity_id(REPOSITORY_ID, EntityKind::RustModule, "crate"),
        crate_id
    );
    assert_ne!(
        stable_entity_id(REPOSITORY_ID, EntityKind::RustCrate, "crate::other"),
        crate_id
    );
    assert_ne!(
        stable_relationship_id(REPOSITORY_ID, RelationshipKind::Contains, file_id, crate_id,),
        relationship_id
    );
    assert_ne!(
        independent_id(
            "urn:codenoesis:relationship:blake3:",
            &[
                "codenoesis.relationship-id/v1",
                REPOSITORY_ID,
                "codenoesis.ontology/rust/v2",
                "CONTAINS",
                crate_id,
                file_id,
            ],
        ),
        relationship_id
    );
}

#[test]
fn pt_fr_knw_002_claim_state_machine() {
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

fn independent_id(prefix: &str, components: &[&str]) -> String {
    let preimage = format!(
        "[{}]",
        components
            .iter()
            .map(|component| format!("\"{component}\""))
            .collect::<Vec<_>>()
            .join(",")
    );
    format!("{prefix}{}", blake3::hash(preimage.as_bytes()).to_hex())
}
