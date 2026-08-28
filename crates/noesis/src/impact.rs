use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fs::{self, Metadata};
use std::io::{Read as _, Seek as _};
use std::path::{Component, Path, PathBuf};

use codenoesis_application::ImpactService;
use codenoesis_contract_extractors::OpenApi31HttpJsonExtractor;
use codenoesis_contracts::{
    CodeNoesisErrorV23, IMPACT_PIPELINE, ImpactBoundFile, ImpactWorkspaceError, ImpactWorkspaceV1,
    S7FederationContractError, S7FederationState, SemanticCompatibilityReportV1,
    SemanticReportError, parse_impact_workspace, parse_s7_federation_authority,
};
use codenoesis_domain::s6::{
    OpenApiContractInput, ProviderBinding, SourceFormat, call_site_id, client_id,
};
use codenoesis_domain::s7::{
    CLIENT_CAPABILITY, CONTRACT_CAPABILITY, ClientFact, ContractProjection, EvidenceSourceKind,
    FederationState, ImpactAnalysisError, ImpactAnalysisInput, ProviderFieldFact,
    ProviderRevisionFacts, S7Limit, S7LimitExceeded, SemanticCompatibilityReport, SourceEvidence,
    SourceEvidenceLocator, SourceExtractionError, SourceSpan,
};
use codenoesis_lang_kotlin::TreeSitterKotlinClientExtractor;
use codenoesis_lang_rust::TreeSitterRustProviderExtractor;
use codenoesis_ports::{
    KotlinClientSourceExtractor, RustProviderSourceExtractor, S7OpenApiContractProjector,
};
use same_file::Handle as FileIdentity;
use sha2::{Digest, Sha256};

pub const PIPELINE_VERSION: &str = "codenoesis.pipeline/s7-v1";

const WORKSPACE_BYTES_MAXIMUM: u64 = S7Limit::WorkspaceBytes.maximum();
const FEDERATION_REPORT_BYTES_MAXIMUM: u64 = S7Limit::FederationReportBytes.maximum();
const SOURCE_FILES_MAXIMUM: u64 = S7Limit::SourceFiles.maximum();
const SOURCE_BYTES_PER_FILE_MAXIMUM: u64 = S7Limit::SourceBytesPerFile.maximum();
const TOTAL_SOURCE_BYTES_MAXIMUM: u64 = S7Limit::TotalSourceBytes.maximum();

pub(crate) struct ImpactFailure {
    pub(crate) error: CodeNoesisErrorV23,
    pub(crate) exit_code: u8,
}

pub(crate) struct MaterializedImpactInput {
    pub(crate) provider_repository_identity: String,
    pub(crate) baseline: MaterializedProviderRevision,
    pub(crate) target: MaterializedProviderRevision,
    pub(crate) clients: Vec<MaterializedClientRevision>,
    pub(crate) federation_report_path: String,
    pub(crate) federation_report: Vec<u8>,
}

pub(crate) struct MaterializedProviderRevision {
    pub(crate) revision: String,
    pub(crate) federation_revision: String,
    pub(crate) contract_path: String,
    pub(crate) contract_sha256: String,
    pub(crate) contract_bytes: Vec<u8>,
    pub(crate) source_path: String,
    pub(crate) source_sha256: String,
    pub(crate) source_bytes: Vec<u8>,
    pub(crate) callable_symbol: String,
}

pub(crate) struct MaterializedClientRevision {
    pub(crate) role: String,
    pub(crate) repository_identity: String,
    pub(crate) revision: String,
    pub(crate) federation_revision: String,
    pub(crate) source_path: String,
    pub(crate) source_bytes: Vec<u8>,
    pub(crate) decoder_symbol: String,
    pub(crate) call_symbol: String,
}

pub(crate) fn requested(arguments: &[OsString]) -> bool {
    arguments.get(1).is_some_and(|value| value == "impact")
}

