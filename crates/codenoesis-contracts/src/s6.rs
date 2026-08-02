use std::collections::{BTreeMap, BTreeSet};

use codenoesis_domain::s6::{
    ANALYSIS_PROFILE, CONTRACT_CAPABILITY, ClientBinding, ClientDeclaration, ClientWorkspaceInput,
    ContractError, EvidenceSelector, FEDERATION_REPORT_SCHEMA, FEDERATION_RULE_CATALOG,
    FederationError, FederationLimit, FederationReport, FederationWorkspace, HttpMethod,
    LimitExceeded, ProviderWorkspaceInput, client_id as expected_client_id, confirmed_link_id,
    contract_gap_id, field_id as expected_field_id, heuristic_candidate_id, heuristic_gap_id,
    json_evidence_id, operation_id as expected_operation_id, rejection_id,
    service_id as expected_service_id, yaml_evidence_id,
};
use serde_json::{Map, Value, json};

const WORKSPACE_SCHEMA: &str = "codenoesis.federation-workspace/v1";
const CLIENT_DECLARATION_SCHEMA: &str = "codenoesis.federation-client-declaration/v1";
const ERROR_SCHEMA: &str = "codenoesis.error/v8";
const SEMANTIC_HASH_DOMAIN: &[u8] = b"codenoesis.federation-report.semantic.v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum S6ContractError {
    InvalidWorkspaceManifest,
    InvalidClientDeclaration,
    LimitExceeded(LimitExceeded),
    ReportInvalid,
    Serialization,
}

/// Parses one closed `FederationWorkspaceV1` document.
///
/// # Errors
///
/// Returns a typed invalid-manifest or repository-cardinality failure.
pub fn parse_federation_workspace(bytes: &[u8]) -> Result<FederationWorkspace, S6ContractError> {
    let value: Value =
        serde_json::from_slice(bytes).map_err(|_| S6ContractError::InvalidWorkspaceManifest)?;
    let object = exact_object(
        &value,
        &[
            "schema_version",
            "workspace_identity",
            "analysis_profile",
            "contract_capability",
            "federation_rule_catalog",
            "provider",
            "clients",
        ],
    )
    .ok_or(S6ContractError::InvalidWorkspaceManifest)?;
    require_const(object, "schema_version", WORKSPACE_SCHEMA)?;
    require_const(object, "analysis_profile", ANALYSIS_PROFILE)?;
    require_const(object, "contract_capability", CONTRACT_CAPABILITY)?;
    require_const(object, "federation_rule_catalog", FEDERATION_RULE_CATALOG)?;
    let workspace_identity = object
        .get("workspace_identity")
        .and_then(Value::as_str)
        .filter(|value| valid_input_identity(value))
        .ok_or(S6ContractError::InvalidWorkspaceManifest)?
        .to_owned();
    let provider = parse_provider(
        object
            .get("provider")
            .ok_or(S6ContractError::InvalidWorkspaceManifest)?,
    )?;
    let clients_value = object
        .get("clients")
        .and_then(Value::as_array)
        .ok_or(S6ContractError::InvalidWorkspaceManifest)?;
    let repositories = clients_value
        .len()
        .checked_add(1)
        .and_then(|value| u64::try_from(value).ok())
        .unwrap_or(u64::MAX);
    if repositories > FederationLimit::Repositories.maximum() {
        return Err(S6ContractError::LimitExceeded(LimitExceeded {
            limit: FederationLimit::Repositories,
            maximum: FederationLimit::Repositories.maximum(),
            observed: repositories,
        }));
    }
    if u64::try_from(clients_value.len()).unwrap_or(u64::MAX) > FederationLimit::Clients.maximum() {
        return Err(S6ContractError::LimitExceeded(LimitExceeded {
            limit: FederationLimit::Clients,
            maximum: FederationLimit::Clients.maximum(),
            observed: u64::try_from(clients_value.len()).unwrap_or(u64::MAX),
        }));
    }
    let clients = clients_value
        .iter()
        .map(parse_client_input)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(FederationWorkspace {
        workspace_identity,
        provider,
        clients,
    })
}

/// Parses one closed `FederationClientDeclarationV1` document and binds its
/// catalog path and reviewed digest.
///
/// # Errors
///
/// Returns an invalid-declaration failure for any schema, role, or value
/// mismatch.
pub fn parse_client_declaration(
    bytes: &[u8],
    expected_role: &str,
    declaration_path: String,
    declaration_sha256: String,
) -> Result<ClientDeclaration, S6ContractError> {
    let value: Value =
        serde_json::from_slice(bytes).map_err(|_| S6ContractError::InvalidClientDeclaration)?;
    let object = exact_object(
        &value,
        &[
            "schema_version",
            "role",
            "repository_identity",
            "revision",
            "source_path",
            "symbol_identity",
            "binding",
        ],
    )
    .ok_or(S6ContractError::InvalidClientDeclaration)?;
    require_client_const(object, "schema_version", CLIENT_DECLARATION_SCHEMA)?;
    let role = client_string(object, "role", valid_role)?;
    if role != expected_role {
        return Err(S6ContractError::InvalidClientDeclaration);
    }
    let repository_identity = client_string(object, "repository_identity", valid_input_identity)?;
    let revision = client_string(object, "revision", valid_revision)?;
    let source_path = client_string(object, "source_path", valid_safe_path)?;
    let symbol_identity = client_string(object, "symbol_identity", |value| {
        !value.is_empty()
            && value.len() <= 512
            && !value
                .chars()
                .any(|character| matches!(character, '\0' | '\r' | '\n'))
    })?;
    let binding = parse_client_binding(
        object
            .get("binding")
            .ok_or(S6ContractError::InvalidClientDeclaration)?,
    )?;
    Ok(ClientDeclaration {
        role,
        repository_identity,
        revision,
        source_path,
        symbol_identity,
        binding,
        declaration_path,
        declaration_sha256,
    })
}

