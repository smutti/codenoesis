use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use serde_json::{Value, json};

pub const LOCAL_CLI_CONFIGURATION_V1_SCHEMA: &str = "codenoesis.configuration/local-cli/v1";
pub const LOCAL_CONFIGURATION_REPORT_V1_SCHEMA: &str = "codenoesis.configuration-report/v1";
pub const LOCAL_DISTRIBUTION_MANIFEST_V1_SCHEMA: &str = "codenoesis.local-distribution/v1";
pub const CODENOESIS_ERROR_V26_SCHEMA: &str = "codenoesis.error/v26";
pub const MAX_LOCAL_CONFIGURATION_BYTES: usize = 65_536;
pub const MAX_LOCAL_CONFIGURATION_REPORT_BYTES: usize = 65_536;
pub const MAX_LOCAL_DISTRIBUTION_BINARY_BYTES: u64 = 268_435_456;
pub const MAX_LOCAL_DISTRIBUTION_MANIFEST_BYTES: usize = 65_536;
pub const LOCAL_DISTRIBUTION_PAYLOAD_COUNT: usize = 5;

pub const DEFAULT_LOCAL_CLI_CONFIGURATION_V1: &[u8] =
    include_bytes!("../../../distribution/local-cli/default-config.json");

#[cfg(all(target_arch = "x86_64", target_os = "linux", target_env = "gnu"))]
const CURRENT_DISTRIBUTION_TARGET: &str = "x86_64-unknown-linux-gnu";
#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
const CURRENT_DISTRIBUTION_TARGET: &str = "aarch64-apple-darwin";
#[cfg(all(target_arch = "x86_64", target_os = "windows", target_env = "msvc"))]
const CURRENT_DISTRIBUTION_TARGET: &str = "x86_64-pc-windows-msvc";
#[cfg(not(any(
    all(target_arch = "x86_64", target_os = "linux", target_env = "gnu"),
    all(target_arch = "aarch64", target_os = "macos"),
    all(target_arch = "x86_64", target_os = "windows", target_env = "msvc")
)))]
const CURRENT_DISTRIBUTION_TARGET: &str = "unsupported-compile-target";

const FROZEN_PAYLOADS: [FrozenPayload; 4] = [
    FrozenPayload {
        path: "etc/codenoesis/config.json",
        length: 301,
        sha256: "a923dcf8937410ed942f6c6f3ec7899f9a2fcccc52b91653bf8aaa3df6e4e327",
    },
    FrozenPayload {
        path: "share/codenoesis/schemas/local-cli-config-v1.schema.json",
        length: 1_443,
        sha256: "e9a5b92168e2163533d20c974e6472fdda8fc43399cad7329d82d2d3eefc30c4",
    },
    FrozenPayload {
        path: "share/doc/codenoesis/INSTALL.md",
        length: 945,
        sha256: "7faf884c53a5c2595850636823227076ef7f71456a3856fef648e705053ee46c",
    },
    FrozenPayload {
        path: "share/doc/codenoesis/LICENSE",
        length: 11_357,
        sha256: "c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4",
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalConfigurationSource {
    EmbeddedDefault,
    ExplicitFile,
}

impl LocalConfigurationSource {
    const fn as_str(self) -> &'static str {
        match self {
            Self::EmbeddedDefault => "embedded-default",
            Self::ExplicitFile => "explicit-file",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalConfigurationError {
    InvalidFile,
    UnsupportedSchema,
    UnsupportedValue,
    ContractInvalid,
}

impl Display for LocalConfigurationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidFile => "invalid local configuration file",
            Self::UnsupportedSchema => "unsupported local configuration schema",
            Self::UnsupportedValue => "unsupported local configuration value",
            Self::ContractInvalid => "invalid local configuration contract",
        })
    }
}

impl Error for LocalConfigurationError {}

#[derive(Clone, Debug)]
pub struct LocalConfigurationReportV1 {
    value: Value,
}

impl LocalConfigurationReportV1 {
    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Serializes the validated configuration report as canonical JSON plus LF.
    ///
    /// # Errors
    ///
    /// Returns [`LocalConfigurationError::ContractInvalid`] if the report no
    /// longer matches the closed contract or exceeds its byte bound.
    pub fn canonical_stdout(&self) -> Result<Vec<u8>, LocalConfigurationError> {
        validate_local_configuration_report_v1(&self.value)?;
        let mut bytes = serde_json::to_vec(&self.value)
            .map_err(|_| LocalConfigurationError::ContractInvalid)?;
        bytes.push(b'\n');
        if bytes.len() > MAX_LOCAL_CONFIGURATION_REPORT_BYTES {
            return Err(LocalConfigurationError::ContractInvalid);
        }
        Ok(bytes)
    }
}

