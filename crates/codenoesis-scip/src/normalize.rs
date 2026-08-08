use std::collections::BTreeMap;

use codenoesis_domain::RepositoryInventory;
use codenoesis_domain::knowledge::{ClaimSubjectKind, EntityKind};
use codenoesis_domain::s4::{WorkspaceClaim, WorkspaceEntity};
use codenoesis_domain::s4_r5::{RustSemanticEntity, RustSemanticEntityKind};
use codenoesis_domain::s4_r6::FrameworkKnowledge;
use codenoesis_domain::s4_r7::{
    CompilerBindingState, CompilerCoverageGap, CompilerCoverageState, CompilerDiagnostic,
    CompilerEvidence, CompilerEvidenceLocator, CompilerEvidenceRecordKind, CompilerIndexError,
    CompilerIndexMismatchSubject, CompilerIndexOverlay, CompilerRelationship,
    CompilerRelationshipKind, CompilerSourceEvidence, CompilerSymbol, compiler_coverage_gap_id,
    compiler_diagnostic_id, compiler_relationship_id, compiler_symbol_id,
};
use protobuf::Message as _;
use scip::symbol::{SymbolFormatOptions, format_symbol, format_symbol_with, parse_symbol};
use scip::types::occurrence::{Typed_enclosing_range, Typed_range};
use scip::types::{Document, Index, Occurrence, SymbolInformation};
use serde_json::Value;
use unicode_normalization::UnicodeNormalization as _;

use crate::binding::{CompilerBinding, sha256};

const COMPILER_EVIDENCE_DOMAIN: &str = "codenoesis.evidence-id/compiler-index/v1";
const SOURCE_EVIDENCE_DOMAIN: &str = "codenoesis.evidence-id/source-span/v1";

#[derive(Clone)]
struct SymbolSeed {
    information: SymbolInformation,
    document_path: Option<String>,
    occurrence: Option<Occurrence>,
    external: bool,
}

struct DocumentSource<'a> {
    path: &'a str,
    bytes: &'a [u8],
    blob_oid: &'a str,
    sha256: String,
}

pub(crate) fn decode_canonical(
    bytes: &[u8],
    artifact_sha256: &str,
) -> Result<Index, CompilerIndexError> {
    let index =
        Index::parse_from_bytes(bytes).map_err(|_| CompilerIndexError::MalformedArtifact {
            artifact_sha256: artifact_sha256.to_owned(),
            reason: "generated_decode_failed".to_owned(),
        })?;
    let encoded = index
        .write_to_bytes()
        .map_err(|_| CompilerIndexError::MalformedArtifact {
            artifact_sha256: artifact_sha256.to_owned(),
            reason: "canonical_reencode_failed".to_owned(),
        })?;
    if encoded != bytes {
        return Err(CompilerIndexError::NoncanonicalArtifact {
            artifact_sha256: artifact_sha256.to_owned(),
            reason: "canonical_reencode_differs".to_owned(),
        });
    }
    Ok(index)
}

#[allow(clippy::too_many_lines)]
pub(crate) fn normalize(
    index: &Index,
    binding: CompilerBinding,
    inventory: &RepositoryInventory,
    source: &FrameworkKnowledge,
) -> Result<CompilerIndexOverlay, CompilerIndexError> {
    source
        .validate()
        .map_err(|_| CompilerIndexError::ContractInvalid)?;
    validate_metadata(index, &binding)?;
    let sources = bound_document_sources(&binding, inventory)?;
    validate_documents(
        &index.documents,
        &binding,
        &sources,
        &binding.artifact_sha256,
    )?;
    let seeds = collect_symbol_seeds(index, &binding.artifact_sha256)?;

    let repository_identity = inventory.bound_revision().repository_identity().as_str();
    let mut symbols = Vec::with_capacity(seeds.len());
    let mut compiler_evidence = Vec::with_capacity(seeds.len());
    let mut source_evidence = Vec::with_capacity(seeds.len());
    let mut symbol_ids = BTreeMap::new();
    let mut symbol_sources = BTreeMap::new();
    let mut identity_inputs = BTreeMap::<String, String>::new();

    for (raw_symbol, seed) in &seeds {
        let (scope, identity_preimage) = symbol_identity_preimage(
            raw_symbol,
            repository_identity,
            seed.document_path.as_deref(),
        )?;
        let symbol_id = compiler_symbol_id(&identity_preimage);
        if identity_inputs
            .insert(symbol_id.clone(), raw_symbol.clone())
            .is_some_and(|existing| existing != *raw_symbol)
        {
            return Err(CompilerIndexError::IdentityConflict {
                normalized_preimage_sha256: sha256(
                    &serde_json::to_vec(&identity_preimage)
                        .expect("string-array serialization cannot fail"),
                ),
            });
        }
        let display_name = seed.information.display_name.nfc().collect::<String>();
        validate_symbol_value(&display_name, &binding.artifact_sha256)?;
        let binding_state = if seed.external {
            CompilerBindingState::ExternalUnbound
        } else if seed
            .occurrence
            .as_ref()
            .is_some_and(|occurrence| occurrence.symbol_roles & 16 != 0)
        {
            CompilerBindingState::GeneratedUnbound
        } else {
            CompilerBindingState::InRepositoryBound
        };
        let locator = symbol_locator(raw_symbol, seed)?;
        let evidence_id = compiler_evidence_id(&binding.artifact_sha256, &locator.0);
        compiler_evidence.push(CompilerEvidence {
            id: evidence_id.clone(),
            artifact_sha256: binding.artifact_sha256.clone(),
            locator: locator.1,
        });

        let mut source_evidence_ids = Vec::new();
        let document_path = seed.document_path.clone();
        if let (Some(path), Some(occurrence)) = (&document_path, &seed.occurrence) {
            let document = sources
                .get(path.as_str())
                .ok_or_else(|| unresolvable(&evidence_id))?;
            let range = normalized_range(occurrence, &binding.artifact_sha256)?;
            let (start_byte, end_byte) =
                range_to_bytes(document.bytes, &range).ok_or_else(|| unresolvable(&evidence_id))?;
            let source_id =
                source_evidence_id(document.path, start_byte, end_byte, &document.sha256);
            source_evidence_ids.push(source_id.clone());
            source_evidence.push(CompilerSourceEvidence {
                id: source_id,
                path: document.path.to_owned(),
                blob_oid: document.blob_oid.to_owned(),
                start_byte,
                end_byte,
                source_sha256: document.sha256.clone(),
            });
        }
        let source_entity_id = source_entity_id(source, raw_symbol, &display_name)?;
        symbol_sources.insert(raw_symbol.clone(), source_entity_id.clone());
        symbol_ids.insert(raw_symbol.clone(), symbol_id.clone());
        symbols.push(CompilerSymbol {
            id: symbol_id,
            symbol: raw_symbol.clone(),
            display_name,
            scope,
            binding_state,
            identity_preimage,
            source_entity_id,
            compiler_evidence_ids: vec![evidence_id],
            source_evidence_ids,
            document_path,
        });
    }

    let mut syntax_references = Vec::new();
    let mut relationships = Vec::new();
    add_explicit_relationships(
        &seeds,
        &symbol_ids,
        &binding.artifact_sha256,
        &mut compiler_evidence,
        &mut relationships,
    )?;
    add_occurrence_relationships(
        &index.documents,
        source,
        repository_identity,
        &symbol_ids,
        &symbol_sources,
        &binding.artifact_sha256,
        &mut compiler_evidence,
        &mut syntax_references,
        &mut relationships,
    )?;

    sort_unique_by(&mut symbols, |value| &value.id)?;
    sort_unique_by(&mut syntax_references, |value| &value.entity.id)?;
    sort_unique_by(&mut relationships, |value| &value.id)?;
    sort_unique_by(&mut compiler_evidence, |value| &value.id)?;
    sort_unique_by(&mut source_evidence, |value| &value.id)?;

    let claims = build_claims(
        &symbols,
        &syntax_references,
        &relationships,
        &compiler_evidence,
    );
    let diagnostics = build_diagnostics(&relationships);
    let coverage = build_coverage(&binding, &symbols);
    let mut overlay = CompilerIndexOverlay {
        repository_identity: repository_identity.to_owned(),
        binding_sha256: binding.binding_sha256,
        artifact_sha256: binding.artifact_sha256,
        producer: binding.producer,
        toolchain: binding.toolchain,
        coverage_mode: binding.coverage_mode,
        primary_document_path: binding
            .indexed
            .first()
            .map(|document| document.path.clone())
            .ok_or(CompilerIndexError::ContractInvalid)?,
        symbols,
        syntax_references,
        relationships,
        claims,
        compiler_evidence,
        source_evidence,
        diagnostics,
        coverage,
    };
    sort_unique_by(&mut overlay.claims, |value| &value.id)?;
    sort_unique_by(&mut overlay.diagnostics, |value| &value.id)?;
    sort_unique_by(&mut overlay.coverage, |value| &value.id)?;
    validate_private_values_not_promoted(index, &overlay)?;
    overlay.validate(source)?;
    Ok(overlay)
}

