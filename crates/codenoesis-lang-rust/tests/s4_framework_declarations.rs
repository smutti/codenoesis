use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::thread;

use codenoesis_domain::knowledge::RelationshipKind;
use codenoesis_domain::s4_r6::{
    FrameworkEpistemicState, FrameworkError, FrameworkLimit, FrameworkRole, FrameworkSourceProfile,
    FrameworkTargetBinding, R6_DETERMINISM_PERMUTATIONS, framework_role_counts,
};
use codenoesis_domain::{
    AcquiredFile, AcquiredRepository, BoundRevision, ObjectId, RegularFileMode, RepositoryIdentity,
    RepositoryInventory,
};
use codenoesis_lang_rust::TreeSitterRustWorkspaceExtractor;
use codenoesis_ports::RustFrameworkDeclarationExtractor;

const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s4-framework-declarations-v1";
const COMMIT_OID: &str = "d6e5ae087cdfe44bf7c47bf31a73dfc09c3795b1";
const TREE_OID: &str = "22edc029079ac4fb600e07bc43596d8794e28d52";
const FIXTURE_FILES: [(&str, &str); 7] = [
    ("Cargo.toml", "5343d18251ca02791a1e2b234ed548becf09e43a"),
    ("build.rs", "24818b624f0e012709421566c4e4ed03fca8e8e6"),
    (
        "generated/framework.rs",
        "9b617a46d696bd8e56e5e0c4ddd34aa2bcb1d67f",
    ),
    (
        "src/attribute_style.rs",
        "c28cf3303d65de6beecee515eee6fd4a8c6bddd4",
    ),
    (
        "src/builder_style.rs",
        "621fa4daec573b24a08bd9e83385ca8b36310bde",
    ),
    ("src/lib.rs", "9b3a9c9f5871133f67cc311378cf332b06ad9c05"),
    (
        "target/generated.rs",
        "a60381cd3e9b5fbee741390564d43ca18a8a0e8f",
    ),
];

#[test]
fn gt_fr_ext_011_explicit_builder_declarations() {
    let extraction = extract_fixture(0, false);
    let declarations = &extraction.knowledge.graph.declarations;
    let explicit = declarations
        .iter()
        .filter(|value| value.source_profile == FrameworkSourceProfile::ExplicitBuilderRegistration)
        .collect::<Vec<_>>();
    assert_eq!(explicit.len(), 12);
    assert!(explicit.iter().all(|value| {
        value.epistemic_state == FrameworkEpistemicState::DeclaredRegistrationSyntax
    }));
    assert_eq!(
        explicit
            .iter()
            .map(|value| value.id.as_str())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "urn:codenoesis:entity:blake3:349c67c32260aadcefd9c1f88c5efdcac001a8885a271e446649b0fc0eb40688",
            "urn:codenoesis:entity:blake3:3a0baa691bda100c9e15a8b3fab33ca003c0216711a707d162b4e6ef08f3b25f",
            "urn:codenoesis:entity:blake3:66d0fa6b5793836d5659ffdbe9ebcceddb90947fe47b4be6e2408c44b84b71b5",
            "urn:codenoesis:entity:blake3:732df788cb75daf7eddd543cac10dc04eb29283fbddfa4f13ced90fab54eaecd",
            "urn:codenoesis:entity:blake3:75284e69953b718c92c4379f91fb1ecea79ef70d03028737944922585fd010e2",
            "urn:codenoesis:entity:blake3:7c061a77b855f230d02b56fd07a10384bdab4fd69dcc0450609f2dfbabf9f63f",
            "urn:codenoesis:entity:blake3:8f2f751cd648fcb1e84c53410cc8608488c5736f27cb2ae59ef8d4a7df62d518",
            "urn:codenoesis:entity:blake3:a5c53fc8f254da6257a0e1950cdb1b7ba3606e2c74706bc9a903966b6a6706ee",
            "urn:codenoesis:entity:blake3:cc19916c08caf7372d763c4e1f792cfa081263f22a6ed0380a984b46e27c0821",
            "urn:codenoesis:entity:blake3:d5e0dc2de378c935a2eb26771b8fe4ae547a4e02a6eaa0261893adbe3b8b516b",
            "urn:codenoesis:entity:blake3:d8b887e3484304b677bb434f3f868e2737b04af5d0c06599c0fbae6b9b109a41",
            "urn:codenoesis:entity:blake3:dfd45749cd2a4b7200d4b8a6d1bb45e3b28c0c27540a8f73de1305e9552345cb",
        ])
    );
    assert_eq!(
        framework_role_counts(declarations),
        BTreeMap::from([
            (FrameworkRole::Component, 4),
            (FrameworkRole::Configuration, 2),
            (FrameworkRole::Endpoint, 2),
            (FrameworkRole::Handler, 3),
            (FrameworkRole::Route, 10),
            (FrameworkRole::Service, 3),
        ])
    );
}

