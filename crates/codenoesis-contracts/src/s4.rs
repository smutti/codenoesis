use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::Write as _;
use std::fmt::{self, Display, Formatter};

use codenoesis_domain::s4::{
    RustWorkspaceKnowledge, S4_ONTOLOGY_VERSION, S4_TREE_SITTER_EXTRACTOR_VERSION,
    S4_WORKSPACE_EXTRACTOR_VERSION, WorkspaceClaim, WorkspaceCoverageGap, WorkspaceDiagnostic,
    WorkspaceEntity, WorkspaceEvidence, WorkspaceExtractionChunk, WorkspaceKnowledgeGraph,
    WorkspaceRelationship,
};
use codenoesis_domain::s4_r3::R3_COVERAGE_CAPABILITIES;
use codenoesis_domain::storage::{
    LocalSnapshotHead, PublicationCandidate, SNAPSHOT_SCHEMA_VERSION_V4, StorageComponent,
    StorageError,
};
use codenoesis_domain::{
    AcquisitionError, LimitKind, RepositoryInventory, STANDARD_LOCAL_S1_LIMITS, limit_exceeded,
};
use serde_json::{Value, json};

use super::{
    CONFIGURATION_HASH_DOMAIN, LimitedVecWriter, PublicationCandidateError, SnapshotEnvelopeV1,
    inventory_value, publication_candidate, semantic_hash,
};

const SNAPSHOT_V4_HASH_DOMAIN: &[u8] = b"codenoesis.repository-snapshot.semantic.v4";
const EXTRACTION_V2_HASH_DOMAIN: &[u8] = b"codenoesis.extraction-chunk.semantic.v2";
const GRAPH_V2_HASH_DOMAIN: &[u8] = b"codenoesis.knowledge-graph.semantic.v2";
const DOCUMENT_ID_DOMAIN: &str = "codenoesis.document-id/v1";
const STATEMENT_ID_DOMAIN: &str = "codenoesis.statement-id/v1";
const GENERATION_DOMAIN: &str = "codenoesis.documentation-generation/v1";
const DOCUMENT_ID_PREFIX: &str = "urn:codenoesis:document:blake3:";
const STATEMENT_ID_PREFIX: &str = "urn:codenoesis:statement:blake3:";
const MAX_DOCUMENTS: usize = 2_001;
const MAX_DOCUMENT_BYTES: usize = 1_048_576;
const MAX_TOTAL_DOCUMENT_BYTES: usize = 33_554_432;
const MAX_STATEMENTS: usize = 200_000;
const MAX_QUERY_BYTES: usize = 4_194_304;

pub const MARKDOWN_RENDERER_VERSION: &str = "codenoesis.renderer/markdown-v1";

#[derive(Clone, Debug)]
pub struct CodeNoesisErrorV5 {
    value: Value,
}

impl CodeNoesisErrorV5 {
    #[must_use]
    pub fn from_workspace(error: &codenoesis_domain::s4::WorkspaceError) -> Self {
        match error {
            codenoesis_domain::s4::WorkspaceError::AmbiguousModule => Self::new(
                "extraction.ambiguous_module",
                "extraction",
                "ambiguous module",
            ),
            codenoesis_domain::s4::WorkspaceError::InvalidUtf8 { .. }
            | codenoesis_domain::s4::WorkspaceError::UnsupportedWorkspace
            | codenoesis_domain::s4::WorkspaceError::ParserCancelled { .. }
            | codenoesis_domain::s4::WorkspaceError::MalformedSyntax { .. }
            | codenoesis_domain::s4::WorkspaceError::ContractInvalid
            | codenoesis_domain::s4::WorkspaceError::LimitExceeded { .. } => Self::new(
                "extraction.unsupported_workspace",
                "extraction",
                "unsupported workspace",
            ),
        }
    }

    #[must_use]
    pub fn invalid_output_root() -> Self {
        Self::new("input.invalid_output_root", "input", "invalid output root")
    }

    #[must_use]
    pub fn invalid_documents_root() -> Self {
        Self::new(
            "input.invalid_documents_root",
            "input",
            "invalid documents root",
        )
    }

    #[must_use]
    pub fn invalid_query_id() -> Self {
        Self::new("input.invalid_query_id", "input", "invalid query id")
    }

    #[must_use]
    pub fn docs_unmarked_nonempty_root() -> Self {
        Self::new(
            "docs.unmarked_nonempty_root",
            "docs",
            "unmarked nonempty generated-document root",
        )
    }

    #[must_use]
    pub fn docs_unsafe_path() -> Self {
        Self::new("docs.unsafe_path", "docs", "unsafe generated-document path")
    }

    #[must_use]
    pub fn docs_snapshot_mismatch() -> Self {
        Self::new("docs.snapshot_mismatch", "docs", "snapshot mismatch")
    }

    #[must_use]
    pub fn docs_corrupt_generation() -> Self {
        Self::new(
            "docs.corrupt_generation",
            "docs",
            "corrupt generated documents",
        )
    }

    #[must_use]
    pub fn docs_failed() -> Self {
        Self::new("docs.failed", "docs", "documentation generation failed")
    }

    #[must_use]
    pub fn query_not_found() -> Self {
        Self::new("query.not_found", "query", "query target not found")
    }

    #[must_use]
    pub fn query_snapshot_mismatch() -> Self {
        Self::new("query.snapshot_mismatch", "query", "snapshot mismatch")
    }

    #[must_use]
    pub fn query_corrupt_documents() -> Self {
        Self::new(
            "query.corrupt_documents",
            "query",
            "corrupt generated documents",
        )
    }

    #[must_use]
    pub fn query_result_limit_exceeded() -> Self {
        Self::new(
            "query.result_limit_exceeded",
            "query",
            "query result limit exceeded",
        )
    }

