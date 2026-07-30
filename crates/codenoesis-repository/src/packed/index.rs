use std::collections::{BTreeMap, BTreeSet};

use codenoesis_domain::s1_packed::{
    PackedComponent, PackedIndexObjectReason, PackedIndexReason, PackedObjectDatabaseInvalid,
};
use codenoesis_domain::{AcquisitionError, LimitKind, ObjectId, limit_exceeded};

use super::catalog::TrackedFile;
use super::hash::collision_detecting_sha1;
use super::invalid;

const INDEX_HEADER_BYTES: usize = 8;
const FANOUT_BYTES: usize = 256 * 4;
const TRAILER_BYTES: usize = 40;

#[derive(Clone)]
pub(super) struct IndexRow {
    pub(super) oid: ObjectId,
    pub(super) crc32: u32,
    pub(super) offset: u64,
}

pub(super) struct ParsedIndex {
    pub(super) rows: Vec<IndexRow>,
    pub(super) pack_checksum: [u8; 20],
    offsets: BTreeMap<u64, usize>,
}

impl ParsedIndex {
    pub(super) fn find(&self, oid: &ObjectId) -> Option<&IndexRow> {
        self.rows
            .binary_search_by(|row| row.oid.as_str().cmp(oid.as_str()))
            .ok()
            .map(|index| &self.rows[index])
    }

    pub(super) fn row_at_offset(&self, offset: u64) -> Option<&IndexRow> {
        self.offsets.get(&offset).map(|row| &self.rows[*row])
    }
}

#[allow(clippy::too_many_lines)]
pub(super) fn parse_index(
    file: &mut TrackedFile,
    pack_id: &ObjectId,
    pack_length: u64,
) -> Result<ParsedIndex, AcquisitionError> {
    let bytes = file.read_all(PackedComponent::Index)?;
    if bytes.len() < INDEX_HEADER_BYTES + FANOUT_BYTES + TRAILER_BYTES {
        return Err(index_invalid(pack_id, PackedIndexReason::Layout));
    }
    if bytes[..4] != [0xff, 0x74, 0x4f, 0x63] {
        return Err(AcquisitionError::UnsupportedRepositoryShape {
            feature: codenoesis_domain::UnsupportedFeature::PackIndexVersionUnsupported,
        });
    }
    if read_u32(&bytes, 4) != Some(2) {
        return Err(AcquisitionError::UnsupportedRepositoryShape {
            feature: codenoesis_domain::UnsupportedFeature::PackIndexVersionUnsupported,
        });
    }

    let fanout_start = INDEX_HEADER_BYTES;
    let mut fanout = [0_u32; 256];
    let mut previous = 0_u32;
    for (bucket, value) in fanout.iter_mut().enumerate() {
        *value = read_u32(&bytes, fanout_start + bucket * 4)
            .ok_or_else(|| index_invalid(pack_id, PackedIndexReason::Layout))?;
        if bucket > 0 && *value < previous {
            return Err(index_invalid(pack_id, PackedIndexReason::Fanout));
        }
        previous = *value;
    }
    let object_count = usize::try_from(fanout[255])
        .map_err(|_| index_invalid(pack_id, PackedIndexReason::Layout))?;
    let object_count_u64 = u64::try_from(object_count).unwrap_or(u64::MAX);
    if object_count_u64 > LimitKind::IndexedObjects.maximum() {
        return Err(limit_exceeded(LimitKind::IndexedObjects, object_count_u64));
    }

    let oid_start = INDEX_HEADER_BYTES + FANOUT_BYTES;
    let oid_bytes = object_count
        .checked_mul(20)
        .ok_or_else(|| index_invalid(pack_id, PackedIndexReason::Layout))?;
    let crc_start = oid_start
        .checked_add(oid_bytes)
        .ok_or_else(|| index_invalid(pack_id, PackedIndexReason::Layout))?;
    let table_bytes = object_count
        .checked_mul(4)
        .ok_or_else(|| index_invalid(pack_id, PackedIndexReason::Layout))?;
    let offset_start = crc_start
        .checked_add(table_bytes)
        .ok_or_else(|| index_invalid(pack_id, PackedIndexReason::Layout))?;
    let large_start = offset_start
        .checked_add(table_bytes)
        .ok_or_else(|| index_invalid(pack_id, PackedIndexReason::Layout))?;
    if large_start
        .checked_add(TRAILER_BYTES)
        .is_none_or(|minimum| minimum > bytes.len())
    {
        return Err(index_invalid(pack_id, PackedIndexReason::Layout));
    }

    let mut oids = Vec::with_capacity(object_count);
    let mut bucket_counts = [0_u32; 256];
    for row in 0..object_count {
        let start = oid_start + row * 20;
        let mut oid_bytes = [0_u8; 20];
        oid_bytes.copy_from_slice(&bytes[start..start + 20]);
        if let Some(previous) = oids.last()
            && previous >= &oid_bytes
        {
            return Err(index_object_invalid(
                pack_id,
                &ObjectId::from_bytes(&oid_bytes),
                PackedIndexObjectReason::ObjectOrder,
            ));
        }
        bucket_counts[usize::from(oid_bytes[0])] += 1;
        oids.push(oid_bytes);
    }
    let mut cumulative = 0_u32;
    for bucket in 0..256 {
        cumulative = cumulative
            .checked_add(bucket_counts[bucket])
            .ok_or_else(|| index_invalid(pack_id, PackedIndexReason::Fanout))?;
        if fanout[bucket] != cumulative {
            return Err(index_invalid(pack_id, PackedIndexReason::Fanout));
        }
    }

    let mut offset_words = Vec::with_capacity(object_count);
    let mut large_slots = BTreeSet::new();
    for (row, oid) in oids.iter().enumerate() {
        let value = read_u32(&bytes, offset_start + row * 4)
            .ok_or_else(|| index_invalid(pack_id, PackedIndexReason::Layout))?;
        if value & 0x8000_0000 != 0 {
            let slot = value & 0x7fff_ffff;
            if !large_slots.insert(slot) {
                return Err(index_object_invalid(
                    pack_id,
                    &ObjectId::from_bytes(oid),
                    PackedIndexObjectReason::Offset,
                ));
            }
        }
        offset_words.push(value);
    }
    let large_count = large_slots.len();
    if large_slots
        .iter()
        .copied()
        .enumerate()
        .any(|(expected, observed)| usize::try_from(observed) != Ok(expected))
    {
        let object_oid = oids
            .first()
            .map_or_else(|| pack_id.clone(), ObjectId::from_bytes);
        return Err(index_object_invalid(
            pack_id,
            &object_oid,
            PackedIndexObjectReason::Offset,
        ));
    }
    let large_bytes = large_count
        .checked_mul(8)
        .ok_or_else(|| index_invalid(pack_id, PackedIndexReason::Layout))?;
    let expected_length = large_start
        .checked_add(large_bytes)
        .and_then(|value| value.checked_add(TRAILER_BYTES))
        .ok_or_else(|| index_invalid(pack_id, PackedIndexReason::Layout))?;
    if expected_length != bytes.len() {
        return Err(index_invalid(pack_id, PackedIndexReason::Layout));
    }

    let entry_end = pack_length
        .checked_sub(20)
        .ok_or_else(|| index_invalid(pack_id, PackedIndexReason::Layout))?;
    let mut offsets = BTreeSet::new();
    let mut rows = Vec::with_capacity(object_count);
    for row in 0..object_count {
        let offset = if offset_words[row] & 0x8000_0000 == 0 {
            u64::from(offset_words[row])
        } else {
            let slot = usize::try_from(offset_words[row] & 0x7fff_ffff)
                .map_err(|_| index_invalid(pack_id, PackedIndexReason::Layout))?;
            read_u64(&bytes, large_start + slot * 8)
                .ok_or_else(|| index_invalid(pack_id, PackedIndexReason::Layout))?
        };
        let object_oid = ObjectId::from_bytes(&oids[row]);
        if offset < 12 || offset >= entry_end || !offsets.insert(offset) {
            return Err(index_object_invalid(
                pack_id,
                &object_oid,
                PackedIndexObjectReason::Offset,
            ));
        }
        let crc32 = read_u32(&bytes, crc_start + row * 4)
            .ok_or_else(|| index_invalid(pack_id, PackedIndexReason::Layout))?;
        rows.push(IndexRow {
            oid: object_oid,
            crc32,
            offset,
        });
    }

    let pack_checksum_start = bytes.len() - TRAILER_BYTES;
    let index_checksum_start = bytes.len() - 20;
    let mut pack_checksum = [0_u8; 20];
    pack_checksum.copy_from_slice(&bytes[pack_checksum_start..index_checksum_start]);
    let expected = &bytes[index_checksum_start..];
    validate_index_checksum(&bytes[..index_checksum_start], expected, pack_id)?;

    let offsets = rows
        .iter()
        .enumerate()
        .map(|(row, entry)| (entry.offset, row))
        .collect();
    Ok(ParsedIndex {
        rows,
        pack_checksum,
        offsets,
    })
}

