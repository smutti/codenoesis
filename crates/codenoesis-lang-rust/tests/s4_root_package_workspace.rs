use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::thread;

use codenoesis_domain::knowledge::EntityKind;
use codenoesis_domain::s4_r3::{
    ExternalWorkspaceBoundary, R3_COVERAGE_CAPABILITIES, RootPackageLimit, RootPackageShape,
    RootPackageWorkspaceError, WorkspaceManifestReason, WorkspaceMemberSource,
};
use codenoesis_domain::{
    AcquiredFile, AcquiredRepository, BoundRevision, ObjectId, RegularFileMode, RepositoryIdentity,
    RepositoryInventory,
};
use codenoesis_lang_rust::TreeSitterRustWorkspaceExtractor;
use codenoesis_ports::{RootPackageWorkspaceExtractor, RustWorkspaceExtractor};

const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s4-root-package-workspace-v1";
const COMMIT_OID: &str = "37eb6d1abf25891c52fbdf9b735973c441a8598b";
const TREE_OID: &str = "8295d04a96f8c2af48cc7492a797080ea08cf2ea";

#[test]
#[allow(clippy::too_many_lines)]
fn gt_fr_ext_008_root_membership_and_targets() {
    let implicit = extract_fixture("implicit").expect("extract implicit non-virtual workspace");
    assert_eq!(
        implicit.knowledge.plan.root_shape,
        RootPackageShape::NonVirtualWorkspace
    );
    assert_eq!(
        implicit
            .knowledge
            .plan
            .members
            .iter()
            .map(|member| member.path.as_str())
            .collect::<Vec<_>>(),
        [".", "crates/cli", "crates/macro-sentinel", "external/model"]
    );
    let expected_targets = [
        (
            "Cargo.toml",
            "lib",
            "root_app",
            "src/lib.rs",
            "urn:codenoesis:entity:blake3:6f20a1ab8dc60551d001178172368742238429b975c2740e41cce9975f04a4dd",
        ),
        (
            "Cargo.toml",
            "bin",
            "root-admin",
            "src/bin/admin.rs",
            "urn:codenoesis:entity:blake3:7619b82a3299db9236b4a5fb8955f38c8ce8bb388391448ddc2df5dbbc8e7249",
        ),
        (
            "Cargo.toml",
            "bin",
            "root-app",
            "src/main.rs",
            "urn:codenoesis:entity:blake3:819b0a4083e3c4553c559735f1d0f2a07a1ad01fdeeaf180669d4e178564e0d5",
        ),
        (
            "crates/cli/Cargo.toml",
            "bin",
            "inspect",
            "crates/cli/src/bin/inspect/main.rs",
            "urn:codenoesis:entity:blake3:f6f2630c8a4b7f71c01be72c910bdbca56bae8c43bc8ff9e51489257801ba092",
        ),
        (
            "crates/cli/Cargo.toml",
            "bin",
            "root-cli",
            "crates/cli/src/main.rs",
            "urn:codenoesis:entity:blake3:ae8f827c9b5e76a5cdf198c8ca6dd2e7b480068c75b0251972158d9f5f85b253",
        ),
        (
            "crates/macro-sentinel/Cargo.toml",
            "lib",
            "macro_sentinel",
            "crates/macro-sentinel/src/lib.rs",
            "urn:codenoesis:entity:blake3:e3ff23bf52b9bc47ecb50ffd231109e9f51c891444a3234c7678b8a0c37f2edd",
        ),
    ];
    assert_eq!(
        implicit
            .knowledge
            .plan
            .targets
            .iter()
            .map(|target| (
                target.manifest_path.as_str(),
                target.target_kind.as_str(),
                target.target_name.as_str(),
                target.source_path.as_str(),
                target.crate_id.as_str(),
            ))
            .collect::<Vec<_>>(),
        expected_targets
    );

    let explicit = extract_fixture("explicit-dot").expect("extract explicit root member");
    assert_eq!(
        explicit
            .knowledge
            .plan
            .targets
            .iter()
            .map(|target| target.crate_id.as_str())
            .collect::<Vec<_>>(),
        implicit
            .knowledge
            .plan
            .targets
            .iter()
            .map(|target| target.crate_id.as_str())
            .collect::<Vec<_>>(),
        "membership provenance must not enter crate identities"
    );
    assert_eq!(
        explicit.knowledge.plan.members[0].member_source,
        WorkspaceMemberSource::ExplicitRootMember
    );
    assert_eq!(
        extract_fixture("standalone")
            .expect("extract standalone root package")
            .knowledge
            .plan
            .root_shape,
        RootPackageShape::StandaloneRootPackage
    );
    assert_eq!(
        extract_fixture("virtual")
            .expect("extract virtual workspace through R3")
            .knowledge
            .plan
            .root_shape,
        RootPackageShape::VirtualWorkspace
    );
    assert_eq!(
        extract_fixture("member-exclude-conflict"),
        Err(RootPackageWorkspaceError::MemberConflict {
            path: "crates/cli".to_owned(),
        })
    );
}

