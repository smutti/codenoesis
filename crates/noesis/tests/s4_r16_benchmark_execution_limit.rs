mod support;

use support::parse_single_document;
use support::s4_r16::{
    MaterializedConstantEvaluationRepository, expected_safe_constant_evaluation,
};

const BENCHMARK_EXECUTION_LIMIT_PROFILE: &str = "real-world-rust-benchmark-75s-v1";

#[test]
fn e2e_nfr_per_001_benchmark_r16_accepts_explicit_75s_execution_limit() {
    let repository = MaterializedConstantEvaluationRepository::fixture();
    let output = repository.scan_with_options(&[
        "--acquisition-profile",
        "local-git-sha1-packed-v1",
        "--output-capacity-profile",
        "local-snapshot-256m-v1",
        "--execution-limit-profile",
        BENCHMARK_EXECUTION_LIMIT_PROFILE,
    ]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "benchmark R16 selector failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let snapshot = parse_single_document(&output.stdout);
    let expected = expected_safe_constant_evaluation();
    assert_eq!(
        snapshot["schema_version"],
        "codenoesis.repository-snapshot/v18"
    );
    assert_eq!(
        snapshot["semantic_hash"]["value"],
        expected["expected_hashes"]["snapshot"]
    );
    assert_eq!(
        snapshot["semantic"]["configuration"]["semantic_hash"]["value"],
        expected["expected_hashes"]["configuration"]
    );
    assert_eq!(
        snapshot["semantic"]["knowledge_graph"]["semantic_hash"]["value"],
        expected["expected_hashes"]["knowledge_graph"]
    );
}

#[test]
fn conf_fr_cli_001_benchmark_execution_limit_compositions_fail_closed() {
    for (options, expected_reason) in [
        (
            vec![
                "--acquisition-profile",
                "local-git-sha1-packed-v1",
                "--output-capacity-profile",
                "local-snapshot-256m-v1",
                "--execution-limit-profile",
                "unknown",
            ],
            "valid_execution_limit_profile_required",
        ),
        (
            vec![
                "--acquisition-profile",
                "local-git-sha1-packed-v1",
                "--output-capacity-profile",
                "local-snapshot-256m-v1",
                "--execution-limit-profile",
                BENCHMARK_EXECUTION_LIMIT_PROFILE,
                "--execution-limit-profile",
                BENCHMARK_EXECUTION_LIMIT_PROFILE,
            ],
            "single_execution_limit_profile_required",
        ),
        (
            vec![
                "--output-capacity-profile",
                "local-snapshot-256m-v1",
                "--execution-limit-profile",
                BENCHMARK_EXECUTION_LIMIT_PROFILE,
            ],
            "benchmark_execution_limit_requires_packed_r16_256m",
        ),
        (
            vec![
                "--acquisition-profile",
                "local-git-sha1-packed-v1",
                "--output-capacity-profile",
                "local-snapshot-64m-v1",
                "--execution-limit-profile",
                BENCHMARK_EXECUTION_LIMIT_PROFILE,
            ],
            "benchmark_execution_limit_requires_packed_r16_256m",
        ),
    ] {
        assert_typed_rejection(&options, expected_reason);
    }
    assert_typed_rejection(
        &["--execution-limit-profile"],
        "complete_option_pair_required",
    );
}

#[test]
fn conf_fr_cli_001_benchmark_execution_limit_requires_exact_r12_boundary_lineage() {
    assert_typed_rejection(
        &[
            "--acquisition-profile",
            "local-git-sha1-packed-v1",
            "--output-capacity-profile",
            "local-snapshot-256m-v1",
            "--execution-limit-profile",
            BENCHMARK_EXECUTION_LIMIT_PROFILE,
            "--repository-boundary-profile",
            "local-gitlinks-v1",
        ],
        "exact_r15_selector_matrix_required",
    );
}

fn assert_typed_rejection(options: &[&str], expected_reason: &str) {
    let repository = MaterializedConstantEvaluationRepository::fixture();
    let output = repository.scan_with_options(options);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error = parse_single_document(&output.stderr);
    assert_eq!(error["schema_version"], "codenoesis.error/v24");
    assert_eq!(
        error["code"],
        "input.unsupported_rust_constant_evaluation_composition"
    );
    assert_eq!(error["context"]["reason"], expected_reason);
    assert!(!repository.store.exists());
    assert!(!repository.build_sentinel().exists());
}
