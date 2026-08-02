use codenoesis_domain::s1_boundaries::{
    AcquiredGitlink, AcquiredGitmodules, BoundaryLimit, BoundaryMetadataReason, BoundarySha256,
    BoundaryUrlKind, MAX_GITLINK_ENTRIES, MAX_GITMODULES_BYTES, MAX_GITMODULES_KEYS_PER_SECTION,
    MAX_GITMODULES_SECTIONS, ParsedGitmodules, RepositoryBoundaryError, RepositoryBoundaryState,
    VerifiedNestedRepository, build_boundary_report, check_boundary_limit, parse_gitmodules,
};
use codenoesis_domain::{BoundRevision, ObjectId, RepositoryIdentity};

struct TestHasher;

impl BoundarySha256 for TestHasher {
    fn digest(&self, bytes: &[u8]) -> [u8; 32] {
        *blake3::hash(bytes).as_bytes()
    }
}

struct ConstantHasher;

impl BoundarySha256 for ConstantHasher {
    fn digest(&self, _bytes: &[u8]) -> [u8; 32] {
        [0; 32]
    }
}

#[test]
fn pt_fr_acq_005_limits_have_max_and_plus_one() {
    assert_eq!(MAX_GITLINK_ENTRIES, 128);
    assert_eq!(MAX_GITMODULES_SECTIONS, 256);
    let limits = [
        BoundaryLimit::BoundaryManifestBytes,
        BoundaryLimit::GitlinkEntries,
        BoundaryLimit::GitmodulesBytes,
        BoundaryLimit::GitmodulesSections,
        BoundaryLimit::GitmodulesKeysPerSection,
        BoundaryLimit::ExplicitNestedRepositories,
        BoundaryLimit::ExplicitNestingDepth,
        BoundaryLimit::BoundaryReportBytes,
    ];
    for limit in limits {
        assert_eq!(check_boundary_limit(limit, limit.maximum()), Ok(()));
        assert_eq!(
            check_boundary_limit(limit, limit.maximum().saturating_add(1)),
            Err(RepositoryBoundaryError::LimitExceeded {
                limit,
                maximum: limit.maximum(),
                observed: limit.maximum().saturating_add(1),
            })
        );
        assert_eq!(
            check_boundary_limit(limit, u64::MAX),
            Err(RepositoryBoundaryError::LimitExceeded {
                limit,
                maximum: limit.maximum(),
                observed: limit.maximum().saturating_add(1),
            })
        );
    }
}

#[test]
fn conf_fr_acq_005_gitmodules_subset_accepts_exact_limits() {
    let sections = gitmodules_sections(usize::try_from(MAX_GITMODULES_SECTIONS).unwrap(), 2);
    let parsed = parse(&sections).expect("256 sections are accepted");
    assert_eq!(parsed.declarations.len(), 256);

    let keys = gitmodules_sections(1, usize::try_from(MAX_GITMODULES_KEYS_PER_SECTION).unwrap());
    let parsed = parse(&keys).expect("32 keys are accepted");
    assert_eq!(parsed.declarations[0].unsupported_keys.len(), 30);

    let mut maximum_bytes = vec![b' '; usize::try_from(MAX_GITMODULES_BYTES).unwrap()];
    maximum_bytes[0] = b'#';
    assert!(parse(&maximum_bytes).is_ok());
}

#[test]
fn conf_fr_acq_005_gitmodules_subset_rejects_limit_plus_one() {
    let sections = gitmodules_sections(usize::try_from(MAX_GITMODULES_SECTIONS + 1).unwrap(), 2);
    assert_limit(
        parse(&sections),
        BoundaryLimit::GitmodulesSections,
        MAX_GITMODULES_SECTIONS + 1,
    );

    let keys = gitmodules_sections(
        1,
        usize::try_from(MAX_GITMODULES_KEYS_PER_SECTION + 1).unwrap(),
    );
    assert_limit(
        parse(&keys),
        BoundaryLimit::GitmodulesKeysPerSection,
        MAX_GITMODULES_KEYS_PER_SECTION + 1,
    );

    let bytes = vec![b'#'; usize::try_from(MAX_GITMODULES_BYTES + 1).unwrap()];
    assert_limit(
        parse(&bytes),
        BoundaryLimit::GitmodulesBytes,
        MAX_GITMODULES_BYTES + 1,
    );
}

