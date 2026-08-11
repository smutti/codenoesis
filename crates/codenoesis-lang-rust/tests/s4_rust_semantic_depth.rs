use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::thread;

use codenoesis_domain::knowledge::{EntityKind, RelationshipKind};
use codenoesis_domain::s4_r5::{
    CompilationPresence, R5_DETERMINISM_PERMUTATIONS, RustMethodContext, RustSemanticAttributeKind,
    RustSemanticEntityKind, RustSemanticError, RustSemanticLimit, RustSemanticProperties,
    capability_state, r5_entity_counts,
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
const EMPTY_SEMANTIC_EXTENSION_SOURCE: &str = r"#![allow(dead_code)]

pub fn choose(value: i32, flag: bool) -> i32 {
    let mut total = value;
    if flag {
        total = total + 1;
    }
    let result = total;
    result
}
";
const PAIRED_CONSTANT_EXTENSION_SOURCE: &str = r"#![allow(dead_code)]

pub const MARKER: u8 = 1;

pub fn choose(value: i32, flag: bool) -> i32 {
    let mut total = value;
    if flag {
        total = total + 1;
    }
    let result = total;
    result
}
";
const CFG_OWNER_ALTERNATIVES_SOURCE: &str = r#"
#[cfg(feature = "desktop")]
pub struct ConditionalOwner;

#[cfg(not(feature = "desktop"))]
pub struct ConditionalOwner {
    pub context: String,
}

#[cfg(feature = "desktop")]
pub enum ConditionalEnum {
    Desktop,
}

#[cfg(not(feature = "desktop"))]
pub enum ConditionalEnum {
    Headless,
}

#[cfg(feature = "desktop")]
pub trait ConditionalTrait {
    fn render(&self);
}

#[cfg(not(feature = "desktop"))]
pub trait ConditionalTrait {
    fn serialize(&self);
}
"#;
const CFG_MEMBER_ALTERNATIVES_SOURCE: &str = r#"
#[cfg(windows)]
const BIN_DATA: &[u8] = include_bytes!("data.bin");

#[cfg(not(windows))]
const BIN_DATA: &[u8] = &[];
"#;
const CFG_ALL_MEMBER_KINDS_SOURCE: &str = r"
pub struct ConditionalFields {
    #[cfg(unix)]
    pub value: u8,
    #[cfg(windows)]
    pub value: u8,
}

pub enum ConditionalVariants {
    #[cfg(unix)]
    Value,
    #[cfg(windows)]
    Value,
}

pub trait ConditionalMembers {
    #[cfg(unix)]
    const LIMIT: u8 = 1;
    #[cfg(windows)]
    const LIMIT: u8 = 2;

    #[cfg(unix)]
    type Output;
    #[cfg(windows)]
    type Output;

    #[cfg(unix)]
    fn render(&self);
    #[cfg(windows)]
    fn render(&self);
}

#[cfg(unix)]
static STATE: u8 = 1;
#[cfg(windows)]
static STATE: u8 = 2;
";

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
fn gt_fr_ext_010_empty_additive_extension_is_valid_and_fail_closed() {
    let extraction = TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_semantic_depth_incremental(
            &synthetic_inventory(EMPTY_SEMANTIC_EXTENSION_SOURCE),
            &[],
            &[],
        )
        .expect("extract valid empty additive R5 extension");
    let knowledge = &extraction.knowledge;
    assert_eq!(knowledge.extraction_chunks.len(), 1);
    assert!(knowledge.graph.entities.is_empty());
    assert!(knowledge.graph.relationships.is_empty());
    assert!(knowledge.graph.claims.is_empty());
    assert!(knowledge.graph.index.member_entity_ids.is_empty());
    assert!(
        knowledge
            .graph
            .index
            .implementation_context_method_ids
            .is_empty()
    );
    assert_eq!(knowledge.graph.coverage.len(), 8);
    assert_eq!(knowledge.validate(), Ok(()));

    let mut missing_chunks = knowledge.clone();
    missing_chunks.extraction_chunks.clear();
    assert_eq!(
        missing_chunks.validate(),
        Err(RustSemanticError::ContractInvalid)
    );

    let mut invalid_index = knowledge.clone();
    invalid_index
        .graph
        .index
        .member_entity_ids
        .push("urn:codenoesis:entity:blake3:invalid".to_owned());
    assert_eq!(
        invalid_index.validate(),
        Err(RustSemanticError::ContractInvalid)
    );

    let mut dangling_evidence = knowledge.clone();
    dangling_evidence.graph.coverage[0].evidence_ids = vec![
        "urn:codenoesis:evidence:sha256:0000000000000000000000000000000000000000000000000000000000000000"
            .to_owned(),
    ];
    assert_eq!(
        dangling_evidence.validate(),
        Err(RustSemanticError::ContractInvalid)
    );
}

#[test]
fn gt_fr_ext_010_paired_constant_still_emits_supported_r5_member() {
    let extraction = TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_semantic_depth_incremental(
            &synthetic_inventory(PAIRED_CONSTANT_EXTENSION_SOURCE),
            &[],
            &[],
        )
        .expect("extract paired supported R5 constant");
    let graph = &extraction.knowledge.graph;
    assert_eq!(graph.entities.len(), 1);
    assert_eq!(graph.entities[0].kind, RustSemanticEntityKind::Constant);
    assert_eq!(graph.entities[0].name, "MARKER");
    assert_eq!(graph.relationships.len(), 1);
    assert_eq!(graph.claims.len(), 2);
    assert_eq!(
        graph.index.member_entity_ids,
        vec![graph.entities[0].id.clone()]
    );
    assert!(graph.index.implementation_context_method_ids.is_empty());
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
fn gt_fr_ext_010_cfg_owner_alternatives_preserve_uncertainty() {
    let extraction = TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_semantic_depth_incremental(
            &synthetic_inventory(CFG_OWNER_ALTERNATIVES_SOURCE),
            &[],
            &[],
        )
        .expect("extract cfg-conditional owner alternatives");
    let graph = &extraction.knowledge.graph;

    for (kind, name) in [
        (EntityKind::RustStruct, "ConditionalOwner"),
        (EntityKind::RustEnum, "ConditionalEnum"),
        (EntityKind::RustTrait, "ConditionalTrait"),
    ] {
        assert_eq!(
            graph
                .legacy_entities
                .iter()
                .filter(|entity| entity.kind == kind && entity.name == name)
                .count(),
            1,
            "cfg alternatives must retain one logical {name} owner"
        );
    }

    for (kind, name) in [
        (RustSemanticEntityKind::Field, "context"),
        (RustSemanticEntityKind::EnumVariant, "Desktop"),
        (RustSemanticEntityKind::EnumVariant, "Headless"),
        (RustSemanticEntityKind::Method, "render"),
        (RustSemanticEntityKind::Method, "serialize"),
    ] {
        assert!(graph.entities.iter().any(|entity| {
            entity.kind == kind
                && entity.name == name
                && entity.compilation_presence == CompilationPresence::ConditionalUnknown
        }));
    }

    let cfg_evidence_ids = graph
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "rust.cfg_presence_unresolved")
        .flat_map(|diagnostic| diagnostic.evidence_ids.iter().cloned())
        .collect::<BTreeSet<_>>();
    assert_eq!(cfg_evidence_ids.len(), 6);
    let evidence_ids = graph
        .evidence
        .iter()
        .map(|evidence| evidence.id.as_str())
        .collect::<BTreeSet<_>>();
    assert!(
        cfg_evidence_ids
            .iter()
            .all(|identifier| evidence_ids.contains(identifier.as_str()))
    );
}

#[test]
fn gt_fr_ext_010_cfg_member_alternatives_merge_attribute_evidence() {
    let extraction = TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_semantic_depth_incremental(
            &synthetic_inventory(CFG_MEMBER_ALTERNATIVES_SOURCE),
            &[],
            &[],
        )
        .expect("extract cfg-conditional member alternatives");
    let graph = &extraction.knowledge.graph;
    let members = graph
        .entities
        .iter()
        .filter(|entity| {
            entity.kind == RustSemanticEntityKind::Constant && entity.name == "BIN_DATA"
        })
        .collect::<Vec<_>>();
    assert_eq!(members.len(), 1);
    let member = members[0];
    assert_eq!(
        member.compilation_presence,
        CompilationPresence::ConditionalUnknown
    );
    assert_eq!(member.attributes().len(), 2);
    assert_eq!(
        member
            .attributes()
            .iter()
            .map(|attribute| attribute.token_text.as_str())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["#[cfg(not(windows))]", "#[cfg(windows)]"])
    );
    let evidence_ids = graph
        .evidence
        .iter()
        .map(|evidence| evidence.id.as_str())
        .collect::<BTreeSet<_>>();
    assert!(
        member
            .attributes()
            .iter()
            .all(|attribute| evidence_ids.contains(attribute.evidence_id.as_str()))
    );
    assert_eq!(
        graph
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "rust.cfg_presence_unresolved")
            .count(),
        2
    );
}

