//! `CodeNoesis` command-line entry point.

use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{self, Write};
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use codenoesis_application::{ScanError, ScanRequest, ScanService};
use codenoesis_contracts::{
    CodeNoesisErrorV1, CodeNoesisErrorV2, RepositorySnapshotV2Error, SnapshotEnvelopeV1,
};
use codenoesis_domain::{
    InputError, LimitKind, RepositoryIdentity, Revision, STANDARD_LOCAL_S1_LIMITS, limit_exceeded,
};
use codenoesis_repository::LocalGitRepository;

static CORRELATION_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn main() -> ExitCode {
    if noesis::install_s0_security_boundary().is_err() {
        return emit_internal_error_v1();
    }
    let arguments = env::args_os().collect::<Vec<_>>();
    let s1_requested = arguments
        .iter()
        .any(|argument| argument == OsStr::new("--profile"));
    let result = if s1_requested {
        run_s1(arguments)
    } else {
        run_s0(arguments)
    };
    match result {
        Ok(stdout) => match io::stdout().lock().write_all(&stdout) {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) if s1_requested => emit_internal_error_v2(),
            Err(_) => emit_internal_error_v1(),
        },
        Err(Failure::Input(error)) if s1_requested => {
            emit_error_v2(&CodeNoesisErrorV2::from_input(error), 2)
        }
        Err(Failure::Input(error)) => emit_error_v1(&CodeNoesisErrorV1::from_input(error), 2),
        Err(Failure::Scan(ScanError::Acquisition(error))) if s1_requested => {
            emit_error_v2(&CodeNoesisErrorV2::from_acquisition(&error), 10)
        }
        Err(Failure::Scan(ScanError::Acquisition(error))) => {
            emit_error_v1(&CodeNoesisErrorV1::from_acquisition(&error), 10)
        }
        Err(Failure::Scan(ScanError::Internal) | Failure::Internal) if s1_requested => {
            emit_internal_error_v2()
        }
        Err(Failure::Scan(ScanError::Internal) | Failure::Internal) => emit_internal_error_v1(),
    }
}

fn run_s0(arguments: impl IntoIterator<Item = OsString>) -> Result<Vec<u8>, Failure> {
    let invocation = Invocation::parse(arguments, false).map_err(Failure::Input)?;
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
    let invocation = Invocation::parse(arguments, true).map_err(Failure::Input)?;
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

enum Failure {
    Input(InputError),
    Scan(ScanError),
    Internal,
}

struct Invocation {
    repository: OsString,
    identity: RepositoryIdentity,
    revision: Revision,
}

impl Invocation {
    fn parse(
        arguments: impl IntoIterator<Item = OsString>,
        require_s1_profile: bool,
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
        while let Some(flag) = arguments.next() {
            let value = arguments.next().ok_or_else(|| {
                if flag == OsStr::new("--profile") {
                    InputError::InvalidProfile
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
        if require_s1_profile {
            if profile.as_deref() != Some("standard-local-s1") {
                return Err(InputError::InvalidProfile);
            }
        } else if profile.is_some() {
            return Err(InputError::InvalidRevision);
        }
        if format.as_deref() != Some("json") {
            return Err(InputError::InvalidRevision);
        }
        Ok(Self {
            repository,
            identity,
            revision,
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
