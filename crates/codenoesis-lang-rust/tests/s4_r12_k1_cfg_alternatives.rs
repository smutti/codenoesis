use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Barrier};

use codenoesis_domain::s4_k1::{
    CallableRelationshipKind, CallableSemanticEntityKind, CallableSemanticProperties,
    DeclaredValueState,
};
use codenoesis_domain::s4_r5::RustSemanticError;
use codenoesis_domain::s4_r10::RustCfgDeclarationAlternativesError;
use codenoesis_domain::s4_r12::CallableCfgAlternativesError;
use codenoesis_domain::{
    AcquiredFile, AcquiredRepository, BoundRevision, ObjectId, RegularFileMode, RepositoryIdentity,
    RepositoryInventory,
};
use codenoesis_lang_rust::TreeSitterRustWorkspaceExtractor;
use codenoesis_ports::RustCallableCfgAlternativesCompositionExtractor;

const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s4-rust-callable-semantics-v1";
const COMMIT_OID: &str = "637091858d6582fbe7f0c75b7c62d4fd9c2d87ca";
const TREE_OID: &str = "c43f0c6e91c8e3e27abbba94cdd666d6c3598414";
const LOGICAL_METHOD_ID: &str =
    "urn:codenoesis:entity:blake3:9c1ccda385d9531f06c53d1b1f92e5761c73a8eaa8f4b6d149035309bad5a1f4";
const UNIX_ALTERNATIVE_ID: &str =
    "urn:codenoesis:entity:blake3:7693e8447f232a6dc09cc48a62f6551805053fad9a9c2cbadb293b597fe47c75";
const WINDOWS_ALTERNATIVE_ID: &str =
    "urn:codenoesis:entity:blake3:83bf1b64052dafd598f5fc1b02dddfe2d7d790488051d9c3fefaa1f1ca465a88";
const UNIX_SIGNATURE_ID: &str =
    "urn:codenoesis:entity:blake3:f0acfdb5ee201b238adb475a85ec3803fcd599b024d1ab33ed86b3fece011960";
const WINDOWS_SIGNATURE_ID: &str =
    "urn:codenoesis:entity:blake3:d4b3dffd96e9cfbe85e21013950b4e92097bb989853a2731095f84b0d64b79b3";
const FIXTURE_FILES: [(&str, &str); 4] = [
    ("Cargo.toml", "cdf88f6e5c3f0bc5444379612ed5bc21796c8036"),
    ("build.rs", "9ab1798b7d4af3395e972d6838ff07d3e7538375"),
    ("src/lib.rs", "c715d312552e61ed74c8a9a33a04bbdbe9d28354"),
    ("src/model.rs", "91215cbdeba2e2ed0803bfe870fba1da59ebcdfc"),
];

#[test]
fn gt_fr_ext_014_cfg_occurrences_are_k1_callable_subjects() {
    let extraction = extract_fixture(0, false);
    extraction
        .knowledge
        .validate()
        .expect("validate R12 cross-lineage composition");
    assert_eq!(
        extraction.knowledge.index.logical_method_ids,
        [LOGICAL_METHOD_ID]
    );
    assert_eq!(
        extraction.knowledge.index.alternative_callable_subject_ids,
        [UNIX_ALTERNATIVE_ID, WINDOWS_ALTERNATIVE_ID]
    );
    assert_eq!(
        extraction.knowledge.index.signature_ids,
        [UNIX_SIGNATURE_ID, WINDOWS_SIGNATURE_ID]
    );

    let graph = &extraction.knowledge.callable.graph;
    assert!(graph.entities.iter().all(|entity| {
        entity.subject_id != LOGICAL_METHOD_ID
            || !matches!(
                entity.kind,
                CallableSemanticEntityKind::Signature
                    | CallableSemanticEntityKind::Parameter
                    | CallableSemanticEntityKind::LocalBinding
                    | CallableSemanticEntityKind::CallSite
                    | CallableSemanticEntityKind::Control
            )
    }));
    for (alternative_id, signature_id) in [
        (UNIX_ALTERNATIVE_ID, UNIX_SIGNATURE_ID),
        (WINDOWS_ALTERNATIVE_ID, WINDOWS_SIGNATURE_ID),
    ] {
        let signature = graph
            .entities
            .iter()
            .find(|entity| entity.id == signature_id)
            .expect("reviewed R12 signature");
        assert_eq!(signature.subject_id, alternative_id);
        assert!(matches!(
            signature.properties,
            CallableSemanticProperties::Signature(_)
        ));
        assert_eq!(
            graph
                .relationships
                .iter()
                .filter(|relationship| {
                    relationship.kind == CallableRelationshipKind::HasSignature
                        && relationship.source == alternative_id
                        && relationship.target == signature_id
                })
                .count(),
            1
        );
    }
}

