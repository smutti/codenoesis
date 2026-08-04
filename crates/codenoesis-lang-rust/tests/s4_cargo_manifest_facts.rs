use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::thread;

use codenoesis_domain::knowledge::EntityKind;
use codenoesis_domain::s4::workspace_crate_id;
use codenoesis_domain::s4_r4::{
    CargoCoverageState, CargoEntityKind, CargoEntityProperties, CargoFactLimit, CargoFactReason,
    CargoManifestFactError, CargoRelationshipKind, DeclaredValue, R4_DETERMINISM_PERMUTATIONS,
    SourceAnalysisState, cargo_fact_limit_exceeded,
};
use codenoesis_domain::{
    AcquiredFile, AcquiredRepository, BoundRevision, ObjectId, RegularFileMode, RepositoryIdentity,
    RepositoryInventory,
};
use codenoesis_lang_rust::TreeSitterRustWorkspaceExtractor;
use codenoesis_ports::CargoManifestFactExtractor;

const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s4-cargo-manifest-facts-v1";
const COMMIT_OID: &str = "7b1fc9073552b5967b1620d1e082a1d45e1b380e";
const TREE_OID: &str = "c99449f6f0651e4f6398521e316f3500d0e508e7";

const FIXTURE_FILES: [(&str, &str); 8] = [
    ("Cargo.toml", "69ba91e42bc24201f35dddbe74fbac4ac029a290"),
    ("README.md", "af3999d46ab8bb426f99532ace2ca4162bda5181"),
    (
        "crates/app/Cargo.toml",
        "c9a7a7513824fa8ff7339e8fcde93b174f080890",
    ),
    (
        "crates/app/README.md",
        "8662ffb2455432929df573b71b4971e96ca53661",
    ),
    (
        "crates/app/build.rs",
        "71c512ae9f213a1d478d3a92befe755865ba81ba",
    ),
    (
        "crates/app/examples/demo.rs",
        "95ed7ad3c099ba852a4c1cda3f7236e9656f0252",
    ),
    (
        "crates/app/src/lib.rs",
        "c9e3ecc8d8db504669ff8eb007564ae543e285cc",
    ),
    (
        "crates/app/src/main.rs",
        "aa650416a681250a160d8acb99bd5a85a2f0837f",
    ),
];

#[test]
fn gt_fr_ext_009_manifest_declaration_entities() {
    let extraction = extract_fixture(0, false);
    let graph = &extraction.knowledge.graph;
    assert_eq!(graph.entities.len(), 23);
    assert_eq!(graph.relationships.len(), 25);
    assert_eq!(graph.claims.len(), 48);
    assert_eq!(graph.manifest_index.len(), 2);
    assert_eq!(
        graph
            .entities
            .iter()
            .map(codenoesis_domain::s4_r4::CargoEntity::kind)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            CargoEntityKind::Manifest,
            CargoEntityKind::WorkspacePackageDefaults,
            CargoEntityKind::Package,
            CargoEntityKind::Target,
            CargoEntityKind::Dependency,
            CargoEntityKind::Feature,
            CargoEntityKind::Patch,
            CargoEntityKind::BuildScript,
        ])
    );
    assert!(graph.entities.iter().any(|entity| {
        entity.id
            == "urn:codenoesis:entity:blake3:13c3407742a8aaf4ff919842621e5ef164757103a995146775b49441da886527"
            && matches!(
                &entity.properties,
                CargoEntityProperties::Target(properties)
                    if properties.manifest_path == "crates/app/Cargo.toml"
                        && properties.target_kind.as_str() == "example"
                        && properties.target_name == "demo"
                        && properties.source_analysis_state == SourceAnalysisState::NotAnalyzed
            )
    }));
    assert!(graph.evidence.iter().any(|evidence| {
        evidence.id
            == "urn:codenoesis:evidence:blake3:e2da79817c12805d7e6a2572b7d8064259e6d47935f0b36df67a7bad138695d2"
            && evidence.path == "crates/app/Cargo.toml"
            && evidence.blob_oid == "c9a7a7513824fa8ff7339e8fcde93b174f080890"
            && evidence.start_byte == 714
            && evidence.end_byte == 739
    }));
    assert!(graph.diagnostics.iter().any(|diagnostic| {
        diagnostic.id
            == "urn:codenoesis:diagnostic:blake3:7aa42489d042d8a69f88910cd472e635995c6e2eee6a81f3c3f3261897d5f486"
    }));
    assert!(graph.coverage.iter().any(|gap| {
        gap.id
            == "urn:codenoesis:coverage-gap:blake3:c7b525aa7556f66951e82b90829d7f4b7090cb03fdf44ee35824635d8bb804ba"
    }));
}