/// Validates one configuration and builds its deterministic public report.
///
/// # Errors
///
/// Returns the corresponding closed configuration or contract error.
pub fn local_configuration_report_v1(
    bytes: &[u8],
    source: LocalConfigurationSource,
) -> Result<LocalConfigurationReportV1, LocalConfigurationError> {
    let configuration = validate_local_cli_configuration_v1(bytes)?;
    let canonical =
        serde_json::to_vec(&configuration).map_err(|_| LocalConfigurationError::ContractInvalid)?;
    let semantic_hash = blake3::hash(&canonical).to_hex().to_string();
    let value = json!({
        "schema_version": LOCAL_CONFIGURATION_REPORT_V1_SCHEMA,
        "configuration_schema": LOCAL_CLI_CONFIGURATION_V1_SCHEMA,
        "source": source.as_str(),
        "release_profile": "local-experimental-r17",
        "semantic_hash": {
            "algorithm": "blake3-256",
            "value": semantic_hash
        },
        "configuration": configuration
    });
    validate_local_configuration_report_v1(&value)?;
    Ok(LocalConfigurationReportV1 { value })
}

/// Validates the complete closed local CLI configuration document.
///
/// # Errors
///
/// Returns an input, schema, value, or internal contract error when the bytes
/// do not represent the sole supported fixed policy.
pub fn validate_local_cli_configuration_v1(bytes: &[u8]) -> Result<Value, LocalConfigurationError> {
    if bytes.len() > MAX_LOCAL_CONFIGURATION_BYTES
        || JsonKeyScanner::has_duplicate_key(bytes)
            .map_err(|()| LocalConfigurationError::InvalidFile)?
    {
        return Err(LocalConfigurationError::InvalidFile);
    }
    let value: Value =
        serde_json::from_slice(bytes).map_err(|_| LocalConfigurationError::InvalidFile)?;
    let schema = value.get("schema_version").and_then(Value::as_str);
    if schema != Some(LOCAL_CLI_CONFIGURATION_V1_SCHEMA) {
        return Err(LocalConfigurationError::UnsupportedSchema);
    }
    if value != embedded_configuration_value()? {
        return Err(LocalConfigurationError::UnsupportedValue);
    }
    Ok(value)
}

fn validate_local_configuration_report_v1(value: &Value) -> Result<(), LocalConfigurationError> {
    let source = match value.get("source").and_then(Value::as_str) {
        Some("embedded-default") => LocalConfigurationSource::EmbeddedDefault,
        Some("explicit-file") => LocalConfigurationSource::ExplicitFile,
        _ => return Err(LocalConfigurationError::ContractInvalid),
    };
    let configuration = value
        .get("configuration")
        .ok_or(LocalConfigurationError::ContractInvalid)?;
    let canonical =
        serde_json::to_vec(configuration).map_err(|_| LocalConfigurationError::ContractInvalid)?;
    let expected = json!({
        "schema_version": LOCAL_CONFIGURATION_REPORT_V1_SCHEMA,
        "configuration_schema": LOCAL_CLI_CONFIGURATION_V1_SCHEMA,
        "source": source.as_str(),
        "release_profile": "local-experimental-r17",
        "semantic_hash": {
            "algorithm": "blake3-256",
            "value": blake3::hash(&canonical).to_hex().to_string()
        },
        "configuration": embedded_configuration_value()?
    });
    if value != &expected {
        return Err(LocalConfigurationError::ContractInvalid);
    }
    Ok(())
}

fn embedded_configuration_value() -> Result<Value, LocalConfigurationError> {
    serde_json::from_slice(DEFAULT_LOCAL_CLI_CONFIGURATION_V1)
        .map_err(|_| LocalConfigurationError::ContractInvalid)
}

struct JsonKeyScanner<'a> {
    bytes: &'a [u8],
    position: usize,
    duplicate: bool,
}

