use std::collections::BTreeMap;
use std::fs::{self, File, Metadata};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[cfg(unix)]
use std::os::unix::fs::MetadataExt as _;
#[cfg(windows)]
use std::os::windows::fs::MetadataExt as _;

use codenoesis_domain::s1_packed::{PackedComponent, STANDARD_LOCAL_PACKED_LIMITS};
use codenoesis_domain::{
    AcquisitionError, LimitKind, ObjectId, UnsupportedFeature, limit_exceeded,
};

use super::{changed, invalid_catalog, unavailable};

pub(super) struct PackPair {
    pub(super) pack_id: ObjectId,
    pub(super) pack: TrackedFile,
    pub(super) index: TrackedFile,
}

pub(super) struct TrackedFile {
    path: PathBuf,
    file: File,
    before: MetadataSnapshot,
}

impl TrackedFile {
    pub(super) fn len(&self) -> u64 {
        self.before.len
    }

    pub(super) fn read_exact_at(
        &mut self,
        offset: u64,
        bytes: &mut [u8],
        component: PackedComponent,
    ) -> Result<(), AcquisitionError> {
        let result = self
            .file
            .seek(SeekFrom::Start(offset))
            .and_then(|_| self.file.read_exact(bytes));
        if result.is_ok() {
            return Ok(());
        }
        if self.is_unchanged() {
            Err(unavailable(component))
        } else {
            Err(changed(component))
        }
    }

    pub(super) fn read_all(
        &mut self,
        component: PackedComponent,
    ) -> Result<Vec<u8>, AcquisitionError> {
        let length = usize::try_from(self.before.len).map_err(|_| unavailable(component))?;
        let mut bytes = vec![0_u8; length];
        self.read_exact_at(0, &mut bytes, component)?;
        Ok(bytes)
    }

    pub(super) fn verify_unchanged(
        &self,
        component: PackedComponent,
    ) -> Result<(), AcquisitionError> {
        if self.is_unchanged() {
            Ok(())
        } else {
            Err(changed(component))
        }
    }

    fn is_unchanged(&self) -> bool {
        let Ok(handle) = self.file.metadata() else {
            return false;
        };
        let Ok(path) = fs::symlink_metadata(&self.path) else {
            return false;
        };
        !path.file_type().is_symlink()
            && path.is_file()
            && self.before.matches(&handle)
            && self.before.matches(&path)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MetadataSnapshot {
    len: u64,
    modified: Option<SystemTime>,
    created: Option<SystemTime>,
    readonly: bool,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(windows)]
    volume_serial_number: Option<u32>,
    #[cfg(windows)]
    file_index: Option<u64>,
}

impl MetadataSnapshot {
    fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            len: metadata.len(),
            modified: metadata.modified().ok(),
            created: metadata.created().ok(),
            readonly: metadata.permissions().readonly(),
            #[cfg(unix)]
            device: metadata.dev(),
            #[cfg(unix)]
            inode: metadata.ino(),
            #[cfg(windows)]
            volume_serial_number: metadata.volume_serial_number(),
            #[cfg(windows)]
            file_index: metadata.file_index(),
        }
    }

    fn matches(&self, metadata: &Metadata) -> bool {
        metadata.is_file()
            && self.len == metadata.len()
            && self.modified == metadata.modified().ok()
            && self.created == metadata.created().ok()
            && self.readonly == metadata.permissions().readonly()
            && {
                #[cfg(unix)]
                {
                    self.device == metadata.dev() && self.inode == metadata.ino()
                }
                #[cfg(windows)]
                {
                    self.volume_serial_number == metadata.volume_serial_number()
                        && self.file_index == metadata.file_index()
                }
                #[cfg(not(any(unix, windows)))]
                {
                    true
                }
            }
    }
}

#[derive(Default)]
struct PairPaths {
    pack: Option<PathBuf>,
    index: Option<PathBuf>,
}

