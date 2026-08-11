use codenoesis_contracts::{RepositorySnapshotV17, RepositorySnapshotV17Error};
use codenoesis_domain::s4_r15::{LocalFlowError, LocalFlowExtraction};
use codenoesis_domain::s5::AnalysisCacheEntry;
use codenoesis_domain::storage::LocalSnapshotHead;
use codenoesis_domain::{K1OutputCapacityProfile, RepositoryInventory};
use codenoesis_ports::{ArtifactStore, MetadataStore, PublicationObserver, SafeRepositoryAcquirer};

use crate::{PublicationService, ScanError, ScanRequest, ScanService, map_repository_error};

pub struct S4R15ScanOutput {
    pub snapshot: RepositorySnapshotV17,
    pub analysis_cache_entries: Vec<AnalysisCacheEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LocalFlowScanError {
    Scan(ScanError),
    LocalFlow(LocalFlowError),
    InvalidSnapshot,
}

impl<A> ScanService<A>
where
    A: SafeRepositoryAcquirer,
{
    /// Executes the exact R14 source-only lineage plus the R15 local-flow overlay.
    ///
    /// # Errors
    ///
    /// Returns an acquisition, extraction, validation, or snapshot failure.
    pub fn scan_s4_r15<F>(
        &self,
        request: ScanRequest,
        output_capacity_profile: K1OutputCapacityProfile,
        extractor: F,
    ) -> Result<S4R15ScanOutput, LocalFlowScanError>
    where
        F: FnOnce(&RepositoryInventory) -> Result<LocalFlowExtraction, LocalFlowError>,
    {
        let acquired = self
            .acquirer
            .acquire_inventory(&request.repository, request.identity, request.revision)
            .map_err(map_repository_error)
            .map_err(LocalFlowScanError::Scan)?;
        let inventory = RepositoryInventory::classify(acquired);
        let extraction = extractor(&inventory).map_err(LocalFlowScanError::LocalFlow)?;
        extraction
            .knowledge
            .validate()
            .map_err(LocalFlowScanError::LocalFlow)?;
        let snapshot = RepositorySnapshotV17::from_inventory_and_local_flow(
            &inventory,
            &extraction.knowledge,
            output_capacity_profile,
            request.envelope,
        )
        .map_err(map_snapshot_error)?;
        Ok(S4R15ScanOutput {
            snapshot,
            analysis_cache_entries: extraction.cache_entries,
        })
    }
}

impl PublicationService {
    /// Publishes one V17 snapshot through the immutable local-store protocol.
    ///
    /// # Errors
    ///
    /// Returns a typed storage/publication failure or internal contract error.
    pub fn publish_v17<C, M>(
        snapshot: &RepositorySnapshotV17,
        artifact_store: &mut C,
        metadata_store: &mut M,
        observer: &mut dyn PublicationObserver,
    ) -> Result<LocalSnapshotHead, ScanError>
    where
        C: ArtifactStore,
        M: MetadataStore,
    {
        let candidate = snapshot
            .publication_candidate()
            .map_err(|_| ScanError::Internal)?;
        Self::publish_candidate(&candidate, artifact_store, metadata_store, observer)
    }
}

fn map_snapshot_error(error: RepositorySnapshotV17Error) -> LocalFlowScanError {
    match error {
        RepositorySnapshotV17Error::LimitExceeded(error) => {
            LocalFlowScanError::Scan(ScanError::Acquisition(error))
        }
        RepositorySnapshotV17Error::Serialization(_)
        | RepositorySnapshotV17Error::OutputLengthOverflow => {
            LocalFlowScanError::Scan(ScanError::Internal)
        }
        RepositorySnapshotV17Error::ContractInvalid => LocalFlowScanError::InvalidSnapshot,
    }
}