#[test]
fn gt_fr_ext_009_workspace_inheritance_references_declarations() {
    let extraction = extract_fixture(0, false);
    let graph = &extraction.knowledge.graph;
    let package_id = "urn:codenoesis:entity:blake3:86230638bf2b8bb2cc43ec80f62fda264d31efc524fa2cf018f35d04430b911a";
    let defaults_id = "urn:codenoesis:entity:blake3:4e7cfb30e61be8f92efd638f48c4053593dc0852b21fc0917a249199dec16cea";
    let package = graph
        .entities
        .iter()
        .find(|entity| entity.id == package_id)
        .expect("reviewed package declaration");
    let CargoEntityProperties::Package(properties) = &package.properties else {
        panic!("reviewed package kind changed");
    };
    let inherited = properties
        .metadata
        .iter()
        .filter(|fact| {
            matches!(
                &fact.value,
                DeclaredValue::WorkspaceReference {
                    source_entity_id,
                    ..
                } if source_entity_id == defaults_id
            )
        })
        .count();
    assert_eq!(inherited, 15);
    assert!(graph.relationships.iter().any(|relationship| {
        relationship.kind == CargoRelationshipKind::ReferencesDeclaration
            && relationship.source == package_id
            && relationship.target == defaults_id
            && relationship.id
                == "urn:codenoesis:relationship:blake3:89193efc764075a52c73316092497ac0597104b9ed2b2cba27a920a5d77dd9bf"
    }));
    assert!(graph.relationships.iter().any(|relationship| {
        relationship.kind == CargoRelationshipKind::ReferencesDeclaration
            && relationship.source
                == "urn:codenoesis:entity:blake3:1c028324ef57523788747f7b86aab44d73ee760ceb2db05152e3d1a89362d80e"
            && relationship.target
                == "urn:codenoesis:entity:blake3:e4628cfdd45f41efab5d3985b760b02c3a7c28b2d32ae16bd45831b66a30de75"
    }));
}

#[test]
fn pt_dr_idn_002_r4_preserves_rust_v3_identity_domains() {
    let r4 = extract_fixture(0, false);
    let plan = &r4.knowledge.workspace.plan;
    for target in &plan.targets {
        assert_eq!(
            target.crate_id,
            workspace_crate_id(
                REPOSITORY_ID,
                &target.manifest_path,
                &target.package_name,
                target.target_kind.as_str(),
                &target.target_name,
            )
        );
    }
    assert_eq!(
        stable_ids(
            r4.knowledge
                .workspace
                .knowledge
                .graph
                .entities
                .iter()
                .filter(|entity| entity.kind == EntityKind::RustCrate)
                .map(|value| value.id.as_str())
        ),
        BTreeSet::from([
            "urn:codenoesis:entity:blake3:f3140f3e0b98d098b027a7c1cc90cc4ad09430f49a9b8440b4d4073ff4b6e9f8",
            "urn:codenoesis:entity:blake3:f752a7c0eb3d4881c0fdc56bfb4df42a55a7f0b5a599b606223b9057736585b5",
        ])
    );
}

#[test]
fn pt_fr_ext_009_limits_have_max_and_plus_one() {
    for (limit, maximum) in [
        (CargoFactLimit::ManifestFactEntities, 10_000),
        (CargoFactLimit::DependenciesPerManifest, 256),
        (CargoFactLimit::FeaturesPerManifest, 256),
        (CargoFactLimit::FeatureMembersPerFeature, 128),
        (CargoFactLimit::TargetsPerPackage, 128),
        (CargoFactLimit::PatchesPerWorkspace, 256),
        (CargoFactLimit::MetadataFieldsPerOwner, 32),
        (CargoFactLimit::RequestedFeaturesPerDeclaration, 64),
        (CargoFactLimit::TargetPredicatesPerManifest, 128),
        (CargoFactLimit::DeclarationStringBytes, 2_048),
        (CargoFactLimit::ExternalLocatorBytes, 4_096),
    ] {
        assert_eq!(limit.maximum(), maximum);
        assert_eq!(
            cargo_fact_limit_exceeded(limit, maximum + 1),
            CargoManifestFactError::LimitExceeded {
                limit,
                maximum,
                observed: maximum + 1,
            }
        );
        assert_eq!(
            cargo_fact_limit_exceeded(limit, u64::MAX),
            CargoManifestFactError::LimitExceeded {
                limit,
                maximum,
                observed: maximum + 1,
            }
        );
    }

    let at_max = extract_inventory(&dependency_inventory(256))
        .expect("256 dependency declarations are supported");
    assert_eq!(
        at_max
            .knowledge
            .graph
            .entities
            .iter()
            .filter(|entity| entity.kind() == CargoEntityKind::Dependency)
            .count(),
        256
    );
    assert_eq!(
        extract_inventory(&dependency_inventory(257)),
        Err(CargoManifestFactError::LimitExceeded {
            limit: CargoFactLimit::DependenciesPerManifest,
            maximum: 256,
            observed: 257,
        })
    );
}

