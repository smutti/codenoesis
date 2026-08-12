use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use codenoesis_domain::s4_r14::ExpressionRelationshipKind;
use codenoesis_domain::s4_r15::{
    LocalFlowError, LocalFlowLimit, LocalFlowRelationshipKind, R15_RULE_VERSION,
};
use codenoesis_domain::{
    AcquiredFile, AcquiredRepository, BoundRevision, ObjectId, RegularFileMode, RepositoryIdentity,
    RepositoryInventory,
};
use codenoesis_lang_rust::TreeSitterRustWorkspaceExtractor;

const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s4-rust-local-flow-v1";
const COMMIT_OID: &str = "552a8cdc76b2dd80dc26ad1e3b381fc0de9eab24";
const TREE_OID: &str = "6a97e7b14d29db6aa50416fd9ac76b9022298104";
const FIXTURE_FILES: [(&str, &str); 2] = [
    ("Cargo.toml", "c8803b72ed28064208e7f3f95958b3f10c90f25b"),
    ("src/lib.rs", "36be3c129a00aee60e6c312fdb4de133ad57b7bb"),
];

#[test]
fn gt_fr_ext_017_reviewed_fixture_matches_closed_local_flow_oracle() {
    let extraction = TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_local_flow(&fixture_inventory())
        .expect("extract reviewed R15 local flow");
    extraction
        .knowledge
        .validate()
        .expect("validate reviewed R15 local flow");
    let graph = &extraction.knowledge.graph;
    assert_eq!(graph.blocks.len(), 5);
    assert_eq!(graph.relationships.len(), 36);
    assert_eq!(graph.claims.len(), 41);
    assert_eq!(graph.evidence.len(), 5);
    assert!(graph.coverage.is_empty());
    assert_eq!(graph.index.completed_callable_ids.len(), 1);
    assert_eq!(graph.index.derivations.len(), 16);

    let counts = graph
        .relationships
        .iter()
        .fold(BTreeMap::new(), |mut counts, relationship| {
            *counts.entry(relationship.kind).or_insert(0_usize) += 1;
            counts
        });
    for (kind, expected) in [
        (LocalFlowRelationshipKind::HasSyntaxBlock, 5),
        (LocalFlowRelationshipKind::ContainsFlowNode, 9),
        (LocalFlowRelationshipKind::HasCondition, 1),
        (LocalFlowRelationshipKind::SyntaxNext, 3),
        (LocalFlowRelationshipKind::SyntaxTrueBranch, 1),
        (LocalFlowRelationshipKind::SyntaxFalseBranch, 1),
        (LocalFlowRelationshipKind::SyntaxReaches, 9),
        (LocalFlowRelationshipKind::LexicalMustReachesRead, 5),
        (LocalFlowRelationshipKind::LexicalMayReachesRead, 2),
    ] {
        assert_eq!(counts[&kind], expected, "{kind:?}");
    }
    assert!(
        graph
            .index
            .derivations
            .iter()
            .all(|derivation| !derivation.input_entity_ids.is_empty())
    );
    assert_eq!(R15_RULE_VERSION, "codenoesis.rule/rust-local-flow/v1");
}

