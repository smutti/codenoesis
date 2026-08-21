#[path = "support/s7_r19.rs"]
mod s7_r19;
mod support;

use s7_r19::MaterializedGitImpactWorkspace;

#[test]
fn e2e_fr_imp_006_git_bound_semantic_diff_is_navigable() {
    let workspace = MaterializedGitImpactWorkspace::reviewed();
    assert!(workspace.root().is_dir(), "R19 fixture root is absent");
    let output = workspace.impact();
    if !output.status.success() {
        assert_eq!(output.status.code(), Some(2), "unexpected R19 Red exit");
        assert!(output.stdout.is_empty(), "R19 Red wrote a partial report");
        let error: serde_json::Value =
            serde_json::from_slice(&output.stderr).expect("parse R19 Red error");
        assert_eq!(error["schema_version"], "codenoesis.error/v23");
        assert_eq!(error["code"], "impact.invalid_workspace");
        assert_eq!(error["context"]["reason"], "invalid_profile");
        panic!("expected R19 Git-backed impact success; observed the approved absent-profile Red");
    }

    assert!(
        output.stderr.is_empty(),
        "successful R19 impact wrote stderr"
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse R19 report V2");
    assert_eq!(
        report["schema_version"],
        "codenoesis.semantic-compatibility-report/v2"
    );
    assert_eq!(report["semantic_diffs"].as_array().map(Vec::len), Some(2));
    assert_eq!(
        report["client_assessments"].as_array().map(Vec::len),
        Some(2)
    );
    assert_eq!(
        report["rejected_candidates"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(report["evidence"].as_array().map(Vec::len), Some(9));
    assert_eq!(report["coverage_gaps"].as_array().map(Vec::len), Some(1));

    let breaking = report["semantic_diffs"]
        .as_array()
        .expect("R19 semantic diffs")
        .iter()
        .find(|diff| diff["classification"] == "breaking")
        .expect("R19 breaking semantic diff");
    let evidence_id = breaking["evidence_ids"]
        .as_array()
        .expect("R19 breaking evidence")
        .first()
        .and_then(serde_json::Value::as_str)
        .expect("R19 selected evidence ID");
    let evidence = report["evidence"]
        .as_array()
        .expect("R19 evidence")
        .iter()
        .find(|record| record["id"] == evidence_id)
        .expect("R19 selected evidence record");
    let report_path = workspace.write_report(&output.stdout);
    let source = workspace.impact_source(&report_path, evidence);
    assert!(
        source.status.success(),
        "R19 source navigation failed: {}",
        String::from_utf8_lossy(&source.stderr)
    );
    assert!(source.stderr.is_empty());
    let excerpt: serde_json::Value =
        serde_json::from_slice(&source.stdout).expect("parse R19 source excerpt");
    assert_eq!(
        excerpt["schema_version"],
        "codenoesis.trusted-impact-source-excerpt/v1"
    );
    assert_eq!(excerpt["evidence"]["id"], evidence_id);
    assert!(
        excerpt["excerpt"]["byte_length"]
            .as_u64()
            .is_some_and(|length| length > 0)
    );
}
