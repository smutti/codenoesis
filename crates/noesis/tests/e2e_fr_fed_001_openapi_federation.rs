use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

const PRE_S6_ERROR_SHA256: &str =
    "6441e0037f864d2fae4a60e6355e4a85b26b00d5e4e24c59ffeb5fe9c6f3859f";
const WORKSPACE_BYTES_MAXIMUM: usize = 8_388_608;
const CONTRACT_BYTES_MAXIMUM: usize = 2_097_152;
const REPORT_BYTES_MAXIMUM: u64 = 67_108_864;

#[test]
fn e2e_fr_fed_001_openapi_federation() {
    let fixture = fixture_root();
    let output = federate(&fixture.join("workspace.json"));

    assert_success_or_expected_red(&output);
    assert!(
        output.stderr.is_empty(),
        "successful federation wrote stderr"
    );
    assert_eq!(
        output.stdout,
        fs::read(fixture.join("expected-federation-report.json"))
            .expect("read reviewed S6 federation report"),
        "S6 federation report differs from the reviewed artifact"
    );
}

#[test]
fn gt_fr_cli_005_provider_only_workspace_is_exact() {
    assert_reviewed_success(
        "workspace-provider-only.json",
        "expected-provider-only-report.json",
    );
}

#[test]
fn gt_fr_ext_004_unsupported_semantics_are_exact_gaps() {
    assert_reviewed_success(
        "workspace-unsupported-semantics.json",
        "expected-unsupported-semantics-report.json",
    );
}

