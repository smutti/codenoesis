use std::error::Error;
use std::fmt::{self, Display, Formatter};

use serde_json::{Map, Value, json};

pub const RELEASE_PROFILE_V1_SCHEMA: &str = "codenoesis.release-profile/v1";
pub const RELEASE_PROFILE_REGISTRY_V1_SCHEMA: &str = "codenoesis.release-profile-registry/v1";
pub const CODENOESIS_ERROR_V25_SCHEMA: &str = "codenoesis.error/v25";
pub const LOCAL_EXPERIMENTAL_R17_PROFILE: &str = "local-experimental-r17";
pub const MAX_RELEASE_PROFILE_OUTPUT_BYTES: usize = 65_536;
pub const MAX_RELEASE_PROFILE_PLATFORMS: usize = 16;
pub const MAX_RELEASE_PROFILE_CAPABILITIES: usize = 64;
pub const MAX_RELEASE_PROFILE_EXCLUSIONS: usize = 64;
pub const MAX_RELEASE_PROFILE_LIMITATIONS: usize = 64;
pub const MAX_RELEASE_PROFILE_TEXT_BYTES: usize = 128;

const CAPABILITIES: [&str; 6] = [
    "local-acquisition-r2",
    "local-analysis-r16",
    "local-docs-query-r16",
    "local-portable-graph-v9",
    "local-function-context-v1",
    "local-explorer-v10",
];

const EXCLUDED_CAPABILITIES: [&str; 10] = [
    "incremental-refresh-s5",
    "federation-s6",
    "implementation-aware-impact-s7",
    "trusted-source-retrieval",
    "remote-acquisition",
    "compiler-index-generation",
    "model-provider",
    "server-runtime",
    "signed-distribution",
    "release-publication",
];

const LIMITATIONS: [&str; 7] = [
    "experimental_source_build_only",
    "not_verified",
    "no_support_window",
    "no_binary_distribution",
    "no_signature_or_attestation",
    "linux_only_normative_os_confinement",
    "no_ga_compatibility_promise",
];

#[derive(Clone, Copy)]
struct PlatformDefinition {
    target: &'static str,
    sandbox_tier: &'static str,
    normative_os_confinement: bool,
}

const PLATFORMS: [PlatformDefinition; 3] = [
    PlatformDefinition {
        target: "x86_64-unknown-linux-gnu",
        sandbox_tier: "normative-linux-seccomp-landlock-v1",
        normative_os_confinement: true,
    },
    PlatformDefinition {
        target: "aarch64-apple-darwin",
        sandbox_tier: "functional-portability-only-v1",
        normative_os_confinement: false,
    },
    PlatformDefinition {
        target: "x86_64-pc-windows-msvc",
        sandbox_tier: "functional-portability-only-v1",
        normative_os_confinement: false,
    },
];

#[cfg(all(target_arch = "x86_64", target_os = "linux", target_env = "gnu"))]
const CURRENT_COMPILE_TARGET: &str = "x86_64-unknown-linux-gnu";
#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
const CURRENT_COMPILE_TARGET: &str = "aarch64-apple-darwin";
#[cfg(all(target_arch = "x86_64", target_os = "windows", target_env = "msvc"))]
const CURRENT_COMPILE_TARGET: &str = "x86_64-pc-windows-msvc";
#[cfg(not(any(
    all(target_arch = "x86_64", target_os = "linux", target_env = "gnu"),
    all(target_arch = "aarch64", target_os = "macos"),
    all(target_arch = "x86_64", target_os = "windows", target_env = "msvc")
)))]
const CURRENT_COMPILE_TARGET: &str = "unsupported-compile-target";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ReleaseProfileLimits {
    pub platform_entries: usize,
    pub capabilities: usize,
    pub excluded_capabilities: usize,
    pub limitations: usize,
    pub maximum_text_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReleaseProfileError {
    UnknownProfile,
    UnsupportedPlatform,
    ContractInvalid,
}

impl Display for ReleaseProfileError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnknownProfile => "unknown release profile",
            Self::UnsupportedPlatform => "unsupported release profile platform",
            Self::ContractInvalid => "invalid release profile contract",
        })
    }
}

impl Error for ReleaseProfileError {}

#[derive(Clone, Debug)]
pub struct ReleaseProfileV1 {
    value: Value,
}

