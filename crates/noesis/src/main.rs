//! `CodeNoesis` command-line entry point.

mod compiler_index;
mod federation;
mod impact;
mod impact_git;
mod impact_source;
mod repository_boundaries;

use std::collections::BTreeMap;
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File, Metadata};
use std::io::{self, Read, Seek, Write};
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use codenoesis_application::{
    BoundaryScanError, CallableBoundaryCompositionScanError, CallableCfgAlternativesScanError,
    CallableScipScanError, CallableSemanticsScanError, CargoManifestScanError,
    CompilerIndexScanError, ConstantEvaluationScanError, ExpressionBindingsScanError,
    FrameworkScanError, LocalFlowScanError, PublicationService, RefreshError, RefreshService,
    RootPackageScanError, RustCfgAlternativesScanError, RustSemanticScanError, ScanError,
    ScanRequest, ScanService, TrustedSourceRequest, TrustedSourceRetrievalError,
    TrustedSourceRetrievalService,
};
use codenoesis_contracts::{
    AnalysisCacheEntryV1, BoundaryManifestReason, CodeNoesisErrorV1, CodeNoesisErrorV2,
    CodeNoesisErrorV3, CodeNoesisErrorV4, CodeNoesisErrorV5, CodeNoesisErrorV6, CodeNoesisErrorV7,
    CodeNoesisErrorV8, CodeNoesisErrorV9, CodeNoesisErrorV10, CodeNoesisErrorV11,
    CodeNoesisErrorV12, CodeNoesisErrorV13, CodeNoesisErrorV14, CodeNoesisErrorV15,
    CodeNoesisErrorV16, CodeNoesisErrorV17, CodeNoesisErrorV18, CodeNoesisErrorV19,
    CodeNoesisErrorV20, CodeNoesisErrorV21, CodeNoesisErrorV22, CodeNoesisErrorV23,
    CodeNoesisErrorV24, CodeNoesisErrorV25, CodeNoesisErrorV26, CodeNoesisErrorV29,
    CodeNoesisErrorV30, DocumentationContractError, FunctionContextError, FunctionContextV1,
    IncrementalRefreshReportError, IncrementalRefreshReportInput, IncrementalRefreshReportV1,
    K1ContractError, LocalConfigurationError, LocalConfigurationSource,
    MAX_LOCAL_CONFIGURATION_BYTES, MAX_RELEASE_PROFILE_TEXT_BYTES,
    NestedRepositoryUnavailableReason, PortableGraphV1, PortableGraphV2, PortableGraphV3,
    PortableGraphV4, PortableGraphV5, PortableGraphV6, PortableGraphV7, PortableGraphV8,
    PortableGraphV9, QueryContractError, R8ContractError, R10ContractError, R11ContractError,
    R12_PORTABLE_GRAPH_VERSION, R12ContractError, R13_PORTABLE_GRAPH_VERSION, R13ContractError,
    R14ContractError, R15ContractError, R16ContractError, R17_CONTEXT_PROFILE, R18_SOURCE_PROFILE,
    ReleaseProfileError, RepositorySnapshotV2Error, RepositorySnapshotV3,
    RepositorySnapshotV3Error, RepositorySnapshotV4, RepositorySnapshotV4Error,
    RepositorySnapshotV5, RepositorySnapshotV5Error, RepositorySnapshotV6,
    RepositorySnapshotV6Error, RepositorySnapshotV7, RepositorySnapshotV7Error,
    RepositorySnapshotV8, RepositorySnapshotV8Error, RepositorySnapshotV9,
    RepositorySnapshotV9Error, RepositorySnapshotV10, RepositorySnapshotV10Error,
    RepositorySnapshotV11, RepositorySnapshotV11Error, RepositorySnapshotV12,
    RepositorySnapshotV12Error, RepositorySnapshotV13, RepositorySnapshotV13Error,
    RepositorySnapshotV14, RepositorySnapshotV14Error, RepositorySnapshotV15,
    RepositorySnapshotV15Error, RepositorySnapshotV16, RepositorySnapshotV16Error,
    RepositorySnapshotV17, RepositorySnapshotV17Error, RepositorySnapshotV18,
    RepositorySnapshotV18Error, SnapshotEnvelopeV1, TrustedSourceError, TrustedSourceSelectionV1,
    ValidatedS4Head, current_release_profile_v1, generate_documentation_v1,
    local_configuration_report_v1, local_query_result_v1, local_query_result_v2,
    local_query_result_v3, local_query_result_v4, local_query_result_v5, local_query_result_v6,
    local_query_result_v7, local_query_result_v8, local_query_result_v9, local_query_result_v10,
    local_query_result_v11, local_query_result_v12, local_query_result_v13,
    validate_stored_snapshot_semantic_v4, validate_stored_snapshot_semantic_v5,
    validate_stored_snapshot_semantic_v6, validate_stored_snapshot_semantic_v7,
    validate_stored_snapshot_semantic_v8, validate_stored_snapshot_semantic_v9,
    validate_stored_snapshot_semantic_v10, validate_stored_snapshot_semantic_v11,
    validate_stored_snapshot_semantic_v12, validate_stored_snapshot_semantic_v13,
    validate_stored_snapshot_semantic_v14, validate_stored_snapshot_semantic_v15,
    validate_stored_snapshot_semantic_v16, validate_stored_snapshot_semantic_v17,
    validate_stored_snapshot_semantic_v18,
};
use codenoesis_domain::knowledge::KnowledgeError;
use codenoesis_domain::s1_boundaries::LOCAL_GITLINKS_V1;
use codenoesis_domain::s1_boundaries::RepositoryBoundaryError;
use codenoesis_domain::s1_packed::LOCAL_GIT_SHA1_PACKED_V1;
use codenoesis_domain::s4::{
    S4_ONTOLOGY_VERSION, S4_TREE_SITTER_EXTRACTOR_VERSION, S4_WORKSPACE_EXTRACTOR_VERSION,
};
use codenoesis_domain::s4_k1::{CallableSemanticsError, K1_PROFILE};
use codenoesis_domain::s4_r3::{R3_WORKSPACE_PROFILE, RootPackageWorkspaceError};
use codenoesis_domain::s4_r4::{CargoManifestFactError, R4_MANIFEST_PROFILE};
use codenoesis_domain::s4_r5::{R5_RUST_SEMANTIC_PROFILE, RustSemanticError};
use codenoesis_domain::s4_r6::{FrameworkError, R6_FRAMEWORK_PROFILE};
use codenoesis_domain::s4_r7::{CompilerIndexError, R7_COMPILER_INDEX_PROFILE};
use codenoesis_domain::s4_r10::R10_PROFILE;
use codenoesis_domain::s4_r14::R14_PROFILE;
use codenoesis_domain::s4_r15::R15_PROFILE;
use codenoesis_domain::s4_r16::R16_PROFILE;
use codenoesis_domain::s5::{
    ANALYSIS_CACHE_SCHEMA_VERSION, AnalysisCacheEntry, DEPENDENCY_RULE_VERSION,
    EXTRACTION_CONTRACT_VERSION, IncrementalRuleOutcome, MAX_ANALYSIS_ENTRIES,
    MAX_REFRESH_WALL_MILLISECONDS, NORMALIZATION_VERSION, TARGET_SEMANTIC_PROFILE,
};
use codenoesis_domain::storage::{
    ArtifactRole, LocalSnapshotHead, SNAPSHOT_SCHEMA_VERSION_V4, SNAPSHOT_SCHEMA_VERSION_V5,
    SNAPSHOT_SCHEMA_VERSION_V6, SNAPSHOT_SCHEMA_VERSION_V7, SNAPSHOT_SCHEMA_VERSION_V8,
    SNAPSHOT_SCHEMA_VERSION_V9, SNAPSHOT_SCHEMA_VERSION_V10, SNAPSHOT_SCHEMA_VERSION_V11,
    SNAPSHOT_SCHEMA_VERSION_V12, SNAPSHOT_SCHEMA_VERSION_V13, SNAPSHOT_SCHEMA_VERSION_V14,
    SNAPSHOT_SCHEMA_VERSION_V15, SNAPSHOT_SCHEMA_VERSION_V16, SNAPSHOT_SCHEMA_VERSION_V17,
    SNAPSHOT_SCHEMA_VERSION_V18, StorageComponent, StorageError,
};
use codenoesis_domain::{AcquisitionError, EntryPolicy, RepositoryError, UnsupportedFeature};
use codenoesis_domain::{
    InputError, K1OutputCapacityProfile, LOCAL_SNAPSHOT_64M_V1, LOCAL_SNAPSHOT_256M_V1, LimitKind,
    RepositoryIdentity, Revision, STANDARD_LOCAL_S1_LIMITS, limit_exceeded,
};
use codenoesis_lang_rust::{TreeSitterRustExtractor, TreeSitterRustWorkspaceExtractor};
use codenoesis_ports::{AnalysisCacheStore, ArtifactStore, NoopPublicationObserver};
use codenoesis_repository::LocalGitRepository;
use codenoesis_scip::StaticScipImporter;
use codenoesis_store_local::{LocalStore, ensure_store_root_for_boundary};
use noesis::generated_docs::{
    GeneratedDocsError, load_validated_manifest, validate_documents_root_for_boundary,
    validate_output_root_for_generation,
};
use same_file::Handle as FileIdentity;
use serde_json::Value;
use sha2::{Digest as _, Sha256};

static CORRELATION_SEQUENCE: AtomicU64 = AtomicU64::new(0);
const SCAN_WORKER_STACK_BYTES: usize = 16 * 1024 * 1024;

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
            .stack_size(SCAN_WORKER_STACK_BYTES)
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
    let bootstrap = match ConfigurationBootstrap::parse(env::args_os().collect()) {
        Ok(bootstrap) => bootstrap,
        Err(error) => return emit_error_v26(&error, 2),
    };
    let configuration = match resolve_startup_configuration(bootstrap.configuration.as_deref()) {
        Ok(configuration) => configuration,
        Err(failure) => return emit_error_v26(&failure.error, failure.exit_code),
    };
    if bootstrap.validate_requested {
        if noesis::install_s0_security_boundary().is_err() {
            return emit_internal_error_v26();
        }
        let Ok(stdout) = configuration.canonical_stdout() else {
            return emit_internal_error_v26();
        };
        return if io::stdout().lock().write_all(&stdout).is_ok() {
            ExitCode::SUCCESS
        } else {
            emit_internal_error_v26()
        };
    }
    drop(configuration);
    let g1_requested = bootstrap.g1_requested;
    let arguments = bootstrap.arguments;
    let profile_requested = arguments.get(1).is_some_and(|value| value == "profile");
    let source_requested = arguments.get(1).is_some_and(|value| value == "source");
    let impact_source_requested = impact_source::requested(&arguments);
    let r10_scan_requested = rust_cfg_alternatives_requested(&arguments);
    let r12_scan_requested = rust_callable_cfg_alternatives_requested(&arguments);
    let r13_scan_requested = rust_callable_scip_requested(&arguments);
    let r16_requested = r16_profile_requested(&arguments);
    let r15_requested = r15_profile_requested(&arguments);
    let r14_requested = r14_profile_requested(&arguments);
    let r10_requested = r10_profile_requested(&arguments);
    let export_requested = arguments.get(1).is_some_and(|value| value == "export");
    let explore_requested = arguments.get(1).is_some_and(|value| value == "explore");
    let r8_requested = export_requested || explore_requested;
    let output_capacity_requested = option_requested(&arguments, "--output-capacity-profile");
    let k1_scan_requested = option_requested(&arguments, "--rust-callable-profile")
        || output_capacity_requested
        || r14_requested
        || r15_requested
        || r16_requested;
    let k1_requested = (k1_scan_requested
        || option_requested(&arguments, "--portable-profile")
        || option_requested(&arguments, "--explorer-profile"))
        && !r10_requested
        && !r12_scan_requested
        && !r13_scan_requested
        && !r14_requested
        && !r15_requested
        && !r16_requested;
    let r11_requested = k1_scan_requested
        && (option_requested(&arguments, "--repository-boundary-profile")
            || option_requested(&arguments, "--repository-boundary-manifest"))
        && !r12_scan_requested
        && !r13_scan_requested;
    let federation_requested = federation::requested(&arguments);
    let impact_git_requested = impact_git::requested(&arguments);
    let impact_requested = impact::requested(&arguments);
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
    let manifest_profile_scan_requested = option_requested(&arguments, "--manifest-profile");
    let rust_semantic_requested =
        option_requested(&arguments, "--rust-semantic-profile") && !r10_requested;
    let framework_requested = option_requested(&arguments, "--rust-framework-profile");
    let r7_requested = option_requested(&arguments, "--compiler-index-profile")
        || option_requested(&arguments, "--compiler-index-binding");
    let mut scan_worker = if k1_scan_requested
        || r7_requested
        || framework_requested
        || r10_scan_requested
        || rust_semantic_requested
        || manifest_profile_scan_requested
        || r3_requested
        || boundary_requested
    {
        ScanWorker::spawn().ok()
    } else {
        None
    };
    if noesis::install_s0_security_boundary().is_err() {
        return if impact_source_requested || impact_git_requested {
            emit_internal_error_v30()
        } else if source_requested {
            emit_internal_error_v29()
        } else if g1_requested {
            emit_internal_error_v26()
        } else {
            emit_internal_error_v1()
        };
    }
    let s4_requested = requested_profile(&arguments, "standard-local-s4");
    let s3_requested = requested_profile(&arguments, "standard-local-s3");
    let s4_error_lineage = s4_requested || docs_requested || query_requested || source_requested;
    let s3_error_lineage = s3_requested || s4_error_lineage;
    let s2_requested = requested_profile(&arguments, "standard-local-s2");
    let result = if profile_requested {
        run_profile(&arguments)
    } else if source_requested {
        run_source(arguments)
    } else if impact_source_requested {
        impact_source::run(arguments).map_err(Failure::R19Source)
    } else if impact_git_requested {
        impact_git::run(arguments).map_err(Failure::R19)
    } else if impact_requested {
        impact::run(arguments).map_err(Failure::S7)
    } else if output_capacity_requested {
        run_s4(arguments, scan_worker.as_mut())
    } else if export_requested {
        run_export(arguments)
    } else if explore_requested {
        run_explore(arguments)
    } else if k1_scan_requested
        || r7_requested
        || framework_requested
        || r10_scan_requested
        || rust_semantic_requested
        || manifest_profile_scan_requested
        || r3_requested
        || boundary_requested
    {
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
            Err(_) if impact_source_requested || impact_git_requested => emit_internal_error_v30(),
            Err(_) if source_requested => emit_internal_error_v29(),
            Err(_) if profile_requested => emit_internal_error_v25(),
            Err(_) if impact_requested => emit_internal_error_v23(),
            Err(_) if r16_requested => emit_internal_error_v24(),
            Err(_) if r15_requested => emit_internal_error_v22(),
            Err(_) if r14_requested => emit_internal_error_v21(),
            Err(_) if r13_scan_requested => emit_internal_error_v20(),
            Err(_) if r12_scan_requested => emit_internal_error_v19(),
            Err(_) if r11_requested => emit_internal_error_v18(),
            Err(_) if r10_requested => emit_internal_error_v17(),
            Err(_) if k1_requested => emit_internal_error_v16(),
            Err(_) if r8_requested => emit_internal_error_v15(),
            Err(_) if federation_requested => emit_internal_error_v8(),
            Err(_) if refresh_requested => emit_internal_error_v7(),
            Err(_) if r7_requested => emit_internal_error_v14(),
            Err(_) if framework_requested => emit_internal_error_v13(),
            Err(_) if rust_semantic_requested => emit_internal_error_v12(),
            Err(_) if manifest_profile_scan_requested => emit_internal_error_v11(),
            Err(_) if r3_requested => emit_internal_error_v10(),
            Err(_) if boundary_requested => emit_internal_error_v9(),
            Err(_) if packed_acquisition_requested => emit_internal_error_v6(),
            Err(_) if s3_error_lineage => emit_internal_error_v4(),
            Err(_) if s2_requested => emit_internal_error_v3(),
            Err(_) if profiled => emit_internal_error_v2(),
            Err(_) => emit_internal_error_v1(),
        },
        Err(Failure::R19Source(failure)) => emit_error_v30(&failure.error, failure.exit_code),
        Err(Failure::R19(failure)) => emit_error_v30(&failure.error, failure.exit_code),
        Err(Failure::R18(failure)) => emit_error_v29(&failure.error, failure.exit_code),
        Err(Failure::G0(failure)) => emit_error_v25(&failure.error, failure.exit_code),
        Err(Failure::S7(failure)) => emit_error_v23(&failure.error, failure.exit_code),
        Err(Failure::R16(failure)) => emit_error_v24(&failure.error, failure.exit_code),
        Err(Failure::R15(failure)) => emit_error_v22(&failure.error, failure.exit_code),
        Err(Failure::R14(failure)) => emit_error_v21(&failure.error, failure.exit_code),
        Err(Failure::R13(failure)) => emit_error_v20(&failure.error, failure.exit_code),
        Err(Failure::R12(failure)) => emit_error_v19(&failure.error, failure.exit_code),
        Err(Failure::R11(failure)) => emit_error_v18(&failure.error, failure.exit_code),
        Err(Failure::K1(failure)) => emit_error_v16(&failure.error, failure.exit_code),
        Err(Failure::R8(failure)) => emit_error_v15(&failure.error, failure.exit_code),
        Err(Failure::S6(failure)) => emit_error_v8(&failure.error, failure.exit_code),
        Err(Failure::R7(failure)) => emit_error_v14(&failure.error, failure.exit_code),
        Err(Failure::R6(failure)) => emit_error_v13(&failure.error, failure.exit_code),
        Err(Failure::R10(failure)) => emit_error_v17(&failure.error, failure.exit_code),
        Err(Failure::R5(failure)) => emit_error_v12(&failure.error, failure.exit_code),
        Err(Failure::R4(failure)) => emit_error_v11(&failure.error, failure.exit_code),
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
        Err(Failure::Scan(ScanError::Internal) | Failure::Internal) if r13_scan_requested => {
            emit_internal_error_v20()
        }
        Err(Failure::Scan(ScanError::Internal) | Failure::Internal) if r12_scan_requested => {
            emit_internal_error_v19()
        }
        Err(Failure::Scan(ScanError::Internal) | Failure::Internal) if r11_requested => {
            emit_internal_error_v18()
        }
        Err(Failure::Scan(ScanError::Internal) | Failure::Internal) if k1_requested => {
            emit_internal_error_v16()
        }
        Err(Failure::Scan(ScanError::Internal) | Failure::Internal) if r10_requested => {
            emit_internal_error_v17()
        }
        Err(Failure::Scan(ScanError::Internal) | Failure::Internal) if rust_semantic_requested => {
            emit_internal_error_v12()
        }
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

fn run_profile(arguments: &[OsString]) -> Result<Vec<u8>, Failure> {
    let invocation = ProfileInvocation::parse(arguments).map_err(|error| match error {
        ProfileInvocationError::InvalidCommand => {
            g0_failure(CodeNoesisErrorV25::invalid_command(), 2)
        }
        ProfileInvocationError::InvalidFormat => {
            g0_failure(CodeNoesisErrorV25::invalid_format(), 2)
        }
    })?;
    let profile = current_release_profile_v1(&invocation.profile_id).map_err(|error| {
        let exit_code = if error == ReleaseProfileError::ContractInvalid {
            1
        } else {
            2
        };
        g0_failure(CodeNoesisErrorV25::from_release_profile(error), exit_code)
    })?;
    profile
        .canonical_stdout()
        .map_err(|error| g0_failure(CodeNoesisErrorV25::from_release_profile(error), 1))
}

struct ProfileInvocation {
    profile_id: String,
}

impl ProfileInvocation {
    fn parse(arguments: &[OsString]) -> Result<Self, ProfileInvocationError> {
        if arguments.len() != 6 || arguments.get(1).is_none_or(|value| value != "profile") {
            return Err(ProfileInvocationError::InvalidCommand);
        }
        let mut profile_id = None;
        let mut format = None;
        for pair in arguments[2..].chunks_exact(2) {
            let flag = pair[0]
                .to_str()
                .ok_or(ProfileInvocationError::InvalidCommand)?;
            let value = pair[1]
                .to_str()
                .ok_or(ProfileInvocationError::InvalidCommand)?;
            match flag {
                "--id" if profile_id.is_none() => profile_id = Some(value.to_owned()),
                "--format" if format.is_none() => format = Some(value),
                _ => return Err(ProfileInvocationError::InvalidCommand),
            }
        }
        let profile_id = profile_id.ok_or(ProfileInvocationError::InvalidCommand)?;
        if profile_id.is_empty() || profile_id.len() > MAX_RELEASE_PROFILE_TEXT_BYTES {
            return Err(ProfileInvocationError::InvalidCommand);
        }
        let format = format.ok_or(ProfileInvocationError::InvalidCommand)?;
        if format != "json" {
            return Err(ProfileInvocationError::InvalidFormat);
        }
        Ok(Self { profile_id })
    }
}

#[derive(Debug)]
enum ProfileInvocationError {
    InvalidCommand,
    InvalidFormat,
}

struct ConfigurationBootstrap {
    arguments: Vec<OsString>,
    configuration: Option<OsString>,
    validate_requested: bool,
    g1_requested: bool,
}

impl ConfigurationBootstrap {
    fn parse(mut arguments: Vec<OsString>) -> Result<Self, CodeNoesisErrorV26> {
        let leading_configuration = arguments
            .get(1)
            .is_some_and(|argument| argument == "--config");
        let mut configuration = None;
        if leading_configuration {
            let value = arguments
                .get(2)
                .filter(|value| !value.is_empty())
                .cloned()
                .ok_or_else(CodeNoesisErrorV26::configuration_invalid_arguments)?;
            configuration = Some(value);
            arguments.drain(1..=2);
        }
        if arguments
            .iter()
            .skip(1)
            .any(|argument| argument == "--config")
        {
            return Err(CodeNoesisErrorV26::configuration_invalid_arguments());
        }

        let validate_requested = arguments.get(1).is_some_and(|value| value == "config");
        if validate_requested {
            if arguments.get(2).is_none_or(|value| value != "validate")
                || !matches!(arguments.len(), 5 | 7)
            {
                return Err(CodeNoesisErrorV26::configuration_invalid_arguments());
            }
            let mut file = None;
            let mut format = None;
            for pair in arguments[3..].chunks_exact(2) {
                let flag = pair[0]
                    .to_str()
                    .ok_or_else(CodeNoesisErrorV26::configuration_invalid_arguments)?;
                match flag {
                    "--file" if file.is_none() => file = Some(pair[1].clone()),
                    "--format" if format.is_none() => format = pair[1].to_str().map(str::to_owned),
                    _ => {
                        return Err(CodeNoesisErrorV26::configuration_invalid_arguments());
                    }
                }
            }
            if file.as_ref().is_some_and(|value| value.is_empty())
                || format.as_deref() != Some("json")
                || configuration.is_some() && file.is_some()
            {
                return Err(CodeNoesisErrorV26::configuration_invalid_arguments());
            }
            configuration = configuration.or(file);
        }
        if arguments.len() < 2 && leading_configuration {
            return Err(CodeNoesisErrorV26::configuration_invalid_arguments());
        }
        Ok(Self {
            arguments,
            configuration,
            validate_requested,
            g1_requested: leading_configuration || validate_requested,
        })
    }
}

#[derive(Debug)]
struct G1Failure {
    error: CodeNoesisErrorV26,
    exit_code: u8,
}

fn resolve_startup_configuration(
    path: Option<&OsStr>,
) -> Result<codenoesis_contracts::LocalConfigurationReportV1, G1Failure> {
    let (bytes, source) = if let Some(path) = path {
        (
            read_stable_configuration(std::path::Path::new(path))?,
            LocalConfigurationSource::ExplicitFile,
        )
    } else {
        (
            codenoesis_contracts::DEFAULT_LOCAL_CLI_CONFIGURATION_V1.to_vec(),
            LocalConfigurationSource::EmbeddedDefault,
        )
    };
    local_configuration_report_v1(&bytes, source).map_err(|error| G1Failure {
        exit_code: if error == LocalConfigurationError::ContractInvalid {
            1
        } else {
            2
        },
        error: CodeNoesisErrorV26::from_configuration(error),
    })
}

fn read_stable_configuration(path: &std::path::Path) -> Result<Vec<u8>, G1Failure> {
    let invalid = || G1Failure {
        error: CodeNoesisErrorV26::configuration_invalid_file(),
        exit_code: 2,
    };
    let unstable = || G1Failure {
        error: CodeNoesisErrorV26::configuration_unstable_input(),
        exit_code: 2,
    };
    let path_identity_before = FileIdentity::from_path(path).map_err(|_| invalid())?;
    let path_before = fs::symlink_metadata(path).map_err(|_| invalid())?;
    if path_before.file_type().is_symlink() || !path_before.is_file() {
        return Err(invalid());
    }
    let mut file = File::open(path).map_err(|_| invalid())?;
    let before = file.metadata().map_err(|_| invalid())?;
    let identity =
        FileIdentity::from_file(file.try_clone().map_err(|_| invalid())?).map_err(|_| invalid())?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(u64::try_from(MAX_LOCAL_CONFIGURATION_BYTES).unwrap_or(u64::MAX) + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| invalid())?;
    if bytes.len() > MAX_LOCAL_CONFIGURATION_BYTES {
        return Err(invalid());
    }
    file.rewind().map_err(|_| unstable())?;
    let mut verification = Vec::new();
    Read::by_ref(&mut file)
        .take(u64::try_from(MAX_LOCAL_CONFIGURATION_BYTES).unwrap_or(u64::MAX) + 1)
        .read_to_end(&mut verification)
        .map_err(|_| unstable())?;
    let after = file.metadata().map_err(|_| unstable())?;
    let path_after = fs::symlink_metadata(path).map_err(|_| unstable())?;
    let path_identity_matches =
        FileIdentity::from_path(path).is_ok_and(|path_identity| path_identity == identity);
    if bytes != verification
        || path_identity_before != identity
        || before.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
        || !g1_same_file_metadata(&path_before, &before)
        || !g1_same_file_metadata(&before, &after)
        || !g1_same_file_metadata(&after, &path_after)
        || !path_identity_matches
        || path_after.file_type().is_symlink()
    {
        return Err(unstable());
    }
    Ok(bytes)
}

#[cfg(unix)]
fn g1_same_file_metadata(left: &Metadata, right: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
}

#[cfg(windows)]
fn g1_same_file_metadata(left: &Metadata, right: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    left.file_size() == right.file_size()
        && left.last_write_time() == right.last_write_time()
        && left.creation_time() == right.creation_time()
        && left.file_attributes() == right.file_attributes()
}

