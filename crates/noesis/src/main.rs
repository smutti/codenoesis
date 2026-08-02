//! `CodeNoesis` command-line entry point.

mod federation;
mod repository_boundaries;

use std::collections::BTreeMap;
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{self, Write};
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use codenoesis_application::{
    BoundaryScanError, PublicationService, RefreshError, RefreshService, RootPackageScanError,
    ScanError, ScanRequest, ScanService,
};
use codenoesis_contracts::{
    AnalysisCacheEntryV1, BoundaryManifestReason, CodeNoesisErrorV1, CodeNoesisErrorV2,
    CodeNoesisErrorV3, CodeNoesisErrorV4, CodeNoesisErrorV5, CodeNoesisErrorV6, CodeNoesisErrorV7,
    CodeNoesisErrorV8, CodeNoesisErrorV9, CodeNoesisErrorV10, DocumentationContractError,
    IncrementalRefreshReportError, IncrementalRefreshReportInput, IncrementalRefreshReportV1,
    NestedRepositoryUnavailableReason, QueryContractError, RepositorySnapshotV2Error,
    RepositorySnapshotV3, RepositorySnapshotV3Error, RepositorySnapshotV4,
    RepositorySnapshotV4Error, RepositorySnapshotV5, RepositorySnapshotV5Error,
    RepositorySnapshotV6, RepositorySnapshotV6Error, SnapshotEnvelopeV1, ValidatedS4Head,
    generate_documentation_v1, local_query_result_v1, validate_stored_snapshot_semantic_v4,
    validate_stored_snapshot_semantic_v5, validate_stored_snapshot_semantic_v6,
};
use codenoesis_domain::AcquisitionError;
use codenoesis_domain::knowledge::KnowledgeError;
use codenoesis_domain::s1_boundaries::LOCAL_GITLINKS_V1;
use codenoesis_domain::s1_boundaries::RepositoryBoundaryError;
use codenoesis_domain::s1_packed::LOCAL_GIT_SHA1_PACKED_V1;
use codenoesis_domain::s4::{
    S4_ONTOLOGY_VERSION, S4_TREE_SITTER_EXTRACTOR_VERSION, S4_WORKSPACE_EXTRACTOR_VERSION,
};
use codenoesis_domain::s4_r3::{R3_WORKSPACE_PROFILE, RootPackageWorkspaceError};
use codenoesis_domain::s5::{
    ANALYSIS_CACHE_SCHEMA_VERSION, AnalysisCacheEntry, DEPENDENCY_RULE_VERSION,
    EXTRACTION_CONTRACT_VERSION, IncrementalRuleOutcome, MAX_ANALYSIS_ENTRIES,
    MAX_REFRESH_WALL_MILLISECONDS, NORMALIZATION_VERSION, TARGET_SEMANTIC_PROFILE,
};
use codenoesis_domain::storage::{
    ArtifactRole, LocalSnapshotHead, SNAPSHOT_SCHEMA_VERSION_V4, SNAPSHOT_SCHEMA_VERSION_V5,
    SNAPSHOT_SCHEMA_VERSION_V6, StorageComponent, StorageError,
};
use codenoesis_domain::{
    InputError, LimitKind, RepositoryIdentity, Revision, STANDARD_LOCAL_S1_LIMITS, limit_exceeded,
};
use codenoesis_lang_rust::{TreeSitterRustExtractor, TreeSitterRustWorkspaceExtractor};
use codenoesis_ports::{AnalysisCacheStore, ArtifactStore, NoopPublicationObserver};
use codenoesis_repository::LocalGitRepository;
use codenoesis_store_local::{LocalStore, ensure_store_root_for_boundary};
use noesis::generated_docs::{
    GeneratedDocsError, load_validated_manifest, validate_documents_root_for_boundary,
    validate_output_root_for_generation,
};
use serde_json::Value;

static CORRELATION_SEQUENCE: AtomicU64 = AtomicU64::new(0);

type ScanJob = Box<dyn FnOnce() + Send + 'static>;

struct ScanWorker {
    sender: Option<mpsc::SyncSender<ScanJob>>,
    handle: Option<thread::JoinHandle<()>>,
}

impl ScanWorker {
    fn spawn() -> io::Result<Self> {
        let (sender, receiver) = mpsc::sync_channel::<ScanJob>(0);
        let handle = thread::Builder::new()
            .name("codenoesis-confined-scan".to_owned())
            .spawn(move || {
                if let Ok(job) = receiver.recv() {
                    job();
                }
            })?;
        Ok(Self {
            sender: Some(sender),
            handle: Some(handle),
        })
    }

    fn run<T, F>(&mut self, operation: F) -> Result<T, ()>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        let sender = self.sender.take().ok_or(())?;
        let (result_sender, result_receiver) = mpsc::sync_channel(0);
        sender
            .send(Box::new(move || {
                let _ = result_sender.send(operation());
            }))
            .map_err(|_| ())?;
        drop(sender);
        let result = result_receiver.recv().map_err(|_| ())?;
        self.handle.take().ok_or(())?.join().map_err(|_| ())?;
        Ok(result)
    }
}

impl Drop for ScanWorker {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[allow(clippy::too_many_lines)]
fn main() -> ExitCode {
    let arguments = env::args_os().collect::<Vec<_>>();
    let federation_requested = federation::requested(&arguments);
    let docs_requested = arguments.get(1).is_some_and(|value| value == "docs");
    let query_requested = arguments.get(1).is_some_and(|value| value == "query");
    let refresh_requested = arguments.get(1).is_some_and(|value| value == "refresh");
    let profiled = arguments
        .iter()
        .any(|argument| argument == OsStr::new("--profile"));
    let packed_acquisition_requested = arguments
        .iter()
        .any(|argument| argument == OsStr::new("--acquisition-profile"));
    let boundary_requested = option_requested(&arguments, "--repository-boundary-profile")
        || option_requested(&arguments, "--repository-boundary-manifest");
    let r3_requested = option_requested(&arguments, "--workspace-profile");
    let mut scan_worker = if r3_requested || boundary_requested {
        ScanWorker::spawn().ok()
    } else {
        None
    };
    if noesis::install_s0_security_boundary().is_err() {
        return emit_internal_error_v1();
    }
    let s4_requested = requested_profile(&arguments, "standard-local-s4");
    let s3_requested = requested_profile(&arguments, "standard-local-s3");
    let s4_error_lineage = s4_requested || docs_requested || query_requested;
    let s3_error_lineage = s3_requested || s4_error_lineage;
    let s2_requested = requested_profile(&arguments, "standard-local-s2");
    let result = if r3_requested || boundary_requested {
        run_s4(arguments, scan_worker.as_mut())
    } else if federation_requested {
        federation::run(arguments).map_err(Failure::S6)
    } else if refresh_requested {
        run_s5(arguments)
    } else if docs_requested {
        run_docs(arguments)
    } else if query_requested {
        run_query(arguments)
    } else if s4_requested {
        run_s4(arguments, None)
    } else if s3_requested {
        run_s3(arguments)
    } else if s2_requested {
        run_s2(arguments)
    } else if profiled {
        run_s1(arguments)
    } else {
        run_s0(arguments)
    };
    match result {
        Ok(stdout) => match io::stdout().lock().write_all(&stdout) {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) if federation_requested => emit_internal_error_v8(),
            Err(_) if refresh_requested => emit_internal_error_v7(),
            Err(_) if r3_requested => emit_internal_error_v10(),
            Err(_) if boundary_requested => emit_internal_error_v9(),
            Err(_) if packed_acquisition_requested => emit_internal_error_v6(),
            Err(_) if s3_error_lineage => emit_internal_error_v4(),
            Err(_) if s2_requested => emit_internal_error_v3(),
            Err(_) if profiled => emit_internal_error_v2(),
            Err(_) => emit_internal_error_v1(),
        },
        Err(Failure::S6(failure)) => emit_error_v8(&failure.error, failure.exit_code),
        Err(Failure::R3(failure)) => emit_error_v10(&failure.error, failure.exit_code),
        Err(Failure::V9(failure)) => emit_error_v9(&failure.error, failure.exit_code),
        Err(Failure::V6Input(error)) => emit_error_v6(&error, 2),
        Err(Failure::S5(failure)) => emit_error_v7(&failure.error, failure.exit_code),
        Err(Failure::Input(error)) if packed_acquisition_requested => {
            emit_error_v6(&CodeNoesisErrorV6::from_input(error), 2)
        }
        Err(Failure::Input(error)) if s3_error_lineage => {
            emit_error_v4(&CodeNoesisErrorV4::from_input(error), 2)
        }
        Err(Failure::Input(error)) if s2_requested => {
            emit_error_v3(&CodeNoesisErrorV3::from_input(error), 2)
        }
        Err(Failure::Input(error)) if profiled => {
            emit_error_v2(&CodeNoesisErrorV2::from_input(error), 2)
        }
        Err(Failure::Input(error)) => emit_error_v1(&CodeNoesisErrorV1::from_input(error), 2),
        Err(Failure::S4Input(error)) => emit_error_v5(&error, 2),
        Err(Failure::Scan(ScanError::Acquisition(error))) if packed_acquisition_requested => {
            emit_error_v6(&CodeNoesisErrorV6::from_acquisition(&error), 10)
        }
        Err(Failure::Scan(ScanError::Acquisition(error))) if s3_error_lineage => {
            emit_error_v4(&CodeNoesisErrorV4::from_acquisition(&error), 10)
        }
        Err(Failure::Scan(ScanError::Acquisition(error))) if s2_requested => {
            emit_error_v3(&CodeNoesisErrorV3::from_acquisition(&error), 10)
        }
        Err(Failure::Scan(ScanError::Acquisition(error))) if profiled => {
            emit_error_v2(&CodeNoesisErrorV2::from_acquisition(&error), 10)
        }
        Err(Failure::Scan(ScanError::Acquisition(error))) => {
            emit_error_v1(&CodeNoesisErrorV1::from_acquisition(&error), 10)
        }
        Err(Failure::Scan(ScanError::Knowledge(error))) if s3_error_lineage => {
            emit_error_v4(&CodeNoesisErrorV4::from_knowledge(&error), 11)
        }
        Err(Failure::Scan(ScanError::Knowledge(error))) if s2_requested => {
            emit_error_v3(&CodeNoesisErrorV3::from_knowledge(&error), 11)
        }
        Err(Failure::Scan(ScanError::Workspace(error))) if s4_requested => {
            emit_error_v5(&CodeNoesisErrorV5::from_workspace(&error), 11)
        }
        Err(Failure::Scan(ScanError::Storage(error))) if s3_error_lineage => {
            emit_error_v4(&CodeNoesisErrorV4::from_storage(&error), 12)
        }
        Err(Failure::Scan(ScanError::Storage(_))) if s2_requested => emit_internal_error_v3(),
        Err(Failure::Scan(
            ScanError::Knowledge(_) | ScanError::Workspace(_) | ScanError::Storage(_),
        )) if profiled => emit_internal_error_v2(),
        Err(Failure::Scan(ScanError::Internal) | Failure::Internal)
            if packed_acquisition_requested =>
        {
            emit_internal_error_v6()
        }
        Err(Failure::Scan(ScanError::Internal) | Failure::Internal) if s3_error_lineage => {
            emit_internal_error_v4()
        }
        Err(Failure::Scan(ScanError::Internal) | Failure::Internal) if s2_requested => {
            emit_internal_error_v3()
        }
        Err(Failure::Scan(ScanError::Internal) | Failure::Internal) if profiled => {
            emit_internal_error_v2()
        }
        Err(
            Failure::Scan(
                ScanError::Knowledge(_)
                | ScanError::Workspace(_)
                | ScanError::Storage(_)
                | ScanError::Internal,
            )
            | Failure::Internal,
        ) => emit_internal_error_v1(),
        Err(Failure::Docs(error)) => emit_docs_error(error),
        Err(Failure::Query(error)) => emit_query_error(error),
    }
}

