use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Barrier};

use codenoesis_domain::s4_r14::{
    BindingModifier, BindingOrigin, ExpressionBindingError, ExpressionBindingKnowledge,
    ExpressionEntityKind, ExpressionEntityProperties, ExpressionRelationshipKind, ExpressionRole,
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
fn gt_fr_ext_016_reviewed_fixture_matches_expression_binding_oracle() {
    let extraction = extract_fixture(0, false);
    extraction
        .knowledge
        .validate()
        .expect("validate reviewed R14 expression bindings");
    let graph = &extraction.knowledge.graph;

    let entity_counts = graph
        .entities
        .iter()
        .fold(BTreeMap::new(), |mut counts, entity| {
            *counts.entry(entity.kind).or_insert(0_usize) += 1;
            counts
        });
    assert_eq!(entity_counts[&ExpressionEntityKind::Expression], 73);
    assert_eq!(entity_counts[&ExpressionEntityKind::CallArgument], 8);
    assert_eq!(entity_counts[&ExpressionEntityKind::PatternBinding], 23);

    let relationship_counts =
        graph
            .relationships
            .iter()
            .fold(BTreeMap::new(), |mut counts, relationship| {
                *counts.entry(relationship.kind).or_insert(0_usize) += 1;
                counts
            });
    for (kind, expected) in [
        (ExpressionRelationshipKind::HasExpression, 73),
        (ExpressionRelationshipKind::ContainsExpression, 38),
        (ExpressionRelationshipKind::HasArgument, 8),
        (ExpressionRelationshipKind::ArgumentValue, 8),
        (ExpressionRelationshipKind::HasReceiver, 4),
        (ExpressionRelationshipKind::RepresentsCallSite, 9),
        (ExpressionRelationshipKind::DeclaresBinding, 23),
        (ExpressionRelationshipKind::BindsFrom, 8),
        (ExpressionRelationshipKind::Reads, 29),
        (ExpressionRelationshipKind::Writes, 7),
    ] {
        assert_eq!(relationship_counts[&kind], expected, "{kind:?}");
    }
    let baseline_evidence = extraction
        .knowledge
        .callable
        .graph
        .evidence
        .iter()
        .map(|evidence| evidence.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(graph.evidence.len(), 96);
    assert_eq!(
        graph
            .evidence
            .iter()
            .filter(|evidence| !baseline_evidence.contains(evidence.id.as_str()))
            .count(),
        86
    );
    assert_eq!(graph.claims.len(), 311);
    assert!(graph.coverage.is_empty());
    assert_eq!(extraction.knowledge.extraction_chunks.len(), 2);

    let mut depths = BTreeMap::new();
    let mut origins = BTreeMap::new();
    let mut modifiers = BTreeMap::new();
    for entity in &graph.entities {
        match &entity.properties {
            ExpressionEntityProperties::Expression(properties) => {
                *depths.entry(properties.lexical_depth).or_insert(0_usize) += 1;
            }
            ExpressionEntityProperties::PatternBinding(properties) => {
                *origins.entry(properties.origin).or_insert(0_usize) += 1;
                *modifiers.entry(properties.modifier).or_insert(0_usize) += 1;
            }
            ExpressionEntityProperties::CallArgument(_) => {}
        }
    }
    assert_eq!(depths, BTreeMap::from([(0, 35), (1, 32), (2, 6)]));
    assert_eq!(origins[&BindingOrigin::Parameter], 15);
    assert_eq!(origins[&BindingOrigin::LocalLet], 4);
    assert_eq!(origins[&BindingOrigin::IfLet], 1);
    assert_eq!(origins[&BindingOrigin::WhileLet], 1);
    assert_eq!(origins[&BindingOrigin::For], 1);
    assert_eq!(origins[&BindingOrigin::MatchArm], 1);
    assert_eq!(modifiers[&BindingModifier::ExplicitMut], 2);
    assert_eq!(modifiers[&BindingModifier::None], 21);
}

#[test]
fn gt_fr_ext_016_unsupported_syntax_emits_coverage_without_guessed_facts() {
    let extraction = TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_expression_bindings(&unsupported_inventory())
        .expect("extract R14 unsupported-syntax coverage");
    extraction
        .knowledge
        .validate()
        .expect("validate R14 unsupported-syntax coverage");
    let coverage = &extraction.knowledge.graph.coverage;
    assert!(coverage.iter().any(|gap| {
        gap.capability == "rust.expression_nested_callable" && gap.state == "unsupported"
    }));
    assert!(coverage.iter().any(|gap| {
        gap.capability == "rust.expression_closure_capture" && gap.state == "unsupported"
    }));
    assert!(
        coverage
            .iter()
            .any(|gap| gap.capability == "rust.pattern_binding" && gap.state == "unsupported")
    );
    assert!(coverage.iter().any(|gap| {
        gap.capability == "rust.lexical_binding_shadowing" && gap.state == "unsupported"
    }));
}

#[test]
#[allow(clippy::too_many_lines)]
fn ft_fr_ext_016_corrupt_expression_graph_fails_with_exact_typed_errors() {
    assert_invalid_knowledge(
        |knowledge| {
            let duplicate = knowledge.graph.entities[0].clone();
            knowledge.graph.entities.insert(1, duplicate);
        },
        ExpressionBindingError::IdentityConflict,
    );
    assert_invalid_knowledge(
        |knowledge| {
            let properties = knowledge
                .graph
                .entities
                .iter_mut()
                .find_map(|entity| match &mut entity.properties {
                    ExpressionEntityProperties::Expression(properties)
                        if properties.parent_expression_id.is_some() =>
                    {
                        Some(properties)
                    }
                    _ => None,
                })
                .expect("R14 nested expression");
            properties.parent_expression_id = None;
        },
        ExpressionBindingError::ParentInvalid,
    );
    assert_invalid_knowledge(
        |knowledge| {
            let properties = knowledge
                .graph
                .entities
                .iter_mut()
                .find_map(|entity| match &mut entity.properties {
                    ExpressionEntityProperties::Expression(properties) => Some(properties),
                    _ => None,
                })
                .expect("R14 expression");
            properties.roles = vec![ExpressionRole::Nested, ExpressionRole::Nested];
        },
        ExpressionBindingError::RoleInvalid,
    );
    assert_invalid_knowledge(
        |knowledge| {
            let properties = knowledge
                .graph
                .entities
                .iter_mut()
                .find_map(|entity| match &mut entity.properties {
                    ExpressionEntityProperties::Expression(properties)
                        if properties.syntax_kind == "binary_expression" =>
                    {
                        Some(properties)
                    }
                    _ => None,
                })
                .expect("R14 binary expression");
            properties.operator = Some("??".to_owned());
        },
        ExpressionBindingError::OperatorInvalid,
    );
    assert_invalid_knowledge(
        |knowledge| {
            let properties = knowledge
                .graph
                .entities
                .iter_mut()
                .find_map(|entity| match &mut entity.properties {
                    ExpressionEntityProperties::CallArgument(properties) => Some(properties),
                    _ => None,
                })
                .expect("R14 call argument");
            properties.ordinal = properties.ordinal.saturating_add(1);
        },
        ExpressionBindingError::ArgumentOrdinalInvalid,
    );
    assert_invalid_knowledge(
        |knowledge| {
            let properties = knowledge
                .graph
                .entities
                .iter_mut()
                .find_map(|entity| match &mut entity.properties {
                    ExpressionEntityProperties::PatternBinding(properties) => Some(properties),
                    _ => None,
                })
                .expect("R14 pattern binding");
            properties.scope_start_byte = properties.scope_end_byte.saturating_add(1);
        },
        ExpressionBindingError::BindingScopeInvalid,
    );
    assert_invalid_knowledge(
        |knowledge| {
            let relationship_index = knowledge
                .graph
                .relationships
                .iter()
                .position(|relationship| relationship.kind == ExpressionRelationshipKind::Reads)
                .expect("R14 read relationship");
            let unrelated = knowledge
                .graph
                .evidence
                .iter()
                .find(|evidence| {
                    !knowledge.graph.relationships[relationship_index]
                        .evidence_ids
                        .contains(&evidence.id)
                })
                .expect("unrelated R14 read evidence")
                .id
                .clone();
            knowledge.graph.relationships[relationship_index].evidence_ids = vec![unrelated];
        },
        ExpressionBindingError::AccessResolutionInvalid,
    );
    assert_invalid_knowledge(
        |knowledge| {
            let relationship = knowledge
                .graph
                .relationships
                .iter_mut()
                .find(|relationship| {
                    relationship.kind == ExpressionRelationshipKind::RepresentsCallSite
                })
                .expect("R14 call-site relationship");
            let unrelated = knowledge
                .graph
                .evidence
                .iter()
                .find(|evidence| !relationship.evidence_ids.contains(&evidence.id))
                .expect("unrelated R14 evidence")
                .id
                .clone();
            relationship.evidence_ids = vec![unrelated];
        },
        ExpressionBindingError::CallSiteEvidenceMismatch,
    );
    assert_invalid_knowledge(
        |knowledge| knowledge.graph.index.expression_entity_ids.clear(),
        ExpressionBindingError::IndexMismatch,
    );
}

#[test]
fn pt_nfr_det_001_r14_inventory_order_is_deterministic() {
    let expected = extract_fixture(0, false).knowledge;
    for permutation in 0..50 {
        assert_eq!(
            extract_fixture(permutation, permutation % 2 == 1).knowledge,
            expected,
            "R14 inventory permutation {permutation}"
        );
    }
}

#[test]
fn pt_nfr_det_001_r14_parallel_schedules_are_deterministic() {
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
        for (schedule, handle) in handles.into_iter().enumerate() {
            assert_eq!(
                handle.join().expect("complete R14 schedule"),
                expected,
                "R14 parallel schedule {schedule}"
            );
        }
    });
}

