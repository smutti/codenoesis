use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::knowledge::{ClaimState, ClaimSubjectKind, EntityKind};
use crate::s4::WorkspaceClaim;
use crate::s4_k1::{CallableRelationshipKind, CallableSemanticsError, CallableSemanticsKnowledge};
use crate::s4_r5::RustSemanticEntityKind;
use crate::s4_r7::{
    CompilerBindingState, CompilerIndexError, CompilerIndexOverlay, CompilerSymbol,
};

pub const R13_CONFIGURATION_VERSION: &str = "codenoesis.configuration/v12";
pub const R13_SNAPSHOT_VERSION: &str = "codenoesis.repository-snapshot/v15";
pub const R13_EXTRACTION_CHUNK_VERSION: &str = "codenoesis.extraction-chunk/v12";
pub const R13_GRAPH_VERSION: &str = "codenoesis.knowledge-graph/v12";
pub const R13_ONTOLOGY_VERSION: &str = "codenoesis.ontology/rust/v12";
pub const R13_PIPELINE_VERSION: &str = "codenoesis.pipeline/s4-r13-v1";
pub const R13_EXTRACTION_CONTRACT_VERSION: &str = "codenoesis.extraction/v12";
pub const R13_COMPOSITION_VERSION: &str = "codenoesis.rust-callable-scip-composition/s4-r13-v1";
pub const R13_INDEX_VERSION: &str = "codenoesis.callable-compiler-join-index/v1";
pub const R13_SEMANTIC_HASH_CONTRACT_VERSION: &str = "codenoesis.semantic-hash-contract/v11";
pub const R13_ERROR_VERSION: &str = "codenoesis.error/v20";
pub const R13_QUERY_VERSION: &str = "codenoesis.local-query-result/v10";
pub const R13_PORTABLE_GRAPH_VERSION: &str = "codenoesis.portable-graph/v6";
pub const R13_LOCAL_EXPLORER_VERSION: &str = "codenoesis.local-explorer/v6";
pub const R13_RELATIONSHIP_KIND: &str = "HAS_COMPILER_SYMBOL";
pub const MAX_R13_CALLABLE_COMPILER_JOINS: u64 = 200_000;

const RELATIONSHIP_ID_DOMAIN: &str = "codenoesis.relationship-id/rust-callable-scip-composition/v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableCompilerJoin {
    pub source_callable_id: String,
    pub signature_id: String,
    pub compiler_symbol_id: String,
    pub relationship_id: String,
    pub evidence_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableCompilerJoinIndex {
    pub joins: Vec<CallableCompilerJoin>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableScipKnowledge {
    pub callable: CallableSemanticsKnowledge,
    pub compiler: CompilerIndexOverlay,
    pub index: CallableCompilerJoinIndex,
    pub claims: Vec<WorkspaceClaim>,
}

impl CallableScipKnowledge {
    /// Composes validated K1 and R7 facts without changing either lineage.
    ///
    /// # Errors
    ///
    /// Returns the first inherited, cardinality, evidence, identity, or limit failure.
    pub fn compose(
        callable: CallableSemanticsKnowledge,
        compiler: CompilerIndexOverlay,
    ) -> Result<Self, CallableScipCompositionError> {
        callable
            .validate()
            .map_err(CallableScipCompositionError::Callable)?;
        compiler
            .validate(&callable.framework)
            .map_err(CallableScipCompositionError::Compiler)?;
        let index = expected_index(&callable, &compiler)?;
        let claims = index
            .joins
            .iter()
            .map(|join| {
                WorkspaceClaim::new(
                    ClaimSubjectKind::Relationship,
                    join.relationship_id.clone(),
                    ClaimState::DeterministicFact,
                    join.evidence_ids.clone(),
                )
            })
            .collect();
        let knowledge = Self {
            callable,
            compiler,
            index,
            claims,
        };
        knowledge.validate()?;
        Ok(knowledge)
    }

    /// Validates the exact callable/compiler correspondence projection.
    ///
    /// # Errors
    ///
    /// Returns the first inherited or cross-lineage contract failure.
    pub fn validate(&self) -> Result<(), CallableScipCompositionError> {
        self.callable
            .validate()
            .map_err(CallableScipCompositionError::Callable)?;
        self.compiler
            .validate(&self.callable.framework)
            .map_err(CallableScipCompositionError::Compiler)?;
        let expected = expected_index(&self.callable, &self.compiler)?;
        if self.index != expected {
            return Err(CallableScipCompositionError::ContractInvalid);
        }
        let expected_claims = expected
            .joins
            .iter()
            .map(|join| {
                WorkspaceClaim::new(
                    ClaimSubjectKind::Relationship,
                    join.relationship_id.clone(),
                    ClaimState::DeterministicFact,
                    join.evidence_ids.clone(),
                )
            })
            .collect::<Vec<_>>();
        if self.claims != expected_claims {
            return Err(CallableScipCompositionError::ContractInvalid);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableScipCompositionError {
    Callable(CallableSemanticsError),
    Compiler(CompilerIndexError),
    SignatureCardinality { callable_id: String, observed: u64 },
    DuplicateCompilerOwnership { callable_id: String, observed: u64 },
    InvalidJoinEvidence { callable_id: String },
    LimitExceeded { maximum: u64, observed: u64 },
    ContractInvalid,
}

impl Display for CallableScipCompositionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Callable(_) => "R13 callable extraction is invalid",
            Self::Compiler(_) => "R13 compiler overlay is invalid",
            Self::SignatureCardinality { .. } => "R13 callable signature cardinality is invalid",
            Self::DuplicateCompilerOwnership { .. } => "R13 callable has multiple compiler symbols",
            Self::InvalidJoinEvidence { .. } => "R13 callable/compiler join evidence is invalid",
            Self::LimitExceeded { .. } => "R13 callable/compiler join limit exceeded",
            Self::ContractInvalid => "R13 callable/compiler composition is invalid",
        })
    }
}