    /// Serializes one strict V5 error followed by one LF.
    ///
    /// # Errors
    ///
    /// Returns a serialization error only if the internal JSON is invalid.
    pub fn canonical_stderr(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = serde_json::to_vec(&self.value)?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    fn new(code: &str, stage: &str, message: &str) -> Self {
        Self {
            value: json!({
                "schema_version": "codenoesis.error/v5",
                "code": code,
                "stage": stage,
                "message": message,
                "retryable": false,
                "context": {}
            }),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedDocumentV1 {
    pub path: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct GeneratedDocumentationV1 {
    manifest: Value,
    documents: Vec<GeneratedDocumentV1>,
}

impl GeneratedDocumentationV1 {
    #[must_use]
    pub const fn manifest(&self) -> &Value {
        &self.manifest
    }

    #[must_use]
    pub fn documents(&self) -> &[GeneratedDocumentV1] {
        &self.documents
    }

    /// Serializes the strict documentation manifest followed by one LF.
    ///
    /// # Errors
    ///
    /// Returns a serialization error if the internal JSON is invalid.
    pub fn canonical_manifest_stdout(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = serde_json::to_vec(&self.manifest)?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Serializes the strict documentation manifest without transport LF.
    ///
    /// # Errors
    ///
    /// Returns a serialization error if the internal JSON is invalid.
    pub fn canonical_manifest_file(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(&self.manifest)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocumentationContractError {
    InvalidSnapshot,
    LimitExceeded,
}

impl Display for DocumentationContractError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidSnapshot => "invalid S4 snapshot for documentation",
            Self::LimitExceeded => "documentation limit exceeded",
        })
    }
}

impl Error for DocumentationContractError {}

/// Renders the deterministic Markdown v1 bundle from one validated S4 semantic
/// payload.
///
/// # Errors
///
/// Returns a strict snapshot or fixed-limit failure.
pub fn generate_documentation_v1(
    semantic: &Value,
    snapshot_id: &str,
    snapshot_semantic_hash: &str,
) -> Result<GeneratedDocumentationV1, DocumentationContractError> {
    let index = GraphIndex::new(semantic)?;
    let mut drafts = vec![overview_document(&index)?];
    let modules = index
        .entities
        .values()
        .filter(|entity| string_field(entity, "kind") == Ok("rust.module"))
        .collect::<Vec<_>>();
    let mut slug_counts = BTreeMap::<String, usize>::new();
    for module in &modules {
        *slug_counts
            .entry(module_slug(string_field(module, "name")?)?)
            .or_default() += 1;
    }
    for module in modules {
        let slug = module_slug(string_field(module, "name")?)?;
        let path = if slug_counts.get(&slug).copied() == Some(1) {
            format!("modules/{slug}.md")
        } else {
            let digest = string_field(module, "id")?
                .strip_prefix("urn:codenoesis:entity:blake3:")
                .ok_or(DocumentationContractError::InvalidSnapshot)?;
            format!("modules/{slug}-{digest}.md")
        };
        drafts.push(module_document(&index, module, path)?);
    }
    drafts.sort_by(|left, right| left.path.as_bytes().cmp(right.path.as_bytes()));
    if drafts.is_empty() || drafts.len() > MAX_DOCUMENTS {
        return Err(DocumentationContractError::LimitExceeded);
    }
    if drafts.windows(2).any(|pair| pair[0].path == pair[1].path) {
        return Err(DocumentationContractError::InvalidSnapshot);
    }
    let statement_count = drafts
        .iter()
        .try_fold(0_usize, |total, draft| {
            total.checked_add(draft.statements.len())
        })
        .ok_or(DocumentationContractError::LimitExceeded)?;
    let total_bytes = drafts.iter().try_fold(0_usize, |total, draft| {
        if draft.bytes.is_empty() || draft.bytes.len() > MAX_DOCUMENT_BYTES {
            return Err(DocumentationContractError::LimitExceeded);
        }
        total
            .checked_add(draft.bytes.len())
            .ok_or(DocumentationContractError::LimitExceeded)
    })?;
    if statement_count > MAX_STATEMENTS || total_bytes > MAX_TOTAL_DOCUMENT_BYTES {
        return Err(DocumentationContractError::LimitExceeded);
    }

    let mut generated = Vec::with_capacity(drafts.len());
    let mut documents = Vec::with_capacity(drafts.len());
    let mut generation_inputs = Vec::with_capacity(drafts.len());
    for draft in drafts {
        let digest = blake3::hash(&draft.bytes).to_hex().to_string();
        let byte_length = u64::try_from(draft.bytes.len())
            .map_err(|_| DocumentationContractError::LimitExceeded)?;
        generation_inputs.push(json!([draft.path, digest, byte_length]));
        documents.push(json!({
            "document_id": draft.document_id,
            "kind": draft.kind,
            "subject_id": draft.subject_id,
            "path": draft.path,
            "byte_length": byte_length,
            "blake3": digest,
            "statements": draft.statements
        }));
        generated.push(GeneratedDocumentV1 {
            path: draft.path,
            bytes: draft.bytes,
        });
    }
    let generation_hash =
        stable_digest(&json!([GENERATION_DOMAIN, snapshot_id, generation_inputs]));
    let manifest = json!({
        "schema_version": "codenoesis.documentation-manifest/v1",
        "repository_identity": index.repository_identity,
        "snapshot_id": snapshot_id,
        "snapshot_semantic_hash": {
            "algorithm": "blake3-256",
            "value": snapshot_semantic_hash
        },
        "renderer_version": MARKDOWN_RENDERER_VERSION,
        "generation_hash": {
            "algorithm": "blake3-256",
            "value": generation_hash
        },
        "documents": documents
    });
    Ok(GeneratedDocumentationV1 {
        manifest,
        documents: generated,
    })
}

/// Validates one complete generated-document generation against exact bytes.
///
/// # Errors
///
/// Returns a strict binding, integrity, grounding, or fixed-limit failure.
#[allow(clippy::too_many_lines)]
pub fn validate_documentation_bundle_v1(
    manifest: &Value,
    document_bytes: &BTreeMap<String, Vec<u8>>,
    repository_identity: &str,
    snapshot_id: &str,
    snapshot_semantic_hash: &str,
) -> Result<(), DocumentationContractError> {
    if manifest.get("schema_version").and_then(Value::as_str)
        != Some("codenoesis.documentation-manifest/v1")
        || manifest.get("repository_identity").and_then(Value::as_str) != Some(repository_identity)
        || manifest.get("snapshot_id").and_then(Value::as_str) != Some(snapshot_id)
        || manifest
            .pointer("/snapshot_semantic_hash/algorithm")
            .and_then(Value::as_str)
            != Some("blake3-256")
        || manifest
            .pointer("/snapshot_semantic_hash/value")
            .and_then(Value::as_str)
            != Some(snapshot_semantic_hash)
        || manifest.get("renderer_version").and_then(Value::as_str)
            != Some(MARKDOWN_RENDERER_VERSION)
    {
        return Err(DocumentationContractError::InvalidSnapshot);
    }
    let documents = manifest
        .get("documents")
        .and_then(Value::as_array)
        .ok_or(DocumentationContractError::InvalidSnapshot)?;
    if documents.is_empty()
        || documents.len() > MAX_DOCUMENTS
        || documents.len() != document_bytes.len()
    {
        return Err(DocumentationContractError::LimitExceeded);
    }
    let mut previous_path = None;
    let mut generation_inputs = Vec::with_capacity(documents.len());
    let mut statement_ids = std::collections::BTreeSet::new();
    let mut total_bytes = 0_usize;
    for document in documents {
        let path = string_field(document, "path")?;
        if previous_path.is_some_and(|previous| previous >= path) || !valid_document_path(path) {
            return Err(DocumentationContractError::InvalidSnapshot);
        }
        previous_path = Some(path);
        let bytes = document_bytes
            .get(path)
            .ok_or(DocumentationContractError::InvalidSnapshot)?;
        if bytes.is_empty() || bytes.len() > MAX_DOCUMENT_BYTES {
            return Err(DocumentationContractError::LimitExceeded);
        }
        total_bytes = total_bytes
            .checked_add(bytes.len())
            .ok_or(DocumentationContractError::LimitExceeded)?;
        let digest = blake3::hash(bytes).to_hex().to_string();
        if document.get("byte_length").and_then(Value::as_u64) != u64::try_from(bytes.len()).ok()
            || document.get("blake3").and_then(Value::as_str) != Some(&digest)
        {
            return Err(DocumentationContractError::InvalidSnapshot);
        }
        generation_inputs.push(json!([
            path,
            digest,
            u64::try_from(bytes.len()).map_err(|_| DocumentationContractError::LimitExceeded)?
        ]));
        let markdown =
            std::str::from_utf8(bytes).map_err(|_| DocumentationContractError::InvalidSnapshot)?;
        let statements = document
            .get("statements")
            .and_then(Value::as_array)
            .ok_or(DocumentationContractError::InvalidSnapshot)?;
        for statement in statements {
            let statement_id = string_field(statement, "statement_id")?;
            if !statement_ids.insert(statement_id)
                || !markdown.contains(&format!("<!-- statement:{statement_id} -->"))
            {
                return Err(DocumentationContractError::InvalidSnapshot);
            }
            let evidence = statement
                .get("evidence_ids")
                .and_then(Value::as_array)
                .ok_or(DocumentationContractError::InvalidSnapshot)?;
            let coverage = statement
                .get("coverage_gap_ids")
                .and_then(Value::as_array)
                .ok_or(DocumentationContractError::InvalidSnapshot)?;
            let truth = string_field(statement, "truth_state")?;
            if (evidence.is_empty() == coverage.is_empty())
                || coverage.is_empty() && !matches!(truth, "deterministic_fact" | "derived_fact")
                || !coverage.is_empty() && truth != "unsupported"
            {
                return Err(DocumentationContractError::InvalidSnapshot);
            }
        }
    }
    if total_bytes > MAX_TOTAL_DOCUMENT_BYTES || statement_ids.len() > MAX_STATEMENTS {
        return Err(DocumentationContractError::LimitExceeded);
    }
    let observed_generation_hash =
        stable_digest(&json!([GENERATION_DOMAIN, snapshot_id, generation_inputs]));
    if manifest
        .pointer("/generation_hash/algorithm")
        .and_then(Value::as_str)
        != Some("blake3-256")
        || manifest
            .pointer("/generation_hash/value")
            .and_then(Value::as_str)
            != Some(&observed_generation_hash)
    {
        return Err(DocumentationContractError::InvalidSnapshot);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryContractError {
    InvalidSnapshot,
    InvalidDocuments,
    NotFound,
    LimitExceeded,
}

impl Display for QueryContractError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidSnapshot => "invalid S4 query snapshot",
            Self::InvalidDocuments => "invalid generated documents",
            Self::NotFound => "query target not found",
            Self::LimitExceeded => "query result limit exceeded",
        })
    }
}

impl Error for QueryContractError {}

#[derive(Clone, Debug)]
pub struct LocalQueryResultV1 {
    value: Value,
}

impl LocalQueryResultV1 {
    /// Serializes one bounded exact-ID result followed by one LF.
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

/// Builds one exact-ID local query result from validated snapshot and
/// documentation contracts.
///
/// # Errors
///
/// Returns a strict snapshot, document, not-found, or output-limit failure.
pub fn local_query_result_v1(
    semantic: &Value,
    manifest: &Value,
    snapshot_id: &str,
    requested_id: &str,
) -> Result<LocalQueryResultV1, QueryContractError> {
    let index = GraphIndex::new(semantic).map_err(|_| QueryContractError::InvalidSnapshot)?;
    validate_manifest_binding(manifest, &index.repository_identity, snapshot_id)?;
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
            "schema_version": "codenoesis.local-query-result/v1",
            "repository_identity": index.repository_identity,
            "snapshot_id": snapshot_id,
            "requested_id": requested_id,
            "result_kind": "entity",
            "entity": entity,
            "claims": [claim],
            "evidence": evidence,
            "document": null,
            "document_statements": linked_statements(documents, requested_id)?
        })
    } else if let Some(claim) = index.claims_by_id.get(requested_id) {
        let evidence = evidence_for_claim(&index, claim)?;
        let entity = if string_field(claim, "subject_kind").is_ok_and(|kind| kind == "entity") {
            string_field(claim, "subject_id")
                .ok()
                .and_then(|id| index.entities.get(id))
                .cloned()
        } else {
            None
        };
        json!({
            "schema_version": "codenoesis.local-query-result/v1",
            "repository_identity": index.repository_identity,
            "snapshot_id": snapshot_id,
            "requested_id": requested_id,
            "result_kind": "claim",
            "entity": entity,
            "claims": [claim],
            "evidence": evidence,
            "document": null,
            "document_statements": linked_statements(documents, requested_id)?
        })
    } else if let Some(evidence) = index.evidence.get(requested_id) {
        json!({
            "schema_version": "codenoesis.local-query-result/v1",
            "repository_identity": index.repository_identity,
            "snapshot_id": snapshot_id,
            "requested_id": requested_id,
            "result_kind": "evidence",
            "entity": null,
            "claims": [],
            "evidence": [evidence],
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
            "schema_version": "codenoesis.local-query-result/v1",
            "repository_identity": index.repository_identity,
            "snapshot_id": snapshot_id,
            "requested_id": requested_id,
            "result_kind": "document",
            "entity": null,
            "claims": [],
            "evidence": [],
            "document": Value::Object(record),
            "document_statements": statements
        })
    } else {
        return Err(QueryContractError::NotFound);
    };
    let result = LocalQueryResultV1 { value };
    result.canonical_stdout()?;
    Ok(result)
}

