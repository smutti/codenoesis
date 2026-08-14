//! Read-only local upgrade and rollback compatibility preflights.

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File, Metadata};
use std::io::Read as _;
use std::path::{Component, Path, PathBuf};

use codenoesis_contracts::{
    CodeNoesisErrorV27, DistributionFileMode, LocalBundleIdentityV1, LocalRollbackReportV1,
    LocalUpgradeContractError, LocalUpgradePlanV1, MAX_LOCAL_DISTRIBUTION_BINARY_BYTES,
    MAX_LOCAL_DISTRIBUTION_MANIFEST_BYTES, MAX_LOCAL_UPGRADE_PLAN_BYTES,
    parse_local_upgrade_plan_v1, validate_local_distribution_manifest_v1,
};
use same_file::Handle as FileIdentity;
use sha2::{Digest as _, Sha256};

const MAX_TREE_ENTRIES: usize = 32;

/// One strict compatibility-command failure and its public exit status.
#[derive(Clone, Debug)]
pub struct UpgradeFailure {
    error: CodeNoesisErrorV27,
    exit_code: u8,
}

impl UpgradeFailure {
    /// Returns an internal compatibility failure.
    #[must_use]
    pub fn internal() -> Self {
        Self::new(CodeNoesisErrorV27::internal(), 1)
    }

    /// Returns the strict public `ErrorV27` value.
    #[must_use]
    pub const fn error(&self) -> &CodeNoesisErrorV27 {
        &self.error
    }

    /// Returns the public process exit status.
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        self.exit_code
    }

    fn from_contract(error: LocalUpgradeContractError) -> Self {
        let exit_code = if error == LocalUpgradeContractError::ContractInvalid {
            1
        } else {
            2
        };
        Self::new(CodeNoesisErrorV27::from_contract(error), exit_code)
    }

    const fn new(error: CodeNoesisErrorV27, exit_code: u8) -> Self {
        Self { error, exit_code }
    }
}

/// Returns whether one command name belongs to the additive G2 adapter.
#[must_use]
pub fn is_upgrade_command(command: Option<&OsString>) -> bool {
    command.is_some_and(|command| {
        command == "preflight-local-upgrade" || command == "preflight-local-rollback"
    })
}

/// Executes one bounded output-only local transition preflight.
///
/// # Errors
///
/// Returns a privacy-safe `ErrorV27` failure for invalid arguments, bundles,
/// plans, races, limits, incompatibility, or internal serialization failure.
pub fn run(arguments: impl IntoIterator<Item = OsString>) -> Result<Vec<u8>, UpgradeFailure> {
    match Invocation::parse(&arguments.into_iter().collect::<Vec<_>>())? {
        Invocation::Upgrade { current, candidate } => preflight_upgrade(&current, &candidate),
        Invocation::Rollback {
            plan,
            current,
            target,
        } => preflight_rollback(&plan, &current, &target),
    }
}

fn preflight_upgrade(current: &Path, candidate: &Path) -> Result<Vec<u8>, UpgradeFailure> {
    let current = StableBundle::inspect(current)?;
    let candidate = StableBundle::inspect(candidate)?;
    if current.root.same_file(&candidate.root) || current.target != candidate.target {
        return Err(incompatible());
    }
    current.revalidate()?;
    candidate.revalidate()?;
    let plan = LocalUpgradePlanV1::new(
        &current.target,
        current.identity.clone(),
        candidate.identity.clone(),
    )
    .map_err(UpgradeFailure::from_contract)?;
    let stdout = plan
        .canonical_stdout()
        .map_err(UpgradeFailure::from_contract)?;
    current.revalidate()?;
    candidate.revalidate()?;
    Ok(stdout)
}

fn preflight_rollback(
    plan_path: &Path,
    current: &Path,
    target: &Path,
) -> Result<Vec<u8>, UpgradeFailure> {
    let stable_plan = StableFile::read(
        plan_path,
        u64::try_from(MAX_LOCAL_UPGRADE_PLAN_BYTES).unwrap_or(u64::MAX),
        true,
    )?;
    let plan_bytes = stable_plan.bytes().ok_or_else(UpgradeFailure::internal)?;
    let plan = parse_local_upgrade_plan_v1(plan_bytes).map_err(UpgradeFailure::from_contract)?;
    let current = StableBundle::inspect(current)?;
    let target = StableBundle::inspect(target)?;
    if current.root.same_file(&target.root)
        || current.target != target.target
        || current.target != plan.target()
    {
        return Err(incompatible());
    }
    stable_plan.revalidate()?;
    current.revalidate()?;
    target.revalidate()?;
    let report = LocalRollbackReportV1::new(
        &plan,
        &current.identity,
        &target.identity,
        stable_plan.sha256(),
    )
    .map_err(UpgradeFailure::from_contract)?;
    let stdout = report
        .canonical_stdout()
        .map_err(UpgradeFailure::from_contract)?;
    stable_plan.revalidate()?;
    current.revalidate()?;
    target.revalidate()?;
    Ok(stdout)
}

