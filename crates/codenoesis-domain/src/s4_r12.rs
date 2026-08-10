use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::s4::WorkspaceEvidence;
use crate::s4_k1::{
    CallableRelationshipKind, CallableSemanticEntityKind, CallableSemanticProperties,
    CallableSemanticsError, CallableSemanticsExtraction, CallableSemanticsKnowledge,
};
use crate::s4_r10::{
    RustCfgDeclarationAlternativesError, RustCfgDeclarationAlternativesExtraction,
    RustCfgDeclarationAlternativesKnowledge,
};
use crate::s5::{AnalysisCacheEntry, SourceAnalysisRecord};

pub const R12_CONFIGURATION_VERSION: &str = "codenoesis.configuration/v11";
pub const R12_SNAPSHOT_VERSION: &str = "codenoesis.repository-snapshot/v14";
pub const R12_EXTRACTION_CHUNK_VERSION: &str = "codenoesis.extraction-chunk/v11";
pub const R12_GRAPH_VERSION: &str = "codenoesis.knowledge-graph/v11";
pub const R12_ONTOLOGY_VERSION: &str = "codenoesis.ontology/rust/v11";
pub const R12_PIPELINE_VERSION: &str = "codenoesis.pipeline/s4-r12-v1";
pub const R12_EXTRACTION_CONTRACT_VERSION: &str = "codenoesis.extraction/v11";
pub const R12_COMPOSITION_VERSION: &str =
    "codenoesis.rust-callable-cfg-alternatives-composition/s4-r12-v1";
