//! Deterministic local release-candidate packaging and offline verification.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{Read as _, Seek as _, Write as _};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use codenoesis_contracts::{
    CodeNoesisErrorV28, DistributionFileMode, EmbeddedLocalBundleV1,
    LocalReleaseCandidateManifestV1, LocalReleaseCandidateVerificationV1,
    LocalReleaseContractError, MAX_LOCAL_DISTRIBUTION_BINARY_BYTES,
    MAX_LOCAL_DISTRIBUTION_MANIFEST_BYTES, MAX_LOCAL_RELEASE_ARCHIVE_BYTES,
    MAX_LOCAL_RELEASE_EVIDENCE_DOCUMENT_BYTES, MAX_LOCAL_RELEASE_PUBLIC_JSON_BYTES,
    MAX_LOCAL_RELEASE_RELATIVE_PATH_BYTES, MAX_LOCAL_RELEASE_ZIP_ENTRIES, ReleaseArchiveRecordV1,
    ReleaseEvidenceRecordV1, local_distribution_bundle_name, local_release_archive_name,
    local_release_candidate_name, parse_local_release_candidate_manifest_v1,
    validate_local_distribution_manifest_v1, validate_local_supply_chain_v1,
};
use crc32fast::Hasher as Crc32;
use same_file::Handle as FileIdentity;
use sha2::{Digest as _, Sha256};

const G1_FILE_COUNT: usize = 6;
const MAX_TREE_ENTRIES: usize = 32;
const MAX_STAGING_ATTEMPTS: u64 = 32;
const ZIP_LOCAL_SIGNATURE: u32 = 0x0403_4b50;
const ZIP_CENTRAL_SIGNATURE: u32 = 0x0201_4b50;
const ZIP_END_SIGNATURE: u32 = 0x0605_4b50;
const ZIP_VERSION: u16 = 20;
const ZIP_UNIX_VERSION: u16 = 0x0314;
const ZIP_DOS_DATE: u16 = 33;
const ZIP_ENTRY_COUNT: u16 = 6;
const ZIP_EXECUTABLE_MODE: u32 = 0o100_755 << 16;
const ZIP_DATA_MODE: u32 = 0o100_644 << 16;
const EVIDENCE_INPUTS: [(&str, &str); 5] = [
    ("advisory-report.json", "evidence/advisory-report.json"),
    ("dependency-lock.json", "evidence/dependency-lock.json"),
    ("license-report.json", "evidence/license-report.json"),
    ("sbom.cdx.json", "evidence/sbom.cdx.json"),
    ("unsafe-inventory.json", "evidence/unsafe-inventory.json"),
];

static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug)]
pub struct ReleaseFailure {
    error: CodeNoesisErrorV28,
    exit_code: u8,
}

impl ReleaseFailure {
    /// Returns one privacy-safe completion failure.
    #[must_use]
    pub fn internal() -> Self {
        Self::new(CodeNoesisErrorV28::internal(), 1)
    }

    /// Returns the exact public `ErrorV28` value.
    #[must_use]
    pub const fn error(&self) -> &CodeNoesisErrorV28 {
        &self.error
    }

    /// Returns the public process exit status.
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        self.exit_code
    }

    fn from_contract(error: LocalReleaseContractError) -> Self {
        let exit_code = if error == LocalReleaseContractError::ContractInvalid {
            1
        } else {
            2
        };
        Self::new(CodeNoesisErrorV28::from_contract(error), exit_code)
    }

    const fn new(error: CodeNoesisErrorV28, exit_code: u8) -> Self {
        Self { error, exit_code }
    }
}

/// Returns whether one command belongs to the additive G1b adapter.
#[must_use]
pub fn is_release_command(command: Option<&OsString>) -> bool {
    command.is_some_and(|command| {
        command == "package-local-release-candidate" || command == "verify-local-release-candidate"
    })
}

/// Packages or verifies one exact local release candidate.
///
/// # Errors
///
/// Returns a privacy-safe `ErrorV28` failure for invalid arguments, input,
/// policy, archive, race, limit, or completion errors.
pub fn run(arguments: impl IntoIterator<Item = OsString>) -> Result<Vec<u8>, ReleaseFailure> {
    match Invocation::parse(&arguments.into_iter().collect::<Vec<_>>())? {
        Invocation::Package {
            bundle,
            source_commit,
            supply_chain,
            output,
        } => package_candidate(&bundle, &source_commit, &supply_chain, &output),
        Invocation::Verify { candidate } => verify_candidate(&candidate, None),
    }
}

enum Invocation {
    Package {
        bundle: PathBuf,
        source_commit: String,
        supply_chain: PathBuf,
        output: PathBuf,
    },
    Verify {
        candidate: PathBuf,
    },
}

impl Invocation {
    fn parse(arguments: &[OsString]) -> Result<Self, ReleaseFailure> {
        match arguments.get(1).and_then(|value| value.to_str()) {
            Some("package-local-release-candidate") if arguments.len() == 10 => {
                let values = parse_pairs(
                    &arguments[2..],
                    &["--bundle", "--source-commit", "--supply-chain", "--output"],
                )?;
                let source_commit = values[1]
                    .to_str()
                    .filter(|value| is_git_sha(value))
                    .ok_or_else(invalid_arguments)?
                    .to_owned();
                Ok(Self::Package {
                    bundle: PathBuf::from(&values[0]),
                    source_commit,
                    supply_chain: PathBuf::from(&values[2]),
                    output: PathBuf::from(&values[3]),
                })
            }
            Some("verify-local-release-candidate") if arguments.len() == 4 => {
                let values = parse_pairs(&arguments[2..], &["--candidate"])?;
                Ok(Self::Verify {
                    candidate: PathBuf::from(&values[0]),
                })
            }
            _ => Err(invalid_arguments()),
        }
    }
}

fn parse_pairs(arguments: &[OsString], flags: &[&str]) -> Result<Vec<OsString>, ReleaseFailure> {
    let mut values = vec![None; flags.len()];
    for pair in arguments.chunks_exact(2) {
        let flag = pair[0].to_str().ok_or_else(invalid_arguments)?;
        if pair[1].is_empty() {
            return Err(invalid_arguments());
        }
        let index = flags
            .iter()
            .position(|expected| flag == *expected)
            .ok_or_else(invalid_arguments)?;
        if values[index].replace(pair[1].clone()).is_some() {
            return Err(invalid_arguments());
        }
    }
    values
        .into_iter()
        .map(|value| value.ok_or_else(invalid_arguments))
        .collect()
}

fn package_candidate(
    bundle_path: &Path,
    source_commit: &str,
    supply_path: &Path,
    output_path: &Path,
) -> Result<Vec<u8>, ReleaseFailure> {
    let output = OutputRoot::inspect(output_path)?;
    output.ensure_empty()?;
    let bundle = StableBundle::inspect(bundle_path)?;
    let supply = StableSupplyChain::inspect(supply_path, &bundle.target)?;
    if output.same_file(&bundle.root)
        || output.same_file(&supply.root)
        || bundle.root.same_file(&supply.root)
        || bundle.shares_file_with(&supply)
    {
        return Err(invalid_arguments());
    }
    output.ensure_empty()?;
    bundle.revalidate()?;
    supply.revalidate()?;

    let archive_name = local_release_archive_name(&bundle.target, &bundle.binary_sha256);
    let mut staging = StagingDirectory::create(&output)?;
    let archive_path = staging.path().join(&archive_name);
    write_archive(&archive_path, &bundle)?;
    let archive = StableFile::read(
        &archive_path,
        MAX_LOCAL_RELEASE_ARCHIVE_BYTES,
        false,
        LocalReleaseContractError::InvalidArchive,
    )?;

    let evidence_records = write_evidence(staging.path(), &supply)?;
    let archive_record =
        ReleaseArchiveRecordV1::new(&archive_name, archive.length, &archive.sha256);
    let embedded_bundle =
        EmbeddedLocalBundleV1::new(&bundle.name, &bundle.manifest_sha256, &bundle.binary_sha256);
    let manifest = LocalReleaseCandidateManifestV1::new(
        &bundle.target,
        source_commit,
        archive_record,
        embedded_bundle,
        &evidence_records,
    )
    .map_err(ReleaseFailure::from_contract)?;
    let manifest_bytes = manifest
        .canonical_stdout()
        .map_err(ReleaseFailure::from_contract)?;
    write_file(&staging.path().join("manifest.json"), &manifest_bytes)?;
    let checksums = checksums_bytes(&manifest, &manifest_bytes);
    write_file(&staging.path().join("checksums.sha256"), &checksums)?;

    let final_name = local_release_candidate_name(&bundle.target, &archive.sha256);
    let final_path = output.path.join(&final_name);
    ensure_absent(&final_path)?;
    let _verification = verify_candidate(staging.path(), Some(&final_name))?;
    bundle.revalidate()?;
    supply.revalidate()?;
    archive.revalidate()?;
    output.revalidate()?;
    output.ensure_single_directory(staging.path())?;
    ensure_absent(&final_path)?;
    fs::rename(staging.path(), &final_path).map_err(|_| ReleaseFailure::internal())?;
    if output.ensure_single_directory(&final_path).is_err() {
        let _ = fs::rename(&final_path, staging.path());
        return Err(unstable_input());
    }
    staging.disarm();
    output.revalidate()?;
    Ok(manifest_bytes)
}