#[test]
fn pt_dr_idn_002_r3_preserves_v2_identity_domains() {
    let inventory = legacy_workspace_inventory();
    let extractor = TreeSitterRustWorkspaceExtractor::new();
    let v2 = extractor
        .extract_workspace(&inventory)
        .expect("extract reviewed ontology v2 fixture");
    let v3 = extractor
        .extract_root_package_workspace_incremental(&inventory, &[], &[])
        .expect("extract reviewed ontology v3 fixture");
    assert_eq!(
        stable_ids(v2.graph.entities.iter().map(|entity| entity.id.as_str())),
        stable_ids(
            v3.knowledge
                .knowledge
                .graph
                .entities
                .iter()
                .map(|entity| entity.id.as_str())
        )
    );
    assert_eq!(
        stable_ids(
            v2.graph
                .relationships
                .iter()
                .map(|relationship| relationship.id.as_str())
        ),
        stable_ids(
            v3.knowledge
                .knowledge
                .graph
                .relationships
                .iter()
                .map(|relationship| relationship.id.as_str())
        )
    );
}

#[test]
fn pt_fr_ext_008_limits_have_max_and_plus_one() {
    let extractor = TreeSitterRustWorkspaceExtractor::new();

    let (members_at_max, boundaries_at_max) = external_member_inventory(200, 0);
    let projected = extractor
        .extract_root_package_workspace_incremental(&members_at_max, &boundaries_at_max, &[])
        .expect("200 literals plus one implicit root are supported");
    assert_eq!(projected.knowledge.plan.members.len(), 201);
    let (members_plus_one, boundaries_plus_one) = external_member_inventory(201, 0);
    assert_limit(
        extractor.extract_root_package_workspace_incremental(
            &members_plus_one,
            &boundaries_plus_one,
            &[],
        ),
        RootPackageLimit::WorkspaceMembers,
        200,
        201,
    );

    let (exclusions_at_max, no_boundaries) = external_member_inventory(0, 200);
    extractor
        .extract_root_package_workspace_incremental(&exclusions_at_max, &no_boundaries, &[])
        .expect("200 exclusions are supported");
    let (exclusions_plus_one, no_boundaries) = external_member_inventory(0, 201);
    assert_limit(
        extractor.extract_root_package_workspace_incremental(
            &exclusions_plus_one,
            &no_boundaries,
            &[],
        ),
        RootPackageLimit::WorkspaceExclusions,
        200,
        201,
    );

    extractor
        .extract_root_package_workspace_incremental(&package_inventory(200), &[], &[])
        .expect("200 package manifests are supported");
    assert_limit(
        extractor.extract_root_package_workspace_incremental(&package_inventory(201), &[], &[]),
        RootPackageLimit::PackageManifests,
        200,
        201,
    );

    extractor
        .extract_root_package_workspace_incremental(&target_inventory(&[64, 64, 64, 8]), &[], &[])
        .expect("200 crate targets are supported");
    assert_limit(
        extractor.extract_root_package_workspace_incremental(
            &target_inventory(&[64, 64, 64, 9]),
            &[],
            &[],
        ),
        RootPackageLimit::WorkspaceCrates,
        200,
        201,
    );

    extractor
        .extract_root_package_workspace_incremental(&target_inventory(&[64]), &[], &[])
        .expect("64 binary roots are supported");
    assert_limit(
        extractor.extract_root_package_workspace_incremental(&target_inventory(&[65]), &[], &[]),
        RootPackageLimit::BinaryRootsPerPackage,
        64,
        65,
    );

    extractor
        .extract_root_package_workspace_incremental(&manifest_size_inventory(4_194_304), &[], &[])
        .expect("maximum manifest bytes are supported");
    assert_limit(
        extractor.extract_root_package_workspace_incremental(
            &manifest_size_inventory(4_194_305),
            &[],
            &[],
        ),
        RootPackageLimit::SingleManifestBytes,
        4_194_304,
        4_194_305,
    );
}

