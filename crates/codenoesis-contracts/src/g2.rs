use std::error::Error;
use std::fmt::{self, Display, Formatter};

use serde_json::{Value, json};

use crate::{
    DistributionFileMode, LocalDistributionError, LocalDistributionFileV1,
    LocalDistributionManifestV1, local_distribution_bundle_name,
};

pub const LOCAL_UPGRADE_PLAN_V1_SCHEMA: &str = "codenoesis.local-upgrade-plan/v1";
pub const LOCAL_ROLLBACK_REPORT_V1_SCHEMA: &str = "codenoesis.local-rollback-report/v1";
pub const CODENOESIS_ERROR_V27_SCHEMA: &str = "codenoesis.error/v27";
pub const MAX_LOCAL_UPGRADE_PLAN_BYTES: usize = 65_536;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalUpgradeContractError {
    InvalidBundle,
    Incompatible,
    InvalidPlan,
    LimitExceeded,
    ContractInvalid,
}

impl Display for LocalUpgradeContractError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidBundle => "invalid local distribution bundle",
            Self::Incompatible => "incompatible local bundle transition",
            Self::InvalidPlan => "invalid local upgrade plan",
            Self::LimitExceeded => "local upgrade contract limit exceeded",
            Self::ContractInvalid => "invalid local upgrade contract",
        })
    }
}

impl Error for LocalUpgradeContractError {}

#[derive(Clone, Debug)]
pub struct ValidatedLocalDistributionManifestV1 {
    target: String,
    binary_sha256: String,
    files: Vec<LocalDistributionFileV1>,
}

impl ValidatedLocalDistributionManifestV1 {
    #[must_use]
    pub fn target(&self) -> &str {
        &self.target
    }

    #[must_use]
    pub fn binary_sha256(&self) -> &str {
        &self.binary_sha256
    }

    #[must_use]
    pub fn files(&self) -> &[LocalDistributionFileV1] {
        &self.files
    }
}

/// Validates byte-exact canonical G1a manifest bytes and returns their bounded
/// filesystem expectations.
///
/// # Errors
///
/// Returns a typed bundle or limit error when the document is not the exact
/// closed G1a contract.
pub fn validate_local_distribution_manifest_v1(
    bytes: &[u8],
) -> Result<ValidatedLocalDistributionManifestV1, LocalUpgradeContractError> {
    if bytes.len() > crate::MAX_LOCAL_DISTRIBUTION_MANIFEST_BYTES {
        return Err(LocalUpgradeContractError::LimitExceeded);
    }
    let value: Value =
        serde_json::from_slice(bytes).map_err(|_| LocalUpgradeContractError::InvalidBundle)?;
    let target = value
        .get("target")
        .and_then(Value::as_str)
        .ok_or(LocalUpgradeContractError::InvalidBundle)?;
    let binary_sha256 = value
        .get("binary_sha256")
        .and_then(Value::as_str)
        .ok_or(LocalUpgradeContractError::InvalidBundle)?;
    let files = value
        .get("files")
        .and_then(Value::as_array)
        .ok_or(LocalUpgradeContractError::InvalidBundle)?
        .iter()
        .map(local_distribution_file_from_value)
        .collect::<Result<Vec<_>, _>>()?;
    let manifest = LocalDistributionManifestV1::new(target, binary_sha256, &files)
        .map_err(map_distribution_error)?;
    let canonical = manifest
        .canonical_stdout()
        .map_err(map_distribution_error)?;
    if canonical != bytes {
        return Err(LocalUpgradeContractError::InvalidBundle);
    }
    Ok(ValidatedLocalDistributionManifestV1 {
        target: target.to_owned(),
        binary_sha256: binary_sha256.to_owned(),
        files,
    })
}

fn local_distribution_file_from_value(
    value: &Value,
) -> Result<LocalDistributionFileV1, LocalUpgradeContractError> {
    let mode = match value.get("mode").and_then(Value::as_str) {
        Some("executable") => DistributionFileMode::Executable,
        Some("data") => DistributionFileMode::Data,
        _ => return Err(LocalUpgradeContractError::InvalidBundle),
    };
    Ok(LocalDistributionFileV1::new(
        value
            .get("path")
            .and_then(Value::as_str)
            .ok_or(LocalUpgradeContractError::InvalidBundle)?,
        value
            .get("length")
            .and_then(Value::as_u64)
            .ok_or(LocalUpgradeContractError::InvalidBundle)?,
        value
            .get("sha256")
            .and_then(Value::as_str)
            .ok_or(LocalUpgradeContractError::InvalidBundle)?,
        mode,
    ))
}

