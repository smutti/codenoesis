use std::fs;
use std::path::{Path, PathBuf};

use codenoesis_contracts::{
    CodeNoesisErrorV8, S6ContractError, parse_client_declaration, parse_federation_workspace,
};
use codenoesis_domain::s6::ContractError;

#[test]
fn conf_fr_cli_005_workspace_and_client_contracts_are_closed() {
    let root = fixture_root();
    let workspace = parse_federation_workspace(
        &fs::read(root.join("workspace.json")).expect("read S6 workspace"),
    )
    .expect("parse reviewed S6 workspace");
    assert_eq!(workspace.clients.len(), 4);
    let input = &workspace.clients[0];
    let path = format!("{}/{}", input.root, input.declaration_path);
    let declaration = parse_client_declaration(
        &fs::read(root.join(&path)).expect("read S6 client declaration"),
        &input.role,
        path,
        input.declaration_sha256.clone(),
    )
    .expect("parse reviewed client declaration");
    assert_eq!(declaration.role, "candidate");

    let invalid = br#"{"schema_version":"codenoesis.federation-workspace/v1","extra":true}"#;
    assert_eq!(
        parse_federation_workspace(invalid),
        Err(S6ContractError::InvalidWorkspaceManifest)
    );
}

#[test]
fn conf_fr_cli_005_reviewed_error_values_are_exact() {
    let root = fixture_root();
    let expected: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("expected-error-duplicate-key.json"))
            .expect("read duplicate-key error golden"),
    )
    .expect("parse duplicate-key error golden");
    let actual = CodeNoesisErrorV8::from_contract(&ContractError::DuplicateKey {
        path: "variants/duplicate-key.yaml".to_owned(),
    });
    assert_eq!(actual.value(), &expected);
    let bytes = actual.canonical_stderr().expect("canonical ErrorV8");
    assert_eq!(bytes.last(), Some(&b'\n'));
    assert!(!bytes[..bytes.len() - 1].contains(&b'\n'));
}

#[test]
fn sec_fr_cli_005_logical_paths_are_bounded_and_platform_neutral() {
    let mut workspace: serde_json::Value = serde_json::from_slice(
        &fs::read(fixture_root().join("workspace-provider-only.json")).unwrap(),
    )
    .unwrap();
    workspace["provider"]["root"] = serde_json::json!("r");
    workspace["provider"]["contract_path"] = serde_json::json!("p".repeat(4094));
    assert!(parse_federation_workspace(&serde_json::to_vec(&workspace).unwrap()).is_ok());

    workspace["provider"]["contract_path"] = serde_json::json!("p".repeat(4095));
    assert_eq!(
        parse_federation_workspace(&serde_json::to_vec(&workspace).unwrap()),
        Err(S6ContractError::InvalidWorkspaceManifest)
    );
    workspace["provider"]["root"] = serde_json::json!("C:/provider");
    workspace["provider"]["contract_path"] = serde_json::json!("openapi.json");
    assert_eq!(
        parse_federation_workspace(&serde_json::to_vec(&workspace).unwrap()),
        Err(S6ContractError::InvalidWorkspaceManifest)
    );
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/s6/openapi-federation-v1")
}