struct DocumentDraft {
    document_id: String,
    kind: &'static str,
    subject_id: String,
    path: String,
    bytes: Vec<u8>,
    statements: Vec<Value>,
}

struct GraphIndex {
    repository_identity: String,
    entities: BTreeMap<String, Value>,
    relationships: Vec<Value>,
    claims: BTreeMap<(String, String), Value>,
    claims_by_id: BTreeMap<String, Value>,
    evidence: BTreeMap<String, Value>,
    coverage: BTreeMap<String, Value>,
}

impl GraphIndex {
    fn new(semantic: &Value) -> Result<Self, DocumentationContractError> {
        let repository_identity = semantic
            .pointer("/repository/identity")
            .and_then(Value::as_str)
            .ok_or(DocumentationContractError::InvalidSnapshot)?
            .to_owned();
        let graph = semantic
            .get("knowledge_graph")
            .ok_or(DocumentationContractError::InvalidSnapshot)?;
        let entities = id_map(graph, "entities")?;
        let relationships = graph
            .get("relationships")
            .and_then(Value::as_array)
            .cloned()
            .ok_or(DocumentationContractError::InvalidSnapshot)?;
        let claims_values = graph
            .get("claims")
            .and_then(Value::as_array)
            .ok_or(DocumentationContractError::InvalidSnapshot)?;
        let mut claims = BTreeMap::new();
        let mut claims_by_id = BTreeMap::new();
        for claim in claims_values {
            let subject_kind = string_field(claim, "subject_kind")?.to_owned();
            let subject_id = string_field(claim, "subject_id")?.to_owned();
            let id = string_field(claim, "id")?.to_owned();
            if claims
                .insert((subject_kind, subject_id), claim.clone())
                .is_some()
                || claims_by_id.insert(id, claim.clone()).is_some()
            {
                return Err(DocumentationContractError::InvalidSnapshot);
            }
        }
        Ok(Self {
            repository_identity,
            entities,
            relationships,
            claims,
            claims_by_id,
            evidence: id_map(graph, "evidence")?,
            coverage: id_map(graph, "coverage")?,
        })
    }

