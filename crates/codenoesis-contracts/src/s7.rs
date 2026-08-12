use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use codenoesis_domain::s6::{
    FederationLimit, HttpMethod, call_site_id as expected_call_site_id,
    client_id as expected_client_id, confirmed_link_id, field_id as expected_field_id,
    heuristic_candidate_id, heuristic_gap_id, json_evidence_id,
    operation_id as expected_operation_id, rejection_id as expected_rejection_id,
    service_id as expected_service_id, yaml_evidence_id,
};
use codenoesis_domain::s7::{
    ANALYSIS_PROFILE, CONFIGURATION_HASH, EVIDENCE_LINEAGE_VERSION, ONTOLOGY_VERSION,
    REPORT_PIPELINE, REPORT_SCHEMA, RULE_CATALOG_VERSION, S7Limit, S7LimitExceeded,
    SemanticCompatibilityReport,
};
use serde_json::{Map, Value, json};

pub const IMPACT_WORKSPACE_SCHEMA: &str = "codenoesis.impact-workspace/v1";
pub const IMPACT_ANALYSIS_PROFILE: &str = "implementation-aware-http-json/v1";
pub const IMPACT_PIPELINE: &str = "codenoesis.pipeline/s7-v1";
pub const IMPACT_CONTRACT_CAPABILITY: &str =
    "codenoesis.contract-capability/openapi-3.1-http-json/v1";
pub const IMPACT_PROVIDER_CAPABILITY: &str = "rust-direct-json-map/v1";
pub const IMPACT_CLIENT_CAPABILITY: &str = "kotlin-direct-json-access/v1";
pub const IMPACT_ERROR_SCHEMA: &str = "codenoesis.error/v23";

