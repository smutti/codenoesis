use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use codenoesis_domain::s1_boundaries::RepositoryBoundaryReport;
use codenoesis_domain::s4::{S4_TREE_SITTER_EXTRACTOR_VERSION, WorkspaceExtractionChunk};
use codenoesis_domain::s4_r3::{
    R3_WORKSPACE_EXTRACTOR_VERSION, R3_WORKSPACE_PROFILE, RootPackageMember,
    RootPackageWorkspacePlan,
};
use codenoesis_domain::s4_r4::{
    CargoCoverageGap, CargoDiagnostic, CargoEntity, CargoEntityProperties, CargoManifestFactError,
    CargoManifestKnowledge, CargoRelationship, DeclaredBoolean, DeclaredName, DeclaredPath,
    DeclaredString, DeclaredValue, DependencySource, FeatureMember, LocatorDigest,
    LocatorReference, ManifestIndexEntry, MetadataFact, PatchSourceSelector,
    R4_CARGO_EXTRACTOR_VERSION, R4_EXTRACTION_CONTRACT_VERSION, R4_MANIFEST_PROFILE,
    R4_ONTOLOGY_VERSION, R4_PIPELINE_VERSION, TargetOptions,
};
use codenoesis_domain::storage::{
    LocalSnapshotHead, PublicationCandidate, SNAPSHOT_SCHEMA_VERSION_V7, StorageComponent,
    StorageError,
};
use codenoesis_domain::{
    AcquisitionError, LimitKind, RepositoryInventory, STANDARD_LOCAL_S1_LIMITS, limit_exceeded,
};
use serde_json::{Value, json};

use super::s1_boundaries::repository_boundary_value;
use super::s4::{
    GraphIndex, MAX_QUERY_BYTES, QueryContractError, claim_value, entity_value, evidence_for_claim,
    evidence_value, id_map, linked_statements, relationship_value, string_array, string_field,
    validate_manifest_binding,
};
use super::{
    LimitedVecWriter, PublicationCandidateError, SnapshotEnvelopeV1, inventory_value,
    publication_candidate, semantic_hash,
};

const CONFIGURATION_V4_HASH_DOMAIN: &[u8] = b"codenoesis.configuration.semantic.v4";
const SNAPSHOT_V7_HASH_DOMAIN: &[u8] = b"codenoesis.repository-snapshot.semantic.v7";
const EXTRACTION_V4_HASH_DOMAIN: &[u8] = b"codenoesis.extraction-chunk.semantic.v4";
const GRAPH_V4_HASH_DOMAIN: &[u8] = b"codenoesis.knowledge-graph.semantic.v4";

#[derive(Clone, Debug)]
pub struct CodeNoesisErrorV11 {
    value: Value,
}

impl CodeNoesisErrorV11 {
    #[must_use]
    pub fn invalid_manifest_profile() -> Self {
        Self::new(
            "input.invalid_manifest_profile",
            "input",
            "invalid manifest profile",
            json!({}),
        )
    }

    #[must_use]
    pub fn from_manifest(error: &CargoManifestFactError) -> Option<Self> {
        match error {
            CargoManifestFactError::InvalidFact {
                reason,
                path,
                fact_kind,
                field,
            } => Some(Self::new(
                "extraction.invalid_cargo_manifest_fact",
                "extraction",
                "invalid Cargo manifest fact",
                json!({
                    "reason": reason.as_str(),
                    "path": path,
                    "fact_kind": fact_kind.as_str(),
                    "field": field
                }),
            )),
            CargoManifestFactError::Conflict {
                path,
                fact_kind,
                declaration_name_sha256,
            } => Some(Self::new(
                "extraction.cargo_manifest_fact_conflict",
                "extraction",
                "conflicting Cargo manifest fact",
                json!({
                    "path": path,
                    "fact_kind": fact_kind.as_str(),
                    "declaration_name_sha256": declaration_name_sha256
                }),
            )),
            CargoManifestFactError::LimitExceeded {
                limit,
                maximum,
                observed,
            } => Some(Self::new(
                "extraction.cargo_manifest_fact_limit_exceeded",
                "extraction",
                "Cargo manifest fact limit exceeded",
                json!({
                    "limit": limit.as_str(),
                    "maximum": maximum,
                    "observed": observed
                }),
            )),
            CargoManifestFactError::ContractInvalid => Some(Self::internal()),
            CargoManifestFactError::Source(_) => None,
        }
    }

    #[must_use]
    pub fn internal() -> Self {
        Self::new(
            "internal.unexpected",
            "internal",
            "internal error",
            json!({}),
        )
    }