#[test]
#[allow(clippy::too_many_lines)]
fn ft_fr_ext_017_loop_and_missing_else_reject_the_whole_callable() {
    for (case, source) in [
        (
            "loop",
            "pub fn looped(value: i32) -> i32 { loop { break; } value }\n",
        ),
        (
            "while",
            "pub fn while_loop(mut value: i32) -> i32 { while value > 0 { value = value - 1; } value }\n",
        ),
        (
            "for",
            "pub fn for_loop(value: i32) -> i32 { for item in [value] { item; } value }\n",
        ),
        (
            "match",
            "pub fn matched(value: i32) -> i32 { match value { _ => value }; value }\n",
        ),
        (
            "return",
            "pub fn returned(value: i32) -> i32 { return value; }\n",
        ),
        (
            "try",
            "pub fn tried(value: i32) -> i32 { consume(value)?; value }\n",
        ),
        (
            "await",
            "pub async fn awaited(value: i32) -> i32 { value.await; value }\n",
        ),
        (
            "closure",
            "pub fn closure(value: i32) -> i32 { let captured = || value; captured(); value }\n",
        ),
        (
            "macro",
            "pub fn macro_call(value: i32) -> i32 { consume!(value); value }\n",
        ),
        (
            "missing_else",
            "pub fn missing_else(mut value: i32, enabled: bool) -> i32 { if enabled { value = value + 1; } value }\n",
        ),
        (
            "empty_branch",
            "pub fn empty_branch(value: i32, enabled: bool) -> i32 { if enabled {} else { value; } value }\n",
        ),
        (
            "uninitialized",
            "pub fn uninitialized(value: i32) -> i32 { let other: i32; value }\n",
        ),
        (
            "destructuring",
            "pub fn destructuring(value: i32) -> i32 { let (first, second) = (value, value); first + second }\n",
        ),
        (
            "indirect_write",
            "pub fn indirect(mut state: State, value: i32) -> i32 { state.value = value; value }\n",
        ),
        (
            "nested",
            "pub fn nested(value: i32) -> i32 { fn inner(value: i32) -> i32 { value } inner(value) }\n",
        ),
        (
            "unsafe",
            "pub fn unsafe_block(value: i32) -> i32 { unsafe { value; } value }\n",
        ),
        (
            "const",
            "pub fn const_block(value: i32) -> i32 { const { 1 }; value }\n",
        ),
        (
            "cfg",
            "pub fn direct_cfg(value: i32) -> i32 { #[cfg(test)] value; value }\n",
        ),
    ] {
        let extraction = TreeSitterRustWorkspaceExtractor::new()
            .extract_rust_local_flow(&inventory_with_source(source))
            .expect("extract unsupported whole-callable R15 source");
        extraction
            .knowledge
            .validate()
            .expect("validate unsupported whole-callable result");
        let graph = &extraction.knowledge.graph;
        assert!(
            graph.blocks.is_empty(),
            "{case}: {:?}",
            extraction.knowledge.expression.graph.coverage
        );
        assert!(graph.relationships.is_empty(), "{case}");
        assert!(graph.claims.is_empty(), "{case}");
        assert!(graph.evidence.is_empty(), "{case}");
        assert!(graph.index.completed_callable_ids.is_empty(), "{case}");
        assert_eq!(graph.coverage.len(), 2, "{case}");
        assert_eq!(
            graph
                .coverage
                .iter()
                .map(|gap| (gap.capability.as_str(), gap.state.as_str()))
                .collect::<BTreeMap<_, _>>(),
            BTreeMap::from([
                (
                    "rust.lexical_reaching_definitions_not_analyzed",
                    "unsupported",
                ),
                ("rust.syntax_normal_flow_not_analyzed", "unsupported"),
            ])
        );
    }
}