#[test]
fn gt_fr_ext_014_callable_counts_match_reviewed_oracle() {
    let graph = &extract_fixture(0, false).knowledge.callable.graph;
    let entity_counts = graph
        .entities
        .iter()
        .fold(BTreeMap::new(), |mut counts, entity| {
            *counts.entry(entity.kind).or_insert(0_usize) += 1;
            counts
        });
    assert_eq!(entity_counts[&CallableSemanticEntityKind::Signature], 10);
    assert_eq!(entity_counts[&CallableSemanticEntityKind::Parameter], 17);
    assert_eq!(
        entity_counts[&CallableSemanticEntityKind::DeclaredValue],
        10
    );
    assert_eq!(entity_counts[&CallableSemanticEntityKind::LocalBinding], 4);
    assert_eq!(entity_counts[&CallableSemanticEntityKind::CallSite], 10);
    assert_eq!(entity_counts[&CallableSemanticEntityKind::Control], 11);

    let relationship_counts =
        graph
            .relationships
            .iter()
            .fold(BTreeMap::new(), |mut counts, relationship| {
                *counts.entry(relationship.kind).or_insert(0_usize) += 1;
                counts
            });
    assert_eq!(
        relationship_counts[&CallableRelationshipKind::HasSignature],
        10
    );
    assert_eq!(
        relationship_counts[&CallableRelationshipKind::HasParameter],
        17
    );
    assert_eq!(
        relationship_counts[&CallableRelationshipKind::DeclaresValue],
        10
    );
    assert_eq!(
        relationship_counts[&CallableRelationshipKind::HasBodyFact],
        25
    );
    assert_eq!(relationship_counts[&CallableRelationshipKind::Calls], 5);
    assert_eq!(graph.index.unresolved_call_site_ids.len(), 5);
}

#[test]
fn gt_real_world_cfg_declared_values_remain_unresolved() {
    let extraction = TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_callable_cfg_alternatives_with_boundaries(
            &fixture_inventory_with_cfg_declared_value_alternatives(),
            &[],
        )
        .expect("extract cfg-conditioned declared values");
    let graph = &extraction.knowledge.callable.graph;
    let values = graph
        .entities
        .iter()
        .filter(|entity| {
            entity.kind == CallableSemanticEntityKind::DeclaredValue
                && entity.name == "PLATFORM_DATA"
        })
        .collect::<Vec<_>>();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].evidence_ids.len(), 2);
    assert!(matches!(
        values[0].properties,
        CallableSemanticProperties::DeclaredValue(ref properties)
            if properties.state == DeclaredValueState::Unresolved
    ));
    let value_gaps = graph
        .coverage
        .iter()
        .filter(|gap| gap.subject_id == values[0].id)
        .collect::<Vec<_>>();
    assert_eq!(value_gaps.len(), 1);
    assert_eq!(
        value_gaps[0].capability,
        "rust.cfg_declared_value_alternatives_not_selected"
    );
    assert_eq!(value_gaps[0].evidence_ids, values[0].evidence_ids);
}

#[test]
fn ft_duplicate_declared_values_without_cfg_remain_invalid() {
    let error = TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_callable_cfg_alternatives_with_boundaries(
            &fixture_inventory_with_duplicate_declared_values(),
            &[],
        )
        .expect_err("reject duplicate declared values without cfg");
    assert!(matches!(
        error,
        CallableCfgAlternativesError::Alternatives(
            RustCfgDeclarationAlternativesError::Source(RustSemanticError::IdentityConflict {
                member_kind,
                normalized_member,
                ..
            })
        ) if member_kind == "rust.constant" && normalized_member == "DUPLICATE_VALUE"
    ));
}