    /// Serializes one strict `ErrorV11` followed by one LF.
    ///
    /// # Errors
    ///
    /// Returns a serialization failure only for an invalid internal document.
    pub fn canonical_stderr(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = serde_json::to_vec(&self.value)?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    #[allow(clippy::needless_pass_by_value)]
    fn new(code: &str, stage: &str, message: &str, context: Value) -> Self {
        Self {
            value: json!({
                "schema_version": "codenoesis.error/v11",
                "code": code,
                "stage": stage,
                "message": message,
                "retryable": false,
                "context": context
            }),
        }
    }
}

#[derive(Clone, Debug)]
pub struct LocalQueryResultV2 {
    value: Value,
}

impl LocalQueryResultV2 {
    /// Serializes one bounded exact-ID V2 result followed by one LF.
    ///
    /// # Errors
    ///
    /// Returns a serialization or fixed query-result limit failure.
    pub fn canonical_stdout(&self) -> Result<Vec<u8>, QueryContractError> {
        let mut bytes =
            serde_json::to_vec(&self.value).map_err(|_| QueryContractError::InvalidSnapshot)?;
        if bytes.len() >= MAX_QUERY_BYTES {
            return Err(QueryContractError::LimitExceeded);
        }
        bytes.push(b'\n');
        Ok(bytes)
    }
}

/// Builds one strict V7 exact-ID query result without adding read authority.
///
/// # Errors
///
/// Returns a strict snapshot, document, not-found, or output-limit failure.
#[allow(clippy::too_many_lines)]
pub fn local_query_result_v2(
    semantic: &Value,
    manifest: &Value,
    snapshot_id: &str,
    requested_id: &str,
) -> Result<LocalQueryResultV2, QueryContractError> {
    let index = GraphIndex::new(semantic).map_err(|_| QueryContractError::InvalidSnapshot)?;
    validate_manifest_binding(manifest, &index.repository_identity, snapshot_id)?;
    let graph = semantic
        .get("knowledge_graph")
        .ok_or(QueryContractError::InvalidSnapshot)?;
    let diagnostics =
        id_map(graph, "diagnostics").map_err(|_| QueryContractError::InvalidSnapshot)?;
    let documents = manifest
        .get("documents")
        .and_then(Value::as_array)
        .ok_or(QueryContractError::InvalidDocuments)?;

    let value = if let Some(entity) = index.entities.get(requested_id) {
        let claim = index
            .claim("entity", requested_id)
            .map_err(|_| QueryContractError::InvalidSnapshot)?;
        let evidence = evidence_for_claim(&index, claim)?;
        json!({
            "schema_version": "codenoesis.local-query-result/v2",
            "repository_identity": index.repository_identity,
            "snapshot_id": snapshot_id,
            "requested_id": requested_id,
            "result_kind": "entity",
            "entity": entity,
            "relationship": null,
            "claims": [claim],
            "evidence": evidence,
            "diagnostic": null,
            "coverage_gap": null,
            "document": null,
            "document_statements": linked_statements(documents, requested_id)?
        })
    } else if let Some(relationship) = index.relationships_by_id.get(requested_id) {
        let claim = index
            .claim("relationship", requested_id)
            .map_err(|_| QueryContractError::InvalidSnapshot)?;
        let evidence = evidence_for_claim(&index, claim)?;
        json!({
            "schema_version": "codenoesis.local-query-result/v2",
            "repository_identity": index.repository_identity,
            "snapshot_id": snapshot_id,
            "requested_id": requested_id,
            "result_kind": "relationship",
            "entity": null,
            "relationship": relationship,
            "claims": [claim],
            "evidence": evidence,
            "diagnostic": null,
            "coverage_gap": null,
            "document": null,
            "document_statements": linked_statements(documents, requested_id)?
        })
    } else if let Some(claim) = index.claims_by_id.get(requested_id) {
        let evidence = evidence_for_claim(&index, claim)?;
        let subject_id =
            string_field(claim, "subject_id").map_err(|_| QueryContractError::InvalidSnapshot)?;
        let (entity, relationship) = match string_field(claim, "subject_kind")
            .map_err(|_| QueryContractError::InvalidSnapshot)?
        {
            "entity" => (
                Some(
                    index
                        .entities
                        .get(subject_id)
                        .ok_or(QueryContractError::InvalidSnapshot)?,
                ),
                None,
            ),
            "relationship" => (
                None,
                Some(
                    index
                        .relationships_by_id
                        .get(subject_id)
                        .ok_or(QueryContractError::InvalidSnapshot)?,
                ),
            ),
            _ => return Err(QueryContractError::InvalidSnapshot),
        };
        json!({
            "schema_version": "codenoesis.local-query-result/v2",
            "repository_identity": index.repository_identity,
            "snapshot_id": snapshot_id,
            "requested_id": requested_id,
            "result_kind": "claim",
            "entity": entity,
            "relationship": relationship,
            "claims": [claim],
            "evidence": evidence,
            "diagnostic": null,
            "coverage_gap": null,
            "document": null,
            "document_statements": linked_statements(documents, requested_id)?
        })
    } else if let Some(evidence) = index.evidence.get(requested_id) {
        json!({
            "schema_version": "codenoesis.local-query-result/v2",
            "repository_identity": index.repository_identity,
            "snapshot_id": snapshot_id,
            "requested_id": requested_id,
            "result_kind": "evidence",
            "entity": null,
            "relationship": null,
            "claims": [],
            "evidence": [evidence],
            "diagnostic": null,
            "coverage_gap": null,
            "document": null,
            "document_statements": linked_statements(documents, requested_id)?
        })
    } else if let Some(diagnostic) = diagnostics.get(requested_id) {
        let evidence = evidence_for_record(&index, diagnostic)?;
        json!({
            "schema_version": "codenoesis.local-query-result/v2",
            "repository_identity": index.repository_identity,
            "snapshot_id": snapshot_id,
            "requested_id": requested_id,
            "result_kind": "diagnostic",
            "entity": null,
            "relationship": null,
            "claims": [],
            "evidence": evidence,
            "diagnostic": diagnostic,
            "coverage_gap": null,
            "document": null,
            "document_statements": linked_statements(documents, requested_id)?
        })
    } else if let Some(coverage_gap) = index.coverage.get(requested_id) {
        let evidence = evidence_for_record(&index, coverage_gap)?;
        json!({
            "schema_version": "codenoesis.local-query-result/v2",
            "repository_identity": index.repository_identity,
            "snapshot_id": snapshot_id,
            "requested_id": requested_id,
            "result_kind": "coverage_gap",
            "entity": null,
            "relationship": null,
            "claims": [],
            "evidence": evidence,
            "diagnostic": null,
            "coverage_gap": coverage_gap,
            "document": null,
            "document_statements": linked_statements(documents, requested_id)?
        })
    } else if let Some(document) = documents
        .iter()
        .find(|document| string_field(document, "document_id") == Ok(requested_id))
    {
        let mut record = document
            .as_object()
            .cloned()
            .ok_or(QueryContractError::InvalidDocuments)?;
        let statements = record
            .remove("statements")
            .and_then(|value| value.as_array().cloned())
            .ok_or(QueryContractError::InvalidDocuments)?;
        json!({
            "schema_version": "codenoesis.local-query-result/v2",
            "repository_identity": index.repository_identity,
            "snapshot_id": snapshot_id,
            "requested_id": requested_id,
            "result_kind": "document",
            "entity": null,
            "relationship": null,
            "claims": [],
            "evidence": [],
            "diagnostic": null,
            "coverage_gap": null,
            "document": Value::Object(record),
            "document_statements": statements
        })
    } else {
        return Err(QueryContractError::NotFound);
    };
    let result = LocalQueryResultV2 { value };
    result.canonical_stdout()?;
    Ok(result)
}

fn evidence_for_record(
    index: &GraphIndex,
    record: &Value,
) -> Result<Vec<Value>, QueryContractError> {
    let evidence_ids =
        string_array(record, "evidence_ids").map_err(|_| QueryContractError::InvalidSnapshot)?;
    if evidence_ids.is_empty()
        || evidence_ids.len() > 64
        || evidence_ids
            .windows(2)
            .any(|pair| pair[0].as_bytes() >= pair[1].as_bytes())
    {
        return Err(QueryContractError::InvalidSnapshot);
    }
    evidence_ids
        .into_iter()
        .map(|identifier| {
            index
                .evidence
                .get(&identifier)
                .cloned()
                .ok_or(QueryContractError::InvalidSnapshot)
        })
        .collect()
}

#[derive(Clone, Debug)]
pub struct RepositorySnapshotV7 {
    value: Value,
}

#[derive(Debug)]
pub enum RepositorySnapshotV7Error {
    Serialization(serde_json::Error),
    LimitExceeded(AcquisitionError),
    ContractInvalid,
    OutputLengthOverflow,
}

impl Display for RepositorySnapshotV7Error {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Serialization(_) => "R4 snapshot serialization failed",
            Self::LimitExceeded(_) => "R4 snapshot output limit exceeded",
            Self::ContractInvalid => "R4 snapshot contract is invalid",
            Self::OutputLengthOverflow => "R4 snapshot output length overflowed",
        })
    }
}