    fn claim(
        &self,
        subject_kind: &str,
        subject_id: &str,
    ) -> Result<&Value, DocumentationContractError> {
        self.claims
            .get(&(subject_kind.to_owned(), subject_id.to_owned()))
            .ok_or(DocumentationContractError::InvalidSnapshot)
    }
}

#[allow(clippy::too_many_lines)]
fn overview_document(index: &GraphIndex) -> Result<DocumentDraft, DocumentationContractError> {
    let mut crates = index
        .entities
        .values()
        .filter(|entity| string_field(entity, "kind") == Ok("rust.crate"))
        .collect::<Vec<_>>();
    crates.sort_by(|left, right| {
        string_field(left, "name")
            .unwrap_or_default()
            .as_bytes()
            .cmp(string_field(right, "name").unwrap_or_default().as_bytes())
    });
    if crates.is_empty() {
        return Err(DocumentationContractError::InvalidSnapshot);
    }
    let document_id = document_id(
        &index.repository_identity,
        "overview",
        &index.repository_identity,
    );
    let root_evidence = index
        .evidence
        .values()
        .find(|evidence| string_field(evidence, "path") == Ok("Cargo.toml"))
        .and_then(|evidence| string_field(evidence, "id").ok())
        .ok_or(DocumentationContractError::InvalidSnapshot)?;
    let crate_ids = crates
        .iter()
        .map(|entity| string_field(entity, "id").map(str::to_owned))
        .collect::<Result<Vec<_>, _>>()?;
    let mut crate_manifest_evidence = Vec::with_capacity(crates.len());
    for entity in &crates {
        let manifest_path = entity
            .pointer("/properties/manifest_path")
            .and_then(Value::as_str)
            .ok_or(DocumentationContractError::InvalidSnapshot)?;
        let evidence_id = index
            .evidence
            .values()
            .find(|evidence| string_field(evidence, "path") == Ok(manifest_path))
            .and_then(|evidence| string_field(evidence, "id").ok())
            .ok_or(DocumentationContractError::InvalidSnapshot)?;
        crate_manifest_evidence.push(evidence_id.to_owned());
    }
    let repository_statement = statement_value(
        &document_id,
        "repository_identity",
        &index.repository_identity,
        0,
        "deterministic_fact",
        vec![index.repository_identity.clone()],
        vec![root_evidence.to_owned()],
        Vec::new(),
    );
    let mut count_evidence = vec![root_evidence.to_owned()];
    count_evidence.extend(crate_manifest_evidence.iter().cloned());
    let count_statement = statement_value(
        &document_id,
        "crate_count",
        &index.repository_identity,
        0,
        "derived_fact",
        crate_ids,
        count_evidence,
        Vec::new(),
    );
    let mut statements = vec![repository_statement.clone(), count_statement.clone()];
    let mut content = format!(
        "# Workspace overview\n\nRepository: `{}`. {}\n\nCrates: {}. {}\n\n## Crates\n\n",
        markdown_code(&index.repository_identity),
        statement_marker(&repository_statement)?,
        crates.len(),
        statement_marker(&count_statement)?
    );
    let crate_count = crates.len();
    for (position, (entity, evidence_id)) in crates.iter().zip(crate_manifest_evidence).enumerate()
    {
        let id = string_field(entity, "id")?;
        let ordinal = crate_count
            .checked_sub(position + 1)
            .ok_or(DocumentationContractError::InvalidSnapshot)?;
        let statement = statement_value(
            &document_id,
            "crate",
            id,
            ordinal,
            "deterministic_fact",
            vec![id.to_owned()],
            vec![evidence_id],
            Vec::new(),
        );
        let properties = entity
            .get("properties")
            .ok_or(DocumentationContractError::InvalidSnapshot)?;
        let package_name = property(properties, "package_name")?;
        let target_name = property(properties, "target_name")?;
        let manifest_path = property(properties, "manifest_path")?;
        let target_kind = property(properties, "target_kind")?;
        let target_label = match target_kind {
            "bin" => "binary",
            "lib" => "library",
            _ => return Err(DocumentationContractError::InvalidSnapshot),
        };
        writeln!(
            content,
            "- `{}`: {target_label} target `{}` from `{}`. {}",
            markdown_code(package_name),
            markdown_code(target_name),
            markdown_code(manifest_path),
            statement_marker(&statement)?
        )
        .expect("writing Markdown to a String cannot fail");
        statements.push(statement);
    }
    let mut gaps = index
        .coverage
        .values()
        .filter(|gap| {
            string_field(gap, "capability").is_ok_and(|capability| {
                capability == "compiler_cross_crate_use_resolution"
                    || R3_COVERAGE_CAPABILITIES.contains(&capability)
            })
        })
        .collect::<Vec<_>>();
    gaps.sort_by(|left, right| {
        string_field(left, "id")
            .unwrap_or_default()
            .cmp(string_field(right, "id").unwrap_or_default())
    });
    if !gaps.is_empty() {
        content.push_str("\n## Coverage\n\n");
        for (ordinal, gap) in gaps.into_iter().enumerate() {
            let id = string_field(gap, "id")?;
            let capability = string_field(gap, "capability")?;
            let statement = statement_value(
                &document_id,
                "coverage_gap",
                id,
                ordinal,
                "unsupported",
                vec![id.to_owned()],
                Vec::new(),
                vec![id.to_owned()],
            );
            let message = if capability == "compiler_cross_crate_use_resolution" {
                "compiler-grade cross-crate Rust use resolution is not available".to_owned()
            } else {
                format!("capability `{}` is deferred", markdown_code(capability))
            };
            writeln!(
                content,
                "- Unsupported: {message}. {}",
                statement_marker(&statement)?
            )
            .expect("writing Markdown to a String cannot fail");
            statements.push(statement);
        }
    }
    Ok(DocumentDraft {
        document_id,
        kind: "overview",
        subject_id: index.repository_identity.clone(),
        path: "overview.md".to_owned(),
        bytes: content.into_bytes(),
        statements,
    })
}