#[allow(clippy::too_many_lines)]
pub(crate) fn run(arguments: Vec<OsString>) -> Result<Vec<u8>, ImpactFailure> {
    if PIPELINE_VERSION != IMPACT_PIPELINE {
        return Err(internal_failure());
    }
    let invocation = ImpactInvocation::parse(arguments)?;
    let manifest_path = canonical_manifest(&invocation.workspace)?;
    let manifest_bytes = read_stable(&manifest_path, WORKSPACE_BYTES_MAXIMUM, "workspace")?;
    enforce_limit(
        "workspace_bytes",
        WORKSPACE_BYTES_MAXIMUM,
        manifest_bytes.len(),
    )?;
    let workspace = parse_impact_workspace(&manifest_bytes).map_err(workspace_failure)?;
    let sources = ImpactSources::resolve(&manifest_path, workspace.clone())?;
    noesis::install_s6_filesystem_boundary(
        sources.federation_report.path.as_os_str(),
        &sources.allowed_read_roots,
    )
    .map_err(|_| internal_failure())?;
    let inputs = sources.read_verified_inputs()?;
    let authority = parse_s7_federation_authority(&inputs.federation_report)
        .map_err(|error| federation_failure(&sources.federation_report, error))?;
    if authority.provider_repository_identity != workspace.provider.repository_identity
        || authority.provider_revision != workspace.provider.baseline.revision
    {
        return Err(operation_failure(
            CodeNoesisErrorV23::invalid_federation_report(
                &sources.federation_report.logical_path,
                "authority_mismatch",
            ),
        ));
    }
    let operation = authority
        .operations
        .first()
        .filter(|_| authority.operations.len() == 1)
        .ok_or_else(|| {
            operation_failure(CodeNoesisErrorV23::invalid_federation_report(
                &sources.federation_report.logical_path,
                "unsupported_operation_set",
            ))
        })?;
    let contract_extractor = OpenApi31HttpJsonExtractor::new();
    let baseline_contract = project_contract(
        contract_extractor,
        &workspace.provider.repository_identity,
        &workspace.provider.baseline.revision,
        &workspace.provider.baseline.contract,
        &authority.service_authority,
        &operation.operation_id,
        &inputs.provider_contracts[0],
    )?;
    let target_contract = project_contract(
        contract_extractor,
        &workspace.provider.repository_identity,
        &workspace.provider.target.revision,
        &workspace.provider.target.contract,
        &authority.service_authority,
        &operation.operation_id,
        &inputs.provider_contracts[1],
    )?;
    validate_contract_authority(
        operation,
        &baseline_contract,
        &sources.federation_report.logical_path,
    )?;
    validate_contract_authority(
        operation,
        &target_contract,
        &sources.federation_report.logical_path,
    )?;

    let provider_extractor = TreeSitterRustProviderExtractor::new();
    let baseline_provider = provider_extractor
        .extract_s7_provider(
            &inputs.provider_sources[0],
            &workspace.provider.baseline.callable_symbol,
        )
        .map_err(|error| source_failure(&error))?;
    let target_provider = provider_extractor
        .extract_s7_provider(
            &inputs.provider_sources[1],
            &workspace.provider.target.callable_symbol,
        )
        .map_err(|error| source_failure(&error))?;
    let mut evidence = Vec::new();
    let baseline = provider_facts(
        &workspace.provider.repository_identity,
        &workspace.provider.baseline,
        baseline_contract,
        &inputs.provider_contracts[0],
        &inputs.provider_sources[0],
        baseline_provider,
        &mut evidence,
    )?;
    let target = provider_facts(
        &workspace.provider.repository_identity,
        &workspace.provider.target,
        target_contract,
        &inputs.provider_contracts[1],
        &inputs.provider_sources[1],
        target_provider,
        &mut evidence,
    )?;

    let client_extractor = TreeSitterKotlinClientExtractor::new();
    let authority_by_repository = authority
        .clients
        .iter()
        .map(|client| (client.repository_identity.as_str(), client))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut clients = Vec::with_capacity(workspace.clients.len());
    for (index, client) in workspace.clients.iter().enumerate() {
        let federated = authority_by_repository
            .get(client.repository_identity.as_str())
            .ok_or_else(|| {
                operation_failure(CodeNoesisErrorV23::invalid_federation_report(
                    &sources.federation_report.logical_path,
                    "client_authority_missing",
                ))
            })?;
        if federated.role != client.role || federated.revision != client.revision {
            return Err(operation_failure(
                CodeNoesisErrorV23::invalid_federation_report(
                    &sources.federation_report.logical_path,
                    "client_authority_mismatch",
                ),
            ));
        }
        let extraction = client_extractor
            .extract_s7_client(
                &inputs.client_sources[index],
                &client.decoder_symbol,
                &client.call_symbol,
            )
            .map_err(|error| source_failure(&error))?;
        let source_evidence = evidence_for_span(
            &client.repository_identity,
            &client.revision,
            &client.source.path,
            &inputs.client_sources[index],
            extraction.evidence_span,
            EvidenceSourceKind::ClientAssumption,
            CLIENT_CAPABILITY,
        )?;
        let calculated_client_id = client_id(&client.repository_identity);
        let calculated_call_site_id = call_site_id(
            &calculated_client_id,
            &client.revision,
            &client.source.path,
            &client.call_symbol,
        );
        if calculated_client_id != federated.client_id
            || calculated_call_site_id != federated.call_site_id
        {
            return Err(operation_failure(
                CodeNoesisErrorV23::invalid_federation_report(
                    &sources.federation_report.logical_path,
                    "client_identity_mismatch",
                ),
            ));
        }
        let federation = match &federated.state {
            S7FederationState::Confirmed { operation_id } => FederationState::Confirmed {
                operation_id: operation_id.clone(),
            },
            S7FederationState::Rejected {
                operation_candidate_id,
            } => FederationState::Rejected {
                operation_candidate_id: operation_candidate_id.clone(),
            },
            S7FederationState::Unresolved => {
                return Err(operation_failure(
                    CodeNoesisErrorV23::invalid_federation_report(
                        &sources.federation_report.logical_path,
                        "unresolved_client_authority",
                    ),
                ));
            }
        };
        clients.push(ClientFact {
            repository_identity: client.repository_identity.clone(),
            revision: client.revision.clone(),
            client_id: calculated_client_id,
            call_site_id: calculated_call_site_id,
            call_symbol: client.call_symbol.clone(),
            path_template: extraction.path_template,
            assumptions: extraction
                .assumptions
                .into_iter()
                .map(|fact| (format!("/{}", fact.field_name), fact.assumption))
                .collect(),
            evidence_id: source_evidence.id.clone(),
            federation,
        });
        evidence.push(source_evidence);
    }

    evidence.sort_by(|left, right| left.id.cmp(&right.id));
    let report = ImpactService::analyze(ImpactAnalysisInput {
        provider_repository_identity: workspace.provider.repository_identity,
        baseline,
        target,
        clients,
        evidence,
    })
    .map_err(|error| analysis_failure(&error))?;
    SemanticCompatibilityReportV1::from_domain(&report)
        .and_then(|report| report.canonical_stdout())
        .map_err(report_failure)
}

