use std::collections::BTreeSet;

use codenoesis_contracts::{
    CodeNoesisErrorV29, MAX_R18_EXCERPT_BYTES, MAX_R18_PATH_BYTES, MAX_R18_STDOUT_BYTES,
    R18_ERROR_VERSION, R18_SOURCE_EXCERPT_VERSION, R18_SOURCE_PROFILE, TrustedSourceError,
    TrustedSourceExcerptV1,
};
use serde_json::Value;

const EXPECTED_EXCERPT: &str = "pub fn scale<T>(&self, value: i32, fallback: T) -> Result<i32, T>\n    where\n        T: Clone,\n    ";
const EXPECTED_EXCERPT_SHA256: &str =
    "2beedeaf7f4333bd21ec5b33de802f1b2006377ad6435ebc983b16029fd19f83";

#[test]
fn ct_fr_ctx_002_reviewed_oracle_is_strict_canonical_output() {
    let value: Value = serde_json::from_slice(include_bytes!(
        "../../../tests/fixtures/s4/trusted-source-retrieval-v1/expected-source-excerpt.json"
    ))
    .expect("reviewed R18 source oracle");
    let mut canonical = serde_json::to_vec(&value).expect("canonical R18 oracle");
    canonical.push(b'\n');

    let parsed = TrustedSourceExcerptV1::from_canonical_stdout(&canonical, fixture_sha256)
        .expect("strict canonical R18 oracle");
    assert_eq!(parsed.canonical_stdout(), canonical);
    assert_eq!(parsed.value(), &value);

    let pretty = include_bytes!(
        "../../../tests/fixtures/s4/trusted-source-retrieval-v1/expected-source-excerpt.json"
    );
    assert_eq!(
        TrustedSourceExcerptV1::from_canonical_stdout(pretty, fixture_sha256).unwrap_err(),
        TrustedSourceError::InvalidEvidence
    );

    let mut changed = value;
    changed["excerpt"]["text"] = Value::String(format!("{EXPECTED_EXCERPT}x"));
    changed["excerpt"]["byte_length"] = Value::from(99_u64);
    changed["evidence"]["span"]["end"] = Value::from(317_u64);
    let mut changed = serde_json::to_vec(&changed).expect("changed oracle");
    changed.push(b'\n');
    assert_eq!(
        TrustedSourceExcerptV1::from_canonical_stdout(&changed, fixture_sha256).unwrap_err(),
        TrustedSourceError::ContentRejected
    );
}

#[test]
fn ct_fr_cli_011_contract_constants_are_exact() {
    assert_eq!(R18_SOURCE_PROFILE, "trusted-local-source-v1");
    assert_eq!(
        R18_SOURCE_EXCERPT_VERSION,
        "codenoesis.trusted-source-excerpt/v1"
    );
    assert_eq!(R18_ERROR_VERSION, "codenoesis.error/v29");
    assert_eq!(MAX_R18_EXCERPT_BYTES, 262_144);
    assert_eq!(MAX_R18_STDOUT_BYTES, 524_288);
    assert_eq!(MAX_R18_PATH_BYTES, 1_024);
}

#[test]
fn ct_fr_cli_011_error_v29_is_strict_private_and_lf_terminated() {
    let errors = [
        (
            CodeNoesisErrorV29::invalid_arguments(),
            "source.invalid_arguments",
            "input",
        ),
        (
            CodeNoesisErrorV29::acquisition_rejected(),
            "source.acquisition_rejected",
            "source",
        ),
        (
            CodeNoesisErrorV29::path_rejected(),
            "source.path_rejected",
            "source",
        ),
        (
            CodeNoesisErrorV29::limit_exceeded(),
            "source.limit_exceeded",
            "source",
        ),
        (
            CodeNoesisErrorV29::unstable_input(),
            "source.unstable_input",
            "source",
        ),
        (
            CodeNoesisErrorV29::repository_mismatch(),
            "source.repository_mismatch",
            "source",
        ),
        (
            CodeNoesisErrorV29::invalid_snapshot(),
            "source.invalid_snapshot",
            "source",
        ),
        (
            CodeNoesisErrorV29::internal(),
            "source.internal",
            "internal",
        ),
    ];

    for (error, code, stage) in errors {
        let bytes = error.canonical_stderr().expect("canonical ErrorV29");
        assert!(bytes.ends_with(b"\n"));
        assert!(!bytes[..bytes.len() - 1].contains(&b'\n'));
        let value: Value =
            serde_json::from_slice(&bytes[..bytes.len() - 1]).expect("strict ErrorV29 JSON");
        let keys = value
            .as_object()
            .expect("ErrorV29 object")
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            keys,
            [
                "code",
                "context",
                "message",
                "retryable",
                "schema_version",
                "stage"
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(value["schema_version"], R18_ERROR_VERSION);
        assert_eq!(value["code"], code);
        assert_eq!(value["stage"], stage);
        assert_eq!(value["retryable"], false);
        assert_eq!(value["context"], serde_json::json!({}));
    }
}

fn fixture_sha256(bytes: &[u8]) -> String {
    if bytes == EXPECTED_EXCERPT.as_bytes() {
        EXPECTED_EXCERPT_SHA256.to_owned()
    } else {
        "0".repeat(64)
    }
}
