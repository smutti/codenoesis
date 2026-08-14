use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use codenoesis_domain::storage::LocalSnapshotHead;
use serde_json::{Value, json};

use super::s4_r16::{
    MAX_R16_PORTABLE_GRAPH_BYTES, PortableGraphV9, R16_GRAPH_VERSION, R16_ONTOLOGY_VERSION,
    R16_PORTABLE_GRAPH_VERSION, R16_SNAPSHOT_VERSION, R16ContractError, R16Sha256,
    validate_stored_snapshot_semantic_v18,
};

pub const R17_CONTEXT_PROFILE: &str = "rust-function-context-v1";
pub const R17_FUNCTION_CONTEXT_VERSION: &str = "codenoesis.function-context/v1";
pub const R17_LOCAL_EXPLORER_VERSION: &str = "codenoesis.local-explorer/v10";
pub const R17_EXPLORER_SECURITY_PROFILE: &str = "codenoesis.local-explorer-security/v10";
pub const R17_EXPLORER_MARKER: &str = ".codenoesis-local-explorer-v10";
pub const MAX_R17_CONTEXT_OUTPUT_BYTES: u64 = 4_194_304;
pub const MAX_R17_FUNCTION_PARAMETERS: u64 = 256;
pub const MAX_R17_LINKED_SUBJECTS: u64 = 256;
pub const MAX_R17_LINKED_RELATIONSHIPS: u64 = 512;
pub const MAX_R17_LINKED_CLAIMS: u64 = 2_048;
pub const MAX_R17_LINKED_EVIDENCE: u64 = 2_048;
pub const MAX_R17_UNCERTAINTY_RECORDS: u64 = 1_024;
pub const MAX_R17_FUNCTION_SEARCH_RESULTS: u64 = 100;
pub const MAX_R17_NAVIGATION_HISTORY: u64 = 128;

const LIMITATIONS: [&str; 8] = [
    "active_cfg_not_selected",
    "compiler_validity_not_proven",
    "declared_types_not_resolved",
    "dispatch_not_resolved",
    "ownership_and_aliasing_not_computed",
    "returned_values_not_proven",
    "runtime_behavior_not_observed",
    "side_effects_not_computed",
];

const PRIVACY_DENIED_FIELDS: [&str; 24] = [
    "absolute_path",
    "absolute_root",
    "argument",
    "arguments",
    "body_text",
    "condition_text",
    "credential",
    "credentials",
    "environment",
    "expression_text",
    "initializer_text",
    "literal_lexeme",
    "literal_text",
    "model_data",
    "raw_url",
    "repository_root",
    "source_contents",
    "source_snippet",
    "telemetry",
    "url",
    "body_source",
    "condition_source",
    "expression_source",
    "initializer_source",
];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FunctionContextCounts {
    pub parameters: u64,
    pub linked_subjects: u64,
    pub linked_relationships: u64,
    pub linked_claims: u64,
    pub linked_evidence: u64,
    pub uncertainty_records: u64,
}

/// Validates the bounded cardinalities of one function context.
///
/// # Errors
///
/// Returns [`FunctionContextError::LimitExceeded`] when any observed count is
/// greater than its reviewed R17 maximum.
pub fn validate_function_context_limits(
    counts: FunctionContextCounts,
) -> Result<(), FunctionContextError> {
    enforce_limit(
        "function_parameters",
        MAX_R17_FUNCTION_PARAMETERS,
        counts.parameters,
    )?;
    enforce_limit(
        "linked_subjects",
        MAX_R17_LINKED_SUBJECTS,
        counts.linked_subjects,
    )?;
    enforce_limit(
        "linked_relationships",
        MAX_R17_LINKED_RELATIONSHIPS,
        counts.linked_relationships,
    )?;
    enforce_limit("linked_claims", MAX_R17_LINKED_CLAIMS, counts.linked_claims)?;
    enforce_limit(
        "linked_evidence",
        MAX_R17_LINKED_EVIDENCE,
        counts.linked_evidence,
    )?;
    enforce_limit(
        "uncertainty_records",
        MAX_R17_UNCERTAINTY_RECORDS,
        counts.uncertainty_records,
    )
}

/// Validates the canonical context output size, including its trailing LF.
///
/// # Errors
///
/// Returns [`FunctionContextError::LimitExceeded`] above the 4 MiB contract.
pub fn validate_function_context_output_bytes(observed: u64) -> Result<(), FunctionContextError> {
    enforce_limit(
        "context_output_bytes_including_lf",
        MAX_R17_CONTEXT_OUTPUT_BYTES,
        observed,
    )
}

#[derive(Clone, Debug)]
pub struct FunctionContextV1 {
    value: Value,
}