#[test]
fn conf_fr_ext_011_reviewed_fixture_newlines_are_platform_neutral() {
    assert_eq!(
        normalize_reviewed_fixture_bytes(b"#[component]\r\nfn app() {}\r\n"),
        Ok(b"#[component]\nfn app() {}\n".to_vec())
    );
    assert_eq!(
        normalize_reviewed_fixture_bytes(b"#[component]\rfn app() {}\n"),
        Err("reviewed fixture contains a bare carriage return")
    );
}

#[test]
fn gt_fr_ext_011_attribute_macro_candidates_remain_unresolved() {
    let graph = &extract_fixture(0, false).knowledge.graph;
    let candidates = graph
        .declarations
        .iter()
        .filter(|value| value.source_profile == FrameworkSourceProfile::AttributeMacroCandidate)
        .collect::<Vec<_>>();
    assert_eq!(candidates.len(), 12);
    assert!(candidates.iter().all(|value| {
        value.epistemic_state == FrameworkEpistemicState::CandidateUnresolved
            && value.method.is_none()
            && value.path.is_none()
            && value.configuration_key.is_none()
    }));
    assert_eq!(graph.diagnostics.len(), 14);
    assert_eq!(graph.coverage.len(), 40);
    assert_eq!(
        graph
            .diagnostics
            .iter()
            .find(|value| {
                value.declaration_id
                    == "urn:codenoesis:entity:blake3:a13acca913973488d06416af51a5da3da283df400bd7933366f59ac867ac4851"
            })
            .expect("reviewed component candidate diagnostic")
            .id,
        "urn:codenoesis:diagnostic:blake3:10ac0a99f47a7edb4d02d619153afb22466c3f0f3acb80bd4152f61751d77b6c"
    );
    assert!(candidates.iter().all(|declaration| {
        graph.coverage.iter().any(|gap| {
            gap.declaration_id == declaration.id
                && gap.capability == "rust.framework_runtime_not_observed"
        })
    }));
}

#[test]
fn gt_fr_ext_011_unique_local_targets_only() {
    let declarations = &extract_fixture(0, false).knowledge.graph.declarations;
    let counts = declarations
        .iter()
        .fold(BTreeMap::new(), |mut counts, value| {
            *counts.entry(value.target_binding).or_insert(0_usize) += 1;
            counts
        });
    assert_eq!(counts[&FrameworkTargetBinding::ResolvedUnique], 21);
    assert_eq!(counts[&FrameworkTargetBinding::UnresolvedExternal], 1);
    assert_eq!(counts[&FrameworkTargetBinding::AmbiguousLocal], 1);
    assert_eq!(counts[&FrameworkTargetBinding::NotApplicable], 1);
    assert!(declarations.iter().all(|value| {
        value.local_target_id.is_some()
            == (value.target_binding == FrameworkTargetBinding::ResolvedUnique)
    }));
}

