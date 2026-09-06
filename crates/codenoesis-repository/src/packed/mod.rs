mod catalog;
mod delta;
mod hash;
mod index;
mod pack;

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use codenoesis_domain::s1_packed::{
    PackedAcquisitionError, PackedComponent, PackedDeltaReason, PackedObjectDatabaseInvalid,
    PackedObjectReason,
};
use codenoesis_domain::{
    AcquisitionError, LimitKind, ObjectId, RepositoryError, UnsupportedFeature, limit_exceeded,
};
use flate2::read::ZlibDecoder;

use self::catalog::open_catalog;
use self::hash::CollisionHasher;
use self::index::parse_index;
use self::pack::{PackEntry, PackEntryKind, ValidatedPack};
use crate::{DeclaredBodyLimit, GitObject, GitObjectKind, ReadObjectError, parse_object_header};

const SELECTED_LOOSE_COMPRESSED_BYTES: u64 = 271_122_432;

pub(super) struct PackedObjectDatabase {
    git_dir: PathBuf,
    packs: Vec<ValidatedPack>,
    budgets: PackBudgets,
    cache: BTreeMap<ObjectId, ResolvedObject>,
    cache_bytes: u64,
}

#[derive(Default)]
pub(super) struct PackBudgets {
    cumulative_index_bytes: u64,
    indexed_objects: u64,
    cumulative_pack_bytes: u64,
    cumulative_inflate_bytes: u64,
    cumulative_delta_work_bytes: u64,
}

impl PackBudgets {
    fn charge_index(&mut self, bytes: u64) -> Result<(), AcquisitionError> {
        if bytes > LimitKind::SinglePackIndexBytes.maximum() {
            return Err(limit_exceeded(LimitKind::SinglePackIndexBytes, bytes));
        }
        self.cumulative_index_bytes = checked_charge(
            LimitKind::CumulativePackIndexBytes,
            self.cumulative_index_bytes,
            bytes,
        )?;
        Ok(())
    }

    fn charge_objects(&mut self, objects: usize) -> Result<(), AcquisitionError> {
        let objects = u64::try_from(objects).unwrap_or(u64::MAX);
        self.indexed_objects =
            checked_charge(LimitKind::IndexedObjects, self.indexed_objects, objects)?;
        Ok(())
    }

    fn charge_pack(&mut self, bytes: u64) -> Result<(), AcquisitionError> {
        if bytes > LimitKind::SinglePackBytes.maximum() {
            return Err(limit_exceeded(LimitKind::SinglePackBytes, bytes));
        }
        self.cumulative_pack_bytes = checked_charge(
            LimitKind::CumulativeVerifiedPackBytes,
            self.cumulative_pack_bytes,
            bytes,
        )?;
        Ok(())
    }

    pub(super) fn charge_inflate(&mut self, bytes: u64) -> Result<(), AcquisitionError> {
        self.cumulative_inflate_bytes = checked_charge(
            LimitKind::CumulativeEntryInflateBytes,
            self.cumulative_inflate_bytes,
            bytes,
        )?;
        Ok(())
    }

    pub(super) fn charge_delta_work(&mut self, bytes: u64) -> Result<(), AcquisitionError> {
        self.cumulative_delta_work_bytes = checked_charge(
            LimitKind::CumulativeDeltaWorkBytes,
            self.cumulative_delta_work_bytes,
            bytes,
        )?;
        Ok(())
    }
}

#[derive(Clone)]
struct ResolvedObject {
    kind: GitObjectKind,
    body: Arc<[u8]>,
}

#[derive(Clone, Copy, Default)]
pub(super) struct RequestedObjectBounds {
    declared_body_limit: Option<DeclaredBodyLimit>,
    body_ceiling: Option<usize>,
}