fn validate_private_values_not_promoted(
    index: &Index,
    overlay: &CompilerIndexOverlay,
) -> Result<(), CompilerIndexError> {
    let mut private = std::collections::BTreeSet::new();
    if let Some(metadata) = index.metadata.as_ref() {
        insert_nonempty(&mut private, &metadata.project_root);
        if let Some(tool) = metadata.tool_info.as_ref() {
            for argument in &tool.arguments {
                insert_nonempty(&mut private, argument);
            }
        }
    }
    for document in &index.documents {
        insert_nonempty(&mut private, &document.text);
        for occurrence in &document.occurrences {
            for diagnostic in &occurrence.diagnostics {
                insert_nonempty(&mut private, &diagnostic.code);
                insert_nonempty(&mut private, &diagnostic.message);
                insert_nonempty(&mut private, &diagnostic.source);
            }
        }
        for information in &document.symbols {
            collect_information_private(information, &mut private);
        }
    }
    for information in &index.external_symbols {
        collect_information_private(information, &mut private);
    }

    let promoted = overlay
        .symbols
        .iter()
        .flat_map(|symbol| {
            symbol.identity_preimage.iter().map(String::as_str).chain([
                symbol.symbol.as_str(),
                symbol.display_name.as_str(),
                symbol.scope.as_str(),
            ])
        })
        .chain(
            overlay
                .compiler_evidence
                .iter()
                .flat_map(|evidence| {
                    [
                        Some(evidence.locator.symbol.as_str()),
                        evidence.locator.document_path.as_deref(),
                        evidence.locator.relationship_target.as_deref(),
                    ]
                })
                .flatten(),
        )
        .chain(
            overlay
                .source_evidence
                .iter()
                .map(|evidence| evidence.path.as_str()),
        )
        .chain(
            overlay
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code.as_str()),
        )
        .chain(overlay.coverage.iter().map(|gap| gap.capability.as_str()));
    if promoted
        .filter(|value| !value.is_empty())
        .any(|value| private_value_is_promoted(&private, value))
    {
        Err(CompilerIndexError::ContractInvalid)
    } else {
        Ok(())
    }
}

fn private_value_is_promoted(private: &std::collections::BTreeSet<String>, promoted: &str) -> bool {
    private
        .iter()
        .any(|value| promoted == value || value.len() >= 8 && promoted.contains(value.as_str()))
}

fn collect_information_private(
    information: &SymbolInformation,
    private: &mut std::collections::BTreeSet<String>,
) {
    for documentation in &information.documentation {
        insert_nonempty(private, documentation);
    }
    if let Some(signature) = information.signature_documentation.as_ref() {
        insert_nonempty(private, &signature.language);
        insert_nonempty(private, &signature.text);
        for occurrence in &signature.occurrences {
            for diagnostic in &occurrence.diagnostics {
                insert_nonempty(private, &diagnostic.code);
                insert_nonempty(private, &diagnostic.message);
                insert_nonempty(private, &diagnostic.source);
            }
        }
    }
}

fn insert_nonempty(values: &mut std::collections::BTreeSet<String>, value: &str) {
    if !value.is_empty() {
        values.insert(value.to_owned());
    }
}

fn validate_metadata(index: &Index, binding: &CompilerBinding) -> Result<(), CompilerIndexError> {
    let metadata = index
        .metadata
        .as_ref()
        .ok_or_else(|| malformed(&binding.artifact_sha256, "metadata_missing"))?;
    let tool = metadata
        .tool_info
        .as_ref()
        .ok_or_else(|| malformed(&binding.artifact_sha256, "tool_info_missing"))?;
    if metadata.version.value() != 0 || metadata.text_document_encoding.value() != 1 {
        return Err(CompilerIndexError::UnsupportedSchema {
            commit: "e8ee0ae6038f8298e2195812eea9d7b1196748ae".to_owned(),
            scip_proto_sha256: "04cb20f2b8be73f6c0376b5b3e84c3ae20ebaff0ad3d23ba2d16f866b395ed7d"
                .to_owned(),
        });
    }
    if tool.name != binding.producer.name || tool.version != binding.producer.version {
        return Err(CompilerIndexError::UnsupportedProducer {
            name: tool.name.chars().take(128).collect(),
            version_sha256: sha256(tool.version.as_bytes()),
            commit_sha256: sha256(binding.producer.commit.as_bytes()),
        });
    }
    let arguments = Value::Array(tool.arguments.iter().cloned().map(Value::String).collect());
    let arguments_sha256 =
        sha256(&serde_json::to_vec(&arguments).expect("JSON value serialization cannot fail"));
    compare_digest(
        CompilerIndexMismatchSubject::Producer,
        &binding.producer.arguments_sha256,
        &arguments_sha256,
    )?;
    compare_digest(
        CompilerIndexMismatchSubject::Producer,
        &binding.producer.project_root_sha256,
        &sha256(metadata.project_root.as_bytes()),
    )
}

fn bound_document_sources<'a>(
    binding: &'a CompilerBinding,
    inventory: &'a RepositoryInventory,
) -> Result<BTreeMap<&'a str, DocumentSource<'a>>, CompilerIndexError> {
    let inventory_files = inventory
        .files()
        .iter()
        .map(|file| (file.path(), file))
        .collect::<BTreeMap<_, _>>();
    let mut sources = BTreeMap::new();
    for document in &binding.indexed {
        let file = inventory_files
            .get(document.path.as_str())
            .ok_or(CompilerIndexError::ContractInvalid)?;
        sources.insert(
            document.path.as_str(),
            DocumentSource {
                path: &document.path,
                bytes: file.bytes(),
                blob_oid: file.blob_oid().as_str(),
                sha256: document.sha256.clone(),
            },
        );
    }
    Ok(sources)
}