#[test]
fn gt_fr_ext_011_qualified_simple_method_wrapper_is_reviewed() {
    let extraction = extract_source(
        "pub fn handler() {}\npub fn routes() { Router::new().route(\"/qualified\", axum::routing::get(handler)) }\n",
    )
    .expect("qualified simple method wrapper must remain source-only");
    let [declaration] = extraction.knowledge.graph.declarations.as_slice() else {
        panic!("expected one qualified-wrapper declaration");
    };
    assert_eq!(declaration.method.as_deref(), Some("GET"));
    assert_eq!(declaration.path.as_deref(), Some("/qualified"));
    assert_eq!(declaration.target_spelling.as_deref(), Some("handler"));
}

#[test]
fn pt_nfr_det_001_r6_permutation_and_replay_invariant() {
    let expected = extract_fixture(0, false).knowledge;
    for permutation in 0..R6_DETERMINISM_PERMUTATIONS {
        let rotation = usize::try_from(permutation).expect("R6 permutation index");
        assert_eq!(
            extract_fixture(rotation, permutation % 2 == 1).knowledge,
            expected,
            "R6 permutation {permutation} changed knowledge"
        );
    }
    thread::scope(|scope| {
        let workers = (0..8)
            .map(|worker| {
                scope.spawn(move || extract_fixture(worker * 3, worker % 2 == 1).knowledge)
            })
            .collect::<Vec<_>>();
        for worker in workers {
            assert_eq!(worker.join().expect("R6 replay worker"), expected);
        }
    });
}

#[test]
fn sec_fr_ext_011_never_executes_or_expands_target_worlds() {
    let graph = &extract_fixture(0, false).knowledge.graph;
    assert!(
        graph
            .relationships
            .iter()
            .all(|value| { value.kind == RelationshipKind::Defines })
    );
    assert!(graph.declarations.iter().all(|declaration| {
        graph.coverage.iter().any(|gap| {
            gap.declaration_id == declaration.id
                && gap.capability == "rust.framework_runtime_not_observed"
        })
    }));
    let debug = format!("{graph:#?}");
    for invented in [
        "Calls",
        "Executes",
        "Serves",
        "Starts",
        "Reaches",
        "Activates",
    ] {
        assert!(!debug.contains(invented));
    }
}

#[test]
fn sec_fr_ext_011_hard_negative_source_forms() {
    let graph = &extract_fixture(0, false).knowledge.graph;
    let debug = format!("{graph:#?}").to_ascii_lowercase();
    for decoy in [
        "comment_route_decoy",
        "string_route_decoy",
        "doc_route_decoy",
        "import_route_decoy",
        "name_only_route_decoy",
        "/unused",
        "generated_handler",
        "generated/framework.rs",
        "target/generated.rs",
    ] {
        assert!(!debug.contains(decoy), "hard negative leaked: {decoy}");
    }
}

#[test]
fn sec_fr_ext_011_comment_only_repository_emits_no_declaration() {
    let extraction = extract_source(
        "// Router::new().route(\"GET\", \"/comment-decoy\", handler)\n\
         pub const TEXT: &str = \"Router::new().route(\\\"GET\\\", \\\"/string-decoy\\\", handler)\";\n",
    )
    .expect("hard-negative-only repository must remain a valid R6 result");
    assert!(extraction.knowledge.graph.declarations.is_empty());
    assert!(extraction.knowledge.graph.relationships.is_empty());
    assert!(extraction.knowledge.graph.diagnostics.is_empty());
    assert!(extraction.knowledge.graph.coverage.is_empty());

    let unsupported = extract_source(
        "pub fn make_layer() {}\npub fn routes() { Router::new().layer(make_layer()) }\n",
    )
    .expect("unsupported target expression must remain unpromoted");
    assert!(unsupported.knowledge.graph.declarations.is_empty());
}