#[allow(clippy::too_many_lines)]
fn module_document(
    index: &GraphIndex,
    module: &Value,
    path: String,
) -> Result<DocumentDraft, DocumentationContractError> {
    let module_id = string_field(module, "id")?;
    let module_name = string_field(module, "name")?;
    let module_path = string_field(module, "module_path")?;
    let source_file_id = module
        .pointer("/properties/source_file_id")
        .and_then(Value::as_str)
        .ok_or(DocumentationContractError::InvalidSnapshot)?;
    let source_file = index
        .entities
        .get(source_file_id)
        .ok_or(DocumentationContractError::InvalidSnapshot)?;
    let source_path = source_file
        .pointer("/properties/path")
        .and_then(Value::as_str)
        .ok_or(DocumentationContractError::InvalidSnapshot)?;
    let document_id = document_id(&index.repository_identity, "module", module_id);
    let module_claim = index.claim("entity", module_id)?;
    let mut module_evidence = string_array(module_claim, "evidence_ids")?;
    if module_path == "crate" {
        module_evidence = module_evidence
            .last()
            .cloned()
            .into_iter()
            .collect::<Vec<_>>();
    }
    let module_statement = statement_value(
        &document_id,
        "module",
        module_id,
        0,
        "deterministic_fact",
        vec![module_id.to_owned()],
        module_evidence,
        Vec::new(),
    );
    let mut content = format!(
        "# Module `{}`\n\nModule path: `{}` in `{}`. {}\n\n",
        markdown_code(module_name),
        markdown_code(module_path),
        markdown_code(source_path),
        statement_marker(&module_statement)?
    );
    let mut statements = vec![module_statement];
    let mut declarations = index
        .relationships
        .iter()
        .filter(|relationship| {
            string_field(relationship, "kind") == Ok("DEFINES")
                && string_field(relationship, "source") == Ok(module_id)
        })
        .filter_map(|relationship| {
            let target = string_field(relationship, "target").ok()?;
            let entity = index.entities.get(target)?;
            (string_field(entity, "kind").ok()? != "rust.module").then_some((relationship, entity))
        })
        .collect::<Vec<_>>();
    declarations.sort_by(|(_, left), (_, right)| {
        (
            string_field(left, "kind").unwrap_or_default(),
            string_field(left, "name").unwrap_or_default(),
        )
            .cmp(&(
                string_field(right, "kind").unwrap_or_default(),
                string_field(right, "name").unwrap_or_default(),
            ))
    });
    let mut ordinals = BTreeMap::<&str, usize>::new();
    for (_, entity) in declarations {
        let entity_id = string_field(entity, "id")?;
        let kind = string_field(entity, "kind")?
            .strip_prefix("rust.")
            .ok_or(DocumentationContractError::InvalidSnapshot)?;
        let ordinal = ordinals.entry(kind).or_default();
        let claim = index.claim("entity", entity_id)?;
        let statement = statement_value(
            &document_id,
            kind,
            entity_id,
            *ordinal,
            "deterministic_fact",
            vec![entity_id.to_owned()],
            string_array(claim, "evidence_ids")?,
            Vec::new(),
        );
        *ordinal += 1;
        let visibility = match string_field(entity, "visibility")? {
            "public" => "Public",
            "private" => "Private",
            _ => return Err(DocumentationContractError::InvalidSnapshot),
        };
        let display_kind = if kind == "type_alias" {
            "type alias"
        } else {
            kind
        };
        writeln!(
            content,
            "- {visibility} {display_kind}: `{}`. {}",
            markdown_code(string_field(entity, "name")?),
            statement_marker(&statement)?
        )
        .expect("writing Markdown to a String cannot fail");
        statements.push(statement);
    }

    let mut imports = index
        .relationships
        .iter()
        .filter(|relationship| {
            string_field(relationship, "kind") == Ok("IMPORTS")
                && string_field(relationship, "source") == Ok(module_id)
        })
        .collect::<Vec<_>>();
    imports.sort_by(|left, right| {
        string_field(left, "id")
            .unwrap_or_default()
            .cmp(string_field(right, "id").unwrap_or_default())
    });
    let mut import_ordinal = 0_usize;
    let mut gap_ordinal = 0_usize;
    for relationship in imports {
        let target_id = string_field(relationship, "target")?;
        let target = index
            .entities
            .get(target_id)
            .ok_or(DocumentationContractError::InvalidSnapshot)?;
        if string_field(target, "kind")? == "rust.symbol_reference" {
            let gap = index
                .coverage
                .values()
                .find(|gap| {
                    string_field(gap, "capability") == Ok("compiler_cross_crate_use_resolution")
                })
                .ok_or(DocumentationContractError::InvalidSnapshot)?;
            let gap_id = string_field(gap, "id")?;
            let statement = statement_value(
                &document_id,
                "coverage_gap",
                gap_id,
                gap_ordinal,
                "unsupported",
                vec![gap_id.to_owned()],
                Vec::new(),
                vec![gap_id.to_owned()],
            );
            gap_ordinal += 1;
            writeln!(
                content,
                "- Unsupported: `{}` remains an unresolved cross-crate symbol. {}",
                markdown_code(string_field(target, "name")?),
                statement_marker(&statement)?
            )
            .expect("writing Markdown to a String cannot fail");
            statements.push(statement);
        } else {
            let relationship_id = string_field(relationship, "id")?;
            let claim = index.claim("relationship", relationship_id)?;
            let statement = statement_value(
                &document_id,
                "import",
                target_id,
                import_ordinal,
                "derived_fact",
                vec![target_id.to_owned()],
                string_array(claim, "evidence_ids")?,
                Vec::new(),
            );
            import_ordinal += 1;
            writeln!(
                content,
                "- Public re-export: `{}` from `{}`. {}",
                markdown_code(string_field(target, "name")?),
                markdown_code(string_field(target, "module_path")?),
                statement_marker(&statement)?
            )
            .expect("writing Markdown to a String cannot fail");
            statements.push(statement);
        }
    }
    Ok(DocumentDraft {
        document_id,
        kind: "module",
        subject_id: module_id.to_owned(),
        path,
        bytes: content.into_bytes(),
        statements,
    })
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::needless_pass_by_value)]
fn statement_value(
    document_id: &str,
    kind: &str,
    stable_subject_id: &str,
    ordinal: usize,
    truth_state: &str,
    subject_ids: Vec<String>,
    evidence_ids: Vec<String>,
    coverage_gap_ids: Vec<String>,
) -> Value {
    let statement_id = stable_contract_id(
        STATEMENT_ID_PREFIX,
        &[
            STATEMENT_ID_DOMAIN,
            document_id,
            kind,
            stable_subject_id,
            &ordinal.to_string(),
        ],
    );
    json!({
        "statement_id": statement_id,
        "truth_state": truth_state,
        "subject_ids": subject_ids,
        "evidence_ids": evidence_ids,
        "coverage_gap_ids": coverage_gap_ids
    })
}

