use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use codenoesis_domain::storage::{StorageComponent, StorageError};
use codenoesis_ports::AnalysisCacheStore;
use tempfile::NamedTempFile;

use crate::path::{
    create_directory_noclobber, is_unsafe_metadata, persist_noclobber, sync_directory,
};

const ENTRY_ID_PREFIX: &str = "urn:codenoesis:analysis-cache-entry:blake3:";

pub struct FilesystemAnalysisCache {
    root: PathBuf,
    temporary: PathBuf,
}

impl FilesystemAnalysisCache {
    #[must_use]
    pub const fn new(root: PathBuf, temporary: PathBuf) -> Self {
        Self { root, temporary }
    }

    fn ensure_directory(path: &Path) -> Result<(), StorageError> {
        match create_directory_noclobber(path) {
            Ok(()) => {
                sync_directory(
                    path.parent()
                        .ok_or_else(|| cache_corrupt("cache_parent_missing"))?,
                )?;
                sync_directory(path)
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let metadata =
                    fs::symlink_metadata(path).map_err(|_| cache_corrupt("cache_path_missing"))?;
                if metadata.is_dir() && !is_unsafe_metadata(&metadata) {
                    Ok(())
                } else {
                    Err(cache_corrupt("cache_path_unsafe"))
                }
            }
            Err(_) => Err(StorageError::PublicationFailed),
        }
    }

    fn digest(entry_id: &str) -> Result<&str, StorageError> {
        let digest = entry_id
            .strip_prefix(ENTRY_ID_PREFIX)
            .ok_or_else(|| cache_corrupt("cache_entry_id_invalid"))?;
        if digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            Ok(digest)
        } else {
            Err(cache_corrupt("cache_entry_id_invalid"))
        }
    }

    fn entry_path(&self, entry_id: &str) -> Result<PathBuf, StorageError> {
        let digest = Self::digest(entry_id)?;
        Ok(self.root.join(&digest[..2]).join(&digest[2..]))
    }
}

impl AnalysisCacheStore for FilesystemAnalysisCache {
    fn stage_entry(
        &mut self,
        analysis_cache_entry_id: &str,
        bytes: &[u8],
    ) -> Result<(), StorageError> {
        if bytes.is_empty() {
            return Err(cache_corrupt("cache_entry_empty"));
        }
        let final_path = self.entry_path(analysis_cache_entry_id)?;
        Self::ensure_directory(&self.root)?;
        let shard = final_path
            .parent()
            .ok_or_else(|| cache_corrupt("cache_shard_missing"))?;
        Self::ensure_directory(shard)?;
        match fs::symlink_metadata(&final_path) {
            Ok(metadata) => {
                if !metadata.is_file()
                    || is_unsafe_metadata(&metadata)
                    || fs::read(&final_path).map_err(|_| cache_corrupt("cache_entry_unreadable"))?
                        != bytes
                {
                    return Err(cache_corrupt("cache_entry_conflict"));
                }
                return Ok(());
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(cache_corrupt("cache_entry_unreadable")),
        }
        let mut temporary =
            NamedTempFile::new_in(&self.temporary).map_err(|_| StorageError::PublicationFailed)?;
        temporary
            .write_all(bytes)
            .and_then(|()| temporary.as_file().sync_all())
            .map_err(|_| StorageError::PublicationFailed)?;
        match persist_noclobber(temporary, &final_path) {
            Ok(()) => {
                sync_directory(shard)?;
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let observed =
                    fs::read(&final_path).map_err(|_| cache_corrupt("cache_entry_unreadable"))?;
                if observed == bytes {
                    Ok(())
                } else {
                    Err(cache_corrupt("cache_entry_conflict"))
                }
            }
            Err(_) => Err(StorageError::PublicationFailed),
        }
    }

    fn load_entries(&self) -> Result<Vec<(String, Vec<u8>)>, StorageError> {
        match fs::symlink_metadata(&self.root) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(_) => return Err(cache_corrupt("cache_path_unreadable")),
            Ok(metadata) if metadata.is_dir() && !is_unsafe_metadata(&metadata) => {}
            Ok(_) => return Err(cache_corrupt("cache_path_unsafe")),
        }
        let mut entries = Vec::new();
        for shard in fs::read_dir(&self.root).map_err(|_| cache_corrupt("cache_path_unreadable"))? {
            let shard = shard.map_err(|_| cache_corrupt("cache_path_unreadable"))?;
            let shard_name = shard.file_name().to_string_lossy().into_owned();
            let shard_metadata = fs::symlink_metadata(shard.path())
                .map_err(|_| cache_corrupt("cache_shard_unreadable"))?;
            if shard_name.len() != 2
                || !shard_name
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                || !shard_metadata.is_dir()
                || is_unsafe_metadata(&shard_metadata)
            {
                return Err(cache_corrupt("cache_shard_invalid"));
            }
            for entry in
                fs::read_dir(shard.path()).map_err(|_| cache_corrupt("cache_shard_unreadable"))?
            {
                let entry = entry.map_err(|_| cache_corrupt("cache_entry_unreadable"))?;
                let suffix = entry.file_name().to_string_lossy().into_owned();
                let metadata = fs::symlink_metadata(entry.path())
                    .map_err(|_| cache_corrupt("cache_entry_unreadable"))?;
                if suffix.len() != 62
                    || !suffix
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                    || !metadata.is_file()
                    || is_unsafe_metadata(&metadata)
                {
                    return Err(cache_corrupt("cache_entry_invalid"));
                }
                let entry_id = format!("{ENTRY_ID_PREFIX}{shard_name}{suffix}");
                entries.push((
                    entry_id,
                    fs::read(entry.path()).map_err(|_| cache_corrupt("cache_entry_unreadable"))?,
                ));
            }
        }
        entries.sort_by(|left, right| left.0.as_bytes().cmp(right.0.as_bytes()));
        if entries.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(cache_corrupt("cache_entry_duplicate"));
        }
        Ok(entries)
    }
}

