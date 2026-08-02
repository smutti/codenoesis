use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

pub const ANALYSIS_PROFILE: &str = "standard-local-s6";
pub const CONTRACT_CAPABILITY: &str = "codenoesis.contract-capability/openapi-3.1-http-json/v1";
pub const FEDERATION_RULE_CATALOG: &str = "codenoesis.federation-rules/http-json/v1";
pub const FEDERATION_REPORT_SCHEMA: &str = "codenoesis.federation-report/v1";

const SERVICE_ID_DOMAIN: &str = "codenoesis.service-id/http/v1";
const OPERATION_ID_DOMAIN: &str = "codenoesis.operation-id/http/v1";
const SCHEMA_ID_DOMAIN: &str = "codenoesis.schema-id/http-json/v1";
const FIELD_ID_DOMAIN: &str = "codenoesis.field-id/http-json/v1";
const CLIENT_ID_DOMAIN: &str = "codenoesis.client-id/v1";
const CALL_SITE_ID_DOMAIN: &str = "codenoesis.call-site-id/v1";
const LINK_ID_DOMAIN: &str = "codenoesis.federation-link-id/v1";
const CANDIDATE_ID_DOMAIN: &str = "codenoesis.federation-candidate-id/v1";
const REJECTION_ID_DOMAIN: &str = "codenoesis.federation-rejection-id/v1";
const GAP_ID_DOMAIN: &str = "codenoesis.federation-gap-id/v1";
const EVIDENCE_ID_DOMAIN: &str = "codenoesis.federation-evidence-id/v1";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FederationLimit {
    WorkspaceManifestBytes,
    Repositories,
    ContractDocuments,
    ContractBytesPerDocument,
    YamlNestingDepth,
    LocalRefDepth,
    PathItems,
    Operations,
    Schemas,
    FieldsPerOperation,
    Clients,
    Declarations,
    ConfirmedLinks,
    Candidates,
    Rejections,
    EvidenceItems,
    CoverageGaps,
    ReportBytes,
    MemoryBytes,
    WallMilliseconds,
}

