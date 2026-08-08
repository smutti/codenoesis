use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::thread;

use codenoesis_domain::s4_k1::{
    CallResolutionState, CallableRelationshipKind, CallableSemanticEntityKind,
    CallableSemanticProperties, ControlKind, DeclaredValueState, K1_DETERMINISM_PERMUTATIONS,
    K1_DETERMINISM_SCHEDULES,
};
use codenoesis_domain::{
    AcquiredFile, AcquiredRepository, BoundRevision, ObjectId, RegularFileMode, RepositoryIdentity,
    RepositoryInventory,
};
use codenoesis_lang_rust::TreeSitterRustWorkspaceExtractor;

const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s4-rust-callable-semantics-v1";
const COMMIT_OID: &str = "9a7bb3adaa5bf30eef3bc9bc656c81f42fbdb845";
const TREE_OID: &str = "ead855e0545cc26b351b305fcad39f2e491b285d";
const FIXTURE_FILES: [(&str, &str); 4] = [
    ("Cargo.toml", "cdf88f6e5c3f0bc5444379612ed5bc21796c8036"),
    ("build.rs", "9ab1798b7d4af3395e972d6838ff07d3e7538375"),
    ("src/lib.rs", "c715d312552e61ed74c8a9a33a04bbdbe9d28354"),
    ("src/model.rs", "6da92321f9b7578a09a9950fd90503f0bc548978"),
];

#[test]
fn gt_fr_ext_012_complete_signatures_and_parameters() {
    let graph = &extract_fixture(0, false).knowledge.graph;
    let counts = entity_counts(graph);
    assert_eq!(counts[&CallableSemanticEntityKind::Signature], 9);
    assert_eq!(counts[&CallableSemanticEntityKind::Parameter], 15);
    assert_eq!(
        graph
            .entities
            .iter()
            .filter(|entity| entity.kind == CallableSemanticEntityKind::Signature)
            .map(|entity| entity.name.as_str())
            .fold(BTreeMap::new(), |mut counts, name| {
                *counts.entry(name).or_insert(0_usize) += 1;
                counts
            }),
        BTreeMap::from([
            ("async_entry", 1),
            ("control_flow", 1),
            ("fallible", 1),
            ("ffi_identity", 1),
            ("helper", 1),
            ("label", 1),
            ("process", 2),
            ("run", 1),
        ])
    );
    assert!(graph.entities.iter().all(|entity| {
        !matches!(
            &entity.properties,
            CallableSemanticProperties::Signature(properties)
                if properties.body_state.as_str() == "present"
                    && (properties.body_digest.is_none()
                        || properties.body_evidence_id.is_none())
        )
    }));
}

