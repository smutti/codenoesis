use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Read as _;
use std::path::{Component, Path, PathBuf};

use codenoesis_contracts::{
    CodeNoesisErrorV23, IMPACT_PIPELINE, ImpactBoundFile, ImpactWorkspaceError, ImpactWorkspaceV1,
    parse_impact_workspace,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const PIPELINE_VERSION: &str = "codenoesis.pipeline/s7-v1";

const WORKSPACE_BYTES_MAXIMUM: u64 = 1_048_576;
const FEDERATION_REPORT_BYTES_MAXIMUM: u64 = 67_108_864;
const SOURCE_FILES_MAXIMUM: u64 = 10_002;
const SOURCE_BYTES_PER_FILE_MAXIMUM: u64 = 2_097_152;
const TOTAL_SOURCE_BYTES_MAXIMUM: u64 = 268_435_456;

pub(crate) struct ImpactFailure {
    pub(crate) error: CodeNoesisErrorV23,
    pub(crate) exit_code: u8,
}

pub(crate) fn requested(arguments: &[OsString]) -> bool {
    arguments.get(1).is_some_and(|value| value == "impact")
}

pub(crate) fn run(arguments: Vec<OsString>) -> Result<Vec<u8>, ImpactFailure> {
    if PIPELINE_VERSION != IMPACT_PIPELINE {
        return Err(internal_failure());
    }
    let invocation = ImpactInvocation::parse(arguments)?;
    let manifest_path = canonical_manifest(&invocation.workspace)?;
    let manifest_bytes = read_bounded(&manifest_path, WORKSPACE_BYTES_MAXIMUM)
        .map_err(|_| input_failure(CodeNoesisErrorV23::invalid_workspace("manifest_read")))?;
    enforce_limit(
        "workspace_bytes",
        WORKSPACE_BYTES_MAXIMUM,
        manifest_bytes.len(),
    )?;
    let workspace = parse_impact_workspace(&manifest_bytes).map_err(workspace_failure)?;
    let sources = ImpactSources::resolve(manifest_path, workspace)?;
    noesis::install_s6_filesystem_boundary(
        sources.workspace_manifest.as_os_str(),
        &sources.allowed_read_roots,
    )
    .map_err(|_| internal_failure())?;
    sources.validate_bound_inputs()?;
    Err(operation_failure(
        CodeNoesisErrorV23::unsupported_implementation_semantics(),
    ))
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
    workspace_manifest: PathBuf,
    allowed_read_roots: Vec<PathBuf>,
    provider_contracts: Vec<BoundInput>,
    provider_sources: Vec<BoundInput>,
    client_sources: Vec<BoundInput>,
    federation_report: BoundInput,
}

impl ImpactSources {
    fn resolve(
        workspace_manifest: PathBuf,
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
        let source_count = provider_sources
            .len()
            .checked_add(client_sources.len())
            .unwrap_or(usize::MAX);
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
        allowed_read_roots.insert(federation_report.path.clone());
        Ok(Self {
            workspace_manifest,
            allowed_read_roots: allowed_read_roots.into_iter().collect(),
            provider_contracts,
            provider_sources,
            client_sources,
            federation_report,
        })
    }

    fn validate_bound_inputs(&self) -> Result<(), ImpactFailure> {
        for input in &self.provider_contracts {
            input.read_and_verify()?;
        }
        for input in self.provider_sources.iter().chain(&self.client_sources) {
            input.read_and_verify()?;
        }
        let report = self.federation_report.read_and_verify()?;
        let report: Value = serde_json::from_slice(&report).map_err(|_| {
            operation_failure(CodeNoesisErrorV23::invalid_federation_report(
                &self.federation_report.logical_path,
                "invalid_json",
            ))
        })?;
        let valid_schema = report
            .as_object()
            .and_then(|object| object.get("schema_version"))
            .and_then(Value::as_str)
            == Some("codenoesis.federation-report/v1");
        if !valid_schema {
            return Err(operation_failure(
                CodeNoesisErrorV23::invalid_federation_report(
                    &self.federation_report.logical_path,
                    "invalid_schema",
                ),
            ));
        }
        Ok(())
    }
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
        let bytes = read_bounded(&self.path, self.maximum)
            .map_err(|_| path_failure(&self.logical_path, self.component, "read_failed"))?;
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

fn read_bounded(path: &Path, maximum: u64) -> std::io::Result<Vec<u8>> {
    let mut file = fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)?;
    Ok(bytes)
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

fn workspace_failure(error: ImpactWorkspaceError) -> ImpactFailure {
    match error {
        ImpactWorkspaceError::Invalid => {
            input_failure(CodeNoesisErrorV23::invalid_workspace("invalid_manifest"))
        }
        ImpactWorkspaceError::TooManyClients { observed } => operation_failure(
            CodeNoesisErrorV23::limit("linked_clients", 10_000, observed),
        ),
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