fn verify_candidate(
    candidate_path: &Path,
    expected_name: Option<&str>,
) -> Result<Vec<u8>, ReleaseFailure> {
    let root = StableRoot::inspect(candidate_path, LocalReleaseContractError::InvalidArchive)?;
    let candidate_name = expected_name.map_or_else(
        || {
            candidate_path
                .file_name()
                .and_then(OsStr::to_str)
                .map(str::to_owned)
                .ok_or_else(invalid_archive)
        },
        |name| Ok(name.to_owned()),
    )?;
    let manifest_file = StableFile::read(
        &candidate_path.join("manifest.json"),
        u64::try_from(MAX_LOCAL_RELEASE_PUBLIC_JSON_BYTES).unwrap_or(u64::MAX),
        true,
        LocalReleaseContractError::InvalidArchive,
    )?;
    let manifest_bytes = manifest_file.bytes().ok_or_else(ReleaseFailure::internal)?;
    if !mode_matches(&manifest_file.metadata, DistributionFileMode::Data) {
        return Err(invalid_archive());
    }
    let manifest = parse_local_release_candidate_manifest_v1(manifest_bytes)
        .map_err(|error| map_manifest_error(error, LocalReleaseContractError::InvalidArchive))?;
    let expected_candidate_name =
        local_release_candidate_name(manifest.target(), manifest.archive().sha256());
    if candidate_name != expected_candidate_name {
        return Err(invalid_archive());
    }

    let expected_tree = candidate_tree(manifest.archive().path());
    if collect_tree(
        candidate_path,
        MAX_TREE_ENTRIES,
        LocalReleaseContractError::InvalidArchive,
    )? != expected_tree
    {
        return Err(invalid_archive());
    }
    let archive = StableFile::read(
        &candidate_path.join(manifest.archive().path()),
        MAX_LOCAL_RELEASE_ARCHIVE_BYTES,
        false,
        LocalReleaseContractError::InvalidArchive,
    )?;
    if archive.length != manifest.archive().length()
        || archive.sha256 != manifest.archive().sha256()
        || !mode_matches(&archive.metadata, DistributionFileMode::Data)
    {
        return Err(invalid_archive());
    }

    let checksums = StableFile::read(
        &candidate_path.join("checksums.sha256"),
        u64::try_from(MAX_LOCAL_RELEASE_PUBLIC_JSON_BYTES).unwrap_or(u64::MAX),
        true,
        LocalReleaseContractError::InvalidArchive,
    )?;
    if !mode_matches(&checksums.metadata, DistributionFileMode::Data) {
        return Err(invalid_archive());
    }
    let evidence = StableSupplyChain::inspect_candidate(candidate_path, manifest.target())?;
    if evidence.records != manifest.evidence() {
        return Err(invalid_evidence());
    }
    let mut candidate_files = vec![&manifest_file, &archive, &checksums];
    candidate_files.extend(evidence.files.iter().map(|(_, file)| file));
    if !has_unique_file_identities(&candidate_files) {
        return Err(invalid_archive());
    }
    let expected_checksums = checksums_bytes(&manifest, manifest_bytes);
    if checksums.bytes() != Some(expected_checksums.as_slice()) {
        return Err(invalid_archive());
    }

    validate_archive(&archive, &manifest)?;
    root.revalidate()?;
    manifest_file.revalidate()?;
    archive.revalidate()?;
    checksums.revalidate()?;
    evidence.revalidate()?;

    let verification = LocalReleaseCandidateVerificationV1::new(
        &candidate_name,
        &manifest,
        &manifest_file.sha256,
        &checksums.sha256,
    )
    .map_err(ReleaseFailure::from_contract)?;
    verification
        .canonical_stdout()
        .map_err(ReleaseFailure::from_contract)
}

fn checksums_bytes(manifest: &LocalReleaseCandidateManifestV1, manifest_bytes: &[u8]) -> Vec<u8> {
    let mut subjects = BTreeMap::new();
    subjects.insert(
        manifest.archive().path().to_owned(),
        manifest.archive().sha256().to_owned(),
    );
    for record in manifest.evidence() {
        subjects.insert(record.path().to_owned(), record.sha256().to_owned());
    }
    subjects.insert("manifest.json".to_owned(), sha256_hex(manifest_bytes));
    let mut checksums = Vec::new();
    for (path, sha256) in subjects {
        checksums.extend_from_slice(sha256.as_bytes());
        checksums.extend_from_slice(b"  ");
        checksums.extend_from_slice(path.as_bytes());
        checksums.push(b'\n');
    }
    checksums
}

struct OutputRoot {
    path: PathBuf,
    identity: FileIdentity,
}

impl OutputRoot {
    fn inspect(path: &Path) -> Result<Self, ReleaseFailure> {
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

    fn ensure_empty(&self) -> Result<(), ReleaseFailure> {
        self.revalidate()?;
        if fs::read_dir(&self.path)
            .map_err(|_| invalid_arguments())?
            .next()
            .transpose()
            .map_err(|_| invalid_arguments())?
            .is_some()
        {
            return Err(invalid_arguments());
        }
        self.revalidate()
    }

    fn same_file(&self, root: &StableRoot) -> bool {
        self.identity == root.identity
    }

    fn ensure_single_directory(&self, expected: &Path) -> Result<(), ReleaseFailure> {
        self.revalidate()?;
        let entries = fs::read_dir(&self.path)
            .map_err(|_| unstable_input())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| unstable_input())?;
        let metadata = fs::symlink_metadata(expected).map_err(|_| unstable_input())?;
        if entries.len() != 1
            || entries[0].path() != expected
            || metadata.file_type().is_symlink()
            || !metadata.is_dir()
            || !FileIdentity::from_path(expected).is_ok_and(|identity| identity != self.identity)
        {
            return Err(unstable_input());
        }
        self.revalidate()
    }

    fn revalidate(&self) -> Result<(), ReleaseFailure> {
        let metadata = fs::symlink_metadata(&self.path).map_err(|_| unstable_input())?;
        if metadata.file_type().is_symlink()
            || !metadata.is_dir()
            || !FileIdentity::from_path(&self.path).is_ok_and(|identity| identity == self.identity)
        {
            return Err(unstable_input());
        }
        Ok(())
    }
}

struct StagingDirectory {
    path: PathBuf,
    armed: bool,
}

