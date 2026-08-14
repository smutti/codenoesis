//! Deterministic unsigned local CLI distribution staging.

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use codenoesis_contracts::{
    CodeNoesisErrorV26, DistributionFileMode, LocalDistributionFileV1, LocalDistributionManifestV1,
    MAX_LOCAL_DISTRIBUTION_BINARY_BYTES, current_local_distribution_target,
    local_distribution_bundle_name,
};
use same_file::Handle as FileIdentity;
use sha2::{Digest as _, Sha256};

const DEFAULT_CONFIGURATION: &[u8] =
    include_bytes!("../../distribution/local-cli/default-config.json");
const CONFIGURATION_SCHEMA: &[u8] =
    include_bytes!("../../distribution/local-cli/local-cli-config-v1.schema.json");
const INSTALLATION_GUIDE: &[u8] = include_bytes!("../../distribution/local-cli/INSTALL.md");
const LICENSE: &[u8] = include_bytes!("../../LICENSE");
const MAX_STAGING_ATTEMPTS: u64 = 32;

static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// One strict command failure and its public process exit status.
#[derive(Clone, Debug)]
pub struct DistributionFailure {
    error: CodeNoesisErrorV26,
    exit_code: u8,
}

impl DistributionFailure {
    /// Returns an internal packaging failure.
    #[must_use]
    pub fn internal() -> Self {
        Self::new(CodeNoesisErrorV26::internal(), 1)
    }

    /// Returns the strict public error contract.
    #[must_use]
    pub const fn error(&self) -> &CodeNoesisErrorV26 {
        &self.error
    }

    /// Returns the public process exit status.
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        self.exit_code
    }

    const fn new(error: CodeNoesisErrorV26, exit_code: u8) -> Self {
        Self { error, exit_code }
    }
}

/// Stages one deterministic current-target local CLI bundle.
///
/// # Errors
///
/// Returns a strict distribution input or internal failure without publishing
/// a partial final bundle.
pub fn run(arguments: impl IntoIterator<Item = OsString>) -> Result<Vec<u8>, DistributionFailure> {
    let arguments = arguments.into_iter().collect::<Vec<_>>();
    let invocation = Invocation::parse(&arguments)?;
    let target = current_local_distribution_target();
    if target == "unsupported-compile-target" {
        return Err(invalid_binary());
    }
    let output = OutputRoot::validate(&invocation.output)?;
    output.ensure_empty()?;
    let binary = StableBinary::inspect(&invocation.binary)?;
    output.ensure_empty()?;

    let binary_path = if target == "x86_64-pc-windows-msvc" {
        "bin/noesis.exe"
    } else {
        "bin/noesis"
    };
    let payloads = data_payloads();
    let manifest = LocalDistributionManifestV1::new(
        target,
        &binary.sha256,
        &payload_records(binary_path, &binary, &payloads),
    )
    .map_err(|_| DistributionFailure::internal())?;
    let manifest_bytes = manifest
        .canonical_stdout()
        .map_err(|_| DistributionFailure::internal())?;
    let final_name = local_distribution_bundle_name(target, &binary.sha256);
    let final_path = output.path.join(final_name);
    ensure_absent(&final_path)?;

    let mut staging = StagingDirectory::create(&output)?;
    binary.write_to(&staging.path().join(binary_path))?;
    write_payloads(staging.path(), &payloads)?;
    write_file(
        &staging.path().join("manifest.json"),
        &manifest_bytes,
        DistributionFileMode::Data,
    )?;
    validate_staging(
        staging.path(),
        binary_path,
        &binary,
        &payloads,
        &manifest_bytes,
    )?;
    output.validate_identity()?;
    ensure_absent(&final_path)?;
    fs::rename(staging.path(), &final_path).map_err(|_| DistributionFailure::internal())?;
    staging.disarm();
    output.validate_identity()?;

    Ok(manifest_bytes)
}