#[cfg(not(any(unix, windows)))]
fn g1_same_file_metadata(left: &Metadata, right: &Metadata) -> bool {
    left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
        && left.file_type() == right.file_type()
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

#[allow(clippy::too_many_lines)]
fn run_s4(
    arguments: impl IntoIterator<Item = OsString>,
    scan_worker: Option<&mut ScanWorker>,
) -> Result<Vec<u8>, Failure> {
    let arguments = arguments.into_iter().collect::<Vec<_>>();
    let r16_requested = option_requested(&arguments, "--rust-constant-profile");
    let r15_requested = option_requested(&arguments, "--rust-flow-profile");
    let r14_requested = option_requested(&arguments, "--rust-expression-profile");
    let r13_requested = rust_callable_scip_requested(&arguments);
    let r12_requested = rust_callable_cfg_alternatives_requested(&arguments);
    let r11_requested = (option_requested(&arguments, "--rust-callable-profile")
        || option_requested(&arguments, "--output-capacity-profile"))
        && (option_requested(&arguments, "--repository-boundary-profile")
            || option_requested(&arguments, "--repository-boundary-manifest"))
        && !r12_requested;
    let invocation = parse_s4_invocation(arguments).map_err(|error| {
        if r16_requested {
            r16_invocation_failure(error)
        } else if r15_requested {
            r15_invocation_failure(error)
        } else if r14_requested {
            r14_invocation_failure(error)
        } else if r13_requested {
            r13_invocation_failure(error)
        } else if r12_requested {
            r12_invocation_failure(error)
        } else if r11_requested {
            r11_invocation_failure(error)
        } else {
            invocation_failure(error)
        }
    })?;
    if invocation.rust_constant_profile {
        let scan_worker = scan_worker.ok_or_else(r16_internal_failure)?;
        return run_s4_r16(invocation, scan_worker);
    }
    if invocation.rust_flow_profile {
        let scan_worker = scan_worker.ok_or_else(r15_internal_failure)?;
        return run_s4_r15(invocation, scan_worker);
    }
    if invocation.rust_expression_profile {
        let scan_worker = scan_worker.ok_or_else(r14_internal_failure)?;
        return run_s4_r14(invocation, scan_worker);
    }
    if invocation.rust_callable_profile && invocation.compiler_index_profile {
        let scan_worker = scan_worker.ok_or_else(r13_internal_failure)?;
        return run_s4_r13(invocation, scan_worker);
    }
    if invocation.rust_cfg_alternatives_profile && invocation.rust_callable_profile {
        let scan_worker = scan_worker.ok_or_else(r12_internal_failure)?;
        return if invocation.boundary_profile {
            run_s4_r12_boundaries(invocation, scan_worker)
        } else {
            run_s4_r12(invocation, scan_worker)
        };
    }
    if invocation.rust_callable_profile {
        let scan_worker = scan_worker.ok_or_else(|| {
            if invocation.boundary_profile {
                r11_internal_failure()
            } else {
                k1_internal_failure()
            }
        })?;
        return if invocation.boundary_profile {
            run_s4_r11_boundaries(invocation, scan_worker)
        } else {
            run_s4_k1(invocation, scan_worker)
        };
    }
    if invocation.compiler_index_profile {
        let scan_worker = scan_worker.ok_or_else(r7_internal_failure)?;
        return if invocation.boundary_profile {
            run_s4_r7_boundaries(invocation, scan_worker)
        } else {
            run_s4_r7(invocation, scan_worker)
        };
    }
    if invocation.rust_framework_profile {
        let scan_worker = scan_worker.ok_or_else(r6_internal_failure)?;
        return if invocation.boundary_profile {
            run_s4_r6_boundaries(invocation, scan_worker)
        } else {
            run_s4_r6(invocation, scan_worker)
        };
    }
    if invocation.rust_cfg_alternatives_profile {
        let scan_worker = scan_worker.ok_or_else(r10_internal_failure)?;
        return if invocation.boundary_profile {
            run_s4_r10_boundaries(invocation, scan_worker)
        } else {
            run_s4_r10(invocation, scan_worker)
        };
    }
    if invocation.rust_semantic_profile {
        let scan_worker = scan_worker.ok_or_else(r5_internal_failure)?;
        return if invocation.boundary_profile {
            run_s4_r5_boundaries(invocation, scan_worker)
        } else {
            run_s4_r5(invocation, scan_worker)
        };
    }
    if invocation.manifest_profile {
        let scan_worker = scan_worker.ok_or_else(r4_internal_failure)?;
        return if invocation.boundary_profile {
            run_s4_r4_boundaries(invocation, scan_worker)
        } else {
            run_s4_r4(invocation, scan_worker)
        };
    }
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

fn run_s4_k1(invocation: Invocation, scan_worker: &mut ScanWorker) -> Result<Vec<u8>, Failure> {
    let store = invocation
        .store
        .clone()
        .ok_or(Failure::Input(InputError::InvalidStoreRoot))?;
    let repository = invocation.repository.clone();
    let output_capacity_profile = invocation.output_capacity_profile;
    let started_at = Instant::now();
    let scan_repository = repository.clone();
    let scan = run_confined_scan(
        scan_worker,
        repository.clone(),
        None,
        Vec::new(),
        move || {
            let envelope = current_envelope().ok_or_else(k1_internal_failure)?;
            let request = ScanRequest::new(
                invocation.repository,
                invocation.identity,
                invocation.revision,
                envelope,
            );
            ScanService::new(repository_adapter(invocation.packed_sha1))
                .scan_s4_k1(request, |inventory| {
                    TreeSitterRustWorkspaceExtractor::new()
                        .extract_rust_callable_semantics(inventory)
                })
                .map_err(k1_scan_failure)
        },
    )
    .map_err(|()| k1_internal_failure())??;
    enforce_scan_deadline(started_at)?;
    let stdout = serialize_v11(&scan.snapshot, output_capacity_profile)?;
    let store_was_absent = fs::symlink_metadata(std::path::Path::new(&store))
        .is_err_and(|error| error.kind() == io::ErrorKind::NotFound);
    let mut rollback = EmptyStoreRollback::new(store.clone(), store_was_absent);
    ensure_store_root_for_boundary(
        std::path::Path::new(&scan_repository),
        std::path::Path::new(&store),
    )
    .map_err(|error| Failure::Scan(ScanError::Storage(error)))?;
    noesis::install_s3_filesystem_boundary(&scan_repository, &store)
        .map_err(|_| k1_internal_failure())?;
    let mut local_store = LocalStore::open(
        std::path::Path::new(&scan_repository),
        std::path::Path::new(&store),
    )
    .map_err(|error| Failure::Scan(ScanError::Storage(error)))?;
    PublicationService::publish_v11(
        &scan.snapshot,
        &mut local_store.artifacts,
        &mut local_store.metadata,
        &mut NoopPublicationObserver,
    )
    .map_err(|error| match error {
        ScanError::Internal => k1_internal_failure(),
        other => Failure::Scan(other),
    })?;
    rollback.disarm();
    stage_analysis_cache_best_effort(&mut local_store, &scan.analysis_cache_entries);
    Ok(stdout)
}

fn run_s4_r15(invocation: Invocation, scan_worker: &mut ScanWorker) -> Result<Vec<u8>, Failure> {
    let store = invocation
        .store
        .clone()
        .ok_or(Failure::Input(InputError::InvalidStoreRoot))?;
    let repository = invocation.repository.clone();
    let output_capacity_profile = invocation.output_capacity_profile;
    let started_at = Instant::now();
    let scan_repository = repository.clone();
    let scan = run_confined_scan(
        scan_worker,
        repository.clone(),
        None,
        Vec::new(),
        move || {
            let envelope = current_envelope().ok_or_else(r15_internal_failure)?;
            let request = ScanRequest::new(
                invocation.repository,
                invocation.identity,
                invocation.revision,
                envelope,
            );
            ScanService::new(repository_adapter(invocation.packed_sha1))
                .scan_s4_r15(request, output_capacity_profile, |inventory| {
                    TreeSitterRustWorkspaceExtractor::new().extract_rust_local_flow(inventory)
                })
                .map_err(r15_scan_failure)
        },
    )
    .map_err(|()| r15_internal_failure())??;
    enforce_scan_deadline(started_at).map_err(r15_upgrade_limit_failure)?;
    let stdout = serialize_v17(&scan.snapshot)?;
    let store_was_absent = fs::symlink_metadata(std::path::Path::new(&store))
        .is_err_and(|error| error.kind() == io::ErrorKind::NotFound);
    let mut rollback = EmptyStoreRollback::new(store.clone(), store_was_absent);
    ensure_store_root_for_boundary(
        std::path::Path::new(&scan_repository),
        std::path::Path::new(&store),
    )
    .map_err(|error| Failure::Scan(ScanError::Storage(error)))?;
    noesis::install_s3_filesystem_boundary(&scan_repository, &store)
        .map_err(|_| r15_internal_failure())?;
    let mut local_store = LocalStore::open(
        std::path::Path::new(&scan_repository),
        std::path::Path::new(&store),
    )
    .map_err(|error| Failure::Scan(ScanError::Storage(error)))?;
    PublicationService::publish_v17(
        &scan.snapshot,
        &mut local_store.artifacts,
        &mut local_store.metadata,
        &mut NoopPublicationObserver,
    )
    .map_err(|error| match error {
        ScanError::Internal => r15_internal_failure(),
        other => Failure::Scan(other),
    })?;
    rollback.disarm();
    stage_analysis_cache_best_effort(&mut local_store, &scan.analysis_cache_entries);
    Ok(stdout)
}

fn run_s4_r16(invocation: Invocation, scan_worker: &mut ScanWorker) -> Result<Vec<u8>, Failure> {
    let store = invocation
        .store
        .clone()
        .ok_or(Failure::Input(InputError::InvalidStoreRoot))?;
    let repository = invocation.repository.clone();
    let output_capacity_profile = invocation.output_capacity_profile;
    let started_at = Instant::now();
    let scan_repository = repository.clone();
    let scan = run_confined_scan(
        scan_worker,
        repository.clone(),
        None,
        Vec::new(),
        move || {
            let envelope = current_envelope().ok_or_else(r16_internal_failure)?;
            let request = ScanRequest::new(
                invocation.repository,
                invocation.identity,
                invocation.revision,
                envelope,
            );
            ScanService::new(repository_adapter(invocation.packed_sha1))
                .scan_s4_r16(request, output_capacity_profile, |inventory| {
                    TreeSitterRustWorkspaceExtractor::new()
                        .extract_rust_constant_evaluation(inventory)
                })
                .map_err(r16_scan_failure)
        },
    )
    .map_err(|()| r16_internal_failure())??;
    enforce_scan_deadline(started_at).map_err(r16_upgrade_limit_failure)?;
    let stdout = serialize_v18(&scan.snapshot)?;
    let store_was_absent = fs::symlink_metadata(std::path::Path::new(&store))
        .is_err_and(|error| error.kind() == io::ErrorKind::NotFound);
    let mut rollback = EmptyStoreRollback::new(store.clone(), store_was_absent);
    ensure_store_root_for_boundary(
        std::path::Path::new(&scan_repository),
        std::path::Path::new(&store),
    )
    .map_err(|error| Failure::Scan(ScanError::Storage(error)))?;
    noesis::install_s3_filesystem_boundary(&scan_repository, &store)
        .map_err(|_| r16_internal_failure())?;
    let mut local_store = LocalStore::open(
        std::path::Path::new(&scan_repository),
        std::path::Path::new(&store),
    )
    .map_err(|error| Failure::Scan(ScanError::Storage(error)))?;
    PublicationService::publish_v18(
        &scan.snapshot,
        &mut local_store.artifacts,
        &mut local_store.metadata,
        &mut NoopPublicationObserver,
    )
    .map_err(|error| match error {
        ScanError::Internal => r16_internal_failure(),
        other => Failure::Scan(other),
    })?;
    rollback.disarm();
    stage_analysis_cache_best_effort(&mut local_store, &scan.analysis_cache_entries);
    Ok(stdout)
}

fn run_s4_r14(invocation: Invocation, scan_worker: &mut ScanWorker) -> Result<Vec<u8>, Failure> {
    let store = invocation
        .store
        .clone()
        .ok_or(Failure::Input(InputError::InvalidStoreRoot))?;
    let repository = invocation.repository.clone();
    let output_capacity_profile = invocation.output_capacity_profile;
    let started_at = Instant::now();
    let scan_repository = repository.clone();
    let scan = run_confined_scan(
        scan_worker,
        repository.clone(),
        None,
        Vec::new(),
        move || {
            let envelope = current_envelope().ok_or_else(r14_internal_failure)?;
            let request = ScanRequest::new(
                invocation.repository,
                invocation.identity,
                invocation.revision,
                envelope,
            );
            ScanService::new(repository_adapter(invocation.packed_sha1))
                .scan_s4_r14(request, output_capacity_profile, |inventory| {
                    TreeSitterRustWorkspaceExtractor::new()
                        .extract_rust_expression_bindings(inventory)
                })
                .map_err(r14_scan_failure)
        },
    )
    .map_err(|()| r14_internal_failure())??;
    enforce_scan_deadline(started_at).map_err(r14_upgrade_limit_failure)?;
    let stdout = serialize_v16(&scan.snapshot)?;
    let store_was_absent = fs::symlink_metadata(std::path::Path::new(&store))
        .is_err_and(|error| error.kind() == io::ErrorKind::NotFound);
    let mut rollback = EmptyStoreRollback::new(store.clone(), store_was_absent);
    ensure_store_root_for_boundary(
        std::path::Path::new(&scan_repository),
        std::path::Path::new(&store),
    )
    .map_err(|error| Failure::Scan(ScanError::Storage(error)))?;
    noesis::install_s3_filesystem_boundary(&scan_repository, &store)
        .map_err(|_| r14_internal_failure())?;
    let mut local_store = LocalStore::open(
        std::path::Path::new(&scan_repository),
        std::path::Path::new(&store),
    )
    .map_err(|error| Failure::Scan(ScanError::Storage(error)))?;
    PublicationService::publish_v16(
        &scan.snapshot,
        &mut local_store.artifacts,
        &mut local_store.metadata,
        &mut NoopPublicationObserver,
    )
    .map_err(|error| match error {
        ScanError::Internal => r14_internal_failure(),
        other => Failure::Scan(other),
    })?;
    rollback.disarm();
    stage_analysis_cache_best_effort(&mut local_store, &scan.analysis_cache_entries);
    Ok(stdout)
}

fn run_s4_r13(invocation: Invocation, scan_worker: &mut ScanWorker) -> Result<Vec<u8>, Failure> {
    let store = invocation
        .store
        .clone()
        .ok_or(Failure::Input(InputError::InvalidStoreRoot))?;
    let binding = invocation
        .compiler_index_binding
        .as_deref()
        .ok_or_else(r13_internal_failure)?;
    let prepared =
        compiler_index::prepare(binding).map_err(|error| r13_compiler_index_failure(&error))?;
    let importer = StaticScipImporter::new(&prepared.binding_path, &prepared.artifact_path);
    let repository = invocation.repository.clone();
    let scan_repository = repository.clone();
    let started_at = Instant::now();
    let scan = run_confined_r7_scan(scan_worker, repository, prepared, move || {
        let envelope = current_envelope().ok_or_else(r13_internal_failure)?;
        let request = ScanRequest::new(
            invocation.repository,
            invocation.identity,
            invocation.revision,
            envelope,
        );
        ScanService::new(repository_adapter(invocation.packed_sha1))
            .scan_s4_r13(
                request,
                |inventory| {
                    TreeSitterRustWorkspaceExtractor::new()
                        .extract_rust_callable_semantics(inventory)
                },
                &importer,
            )
            .map_err(r13_scan_failure)
    })
    .map_err(|()| r13_internal_failure())??;
    enforce_scan_deadline(started_at).map_err(r13_upgrade_limit_failure)?;
    let stdout = serialize_v15(&scan.snapshot)?;
    let store_was_absent = fs::symlink_metadata(std::path::Path::new(&store))
        .is_err_and(|error| error.kind() == io::ErrorKind::NotFound);
    let mut rollback = EmptyStoreRollback::new(store.clone(), store_was_absent);
    ensure_store_root_for_boundary(
        std::path::Path::new(&scan_repository),
        std::path::Path::new(&store),
    )
    .map_err(|error| Failure::Scan(ScanError::Storage(error)))?;
    noesis::install_s3_filesystem_boundary(&scan_repository, &store)
        .map_err(|_| r13_internal_failure())?;
    let mut local_store = LocalStore::open(
        std::path::Path::new(&scan_repository),
        std::path::Path::new(&store),
    )
    .map_err(|error| Failure::Scan(ScanError::Storage(error)))?;
    PublicationService::publish_v15(
        &scan.snapshot,
        &mut local_store.artifacts,
        &mut local_store.metadata,
        &mut NoopPublicationObserver,
    )
    .map_err(|error| match error {
        ScanError::Internal => r13_internal_failure(),
        other => Failure::Scan(other),
    })?;
    rollback.disarm();
    stage_analysis_cache_best_effort(&mut local_store, &scan.analysis_cache_entries);
    Ok(stdout)
}

fn run_s4_r11_boundaries(
    invocation: Invocation,
    scan_worker: &mut ScanWorker,
) -> Result<Vec<u8>, Failure> {
    let mut prepared = repository_boundaries::prepare(
        invocation.boundary_manifest.as_deref(),
        &invocation.identity,
        &invocation.revision,
    )
    .map_err(|failure| repository_boundary_input_failure_r11(&failure))?;
    let store = invocation
        .store
        .clone()
        .ok_or(Failure::Input(InputError::InvalidStoreRoot))?;
    let repository = invocation.repository.clone();
    let output_capacity_profile = invocation.output_capacity_profile;
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
            let envelope = current_envelope().ok_or_else(r11_internal_failure)?;
            let request = ScanRequest::new(
                invocation.repository,
                invocation.identity,
                invocation.revision,
                envelope,
            );
            ScanService::new(repository_adapter(invocation.packed_sha1))
                .scan_s4_r11_boundaries(
                    request,
                    prepared.scan_input,
                    &TreeSitterRustWorkspaceExtractor::new(),
                    &repository_boundaries::Sha256BoundaryHasher,
                )
                .map_err(r11_scan_failure)
        },
    )
    .map_err(|()| r11_internal_failure())??;
    enforce_scan_deadline(started_at).map_err(r11_upgrade_limit_failure)?;
    let stdout = serialize_v13(&scan.snapshot, output_capacity_profile)?;
    let store_was_absent = fs::symlink_metadata(std::path::Path::new(&store))
        .is_err_and(|error| error.kind() == io::ErrorKind::NotFound);
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
    .map_err(|_| r11_internal_failure())?;
    let mut local_store = LocalStore::open(
        std::path::Path::new(&scan_repository),
        std::path::Path::new(&store),
    )
    .map_err(|error| Failure::Scan(ScanError::Storage(error)))?;
    PublicationService::publish_v13(
        &scan.snapshot,
        &mut local_store.artifacts,
        &mut local_store.metadata,
        &mut NoopPublicationObserver,
    )
    .map_err(|error| match error {
        ScanError::Internal => r11_internal_failure(),
        other => Failure::Scan(other),
    })?;
    rollback.disarm();
    stage_analysis_cache_best_effort(&mut local_store, &scan.analysis_cache_entries);
    Ok(stdout)
}

fn run_s4_r12(invocation: Invocation, scan_worker: &mut ScanWorker) -> Result<Vec<u8>, Failure> {
    let store = invocation
        .store
        .clone()
        .ok_or(Failure::Input(InputError::InvalidStoreRoot))?;
    let repository = invocation.repository.clone();
    let output_capacity_profile = invocation.output_capacity_profile;
    let started_at = Instant::now();
    let scan_repository = repository.clone();
    let scan = run_confined_scan(
        scan_worker,
        repository.clone(),
        None,
        Vec::new(),
        move || {
            let envelope = current_envelope().ok_or_else(r12_internal_failure)?;
            let request = ScanRequest::new(
                invocation.repository,
                invocation.identity,
                invocation.revision,
                envelope,
            );
            ScanService::new(repository_adapter(invocation.packed_sha1))
                .scan_s4_r12(request, &TreeSitterRustWorkspaceExtractor::new())
                .map_err(r12_scan_failure)
        },
    )
    .map_err(|()| r12_internal_failure())??;
    enforce_scan_deadline(started_at).map_err(r12_upgrade_limit_failure)?;
    let stdout = serialize_v14(&scan.snapshot, output_capacity_profile)?;
    let store_was_absent = fs::symlink_metadata(std::path::Path::new(&store))
        .is_err_and(|error| error.kind() == io::ErrorKind::NotFound);
    let mut rollback = EmptyStoreRollback::new(store.clone(), store_was_absent);
    ensure_store_root_for_boundary(
        std::path::Path::new(&scan_repository),
        std::path::Path::new(&store),
    )
    .map_err(|error| Failure::Scan(ScanError::Storage(error)))?;
    noesis::install_s3_filesystem_boundary(&scan_repository, &store)
        .map_err(|_| r12_internal_failure())?;
    let mut local_store = LocalStore::open(
        std::path::Path::new(&scan_repository),
        std::path::Path::new(&store),
    )
    .map_err(|error| Failure::Scan(ScanError::Storage(error)))?;
    PublicationService::publish_v14(
        &scan.snapshot,
        &mut local_store.artifacts,
        &mut local_store.metadata,
        &mut NoopPublicationObserver,
    )
    .map_err(|error| match error {
        ScanError::Internal => r12_internal_failure(),
        other => Failure::Scan(other),
    })?;
    rollback.disarm();
    stage_analysis_cache_best_effort(&mut local_store, &scan.analysis_cache_entries);
    Ok(stdout)
}

fn run_s4_r12_boundaries(
    invocation: Invocation,
    scan_worker: &mut ScanWorker,
) -> Result<Vec<u8>, Failure> {
    let mut prepared = repository_boundaries::prepare(
        invocation.boundary_manifest.as_deref(),
        &invocation.identity,
        &invocation.revision,
    )
    .map_err(|failure| repository_boundary_input_failure_r12(&failure))?;
    let store = invocation
        .store
        .clone()
        .ok_or(Failure::Input(InputError::InvalidStoreRoot))?;
    let repository = invocation.repository.clone();
    let output_capacity_profile = invocation.output_capacity_profile;
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
            let envelope = current_envelope().ok_or_else(r12_internal_failure)?;
            let request = ScanRequest::new(
                invocation.repository,
                invocation.identity,
                invocation.revision,
                envelope,
            );
            ScanService::new(repository_adapter(invocation.packed_sha1))
                .scan_s4_r12_boundaries(
                    request,
                    prepared.scan_input,
                    &TreeSitterRustWorkspaceExtractor::new(),
                    &repository_boundaries::Sha256BoundaryHasher,
                )
                .map_err(r12_scan_failure)
        },
    )
    .map_err(|()| r12_internal_failure())??;
    enforce_scan_deadline(started_at).map_err(r12_upgrade_limit_failure)?;
    let stdout = serialize_v14(&scan.snapshot, output_capacity_profile)?;
    let store_was_absent = fs::symlink_metadata(std::path::Path::new(&store))
        .is_err_and(|error| error.kind() == io::ErrorKind::NotFound);
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
    .map_err(|_| r12_internal_failure())?;
    let mut local_store = LocalStore::open(
        std::path::Path::new(&scan_repository),
        std::path::Path::new(&store),
    )
    .map_err(|error| Failure::Scan(ScanError::Storage(error)))?;
    PublicationService::publish_v14(
        &scan.snapshot,
        &mut local_store.artifacts,
        &mut local_store.metadata,
        &mut NoopPublicationObserver,
    )
    .map_err(|error| match error {
        ScanError::Internal => r12_internal_failure(),
        other => Failure::Scan(other),
    })?;
    rollback.disarm();
    stage_analysis_cache_best_effort(&mut local_store, &scan.analysis_cache_entries);
    Ok(stdout)
}

fn run_s4_r7(invocation: Invocation, scan_worker: &mut ScanWorker) -> Result<Vec<u8>, Failure> {
    let store = invocation
        .store
        .clone()
        .ok_or(Failure::Input(InputError::InvalidStoreRoot))?;
    let binding = invocation
        .compiler_index_binding
        .as_deref()
        .ok_or_else(r7_internal_failure)?;
    let prepared =
        compiler_index::prepare(binding).map_err(|error| r7_compiler_index_failure(&error))?;
    let importer = StaticScipImporter::new(&prepared.binding_path, &prepared.artifact_path);
    let repository = invocation.repository.clone();
    let started_at = Instant::now();
    let scan_repository = repository.clone();
    let scan = run_confined_r7_scan(scan_worker, repository.clone(), prepared, move || {
        let envelope = current_envelope().ok_or_else(r7_internal_failure)?;
        let request = ScanRequest::new(
            invocation.repository,
            invocation.identity,
            invocation.revision,
            envelope,
        );
        ScanService::new(repository_adapter(invocation.packed_sha1))
            .scan_s4_r7(request, &TreeSitterRustWorkspaceExtractor::new(), &importer)
            .map_err(r7_scan_failure)
    })
    .map_err(|()| r7_internal_failure())??;
    enforce_scan_deadline(started_at)?;
    let stdout = serialize_v10(&scan.snapshot)?;
    let store_was_absent = fs::symlink_metadata(std::path::Path::new(&store))
        .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound);
    let mut rollback = EmptyStoreRollback::new(store.clone(), store_was_absent);
    ensure_store_root_for_boundary(
        std::path::Path::new(&scan_repository),
        std::path::Path::new(&store),
    )
    .map_err(|error| Failure::Scan(ScanError::Storage(error)))?;
    noesis::install_s3_filesystem_boundary(&scan_repository, &store)
        .map_err(|_| r7_internal_failure())?;
    let mut local_store = LocalStore::open(
        std::path::Path::new(&scan_repository),
        std::path::Path::new(&store),
    )
    .map_err(|error| Failure::Scan(ScanError::Storage(error)))?;
    PublicationService::publish_v10(
        &scan.snapshot,
        &mut local_store.artifacts,
        &mut local_store.metadata,
        &mut NoopPublicationObserver,
    )
    .map_err(|error| match error {
        ScanError::Internal => r7_internal_failure(),
        other => Failure::Scan(other),
    })?;
    rollback.disarm();
    stage_analysis_cache_best_effort(&mut local_store, &scan.analysis_cache_entries);
    Ok(stdout)
}

fn run_s4_r7_boundaries(
    invocation: Invocation,
    scan_worker: &mut ScanWorker,
) -> Result<Vec<u8>, Failure> {
    let mut prepared_boundaries = repository_boundaries::prepare(
        invocation.boundary_manifest.as_deref(),
        &invocation.identity,
        &invocation.revision,
    )
    .map_err(repository_boundary_input_failure)?;
    let store = invocation
        .store
        .clone()
        .ok_or(Failure::Input(InputError::InvalidStoreRoot))?;
    let binding = invocation
        .compiler_index_binding
        .as_deref()
        .ok_or_else(r7_internal_failure)?;
    let prepared_compiler =
        compiler_index::prepare(binding).map_err(|error| r7_compiler_index_failure(&error))?;
    let importer = StaticScipImporter::new(
        &prepared_compiler.binding_path,
        &prepared_compiler.artifact_path,
    );
    let repository = invocation.repository.clone();
    if let Some(canonical_store) = canonical_existing_or_absent_leaf(&store) {
        prepared_boundaries.reject_overlaps(&canonical_store);
    }
    if let Ok(canonical_repository) = fs::canonicalize(std::path::Path::new(&repository)) {
        prepared_boundaries.reject_overlaps(&canonical_repository);
    }
    let manifest_path = prepared_boundaries.manifest_path;
    let nested_roots = prepared_boundaries.nested_roots;
    let started_at = Instant::now();
    let scan_repository = repository.clone();
    let scan = run_confined_r7_boundary_scan(
        scan_worker,
        repository.clone(),
        manifest_path.clone(),
        nested_roots.clone(),
        prepared_compiler,
        move || {
            let envelope = current_envelope().ok_or_else(r7_internal_failure)?;
            let request = ScanRequest::new(
                invocation.repository,
                invocation.identity,
                invocation.revision,
                envelope,
            );
            ScanService::new(repository_adapter(invocation.packed_sha1))
                .scan_s4_r7_boundaries(
                    request,
                    prepared_boundaries.scan_input,
                    &TreeSitterRustWorkspaceExtractor::new(),
                    &importer,
                    &repository_boundaries::Sha256BoundaryHasher,
                )
                .map_err(r7_scan_failure)
        },
    )
    .map_err(|()| r7_internal_failure())??;
    enforce_scan_deadline(started_at)?;
    let stdout = serialize_v10(&scan.snapshot)?;
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
    PublicationService::publish_v10(
        &scan.snapshot,
        &mut local_store.artifacts,
        &mut local_store.metadata,
        &mut NoopPublicationObserver,
    )
    .map_err(|error| match error {
        ScanError::Internal => r7_internal_failure(),
        other => Failure::Scan(other),
    })?;
    rollback.disarm();
    stage_analysis_cache_best_effort(&mut local_store, &scan.analysis_cache_entries);
    Ok(stdout)
}

fn run_s4_r6(invocation: Invocation, scan_worker: &mut ScanWorker) -> Result<Vec<u8>, Failure> {
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
            let envelope = current_envelope().ok_or_else(r6_internal_failure)?;
            let request = ScanRequest::new(
                invocation.repository,
                invocation.identity,
                invocation.revision,
                envelope,
            );
            ScanService::new(repository_adapter(invocation.packed_sha1))
                .scan_s4_r6(request, &TreeSitterRustWorkspaceExtractor::new())
                .map_err(r6_scan_failure)
        },
    )
    .map_err(|()| r6_internal_failure())??;
    enforce_scan_deadline(started_at)?;
    let stdout = serialize_v9(&scan.snapshot)?;
    let store_was_absent = fs::symlink_metadata(std::path::Path::new(&store))
        .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound);
    let mut rollback = EmptyStoreRollback::new(store.clone(), store_was_absent);
    ensure_store_root_for_boundary(
        std::path::Path::new(&scan_repository),
        std::path::Path::new(&store),
    )
    .map_err(|error| Failure::Scan(ScanError::Storage(error)))?;
    noesis::install_s3_filesystem_boundary(&scan_repository, &store)
        .map_err(|_| r6_internal_failure())?;
    let mut local_store = LocalStore::open(
        std::path::Path::new(&scan_repository),
        std::path::Path::new(&store),
    )
    .map_err(|error| Failure::Scan(ScanError::Storage(error)))?;
    PublicationService::publish_v9(
        &scan.snapshot,
        &mut local_store.artifacts,
        &mut local_store.metadata,
        &mut NoopPublicationObserver,
    )
    .map_err(|error| match error {
        ScanError::Internal => r6_internal_failure(),
        other => Failure::Scan(other),
    })?;
    rollback.disarm();
    stage_analysis_cache_best_effort(&mut local_store, &scan.analysis_cache_entries);
    Ok(stdout)
}

fn run_s4_r6_boundaries(
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
            let envelope = current_envelope().ok_or_else(r6_internal_failure)?;
            let request = ScanRequest::new(
                invocation.repository,
                invocation.identity,
                invocation.revision,
                envelope,
            );
            ScanService::new(repository_adapter(invocation.packed_sha1))
                .scan_s4_r6_boundaries(
                    request,
                    prepared.scan_input,
                    &TreeSitterRustWorkspaceExtractor::new(),
                    &repository_boundaries::Sha256BoundaryHasher,
                )
                .map_err(r6_scan_failure)
        },
    )
    .map_err(|()| r6_internal_failure())??;
    enforce_scan_deadline(started_at)?;
    let stdout = serialize_v9(&scan.snapshot)?;
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
    PublicationService::publish_v9(
        &scan.snapshot,
        &mut local_store.artifacts,
        &mut local_store.metadata,
        &mut NoopPublicationObserver,
    )
    .map_err(|error| match error {
        ScanError::Internal => r6_internal_failure(),
        other => Failure::Scan(other),
    })?;
    rollback.disarm();
    stage_analysis_cache_best_effort(&mut local_store, &scan.analysis_cache_entries);
    Ok(stdout)
}

fn run_s4_r10(invocation: Invocation, scan_worker: &mut ScanWorker) -> Result<Vec<u8>, Failure> {
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
            let envelope = current_envelope().ok_or_else(r10_internal_failure)?;
            let request = ScanRequest::new(
                invocation.repository,
                invocation.identity,
                invocation.revision,
                envelope,
            );
            ScanService::new(repository_adapter(invocation.packed_sha1))
                .scan_s4_r10(request, &TreeSitterRustWorkspaceExtractor::new())
                .map_err(r10_scan_failure)
        },
    )
    .map_err(|()| r10_internal_failure())??;
    enforce_scan_deadline(started_at)?;
    let stdout = serialize_v12(&scan.snapshot)?;
    let store_was_absent = fs::symlink_metadata(std::path::Path::new(&store))
        .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound);
    let mut rollback = EmptyStoreRollback::new(store.clone(), store_was_absent);
    ensure_store_root_for_boundary(
        std::path::Path::new(&scan_repository),
        std::path::Path::new(&store),
    )
    .map_err(|error| Failure::Scan(ScanError::Storage(error)))?;
    noesis::install_s3_filesystem_boundary(&scan_repository, &store)
        .map_err(|_| r10_internal_failure())?;
    let mut local_store = LocalStore::open(
        std::path::Path::new(&scan_repository),
        std::path::Path::new(&store),
    )
    .map_err(|error| Failure::Scan(ScanError::Storage(error)))?;
    PublicationService::publish_v12(
        &scan.snapshot,
        &mut local_store.artifacts,
        &mut local_store.metadata,
        &mut NoopPublicationObserver,
    )
    .map_err(|error| match error {
        ScanError::Internal => r10_internal_failure(),
        other => Failure::Scan(other),
    })?;
    rollback.disarm();
    stage_analysis_cache_best_effort(&mut local_store, &scan.analysis_cache_entries);
    Ok(stdout)
}

fn run_s4_r10_boundaries(
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
            let envelope = current_envelope().ok_or_else(r10_internal_failure)?;
            let request = ScanRequest::new(
                invocation.repository,
                invocation.identity,
                invocation.revision,
                envelope,
            );
            ScanService::new(repository_adapter(invocation.packed_sha1))
                .scan_s4_r10_boundaries(
                    request,
                    prepared.scan_input,
                    &TreeSitterRustWorkspaceExtractor::new(),
                    &repository_boundaries::Sha256BoundaryHasher,
                )
                .map_err(r10_scan_failure)
        },
    )
    .map_err(|()| r10_internal_failure())??;
    enforce_scan_deadline(started_at)?;
    let stdout = serialize_v12(&scan.snapshot)?;
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
    PublicationService::publish_v12(
        &scan.snapshot,
        &mut local_store.artifacts,
        &mut local_store.metadata,
        &mut NoopPublicationObserver,
    )
    .map_err(|error| match error {
        ScanError::Internal => r10_internal_failure(),
        other => Failure::Scan(other),
    })?;
    rollback.disarm();
    stage_analysis_cache_best_effort(&mut local_store, &scan.analysis_cache_entries);
    Ok(stdout)
}

fn run_s4_r5(invocation: Invocation, scan_worker: &mut ScanWorker) -> Result<Vec<u8>, Failure> {
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
            let envelope = current_envelope().ok_or_else(r5_internal_failure)?;
            let request = ScanRequest::new(
                invocation.repository,
                invocation.identity,
                invocation.revision,
                envelope,
            );
            ScanService::new(repository_adapter(invocation.packed_sha1))
                .scan_s4_r5(request, &TreeSitterRustWorkspaceExtractor::new())
                .map_err(r5_scan_failure)
        },
    )
    .map_err(|()| r5_internal_failure())??;
    enforce_scan_deadline(started_at)?;
    let stdout = serialize_v8(&scan.snapshot)?;
    let store_was_absent = fs::symlink_metadata(std::path::Path::new(&store))
        .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound);
    let mut rollback = EmptyStoreRollback::new(store.clone(), store_was_absent);
    ensure_store_root_for_boundary(
        std::path::Path::new(&scan_repository),
        std::path::Path::new(&store),
    )
    .map_err(|error| Failure::Scan(ScanError::Storage(error)))?;
    noesis::install_s3_filesystem_boundary(&scan_repository, &store)
        .map_err(|_| r5_internal_failure())?;
    let mut local_store = LocalStore::open(
        std::path::Path::new(&scan_repository),
        std::path::Path::new(&store),
    )
    .map_err(|error| Failure::Scan(ScanError::Storage(error)))?;
    PublicationService::publish_v8(
        &scan.snapshot,
        &mut local_store.artifacts,
        &mut local_store.metadata,
        &mut NoopPublicationObserver,
    )
    .map_err(|error| match error {
        ScanError::Internal => r5_internal_failure(),
        other => Failure::Scan(other),
    })?;
    rollback.disarm();
    stage_analysis_cache_best_effort(&mut local_store, &scan.analysis_cache_entries);
    Ok(stdout)
}

fn run_s4_r5_boundaries(
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
            let envelope = current_envelope().ok_or_else(r5_internal_failure)?;
            let request = ScanRequest::new(
                invocation.repository,
                invocation.identity,
                invocation.revision,
                envelope,
            );
            ScanService::new(repository_adapter(invocation.packed_sha1))
                .scan_s4_r5_boundaries(
                    request,
                    prepared.scan_input,
                    &TreeSitterRustWorkspaceExtractor::new(),
                    &repository_boundaries::Sha256BoundaryHasher,
                )
                .map_err(r5_scan_failure)
        },
    )
    .map_err(|()| r5_internal_failure())??;
    enforce_scan_deadline(started_at)?;
    let stdout = serialize_v8(&scan.snapshot)?;
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
    PublicationService::publish_v8(
        &scan.snapshot,
        &mut local_store.artifacts,
        &mut local_store.metadata,
        &mut NoopPublicationObserver,
    )
    .map_err(|error| match error {
        ScanError::Internal => r5_internal_failure(),
        other => Failure::Scan(other),
    })?;
    rollback.disarm();
    stage_analysis_cache_best_effort(&mut local_store, &scan.analysis_cache_entries);
    Ok(stdout)
}

fn run_s4_r4(invocation: Invocation, scan_worker: &mut ScanWorker) -> Result<Vec<u8>, Failure> {
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
            let envelope = current_envelope().ok_or_else(r4_internal_failure)?;
            let request = ScanRequest::new(
                invocation.repository,
                invocation.identity,
                invocation.revision,
                envelope,
            );
            ScanService::new(repository_adapter(invocation.packed_sha1))
                .scan_s4_r4(request, &TreeSitterRustWorkspaceExtractor::new())
                .map_err(r4_scan_failure)
        },
    )
    .map_err(|()| r4_internal_failure())??;
    enforce_scan_deadline(started_at)?;
    let stdout = serialize_v7(&scan.snapshot)?;
    let store_was_absent = fs::symlink_metadata(std::path::Path::new(&store))
        .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound);
    let mut rollback = EmptyStoreRollback::new(store.clone(), store_was_absent);
    ensure_store_root_for_boundary(
        std::path::Path::new(&scan_repository),
        std::path::Path::new(&store),
    )
    .map_err(|error| Failure::Scan(ScanError::Storage(error)))?;
    noesis::install_s3_filesystem_boundary(&scan_repository, &store)
        .map_err(|_| r4_internal_failure())?;
    let mut local_store = LocalStore::open(
        std::path::Path::new(&scan_repository),
        std::path::Path::new(&store),
    )
    .map_err(|error| Failure::Scan(ScanError::Storage(error)))?;
    PublicationService::publish_v7(
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

fn run_s4_r4_boundaries(
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
            let envelope = current_envelope().ok_or_else(r4_internal_failure)?;
            let request = ScanRequest::new(
                invocation.repository,
                invocation.identity,
                invocation.revision,
                envelope,
            );
            ScanService::new(repository_adapter(invocation.packed_sha1))
                .scan_s4_r4_boundaries(
                    request,
                    prepared.scan_input,
                    &TreeSitterRustWorkspaceExtractor::new(),
                    &repository_boundaries::Sha256BoundaryHasher,
                )
                .map_err(r4_scan_failure)
        },
    )
    .map_err(|()| r4_internal_failure())??;
    enforce_scan_deadline(started_at)?;
    let stdout = serialize_v7(&scan.snapshot)?;
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
    PublicationService::publish_v7(
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

fn run_confined_r7_scan<T, F>(
    scan_worker: &mut ScanWorker,
    repository: OsString,
    input: compiler_index::PreparedCompilerIndex,
    operation: F,
) -> Result<T, ()>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    scan_worker.run(move || {
        compiler_index::install_scan_boundary(&repository, &input)?;
        Ok(operation())
    })?
}

fn run_confined_r7_boundary_scan<T, F>(
    scan_worker: &mut ScanWorker,
    repository: OsString,
    manifest_path: Option<std::path::PathBuf>,
    nested_roots: Vec<std::path::PathBuf>,
    input: compiler_index::PreparedCompilerIndex,
    operation: F,
) -> Result<T, ()>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    scan_worker.run(move || {
        compiler_index::install_boundary_scan_boundary(
            &repository,
            manifest_path.as_deref().map(std::path::Path::as_os_str),
            &nested_roots,
            &input,
        )?;
        Ok(operation())
    })?
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

