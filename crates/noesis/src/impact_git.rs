use std::ffi::{OsStr, OsString};
use std::fs::{self, Metadata};
use std::io::{Read as _, Seek as _};
use std::path::{Component, Path, PathBuf};

use codenoesis_application::{
    GitImpactAcquisitionError, GitImpactAcquisitionService, GitImpactRepositoryRequest,
};
use codenoesis_contracts::{
    CodeNoesisErrorV30, GitImpactSourceFile, ImpactGitClientInput, ImpactGitProviderInput,
    ImpactGitRevisionInput, ImpactGitWorkspaceError, MAX_R19_FEDERATION_BYTES,
    MAX_R19_WORKSPACE_BYTES, R19_ANALYSIS_PROFILE, SemanticCompatibilityReportV2,
    SemanticReportV2Error, parse_impact_git_workspace,
};
use codenoesis_domain::{RepositoryIdentity, Revision};
use codenoesis_repository::LocalGitRepository;
use same_file::Handle as FileIdentity;
use sha2::{Digest as _, Sha256};

use crate::impact::{
    MaterializedClientRevision, MaterializedImpactInput, MaterializedProviderRevision,
    analyze_materialized,
};

pub(crate) struct ImpactGitFailure {
    pub(crate) error: CodeNoesisErrorV30,
    pub(crate) exit_code: u8,
}

pub(crate) fn requested(arguments: &[OsString]) -> bool {
    arguments.get(1).is_some_and(|value| value == "impact")
        && arguments.windows(2).any(|pair| {
            pair[0] == OsStr::new("--profile") && pair[1] == OsStr::new(R19_ANALYSIS_PROFILE)
        })
}

pub(crate) fn run(arguments: Vec<OsString>) -> Result<Vec<u8>, ImpactGitFailure> {
    let invocation = ImpactGitInvocation::parse(arguments)?;
    let manifest_path = canonical_file(&invocation.workspace, "workspace")?;
    let manifest_bytes = read_stable_input(&manifest_path, MAX_R19_WORKSPACE_BYTES, "workspace")?;
    let workspace = parse_impact_git_workspace(&manifest_bytes).map_err(workspace_failure)?;
    let base = manifest_path
        .parent()
        .ok_or_else(|| input_failure(CodeNoesisErrorV30::impact_invalid_workspace()))?;
    let provider_root = resolve_root(base, &workspace.provider.root)?;
    let mut roots = vec![provider_root.clone()];
    let mut client_roots = Vec::with_capacity(workspace.clients.len());
    for client in &workspace.clients {
        let root = resolve_root(base, &client.root)?;
        roots.push(root.clone());
        client_roots.push(root);
    }
    roots.sort();
    roots.dedup();
    if roots.len() != workspace.clients.len().saturating_add(1) {
        return Err(input_failure(CodeNoesisErrorV30::impact_invalid_workspace()));
    }
    let federation_path = resolve_file(base, &workspace.federation_report.path)?;
    noesis::install_s6_filesystem_boundary(federation_path.as_os_str(), &roots)
        .map_err(|_| internal_failure())?;
    let federation_report = read_stable_input(
        &federation_path,
        MAX_R19_FEDERATION_BYTES,
        "federation_report",
    )?;
    if sha256(&federation_report) != workspace.federation_report.sha256 {
        return Err(operation_failure(
            CodeNoesisErrorV30::impact_invalid_federation_report(),
        ));
    }

    let requests = acquisition_requests(
        &workspace.provider,
        &provider_root,
        &workspace.clients,
        &client_roots,
    )?;
    let sources = if invocation.packed {
        GitImpactAcquisitionService::new(LocalGitRepository::new_packed_sha1()).acquire(&requests)
    } else {
        GitImpactAcquisitionService::new(LocalGitRepository::new()).acquire(&requests)
    }
    .map_err(|error| acquisition_failure(&error))?;

    let input = materialized_input(
        &workspace.provider,
        &workspace.clients,
        &sources,
        workspace.federation_report.path,
        federation_report,
    )?;
    let report = analyze_materialized(input).map_err(|failure| {
        let code = failure.error.value()["code"].as_str().unwrap_or_default();
        if code.contains("federation") {
            operation_failure(CodeNoesisErrorV30::impact_invalid_federation_report())
        } else if code.contains("limit") {
            operation_failure(CodeNoesisErrorV30::impact_limit_exceeded())
        } else {
            operation_failure(CodeNoesisErrorV30::impact_source_rejected())
        }
    })?;
    SemanticCompatibilityReportV2::from_domain(&report, &sources, sha256)
        .map(|report| report.canonical_stdout())
        .map_err(report_failure)
}