#[test]
fn conf_fr_acq_005_gitmodules_subset_rejects_each_closed_reason() {
    let cases: Vec<(Vec<u8>, BoundaryMetadataReason)> = vec![
        (vec![0xff], BoundaryMetadataReason::InvalidEncoding),
        (vec![0], BoundaryMetadataReason::NulOrControl),
        (
            b"# comment\rnext".to_vec(),
            BoundaryMetadataReason::BareCarriageReturn,
        ),
        (
            b"[module \"x\"]\n".to_vec(),
            BoundaryMetadataReason::MalformedSection,
        ),
        (
            b"[submodule \"bad name\"]\n".to_vec(),
            BoundaryMetadataReason::InvalidName,
        ),
        (
            b"path = x\n".to_vec(),
            BoundaryMetadataReason::KeyOutsideSection,
        ),
        (
            b"[submodule \"x\"]\npath = x\nurl = a\n[submodule \"x\"]\npath = y\nurl = b\n"
                .to_vec(),
            BoundaryMetadataReason::DuplicateSection,
        ),
        (
            b"[submodule \"x\"]\npath = x\npath = y\nurl = a\n".to_vec(),
            BoundaryMetadataReason::DuplicateKey,
        ),
        (
            b"[submodule \"x\"]\npath = x\n".to_vec(),
            BoundaryMetadataReason::RequiredKeyMissing,
        ),
        (
            b"[submodule \"x\"]\npath = x\nurl = \"quoted\"\n".to_vec(),
            BoundaryMetadataReason::MalformedSection,
        ),
        (
            b"[submodule \"x\"]\npath = x\nurl = ${interpolated}\n".to_vec(),
            BoundaryMetadataReason::MalformedSection,
        ),
        (
            b"[submodule \"x\"]\npath = x\nurl = value # inline\n".to_vec(),
            BoundaryMetadataReason::MalformedSection,
        ),
        (
            b"[submodule \"x\"]\npath = ../x\nurl = a\n".to_vec(),
            BoundaryMetadataReason::PathInvalid,
        ),
        (
            b"[submodule \"x\"]\npath = x\nurl = a\n[submodule \"y\"]\npath = x\nurl = b\n"
                .to_vec(),
            BoundaryMetadataReason::AmbiguousMapping,
        ),
    ];
    for (bytes, expected) in cases {
        assert_eq!(metadata_reason(parse(&bytes)), expected);
    }

    let mut source = source(b"[submodule \"x\"]\npath = x\nurl = a\n");
    source.mode = "100755".to_owned();
    assert_eq!(
        metadata_reason(parse_gitmodules(&root(), Some(&source), &TestHasher)),
        BoundaryMetadataReason::UnsafeEntryKind
    );

    assert_eq!(
        metadata_reason(parse(
            b"[submodule \"first\"]\npath = first\n[submodule \"second\"]\ninvalid\n"
        )),
        BoundaryMetadataReason::RequiredKeyMissing,
        "the earlier incomplete section wins over later syntax"
    );
    assert_eq!(
        metadata_reason(parse(&[0xff, 0])),
        BoundaryMetadataReason::InvalidEncoding,
        "the lowest source byte wins across byte-level reasons"
    );
    assert_eq!(
        metadata_reason(parse(&[b'#', 0, 0xff])),
        BoundaryMetadataReason::NulOrControl,
        "a control byte before invalid UTF-8 wins"
    );
}