#[test]
fn ft_fr_ext_017_validator_rejects_missing_branch_and_wrong_r14_derivation_input() {
    let extraction = TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_local_flow(&fixture_inventory())
        .expect("extract reviewed R15 validation fixture");

    let mut missing_branch = extraction.knowledge.clone();
    let branch_index = missing_branch
        .graph
        .relationships
        .iter()
        .position(|relationship| relationship.kind == LocalFlowRelationshipKind::SyntaxTrueBranch)
        .expect("reviewed true branch");
    missing_branch.graph.relationships.remove(branch_index);
    assert_eq!(missing_branch.validate(), Err(LocalFlowError::EdgeInvalid));

    let mut wrong_input = extraction.knowledge;
    let lexical_relationship = wrong_input
        .graph
        .relationships
        .iter()
        .find(|relationship| {
            matches!(
                relationship.kind,
                LocalFlowRelationshipKind::LexicalMustReachesRead
                    | LocalFlowRelationshipKind::LexicalMayReachesRead
            )
        })
        .expect("reviewed lexical relationship")
        .clone();
    let expected_read = wrong_input
        .expression
        .graph
        .relationships
        .iter()
        .find(|relationship| {
            relationship.kind == ExpressionRelationshipKind::Reads
                && relationship.source == lexical_relationship.target
        })
        .expect("reviewed R14 read")
        .id
        .clone();
    let replacement = wrong_input
        .expression
        .graph
        .relationships
        .iter()
        .find(|relationship| relationship.id != expected_read)
        .expect("different inherited relationship")
        .id
        .clone();
    let derivation = wrong_input
        .graph
        .index
        .derivations
        .iter_mut()
        .find(|derivation| derivation.relationship_id == lexical_relationship.id)
        .expect("reviewed lexical derivation");
    let input = derivation
        .input_relationship_ids
        .iter_mut()
        .find(|identifier| **identifier == expected_read)
        .expect("R14 read derivation input");
    *input = replacement;
    derivation.input_relationship_ids.sort();
    derivation.input_relationship_ids.dedup();
    assert_eq!(wrong_input.validate(), Err(LocalFlowError::AccessMismatch));
}

#[test]
fn gt_fr_ext_017_one_arm_assignment_remains_may_reaching() {
    let source = "pub fn one_arm(mut value: i32, enabled: bool) -> i32 {\n    if enabled { value = value + 1; } else { value; }\n    value\n}\n";
    let extraction = TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_local_flow(&inventory_with_source(source))
        .expect("extract one-arm assignment R15 source");
    extraction
        .knowledge
        .validate()
        .expect("validate one-arm assignment R15 source");
    let graph = &extraction.knowledge.graph;
    assert_eq!(graph.index.completed_callable_ids.len(), 1);
    assert!(graph.relationships.iter().any(|relationship| {
        relationship.kind == LocalFlowRelationshipKind::LexicalMayReachesRead
    }));
}

#[test]
fn gt_fr_ext_017_compound_assignment_preserves_read_then_write() {
    let source = "pub fn compound(mut value: i32) -> i32 {\n    value += 1;\n    value\n}\n";
    let extraction = TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_local_flow(&inventory_with_source(source))
        .expect("extract compound assignment R15 source");
    extraction
        .knowledge
        .validate()
        .expect("validate compound assignment R15 source");
    let graph = &extraction.knowledge.graph;
    assert_eq!(graph.index.completed_callable_ids.len(), 1);
    assert!(
        graph
            .relationships
            .iter()
            .filter(|relationship| {
                relationship.kind == LocalFlowRelationshipKind::LexicalMustReachesRead
            })
            .count()
            >= 2
    );
    assert!(graph.relationships.iter().all(|relationship| {
        relationship.kind != LocalFlowRelationshipKind::LexicalMayReachesRead
    }));
}

#[test]
fn pt_fr_ext_017_nested_branches_are_deterministic_and_enforce_depth_boundary() {
    let source = "pub fn nested(mut value: i32, first: bool, second: bool) -> i32 {\n    if first { if second { value = value + 1; } else { value = value + 2; } } else { value = value + 3; }\n    value\n}\n";
    let first = TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_local_flow(&inventory_with_source(source))
        .expect("extract nested R15 branches");
    let second = TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_local_flow(&inventory_with_source(source))
        .expect("repeat nested R15 branches");
    assert_eq!(first.knowledge, second.knowledge);
    first
        .knowledge
        .validate()
        .expect("validate nested R15 branches");
    assert_eq!(first.knowledge.graph.index.completed_callable_ids.len(), 1);

    let maximum_source = nested_branch_source(64);
    let maximum =
        extract_with_runtime_stack(maximum_source).expect("accept R15 nested-branch maximum");
    maximum
        .knowledge
        .validate()
        .expect("validate R15 nested-branch maximum");
    let plus_one_source = nested_branch_source(65);
    assert_eq!(
        extract_with_runtime_stack(plus_one_source),
        Err(LocalFlowError::LimitExceeded {
            limit: LocalFlowLimit::NestedBranches,
            maximum: 64,
            observed: 65,
        })
    );
}