#[test]
fn pt_dr_idn_002_r6_nfc_collision_is_rejected() {
    let source = concat!(
        "pub fn handler() {}\n",
        "pub fn routes() {\n",
        "    Router::new()\n",
        "        .route(\"GET\", \"/café\", handler)\n",
        "        .route(\"GET\", \"/cafe\u{301}\", handler)\n",
        "}\n",
    );
    let error = extract_source(source).expect_err("NFC-equivalent declaration IDs must collide");
    let FrameworkError::IdentityConflict {
        normalized_preimage_sha256,
    } = error
    else {
        panic!("unexpected NFC collision error: {error:?}");
    };
    assert_eq!(normalized_preimage_sha256.len(), 64);
    assert!(
        normalized_preimage_sha256
            .bytes()
            .all(|value| value.is_ascii_hexdigit() && !value.is_ascii_uppercase())
    );
}

#[test]
fn sec_fr_ext_011_malformed_private_and_unsafe_inputs_are_typed() {
    for (source, reason) in [
        (
            "pub fn handler() {}\npub fn routes() { Router::new().route(method(), \"/\", handler) }\n",
            "reviewed_literal_required",
        ),
        (
            "pub fn handler() {}\npub fn other() {}\npub fn routes() { Router::new().route(\"/\", get(handler, other)) }\n",
            "malformed_method_wrapper",
        ),
        (
            "pub fn handler() {}\npub fn routes() { Router::new().route(\"GET\", \"https://user:secret@example.invalid\", handler) }\n",
            "private_locator_or_credential",
        ),
    ] {
        assert_eq!(
            extract_source(source).expect_err("invalid reviewed R6 form must fail"),
            FrameworkError::InvalidDeclaration {
                path: "src/lib.rs".to_owned(),
                reason: reason.to_owned(),
            }
        );
    }

    let inventory = synthetic_inventory(
        "pub struct Safe;\n",
        Some(("../outside.rs", b"pub struct Outside;\n".to_vec())),
    );
    assert_eq!(
        TreeSitterRustWorkspaceExtractor::new()
            .extract_rust_framework_declarations_incremental(&inventory, &[], &[])
            .expect_err("unsafe inventory path must fail before inherited extraction"),
        FrameworkError::UnsafePath {
            path: "repository-path".to_owned(),
            reason: "unsafe_path_component".to_owned(),
        }
    );
}