impl RequestedObjectBounds {
    fn check(
        self,
        kind: GitObjectKind,
        body_size: usize,
        object_oid: &ObjectId,
    ) -> Result<(), AcquisitionError> {
        if kind == GitObjectKind::Blob
            && let Some(limit) = self.declared_body_limit
            && body_size > limit.body_maximum
        {
            let maximum = limit
                .observed_offset
                .saturating_add(u64::try_from(limit.body_maximum).unwrap_or(u64::MAX));
            return Err(AcquisitionError::LimitExceeded {
                limit: limit.limit,
                maximum,
                observed: maximum.saturating_add(1),
            });
        }
        if self.body_ceiling.is_some_and(|maximum| body_size > maximum) {
            return Err(object_invalid(object_oid, PackedObjectReason::Size));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum ObjectLocation {
    Loose,
    Pack { pack: usize, offset: u64 },
}

#[derive(Clone)]
struct DeltaCause {
    pack_id: ObjectId,
    object_oid: ObjectId,
}

enum ResolveTask {
    ResolveOid {
        oid: ObjectId,
        depth: u64,
        cause: Option<DeltaCause>,
    },
    FinishOid {
        oid: ObjectId,
        locations: usize,
    },
    ResolveLocation {
        oid: ObjectId,
        location: ObjectLocation,
        depth: u64,
    },
    ApplyDelta {
        oid: ObjectId,
        pack: usize,
        entry: PackEntry,
        program: Vec<u8>,
        bounds: RequestedObjectBounds,
    },
}

impl PackedObjectDatabase {
    pub(super) fn open(git_dir: &Path) -> Result<Self, RepositoryError> {
        let pairs = open_catalog(git_dir).map_err(RepositoryError::from)?;
        let mut budgets = PackBudgets::default();
        let mut packs = Vec::with_capacity(pairs.len());
        for mut pair in pairs {
            budgets
                .charge_index(pair.index.len())
                .map_err(RepositoryError::from)?;
            budgets
                .charge_pack(pair.pack.len())
                .map_err(RepositoryError::from)?;
            let index = parse_index(&mut pair.index, &pair.pack_id, pair.pack.len())
                .map_err(RepositoryError::from)?;
            budgets
                .charge_objects(index.rows.len())
                .map_err(RepositoryError::from)?;
            let pack =
                ValidatedPack::open(pair, index, &mut budgets).map_err(RepositoryError::from)?;
            packs.push(pack);
        }
        Ok(Self {
            git_dir: git_dir.to_path_buf(),
            packs,
            budgets,
            cache: BTreeMap::new(),
            cache_bytes: 0,
        })
    }

    pub(super) fn read_object(
        &mut self,
        object_id: &ObjectId,
        capture_limit: Option<usize>,
        declared_body_limit: Option<DeclaredBodyLimit>,
        body_ceiling: Option<usize>,
    ) -> Result<Option<GitObject>, ReadObjectError> {
        let bounds = RequestedObjectBounds {
            declared_body_limit,
            body_ceiling,
        };
        let locations = self
            .locations(object_id)
            .map_err(ReadObjectError::Acquisition)?;
        if locations.is_empty() {
            return Ok(None);
        }
        let object = self
            .resolve(object_id.clone(), locations, bounds)
            .map_err(ReadObjectError::Acquisition)?;
        let default_capture = match object.kind {
            GitObjectKind::Commit | GitObjectKind::Blob => 64,
            GitObjectKind::Tree => 512,
            GitObjectKind::Tag => 0,
        };
        let capture = capture_limit
            .unwrap_or(default_capture)
            .min(object.body.len());
        Ok(Some(GitObject {
            kind: object.kind,
            body_prefix: object.body[..capture].to_vec(),
            body_size: object.body.len(),
        }))
    }

    pub(super) fn verify_unchanged(&self) -> Result<(), RepositoryError> {
        for pack in &self.packs {
            pack.pair
                .index
                .verify_unchanged(PackedComponent::Index)
                .map_err(RepositoryError::from)?;
            pack.pair
                .pack
                .verify_unchanged(PackedComponent::Pack)
                .map_err(RepositoryError::from)?;
        }
        Ok(())
    }

    fn locations(&self, oid: &ObjectId) -> Result<Vec<ObjectLocation>, AcquisitionError> {
        let mut locations = Vec::new();
        if self.loose_exists(oid)? {
            locations.push(ObjectLocation::Loose);
        }
        for (pack, candidate) in self.packs.iter().enumerate() {
            if let Some(entry) = candidate.entry_for_oid(oid) {
                locations.push(ObjectLocation::Pack {
                    pack,
                    offset: entry.offset,
                });
            }
        }
        let observed = u64::try_from(locations.len()).unwrap_or(u64::MAX);
        if observed > LimitKind::ObjectLocations.maximum() {
            return Err(limit_exceeded(LimitKind::ObjectLocations, observed));
        }
        Ok(locations)
    }

    #[allow(clippy::too_many_lines)]
    fn resolve(
        &mut self,
        oid: ObjectId,
        locations: Vec<ObjectLocation>,
        requested_bounds: RequestedObjectBounds,
    ) -> Result<ResolvedObject, AcquisitionError> {
        let requested_oid = oid.clone();
        let mut tasks = vec![ResolveTask::ResolveOid {
            oid,
            depth: 0,
            cause: None,
        }];
        let mut values = Vec::<ResolvedObject>::new();
        let mut active = BTreeSet::<ObjectId>::new();
        let initial_locations = locations;
        while let Some(task) = tasks.pop() {
            match task {
                ResolveTask::ResolveOid { oid, depth, cause } => {
                    if active.contains(&oid) {
                        let cause = cause.expect("only delta traversal can revisit an active OID");
                        return Err(invalid(PackedObjectDatabaseInvalid::Delta {
                            reason: PackedDeltaReason::Cycle,
                            pack_id: cause.pack_id,
                            object_oid: cause.object_oid,
                        }));
                    }
                    if let Some(cached) = self.cache.get(&oid) {
                        if oid == requested_oid {
                            requested_bounds.check(cached.kind, cached.body.len(), &oid)?;
                        }
                        values.push(cached.clone());
                        continue;
                    }
                    check_delta_depth(depth)?;
                    let locations = if depth == 0 {
                        initial_locations.clone()
                    } else {
                        self.locations(&oid)?
                    };
                    if locations.is_empty() {
                        let cause = cause.expect("top-level locations were checked");
                        return Err(invalid(PackedObjectDatabaseInvalid::Delta {
                            reason: PackedDeltaReason::Base,
                            pack_id: cause.pack_id,
                            object_oid: cause.object_oid,
                        }));
                    }
                    active.insert(oid.clone());
                    tasks.push(ResolveTask::FinishOid {
                        oid: oid.clone(),
                        locations: locations.len(),
                    });
                    for location in locations.into_iter().rev() {
                        tasks.push(ResolveTask::ResolveLocation {
                            oid: oid.clone(),
                            location,
                            depth,
                        });
                    }
                }
                ResolveTask::FinishOid { oid, locations } => {
                    let start = values
                        .len()
                        .checked_sub(locations)
                        .expect("each location returns one object");
                    let candidates = values.split_off(start);
                    let first = candidates
                        .first()
                        .expect("an OID is finished only with locations")
                        .clone();
                    if candidates.iter().skip(1).any(|candidate| {
                        candidate.kind != first.kind || candidate.body != first.body
                    }) {
                        return Err(invalid(PackedObjectDatabaseInvalid::Object {
                            reason: PackedObjectReason::DuplicateConflict,
                            object_oid: oid,
                        }));
                    }
                    active.remove(&oid);
                    self.cache_object(oid, first.clone())?;
                    values.push(first);
                }
                ResolveTask::ResolveLocation {
                    oid,
                    location,
                    depth,
                } => match location {
                    ObjectLocation::Loose => {
                        let bounds = if oid == requested_oid {
                            requested_bounds
                        } else {
                            RequestedObjectBounds::default()
                        };
                        let object = self.read_loose(&oid, bounds)?;
                        values.push(object);
                    }
                    ObjectLocation::Pack { pack, offset } => {
                        let entry = self.packs[pack]
                            .entry_at_offset(offset)
                            .expect("validated index and entry map are one-to-one")
                            .clone();
                        match entry.kind.clone() {
                            PackEntryKind::Base(kind) => {
                                if oid == requested_oid {
                                    requested_bounds.check(kind, entry.declared_size, &oid)?;
                                }
                                let body =
                                    self.packs[pack].inflate_entry(&entry, &mut self.budgets)?;
                                let object = ResolvedObject {
                                    kind,
                                    body: body.into(),
                                };
                                verify_oid(&object, &oid)?;
                                values.push(object);
                            }
                            PackEntryKind::OfsDelta { base_offset } => {
                                let base_oid = self.packs[pack]
                                    .entry_at_offset(base_offset)
                                    .ok_or_else(|| {
                                        invalid(PackedObjectDatabaseInvalid::Delta {
                                            reason: PackedDeltaReason::Base,
                                            pack_id: self.packs[pack].pair.pack_id.clone(),
                                            object_oid: oid.clone(),
                                        })
                                    })?
                                    .oid
                                    .clone();
                                let program =
                                    self.packs[pack].inflate_entry(&entry, &mut self.budgets)?;
                                tasks.push(ResolveTask::ApplyDelta {
                                    oid: oid.clone(),
                                    pack,
                                    entry,
                                    program,
                                    bounds: if oid == requested_oid {
                                        requested_bounds
                                    } else {
                                        RequestedObjectBounds::default()
                                    },
                                });
                                tasks.push(ResolveTask::ResolveOid {
                                    oid: base_oid,
                                    depth: depth.saturating_add(1),
                                    cause: Some(DeltaCause {
                                        pack_id: self.packs[pack].pair.pack_id.clone(),
                                        object_oid: oid,
                                    }),
                                });
                            }
                            PackEntryKind::RefDelta { base_oid } => {
                                let program =
                                    self.packs[pack].inflate_entry(&entry, &mut self.budgets)?;
                                tasks.push(ResolveTask::ApplyDelta {
                                    oid: oid.clone(),
                                    pack,
                                    entry,
                                    program,
                                    bounds: if oid == requested_oid {
                                        requested_bounds
                                    } else {
                                        RequestedObjectBounds::default()
                                    },
                                });
                                tasks.push(ResolveTask::ResolveOid {
                                    oid: base_oid,
                                    depth: depth.saturating_add(1),
                                    cause: Some(DeltaCause {
                                        pack_id: self.packs[pack].pair.pack_id.clone(),
                                        object_oid: oid,
                                    }),
                                });
                            }
                        }
                    }
                },
                ResolveTask::ApplyDelta {
                    oid,
                    pack,
                    entry,
                    program,
                    bounds,
                } => {
                    let base = values.pop().expect("delta base task returns one object");
                    let pack_id = self.packs[pack].pair.pack_id.clone();
                    let body = delta::apply(
                        &base.body,
                        &program,
                        base.kind,
                        &pack_id,
                        &entry.oid,
                        &mut self.budgets,
                        bounds,
                    )?;
                    let object = ResolvedObject {
                        kind: base.kind,
                        body: body.into(),
                    };
                    verify_oid(&object, &oid)?;
                    values.push(object);
                }
            }
        }
        values
            .pop()
            .ok_or_else(|| unavailable(PackedComponent::Object))
    }

    fn cache_object(
        &mut self,
        oid: ObjectId,
        object: ResolvedObject,
    ) -> Result<(), AcquisitionError> {
        if self.cache.contains_key(&oid) {
            return Ok(());
        }
        let bytes = u64::try_from(object.body.len()).unwrap_or(u64::MAX);
        self.cache_bytes = checked_charge(
            LimitKind::ReconstructedObjectCacheBytes,
            self.cache_bytes,
            bytes,
        )?;
        self.cache.insert(oid, object);
        Ok(())
    }

    fn loose_exists(&self, oid: &ObjectId) -> Result<bool, AcquisitionError> {
        let objects = self.git_dir.join("objects");
        let object_directory = objects.join(&oid.as_str()[..2]);
        let object_path = object_directory.join(&oid.as_str()[2..]);
        for (path, directory) in [(&objects, true), (&object_directory, true)] {
            let metadata = match fs::symlink_metadata(path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
                Err(_) => return Err(unavailable(PackedComponent::Object)),
            };
            if metadata.file_type().is_symlink() || directory && !metadata.is_dir() {
                return Err(object_invalid(oid, PackedObjectReason::Oid));
            }
        }
        let metadata = match fs::symlink_metadata(&object_path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(_) => return Err(unavailable(PackedComponent::Object)),
        };
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.len() > SELECTED_LOOSE_COMPRESSED_BYTES
        {
            return Err(object_invalid(oid, PackedObjectReason::Oid));
        }
        Ok(true)
    }

    fn read_loose(
        &self,
        oid: &ObjectId,
        bounds: RequestedObjectBounds,
    ) -> Result<ResolvedObject, AcquisitionError> {
        let path = self
            .git_dir
            .join("objects")
            .join(&oid.as_str()[..2])
            .join(&oid.as_str()[2..]);
        let file = File::open(path).map_err(|_| unavailable(PackedComponent::Object))?;
        let mut decoder = ZlibDecoder::new(file);
        let mut hasher = CollisionHasher::new();
        let mut header = Vec::with_capacity(64);
        loop {
            let mut byte = [0_u8; 1];
            if decoder
                .read(&mut byte)
                .map_err(|_| object_invalid(oid, PackedObjectReason::Oid))?
                == 0
            {
                return Err(object_invalid(oid, PackedObjectReason::Size));
            }
            hasher.update(&byte);
            if byte[0] == 0 {
                break;
            }
            if header.len() == 64 {
                return Err(object_invalid(oid, PackedObjectReason::Size));
            }
            header.push(byte[0]);
        }
        let (kind, body_size) = parse_object_header(&header)
            .ok_or_else(|| object_invalid(oid, PackedObjectReason::Size))?;
        bounds.check(kind, body_size, oid)?;
        let body_size_u64 = u64::try_from(body_size).unwrap_or(u64::MAX);
        if body_size_u64 > LimitKind::DeltaIntermediateBytes.maximum() {
            return Err(limit_exceeded(
                LimitKind::DeltaIntermediateBytes,
                body_size_u64,
            ));
        }
        let mut body = Vec::new();
        body.try_reserve_exact(body_size)
            .map_err(|_| unavailable(PackedComponent::Object))?;
        let mut limited = decoder.take(body_size_u64.saturating_add(1));
        limited
            .read_to_end(&mut body)
            .map_err(|_| object_invalid(oid, PackedObjectReason::Size))?;
        if body.len() != body_size {
            return Err(object_invalid(oid, PackedObjectReason::Size));
        }
        hasher.update(&body);
        let digest = hasher
            .finalize()
            .map_err(|()| object_invalid(oid, PackedObjectReason::Sha1Collision))?;
        if ObjectId::from_bytes(&digest) != *oid {
            return Err(object_invalid(oid, PackedObjectReason::Oid));
        }
        Ok(ResolvedObject {
            kind,
            body: body.into(),
        })
    }
}

fn checked_charge(limit: LimitKind, current: u64, added: u64) -> Result<u64, AcquisitionError> {
    let observed = current.saturating_add(added);
    if observed > limit.maximum() {
        Err(limit_exceeded(limit, observed))
    } else {
        Ok(observed)
    }
}

fn check_delta_depth(depth: u64) -> Result<(), AcquisitionError> {
    if depth > LimitKind::DeltaDepth.maximum() {
        Err(limit_exceeded(LimitKind::DeltaDepth, depth))
    } else {
        Ok(())
    }
}

fn verify_oid(object: &ResolvedObject, oid: &ObjectId) -> Result<(), AcquisitionError> {
    let kind = match object.kind {
        GitObjectKind::Commit => "commit",
        GitObjectKind::Tree => "tree",
        GitObjectKind::Blob => "blob",
        GitObjectKind::Tag => "tag",
    };
    let header = format!("{kind} {}\0", object.body.len());
    verify_object_hash(header.as_bytes(), &object.body, oid)
}

fn verify_object_hash(header: &[u8], body: &[u8], oid: &ObjectId) -> Result<(), AcquisitionError> {
    let mut hasher = CollisionHasher::new();
    hasher.update(header);
    hasher.update(body);
    let digest = hasher
        .finalize()
        .map_err(|()| object_invalid(oid, PackedObjectReason::Sha1Collision))?;
    if ObjectId::from_bytes(&digest) != *oid {
        return Err(object_invalid(oid, PackedObjectReason::Oid));
    }
    Ok(())
}

fn object_invalid(oid: &ObjectId, reason: PackedObjectReason) -> AcquisitionError {
    invalid(PackedObjectDatabaseInvalid::Object {
        reason,
        object_oid: oid.clone(),
    })
}

fn invalid(error: PackedObjectDatabaseInvalid) -> AcquisitionError {
    packed_error(PackedAcquisitionError::Invalid(error))
}

fn invalid_catalog() -> AcquisitionError {
    invalid(PackedObjectDatabaseInvalid::CatalogEntry)
}

fn changed(component: PackedComponent) -> AcquisitionError {
    packed_error(PackedAcquisitionError::Changed(component))
}

fn unavailable(component: PackedComponent) -> AcquisitionError {
    packed_error(PackedAcquisitionError::Unavailable(component))
}

fn packed_error(error: PackedAcquisitionError) -> AcquisitionError {
    AcquisitionError::UnsupportedRepositoryShape {
        feature: UnsupportedFeature::packed_acquisition(error),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write as _;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        PackedObjectDatabase, RequestedObjectBounds, check_delta_depth, object_invalid,
        verify_object_hash,
    };
    use crate::packed::hash::reviewed_collision_vector;
    use crate::{GitObjectKind, ReadObjectError, s1_blob_body_limit};
    use codenoesis_domain::s1_packed::PackedObjectReason;
    use codenoesis_domain::{AcquisitionError, LimitKind, ObjectId, limit_exceeded};
    use flate2::{Compression, write::ZlibEncoder};

    #[test]
    fn pt_fr_acq_004_requested_blob_bounds_report_selected_maximum() {
        let oid =
            ObjectId::parse_sha1("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").expect("blob OID");
        for maximum in [1_024, 4_194_304, 8_388_608] {
            let bounds = RequestedObjectBounds {
                declared_body_limit: Some(s1_blob_body_limit(0, maximum)),
                body_ceiling: None,
            };
            assert_eq!(
                bounds.check(
                    GitObjectKind::Blob,
                    usize::try_from(maximum).expect("selected maximum fits usize"),
                    &oid
                ),
                Ok(())
            );
            if maximum == 8_388_608 {
                assert_eq!(bounds.check(GitObjectKind::Blob, 4_194_305, &oid), Ok(()));
            }
            for observed in [maximum + 1, maximum + 1_000] {
                assert_eq!(
                    bounds.check(
                        GitObjectKind::Blob,
                        usize::try_from(observed).expect("observed size fits usize"),
                        &oid
                    ),
                    Err(AcquisitionError::LimitExceeded {
                        limit: LimitKind::SingleFileBytes,
                        maximum,
                        observed: maximum + 1,
                    })
                );
            }
        }
    }

    #[test]
    fn pt_fr_acq_004_requested_blob_bounds_preserve_cumulative_offset() {
        let oid =
            ObjectId::parse_sha1("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").expect("blob OID");
        let maximum = LimitKind::CumulativeFileBytes.maximum();
        let bounds = RequestedObjectBounds {
            declared_body_limit: Some(s1_blob_body_limit(maximum - 17, 8_388_608)),
            body_ceiling: None,
        };
        assert_eq!(bounds.check(GitObjectKind::Blob, 17, &oid), Ok(()));
        for body_size in [18, 1_000, usize::MAX] {
            assert_eq!(
                bounds.check(GitObjectKind::Blob, body_size, &oid),
                Err(AcquisitionError::LimitExceeded {
                    limit: LimitKind::CumulativeFileBytes,
                    maximum,
                    observed: maximum + 1,
                })
            );
        }
    }

    #[test]
    fn sec_fr_acq_004_selected_loose_blob_bounds_fail_before_body() {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "codenoesis-packed-blob-bounds-{}-{timestamp}",
            std::process::id()
        ));
        let object_dir = root.join("objects/aa");
        fs::create_dir_all(&object_dir).expect("create object fanout");
        let oid =
            ObjectId::parse_sha1("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").expect("blob OID");
        let mut database = PackedObjectDatabase::open(&root).expect("open loose-only catalog");
        for maximum in [1_024, 4_194_304, 8_388_608] {
            let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
            write!(encoder, "blob {}\0", maximum + 1_000).expect("encode oversized header");
            fs::write(
                object_dir.join(&oid.as_str()[2..]),
                encoder.finish().expect("finish oversized header"),
            )
            .expect("write header-only object");
            let result =
                database.read_object(&oid, None, Some(s1_blob_body_limit(0, maximum)), None);
            assert!(matches!(
                result,
                Err(ReadObjectError::Acquisition(AcquisitionError::LimitExceeded {
                    limit: LimitKind::SingleFileBytes,
                    maximum: reported_maximum,
                    observed,
                })) if reported_maximum == maximum && observed == maximum + 1
            ));
        }
        fs::remove_dir_all(root).expect("remove loose object test root");
    }

    #[test]
    fn conf_fr_acq_004_object_sha1_collision_boundary() {
        let object_oid =
            ObjectId::parse_sha1("0000000000000000000000000000000000000000").expect("object ID");
        let error = verify_object_hash(&reviewed_collision_vector(), &[], &object_oid)
            .expect_err("reviewed collision must fail the object boundary");
        assert_eq!(
            error,
            object_invalid(&object_oid, PackedObjectReason::Sha1Collision)
        );
    }

    #[test]
    fn pt_fr_acq_004_delta_depth_maximum_and_plus_one() {
        assert_eq!(check_delta_depth(LimitKind::DeltaDepth.maximum()), Ok(()));
        assert_eq!(
            check_delta_depth(LimitKind::DeltaDepth.maximum() + 1),
            Err(limit_exceeded(
                LimitKind::DeltaDepth,
                LimitKind::DeltaDepth.maximum() + 1,
            ))
        );
    }
}
