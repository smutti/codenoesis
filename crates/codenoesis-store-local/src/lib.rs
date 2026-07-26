//! Local `SQLite` and filesystem-CAS adapters for the S3 storage ports.

mod cas;
mod path;
mod sqlite;

use std::path::Path;

pub use cas::FilesystemCas;
pub use sqlite::{SqliteEvidence, SqliteMetadataStore};

use codenoesis_domain::storage::StorageError;

pub struct LocalStore {
    pub artifacts: FilesystemCas,
    pub metadata: SqliteMetadataStore,
}

impl LocalStore {
    /// Opens an exact existing v1 store or initializes one empty safe root.
    ///
    /// # Errors
    ///
    /// Returns a typed path, marker, schema, or initialization failure.
    pub fn open(repository_root: &Path, store_root: &Path) -> Result<Self, StorageError> {
        let prepared = path::prepare(repository_root, store_root)?;
        let metadata = SqliteMetadataStore::open(&prepared.database, prepared.fresh)?;
        if prepared.fresh {
            path::write_marker(&prepared)?;
        }
        Ok(Self {
            artifacts: FilesystemCas::new(
                prepared.root,
                prepared.objects_blake3,
                prepared.temporary,
            ),
            metadata,
        })
    }
}

/// Verifies the explicit roots and creates only an absent empty store leaf so
/// an operating-system capability boundary can reference it.
///
/// # Errors
///
/// Returns a typed path or root-creation failure.
pub fn ensure_store_root_for_boundary(
    repository_root: &Path,
    store_root: &Path,
) -> Result<(), StorageError> {
    path::ensure_root(repository_root, store_root).map(drop)
}
