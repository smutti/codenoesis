use std::collections::BTreeSet;

use codenoesis_domain::s4_r7::{
    CompilerIndexError, CompilerIndexLimit, MAX_R7_DOCUMENTS, MAX_R7_OCCURRENCES_PER_DOCUMENT,
    MAX_R7_OCCURRENCES_TOTAL, MAX_R7_PROTOBUF_RECURSION, MAX_R7_RELATIONSHIPS_TOTAL,
    MAX_R7_SYMBOL_INFORMATION_TOTAL, MAX_R7_SYMBOL_OR_DISPLAY_BYTES, MAX_R7_TOOL_ARGUMENT_BYTES,
    MAX_R7_TOOL_ARGUMENTS, MAX_R7_UNPROMOTED_VALUE_BYTES, compiler_index_limit_exceeded,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MessageKind {
    Index,
    Metadata,
    ToolInfo,
    Document,
    SymbolInformation,
    Relationship,
    Signature,
    Occurrence,
    Diagnostic,
    SingleLineRange,
    MultiLineRange,
}

#[derive(Clone, Copy)]
enum FieldValue {
    Varint,
    PackedVarint,
    String(StringClass),
    Message(MessageKind),
}

#[derive(Clone, Copy)]
enum StringClass {
    Symbol,
    ToolArgument,
    Unpromoted,
    Ordinary,
}

#[derive(Clone, Copy)]
struct FieldRule {
    value: FieldValue,
    repeated: bool,
    oneof: Option<u8>,
}

#[derive(Default)]
struct Counts {
    documents: u64,
    occurrences: u64,
    symbols: u64,
    relationships: u64,
    tool_arguments: u64,
}

pub(crate) fn preflight(bytes: &[u8], artifact_sha256: &str) -> Result<(), CompilerIndexError> {
    validate_group_recursion(bytes, artifact_sha256)?;
    let mut counts = Counts::default();
    scan_message(bytes, MessageKind::Index, 1, &mut counts, artifact_sha256)?;
    Ok(())
}

fn validate_group_recursion(bytes: &[u8], artifact_sha256: &str) -> Result<(), CompilerIndexError> {
    let mut cursor = 0;
    scan_wire_stream(bytes, &mut cursor, None, 0, artifact_sha256)
}

fn scan_wire_stream(
    bytes: &[u8],
    cursor: &mut usize,
    end_group: Option<u32>,
    depth: u64,
    artifact_sha256: &str,
) -> Result<(), CompilerIndexError> {
    if depth > MAX_R7_PROTOBUF_RECURSION {
        return Err(compiler_index_limit_exceeded(
            CompilerIndexLimit::ProtobufRecursion,
            depth,
        ));
    }
    while *cursor < bytes.len() {
        let tag = read_varint(bytes, cursor, artifact_sha256)?;
        let field_number = u32::try_from(tag >> 3).unwrap_or(u32::MAX);
        let wire_type = u8::try_from(tag & 0x07).unwrap_or(u8::MAX);
        if field_number == 0 || matches!(wire_type, 6 | 7) {
            return malformed(artifact_sha256, "illegal_wire_tag");
        }
        match wire_type {
            0 => {
                read_varint(bytes, cursor, artifact_sha256)?;
            }
            1 => skip_fixed(bytes, cursor, 8, artifact_sha256)?,
            2 => {
                read_length(bytes, cursor, artifact_sha256)?;
            }
            3 => scan_wire_stream(
                bytes,
                cursor,
                Some(field_number),
                depth.saturating_add(1),
                artifact_sha256,
            )?,
            4 if end_group == Some(field_number) => return Ok(()),
            4 => return malformed(artifact_sha256, "unexpected_end_group"),
            5 => skip_fixed(bytes, cursor, 4, artifact_sha256)?,
            _ => return malformed(artifact_sha256, "illegal_wire_tag"),
        }
    }
    if end_group.is_some() {
        malformed(artifact_sha256, "unterminated_group")
    } else {
        Ok(())
    }
}

fn skip_fixed(
    bytes: &[u8],
    cursor: &mut usize,
    width: usize,
    artifact_sha256: &str,
) -> Result<(), CompilerIndexError> {
    let end = cursor
        .checked_add(width)
        .ok_or_else(|| malformed_error(artifact_sha256, "length_overflow"))?;
    bytes
        .get(*cursor..end)
        .ok_or_else(|| malformed_error(artifact_sha256, "truncated_fixed_width"))?;
    *cursor = end;
    Ok(())
}

fn scan_message(
    bytes: &[u8],
    kind: MessageKind,
    depth: u64,
    counts: &mut Counts,
    artifact_sha256: &str,
) -> Result<(), CompilerIndexError> {
    if depth > MAX_R7_PROTOBUF_RECURSION {
        return Err(compiler_index_limit_exceeded(
            CompilerIndexLimit::ProtobufRecursion,
            depth,
        ));
    }
    let mut cursor = 0;
    let mut singular = BTreeSet::new();
    let mut oneofs = BTreeSet::new();
    let mut first_field = None;
    let mut metadata_count = 0_u64;
    let occurrence_start = counts.occurrences;
    while cursor < bytes.len() {
        let tag = read_varint(bytes, &mut cursor, artifact_sha256)?;
        let field_number = u32::try_from(tag >> 3).unwrap_or(u32::MAX);
        let wire_type = u8::try_from(tag & 0x07).unwrap_or(u8::MAX);
        if field_number == 0 || matches!(wire_type, 3 | 4 | 6 | 7) {
            return malformed(artifact_sha256, "illegal_wire_tag");
        }
        if kind == MessageKind::Index && field_number == 1 && first_field.is_some() {
            return Err(CompilerIndexError::NoncanonicalArtifact {
                artifact_sha256: artifact_sha256.to_owned(),
                reason: "metadata_not_first_or_duplicate".to_owned(),
            });
        }
        first_field.get_or_insert(field_number);
        let Some(rule) = rule(kind, field_number, wire_type) else {
            return malformed(artifact_sha256, "unknown_or_illegal_field");
        };
        if !rule.repeated && !singular.insert(field_number) {
            return malformed(artifact_sha256, "duplicate_singular_field");
        }
        if let Some(oneof) = rule.oneof
            && !oneofs.insert(oneof)
        {
            return malformed(artifact_sha256, "duplicate_oneof_field");
        }
        if kind == MessageKind::Index && field_number == 1 {
            metadata_count += 1;
        }
        count_field(kind, field_number, counts)?;
        match rule.value {
            FieldValue::Varint => {
                read_varint(bytes, &mut cursor, artifact_sha256)?;
            }
            FieldValue::PackedVarint if wire_type == 0 => {
                read_varint(bytes, &mut cursor, artifact_sha256)?;
            }
            FieldValue::PackedVarint => {
                let value = read_length(bytes, &mut cursor, artifact_sha256)?;
                let mut packed = 0;
                while packed < value.len() {
                    read_varint(value, &mut packed, artifact_sha256)?;
                }
            }
            FieldValue::String(class) => {
                let value = read_length(bytes, &mut cursor, artifact_sha256)?;
                validate_string_length(class, value.len())?;
                std::str::from_utf8(value)
                    .map_err(|_| malformed_error(artifact_sha256, "invalid_utf8"))?;
            }
            FieldValue::Message(child) => {
                let value = read_length(bytes, &mut cursor, artifact_sha256)?;
                scan_message(value, child, depth + 1, counts, artifact_sha256)?;
            }
        }
    }
    if kind == MessageKind::Index && metadata_count == 0 {
        return malformed(artifact_sha256, "metadata_missing");
    }
    if kind == MessageKind::Index && first_field != Some(1) {
        return Err(CompilerIndexError::NoncanonicalArtifact {
            artifact_sha256: artifact_sha256.to_owned(),
            reason: "metadata_not_first_or_duplicate".to_owned(),
        });
    }
    if kind == MessageKind::Document {
        let per_document = counts.occurrences.saturating_sub(occurrence_start);
        if per_document > MAX_R7_OCCURRENCES_PER_DOCUMENT {
            return Err(compiler_index_limit_exceeded(
                CompilerIndexLimit::OccurrencesPerDocument,
                per_document,
            ));
        }
    }
    Ok(())
}

fn count_field(
    kind: MessageKind,
    field_number: u32,
    counts: &mut Counts,
) -> Result<(), CompilerIndexError> {
    match (kind, field_number) {
        (MessageKind::Index, 2) => increment(
            &mut counts.documents,
            MAX_R7_DOCUMENTS,
            CompilerIndexLimit::Documents,
        )?,
        (MessageKind::Document | MessageKind::Signature, 2) => increment(
            &mut counts.occurrences,
            MAX_R7_OCCURRENCES_TOTAL,
            CompilerIndexLimit::OccurrencesTotal,
        )?,
        (MessageKind::Document | MessageKind::Index, 3) => increment(
            &mut counts.symbols,
            MAX_R7_SYMBOL_INFORMATION_TOTAL,
            CompilerIndexLimit::SymbolInformationTotal,
        )?,
        (MessageKind::SymbolInformation, 4) => increment(
            &mut counts.relationships,
            MAX_R7_RELATIONSHIPS_TOTAL,
            CompilerIndexLimit::RelationshipsTotal,
        )?,
        (MessageKind::ToolInfo, 3) => increment(
            &mut counts.tool_arguments,
            MAX_R7_TOOL_ARGUMENTS,
            CompilerIndexLimit::ToolArguments,
        )?,
        _ => {}
    }
    Ok(())
}

fn increment(
    value: &mut u64,
    maximum: u64,
    limit: CompilerIndexLimit,
) -> Result<(), CompilerIndexError> {
    *value = value.saturating_add(1);
    if *value > maximum {
        Err(compiler_index_limit_exceeded(limit, *value))
    } else {
        Ok(())
    }
}

fn validate_string_length(class: StringClass, length: usize) -> Result<(), CompilerIndexError> {
    let observed = u64::try_from(length).unwrap_or(u64::MAX);
    let (maximum, limit) = match class {
        StringClass::Symbol => (
            MAX_R7_SYMBOL_OR_DISPLAY_BYTES,
            CompilerIndexLimit::SymbolOrDisplayBytes,
        ),
        StringClass::ToolArgument => (
            MAX_R7_TOOL_ARGUMENT_BYTES,
            CompilerIndexLimit::ToolArgumentBytes,
        ),
        StringClass::Unpromoted => (
            MAX_R7_UNPROMOTED_VALUE_BYTES,
            CompilerIndexLimit::UnpromotedValueBytes,
        ),
        StringClass::Ordinary => return Ok(()),
    };
    if observed > maximum {
        Err(compiler_index_limit_exceeded(limit, observed))
    } else {
        Ok(())
    }
}

#[allow(clippy::match_same_arms)]
fn rule(kind: MessageKind, field: u32, wire: u8) -> Option<FieldRule> {
    use FieldValue::{Message, PackedVarint, String, Varint};
    use MessageKind::{
        Diagnostic, Document, Metadata, MultiLineRange, Occurrence, Relationship, Signature,
        SingleLineRange, SymbolInformation, ToolInfo,
    };
    use StringClass::{Ordinary, Symbol, ToolArgument, Unpromoted};
    let candidate = match (kind, field) {
        (MessageKind::Index, 1) => field_rule(Message(Metadata), false, None),
        (MessageKind::Index, 2) => field_rule(Message(Document), true, None),
        (MessageKind::Index, 3) => field_rule(Message(SymbolInformation), true, None),
        (Metadata, 1 | 4) => field_rule(Varint, false, None),
        (Metadata, 2) => field_rule(Message(ToolInfo), false, None),
        (Metadata, 3) => field_rule(String(Unpromoted), false, None),
        (ToolInfo, 1 | 2) => field_rule(String(Ordinary), false, None),
        (ToolInfo, 3) => field_rule(String(ToolArgument), true, None),
        (Document, 1 | 4) => field_rule(String(Ordinary), false, None),
        (Document, 2) => field_rule(Message(Occurrence), true, None),
        (Document, 3) => field_rule(Message(SymbolInformation), true, None),
        (Document, 5) => field_rule(String(Unpromoted), false, None),
        (Document, 6) => field_rule(Varint, false, None),
        (SymbolInformation, 1 | 6 | 8) => field_rule(String(Symbol), false, None),
        (SymbolInformation, 3) => field_rule(String(Unpromoted), true, None),
        (SymbolInformation, 4) => field_rule(Message(Relationship), true, None),
        (SymbolInformation, 5) => field_rule(Varint, false, None),
        (SymbolInformation, 7) => field_rule(Message(Signature), false, None),
        (Relationship, 1) => field_rule(String(Symbol), false, None),
        (Relationship, 2..=5) => field_rule(Varint, false, None),
        (Signature, 2) => field_rule(Message(Occurrence), true, None),
        (Signature, 4) => field_rule(String(Ordinary), false, None),
        (Signature, 5) => field_rule(String(Unpromoted), false, None),
        (Occurrence, 1 | 7) => field_rule(PackedVarint, true, None),
        (Occurrence, 2) => field_rule(String(Symbol), false, None),
        (Occurrence, 3 | 5) => field_rule(Varint, false, None),
        (Occurrence, 4) => field_rule(String(Unpromoted), true, None),
        (Occurrence, 6) => field_rule(Message(Diagnostic), true, None),
        (Occurrence, 8) => field_rule(Message(SingleLineRange), false, Some(1)),
        (Occurrence, 9) => field_rule(Message(MultiLineRange), false, Some(1)),
        (Occurrence, 10) => field_rule(Message(SingleLineRange), false, Some(2)),
        (Occurrence, 11) => field_rule(Message(MultiLineRange), false, Some(2)),
        (Diagnostic, 1) => field_rule(Varint, false, None),
        (Diagnostic, 2..=4) => field_rule(String(Unpromoted), false, None),
        (Diagnostic, 5) => field_rule(PackedVarint, true, None),
        (SingleLineRange, 1..=3) | (MultiLineRange, 1..=4) => field_rule(Varint, false, None),
        _ => return None,
    };
    match candidate.value {
        Varint if wire == 0 => Some(candidate),
        PackedVarint if matches!(wire, 0 | 2) => Some(candidate),
        String(_) | Message(_) if wire == 2 => Some(candidate),
        _ => None,
    }
}

const fn field_rule(value: FieldValue, repeated: bool, oneof: Option<u8>) -> FieldRule {
    FieldRule {
        value,
        repeated,
        oneof,
    }
}

fn read_length<'a>(
    bytes: &'a [u8],
    cursor: &mut usize,
    artifact_sha256: &str,
) -> Result<&'a [u8], CompilerIndexError> {
    let length = read_varint(bytes, cursor, artifact_sha256)?;
    let length =
        usize::try_from(length).map_err(|_| malformed_error(artifact_sha256, "length_overflow"))?;
    let end = cursor
        .checked_add(length)
        .ok_or_else(|| malformed_error(artifact_sha256, "length_overflow"))?;
    let value = bytes
        .get(*cursor..end)
        .ok_or_else(|| malformed_error(artifact_sha256, "truncated_length_delimited"))?;
    *cursor = end;
    Ok(value)
}