impl StagingDirectory {
    fn create(output: &OutputRoot) -> Result<Self, ReleaseFailure> {
        for _ in 0..MAX_STAGING_ATTEMPTS {
            output.revalidate()?;
            let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = output.path.join(format!(
                ".codenoesis-release-staging-{}-{sequence}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => {
                    set_private_directory(&path)?;
                    return Ok(Self { path, armed: true });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(_) => return Err(ReleaseFailure::internal()),
            }
        }
        Err(ReleaseFailure::internal())
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

struct StableRoot {
    path: PathBuf,
    identity: FileIdentity,
    metadata: Metadata,
}

impl StableRoot {
    fn inspect(path: &Path, invalid: LocalReleaseContractError) -> Result<Self, ReleaseFailure> {
        let metadata = fs::symlink_metadata(path).map_err(|_| contract_failure(invalid))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(contract_failure(invalid));
        }
        let identity = FileIdentity::from_path(path).map_err(|_| contract_failure(invalid))?;
        Ok(Self {
            path: path.to_path_buf(),
            identity,
            metadata,
        })
    }

    fn same_file(&self, other: &Self) -> bool {
        self.identity == other.identity
    }

    fn revalidate(&self) -> Result<(), ReleaseFailure> {
        let metadata = fs::symlink_metadata(&self.path).map_err(|_| unstable_input())?;
        if metadata.file_type().is_symlink()
            || !metadata.is_dir()
            || !same_file_metadata(&self.metadata, &metadata)
            || !FileIdentity::from_path(&self.path).is_ok_and(|identity| identity == self.identity)
        {
            return Err(unstable_input());
        }
        Ok(())
    }
}

struct StableFile {
    path: PathBuf,
    identity: FileIdentity,
    metadata: Metadata,
    length: u64,
    sha256: String,
    crc32: u32,
    bytes: Option<Vec<u8>>,
}

impl StableFile {
    fn read(
        path: &Path,
        maximum: u64,
        capture: bool,
        invalid: LocalReleaseContractError,
    ) -> Result<Self, ReleaseFailure> {
        let path_before = fs::symlink_metadata(path).map_err(|_| contract_failure(invalid))?;
        if path_before.file_type().is_symlink() || !path_before.is_file() {
            return Err(contract_failure(invalid));
        }
        if path_before.len() > maximum {
            return Err(limit_exceeded());
        }
        let mut file = File::open(path).map_err(|_| contract_failure(invalid))?;
        let before = file.metadata().map_err(|_| contract_failure(invalid))?;
        let identity =
            FileIdentity::from_file(file.try_clone().map_err(|_| contract_failure(invalid))?)
                .map_err(|_| contract_failure(invalid))?;
        let first = digest_reader(&mut file, maximum, capture)?;
        let after = file.metadata().map_err(|_| unstable_input())?;
        let path_before_reopen = fs::symlink_metadata(path).map_err(|_| unstable_input())?;
        let mut reopened = File::open(path).map_err(|_| unstable_input())?;
        let reopened_before = reopened.metadata().map_err(|_| unstable_input())?;
        let reopened_identity =
            FileIdentity::from_file(reopened.try_clone().map_err(|_| unstable_input())?)
                .map_err(|_| unstable_input())?;
        let second = digest_reader(&mut reopened, maximum, capture)?;
        let reopened_after = reopened.metadata().map_err(|_| unstable_input())?;
        let path_after = fs::symlink_metadata(path).map_err(|_| unstable_input())?;
        if first.length == 0
            || first != second
            || identity != reopened_identity
            || path_after.file_type().is_symlink()
            || !same_file_metadata(&path_before, &before)
            || !same_file_metadata(&before, &after)
            || !same_file_metadata(&after, &path_before_reopen)
            || !same_file_metadata(&path_before_reopen, &reopened_before)
            || !same_file_metadata(&reopened_before, &reopened_after)
            || !same_file_metadata(&reopened_after, &path_after)
            || !FileIdentity::from_path(path).is_ok_and(|path_identity| path_identity == identity)
        {
            return Err(unstable_input());
        }
        Ok(Self {
            path: path.to_path_buf(),
            identity,
            metadata: reopened_after,
            length: first.length,
            sha256: first.sha256,
            crc32: first.crc32,
            bytes: first.bytes,
        })
    }

    fn revalidate(&self) -> Result<(), ReleaseFailure> {
        let metadata = fs::symlink_metadata(&self.path).map_err(|_| unstable_input())?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || !same_file_metadata(&self.metadata, &metadata)
            || !FileIdentity::from_path(&self.path).is_ok_and(|identity| identity == self.identity)
        {
            return Err(unstable_input());
        }
        let mut file = File::open(&self.path).map_err(|_| unstable_input())?;
        let digest = digest_reader(&mut file, self.length, self.bytes.is_some())?;
        if digest.length != self.length
            || digest.sha256 != self.sha256
            || digest.crc32 != self.crc32
            || digest.bytes != self.bytes
        {
            return Err(unstable_input());
        }
        Ok(())
    }

    fn bytes(&self) -> Option<&[u8]> {
        self.bytes.as_deref()
    }
}

#[derive(Eq, PartialEq)]
struct DigestedInput {
    length: u64,
    sha256: String,
    crc32: u32,
    bytes: Option<Vec<u8>>,
}

fn digest_reader(
    reader: &mut File,
    maximum: u64,
    capture: bool,
) -> Result<DigestedInput, ReleaseFailure> {
    let capacity = if capture {
        usize::try_from(maximum.min(65_536)).unwrap_or(65_536)
    } else {
        0
    };
    let mut bytes = capture.then(|| Vec::with_capacity(capacity));
    let mut sha256 = Sha256::new();
    let mut crc32 = Crc32::new();
    let mut length = 0_u64;
    let mut buffer = [0_u8; 16_384];
    loop {
        let read = reader.read(&mut buffer).map_err(|_| unstable_input())?;
        if read == 0 {
            break;
        }
        length = length
            .checked_add(u64::try_from(read).unwrap_or(u64::MAX))
            .ok_or_else(limit_exceeded)?;
        if length > maximum {
            return Err(limit_exceeded());
        }
        sha256.update(&buffer[..read]);
        crc32.update(&buffer[..read]);
        if let Some(bytes) = &mut bytes {
            bytes.extend_from_slice(&buffer[..read]);
        }
    }
    Ok(DigestedInput {
        length,
        sha256: encode_digest(sha256.finalize()),
        crc32: crc32.finalize(),
        bytes,
    })
}

struct StableBundle {
    root: StableRoot,
    files: BTreeMap<String, StableFile>,
    target: String,
    name: String,
    manifest_sha256: String,
    binary_sha256: String,
}

impl StableBundle {
    fn inspect(path: &Path) -> Result<Self, ReleaseFailure> {
        let root = StableRoot::inspect(path, LocalReleaseContractError::InvalidBundle)?;
        let manifest_file = StableFile::read(
            &path.join("manifest.json"),
            u64::try_from(MAX_LOCAL_DISTRIBUTION_MANIFEST_BYTES).unwrap_or(u64::MAX),
            true,
            LocalReleaseContractError::InvalidBundle,
        )?;
        if !mode_matches(&manifest_file.metadata, DistributionFileMode::Data) {
            return Err(invalid_bundle());
        }
        let manifest = validate_local_distribution_manifest_v1(
            manifest_file.bytes().ok_or_else(ReleaseFailure::internal)?,
        )
        .map_err(|error| {
            if error == codenoesis_contracts::LocalUpgradeContractError::LimitExceeded {
                limit_exceeded()
            } else {
                invalid_bundle()
            }
        })?;
        let name = path
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or_else(invalid_bundle)?
            .to_owned();
        if name != local_distribution_bundle_name(manifest.target(), manifest.binary_sha256()) {
            return Err(invalid_bundle());
        }

        let expected_tree = g1_tree(manifest.files())?;
        if collect_tree(
            path,
            MAX_TREE_ENTRIES,
            LocalReleaseContractError::InvalidBundle,
        )? != expected_tree
        {
            return Err(invalid_bundle());
        }
        let mut files = BTreeMap::new();
        files.insert("manifest.json".to_owned(), manifest_file);
        for expected in manifest.files() {
            let relative =
                strict_relative_path(expected.path(), LocalReleaseContractError::InvalidBundle)?;
            let maximum = if expected.mode() == DistributionFileMode::Executable {
                MAX_LOCAL_DISTRIBUTION_BINARY_BYTES
            } else {
                expected.length()
            };
            let file = StableFile::read(
                &path.join(relative),
                maximum,
                false,
                LocalReleaseContractError::InvalidBundle,
            )?;
            if file.length != expected.length()
                || file.sha256 != expected.sha256()
                || !mode_matches(&file.metadata, expected.mode())
                || files.insert(expected.path().to_owned(), file).is_some()
            {
                return Err(invalid_bundle());
            }
        }
        root.revalidate()?;
        for file in files.values() {
            file.revalidate()?;
        }
        if !has_unique_file_identities(&files.values().collect::<Vec<_>>()) {
            return Err(invalid_bundle());
        }
        let manifest_sha256 = files
            .get("manifest.json")
            .map(|file| file.sha256.clone())
            .ok_or_else(invalid_bundle)?;
        Ok(Self {
            root,
            files,
            target: manifest.target().to_owned(),
            name,
            manifest_sha256,
            binary_sha256: manifest.binary_sha256().to_owned(),
        })
    }

    fn revalidate(&self) -> Result<(), ReleaseFailure> {
        self.root.revalidate()?;
        for file in self.files.values() {
            file.revalidate()?;
        }
        Ok(())
    }

    fn shares_file_with(&self, supply: &StableSupplyChain) -> bool {
        self.files.values().any(|bundle_file| {
            supply
                .files
                .iter()
                .any(|(_, supply_file)| bundle_file.identity == supply_file.identity)
        })
    }
}

struct StableSupplyChain {
    root: StableRoot,
    files: Vec<(String, StableFile)>,
    records: Vec<ReleaseEvidenceRecordV1>,
}

impl StableSupplyChain {
    fn inspect(path: &Path, target: &str) -> Result<Self, ReleaseFailure> {
        let root = StableRoot::inspect(path, LocalReleaseContractError::InvalidEvidence)?;
        let expected = EVIDENCE_INPUTS
            .iter()
            .map(|(input, _)| format!("f:{input}"))
            .collect::<BTreeSet<_>>();
        if collect_tree(
            path,
            MAX_TREE_ENTRIES,
            LocalReleaseContractError::InvalidEvidence,
        )? != expected
        {
            return Err(invalid_evidence());
        }
        let mut files = Vec::with_capacity(EVIDENCE_INPUTS.len());
        for (input, candidate_path) in EVIDENCE_INPUTS {
            let file = StableFile::read(
                &path.join(input),
                u64::try_from(MAX_LOCAL_RELEASE_EVIDENCE_DOCUMENT_BYTES).unwrap_or(u64::MAX),
                true,
                LocalReleaseContractError::InvalidEvidence,
            )?;
            if !mode_matches(&file.metadata, DistributionFileMode::Data) {
                return Err(invalid_evidence());
            }
            files.push((candidate_path.to_owned(), file));
        }
        Self::validated(root, files, target)
    }

    fn inspect_candidate(path: &Path, target: &str) -> Result<Self, ReleaseFailure> {
        let evidence_root = path.join("evidence");
        let root = StableRoot::inspect(&evidence_root, LocalReleaseContractError::InvalidEvidence)?;
        let expected = EVIDENCE_INPUTS
            .iter()
            .map(|(input, _)| format!("f:{input}"))
            .collect::<BTreeSet<_>>();
        if collect_tree(
            &evidence_root,
            MAX_TREE_ENTRIES,
            LocalReleaseContractError::InvalidEvidence,
        )? != expected
        {
            return Err(invalid_evidence());
        }
        let mut files = Vec::with_capacity(EVIDENCE_INPUTS.len());
        for (input, candidate_path) in EVIDENCE_INPUTS {
            let file = StableFile::read(
                &evidence_root.join(input),
                u64::try_from(MAX_LOCAL_RELEASE_EVIDENCE_DOCUMENT_BYTES).unwrap_or(u64::MAX),
                true,
                LocalReleaseContractError::InvalidEvidence,
            )?;
            if !mode_matches(&file.metadata, DistributionFileMode::Data) {
                return Err(invalid_evidence());
            }
            files.push((candidate_path.to_owned(), file));
        }
        Self::validated(root, files, target)
    }

    fn validated(
        root: StableRoot,
        files: Vec<(String, StableFile)>,
        target: &str,
    ) -> Result<Self, ReleaseFailure> {
        let inputs = files
            .iter()
            .map(|(path, file)| {
                Ok((
                    path.as_str(),
                    file.bytes().ok_or_else(ReleaseFailure::internal)?,
                    file.sha256.as_str(),
                ))
            })
            .collect::<Result<Vec<_>, ReleaseFailure>>()?;
        let validated = validate_local_supply_chain_v1(target, &inputs)
            .map_err(ReleaseFailure::from_contract)?;
        root.revalidate()?;
        for (_, file) in &files {
            file.revalidate()?;
        }
        if !has_unique_file_identities(&files.iter().map(|(_, file)| file).collect::<Vec<_>>()) {
            return Err(invalid_evidence());
        }
        Ok(Self {
            root,
            files,
            records: validated.records().to_vec(),
        })
    }

    fn revalidate(&self) -> Result<(), ReleaseFailure> {
        self.root.revalidate()?;
        for (_, file) in &self.files {
            file.revalidate()?;
        }
        Ok(())
    }
}

fn has_unique_file_identities(files: &[&StableFile]) -> bool {
    files.iter().enumerate().all(|(index, file)| {
        files[index + 1..]
            .iter()
            .all(|other| file.identity != other.identity)
    })
}

fn write_evidence(
    root: &Path,
    supply: &StableSupplyChain,
) -> Result<Vec<ReleaseEvidenceRecordV1>, ReleaseFailure> {
    let evidence_root = root.join("evidence");
    fs::create_dir(&evidence_root).map_err(|_| ReleaseFailure::internal())?;
    set_private_directory(&evidence_root)?;
    for (path, file) in &supply.files {
        let name = path
            .strip_prefix("evidence/")
            .ok_or_else(ReleaseFailure::internal)?;
        write_file(
            &evidence_root.join(name),
            file.bytes().ok_or_else(ReleaseFailure::internal)?,
        )?;
    }
    supply.revalidate()?;
    Ok(supply.records.clone())
}

#[derive(Clone)]
struct ZipRecord {
    name: String,
    relative: String,
    length: u32,
    crc32: u32,
    sha256: String,
    local_offset: u32,
    mode: DistributionFileMode,
    captured: Option<Vec<u8>>,
}

fn write_archive(path: &Path, bundle: &StableBundle) -> Result<(), ReleaseFailure> {
    if bundle.files.len() != G1_FILE_COUNT || G1_FILE_COUNT > MAX_LOCAL_RELEASE_ZIP_ENTRIES {
        return Err(invalid_bundle());
    }
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| ReleaseFailure::internal())?;
    let mut records = Vec::with_capacity(bundle.files.len());
    let mut offset = 0_u64;
    for (relative, file) in &bundle.files {
        let name = format!("{}/{relative}", bundle.name);
        let name_bytes = name.as_bytes();
        let name_length = u16::try_from(name_bytes.len()).map_err(|_| limit_exceeded())?;
        let length = u32::try_from(file.length).map_err(|_| limit_exceeded())?;
        let local_offset = u32::try_from(offset).map_err(|_| limit_exceeded())?;
        write_u32(&mut output, ZIP_LOCAL_SIGNATURE)?;
        write_u16(&mut output, ZIP_VERSION)?;
        write_u16(&mut output, 0)?;
        write_u16(&mut output, 0)?;
        write_u16(&mut output, 0)?;
        write_u16(&mut output, ZIP_DOS_DATE)?;
        write_u32(&mut output, file.crc32)?;
        write_u32(&mut output, length)?;
        write_u32(&mut output, length)?;
        write_u16(&mut output, name_length)?;
        write_u16(&mut output, 0)?;
        output
            .write_all(name_bytes)
            .map_err(|_| ReleaseFailure::internal())?;
        let mut source = File::open(&file.path).map_err(|_| unstable_input())?;
        let copied = copy_exact(&mut source, &mut output, file.length)?;
        if copied.sha256 != file.sha256 || copied.crc32 != file.crc32 {
            return Err(unstable_input());
        }
        let mode = if relative == "bin/noesis" || relative == "bin/noesis.exe" {
            DistributionFileMode::Executable
        } else {
            DistributionFileMode::Data
        };
        records.push(ZipRecord {
            name,
            relative: relative.clone(),
            length,
            crc32: file.crc32,
            sha256: file.sha256.clone(),
            local_offset,
            mode,
            captured: None,
        });
        offset = offset
            .checked_add(30)
            .and_then(|value| value.checked_add(u64::from(name_length)))
            .and_then(|value| value.checked_add(file.length))
            .ok_or_else(limit_exceeded)?;
    }
    let central_offset = u32::try_from(offset).map_err(|_| limit_exceeded())?;
    let central_size = write_central_directory(&mut output, &records, &mut offset, central_offset)?;
    write_archive_end(&mut output, central_size, central_offset)?;
    output.sync_all().map_err(|_| ReleaseFailure::internal())?;
    bundle.revalidate()?;
    let metadata = output.metadata().map_err(|_| ReleaseFailure::internal())?;
    if metadata.len() == 0 || metadata.len() > MAX_LOCAL_RELEASE_ARCHIVE_BYTES {
        return Err(limit_exceeded());
    }
    set_mode(path, DistributionFileMode::Data)
}

fn write_central_directory(
    output: &mut File,
    records: &[ZipRecord],
    offset: &mut u64,
    central_offset: u32,
) -> Result<u32, ReleaseFailure> {
    for record in records {
        let name_length = u16::try_from(record.name.len()).map_err(|_| limit_exceeded())?;
        write_u32(output, ZIP_CENTRAL_SIGNATURE)?;
        write_u16(output, ZIP_UNIX_VERSION)?;
        write_u16(output, ZIP_VERSION)?;
        write_u16(output, 0)?;
        write_u16(output, 0)?;
        write_u16(output, 0)?;
        write_u16(output, ZIP_DOS_DATE)?;
        write_u32(output, record.crc32)?;
        write_u32(output, record.length)?;
        write_u32(output, record.length)?;
        write_u16(output, name_length)?;
        write_u16(output, 0)?;
        write_u16(output, 0)?;
        write_u16(output, 0)?;
        write_u16(output, 0)?;
        write_u32(output, zip_external_mode(record.mode))?;
        write_u32(output, record.local_offset)?;
        output
            .write_all(record.name.as_bytes())
            .map_err(|_| ReleaseFailure::internal())?;
        *offset = offset
            .checked_add(46)
            .and_then(|value| value.checked_add(u64::from(name_length)))
            .ok_or_else(limit_exceeded)?;
    }
    u32::try_from(*offset - u64::from(central_offset)).map_err(|_| limit_exceeded())
}

fn write_archive_end(
    output: &mut File,
    central_size: u32,
    central_offset: u32,
) -> Result<(), ReleaseFailure> {
    write_u32(output, ZIP_END_SIGNATURE)?;
    write_u16(output, 0)?;
    write_u16(output, 0)?;
    write_u16(output, ZIP_ENTRY_COUNT)?;
    write_u16(output, ZIP_ENTRY_COUNT)?;
    write_u32(output, central_size)?;
    write_u32(output, central_offset)?;
    write_u16(output, 0)
}

fn validate_archive(
    archive: &StableFile,
    manifest: &LocalReleaseCandidateManifestV1,
) -> Result<(), ReleaseFailure> {
    let mut input = File::open(&archive.path).map_err(|_| invalid_archive())?;
    let mut records = Vec::with_capacity(G1_FILE_COUNT);
    for _ in 0..G1_FILE_COUNT {
        let local_offset = u32::try_from(input.stream_position().map_err(|_| invalid_archive())?)
            .map_err(|_| invalid_archive())?;
        if read_u32(&mut input)? != ZIP_LOCAL_SIGNATURE
            || read_u16(&mut input)? != ZIP_VERSION
            || read_u16(&mut input)? != 0
            || read_u16(&mut input)? != 0
            || read_u16(&mut input)? != 0
            || read_u16(&mut input)? != ZIP_DOS_DATE
        {
            return Err(invalid_archive());
        }
        let crc32 = read_u32(&mut input)?;
        let compressed_length = read_u32(&mut input)?;
        let length = read_u32(&mut input)?;
        let name_length = read_u16(&mut input)?;
        let extra_length = read_u16(&mut input)?;
        if compressed_length != length
            || length == 0
            || extra_length != 0
            || usize::from(name_length) > MAX_LOCAL_RELEASE_RELATIVE_PATH_BYTES
        {
            return Err(invalid_archive());
        }
        let name = read_utf8(&mut input, usize::from(name_length))?;
        let relative = archive_relative_path(&name, manifest.bundle().name())?;
        let captured = relative == "manifest.json";
        let digested = digest_limited_reader(&mut input, u64::from(length), captured)?;
        if digested.length != u64::from(length) || digested.crc32 != crc32 {
            return Err(invalid_archive());
        }
        records.push(ZipRecord {
            name,
            relative,
            length,
            crc32,
            sha256: digested.sha256,
            local_offset,
            mode: DistributionFileMode::Data,
            captured: digested.bytes,
        });
    }
    if records.windows(2).any(|pair| pair[0].name >= pair[1].name) {
        return Err(invalid_archive());
    }
    let central_offset = u32::try_from(input.stream_position().map_err(|_| invalid_archive())?)
        .map_err(|_| invalid_archive())?;
    let central_end = validate_central_records(&mut input, &mut records)?;
    validate_archive_end(&mut input, archive.length, central_offset, central_end)?;
    validate_embedded_bundle(&records, manifest)
}

fn validate_central_records(
    input: &mut File,
    records: &mut [ZipRecord],
) -> Result<u32, ReleaseFailure> {
    for record in records {
        if read_u32(input)? != ZIP_CENTRAL_SIGNATURE
            || read_u16(input)? != ZIP_UNIX_VERSION
            || read_u16(input)? != ZIP_VERSION
            || read_u16(input)? != 0
            || read_u16(input)? != 0
            || read_u16(input)? != 0
            || read_u16(input)? != ZIP_DOS_DATE
            || read_u32(input)? != record.crc32
            || read_u32(input)? != record.length
            || read_u32(input)? != record.length
        {
            return Err(invalid_archive());
        }
        let name_length = read_u16(input)?;
        let extra_length = read_u16(input)?;
        let comment_length = read_u16(input)?;
        let disk = read_u16(input)?;
        let internal_attributes = read_u16(input)?;
        let external_attributes = read_u32(input)?;
        let local_offset = read_u32(input)?;
        if usize::from(name_length) > MAX_LOCAL_RELEASE_RELATIVE_PATH_BYTES {
            return Err(invalid_archive());
        }
        let central_name = read_utf8(input, usize::from(name_length))?;
        if central_name != record.name
            || extra_length != 0
            || comment_length != 0
            || disk != 0
            || internal_attributes != 0
            || local_offset != record.local_offset
        {
            return Err(invalid_archive());
        }
        record.mode = match external_attributes {
            ZIP_EXECUTABLE_MODE => DistributionFileMode::Executable,
            ZIP_DATA_MODE => DistributionFileMode::Data,
            _ => return Err(invalid_archive()),
        };
    }
    u32::try_from(input.stream_position().map_err(|_| invalid_archive())?)
        .map_err(|_| invalid_archive())
}

fn validate_archive_end(
    input: &mut File,
    archive_length: u64,
    central_offset: u32,
    central_end: u32,
) -> Result<(), ReleaseFailure> {
    if read_u32(input)? != ZIP_END_SIGNATURE
        || read_u16(input)? != 0
        || read_u16(input)? != 0
        || read_u16(input)? != ZIP_ENTRY_COUNT
        || read_u16(input)? != ZIP_ENTRY_COUNT
        || read_u32(input)? != central_end - central_offset
        || read_u32(input)? != central_offset
        || read_u16(input)? != 0
    {
        return Err(invalid_archive());
    }
    let end = input.stream_position().map_err(|_| invalid_archive())?;
    if end != archive_length || read_one(input)?.is_some() {
        return Err(invalid_archive());
    }
    Ok(())
}

fn validate_embedded_bundle(
    records: &[ZipRecord],
    outer: &LocalReleaseCandidateManifestV1,
) -> Result<(), ReleaseFailure> {
    if records.len() != G1_FILE_COUNT {
        return Err(invalid_archive());
    }
    let embedded_manifest = records
        .iter()
        .find(|record| record.relative == "manifest.json")
        .and_then(|record| record.captured.as_deref())
        .ok_or_else(invalid_archive)?;
    if sha256_hex(embedded_manifest) != outer.bundle().manifest_sha256() {
        return Err(invalid_archive());
    }
    let manifest = validate_local_distribution_manifest_v1(embedded_manifest)
        .map_err(|_| invalid_archive())?;
    if manifest.target() != outer.target()
        || manifest.binary_sha256() != outer.bundle().binary_sha256()
        || local_distribution_bundle_name(manifest.target(), manifest.binary_sha256())
            != outer.bundle().name()
    {
        return Err(invalid_archive());
    }
    let mut expected = BTreeMap::new();
    expected.insert(
        "manifest.json".to_owned(),
        (
            u64::try_from(embedded_manifest.len()).unwrap_or(u64::MAX),
            outer.bundle().manifest_sha256().to_owned(),
            DistributionFileMode::Data,
        ),
    );
    for file in manifest.files() {
        expected.insert(
            file.path().to_owned(),
            (file.length(), file.sha256().to_owned(), file.mode()),
        );
    }
    for record in records {
        let Some((length, sha256, mode)) = expected.get(&record.relative) else {
            return Err(invalid_archive());
        };
        if u64::from(record.length) != *length || record.sha256 != *sha256 || record.mode != *mode {
            return Err(invalid_archive());
        }
    }
    Ok(())
}

fn copy_exact(
    source: &mut File,
    destination: &mut File,
    length: u64,
) -> Result<DigestedInput, ReleaseFailure> {
    let mut remaining = length;
    let mut sha256 = Sha256::new();
    let mut crc32 = Crc32::new();
    let mut buffer = [0_u8; 16_384];
    while remaining > 0 {
        let maximum = usize::try_from(remaining.min(buffer.len() as u64)).unwrap_or(buffer.len());
        let read = source
            .read(&mut buffer[..maximum])
            .map_err(|_| unstable_input())?;
        if read == 0 {
            return Err(unstable_input());
        }
        destination
            .write_all(&buffer[..read])
            .map_err(|_| ReleaseFailure::internal())?;
        sha256.update(&buffer[..read]);
        crc32.update(&buffer[..read]);
        remaining -= u64::try_from(read).unwrap_or(u64::MAX);
    }
    if source.read(&mut [0_u8; 1]).map_err(|_| unstable_input())? != 0 {
        return Err(unstable_input());
    }
    Ok(DigestedInput {
        length,
        sha256: encode_digest(sha256.finalize()),
        crc32: crc32.finalize(),
        bytes: None,
    })
}

fn digest_limited_reader(
    reader: &mut File,
    length: u64,
    capture: bool,
) -> Result<DigestedInput, ReleaseFailure> {
    let capacity = usize::try_from(length.min(65_536)).unwrap_or(65_536);
    let mut bytes = capture.then(|| Vec::with_capacity(capacity));
    let mut sha256 = Sha256::new();
    let mut crc32 = Crc32::new();
    let mut remaining = length;
    let mut buffer = [0_u8; 16_384];
    while remaining > 0 {
        let maximum = usize::try_from(remaining.min(buffer.len() as u64)).unwrap_or(buffer.len());
        let read = reader
            .read(&mut buffer[..maximum])
            .map_err(|_| invalid_archive())?;
        if read == 0 {
            return Err(invalid_archive());
        }
        sha256.update(&buffer[..read]);
        crc32.update(&buffer[..read]);
        if let Some(bytes) = &mut bytes {
            bytes.extend_from_slice(&buffer[..read]);
        }
        remaining -= u64::try_from(read).unwrap_or(u64::MAX);
    }
    Ok(DigestedInput {
        length,
        sha256: encode_digest(sha256.finalize()),
        crc32: crc32.finalize(),
        bytes,
    })
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), ReleaseFailure> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| ReleaseFailure::internal())?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| ReleaseFailure::internal())?;
    set_mode(path, DistributionFileMode::Data)
}