struct Invocation {
    binary: PathBuf,
    output: PathBuf,
}

impl Invocation {
    fn parse(arguments: &[OsString]) -> Result<Self, DistributionFailure> {
        if arguments.len() != 6
            || arguments
                .get(1)
                .is_none_or(|argument| argument != "package-local-cli")
        {
            return Err(invalid_arguments());
        }
        let mut binary = None;
        let mut output = None;
        for pair in arguments[2..].chunks_exact(2) {
            let flag = pair[0].to_str().ok_or_else(invalid_arguments)?;
            if pair[1].is_empty() {
                return Err(invalid_arguments());
            }
            match flag {
                "--binary" if binary.is_none() => binary = Some(PathBuf::from(&pair[1])),
                "--output" if output.is_none() => output = Some(PathBuf::from(&pair[1])),
                _ => return Err(invalid_arguments()),
            }
        }
        Ok(Self {
            binary: binary.ok_or_else(invalid_arguments)?,
            output: output.ok_or_else(invalid_arguments)?,
        })
    }
}

struct OutputRoot {
    path: PathBuf,
    identity: FileIdentity,
}

impl OutputRoot {
    fn validate(path: &Path) -> Result<Self, DistributionFailure> {
        let metadata = fs::symlink_metadata(path).map_err(|_| invalid_arguments())?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(invalid_arguments());
        }
        let identity = FileIdentity::from_path(path).map_err(|_| invalid_arguments())?;
        Ok(Self {
            path: path.to_path_buf(),
            identity,
        })
    }

    fn ensure_empty(&self) -> Result<(), DistributionFailure> {
        self.validate_identity()?;
        if fs::read_dir(&self.path)
            .map_err(|_| invalid_arguments())?
            .next()
            .transpose()
            .map_err(|_| invalid_arguments())?
            .is_some()
        {
            return Err(output_exists());
        }
        self.validate_identity()
    }

    fn validate_identity(&self) -> Result<(), DistributionFailure> {
        let metadata =
            fs::symlink_metadata(&self.path).map_err(|_| DistributionFailure::internal())?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(DistributionFailure::internal());
        }
        let identity =
            FileIdentity::from_path(&self.path).map_err(|_| DistributionFailure::internal())?;
        if identity != self.identity {
            return Err(DistributionFailure::internal());
        }
        Ok(())
    }
}

struct StagingDirectory {
    path: PathBuf,
    armed: bool,
}

impl StagingDirectory {
    fn create(output: &OutputRoot) -> Result<Self, DistributionFailure> {
        for _ in 0..MAX_STAGING_ATTEMPTS {
            output.validate_identity()?;
            let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = output.path.join(format!(
                ".codenoesis-staging-{}-{sequence}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => {
                    let staging = Self { path, armed: true };
                    set_private_directory(staging.path())?;
                    return Ok(staging);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(_) => return Err(DistributionFailure::internal()),
            }
        }
        Err(DistributionFailure::internal())
    }

    fn path(&self) -> &Path {
        &self.path
    }

    const fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for StagingDirectory {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

struct Payload {
    path: &'static str,
    bytes: &'static [u8],
    sha256: String,
}

fn data_payloads() -> Vec<Payload> {
    vec![
        data_payload("etc/codenoesis/config.json", DEFAULT_CONFIGURATION),
        data_payload(
            "share/codenoesis/schemas/local-cli-config-v1.schema.json",
            CONFIGURATION_SCHEMA,
        ),
        data_payload("share/doc/codenoesis/INSTALL.md", INSTALLATION_GUIDE),
        data_payload("share/doc/codenoesis/LICENSE", LICENSE),
    ]
}

fn data_payload(path: &'static str, bytes: &'static [u8]) -> Payload {
    Payload {
        path,
        bytes,
        sha256: sha256_hex(bytes),
    }
}

fn payload_records(
    binary_path: &str,
    binary: &StableBinary,
    payloads: &[Payload],
) -> Vec<LocalDistributionFileV1> {
    let mut records = vec![LocalDistributionFileV1::new(
        binary_path,
        binary.length,
        &binary.sha256,
        DistributionFileMode::Executable,
    )];
    records.extend(payloads.iter().map(|payload| {
        LocalDistributionFileV1::new(
            payload.path,
            u64::try_from(payload.bytes.len()).unwrap_or(u64::MAX),
            &payload.sha256,
            DistributionFileMode::Data,
        )
    }));
    records
}

fn write_payloads(root: &Path, payloads: &[Payload]) -> Result<(), DistributionFailure> {
    for payload in payloads {
        let path = root.join(payload.path);
        let parent = path.parent().ok_or_else(DistributionFailure::internal)?;
        fs::create_dir_all(parent).map_err(|_| DistributionFailure::internal())?;
        write_file(&path, payload.bytes, DistributionFileMode::Data)?;
    }
    Ok(())
}

fn write_file(
    path: &Path,
    bytes: &[u8],
    mode: DistributionFileMode,
) -> Result<(), DistributionFailure> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| DistributionFailure::internal())?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| DistributionFailure::internal())?;
    set_mode(path, mode)?;
    Ok(())
}

