use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::thread;

use codenoesis_domain::s4_r5::{RustMethodContext, RustSemanticLimit, RustSemanticVisibility};
use codenoesis_domain::s4_r10::{
    MAX_R10_ALTERNATIVES_PER_METHOD, R10_DETERMINISM_PERMUTATIONS, R10_PARALLEL_SCHEDULES,
    RustCfgDeclarationAlternativesError, RustCfgDeclarationAlternativesLimit,
    RustCfgDeclarationAlternativesSourceChunk,
};
use codenoesis_domain::{
    AcquiredFile, AcquiredRepository, BoundRevision, ObjectId, RegularFileMode, RepositoryIdentity,
    RepositoryInventory,
};
use codenoesis_lang_rust::TreeSitterRustWorkspaceExtractor;
use codenoesis_ports::RustCfgDeclarationAlternativesExtractor;

const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s4-rust-cfg-declaration-alternatives-v1";
const COMMIT_OID: &str = "d5a44bb5bb12ddb6f71ea4dd0c88944dc41eefec";
const TREE_OID: &str = "6aa31d889f4c87b2b7dfbff3fef3b32ee7fa0363";
const LOGICAL_METHOD_ID: &str =
    "urn:codenoesis:entity:blake3:437b0bfcd3821ae91eabe8c395d99c80ec54cc53e6f1e6ca6e24098b20bf4b45";
const UNIX_ALTERNATIVE_ID: &str =
    "urn:codenoesis:entity:blake3:452f8e5e1fe8f0e22b43d49b7393c1b224261b8b2f47452ebaee4ca19794542d";
const WINDOWS_ALTERNATIVE_ID: &str =
    "urn:codenoesis:entity:blake3:df85456dbd86d6ded1dd8a752c159e8d838ebf954feb556f47c6200e9a2843c6";
const FIXTURE_FILES: [(&str, &str); 3] = [
    ("Cargo.toml", "cf6685001d2ced704c6bd3beafb0afd8acdd3589"),
    ("build.rs", "e7a4889f33043633039b7e1a1222fc0725f9bd3d"),
    ("src/lib.rs", "d3d391d139dd8349762c533ce00b5ff063fcc8e1"),
];

#[test]
fn gt_fr_ext_013_reviewed_heterogeneous_alternatives_are_exact() {
    let extraction = extract_fixture(0, false);
    let graph = &extraction.knowledge.graph;
    assert_eq!(graph.index.logical_method_ids, [LOGICAL_METHOD_ID]);
    assert_eq!(
        graph.index.alternative_entity_ids,
        [UNIX_ALTERNATIVE_ID, WINDOWS_ALTERNATIVE_ID]
    );
    assert_eq!(graph.alternatives.len(), 2);
    assert_eq!(
        graph
            .alternatives
            .iter()
            .map(|alternative| alternative.properties.declared_signature.as_str())
            .collect::<Vec<_>>(),
        [
            "pub fn try_start_clipboard(&self, context: Option<Context>)",
            "pub fn try_start_clipboard(&self, value: Option<()>)",
        ]
    );
    assert!(
        graph
            .alternatives
            .iter()
            .all(|alternative| alternative.subject_id == LOGICAL_METHOD_ID
                && alternative.properties.attributes.is_empty()
                && alternative.direct_cfg_evidence_ids.len() == 1)
    );
}

#[test]
fn conf_fr_ext_013_reviewed_fixture_newlines_are_platform_neutral() {
    assert_eq!(
        normalize_reviewed_fixture_bytes(b"#[cfg(unix)]\r\nfn connect() {}\r\n"),
        Ok(b"#[cfg(unix)]\nfn connect() {}\n".to_vec())
    );
    assert_eq!(
        normalize_reviewed_fixture_bytes(b"#[cfg(unix)]\rfn connect() {}\n"),
        Err("reviewed fixture contains a bare carriage return")
    );
}

#[test]
fn conf_fr_ext_013_homogeneous_direct_cfg_methods_are_alternatives() {
    let source = r"
pub struct Client;

impl Client {
    #[cfg(unix)]
    pub fn connect(&self, value: u8) {}

    #[cfg(windows)]
    pub fn connect(&self, value: u8) {}
}
";
    let extraction = extract_synthetic(source).expect("homogeneous R10 alternatives");
    let alternatives = &extraction.knowledge.graph.alternatives;
    assert_eq!(alternatives.len(), 2);
    assert_eq!(alternatives[0].subject_id, alternatives[1].subject_id);
    assert_eq!(
        alternatives[0].properties.declared_signature,
        alternatives[1].properties.declared_signature
    );
}