impl Error for RepositorySnapshotV7Error {}

impl RepositorySnapshotV7 {
    /// Builds the selector-bound additive R4 snapshot over validated R3 and Cargo knowledge.
    ///
    /// # Errors
    ///
    /// Returns a domain contract, serialization, or output-bound failure.
    pub fn from_inventory_and_manifest_facts(
        inventory: &RepositoryInventory,
        knowledge: &CargoManifestKnowledge,
        boundaries: Option<&RepositoryBoundaryReport>,
        envelope: SnapshotEnvelopeV1,
    ) -> Result<Self, RepositorySnapshotV7Error> {
        knowledge
            .validate()
            .map_err(|_| RepositorySnapshotV7Error::ContractInvalid)?;
        if boundaries.is_some_and(|report| report.root_repository != *inventory.bound_revision()) {
            return Err(RepositorySnapshotV7Error::ContractInvalid);
        }
        let SnapshotEnvelopeV1 {
            created_at,
            job_id,
            correlation_id,
        } = envelope;
        let boundary_profile = boundaries.map(|_| "local-gitlinks-v1");
        let configuration_without_hash = json!({
            "schema_version": "codenoesis.configuration/v4",
            "profile": "standard-local-s4",
            "workspace_profile": R3_WORKSPACE_PROFILE,
            "manifest_profile": R4_MANIFEST_PROFILE,
            "repository_boundary_profile": boundary_profile
        });
        let configuration_hash =
            semantic_hash(CONFIGURATION_V4_HASH_DOMAIN, &configuration_without_hash);
        let mut configuration = configuration_without_hash;
        configuration
            .as_object_mut()
            .ok_or(RepositorySnapshotV7Error::ContractInvalid)?
            .insert(
                "semantic_hash".to_owned(),
                json!({"algorithm": "blake3-256", "value": configuration_hash}),
            );
        let extraction_chunks = extraction_chunks_v4(knowledge)?;
        let graph = knowledge_graph_v4(inventory, knowledge)?;
        let mut semantic = json!({
            "repository": repository_value(inventory),
            "configuration": configuration,
            "pipeline_version": R4_PIPELINE_VERSION,
            "ontology_version": R4_ONTOLOGY_VERSION,
            "extractor_contract_version": R4_EXTRACTION_CONTRACT_VERSION,
            "extractor_versions": [
                "codenoesis.inventory-classifier/s1-v1",
                S4_TREE_SITTER_EXTRACTOR_VERSION,
                R3_WORKSPACE_EXTRACTOR_VERSION,
                R4_CARGO_EXTRACTOR_VERSION
            ],
            "evidence_lineage_version": "codenoesis.evidence-lineage/v2",
            "inventory": inventory_value(inventory),
            "extraction_chunks": extraction_chunks,
            "knowledge_graph": graph
        });
        if let Some(report) = boundaries {
            let semantic_object = semantic
                .as_object_mut()
                .ok_or(RepositorySnapshotV7Error::ContractInvalid)?;
            semantic_object.insert(
                "extractor_versions".to_owned(),
                json!([
                    "codenoesis.inventory-classifier/s1-v1",
                    S4_TREE_SITTER_EXTRACTOR_VERSION,
                    R3_WORKSPACE_EXTRACTOR_VERSION,
                    R4_CARGO_EXTRACTOR_VERSION,
                    "codenoesis.git-boundary/s1-v1"
                ]),
            );
            semantic_object.insert(
                "repository_boundaries".to_owned(),
                repository_boundary_value(report),
            );
        }
        let snapshot_hash = semantic_hash(SNAPSHOT_V7_HASH_DOMAIN, &semantic);
        Ok(Self {
            value: json!({
                "schema_version": SNAPSHOT_SCHEMA_VERSION_V7,
                "semantic_hash": {"algorithm": "blake3-256", "value": snapshot_hash},
                "semantic": semantic,
                "envelope": {
                    "created_at": created_at,
                    "job_id": job_id,
                    "correlation_id": correlation_id
                }
            }),
        })
    }