fn validate_staging(
    root: &Path,
    binary_path: &str,
    binary: &StableBinary,
    payloads: &[Payload],
    manifest: &[u8],
) -> Result<(), DistributionFailure> {
    let mut expected = BTreeSet::from([binary_path.to_owned()]);
    validate_file_digest(
        &root.join(binary_path),
        binary.length,
        &binary.sha256,
        DistributionFileMode::Executable,
    )?;
    for payload in payloads {
        expected.insert(payload.path.to_owned());
        validate_file(
            &root.join(payload.path),
            payload.bytes,
            DistributionFileMode::Data,
        )?;
    }
    expected.insert("manifest.json".to_owned());
    validate_file(
        &root.join("manifest.json"),
        manifest,
        DistributionFileMode::Data,
    )?;
    let actual = collect_relative_files(root)?;
    if actual != expected {
        return Err(DistributionFailure::internal());
    }
    Ok(())
}

fn validate_file_digest(
    path: &Path,
    expected_length: u64,
    expected_sha256: &str,
    mode: DistributionFileMode,
) -> Result<(), DistributionFailure> {
    let metadata = fs::symlink_metadata(path).map_err(|_| DistributionFailure::internal())?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() != expected_length
        || !mode_matches(&metadata, mode)
    {
        return Err(DistributionFailure::internal());
    }
    let mut file = File::open(path).map_err(|_| DistributionFailure::internal())?;
    let (length, digest) = digest_binary(&mut file).map_err(|_| DistributionFailure::internal())?;
    if length != expected_length || digest != expected_sha256 {
        return Err(DistributionFailure::internal());
    }
    Ok(())
}

fn validate_file(
    path: &Path,
    expected: &[u8],
    mode: DistributionFileMode,
) -> Result<(), DistributionFailure> {
    let metadata = fs::symlink_metadata(path).map_err(|_| DistributionFailure::internal())?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() != u64::try_from(expected.len()).unwrap_or(u64::MAX)
        || !mode_matches(&metadata, mode)
    {
        return Err(DistributionFailure::internal());
    }
    let actual = fs::read(path).map_err(|_| DistributionFailure::internal())?;
    if actual != expected || sha256_hex(&actual) != sha256_hex(expected) {
        return Err(DistributionFailure::internal());
    }
    Ok(())
}

