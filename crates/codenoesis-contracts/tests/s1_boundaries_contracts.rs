use codenoesis_contracts::{
    BoundaryManifestReason, CodeNoesisErrorV9, RepositoryBoundaryInputError,
    parse_repository_boundary_input, repository_boundary_value,
    validate_repository_boundary_report_size,
};
use codenoesis_domain::s1_boundaries::{
    AcquiredGitmodules, BoundaryLimit, BoundarySha256, MAX_BOUNDARY_MANIFEST_BYTES,
    MAX_BOUNDARY_REPORT_BYTES, MAX_EXPLICIT_NESTED_REPOSITORIES, RepositoryBoundaryError,
    build_boundary_report, parse_gitmodules,
};
use codenoesis_domain::{BoundRevision, ObjectId, RepositoryIdentity};
use serde_json::{Value, json};

const ROOT_IDENTITY: &str = "urn:codenoesis:repository:root";
const ROOT_COMMIT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

struct TestHasher;

impl BoundarySha256 for TestHasher {
    fn digest(&self, bytes: &[u8]) -> [u8; 32] {
        *blake3::hash(bytes).as_bytes()
    }
}

#[test]
fn conf_fr_acq_005_boundary_input_v1_is_strict_and_ordered() {
    let entries = (0..8).map(entry).collect::<Vec<_>>();
    let expected = parse_value(&json!({
        "schema_version": "codenoesis.repository-boundary-input/v1",
        "root": {"repository_identity": ROOT_IDENTITY, "commit_oid": ROOT_COMMIT},
        "nested_repositories": entries
    }))
    .unwrap();
    for permutation in 0..50 {
        let mut entries = (0..8).map(entry).collect::<Vec<_>>();
        entries.rotate_left(permutation % 8);
        if permutation % 2 == 1 {
            entries.reverse();
        }
        let parsed = parse_value(&json!({
            "schema_version": "codenoesis.repository-boundary-input/v1",
            "root": {"repository_identity": ROOT_IDENTITY, "commit_oid": ROOT_COMMIT},
            "nested_repositories": entries
        }))
        .unwrap();
        assert_eq!(parsed, expected, "permutation {permutation}");
    }
}

#[test]
fn conf_fr_acq_005_boundary_input_v1_rejects_duplicate_members() {
    let bytes = format!(
        "{{\"schema_version\":\"codenoesis.repository-boundary-input/v1\",\"schema_version\":\"codenoesis.repository-boundary-input/v1\",\"root\":{{\"repository_identity\":\"{ROOT_IDENTITY}\",\"commit_oid\":\"{ROOT_COMMIT}\"}},\"nested_repositories\":[]}}"
    );
    assert_invalid(
        parse_repository_boundary_input(bytes.as_bytes()),
        BoundaryManifestReason::SchemaInvalid,
    );
}

#[test]
fn conf_fr_acq_005_boundary_input_v1_rejects_duplicate_and_recursive_entries() {
    let duplicate = json!({
        "schema_version": "codenoesis.repository-boundary-input/v1",
        "root": {"repository_identity": ROOT_IDENTITY, "commit_oid": ROOT_COMMIT},
        "nested_repositories": [entry(0), entry(0)]
    });
    assert_invalid(
        parse_value(&duplicate),
        BoundaryManifestReason::DuplicateBoundaryPath,
    );

    let mut recursive = entry(0);
    recursive
        .as_object_mut()
        .unwrap()
        .insert("nested_repositories".to_owned(), json!([]));
    assert_invalid(
        parse_value(&json!({
            "schema_version": "codenoesis.repository-boundary-input/v1",
            "root": {"repository_identity": ROOT_IDENTITY, "commit_oid": ROOT_COMMIT},
            "nested_repositories": [recursive]
        })),
        BoundaryManifestReason::RecursiveInput,
    );
}

#[test]
fn conf_fr_acq_005_boundary_input_v1_enforces_identity_contract() {
    let oversized_identity = format!("urn:codenoesis:{}", "a".repeat(241));
    let oversized = json!({
        "schema_version": "codenoesis.repository-boundary-input/v1",
        "root": {"repository_identity": oversized_identity, "commit_oid": ROOT_COMMIT},
        "nested_repositories": []
    });
    assert_invalid(
        parse_value(&oversized),
        BoundaryManifestReason::SchemaInvalid,
    );

    let mut same_identity = entry(0);
    same_identity["repository_identity"] = Value::String(ROOT_IDENTITY.to_owned());
    assert_invalid(
        parse_value(&manifest(&[same_identity])),
        BoundaryManifestReason::SchemaInvalid,
    );
}