impl Error for CallableScipCompositionError {}

#[must_use]
pub fn callable_compiler_relationship_id(
    source_callable_id: &str,
    compiler_symbol_id: &str,
) -> String {
    stable_id(
        "urn:codenoesis:relationship:blake3:",
        &[
            RELATIONSHIP_ID_DOMAIN,
            R13_RELATIONSHIP_KIND,
            source_callable_id,
            compiler_symbol_id,
        ],
    )
}

fn expected_index(
    callable: &CallableSemanticsKnowledge,
    compiler: &CompilerIndexOverlay,
) -> Result<CallableCompilerJoinIndex, CallableScipCompositionError> {
    let mut signatures = BTreeMap::<&str, Vec<&str>>::new();
    for relationship in &callable.graph.relationships {
        if relationship.kind == CallableRelationshipKind::HasSignature {
            signatures
                .entry(relationship.source.as_str())
                .or_default()
                .push(relationship.target.as_str());
        }
    }

    let mut owned = BTreeMap::<&str, Vec<&CompilerSymbol>>::new();
    for symbol in &compiler.symbols {
        if symbol.binding_state != CompilerBindingState::InRepositoryBound {
            continue;
        }
        let Some(source_id) = symbol.source_entity_id.as_deref() else {
            continue;
        };
        if callable_source_kind(callable, source_id).is_some() {
            owned.entry(source_id).or_default().push(symbol);
        }
    }

    let mut joins = Vec::with_capacity(owned.len());
    for (source_callable_id, symbols) in owned {
        let [symbol] = symbols.as_slice() else {
            return Err(CallableScipCompositionError::DuplicateCompilerOwnership {
                callable_id: source_callable_id.to_owned(),
                observed: u64::try_from(symbols.len()).unwrap_or(u64::MAX),
            });
        };
        let signature_values = signatures
            .get(source_callable_id)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let [signature_id] = signature_values else {
            return Err(CallableScipCompositionError::SignatureCardinality {
                callable_id: source_callable_id.to_owned(),
                observed: u64::try_from(signature_values.len()).unwrap_or(u64::MAX),
            });
        };
        let ([source_evidence_id], [compiler_evidence_id]) = (
            symbol.source_evidence_ids.as_slice(),
            symbol.compiler_evidence_ids.as_slice(),
        ) else {
            return Err(CallableScipCompositionError::InvalidJoinEvidence {
                callable_id: source_callable_id.to_owned(),
            });
        };
        let mut evidence_ids = vec![source_evidence_id.clone(), compiler_evidence_id.clone()];
        evidence_ids.sort();
        joins.push(CallableCompilerJoin {
            source_callable_id: source_callable_id.to_owned(),
            signature_id: (*signature_id).to_owned(),
            compiler_symbol_id: symbol.id.clone(),
            relationship_id: callable_compiler_relationship_id(source_callable_id, &symbol.id),
            evidence_ids,
        });
    }
    joins.sort_by(|left, right| left.source_callable_id.cmp(&right.source_callable_id));
    let observed = u64::try_from(joins.len()).unwrap_or(u64::MAX);
    enforce_join_limit(observed)?;
    if joins
        .iter()
        .map(|join| join.relationship_id.as_str())
        .collect::<BTreeSet<_>>()
        .len()
        != joins.len()
        || joins
            .iter()
            .flat_map(|join| join.evidence_ids.iter())
            .any(|identifier| !valid_sha256_evidence_id(identifier))
    {
        return Err(CallableScipCompositionError::ContractInvalid);
    }
    Ok(CallableCompilerJoinIndex { joins })
}