#[test]
fn gt_fr_ext_004_yaml_json_reports_are_semantically_equivalent() {
    let yaml_output = federate(&fixture_root().join("workspace.json"));
    assert!(yaml_output.status.success());
    let mut workspace = read_json(&fixture_root().join("workspace.json"));
    workspace["provider"]["contract_path"] = json!("openapi.json");
    workspace["provider"]["contract_sha256"] = json!(sha256_hex(
        &fs::read(fixture_root().join("provider/openapi.json")).unwrap()
    ));
    let materialized = MaterializedWorkspace::with_inputs(&workspace);
    let json_output = federate(&materialized.manifest);
    assert!(json_output.status.success(), "{json_output:?}");

    let yaml_report: Value = serde_json::from_slice(&yaml_output.stdout).unwrap();
    let json_report: Value = serde_json::from_slice(&json_output.stdout).unwrap();
    assert_eq!(yaml_report["semantic_hash"], json_report["semantic_hash"]);
    assert_eq!(json_report["provider"]["source_format"], "json");
    assert_eq!(
        yaml_report["provider"]["service_id"],
        json_report["provider"]["service_id"]
    );
    for key in [
        "operations",
        "clients",
        "confirmed_links",
        "candidates",
        "rejections",
        "coverage_gaps",
    ] {
        let mut yaml_value = yaml_report[key].clone();
        let mut json_value = json_report[key].clone();
        remove_evidence_references(&mut yaml_value);
        remove_evidence_references(&mut json_value);
        assert_eq!(yaml_value, json_value, "source-neutral mismatch in {key}");
    }
    assert!(
        json_report["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| item["path"] == "provider/openapi.json")
            .all(|item| item["kind"] == "openapi_json_pointer")
    );
}

#[test]
fn sec_fr_ext_004_hostile_contracts_fail_closed() {
    for (file, expected_code) in [
        ("duplicate-key.yaml", "contract.duplicate_key"),
        ("alias.yaml", "contract.unsupported_yaml_feature"),
        ("merge-key.yaml", "contract.unsupported_yaml_feature"),
        ("custom-tag.yaml", "contract.unsupported_yaml_feature"),
        (
            "multiple-documents.yaml",
            "contract.unsupported_yaml_feature",
        ),
        ("remote-ref.yaml", "contract.remote_reference_forbidden"),
        ("ref-cycle.yaml", "contract.reference_cycle"),
        ("malformed.yaml", "contract.invalid_yaml"),
        (
            "unsupported-openapi.yaml",
            "contract.unsupported_openapi_version",
        ),
    ] {
        let workspace = provider_variant_workspace(file);
        let materialized = MaterializedWorkspace::with_inputs(&workspace);
        let output = federate(&materialized.manifest);
        let error = assert_error(&output, 10, expected_code);
        if file == "duplicate-key.yaml" {
            assert_eq!(
                error,
                read_json(&fixture_root().join("expected-error-duplicate-key.json"))
            );
        }
        if file == "remote-ref.yaml" {
            assert_eq!(
                error,
                read_json(&fixture_root().join("expected-error-remote-ref.json"))
            );
        }
    }
}

#[test]
fn conf_fr_fed_002_conflicting_authority_fails_closed() {
    let mut workspace = read_json(&fixture_root().join("workspace.json"));
    workspace["clients"].as_array_mut().unwrap().push(json!({
        "role": "conflict",
        "root": "variants",
        "declaration_path": "conflicting-client.json",
        "declaration_sha256": sha256_hex(
            &fs::read(fixture_root().join("variants/conflicting-client.json")).unwrap()
        )
    }));
    let materialized = MaterializedWorkspace::with_inputs(&workspace);
    let output = federate(&materialized.manifest);
    let error = assert_error(&output, 10, "federation.identity_conflict");
    assert_eq!(
        error,
        read_json(&fixture_root().join("expected-error-identity-conflict.json"))
    );
}

#[test]
fn conf_fr_fed_002_heuristic_selection_is_exact() {
    let reviewed = federate(&fixture_root().join("workspace.json"));
    assert_success(&reviewed);
    let reviewed: Value = serde_json::from_slice(&reviewed.stdout).unwrap();
    assert_eq!(reviewed["candidates"].as_array().unwrap().len(), 1);
    assert_eq!(
        reviewed["coverage_gaps"][0]["reason_code"],
        "heuristic_requires_confirmation"
    );

    let provider = fs::read(fixture_root().join("provider/openapi.json")).unwrap();
    let no_match = run_heuristic_case(&provider, "Missing Service", "getUser");
    assert_success(&no_match);
    let no_match: Value = serde_json::from_slice(&no_match.stdout).unwrap();
    assert!(no_match["candidates"].as_array().unwrap().is_empty());
    assert_eq!(
        no_match["coverage_gaps"][0]["reason_code"],
        "heuristic_no_match"
    );

    let mut ambiguous_provider: Value = serde_json::from_slice(&provider).unwrap();
    let operation = ambiguous_provider["paths"]["/users/{id}"]["get"].clone();
    ambiguous_provider["paths"]["/users/by-name"] = json!({"get": operation});
    let ambiguous = run_heuristic_case(
        &serde_json::to_vec(&ambiguous_provider).unwrap(),
        "Fixture User Service",
        "getUser",
    );
    assert_success(&ambiguous);
    let ambiguous: Value = serde_json::from_slice(&ambiguous.stdout).unwrap();
    assert!(ambiguous["candidates"].as_array().unwrap().is_empty());
    assert_eq!(
        ambiguous["coverage_gaps"][0]["reason_code"],
        "heuristic_ambiguous"
    );
    assert!(
        ambiguous["confirmed_links"].as_array().unwrap().is_empty(),
        "heuristic ambiguity must never auto-confirm"
    );
}

#[test]
fn conf_fr_cli_005_invocation_and_digest_fail_without_stdout() {
    let fixture = fixture_root();
    let manifest = fixture.join("workspace.json");
    let invalid_profile = Command::new(env!("CARGO_BIN_EXE_noesis"))
        .args(["federate", "--workspace-manifest"])
        .arg(&manifest)
        .args(["--profile", "standard-local-s5", "--format", "json"])
        .output()
        .unwrap();
    assert_error(&invalid_profile, 2, "input.invalid_profile");

    let invalid_format = Command::new(env!("CARGO_BIN_EXE_noesis"))
        .args(["federate", "--workspace-manifest"])
        .arg(&manifest)
        .args(["--profile", "standard-local-s6", "--format", "yaml"])
        .output()
        .unwrap();
    assert_error(&invalid_format, 2, "input.invalid_format");

    let mut workspace = read_json(&manifest);
    workspace["provider"]["contract_sha256"] = json!("0".repeat(64));
    let materialized = MaterializedWorkspace::with_inputs(&workspace);
    let digest_failure = federate(&materialized.manifest);
    assert_error(&digest_failure, 10, "acquisition.repository_inconsistent");
}

#[test]
fn conf_fr_cli_005_stdout_failure_uses_internal_error_v8() {
    let manifest = fixture_root().join("workspace-provider-only.json");
    let mut child = Command::new(env!("CARGO_BIN_EXE_noesis"))
        .args(["federate", "--workspace-manifest"])
        .arg(&manifest)
        .args(["--profile", "standard-local-s6", "--format", "json"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let output = child.wait_with_output().unwrap();
    assert_error(&output, 70, "internal.unexpected");
}

#[test]
fn sec_fr_fed_001_unsafe_workspace_paths_are_rejected_before_reads() {
    let unsafe_paths = [
        "/absolute/provider".to_owned(),
        "../outside".to_owned(),
        "provider\\alias".to_owned(),
        "provider\0escape".to_owned(),
        "C:/drive-alias".to_owned(),
    ];
    for field in ["root", "contract_path"] {
        for value in &unsafe_paths {
            let mut workspace = read_json(&fixture_root().join("workspace-provider-only.json"));
            workspace["provider"][field] = json!(value);
            let materialized = MaterializedWorkspace::without_inputs(&workspace);
            assert_error(
                &federate(&materialized.manifest),
                2,
                "input.invalid_workspace_manifest",
            );
        }
    }
    for field in ["root", "declaration_path"] {
        for value in &unsafe_paths {
            let mut workspace = read_json(&fixture_root().join("workspace.json"));
            workspace["clients"][0][field] = json!(value);
            let materialized = MaterializedWorkspace::without_inputs(&workspace);
            assert_error(
                &federate(&materialized.manifest),
                2,
                "input.invalid_workspace_manifest",
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn sec_fr_fed_001_symlink_root_is_rejected_before_content_read() {
    use std::os::unix::fs::symlink;

    let workspace = read_json(&fixture_root().join("workspace-provider-only.json"));
    let materialized = MaterializedWorkspace::with_inputs(&workspace);
    let provider = materialized.root.join("provider");
    let outside = materialized.root.join("outside");
    fs::rename(&provider, &outside).unwrap();
    symlink(&outside, &provider).unwrap();
    let output = federate(&materialized.manifest);
    assert_error(&output, 10, "acquisition.path_invalid");

    let manifest_link = materialized.root.join("workspace-link.json");
    symlink(&materialized.manifest, &manifest_link).unwrap();
    assert_error(
        &federate(&manifest_link),
        2,
        "input.invalid_workspace_manifest",
    );
}

#[test]
fn pt_fr_fed_001_fifty_input_orders_are_byte_identical() {
    let mut workspace = read_json(&fixture_root().join("workspace.json"));
    let clients = workspace["clients"].as_array().unwrap().clone();
    let materialized = MaterializedWorkspace::with_inputs(&workspace);
    let expected = fs::read(fixture_root().join("expected-federation-report.json")).unwrap();
    for seed in 0..50_u64 {
        let mut permutation = clients.clone();
        shuffle(&mut permutation, seed);
        workspace["clients"] = Value::Array(permutation);
        materialized.write_manifest(&workspace);
        let output = federate(&materialized.manifest);
        assert!(output.status.success(), "seed {seed}: {output:?}");
        assert_eq!(output.stdout, expected, "seed {seed}");
    }
}

#[test]
fn pt_fr_fed_001_ten_parallel_schedules_are_byte_identical() {
    let workspace = read_json(&fixture_root().join("workspace.json"));
    let materialized = MaterializedWorkspace::with_inputs(&workspace);
    let expected = fs::read(fixture_root().join("expected-federation-report.json")).unwrap();
    let handles = (0..10)
        .map(|_| {
            let manifest = materialized.manifest.clone();
            thread::spawn(move || federate(&manifest))
        })
        .collect::<Vec<_>>();
    for handle in handles {
        let output = handle.join().unwrap();
        assert!(output.status.success(), "{output:?}");
        assert_eq!(output.stdout, expected);
    }
}

#[test]
fn sec_fr_fed_001_standard_path_does_not_mutate_inputs() {
    let workspace = read_json(&fixture_root().join("workspace.json"));
    let materialized = MaterializedWorkspace::with_inputs(&workspace);
    let before = snapshot_tree(&materialized.root);
    let output = federate(&materialized.manifest);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(snapshot_tree(&materialized.root), before);
}

#[test]
fn pt_fr_fed_001_public_limits_accept_max_and_reject_plus_one() {
    assert_workspace_bytes_boundary();
    assert_repository_boundary();
    assert_contract_bytes_boundary();
    assert_yaml_depth_boundary();
    assert_local_reference_boundary();
    assert_path_item_boundary();
    assert_operation_boundary();
    assert_schema_boundary();
    assert_field_boundary();
    assert_report_bytes_boundary();
}

fn assert_workspace_bytes_boundary() {
    let provider = fs::read(fixture_root().join("provider/openapi.json")).unwrap();
    let (materialized, workspace) = materialize_provider(
        &provider,
        "openapi.json",
        read_json(&fixture_root().join("workspace-provider-only.json")),
    );
    let mut manifest = serde_json::to_vec(&workspace).unwrap();
    assert!(manifest.len() < WORKSPACE_BYTES_MAXIMUM);
    manifest.resize(WORKSPACE_BYTES_MAXIMUM, b' ');
    materialized.write_manifest_bytes(&manifest);
    assert_success(&federate(&materialized.manifest));

    manifest.push(b' ');
    materialized.write_manifest_bytes(&manifest);
    assert_limit_error(
        &federate(&materialized.manifest),
        "federation.limit_exceeded",
        "workspace_manifest_bytes",
        u64::try_from(WORKSPACE_BYTES_MAXIMUM).unwrap(),
    );
}

fn assert_repository_boundary() {
    let provider = fs::read(fixture_root().join("provider/openapi.json")).unwrap();
    let mut workspace = read_json(&fixture_root().join("workspace-provider-only.json"));
    workspace["provider"]["contract_path"] = json!("openapi.json");
    workspace["provider"]["contract_sha256"] = json!(sha256_hex(&provider));
    let clients = (0..127).map(boundary_client).collect::<Vec<_>>();
    workspace["clients"] = Value::Array(clients.iter().map(|(input, _)| input.clone()).collect());
    let materialized = MaterializedWorkspace::without_inputs(&workspace);
    materialized.write_relative("provider/openapi.json", &provider);
    for (input, declaration) in &clients {
        materialized.write_relative(
            &format!(
                "{}/{}",
                input["root"].as_str().unwrap(),
                input["declaration_path"].as_str().unwrap()
            ),
            declaration,
        );
    }
    assert_success(&federate(&materialized.manifest));

    let (extra, _) = boundary_client(127);
    workspace["clients"].as_array_mut().unwrap().push(extra);
    materialized.write_manifest(&workspace);
    assert_limit_error(
        &federate(&materialized.manifest),
        "federation.limit_exceeded",
        "repositories",
        128,
    );
}

fn assert_contract_bytes_boundary() {
    let mut provider = fs::read(fixture_root().join("provider/openapi.json")).unwrap();
    assert!(provider.len() < CONTRACT_BYTES_MAXIMUM);
    provider.resize(CONTRACT_BYTES_MAXIMUM, b' ');
    let (materialized, mut workspace) = materialize_provider(
        &provider,
        "openapi.json",
        read_json(&fixture_root().join("workspace-provider-only.json")),
    );
    assert_success(&federate(&materialized.manifest));

    provider.push(b' ');
    workspace["provider"]["contract_sha256"] = json!(sha256_hex(&provider));
    materialized.write_relative("provider/openapi.json", &provider);
    materialized.write_manifest(&workspace);
    assert_limit_error(
        &federate(&materialized.manifest),
        "contract.limit_exceeded",
        "contract_bytes_per_document",
        u64::try_from(CONTRACT_BYTES_MAXIMUM).unwrap(),
    );
}

fn assert_yaml_depth_boundary() {
    let maximum = yaml_with_nested_mappings(30);
    assert_success(&run_generated_provider(&maximum, "openapi.yaml"));
    let maximum_plus_one = yaml_with_nested_mappings(31);
    assert_limit_error(
        &run_generated_provider(&maximum_plus_one, "openapi.yaml"),
        "contract.limit_exceeded",
        "yaml_nesting_depth",
        32,
    );
}

fn assert_local_reference_boundary() {
    assert_success(&run_generated_provider(
        &local_reference_document(16),
        "openapi.json",
    ));
    assert_limit_error(
        &run_generated_provider(&local_reference_document(17), "openapi.json"),
        "contract.limit_exceeded",
        "local_ref_depth",
        16,
    );
}

fn assert_path_item_boundary() {
    assert_success(&run_generated_provider(
        &path_item_document(10_000),
        "openapi.json",
    ));
    assert_limit_error(
        &run_generated_provider(&path_item_document(10_001), "openapi.json"),
        "contract.limit_exceeded",
        "path_items",
        10_000,
    );
}

fn assert_operation_boundary() {
    let maximum = operation_document(10_000);
    assert!(maximum.len() <= CONTRACT_BYTES_MAXIMUM);
    assert_success(&run_generated_provider(&maximum, "openapi.json"));
    let maximum_plus_one = operation_document(10_001);
    assert!(maximum_plus_one.len() <= CONTRACT_BYTES_MAXIMUM);
    assert_limit_error(
        &run_generated_provider(&maximum_plus_one, "openapi.json"),
        "contract.limit_exceeded",
        "operations",
        10_000,
    );
}

fn assert_schema_boundary() {
    assert_success(&run_generated_provider(
        &schema_document(20_000),
        "openapi.json",
    ));
    assert_limit_error(
        &run_generated_provider(&schema_document(20_001), "openapi.json"),
        "contract.limit_exceeded",
        "schemas",
        20_000,
    );
}

fn assert_field_boundary() {
    assert_success(&run_generated_provider(
        &field_document(5_000),
        "openapi.json",
    ));
    assert_limit_error(
        &run_generated_provider(&field_document(5_001), "openapi.json"),
        "contract.limit_exceeded",
        "fields_per_operation",
        5_000,
    );
}

fn assert_report_bytes_boundary() {
    let maximum = report_boundary_document(39);
    let (materialized, _) =
        materialize_provider(&maximum, "openapi.json", report_boundary_workspace());
    let stdout = materialized.root.join("report-max.stdout");
    let output = federate_to_file(&materialized.manifest, &stdout);
    assert_success(&output);
    assert_eq!(fs::metadata(&stdout).unwrap().len(), REPORT_BYTES_MAXIMUM);

    let maximum_plus_one = report_boundary_document(40);
    let mut workspace = report_boundary_workspace();
    workspace["provider"]["contract_sha256"] = json!(sha256_hex(&maximum_plus_one));
    materialized.write_relative("provider/openapi.json", &maximum_plus_one);
    materialized.write_manifest(&workspace);
    let failed_stdout = materialized.root.join("report-max-plus-one.stdout");
    let output = federate_to_file(&materialized.manifest, &failed_stdout);
    assert_eq!(fs::metadata(&failed_stdout).unwrap().len(), 0);
    assert_limit_error(
        &output,
        "federation.limit_exceeded",
        "report_bytes",
        REPORT_BYTES_MAXIMUM,
    );
}

fn assert_success(output: &Output) {
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "successful command wrote stderr");
}

fn assert_limit_error(output: &Output, code: &str, limit: &str, maximum: u64) {
    let error = assert_error(output, 10, code);
    assert_eq!(error["context"]["limit"], limit);
    assert_eq!(error["context"]["maximum"], maximum);
    assert_eq!(error["context"]["observed"], maximum + 1);
}

fn boundary_client(index: usize) -> (Value, Vec<u8>) {
    let role = format!("client{index}");
    let declaration = serde_json::to_vec(&json!({
        "schema_version": "codenoesis.federation-client-declaration/v1",
        "role": role,
        "repository_identity": format!("urn:codenoesis:boundary:client:{index}"),
        "revision": "v1",
        "source_path": format!("src/client{index}.rs"),
        "symbol_identity": format!("call{index}"),
        "binding": {
            "kind": "explicit_operation_identity",
            "service_authority": "https://api.example.invalid",
            "method": "GET",
            "path_template": "/users/{id}",
            "operation_id": "getUser"
        }
    }))
    .unwrap();
    (
        json!({
            "role": role,
            "root": format!("clients/client{index}"),
            "declaration_path": "federation.json",
            "declaration_sha256": sha256_hex(&declaration)
        }),
        declaration,
    )
}

fn yaml_with_nested_mappings(nested_mappings: usize) -> Vec<u8> {
    let mut yaml = fs::read(fixture_root().join("provider/openapi.yaml")).unwrap();
    if !yaml.ends_with(b"\n") {
        yaml.push(b'\n');
    }
    yaml.extend_from_slice(b"x-depth:\n");
    for index in 0..nested_mappings {
        yaml.extend(std::iter::repeat_n(b' ', (index + 1) * 2));
        yaml.extend_from_slice(format!("n{index}:\n").as_bytes());
    }
    yaml.extend(std::iter::repeat_n(b' ', (nested_mappings + 1) * 2));
    yaml.extend_from_slice(b"leaf: value\n");
    yaml
}

fn local_reference_document(reference_depth: usize) -> Vec<u8> {
    let mut document = base_openapi_document();
    document["paths"]["/users/{id}"]["get"]["responses"]["200"]["content"]["application/json"]["schema"]
        ["$ref"] = json!("#/components/schemas/S0");
    let schemas = document["components"]["schemas"].as_object_mut().unwrap();
    schemas.clear();
    for index in 0..reference_depth {
        let value = if index + 1 == reference_depth {
            json!({
                "type": "object",
                "properties": {"id": {"type": "string"}}
            })
        } else {
            json!({"$ref": format!("#/components/schemas/S{}", index + 1)})
        };
        schemas.insert(format!("S{index}"), value);
    }
    serde_json::to_vec(&document).unwrap()
}

fn path_item_document(path_items: usize) -> Vec<u8> {
    let mut document = base_openapi_document();
    let paths = document["paths"].as_object_mut().unwrap();
    for index in 1..path_items {
        paths.insert(format!("/empty/{index}"), json!({}));
    }
    serde_json::to_vec(&document).unwrap()
}

fn operation_document(operations: usize) -> Vec<u8> {
    let mut document = minimal_openapi_document();
    let paths = document["paths"].as_object_mut().unwrap();
    let methods = ["delete", "get", "patch", "post", "put"];
    for index in 0..operations {
        let path = format!("/operation/{}", index / methods.len());
        let path_item = paths.entry(path).or_insert_with(|| json!({}));
        path_item.as_object_mut().unwrap().insert(
            methods[index % methods.len()].to_owned(),
            operation_value(&format!("operation{index}"), "User"),
        );
    }
    serde_json::to_vec(&document).unwrap()
}

fn schema_document(schemas: usize) -> Vec<u8> {
    let mut document = base_openapi_document();
    let values = document["components"]["schemas"].as_object_mut().unwrap();
    for index in 1..schemas {
        values.insert(format!("Schema{index}"), json!({}));
    }
    serde_json::to_vec(&document).unwrap()
}

fn field_document(fields: usize) -> Vec<u8> {
    let mut document = base_openapi_document();
    let mut properties = Map::new();
    for index in 0..fields {
        properties.insert(format!("field{index}"), json!({"type": "string"}));
    }
    document["components"]["schemas"]["User"]["required"] = json!([]);
    document["components"]["schemas"]["User"]["properties"] = Value::Object(properties);
    serde_json::to_vec(&document).unwrap()
}

fn report_boundary_document(first_tune_suffix: usize) -> Vec<u8> {
    let mut document = minimal_openapi_document();
    let paths = document["paths"].as_object_mut().unwrap();
    for index in 0..47 {
        paths.insert(
            format!("/bulk/{index}"),
            json!({"get": operation_value(&format!("bulk{index}"), "Bulk")}),
        );
    }
    paths.insert(
        "/tune".to_owned(),
        json!({"get": operation_value("tune", "Tune")}),
    );

    let schemas = document["components"]["schemas"].as_object_mut().unwrap();
    schemas.clear();
    let mut bulk = Map::new();
    for index in 0..5_000 {
        bulk.insert(format!("f{index:04}"), json!({"type": "string"}));
    }
    let mut tune = Map::new();
    for index in 0..4_470 {
        let mut name = format!("t{index}");
        if index == 0 {
            name.push_str(&"x".repeat(first_tune_suffix));
        }
        tune.insert(name, json!({"type": "string"}));
    }
    schemas.insert(
        "Bulk".to_owned(),
        json!({"type": "object", "properties": bulk}),
    );
    schemas.insert(
        "Tune".to_owned(),
        json!({"type": "object", "properties": tune}),
    );
    serde_json::to_vec(&document).unwrap()
}

fn report_boundary_workspace() -> Value {
    json!({
        "schema_version": "codenoesis.federation-workspace/v1",
        "workspace_identity": "urn:codenoesis:calibration",
        "analysis_profile": "standard-local-s6",
        "contract_capability": "codenoesis.contract-capability/openapi-3.1-http-json/v1",
        "federation_rule_catalog": "codenoesis.federation-rules/http-json/v1",
        "provider": {
            "repository_identity": "urn:codenoesis:calibration:provider",
            "revision": "v1",
            "root": "provider",
            "contract_path": "openapi.json",
            "contract_sha256": "0".repeat(64),
            "service_authority": "https://api.example.invalid"
        },
        "clients": []
    })
}

fn base_openapi_document() -> Value {
    read_json(&fixture_root().join("provider/openapi.json"))
}

fn minimal_openapi_document() -> Value {
    json!({
        "openapi": "3.1.0",
        "info": {"title": "T", "version": "1"},
        "servers": [{"url": "https://api.example.invalid"}],
        "paths": {},
        "components": {
            "schemas": {
                "User": {
                    "type": "object",
                    "properties": {"id": {"type": "string"}}
                }
            }
        }
    })
}

fn operation_value(operation_id: &str, schema: &str) -> Value {
    json!({
        "operationId": operation_id,
        "responses": {
            "200": {
                "content": {
                    "application/json": {
                        "schema": {"$ref": format!("#/components/schemas/{schema}")}
                    }
                }
            }
        }
    })
}

fn run_generated_provider(provider: &[u8], contract_path: &str) -> Output {
    let (materialized, _) = materialize_provider(
        provider,
        contract_path,
        read_json(&fixture_root().join("workspace-provider-only.json")),
    );
    federate(&materialized.manifest)
}

fn run_heuristic_case(provider: &[u8], service_hint: &str, operation_hint: &str) -> Output {
    let mut workspace = read_json(&fixture_root().join("workspace-provider-only.json"));
    let declaration = serde_json::to_vec(&json!({
        "schema_version": "codenoesis.federation-client-declaration/v1",
        "role": "heuristic",
        "repository_identity": "urn:codenoesis:boundary:heuristic-client",
        "revision": "v1",
        "source_path": "src/heuristic.rs",
        "symbol_identity": "callHeuristic",
        "binding": {
            "kind": "heuristic_name",
            "service_hint": service_hint,
            "operation_hint": operation_hint
        }
    }))
    .unwrap();
    workspace["clients"] = json!([{
        "role": "heuristic",
        "root": "clients/heuristic",
        "declaration_path": "federation.json",
        "declaration_sha256": sha256_hex(&declaration)
    }]);
    let (materialized, workspace) = materialize_provider(provider, "openapi.json", workspace);
    materialized.write_relative("clients/heuristic/federation.json", &declaration);
    materialized.write_manifest(&workspace);
    federate(&materialized.manifest)
}

fn materialize_provider(
    provider: &[u8],
    contract_path: &str,
    mut workspace: Value,
) -> (MaterializedWorkspace, Value) {
    workspace["provider"]["root"] = json!("provider");
    workspace["provider"]["contract_path"] = json!(contract_path);
    workspace["provider"]["contract_sha256"] = json!(sha256_hex(provider));
    let materialized = MaterializedWorkspace::without_inputs(&workspace);
    materialized.write_relative(&format!("provider/{contract_path}"), provider);
    (materialized, workspace)
}

fn federate(workspace_manifest: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_noesis"))
        .args(["federate", "--workspace-manifest"])
        .arg(workspace_manifest)
        .args(["--profile", "standard-local-s6", "--format", "json"])
        .output()
        .expect("launch S6 federation subject")
}

fn federate_to_file(workspace_manifest: &Path, stdout: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_noesis"))
        .args(["federate", "--workspace-manifest"])
        .arg(workspace_manifest)
        .args(["--profile", "standard-local-s6", "--format", "json"])
        .stdout(Stdio::from(fs::File::create(stdout).unwrap()))
        .output()
        .expect("launch S6 federation subject with file stdout")
}

fn assert_success_or_expected_red(output: &Output) {
    if output.status.success() {
        return;
    }

    assert_eq!(
        output.status.code(),
        Some(2),
        "pre-S6 subject must fail only at the unrecognized command boundary"
    );
    assert!(output.stdout.is_empty(), "pre-S6 subject wrote stdout");
    assert_eq!(output.stderr.len(), 149, "pre-S6 ErrorV2 length changed");
    let stderr_sha256 = sha256_hex(&output.stderr);
    assert_eq!(
        stderr_sha256, PRE_S6_ERROR_SHA256,
        "pre-S6 ErrorV2 bytes changed"
    );
    assert_eq!(
        output.stderr,
        b"{\"code\":\"input.invalid_revision\",\"context\":{},\"message\":\"invalid revision\",\"retryable\":false,\"schema_version\":\"codenoesis.error/v2\",\"stage\":\"input\"}\n"
    );
    panic!("expected S6 federation success; observed the approved pre-S6 command-boundary Red");
}

fn assert_reviewed_success(workspace: &str, expected: &str) {
    let fixture = fixture_root();
    let output = federate(&fixture.join(workspace));
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, fs::read(fixture.join(expected)).unwrap());
}

fn assert_error(output: &Output, exit_code: i32, code: &str) -> Value {
    assert_eq!(output.status.code(), Some(exit_code));
    assert!(output.stdout.is_empty(), "failed command wrote stdout");
    assert!(output.stderr.ends_with(b"\n"));
    assert!(!output.stderr[..output.stderr.len() - 1].contains(&b'\n'));
    let value: Value = serde_json::from_slice(&output.stderr[..output.stderr.len() - 1]).unwrap();
    assert_eq!(value["schema_version"], "codenoesis.error/v8");
    assert_eq!(value["code"], code);
    value
}

fn provider_variant_workspace(file: &str) -> Value {
    let source = fixture_root().join("variants").join(file);
    json!({
        "schema_version": "codenoesis.federation-workspace/v1",
        "workspace_identity": "urn:codenoesis:fixture:s6-openapi-federation-v1",
        "analysis_profile": "standard-local-s6",
        "contract_capability": "codenoesis.contract-capability/openapi-3.1-http-json/v1",
        "federation_rule_catalog": "codenoesis.federation-rules/http-json/v1",
        "provider": {
            "repository_identity": "urn:codenoesis:fixture:s7-provider",
            "revision": "fixture-provider-a",
            "root": "variants",
            "contract_path": file,
            "contract_sha256": sha256_hex(&fs::read(source).unwrap()),
            "service_authority": "https://api.example.invalid"
        },
        "clients": []
    })
}

struct MaterializedWorkspace {
    root: PathBuf,
    manifest: PathBuf,
}

impl MaterializedWorkspace {
    fn with_inputs(workspace: &Value) -> Self {
        let materialized = Self::without_inputs(workspace);
        for logical_path in workspace_input_paths(workspace) {
            let destination = materialized.root.join(&logical_path);
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::copy(fixture_root().join(&logical_path), destination).unwrap();
        }
        materialized
    }

    fn without_inputs(workspace: &Value) -> Self {
        let root = unique_temp_root();
        let manifest = root.join("workspace.json");
        let materialized = Self { root, manifest };
        materialized.write_manifest(workspace);
        materialized
    }

    fn write_manifest(&self, workspace: &Value) {
        fs::write(&self.manifest, serde_json::to_vec(workspace).unwrap()).unwrap();
    }

    fn write_manifest_bytes(&self, bytes: &[u8]) {
        fs::write(&self.manifest, bytes).unwrap();
    }

    fn write_relative(&self, path: &str, bytes: &[u8]) {
        let destination = self.root.join(path);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::write(destination, bytes).unwrap();
    }
}

impl Drop for MaterializedWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn workspace_input_paths(workspace: &Value) -> Vec<String> {
    let provider = &workspace["provider"];
    let mut paths = vec![format!(
        "{}/{}",
        provider["root"].as_str().unwrap(),
        provider["contract_path"].as_str().unwrap()
    )];
    paths.extend(
        workspace["clients"]
            .as_array()
            .unwrap()
            .iter()
            .map(|client| {
                format!(
                    "{}/{}",
                    client["root"].as_str().unwrap(),
                    client["declaration_path"].as_str().unwrap()
                )
            }),
    );
    paths
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn remove_evidence_references(value: &mut Value) {
    match value {
        Value::Object(object) => {
            object.remove("evidence_ids");
            for child in object.values_mut() {
                remove_evidence_references(child);
            }
        }
        Value::Array(values) => {
            for child in values {
                remove_evidence_references(child);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn shuffle(values: &mut [Value], seed: u64) {
    let mut state = seed.saturating_add(1);
    for index in (1..values.len()).rev() {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let selected = usize::try_from(state % u64::try_from(index + 1).unwrap()).unwrap();
        values.swap(index, selected);
    }
}

fn snapshot_tree(root: &Path) -> BTreeMap<PathBuf, String> {
    fn visit(root: &Path, current: &Path, snapshot: &mut BTreeMap<PathBuf, String>) {
        for entry in fs::read_dir(current).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                visit(root, &path, snapshot);
            } else {
                snapshot.insert(
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    sha256_hex(&fs::read(path).unwrap()),
                );
            }
        }
    }
    let mut snapshot = BTreeMap::new();
    visit(root, root, &mut snapshot);
    snapshot
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hexadecimal = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        let _ = write!(hexadecimal, "{byte:02x}");
    }
    hexadecimal
}

fn unique_temp_root() -> PathBuf {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "codenoesis-s6-{}-{timestamp}-{sequence}",
        std::process::id()
    ));
    fs::create_dir(&root).unwrap();
    root
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/s6/openapi-federation-v1")
}