#[test]
fn pt_fr_acq_005_boundary_manifest_bytes_have_max_and_plus_one() {
    let mut bytes = serde_json::to_vec(&json!({
        "schema_version": "codenoesis.repository-boundary-input/v1",
        "root": {"repository_identity": ROOT_IDENTITY, "commit_oid": ROOT_COMMIT},
        "nested_repositories": []
    }))
    .unwrap();
    bytes.resize(usize::try_from(MAX_BOUNDARY_MANIFEST_BYTES).unwrap(), b' ');
    assert!(parse_repository_boundary_input(&bytes).is_ok());
    bytes.push(b' ');
    assert_eq!(
        parse_repository_boundary_input(&bytes),
        Err(RepositoryBoundaryInputError::Limit(
            RepositoryBoundaryError::LimitExceeded {
                limit: BoundaryLimit::BoundaryManifestBytes,
                maximum: MAX_BOUNDARY_MANIFEST_BYTES,
                observed: MAX_BOUNDARY_MANIFEST_BYTES + 1,
            }
        ))
    );
}

#[test]
fn pt_fr_acq_005_explicit_roots_have_max_and_plus_one() {
    let maximum = usize::try_from(MAX_EXPLICIT_NESTED_REPOSITORIES).unwrap();
    let entries = (0..maximum).map(entry).collect::<Vec<_>>();
    let value = manifest(&entries);
    assert_eq!(
        parse_value(&value).unwrap().nested_repositories.len(),
        maximum
    );

    let entries = (0..=maximum).map(entry).collect::<Vec<_>>();
    let value = manifest(&entries);
    assert_eq!(
        parse_value(&value),
        Err(RepositoryBoundaryInputError::Limit(
            RepositoryBoundaryError::LimitExceeded {
                limit: BoundaryLimit::ExplicitNestedRepositories,
                maximum: MAX_EXPLICIT_NESTED_REPOSITORIES,
                observed: MAX_EXPLICIT_NESTED_REPOSITORIES + 1,
            }
        ))
    );
}