const S6_ANALYSIS_PROFILE: &str = "standard-local-s6";
const S6_FEDERATION_SCHEMA: &str = "codenoesis.federation-report/v1";
const S6_RULE_CATALOG: &str = "codenoesis.federation-rules/http-json/v1";
const S6_SEMANTIC_HASH_DOMAIN: &[u8] = b"codenoesis.federation-report.semantic.v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct S7FederationAuthority {
    pub provider_repository_identity: String,
    pub provider_revision: String,
    pub service_authority: String,
    pub service_id: String,
    pub operations: Vec<S7FederatedOperation>,
    pub clients: Vec<S7FederatedClient>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct S7FederatedOperation {
    pub operation_id: String,
    pub method: String,
    pub path_template: String,
    pub explicit_operation_id: String,
    pub response_status: String,
    pub fields: Vec<(String, String, bool)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct S7FederatedClient {
    pub role: String,
    pub repository_identity: String,
    pub revision: String,
    pub client_id: String,
    pub call_site_id: String,
    pub state: S7FederationState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum S7FederationState {
    Confirmed { operation_id: String },
    Rejected { operation_candidate_id: String },
    Unresolved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum S7FederationContractError {
    Invalid,
    LimitExceeded { limit: &'static str, observed: u64 },
}

/// Parses the S6 artifact as immutable operation/link authority for S7.
///
/// # Errors
///
/// Returns a closed schema, semantic-hash, identity, reference, ordering, or
/// cardinality failure.
#[allow(clippy::too_many_lines)]
pub fn parse_s7_federation_authority(
    bytes: &[u8],
) -> Result<S7FederationAuthority, S7FederationContractError> {
    let value: Value =
        serde_json::from_slice(bytes).map_err(|_| S7FederationContractError::Invalid)?;
    validate_s6_semantic_hash(&value)?;
    let root = exact_object(
        &value,
        &[
            "analysis_profile",
            "candidates",
            "clients",
            "confirmed_links",
            "contract_capability",
            "coverage_gaps",
            "evidence",
            "federation_rule_catalog",
            "limits",
            "operations",
            "provider",
            "rejections",
            "schema_version",
            "semantic_hash",
            "workspace_identity",
        ],
    )
    .ok_or(S7FederationContractError::Invalid)?;
    if root.get("schema_version").and_then(Value::as_str) != Some(S6_FEDERATION_SCHEMA)
        || root.get("analysis_profile").and_then(Value::as_str) != Some(S6_ANALYSIS_PROFILE)
        || root.get("contract_capability").and_then(Value::as_str)
            != Some(IMPACT_CONTRACT_CAPABILITY)
        || root.get("federation_rule_catalog").and_then(Value::as_str) != Some(S6_RULE_CATALOG)
    {
        return Err(S7FederationContractError::Invalid);
    }
    for collection in ["candidates", "coverage_gaps", "evidence"] {
        if !root.get(collection).is_some_and(Value::is_array) {
            return Err(S7FederationContractError::Invalid);
        }
    }
    validate_s6_limits(root)?;
    let provider_value = root
        .get("provider")
        .ok_or(S7FederationContractError::Invalid)?;
    let provider = exact_object(
        provider_value,
        &[
            "contract_path",
            "contract_sha256",
            "evidence_ids",
            "repository_identity",
            "revision",
            "service_authority",
            "service_id",
            "source_format",
        ],
    )
    .ok_or(S7FederationContractError::Invalid)?;
    let provider_repository_identity = required(provider, "repository_identity")?.to_owned();
    let provider_revision = required(provider, "revision")?.to_owned();
    let service_authority = required(provider, "service_authority")?.to_owned();
    let service_id = required(provider, "service_id")?.to_owned();
    if service_id != expected_service_id(&service_authority)
        || required(provider, "source_format")? != "yaml"
        || !valid_safe_path(required(provider, "contract_path")?)
        || !valid_digest(required(provider, "contract_sha256")?)
    {
        return Err(S7FederationContractError::Invalid);
    }

    let operation_values = array(root, "operations")?;
    cardinality(
        "operations",
        S7Limit::Operations.maximum(),
        operation_values.len(),
    )?;
    if !sorted_by(operation_values, "operation_id") {
        return Err(S7FederationContractError::Invalid);
    }
    let mut operations = Vec::with_capacity(operation_values.len());
    let mut operation_ids = BTreeSet::new();
    for operation in operation_values {
        let operation = exact_object(
            operation,
            &[
                "evidence_ids",
                "explicit_operation_id",
                "fields",
                "method",
                "operation_id",
                "path_template",
                "response_status",
                "schema_id",
                "service_id",
            ],
        )
        .ok_or(S7FederationContractError::Invalid)?;
        let operation_id = required(operation, "operation_id")?.to_owned();
        let method = required(operation, "method")?.to_owned();
        let path_template = required(operation, "path_template")?.to_owned();
        let explicit_operation_id = required(operation, "explicit_operation_id")?.to_owned();
        let response_status = required(operation, "response_status")?.to_owned();
        let method_value = parse_http_method(&method).ok_or(S7FederationContractError::Invalid)?;
        if required(operation, "service_id")? != service_id
            || operation_id
                != expected_operation_id(
                    &service_id,
                    method_value,
                    &path_template,
                    &explicit_operation_id,
                )
            || !operation_ids.insert(operation_id.clone())
        {
            return Err(S7FederationContractError::Invalid);
        }
        let field_values = array(operation, "fields")?;
        cardinality(
            "fields_per_operation",
            S7Limit::FieldsPerOperation.maximum(),
            field_values.len(),
        )?;
        if !sorted_by(field_values, "field_id") {
            return Err(S7FederationContractError::Invalid);
        }
        let mut fields = Vec::with_capacity(field_values.len());
        for field in field_values {
            let field = exact_object(
                field,
                &[
                    "evidence_ids",
                    "field_id",
                    "json_pointer",
                    "required",
                    "type",
                ],
            )
            .ok_or(S7FederationContractError::Invalid)?;
            let field_id = required(field, "field_id")?.to_owned();
            let pointer = required(field, "json_pointer")?.to_owned();
            let required_presence = field
                .get("required")
                .and_then(Value::as_bool)
                .ok_or(S7FederationContractError::Invalid)?;
            if field_id != expected_field_id(&operation_id, "response", &response_status, &pointer)
            {
                return Err(S7FederationContractError::Invalid);
            }
            fields.push((field_id, pointer, required_presence));
        }
        operations.push(S7FederatedOperation {
            operation_id,
            method,
            path_template,
            explicit_operation_id,
            response_status,
            fields,
        });
    }

    let client_values = array(root, "clients")?;
    cardinality(
        "linked_clients",
        S7Limit::LinkedClients.maximum(),
        client_values.len(),
    )?;
    if !sorted_by_pair(client_values, "client_id", "call_site_id") {
        return Err(S7FederationContractError::Invalid);
    }
    let mut clients = BTreeMap::new();
    for client in client_values {
        let client = exact_object(
            client,
            &[
                "binding_kind",
                "call_site_id",
                "client_id",
                "declaration_path",
                "evidence_ids",
                "operation_candidate_id",
                "repository_identity",
                "revision",
                "role",
            ],
        )
        .ok_or(S7FederationContractError::Invalid)?;
        let repository_identity = required(client, "repository_identity")?.to_owned();
        let client_id = required(client, "client_id")?.to_owned();
        let call_site_id = required(client, "call_site_id")?.to_owned();
        if client_id != expected_client_id(&repository_identity)
            || !matches!(
                required(client, "binding_kind")?,
                "explicit_operation_identity" | "heuristic_name"
            )
            || !valid_safe_path(required(client, "declaration_path")?)
            || clients
                .insert(
                    (client_id.clone(), call_site_id.clone()),
                    S7FederatedClient {
                        role: required(client, "role")?.to_owned(),
                        repository_identity,
                        revision: required(client, "revision")?.to_owned(),
                        client_id,
                        call_site_id,
                        state: S7FederationState::Unresolved,
                    },
                )
                .is_some()
        {
            return Err(S7FederationContractError::Invalid);
        }
    }
    cardinality("call_sites", S7Limit::CallSites.maximum(), clients.len())?;
    let links = array(root, "confirmed_links")?;
    if !sorted_by(links, "link_id") {
        return Err(S7FederationContractError::Invalid);
    }
    for link in links {
        let link = exact_object(
            link,
            &[
                "authority",
                "call_site_id",
                "client_id",
                "evidence_ids",
                "link_id",
                "operation_id",
                "rule_id",
                "state",
            ],
        )
        .ok_or(S7FederationContractError::Invalid)?;
        let operation_id = required(link, "operation_id")?.to_owned();
        let client_id = required(link, "client_id")?;
        let call_site_id = required(link, "call_site_id")?;
        let client = clients
            .get_mut(&(client_id.to_owned(), call_site_id.to_owned()))
            .ok_or(S7FederationContractError::Invalid)?;
        if !operation_ids.contains(&operation_id)
            || required(link, "authority")? != "explicit_workspace_identity"
            || required(link, "rule_id")? != "fed.explicit-operation.confirm/v1"
            || required(link, "state")? != "confirmed"
            || required(link, "link_id")?
                != confirmed_link_id(
                    &operation_id,
                    client_id,
                    call_site_id,
                    "explicit_operation_identity",
                )
            || !matches!(client.state, S7FederationState::Unresolved)
        {
            return Err(S7FederationContractError::Invalid);
        }
        client.state = S7FederationState::Confirmed { operation_id };
    }
    let rejections = array(root, "rejections")?;
    if !sorted_by(rejections, "rejection_id") {
        return Err(S7FederationContractError::Invalid);
    }
    for rejection in rejections {
        let rejection = exact_object(
            rejection,
            &[
                "call_site_id",
                "client_id",
                "evidence_ids",
                "operation_candidate_id",
                "reason_code",
                "rejection_id",
                "rule_id",
                "state",
            ],
        )
        .ok_or(S7FederationContractError::Invalid)?;
        let client_id = required(rejection, "client_id")?;
        let call_site_id = required(rejection, "call_site_id")?;
        let candidate = required(rejection, "operation_candidate_id")?.to_owned();
        let reason = required(rejection, "reason_code")?;
        let client = clients
            .get_mut(&(client_id.to_owned(), call_site_id.to_owned()))
            .ok_or(S7FederationContractError::Invalid)?;
        if required(rejection, "rejection_id")?
            != expected_rejection_id(&candidate, client_id, call_site_id, reason)
            || required(rejection, "rule_id")? != "fed.operation-decoy.reject/v1"
            || required(rejection, "state")? != "rejected"
            || !matches!(client.state, S7FederationState::Unresolved)
        {
            return Err(S7FederationContractError::Invalid);
        }
        client.state = S7FederationState::Rejected {
            operation_candidate_id: candidate,
        };
    }

    validate_s6_evidence_and_references(root, &operation_ids, &clients)?;

    Ok(S7FederationAuthority {
        provider_repository_identity,
        provider_revision,
        service_authority,
        service_id,
        operations,
        clients: clients.into_values().collect(),
    })
}

#[derive(Clone, Debug)]
pub struct SemanticCompatibilityReportV1 {
    value: PrettyValue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticReportError {
    Invalid,
    Serialization,
    LimitExceeded(S7LimitExceeded),
}

impl SemanticCompatibilityReportV1 {
    /// Builds and validates the public report from reconciled S7 domain facts.
    ///
    /// # Errors
    ///
    /// Returns a closed identity, ordering, reference, or report-limit failure.
    pub fn from_domain(report: &SemanticCompatibilityReport) -> Result<Self, SemanticReportError> {
        validate_semantic_report(report)?;
        Ok(Self {
            value: semantic_report_value(report),
        })
    }

    /// Serializes one deterministic two-space JSON document plus LF.
    ///
    /// # Errors
    ///
    /// Returns a serialization or exact report-size failure before publication.
    pub fn canonical_stdout(&self) -> Result<Vec<u8>, SemanticReportError> {
        let mut output = String::new();
        self.value.write(&mut output, 0)?;
        output.push('\n');
        let observed = u64::try_from(output.len()).unwrap_or(u64::MAX);
        let maximum = S7Limit::ReportBytes.maximum();
        if observed > maximum {
            return Err(SemanticReportError::LimitExceeded(S7LimitExceeded {
                limit: S7Limit::ReportBytes,
                maximum,
                observed,
            }));
        }
        Ok(output.into_bytes())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImpactWorkspaceV1 {
    pub provider: ImpactProviderInput,
    pub clients: Vec<ImpactClientInput>,
    pub federation_report: ImpactBoundFile,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImpactProviderInput {
    pub repository_identity: String,
    pub baseline: ImpactRevisionInput,
    pub target: ImpactRevisionInput,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImpactRevisionInput {
    pub revision: String,
    pub root: String,
    pub contract: ImpactBoundFile,
    pub source: ImpactBoundFile,
    pub callable_symbol: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImpactClientInput {
    pub role: String,
    pub repository_identity: String,
    pub revision: String,
    pub root: String,
    pub source: ImpactBoundFile,
    pub decoder_symbol: String,
    pub call_symbol: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImpactBoundFile {
    pub path: String,
    pub sha256: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImpactWorkspaceError {
    Invalid,
    TooManyClients {
        observed: u64,
    },
    LimitExceeded {
        limit: &'static str,
        maximum: u64,
        observed: u64,
    },
}

/// Parses the closed `ImpactWorkspaceV1` authority document.
///
/// # Errors
///
/// Returns a typed failure for malformed, unknown, unsafe, duplicate, or
/// over-limit workspace input.
pub fn parse_impact_workspace(bytes: &[u8]) -> Result<ImpactWorkspaceV1, ImpactWorkspaceError> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| ImpactWorkspaceError::Invalid)?;
    let object = exact_object(
        &value,
        &[
            "schema_version",
            "analysis_profile",
            "pipeline",
            "contract_capability",
            "provider_capability",
            "client_capability",
            "provider",
            "clients",
            "federation_report",
        ],
    )
    .ok_or(ImpactWorkspaceError::Invalid)?;
    require_const(object, "schema_version", IMPACT_WORKSPACE_SCHEMA)?;
    require_const(object, "analysis_profile", IMPACT_ANALYSIS_PROFILE)?;
    require_const(object, "pipeline", IMPACT_PIPELINE)?;
    require_const(object, "contract_capability", IMPACT_CONTRACT_CAPABILITY)?;
    require_const(object, "provider_capability", IMPACT_PROVIDER_CAPABILITY)?;
    require_const(object, "client_capability", IMPACT_CLIENT_CAPABILITY)?;

    let provider = parse_provider(
        object
            .get("provider")
            .ok_or(ImpactWorkspaceError::Invalid)?,
    )?;
    let clients_value = object
        .get("clients")
        .and_then(Value::as_array)
        .filter(|clients| !clients.is_empty())
        .ok_or(ImpactWorkspaceError::Invalid)?;
    let observed = u64::try_from(clients_value.len()).unwrap_or(u64::MAX);
    if observed > 10_000 {
        return Err(ImpactWorkspaceError::TooManyClients { observed });
    }
    let clients = clients_value
        .iter()
        .map(parse_client)
        .collect::<Result<Vec<_>, _>>()?;
    let mut repositories = BTreeSet::new();
    for client in &clients {
        if client.repository_identity == provider.repository_identity
            || !repositories.insert(client.repository_identity.as_str())
        {
            return Err(ImpactWorkspaceError::Invalid);
        }
    }
    let federation_report = parse_bound_file(
        object
            .get("federation_report")
            .ok_or(ImpactWorkspaceError::Invalid)?,
    )?;
    Ok(ImpactWorkspaceV1 {
        provider,
        clients,
        federation_report,
    })
}

#[derive(Clone, Debug)]
pub struct CodeNoesisErrorV23 {
    value: Value,
}

impl CodeNoesisErrorV23 {
    #[must_use]
    pub fn invalid_workspace(reason: &str) -> Self {
        Self::new(
            "impact.invalid_workspace",
            "input",
            "invalid impact workspace",
            context([
                ("component", Value::String("workspace".to_owned())),
                ("reason", Value::String(reason.to_owned())),
            ]),
        )
    }

    #[must_use]
    pub fn invalid_workspace_path(path: &str, component: &str, reason: &str) -> Self {
        Self::new(
            "impact.invalid_workspace",
            "input",
            "invalid authorized impact input",
            context([
                ("component", Value::String(component.to_owned())),
                ("path", Value::String(path.to_owned())),
                ("reason", Value::String(reason.to_owned())),
            ]),
        )
    }

    #[must_use]
    pub fn invalid_federation_report(path: &str, reason: &str) -> Self {
        Self::new(
            "impact.invalid_federation_report",
            "impact",
            "invalid federation report",
            context([
                ("component", Value::String("federation_report".to_owned())),
                ("path", Value::String(path.to_owned())),
                ("reason", Value::String(reason.to_owned())),
            ]),
        )
    }

    #[must_use]
    pub fn unsupported_implementation_semantics() -> Self {
        Self::new(
            "impact.unsupported_implementation_semantics",
            "impact",
            "implementation-aware source semantics are not available",
            context([
                ("component", Value::String("semantic_report".to_owned())),
                (
                    "reason",
                    Value::String("capability_not_implemented".to_owned()),
                ),
            ]),
        )
    }

    #[must_use]
    pub fn limit(limit: &str, maximum: u64, observed: u64) -> Self {
        Self::new(
            "impact.limit_exceeded",
            "impact",
            "S7 resource limit exceeded",
            context([
                ("limit", Value::String(limit.to_owned())),
                ("maximum", Value::from(maximum)),
                ("observed", Value::from(observed)),
            ]),
        )
    }

    #[must_use]
    pub fn mutable_input(component: &str) -> Self {
        Self::new(
            "impact.mutable_input",
            "impact",
            "authorized impact input changed during validation",
            context([
                ("component", Value::String(component.to_owned())),
                ("reason", Value::String("input_replaced".to_owned())),
            ]),
        )
    }

    #[must_use]
    pub fn internal() -> Self {
        Self::new(
            "impact.internal",
            "internal",
            "unexpected impact failure",
            context([("component", Value::String("semantic_report".to_owned()))]),
        )
    }

    /// Serializes one strict canonical `ErrorV23` plus LF.
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

    fn new(code: &str, stage: &str, message: &str, context: Map<String, Value>) -> Self {
        Self {
            value: json!({
                "schema_version": IMPACT_ERROR_SCHEMA,
                "code": code,
                "stage": stage,
                "message": message,
                "retryable": false,
                "context": Value::Object(context)
            }),
        }
    }
}

fn parse_provider(value: &Value) -> Result<ImpactProviderInput, ImpactWorkspaceError> {
    let object = exact_object(value, &["repository_identity", "baseline", "target"])
        .ok_or(ImpactWorkspaceError::Invalid)?;
    let repository_identity = value_string(object, "repository_identity", valid_identity)?;
    let baseline = parse_revision(
        object
            .get("baseline")
            .ok_or(ImpactWorkspaceError::Invalid)?,
    )?;
    let target = parse_revision(object.get("target").ok_or(ImpactWorkspaceError::Invalid)?)?;
    if baseline.revision == target.revision {
        return Err(ImpactWorkspaceError::Invalid);
    }
    Ok(ImpactProviderInput {
        repository_identity,
        baseline,
        target,
    })
}

fn parse_revision(value: &Value) -> Result<ImpactRevisionInput, ImpactWorkspaceError> {
    let object = exact_object(
        value,
        &["revision", "root", "contract", "source", "callable_symbol"],
    )
    .ok_or(ImpactWorkspaceError::Invalid)?;
    Ok(ImpactRevisionInput {
        revision: value_string(object, "revision", valid_revision)?,
        root: path_string(object, "root")?,
        contract: parse_bound_file(
            object
                .get("contract")
                .ok_or(ImpactWorkspaceError::Invalid)?,
        )?,
        source: parse_bound_file(object.get("source").ok_or(ImpactWorkspaceError::Invalid)?)?,
        callable_symbol: symbol_string(object, "callable_symbol")?,
    })
}

fn parse_client(value: &Value) -> Result<ImpactClientInput, ImpactWorkspaceError> {
    let object = exact_object(
        value,
        &[
            "role",
            "repository_identity",
            "revision",
            "root",
            "source",
            "decoder_symbol",
            "call_symbol",
        ],
    )
    .ok_or(ImpactWorkspaceError::Invalid)?;
    Ok(ImpactClientInput {
        role: value_string(object, "role", |role| {
            matches!(role, "decoy" | "safe" | "strict")
        })?,
        repository_identity: value_string(object, "repository_identity", valid_identity)?,
        revision: value_string(object, "revision", valid_revision)?,
        root: path_string(object, "root")?,
        source: parse_bound_file(object.get("source").ok_or(ImpactWorkspaceError::Invalid)?)?,
        decoder_symbol: symbol_string(object, "decoder_symbol")?,
        call_symbol: symbol_string(object, "call_symbol")?,
    })
}

fn parse_bound_file(value: &Value) -> Result<ImpactBoundFile, ImpactWorkspaceError> {
    let object = exact_object(value, &["path", "sha256"]).ok_or(ImpactWorkspaceError::Invalid)?;
    Ok(ImpactBoundFile {
        path: path_string(object, "path")?,
        sha256: value_string(object, "sha256", valid_digest)?,
    })
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
) -> Result<(), ImpactWorkspaceError> {
    (object.get(key).and_then(Value::as_str) == Some(expected))
        .then_some(())
        .ok_or(ImpactWorkspaceError::Invalid)
}

fn value_string(
    object: &Map<String, Value>,
    key: &str,
    validator: impl FnOnce(&str) -> bool,
) -> Result<String, ImpactWorkspaceError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| validator(value))
        .map(str::to_owned)
        .ok_or(ImpactWorkspaceError::Invalid)
}

fn path_string(object: &Map<String, Value>, key: &str) -> Result<String, ImpactWorkspaceError> {
    bounded_string(
        object,
        key,
        S7Limit::LogicalPathBytes.as_str(),
        S7Limit::LogicalPathBytes.maximum(),
        valid_safe_path,
    )
}

fn symbol_string(object: &Map<String, Value>, key: &str) -> Result<String, ImpactWorkspaceError> {
    bounded_string(
        object,
        key,
        S7Limit::CallableSymbolBytes.as_str(),
        S7Limit::CallableSymbolBytes.maximum(),
        valid_symbol,
    )
}

fn bounded_string(
    object: &Map<String, Value>,
    key: &str,
    limit: &'static str,
    maximum: u64,
    validator: impl FnOnce(&str) -> bool,
) -> Result<String, ImpactWorkspaceError> {
    let value = object
        .get(key)
        .and_then(Value::as_str)
        .ok_or(ImpactWorkspaceError::Invalid)?;
    let observed = u64::try_from(value.len()).unwrap_or(u64::MAX);
    if observed > maximum {
        return Err(ImpactWorkspaceError::LimitExceeded {
            limit,
            maximum,
            observed,
        });
    }
    validator(value)
        .then(|| value.to_owned())
        .ok_or(ImpactWorkspaceError::Invalid)
}

fn valid_identity(value: &str) -> bool {
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

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn valid_symbol(value: &str) -> bool {
    !value.is_empty() && !value.contains(['\0', '\r', '\n'])
}

fn context<'a>(values: impl IntoIterator<Item = (&'a str, Value)>) -> Map<String, Value> {
    values
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value))
        .collect()
}

fn validate_s6_semantic_hash(value: &Value) -> Result<(), S7FederationContractError> {
    let expected = value
        .get("semantic_hash")
        .and_then(Value::as_str)
        .ok_or(S7FederationContractError::Invalid)?;
    let mut projection = value.clone();
    let root = projection
        .as_object_mut()
        .ok_or(S7FederationContractError::Invalid)?;
    root.remove("semantic_hash");
    root.remove("evidence");
    let provider = root
        .get_mut("provider")
        .and_then(Value::as_object_mut)
        .ok_or(S7FederationContractError::Invalid)?;
    provider.remove("contract_path");
    provider.remove("contract_sha256");
    provider.remove("source_format");
    for client in root
        .get_mut("clients")
        .and_then(Value::as_array_mut)
        .ok_or(S7FederationContractError::Invalid)?
    {
        client
            .as_object_mut()
            .ok_or(S7FederationContractError::Invalid)?
            .remove("declaration_path");
    }
    remove_evidence_ids(&mut projection);
    let canonical =
        serde_json::to_vec(&projection).map_err(|_| S7FederationContractError::Invalid)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(S6_SEMANTIC_HASH_DOMAIN);
    hasher.update(&[0]);
    hasher.update(&canonical);
    if expected != format!("blake3:{}", hasher.finalize().to_hex()) {
        return Err(S7FederationContractError::Invalid);
    }
    Ok(())
}

fn validate_s6_limits(root: &Map<String, Value>) -> Result<(), S7FederationContractError> {
    let limits = root
        .get("limits")
        .and_then(Value::as_object)
        .ok_or(S7FederationContractError::Invalid)?;
    let expected = [
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
    if limits.len() != expected.len()
        || expected.iter().any(|limit| {
            limits.get(limit.as_str()).and_then(Value::as_u64) != Some(limit.maximum())
        })
    {
        return Err(S7FederationContractError::Invalid);
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn validate_s6_evidence_and_references(
    root: &Map<String, Value>,
    operation_ids: &BTreeSet<String>,
    clients: &BTreeMap<(String, String), S7FederatedClient>,
) -> Result<(), S7FederationContractError> {
    let evidence_values = array(root, "evidence")?;
    cardinality(
        "evidence_items",
        S7Limit::EvidenceItems.maximum(),
        evidence_values.len(),
    )?;
    if !sorted_by(evidence_values, "evidence_id") {
        return Err(S7FederationContractError::Invalid);
    }
    let mut evidence_ids = BTreeSet::new();
    for evidence in evidence_values {
        let evidence = exact_object(
            evidence,
            &[
                "evidence_id",
                "file_sha256",
                "kind",
                "path",
                "repository_identity",
                "revision",
                "selector",
            ],
        )
        .ok_or(S7FederationContractError::Invalid)?;
        let evidence_id = required(evidence, "evidence_id")?;
        let kind = required(evidence, "kind")?;
        let repository = required(evidence, "repository_identity")?;
        let revision = required(evidence, "revision")?;
        let path = required(evidence, "path")?;
        let digest = required(evidence, "file_sha256")?;
        if !valid_identity(repository)
            || !valid_revision(revision)
            || !valid_safe_path(path)
            || !valid_digest(digest)
        {
            return Err(S7FederationContractError::Invalid);
        }
        let selector_value = evidence
            .get("selector")
            .ok_or(S7FederationContractError::Invalid)?;
        let expected = match kind {
            "openapi_yaml_span" => {
                let selector = exact_object(
                    selector_value,
                    &["end_line", "kind", "location", "start_line"],
                )
                .ok_or(S7FederationContractError::Invalid)?;
                if required(selector, "kind")? != "openapi_location_span" {
                    return Err(S7FederationContractError::Invalid);
                }
                let start_line = selector
                    .get("start_line")
                    .and_then(Value::as_u64)
                    .filter(|line| *line > 0)
                    .ok_or(S7FederationContractError::Invalid)?;
                let end_line = selector
                    .get("end_line")
                    .and_then(Value::as_u64)
                    .filter(|line| *line >= start_line)
                    .ok_or(S7FederationContractError::Invalid)?;
                yaml_evidence_id(
                    repository,
                    revision,
                    path,
                    required(selector, "location")?,
                    start_line,
                    end_line,
                    digest,
                )
            }
            "openapi_json_pointer" | "workspace_json_pointer" => {
                let selector = exact_object(selector_value, &["kind", "pointer"])
                    .ok_or(S7FederationContractError::Invalid)?;
                if required(selector, "kind")? != "json_pointer" {
                    return Err(S7FederationContractError::Invalid);
                }
                json_evidence_id(
                    repository,
                    revision,
                    path,
                    required(selector, "pointer")?,
                    digest,
                )
            }
            _ => return Err(S7FederationContractError::Invalid),
        };
        if evidence_id != expected || !evidence_ids.insert(evidence_id) {
            return Err(S7FederationContractError::Invalid);
        }
    }

    let gaps = array(root, "coverage_gaps")?;
    cardinality("coverage_gaps", S7Limit::CoverageGaps.maximum(), gaps.len())?;
    if !sorted_by(gaps, "coverage_gap_id") {
        return Err(S7FederationContractError::Invalid);
    }
    let mut gap_ids = BTreeSet::new();
    for gap in gaps {
        let gap = exact_object(
            gap,
            &[
                "coverage_gap_id",
                "evidence_ids",
                "reason_code",
                "state",
                "subject_id",
            ],
        )
        .ok_or(S7FederationContractError::Invalid)?;
        let ids = string_array(gap, "evidence_ids")?;
        let [evidence_id] = ids.as_slice() else {
            return Err(S7FederationContractError::Invalid);
        };
        let gap_id = required(gap, "coverage_gap_id")?;
        if required(gap, "state")? != "unresolved"
            || gap_id
                != heuristic_gap_id(
                    required(gap, "subject_id")?,
                    required(gap, "reason_code")?,
                    evidence_id,
                )
            || !gap_ids.insert(gap_id)
        {
            return Err(S7FederationContractError::Invalid);
        }
    }

    let candidates = array(root, "candidates")?;
    if !sorted_by(candidates, "candidate_id") {
        return Err(S7FederationContractError::Invalid);
    }
    for candidate in candidates {
        let candidate = exact_object(
            candidate,
            &[
                "authority",
                "call_site_id",
                "candidate_id",
                "client_id",
                "coverage_gap_id",
                "evidence_ids",
                "operation_hint",
                "rule_id",
                "service_hint",
                "state",
                "target_operation_id",
            ],
        )
        .ok_or(S7FederationContractError::Invalid)?;
        let target = required(candidate, "target_operation_id")?;
        let client_id = required(candidate, "client_id")?;
        let call_site_id = required(candidate, "call_site_id")?;
        if !operation_ids.contains(target)
            || !clients.contains_key(&(client_id.to_owned(), call_site_id.to_owned()))
            || !gap_ids.contains(required(candidate, "coverage_gap_id")?)
            || required(candidate, "authority")? != "heuristic_candidate"
            || required(candidate, "rule_id")? != "fed.heuristic-name.candidate/v1"
            || required(candidate, "state")? != "candidate"
            || required(candidate, "candidate_id")?
                != heuristic_candidate_id(
                    target,
                    client_id,
                    call_site_id,
                    required(candidate, "service_hint")?,
                    required(candidate, "operation_hint")?,
                )
        {
            return Err(S7FederationContractError::Invalid);
        }
    }

    validate_all_s6_evidence_references(&Value::Object(root.clone()), &evidence_ids)
}

fn validate_all_s6_evidence_references(
    value: &Value,
    evidence_ids: &BTreeSet<&str>,
) -> Result<(), S7FederationContractError> {
    match value {
        Value::Object(object) => {
            if object.contains_key("evidence_ids") {
                let ids = string_array(object, "evidence_ids")?;
                if ids.iter().any(|id| !evidence_ids.contains(id.as_str())) {
                    return Err(S7FederationContractError::Invalid);
                }
            }
            for child in object.values() {
                validate_all_s6_evidence_references(child, evidence_ids)?;
            }
        }
        Value::Array(values) => {
            for child in values {
                validate_all_s6_evidence_references(child, evidence_ids)?;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    Ok(())
}

fn string_array(
    object: &Map<String, Value>,
    key: &str,
) -> Result<Vec<String>, S7FederationContractError> {
    let values = array(object, key)?;
    let ids = values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or(S7FederationContractError::Invalid)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if !ordered_unique(ids.iter().map(String::as_str)) {
        return Err(S7FederationContractError::Invalid);
    }
    Ok(ids)
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

fn required<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a str, S7FederationContractError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or(S7FederationContractError::Invalid)
}

fn array<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a [Value], S7FederationContractError> {
    object
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or(S7FederationContractError::Invalid)
}

fn cardinality(
    limit: &'static str,
    maximum: u64,
    observed: usize,
) -> Result<(), S7FederationContractError> {
    let observed = u64::try_from(observed).unwrap_or(u64::MAX);
    if observed > maximum {
        return Err(S7FederationContractError::LimitExceeded { limit, observed });
    }
    Ok(())
}

fn sorted_by(values: &[Value], key: &str) -> bool {
    ordered_unique(
        values
            .iter()
            .filter_map(|value| value.get(key).and_then(Value::as_str)),
    ) && values
        .iter()
        .all(|value| value.get(key).and_then(Value::as_str).is_some())
}

fn sorted_by_pair(values: &[Value], first: &str, second: &str) -> bool {
    let pairs = values
        .iter()
        .filter_map(|value| Some((value.get(first)?.as_str()?, value.get(second)?.as_str()?)))
        .collect::<Vec<_>>();
    pairs.len() == values.len() && pairs.windows(2).all(|pair| pair[0] < pair[1])
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

fn parse_http_method(value: &str) -> Option<HttpMethod> {
    match value {
        "DELETE" => Some(HttpMethod::Delete),
        "GET" => Some(HttpMethod::Get),
        "PATCH" => Some(HttpMethod::Patch),
        "POST" => Some(HttpMethod::Post),
        "PUT" => Some(HttpMethod::Put),
        _ => None,
    }
}

#[allow(clippy::too_many_lines)]
fn validate_semantic_report(
    report: &SemanticCompatibilityReport,
) -> Result<(), SemanticReportError> {
    if !ordered_unique(report.semantic_diffs.iter().map(|diff| diff.id.as_str()))
        || !ordered_unique(
            report
                .client_assessments
                .iter()
                .map(|assessment| assessment.client_identity.as_str()),
        )
        || !ordered_unique(
            report
                .rejected_candidates
                .iter()
                .map(|candidate| candidate.client_identity.as_str()),
        )
        || !ordered_unique(report.evidence.iter().map(|evidence| evidence.id.as_str()))
        || !ordered_unique(report.coverage_gaps.iter().map(|gap| gap.id.as_str()))
    {
        return Err(SemanticReportError::Invalid);
    }
    for (limit, observed) in [
        (S7Limit::SemanticDiffs, report.semantic_diffs.len()),
        (S7Limit::LinkedClients, report.client_assessments.len()),
        (S7Limit::EvidenceItems, report.evidence.len()),
        (S7Limit::CoverageGaps, report.coverage_gaps.len()),
    ] {
        let observed = u64::try_from(observed).unwrap_or(u64::MAX);
        let maximum = limit.maximum();
        if observed > maximum {
            return Err(SemanticReportError::LimitExceeded(S7LimitExceeded {
                limit,
                maximum,
                observed,
            }));
        }
    }
    let evidence = report
        .evidence
        .iter()
        .map(|item| (item.id.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    for item in &report.evidence {
        if item.id
            != codenoesis_domain::s7::evidence_id(
                &item.repository_identity,
                &item.revision,
                &item.path,
                item.start_line,
                item.end_line,
                &item.excerpt_sha256,
            )
            || item.start_line == 0
            || item.end_line < item.start_line
            || !valid_digest(&item.excerpt_sha256)
            || !valid_safe_path(&item.path)
        {
            return Err(SemanticReportError::Invalid);
        }
    }
    let assessments = report
        .client_assessments
        .iter()
        .map(|assessment| (assessment.client_identity.as_str(), assessment))
        .collect::<BTreeMap<_, _>>();
    let gap_ids = report
        .coverage_gaps
        .iter()
        .map(|gap| gap.id.as_str())
        .collect::<BTreeSet<_>>();
    for diff in &report.semantic_diffs {
        if diff.id
            != codenoesis_domain::s7::diff_id(
                &report.provider.repository_identity,
                &report.provider.baseline_revision,
                &report.provider.target_revision,
                &diff.field_id,
                "presence",
            )
            || !ordered_unique(diff.affected_client_ids.iter().map(String::as_str))
            || !ordered_unique(diff.evidence_ids.iter().map(String::as_str))
            || !ordered_unique(diff.coverage_gap_ids.iter().map(String::as_str))
            || diff
                .affected_client_ids
                .iter()
                .any(|id| !assessments.contains_key(id.as_str()))
            || diff
                .coverage_gap_ids
                .iter()
                .any(|id| !gap_ids.contains(id.as_str()))
        {
            return Err(SemanticReportError::Invalid);
        }
        validate_evidence_ids(&diff.contract.evidence_ids, &evidence)?;
        validate_evidence_ids(&diff.implementation.evidence_ids, &evidence)?;
        validate_evidence_ids(&diff.evidence_ids, &evidence)?;
    }
    for assessment in &report.client_assessments {
        let client_evidence = assessment
            .evidence_ids
            .iter()
            .filter_map(|id| evidence.get(id.as_str()).copied())
            .find(|item| {
                item.source_kind == codenoesis_domain::s7::EvidenceSourceKind::ClientAssumption
                    && item.repository_identity == assessment.repository_identity
            })
            .ok_or(SemanticReportError::Invalid)?;
        if assessment.client_identity != expected_client_id(&assessment.repository_identity)
            || assessment.call_site_id
                != expected_call_site_id(
                    &assessment.client_identity,
                    &client_evidence.revision,
                    &client_evidence.path,
                    &assessment.call_symbol,
                )
            || !ordered_unique(assessment.evidence_ids.iter().map(String::as_str))
            || !ordered_unique(assessment.coverage_gap_ids.iter().map(String::as_str))
        {
            return Err(SemanticReportError::Invalid);
        }
        validate_evidence_ids(&assessment.evidence_ids, &evidence)?;
    }
    for candidate in &report.rejected_candidates {
        let client_evidence = candidate
            .evidence_ids
            .iter()
            .filter_map(|id| evidence.get(id.as_str()).copied())
            .find(|item| {
                item.source_kind == codenoesis_domain::s7::EvidenceSourceKind::ClientAssumption
                    && item.repository_identity == candidate.repository_identity
            })
            .ok_or(SemanticReportError::Invalid)?;
        if candidate.client_identity != expected_client_id(&candidate.repository_identity)
            || candidate.call_site_id
                != expected_call_site_id(
                    &candidate.client_identity,
                    &client_evidence.revision,
                    &client_evidence.path,
                    &candidate.call_symbol,
                )
            || !ordered_unique(candidate.evidence_ids.iter().map(String::as_str))
        {
            return Err(SemanticReportError::Invalid);
        }
        validate_evidence_ids(&candidate.evidence_ids, &evidence)?;
    }
    for gap in &report.coverage_gaps {
        if gap.id
            != codenoesis_domain::s7::coverage_gap_id(
                &gap.subject_id,
                "unsupported_custom_provider_mapping",
                &report.provider.baseline_revision,
                &report.provider.target_revision,
            )
            || gap.revisions
                != [
                    report.provider.baseline_revision.clone(),
                    report.provider.target_revision.clone(),
                ]
            || !ordered_unique(gap.evidence_ids.iter().map(String::as_str))
        {
            return Err(SemanticReportError::Invalid);
        }
        validate_evidence_ids(&gap.evidence_ids, &evidence)?;
    }
    let mut referenced = BTreeSet::new();
    for ids in report
        .semantic_diffs
        .iter()
        .flat_map(|diff| {
            [
                &diff.evidence_ids,
                &diff.contract.evidence_ids,
                &diff.implementation.evidence_ids,
            ]
        })
        .chain(
            report
                .client_assessments
                .iter()
                .map(|assessment| &assessment.evidence_ids),
        )
        .chain(
            report
                .rejected_candidates
                .iter()
                .map(|candidate| &candidate.evidence_ids),
        )
        .chain(report.coverage_gaps.iter().map(|gap| &gap.evidence_ids))
    {
        referenced.extend(ids.iter().map(String::as_str));
    }
    if referenced != evidence.keys().copied().collect() {
        return Err(SemanticReportError::Invalid);
    }
    Ok(())
}

fn validate_evidence_ids(
    ids: &[String],
    evidence: &BTreeMap<&str, &codenoesis_domain::s7::SourceEvidence>,
) -> Result<(), SemanticReportError> {
    if !ordered_unique(ids.iter().map(String::as_str))
        || ids.iter().any(|id| !evidence.contains_key(id.as_str()))
    {
        return Err(SemanticReportError::Invalid);
    }
    Ok(())
}

#[derive(Clone, Debug)]
enum PrettyValue {
    Object(Vec<(&'static str, PrettyValue)>),
    Array(Vec<PrettyValue>),
    String(String),
    Number(u64),
    Bool(bool),
}

impl PrettyValue {
    fn write(&self, output: &mut String, indent: usize) -> Result<(), SemanticReportError> {
        match self {
            Self::Object(fields) if fields.is_empty() => output.push_str("{}"),
            Self::Object(fields) => {
                output.push_str("{\n");
                for (index, (key, value)) in fields.iter().enumerate() {
                    write_indent(output, indent + 2);
                    output.push_str(
                        &serde_json::to_string(key)
                            .map_err(|_| SemanticReportError::Serialization)?,
                    );
                    output.push_str(": ");
                    value.write(output, indent + 2)?;
                    if index + 1 < fields.len() {
                        output.push(',');
                    }
                    output.push('\n');
                }
                write_indent(output, indent);
                output.push('}');
            }
            Self::Array(values) if values.is_empty() => output.push_str("[]"),
            Self::Array(values) => {
                output.push_str("[\n");
                for (index, value) in values.iter().enumerate() {
                    write_indent(output, indent + 2);
                    value.write(output, indent + 2)?;
                    if index + 1 < values.len() {
                        output.push(',');
                    }
                    output.push('\n');
                }
                write_indent(output, indent);
                output.push(']');
            }
            Self::String(value) => output.push_str(
                &serde_json::to_string(value).map_err(|_| SemanticReportError::Serialization)?,
            ),
            Self::Number(value) => {
                write!(output, "{value}").map_err(|_| SemanticReportError::Serialization)?;
            }
            Self::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
        }
        Ok(())
    }
}

fn write_indent(output: &mut String, indent: usize) {
    output.extend(std::iter::repeat_n(' ', indent));
}

#[allow(clippy::too_many_lines)]
fn semantic_report_value(report: &SemanticCompatibilityReport) -> PrettyValue {
    object(vec![
        ("schema_version", string(REPORT_SCHEMA)),
        ("analysis_profile", string(ANALYSIS_PROFILE)),
        ("configuration_hash", string(CONFIGURATION_HASH)),
        ("pipeline_version", string(REPORT_PIPELINE)),
        ("ontology_versions", strings([ONTOLOGY_VERSION])),
        (
            "extractor_versions",
            array_value(
                [
                    ("kotlin-direct-json-access", "v1"),
                    ("openapi-contract", "v1"),
                    ("rust-direct-json-map", "v1"),
                    ("semantic-impact-classifier", "v1"),
                ]
                .into_iter()
                .map(|(name, version)| {
                    object(vec![("name", string(name)), ("version", string(version))])
                }),
            ),
        ),
        ("evidence_lineage_version", string(EVIDENCE_LINEAGE_VERSION)),
        ("rule_catalog_version", string(RULE_CATALOG_VERSION)),
        (
            "provider",
            object(vec![
                (
                    "service_identity",
                    string(&report.provider.service_identity),
                ),
                (
                    "repository_identity",
                    string(&report.provider.repository_identity),
                ),
                (
                    "baseline",
                    object(vec![
                        ("revision", string(&report.provider.baseline_revision)),
                        (
                            "contract_sha256",
                            string(&report.provider.baseline_contract_sha256),
                        ),
                    ]),
                ),
                (
                    "target",
                    object(vec![
                        ("revision", string(&report.provider.target_revision)),
                        (
                            "contract_sha256",
                            string(&report.provider.target_contract_sha256),
                        ),
                    ]),
                ),
            ]),
        ),
        (
            "semantic_diffs",
            array_value(report.semantic_diffs.iter().map(|diff| {
                object(vec![
                    ("id", string(&diff.id)),
                    ("operation_id", string(&diff.operation_id)),
                    ("field_id", string(&diff.field_id)),
                    ("field_pointer", string(&diff.field_pointer)),
                    ("direction", string("response")),
                    ("dimension", string("presence")),
                    ("contract", view_delta(&diff.contract)),
                    ("implementation", view_delta(&diff.implementation)),
                    ("change_kind", string(diff.change_kind)),
                    ("classification", string(diff.classification)),
                    ("claim_state", string("derived_fact")),
                    ("rule_id", string(diff.rule_id)),
                    ("affected_client_ids", strings(&diff.affected_client_ids)),
                    ("evidence_ids", strings(&diff.evidence_ids)),
                    ("coverage_gap_ids", strings(&diff.coverage_gap_ids)),
                ])
            })),
        ),
        (
            "client_assessments",
            array_value(report.client_assessments.iter().map(|assessment| {
                object(vec![
                    ("client_identity", string(&assessment.client_identity)),
                    (
                        "repository_identity",
                        string(&assessment.repository_identity),
                    ),
                    ("operation_id", string(&assessment.operation_id)),
                    ("call_site_id", string(&assessment.call_site_id)),
                    ("link_state", string("deterministic_fact")),
                    (
                        "presence_assumption",
                        string(assessment.presence_assumption.as_str()),
                    ),
                    ("assumption_claim_state", string("derived_fact")),
                    ("baseline_risk", string(assessment.baseline_risk)),
                    ("target_impact", string(assessment.target_impact)),
                    ("affected", PrettyValue::Bool(assessment.affected)),
                    ("rule_ids", strings(&assessment.rule_ids)),
                    ("evidence_ids", strings(&assessment.evidence_ids)),
                    ("coverage_gap_ids", strings(&assessment.coverage_gap_ids)),
                ])
            })),
        ),
        (
            "rejected_candidates",
            array_value(report.rejected_candidates.iter().map(|candidate| {
                object(vec![
                    ("client_identity", string(&candidate.client_identity)),
                    (
                        "repository_identity",
                        string(&candidate.repository_identity),
                    ),
                    ("call_site_id", string(&candidate.call_site_id)),
                    ("reason_code", string("operation_identity_mismatch")),
                    ("evidence_ids", strings(&candidate.evidence_ids)),
                ])
            })),
        ),
        (
            "evidence",
            array_value(report.evidence.iter().map(|evidence| {
                object(vec![
                    ("id", string(&evidence.id)),
                    ("repository_identity", string(&evidence.repository_identity)),
                    ("revision", string(&evidence.revision)),
                    ("path", string(&evidence.path)),
                    ("start_line", PrettyValue::Number(evidence.start_line)),
                    ("end_line", PrettyValue::Number(evidence.end_line)),
                    ("excerpt_sha256", string(&evidence.excerpt_sha256)),
                    ("source_kind", string(evidence.source_kind.as_str())),
                    ("claim_state", string("deterministic_fact")),
                    ("capability_version", string(evidence.capability_version)),
                ])
            })),
        ),
        (
            "coverage_gaps",
            array_value(report.coverage_gaps.iter().map(|gap| {
                object(vec![
                    ("id", string(&gap.id)),
                    ("subject_id", string(&gap.subject_id)),
                    ("dimension", string("presence")),
                    ("reason_code", string("unsupported_custom_provider_mapping")),
                    ("revisions", strings(&gap.revisions)),
                    ("blocks_classification", PrettyValue::Bool(true)),
                    ("evidence_ids", strings(&gap.evidence_ids)),
                ])
            })),
        ),
    ])
}

fn view_delta(delta: &codenoesis_domain::s7::ViewDelta) -> PrettyValue {
    object(vec![
        ("before", string(delta.before)),
        ("after", string(delta.after)),
        ("delta", string(delta.delta)),
        ("claim_state", string(delta.claim_state)),
        ("evidence_ids", strings(&delta.evidence_ids)),
    ])
}

fn object(fields: Vec<(&'static str, PrettyValue)>) -> PrettyValue {
    PrettyValue::Object(fields)
}

fn array_value(values: impl IntoIterator<Item = PrettyValue>) -> PrettyValue {
    PrettyValue::Array(values.into_iter().collect())
}

fn string(value: impl AsRef<str>) -> PrettyValue {
    PrettyValue::String(value.as_ref().to_owned())
}

fn strings<T, S>(values: T) -> PrettyValue
where
    T: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    array_value(values.into_iter().map(string))
}