#[test]
fn pt_nfr_det_001_r3_permutation_and_schedule_invariant() {
    let expected = extract_fixture_with_order("implicit", 0, false)
        .expect("extract canonical fixture")
        .knowledge;
    for permutation in 0..50 {
        let observed = extract_fixture_with_order("implicit", permutation, permutation % 2 == 1)
            .expect("extract permuted fixture")
            .knowledge;
        assert_eq!(observed, expected, "permutation {permutation} changed R3");
    }
    let handles = (0..8)
        .map(|schedule| {
            thread::spawn(move || {
                extract_fixture_with_order("implicit", schedule, schedule % 2 == 0)
                    .expect("parallel R3 replay")
                    .knowledge
            })
        })
        .collect::<Vec<_>>();
    for handle in handles {
        assert_eq!(handle.join().expect("join R3 replay"), expected);
    }
}

#[test]
fn sec_fr_ext_008_deferred_cargo_meaning_never_executes() {
    let implicit = extract_fixture("implicit").expect("sentinel fixture remains data only");
    let virtual_workspace =
        extract_fixture("virtual").expect("workspace inheritance remains data only");
    let capabilities = implicit
        .knowledge
        .knowledge
        .graph
        .coverage
        .iter()
        .chain(virtual_workspace.knowledge.knowledge.graph.coverage.iter())
        .map(|gap| gap.capability.as_str())
        .collect::<BTreeSet<_>>();
    for capability in R3_COVERAGE_CAPABILITIES {
        assert!(capabilities.contains(capability), "missing {capability}");
    }
}

#[test]
fn sec_fr_ext_008_gitlink_member_stays_external() {
    let extraction = extract_fixture("implicit").expect("extract R2-composed fixture");
    let external = extraction
        .knowledge
        .plan
        .members
        .iter()
        .find(|member| member.path == "external/model")
        .expect("reviewed external member");
    assert!(external.manifest_path.is_none());
    assert!(external.crate_ids.is_empty());
    assert!(external.external_boundary_id.is_some());
    assert!(
        extraction
            .knowledge
            .knowledge
            .graph
            .entities
            .iter()
            .all(|entity| !entity.name.starts_with("external/model"))
    );
}

#[test]
fn sec_fr_ext_008_compiler_dependent_source_is_deferred() {
    let inventory = synthetic_inventory(vec![
        (
            "Cargo.toml".to_owned(),
            b"[package]\nname=\"root\"\nedition=\"2024\"\n".to_vec(),
        ),
        (
            "src/lib.rs".to_owned(),
            br#"#[cfg(target_os = "linux")]
pub struct Platform;
#[cfg(not(target_os = "linux"))]
pub struct Platform;
mod generated;
use external::One;
use external::Two;
pub struct Stable;
"#
            .to_vec(),
        ),
    ]);

    let extraction = TreeSitterRustWorkspaceExtractor::new()
        .extract_root_package_workspace_incremental(&inventory, &[], &[])
        .expect("defer compiler-dependent source facts");
    let graph = &extraction.knowledge.knowledge.graph;
    assert!(
        graph
            .entities
            .iter()
            .any(|entity| { entity.kind == EntityKind::RustStruct && entity.name == "Stable" })
    );
    assert!(
        graph.entities.iter().all(|entity| {
            !(entity.kind == EntityKind::RustStruct && entity.name == "Platform")
        })
    );
    assert!(graph.entities.iter().all(|entity| {
        !(entity.kind == EntityKind::RustModule && entity.name.ends_with("::generated"))
    }));
    assert!(
        graph
            .coverage
            .iter()
            .any(|gap| gap.capability == "rust_unsupported_construct")
    );
    assert_eq!(
        graph.coverage.len(),
        graph
            .coverage
            .iter()
            .map(|gap| gap.id.as_str())
            .collect::<BTreeSet<_>>()
            .len()
    );
}