fn run_s0(arguments: impl IntoIterator<Item = OsString>) -> Result<Vec<u8>, Failure> {
    let invocation = Invocation::parse(arguments, None).map_err(invocation_failure)?;
    let envelope = current_envelope().ok_or(Failure::Internal)?;
    let request = ScanRequest::new(
        invocation.repository,
        invocation.identity,
        invocation.revision,
        envelope,
    );
    ScanService::new(LocalGitRepository::new())
        .scan(request)
        .map_err(Failure::Scan)?
        .canonical_stdout()
        .map_err(|_| Failure::Internal)
}

fn run_s1(arguments: impl IntoIterator<Item = OsString>) -> Result<Vec<u8>, Failure> {
    let invocation =
        Invocation::parse(arguments, Some("standard-local-s1")).map_err(invocation_failure)?;
    let repository_adapter = repository_adapter(invocation.packed_sha1);
    let started_at = Instant::now();
    if s1_boundary_applies(&invocation.repository)
        && noesis::install_s1_filesystem_boundary(&invocation.repository).is_err()
    {
        return Err(Failure::Internal);
    }
    let envelope = current_envelope().ok_or(Failure::Internal)?;
    let request = ScanRequest::new(
        invocation.repository,
        invocation.identity,
        invocation.revision,
        envelope,
    );
    let stdout = ScanService::new(repository_adapter)
        .scan_s1(request)
        .map_err(Failure::Scan)?
        .canonical_stdout()
        .map_err(|error| match error {
            RepositorySnapshotV2Error::LimitExceeded(error) => {
                Failure::Scan(ScanError::Acquisition(error))
            }
            RepositorySnapshotV2Error::Serialization(_)
            | RepositorySnapshotV2Error::OutputLengthOverflow => Failure::Internal,
        })?;
    let elapsed = u64::try_from(started_at.elapsed().as_millis()).map_err(|_| Failure::Internal)?;
    if elapsed > STANDARD_LOCAL_S1_LIMITS.scan_wall_milliseconds {
        return Err(Failure::Scan(ScanError::Acquisition(limit_exceeded(
            LimitKind::ScanWallMilliseconds,
            elapsed,
        ))));
    }
    Ok(stdout)
}

fn run_s2(arguments: impl IntoIterator<Item = OsString>) -> Result<Vec<u8>, Failure> {
    let invocation =
        Invocation::parse(arguments, Some("standard-local-s2")).map_err(invocation_failure)?;
    let repository_adapter = repository_adapter(invocation.packed_sha1);
    let started_at = Instant::now();
    if s1_boundary_applies(&invocation.repository)
        && noesis::install_s1_filesystem_boundary(&invocation.repository).is_err()
    {
        return Err(Failure::Internal);
    }
    let envelope = current_envelope().ok_or(Failure::Internal)?;
    let request = ScanRequest::new(
        invocation.repository,
        invocation.identity,
        invocation.revision,
        envelope,
    );
    let stdout = ScanService::new(repository_adapter)
        .scan_s2(request, &TreeSitterRustExtractor::new())
        .map_err(Failure::Scan)?
        .canonical_stdout()
        .map_err(|error| match error {
            RepositorySnapshotV3Error::LimitExceeded(AcquisitionError::LimitExceeded {
                limit,
                maximum,
                observed,
            }) => Failure::Scan(ScanError::Knowledge(KnowledgeError::GraphLimitExceeded {
                limit: limit.as_str(),
                maximum,
                observed,
            })),
            RepositorySnapshotV3Error::LimitExceeded(_)
            | RepositorySnapshotV3Error::Serialization(_)
            | RepositorySnapshotV3Error::OutputLengthOverflow => Failure::Internal,
        })?;
    let elapsed = u64::try_from(started_at.elapsed().as_millis()).map_err(|_| Failure::Internal)?;
    if elapsed > STANDARD_LOCAL_S1_LIMITS.scan_wall_milliseconds {
        return Err(Failure::Scan(ScanError::Acquisition(limit_exceeded(
            LimitKind::ScanWallMilliseconds,
            elapsed,
        ))));
    }
    Ok(stdout)
}

fn run_s3(arguments: impl IntoIterator<Item = OsString>) -> Result<Vec<u8>, Failure> {
    let invocation =
        Invocation::parse(arguments, Some("standard-local-s3")).map_err(invocation_failure)?;
    let repository_adapter = repository_adapter(invocation.packed_sha1);
    let store = invocation
        .store
        .clone()
        .ok_or(Failure::Input(InputError::InvalidStoreRoot))?;
    let repository = invocation.repository.clone();
    let store_preparation = ensure_store_root_for_boundary(
        std::path::Path::new(&repository),
        std::path::Path::new(&store),
    );
    let boundary_result = if store_preparation.is_ok() {
        noesis::install_s3_filesystem_boundary(&repository, &store)
    } else if s1_boundary_applies(&repository) {
        noesis::install_s1_filesystem_boundary(&repository)
    } else {
        Ok(())
    };
    if boundary_result.is_err() {
        return Err(Failure::Internal);
    }
    let started_at = Instant::now();
    let envelope = current_envelope().ok_or(Failure::Internal)?;
    let request = ScanRequest::new(
        invocation.repository,
        invocation.identity,
        invocation.revision,
        envelope,
    );
    let snapshot = ScanService::new(repository_adapter)
        .scan_s2(request, &TreeSitterRustExtractor::new())
        .map_err(Failure::Scan)?;
    enforce_scan_deadline(started_at)?;
    store_preparation.map_err(|error| Failure::Scan(ScanError::Storage(error)))?;
    let mut local_store = LocalStore::open(
        std::path::Path::new(&repository),
        std::path::Path::new(&store),
    )
    .map_err(|error| Failure::Scan(ScanError::Storage(error)))?;
    PublicationService::publish(
        &snapshot,
        &mut local_store.artifacts,
        &mut local_store.metadata,
        &mut NoopPublicationObserver,
    )
    .map_err(Failure::Scan)?;
    serialize_v3(&snapshot)
}

fn run_s4(
    arguments: impl IntoIterator<Item = OsString>,
    scan_worker: Option<&mut ScanWorker>,
) -> Result<Vec<u8>, Failure> {
    let invocation =
        Invocation::parse(arguments, Some("standard-local-s4")).map_err(invocation_failure)?;
    if invocation.workspace_profile {
        let scan_worker = scan_worker.ok_or_else(r3_internal_failure)?;
        return if invocation.boundary_profile {
            run_s4_r3_boundaries(invocation, scan_worker)
        } else {
            run_s4_r3(invocation, scan_worker)
        };
    }
    if invocation.boundary_profile {
        return run_s4_boundaries(
            invocation,
            scan_worker.ok_or_else(boundary_internal_failure)?,
        );
    }
    let repository_adapter = repository_adapter(invocation.packed_sha1);
    let store = invocation
        .store
        .clone()
        .ok_or(Failure::Input(InputError::InvalidStoreRoot))?;
    let repository = invocation.repository.clone();
    let store_preparation = ensure_store_root_for_boundary(
        std::path::Path::new(&repository),
        std::path::Path::new(&store),
    );
    let boundary_result = if store_preparation.is_ok() {
        noesis::install_s3_filesystem_boundary(&repository, &store)
    } else if s1_boundary_applies(&repository) {
        noesis::install_s1_filesystem_boundary(&repository)
    } else {
        Ok(())
    };
    if boundary_result.is_err() {
        return Err(Failure::Internal);
    }
    let started_at = Instant::now();
    let envelope = current_envelope().ok_or(Failure::Internal)?;
    let request = ScanRequest::new(
        invocation.repository,
        invocation.identity,
        invocation.revision,
        envelope,
    );
    let scan = ScanService::new(repository_adapter)
        .scan_s4_with_analysis(request, &TreeSitterRustWorkspaceExtractor::new())
        .map_err(Failure::Scan)?;
    enforce_scan_deadline(started_at)?;
    store_preparation.map_err(|error| Failure::Scan(ScanError::Storage(error)))?;
    let mut local_store = LocalStore::open(
        std::path::Path::new(&repository),
        std::path::Path::new(&store),
    )
    .map_err(|error| Failure::Scan(ScanError::Storage(error)))?;
    PublicationService::publish_v4(
        &scan.snapshot,
        &mut local_store.artifacts,
        &mut local_store.metadata,
        &mut NoopPublicationObserver,
    )
    .map_err(Failure::Scan)?;
    for entry in &scan.analysis_cache_entries {
        let contract = AnalysisCacheEntryV1::from_domain(entry);
        if let Ok(bytes) = contract.canonical_bytes() {
            let _ = local_store
                .analysis_cache
                .stage_entry(&entry.analysis_cache_entry_id, &bytes);
        }
    }
    serialize_v4(&scan.snapshot)
}

