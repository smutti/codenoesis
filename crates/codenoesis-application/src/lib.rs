//! Application orchestration for the `CodeNoesis` S0 and S1 slices.

use std::ffi::OsString;

use codenoesis_contracts::{RepositorySnapshotV1, RepositorySnapshotV2, SnapshotEnvelopeV1};
use codenoesis_domain::{
    AcquisitionError, RepositoryError, RepositoryIdentity, RepositoryInventory, Revision,
};
use codenoesis_ports::{RepositoryAcquirer, SafeRepositoryAcquirer};

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

impl<A> ScanService<A>
where
    A: SafeRepositoryAcquirer,
{
    /// Executes an S1 safe-inventory scan from one immutable repository binding.
    ///
    /// # Errors
    ///
    /// Returns a typed acquisition or policy failure, or a redacted internal failure.
    pub fn scan_s1(&self, request: ScanRequest) -> Result<RepositorySnapshotV2, ScanError> {
        let acquired = self
            .acquirer
            .acquire_inventory(&request.repository, request.identity, request.revision)
            .map_err(|error| match error {
                RepositoryError::Acquisition(acquisition) => ScanError::Acquisition(acquisition),
                RepositoryError::Unexpected => ScanError::Internal,
            })?;
        let inventory = RepositoryInventory::classify(acquired);
        Ok(RepositorySnapshotV2::from_inventory(
            &inventory,
            request.envelope,
        ))
    }
}