#[derive(Clone, Debug)]
pub struct FederationReportV1 {
    value: Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FederationReportError {
    Invalid,
    Serialization,
    LimitExceeded(LimitExceeded),
}

impl FederationReportV1 {
    /// Builds and validates the strict public S6 report.
    ///
    /// # Errors
    ///
    /// Returns an invalid report when ordering, references, or semantic hash
    /// construction cannot satisfy the approved contract.
    pub fn from_domain(report: &FederationReport) -> Result<Self, FederationReportError> {
        let mut value = report_value(report);
        let semantic_hash = semantic_hash(&value)?;
        value
            .as_object_mut()
            .ok_or(FederationReportError::Invalid)?
            .insert("semantic_hash".to_owned(), Value::String(semantic_hash));
        validate_report_value(&value)?;
        Ok(Self { value })
    }

    /// Serializes the complete report as canonical JSON plus LF and enforces
    /// the public report-byte limit before publication.
    ///
    /// # Errors
    ///
    /// Returns a serialization or exact report limit failure.
    pub fn canonical_stdout(&self) -> Result<Vec<u8>, FederationReportError> {
        let mut bytes =
            serde_json::to_vec(&self.value).map_err(|_| FederationReportError::Serialization)?;
        bytes.push(b'\n');
        let observed = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if observed > FederationLimit::ReportBytes.maximum() {
            return Err(FederationReportError::LimitExceeded(LimitExceeded {
                limit: FederationLimit::ReportBytes,
                maximum: FederationLimit::ReportBytes.maximum(),
                observed,
            }));
        }
        Ok(bytes)
    }

    #[must_use]
    pub fn value(&self) -> &Value {
        &self.value
    }
}

#[derive(Clone, Debug)]
pub struct CodeNoesisErrorV8 {
    value: Value,
}

impl CodeNoesisErrorV8 {
    #[must_use]
    pub fn invalid_profile() -> Self {
        Self::new(
            "input.invalid_profile",
            "input",
            "invalid analysis profile",
            Map::new(),
        )
    }

    #[must_use]
    pub fn invalid_format() -> Self {
        Self::new(
            "input.invalid_format",
            "input",
            "invalid output format",
            Map::new(),
        )
    }

    #[must_use]
    pub fn invalid_workspace_manifest() -> Self {
        Self::new(
            "input.invalid_workspace_manifest",
            "input",
            "invalid federation workspace manifest",
            Map::new(),
        )
    }

    #[must_use]
    pub fn path_invalid(path: &str) -> Self {
        Self::new(
            "acquisition.path_invalid",
            "acquisition",
            "invalid authorized path",
            context([
                ("path", Value::String(path.to_owned())),
                ("component", Value::String("workspace_manifest".to_owned())),
                ("reason", Value::String("unsafe_path".to_owned())),
            ]),
        )
    }

    #[must_use]
    pub fn root_policy_violation(path: &str) -> Self {
        Self::new(
            "acquisition.root_policy_violation",
            "acquisition",
            "repository root policy violation",
            context([
                ("path", Value::String(path.to_owned())),
                ("component", Value::String("workspace_manifest".to_owned())),
                ("reason", Value::String("root_policy_violation".to_owned())),
            ]),
        )
    }

    #[must_use]
    pub fn digest_mismatch(
        path: &str,
        component: &'static str,
        expected: &str,
        observed: &str,
    ) -> Self {
        Self::new(
            "acquisition.repository_inconsistent",
            "acquisition",
            "authorized file digest mismatch",
            context([
                ("path", Value::String(path.to_owned())),
                ("component", Value::String(component.to_owned())),
                ("reason", Value::String("digest_mismatch".to_owned())),
                ("expected_hash", Value::String(expected.to_owned())),
                ("observed_hash", Value::String(observed.to_owned())),
            ]),
        )
    }

    #[must_use]
    pub fn from_contract(error: &ContractError) -> Self {
        let path = error.path();
        match error {
            ContractError::InvalidEncoding { .. } => Self::contract(
                "contract.invalid_encoding",
                "contract is not valid UTF-8",
                path,
                "invalid_encoding",
            ),
            ContractError::InvalidYaml { .. } => Self::contract(
                "contract.invalid_yaml",
                "invalid YAML contract",
                path,
                "invalid_yaml",
            ),
            ContractError::DuplicateKey { .. } => Self::contract(
                "contract.duplicate_key",
                "duplicate YAML mapping key",
                path,
                "duplicate_mapping_key",
            ),
            ContractError::UnsupportedYamlFeature { .. } => Self::contract(
                "contract.unsupported_yaml_feature",
                "unsupported YAML feature",
                path,
                "unsupported_yaml_feature",
            ),
            ContractError::UnsupportedOpenApiVersion { .. } => Self::contract(
                "contract.unsupported_openapi_version",
                "unsupported OpenAPI version",
                path,
                "unsupported_openapi_version",
            ),
            ContractError::UnsupportedCapability { .. } => Self::contract(
                "contract.unsupported_capability",
                "unsupported contract capability",
                path,
                "unsupported_capability",
            ),
            ContractError::RemoteReferenceForbidden { .. } => Self::contract(
                "contract.remote_reference_forbidden",
                "remote contract reference is forbidden",
                path,
                "remote_reference",
            ),
            ContractError::ReferenceCycle { .. } => Self::contract(
                "contract.reference_cycle",
                "local contract reference cycle",
                path,
                "reference_cycle",
            ),
            ContractError::InvalidServiceAuthority { .. } => Self::contract(
                "contract.invalid_service_authority",
                "invalid service authority",
                path,
                "invalid_service_authority",
            ),
            ContractError::InvalidOperation { .. } => Self::contract(
                "contract.invalid_operation",
                "invalid OpenAPI operation",
                path,
                "invalid_operation",
            ),
            ContractError::LimitExceeded { error, .. } => Self::limit(error, Some(path)),
        }
    }