fn run_s4_r3(invocation: Invocation, scan_worker: &mut ScanWorker) -> Result<Vec<u8>, Failure> {
    let store = invocation
        .store
        .clone()
        .ok_or(Failure::Input(InputError::InvalidStoreRoot))?;
    let repository = invocation.repository.clone();
    let started_at = Instant::now();
    let scan_repository = repository.clone();
    let scan = run_confined_scan(
        scan_worker,
        repository.clone(),
        None,
        Vec::new(),
        move || {
            let envelope = current_envelope().ok_or_else(r3_internal_failure)?;
            let request = ScanRequest::new(
                invocation.repository,
                invocation.identity,
                invocation.revision,
                envelope,
            );
            ScanService::new(repository_adapter(invocation.packed_sha1))
                .scan_s4_r3(request, &TreeSitterRustWorkspaceExtractor::new())
                .map_err(r3_scan_failure)
        },
    )
    .map_err(|()| r3_internal_failure())??;
    enforce_scan_deadline(started_at)?;
    let stdout = serialize_v6(&scan.snapshot)?;
    let store_was_absent = fs::symlink_metadata(std::path::Path::new(&store))
        .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound);
    let mut rollback = EmptyStoreRollback::new(store.clone(), store_was_absent);
    ensure_store_root_for_boundary(
        std::path::Path::new(&scan_repository),
        std::path::Path::new(&store),
    )
    .map_err(|error| Failure::Scan(ScanError::Storage(error)))?;
    noesis::install_s3_filesystem_boundary(&scan_repository, &store)
        .map_err(|_| r3_internal_failure())?;
    let mut local_store = LocalStore::open(
        std::path::Path::new(&scan_repository),
        std::path::Path::new(&store),
    )
    .map_err(|error| Failure::Scan(ScanError::Storage(error)))?;
    PublicationService::publish_v6(
        &scan.snapshot,
        &mut local_store.artifacts,
        &mut local_store.metadata,
        &mut NoopPublicationObserver,
    )
    .map_err(Failure::Scan)?;
    rollback.disarm();
    stage_analysis_cache_best_effort(&mut local_store, &scan.analysis_cache_entries);
    Ok(stdout)
}

fn run_s4_r3_boundaries(
    invocation: Invocation,
    scan_worker: &mut ScanWorker,
) -> Result<Vec<u8>, Failure> {
    let mut prepared = repository_boundaries::prepare(
        invocation.boundary_manifest.as_deref(),
        &invocation.identity,
        &invocation.revision,
    )
    .map_err(repository_boundary_input_failure)?;
    let store = invocation
        .store
        .clone()
        .ok_or(Failure::Input(InputError::InvalidStoreRoot))?;
    let repository = invocation.repository.clone();
    if let Some(canonical_store) = canonical_existing_or_absent_leaf(&store) {
        prepared.reject_overlaps(&canonical_store);
    }
    if let Ok(canonical_repository) = fs::canonicalize(std::path::Path::new(&repository)) {
        prepared.reject_overlaps(&canonical_repository);
    }
    let manifest_path = prepared.manifest_path;
    let nested_roots = prepared.nested_roots;
    let started_at = Instant::now();
    let scan_repository = repository.clone();
    let scan = run_confined_scan(
        scan_worker,
        repository.clone(),
        manifest_path.clone(),
        nested_roots.clone(),
        move || {
            let envelope = current_envelope().ok_or_else(r3_internal_failure)?;
            let request = ScanRequest::new(
                invocation.repository,
                invocation.identity,
                invocation.revision,
                envelope,
            );
            ScanService::new(repository_adapter(invocation.packed_sha1))
                .scan_s4_r3_boundaries(
                    request,
                    prepared.scan_input,
                    &TreeSitterRustWorkspaceExtractor::new(),
                    &repository_boundaries::Sha256BoundaryHasher,
                )
                .map_err(r3_scan_failure)
        },
    )
    .map_err(|()| r3_internal_failure())??;
    enforce_scan_deadline(started_at)?;
    let stdout = serialize_v6(&scan.snapshot)?;
    let store_was_absent = fs::symlink_metadata(std::path::Path::new(&store))
        .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound);
    let mut rollback = EmptyStoreRollback::new(store.clone(), store_was_absent);
    ensure_store_root_for_boundary(
        std::path::Path::new(&scan_repository),
        std::path::Path::new(&store),
    )
    .map_err(|error| Failure::Scan(ScanError::Storage(error)))?;
    noesis::install_s1_boundaries_filesystem_boundary(
        &scan_repository,
        &store,
        manifest_path.as_deref().map(std::path::Path::as_os_str),
        &nested_roots,
    )
    .map_err(|_| boundary_internal_failure())?;
    let mut local_store = LocalStore::open(
        std::path::Path::new(&scan_repository),
        std::path::Path::new(&store),
    )
    .map_err(|error| Failure::Scan(ScanError::Storage(error)))?;
    PublicationService::publish_v6(
        &scan.snapshot,
        &mut local_store.artifacts,
        &mut local_store.metadata,
        &mut NoopPublicationObserver,
    )
    .map_err(Failure::Scan)?;
    rollback.disarm();
    stage_analysis_cache_best_effort(&mut local_store, &scan.analysis_cache_entries);
    Ok(stdout)
}

fn stage_analysis_cache_best_effort(local_store: &mut LocalStore, entries: &[AnalysisCacheEntry]) {
    for entry in entries {
        let contract = AnalysisCacheEntryV1::from_domain(entry);
        if let Ok(bytes) = contract.canonical_bytes() {
            let _ = local_store
                .analysis_cache
                .stage_entry(&entry.analysis_cache_entry_id, &bytes);
        }
    }
}

fn run_s4_boundaries(
    invocation: Invocation,
    scan_worker: &mut ScanWorker,
) -> Result<Vec<u8>, Failure> {
    let mut prepared = repository_boundaries::prepare(
        invocation.boundary_manifest.as_deref(),
        &invocation.identity,
        &invocation.revision,
    )
    .map_err(repository_boundary_input_failure)?;
    let store = invocation
        .store
        .clone()
        .ok_or(Failure::Input(InputError::InvalidStoreRoot))?;
    let repository = invocation.repository.clone();
    if let Some(canonical_store) = canonical_existing_or_absent_leaf(&store) {
        prepared.reject_overlaps(&canonical_store);
    }
    if let Ok(canonical_repository) = fs::canonicalize(std::path::Path::new(&repository)) {
        prepared.reject_overlaps(&canonical_repository);
    }
    let manifest_path = prepared.manifest_path;
    let nested_roots = prepared.nested_roots;
    let started_at = Instant::now();
    let scan_repository = repository.clone();
    let scan = run_confined_scan(
        scan_worker,
        repository.clone(),
        manifest_path.clone(),
        nested_roots.clone(),
        move || {
            let envelope = current_envelope().ok_or_else(boundary_internal_failure)?;
            let request = ScanRequest::new(
                invocation.repository,
                invocation.identity,
                invocation.revision,
                envelope,
            );
            ScanService::new(repository_adapter(invocation.packed_sha1))
                .scan_s4_boundaries(
                    request,
                    prepared.scan_input,
                    &TreeSitterRustWorkspaceExtractor::new(),
                    &repository_boundaries::Sha256BoundaryHasher,
                )
                .map_err(boundary_scan_failure)
        },
    )
    .map_err(|()| boundary_internal_failure())??;
    enforce_scan_deadline(started_at)?;
    let stdout = serialize_v5(&scan.snapshot)?;
    let store_was_absent = fs::symlink_metadata(std::path::Path::new(&store))
        .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound);
    let mut rollback = EmptyStoreRollback::new(store.clone(), store_was_absent);
    ensure_store_root_for_boundary(
        std::path::Path::new(&scan_repository),
        std::path::Path::new(&store),
    )
    .map_err(|error| Failure::Scan(ScanError::Storage(error)))?;
    noesis::install_s1_boundaries_filesystem_boundary(
        &scan_repository,
        &store,
        manifest_path.as_deref().map(std::path::Path::as_os_str),
        &nested_roots,
    )
    .map_err(|_| boundary_internal_failure())?;
    let mut local_store = LocalStore::open(
        std::path::Path::new(&scan_repository),
        std::path::Path::new(&store),
    )
    .map_err(|error| Failure::Scan(ScanError::Storage(error)))?;
    PublicationService::publish_v5(
        &scan.snapshot,
        &mut local_store.artifacts,
        &mut local_store.metadata,
        &mut NoopPublicationObserver,
    )
    .map_err(Failure::Scan)?;
    rollback.disarm();
    for entry in &scan.analysis_cache_entries {
        let contract = AnalysisCacheEntryV1::from_domain(entry);
        if let Ok(bytes) = contract.canonical_bytes() {
            let _ = local_store
                .analysis_cache
                .stage_entry(&entry.analysis_cache_entry_id, &bytes);
        }
    }
    Ok(stdout)
}

fn run_confined_scan<T, F>(
    scan_worker: &mut ScanWorker,
    repository: OsString,
    manifest_path: Option<std::path::PathBuf>,
    nested_roots: Vec<std::path::PathBuf>,
    operation: F,
) -> Result<T, ()>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    scan_worker
        .run(move || {
            let boundary_installed = if let Some(manifest_path) = manifest_path {
                let mut repository_roots = Vec::with_capacity(nested_roots.len() + 1);
                repository_roots.push(std::path::PathBuf::from(&repository));
                repository_roots.extend(nested_roots);
                noesis::install_s6_filesystem_boundary(manifest_path.as_os_str(), &repository_roots)
                    .is_ok()
            } else if nested_roots.is_empty() && s1_boundary_applies(&repository) {
                noesis::install_s1_filesystem_boundary(&repository).is_ok()
            } else {
                nested_roots.is_empty()
            };
            boundary_installed.then(operation)
        })?
        .ok_or(())
}

fn canonical_existing_or_absent_leaf(path: &OsStr) -> Option<std::path::PathBuf> {
    let path = std::path::Path::new(path);
    if path.exists() {
        return fs::canonicalize(path).ok();
    }
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir().ok()?.join(path)
    };
    let parent = fs::canonicalize(absolute.parent()?).ok()?;
    Some(parent.join(absolute.file_name()?))
}

struct EmptyStoreRollback {
    path: OsString,
    armed: bool,
}

impl EmptyStoreRollback {
    const fn new(path: OsString, armed: bool) -> Self {
        Self { path, armed }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for EmptyStoreRollback {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_dir(std::path::Path::new(&self.path));
        }
    }
}

