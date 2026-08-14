use codenoesis_contracts::{
    CodeNoesisErrorV27, LocalBundleIdentityV1, LocalRollbackReportV1, LocalUpgradeContractError,
    LocalUpgradePlanV1, parse_local_upgrade_plan_v1, validate_local_distribution_manifest_v1,
};

const CURRENT_BINARY_SHA256: &str =
    "f82e988be32ec8a3f077e49f4034d42847adc415cd05ec82a9affec2fe25fb6b";
const CANDIDATE_BINARY_SHA256: &str =
    "9d0005fbbae37436afe941a35dd6324b015be474371f808c6016b031f24d92ad";

struct TargetOracle {
    target: &'static str,
    current_manifest: &'static [u8],
    current_manifest_sha256: &'static str,
    candidate_manifest_sha256: &'static str,
    expected_plan: &'static [u8],
    plan_sha256: &'static str,
    expected_rollback: &'static [u8],
}

#[test]
fn ct_fr_cmp_001_target_plans_and_rollbacks_match_exact_goldens() {
    for oracle in target_oracles() {
        let manifest =
            validate_local_distribution_manifest_v1(&canonical_lf_golden(oracle.current_manifest))
                .expect("validate exact G1a manifest");
        assert_eq!(manifest.target(), oracle.target);
        assert_eq!(manifest.binary_sha256(), CURRENT_BINARY_SHA256);
        assert_eq!(manifest.files().len(), 5);

        let current = bundle_identity(
            oracle.target,
            CURRENT_BINARY_SHA256,
            oracle.current_manifest_sha256,
        );
        let candidate = bundle_identity(
            oracle.target,
            CANDIDATE_BINARY_SHA256,
            oracle.candidate_manifest_sha256,
        );
        let plan = LocalUpgradePlanV1::new(oracle.target, current.clone(), candidate.clone())
            .expect("build upgrade plan");
        let plan_bytes = plan.canonical_stdout().expect("serialize upgrade plan");
        assert_eq!(plan_bytes, canonical_lf_golden(oracle.expected_plan));
        let parsed = parse_local_upgrade_plan_v1(&plan_bytes).expect("parse exact plan");
        assert_eq!(parsed.current(), &current);
        assert_eq!(parsed.candidate(), &candidate);

        let rollback =
            LocalRollbackReportV1::new(&parsed, &candidate, &current, oracle.plan_sha256)
                .expect("build rollback report");
        assert_eq!(
            rollback.canonical_stdout().expect("serialize rollback"),
            canonical_lf_golden(oracle.expected_rollback)
        );
    }
}

#[test]
fn sec_fr_cmp_001_noncanonical_plan_and_substitution_fail_closed() {
    let oracle = &target_oracles()[0];
    let expected = canonical_lf_golden(oracle.expected_plan);
    let mut whitespace = expected.clone();
    whitespace.insert(0, b' ');
    assert!(matches!(
        parse_local_upgrade_plan_v1(&whitespace),
        Err(LocalUpgradeContractError::InvalidPlan)
    ));

    let current = bundle_identity(
        oracle.target,
        CURRENT_BINARY_SHA256,
        oracle.current_manifest_sha256,
    );
    let candidate = bundle_identity(
        oracle.target,
        CANDIDATE_BINARY_SHA256,
        oracle.candidate_manifest_sha256,
    );
    assert!(matches!(
        LocalUpgradePlanV1::new(oracle.target, current.clone(), current.clone()),
        Err(LocalUpgradeContractError::Incompatible)
    ));
    let plan = LocalUpgradePlanV1::new(oracle.target, current.clone(), candidate.clone()).unwrap();
    assert!(matches!(
        LocalRollbackReportV1::new(&plan, &current, &candidate, oracle.plan_sha256),
        Err(LocalUpgradeContractError::Incompatible)
    ));

    let oversized = vec![b' '; 65_537];
    assert!(matches!(
        parse_local_upgrade_plan_v1(&oversized),
        Err(LocalUpgradeContractError::LimitExceeded)
    ));
}

#[test]
fn ct_fr_cli_009_error_v27_matches_exact_goldens() {
    let cases = [
        (
            CodeNoesisErrorV27::invalid_arguments(),
            include_bytes!(
                "../../../tests/specifications/g2/local-upgrade-safety-v1/expected-error-invalid-arguments.json"
            )
            .as_slice(),
        ),
        (
            CodeNoesisErrorV27::incompatible(),
            include_bytes!(
                "../../../tests/specifications/g2/local-upgrade-safety-v1/expected-error-incompatible.json"
            )
            .as_slice(),
        ),
        (
            CodeNoesisErrorV27::invalid_plan(),
            include_bytes!(
                "../../../tests/specifications/g2/local-upgrade-safety-v1/expected-error-invalid-plan.json"
            )
            .as_slice(),
        ),
    ];
    for (error, expected) in cases {
        assert_eq!(
            error.canonical_stderr().expect("serialize ErrorV27"),
            canonical_lf_golden(expected)
        );
    }
}