#[test]
fn gt_fr_ext_010_cfg_alternatives_cover_every_r5_member_kind() {
    let extraction = TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_semantic_depth_incremental(
            &synthetic_inventory(CFG_ALL_MEMBER_KINDS_SOURCE),
            &[],
            &[],
        )
        .expect("extract all homogeneous cfg member alternatives");
    let graph = &extraction.knowledge.graph;
    for (kind, name) in [
        (RustSemanticEntityKind::Field, "value"),
        (RustSemanticEntityKind::EnumVariant, "Value"),
        (RustSemanticEntityKind::Constant, "LIMIT"),
        (RustSemanticEntityKind::Static, "STATE"),
        (RustSemanticEntityKind::AssociatedType, "Output"),
        (RustSemanticEntityKind::Method, "render"),
    ] {
        let members = graph
            .entities
            .iter()
            .filter(|entity| entity.kind == kind && entity.name == name)
            .collect::<Vec<_>>();
        assert_eq!(
            members.len(),
            1,
            "unexpected logical member count for {name}"
        );
        assert_eq!(
            members[0].compilation_presence,
            CompilationPresence::ConditionalUnknown
        );
        assert_eq!(members[0].attributes().len(), 2);
        assert!(
            members[0]
                .attributes()
                .iter()
                .all(|attribute| attribute.kind == RustSemanticAttributeKind::Cfg)
        );
    }
}