#[allow(clippy::too_many_lines)]
fn run_s5(arguments: impl IntoIterator<Item = OsString>) -> Result<Vec<u8>, Failure> {
    let invocation = Invocation::parse_command(arguments, "refresh", Some("standard-local-s5"))
        .map_err(s5_invocation_failure)?;
    let store = invocation
        .store
        .clone()
        .ok_or_else(|| s5_input_failure(InputError::InvalidStoreRoot))?;
    let repository = invocation.repository.clone();
    let repository_identity = invocation.identity.as_str().to_owned();
    if fs::symlink_metadata(std::path::Path::new(&store))
        .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
    {
        return Err(s5_failure(
            CodeNoesisErrorV7::baseline_missing(&repository_identity),
            15,
        ));
    }
    let store_preparation = ensure_store_root_for_boundary(
        std::path::Path::new(&repository),
        std::path::Path::new(&store),
    );
    let boundary_result = if store_preparation.is_ok() {
        noesis::install_s3_filesystem_boundary(&repository, &store)
    } else if s1_boundary_applies(&repository) {
        noesis::install_s1_filesystem_boundary(&repository)
    } else {
        Ok(())
    };
    if boundary_result.is_err() {
        return Err(s5_internal_failure());
    }
    store_preparation.map_err(|error| s5_storage_failure(&error))?;
    let started_at = Instant::now();
    let mut local_store =
        LocalStore::open_existing(std::path::Path::new(&store)).map_err(|error| {
            if error == StorageError::UnmarkedNonemptyRoot {
                s5_failure(
                    CodeNoesisErrorV7::baseline_missing(&repository_identity),
                    15,
                )
            } else {
                s5_storage_failure(&error)
            }
        })?;
    let baseline = load_s5_baseline(&local_store, &invocation.identity, &repository_identity)?;
    let cache = load_s5_analysis_cache(&local_store, &baseline)?;
    enforce_refresh_deadline(started_at)?;

    let baseline_documentation = generate_documentation_v1(
        baseline.semantic(),
        baseline.head().snapshot_id.as_str(),
        &baseline.head().semantic_hash.value,
    )
    .map_err(|_| {
        s5_failure(
            CodeNoesisErrorV7::baseline_incompatible(
                SNAPSHOT_SCHEMA_VERSION_V4,
                &baseline.head().snapshot_schema_version,
            ),
            15,
        )
    })?;
    let envelope = current_envelope().ok_or_else(s5_internal_failure)?;
    let request = ScanRequest::new(
        invocation.repository,
        invocation.identity,
        invocation.revision,
        envelope,
    );
    let plan = RefreshService::new(LocalGitRepository::new())
        .plan(
            request,
            &baseline,
            &cache.entries,
            baseline.supports_current_s5_versions() && cache.versions_compatible,
            &TreeSitterRustWorkspaceExtractor::new(),
        )
        .map_err(|error| s5_refresh_failure(error, &baseline, &repository_identity))?;
    enforce_refresh_deadline(started_at)?;

    let target_candidate = plan
        .target_snapshot
        .publication_candidate()
        .map_err(|_| s5_internal_failure())?;
    let target_semantic = plan
        .target_snapshot
        .value()
        .get("semantic")
        .ok_or_else(s5_internal_failure)?;
    let target_documentation = generate_documentation_v1(
        target_semantic,
        target_candidate.snapshot.snapshot_id.as_str(),
        &target_candidate.snapshot.semantic_hash.value,
    )
    .map_err(|_| {
        s5_failure(
            CodeNoesisErrorV7::cold_equivalence_failed(
                "documentation",
                &target_candidate.snapshot.semantic_hash.value,
                "invalid",
            ),
            15,
        )
    })?;
    let report = IncrementalRefreshReportV1::new(&IncrementalRefreshReportInput {
        baseline: &baseline,
        target: &plan.target_snapshot,
        baseline_documentation: &baseline_documentation,
        target_documentation: &target_documentation,
        changed_paths: &plan.changed_paths,
        baseline_cache_entries: &plan.baseline_cache_entries,
        target_extraction: &plan.target_extraction,
        rule: plan.rule,
    })
    .map_err(|error| {
        s5_report_failure(
            error,
            &baseline,
            &target_candidate.snapshot.semantic_hash.value,
        )
    })?;
    let stdout = report
        .canonical_stdout()
        .map_err(|_| s5_internal_failure())?;
    enforce_refresh_deadline(started_at)?;

    if plan.rule == IncrementalRuleOutcome::NoChange {
        return Ok(stdout);
    }
    stage_s5_analysis_cache(
        &mut local_store,
        &plan.baseline_cache_entries,
        &plan.target_extraction.cache_entries,
    )?;
    enforce_refresh_deadline(started_at)?;
    PublicationService::publish_v4_expected(
        &plan.target_snapshot,
        &baseline.head().snapshot_id,
        &mut local_store.artifacts,
        &mut local_store.metadata,
        &mut NoopPublicationObserver,
    )
    .map_err(s5_scan_failure)?;
    Ok(stdout)
}

fn run_docs(arguments: impl IntoIterator<Item = OsString>) -> Result<Vec<u8>, Failure> {
    let invocation = DocsInvocation::parse(arguments)?;
    let loaded =
        load_s4_snapshot(&invocation.store, &invocation.identity).map_err(|error| match error {
            LoadS4Error::Scan(error) => Failure::Scan(error),
            LoadS4Error::SnapshotMismatch => Failure::Docs(GeneratedDocsError::SnapshotMismatch),
        })?;
    validate_output_root_for_generation(
        std::path::Path::new(&invocation.store),
        std::path::Path::new(&invocation.output),
    )
    .map_err(|error| match error {
        GeneratedDocsError::InvalidRoot => {
            Failure::S4Input(CodeNoesisErrorV5::invalid_output_root())
        }
        other => Failure::Docs(other),
    })?;
    let generated = generate_documentation_v1(
        &loaded.semantic,
        loaded.head.snapshot_id.as_str(),
        &loaded.head.semantic_hash.value,
    )
    .map_err(|error| match error {
        DocumentationContractError::InvalidSnapshot | DocumentationContractError::LimitExceeded => {
            Failure::Docs(GeneratedDocsError::Failed)
        }
    })?;
    noesis::generated_docs::ensure_output_root_for_boundary(
        std::path::Path::new(&invocation.store),
        std::path::Path::new(&invocation.output),
    )
    .map_err(|error| match error {
        GeneratedDocsError::InvalidRoot => {
            Failure::S4Input(CodeNoesisErrorV5::invalid_output_root())
        }
        other => Failure::Docs(other),
    })?;
    noesis::install_s4_docs_filesystem_boundary(&invocation.store, &invocation.output)
        .map_err(|_| Failure::Internal)?;
    noesis::generated_docs::publish(
        std::path::Path::new(&invocation.store),
        std::path::Path::new(&invocation.output),
        &generated,
    )
    .map_err(Failure::Docs)?;
    generated
        .canonical_manifest_stdout()
        .map_err(|_| Failure::Internal)
}

fn run_query(arguments: impl IntoIterator<Item = OsString>) -> Result<Vec<u8>, Failure> {
    let invocation = QueryInvocation::parse(arguments)?;
    let loaded =
        load_s4_snapshot(&invocation.store, &invocation.identity).map_err(|error| match error {
            LoadS4Error::Scan(error) => Failure::Scan(error),
            LoadS4Error::SnapshotMismatch => Failure::Query(QueryFailure::SnapshotMismatch),
        })?;
    validate_documents_root_for_boundary(
        std::path::Path::new(&invocation.store),
        std::path::Path::new(&invocation.documents),
    )
    .map_err(|_| Failure::S4Input(CodeNoesisErrorV5::invalid_documents_root()))?;
    noesis::install_s4_query_filesystem_boundary(&invocation.store, &invocation.documents)
        .map_err(|_| Failure::Internal)?;
    let manifest = load_validated_manifest(
        std::path::Path::new(&invocation.documents),
        invocation.identity.as_str(),
        loaded.head.snapshot_id.as_str(),
        &loaded.head.semantic_hash.value,
    )
    .map_err(|error| match error {
        GeneratedDocsError::SnapshotMismatch => Failure::Query(QueryFailure::SnapshotMismatch),
        GeneratedDocsError::InvalidRoot
        | GeneratedDocsError::UnmarkedNonemptyRoot
        | GeneratedDocsError::UnsafePath
        | GeneratedDocsError::CorruptGeneration
        | GeneratedDocsError::Failed => Failure::Query(QueryFailure::CorruptDocuments),
    })?;
    local_query_result_v1(
        &loaded.semantic,
        &manifest,
        loaded.head.snapshot_id.as_str(),
        &invocation.requested_id,
    )
    .map_err(|error| match error {
        QueryContractError::NotFound => Failure::Query(QueryFailure::NotFound),
        QueryContractError::InvalidDocuments => Failure::Query(QueryFailure::CorruptDocuments),
        QueryContractError::InvalidSnapshot => Failure::Query(QueryFailure::SnapshotMismatch),
        QueryContractError::LimitExceeded => Failure::Query(QueryFailure::LimitExceeded),
    })?
    .canonical_stdout()
    .map_err(|error| match error {
        QueryContractError::LimitExceeded => Failure::Query(QueryFailure::LimitExceeded),
        QueryContractError::InvalidSnapshot
        | QueryContractError::InvalidDocuments
        | QueryContractError::NotFound => Failure::Internal,
    })
}

fn load_s4_snapshot(
    store: &OsStr,
    identity: &RepositoryIdentity,
) -> Result<LoadedS4Snapshot, LoadS4Error> {
    let local_store = LocalStore::open_existing(std::path::Path::new(store))
        .map_err(|error| LoadS4Error::Scan(ScanError::Storage(error)))?;
    load_s4_snapshot_from_store(&local_store, identity)
}

fn load_s4_snapshot_from_store(
    local_store: &LocalStore,
    identity: &RepositoryIdentity,
) -> Result<LoadedS4Snapshot, LoadS4Error> {
    let head =
        PublicationService::load_head(identity, &local_store.artifacts, &local_store.metadata)
            .map_err(LoadS4Error::Scan)?
            .ok_or(LoadS4Error::Scan(ScanError::Storage(
                StorageError::CorruptMetadata {
                    component: StorageComponent::Head,
                    reason: "current_head_missing",
                    snapshot_id: None,
                },
            )))?;
    if !matches!(
        head.snapshot_schema_version.as_str(),
        SNAPSHOT_SCHEMA_VERSION_V4 | SNAPSHOT_SCHEMA_VERSION_V5 | SNAPSHOT_SCHEMA_VERSION_V6
    ) {
        return Err(LoadS4Error::SnapshotMismatch);
    }
    let snapshot = head
        .artifacts
        .iter()
        .find(|artifact| artifact.role == ArtifactRole::SnapshotSemantic && artifact.ordinal == 0)
        .ok_or_else(|| {
            LoadS4Error::Scan(ScanError::Storage(StorageError::CorruptMetadata {
                component: StorageComponent::Head,
                reason: "snapshot_semantic_missing",
                snapshot_id: Some(head.snapshot_id.to_string()),
            }))
        })?;
    let bytes = local_store
        .artifacts
        .read(&snapshot.artifact_id, snapshot.byte_length)
        .map_err(|error| LoadS4Error::Scan(ScanError::Storage(error)))?;
    let semantic = serde_json::from_slice::<Value>(&bytes).map_err(|_| {
        LoadS4Error::Scan(ScanError::Storage(StorageError::CorruptMetadata {
            component: StorageComponent::Head,
            reason: "snapshot_semantic_invalid",
            snapshot_id: Some(head.snapshot_id.to_string()),
        }))
    })?;
    match head.snapshot_schema_version.as_str() {
        SNAPSHOT_SCHEMA_VERSION_V4 => validate_stored_snapshot_semantic_v4(&semantic, &head),
        SNAPSHOT_SCHEMA_VERSION_V5 => validate_stored_snapshot_semantic_v5(&semantic, &head),
        SNAPSHOT_SCHEMA_VERSION_V6 => validate_stored_snapshot_semantic_v6(&semantic, &head),
        _ => return Err(LoadS4Error::SnapshotMismatch),
    }
    .map_err(|error| LoadS4Error::Scan(ScanError::Storage(error)))?;
    if semantic
        .pointer("/repository/identity")
        .and_then(Value::as_str)
        != Some(identity.as_str())
    {
        return Err(LoadS4Error::SnapshotMismatch);
    }
    Ok(LoadedS4Snapshot { head, semantic })
}