#[allow(clippy::too_many_lines)]
pub(crate) fn analyze_materialized(
    input: MaterializedImpactInput,
) -> Result<SemanticCompatibilityReport, ImpactFailure> {
    let authority = parse_s7_federation_authority(&input.federation_report)
        .map_err(|error| federation_bytes_failure(&input.federation_report_path, error))?;
    if authority.provider_repository_identity != input.provider_repository_identity
        || authority.provider_revision != input.baseline.federation_revision
    {
        return Err(operation_failure(
            CodeNoesisErrorV23::invalid_federation_report(
                &input.federation_report_path,
                "authority_mismatch",
            ),
        ));
    }
    let operation = authority
        .operations
        .first()
        .filter(|_| authority.operations.len() == 1)
        .ok_or_else(|| {
            operation_failure(CodeNoesisErrorV23::invalid_federation_report(
                &input.federation_report_path,
                "unsupported_operation_set",
            ))
        })?;
    let baseline_revision = materialized_revision_contract(&input.baseline);
    let target_revision = materialized_revision_contract(&input.target);
    let baseline_contract = project_contract(
        OpenApi31HttpJsonExtractor::new(),
        &input.provider_repository_identity,
        &input.baseline.revision,
        &baseline_revision.contract,
        &authority.service_authority,
        &operation.operation_id,
        &input.baseline.contract_bytes,
    )?;
    let target_contract = project_contract(
        OpenApi31HttpJsonExtractor::new(),
        &input.provider_repository_identity,
        &input.target.revision,
        &target_revision.contract,
        &authority.service_authority,
        &operation.operation_id,
        &input.target.contract_bytes,
    )?;
    validate_contract_authority(operation, &baseline_contract, &input.federation_report_path)?;
    validate_contract_authority(operation, &target_contract, &input.federation_report_path)?;

    let provider_extractor = TreeSitterRustProviderExtractor::new();
    let baseline_provider = provider_extractor
        .extract_s7_provider(
            &input.baseline.source_bytes,
            &input.baseline.callable_symbol,
        )
        .map_err(|error| source_failure(&error))?;
    let target_provider = provider_extractor
        .extract_s7_provider(&input.target.source_bytes, &input.target.callable_symbol)
        .map_err(|error| source_failure(&error))?;
    let mut evidence = Vec::new();
    let baseline = provider_facts(
        &input.provider_repository_identity,
        &baseline_revision,
        baseline_contract,
        &input.baseline.contract_bytes,
        &input.baseline.source_bytes,
        baseline_provider,
        &mut evidence,
    )?;
    let target = provider_facts(
        &input.provider_repository_identity,
        &target_revision,
        target_contract,
        &input.target.contract_bytes,
        &input.target.source_bytes,
        target_provider,
        &mut evidence,
    )?;

    let client_extractor = TreeSitterKotlinClientExtractor::new();
    let authority_by_repository = authority
        .clients
        .iter()
        .map(|client| (client.repository_identity.as_str(), client))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut clients = Vec::with_capacity(input.clients.len());
    for client in &input.clients {
        let federated = authority_by_repository
            .get(client.repository_identity.as_str())
            .ok_or_else(|| {
                operation_failure(CodeNoesisErrorV23::invalid_federation_report(
                    &input.federation_report_path,
                    "client_authority_missing",
                ))
            })?;
        if federated.role != client.role || federated.revision != client.federation_revision {
            return Err(operation_failure(
                CodeNoesisErrorV23::invalid_federation_report(
                    &input.federation_report_path,
                    "client_authority_mismatch",
                ),
            ));
        }
        let extraction = client_extractor
            .extract_s7_client(
                &client.source_bytes,
                &client.decoder_symbol,
                &client.call_symbol,
            )
            .map_err(|error| source_failure(&error))?;
        let source_evidence = evidence_for_span(
            &client.repository_identity,
            &client.revision,
            &client.source_path,
            &client.source_bytes,
            extraction.evidence_span,
            EvidenceSourceKind::ClientAssumption,
            CLIENT_CAPABILITY,
        )?;
        let calculated_client_id = client_id(&client.repository_identity);
        let federation_call_site_id = call_site_id(
            &calculated_client_id,
            &client.federation_revision,
            &client.source_path,
            &client.call_symbol,
        );
        if calculated_client_id != federated.client_id
            || federation_call_site_id != federated.call_site_id
        {
            return Err(operation_failure(
                CodeNoesisErrorV23::invalid_federation_report(
                    &input.federation_report_path,
                    "client_identity_mismatch",
                ),
            ));
        }
        let git_call_site_id = call_site_id(
            &calculated_client_id,
            &client.revision,
            &client.source_path,
            &client.call_symbol,
        );
        let federation = match &federated.state {
            S7FederationState::Confirmed { operation_id } => FederationState::Confirmed {
                operation_id: operation_id.clone(),
            },
            S7FederationState::Rejected {
                operation_candidate_id,
            } => FederationState::Rejected {
                operation_candidate_id: operation_candidate_id.clone(),
            },
            S7FederationState::Unresolved => {
                return Err(operation_failure(
                    CodeNoesisErrorV23::invalid_federation_report(
                        &input.federation_report_path,
                        "unresolved_client_authority",
                    ),
                ));
            }
        };
        clients.push(ClientFact {
            repository_identity: client.repository_identity.clone(),
            revision: client.revision.clone(),
            client_id: calculated_client_id,
            call_site_id: git_call_site_id,
            call_symbol: client.call_symbol.clone(),
            path_template: extraction.path_template,
            assumptions: extraction
                .assumptions
                .into_iter()
                .map(|fact| (format!("/{}", fact.field_name), fact.assumption))
                .collect(),
            evidence_id: source_evidence.id.clone(),
            federation,
        });
        evidence.push(source_evidence);
    }

    evidence.sort_by(|left, right| left.id.cmp(&right.id));
    ImpactService::analyze(ImpactAnalysisInput {
        provider_repository_identity: input.provider_repository_identity,
        baseline,
        target,
        clients,
        evidence,
    })
    .map_err(|error| analysis_failure(&error))
}