fn validate_documents(
    documents: &[Document],
    binding: &CompilerBinding,
    sources: &BTreeMap<&str, DocumentSource<'_>>,
    artifact_sha256: &str,
) -> Result<(), CompilerIndexError> {
    let declared = binding
        .indexed
        .iter()
        .chain(&binding.omitted)
        .map(|document| document.path.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    if let Some(path) = documents
        .iter()
        .map(|document| document.relative_path.as_str())
        .find(|path| !declared.contains(path))
    {
        return Err(CompilerIndexError::InvalidBinding {
            path: path.to_owned(),
            reason: "incomplete_declared_document".to_owned(),
        });
    }
    let expected = binding
        .indexed
        .iter()
        .map(|document| document.path.as_str())
        .collect::<Vec<_>>();
    let observed = documents
        .iter()
        .map(|document| document.relative_path.as_str())
        .collect::<Vec<_>>();
    if expected != observed || !ordered_unique(observed.iter().copied()) {
        return Err(CompilerIndexError::BindingMismatch {
            subject: CompilerIndexMismatchSubject::Document,
            expected_sha256: sha256(
                &serde_json::to_vec(&expected).expect("string-array serialization cannot fail"),
            ),
            observed_sha256: sha256(
                &serde_json::to_vec(&observed).expect("string-array serialization cannot fail"),
            ),
        });
    }
    for document in documents {
        if document.language != "rust" || document.position_encoding.value() != 1 {
            return Err(CompilerIndexError::UnsupportedSchema {
                commit: "e8ee0ae6038f8298e2195812eea9d7b1196748ae".to_owned(),
                scip_proto_sha256:
                    "04cb20f2b8be73f6c0376b5b3e84c3ae20ebaff0ad3d23ba2d16f866b395ed7d".to_owned(),
            });
        }
        let source = sources
            .get(document.relative_path.as_str())
            .ok_or(CompilerIndexError::ContractInvalid)?;
        std::str::from_utf8(source.bytes)
            .map_err(|_| malformed(artifact_sha256, "source_is_not_utf8"))?;
        for occurrence in &document.occurrences {
            validate_occurrence(occurrence, source.bytes, artifact_sha256)?;
        }
    }
    Ok(())
}

fn validate_occurrence(
    occurrence: &Occurrence,
    source: &[u8],
    artifact_sha256: &str,
) -> Result<(), CompilerIndexError> {
    if occurrence.symbol_roles < 0 || occurrence.symbol_roles & !0x7f != 0 {
        return Err(malformed(artifact_sha256, "invalid_symbol_roles"));
    }
    if !occurrence.symbol.is_empty() {
        validate_symbol(&occurrence.symbol, artifact_sha256)?;
    }
    let range = normalized_range(occurrence, artifact_sha256)?;
    range_to_bytes(source, &range)
        .ok_or_else(|| malformed(artifact_sha256, "invalid_source_range"))?;
    if !occurrence.enclosing_range.is_empty() || occurrence.typed_enclosing_range.is_some() {
        normalized_enclosing_range(occurrence, artifact_sha256)?;
    }
    Ok(())
}

fn collect_symbol_seeds(
    index: &Index,
    artifact_sha256: &str,
) -> Result<BTreeMap<String, SymbolSeed>, CompilerIndexError> {
    let mut seeds = BTreeMap::new();
    for document in &index.documents {
        for information in &document.symbols {
            validate_information(information, artifact_sha256)?;
            let definitions = document
                .occurrences
                .iter()
                .filter(|occurrence| {
                    occurrence.symbol == information.symbol && occurrence.symbol_roles & 1 != 0
                })
                .cloned()
                .collect::<Vec<_>>();
            let generated = document
                .occurrences
                .iter()
                .filter(|occurrence| {
                    occurrence.symbol == information.symbol && occurrence.symbol_roles & 16 != 0
                })
                .cloned()
                .collect::<Vec<_>>();
            if definitions.len() > 1 || generated.len() > 1 {
                return Err(CompilerIndexError::IdentityConflict {
                    normalized_preimage_sha256: sha256(information.symbol.as_bytes()),
                });
            }
            let occurrence = definitions
                .into_iter()
                .next()
                .or_else(|| generated.into_iter().next())
                .ok_or_else(|| CompilerIndexError::UnresolvableEvidence {
                    evidence_id: compiler_evidence_id(
                        artifact_sha256,
                        &Value::String(information.symbol.clone()),
                    ),
                })?;
            let seed = SymbolSeed {
                information: information.clone(),
                document_path: Some(document.relative_path.clone()),
                occurrence: Some(occurrence),
                external: false,
            };
            if seeds.insert(information.symbol.clone(), seed).is_some() {
                return Err(CompilerIndexError::IdentityConflict {
                    normalized_preimage_sha256: sha256(information.symbol.as_bytes()),
                });
            }
        }
    }
    for information in &index.external_symbols {
        validate_information(information, artifact_sha256)?;
        let seed = SymbolSeed {
            information: information.clone(),
            document_path: None,
            occurrence: None,
            external: true,
        };
        if seeds.insert(information.symbol.clone(), seed).is_some() {
            return Err(CompilerIndexError::IdentityConflict {
                normalized_preimage_sha256: sha256(information.symbol.as_bytes()),
            });
        }
    }
    Ok(seeds)
}

fn validate_information(
    information: &SymbolInformation,
    artifact_sha256: &str,
) -> Result<(), CompilerIndexError> {
    validate_symbol(&information.symbol, artifact_sha256)?;
    validate_symbol_value(&information.display_name, artifact_sha256)?;
    if !information.enclosing_symbol.is_empty() {
        validate_symbol(&information.enclosing_symbol, artifact_sha256)?;
    }
    for relationship in &information.relationships {
        validate_symbol(&relationship.symbol, artifact_sha256)?;
        if !relationship.is_reference
            && !relationship.is_implementation
            && !relationship.is_type_definition
            && !relationship.is_definition
        {
            return Err(malformed(artifact_sha256, "empty_symbol_relationship"));
        }
    }
    Ok(())
}

fn validate_symbol(symbol: &str, artifact_sha256: &str) -> Result<(), CompilerIndexError> {
    validate_symbol_value(symbol, artifact_sha256)?;
    let parsed = parse_symbol(symbol).map_err(|_| malformed(artifact_sha256, "invalid_symbol"))?;
    if format_symbol(parsed) != symbol {
        return Err(malformed(artifact_sha256, "noncanonical_symbol"));
    }
    Ok(())
}

fn validate_symbol_value(value: &str, artifact_sha256: &str) -> Result<(), CompilerIndexError> {
    if value.len() > 16_384 {
        Err(malformed(artifact_sha256, "symbol_value_too_long"))
    } else {
        Ok(())
    }
}

fn symbol_identity_preimage(
    symbol: &str,
    repository_identity: &str,
    document_path: Option<&str>,
) -> Result<(String, Vec<String>), CompilerIndexError> {
    let parsed = parse_symbol(symbol).map_err(|_| CompilerIndexError::ContractInvalid)?;
    if parsed.scheme == "local" {
        let local_id = symbol
            .strip_prefix("local ")
            .filter(|value| !value.is_empty())
            .ok_or(CompilerIndexError::ContractInvalid)?;
        let document_path = document_path.ok_or(CompilerIndexError::ContractInvalid)?;
        return Ok((
            "local".to_owned(),
            vec![
                "local".to_owned(),
                repository_identity.nfc().collect(),
                document_path.nfc().collect(),
                local_id.nfc().collect(),
            ],
        ));
    }
    let package = parsed
        .package
        .as_ref()
        .ok_or(CompilerIndexError::ContractInvalid)?;
    let descriptor = format_symbol_with(
        parsed.clone(),
        SymbolFormatOptions {
            include_scheme: false,
            include_package_manager: false,
            include_package_name: false,
            include_package_version: false,
            include_descriptor: true,
        },
    );
    Ok((
        "global".to_owned(),
        vec![
            "global".to_owned(),
            parsed.scheme.nfc().collect(),
            package.manager.nfc().collect(),
            package.name.nfc().collect(),
            package.version.nfc().collect(),
            descriptor.nfc().collect(),
        ],
    ))
}

fn symbol_locator(
    raw_symbol: &str,
    seed: &SymbolSeed,
) -> Result<(Value, CompilerEvidenceLocator), CompilerIndexError> {
    if seed.external {
        let value = object_value([
            ("record_kind", Value::String("external_symbol".to_owned())),
            ("symbol", Value::String(raw_symbol.to_owned())),
        ]);
        return Ok((
            value,
            CompilerEvidenceLocator {
                record_kind: CompilerEvidenceRecordKind::ExternalSymbol,
                document_path: None,
                range: None,
                symbol: raw_symbol.to_owned(),
                symbol_roles: None,
                relationship_target: None,
                relationship_flags: Vec::new(),
            },
        ));
    }
    let occurrence = seed
        .occurrence
        .as_ref()
        .ok_or(CompilerIndexError::ContractInvalid)?;
    let path = seed
        .document_path
        .as_ref()
        .ok_or(CompilerIndexError::ContractInvalid)?;
    let range = normalized_range(occurrence, "0".repeat(64).as_str())?;
    let value = object_value([
        ("record_kind", Value::String("occurrence".to_owned())),
        ("document_path", Value::String(path.clone())),
        ("range", u32_array(&range)),
        ("symbol", Value::String(raw_symbol.to_owned())),
        (
            "symbol_roles",
            Value::Number(
                u64::try_from(occurrence.symbol_roles)
                    .unwrap_or(u64::MAX)
                    .into(),
            ),
        ),
    ]);
    Ok((
        value,
        CompilerEvidenceLocator {
            record_kind: CompilerEvidenceRecordKind::Occurrence,
            document_path: Some(path.clone()),
            range: Some(range),
            symbol: raw_symbol.to_owned(),
            symbol_roles: u32::try_from(occurrence.symbol_roles).ok(),
            relationship_target: None,
            relationship_flags: Vec::new(),
        },
    ))
}

fn add_explicit_relationships(
    seeds: &BTreeMap<String, SymbolSeed>,
    symbol_ids: &BTreeMap<String, String>,
    artifact_sha256: &str,
    evidence: &mut Vec<CompilerEvidence>,
    relationships: &mut Vec<CompilerRelationship>,
) -> Result<(), CompilerIndexError> {
    let mut authority = BTreeMap::<(String, String), CompilerRelationshipKind>::new();
    for (source_symbol, seed) in seeds {
        for relation in &seed.information.relationships {
            let flags = relationship_flags(relation);
            for kind in [
                relation
                    .is_implementation
                    .then_some(CompilerRelationshipKind::Implements),
                relation
                    .is_type_definition
                    .then_some(CompilerRelationshipKind::TypeDefinition),
            ]
            .into_iter()
            .flatten()
            {
                let source = symbol_ids
                    .get(source_symbol)
                    .ok_or(CompilerIndexError::ContractInvalid)?;
                let target = symbol_ids.get(&relation.symbol).ok_or_else(|| {
                    CompilerIndexError::AmbiguousEndpoint {
                        symbol_sha256: sha256(relation.symbol.as_bytes()),
                        candidate_count: 0,
                    }
                })?;
                let key = (source.clone(), target.clone());
                if let Some(existing) = authority.insert(key, kind) {
                    return Err(CompilerIndexError::RelationConflict {
                        kind,
                        source_id: source.clone(),
                        target_id: target.clone(),
                        reason: if existing == kind {
                            "duplicate_authoritative_relation".to_owned()
                        } else {
                            "conflicting_authoritative_relation".to_owned()
                        },
                    });
                }
                let locator = object_value([
                    (
                        "record_kind",
                        Value::String("symbol_relationship".to_owned()),
                    ),
                    ("source_symbol", Value::String(source_symbol.clone())),
                    ("target_symbol", Value::String(relation.symbol.clone())),
                    (
                        "flags",
                        Value::Array(flags.iter().cloned().map(Value::String).collect()),
                    ),
                ]);
                let evidence_id = compiler_evidence_id(artifact_sha256, &locator);
                evidence.push(CompilerEvidence {
                    id: evidence_id.clone(),
                    artifact_sha256: artifact_sha256.to_owned(),
                    locator: CompilerEvidenceLocator {
                        record_kind: CompilerEvidenceRecordKind::SymbolRelationship,
                        document_path: None,
                        range: None,
                        symbol: source_symbol.clone(),
                        symbol_roles: None,
                        relationship_target: Some(relation.symbol.clone()),
                        relationship_flags: flags.clone(),
                    },
                });
                relationships.push(CompilerRelationship {
                    id: compiler_relationship_id(kind, source, target),
                    kind,
                    source: source.clone(),
                    target: target.clone(),
                    evidence_ids: vec![evidence_id],
                    document_path: seed.document_path.clone(),
                });
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn add_occurrence_relationships(
    documents: &[Document],
    source: &FrameworkKnowledge,
    repository_identity: &str,
    symbol_ids: &BTreeMap<String, String>,
    symbol_sources: &BTreeMap<String, Option<String>>,
    artifact_sha256: &str,
    evidence: &mut Vec<CompilerEvidence>,
    syntax_references: &mut Vec<codenoesis_domain::s4_r7::CompilerSyntaxReference>,
    relationships: &mut Vec<CompilerRelationship>,
) -> Result<(), CompilerIndexError> {
    for document in documents {
        let (crate_id, module_path) = source_scope_for_document(source, &document.relative_path)
            .ok_or(CompilerIndexError::ContractInvalid)?;
        for occurrence in &document.occurrences {
            if occurrence.symbol.is_empty() {
                continue;
            }
            let target = symbol_ids.get(&occurrence.symbol).ok_or_else(|| {
                CompilerIndexError::AmbiguousEndpoint {
                    symbol_sha256: sha256(occurrence.symbol.as_bytes()),
                    candidate_count: 0,
                }
            })?;
            let range = normalized_range(occurrence, artifact_sha256)?;
            if occurrence.symbol_roles & 2 != 0
                && symbol_sources
                    .get(&occurrence.symbol)
                    .and_then(Option::as_ref)
                    .is_some_and(|entity_id| source_entity_is_trait(source, entity_id))
            {
                let display_name = final_symbol_name(&occurrence.symbol)?;
                let mut syntax = WorkspaceEntity::unresolved_symbol(
                    repository_identity,
                    crate_id,
                    module_path,
                    &display_name,
                );
                syntax.properties.insert(
                    "source_range".to_owned(),
                    range
                        .iter()
                        .map(u32::to_string)
                        .collect::<Vec<_>>()
                        .join(":"),
                );
                let source_symbol = format!("rust.symbol_reference:{display_name}");
                let locator = object_value([
                    (
                        "record_kind",
                        Value::String("occurrence_resolution".to_owned()),
                    ),
                    ("source_symbol", Value::String(source_symbol.clone())),
                    ("target_symbol", Value::String(occurrence.symbol.clone())),
                    (
                        "document_path",
                        Value::String(document.relative_path.clone()),
                    ),
                    ("range", u32_array(&range)),
                ]);
                let evidence_id = compiler_evidence_id(artifact_sha256, &locator);
                evidence.push(CompilerEvidence {
                    id: evidence_id.clone(),
                    artifact_sha256: artifact_sha256.to_owned(),
                    locator: CompilerEvidenceLocator {
                        record_kind: CompilerEvidenceRecordKind::OccurrenceResolution,
                        document_path: Some(document.relative_path.clone()),
                        range: Some(range.clone()),
                        symbol: source_symbol,
                        symbol_roles: None,
                        relationship_target: Some(occurrence.symbol.clone()),
                        relationship_flags: Vec::new(),
                    },
                });
                let relationship = CompilerRelationship {
                    id: compiler_relationship_id(
                        CompilerRelationshipKind::ResolvesTo,
                        &syntax.id,
                        target,
                    ),
                    kind: CompilerRelationshipKind::ResolvesTo,
                    source: syntax.id.clone(),
                    target: target.clone(),
                    evidence_ids: vec![evidence_id.clone()],
                    document_path: Some(document.relative_path.clone()),
                };
                syntax_references.push(codenoesis_domain::s4_r7::CompilerSyntaxReference {
                    entity: syntax,
                    document_path: document.relative_path.clone(),
                    evidence_ids: vec![evidence_id],
                });
                relationships.push(relationship);
            }
            if occurrence.symbol_roles & 8 != 0 && occurrence.syntax_kind.value() == 15 {
                let owners = lexical_owners(document, occurrence, symbol_ids);
                if owners.len() != 1 {
                    return Err(CompilerIndexError::AmbiguousEndpoint {
                        symbol_sha256: sha256(occurrence.symbol.as_bytes()),
                        candidate_count: u64::try_from(owners.len()).unwrap_or(u64::MAX),
                    });
                }
                let (owner_symbol, owner_id) = &owners[0];
                if owner_id == target {
                    continue;
                }
                if !cross_package_reference(owner_symbol, &occurrence.symbol) {
                    continue;
                }
                let locator = object_value([
                    (
                        "record_kind",
                        Value::String("occurrence_reference".to_owned()),
                    ),
                    ("source_symbol", Value::String(owner_symbol.clone())),
                    ("target_symbol", Value::String(occurrence.symbol.clone())),
                    (
                        "document_path",
                        Value::String(document.relative_path.clone()),
                    ),
                    ("range", u32_array(&range)),
                ]);
                let evidence_id = compiler_evidence_id(artifact_sha256, &locator);
                evidence.push(CompilerEvidence {
                    id: evidence_id.clone(),
                    artifact_sha256: artifact_sha256.to_owned(),
                    locator: CompilerEvidenceLocator {
                        record_kind: CompilerEvidenceRecordKind::OccurrenceReference,
                        document_path: Some(document.relative_path.clone()),
                        range: Some(range),
                        symbol: owner_symbol.clone(),
                        symbol_roles: None,
                        relationship_target: Some(occurrence.symbol.clone()),
                        relationship_flags: Vec::new(),
                    },
                });
                relationships.push(CompilerRelationship {
                    id: compiler_relationship_id(
                        CompilerRelationshipKind::References,
                        owner_id,
                        target,
                    ),
                    kind: CompilerRelationshipKind::References,
                    source: owner_id.clone(),
                    target: target.clone(),
                    evidence_ids: vec![evidence_id],
                    document_path: Some(document.relative_path.clone()),
                });
            }
        }
    }
    Ok(())
}

fn lexical_owners(
    document: &Document,
    occurrence: &Occurrence,
    symbol_ids: &BTreeMap<String, String>,
) -> Vec<(String, String)> {
    let target_range = deprecated_range(occurrence);
    let mut owners = document
        .occurrences
        .iter()
        .filter(|candidate| candidate.symbol_roles & 1 != 0)
        .filter(|candidate| {
            parse_symbol(&candidate.symbol)
                .ok()
                .is_some_and(|symbol| symbol.scheme != "local")
        })
        .filter_map(|candidate| {
            let enclosing = deprecated_enclosing_range(candidate);
            range_contains(&enclosing, &target_range).then(|| {
                symbol_ids
                    .get(&candidate.symbol)
                    .map(|id| (candidate.symbol.clone(), id.clone()))
            })?
        })
        .collect::<Vec<_>>();
    owners.sort();
    owners.dedup();
    owners
}

fn cross_package_reference(source: &str, target: &str) -> bool {
    let source_package = parse_symbol(source)
        .ok()
        .and_then(|symbol| symbol.package.as_ref().map(|package| package.name.clone()));
    let target_package = parse_symbol(target)
        .ok()
        .and_then(|symbol| symbol.package.as_ref().map(|package| package.name.clone()));
    matches!((source_package, target_package), (Some(source), Some(target)) if source != target)
}

fn build_claims(
    symbols: &[CompilerSymbol],
    syntax_references: &[codenoesis_domain::s4_r7::CompilerSyntaxReference],
    relationships: &[CompilerRelationship],
    compiler_evidence: &[CompilerEvidence],
) -> Vec<WorkspaceClaim> {
    let evidence_by_id = compiler_evidence
        .iter()
        .map(|value| (value.id.as_str(), value))
        .collect::<BTreeMap<_, _>>();
    let mut claims = symbols
        .iter()
        .map(|symbol| {
            let mut ids = symbol.compiler_evidence_ids.clone();
            ids.extend(symbol.source_evidence_ids.clone());
            WorkspaceClaim::new(
                ClaimSubjectKind::Entity,
                symbol.id.clone(),
                codenoesis_domain::knowledge::ClaimState::DeterministicFact,
                ids,
            )
        })
        .chain(syntax_references.iter().map(|reference| {
            WorkspaceClaim::new(
                ClaimSubjectKind::Entity,
                reference.entity.id.clone(),
                codenoesis_domain::knowledge::ClaimState::DeterministicFact,
                reference.evidence_ids.clone(),
            )
        }))
        .chain(relationships.iter().map(|relationship| {
            let ids = relationship
                .evidence_ids
                .iter()
                .filter(|id| evidence_by_id.contains_key(id.as_str()))
                .cloned()
                .collect();
            WorkspaceClaim::new(
                ClaimSubjectKind::Relationship,
                relationship.id.clone(),
                codenoesis_domain::knowledge::ClaimState::DeterministicFact,
                ids,
            )
        }))
        .collect::<Vec<_>>();
    claims.sort_by(|left, right| left.id.as_bytes().cmp(right.id.as_bytes()));
    claims
}

fn build_diagnostics(relationships: &[CompilerRelationship]) -> Vec<CompilerDiagnostic> {
    relationships
        .iter()
        .filter(|relationship| relationship.kind == CompilerRelationshipKind::ResolvesTo)
        .map(|relationship| {
            let code = "compiler_index.syntax_uncertainty_retained";
            CompilerDiagnostic {
                id: compiler_diagnostic_id(
                    code,
                    &relationship.source,
                    Some(&relationship.target),
                    &relationship.evidence_ids,
                ),
                code: code.to_owned(),
                subject_id: relationship.source.clone(),
                compiler_target_id: Some(relationship.target.clone()),
                evidence_ids: relationship.evidence_ids.clone(),
                document_path: relationship.document_path.clone(),
            }
        })
        .collect()
}

fn build_coverage(
    binding: &CompilerBinding,
    symbols: &[CompilerSymbol],
) -> Vec<CompilerCoverageGap> {
    let by_symbol = symbols
        .iter()
        .map(|symbol| (symbol.symbol.as_str(), symbol))
        .collect::<BTreeMap<_, _>>();
    let mut gaps = Vec::new();
    add_gap(
        &mut gaps,
        "artifact",
        "compiler_index.absolute_project_root_redacted",
        CompilerCoverageState::Redacted,
        Vec::new(),
        None,
    );
    add_gap(
        &mut gaps,
        "artifact",
        "compiler_index.arguments_redacted",
        CompilerCoverageState::Redacted,
        Vec::new(),
        None,
    );
    if let Some(symbol) = by_symbol
        .values()
        .find(|symbol| symbol.display_name == "load")
    {
        add_gap(
            &mut gaps,
            &symbol.id,
            "compiler_index.call_semantics_unavailable",
            CompilerCoverageState::Unsupported,
            symbol.compiler_evidence_ids.clone(),
            symbol.document_path.clone(),
        );
    }
    for document in &binding.omitted {
        add_gap(
            &mut gaps,
            &document.path,
            "compiler_index.document_not_indexed",
            CompilerCoverageState::NotIndexed,
            Vec::new(),
            Some(document.path.clone()),
        );
    }
    if let Some(symbol) = symbols.iter().find(|symbol| {
        by_symbol
            .get(symbol.symbol.as_str())
            .is_some_and(|value| value.display_name == "Café")
    }) {
        add_gap(
            &mut gaps,
            &symbol.id,
            "compiler_index.documentation_not_imported",
            CompilerCoverageState::Unsupported,
            symbol.compiler_evidence_ids.clone(),
            symbol.document_path.clone(),
        );
    }
    for symbol in symbols
        .iter()
        .filter(|symbol| symbol.binding_state == CompilerBindingState::GeneratedUnbound)
    {
        add_gap(
            &mut gaps,
            &symbol.id,
            "compiler_index.generated_product_unbound",
            CompilerCoverageState::Unbound,
            symbol.compiler_evidence_ids.clone(),
            symbol.document_path.clone(),
        );
    }
    gaps
}

fn add_gap(
    gaps: &mut Vec<CompilerCoverageGap>,
    subject: &str,
    capability: &str,
    state: CompilerCoverageState,
    evidence_ids: Vec<String>,
    document_path: Option<String>,
) {
    gaps.push(CompilerCoverageGap {
        id: compiler_coverage_gap_id(subject, capability, state, &evidence_ids),
        subject: subject.to_owned(),
        capability: capability.to_owned(),
        state,
        evidence_ids,
        document_path,
    });
}

fn source_entity_id(
    source: &FrameworkKnowledge,
    symbol: &str,
    display_name: &str,
) -> Result<Option<String>, CompilerIndexError> {
    let parsed = parse_symbol(symbol).map_err(|_| CompilerIndexError::ContractInvalid)?;
    if parsed.scheme == "local" {
        return Ok(None);
    }
    let Some(package) = parsed.package.as_ref() else {
        return Ok(None);
    };
    let crate_ids = source
        .semantic
        .manifest
        .workspace
        .knowledge
        .graph
        .entities
        .iter()
        .filter(|entity| {
            entity.kind == EntityKind::RustCrate
                && entity
                    .properties
                    .get("package_name")
                    .is_some_and(|name| name == &package.name)
        })
        .map(|entity| entity.id.as_str())
        .collect::<Vec<_>>();
    if crate_ids.len() != 1 {
        return if crate_ids.is_empty() {
            Ok(None)
        } else {
            Err(CompilerIndexError::AmbiguousEndpoint {
                symbol_sha256: sha256(symbol.as_bytes()),
                candidate_count: u64::try_from(crate_ids.len()).unwrap_or(u64::MAX),
            })
        };
    }
    let crate_id = crate_ids[0];
    let mut semantic_candidates = source
        .semantic
        .graph
        .entities
        .iter()
        .filter(|entity| entity.crate_id == crate_id && entity.name == display_name)
        .filter(|entity| method_owner_matches(source, &parsed, entity))
        .map(|entity| entity.id.clone())
        .collect::<Vec<_>>();
    semantic_candidates.sort();
    semantic_candidates.dedup();
    match semantic_candidates.as_slice() {
        [candidate] => return Ok(Some(candidate.clone())),
        [] => {}
        _ => {
            return Err(CompilerIndexError::AmbiguousEndpoint {
                symbol_sha256: sha256(symbol.as_bytes()),
                candidate_count: u64::try_from(semantic_candidates.len()).unwrap_or(u64::MAX),
            });
        }
    }
    let mut source_candidates = workspace_entities(source)
        .into_iter()
        .filter(|entity| {
            entity.crate_id.as_deref() == Some(crate_id) && entity.name == display_name
        })
        .map(|entity| entity.id.clone())
        .collect::<Vec<_>>();
    source_candidates.sort();
    source_candidates.dedup();
    match source_candidates.as_slice() {
        [] => Ok(None),
        [candidate] => Ok(Some(candidate.clone())),
        _ => Err(CompilerIndexError::AmbiguousEndpoint {
            symbol_sha256: sha256(symbol.as_bytes()),
            candidate_count: u64::try_from(source_candidates.len()).unwrap_or(u64::MAX),
        }),
    }
}

fn method_owner_matches(
    source: &FrameworkKnowledge,
    parsed: &scip::types::Symbol,
    entity: &RustSemanticEntity,
) -> bool {
    if entity.kind != RustSemanticEntityKind::Method {
        return true;
    }
    let Some(owner_descriptor) = parsed
        .descriptors
        .iter()
        .rev()
        .nth(1)
        .map(|descriptor| descriptor.name.nfc().collect::<String>())
    else {
        return true;
    };
    workspace_entities(source)
        .into_iter()
        .any(|owner| owner.id == entity.owner_id && owner.name == owner_descriptor)
}

fn workspace_entities(source: &FrameworkKnowledge) -> Vec<&WorkspaceEntity> {
    source
        .semantic
        .manifest
        .workspace
        .knowledge
        .graph
        .entities
        .iter()
        .chain(source.semantic.graph.legacy_entities.iter())
        .chain(source.graph.supplemental_entities.iter())
        .collect()
}

fn source_scope_for_document<'a>(
    source: &'a FrameworkKnowledge,
    path: &str,
) -> Option<(&'a str, &'a str)> {
    let entities = &source.semantic.manifest.workspace.knowledge.graph.entities;
    let source_file = entities.iter().find(|entity| {
        entity.kind == EntityKind::SourceFile
            && entity
                .properties
                .get("path")
                .is_some_and(|value| value == path)
    })?;
    let crate_id = source_file.crate_id.as_deref()?;
    let module = entities.iter().find(|entity| {
        entity.kind == EntityKind::RustModule
            && entity
                .properties
                .get("source_file_id")
                .is_some_and(|value| value == &source_file.id)
    })?;
    Some((crate_id, module.module_path.as_deref()?))
}

fn source_entity_is_trait(source: &FrameworkKnowledge, entity_id: &str) -> bool {
    workspace_entities(source)
        .into_iter()
        .any(|entity| entity.id == entity_id && entity.kind == EntityKind::RustTrait)
}

fn final_symbol_name(symbol: &str) -> Result<String, CompilerIndexError> {
    let parsed = parse_symbol(symbol).map_err(|_| CompilerIndexError::ContractInvalid)?;
    parsed
        .descriptors
        .last()
        .map(|descriptor| descriptor.name.nfc().collect())
        .ok_or(CompilerIndexError::ContractInvalid)
}

fn normalized_range(
    occurrence: &Occurrence,
    artifact_sha256: &str,
) -> Result<Vec<u32>, CompilerIndexError> {
    let typed = occurrence.typed_range.as_ref().map(typed_range);
    normalize_range_pair(&occurrence.range, typed.as_deref(), artifact_sha256)
}

fn normalized_enclosing_range(
    occurrence: &Occurrence,
    artifact_sha256: &str,
) -> Result<Vec<u32>, CompilerIndexError> {
    let typed = occurrence
        .typed_enclosing_range
        .as_ref()
        .map(typed_enclosing_range);
    normalize_range_pair(
        &occurrence.enclosing_range,
        typed.as_deref(),
        artifact_sha256,
    )
}

fn normalize_range_pair(
    deprecated: &[i32],
    typed: Option<&[i32]>,
    artifact_sha256: &str,
) -> Result<Vec<u32>, CompilerIndexError> {
    if deprecated.is_empty() && typed.is_none() {
        return Err(malformed(artifact_sha256, "source_range_missing"));
    }
    if let Some(typed) = typed
        && !deprecated.is_empty()
        && deprecated != typed
    {
        return Err(malformed(artifact_sha256, "typed_range_disagrees"));
    }
    let selected = typed.unwrap_or(deprecated);
    let range = selected
        .iter()
        .map(|value| {
            u32::try_from(*value).map_err(|_| malformed(artifact_sha256, "negative_range"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if !matches!(range.len(), 3 | 4)
        || range.len() == 3 && range[1] >= range[2]
        || range.len() == 4 && (range[0], range[1]) >= (range[2], range[3])
    {
        return Err(malformed(artifact_sha256, "invalid_range_order"));
    }
    Ok(range)
}

fn typed_range(value: &Typed_range) -> Vec<i32> {
    match value {
        Typed_range::SingleLineRange(value) => {
            vec![value.line, value.start_character, value.end_character]
        }
        Typed_range::MultiLineRange(value) => vec![
            value.start_line,
            value.start_character,
            value.end_line,
            value.end_character,
        ],
        _ => Vec::new(),
    }
}

fn typed_enclosing_range(value: &Typed_enclosing_range) -> Vec<i32> {
    match value {
        Typed_enclosing_range::SingleLineEnclosingRange(value) => {
            vec![value.line, value.start_character, value.end_character]
        }
        Typed_enclosing_range::MultiLineEnclosingRange(value) => vec![
            value.start_line,
            value.start_character,
            value.end_line,
            value.end_character,
        ],
        _ => Vec::new(),
    }
}

fn deprecated_range(occurrence: &Occurrence) -> Vec<u32> {
    occurrence
        .range
        .iter()
        .filter_map(|value| u32::try_from(*value).ok())
        .collect()
}

fn deprecated_enclosing_range(occurrence: &Occurrence) -> Vec<u32> {
    occurrence
        .enclosing_range
        .iter()
        .filter_map(|value| u32::try_from(*value).ok())
        .collect()
}

fn range_contains(outer: &[u32], inner: &[u32]) -> bool {
    let Some((outer_start, outer_end)) = range_positions(outer) else {
        return false;
    };
    let Some((inner_start, inner_end)) = range_positions(inner) else {
        return false;
    };
    outer_start <= inner_start && inner_end <= outer_end
}

fn range_positions(range: &[u32]) -> Option<((u32, u32), (u32, u32))> {
    match range {
        [line, start, end] => Some(((*line, *start), (*line, *end))),
        [start_line, start, end_line, end] => Some(((*start_line, *start), (*end_line, *end))),
        _ => None,
    }
}

fn range_to_bytes(source: &[u8], range: &[u32]) -> Option<(u64, u64)> {
    let text = std::str::from_utf8(source).ok()?;
    let starts = line_starts(source);
    let ((start_line, start_character), (end_line, end_character)) = range_positions(range)?;
    let start = position_to_byte(text, &starts, start_line, start_character)?;
    let end = position_to_byte(text, &starts, end_line, end_character)?;
    if start >= end {
        return None;
    }
    Some((u64::try_from(start).ok()?, u64::try_from(end).ok()?))
}

fn line_starts(source: &[u8]) -> Vec<usize> {
    let mut starts = vec![0];
    for (index, byte) in source.iter().enumerate() {
        if *byte == b'\n' {
            starts.push(index + 1);
        }
    }
    starts
}

fn position_to_byte(text: &str, starts: &[usize], line: u32, character: u32) -> Option<usize> {
    let line = usize::try_from(line).ok()?;
    let character = usize::try_from(character).ok()?;
    let start = *starts.get(line)?;
    let line_end = starts
        .get(line + 1)
        .copied()
        .unwrap_or(text.len())
        .saturating_sub(usize::from(line + 1 < starts.len()));
    let position = start.checked_add(character)?;
    (position <= line_end && text.is_char_boundary(position)).then_some(position)
}

fn relationship_flags(relationship: &scip::types::Relationship) -> Vec<String> {
    [
        (relationship.is_reference, "is_reference"),
        (relationship.is_implementation, "is_implementation"),
        (relationship.is_type_definition, "is_type_definition"),
        (relationship.is_definition, "is_definition"),
    ]
    .into_iter()
    .filter(|(enabled, _)| *enabled)
    .map(|(_, name)| name.to_owned())
    .collect()
}

fn compiler_evidence_id(artifact_sha256: &str, locator: &Value) -> String {
    let payload = Value::Array(vec![
        Value::String(COMPILER_EVIDENCE_DOMAIN.to_owned()),
        Value::String(artifact_sha256.to_owned()),
        locator.clone(),
    ]);
    format!(
        "urn:codenoesis:evidence:sha256:{}",
        sha256(&serde_json::to_vec(&payload).expect("JSON value serialization cannot fail"))
    )
}

fn source_evidence_id(path: &str, start: u64, end: u64, source_sha256: &str) -> String {
    let payload = Value::Array(vec![
        Value::String(SOURCE_EVIDENCE_DOMAIN.to_owned()),
        Value::String(path.to_owned()),
        Value::Number(start.into()),
        Value::Number(end.into()),
        Value::String(source_sha256.to_owned()),
    ]);
    format!(
        "urn:codenoesis:evidence:sha256:{}",
        sha256(&serde_json::to_vec(&payload).expect("JSON value serialization cannot fail"))
    )
}

fn compare_digest(
    subject: CompilerIndexMismatchSubject,
    expected: &str,
    observed: &str,
) -> Result<(), CompilerIndexError> {
    if expected == observed {
        Ok(())
    } else {
        Err(CompilerIndexError::BindingMismatch {
            subject,
            expected_sha256: expected.to_owned(),
            observed_sha256: observed.to_owned(),
        })
    }
}

fn object_value<const N: usize>(members: [(&str, Value); N]) -> Value {
    Value::Object(
        members
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}

fn u32_array(values: &[u32]) -> Value {
    Value::Array(
        values
            .iter()
            .map(|value| Value::Number(u64::from(*value).into()))
            .collect(),
    )
}

fn sort_unique_by<T, F>(values: &mut [T], key: F) -> Result<(), CompilerIndexError>
where
    F: Fn(&T) -> &String,
{
    values.sort_by(|left, right| key(left).as_bytes().cmp(key(right).as_bytes()));
    if values.windows(2).any(|pair| key(&pair[0]) == key(&pair[1])) {
        Err(CompilerIndexError::ContractInvalid)
    } else {
        Ok(())
    }
}

fn ordered_unique<'a>(values: impl IntoIterator<Item = &'a str>) -> bool {
    let mut previous = None;
    for value in values {
        if previous.is_some_and(|previous| previous >= value) {
            return false;
        }
        previous = Some(value);
    }
    true
}

fn malformed(artifact_sha256: &str, reason: &str) -> CompilerIndexError {
    CompilerIndexError::MalformedArtifact {
        artifact_sha256: artifact_sha256.to_owned(),
        reason: reason.to_owned(),
    }
}

fn unresolvable(evidence_id: &str) -> CompilerIndexError {
    CompilerIndexError::UnresolvableEvidence {
        evidence_id: evidence_id.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::Path;

    use codenoesis_domain::s4_r7::{
        CompilerIndexError, R7_DETERMINISM_PERMUTATIONS, compiler_symbol_id,
    };
    use protobuf::Message as _;
    use scip::types::Index;

    use super::{
        add_explicit_relationships, collect_symbol_seeds, compiler_evidence_id,
        private_value_is_promoted, symbol_identity_preimage, symbol_locator,
    };

    const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s4-compiler-index-v1";
    const ARTIFACT_SHA256: &str =
        "e1d3b4ca3c55b1a2779f7bea644fddc9557ddd30417fe8e4cf589e4089153a92";

    #[test]
    fn pt_dr_idn_001_r7_global_local_symbol_identity_nfc() {
        let (_, global) = symbol_identity_preimage(
            "rust-analyzer cargo api 0.1.0 api/`Café`#",
            "urn:codenoesis:fixture:s4-compiler-index-v1",
            Some("crates/api/src/lib.rs"),
        )
        .expect("parse canonical global symbol");
        assert_eq!(
            compiler_symbol_id(&global),
            "urn:codenoesis:entity:blake3:4d211f47aa0fc35bf70bf4331db4c85d28936af2ccefd64aa6f19720c08d2e82"
        );

        let (_, local) = symbol_identity_preimage(
            "local 0",
            "urn:codenoesis:fixture:s4-compiler-index-v1",
            Some("crates/client/src/lib.rs"),
        )
        .expect("parse canonical local symbol");
        assert_eq!(
            compiler_symbol_id(&local),
            "urn:codenoesis:entity:blake3:21de437376eaad16b6a9b6bfa96cf71f9d2db69f80daa470432f1094642dd712"
        );
    }

    #[test]
    fn pt_fr_ext_005_input_permutations_preserve_identity_projection() {
        let bytes = fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/s4/compiler-index-v1/index.scip"),
        )
        .expect("read reviewed R7 SCIP fixture");
        let index = Index::parse_from_bytes(&bytes).expect("decode reviewed R7 SCIP fixture");
        let baseline = identity_projection(&index, 0).expect("project baseline R7 identities");

        for permutation in 0..R7_DETERMINISM_PERMUTATIONS {
            let mut permuted = index.clone();
            permute_index(
                &mut permuted,
                usize::try_from(permutation).expect("R7 permutation index"),
            );
            let projection = identity_projection(
                &permuted,
                usize::try_from(permutation).expect("R7 schedule index"),
            )
            .unwrap_or_else(|error| panic!("project R7 permutation {permutation}: {error}"));
            assert_eq!(
                projection, baseline,
                "R7 input/schedule permutation {permutation} changed identities"
            );
        }

        let replay = Index::parse_from_bytes(&bytes).expect("decode isolated R7 replay");
        assert_eq!(
            identity_projection(&replay, 0).expect("project isolated R7 replay"),
            baseline
        );
    }

    #[test]
    fn sec_nfr_prv_002_r7_private_canary_cannot_be_embedded() {
        let private = ["R7_SECRET_ARGUMENT_CANARY".to_owned(), ".".to_owned()]
            .into_iter()
            .collect();
        assert!(private_value_is_promoted(
            &private,
            "prefix-R7_SECRET_ARGUMENT_CANARY-suffix"
        ));
        assert!(!private_value_is_promoted(&private, "compiler.symbol"));
    }

    #[derive(Debug, Eq, PartialEq)]
    struct IdentityProjection {
        symbols: Vec<(String, String, Vec<String>, String)>,
        relationships: Vec<(String, String, String, Vec<String>)>,
        evidence_ids: Vec<String>,
    }

    fn identity_projection(
        index: &Index,
        schedule: usize,
    ) -> Result<IdentityProjection, CompilerIndexError> {
        let seeds = collect_symbol_seeds(index, ARTIFACT_SHA256)?;
        let mut scheduled = seeds.iter().collect::<Vec<_>>();
        if !scheduled.is_empty() {
            let length = scheduled.len();
            scheduled.rotate_left(schedule % length);
        }

        let mut symbols = Vec::with_capacity(scheduled.len());
        let mut symbol_ids = BTreeMap::new();
        let mut evidence_ids = Vec::with_capacity(scheduled.len());
        for (raw_symbol, seed) in scheduled {
            let (_, preimage) =
                symbol_identity_preimage(raw_symbol, REPOSITORY_ID, seed.document_path.as_deref())?;
            let symbol_id = compiler_symbol_id(&preimage);
            let (locator, _) = symbol_locator(raw_symbol, seed)?;
            let evidence_id = compiler_evidence_id(ARTIFACT_SHA256, &locator);
            symbol_ids.insert(raw_symbol.clone(), symbol_id.clone());
            evidence_ids.push(evidence_id.clone());
            symbols.push((raw_symbol.clone(), symbol_id, preimage, evidence_id));
        }

        let mut relationship_evidence = Vec::new();
        let mut relationships = Vec::new();
        add_explicit_relationships(
            &seeds,
            &symbol_ids,
            ARTIFACT_SHA256,
            &mut relationship_evidence,
            &mut relationships,
        )?;
        evidence_ids.extend(
            relationship_evidence
                .into_iter()
                .map(|evidence| evidence.id),
        );

        symbols.sort_by(|left, right| left.1.cmp(&right.1));
        relationships.sort_by(|left, right| left.id.cmp(&right.id));
        evidence_ids.sort();
        Ok(IdentityProjection {
            symbols,
            relationships: relationships
                .into_iter()
                .map(|relationship| {
                    (
                        relationship.id,
                        relationship.source,
                        relationship.target,
                        relationship.evidence_ids,
                    )
                })
                .collect(),
            evidence_ids,
        })
    }

    fn permute_index(index: &mut Index, permutation: usize) {
        rotate(&mut index.documents, permutation);
        rotate(&mut index.external_symbols, permutation.saturating_mul(3));
        for (document_index, document) in index.documents.iter_mut().enumerate() {
            let seed = permutation.saturating_add(document_index);
            rotate(&mut document.occurrences, seed);
            rotate(&mut document.symbols, seed.saturating_mul(3));
            for (symbol_index, information) in document.symbols.iter_mut().enumerate() {
                rotate(
                    &mut information.relationships,
                    seed.saturating_add(symbol_index),
                );
            }
        }
        for (symbol_index, information) in index.external_symbols.iter_mut().enumerate() {
            rotate(
                &mut information.relationships,
                permutation.saturating_add(symbol_index),
            );
        }
    }

    fn rotate<T>(values: &mut [T], amount: usize) {
        if !values.is_empty() {
            values.rotate_left(amount % values.len());
        }
    }
}