fn document_id(repository_identity: &str, kind: &str, subject_id: &str) -> String {
    stable_contract_id(
        DOCUMENT_ID_PREFIX,
        &[
            DOCUMENT_ID_DOMAIN,
            repository_identity,
            kind,
            subject_id,
            MARKDOWN_RENDERER_VERSION,
        ],
    )
}

fn statement_marker(statement: &Value) -> Result<String, DocumentationContractError> {
    Ok(format!(
        "<!-- statement:{} -->",
        string_field(statement, "statement_id")?
    ))
}

fn module_slug(name: &str) -> Result<String, DocumentationContractError> {
    let name = name.strip_prefix("workspace_").unwrap_or(name);
    let mut slug = String::new();
    let mut separator = false;
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            if separator && !slug.is_empty() {
                slug.push('-');
            }
            slug.push(character.to_ascii_lowercase());
            separator = false;
        } else if matches!(character, '_' | ':' | '-') {
            separator = true;
        } else {
            return Err(DocumentationContractError::InvalidSnapshot);
        }
    }
    if slug.is_empty() {
        Err(DocumentationContractError::InvalidSnapshot)
    } else {
        Ok(slug)
    }
}

fn valid_document_path(path: &str) -> bool {
    path == "overview.md"
        || path
            .strip_prefix("modules/")
            .and_then(|value| value.strip_suffix(".md"))
            .is_some_and(|slug| {
                !slug.is_empty()
                    && slug.bytes().all(|byte| {
                        byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'
                    })
            })
}

fn markdown_code(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('`', "&#96;")
        .replace('[', "&#91;")
        .replace(']', "&#93;")
}

fn id_map(
    parent: &Value,
    field: &str,
) -> Result<BTreeMap<String, Value>, DocumentationContractError> {
    let values = parent
        .get(field)
        .and_then(Value::as_array)
        .ok_or(DocumentationContractError::InvalidSnapshot)?;
    let mut result = BTreeMap::new();
    for value in values {
        let id = string_field(value, "id")?.to_owned();
        if result.insert(id, value.clone()).is_some() {
            return Err(DocumentationContractError::InvalidSnapshot);
        }
    }
    Ok(result)
}

fn string_field<'a>(value: &'a Value, field: &str) -> Result<&'a str, DocumentationContractError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or(DocumentationContractError::InvalidSnapshot)
}

fn property<'a>(properties: &'a Value, field: &str) -> Result<&'a str, DocumentationContractError> {
    string_field(properties, field)
}

fn string_array(value: &Value, field: &str) -> Result<Vec<String>, DocumentationContractError> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or(DocumentationContractError::InvalidSnapshot)?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_owned)
                .ok_or(DocumentationContractError::InvalidSnapshot)
        })
        .collect()
}

fn stable_contract_id(prefix: &str, components: &[&str]) -> String {
    let value = Value::Array(
        components
            .iter()
            .map(|component| Value::String((*component).to_owned()))
            .collect(),
    );
    format!("{prefix}{}", stable_digest(&value))
}

fn stable_digest(value: &Value) -> String {
    blake3::hash(
        &serde_json::to_vec(value).expect("contract identity JSON serialization cannot fail"),
    )
    .to_hex()
    .to_string()
}

