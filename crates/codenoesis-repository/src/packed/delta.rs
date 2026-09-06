use codenoesis_domain::s1_packed::{
    PackedDeltaReason, PackedObjectDatabaseInvalid, PackedObjectReason,
};
use codenoesis_domain::{AcquisitionError, LimitKind, ObjectId, limit_exceeded};

use super::{PackBudgets, RequestedObjectBounds, invalid};
use crate::GitObjectKind;

#[allow(clippy::too_many_lines)]
pub(super) fn apply(
    base: &[u8],
    program: &[u8],
    base_kind: GitObjectKind,
    pack_id: &ObjectId,
    object_oid: &ObjectId,
    budgets: &mut PackBudgets,
    bounds: RequestedObjectBounds,
) -> Result<Vec<u8>, AcquisitionError> {
    let mut cursor = 0_usize;
    let base_size = parse_size(program, &mut cursor)
        .ok_or_else(|| delta_invalid(pack_id, object_oid, PackedDeltaReason::Program))?;
    let result_size = parse_size(program, &mut cursor)
        .ok_or_else(|| delta_invalid(pack_id, object_oid, PackedDeltaReason::Program))?;
    if base_size != base.len() {
        return Err(delta_invalid(
            pack_id,
            object_oid,
            PackedDeltaReason::Program,
        ));
    }
    bounds.check(base_kind, result_size, object_oid)?;
    let result_size_u64 = u64::try_from(result_size).unwrap_or(u64::MAX);
    if result_size_u64 > LimitKind::DeltaIntermediateBytes.maximum() {
        return Err(limit_exceeded(
            LimitKind::DeltaIntermediateBytes,
            result_size_u64,
        ));
    }
    budgets.charge_delta_work(result_size_u64)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(result_size)
        .map_err(|_| super::unavailable(codenoesis_domain::s1_packed::PackedComponent::Delta))?;
    let mut instructions = 0_u64;
    while cursor < program.len() {
        charge_instruction(&mut instructions)?;
        let opcode = program[cursor];
        cursor += 1;
        if opcode == 0 {
            return Err(delta_invalid(
                pack_id,
                object_oid,
                PackedDeltaReason::Program,
            ));
        }
        if opcode & 0x80 == 0 {
            let length = usize::from(opcode & 0x7f);
            let end = cursor
                .checked_add(length)
                .filter(|end| *end <= program.len())
                .ok_or_else(|| delta_invalid(pack_id, object_oid, PackedDeltaReason::Program))?;
            let result_end = result
                .len()
                .checked_add(length)
                .ok_or_else(|| delta_invalid(pack_id, object_oid, PackedDeltaReason::Program))?;
            if result_end > result_size {
                return Err(delta_invalid(
                    pack_id,
                    object_oid,
                    PackedDeltaReason::Program,
                ));
            }
            result.extend_from_slice(&program[cursor..end]);
            cursor = end;
            continue;
        }

        let mut copy_offset = 0_usize;
        for byte_index in 0..4 {
            if opcode & (1 << byte_index) != 0 {
                let byte = *program.get(cursor).ok_or_else(|| {
                    delta_invalid(pack_id, object_oid, PackedDeltaReason::Program)
                })?;
                cursor += 1;
                copy_offset |= usize::from(byte) << (byte_index * 8);
            }
        }
        let mut copy_size = 0_usize;
        for byte_index in 0..3 {
            if opcode & (1 << (4 + byte_index)) != 0 {
                let byte = *program.get(cursor).ok_or_else(|| {
                    delta_invalid(pack_id, object_oid, PackedDeltaReason::Program)
                })?;
                cursor += 1;
                copy_size |= usize::from(byte) << (byte_index * 8);
            }
        }
        if copy_size == 0 {
            copy_size = 0x1_0000;
        }
        let copy_end = copy_offset
            .checked_add(copy_size)
            .ok_or_else(|| delta_invalid(pack_id, object_oid, PackedDeltaReason::Program))?;
        let source = base
            .get(copy_offset..copy_end)
            .ok_or_else(|| delta_invalid(pack_id, object_oid, PackedDeltaReason::Program))?;
        let result_end = result
            .len()
            .checked_add(copy_size)
            .ok_or_else(|| delta_invalid(pack_id, object_oid, PackedDeltaReason::Program))?;
        if result_end > result_size {
            return Err(delta_invalid(
                pack_id,
                object_oid,
                PackedDeltaReason::Program,
            ));
        }
        result.extend_from_slice(source);
    }
    if result.len() != result_size {
        return Err(invalid(PackedObjectDatabaseInvalid::Object {
            reason: PackedObjectReason::Size,
            object_oid: object_oid.clone(),
        }));
    }
    Ok(result)
}

fn charge_instruction(instructions: &mut u64) -> Result<(), AcquisitionError> {
    *instructions = instructions
        .checked_add(1)
        .ok_or_else(|| limit_exceeded(LimitKind::DeltaInstructions, u64::MAX))?;
    if *instructions > LimitKind::DeltaInstructions.maximum() {
        return Err(limit_exceeded(LimitKind::DeltaInstructions, *instructions));
    }
    Ok(())
}

fn parse_size(program: &[u8], cursor: &mut usize) -> Option<usize> {
    let mut value = 0_u64;
    let mut shift = 0_u32;
    loop {
        let byte = *program.get(*cursor)?;
        *cursor += 1;
        if shift >= 64 {
            return None;
        }
        value |= u64::from(byte & 0x7f).checked_shl(shift)?;
        if byte & 0x80 == 0 {
            return usize::try_from(value).ok();
        }
        shift += 7;
    }
}