enum Invocation {
    Upgrade {
        current: PathBuf,
        candidate: PathBuf,
    },
    Rollback {
        plan: PathBuf,
        current: PathBuf,
        target: PathBuf,
    },
}

impl Invocation {
    fn parse(arguments: &[OsString]) -> Result<Self, UpgradeFailure> {
        match arguments.get(1).and_then(|value| value.to_str()) {
            Some("preflight-local-upgrade") if arguments.len() == 6 => {
                let pairs = parse_pairs(&arguments[2..], &["--current", "--candidate"])?;
                Ok(Self::Upgrade {
                    current: pairs[0].clone(),
                    candidate: pairs[1].clone(),
                })
            }
            Some("preflight-local-rollback") if arguments.len() == 8 => {
                let pairs = parse_pairs(&arguments[2..], &["--plan", "--current", "--target"])?;
                Ok(Self::Rollback {
                    plan: pairs[0].clone(),
                    current: pairs[1].clone(),
                    target: pairs[2].clone(),
                })
            }
            _ => Err(invalid_arguments()),
        }
    }
}

fn parse_pairs(arguments: &[OsString], flags: &[&str]) -> Result<Vec<PathBuf>, UpgradeFailure> {
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
        if values[index].replace(PathBuf::from(&pair[1])).is_some() {
            return Err(invalid_arguments());
        }
    }
    values
        .into_iter()
        .map(|value| value.ok_or_else(invalid_arguments))
        .collect()
}

struct StableBundle {
    root: StableRoot,
    files: Vec<StableFile>,
    target: String,
    identity: LocalBundleIdentityV1,
}

impl StableBundle {
    fn inspect(path: &Path) -> Result<Self, UpgradeFailure> {
        let root = StableRoot::inspect(path)?;
        let tree_before = collect_tree(path)?;
        let manifest_file = StableFile::read(
            &path.join("manifest.json"),
            u64::try_from(MAX_LOCAL_DISTRIBUTION_MANIFEST_BYTES).unwrap_or(u64::MAX),
            true,
        )?;
        if !mode_matches(manifest_file.metadata(), DistributionFileMode::Data) {
            return Err(invalid_bundle());
        }
        let manifest = validate_local_distribution_manifest_v1(
            manifest_file.bytes().ok_or_else(UpgradeFailure::internal)?,
        )
        .map_err(UpgradeFailure::from_contract)?;
        let expected_tree = expected_tree(manifest.files())?;
        if tree_before != expected_tree {
            return Err(invalid_bundle());
        }

        let mut files = Vec::with_capacity(manifest.files().len() + 1);
        files.push(manifest_file);
        for expected in manifest.files() {
            let relative = strict_relative_path(expected.path())?;
            let maximum = if expected.mode() == DistributionFileMode::Executable {
                MAX_LOCAL_DISTRIBUTION_BINARY_BYTES
            } else {
                expected.length()
            };
            let file = StableFile::read(&path.join(relative), maximum, false)?;
            if file.length() != expected.length()
                || file.sha256() != expected.sha256()
                || !mode_matches(file.metadata(), expected.mode())
            {
                return Err(invalid_bundle());
            }
            files.push(file);
        }
        if collect_tree(path)? != expected_tree {
            return Err(unstable_input());
        }
        root.revalidate()?;
        for file in &files {
            file.revalidate()?;
        }

        let bundle_name = path
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or_else(invalid_bundle)?;
        let identity = LocalBundleIdentityV1::new(
            manifest.target(),
            manifest.binary_sha256(),
            bundle_name,
            files[0].sha256(),
        )
        .map_err(UpgradeFailure::from_contract)?;
        Ok(Self {
            root,
            files,
            target: manifest.target().to_owned(),
            identity,
        })
    }

    fn revalidate(&self) -> Result<(), UpgradeFailure> {
        self.root.revalidate()?;
        for file in &self.files {
            file.revalidate()?;
        }
        Ok(())
    }
}

struct StableRoot {
    path: PathBuf,
    identity: FileIdentity,
    metadata: Metadata,
}

