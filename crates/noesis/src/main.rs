//! `CodeNoesis` command-line entry point.

use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{self, Write};
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use codenoesis_application::{PublicationService, ScanError, ScanRequest, ScanService};
use codenoesis_contracts::{
    CodeNoesisErrorV1, CodeNoesisErrorV2, CodeNoesisErrorV3, CodeNoesisErrorV4, CodeNoesisErrorV5,
    DocumentationContractError, QueryContractError, RepositorySnapshotV2Error,
    RepositorySnapshotV3, RepositorySnapshotV3Error, RepositorySnapshotV4,
    RepositorySnapshotV4Error, SnapshotEnvelopeV1, generate_documentation_v1,
    local_query_result_v1, validate_stored_snapshot_semantic_v4,
};
use codenoesis_domain::AcquisitionError;
use codenoesis_domain::knowledge::KnowledgeError;
use codenoesis_domain::storage::{
    ArtifactRole, LocalSnapshotHead, SNAPSHOT_SCHEMA_VERSION_V4, StorageComponent, StorageError,
};
use codenoesis_domain::{
    InputError, LimitKind, RepositoryIdentity, Revision, STANDARD_LOCAL_S1_LIMITS, limit_exceeded,
};
use codenoesis_lang_rust::{TreeSitterRustExtractor, TreeSitterRustWorkspaceExtractor};
use codenoesis_ports::{ArtifactStore, NoopPublicationObserver};
use codenoesis_repository::LocalGitRepository;
use codenoesis_store_local::{LocalStore, ensure_store_root_for_boundary};
use noesis::generated_docs::{
    GeneratedDocsError, load_validated_manifest, validate_documents_root_for_boundary,
    validate_output_root_for_generation,
};
use serde_json::Value;

static CORRELATION_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn main() -> ExitCode {
    if noesis::install_s0_security_boundary().is_err() {
        return emit_internal_error_v1();
    }
    let arguments = env::args_os().collect::<Vec<_>>();
    let docs_requested = arguments.get(1).is_some_and(|value| value == "docs");
    let query_requested = arguments.get(1).is_some_and(|value| value == "query");
    let profiled = arguments
        .iter()
        .any(|argument| argument == OsStr::new("--profile"));
    let s4_requested = requested_profile(&arguments, "standard-local-s4");
    let s3_requested = requested_profile(&arguments, "standard-local-s3");
    let s4_error_lineage = s4_requested || docs_requested || query_requested;
    let s3_error_lineage = s3_requested || s4_error_lineage;
    let s2_requested = requested_profile(&arguments, "standard-local-s2");
    let result = if docs_requested {
        run_docs(arguments)
    } else if query_requested {
        run_query(arguments)
    } else if s4_requested {
        run_s4(arguments)
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
            Err(_) if s3_error_lineage => emit_internal_error_v4(),
            Err(_) if s2_requested => emit_internal_error_v3(),
            Err(_) if profiled => emit_internal_error_v2(),
            Err(_) => emit_internal_error_v1(),
        },
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
    let invocation = Invocation::parse(arguments, None).map_err(Failure::Input)?;
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
        Invocation::parse(arguments, Some("standard-local-s1")).map_err(Failure::Input)?;
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
    let stdout = ScanService::new(LocalGitRepository::new())
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
        Invocation::parse(arguments, Some("standard-local-s2")).map_err(Failure::Input)?;
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
    let stdout = ScanService::new(LocalGitRepository::new())
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
        Invocation::parse(arguments, Some("standard-local-s3")).map_err(Failure::Input)?;
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
    let snapshot = ScanService::new(LocalGitRepository::new())
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

fn run_s4(arguments: impl IntoIterator<Item = OsString>) -> Result<Vec<u8>, Failure> {
    let invocation =
        Invocation::parse(arguments, Some("standard-local-s4")).map_err(Failure::Input)?;
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
    let snapshot = ScanService::new(LocalGitRepository::new())
        .scan_s4(request, &TreeSitterRustWorkspaceExtractor::new())
        .map_err(Failure::Scan)?;
    enforce_scan_deadline(started_at)?;
    store_preparation.map_err(|error| Failure::Scan(ScanError::Storage(error)))?;
    let mut local_store = LocalStore::open(
        std::path::Path::new(&repository),
        std::path::Path::new(&store),
    )
    .map_err(|error| Failure::Scan(ScanError::Storage(error)))?;
    PublicationService::publish_v4(
        &snapshot,
        &mut local_store.artifacts,
        &mut local_store.metadata,
        &mut NoopPublicationObserver,
    )
    .map_err(Failure::Scan)?;
    serialize_v4(&snapshot)
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
    if head.snapshot_schema_version != SNAPSHOT_SCHEMA_VERSION_V4 {
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
    validate_stored_snapshot_semantic_v4(&semantic, &head)
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

fn requested_profile(arguments: &[OsString], expected: &str) -> bool {
    arguments
        .windows(2)
        .any(|pair| pair[0] == OsStr::new("--profile") && pair[1] == OsStr::new(expected))
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
    Input(InputError),
    S4Input(CodeNoesisErrorV5),
    Scan(ScanError),
    Docs(GeneratedDocsError),
    Query(QueryFailure),
    Internal,
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
}

impl Invocation {
    fn parse(
        arguments: impl IntoIterator<Item = OsString>,
        required_profile: Option<&str>,
    ) -> Result<Self, InputError> {
        let mut arguments = arguments.into_iter();
        let _program = arguments.next();
        if arguments.next().as_deref() != Some(OsStr::new("scan")) {
            return Err(InputError::InvalidRevision);
        }
        let mut repository = None;
        let mut identity = None;
        let mut revision = None;
        let mut profile = None;
        let mut format = None;
        let mut store = None;
        while let Some(flag) = arguments.next() {
            let value = arguments.next().ok_or_else(|| {
                if flag == OsStr::new("--profile") {
                    InputError::InvalidProfile
                } else if flag == OsStr::new("--store") {
                    InputError::InvalidStoreRoot
                } else {
                    InputError::InvalidRevision
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
                Some("--store") if store.is_none() => store = Some(value),
                Some("--format") if format.is_none() => format = value.to_str().map(str::to_owned),
                _ => return Err(InputError::InvalidRevision),
            }
        }
        let repository = repository.ok_or(InputError::InvalidRevision)?;
        let identity = identity
            .ok_or(InputError::InvalidRepositoryIdentity)
            .and_then(|value| RepositoryIdentity::parse(&value))?;
        let revision = revision
            .ok_or(InputError::InvalidRevision)
            .and_then(|value| Revision::parse(&value))?;
        if let Some(required_profile) = required_profile {
            if profile.as_deref() != Some(required_profile) {
                return Err(InputError::InvalidProfile);
            }
        } else if profile.is_some() {
            return Err(InputError::InvalidRevision);
        }
        if format.as_deref() != Some("json") {
            return Err(InputError::InvalidRevision);
        }
        if matches!(
            required_profile,
            Some("standard-local-s3" | "standard-local-s4")
        ) {
            if store
                .as_ref()
                .is_none_or(|value| value.as_os_str().is_empty())
            {
                return Err(InputError::InvalidStoreRoot);
            }
        } else if store.is_some() {
            return Err(InputError::InvalidRevision);
        }
        Ok(Self {
            repository,
            identity,
            revision,
            store,
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
