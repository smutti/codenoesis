use std::collections::BTreeMap;

use codenoesis_domain::s4_r16::{ConstantEvaluationError, ConstantEvaluationLimit};
use codenoesis_domain::{
    AcquiredFile, AcquiredRepository, BoundRevision, ObjectId, RegularFileMode, RepositoryIdentity,
    RepositoryInventory,
};
use codenoesis_lang_rust::TreeSitterRustWorkspaceExtractor;

#[test]
fn gt_fr_ext_020_closed_constant_grammar_is_exact_and_deterministic() {
    let source = r"
pub const BASE: i8 = 0b0000_0111;
pub const NEGATED: i8 = -(BASE + 1);
pub const MASK: u8 = !0b1111_0000u8;
pub const ENABLED: bool = (true && !false) || (false > true);
pub static LIMIT: u16 = 0x00ff + 0o1;
";
    let inventory = inventory_with_source(source);
    let first = TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_constant_evaluation(&inventory)
        .expect("extract supported R16 constants");
    let second = TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_constant_evaluation(&inventory)
        .expect("repeat supported R16 constants");
    assert_eq!(first, second);
    first
        .knowledge
        .validate()
        .expect("validate supported R16 constants");
    assert_eq!(
        evaluated_values(&first),
        BTreeMap::from([
            ("BASE".to_owned(), ("7".to_owned(), "i8".to_owned())),
            ("ENABLED".to_owned(), ("true".to_owned(), "bool".to_owned()),),
            ("LIMIT".to_owned(), ("256".to_owned(), "u16".to_owned())),
            ("MASK".to_owned(), ("15".to_owned(), "u8".to_owned())),
            ("NEGATED".to_owned(), ("-8".to_owned(), "i8".to_owned()),),
        ])
    );
    assert!(first.knowledge.graph.coverage.is_empty());
    assert_eq!(
        first
            .knowledge
            .graph
            .index
            .derivations
            .iter()
            .filter(|derivation| !derivation.dependency_entity_ids.is_empty())
            .count(),
        1
    );
}

#[test]
fn ft_fr_ext_020_unsupported_sources_emit_typed_gaps_without_guessed_values() {
    let source = r"
pub const TARGET: usize = 4;
pub const FLOAT: f64 = 1.5;
pub const CALL: i32 = helper();
pub const MISSING: i32 = UNKNOWN + 1;
pub const OVERFLOW: i8 = 127 + 1;

const fn helper() -> i32 { 1 }

pub enum PlatformSized { First, Second }

#[repr(u8)]
pub enum Partial {
    First = 1,
    Second = UNKNOWN,
}
";
    let extraction = TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_constant_evaluation(&inventory_with_source(source))
        .expect("extract unsupported R16 constants");
    extraction
        .knowledge
        .validate()
        .expect("validate unsupported R16 gaps");
    assert!(extraction.knowledge.graph.entities.is_empty());
    assert!(extraction.knowledge.graph.relationships.is_empty());
    assert!(extraction.knowledge.graph.claims.is_empty());
    assert_eq!(
        extraction
            .knowledge
            .graph
            .coverage
            .iter()
            .fold(BTreeMap::new(), |mut counts, gap| {
                *counts.entry(gap.capability.as_str()).or_insert(0_usize) += 1;
                counts
            }),
        BTreeMap::from([
            ("rust.constant_arithmetic_not_defined", 1),
            ("rust.constant_dependency_not_evaluated", 1),
            ("rust.constant_expression_not_evaluated", 2),
            ("rust.constant_target_dependent", 1),
            ("rust.enum_discriminant_not_evaluated", 2),
        ])
    );
}

#[test]
fn gt_fr_ext_020_cfg_subject_value_is_exact_but_cannot_authorize_a_dependency() {
    let source = r#"
#[cfg(feature = "hydrate")]
pub const CONDITIONAL: u32 = 250;
pub const DEPENDENT: u32 = CONDITIONAL + 1;

#[cfg(feature = "hydrate")]
pub mod internal {
    pub const NESTED: u8 = 0x4D;
}
"#;
    let extraction = TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_constant_evaluation(&inventory_with_source(source))
        .expect("extract conditional R16 subjects");
    extraction
        .knowledge
        .validate()
        .expect("validate conditional R16 subjects");
    assert_eq!(
        evaluated_values(&extraction),
        BTreeMap::from([
            (
                "CONDITIONAL".to_owned(),
                ("250".to_owned(), "u32".to_owned()),
            ),
            ("NESTED".to_owned(), ("77".to_owned(), "u8".to_owned())),
        ])
    );
    assert!(
        extraction
            .knowledge
            .graph
            .coverage
            .iter()
            .any(|gap| { gap.capability == "rust.constant_dependency_not_evaluated" })
    );
}