#[test]
fn negative_fr_ext_009_manifest_variants_fail_closed() {
    let escaping = manifest_inventory("[dependencies]\nescaping = { path = \"../../outside\" }\n");
    assert!(matches!(
        extract_inventory(&escaping),
        Err(CargoManifestFactError::InvalidFact {
            reason: CargoFactReason::InvalidRelativePath,
            ..
        })
    ));

    let conflicting = manifest_inventory(
        "[dependencies]\nconflict = { path = \"inside\", git = \"https://example.invalid/repo.git\" }\n",
    );
    assert!(matches!(
        extract_inventory(&conflicting),
        Err(CargoManifestFactError::InvalidFact {
            reason: CargoFactReason::ConflictingSourceSelectors,
            ..
        })
    ));

    let unicode_collision =
        manifest_inventory("[dependencies]\n\"Café\" = \"1\"\n\"Cafe\u{301}\" = \"1\"\n");
    let unicode_result = extract_inventory(&unicode_collision);
    assert!(
        matches!(
            &unicode_result,
            Err(CargoManifestFactError::Conflict { .. }
                | CargoManifestFactError::InvalidFact {
                    reason: CargoFactReason::UnicodeNormalizationCollision,
                    ..
                })
        ),
        "unexpected Unicode collision result: {unicode_result:?}"
    );
}

#[test]
fn pt_nfr_det_001_r4_permutation_and_schedule_invariant() {
    let expected = extract_fixture(0, false).knowledge;
    for permutation in 0..R4_DETERMINISM_PERMUTATIONS {
        let rotation = usize::try_from(permutation).expect("permutation index");
        assert_eq!(
            extract_fixture(rotation, permutation % 2 == 1).knowledge,
            expected,
            "R4 permutation {permutation} changed knowledge"
        );
    }
    thread::scope(|scope| {
        let handles = (0..8)
            .map(|worker| {
                scope.spawn(move || extract_fixture(worker * 3, worker % 2 == 1).knowledge)
            })
            .collect::<Vec<_>>();
        for handle in handles {
            assert_eq!(handle.join().expect("R4 replay worker"), expected);
        }
    });
}

#[test]
fn sec_fr_ext_009_external_locators_are_digest_only() {
    let debug = format!("{:#?}", extract_fixture(0, false).knowledge);
    for digest in [
        "091362da76784f8483687654ce191598047e726e17d2f7d2b7bff0e41a866c7b",
        "12a25223ba69da250347769233115f8e36107791a3977fe9176d5f04e56562e0",
        "96785b267fe3b6bbce8ffad1ff44fd67679c993e28c6e243cbebf4846d0aa981",
        "b3c8586c3b834e73b7edbb2dd75c936d798e00ba781ee10aee7e31616cc7b323",
        "c694e9cf2813be58bafc73ccea356e4ca7ea59fe8e783cfd8b8d1412d9c3b594",
        "cdd511bfab82a3e0b8cd6588b318ea97a24458549aed4e6e5b749cf3d3e149fa",
        "f76a4c531d3520bc86a5c99a8e5d8d9ab87fe5b185b6408baca70c5a06818e58",
    ] {
        assert!(debug.contains(digest), "missing locator digest {digest}");
    }
    for plaintext in [
        "fixture-token",
        "member-token",
        "https://example.invalid/serde.git",
        "0123456789abcdef0123456789abcdef01234567",
        "v2.0.0",
        "stable",
    ] {
        assert!(
            !debug.contains(plaintext),
            "locator plaintext leaked: {plaintext}"
        );
    }
}

