use crate::{EmptyStoreRollback, ScanWorker, current_envelope, run_confined_scan};
use codenoesis_application::{
    PublicationService, ScanError, ScanRequest, ScanService, s8_kotlin::KotlinScanError,
};
use codenoesis_contracts::{CodeNoesisErrorV4, CodeNoesisErrorV6, s8_kotlin as contracts};
use codenoesis_domain::{
    RepositoryIdentity, Revision,
    s8_kotlin::{KotlinError, MAX_OUTPUT_BYTES, PROFILE, SNAPSHOT_VERSION},
    storage::{ArtifactRole, LocalSnapshotHead},
};
use codenoesis_lang_kotlin::workspace::TreeSitterKotlinWorkspaceExtractor;
use codenoesis_ports::{ArtifactStore, NoopPublicationObserver};
use codenoesis_repository::LocalGitRepository;
use codenoesis_store_local::{LocalStore, ensure_store_root_for_boundary};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::io::{self, Write as _};
use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

struct Failure {
    value: Value,
    exit: u8,
}
impl Failure {
    fn new(code: &str, stage: &str, exit: u8) -> Self {
        Self {
            value: json!({"schema_version":"codenoesis.kotlin-error/v1","code":code,"stage":stage,"message":code,"retryable":false,"context":{}}),
            exit,
        }
    }
    fn input() -> Self {
        Self::new("input.invalid_kotlin_arguments", "input", 2)
    }
    fn internal() -> Self {
        Self::new("internal.kotlin_failure", "internal", 1)
    }
    fn artifact() -> Self {
        Self::new(
            "artifact.invalid_or_unsafe_kotlin_projection",
            "projection",
            16,
        )
    }
    fn extraction(error: KotlinError) -> Self {
        let code = match error {
            KotlinError::InvalidUtf8 => "extraction.kotlin_invalid_utf8",
            KotlinError::InvalidSyntax => "extraction.kotlin_invalid_syntax",
            KotlinError::NoKotlinSources => "extraction.no_kotlin_sources",
            KotlinError::InvalidContract => "contract.invalid_kotlin_graph",
            KotlinError::LimitExceeded(_) => "limit.kotlin_capacity_exceeded",
        };
        let mut failure = Self::new(code, "extraction", 11);
        if let KotlinError::LimitExceeded(limit) = error {
            failure.value["context"] = json!({"limit":limit});
        }
        failure
    }
    fn scan(error: ScanError) -> Self {
        let mut value = match error {
            ScanError::Acquisition(error) => CodeNoesisErrorV6::from_acquisition(&error)
                .canonical_stderr()
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
                .unwrap_or_else(|| Self::internal().value),
            ScanError::Storage(error) => CodeNoesisErrorV4::from_storage(&error).value().clone(),
            _ => return Self::internal(),
        };
        value["schema_version"] = json!("codenoesis.kotlin-error/v1");
        Self { value, exit: 10 }
    }
}

pub(crate) fn requested(arguments: &[OsString]) -> bool {
    arguments.iter().any(|a| {
        a == "--kotlin-profile"
            || a.to_str()
                .is_some_and(|s| s.starts_with("--kotlin-profile="))
    }) || arguments
        .windows(2)
        .any(|p| p[0] == "--profile" && p[1] == "standard-local-s8")
}
pub(crate) fn security_failure() -> ExitCode {
    emit(&Failure::new(
        "security.kotlin_boundary_unavailable",
        "security",
        1,
    ))
}
pub(crate) fn entry(arguments: &[OsString], worker: Option<&mut ScanWorker>) -> ExitCode {
    match run(arguments, worker) {
        Ok(bytes) => {
            if io::stdout().lock().write_all(&bytes).is_ok() {
                ExitCode::SUCCESS
            } else {
                emit(&Failure::internal())
            }
        }
        Err(error) => emit(&error),
    }
}
fn emit(failure: &Failure) -> ExitCode {
    let bytes=contracts::canonical(&failure.value).unwrap_or_else(|_|b"{\"schema_version\":\"codenoesis.kotlin-error/v1\",\"code\":\"internal.serialization\"}\n".to_vec());
    let _ = io::stderr().lock().write_all(&bytes);
    ExitCode::from(failure.exit)
}