    #[must_use]
    pub fn invalid_declaration(path: &str) -> Self {
        Self::new(
            "federation.invalid_declaration",
            "federation",
            "invalid federation declaration",
            context([
                ("path", Value::String(path.to_owned())),
                ("component", Value::String("client_declaration".to_owned())),
                ("reason", Value::String("invalid_declaration".to_owned())),
            ]),
        )
    }

    #[must_use]
    pub fn from_federation(error: &FederationError) -> Self {
        match error {
            FederationError::InvalidDeclaration { path, .. } => Self::invalid_declaration(path),
            FederationError::IdentityConflict { path, subject_id } => Self::new(
                "federation.identity_conflict",
                "federation",
                "conflicting authoritative federation identity",
                context([
                    ("path", Value::String(path.clone())),
                    ("component", Value::String("operation_identity".to_owned())),
                    (
                        "reason",
                        Value::String("conflicting_authoritative_evidence".to_owned()),
                    ),
                    ("subject_id", Value::String(subject_id.clone())),
                ]),
            ),
            FederationError::AmbiguousAuthority { subject_id } => Self::new(
                "federation.ambiguous_authority",
                "federation",
                "ambiguous federation authority",
                context([
                    ("component", Value::String("operation_identity".to_owned())),
                    ("reason", Value::String("ambiguous_authority".to_owned())),
                    ("subject_id", Value::String(subject_id.clone())),
                ]),
            ),
            FederationError::LimitExceeded(error) => Self::limit(error, None),
            FederationError::ReportInvalid => Self::report_invalid(),
        }
    }

    #[must_use]
    pub fn limit(error: &LimitExceeded, path: Option<&str>) -> Self {
        let code = if error.limit.is_contract() {
            "contract.limit_exceeded"
        } else {
            "federation.limit_exceeded"
        };
        let stage = if error.limit.is_contract() {
            "contract"
        } else {
            "federation"
        };
        let mut values = vec![
            ("limit", Value::String(error.limit.as_str().to_owned())),
            ("maximum", Value::from(error.maximum)),
            ("observed", Value::from(error.observed)),
        ];
        if let Some(path) = path {
            values.push(("path", Value::String(path.to_owned())));
        }
        Self::new(code, stage, "S6 resource limit exceeded", context(values))
    }

    #[must_use]
    pub fn report_invalid() -> Self {
        Self::new(
            "federation.report_invalid",
            "federation",
            "invalid federation report",
            context([
                ("component", Value::String("federation_report".to_owned())),
                ("reason", Value::String("report_validation".to_owned())),
            ]),
        )
    }

    #[must_use]
    pub fn internal() -> Self {
        Self::new(
            "internal.unexpected",
            "internal",
            "unexpected internal failure",
            Map::new(),
        )
    }