fn collect_relative_files(root: &Path) -> Result<BTreeSet<String>, DistributionFailure> {
    let mut directories = vec![root.to_path_buf()];
    let mut files = BTreeSet::new();
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(&directory).map_err(|_| DistributionFailure::internal())? {
            let entry = entry.map_err(|_| DistributionFailure::internal())?;
            let path = entry.path();
            let metadata =
                fs::symlink_metadata(&path).map_err(|_| DistributionFailure::internal())?;
            if metadata.file_type().is_symlink() {
                return Err(DistributionFailure::internal());
            }
            if metadata.is_dir() {
                directories.push(path);
            } else if metadata.is_file() {
                let relative = path
                    .strip_prefix(root)
                    .map_err(|_| DistributionFailure::internal())?;
                files.insert(relative_path(relative)?);
            } else {
                return Err(DistributionFailure::internal());
            }
        }
    }
    Ok(files)
}

fn relative_path(path: &Path) -> Result<String, DistributionFailure> {
    let components = path
        .iter()
        .map(OsStr::to_str)
        .collect::<Option<Vec<_>>>()
        .ok_or_else(DistributionFailure::internal)?;
    Ok(components.join("/"))
}

struct StableBinary {
    path: PathBuf,
    identity: FileIdentity,
    metadata: Metadata,
    length: u64,
    sha256: String,
}

impl StableBinary {
    fn inspect(path: &Path) -> Result<Self, DistributionFailure> {
        let path_before = fs::symlink_metadata(path).map_err(|_| invalid_binary())?;
        if path_before.file_type().is_symlink() || !path_before.is_file() {
            return Err(invalid_binary());
        }
        if path_before.len() > MAX_LOCAL_DISTRIBUTION_BINARY_BYTES {
            return Err(limit_exceeded());
        }
        let mut file = File::open(path).map_err(|_| invalid_binary())?;
        let before = file.metadata().map_err(|_| invalid_binary())?;
        let identity = FileIdentity::from_file(file.try_clone().map_err(|_| invalid_binary())?)
            .map_err(|_| invalid_binary())?;
        let (length, sha256) = digest_binary(&mut file)?;
        let after = file.metadata().map_err(|_| invalid_binary())?;
        let path_before_reopen = fs::symlink_metadata(path).map_err(|_| invalid_binary())?;
        let mut reopened = File::open(path).map_err(|_| invalid_binary())?;
        let reopened_before = reopened.metadata().map_err(|_| invalid_binary())?;
        let reopened_identity =
            FileIdentity::from_file(reopened.try_clone().map_err(|_| invalid_binary())?)
                .map_err(|_| invalid_binary())?;
        let (verification_length, verification_sha256) = digest_binary(&mut reopened)?;
        let reopened_after = reopened.metadata().map_err(|_| invalid_binary())?;
        let path_after = fs::symlink_metadata(path).map_err(|_| invalid_binary())?;
        let path_identity_matches =
            FileIdentity::from_path(path).is_ok_and(|path_identity| path_identity == identity);
        if length == 0
            || length != verification_length
            || sha256 != verification_sha256
            || identity != reopened_identity
            || !path_identity_matches
            || path_after.file_type().is_symlink()
            || !same_file_metadata(&path_before, &before)
            || !same_file_metadata(&before, &after)
            || !same_file_metadata(&after, &path_before_reopen)
            || !same_file_metadata(&path_before_reopen, &reopened_before)
            || !same_file_metadata(&reopened_before, &reopened_after)
            || !same_file_metadata(&reopened_after, &path_after)
        {
            return Err(invalid_binary());
        }
        Ok(Self {
            path: path.to_path_buf(),
            identity,
            metadata: reopened_after,
            length,
            sha256,
        })
    }

