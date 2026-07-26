use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use codenoesis_domain::storage::{
    ArtifactId, PublicationBoundary, PublicationEvent, StorageError, StoredArtifact, SweepResult,
};
use codenoesis_ports::{ArtifactStore, PublicationObserver};
use tempfile::NamedTempFile;

use crate::path::{is_unsafe_metadata, sync_directory};

pub struct FilesystemCas {
    root: PathBuf,
    objects_blake3: PathBuf,
    temporary: PathBuf,
}

impl FilesystemCas {
    #[must_use]
    pub const fn new(root: PathBuf, objects_blake3: PathBuf, temporary: PathBuf) -> Self {
        Self {
            root,
            objects_blake3,
            temporary,
        }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn object_path(&self, artifact_id: &ArtifactId) -> PathBuf {
        let digest = artifact_id.digest();
        self.objects_blake3.join(&digest[..2]).join(&digest[2..])
    }

    fn ensure_shard(&self, artifact_id: &ArtifactId) -> Result<PathBuf, StorageError> {
        let shard = self.objects_blake3.join(&artifact_id.digest()[..2]);
        match fs::create_dir(&shard) {
            Ok(()) => {
                sync_directory(&self.objects_blake3)?;
                sync_directory(&shard)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let metadata =
                    fs::symlink_metadata(&shard).map_err(|_| StorageError::PublicationFailed)?;
                if !metadata.is_dir() || is_unsafe_metadata(&metadata) {
                    return Err(StorageError::UnsafePath {
                        reason: "unsafe_path_component",
                    });
                }
            }
            Err(_) => return Err(StorageError::PublicationFailed),
        }
        Ok(shard)
    }

    fn verify_path(
        path: &Path,
        artifact_id: &ArtifactId,
        expected_length: Option<u64>,
    ) -> Result<Vec<u8>, StorageError> {
        let parent = path.parent().ok_or(StorageError::PublicationFailed)?;
        let parent_metadata =
            fs::symlink_metadata(parent).map_err(|_| StorageError::PublicationFailed)?;
        let metadata = fs::symlink_metadata(path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                StorageError::MissingObject {
                    artifact_id: artifact_id.to_string(),
                }
            } else {
                StorageError::PublicationFailed
            }
        })?;
        if is_unsafe_metadata(&parent_metadata)
            || !parent_metadata.is_dir()
            || is_unsafe_metadata(&metadata)
            || !metadata.is_file()
        {
            return Err(StorageError::UnsafePath {
                reason: "unsafe_path_component",
            });
        }
        let bytes = fs::read(path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                StorageError::MissingObject {
                    artifact_id: artifact_id.to_string(),
                }
            } else {
                StorageError::PublicationFailed
            }
        })?;
        let observed = ArtifactId::from_bytes(&bytes);
        if expected_length.is_some_and(|length| u64::try_from(bytes.len()).ok() != Some(length))
            || observed != *artifact_id
        {
            return Err(StorageError::CorruptObject {
                artifact_id: artifact_id.to_string(),
                expected_hash: artifact_id.digest().to_owned(),
                observed_hash: observed.digest().to_owned(),
            });
        }
        Ok(bytes)
    }

    fn remove_temporary_files(&self) -> Result<u64, StorageError> {
        let mut removed = 0_u64;
        for entry in fs::read_dir(&self.temporary).map_err(|_| StorageError::PublicationFailed)? {
            let entry = entry.map_err(|_| StorageError::PublicationFailed)?;
            let metadata =
                fs::symlink_metadata(entry.path()).map_err(|_| StorageError::PublicationFailed)?;
            if is_unsafe_metadata(&metadata) || !metadata.is_file() {
                return Err(StorageError::UnsafePath {
                    reason: "unsafe_path_component",
                });
            }
            fs::remove_file(entry.path()).map_err(|_| StorageError::PublicationFailed)?;
            removed = removed.saturating_add(1);
        }
        sync_directory(&self.temporary)?;
        Ok(removed)
    }

    fn object_entries(&self) -> Result<Vec<(ArtifactId, PathBuf)>, StorageError> {
        let mut objects = Vec::new();
        for shard in
            fs::read_dir(&self.objects_blake3).map_err(|_| StorageError::PublicationFailed)?
        {
            let shard = shard.map_err(|_| StorageError::PublicationFailed)?;
            let shard_metadata =
                fs::symlink_metadata(shard.path()).map_err(|_| StorageError::PublicationFailed)?;
            if is_unsafe_metadata(&shard_metadata) || !shard_metadata.is_dir() {
                return Err(StorageError::UnsafePath {
                    reason: "unsafe_path_component",
                });
            }
            let prefix = shard.file_name().to_string_lossy().into_owned();
            for object in fs::read_dir(shard.path()).map_err(|_| StorageError::PublicationFailed)? {
                let object = object.map_err(|_| StorageError::PublicationFailed)?;
                let metadata = fs::symlink_metadata(object.path())
                    .map_err(|_| StorageError::PublicationFailed)?;
                if is_unsafe_metadata(&metadata) || !metadata.is_file() {
                    return Err(StorageError::UnsafePath {
                        reason: "unsafe_path_component",
                    });
                }
                let digest = format!("{}{}", prefix, object.file_name().to_string_lossy());
                let value = format!("urn:codenoesis:artifact:blake3:{digest}");
                let artifact_id =
                    ArtifactId::parse(&value).ok_or(StorageError::CorruptMetadata {
                        component: codenoesis_domain::storage::StorageComponent::Cas,
                        reason: "invalid_object_path",
                        snapshot_id: None,
                    })?;
                objects.push((artifact_id, object.path()));
            }
        }
        objects.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(objects)
    }
}