#[test]
fn pt_fr_ext_020_syntax_node_boundary_is_hard_and_typed() {
    let maximum = repeated_sum(256);
    let extraction = TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_constant_evaluation(&inventory_with_source(&format!(
            "pub const VALUE: u16 = {maximum};\n"
        )))
        .expect("accept R16 syntax-node maximum");
    assert_eq!(
        evaluated_values(&extraction).get("VALUE"),
        Some(&("256".to_owned(), "u16".to_owned()))
    );

    let plus_one = repeated_sum(257);
    assert_eq!(
        TreeSitterRustWorkspaceExtractor::new().extract_rust_constant_evaluation(
            &inventory_with_source(&format!("pub const VALUE: u16 = {plus_one};\n"))
        ),
        Err(ConstantEvaluationError::LimitExceeded {
            limit: ConstantEvaluationLimit::SyntaxNodesPerExpression,
            maximum: 256,
            observed: 257,
        })
    );
}

#[test]
fn pt_fr_ext_020_dependency_depth_is_cache_and_order_independent() {
    let maximum = dependency_chain(64);
    let extraction = TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_constant_evaluation(&inventory_with_source(&maximum))
        .expect("accept R16 dependency-level maximum");
    assert_eq!(
        evaluated_values(&extraction).get("VALUE_64"),
        Some(&("1".to_owned(), "i32".to_owned()))
    );

    let plus_one = dependency_chain(65);
    assert_eq!(
        TreeSitterRustWorkspaceExtractor::new()
            .extract_rust_constant_evaluation(&inventory_with_source(&plus_one)),
        Err(ConstantEvaluationError::LimitExceeded {
            limit: ConstantEvaluationLimit::DependencyLevels,
            maximum: 64,
            observed: 65,
        })
    );
}

fn evaluated_values(
    extraction: &codenoesis_domain::s4_r16::ConstantEvaluationExtraction,
) -> BTreeMap<String, (String, String)> {
    let names = extraction
        .knowledge
        .local_flow
        .expression
        .callable
        .graph
        .entities
        .iter()
        .map(|entity| (entity.id.as_str(), entity.name.as_str()))
        .collect::<BTreeMap<_, _>>();
    extraction
        .knowledge
        .graph
        .entities
        .iter()
        .map(|entity| {
            (
                names[entity.declared_value_id.as_str()].to_owned(),
                (entity.canonical_value.clone(), entity.rust_type.clone()),
            )
        })
        .collect()
}

fn inventory_with_source(source: &str) -> RepositoryInventory {
    RepositoryInventory::classify(AcquiredRepository::new(
        BoundRevision::new(
            RepositoryIdentity::parse("urn:codenoesis:test:r16-constant-evaluation")
                .expect("R16 synthetic repository identity"),
            oid('a'),
            oid('b'),
        ),
        2,
        vec![
            AcquiredFile::new(
                "Cargo.toml".to_owned(),
                RegularFileMode::Regular,
                oid('c'),
                b"[package]\nname=\"r16-constant-evaluation\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[lib]\npath=\"src/lib.rs\"\n"
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

fn repeated_sum(terms: usize) -> String {
    std::iter::repeat_n("1", terms)
        .collect::<Vec<_>>()
        .join(" + ")
}

fn dependency_chain(levels: usize) -> String {
    let mut source = "pub const VALUE_0: i32 = 1;\n".to_owned();
    for level in 1..=levels {
        use std::fmt::Write as _;
        writeln!(
            source,
            "pub const VALUE_{level}: i32 = VALUE_{};",
            level - 1
        )
        .expect("write R16 dependency chain");
    }
    source
}

fn oid(value: char) -> ObjectId {
    ObjectId::parse_sha1(&value.to_string().repeat(40)).expect("synthetic SHA-1 object ID")
}
