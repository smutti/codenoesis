use std::collections::BTreeMap;

use codenoesis_domain::s1_packed::{
    PackedComponent, PackedDeltaReason, PackedEntryReason, PackedIndexObjectReason,
    PackedObjectDatabaseInvalid, PackedPackReason,
};
use codenoesis_domain::{
    AcquisitionError, LimitKind, ObjectId, UnsupportedFeature, limit_exceeded,
};
use crc32fast::Hasher as Crc32;
use flate2::{Decompress, FlushDecompress, Status};

use super::catalog::PackPair;
use super::hash::CollisionHasher;
use super::index::{IndexRow, ParsedIndex};
use super::{PackBudgets, invalid};
use crate::GitObjectKind;

const PACK_HEADER_BYTES: u64 = 12;
const PACK_TRAILER_BYTES: u64 = 20;
const IO_BUFFER_BYTES: usize = 32 * 1024;

#[derive(Clone)]
pub(super) enum PackEntryKind {
    Base(GitObjectKind),
    OfsDelta { base_offset: u64 },
    RefDelta { base_oid: ObjectId },
}

#[derive(Clone)]
pub(super) struct PackEntry {
    pub(super) oid: ObjectId,
    pub(super) offset: u64,
    pub(super) data_offset: u64,
    pub(super) declared_size: usize,
    pub(super) kind: PackEntryKind,
}

pub(super) struct ValidatedPack {
    pub(super) pair: PackPair,
    pub(super) index: ParsedIndex,
    pub(super) entries: Vec<PackEntry>,
    entries_by_offset: BTreeMap<u64, usize>,
}

impl ValidatedPack {
    pub(super) fn open(
        mut pair: PackPair,
        index: ParsedIndex,
        budgets: &mut PackBudgets,
    ) -> Result<Self, AcquisitionError> {
        let pack_id = pair.pack_id.clone();
        let pack_length = pair.pack.len();
        if pack_length < PACK_HEADER_BYTES + PACK_TRAILER_BYTES {
            return Err(pack_invalid(&pack_id, PackedPackReason::Header));
        }
        let mut header = [0_u8; 12];
        pair.pack
            .read_exact_at(0, &mut header, PackedComponent::Pack)?;
        if header[..4] != *b"PACK" {
            return Err(pack_invalid(&pack_id, PackedPackReason::Header));
        }
        let version = u32::from_be_bytes(header[4..8].try_into().expect("fixed header"));
        if version != 2 {
            return Err(AcquisitionError::UnsupportedRepositoryShape {
                feature: UnsupportedFeature::PackVersionUnsupported,
            });
        }
        let declared_count = usize::try_from(u32::from_be_bytes(
            header[8..12].try_into().expect("fixed header"),
        ))
        .map_err(|_| pack_invalid(&pack_id, PackedPackReason::ObjectCount))?;
        if declared_count != index.rows.len() {
            return Err(pack_invalid(&pack_id, PackedPackReason::ObjectCount));
        }

        let trailer_offset = pack_length - PACK_TRAILER_BYTES;
        let mut trailer = [0_u8; 20];
        pair.pack
            .read_exact_at(trailer_offset, &mut trailer, PackedComponent::Pack)?;
        let calculated = hash_prefix(&mut pair, trailer_offset)?;
        if calculated != trailer {
            return Err(pack_invalid(&pack_id, PackedPackReason::Checksum));
        }
        if oid_bytes(&pack_id) != Some(trailer) {
            return Err(pack_invalid(&pack_id, PackedPackReason::Checksum));
        }
        if index.pack_checksum != trailer {
            return Err(pack_invalid(&pack_id, PackedPackReason::IndexMismatch));
        }

        let entries = build_entry_map(&mut pair, &index, declared_count, trailer_offset, budgets)?;
        let entries_by_offset = entries
            .iter()
            .enumerate()
            .map(|(entry, value)| (value.offset, entry))
            .collect();
        Ok(Self {
            pair,
            index,
            entries,
            entries_by_offset,
        })
    }