pub(super) fn open_catalog(git_dir: &Path) -> Result<Vec<PackPair>, AcquisitionError> {
    let pack_directory = git_dir.join("objects").join("pack");
    let metadata = match fs::symlink_metadata(&pack_directory) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => return Err(unavailable(PackedComponent::Catalog)),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(invalid_catalog());
    }

    let entries =
        fs::read_dir(&pack_directory).map_err(|_| unavailable(PackedComponent::Catalog))?;
    let mut observed = 0_u64;
    let mut pairs = BTreeMap::<String, PairPaths>::new();
    let mut multi_pack_index = false;
    for entry in entries {
        observed = observed
            .checked_add(1)
            .ok_or_else(|| limit_exceeded(LimitKind::PackDirectoryEntries, u64::MAX))?;
        if observed > STANDARD_LOCAL_PACKED_LIMITS.pack_directory_entries {
            return Err(limit_exceeded(LimitKind::PackDirectoryEntries, observed));
        }
        let entry = entry.map_err(|_| changed(PackedComponent::Catalog))?;
        let file_type = entry
            .file_type()
            .map_err(|_| changed(PackedComponent::Catalog))?;
        if file_type.is_symlink() || !file_type.is_file() {
            return Err(invalid_catalog());
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| invalid_catalog())?;
        if name == "multi-pack-index" {
            multi_pack_index = true;
            continue;
        }
        let Some((pack_id, extension)) = parse_pack_name(&name) else {
            return Err(invalid_catalog());
        };
        match extension {
            "pack" => {
                let pair = pairs.entry(pack_id.to_owned()).or_default();
                if pair.pack.replace(entry.path()).is_some() {
                    return Err(changed(PackedComponent::Catalog));
                }
            }
            "idx" => {
                let pair = pairs.entry(pack_id.to_owned()).or_default();
                if pair.index.replace(entry.path()).is_some() {
                    return Err(changed(PackedComponent::Catalog));
                }
            }
            "promisor" => {
                return Err(AcquisitionError::UnsupportedRepositoryShape {
                    feature: UnsupportedFeature::PromisorObjectDatabase,
                });
            }
            "bitmap" | "keep" | "mtimes" | "rev" => {}
            _ => return Err(invalid_catalog()),
        }
    }

    if multi_pack_index && pairs.is_empty() {
        return Err(AcquisitionError::UnsupportedRepositoryShape {
            feature: UnsupportedFeature::MultiPackIndexOnly,
        });
    }
    let pair_count = u64::try_from(pairs.len()).unwrap_or(u64::MAX);
    if pair_count > STANDARD_LOCAL_PACKED_LIMITS.pack_pairs {
        return Err(limit_exceeded(LimitKind::PackPairs, pair_count));
    }

    let mut opened = Vec::with_capacity(pairs.len());
    for (pack_id, paths) in pairs {
        let (Some(pack), Some(index)) = (paths.pack, paths.index) else {
            return Err(changed(PackedComponent::Catalog));
        };
        let pack_id = ObjectId::parse_sha1(&pack_id).ok_or_else(invalid_catalog)?;
        opened.push(PackPair {
            pack_id,
            pack: open_tracked(pack)?,
            index: open_tracked(index)?,
        });
    }
    Ok(opened)
}

fn open_tracked(path: PathBuf) -> Result<TrackedFile, AcquisitionError> {
    let before = fs::symlink_metadata(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            changed(PackedComponent::Catalog)
        } else {
            unavailable(PackedComponent::Catalog)
        }
    })?;
    if before.file_type().is_symlink() || !before.is_file() {
        return Err(invalid_catalog());
    }
    let snapshot = MetadataSnapshot::from_metadata(&before);
    let file = File::open(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            changed(PackedComponent::Catalog)
        } else {
            unavailable(PackedComponent::Catalog)
        }
    })?;
    let handle = file
        .metadata()
        .map_err(|_| unavailable(PackedComponent::Catalog))?;
    if !snapshot.matches(&handle) {
        return Err(changed(PackedComponent::Catalog));
    }
    Ok(TrackedFile {
        path,
        file,
        before: snapshot,
    })
}

