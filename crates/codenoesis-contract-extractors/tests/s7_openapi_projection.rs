use codenoesis_contract_extractors::OpenApi31HttpJsonExtractor;
use codenoesis_domain::s6::{OpenApiContractInput, ProviderBinding, SourceFormat};
use codenoesis_ports::S7OpenApiContractProjector;

const CONTRACT: &[u8] = include_bytes!(
    "../../../tests/fixtures/s7/implementation-aware-api-v1/provider/revision-a/openapi.yaml"
);

#[test]
fn conf_fr_imp_004_projects_exact_s6_operation() {
    let operation_id = "urn:codenoesis:operation:blake3:071cbb8fa33a959879d7d8a2bfbbac31e1fea4850c28fdb73227c605f5974923";
    let projection = OpenApi31HttpJsonExtractor::new()
        .project_s7(
            OpenApiContractInput {
                binding: ProviderBinding {
                    repository_identity: "urn:codenoesis:fixture:s7-provider".to_owned(),
                    revision: "fixture-provider-a".to_owned(),
                    contract_path: "openapi.yaml".to_owned(),
                    contract_sha256:
                        "d6decc18d428316b209aa554ee028fe9db8761df515bf34b9e92c3a369f2de3d"
                            .to_owned(),
                    service_authority: "https://api.example.invalid".to_owned(),
                    source_format: SourceFormat::Yaml,
                },
                bytes: CONTRACT,
            },
            operation_id,
        )
        .expect("project S7 operation");

    assert_eq!(projection.operation_id, operation_id);
    assert_eq!(projection.path_template, "/users/{id}");
    assert_eq!(
        (
            projection.evidence_span.start_line,
            projection.evidence_span.end_line
        ),
        (20, 30)
    );
    assert_eq!(projection.fields.len(), 3);
}