fn write_u16(writer: &mut File, value: u16) -> Result<(), ReleaseFailure> {
    writer
        .write_all(&value.to_le_bytes())
        .map_err(|_| ReleaseFailure::internal())
}

fn write_u32(writer: &mut File, value: u32) -> Result<(), ReleaseFailure> {
    writer
        .write_all(&value.to_le_bytes())
        .map_err(|_| ReleaseFailure::internal())
}

fn read_u16(reader: &mut File) -> Result<u16, ReleaseFailure> {
    let mut bytes = [0_u8; 2];
    reader
        .read_exact(&mut bytes)
        .map_err(|_| invalid_archive())?;
    Ok(u16::from_le_bytes(bytes))
}

fn read_u32(reader: &mut File) -> Result<u32, ReleaseFailure> {
    let mut bytes = [0_u8; 4];
    reader
        .read_exact(&mut bytes)
        .map_err(|_| invalid_archive())?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_utf8(reader: &mut File, length: usize) -> Result<String, ReleaseFailure> {
    let mut bytes = vec![0_u8; length];
    reader
        .read_exact(&mut bytes)
        .map_err(|_| invalid_archive())?;
    String::from_utf8(bytes).map_err(|_| invalid_archive())
}

fn read_one(reader: &mut File) -> Result<Option<u8>, ReleaseFailure> {
    let mut byte = [0_u8; 1];
    match reader.read(&mut byte).map_err(|_| invalid_archive())? {
        0 => Ok(None),
        1 => Ok(Some(byte[0])),
        _ => Err(invalid_archive()),
    }
}

fn archive_relative_path(name: &str, bundle_name: &str) -> Result<String, ReleaseFailure> {
    let prefix = format!("{bundle_name}/");
    if name.len() > MAX_LOCAL_RELEASE_RELATIVE_PATH_BYTES
        || name.contains('\\')
        || !name.starts_with(&prefix)
    {
        return Err(invalid_archive());
    }
    let relative = &name[prefix.len()..];
    strict_relative_path(relative, LocalReleaseContractError::InvalidArchive)?;
    Ok(relative.to_owned())
}

fn strict_relative_path(
    path: &str,
    failure: LocalReleaseContractError,
) -> Result<&Path, ReleaseFailure> {
    if path.is_empty() || path.len() > MAX_LOCAL_RELEASE_RELATIVE_PATH_BYTES || path.contains('\\')
    {
        return Err(contract_failure(failure));
    }
    let path = Path::new(path);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(contract_failure(failure));
    }
    Ok(path)
}