#[test]
fn gt_fr_ext_012_declared_values_are_bounded_and_honest() {
    let graph = &extract_fixture(0, false).knowledge.graph;
    let values = graph
        .entities
        .iter()
        .filter_map(|entity| match &entity.properties {
            CallableSemanticProperties::DeclaredValue(properties) => {
                Some((entity.name.as_str(), properties))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(values.len(), 10);
    let mut states = BTreeMap::new();
    for (_, properties) in &values {
        *states.entry(properties.state).or_insert(0_usize) += 1;
    }
    assert_eq!(states[&DeclaredValueState::NormalizedScalar], 7);
    assert_eq!(states[&DeclaredValueState::ExpressionOnly], 2);
    assert_eq!(states[&DeclaredValueState::Unresolved], 1);
    assert_eq!(
        values
            .iter()
            .filter(|(_, properties)| properties.state == DeclaredValueState::ExpressionOnly)
            .map(|(name, _)| *name)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["COMPUTED", "Computed"])
    );
    assert!(values.iter().any(|(name, properties)| {
        *name == "Pending"
            && properties.expression_digest.is_none()
            && properties.normalized.is_none()
    }));
}

#[test]
fn gt_fr_ext_012_calls_controls_and_lexical_nesting() {
    let graph = &extract_fixture(0, false).knowledge.graph;
    let counts = entity_counts(graph);
    assert_eq!(counts[&CallableSemanticEntityKind::LocalBinding], 4);
    assert_eq!(counts[&CallableSemanticEntityKind::CallSite], 9);
    assert_eq!(counts[&CallableSemanticEntityKind::Control], 11);
    assert_eq!(
        graph
            .relationships
            .iter()
            .filter(|relationship| relationship.kind == CallableRelationshipKind::HasBodyFact)
            .count(),
        24
    );
    assert_eq!(
        graph
            .relationships
            .iter()
            .filter(|relationship| relationship.kind == CallableRelationshipKind::Calls)
            .count(),
        4
    );
    let unresolved = graph
        .entities
        .iter()
        .filter_map(|entity| match &entity.properties {
            CallableSemanticProperties::CallSite(properties)
                if properties.resolution_state == CallResolutionState::CandidateUnresolved =>
            {
                Some(properties.target_spelling.as_str())
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        unresolved,
        BTreeSet::from([
            "external::dispatch",
            "input.clone",
            "input.to_owned",
            "items.pop",
            "worker.run",
        ])
    );
    let controls = graph
        .entities
        .iter()
        .filter_map(|entity| match &entity.properties {
            CallableSemanticProperties::Control(properties) => Some(properties.control_kind),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(controls, ControlKind::ALL.into_iter().collect());
    assert!(graph.entities.iter().any(|entity| {
        matches!(
            &entity.properties,
            CallableSemanticProperties::Control(properties)
                if properties.lexical_depth > 0 && properties.parent_fact_id.is_some()
        )
    }));
}

#[test]
fn pt_nfr_det_001_k1_fifty_permutations_ten_schedules() {
    let expected = extract_fixture(0, false).knowledge;
    for permutation in 0..K1_DETERMINISM_PERMUTATIONS {
        let rotation = usize::try_from(permutation).expect("K1 permutation index");
        assert_eq!(
            extract_fixture(rotation, permutation % 2 == 1).knowledge,
            expected
        );
    }
    thread::scope(|scope| {
        let handles = (0..K1_DETERMINISM_SCHEDULES)
            .map(|schedule| {
                scope.spawn(move || {
                    let rotation = usize::try_from(schedule).expect("K1 schedule index");
                    extract_fixture(rotation, schedule % 2 == 1).knowledge
                })
            })
            .collect::<Vec<_>>();
        for handle in handles {
            assert_eq!(handle.join().expect("K1 replay worker"), expected);
        }
    });
}

#[test]
fn sec_fr_ext_012_never_executes_target_or_toolchain() {
    let extraction = extract_fixture(0, false);
    assert!(!format!("{extraction:?}").contains("K1_BUILD_SENTINEL_EXECUTED"));
}

fn entity_counts(
    graph: &codenoesis_domain::s4_k1::CallableSemanticsGraph,
) -> BTreeMap<CallableSemanticEntityKind, usize> {
    let mut counts = BTreeMap::new();
    for entity in &graph.entities {
        *counts.entry(entity.kind).or_insert(0) += 1;
    }
    counts
}

fn extract_fixture(
    rotation: usize,
    reverse: bool,
) -> codenoesis_domain::s4_k1::CallableSemanticsExtraction {
    TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_callable_semantics(&fixture_inventory(rotation, reverse))
        .expect("extract reviewed K1 callable-semantics fixture")
}

fn fixture_inventory(rotation: usize, reverse: bool) -> RepositoryInventory {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/s4/rust-callable-semantics-v1/repository");
    let mut files = FIXTURE_FILES
        .into_iter()
        .map(|(path, blob_oid)| {
            AcquiredFile::new(
                path.to_owned(),
                RegularFileMode::Regular,
                ObjectId::parse_sha1(blob_oid).expect("reviewed K1 blob OID"),
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
            RepositoryIdentity::parse(REPOSITORY_ID).expect("reviewed K1 repository identity"),
            ObjectId::parse_sha1(COMMIT_OID).expect("reviewed K1 commit OID"),
            ObjectId::parse_sha1(TREE_OID).expect("reviewed K1 tree OID"),
        ),
        u64::try_from(files.len()).expect("reviewed K1 file count"),
        files,
    ))
}

fn read_reviewed_fixture(path: &Path) -> Vec<u8> {
    let bytes = fs::read(path).unwrap_or_else(|error| {
        panic!("read reviewed K1 fixture file {}: {error}", path.display())
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
