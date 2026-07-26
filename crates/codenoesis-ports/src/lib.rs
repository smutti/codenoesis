//! Inward-owned ports for the `CodeNoesis` S0 and S1 acquisition slices.

use std::ffi::OsStr;

use codenoesis_domain::knowledge::{KnowledgeError, RustKnowledge};
use codenoesis_domain::{
    AcquiredRepository, BoundRevision, RepositoryError, RepositoryIdentity, RepositoryInventory,
    Revision,
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

pub trait RustKnowledgeExtractor {
    /// Extracts and validates the approved S2 Rust knowledge subset.
    ///
    /// # Errors
    ///
    /// Returns a typed extraction, ontology, or graph failure without partial
    /// publication.
    fn extract(&self, inventory: &RepositoryInventory) -> Result<RustKnowledge, KnowledgeError>;
}