fn g1_tree(
    files: &[codenoesis_contracts::LocalDistributionFileV1],
) -> Result<BTreeSet<String>, ReleaseFailure> {
    let mut expected = BTreeSet::new();
    add_expected_file(
        &mut expected,
        Path::new("manifest.json"),
        LocalReleaseContractError::InvalidBundle,
    )?;
    for file in files {
        add_expected_file(
            &mut expected,
            strict_relative_path(file.path(), LocalReleaseContractError::InvalidBundle)?,
            LocalReleaseContractError::InvalidBundle,
        )?;
    }
    Ok(expected)
}

fn candidate_tree(archive_name: &str) -> BTreeSet<String> {
    let mut expected = BTreeSet::from([
        format!("f:{archive_name}"),
        "f:checksums.sha256".to_owned(),
        "f:manifest.json".to_owned(),
        "d:evidence".to_owned(),
    ]);
    for (_, path) in EVIDENCE_INPUTS {
        expected.insert(format!("f:{path}"));
    }
    expected
}

fn add_expected_file(
    expected: &mut BTreeSet<String>,
    relative: &Path,
    failure: LocalReleaseContractError,
) -> Result<(), ReleaseFailure> {
    expected.insert(format!("f:{}", relative_path(relative, failure)?));
    let mut parent = relative.parent();
    while let Some(directory) = parent {
        if directory.as_os_str().is_empty() {
            break;
        }
        expected.insert(format!("d:{}", relative_path(directory, failure)?));
        parent = directory.parent();
    }
    Ok(())
}