struct Invocation {
    command: String,
    options: BTreeMap<String, OsString>,
}
impl Invocation {
    fn parse(arguments: &[OsString]) -> Result<Self, Failure> {
        let command = arguments
            .get(1)
            .and_then(|a| a.to_str())
            .ok_or_else(Failure::input)?;
        let allowed: &[&str] = match command {
            "scan" => &[
                "--profile",
                "--kotlin-profile",
                "--repository",
                "--repository-identity",
                "--revision",
                "--store",
            ],
            "query" => &[
                "--kotlin-profile",
                "--store",
                "--repository-identity",
                "--search",
            ],
            "export" => &[
                "--kotlin-profile",
                "--store",
                "--repository-identity",
                "--output",
            ],
            "docs" => &["--kotlin-profile", "--store", "--repository-identity"],
            "explore" => &["--kotlin-profile", "--input", "--output"],
            _ => return Err(Failure::input()),
        };
        let mut options = BTreeMap::new();
        let mut args = arguments.iter().skip(2);
        while let Some(flag) = args.next() {
            let flag = flag
                .to_str()
                .filter(|f| allowed.contains(f))
                .ok_or_else(Failure::input)?;
            let value = args
                .next()
                .filter(|v| !v.is_empty())
                .ok_or_else(Failure::input)?;
            if options.insert(flag.to_owned(), value.clone()).is_some() {
                return Err(Failure::input());
            }
        }
        if options.get("--kotlin-profile").and_then(|v| v.to_str()) != Some(PROFILE) {
            return Err(Failure::input());
        }
        let invocation = Self {
            command: command.to_owned(),
            options,
        };
        for flag in allowed {
            invocation.get(flag)?;
        }
        if command == "scan" && invocation.text("--profile")? != "standard-local-s8" {
            return Err(Failure::input());
        }
        if command == "query" && invocation.text("--search")?.len() > 256 {
            return Err(Failure::input());
        }
        Ok(invocation)
    }
    fn get(&self, key: &str) -> Result<&OsStr, Failure> {
        self.options
            .get(key)
            .map(OsString::as_os_str)
            .ok_or_else(Failure::input)
    }
    fn text(&self, key: &str) -> Result<&str, Failure> {
        self.get(key)?.to_str().ok_or_else(Failure::input)
    }
    fn identity(&self) -> Result<RepositoryIdentity, Failure> {
        RepositoryIdentity::parse(self.text("--repository-identity")?).map_err(|_| Failure::input())
    }
}
fn run(arguments: &[OsString], worker: Option<&mut ScanWorker>) -> Result<Vec<u8>, Failure> {
    let invocation = Invocation::parse(arguments)?;
    match invocation.command.as_str() {
        "scan" => scan(&invocation, worker.ok_or_else(Failure::internal)?),
        "explore" => explore(&invocation),
        "query" | "export" | "docs" => project(&invocation),
        _ => Err(Failure::input()),
    }
}
fn scan(invocation: &Invocation, worker: &mut ScanWorker) -> Result<Vec<u8>, Failure> {
    let repository = invocation.get("--repository")?.to_owned();
    let store = invocation.get("--store")?.to_owned();
    let identity = invocation.identity()?;
    let revision = Revision::parse(invocation.text("--revision")?).map_err(|_| Failure::input())?;
    let started = Instant::now();
    let scan_repository = repository.clone();
    let snapshot = run_confined_scan(worker, repository.clone(), None, Vec::new(), move || {
        let envelope = current_envelope().ok_or_else(Failure::internal)?;
        ScanService::new(LocalGitRepository::new_packed_sha1())
            .scan_kotlin(
                ScanRequest::new(scan_repository, identity, revision, envelope),
                &TreeSitterKotlinWorkspaceExtractor,
            )
            .map_err(|e| match e {
                KotlinScanError::Scan(e) => Failure::scan(e),
                KotlinScanError::Extraction(e) => Failure::extraction(e),
            })
    })
    .map_err(|()| Failure::internal())??;
    if started.elapsed().as_millis() > 60_000 {
        return Err(Failure::extraction(KotlinError::LimitExceeded(
            "scan_wall_milliseconds",
        )));
    }
    let bytes = snapshot.canonical_stdout().map_err(Failure::extraction)?;
    let absent =
        std::fs::symlink_metadata(&store).is_err_and(|e| e.kind() == io::ErrorKind::NotFound);
    let mut rollback = EmptyStoreRollback::new(store.clone(), absent);
    ensure_store_root_for_boundary(Path::new(&repository), Path::new(&store))
        .map_err(|e| Failure::scan(ScanError::Storage(e)))?;
    noesis::install_s3_filesystem_boundary(&repository, &store).map_err(|_| Failure::internal())?;
    let mut local = LocalStore::open(Path::new(&repository), Path::new(&store))
        .map_err(|e| Failure::scan(ScanError::Storage(e)))?;
    PublicationService::publish_kotlin(
        &snapshot,
        &mut local.artifacts,
        &mut local.metadata,
        &mut NoopPublicationObserver,
    )
    .map_err(Failure::scan)?;
    rollback.disarm();
    Ok(bytes)
}
fn load(invocation: &Invocation) -> Result<(Value, LocalSnapshotHead), Failure> {
    let local = LocalStore::open_existing(Path::new(invocation.get("--store")?))
        .map_err(|e| Failure::scan(ScanError::Storage(e)))?;
    let head =
        PublicationService::load_head(&invocation.identity()?, &local.artifacts, &local.metadata)
            .map_err(Failure::scan)?
            .ok_or_else(Failure::artifact)?;
    if head.snapshot_schema_version != SNAPSHOT_VERSION {
        return Err(Failure::artifact());
    }
    let artifact = head
        .artifacts
        .iter()
        .find(|a| a.role == ArtifactRole::SnapshotSemantic && a.ordinal == 0)
        .ok_or_else(Failure::artifact)?;
    if artifact.byte_length > MAX_OUTPUT_BYTES as u64 {
        return Err(Failure::artifact());
    }
    let bytes = local
        .artifacts
        .read(&artifact.artifact_id, artifact.byte_length)
        .map_err(|e| Failure::scan(ScanError::Storage(e)))?;
    let semantic: Value = serde_json::from_slice(&bytes).map_err(|_| Failure::artifact())?;
    contracts::validate_stored(&semantic, &head).map_err(Failure::extraction)?;
    Ok((semantic, head))
}
fn project(invocation: &Invocation) -> Result<Vec<u8>, Failure> {
    let (semantic, head) = load(invocation)?;
    let store = invocation.get("--store")?;
    if invocation.command == "export" {
        let portable = contracts::portable(&semantic, &head).map_err(Failure::extraction)?;
        let bytes = contracts::canonical(&portable).map_err(Failure::extraction)?;
        let output = invocation.get("--output")?;
        let prepared =
            noesis::portable_explorer::kotlin::prepare(Path::new(store), Path::new(output), false)
                .map_err(|_| Failure::artifact())?;
        noesis::install_r8_export_filesystem_boundary(store, output)
            .map_err(|_| Failure::internal())?;
        return noesis::portable_explorer::kotlin::publish(&prepared, &bytes, false)
            .map_err(|_| Failure::artifact());
    }
    noesis::install_s4_query_filesystem_boundary(store, store).map_err(|_| Failure::internal())?;
    if invocation.command == "docs" {
        let graph = &semantic["knowledge_graph"];
        return contracts::canonical(&json!({"schema_version":"codenoesis.kotlin-documentation/v1","snapshot_id":head.snapshot_id.as_str(),"repository":semantic["repository"],"statements":graph["claims"],"entities":graph["entities"],"relationships":graph["relationships"],"evidence":graph["evidence"],"coverage_gaps":graph["coverage"]})).map_err(Failure::extraction);
    }
    let search = invocation.text("--search")?.to_lowercase();
    let graph = &semantic["knowledge_graph"];
    let matches: Vec<_> = graph["entities"]
        .as_array()
        .ok_or_else(Failure::artifact)?
        .iter()
        .filter(|e| {
            format!(
                "{} {}",
                e["name"].as_str().unwrap_or_default(),
                e["properties"]["path"].as_str().unwrap_or_default()
            )
            .to_lowercase()
            .contains(&search)
        })
        .collect();
    contracts::canonical(&json!({"schema_version":"codenoesis.kotlin-query/v1","snapshot_id":head.snapshot_id.as_str(),"total_matches":matches.len(),"truncated":matches.len()>100,"results":matches.into_iter().take(100).collect::<Vec<_>>()})).map_err(Failure::extraction)
}
fn explore(invocation: &Invocation) -> Result<Vec<u8>, Failure> {
    let input =
        noesis::portable_explorer::kotlin::input_path(Path::new(invocation.get("--input")?))
            .map_err(|_| Failure::artifact())?;
    let bytes = crate::impact_git::read_stable_input(&input, MAX_OUTPUT_BYTES as u64, "report")
        .map_err(|_| Failure::artifact())?;
    contracts::parse_portable(&bytes).map_err(Failure::extraction)?;
    let output = invocation.get("--output")?;
    let prepared = noesis::portable_explorer::kotlin::prepare(&input, Path::new(output), true)
        .map_err(|_| Failure::artifact())?;
    noesis::install_r8_explorer_filesystem_boundary(input.as_os_str(), output)
        .map_err(|_| Failure::internal())?;
    noesis::portable_explorer::kotlin::publish(&prepared, &bytes, true)
        .map_err(|_| Failure::artifact())
}
