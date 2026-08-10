use codenoesis_contracts::{RepositorySnapshotV16, RepositorySnapshotV16Error};
use codenoesis_domain::s4_r14::{ExpressionBindingError, ExpressionBindingExtraction};
use codenoesis_domain::s5::AnalysisCacheEntry;
use codenoesis_domain::storage::LocalSnapshotHead;
use codenoesis_domain::{K1OutputCapacityProfile, RepositoryInventory};
use codenoesis_ports::{ArtifactStore, MetadataStore, PublicationObserver, SafeRepositoryAcquirer};

use crate::{PublicationService, ScanError, ScanRequest, ScanService, map_repository_error};

pub struct S4R14ScanOutput {
    pub snapshot: RepositorySnapshotV16,
    pub analysis_cache_entries: Vec<AnalysisCacheEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExpressionBindingsScanError {
    Scan(ScanError),
    Expression(ExpressionBindingError),
    InvalidSnapshot,
}

impl<A> ScanService<A>
where
    A: SafeRepositoryAcquirer,
{
    /// Executes the exact K1 source-only lineage plus the R14 expression overlay.
    ///
    /// # Errors
    ///
    /// Returns an acquisition, extraction, validation, or snapshot failure.
    pub fn scan_s4_r14<F>(
        &self,
        request: ScanRequest,
        output_capacity_profile: K1OutputCapacityProfile,
        extractor: F,
    ) -> Result<S4R14ScanOutput, ExpressionBindingsScanError>
    where
        F: FnOnce(
            &RepositoryInventory,
        ) -> Result<ExpressionBindingExtraction, ExpressionBindingError>,
    {
        let acquired = self
            .acquirer
            .acquire_inventory(&request.repository, request.identity, request.revision)
            .map_err(map_repository_error)
            .map_err(ExpressionBindingsScanError::Scan)?;
        let inventory = RepositoryInventory::classify(acquired);
        let extraction = extractor(&inventory).map_err(ExpressionBindingsScanError::Expression)?;
        extraction
            .knowledge
            .validate()
            .map_err(ExpressionBindingsScanError::Expression)?;
        let snapshot = RepositorySnapshotV16::from_inventory_and_expression_bindings(
            &inventory,
            &extraction.knowledge,
            output_capacity_profile,
            request.envelope,
        )
        .map_err(map_snapshot_error)?;
        Ok(S4R14ScanOutput {
            snapshot,
            analysis_cache_entries: extraction.cache_entries,
        })
    }
}

impl PublicationService {
    /// Publishes one V16 snapshot through the immutable local-store protocol.
    ///
    /// # Errors
    ///
    /// Returns a typed storage/publication failure or internal contract error.
    pub fn publish_v16<C, M>(
        snapshot: &RepositorySnapshotV16,
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

fn map_snapshot_error(error: RepositorySnapshotV16Error) -> ExpressionBindingsScanError {
    match error {
        RepositorySnapshotV16Error::LimitExceeded(error) => {
            ExpressionBindingsScanError::Scan(ScanError::Acquisition(error))
        }
        RepositorySnapshotV16Error::Serialization(_)
        | RepositorySnapshotV16Error::OutputLengthOverflow => {
            ExpressionBindingsScanError::Scan(ScanError::Internal)
        }
        RepositorySnapshotV16Error::ContractInvalid => ExpressionBindingsScanError::InvalidSnapshot,
    }
}