#[test]
fn sec_fr_acq_005_url_redaction_and_classification_are_exact() {
    let values = [
        ("./local", BoundaryUrlKind::Relative),
        ("/local", BoundaryUrlKind::AbsolutePath),
        ("file:local", BoundaryUrlKind::File),
        ("ssh:local", BoundaryUrlKind::Ssh),
        ("https:local", BoundaryUrlKind::Https),
        ("http:local", BoundaryUrlKind::Http),
        ("git:local", BoundaryUrlKind::Git),
        ("user@host:path", BoundaryUrlKind::ScpLike),
        ("HTTPS:other", BoundaryUrlKind::Other),
    ];
    let canary = "user:secret@example.invalid/path?token=value#fragment";
    let mut bytes = Vec::new();
    for (index, (value, _)) in values.iter().enumerate() {
        bytes.extend_from_slice(
            format!(
                "[submodule \"s{index}\"]\npath = p{index}\nurl = {value}{canary}\nbranch = {canary}\n"
            )
            .as_bytes(),
        );
    }
    let parsed = parse(&bytes).expect("parse classified URLs");
    for (declaration, (_, expected)) in parsed.declarations.iter().zip(values) {
        assert_eq!(declaration.url_kind, expected);
        assert_eq!(declaration.url_sha256.len(), 64);
        assert_eq!(declaration.unsupported_keys.len(), 1);
        assert_eq!(declaration.unsupported_keys[0].key, "branch");
        assert_eq!(declaration.unsupported_keys[0].value_sha256.len(), 64);
    }
    let debug = format!("{parsed:?}");
    assert!(!debug.contains(canary));
    assert!(!debug.contains("secret"));
}

#[test]
fn gt_fr_acq_005_boundary_states_and_gaps_are_complete() {
    let root = root();
    let gitlinks = vec![gitlink("declared", 'a'), gitlink("missing", 'b')];
    let parsed = parse(
        b"[submodule \"declared\"]\npath = declared\nurl = https:declared\nbranch = main\n\
          [submodule \"orphan\"]\npath = orphan\nurl = https:orphan\n",
    )
    .expect("parse report declarations");
    let report = build_boundary_report(&root, gitlinks, parsed, &[], &TestHasher)
        .expect("build unbound report");
    assert_eq!(
        report.boundaries[0].state,
        RepositoryBoundaryState::DeclaredUnbound
    );
    assert_eq!(
        report.boundaries[1].state,
        RepositoryBoundaryState::UndeclaredUnbound
    );
    let codes = report
        .coverage_gaps
        .iter()
        .map(|gap| gap.code)
        .collect::<Vec<_>>();
    assert!(codes.contains(&"boundary.nested_repository_unbound"));
    assert!(codes.contains(&"boundary.gitmodules_declaration_missing"));
    assert!(codes.contains(&"boundary.gitmodules_declaration_orphan"));
    assert!(codes.contains(&"boundary.gitmodules_key_unsupported"));

    let nested = VerifiedNestedRepository {
        boundary_path: "declared".to_owned(),
        bound_revision: BoundRevision::new(
            RepositoryIdentity::parse("urn:codenoesis:repository:nested").unwrap(),
            oid('a'),
            oid('c'),
        ),
    };
    let parsed =
        parse(b"[submodule \"declared\"]\npath = declared\nurl = https:declared\n").unwrap();
    let report = build_boundary_report(
        &root,
        vec![gitlink("declared", 'a')],
        parsed,
        &[nested],
        &TestHasher,
    )
    .unwrap();
    assert_eq!(
        report.boundaries[0].state,
        RepositoryBoundaryState::ExplicitlyBound
    );
    assert_eq!(
        report.coverage_gaps[0].code,
        "boundary.nested_repository_not_analyzed"
    );
}

#[test]
fn pt_fr_acq_005_order_invariant() {
    let bytes = b"[submodule \"c\"]\npath = c\nurl = https:c\nbranch = main\n\
                  [submodule \"a\"]\npath = a\nurl = https:a\n\
                  [submodule \"b\"]\npath = b\nurl = https:b\n";
    let baseline = build_boundary_report(
        &root(),
        vec![gitlink("c", 'c'), gitlink("a", 'a'), gitlink("b", 'b')],
        parse(bytes).unwrap(),
        &[],
        &TestHasher,
    )
    .unwrap();

    for permutation in 0..50 {
        let mut gitlinks = vec![gitlink("c", 'c'), gitlink("a", 'a'), gitlink("b", 'b')];
        gitlinks.rotate_left(permutation % 3);
        let mut parsed = parse(bytes).unwrap();
        parsed.declarations.rotate_left(permutation % 3);
        if !permutation.is_multiple_of(2) {
            gitlinks.reverse();
            parsed.declarations.reverse();
        }
        assert_eq!(
            build_boundary_report(&root(), gitlinks, parsed, &[], &TestHasher).unwrap(),
            baseline,
            "permutation {permutation}"
        );
    }
}