impl ArtifactStore for FilesystemCas {
    fn stage(
        &mut self,
        artifact: &StoredArtifact,
        observer: &mut dyn PublicationObserver,
    ) -> Result<(), StorageError> {
        let final_path = self.object_path(&artifact.artifact_id);
        match fs::symlink_metadata(&final_path) {
            Ok(_) => {
                Self::verify_path(
                    &final_path,
                    &artifact.artifact_id,
                    Some(artifact.byte_length()),
                )?;
                return Ok(());
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(StorageError::PublicationFailed),
        }
        let shard = self.ensure_shard(&artifact.artifact_id)?;
        observer.observe(&PublicationEvent::cas(
            PublicationBoundary::CasBeforeTempCreate,
            artifact.role,
            artifact.ordinal,
        ))?;
        let mut temporary =
            NamedTempFile::new_in(&self.temporary).map_err(|_| StorageError::PublicationFailed)?;
        temporary
            .write_all(&artifact.bytes)
            .and_then(|()| temporary.as_file().sync_all())
            .map_err(|_| StorageError::PublicationFailed)?;
        observer.observe(&PublicationEvent::cas(
            PublicationBoundary::CasAfterTempSync,
            artifact.role,
            artifact.ordinal,
        ))?;
        Self::verify_path(
            temporary.path(),
            &artifact.artifact_id,
            Some(artifact.byte_length()),
        )?;
        match temporary.persist_noclobber(&final_path) {
            Ok(_) => {}
            Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                Self::verify_path(
                    &final_path,
                    &artifact.artifact_id,
                    Some(artifact.byte_length()),
                )?;
                return Ok(());
            }
            Err(_) => return Err(StorageError::PublicationFailed),
        }
        observer.observe(&PublicationEvent::cas(
            PublicationBoundary::CasAfterObjectMove,
            artifact.role,
            artifact.ordinal,
        ))?;
        sync_directory(&shard)?;
        observer.observe(&PublicationEvent::cas(
            PublicationBoundary::CasAfterParentSync,
            artifact.role,
            artifact.ordinal,
        ))
    }

    fn read(&self, artifact_id: &ArtifactId, byte_length: u64) -> Result<Vec<u8>, StorageError> {
        Self::verify_path(
            &self.object_path(artifact_id),
            artifact_id,
            Some(byte_length),
        )
    }

    fn sweep(
        &mut self,
        reachable: &BTreeSet<ArtifactId>,
        recheck: &mut dyn FnMut(&ArtifactId) -> Result<bool, StorageError>,
    ) -> Result<SweepResult, StorageError> {
        let temporary_files_removed = self.remove_temporary_files()?;
        let mut objects_removed = 0_u64;
        for (artifact_id, path) in self.object_entries()? {
            if reachable.contains(&artifact_id) {
                Self::verify_path(&path, &artifact_id, None)?;
                continue;
            }
            if recheck(&artifact_id)? {
                Self::verify_path(&path, &artifact_id, None)?;
                continue;
            }
            fs::remove_file(&path).map_err(|_| StorageError::PublicationFailed)?;
            let parent = path.parent().ok_or(StorageError::PublicationFailed)?;
            sync_directory(parent)?;
            objects_removed = objects_removed.saturating_add(1);
        }
        Ok(SweepResult {
            temporary_files_removed,
            objects_removed,
        })
    }
}