fn materialized_revision_contract(
    revision: &MaterializedProviderRevision,
) -> codenoesis_contracts::ImpactRevisionInput {
    codenoesis_contracts::ImpactRevisionInput {
        revision: revision.revision.clone(),
        root: String::new(),
        contract: ImpactBoundFile {
            path: revision.contract_path.clone(),
            sha256: revision.contract_sha256.clone(),
        },
        source: ImpactBoundFile {
            path: revision.source_path.clone(),
            sha256: revision.source_sha256.clone(),
        },
        callable_symbol: revision.callable_symbol.clone(),
    }
}

struct ImpactInvocation {
    workspace: PathBuf,
}

impl ImpactInvocation {
    fn parse(arguments: Vec<OsString>) -> Result<Self, ImpactFailure> {
        let mut arguments = arguments.into_iter();
        let _binary = arguments.next();
        if arguments.next().as_deref() != Some(OsStr::new("impact")) {
            return Err(input_failure(CodeNoesisErrorV23::invalid_workspace(
                "invalid_invocation",
            )));
        }

        let mut workspace = None;
        let mut profile = None;
        let mut format = None;
        while let Some(flag) = arguments.next() {
            let Some(value) = arguments.next() else {
                return Err(input_failure(CodeNoesisErrorV23::invalid_workspace(
                    "missing_argument_value",
                )));
            };
            match flag.to_str() {
                Some("--workspace") if workspace.is_none() => {
                    workspace = Some(PathBuf::from(value));
                }
                Some("--profile") if profile.is_none() => profile = value.into_string().ok(),
                Some("--format") if format.is_none() => format = value.into_string().ok(),
                _ => {
                    return Err(input_failure(CodeNoesisErrorV23::invalid_workspace(
                        "invalid_invocation",
                    )));
                }
            }
        }
        if profile.as_deref() != Some("implementation-aware-http-json-v1") {
            return Err(input_failure(CodeNoesisErrorV23::invalid_workspace(
                "invalid_profile",
            )));
        }
        if format.as_deref() != Some("json") {
            return Err(input_failure(CodeNoesisErrorV23::invalid_workspace(
                "invalid_format",
            )));
        }
        let workspace = workspace
            .filter(|path| !path.as_os_str().is_empty())
            .ok_or_else(|| {
                input_failure(CodeNoesisErrorV23::invalid_workspace("missing_workspace"))
            })?;
        Ok(Self { workspace })
    }
}

struct ImpactSources {
    allowed_read_roots: Vec<PathBuf>,
    provider_contracts: Vec<BoundInput>,
    provider_sources: Vec<BoundInput>,
    client_sources: Vec<BoundInput>,
    federation_report: BoundInput,
}

impl ImpactSources {
    fn resolve(
        workspace_manifest: &Path,
        workspace: ImpactWorkspaceV1,
    ) -> Result<Self, ImpactFailure> {
        let base = workspace_manifest.parent().ok_or_else(|| {
            input_failure(CodeNoesisErrorV23::invalid_workspace("manifest_parent"))
        })?;
        let baseline_root = resolve_root(base, &workspace.provider.baseline.root)?;
        let target_root = resolve_root(base, &workspace.provider.target.root)?;
        let provider_contracts = vec![
            BoundInput::resolve(
                &baseline_root,
                &workspace.provider.baseline.contract,
                "provider_contract",
                SOURCE_BYTES_PER_FILE_MAXIMUM,
            )?,
            BoundInput::resolve(
                &target_root,
                &workspace.provider.target.contract,
                "provider_contract",
                SOURCE_BYTES_PER_FILE_MAXIMUM,
            )?,
        ];
        let provider_sources = vec![
            BoundInput::resolve(
                &baseline_root,
                &workspace.provider.baseline.source,
                "provider_source",
                SOURCE_BYTES_PER_FILE_MAXIMUM,
            )?,
            BoundInput::resolve(
                &target_root,
                &workspace.provider.target.source,
                "provider_source",
                SOURCE_BYTES_PER_FILE_MAXIMUM,
            )?,
        ];

        let mut allowed_read_roots = BTreeSet::from([baseline_root, target_root]);
        let mut client_sources = Vec::with_capacity(workspace.clients.len());
        for client in workspace.clients {
            let root = resolve_root(base, &client.root)?;
            client_sources.push(BoundInput::resolve(
                &root,
                &client.source,
                "client_source",
                SOURCE_BYTES_PER_FILE_MAXIMUM,
            )?);
            allowed_read_roots.insert(root);
        }
        let source_count = provider_sources.len().saturating_add(client_sources.len());
        enforce_limit("source_files", SOURCE_FILES_MAXIMUM, source_count)?;
        let source_bytes = provider_sources
            .iter()
            .chain(&client_sources)
            .fold(0_u64, |total, source| {
                total.saturating_add(source.byte_length)
            });
        enforce_limit_u64(
            "total_source_bytes",
            TOTAL_SOURCE_BYTES_MAXIMUM,
            source_bytes,
        )?;

        let federation_report = BoundInput::resolve(
            base,
            &workspace.federation_report,
            "federation_report",
            FEDERATION_REPORT_BYTES_MAXIMUM,
        )?;
        Ok(Self {
            allowed_read_roots: allowed_read_roots.into_iter().collect(),
            provider_contracts,
            provider_sources,
            client_sources,
            federation_report,
        })
    }