#[test]
fn gt_fr_ext_008_invalid_manifests_fail_closed() {
    assert_invalid_manifest(
        vec![("Cargo.toml".to_owned(), b"[package\n".to_vec())],
        WorkspaceManifestReason::MalformedToml,
        Some("Cargo.toml"),
    );
    assert_invalid_manifest(
        vec![("Cargo.toml".to_owned(), vec![0xff, 0xfe])],
        WorkspaceManifestReason::MalformedToml,
        Some("Cargo.toml"),
    );
    assert_invalid_manifest(
        root_files(
            b"[package]\nname=\"root\"\nedition=\"2024\"\nautobins=false\n[lib]\npath=\"src/lib.rs\"\n",
        ),
        WorkspaceManifestReason::UnsupportedStructuralKey,
        Some("Cargo.toml"),
    );
    for member in ["../escape", "member/*", "/absolute"] {
        let manifest = format!("[workspace]\nmembers=[\"{member}\"]\n");
        assert_invalid_manifest(
            vec![("Cargo.toml".to_owned(), manifest.into_bytes())],
            WorkspaceManifestReason::InvalidMemberPath,
            Some("Cargo.toml"),
        );
    }
    assert_invalid_manifest(
        vec![(
            "Cargo.toml".to_owned(),
            b"[workspace]\nmembers=['member\\child']\n".to_vec(),
        )],
        WorkspaceManifestReason::InvalidMemberPath,
        Some("Cargo.toml"),
    );
    assert_invalid_manifest(
        vec![(
            "Cargo.toml".to_owned(),
            b"[workspace]\nmembers=[\"member\",\"member\"]\n".to_vec(),
        )],
        WorkspaceManifestReason::InvalidMemberPath,
        Some("Cargo.toml"),
    );
    assert_invalid_manifest(
        vec![(
            "Cargo.toml".to_owned(),
            b"[workspace]\nmembers=[\"member-b\",\"member-a\"]\n".to_vec(),
        )],
        WorkspaceManifestReason::MissingMemberManifest,
        Some("member-a/Cargo.toml"),
    );
    let unicode_manifest = "[workspace]\nmembers=[\"café\",\"cafe\u{301}\"]\n";
    assert_invalid_manifest(
        vec![(
            "Cargo.toml".to_owned(),
            unicode_manifest.as_bytes().to_vec(),
        )],
        WorkspaceManifestReason::UnicodeNormalizationCollision,
        Some("Cargo.toml"),
    );
    assert_invalid_manifest(
        vec![
            (
                "Cargo.toml".to_owned(),
                b"[package]\nname=\"root\"\nedition=\"2024\"\n".to_vec(),
            ),
            ("src/bin/tool.rs".to_owned(), b"fn main() {}\n".to_vec()),
            (
                "src/bin/tool/main.rs".to_owned(),
                b"fn main() {}\n".to_vec(),
            ),
        ],
        WorkspaceManifestReason::AmbiguousConventionalTarget,
        Some("src/bin/tool/main.rs"),
    );

    let first = target_conflict_inventory(["src/bin/a.rs", "src/bin/z.rs"]);
    let second = target_conflict_inventory(["src/bin/z.rs", "src/bin/a.rs"]);
    for inventory in [first, second] {
        assert_eq!(
            TreeSitterRustWorkspaceExtractor::new().extract_root_package_workspace_incremental(
                &inventory,
                &[],
                &[]
            ),
            Err(RootPackageWorkspaceError::TargetConflict {
                path: "src/bin/a.rs".to_owned(),
                target_kind: codenoesis_domain::s4_r3::WorkspaceTargetKind::Binary,
                target_name: "duplicate".to_owned(),
            })
        );
    }
}

fn extract_fixture(
    variant: &str,
) -> Result<codenoesis_domain::s4_r3::RootPackageWorkspaceExtraction, RootPackageWorkspaceError> {
    extract_fixture_with_order(variant, 0, false)
}

fn extract_fixture_with_order(
    variant: &str,
    rotation: usize,
    reverse: bool,
) -> Result<codenoesis_domain::s4_r3::RootPackageWorkspaceExtraction, RootPackageWorkspaceError> {
    let inventory = fixture_inventory(variant, rotation, reverse);
    let boundaries = if matches!(variant, "implicit" | "explicit-dot" | "virtual") {
        vec![ExternalWorkspaceBoundary {
            path: "external/model".to_owned(),
            boundary_id: format!(
                "urn:codenoesis:repository-boundary:sha256:{}",
                "a".repeat(64)
            ),
        }]
    } else {
        Vec::new()
    };
    TreeSitterRustWorkspaceExtractor::new().extract_root_package_workspace_incremental(
        &inventory,
        &boundaries,
        &[],
    )
}

