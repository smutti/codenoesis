use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

use codenoesis_application::{
    TrustedImpactSourceRequest, TrustedImpactSourceRetrievalError,
    TrustedImpactSourceRetrievalService,
};
use codenoesis_contracts::{
    CodeNoesisErrorV30, ImpactSourceSelectionV1, MAX_R19_REPORT_BYTES, R19_SOURCE_PROFILE,
};
use codenoesis_domain::s1_packed::LOCAL_GIT_SHA1_PACKED_V1;
use codenoesis_repository::LocalGitRepository;

use crate::impact_git::{canonical_file, read_stable_input, sha256};

pub(crate) struct ImpactSourceFailure {
    pub(crate) error: CodeNoesisErrorV30,
    pub(crate) exit_code: u8,
}

pub(crate) fn requested(arguments: &[OsString]) -> bool {
    arguments
        .get(1)
        .is_some_and(|value| value == "impact-source")
}

pub(crate) fn run(arguments: Vec<OsString>) -> Result<Vec<u8>, ImpactSourceFailure> {
    let invocation = ImpactSourceInvocation::parse(arguments)?;
    let report_path = canonical_file(&invocation.report, "report").map_err(map_input_failure)?;
    let repository = canonical_repository(&invocation.repository)?;
    noesis::install_s6_filesystem_boundary(
        report_path.as_os_str(),
        std::slice::from_ref(&repository),
    )
    .map_err(|_| internal_failure())?;
    let report = read_stable_input(&report_path, MAX_R19_REPORT_BYTES, "report")
        .map_err(map_input_failure)?;
    let selection = ImpactSourceSelectionV1::from_report(
        &report,
        &invocation.evidence_id,
        &invocation.repository_identity,
        &invocation.revision,
        sha256,
    )
    .map_err(|error| operation_failure(CodeNoesisErrorV30::from_source(&error)))?;
    let request = TrustedImpactSourceRequest::new(repository.into_os_string(), selection);
    let excerpt = if invocation.packed {
        TrustedImpactSourceRetrievalService::new(LocalGitRepository::new_packed_sha1())
            .retrieve(&request, sha256)
    } else {
        TrustedImpactSourceRetrievalService::new(LocalGitRepository::new())
            .retrieve(&request, sha256)
    }
    .map_err(retrieval_failure)?;
    Ok(excerpt.canonical_stdout())
}

struct ImpactSourceInvocation {
    repository: PathBuf,
    repository_identity: String,
    revision: String,
    report: PathBuf,
    evidence_id: String,
    packed: bool,
}

impl ImpactSourceInvocation {
    fn parse(arguments: Vec<OsString>) -> Result<Self, ImpactSourceFailure> {
        let mut arguments = arguments.into_iter();
        let _binary = arguments.next();
        if arguments.next().as_deref() != Some(OsStr::new("impact-source")) {
            return Err(input_failure(CodeNoesisErrorV30::source_invalid_arguments()));
        }
        let mut repository = None;
        let mut repository_identity = None;
        let mut revision = None;
        let mut report = None;
        let mut evidence_id = None;
        let mut source_profile = None;
        let mut acquisition_profile = None;
        let mut format = None;
        while let Some(flag) = arguments.next() {
            let Some(value) = arguments.next() else {
                return Err(input_failure(CodeNoesisErrorV30::source_invalid_arguments()));
            };
            match flag.to_str() {
                Some("--repository") if repository.is_none() => {
                    repository = Some(PathBuf::from(value));
                }
                Some("--repository-id") if repository_identity.is_none() => {
                    repository_identity = value.into_string().ok();
                }
                Some("--revision") if revision.is_none() => revision = value.into_string().ok(),
                Some("--report") if report.is_none() => report = Some(PathBuf::from(value)),
                Some("--evidence-id") if evidence_id.is_none() => {
                    evidence_id = value.into_string().ok();
                }
                Some("--source-profile") if source_profile.is_none() => {
                    source_profile = value.into_string().ok();
                }
                Some("--acquisition-profile") if acquisition_profile.is_none() => {
                    acquisition_profile = value.into_string().ok();
                }
                Some("--format") if format.is_none() => format = value.into_string().ok(),
                _ => {
                    return Err(input_failure(CodeNoesisErrorV30::source_invalid_arguments()));
                }
            }
        }
        if source_profile.as_deref() != Some(R19_SOURCE_PROFILE)
            || format.as_deref() != Some("json")
            || acquisition_profile
                .as_deref()
                .is_some_and(|profile| profile != LOCAL_GIT_SHA1_PACKED_V1)
        {
            return Err(input_failure(CodeNoesisErrorV30::source_invalid_arguments()));
        }
        Ok(Self {
            repository: required_path(repository)?,
            repository_identity: required_string(repository_identity)?,
            revision: required_string(revision)?,
            report: required_path(report)?,
            evidence_id: required_string(evidence_id)?,
            packed: acquisition_profile.is_some(),
        })
    }
}

fn required_path(value: Option<PathBuf>) -> Result<PathBuf, ImpactSourceFailure> {
    value
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| input_failure(CodeNoesisErrorV30::source_invalid_arguments()))
}

fn required_string(value: Option<String>) -> Result<String, ImpactSourceFailure> {
    value
        .filter(|value| !value.is_empty())
        .ok_or_else(|| input_failure(CodeNoesisErrorV30::source_invalid_arguments()))
}

fn canonical_repository(path: &Path) -> Result<PathBuf, ImpactSourceFailure> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| input_failure(CodeNoesisErrorV30::source_invalid_arguments()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(input_failure(CodeNoesisErrorV30::source_invalid_arguments()));
    }
    fs::canonicalize(path)
        .map_err(|_| input_failure(CodeNoesisErrorV30::source_invalid_arguments()))
}

fn retrieval_failure(error: TrustedImpactSourceRetrievalError) -> ImpactSourceFailure {
    match error {
        TrustedImpactSourceRetrievalError::Repository(_) => {
            operation_failure(CodeNoesisErrorV30::source_acquisition_rejected())
        }
        TrustedImpactSourceRetrievalError::Contract(error) => {
            operation_failure(CodeNoesisErrorV30::from_source(&error))
        }
    }
}

fn map_input_failure(failure: crate::impact_git::ImpactGitFailure) -> ImpactSourceFailure {
    ImpactSourceFailure {
        error: failure.error,
        exit_code: failure.exit_code,
    }
}

fn input_failure(error: CodeNoesisErrorV30) -> ImpactSourceFailure {
    ImpactSourceFailure {
        error,
        exit_code: 2,
    }
}

fn operation_failure(error: CodeNoesisErrorV30) -> ImpactSourceFailure {
    ImpactSourceFailure {
        error,
        exit_code: 2,
    }
}

fn internal_failure() -> ImpactSourceFailure {
    ImpactSourceFailure {
        error: CodeNoesisErrorV30::source_internal(),
        exit_code: 1,
    }
}
