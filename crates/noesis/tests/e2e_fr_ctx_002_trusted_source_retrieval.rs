mod support;

use std::process::Output;

use support::parse_single_document;
use support::s4_r17::ROOT_CALLABLE_ID;
use support::s4_r18::{
    MaterializedTrustedSourceRepository, SIGNATURE_EVIDENCE_ID, expected_source_excerpt,
};

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

#[test]
fn e2e_fr_ctx_002_retrieves_exact_committed_excerpt() {
    let repository = MaterializedTrustedSourceRepository::fixture();
    let scan = repository.inherited.scan();
    assert_success(&scan, "R18 fixture scan");
    let docs = repository.inherited.docs();
    assert_success(&docs, "R18 documentation");

    let context_output = repository.inherited.query_context(ROOT_CALLABLE_ID);
    assert_success(&context_output, "R18 inherited function context");
    let context = parse_single_document(&context_output.stdout);
    assert!(
        context["evidence"]
            .as_array()
            .expect("R18 context evidence")
            .iter()
            .any(|record| record["id"] == SIGNATURE_EVIDENCE_ID),
        "reviewed signature evidence must be navigable from FunctionContextV1"
    );

    let source = repository.source(SIGNATURE_EVIDENCE_ID);
    assert_success(&source, "R18 trusted source excerpt");
    assert_eq!(
        parse_single_document(&source.stdout),
        expected_source_excerpt()
    );
}