fn load_s5_baseline(
    local_store: &LocalStore,
    identity: &RepositoryIdentity,
    repository_identity: &str,
) -> Result<ValidatedS4Head, Failure> {
    let loaded =
        load_s4_snapshot_from_store(local_store, identity).map_err(|error| match error {
            LoadS4Error::Scan(ScanError::Storage(StorageError::CorruptMetadata {
                reason: "current_head_missing",
                ..
            })) => s5_failure(CodeNoesisErrorV7::baseline_missing(repository_identity), 15),
            LoadS4Error::SnapshotMismatch => s5_failure(
                CodeNoesisErrorV7::baseline_incompatible(SNAPSHOT_SCHEMA_VERSION_V4, "invalid"),
                15,
            ),
            LoadS4Error::Scan(error) => s5_scan_failure(error),
        })?;
    if loaded.head.snapshot_schema_version != SNAPSHOT_SCHEMA_VERSION_V4 {
        return Err(s5_failure(
            CodeNoesisErrorV7::baseline_incompatible(
                SNAPSHOT_SCHEMA_VERSION_V4,
                &loaded.head.snapshot_schema_version,
            ),
            15,
        ));
    }
    if loaded.head.repository_identity != *identity {
        return Err(s5_failure(
            CodeNoesisErrorV7::baseline_repository_mismatch(
                repository_identity,
                loaded.head.repository_identity.as_str(),
            ),
            15,
        ));
    }
    ValidatedS4Head::new(loaded.semantic, loaded.head).map_err(|error| s5_storage_failure(&error))
}

fn load_s5_analysis_cache(
    local_store: &LocalStore,
    baseline: &ValidatedS4Head,
) -> Result<LoadedS5AnalysisCache, Failure> {
    let stored = local_store.analysis_cache.load_entries().map_err(|error| {
        s5_cache_storage_failure(&error, "analysis-cache/layout", "invalid", "invalid")
    })?;
    if stored.len() > MAX_ANALYSIS_ENTRIES {
        return Err(s5_failure(
            CodeNoesisErrorV7::limit_exceeded(
                "analysis_entries",
                MAX_ANALYSIS_ENTRIES as u64,
                u64::try_from(stored.len()).unwrap_or(u64::MAX),
            ),
            15,
        ));
    }
    let baseline_inventory = baseline
        .inventory_blobs()
        .map_err(|error| s5_storage_failure(&error))?
        .into_iter()
        .map(|file| (file.path, file.blob_oid))
        .collect::<BTreeMap<_, _>>();
    let mut entries = Vec::new();
    let mut versions_compatible = true;
    for (stored_id, bytes) in stored {
        let expected_hash = stored_id
            .strip_prefix("urn:codenoesis:analysis-cache-entry:blake3:")
            .unwrap_or("invalid");
        let path = format!("analysis-cache/{expected_hash}");
        let raw = serde_json::from_slice::<Value>(&bytes)
            .map_err(|_| s5_cache_failure(&path, expected_hash, "invalid"))?;
        let observed_hash = raw
            .get("payload_hash")
            .and_then(Value::as_str)
            .map_or_else(|| "invalid".to_owned(), str::to_owned);
        if cache_versions_compatible(&raw) == Some(false) {
            match cache_entry_matches_baseline(
                &raw,
                baseline.head().repository_identity.as_str(),
                &baseline_inventory,
            ) {
                Some(true) => versions_compatible = false,
                None if raw.get("schema_version").and_then(Value::as_str)
                    == Some(ANALYSIS_CACHE_SCHEMA_VERSION) =>
                {
                    return Err(s5_cache_failure(&path, expected_hash, &observed_hash));
                }
                Some(false) | None => {}
            }
            continue;
        }
        let contract = AnalysisCacheEntryV1::parse(&bytes)
            .map_err(|_| s5_cache_failure(&path, expected_hash, &observed_hash))?;
        let entry = contract
            .to_domain()
            .map_err(|_| s5_cache_failure(&path, expected_hash, &observed_hash))?;
        if entry.analysis_cache_entry_id != stored_id {
            return Err(s5_cache_failure(
                &path,
                expected_hash,
                entry
                    .analysis_cache_entry_id
                    .strip_prefix("urn:codenoesis:analysis-cache-entry:blake3:")
                    .unwrap_or("invalid"),
            ));
        }
        if entry.key.repository_identity == baseline.head().repository_identity.as_str()
            && baseline_inventory
                .get(&entry.key.canonical_source_path)
                .is_some_and(|blob_oid| blob_oid == &entry.key.source_blob_oid)
        {
            entries.push(entry);
        }
    }
    entries.sort_by(|left, right| {
        left.analysis_cache_entry_id
            .as_bytes()
            .cmp(right.analysis_cache_entry_id.as_bytes())
    });
    if entries
        .windows(2)
        .any(|pair| pair[0].analysis_cache_entry_id == pair[1].analysis_cache_entry_id)
    {
        return Err(s5_cache_failure(
            "analysis-cache/duplicate",
            "invalid",
            "invalid",
        ));
    }
    Ok(LoadedS5AnalysisCache {
        entries,
        versions_compatible,
    })
}

fn cache_versions_compatible(value: &Value) -> Option<bool> {
    if value.get("schema_version")?.as_str()? != ANALYSIS_CACHE_SCHEMA_VERSION {
        return Some(false);
    }
    let versions = value.get("versions")?;
    Some(
        versions.get("language_extractor")?.as_str()? == S4_TREE_SITTER_EXTRACTOR_VERSION
            && versions.get("workspace_mapper")?.as_str()? == S4_WORKSPACE_EXTRACTOR_VERSION
            && versions.get("normalization")?.as_str()? == NORMALIZATION_VERSION
            && versions.get("ontology")?.as_str()? == S4_ONTOLOGY_VERSION
            && versions.get("extraction_contract")?.as_str()? == EXTRACTION_CONTRACT_VERSION
            && versions.get("semantic_profile")?.as_str()? == TARGET_SEMANTIC_PROFILE
            && versions.get("dependency_rules")?.as_str()? == DEPENDENCY_RULE_VERSION,
    )
}

fn cache_entry_matches_baseline(
    value: &Value,
    repository_identity: &str,
    baseline_inventory: &BTreeMap<String, String>,
) -> Option<bool> {
    let source = value.get("source")?;
    let path = source.get("path")?.as_str()?;
    let blob_oid = source.get("blob_oid")?.as_str()?;
    Some(
        value.get("repository_identity")?.as_str()? == repository_identity
            && baseline_inventory
                .get(path)
                .is_some_and(|baseline_blob| baseline_blob == blob_oid),
    )
}

fn stage_s5_analysis_cache(
    local_store: &mut LocalStore,
    baseline_entries: &[AnalysisCacheEntry],
    target_entries: &[AnalysisCacheEntry],
) -> Result<(), Failure> {
    let mut entries = BTreeMap::<String, &AnalysisCacheEntry>::new();
    for entry in baseline_entries.iter().chain(target_entries) {
        if entries
            .insert(entry.analysis_cache_entry_id.clone(), entry)
            .is_some_and(|existing| existing != entry)
        {
            return Err(s5_cache_failure(
                "analysis-cache/conflict",
                "invalid",
                "invalid",
            ));
        }
    }
    for (entry_id, entry) in entries {
        let digest = entry_id
            .strip_prefix("urn:codenoesis:analysis-cache-entry:blake3:")
            .unwrap_or("invalid");
        let path = format!("analysis-cache/{digest}");
        let bytes = AnalysisCacheEntryV1::from_domain(entry)
            .canonical_bytes()
            .map_err(|_| s5_internal_failure())?;
        local_store
            .analysis_cache
            .stage_entry(&entry_id, &bytes)
            .map_err(|error| s5_cache_storage_failure(&error, &path, digest, "invalid"))?;
    }
    Ok(())
}

fn serialize_v3(snapshot: &RepositorySnapshotV3) -> Result<Vec<u8>, Failure> {
    snapshot.canonical_stdout().map_err(|error| match error {
        RepositorySnapshotV3Error::LimitExceeded(AcquisitionError::LimitExceeded {
            limit,
            maximum,
            observed,
        }) => Failure::Scan(ScanError::Knowledge(KnowledgeError::GraphLimitExceeded {
            limit: limit.as_str(),
            maximum,
            observed,
        })),
        RepositorySnapshotV3Error::LimitExceeded(_)
        | RepositorySnapshotV3Error::Serialization(_)
        | RepositorySnapshotV3Error::OutputLengthOverflow => Failure::Internal,
    })
}

fn serialize_v4(snapshot: &RepositorySnapshotV4) -> Result<Vec<u8>, Failure> {
    snapshot.canonical_stdout().map_err(|error| match error {
        RepositorySnapshotV4Error::LimitExceeded(error) => {
            Failure::Scan(ScanError::Acquisition(error))
        }
        RepositorySnapshotV4Error::Serialization(_)
        | RepositorySnapshotV4Error::OutputLengthOverflow => Failure::Internal,
    })
}

fn serialize_v5(snapshot: &RepositorySnapshotV5) -> Result<Vec<u8>, Failure> {
    snapshot.canonical_stdout().map_err(|error| match error {
        RepositorySnapshotV5Error::Boundary(error) => boundary_error_failure(&error),
        RepositorySnapshotV5Error::LimitExceeded(error) => {
            Failure::Scan(ScanError::Acquisition(error))
        }
        RepositorySnapshotV5Error::Serialization(_)
        | RepositorySnapshotV5Error::OutputLengthOverflow => boundary_internal_failure(),
    })
}

fn serialize_v6(snapshot: &RepositorySnapshotV6) -> Result<Vec<u8>, Failure> {
    snapshot.canonical_stdout().map_err(|error| match error {
        RepositorySnapshotV6Error::LimitExceeded(error) => {
            Failure::Scan(ScanError::Acquisition(error))
        }
        RepositorySnapshotV6Error::Serialization(_)
        | RepositorySnapshotV6Error::ContractInvalid
        | RepositorySnapshotV6Error::OutputLengthOverflow => r3_internal_failure(),
    })
}