    fn read_verified_inputs(&self) -> Result<VerifiedImpactInputs, ImpactFailure> {
        Ok(VerifiedImpactInputs {
            provider_contracts: self
                .provider_contracts
                .iter()
                .map(BoundInput::read_and_verify)
                .collect::<Result<_, _>>()?,
            provider_sources: self
                .provider_sources
                .iter()
                .map(BoundInput::read_and_verify)
                .collect::<Result<_, _>>()?,
            client_sources: self
                .client_sources
                .iter()
                .map(BoundInput::read_and_verify)
                .collect::<Result<_, _>>()?,
            federation_report: self.federation_report.read_and_verify()?,
        })
    }
}

struct VerifiedImpactInputs {
    provider_contracts: Vec<Vec<u8>>,
    provider_sources: Vec<Vec<u8>>,
    client_sources: Vec<Vec<u8>>,
    federation_report: Vec<u8>,
}

struct BoundInput {
    path: PathBuf,
    logical_path: String,
    expected_sha256: String,
    component: &'static str,
    byte_length: u64,
    maximum: u64,
}

impl BoundInput {
    fn resolve(
        root: &Path,
        input: &ImpactBoundFile,
        component: &'static str,
        maximum: u64,
    ) -> Result<Self, ImpactFailure> {
        let path = resolve_file(root, &input.path, component)?;
        let byte_length = fs::metadata(&path)
            .map_err(|_| path_failure(&input.path, component, "metadata_unavailable"))?
            .len();
        enforce_component_limit(component, maximum, byte_length)?;
        Ok(Self {
            path,
            logical_path: input.path.clone(),
            expected_sha256: input.sha256.clone(),
            component,
            byte_length,
            maximum,
        })
    }

    fn read_and_verify(&self) -> Result<Vec<u8>, ImpactFailure> {
        let bytes = read_stable(&self.path, self.maximum, self.component)?;
        enforce_component_limit(
            self.component,
            self.maximum,
            u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        )?;
        let observed = sha256(&bytes);
        if observed != self.expected_sha256 {
            let error = if self.component == "federation_report" {
                CodeNoesisErrorV23::invalid_federation_report(&self.logical_path, "digest_mismatch")
            } else {
                CodeNoesisErrorV23::invalid_workspace_path(
                    &self.logical_path,
                    self.component,
                    "digest_mismatch",
                )
            };
            return Err(operation_failure(error));
        }
        Ok(bytes)
    }
}

fn canonical_manifest(path: &Path) -> Result<PathBuf, ImpactFailure> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| input_failure(CodeNoesisErrorV23::invalid_workspace("manifest_missing")))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(input_failure(CodeNoesisErrorV23::invalid_workspace(
            "manifest_type",
        )));
    }
    fs::canonicalize(path)
        .map_err(|_| input_failure(CodeNoesisErrorV23::invalid_workspace("manifest_path")))
}