fn fixture_inventory(variant: &str, rotation: usize, reverse: bool) -> RepositoryInventory {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/s4/root-package-workspace-v1");
    let (root_manifest, root_blob) = match variant {
        "implicit" => (
            "root-manifests/implicit.toml",
            "50752bfc12630cf02e108a82695e3380383d3bed",
        ),
        "explicit-dot" => (
            "root-manifests/explicit-dot.toml",
            "49580b9295e54dbc6e38812cad1793393f252273",
        ),
        "standalone" => (
            "root-manifests/standalone.toml",
            "c793a514ddff6d4e18627d63806290cca1c0f825",
        ),
        "virtual" => (
            "root-manifests/virtual.toml",
            "b49cce7433924d2fa9b6e6e53e950607b4cc7589",
        ),
        "member-exclude-conflict" => (
            "root-manifests/member-exclude-conflict.toml",
            "e253a6a2d925b9e2616d71ffc9bc4d5d0578816c",
        ),
        _ => panic!("unknown fixture variant"),
    };
    let mut files = vec![fixture_file(&root, root_manifest, "Cargo.toml", root_blob)];
    for (path, blob_oid) in [
        (
            "shared-tree/.gitmodules",
            "e126cd695ba05a606db72d95deffa187aaf8709d",
        ),
        (
            "shared-tree/build.rs",
            "31ac777d8dc6c0e692a02d8fdcbae464c8d564aa",
        ),
        (
            "shared-tree/crates/cli/Cargo.toml",
            "1d8c9f33a79a9f44899f1946b214f3812cbe09d0",
        ),
        (
            "shared-tree/crates/cli/src/bin/inspect/main.rs",
            "7458f21bda0382f14c8c088cefb2d34068722e3d",
        ),
        (
            "shared-tree/crates/cli/src/main.rs",
            "d8c32ec762591814b1af1c5710785ae06c868580",
        ),
        (
            "shared-tree/crates/macro-sentinel/Cargo.toml",
            "65e5f14a530d1c29bae9935e9665a9b1263c21df",
        ),
        (
            "shared-tree/crates/macro-sentinel/src/lib.rs",
            "298f4c20f0d4a04d08233d144fcf4e2bdc614eb5",
        ),
        (
            "shared-tree/src/bin/admin.rs",
            "ca0b06346cb575e8e4505bb818d5cb8b16cc2d36",
        ),
        (
            "shared-tree/src/lib.rs",
            "d21e8dc21f9b510da4a2335b60824595e55d3009",
        ),
        (
            "shared-tree/src/main.rs",
            "ac31ce1326587fed2921c810fb35f0a48467bbfc",
        ),
        (
            "shared-tree/src/model.rs",
            "07e93b30c8b338233096a109de035fdcd1478572",
        ),
    ] {
        files.push(fixture_file(
            &root,
            path,
            path.strip_prefix("shared-tree/")
                .expect("shared fixture path"),
            blob_oid,
        ));
    }
    if !files.is_empty() {
        let length = files.len();
        files.rotate_left(rotation % length);
    }
    if reverse {
        files.reverse();
    }
    inventory(REPOSITORY_ID, COMMIT_OID, TREE_OID, files)
}

fn fixture_file(root: &Path, source: &str, destination: &str, blob_oid: &str) -> AcquiredFile {
    AcquiredFile::new(
        destination.to_owned(),
        RegularFileMode::Regular,
        oid(blob_oid),
        fs::read(root.join(source)).expect("read reviewed R3 fixture"),
    )
}

fn legacy_workspace_inventory() -> RepositoryInventory {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/s4/workspace-docs-v1/revision-a");
    let files = [
        ("Cargo.toml", "9a61bc964dd0b80e54880d0a40471b50324b6e15"),
        (
            "crates/app/Cargo.toml",
            "887b09de92b06c4003daf2ffc60a48e33aa1187f",
        ),
        (
            "crates/app/build.rs",
            "817cc01b6a06743e810217ca57f9abfbbdf34824",
        ),
        (
            "crates/app/src/main.rs",
            "7e6f91e233eff28c5511d686f7c46f67dd919505",
        ),
        (
            "crates/model/Cargo.toml",
            "bd9cc68a4885e014ce4a034be8c2317425f3e3dc",
        ),
        (
            "crates/model/src/item.rs",
            "885b4746097a67e5c4fb997a2082597dc23699e6",
        ),
        (
            "crates/model/src/lib.rs",
            "f001952aa75e3c631153f1fe5dec0af6ca11c429",
        ),
    ]
    .into_iter()
    .map(|(path, blob_oid)| {
        AcquiredFile::new(
            path.to_owned(),
            RegularFileMode::Regular,
            oid(blob_oid),
            fs::read(root.join(path)).expect("read reviewed S4 fixture"),
        )
    })
    .collect();
    inventory(
        "urn:codenoesis:fixture:s4-workspace-docs-v1",
        "c09d8c24e4704036c31b4f42e2f4df6e4acd347f",
        "8f9e36122bec5caac5dc0f739ea7ab4c830bd356",
        files,
    )
}