struct ImpactGitInvocation {
    workspace: PathBuf,
    packed: bool,
}

impl ImpactGitInvocation {
    fn parse(arguments: Vec<OsString>) -> Result<Self, ImpactGitFailure> {
        let mut arguments = arguments.into_iter();
        let _binary = arguments.next();
        if arguments.next().as_deref() != Some(OsStr::new("impact")) {
            return Err(input_failure(CodeNoesisErrorV30::impact_invalid_workspace()));
        }
        let mut workspace = None;
        let mut profile = None;
        let mut acquisition_profile = None;
        let mut format = None;
        while let Some(flag) = arguments.next() {
            let Some(value) = arguments.next() else {
                return Err(input_failure(CodeNoesisErrorV30::impact_invalid_workspace()));
            };
            match flag.to_str() {
                Some("--workspace") if workspace.is_none() => {
                    workspace = Some(PathBuf::from(value));
                }
                Some("--profile") if profile.is_none() => profile = value.into_string().ok(),
                Some("--acquisition-profile") if acquisition_profile.is_none() => {
                    acquisition_profile = value.into_string().ok();
                }
                Some("--format") if format.is_none() => format = value.into_string().ok(),
                _ => {
                    return Err(input_failure(CodeNoesisErrorV30::impact_invalid_workspace()));
                }
            }
        }
        if profile.as_deref() != Some(R19_ANALYSIS_PROFILE)
            || format.as_deref() != Some("json")
            || acquisition_profile.as_deref().is_some_and(|profile| {
                profile != codenoesis_domain::s1_packed::LOCAL_GIT_SHA1_PACKED_V1
            })
        {
            return Err(input_failure(CodeNoesisErrorV30::impact_invalid_workspace()));
        }
        let workspace = workspace
            .filter(|path| !path.as_os_str().is_empty())
            .ok_or_else(|| input_failure(CodeNoesisErrorV30::impact_invalid_workspace()))?;
        Ok(Self {
            workspace,
            packed: acquisition_profile.is_some(),
        })
    }
}

fn acquisition_requests(
    provider: &ImpactGitProviderInput,
    provider_root: &Path,
    clients: &[ImpactGitClientInput],
    client_roots: &[PathBuf],
) -> Result<Vec<GitImpactRepositoryRequest>, ImpactGitFailure> {
    let provider_identity = RepositoryIdentity::parse(&provider.repository_identity)
        .map_err(|_| input_failure(CodeNoesisErrorV30::impact_invalid_workspace()))?;
    let mut requests = vec![
        GitImpactRepositoryRequest::new(
            provider_root.as_os_str().to_owned(),
            provider_identity.clone(),
            Revision::parse(&provider.baseline.revision)
                .map_err(|_| input_failure(CodeNoesisErrorV30::impact_invalid_workspace()))?,
            vec![
                provider.baseline.contract_path.clone(),
                provider.baseline.source_path.clone(),
            ],
        ),
        GitImpactRepositoryRequest::new(
            provider_root.as_os_str().to_owned(),
            provider_identity,
            Revision::parse(&provider.target.revision)
                .map_err(|_| input_failure(CodeNoesisErrorV30::impact_invalid_workspace()))?,
            vec![
                provider.target.contract_path.clone(),
                provider.target.source_path.clone(),
            ],
        ),
    ];
    for (client, root) in clients.iter().zip(client_roots) {
        requests.push(GitImpactRepositoryRequest::new(
            root.as_os_str().to_owned(),
            RepositoryIdentity::parse(&client.repository_identity)
                .map_err(|_| input_failure(CodeNoesisErrorV30::impact_invalid_workspace()))?,
            Revision::parse(&client.revision)
                .map_err(|_| input_failure(CodeNoesisErrorV30::impact_invalid_workspace()))?,
            vec![client.source_path.clone()],
        ));
    }
    Ok(requests)
}