fn cache_corrupt(reason: &'static str) -> StorageError {
    StorageError::CorruptMetadata {
        component: StorageComponent::Cas,
        reason,
        snapshot_id: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ft_fr_inc_003_cache_stage_and_reload() {
        let root = tempfile::tempdir().expect("cache test root");
        let temporary = root.path().join("tmp");
        fs::create_dir(&temporary).expect("cache temporary directory");
        fs::create_dir(root.path().join("objects")).expect("cache objects directory");
        let mut cache = FilesystemAnalysisCache::new(root.path().join("objects/cache"), temporary);
        let entry_id = format!("{ENTRY_ID_PREFIX}{}", "a".repeat(64));
        cache
            .stage_entry(&entry_id, b"{\"schema_version\":\"test\"}")
            .expect("stage cache entry");
        assert_eq!(
            cache.load_entries().expect("load cache entries"),
            vec![(entry_id, b"{\"schema_version\":\"test\"}".to_vec())]
        );
    }

    #[cfg(unix)]
    #[test]
    fn sec_fr_inc_001_cache_symlink_is_rejected() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().expect("cache confinement root");
        let temporary = root.path().join("tmp");
        fs::create_dir(&temporary).expect("cache temporary directory");
        fs::create_dir(root.path().join("objects")).expect("cache objects directory");
        let mut cache = FilesystemAnalysisCache::new(root.path().join("objects/cache"), temporary);
        let entry_id = format!("{ENTRY_ID_PREFIX}{}", "a".repeat(64));
        cache
            .stage_entry(&entry_id, b"{\"schema_version\":\"test\"}")
            .expect("stage confined cache entry");
        let entry_path = cache
            .entry_path(&entry_id)
            .expect("resolve confined cache entry");
        fs::remove_file(&entry_path).expect("remove confined cache entry");
        let outside = root.path().join("outside");
        fs::write(&outside, b"outside").expect("write outside cache sentinel");
        symlink(&outside, &entry_path).expect("replace cache entry with symlink");

        assert!(matches!(
            cache.load_entries(),
            Err(StorageError::CorruptMetadata {
                component: StorageComponent::Cas,
                reason: "cache_entry_invalid",
                snapshot_id: None
            })
        ));
    }
}
