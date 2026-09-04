mod support;

use std::process::{Command, Output};

use serde_json::Value;

use support::parse_single_document;
use support::s4_r17::{
    MaterializedFunctionContextRepository, REPOSITORY_ID, ROOT_CALLABLE_ID,
    expected_function_context,
};

const LLM_CONTEXT_PROFILE: &str = "rust-llm-context-v1";

fn assert_success(output: &Output, label: &str) {
    assert!(
        output.status.success(),
        "{label} failed: status={:?}; stdout={}; stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "{label} stderr must be empty");
}

fn query_llm_context(
    repository: &MaterializedFunctionContextRepository,
    requested_id: &str,
) -> Output {
    Command::new(env!("CARGO_BIN_EXE_noesis"))
        .args(["query", "--store"])
        .arg(&repository.store)
        .args(["--repository-id", REPOSITORY_ID, "--documents"])
        .arg(&repository.documents)
        .args([
            "--id",
            requested_id,
            "--context-profile",
            LLM_CONTEXT_PROFILE,
            "--format",
            "json",
        ])
        .output()
        .expect("launch compact LLM-context query")
}

#[test]
fn e2e_rw4_compact_llm_function_context_is_deterministic_and_useful() {
    let repository = MaterializedFunctionContextRepository::fixture();
    assert_success(&repository.scan(), "RW4 fixture scan");
    assert_success(&repository.docs(), "RW4 documentation");

    let full_context = repository.query_context(ROOT_CALLABLE_ID);
    assert_success(&full_context, "RW4 full function context");
    let output = query_llm_context(&repository, ROOT_CALLABLE_ID);
    assert_success(&output, "RW4 compact LLM function context");
    assert!(output.stdout.len() < full_context.stdout.len());
    for schedule in 1..10 {
        let replay = query_llm_context(&repository, ROOT_CALLABLE_ID);
        assert_success(&replay, "RW4 compact LLM function context replay");
        assert_eq!(replay.stdout, output.stdout, "LLM schedule {schedule}");
    }

    let llm = parse_single_document(&output.stdout);
    let oracle = expected_function_context();
    assert_eq!(llm["schema_version"], "codenoesis.llm-function-context/v1");
    assert_eq!(llm["profile"], LLM_CONTEXT_PROFILE);
    assert_eq!(llm["authority"], "declared_source_only");
    assert_eq!(llm["model_authority"], false);
    assert_eq!(llm["focus"]["name"], "scale");
    assert_eq!(llm["focus"]["kind"], "rust.method");
    assert_eq!(
        llm["focus"]["signature"]["display"],
        oracle["display_signature"]
    );
    assert_eq!(
        llm["focus"]["signature"]["output"]["declared_type"],
        "Result<i32, T>"
    );
    assert_eq!(
        llm["focus"]["signature"]["inputs"]
            .as_array()
            .expect("LLM inputs")
            .len(),
        3
    );
    assert_eq!(llm["calls"]["outgoing"][0]["target_name"], "clamp");
    assert!(
        !llm["evidence_summary"]["locations"]
            .as_array()
            .expect("LLM evidence locations")
            .is_empty()
    );
    assert_eq!(llm["uncertainty"]["limitations"], oracle["limitations"]);
    assert!(llm.get("relationships").is_none());
    assert!(llm.get("claims").is_none());
    assert!(llm.get("navigation").is_none());
    assert_eq!(llm["body_facts"].as_array().map(Vec::len), Some(4));
    assert_eq!(llm["calls"]["incoming"], Value::Array(Vec::new()));
    assert!(llm["claim_summary"]["states"][0].get("state").is_some());
    assert_eq!(llm["resource_bounds"]["source_counts"]["parameters"], 3);
}