#[test]
fn conf_fr_acq_005_error_v9_is_closed_and_redacted() {
    let mismatch = CodeNoesisErrorV9::nested_mismatch(
        "external/model",
        &codenoesis_domain::ObjectId::parse_sha1("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .unwrap(),
        &codenoesis_domain::ObjectId::parse_sha1("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
            .unwrap(),
    );
    assert_eq!(
        mismatch.value(),
        &json!({
            "schema_version": "codenoesis.error/v9",
            "code": "acquisition.nested_repository_mismatch",
            "stage": "acquisition",
            "message": "nested repository mismatch",
            "retryable": false,
            "context": {
                "component": "nested_repository",
                "path": "external/model",
                "expected_oid": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "observed_oid": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
            }
        })
    );
    let changed = CodeNoesisErrorV9::nested_changed("external/model");
    assert_eq!(changed.value()["retryable"], true);
    assert_eq!(
        changed.value()["context"]["reason"],
        "changed_during_binding"
    );
    for error in [mismatch, changed, CodeNoesisErrorV9::invalid_profile()] {
        let stderr = error.canonical_stderr().unwrap();
        assert!(stderr.ends_with(b"\n"));
        assert!(!stderr.windows(5).any(|window| window == b"/tmp/"));
        assert!(!stderr.windows(7).any(|window| window == b"secret="));
    }
}

#[test]
fn pt_fr_acq_005_boundary_report_bytes_have_exact_max_and_plus_one() {
    let maximum = usize::try_from(MAX_BOUNDARY_REPORT_BYTES).unwrap();
    let mut report = (1..=30)
        .rev()
        .find_map(|key_count| {
            let report = orphan_report(key_count);
            let length = boundary_report_length(&report);
            let capacity = report
                .declarations
                .iter()
                .flat_map(|declaration| &declaration.unsupported_keys)
                .map(|key| 64_usize.saturating_sub(key.key.len()))
                .sum::<usize>();
            (length <= maximum && length.saturating_add(capacity) > maximum).then_some(report)
        })
        .expect("constructible one-MiB boundary report");
    let mut remaining = maximum - boundary_report_length(&report);
    for key in report
        .declarations
        .iter_mut()
        .flat_map(|declaration| &mut declaration.unsupported_keys)
    {
        let added = remaining.min(64 - key.key.len());
        key.key.push_str(&"x".repeat(added));
        remaining -= added;
        if remaining == 0 {
            break;
        }
    }
    assert_eq!(remaining, 0);
    assert_eq!(boundary_report_length(&report), maximum);
    assert_eq!(validate_repository_boundary_report_size(&report), Ok(()));

    let key = report
        .declarations
        .iter_mut()
        .flat_map(|declaration| &mut declaration.unsupported_keys)
        .find(|key| key.key.len() < 64)
        .expect("one byte remains for plus-one observation");
    key.key.push('x');
    assert_eq!(boundary_report_length(&report), maximum + 1);
    assert_eq!(
        validate_repository_boundary_report_size(&report),
        Err(RepositoryBoundaryError::LimitExceeded {
            limit: BoundaryLimit::BoundaryReportBytes,
            maximum: MAX_BOUNDARY_REPORT_BYTES,
            observed: MAX_BOUNDARY_REPORT_BYTES + 1,
        })
    );
}

fn manifest(entries: &[Value]) -> Value {
    json!({
        "schema_version": "codenoesis.repository-boundary-input/v1",
        "root": {"repository_identity": ROOT_IDENTITY, "commit_oid": ROOT_COMMIT},
        "nested_repositories": entries
    })
}

fn entry(index: usize) -> Value {
    json!({
        "boundary_path": format!("external/model-{index:02}"),
        "repository_identity": format!("urn:codenoesis:repository:nested-{index:02}"),
        "repository_root": format!("generated/model-{index:02}"),
        "revision": format!("{index:040x}"),
        "acquisition_profile": if index.is_multiple_of(2) {
            "verified-loose-sha1-v1"
        } else {
            "local-git-sha1-packed-v1"
        }
    })
}

fn parse_value(
    value: &Value,
) -> Result<codenoesis_domain::s1_boundaries::RepositoryBoundaryInput, RepositoryBoundaryInputError>
{
    parse_repository_boundary_input(&serde_json::to_vec(&value).unwrap())
}

fn assert_invalid(
    result: Result<
        codenoesis_domain::s1_boundaries::RepositoryBoundaryInput,
        RepositoryBoundaryInputError,
    >,
    expected: BoundaryManifestReason,
) {
    assert_eq!(
        result.unwrap_err(),
        RepositoryBoundaryInputError::Invalid(expected)
    );
}

fn orphan_report(key_count: usize) -> codenoesis_domain::s1_boundaries::RepositoryBoundaryReport {
    let mut bytes = Vec::new();
    for section in 0..256 {
        bytes.extend_from_slice(
            format!(
                "[submodule \"s{section}\"]\npath = orphan-{section:03}\nurl = https:orphan-{section:03}\n"
            )
            .as_bytes(),
        );
        for key in 0..key_count {
            bytes.extend_from_slice(format!("k-{key:02} = value\n").as_bytes());
        }
    }
    let root = BoundRevision::new(
        RepositoryIdentity::parse(ROOT_IDENTITY).unwrap(),
        ObjectId::parse_sha1(ROOT_COMMIT).unwrap(),
        ObjectId::parse_sha1("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").unwrap(),
    );
    let source = AcquiredGitmodules {
        mode: "100644".to_owned(),
        blob_oid: ObjectId::parse_sha1("cccccccccccccccccccccccccccccccccccccccc").unwrap(),
        bytes,
    };
    let parsed = parse_gitmodules(&root, Some(&source), &TestHasher).unwrap();
    build_boundary_report(&root, Vec::new(), parsed, &[], &TestHasher).unwrap()
}

fn boundary_report_length(
    report: &codenoesis_domain::s1_boundaries::RepositoryBoundaryReport,
) -> usize {
    serde_json::to_vec(&repository_boundary_value(report))
        .unwrap()
        .len()
}