impl FederationLimit {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WorkspaceManifestBytes => "workspace_manifest_bytes",
            Self::Repositories => "repositories",
            Self::ContractDocuments => "contract_documents",
            Self::ContractBytesPerDocument => "contract_bytes_per_document",
            Self::YamlNestingDepth => "yaml_nesting_depth",
            Self::LocalRefDepth => "local_ref_depth",
            Self::PathItems => "path_items",
            Self::Operations => "operations",
            Self::Schemas => "schemas",
            Self::FieldsPerOperation => "fields_per_operation",
            Self::Clients => "clients",
            Self::Declarations => "declarations",
            Self::ConfirmedLinks => "confirmed_links",
            Self::Candidates => "candidates",
            Self::Rejections => "rejections",
            Self::EvidenceItems => "evidence_items",
            Self::CoverageGaps => "coverage_gaps",
            Self::ReportBytes => "report_bytes",
            Self::MemoryBytes => "memory_bytes",
            Self::WallMilliseconds => "wall_milliseconds",
        }
    }

    #[must_use]
    pub const fn maximum(self) -> u64 {
        match self {
            Self::WorkspaceManifestBytes => 8_388_608,
            Self::Repositories => 128,
            Self::ContractDocuments => 256,
            Self::ContractBytesPerDocument => 2_097_152,
            Self::YamlNestingDepth => 32,
            Self::LocalRefDepth => 16,
            Self::PathItems | Self::Operations | Self::Clients => 10_000,
            Self::Schemas => 20_000,
            Self::FieldsPerOperation => 5_000,
            Self::Declarations => 100_000,
            Self::ConfirmedLinks | Self::EvidenceItems => 1_000_000,
            Self::Candidates | Self::Rejections | Self::CoverageGaps => 200_000,
            Self::ReportBytes => 67_108_864,
            Self::MemoryBytes => 536_870_912,
            Self::WallMilliseconds => 60_000,
        }
    }

    #[must_use]
    pub const fn is_contract(self) -> bool {
        matches!(
            self,
            Self::ContractDocuments
                | Self::ContractBytesPerDocument
                | Self::YamlNestingDepth
                | Self::LocalRefDepth
                | Self::PathItems
                | Self::Operations
                | Self::Schemas
                | Self::FieldsPerOperation
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LimitExceeded {
    pub limit: FederationLimit,
    pub maximum: u64,
    pub observed: u64,
}

#[derive(Clone, Debug, Default)]
pub struct ResourceCounter {
    observed: BTreeMap<FederationLimit, u64>,
}

impl ResourceCounter {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            observed: BTreeMap::new(),
        }
    }

    /// Charges a bounded resource before its allocation or traversal.
    ///
    /// # Errors
    ///
    /// Returns the exact inclusive maximum-plus-one observation when the
    /// configured S6 limit would be exceeded.
    pub fn charge(&mut self, limit: FederationLimit, amount: u64) -> Result<u64, LimitExceeded> {
        let current = self.observed.get(&limit).copied().unwrap_or(0);
        let observed = current.saturating_add(amount);
        let maximum = limit.maximum();
        if observed > maximum {
            return Err(LimitExceeded {
                limit,
                maximum,
                observed,
            });
        }
        self.observed.insert(limit, observed);
        Ok(observed)
    }

    #[must_use]
    pub fn observed(&self, limit: FederationLimit) -> u64 {
        self.observed.get(&limit).copied().unwrap_or(0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContractError {
    InvalidEncoding { path: String },
    InvalidYaml { path: String },
    DuplicateKey { path: String },
    UnsupportedYamlFeature { path: String },
    UnsupportedOpenApiVersion { path: String },
    UnsupportedCapability { path: String },
    RemoteReferenceForbidden { path: String },
    ReferenceCycle { path: String },
    InvalidServiceAuthority { path: String },
    InvalidOperation { path: String },
    LimitExceeded { path: String, error: LimitExceeded },
}

impl ContractError {
    #[must_use]
    pub fn path(&self) -> &str {
        match self {
            Self::InvalidEncoding { path }
            | Self::InvalidYaml { path }
            | Self::DuplicateKey { path }
            | Self::UnsupportedYamlFeature { path }
            | Self::UnsupportedOpenApiVersion { path }
            | Self::UnsupportedCapability { path }
            | Self::RemoteReferenceForbidden { path }
            | Self::ReferenceCycle { path }
            | Self::InvalidServiceAuthority { path }
            | Self::InvalidOperation { path }
            | Self::LimitExceeded { path, .. } => path,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FederationError {
    InvalidDeclaration { path: String, subject_id: String },
    IdentityConflict { path: String, subject_id: String },
    AmbiguousAuthority { subject_id: String },
    LimitExceeded(LimitExceeded),
    ReportInvalid,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SourceFormat {
    Json,
    Yaml,
}

impl SourceFormat {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Yaml => "yaml",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum HttpMethod {
    Delete,
    Get,
    Patch,
    Post,
    Put,
}

impl HttpMethod {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Delete => "DELETE",
            Self::Get => "GET",
            Self::Patch => "PATCH",
            Self::Post => "POST",
            Self::Put => "PUT",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum JsonSchemaType {
    Array,
    Boolean,
    Integer,
    Number,
    Object,
    String,
}

impl JsonSchemaType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Array => "array",
            Self::Boolean => "boolean",
            Self::Integer => "integer",
            Self::Number => "number",
            Self::Object => "object",
            Self::String => "string",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderWorkspaceInput {
    pub repository_identity: String,
    pub revision: String,
    pub root: String,
    pub contract_path: String,
    pub contract_sha256: String,
    pub service_authority: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientWorkspaceInput {
    pub role: String,
    pub root: String,
    pub declaration_path: String,
    pub declaration_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FederationWorkspace {
    pub workspace_identity: String,
    pub provider: ProviderWorkspaceInput,
    pub clients: Vec<ClientWorkspaceInput>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderBinding {
    pub repository_identity: String,
    pub revision: String,
    pub contract_path: String,
    pub contract_sha256: String,
    pub service_authority: String,
    pub source_format: SourceFormat,
}

pub struct OpenApiContractInput<'a> {
    pub binding: ProviderBinding,
    pub bytes: &'a [u8],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EvidenceSelector {
    JsonPointer(String),
    OpenApiLocationSpan {
        location: String,
        start_line: u64,
        end_line: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FederationEvidence {
    pub evidence_id: String,
    pub kind: &'static str,
    pub repository_identity: String,
    pub revision: String,
    pub path: String,
    pub file_sha256: String,
    pub selector: EvidenceSelector,
}

impl FederationEvidence {
    #[must_use]
    pub fn openapi_json(binding: &ProviderBinding, pointer: &str) -> Self {
        Self {
            evidence_id: json_evidence_id(
                &binding.repository_identity,
                &binding.revision,
                &binding.contract_path,
                pointer,
                &binding.contract_sha256,
            ),
            kind: "openapi_json_pointer",
            repository_identity: binding.repository_identity.clone(),
            revision: binding.revision.clone(),
            path: binding.contract_path.clone(),
            file_sha256: binding.contract_sha256.clone(),
            selector: EvidenceSelector::JsonPointer(pointer.to_owned()),
        }
    }

    #[must_use]
    pub fn openapi_yaml(
        binding: &ProviderBinding,
        location: &str,
        start_line: u64,
        end_line: u64,
    ) -> Self {
        Self {
            evidence_id: yaml_evidence_id(
                &binding.repository_identity,
                &binding.revision,
                &binding.contract_path,
                location,
                start_line,
                end_line,
                &binding.contract_sha256,
            ),
            kind: "openapi_yaml_span",
            repository_identity: binding.repository_identity.clone(),
            revision: binding.revision.clone(),
            path: binding.contract_path.clone(),
            file_sha256: binding.contract_sha256.clone(),
            selector: EvidenceSelector::OpenApiLocationSpan {
                location: location.to_owned(),
                start_line,
                end_line,
            },
        }
    }

    #[must_use]
    pub fn workspace_binding(declaration: &ClientDeclaration) -> Self {
        let pointer = "/binding";
        Self {
            evidence_id: json_evidence_id(
                &declaration.repository_identity,
                &declaration.revision,
                &declaration.declaration_path,
                pointer,
                &declaration.declaration_sha256,
            ),
            kind: "workspace_json_pointer",
            repository_identity: declaration.repository_identity.clone(),
            revision: declaration.revision.clone(),
            path: declaration.declaration_path.clone(),
            file_sha256: declaration.declaration_sha256.clone(),
            selector: EvidenceSelector::JsonPointer(pointer.to_owned()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationField {
    pub field_id: String,
    pub json_pointer: String,
    pub required: bool,
    pub schema_type: JsonSchemaType,
    pub evidence_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderOperation {
    pub operation_id: String,
    pub service_id: String,
    pub method: HttpMethod,
    pub path_template: String,
    pub explicit_operation_id: String,
    pub response_status: String,
    pub schema_id: String,
    pub fields: Vec<OperationField>,
    pub evidence_ids: Vec<String>,
    pub primary_evidence_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoverageGap {
    pub coverage_gap_id: String,
    pub subject_id: String,
    pub reason_code: String,
    pub evidence_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderContract {
    pub binding: ProviderBinding,
    pub service_id: String,
    pub title: String,
    pub operations: Vec<ProviderOperation>,
    pub evidence: Vec<FederationEvidence>,
    pub evidence_ids: Vec<String>,
    pub coverage_gaps: Vec<CoverageGap>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClientBinding {
    ExplicitOperationIdentity {
        service_authority: String,
        method: HttpMethod,
        path_template: String,
        operation_id: String,
    },
    HeuristicName {
        service_hint: String,
        operation_hint: String,
    },
}

impl ClientBinding {
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::ExplicitOperationIdentity { .. } => "explicit_operation_identity",
            Self::HeuristicName { .. } => "heuristic_name",
        }
    }

    fn fingerprint(&self) -> Vec<&str> {
        match self {
            Self::ExplicitOperationIdentity {
                service_authority,
                method,
                path_template,
                operation_id,
            } => vec![
                self.kind(),
                service_authority,
                method.as_str(),
                path_template,
                operation_id,
            ],
            Self::HeuristicName {
                service_hint,
                operation_hint,
            } => vec![self.kind(), service_hint, operation_hint],
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDeclaration {
    pub role: String,
    pub repository_identity: String,
    pub revision: String,
    pub source_path: String,
    pub symbol_identity: String,
    pub binding: ClientBinding,
    pub declaration_path: String,
    pub declaration_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FederatedClient {
    pub role: String,
    pub repository_identity: String,
    pub revision: String,
    pub client_id: String,
    pub call_site_id: String,
    pub declaration_path: String,
    pub binding_kind: &'static str,
    pub operation_candidate_id: Option<String>,
    pub evidence_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfirmedLink {
    pub link_id: String,
    pub operation_id: String,
    pub client_id: String,
    pub call_site_id: String,
    pub evidence_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FederationCandidate {
    pub candidate_id: String,
    pub target_operation_id: String,
    pub client_id: String,
    pub call_site_id: String,
    pub service_hint: String,
    pub operation_hint: String,
    pub evidence_ids: Vec<String>,
    pub coverage_gap_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FederationRejection {
    pub rejection_id: String,
    pub operation_candidate_id: String,
    pub client_id: String,
    pub call_site_id: String,
    pub evidence_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FederationReport {
    pub workspace_identity: String,
    pub provider: ProviderContract,
    pub clients: Vec<FederatedClient>,
    pub confirmed_links: Vec<ConfirmedLink>,
    pub candidates: Vec<FederationCandidate>,
    pub rejections: Vec<FederationRejection>,
    pub evidence: Vec<FederationEvidence>,
    pub coverage_gaps: Vec<CoverageGap>,
}

/// Applies the approved deterministic S6 federation rules.
///
/// # Errors
///
/// Returns a typed declaration, authority, resource, or report failure without
/// publishing a partial result.
#[allow(clippy::too_many_lines)]
pub fn federate(
    workspace_identity: String,
    mut provider: ProviderContract,
    mut declarations: Vec<ClientDeclaration>,
) -> Result<FederationReport, FederationError> {
    for operation in &mut provider.operations {
        operation
            .fields
            .sort_by(|left, right| left.field_id.cmp(&right.field_id));
        operation.evidence_ids.sort();
        operation.evidence_ids.dedup();
        for field in &mut operation.fields {
            field.evidence_ids.sort();
            field.evidence_ids.dedup();
        }
    }
    for gap in &mut provider.coverage_gaps {
        gap.evidence_ids.sort();
        gap.evidence_ids.dedup();
    }
    provider
        .operations
        .sort_by(|left, right| left.operation_id.cmp(&right.operation_id));
    provider
        .coverage_gaps
        .sort_by(|left, right| left.coverage_gap_id.cmp(&right.coverage_gap_id));
    provider
        .evidence
        .sort_by(|left, right| left.evidence_id.cmp(&right.evidence_id));
    provider.evidence_ids.sort();
    provider.evidence_ids.dedup();
    declarations.sort_by(|left, right| {
        left.declaration_path
            .cmp(&right.declaration_path)
            .then_with(|| left.role.cmp(&right.role))
    });

    let mut counter = ResourceCounter::new();
    counter
        .charge(
            FederationLimit::Clients,
            u64::try_from(declarations.len()).unwrap_or(u64::MAX),
        )
        .map_err(FederationError::LimitExceeded)?;
    counter
        .charge(
            FederationLimit::Declarations,
            u64::try_from(declarations.len()).unwrap_or(u64::MAX),
        )
        .map_err(FederationError::LimitExceeded)?;
    counter
        .charge(
            FederationLimit::EvidenceItems,
            u64::try_from(provider.evidence.len()).unwrap_or(u64::MAX),
        )
        .map_err(FederationError::LimitExceeded)?;
    counter
        .charge(
            FederationLimit::CoverageGaps,
            u64::try_from(provider.coverage_gaps.len()).unwrap_or(u64::MAX),
        )
        .map_err(FederationError::LimitExceeded)?;

    let operation_by_id = provider
        .operations
        .iter()
        .map(|operation| (operation.operation_id.as_str(), operation))
        .collect::<BTreeMap<_, _>>();
    let fallback_provider_evidence = provider
        .operations
        .first()
        .map(|operation| operation.primary_evidence_id.as_str())
        .or_else(|| provider.evidence_ids.first().map(String::as_str));
    let mut authoritative_subjects = BTreeMap::<(String, String), Vec<String>>::new();
    let mut clients = Vec::with_capacity(declarations.len());
    let mut confirmed_links = Vec::new();
    let mut candidates = Vec::new();
    let mut rejections = Vec::new();
    let mut coverage_gaps = provider.coverage_gaps.clone();
    let mut evidence_by_id = provider
        .evidence
        .iter()
        .cloned()
        .map(|item| (item.evidence_id.clone(), item))
        .collect::<BTreeMap<_, _>>();

    for declaration in declarations {
        let client_id = client_id(&declaration.repository_identity);
        let call_site_id = call_site_id(
            &client_id,
            &declaration.revision,
            &declaration.source_path,
            &declaration.symbol_identity,
        );
        let fingerprint = declaration
            .binding
            .fingerprint()
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let subject = (client_id.clone(), call_site_id.clone());
        if let Some(previous) = authoritative_subjects.get(&subject) {
            return if previous == &fingerprint {
                Err(FederationError::InvalidDeclaration {
                    path: declaration.declaration_path,
                    subject_id: client_id,
                })
            } else {
                Err(FederationError::IdentityConflict {
                    path: declaration.declaration_path,
                    subject_id: client_id,
                })
            };
        }
        authoritative_subjects.insert(subject, fingerprint);

        let client_evidence = FederationEvidence::workspace_binding(&declaration);
        let client_evidence_id = client_evidence.evidence_id.clone();
        if !evidence_by_id.contains_key(&client_evidence_id) {
            counter
                .charge(FederationLimit::EvidenceItems, 1)
                .map_err(FederationError::LimitExceeded)?;
        }
        evidence_by_id.insert(client_evidence_id.clone(), client_evidence);
        let mut operation_candidate = None;

        match &declaration.binding {
            ClientBinding::ExplicitOperationIdentity {
                service_authority,
                method,
                path_template,
                operation_id: explicit_operation_id,
            } => {
                let candidate_service_id = service_id(service_authority);
                let candidate_operation_id = operation_id(
                    &candidate_service_id,
                    *method,
                    path_template,
                    explicit_operation_id,
                );
                operation_candidate = Some(candidate_operation_id.clone());
                if let Some(operation) = operation_by_id.get(candidate_operation_id.as_str()) {
                    counter
                        .charge(FederationLimit::ConfirmedLinks, 1)
                        .map_err(FederationError::LimitExceeded)?;
                    let evidence_ids = ordered_ids([
                        operation.primary_evidence_id.clone(),
                        client_evidence_id.clone(),
                    ]);
                    confirmed_links.push(ConfirmedLink {
                        link_id: confirmed_link_id(
                            &operation.operation_id,
                            &client_id,
                            &call_site_id,
                            declaration.binding.kind(),
                        ),
                        operation_id: operation.operation_id.clone(),
                        client_id: client_id.clone(),
                        call_site_id: call_site_id.clone(),
                        evidence_ids,
                    });
                } else {
                    counter
                        .charge(FederationLimit::Rejections, 1)
                        .map_err(FederationError::LimitExceeded)?;
                    let mut ids = vec![client_evidence_id.clone()];
                    if let Some(provider_evidence) = fallback_provider_evidence {
                        ids.push(provider_evidence.to_owned());
                    }
                    rejections.push(FederationRejection {
                        rejection_id: rejection_id(
                            &candidate_operation_id,
                            &client_id,
                            &call_site_id,
                            "operation_not_exposed",
                        ),
                        operation_candidate_id: candidate_operation_id,
                        client_id: client_id.clone(),
                        call_site_id: call_site_id.clone(),
                        evidence_ids: ordered_ids(ids),
                    });
                }
            }
            ClientBinding::HeuristicName {
                service_hint,
                operation_hint,
            } => {
                let matching = provider
                    .operations
                    .iter()
                    .filter(|operation| {
                        provider.title == *service_hint
                            && operation.explicit_operation_id == *operation_hint
                    })
                    .collect::<Vec<_>>();
                match matching.as_slice() {
                    [operation] => {
                        counter
                            .charge(FederationLimit::Candidates, 1)
                            .map_err(FederationError::LimitExceeded)?;
                        counter
                            .charge(FederationLimit::CoverageGaps, 1)
                            .map_err(FederationError::LimitExceeded)?;
                        let candidate_id = heuristic_candidate_id(
                            &operation.operation_id,
                            &client_id,
                            &call_site_id,
                            service_hint,
                            operation_hint,
                        );
                        let gap_id = heuristic_gap_id(
                            &candidate_id,
                            "heuristic_requires_confirmation",
                            &client_evidence_id,
                        );
                        candidates.push(FederationCandidate {
                            candidate_id: candidate_id.clone(),
                            target_operation_id: operation.operation_id.clone(),
                            client_id: client_id.clone(),
                            call_site_id: call_site_id.clone(),
                            service_hint: service_hint.clone(),
                            operation_hint: operation_hint.clone(),
                            evidence_ids: ordered_ids([
                                operation.primary_evidence_id.clone(),
                                client_evidence_id.clone(),
                            ]),
                            coverage_gap_id: gap_id.clone(),
                        });
                        coverage_gaps.push(CoverageGap {
                            coverage_gap_id: gap_id,
                            subject_id: candidate_id,
                            reason_code: "heuristic_requires_confirmation".to_owned(),
                            evidence_ids: vec![client_evidence_id.clone()],
                        });
                    }
                    [] => push_heuristic_gap(
                        &mut counter,
                        &mut coverage_gaps,
                        &call_site_id,
                        "heuristic_no_match",
                        &client_evidence_id,
                    )?,
                    _ => push_heuristic_gap(
                        &mut counter,
                        &mut coverage_gaps,
                        &call_site_id,
                        "heuristic_ambiguous",
                        &client_evidence_id,
                    )?,
                }
            }
        }

        clients.push(FederatedClient {
            role: declaration.role,
            repository_identity: declaration.repository_identity,
            revision: declaration.revision,
            client_id,
            call_site_id,
            declaration_path: declaration.declaration_path,
            binding_kind: declaration.binding.kind(),
            operation_candidate_id: operation_candidate,
            evidence_ids: vec![client_evidence_id],
        });
    }

    clients.sort_by(|left, right| {
        left.client_id
            .cmp(&right.client_id)
            .then_with(|| left.call_site_id.cmp(&right.call_site_id))
    });
    confirmed_links.sort_by(|left, right| left.link_id.cmp(&right.link_id));
    candidates.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    rejections.sort_by(|left, right| left.rejection_id.cmp(&right.rejection_id));
    coverage_gaps.sort_by(|left, right| left.coverage_gap_id.cmp(&right.coverage_gap_id));
    let evidence = evidence_by_id.into_values().collect::<Vec<_>>();

    if !ordered_unique_pairs(
        clients
            .iter()
            .map(|item| (item.client_id.as_str(), item.call_site_id.as_str())),
    ) || !ordered_unique(
        provider
            .operations
            .iter()
            .map(|item| item.operation_id.as_str()),
    ) || provider.operations.iter().any(|operation| {
        !ordered_unique(operation.fields.iter().map(|field| field.field_id.as_str()))
    }) || !ordered_unique(confirmed_links.iter().map(|item| item.link_id.as_str()))
        || !ordered_unique(candidates.iter().map(|item| item.candidate_id.as_str()))
        || !ordered_unique(rejections.iter().map(|item| item.rejection_id.as_str()))
        || !ordered_unique(
            coverage_gaps
                .iter()
                .map(|item| item.coverage_gap_id.as_str()),
        )
        || !ordered_unique(evidence.iter().map(|item| item.evidence_id.as_str()))
    {
        return Err(FederationError::ReportInvalid);
    }

    Ok(FederationReport {
        workspace_identity,
        provider,
        clients,
        confirmed_links,
        candidates,
        rejections,
        evidence,
        coverage_gaps,
    })
}

fn push_heuristic_gap(
    counter: &mut ResourceCounter,
    gaps: &mut Vec<CoverageGap>,
    call_site_id: &str,
    reason_code: &str,
    evidence_id: &str,
) -> Result<(), FederationError> {
    counter
        .charge(FederationLimit::CoverageGaps, 1)
        .map_err(FederationError::LimitExceeded)?;
    gaps.push(CoverageGap {
        coverage_gap_id: heuristic_gap_id(call_site_id, reason_code, evidence_id),
        subject_id: call_site_id.to_owned(),
        reason_code: reason_code.to_owned(),
        evidence_ids: vec![evidence_id.to_owned()],
    });
    Ok(())
}

#[must_use]
pub fn service_id(service_authority: &str) -> String {
    stable_id(
        "urn:codenoesis:service:blake3:",
        &[SERVICE_ID_DOMAIN, service_authority],
    )
}

#[must_use]
pub fn operation_id(
    service_id: &str,
    method: HttpMethod,
    path_template: &str,
    explicit_operation_id: &str,
) -> String {
    stable_id(
        "urn:codenoesis:operation:blake3:",
        &[
            OPERATION_ID_DOMAIN,
            service_id,
            method.as_str(),
            path_template,
            explicit_operation_id,
        ],
    )
}

#[must_use]
pub fn schema_id(
    operation_id: &str,
    direction: &str,
    response_status_or_request: &str,
    component_pointer: &str,
) -> String {
    stable_id(
        "urn:codenoesis:schema:blake3:",
        &[
            SCHEMA_ID_DOMAIN,
            operation_id,
            direction,
            response_status_or_request,
            component_pointer,
        ],
    )
}

#[must_use]
pub fn field_id(
    operation_id: &str,
    direction: &str,
    response_status_or_request: &str,
    json_pointer: &str,
) -> String {
    stable_id(
        "urn:codenoesis:field:blake3:",
        &[
            FIELD_ID_DOMAIN,
            operation_id,
            direction,
            response_status_or_request,
            json_pointer,
        ],
    )
}

#[must_use]
pub fn client_id(repository_identity: &str) -> String {
    stable_id(
        "urn:codenoesis:client:blake3:",
        &[CLIENT_ID_DOMAIN, repository_identity],
    )
}

#[must_use]
pub fn call_site_id(
    client_id: &str,
    revision: &str,
    source_path: &str,
    symbol_identity: &str,
) -> String {
    stable_id(
        "urn:codenoesis:call-site:blake3:",
        &[
            CALL_SITE_ID_DOMAIN,
            client_id,
            revision,
            source_path,
            symbol_identity,
        ],
    )
}

#[must_use]
pub fn confirmed_link_id(
    operation_id: &str,
    client_id: &str,
    call_site_id: &str,
    binding_kind: &str,
) -> String {
    stable_id(
        "urn:codenoesis:federation-link:blake3:",
        &[
            LINK_ID_DOMAIN,
            operation_id,
            client_id,
            call_site_id,
            binding_kind,
        ],
    )
}

#[must_use]
pub fn heuristic_candidate_id(
    operation_id: &str,
    client_id: &str,
    call_site_id: &str,
    service_hint: &str,
    operation_hint: &str,
) -> String {
    stable_id(
        "urn:codenoesis:federation-candidate:blake3:",
        &[
            CANDIDATE_ID_DOMAIN,
            operation_id,
            client_id,
            call_site_id,
            "heuristic_name",
            service_hint,
            operation_hint,
        ],
    )
}

#[must_use]
pub fn rejection_id(
    operation_candidate_id: &str,
    client_id: &str,
    call_site_id: &str,
    reason_code: &str,
) -> String {
    stable_id(
        "urn:codenoesis:federation-rejection:blake3:",
        &[
            REJECTION_ID_DOMAIN,
            operation_candidate_id,
            client_id,
            call_site_id,
            reason_code,
        ],
    )
}

#[must_use]
pub fn heuristic_gap_id(subject_id: &str, reason_code: &str, evidence_id: &str) -> String {
    stable_id(
        "urn:codenoesis:coverage-gap:blake3:",
        &[GAP_ID_DOMAIN, subject_id, reason_code, evidence_id],
    )
}

#[must_use]
pub fn contract_gap_id(subject_id: &str, reason_code: &str, location: &str) -> String {
    stable_id(
        "urn:codenoesis:coverage-gap:blake3:",
        &[GAP_ID_DOMAIN, subject_id, reason_code, location],
    )
}

#[must_use]
pub fn json_evidence_id(
    repository_identity: &str,
    revision: &str,
    path: &str,
    json_pointer: &str,
    file_sha256: &str,
) -> String {
    stable_id(
        "urn:codenoesis:evidence:blake3:",
        &[
            EVIDENCE_ID_DOMAIN,
            repository_identity,
            revision,
            path,
            json_pointer,
            file_sha256,
        ],
    )
}

#[must_use]
pub fn yaml_evidence_id(
    repository_identity: &str,
    revision: &str,
    path: &str,
    location: &str,
    start_line: u64,
    end_line: u64,
    file_sha256: &str,
) -> String {
    stable_id(
        "urn:codenoesis:evidence:blake3:",
        &[
            EVIDENCE_ID_DOMAIN,
            repository_identity,
            revision,
            path,
            location,
            "yaml",
            &start_line.to_string(),
            &end_line.to_string(),
            file_sha256,
        ],
    )
}

fn stable_id(prefix: &str, components: &[&str]) -> String {
    format!(
        "{prefix}{}",
        blake3::hash(&canonical_string_array(components)).to_hex()
    )
}

fn canonical_string_array(components: &[&str]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.push(b'[');
    for (index, component) in components.iter().enumerate() {
        if index > 0 {
            bytes.push(b',');
        }
        write_json_string(&mut bytes, component);
    }
    bytes.push(b']');
    bytes
}

fn write_json_string(bytes: &mut Vec<u8>, value: &str) {
    bytes.push(b'"');
    for character in value.chars() {
        match character {
            '"' => bytes.extend_from_slice(br#"\""#),
            '\\' => bytes.extend_from_slice(br"\\"),
            '\u{0008}' => bytes.extend_from_slice(br"\b"),
            '\u{000c}' => bytes.extend_from_slice(br"\f"),
            '\n' => bytes.extend_from_slice(br"\n"),
            '\r' => bytes.extend_from_slice(br"\r"),
            '\t' => bytes.extend_from_slice(br"\t"),
            '\u{0000}'..='\u{001f}' => {
                let _ = write!(ByteWriter(bytes), "\\u{:04x}", u32::from(character));
            }
            _ => {
                let mut buffer = [0; 4];
                bytes.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
            }
        }
    }
    bytes.push(b'"');
}

struct ByteWriter<'a>(&'a mut Vec<u8>);

impl std::fmt::Write for ByteWriter<'_> {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        self.0.extend_from_slice(value.as_bytes());
        Ok(())
    }
}

fn ordered_ids(values: impl IntoIterator<Item = String>) -> Vec<String> {
    values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn ordered_unique<'a>(values: impl IntoIterator<Item = &'a str>) -> bool {
    let mut previous = None;
    for value in values {
        if previous.is_some_and(|prior| prior >= value) {
            return false;
        }
        previous = Some(value);
    }
    true
}

fn ordered_unique_pairs<'a>(values: impl IntoIterator<Item = (&'a str, &'a str)>) -> bool {
    let mut previous = None;
    for value in values {
        if previous.is_some_and(|prior| prior >= value) {
            return false;
        }
        previous = Some(value);
    }
    true
}
