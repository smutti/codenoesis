use std::collections::BTreeSet;
use std::ffi::OsString;

use codenoesis_contracts::{
    GitImpactSourceFile, ImpactSourceError, ImpactSourceSelectionV1, MAX_R19_SOURCE_BYTES_PER_FILE,
    MAX_R19_TOTAL_SOURCE_BYTES, R19Sha256, TrustedImpactSourceExcerptV1,
};
use codenoesis_domain::{RepositoryError, RepositoryIdentity, RepositoryInventory, Revision};
use codenoesis_ports::SafeRepositoryAcquirer;

pub struct GitImpactRepositoryRequest {
    repository: OsString,
    identity: RepositoryIdentity,
    revision: Revision,
    paths: Vec<String>,
}

impl GitImpactRepositoryRequest {
    #[must_use]
    pub const fn new(
        repository: OsString,
        identity: RepositoryIdentity,
        revision: Revision,
        paths: Vec<String>,
    ) -> Self {
        Self {
            repository,
            identity,
            revision,
            paths,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GitImpactAcquisitionError {
    Repository(RepositoryError),
    InvalidSelection,
    LimitExceeded,
}

pub struct GitImpactAcquisitionService<A> {
    acquirer: A,
}

impl<A> GitImpactAcquisitionService<A>
where
    A: SafeRepositoryAcquirer,
{
    #[must_use]
    pub const fn new(acquirer: A) -> Self {
        Self { acquirer }
    }

    /// Reacquires explicit repositories and returns only exact selected Git files.
    ///
    /// # Errors
    ///
    /// Returns a typed repository, missing/duplicate path, or selected-byte
    /// limit failure without exposing local roots or partial output.
    pub fn acquire(
        &self,
        requests: &[GitImpactRepositoryRequest],
    ) -> Result<Vec<GitImpactSourceFile>, GitImpactAcquisitionError> {
        let mut selected = Vec::new();
        let mut total_bytes = 0_u64;
        for request in requests {
            let acquired = self
                .acquirer
                .acquire_inventory(
                    &request.repository,
                    request.identity.clone(),
                    request.revision.clone(),
                )
                .map_err(GitImpactAcquisitionError::Repository)?;
            let inventory = RepositoryInventory::classify(acquired);
            let mut paths = BTreeSet::new();
            for path in &request.paths {
                if !paths.insert(path.as_str()) {
                    return Err(GitImpactAcquisitionError::InvalidSelection);
                }
                let matches = inventory
                    .files()
                    .iter()
                    .filter(|file| file.path() == path)
                    .collect::<Vec<_>>();
                let [file] = matches.as_slice() else {
                    return Err(GitImpactAcquisitionError::InvalidSelection);
                };
                if file.byte_length() > MAX_R19_SOURCE_BYTES_PER_FILE {
                    return Err(GitImpactAcquisitionError::LimitExceeded);
                }
                total_bytes = total_bytes.saturating_add(file.byte_length());
                if total_bytes > MAX_R19_TOTAL_SOURCE_BYTES {
                    return Err(GitImpactAcquisitionError::LimitExceeded);
                }
                let bound = inventory.bound_revision();
                selected.push(GitImpactSourceFile {
                    repository_identity: bound.repository_identity().as_str().to_owned(),
                    commit_oid: bound.commit_oid().as_str().to_owned(),
                    tree_oid: bound.tree_oid().as_str().to_owned(),
                    path: file.path().to_owned(),
                    blob_oid: file.blob_oid().as_str().to_owned(),
                    bytes: file.bytes().to_vec(),
                });
            }
        }
        selected.sort_by(|left, right| {
            (
                left.repository_identity.as_bytes(),
                left.commit_oid.as_bytes(),
                left.path.as_bytes(),
            )
                .cmp(&(
                    right.repository_identity.as_bytes(),
                    right.commit_oid.as_bytes(),
                    right.path.as_bytes(),
                ))
        });
        Ok(selected)
    }
}

pub struct TrustedImpactSourceRequest {
    repository: OsString,
    selection: ImpactSourceSelectionV1,
}

impl TrustedImpactSourceRequest {
    #[must_use]
    pub const fn new(repository: OsString, selection: ImpactSourceSelectionV1) -> Self {
        Self {
            repository,
            selection,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrustedImpactSourceRetrievalError {
    Repository(RepositoryError),
    Contract(ImpactSourceError),
}

pub struct TrustedImpactSourceRetrievalService<A> {
    acquirer: A,
}

impl<A> TrustedImpactSourceRetrievalService<A>
where
    A: SafeRepositoryAcquirer,
{
    #[must_use]
    pub const fn new(acquirer: A) -> Self {
        Self { acquirer }
    }

    /// Independently reacquires one report-selected immutable Git source span.
    ///
    /// # Errors
    ///
    /// Returns a typed repository or source-contract failure without exposing
    /// the local root or source bytes in errors.
    pub fn retrieve(
        &self,
        request: &TrustedImpactSourceRequest,
        sha256: R19Sha256,
    ) -> Result<TrustedImpactSourceExcerptV1, TrustedImpactSourceRetrievalError> {
        let acquired = self
            .acquirer
            .acquire_inventory(
                &request.repository,
                request.selection.repository_identity().clone(),
                Revision::Commit(request.selection.commit_oid().clone()),
            )
            .map_err(TrustedImpactSourceRetrievalError::Repository)?;
        let inventory = RepositoryInventory::classify(acquired);
        TrustedImpactSourceExcerptV1::from_inventory(&request.selection, &inventory, sha256)
            .map_err(TrustedImpactSourceRetrievalError::Contract)
    }
}