impl<'a> JsonKeyScanner<'a> {
    fn has_duplicate_key(bytes: &'a [u8]) -> Result<bool, ()> {
        let mut scanner = Self {
            bytes,
            position: 0,
            duplicate: false,
        };
        scanner.skip_whitespace();
        scanner.scan_value()?;
        scanner.skip_whitespace();
        if scanner.position != bytes.len() {
            return Err(());
        }
        Ok(scanner.duplicate)
    }

    fn scan_value(&mut self) -> Result<(), ()> {
        self.skip_whitespace();
        match self.peek() {
            Some(b'{') => self.scan_object(),
            Some(b'[') => self.scan_array(),
            Some(b'"') => self.scan_string().map(|_| ()),
            Some(_) => self.scan_atom(),
            None => Err(()),
        }
    }

    fn scan_object(&mut self) -> Result<(), ()> {
        self.expect(b'{')?;
        self.skip_whitespace();
        if self.consume(b'}') {
            return Ok(());
        }
        let mut keys = BTreeSet::new();
        loop {
            self.skip_whitespace();
            let (start, end) = self.scan_string()?;
            let key = serde_json::from_slice::<String>(&self.bytes[start..end]).map_err(|_| ())?;
            if !keys.insert(key) {
                self.duplicate = true;
            }
            self.skip_whitespace();
            self.expect(b':')?;
            self.scan_value()?;
            self.skip_whitespace();
            if self.consume(b'}') {
                return Ok(());
            }
            self.expect(b',')?;
        }
    }

    fn scan_array(&mut self) -> Result<(), ()> {
        self.expect(b'[')?;
        self.skip_whitespace();
        if self.consume(b']') {
            return Ok(());
        }
        loop {
            self.scan_value()?;
            self.skip_whitespace();
            if self.consume(b']') {
                return Ok(());
            }
            self.expect(b',')?;
        }
    }

    fn scan_string(&mut self) -> Result<(usize, usize), ()> {
        let start = self.position;
        self.expect(b'"')?;
        while let Some(byte) = self.peek() {
            self.position += 1;
            match byte {
                b'"' => return Ok((start, self.position)),
                b'\\' => {
                    if self.peek().is_none() {
                        return Err(());
                    }
                    self.position += 1;
                }
                _ => {}
            }
        }
        Err(())
    }

    fn scan_atom(&mut self) -> Result<(), ()> {
        let start = self.position;
        while self
            .peek()
            .is_some_and(|byte| !byte.is_ascii_whitespace() && !matches!(byte, b',' | b']' | b'}'))
        {
            self.position += 1;
        }
        if self.position == start {
            return Err(());
        }
        Ok(())
    }

    fn skip_whitespace(&mut self) {
        while self.peek().is_some_and(|byte| byte.is_ascii_whitespace()) {
            self.position += 1;
        }
    }

    fn expect(&mut self, expected: u8) -> Result<(), ()> {
        if self.consume(expected) {
            Ok(())
        } else {
            Err(())
        }
    }

    fn consume(&mut self, expected: u8) -> bool {
        if self.peek() == Some(expected) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.position).copied()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DistributionFileMode {
    Executable,
    Data,
}

impl DistributionFileMode {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Executable => "executable",
            Self::Data => "data",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalDistributionFileV1 {
    path: String,
    length: u64,
    sha256: String,
    mode: DistributionFileMode,
}

impl LocalDistributionFileV1 {
    #[must_use]
    pub fn new(
        path: impl Into<String>,
        length: u64,
        sha256: impl Into<String>,
        mode: DistributionFileMode,
    ) -> Self {
        Self {
            path: path.into(),
            length,
            sha256: sha256.into(),
            mode,
        }
    }

    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[must_use]
    pub const fn length(&self) -> u64 {
        self.length
    }

    #[must_use]
    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    #[must_use]
    pub const fn mode(&self) -> DistributionFileMode {
        self.mode
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalDistributionError {
    UnsupportedTarget,
    InvalidManifest,
    LimitExceeded,
}

impl Display for LocalDistributionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedTarget => "unsupported local distribution target",
            Self::InvalidManifest => "invalid local distribution manifest",
            Self::LimitExceeded => "local distribution limit exceeded",
        })
    }
}

impl Error for LocalDistributionError {}

#[derive(Clone, Debug)]
pub struct LocalDistributionManifestV1 {
    value: Value,
}