fn enforce_scan_deadline(started_at: Instant) -> Result<(), Failure> {
    let elapsed = u64::try_from(started_at.elapsed().as_millis()).map_err(|_| Failure::Internal)?;
    if elapsed > STANDARD_LOCAL_S1_LIMITS.scan_wall_milliseconds {
        return Err(Failure::Scan(ScanError::Acquisition(limit_exceeded(
            LimitKind::ScanWallMilliseconds,
            elapsed,
        ))));
    }
    Ok(())
}

fn enforce_refresh_deadline(started_at: Instant) -> Result<(), Failure> {
    let elapsed =
        u64::try_from(started_at.elapsed().as_millis()).map_err(|_| s5_internal_failure())?;
    if elapsed > MAX_REFRESH_WALL_MILLISECONDS {
        return Err(s5_failure(
            CodeNoesisErrorV7::limit_exceeded(
                "refresh_wall_milliseconds",
                MAX_REFRESH_WALL_MILLISECONDS,
                elapsed,
            ),
            15,
        ));
    }
    Ok(())
}

fn s5_failure(error: CodeNoesisErrorV7, exit_code: u8) -> Failure {
    Failure::S5(S5Failure { error, exit_code })
}

fn s5_input_failure(error: InputError) -> Failure {
    s5_failure(CodeNoesisErrorV7::from_input(error), 2)
}

fn invocation_failure(error: InvocationError) -> Failure {
    match error {
        InvocationError::Input(error) => Failure::Input(error),
        InvocationError::InvalidAcquisitionProfile => {
            Failure::V6Input(CodeNoesisErrorV6::invalid_acquisition_profile())
        }
        InvocationError::InvalidWorkspaceProfile => {
            r3_failure(CodeNoesisErrorV10::invalid_workspace_profile(), 2)
        }
        InvocationError::InvalidBoundaryProfile => {
            boundary_failure(CodeNoesisErrorV9::invalid_profile(), 2)
        }
        InvocationError::InvalidBoundaryManifest(reason) => {
            boundary_failure(CodeNoesisErrorV9::invalid_manifest(reason), 2)
        }
    }
}

fn s5_invocation_failure(error: InvocationError) -> Failure {
    match error {
        InvocationError::Input(error) => s5_input_failure(error),
        InvocationError::InvalidAcquisitionProfile => {
            Failure::V6Input(CodeNoesisErrorV6::invalid_acquisition_profile())
        }
        InvocationError::InvalidWorkspaceProfile => {
            r3_failure(CodeNoesisErrorV10::invalid_workspace_profile(), 2)
        }
        InvocationError::InvalidBoundaryProfile => {
            boundary_failure(CodeNoesisErrorV9::invalid_profile(), 2)
        }
        InvocationError::InvalidBoundaryManifest(reason) => {
            boundary_failure(CodeNoesisErrorV9::invalid_manifest(reason), 2)
        }
    }
}

fn repository_boundary_input_failure(
    failure: repository_boundaries::RepositoryBoundaryFailure,
) -> Failure {
    boundary_failure(failure.error, failure.exit_code)
}

fn boundary_scan_failure(error: BoundaryScanError) -> Failure {
    match error {
        BoundaryScanError::Scan(ScanError::Internal) => boundary_internal_failure(),
        BoundaryScanError::Scan(error) => Failure::Scan(error),
        BoundaryScanError::Boundary(error) => boundary_error_failure(&error),
        BoundaryScanError::InvalidManifest => boundary_failure(
            CodeNoesisErrorV9::invalid_manifest(BoundaryManifestReason::SchemaInvalid),
            2,
        ),
        BoundaryScanError::NestedMismatch {
            path,
            expected,
            observed,
        } => boundary_failure(
            CodeNoesisErrorV9::nested_mismatch(&path, &expected, &observed),
            10,
        ),
        BoundaryScanError::NestedUnavailable { path, error } => boundary_failure(
            CodeNoesisErrorV9::nested_unavailable(
                &path,
                repository_boundaries::nested_reason(&error),
            ),
            10,
        ),
        BoundaryScanError::NestedChanged { path } => {
            boundary_failure(CodeNoesisErrorV9::nested_changed(&path), 10)
        }
        BoundaryScanError::NestedPathUnavailable { path } => boundary_failure(
            CodeNoesisErrorV9::nested_unavailable(
                &path,
                NestedRepositoryUnavailableReason::PathInvalid,
            ),
            10,
        ),
    }
}

fn r3_scan_failure(error: RootPackageScanError) -> Failure {
    match error {
        RootPackageScanError::Scan(ScanError::Internal) => r3_internal_failure(),
        RootPackageScanError::Scan(error) => Failure::Scan(error),
        RootPackageScanError::Workspace(RootPackageWorkspaceError::Source(error)) => {
            Failure::Scan(ScanError::Workspace(error))
        }
        RootPackageScanError::Workspace(error) => CodeNoesisErrorV10::from_workspace(&error)
            .map_or_else(r3_internal_failure, |error| r3_failure(error, 11)),
        RootPackageScanError::Boundary(error) => boundary_scan_failure(error),
    }
}

fn r3_failure(error: CodeNoesisErrorV10, exit_code: u8) -> Failure {
    Failure::R3(R3Failure { error, exit_code })
}

fn r3_internal_failure() -> Failure {
    r3_failure(CodeNoesisErrorV10::internal(), 70)
}

fn boundary_error_failure(error: &RepositoryBoundaryError) -> Failure {
    let exit_code = if error == &RepositoryBoundaryError::InvalidReport {
        70
    } else {
        10
    };
    boundary_failure(CodeNoesisErrorV9::from_boundary(error), exit_code)
}

fn boundary_failure(error: CodeNoesisErrorV9, exit_code: u8) -> Failure {
    Failure::V9(V9Failure { error, exit_code })
}

fn boundary_internal_failure() -> Failure {
    boundary_failure(CodeNoesisErrorV9::internal(), 70)
}

fn repository_adapter(packed_sha1: bool) -> LocalGitRepository {
    if packed_sha1 {
        LocalGitRepository::new_packed_sha1()
    } else {
        LocalGitRepository::new()
    }
}

fn s5_storage_failure(error: &StorageError) -> Failure {
    s5_failure(CodeNoesisErrorV7::from_storage(error), 12)
}

fn s5_scan_failure(error: ScanError) -> Failure {
    match error {
        ScanError::Acquisition(error) => {
            s5_failure(CodeNoesisErrorV7::from_acquisition(&error), 10)
        }
        ScanError::Workspace(error) => s5_failure(CodeNoesisErrorV7::from_workspace(&error), 11),
        ScanError::Storage(error) => s5_storage_failure(&error),
        ScanError::Knowledge(_) | ScanError::Internal => s5_internal_failure(),
    }
}

fn s5_refresh_failure(
    error: RefreshError,
    baseline: &ValidatedS4Head,
    expected_repository_identity: &str,
) -> Failure {
    match error {
        RefreshError::Acquisition(error) => {
            s5_failure(CodeNoesisErrorV7::from_acquisition(&error), 10)
        }
        RefreshError::Workspace(error) => s5_failure(CodeNoesisErrorV7::from_workspace(&error), 11),
        RefreshError::BaselineRepositoryMismatch => s5_failure(
            CodeNoesisErrorV7::baseline_repository_mismatch(
                expected_repository_identity,
                baseline.head().repository_identity.as_str(),
            ),
            15,
        ),
        RefreshError::BaselineIncompatible => s5_failure(
            CodeNoesisErrorV7::baseline_incompatible(
                SNAPSHOT_SCHEMA_VERSION_V4,
                &baseline.head().snapshot_schema_version,
            ),
            15,
        ),
        RefreshError::ColdEquivalenceFailed {
            expected_hash,
            observed_hash,
        } => s5_failure(
            CodeNoesisErrorV7::cold_equivalence_failed("snapshot", &expected_hash, &observed_hash),
            15,
        ),
        RefreshError::LimitExceeded {
            limit,
            maximum,
            observed,
        } => s5_failure(
            CodeNoesisErrorV7::limit_exceeded(limit, maximum, observed),
            15,
        ),
        RefreshError::Internal => s5_internal_failure(),
    }
}

fn s5_report_failure(
    error: IncrementalRefreshReportError,
    baseline: &ValidatedS4Head,
    target_hash: &str,
) -> Failure {
    match error {
        IncrementalRefreshReportError::InvalidBaseline => s5_failure(
            CodeNoesisErrorV7::baseline_incompatible(
                SNAPSHOT_SCHEMA_VERSION_V4,
                &baseline.head().snapshot_schema_version,
            ),
            15,
        ),
        IncrementalRefreshReportError::InvalidTarget
        | IncrementalRefreshReportError::InvalidDocumentation
        | IncrementalRefreshReportError::InvalidAnalysis => s5_failure(
            CodeNoesisErrorV7::cold_equivalence_failed("snapshot", target_hash, "invalid"),
            15,
        ),
        IncrementalRefreshReportError::LimitExceeded {
            limit,
            maximum,
            observed,
        } => s5_failure(
            CodeNoesisErrorV7::limit_exceeded(
                limit,
                u64::try_from(maximum).unwrap_or(u64::MAX),
                u64::try_from(observed).unwrap_or(u64::MAX),
            ),
            15,
        ),
        IncrementalRefreshReportError::Serialization => s5_internal_failure(),
    }
}

fn s5_cache_failure(path: &str, expected_hash: &str, observed_hash: &str) -> Failure {
    s5_failure(
        CodeNoesisErrorV7::cache_corrupt(path, expected_hash, observed_hash),
        15,
    )
}

fn s5_cache_storage_failure(
    error: &StorageError,
    path: &str,
    expected_hash: &str,
    observed_hash: &str,
) -> Failure {
    if matches!(
        error,
        StorageError::CorruptMetadata {
            component: StorageComponent::Cas,
            ..
        }
    ) {
        s5_cache_failure(path, expected_hash, observed_hash)
    } else {
        s5_storage_failure(error)
    }
}

fn s5_internal_failure() -> Failure {
    s5_failure(CodeNoesisErrorV7::internal(), 70)
}

fn requested_profile(arguments: &[OsString], expected: &str) -> bool {
    arguments
        .windows(2)
        .any(|pair| pair[0] == OsStr::new("--profile") && pair[1] == OsStr::new(expected))
}

fn option_requested(arguments: &[OsString], expected: &str) -> bool {
    arguments
        .get(2..)
        .unwrap_or_default()
        .chunks(2)
        .any(|pair| pair.first().is_some_and(|flag| flag == expected))
}

fn s1_boundary_applies(repository: &OsStr) -> bool {
    fs::symlink_metadata(repository)
        .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
}

fn emit_internal_error_v1() -> ExitCode {
    emit_error_v1(&CodeNoesisErrorV1::internal(), 70)
}

fn emit_internal_error_v2() -> ExitCode {
    emit_error_v2(&CodeNoesisErrorV2::internal(), 70)
}