#[test]
fn sec_fr_ext_010_cfg_owner_alternative_boundary_failures_remain_typed() {
    for (label, source, member_kind, normalized_member) in [
        (
            "unconditional duplicates",
            "pub struct Repeated;\npub struct Repeated;\n",
            "rust.struct",
            "Repeated",
        ),
        (
            "mixed unconditional and cfg",
            "pub struct Repeated;\n#[cfg(feature = \"desktop\")] pub struct Repeated;\n",
            "rust.struct",
            "Repeated",
        ),
        (
            "cfg_attr only",
            "#[cfg_attr(feature = \"desktop\", repr(C))] pub struct Repeated;\n#[cfg_attr(not(feature = \"desktop\"), repr(transparent))] pub struct Repeated;\n",
            "rust.struct",
            "Repeated",
        ),
        (
            "visibility mismatch",
            "#[cfg(feature = \"desktop\")] pub struct Repeated;\n#[cfg(not(feature = \"desktop\"))] struct Repeated;\n",
            "rust.struct",
            "Repeated",
        ),
        (
            "module alternatives",
            "#[cfg(feature = \"desktop\")] mod repeated {}\n#[cfg(not(feature = \"desktop\"))] mod repeated {}\n",
            "rust.module",
            "repeated",
        ),
        (
            "duplicate member preimage",
            "#[cfg(feature = \"desktop\")] pub struct Repeated { pub value: u8 }\n#[cfg(not(feature = \"desktop\"))] pub struct Repeated { pub value: u16 }\n",
            "rust.field",
            "value",
        ),
        (
            "direct cfg member type mismatch",
            "pub struct Holder {\n#[cfg(unix)] pub value: u8,\n#[cfg(windows)] pub value: u16,\n}\n",
            "rust.field",
            "value",
        ),
        (
            "direct cfg method signature mismatch",
            "pub trait Clipboard {\n#[cfg(unix)] fn start(&self, value: u8);\n#[cfg(windows)] fn start(&self, value: u16);\n}\n",
            "rust.method",
            "start",
        ),
        (
            "direct cfg member visibility mismatch",
            "#[cfg(unix)] pub const VALUE: u8 = 1;\n#[cfg(windows)] const VALUE: u8 = 2;\n",
            "rust.constant",
            "VALUE",
        ),
        (
            "cfg_attr-only members",
            "pub struct Holder {\n#[cfg_attr(unix, allow(dead_code))] pub value: u8,\n#[cfg_attr(windows, allow(dead_code))] pub value: u8,\n}\n",
            "rust.field",
            "value",
        ),
        (
            "mixed unconditional and direct cfg members",
            "const VALUE: u8 = 1;\n#[cfg(windows)] const VALUE: u8 = 2;\n",
            "rust.constant",
            "VALUE",
        ),
    ] {
        let error = TreeSitterRustWorkspaceExtractor::new()
            .extract_rust_semantic_depth_incremental(&synthetic_inventory(source), &[], &[])
            .expect_err(label);
        assert!(
            matches!(
                &error,
                RustSemanticError::IdentityConflict {
                    member_kind: actual_kind,
                    normalized_member: actual_member,
                    ..
                } if actual_kind == member_kind && actual_member == normalized_member
            ),
            "unexpected {label} error: {error:?}"
        );
    }
}