impl StableRoot {
    fn inspect(path: &Path) -> Result<Self, UpgradeFailure> {
        let metadata = fs::symlink_metadata(path).map_err(|_| invalid_bundle())?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(invalid_bundle());
        }
        let identity = FileIdentity::from_path(path).map_err(|_| invalid_bundle())?;
        Ok(Self {
            path: path.to_path_buf(),
            identity,
            metadata,
        })
    }

    fn same_file(&self, other: &Self) -> bool {
        self.identity == other.identity
    }

    fn revalidate(&self) -> Result<(), UpgradeFailure> {
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
    bytes: Option<Vec<u8>>,
}

impl StableFile {
    fn read(path: &Path, maximum: u64, capture: bool) -> Result<Self, UpgradeFailure> {
        let path_before = fs::symlink_metadata(path).map_err(|_| invalid_bundle())?;
        if path_before.file_type().is_symlink() || !path_before.is_file() {
            return Err(invalid_bundle());
        }
        if path_before.len() > maximum {
            return Err(limit_exceeded());
        }
        let mut file = File::open(path).map_err(|_| invalid_bundle())?;
        let before = file.metadata().map_err(|_| invalid_bundle())?;
        let identity = FileIdentity::from_file(file.try_clone().map_err(|_| invalid_bundle())?)
            .map_err(|_| invalid_bundle())?;
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
            || first.length != second.length
            || first.sha256 != second.sha256
            || first.bytes != second.bytes
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
            bytes: first.bytes,
        })
    }

    fn revalidate(&self) -> Result<(), UpgradeFailure> {
        let metadata = fs::symlink_metadata(&self.path).map_err(|_| unstable_input())?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || !same_file_metadata(&self.metadata, &metadata)
            || !FileIdentity::from_path(&self.path).is_ok_and(|identity| identity == self.identity)
        {
            return Err(unstable_input());
        }
        Ok(())
    }

    const fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    const fn length(&self) -> u64 {
        self.length
    }

    fn sha256(&self) -> &str {
        &self.sha256
    }

    fn bytes(&self) -> Option<&[u8]> {
        self.bytes.as_deref()
    }
}

struct DigestedInput {
    length: u64,
    sha256: String,
    bytes: Option<Vec<u8>>,
}

fn digest_reader(
    reader: &mut File,
    maximum: u64,
    capture: bool,
) -> Result<DigestedInput, UpgradeFailure> {
    let capacity = if capture {
        usize::try_from(maximum.min(65_536)).unwrap_or(65_536)
    } else {
        0
    };
    let mut bytes = capture.then(|| Vec::with_capacity(capacity));
    let mut hasher = Sha256::new();
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
        hasher.update(&buffer[..read]);
        if let Some(bytes) = &mut bytes {
            bytes.extend_from_slice(&buffer[..read]);
        }
    }
    Ok(DigestedInput {
        length,
        sha256: encode_digest(hasher.finalize()),
        bytes,
    })
}

fn expected_tree(
    files: &[codenoesis_contracts::LocalDistributionFileV1],
) -> Result<BTreeSet<String>, UpgradeFailure> {
    let mut expected = BTreeSet::new();
    add_expected_file(&mut expected, Path::new("manifest.json"))?;
    for file in files {
        add_expected_file(&mut expected, strict_relative_path(file.path())?)?;
    }
    Ok(expected)
}

fn add_expected_file(
    expected: &mut BTreeSet<String>,
    relative: &Path,
) -> Result<(), UpgradeFailure> {
    expected.insert(format!("f:{}", relative_path(relative)?));
    let mut parent = relative.parent();
    while let Some(directory) = parent {
        if directory.as_os_str().is_empty() {
            break;
        }
        expected.insert(format!("d:{}", relative_path(directory)?));
        parent = directory.parent();
    }
    Ok(())
}

fn collect_tree(root: &Path) -> Result<BTreeSet<String>, UpgradeFailure> {
    let mut directories = vec![root.to_path_buf()];
    let mut entries = BTreeSet::new();
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(&directory).map_err(|_| invalid_bundle())? {
            let entry = entry.map_err(|_| invalid_bundle())?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).map_err(|_| invalid_bundle())?;
            if metadata.file_type().is_symlink() {
                return Err(invalid_bundle());
            }
            let relative = path.strip_prefix(root).map_err(|_| invalid_bundle())?;
            let relative = relative_path(relative)?;
            let record = if metadata.is_dir() {
                directories.push(path);
                format!("d:{relative}")
            } else if metadata.is_file() {
                format!("f:{relative}")
            } else {
                return Err(invalid_bundle());
            };
            entries.insert(record);
            if entries.len() > MAX_TREE_ENTRIES {
                return Err(limit_exceeded());
            }
        }
    }
    Ok(entries)
}