fn emit_internal_error_v3() -> ExitCode {
    emit_error_v3(&CodeNoesisErrorV3::internal(), 70)
}

fn emit_internal_error_v4() -> ExitCode {
    emit_error_v4(&CodeNoesisErrorV4::internal(), 70)
}

fn emit_internal_error_v6() -> ExitCode {
    emit_error_v6(&CodeNoesisErrorV6::internal(), 70)
}

fn emit_internal_error_v7() -> ExitCode {
    emit_error_v7(&CodeNoesisErrorV7::internal(), 70)
}

fn emit_internal_error_v8() -> ExitCode {
    emit_error_v8(&CodeNoesisErrorV8::internal(), 70)
}

fn emit_internal_error_v9() -> ExitCode {
    emit_error_v9(&CodeNoesisErrorV9::internal(), 70)
}

fn emit_error_v1(error: &CodeNoesisErrorV1, code: u8) -> ExitCode {
    if let Ok(bytes) = error.canonical_stderr() {
        let _ = io::stderr().lock().write_all(&bytes);
    }
    ExitCode::from(code)
}

fn emit_error_v2(error: &CodeNoesisErrorV2, code: u8) -> ExitCode {
    if let Ok(bytes) = error.canonical_stderr() {
        let _ = io::stderr().lock().write_all(&bytes);
    }
    ExitCode::from(code)
}

fn emit_error_v3(error: &CodeNoesisErrorV3, code: u8) -> ExitCode {
    if let Ok(bytes) = error.canonical_stderr() {
        let _ = io::stderr().lock().write_all(&bytes);
    }
    ExitCode::from(code)
}

fn emit_error_v4(error: &CodeNoesisErrorV4, code: u8) -> ExitCode {
    if let Ok(bytes) = error.canonical_stderr() {
        let _ = io::stderr().lock().write_all(&bytes);
    }
    ExitCode::from(code)
}

fn emit_error_v5(error: &CodeNoesisErrorV5, code: u8) -> ExitCode {
    if let Ok(bytes) = error.canonical_stderr() {
        let _ = io::stderr().lock().write_all(&bytes);
    }
    ExitCode::from(code)
}

fn emit_error_v6(error: &CodeNoesisErrorV6, code: u8) -> ExitCode {
    match error.canonical_stderr() {
        Ok(stderr) => {
            let _ = io::stderr().lock().write_all(&stderr);
            ExitCode::from(code)
        }
        Err(_) => ExitCode::from(70),
    }
}

fn emit_error_v7(error: &CodeNoesisErrorV7, code: u8) -> ExitCode {
    if let Ok(bytes) = error.canonical_stderr() {
        let _ = io::stderr().lock().write_all(&bytes);
    }
    ExitCode::from(code)
}

fn emit_error_v8(error: &CodeNoesisErrorV8, code: u8) -> ExitCode {
    let Ok(bytes) = error.canonical_stderr() else {
        return ExitCode::from(70);
    };
    if io::stderr().lock().write_all(&bytes).is_ok() {
        ExitCode::from(code)
    } else {
        ExitCode::from(70)
    }
}

fn emit_error_v9(error: &CodeNoesisErrorV9, code: u8) -> ExitCode {
    let Ok(bytes) = error.canonical_stderr() else {
        return ExitCode::from(70);
    };
    if io::stderr().lock().write_all(&bytes).is_ok() {
        ExitCode::from(code)
    } else {
        ExitCode::from(70)
    }
}

fn emit_error_v10(error: &CodeNoesisErrorV10, code: u8) -> ExitCode {
    let Ok(bytes) = error.canonical_stderr() else {
        return ExitCode::from(70);
    };
    if io::stderr().lock().write_all(&bytes).is_ok() {
        ExitCode::from(code)
    } else {
        ExitCode::from(70)
    }
}

fn emit_internal_error_v10() -> ExitCode {
    emit_error_v10(&CodeNoesisErrorV10::internal(), 70)
}

fn emit_docs_error(error: GeneratedDocsError) -> ExitCode {
    let error = match error {
        GeneratedDocsError::UnmarkedNonemptyRoot => {
            CodeNoesisErrorV5::docs_unmarked_nonempty_root()
        }
        GeneratedDocsError::InvalidRoot | GeneratedDocsError::UnsafePath => {
            CodeNoesisErrorV5::docs_unsafe_path()
        }
        GeneratedDocsError::SnapshotMismatch => CodeNoesisErrorV5::docs_snapshot_mismatch(),
        GeneratedDocsError::CorruptGeneration => CodeNoesisErrorV5::docs_corrupt_generation(),
        GeneratedDocsError::Failed => CodeNoesisErrorV5::docs_failed(),
    };
    emit_error_v5(&error, 13)
}

fn emit_query_error(error: QueryFailure) -> ExitCode {
    let error = match error {
        QueryFailure::NotFound => CodeNoesisErrorV5::query_not_found(),
        QueryFailure::SnapshotMismatch => CodeNoesisErrorV5::query_snapshot_mismatch(),
        QueryFailure::CorruptDocuments => CodeNoesisErrorV5::query_corrupt_documents(),
        QueryFailure::LimitExceeded => CodeNoesisErrorV5::query_result_limit_exceeded(),
    };
    emit_error_v5(&error, 14)
}

enum Failure {
    S6(federation::FederationFailure),
    R3(R3Failure),
    V9(V9Failure),
    Input(InputError),
    V6Input(CodeNoesisErrorV6),
    S4Input(CodeNoesisErrorV5),
    S5(S5Failure),
    Scan(ScanError),
    Docs(GeneratedDocsError),
    Query(QueryFailure),
    Internal,
}

struct R3Failure {
    error: CodeNoesisErrorV10,
    exit_code: u8,
}

struct V9Failure {
    error: CodeNoesisErrorV9,
    exit_code: u8,
}

struct S5Failure {
    error: CodeNoesisErrorV7,
    exit_code: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum QueryFailure {
    NotFound,
    SnapshotMismatch,
    CorruptDocuments,
    LimitExceeded,
}

enum LoadS4Error {
    Scan(ScanError),
    SnapshotMismatch,
}

struct LoadedS4Snapshot {
    head: LocalSnapshotHead,
    semantic: Value,
}

struct LoadedS5AnalysisCache {
    entries: Vec<AnalysisCacheEntry>,
    versions_compatible: bool,
}

struct DocsInvocation {
    store: OsString,
    identity: RepositoryIdentity,
    output: OsString,
}

impl DocsInvocation {
    fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Self, Failure> {
        let mut arguments = arguments.into_iter();
        let _program = arguments.next();
        if arguments.next().as_deref() != Some(OsStr::new("docs")) {
            return Err(Failure::S4Input(CodeNoesisErrorV5::invalid_output_root()));
        }
        let mut store = None;
        let mut identity = None;
        let mut output = None;
        let mut format = None;
        while let Some(flag) = arguments.next() {
            let value = arguments.next().ok_or_else(|| {
                if flag == OsStr::new("--output") {
                    Failure::S4Input(CodeNoesisErrorV5::invalid_output_root())
                } else {
                    Failure::Input(InputError::InvalidStoreRoot)
                }
            })?;
            match flag.to_str() {
                Some("--store") if store.is_none() => store = Some(value),
                Some("--repository-id") if identity.is_none() => {
                    identity = value.to_str().map(str::to_owned);
                }
                Some("--output") if output.is_none() => output = Some(value),
                Some("--format") if format.is_none() => format = value.to_str().map(str::to_owned),
                _ => {
                    return Err(Failure::S4Input(CodeNoesisErrorV5::invalid_output_root()));
                }
            }
        }
        let store = store
            .filter(|value: &OsString| !value.is_empty())
            .ok_or(Failure::Input(InputError::InvalidStoreRoot))?;
        let identity = identity
            .ok_or(Failure::Input(InputError::InvalidRepositoryIdentity))
            .and_then(|value| {
                RepositoryIdentity::parse(&value)
                    .map_err(|_| Failure::Input(InputError::InvalidRepositoryIdentity))
            })?;
        let output = output
            .filter(|value: &OsString| valid_s4_root_argument(value))
            .ok_or_else(|| Failure::S4Input(CodeNoesisErrorV5::invalid_output_root()))?;
        if format.as_deref() != Some("json") {
            return Err(Failure::S4Input(CodeNoesisErrorV5::invalid_output_root()));
        }
        Ok(Self {
            store,
            identity,
            output,
        })
    }
}

struct QueryInvocation {
    store: OsString,
    identity: RepositoryIdentity,
    documents: OsString,
    requested_id: String,
}

impl QueryInvocation {
    fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Self, Failure> {
        let mut arguments = arguments.into_iter();
        let _program = arguments.next();
        if arguments.next().as_deref() != Some(OsStr::new("query")) {
            return Err(Failure::S4Input(CodeNoesisErrorV5::invalid_query_id()));
        }
        let mut store = None;
        let mut identity = None;
        let mut documents = None;
        let mut requested_id = None;
        let mut format = None;
        while let Some(flag) = arguments.next() {
            let value = arguments.next().ok_or_else(|| {
                if flag == OsStr::new("--documents") {
                    Failure::S4Input(CodeNoesisErrorV5::invalid_documents_root())
                } else if flag == OsStr::new("--id") {
                    Failure::S4Input(CodeNoesisErrorV5::invalid_query_id())
                } else {
                    Failure::Input(InputError::InvalidStoreRoot)
                }
            })?;
            match flag.to_str() {
                Some("--store") if store.is_none() => store = Some(value),
                Some("--repository-id") if identity.is_none() => {
                    identity = value.to_str().map(str::to_owned);
                }
                Some("--documents") if documents.is_none() => documents = Some(value),
                Some("--id") if requested_id.is_none() => {
                    requested_id = value.to_str().map(str::to_owned);
                }
                Some("--format") if format.is_none() => format = value.to_str().map(str::to_owned),
                _ => {
                    return Err(Failure::S4Input(CodeNoesisErrorV5::invalid_query_id()));
                }
            }
        }
        let store = store
            .filter(|value: &OsString| !value.is_empty())
            .ok_or(Failure::Input(InputError::InvalidStoreRoot))?;
        let identity = identity
            .ok_or(Failure::Input(InputError::InvalidRepositoryIdentity))
            .and_then(|value| {
                RepositoryIdentity::parse(&value)
                    .map_err(|_| Failure::Input(InputError::InvalidRepositoryIdentity))
            })?;
        let documents = documents
            .filter(|value: &OsString| valid_s4_root_argument(value))
            .ok_or_else(|| Failure::S4Input(CodeNoesisErrorV5::invalid_documents_root()))?;
        let requested_id = requested_id
            .filter(|value| valid_query_id(value))
            .ok_or_else(|| Failure::S4Input(CodeNoesisErrorV5::invalid_query_id()))?;
        if format.as_deref() != Some("json") {
            return Err(Failure::S4Input(CodeNoesisErrorV5::invalid_query_id()));
        }
        Ok(Self {
            store,
            identity,
            documents,
            requested_id,
        })
    }
}

fn valid_query_id(value: &str) -> bool {
    [
        "urn:codenoesis:entity:blake3:",
        "urn:codenoesis:claim:blake3:",
        "urn:codenoesis:evidence:blake3:",
        "urn:codenoesis:document:blake3:",
    ]
    .into_iter()
    .find_map(|prefix| value.strip_prefix(prefix))
    .is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    })
}