fn materialized_input(
    provider: &ImpactGitProviderInput,
    clients: &[ImpactGitClientInput],
    sources: &[GitImpactSourceFile],
    federation_report_path: String,
    federation_report: Vec<u8>,
) -> Result<MaterializedImpactInput, ImpactGitFailure> {
    let baseline = provider_revision(provider, &provider.baseline, sources)?;
    let target = provider_revision(provider, &provider.target, sources)?;
    let mut materialized_clients = Vec::with_capacity(clients.len());
    for client in clients {
        let source = selected_source(
            sources,
            &client.repository_identity,
            &client.revision,
            &client.source_path,
        )?;
        materialized_clients.push(MaterializedClientRevision {
            role: client.role.clone(),
            repository_identity: client.repository_identity.clone(),
            revision: client.revision.clone(),
            federation_revision: client.federation_revision.clone(),
            source_path: client.source_path.clone(),
            source_bytes: source.bytes.clone(),
            decoder_symbol: client.decoder_symbol.clone(),
            call_symbol: client.call_symbol.clone(),
        });
    }
    Ok(MaterializedImpactInput {
        provider_repository_identity: provider.repository_identity.clone(),
        baseline,
        target,
        clients: materialized_clients,
        federation_report_path,
        federation_report,
    })
}

fn provider_revision(
    provider: &ImpactGitProviderInput,
    revision: &ImpactGitRevisionInput,
    sources: &[GitImpactSourceFile],
) -> Result<MaterializedProviderRevision, ImpactGitFailure> {
    let contract = selected_source(
        sources,
        &provider.repository_identity,
        &revision.revision,
        &revision.contract_path,
    )?;
    let source = selected_source(
        sources,
        &provider.repository_identity,
        &revision.revision,
        &revision.source_path,
    )?;
    Ok(MaterializedProviderRevision {
        revision: revision.revision.clone(),
        federation_revision: revision.federation_revision.clone(),
        contract_path: revision.contract_path.clone(),
        contract_sha256: sha256(&contract.bytes),
        contract_bytes: contract.bytes.clone(),
        source_path: revision.source_path.clone(),
        source_sha256: sha256(&source.bytes),
        source_bytes: source.bytes.clone(),
        callable_symbol: revision.callable_symbol.clone(),
    })
}

fn selected_source<'a>(
    sources: &'a [GitImpactSourceFile],
    repository_identity: &str,
    revision: &str,
    path: &str,
) -> Result<&'a GitImpactSourceFile, ImpactGitFailure> {
    let matches = sources
        .iter()
        .filter(|source| {
            source.repository_identity == repository_identity
                && source.commit_oid == revision
                && source.path == path
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [source] => Ok(*source),
        _ => Err(operation_failure(
            CodeNoesisErrorV30::impact_source_rejected(),
        )),
    }
}

pub(crate) fn canonical_file(path: &Path, component: &str) -> Result<PathBuf, ImpactGitFailure> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_| input_failure_for_component(component))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(input_failure_for_component(component));
    }
    fs::canonicalize(path).map_err(|_| input_failure_for_component(component))
}