fn read_varint(
    bytes: &[u8],
    cursor: &mut usize,
    artifact_sha256: &str,
) -> Result<u64, CompilerIndexError> {
    let start = *cursor;
    let mut value = 0_u64;
    for shift in (0..70).step_by(7) {
        let byte = *bytes
            .get(*cursor)
            .ok_or_else(|| malformed_error(artifact_sha256, "truncated_varint"))?;
        *cursor += 1;
        if shift == 63 && byte > 1 {
            return malformed(artifact_sha256, "varint_overflow");
        }
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            let width = *cursor - start;
            if width != minimal_varint_width(value) {
                return Err(CompilerIndexError::NoncanonicalArtifact {
                    artifact_sha256: artifact_sha256.to_owned(),
                    reason: "nonminimal_varint".to_owned(),
                });
            }
            return Ok(value);
        }
    }
    malformed(artifact_sha256, "varint_overflow")
}

const fn minimal_varint_width(mut value: u64) -> usize {
    let mut width = 1;
    while value >= 0x80 {
        value >>= 7;
        width += 1;
    }
    width
}

fn malformed<T>(artifact_sha256: &str, reason: &str) -> Result<T, CompilerIndexError> {
    Err(malformed_error(artifact_sha256, reason))
}

fn malformed_error(artifact_sha256: &str, reason: &str) -> CompilerIndexError {
    CompilerIndexError::MalformedArtifact {
        artifact_sha256: artifact_sha256.to_owned(),
        reason: reason.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use codenoesis_domain::s4_r7::CompilerIndexError;
    use protobuf::Message as _;
    use scip::types::Index;

    use super::preflight;
    use crate::binding::sha256;
    use crate::normalize::decode_canonical;

    fn fixture_bytes() -> Vec<u8> {
        fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/s4/compiler-index-v1/index.scip"),
        )
        .expect("read reviewed SCIP fixture")
    }

    #[test]
    fn sec_fr_ext_005_protobuf_preflight_precedes_decode() {
        let bytes = fixture_bytes();
        let digest = sha256(&bytes);
        preflight(&bytes, &digest).expect("reviewed fixture passes bounded preflight");
        let decoded = Index::parse_from_bytes(&bytes).expect("decode reviewed fixture");
        let encoded = decoded
            .write_to_bytes()
            .expect("re-encode reviewed fixture");
        let first_difference = bytes
            .iter()
            .zip(&encoded)
            .position(|(left, right)| left != right);
        if encoded != bytes {
            let index = first_difference.unwrap_or(bytes.len().min(encoded.len()));
            let start = index.saturating_sub(12);
            let source_end = (index + 24).min(bytes.len());
            let encoded_end = (index + 24).min(encoded.len());
            panic!(
                "pinned re-encode changed: first_difference={first_difference:?}, source_len={}, encoded_len={}, source={:?}, encoded={:?}",
                bytes.len(),
                encoded.len(),
                &bytes[start..source_end],
                &encoded[start..encoded_end]
            );
        }
        decode_canonical(&bytes, &digest).expect("reviewed fixture is canonical");

        let mut nonminimal = vec![0x8a, 0x00];
        nonminimal.extend_from_slice(&bytes[1..]);
        assert!(matches!(
            preflight(&nonminimal, &sha256(&nonminimal)),
            Err(CompilerIndexError::NoncanonicalArtifact { .. })
        ));

        let mut truncated = bytes.clone();
        truncated.pop();
        assert!(matches!(
            preflight(&truncated, &sha256(&truncated)),
            Err(CompilerIndexError::MalformedArtifact { .. })
        ));
    }
}
