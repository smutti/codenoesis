use std::collections::BTreeSet;

use serde_json::{Map, Value, json};

pub const IMPACT_WORKSPACE_SCHEMA: &str = "codenoesis.impact-workspace/v1";
pub const IMPACT_ANALYSIS_PROFILE: &str = "implementation-aware-http-json/v1";
pub const IMPACT_PIPELINE: &str = "codenoesis.pipeline/s7-v1";
pub const IMPACT_CONTRACT_CAPABILITY: &str =
    "codenoesis.contract-capability/openapi-3.1-http-json/v1";
pub const IMPACT_PROVIDER_CAPABILITY: &str = "rust-direct-json-map/v1";
pub const IMPACT_CLIENT_CAPABILITY: &str = "kotlin-direct-json-access/v1";
pub const IMPACT_ERROR_SCHEMA: &str = "codenoesis.error/v23";

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
    TooManyClients { observed: u64 },
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
        root: value_string(object, "root", valid_safe_path)?,
        contract: parse_bound_file(
            object
                .get("contract")
                .ok_or(ImpactWorkspaceError::Invalid)?,
        )?,
        source: parse_bound_file(object.get("source").ok_or(ImpactWorkspaceError::Invalid)?)?,
        callable_symbol: value_string(object, "callable_symbol", valid_symbol)?,
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
        root: value_string(object, "root", valid_safe_path)?,
        source: parse_bound_file(object.get("source").ok_or(ImpactWorkspaceError::Invalid)?)?,
        decoder_symbol: value_string(object, "decoder_symbol", valid_symbol)?,
        call_symbol: value_string(object, "call_symbol", valid_symbol)?,
    })
}

fn parse_bound_file(value: &Value) -> Result<ImpactBoundFile, ImpactWorkspaceError> {
    let object = exact_object(value, &["path", "sha256"]).ok_or(ImpactWorkspaceError::Invalid)?;
    Ok(ImpactBoundFile {
        path: value_string(object, "path", valid_safe_path)?,
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

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn valid_symbol(value: &str) -> bool {
    !value.is_empty() && value.len() <= 1024 && !value.contains(['\0', '\r', '\n'])
}

fn context<'a>(values: impl IntoIterator<Item = (&'a str, Value)>) -> Map<String, Value> {
    values
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value))
        .collect()
}