fn resolve_root(base: &Path, relative: &str) -> Result<PathBuf, ImpactGitFailure> {
    let path = resolve_components(base, relative)?;
    let metadata = fs::symlink_metadata(&path)
        .map_err(|_| input_failure(CodeNoesisErrorV30::impact_invalid_workspace()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(input_failure(CodeNoesisErrorV30::impact_invalid_workspace()));
    }
    let canonical = fs::canonicalize(&path)
        .map_err(|_| input_failure(CodeNoesisErrorV30::impact_invalid_workspace()))?;
    if !canonical.starts_with(base) {
        return Err(input_failure(CodeNoesisErrorV30::impact_invalid_workspace()));
    }
    Ok(canonical)
}

fn resolve_file(base: &Path, relative: &str) -> Result<PathBuf, ImpactGitFailure> {
    let path = resolve_components(base, relative)?;
    let metadata = fs::symlink_metadata(&path)
        .map_err(|_| input_failure(CodeNoesisErrorV30::impact_invalid_workspace()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(input_failure(CodeNoesisErrorV30::impact_invalid_workspace()));
    }
    let canonical = fs::canonicalize(&path)
        .map_err(|_| input_failure(CodeNoesisErrorV30::impact_invalid_workspace()))?;
    if !canonical.starts_with(base) {
        return Err(input_failure(CodeNoesisErrorV30::impact_invalid_workspace()));
    }
    Ok(canonical)
}

fn resolve_components(base: &Path, relative: &str) -> Result<PathBuf, ImpactGitFailure> {
    let mut resolved = base.to_path_buf();
    for component in Path::new(relative).components() {
        let Component::Normal(component) = component else {
            return Err(input_failure(CodeNoesisErrorV30::impact_invalid_workspace()));
        };
        resolved.push(component);
        let metadata = fs::symlink_metadata(&resolved)
            .map_err(|_| input_failure(CodeNoesisErrorV30::impact_invalid_workspace()))?;
        if metadata.file_type().is_symlink() {
            return Err(input_failure(CodeNoesisErrorV30::impact_invalid_workspace()));
        }
    }
    Ok(resolved)
}

pub(crate) fn read_stable_input(
    path: &Path,
    maximum: u64,
    component: &str,
) -> Result<Vec<u8>, ImpactGitFailure> {
    let path_identity_before =
        FileIdentity::from_path(path).map_err(|_| input_failure_for_component(component))?;
    let path_before =
        fs::symlink_metadata(path).map_err(|_| input_failure_for_component(component))?;
    if !path_before.is_file() || path_before.file_type().is_symlink() {
        return Err(input_failure_for_component(component));
    }
    let mut file = fs::File::open(path).map_err(|_| input_failure_for_component(component))?;
    let before = file
        .metadata()
        .map_err(|_| input_failure_for_component(component))?;
    let identity = FileIdentity::from_file(
        file.try_clone()
            .map_err(|_| input_failure_for_component(component))?,
    )
    .map_err(|_| input_failure_for_component(component))?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| input_failure_for_component(component))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum {
        return Err(operation_failure(
            CodeNoesisErrorV30::impact_limit_exceeded(),
        ));
    }
    file.rewind()
        .map_err(|_| unstable_failure_for_component(component))?;
    let mut verification = Vec::new();
    file.by_ref()
        .take(maximum.saturating_add(1))
        .read_to_end(&mut verification)
        .map_err(|_| unstable_failure_for_component(component))?;
    let after = file
        .metadata()
        .map_err(|_| unstable_failure_for_component(component))?;
    let path_after =
        fs::symlink_metadata(path).map_err(|_| unstable_failure_for_component(component))?;
    let path_identity_matches =
        FileIdentity::from_path(path).is_ok_and(|path_identity| path_identity == identity);
    if bytes != verification
        || path_identity_before != identity
        || before.len() != bytes.len() as u64
        || !same_file_metadata(&path_before, &before)
        || !same_file_metadata(&before, &after)
        || !same_file_metadata(&after, &path_after)
        || !path_identity_matches
        || path_after.file_type().is_symlink()
    {
        return Err(unstable_failure_for_component(component));
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

pub(crate) fn sha256(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(64);
    for byte in digest {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn input_failure_for_component(component: &str) -> ImpactGitFailure {
    if component == "report" {
        input_failure(CodeNoesisErrorV30::source_invalid_report())
    } else {
        input_failure(CodeNoesisErrorV30::impact_invalid_workspace())
    }
}

fn unstable_failure_for_component(component: &str) -> ImpactGitFailure {
    if component == "report" {
        operation_failure(CodeNoesisErrorV30::source_unstable_input())
    } else {
        operation_failure(CodeNoesisErrorV30::impact_unstable_input())
    }
}

fn workspace_failure(error: ImpactGitWorkspaceError) -> ImpactGitFailure {
    match error {
        ImpactGitWorkspaceError::Invalid => {
            input_failure(CodeNoesisErrorV30::impact_invalid_workspace())
        }
        ImpactGitWorkspaceError::TooManyClients { .. } => {
            operation_failure(CodeNoesisErrorV30::impact_limit_exceeded())
        }
    }
}

fn acquisition_failure(error: &GitImpactAcquisitionError) -> ImpactGitFailure {
    match error {
        GitImpactAcquisitionError::Repository(_) => {
            operation_failure(CodeNoesisErrorV30::impact_acquisition_rejected())
        }
        GitImpactAcquisitionError::InvalidSelection => {
            operation_failure(CodeNoesisErrorV30::impact_source_rejected())
        }
        GitImpactAcquisitionError::LimitExceeded => {
            operation_failure(CodeNoesisErrorV30::impact_limit_exceeded())
        }
    }
}

fn report_failure(error: SemanticReportV2Error) -> ImpactGitFailure {
    match error {
        SemanticReportV2Error::Invalid => {
            operation_failure(CodeNoesisErrorV30::impact_source_rejected())
        }
        SemanticReportV2Error::LimitExceeded => {
            operation_failure(CodeNoesisErrorV30::impact_limit_exceeded())
        }
        SemanticReportV2Error::Serialization => internal_failure(),
    }
}

fn input_failure(error: CodeNoesisErrorV30) -> ImpactGitFailure {
    ImpactGitFailure {
        error,
        exit_code: 2,
    }
}

fn operation_failure(error: CodeNoesisErrorV30) -> ImpactGitFailure {
    ImpactGitFailure {
        error,
        exit_code: 2,
    }
}

fn internal_failure() -> ImpactGitFailure {
    ImpactGitFailure {
        error: CodeNoesisErrorV30::source_internal(),
        exit_code: 1,
    }
}