#[test]
fn pt_fr_acq_005_parallel_replay() {
    let bytes = b"[submodule \"a\"]\npath = a\nurl = https:a\n";
    let expected = build_boundary_report(
        &root(),
        vec![gitlink("a", 'a')],
        parse(bytes).unwrap(),
        &[],
        &TestHasher,
    )
    .unwrap();
    std::thread::scope(|scope| {
        let workers = (0..8)
            .map(|_| {
                scope.spawn(|| {
                    build_boundary_report(
                        &root(),
                        vec![gitlink("a", 'a')],
                        parse(bytes).unwrap(),
                        &[],
                        &TestHasher,
                    )
                    .unwrap()
                })
            })
            .collect::<Vec<_>>();
        for worker in workers {
            assert_eq!(worker.join().unwrap(), expected);
        }
    });
}

#[test]
fn fz_fr_acq_005_gitmodules_seed_corpus_is_deterministic() {
    let seeds = [
        b"".as_slice(),
        b"# comment\n",
        b"[submodule \"a\"]\npath = a\nurl = https:a\n",
        b"[submodule \"a\"]\r\npath = a\r\nurl = user@host:path\r\n",
        b"[submodule \"a\"]\npath = ../escape\nurl = file:secret\n",
        b"[submodule \"a\"]\npath = a\nurl = ${value}\n",
        &[0xff, b'\n'],
        &[0, b'\n'],
    ];
    for seed in seeds {
        assert_eq!(parse(seed), parse(seed));
    }
}

#[test]
fn conf_fr_acq_005_duplicate_derived_ids_are_rejected() {
    assert_eq!(
        build_boundary_report(
            &root(),
            vec![gitlink("a", 'a'), gitlink("b", 'b')],
            ParsedGitmodules::default(),
            &[],
            &ConstantHasher,
        ),
        Err(RepositoryBoundaryError::InvalidReport)
    );
}

fn parse(bytes: &[u8]) -> Result<ParsedGitmodules, RepositoryBoundaryError> {
    let source = source(bytes);
    parse_gitmodules(&root(), Some(&source), &TestHasher)
}

fn source(bytes: &[u8]) -> AcquiredGitmodules {
    AcquiredGitmodules {
        mode: "100644".to_owned(),
        blob_oid: oid('d'),
        bytes: bytes.to_vec(),
    }
}

fn root() -> BoundRevision {
    BoundRevision::new(
        RepositoryIdentity::parse("urn:codenoesis:repository:root").unwrap(),
        oid('e'),
        oid('f'),
    )
}

fn gitlink(path: &str, value: char) -> AcquiredGitlink {
    AcquiredGitlink {
        path: path.to_owned(),
        containing_tree_oid: oid('f'),
        gitlink_oid: oid(value),
    }
}

fn oid(value: char) -> ObjectId {
    ObjectId::parse_sha1(&value.to_string().repeat(40)).unwrap()
}

fn metadata_reason(
    result: Result<ParsedGitmodules, RepositoryBoundaryError>,
) -> BoundaryMetadataReason {
    match result.unwrap_err() {
        RepositoryBoundaryError::MetadataInvalid { reason, .. } => reason,
        error => panic!("unexpected error: {error:?}"),
    }
}

fn assert_limit(
    result: Result<ParsedGitmodules, RepositoryBoundaryError>,
    expected_limit: BoundaryLimit,
    expected_observed: u64,
) {
    assert_eq!(
        result.unwrap_err(),
        RepositoryBoundaryError::LimitExceeded {
            limit: expected_limit,
            maximum: expected_limit.maximum(),
            observed: expected_observed,
        }
    );
}

fn gitmodules_sections(section_count: usize, key_count: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    for section in 0..section_count {
        bytes.extend_from_slice(
            format!("[submodule \"s{section}\"]\npath = p{section}\nurl = https:s{section}\n")
                .as_bytes(),
        );
        for key in 2..key_count {
            bytes.extend_from_slice(format!("key-{key} = value-{key}\n").as_bytes());
        }
    }
    bytes
}