fn strict_relative_path(path: &str) -> Result<&Path, UpgradeFailure> {
    let path = Path::new(path);
    if path.is_absolute()
        || path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(invalid_bundle());
    }
    Ok(path)
}

fn relative_path(path: &Path) -> Result<String, UpgradeFailure> {
    path.iter()
        .map(OsStr::to_str)
        .collect::<Option<Vec<_>>>()
        .map(|components| components.join("/"))
        .ok_or_else(invalid_bundle)
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

fn invalid_arguments() -> UpgradeFailure {
    UpgradeFailure::new(CodeNoesisErrorV27::invalid_arguments(), 2)
}

fn invalid_bundle() -> UpgradeFailure {
    UpgradeFailure::new(CodeNoesisErrorV27::invalid_bundle(), 2)
}

fn unstable_input() -> UpgradeFailure {
    UpgradeFailure::new(CodeNoesisErrorV27::unstable_input(), 2)
}

fn incompatible() -> UpgradeFailure {
    UpgradeFailure::new(CodeNoesisErrorV27::incompatible(), 2)
}

fn limit_exceeded() -> UpgradeFailure {
    UpgradeFailure::new(CodeNoesisErrorV27::limit_exceeded(), 2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const BINARY_V1: &[u8] = include_bytes!(
        "../../tests/specifications/g1/local-cli-distribution-v1/fixtures/noesis-v1.bin"
    );
    const BINARY_V2: &[u8] = include_bytes!(
        "../../tests/specifications/g1/local-cli-distribution-v1/fixtures/noesis-v2.bin"
    );

    #[test]
    fn pt_fr_cmp_001_ten_parallel_schedules_are_byte_identical() {
        if codenoesis_contracts::current_local_distribution_target() == "unsupported-compile-target"
        {
            return;
        }
        let root = tempfile::tempdir().expect("temporary root");
        let current = package_fixture(root.path(), "current", BINARY_V1);
        let candidate = package_fixture(root.path(), "candidate", BINARY_V2);
        let outputs = std::thread::scope(|scope| {
            (0..10)
                .map(|_| {
                    let current = &current;
                    let candidate = &candidate;
                    scope.spawn(move || {
                        run(upgrade_arguments(current, candidate, false))
                            .expect("parallel upgrade preflight")
                    })
                })
                .collect::<Vec<_>>()
                .into_iter()
                .map(|handle| handle.join().expect("join schedule"))
                .collect::<Vec<_>>()
        });
        for (schedule, output) in outputs[1..].iter().enumerate() {
            assert_eq!(output, &outputs[0], "schedule {}", schedule + 1);
        }
    }

    #[test]
    fn sec_fr_cmp_001_tamper_extra_tree_and_same_bundle_fail_closed() {
        if codenoesis_contracts::current_local_distribution_target() == "unsupported-compile-target"
        {
            return;
        }
        let root = tempfile::tempdir().expect("temporary root");
        let current = package_fixture(root.path(), "current", BINARY_V1);
        let candidate = package_fixture(root.path(), "candidate", BINARY_V2);

        let failure = run(upgrade_arguments(&current, &current, false)).unwrap_err();
        assert_code(&failure, "compatibility.incompatible");

        fs::create_dir(candidate.join("private-credential-canary"))
            .expect("create unauthorized empty directory");
        let failure = run(upgrade_arguments(&current, &candidate, false)).unwrap_err();
        assert_code(&failure, "compatibility.invalid_bundle");
        assert!(
            !failure
                .error()
                .canonical_stderr()
                .unwrap()
                .windows(b"credential".len())
                .any(|window| window == b"credential")
        );

        fs::remove_dir(candidate.join("private-credential-canary"))
            .expect("remove unauthorized directory");
        fs::write(candidate.join("etc/codenoesis/config.json"), b"tampered")
            .expect("tamper fixed payload");
        let failure = run(upgrade_arguments(&current, &candidate, false)).unwrap_err();
        assert_code(&failure, "compatibility.invalid_bundle");
    }

    #[test]
    fn sec_fr_cli_009_plan_limit_substitution_and_arguments_fail_closed() {
        if codenoesis_contracts::current_local_distribution_target() == "unsupported-compile-target"
        {
            return;
        }
        let root = tempfile::tempdir().expect("temporary root");
        let current = package_fixture(root.path(), "current", BINARY_V1);
        let candidate = package_fixture(root.path(), "candidate", BINARY_V2);
        let plan = run(upgrade_arguments(&current, &candidate, false)).expect("upgrade plan");

        let plan_path = root.path().join("plan.json");
        fs::write(&plan_path, &plan).expect("write plan");
        let rollback = run(rollback_arguments(&plan_path, &candidate, &current))
            .expect("exact rollback preflight");
        assert!(!rollback.is_empty());

        let mut substituted = plan.clone();
        substituted[1] = if substituted[1] == b'"' { b'{' } else { b'"' };
        fs::write(&plan_path, substituted).expect("substitute plan");
        let failure = run(rollback_arguments(&plan_path, &candidate, &current)).unwrap_err();
        assert_code(&failure, "compatibility.invalid_plan");

        fs::write(&plan_path, vec![b'x'; MAX_LOCAL_UPGRADE_PLAN_BYTES + 1])
            .expect("write oversized plan");
        let failure = run(rollback_arguments(&plan_path, &candidate, &current)).unwrap_err();
        assert_code(&failure, "compatibility.limit_exceeded");

        let failure = run([
            OsString::from("xtask"),
            OsString::from("preflight-local-upgrade"),
            OsString::from("--current"),
            current.as_os_str().to_owned(),
            OsString::from("--current"),
            candidate.as_os_str().to_owned(),
        ])
        .unwrap_err();
        assert_code(&failure, "compatibility.invalid_arguments");
    }

    #[test]
    fn sec_fr_cmp_001_post_read_revalidation_detects_mutation() {
        let root = tempfile::tempdir().expect("temporary root");
        let path = root.path().join("stable-input");
        fs::write(&path, b"first").expect("write stable input");
        let stable = StableFile::read(&path, 64, true).expect("inspect stable input");
        fs::write(&path, b"second").expect("replace inspected bytes");
        let failure = stable.revalidate().unwrap_err();
        assert_code(&failure, "compatibility.unstable_input");
    }

    #[cfg(unix)]
    #[test]
    fn sec_fr_cmp_001_symlink_bundle_root_is_rejected() {
        use std::os::unix::fs::symlink;

        if codenoesis_contracts::current_local_distribution_target() == "unsupported-compile-target"
        {
            return;
        }
        let root = tempfile::tempdir().expect("temporary root");
        let current = package_fixture(root.path(), "current", BINARY_V1);
        let candidate = package_fixture(root.path(), "candidate", BINARY_V2);
        let link = root.path().join("linked-current");
        symlink(&current, &link).expect("create bundle symlink");
        let failure = run(upgrade_arguments(&link, &candidate, false)).unwrap_err();
        assert_code(&failure, "compatibility.invalid_bundle");
    }

    fn package_fixture(root: &Path, label: &str, bytes: &[u8]) -> PathBuf {
        let binary = root.join(format!("{label}.bin"));
        fs::write(&binary, bytes).expect("write fixture binary");
        let output = root.join(format!("{label}-output"));
        fs::create_dir(&output).expect("create package output root");
        crate::distribution::run([
            OsString::from("xtask"),
            OsString::from("package-local-cli"),
            OsString::from("--binary"),
            binary.into_os_string(),
            OsString::from("--output"),
            output.as_os_str().to_owned(),
        ])
        .expect("package fixture");
        let entries = fs::read_dir(&output)
            .expect("read output")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect output entries");
        assert_eq!(entries.len(), 1);
        entries[0].path()
    }

    fn upgrade_arguments(current: &Path, candidate: &Path, reverse: bool) -> Vec<OsString> {
        let pairs = if reverse {
            [
                ("--candidate", candidate.as_os_str()),
                ("--current", current.as_os_str()),
            ]
        } else {
            [
                ("--current", current.as_os_str()),
                ("--candidate", candidate.as_os_str()),
            ]
        };
        let mut arguments = vec![
            OsString::from("xtask"),
            OsString::from("preflight-local-upgrade"),
        ];
        for (flag, value) in pairs {
            arguments.push(OsString::from(flag));
            arguments.push(value.to_owned());
        }
        arguments
    }

    fn rollback_arguments(plan: &Path, current: &Path, target: &Path) -> Vec<OsString> {
        vec![
            OsString::from("xtask"),
            OsString::from("preflight-local-rollback"),
            OsString::from("--plan"),
            plan.as_os_str().to_owned(),
            OsString::from("--current"),
            current.as_os_str().to_owned(),
            OsString::from("--target"),
            target.as_os_str().to_owned(),
        ]
    }

    fn assert_code(failure: &UpgradeFailure, expected: &str) {
        assert_eq!(failure.exit_code(), 2);
        assert_eq!(
            failure.error().value().get("code").and_then(Value::as_str),
            Some(expected)
        );
    }
}