pub const R12_EXTRACTOR_VERSION: &str = "codenoesis.rust-callable-cfg-alternatives/s4-r12-v1";
pub const R12_INDEX_VERSION: &str = "codenoesis.callable-cfg-alternatives-index/v1";
pub const R12_SEMANTIC_HASH_CONTRACT_VERSION: &str = "codenoesis.semantic-hash-contract/v10";
pub const R12_ERROR_VERSION: &str = "codenoesis.error/v19";
pub const R12_QUERY_VERSION: &str = "codenoesis.local-query-result/v9";
pub const R12_PORTABLE_GRAPH_VERSION: &str = "codenoesis.portable-graph/v5";
pub const R12_LOCAL_EXPLORER_VERSION: &str = "codenoesis.local-explorer/v5";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableCfgAlternativesIndex {
    pub logical_method_ids: Vec<String>,
    pub alternative_callable_subject_ids: Vec<String>,
    pub signature_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableCfgAlternativesKnowledge {
    pub alternatives: RustCfgDeclarationAlternativesKnowledge,
    pub callable: CallableSemanticsKnowledge,
    pub index: CallableCfgAlternativesIndex,
}

impl CallableCfgAlternativesKnowledge {
    /// Validates the complete R10 + R6 + K1 composition without selecting a cfg world.
    ///
    /// # Errors
    ///
    /// Returns the first inherited or cross-layer identity/evidence failure.
    #[allow(clippy::too_many_lines)]
    pub fn validate(&self) -> Result<(), CallableCfgAlternativesError> {
        self.alternatives
            .validate()
            .map_err(CallableCfgAlternativesError::Alternatives)?;
        if self.callable.framework.semantic != self.alternatives.semantic {
            return Err(CallableCfgAlternativesError::ContractInvalid);
        }

        let logical_method_ids = self
            .alternatives
            .graph
            .index
            .logical_method_ids
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let alternative_ids = self
            .alternatives
            .graph
            .alternatives
            .iter()
            .map(|alternative| alternative.id.clone())
            .collect::<BTreeSet<_>>();
        if self.callable.graph.entities.iter().any(|entity| {
            logical_method_ids.contains(&entity.subject_id)
                && matches!(
                    entity.kind,
                    CallableSemanticEntityKind::Signature
                        | CallableSemanticEntityKind::Parameter
                        | CallableSemanticEntityKind::LocalBinding
                        | CallableSemanticEntityKind::CallSite
                        | CallableSemanticEntityKind::Control
                )
        }) || self
            .callable
            .graph
            .relationships
            .iter()
            .any(|relationship| {
                logical_method_ids.contains(&relationship.source)
                    && matches!(
                        relationship.kind,
                        CallableRelationshipKind::HasSignature
                            | CallableRelationshipKind::HasBodyFact
                            | CallableRelationshipKind::Calls
                    )
            })
        {
            return Err(CallableCfgAlternativesError::LogicalMethodHasOccurrenceShape);
        }

        let signatures = self
            .callable
            .graph
            .entities
            .iter()
            .filter(|entity| entity.kind == CallableSemanticEntityKind::Signature)
            .fold(BTreeMap::<&str, Vec<&str>>::new(), |mut values, entity| {
                values
                    .entry(entity.subject_id.as_str())
                    .or_default()
                    .push(entity.id.as_str());
                values
            });
        let alternative_evidence = self
            .alternatives
            .semantic
            .graph
            .evidence
            .iter()
            .map(|evidence| (evidence.id.as_str(), evidence))
            .collect::<BTreeMap<_, _>>();
        let callable_evidence = self
            .callable
            .graph
            .evidence
            .iter()
            .map(|evidence| (evidence.id.as_str(), evidence))
            .collect::<BTreeMap<_, _>>();
        for signature in self
            .callable
            .graph
            .entities
            .iter()
            .filter(|entity| entity.kind == CallableSemanticEntityKind::Signature)
        {
            if alternative_ids.contains(&signature.subject_id) {
                continue;
            }
            let spans = signature
                .evidence_ids
                .iter()
                .filter_map(|identifier| callable_evidence.get(identifier.as_str()).copied())
                .collect::<Vec<_>>();
            if let Some(alternative) = self.alternatives.graph.alternatives.iter().find(|value| {
                alternative_evidence
                    .get(value.properties.declaration_evidence_id.as_str())
                    .is_some_and(|declaration| {
                        !spans.is_empty()
                            && spans
                                .iter()
                                .all(|evidence| evidence_within(evidence, declaration))
                            && spans.iter().map(|evidence| evidence.start_byte).min()
                                == Some(declaration.start_byte)
                            && spans.iter().map(|evidence| evidence.end_byte).max()
                                == Some(declaration.end_byte)
                    })
            }) {
                return Err(CallableCfgAlternativesError::AlternativeSubjectMismatch {
                    alternative_id: alternative.id.clone(),
                    observed_subject_id: signature.subject_id.clone(),
                });
            }
        }
        let mut expected_signature_ids = Vec::with_capacity(alternative_ids.len());
        for alternative in &self.alternatives.graph.alternatives {
            let Some([signature_id]) = signatures.get(alternative.id.as_str()).map(Vec::as_slice)
            else {
                let observed = signatures.get(alternative.id.as_str()).map_or(0, Vec::len);
                return Err(
                    CallableCfgAlternativesError::AlternativeSignatureCardinality {
                        alternative_id: alternative.id.clone(),
                        observed: u64::try_from(observed).unwrap_or(u64::MAX),
                    },
                );
            };
            expected_signature_ids.push((*signature_id).to_owned());
        }

        self.callable
            .validate_with_additional_subjects(&alternative_ids)
            .map_err(CallableCfgAlternativesError::Callable)?;

        for (alternative, signature_id) in self
            .alternatives
            .graph
            .alternatives
            .iter()
            .zip(expected_signature_ids.iter())
        {
            let signature = self
                .callable
                .graph
                .entities
                .iter()
                .find(|entity| entity.id == *signature_id)
                .ok_or(CallableCfgAlternativesError::ContractInvalid)?;
            let declaration = alternative_evidence
                .get(alternative.properties.declaration_evidence_id.as_str())
                .ok_or(CallableCfgAlternativesError::ContractInvalid)?;
            validate_occurrence_evidence(signature, declaration, &callable_evidence)?;
            for entity in self
                .callable
                .graph
                .entities
                .iter()
                .filter(|entity| entity.subject_id == alternative.id)
            {
                for evidence_id in &entity.evidence_ids {
                    let evidence = callable_evidence
                        .get(evidence_id.as_str())
                        .ok_or(CallableCfgAlternativesError::ContractInvalid)?;
                    if !evidence_within(evidence, declaration) {
                        return Err(CallableCfgAlternativesError::OccurrenceEvidenceMismatch {
                            alternative_id: alternative.id.clone(),
                        });
                    }
                }
            }
        }

        let expected = CallableCfgAlternativesIndex {
            logical_method_ids: self.alternatives.graph.index.logical_method_ids.clone(),
            alternative_callable_subject_ids: self
                .alternatives
                .graph
                .index
                .alternative_entity_ids
                .clone(),
            signature_ids: expected_signature_ids,
        };
        if self.index != expected {
            return Err(CallableCfgAlternativesError::ContractInvalid);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableCfgAlternativesExtraction {
    pub knowledge: CallableCfgAlternativesKnowledge,
    pub cache_entries: Vec<AnalysisCacheEntry>,
    pub source_records: Vec<SourceAnalysisRecord>,
    pub parser_invocation_count: u64,
}

impl CallableCfgAlternativesExtraction {
    /// Combines validated R10 occurrence authority with R6 and K1 facts.
    ///
    /// # Errors
    ///
    /// Returns a cross-layer contract failure when lineage or cache identity differs.
    pub fn compose(
        alternatives: RustCfgDeclarationAlternativesExtraction,
        callable: CallableSemanticsExtraction,
    ) -> Result<Self, CallableCfgAlternativesError> {
        if alternatives.knowledge.semantic != callable.knowledge.framework.semantic
            || alternatives.cache_entries != callable.cache_entries
        {
            return Err(CallableCfgAlternativesError::ContractInvalid);
        }
        let signature_by_subject = callable
            .knowledge
            .graph
            .entities
            .iter()
            .filter(|entity| entity.kind == CallableSemanticEntityKind::Signature)
            .map(|entity| (entity.subject_id.as_str(), entity.id.clone()))
            .collect::<BTreeMap<_, _>>();
        let index = CallableCfgAlternativesIndex {
            logical_method_ids: alternatives
                .knowledge
                .graph
                .index
                .logical_method_ids
                .clone(),
            alternative_callable_subject_ids: alternatives
                .knowledge
                .graph
                .index
                .alternative_entity_ids
                .clone(),
            signature_ids: alternatives
                .knowledge
                .graph
                .alternatives
                .iter()
                .map(|alternative| {
                    signature_by_subject
                        .get(alternative.id.as_str())
                        .cloned()
                        .ok_or_else(|| {
                            CallableCfgAlternativesError::AlternativeSignatureCardinality {
                                alternative_id: alternative.id.clone(),
                                observed: 0,
                            }
                        })
                })
                .collect::<Result<_, _>>()?,
        };
        let extraction = Self {
            cache_entries: callable.cache_entries,
            source_records: alternatives.source_records,
            parser_invocation_count: callable.parser_invocation_count,
            knowledge: CallableCfgAlternativesKnowledge {
                alternatives: alternatives.knowledge,
                callable: callable.knowledge,
                index,
            },
        };
        extraction.knowledge.validate()?;
        Ok(extraction)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableCfgAlternativesError {
    Alternatives(RustCfgDeclarationAlternativesError),
    Callable(CallableSemanticsError),
    LogicalMethodHasOccurrenceShape,
    AlternativeSubjectMismatch {
        alternative_id: String,
        observed_subject_id: String,
    },
    AlternativeSignatureCardinality {
        alternative_id: String,
        observed: u64,
    },
    OccurrenceEvidenceMismatch {
        alternative_id: String,
    },
    ContractInvalid,
}

impl Display for CallableCfgAlternativesError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Alternatives(_) => "R12 cfg-alternative extraction failed",
            Self::Callable(_) => "R12 callable extraction failed",
            Self::LogicalMethodHasOccurrenceShape => {
                "R12 logical cfg-alternative method has occurrence shape"
            }
            Self::AlternativeSubjectMismatch { .. } => {
                "R12 declaration alternative callable subject is invalid"
            }
            Self::AlternativeSignatureCardinality { .. } => {
                "R12 declaration alternative signature cardinality is invalid"
            }
            Self::OccurrenceEvidenceMismatch { .. } => {
                "R12 callable occurrence evidence does not match its alternative"
            }
            Self::ContractInvalid => "R12 callable cfg-alternatives contract is invalid",
        })
    }
}

impl Error for CallableCfgAlternativesError {}

fn validate_occurrence_evidence(
    signature: &crate::s4_k1::CallableSemanticEntity,
    declaration: &WorkspaceEvidence,
    evidence: &BTreeMap<&str, &WorkspaceEvidence>,
) -> Result<(), CallableCfgAlternativesError> {
    let CallableSemanticProperties::Signature(_) = &signature.properties else {
        return Err(CallableCfgAlternativesError::ContractInvalid);
    };
    let spans = signature
        .evidence_ids
        .iter()
        .map(|identifier| {
            evidence
                .get(identifier.as_str())
                .copied()
                .ok_or(CallableCfgAlternativesError::ContractInvalid)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if spans.is_empty()
        || spans
            .iter()
            .any(|value| !evidence_within(value, declaration))
        || spans.iter().map(|value| value.start_byte).min() != Some(declaration.start_byte)
        || spans.iter().map(|value| value.end_byte).max() != Some(declaration.end_byte)
    {
        return Err(CallableCfgAlternativesError::OccurrenceEvidenceMismatch {
            alternative_id: signature.subject_id.clone(),
        });
    }
    Ok(())
}

fn evidence_within(evidence: &WorkspaceEvidence, declaration: &WorkspaceEvidence) -> bool {
    evidence.path == declaration.path
        && evidence.blob_oid == declaration.blob_oid
        && evidence.start_byte >= declaration.start_byte
        && evidence.end_byte <= declaration.end_byte
        && evidence.start_byte < evidence.end_byte
}