#[test]
fn sec_fr_ext_010_cfg_member_attribute_maximum_and_plus_one_are_typed() {
    let maximum = cfg_member_attribute_source(63, 63);
    let extraction = TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_semantic_depth_incremental(&synthetic_inventory(&maximum), &[], &[])
        .expect("combined cfg member attribute maximum must succeed");
    let member = extraction
        .knowledge
        .graph
        .entities
        .iter()
        .find(|entity| entity.name == "BOUNDED")
        .expect("bounded cfg member");
    assert_eq!(member.attributes().len(), 128);

    let plus_one = cfg_member_attribute_source(63, 64);
    let error = TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_semantic_depth_incremental(&synthetic_inventory(&plus_one), &[], &[])
        .expect_err("combined cfg member attribute maximum plus one must fail");
    assert_eq!(
        error,
        RustSemanticError::LimitExceeded {
            limit: RustSemanticLimit::OuterAttributesPerDeclaration,
            maximum: 128,
            observed: 129,
        }
    );
}

#[test]
fn pt_nfr_det_001_cfg_alternatives_are_permutation_and_schedule_invariant() {
    for source in [
        CFG_OWNER_ALTERNATIVES_SOURCE,
        CFG_MEMBER_ALTERNATIVES_SOURCE,
        CFG_ALL_MEMBER_KINDS_SOURCE,
    ] {
        assert_cfg_alternative_determinism(source);
    }
}

fn assert_cfg_alternative_determinism(source: &str) {
    let expected = TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_semantic_depth_incremental(
            &synthetic_inventory_with_order(source, false),
            &[],
            &[],
        )
        .expect("extract cfg alternatives baseline")
        .knowledge;
    for permutation in 0..R5_DETERMINISM_PERMUTATIONS {
        let actual = TreeSitterRustWorkspaceExtractor::new()
            .extract_rust_semantic_depth_incremental(
                &synthetic_inventory_with_order(source, permutation % 2 == 1),
                &[],
                &[],
            )
            .expect("extract cfg alternatives permutation")
            .knowledge;
        assert_eq!(
            actual, expected,
            "cfg permutation {permutation} changed knowledge"
        );
    }
    thread::scope(|scope| {
        let handles = (0..8)
            .map(|worker| {
                scope.spawn(move || {
                    TreeSitterRustWorkspaceExtractor::new()
                        .extract_rust_semantic_depth_incremental(
                            &synthetic_inventory_with_order(source, worker % 2 == 1),
                            &[],
                            &[],
                        )
                        .expect("extract cfg alternatives schedule")
                        .knowledge
                })
            })
            .collect::<Vec<_>>();
        for handle in handles {
            assert_eq!(handle.join().expect("cfg replay worker"), expected);
        }
    });
}