    fn write_to(&self, destination: &Path) -> Result<(), DistributionFailure> {
        let path_before = fs::symlink_metadata(&self.path).map_err(|_| invalid_binary())?;
        if path_before.file_type().is_symlink()
            || !same_file_metadata(&self.metadata, &path_before)
            || !FileIdentity::from_path(&self.path)
                .is_ok_and(|path_identity| path_identity == self.identity)
        {
            return Err(invalid_binary());
        }
        let mut source = File::open(&self.path).map_err(|_| invalid_binary())?;
        let source_before = source.metadata().map_err(|_| invalid_binary())?;
        let source_identity =
            FileIdentity::from_file(source.try_clone().map_err(|_| invalid_binary())?)
                .map_err(|_| invalid_binary())?;
        if source_identity != self.identity || !same_file_metadata(&self.metadata, &source_before) {
            return Err(invalid_binary());
        }
        let parent = destination
            .parent()
            .ok_or_else(DistributionFailure::internal)?;
        fs::create_dir_all(parent).map_err(|_| DistributionFailure::internal())?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)
            .map_err(|_| DistributionFailure::internal())?;
        let (length, sha256) = copy_binary(&mut source, &mut output)?;
        output
            .sync_all()
            .map_err(|_| DistributionFailure::internal())?;
        let source_after = source.metadata().map_err(|_| invalid_binary())?;
        let path_after = fs::symlink_metadata(&self.path).map_err(|_| invalid_binary())?;
        if length != self.length
            || sha256 != self.sha256
            || path_after.file_type().is_symlink()
            || !same_file_metadata(&self.metadata, &source_after)
            || !same_file_metadata(&source_after, &path_after)
            || !FileIdentity::from_path(&self.path)
                .is_ok_and(|path_identity| path_identity == self.identity)
        {
            return Err(invalid_binary());
        }
        set_mode(destination, DistributionFileMode::Executable)
    }
}

fn digest_binary(file: &mut File) -> Result<(u64, String), DistributionFailure> {
    let mut hasher = Sha256::new();
    let mut length = 0_u64;
    let mut buffer = [0_u8; 16_384];
    loop {
        let read = file.read(&mut buffer).map_err(|_| invalid_binary())?;
        if read == 0 {
            break;
        }
        length = length
            .checked_add(u64::try_from(read).unwrap_or(u64::MAX))
            .ok_or_else(limit_exceeded)?;
        if length > MAX_LOCAL_DISTRIBUTION_BINARY_BYTES {
            return Err(limit_exceeded());
        }
        hasher.update(&buffer[..read]);
    }
    Ok((length, encode_digest(hasher.finalize())))
}

fn copy_binary(
    source: &mut File,
    destination: &mut File,
) -> Result<(u64, String), DistributionFailure> {
    let mut hasher = Sha256::new();
    let mut length = 0_u64;
    let mut buffer = [0_u8; 16_384];
    loop {
        let read = source.read(&mut buffer).map_err(|_| invalid_binary())?;
        if read == 0 {
            break;
        }
        length = length
            .checked_add(u64::try_from(read).unwrap_or(u64::MAX))
            .ok_or_else(invalid_binary)?;
        if length > MAX_LOCAL_DISTRIBUTION_BINARY_BYTES {
            return Err(invalid_binary());
        }
        destination
            .write_all(&buffer[..read])
            .map_err(|_| DistributionFailure::internal())?;
        hasher.update(&buffer[..read]);
    }
    Ok((length, encode_digest(hasher.finalize())))
}

fn ensure_absent(path: &Path) -> Result<(), DistributionFailure> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(output_exists()),
        Err(_) => Err(DistributionFailure::internal()),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    encode_digest(Sha256::digest(bytes))
}

