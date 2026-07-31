use codenoesis_application::{FederationRequest, FederationService, FederationServiceError};
use codenoesis_domain::s6::{
    ClientWorkspaceInput, ContractError, FederationEvidence, FederationWorkspace,
    OpenApiContractInput, ProviderBinding, ProviderContract, ProviderWorkspaceInput, SourceFormat,
    service_id,
};
use codenoesis_ports::OpenApiContractExtractor;

#[test]
fn conf_fr_cli_005_application_buffers_one_validated_report() {
    let service = FederationService::new(FakeExtractor::success());
    let output = service
        .federate(FederationRequest::new(workspace(Vec::new()), b"{}", &[]))
        .unwrap();

    assert_eq!(output.last(), Some(&b'\n'));
    assert!(!output[..output.len() - 1].contains(&b'\n'));
    let output = std::str::from_utf8(&output).unwrap();
    assert!(output.contains("\"schema_version\":\"codenoesis.federation-report/v1\""));
    assert!(output.contains("\"clients\":[]"));
    assert!(output.contains("\"confirmed_links\":[]"));
}

#[test]
fn conf_fr_cli_005_application_rejects_unbound_input_cardinality() {
    let clients = vec![ClientWorkspaceInput {
        role: "client".to_owned(),
        root: "client".to_owned(),
        declaration_path: "federation.json".to_owned(),
        declaration_sha256: "1".repeat(64),
    }];
    let error = FederationService::new(FakeExtractor::success())
        .federate(FederationRequest::new(workspace(clients), b"{}", &[]))
        .unwrap_err();
    assert_eq!(error, FederationServiceError::InputMismatch);
}

#[derive(Clone)]
struct FakeExtractor {
    provider: ProviderContract,
}

impl FakeExtractor {
    fn success() -> Self {
        let binding = binding();
        let evidence = FederationEvidence::openapi_json(&binding, "/servers/0/url");
        Self {
            provider: ProviderContract {
                service_id: service_id(&binding.service_authority),
                title: "Application Fixture".to_owned(),
                evidence_ids: vec![evidence.evidence_id.clone()],
                evidence: vec![evidence],
                binding,
                operations: Vec::new(),
                coverage_gaps: Vec::new(),
            },
        }
    }
}

impl OpenApiContractExtractor for FakeExtractor {
    fn extract(&self, _input: OpenApiContractInput<'_>) -> Result<ProviderContract, ContractError> {
        Ok(self.provider.clone())
    }
}

fn workspace(clients: Vec<ClientWorkspaceInput>) -> FederationWorkspace {
    FederationWorkspace {
        workspace_identity: "urn:codenoesis:test:application".to_owned(),
        provider: ProviderWorkspaceInput {
            repository_identity: "urn:codenoesis:test:provider".to_owned(),
            revision: "v1".to_owned(),
            root: "provider".to_owned(),
            contract_path: "openapi.json".to_owned(),
            contract_sha256: "0".repeat(64),
            service_authority: "https://api.example.invalid".to_owned(),
        },
        clients,
    }
}

fn binding() -> ProviderBinding {
    ProviderBinding {
        repository_identity: "urn:codenoesis:test:provider".to_owned(),
        revision: "v1".to_owned(),
        contract_path: "provider/openapi.json".to_owned(),
        contract_sha256: "0".repeat(64),
        service_authority: "https://api.example.invalid".to_owned(),
        source_format: SourceFormat::Json,
    }
}