#[test]
fn gt_fr_ext_017_callables_absent_from_k1_are_skipped() {
    let source = r#"
pub fn complete(mut value: i32, enabled: bool) -> i32 {
    if enabled { value = value + 1; } else { value = value + 2; }
    value
}

#[cfg(test)]
pub fn test_only(value: i32) -> i32 { value }
"#;
    let extraction = TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_local_flow(&inventory_with_source(source))
        .expect("extract R15 with an absent inherited callable");
    extraction
        .knowledge
        .validate()
        .expect("validate R15 absent-callable skip");
    let signatures = extraction
        .knowledge
        .expression
        .callable
        .graph
        .entities
        .iter()
        .filter(|entity| {
            entity.kind == codenoesis_domain::s4_k1::CallableSemanticEntityKind::Signature
        })
        .collect::<Vec<_>>();
    assert_eq!(signatures.len(), 1);
    assert_eq!(signatures[0].name, "complete");
    assert_eq!(
        extraction.knowledge.graph.index.completed_callable_ids,
        vec![signatures[0].subject_id.clone()]
    );
    assert!(extraction.knowledge.graph.coverage.is_empty());
}

fn fixture_inventory() -> RepositoryInventory {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/s4/rust-local-flow-v1/repository");
    let files = FIXTURE_FILES
        .into_iter()
        .map(|(path, blob_oid)| {
            AcquiredFile::new(
                path.to_owned(),
                RegularFileMode::Regular,
                ObjectId::parse_sha1(blob_oid).expect("reviewed R15 blob"),
                fs::read(root.join(path)).expect("read R15 fixture"),
            )
        })
        .collect::<Vec<_>>();
    RepositoryInventory::classify(AcquiredRepository::new(
        BoundRevision::new(
            RepositoryIdentity::parse(REPOSITORY_ID).expect("R15 repository identity"),
            ObjectId::parse_sha1(COMMIT_OID).expect("R15 commit"),
            ObjectId::parse_sha1(TREE_OID).expect("R15 tree"),
        ),
        u64::try_from(files.len()).expect("R15 file count"),
        files,
    ))
}

fn inventory_with_source(source: &str) -> RepositoryInventory {
    RepositoryInventory::classify(AcquiredRepository::new(
        BoundRevision::new(
            RepositoryIdentity::parse("urn:codenoesis:test:r15-local-flow")
                .expect("R15 synthetic repository identity"),
            oid('a'),
            oid('b'),
        ),
        2,
        vec![
            AcquiredFile::new(
                "Cargo.toml".to_owned(),
                RegularFileMode::Regular,
                oid('c'),
                b"[package]\nname=\"r15-local-flow\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[lib]\npath=\"src/lib.rs\"\n"
                    .to_vec(),
            ),
            AcquiredFile::new(
                "src/lib.rs".to_owned(),
                RegularFileMode::Regular,
                oid('d'),
                source.as_bytes().to_vec(),
            ),
        ],
    ))
}

fn oid(value: char) -> ObjectId {
    ObjectId::parse_sha1(&value.to_string().repeat(40)).expect("synthetic SHA-1 object ID")
}

fn nested_branch_source(depth: usize) -> String {
    let mut body = "value".to_owned();
    for _ in 0..depth {
        body = format!("if enabled {{ {body} }} else {{ value }}");
    }
    format!("pub fn depth(value: i32, enabled: bool) -> i32 {{ {body} }}\n")
}

fn extract_with_runtime_stack(
    source: String,
) -> Result<codenoesis_domain::s4_r15::LocalFlowExtraction, LocalFlowError> {
    std::thread::Builder::new()
        .name("r15-runtime-stack-test".to_owned())
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            TreeSitterRustWorkspaceExtractor::new()
                .extract_rust_local_flow(&inventory_with_source(&source))
        })
        .expect("spawn R15 runtime-stack test")
        .join()
        .expect("join R15 runtime-stack test")
}
