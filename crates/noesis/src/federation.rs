use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fmt::Write as _;
use std::fs;
use std::io::Read as _;
use std::path::{Component, Path, PathBuf};
use std::time::Instant;

use codenoesis_application::{FederationRequest, FederationService, FederationServiceError};
use codenoesis_contract_extractors::OpenApi31HttpJsonExtractor;
use codenoesis_contracts::{
    CodeNoesisErrorV8, FederationReportError, S6ContractError, parse_federation_workspace,
};
use codenoesis_domain::s6::{FederationLimit, FederationWorkspace, LimitExceeded, ResourceCounter};
use sha2::{Digest, Sha256};

pub(crate) struct FederationFailure {
    pub(crate) error: CodeNoesisErrorV8,
    pub(crate) exit_code: u8,
}

pub(crate) fn requested(arguments: &[OsString]) -> bool {
    arguments.get(1).is_some_and(|value| value == "federate")
}

pub(crate) fn run(arguments: Vec<OsString>) -> Result<Vec<u8>, FederationFailure> {
    let started_at = Instant::now();
    let invocation = FederationInvocation::parse(arguments)?;
    let manifest_path = canonical_manifest(&invocation.workspace_manifest)?;
    let manifest_bytes = read_manifest(&manifest_path)?;
    let workspace =
        parse_federation_workspace(&manifest_bytes).map_err(|error| workspace_failure(&error))?;
    enforce_wall_limit(started_at)?;

    let sources = FederationSources::resolve(manifest_path, &workspace)?;
    enforce_memory_limit(
        u64::try_from(manifest_bytes.len()).unwrap_or(u64::MAX),
        &sources,
    )?;
    noesis::install_s6_filesystem_boundary(
        sources.workspace_manifest.as_os_str(),
        &sources.repository_roots,
    )
    .map_err(|_| internal_failure())?;

    let mut memory = ResourceCounter::new();
    charge_memory(
        &mut memory,
        u64::try_from(manifest_bytes.len()).unwrap_or(u64::MAX),
    )?;
    let provider_bytes = sources.provider.read_and_verify(&mut memory)?;
    let mut client_bytes = Vec::with_capacity(sources.clients.len());
    for source in &sources.clients {
        client_bytes.push(source.read_and_verify(&mut memory)?);
    }
    enforce_wall_limit(started_at)?;

    let output = FederationService::new(OpenApi31HttpJsonExtractor::new())
        .federate(FederationRequest::new(
            workspace,
            &provider_bytes,
            &client_bytes,
        ))
        .map_err(application_failure)?;
    enforce_wall_limit(started_at)?;
    Ok(output)
}

struct FederationInvocation {
    workspace_manifest: PathBuf,
}

impl FederationInvocation {
    fn parse(arguments: Vec<OsString>) -> Result<Self, FederationFailure> {
        let mut arguments = arguments.into_iter();
        let _binary = arguments.next();
        if arguments.next().as_deref() != Some(OsStr::new("federate")) {
            return Err(input_failure(
                CodeNoesisErrorV8::invalid_workspace_manifest(),
            ));
        }

        let mut workspace_manifest = None;
        let mut profile = None;
        let mut format = None;
        while let Some(flag) = arguments.next() {
            let Some(value) = arguments.next() else {
                return Err(invocation_flag_failure(&flag));
            };
            match flag.to_str() {
                Some("--workspace-manifest") if workspace_manifest.is_none() => {
                    workspace_manifest = Some(PathBuf::from(value));
                }
                Some("--profile") if profile.is_none() => profile = value.into_string().ok(),
                Some("--format") if format.is_none() => format = value.into_string().ok(),
                Some("--profile") => {
                    return Err(input_failure(CodeNoesisErrorV8::invalid_profile()));
                }
                Some("--format") => {
                    return Err(input_failure(CodeNoesisErrorV8::invalid_format()));
                }
                _ => {
                    return Err(input_failure(
                        CodeNoesisErrorV8::invalid_workspace_manifest(),
                    ));
                }
            }
        }
        if profile.as_deref() != Some("standard-local-s6") {
            return Err(input_failure(CodeNoesisErrorV8::invalid_profile()));
        }
        if format.as_deref() != Some("json") {
            return Err(input_failure(CodeNoesisErrorV8::invalid_format()));
        }
        let workspace_manifest = workspace_manifest
            .filter(|path| !path.as_os_str().is_empty())
            .ok_or_else(|| input_failure(CodeNoesisErrorV8::invalid_workspace_manifest()))?;
        Ok(Self { workspace_manifest })
    }
}