#[test]
fn sec_fr_ext_009_manifest_facts_never_resolve_fetch_or_execute() {
    let extraction = extract_fixture(0, false);
    let graph = &extraction.knowledge.graph;
    assert!(graph.relationships.iter().all(|relationship| matches!(
        relationship.kind,
        CargoRelationshipKind::Declares
            | CargoRelationshipKind::ReferencesDeclaration
            | CargoRelationshipKind::Materializes
    )));
    assert!(graph.coverage.iter().any(|gap| {
        gap.capability == "cargo.dependency_graph_not_resolved"
            && gap.state == CargoCoverageState::NotResolved
    }));
    assert!(graph.coverage.iter().any(|gap| {
        gap.capability == "cargo.dependency_source_not_fetched"
            && gap.state == CargoCoverageState::NotFetched
    }));
    assert!(graph.coverage.iter().any(|gap| {
        gap.capability == "cargo.build_script_not_executed"
            && gap.state == CargoCoverageState::NotExecuted
    }));
    assert!(graph.coverage.iter().any(|gap| {
        gap.capability == "cargo.patch_not_applied" && gap.state == CargoCoverageState::NotApplied
    }));
    let debug = format!("{:#?}", extraction.knowledge);
    for forbidden in [
        "DEPENDS_ON",
        "RESOLVES_TO",
        "ACTIVATES",
        "SELECTS_TARGET",
        "APPLIES_PATCH",
        "EXECUTES",
    ] {
        assert!(!debug.contains(forbidden));
    }
}

fn extract_fixture(
    rotation: usize,
    reverse: bool,
) -> codenoesis_domain::s4_r4::CargoManifestFactExtraction {
    TreeSitterRustWorkspaceExtractor::new()
        .extract_cargo_manifest_facts_incremental(&fixture_inventory(rotation, reverse), &[], &[])
        .expect("extract reviewed R4 manifest fixture")
}

fn extract_inventory(
    inventory: &RepositoryInventory,
) -> Result<codenoesis_domain::s4_r4::CargoManifestFactExtraction, CargoManifestFactError> {
    TreeSitterRustWorkspaceExtractor::new().extract_cargo_manifest_facts_incremental(
        inventory,
        &[],
        &[],
    )
}

fn fixture_inventory(rotation: usize, reverse: bool) -> RepositoryInventory {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/s4/cargo-manifest-facts-v1/repository");
    let mut files = FIXTURE_FILES
        .into_iter()
        .map(|(path, blob_oid)| {
            AcquiredFile::new(
                path.to_owned(),
                RegularFileMode::Regular,
                ObjectId::parse_sha1(blob_oid).expect("reviewed R4 blob OID"),
                fs::read(root.join(path)).expect("read reviewed R4 fixture file"),
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
            RepositoryIdentity::parse(REPOSITORY_ID).expect("reviewed R4 repository identity"),
            ObjectId::parse_sha1(COMMIT_OID).expect("reviewed R4 commit OID"),
            ObjectId::parse_sha1(TREE_OID).expect("reviewed R4 tree OID"),
        ),
        u64::try_from(files.len()).expect("reviewed R4 file count"),
        files,
    ))
}

fn dependency_inventory(count: usize) -> RepositoryInventory {
    let dependencies = (0..count)
        .map(|index| format!("dep{index:03} = \"1\""))
        .collect::<Vec<_>>()
        .join("\n");
    manifest_inventory(&format!("[dependencies]\n{dependencies}\n"))
}

fn manifest_inventory(extra: &str) -> RepositoryInventory {
    let manifest = format!(
        "[package]\nname = \"limit-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[lib]\npath = \"src/lib.rs\"\n\n{extra}"
    );
    let files = [
        ("Cargo.toml", manifest.into_bytes()),
        ("src/lib.rs", b"pub struct Item;\n".to_vec()),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (path, bytes))| {
        AcquiredFile::new(
            path.to_owned(),
            RegularFileMode::Regular,
            ObjectId::parse_sha1(&format!("{:040x}", index + 1)).expect("synthetic blob OID"),
            bytes,
        )
    })
    .collect::<Vec<_>>();
    RepositoryInventory::classify(AcquiredRepository::new(
        BoundRevision::new(
            RepositoryIdentity::parse("urn:codenoesis:test:r4-manifest-variant")
                .expect("synthetic repository identity"),
            ObjectId::parse_sha1("1111111111111111111111111111111111111111")
                .expect("synthetic commit OID"),
            ObjectId::parse_sha1("2222222222222222222222222222222222222222")
                .expect("synthetic tree OID"),
        ),
        u64::try_from(files.len()).expect("synthetic file count"),
        files,
    ))
}

fn stable_ids<'a>(values: impl IntoIterator<Item = &'a str>) -> BTreeSet<&'a str> {
    values.into_iter().collect()
}