    pub(super) fn entry_at_offset(&self, offset: u64) -> Option<&PackEntry> {
        self.entries_by_offset
            .get(&offset)
            .map(|entry| &self.entries[*entry])
    }

    pub(super) fn entry_for_oid(&self, oid: &ObjectId) -> Option<&PackEntry> {
        let row = self.index.find(oid)?;
        self.entry_at_offset(row.offset)
    }

    pub(super) fn inflate_entry(
        &mut self,
        entry: &PackEntry,
        budgets: &mut PackBudgets,
    ) -> Result<Vec<u8>, AcquisitionError> {
        let declared = u64::try_from(entry.declared_size).unwrap_or(u64::MAX);
        let limit = if matches!(
            &entry.kind,
            PackEntryKind::OfsDelta { .. } | PackEntryKind::RefDelta { .. }
        ) {
            LimitKind::DeltaProgramBytes
        } else {
            LimitKind::InflatedEntryBytes
        };
        if declared > limit.maximum() {
            return Err(limit_exceeded(limit, declared));
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(entry.declared_size)
            .map_err(|_| super::unavailable(PackedComponent::Entry))?;
        let trailer_offset = self.pair.pack.len() - PACK_TRAILER_BYTES;
        let pack_id = self.pair.pack_id.clone();
        inflate_stream(
            &mut self.pair,
            entry.data_offset,
            trailer_offset,
            entry.declared_size,
            Some(&mut bytes),
            budgets,
            &pack_id,
            &entry.oid,
        )?;
        Ok(bytes)
    }
}

fn build_entry_map(
    pair: &mut PackPair,
    index: &ParsedIndex,
    object_count: usize,
    trailer_offset: u64,
    budgets: &mut PackBudgets,
) -> Result<Vec<PackEntry>, AcquisitionError> {
    let mut entries = Vec::with_capacity(object_count);
    let mut offset = PACK_HEADER_BYTES;
    for _ in 0..object_count {
        let row = index.row_at_offset(offset).ok_or_else(|| {
            let row = first_offset_mismatch(index, offset);
            index_offset_invalid(&pair.pack_id, &row.oid)
        })?;
        let parsed = parse_entry_header(pair, row, offset, trailer_offset)?;
        let pack_id = pair.pack_id.clone();
        let end = inflate_stream(
            pair,
            parsed.data_offset,
            trailer_offset,
            parsed.declared_size,
            None,
            budgets,
            &pack_id,
            &row.oid,
        )?;
        let compressed = end
            .checked_sub(parsed.data_offset)
            .ok_or_else(|| entry_invalid(pair, row, PackedEntryReason::ZlibStream))?;
        if compressed > LimitKind::CompressedEntryBytes.maximum() {
            return Err(limit_exceeded(LimitKind::CompressedEntryBytes, compressed));
        }
        let crc = crc32_range(pair, offset, end)?;
        if crc != row.crc32 {
            return Err(entry_invalid(pair, row, PackedEntryReason::Crc));
        }
        entries.push(PackEntry {
            oid: row.oid.clone(),
            offset,
            data_offset: parsed.data_offset,
            declared_size: parsed.declared_size,
            kind: parsed.kind,
        });
        offset = end;
    }
    if offset != trailer_offset || entries.len() != index.rows.len() {
        return Err(pack_invalid(&pair.pack_id, PackedPackReason::ObjectCount));
    }
    if index
        .rows
        .iter()
        .any(|row| entries.iter().all(|entry| entry.offset != row.offset))
    {
        let row = first_offset_mismatch(index, offset);
        return Err(index_offset_invalid(&pair.pack_id, &row.oid));
    }
    Ok(entries)
}

struct ParsedEntryHeader {
    data_offset: u64,
    declared_size: usize,
    kind: PackEntryKind,
}

fn parse_entry_header(
    pair: &mut PackPair,
    row: &IndexRow,
    entry_offset: u64,
    trailer_offset: u64,
) -> Result<ParsedEntryHeader, AcquisitionError> {
    let mut cursor = entry_offset;
    let first = read_byte(pair, &mut cursor, trailer_offset, row)?;
    let kind_code = (first >> 4) & 0x07;
    let mut declared_size = u64::from(first & 0x0f);
    let mut shift = 4_u32;
    let mut continuation = first & 0x80 != 0;
    while continuation {
        let byte = read_byte(pair, &mut cursor, trailer_offset, row)?;
        if shift >= 64 {
            return Err(entry_invalid(pair, row, PackedEntryReason::Header));
        }
        declared_size |= u64::from(byte & 0x7f)
            .checked_shl(shift)
            .ok_or_else(|| entry_invalid(pair, row, PackedEntryReason::Header))?;
        shift += 7;
        continuation = byte & 0x80 != 0;
    }
    let declared_size = usize::try_from(declared_size)
        .map_err(|_| entry_invalid(pair, row, PackedEntryReason::Header))?;
    let kind = match kind_code {
        1 => PackEntryKind::Base(GitObjectKind::Commit),
        2 => PackEntryKind::Base(GitObjectKind::Tree),
        3 => PackEntryKind::Base(GitObjectKind::Blob),
        4 => PackEntryKind::Base(GitObjectKind::Tag),
        6 => {
            let base_offset = parse_ofs_base(pair, row, entry_offset, &mut cursor, trailer_offset)?;
            PackEntryKind::OfsDelta { base_offset }
        }
        7 => {
            let mut bytes = [0_u8; 20];
            if cursor
                .checked_add(20)
                .is_none_or(|end| end > trailer_offset)
            {
                return Err(entry_invalid(pair, row, PackedEntryReason::Header));
            }
            pair.pack
                .read_exact_at(cursor, &mut bytes, PackedComponent::Entry)?;
            cursor += 20;
            PackEntryKind::RefDelta {
                base_oid: ObjectId::from_bytes(&bytes),
            }
        }
        _ => return Err(entry_invalid(pair, row, PackedEntryReason::Header)),
    };
    let declared = u64::try_from(declared_size).unwrap_or(u64::MAX);
    let limit = if matches!(
        kind,
        PackEntryKind::OfsDelta { .. } | PackEntryKind::RefDelta { .. }
    ) {
        LimitKind::DeltaProgramBytes
    } else {
        LimitKind::InflatedEntryBytes
    };
    if declared > limit.maximum() {
        return Err(limit_exceeded(limit, declared));
    }
    Ok(ParsedEntryHeader {
        data_offset: cursor,
        declared_size,
        kind,
    })
}

fn parse_ofs_base(
    pair: &mut PackPair,
    row: &IndexRow,
    entry_offset: u64,
    cursor: &mut u64,
    trailer_offset: u64,
) -> Result<u64, AcquisitionError> {
    let first = read_byte(pair, cursor, trailer_offset, row)?;
    let mut distance = u64::from(first & 0x7f);
    let mut byte = first;
    while byte & 0x80 != 0 {
        byte = read_byte(pair, cursor, trailer_offset, row)?;
        distance = distance
            .checked_add(1)
            .and_then(|value| value.checked_shl(7))
            .and_then(|value| value.checked_add(u64::from(byte & 0x7f)))
            .ok_or_else(|| delta_base_invalid(pair, row))?;
    }
    if distance == 0 {
        return Err(delta_base_invalid(pair, row));
    }
    let base_offset = entry_offset
        .checked_sub(distance)
        .ok_or_else(|| delta_base_invalid(pair, row))?;
    if base_offset < PACK_HEADER_BYTES || base_offset >= entry_offset {
        return Err(delta_base_invalid(pair, row));
    }
    Ok(base_offset)
}

#[allow(clippy::too_many_arguments)]
fn inflate_stream(
    pair: &mut PackPair,
    data_offset: u64,
    trailer_offset: u64,
    declared_size: usize,
    mut retained: Option<&mut Vec<u8>>,
    budgets: &mut PackBudgets,
    pack_id: &ObjectId,
    object_oid: &ObjectId,
) -> Result<u64, AcquisitionError> {
    let declared = u64::try_from(declared_size).unwrap_or(u64::MAX);
    budgets.charge_inflate(declared)?;
    let mut decompressor = Decompress::new(true);
    let mut input_offset = data_offset;
    let mut compressed = 0_u64;
    let mut inflated = 0_u64;
    let mut input = vec![0_u8; IO_BUFFER_BYTES].into_boxed_slice();
    let mut output = vec![0_u8; IO_BUFFER_BYTES].into_boxed_slice();
    loop {
        if input_offset >= trailer_offset {
            return Err(entry_error(
                pack_id,
                object_oid,
                PackedEntryReason::ZlibStream,
            ));
        }
        let remaining = trailer_offset - input_offset;
        let input_length = usize::try_from(remaining.min(IO_BUFFER_BYTES as u64))
            .map_err(|_| entry_error(pack_id, object_oid, PackedEntryReason::ZlibStream))?;
        pair.pack.read_exact_at(
            input_offset,
            &mut input[..input_length],
            PackedComponent::Entry,
        )?;
        let before_input = decompressor.total_in();
        let before_output = decompressor.total_out();
        let status = decompressor
            .decompress(&input[..input_length], &mut output, FlushDecompress::None)
            .map_err(|_| entry_error(pack_id, object_oid, PackedEntryReason::ZlibStream))?;
        let consumed = decompressor.total_in() - before_input;
        let produced = decompressor.total_out() - before_output;
        if consumed == 0 && produced == 0 {
            return Err(entry_error(
                pack_id,
                object_oid,
                PackedEntryReason::ZlibStream,
            ));
        }
        compressed = compressed
            .checked_add(consumed)
            .ok_or_else(|| limit_exceeded(LimitKind::CompressedEntryBytes, u64::MAX))?;
        if compressed > LimitKind::CompressedEntryBytes.maximum() {
            return Err(limit_exceeded(LimitKind::CompressedEntryBytes, compressed));
        }
        inflated = inflated
            .checked_add(produced)
            .ok_or_else(|| limit_exceeded(LimitKind::InflatedEntryBytes, u64::MAX))?;
        if inflated > LimitKind::InflatedEntryBytes.maximum() {
            return Err(limit_exceeded(LimitKind::InflatedEntryBytes, inflated));
        }
        input_offset = input_offset
            .checked_add(consumed)
            .ok_or_else(|| entry_error(pack_id, object_oid, PackedEntryReason::ZlibStream))?;
        if let Some(bytes) = retained.as_deref_mut() {
            let produced = usize::try_from(produced)
                .map_err(|_| entry_error(pack_id, object_oid, PackedEntryReason::ZlibStream))?;
            bytes.extend_from_slice(&output[..produced]);
        }
        if status == Status::StreamEnd {
            break;
        }
    }
    if inflated != declared {
        return Err(entry_error(
            pack_id,
            object_oid,
            PackedEntryReason::ZlibStream,
        ));
    }
    Ok(input_offset)
}

fn hash_prefix(pair: &mut PackPair, length: u64) -> Result<[u8; 20], AcquisitionError> {
    let mut hasher = CollisionHasher::new();
    let mut offset = 0_u64;
    let mut buffer = vec![0_u8; IO_BUFFER_BYTES].into_boxed_slice();
    while offset < length {
        let count = usize::try_from((length - offset).min(IO_BUFFER_BYTES as u64))
            .map_err(|_| super::unavailable(PackedComponent::Pack))?;
        pair.pack
            .read_exact_at(offset, &mut buffer[..count], PackedComponent::Pack)?;
        hasher.update(&buffer[..count]);
        offset += u64::try_from(count).expect("bounded buffer length");
    }
    finish_pack_hash(hasher, &pair.pack_id)
}

fn finish_pack_hash(
    hasher: CollisionHasher,
    pack_id: &ObjectId,
) -> Result<[u8; 20], AcquisitionError> {
    hasher
        .finalize()
        .map_err(|()| pack_invalid(pack_id, PackedPackReason::Sha1Collision))
}

fn crc32_range(pair: &mut PackPair, start: u64, end: u64) -> Result<u32, AcquisitionError> {
    let mut hasher = Crc32::new();
    let mut offset = start;
    let mut buffer = vec![0_u8; IO_BUFFER_BYTES].into_boxed_slice();
    while offset < end {
        let count = usize::try_from((end - offset).min(IO_BUFFER_BYTES as u64))
            .map_err(|_| super::unavailable(PackedComponent::Entry))?;
        pair.pack
            .read_exact_at(offset, &mut buffer[..count], PackedComponent::Entry)?;
        hasher.update(&buffer[..count]);
        offset += u64::try_from(count).expect("bounded buffer length");
    }
    Ok(hasher.finalize())
}

fn read_byte(
    pair: &mut PackPair,
    cursor: &mut u64,
    trailer_offset: u64,
    row: &IndexRow,
) -> Result<u8, AcquisitionError> {
    if *cursor >= trailer_offset {
        return Err(entry_invalid(pair, row, PackedEntryReason::Header));
    }
    let mut byte = [0_u8; 1];
    pair.pack
        .read_exact_at(*cursor, &mut byte, PackedComponent::Entry)?;
    *cursor += 1;
    Ok(byte[0])
}

fn first_offset_mismatch(index: &ParsedIndex, offset: u64) -> &IndexRow {
    index
        .rows
        .iter()
        .min_by_key(|row| row.offset.abs_diff(offset))
        .expect("pack count and index count are equal and nonzero")
}

fn oid_bytes(oid: &ObjectId) -> Option<[u8; 20]> {
    let mut bytes = [0_u8; 20];
    for (index, pair) in oid.as_str().as_bytes().chunks_exact(2).enumerate() {
        let high = decode_hex(pair[0])?;
        let low = decode_hex(pair[1])?;
        bytes[index] = high << 4 | low;
    }
    Some(bytes)
}

fn decode_hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn pack_invalid(pack_id: &ObjectId, reason: PackedPackReason) -> AcquisitionError {
    invalid(PackedObjectDatabaseInvalid::Pack {
        reason,
        pack_id: pack_id.clone(),
    })
}

fn index_offset_invalid(pack_id: &ObjectId, object_oid: &ObjectId) -> AcquisitionError {
    invalid(PackedObjectDatabaseInvalid::IndexObject {
        reason: PackedIndexObjectReason::Offset,
        pack_id: pack_id.clone(),
        object_oid: object_oid.clone(),
    })
}

fn entry_invalid(pair: &PackPair, row: &IndexRow, reason: PackedEntryReason) -> AcquisitionError {
    entry_error(&pair.pack_id, &row.oid, reason)
}

fn entry_error(
    pack_id: &ObjectId,
    object_oid: &ObjectId,
    reason: PackedEntryReason,
) -> AcquisitionError {
    invalid(PackedObjectDatabaseInvalid::Entry {
        reason,
        pack_id: pack_id.clone(),
        object_oid: object_oid.clone(),
    })
}

fn delta_base_invalid(pair: &PackPair, row: &IndexRow) -> AcquisitionError {
    invalid(PackedObjectDatabaseInvalid::Delta {
        reason: PackedDeltaReason::Base,
        pack_id: pair.pack_id.clone(),
        object_oid: row.oid.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::{finish_pack_hash, pack_invalid};
    use crate::packed::hash::{CollisionHasher, reviewed_collision_vector};
    use codenoesis_domain::ObjectId;
    use codenoesis_domain::s1_packed::PackedPackReason;

    #[test]
    fn sec_fr_acq_004_pack_sha1_collision_boundary() {
        let pack_id =
            ObjectId::parse_sha1("0000000000000000000000000000000000000000").expect("pack ID");
        let mut hasher = CollisionHasher::new();
        hasher.update(&reviewed_collision_vector());
        let error =
            finish_pack_hash(hasher, &pack_id).expect_err("reviewed collision must fail the pack");
        assert_eq!(
            error,
            pack_invalid(&pack_id, PackedPackReason::Sha1Collision)
        );
    }
}