fn encode_digest(digest: impl AsRef<[u8]>) -> String {
    let mut encoded = String::with_capacity(64);
    for byte in digest.as_ref() {
        use std::fmt::Write as _;
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

#[cfg(unix)]
fn set_private_directory(path: &Path) -> Result<(), DistributionFailure> {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|_| DistributionFailure::internal())
}

#[cfg(not(unix))]
fn set_private_directory(path: &Path) -> Result<(), DistributionFailure> {
    let mut permissions = fs::metadata(path)
        .map_err(|_| DistributionFailure::internal())?
        .permissions();
    permissions.set_readonly(false);
    fs::set_permissions(path, permissions).map_err(|_| DistributionFailure::internal())
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: DistributionFileMode) -> Result<(), DistributionFailure> {
    use std::os::unix::fs::PermissionsExt as _;
    let bits = match mode {
        DistributionFileMode::Executable => 0o755,
        DistributionFileMode::Data => 0o644,
    };
    fs::set_permissions(path, fs::Permissions::from_mode(bits))
        .map_err(|_| DistributionFailure::internal())
}

#[cfg(not(unix))]
fn set_mode(path: &Path, _mode: DistributionFileMode) -> Result<(), DistributionFailure> {
    let mut permissions = fs::metadata(path)
        .map_err(|_| DistributionFailure::internal())?
        .permissions();
    permissions.set_readonly(false);
    fs::set_permissions(path, permissions).map_err(|_| DistributionFailure::internal())
}

#[cfg(unix)]
fn mode_matches(metadata: &Metadata, mode: DistributionFileMode) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    let expected = match mode {
        DistributionFileMode::Executable => 0o755,
        DistributionFileMode::Data => 0o644,
    };
    metadata.permissions().mode() & 0o777 == expected
}

