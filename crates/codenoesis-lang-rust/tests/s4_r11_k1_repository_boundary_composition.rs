use std::thread;

use codenoesis_domain::s4_k1::{
    CallableSemanticEntityKind, K1_DETERMINISM_PERMUTATIONS, K1_DETERMINISM_SCHEDULES,
};
use codenoesis_domain::s4_r3::ExternalWorkspaceBoundary;
use codenoesis_domain::{
    AcquiredFile, AcquiredRepository, BoundRevision, ObjectId, RegularFileMode, RepositoryIdentity,
    RepositoryInventory,
};
use codenoesis_lang_rust::TreeSitterRustWorkspaceExtractor;
use codenoesis_ports::RustCallableBoundaryCompositionExtractor;

const BOUNDARY_ID: &str = "urn:codenoesis:repository-boundary:sha256:7f8f79973410e908962009f651f418416b57a4921c6b27e539dbed2696c45fd1";

#[test]
fn gt_fr_ext_012_boundary_aware_k1_keeps_gitlink_external() {
    let inventory = inventory(0, false);
    let extraction = extract(&inventory);
    extraction
        .knowledge
        .validate()
        .expect("validate boundary-aware K1 projection");

    let workspace = &extraction.knowledge.framework.semantic.manifest.workspace;
    let external = workspace
        .plan
        .members
        .iter()
        .find(|member| member.path == "external/model")
        .expect("external workspace member");
    assert_eq!(external.external_boundary_id.as_deref(), Some(BOUNDARY_ID));
    assert!(external.manifest_path.is_none());
    assert!(external.crate_ids.is_empty());
    assert_eq!(inventory.files().len(), 2);
    assert_eq!(
        extraction
            .knowledge
            .graph
            .entities
            .iter()
            .filter(|entity| entity.kind == CallableSemanticEntityKind::Signature)
            .count(),
        1
    );
}

#[test]
fn pt_nfr_det_001_r11_fifty_permutations_ten_schedules() {
    let expected = extract(&inventory(0, false)).knowledge;
    for permutation in 0..K1_DETERMINISM_PERMUTATIONS {
        let rotation = usize::try_from(permutation).expect("R11 permutation index");
        assert_eq!(
            extract(&inventory(rotation, permutation % 2 == 1)).knowledge,
            expected
        );
    }
    thread::scope(|scope| {
        let handles = (0..K1_DETERMINISM_SCHEDULES)
            .map(|schedule| {
                scope.spawn(move || {
                    let rotation = usize::try_from(schedule).expect("R11 schedule index");
                    extract(&inventory(rotation, schedule % 2 == 1)).knowledge
                })
            })
            .collect::<Vec<_>>();
        for handle in handles {
            assert_eq!(handle.join().expect("R11 replay worker"), expected);
        }
    });
}

fn extract(
    inventory: &RepositoryInventory,
) -> codenoesis_domain::s4_k1::CallableSemanticsExtraction {
    TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_callable_semantics_with_boundaries(
            inventory,
            &[ExternalWorkspaceBoundary {
                path: "external/model".to_owned(),
                boundary_id: BOUNDARY_ID.to_owned(),
            }],
        )
        .expect("extract boundary-aware K1 projection")
}

fn inventory(rotation: usize, reverse: bool) -> RepositoryInventory {
    let mut files = vec![
        AcquiredFile::new(
            "Cargo.toml".to_owned(),
            RegularFileMode::Regular,
            oid('c'),
            b"[package]\nname=\"r11-lang\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[workspace]\nmembers=[\"external/model\"]\n[lib]\npath=\"src/lib.rs\"\n"
                .to_vec(),
        ),
        AcquiredFile::new(
            "src/lib.rs".to_owned(),
            RegularFileMode::Regular,
            oid('d'),
            b"pub struct Root;\nimpl Root { pub fn root_callable(value: u8) -> u8 { value } }\n"
                .to_vec(),
        ),
    ];
    let length = files.len();
    files.rotate_left(rotation % length);
    if reverse {
        files.reverse();
    }
    RepositoryInventory::classify(AcquiredRepository::new(
        BoundRevision::new(
            RepositoryIdentity::parse("urn:codenoesis:test:r11-lang")
                .expect("R11 language repository identity"),
            oid('a'),
            oid('b'),
        ),
        2,
        files,
    ))
}

fn oid(value: char) -> ObjectId {
    ObjectId::parse_sha1(&value.to_string().repeat(40)).expect("synthetic SHA-1 object ID")
}