fn enforce_join_limit(observed: u64) -> Result<(), CallableScipCompositionError> {
    if observed > MAX_R13_CALLABLE_COMPILER_JOINS {
        return Err(CallableScipCompositionError::LimitExceeded {
            maximum: MAX_R13_CALLABLE_COMPILER_JOINS,
            observed,
        });
    }
    Ok(())
}

fn callable_source_kind<'a>(
    callable: &'a CallableSemanticsKnowledge,
    source_id: &str,
) -> Option<&'a str> {
    let mut workspace_entities = callable
        .framework
        .semantic
        .manifest
        .workspace
        .knowledge
        .graph
        .entities
        .iter()
        .chain(callable.framework.semantic.graph.legacy_entities.iter());
    if let Some(entity) = workspace_entities.find(|entity| entity.id == source_id) {
        return match entity.kind {
            EntityKind::RustFunction => Some("rust.function"),
            EntityKind::RustMethod => Some("rust.method"),
            EntityKind::RustCrate
            | EntityKind::SourceFile
            | EntityKind::RustModule
            | EntityKind::RustStruct
            | EntityKind::RustEnum
            | EntityKind::RustTrait
            | EntityKind::RustTypeAlias
            | EntityKind::RustSymbolReference => None,
        };
    }
    callable
        .framework
        .semantic
        .graph
        .entities
        .iter()
        .find(|entity| entity.id == source_id && entity.kind == RustSemanticEntityKind::Method)
        .map(|_| "rust.method")
}

fn valid_sha256_evidence_id(value: &str) -> bool {
    value
        .strip_prefix("urn:codenoesis:evidence:sha256:")
        .is_some_and(|digest| {
            digest.len() == 64
                && digest
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        })
}

fn stable_id(prefix: &str, components: &[&str]) -> String {
    format!(
        "{prefix}{}",
        blake3::hash(&canonical_string_array(components)).to_hex()
    )
}

fn canonical_string_array(components: &[&str]) -> Vec<u8> {
    let mut bytes = vec![b'['];
    for (index, component) in components.iter().enumerate() {
        if index > 0 {
            bytes.push(b',');
        }
        write_json_string(&mut bytes, component);
    }
    bytes.push(b']');
    bytes
}

fn write_json_string(bytes: &mut Vec<u8>, value: &str) {
    bytes.push(b'"');
    for character in value.chars() {
        match character {
            '"' => bytes.extend_from_slice(br#"\""#),
            '\\' => bytes.extend_from_slice(br"\\"),
            '\u{08}' => bytes.extend_from_slice(br"\b"),
            '\u{0c}' => bytes.extend_from_slice(br"\f"),
            '\n' => bytes.extend_from_slice(br"\n"),
            '\r' => bytes.extend_from_slice(br"\r"),
            '\t' => bytes.extend_from_slice(br"\t"),
            character if character <= '\u{1f}' => {
                use std::fmt::Write as _;
                write!(StringByteWriter(bytes), "\\u{:04x}", character as u32)
                    .expect("writing to a byte vector cannot fail");
            }
            character => {
                let mut encoded = [0_u8; 4];
                bytes.extend_from_slice(character.encode_utf8(&mut encoded).as_bytes());
            }
        }
    }
    bytes.push(b'"');
}

struct StringByteWriter<'a>(&'a mut Vec<u8>);

impl fmt::Write for StringByteWriter<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.0.extend_from_slice(value.as_bytes());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CallableScipCompositionError, MAX_R13_CALLABLE_COMPILER_JOINS, enforce_join_limit,
    };

    #[test]
    fn pt_fr_ext_015_join_limit_has_exact_maximum_and_plus_one() {
        assert_eq!(enforce_join_limit(MAX_R13_CALLABLE_COMPILER_JOINS), Ok(()));
        assert_eq!(
            enforce_join_limit(MAX_R13_CALLABLE_COMPILER_JOINS + 1),
            Err(CallableScipCompositionError::LimitExceeded {
                maximum: MAX_R13_CALLABLE_COMPILER_JOINS,
                observed: MAX_R13_CALLABLE_COMPILER_JOINS + 1,
            })
        );
    }
}