fn run_export(arguments: impl IntoIterator<Item = OsString>) -> Result<Vec<u8>, Failure> {
    let invocation = ExportInvocation::parse(arguments)?;
    if invocation.profile == PortableProfile::R16 {
        return run_export_r16(&invocation);
    }
    if invocation.profile == PortableProfile::R15 {
        return run_export_r15(&invocation);
    }
    if invocation.profile == PortableProfile::R14 {
        return run_export_r14(&invocation);
    }
    if invocation.profile == PortableProfile::R10 {
        return run_export_r10(&invocation);
    }
    if invocation.profile == PortableProfile::K1 {
        return run_export_k1(&invocation);
    }
    let loaded = load_s4_snapshot(&invocation.store, &invocation.identity)
        .map_err(r8_snapshot_load_failure)?;
    let documentation = generate_documentation_v1(
        &loaded.semantic,
        loaded.head.snapshot_id.as_str(),
        &loaded.head.semantic_hash.value,
    )
    .map_err(|_| {
        r8_contract_failure(
            R8ContractError::InvalidSnapshot {
                snapshot_id: loaded.head.snapshot_id.to_string(),
                reason: "incomplete",
            },
            16,
        )
    })?;
    let portable = PortableGraphV1::from_validated_v10(
        &loaded.semantic,
        &loaded.head,
        documentation.manifest(),
        noesis::portable_explorer::sha256,
    )
    .map_err(|error| r8_contract_failure(error, 16))?;
    let store = std::path::Path::new(&invocation.store);
    let output = std::path::Path::new(&invocation.output);
    noesis::portable_explorer::validate_export_output_root(store, output)
        .map_err(|error| r8_portable_failure(error, 16))?;
    let prepared = noesis::portable_explorer::ensure_export_output_root_for_boundary(store, output)
        .map_err(|error| r8_portable_failure(error, 16))?;
    noesis::install_r8_export_filesystem_boundary(&invocation.store, &invocation.output)
        .map_err(|_| r8_internal_failure())?;
    noesis::portable_explorer::publish_portable_graph(&prepared, &portable)
        .map_err(|error| r8_portable_failure(error, 16))
}

fn run_export_r16(invocation: &ExportInvocation) -> Result<Vec<u8>, Failure> {
    let loaded = load_s4_snapshot(&invocation.store, &invocation.identity)
        .map_err(|error| r16_snapshot_load_failure(&error))?;
    if loaded.head.snapshot_schema_version != SNAPSHOT_SCHEMA_VERSION_V18 {
        return Err(r16_contract_failure(
            &R16ContractError::InvalidSnapshot,
            16,
            false,
        ));
    }
    let documents = invocation.documents.as_deref().ok_or_else(|| {
        r16_failure(
            CodeNoesisErrorV24::unsupported_composition("documents_required"),
            2,
        )
    })?;
    validate_documents_root_for_boundary(
        std::path::Path::new(&invocation.store),
        std::path::Path::new(documents),
    )
    .map_err(|_| r16_contract_failure(&R16ContractError::InvalidSnapshot, 16, false))?;
    let documentation = generate_documentation_v1(
        &loaded.semantic,
        loaded.head.snapshot_id.as_str(),
        &loaded.head.semantic_hash.value,
    )
    .map_err(|_| r16_contract_failure(&R16ContractError::InvalidSnapshot, 16, false))?;
    let manifest = load_validated_manifest(
        std::path::Path::new(documents),
        invocation.identity.as_str(),
        loaded.head.snapshot_id.as_str(),
        &loaded.head.semantic_hash.value,
    )
    .map_err(|_| r16_contract_failure(&R16ContractError::InvalidSnapshot, 16, false))?;
    if &manifest != documentation.manifest() {
        return Err(r16_contract_failure(
            &R16ContractError::InvalidSnapshot,
            16,
            false,
        ));
    }
    let portable = PortableGraphV9::from_validated_v18(
        &loaded.semantic,
        &loaded.head,
        &manifest,
        noesis::portable_explorer::sha256,
    )
    .map_err(|error| r16_contract_failure(&error, 16, false))?;
    let store = std::path::Path::new(&invocation.store);
    let output = std::path::Path::new(&invocation.output);
    noesis::portable_explorer::validate_r16_export_output_root(store, output)
        .map_err(|error| r16_portable_failure(error, 16, false))?;
    let prepared =
        noesis::portable_explorer::ensure_r16_export_output_root_for_boundary(store, output)
            .map_err(|error| r16_portable_failure(error, 16, false))?;
    noesis::install_r8_export_filesystem_boundary(&invocation.store, &invocation.output)
        .map_err(|_| r16_internal_failure())?;
    noesis::portable_explorer::publish_portable_graph_v9(&prepared, &portable)
        .map_err(|error| r16_portable_failure(error, 16, false))
}

fn run_export_r15(invocation: &ExportInvocation) -> Result<Vec<u8>, Failure> {
    let loaded = load_s4_snapshot(&invocation.store, &invocation.identity)
        .map_err(r15_snapshot_load_failure)?;
    if loaded.head.snapshot_schema_version != SNAPSHOT_SCHEMA_VERSION_V17 {
        return Err(r15_contract_failure(
            &R15ContractError::UnsupportedSnapshotSchema(
                loaded.head.snapshot_schema_version.clone(),
            ),
            16,
            false,
        ));
    }
    let documents = invocation.documents.as_deref().ok_or_else(|| {
        r15_failure(
            CodeNoesisErrorV22::unsupported_composition("documents_required"),
            2,
        )
    })?;
    validate_documents_root_for_boundary(
        std::path::Path::new(&invocation.store),
        std::path::Path::new(documents),
    )
    .map_err(|_| r15_contract_failure(&R15ContractError::InvalidSnapshot, 16, false))?;
    let documentation = generate_documentation_v1(
        &loaded.semantic,
        loaded.head.snapshot_id.as_str(),
        &loaded.head.semantic_hash.value,
    )
    .map_err(|_| r15_contract_failure(&R15ContractError::InvalidSnapshot, 16, false))?;
    let manifest = load_validated_manifest(
        std::path::Path::new(documents),
        invocation.identity.as_str(),
        loaded.head.snapshot_id.as_str(),
        &loaded.head.semantic_hash.value,
    )
    .map_err(|_| r15_contract_failure(&R15ContractError::InvalidSnapshot, 16, false))?;
    if &manifest != documentation.manifest() {
        return Err(r15_contract_failure(
            &R15ContractError::InvalidSnapshot,
            16,
            false,
        ));
    }
    let portable = PortableGraphV8::from_validated_v17(
        &loaded.semantic,
        &loaded.head,
        &manifest,
        noesis::portable_explorer::sha256,
    )
    .map_err(|error| r15_contract_failure(&error, 16, false))?;
    let store = std::path::Path::new(&invocation.store);
    let output = std::path::Path::new(&invocation.output);
    noesis::portable_explorer::validate_r15_export_output_root(store, output)
        .map_err(|error| r15_portable_failure(error, 16, false))?;
    let prepared =
        noesis::portable_explorer::ensure_r15_export_output_root_for_boundary(store, output)
            .map_err(|error| r15_portable_failure(error, 16, false))?;
    noesis::install_r8_export_filesystem_boundary(&invocation.store, &invocation.output)
        .map_err(|_| r15_internal_failure())?;
    noesis::portable_explorer::publish_portable_graph_v8(&prepared, &portable)
        .map_err(|error| r15_portable_failure(error, 16, false))
}

fn run_export_r10(invocation: &ExportInvocation) -> Result<Vec<u8>, Failure> {
    let loaded = load_s4_snapshot(&invocation.store, &invocation.identity)
        .map_err(r10_snapshot_load_failure)?;
    if loaded.head.snapshot_schema_version != SNAPSHOT_SCHEMA_VERSION_V12 {
        return Err(r10_contract_failure(
            &R10ContractError::UnsupportedSnapshotSchema(
                loaded.head.snapshot_schema_version.clone(),
            ),
            16,
            false,
        ));
    }
    let documentation = generate_documentation_v1(
        &loaded.semantic,
        loaded.head.snapshot_id.as_str(),
        &loaded.head.semantic_hash.value,
    )
    .map_err(|_| r10_contract_failure(&R10ContractError::InvalidSnapshot, 16, false))?;
    let portable = PortableGraphV3::from_validated_v12(
        &loaded.semantic,
        &loaded.head,
        documentation.manifest(),
        noesis::portable_explorer::sha256,
    )
    .map_err(|error| r10_contract_failure(&error, 16, false))?;
    let store = std::path::Path::new(&invocation.store);
    let output = std::path::Path::new(&invocation.output);
    noesis::portable_explorer::validate_r10_export_output_root(store, output)
        .map_err(|error| r10_portable_failure(error, 16, false))?;
    let prepared =
        noesis::portable_explorer::ensure_r10_export_output_root_for_boundary(store, output)
            .map_err(|error| r10_portable_failure(error, 16, false))?;
    noesis::install_r8_export_filesystem_boundary(&invocation.store, &invocation.output)
        .map_err(|_| r10_internal_failure())?;
    noesis::portable_explorer::publish_portable_graph_v3(&prepared, &portable)
        .map_err(|error| r10_portable_failure(error, 16, false))
}

fn run_export_k1(invocation: &ExportInvocation) -> Result<Vec<u8>, Failure> {
    let loaded = load_s4_snapshot(&invocation.store, &invocation.identity)
        .map_err(k1_snapshot_load_failure)?;
    if loaded.head.snapshot_schema_version == SNAPSHOT_SCHEMA_VERSION_V15 {
        return run_export_r13(invocation, &loaded);
    }
    if loaded.head.snapshot_schema_version == SNAPSHOT_SCHEMA_VERSION_V14 {
        return run_export_r12(invocation, &loaded);
    }
    if loaded.head.snapshot_schema_version == SNAPSHOT_SCHEMA_VERSION_V13 {
        return run_export_r11(invocation, &loaded);
    }
    if loaded.head.snapshot_schema_version != SNAPSHOT_SCHEMA_VERSION_V11 {
        return Err(k1_failure(
            CodeNoesisErrorV16::from_contract(&K1ContractError::UnsupportedSnapshotSchema(
                loaded.head.snapshot_schema_version,
            )),
            16,
        ));
    }
    let documentation = generate_documentation_v1(
        &loaded.semantic,
        loaded.head.snapshot_id.as_str(),
        &loaded.head.semantic_hash.value,
    )
    .map_err(|_| {
        k1_failure(
            CodeNoesisErrorV16::from_contract(&K1ContractError::InvalidSnapshot),
            16,
        )
    })?;
    let portable = PortableGraphV2::from_validated_v11(
        &loaded.semantic,
        &loaded.head,
        documentation.manifest(),
        noesis::portable_explorer::sha256,
    )
    .map_err(|error| k1_contract_failure(&error, 16, false))?;
    let store = std::path::Path::new(&invocation.store);
    let output = std::path::Path::new(&invocation.output);
    noesis::portable_explorer::validate_k1_export_output_root(store, output)
        .map_err(|error| k1_portable_failure(error, 16, false))?;
    let prepared =
        noesis::portable_explorer::ensure_k1_export_output_root_for_boundary(store, output)
            .map_err(|error| k1_portable_failure(error, 16, false))?;
    noesis::install_r8_export_filesystem_boundary(&invocation.store, &invocation.output)
        .map_err(|_| k1_internal_failure())?;
    noesis::portable_explorer::publish_portable_graph_v2(&prepared, &portable)
        .map_err(|error| k1_portable_failure(error, 16, false))
}

fn run_export_r11(
    invocation: &ExportInvocation,
    loaded: &LoadedS4Snapshot,
) -> Result<Vec<u8>, Failure> {
    let documentation = generate_documentation_v1(
        &loaded.semantic,
        loaded.head.snapshot_id.as_str(),
        &loaded.head.semantic_hash.value,
    )
    .map_err(|_| r11_contract_failure(&R11ContractError::InvalidSnapshot, 16, false))?;
    let portable = PortableGraphV4::from_validated_v13(
        &loaded.semantic,
        &loaded.head,
        documentation.manifest(),
        noesis::portable_explorer::sha256,
    )
    .map_err(|error| r11_contract_failure(&error, 16, false))?;
    let store = std::path::Path::new(&invocation.store);
    let output = std::path::Path::new(&invocation.output);
    noesis::portable_explorer::validate_r11_export_output_root(store, output)
        .map_err(|error| r11_portable_failure(error, 16, false))?;
    let prepared =
        noesis::portable_explorer::ensure_r11_export_output_root_for_boundary(store, output)
            .map_err(|error| r11_portable_failure(error, 16, false))?;
    noesis::install_r8_export_filesystem_boundary(&invocation.store, &invocation.output)
        .map_err(|_| r11_internal_failure())?;
    noesis::portable_explorer::publish_portable_graph_v4(&prepared, &portable)
        .map_err(|error| r11_portable_failure(error, 16, false))
}

fn run_export_r12(
    invocation: &ExportInvocation,
    loaded: &LoadedS4Snapshot,
) -> Result<Vec<u8>, Failure> {
    let documentation = generate_documentation_v1(
        &loaded.semantic,
        loaded.head.snapshot_id.as_str(),
        &loaded.head.semantic_hash.value,
    )
    .map_err(|_| r12_contract_failure(&R12ContractError::InvalidSnapshot, 16, false))?;
    let portable = PortableGraphV5::from_validated_v14(
        &loaded.semantic,
        &loaded.head,
        documentation.manifest(),
        noesis::portable_explorer::sha256,
    )
    .map_err(|error| r12_contract_failure(&error, 16, false))?;
    let store = std::path::Path::new(&invocation.store);
    let output = std::path::Path::new(&invocation.output);
    noesis::portable_explorer::validate_r12_export_output_root(store, output)
        .map_err(|error| r12_portable_failure(error, 16, false))?;
    let prepared =
        noesis::portable_explorer::ensure_r12_export_output_root_for_boundary(store, output)
            .map_err(|error| r12_portable_failure(error, 16, false))?;
    noesis::install_r8_export_filesystem_boundary(&invocation.store, &invocation.output)
        .map_err(|_| r12_internal_failure())?;
    noesis::portable_explorer::publish_portable_graph_v5(&prepared, &portable)
        .map_err(|error| r12_portable_failure(error, 16, false))
}

fn run_export_r13(
    invocation: &ExportInvocation,
    loaded: &LoadedS4Snapshot,
) -> Result<Vec<u8>, Failure> {
    let documentation = generate_documentation_v1(
        &loaded.semantic,
        loaded.head.snapshot_id.as_str(),
        &loaded.head.semantic_hash.value,
    )
    .map_err(|_| r13_contract_failure(&R13ContractError::InvalidSnapshot, 16, false))?;
    let portable = PortableGraphV6::from_validated_v15(
        &loaded.semantic,
        &loaded.head,
        documentation.manifest(),
        noesis::portable_explorer::sha256,
    )
    .map_err(|error| r13_contract_failure(&error, 16, false))?;
    let store = std::path::Path::new(&invocation.store);
    let output = std::path::Path::new(&invocation.output);
    noesis::portable_explorer::validate_r13_export_output_root(store, output)
        .map_err(|error| r13_portable_failure(error, 16, false))?;
    let prepared =
        noesis::portable_explorer::ensure_r13_export_output_root_for_boundary(store, output)
            .map_err(|error| r13_portable_failure(error, 16, false))?;
    noesis::install_r8_export_filesystem_boundary(&invocation.store, &invocation.output)
        .map_err(|_| r13_internal_failure())?;
    noesis::portable_explorer::publish_portable_graph_v6(&prepared, &portable)
        .map_err(|error| r13_portable_failure(error, 16, false))
}

fn run_export_r14(invocation: &ExportInvocation) -> Result<Vec<u8>, Failure> {
    let loaded = load_s4_snapshot(&invocation.store, &invocation.identity)
        .map_err(r14_snapshot_load_failure)?;
    if loaded.head.snapshot_schema_version != SNAPSHOT_SCHEMA_VERSION_V16 {
        return Err(r14_contract_failure(
            &R14ContractError::UnsupportedSnapshotSchema(
                loaded.head.snapshot_schema_version.clone(),
            ),
            16,
            false,
        ));
    }
    let documents = invocation.documents.as_deref().ok_or_else(|| {
        r14_failure(
            CodeNoesisErrorV21::unsupported_composition("documents_required"),
            2,
        )
    })?;
    validate_documents_root_for_boundary(
        std::path::Path::new(&invocation.store),
        std::path::Path::new(documents),
    )
    .map_err(|_| r14_contract_failure(&R14ContractError::InvalidSnapshot, 16, false))?;
    let documentation = generate_documentation_v1(
        &loaded.semantic,
        loaded.head.snapshot_id.as_str(),
        &loaded.head.semantic_hash.value,
    )
    .map_err(|_| r14_contract_failure(&R14ContractError::InvalidSnapshot, 16, false))?;
    let manifest = load_validated_manifest(
        std::path::Path::new(documents),
        invocation.identity.as_str(),
        loaded.head.snapshot_id.as_str(),
        &loaded.head.semantic_hash.value,
    )
    .map_err(|_| r14_contract_failure(&R14ContractError::InvalidSnapshot, 16, false))?;
    if &manifest != documentation.manifest() {
        return Err(r14_contract_failure(
            &R14ContractError::InvalidSnapshot,
            16,
            false,
        ));
    }
    let portable = PortableGraphV7::from_validated_v16(
        &loaded.semantic,
        &loaded.head,
        &manifest,
        noesis::portable_explorer::sha256,
    )
    .map_err(|error| r14_contract_failure(&error, 16, false))?;
    let store = std::path::Path::new(&invocation.store);
    let output = std::path::Path::new(&invocation.output);
    noesis::portable_explorer::validate_r14_export_output_root(store, output)
        .map_err(|error| r14_portable_failure(error, 16, false))?;
    let prepared =
        noesis::portable_explorer::ensure_r14_export_output_root_for_boundary(store, output)
            .map_err(|error| r14_portable_failure(error, 16, false))?;
    noesis::install_r8_export_filesystem_boundary(&invocation.store, &invocation.output)
        .map_err(|_| r14_internal_failure())?;
    noesis::portable_explorer::publish_portable_graph_v7(&prepared, &portable)
        .map_err(|error| r14_portable_failure(error, 16, false))
}

fn run_explore(arguments: impl IntoIterator<Item = OsString>) -> Result<Vec<u8>, Failure> {
    let invocation = ExploreInvocation::parse(arguments)?;
    if invocation.profile == PortableProfile::R17 {
        return run_explore_r17(&invocation);
    }
    if invocation.profile == PortableProfile::R16 {
        return run_explore_r16(&invocation);
    }
    if invocation.profile == PortableProfile::R15 {
        return run_explore_r15(&invocation);
    }
    if invocation.profile == PortableProfile::R14 {
        return run_explore_r14(&invocation);
    }
    if invocation.profile == PortableProfile::R10 {
        return run_explore_r10(&invocation);
    }
    if invocation.profile == PortableProfile::K1 {
        return run_explore_k1(&invocation);
    }
    let input = std::path::Path::new(&invocation.input);
    let output = std::path::Path::new(&invocation.output);
    let (portable, portable_bytes) = noesis::portable_explorer::read_portable_graph(input)
        .map_err(|error| r8_portable_failure(error, 17))?;
    noesis::portable_explorer::validate_explorer_output_root(input, output)
        .map_err(|error| r8_portable_failure(error, 17))?;
    let prepared =
        noesis::portable_explorer::ensure_explorer_output_root_for_boundary(input, output)
            .map_err(|error| r8_portable_failure(error, 17))?;
    noesis::install_r8_explorer_filesystem_boundary(&invocation.input, &invocation.output)
        .map_err(|_| r8_internal_failure())?;
    noesis::portable_explorer::publish_local_explorer(&prepared, &portable, &portable_bytes)
        .map_err(|error| r8_portable_failure(error, 17))
}

fn run_explore_r17(invocation: &ExploreInvocation) -> Result<Vec<u8>, Failure> {
    let input = std::path::Path::new(&invocation.input);
    let output = std::path::Path::new(&invocation.output);
    let (portable, portable_bytes) = noesis::portable_explorer::read_portable_graph_v9(input)
        .map_err(|error| r16_portable_failure(error, 17, true))?;
    noesis::portable_explorer::validate_r17_explorer_output_root(input, output)
        .map_err(|error| r16_portable_failure(error, 17, true))?;
    let prepared =
        noesis::portable_explorer::ensure_r17_explorer_output_root_for_boundary(input, output)
            .map_err(|error| r16_portable_failure(error, 17, true))?;
    noesis::install_r8_explorer_filesystem_boundary(&invocation.input, &invocation.output)
        .map_err(|_| r16_internal_failure())?;
    noesis::portable_explorer::publish_local_explorer_v10(&prepared, &portable, &portable_bytes)
        .map_err(|error| r16_portable_failure(error, 17, true))
}

fn run_explore_r16(invocation: &ExploreInvocation) -> Result<Vec<u8>, Failure> {
    let input = std::path::Path::new(&invocation.input);
    let output = std::path::Path::new(&invocation.output);
    let (portable, portable_bytes) = noesis::portable_explorer::read_portable_graph_v9(input)
        .map_err(|error| r16_portable_failure(error, 17, true))?;
    noesis::portable_explorer::validate_r16_explorer_output_root(input, output)
        .map_err(|error| r16_portable_failure(error, 17, true))?;
    let prepared =
        noesis::portable_explorer::ensure_r16_explorer_output_root_for_boundary(input, output)
            .map_err(|error| r16_portable_failure(error, 17, true))?;
    noesis::install_r8_explorer_filesystem_boundary(&invocation.input, &invocation.output)
        .map_err(|_| r16_internal_failure())?;
    noesis::portable_explorer::publish_local_explorer_v9(&prepared, &portable, &portable_bytes)
        .map_err(|error| r16_portable_failure(error, 17, true))
}

fn run_explore_r15(invocation: &ExploreInvocation) -> Result<Vec<u8>, Failure> {
    let input = std::path::Path::new(&invocation.input);
    let output = std::path::Path::new(&invocation.output);
    let (portable, portable_bytes) = noesis::portable_explorer::read_portable_graph_v8(input)
        .map_err(|error| r15_portable_failure(error, 17, true))?;
    noesis::portable_explorer::validate_r15_explorer_output_root(input, output)
        .map_err(|error| r15_portable_failure(error, 17, true))?;
    let prepared =
        noesis::portable_explorer::ensure_r15_explorer_output_root_for_boundary(input, output)
            .map_err(|error| r15_portable_failure(error, 17, true))?;
    noesis::install_r8_explorer_filesystem_boundary(&invocation.input, &invocation.output)
        .map_err(|_| r15_internal_failure())?;
    noesis::portable_explorer::publish_local_explorer_v8(&prepared, &portable, &portable_bytes)
        .map_err(|error| r15_portable_failure(error, 17, true))
}

fn run_explore_r10(invocation: &ExploreInvocation) -> Result<Vec<u8>, Failure> {
    let input = std::path::Path::new(&invocation.input);
    let output = std::path::Path::new(&invocation.output);
    let (portable, portable_bytes) = noesis::portable_explorer::read_portable_graph_v3(input)
        .map_err(|error| r10_portable_failure(error, 17, true))?;
    noesis::portable_explorer::validate_r10_explorer_output_root(input, output)
        .map_err(|error| r10_portable_failure(error, 17, true))?;
    let prepared =
        noesis::portable_explorer::ensure_r10_explorer_output_root_for_boundary(input, output)
            .map_err(|error| r10_portable_failure(error, 17, true))?;
    noesis::install_r8_explorer_filesystem_boundary(&invocation.input, &invocation.output)
        .map_err(|_| r10_internal_failure())?;
    noesis::portable_explorer::publish_local_explorer_v3(&prepared, &portable, &portable_bytes)
        .map_err(|error| r10_portable_failure(error, 17, true))
}

fn run_explore_k1(invocation: &ExploreInvocation) -> Result<Vec<u8>, Failure> {
    let input = std::path::Path::new(&invocation.input);
    let output = std::path::Path::new(&invocation.output);
    let (portable, portable_bytes) = match noesis::portable_explorer::read_portable_graph_v2(input)
    {
        Ok(portable) => portable,
        Err(noesis::portable_explorer::PortableExplorerError::K1Contract(
            K1ContractError::UnsupportedPortableGraphSchema(_),
        )) => return run_explore_r11(invocation),
        Err(error) => return Err(k1_portable_failure(error, 17, true)),
    };
    noesis::portable_explorer::validate_k1_explorer_output_root(input, output)
        .map_err(|error| k1_portable_failure(error, 17, true))?;
    let prepared =
        noesis::portable_explorer::ensure_k1_explorer_output_root_for_boundary(input, output)
            .map_err(|error| k1_portable_failure(error, 17, true))?;
    noesis::install_r8_explorer_filesystem_boundary(&invocation.input, &invocation.output)
        .map_err(|_| k1_internal_failure())?;
    noesis::portable_explorer::publish_local_explorer_v2(&prepared, &portable, &portable_bytes)
        .map_err(|error| k1_portable_failure(error, 17, true))
}

fn run_explore_r11(invocation: &ExploreInvocation) -> Result<Vec<u8>, Failure> {
    let input = std::path::Path::new(&invocation.input);
    let output = std::path::Path::new(&invocation.output);
    let (portable, portable_bytes) = match noesis::portable_explorer::read_portable_graph_v4(input)
    {
        Ok(portable) => portable,
        Err(noesis::portable_explorer::PortableExplorerError::R11Contract(
            R11ContractError::UnsupportedPortableGraphSchema(observed),
        )) if matches!(
            observed.as_str(),
            R12_PORTABLE_GRAPH_VERSION | R13_PORTABLE_GRAPH_VERSION
        ) =>
        {
            return run_explore_r12(invocation);
        }
        Err(error) => return Err(r11_portable_failure(error, 17, true)),
    };
    noesis::portable_explorer::validate_r11_explorer_output_root(input, output)
        .map_err(|error| r11_portable_failure(error, 17, true))?;
    let prepared =
        noesis::portable_explorer::ensure_r11_explorer_output_root_for_boundary(input, output)
            .map_err(|error| r11_portable_failure(error, 17, true))?;
    noesis::install_r8_explorer_filesystem_boundary(&invocation.input, &invocation.output)
        .map_err(|_| r11_internal_failure())?;
    noesis::portable_explorer::publish_local_explorer_v4(&prepared, &portable, &portable_bytes)
        .map_err(|error| r11_portable_failure(error, 17, true))
}

fn run_explore_r12(invocation: &ExploreInvocation) -> Result<Vec<u8>, Failure> {
    let input = std::path::Path::new(&invocation.input);
    let output = std::path::Path::new(&invocation.output);
    let (portable, portable_bytes) = match noesis::portable_explorer::read_portable_graph_v5(input)
    {
        Ok(portable) => portable,
        Err(noesis::portable_explorer::PortableExplorerError::R12Contract(
            R12ContractError::UnsupportedPortableGraphSchema(observed),
        )) if observed == R13_PORTABLE_GRAPH_VERSION => return run_explore_r13(invocation),
        Err(error) => return Err(r12_portable_failure(error, 17, true)),
    };
    noesis::portable_explorer::validate_r12_explorer_output_root(input, output)
        .map_err(|error| r12_portable_failure(error, 17, true))?;
    let prepared =
        noesis::portable_explorer::ensure_r12_explorer_output_root_for_boundary(input, output)
            .map_err(|error| r12_portable_failure(error, 17, true))?;
    noesis::install_r8_explorer_filesystem_boundary(&invocation.input, &invocation.output)
        .map_err(|_| r12_internal_failure())?;
    noesis::portable_explorer::publish_local_explorer_v5(&prepared, &portable, &portable_bytes)
        .map_err(|error| r12_portable_failure(error, 17, true))
}

fn run_explore_r13(invocation: &ExploreInvocation) -> Result<Vec<u8>, Failure> {
    let input = std::path::Path::new(&invocation.input);
    let output = std::path::Path::new(&invocation.output);
    let (portable, portable_bytes) = noesis::portable_explorer::read_portable_graph_v6(input)
        .map_err(|error| r13_portable_failure(error, 17, true))?;
    noesis::portable_explorer::validate_r13_explorer_output_root(input, output)
        .map_err(|error| r13_portable_failure(error, 17, true))?;
    let prepared =
        noesis::portable_explorer::ensure_r13_explorer_output_root_for_boundary(input, output)
            .map_err(|error| r13_portable_failure(error, 17, true))?;
    noesis::install_r8_explorer_filesystem_boundary(&invocation.input, &invocation.output)
        .map_err(|_| r13_internal_failure())?;
    noesis::portable_explorer::publish_local_explorer_v6(&prepared, &portable, &portable_bytes)
        .map_err(|error| r13_portable_failure(error, 17, true))
}

fn run_explore_r14(invocation: &ExploreInvocation) -> Result<Vec<u8>, Failure> {
    let input = std::path::Path::new(&invocation.input);
    let output = std::path::Path::new(&invocation.output);
    let (portable, portable_bytes) = noesis::portable_explorer::read_portable_graph_v7(input)
        .map_err(|error| r14_portable_failure(error, 17, true))?;
    noesis::portable_explorer::validate_r14_explorer_output_root(input, output)
        .map_err(|error| r14_portable_failure(error, 17, true))?;
    let prepared =
        noesis::portable_explorer::ensure_r14_explorer_output_root_for_boundary(input, output)
            .map_err(|error| r14_portable_failure(error, 17, true))?;
    noesis::install_r8_explorer_filesystem_boundary(&invocation.input, &invocation.output)
        .map_err(|_| r14_internal_failure())?;
    noesis::portable_explorer::publish_local_explorer_v7(&prepared, &portable, &portable_bytes)
        .map_err(|error| r14_portable_failure(error, 17, true))
}

fn run_source(arguments: impl IntoIterator<Item = OsString>) -> Result<Vec<u8>, Failure> {
    let invocation = SourceInvocation::parse(arguments)?;
    let loaded = load_s4_snapshot(&invocation.store, &invocation.identity)
        .map_err(|_| r18_failure(CodeNoesisErrorV29::invalid_snapshot(), 2))?;
    let selection = TrustedSourceSelectionV1::from_validated_v18(
        &loaded.semantic,
        &loaded.head,
        &invocation.evidence_id,
    )
    .map_err(|error| r18_contract_failure(&error))?;
    if selection.commit_oid() != &invocation.revision {
        return Err(r18_failure(CodeNoesisErrorV29::repository_mismatch(), 2));
    }
    let acquirer = if invocation.packed_sha1 {
        LocalGitRepository::new_packed_sha1()
    } else {
        LocalGitRepository::new()
    };
    let service = TrustedSourceRetrievalService::new(acquirer);
    let request = TrustedSourceRequest::new(
        invocation.repository,
        selection,
        invocation.boundary_profile,
    );
    let excerpt = service
        .retrieve(
            &request,
            &repository_boundaries::Sha256BoundaryHasher,
            r18_sha256,
        )
        .map_err(r18_retrieval_failure)?;
    let reloaded = load_s4_snapshot(&invocation.store, &invocation.identity)
        .map_err(|_| r18_failure(CodeNoesisErrorV29::unstable_input(), 2))?;
    if reloaded.head != loaded.head || reloaded.semantic != loaded.semantic {
        return Err(r18_failure(CodeNoesisErrorV29::unstable_input(), 2));
    }
    Ok(excerpt.canonical_stdout())
}

fn r18_retrieval_failure(error: TrustedSourceRetrievalError) -> Failure {
    match error {
        TrustedSourceRetrievalError::Repository(RepositoryError::Unexpected) => {
            r18_internal_failure()
        }
        TrustedSourceRetrievalError::Repository(RepositoryError::Acquisition(error)) => {
            r18_acquisition_failure(&error)
        }
        TrustedSourceRetrievalError::Boundary(RepositoryBoundaryError::LimitExceeded {
            ..
        }) => r18_failure(CodeNoesisErrorV29::limit_exceeded(), 2),
        TrustedSourceRetrievalError::Boundary(_) => {
            r18_failure(CodeNoesisErrorV29::acquisition_rejected(), 2)
        }
        TrustedSourceRetrievalError::Contract(error) => r18_contract_failure(&error),
    }
}

fn r18_acquisition_failure(error: &AcquisitionError) -> Failure {
    match error {
        AcquisitionError::LimitExceeded { .. } => {
            r18_failure(CodeNoesisErrorV29::limit_exceeded(), 2)
        }
        AcquisitionError::PathInvalid { .. }
        | AcquisitionError::RootPolicyViolation { .. }
        | AcquisitionError::EntryPolicyViolation { .. } => {
            r18_failure(CodeNoesisErrorV29::path_rejected(), 2)
        }
        AcquisitionError::UnsupportedRepositoryShape {
            feature:
                UnsupportedFeature::PackedAcquisition(
                    codenoesis_domain::s1_packed::PackedAcquisitionError::Changed(_),
                ),
        } => r18_failure(CodeNoesisErrorV29::unstable_input(), 2),
        _ => r18_failure(CodeNoesisErrorV29::acquisition_rejected(), 2),
    }
}

fn r18_contract_failure(error: &TrustedSourceError) -> Failure {
    let exit_code = if error == &TrustedSourceError::Internal {
        1
    } else {
        2
    };
    r18_failure(CodeNoesisErrorV29::from_contract(error), exit_code)
}

fn r18_sha256(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        write!(&mut output, "{byte:02x}").expect("writing SHA-256 hex cannot fail");
    }
    output
}