    /// Serializes one strict canonical `ErrorV8` plus LF.
    ///
    /// # Errors
    ///
    /// Returns only an internal JSON serialization failure.
    pub fn canonical_stderr(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = serde_json::to_vec(&self.value)?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    #[must_use]
    pub fn value(&self) -> &Value {
        &self.value
    }

    fn contract(code: &str, message: &str, path: &str, reason: &str) -> Self {
        Self::new(
            code,
            "contract",
            message,
            context([
                ("path", Value::String(path.to_owned())),
                ("component", Value::String("provider_contract".to_owned())),
                ("reason", Value::String(reason.to_owned())),
            ]),
        )
    }

    fn new(code: &str, stage: &str, message: &str, context: Map<String, Value>) -> Self {
        Self {
            value: json!({
                "schema_version": ERROR_SCHEMA,
                "code": code,
                "stage": stage,
                "message": message,
                "retryable": false,
                "context": Value::Object(context)
            }),
        }
    }
}

fn parse_provider(value: &Value) -> Result<ProviderWorkspaceInput, S6ContractError> {
    let object = exact_object(
        value,
        &[
            "repository_identity",
            "revision",
            "root",
            "contract_path",
            "contract_sha256",
            "service_authority",
        ],
    )
    .ok_or(S6ContractError::InvalidWorkspaceManifest)?;
    let root = workspace_string(object, "root", valid_safe_path)?;
    let contract_path = workspace_string(object, "contract_path", valid_safe_path)?;
    if !valid_logical_path(&root, &contract_path) {
        return Err(S6ContractError::InvalidWorkspaceManifest);
    }
    Ok(ProviderWorkspaceInput {
        repository_identity: workspace_string(object, "repository_identity", valid_input_identity)?,
        revision: workspace_string(object, "revision", valid_revision)?,
        root,
        contract_path,
        contract_sha256: workspace_string(object, "contract_sha256", valid_digest)?,
        service_authority: workspace_string(object, "service_authority", valid_service_authority)?,
    })
}

fn parse_client_input(value: &Value) -> Result<ClientWorkspaceInput, S6ContractError> {
    let object = exact_object(
        value,
        &["role", "root", "declaration_path", "declaration_sha256"],
    )
    .ok_or(S6ContractError::InvalidWorkspaceManifest)?;
    let root = workspace_string(object, "root", valid_safe_path)?;
    let declaration_path = workspace_string(object, "declaration_path", valid_safe_path)?;
    if !valid_logical_path(&root, &declaration_path) {
        return Err(S6ContractError::InvalidWorkspaceManifest);
    }
    Ok(ClientWorkspaceInput {
        role: workspace_string(object, "role", valid_role)?,
        root,
        declaration_path,
        declaration_sha256: workspace_string(object, "declaration_sha256", valid_digest)?,
    })
}

fn parse_client_binding(value: &Value) -> Result<ClientBinding, S6ContractError> {
    let object = value
        .as_object()
        .ok_or(S6ContractError::InvalidClientDeclaration)?;
    match object.get("kind").and_then(Value::as_str) {
        Some("explicit_operation_identity") => {
            let object = exact_object(
                value,
                &[
                    "kind",
                    "service_authority",
                    "method",
                    "path_template",
                    "operation_id",
                ],
            )
            .ok_or(S6ContractError::InvalidClientDeclaration)?;
            Ok(ClientBinding::ExplicitOperationIdentity {
                service_authority: client_string(
                    object,
                    "service_authority",
                    valid_service_authority,
                )?,
                method: parse_method(
                    object
                        .get("method")
                        .and_then(Value::as_str)
                        .ok_or(S6ContractError::InvalidClientDeclaration)?,
                )?,
                path_template: client_string(object, "path_template", valid_path_template)?,
                operation_id: client_string(object, "operation_id", valid_operation_name)?,
            })
        }
        Some("heuristic_name") => {
            let object = exact_object(value, &["kind", "service_hint", "operation_hint"])
                .ok_or(S6ContractError::InvalidClientDeclaration)?;
            Ok(ClientBinding::HeuristicName {
                service_hint: client_string(object, "service_hint", |value| {
                    !value.is_empty()
                        && value.len() <= 256
                        && !value
                            .chars()
                            .any(|character| matches!(character, '\0' | '\r' | '\n'))
                })?,
                operation_hint: client_string(object, "operation_hint", valid_operation_name)?,
            })
        }
        _ => Err(S6ContractError::InvalidClientDeclaration),
    }
}

#[allow(clippy::too_many_lines)]
fn report_value(report: &FederationReport) -> Value {
    let provider = &report.provider;
    json!({
        "schema_version": FEDERATION_REPORT_SCHEMA,
        "analysis_profile": ANALYSIS_PROFILE,
        "contract_capability": CONTRACT_CAPABILITY,
        "federation_rule_catalog": FEDERATION_RULE_CATALOG,
        "workspace_identity": report.workspace_identity,
        "provider": {
            "repository_identity": provider.binding.repository_identity,
            "revision": provider.binding.revision,
            "contract_path": provider.binding.contract_path,
            "contract_sha256": provider.binding.contract_sha256,
            "source_format": provider.binding.source_format.as_str(),
            "service_id": provider.service_id,
            "service_authority": provider.binding.service_authority,
            "evidence_ids": provider.evidence_ids
        },
        "operations": report.provider.operations.iter().map(|operation| json!({
            "operation_id": operation.operation_id,
            "service_id": operation.service_id,
            "method": operation.method.as_str(),
            "path_template": operation.path_template,
            "explicit_operation_id": operation.explicit_operation_id,
            "response_status": operation.response_status,
            "schema_id": operation.schema_id,
            "fields": operation.fields.iter().map(|field| json!({
                "field_id": field.field_id,
                "json_pointer": field.json_pointer,
                "required": field.required,
                "type": field.schema_type.as_str(),
                "evidence_ids": field.evidence_ids
            })).collect::<Vec<_>>(),
            "evidence_ids": operation.evidence_ids
        })).collect::<Vec<_>>(),
        "clients": report.clients.iter().map(|client| json!({
            "role": client.role,
            "repository_identity": client.repository_identity,
            "revision": client.revision,
            "client_id": client.client_id,
            "call_site_id": client.call_site_id,
            "declaration_path": client.declaration_path,
            "binding_kind": client.binding_kind,
            "operation_candidate_id": client.operation_candidate_id,
            "evidence_ids": client.evidence_ids
        })).collect::<Vec<_>>(),
        "confirmed_links": report.confirmed_links.iter().map(|link| json!({
            "link_id": link.link_id,
            "operation_id": link.operation_id,
            "client_id": link.client_id,
            "call_site_id": link.call_site_id,
            "authority": "explicit_workspace_identity",
            "rule_id": "fed.explicit-operation.confirm/v1",
            "state": "confirmed",
            "evidence_ids": link.evidence_ids
        })).collect::<Vec<_>>(),
        "candidates": report.candidates.iter().map(|candidate| json!({
            "candidate_id": candidate.candidate_id,
            "target_operation_id": candidate.target_operation_id,
            "client_id": candidate.client_id,
            "call_site_id": candidate.call_site_id,
            "authority": "heuristic_candidate",
            "service_hint": candidate.service_hint,
            "operation_hint": candidate.operation_hint,
            "rule_id": "fed.heuristic-name.candidate/v1",
            "state": "candidate",
            "evidence_ids": candidate.evidence_ids,
            "coverage_gap_id": candidate.coverage_gap_id
        })).collect::<Vec<_>>(),
        "rejections": report.rejections.iter().map(|rejection| json!({
            "rejection_id": rejection.rejection_id,
            "operation_candidate_id": rejection.operation_candidate_id,
            "client_id": rejection.client_id,
            "call_site_id": rejection.call_site_id,
            "reason_code": "operation_not_exposed",
            "rule_id": "fed.operation-decoy.reject/v1",
            "state": "rejected",
            "evidence_ids": rejection.evidence_ids
        })).collect::<Vec<_>>(),
        "evidence": report.evidence.iter().map(|evidence| {
            let selector = match &evidence.selector {
                EvidenceSelector::JsonPointer(pointer) => json!({
                    "kind": "json_pointer",
                    "pointer": pointer
                }),
                EvidenceSelector::OpenApiLocationSpan { location, start_line, end_line } => json!({
                    "kind": "openapi_location_span",
                    "location": location,
                    "start_line": start_line,
                    "end_line": end_line
                })
            };
            json!({
                "evidence_id": evidence.evidence_id,
                "kind": evidence.kind,
                "repository_identity": evidence.repository_identity,
                "revision": evidence.revision,
                "path": evidence.path,
                "file_sha256": evidence.file_sha256,
                "selector": selector
            })
        }).collect::<Vec<_>>(),
        "coverage_gaps": report.coverage_gaps.iter().map(|gap| json!({
            "coverage_gap_id": gap.coverage_gap_id,
            "subject_id": gap.subject_id,
            "reason_code": gap.reason_code,
            "state": "unresolved",
            "evidence_ids": gap.evidence_ids
        })).collect::<Vec<_>>(),
        "limits": limits_value()
    })
}

fn limits_value() -> Value {
    let limits = [
        FederationLimit::WorkspaceManifestBytes,
        FederationLimit::Repositories,
        FederationLimit::ContractDocuments,
        FederationLimit::ContractBytesPerDocument,
        FederationLimit::YamlNestingDepth,
        FederationLimit::LocalRefDepth,
        FederationLimit::PathItems,
        FederationLimit::Operations,
        FederationLimit::Schemas,
        FederationLimit::FieldsPerOperation,
        FederationLimit::Clients,
        FederationLimit::Declarations,
        FederationLimit::ConfirmedLinks,
        FederationLimit::Candidates,
        FederationLimit::Rejections,
        FederationLimit::EvidenceItems,
        FederationLimit::CoverageGaps,
        FederationLimit::ReportBytes,
        FederationLimit::MemoryBytes,
        FederationLimit::WallMilliseconds,
    ];
    Value::Object(
        limits
            .into_iter()
            .map(|limit| (limit.as_str().to_owned(), Value::from(limit.maximum())))
            .collect(),
    )
}

fn semantic_hash(value: &Value) -> Result<String, FederationReportError> {
    let mut projection = value.clone();
    let root = projection
        .as_object_mut()
        .ok_or(FederationReportError::Invalid)?;
    root.remove("semantic_hash");
    root.remove("evidence");
    let provider = root
        .get_mut("provider")
        .and_then(Value::as_object_mut)
        .ok_or(FederationReportError::Invalid)?;
    provider.remove("contract_path");
    provider.remove("contract_sha256");
    provider.remove("source_format");
    if let Some(clients) = root.get_mut("clients").and_then(Value::as_array_mut) {
        for client in clients {
            client
                .as_object_mut()
                .ok_or(FederationReportError::Invalid)?
                .remove("declaration_path");
        }
    }
    remove_evidence_ids(&mut projection);
    let canonical =
        serde_json::to_vec(&projection).map_err(|_| FederationReportError::Serialization)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(SEMANTIC_HASH_DOMAIN);
    hasher.update(&[0]);
    hasher.update(&canonical);
    Ok(format!("blake3:{}", hasher.finalize().to_hex()))
}

fn remove_evidence_ids(value: &mut Value) {
    match value {
        Value::Object(object) => {
            object.remove("evidence_ids");
            for child in object.values_mut() {
                remove_evidence_ids(child);
            }
        }
        Value::Array(values) => {
            for child in values {
                remove_evidence_ids(child);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn validate_report_value(value: &Value) -> Result<(), FederationReportError> {
    let object = value.as_object().ok_or(FederationReportError::Invalid)?;
    if object.get("schema_version").and_then(Value::as_str) != Some(FEDERATION_REPORT_SCHEMA)
        || object.get("analysis_profile").and_then(Value::as_str) != Some(ANALYSIS_PROFILE)
        || object.get("contract_capability").and_then(Value::as_str) != Some(CONTRACT_CAPABILITY)
        || object
            .get("federation_rule_catalog")
            .and_then(Value::as_str)
            != Some(FEDERATION_RULE_CATALOG)
    {
        return Err(FederationReportError::Invalid);
    }
    let expected_hash = semantic_hash(value)?;
    if object.get("semantic_hash").and_then(Value::as_str) != Some(expected_hash.as_str()) {
        return Err(FederationReportError::Invalid);
    }
    let evidence = object
        .get("evidence")
        .and_then(Value::as_array)
        .ok_or(FederationReportError::Invalid)?;
    let evidence_ids = evidence
        .iter()
        .filter_map(|item| item.get("evidence_id").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    if evidence_ids.len() != evidence.len() || !array_sorted_by(evidence, "evidence_id") {
        return Err(FederationReportError::Invalid);
    }
    validate_nested_evidence_references(value, &evidence_ids)?;
    for (collection, identity_key) in [
        ("operations", "operation_id"),
        ("confirmed_links", "link_id"),
        ("candidates", "candidate_id"),
        ("rejections", "rejection_id"),
        ("coverage_gaps", "coverage_gap_id"),
    ] {
        let values = object
            .get(collection)
            .and_then(Value::as_array)
            .ok_or(FederationReportError::Invalid)?;
        if !array_sorted_by(values, identity_key) {
            return Err(FederationReportError::Invalid);
        }
    }
    let clients = object
        .get("clients")
        .and_then(Value::as_array)
        .ok_or(FederationReportError::Invalid)?;
    if !array_sorted_by_pair(clients, "client_id", "call_site_id") {
        return Err(FederationReportError::Invalid);
    }
    for operation in object
        .get("operations")
        .and_then(Value::as_array)
        .ok_or(FederationReportError::Invalid)?
    {
        let fields = operation
            .get("fields")
            .and_then(Value::as_array)
            .ok_or(FederationReportError::Invalid)?;
        if !array_sorted_by(fields, "field_id") {
            return Err(FederationReportError::Invalid);
        }
    }
    validate_report_identities_and_references(object)?;
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn validate_report_identities_and_references(
    report: &Map<String, Value>,
) -> Result<(), FederationReportError> {
    let provider = report
        .get("provider")
        .and_then(Value::as_object)
        .ok_or(FederationReportError::Invalid)?;
    let service_authority = value_string(provider, "service_authority")?;
    let service_id = value_string(provider, "service_id")?;
    if service_id != expected_service_id(service_authority) {
        return Err(FederationReportError::Invalid);
    }

    let operations = report
        .get("operations")
        .and_then(Value::as_array)
        .ok_or(FederationReportError::Invalid)?;
    let mut operation_ids = BTreeSet::new();
    for operation in operations {
        let operation = operation
            .as_object()
            .ok_or(FederationReportError::Invalid)?;
        let operation_id = value_string(operation, "operation_id")?;
        let method = report_method(value_string(operation, "method")?)
            .ok_or(FederationReportError::Invalid)?;
        if value_string(operation, "service_id")? != service_id
            || operation_id
                != expected_operation_id(
                    service_id,
                    method,
                    value_string(operation, "path_template")?,
                    value_string(operation, "explicit_operation_id")?,
                )
            || !operation_ids.insert(operation_id)
        {
            return Err(FederationReportError::Invalid);
        }
        let status = value_string(operation, "response_status")?;
        for field in operation
            .get("fields")
            .and_then(Value::as_array)
            .ok_or(FederationReportError::Invalid)?
        {
            let field = field.as_object().ok_or(FederationReportError::Invalid)?;
            if value_string(field, "field_id")?
                != expected_field_id(
                    operation_id,
                    "response",
                    status,
                    value_string(field, "json_pointer")?,
                )
            {
                return Err(FederationReportError::Invalid);
            }
        }
    }

    let clients = report
        .get("clients")
        .and_then(Value::as_array)
        .ok_or(FederationReportError::Invalid)?;
    let mut client_call_sites = BTreeMap::new();
    for client in clients {
        let client = client.as_object().ok_or(FederationReportError::Invalid)?;
        let client_id = value_string(client, "client_id")?;
        let call_site_id = value_string(client, "call_site_id")?;
        if client_id != expected_client_id(value_string(client, "repository_identity")?)
            || client_call_sites
                .insert((client_id, call_site_id), client)
                .is_some()
        {
            return Err(FederationReportError::Invalid);
        }
    }

    let gaps = report
        .get("coverage_gaps")
        .and_then(Value::as_array)
        .ok_or(FederationReportError::Invalid)?;
    let gap_ids = gaps
        .iter()
        .map(|gap| value_string_value(gap, "coverage_gap_id"))
        .collect::<Result<BTreeSet<_>, _>>()?;
    let candidates = report
        .get("candidates")
        .and_then(Value::as_array)
        .ok_or(FederationReportError::Invalid)?;
    let candidate_ids = candidates
        .iter()
        .map(|candidate| value_string_value(candidate, "candidate_id"))
        .collect::<Result<BTreeSet<_>, _>>()?;

    for link in report
        .get("confirmed_links")
        .and_then(Value::as_array)
        .ok_or(FederationReportError::Invalid)?
    {
        let link = link.as_object().ok_or(FederationReportError::Invalid)?;
        let operation_id = value_string(link, "operation_id")?;
        let client_id = value_string(link, "client_id")?;
        let call_site_id = value_string(link, "call_site_id")?;
        let client = client_call_sites
            .get(&(client_id, call_site_id))
            .ok_or(FederationReportError::Invalid)?;
        if !operation_ids.contains(operation_id)
            || value_string(link, "link_id")?
                != confirmed_link_id(
                    operation_id,
                    client_id,
                    call_site_id,
                    value_string(client, "binding_kind")?,
                )
        {
            return Err(FederationReportError::Invalid);
        }
    }

    for candidate in candidates {
        let candidate = candidate
            .as_object()
            .ok_or(FederationReportError::Invalid)?;
        let target = value_string(candidate, "target_operation_id")?;
        let client_id = value_string(candidate, "client_id")?;
        let call_site_id = value_string(candidate, "call_site_id")?;
        if !operation_ids.contains(target)
            || !client_call_sites.contains_key(&(client_id, call_site_id))
            || !gap_ids.contains(value_string(candidate, "coverage_gap_id")?)
            || value_string(candidate, "candidate_id")?
                != heuristic_candidate_id(
                    target,
                    client_id,
                    call_site_id,
                    value_string(candidate, "service_hint")?,
                    value_string(candidate, "operation_hint")?,
                )
        {
            return Err(FederationReportError::Invalid);
        }
    }

    for rejection in report
        .get("rejections")
        .and_then(Value::as_array)
        .ok_or(FederationReportError::Invalid)?
    {
        let rejection = rejection
            .as_object()
            .ok_or(FederationReportError::Invalid)?;
        let client_id = value_string(rejection, "client_id")?;
        let call_site_id = value_string(rejection, "call_site_id")?;
        let candidate = value_string(rejection, "operation_candidate_id")?;
        let reason = value_string(rejection, "reason_code")?;
        if !client_call_sites.contains_key(&(client_id, call_site_id))
            || value_string(rejection, "rejection_id")?
                != rejection_id(candidate, client_id, call_site_id, reason)
        {
            return Err(FederationReportError::Invalid);
        }
    }

    let evidence = report
        .get("evidence")
        .and_then(Value::as_array)
        .ok_or(FederationReportError::Invalid)?;
    let evidence_by_id = evidence
        .iter()
        .map(|item| Ok((value_string_value(item, "evidence_id")?, item)))
        .collect::<Result<BTreeMap<_, _>, FederationReportError>>()?;
    for item in evidence {
        validate_evidence_identity(item)?;
    }
    for gap in gaps {
        validate_gap_identity(
            gap,
            service_id,
            &operation_ids,
            &candidate_ids,
            &client_call_sites,
            &evidence_by_id,
        )?;
    }
    Ok(())
}

fn validate_evidence_identity(item: &Value) -> Result<(), FederationReportError> {
    let item = item.as_object().ok_or(FederationReportError::Invalid)?;
    let selector = item
        .get("selector")
        .and_then(Value::as_object)
        .ok_or(FederationReportError::Invalid)?;
    let expected = match value_string(item, "kind")? {
        "openapi_json_pointer" | "workspace_json_pointer" => json_evidence_id(
            value_string(item, "repository_identity")?,
            value_string(item, "revision")?,
            value_string(item, "path")?,
            value_string(selector, "pointer")?,
            value_string(item, "file_sha256")?,
        ),
        "openapi_yaml_span" => yaml_evidence_id(
            value_string(item, "repository_identity")?,
            value_string(item, "revision")?,
            value_string(item, "path")?,
            value_string(selector, "location")?,
            selector
                .get("start_line")
                .and_then(Value::as_u64)
                .ok_or(FederationReportError::Invalid)?,
            selector
                .get("end_line")
                .and_then(Value::as_u64)
                .ok_or(FederationReportError::Invalid)?,
            value_string(item, "file_sha256")?,
        ),
        _ => return Err(FederationReportError::Invalid),
    };
    if value_string(item, "evidence_id")? != expected {
        return Err(FederationReportError::Invalid);
    }
    Ok(())
}

fn validate_gap_identity<'a>(
    gap: &'a Value,
    service_id: &str,
    operation_ids: &BTreeSet<&'a str>,
    candidate_ids: &BTreeSet<&'a str>,
    client_call_sites: &BTreeMap<(&'a str, &'a str), &'a Map<String, Value>>,
    evidence_by_id: &BTreeMap<&'a str, &'a Value>,
) -> Result<(), FederationReportError> {
    let gap = gap.as_object().ok_or(FederationReportError::Invalid)?;
    let subject = value_string(gap, "subject_id")?;
    let reason = value_string(gap, "reason_code")?;
    let evidence_ids = gap
        .get("evidence_ids")
        .and_then(Value::as_array)
        .ok_or(FederationReportError::Invalid)?;
    if evidence_ids.len() != 1 {
        return Err(FederationReportError::Invalid);
    }
    let evidence_id = evidence_ids[0]
        .as_str()
        .ok_or(FederationReportError::Invalid)?;
    let expected = match reason {
        "heuristic_requires_confirmation" => {
            if !candidate_ids.contains(subject) {
                return Err(FederationReportError::Invalid);
            }
            heuristic_gap_id(subject, reason, evidence_id)
        }
        "heuristic_no_match" | "heuristic_ambiguous" => {
            if !client_call_sites
                .keys()
                .any(|(_, call_site_id)| *call_site_id == subject)
            {
                return Err(FederationReportError::Invalid);
            }
            heuristic_gap_id(subject, reason, evidence_id)
        }
        "unsupported_callbacks"
        | "unsupported_links"
        | "unsupported_media_type"
        | "unsupported_security_semantics"
        | "unsupported_server_variables"
        | "unsupported_webhooks" => {
            if subject != service_id && !operation_ids.contains(subject) {
                return Err(FederationReportError::Invalid);
            }
            let evidence = evidence_by_id
                .get(evidence_id)
                .ok_or(FederationReportError::Invalid)?;
            contract_gap_id(subject, reason, &evidence_openapi_location(evidence)?)
        }
        _ => return Err(FederationReportError::Invalid),
    };
    if value_string(gap, "coverage_gap_id")? != expected {
        return Err(FederationReportError::Invalid);
    }
    Ok(())
}

fn evidence_openapi_location(evidence: &Value) -> Result<String, FederationReportError> {
    let selector = evidence
        .get("selector")
        .and_then(Value::as_object)
        .ok_or(FederationReportError::Invalid)?;
    match value_string(selector, "kind")? {
        "json_pointer" => Ok(format!("#{}", value_string(selector, "pointer")?)),
        "openapi_location_span" => Ok(value_string(selector, "location")?.to_owned()),
        _ => Err(FederationReportError::Invalid),
    }
}

fn value_string<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a str, FederationReportError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or(FederationReportError::Invalid)
}

fn value_string_value<'a>(value: &'a Value, key: &str) -> Result<&'a str, FederationReportError> {
    value
        .as_object()
        .ok_or(FederationReportError::Invalid)
        .and_then(|object| value_string(object, key))
}

fn report_method(value: &str) -> Option<HttpMethod> {
    match value {
        "DELETE" => Some(HttpMethod::Delete),
        "GET" => Some(HttpMethod::Get),
        "PATCH" => Some(HttpMethod::Patch),
        "POST" => Some(HttpMethod::Post),
        "PUT" => Some(HttpMethod::Put),
        _ => None,
    }
}

fn validate_nested_evidence_references(
    value: &Value,
    evidence_ids: &BTreeSet<&str>,
) -> Result<(), FederationReportError> {
    match value {
        Value::Object(object) => {
            if let Some(references) = object.get("evidence_ids") {
                let references = references
                    .as_array()
                    .ok_or(FederationReportError::Invalid)?;
                let mut previous = None;
                for reference in references {
                    let reference = reference.as_str().ok_or(FederationReportError::Invalid)?;
                    if !evidence_ids.contains(reference)
                        || previous.is_some_and(|prior| prior >= reference)
                    {
                        return Err(FederationReportError::Invalid);
                    }
                    previous = Some(reference);
                }
            }
            for child in object.values() {
                validate_nested_evidence_references(child, evidence_ids)?;
            }
        }
        Value::Array(values) => {
            for child in values {
                validate_nested_evidence_references(child, evidence_ids)?;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    Ok(())
}

fn array_sorted_by(values: &[Value], key: &str) -> bool {
    let mut previous = None;
    for value in values {
        let Some(identity) = value.get(key).and_then(Value::as_str) else {
            return false;
        };
        if previous.is_some_and(|prior| prior >= identity) {
            return false;
        }
        previous = Some(identity);
    }
    true
}

fn array_sorted_by_pair(values: &[Value], first: &str, second: &str) -> bool {
    let mut previous = None;
    for value in values {
        let Some(pair) = value
            .get(first)
            .and_then(Value::as_str)
            .zip(value.get(second).and_then(Value::as_str))
        else {
            return false;
        };
        if previous.is_some_and(|prior| prior >= pair) {
            return false;
        }
        previous = Some(pair);
    }
    true
}

fn exact_object<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Map<String, Value>> {
    let object = value.as_object()?;
    (object.len() == keys.len() && keys.iter().all(|key| object.contains_key(*key)))
        .then_some(object)
}

fn require_const(
    object: &Map<String, Value>,
    key: &str,
    expected: &str,
) -> Result<(), S6ContractError> {
    (object.get(key).and_then(Value::as_str) == Some(expected))
        .then_some(())
        .ok_or(S6ContractError::InvalidWorkspaceManifest)
}

fn require_client_const(
    object: &Map<String, Value>,
    key: &str,
    expected: &str,
) -> Result<(), S6ContractError> {
    (object.get(key).and_then(Value::as_str) == Some(expected))
        .then_some(())
        .ok_or(S6ContractError::InvalidClientDeclaration)
}

fn workspace_string(
    object: &Map<String, Value>,
    key: &str,
    validator: impl FnOnce(&str) -> bool,
) -> Result<String, S6ContractError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| validator(value))
        .map(str::to_owned)
        .ok_or(S6ContractError::InvalidWorkspaceManifest)
}

fn client_string(
    object: &Map<String, Value>,
    key: &str,
    validator: impl FnOnce(&str) -> bool,
) -> Result<String, S6ContractError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| validator(value))
        .map(str::to_owned)
        .ok_or(S6ContractError::InvalidClientDeclaration)
}

fn parse_method(value: &str) -> Result<HttpMethod, S6ContractError> {
    match value {
        "DELETE" => Ok(HttpMethod::Delete),
        "GET" => Ok(HttpMethod::Get),
        "PATCH" => Ok(HttpMethod::Patch),
        "POST" => Ok(HttpMethod::Post),
        "PUT" => Ok(HttpMethod::Put),
        _ => Err(S6ContractError::InvalidClientDeclaration),
    }
}

fn valid_input_identity(value: &str) -> bool {
    let Some(suffix) = value.strip_prefix("urn:codenoesis:") else {
        return false;
    };
    let mut bytes = suffix.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    value.len() <= 512
        && (first.is_ascii_lowercase() || first.is_ascii_digit())
        && bytes.all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b':' | b'.' | b'_' | b'/' | b'-')
        })
}