impl LocalDistributionManifestV1 {
    /// Builds a validated deterministic local distribution manifest.
    ///
    /// # Errors
    ///
    /// Returns a target, limit, or manifest error when any payload record
    /// differs from the frozen local distribution contract.
    pub fn new(
        target: &str,
        binary_sha256: &str,
        files: &[LocalDistributionFileV1],
    ) -> Result<Self, LocalDistributionError> {
        validate_distribution_inputs(target, binary_sha256, files)?;
        let value = distribution_manifest_value(target, binary_sha256, files);
        Ok(Self { value })
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Serializes the revalidated manifest as canonical JSON plus LF.
    ///
    /// # Errors
    ///
    /// Returns a manifest or limit error when the in-memory value no longer
    /// matches the closed contract.
    pub fn canonical_stdout(&self) -> Result<Vec<u8>, LocalDistributionError> {
        let target = self
            .value
            .get("target")
            .and_then(Value::as_str)
            .ok_or(LocalDistributionError::InvalidManifest)?;
        let binary_sha256 = self
            .value
            .get("binary_sha256")
            .and_then(Value::as_str)
            .ok_or(LocalDistributionError::InvalidManifest)?;
        let files = files_from_value(&self.value)?;
        validate_distribution_inputs(target, binary_sha256, &files)?;
        if self.value != distribution_manifest_value(target, binary_sha256, &files) {
            return Err(LocalDistributionError::InvalidManifest);
        }
        let mut bytes =
            serde_json::to_vec(&self.value).map_err(|_| LocalDistributionError::InvalidManifest)?;
        bytes.push(b'\n');
        if bytes.len() > MAX_LOCAL_DISTRIBUTION_MANIFEST_BYTES {
            return Err(LocalDistributionError::LimitExceeded);
        }
        Ok(bytes)
    }
}

#[must_use]
pub const fn current_local_distribution_target() -> &'static str {
    CURRENT_DISTRIBUTION_TARGET
}

#[must_use]
pub fn local_distribution_bundle_name(target: &str, binary_sha256: &str) -> String {
    format!("codenoesis-local-experimental-r17-{target}-{binary_sha256}")
}

fn validate_distribution_inputs(
    target: &str,
    binary_sha256: &str,
    files: &[LocalDistributionFileV1],
) -> Result<(), LocalDistributionError> {
    let binary_path =
        binary_path_for_target(target).ok_or(LocalDistributionError::UnsupportedTarget)?;
    if !is_sha256(binary_sha256) || files.len() != LOCAL_DISTRIBUTION_PAYLOAD_COUNT {
        return Err(LocalDistributionError::InvalidManifest);
    }
    let binary = &files[0];
    if binary.path != binary_path
        || binary.length == 0
        || binary.length > MAX_LOCAL_DISTRIBUTION_BINARY_BYTES
        || binary.sha256 != binary_sha256
        || binary.mode != DistributionFileMode::Executable
    {
        return Err(if binary.length > MAX_LOCAL_DISTRIBUTION_BINARY_BYTES {
            LocalDistributionError::LimitExceeded
        } else {
            LocalDistributionError::InvalidManifest
        });
    }
    for (file, frozen) in files[1..].iter().zip(FROZEN_PAYLOADS) {
        if file.path != frozen.path
            || file.length != frozen.length
            || file.sha256 != frozen.sha256
            || file.mode != DistributionFileMode::Data
        {
            return Err(LocalDistributionError::InvalidManifest);
        }
    }
    Ok(())
}

fn distribution_manifest_value(
    target: &str,
    binary_sha256: &str,
    files: &[LocalDistributionFileV1],
) -> Value {
    let files = files
        .iter()
        .map(|file| {
            json!({
                "path": file.path,
                "length": file.length,
                "sha256": file.sha256,
                "mode": file.mode.as_str()
            })
        })
        .collect::<Vec<_>>();
    json!({
        "schema_version": LOCAL_DISTRIBUTION_MANIFEST_V1_SCHEMA,
        "profile_id": "local-experimental-r17",
        "target": target,
        "distribution": "unsigned-staged-directory",
        "support": "none",
        "verification": "not-verified",
        "release_status": "not-ga",
        "signing": "not-available",
        "artifact_attestation": "not-available",
        "release_provenance": false,
        "publication": false,
        "binary_sha256": binary_sha256,
        "files": files
    })
}

