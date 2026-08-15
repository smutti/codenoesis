use std::ffi::OsString;

use codenoesis_contracts::{
    R18Sha256, TrustedSourceError, TrustedSourceExcerptV1, TrustedSourceSelectionV1,
    validate_repository_boundary_report_size,
};
use codenoesis_domain::s1_boundaries::{
    BoundarySha256, RepositoryBoundaryAcquisitionError, RepositoryBoundaryError,
    build_boundary_report, parse_gitmodules,
};
use codenoesis_domain::{RepositoryError, RepositoryInventory, Revision};
use codenoesis_ports::{RepositoryBoundaryAcquirer, SafeRepositoryAcquirer};

pub struct TrustedSourceRequest {
    repository: OsString,
    selection: TrustedSourceSelectionV1,
    boundary_profile: bool,
}

impl TrustedSourceRequest {
    #[must_use]
    pub const fn new(
        repository: OsString,
        selection: TrustedSourceSelectionV1,
        boundary_profile: bool,
    ) -> Self {
        Self {
            repository,
            selection,
            boundary_profile,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrustedSourceRetrievalError {
    Repository(RepositoryError),
    Boundary(RepositoryBoundaryError),
    Contract(TrustedSourceError),
}

pub struct TrustedSourceRetrievalService<A> {
    acquirer: A,
}

impl<A> TrustedSourceRetrievalService<A>
where
    A: SafeRepositoryAcquirer + RepositoryBoundaryAcquirer,
{
    #[must_use]
    pub const fn new(acquirer: A) -> Self {
        Self { acquirer }
    }

    /// Reacquires one immutable repository and resolves one selected evidence span.
    ///
    /// # Errors
    ///
    /// Returns a typed repository, boundary, or source-contract failure without
    /// exposing the local root or source bytes.
    pub fn retrieve<H>(
        &self,
        request: &TrustedSourceRequest,
        boundary_sha256: &H,
        source_sha256: R18Sha256,
    ) -> Result<TrustedSourceExcerptV1, TrustedSourceRetrievalError>
    where
        H: BoundarySha256,
    {
        let identity = request.selection.repository_identity().clone();
        let revision = Revision::Commit(request.selection.commit_oid().clone());
        let acquired = if request.boundary_profile {
            let acquired = self
                .acquirer
                .acquire_inventory_with_boundaries(&request.repository, identity, revision)
                .map_err(|error| match error {
                    RepositoryBoundaryAcquisitionError::Repository(error) => {
                        TrustedSourceRetrievalError::Repository(error)
                    }
                    RepositoryBoundaryAcquisitionError::Boundary(error) => {
                        TrustedSourceRetrievalError::Boundary(error)
                    }
                })?;
            let bound = acquired.repository.bound_revision().clone();
            let parsed = parse_gitmodules(&bound, acquired.gitmodules.as_ref(), boundary_sha256)
                .map_err(TrustedSourceRetrievalError::Boundary)?;
            let report = build_boundary_report(
                &bound,
                acquired.gitlinks.clone(),
                parsed,
                &[],
                boundary_sha256,
            )
            .map_err(TrustedSourceRetrievalError::Boundary)?;
            validate_repository_boundary_report_size(&report)
                .map_err(TrustedSourceRetrievalError::Boundary)?;
            acquired.repository
        } else {
            self.acquirer
                .acquire_inventory(&request.repository, identity, revision)
                .map_err(TrustedSourceRetrievalError::Repository)?
        };
        let inventory = RepositoryInventory::classify(acquired);
        TrustedSourceExcerptV1::from_inventory(&request.selection, &inventory, source_sha256)
            .map_err(TrustedSourceRetrievalError::Contract)
    }
}