    /// Serializes the complete V7 snapshot with the inherited output bound.
    ///
    /// # Errors
    ///
    /// Returns a typed serialization or output-limit failure.
    pub fn canonical_stdout(&self) -> Result<Vec<u8>, RepositorySnapshotV7Error> {
        let maximum = usize::try_from(STANDARD_LOCAL_S1_LIMITS.canonical_output_bytes)
            .map_err(|_| RepositorySnapshotV7Error::OutputLengthOverflow)?;
        let body_maximum = maximum
            .checked_sub(1)
            .ok_or(RepositorySnapshotV7Error::OutputLengthOverflow)?;
        let mut writer = LimitedVecWriter::new(body_maximum);
        let result = serde_json::to_writer(&mut writer, &self.value);
        if writer.overflowed() {
            return Err(RepositorySnapshotV7Error::LimitExceeded(limit_exceeded(
                LimitKind::CanonicalOutputBytes,
                STANDARD_LOCAL_S1_LIMITS
                    .canonical_output_bytes
                    .saturating_add(1),
            )));
        }
        result.map_err(RepositorySnapshotV7Error::Serialization)?;
        let mut bytes = writer.into_inner();
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Serializes the exact V7 semantic payload stored by S4.
    ///
    /// # Errors
    ///
    /// Returns a serialization failure only for an invalid internal value.
    pub fn canonical_semantic(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(&self.value["semantic"])
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Converts V7 into the unchanged immutable publication model.
    ///
    /// # Errors
    ///
    /// Returns a strict contract or storage-integrity failure.
    pub fn publication_candidate(&self) -> Result<PublicationCandidate, PublicationCandidateError> {
        publication_candidate(&self.value)
    }
}

/// Validates one loaded V7 semantic payload against the complete visible head.
///
/// # Errors
///
/// Returns a typed metadata-integrity failure on any binding mismatch.
pub fn validate_stored_snapshot_semantic_v7(
    semantic: &Value,
    head: &LocalSnapshotHead,
) -> Result<(), StorageError> {
    if head.snapshot_schema_version != SNAPSHOT_SCHEMA_VERSION_V7 {
        return Err(stored_snapshot_error(
            head,
            "stored_snapshot_schema_mismatch",
        ));
    }
    let value = json!({
        "schema_version": head.snapshot_schema_version,
        "semantic_hash": {
            "algorithm": head.semantic_hash.algorithm,
            "value": head.semantic_hash.value
        },
        "semantic": semantic
    });
    let candidate = publication_candidate(&value)
        .map_err(|_| stored_snapshot_error(head, "stored_snapshot_contract_invalid"))?;
    if candidate.snapshot.repository_identity != head.repository_identity
        || candidate.snapshot.snapshot_id != head.snapshot_id
        || candidate.snapshot.commit_oid != head.commit_oid
        || candidate.snapshot.snapshot_schema_version != head.snapshot_schema_version
        || candidate.snapshot.semantic_hash != head.semantic_hash
        || candidate.snapshot.graph_semantic_hash != head.graph_semantic_hash
        || candidate.artifact_references() != head.artifacts
    {
        return Err(stored_snapshot_error(head, "stored_snapshot_head_mismatch"));
    }
    Ok(())
}

fn extraction_chunks_v4(
    knowledge: &CargoManifestKnowledge,
) -> Result<Vec<Value>, RepositorySnapshotV7Error> {
    let cargo_manifest_ids = knowledge
        .graph
        .manifest_index
        .iter()
        .map(|entry| (entry.manifest_path.as_str(), entry.manifest_id.as_str()))
        .collect::<BTreeMap<_, _>>();
    let mut chunks = knowledge
        .workspace
        .knowledge
        .extraction_chunks
        .iter()
        .map(|chunk| {
            rust_extraction_chunk_v4(chunk, &knowledge.workspace.plan, &cargo_manifest_ids)
        })
        .collect::<Result<Vec<_>, _>>()?;
    chunks.extend(
        knowledge
            .extraction_chunks
            .iter()
            .map(|chunk| {
                cargo_extraction_chunk_v4(
                    chunk,
                    &knowledge.workspace.knowledge.graph.repository_identity,
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
    );
    chunks.sort_by(|left, right| chunk_order(left).cmp(&chunk_order(right)));
    Ok(chunks)
}

fn rust_extraction_chunk_v4(
    chunk: &WorkspaceExtractionChunk,
    plan: &RootPackageWorkspacePlan,
    manifest_ids: &BTreeMap<&str, &str>,
) -> Result<Value, RepositorySnapshotV7Error> {
    let target = plan
        .target(&chunk.crate_id)
        .ok_or(RepositorySnapshotV7Error::ContractInvalid)?;
    let manifest_id = manifest_ids
        .get(target.manifest_path.as_str())
        .copied()
        .ok_or(RepositorySnapshotV7Error::ContractInvalid)?;
    let mut value = json!({
        "schema_version": "codenoesis.extraction-chunk/v4",
        "ontology_version": R4_ONTOLOGY_VERSION,
        "repository_identity": chunk.repository_identity,
        "subject": {
            "kind": "rust_source",
            "manifest_id": manifest_id,
            "crate_id": chunk.crate_id,
            "source_file_id": chunk.source_file_id,
            "workspace": {
                "root_shape": plan.root_shape.as_str(),
                "member_path": target.member_path,
                "member_source": target.member_source.as_str(),
                "manifest_path": target.manifest_path,
                "target_kind": target.target_kind.as_str(),
                "target_name": target.target_name
            }
        },
        "entities": chunk.entities.iter().map(entity_value).collect::<Vec<_>>(),
        "relationships": chunk.relationships.iter().map(relationship_value).collect::<Vec<_>>(),
        "claims": chunk.claims.iter().map(claim_value).collect::<Vec<_>>(),
        "evidence": chunk.evidence.iter().map(evidence_value).collect::<Vec<_>>(),
        "diagnostics": [],
        "coverage": []
    });
    insert_semantic_hash(&mut value, EXTRACTION_V4_HASH_DOMAIN)?;
    Ok(value)
}

fn cargo_extraction_chunk_v4(
    chunk: &codenoesis_domain::s4_r4::CargoManifestExtractionChunk,
    repository_identity: &str,
) -> Result<Value, RepositorySnapshotV7Error> {
    let mut value = json!({
        "schema_version": "codenoesis.extraction-chunk/v4",
        "ontology_version": R4_ONTOLOGY_VERSION,
        "repository_identity": repository_identity,
        "subject": {
            "kind": "cargo_manifest",
            "manifest_id": chunk.manifest_id,
            "manifest_path": chunk.manifest_path
        },
        "entities": chunk.entities.iter().map(cargo_entity_value).collect::<Vec<_>>(),
        "relationships": chunk.relationships.iter().map(cargo_relationship_value).collect::<Vec<_>>(),
        "claims": chunk.claims.iter().map(claim_value).collect::<Vec<_>>(),
        "evidence": chunk.evidence.iter().map(evidence_value).collect::<Vec<_>>(),
        "diagnostics": chunk.diagnostics.iter().map(cargo_diagnostic_value).collect::<Vec<_>>(),
        "coverage": chunk.coverage.iter().map(cargo_coverage_value).collect::<Vec<_>>()
    });
    insert_semantic_hash(&mut value, EXTRACTION_V4_HASH_DOMAIN)?;
    Ok(value)
}

fn knowledge_graph_v4(
    inventory: &RepositoryInventory,
    knowledge: &CargoManifestKnowledge,
) -> Result<Value, RepositorySnapshotV7Error> {
    let rust_graph = &knowledge.workspace.knowledge.graph;
    let mut entities = rust_graph
        .entities
        .iter()
        .map(entity_value)
        .chain(knowledge.graph.entities.iter().map(cargo_entity_value))
        .collect::<Vec<_>>();
    sort_values_by_id(&mut entities)?;
    let mut relationships = rust_graph
        .relationships
        .iter()
        .map(relationship_value)
        .chain(
            knowledge
                .graph
                .relationships
                .iter()
                .map(cargo_relationship_value),
        )
        .collect::<Vec<_>>();
    sort_values_by_id(&mut relationships)?;
    let mut claims = rust_graph
        .claims
        .iter()
        .map(claim_value)
        .chain(knowledge.graph.claims.iter().map(claim_value))
        .collect::<Vec<_>>();
    sort_values_by_id(&mut claims)?;
    let mut evidence = rust_graph
        .evidence
        .iter()
        .map(evidence_value)
        .chain(knowledge.graph.evidence.iter().map(evidence_value))
        .collect::<Vec<_>>();
    sort_and_dedup_values_by_id(&mut evidence)?;
    let mut value = json!({
        "schema_version": "codenoesis.knowledge-graph/v4",
        "ontology_version": R4_ONTOLOGY_VERSION,
        "repository": repository_value(inventory),
        "extractor_versions": [
            S4_TREE_SITTER_EXTRACTOR_VERSION,
            R3_WORKSPACE_EXTRACTOR_VERSION,
            R4_CARGO_EXTRACTOR_VERSION
        ],
        "workspace": workspace_value(&knowledge.workspace.plan),
        "manifest_index": {
            "schema_version": "codenoesis.cargo-manifest-index/v1",
            "manifests": knowledge.graph.manifest_index.iter().map(manifest_index_value).collect::<Vec<_>>()
        },
        "entities": entities,
        "relationships": relationships,
        "claims": claims,
        "evidence": evidence,
        "diagnostics": knowledge.graph.diagnostics.iter().map(cargo_diagnostic_value).collect::<Vec<_>>(),
        "coverage": knowledge.graph.coverage.iter().map(cargo_coverage_value).collect::<Vec<_>>()
    });
    insert_semantic_hash(&mut value, GRAPH_V4_HASH_DOMAIN)?;
    Ok(value)
}

fn cargo_entity_value(entity: &CargoEntity) -> Value {
    json!({
        "id": entity.id,
        "kind": entity.kind().as_str(),
        "crate_id": null,
        "module_path": null,
        "name": entity.name,
        "visibility": "not_applicable",
        "properties": cargo_properties_value(&entity.properties)
    })
}

#[allow(clippy::too_many_lines)]
fn cargo_properties_value(properties: &CargoEntityProperties) -> Value {
    match properties {
        CargoEntityProperties::Manifest(properties) => json!({
            "manifest_path": properties.manifest_path,
            "manifest_role": properties.manifest_role.as_str(),
            "root_shape": properties.root_shape,
            "package_table_present": properties.package_table_present,
            "workspace_table_present": properties.workspace_table_present,
            "evidence_id": properties.evidence_id
        }),
        CargoEntityProperties::WorkspacePackageDefaults(properties) => json!({
            "manifest_id": properties.manifest_id,
            "manifest_path": properties.manifest_path,
            "metadata": properties.metadata.iter().map(metadata_value).collect::<Vec<_>>(),
            "evidence_id": properties.evidence_id
        }),
        CargoEntityProperties::Package(properties) => json!({
            "manifest_id": properties.manifest_id,
            "manifest_path": properties.manifest_path,
            "package_name": properties.package_name,
            "metadata": properties.metadata.iter().map(metadata_value).collect::<Vec<_>>(),
            "evidence_id": properties.evidence_id
        }),
        CargoEntityProperties::Target(properties) => json!({
            "manifest_id": properties.manifest_id,
            "package_id": properties.package_id,
            "manifest_path": properties.manifest_path,
            "target_kind": properties.target_kind.as_str(),
            "target_name": properties.target_name,
            "name_source": properties.name_source.as_str(),
            "source_path": declared_path_value(&properties.source_path),
            "path_source": properties.path_source.as_str(),
            "required_features": properties.required_features.iter().map(declared_name_value).collect::<Vec<_>>(),
            "options": target_options_value(&properties.options),
            "source_analysis_state": properties.source_analysis_state.as_str(),
            "materialized_crate_id": properties.materialized_crate_id,
            "evidence_id": properties.evidence_id
        }),
        CargoEntityProperties::Dependency(properties) => json!({
            "manifest_id": properties.manifest_id,
            "owner_id": properties.owner_id,
            "manifest_path": properties.manifest_path,
            "scope": properties.scope.as_str(),
            "dependency_kind": properties.dependency_kind.as_str(),
            "target_predicate": properties.target_predicate.as_ref().map(declared_string_value),
            "declared_name": properties.declared_name,
            "package_name": properties.package_name.as_ref().map(declared_string_value),
            "source": dependency_source_value(&properties.source),
            "optional": properties.optional.as_ref().map(declared_boolean_value),
            "default_features": properties.default_features.as_ref().map(declared_boolean_value),
            "requested_features": properties.requested_features.iter().map(declared_name_value).collect::<Vec<_>>(),
            "evidence_id": properties.evidence_id
        }),
        CargoEntityProperties::Feature(properties) => json!({
            "manifest_id": properties.manifest_id,
            "package_id": properties.package_id,
            "manifest_path": properties.manifest_path,
            "feature_name": properties.feature_name,
            "members": properties.members.iter().map(feature_member_value).collect::<Vec<_>>(),
            "evidence_id": properties.evidence_id
        }),
        CargoEntityProperties::Patch(properties) => json!({
            "manifest_id": properties.manifest_id,
            "manifest_path": properties.manifest_path,
            "source_selector": patch_selector_value(&properties.source_selector),
            "declared_name": properties.declared_name,
            "package_name": properties.package_name.as_ref().map(declared_string_value),
            "source": dependency_source_value(&properties.source),
            "applied": false,
            "evidence_id": properties.evidence_id
        }),
        CargoEntityProperties::BuildScript(properties) => json!({
            "manifest_id": properties.manifest_id,
            "package_id": properties.package_id,
            "manifest_path": properties.manifest_path,
            "selection": properties.selection.as_str(),
            "path": properties.path.as_ref().map(declared_path_value),
            "committed_present": properties.committed_present,
            "executed": false,
            "evidence_id": properties.evidence_id
        }),
    }
}

fn metadata_value(fact: &MetadataFact) -> Value {
    json!({
        "field": fact.field,
        "value": declared_value(&fact.value),
        "inherited_from": fact.inherited_from,
        "evidence_id": fact.evidence_id
    })
}

fn declared_value(value: &DeclaredValue) -> Value {
    match value {
        DeclaredValue::String(value) => json!({"kind": "string", "value": value}),
        DeclaredValue::Boolean(value) => json!({"kind": "boolean", "value": value}),
        DeclaredValue::StringArray(values) => json!({"kind": "string_array", "values": values}),
        DeclaredValue::Publish {
            enabled,
            registries,
        } => {
            json!({"kind": "publish", "enabled": enabled, "registries": registries})
        }
        DeclaredValue::LocatorSha256(sha256) => {
            json!({"kind": "locator_sha256", "sha256": sha256, "redacted": true})
        }
        DeclaredValue::Path {
            declared,
            normalized,
        } => json!({"kind": "path", "declared": declared, "normalized": normalized}),
        DeclaredValue::WorkspaceReference {
            source_entity_id,
            source_field,
        } => json!({
            "kind": "workspace_reference",
            "source_entity_id": source_entity_id,
            "source_field": source_field
        }),
    }
}

fn declared_string_value(value: &DeclaredString) -> Value {
    json!({"value": value.value, "evidence_id": value.evidence_id})
}

fn declared_name_value(value: &DeclaredName) -> Value {
    json!({"value": value.value, "evidence_id": value.evidence_id})
}

fn declared_boolean_value(value: &DeclaredBoolean) -> Value {
    json!({"value": value.value, "evidence_id": value.evidence_id})
}

fn declared_path_value(value: &DeclaredPath) -> Value {
    json!({
        "declared": value.declared,
        "normalized": value.normalized,
        "evidence_id": value.evidence_id
    })
}

fn locator_digest_value(value: &LocatorDigest) -> Value {
    json!({"sha256": value.sha256, "redacted": true, "evidence_id": value.evidence_id})
}

fn locator_reference_value(value: &LocatorReference) -> Value {
    json!({
        "kind": value.kind.as_str(),
        "sha256": value.sha256,
        "redacted": true,
        "evidence_id": value.evidence_id
    })
}

fn dependency_source_value(source: &DependencySource) -> Value {
    json!({
        "kind": source.kind.as_str(),
        "version_requirement": source.version_requirement.as_ref().map(declared_string_value),
        "registry_name": source.registry_name.as_ref().map(declared_string_value),
        "path": source.path.as_ref().map(declared_path_value),
        "git_locator": source.git_locator.as_ref().map(locator_digest_value),
        "git_reference": source.git_reference.as_ref().map(locator_reference_value),
        "workspace_reference_id": source.workspace_reference_id
    })
}

fn target_options_value(options: &TargetOptions) -> Value {
    json!({
        "crate_types": options.crate_types.iter().map(declared_name_value).collect::<Vec<_>>(),
        "proc_macro": options.proc_macro.as_ref().map(declared_boolean_value),
        "bench": options.bench.as_ref().map(declared_boolean_value),
        "doc": options.doc.as_ref().map(declared_boolean_value),
        "doctest": options.doctest.as_ref().map(declared_boolean_value),
        "test": options.test.as_ref().map(declared_boolean_value),
        "harness": options.harness.as_ref().map(declared_boolean_value),
        "edition": options.edition.as_ref().map(declared_string_value)
    })
}

fn feature_member_value(member: &FeatureMember) -> Value {
    json!({
        "lexeme": member.lexeme,
        "syntax": member.syntax.as_str(),
        "dependency_name": member.dependency_name,
        "feature_name": member.feature_name,
        "evidence_id": member.evidence_id
    })
}

fn patch_selector_value(selector: &PatchSourceSelector) -> Value {
    json!({
        "kind": selector.kind.as_str(),
        "name": selector.name,
        "sha256": selector.sha256,
        "evidence_id": selector.evidence_id
    })
}

fn cargo_relationship_value(relationship: &CargoRelationship) -> Value {
    json!({
        "id": relationship.id,
        "kind": relationship.kind.as_str(),
        "source": relationship.source,
        "target": relationship.target,
        "evidence_ids": relationship.evidence_ids
    })
}

fn cargo_diagnostic_value(diagnostic: &CargoDiagnostic) -> Value {
    json!({
        "id": diagnostic.id,
        "code": diagnostic.code,
        "message": diagnostic.message,
        "evidence_ids": diagnostic.evidence_ids
    })
}

fn cargo_coverage_value(gap: &CargoCoverageGap) -> Value {
    json!({
        "id": gap.id,
        "capability": gap.capability,
        "state": gap.state.as_str(),
        "evidence_ids": gap.evidence_ids
    })
}

fn manifest_index_value(entry: &ManifestIndexEntry) -> Value {
    json!({
        "manifest_id": entry.manifest_id,
        "manifest_path": entry.manifest_path,
        "package_id": entry.package_id,
        "fact_entity_ids": entry.fact_entity_ids
    })
}

fn workspace_value(plan: &RootPackageWorkspacePlan) -> Value {
    json!({
        "root_shape": plan.root_shape.as_str(),
        "members": plan.members.iter().map(member_value).collect::<Vec<_>>(),
        "excluded_paths": plan.excluded_paths
    })
}

fn member_value(member: &RootPackageMember) -> Value {
    json!({
        "path": member.path,
        "manifest_path": member.manifest_path,
        "member_source": member.member_source.as_str(),
        "crate_ids": member.crate_ids,
        "external_boundary_id": member.external_boundary_id
    })
}

fn repository_value(inventory: &RepositoryInventory) -> Value {
    let bound = inventory.bound_revision();
    json!({
        "contract_version": "codenoesis.repository/v1",
        "identity_schema_version": "codenoesis.repository-identity/v1",
        "identity": bound.repository_identity().as_str(),
        "vcs": "git",
        "object_format": "sha1",
        "commit_oid": bound.commit_oid().as_str(),
        "tree_oid": bound.tree_oid().as_str()
    })
}

fn insert_semantic_hash(value: &mut Value, domain: &[u8]) -> Result<(), RepositorySnapshotV7Error> {
    let hash = semantic_hash(domain, value);
    value
        .as_object_mut()
        .ok_or(RepositorySnapshotV7Error::ContractInvalid)?
        .insert(
            "semantic_hash".to_owned(),
            json!({"algorithm": "blake3-256", "value": hash}),
        );
    Ok(())
}

fn chunk_order(value: &Value) -> (u8, &str, &str) {
    let subject = &value["subject"];
    if subject["kind"] == "cargo_manifest" {
        (
            0,
            subject["manifest_path"].as_str().unwrap_or_default(),
            subject["manifest_id"].as_str().unwrap_or_default(),
        )
    } else {
        (
            1,
            subject["workspace"]["manifest_path"]
                .as_str()
                .unwrap_or_default(),
            subject["source_file_id"].as_str().unwrap_or_default(),
        )
    }
}

fn sort_values_by_id(values: &mut [Value]) -> Result<(), RepositorySnapshotV7Error> {
    if values.iter().any(|value| value["id"].as_str().is_none()) {
        return Err(RepositorySnapshotV7Error::ContractInvalid);
    }
    values.sort_by(|left, right| {
        left["id"]
            .as_str()
            .unwrap_or_default()
            .as_bytes()
            .cmp(right["id"].as_str().unwrap_or_default().as_bytes())
    });
    if values.windows(2).any(|pair| pair[0]["id"] == pair[1]["id"]) {
        return Err(RepositorySnapshotV7Error::ContractInvalid);
    }
    Ok(())
}

fn sort_and_dedup_values_by_id(values: &mut Vec<Value>) -> Result<(), RepositorySnapshotV7Error> {
    if values.iter().any(|value| value["id"].as_str().is_none()) {
        return Err(RepositorySnapshotV7Error::ContractInvalid);
    }
    values.sort_by(|left, right| {
        left["id"]
            .as_str()
            .unwrap_or_default()
            .as_bytes()
            .cmp(right["id"].as_str().unwrap_or_default().as_bytes())
    });
    values.dedup_by(|left, right| left["id"] == right["id"] && left == right);
    if values.windows(2).any(|pair| pair[0]["id"] == pair[1]["id"]) {
        return Err(RepositorySnapshotV7Error::ContractInvalid);
    }
    Ok(())
}

fn stored_snapshot_error(head: &LocalSnapshotHead, reason: &'static str) -> StorageError {
    StorageError::CorruptMetadata {
        component: StorageComponent::Head,
        reason,
        snapshot_id: Some(head.snapshot_id.to_string()),
    }
}