fn map_distribution_error(error: LocalDistributionError) -> LocalUpgradeContractError {
    match error {
        LocalDistributionError::LimitExceeded => LocalUpgradeContractError::LimitExceeded,
        LocalDistributionError::UnsupportedTarget | LocalDistributionError::InvalidManifest => {
            LocalUpgradeContractError::InvalidBundle
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalBundleIdentityV1 {
    binary_sha256: String,
    bundle_name: String,
    manifest_sha256: String,
}

impl LocalBundleIdentityV1 {
    /// Builds one exact G1a bundle identity for the supplied platform target.
    ///
    /// # Errors
    ///
    /// Returns [`LocalUpgradeContractError::InvalidBundle`] for malformed
    /// digests, unsupported targets, or a non-canonical bundle name.
    pub fn new(
        target: &str,
        binary_sha256: impl Into<String>,
        bundle_name: impl Into<String>,
        manifest_sha256: impl Into<String>,
    ) -> Result<Self, LocalUpgradeContractError> {
        let binary_sha256 = binary_sha256.into();
        let bundle_name = bundle_name.into();
        let manifest_sha256 = manifest_sha256.into();
        if !is_supported_target(target)
            || !is_sha256(&binary_sha256)
            || !is_sha256(&manifest_sha256)
            || bundle_name != local_distribution_bundle_name(target, &binary_sha256)
        {
            return Err(LocalUpgradeContractError::InvalidBundle);
        }
        Ok(Self {
            binary_sha256,
            bundle_name,
            manifest_sha256,
        })
    }

    #[must_use]
    pub fn binary_sha256(&self) -> &str {
        &self.binary_sha256
    }

    #[must_use]
    pub fn bundle_name(&self) -> &str {
        &self.bundle_name
    }

    #[must_use]
    pub fn manifest_sha256(&self) -> &str {
        &self.manifest_sha256
    }

    fn value(&self) -> Value {
        json!({
            "binary_sha256": self.binary_sha256,
            "bundle_name": self.bundle_name,
            "manifest_sha256": self.manifest_sha256
        })
    }
}

#[derive(Clone, Debug)]
pub struct LocalUpgradePlanV1 {
    target: String,
    current: LocalBundleIdentityV1,
    candidate: LocalBundleIdentityV1,
    value: Value,
}

impl LocalUpgradePlanV1 {
    /// Builds one exact output-only side-by-side transition plan.
    ///
    /// # Errors
    ///
    /// Returns a typed incompatibility or contract error when the pair is not
    /// a distinct supported G1a transition.
    pub fn new(
        target: &str,
        current: LocalBundleIdentityV1,
        candidate: LocalBundleIdentityV1,
    ) -> Result<Self, LocalUpgradeContractError> {
        if !is_supported_target(target) {
            return Err(LocalUpgradeContractError::InvalidBundle);
        }
        if current == candidate {
            return Err(LocalUpgradeContractError::Incompatible);
        }
        let value = local_upgrade_plan_value(target, &current, &candidate);
        let plan = Self {
            target: target.to_owned(),
            current,
            candidate,
            value,
        };
        plan.canonical_stdout()?;
        Ok(plan)
    }

    #[must_use]
    pub fn target(&self) -> &str {
        &self.target
    }

    #[must_use]
    pub const fn current(&self) -> &LocalBundleIdentityV1 {
        &self.current
    }

    #[must_use]
    pub const fn candidate(&self) -> &LocalBundleIdentityV1 {
        &self.candidate
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Serializes the revalidated plan as canonical JSON plus LF.
    ///
    /// # Errors
    ///
    /// Returns a contract or limit error if the in-memory value differs from
    /// the closed V1 plan.
    pub fn canonical_stdout(&self) -> Result<Vec<u8>, LocalUpgradeContractError> {
        if self.value != local_upgrade_plan_value(&self.target, &self.current, &self.candidate) {
            return Err(LocalUpgradeContractError::ContractInvalid);
        }
        canonical_bounded(&self.value)
    }
}

/// Parses one exact canonical V1 plan for rollback binding.
///
/// # Errors
///
/// Returns a typed plan or limit error for any non-canonical, substituted, or
/// unsupported document.
pub fn parse_local_upgrade_plan_v1(
    bytes: &[u8],
) -> Result<LocalUpgradePlanV1, LocalUpgradeContractError> {
    if bytes.len() > MAX_LOCAL_UPGRADE_PLAN_BYTES {
        return Err(LocalUpgradeContractError::LimitExceeded);
    }
    let value: Value =
        serde_json::from_slice(bytes).map_err(|_| LocalUpgradeContractError::InvalidPlan)?;
    let target = value
        .get("platform_target")
        .and_then(Value::as_str)
        .ok_or(LocalUpgradeContractError::InvalidPlan)?;
    let current = bundle_identity_from_value(
        target,
        value
            .get("current")
            .ok_or(LocalUpgradeContractError::InvalidPlan)?,
    )
    .map_err(|_| LocalUpgradeContractError::InvalidPlan)?;
    let candidate = bundle_identity_from_value(
        target,
        value
            .get("candidate")
            .ok_or(LocalUpgradeContractError::InvalidPlan)?,
    )
    .map_err(|_| LocalUpgradeContractError::InvalidPlan)?;
    let plan = LocalUpgradePlanV1::new(target, current, candidate)
        .map_err(|_| LocalUpgradeContractError::InvalidPlan)?;
    if plan.canonical_stdout()? != bytes {
        return Err(LocalUpgradeContractError::InvalidPlan);
    }
    Ok(plan)
}

fn bundle_identity_from_value(
    target: &str,
    value: &Value,
) -> Result<LocalBundleIdentityV1, LocalUpgradeContractError> {
    LocalBundleIdentityV1::new(
        target,
        value
            .get("binary_sha256")
            .and_then(Value::as_str)
            .ok_or(LocalUpgradeContractError::InvalidPlan)?,
        value
            .get("bundle_name")
            .and_then(Value::as_str)
            .ok_or(LocalUpgradeContractError::InvalidPlan)?,
        value
            .get("manifest_sha256")
            .and_then(Value::as_str)
            .ok_or(LocalUpgradeContractError::InvalidPlan)?,
    )
}

fn local_upgrade_plan_value(
    target: &str,
    current: &LocalBundleIdentityV1,
    candidate: &LocalBundleIdentityV1,
) -> Value {
    json!({
        "activation": "caller-owned",
        "automatic_update": false,
        "candidate": candidate.value(),
        "compatibility": "exact-g1a-side-by-side",
        "configuration_transition": "identical-v1-no-migration",
        "current": current.value(),
        "downgrade": "forbidden-without-exact-plan",
        "platform_target": target,
        "profile_id": "local-experimental-r17",
        "publication": false,
        "release_status": "not-ga",
        "rollback": {
            "mode": "exact-prior-bundle",
            "required_current_manifest_sha256": candidate.manifest_sha256,
            "target_manifest_sha256": current.manifest_sha256
        },
        "schema_version": LOCAL_UPGRADE_PLAN_V1_SCHEMA,
        "signing": "not-available",
        "support": "none",
        "verification": "not-verified"
    })
}

#[derive(Clone, Debug)]
pub struct LocalRollbackReportV1 {
    value: Value,
}

impl LocalRollbackReportV1 {
    /// Builds the exact rollback report bound to one canonical prior plan.
    ///
    /// # Errors
    ///
    /// Returns an incompatibility or contract error unless the current and
    /// target bundles exactly reverse the supplied plan.
    pub fn new(
        plan: &LocalUpgradePlanV1,
        current: &LocalBundleIdentityV1,
        target: &LocalBundleIdentityV1,
        plan_sha256: &str,
    ) -> Result<Self, LocalUpgradeContractError> {
        if current != plan.candidate() || target != plan.current() || !is_sha256(plan_sha256) {
            return Err(LocalUpgradeContractError::Incompatible);
        }
        let value = local_rollback_report_value(plan, current, target, plan_sha256);
        let report = Self { value };
        report.canonical_stdout()?;
        Ok(report)
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Serializes one canonical rollback report plus LF.
    ///
    /// # Errors
    ///
    /// Returns a limit error when the output exceeds the V1 plan bound.
    pub fn canonical_stdout(&self) -> Result<Vec<u8>, LocalUpgradeContractError> {
        canonical_bounded(&self.value)
    }
}

fn local_rollback_report_value(
    plan: &LocalUpgradePlanV1,
    current: &LocalBundleIdentityV1,
    target: &LocalBundleIdentityV1,
    plan_sha256: &str,
) -> Value {
    json!({
        "activation": "caller-owned",
        "compatibility": "exact-plan-match",
        "configuration_transition": "identical-v1-no-migration",
        "current": current.value(),
        "downgrade": "exact-plan-only",
        "operation": "rollback-preflight",
        "plan_sha256": plan_sha256,
        "platform_target": plan.target(),
        "profile_id": "local-experimental-r17",
        "publication": false,
        "release_status": "not-ga",
        "schema_version": LOCAL_ROLLBACK_REPORT_V1_SCHEMA,
        "signing": "not-available",
        "support": "none",
        "target_bundle": target.value(),
        "verification": "not-verified"
    })
}

fn canonical_bounded(value: &Value) -> Result<Vec<u8>, LocalUpgradeContractError> {
    let mut bytes =
        serde_json::to_vec(value).map_err(|_| LocalUpgradeContractError::ContractInvalid)?;
    bytes.push(b'\n');
    if bytes.len() > MAX_LOCAL_UPGRADE_PLAN_BYTES {
        return Err(LocalUpgradeContractError::LimitExceeded);
    }
    Ok(bytes)
}

fn is_supported_target(target: &str) -> bool {
    matches!(
        target,
        "aarch64-apple-darwin" | "x86_64-pc-windows-msvc" | "x86_64-unknown-linux-gnu"
    )
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Clone, Debug)]
pub struct CodeNoesisErrorV27 {
    value: Value,
}

impl CodeNoesisErrorV27 {
    #[must_use]
    pub fn invalid_arguments() -> Self {
        Self::new(
            "compatibility.invalid_arguments",
            "input",
            "invalid compatibility command",
            &json!({}),
        )
    }

    #[must_use]
    pub fn invalid_bundle() -> Self {
        Self::new(
            "compatibility.invalid_bundle",
            "compatibility",
            "invalid local distribution bundle",
            &json!({}),
        )
    }

    #[must_use]
    pub fn unstable_input() -> Self {
        Self::new(
            "compatibility.unstable_input",
            "compatibility",
            "unstable compatibility input",
            &json!({}),
        )
    }

    #[must_use]
    pub fn incompatible() -> Self {
        Self::new(
            "compatibility.incompatible",
            "compatibility",
            "incompatible local bundle transition",
            &json!({}),
        )
    }

    #[must_use]
    pub fn invalid_plan() -> Self {
        Self::new(
            "compatibility.invalid_plan",
            "compatibility",
            "invalid local upgrade plan",
            &json!({}),
        )
    }

    #[must_use]
    pub fn limit_exceeded() -> Self {
        Self::new(
            "compatibility.limit_exceeded",
            "compatibility",
            "compatibility input limit exceeded",
            &json!({"limit": "plan_bytes", "maximum": MAX_LOCAL_UPGRADE_PLAN_BYTES}),
        )
    }

    #[must_use]
    pub fn internal() -> Self {
        Self::new(
            "compatibility.internal",
            "internal",
            "internal compatibility failure",
            &json!({}),
        )
    }

    #[must_use]
    pub fn from_contract(error: LocalUpgradeContractError) -> Self {
        match error {
            LocalUpgradeContractError::InvalidBundle => Self::invalid_bundle(),
            LocalUpgradeContractError::Incompatible => Self::incompatible(),
            LocalUpgradeContractError::InvalidPlan => Self::invalid_plan(),
            LocalUpgradeContractError::LimitExceeded => Self::limit_exceeded(),
            LocalUpgradeContractError::ContractInvalid => Self::internal(),
        }
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Serializes one strict `ErrorV27` followed by LF.
    ///
    /// # Errors
    ///
    /// Returns only when the internally constructed JSON cannot be serialized.
    pub fn canonical_stderr(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = serde_json::to_vec(&self.value)?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    fn new(code: &str, stage: &str, message: &str, context: &Value) -> Self {
        Self {
            value: json!({
                "code": code,
                "context": context,
                "message": message,
                "retryable": false,
                "schema_version": CODENOESIS_ERROR_V27_SCHEMA,
                "stage": stage
            }),
        }
    }
}
