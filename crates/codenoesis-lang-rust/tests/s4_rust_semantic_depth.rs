use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::thread;

use codenoesis_domain::knowledge::RelationshipKind;
use codenoesis_domain::s4_r5::{
    CompilationPresence, R5_DETERMINISM_PERMUTATIONS, RustMethodContext, RustSemanticEntityKind,
    RustSemanticProperties, capability_state, r5_entity_counts,
};
use codenoesis_domain::{
    AcquiredFile, AcquiredRepository, BoundRevision, ObjectId, RegularFileMode, RepositoryIdentity,
    RepositoryInventory,
};
use codenoesis_lang_rust::TreeSitterRustWorkspaceExtractor;
use codenoesis_ports::RustSemanticDepthExtractor;

const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s4-rust-semantic-depth-v1";
const COMMIT_OID: &str = "4fc9e6efd37c289a347c9f642fdac0ef611c9fe8";
const TREE_OID: &str = "9f2229d9caf4efacfb8dc0eccf1e47e6bd738fef";
const FIXTURE_FILES: [(&str, &str); 4] = [
    ("Cargo.toml", "e6f7f97a4b080927cdee4e494576e43344246eee"),
    ("build.rs", "7990ab0f9cf4d3e3acf2020c7aa7cbd616870d64"),
    ("src/lib.rs", "568a344c63d688a4c4b5b391d106848a39f25a04"),
    ("src/model.rs", "486549154e06c2a7c9d017109a00cadf1e6eaa69"),
];

#[test]
fn gt_fr_ext_010_fields_and_variants_are_owned() {
    let extraction = extract_fixture(0, false);
    let graph = &extraction.knowledge.graph;
    let counts = r5_entity_counts(&graph.entities);
    assert_eq!(counts[&RustSemanticEntityKind::Field], 16);
    assert_eq!(counts[&RustSemanticEntityKind::EnumVariant], 5);
    let member_ids = graph
        .entities
        .iter()
        .map(|entity| entity.id.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        graph
            .relationships
            .iter()
            .filter(|relationship| {
                relationship.kind == RelationshipKind::Defines
                    && member_ids.contains(relationship.target.as_str())
            })
            .count(),
        40
    );
    assert!(graph.entities.iter().any(|entity| {
        entity.id
            == "urn:codenoesis:entity:blake3:ab24c82375e533b482ef71fe657a00b190ecf49712498cdc772c8d9db946b9d4"
            && entity.kind == RustSemanticEntityKind::Field
            && entity.name == "key"
    }));
}

#[test]
fn gt_fr_ext_010_constants_statics_and_associated_types() {
    let extraction = extract_fixture(0, false);
    let graph = &extraction.knowledge.graph;
    let counts = r5_entity_counts(&graph.entities);
    assert_eq!(counts[&RustSemanticEntityKind::Constant], 6);
    assert_eq!(counts[&RustSemanticEntityKind::Static], 2);
    assert_eq!(counts[&RustSemanticEntityKind::AssociatedType], 2);
    assert!(graph.entities.iter().any(|entity| {
        entity.id
            == "urn:codenoesis:entity:blake3:ecc0131c233fb18b11b7ff701d7304763f5483c6b21fe3675a415e31bd70e756"
            && entity.name == "DEFAULT_LIMIT"
    }));
}

#[test]
fn gt_fr_ext_010_method_context_prevents_trait_collisions() {
    let graph = &extract_fixture(0, false).knowledge.graph;
    assert_eq!(
        r5_entity_counts(&graph.entities)[&RustSemanticEntityKind::Method],
        9
    );
    let render_methods = graph
        .entities
        .iter()
        .filter(|entity| entity.kind == RustSemanticEntityKind::Method && entity.name == "render")
        .collect::<Vec<_>>();
    assert_eq!(render_methods.len(), 4);
    let implementation_methods = render_methods
        .iter()
        .filter(|entity| {
            matches!(
                &entity.properties,
                RustSemanticProperties::Method(properties)
                    if properties.implementation_context
                        == RustMethodContext::NamedLocalTraitImplementation
            )
        })
        .map(|entity| entity.id.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        implementation_methods,
        BTreeSet::from([
            "urn:codenoesis:entity:blake3:1f78bcb85d1c36b3b5fd7c5886002baea09b44bbfb7a0efa8d04963f3272877f",
            "urn:codenoesis:entity:blake3:4f9b4995acaeb649751a33f150e20e6b70ae1550348e18e5a8eb8f14d7b00164",
        ])
    );
    assert_eq!(
        graph
            .relationships
            .iter()
            .filter(|relationship| relationship.kind == RelationshipKind::Implements)
            .count(),
        3
    );
}