impl FunctionContextV1 {
    /// Projects one exact callable from a validated `RepositorySnapshotV18`.
    ///
    /// # Errors
    ///
    /// Returns a typed [`FunctionContextError`] when the snapshot, callable,
    /// linked facts, evidence, privacy boundary, or cardinality is invalid.
    #[allow(clippy::too_many_lines)]
    pub fn from_validated_v18(
        semantic: &Value,
        head: &LocalSnapshotHead,
        requested_id: &str,
    ) -> Result<Self, FunctionContextError> {
        validate_stored_snapshot_semantic_v18(semantic, head)
            .map_err(|_| FunctionContextError::InvalidSnapshot)?;
        let graph = semantic
            .get("knowledge_graph")
            .and_then(Value::as_object)
            .ok_or(FunctionContextError::InvalidSnapshot)?;
        let entities = identified_map(graph.get("entities"))?;
        identified_map(graph.get("relationships"))?;
        let claim_map = identified_map(graph.get("claims"))?;
        let evidence_map = identified_map(graph.get("evidence"))?;
        let root = entities
            .get(requested_id)
            .copied()
            .ok_or(FunctionContextError::NotFound)?;
        if !matches!(
            required_string(root, "kind")?,
            "rust.function" | "rust.method"
        ) {
            return Err(FunctionContextError::InvalidRootKind);
        }

        let relationships = graph
            .get("relationships")
            .and_then(Value::as_array)
            .ok_or(FunctionContextError::InvalidSnapshot)?;
        let signature_relationships = relationships
            .iter()
            .filter(|relationship| {
                relationship.get("kind").and_then(Value::as_str) == Some("HAS_SIGNATURE")
                    && relationship.get("source").and_then(Value::as_str) == Some(requested_id)
            })
            .collect::<Vec<_>>();
        let signature_relationship = match signature_relationships.as_slice() {
            [] => return Err(FunctionContextError::MissingSignature),
            [relationship] => *relationship,
            _ => return Err(FunctionContextError::DuplicateSignature),
        };
        let signature_id = relationship_target(signature_relationship)?;
        let signature = entities
            .get(signature_id)
            .copied()
            .ok_or_else(|| FunctionContextError::DanglingReference(signature_id.to_owned()))?;
        if required_string(signature, "kind")? != "rust.callable_signature"
            || signature.get("subject_id").and_then(Value::as_str) != Some(requested_id)
        {
            return Err(FunctionContextError::InvalidRelationship);
        }

        let parameter_relationships = relationships
            .iter()
            .filter(|relationship| {
                relationship.get("kind").and_then(Value::as_str) == Some("HAS_PARAMETER")
                    && relationship.get("source").and_then(Value::as_str) == Some(signature_id)
            })
            .collect::<Vec<_>>();
        let mut parameters = Vec::with_capacity(parameter_relationships.len());
        let mut parameter_ids = BTreeSet::new();
        let mut parameter_ordinals = BTreeSet::new();
        for relationship in &parameter_relationships {
            let parameter_id = relationship_target(relationship)?;
            if !parameter_ids.insert(parameter_id.to_owned()) {
                return Err(FunctionContextError::InvalidRelationship);
            }
            let parameter = entities
                .get(parameter_id)
                .copied()
                .ok_or_else(|| FunctionContextError::DanglingReference(parameter_id.to_owned()))?;
            if required_string(parameter, "kind")? != "rust.parameter"
                || parameter.get("subject_id").and_then(Value::as_str) != Some(requested_id)
            {
                return Err(FunctionContextError::InvalidRelationship);
            }
            let ordinal = parameter
                .get("ordinal")
                .and_then(Value::as_u64)
                .ok_or(FunctionContextError::InvalidParameterOrdinal)?;
            if !parameter_ordinals.insert(ordinal) {
                return Err(FunctionContextError::InvalidParameterOrdinal);
            }
            parameters.push(parameter.clone());
        }
        parameters.sort_by_key(|parameter| parameter.get("ordinal").and_then(Value::as_u64));
        if parameters.iter().enumerate().any(|(index, parameter)| {
            parameter.get("ordinal").and_then(Value::as_u64) != u64::try_from(index).ok()
        }) {
            return Err(FunctionContextError::InvalidParameterOrdinal);
        }

        let body_relationships = relationships
            .iter()
            .filter(|relationship| {
                relationship.get("kind").and_then(Value::as_str) == Some("HAS_BODY_FACT")
                    && relationship.get("source").and_then(Value::as_str) == Some(requested_id)
            })
            .collect::<Vec<_>>();
        let mut body_ids = BTreeSet::new();
        let mut body_facts = Vec::with_capacity(body_relationships.len());
        for relationship in &body_relationships {
            let body_id = relationship_target(relationship)?;
            if !body_ids.insert(body_id.to_owned()) {
                return Err(FunctionContextError::InvalidRelationship);
            }
            let body = entities
                .get(body_id)
                .copied()
                .ok_or_else(|| FunctionContextError::DanglingReference(body_id.to_owned()))?;
            if body.get("subject_id").and_then(Value::as_str) != Some(requested_id)
                || !matches!(
                    required_string(body, "kind")?,
                    "rust.local_binding" | "rust.call_site" | "rust.control"
                )
            {
                return Err(FunctionContextError::InvalidRelationship);
            }
            body_facts.push(body.clone());
        }
        body_facts.sort_by(|left, right| {
            left.get("ordinal")
                .and_then(Value::as_u64)
                .cmp(&right.get("ordinal").and_then(Value::as_u64))
                .then_with(|| record_id(left).cmp(&record_id(right)))
        });

        let call_relationships = relationships
            .iter()
            .filter(|relationship| {
                relationship.get("kind").and_then(Value::as_str) == Some("CALLS")
                    && (relationship.get("source").and_then(Value::as_str) == Some(requested_id)
                        || relationship.get("target").and_then(Value::as_str) == Some(requested_id))
            })
            .collect::<Vec<_>>();
        let mut outgoing_calls = Vec::new();
        let mut incoming_calls = Vec::new();
        for relationship in &call_relationships {
            let source_id = required_string(relationship, "source")?;
            let target_id = relationship_target(relationship)?;
            let source = entities
                .get(source_id)
                .copied()
                .ok_or_else(|| FunctionContextError::DanglingReference(source_id.to_owned()))?;
            let target = entities
                .get(target_id)
                .copied()
                .ok_or_else(|| FunctionContextError::DanglingReference(target_id.to_owned()))?;
            if !matches!(
                required_string(source, "kind")?,
                "rust.function" | "rust.method"
            ) || !matches!(
                required_string(target, "kind")?,
                "rust.function" | "rust.method"
            ) {
                return Err(FunctionContextError::InvalidRelationship);
            }
            let summary = call_summary(relationship, source, target)?;
            if source_id == requested_id {
                outgoing_calls.push(summary.clone());
            }
            if target_id == requested_id {
                incoming_calls.push(summary);
            }
        }
        sort_by_string_field(&mut outgoing_calls, "relationship_id")?;
        sort_by_string_field(&mut incoming_calls, "relationship_id")?;

        let owner = match root.get("owner_id").and_then(Value::as_str) {
            Some(owner_id) => Some(
                entities
                    .get(owner_id)
                    .copied()
                    .ok_or_else(|| FunctionContextError::DanglingReference(owner_id.to_owned()))?
                    .clone(),
            ),
            None => None,
        };

        let mut selected_entity_ids =
            BTreeSet::from([requested_id.to_owned(), signature_id.to_owned()]);
        if let Some(owner_id) = root.get("owner_id").and_then(Value::as_str) {
            selected_entity_ids.insert(owner_id.to_owned());
        }
        selected_entity_ids.extend(parameter_ids);
        selected_entity_ids.extend(body_ids);
        for relationship in &call_relationships {
            selected_entity_ids.insert(required_string(relationship, "source")?.to_owned());
            selected_entity_ids.insert(relationship_target(relationship)?.to_owned());
        }

        let mut selected_relationships = signature_relationships
            .into_iter()
            .chain(parameter_relationships)
            .chain(body_relationships)
            .chain(call_relationships)
            .cloned()
            .collect::<Vec<_>>();
        sort_identified(&mut selected_relationships)?;
        let selected_relationship_ids = selected_relationships
            .iter()
            .map(|relationship| {
                record_id(relationship)
                    .map(str::to_owned)
                    .ok_or(FunctionContextError::InvalidSnapshot)
            })
            .collect::<Result<BTreeSet<_>, _>>()?;

        let mut claims = graph
            .get("claims")
            .and_then(Value::as_array)
            .ok_or(FunctionContextError::InvalidSnapshot)?
            .iter()
            .filter(|claim| {
                claim
                    .get("subject_id")
                    .and_then(Value::as_str)
                    .is_some_and(|subject| {
                        selected_entity_ids.contains(subject)
                            || selected_relationship_ids.contains(subject)
                    })
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut selected_claim_ids = claims
            .iter()
            .map(|claim| {
                record_id(claim)
                    .map(str::to_owned)
                    .ok_or(FunctionContextError::InvalidSnapshot)
            })
            .collect::<Result<BTreeSet<_>, _>>()?;

        let mut derivations = Vec::new();
        for index_name in ["local_flow_index", "constant_evaluation_index"] {
            let values = graph
                .get(index_name)
                .and_then(|index| index.get("derivations"))
                .and_then(Value::as_array)
                .ok_or(FunctionContextError::InvalidSnapshot)?;
            derivations.extend(
                values
                    .iter()
                    .filter(|derivation| {
                        contains_selected_reference(
                            derivation,
                            &selected_entity_ids,
                            &selected_relationship_ids,
                        )
                    })
                    .cloned(),
            );
        }
        sort_derivations(&mut derivations)?;
        for derivation in &derivations {
            collect_references(derivation, "input_claim_ids", |identifier| {
                let claim = claim_map.get(identifier).copied().ok_or_else(|| {
                    FunctionContextError::DanglingReference(identifier.to_owned())
                })?;
                if selected_claim_ids.insert(identifier.to_owned()) {
                    claims.push(claim.clone());
                }
                Ok(())
            })?;
        }
        sort_identified(&mut claims)?;
        validate_claim_subjects(&claims, &selected_entity_ids, &selected_relationship_ids)?;

        let mut diagnostics = selected_uncertainty(graph, "diagnostics", &selected_entity_ids)?;
        let mut coverage_gaps = selected_uncertainty(graph, "coverage", &selected_entity_ids)?;
        sort_identified(&mut diagnostics)?;
        sort_identified(&mut coverage_gaps)?;

        let mut evidence_ids = BTreeSet::new();
        for value in std::iter::once(root)
            .chain(owner.as_ref())
            .chain(std::iter::once(signature))
            .chain(parameters.iter())
            .chain(body_facts.iter())
            .chain(selected_relationships.iter())
            .chain(claims.iter())
            .chain(diagnostics.iter())
            .chain(coverage_gaps.iter())
            .chain(derivations.iter())
        {
            collect_evidence_ids(value, &mut evidence_ids)?;
        }
        let mut evidence = Vec::new();
        let mut pending = evidence_ids.iter().cloned().collect::<Vec<_>>();
        let mut processed_evidence_ids = BTreeSet::new();
        while let Some(identifier) = pending.pop() {
            if !processed_evidence_ids.insert(identifier.clone()) {
                continue;
            }
            let value = evidence_map
                .get(identifier.as_str())
                .copied()
                .ok_or_else(|| FunctionContextError::DanglingReference(identifier.clone()))?;
            evidence.push(value.clone());
            let before = evidence_ids.len();
            collect_evidence_ids(value, &mut evidence_ids)?;
            if evidence_ids.len() != before {
                pending.extend(
                    evidence_ids
                        .iter()
                        .filter(|candidate| {
                            !evidence
                                .iter()
                                .any(|record| record_id(record) == Some(candidate.as_str()))
                        })
                        .cloned(),
                );
            }
        }
        sort_identified(&mut evidence)?;

        let navigation = build_navigation(
            &entities,
            &selected_entity_ids,
            &selected_relationships,
            requested_id,
            root.get("owner_id").and_then(Value::as_str),
            signature_id,
            &parameters,
            &body_facts,
            &outgoing_calls,
            &incoming_calls,
        )?;
        let uncertainty_records = diagnostics
            .len()
            .saturating_add(coverage_gaps.len())
            .saturating_add(derivations.len());
        validate_function_context_limits(FunctionContextCounts {
            parameters: length(parameters.len()),
            linked_subjects: length(selected_entity_ids.len()),
            linked_relationships: length(selected_relationships.len()),
            linked_claims: length(claims.len()),
            linked_evidence: length(evidence.len()),
            uncertainty_records: length(uncertainty_records),
        })?;

        let display_signature = display_signature(root, signature, &parameters)?;
        let value = json!({
            "schema_version": R17_FUNCTION_CONTEXT_VERSION,
            "authority": "declared_source_only",
            "source": {
                "repository_identity": semantic.pointer("/repository/identity").and_then(Value::as_str).ok_or(FunctionContextError::InvalidSnapshot)?,
                "commit_oid": semantic.pointer("/repository/commit_oid").and_then(Value::as_str).ok_or(FunctionContextError::InvalidSnapshot)?,
                "tree_oid": semantic.pointer("/repository/tree_oid").and_then(Value::as_str).ok_or(FunctionContextError::InvalidSnapshot)?,
                "snapshot_id": head.snapshot_id.as_str(),
                "snapshot_schema_version": R16_SNAPSHOT_VERSION,
                "graph_schema_version": R16_GRAPH_VERSION,
                "ontology_version": R16_ONTOLOGY_VERSION,
                "snapshot_semantic_hash": head.semantic_hash.value,
                "graph_semantic_hash": head.graph_semantic_hash.value
            },
            "display_signature": display_signature,
            "callable": root,
            "owner": owner,
            "signature": signature,
            "parameters": parameters,
            "body_facts": body_facts,
            "calls": {
                "outgoing": outgoing_calls,
                "incoming": incoming_calls
            },
            "relationships": selected_relationships,
            "claims": claims,
            "evidence": evidence,
            "diagnostics": diagnostics,
            "coverage_gaps": coverage_gaps,
            "derivations": derivations,
            "navigation": navigation,
            "limitations": LIMITATIONS
        });
        validate_private_fields(&value)?;
        let context = Self { value };
        context.canonical_stdout()?;
        Ok(context)
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Serializes the context as canonical compact JSON followed by one LF.
    ///
    /// # Errors
    ///
    /// Returns a typed error when serialization fails or the output exceeds
    /// the reviewed R17 byte limit.
    pub fn canonical_stdout(&self) -> Result<Vec<u8>, FunctionContextError> {
        let mut bytes =
            serde_json::to_vec(&self.value).map_err(|_| FunctionContextError::Serialization)?;
        let observed = length(bytes.len().saturating_add(1));
        validate_function_context_output_bytes(observed)?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

#[derive(Clone, Debug)]
pub struct LocalExplorerManifestV10 {
    value: Value,
}

impl LocalExplorerManifestV10 {
    /// Creates a manifest for an integrity-checked offline R17 explorer.
    ///
    /// # Errors
    ///
    /// Returns [`R16ContractError::AssetIntegrityMismatch`] when the graph,
    /// viewer asset, digest, or content-security policy is not exact and safe.
    pub fn new(
        portable: &PortableGraphV9,
        viewer_bytes: &[u8],
        expected_viewer_sha256: &str,
        content_security_policy: &str,
        sha256: R16Sha256,
    ) -> Result<Self, R16ContractError> {
        if portable
            .value()
            .get("schema_version")
            .and_then(Value::as_str)
            != Some(R16_PORTABLE_GRAPH_VERSION)
            || sha256(viewer_bytes) != expected_viewer_sha256
            || !safe_viewer(viewer_bytes)
            || !safe_content_security_policy(content_security_policy)
        {
            return Err(R16ContractError::AssetIntegrityMismatch);
        }
        let portable_bytes = portable.canonical_file();
        Ok(Self {
            value: json!({
                "schema_version": R17_LOCAL_EXPLORER_VERSION,
                "profile": R17_CONTEXT_PROFILE,
                "portable_graph": {
                    "path": "portable-graph.json",
                    "schema_version": R16_PORTABLE_GRAPH_VERSION,
                    "sha256": portable.canonical_sha256(),
                    "byte_length": portable_bytes.len()
                },
                "entrypoint": {
                    "path": "index.html",
                    "sha256": expected_viewer_sha256,
                    "byte_length": viewer_bytes.len()
                },
                "security": {
                    "profile": R17_EXPLORER_SECURITY_PROFILE,
                    "content_security_policy": content_security_policy,
                    "network": false,
                    "dynamic_code": false,
                    "storage": false,
                    "telemetry": false,
                    "clipboard": false,
                    "source_access": false,
                    "process_execution": false,
                    "mutation": false,
                    "repair": false,
                    "inference": false
                },
                "capabilities": [
                    "bounded_svg_neighborhood",
                    "call_navigation",
                    "claim_inspection",
                    "declared_signature_cards",
                    "derivation_inspection",
                    "diagnostic_inspection",
                    "evidence_inspection",
                    "exact_id_search",
                    "function_search",
                    "history_navigation",
                    "parameter_navigation",
                    "uncertainty_inspection"
                ],
                "limits": {
                    "portable_graph_bytes": MAX_R16_PORTABLE_GRAPH_BYTES,
                    "function_search_results": MAX_R17_FUNCTION_SEARCH_RESULTS,
                    "navigation_history": MAX_R17_NAVIGATION_HISTORY,
                    "neighborhood_subjects": MAX_R17_LINKED_SUBJECTS,
                    "neighborhood_relationships": MAX_R17_LINKED_RELATIONSHIPS
                }
            }),
        })
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Serializes the manifest as compact JSON followed by one LF.
    ///
    /// # Errors
    ///
    /// Returns a JSON serialization error if the manifest cannot be encoded.
    pub fn canonical_file(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = serde_json::to_vec(&self.value)?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FunctionContextError {
    InvalidSnapshot,
    NotFound,
    InvalidRootKind,
    MissingSignature,
    DuplicateSignature,
    InvalidRelationship,
    InvalidParameterOrdinal,
    DanglingReference(String),
    LimitExceeded {
        limit: &'static str,
        maximum: u64,
        observed: u64,
    },
    UnsafePayload(String),
    Serialization,
}

impl Display for FunctionContextError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidSnapshot => "invalid R17 source snapshot",
            Self::NotFound => "R17 function context root not found",
            Self::InvalidRootKind => "invalid R17 function context root kind",
            Self::MissingSignature => "R17 callable signature is missing",
            Self::DuplicateSignature => "R17 callable signature is ambiguous",
            Self::InvalidRelationship => "invalid R17 function context relationship",
            Self::InvalidParameterOrdinal => "invalid R17 parameter ordinal",
            Self::DanglingReference(_) => "dangling R17 function context reference",
            Self::LimitExceeded { .. } => "R17 function context limit exceeded",
            Self::UnsafePayload(_) => "unsafe R17 function context payload",
            Self::Serialization => "R17 function context serialization failed",
        })
    }
}

impl Error for FunctionContextError {}

fn identified_map(value: Option<&Value>) -> Result<BTreeMap<&str, &Value>, FunctionContextError> {
    let values = value
        .and_then(Value::as_array)
        .ok_or(FunctionContextError::InvalidSnapshot)?;
    let mut result = BTreeMap::new();
    for value in values {
        let identifier = record_id(value).ok_or(FunctionContextError::InvalidSnapshot)?;
        if result.insert(identifier, value).is_some() {
            return Err(FunctionContextError::InvalidSnapshot);
        }
    }
    Ok(result)
}

fn required_string<'a>(value: &'a Value, field: &str) -> Result<&'a str, FunctionContextError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or(FunctionContextError::InvalidSnapshot)
}

fn relationship_target(value: &Value) -> Result<&str, FunctionContextError> {
    required_string(value, "target")
}

fn call_summary(
    relationship: &Value,
    source: &Value,
    target: &Value,
) -> Result<Value, FunctionContextError> {
    Ok(json!({
        "relationship_id": required_string(relationship, "id")?,
        "source_id": required_string(source, "id")?,
        "source_kind": required_string(source, "kind")?,
        "source_name": required_string(source, "name")?,
        "target_id": required_string(target, "id")?,
        "target_kind": required_string(target, "kind")?,
        "target_name": required_string(target, "name")?,
        "state": "proven_unique_local"
    }))
}

fn display_signature(
    root: &Value,
    signature: &Value,
    parameters: &[Value],
) -> Result<String, FunctionContextError> {
    if let Some(value) = root
        .pointer("/properties/declared_signature")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        return Ok(normalized_component(value));
    }

    let properties = signature
        .get("properties")
        .and_then(Value::as_object)
        .ok_or(FunctionContextError::InvalidSnapshot)?;
    let visibility = properties
        .get("visibility")
        .and_then(Value::as_str)
        .ok_or(FunctionContextError::InvalidSnapshot)?;
    let mut value = match visibility {
        "public" => "pub ".to_owned(),
        "crate" => "pub(crate) ".to_owned(),
        "restricted" => "pub(…) ".to_owned(),
        "private" | "inherited_trait" | "not_applicable" => String::new(),
        _ => return Err(FunctionContextError::InvalidSnapshot),
    };
    for (field, keyword) in [
        ("const", "const "),
        ("async", "async "),
        ("unsafe", "unsafe "),
    ] {
        match properties.get(field).and_then(Value::as_bool) {
            Some(true) => value.push_str(keyword),
            Some(false) => {}
            None => return Err(FunctionContextError::InvalidSnapshot),
        }
    }
    if let Some(abi) = optional_non_empty_string(properties.get("abi"))? {
        value.push_str("extern \"");
        value.push_str(&normalized_component(abi));
        value.push_str("\" ");
    }
    value.push_str("fn ");
    value.push_str(required_string(root, "name")?);
    if let Some(generics) = optional_non_empty_string(properties.get("generic_parameters"))? {
        value.push_str(&normalized_component(generics));
    }
    value.push('(');
    for (index, parameter) in parameters.iter().enumerate() {
        if index != 0 {
            value.push_str(", ");
        }
        let pattern = parameter
            .pointer("/properties/pattern")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or(FunctionContextError::InvalidSnapshot)?;
        value.push_str(&normalized_component(pattern));
        if let Some(declared_type) =
            optional_non_empty_string(parameter.pointer("/properties/declared_type"))?
        {
            value.push_str(": ");
            value.push_str(&normalized_component(declared_type));
        }
    }
    value.push(')');
    match properties.get("return_state").and_then(Value::as_str) {
        Some("declared") => {
            let return_type = optional_non_empty_string(properties.get("return_type"))?
                .ok_or(FunctionContextError::InvalidSnapshot)?;
            value.push_str(" -> ");
            value.push_str(&normalized_component(return_type));
        }
        Some("unit_default") => {
            if !properties.get("return_type").is_none_or(Value::is_null) {
                return Err(FunctionContextError::InvalidSnapshot);
            }
        }
        _ => return Err(FunctionContextError::InvalidSnapshot),
    }
    if let Some(where_clause) = optional_non_empty_string(properties.get("where_clause"))? {
        value.push(' ');
        value.push_str(&normalized_component(where_clause));
    }
    Ok(value)
}

fn optional_non_empty_string(value: Option<&Value>) -> Result<Option<&str>, FunctionContextError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if !value.trim().is_empty() => Ok(Some(value)),
        Some(_) => Err(FunctionContextError::InvalidSnapshot),
    }
}