fn validate_manifest_binding(
    manifest: &Value,
    repository_identity: &str,
    snapshot_id: &str,
) -> Result<(), QueryContractError> {
    if manifest.get("schema_version").and_then(Value::as_str)
        != Some("codenoesis.documentation-manifest/v1")
        || manifest.get("repository_identity").and_then(Value::as_str) != Some(repository_identity)
        || manifest.get("snapshot_id").and_then(Value::as_str) != Some(snapshot_id)
        || manifest.get("renderer_version").and_then(Value::as_str)
            != Some(MARKDOWN_RENDERER_VERSION)
    {
        return Err(QueryContractError::InvalidDocuments);
    }
    Ok(())
}

fn evidence_for_claim(index: &GraphIndex, claim: &Value) -> Result<Vec<Value>, QueryContractError> {
    string_array(claim, "evidence_ids")
        .map_err(|_| QueryContractError::InvalidSnapshot)?
        .into_iter()
        .map(|id| {
            index
                .evidence
                .get(&id)
                .cloned()
                .ok_or(QueryContractError::InvalidSnapshot)
        })
        .collect()
}

fn linked_statements(
    documents: &[Value],
    requested_id: &str,
) -> Result<Vec<Value>, QueryContractError> {
    let mut linked = Vec::new();
    for document in documents {
        let document_id = document
            .get("document_id")
            .and_then(Value::as_str)
            .ok_or(QueryContractError::InvalidDocuments)?;
        let statements = document
            .get("statements")
            .and_then(Value::as_array)
            .ok_or(QueryContractError::InvalidDocuments)?;
        for statement in statements {
            let references_subject = statement
                .get("subject_ids")
                .and_then(Value::as_array)
                .is_some_and(|ids| ids.iter().any(|id| id.as_str() == Some(requested_id)));
            let references_evidence = statement
                .get("evidence_ids")
                .and_then(Value::as_array)
                .is_some_and(|ids| ids.iter().any(|id| id.as_str() == Some(requested_id)));
            let references_coverage = statement
                .get("coverage_gap_ids")
                .and_then(Value::as_array)
                .is_some_and(|ids| ids.iter().any(|id| id.as_str() == Some(requested_id)));
            if references_subject || references_evidence || references_coverage {
                let mut value = statement
                    .as_object()
                    .cloned()
                    .ok_or(QueryContractError::InvalidDocuments)?;
                value.insert(
                    "document_id".to_owned(),
                    Value::String(document_id.to_owned()),
                );
                linked.push(Value::Object(value));
            }
        }
    }
    Ok(linked)
}

#[derive(Clone, Debug)]
pub struct RepositorySnapshotV4 {
    value: Value,
}

#[derive(Debug)]
pub enum RepositorySnapshotV4Error {
    Serialization(serde_json::Error),
    LimitExceeded(AcquisitionError),
    OutputLengthOverflow,
}

impl RepositorySnapshotV4 {
    #[must_use]
    pub fn from_inventory_and_workspace(
        inventory: &RepositoryInventory,
        knowledge: &RustWorkspaceKnowledge,
        envelope: SnapshotEnvelopeV1,
    ) -> Self {
        let SnapshotEnvelopeV1 {
            created_at,
            job_id,
            correlation_id,
        } = envelope;
        let configuration_semantic = json!({"profile": "standard-local-s4"});
        let configuration_hash = semantic_hash(CONFIGURATION_HASH_DOMAIN, &configuration_semantic);
        let bound = inventory.bound_revision();
        let extraction_chunks = knowledge
            .extraction_chunks
            .iter()
            .map(extraction_chunk_value)
            .collect::<Vec<_>>();
        let graph = knowledge_graph_value(&knowledge.graph);
        let semantic = json!({
            "repository": {
                "contract_version": "codenoesis.repository/v1",
                "identity_schema_version": "codenoesis.repository-identity/v1",
                "identity": bound.repository_identity().as_str(),
                "vcs": "git",
                "object_format": "sha1",
                "commit_oid": bound.commit_oid().as_str(),
                "tree_oid": bound.tree_oid().as_str()
            },
            "configuration": {
                "schema_version": "codenoesis.configuration/v1",
                "profile": "standard-local-s4",
                "semantic_hash": {
                    "algorithm": "blake3-256",
                    "value": configuration_hash
                }
            },
            "pipeline_version": "codenoesis.pipeline/s4-v1",
            "ontology_version": S4_ONTOLOGY_VERSION,
            "extractor_contract_version": "codenoesis.extraction/v2",
            "extractor_versions": [
                "codenoesis.inventory-classifier/s1-v1",
                S4_TREE_SITTER_EXTRACTOR_VERSION,
                S4_WORKSPACE_EXTRACTOR_VERSION
            ],
            "evidence_lineage_version": "codenoesis.evidence-lineage/v2",
            "inventory": inventory_value(inventory),
            "extraction_chunks": extraction_chunks,
            "knowledge_graph": graph
        });
        let snapshot_hash = semantic_hash(SNAPSHOT_V4_HASH_DOMAIN, &semantic);
        Self {
            value: json!({
                "schema_version": "codenoesis.repository-snapshot/v4",
                "semantic_hash": {
                    "algorithm": "blake3-256",
                    "value": snapshot_hash
                },
                "semantic": semantic,
                "envelope": {
                    "created_at": created_at,
                    "job_id": job_id,
                    "correlation_id": correlation_id
                }
            }),
        }
    }