fn collect_tree(
    root: &Path,
    maximum: usize,
    failure: LocalReleaseContractError,
) -> Result<BTreeSet<String>, ReleaseFailure> {
    let mut directories = vec![root.to_path_buf()];
    let mut entries = BTreeSet::new();
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(&directory).map_err(|_| contract_failure(failure))? {
            let entry = entry.map_err(|_| contract_failure(failure))?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).map_err(|_| contract_failure(failure))?;
            if metadata.file_type().is_symlink() {
                return Err(contract_failure(failure));
            }
            let relative = path
                .strip_prefix(root)
                .map_err(|_| contract_failure(failure))?;
            let relative = relative_path(relative, failure)?;
            let record = if metadata.is_dir() {
                directories.push(path);
                format!("d:{relative}")
            } else if metadata.is_file() {
                format!("f:{relative}")
            } else {
                return Err(contract_failure(failure));
            };
            entries.insert(record);
            if entries.len() > maximum {
                return Err(limit_exceeded());
            }
        }
    }
    Ok(entries)
}

fn relative_path(
    path: &Path,
    failure: LocalReleaseContractError,
) -> Result<String, ReleaseFailure> {
    path.iter()
        .map(OsStr::to_str)
        .collect::<Option<Vec<_>>>()
        .map(|components| components.join("/"))
        .filter(|value| {
            !value.is_empty()
                && value.len() <= MAX_LOCAL_RELEASE_RELATIVE_PATH_BYTES
                && !value.contains('\\')
        })
        .ok_or_else(|| contract_failure(failure))
}