fn normalized_component(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn validate_claim_subjects(
    claims: &[Value],
    entity_ids: &BTreeSet<String>,
    relationship_ids: &BTreeSet<String>,
) -> Result<(), FunctionContextError> {
    for claim in claims {
        let subject_id = required_string(claim, "subject_id")?;
        let valid = match required_string(claim, "subject_kind")? {
            "entity" => entity_ids.contains(subject_id),
            "relationship" => relationship_ids.contains(subject_id),
            _ => false,
        };
        if !valid {
            return Err(FunctionContextError::InvalidRelationship);
        }
    }
    Ok(())
}

fn selected_uncertainty(
    graph: &serde_json::Map<String, Value>,
    family: &str,
    entity_ids: &BTreeSet<String>,
) -> Result<Vec<Value>, FunctionContextError> {
    Ok(graph
        .get(family)
        .and_then(Value::as_array)
        .ok_or(FunctionContextError::InvalidSnapshot)?
        .iter()
        .filter(|record| {
            record
                .get("subject_id")
                .and_then(Value::as_str)
                .is_some_and(|identifier| entity_ids.contains(identifier))
        })
        .cloned()
        .collect())
}

#[allow(clippy::too_many_arguments)]
fn build_navigation(
    entities: &BTreeMap<&str, &Value>,
    selected_ids: &BTreeSet<String>,
    relationships: &[Value],
    root_id: &str,
    owner_id: Option<&str>,
    signature_id: &str,
    parameters: &[Value],
    body_facts: &[Value],
    outgoing_calls: &[Value],
    incoming_calls: &[Value],
) -> Result<Vec<Value>, FunctionContextError> {
    let mut roles = BTreeMap::<String, BTreeSet<&str>>::new();
    roles
        .entry(root_id.to_owned())
        .or_default()
        .insert("callable");
    if let Some(owner_id) = owner_id {
        roles
            .entry(owner_id.to_owned())
            .or_default()
            .insert("owner");
    }
    roles
        .entry(signature_id.to_owned())
        .or_default()
        .insert("signature");
    for parameter in parameters {
        roles
            .entry(required_string(parameter, "id")?.to_owned())
            .or_default()
            .insert("parameter");
    }
    for body_fact in body_facts {
        roles
            .entry(required_string(body_fact, "id")?.to_owned())
            .or_default()
            .insert("body_fact");
    }
    for call in outgoing_calls {
        roles
            .entry(required_string(call, "target_id")?.to_owned())
            .or_default()
            .insert("outgoing_callee");
    }
    for call in incoming_calls {
        roles
            .entry(required_string(call, "source_id")?.to_owned())
            .or_default()
            .insert("incoming_caller");
    }
    let mut navigation = Vec::with_capacity(selected_ids.len());
    for identifier in selected_ids {
        let entity = entities
            .get(identifier.as_str())
            .copied()
            .ok_or_else(|| FunctionContextError::DanglingReference(identifier.clone()))?;
        let relationship_ids = relationships
            .iter()
            .filter(|relationship| {
                relationship.get("source").and_then(Value::as_str) == Some(identifier)
                    || relationship.get("target").and_then(Value::as_str) == Some(identifier)
            })
            .map(|relationship| required_string(relationship, "id").map(str::to_owned))
            .collect::<Result<Vec<_>, _>>()?;
        let entry_roles = roles
            .get(identifier)
            .ok_or(FunctionContextError::InvalidRelationship)?
            .iter()
            .copied()
            .collect::<Vec<_>>();
        navigation.push(json!({
            "id": identifier,
            "family": "entities",
            "kind": required_string(entity, "kind")?,
            "name": required_string(entity, "name")?,
            "roles": entry_roles,
            "relationship_ids": relationship_ids
        }));
    }
    Ok(navigation)
}

fn collect_references(
    value: &Value,
    field: &str,
    mut visit: impl FnMut(&str) -> Result<(), FunctionContextError>,
) -> Result<(), FunctionContextError> {
    let Some(values) = value.get(field) else {
        return Ok(());
    };
    for identifier in values
        .as_array()
        .ok_or(FunctionContextError::InvalidSnapshot)?
    {
        visit(
            identifier
                .as_str()
                .ok_or(FunctionContextError::InvalidSnapshot)?,
        )?;
    }
    Ok(())
}

fn collect_evidence_ids(
    value: &Value,
    identifiers: &mut BTreeSet<String>,
) -> Result<(), FunctionContextError> {
    match value {
        Value::Object(fields) => {
            for (field, nested) in fields {
                if field == "evidence_id" || field.ends_with("_evidence_id") {
                    if nested.is_null() {
                        continue;
                    }
                    identifiers.insert(
                        nested
                            .as_str()
                            .ok_or(FunctionContextError::InvalidSnapshot)?
                            .to_owned(),
                    );
                } else if field == "evidence_ids" || field.ends_with("_evidence_ids") {
                    for identifier in nested
                        .as_array()
                        .ok_or(FunctionContextError::InvalidSnapshot)?
                    {
                        identifiers.insert(
                            identifier
                                .as_str()
                                .ok_or(FunctionContextError::InvalidSnapshot)?
                                .to_owned(),
                        );
                    }
                } else {
                    collect_evidence_ids(nested, identifiers)?;
                }
            }
        }
        Value::Array(values) => {
            for nested in values {
                collect_evidence_ids(nested, identifiers)?;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    Ok(())
}

fn contains_selected_reference(
    value: &Value,
    entity_ids: &BTreeSet<String>,
    relationship_ids: &BTreeSet<String>,
) -> bool {
    match value {
        Value::String(value) => entity_ids.contains(value) || relationship_ids.contains(value),
        Value::Array(values) => values
            .iter()
            .any(|value| contains_selected_reference(value, entity_ids, relationship_ids)),
        Value::Object(values) => values
            .values()
            .any(|value| contains_selected_reference(value, entity_ids, relationship_ids)),
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}

fn validate_private_fields(value: &Value) -> Result<(), FunctionContextError> {
    match value {
        Value::Object(fields) => {
            for (field, nested) in fields {
                if PRIVACY_DENIED_FIELDS.contains(&field.as_str()) {
                    return Err(FunctionContextError::UnsafePayload(field.clone()));
                }
                validate_private_fields(nested)?;
            }
        }
        Value::Array(values) => {
            for nested in values {
                validate_private_fields(nested)?;
            }
        }
        Value::String(value) if value.contains("://") => {
            return Err(FunctionContextError::UnsafePayload("url".to_owned()));
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    Ok(())
}

fn sort_identified(values: &mut [Value]) -> Result<(), FunctionContextError> {
    values.sort_by(|left, right| record_id(left).cmp(&record_id(right)));
    if values.iter().any(|value| record_id(value).is_none()) {
        return Err(FunctionContextError::InvalidSnapshot);
    }
    if values
        .windows(2)
        .any(|pair| record_id(&pair[0]) == record_id(&pair[1]))
    {
        return Err(FunctionContextError::InvalidRelationship);
    }
    Ok(())
}

fn sort_by_string_field(values: &mut [Value], field: &str) -> Result<(), FunctionContextError> {
    if values
        .iter()
        .any(|value| value.get(field).and_then(Value::as_str).is_none())
    {
        return Err(FunctionContextError::InvalidSnapshot);
    }
    values.sort_by(|left, right| {
        left.get(field)
            .and_then(Value::as_str)
            .cmp(&right.get(field).and_then(Value::as_str))
    });
    if values
        .windows(2)
        .any(|pair| pair[0][field] == pair[1][field])
    {
        return Err(FunctionContextError::InvalidRelationship);
    }
    Ok(())
}

fn sort_derivations(values: &mut [Value]) -> Result<(), FunctionContextError> {
    let mut keyed = values
        .iter()
        .map(|value| {
            serde_json::to_string(value)
                .map(|key| (key, value.clone()))
                .map_err(|_| FunctionContextError::Serialization)
        })
        .collect::<Result<Vec<_>, _>>()?;
    keyed.sort_by(|left, right| left.0.cmp(&right.0));
    if keyed.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(FunctionContextError::InvalidRelationship);
    }
    for (target, (_, value)) in values.iter_mut().zip(keyed) {
        *target = value;
    }
    Ok(())
}

fn record_id(value: &Value) -> Option<&str> {
    value.get("id").and_then(Value::as_str)
}

fn enforce_limit(
    limit: &'static str,
    maximum: u64,
    observed: u64,
) -> Result<(), FunctionContextError> {
    if observed > maximum {
        Err(FunctionContextError::LimitExceeded {
            limit,
            maximum,
            observed,
        })
    } else {
        Ok(())
    }
}

fn length(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn safe_viewer(bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let lower = text.to_ascii_lowercase();
    lower.match_indices("<script").count() == 1
        && lower.match_indices("</script>").count() == 1
        && ![
            "http://",
            "https://",
            "<script src",
            "fetch(",
            "xmlhttprequest",
            "websocket",
            "eval(",
            "new function",
            "import(",
            "javascript:",
            ".innerhtml",
            "document.write",
            "localstorage",
            "sessionstorage",
            "indexeddb",
            "document.cookie",
            "navigator.clipboard",
        ]
        .iter()
        .any(|forbidden| lower.contains(forbidden))
}

fn safe_content_security_policy(value: &str) -> bool {
    value.starts_with("default-src 'none';")
        && value.contains("connect-src 'none'")
        && value.contains("worker-src 'none'")
        && value.contains("object-src 'none'")
        && value.contains("frame-src 'none'")
        && value.contains("form-action 'none'")
        && !value.contains("http:")
        && !value.contains("https:")
        && !value.contains("unsafe-inline")
        && !value.contains("unsafe-eval")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::display_signature;

    #[test]
    fn ct_fr_ctx_001_display_signature_uses_only_declared_facts_when_root_has_no_card() {
        let root = json!({
            "kind": "rust.function",
            "name": "clamp",
            "properties": {}
        });
        let signature = json!({
            "properties": {
                "visibility": "public",
                "async": false,
                "const": false,
                "unsafe": false,
                "abi": null,
                "generic_parameters": null,
                "where_clause": null,
                "return_state": "declared",
                "return_type": "i32"
            }
        });
        let parameters = [
            json!({"properties": {"pattern": "input", "declared_type": "i32"}}),
            json!({"properties": {"pattern": "limit", "declared_type": "i32"}}),
        ];

        assert_eq!(
            display_signature(&root, &signature, &parameters).expect("declared display signature"),
            "pub fn clamp(input: i32, limit: i32) -> i32"
        );
    }
}