fn invocation_flag_failure(flag: &OsStr) -> FederationFailure {
    if flag == "--profile" {
        input_failure(CodeNoesisErrorV8::invalid_profile())
    } else if flag == "--format" {
        input_failure(CodeNoesisErrorV8::invalid_format())
    } else {
        input_failure(CodeNoesisErrorV8::invalid_workspace_manifest())
    }
}

struct FederationSources {
    workspace_manifest: PathBuf,
    repository_roots: Vec<PathBuf>,
    provider: BoundInput,
    clients: Vec<BoundInput>,
}

impl FederationSources {
    fn resolve(
        workspace_manifest: PathBuf,
        workspace: &FederationWorkspace,
    ) -> Result<Self, FederationFailure> {
        let base = workspace_manifest
            .parent()
            .ok_or_else(|| input_failure(CodeNoesisErrorV8::invalid_workspace_manifest()))?;
        let provider_root = resolve_root(base, &workspace.provider.root)?;
        let provider_logical =
            logical_path(&workspace.provider.root, &workspace.provider.contract_path);
        let provider = BoundInput::resolve(
            &provider_root,
            &workspace.provider.contract_path,
            provider_logical,
            workspace.provider.contract_sha256.clone(),
            "provider_contract",
            Some(FederationLimit::ContractBytesPerDocument),
        )?;

        let mut roots = BTreeSet::from([provider_root]);
        let mut clients = Vec::with_capacity(workspace.clients.len());
        for client in &workspace.clients {
            let root = resolve_root(base, &client.root)?;
            let logical = logical_path(&client.root, &client.declaration_path);
            clients.push(BoundInput::resolve(
                &root,
                &client.declaration_path,
                logical,
                client.declaration_sha256.clone(),
                "client_declaration",
                None,
            )?);
            roots.insert(root);
        }

        Ok(Self {
            workspace_manifest,
            repository_roots: roots.into_iter().collect(),
            provider,
            clients,
        })
    }
}

struct BoundInput {
    path: PathBuf,
    logical_path: String,
    expected_sha256: String,
    component: &'static str,
    byte_length: u64,
    byte_limit: Option<FederationLimit>,
}

impl BoundInput {
    fn resolve(
        root: &Path,
        relative_path: &str,
        logical_path: String,
        expected_sha256: String,
        component: &'static str,
        byte_limit: Option<FederationLimit>,
    ) -> Result<Self, FederationFailure> {
        let path = resolve_file(root, relative_path, &logical_path)?;
        let byte_length = fs::metadata(&path)
            .map_err(|_| operation_failure(CodeNoesisErrorV8::path_invalid(&logical_path)))?
            .len();
        if let Some(limit) = byte_limit {
            enforce_limit(limit, byte_length, Some(&logical_path))?;
        }
        Ok(Self {
            path,
            logical_path,
            expected_sha256,
            component,
            byte_length,
            byte_limit,
        })
    }

