use codenoesis_contracts::{RepositorySnapshotV6, RepositorySnapshotV6Error};
use codenoesis_domain::RepositoryInventory;
use codenoesis_domain::s1_boundaries::{
    BoundarySha256, NestedRepositoryAcquisitionError, RepositoryBoundaryAcquisitionError,
    VerifiedNestedRepository, build_boundary_report, parse_gitmodules,
};
use codenoesis_domain::s4_r3::{ExternalWorkspaceBoundary, RootPackageWorkspaceError};
use codenoesis_domain::s5::AnalysisCacheEntry;
use codenoesis_domain::storage::LocalSnapshotHead;
use codenoesis_domain::{RepositoryError, Revision};
use codenoesis_ports::{
    ArtifactStore, MetadataStore, PublicationObserver, RepositoryBoundaryAcquirer,
    RootPackageWorkspaceExtractor, SafeRepositoryAcquirer,
};

use crate::{
    BoundaryScanError, PreparedNestedRepositoryRoot, PublicationService,
    RepositoryBoundaryScanInput, ScanError, ScanRequest, ScanService, map_repository_error,
};

pub struct S4R3ScanOutput {
    pub snapshot: RepositorySnapshotV6,
    pub analysis_cache_entries: Vec<AnalysisCacheEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RootPackageScanError {
    Scan(ScanError),
    Workspace(RootPackageWorkspaceError),
    Boundary(BoundaryScanError),
}

impl<A> ScanService<A>
where
    A: SafeRepositoryAcquirer,
{
    /// Executes the explicit R3 root-package profile over one safe root inventory.
    ///
    /// # Errors
    ///
    /// Returns an inherited acquisition failure or a typed R3 extraction failure.
    pub fn scan_s4_r3<E>(
        &self,
        request: ScanRequest,
        extractor: &E,
    ) -> Result<S4R3ScanOutput, RootPackageScanError>
    where
        E: RootPackageWorkspaceExtractor,
    {
        let acquired = self
            .acquirer
            .acquire_inventory(&request.repository, request.identity, request.revision)
            .map_err(map_repository_error)
            .map_err(RootPackageScanError::Scan)?;
        let inventory = RepositoryInventory::classify(acquired);
        build_output(request.envelope, inventory, None, extractor)
    }
}

impl<A> ScanService<A>
where
    A: RepositoryBoundaryAcquirer,
{
    /// Executes R3 with the explicit R2 boundary projection and no nested source authority.
    ///
    /// # Errors
    ///
    /// Returns the inherited R2 lineage or a typed R3 extraction failure.
    pub fn scan_s4_r3_boundaries<E, H>(
        &self,
        request: ScanRequest,
        input: RepositoryBoundaryScanInput,
        extractor: &E,
        hasher: &H,
    ) -> Result<S4R3ScanOutput, RootPackageScanError>
    where
        E: RootPackageWorkspaceExtractor,
        H: BoundarySha256,
    {
        validate_boundary_input(&request, &input).map_err(RootPackageScanError::Boundary)?;
        let acquired = self
            .acquirer
            .acquire_inventory_with_boundaries(
                &request.repository,
                request.identity,
                request.revision,
            )
            .map_err(map_boundary_acquisition)
            .map_err(RootPackageScanError::Boundary)?;
        let root = acquired.repository.bound_revision().clone();
        let parsed = parse_gitmodules(&root, acquired.gitmodules.as_ref(), hasher)
            .map_err(BoundaryScanError::Boundary)
            .map_err(RootPackageScanError::Boundary)?;
        let verified = self
            .verify_nested_boundaries(&acquired.gitlinks, input)
            .map_err(RootPackageScanError::Boundary)?;
        let report = build_boundary_report(&root, acquired.gitlinks, parsed, &verified, hasher)
            .map_err(BoundaryScanError::Boundary)
            .map_err(RootPackageScanError::Boundary)?;
        codenoesis_contracts::validate_repository_boundary_report_size(&report)
            .map_err(BoundaryScanError::Boundary)
            .map_err(RootPackageScanError::Boundary)?;
        let inventory = RepositoryInventory::classify(acquired.repository);
        build_output(request.envelope, inventory, Some(&report), extractor)
    }

    fn verify_nested_boundaries(
        &self,
        gitlinks: &[codenoesis_domain::s1_boundaries::AcquiredGitlink],
        input: RepositoryBoundaryScanInput,
    ) -> Result<Vec<VerifiedNestedRepository>, BoundaryScanError> {
        let mut verified = Vec::new();
        if let Some(manifest) = input.manifest {
            for (nested, nested_root) in manifest
                .nested_repositories
                .into_iter()
                .zip(input.nested_roots)
            {
                let Some(gitlink) = gitlinks
                    .iter()
                    .find(|gitlink| gitlink.path == nested.boundary_path)
                else {
                    return Err(BoundaryScanError::InvalidManifest);
                };
                if gitlink.gitlink_oid != nested.revision {
                    return Err(BoundaryScanError::NestedMismatch {
                        path: nested.boundary_path,
                        expected: gitlink.gitlink_oid.clone(),
                        observed: nested.revision,
                    });
                }
                let PreparedNestedRepositoryRoot::Available(nested_root) = nested_root else {
                    return Err(BoundaryScanError::NestedPathUnavailable {
                        path: nested.boundary_path,
                    });
                };
                let bound = self
                    .acquirer
                    .bind_nested_repository(
                        &nested_root,
                        nested.repository_identity,
                        Revision::Commit(nested.revision),
                        nested.acquisition_profile,
                    )
                    .map_err(|error| match error {
                        NestedRepositoryAcquisitionError::Repository(error) => {
                            BoundaryScanError::NestedUnavailable {
                                path: nested.boundary_path.clone(),
                                error,
                            }
                        }
                        NestedRepositoryAcquisitionError::Changed => {
                            BoundaryScanError::NestedChanged {
                                path: nested.boundary_path.clone(),
                            }
                        }
                    })?;
                verified.push(VerifiedNestedRepository {
                    boundary_path: nested.boundary_path,
                    bound_revision: bound,
                });
            }
        }
        Ok(verified)
    }
}

impl PublicationService {
    /// Publishes one V6 snapshot through the unchanged immutable local-store protocol.
    ///
    /// # Errors
    ///
    /// Returns a typed storage/publication failure or internal contract error.
    pub fn publish_v6<C, M>(
        snapshot: &RepositorySnapshotV6,
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

fn validate_boundary_input(
    request: &ScanRequest,
    input: &RepositoryBoundaryScanInput,
) -> Result<(), BoundaryScanError> {
    if input
        .manifest
        .as_ref()
        .is_some_and(|manifest| manifest.nested_repositories.len() != input.nested_roots.len())
        || input.manifest.is_none() && !input.nested_roots.is_empty()
    {
        return Err(BoundaryScanError::InvalidManifest);
    }
    if let Some(manifest) = &input.manifest
        && (manifest.root_repository_identity != request.identity
            || !matches!(
                &request.revision,
                Revision::Commit(commit) if commit == &manifest.root_commit_oid
            ))
    {
        return Err(BoundaryScanError::InvalidManifest);
    }
    Ok(())
}

fn map_boundary_acquisition(error: RepositoryBoundaryAcquisitionError) -> BoundaryScanError {
    match error {
        RepositoryBoundaryAcquisitionError::Repository(RepositoryError::Acquisition(error)) => {
            BoundaryScanError::Scan(ScanError::Acquisition(error))
        }
        RepositoryBoundaryAcquisitionError::Boundary(error) => BoundaryScanError::Boundary(error),
        RepositoryBoundaryAcquisitionError::Repository(RepositoryError::Unexpected) => {
            BoundaryScanError::Scan(ScanError::Internal)
        }
    }
}

fn build_output<E>(
    envelope: codenoesis_contracts::SnapshotEnvelopeV1,
    inventory: RepositoryInventory,
    boundaries: Option<&codenoesis_domain::s1_boundaries::RepositoryBoundaryReport>,
    extractor: &E,
) -> Result<S4R3ScanOutput, RootPackageScanError>
where
    E: RootPackageWorkspaceExtractor,
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
        .extract_root_package_workspace_incremental(&inventory, &external_boundaries, &[])
        .map_err(RootPackageScanError::Workspace)?;
    extraction
        .knowledge
        .validate()
        .map_err(RootPackageScanError::Workspace)?;
    let snapshot = RepositorySnapshotV6::from_inventory_and_workspace(
        &inventory,
        &extraction.knowledge,
        boundaries,
        envelope,
    )
    .map_err(map_snapshot_error)?;
    Ok(S4R3ScanOutput {
        snapshot,
        analysis_cache_entries: extraction.cache_entries,
    })
}

fn map_snapshot_error(error: RepositorySnapshotV6Error) -> RootPackageScanError {
    match error {
        RepositorySnapshotV6Error::LimitExceeded(error) => {
            RootPackageScanError::Scan(ScanError::Acquisition(error))
        }
        RepositorySnapshotV6Error::Serialization(_)
        | RepositorySnapshotV6Error::ContractInvalid
        | RepositorySnapshotV6Error::OutputLengthOverflow => {
            RootPackageScanError::Scan(ScanError::Internal)
        }
    }
}