#[test]
fn pt_fr_ext_011_adapter_limits_have_max_and_plus_one() {
    let chain_limit = FrameworkLimit::ExplicitRegistrationChainSegments;
    let maximum_chain = usize::try_from(chain_limit.maximum()).expect("chain maximum");
    let exact_chain = builder_source(&[("routes", 0, maximum_chain)]);
    assert_eq!(
        extract_source(&exact_chain)
            .expect("maximum registration chain must be accepted")
            .knowledge
            .graph
            .declarations
            .len(),
        maximum_chain
    );
    assert_limit(
        &builder_source(&[("routes", 0, maximum_chain + 1)]),
        chain_limit,
    );
    let unrelated = unrelated_chain_source(maximum_chain + 1);
    assert!(
        extract_source(&unrelated)
            .expect("unrelated deep call chain must not consume the R6 builder limit")
            .knowledge
            .graph
            .declarations
            .is_empty()
    );

    let depth_limit = FrameworkLimit::RegistrationExpressionDepth;
    let nested_target = format!(
        "{}handler{}",
        "(".repeat(usize::try_from(depth_limit.maximum()).expect("depth maximum")),
        ")".repeat(usize::try_from(depth_limit.maximum()).expect("depth maximum"))
    );
    assert_limit(&route_source("GET", "/depth", &nested_target), depth_limit);

    for (limit, exact_source, plus_one_source) in [
        {
            let limit = FrameworkLimit::LiteralRoutePathBytes;
            let maximum = usize::try_from(limit.maximum()).expect("route path maximum");
            (
                limit,
                route_source("GET", &"p".repeat(maximum), "handler"),
                route_source("GET", &"p".repeat(maximum + 1), "handler"),
            )
        },
        {
            let limit = FrameworkLimit::LiteralMethodOrConfigurationKeyBytes;
            let maximum = usize::try_from(limit.maximum()).expect("method maximum");
            (
                limit,
                route_source(&"M".repeat(maximum), "/method", "handler"),
                route_source(&"M".repeat(maximum + 1), "/method", "handler"),
            )
        },
        {
            let limit = FrameworkLimit::TargetSpellingBytes;
            let maximum = usize::try_from(limit.maximum()).expect("target maximum");
            (
                limit,
                route_source("GET", "/target", &"t".repeat(maximum)),
                route_source("GET", "/target", &"t".repeat(maximum + 1)),
            )
        },
        {
            let limit = FrameworkLimit::AttributeTokenBytes;
            let maximum = usize::try_from(limit.maximum()).expect("attribute token maximum");
            (
                limit,
                attribute_token_source(maximum),
                attribute_token_source(maximum + 1),
            )
        },
    ] {
        extract_source(&exact_source).expect("exact R6 byte maximum must be accepted");
        assert_limit(&plus_one_source, limit);
    }

    let attribute_limit = FrameworkLimit::OuterAttributesPerDeclaration;
    let maximum_attributes = usize::try_from(attribute_limit.maximum()).expect("attribute maximum");
    extract_source(&outer_attribute_source(maximum_attributes))
        .expect("maximum outer attribute count must be accepted");
    assert_limit(
        &outer_attribute_source(maximum_attributes + 1),
        attribute_limit,
    );

    let declaration_limit = FrameworkLimit::FrameworkDeclarationsPerSource;
    let maximum_declarations =
        usize::try_from(declaration_limit.maximum()).expect("declaration maximum");
    let mut chains = Vec::new();
    let mut remaining = maximum_declarations + 1;
    let mut first_route = 0;
    let mut function = 0;
    while remaining > 0 {
        let count = remaining.min(maximum_chain);
        chains.push((format!("routes_{function}"), first_route, count));
        first_route += count;
        remaining -= count;
        function += 1;
    }
    let chain_refs = chains
        .iter()
        .map(|(name, start, count)| (name.as_str(), *start, *count))
        .collect::<Vec<_>>();
    assert_limit(&builder_source(&chain_refs), declaration_limit);
}

fn extract_fixture(
    rotation: usize,
    reverse: bool,
) -> codenoesis_domain::s4_r6::FrameworkExtraction {
    TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_framework_declarations_incremental(
            &fixture_inventory(rotation, reverse),
            &[],
            &[],
        )
        .expect("extract reviewed R6 framework-declarations fixture")
}

fn extract_source(
    source: &str,
) -> Result<codenoesis_domain::s4_r6::FrameworkExtraction, FrameworkError> {
    TreeSitterRustWorkspaceExtractor::new().extract_rust_framework_declarations_incremental(
        &synthetic_inventory(source, None),
        &[],
        &[],
    )
}

fn assert_limit(source: &str, limit: FrameworkLimit) {
    assert_eq!(
        extract_source(source).expect_err("maximum-plus-one R6 input must fail"),
        FrameworkError::LimitExceeded {
            limit,
            maximum: limit.maximum(),
            observed: limit.maximum() + 1,
        }
    );
}

fn route_source(method: &str, path: &str, target: &str) -> String {
    format!(
        "pub fn handler() {{}}\npub fn routes() {{ Router::new().route(\"{method}\", \"{path}\", {target}) }}\n"
    )
}

fn builder_source(chains: &[(&str, usize, usize)]) -> String {
    let mut source = String::from("pub fn handler() {}\n");
    for (name, start, count) in chains {
        write!(source, "pub fn {name}() {{\n    Router::new()")
            .expect("write synthetic builder root");
        for route in *start..start.saturating_add(*count) {
            write!(source, "\n        .route(\"GET\", \"/{route}\", handler)")
                .expect("write synthetic builder segment");
        }
        source.push_str("\n}\n");
    }
    source
}