#[test]
fn conf_fr_ext_014_reviewed_fixture_newlines_are_platform_neutral() {
    assert_eq!(
        normalize_reviewed_fixture_bytes(b"#[cfg(unix)]\r\nfn run() {}\r\n"),
        Ok(b"#[cfg(unix)]\nfn run() {}\n".to_vec())
    );
    assert_eq!(
        normalize_reviewed_fixture_bytes(b"#[cfg(unix)]\rfn run() {}\n"),
        Err("reviewed fixture contains a bare carriage return")
    );
}

#[test]
fn ft_fr_ext_014_cross_layer_shape_failures_are_typed() {
    let mut subject_mismatch = extract_fixture(0, false).knowledge;
    subject_mismatch
        .callable
        .graph
        .entities
        .iter_mut()
        .find(|entity| entity.id == UNIX_SIGNATURE_ID)
        .expect("reviewed Unix signature")
        .subject_id = "urn:codenoesis:entity:blake3:subject-mismatch".to_owned();
    assert_eq!(
        subject_mismatch.validate().unwrap_err(),
        CallableCfgAlternativesError::AlternativeSubjectMismatch {
            alternative_id: UNIX_ALTERNATIVE_ID.to_owned(),
            observed_subject_id: "urn:codenoesis:entity:blake3:subject-mismatch".to_owned()
        }
    );

    let mut missing = extract_fixture(0, false).knowledge;
    missing
        .callable
        .graph
        .entities
        .retain(|entity| entity.id != UNIX_SIGNATURE_ID);
    assert_eq!(
        missing.validate().unwrap_err(),
        CallableCfgAlternativesError::AlternativeSignatureCardinality {
            alternative_id: UNIX_ALTERNATIVE_ID.to_owned(),
            observed: 0
        }
    );

    let mut duplicate = extract_fixture(0, false).knowledge;
    let signature = duplicate
        .callable
        .graph
        .entities
        .iter()
        .find(|entity| entity.id == UNIX_SIGNATURE_ID)
        .expect("reviewed Unix signature")
        .clone();
    duplicate.callable.graph.entities.push(signature);
    assert_eq!(
        duplicate.validate().unwrap_err(),
        CallableCfgAlternativesError::AlternativeSignatureCardinality {
            alternative_id: UNIX_ALTERNATIVE_ID.to_owned(),
            observed: 2
        }
    );

    let mut logical_shape = extract_fixture(0, false).knowledge;
    logical_shape
        .callable
        .graph
        .entities
        .iter_mut()
        .find(|entity| entity.id == UNIX_SIGNATURE_ID)
        .expect("reviewed Unix signature")
        .subject_id = LOGICAL_METHOD_ID.to_owned();
    assert_eq!(
        logical_shape.validate().unwrap_err(),
        CallableCfgAlternativesError::LogicalMethodHasOccurrenceShape
    );
}

#[test]
fn ft_fr_ext_014_cross_occurrence_evidence_is_rejected() {
    let mut knowledge = extract_fixture(0, false).knowledge;
    let unix_evidence = knowledge
        .callable
        .graph
        .entities
        .iter()
        .find(|entity| entity.id == UNIX_SIGNATURE_ID)
        .expect("reviewed Unix signature")
        .evidence_ids
        .clone();
    knowledge
        .callable
        .graph
        .entities
        .iter_mut()
        .find(|entity| entity.id == WINDOWS_SIGNATURE_ID)
        .expect("reviewed Windows signature")
        .evidence_ids = unix_evidence;
    assert_eq!(
        knowledge.validate().unwrap_err(),
        CallableCfgAlternativesError::OccurrenceEvidenceMismatch {
            alternative_id: WINDOWS_ALTERNATIVE_ID.to_owned()
        }
    );
}

#[test]
fn pt_nfr_det_001_r12_inventory_order_is_deterministic() {
    let expected = extract_fixture(0, false).knowledge;
    for permutation in 0..50 {
        assert_eq!(
            extract_fixture(permutation, permutation % 2 == 1).knowledge,
            expected
        );
    }
}