fn valid_s4_root_argument(value: &OsStr) -> bool {
    !value.is_empty()
        && !std::path::Path::new(value)
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
}

struct Invocation {
    repository: OsString,
    identity: RepositoryIdentity,
    revision: Revision,
    store: Option<OsString>,
    packed_sha1: bool,
    workspace_profile: bool,
    boundary_profile: bool,
    boundary_manifest: Option<OsString>,
}

#[derive(Clone, Copy)]
enum InvocationError {
    Input(InputError),
    InvalidAcquisitionProfile,
    InvalidWorkspaceProfile,
    InvalidBoundaryProfile,
    InvalidBoundaryManifest(BoundaryManifestReason),
}

impl From<InputError> for InvocationError {
    fn from(error: InputError) -> Self {
        Self::Input(error)
    }
}

impl Invocation {
    fn parse(
        arguments: impl IntoIterator<Item = OsString>,
        required_profile: Option<&str>,
    ) -> Result<Self, InvocationError> {
        Self::parse_command(arguments, "scan", required_profile)
    }

    #[allow(clippy::too_many_lines)]
    fn parse_command(
        arguments: impl IntoIterator<Item = OsString>,
        command: &str,
        required_profile: Option<&str>,
    ) -> Result<Self, InvocationError> {
        let arguments = arguments.into_iter().collect::<Vec<_>>();
        let boundary_options_requested =
            option_requested(&arguments, "--repository-boundary-profile")
                || option_requested(&arguments, "--repository-boundary-manifest");
        let workspace_option_requested = option_requested(&arguments, "--workspace-profile");
        let mut arguments = arguments.into_iter();
        let _program = arguments.next();
        if arguments.next().as_deref() != Some(OsStr::new(command)) {
            return Err(if boundary_options_requested {
                InvocationError::InvalidBoundaryProfile
            } else if workspace_option_requested {
                InvocationError::InvalidWorkspaceProfile
            } else {
                InputError::InvalidRevision.into()
            });
        }
        let mut repository = None;
        let mut identity = None;
        let mut revision = None;
        let mut profile = None;
        let mut format = None;
        let mut store = None;
        let mut acquisition_profile = None;
        let mut workspace_profile = None;
        let mut boundary_profile = None;
        let mut boundary_manifest = None;
        while let Some(flag) = arguments.next() {
            let value = arguments.next().ok_or_else(|| {
                if flag == OsStr::new("--acquisition-profile") {
                    InvocationError::InvalidAcquisitionProfile
                } else if flag == OsStr::new("--workspace-profile") {
                    InvocationError::InvalidWorkspaceProfile
                } else if flag == OsStr::new("--repository-boundary-profile") {
                    InvocationError::InvalidBoundaryProfile
                } else if flag == OsStr::new("--repository-boundary-manifest") {
                    InvocationError::InvalidBoundaryManifest(
                        BoundaryManifestReason::ManifestUnavailable,
                    )
                } else if flag == OsStr::new("--profile") {
                    InvocationError::Input(InputError::InvalidProfile)
                } else if flag == OsStr::new("--store") {
                    InvocationError::Input(InputError::InvalidStoreRoot)
                } else {
                    InvocationError::Input(InputError::InvalidRevision)
                }
            })?;
            match flag.to_str() {
                Some("--repository") if repository.is_none() => repository = Some(value),
                Some("--repository-id") if identity.is_none() => {
                    identity = value.to_str().map(str::to_owned);
                }
                Some("--revision") if revision.is_none() => {
                    revision = value.to_str().map(str::to_owned);
                }
                Some("--profile") if profile.is_none() => {
                    profile = value.to_str().map(str::to_owned);
                }
                Some("--acquisition-profile") if acquisition_profile.is_none() => {
                    let Some(value) = value.to_str() else {
                        return Err(InvocationError::InvalidAcquisitionProfile);
                    };
                    acquisition_profile = Some(value.to_owned());
                }
                Some("--workspace-profile") if workspace_profile.is_none() => {
                    let Some(value) = value.to_str() else {
                        return Err(InvocationError::InvalidWorkspaceProfile);
                    };
                    workspace_profile = Some(value.to_owned());
                }
                Some("--repository-boundary-profile") if boundary_profile.is_none() => {
                    let Some(value) = value.to_str() else {
                        return Err(InvocationError::InvalidBoundaryProfile);
                    };
                    boundary_profile = Some(value.to_owned());
                }
                Some("--repository-boundary-manifest") if boundary_manifest.is_none() => {
                    boundary_manifest = Some(value);
                }
                Some("--store") if store.is_none() => store = Some(value),
                Some("--format") if format.is_none() => format = value.to_str().map(str::to_owned),
                Some("--acquisition-profile") => {
                    return Err(InvocationError::InvalidAcquisitionProfile);
                }
                Some("--workspace-profile") => {
                    return Err(InvocationError::InvalidWorkspaceProfile);
                }
                Some("--repository-boundary-profile") => {
                    return Err(InvocationError::InvalidBoundaryProfile);
                }
                Some("--repository-boundary-manifest") => {
                    return Err(InvocationError::InvalidBoundaryManifest(
                        BoundaryManifestReason::SchemaInvalid,
                    ));
                }
                _ => return Err(InputError::InvalidRevision.into()),
            }
        }
        let repository = repository.ok_or(InvocationError::Input(InputError::InvalidRevision))?;
        let identity = identity
            .ok_or(InputError::InvalidRepositoryIdentity)
            .and_then(|value| RepositoryIdentity::parse(&value))
            .map_err(InvocationError::Input)?;
        let revision = revision
            .ok_or(InputError::InvalidRevision)
            .and_then(|value| Revision::parse(&value))
            .map_err(InvocationError::Input)?;
        if let Some(required_profile) = required_profile {
            if profile.as_deref() != Some(required_profile) {
                return Err(if boundary_options_requested {
                    InvocationError::InvalidBoundaryProfile
                } else if workspace_option_requested {
                    InvocationError::InvalidWorkspaceProfile
                } else {
                    InputError::InvalidProfile.into()
                });
            }
        } else if profile.is_some() {
            return Err(if boundary_options_requested {
                InvocationError::InvalidBoundaryProfile
            } else if workspace_option_requested {
                InvocationError::InvalidWorkspaceProfile
            } else {
                InputError::InvalidRevision.into()
            });
        }
        let boundary_profile = match boundary_profile.as_deref() {
            None if boundary_manifest.is_none() => false,
            Some(LOCAL_GITLINKS_V1)
                if command == "scan" && required_profile == Some("standard-local-s4") =>
            {
                true
            }
            _ => return Err(InvocationError::InvalidBoundaryProfile),
        };
        let workspace_profile = match workspace_profile.as_deref() {
            None => false,
            Some(R3_WORKSPACE_PROFILE)
                if command == "scan" && required_profile == Some("standard-local-s4") =>
            {
                true
            }
            _ => return Err(InvocationError::InvalidWorkspaceProfile),
        };
        if boundary_manifest
            .as_ref()
            .is_some_and(|value| value.is_empty())
        {
            return Err(InvocationError::InvalidBoundaryManifest(
                BoundaryManifestReason::ManifestUnavailable,
            ));
        }
        let packed_sha1 = match acquisition_profile.as_deref() {
            None => false,
            Some(LOCAL_GIT_SHA1_PACKED_V1)
                if matches!(
                    required_profile,
                    Some(
                        "standard-local-s1"
                            | "standard-local-s2"
                            | "standard-local-s3"
                            | "standard-local-s4"
                    )
                ) =>
            {
                true
            }
            Some(_) => return Err(InvocationError::InvalidAcquisitionProfile),
        };
        if format.as_deref() != Some("json") {
            return Err(InputError::InvalidRevision.into());
        }
        if matches!(
            required_profile,
            Some("standard-local-s3" | "standard-local-s4" | "standard-local-s5")
        ) {
            if store
                .as_ref()
                .is_none_or(|value| value.as_os_str().is_empty())
            {
                return Err(InputError::InvalidStoreRoot.into());
            }
        } else if store.is_some() {
            return Err(InputError::InvalidRevision.into());
        }
        Ok(Self {
            repository,
            identity,
            revision,
            store,
            packed_sha1,
            workspace_profile,
            boundary_profile,
            boundary_manifest,
        })
    }
}

fn current_envelope() -> Option<SnapshotEnvelopeV1> {
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).ok()?;
    let created_at = rfc3339_utc(duration.as_secs())?;
    let sequence = CORRELATION_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let correlation_id = format!(
        "scan-{}-{:09}-{}-{sequence}",
        duration.as_secs(),
        duration.subsec_nanos(),
        std::process::id()
    );
    Some(SnapshotEnvelopeV1::new(created_at, None, correlation_id))
}

fn rfc3339_utc(timestamp: u64) -> Option<String> {
    let days = i64::try_from(timestamp / 86_400).ok()?;
    let seconds = timestamp % 86_400;
    let (year, month, day) = civil_date(days);
    if !(0..=9999).contains(&year) {
        return None;
    }
    let hour = seconds / 3_600;
    let minute = seconds % 3_600 / 60;
    let second = seconds % 60;
    Some(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"
    ))
}

fn civil_date(days_since_epoch: i64) -> (i64, i64, i64) {
    let shifted = days_since_epoch + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod s4_root_argument_tests {
    use super::*;

    #[test]
    fn sec_fr_cli_001_docs_traversal_is_rejected_during_parsing() {
        let arguments = [
            "noesis",
            "docs",
            "--store",
            "store",
            "--repository-id",
            "urn:codenoesis:repository:test",
            "--output",
            "../documents",
            "--format",
            "json",
        ]
        .map(OsString::from);

        assert!(matches!(
            DocsInvocation::parse(arguments),
            Err(Failure::S4Input(_))
        ));
    }

    #[test]
    fn sec_fr_cli_001_query_traversal_is_rejected_during_parsing() {
        let arguments = [
            "noesis",
            "query",
            "--store",
            "store",
            "--repository-id",
            "urn:codenoesis:repository:test",
            "--documents",
            "../documents",
            "--id",
            "urn:codenoesis:document:blake3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "--format",
            "json",
        ]
        .map(OsString::from);

        assert!(matches!(
            QueryInvocation::parse(arguments),
            Err(Failure::S4Input(_))
        ));
    }
}
