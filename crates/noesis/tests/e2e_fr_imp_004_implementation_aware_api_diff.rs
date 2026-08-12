mod support;

use std::process::Command;

use support::parse_single_document;
use support::s7::{MaterializedImpactWorkspace, reviewed_golden};

#[test]
fn e2e_fr_imp_004_implementation_aware_api_diff() {
    let workspace = MaterializedImpactWorkspace::reviewed();
    let output = Command::new(env!("CARGO_BIN_EXE_noesis"))
        .args(["impact", "--workspace"])
        .arg(&workspace.manifest)
        .args([
            "--profile",
            "implementation-aware-http-json-v1",
            "--format",
            "json",
        ])
        .output()
        .expect("launch noesis impact");

    if !output.status.success() {
        assert_eq!(output.status.code(), Some(2), "unexpected impact exit");
        assert!(output.stdout.is_empty(), "failed impact wrote a report");
        let error = parse_single_document(&output.stderr);
        assert_eq!(error["schema_version"], "codenoesis.error/v23");
        assert_eq!(
            error["code"], "impact.unsupported_implementation_semantics",
            "S7 product Red reached the wrong boundary"
        );
        assert_eq!(error["stage"], "impact");
        panic!("expected S7 impact success; observed the approved unsupported-semantics Red");
    }

    assert!(output.stderr.is_empty(), "successful impact wrote stderr");
    assert_eq!(
        output.stdout,
        reviewed_golden(),
        "S7 semantic compatibility report differs from the immutable golden"
    );
}
