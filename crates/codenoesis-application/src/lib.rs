//! Application orchestration for the `CodeNoesis` S0 slice.

use std::ffi::OsString;

use codenoesis_contracts::{RepositorySnapshotV1, SnapshotEnvelopeV1};
use codenoesis_domain::{AcquisitionError, RepositoryError, RepositoryIdentity, Revision};
use codenoesis_ports::RepositoryAcquirer;

pub struct ScanRequest {
    repository: OsString,
    identity: RepositoryIdentity,
    revision: Revision,
    envelope: SnapshotEnvelopeV1,
}

impl ScanRequest {
    #[must_use]
    pub const fn new(
        repository: OsString,
        identity: RepositoryIdentity,
        revision: Revision,
        envelope: SnapshotEnvelopeV1,
    ) -> Self {
        Self {
            repository,
            identity,
            revision,
            envelope,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScanError {
    Acquisition(AcquisitionError),
    Internal,
}

pub struct ScanService<A> {
    acquirer: A,
}

impl<A> ScanService<A>
where
    A: RepositoryAcquirer,
{
    #[must_use]
    pub const fn new(acquirer: A) -> Self {
        Self { acquirer }
    }

    /// Executes an S0 scan from one immutable repository binding.
    ///
    /// # Errors
    ///
    /// Returns a typed acquisition failure or a redacted internal failure.
    pub fn scan(&self, request: ScanRequest) -> Result<RepositorySnapshotV1, ScanError> {
        let bound = self
            .acquirer
            .bind(&request.repository, request.identity, request.revision)
            .map_err(|error| match error {
                RepositoryError::Acquisition(acquisition) => ScanError::Acquisition(acquisition),
                RepositoryError::Unexpected => ScanError::Internal,
            })?;
        Ok(RepositorySnapshotV1::from_bound_revision(
            &bound,
            request.envelope,
        ))
    }
}