fn parse_pack_name(name: &str) -> Option<(&str, &str)> {
    let suffix = name.strip_prefix("pack-")?;
    let (pack_id, extension) = suffix.rsplit_once('.')?;
    (pack_id.len() == 40
        && pack_id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        && matches!(
            extension,
            "pack" | "idx" | "promisor" | "bitmap" | "keep" | "mtimes" | "rev"
        ))
    .then_some((pack_id, extension))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt as _;

    use super::open_tracked;
    use crate::packed::changed;
    use codenoesis_domain::s1_packed::PackedComponent;

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn race_fr_acq_004_tracked_file_schedules_are_changed() {
        let root = unique_root();

        let rename_path = write_subject(&root, "rename");
        let rename = open_tracked(rename_path.clone()).expect("open rename subject");
        fs::rename(&rename_path, root.join("renamed-away")).expect("rename tracked file");
        assert_eq!(
            rename.verify_unchanged(PackedComponent::Pack),
            Err(changed(PackedComponent::Pack))
        );

        let truncate_path = write_subject(&root, "truncate");
        let truncate = open_tracked(truncate_path.clone()).expect("open truncate subject");
        fs::OpenOptions::new()
            .write(true)
            .open(&truncate_path)
            .expect("open tracked file for truncation")
            .set_len(0)
            .expect("truncate tracked file");
        assert_eq!(
            truncate.verify_unchanged(PackedComponent::Pack),
            Err(changed(PackedComponent::Pack))
        );

        let rewrite_path = write_subject(&root, "rewrite");
        let rewrite = open_tracked(rewrite_path.clone()).expect("open rewrite subject");
        fs::write(&rewrite_path, b"modified").expect("rewrite tracked file");
        let mut permissions = fs::metadata(&rewrite_path)
            .expect("rewritten file metadata")
            .permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&rewrite_path, permissions).expect("mark rewritten file read-only");
        assert_eq!(
            rewrite.verify_unchanged(PackedComponent::Pack),
            Err(changed(PackedComponent::Pack))
        );

        let repack_path = write_subject(&root, "repack");
        let repack = open_tracked(repack_path.clone()).expect("open repack subject");
        let replacement = root.join("replacement");
        fs::write(&replacement, b"original").expect("write same-length replacement");
        fs::rename(&repack_path, root.join("repack-away")).expect("archive tracked pack");
        fs::rename(replacement, &repack_path).expect("install replacement pack");
        assert_eq!(
            repack.verify_unchanged(PackedComponent::Pack),
            Err(changed(PackedComponent::Pack))
        );

        restore_writable(&rewrite_path);
        fs::remove_dir_all(root).expect("remove race test root");
    }

    fn write_subject(root: &Path, name: &str) -> PathBuf {
        let path = root.join(name);
        fs::write(&path, b"original").expect("write tracked race subject");
        path
    }

    fn unique_root() -> PathBuf {
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "codenoesis-packed-race-{}-{timestamp}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("create race test root");
        root
    }

    #[cfg(unix)]
    fn restore_writable(path: &Path) {
        let mut permissions = fs::metadata(path)
            .expect("read-only file metadata")
            .permissions();
        permissions.set_mode(0o600);
        fs::set_permissions(path, permissions).expect("restore writable test file");
    }

    #[cfg(not(unix))]
    #[allow(clippy::permissions_set_readonly_false)]
    fn restore_writable(path: &Path) {
        let mut permissions = fs::metadata(path)
            .expect("read-only file metadata")
            .permissions();
        permissions.set_readonly(false);
        fs::set_permissions(path, permissions).expect("restore writable test file");
    }
}