fn cfg_member_attribute_source(first_other: usize, second_other: usize) -> String {
    let mut source = String::new();
    for index in 0..first_other {
        writeln!(&mut source, "#[first_{index}]")
            .expect("writing Rust source to a String cannot fail");
    }
    source.push_str("#[cfg(unix)]\nconst BOUNDED: u8 = 1;\n");
    for index in 0..second_other {
        writeln!(&mut source, "#[second_{index}]")
            .expect("writing Rust source to a String cannot fail");
    }
    source.push_str("#[cfg(windows)]\nconst BOUNDED: u8 = 2;\n");
    source
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

#[test]
fn sec_fr_ext_010_malformed_and_limit_plus_one_fail_closed() {
    let extractor = TreeSitterRustWorkspaceExtractor::new();
    let malformed = extractor
        .extract_rust_semantic_depth_incremental(
            &synthetic_inventory("pub struct Broken { value: }\n"),
            &[],
            &[],
        )
        .expect_err("malformed R5 declaration must fail");
    assert!(
        matches!(
            &malformed,
            RustSemanticError::InvalidDeclaration {
                path,
                declaration_kind,
                ..
            } if path == "src/lib.rs" && !declaration_kind.is_empty()
        ),
        "unexpected malformed-declaration error: {malformed:?}"
    );

    let mut fields = String::from("pub struct TooMany {\n");
    for index in 0..=1_024 {
        writeln!(&mut fields, "field_{index}: u8,")
            .expect("writing Rust source to a String cannot fail");
    }
    fields.push_str("}\n");
    let exceeded = extractor
        .extract_rust_semantic_depth_incremental(&synthetic_inventory(&fields), &[], &[])
        .expect_err("R5 field maximum plus one must fail");
    assert_eq!(
        exceeded,
        RustSemanticError::LimitExceeded {
            limit: RustSemanticLimit::FieldsPerOwner,
            maximum: 1_024,
            observed: 1_025,
        }
    );

    let collision = extractor
        .extract_rust_semantic_depth_incremental(
            &synthetic_inventory("pub struct Collision { café: u8, cafe\u{301}: u8 }\n"),
            &[],
            &[],
        )
        .expect_err("NFC-equivalent R5 members must fail closed");
    assert!(
        matches!(
            &collision,
            RustSemanticError::IdentityConflict {
                member_kind,
                normalized_member,
                ..
            } if member_kind == "rust.field" && normalized_member == "café"
        ),
        "unexpected R5 identity-collision error: {collision:?}"
    );
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

fn synthetic_inventory(source: &str) -> RepositoryInventory {
    synthetic_inventory_with_order(source, false)
}

fn synthetic_inventory_with_order(source: &str, reverse: bool) -> RepositoryInventory {
    let mut files = vec![
        AcquiredFile::new(
            "Cargo.toml".to_owned(),
            RegularFileMode::Regular,
            ObjectId::parse_sha1(&"c".repeat(40)).expect("synthetic manifest OID"),
            b"[package]\nname = \"fail-closed\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[lib]\npath = \"src/lib.rs\"\n"
                .to_vec(),
        ),
        AcquiredFile::new(
            "src/lib.rs".to_owned(),
            RegularFileMode::Regular,
            ObjectId::parse_sha1(&"d".repeat(40)).expect("synthetic source OID"),
            source.as_bytes().to_vec(),
        ),
    ];
    if reverse {
        files.reverse();
    }
    RepositoryInventory::classify(AcquiredRepository::new(
        BoundRevision::new(
            RepositoryIdentity::parse("urn:codenoesis:test:r5-fail-closed")
                .expect("synthetic repository identity"),
            ObjectId::parse_sha1(&"a".repeat(40)).expect("synthetic commit OID"),
            ObjectId::parse_sha1(&"b".repeat(40)).expect("synthetic tree OID"),
        ),
        u64::try_from(files.len()).expect("synthetic file count"),
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
