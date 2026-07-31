use codenoesis_contracts::{
    FederationReportError, FederationReportV1, S6ContractError, parse_client_declaration,
};
use codenoesis_domain::s6::{
    ClientDeclaration, ContractError, FederationError, FederationWorkspace, OpenApiContractInput,
    ProviderBinding, SourceFormat, federate,
};
use codenoesis_ports::OpenApiContractExtractor;

/// Complete in-memory input for one output-only S6 federation run.
pub struct FederationRequest<'a> {
    workspace: FederationWorkspace,
    provider_contract: &'a [u8],
    client_declarations: &'a [Vec<u8>],
}

impl<'a> FederationRequest<'a> {
    #[must_use]
    pub const fn new(
        workspace: FederationWorkspace,
        provider_contract: &'a [u8],
        client_declarations: &'a [Vec<u8>],
    ) -> Self {
        Self {
            workspace,
            provider_contract,
            client_declarations,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FederationServiceError {
    InputMismatch,
    Contract(ContractError),
    ClientDeclaration { path: String },
    Federation(FederationError),
    Report(FederationReportError),
}

pub struct FederationService<E> {
    extractor: E,
}

impl<E> FederationService<E>
where
    E: OpenApiContractExtractor,
{
    #[must_use]
    pub const fn new(extractor: E) -> Self {
        Self { extractor }
    }

    /// Produces one validated canonical S6 report without filesystem or
    /// persistence authority.
    ///
    /// # Errors
    ///
    /// Returns a typed contract, declaration, federation, or report failure.
    pub fn federate(
        &self,
        request: FederationRequest<'_>,
    ) -> Result<Vec<u8>, FederationServiceError> {
        let FederationRequest {
            workspace,
            provider_contract,
            client_declarations,
        } = request;
        if workspace.clients.len() != client_declarations.len() {
            return Err(FederationServiceError::InputMismatch);
        }

        let provider_path =
            logical_path(&workspace.provider.root, &workspace.provider.contract_path);
        let source_format = source_format(&provider_path).ok_or_else(|| {
            FederationServiceError::Contract(ContractError::UnsupportedCapability {
                path: provider_path.clone(),
            })
        })?;
        let provider = self
            .extractor
            .extract(OpenApiContractInput {
                binding: ProviderBinding {
                    repository_identity: workspace.provider.repository_identity,
                    revision: workspace.provider.revision,
                    contract_path: provider_path,
                    contract_sha256: workspace.provider.contract_sha256,
                    service_authority: workspace.provider.service_authority,
                    source_format,
                },
                bytes: provider_contract,
            })
            .map_err(FederationServiceError::Contract)?;

        let declarations = workspace
            .clients
            .into_iter()
            .zip(client_declarations)
            .map(|(input, bytes)| {
                let path = logical_path(&input.root, &input.declaration_path);
                parse_client_declaration(bytes, &input.role, path.clone(), input.declaration_sha256)
                    .map_err(|error| client_contract_error(&error, path))
            })
            .collect::<Result<Vec<ClientDeclaration>, FederationServiceError>>()?;
        let report = federate(workspace.workspace_identity, provider, declarations)
            .map_err(FederationServiceError::Federation)?;
        FederationReportV1::from_domain(&report)
            .and_then(|value| value.canonical_stdout())
            .map_err(FederationServiceError::Report)
    }
}

fn client_contract_error(error: &S6ContractError, path: String) -> FederationServiceError {
    match error {
        S6ContractError::LimitExceeded(error) => {
            FederationServiceError::Federation(FederationError::LimitExceeded(*error))
        }
        S6ContractError::InvalidWorkspaceManifest
        | S6ContractError::InvalidClientDeclaration
        | S6ContractError::ReportInvalid
        | S6ContractError::Serialization => FederationServiceError::ClientDeclaration { path },
    }
}

fn logical_path(root: &str, path: &str) -> String {
    format!("{root}/{path}")
}

fn source_format(path: &str) -> Option<SourceFormat> {
    match std::path::Path::new(path)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
    {
        Some("json") => Some(SourceFormat::Json),
        Some("yaml") => Some(SourceFormat::Yaml),
        _ => None,
    }
}