fn external_member_inventory(
    member_count: usize,
    exclusion_count: usize,
) -> (RepositoryInventory, Vec<ExternalWorkspaceBoundary>) {
    let members = (0..member_count)
        .map(|index| format!("\"external/{index:03}\""))
        .collect::<Vec<_>>()
        .join(",");
    let exclusions = (0..exclusion_count)
        .map(|index| format!("\"excluded/{index:03}\""))
        .collect::<Vec<_>>()
        .join(",");
    let manifest = format!(
        "[package]\nname=\"root\"\nedition=\"2024\"\n[workspace]\nmembers=[{members}]\nexclude=[{exclusions}]\n[lib]\npath=\"src/lib.rs\"\n"
    );
    let boundaries = (0..member_count)
        .map(|index| ExternalWorkspaceBoundary {
            path: format!("external/{index:03}"),
            boundary_id: format!(
                "urn:codenoesis:repository-boundary:sha256:{:064x}",
                index + 1
            ),
        })
        .collect();
    (
        synthetic_inventory(vec![
            ("Cargo.toml".to_owned(), manifest.into_bytes()),
            ("src/lib.rs".to_owned(), b"pub struct Root;\n".to_vec()),
        ]),
        boundaries,
    )
}

fn package_inventory(package_count: usize) -> RepositoryInventory {
    let member_paths = (1..package_count)
        .map(|index| format!("member-{index:03}"))
        .collect::<Vec<_>>();
    let members = member_paths
        .iter()
        .map(|path| format!("\"{path}\""))
        .collect::<Vec<_>>()
        .join(",");
    let root_manifest = format!(
        "[package]\nname=\"root\"\nedition=\"2024\"\n[workspace]\nmembers=[{members}]\n[lib]\npath=\"src/lib.rs\"\n"
    );
    let mut files = vec![
        ("Cargo.toml".to_owned(), root_manifest.into_bytes()),
        ("src/lib.rs".to_owned(), b"pub struct Item;\n".to_vec()),
    ];
    for (index, member) in member_paths.iter().enumerate() {
        files.push((
            format!("{member}/Cargo.toml"),
            format!(
                "[package]\nname=\"member-{index:03}\"\nedition=\"2024\"\n[lib]\npath=\"src/lib.rs\"\n"
            )
            .into_bytes(),
        ));
        files.push((
            format!("{member}/src/lib.rs"),
            b"pub struct Item;\n".to_vec(),
        ));
    }
    synthetic_inventory(files)
}

fn target_inventory(binary_counts: &[usize]) -> RepositoryInventory {
    let member_paths = (1..binary_counts.len())
        .map(|index| format!("member-{index:03}"))
        .collect::<Vec<_>>();
    let members = member_paths
        .iter()
        .map(|path| format!("\"{path}\""))
        .collect::<Vec<_>>()
        .join(",");
    let mut files = Vec::new();
    for (package_index, binary_count) in binary_counts.iter().copied().enumerate() {
        let member = if package_index == 0 {
            ".".to_owned()
        } else {
            member_paths[package_index - 1].clone()
        };
        let package_name = format!("package-{package_index:03}");
        let mut manifest = format!("[package]\nname=\"{package_name}\"\nedition=\"2024\"\n");
        if package_index == 0 && !member_paths.is_empty() {
            writeln!(manifest, "[workspace]\nmembers=[{members}]").expect("write workspace");
        }
        for binary_index in 0..binary_count {
            writeln!(
                manifest,
                "[[bin]]\nname=\"bin-{binary_index:03}\"\npath=\"src/bin/bin-{binary_index:03}.rs\""
            )
            .expect("write binary target");
            files.push((
                join_member(&member, &format!("src/bin/bin-{binary_index:03}.rs")),
                b"fn main() {}\n".to_vec(),
            ));
        }
        files.push((join_member(&member, "Cargo.toml"), manifest.into_bytes()));
    }
    synthetic_inventory(files)
}