fn resolve_root(base: &Path, relative: &str) -> Result<PathBuf, ImpactFailure> {
    let path = resolve_components(base, relative, "workspace")?;
    let metadata = fs::symlink_metadata(&path)
        .map_err(|_| path_failure(relative, "workspace", "root_missing"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(path_failure(relative, "workspace", "root_type"));
    }
    let canonical =
        fs::canonicalize(&path).map_err(|_| path_failure(relative, "workspace", "root_path"))?;
    if !canonical.starts_with(base) {
        return Err(path_failure(relative, "workspace", "root_escape"));
    }
    Ok(canonical)
}

fn resolve_file(
    root: &Path,
    relative: &str,
    component: &'static str,
) -> Result<PathBuf, ImpactFailure> {
    let path = resolve_components(root, relative, component)?;
    let metadata = fs::symlink_metadata(&path)
        .map_err(|_| path_failure(relative, component, "file_missing"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(path_failure(relative, component, "file_type"));
    }
    let canonical =
        fs::canonicalize(&path).map_err(|_| path_failure(relative, component, "file_path"))?;
    if !canonical.starts_with(root) {
        return Err(path_failure(relative, component, "file_escape"));
    }
    Ok(canonical)
}

fn resolve_components(
    base: &Path,
    relative: &str,
    component: &'static str,
) -> Result<PathBuf, ImpactFailure> {
    let mut resolved = base.to_path_buf();
    for path_component in Path::new(relative).components() {
        let Component::Normal(path_component) = path_component else {
            return Err(path_failure(relative, component, "unsafe_path"));
        };
        resolved.push(path_component);
        let metadata = fs::symlink_metadata(&resolved)
            .map_err(|_| path_failure(relative, component, "path_missing"))?;
        if metadata.file_type().is_symlink() {
            return Err(path_failure(relative, component, "symlink"));
        }
    }
    Ok(resolved)
}

fn read_stable(
    path: &Path,
    maximum: u64,
    component: &'static str,
) -> Result<Vec<u8>, ImpactFailure> {
    read_stable_with(path, maximum, component, || {})
}

fn read_stable_with(
    path: &Path,
    maximum: u64,
    component: &'static str,
    after_first_read: impl FnOnce(),
) -> Result<Vec<u8>, ImpactFailure> {
    let path_identity_before = FileIdentity::from_path(path)
        .map_err(|_| path_failure(&bounded_path(path), component, "metadata_unavailable"))?;
    let path_before = fs::symlink_metadata(path)
        .map_err(|_| path_failure(&bounded_path(path), component, "metadata_unavailable"))?;
    if !path_before.is_file() || path_before.file_type().is_symlink() {
        return Err(path_failure(&bounded_path(path), component, "file_type"));
    }
    let mut file = fs::File::open(path)
        .map_err(|_| path_failure(&bounded_path(path), component, "read_failed"))?;
    let before = file
        .metadata()
        .map_err(|_| path_failure(&bounded_path(path), component, "metadata_unavailable"))?;
    let identity = FileIdentity::from_file(
        file.try_clone()
            .map_err(|_| path_failure(&bounded_path(path), component, "metadata_unavailable"))?,
    )
    .map_err(|_| path_failure(&bounded_path(path), component, "metadata_unavailable"))?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| path_failure(&bounded_path(path), component, "read_failed"))?;
    let observed = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    let limit = if component == "workspace" {
        "workspace_bytes"
    } else if component == "federation_report" {
        "federation_report_bytes"
    } else {
        "source_bytes_per_file"
    };
    enforce_limit_u64(limit, maximum, observed)?;
    after_first_read();
    file.rewind()
        .map_err(|_| operation_failure(CodeNoesisErrorV23::mutable_input(component)))?;
    let mut verification = Vec::new();
    file.by_ref()
        .take(maximum.saturating_add(1))
        .read_to_end(&mut verification)
        .map_err(|_| operation_failure(CodeNoesisErrorV23::mutable_input(component)))?;
    let after = file
        .metadata()
        .map_err(|_| operation_failure(CodeNoesisErrorV23::mutable_input(component)))?;
    let path_after = fs::symlink_metadata(path)
        .map_err(|_| operation_failure(CodeNoesisErrorV23::mutable_input(component)))?;
    let path_identity_matches =
        FileIdentity::from_path(path).is_ok_and(|path_identity| path_identity == identity);
    if bytes != verification
        || path_identity_before != identity
        || before.len() != observed
        || !same_file_metadata(&path_before, &before)
        || !same_file_metadata(&before, &after)
        || !same_file_metadata(&after, &path_after)
        || !path_identity_matches
        || path_after.file_type().is_symlink()
    {
        return Err(operation_failure(CodeNoesisErrorV23::mutable_input(
            component,
        )));
    }
    Ok(bytes)
}

#[cfg(unix)]
fn same_file_metadata(left: &Metadata, right: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
}

#[cfg(windows)]
fn same_file_metadata(left: &Metadata, right: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    left.file_size() == right.file_size()
        && left.last_write_time() == right.last_write_time()
        && left.creation_time() == right.creation_time()
        && left.file_attributes() == right.file_attributes()
}

#[cfg(not(any(unix, windows)))]
fn same_file_metadata(left: &Metadata, right: &Metadata) -> bool {
    left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
        && left.file_type() == right.file_type()
}

fn bounded_path(path: &Path) -> String {
    path.file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("input")
        .chars()
        .take(128)
        .collect()
}

fn enforce_limit(limit: &'static str, maximum: u64, observed: usize) -> Result<(), ImpactFailure> {
    enforce_limit_u64(limit, maximum, u64::try_from(observed).unwrap_or(u64::MAX))
}

fn enforce_limit_u64(
    limit: &'static str,
    maximum: u64,
    observed: u64,
) -> Result<(), ImpactFailure> {
    if observed > maximum {
        Err(operation_failure(CodeNoesisErrorV23::limit(
            limit, maximum, observed,
        )))
    } else {
        Ok(())
    }
}

fn enforce_component_limit(
    component: &str,
    maximum: u64,
    observed: u64,
) -> Result<(), ImpactFailure> {
    let limit = if component == "federation_report" {
        "federation_report_bytes"
    } else {
        "source_bytes_per_file"
    };
    enforce_limit_u64(limit, maximum, observed)
}

fn sha256(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(64);
    for byte in digest {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn project_contract(
    extractor: OpenApi31HttpJsonExtractor,
    repository_identity: &str,
    revision: &str,
    binding: &ImpactBoundFile,
    service_authority: &str,
    operation_id: &str,
    bytes: &[u8],
) -> Result<ContractProjection, ImpactFailure> {
    extractor
        .project_s7(
            OpenApiContractInput {
                binding: ProviderBinding {
                    repository_identity: repository_identity.to_owned(),
                    revision: revision.to_owned(),
                    contract_path: binding.path.clone(),
                    contract_sha256: binding.sha256.clone(),
                    service_authority: service_authority.to_owned(),
                    source_format: SourceFormat::Yaml,
                },
                bytes,
            },
            operation_id,
        )
        .map_err(|_| operation_failure(CodeNoesisErrorV23::unsupported_implementation_semantics()))
}

fn validate_contract_authority(
    authority: &codenoesis_contracts::S7FederatedOperation,
    contract: &ContractProjection,
    federation_path: &str,
) -> Result<(), ImpactFailure> {
    if authority.operation_id != contract.operation_id
        || authority.method != contract.method
        || authority.path_template != contract.path_template
        || authority.explicit_operation_id != contract.explicit_operation_id
        || authority.response_status != contract.response_status
        || authority.fields
            != contract
                .fields
                .iter()
                .map(|field| {
                    (
                        field.field_id.clone(),
                        field.json_pointer.clone(),
                        field.required,
                    )
                })
                .collect::<Vec<_>>()
    {
        return Err(operation_failure(
            CodeNoesisErrorV23::invalid_federation_report(
                federation_path,
                "contract_authority_mismatch",
            ),
        ));
    }
    Ok(())
}

fn provider_facts(
    repository_identity: &str,
    revision: &codenoesis_contracts::ImpactRevisionInput,
    contract: ContractProjection,
    contract_bytes: &[u8],
    source_bytes: &[u8],
    extraction: codenoesis_domain::s7::ProviderSourceExtraction,
    evidence: &mut Vec<SourceEvidence>,
) -> Result<ProviderRevisionFacts, ImpactFailure> {
    let contract_evidence = evidence_for_lines(
        EvidenceLines {
            repository_identity,
            revision: &revision.revision,
            path: &revision.contract.path,
            source: contract_bytes,
            start_line: contract.evidence_span.start_line,
            end_line: contract.evidence_span.end_line,
        },
        EvidenceSourceKind::DeclaredContract,
        CONTRACT_CAPABILITY,
    )?;
    let mut fields = Vec::with_capacity(extraction.fields.len());
    for field in extraction.fields {
        let source_evidence = evidence_for_span(
            repository_identity,
            &revision.revision,
            &revision.source.path,
            source_bytes,
            field.span,
            EvidenceSourceKind::ProviderImplementation,
            codenoesis_domain::s7::PROVIDER_CAPABILITY,
        )?;
        fields.push(ProviderFieldFact {
            field_pointer: format!("/{}", field.field_name),
            presence: field.presence,
            evidence_id: source_evidence.id.clone(),
        });
        evidence.push(source_evidence);
    }
    fields.sort_by(|left, right| left.field_pointer.cmp(&right.field_pointer));
    let mut custom_mapping_evidence_ids = Vec::new();
    for span in extraction.custom_mapping_spans {
        let source_evidence = evidence_for_span(
            repository_identity,
            &revision.revision,
            &revision.source.path,
            source_bytes,
            span,
            EvidenceSourceKind::ProviderImplementation,
            codenoesis_domain::s7::PROVIDER_CAPABILITY,
        )?;
        custom_mapping_evidence_ids.push(source_evidence.id.clone());
        evidence.push(source_evidence);
    }
    custom_mapping_evidence_ids.sort();
    evidence.push(contract_evidence.clone());
    Ok(ProviderRevisionFacts {
        revision: revision.revision.clone(),
        contract_sha256: revision.contract.sha256.clone(),
        contract,
        contract_evidence_id: contract_evidence.id,
        fields,
        custom_mapping_evidence_ids,
    })
}

fn evidence_for_span(
    repository_identity: &str,
    revision: &str,
    path: &str,
    source: &[u8],
    span: SourceSpan,
    source_kind: EvidenceSourceKind,
    capability_version: &'static str,
) -> Result<SourceEvidence, ImpactFailure> {
    if !span.is_valid_for(source.len()) {
        return Err(operation_failure(
            CodeNoesisErrorV23::unsupported_implementation_semantics(),
        ));
    }
    evidence_for_lines(
        EvidenceLines {
            repository_identity,
            revision,
            path,
            source,
            start_line: span.start_line,
            end_line: span.end_line,
        },
        source_kind,
        capability_version,
    )
}

#[derive(Clone, Copy)]
struct EvidenceLines<'a> {
    repository_identity: &'a str,
    revision: &'a str,
    path: &'a str,
    source: &'a [u8],
    start_line: u64,
    end_line: u64,
}

fn evidence_for_lines(
    lines: EvidenceLines<'_>,
    source_kind: EvidenceSourceKind,
    capability_version: &'static str,
) -> Result<SourceEvidence, ImpactFailure> {
    let excerpt =
        line_excerpt(lines.source, lines.start_line, lines.end_line).ok_or_else(|| {
            operation_failure(CodeNoesisErrorV23::unsupported_implementation_semantics())
        })?;
    Ok(SourceEvidence::new(
        SourceEvidenceLocator {
            repository_identity: lines.repository_identity.to_owned(),
            revision: lines.revision.to_owned(),
            path: lines.path.to_owned(),
            start_line: lines.start_line,
            end_line: lines.end_line,
        },
        sha256(excerpt),
        source_kind,
        capability_version,
    ))
}

fn line_excerpt(source: &[u8], start_line: u64, end_line: u64) -> Option<&[u8]> {
    if start_line == 0 || end_line < start_line {
        return None;
    }
    let mut line = 1_u64;
    let mut start = None;
    let mut end = None;
    for (index, byte) in source.iter().enumerate() {
        if line == start_line && start.is_none() {
            start = Some(index);
        }
        if *byte == b'\n' {
            if line == end_line {
                end = Some(index + 1);
                break;
            }
            line = line.saturating_add(1);
        }
    }
    if start.is_none() && line == start_line {
        start = Some(source.len());
    }
    if end.is_none() && line == end_line {
        end = Some(source.len());
    }
    let start = start?;
    let end = end?;
    (start < end).then_some(&source[start..end])
}

fn source_failure(error: &SourceExtractionError) -> ImpactFailure {
    match error {
        SourceExtractionError::LimitExceeded(exceeded) => s7_limit_failure(*exceeded),
        SourceExtractionError::InvalidUtf8
        | SourceExtractionError::InvalidSyntax
        | SourceExtractionError::CallableMissing
        | SourceExtractionError::CallableAmbiguous
        | SourceExtractionError::UnsupportedSemantics => {
            operation_failure(CodeNoesisErrorV23::unsupported_implementation_semantics())
        }
    }
}

fn analysis_failure(error: &ImpactAnalysisError) -> ImpactFailure {
    match error {
        ImpactAnalysisError::LimitExceeded(exceeded) => s7_limit_failure(*exceeded),
        ImpactAnalysisError::InvalidAuthority | ImpactAnalysisError::UnsupportedSemantics => {
            operation_failure(CodeNoesisErrorV23::unsupported_implementation_semantics())
        }
    }
}

fn report_failure(error: SemanticReportError) -> ImpactFailure {
    match error {
        SemanticReportError::LimitExceeded(exceeded) => s7_limit_failure(exceeded),
        SemanticReportError::Invalid | SemanticReportError::Serialization => internal_failure(),
    }
}

fn s7_limit_failure(exceeded: S7LimitExceeded) -> ImpactFailure {
    operation_failure(CodeNoesisErrorV23::limit(
        exceeded.limit.as_str(),
        exceeded.maximum,
        exceeded.observed,
    ))
}

fn federation_failure(input: &BoundInput, error: S7FederationContractError) -> ImpactFailure {
    federation_bytes_failure(&input.logical_path, error)
}

fn federation_bytes_failure(logical_path: &str, error: S7FederationContractError) -> ImpactFailure {
    match error {
        S7FederationContractError::Invalid => operation_failure(
            CodeNoesisErrorV23::invalid_federation_report(logical_path, "report_validation"),
        ),
        S7FederationContractError::LimitExceeded { limit, observed } => {
            let maximum = match limit {
                "fields_per_operation" => 5_000,
                "operations" | "linked_clients" => 10_000,
                _ => return internal_failure(),
            };
            operation_failure(CodeNoesisErrorV23::limit(limit, maximum, observed))
        }
    }
}

fn workspace_failure(error: ImpactWorkspaceError) -> ImpactFailure {
    match error {
        ImpactWorkspaceError::Invalid => {
            input_failure(CodeNoesisErrorV23::invalid_workspace("invalid_manifest"))
        }
        ImpactWorkspaceError::TooManyClients { observed } => operation_failure(
            CodeNoesisErrorV23::limit("linked_clients", 10_000, observed),
        ),
        ImpactWorkspaceError::LimitExceeded {
            limit,
            maximum,
            observed,
        } => operation_failure(CodeNoesisErrorV23::limit(limit, maximum, observed)),
    }
}

fn path_failure(path: &str, component: &str, reason: &str) -> ImpactFailure {
    input_failure(CodeNoesisErrorV23::invalid_workspace_path(
        path, component, reason,
    ))
}

fn input_failure(error: CodeNoesisErrorV23) -> ImpactFailure {
    ImpactFailure {
        error,
        exit_code: 2,
    }
}

fn operation_failure(error: CodeNoesisErrorV23) -> ImpactFailure {
    ImpactFailure {
        error,
        exit_code: 2,
    }
}

fn internal_failure() -> ImpactFailure {
    ImpactFailure {
        error: CodeNoesisErrorV23::internal(),
        exit_code: 1,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::read_stable_with;

    static NEXT_TEST_ROOT: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn race_fr_cli_006_mutable_input_is_rejected() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let sequence = NEXT_TEST_ROOT.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("codenoesis-s7-mutable-{nonce}-{sequence}"));
        fs::create_dir_all(&root).expect("create mutable-input test root");
        let path = root.join("source.rs");
        fs::write(&path, b"baseline").expect("write baseline");

        let result = read_stable_with(&path, 64, "provider_source", || {
            fs::write(&path, b"modified").expect("replace input during stable read");
        });
        let failure = result.expect_err("mutable input must fail closed");
        assert_eq!(failure.exit_code, 2);
        assert_eq!(failure.error.value()["code"], "impact.mutable_input");
        assert_eq!(failure.error.value()["stage"], "impact");

        fs::remove_dir_all(root).expect("remove mutable-input test root");
    }
}