fn valid_revision(value: &str) -> bool {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    value.len() <= 128
        && first.is_ascii_alphanumeric()
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn valid_safe_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 4096
        && !value.starts_with('/')
        && !value
            .as_bytes()
            .get(1)
            .is_some_and(|byte| *byte == b':' && value.as_bytes()[0].is_ascii_alphabetic())
        && !value.contains(['\\', '\0', '\r', '\n'])
        && value
            .split('/')
            .all(|component| !component.is_empty() && !matches!(component, "." | ".."))
}

fn valid_logical_path(root: &str, path: &str) -> bool {
    root.len()
        .checked_add(path.len())
        .and_then(|length| length.checked_add(1))
        .is_some_and(|length| length <= 4096)
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn valid_role(value: &str) -> bool {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    value.len() <= 64
        && first.is_ascii_lowercase()
        && bytes.all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

fn valid_service_authority(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("https://") else {
        return false;
    };
    if value.len() > 2048
        || rest.is_empty()
        || rest.contains(['@', '?', '#', '{', '}'])
        || rest.bytes().any(|byte| byte.is_ascii_uppercase())
    {
        return false;
    }
    let authority = rest.split('/').next().unwrap_or_default();
    let (host, port) = authority
        .rsplit_once(':')
        .filter(|(_, port)| !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit()))
        .map_or((authority, None), |(host, port)| (host, Some(port)));
    !host.is_empty()
        && host.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-')
        })
        && port
            .is_none_or(|port| port.len() <= 5 && port.parse::<u16>().is_ok_and(|value| value > 0))
        && rest.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'.' | b'_'
                        | b'~'
                        | b'!'
                        | b'$'
                        | b'&'
                        | b'\''
                        | b'('
                        | b')'
                        | b'*'
                        | b'+'
                        | b','
                        | b';'
                        | b'='
                        | b':'
                        | b'@'
                        | b'%'
                        | b'/'
                        | b'-'
                )
        })
}

fn valid_path_template(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 2048
        && value.starts_with('/')
        && (value == "/" || !value[1..].split('/').any(str::is_empty))
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'.' | b'_'
                        | b'~'
                        | b'!'
                        | b'$'
                        | b'&'
                        | b'\''
                        | b'('
                        | b')'
                        | b'*'
                        | b'+'
                        | b','
                        | b';'
                        | b'='
                        | b':'
                        | b'@'
                        | b'%'
                        | b'{'
                        | b'}'
                        | b'/'
                        | b'-'
                )
        })
}

fn valid_operation_name(value: &str) -> bool {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    value.len() <= 256
        && first.is_ascii_alphabetic()
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn context<'a>(values: impl IntoIterator<Item = (&'a str, Value)>) -> Map<String, Value> {
    values
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value))
        .collect()
}