fn manifest_size_inventory(byte_length: usize) -> RepositoryInventory {
    let mut manifest =
        b"[package]\nname=\"root\"\nedition=\"2024\"\n[lib]\npath=\"src/lib.rs\"\n#".to_vec();
    manifest.resize(byte_length.saturating_sub(1), b'x');
    manifest.push(b'\n');
    assert_eq!(manifest.len(), byte_length);
    synthetic_inventory(vec![
        ("Cargo.toml".to_owned(), manifest),
        ("src/lib.rs".to_owned(), b"pub struct Root;\n".to_vec()),
    ])
}

fn synthetic_inventory(files: Vec<(String, Vec<u8>)>) -> RepositoryInventory {
    let files = files
        .into_iter()
        .enumerate()
        .map(|(index, (path, bytes))| {
            AcquiredFile::new(
                path,
                RegularFileMode::Regular,
                oid(&format!("{:040x}", index + 1)),
                bytes,
            )
        })
        .collect();
    inventory(
        "urn:codenoesis:test:r3-limits",
        "1111111111111111111111111111111111111111",
        "2222222222222222222222222222222222222222",
        files,
    )
}

fn root_files(manifest: &[u8]) -> Vec<(String, Vec<u8>)> {
    vec![
        ("Cargo.toml".to_owned(), manifest.to_vec()),
        ("src/lib.rs".to_owned(), b"pub struct Root;\n".to_vec()),
    ]
}

fn assert_invalid_manifest(
    files: Vec<(String, Vec<u8>)>,
    expected_reason: WorkspaceManifestReason,
    expected_path: Option<&str>,
) {
    assert_eq!(
        TreeSitterRustWorkspaceExtractor::new().extract_root_package_workspace_incremental(
            &synthetic_inventory(files),
            &[],
            &[],
        ),
        Err(RootPackageWorkspaceError::InvalidManifest {
            reason: expected_reason,
            path: expected_path.map(str::to_owned),
        })
    );
}

fn target_conflict_inventory(paths: [&str; 2]) -> RepositoryInventory {
    let manifest = format!(
        "[package]\nname=\"root\"\nedition=\"2024\"\n[[bin]]\nname=\"duplicate\"\npath=\"{}\"\n[[bin]]\nname=\"duplicate\"\npath=\"{}\"\n",
        paths[0], paths[1]
    );
    synthetic_inventory(vec![
        ("Cargo.toml".to_owned(), manifest.into_bytes()),
        ("src/bin/a.rs".to_owned(), b"fn main() {}\n".to_vec()),
        ("src/bin/z.rs".to_owned(), b"fn main() {}\n".to_vec()),
    ])
}

fn inventory(
    repository_identity: &str,
    commit_oid: &str,
    tree_oid: &str,
    files: Vec<AcquiredFile>,
) -> RepositoryInventory {
    RepositoryInventory::classify(AcquiredRepository::new(
        BoundRevision::new(
            RepositoryIdentity::parse(repository_identity).expect("repository identity"),
            oid(commit_oid),
            oid(tree_oid),
        ),
        64,
        files,
    ))
}

#[allow(clippy::needless_pass_by_value)]
fn assert_limit(
    result: Result<
        codenoesis_domain::s4_r3::RootPackageWorkspaceExtraction,
        RootPackageWorkspaceError,
    >,
    expected_limit: RootPackageLimit,
    expected_maximum: u64,
    expected_observed: u64,
) {
    assert_eq!(
        result,
        Err(RootPackageWorkspaceError::LimitExceeded {
            limit: expected_limit,
            maximum: expected_maximum,
            observed: expected_observed,
        })
    );
}

fn stable_ids<'a>(values: impl IntoIterator<Item = &'a str>) -> BTreeSet<String> {
    values.into_iter().map(str::to_owned).collect()
}

fn join_member(member: &str, relative: &str) -> String {
    if member == "." {
        relative.to_owned()
    } else {
        format!("{member}/{relative}")
    }
}

fn oid(value: &str) -> ObjectId {
    ObjectId::parse_sha1(value).expect("SHA-1 object ID")
}