#[test]
fn gt_fr_ext_010_attributes_preserve_declarations_and_gaps() {
    let graph = &extract_fixture(0, false).knowledge.graph;
    let mut presence = BTreeMap::new();
    for entity in &graph.entities {
        *presence
            .entry(entity.compilation_presence)
            .or_insert(0_usize) += 1;
    }
    assert_eq!(presence[&CompilationPresence::Unconditional], 38);
    assert_eq!(presence[&CompilationPresence::ConditionalUnknown], 1);
    assert_eq!(presence[&CompilationPresence::AttributeTransformUnknown], 1);
    assert_eq!(
        graph
            .coverage
            .iter()
            .map(|gap| (gap.capability.as_str(), gap.state))
            .collect::<BTreeSet<_>>(),
        [
            "rust.attribute_semantics_not_interpreted",
            "rust.cfg_presence_unresolved",
            "rust.foreign_block_unsupported",
            "rust.macro_generated_items_not_analyzed",
            "rust.type_resolution_not_performed",
            "rust.union_unsupported",
            "rust.unsupported_impl_header",
            "rust.value_not_evaluated",
        ]
        .into_iter()
        .map(|capability| (
            capability,
            capability_state(capability).expect("reviewed state")
        ))
        .collect::<BTreeSet<_>>()
    );
}

#[test]
fn pt_nfr_det_001_r5_permutation_and_schedule_invariant() {
    let expected = extract_fixture(0, false).knowledge;
    for permutation in 0..R5_DETERMINISM_PERMUTATIONS {
        let rotation = usize::try_from(permutation).expect("permutation index");
        assert_eq!(
            extract_fixture(rotation, permutation % 2 == 1).knowledge,
            expected,
            "R5 permutation {permutation} changed knowledge"
        );
    }
    thread::scope(|scope| {
        let handles = (0..8)
            .map(|worker| {
                scope.spawn(move || extract_fixture(worker * 3, worker % 2 == 1).knowledge)
            })
            .collect::<Vec<_>>();
        for handle in handles {
            assert_eq!(handle.join().expect("R5 replay worker"), expected);
        }
    });
}

#[test]
fn sec_fr_ext_010_never_executes_or_interprets_target_worlds() {
    let extraction = extract_fixture(0, false);
    let debug = format!("{:#?}", extraction.knowledge);
    for decoy in [
        "CommentFieldDecoy",
        "StringVariantDecoy",
        "MACRO_CONSTANT_DECOY",
        "MacroFieldDecoy",
    ] {
        assert!(
            !extraction
                .knowledge
                .graph
                .entities
                .iter()
                .any(|entity| entity.name == decoy)
        );
    }
    for invented in ["CALLS", "EXECUTES", "framework_role", "active_cfg"] {
        assert!(!debug.contains(invented));
    }
    assert!(
        extraction.knowledge.graph.entities.iter().any(|entity| {
            entity.compilation_presence == CompilationPresence::ConditionalUnknown
        })
    );
    assert!(debug.contains("rust.macro_generated_items_not_analyzed"));
}

fn extract_fixture(
    rotation: usize,
    reverse: bool,
) -> codenoesis_domain::s4_r5::RustSemanticDepthExtraction {
    TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_semantic_depth_incremental(&fixture_inventory(rotation, reverse), &[], &[])
        .expect("extract reviewed R5 semantic-depth fixture")
}

fn fixture_inventory(rotation: usize, reverse: bool) -> RepositoryInventory {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/s4/rust-semantic-depth-v1/repository");
    let mut files = FIXTURE_FILES
        .into_iter()
        .map(|(path, blob_oid)| {
            AcquiredFile::new(
                path.to_owned(),
                RegularFileMode::Regular,
                ObjectId::parse_sha1(blob_oid).expect("reviewed R5 blob OID"),
                read_reviewed_fixture(&root.join(path)),
            )
        })
        .collect::<Vec<_>>();
    let length = files.len();
    files.rotate_left(rotation % length);
    if reverse {
        files.reverse();
    }
    RepositoryInventory::classify(AcquiredRepository::new(
        BoundRevision::new(
            RepositoryIdentity::parse(REPOSITORY_ID).expect("reviewed R5 repository identity"),
            ObjectId::parse_sha1(COMMIT_OID).expect("reviewed R5 commit OID"),
            ObjectId::parse_sha1(TREE_OID).expect("reviewed R5 tree OID"),
        ),
        u64::try_from(files.len()).expect("reviewed R5 file count"),
        files,
    ))
}

fn read_reviewed_fixture(path: &Path) -> Vec<u8> {
    let bytes = fs::read(path).unwrap_or_else(|error| {
        panic!("read reviewed R5 fixture file {}: {error}", path.display())
    });
    let mut normalized = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'\r' {
            normalized.push(bytes[index]);
            index += 1;
            continue;
        }
        assert_eq!(bytes.get(index + 1), Some(&b'\n'));
        normalized.push(b'\n');
        index += 2;
    }
    normalized
}