fn files_from_value(value: &Value) -> Result<Vec<LocalDistributionFileV1>, LocalDistributionError> {
    value
        .get("files")
        .and_then(Value::as_array)
        .ok_or(LocalDistributionError::InvalidManifest)?
        .iter()
        .map(|file| {
            let mode = match file.get("mode").and_then(Value::as_str) {
                Some("executable") => DistributionFileMode::Executable,
                Some("data") => DistributionFileMode::Data,
                _ => return Err(LocalDistributionError::InvalidManifest),
            };
            Ok(LocalDistributionFileV1::new(
                file.get("path")
                    .and_then(Value::as_str)
                    .ok_or(LocalDistributionError::InvalidManifest)?,
                file.get("length")
                    .and_then(Value::as_u64)
                    .ok_or(LocalDistributionError::InvalidManifest)?,
                file.get("sha256")
                    .and_then(Value::as_str)
                    .ok_or(LocalDistributionError::InvalidManifest)?,
                mode,
            ))
        })
        .collect()
}

fn binary_path_for_target(target: &str) -> Option<&'static str> {
    match target {
        "x86_64-unknown-linux-gnu" | "aarch64-apple-darwin" => Some("bin/noesis"),
        "x86_64-pc-windows-msvc" => Some("bin/noesis.exe"),
        _ => None,
    }
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Clone, Debug)]
pub struct CodeNoesisErrorV26 {
    value: Value,
}

impl CodeNoesisErrorV26 {
    #[must_use]
    pub fn configuration_invalid_arguments() -> Self {
        Self::new(
            "configuration.invalid_arguments",
            "input",
            "invalid configuration command",
            &json!({}),
        )
    }

    #[must_use]
    pub fn configuration_invalid_file() -> Self {
        Self::new(
            "configuration.invalid_file",
            "configuration",
            "invalid configuration file",
            &json!({"limit": "configuration_bytes", "maximum": MAX_LOCAL_CONFIGURATION_BYTES}),
        )
    }

    #[must_use]
    pub fn configuration_unstable_input() -> Self {
        Self::new(
            "configuration.unstable_input",
            "configuration",
            "unstable configuration input",
            &json!({}),
        )
    }

    #[must_use]
    pub fn from_configuration(error: LocalConfigurationError) -> Self {
        match error {
            LocalConfigurationError::InvalidFile => Self::configuration_invalid_file(),
            LocalConfigurationError::UnsupportedSchema => Self::new(
                "configuration.unsupported_schema",
                "configuration",
                "unsupported configuration schema",
                &json!({}),
            ),
            LocalConfigurationError::UnsupportedValue => Self::new(
                "configuration.unsupported_value",
                "configuration",
                "unsupported configuration value",
                &json!({}),
            ),
            LocalConfigurationError::ContractInvalid => Self::internal(),
        }
    }

    #[must_use]
    pub fn distribution_invalid_arguments() -> Self {
        Self::new(
            "distribution.invalid_arguments",
            "input",
            "invalid distribution command",
            &json!({}),
        )
    }

    #[must_use]
    pub fn distribution_invalid_binary() -> Self {
        Self::new(
            "distribution.invalid_binary",
            "distribution",
            "invalid distribution binary",
            &json!({}),
        )
    }

    #[must_use]
    pub fn distribution_output_exists() -> Self {
        Self::new(
            "distribution.output_exists",
            "distribution",
            "distribution output is not empty",
            &json!({}),
        )
    }

    #[must_use]
    pub fn distribution_limit_exceeded() -> Self {
        Self::new(
            "distribution.limit_exceeded",
            "distribution",
            "distribution limit exceeded",
            &json!({"limit": "binary_bytes", "maximum": MAX_LOCAL_DISTRIBUTION_BINARY_BYTES}),
        )
    }

    #[must_use]
    pub fn internal() -> Self {
        Self::new(
            "distribution.internal",
            "internal",
            "internal distribution failure",
            &json!({}),
        )
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Serializes the strict public error as canonical JSON plus LF.
    ///
    /// # Errors
    ///
    /// Returns the underlying JSON serialization error.
    pub fn canonical_stderr(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = serde_json::to_vec(&self.value)?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    fn new(code: &str, stage: &str, message: &str, context: &Value) -> Self {
        Self {
            value: json!({
                "schema_version": CODENOESIS_ERROR_V26_SCHEMA,
                "code": code,
                "stage": stage,
                "message": message,
                "retryable": false,
                "context": context
            }),
        }
    }
}

#[derive(Clone, Copy)]
struct FrozenPayload {
    path: &'static str,
    length: u64,
    sha256: &'static str,
}
