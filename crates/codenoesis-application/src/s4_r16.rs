use codenoesis_contracts::{RepositorySnapshotV18, RepositorySnapshotV18Error};
use codenoesis_domain::s4_r16::{ConstantEvaluationError, ConstantEvaluationExtraction};
use codenoesis_domain::s5::AnalysisCacheEntry;
use codenoesis_domain::storage::LocalSnapshotHead;
use codenoesis_domain::{K1OutputCapacityProfile, RepositoryInventory};
use codenoesis_ports::{ArtifactStore, MetadataStore, PublicationObserver, SafeRepositoryAcquirer};

use crate::{PublicationService, ScanError, ScanRequest, ScanService, map_repository_error};

pub struct S4R16ScanOutput {
    pub snapshot: RepositorySnapshotV18,
    pub analysis_cache_entries: Vec<AnalysisCacheEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConstantEvaluationScanError {
    Scan(ScanError),
    ConstantEvaluation(ConstantEvaluationError),
    InvalidSnapshot,
}

impl<A> ScanService<A>
where
    A: SafeRepositoryAcquirer,
{
    /// Executes the exact R15 source-only lineage plus the R16 constant-evaluation overlay.
    ///
    /// # Errors
    ///
    /// Returns an acquisition, extraction, validation, or snapshot failure.
    pub fn scan_s4_r16<F>(
        &self,
        request: ScanRequest,
        output_capacity_profile: K1OutputCapacityProfile,
        extractor: F,
    ) -> Result<S4R16ScanOutput, ConstantEvaluationScanError>
    where
        F: FnOnce(
            &RepositoryInventory,
        ) -> Result<ConstantEvaluationExtraction, ConstantEvaluationError>,
    {
        let acquired = self
            .acquirer
            .acquire_inventory(&request.repository, request.identity, request.revision)
            .map_err(map_repository_error)
            .map_err(ConstantEvaluationScanError::Scan)?;
        let inventory = RepositoryInventory::classify(acquired);
        let extraction =
            extractor(&inventory).map_err(ConstantEvaluationScanError::ConstantEvaluation)?;
        extraction
            .knowledge
            .validate()
            .map_err(ConstantEvaluationScanError::ConstantEvaluation)?;
        let snapshot = RepositorySnapshotV18::from_inventory_and_constant_evaluation(
            &inventory,
            &extraction.knowledge,
            output_capacity_profile,
            request.envelope,
        )
        .map_err(map_snapshot_error)?;
        Ok(S4R16ScanOutput {
            snapshot,
            analysis_cache_entries: extraction.cache_entries,
        })
    }
}

impl PublicationService {
    /// Publishes one V18 snapshot through the immutable local-store protocol.
    ///
    /// # Errors
    ///
    /// Returns a typed storage/publication failure or internal contract error.
    pub fn publish_v18<C, M>(
        snapshot: &RepositorySnapshotV18,
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

fn map_snapshot_error(error: RepositorySnapshotV18Error) -> ConstantEvaluationScanError {
    match error {
        RepositorySnapshotV18Error::LimitExceeded(error) => {
            ConstantEvaluationScanError::Scan(ScanError::Acquisition(error))
        }
        RepositorySnapshotV18Error::Serialization(_)
        | RepositorySnapshotV18Error::OutputLengthOverflow => {
            ConstantEvaluationScanError::Scan(ScanError::Internal)
        }
        RepositorySnapshotV18Error::ContractInvalid => ConstantEvaluationScanError::InvalidSnapshot,
    }
}