fn run_docs(arguments: impl IntoIterator<Item = OsString>) -> Result<Vec<u8>, Failure> {
    let invocation = DocsInvocation::parse(arguments)?;
    let loaded =
        load_s4_snapshot(&invocation.store, &invocation.identity).map_err(|error| match error {
            LoadS4Error::Scan(error) => Failure::Scan(error),
            LoadS4Error::SnapshotMismatch | LoadS4Error::UnsupportedSnapshotSchema(_) => {
                Failure::Docs(GeneratedDocsError::SnapshotMismatch)
            }
        })?;
    let docs_v12 = loaded.head.snapshot_schema_version == SNAPSHOT_SCHEMA_VERSION_V12;
    let docs_v15 = loaded.head.snapshot_schema_version == SNAPSHOT_SCHEMA_VERSION_V15;
    let docs_v16 = loaded.head.snapshot_schema_version == SNAPSHOT_SCHEMA_VERSION_V16;
    let docs_v17 = loaded.head.snapshot_schema_version == SNAPSHOT_SCHEMA_VERSION_V17;
    let docs_v18 = loaded.head.snapshot_schema_version == SNAPSHOT_SCHEMA_VERSION_V18;
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
        DocumentationContractError::InvalidSnapshot if docs_v18 => {
            r16_failure(CodeNoesisErrorV24::invalid_snapshot(), 13)
        }
        DocumentationContractError::InvalidSnapshot if docs_v17 => {
            r15_failure(CodeNoesisErrorV22::invalid_snapshot(), 13)
        }
        DocumentationContractError::InvalidSnapshot if docs_v16 => {
            r14_failure(CodeNoesisErrorV21::invalid_snapshot(), 13)
        }
        DocumentationContractError::InvalidSnapshot if docs_v15 => {
            r13_failure(CodeNoesisErrorV20::invalid_snapshot(), 13)
        }
        DocumentationContractError::InvalidSnapshot if docs_v12 => {
            r10_failure(CodeNoesisErrorV17::invalid_snapshot(), 13)
        }
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

#[allow(clippy::too_many_lines)]
fn run_query(arguments: impl IntoIterator<Item = OsString>) -> Result<Vec<u8>, Failure> {
    let invocation = QueryInvocation::parse(arguments)?;
    let loaded =
        load_s4_snapshot(&invocation.store, &invocation.identity).map_err(|error| match error {
            LoadS4Error::Scan(error) => Failure::Scan(error),
            LoadS4Error::SnapshotMismatch | LoadS4Error::UnsupportedSnapshotSchema(_) => {
                Failure::Query(QueryFailure::SnapshotMismatch)
            }
        })?;
    let constant_query = loaded.head.snapshot_schema_version == SNAPSHOT_SCHEMA_VERSION_V18;
    let latest_query_schema = loaded.head.snapshot_schema_version == SNAPSHOT_SCHEMA_VERSION_V17;
    let query_v11 = loaded.head.snapshot_schema_version == SNAPSHOT_SCHEMA_VERSION_V16;
    let query_v10 = loaded.head.snapshot_schema_version == SNAPSHOT_SCHEMA_VERSION_V15;
    let query_v9 = loaded.head.snapshot_schema_version == SNAPSHOT_SCHEMA_VERSION_V14;
    let query_v8 = loaded.head.snapshot_schema_version == SNAPSHOT_SCHEMA_VERSION_V13;
    let query_v7 = loaded.head.snapshot_schema_version == SNAPSHOT_SCHEMA_VERSION_V12;
    let query_v6 = loaded.head.snapshot_schema_version == SNAPSHOT_SCHEMA_VERSION_V11;
    let query_v5 = loaded.head.snapshot_schema_version == SNAPSHOT_SCHEMA_VERSION_V10;
    let query_v4 = loaded.head.snapshot_schema_version == SNAPSHOT_SCHEMA_VERSION_V9;
    let query_v3 = loaded.head.snapshot_schema_version == SNAPSHOT_SCHEMA_VERSION_V8;
    let query_v2 = loaded.head.snapshot_schema_version == SNAPSHOT_SCHEMA_VERSION_V7;
    if !constant_query
        && !latest_query_schema
        && !query_v11
        && !query_v10
        && !query_v9
        && !query_v8
        && !query_v7
        && !query_v6
        && !query_v5
        && !query_v4
        && v4_only_query_id(&invocation.requested_id)
    {
        return Err(Failure::S4Input(CodeNoesisErrorV5::invalid_query_id()));
    }
    if !constant_query
        && !latest_query_schema
        && !query_v11
        && !query_v10
        && !query_v9
        && !query_v8
        && !query_v7
        && !query_v6
        && !query_v5
        && !query_v4
        && !query_v3
        && !query_v2
        && v2_only_query_id(&invocation.requested_id)
    {
        return Err(Failure::S4Input(CodeNoesisErrorV5::invalid_query_id()));
    }
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
    .map_err(|error| {
        if constant_query {
            return match error {
                GeneratedDocsError::SnapshotMismatch
                | GeneratedDocsError::InvalidRoot
                | GeneratedDocsError::UnmarkedNonemptyRoot
                | GeneratedDocsError::UnsafePath
                | GeneratedDocsError::CorruptGeneration
                | GeneratedDocsError::Failed => {
                    r16_failure(CodeNoesisErrorV24::invalid_query(), 14)
                }
            };
        }
        if latest_query_schema {
            return match error {
                GeneratedDocsError::SnapshotMismatch
                | GeneratedDocsError::InvalidRoot
                | GeneratedDocsError::UnmarkedNonemptyRoot
                | GeneratedDocsError::UnsafePath
                | GeneratedDocsError::CorruptGeneration
                | GeneratedDocsError::Failed => {
                    r15_failure(CodeNoesisErrorV22::invalid_query(), 14)
                }
            };
        }
        if query_v11 {
            return match error {
                GeneratedDocsError::SnapshotMismatch
                | GeneratedDocsError::InvalidRoot
                | GeneratedDocsError::UnmarkedNonemptyRoot
                | GeneratedDocsError::UnsafePath
                | GeneratedDocsError::CorruptGeneration
                | GeneratedDocsError::Failed => {
                    r14_failure(CodeNoesisErrorV21::invalid_query(), 14)
                }
            };
        }
        if query_v10 {
            return match error {
                GeneratedDocsError::SnapshotMismatch => {
                    r13_failure(CodeNoesisErrorV20::invalid_query("snapshot_mismatch"), 14)
                }
                GeneratedDocsError::InvalidRoot
                | GeneratedDocsError::UnmarkedNonemptyRoot
                | GeneratedDocsError::UnsafePath
                | GeneratedDocsError::CorruptGeneration
                | GeneratedDocsError::Failed => {
                    r13_failure(CodeNoesisErrorV20::invalid_query("documents_invalid"), 14)
                }
            };
        }
        if query_v9 {
            return match error {
                GeneratedDocsError::SnapshotMismatch => {
                    r12_failure(CodeNoesisErrorV19::invalid_query("snapshot_mismatch"), 14)
                }
                GeneratedDocsError::InvalidRoot
                | GeneratedDocsError::UnmarkedNonemptyRoot
                | GeneratedDocsError::UnsafePath
                | GeneratedDocsError::CorruptGeneration
                | GeneratedDocsError::Failed => {
                    r12_failure(CodeNoesisErrorV19::invalid_query("documents_invalid"), 14)
                }
            };
        }
        if query_v8 {
            return match error {
                GeneratedDocsError::SnapshotMismatch => {
                    r11_failure(CodeNoesisErrorV18::invalid_snapshot(), 14)
                }
                GeneratedDocsError::InvalidRoot
                | GeneratedDocsError::UnmarkedNonemptyRoot
                | GeneratedDocsError::UnsafePath
                | GeneratedDocsError::CorruptGeneration
                | GeneratedDocsError::Failed => {
                    r11_failure(CodeNoesisErrorV18::invalid_query(), 14)
                }
            };
        }
        if query_v7 {
            return match error {
                GeneratedDocsError::SnapshotMismatch => {
                    r10_failure(CodeNoesisErrorV17::invalid_snapshot(), 14)
                }
                GeneratedDocsError::InvalidRoot
                | GeneratedDocsError::UnmarkedNonemptyRoot
                | GeneratedDocsError::UnsafePath
                | GeneratedDocsError::CorruptGeneration
                | GeneratedDocsError::Failed => {
                    r10_failure(CodeNoesisErrorV17::invalid_query(), 14)
                }
            };
        }
        match error {
            GeneratedDocsError::SnapshotMismatch => Failure::Query(QueryFailure::SnapshotMismatch),
            GeneratedDocsError::InvalidRoot
            | GeneratedDocsError::UnmarkedNonemptyRoot
            | GeneratedDocsError::UnsafePath
            | GeneratedDocsError::CorruptGeneration
            | GeneratedDocsError::Failed => Failure::Query(QueryFailure::CorruptDocuments),
        }
    })?;
    let stdout = if invocation.context_profile.is_some() {
        if !constant_query {
            return Err(r16_failure(CodeNoesisErrorV24::invalid_query(), 14));
        }
        FunctionContextV1::from_validated_v18(
            &loaded.semantic,
            &loaded.head,
            &invocation.requested_id,
        )
        .map_err(|error| function_context_contract_failure(&error))?
        .canonical_stdout()
        .map_err(|error| function_context_contract_failure(&error))?
    } else if constant_query {
        local_query_result_v13(
            &loaded.semantic,
            &manifest,
            loaded.head.snapshot_id.as_str(),
            &invocation.requested_id,
        )
        .map_err(query_v13_contract_failure)?
        .canonical_stdout()
        .map_err(query_v13_stdout_failure)?
    } else if latest_query_schema {
        local_query_result_v12(
            &loaded.semantic,
            &manifest,
            loaded.head.snapshot_id.as_str(),
            &invocation.requested_id,
        )
        .map_err(query_v12_contract_failure)?
        .canonical_stdout()
        .map_err(query_v12_stdout_failure)?
    } else if query_v11 {
        local_query_result_v11(
            &loaded.semantic,
            &manifest,
            loaded.head.snapshot_id.as_str(),
            &invocation.requested_id,
        )
        .map_err(query_v11_contract_failure)?
        .canonical_stdout()
        .map_err(query_v11_stdout_failure)?
    } else if query_v10 {
        local_query_result_v10(
            &loaded.semantic,
            &manifest,
            loaded.head.snapshot_id.as_str(),
            &invocation.requested_id,
        )
        .map_err(query_v10_contract_failure)?
        .canonical_stdout()
        .map_err(query_v10_stdout_failure)?
    } else if query_v9 {
        local_query_result_v9(
            &loaded.semantic,
            &manifest,
            loaded.head.snapshot_id.as_str(),
            &invocation.requested_id,
        )
        .map_err(query_v9_contract_failure)?
        .canonical_stdout()
        .map_err(query_v9_stdout_failure)?
    } else if query_v8 {
        local_query_result_v8(
            &loaded.semantic,
            &manifest,
            loaded.head.snapshot_id.as_str(),
            &invocation.requested_id,
        )
        .map_err(query_v8_contract_failure)?
        .canonical_stdout()
        .map_err(query_v8_stdout_failure)?
    } else if query_v7 {
        local_query_result_v7(
            &loaded.semantic,
            &manifest,
            loaded.head.snapshot_id.as_str(),
            &invocation.requested_id,
        )
        .map_err(query_v7_contract_failure)?
        .canonical_stdout()
        .map_err(query_v7_stdout_failure)?
    } else if query_v6 {
        local_query_result_v6(
            &loaded.semantic,
            &manifest,
            loaded.head.snapshot_id.as_str(),
            &invocation.requested_id,
        )
        .map_err(query_contract_failure)?
        .canonical_stdout()
        .map_err(query_stdout_failure)?
    } else if query_v5 {
        local_query_result_v5(
            &loaded.semantic,
            &manifest,
            loaded.head.snapshot_id.as_str(),
            &invocation.requested_id,
        )
        .map_err(query_contract_failure)?
        .canonical_stdout()
        .map_err(query_stdout_failure)?
    } else if query_v4 {
        local_query_result_v4(
            &loaded.semantic,
            &manifest,
            loaded.head.snapshot_id.as_str(),
            &invocation.requested_id,
        )
        .map_err(query_contract_failure)?
        .canonical_stdout()
        .map_err(query_stdout_failure)?
    } else if query_v3 {
        local_query_result_v3(
            &loaded.semantic,
            &manifest,
            loaded.head.snapshot_id.as_str(),
            &invocation.requested_id,
        )
        .map_err(query_contract_failure)?
        .canonical_stdout()
        .map_err(query_stdout_failure)?
    } else if query_v2 {
        local_query_result_v2(
            &loaded.semantic,
            &manifest,
            loaded.head.snapshot_id.as_str(),
            &invocation.requested_id,
        )
        .map_err(query_contract_failure)?
        .canonical_stdout()
        .map_err(query_stdout_failure)?
    } else {
        local_query_result_v1(
            &loaded.semantic,
            &manifest,
            loaded.head.snapshot_id.as_str(),
            &invocation.requested_id,
        )
        .map_err(query_contract_failure)?
        .canonical_stdout()
        .map_err(query_stdout_failure)?
    };
    Ok(stdout)
}

fn function_context_contract_failure(error: &FunctionContextError) -> Failure {
    match error {
        FunctionContextError::NotFound => Failure::Query(QueryFailure::NotFound),
        FunctionContextError::InvalidSnapshot
        | FunctionContextError::InvalidRootKind
        | FunctionContextError::MissingSignature
        | FunctionContextError::DuplicateSignature
        | FunctionContextError::InvalidRelationship
        | FunctionContextError::InvalidParameterOrdinal
        | FunctionContextError::DanglingReference(_)
        | FunctionContextError::LimitExceeded { .. }
        | FunctionContextError::UnsafePayload(_)
        | FunctionContextError::Serialization => {
            r16_failure(CodeNoesisErrorV24::invalid_query(), 14)
        }
    }
}

fn query_v13_contract_failure(error: QueryContractError) -> Failure {
    match error {
        QueryContractError::NotFound => Failure::Query(QueryFailure::NotFound),
        QueryContractError::InvalidSnapshot
        | QueryContractError::InvalidDocuments
        | QueryContractError::LimitExceeded => r16_failure(CodeNoesisErrorV24::invalid_query(), 14),
    }
}

fn query_v13_stdout_failure(_error: QueryContractError) -> Failure {
    r16_failure(CodeNoesisErrorV24::invalid_query(), 14)
}

fn query_v12_contract_failure(error: QueryContractError) -> Failure {
    match error {
        QueryContractError::NotFound => Failure::Query(QueryFailure::NotFound),
        QueryContractError::InvalidSnapshot
        | QueryContractError::InvalidDocuments
        | QueryContractError::LimitExceeded => r15_failure(CodeNoesisErrorV22::invalid_query(), 14),
    }
}

fn query_v12_stdout_failure(_error: QueryContractError) -> Failure {
    r15_failure(CodeNoesisErrorV22::invalid_query(), 14)
}

fn query_v11_contract_failure(error: QueryContractError) -> Failure {
    match error {
        QueryContractError::NotFound => Failure::Query(QueryFailure::NotFound),
        QueryContractError::InvalidSnapshot
        | QueryContractError::InvalidDocuments
        | QueryContractError::LimitExceeded => r14_failure(CodeNoesisErrorV21::invalid_query(), 14),
    }
}

fn query_v11_stdout_failure(_error: QueryContractError) -> Failure {
    r14_failure(CodeNoesisErrorV21::invalid_query(), 14)
}

fn query_v10_contract_failure(error: QueryContractError) -> Failure {
    match error {
        QueryContractError::NotFound => Failure::Query(QueryFailure::NotFound),
        QueryContractError::InvalidSnapshot => {
            r13_failure(CodeNoesisErrorV20::invalid_query("snapshot_invalid"), 14)
        }
        QueryContractError::InvalidDocuments | QueryContractError::LimitExceeded => {
            r13_failure(CodeNoesisErrorV20::invalid_query("query_invalid"), 14)
        }
    }
}

fn query_v10_stdout_failure(_error: QueryContractError) -> Failure {
    r13_failure(CodeNoesisErrorV20::invalid_query("output_invalid"), 14)
}

fn query_contract_failure(error: QueryContractError) -> Failure {
    match error {
        QueryContractError::NotFound => Failure::Query(QueryFailure::NotFound),
        QueryContractError::InvalidDocuments => Failure::Query(QueryFailure::CorruptDocuments),
        QueryContractError::InvalidSnapshot => Failure::Query(QueryFailure::SnapshotMismatch),
        QueryContractError::LimitExceeded => Failure::Query(QueryFailure::LimitExceeded),
    }
}

fn query_v7_contract_failure(error: QueryContractError) -> Failure {
    match error {
        QueryContractError::NotFound => Failure::Query(QueryFailure::NotFound),
        QueryContractError::InvalidSnapshot => {
            r10_failure(CodeNoesisErrorV17::invalid_snapshot(), 14)
        }
        QueryContractError::InvalidDocuments | QueryContractError::LimitExceeded => {
            r10_failure(CodeNoesisErrorV17::invalid_query(), 14)
        }
    }
}

fn query_v8_contract_failure(error: QueryContractError) -> Failure {
    match error {
        QueryContractError::NotFound => Failure::Query(QueryFailure::NotFound),
        QueryContractError::InvalidSnapshot => {
            r11_failure(CodeNoesisErrorV18::invalid_snapshot(), 14)
        }
        QueryContractError::InvalidDocuments | QueryContractError::LimitExceeded => {
            r11_failure(CodeNoesisErrorV18::invalid_query(), 14)
        }
    }
}

fn query_v9_contract_failure(error: QueryContractError) -> Failure {
    match error {
        QueryContractError::NotFound => Failure::Query(QueryFailure::NotFound),
        QueryContractError::InvalidSnapshot => {
            r12_failure(CodeNoesisErrorV19::invalid_query("snapshot_invalid"), 14)
        }
        QueryContractError::InvalidDocuments | QueryContractError::LimitExceeded => {
            r12_failure(CodeNoesisErrorV19::invalid_query("query_invalid"), 14)
        }
    }
}

fn query_v9_stdout_failure(_error: QueryContractError) -> Failure {
    r12_failure(CodeNoesisErrorV19::invalid_query("output_invalid"), 14)
}

fn query_v8_stdout_failure(_error: QueryContractError) -> Failure {
    r11_failure(CodeNoesisErrorV18::invalid_query(), 14)
}

fn query_v7_stdout_failure(_error: QueryContractError) -> Failure {
    r10_failure(CodeNoesisErrorV17::invalid_query(), 14)
}

fn query_stdout_failure(error: QueryContractError) -> Failure {
    match error {
        QueryContractError::LimitExceeded => Failure::Query(QueryFailure::LimitExceeded),
        QueryContractError::InvalidSnapshot
        | QueryContractError::InvalidDocuments
        | QueryContractError::NotFound => Failure::Internal,
    }
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
        SNAPSHOT_SCHEMA_VERSION_V4
            | SNAPSHOT_SCHEMA_VERSION_V5
            | SNAPSHOT_SCHEMA_VERSION_V6
            | SNAPSHOT_SCHEMA_VERSION_V7
            | SNAPSHOT_SCHEMA_VERSION_V8
            | SNAPSHOT_SCHEMA_VERSION_V9
            | SNAPSHOT_SCHEMA_VERSION_V10
            | SNAPSHOT_SCHEMA_VERSION_V11
            | SNAPSHOT_SCHEMA_VERSION_V12
            | SNAPSHOT_SCHEMA_VERSION_V13
            | SNAPSHOT_SCHEMA_VERSION_V14
            | SNAPSHOT_SCHEMA_VERSION_V15
            | SNAPSHOT_SCHEMA_VERSION_V16
            | SNAPSHOT_SCHEMA_VERSION_V17
            | SNAPSHOT_SCHEMA_VERSION_V18
    ) {
        return Err(LoadS4Error::UnsupportedSnapshotSchema(
            head.snapshot_schema_version.clone(),
        ));
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
        SNAPSHOT_SCHEMA_VERSION_V7 => validate_stored_snapshot_semantic_v7(&semantic, &head),
        SNAPSHOT_SCHEMA_VERSION_V8 => validate_stored_snapshot_semantic_v8(&semantic, &head),
        SNAPSHOT_SCHEMA_VERSION_V9 => validate_stored_snapshot_semantic_v9(&semantic, &head),
        SNAPSHOT_SCHEMA_VERSION_V10 => validate_stored_snapshot_semantic_v10(&semantic, &head),
        SNAPSHOT_SCHEMA_VERSION_V11 => validate_stored_snapshot_semantic_v11(&semantic, &head),
        SNAPSHOT_SCHEMA_VERSION_V12 => validate_stored_snapshot_semantic_v12(&semantic, &head),
        SNAPSHOT_SCHEMA_VERSION_V13 => validate_stored_snapshot_semantic_v13(&semantic, &head),
        SNAPSHOT_SCHEMA_VERSION_V14 => validate_stored_snapshot_semantic_v14(&semantic, &head),
        SNAPSHOT_SCHEMA_VERSION_V15 => validate_stored_snapshot_semantic_v15(&semantic, &head),
        SNAPSHOT_SCHEMA_VERSION_V16 => validate_stored_snapshot_semantic_v16(&semantic, &head),
        SNAPSHOT_SCHEMA_VERSION_V17 => validate_stored_snapshot_semantic_v17(&semantic, &head),
        SNAPSHOT_SCHEMA_VERSION_V18 => validate_stored_snapshot_semantic_v18(&semantic, &head),
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
            LoadS4Error::SnapshotMismatch | LoadS4Error::UnsupportedSnapshotSchema(_) => {
                s5_failure(
                    CodeNoesisErrorV7::baseline_incompatible(SNAPSHOT_SCHEMA_VERSION_V4, "invalid"),
                    15,
                )
            }
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

fn serialize_v7(snapshot: &RepositorySnapshotV7) -> Result<Vec<u8>, Failure> {
    snapshot.canonical_stdout().map_err(|error| match error {
        RepositorySnapshotV7Error::LimitExceeded(error) => {
            Failure::Scan(ScanError::Acquisition(error))
        }
        RepositorySnapshotV7Error::Serialization(_)
        | RepositorySnapshotV7Error::ContractInvalid
        | RepositorySnapshotV7Error::OutputLengthOverflow => r4_internal_failure(),
    })
}

fn serialize_v8(snapshot: &RepositorySnapshotV8) -> Result<Vec<u8>, Failure> {
    snapshot.canonical_stdout().map_err(|error| match error {
        RepositorySnapshotV8Error::LimitExceeded(error) => {
            Failure::Scan(ScanError::Acquisition(error))
        }
        RepositorySnapshotV8Error::Serialization(_)
        | RepositorySnapshotV8Error::ContractInvalid
        | RepositorySnapshotV8Error::OutputLengthOverflow => r5_internal_failure(),
    })
}

fn serialize_v12(snapshot: &RepositorySnapshotV12) -> Result<Vec<u8>, Failure> {
    snapshot.canonical_stdout().map_err(|error| match error {
        RepositorySnapshotV12Error::LimitExceeded(error) => {
            Failure::Scan(ScanError::Acquisition(error))
        }
        RepositorySnapshotV12Error::Serialization(_)
        | RepositorySnapshotV12Error::ContractInvalid
        | RepositorySnapshotV12Error::OutputLengthOverflow => r10_internal_failure(),
    })
}

fn serialize_v9(snapshot: &RepositorySnapshotV9) -> Result<Vec<u8>, Failure> {
    snapshot.canonical_stdout().map_err(|error| match error {
        RepositorySnapshotV9Error::LimitExceeded(error) => {
            Failure::Scan(ScanError::Acquisition(error))
        }
        RepositorySnapshotV9Error::Serialization(_)
        | RepositorySnapshotV9Error::ContractInvalid
        | RepositorySnapshotV9Error::OutputLengthOverflow => r6_internal_failure(),
    })
}

fn serialize_v10(snapshot: &RepositorySnapshotV10) -> Result<Vec<u8>, Failure> {
    snapshot.canonical_stdout().map_err(|error| match error {
        RepositorySnapshotV10Error::LimitExceeded(error) => {
            Failure::Scan(ScanError::Acquisition(error))
        }
        RepositorySnapshotV10Error::Serialization(_)
        | RepositorySnapshotV10Error::ContractInvalid
        | RepositorySnapshotV10Error::OutputLengthOverflow => r7_internal_failure(),
    })
}

fn serialize_v11(
    snapshot: &RepositorySnapshotV11,
    output_capacity_profile: K1OutputCapacityProfile,
) -> Result<Vec<u8>, Failure> {
    snapshot
        .canonical_stdout_with_output_capacity(output_capacity_profile)
        .map_err(|error| match error {
            RepositorySnapshotV11Error::LimitExceeded(error) => {
                Failure::Scan(ScanError::Acquisition(error))
            }
            RepositorySnapshotV11Error::Serialization(_)
            | RepositorySnapshotV11Error::ContractInvalid
            | RepositorySnapshotV11Error::OutputLengthOverflow => k1_internal_failure(),
        })
}

fn serialize_v13(
    snapshot: &RepositorySnapshotV13,
    output_capacity_profile: K1OutputCapacityProfile,
) -> Result<Vec<u8>, Failure> {
    snapshot
        .canonical_stdout_with_output_capacity(output_capacity_profile)
        .map_err(|error| match error {
            RepositorySnapshotV13Error::LimitExceeded(error) => {
                CodeNoesisErrorV18::from_acquisition_limit(&error)
                    .map_or_else(r11_internal_failure, |error| r11_failure(error, 10))
            }
            RepositorySnapshotV13Error::Serialization(_)
            | RepositorySnapshotV13Error::ContractInvalid
            | RepositorySnapshotV13Error::OutputLengthOverflow => r11_internal_failure(),
        })
}

fn serialize_v14(
    snapshot: &RepositorySnapshotV14,
    output_capacity_profile: K1OutputCapacityProfile,
) -> Result<Vec<u8>, Failure> {
    snapshot
        .canonical_stdout_with_output_capacity(output_capacity_profile)
        .map_err(|error| match error {
            RepositorySnapshotV14Error::LimitExceeded(error) => {
                Failure::Scan(ScanError::Acquisition(error))
            }
            RepositorySnapshotV14Error::Serialization(_)
            | RepositorySnapshotV14Error::ContractInvalid
            | RepositorySnapshotV14Error::OutputLengthOverflow => r12_internal_failure(),
        })
}

fn serialize_v15(snapshot: &RepositorySnapshotV15) -> Result<Vec<u8>, Failure> {
    snapshot.canonical_stdout().map_err(|error| match error {
        RepositorySnapshotV15Error::LimitExceeded(error) => {
            CodeNoesisErrorV20::from_acquisition_limit(&error)
                .map_or_else(r13_internal_failure, |error| r13_failure(error, 10))
        }
        RepositorySnapshotV15Error::Serialization(_)
        | RepositorySnapshotV15Error::ContractInvalid
        | RepositorySnapshotV15Error::OutputLengthOverflow => r13_internal_failure(),
    })
}

fn serialize_v16(snapshot: &RepositorySnapshotV16) -> Result<Vec<u8>, Failure> {
    snapshot.canonical_stdout().map_err(|error| match error {
        RepositorySnapshotV16Error::LimitExceeded(error) => {
            r14_failure(CodeNoesisErrorV21::acquisition_limit(&error), 10)
        }
        RepositorySnapshotV16Error::Serialization(_)
        | RepositorySnapshotV16Error::ContractInvalid
        | RepositorySnapshotV16Error::OutputLengthOverflow => r14_internal_failure(),
    })
}

fn serialize_v17(snapshot: &RepositorySnapshotV17) -> Result<Vec<u8>, Failure> {
    snapshot.canonical_stdout().map_err(|error| match error {
        RepositorySnapshotV17Error::LimitExceeded(error) => {
            r15_failure(CodeNoesisErrorV22::acquisition_limit(&error), 10)
        }
        RepositorySnapshotV17Error::Serialization(_)
        | RepositorySnapshotV17Error::ContractInvalid
        | RepositorySnapshotV17Error::OutputLengthOverflow => r15_internal_failure(),
    })
}

fn serialize_v18(snapshot: &RepositorySnapshotV18) -> Result<Vec<u8>, Failure> {
    snapshot.canonical_stdout().map_err(|error| match error {
        RepositorySnapshotV18Error::LimitExceeded(error) => {
            r16_failure(CodeNoesisErrorV24::acquisition_limit(&error), 10)
        }
        RepositorySnapshotV18Error::Serialization(_)
        | RepositorySnapshotV18Error::ContractInvalid
        | RepositorySnapshotV18Error::OutputLengthOverflow => r16_internal_failure(),
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
        InvocationError::InvalidManifestProfile => {
            r4_failure(CodeNoesisErrorV11::invalid_manifest_profile(), 2)
        }
        InvocationError::InvalidRustSemanticProfile(provided_profile) => r5_failure(
            CodeNoesisErrorV12::invalid_rust_semantic_profile(&provided_profile),
            2,
        ),
        InvocationError::InvalidRustSemanticComposition(reason) => {
            r5_failure(CodeNoesisErrorV12::unsupported_composition(reason), 11)
        }
        InvocationError::InvalidRustCfgAlternativesProfile(profile) => {
            r10_failure(CodeNoesisErrorV17::invalid_profile(&profile), 2)
        }
        InvocationError::InvalidRustCfgAlternativesComposition(reason) => {
            r10_failure(CodeNoesisErrorV17::unsupported_composition(reason), 2)
        }
        InvocationError::InvalidR12Profile(field, profile) => {
            r12_failure(CodeNoesisErrorV19::invalid_profile(field, &profile), 2)
        }
        InvocationError::InvalidR12Composition(reason) => {
            r12_failure(CodeNoesisErrorV19::unsupported_composition(reason), 2)
        }
        InvocationError::InvalidR13Profile(field, profile) => {
            r13_failure(CodeNoesisErrorV20::invalid_profile(field, &profile), 2)
        }
        InvocationError::InvalidR13Composition(reason) => {
            r13_failure(CodeNoesisErrorV20::unsupported_composition(reason), 2)
        }
        InvocationError::InvalidR14Profile(profile) => {
            r14_failure(CodeNoesisErrorV21::invalid_profile(&profile), 2)
        }
        InvocationError::InvalidR14Composition(reason) => {
            r14_failure(CodeNoesisErrorV21::unsupported_composition(reason), 2)
        }
        InvocationError::InvalidR15Profile(profile) => {
            r15_failure(CodeNoesisErrorV22::invalid_profile(&profile), 2)
        }
        InvocationError::InvalidR15Composition(reason) => {
            r15_failure(CodeNoesisErrorV22::unsupported_composition(reason), 2)
        }
        InvocationError::InvalidR16Profile(profile) => {
            r16_failure(CodeNoesisErrorV24::invalid_profile(&profile), 2)
        }
        InvocationError::InvalidR16Composition(reason) => {
            r16_failure(CodeNoesisErrorV24::unsupported_composition(reason), 2)
        }
        InvocationError::InvalidRustFrameworkProfile(profile) => r6_failure(
            CodeNoesisErrorV13::invalid_rust_framework_profile(&profile),
            2,
        ),
        InvocationError::InvalidRustFrameworkComposition(selected_profiles) => r6_failure(
            CodeNoesisErrorV13::unsupported_composition(&selected_profiles),
            11,
        ),
        InvocationError::InvalidRustCallableProfile(profile) => k1_failure(
            CodeNoesisErrorV16::invalid_rust_callable_profile(&profile),
            2,
        ),
        InvocationError::InvalidRustCallableComposition => {
            k1_failure(CodeNoesisErrorV16::unsupported_composition(), 11)
        }
        InvocationError::InvalidCompilerIndexProfile(profile) => r7_failure(
            CodeNoesisErrorV14::invalid_compiler_index_profile(&profile),
            10,
        ),
        InvocationError::InvalidCompilerIndexComposition(selected_profiles) => r7_failure(
            CodeNoesisErrorV14::unsupported_composition(&selected_profiles),
            10,
        ),
        InvocationError::InvalidBoundaryProfile => {
            boundary_failure(CodeNoesisErrorV9::invalid_profile(), 2)
        }
        InvocationError::InvalidBoundaryManifest(reason) => {
            boundary_failure(CodeNoesisErrorV9::invalid_manifest(reason), 2)
        }
    }
}

fn r16_invocation_failure(error: InvocationError) -> Failure {
    match error {
        InvocationError::Input(error) => Failure::Input(error),
        InvocationError::InvalidR16Profile(profile) => {
            r16_failure(CodeNoesisErrorV24::invalid_profile(&profile), 2)
        }
        InvocationError::InvalidR16Composition(reason) => {
            r16_failure(CodeNoesisErrorV24::unsupported_composition(reason), 2)
        }
        _ => r16_failure(
            CodeNoesisErrorV24::unsupported_composition("exact_selector_matrix_required"),
            2,
        ),
    }
}

fn r15_invocation_failure(error: InvocationError) -> Failure {
    match error {
        InvocationError::Input(error) => Failure::Input(error),
        InvocationError::InvalidR15Profile(profile) => {
            r15_failure(CodeNoesisErrorV22::invalid_profile(&profile), 2)
        }
        InvocationError::InvalidR15Composition(reason) => {
            r15_failure(CodeNoesisErrorV22::unsupported_composition(reason), 2)
        }
        InvocationError::InvalidR16Profile(profile) => {
            r16_failure(CodeNoesisErrorV24::invalid_profile(&profile), 2)
        }
        InvocationError::InvalidR16Composition(reason) => {
            r16_failure(CodeNoesisErrorV24::unsupported_composition(reason), 2)
        }
        _ => r15_failure(
            CodeNoesisErrorV22::unsupported_composition("exact_selector_matrix_required"),
            2,
        ),
    }
}

fn r14_invocation_failure(error: InvocationError) -> Failure {
    match error {
        InvocationError::Input(error) => Failure::Input(error),
        InvocationError::InvalidR14Profile(profile) => {
            r14_failure(CodeNoesisErrorV21::invalid_profile(&profile), 2)
        }
        InvocationError::InvalidR14Composition(reason) => {
            r14_failure(CodeNoesisErrorV21::unsupported_composition(reason), 2)
        }
        _ => r14_failure(
            CodeNoesisErrorV21::unsupported_composition("exact_selector_matrix_required"),
            2,
        ),
    }
}

fn r13_invocation_failure(error: InvocationError) -> Failure {
    match error {
        InvocationError::Input(error) => Failure::Input(error),
        InvocationError::InvalidR13Profile(field, profile) => {
            r13_failure(CodeNoesisErrorV20::invalid_profile(field, &profile), 2)
        }
        InvocationError::InvalidR13Composition(reason) => {
            r13_failure(CodeNoesisErrorV20::unsupported_composition(reason), 2)
        }
        _ => r13_failure(
            CodeNoesisErrorV20::unsupported_composition("exact_selector_matrix_required"),
            2,
        ),
    }
}

fn r11_invocation_failure(error: InvocationError) -> Failure {
    match error {
        InvocationError::InvalidRustCallableProfile(profile) => r11_failure(
            CodeNoesisErrorV18::invalid_rust_callable_profile(&profile),
            2,
        ),
        InvocationError::InvalidBoundaryProfile => r11_failure(
            CodeNoesisErrorV18::from_boundary_error(&CodeNoesisErrorV9::invalid_profile()),
            2,
        ),
        InvocationError::InvalidBoundaryManifest(reason) => r11_failure(
            CodeNoesisErrorV18::from_boundary_error(&CodeNoesisErrorV9::invalid_manifest(reason)),
            2,
        ),
        InvocationError::InvalidWorkspaceProfile
        | InvocationError::InvalidManifestProfile
        | InvocationError::InvalidRustSemanticProfile(_)
        | InvocationError::InvalidRustSemanticComposition(_)
        | InvocationError::InvalidRustCfgAlternativesProfile(_)
        | InvocationError::InvalidRustCfgAlternativesComposition(_)
        | InvocationError::InvalidR12Profile(_, _)
        | InvocationError::InvalidR12Composition(_)
        | InvocationError::InvalidRustFrameworkProfile(_)
        | InvocationError::InvalidRustFrameworkComposition(_)
        | InvocationError::InvalidRustCallableComposition
        | InvocationError::InvalidCompilerIndexProfile(_)
        | InvocationError::InvalidCompilerIndexComposition(_) => {
            r11_failure(CodeNoesisErrorV18::unsupported_composition(), 11)
        }
        other => invocation_failure(other),
    }
}

fn r12_invocation_failure(error: InvocationError) -> Failure {
    match error {
        InvocationError::InvalidR12Profile(field, profile) => {
            r12_failure(CodeNoesisErrorV19::invalid_profile(field, &profile), 2)
        }
        InvocationError::InvalidBoundaryProfile => r12_failure(
            CodeNoesisErrorV19::from_boundary_error(&CodeNoesisErrorV9::invalid_profile()),
            2,
        ),
        InvocationError::InvalidBoundaryManifest(reason) => r12_failure(
            CodeNoesisErrorV19::from_boundary_error(&CodeNoesisErrorV9::invalid_manifest(reason)),
            2,
        ),
        InvocationError::Input(error) => Failure::Input(error),
        InvocationError::InvalidR12Composition(reason) => {
            r12_failure(CodeNoesisErrorV19::unsupported_composition(reason), 2)
        }
        _ => r12_failure(
            CodeNoesisErrorV19::unsupported_composition("exact_selector_matrix_required"),
            2,
        ),
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
        InvocationError::InvalidManifestProfile => {
            r4_failure(CodeNoesisErrorV11::invalid_manifest_profile(), 2)
        }
        InvocationError::InvalidRustSemanticProfile(provided_profile) => r5_failure(
            CodeNoesisErrorV12::invalid_rust_semantic_profile(&provided_profile),
            2,
        ),
        InvocationError::InvalidRustSemanticComposition(reason) => {
            r5_failure(CodeNoesisErrorV12::unsupported_composition(reason), 11)
        }
        InvocationError::InvalidRustCfgAlternativesProfile(profile) => {
            r10_failure(CodeNoesisErrorV17::invalid_profile(&profile), 2)
        }
        InvocationError::InvalidRustCfgAlternativesComposition(reason) => {
            r10_failure(CodeNoesisErrorV17::unsupported_composition(reason), 2)
        }
        InvocationError::InvalidR12Profile(field, profile) => {
            r12_failure(CodeNoesisErrorV19::invalid_profile(field, &profile), 2)
        }
        InvocationError::InvalidR12Composition(reason) => {
            r12_failure(CodeNoesisErrorV19::unsupported_composition(reason), 2)
        }
        InvocationError::InvalidR13Profile(field, profile) => {
            r13_failure(CodeNoesisErrorV20::invalid_profile(field, &profile), 2)
        }
        InvocationError::InvalidR13Composition(reason) => {
            r13_failure(CodeNoesisErrorV20::unsupported_composition(reason), 2)
        }
        InvocationError::InvalidR14Profile(profile) => {
            r14_failure(CodeNoesisErrorV21::invalid_profile(&profile), 2)
        }
        InvocationError::InvalidR14Composition(reason) => {
            r14_failure(CodeNoesisErrorV21::unsupported_composition(reason), 2)
        }
        InvocationError::InvalidR15Profile(profile) => {
            r15_failure(CodeNoesisErrorV22::invalid_profile(&profile), 2)
        }
        InvocationError::InvalidR15Composition(reason) => {
            r15_failure(CodeNoesisErrorV22::unsupported_composition(reason), 2)
        }
        InvocationError::InvalidR16Profile(profile) => {
            r16_failure(CodeNoesisErrorV24::invalid_profile(&profile), 2)
        }
        InvocationError::InvalidR16Composition(reason) => {
            r16_failure(CodeNoesisErrorV24::unsupported_composition(reason), 2)
        }
        InvocationError::InvalidRustFrameworkProfile(profile) => r6_failure(
            CodeNoesisErrorV13::invalid_rust_framework_profile(&profile),
            2,
        ),
        InvocationError::InvalidRustFrameworkComposition(selected_profiles) => r6_failure(
            CodeNoesisErrorV13::unsupported_composition(&selected_profiles),
            11,
        ),
        InvocationError::InvalidRustCallableProfile(profile) => k1_failure(
            CodeNoesisErrorV16::invalid_rust_callable_profile(&profile),
            2,
        ),
        InvocationError::InvalidRustCallableComposition => {
            k1_failure(CodeNoesisErrorV16::unsupported_composition(), 11)
        }
        InvocationError::InvalidCompilerIndexProfile(profile) => r7_failure(
            CodeNoesisErrorV14::invalid_compiler_index_profile(&profile),
            10,
        ),
        InvocationError::InvalidCompilerIndexComposition(selected_profiles) => r7_failure(
            CodeNoesisErrorV14::unsupported_composition(&selected_profiles),
            10,
        ),
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

fn repository_boundary_input_failure_r11(
    failure: &repository_boundaries::RepositoryBoundaryFailure,
) -> Failure {
    r11_failure(
        CodeNoesisErrorV18::from_boundary_error(&failure.error),
        failure.exit_code,
    )
}

fn repository_boundary_input_failure_r12(
    failure: &repository_boundaries::RepositoryBoundaryFailure,
) -> Failure {
    r12_failure(
        CodeNoesisErrorV19::from_boundary_error(&failure.error),
        failure.exit_code,
    )
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

fn r11_boundary_scan_failure(error: BoundaryScanError) -> Failure {
    match boundary_scan_failure(error) {
        Failure::V9(failure) => r11_failure(
            CodeNoesisErrorV18::from_boundary_error(&failure.error),
            failure.exit_code,
        ),
        other => r11_upgrade_limit_failure(other),
    }
}

fn r12_boundary_scan_failure(error: BoundaryScanError) -> Failure {
    match boundary_scan_failure(error) {
        Failure::V9(failure) => r12_failure(
            CodeNoesisErrorV19::from_boundary_error(&failure.error),
            failure.exit_code,
        ),
        other => r12_upgrade_limit_failure(other),
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

fn r4_scan_failure(error: CargoManifestScanError) -> Failure {
    match error {
        CargoManifestScanError::Scan(ScanError::Internal) => r4_internal_failure(),
        CargoManifestScanError::Scan(error) => Failure::Scan(error),
        CargoManifestScanError::Manifest(CargoManifestFactError::Source(
            RootPackageWorkspaceError::Source(error),
        )) => Failure::Scan(ScanError::Workspace(error)),
        CargoManifestScanError::Manifest(CargoManifestFactError::Source(error)) => {
            CodeNoesisErrorV10::from_workspace(&error)
                .map_or_else(r3_internal_failure, |error| r3_failure(error, 11))
        }
        CargoManifestScanError::Manifest(error) => CodeNoesisErrorV11::from_manifest(&error)
            .map_or_else(r4_internal_failure, |error| r4_failure(error, 11)),
        CargoManifestScanError::Boundary(error) => boundary_scan_failure(error),
    }
}

fn r5_scan_failure(error: RustSemanticScanError) -> Failure {
    match error {
        RustSemanticScanError::Scan(ScanError::Internal) => r5_internal_failure(),
        RustSemanticScanError::Scan(error) => Failure::Scan(error),
        RustSemanticScanError::Semantic(RustSemanticError::Source(
            CargoManifestFactError::Source(RootPackageWorkspaceError::Source(error)),
        )) => Failure::Scan(ScanError::Workspace(error)),
        RustSemanticScanError::Semantic(RustSemanticError::Source(
            CargoManifestFactError::Source(error),
        )) => CodeNoesisErrorV10::from_workspace(&error)
            .map_or_else(r3_internal_failure, |error| r3_failure(error, 11)),
        RustSemanticScanError::Semantic(RustSemanticError::Source(error)) => {
            CodeNoesisErrorV11::from_manifest(&error)
                .map_or_else(r4_internal_failure, |error| r4_failure(error, 11))
        }
        RustSemanticScanError::Semantic(error) => CodeNoesisErrorV12::from_semantic(&error)
            .map_or_else(r5_internal_failure, |error| r5_failure(error, 11)),
        RustSemanticScanError::Boundary(error) => boundary_scan_failure(error),
    }
}

fn r10_scan_failure(error: RustCfgAlternativesScanError) -> Failure {
    match error {
        RustCfgAlternativesScanError::Scan(ScanError::Internal) => r10_internal_failure(),
        RustCfgAlternativesScanError::Scan(error) => Failure::Scan(error),
        RustCfgAlternativesScanError::Alternatives(error) => {
            r10_failure(CodeNoesisErrorV17::from_extraction(&error), 11)
        }
        RustCfgAlternativesScanError::InvalidSnapshot => {
            r10_failure(CodeNoesisErrorV17::invalid_snapshot(), 12)
        }
        RustCfgAlternativesScanError::Boundary(error) => boundary_scan_failure(error),
    }
}

fn r6_scan_failure(error: FrameworkScanError) -> Failure {
    match error {
        FrameworkScanError::Scan(ScanError::Internal) => r6_internal_failure(),
        FrameworkScanError::Scan(error) => Failure::Scan(error),
        FrameworkScanError::Framework(FrameworkError::Source(error)) => {
            r5_scan_failure(RustSemanticScanError::Semantic(error))
        }
        FrameworkScanError::Framework(error) => CodeNoesisErrorV13::from_framework(&error)
            .map_or_else(r6_internal_failure, |error| r6_failure(error, 11)),
        FrameworkScanError::Boundary(error) => boundary_scan_failure(error),
    }
}

fn r7_scan_failure(error: CompilerIndexScanError) -> Failure {
    match error {
        CompilerIndexScanError::Scan(ScanError::Internal) => r7_internal_failure(),
        CompilerIndexScanError::Scan(error) => Failure::Scan(error),
        CompilerIndexScanError::Framework(FrameworkError::Source(error)) => {
            r5_scan_failure(RustSemanticScanError::Semantic(error))
        }
        CompilerIndexScanError::Framework(error) => CodeNoesisErrorV13::from_framework(&error)
            .map_or_else(r6_internal_failure, |error| r6_failure(error, 11)),
        CompilerIndexScanError::CompilerIndex(error) => r7_compiler_index_failure(&error),
        CompilerIndexScanError::Boundary(error) => boundary_scan_failure(error),
    }
}

fn k1_scan_failure(error: CallableSemanticsScanError) -> Failure {
    match error {
        CallableSemanticsScanError::Scan(ScanError::Internal) => k1_internal_failure(),
        CallableSemanticsScanError::Scan(error) => Failure::Scan(error),
        CallableSemanticsScanError::Callable(CallableSemanticsError::Source(error)) => {
            r6_scan_failure(FrameworkScanError::Framework(error))
        }
        CallableSemanticsScanError::Callable(error) => CodeNoesisErrorV16::from_callable(&error)
            .map_or_else(k1_internal_failure, |error| k1_failure(error, 11)),
    }
}

fn r11_scan_failure(error: CallableBoundaryCompositionScanError) -> Failure {
    match error {
        CallableBoundaryCompositionScanError::Scan(ScanError::Internal) => r11_internal_failure(),
        CallableBoundaryCompositionScanError::Scan(ScanError::Acquisition(error)) => {
            CodeNoesisErrorV18::from_acquisition_limit(&error).map_or_else(
                || Failure::Scan(ScanError::Acquisition(error)),
                |error| r11_failure(error, 10),
            )
        }
        CallableBoundaryCompositionScanError::Scan(error) => Failure::Scan(error),
        CallableBoundaryCompositionScanError::Callable(CallableSemanticsError::Source(
            FrameworkError::Source(error),
        )) => CodeNoesisErrorV18::from_rust_semantic_identity_conflict(&error).map_or_else(
            || r5_scan_failure(RustSemanticScanError::Semantic(error)),
            |error| r11_failure(error, 11),
        ),
        CallableBoundaryCompositionScanError::Callable(CallableSemanticsError::Source(error)) => {
            r6_scan_failure(FrameworkScanError::Framework(error))
        }
        CallableBoundaryCompositionScanError::Callable(error) => {
            CodeNoesisErrorV18::from_callable(&error)
                .map_or_else(r11_internal_failure, |error| r11_failure(error, 11))
        }
        CallableBoundaryCompositionScanError::InvalidSnapshot => {
            r11_failure(CodeNoesisErrorV18::invalid_snapshot(), 12)
        }
        CallableBoundaryCompositionScanError::Boundary(error) => r11_boundary_scan_failure(error),
    }
}

fn r12_scan_failure(error: CallableCfgAlternativesScanError) -> Failure {
    match error {
        CallableCfgAlternativesScanError::Scan(ScanError::Internal) => r12_internal_failure(),
        CallableCfgAlternativesScanError::Scan(ScanError::Acquisition(error)) => {
            CodeNoesisErrorV19::from_acquisition_limit(&error).map_or_else(
                || Failure::Scan(ScanError::Acquisition(error)),
                |error| r12_failure(error, 10),
            )
        }
        CallableCfgAlternativesScanError::Scan(error) => Failure::Scan(error),
        CallableCfgAlternativesScanError::Composition(error) => {
            r12_failure(CodeNoesisErrorV19::from_extraction(&error), 11)
        }
        CallableCfgAlternativesScanError::InvalidSnapshot => {
            r12_failure(CodeNoesisErrorV19::invalid_snapshot(), 12)
        }
        CallableCfgAlternativesScanError::Boundary(error) => r12_boundary_scan_failure(error),
    }
}

fn r13_scan_failure(error: CallableScipScanError) -> Failure {
    match error {
        CallableScipScanError::Scan(ScanError::Internal) => r13_internal_failure(),
        CallableScipScanError::Scan(ScanError::Acquisition(error)) => {
            CodeNoesisErrorV20::from_acquisition_limit(&error).map_or_else(
                || Failure::Scan(ScanError::Acquisition(error)),
                |error| r13_failure(error, 10),
            )
        }
        CallableScipScanError::Scan(error) => Failure::Scan(error),
        CallableScipScanError::Callable(error) => {
            r13_failure(CodeNoesisErrorV20::from_callable(&error), 11)
        }
        CallableScipScanError::CompilerIndex(error) => {
            r13_failure(CodeNoesisErrorV20::from_compiler_index(&error), 10)
        }
        CallableScipScanError::Composition(error) => {
            r13_failure(CodeNoesisErrorV20::from_composition(&error), 11)
        }
        CallableScipScanError::InvalidSnapshot => {
            r13_failure(CodeNoesisErrorV20::invalid_snapshot(), 12)
        }
    }
}

fn r14_scan_failure(error: ExpressionBindingsScanError) -> Failure {
    match error {
        ExpressionBindingsScanError::Scan(ScanError::Internal) => r14_internal_failure(),
        ExpressionBindingsScanError::Scan(ScanError::Acquisition(error))
            if is_gitlink_acquisition(&error) =>
        {
            r14_failure(
                CodeNoesisErrorV21::unsupported_composition("repository_boundary_not_supported"),
                2,
            )
        }
        ExpressionBindingsScanError::Scan(ScanError::Acquisition(error)) => {
            r14_failure(CodeNoesisErrorV21::acquisition_limit(&error), 10)
        }
        ExpressionBindingsScanError::Scan(error) => Failure::Scan(error),
        ExpressionBindingsScanError::Expression(error) => {
            r14_failure(CodeNoesisErrorV21::from_expression(&error), 11)
        }
        ExpressionBindingsScanError::InvalidSnapshot => {
            r14_failure(CodeNoesisErrorV21::invalid_snapshot(), 12)
        }
    }
}

fn r15_scan_failure(error: LocalFlowScanError) -> Failure {
    match error {
        LocalFlowScanError::Scan(ScanError::Internal) => r15_internal_failure(),
        LocalFlowScanError::Scan(ScanError::Acquisition(error))
            if is_gitlink_acquisition(&error) =>
        {
            r15_failure(
                CodeNoesisErrorV22::unsupported_composition("repository_boundary_not_supported"),
                2,
            )
        }
        LocalFlowScanError::Scan(ScanError::Acquisition(error)) => {
            r15_failure(CodeNoesisErrorV22::acquisition_limit(&error), 10)
        }
        LocalFlowScanError::Scan(error) => Failure::Scan(error),
        LocalFlowScanError::LocalFlow(error) => {
            r15_failure(CodeNoesisErrorV22::from_local_flow(&error), 11)
        }
        LocalFlowScanError::InvalidSnapshot => {
            r15_failure(CodeNoesisErrorV22::invalid_snapshot(), 12)
        }
    }
}

fn r16_scan_failure(error: ConstantEvaluationScanError) -> Failure {
    match error {
        ConstantEvaluationScanError::Scan(ScanError::Internal) => r16_internal_failure(),
        ConstantEvaluationScanError::Scan(ScanError::Acquisition(error))
            if is_gitlink_acquisition(&error) =>
        {
            r16_failure(
                CodeNoesisErrorV24::unsupported_composition("repository_boundary_not_supported"),
                2,
            )
        }
        ConstantEvaluationScanError::Scan(ScanError::Acquisition(error)) => {
            r16_failure(CodeNoesisErrorV24::acquisition_limit(&error), 10)
        }
        ConstantEvaluationScanError::Scan(error) => Failure::Scan(error),
        ConstantEvaluationScanError::ConstantEvaluation(error) => {
            r16_failure(CodeNoesisErrorV24::from_constant_evaluation(&error), 11)
        }
        ConstantEvaluationScanError::InvalidSnapshot => {
            r16_failure(CodeNoesisErrorV24::invalid_snapshot(), 12)
        }
    }
}

fn r16_upgrade_limit_failure(failure: Failure) -> Failure {
    match failure {
        Failure::Scan(ScanError::Acquisition(error)) if is_gitlink_acquisition(&error) => {
            r16_failure(
                CodeNoesisErrorV24::unsupported_composition("repository_boundary_not_supported"),
                2,
            )
        }
        Failure::Scan(ScanError::Acquisition(error)) => {
            r16_failure(CodeNoesisErrorV24::acquisition_limit(&error), 10)
        }
        other => other,
    }
}

fn r15_upgrade_limit_failure(failure: Failure) -> Failure {
    match failure {
        Failure::Scan(ScanError::Acquisition(error)) if is_gitlink_acquisition(&error) => {
            r15_failure(
                CodeNoesisErrorV22::unsupported_composition("repository_boundary_not_supported"),
                2,
            )
        }
        Failure::Scan(ScanError::Acquisition(error)) => {
            r15_failure(CodeNoesisErrorV22::acquisition_limit(&error), 10)
        }
        other => other,
    }
}

fn r14_upgrade_limit_failure(failure: Failure) -> Failure {
    match failure {
        Failure::Scan(ScanError::Acquisition(error)) if is_gitlink_acquisition(&error) => {
            r14_failure(
                CodeNoesisErrorV21::unsupported_composition("repository_boundary_not_supported"),
                2,
            )
        }
        Failure::Scan(ScanError::Acquisition(error)) => {
            r14_failure(CodeNoesisErrorV21::acquisition_limit(&error), 10)
        }
        other => other,
    }
}

fn is_gitlink_acquisition(error: &AcquisitionError) -> bool {
    matches!(
        error,
        AcquisitionError::UnsupportedRepositoryShape {
            feature: UnsupportedFeature::SubmoduleOrGitlink,
        } | AcquisitionError::EntryPolicyViolation {
            entry: EntryPolicy::Gitlink,
            ..
        }
    )
}

fn r13_upgrade_limit_failure(failure: Failure) -> Failure {
    match failure {
        Failure::Scan(ScanError::Acquisition(error)) => {
            CodeNoesisErrorV20::from_acquisition_limit(&error).map_or_else(
                || Failure::Scan(ScanError::Acquisition(error)),
                |error| r13_failure(error, 10),
            )
        }
        other => other,
    }
}

fn r12_upgrade_limit_failure(failure: Failure) -> Failure {
    match failure {
        Failure::Scan(ScanError::Acquisition(error)) => {
            CodeNoesisErrorV19::from_acquisition_limit(&error).map_or_else(
                || Failure::Scan(ScanError::Acquisition(error)),
                |error| r12_failure(error, 10),
            )
        }
        other => other,
    }
}

fn r11_upgrade_limit_failure(failure: Failure) -> Failure {
    match failure {
        Failure::Scan(ScanError::Acquisition(error)) => {
            CodeNoesisErrorV18::from_acquisition_limit(&error).map_or_else(
                || Failure::Scan(ScanError::Acquisition(error)),
                |error| r11_failure(error, 10),
            )
        }
        other => other,
    }
}

fn r7_compiler_index_failure(error: &CompilerIndexError) -> Failure {
    r7_failure(CodeNoesisErrorV14::from_compiler_index(error), 10)
}

fn r13_compiler_index_failure(error: &CompilerIndexError) -> Failure {
    r13_failure(CodeNoesisErrorV20::from_compiler_index(error), 10)
}

fn k1_failure(error: CodeNoesisErrorV16, exit_code: u8) -> Failure {
    Failure::K1(K1Failure { error, exit_code })
}

fn r12_failure(error: CodeNoesisErrorV19, exit_code: u8) -> Failure {
    Failure::R12(R12Failure { error, exit_code })
}

fn r13_failure(error: CodeNoesisErrorV20, exit_code: u8) -> Failure {
    Failure::R13(R13Failure { error, exit_code })
}

fn r14_failure(error: CodeNoesisErrorV21, exit_code: u8) -> Failure {
    Failure::R14(R14Failure { error, exit_code })
}

fn r15_failure(error: CodeNoesisErrorV22, exit_code: u8) -> Failure {
    Failure::R15(R15Failure { error, exit_code })
}

fn r16_failure(error: CodeNoesisErrorV24, exit_code: u8) -> Failure {
    Failure::R16(R16Failure { error, exit_code })
}

fn r18_failure(error: CodeNoesisErrorV29, exit_code: u8) -> Failure {
    Failure::R18(R18Failure { error, exit_code })
}

fn r18_internal_failure() -> Failure {
    r18_failure(CodeNoesisErrorV29::internal(), 1)
}

fn r16_internal_failure() -> Failure {
    r16_failure(CodeNoesisErrorV24::internal("runtime"), 70)
}

fn r15_internal_failure() -> Failure {
    r15_failure(CodeNoesisErrorV22::internal("runtime"), 70)
}

fn r14_internal_failure() -> Failure {
    r14_failure(CodeNoesisErrorV21::internal("runtime"), 70)
}

fn r13_internal_failure() -> Failure {
    r13_failure(CodeNoesisErrorV20::internal("runtime"), 70)
}

fn r12_internal_failure() -> Failure {
    r12_failure(CodeNoesisErrorV19::internal("runtime"), 70)
}

fn r11_failure(error: CodeNoesisErrorV18, exit_code: u8) -> Failure {
    Failure::R11(R11Failure { error, exit_code })
}

fn r11_internal_failure() -> Failure {
    r11_failure(CodeNoesisErrorV18::internal(), 70)
}

fn k1_internal_failure() -> Failure {
    k1_failure(CodeNoesisErrorV16::internal(), 70)
}

fn r8_snapshot_load_failure(error: LoadS4Error) -> Failure {
    match error {
        LoadS4Error::Scan(ScanError::Storage(StorageError::CorruptMetadata {
            reason: "current_head_missing",
            ..
        })) => r8_contract_failure(
            R8ContractError::InvalidSnapshot {
                snapshot_id: zero_snapshot_id(),
                reason: "missing_visible_head",
            },
            16,
        ),
        LoadS4Error::UnsupportedSnapshotSchema(observed) => {
            r8_contract_failure(R8ContractError::UnsupportedSnapshotSchema(observed), 16)
        }
        LoadS4Error::Scan(_) | LoadS4Error::SnapshotMismatch => r8_contract_failure(
            R8ContractError::InvalidSnapshot {
                snapshot_id: zero_snapshot_id(),
                reason: "corrupt",
            },
            16,
        ),
    }
}

fn k1_snapshot_load_failure(error: LoadS4Error) -> Failure {
    match error {
        LoadS4Error::UnsupportedSnapshotSchema(observed) => k1_contract_failure(
            &K1ContractError::UnsupportedSnapshotSchema(observed),
            16,
            false,
        ),
        LoadS4Error::Scan(_) | LoadS4Error::SnapshotMismatch => {
            k1_contract_failure(&K1ContractError::InvalidSnapshot, 16, false)
        }
    }
}

fn r10_snapshot_load_failure(error: LoadS4Error) -> Failure {
    match error {
        LoadS4Error::UnsupportedSnapshotSchema(observed) => r10_contract_failure(
            &R10ContractError::UnsupportedSnapshotSchema(observed),
            16,
            false,
        ),
        LoadS4Error::Scan(_) | LoadS4Error::SnapshotMismatch => {
            r10_contract_failure(&R10ContractError::InvalidSnapshot, 16, false)
        }
    }
}

fn r14_snapshot_load_failure(error: LoadS4Error) -> Failure {
    match error {
        LoadS4Error::UnsupportedSnapshotSchema(observed) => r14_contract_failure(
            &R14ContractError::UnsupportedSnapshotSchema(observed),
            16,
            false,
        ),
        LoadS4Error::Scan(_) | LoadS4Error::SnapshotMismatch => {
            r14_contract_failure(&R14ContractError::InvalidSnapshot, 16, false)
        }
    }
}

fn r15_snapshot_load_failure(error: LoadS4Error) -> Failure {
    match error {
        LoadS4Error::UnsupportedSnapshotSchema(observed) => r15_contract_failure(
            &R15ContractError::UnsupportedSnapshotSchema(observed),
            16,
            false,
        ),
        LoadS4Error::Scan(_) | LoadS4Error::SnapshotMismatch => {
            r15_contract_failure(&R15ContractError::InvalidSnapshot, 16, false)
        }
    }
}

fn r16_snapshot_load_failure(error: &LoadS4Error) -> Failure {
    match error {
        LoadS4Error::UnsupportedSnapshotSchema(_)
        | LoadS4Error::Scan(_)
        | LoadS4Error::SnapshotMismatch => {
            r16_contract_failure(&R16ContractError::InvalidSnapshot, 16, false)
        }
    }
}

fn k1_portable_failure(
    error: noesis::portable_explorer::PortableExplorerError,
    code: u8,
    explorer: bool,
) -> Failure {
    match error {
        noesis::portable_explorer::PortableExplorerError::UnsafeOutput {
            path_sha256,
            reason,
        } => k1_failure(
            CodeNoesisErrorV16::unsafe_output_path(&path_sha256, reason),
            2,
        ),
        noesis::portable_explorer::PortableExplorerError::K1Contract(error) => {
            k1_contract_failure(&error, code, explorer)
        }
        noesis::portable_explorer::PortableExplorerError::Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R10Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R11Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R12Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R13Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R14Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R15Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R16Contract(_)
        | noesis::portable_explorer::PortableExplorerError::Internal => k1_internal_failure(),
    }
}

fn k1_contract_failure(error: &K1ContractError, code: u8, explorer: bool) -> Failure {
    if error == &K1ContractError::Internal {
        k1_internal_failure()
    } else {
        let error = if explorer {
            CodeNoesisErrorV16::from_explorer_contract(error)
        } else {
            CodeNoesisErrorV16::from_contract(error)
        };
        k1_failure(error, code)
    }
}

fn r10_portable_failure(
    error: noesis::portable_explorer::PortableExplorerError,
    code: u8,
    explorer: bool,
) -> Failure {
    match error {
        noesis::portable_explorer::PortableExplorerError::UnsafeOutput {
            path_sha256,
            reason,
        } => r10_failure(
            CodeNoesisErrorV17::unsafe_output_path(&path_sha256, reason),
            2,
        ),
        noesis::portable_explorer::PortableExplorerError::R10Contract(error) => {
            r10_contract_failure(&error, code, explorer)
        }
        noesis::portable_explorer::PortableExplorerError::Contract(_)
        | noesis::portable_explorer::PortableExplorerError::K1Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R11Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R12Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R13Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R14Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R15Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R16Contract(_)
        | noesis::portable_explorer::PortableExplorerError::Internal => r10_internal_failure(),
    }
}

fn r11_portable_failure(
    error: noesis::portable_explorer::PortableExplorerError,
    code: u8,
    explorer: bool,
) -> Failure {
    match error {
        noesis::portable_explorer::PortableExplorerError::UnsafeOutput {
            path_sha256,
            reason,
        } => r11_failure(
            CodeNoesisErrorV18::unsafe_output_path(&path_sha256, reason),
            2,
        ),
        noesis::portable_explorer::PortableExplorerError::R11Contract(error) => {
            r11_contract_failure(&error, code, explorer)
        }
        noesis::portable_explorer::PortableExplorerError::Contract(_)
        | noesis::portable_explorer::PortableExplorerError::K1Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R10Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R12Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R13Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R14Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R15Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R16Contract(_)
        | noesis::portable_explorer::PortableExplorerError::Internal => r11_internal_failure(),
    }
}

fn r12_portable_failure(
    error: noesis::portable_explorer::PortableExplorerError,
    code: u8,
    explorer: bool,
) -> Failure {
    match error {
        noesis::portable_explorer::PortableExplorerError::UnsafeOutput {
            path_sha256,
            reason,
        } => r12_failure(
            CodeNoesisErrorV19::unsafe_output_path(&path_sha256, reason),
            2,
        ),
        noesis::portable_explorer::PortableExplorerError::R12Contract(error) => {
            r12_contract_failure(&error, code, explorer)
        }
        noesis::portable_explorer::PortableExplorerError::Contract(_)
        | noesis::portable_explorer::PortableExplorerError::K1Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R10Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R11Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R13Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R14Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R15Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R16Contract(_)
        | noesis::portable_explorer::PortableExplorerError::Internal => r12_internal_failure(),
    }
}

fn r13_portable_failure(
    error: noesis::portable_explorer::PortableExplorerError,
    code: u8,
    explorer: bool,
) -> Failure {
    match error {
        noesis::portable_explorer::PortableExplorerError::UnsafeOutput {
            path_sha256,
            reason,
        } => r13_failure(
            CodeNoesisErrorV20::unsafe_output_path(&path_sha256, reason),
            2,
        ),
        noesis::portable_explorer::PortableExplorerError::R13Contract(error) => {
            r13_contract_failure(&error, code, explorer)
        }
        noesis::portable_explorer::PortableExplorerError::Contract(_)
        | noesis::portable_explorer::PortableExplorerError::K1Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R10Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R11Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R12Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R14Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R15Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R16Contract(_)
        | noesis::portable_explorer::PortableExplorerError::Internal => r13_internal_failure(),
    }
}

fn r13_contract_failure(error: &R13ContractError, code: u8, explorer: bool) -> Failure {
    if error == &R13ContractError::Internal {
        r13_internal_failure()
    } else {
        r13_failure(CodeNoesisErrorV20::from_contract(error, explorer), code)
    }
}

fn r14_portable_failure(
    error: noesis::portable_explorer::PortableExplorerError,
    code: u8,
    explorer: bool,
) -> Failure {
    match error {
        noesis::portable_explorer::PortableExplorerError::UnsafeOutput {
            path_sha256,
            reason,
        } => r14_failure(
            CodeNoesisErrorV21::unsafe_output_path(&path_sha256, reason),
            2,
        ),
        noesis::portable_explorer::PortableExplorerError::R14Contract(error) => {
            r14_contract_failure(&error, code, explorer)
        }
        noesis::portable_explorer::PortableExplorerError::Contract(_)
        | noesis::portable_explorer::PortableExplorerError::K1Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R10Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R11Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R12Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R13Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R15Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R16Contract(_)
        | noesis::portable_explorer::PortableExplorerError::Internal => r14_internal_failure(),
    }
}

fn r14_contract_failure(error: &R14ContractError, code: u8, explorer: bool) -> Failure {
    if error == &R14ContractError::Internal {
        r14_internal_failure()
    } else {
        let error = if explorer {
            CodeNoesisErrorV21::from_explorer_contract(error)
        } else {
            CodeNoesisErrorV21::from_contract(error)
        };
        r14_failure(error, code)
    }
}

fn r15_portable_failure(
    error: noesis::portable_explorer::PortableExplorerError,
    code: u8,
    explorer: bool,
) -> Failure {
    match error {
        noesis::portable_explorer::PortableExplorerError::UnsafeOutput {
            path_sha256,
            reason,
        } => r15_failure(
            CodeNoesisErrorV22::unsafe_output_path(&path_sha256, reason),
            2,
        ),
        noesis::portable_explorer::PortableExplorerError::R15Contract(error) => {
            r15_contract_failure(&error, code, explorer)
        }
        noesis::portable_explorer::PortableExplorerError::Contract(_)
        | noesis::portable_explorer::PortableExplorerError::K1Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R10Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R11Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R12Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R13Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R14Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R16Contract(_)
        | noesis::portable_explorer::PortableExplorerError::Internal => r15_internal_failure(),
    }
}

fn r15_contract_failure(error: &R15ContractError, code: u8, explorer: bool) -> Failure {
    if error == &R15ContractError::Internal {
        r15_internal_failure()
    } else {
        let error = if explorer {
            CodeNoesisErrorV22::from_explorer_contract(error)
        } else {
            CodeNoesisErrorV22::from_contract(error)
        };
        r15_failure(error, code)
    }
}

fn r16_portable_failure(
    error: noesis::portable_explorer::PortableExplorerError,
    code: u8,
    explorer: bool,
) -> Failure {
    match error {
        noesis::portable_explorer::PortableExplorerError::UnsafeOutput {
            path_sha256,
            reason,
        } => r16_failure(
            CodeNoesisErrorV24::unsafe_output_path(&path_sha256, reason),
            2,
        ),
        noesis::portable_explorer::PortableExplorerError::R16Contract(error) => {
            r16_contract_failure(&error, code, explorer)
        }
        noesis::portable_explorer::PortableExplorerError::Contract(_)
        | noesis::portable_explorer::PortableExplorerError::K1Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R10Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R11Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R12Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R13Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R14Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R15Contract(_)
        | noesis::portable_explorer::PortableExplorerError::Internal => r16_internal_failure(),
    }
}

fn r16_contract_failure(error: &R16ContractError, code: u8, explorer: bool) -> Failure {
    if error == &R16ContractError::Internal {
        r16_internal_failure()
    } else {
        let error = if explorer {
            CodeNoesisErrorV24::from_explorer_contract(error)
        } else {
            CodeNoesisErrorV24::from_contract(error)
        };
        r16_failure(error, code)
    }
}

fn r12_contract_failure(error: &R12ContractError, code: u8, explorer: bool) -> Failure {
    if error == &R12ContractError::Internal {
        r12_internal_failure()
    } else {
        r12_failure(CodeNoesisErrorV19::from_contract(error, explorer), code)
    }
}

fn r11_contract_failure(error: &R11ContractError, code: u8, explorer: bool) -> Failure {
    if error == &R11ContractError::Internal {
        r11_internal_failure()
    } else {
        r11_failure(CodeNoesisErrorV18::from_contract(error, explorer), code)
    }
}

fn r10_contract_failure(error: &R10ContractError, code: u8, explorer: bool) -> Failure {
    if error == &R10ContractError::Internal {
        r10_internal_failure()
    } else {
        r10_failure(CodeNoesisErrorV17::from_contract(error, explorer), code)
    }
}

fn r8_portable_failure(
    error: noesis::portable_explorer::PortableExplorerError,
    code: u8,
) -> Failure {
    match error {
        noesis::portable_explorer::PortableExplorerError::UnsafeOutput {
            path_sha256,
            reason,
        } => r8_failure(
            CodeNoesisErrorV15::unsafe_output_path(&path_sha256, reason),
            2,
        ),
        noesis::portable_explorer::PortableExplorerError::Contract(error) => {
            r8_contract_failure(error, code)
        }
        noesis::portable_explorer::PortableExplorerError::K1Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R10Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R11Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R12Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R13Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R14Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R15Contract(_)
        | noesis::portable_explorer::PortableExplorerError::R16Contract(_)
        | noesis::portable_explorer::PortableExplorerError::Internal => r8_internal_failure(),
    }
}

fn r8_contract_failure(error: R8ContractError, code: u8) -> Failure {
    let failure = if error == R8ContractError::Internal {
        r8_internal_failure()
    } else {
        r8_failure(CodeNoesisErrorV15::from_contract(&error), code)
    };
    drop(error);
    failure
}

fn r8_failure(error: CodeNoesisErrorV15, exit_code: u8) -> Failure {
    Failure::R8(R8Failure { error, exit_code })
}

fn r8_internal_failure() -> Failure {
    r8_failure(CodeNoesisErrorV15::internal(), 70)
}

fn zero_snapshot_id() -> String {
    "urn:codenoesis:snapshot:blake3:0000000000000000000000000000000000000000000000000000000000000000"
        .to_owned()
}

fn r7_failure(error: CodeNoesisErrorV14, exit_code: u8) -> Failure {
    Failure::R7(R7Failure { error, exit_code })
}

fn r7_internal_failure() -> Failure {
    r7_failure(CodeNoesisErrorV14::internal(), 70)
}

fn r6_failure(error: CodeNoesisErrorV13, exit_code: u8) -> Failure {
    Failure::R6(R6Failure { error, exit_code })
}

fn r6_internal_failure() -> Failure {
    r6_failure(CodeNoesisErrorV13::internal(), 70)
}

fn r5_failure(error: CodeNoesisErrorV12, exit_code: u8) -> Failure {
    Failure::R5(R5Failure { error, exit_code })
}

fn r10_failure(error: CodeNoesisErrorV17, exit_code: u8) -> Failure {
    Failure::R10(R10Failure { error, exit_code })
}

fn r10_internal_failure() -> Failure {
    r10_failure(CodeNoesisErrorV17::internal(), 70)
}

fn r5_internal_failure() -> Failure {
    r5_failure(CodeNoesisErrorV12::internal(), 70)
}

fn r4_failure(error: CodeNoesisErrorV11, exit_code: u8) -> Failure {
    Failure::R4(R4Failure { error, exit_code })
}

fn r4_internal_failure() -> Failure {
    r4_failure(CodeNoesisErrorV11::internal(), 70)
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

fn profile_option_requested(arguments: &[OsString], option: &str, profile: &str) -> bool {
    arguments
        .get(2..)
        .unwrap_or_default()
        .chunks(2)
        .any(|pair| {
            pair.first().is_some_and(|flag| flag == option)
                && pair.get(1).is_some_and(|value| value == profile)
        })
}

fn r14_profile_requested(arguments: &[OsString]) -> bool {
    option_requested(arguments, "--rust-expression-profile")
        || arguments
            .get(2..)
            .unwrap_or_default()
            .chunks(2)
            .any(|pair| {
                pair.first().is_some_and(|flag| {
                    flag == "--portable-profile" || flag == "--explorer-profile"
                }) && pair.get(1).is_some_and(|value| value == R14_PROFILE)
            })
}

fn r15_profile_requested(arguments: &[OsString]) -> bool {
    option_requested(arguments, "--rust-flow-profile")
        || arguments
            .get(2..)
            .unwrap_or_default()
            .chunks(2)
            .any(|pair| {
                pair.first().is_some_and(|flag| {
                    flag == "--portable-profile" || flag == "--explorer-profile"
                }) && pair.get(1).is_some_and(|value| value == R15_PROFILE)
            })
}

fn r16_profile_requested(arguments: &[OsString]) -> bool {
    option_requested(arguments, "--rust-constant-profile")
        || arguments
            .get(2..)
            .unwrap_or_default()
            .chunks(2)
            .any(|pair| {
                pair.first().is_some_and(|flag| {
                    flag == "--portable-profile" || flag == "--explorer-profile"
                }) && pair.get(1).is_some_and(|value| value == R16_PROFILE)
            })
}

fn rust_cfg_alternatives_requested(arguments: &[OsString]) -> bool {
    arguments
        .get(2..)
        .unwrap_or_default()
        .chunks(2)
        .any(|pair| {
            pair.first()
                .is_some_and(|flag| flag == "--rust-semantic-profile")
                && pair
                    .get(1)
                    .and_then(|value| value.to_str())
                    .is_some_and(|value| {
                        value == R10_PROFILE
                            || value.starts_with("rust-cfg-declaration-alternatives")
                    })
        })
}

fn rust_callable_cfg_alternatives_requested(arguments: &[OsString]) -> bool {
    rust_cfg_alternatives_requested(arguments)
        && option_requested(arguments, "--rust-callable-profile")
        && option_requested(arguments, "--rust-framework-profile")
}

fn rust_callable_scip_requested(arguments: &[OsString]) -> bool {
    let compiler_profile = option_requested(arguments, "--compiler-index-profile");
    let compiler_binding = option_requested(arguments, "--compiler-index-binding");
    option_requested(arguments, "--rust-callable-profile")
        && compiler_profile
        && (!option_requested(arguments, "--output-capacity-profile") || compiler_binding)
        && !legacy_r13_lineage_precedes_compiler(arguments)
}

fn legacy_r13_lineage_precedes_compiler(arguments: &[OsString]) -> bool {
    let boundary = first_option_position(
        arguments,
        &[
            "--repository-boundary-profile",
            "--repository-boundary-manifest",
        ],
    );
    let output_capacity = first_option_position(arguments, &["--output-capacity-profile"]);
    let cfg_alternatives = arguments
        .get(2..)
        .unwrap_or_default()
        .chunks(2)
        .position(|pair| {
            pair.first()
                .is_some_and(|flag| flag == "--rust-semantic-profile")
                && pair
                    .get(1)
                    .and_then(|value| value.to_str())
                    .is_some_and(|value| {
                        value == R10_PROFILE
                            || value.starts_with("rust-cfg-declaration-alternatives")
                    })
        });
    let compiler = first_option_position(
        arguments,
        &["--compiler-index-profile", "--compiler-index-binding"],
    );
    [boundary, output_capacity, cfg_alternatives]
        .into_iter()
        .flatten()
        .min()
        .zip(compiler)
        .is_some_and(|(legacy, compiler)| legacy < compiler)
}

fn first_option_position(arguments: &[OsString], expected: &[&str]) -> Option<usize> {
    arguments
        .get(2..)
        .unwrap_or_default()
        .chunks(2)
        .position(|pair| {
            pair.first()
                .is_some_and(|flag| expected.iter().any(|expected| flag == OsStr::new(expected)))
        })
}

fn r10_profile_requested(arguments: &[OsString]) -> bool {
    arguments
        .get(2..)
        .unwrap_or_default()
        .chunks(2)
        .any(|pair| {
            pair.first().is_some_and(|flag| {
                matches!(
                    flag.to_str(),
                    Some("--rust-semantic-profile" | "--portable-profile" | "--explorer-profile")
                )
            }) && pair
                .get(1)
                .and_then(|value| value.to_str())
                .is_some_and(|value| {
                    value == R10_PROFILE || value.starts_with("rust-cfg-declaration-alternatives")
                })
        })
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

fn emit_internal_error_v23() -> ExitCode {
    emit_error_v23(&CodeNoesisErrorV23::internal(), 1)
}

fn emit_internal_error_v25() -> ExitCode {
    emit_error_v25(&CodeNoesisErrorV25::contract_invalid(), 1)
}

fn emit_internal_error_v26() -> ExitCode {
    emit_error_v26(&CodeNoesisErrorV26::internal(), 1)
}

fn emit_error_v1(error: &CodeNoesisErrorV1, code: u8) -> ExitCode {
    if let Ok(bytes) = error.canonical_stderr() {
        let _ = io::stderr().lock().write_all(&bytes);
    }
    ExitCode::from(code)
}

fn emit_error_v26(error: &CodeNoesisErrorV26, code: u8) -> ExitCode {
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

fn emit_error_v23(error: &CodeNoesisErrorV23, code: u8) -> ExitCode {
    let Ok(bytes) = error.canonical_stderr() else {
        return ExitCode::from(1);
    };
    if io::stderr().lock().write_all(&bytes).is_ok() {
        ExitCode::from(code)
    } else {
        ExitCode::from(1)
    }
}

fn emit_error_v25(error: &CodeNoesisErrorV25, code: u8) -> ExitCode {
    let Ok(bytes) = error.canonical_stderr() else {
        return ExitCode::from(1);
    };
    if io::stderr().lock().write_all(&bytes).is_ok() {
        ExitCode::from(code)
    } else {
        ExitCode::from(1)
    }
}

fn emit_error_v11(error: &CodeNoesisErrorV11, code: u8) -> ExitCode {
    let Ok(bytes) = error.canonical_stderr() else {
        return ExitCode::from(70);
    };
    if io::stderr().lock().write_all(&bytes).is_ok() {
        ExitCode::from(code)
    } else {
        ExitCode::from(70)
    }
}

fn emit_error_v12(error: &CodeNoesisErrorV12, code: u8) -> ExitCode {
    let Ok(bytes) = error.canonical_stderr() else {
        return ExitCode::from(70);
    };
    if io::stderr().lock().write_all(&bytes).is_ok() {
        ExitCode::from(code)
    } else {
        ExitCode::from(70)
    }
}

fn emit_error_v13(error: &CodeNoesisErrorV13, code: u8) -> ExitCode {
    let Ok(bytes) = error.canonical_stderr() else {
        return ExitCode::from(70);
    };
    if io::stderr().lock().write_all(&bytes).is_ok() {
        ExitCode::from(code)
    } else {
        ExitCode::from(70)
    }
}

fn emit_error_v14(error: &CodeNoesisErrorV14, code: u8) -> ExitCode {
    let Ok(bytes) = error.canonical_stderr() else {
        return ExitCode::from(70);
    };
    if io::stderr().lock().write_all(&bytes).is_ok() {
        ExitCode::from(code)
    } else {
        ExitCode::from(70)
    }
}

fn emit_error_v15(error: &CodeNoesisErrorV15, code: u8) -> ExitCode {
    let Ok(bytes) = error.canonical_stderr() else {
        return ExitCode::from(70);
    };
    if io::stderr().lock().write_all(&bytes).is_ok() {
        ExitCode::from(code)
    } else {
        ExitCode::from(70)
    }
}

fn emit_error_v16(error: &CodeNoesisErrorV16, code: u8) -> ExitCode {
    let Ok(bytes) = error.canonical_stderr() else {
        return ExitCode::from(70);
    };
    if io::stderr().lock().write_all(&bytes).is_ok() {
        ExitCode::from(code)
    } else {
        ExitCode::from(70)
    }
}

fn emit_error_v17(error: &CodeNoesisErrorV17, code: u8) -> ExitCode {
    let Ok(bytes) = error.canonical_stderr() else {
        return ExitCode::from(70);
    };
    if io::stderr().lock().write_all(&bytes).is_ok() {
        ExitCode::from(code)
    } else {
        ExitCode::from(70)
    }
}

fn emit_error_v18(error: &CodeNoesisErrorV18, code: u8) -> ExitCode {
    let Ok(bytes) = error.canonical_stderr() else {
        return ExitCode::from(70);
    };
    if io::stderr().lock().write_all(&bytes).is_ok() {
        ExitCode::from(code)
    } else {
        ExitCode::from(70)
    }
}

fn emit_error_v19(error: &CodeNoesisErrorV19, code: u8) -> ExitCode {
    let Ok(bytes) = error.canonical_stderr() else {
        return ExitCode::from(70);
    };
    if io::stderr().lock().write_all(&bytes).is_ok() {
        ExitCode::from(code)
    } else {
        ExitCode::from(70)
    }
}

fn emit_error_v20(error: &CodeNoesisErrorV20, code: u8) -> ExitCode {
    let Ok(bytes) = error.canonical_stderr() else {
        return ExitCode::from(70);
    };
    if io::stderr().lock().write_all(&bytes).is_ok() {
        ExitCode::from(code)
    } else {
        ExitCode::from(70)
    }
}

fn emit_error_v21(error: &CodeNoesisErrorV21, code: u8) -> ExitCode {
    let Ok(bytes) = error.canonical_stderr() else {
        return ExitCode::from(70);
    };
    if io::stderr().lock().write_all(&bytes).is_ok() {
        ExitCode::from(code)
    } else {
        ExitCode::from(70)
    }
}

fn emit_error_v22(error: &CodeNoesisErrorV22, code: u8) -> ExitCode {
    let Ok(bytes) = error.canonical_stderr() else {
        return ExitCode::from(70);
    };
    if io::stderr().lock().write_all(&bytes).is_ok() {
        ExitCode::from(code)
    } else {
        ExitCode::from(70)
    }
}

fn emit_error_v24(error: &CodeNoesisErrorV24, code: u8) -> ExitCode {
    let Ok(bytes) = error.canonical_stderr() else {
        return ExitCode::from(70);
    };
    if io::stderr().lock().write_all(&bytes).is_ok() {
        ExitCode::from(code)
    } else {
        ExitCode::from(70)
    }
}

fn emit_error_v29(error: &CodeNoesisErrorV29, code: u8) -> ExitCode {
    let Ok(bytes) = error.canonical_stderr() else {
        return ExitCode::from(1);
    };
    if io::stderr().lock().write_all(&bytes).is_ok() {
        ExitCode::from(code)
    } else {
        ExitCode::from(1)
    }
}

fn emit_error_v30(error: &CodeNoesisErrorV30, code: u8) -> ExitCode {
    let Ok(bytes) = error.canonical_stderr() else {
        return ExitCode::from(1);
    };
    if io::stderr().lock().write_all(&bytes).is_ok() {
        ExitCode::from(code)
    } else {
        ExitCode::from(1)
    }
}

fn emit_internal_error_v10() -> ExitCode {
    emit_error_v10(&CodeNoesisErrorV10::internal(), 70)
}

fn emit_internal_error_v11() -> ExitCode {
    emit_error_v11(&CodeNoesisErrorV11::internal(), 70)
}

fn emit_internal_error_v12() -> ExitCode {
    emit_error_v12(&CodeNoesisErrorV12::internal(), 70)
}

fn emit_internal_error_v13() -> ExitCode {
    emit_error_v13(&CodeNoesisErrorV13::internal(), 70)
}

fn emit_internal_error_v14() -> ExitCode {
    emit_error_v14(&CodeNoesisErrorV14::internal(), 70)
}

fn emit_internal_error_v15() -> ExitCode {
    emit_error_v15(&CodeNoesisErrorV15::internal(), 70)
}

fn emit_internal_error_v16() -> ExitCode {
    emit_error_v16(&CodeNoesisErrorV16::internal(), 70)
}

fn emit_internal_error_v17() -> ExitCode {
    emit_error_v17(&CodeNoesisErrorV17::internal(), 70)
}

fn emit_internal_error_v18() -> ExitCode {
    emit_error_v18(&CodeNoesisErrorV18::internal(), 70)
}

fn emit_internal_error_v19() -> ExitCode {
    emit_error_v19(&CodeNoesisErrorV19::internal("runtime"), 70)
}

fn emit_internal_error_v20() -> ExitCode {
    emit_error_v20(&CodeNoesisErrorV20::internal("runtime"), 70)
}

fn emit_internal_error_v21() -> ExitCode {
    emit_error_v21(&CodeNoesisErrorV21::internal("runtime"), 70)
}

fn emit_internal_error_v22() -> ExitCode {
    emit_error_v22(&CodeNoesisErrorV22::internal("runtime"), 70)
}

fn emit_internal_error_v24() -> ExitCode {
    emit_error_v24(&CodeNoesisErrorV24::internal("runtime"), 70)
}

fn emit_internal_error_v29() -> ExitCode {
    emit_error_v29(&CodeNoesisErrorV29::internal(), 1)
}

fn emit_internal_error_v30() -> ExitCode {
    emit_error_v30(&CodeNoesisErrorV30::source_internal(), 1)
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
    R19Source(impact_source::ImpactSourceFailure),
    R19(impact_git::ImpactGitFailure),
    R18(R18Failure),
    G0(G0Failure),
    S7(impact::ImpactFailure),
    R16(R16Failure),
    R15(R15Failure),
    R14(R14Failure),
    R13(R13Failure),
    R12(R12Failure),
    R11(R11Failure),
    K1(K1Failure),
    R8(R8Failure),
    S6(federation::FederationFailure),
    R7(R7Failure),
    R6(R6Failure),
    R10(R10Failure),
    R5(R5Failure),
    R4(R4Failure),
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

struct R18Failure {
    error: CodeNoesisErrorV29,
    exit_code: u8,
}

struct G0Failure {
    error: CodeNoesisErrorV25,
    exit_code: u8,
}

fn g0_failure(error: CodeNoesisErrorV25, exit_code: u8) -> Failure {
    Failure::G0(G0Failure { error, exit_code })
}

struct R16Failure {
    error: CodeNoesisErrorV24,
    exit_code: u8,
}

struct R15Failure {
    error: CodeNoesisErrorV22,
    exit_code: u8,
}

struct R14Failure {
    error: CodeNoesisErrorV21,
    exit_code: u8,
}

struct R13Failure {
    error: CodeNoesisErrorV20,
    exit_code: u8,
}

struct R12Failure {
    error: CodeNoesisErrorV19,
    exit_code: u8,
}

struct R11Failure {
    error: CodeNoesisErrorV18,
    exit_code: u8,
}

struct K1Failure {
    error: CodeNoesisErrorV16,
    exit_code: u8,
}

struct R8Failure {
    error: CodeNoesisErrorV15,
    exit_code: u8,
}

struct R7Failure {
    error: CodeNoesisErrorV14,
    exit_code: u8,
}

struct R6Failure {
    error: CodeNoesisErrorV13,
    exit_code: u8,
}

struct R10Failure {
    error: CodeNoesisErrorV17,
    exit_code: u8,
}

struct R5Failure {
    error: CodeNoesisErrorV12,
    exit_code: u8,
}

struct R4Failure {
    error: CodeNoesisErrorV11,
    exit_code: u8,
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
    UnsupportedSnapshotSchema(String),
}

struct LoadedS4Snapshot {
    head: LocalSnapshotHead,
    semantic: Value,
}

struct LoadedS5AnalysisCache {
    entries: Vec<AnalysisCacheEntry>,
    versions_compatible: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PortableProfile {
    R8,
    K1,
    R10,
    R14,
    R15,
    R16,
    R17,
}

impl PortableProfile {
    fn requested(arguments: &[OsString], option: &str) -> Self {
        if profile_option_requested(arguments, option, R17_CONTEXT_PROFILE) {
            Self::R17
        } else if r16_profile_requested(arguments) {
            Self::R16
        } else if r15_profile_requested(arguments) {
            Self::R15
        } else if r14_profile_requested(arguments) {
            Self::R14
        } else if r10_profile_requested(arguments) {
            Self::R10
        } else if option_requested(arguments, option) {
            Self::K1
        } else {
            Self::R8
        }
    }

    fn requires_documents(self) -> bool {
        matches!(self, Self::R14 | Self::R15 | Self::R16)
    }
}

struct ExportInvocation {
    store: OsString,
    identity: RepositoryIdentity,
    documents: Option<OsString>,
    output: OsString,
    profile: PortableProfile,
}

impl ExportInvocation {
    #[allow(clippy::too_many_lines)]
    fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Self, Failure> {
        let arguments = arguments.into_iter().collect::<Vec<_>>();
        let requested_profile = PortableProfile::requested(&arguments, "--portable-profile");
        let mut arguments = arguments.into_iter();
        let _program = arguments.next();
        if arguments.next().as_deref() != Some(OsStr::new("export")) {
            return Err(invalid_export_profile(requested_profile));
        }
        let mut store = None;
        let mut identity = None;
        let mut documents = None;
        let mut output = None;
        let mut format = None;
        let mut portable_profile = None;
        while let Some(flag) = arguments.next() {
            let value = arguments
                .next()
                .ok_or_else(|| invalid_export_profile(requested_profile))?;
            if value.to_str().is_some_and(|value| value.starts_with("--")) {
                return Err(invalid_export_profile(requested_profile));
            }
            match flag.to_str() {
                Some("--store") if store.is_none() => store = Some(value),
                Some("--repository-id") if identity.is_none() => {
                    identity = value.to_str().map(str::to_owned);
                }
                Some("--documents") if documents.is_none() => documents = Some(value),
                Some("--output") if output.is_none() => output = Some(value),
                Some("--format") if format.is_none() => format = value.to_str().map(str::to_owned),
                Some("--portable-profile") if portable_profile.is_none() => {
                    portable_profile = value.to_str().map(str::to_owned);
                }
                _ => {
                    return Err(invalid_export_profile(requested_profile));
                }
            }
        }
        let store = store
            .filter(|value: &OsString| !value.is_empty())
            .ok_or_else(|| invalid_export_profile(requested_profile))?;
        let identity = identity
            .and_then(|value| RepositoryIdentity::parse(&value).ok())
            .ok_or_else(|| invalid_export_profile(requested_profile))?;
        let output = output
            .filter(|value: &OsString| !value.is_empty())
            .ok_or_else(|| invalid_export_profile(requested_profile))?;
        if format.as_deref() != Some("json") {
            return Err(invalid_export_profile(requested_profile));
        }
        let profile = match portable_profile.as_deref() {
            None => PortableProfile::R8,
            Some(K1_PROFILE) => PortableProfile::K1,
            Some(R10_PROFILE) => PortableProfile::R10,
            Some(R14_PROFILE) => PortableProfile::R14,
            Some(R15_PROFILE) => PortableProfile::R15,
            Some(R16_PROFILE) => PortableProfile::R16,
            Some(R17_CONTEXT_PROFILE) => {
                return Err(invalid_export_profile(PortableProfile::R17));
            }
            Some(profile) if profile.starts_with("rust-safe-constant-evaluation") => {
                return Err(r16_failure(CodeNoesisErrorV24::invalid_profile(profile), 2));
            }
            Some(profile) if profile.starts_with("rust-local-flow") => {
                return Err(r15_failure(CodeNoesisErrorV22::invalid_profile(profile), 2));
            }
            Some(profile) if profile.starts_with("rust-expression-bindings") => {
                return Err(r14_failure(CodeNoesisErrorV21::invalid_profile(profile), 2));
            }
            Some(profile) if profile.starts_with("rust-cfg-declaration-alternatives") => {
                return Err(r10_failure(CodeNoesisErrorV17::invalid_profile(profile), 2));
            }
            Some(profile) => {
                return Err(k1_failure(
                    CodeNoesisErrorV16::invalid_rust_callable_profile(profile),
                    2,
                ));
            }
        };
        let documents = match (profile.requires_documents(), documents) {
            (true, Some(value)) if valid_s4_root_argument(&value) => Some(value),
            (true, _) | (false, Some(_)) => {
                return Err(invalid_export_profile(requested_profile));
            }
            (false, None) => None,
        };
        reject_parent_output(&output, profile)?;
        Ok(Self {
            store,
            identity,
            documents,
            output,
            profile,
        })
    }
}

struct ExploreInvocation {
    input: OsString,
    output: OsString,
    profile: PortableProfile,
}

impl ExploreInvocation {
    fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Self, Failure> {
        let arguments = arguments.into_iter().collect::<Vec<_>>();
        let requested_profile = PortableProfile::requested(&arguments, "--explorer-profile");
        let mut arguments = arguments.into_iter();
        let _program = arguments.next();
        if arguments.next().as_deref() != Some(OsStr::new("explore")) {
            return Err(invalid_explorer_profile(requested_profile));
        }
        let mut input = None;
        let mut output = None;
        let mut format = None;
        let mut explorer_profile = None;
        while let Some(flag) = arguments.next() {
            let value = arguments
                .next()
                .ok_or_else(|| invalid_explorer_profile(requested_profile))?;
            if value.to_str().is_some_and(|value| value.starts_with("--")) {
                return Err(invalid_explorer_profile(requested_profile));
            }
            match flag.to_str() {
                Some("--input") if input.is_none() => input = Some(value),
                Some("--output") if output.is_none() => output = Some(value),
                Some("--format") if format.is_none() => format = value.to_str().map(str::to_owned),
                Some("--explorer-profile") if explorer_profile.is_none() => {
                    explorer_profile = value.to_str().map(str::to_owned);
                }
                _ => {
                    return Err(invalid_explorer_profile(requested_profile));
                }
            }
        }
        let input = input
            .filter(|value: &OsString| !value.is_empty())
            .ok_or_else(|| invalid_explorer_profile(requested_profile))?;
        let output = output
            .filter(|value: &OsString| !value.is_empty())
            .ok_or_else(|| invalid_explorer_profile(requested_profile))?;
        if format.as_deref() != Some("json") {
            return Err(invalid_explorer_profile(requested_profile));
        }
        let profile = match explorer_profile.as_deref() {
            None => PortableProfile::R8,
            Some(K1_PROFILE) => PortableProfile::K1,
            Some(R10_PROFILE) => PortableProfile::R10,
            Some(R14_PROFILE) => PortableProfile::R14,
            Some(R15_PROFILE) => PortableProfile::R15,
            Some(R16_PROFILE) => PortableProfile::R16,
            Some(R17_CONTEXT_PROFILE) => PortableProfile::R17,
            Some(profile) if profile.starts_with("rust-safe-constant-evaluation") => {
                return Err(r16_failure(CodeNoesisErrorV24::invalid_profile(profile), 2));
            }
            Some(profile) if profile.starts_with("rust-local-flow") => {
                return Err(r15_failure(CodeNoesisErrorV22::invalid_profile(profile), 2));
            }
            Some(profile) if profile.starts_with("rust-expression-bindings") => {
                return Err(r14_failure(CodeNoesisErrorV21::invalid_profile(profile), 2));
            }
            Some(profile) if profile.starts_with("rust-cfg-declaration-alternatives") => {
                return Err(r10_failure(CodeNoesisErrorV17::invalid_profile(profile), 2));
            }
            Some(profile) => {
                return Err(k1_failure(
                    CodeNoesisErrorV16::invalid_rust_callable_profile(profile),
                    2,
                ));
            }
        };
        reject_parent_output(&output, profile)?;
        Ok(Self {
            input,
            output,
            profile,
        })
    }
}

fn invalid_export_profile(profile: PortableProfile) -> Failure {
    match profile {
        PortableProfile::R17 => r16_failure(
            CodeNoesisErrorV24::unsupported_composition("context_has_no_portable_profile"),
            2,
        ),
        PortableProfile::R16 => r16_failure(
            CodeNoesisErrorV24::unsupported_composition("invalid_export"),
            2,
        ),
        PortableProfile::R15 => r15_failure(
            CodeNoesisErrorV22::unsupported_composition("invalid_export"),
            2,
        ),
        PortableProfile::R14 => r14_failure(
            CodeNoesisErrorV21::unsupported_composition("invalid_export"),
            2,
        ),
        PortableProfile::R10 => r10_failure(
            CodeNoesisErrorV17::unsupported_composition("invalid_export"),
            2,
        ),
        PortableProfile::K1 => k1_failure(CodeNoesisErrorV16::unsupported_composition(), 2),
        PortableProfile::R8 => invalid_r8_export_profile(),
    }
}

fn invalid_explorer_profile(profile: PortableProfile) -> Failure {
    match profile {
        PortableProfile::R17 => r16_failure(
            CodeNoesisErrorV24::unsupported_composition("invalid_function_context_explorer"),
            2,
        ),
        PortableProfile::R16 => r16_failure(
            CodeNoesisErrorV24::unsupported_composition("invalid_explorer"),
            2,
        ),
        PortableProfile::R15 => r15_failure(
            CodeNoesisErrorV22::unsupported_composition("invalid_explorer"),
            2,
        ),
        PortableProfile::R14 => r14_failure(
            CodeNoesisErrorV21::unsupported_composition("invalid_explorer"),
            2,
        ),
        PortableProfile::R10 => r10_failure(
            CodeNoesisErrorV17::unsupported_composition("invalid_explorer"),
            2,
        ),
        PortableProfile::K1 => k1_failure(CodeNoesisErrorV16::unsupported_composition(), 2),
        PortableProfile::R8 => invalid_r8_explorer_profile(),
    }
}

fn invalid_r8_export_profile() -> Failure {
    r8_failure(CodeNoesisErrorV15::invalid_export_profile(), 2)
}

fn invalid_r8_explorer_profile() -> Failure {
    r8_failure(CodeNoesisErrorV15::invalid_explorer_profile(), 2)
}

fn reject_parent_output(output: &OsStr, profile: PortableProfile) -> Result<(), Failure> {
    if std::path::Path::new(output)
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        let path_sha256 = noesis::portable_explorer::sha256(output.as_encoded_bytes());
        match profile {
            PortableProfile::R17 | PortableProfile::R16 => Err(r16_failure(
                CodeNoesisErrorV24::unsafe_output_path(&path_sha256, "parent_escape"),
                2,
            )),
            PortableProfile::R15 => Err(r15_failure(
                CodeNoesisErrorV22::unsafe_output_path(&path_sha256, "parent_escape"),
                2,
            )),
            PortableProfile::R14 => Err(r14_failure(
                CodeNoesisErrorV21::unsafe_output_path(&path_sha256, "parent_escape"),
                2,
            )),
            PortableProfile::R10 => Err(r10_failure(
                CodeNoesisErrorV17::unsafe_output_path(&path_sha256, "parent_escape"),
                2,
            )),
            PortableProfile::K1 => Err(k1_failure(
                CodeNoesisErrorV16::unsafe_output_path(&path_sha256, "parent_escape"),
                2,
            )),
            PortableProfile::R8 => Err(r8_failure(
                CodeNoesisErrorV15::unsafe_output_path(&path_sha256, "parent_escape"),
                2,
            )),
        }
    } else {
        Ok(())
    }
}

struct SourceInvocation {
    repository: OsString,
    revision: codenoesis_domain::ObjectId,
    identity: RepositoryIdentity,
    store: OsString,
    evidence_id: String,
    packed_sha1: bool,
    boundary_profile: bool,
}

impl SourceInvocation {
    fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Self, Failure> {
        let mut arguments = arguments.into_iter();
        let _program = arguments.next();
        if arguments.next().as_deref() != Some(OsStr::new("source")) {
            return Err(r18_failure(CodeNoesisErrorV29::invalid_arguments(), 2));
        }
        let mut repository = None;
        let mut revision = None;
        let mut identity = None;
        let mut store = None;
        let mut evidence_id = None;
        let mut source_profile = None;
        let mut acquisition_profile = None;
        let mut boundary_profile = None;
        let mut format = None;
        while let Some(flag) = arguments.next() {
            let Some(value) = arguments.next() else {
                return Err(r18_failure(CodeNoesisErrorV29::invalid_arguments(), 2));
            };
            match flag.to_str() {
                Some("--repository") if repository.is_none() => repository = Some(value),
                Some("--revision") if revision.is_none() => {
                    revision = value.to_str().map(str::to_owned);
                }
                Some("--repository-id") if identity.is_none() => {
                    identity = value.to_str().map(str::to_owned);
                }
                Some("--store") if store.is_none() => store = Some(value),
                Some("--evidence-id") if evidence_id.is_none() => {
                    evidence_id = value.to_str().map(str::to_owned);
                }
                Some("--source-profile") if source_profile.is_none() => {
                    source_profile = value.to_str().map(str::to_owned);
                }
                Some("--acquisition-profile") if acquisition_profile.is_none() => {
                    acquisition_profile = value.to_str().map(str::to_owned);
                }
                Some("--repository-boundary-profile") if boundary_profile.is_none() => {
                    boundary_profile = value.to_str().map(str::to_owned);
                }
                Some("--format") if format.is_none() => {
                    format = value.to_str().map(str::to_owned);
                }
                _ => return Err(r18_failure(CodeNoesisErrorV29::invalid_arguments(), 2)),
            }
        }
        let repository = repository
            .filter(|value: &OsString| !value.is_empty())
            .ok_or_else(|| r18_failure(CodeNoesisErrorV29::invalid_arguments(), 2))?;
        let revision = revision
            .as_deref()
            .and_then(codenoesis_domain::ObjectId::parse_sha1)
            .ok_or_else(|| r18_failure(CodeNoesisErrorV29::invalid_arguments(), 2))?;
        let identity = identity
            .as_deref()
            .and_then(|value| RepositoryIdentity::parse(value).ok())
            .ok_or_else(|| r18_failure(CodeNoesisErrorV29::invalid_arguments(), 2))?;
        let store = store
            .filter(|value: &OsString| !value.is_empty())
            .ok_or_else(|| r18_failure(CodeNoesisErrorV29::invalid_arguments(), 2))?;
        let evidence_id = evidence_id
            .filter(|value| valid_r18_evidence_id(value))
            .ok_or_else(|| r18_failure(CodeNoesisErrorV29::invalid_arguments(), 2))?;
        if source_profile.as_deref() != Some(R18_SOURCE_PROFILE)
            || acquisition_profile
                .as_deref()
                .is_some_and(|profile| profile != LOCAL_GIT_SHA1_PACKED_V1)
            || boundary_profile
                .as_deref()
                .is_some_and(|profile| profile != LOCAL_GITLINKS_V1)
            || format.as_deref() != Some("json")
        {
            return Err(r18_failure(CodeNoesisErrorV29::invalid_arguments(), 2));
        }
        Ok(Self {
            repository,
            revision,
            identity,
            store,
            evidence_id,
            packed_sha1: acquisition_profile.is_some(),
            boundary_profile: boundary_profile.is_some(),
        })
    }
}

fn valid_r18_evidence_id(value: &str) -> bool {
    [
        "urn:codenoesis:evidence:blake3:",
        "urn:codenoesis:evidence:sha256:",
    ]
    .iter()
    .any(|prefix| {
        value.strip_prefix(prefix).is_some_and(|digest| {
            digest.len() == 64
                && digest
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        })
    })
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
    context_profile: Option<String>,
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
        let mut context_profile = None;
        let mut format = None;
        while let Some(flag) = arguments.next() {
            let value = arguments.next().ok_or_else(|| {
                if flag == OsStr::new("--documents") {
                    Failure::S4Input(CodeNoesisErrorV5::invalid_documents_root())
                } else if flag == OsStr::new("--id") {
                    Failure::S4Input(CodeNoesisErrorV5::invalid_query_id())
                } else if flag == OsStr::new("--context-profile") {
                    r16_failure(CodeNoesisErrorV24::invalid_query(), 2)
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
                Some("--context-profile") if context_profile.is_none() => {
                    context_profile = value.to_str().map(str::to_owned);
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
        if context_profile
            .as_deref()
            .is_some_and(|profile| profile != R17_CONTEXT_PROFILE)
        {
            return Err(r16_failure(CodeNoesisErrorV24::invalid_query(), 2));
        }
        Ok(Self {
            store,
            identity,
            documents,
            requested_id,
            context_profile,
        })
    }
}

fn valid_query_id(value: &str) -> bool {
    [
        "urn:codenoesis:entity:blake3:",
        "urn:codenoesis:relationship:blake3:",
        "urn:codenoesis:claim:blake3:",
        "urn:codenoesis:evidence:blake3:",
        "urn:codenoesis:evidence:sha256:",
        "urn:codenoesis:diagnostic:blake3:",
        "urn:codenoesis:coverage-gap:blake3:",
        "urn:codenoesis:document:blake3:",
        "urn:codenoesis:repository-boundary:sha256:",
        "urn:codenoesis:gitmodules-declaration:sha256:",
        "urn:codenoesis:boundary-evidence:sha256:",
        "urn:codenoesis:boundary-gap:sha256:",
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

fn v2_only_query_id(value: &str) -> bool {
    value.starts_with("urn:codenoesis:diagnostic:blake3:")
}

fn v4_only_query_id(value: &str) -> bool {
    value.starts_with("urn:codenoesis:evidence:sha256:")
}

fn valid_s4_root_argument(value: &OsStr) -> bool {
    !value.is_empty()
        && !std::path::Path::new(value)
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
}

#[allow(clippy::too_many_lines)]
fn parse_s4_invocation(arguments: Vec<OsString>) -> Result<Invocation, InvocationError> {
    if option_requested(&arguments, "--rust-constant-profile") {
        return parse_r16_invocation(&arguments);
    }
    if option_requested(&arguments, "--rust-flow-profile") {
        return parse_r15_invocation(&arguments);
    }
    if option_requested(&arguments, "--rust-expression-profile") {
        return parse_r14_invocation(&arguments);
    }
    if rust_callable_scip_requested(&arguments) {
        return parse_r13_invocation(&arguments);
    }
    if rust_callable_cfg_alternatives_requested(&arguments) {
        return parse_r12_invocation(&arguments);
    }
    if rust_cfg_alternatives_requested(&arguments) {
        return parse_r10_invocation(&arguments);
    }
    if option_requested(&arguments, "--rust-callable-profile")
        || option_requested(&arguments, "--output-capacity-profile")
    {
        return parse_k1_invocation(&arguments);
    }
    let compiler_requested = option_requested(&arguments, "--compiler-index-profile")
        || option_requested(&arguments, "--compiler-index-binding");
    if !compiler_requested {
        return Invocation::parse(arguments, Some("standard-local-s4"));
    }

    let selected_profiles = selected_compiler_profiles(&arguments);
    let mut stripped = arguments.iter().take(2).cloned().collect::<Vec<_>>();
    let mut compiler_profile = None;
    let mut compiler_binding = None;
    let mut index = 2;
    while index < arguments.len() {
        let flag = &arguments[index];
        let Some(value) = arguments.get(index + 1) else {
            return Err(InvocationError::InvalidCompilerIndexComposition(
                selected_profiles,
            ));
        };
        if flag == OsStr::new("--compiler-index-profile") {
            if compiler_profile.is_some() {
                return Err(InvocationError::InvalidCompilerIndexComposition(
                    selected_profiles,
                ));
            }
            let Some(value) = value.to_str() else {
                return Err(InvocationError::InvalidCompilerIndexProfile(String::new()));
            };
            compiler_profile = Some(value.to_owned());
        } else if flag == OsStr::new("--compiler-index-binding") {
            if compiler_binding.is_some() {
                return Err(InvocationError::InvalidCompilerIndexComposition(
                    selected_profiles,
                ));
            }
            compiler_binding = Some(value.clone());
        } else {
            stripped.push(flag.clone());
            stripped.push(value.clone());
        }
        index += 2;
    }

    let Some(compiler_profile) = compiler_profile else {
        return Err(InvocationError::InvalidCompilerIndexComposition(
            selected_profiles,
        ));
    };
    if compiler_profile != R7_COMPILER_INDEX_PROFILE {
        return Err(InvocationError::InvalidCompilerIndexProfile(
            compiler_profile,
        ));
    }
    let Some(compiler_binding) = compiler_binding else {
        return Err(InvocationError::InvalidCompilerIndexComposition(
            selected_profiles,
        ));
    };
    if !required_r7_profiles_selected(&selected_profiles) {
        return Err(InvocationError::InvalidCompilerIndexComposition(
            selected_profiles,
        ));
    }

    let mut invocation =
        Invocation::parse(stripped, Some("standard-local-s4")).map_err(|error| match error {
            InvocationError::InvalidWorkspaceProfile
            | InvocationError::InvalidManifestProfile
            | InvocationError::InvalidRustSemanticProfile(_)
            | InvocationError::InvalidRustSemanticComposition(_)
            | InvocationError::InvalidRustFrameworkProfile(_)
            | InvocationError::InvalidRustFrameworkComposition(_) => {
                InvocationError::InvalidCompilerIndexComposition(selected_profiles.clone())
            }
            other => other,
        })?;
    if !(invocation.workspace_profile
        && invocation.manifest_profile
        && invocation.rust_semantic_profile
        && invocation.rust_framework_profile)
    {
        return Err(InvocationError::InvalidCompilerIndexComposition(
            selected_profiles,
        ));
    }
    invocation.compiler_index_profile = true;
    invocation.compiler_index_binding = Some(compiler_binding);
    Ok(invocation)
}

fn parse_r16_invocation(arguments: &[OsString]) -> Result<Invocation, InvocationError> {
    if arguments
        .get(1)
        .is_none_or(|value| value != OsStr::new("scan"))
    {
        return Err(InvocationError::InvalidR16Composition(
            "r15_source_only_without_boundaries_cfg_or_compiler_index",
        ));
    }
    if option_requested(arguments, "--repository-boundary-profile")
        || option_requested(arguments, "--repository-boundary-manifest")
    {
        return Err(InvocationError::InvalidR16Composition(
            "repository_boundary_not_supported",
        ));
    }
    if option_requested(arguments, "--compiler-index-profile")
        || option_requested(arguments, "--compiler-index-binding")
        || rust_cfg_alternatives_requested(arguments)
    {
        return Err(InvocationError::InvalidR16Composition(
            "r15_source_only_without_boundaries_cfg_or_compiler_index",
        ));
    }
    let mut stripped = arguments.iter().take(2).cloned().collect::<Vec<_>>();
    let mut constant_profile = None;
    let mut index = 2;
    while index < arguments.len() {
        let flag = &arguments[index];
        let Some(value) = arguments.get(index + 1) else {
            return Err(InvocationError::InvalidR16Composition(
                "complete_option_pair_required",
            ));
        };
        if flag == OsStr::new("--rust-constant-profile") {
            if constant_profile.is_some() {
                return Err(InvocationError::InvalidR16Composition(
                    "single_constant_profile_required",
                ));
            }
            let Some(value) = value.to_str() else {
                return Err(InvocationError::InvalidR16Profile(String::new()));
            };
            constant_profile = Some(value.to_owned());
        } else {
            stripped.push(flag.clone());
            stripped.push(value.clone());
        }
        index += 2;
    }
    let Some(constant_profile) = constant_profile else {
        return Err(InvocationError::InvalidR16Composition(
            "constant_profile_required",
        ));
    };
    if constant_profile != R16_PROFILE {
        return Err(InvocationError::InvalidR16Profile(constant_profile));
    }
    let mut invocation = parse_r15_invocation(&stripped).map_err(|error| match error {
        InvocationError::Input(error) => InvocationError::Input(error),
        InvocationError::InvalidR15Profile(_) => {
            InvocationError::InvalidR16Composition("exact_r15_flow_profile_required")
        }
        _ => InvocationError::InvalidR16Composition("exact_r15_selector_matrix_required"),
    })?;
    if !invocation.rust_flow_profile
        || invocation.boundary_profile
        || invocation.rust_cfg_alternatives_profile
        || invocation.compiler_index_profile
    {
        return Err(InvocationError::InvalidR16Composition(
            "complete_r15_source_only_profiles_required",
        ));
    }
    invocation.rust_constant_profile = true;
    Ok(invocation)
}

fn parse_r15_invocation(arguments: &[OsString]) -> Result<Invocation, InvocationError> {
    if arguments
        .get(1)
        .is_none_or(|value| value != OsStr::new("scan"))
        || option_requested(arguments, "--repository-boundary-profile")
        || option_requested(arguments, "--repository-boundary-manifest")
        || option_requested(arguments, "--compiler-index-profile")
        || option_requested(arguments, "--compiler-index-binding")
        || rust_cfg_alternatives_requested(arguments)
    {
        return Err(InvocationError::InvalidR15Composition(
            "r14_source_only_without_boundaries_cfg_or_compiler_index",
        ));
    }
    let mut stripped = arguments.iter().take(2).cloned().collect::<Vec<_>>();
    let mut flow_profile = None;
    let mut index = 2;
    while index < arguments.len() {
        let flag = &arguments[index];
        let Some(value) = arguments.get(index + 1) else {
            return Err(InvocationError::InvalidR15Composition(
                "complete_option_pair_required",
            ));
        };
        if flag == OsStr::new("--rust-flow-profile") {
            if flow_profile.is_some() {
                return Err(InvocationError::InvalidR15Composition(
                    "single_flow_profile_required",
                ));
            }
            let Some(value) = value.to_str() else {
                return Err(InvocationError::InvalidR15Profile(String::new()));
            };
            flow_profile = Some(value.to_owned());
        } else {
            stripped.push(flag.clone());
            stripped.push(value.clone());
        }
        index += 2;
    }
    let Some(flow_profile) = flow_profile else {
        return Err(InvocationError::InvalidR15Composition(
            "flow_profile_required",
        ));
    };
    if flow_profile != R15_PROFILE {
        return Err(InvocationError::InvalidR15Profile(flow_profile));
    }
    let mut invocation = parse_r14_invocation(&stripped).map_err(|error| match error {
        InvocationError::Input(error) => InvocationError::Input(error),
        InvocationError::InvalidR14Profile(_) => {
            InvocationError::InvalidR15Composition("exact_r14_expression_profile_required")
        }
        _ => InvocationError::InvalidR15Composition("exact_r14_selector_matrix_required"),
    })?;
    if !invocation.rust_expression_profile
        || invocation.boundary_profile
        || invocation.rust_cfg_alternatives_profile
        || invocation.compiler_index_profile
    {
        return Err(InvocationError::InvalidR15Composition(
            "complete_r14_source_only_profiles_required",
        ));
    }
    invocation.rust_flow_profile = true;
    Ok(invocation)
}

fn parse_r14_invocation(arguments: &[OsString]) -> Result<Invocation, InvocationError> {
    if arguments
        .get(1)
        .is_none_or(|value| value != OsStr::new("scan"))
        || option_requested(arguments, "--repository-boundary-profile")
        || option_requested(arguments, "--repository-boundary-manifest")
        || option_requested(arguments, "--compiler-index-profile")
        || option_requested(arguments, "--compiler-index-binding")
        || rust_cfg_alternatives_requested(arguments)
    {
        return Err(InvocationError::InvalidR14Composition(
            "k1_source_only_without_boundaries_cfg_or_compiler_index",
        ));
    }
    let mut stripped = arguments.iter().take(2).cloned().collect::<Vec<_>>();
    let mut expression_profile = None;
    let mut output_capacity_profile = None;
    let mut index = 2;
    while index < arguments.len() {
        let flag = &arguments[index];
        let Some(value) = arguments.get(index + 1) else {
            return Err(InvocationError::InvalidR14Composition(
                "complete_option_pair_required",
            ));
        };
        if flag == OsStr::new("--rust-expression-profile") {
            if expression_profile.is_some() {
                return Err(InvocationError::InvalidR14Composition(
                    "single_expression_profile_required",
                ));
            }
            let Some(value) = value.to_str() else {
                return Err(InvocationError::InvalidR14Profile(String::new()));
            };
            expression_profile = Some(value.to_owned());
        } else if flag == OsStr::new("--output-capacity-profile") {
            if output_capacity_profile.is_some() {
                return Err(InvocationError::InvalidR14Composition(
                    "single_output_capacity_profile_required",
                ));
            }
            let Some(value) = value.to_str() else {
                return Err(InvocationError::InvalidR14Composition(
                    "valid_output_capacity_profile_required",
                ));
            };
            output_capacity_profile = Some(value.to_owned());
        } else {
            stripped.push(flag.clone());
            stripped.push(value.clone());
        }
        index += 2;
    }
    let Some(expression_profile) = expression_profile else {
        return Err(InvocationError::InvalidR14Composition(
            "expression_profile_required",
        ));
    };
    if expression_profile != R14_PROFILE {
        return Err(InvocationError::InvalidR14Profile(expression_profile));
    }
    let output_capacity_profile = match output_capacity_profile.as_deref() {
        None => K1OutputCapacityProfile::Standard,
        Some(LOCAL_SNAPSHOT_64M_V1) => K1OutputCapacityProfile::LocalSnapshot64MV1,
        Some(LOCAL_SNAPSHOT_256M_V1) => K1OutputCapacityProfile::LocalSnapshot256MV1,
        Some(_) => {
            return Err(InvocationError::InvalidR14Composition(
                "valid_output_capacity_profile_required",
            ));
        }
    };
    let mut invocation = parse_k1_invocation(&stripped).map_err(|error| match error {
        InvocationError::Input(error) => InvocationError::Input(error),
        InvocationError::InvalidRustCallableProfile(profile) => {
            InvocationError::InvalidR14Composition(if profile.is_empty() {
                "valid_callable_profile_required"
            } else {
                "exact_k1_callable_profile_required"
            })
        }
        _ => InvocationError::InvalidR14Composition("exact_k1_selector_matrix_required"),
    })?;
    if !(invocation.workspace_profile
        && invocation.manifest_profile
        && invocation.rust_semantic_profile
        && invocation.rust_framework_profile
        && invocation.rust_callable_profile
        && !invocation.boundary_profile
        && !invocation.rust_cfg_alternatives_profile
        && !invocation.compiler_index_profile)
    {
        return Err(InvocationError::InvalidR14Composition(
            "r5_r6_k1_source_only_profiles_required",
        ));
    }
    invocation.output_capacity_profile = output_capacity_profile;
    invocation.rust_expression_profile = true;
    Ok(invocation)
}

fn parse_r13_invocation(arguments: &[OsString]) -> Result<Invocation, InvocationError> {
    if arguments
        .get(1)
        .is_none_or(|value| value != OsStr::new("scan"))
        || option_requested(arguments, "--repository-boundary-profile")
        || option_requested(arguments, "--repository-boundary-manifest")
        || option_requested(arguments, "--output-capacity-profile")
        || rust_cfg_alternatives_requested(arguments)
    {
        return Err(InvocationError::InvalidR13Composition(
            "source_only_standard_capacity_without_cfg_or_boundaries",
        ));
    }
    let mut stripped = arguments.iter().take(2).cloned().collect::<Vec<_>>();
    let mut callable_profile = None;
    let mut index = 2;
    while index < arguments.len() {
        let flag = &arguments[index];
        let Some(value) = arguments.get(index + 1) else {
            return Err(InvocationError::InvalidR13Composition(
                "complete_option_pair_required",
            ));
        };
        if flag == OsStr::new("--rust-callable-profile") {
            if callable_profile.is_some() {
                return Err(InvocationError::InvalidR13Composition(
                    "single_callable_profile_required",
                ));
            }
            let Some(value) = value.to_str() else {
                return Err(InvocationError::InvalidR13Profile(
                    "rust_callable_profile",
                    String::new(),
                ));
            };
            if value != K1_PROFILE {
                return Err(InvocationError::InvalidR13Profile(
                    "rust_callable_profile",
                    value.to_owned(),
                ));
            }
            callable_profile = Some(value.to_owned());
        } else {
            stripped.push(flag.clone());
            stripped.push(value.clone());
        }
        index += 2;
    }
    if callable_profile.is_none() {
        return Err(InvocationError::InvalidR13Composition(
            "callable_profile_required",
        ));
    }
    let mut invocation = parse_s4_invocation(stripped).map_err(|error| match error {
        InvocationError::Input(error) => InvocationError::Input(error),
        InvocationError::InvalidCompilerIndexProfile(profile) => {
            InvocationError::InvalidR13Profile("compiler_index_profile", profile)
        }
        _ => InvocationError::InvalidR13Composition("exact_selector_matrix_required"),
    })?;
    if !(invocation.workspace_profile
        && invocation.manifest_profile
        && invocation.rust_semantic_profile
        && invocation.rust_framework_profile
        && invocation.compiler_index_profile
        && invocation.compiler_index_binding.is_some())
    {
        return Err(InvocationError::InvalidR13Composition(
            "r5_r6_r7_k1_profiles_and_binding_required",
        ));
    }
    invocation.rust_callable_profile = true;
    Ok(invocation)
}

#[allow(clippy::too_many_lines)]
fn parse_r12_invocation(arguments: &[OsString]) -> Result<Invocation, InvocationError> {
    if arguments
        .get(1)
        .is_none_or(|value| value != OsStr::new("scan"))
        || option_requested(arguments, "--compiler-index-profile")
        || option_requested(arguments, "--compiler-index-binding")
    {
        return Err(InvocationError::InvalidR12Composition(
            "scan_only_without_compiler_index",
        ));
    }
    let mut stripped = arguments.iter().take(2).cloned().collect::<Vec<_>>();
    let mut semantic_profile = None;
    let mut callable_profile = None;
    let mut output_capacity_profile = None;
    let mut index = 2;
    while index < arguments.len() {
        let flag = &arguments[index];
        let Some(value) = arguments.get(index + 1) else {
            return Err(InvocationError::InvalidR12Composition(
                "complete_option_pair_required",
            ));
        };
        if flag == OsStr::new("--rust-semantic-profile") {
            if semantic_profile.is_some() {
                return Err(InvocationError::InvalidR12Composition(
                    "single_semantic_profile_required",
                ));
            }
            let Some(value) = value.to_str() else {
                return Err(InvocationError::InvalidR12Profile(
                    "rust_semantic_profile",
                    String::new(),
                ));
            };
            if value != R10_PROFILE {
                return Err(InvocationError::InvalidR12Profile(
                    "rust_semantic_profile",
                    value.to_owned(),
                ));
            }
            semantic_profile = Some(value.to_owned());
            stripped.push(flag.clone());
            stripped.push(OsString::from(R5_RUST_SEMANTIC_PROFILE));
        } else if flag == OsStr::new("--rust-callable-profile") {
            if callable_profile.is_some() {
                return Err(InvocationError::InvalidR12Composition(
                    "single_callable_profile_required",
                ));
            }
            let Some(value) = value.to_str() else {
                return Err(InvocationError::InvalidR12Profile(
                    "rust_callable_profile",
                    String::new(),
                ));
            };
            if value != K1_PROFILE {
                return Err(InvocationError::InvalidR12Profile(
                    "rust_callable_profile",
                    value.to_owned(),
                ));
            }
            callable_profile = Some(value.to_owned());
        } else if flag == OsStr::new("--output-capacity-profile") {
            if output_capacity_profile.is_some() {
                return Err(InvocationError::InvalidR12Composition(
                    "single_output_capacity_profile_required",
                ));
            }
            output_capacity_profile = value.to_str().map(str::to_owned);
            if output_capacity_profile.is_none() {
                return Err(InvocationError::InvalidR12Profile(
                    "output_capacity_profile",
                    String::new(),
                ));
            }
        } else {
            stripped.push(flag.clone());
            stripped.push(value.clone());
        }
        index += 2;
    }
    if semantic_profile.is_none() || callable_profile.is_none() {
        return Err(InvocationError::InvalidR12Composition(
            "r10_r6_k1_profiles_required",
        ));
    }
    let output_capacity_profile = match output_capacity_profile.as_deref() {
        None => K1OutputCapacityProfile::Standard,
        Some(LOCAL_SNAPSHOT_64M_V1) => K1OutputCapacityProfile::LocalSnapshot64MV1,
        Some(value) => {
            return Err(InvocationError::InvalidR12Profile(
                "output_capacity_profile",
                value.to_owned(),
            ));
        }
    };
    if option_requested(arguments, "--repository-boundary-manifest")
        && !option_requested(arguments, "--repository-boundary-profile")
    {
        return Err(InvocationError::InvalidBoundaryManifest(
            BoundaryManifestReason::SchemaInvalid,
        ));
    }
    let mut invocation =
        Invocation::parse(stripped, Some("standard-local-s4")).map_err(|error| match error {
            InvocationError::InvalidBoundaryManifest(reason) => {
                InvocationError::InvalidBoundaryManifest(reason)
            }
            InvocationError::InvalidBoundaryProfile => InvocationError::InvalidBoundaryProfile,
            _ => InvocationError::InvalidR12Composition("exact_selector_matrix_required"),
        })?;
    if !(invocation.workspace_profile
        && invocation.manifest_profile
        && invocation.rust_semantic_profile
        && invocation.rust_framework_profile)
    {
        return Err(InvocationError::InvalidR12Composition(
            "workspace_manifest_r10_r6_k1_required",
        ));
    }
    invocation.rust_semantic_profile = false;
    invocation.rust_cfg_alternatives_profile = true;
    invocation.rust_callable_profile = true;
    invocation.output_capacity_profile = output_capacity_profile;
    Ok(invocation)
}

fn parse_r10_invocation(arguments: &[OsString]) -> Result<Invocation, InvocationError> {
    if arguments
        .get(1)
        .is_none_or(|value| value != OsStr::new("scan"))
        || option_requested(arguments, "--rust-framework-profile")
        || option_requested(arguments, "--compiler-index-profile")
        || option_requested(arguments, "--compiler-index-binding")
        || option_requested(arguments, "--rust-callable-profile")
        || option_requested(arguments, "--output-capacity-profile")
    {
        return Err(InvocationError::InvalidRustCfgAlternativesComposition(
            "source_only_lineage_required",
        ));
    }
    let mut stripped = arguments.iter().take(2).cloned().collect::<Vec<_>>();
    let mut selected = None;
    let mut index = 2;
    while index < arguments.len() {
        let flag = &arguments[index];
        let Some(value) = arguments.get(index + 1) else {
            return Err(InvocationError::InvalidRustCfgAlternativesComposition(
                "complete_option_pair_required",
            ));
        };
        if flag == OsStr::new("--rust-semantic-profile") {
            if selected.is_some() {
                return Err(InvocationError::InvalidRustCfgAlternativesComposition(
                    "single_profile_required",
                ));
            }
            let Some(value) = value.to_str() else {
                return Err(InvocationError::InvalidRustCfgAlternativesProfile(
                    String::new(),
                ));
            };
            if value != R10_PROFILE {
                return Err(InvocationError::InvalidRustCfgAlternativesProfile(
                    value.to_owned(),
                ));
            }
            selected = Some(value.to_owned());
            stripped.push(flag.clone());
            stripped.push(OsString::from(R5_RUST_SEMANTIC_PROFILE));
        } else {
            stripped.push(flag.clone());
            stripped.push(value.clone());
        }
        index += 2;
    }
    if selected.is_none() {
        return Err(InvocationError::InvalidRustCfgAlternativesProfile(
            String::new(),
        ));
    }
    let mut invocation =
        Invocation::parse(stripped, Some("standard-local-s4")).map_err(|error| match error {
            InvocationError::InvalidWorkspaceProfile
            | InvocationError::InvalidManifestProfile
            | InvocationError::InvalidRustSemanticProfile(_)
            | InvocationError::InvalidRustSemanticComposition(_)
            | InvocationError::InvalidRustFrameworkProfile(_)
            | InvocationError::InvalidRustFrameworkComposition(_)
            | InvocationError::InvalidRustCallableProfile(_)
            | InvocationError::InvalidRustCallableComposition
            | InvocationError::InvalidCompilerIndexProfile(_)
            | InvocationError::InvalidCompilerIndexComposition(_) => {
                InvocationError::InvalidRustCfgAlternativesComposition("r5_source_lineage_required")
            }
            other => other,
        })?;
    if !(invocation.workspace_profile
        && invocation.manifest_profile
        && invocation.rust_semantic_profile)
    {
        return Err(InvocationError::InvalidRustCfgAlternativesComposition(
            "workspace_and_manifest_profiles_required",
        ));
    }
    invocation.rust_semantic_profile = false;
    invocation.rust_cfg_alternatives_profile = true;
    Ok(invocation)
}

fn parse_k1_invocation(arguments: &[OsString]) -> Result<Invocation, InvocationError> {
    if arguments
        .get(1)
        .is_none_or(|value| value != OsStr::new("scan"))
        || option_requested(arguments, "--compiler-index-profile")
        || option_requested(arguments, "--compiler-index-binding")
    {
        return Err(InvocationError::InvalidRustCallableComposition);
    }
    let mut stripped = arguments.iter().take(2).cloned().collect::<Vec<_>>();
    let mut callable_profile = None;
    let mut output_capacity_profile = None;
    let mut index = 2;
    while index < arguments.len() {
        let flag = &arguments[index];
        let Some(value) = arguments.get(index + 1) else {
            return Err(InvocationError::InvalidRustCallableComposition);
        };
        if flag == OsStr::new("--rust-callable-profile") {
            if callable_profile.is_some() {
                return Err(InvocationError::InvalidRustCallableComposition);
            }
            let Some(value) = value.to_str() else {
                return Err(InvocationError::InvalidRustCallableProfile(String::new()));
            };
            callable_profile = Some(value.to_owned());
        } else if flag == OsStr::new("--output-capacity-profile") {
            if output_capacity_profile.is_some() {
                return Err(InvocationError::InvalidRustCallableComposition);
            }
            let Some(value) = value.to_str() else {
                return Err(InvocationError::InvalidRustCallableComposition);
            };
            output_capacity_profile = Some(value.to_owned());
        } else {
            stripped.push(flag.clone());
            stripped.push(value.clone());
        }
        index += 2;
    }
    let Some(callable_profile) = callable_profile else {
        return Err(InvocationError::InvalidRustCallableComposition);
    };
    if callable_profile != K1_PROFILE {
        return Err(InvocationError::InvalidRustCallableProfile(
            callable_profile,
        ));
    }
    let output_capacity_profile = match output_capacity_profile.as_deref() {
        None => K1OutputCapacityProfile::Standard,
        Some(LOCAL_SNAPSHOT_64M_V1) => K1OutputCapacityProfile::LocalSnapshot64MV1,
        Some(_) => return Err(InvocationError::InvalidRustCallableComposition),
    };
    if option_requested(arguments, "--repository-boundary-manifest")
        && !option_requested(arguments, "--repository-boundary-profile")
    {
        return Err(InvocationError::InvalidBoundaryManifest(
            BoundaryManifestReason::SchemaInvalid,
        ));
    }
    let mut invocation =
        Invocation::parse(stripped, Some("standard-local-s4")).map_err(|error| match error {
            InvocationError::InvalidWorkspaceProfile
            | InvocationError::InvalidManifestProfile
            | InvocationError::InvalidRustSemanticProfile(_)
            | InvocationError::InvalidRustSemanticComposition(_)
            | InvocationError::InvalidRustFrameworkProfile(_)
            | InvocationError::InvalidRustFrameworkComposition(_)
            | InvocationError::InvalidCompilerIndexProfile(_)
            | InvocationError::InvalidCompilerIndexComposition(_) => {
                InvocationError::InvalidRustCallableComposition
            }
            other => other,
        })?;
    if !(invocation.workspace_profile
        && invocation.manifest_profile
        && invocation.rust_semantic_profile
        && invocation.rust_framework_profile)
    {
        return Err(InvocationError::InvalidRustCallableComposition);
    }
    invocation.rust_callable_profile = true;
    invocation.output_capacity_profile = output_capacity_profile;
    Ok(invocation)
}

fn selected_compiler_profiles(arguments: &[OsString]) -> Vec<String> {
    let profile_flags = [
        "--profile",
        "--workspace-profile",
        "--manifest-profile",
        "--rust-semantic-profile",
        "--rust-framework-profile",
        "--compiler-index-profile",
    ];
    arguments
        .iter()
        .skip(2)
        .collect::<Vec<_>>()
        .chunks(2)
        .filter_map(|pair| {
            let [flag, value] = pair else {
                return None;
            };
            profile_flags
                .iter()
                .any(|candidate| flag.as_os_str() == OsStr::new(candidate))
                .then(|| value.to_string_lossy().into_owned())
        })
        .collect()
}

fn required_r7_profiles_selected(selected: &[String]) -> bool {
    [
        "standard-local-s4",
        R3_WORKSPACE_PROFILE,
        R4_MANIFEST_PROFILE,
        R5_RUST_SEMANTIC_PROFILE,
        R6_FRAMEWORK_PROFILE,
        R7_COMPILER_INDEX_PROFILE,
    ]
    .iter()
    .all(|required| selected.iter().any(|value| value == required))
}

#[allow(clippy::struct_excessive_bools)]
struct Invocation {
    repository: OsString,
    identity: RepositoryIdentity,
    revision: Revision,
    store: Option<OsString>,
    packed_sha1: bool,
    workspace_profile: bool,
    manifest_profile: bool,
    rust_semantic_profile: bool,
    rust_cfg_alternatives_profile: bool,
    rust_framework_profile: bool,
    rust_callable_profile: bool,
    rust_expression_profile: bool,
    rust_flow_profile: bool,
    rust_constant_profile: bool,
    output_capacity_profile: K1OutputCapacityProfile,
    compiler_index_profile: bool,
    compiler_index_binding: Option<OsString>,
    boundary_profile: bool,
    boundary_manifest: Option<OsString>,
}

#[derive(Clone)]
enum InvocationError {
    Input(InputError),
    InvalidAcquisitionProfile,
    InvalidWorkspaceProfile,
    InvalidManifestProfile,
    InvalidRustSemanticProfile(String),
    InvalidRustSemanticComposition(&'static str),
    InvalidRustCfgAlternativesProfile(String),
    InvalidRustCfgAlternativesComposition(&'static str),
    InvalidR12Profile(&'static str, String),
    InvalidR12Composition(&'static str),
    InvalidR13Profile(&'static str, String),
    InvalidR13Composition(&'static str),
    InvalidR14Profile(String),
    InvalidR14Composition(&'static str),
    InvalidR15Profile(String),
    InvalidR15Composition(&'static str),
    InvalidR16Profile(String),
    InvalidR16Composition(&'static str),
    InvalidRustFrameworkProfile(String),
    InvalidRustFrameworkComposition(Vec<String>),
    InvalidRustCallableProfile(String),
    InvalidRustCallableComposition,
    InvalidCompilerIndexProfile(String),
    InvalidCompilerIndexComposition(Vec<String>),
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
        let manifest_option_requested = option_requested(&arguments, "--manifest-profile");
        let rust_semantic_option_requested =
            option_requested(&arguments, "--rust-semantic-profile");
        let rust_framework_option_requested =
            option_requested(&arguments, "--rust-framework-profile");
        let mut arguments = arguments.into_iter();
        let _program = arguments.next();
        if arguments.next().as_deref() != Some(OsStr::new(command)) {
            return Err(if rust_framework_option_requested {
                InvocationError::InvalidRustFrameworkComposition(Vec::new())
            } else if rust_semantic_option_requested {
                InvocationError::InvalidRustSemanticComposition("scan_command_required")
            } else if boundary_options_requested {
                InvocationError::InvalidBoundaryProfile
            } else if workspace_option_requested {
                InvocationError::InvalidWorkspaceProfile
            } else if manifest_option_requested {
                InvocationError::InvalidManifestProfile
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
        let mut manifest_profile = None;
        let mut rust_semantic_profile = None;
        let mut rust_framework_profile = None;
        let mut boundary_profile = None;
        let mut boundary_manifest = None;
        while let Some(flag) = arguments.next() {
            let value = arguments.next().ok_or_else(|| {
                if rust_framework_option_requested
                    && matches!(
                        flag.to_str(),
                        Some(
                            "--profile"
                                | "--workspace-profile"
                                | "--manifest-profile"
                                | "--rust-semantic-profile"
                        )
                    )
                {
                    InvocationError::InvalidRustFrameworkComposition(selected_framework_profiles(
                        profile.as_deref(),
                        workspace_profile.as_deref(),
                        manifest_profile.as_deref(),
                        rust_semantic_profile.as_deref(),
                        rust_framework_profile.as_deref(),
                    ))
                } else if flag == OsStr::new("--acquisition-profile") {
                    InvocationError::InvalidAcquisitionProfile
                } else if flag == OsStr::new("--workspace-profile") {
                    InvocationError::InvalidWorkspaceProfile
                } else if flag == OsStr::new("--manifest-profile") {
                    InvocationError::InvalidManifestProfile
                } else if flag == OsStr::new("--rust-semantic-profile") {
                    InvocationError::InvalidRustSemanticProfile(String::new())
                } else if flag == OsStr::new("--rust-framework-profile") {
                    InvocationError::InvalidRustFrameworkProfile(String::new())
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
                        return Err(if rust_framework_option_requested {
                            InvocationError::InvalidRustFrameworkComposition(
                                selected_framework_profiles(
                                    profile.as_deref(),
                                    workspace_profile.as_deref(),
                                    manifest_profile.as_deref(),
                                    rust_semantic_profile.as_deref(),
                                    rust_framework_profile.as_deref(),
                                ),
                            )
                        } else {
                            InvocationError::InvalidWorkspaceProfile
                        });
                    };
                    workspace_profile = Some(value.to_owned());
                }
                Some("--manifest-profile") if manifest_profile.is_none() => {
                    let Some(value) = value.to_str() else {
                        return Err(if rust_framework_option_requested {
                            InvocationError::InvalidRustFrameworkComposition(
                                selected_framework_profiles(
                                    profile.as_deref(),
                                    workspace_profile.as_deref(),
                                    manifest_profile.as_deref(),
                                    rust_semantic_profile.as_deref(),
                                    rust_framework_profile.as_deref(),
                                ),
                            )
                        } else {
                            InvocationError::InvalidManifestProfile
                        });
                    };
                    manifest_profile = Some(value.to_owned());
                }
                Some("--rust-semantic-profile") if rust_semantic_profile.is_none() => {
                    let Some(value) = value.to_str() else {
                        return Err(if rust_framework_option_requested {
                            InvocationError::InvalidRustFrameworkComposition(
                                selected_framework_profiles(
                                    profile.as_deref(),
                                    workspace_profile.as_deref(),
                                    manifest_profile.as_deref(),
                                    rust_semantic_profile.as_deref(),
                                    rust_framework_profile.as_deref(),
                                ),
                            )
                        } else {
                            InvocationError::InvalidRustSemanticProfile(String::new())
                        });
                    };
                    rust_semantic_profile = Some(value.to_owned());
                }
                Some("--rust-framework-profile") if rust_framework_profile.is_none() => {
                    let Some(value) = value.to_str() else {
                        return Err(InvocationError::InvalidRustFrameworkProfile(String::new()));
                    };
                    rust_framework_profile = Some(value.to_owned());
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
                    return Err(if rust_framework_option_requested {
                        InvocationError::InvalidRustFrameworkComposition(
                            selected_framework_profiles(
                                profile.as_deref(),
                                workspace_profile.as_deref(),
                                manifest_profile.as_deref(),
                                rust_semantic_profile.as_deref(),
                                rust_framework_profile.as_deref(),
                            ),
                        )
                    } else {
                        InvocationError::InvalidWorkspaceProfile
                    });
                }
                Some("--manifest-profile") => {
                    return Err(if rust_framework_option_requested {
                        InvocationError::InvalidRustFrameworkComposition(
                            selected_framework_profiles(
                                profile.as_deref(),
                                workspace_profile.as_deref(),
                                manifest_profile.as_deref(),
                                rust_semantic_profile.as_deref(),
                                rust_framework_profile.as_deref(),
                            ),
                        )
                    } else {
                        InvocationError::InvalidManifestProfile
                    });
                }
                Some("--rust-semantic-profile") => {
                    return Err(if rust_framework_option_requested {
                        InvocationError::InvalidRustFrameworkComposition(
                            selected_framework_profiles(
                                profile.as_deref(),
                                workspace_profile.as_deref(),
                                manifest_profile.as_deref(),
                                rust_semantic_profile.as_deref(),
                                rust_framework_profile.as_deref(),
                            ),
                        )
                    } else {
                        InvocationError::InvalidRustSemanticProfile(
                            value.to_string_lossy().into_owned(),
                        )
                    });
                }
                Some("--rust-framework-profile") => {
                    return Err(InvocationError::InvalidRustFrameworkProfile(
                        value.to_string_lossy().into_owned(),
                    ));
                }
                Some("--repository-boundary-profile") => {
                    return Err(InvocationError::InvalidBoundaryProfile);
                }
                Some("--repository-boundary-manifest") => {
                    return Err(InvocationError::InvalidBoundaryManifest(
                        BoundaryManifestReason::SchemaInvalid,
                    ));
                }
                Some("--profile") if rust_framework_option_requested => {
                    return Err(InvocationError::InvalidRustFrameworkComposition(
                        selected_framework_profiles(
                            profile.as_deref(),
                            workspace_profile.as_deref(),
                            manifest_profile.as_deref(),
                            rust_semantic_profile.as_deref(),
                            rust_framework_profile.as_deref(),
                        ),
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
                return Err(if rust_framework_option_requested {
                    InvocationError::InvalidRustFrameworkComposition(selected_framework_profiles(
                        profile.as_deref(),
                        workspace_profile.as_deref(),
                        manifest_profile.as_deref(),
                        rust_semantic_profile.as_deref(),
                        rust_framework_profile.as_deref(),
                    ))
                } else if rust_semantic_option_requested {
                    InvocationError::InvalidRustSemanticComposition(
                        "standard_local_s4_profile_required",
                    )
                } else if boundary_options_requested {
                    InvocationError::InvalidBoundaryProfile
                } else if workspace_option_requested {
                    InvocationError::InvalidWorkspaceProfile
                } else if manifest_option_requested {
                    InvocationError::InvalidManifestProfile
                } else {
                    InputError::InvalidProfile.into()
                });
            }
        } else if profile.is_some() {
            return Err(if rust_framework_option_requested {
                InvocationError::InvalidRustFrameworkComposition(selected_framework_profiles(
                    profile.as_deref(),
                    workspace_profile.as_deref(),
                    manifest_profile.as_deref(),
                    rust_semantic_profile.as_deref(),
                    rust_framework_profile.as_deref(),
                ))
            } else if rust_semantic_option_requested {
                InvocationError::InvalidRustSemanticComposition(
                    "standard_local_s4_profile_required",
                )
            } else if boundary_options_requested {
                InvocationError::InvalidBoundaryProfile
            } else if workspace_option_requested {
                InvocationError::InvalidWorkspaceProfile
            } else if manifest_option_requested {
                InvocationError::InvalidManifestProfile
            } else {
                InputError::InvalidRevision.into()
            });
        }
        let framework_composition = selected_framework_profiles(
            profile.as_deref(),
            workspace_profile.as_deref(),
            manifest_profile.as_deref(),
            rust_semantic_profile.as_deref(),
            rust_framework_profile.as_deref(),
        );
        let rust_framework_profile = match rust_framework_profile.as_deref() {
            None => false,
            Some(R6_FRAMEWORK_PROFILE)
                if command == "scan" && required_profile == Some("standard-local-s4") =>
            {
                true
            }
            Some(provided) => {
                return Err(InvocationError::InvalidRustFrameworkProfile(
                    provided.to_owned(),
                ));
            }
        };
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
            _ if rust_framework_profile => {
                return Err(InvocationError::InvalidRustFrameworkComposition(
                    framework_composition.clone(),
                ));
            }
            _ => return Err(InvocationError::InvalidWorkspaceProfile),
        };
        let rust_semantic_profile = match rust_semantic_profile.as_deref() {
            None => false,
            Some(R5_RUST_SEMANTIC_PROFILE)
                if command == "scan" && required_profile == Some("standard-local-s4") =>
            {
                true
            }
            Some(_) if rust_framework_profile => {
                return Err(InvocationError::InvalidRustFrameworkComposition(
                    framework_composition.clone(),
                ));
            }
            Some(provided) => {
                return Err(InvocationError::InvalidRustSemanticProfile(
                    provided.to_owned(),
                ));
            }
        };
        let manifest_profile = match manifest_profile.as_deref() {
            None => false,
            Some(R4_MANIFEST_PROFILE)
                if command == "scan"
                    && required_profile == Some("standard-local-s4")
                    && (workspace_profile || rust_semantic_profile || rust_framework_profile) =>
            {
                true
            }
            _ if rust_framework_profile => {
                return Err(InvocationError::InvalidRustFrameworkComposition(
                    framework_composition.clone(),
                ));
            }
            _ => return Err(InvocationError::InvalidManifestProfile),
        };
        if rust_framework_profile
            && !(workspace_profile && manifest_profile && rust_semantic_profile)
        {
            return Err(InvocationError::InvalidRustFrameworkComposition(
                framework_composition,
            ));
        }
        if rust_semantic_profile && !workspace_profile {
            return Err(InvocationError::InvalidRustSemanticComposition(
                "cargo_root_package_profile_required",
            ));
        }
        if rust_semantic_profile && !manifest_profile {
            return Err(InvocationError::InvalidRustSemanticComposition(
                "cargo_manifest_facts_profile_required",
            ));
        }
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
            manifest_profile,
            rust_semantic_profile,
            rust_cfg_alternatives_profile: false,
            rust_framework_profile,
            rust_callable_profile: false,
            rust_expression_profile: false,
            rust_flow_profile: false,
            rust_constant_profile: false,
            output_capacity_profile: K1OutputCapacityProfile::Standard,
            compiler_index_profile: false,
            compiler_index_binding: None,
            boundary_profile,
            boundary_manifest,
        })
    }
}

fn selected_framework_profiles(
    profile: Option<&str>,
    workspace_profile: Option<&str>,
    manifest_profile: Option<&str>,
    rust_semantic_profile: Option<&str>,
    rust_framework_profile: Option<&str>,
) -> Vec<String> {
    [
        profile,
        workspace_profile,
        manifest_profile,
        rust_semantic_profile,
        rust_framework_profile,
    ]
    .into_iter()
    .flatten()
    .map(str::to_owned)
    .collect()
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
    fn pt_fr_cfg_001_accepts_fifty_configuration_argument_constructions() {
        for seed in 0..50 {
            let mut pairs = [("--file", "configuration.json"), ("--format", "json")];
            let pair_count = pairs.len();
            pairs.rotate_left(seed % pair_count);
            let mut arguments = vec![OsString::from("noesis"), OsString::from("config")];
            arguments.push(OsString::from("validate"));
            for (flag, value) in pairs {
                arguments.push(OsString::from(flag));
                arguments.push(OsString::from(value));
            }
            let bootstrap = ConfigurationBootstrap::parse(arguments).expect("valid G1 command");
            assert!(bootstrap.validate_requested);
            assert!(bootstrap.g1_requested);
            assert_eq!(
                bootstrap.configuration.as_deref(),
                Some(OsStr::new("configuration.json"))
            );
        }
    }

    #[test]
    fn fr_cfg_001_global_configuration_is_only_a_leading_pair() {
        let bootstrap = ConfigurationBootstrap::parse(
            [
                "noesis",
                "--config",
                "configuration.json",
                "profile",
                "--id",
                "local-experimental-r17",
                "--format",
                "json",
            ]
            .map(OsString::from)
            .to_vec(),
        )
        .expect("leading configuration is valid");
        assert_eq!(bootstrap.arguments[1], "profile");
        assert_eq!(
            bootstrap.configuration.as_deref(),
            Some(OsStr::new("configuration.json"))
        );

        for arguments in [
            vec!["noesis", "--config"],
            vec!["noesis", "--config", "configuration.json"],
            vec!["noesis", "profile", "--config", "configuration.json"],
            vec!["noesis", "config", "validate", "--format", "text"],
            vec![
                "noesis",
                "config",
                "validate",
                "--file",
                "configuration.json",
                "--file",
                "configuration.json",
            ],
            vec![
                "noesis",
                "--config",
                "configuration.json",
                "config",
                "validate",
                "--file",
                "configuration.json",
                "--format",
                "json",
            ],
        ] {
            assert!(
                ConfigurationBootstrap::parse(arguments.into_iter().map(OsString::from).collect())
                    .is_err()
            );
        }
    }

    #[test]
    fn fr_cfg_001_selector_absence_preserves_historical_dispatch_arguments() {
        let arguments = [
            "noesis",
            "profile",
            "--id",
            "local-experimental-r17",
            "--format",
            "json",
        ]
        .map(OsString::from)
        .to_vec();
        let bootstrap = ConfigurationBootstrap::parse(arguments.clone()).expect("historical args");
        assert_eq!(bootstrap.arguments, arguments);
        assert!(bootstrap.configuration.is_none());
        assert!(!bootstrap.validate_requested);
        assert!(!bootstrap.g1_requested);

        let empty = ConfigurationBootstrap::parse(vec![OsString::from("noesis")])
            .expect("historical empty invocation");
        assert_eq!(empty.arguments, [OsString::from("noesis")]);
    }

    #[test]
    fn inv_bnd_001_configuration_maximum_and_plus_one_are_exact() {
        let root = G1TestDirectory::new("configuration-boundary");
        let configuration = root.path().join("configuration.json");
        let mut maximum = codenoesis_contracts::DEFAULT_LOCAL_CLI_CONFIGURATION_V1.to_vec();
        maximum.resize(MAX_LOCAL_CONFIGURATION_BYTES, b' ');
        fs::write(&configuration, &maximum).expect("write maximum configuration");
        assert_eq!(
            read_stable_configuration(&configuration).expect("read maximum configuration"),
            maximum
        );

        maximum.push(b' ');
        fs::write(&configuration, maximum).expect("write oversized configuration");
        let failure = read_stable_configuration(&configuration).unwrap_err();
        assert_eq!(failure.exit_code, 2);
        assert_eq!(failure.error.value()["code"], "configuration.invalid_file");
    }

    #[cfg(unix)]
    #[test]
    fn sec_fr_cfg_001_symlink_configuration_is_rejected() {
        use std::os::unix::fs::symlink;

        let root = G1TestDirectory::new("configuration-symlink");
        let configuration = root.path().join("configuration.json");
        fs::write(
            &configuration,
            codenoesis_contracts::DEFAULT_LOCAL_CLI_CONFIGURATION_V1,
        )
        .expect("write configuration");
        let link = root.path().join("configuration-link.json");
        symlink(&configuration, &link).expect("link configuration");
        let failure = read_stable_configuration(&link).unwrap_err();
        assert_eq!(failure.exit_code, 2);
        assert_eq!(failure.error.value()["code"], "configuration.invalid_file");
    }

    #[test]
    fn pt_fr_cli_007_g0_profile_accepts_fifty_pair_orderings() {
        let pairs = [("--id", "local-experimental-r17"), ("--format", "json")];
        for seed in 0..50 {
            let mut permutation = pairs;
            let distance = seed % permutation.len();
            permutation.rotate_left(distance);
            let mut arguments = vec![OsString::from("noesis"), OsString::from("profile")];
            for (flag, value) in permutation {
                arguments.push(OsString::from(flag));
                arguments.push(OsString::from(value));
            }
            let invocation = ProfileInvocation::parse(&arguments).expect("valid G0 command");
            assert_eq!(invocation.profile_id, "local-experimental-r17");
        }
    }

    #[test]
    fn sec_fr_cli_007_g0_profile_argument_matrix_fails_closed() {
        for arguments in [
            vec!["noesis", "profile"],
            vec![
                "noesis",
                "profile",
                "--id",
                "local-experimental-r17",
                "--format",
                "text",
            ],
            vec![
                "noesis",
                "profile",
                "--target",
                "aarch64-apple-darwin",
                "--format",
                "json",
            ],
            vec![
                "noesis",
                "profile",
                "--id",
                "local-experimental-r17",
                "--id",
                "local-experimental-r17",
            ],
            vec![
                "noesis",
                "profile",
                "--id",
                "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
                "--format",
                "json",
            ],
        ] {
            let arguments = arguments
                .into_iter()
                .map(OsString::from)
                .collect::<Vec<_>>();
            assert!(ProfileInvocation::parse(&arguments).is_err());
        }
    }

    #[test]
    fn pt_nfr_det_001_r17_query_selector_accepts_fifty_argument_permutations() {
        let pairs = [
            ("--store", "store"),
            ("--repository-id", "urn:codenoesis:repository:test"),
            ("--documents", "documents"),
            (
                "--id",
                "urn:codenoesis:entity:blake3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            ),
            ("--context-profile", R17_CONTEXT_PROFILE),
            ("--format", "json"),
        ];
        for seed in 0..50 {
            let mut permutation = pairs;
            for index in 0..permutation.len() {
                let target = (seed * 17 + index * 11 + 3) % permutation.len();
                permutation.swap(index, target);
            }
            let mut arguments = vec![OsString::from("noesis"), OsString::from("query")];
            for (flag, value) in permutation {
                arguments.push(OsString::from(flag));
                arguments.push(OsString::from(value));
            }
            let Ok(invocation) = QueryInvocation::parse(arguments) else {
                panic!("valid R17 query selector");
            };
            assert_eq!(
                invocation.context_profile.as_deref(),
                Some(R17_CONTEXT_PROFILE)
            );
        }
    }

    #[test]
    fn pt_nfr_det_001_r18_source_accepts_fifty_argument_permutations() {
        let evidence_id = format!("urn:codenoesis:evidence:blake3:{}", "a".repeat(64));
        let pairs = [
            ("--repository", "repository"),
            ("--revision", "1111111111111111111111111111111111111111"),
            ("--repository-id", "urn:codenoesis:repository:test"),
            ("--store", "store"),
            ("--evidence-id", evidence_id.as_str()),
            ("--source-profile", R18_SOURCE_PROFILE),
            ("--acquisition-profile", LOCAL_GIT_SHA1_PACKED_V1),
            ("--repository-boundary-profile", LOCAL_GITLINKS_V1),
            ("--format", "json"),
        ];
        for seed in 0..50 {
            let mut permutation = pairs;
            for index in 0..permutation.len() {
                let target = (seed * 19 + index * 13 + 5) % permutation.len();
                permutation.swap(index, target);
            }
            let mut arguments = vec![OsString::from("noesis"), OsString::from("source")];
            for (flag, value) in permutation {
                arguments.push(OsString::from(flag));
                arguments.push(OsString::from(value));
            }
            let Ok(invocation) = SourceInvocation::parse(arguments) else {
                panic!("valid R18 source selector");
            };
            assert!(invocation.packed_sha1);
            assert!(invocation.boundary_profile);
            assert_eq!(invocation.evidence_id, evidence_id);
        }
    }

    #[test]
    fn sec_fr_cli_011_r18_source_argument_matrix_fails_closed() {
        let evidence_id = format!("urn:codenoesis:evidence:blake3:{}", "a".repeat(64));
        let valid = vec![
            "noesis".to_owned(),
            "source".to_owned(),
            "--repository".to_owned(),
            "repository".to_owned(),
            "--revision".to_owned(),
            "1111111111111111111111111111111111111111".to_owned(),
            "--repository-id".to_owned(),
            "urn:codenoesis:repository:test".to_owned(),
            "--store".to_owned(),
            "store".to_owned(),
            "--evidence-id".to_owned(),
            evidence_id,
            "--source-profile".to_owned(),
            R18_SOURCE_PROFILE.to_owned(),
            "--format".to_owned(),
            "json".to_owned(),
        ];
        assert!(SourceInvocation::parse(valid.iter().map(OsString::from)).is_ok());

        for mutation in [
            vec!["--revision", "refs/heads/main"],
            vec!["--revision", "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"],
            vec!["--evidence-id", "not-an-evidence-id"],
            vec![
                "--evidence-id",
                "urn:codenoesis:evidence:blake3:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            ],
            vec!["--source-profile", "trusted-local-source-v2"],
            vec!["--format", "text"],
            vec!["--acquisition-profile", "unknown"],
            vec!["--repository-boundary-profile", "unknown"],
        ] {
            let mut arguments = valid.clone();
            let index = arguments
                .iter()
                .position(|argument| argument == mutation[0])
                .unwrap_or_else(|| {
                    arguments.push(mutation[0].to_owned());
                    arguments.push(String::new());
                    arguments.len() - 2
                });
            arguments[index + 1] = mutation[1].to_owned();
            assert!(SourceInvocation::parse(arguments.into_iter().map(OsString::from)).is_err());
        }

        let mut duplicate = valid.clone();
        duplicate.extend(["--store".to_owned(), "store".to_owned()]);
        assert!(SourceInvocation::parse(duplicate.into_iter().map(OsString::from)).is_err());

        let mut forbidden = valid;
        forbidden.extend([
            "--repository-boundary-manifest".to_owned(),
            "boundaries.json".to_owned(),
        ]);
        assert!(SourceInvocation::parse(forbidden.into_iter().map(OsString::from)).is_err());

        for incomplete in [
            vec!["noesis", "source"],
            vec!["noesis", "source", "--repository"],
            vec!["noesis", "source", "--unknown", "value"],
        ] {
            assert!(SourceInvocation::parse(incomplete.into_iter().map(OsString::from)).is_err());
        }
    }

    #[test]
    fn sec_fr_ctx_001_r17_selector_matrix_fails_closed() {
        let valid = [
            "noesis",
            "query",
            "--store",
            "store",
            "--repository-id",
            "urn:codenoesis:repository:test",
            "--documents",
            "documents",
            "--id",
            "urn:codenoesis:entity:blake3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "--context-profile",
            R17_CONTEXT_PROFILE,
            "--format",
            "json",
        ]
        .map(OsString::from);
        assert!(QueryInvocation::parse(valid).is_ok());

        let invalid = [
            "noesis",
            "query",
            "--store",
            "store",
            "--repository-id",
            "urn:codenoesis:repository:test",
            "--documents",
            "documents",
            "--id",
            "urn:codenoesis:entity:blake3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "--context-profile",
            "rust-function-context-v2",
            "--format",
            "json",
        ]
        .map(OsString::from);
        assert!(matches!(
            QueryInvocation::parse(invalid),
            Err(Failure::R16(_))
        ));

        let explorer = [
            "noesis",
            "explore",
            "--input",
            "portable-graph.json",
            "--output",
            "explorer",
            "--explorer-profile",
            R17_CONTEXT_PROFILE,
            "--format",
            "json",
        ]
        .map(OsString::from);
        let Ok(explorer) = ExploreInvocation::parse(explorer) else {
            panic!("valid R17 explorer selector");
        };
        assert_eq!(explorer.profile, PortableProfile::R17);

        let export = [
            "noesis",
            "export",
            "--store",
            "store",
            "--repository-id",
            "urn:codenoesis:repository:test",
            "--output",
            "portable",
            "--portable-profile",
            R17_CONTEXT_PROFILE,
            "--format",
            "json",
        ]
        .map(OsString::from);
        assert!(matches!(
            ExportInvocation::parse(export),
            Err(Failure::R16(_))
        ));
    }

    #[test]
    fn conf_nfr_per_001_benchmark_execution_limit_profile_is_exact() {
        let exact = r16_b1a_arguments();
        let invocation = parse_s4_invocation(exact.clone()).expect("valid B1a selector matrix");
        assert!(invocation.packed_sha1);
        assert!(invocation.rust_constant_profile);
        assert_eq!(
            invocation.execution_limit_profile,
            ScanExecutionLimitProfile::RealWorldRustBenchmark75sV1
        );

        let mut absent = exact.clone();
        remove_option(&mut absent, "--execution-limit-profile");
        assert_eq!(
            parse_s4_invocation(absent)
                .expect("selector absence remains valid")
                .execution_limit_profile,
            ScanExecutionLimitProfile::Standard
        );

        let mut unknown = exact.clone();
        replace_option_value(&mut unknown, "--execution-limit-profile", "unknown");
        assert!(parse_s4_invocation(unknown).is_err());

        let mut duplicate = exact.clone();
        duplicate.extend([
            OsString::from("--execution-limit-profile"),
            OsString::from(B1A_EXECUTION_LIMIT_PROFILE),
        ]);
        assert!(parse_s4_invocation(duplicate).is_err());

        let mut incomplete = exact.clone();
        remove_option(&mut incomplete, "--execution-limit-profile");
        incomplete.push(OsString::from("--execution-limit-profile"));
        assert!(parse_s4_invocation(incomplete).is_err());

        let mut unpacked = exact.clone();
        remove_option(&mut unpacked, "--acquisition-profile");
        assert!(parse_s4_invocation(unpacked).is_err());

        let mut wrong_capacity = exact.clone();
        replace_option_value(
            &mut wrong_capacity,
            "--output-capacity-profile",
            LOCAL_SNAPSHOT_64M_V1,
        );
        assert!(parse_s4_invocation(wrong_capacity).is_err());

        let mut boundary = exact;
        boundary.extend([
            OsString::from("--repository-boundary-profile"),
            OsString::from(LOCAL_GITLINKS_V1),
        ]);
        assert!(matches!(
            parse_s4_invocation(boundary),
            Err(InvocationError::InvalidR16Composition(
                "repository_boundary_not_supported"
            ))
        ));
    }

    #[test]
    fn inv_bnd_001_benchmark_execution_limit_boundary_is_exact() {
        assert_eq!(
            ScanExecutionLimitProfile::Standard.maximum_milliseconds(),
            60_000
        );
        assert_eq!(
            ScanExecutionLimitProfile::RealWorldRustBenchmark75sV1.maximum_milliseconds(),
            75_000
        );
        assert!(check_scan_wall_milliseconds(75_000, 75_000).is_ok());

        let failure = check_scan_wall_milliseconds(75_001, 75_000).unwrap_err();
        assert!(matches!(
            failure,
            Failure::Scan(ScanError::Acquisition(AcquisitionError::LimitExceeded {
                limit: LimitKind::ScanWallMilliseconds,
                maximum: 75_000,
                observed: 75_001,
            }))
        ));
    }

    #[test]
    fn conf_fr_cli_001_r14_r15_256m_profile_has_an_exact_selector_matrix() {
        let r14 = r14_256m_arguments();
        let Ok(invocation) = parse_s4_invocation(r14.clone()) else {
            panic!("valid R14 256 MiB selector");
        };
        assert!(invocation.rust_expression_profile);
        assert!(!invocation.rust_flow_profile);
        assert_eq!(
            invocation.output_capacity_profile,
            K1OutputCapacityProfile::LocalSnapshot256MV1
        );

        let mut r15 = r14.clone();
        r15.extend([
            OsString::from("--rust-flow-profile"),
            OsString::from(R15_PROFILE),
        ]);
        let Ok(invocation) = parse_s4_invocation(r15) else {
            panic!("valid R15 256 MiB selector");
        };
        assert!(invocation.rust_expression_profile);
        assert!(invocation.rust_flow_profile);
        assert_eq!(
            invocation.output_capacity_profile,
            K1OutputCapacityProfile::LocalSnapshot256MV1
        );

        let mut unknown = r14.clone();
        replace_option_value(&mut unknown, "--output-capacity-profile", "unknown");
        assert!(parse_s4_invocation(unknown).is_err());

        let mut duplicate = r14.clone();
        duplicate.extend([
            OsString::from("--output-capacity-profile"),
            OsString::from(LOCAL_SNAPSHOT_256M_V1),
        ]);
        assert!(parse_s4_invocation(duplicate).is_err());

        let mut incomplete = r14.clone();
        remove_option(&mut incomplete, "--output-capacity-profile");
        incomplete.push(OsString::from("--output-capacity-profile"));
        assert!(parse_s4_invocation(incomplete).is_err());

        let mut k1 = r14.clone();
        remove_option(&mut k1, "--rust-expression-profile");
        assert!(parse_s4_invocation(k1).is_err());

        let mut boundary = r14.clone();
        boundary.extend([
            OsString::from("--repository-boundary-profile"),
            OsString::from(LOCAL_GITLINKS_V1),
        ]);
        assert!(parse_s4_invocation(boundary).is_err());

        let mut r10 = r14.clone();
        remove_option(&mut r10, "--rust-expression-profile");
        remove_option(&mut r10, "--rust-callable-profile");
        remove_option(&mut r10, "--rust-framework-profile");
        replace_option_value(&mut r10, "--rust-semantic-profile", R10_PROFILE);
        assert!(parse_s4_invocation(r10).is_err());

        let mut r11 = r14.clone();
        remove_option(&mut r11, "--rust-expression-profile");
        r11.extend([
            OsString::from("--repository-boundary-profile"),
            OsString::from(LOCAL_GITLINKS_V1),
        ]);
        assert!(parse_s4_invocation(r11).is_err());

        let mut r12 = r14.clone();
        remove_option(&mut r12, "--rust-expression-profile");
        replace_option_value(&mut r12, "--rust-semantic-profile", R10_PROFILE);
        assert!(parse_s4_invocation(r12).is_err());

        let mut compiler = r14.clone();
        remove_option(&mut compiler, "--rust-expression-profile");
        compiler.extend([
            OsString::from("--compiler-index-profile"),
            OsString::from(R7_COMPILER_INDEX_PROFILE),
            OsString::from("--compiler-index-binding"),
            OsString::from("fixture.scip"),
        ]);
        assert!(parse_s4_invocation(compiler).is_err());

        let mut non_scan = r14;
        non_scan[1] = OsString::from("query");
        assert!(parse_s4_invocation(non_scan).is_err());
    }

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

    fn r14_256m_arguments() -> Vec<OsString> {
        [
            "noesis",
            "scan",
            "--repository",
            ".",
            "--repository-id",
            "urn:codenoesis:test:r14-r15-output-capacity",
            "--revision",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "--profile",
            "standard-local-s4",
            "--workspace-profile",
            R3_WORKSPACE_PROFILE,
            "--manifest-profile",
            R4_MANIFEST_PROFILE,
            "--rust-semantic-profile",
            R5_RUST_SEMANTIC_PROFILE,
            "--rust-framework-profile",
            R6_FRAMEWORK_PROFILE,
            "--rust-callable-profile",
            K1_PROFILE,
            "--rust-expression-profile",
            R14_PROFILE,
            "--output-capacity-profile",
            LOCAL_SNAPSHOT_256M_V1,
            "--store",
            "store",
            "--format",
            "json",
        ]
        .map(OsString::from)
        .to_vec()
    }

    fn r16_b1a_arguments() -> Vec<OsString> {
        let mut arguments = r14_256m_arguments();
        arguments.extend([
            OsString::from("--acquisition-profile"),
            OsString::from(LOCAL_GIT_SHA1_PACKED_V1),
            OsString::from("--rust-flow-profile"),
            OsString::from(R15_PROFILE),
            OsString::from("--rust-constant-profile"),
            OsString::from(R16_PROFILE),
            OsString::from("--execution-limit-profile"),
            OsString::from(B1A_EXECUTION_LIMIT_PROFILE),
        ]);
        arguments
    }

    fn replace_option_value(arguments: &mut [OsString], flag: &str, value: &str) {
        let index = arguments
            .iter()
            .position(|argument| argument == flag)
            .expect("reviewed option");
        arguments[index + 1] = OsString::from(value);
    }

    fn remove_option(arguments: &mut Vec<OsString>, flag: &str) {
        let index = arguments
            .iter()
            .position(|argument| argument == flag)
            .expect("reviewed option");
        arguments.drain(index..=index + 1);
    }

    struct G1TestDirectory {
        path: std::path::PathBuf,
    }

    impl G1TestDirectory {
        fn new(label: &str) -> Self {
            static SEQUENCE: AtomicU64 = AtomicU64::new(0);
            let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = env::temp_dir().join(format!(
                "codenoesis-g1-{label}-{}-{sequence}",
                std::process::id()
            ));
            if path.exists() {
                fs::remove_dir_all(&path).expect("remove stale G1 test root");
            }
            fs::create_dir(&path).expect("create G1 test root");
            Self { path }
        }

        fn path(&self) -> &std::path::Path {
            &self.path
        }
    }

    impl Drop for G1TestDirectory {
        fn drop(&mut self) {
            if self.path.exists() {
                fs::remove_dir_all(&self.path).expect("remove G1 test root");
            }
        }
    }
}