fn ensure_absent(path: &Path) -> Result<(), ReleaseFailure> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(invalid_arguments()),
        Err(_) => Err(ReleaseFailure::internal()),
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

fn zip_external_mode(mode: DistributionFileMode) -> u32 {
    match mode {
        DistributionFileMode::Executable => ZIP_EXECUTABLE_MODE,
        DistributionFileMode::Data => ZIP_DATA_MODE,
    }
}

fn is_git_sha(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn map_manifest_error(
    error: LocalReleaseContractError,
    fallback: LocalReleaseContractError,
) -> ReleaseFailure {
    if error == LocalReleaseContractError::LimitExceeded {
        limit_exceeded()
    } else {
        contract_failure(fallback)
    }
}

fn contract_failure(error: LocalReleaseContractError) -> ReleaseFailure {
    ReleaseFailure::from_contract(error)
}

fn invalid_arguments() -> ReleaseFailure {
    ReleaseFailure::new(CodeNoesisErrorV28::invalid_arguments(), 2)
}

fn invalid_bundle() -> ReleaseFailure {
    ReleaseFailure::new(CodeNoesisErrorV28::invalid_bundle(), 2)
}

fn invalid_evidence() -> ReleaseFailure {
    ReleaseFailure::new(CodeNoesisErrorV28::invalid_evidence(), 2)
}

fn invalid_archive() -> ReleaseFailure {
    ReleaseFailure::new(CodeNoesisErrorV28::invalid_archive(), 2)
}

fn unstable_input() -> ReleaseFailure {
    ReleaseFailure::new(CodeNoesisErrorV28::unstable_input(), 2)
}

fn limit_exceeded() -> ReleaseFailure {
    ReleaseFailure::new(CodeNoesisErrorV28::limit_exceeded(), 2)
}

#[cfg(unix)]
fn set_private_directory(path: &Path) -> Result<(), ReleaseFailure> {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|_| ReleaseFailure::internal())
}

#[cfg(not(unix))]
fn set_private_directory(path: &Path) -> Result<(), ReleaseFailure> {
    let mut permissions = fs::metadata(path)
        .map_err(|_| ReleaseFailure::internal())?
        .permissions();
    permissions.set_readonly(false);
    fs::set_permissions(path, permissions).map_err(|_| ReleaseFailure::internal())
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: DistributionFileMode) -> Result<(), ReleaseFailure> {
    use std::os::unix::fs::PermissionsExt as _;
    let bits = match mode {
        DistributionFileMode::Executable => 0o755,
        DistributionFileMode::Data => 0o644,
    };
    fs::set_permissions(path, fs::Permissions::from_mode(bits))
        .map_err(|_| ReleaseFailure::internal())
}