fn delta_invalid(
    pack_id: &ObjectId,
    object_oid: &ObjectId,
    reason: PackedDeltaReason,
) -> AcquisitionError {
    invalid(PackedObjectDatabaseInvalid::Delta {
        reason,
        pack_id: pack_id.clone(),
        object_oid: object_oid.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::{apply, charge_instruction, delta_invalid};
    use crate::packed::{PackBudgets, RequestedObjectBounds, invalid};
    use crate::{DeclaredBodyLimit, GitObjectKind};
    use codenoesis_domain::s1_packed::{
        PackedDeltaReason, PackedObjectDatabaseInvalid, PackedObjectReason,
    };
    use codenoesis_domain::{AcquisitionError, LimitKind, ObjectId, limit_exceeded};

    #[test]
    fn sec_fr_acq_004_delta_program_accepts_bounded_copy_and_insert() {
        let result = apply_program(
            b"abcdef",
            &[6, 6, 0x90, 3, 3, b'X', b'Y', b'Z'],
            RequestedObjectBounds::default(),
        )
        .expect("valid bounded delta");
        assert_eq!(result, b"abcXYZ");
    }

    #[test]
    fn sec_fr_acq_004_delta_program_rejects_reviewed_malformed_opcodes() {
        let pack_id = oid("1111111111111111111111111111111111111111");
        let object_oid = oid("2222222222222222222222222222222222222222");
        for (base, program) in [
            (&b""[..], &b"\x00\x00\x00"[..]),
            (&b""[..], &b"\x00\x03\x03a"[..]),
            (&b"a"[..], &b"\x01\x02\x90\x02"[..]),
            (&b""[..], &b"\x80"[..]),
        ] {
            let mut budgets = PackBudgets::default();
            let error = apply(
                base,
                program,
                GitObjectKind::Blob,
                &pack_id,
                &object_oid,
                &mut budgets,
                RequestedObjectBounds::default(),
            )
            .expect_err("malformed delta must fail");
            assert_eq!(
                error,
                delta_invalid(&pack_id, &object_oid, PackedDeltaReason::Program)
            );
        }
    }

    #[test]
    fn sec_fr_acq_004_delta_program_rejects_result_size_mismatch() {
        let object_oid = oid("2222222222222222222222222222222222222222");
        let error = apply_program(b"", &[0, 2, 1, b'a'], RequestedObjectBounds::default())
            .expect_err("delta size mismatch must fail");
        assert_eq!(
            error,
            invalid(PackedObjectDatabaseInvalid::Object {
                reason: PackedObjectReason::Size,
                object_oid,
            })
        );
    }

    #[test]
    fn pt_fr_acq_004_delta_limits_charge_before_allocation_or_instruction() {
        let pack_id = oid("1111111111111111111111111111111111111111");
        let object_oid = oid("2222222222222222222222222222222222222222");
        let oversized_result = encode_size(LimitKind::DeltaIntermediateBytes.maximum() + 1);
        let mut program = vec![0];
        program.extend_from_slice(&oversized_result);
        let mut budgets = PackBudgets::default();
        let error = apply(
            b"",
            &program,
            GitObjectKind::Blob,
            &pack_id,
            &object_oid,
            &mut budgets,
            RequestedObjectBounds::default(),
        )
        .expect_err("oversized delta result must fail before allocation");
        assert_eq!(
            error,
            limit_exceeded(
                LimitKind::DeltaIntermediateBytes,
                LimitKind::DeltaIntermediateBytes.maximum() + 1,
            )
        );

        let mut instructions = LimitKind::DeltaInstructions.maximum();
        assert_eq!(
            charge_instruction(&mut instructions),
            Err(limit_exceeded(
                LimitKind::DeltaInstructions,
                LimitKind::DeltaInstructions.maximum() + 1,
            ))
        );

        let bounds = RequestedObjectBounds {
            declared_body_limit: Some(DeclaredBodyLimit {
                limit: LimitKind::SingleFileBytes,
                body_maximum: 4,
                observed_offset: 0,
            }),
            body_ceiling: Some(4),
        };
        let error = apply_program(b"", &[0, 5, 5, b'a', b'b', b'c', b'd', b'e'], bounds)
            .expect_err("inherited blob maximum must fail before result allocation");
        assert_eq!(
            error,
            AcquisitionError::LimitExceeded {
                limit: LimitKind::SingleFileBytes,
                maximum: 4,
                observed: 5,
            }
        );
    }

    fn apply_program(
        base: &[u8],
        program: &[u8],
        bounds: RequestedObjectBounds,
    ) -> Result<Vec<u8>, codenoesis_domain::AcquisitionError> {
        let mut budgets = PackBudgets::default();
        apply(
            base,
            program,
            GitObjectKind::Blob,
            &oid("1111111111111111111111111111111111111111"),
            &oid("2222222222222222222222222222222222222222"),
            &mut budgets,
            bounds,
        )
    }

    fn encode_size(mut value: u64) -> Vec<u8> {
        let mut bytes = Vec::new();
        loop {
            let mut byte = u8::try_from(value & 0x7f).expect("seven-bit chunk");
            value >>= 7;
            if value != 0 {
                byte |= 0x80;
            }
            bytes.push(byte);
            if value == 0 {
                return bytes;
            }
        }
    }

    fn oid(value: &str) -> ObjectId {
        ObjectId::parse_sha1(value).expect("test object ID")
    }
}
