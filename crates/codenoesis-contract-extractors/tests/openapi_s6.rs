use std::fs;
use std::path::{Path, PathBuf};

use codenoesis_contract_extractors::OpenApi31HttpJsonExtractor;
use codenoesis_domain::s6::{
    ContractError, OpenApiContractInput, ProviderBinding, ProviderOperation, SourceFormat,
};
use codenoesis_ports::OpenApiContractExtractor;

const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s7-provider";
const REVISION: &str = "fixture-provider-a";
const AUTHORITY: &str = "https://api.example.invalid";

#[test]
fn gt_fr_ext_004_yaml_json_semantic_projection_is_equivalent() {
    let yaml = extract(
        "provider/openapi.yaml",
        "d6decc18d428316b209aa554ee028fe9db8761df515bf34b9e92c3a369f2de3d",
        SourceFormat::Yaml,
    )
    .expect("extract reviewed YAML contract");
    let json = extract(
        "provider/openapi.json",
        "f0f5f5231acb3ff5ff0cc1f699c38be1c6455bb84f8f0114419f7c4999216acc",
        SourceFormat::Json,
    )
    .expect("extract reviewed JSON contract");
    assert_eq!(yaml.service_id, json.service_id);
    assert_eq!(
        semantic_operations(yaml.operations.clone()),
        semantic_operations(json.operations)
    );
    assert_eq!(yaml.operations.len(), 1);
    assert_eq!(
        yaml.operations[0].primary_evidence_id,
        "urn:codenoesis:evidence:blake3:0eb6cb716e2451f7003c0437b339c93ba0c727bc40add79c7a6f7c99ad8e7990"
    );
}

fn semantic_operations(mut operations: Vec<ProviderOperation>) -> Vec<ProviderOperation> {
    for operation in &mut operations {
        operation.evidence_ids.clear();
        operation.primary_evidence_id.clear();
        for field in &mut operation.fields {
            field.evidence_ids.clear();
        }
    }
    operations
}

#[test]
fn sec_fr_ext_004_restricted_yaml_and_refs_fail_closed() {
    for (file, expected) in [
        ("variants/duplicate-key.yaml", "duplicate"),
        ("variants/alias.yaml", "yaml_feature"),
        ("variants/merge-key.yaml", "yaml_feature"),
        ("variants/custom-tag.yaml", "yaml_feature"),
        ("variants/multiple-documents.yaml", "yaml_feature"),
        ("variants/malformed.yaml", "invalid_yaml"),
        ("variants/remote-ref.yaml", "remote_ref"),
        ("variants/ref-cycle.yaml", "ref_cycle"),
        ("variants/unsupported-openapi.yaml", "openapi_version"),
    ] {
        let bytes = fs::read(fixture_root().join(file)).expect("read hostile S6 fixture");
        let error = OpenApi31HttpJsonExtractor::new()
            .extract(OpenApiContractInput {
                binding: ProviderBinding {
                    repository_identity: REPOSITORY_ID.to_owned(),
                    revision: REVISION.to_owned(),
                    contract_path: file.to_owned(),
                    contract_sha256: "0".repeat(64),
                    service_authority: AUTHORITY.to_owned(),
                    source_format: SourceFormat::Yaml,
                },
                bytes: &bytes,
            })
            .expect_err("hostile S6 input must fail");
        assert!(
            matches!(
                (&error, expected),
                (ContractError::DuplicateKey { .. }, "duplicate")
                    | (ContractError::UnsupportedYamlFeature { .. }, "yaml_feature")
                    | (ContractError::InvalidYaml { .. }, "invalid_yaml")
                    | (ContractError::RemoteReferenceForbidden { .. }, "remote_ref")
                    | (ContractError::ReferenceCycle { .. }, "ref_cycle")
                    | (
                        ContractError::UnsupportedOpenApiVersion { .. },
                        "openapi_version"
                    )
            ),
            "unexpected {file} failure: {error:?}"
        );
    }
}

#[test]
fn gt_fr_ext_004_unsupported_semantics_become_exact_gaps() {
    let contract = extract(
        "variants/unsupported-semantics.yaml",
        "87817e54dc065a407b8d035df1c779e092a92286a534e3cb3a0a72b1572ec1ea",
        SourceFormat::Yaml,
    )
    .expect("extract representable unsupported semantics");
    let reasons = contract
        .coverage_gaps
        .iter()
        .map(|gap| gap.reason_code.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(contract.coverage_gaps.len(), 6);
    assert_eq!(
        reasons,
        std::collections::BTreeSet::from([
            "unsupported_callbacks",
            "unsupported_links",
            "unsupported_media_type",
            "unsupported_security_semantics",
            "unsupported_server_variables",
            "unsupported_webhooks",
        ])
    );
}

#[test]
fn fz_fr_ext_004_malformed_contract_seeds_fail_closed() {
    for (bytes, expected) in [
        (&b"openapi: [\n"[..], "invalid_yaml"),
        (&b"openapi: 3.1.0\ninfo: {title: x\n"[..], "invalid_yaml"),
        (&b"openapi: 3.1.0\n\tinfo: {}\n"[..], "yaml_feature"),
        (&[0xff, 0xfe][..], "invalid_encoding"),
    ] {
        let error = extract_bytes(bytes, "fuzz.yaml", &"0".repeat(64), SourceFormat::Yaml)
            .expect_err("malformed seed must fail");
        assert!(
            matches!(
                (&error, expected),
                (ContractError::InvalidYaml { .. }, "invalid_yaml")
                    | (ContractError::UnsupportedYamlFeature { .. }, "yaml_feature")
                    | (ContractError::InvalidEncoding { .. }, "invalid_encoding")
            ),
            "unexpected malformed-seed failure: {error:?}"
        );
    }
}

fn extract(
    path: &str,
    digest: &str,
    source_format: SourceFormat,
) -> Result<codenoesis_domain::s6::ProviderContract, ContractError> {
    let bytes = fs::read(fixture_root().join(path)).expect("read S6 provider fixture");
    extract_bytes(&bytes, path, digest, source_format)
}

fn extract_bytes(
    bytes: &[u8],
    path: &str,
    digest: &str,
    source_format: SourceFormat,
) -> Result<codenoesis_domain::s6::ProviderContract, ContractError> {
    OpenApi31HttpJsonExtractor::new().extract(OpenApiContractInput {
        binding: ProviderBinding {
            repository_identity: REPOSITORY_ID.to_owned(),
            revision: REVISION.to_owned(),
            contract_path: path.to_owned(),
            contract_sha256: digest.to_owned(),
            service_authority: AUTHORITY.to_owned(),
            source_format,
        },
        bytes,
    })
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/s6/openapi-federation-v1")
}