#[cfg(not(unix))]
fn set_mode(path: &Path, _mode: DistributionFileMode) -> Result<(), ReleaseFailure> {
    let mut permissions = fs::metadata(path)
        .map_err(|_| ReleaseFailure::internal())?
        .permissions();
    permissions.set_readonly(false);
    fs::set_permissions(path, permissions).map_err(|_| ReleaseFailure::internal())
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
        && left.mode() == right.mode()
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    const SOURCE_COMMIT: &str = "c5d259d7689b8a49527f8322b606e58cc0e1e61d";
    const CARGO_LOCK_SHA256: &str =
        "434cc5e8e38a4c57f35990431d4682974b6cae94893860e1948c8f7cc21ffbca";
    const SERDE_JSON_SHA256: &str =
        "c841b55ecdae098c80dcae9cf767f6f8a0c2cdb3416bbef72181df4d0fe73f14";
    const BINARY: &[u8] = include_bytes!(
        "../../tests/specifications/g1/local-cli-distribution-v1/fixtures/noesis-v1.bin"
    );

    #[test]
    fn sec_fr_cli_010_invalid_arguments_and_nonempty_output_fail_closed() {
        let failure = run([
            OsString::from("xtask"),
            OsString::from("package-local-release-candidate"),
        ])
        .unwrap_err();
        assert_code(&failure, "release.invalid_arguments");

        let root = tempfile::tempdir().expect("temporary root");
        let output = root.path().join("output");
        fs::create_dir(&output).expect("create output");
        fs::write(output.join("private-credential-canary"), b"private")
            .expect("write existing output");
        let failure = run(package_arguments(
            Path::new("missing"),
            Path::new("missing"),
            &output,
            SOURCE_COMMIT,
        ))
        .unwrap_err();
        assert_code(&failure, "release.invalid_arguments");
        assert_eq!(fs::read_dir(output).unwrap().count(), 1);
    }

    #[test]
    fn sec_fr_rel_003_candidate_tree_and_archive_tamper_fail_closed() {
        let Some(fixture) = CandidateFixture::new() else {
            return;
        };
        fs::write(fixture.candidate.join("unexpected"), b"private")
            .expect("write unexpected candidate file");
        let failure = run(verify_arguments(&fixture.candidate)).unwrap_err();
        assert_code(&failure, "release.invalid_archive");

        let Some(fixture) = CandidateFixture::new() else {
            return;
        };
        let manifest_bytes =
            fs::read(fixture.candidate.join("manifest.json")).expect("read candidate manifest");
        let manifest = parse_local_release_candidate_manifest_v1(&manifest_bytes)
            .expect("parse candidate manifest");
        let archive_path = fixture.candidate.join(manifest.archive().path());
        let mut archive_bytes = fs::read(&archive_path).expect("read archive");
        let central = archive_bytes
            .windows(4)
            .position(|window| window == ZIP_CENTRAL_SIGNATURE.to_le_bytes())
            .expect("central directory");
        archive_bytes[central + 6] ^= 1;
        fs::write(&archive_path, archive_bytes).expect("tamper central record");
        let archive = StableFile::read(
            &archive_path,
            MAX_LOCAL_RELEASE_ARCHIVE_BYTES,
            false,
            LocalReleaseContractError::InvalidArchive,
        )
        .expect("stable tampered archive");
        let failure = validate_archive(&archive, &manifest).unwrap_err();
        assert_code(&failure, "release.invalid_archive");
    }

    #[test]
    fn sec_nfr_sup_001_vulnerability_rejects_without_partial_candidate() {
        if codenoesis_contracts::current_local_distribution_target() == "unsupported-compile-target"
        {
            return;
        }
        let root = tempfile::tempdir().expect("temporary root");
        let bundle = package_g1(root.path());
        let supply = root.path().join("supply");
        fs::create_dir(&supply).expect("create supply");
        write_supply(&supply, true);
        let output = root.path().join("output");
        fs::create_dir(&output).expect("create output");
        let failure = run(package_arguments(&bundle, &supply, &output, SOURCE_COMMIT)).unwrap_err();
        assert_code(&failure, "release.policy_rejected");
        assert_eq!(fs::read_dir(output).unwrap().count(), 0);
    }

    struct CandidateFixture {
        _root: tempfile::TempDir,
        candidate: PathBuf,
    }

    impl CandidateFixture {
        fn new() -> Option<Self> {
            if codenoesis_contracts::current_local_distribution_target()
                == "unsupported-compile-target"
            {
                return None;
            }
            let root = tempfile::tempdir().expect("temporary root");
            let bundle = package_g1(root.path());
            let supply = root.path().join("supply");
            fs::create_dir(&supply).expect("create supply");
            write_supply(&supply, false);
            let output = root.path().join("output");
            fs::create_dir(&output).expect("create output");
            run(package_arguments(&bundle, &supply, &output, SOURCE_COMMIT))
                .expect("package candidate");
            let candidate = only_directory(&output);
            Some(Self {
                _root: root,
                candidate,
            })
        }
    }

    fn package_g1(root: &Path) -> PathBuf {
        let binary = root.join("noesis");
        fs::write(&binary, BINARY).expect("write binary fixture");
        let output = root.join("g1");
        fs::create_dir(&output).expect("create G1 output");
        crate::distribution::run([
            OsString::from("xtask"),
            OsString::from("package-local-cli"),
            OsString::from("--binary"),
            binary.into_os_string(),
            OsString::from("--output"),
            output.clone().into_os_string(),
        ])
        .expect("package G1 fixture");
        only_directory(&output)
    }

    fn only_directory(root: &Path) -> PathBuf {
        let entries = fs::read_dir(root)
            .expect("read root")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect root");
        assert_eq!(entries.len(), 1);
        entries[0].path()
    }

    fn package_arguments(
        bundle: &Path,
        supply: &Path,
        output: &Path,
        source_commit: &str,
    ) -> Vec<OsString> {
        [
            OsString::from("xtask"),
            OsString::from("package-local-release-candidate"),
            OsString::from("--bundle"),
            bundle.as_os_str().to_owned(),
            OsString::from("--source-commit"),
            OsString::from(source_commit),
            OsString::from("--supply-chain"),
            supply.as_os_str().to_owned(),
            OsString::from("--output"),
            output.as_os_str().to_owned(),
        ]
        .into()
    }

    fn verify_arguments(candidate: &Path) -> Vec<OsString> {
        [
            OsString::from("xtask"),
            OsString::from("verify-local-release-candidate"),
            OsString::from("--candidate"),
            candidate.as_os_str().to_owned(),
        ]
        .into()
    }

    fn write_supply(root: &Path, vulnerable: bool) {
        write_json(root.join("advisory-report.json"), &advisory(vulnerable));
        write_json(root.join("dependency-lock.json"), &dependency());
        write_json(root.join("license-report.json"), &license());
        write_json(root.join("unsafe-inventory.json"), &unsafe_inventory());
        write_json(root.join("sbom.cdx.json"), &sbom());
    }

    fn write_json(path: PathBuf, value: &Value) {
        let mut bytes = serde_json::to_vec(&value).expect("serialize supply fixture");
        bytes.push(b'\n');
        fs::write(path, bytes).expect("write supply fixture");
    }

    fn advisory(vulnerable: bool) -> Value {
        let vulnerabilities = if vulnerable {
            vec![json!({"id": "RUSTSEC-TEST"})]
        } else {
            Vec::new()
        };
        json!({
            "schema_version": "codenoesis.local-advisory-report/v1",
            "cargo_lock_sha256": CARGO_LOCK_SHA256,
            "tool": {"name": "cargo-audit", "version": "0.22.2"},
            "database": {"commit": "69f93e1d081d8b6fbee010e48f0b5e0d13661415", "updated": "2026-08-12T12:42:29+02:00"},
            "status": "accepted",
            "vulnerabilities": vulnerabilities,
            "warnings": []
        })
    }

    fn dependency() -> Value {
        json!({
            "schema_version": "codenoesis.local-dependency-lock/v1",
            "target": codenoesis_contracts::current_local_distribution_target(),
            "root": "noesis@0.1.0",
            "cargo_lock_sha256": CARGO_LOCK_SHA256,
            "packages": [
                {"id": "noesis@0.1.0", "name": "noesis", "version": "0.1.0", "source": "workspace", "checksum": null, "dependencies": ["serde_json@1.0.151"]},
                {"id": "serde_json@1.0.151", "name": "serde_json", "version": "1.0.151", "source": "registry+https://github.com/rust-lang/crates.io-index", "checksum": SERDE_JSON_SHA256, "dependencies": []}
            ],
            "dependency_edges": 1
        })
    }

    fn license() -> Value {
        json!({
            "schema_version": "codenoesis.local-license-report/v1",
            "target": codenoesis_contracts::current_local_distribution_target(),
            "cargo_lock_sha256": CARGO_LOCK_SHA256,
            "policy": "codenoesis.local-release-policy/v1",
            "status": "accepted",
            "packages": [
                {"id": "noesis@0.1.0", "expression": "Apache-2.0", "decision": "allowed"},
                {"id": "serde_json@1.0.151", "expression": "MIT OR Apache-2.0", "decision": "allowed"}
            ],
            "exceptions": []
        })
    }

    fn unsafe_inventory() -> Value {
        json!({
            "schema_version": "codenoesis.local-unsafe-inventory/v1",
            "target": codenoesis_contracts::current_local_distribution_target(),
            "cargo_lock_sha256": CARGO_LOCK_SHA256,
            "method": "conservative-rust-token-scan-v1",
            "status": "accepted",
            "packages": [
                {"id": "noesis@0.1.0", "rust_files": 1, "unsafe_tokens": 0, "exception_id": null},
                {"id": "serde_json@1.0.151", "rust_files": 69, "unsafe_tokens": 20, "exception_id": "unsafe-serde-json-1-0-151"}
            ],
            "exceptions": [
                {"id": "unsafe-serde-json-1-0-151", "package": "serde_json", "version": "1.0.151", "owner": "@smutti", "expires_on": "2026-11-14"}
            ]
        })
    }

    fn sbom() -> Value {
        let target = codenoesis_contracts::current_local_distribution_target();
        json!({
            "bomFormat": "CycloneDX",
            "specVersion": "1.6",
            "serialNumber": "urn:uuid:00000000-0000-5000-8000-000000000000",
            "version": 1,
            "metadata": {
                "component": {"type": "application", "bom-ref": "pkg:cargo/noesis@0.1.0", "name": "noesis", "version": "0.1.0"},
                "properties": [
                    {"name": "codenoesis:cargo-lock-sha256", "value": CARGO_LOCK_SHA256},
                    {"name": "codenoesis:target", "value": target}
                ]
            },
            "components": [
                {"type": "library", "bom-ref": "pkg:cargo/serde_json@1.0.151", "name": "serde_json", "version": "1.0.151", "hashes": [{"alg": "SHA-256", "content": SERDE_JSON_SHA256}], "licenses": [{"expression": "MIT OR Apache-2.0"}], "purl": "pkg:cargo/serde_json@1.0.151"}
            ],
            "dependencies": [
                {"ref": "pkg:cargo/noesis@0.1.0", "dependsOn": ["pkg:cargo/serde_json@1.0.151"]},
                {"ref": "pkg:cargo/serde_json@1.0.151", "dependsOn": []}
            ]
        })
    }

    fn assert_code(failure: &ReleaseFailure, expected: &str) {
        assert_eq!(failure.error().value()["code"], expected);
        let stderr = failure.error().canonical_stderr().unwrap();
        assert!(!stderr.windows(10).any(|window| window == b"credential"));
    }
}