#[cfg(not(unix))]
fn mode_matches(_metadata: &Metadata, _mode: DistributionFileMode) -> bool {
    true
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

fn invalid_arguments() -> DistributionFailure {
    DistributionFailure::new(CodeNoesisErrorV26::distribution_invalid_arguments(), 2)
}

fn invalid_binary() -> DistributionFailure {
    DistributionFailure::new(CodeNoesisErrorV26::distribution_invalid_binary(), 2)
}

fn output_exists() -> DistributionFailure {
    DistributionFailure::new(CodeNoesisErrorV26::distribution_output_exists(), 2)
}

fn limit_exceeded() -> DistributionFailure {
    DistributionFailure::new(CodeNoesisErrorV26::distribution_limit_exceeded(), 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BINARY_V1: &[u8] = include_bytes!(
        "../../tests/specifications/g1/local-cli-distribution-v1/fixtures/noesis-v1.bin"
    );
    const BINARY_V2: &[u8] = include_bytes!(
        "../../tests/specifications/g1/local-cli-distribution-v1/fixtures/noesis-v2.bin"
    );

    #[test]
    fn pt_fr_rel_002_fifty_argument_constructions_are_byte_identical() {
        if current_local_distribution_target() == "unsupported-compile-target" {
            return;
        }
        let root = tempfile::tempdir().expect("temporary root");
        let binary = root.path().join("noesis-input");
        fs::write(&binary, BINARY_V1).expect("write fixture binary");
        let mut expected = None;
        for construction in 0..50 {
            let output = root.path().join(format!("output-{construction}"));
            fs::create_dir(&output).expect("create output root");
            let stdout = run(package_arguments(&binary, &output, construction % 2 == 1))
                .expect("package fixture");
            if let Some(expected) = &expected {
                assert_eq!(&stdout, expected, "construction {construction}");
            } else {
                expected = Some(stdout);
            }
        }
    }

    #[test]
    fn pt_fr_rel_002_ten_parallel_schedules_are_byte_identical() {
        if current_local_distribution_target() == "unsupported-compile-target" {
            return;
        }
        let root = tempfile::tempdir().expect("temporary root");
        let binary = root.path().join("noesis-input");
        fs::write(&binary, BINARY_V1).expect("write fixture binary");
        let outputs = (0..10)
            .map(|schedule| {
                let output = root.path().join(format!("schedule-{schedule}"));
                fs::create_dir(&output).expect("create schedule output");
                output
            })
            .collect::<Vec<_>>();
        let results = std::thread::scope(|scope| {
            outputs
                .iter()
                .enumerate()
                .map(|(schedule, output)| {
                    let binary = &binary;
                    scope.spawn(move || {
                        run(package_arguments(binary, output, schedule % 2 == 1))
                            .expect("package schedule")
                    })
                })
                .collect::<Vec<_>>()
                .into_iter()
                .map(|handle| handle.join().expect("join schedule"))
                .collect::<Vec<_>>()
        });
        for (schedule, result) in results[1..].iter().enumerate() {
            assert_eq!(result, &results[0], "schedule {}", schedule + 1);
        }
    }

    #[test]
    fn ft_fr_rel_002_side_by_side_upgrade_rollback_and_uninstall_are_explicit() {
        if current_local_distribution_target() == "unsupported-compile-target" {
            return;
        }
        let root = tempfile::tempdir().expect("temporary root");
        let installation = root.path().join("installation");
        fs::create_dir(&installation).expect("create installation prefix");
        let generation_one = package_fixture(root.path(), "one", BINARY_V1);
        let generation_two = package_fixture(root.path(), "two", BINARY_V2);
        let installed_one = installation.join(generation_one.file_name().expect("bundle name"));
        let installed_two = installation.join(generation_two.file_name().expect("bundle name"));
        fs::rename(&generation_one, &installed_one).expect("install generation one");
        let mut selected = installed_one.clone();
        assert!(selected.join("manifest.json").is_file());

        fs::rename(&generation_two, &installed_two).expect("install generation two");
        selected.clone_from(&installed_two);
        assert!(selected.join("manifest.json").is_file());
        assert!(installed_one.is_dir());

        selected.clone_from(&installed_one);
        assert!(selected.join("manifest.json").is_file());
        assert!(!installation.join("current").exists());

        fs::remove_dir_all(&installed_two).expect("uninstall generation two");
        fs::remove_dir_all(&selected).expect("uninstall selected generation one");
        assert_eq!(fs::read_dir(&installation).unwrap().count(), 0);
    }

    #[test]
    fn sec_fr_rel_002_invalid_inputs_fail_closed_without_partial_bundle() {
        let root = tempfile::tempdir().expect("temporary root");
        let binary = root.path().join("noesis-input");
        fs::write(&binary, BINARY_V1).expect("write fixture binary");

        let nonempty = root.path().join("nonempty");
        fs::create_dir(&nonempty).expect("create output");
        fs::write(nonempty.join("occupied"), b"occupied").expect("occupy output");
        let failure = run(package_arguments(&binary, &nonempty, false)).unwrap_err();
        assert_code(&failure, "distribution.output_exists", 2);
        assert_eq!(fs::read_dir(&nonempty).unwrap().count(), 1);

        let invalid_binary_path = root.path().join("binary-directory");
        fs::create_dir(&invalid_binary_path).expect("create binary directory");
        let empty = root.path().join("empty");
        fs::create_dir(&empty).expect("create empty output");
        let failure = run(package_arguments(&invalid_binary_path, &empty, false)).unwrap_err();
        assert_code(&failure, "distribution.invalid_binary", 2);
        assert_eq!(fs::read_dir(&empty).unwrap().count(), 0);

        let zero = root.path().join("zero");
        fs::write(&zero, []).expect("write zero binary");
        let failure = run(package_arguments(&zero, &empty, false)).unwrap_err();
        assert_code(&failure, "distribution.invalid_binary", 2);
        assert_eq!(fs::read_dir(&empty).unwrap().count(), 0);
    }

    #[test]
    fn inv_bnd_001_binary_maximum_plus_one_is_rejected_before_output() {
        let root = tempfile::tempdir().expect("temporary root");
        let binary = root.path().join("oversized");
        File::create(&binary)
            .and_then(|file| file.set_len(MAX_LOCAL_DISTRIBUTION_BINARY_BYTES + 1))
            .expect("create sparse oversized binary");
        let output = root.path().join("output");
        fs::create_dir(&output).expect("create output root");
        let failure = run(package_arguments(&binary, &output, false)).unwrap_err();
        assert_code(&failure, "distribution.limit_exceeded", 2);
        assert_eq!(fs::read_dir(&output).unwrap().count(), 0);
    }

    #[test]
    fn inv_bnd_001_binary_maximum_is_accepted_with_bounded_memory() {
        let root = tempfile::tempdir().expect("temporary root");
        let binary = root.path().join("maximum");
        File::create(&binary)
            .and_then(|file| file.set_len(MAX_LOCAL_DISTRIBUTION_BINARY_BYTES))
            .expect("create sparse maximum binary");
        let inspected = StableBinary::inspect(&binary).expect("inspect maximum binary");
        assert_eq!(inspected.length, MAX_LOCAL_DISTRIBUTION_BINARY_BYTES);
        assert_eq!(inspected.sha256.len(), 64);
    }

    #[test]
    fn ft_fr_rel_002_abandoned_private_staging_is_cleaned() {
        let root = tempfile::tempdir().expect("temporary root");
        let output_path = root.path().join("output");
        fs::create_dir(&output_path).expect("create output root");
        let output = OutputRoot::validate(&output_path).expect("valid output root");
        {
            let staging = StagingDirectory::create(&output).expect("create staging");
            fs::write(staging.path().join("partial"), b"partial").expect("write partial file");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                assert_eq!(
                    fs::metadata(staging.path()).unwrap().permissions().mode() & 0o777,
                    0o700
                );
            }
        }
        assert_eq!(fs::read_dir(output_path).unwrap().count(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn sec_fr_rel_002_symlink_binary_and_output_are_rejected() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().expect("temporary root");
        let binary = root.path().join("binary");
        fs::write(&binary, BINARY_V1).expect("write binary");
        let binary_link = root.path().join("binary-link");
        symlink(&binary, &binary_link).expect("link binary");
        let output = root.path().join("output");
        fs::create_dir(&output).expect("create output");
        let failure = run(package_arguments(&binary_link, &output, false)).unwrap_err();
        assert_code(&failure, "distribution.invalid_binary", 2);

        let output_link = root.path().join("output-link");
        symlink(&output, &output_link).expect("link output");
        let failure = run(package_arguments(&binary, &output_link, false)).unwrap_err();
        assert_code(&failure, "distribution.invalid_arguments", 2);
    }

    #[test]
    fn sec_fr_rel_002_argument_matrix_is_strict() {
        for arguments in [
            vec!["xtask"],
            vec!["xtask", "package-local-cli"],
            vec![
                "xtask",
                "package-local-cli",
                "--binary",
                "binary",
                "--binary",
                "binary",
            ],
            vec![
                "xtask",
                "package-local-cli",
                "--target",
                "runtime-target",
                "--output",
                "output",
            ],
        ] {
            let failure = run(arguments.into_iter().map(OsString::from)).unwrap_err();
            assert_code(&failure, "distribution.invalid_arguments", 2);
        }
    }

    fn package_fixture(parent: &Path, label: &str, bytes: &[u8]) -> PathBuf {
        let binary = parent.join(format!("binary-{label}"));
        fs::write(&binary, bytes).expect("write fixture");
        let output = parent.join(format!("output-{label}"));
        fs::create_dir(&output).expect("create output");
        run(package_arguments(&binary, &output, false)).expect("package fixture");
        fs::read_dir(output)
            .expect("read output")
            .next()
            .expect("one bundle")
            .expect("bundle entry")
            .path()
    }

    fn package_arguments(binary: &Path, output: &Path, reversed: bool) -> Vec<OsString> {
        let mut arguments = vec![OsString::from("xtask"), OsString::from("package-local-cli")];
        let pairs = if reversed {
            [("--output", output), ("--binary", binary)]
        } else {
            [("--binary", binary), ("--output", output)]
        };
        for (flag, value) in pairs {
            arguments.push(OsString::from(flag));
            arguments.push(value.as_os_str().to_owned());
        }
        arguments
    }

    fn assert_code(failure: &DistributionFailure, code: &str, exit_code: u8) {
        assert_eq!(failure.exit_code(), exit_code);
        assert_eq!(failure.error().value()["code"], code);
    }
}