fn unrelated_chain_source(count: usize) -> String {
    let mut source = String::from("pub fn unrelated() {\n    Other::new()");
    for _ in 0..count {
        source.push_str("\n        .step()");
    }
    source.push_str("\n}\n");
    source
}

fn outer_attribute_source(count: usize) -> String {
    format!("{}pub fn handler() {{}}\n", "#[route()]\n".repeat(count))
}

fn attribute_token_source(length: usize) -> String {
    const PREFIX: &str = "#[route(";
    const SUFFIX: &str = ")]";
    assert!(length >= PREFIX.len() + SUFFIX.len());
    format!(
        "{PREFIX}{}{SUFFIX}\npub fn handler() {{}}\n",
        "x".repeat(length - PREFIX.len() - SUFFIX.len())
    )
}

fn synthetic_inventory(source: &str, extra: Option<(&str, Vec<u8>)>) -> RepositoryInventory {
    let source = format!("pub struct R6Anchor {{ pub value: u8 }}\n{source}");
    let mut files = vec![
        AcquiredFile::new(
            "Cargo.toml".to_owned(),
            RegularFileMode::Regular,
            ObjectId::parse_sha1("cccccccccccccccccccccccccccccccccccccccc")
                .expect("synthetic manifest OID"),
            b"[package]\nname = \"fail-closed\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[lib]\npath = \"src/lib.rs\"\n"
                .to_vec(),
        ),
        AcquiredFile::new(
            "src/lib.rs".to_owned(),
            RegularFileMode::Regular,
            ObjectId::parse_sha1("dddddddddddddddddddddddddddddddddddddddd")
                .expect("synthetic source OID"),
            source.into_bytes(),
        ),
    ];
    if let Some((path, bytes)) = extra {
        files.push(AcquiredFile::new(
            path.to_owned(),
            RegularFileMode::Regular,
            ObjectId::parse_sha1("3333333333333333333333333333333333333333")
                .expect("synthetic extra OID"),
            bytes,
        ));
    }
    RepositoryInventory::classify(AcquiredRepository::new(
        BoundRevision::new(
            RepositoryIdentity::parse("urn:codenoesis:test:r5-fail-closed")
                .expect("synthetic repository identity"),
            ObjectId::parse_sha1("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                .expect("synthetic commit OID"),
            ObjectId::parse_sha1("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
                .expect("synthetic tree OID"),
        ),
        u64::try_from(files.len()).expect("synthetic file count"),
        files,
    ))
}

fn fixture_inventory(rotation: usize, reverse: bool) -> RepositoryInventory {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/s4/framework-declarations-v1/repository");
    let mut files = FIXTURE_FILES
        .into_iter()
        .map(|(path, blob_oid)| {
            AcquiredFile::new(
                path.to_owned(),
                RegularFileMode::Regular,
                ObjectId::parse_sha1(blob_oid).expect("reviewed R6 blob OID"),
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
            RepositoryIdentity::parse(REPOSITORY_ID).expect("reviewed R6 repository identity"),
            ObjectId::parse_sha1(COMMIT_OID).expect("reviewed R6 commit OID"),
            ObjectId::parse_sha1(TREE_OID).expect("reviewed R6 tree OID"),
        ),
        u64::try_from(files.len()).expect("reviewed R6 file count"),
        files,
    ))
}

fn read_reviewed_fixture(path: &Path) -> Vec<u8> {
    let bytes = fs::read(path).unwrap_or_else(|error| {
        panic!("read reviewed R6 fixture file {}: {error}", path.display())
    });
    normalize_reviewed_fixture_bytes(&bytes)
        .unwrap_or_else(|reason| panic!("invalid reviewed R6 fixture {}: {reason}", path.display()))
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