    /// Serializes the complete V4 snapshot with the inherited output bound.
    ///
    /// # Errors
    ///
    /// Returns a typed serialization or output-limit failure.
    pub fn canonical_stdout(&self) -> Result<Vec<u8>, RepositorySnapshotV4Error> {
        let maximum = usize::try_from(STANDARD_LOCAL_S1_LIMITS.canonical_output_bytes)
            .map_err(|_| RepositorySnapshotV4Error::OutputLengthOverflow)?;
        let body_maximum = maximum
            .checked_sub(1)
            .ok_or(RepositorySnapshotV4Error::OutputLengthOverflow)?;
        let mut writer = LimitedVecWriter::new(body_maximum);
        let result = serde_json::to_writer(&mut writer, &self.value);
        if writer.overflowed() {
            return Err(RepositorySnapshotV4Error::LimitExceeded(limit_exceeded(
                LimitKind::CanonicalOutputBytes,
                STANDARD_LOCAL_S1_LIMITS
                    .canonical_output_bytes
                    .saturating_add(1),
            )));
        }
        result.map_err(RepositorySnapshotV4Error::Serialization)?;
        let mut bytes = writer.into_inner();
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Serializes the exact semantic payload stored by S4.
    ///
    /// # Errors
    ///
    /// Returns a serialization error if the internally built JSON is invalid.
    pub fn canonical_semantic(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(&self.value["semantic"])
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Converts the strict V4 document into the immutable S3 store model.
    ///
    /// # Errors
    ///
    /// Returns a contract, serialization, or storage-integrity failure.
    pub fn publication_candidate(&self) -> Result<PublicationCandidate, PublicationCandidateError> {
        publication_candidate(&self.value)
    }
}

/// Validates a loaded V4 semantic payload against every field and immutable
/// artifact reference in its visible local-store head.
///
/// # Errors
///
/// Returns a typed metadata-integrity failure when the semantic contract or
/// any snapshot, graph, or extraction artifact binding differs from the head.
pub fn validate_stored_snapshot_semantic_v4(
    semantic: &Value,
    head: &LocalSnapshotHead,
) -> Result<(), StorageError> {
    if head.snapshot_schema_version != SNAPSHOT_SCHEMA_VERSION_V4 {
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

fn stored_snapshot_error(head: &LocalSnapshotHead, reason: &'static str) -> StorageError {
    StorageError::CorruptMetadata {
        component: StorageComponent::Head,
        reason,
        snapshot_id: Some(head.snapshot_id.to_string()),
    }
}

pub(crate) fn extraction_chunk_value(chunk: &WorkspaceExtractionChunk) -> Value {
    let mut value = json!({
        "schema_version": "codenoesis.extraction-chunk/v2",
        "ontology_version": S4_ONTOLOGY_VERSION,
        "repository_identity": chunk.repository_identity,
        "crate_id": chunk.crate_id,
        "source_file_id": chunk.source_file_id,
        "entities": chunk.entities.iter().map(entity_value).collect::<Vec<_>>(),
        "relationships": chunk.relationships.iter().map(relationship_value).collect::<Vec<_>>(),
        "claims": chunk.claims.iter().map(claim_value).collect::<Vec<_>>(),
        "evidence": chunk.evidence.iter().map(evidence_value).collect::<Vec<_>>(),
        "diagnostics": chunk.diagnostics.iter().map(diagnostic_value).collect::<Vec<_>>(),
        "coverage": chunk.coverage.iter().map(coverage_value).collect::<Vec<_>>()
    });
    let hash = semantic_hash(EXTRACTION_V2_HASH_DOMAIN, &value);
    value
        .as_object_mut()
        .expect("extraction chunk is an object")
        .insert(
            "semantic_hash".to_owned(),
            json!({"algorithm": "blake3-256", "value": hash}),
        );
    value
}

pub(crate) fn knowledge_graph_value(graph: &WorkspaceKnowledgeGraph) -> Value {
    let mut value = json!({
        "schema_version": "codenoesis.knowledge-graph/v2",
        "ontology_version": S4_ONTOLOGY_VERSION,
        "repository": {
            "identity": graph.repository_identity,
            "commit_oid": graph.commit_oid
        },
        "extractor_versions": [
            S4_TREE_SITTER_EXTRACTOR_VERSION,
            S4_WORKSPACE_EXTRACTOR_VERSION
        ],
        "entities": graph.entities.iter().map(entity_value).collect::<Vec<_>>(),
        "relationships": graph.relationships.iter().map(relationship_value).collect::<Vec<_>>(),
        "claims": graph.claims.iter().map(claim_value).collect::<Vec<_>>(),
        "evidence": graph.evidence.iter().map(evidence_value).collect::<Vec<_>>(),
        "diagnostics": graph.diagnostics.iter().map(diagnostic_value).collect::<Vec<_>>(),
        "coverage": graph.coverage.iter().map(coverage_value).collect::<Vec<_>>()
    });
    let hash = semantic_hash(GRAPH_V2_HASH_DOMAIN, &value);
    value
        .as_object_mut()
        .expect("knowledge graph is an object")
        .insert(
            "semantic_hash".to_owned(),
            json!({"algorithm": "blake3-256", "value": hash}),
        );
    value
}

pub(crate) fn entity_value(entity: &WorkspaceEntity) -> Value {
    json!({
        "id": entity.id,
        "kind": entity.kind.as_str(),
        "crate_id": entity.crate_id,
        "module_path": entity.module_path,
        "name": entity.name,
        "visibility": entity.visibility.as_str(),
        "properties": properties_value(&entity.properties)
    })
}

pub(crate) fn relationship_value(relationship: &WorkspaceRelationship) -> Value {
    json!({
        "id": relationship.id,
        "kind": relationship.kind.as_str(),
        "source": relationship.source,
        "target": relationship.target,
        "evidence_ids": relationship.evidence_ids
    })
}

pub(crate) fn claim_value(claim: &WorkspaceClaim) -> Value {
    json!({
        "id": claim.id,
        "subject_kind": claim.subject_kind.as_str(),
        "subject_id": claim.subject_id,
        "state": claim.state.as_str(),
        "evidence_ids": claim.evidence_ids
    })
}

pub(crate) fn evidence_value(evidence: &WorkspaceEvidence) -> Value {
    json!({
        "id": evidence.id,
        "path": evidence.path,
        "blob_oid": evidence.blob_oid,
        "start_byte": evidence.start_byte,
        "end_byte": evidence.end_byte
    })
}

pub(crate) fn diagnostic_value(diagnostic: &WorkspaceDiagnostic) -> Value {
    json!({
        "code": diagnostic.code,
        "message": diagnostic.message,
        "evidence_ids": diagnostic.evidence_ids
    })
}

pub(crate) fn coverage_value(gap: &WorkspaceCoverageGap) -> Value {
    json!({
        "id": gap.id,
        "capability": gap.capability,
        "state": "unsupported",
        "evidence_ids": gap.evidence_ids
    })
}

fn properties_value(properties: &BTreeMap<String, String>) -> Value {
    Value::Object(
        properties
            .iter()
            .map(|(key, value)| (key.clone(), Value::String(value.clone())))
            .collect(),
    )
}
