use codenoesis_contracts::{
    CODENOESIS_ERROR_V25_SCHEMA, CodeNoesisErrorV25, LOCAL_EXPERIMENTAL_R17_PROFILE,
    MAX_RELEASE_PROFILE_CAPABILITIES, ReleaseProfileError, embedded_release_profile_registry_v1,
    release_profile_v1_for_target, release_profile_v1_from_registry,
    validate_release_profile_registry_v1,
};
use serde_json::Value;

const REGISTRY: &str =
    include_str!("../../../tests/specifications/g0/release-profile-v1/registry-v1.json");
const LINUX: &[u8] = include_bytes!(
    "../../../tests/specifications/g0/release-profile-v1/expected-x86_64-unknown-linux-gnu.json"
);
const MACOS: &[u8] = include_bytes!(
    "../../../tests/specifications/g0/release-profile-v1/expected-aarch64-apple-darwin.json"
);
const WINDOWS: &[u8] = include_bytes!(
    "../../../tests/specifications/g0/release-profile-v1/expected-x86_64-pc-windows-msvc.json"
);

#[test]
fn conf_fr_rel_001_embedded_registry_matches_reviewed_fixture() {
    let fixture: Value = serde_json::from_str(REGISTRY).expect("parse reviewed registry");
    assert_eq!(embedded_release_profile_registry_v1(), fixture);
    assert_eq!(validate_release_profile_registry_v1(&fixture), Ok(()));
}

#[test]
fn conf_fr_rel_001_each_target_matches_exact_golden() {
    for (target, expected) in [
        ("x86_64-unknown-linux-gnu", LINUX),
        ("aarch64-apple-darwin", MACOS),
        ("x86_64-pc-windows-msvc", WINDOWS),
    ] {
        let report = release_profile_v1_for_target(LOCAL_EXPERIMENTAL_R17_PROFILE, target)
            .expect("accepted target");
        assert_eq!(
            report.canonical_stdout().expect("canonical report"),
            expected
        );
    }
}

#[test]
fn pt_fr_rel_001_fifty_constructions_and_ten_schedules_are_identical() {
    let expected =
        release_profile_v1_for_target(LOCAL_EXPERIMENTAL_R17_PROFILE, "x86_64-unknown-linux-gnu")
            .expect("accepted target")
            .canonical_stdout()
            .expect("canonical report");

    for construction in 0..50 {
        let registry: Value = serde_json::from_str(REGISTRY).expect("parse reviewed registry");
        let report = release_profile_v1_from_registry(
            &registry,
            LOCAL_EXPERIMENTAL_R17_PROFILE,
            "x86_64-unknown-linux-gnu",
        )
        .expect("construct report");
        assert_eq!(
            report.canonical_stdout().expect("canonical report"),
            expected,
            "construction {construction}"
        );
    }

    for schedule in 0..10 {
        let report = release_profile_v1_for_target(
            LOCAL_EXPERIMENTAL_R17_PROFILE,
            "x86_64-unknown-linux-gnu",
        )
        .expect("accepted target");
        assert_eq!(
            report.canonical_stdout().expect("canonical report"),
            expected,
            "schedule {schedule}"
        );
    }
}

#[test]
fn sec_fr_rel_001_invalid_private_and_oversized_registry_fails_closed() {
    let mut private = embedded_release_profile_registry_v1();
    private["profiles"][0]["source_text"] = Value::String("private".to_owned());
    assert_eq!(
        validate_release_profile_registry_v1(&private),
        Err(ReleaseProfileError::ContractInvalid)
    );

    let mut oversized = embedded_release_profile_registry_v1();
    oversized["profiles"][0]["capabilities"] = Value::Array(
        (0..=MAX_RELEASE_PROFILE_CAPABILITIES)
            .map(|index| Value::String(format!("capability-{index:02}")))
            .collect(),
    );
    assert_eq!(
        validate_release_profile_registry_v1(&oversized),
        Err(ReleaseProfileError::ContractInvalid)
    );
}

#[test]
fn conf_fr_cli_007_unknown_and_unsupported_are_strict_error_v25() {
    for (error, code) in [
        (ReleaseProfileError::UnknownProfile, "profile.unknown"),
        (
            ReleaseProfileError::UnsupportedPlatform,
            "profile.unsupported_platform",
        ),
        (
            ReleaseProfileError::ContractInvalid,
            "profile.contract_invalid",
        ),
    ] {
        let error = CodeNoesisErrorV25::from_release_profile(error);
        let bytes = error.canonical_stderr().expect("canonical ErrorV25");
        assert!(bytes.ends_with(b"\n"));
        let value: Value = serde_json::from_slice(&bytes).expect("parse ErrorV25");
        assert_eq!(value["schema_version"], CODENOESIS_ERROR_V25_SCHEMA);
        assert_eq!(value["code"], code);
        assert_eq!(value["retryable"], false);
    }

    assert_eq!(
        release_profile_v1_for_target("unknown", "x86_64-unknown-linux-gnu")
            .expect_err("unknown profile"),
        ReleaseProfileError::UnknownProfile
    );
    assert_eq!(
        release_profile_v1_for_target(
            LOCAL_EXPERIMENTAL_R17_PROFILE,
            "riscv64gc-unknown-linux-gnu"
        )
        .expect_err("unsupported target"),
        ReleaseProfileError::UnsupportedPlatform
    );
}