    fn read_and_verify(&self, memory: &mut ResourceCounter) -> Result<Vec<u8>, FederationFailure> {
        let remaining_memory = FederationLimit::MemoryBytes
            .maximum()
            .saturating_sub(memory.observed(FederationLimit::MemoryBytes));
        let read_limit = self
            .byte_limit
            .map_or(remaining_memory, FederationLimit::maximum);
        let bytes = read_bounded(&self.path, read_limit)
            .map_err(|_| operation_failure(CodeNoesisErrorV8::path_invalid(&self.logical_path)))?;
        let observed_length = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if let Some(limit) = self.byte_limit {
            enforce_limit(limit, observed_length, Some(&self.logical_path))?;
        }
        charge_memory(memory, observed_length)?;
        let observed_sha256 = sha256(&bytes);
        if observed_sha256 != self.expected_sha256 {
            return Err(operation_failure(CodeNoesisErrorV8::digest_mismatch(
                &self.logical_path,
                self.component,
                &self.expected_sha256,
                &observed_sha256,
            )));
        }
        Ok(bytes)
    }
}

fn canonical_manifest(path: &Path) -> Result<PathBuf, FederationFailure> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| input_failure(CodeNoesisErrorV8::invalid_workspace_manifest()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(input_failure(
            CodeNoesisErrorV8::invalid_workspace_manifest(),
        ));
    }
    fs::canonicalize(path)
        .map_err(|_| input_failure(CodeNoesisErrorV8::invalid_workspace_manifest()))
}

fn read_manifest(path: &Path) -> Result<Vec<u8>, FederationFailure> {
    let byte_length = fs::metadata(path)
        .map_err(|_| input_failure(CodeNoesisErrorV8::invalid_workspace_manifest()))?
        .len();
    enforce_limit(FederationLimit::WorkspaceManifestBytes, byte_length, None)?;
    let bytes = read_bounded(path, FederationLimit::WorkspaceManifestBytes.maximum())
        .map_err(|_| input_failure(CodeNoesisErrorV8::invalid_workspace_manifest()))?;
    enforce_limit(
        FederationLimit::WorkspaceManifestBytes,
        u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        None,
    )?;
    Ok(bytes)
}

fn read_bounded(path: &Path, maximum: u64) -> std::io::Result<Vec<u8>> {
    let mut file = fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn resolve_root(base: &Path, relative: &str) -> Result<PathBuf, FederationFailure> {
    let path = resolve_components(base, relative, relative)?;
    let metadata = fs::metadata(&path)
        .map_err(|_| operation_failure(CodeNoesisErrorV8::root_policy_violation(relative)))?;
    if !metadata.is_dir() {
        return Err(operation_failure(CodeNoesisErrorV8::root_policy_violation(
            relative,
        )));
    }
    let canonical = fs::canonicalize(&path)
        .map_err(|_| operation_failure(CodeNoesisErrorV8::root_policy_violation(relative)))?;
    if !canonical.starts_with(base) {
        return Err(operation_failure(CodeNoesisErrorV8::root_policy_violation(
            relative,
        )));
    }
    Ok(canonical)
}

fn resolve_file(root: &Path, relative: &str, logical: &str) -> Result<PathBuf, FederationFailure> {
    let path = resolve_components(root, relative, logical)?;
    let metadata = fs::metadata(&path)
        .map_err(|_| operation_failure(CodeNoesisErrorV8::path_invalid(logical)))?;
    if !metadata.is_file() {
        return Err(operation_failure(CodeNoesisErrorV8::path_invalid(logical)));
    }
    let canonical = fs::canonicalize(&path)
        .map_err(|_| operation_failure(CodeNoesisErrorV8::path_invalid(logical)))?;
    if !canonical.starts_with(root) {
        return Err(operation_failure(CodeNoesisErrorV8::path_invalid(logical)));
    }
    Ok(canonical)
}

fn resolve_components(
    base: &Path,
    relative: &str,
    error_path: &str,
) -> Result<PathBuf, FederationFailure> {
    let mut resolved = base.to_path_buf();
    for component in Path::new(relative).components() {
        let Component::Normal(component) = component else {
            return Err(operation_failure(CodeNoesisErrorV8::path_invalid(
                error_path,
            )));
        };
        resolved.push(component);
        let metadata = fs::symlink_metadata(&resolved)
            .map_err(|_| operation_failure(CodeNoesisErrorV8::path_invalid(error_path)))?;
        if metadata.file_type().is_symlink() {
            return Err(operation_failure(CodeNoesisErrorV8::path_invalid(
                error_path,
            )));
        }
    }
    Ok(resolved)
}

fn enforce_memory_limit(
    manifest_bytes: u64,
    sources: &FederationSources,
) -> Result<(), FederationFailure> {
    let observed = sources.clients.iter().fold(
        manifest_bytes.saturating_add(sources.provider.byte_length),
        |total, source| total.saturating_add(source.byte_length),
    );
    enforce_limit(FederationLimit::MemoryBytes, observed, None)
}

fn charge_memory(counter: &mut ResourceCounter, amount: u64) -> Result<(), FederationFailure> {
    counter
        .charge(FederationLimit::MemoryBytes, amount)
        .map(|_| ())
        .map_err(|error| operation_failure(CodeNoesisErrorV8::limit(&error, None)))
}

fn enforce_wall_limit(started_at: Instant) -> Result<(), FederationFailure> {
    let observed = u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX);
    enforce_limit(FederationLimit::WallMilliseconds, observed, None)
}