#[test]
fn pt_nfr_det_001_r12_parallel_schedules_are_deterministic() {
    let expected = extract_fixture(0, false).knowledge;
    let barrier = Arc::new(Barrier::new(10));
    std::thread::scope(|scope| {
        let handles = (0..10)
            .map(|schedule| {
                let barrier = barrier.clone();
                scope.spawn(move || {
                    barrier.wait();
                    extract_fixture(schedule, schedule % 2 == 1).knowledge
                })
            })
            .collect::<Vec<_>>();
        for handle in handles {
            assert_eq!(handle.join().expect("complete R12 schedule"), expected);
        }
    });
}

fn extract_fixture(
    rotation: usize,
    reverse: bool,
) -> codenoesis_domain::s4_r12::CallableCfgAlternativesExtraction {
    TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_callable_cfg_alternatives_with_boundaries(
            &fixture_inventory(rotation, reverse),
            &[],
        )
        .expect("extract reviewed R12 callable cfg-alternatives fixture")
}

fn fixture_inventory(rotation: usize, reverse: bool) -> RepositoryInventory {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/s4/rust-callable-cfg-alternatives-v1/repository");
    let mut files = FIXTURE_FILES
        .into_iter()
        .map(|(path, blob_oid)| {
            AcquiredFile::new(
                path.to_owned(),
                RegularFileMode::Regular,
                ObjectId::parse_sha1(blob_oid).expect("reviewed R12 blob OID"),
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
            RepositoryIdentity::parse(REPOSITORY_ID).expect("reviewed R12 repository identity"),
            ObjectId::parse_sha1(COMMIT_OID).expect("reviewed R12 commit OID"),
            ObjectId::parse_sha1(TREE_OID).expect("reviewed R12 tree OID"),
        ),
        u64::try_from(files.len()).expect("reviewed R12 file count"),
        files,
    ))
}

fn fixture_inventory_with_cfg_declared_value_alternatives() -> RepositoryInventory {
    fixture_inventory_with_appended_source(
        b"\n#[cfg(windows)]\nconst PLATFORM_DATA: &[u8] = include_bytes!(\"data.bin\");\n#[cfg(not(windows))]\nconst PLATFORM_DATA: &[u8] = &[];\n",
    )
}

fn fixture_inventory_with_duplicate_declared_values() -> RepositoryInventory {
    fixture_inventory_with_appended_source(
        b"\nconst DUPLICATE_VALUE: u8 = 1;\nconst DUPLICATE_VALUE: u8 = 2;\n",
    )
}

fn fixture_inventory_with_appended_source(appended_source: &[u8]) -> RepositoryInventory {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/s4/rust-callable-cfg-alternatives-v1/repository");
    let files = FIXTURE_FILES
        .into_iter()
        .map(|(path, blob_oid)| {
            let mut contents = read_reviewed_fixture(&root.join(path));
            if path == "src/lib.rs" {
                contents.extend_from_slice(appended_source);
            }
            AcquiredFile::new(
                path.to_owned(),
                RegularFileMode::Regular,
                ObjectId::parse_sha1(blob_oid).expect("reviewed R12 blob OID"),
                contents,
            )
        })
        .collect::<Vec<_>>();
    RepositoryInventory::classify(AcquiredRepository::new(
        BoundRevision::new(
            RepositoryIdentity::parse(REPOSITORY_ID).expect("reviewed R12 repository identity"),
            ObjectId::parse_sha1(COMMIT_OID).expect("reviewed R12 commit OID"),
            ObjectId::parse_sha1(TREE_OID).expect("reviewed R12 tree OID"),
        ),
        u64::try_from(files.len()).expect("reviewed R12 file count"),
        files,
    ))
}

fn read_reviewed_fixture(path: &Path) -> Vec<u8> {
    let bytes = fs::read(path).unwrap_or_else(|error| {
        panic!("read reviewed R12 fixture file {}: {error}", path.display())
    });
    normalize_reviewed_fixture_bytes(&bytes).unwrap_or_else(|reason| {
        panic!("invalid reviewed R12 fixture {}: {reason}", path.display())
    })
}

fn normalize_reviewed_fixture_bytes(bytes: &[u8]) -> Result<Vec<u8>, &'static str> {
    let mut normalized = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'\r' {
            normalized.push(bytes[index]);
            index += 1;
            continue;
        }
        if bytes.get(index + 1) != Some(&b'\n') {
            return Err("reviewed fixture contains a bare carriage return");
        }
        normalized.push(b'\n');
        index += 2;
    }
    Ok(normalized)
}
