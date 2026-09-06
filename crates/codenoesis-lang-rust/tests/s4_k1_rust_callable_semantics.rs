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
const UNCERTAINTY_REPOSITORY_ID: &str =
    "urn:codenoesis:fixture:s4-rust-callable-inherited-uncertainty-v1";
const UNCERTAINTY_COMMIT_OID: &str = "f287e22afe2a9800a5a65b4452010d2113f6b6ef";
const UNCERTAINTY_TREE_OID: &str = "98b1dc6aa284c7036996f4176938e0ad9f00e8c9";
const UNCERTAINTY_FIXTURE_FILES: [(&str, &str); 2] = [
    ("Cargo.toml", "67ee2f2696e98ea3abd33a52898f95f7f8872784"),
    ("src/lib.rs", "a50cdb5e934b5249c406d217b8657ad59251cd99"),
];
const IMPORTED_OWNER_REPOSITORY_ID: &str =
    "urn:codenoesis:fixture:s4-rust-callable-imported-owner-v1";
const IMPORTED_OWNER_COMMIT_OID: &str = "b699009b4837fae23891d89b7f435002598921fe";
const IMPORTED_OWNER_TREE_OID: &str = "c87545b7b46be97892042f7b128d35138de61712";
const IMPORTED_OWNER_FIXTURE_FILES: [(&str, &str); 2] = [
    ("Cargo.toml", "b4b1a880da1a90ba945251b49900a913b8dcd529"),
    ("src/lib.rs", "0f5d95ecbbba42504a2596a555cf13aeada8b216"),
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
fn gt_fr_ext_012_oversized_declared_value_is_covered_without_retaining_metadata() {
    let source = format!("pub const LARGE: &str = \"{}\";", "a".repeat(4_097));
    let extraction = TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_callable_semantics(&synthetic_inventory(&source))
        .expect("cover oversized declared value");
    let entity = extraction
        .knowledge
        .graph
        .entities
        .iter()
        .find(|entity| entity.name == "LARGE")
        .expect("large declared value entity");
    let CallableSemanticProperties::DeclaredValue(properties) = &entity.properties else {
        panic!("declared value properties");
    };
    assert_eq!(properties.state, DeclaredValueState::ExpressionOnly);
    assert!(properties.expression_digest.is_none());
    assert!(properties.expression_byte_length > 4_096);
    assert!(extraction.knowledge.graph.coverage.iter().any(|gap| {
        gap.subject_id == entity.id && gap.capability == "rust.expression_metadata_too_large"
    }));
}

#[test]
fn gt_fr_ext_012_attribute_transformed_overloads_remain_explicitly_unmodeled() {
    let source = r"
pub struct BindGroup;

impl BindGroup {
    #[getter]
    fn label(&self) -> String { String::new() }

    #[setter]
    fn label(&self, value: String) { let _ = value; }
}
";
    let extraction = TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_callable_cfg_alternatives(&synthetic_inventory(source), &[])
        .expect("preserve macro-transformed overload uncertainty through R12");
    assert!(
        extraction
            .knowledge
            .callable
            .graph
            .entities
            .iter()
            .all(|entity| {
                entity.kind != CallableSemanticEntityKind::Signature || entity.name != "label"
            })
    );
    assert!(
        extraction
            .knowledge
            .callable
            .framework
            .semantic
            .graph
            .coverage
            .iter()
            .any(|gap| gap.capability == "rust.attribute_semantics_not_interpreted")
    );
}

#[test]
fn gt_fr_ext_012_cfg_free_function_alternatives_do_not_claim_one_signature() {
    let source = r#"
#[cfg(target_os = "ios")]
#[component]
fn app(value: String) {}

#[cfg(not(target_os = "ios"))]
#[component]
fn app(value: u64) {}
"#;
    let extraction = TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_callable_cfg_alternatives(&synthetic_inventory(source), &[])
        .expect("preserve free-function cfg uncertainty through R12");
    assert!(
        extraction
            .knowledge
            .callable
            .graph
            .entities
            .iter()
            .all(|entity| {
                entity.kind != CallableSemanticEntityKind::Signature || entity.name != "app"
            })
    );
    assert!(
        extraction
            .knowledge
            .callable
            .framework
            .semantic
            .graph
            .coverage
            .iter()
            .any(|gap| gap.capability == "rust.cfg_presence_unresolved")
    );
    let component = extraction
        .knowledge
        .callable
        .framework
        .graph
        .declarations
        .iter()
        .find(|declaration| declaration.declared_key_or_target.ends_with(" -> app"))
        .expect("component candidate");
    assert_eq!(component.evidence_ids.len(), 2);
}

#[test]
fn gt_fr_ext_012_repeated_anonymous_constants_do_not_claim_one_value() {
    let extraction = TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_callable_cfg_alternatives(
            &synthetic_inventory("const _: () = ();\nconst _: () = { panic!() };\n"),
            &[],
        )
        .expect("preserve repeated anonymous constant uncertainty through R12");
    let values = extraction
        .knowledge
        .callable
        .graph
        .entities
        .iter()
        .filter(|entity| entity.kind == CallableSemanticEntityKind::DeclaredValue)
        .collect::<Vec<_>>();
    assert_eq!(values.len(), 1);
    let CallableSemanticProperties::DeclaredValue(properties) = &values[0].properties else {
        panic!("declared value properties");
    };
    assert_eq!(properties.state, DeclaredValueState::Unresolved);
    assert_eq!(values[0].evidence_ids.len(), 2);
    assert!(
        extraction
            .knowledge
            .callable
            .graph
            .coverage
            .iter()
            .any(|gap| {
                gap.subject_id == values[0].id
                    && gap.capability
                        == "rust.anonymous_declared_value_occurrences_not_distinguished"
            })
    );
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

#[test]
fn reg_fr_ext_012_k1_preserves_inherited_uncertainty() {
    let extraction = TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_callable_semantics(&uncertainty_fixture_inventory())
        .expect("K1 must preserve inherited uncertainty without an internal failure");
    extraction
        .knowledge
        .validate()
        .expect("reviewed inherited-uncertainty graph");
    let signature_names = extraction
        .knowledge
        .graph
        .entities
        .iter()
        .filter(|entity| entity.kind == CallableSemanticEntityKind::Signature)
        .fold(BTreeMap::new(), |mut counts, entity| {
            *counts.entry(entity.name.as_str()).or_insert(0_usize) += 1;
            counts
        });
    assert_eq!(
        signature_names,
        BTreeMap::from([("inherent", 1), ("known", 1), ("local", 2)])
    );
    for excluded in [
        "gated",
        "hidden_test",
        "unresolved_external_target",
        "unresolved_external_trait",
    ] {
        assert!(
            extraction
                .knowledge
                .graph
                .entities
                .iter()
                .all(|entity| entity.name != excluded),
            "K1 synthesized an absent inherited callable: {excluded}"
        );
    }
    let inherited_coverage = extraction
        .knowledge
        .framework
        .semantic
        .graph
        .coverage
        .iter()
        .map(|gap| gap.capability.as_str())
        .collect::<BTreeSet<_>>();
    assert!(inherited_coverage.contains("rust.cfg_presence_unresolved"));
    assert!(inherited_coverage.contains("rust.unsupported_impl_header"));
}

#[test]
fn reg_fr_ext_012_k1_matches_inherited_imported_owner_resolution() {
    let extraction = TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_callable_semantics(&imported_owner_fixture_inventory())
        .expect("K1 must reuse the inherited unique local owner resolution");
    extraction
        .knowledge
        .validate()
        .expect("reviewed imported-owner graph");
    let signature_names = extraction
        .knowledge
        .graph
        .entities
        .iter()
        .filter(|entity| entity.kind == CallableSemanticEntityKind::Signature)
        .fold(BTreeMap::new(), |mut counts, entity| {
            *counts.entry(entity.name.as_str()).or_insert(0_usize) += 1;
            counts
        });
    assert_eq!(
        signature_names,
        BTreeMap::from([("inherent", 1), ("local", 2)])
    );
}

#[test]
fn sec_fr_knw_003_complex_call_targets_are_bounded_without_changing_simple_targets() {
    let source = r#"
pub fn calls(value: Client) {
    simple_target();
    module::target();
    simple_target::<i32>();
    value.field.send();
    factory("https://fixture.invalid").send();
    (factory)("https://fixture.invalid");
}
"#;
    let extraction = TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_callable_semantics(&synthetic_inventory(source))
        .expect("extract bounded K1 call targets");
    extraction
        .knowledge
        .validate()
        .expect("validate bounded K1 call targets");
    let spellings = extraction
        .knowledge
        .graph
        .entities
        .iter()
        .filter_map(|entity| match &entity.properties {
            CallableSemanticProperties::CallSite(properties) => {
                Some(properties.target_spelling.as_str())
            }
            _ => None,
        })
        .fold(BTreeMap::new(), |mut counts, spelling| {
            *counts.entry(spelling).or_insert(0_usize) += 1;
            counts
        });
    assert_eq!(
        spellings,
        BTreeMap::from([
            ("<unsupported-call-target>", 1),
            ("<unsupported-receiver>.send", 1),
            ("factory", 1),
            ("module::target", 1),
            ("simple_target", 1),
            ("simple_target::<i32>", 1),
            ("value.field.send", 1),
        ])
    );
    assert!(
        extraction
            .knowledge
            .graph
            .entities
            .iter()
            .all(|entity| !entity.name.contains("://"))
    );
}

#[test]
fn gt_rw2_k1_classifies_each_if_from_its_own_condition() {
    let source = r"
pub fn classify(value: Option<i32>, enabled: bool) -> i32 {
    if enabled {
        1
    } else if let Some(value) = value {
        value
    } else {
        0
    }
}
";
    let extraction = TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_callable_semantics(&synthetic_inventory(source))
        .expect("extract nested conditional controls");
    let controls = extraction
        .knowledge
        .graph
        .entities
        .iter()
        .filter_map(|entity| match &entity.properties {
            CallableSemanticProperties::Control(properties) => Some(properties.control_kind),
            _ => None,
        })
        .fold(BTreeMap::new(), |mut counts, kind| {
            *counts.entry(kind).or_insert(0_usize) += 1;
            counts
        });

    assert_eq!(controls[&ControlKind::If], 1);
    assert_eq!(controls[&ControlKind::IfLet], 1);
}

#[test]
fn gt_fr_ext_014_macro_pattern_input_remains_explicitly_unexpanded() {
    let extraction = TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_expression_bindings(&synthetic_inventory(
            "pub fn inspect() {\n\
             if let Some(value) = deferred_input!() {\n\
             let _ = value;\n\
             }\n\
             }\n",
        ))
        .expect("macro pattern input is a typed coverage gap");
    assert!(
        extraction
            .knowledge
            .graph
            .coverage
            .iter()
            .any(|gap| { gap.capability == "rust.pattern_input_unexpanded" })
    );
}

#[test]
fn sec_rw2_k1_url_scalar_keeps_digest_without_raw_value() {
    let source = r#"
pub const DOCUMENTATION: &str = "https://example.invalid/private";
"#;
    let extraction = TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_callable_semantics(&synthetic_inventory(source))
        .expect("extract URL-bearing declared value");
    let properties = extraction
        .knowledge
        .graph
        .entities
        .iter()
        .find_map(|entity| match &entity.properties {
            CallableSemanticProperties::DeclaredValue(properties)
                if entity.name == "DOCUMENTATION" =>
            {
                Some(properties)
            }
            _ => None,
        })
        .expect("URL-bearing declared value");

    assert_eq!(properties.state, DeclaredValueState::ExpressionOnly);
    assert!(properties.normalized.is_none());
    assert!(properties.expression_digest.is_some());
    assert!(!format!("{:?}", extraction.knowledge).contains("https://"));
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

fn uncertainty_fixture_inventory() -> RepositoryInventory {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/s4/rust-callable-inherited-uncertainty-v1/repository");
    let files = UNCERTAINTY_FIXTURE_FILES
        .into_iter()
        .map(|(path, blob_oid)| {
            AcquiredFile::new(
                path.to_owned(),
                RegularFileMode::Regular,
                ObjectId::parse_sha1(blob_oid).expect("reviewed uncertainty fixture blob OID"),
                read_reviewed_fixture(&root.join(path)),
            )
        })
        .collect::<Vec<_>>();
    RepositoryInventory::classify(AcquiredRepository::new(
        BoundRevision::new(
            RepositoryIdentity::parse(UNCERTAINTY_REPOSITORY_ID)
                .expect("reviewed uncertainty repository identity"),
            ObjectId::parse_sha1(UNCERTAINTY_COMMIT_OID).expect("reviewed uncertainty commit OID"),
            ObjectId::parse_sha1(UNCERTAINTY_TREE_OID).expect("reviewed uncertainty tree OID"),
        ),
        u64::try_from(files.len()).expect("reviewed uncertainty file count"),
        files,
    ))
}

fn imported_owner_fixture_inventory() -> RepositoryInventory {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../tests/fixtures/s4/rust-callable-inherited-uncertainty-v1/imported-owner-repository",
    );
    let files = IMPORTED_OWNER_FIXTURE_FILES
        .into_iter()
        .map(|(path, blob_oid)| {
            AcquiredFile::new(
                path.to_owned(),
                RegularFileMode::Regular,
                ObjectId::parse_sha1(blob_oid).expect("reviewed imported-owner fixture blob OID"),
                read_reviewed_fixture(&root.join(path)),
            )
        })
        .collect::<Vec<_>>();
    RepositoryInventory::classify(AcquiredRepository::new(
        BoundRevision::new(
            RepositoryIdentity::parse(IMPORTED_OWNER_REPOSITORY_ID)
                .expect("reviewed imported-owner repository identity"),
            ObjectId::parse_sha1(IMPORTED_OWNER_COMMIT_OID)
                .expect("reviewed imported-owner commit OID"),
            ObjectId::parse_sha1(IMPORTED_OWNER_TREE_OID)
                .expect("reviewed imported-owner tree OID"),
        ),
        u64::try_from(files.len()).expect("reviewed imported-owner file count"),
        files,
    ))
}

fn synthetic_inventory(source: &str) -> RepositoryInventory {
    let files = vec![
        AcquiredFile::new(
            "Cargo.toml".to_owned(),
            RegularFileMode::Regular,
            synthetic_oid('c'),
            b"[package]\nname=\"k1-correction\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[lib]\npath=\"src/lib.rs\"\n"
                .to_vec(),
        ),
        AcquiredFile::new(
            "src/lib.rs".to_owned(),
            RegularFileMode::Regular,
            synthetic_oid('d'),
            source.as_bytes().to_vec(),
        ),
    ];
    RepositoryInventory::classify(AcquiredRepository::new(
        BoundRevision::new(
            RepositoryIdentity::parse("urn:codenoesis:test:k1-call-target-correction")
                .expect("synthetic K1 repository identity"),
            synthetic_oid('a'),
            synthetic_oid('b'),
        ),
        u64::try_from(files.len()).expect("synthetic K1 file count"),
        files,
    ))
}

fn synthetic_oid(value: char) -> ObjectId {
    ObjectId::parse_sha1(&value.to_string().repeat(40)).expect("synthetic SHA-1 object ID")
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
