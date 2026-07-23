//! Inward-owned ports for the `CodeNoesis` S0 acquisition slice.

use std::ffi::OsStr;

use codenoesis_domain::{BoundRevision, RepositoryError, RepositoryIdentity, Revision};

pub trait RepositoryAcquirer {
    /// Resolves and verifies the complete supported repository closure once.
    ///
    /// # Errors
    ///
    /// Returns a typed repository failure without exposing the source locator.
    fn bind(
        &self,
        repository: &OsStr,
        identity: RepositoryIdentity,
        revision: Revision,
    ) -> Result<BoundRevision, RepositoryError>;
}