fn bundle_identity(
    target: &str,
    binary_sha256: &str,
    manifest_sha256: &str,
) -> LocalBundleIdentityV1 {
    LocalBundleIdentityV1::new(
        target,
        binary_sha256,
        format!("codenoesis-local-experimental-r17-{target}-{binary_sha256}"),
        manifest_sha256,
    )
    .expect("build bundle identity")
}

fn target_oracles() -> Vec<TargetOracle> {
    vec![
        TargetOracle {
            target: "aarch64-apple-darwin",
            current_manifest: include_bytes!(
                "../../../tests/specifications/g1/local-cli-distribution-v1/expected-manifest-aarch64-apple-darwin.json"
            ),
            current_manifest_sha256: "757a9253736f2712dbe60a480af4ffa6b7d3b06a39458bc0e8714be288caf7b1",
            candidate_manifest_sha256: "fb6e8f203768cc7bbdce3deaece09b46ead5d519d5ce7bed308edb31332c6b59",
            expected_plan: include_bytes!(
                "../../../tests/specifications/g2/local-upgrade-safety-v1/expected-upgrade-plan-aarch64-apple-darwin.json"
            ),
            plan_sha256: "514d1eacf9dc61410e797c6fcf6424dd3fd5e5325d1f25db602335f9869fe23f",
            expected_rollback: include_bytes!(
                "../../../tests/specifications/g2/local-upgrade-safety-v1/expected-rollback-report-aarch64-apple-darwin.json"
            ),
        },
        TargetOracle {
            target: "x86_64-pc-windows-msvc",
            current_manifest: include_bytes!(
                "../../../tests/specifications/g1/local-cli-distribution-v1/expected-manifest-x86_64-pc-windows-msvc.json"
            ),
            current_manifest_sha256: "409e6050f606b5067ed63896ca54b9a1fbe3577fe20e253152ab79aec45486e5",
            candidate_manifest_sha256: "f234b15b8d4326005d40642e1a3cb23a5b02c5d5c9478a08146f2492945c6d64",
            expected_plan: include_bytes!(
                "../../../tests/specifications/g2/local-upgrade-safety-v1/expected-upgrade-plan-x86_64-pc-windows-msvc.json"
            ),
            plan_sha256: "94b95ba5627e77503459d65cb01b59cb1282a65de3b52eedcd0100256ce05bf4",
            expected_rollback: include_bytes!(
                "../../../tests/specifications/g2/local-upgrade-safety-v1/expected-rollback-report-x86_64-pc-windows-msvc.json"
            ),
        },
        TargetOracle {
            target: "x86_64-unknown-linux-gnu",
            current_manifest: include_bytes!(
                "../../../tests/specifications/g1/local-cli-distribution-v1/expected-manifest-x86_64-unknown-linux-gnu.json"
            ),
            current_manifest_sha256: "d332001a85f9ae7608cf6ec8dd4f0aad03e6c77478052bf526bca63693eb6faf",
            candidate_manifest_sha256: "5ae7ec9b0a0c3196b73ac44d25f54b5877186fbea1be2373dda668f4a9ece4a1",
            expected_plan: include_bytes!(
                "../../../tests/specifications/g2/local-upgrade-safety-v1/expected-upgrade-plan-x86_64-unknown-linux-gnu.json"
            ),
            plan_sha256: "49a200ec38108c668be6d7147708904e0aacc6d2e4924d5d537dac92ca2b705c",
            expected_rollback: include_bytes!(
                "../../../tests/specifications/g2/local-upgrade-safety-v1/expected-rollback-report-x86_64-unknown-linux-gnu.json"
            ),
        },
    ]
}

fn canonical_lf_golden(checked_out: &[u8]) -> Vec<u8> {
    let body = checked_out
        .strip_suffix(b"\r\n")
        .or_else(|| checked_out.strip_suffix(b"\n"))
        .expect("golden has one final LF or CRLF");
    assert!(!body.contains(&b'\r'));
    assert!(!body.contains(&b'\n'));
    let mut canonical = body.to_vec();
    canonical.push(b'\n');
    canonical
}