fn enforce_limit(
    limit: FederationLimit,
    observed: u64,
    path: Option<&str>,
) -> Result<(), FederationFailure> {
    let maximum = limit.maximum();
    if observed <= maximum {
        return Ok(());
    }
    let error = LimitExceeded {
        limit,
        maximum,
        observed: observed.min(maximum.saturating_add(1)),
    };
    Err(operation_failure(CodeNoesisErrorV8::limit(&error, path)))
}

fn workspace_failure(error: &S6ContractError) -> FederationFailure {
    match error {
        S6ContractError::InvalidWorkspaceManifest | S6ContractError::InvalidClientDeclaration => {
            input_failure(CodeNoesisErrorV8::invalid_workspace_manifest())
        }
        S6ContractError::LimitExceeded(error) => {
            operation_failure(CodeNoesisErrorV8::limit(error, None))
        }
        S6ContractError::ReportInvalid => operation_failure(CodeNoesisErrorV8::report_invalid()),
        S6ContractError::Serialization => internal_failure(),
    }
}

fn application_failure(error: FederationServiceError) -> FederationFailure {
    match error {
        FederationServiceError::InputMismatch
        | FederationServiceError::Report(FederationReportError::Serialization) => {
            internal_failure()
        }
        FederationServiceError::Contract(error) => {
            operation_failure(CodeNoesisErrorV8::from_contract(&error))
        }
        FederationServiceError::ClientDeclaration { path } => {
            operation_failure(CodeNoesisErrorV8::invalid_declaration(&path))
        }
        FederationServiceError::Federation(error) => {
            operation_failure(CodeNoesisErrorV8::from_federation(&error))
        }
        FederationServiceError::Report(FederationReportError::LimitExceeded(error)) => {
            operation_failure(CodeNoesisErrorV8::limit(&error, None))
        }
        FederationServiceError::Report(FederationReportError::Invalid) => {
            operation_failure(CodeNoesisErrorV8::report_invalid())
        }
    }
}

fn logical_path(root: &str, path: &str) -> String {
    format!("{root}/{path}")
}

fn sha256(bytes: &[u8]) -> String {
    let mut hexadecimal = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        let _ = write!(hexadecimal, "{byte:02x}");
    }
    hexadecimal
}

fn input_failure(error: CodeNoesisErrorV8) -> FederationFailure {
    FederationFailure {
        error,
        exit_code: 2,
    }
}

fn operation_failure(error: CodeNoesisErrorV8) -> FederationFailure {
    FederationFailure {
        error,
        exit_code: 10,
    }
}

fn internal_failure() -> FederationFailure {
    FederationFailure {
        error: CodeNoesisErrorV8::internal(),
        exit_code: 70,
    }
}
