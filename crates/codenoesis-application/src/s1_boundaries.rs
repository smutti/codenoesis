use std::ffi::OsString;

use codenoesis_contracts::{
    RepositorySnapshotV4, RepositorySnapshotV5, validate_repository_boundary_report_size,
};
use codenoesis_domain::s1_boundaries::{
    BoundarySha256, NestedRepositoryAcquisitionError, RepositoryBoundaryAcquisitionError,
    RepositoryBoundaryError, RepositoryBoundaryInput, VerifiedNestedRepository,
    build_boundary_report, parse_gitmodules,
};
use codenoesis_domain::s5::AnalysisCacheEntry;
use codenoesis_domain::storage::LocalSnapshotHead;
use codenoesis_domain::{ObjectId, RepositoryError, RepositoryInventory, Revision};
use codenoesis_ports::{
    ArtifactStore, IncrementalRustWorkspaceExtractor, MetadataStore, PublicationObserver,
    RepositoryBoundaryAcquirer,
};

use crate::{PublicationService, ScanError, ScanRequest, ScanService};

pub struct RepositoryBoundaryScanInput {
    pub manifest: Option<RepositoryBoundaryInput>,
    pub nested_roots: Vec<PreparedNestedRepositoryRoot>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PreparedNestedRepositoryRoot {
    Available(OsString),
    Unavailable,
}

pub struct S4BoundaryScanOutput {
    pub snapshot: RepositorySnapshotV5,
    pub analysis_cache_entries: Vec<AnalysisCacheEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BoundaryScanError {
    Scan(ScanError),
    Boundary(RepositoryBoundaryError),
    InvalidManifest,
    NestedMismatch {
        path: String,
        expected: ObjectId,
        observed: ObjectId,
    },
    NestedUnavailable {
        path: String,
        error: RepositoryError,
    },
    NestedChanged {
        path: String,
    },
    NestedPathUnavailable {
        path: String,
    },
}

impl<A> ScanService<A>
where
    A: RepositoryBoundaryAcquirer,
{
    /// Executes the complete explicit S4-R2 scan without ambient nested authority.
    ///
    /// # Errors
    ///
    /// Returns a root lineage, boundary, manifest, or nested-binding failure.
    #[allow(clippy::too_many_lines)]
    pub fn scan_s4_boundaries<E, H>(
        &self,
        request: ScanRequest,
        input: RepositoryBoundaryScanInput,
        extractor: &E,
        hasher: &H,
    ) -> Result<S4BoundaryScanOutput, BoundaryScanError>
    where
        E: IncrementalRustWorkspaceExtractor,
        H: BoundarySha256,
    {
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
        let acquired = self
            .acquirer
            .acquire_inventory_with_boundaries(
                &request.repository,
                request.identity,
                request.revision,
            )
            .map_err(|error| match error {
                RepositoryBoundaryAcquisitionError::Repository(RepositoryError::Acquisition(
                    acquisition,
                )) => BoundaryScanError::Scan(ScanError::Acquisition(acquisition)),
                RepositoryBoundaryAcquisitionError::Boundary(boundary) => {
                    BoundaryScanError::Boundary(boundary)
                }
                RepositoryBoundaryAcquisitionError::Repository(RepositoryError::Unexpected) => {
                    BoundaryScanError::Scan(ScanError::Internal)
                }
            })?;
        let root = acquired.repository.bound_revision().clone();
        let parsed = parse_gitmodules(&root, acquired.gitmodules.as_ref(), hasher)
            .map_err(BoundaryScanError::Boundary)?;
        let mut verified = Vec::new();
        if let Some(manifest) = input.manifest {
            for (nested, nested_root) in manifest
                .nested_repositories
                .into_iter()
                .zip(input.nested_roots)
            {
                let Some(gitlink) = acquired
                    .gitlinks
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
        let report = build_boundary_report(&root, acquired.gitlinks, parsed, &verified, hasher)
            .map_err(BoundaryScanError::Boundary)?;
        validate_repository_boundary_report_size(&report).map_err(BoundaryScanError::Boundary)?;
        let inventory = RepositoryInventory::classify(acquired.repository);
        let extraction = extractor
            .extract_workspace_incremental(&inventory, &[])
            .map_err(|error| BoundaryScanError::Scan(ScanError::Workspace(error)))?;
        extraction
            .knowledge
            .validate()
            .map_err(|error| BoundaryScanError::Scan(ScanError::Workspace(error)))?;
        let v4 = RepositorySnapshotV4::from_inventory_and_workspace(
            &inventory,
            &extraction.knowledge,
            request.envelope,
        );
        let snapshot = RepositorySnapshotV5::from_v4_and_boundaries(&v4, &report).map_err(
            |error| match error {
                codenoesis_contracts::RepositorySnapshotV5Error::Boundary(error) => {
                    BoundaryScanError::Boundary(error)
                }
                codenoesis_contracts::RepositorySnapshotV5Error::LimitExceeded(error) => {
                    BoundaryScanError::Scan(ScanError::Acquisition(error))
                }
                codenoesis_contracts::RepositorySnapshotV5Error::Serialization(_)
                | codenoesis_contracts::RepositorySnapshotV5Error::OutputLengthOverflow => {
                    BoundaryScanError::Scan(ScanError::Internal)
                }
            },
        )?;
        Ok(S4BoundaryScanOutput {
            snapshot,
            analysis_cache_entries: extraction.cache_entries,
        })
    }
}

impl PublicationService {
    /// Publishes a validated v5 snapshot using the existing immutable storage protocol.
    ///
    /// # Errors
    ///
    /// Returns an error when validation or immutable publication fails.
    pub fn publish_v5<C, M>(
        snapshot: &RepositorySnapshotV5,
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