impl ReleaseProfileV1 {
    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Serializes one validated report in the exact G0 field order plus LF.
    ///
    /// # Errors
    ///
    /// Returns [`ReleaseProfileError::ContractInvalid`] when the report differs
    /// from the closed registry or exceeds its output bound.
    pub fn canonical_stdout(&self) -> Result<Vec<u8>, ReleaseProfileError> {
        validate_release_profile_v1(&self.value)?;
        let bytes = serialize_release_profile(&self.value)?;
        validate_release_profile_output_bytes(bytes.len())?;
        Ok(bytes)
    }
}

#[derive(Clone, Debug)]
pub struct CodeNoesisErrorV25 {
    value: Value,
}

impl CodeNoesisErrorV25 {
    #[must_use]
    pub fn invalid_command() -> Self {
        Self::new(
            "input.invalid_profile_command",
            "input",
            "invalid profile command",
            json!({}),
        )
    }

    #[must_use]
    pub fn invalid_format() -> Self {
        Self::new(
            "input.invalid_format",
            "input",
            "invalid output format",
            json!({}),
        )
    }

    #[must_use]
    pub fn from_release_profile(error: ReleaseProfileError) -> Self {
        match error {
            ReleaseProfileError::UnknownProfile => Self::new(
                "profile.unknown",
                "profile",
                "unknown release profile",
                json!({}),
            ),
            ReleaseProfileError::UnsupportedPlatform => Self::new(
                "profile.unsupported_platform",
                "profile",
                "unsupported release profile platform",
                json!({"target": "unsupported-compile-target"}),
            ),
            ReleaseProfileError::ContractInvalid => Self::contract_invalid(),
        }
    }