#[test]
fn gt_fr_ext_013_direct_cfg_modules_share_one_logical_owner() {
    let source = r"
#[cfg(unix)]
mod trace {
    pub struct LockTrace { purpose: &'static str }
    pub struct Payload(u8);

    impl LockTrace {
        pub fn enter(purpose: &'static str) { let _ = purpose; }
        pub fn exit() {}
    }
}

#[cfg(windows)]
mod trace {
    pub struct LockTrace { private: () }
    pub struct Payload(String);

    impl LockTrace {
        pub fn enter(_purpose: &'static str) {}
        pub fn exit() {}
    }
}
";
    let extraction = extract_synthetic(source).expect("direct cfg module alternatives");
    let modules = extraction
        .knowledge
        .semantic
        .graph
        .legacy_entities
        .iter()
        .filter(|entity| {
            entity.kind == codenoesis_domain::knowledge::EntityKind::RustModule
                && entity.module_path.as_deref() == Some("crate::trace")
        })
        .collect::<Vec<_>>();
    assert_eq!(modules.len(), 1);
    assert!(
        extraction
            .knowledge
            .semantic
            .graph
            .legacy_entities
            .iter()
            .any(|entity| entity.name == "LockTrace")
    );
    assert_eq!(extraction.knowledge.graph.index.logical_method_ids.len(), 2);
    assert_eq!(extraction.knowledge.graph.alternatives.len(), 4);
    assert!(
        !extraction
            .knowledge
            .semantic
            .graph
            .entities
            .iter()
            .any(|entity| {
                entity.kind == codenoesis_domain::s4_r5::RustSemanticEntityKind::Field
                    && entity.name == "0"
            })
    );
    assert!(
        extraction
            .knowledge
            .graph
            .alternatives
            .iter()
            .all(|alternative| {
                alternative.properties.compilation_presence
                    == codenoesis_domain::s4_r5::CompilationPresence::ConditionalUnknown
                    && alternative.direct_cfg_evidence_ids.len() == 1
            })
    );
}

#[test]
fn ft_fr_ext_013_unicode_nfc_collisions_fail_typed() {
    let source = "pub struct Client;\nimpl Client {\n#[cfg(unix)]\npub fn caf\u{e9}(&self) {}\n#[cfg(windows)]\npub fn cafe\u{301}(&self) {}\n}\n";
    assert!(matches!(
        extract_synthetic(source),
        Err(RustCfgDeclarationAlternativesError::IdentityMismatch {
            reason: "unicode_nfc_collision",
            ..
        })
    ));
}

#[test]
fn ft_fr_ext_013_mixed_or_non_direct_cfg_fails_typed() {
    for source in [
        r"
pub struct Client;
impl Client {
    pub fn connect(&self) {}
    #[cfg(windows)]
    pub fn connect(&self) {}
}
",
        r"
pub struct Client;
impl Client {
    #[cfg_attr(unix, allow(dead_code))]
    pub fn connect(&self) {}
    #[cfg_attr(windows, allow(dead_code))]
    pub fn connect(&self) {}
}
",
        r"
pub struct Client;
impl Client {
    #[cfg(unix)]
    pub fn connect(&self) {}
    #[cfg(windows)]
    fn connect(&self) {}
}
",
    ] {
        assert!(matches!(
            extract_synthetic(source),
            Err(RustCfgDeclarationAlternativesError::IdentityMismatch { .. })
        ));
    }
}

#[test]
fn ft_fr_ext_013_malformed_source_fails_without_alternatives() {
    assert!(matches!(
        extract_synthetic("pub struct Client; impl Client { #[cfg(unix)] pub fn broken( }"),
        Err(RustCfgDeclarationAlternativesError::Source(_))
    ));
}

#[test]
fn ft_fr_ext_013_identity_duplicate_overlap_and_cross_source_fail_typed() {
    let extraction = extract_fixture(0, false);

    let mut identity = extraction.knowledge.clone();
    identity.graph.alternatives[0].properties.visibility = RustSemanticVisibility::Private;
    assert!(matches!(
        identity.validate(),
        Err(RustCfgDeclarationAlternativesError::IdentityMismatch { .. })
    ));

    let mut context = extraction.knowledge.clone();
    context.graph.alternatives[0]
        .properties
        .implementation_context = RustMethodContext::TraitDeclaration;
    assert!(matches!(
        context.validate(),
        Err(RustCfgDeclarationAlternativesError::IdentityMismatch { .. })
    ));

    let alternative = extraction.knowledge.graph.alternatives[0].clone();
    assert!(matches!(
        RustCfgDeclarationAlternativesSourceChunk::new(
            alternative.source_file_id.clone(),
            vec![alternative.clone(), alternative],
        ),
        Err(RustCfgDeclarationAlternativesError::Duplicate { .. })
    ));

    let mut cross_source = extraction.knowledge.clone();
    cross_source.graph.alternatives[1].source_file_id = "different-source".to_owned();
    assert!(matches!(
        cross_source.validate(),
        Err(RustCfgDeclarationAlternativesError::CrossSource { .. })
    ));

    let mut overlap = extraction.knowledge;
    let first_id = overlap.graph.alternatives[0]
        .properties
        .declaration_evidence_id
        .clone();
    let second_id = overlap.graph.alternatives[1]
        .properties
        .declaration_evidence_id
        .clone();
    let first_span = overlap
        .semantic
        .graph
        .evidence
        .iter()
        .find(|evidence| evidence.id == first_id)
        .map(|evidence| (evidence.start_byte, evidence.end_byte))
        .expect("first R10 declaration evidence");
    for evidence in overlap
        .semantic
        .graph
        .evidence
        .iter_mut()
        .chain(
            overlap
                .semantic
                .extraction_chunks
                .iter_mut()
                .flat_map(|chunk| chunk.evidence.iter_mut()),
        )
        .filter(|evidence| evidence.id == second_id)
    {
        evidence.start_byte = first_span.0 + 1;
        evidence.end_byte = first_span.1 + 1;
    }
    assert!(matches!(
        overlap.validate(),
        Err(RustCfgDeclarationAlternativesError::Overlap { .. })
    ));
}

#[test]
fn pt_fr_ext_013_method_limit_accepts_32_and_rejects_33() {
    let maximum = repeated_methods(
        usize::try_from(MAX_R10_ALTERNATIVES_PER_METHOD).expect("R10 method maximum"),
    );
    assert_eq!(
        extract_synthetic(&maximum)
            .expect("R10 method maximum is accepted")
            .knowledge
            .graph
            .alternatives
            .len(),
        32
    );
    let plus_one = repeated_methods(
        usize::try_from(MAX_R10_ALTERNATIVES_PER_METHOD + 1).expect("R10 method maximum plus one"),
    );
    assert!(matches!(
        extract_synthetic(&plus_one),
        Err(RustCfgDeclarationAlternativesError::LimitExceeded {
            limit: RustCfgDeclarationAlternativesLimit::AlternativesPerLogicalMethod,
            maximum: MAX_R10_ALTERNATIVES_PER_METHOD,
            observed: 33,
        })
    ));
}

#[test]
fn pt_fr_ext_013_source_limit_accepts_4096_and_rejects_4097() {
    let maximum = repeated_logical_methods(2_048);
    assert_eq!(
        extract_synthetic(&maximum)
            .expect("R10 source maximum is accepted")
            .knowledge
            .graph
            .alternatives
            .len(),
        4_096
    );
    let plus_one = repeated_logical_methods(2_049);
    assert!(matches!(
        extract_synthetic(&plus_one),
        Err(RustCfgDeclarationAlternativesError::LimitExceeded {
            limit: RustCfgDeclarationAlternativesLimit::AlternativesPerSource,
            maximum: 4_096,
            observed: 4_097,
        })
    ));
}

#[test]
fn pt_fr_ext_013_signature_limit_accepts_4096_and_rejects_4097() {
    let maximum = repeated_long_signature(4_096);
    assert_eq!(
        extract_synthetic(&maximum)
            .expect("R10 signature maximum is accepted")
            .knowledge
            .graph
            .alternatives
            .len(),
        2
    );
    let plus_one = repeated_long_signature(4_097);
    assert!(matches!(
        extract_synthetic(&plus_one),
        Err(RustCfgDeclarationAlternativesError::Source(
            codenoesis_domain::s4_r5::RustSemanticError::LimitExceeded {
                limit: RustSemanticLimit::DeclaredTypeOrHeaderBytes,
                maximum: 4_096,
                observed: 4_097,
            }
        ))
    ));
}

#[test]
fn pt_fr_ext_013_attribute_limits_accept_maxima_and_reject_plus_one() {
    let token_maximum = repeated_long_attribute(16_384);
    assert_eq!(
        extract_synthetic(&token_maximum)
            .expect("R10 attribute-token maximum is accepted")
            .knowledge
            .graph
            .alternatives
            .len(),
        2
    );
    let token_plus_one = repeated_long_attribute(16_385);
    assert!(matches!(
        extract_synthetic(&token_plus_one),
        Err(RustCfgDeclarationAlternativesError::Source(
            codenoesis_domain::s4_r5::RustSemanticError::LimitExceeded {
                limit: RustSemanticLimit::AttributeTokenBytes,
                maximum: 16_384,
                observed: 16_385,
            }
        ))
    ));

    let count_maximum = repeated_attributes(127);
    let extraction = extract_synthetic(&count_maximum).expect("R10 attribute maximum is accepted");
    assert_eq!(
        extraction
            .knowledge
            .graph
            .alternatives
            .iter()
            .map(|alternative| alternative.properties.attributes.len())
            .max(),
        Some(127)
    );
    let count_plus_one = repeated_attributes(128);
    assert!(matches!(
        extract_synthetic(&count_plus_one),
        Err(RustCfgDeclarationAlternativesError::Source(
            codenoesis_domain::s4_r5::RustSemanticError::LimitExceeded {
                limit: RustSemanticLimit::OuterAttributesPerDeclaration,
                maximum: 128,
                observed: 129,
            }
        ))
    ));
}

#[test]
fn pt_nfr_det_001_r10_fifty_permutations_and_ten_schedules_are_identical() {
    let expected = extract_fixture(0, false).knowledge;
    for permutation in 0..R10_DETERMINISM_PERMUTATIONS {
        let rotation = usize::try_from(permutation).expect("R10 permutation index");
        assert_eq!(
            extract_fixture(rotation, permutation % 2 == 1).knowledge,
            expected,
            "R10 permutation {permutation}"
        );
    }
    thread::scope(|scope| {
        let handles = (0..R10_PARALLEL_SCHEDULES)
            .map(|schedule| {
                scope.spawn(move || {
                    let rotation = usize::try_from(schedule).expect("R10 schedule index");
                    extract_fixture(rotation, schedule % 2 == 0).knowledge
                })
            })
            .collect::<Vec<_>>();
        for (schedule, handle) in handles.into_iter().enumerate() {
            assert_eq!(
                handle.join().expect("R10 extraction schedule"),
                expected,
                "R10 schedule {schedule}"
            );
        }
    });
}

fn extract_fixture(
    rotation: usize,
    reverse: bool,
) -> codenoesis_domain::s4_r10::RustCfgDeclarationAlternativesExtraction {
    TreeSitterRustWorkspaceExtractor::new()
        .extract_rust_cfg_declaration_alternatives_incremental(
            &fixture_inventory(rotation, reverse),
            &[],
            &[],
        )
        .expect("extract reviewed R10 fixture")
}

fn extract_synthetic(
    source: &str,
) -> Result<
    codenoesis_domain::s4_r10::RustCfgDeclarationAlternativesExtraction,
    RustCfgDeclarationAlternativesError,
> {
    TreeSitterRustWorkspaceExtractor::new().extract_rust_cfg_declaration_alternatives_incremental(
        &synthetic_inventory(source),
        &[],
        &[],
    )
}

fn fixture_inventory(rotation: usize, reverse: bool) -> RepositoryInventory {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/s4/rust-cfg-declaration-alternatives-v1/repository");
    let mut files = FIXTURE_FILES
        .into_iter()
        .map(|(path, blob_oid)| {
            AcquiredFile::new(
                path.to_owned(),
                RegularFileMode::Regular,
                ObjectId::parse_sha1(blob_oid).expect("reviewed R10 blob OID"),
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
            RepositoryIdentity::parse(REPOSITORY_ID).expect("reviewed R10 repository identity"),
            ObjectId::parse_sha1(COMMIT_OID).expect("reviewed R10 commit OID"),
            ObjectId::parse_sha1(TREE_OID).expect("reviewed R10 tree OID"),
        ),
        u64::try_from(files.len()).expect("reviewed R10 file count"),
        files,
    ))
}

fn read_reviewed_fixture(path: &Path) -> Vec<u8> {
    let bytes = fs::read(path).unwrap_or_else(|error| {
        panic!("read reviewed R10 fixture file {}: {error}", path.display())
    });
    normalize_reviewed_fixture_bytes(&bytes).unwrap_or_else(|reason| {
        panic!("invalid reviewed R10 fixture {}: {reason}", path.display())
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

fn synthetic_inventory(source: &str) -> RepositoryInventory {
    let files = vec![
        AcquiredFile::new(
            "Cargo.toml".to_owned(),
            RegularFileMode::Regular,
            ObjectId::parse_sha1(&"c".repeat(40)).expect("synthetic R10 manifest OID"),
            b"[package]\nname = \"r10-test\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[lib]\npath = \"src/lib.rs\"\n"
                .to_vec(),
        ),
        AcquiredFile::new(
            "src/lib.rs".to_owned(),
            RegularFileMode::Regular,
            ObjectId::parse_sha1(&"d".repeat(40)).expect("synthetic R10 source OID"),
            source.as_bytes().to_vec(),
        ),
    ];
    RepositoryInventory::classify(AcquiredRepository::new(
        BoundRevision::new(
            RepositoryIdentity::parse("urn:codenoesis:test:r10-cfg-alternatives")
                .expect("synthetic R10 repository identity"),
            ObjectId::parse_sha1(&"a".repeat(40)).expect("synthetic R10 commit OID"),
            ObjectId::parse_sha1(&"b".repeat(40)).expect("synthetic R10 tree OID"),
        ),
        u64::try_from(files.len()).expect("synthetic R10 file count"),
        files,
    ))
}

fn repeated_methods(count: usize) -> String {
    let mut source = String::from("pub struct Client;\nimpl Client {\n");
    for index in 0..count {
        writeln!(
            &mut source,
            "#[cfg(r10_case_{index})]\npub fn connect(&self, value: u8) {{}}"
        )
        .expect("write repeated R10 method");
    }
    source.push_str("}\n");
    source
}

fn repeated_logical_methods(count: usize) -> String {
    let mut source = String::from("pub struct Client;\n");
    for block_start in (0..count).step_by(512) {
        source.push_str("impl Client {\n");
        for index in block_start..count.min(block_start + 512) {
            writeln!(
                &mut source,
                "#[cfg(unix)]\npub fn method_{index}(&self) {{}}\n#[cfg(windows)]\npub fn method_{index}(&self) {{}}"
            )
            .expect("write repeated R10 logical method");
        }
        source.push_str("}\n");
    }
    source
}

fn repeated_long_signature(length: usize) -> String {
    let prefix = "pub fn bounded(&self, ";
    let suffix = ": u8)";
    let identifier_length = length
        .checked_sub(prefix.len() + suffix.len())
        .expect("R10 signature length accommodates syntax");
    let signature = format!("{prefix}{}{suffix}", "x".repeat(identifier_length));
    assert_eq!(signature.len(), length);
    format!(
        "pub struct Client;\nimpl Client {{\n#[cfg(unix)]\n{signature} {{}}\n#[cfg(windows)]\n{signature} {{}}\n}}\n"
    )
}

fn repeated_long_attribute(length: usize) -> String {
    let prefix = "#[doc = \"";
    let suffix = "\"]";
    let payload_length = length
        .checked_sub(prefix.len() + suffix.len())
        .expect("R10 attribute length accommodates syntax");
    let attribute = format!("{prefix}{}{suffix}", "x".repeat(payload_length));
    assert_eq!(attribute.len(), length);
    format!(
        "pub struct Client;\nimpl Client {{\n#[cfg(unix)]\n{attribute}\npub fn bounded(&self) {{}}\n#[cfg(windows)]\npub fn bounded(&self) {{}}\n}}\n"
    )
}

fn repeated_attributes(extra_attributes: usize) -> String {
    let mut attributes = String::new();
    for index in 0..extra_attributes {
        writeln!(&mut attributes, "#[doc = \"attribute-{index}\"]")
            .expect("write repeated R10 attribute");
    }
    format!(
        "pub struct Client;\nimpl Client {{\n#[cfg(unix)]\n{attributes}pub fn bounded(&self) {{}}\n#[cfg(windows)]\npub fn bounded(&self) {{}}\n}}\n"
    )
}
