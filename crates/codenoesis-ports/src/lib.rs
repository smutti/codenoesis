//! Inward-owned ports for the `CodeNoesis` S0 and S1 acquisition slices.

use std::ffi::OsStr;

use codenoesis_domain::{
    AcquiredRepository, BoundRevision, RepositoryError, RepositoryIdentity, Revision,
};

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

pub trait SafeRepositoryAcquirer {
    /// Resolves, verifies, and safely inventories the approved S1 repository subset.
    ///
    /// # Errors
    ///
    /// Returns a typed repository failure without exposing the source locator.
    fn acquire_inventory(
        &self,
        repository: &OsStr,
        identity: RepositoryIdentity,
        revision: Revision,
    ) -> Result<AcquiredRepository, RepositoryError>;
}
