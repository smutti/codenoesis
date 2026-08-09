use codenoesis_contracts::{RepositorySnapshotV12, RepositorySnapshotV12Error};
use codenoesis_domain::RepositoryInventory;
use codenoesis_domain::s1_boundaries::{BoundarySha256, build_boundary_report, parse_gitmodules};
use codenoesis_domain::s4_r3::ExternalWorkspaceBoundary;
use codenoesis_domain::s4_r10::RustCfgDeclarationAlternativesError;
use codenoesis_domain::s5::AnalysisCacheEntry;
use codenoesis_domain::storage::LocalSnapshotHead;
use codenoesis_ports::{
    ArtifactStore, MetadataStore, PublicationObserver, RepositoryBoundaryAcquirer,
    RustCfgDeclarationAlternativesExtractor, SafeRepositoryAcquirer,
};

use crate::s4_r4::{map_boundary_acquisition, validate_boundary_input};
use crate::{
    BoundaryScanError, PublicationService, RepositoryBoundaryScanInput, ScanError, ScanRequest,
    ScanService, map_repository_error,
};

pub struct S4R10ScanOutput {
    pub snapshot: RepositorySnapshotV12,
    pub analysis_cache_entries: Vec<AnalysisCacheEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RustCfgAlternativesScanError {
    Scan(ScanError),
    Alternatives(RustCfgDeclarationAlternativesError),
    InvalidSnapshot,
    Boundary(BoundaryScanError),
}

impl<A> ScanService<A>
where
    A: SafeRepositoryAcquirer,
{
    /// Executes the explicit R10 cfg declaration alternatives profile.
    ///
    /// # Errors
    ///
    /// Returns inherited acquisition/R5 failures or a typed R10 alternative failure.
    pub fn scan_s4_r10<E>(
        &self,
        request: ScanRequest,
        extractor: &E,
    ) -> Result<S4R10ScanOutput, RustCfgAlternativesScanError>
    where
        E: RustCfgDeclarationAlternativesExtractor,
    {
        let acquired = self
            .acquirer
            .acquire_inventory(&request.repository, request.identity, request.revision)
            .map_err(map_repository_error)
            .map_err(RustCfgAlternativesScanError::Scan)?;
        let inventory = RepositoryInventory::classify(acquired);
        build_output(request.envelope, &inventory, None, extractor)
    }
}

impl<A> ScanService<A>
where
    A: RepositoryBoundaryAcquirer,
{
    /// Executes R10 with the explicit R2 boundary projection and no nested source authority.
    ///
    /// # Errors
    ///
    /// Returns inherited boundary/acquisition failures or a typed R10 alternative failure.
    pub fn scan_s4_r10_boundaries<E, H>(
        &self,
        request: ScanRequest,
        input: RepositoryBoundaryScanInput,
        extractor: &E,
        hasher: &H,
    ) -> Result<S4R10ScanOutput, RustCfgAlternativesScanError>
    where
        E: RustCfgDeclarationAlternativesExtractor,
        H: BoundarySha256,
    {
        validate_boundary_input(&request, &input)
            .map_err(RustCfgAlternativesScanError::Boundary)?;
        let acquired = self
            .acquirer
            .acquire_inventory_with_boundaries(
                &request.repository,
                request.identity,
                request.revision,
            )
            .map_err(map_boundary_acquisition)
            .map_err(RustCfgAlternativesScanError::Boundary)?;
        let root = acquired.repository.bound_revision().clone();
        let parsed = parse_gitmodules(&root, acquired.gitmodules.as_ref(), hasher)
            .map_err(BoundaryScanError::Boundary)
            .map_err(RustCfgAlternativesScanError::Boundary)?;
        let verified = self
            .verify_r4_nested_boundaries(&acquired.gitlinks, input)
            .map_err(RustCfgAlternativesScanError::Boundary)?;
        let report = build_boundary_report(&root, acquired.gitlinks, parsed, &verified, hasher)
            .map_err(BoundaryScanError::Boundary)
            .map_err(RustCfgAlternativesScanError::Boundary)?;
        codenoesis_contracts::validate_repository_boundary_report_size(&report)
            .map_err(BoundaryScanError::Boundary)
            .map_err(RustCfgAlternativesScanError::Boundary)?;
        let inventory = RepositoryInventory::classify(acquired.repository);
        build_output(request.envelope, &inventory, Some(&report), extractor)
    }
}

impl PublicationService {
    /// Publishes one V12 snapshot through the unchanged immutable local-store protocol.
    ///
    /// # Errors
    ///
    /// Returns a typed storage/publication failure or internal contract error.
    pub fn publish_v12<C, M>(
        snapshot: &RepositorySnapshotV12,
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

fn build_output<E>(
    envelope: codenoesis_contracts::SnapshotEnvelopeV1,
    inventory: &RepositoryInventory,
    boundaries: Option<&codenoesis_domain::s1_boundaries::RepositoryBoundaryReport>,
    extractor: &E,
) -> Result<S4R10ScanOutput, RustCfgAlternativesScanError>
where
    E: RustCfgDeclarationAlternativesExtractor,
{
    let external_boundaries = boundaries
        .into_iter()
        .flat_map(|report| &report.boundaries)
        .map(|boundary| ExternalWorkspaceBoundary {
            path: boundary.path.clone(),
            boundary_id: boundary.boundary_id.clone(),
        })
        .collect::<Vec<_>>();
    let extraction = extractor
        .extract_rust_cfg_declaration_alternatives_incremental(inventory, &external_boundaries, &[])
        .map_err(RustCfgAlternativesScanError::Alternatives)?;
    extraction
        .knowledge
        .validate()
        .map_err(RustCfgAlternativesScanError::Alternatives)?;
    let snapshot = RepositorySnapshotV12::from_inventory_and_cfg_declaration_alternatives(
        inventory,
        &extraction.knowledge,
        boundaries,
        envelope,
    )
    .map_err(map_snapshot_error)?;
    Ok(S4R10ScanOutput {
        snapshot,
        analysis_cache_entries: extraction.cache_entries,
    })
}

fn map_snapshot_error(error: RepositorySnapshotV12Error) -> RustCfgAlternativesScanError {
    match error {
        RepositorySnapshotV12Error::LimitExceeded(error) => {
            RustCfgAlternativesScanError::Scan(ScanError::Acquisition(error))
        }
        RepositorySnapshotV12Error::ContractInvalid => {
            RustCfgAlternativesScanError::InvalidSnapshot
        }
        RepositorySnapshotV12Error::Serialization(_)
        | RepositorySnapshotV12Error::OutputLengthOverflow => {
            RustCfgAlternativesScanError::Scan(ScanError::Internal)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pt_fr_ext_013_invalid_v12_contract_remains_typed() {
        assert_eq!(
            map_snapshot_error(RepositorySnapshotV12Error::ContractInvalid),
            RustCfgAlternativesScanError::InvalidSnapshot
        );
    }
}