#[test]
fn gt_fr_ext_016_partial_real_syntax_is_omitted_as_complete_families() {
    let source = r"
pub fn partial(value: i32) -> i32 {
    let direct = simple_target(value);
    let mixed = consume(value, || value);
    let opaque = client_factory!().send(value);
    let chosen = match value { 0 => direct, _ => opaque };
    direct + mixed + opaque + chosen
}

#[cfg(test)]
pub fn test_only(value: i32) -> i32 { simple_target(value) }
";
    let extraction = TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_expression_bindings(&correction_inventory(source))
        .expect("extract fail-closed R14 syntax");
    extraction
        .knowledge
        .validate()
        .expect("validate fail-closed R14 syntax");
    let graph = &extraction.knowledge.graph;

    let mixed = expression_id_for(source, "consume(value, || value)", graph);
    assert!(graph.relationships.iter().all(|relationship| {
        relationship.kind != ExpressionRelationshipKind::HasArgument || relationship.source != mixed
    }));

    let opaque = expression_id_for(source, "client_factory!().send(value)", graph);
    assert!(graph.relationships.iter().all(|relationship| {
        relationship.kind != ExpressionRelationshipKind::HasReceiver
            || relationship.source != opaque
    }));

    let chosen = graph
        .entities
        .iter()
        .find(|entity| {
            entity.kind == ExpressionEntityKind::PatternBinding && entity.name == "chosen"
        })
        .expect("preserved match binding");
    assert!(graph.relationships.iter().all(|relationship| {
        relationship.kind != ExpressionRelationshipKind::BindsFrom
            || relationship.source != chosen.id
    }));
    assert!(
        graph
            .coverage
            .iter()
            .any(|gap| gap.capability == "rust.pattern_input_unexpanded")
    );

    let test_only_start = u64::try_from(source.find("pub fn test_only").expect("test-only source"))
        .expect("test-only offset");
    assert!(
        graph
            .entities
            .iter()
            .all(|entity| entity.locator.start_byte < test_only_start)
    );
}