fn validate_index_checksum(
    payload: &[u8],
    expected: &[u8],
    pack_id: &ObjectId,
) -> Result<(), AcquisitionError> {
    let actual = collision_detecting_sha1(payload)
        .map_err(|()| index_invalid(pack_id, PackedIndexReason::Sha1Collision))?;
    if actual != expected {
        return Err(index_invalid(pack_id, PackedIndexReason::Checksum));
    }
    Ok(())
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let bytes: [u8; 4] = bytes.get(offset..offset + 4)?.try_into().ok()?;
    Some(u32::from_be_bytes(bytes))
}

fn read_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    let bytes: [u8; 8] = bytes.get(offset..offset + 8)?.try_into().ok()?;
    Some(u64::from_be_bytes(bytes))
}

fn index_invalid(pack_id: &ObjectId, reason: PackedIndexReason) -> AcquisitionError {
    invalid(PackedObjectDatabaseInvalid::Index {
        reason,
        pack_id: pack_id.clone(),
    })
}

fn index_object_invalid(
    pack_id: &ObjectId,
    object_oid: &ObjectId,
    reason: PackedIndexObjectReason,
) -> AcquisitionError {
    invalid(PackedObjectDatabaseInvalid::IndexObject {
        reason,
        pack_id: pack_id.clone(),
        object_oid: object_oid.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::{index_invalid, validate_index_checksum};
    use crate::packed::hash::reviewed_collision_vector;
    use codenoesis_domain::ObjectId;
    use codenoesis_domain::s1_packed::PackedIndexReason;

    #[test]
    fn conf_fr_acq_004_index_sha1_collision_boundary() {
        let pack_id =
            ObjectId::parse_sha1("0000000000000000000000000000000000000000").expect("pack ID");
        let error = validate_index_checksum(&reviewed_collision_vector(), &[0_u8; 20], &pack_id)
            .expect_err("reviewed collision vector must be rejected");
        assert_eq!(
            error,
            index_invalid(&pack_id, PackedIndexReason::Sha1Collision)
        );
    }
}