    #[must_use]
    pub fn contract_invalid() -> Self {
        Self::new(
            "profile.contract_invalid",
            "internal",
            "invalid release profile contract",
            json!({}),
        )
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Serializes one strict `CodeNoesisErrorV25` followed by LF.
    ///
    /// # Errors
    ///
    /// Returns only when the internally constructed JSON cannot be serialized.
    pub fn canonical_stderr(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = serde_json::to_vec(&self.value)?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    fn new(code: &str, stage: &str, message: &str, context: Value) -> Self {
        let mut value = json!({
            "schema_version": CODENOESIS_ERROR_V25_SCHEMA,
            "code": code,
            "stage": stage,
            "message": message,
            "retryable": false,
            "context": null
        });
        value["context"] = context;
        Self { value }
    }
}

/// Returns the closed first-party G0 registry as structured JSON.
#[must_use]
pub fn embedded_release_profile_registry_v1() -> Value {
    registry_value()
}

/// Resolves the selected profile for the current compile target.
///
/// # Errors
///
/// Returns a typed profile error for an unknown profile, unsupported compile
/// target, or invalid embedded contract.
pub fn current_release_profile_v1(
    profile_id: &str,
) -> Result<ReleaseProfileV1, ReleaseProfileError> {
    release_profile_v1_for_target(profile_id, CURRENT_COMPILE_TARGET)
}

/// Resolves the selected profile for one explicit testable compile target.
///
/// The CLI never exposes this target argument; it exists so the three reviewed
/// target contracts and unsupported-target behavior can be tested everywhere.
///
/// # Errors
///
/// Returns a typed profile error for an unknown profile, unsupported target,
/// or invalid embedded contract.
pub fn release_profile_v1_for_target(
    profile_id: &str,
    target: &str,
) -> Result<ReleaseProfileV1, ReleaseProfileError> {
    release_profile_v1_from_registry(&registry_value(), profile_id, target)
}

/// Resolves a profile from a supplied registry used by conformance tests.
///
/// # Errors
///
/// Returns [`ReleaseProfileError::ContractInvalid`] unless the complete input
/// is byte-semantically equivalent to the closed first-party registry.
pub fn release_profile_v1_from_registry(
    registry: &Value,
    profile_id: &str,
    target: &str,
) -> Result<ReleaseProfileV1, ReleaseProfileError> {
    validate_release_profile_registry_v1(registry)?;
    if profile_id != LOCAL_EXPERIMENTAL_R17_PROFILE {
        return Err(ReleaseProfileError::UnknownProfile);
    }
    let platform = platform_definition(target).ok_or(ReleaseProfileError::UnsupportedPlatform)?;
    let value = report_value(platform);
    validate_release_profile_v1(&value)?;
    Ok(ReleaseProfileV1 { value })
}

/// Validates the complete closed registry and all reviewed bounds.
///
/// # Errors
///
/// Returns [`ReleaseProfileError::ContractInvalid`] for any missing, extra,
/// reordered, private, oversized, or otherwise noncanonical value.
pub fn validate_release_profile_registry_v1(registry: &Value) -> Result<(), ReleaseProfileError> {
    let profile = registry
        .get("profiles")
        .and_then(Value::as_array)
        .filter(|profiles| profiles.len() == 1)
        .and_then(|profiles| profiles.first())
        .ok_or(ReleaseProfileError::ContractInvalid)?;
    let limits = limits_from_profile(profile)?;
    validate_release_profile_limits(limits)?;
    if registry != &registry_value() {
        return Err(ReleaseProfileError::ContractInvalid);
    }
    Ok(())
}

/// Validates one complete `ReleaseProfileV1` value.
///
/// # Errors
///
/// Returns [`ReleaseProfileError::ContractInvalid`] when any field, value,
/// ordering rule, privacy rule, or bound differs from Decision 0033.
pub fn validate_release_profile_v1(value: &Value) -> Result<(), ReleaseProfileError> {
    let target = value
        .pointer("/selected_platform/target")
        .and_then(Value::as_str)
        .ok_or(ReleaseProfileError::ContractInvalid)?;
    let platform = platform_definition(target).ok_or(ReleaseProfileError::ContractInvalid)?;
    let limits = limits_from_profile(value)?;
    validate_release_profile_limits(limits)?;
    if value != &report_value(platform) {
        return Err(ReleaseProfileError::ContractInvalid);
    }
    Ok(())
}

/// Validates the reviewed collection and UTF-8 text bounds.
///
/// # Errors
///
/// Returns [`ReleaseProfileError::ContractInvalid`] above any exact maximum.
pub fn validate_release_profile_limits(
    observed: ReleaseProfileLimits,
) -> Result<(), ReleaseProfileError> {
    if observed.platform_entries > MAX_RELEASE_PROFILE_PLATFORMS
        || observed.capabilities > MAX_RELEASE_PROFILE_CAPABILITIES
        || observed.excluded_capabilities > MAX_RELEASE_PROFILE_EXCLUSIONS
        || observed.limitations > MAX_RELEASE_PROFILE_LIMITATIONS
        || observed.maximum_text_bytes > MAX_RELEASE_PROFILE_TEXT_BYTES
    {
        return Err(ReleaseProfileError::ContractInvalid);
    }
    Ok(())
}

/// Validates the complete report size including its trailing LF.
///
/// # Errors
///
/// Returns [`ReleaseProfileError::ContractInvalid`] above 65,536 bytes.
pub fn validate_release_profile_output_bytes(observed: usize) -> Result<(), ReleaseProfileError> {
    if observed > MAX_RELEASE_PROFILE_OUTPUT_BYTES {
        return Err(ReleaseProfileError::ContractInvalid);
    }
    Ok(())
}

fn registry_value() -> Value {
    json!({
        "schema_version": RELEASE_PROFILE_REGISTRY_V1_SCHEMA,
        "profiles": [{
            "profile_id": LOCAL_EXPERIMENTAL_R17_PROFILE,
            "classification": "experimental",
            "distribution": "source-build-only",
            "support": "none",
            "owner": "@smutti",
            "verification": "not-verified",
            "release_status": "not-ga",
            "platform_matrix": PLATFORMS.map(platform_value),
            "capabilities": CAPABILITIES,
            "excluded_capabilities": EXCLUDED_CAPABILITIES,
            "release_authority": release_authority_value(),
            "limitations": LIMITATIONS
        }]
    })
}

fn report_value(platform: PlatformDefinition) -> Value {
    json!({
        "schema_version": RELEASE_PROFILE_V1_SCHEMA,
        "profile_id": LOCAL_EXPERIMENTAL_R17_PROFILE,
        "classification": "experimental",
        "distribution": "source-build-only",
        "support": "none",
        "owner": "@smutti",
        "verification": "not-verified",
        "release_status": "not-ga",
        "selected_platform": platform_value(platform),
        "platform_matrix": PLATFORMS.map(platform_value),
        "capabilities": CAPABILITIES,
        "excluded_capabilities": EXCLUDED_CAPABILITIES,
        "release_authority": release_authority_value(),
        "limitations": LIMITATIONS
    })
}

fn platform_value(platform: PlatformDefinition) -> Value {
    json!({
        "target": platform.target,
        "classification": "ci-observed-experimental",
        "sandbox_tier": platform.sandbox_tier,
        "normative_os_confinement": platform.normative_os_confinement
    })
}

fn release_authority_value() -> Value {
    json!({
        "signing": "not-available",
        "artifact_attestation": "not-available",
        "build_provenance": "protected-git-and-ci-evidence-only",
        "release_provenance": false,
        "release_publication": false,
        "deployment": false,
        "secrets": false
    })
}

fn platform_definition(target: &str) -> Option<PlatformDefinition> {
    PLATFORMS
        .iter()
        .copied()
        .find(|platform| platform.target == target)
}

fn limits_from_profile(value: &Value) -> Result<ReleaseProfileLimits, ReleaseProfileError> {
    Ok(ReleaseProfileLimits {
        platform_entries: array_len(value, "platform_matrix")?,
        capabilities: array_len(value, "capabilities")?,
        excluded_capabilities: array_len(value, "excluded_capabilities")?,
        limitations: array_len(value, "limitations")?,
        maximum_text_bytes: maximum_text_bytes(value),
    })
}

fn array_len(value: &Value, field: &str) -> Result<usize, ReleaseProfileError> {
    value
        .get(field)
        .and_then(Value::as_array)
        .map(Vec::len)
        .ok_or(ReleaseProfileError::ContractInvalid)
}

fn maximum_text_bytes(value: &Value) -> usize {
    match value {
        Value::String(text) => text.len(),
        Value::Array(values) => values.iter().map(maximum_text_bytes).max().unwrap_or(0),
        Value::Object(values) => values
            .iter()
            .map(|(key, value)| key.len().max(maximum_text_bytes(value)))
            .max()
            .unwrap_or(0),
        Value::Null | Value::Bool(_) | Value::Number(_) => 0,
    }
}

fn serialize_release_profile(value: &Value) -> Result<Vec<u8>, ReleaseProfileError> {
    let object = value
        .as_object()
        .ok_or(ReleaseProfileError::ContractInvalid)?;
    let mut output = Vec::new();
    output.push(b'{');
    write_field(&mut output, object, "schema_version", true)?;
    write_field(&mut output, object, "profile_id", false)?;
    write_field(&mut output, object, "classification", false)?;
    write_field(&mut output, object, "distribution", false)?;
    write_field(&mut output, object, "support", false)?;
    write_field(&mut output, object, "owner", false)?;
    write_field(&mut output, object, "verification", false)?;
    write_field(&mut output, object, "release_status", false)?;
    write_platform_field(&mut output, object, "selected_platform")?;
    write_platform_matrix(&mut output, object)?;
    write_field(&mut output, object, "capabilities", false)?;
    write_field(&mut output, object, "excluded_capabilities", false)?;
    write_release_authority(&mut output, object)?;
    write_field(&mut output, object, "limitations", false)?;
    output.extend_from_slice(b"}\n");
    Ok(output)
}

fn write_field(
    output: &mut Vec<u8>,
    object: &Map<String, Value>,
    name: &str,
    first: bool,
) -> Result<(), ReleaseProfileError> {
    if !first {
        output.push(b',');
    }
    write_name(output, name);
    let value = object
        .get(name)
        .ok_or(ReleaseProfileError::ContractInvalid)?;
    serde_json::to_writer(&mut *output, value).map_err(|_| ReleaseProfileError::ContractInvalid)
}

fn write_platform_field(
    output: &mut Vec<u8>,
    object: &Map<String, Value>,
    name: &str,
) -> Result<(), ReleaseProfileError> {
    output.push(b',');
    write_name(output, name);
    let platform = object
        .get(name)
        .and_then(Value::as_object)
        .ok_or(ReleaseProfileError::ContractInvalid)?;
    write_platform(output, platform)
}

fn write_platform_matrix(
    output: &mut Vec<u8>,
    object: &Map<String, Value>,
) -> Result<(), ReleaseProfileError> {
    output.push(b',');
    write_name(output, "platform_matrix");
    let platforms = object
        .get("platform_matrix")
        .and_then(Value::as_array)
        .ok_or(ReleaseProfileError::ContractInvalid)?;
    output.push(b'[');
    for (index, platform) in platforms.iter().enumerate() {
        if index > 0 {
            output.push(b',');
        }
        write_platform(
            output,
            platform
                .as_object()
                .ok_or(ReleaseProfileError::ContractInvalid)?,
        )?;
    }
    output.push(b']');
    Ok(())
}

fn write_platform(
    output: &mut Vec<u8>,
    platform: &Map<String, Value>,
) -> Result<(), ReleaseProfileError> {
    output.push(b'{');
    write_field(output, platform, "target", true)?;
    write_field(output, platform, "classification", false)?;
    write_field(output, platform, "sandbox_tier", false)?;
    write_field(output, platform, "normative_os_confinement", false)?;
    output.push(b'}');
    Ok(())
}

fn write_release_authority(
    output: &mut Vec<u8>,
    object: &Map<String, Value>,
) -> Result<(), ReleaseProfileError> {
    output.push(b',');
    write_name(output, "release_authority");
    let authority = object
        .get("release_authority")
        .and_then(Value::as_object)
        .ok_or(ReleaseProfileError::ContractInvalid)?;
    output.push(b'{');
    write_field(output, authority, "signing", true)?;
    write_field(output, authority, "artifact_attestation", false)?;
    write_field(output, authority, "build_provenance", false)?;
    write_field(output, authority, "release_provenance", false)?;
    write_field(output, authority, "release_publication", false)?;
    write_field(output, authority, "deployment", false)?;
    write_field(output, authority, "secrets", false)?;
    output.push(b'}');
    Ok(())
}

fn write_name(output: &mut Vec<u8>, name: &str) {
    output.push(b'"');
    output.extend_from_slice(name.as_bytes());
    output.extend_from_slice(b"\":");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fr_rel_001_exact_limits_accept_maximum_and_reject_plus_one() {
        let maximum = ReleaseProfileLimits {
            platform_entries: MAX_RELEASE_PROFILE_PLATFORMS,
            capabilities: MAX_RELEASE_PROFILE_CAPABILITIES,
            excluded_capabilities: MAX_RELEASE_PROFILE_EXCLUSIONS,
            limitations: MAX_RELEASE_PROFILE_LIMITATIONS,
            maximum_text_bytes: MAX_RELEASE_PROFILE_TEXT_BYTES,
        };
        assert_eq!(validate_release_profile_limits(maximum), Ok(()));
        assert_eq!(
            validate_release_profile_output_bytes(MAX_RELEASE_PROFILE_OUTPUT_BYTES),
            Ok(())
        );

        for plus_one in [
            ReleaseProfileLimits {
                platform_entries: MAX_RELEASE_PROFILE_PLATFORMS + 1,
                ..maximum
            },
            ReleaseProfileLimits {
                capabilities: MAX_RELEASE_PROFILE_CAPABILITIES + 1,
                ..maximum
            },
            ReleaseProfileLimits {
                excluded_capabilities: MAX_RELEASE_PROFILE_EXCLUSIONS + 1,
                ..maximum
            },
            ReleaseProfileLimits {
                limitations: MAX_RELEASE_PROFILE_LIMITATIONS + 1,
                ..maximum
            },
            ReleaseProfileLimits {
                maximum_text_bytes: MAX_RELEASE_PROFILE_TEXT_BYTES + 1,
                ..maximum
            },
        ] {
            assert_eq!(
                validate_release_profile_limits(plus_one),
                Err(ReleaseProfileError::ContractInvalid)
            );
        }
        assert_eq!(
            validate_release_profile_output_bytes(MAX_RELEASE_PROFILE_OUTPUT_BYTES + 1),
            Err(ReleaseProfileError::ContractInvalid)
        );
    }

    #[test]
    fn fr_rel_001_malformed_and_private_registries_fail_closed() {
        let mut reordered = registry_value();
        reordered["profiles"][0]["platform_matrix"]
            .as_array_mut()
            .expect("platform matrix")
            .swap(0, 1);
        assert_eq!(
            validate_release_profile_registry_v1(&reordered),
            Err(ReleaseProfileError::ContractInvalid)
        );

        let mut private = registry_value();
        private["profiles"][0]["owner"] = Value::String("CODENOESIS_PRIVATE_CANARY".to_owned());
        assert_eq!(
            validate_release_profile_registry_v1(&private),
            Err(ReleaseProfileError::ContractInvalid)
        );
    }
}