fn extract_fixture(
    rotation: usize,
    reverse: bool,
) -> codenoesis_domain::s4_r14::ExpressionBindingExtraction {
    TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_expression_bindings(&fixture_inventory(rotation, reverse))
        .expect("extract reviewed R14 expression-binding fixture")
}

fn assert_invalid_knowledge(
    mutate: impl FnOnce(&mut ExpressionBindingKnowledge),
    expected: ExpressionBindingError,
) {
    let mut knowledge = extract_fixture(0, false).knowledge;
    mutate(&mut knowledge);
    assert_eq!(knowledge.validate(), Err(expected));
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
                ObjectId::parse_sha1(blob_oid).expect("reviewed R14 blob OID"),
                fs::read(root.join(path)).expect("read reviewed R14 fixture"),
            )
        })
        .collect::<Vec<_>>();
    let length = files.len();
    files.rotate_left(rotation % length);
    if reverse {
        files.reverse();
    }
    inventory(REPOSITORY_ID, COMMIT_OID, TREE_OID, files)
}

fn unsupported_inventory() -> RepositoryInventory {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/s4/rust-callable-semantics-v1/repository");
    let files = FIXTURE_FILES
        .into_iter()
        .map(|(path, blob_oid)| {
            let mut bytes = fs::read(root.join(path)).expect("read reviewed R14 fixture");
            if path == "src/lib.rs" {
                bytes.extend_from_slice(
                    br"
pub fn unsupported_r14(input: Option<i32>) {
    fn nested(value: i32) -> i32 { value }
    let closure = || input;
    let [first, ..] = [1, 2];
    let (duplicate, duplicate) = (1, 2);
    let _ambiguous = duplicate;
    let _value = (first, nested(1));
}
",
                );
            }
            AcquiredFile::new(
                path.to_owned(),
                RegularFileMode::Regular,
                ObjectId::parse_sha1(blob_oid).expect("reviewed R14 blob OID"),
                bytes,
            )
        })
        .collect();
    inventory(REPOSITORY_ID, COMMIT_OID, TREE_OID, files)
}

