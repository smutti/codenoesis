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