fn correction_inventory(source: &str) -> RepositoryInventory {
    inventory(
        "urn:codenoesis:test:r14-real-syntax-correction",
        &"a".repeat(40),
        &"b".repeat(40),
        vec![
            AcquiredFile::new(
                "Cargo.toml".to_owned(),
                RegularFileMode::Regular,
                ObjectId::parse_sha1(&"c".repeat(40)).expect("synthetic R14 manifest blob"),
                b"[package]\nname=\"r14-correction\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[lib]\npath=\"src/lib.rs\"\n"
                    .to_vec(),
            ),
            AcquiredFile::new(
                "src/lib.rs".to_owned(),
                RegularFileMode::Regular,
                ObjectId::parse_sha1(&"d".repeat(40)).expect("synthetic R14 source blob"),
                source.as_bytes().to_vec(),
            ),
        ],
    )
}

fn expression_id_for(
    source: &str,
    snippet: &str,
    graph: &codenoesis_domain::s4_r14::ExpressionBindingGraph,
) -> String {
    let start = source.find(snippet).expect("reviewed expression snippet");
    let end = start + snippet.len();
    graph
        .entities
        .iter()
        .find(|entity| {
            entity.kind == ExpressionEntityKind::Expression
                && usize::try_from(entity.locator.start_byte).ok() == Some(start)
                && usize::try_from(entity.locator.end_byte).ok() == Some(end)
        })
        .expect("reviewed expression entity")
        .id
        .clone()
}

fn inventory(
    repository_id: &str,
    commit_oid: &str,
    tree_oid: &str,
    files: Vec<AcquiredFile>,
) -> RepositoryInventory {
    RepositoryInventory::classify(AcquiredRepository::new(
        BoundRevision::new(
            RepositoryIdentity::parse(repository_id).expect("R14 repository identity"),
            ObjectId::parse_sha1(commit_oid).expect("R14 commit OID"),
            ObjectId::parse_sha1(tree_oid).expect("R14 tree OID"),
        ),
        u64::try_from(files.len()).expect("R14 file count"),
        files,
    ))
}
