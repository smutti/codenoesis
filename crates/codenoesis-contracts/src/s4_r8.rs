use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::path::{Component, Path};

use codenoesis_domain::storage::{LocalSnapshotHead, SNAPSHOT_SCHEMA_VERSION_V10};
use serde_json::{Map, Value, json};

pub type R8Sha256 = fn(&[u8]) -> String;

pub const R8_PORTABLE_GRAPH_VERSION: &str = "codenoesis.portable-graph/v1";
pub const R8_LOCAL_EXPLORER_VERSION: &str = "codenoesis.local-explorer/v1";
pub const R8_ERROR_VERSION: &str = "codenoesis.error/v15";
pub const R8_SECURITY_PROFILE: &str = "codenoesis.local-explorer-security/v1";
pub const R8_QUERY_VERSION: &str = "codenoesis.local-query-result/v5";
pub const R8_ONTOLOGY_VERSION: &str = "codenoesis.ontology/rust/v7";
pub const R8_PORTABLE_MARKER: &str = ".codenoesis-portable-graph-v1";
pub const R8_EXPLORER_MARKER: &str = ".codenoesis-local-explorer-v1";
pub const R8_CSP_ORACLE_SHA256: &str =
    "b1aefa1dbfd9e988445b626f1bf5b9081d924a78c5d3d44c762457524b2faf0b";
const R8_CONTENT_SECURITY_POLICY: &str = "default-src 'none'; script-src 'sha256-+wXgA8YACUINip3Bd70PW/CNa99al9KY+mUSpK5D8B0='; style-src 'sha256-acsH8EvKpLUUAIFsfEjiVSLLnbHkOls76PFfS1E7Ei4='; img-src 'self' data:; font-src 'none'; connect-src 'none'; object-src 'none'; frame-src 'none'; frame-ancestors 'none'; form-action 'none'; base-uri 'none'; manifest-src 'none'; media-src 'none'; worker-src 'none'";

pub const MAX_R8_PORTABLE_GRAPH_BYTES: u64 = 268_435_456;
pub const MAX_R8_VIEWER_NON_DATA_BYTES: u64 = 1_048_576;
pub const MAX_R8_TEXT_SEARCH_RESULTS: u64 = 100;
pub const R8_TRAVERSAL_DEPTH_DEFAULT: u64 = 1;
pub const MAX_R8_TRAVERSAL_DEPTH: u64 = 2;
pub const MAX_R8_NEIGHBORHOOD_SUBJECTS: u64 = 256;
pub const MAX_R8_NEIGHBORHOOD_RELATIONSHIPS: u64 = 512;
pub const MAX_R8_JSON_NESTING: u64 = 64;
pub const R8_DETERMINISM_PERMUTATIONS: u64 = 50;

const MAX_ENTITIES: usize = 30_000;
const MAX_RELATIONSHIPS: usize = 60_000;
const MAX_CLAIMS: usize = 90_000;
const MAX_EVIDENCE: usize = 90_000;
const MAX_DIAGNOSTICS: usize = 10_000;
const MAX_COVERAGE_GAPS: usize = 10_000;
const MAX_DOCUMENTS: usize = 2_001;
const MAX_DOCUMENT_STATEMENTS: usize = 200_000;
const MAX_DISPLAY_BYTES: usize = 16_384;
const MAX_METADATA_BYTES: usize = 65_536;
const ZERO_SHA256: &str = "0000000000000000000000000000000000000000000000000000000000000000";
const ZERO_SNAPSHOT_ID: &str = "urn:codenoesis:snapshot:blake3:0000000000000000000000000000000000000000000000000000000000000000";

const FAMILY_ORDER: [&str; 8] = [
    "entities",
    "relationships",
    "claims",
    "evidence",
    "diagnostics",
    "coverage_gaps",
    "documents",
    "document_statements",
];

#[derive(Clone, Debug)]
pub struct PortableGraphV1 {
    value: Value,
    canonical: Vec<u8>,
    sha256: R8Sha256,
}

impl PortableGraphV1 {
    /// Projects one validated V10 stored head and its deterministic documentation manifest.
    ///
    /// # Errors
    ///
    /// Returns a closed snapshot, identity, reference, evidence, payload, or limit failure.
    pub fn from_validated_v10(
        semantic: &Value,
        head: &LocalSnapshotHead,
        documentation_manifest: &Value,
        sha256: R8Sha256,
    ) -> Result<Self, R8ContractError> {
        if head.snapshot_schema_version != SNAPSHOT_SCHEMA_VERSION_V10 {
            return Err(R8ContractError::UnsupportedSnapshotSchema(
                head.snapshot_schema_version.clone(),
            ));
        }
        let repository = semantic
            .get("repository")
            .cloned()
            .ok_or_else(|| invalid_snapshot(head, "incomplete"))?;
        if repository.get("identity").and_then(Value::as_str)
            != Some(head.repository_identity.as_str())
            || repository.get("commit_oid").and_then(Value::as_str)
                != Some(head.commit_oid.as_str())
            || semantic.get("ontology_version").and_then(Value::as_str) != Some(R8_ONTOLOGY_VERSION)
        {
            return Err(invalid_snapshot(head, "unbound"));
        }
        validate_documentation_binding(documentation_manifest, head)?;
        let graph = semantic
            .get("knowledge_graph")
            .and_then(Value::as_object)
            .ok_or_else(|| invalid_snapshot(head, "incomplete"))?;

        let entities = sorted_source_family(graph, "entities", "id")?;
        let relationships = sorted_source_family(graph, "relationships", "id")?;
        let claims = sorted_source_family(graph, "claims", "id")?;
        let evidence = sorted_source_family(graph, "evidence", "id")?;
        let diagnostics = sorted_source_family(graph, "diagnostics", "id")?;
        let coverage_gaps = sorted_source_family(graph, "coverage", "id")?;
        let (documents, document_statements) = portable_documents(documentation_manifest)?;

        let value = json!({
            "schema_version": R8_PORTABLE_GRAPH_VERSION,
            "repository": repository,
            "source_snapshot": {
                "schema_version": SNAPSHOT_SCHEMA_VERSION_V10,
                "snapshot_id": head.snapshot_id.as_str(),
                "semantic_hash": {
                    "algorithm": "blake3-256",
                    "value": head.semantic_hash.value
                }
            },
            "ontology_version": R8_ONTOLOGY_VERSION,
            "query_contract_version": R8_QUERY_VERSION,
            "projection": projection_value(),
            "entities": entities,
            "relationships": relationships,
            "claims": claims,
            "evidence": evidence,
            "diagnostics": diagnostics,
            "coverage_gaps": coverage_gaps,
            "documents": documents,
            "document_statements": document_statements
        });
        Self::from_generated_value(value, sha256)
    }

    /// Validates and reimports one canonical LF-terminated `PortableGraphV1` artifact.
    ///
    /// # Errors
    ///
    /// Returns the first governance-ordered decode, schema, identity, reference, or limit failure.
    pub fn from_canonical_file(bytes: &[u8], sha256: R8Sha256) -> Result<Self, R8ContractError> {
        let observed = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if observed > MAX_R8_PORTABLE_GRAPH_BYTES {
            return Err(R8ContractError::LimitExceeded {
                limit: "portable_graph_bytes",
                maximum: MAX_R8_PORTABLE_GRAPH_BYTES,
                observed,
            });
        }
        preflight_json(bytes, sha256)?;
        let value = serde_json::from_slice::<Value>(bytes).map_err(|_| {
            R8ContractError::InvalidProjection {
                projection_sha256: sha256(bytes),
                reason: "invalid_json",
            }
        })?;
        validate_portable_value(&value, sha256)?;
        let canonical = serde_json::to_vec(&value).map_err(|_| R8ContractError::Internal)?;
        let mut expected_file = canonical.clone();
        expected_file.push(b'\n');
        if expected_file != bytes {
            return Err(R8ContractError::Noncanonical {
                expected_sha256: sha256(&expected_file),
                observed_sha256: sha256(bytes),
            });
        }
        Ok(Self {
            value,
            canonical,
            sha256,
        })
    }

    fn from_generated_value(value: Value, sha256: R8Sha256) -> Result<Self, R8ContractError> {
        validate_portable_value(&value, sha256)?;
        let canonical = serde_json::to_vec(&value).map_err(|_| R8ContractError::Internal)?;
        let file_length = canonical
            .len()
            .checked_add(1)
            .and_then(|length| u64::try_from(length).ok())
            .ok_or(R8ContractError::LimitExceeded {
                limit: "portable_graph_bytes",
                maximum: MAX_R8_PORTABLE_GRAPH_BYTES,
                observed: u64::MAX,
            })?;
        if file_length > MAX_R8_PORTABLE_GRAPH_BYTES {
            return Err(R8ContractError::LimitExceeded {
                limit: "portable_graph_bytes",
                maximum: MAX_R8_PORTABLE_GRAPH_BYTES,
                observed: file_length,
            });
        }
        Ok(Self {
            value,
            canonical,
            sha256,
        })
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    #[must_use]
    pub fn canonical_value(&self) -> &[u8] {
        &self.canonical
    }

    #[must_use]
    pub fn canonical_file(&self) -> Vec<u8> {
        let mut bytes = self.canonical.clone();
        bytes.push(b'\n');
        bytes
    }

    #[must_use]
    pub fn canonical_sha256(&self) -> String {
        (self.sha256)(&self.canonical)
    }

    /// Returns exact count, ordered identity list, and SHA-256 for every portable family.
    ///
    /// # Errors
    ///
    /// Returns a contract failure if the internally validated value is no longer complete.
    pub fn family_digests(&self) -> Result<Value, R8ContractError> {
        let mut families = Map::new();
        for family in FAMILY_ORDER {
            let id_field = family_id_field(family);
            let values = self
                .value
                .get(family)
                .and_then(Value::as_array)
                .ok_or(R8ContractError::Internal)?;
            let ids = values
                .iter()
                .map(|value| required_string(value, id_field).map(str::to_owned))
                .collect::<Result<Vec<_>, _>>()?;
            let bytes = serde_json::to_vec(values).map_err(|_| R8ContractError::Internal)?;
            families.insert(
                family.to_owned(),
                json!({
                    "count": values.len(),
                    "canonical_sha256": (self.sha256)(&bytes),
                    "ids": ids
                }),
            );
        }
        Ok(Value::Object(families))
    }
}

#[derive(Clone, Debug)]
pub struct LocalExplorerManifestV1 {
    value: Value,
    canonical: Vec<u8>,
}

impl LocalExplorerManifestV1 {
    /// Binds one validated portable graph and one reviewed first-party static entrypoint.
    ///
    /// # Errors
    ///
    /// Returns an asset or fixed-limit failure.
    pub fn new(
        portable_graph: &PortableGraphV1,
        portable_file: &[u8],
        entrypoint: &[u8],
        expected_entrypoint_sha256: &str,
    ) -> Result<Self, R8ContractError> {
        if portable_file != portable_graph.canonical_file() {
            return Err(R8ContractError::AssetIntegrityMismatch {
                path: "portable-graph.json",
                expected_sha256: (portable_graph.sha256)(&portable_graph.canonical_file()),
                observed_sha256: (portable_graph.sha256)(portable_file),
            });
        }
        validate_r8_viewer_asset(
            entrypoint,
            expected_entrypoint_sha256,
            portable_graph.sha256,
        )?;
        validate_r8_view_limits(
            MAX_R8_TEXT_SEARCH_RESULTS,
            MAX_R8_TRAVERSAL_DEPTH,
            MAX_R8_NEIGHBORHOOD_SUBJECTS,
            MAX_R8_NEIGHBORHOOD_RELATIONSHIPS,
        )?;
        let viewer_bytes = u64::try_from(entrypoint.len()).unwrap_or(u64::MAX);
        let portable_bytes = u64::try_from(portable_file.len()).unwrap_or(u64::MAX);
        let value = json!({
            "schema_version": R8_LOCAL_EXPLORER_VERSION,
            "portable_graph": {
                "path": "portable-graph.json",
                "byte_length": portable_bytes,
                "sha256": (portable_graph.sha256)(portable_file),
                "canonical_sha256": portable_graph.canonical_sha256()
            },
            "entrypoint": {
                "path": "index.html",
                "byte_length": viewer_bytes,
                "sha256": expected_entrypoint_sha256
            },
            "security": {
                "profile": R8_SECURITY_PROFILE,
                "csp_sha256": R8_CSP_ORACLE_SHA256,
                "untrusted_rendering": "textContent_only",
                "explicit_file_selection_required": true,
                "network_allowed": false,
                "remote_resources_allowed": false,
                "browser_auto_launch": false,
                "child_process_allowed": false,
                "persistent_browser_state_allowed": false
            },
            "capabilities": {
                "exact_id_search": true,
                "text_search": "nfc_case_sensitive_codepoint_substring",
                "filters": [
                    "subject_kind",
                    "relationship_kind",
                    "claim_state",
                    "diagnostic_code",
                    "coverage_capability"
                ],
                "bounded_neighborhood": "deterministic_breadth_first",
                "evidence_metadata": true,
                "diagnostics": true,
                "coverage_gaps": true,
                "source_repository_required": false,
                "projection_mutation_allowed": false
            },
            "limits": {
                "text_search_results": MAX_R8_TEXT_SEARCH_RESULTS,
                "traversal_depth_default": R8_TRAVERSAL_DEPTH_DEFAULT,
                "traversal_depth_maximum": MAX_R8_TRAVERSAL_DEPTH,
                "neighborhood_subjects": MAX_R8_NEIGHBORHOOD_SUBJECTS,
                "neighborhood_relationships": MAX_R8_NEIGHBORHOOD_RELATIONSHIPS
            },
            "generation": {
                "output_publication": "atomic_marker_owned_directory",
                "destination_ownership": "empty_or_exact_r8_marker",
                "server_required": false,
                "package_manager_required": false,
                "model_provider_required": false
            }
        });
        let canonical = render_local_explorer_manifest(
            portable_bytes,
            &(portable_graph.sha256)(portable_file),
            &portable_graph.canonical_sha256(),
            viewer_bytes,
            expected_entrypoint_sha256,
        );
        Ok(Self { value, canonical })
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Serializes the strict manifest followed by one LF.
    ///
    /// # Errors
    ///
    /// Returns a serialization failure only for an invalid internal value.
    pub fn canonical_file(&self) -> Result<Vec<u8>, serde_json::Error> {
        Ok(self.canonical.clone())
    }
}

/// Validates the fixed local-explorer display bounds before proportional work.
///
/// # Errors
///
/// Returns the first fixed limit exceeded in deterministic contract order.
pub fn validate_r8_view_limits(
    text_results: u64,
    traversal_depth: u64,
    neighborhood_subjects: u64,
    neighborhood_relationships: u64,
) -> Result<(), R8ContractError> {
    for (limit, maximum, observed) in [
        (
            "text_search_results",
            MAX_R8_TEXT_SEARCH_RESULTS,
            text_results,
        ),
        (
            "traversal_depth_maximum",
            MAX_R8_TRAVERSAL_DEPTH,
            traversal_depth,
        ),
        (
            "neighborhood_subjects",
            MAX_R8_NEIGHBORHOOD_SUBJECTS,
            neighborhood_subjects,
        ),
        (
            "neighborhood_relationships",
            MAX_R8_NEIGHBORHOOD_RELATIONSHIPS,
            neighborhood_relationships,
        ),
    ] {
        if observed > maximum {
            return Err(R8ContractError::LimitExceeded {
                limit,
                maximum,
                observed,
            });
        }
    }
    Ok(())
}

/// Validates one first-party local-explorer entrypoint and its reviewed digest.
///
/// # Errors
///
/// Returns a fixed limit, integrity, encoding, CSP, or active-content failure.
pub fn validate_r8_viewer_asset(
    entrypoint: &[u8],
    expected_sha256: &str,
    sha256: R8Sha256,
) -> Result<(), R8ContractError> {
    let observed_bytes = u64::try_from(entrypoint.len()).unwrap_or(u64::MAX);
    if observed_bytes == 0 || observed_bytes > MAX_R8_VIEWER_NON_DATA_BYTES {
        return Err(R8ContractError::LimitExceeded {
            limit: "viewer_non_data_bytes",
            maximum: MAX_R8_VIEWER_NON_DATA_BYTES,
            observed: observed_bytes.max(1),
        });
    }
    let observed_sha256 = sha256(entrypoint);
    if observed_sha256 != expected_sha256 {
        return Err(R8ContractError::AssetIntegrityMismatch {
            path: "index.html",
            expected_sha256: valid_sha256_or_zero(expected_sha256).to_owned(),
            observed_sha256,
        });
    }
    let html = std::str::from_utf8(entrypoint)
        .map_err(|_| unsafe_payload_bytes(entrypoint, "active_content", sha256))?;
    if !html.contains(R8_CONTENT_SECURITY_POLICY)
        || !html.contains(".textContent")
        || html.contains(".innerHTML")
        || html.contains("insertAdjacentHTML")
        || html.contains("document.write")
        || html.contains("<script src")
        || html.contains("http://")
        || html.contains("https://")
        || html.contains("new Function(")
        || html.contains("eval(")
        || html.contains("import(")
        || html.contains("fetch(")
        || html.contains("XMLHttpRequest")
        || html.contains("WebSocket")
        || html.contains("localStorage")
        || html.contains("sessionStorage")
    {
        return Err(unsafe_payload_bytes(entrypoint, "active_content", sha256));
    }
    Ok(())
}

fn render_local_explorer_manifest(
    portable_bytes: u64,
    portable_sha256: &str,
    portable_canonical_sha256: &str,
    viewer_bytes: u64,
    viewer_sha256: &str,
) -> Vec<u8> {
    let mut output = String::from(
        "{\n  \"schema_version\": \"codenoesis.local-explorer/v1\",\n  \"portable_graph\": {\n    \"path\": \"portable-graph.json\",\n    \"byte_length\": ",
    );
    output.push_str(&portable_bytes.to_string());
    output.push_str(",\n    \"sha256\": \"");
    output.push_str(portable_sha256);
    output.push_str("\",\n    \"canonical_sha256\": \"");
    output.push_str(portable_canonical_sha256);
    output.push_str(
        "\"\n  },\n  \"entrypoint\": {\n    \"path\": \"index.html\",\n    \"byte_length\": ",
    );
    output.push_str(&viewer_bytes.to_string());
    output.push_str(",\n    \"sha256\": \"");
    output.push_str(viewer_sha256);
    output.push_str(
        "\"\n  },\n  \"security\": {\n    \"profile\": \"codenoesis.local-explorer-security/v1\",\n    \"csp_sha256\": \"b1aefa1dbfd9e988445b626f1bf5b9081d924a78c5d3d44c762457524b2faf0b\",\n    \"untrusted_rendering\": \"textContent_only\",\n    \"explicit_file_selection_required\": true,\n    \"network_allowed\": false,\n    \"remote_resources_allowed\": false,\n    \"browser_auto_launch\": false,\n    \"child_process_allowed\": false,\n    \"persistent_browser_state_allowed\": false\n  },\n  \"capabilities\": {\n    \"exact_id_search\": true,\n    \"text_search\": \"nfc_case_sensitive_codepoint_substring\",\n    \"filters\": [\n      \"subject_kind\",\n      \"relationship_kind\",\n      \"claim_state\",\n      \"diagnostic_code\",\n      \"coverage_capability\"\n    ],\n    \"bounded_neighborhood\": \"deterministic_breadth_first\",\n    \"evidence_metadata\": true,\n    \"diagnostics\": true,\n    \"coverage_gaps\": true,\n    \"source_repository_required\": false,\n    \"projection_mutation_allowed\": false\n  },\n  \"limits\": {\n    \"text_search_results\": 100,\n    \"traversal_depth_default\": 1,\n    \"traversal_depth_maximum\": 2,\n    \"neighborhood_subjects\": 256,\n    \"neighborhood_relationships\": 512\n  },\n  \"generation\": {\n    \"output_publication\": \"atomic_marker_owned_directory\",\n    \"destination_ownership\": \"empty_or_exact_r8_marker\",\n    \"server_required\": false,\n    \"package_manager_required\": false,\n    \"model_provider_required\": false\n  }\n}\n",
    );
    output.into_bytes()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum R8ContractError {
    InvalidSnapshot {
        snapshot_id: String,
        reason: &'static str,
    },
    UnsupportedSnapshotSchema(String),
    UnsupportedPortableGraphSchema(String),
    Noncanonical {
        expected_sha256: String,
        observed_sha256: String,
    },
    IdentityConflict {
        family: &'static str,
        identity_sha256: String,
    },
    ReferenceMismatch {
        family: &'static str,
        subject_id_sha256: String,
        reference_id_sha256: String,
    },
    UnresolvedEvidence {
        subject_id_sha256: String,
        evidence_id: String,
    },
    LimitExceeded {
        limit: &'static str,
        maximum: u64,
        observed: u64,
    },
    InvalidProjection {
        projection_sha256: String,
        reason: &'static str,
    },
    UnsafePayload {
        field_sha256: String,
        reason: &'static str,
    },
    AssetIntegrityMismatch {
        path: &'static str,
        expected_sha256: String,
        observed_sha256: String,
    },
    Internal,
}

impl Display for R8ContractError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidSnapshot { .. } => "invalid R8 source snapshot",
            Self::UnsupportedSnapshotSchema(_) => "unsupported R8 source snapshot schema",
            Self::UnsupportedPortableGraphSchema(_) => "unsupported portable graph schema",
            Self::Noncanonical { .. } => "noncanonical portable graph",
            Self::IdentityConflict { .. } => "portable graph identity conflict",
            Self::ReferenceMismatch { .. } => "portable graph reference mismatch",
            Self::UnresolvedEvidence { .. } => "portable graph evidence is unresolved",
            Self::LimitExceeded { .. } => "R8 fixed limit exceeded",
            Self::InvalidProjection { .. } => "invalid portable graph projection",
            Self::UnsafePayload { .. } => "unsafe explorer payload",
            Self::AssetIntegrityMismatch { .. } => "explorer asset integrity mismatch",
            Self::Internal => "unexpected R8 contract failure",
        })
    }
}

impl Error for R8ContractError {}

#[derive(Clone, Debug)]
pub struct CodeNoesisErrorV15 {
    value: Value,
}

impl CodeNoesisErrorV15 {
    #[must_use]
    pub fn invalid_export_profile() -> Self {
        Self::new(
            "input.invalid_export_profile",
            "input",
            "invalid export profile",
            json!({"profile": "portable-graph-v1"}),
        )
    }

    #[must_use]
    pub fn invalid_explorer_profile() -> Self {
        Self::new(
            "input.invalid_explorer_profile",
            "input",
            "invalid explorer profile",
            json!({"profile": "local-explorer-v1"}),
        )
    }

    #[must_use]
    pub fn unsafe_output_path(path_sha256: &str, reason: &'static str) -> Self {
        Self::new(
            "input.unsafe_output_path",
            "input",
            "unsafe output path",
            json!({
                "path_sha256": valid_sha256_or_zero(path_sha256),
                "reason": valid_path_reason(reason)
            }),
        )
    }

    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn from_contract(error: &R8ContractError) -> Self {
        match error {
            R8ContractError::InvalidSnapshot {
                snapshot_id,
                reason,
            } => Self::new(
                "export.invalid_snapshot",
                "export",
                "invalid source snapshot",
                json!({
                    "snapshot_id": valid_snapshot_id_or_zero(snapshot_id),
                    "reason": valid_snapshot_reason(reason)
                }),
            ),
            R8ContractError::UnsupportedSnapshotSchema(observed) => Self::new(
                "export.unsupported_snapshot_schema",
                "export",
                "unsupported snapshot schema",
                json!({"subject": "repository_snapshot", "observed": bounded(observed, 128)}),
            ),
            R8ContractError::UnsupportedPortableGraphSchema(observed) => Self::new(
                "export.unsupported_portable_graph_schema",
                "export",
                "unsupported portable graph schema",
                json!({"subject": "portable_graph", "observed": bounded(observed, 128)}),
            ),
            R8ContractError::Noncanonical {
                expected_sha256,
                observed_sha256,
            } => Self::new(
                "export.noncanonical_portable_graph",
                "export",
                "noncanonical portable graph",
                json!({
                    "expected_sha256": valid_sha256_or_zero(expected_sha256),
                    "observed_sha256": valid_sha256_or_zero(observed_sha256)
                }),
            ),
            R8ContractError::IdentityConflict {
                family,
                identity_sha256,
            } => Self::new(
                "export.identity_conflict",
                "export",
                "portable graph identity conflict",
                json!({
                    "family": family,
                    "identity_sha256": valid_sha256_or_zero(identity_sha256)
                }),
            ),
            R8ContractError::ReferenceMismatch {
                family,
                subject_id_sha256,
                reference_id_sha256,
            } => Self::new(
                "export.reference_mismatch",
                "export",
                "portable graph reference mismatch",
                json!({
                    "family": family,
                    "subject_id_sha256": valid_sha256_or_zero(subject_id_sha256),
                    "reference_id_sha256": valid_sha256_or_zero(reference_id_sha256)
                }),
            ),
            R8ContractError::UnresolvedEvidence {
                subject_id_sha256,
                evidence_id,
            } => Self::new(
                "export.unresolved_evidence",
                "export",
                "portable graph evidence is unresolved",
                json!({
                    "subject_id_sha256": valid_sha256_or_zero(subject_id_sha256),
                    "evidence_id": valid_evidence_id_or_zero(evidence_id)
                }),
            ),
            R8ContractError::LimitExceeded {
                limit,
                maximum,
                observed,
            } => {
                let stage = if limit.starts_with("viewer_")
                    || matches!(
                        *limit,
                        "text_search_results"
                            | "traversal_depth_maximum"
                            | "neighborhood_subjects"
                            | "neighborhood_relationships"
                    ) {
                    "explorer"
                } else {
                    "export"
                };
                let code = if stage == "explorer" {
                    "explorer.limit_exceeded"
                } else {
                    "export.limit_exceeded"
                };
                Self::new(
                    code,
                    stage,
                    "R8 fixed limit exceeded",
                    json!({"limit": limit, "maximum": maximum.max(&1), "observed": observed.max(&1)}),
                )
            }
            R8ContractError::InvalidProjection {
                projection_sha256,
                reason,
            } => Self::new(
                "explorer.invalid_projection",
                "explorer",
                "invalid explorer projection",
                json!({
                    "projection_sha256": valid_sha256_or_zero(projection_sha256),
                    "reason": valid_projection_reason(reason)
                }),
            ),
            R8ContractError::UnsafePayload {
                field_sha256,
                reason,
            } => Self::new(
                "explorer.unsafe_payload",
                "explorer",
                "unsafe explorer payload",
                json!({
                    "field_sha256": valid_sha256_or_zero(field_sha256),
                    "reason": valid_payload_reason(reason)
                }),
            ),
            R8ContractError::AssetIntegrityMismatch {
                path,
                expected_sha256,
                observed_sha256,
            } => Self::new(
                "explorer.asset_integrity_mismatch",
                "explorer",
                "explorer asset integrity mismatch",
                json!({
                    "path": path,
                    "expected_sha256": valid_sha256_or_zero(expected_sha256),
                    "observed_sha256": valid_sha256_or_zero(observed_sha256)
                }),
            ),
            R8ContractError::Internal => Self::internal(),
        }
    }

    #[must_use]
    pub fn internal() -> Self {
        Self::new(
            "internal.unexpected",
            "internal",
            "unexpected internal error",
            json!({}),
        )
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Serializes strict `ErrorV15` followed by one LF.
    ///
    /// # Errors
    ///
    /// Returns a serialization failure only for an invalid internal value.
    pub fn canonical_stderr(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = serde_json::to_vec(&self.value)?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    fn new(code: &str, stage: &str, message: &str, context: Value) -> Self {
        let value = json!({
            "schema_version": R8_ERROR_VERSION,
            "code": code,
            "stage": stage,
            "message": message,
            "retryable": false,
            "context": context
        });
        drop(context);
        Self { value }
    }
}

fn validate_documentation_binding(
    manifest: &Value,
    head: &LocalSnapshotHead,
) -> Result<(), R8ContractError> {
    if manifest.get("schema_version").and_then(Value::as_str)
        != Some("codenoesis.documentation-manifest/v1")
        || manifest.get("repository_identity").and_then(Value::as_str)
            != Some(head.repository_identity.as_str())
        || manifest.get("snapshot_id").and_then(Value::as_str) != Some(head.snapshot_id.as_str())
        || manifest
            .pointer("/snapshot_semantic_hash/algorithm")
            .and_then(Value::as_str)
            != Some("blake3-256")
        || manifest
            .pointer("/snapshot_semantic_hash/value")
            .and_then(Value::as_str)
            != Some(head.semantic_hash.value.as_str())
    {
        return Err(invalid_snapshot(head, "unbound"));
    }
    Ok(())
}

fn invalid_snapshot(head: &LocalSnapshotHead, reason: &'static str) -> R8ContractError {
    R8ContractError::InvalidSnapshot {
        snapshot_id: head.snapshot_id.to_string(),
        reason,
    }
}

fn sorted_source_family(
    graph: &Map<String, Value>,
    field: &str,
    id_field: &str,
) -> Result<Vec<Value>, R8ContractError> {
    let mut values = graph
        .get(field)
        .and_then(Value::as_array)
        .cloned()
        .ok_or(R8ContractError::Internal)?;
    values.sort_by(|left, right| {
        left.get(id_field)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .as_bytes()
            .cmp(
                right
                    .get(id_field)
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .as_bytes(),
            )
    });
    Ok(values)
}

fn portable_documents(manifest: &Value) -> Result<(Vec<Value>, Vec<Value>), R8ContractError> {
    let source = manifest
        .get("documents")
        .and_then(Value::as_array)
        .ok_or(R8ContractError::Internal)?;
    let mut documents = Vec::with_capacity(source.len());
    let mut statements = Vec::new();
    for document in source {
        let mut record = document
            .as_object()
            .cloned()
            .ok_or(R8ContractError::Internal)?;
        let document_id = record
            .get("document_id")
            .and_then(Value::as_str)
            .ok_or(R8ContractError::Internal)?
            .to_owned();
        let source_statements = record
            .remove("statements")
            .and_then(|value| value.as_array().cloned())
            .ok_or(R8ContractError::Internal)?;
        documents.push(Value::Object(record));
        for statement in source_statements {
            let mut statement = statement
                .as_object()
                .cloned()
                .ok_or(R8ContractError::Internal)?;
            statement.insert("document_id".to_owned(), Value::String(document_id.clone()));
            statements.push(Value::Object(statement));
        }
    }
    documents.sort_by_id("document_id")?;
    statements.sort_by_id("statement_id")?;
    Ok((documents, statements))
}

trait SortValuesById {
    fn sort_by_id(&mut self, field: &str) -> Result<(), R8ContractError>;
}

impl SortValuesById for Vec<Value> {
    fn sort_by_id(&mut self, field: &str) -> Result<(), R8ContractError> {
        for value in self.iter() {
            required_string(value, field)?;
        }
        self.sort_by(|left, right| {
            left.get(field)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .as_bytes()
                .cmp(
                    right
                        .get(field)
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .as_bytes(),
                )
        });
        Ok(())
    }
}

fn projection_value() -> Value {
    json!({
        "canonicalization": "RFC8785",
        "family_order": FAMILY_ORDER,
        "identity_preservation": "exact",
        "reference_preservation": "exact",
        "evidence_preservation": "lossless_redacted_metadata",
        "claim_state_policy": "preserve_without_upgrade",
        "unknown_fields_allowed": false,
        "source_contents_included": false,
        "source_snippets_included": false
    })
}

#[allow(clippy::too_many_lines)]
fn validate_portable_value(value: &Value, sha256: R8Sha256) -> Result<(), R8ContractError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid_projection_value(value, "invalid_shape"))?;
    let schema = object
        .get("schema_version")
        .and_then(Value::as_str)
        .unwrap_or("missing");
    if schema != R8_PORTABLE_GRAPH_VERSION {
        return Err(R8ContractError::UnsupportedPortableGraphSchema(bounded(
            schema, 128,
        )));
    }
    require_exact_keys(
        object,
        &[
            "claims",
            "coverage_gaps",
            "diagnostics",
            "document_statements",
            "documents",
            "entities",
            "evidence",
            "ontology_version",
            "projection",
            "query_contract_version",
            "relationships",
            "repository",
            "schema_version",
            "source_snapshot",
        ],
        value,
    )?;
    validate_repository(object.get("repository"), value)?;
    validate_source_snapshot(object.get("source_snapshot"), value)?;
    if object.get("ontology_version").and_then(Value::as_str) != Some(R8_ONTOLOGY_VERSION)
        || object.get("query_contract_version").and_then(Value::as_str) != Some(R8_QUERY_VERSION)
        || object.get("projection") != Some(&projection_value())
    {
        return Err(invalid_projection_value(value, "invalid_shape"));
    }

    let entities = family(value, "entities", MAX_ENTITIES)?;
    let relationships = family(value, "relationships", MAX_RELATIONSHIPS)?;
    let claims = family(value, "claims", MAX_CLAIMS)?;
    let evidence = family(value, "evidence", MAX_EVIDENCE)?;
    let diagnostics = family(value, "diagnostics", MAX_DIAGNOSTICS)?;
    let coverage = family(value, "coverage_gaps", MAX_COVERAGE_GAPS)?;
    let documents = family(value, "documents", MAX_DOCUMENTS)?;
    let statements = family(value, "document_statements", MAX_DOCUMENT_STATEMENTS)?;

    validate_family_shapes(
        entities,
        relationships,
        claims,
        evidence,
        diagnostics,
        coverage,
        documents,
        statements,
        value,
    )?;

    let entity_ids = validate_family_ids(entities, "entities", "id", sha256)?;
    let relationship_ids = validate_family_ids(relationships, "relationships", "id", sha256)?;
    let claim_ids = validate_family_ids(claims, "claims", "id", sha256)?;
    let evidence_ids = validate_family_ids(evidence, "evidence", "id", sha256)?;
    let diagnostic_ids = validate_family_ids(diagnostics, "diagnostics", "id", sha256)?;
    let coverage_ids = validate_family_ids(coverage, "coverage_gaps", "id", sha256)?;
    let document_ids = validate_family_ids(documents, "documents", "document_id", sha256)?;
    let statement_ids =
        validate_family_ids(statements, "document_statements", "statement_id", sha256)?;

    let mut subject_ids = entity_ids.clone();
    subject_ids.extend(relationship_ids.iter().cloned());
    subject_ids.extend(claim_ids.iter().cloned());
    subject_ids.extend(evidence_ids.iter().cloned());
    subject_ids.extend(diagnostic_ids.iter().cloned());
    subject_ids.extend(coverage_ids.iter().cloned());
    subject_ids.extend(document_ids.iter().cloned());
    subject_ids.extend(statement_ids);
    let repository_identity = value
        .pointer("/repository/identity")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_projection_value(value, "invalid_shape"))?;

    for relationship in relationships {
        let id = required_string(relationship, "id")?;
        for field in ["source", "target"] {
            let endpoint = required_string(relationship, field)?;
            if !entity_ids.contains(endpoint) {
                return Err(R8ContractError::ReferenceMismatch {
                    family: "relationship_endpoint",
                    subject_id_sha256: sha256(id.as_bytes()),
                    reference_id_sha256: sha256(endpoint.as_bytes()),
                });
            }
        }
    }
    for claim in claims {
        let id = required_string(claim, "id")?;
        let subject = required_string(claim, "subject_id")?;
        let subject_exists = match required_string(claim, "subject_kind")? {
            "entity" => entity_ids.contains(subject),
            "relationship" => relationship_ids.contains(subject),
            _ => false,
        };
        if !subject_exists {
            return Err(R8ContractError::ReferenceMismatch {
                family: "claim_subject",
                subject_id_sha256: sha256(id.as_bytes()),
                reference_id_sha256: sha256(subject.as_bytes()),
            });
        }
    }
    for document in documents {
        let id = required_string(document, "document_id")?;
        let subject = required_string(document, "subject_id")?;
        if subject != repository_identity && !subject_ids.contains(subject) {
            return Err(R8ContractError::ReferenceMismatch {
                family: "document_subject",
                subject_id_sha256: sha256(id.as_bytes()),
                reference_id_sha256: sha256(subject.as_bytes()),
            });
        }
    }
    for statement in statements {
        let id = required_string(statement, "statement_id")?;
        let document_id = required_string(statement, "document_id")?;
        if !document_ids.contains(document_id) {
            return Err(R8ContractError::ReferenceMismatch {
                family: "document_subject",
                subject_id_sha256: sha256(id.as_bytes()),
                reference_id_sha256: sha256(document_id.as_bytes()),
            });
        }
        for subject in required_string_array(statement, "subject_ids")? {
            if subject != repository_identity && !subject_ids.contains(subject) {
                return Err(R8ContractError::ReferenceMismatch {
                    family: "statement_subject",
                    subject_id_sha256: sha256(id.as_bytes()),
                    reference_id_sha256: sha256(subject.as_bytes()),
                });
            }
        }
        for gap in required_string_array(statement, "coverage_gap_ids")? {
            if !coverage_ids.contains(gap) {
                return Err(R8ContractError::ReferenceMismatch {
                    family: "coverage_gap",
                    subject_id_sha256: sha256(id.as_bytes()),
                    reference_id_sha256: sha256(gap.as_bytes()),
                });
            }
        }
    }
    for (family_name, records, id_field) in [
        ("entities", entities, "id"),
        ("relationships", relationships, "id"),
        ("claims", claims, "id"),
        ("diagnostics", diagnostics, "id"),
        ("coverage_gaps", coverage, "id"),
        ("document_statements", statements, "statement_id"),
    ] {
        validate_evidence_references(family_name, records, id_field, &evidence_ids, sha256)?;
    }
    validate_portable_paths(value, sha256)?;
    validate_untrusted_values(value, sha256)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_family_shapes(
    entities: &[Value],
    relationships: &[Value],
    claims: &[Value],
    evidence: &[Value],
    diagnostics: &[Value],
    coverage: &[Value],
    documents: &[Value],
    statements: &[Value],
    complete: &Value,
) -> Result<(), R8ContractError> {
    for entity in entities {
        validate_entity_shape(entity, complete)?;
    }
    for relationship in relationships {
        validate_relationship_shape(relationship, complete)?;
    }
    for claim in claims {
        validate_claim_shape(claim, complete)?;
    }
    for evidence in evidence {
        validate_evidence_shape(evidence, complete)?;
    }
    for diagnostic in diagnostics {
        validate_diagnostic_shape(diagnostic, complete)?;
    }
    for gap in coverage {
        validate_coverage_shape(gap, complete)?;
    }
    for document in documents {
        validate_document_shape(document, complete)?;
    }
    for statement in statements {
        validate_statement_shape(statement, complete)?;
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn validate_entity_shape(entity: &Value, complete: &Value) -> Result<(), R8ContractError> {
    let object = entity
        .as_object()
        .ok_or_else(|| invalid_projection_value(complete, "invalid_shape"))?;
    let kind = object
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_projection_value(complete, "invalid_shape"))?;
    match kind {
        "compiler.symbol" => require_exact_keys(
            object,
            &[
                "binding_state",
                "compiler_evidence_ids",
                "display_name",
                "id",
                "identity_preimage",
                "kind",
                "scope",
                "source_entity_id",
                "source_evidence_ids",
                "symbol",
            ],
            complete,
        )?,
        "framework.component_declaration"
        | "framework.configuration_declaration"
        | "framework.endpoint_declaration"
        | "framework.handler_declaration"
        | "framework.route_declaration"
        | "framework.service_declaration" => require_exact_keys(
            object,
            &[
                "compilation_presence",
                "configuration_key",
                "crate_id",
                "declared_key_or_target",
                "epistemic_state",
                "evidence_ids",
                "id",
                "kind",
                "lexical_owner_id",
                "local_target_id",
                "method",
                "path",
                "role",
                "source_form_identity",
                "source_profile",
                "target_binding",
                "target_spelling",
            ],
            complete,
        )?,
        "rust.field"
        | "rust.enum_variant"
        | "rust.constant"
        | "rust.static"
        | "rust.associated_type" => {
            require_exact_keys(
                object,
                &[
                    "compilation_presence",
                    "crate_id",
                    "id",
                    "kind",
                    "module_path",
                    "name",
                    "owner_id",
                    "properties",
                    "trait_context_id",
                    "visibility",
                ],
                complete,
            )?;
            validate_member_properties(object.get("properties"), complete)?;
        }
        "rust.method" if object.contains_key("owner_id") => {
            require_exact_keys(
                object,
                &[
                    "crate_id",
                    "id",
                    "kind",
                    "module_path",
                    "name",
                    "owner_id",
                    "properties",
                    "visibility",
                ],
                complete,
            )?;
            validate_method_properties(object.get("properties"), complete)?;
        }
        "cargo.manifest"
        | "cargo.workspace_package_defaults"
        | "cargo.package"
        | "cargo.target"
        | "cargo.dependency"
        | "cargo.feature"
        | "cargo.patch"
        | "cargo.build_script" => {
            require_exact_keys(
                object,
                &[
                    "crate_id",
                    "id",
                    "kind",
                    "module_path",
                    "name",
                    "properties",
                    "visibility",
                ],
                complete,
            )?;
            validate_cargo_properties(kind, object.get("properties"), complete)?;
        }
        "rust.crate"
        | "source.file"
        | "rust.module"
        | "rust.struct"
        | "rust.enum"
        | "rust.trait"
        | "rust.type_alias"
        | "rust.function"
        | "rust.method"
        | "rust.symbol_reference" => {
            require_exact_keys(
                object,
                &[
                    "crate_id",
                    "id",
                    "kind",
                    "module_path",
                    "name",
                    "properties",
                    "visibility",
                ],
                complete,
            )?;
            if !object.get("properties").is_some_and(Value::is_object) {
                return Err(invalid_projection_value(complete, "invalid_shape"));
            }
        }
        _ => return Err(invalid_projection_value(complete, "invalid_shape")),
    }
    Ok(())
}

fn validate_member_properties(
    properties: Option<&Value>,
    complete: &Value,
) -> Result<(), R8ContractError> {
    let object = properties
        .and_then(Value::as_object)
        .ok_or_else(|| invalid_projection_value(complete, "invalid_shape"))?;
    require_exact_keys(
        object,
        &[
            "attributes",
            "bounds_present",
            "declared_name",
            "declared_type_or_header",
            "default_present",
            "discriminant_present",
            "form",
            "initializer_present",
            "mutable",
            "owner_kind",
            "tuple_index",
        ],
        complete,
    )?;
    validate_attributes(object.get("attributes"), complete)
}

fn validate_method_properties(
    properties: Option<&Value>,
    complete: &Value,
) -> Result<(), R8ContractError> {
    let object = properties
        .and_then(Value::as_object)
        .ok_or_else(|| invalid_projection_value(complete, "invalid_shape"))?;
    require_exact_keys(
        object,
        &[
            "attributes",
            "compilation_presence",
            "declared_signature",
            "implementation_context",
            "receiver_present",
            "trait_context_id",
        ],
        complete,
    )?;
    validate_attributes(object.get("attributes"), complete)
}

fn validate_attributes(
    attributes: Option<&Value>,
    complete: &Value,
) -> Result<(), R8ContractError> {
    let attributes = attributes
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_projection_value(complete, "invalid_shape"))?;
    if attributes.len() > 128 {
        return Err(invalid_projection_value(complete, "invalid_shape"));
    }
    for attribute in attributes {
        let object = attribute
            .as_object()
            .ok_or_else(|| invalid_projection_value(complete, "invalid_shape"))?;
        require_exact_keys(object, &["evidence_id", "kind", "token_text"], complete)?;
    }
    Ok(())
}

fn validate_cargo_properties(
    kind: &str,
    properties: Option<&Value>,
    complete: &Value,
) -> Result<(), R8ContractError> {
    let object = properties
        .and_then(Value::as_object)
        .ok_or_else(|| invalid_projection_value(complete, "invalid_shape"))?;
    let expected: &[&str] = match kind {
        "cargo.manifest" => &[
            "evidence_id",
            "manifest_path",
            "manifest_role",
            "package_table_present",
            "root_shape",
            "workspace_table_present",
        ],
        "cargo.workspace_package_defaults" => {
            &["evidence_id", "manifest_id", "manifest_path", "metadata"]
        }
        "cargo.package" => &[
            "evidence_id",
            "manifest_id",
            "manifest_path",
            "metadata",
            "package_name",
        ],
        "cargo.target" => &[
            "evidence_id",
            "manifest_id",
            "manifest_path",
            "materialized_crate_id",
            "name_source",
            "options",
            "package_id",
            "path_source",
            "required_features",
            "source_analysis_state",
            "source_path",
            "target_kind",
            "target_name",
        ],
        "cargo.dependency" => &[
            "declared_name",
            "default_features",
            "dependency_kind",
            "evidence_id",
            "manifest_id",
            "manifest_path",
            "optional",
            "owner_id",
            "package_name",
            "requested_features",
            "scope",
            "source",
            "target_predicate",
        ],
        "cargo.feature" => &[
            "evidence_id",
            "feature_name",
            "manifest_id",
            "manifest_path",
            "members",
            "package_id",
        ],
        "cargo.patch" => &[
            "applied",
            "declared_name",
            "evidence_id",
            "manifest_id",
            "manifest_path",
            "package_name",
            "source",
            "source_selector",
        ],
        "cargo.build_script" => &[
            "committed_present",
            "evidence_id",
            "executed",
            "manifest_id",
            "manifest_path",
            "package_id",
            "path",
            "selection",
        ],
        _ => return Err(invalid_projection_value(complete, "invalid_shape")),
    };
    require_exact_keys(object, expected, complete)
}

fn validate_relationship_shape(
    relationship: &Value,
    complete: &Value,
) -> Result<(), R8ContractError> {
    let object = relationship
        .as_object()
        .ok_or_else(|| invalid_projection_value(complete, "invalid_shape"))?;
    if object.contains_key("provenance") || object.contains_key("endpoint_binding") {
        require_exact_keys(
            object,
            &[
                "endpoint_binding",
                "evidence_ids",
                "id",
                "kind",
                "provenance",
                "source",
                "target",
            ],
            complete,
        )?;
    } else {
        require_exact_keys(
            object,
            &["evidence_ids", "id", "kind", "source", "target"],
            complete,
        )?;
    }
    Ok(())
}

fn validate_claim_shape(claim: &Value, complete: &Value) -> Result<(), R8ContractError> {
    let object = claim
        .as_object()
        .ok_or_else(|| invalid_projection_value(complete, "invalid_shape"))?;
    require_exact_keys(
        object,
        &["evidence_ids", "id", "state", "subject_id", "subject_kind"],
        complete,
    )?;
    if !object
        .get("subject_kind")
        .and_then(Value::as_str)
        .is_some_and(|kind| matches!(kind, "entity" | "relationship"))
    {
        return Err(invalid_projection_value(complete, "invalid_shape"));
    }
    Ok(())
}

fn validate_evidence_shape(evidence: &Value, complete: &Value) -> Result<(), R8ContractError> {
    let object = evidence
        .as_object()
        .ok_or_else(|| invalid_projection_value(complete, "invalid_shape"))?;
    if object.contains_key("artifact_sha256") {
        require_exact_keys(
            object,
            &[
                "artifact_sha256",
                "document_path",
                "id",
                "range",
                "record_kind",
                "relationship_flags",
                "relationship_target",
                "symbol",
                "symbol_roles",
            ],
            complete,
        )?;
    } else {
        require_exact_keys(
            object,
            &["blob_oid", "end_byte", "id", "path", "start_byte"],
            complete,
        )?;
    }
    Ok(())
}

fn validate_diagnostic_shape(diagnostic: &Value, complete: &Value) -> Result<(), R8ContractError> {
    let object = diagnostic
        .as_object()
        .ok_or_else(|| invalid_projection_value(complete, "invalid_shape"))?;
    if object.contains_key("subject_id") || object.contains_key("compiler_target_id") {
        require_exact_keys(
            object,
            &[
                "code",
                "compiler_target_id",
                "evidence_ids",
                "id",
                "subject_id",
            ],
            complete,
        )?;
    } else {
        require_exact_keys(object, &["code", "evidence_ids", "id", "message"], complete)?;
    }
    Ok(())
}

fn validate_coverage_shape(gap: &Value, complete: &Value) -> Result<(), R8ContractError> {
    let object = gap
        .as_object()
        .ok_or_else(|| invalid_projection_value(complete, "invalid_shape"))?;
    if object.contains_key("subject") {
        require_exact_keys(
            object,
            &["capability", "evidence_ids", "id", "state", "subject"],
            complete,
        )?;
    } else {
        require_exact_keys(
            object,
            &["capability", "evidence_ids", "id", "state"],
            complete,
        )?;
    }
    Ok(())
}

fn validate_repository(
    repository: Option<&Value>,
    complete: &Value,
) -> Result<(), R8ContractError> {
    let repository = repository
        .and_then(Value::as_object)
        .ok_or_else(|| invalid_projection_value(complete, "invalid_shape"))?;
    require_exact_keys(
        repository,
        &[
            "commit_oid",
            "contract_version",
            "identity",
            "identity_schema_version",
            "object_format",
            "tree_oid",
            "vcs",
        ],
        complete,
    )?;
    let valid = repository.get("contract_version").and_then(Value::as_str)
        == Some("codenoesis.repository/v1")
        && repository
            .get("identity_schema_version")
            .and_then(Value::as_str)
            == Some("codenoesis.repository-identity/v1")
        && repository.get("vcs").and_then(Value::as_str) == Some("git")
        && repository.get("object_format").and_then(Value::as_str) == Some("sha1")
        && repository
            .get("identity")
            .and_then(Value::as_str)
            .is_some_and(|identity| !identity.is_empty() && identity.len() <= 512)
        && repository
            .get("commit_oid")
            .and_then(Value::as_str)
            .is_some_and(valid_sha1)
        && repository
            .get("tree_oid")
            .and_then(Value::as_str)
            .is_some_and(valid_sha1);
    if valid {
        Ok(())
    } else {
        Err(invalid_projection_value(complete, "invalid_shape"))
    }
}

fn validate_source_snapshot(
    source: Option<&Value>,
    complete: &Value,
) -> Result<(), R8ContractError> {
    let source = source
        .and_then(Value::as_object)
        .ok_or_else(|| invalid_projection_value(complete, "invalid_shape"))?;
    require_exact_keys(
        source,
        &["schema_version", "semantic_hash", "snapshot_id"],
        complete,
    )?;
    if source.get("schema_version").and_then(Value::as_str) != Some(SNAPSHOT_SCHEMA_VERSION_V10) {
        return Err(R8ContractError::UnsupportedSnapshotSchema(
            source
                .get("schema_version")
                .and_then(Value::as_str)
                .map_or_else(|| "missing".to_owned(), |value| bounded(value, 128)),
        ));
    }
    let valid = source
        .get("snapshot_id")
        .and_then(Value::as_str)
        .is_some_and(valid_snapshot_id)
        && source
            .get("semantic_hash")
            .and_then(Value::as_object)
            .and_then(|hash| hash.get("algorithm"))
            .and_then(Value::as_str)
            == Some("blake3-256")
        && source
            .get("semantic_hash")
            .and_then(Value::as_object)
            .and_then(|hash| hash.get("value"))
            .and_then(Value::as_str)
            .is_some_and(valid_sha256)
        && source
            .get("semantic_hash")
            .and_then(Value::as_object)
            .is_some_and(|hash| exact_key_set(hash, &["algorithm", "value"]));
    if valid {
        Ok(())
    } else {
        Err(invalid_projection_value(complete, "invalid_shape"))
    }
}

fn family<'a>(
    value: &'a Value,
    field: &'static str,
    maximum: usize,
) -> Result<&'a [Value], R8ContractError> {
    let values = value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_projection_value(value, "missing_member"))?;
    if values.len() > maximum {
        return Err(R8ContractError::LimitExceeded {
            limit: "portable_graph_bytes",
            maximum: u64::try_from(maximum).unwrap_or(u64::MAX),
            observed: u64::try_from(values.len()).unwrap_or(u64::MAX),
        });
    }
    Ok(values)
}

fn validate_family_ids(
    values: &[Value],
    family: &'static str,
    id_field: &str,
    sha256: R8Sha256,
) -> Result<BTreeSet<String>, R8ContractError> {
    let mut ids = BTreeSet::new();
    let mut previous: Option<&str> = None;
    for value in values {
        let id = required_string(value, id_field)?;
        if !valid_family_id(family, id) {
            return Err(invalid_projection_value(value, "invalid_shape"));
        }
        if !ids.insert(id.to_owned()) {
            return Err(R8ContractError::IdentityConflict {
                family: error_family_name(family),
                identity_sha256: sha256(id.as_bytes()),
            });
        }
        if previous.is_some_and(|previous| previous.as_bytes() >= id.as_bytes()) {
            let mut canonical = values.to_vec();
            canonical.sort_by(|left, right| {
                left.get(id_field)
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .as_bytes()
                    .cmp(
                        right
                            .get(id_field)
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .as_bytes(),
                    )
            });
            let expected = serde_json::to_vec(&canonical).map_err(|_| R8ContractError::Internal)?;
            let observed = serde_json::to_vec(values).map_err(|_| R8ContractError::Internal)?;
            return Err(R8ContractError::Noncanonical {
                expected_sha256: sha256(&expected),
                observed_sha256: sha256(&observed),
            });
        }
        previous = Some(id);
    }
    Ok(ids)
}

fn validate_document_shape(document: &Value, complete: &Value) -> Result<(), R8ContractError> {
    let object = document
        .as_object()
        .ok_or_else(|| invalid_projection_value(complete, "invalid_shape"))?;
    require_exact_keys(
        object,
        &[
            "blake3",
            "byte_length",
            "document_id",
            "kind",
            "path",
            "subject_id",
        ],
        complete,
    )?;
    let valid = object
        .get("kind")
        .and_then(Value::as_str)
        .is_some_and(|kind| matches!(kind, "overview" | "module"))
        && object
            .get("byte_length")
            .and_then(Value::as_u64)
            .is_some_and(|length| (1..=1_048_576).contains(&length))
        && object
            .get("blake3")
            .and_then(Value::as_str)
            .is_some_and(valid_sha256)
        && object
            .get("path")
            .and_then(Value::as_str)
            .is_some_and(valid_document_path);
    if valid {
        Ok(())
    } else {
        Err(invalid_projection_value(complete, "invalid_shape"))
    }
}

fn validate_statement_shape(statement: &Value, complete: &Value) -> Result<(), R8ContractError> {
    let object = statement
        .as_object()
        .ok_or_else(|| invalid_projection_value(complete, "invalid_shape"))?;
    require_exact_keys(
        object,
        &[
            "coverage_gap_ids",
            "document_id",
            "evidence_ids",
            "statement_id",
            "subject_ids",
            "truth_state",
        ],
        complete,
    )?;
    let truth = object.get("truth_state").and_then(Value::as_str);
    let evidence = required_string_array(statement, "evidence_ids")?;
    let gaps = required_string_array(statement, "coverage_gap_ids")?;
    let valid = match truth {
        Some("deterministic_fact" | "derived_fact") => !evidence.is_empty() && gaps.is_empty(),
        Some("unsupported") => evidence.is_empty() && !gaps.is_empty(),
        _ => false,
    };
    if valid && !required_string_array(statement, "subject_ids")?.is_empty() {
        Ok(())
    } else {
        Err(invalid_projection_value(complete, "invalid_shape"))
    }
}

fn validate_evidence_references(
    _family: &str,
    records: &[Value],
    id_field: &str,
    evidence_ids: &BTreeSet<String>,
    sha256: R8Sha256,
) -> Result<(), R8ContractError> {
    for record in records {
        let subject = required_string(record, id_field)?;
        collect_evidence_ids(record, &mut |evidence_id| {
            if evidence_ids.contains(evidence_id) {
                Ok(())
            } else {
                Err(R8ContractError::UnresolvedEvidence {
                    subject_id_sha256: sha256(subject.as_bytes()),
                    evidence_id: evidence_id.to_owned(),
                })
            }
        })?;
    }
    Ok(())
}

fn collect_evidence_ids(
    value: &Value,
    visit: &mut impl FnMut(&str) -> Result<(), R8ContractError>,
) -> Result<(), R8ContractError> {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                if matches!(
                    key.as_str(),
                    "evidence_ids" | "compiler_evidence_ids" | "source_evidence_ids"
                ) {
                    for evidence_id in value
                        .as_array()
                        .ok_or_else(|| invalid_projection_value(value, "invalid_shape"))?
                    {
                        visit(
                            evidence_id
                                .as_str()
                                .ok_or_else(|| invalid_projection_value(value, "invalid_shape"))?,
                        )?;
                    }
                } else {
                    collect_evidence_ids(value, visit)?;
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_evidence_ids(value, visit)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_portable_paths(value: &Value, sha256: R8Sha256) -> Result<(), R8ContractError> {
    fn visit(value: &Value, field: Option<&str>, sha256: R8Sha256) -> Result<(), R8ContractError> {
        match value {
            Value::Object(object) => {
                for (key, value) in object {
                    visit(value, Some(key), sha256)?;
                }
            }
            Value::Array(values) => {
                for value in values {
                    visit(value, field, sha256)?;
                }
            }
            Value::String(path)
                if matches!(field, Some("path" | "document_path")) && !safe_relative_path(path) =>
            {
                return Err(R8ContractError::InvalidProjection {
                    projection_sha256: sha256(path.as_bytes()),
                    reason: "unsafe_path",
                });
            }
            _ => {}
        }
        Ok(())
    }
    visit(value, None, sha256)
}

fn validate_untrusted_values(value: &Value, sha256: R8Sha256) -> Result<(), R8ContractError> {
    fn visit(value: &Value, field: Option<&str>, sha256: R8Sha256) -> Result<(), R8ContractError> {
        match value {
            Value::Object(object) => {
                for (key, value) in object {
                    if matches!(
                        key.as_str(),
                        "source_content"
                            | "source_contents"
                            | "source_snippet"
                            | "source_snippets"
                            | "raw_arguments"
                            | "raw_project_root"
                            | "absolute_path"
                            | "file_url"
                    ) {
                        return Err(R8ContractError::InvalidProjection {
                            projection_sha256: sha256(key.as_bytes()),
                            reason: "unknown_field",
                        });
                    }
                    visit(value, Some(key), sha256)?;
                }
            }
            Value::Array(values) => {
                for value in values {
                    visit(value, field, sha256)?;
                }
            }
            Value::String(text) => {
                let maximum = if matches!(field, Some("display_name" | "name" | "message")) {
                    MAX_DISPLAY_BYTES
                } else {
                    MAX_METADATA_BYTES
                };
                if text.len() > maximum {
                    return Err(unsafe_payload(text, "oversized_value", sha256));
                }
                if text.chars().any(disallowed_control) {
                    return Err(unsafe_payload(text, "control_character", sha256));
                }
                if text.chars().any(is_bidi_control) {
                    return Err(unsafe_payload(text, "bidi_control", sha256));
                }
                if matches!(field, Some("display_name" | "name" | "message")) {
                    let lower = text.to_ascii_lowercase();
                    if lower.contains("</script")
                        || lower.contains("<script")
                        || lower.contains("onfocus=")
                        || lower.contains("onerror=")
                    {
                        return Err(unsafe_payload(text, "active_content", sha256));
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
    visit(value, None, sha256)
}

fn preflight_json(bytes: &[u8], sha256: R8Sha256) -> Result<(), R8ContractError> {
    let projection_sha256 = sha256(bytes);
    let mut parser = JsonPreflight::new(bytes, projection_sha256.clone());
    parser.parse_value(1)?;
    parser.skip_whitespace();
    if parser.cursor == bytes.len() {
        Ok(())
    } else {
        Err(R8ContractError::InvalidProjection {
            projection_sha256,
            reason: "invalid_json",
        })
    }
}

struct JsonPreflight<'a> {
    bytes: &'a [u8],
    cursor: usize,
    projection_sha256: String,
}

impl<'a> JsonPreflight<'a> {
    fn new(bytes: &'a [u8], projection_sha256: String) -> Self {
        Self {
            bytes,
            cursor: 0,
            projection_sha256,
        }
    }

    fn parse_value(&mut self, depth: u64) -> Result<(), R8ContractError> {
        self.skip_whitespace();
        match self.bytes.get(self.cursor).copied() {
            Some(b'{' | b'[') if depth > MAX_R8_JSON_NESTING => {
                Err(R8ContractError::LimitExceeded {
                    limit: "json_nesting",
                    maximum: MAX_R8_JSON_NESTING,
                    observed: depth,
                })
            }
            Some(b'{') => self.parse_object(depth),
            Some(b'[') => self.parse_array(depth),
            Some(b'"') => self.parse_string().map(|_| ()),
            Some(_) => self.parse_primitive(),
            None => self.invalid(),
        }
    }

    fn parse_object(&mut self, depth: u64) -> Result<(), R8ContractError> {
        self.cursor += 1;
        self.skip_whitespace();
        if self.consume(b'}') {
            return Ok(());
        }
        let mut keys = BTreeSet::new();
        loop {
            self.skip_whitespace();
            let key = self.parse_string()?;
            if !keys.insert(key) {
                return Err(R8ContractError::InvalidProjection {
                    projection_sha256: self.projection_sha256.clone(),
                    reason: "duplicate_member",
                });
            }
            self.skip_whitespace();
            if !self.consume(b':') {
                return self.invalid();
            }
            self.parse_value(depth + 1)?;
            self.skip_whitespace();
            if self.consume(b'}') {
                return Ok(());
            }
            if !self.consume(b',') {
                return self.invalid();
            }
        }
    }

    fn parse_array(&mut self, depth: u64) -> Result<(), R8ContractError> {
        self.cursor += 1;
        self.skip_whitespace();
        if self.consume(b']') {
            return Ok(());
        }
        loop {
            self.parse_value(depth + 1)?;
            self.skip_whitespace();
            if self.consume(b']') {
                return Ok(());
            }
            if !self.consume(b',') {
                return self.invalid();
            }
        }
    }

    fn parse_string(&mut self) -> Result<String, R8ContractError> {
        let start = self.cursor;
        if !self.consume(b'"') {
            return self.invalid();
        }
        let mut escaped = false;
        while let Some(byte) = self.bytes.get(self.cursor).copied() {
            self.cursor += 1;
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                let projection_sha256 = self.projection_sha256.clone();
                return serde_json::from_slice(&self.bytes[start..self.cursor]).map_err(|_| {
                    R8ContractError::InvalidProjection {
                        projection_sha256,
                        reason: "invalid_json",
                    }
                });
            } else if byte < 0x20 {
                return self.invalid();
            }
        }
        self.invalid()
    }

    fn parse_primitive(&mut self) -> Result<(), R8ContractError> {
        let start = self.cursor;
        while self
            .bytes
            .get(self.cursor)
            .is_some_and(|byte| !matches!(*byte, b',' | b']' | b'}' | b' ' | b'\t' | b'\r' | b'\n'))
        {
            self.cursor += 1;
        }
        if self.cursor == start {
            self.invalid()
        } else {
            Ok(())
        }
    }

    fn skip_whitespace(&mut self) {
        while self
            .bytes
            .get(self.cursor)
            .is_some_and(|byte| matches!(*byte, b' ' | b'\t' | b'\r' | b'\n'))
        {
            self.cursor += 1;
        }
    }

    fn consume(&mut self, expected: u8) -> bool {
        if self.bytes.get(self.cursor) == Some(&expected) {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    fn invalid<T>(&self) -> Result<T, R8ContractError> {
        Err(R8ContractError::InvalidProjection {
            projection_sha256: self.projection_sha256.clone(),
            reason: "invalid_json",
        })
    }
}

fn require_exact_keys(
    object: &Map<String, Value>,
    expected: &[&str],
    complete: &Value,
) -> Result<(), R8ContractError> {
    if exact_key_set(object, expected) {
        Ok(())
    } else {
        let expected = expected.iter().copied().collect::<BTreeSet<_>>();
        let observed = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
        let reason = if observed.is_superset(&expected) {
            "unknown_field"
        } else {
            "missing_member"
        };
        Err(invalid_projection_value(complete, reason))
    }
}

fn exact_key_set(object: &Map<String, Value>, expected: &[&str]) -> bool {
    object.len() == expected.len() && expected.iter().all(|key| object.contains_key(*key))
}

fn required_string<'a>(value: &'a Value, field: &str) -> Result<&'a str, R8ContractError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid_projection_value(value, "invalid_shape"))
}

fn required_string_array<'a>(
    value: &'a Value,
    field: &str,
) -> Result<Vec<&'a str>, R8ContractError> {
    let values = value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_projection_value(value, "invalid_shape"))?;
    let mut seen = BTreeSet::new();
    values
        .iter()
        .map(|item| {
            let item = item
                .as_str()
                .filter(|item| !item.is_empty())
                .ok_or_else(|| invalid_projection_value(value, "invalid_shape"))?;
            if seen.insert(item) {
                Ok(item)
            } else {
                Err(invalid_projection_value(value, "invalid_shape"))
            }
        })
        .collect()
}

fn valid_family_id(family: &str, id: &str) -> bool {
    let prefix = match family {
        "entities" => "urn:codenoesis:entity:blake3:",
        "relationships" => "urn:codenoesis:relationship:blake3:",
        "claims" => "urn:codenoesis:claim:blake3:",
        "evidence" => {
            return [
                "urn:codenoesis:evidence:blake3:",
                "urn:codenoesis:evidence:sha256:",
            ]
            .iter()
            .any(|prefix| valid_prefixed_digest(id, prefix));
        }
        "diagnostics" => "urn:codenoesis:diagnostic:blake3:",
        "coverage_gaps" => "urn:codenoesis:coverage-gap:blake3:",
        "documents" => "urn:codenoesis:document:blake3:",
        "document_statements" => "urn:codenoesis:statement:blake3:",
        _ => return false,
    };
    valid_prefixed_digest(id, prefix)
}

fn valid_prefixed_digest(value: &str, prefix: &str) -> bool {
    value.strip_prefix(prefix).is_some_and(valid_sha256)
}

fn error_family_name(family: &str) -> &'static str {
    match family {
        "relationships" => "relationship",
        "claims" => "claim",
        "evidence" => "evidence",
        "diagnostics" => "diagnostic",
        "coverage_gaps" => "coverage_gap",
        "documents" => "document",
        "document_statements" => "document_statement",
        _ => "entity",
    }
}

fn family_id_field(family: &str) -> &'static str {
    match family {
        "documents" => "document_id",
        "document_statements" => "statement_id",
        _ => "id",
    }
}

fn valid_sha1(value: &str) -> bool {
    value.len() == 40 && lower_hex_bytes(value)
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && lower_hex_bytes(value)
}

fn lower_hex_bytes(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn valid_snapshot_id(value: &str) -> bool {
    valid_prefixed_digest(value, "urn:codenoesis:snapshot:blake3:")
}

fn valid_evidence_id(value: &str) -> bool {
    [
        "urn:codenoesis:evidence:blake3:",
        "urn:codenoesis:evidence:sha256:",
    ]
    .iter()
    .any(|prefix| valid_prefixed_digest(value, prefix))
}

fn valid_document_path(path: &str) -> bool {
    path == "overview.md"
        || path
            .strip_prefix("modules/")
            .and_then(|path| path.strip_suffix(".md"))
            .is_some_and(|slug| {
                !slug.is_empty()
                    && slug.bytes().all(|byte| {
                        byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'
                    })
            })
}

fn safe_relative_path(path: &str) -> bool {
    let path = Path::new(path);
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path.components().all(|component| {
            matches!(component, Component::Normal(_))
                && component
                    .as_os_str()
                    .to_str()
                    .is_some_and(|value| value != "." && value != "..")
        })
}

fn disallowed_control(character: char) -> bool {
    matches!(character, '\u{0000}'..='\u{0008}' | '\u{000b}' | '\u{000c}' | '\u{000e}'..='\u{001f}' | '\u{007f}' | '\u{2028}' | '\u{2029}')
}

fn is_bidi_control(character: char) -> bool {
    matches!(
        character,
        '\u{061c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
    )
}

fn unsafe_payload(value: &str, reason: &'static str, sha256: R8Sha256) -> R8ContractError {
    R8ContractError::UnsafePayload {
        field_sha256: sha256(value.as_bytes()),
        reason,
    }
}

fn unsafe_payload_bytes(value: &[u8], reason: &'static str, sha256: R8Sha256) -> R8ContractError {
    R8ContractError::UnsafePayload {
        field_sha256: sha256(value),
        reason,
    }
}

fn invalid_projection_value(value: &Value, reason: &'static str) -> R8ContractError {
    let _ = value;
    R8ContractError::InvalidProjection {
        projection_sha256: ZERO_SHA256.to_owned(),
        reason,
    }
}

fn bounded(value: &str, maximum: usize) -> String {
    value.chars().take(maximum).collect()
}

fn valid_sha256_or_zero(value: &str) -> &str {
    if valid_sha256(value) {
        value
    } else {
        ZERO_SHA256
    }
}

fn valid_snapshot_id_or_zero(value: &str) -> &str {
    if valid_snapshot_id(value) {
        value
    } else {
        ZERO_SNAPSHOT_ID
    }
}

fn valid_evidence_id_or_zero(value: &str) -> &str {
    if valid_evidence_id(value) {
        value
    } else {
        "urn:codenoesis:evidence:sha256:0000000000000000000000000000000000000000000000000000000000000000"
    }
}

fn valid_path_reason(reason: &str) -> &str {
    match reason {
        "absolute"
        | "parent_escape"
        | "symlink_escape"
        | "non_empty_unmarked"
        | "marker_mismatch"
        | "outside_destination" => reason,
        _ => "outside_destination",
    }
}

fn valid_snapshot_reason(reason: &str) -> &str {
    match reason {
        "missing_visible_head"
        | "corrupt"
        | "unbound"
        | "semantic_hash_mismatch"
        | "incomplete" => reason,
        _ => "corrupt",
    }
}

fn valid_projection_reason(reason: &str) -> &str {
    match reason {
        "invalid_json" | "duplicate_member" | "missing_member" | "unknown_field"
        | "invalid_shape" | "unsafe_path" => reason,
        _ => "invalid_shape",
    }
}

fn valid_payload_reason(reason: &str) -> &str {
    match reason {
        "active_content" | "control_character" | "bidi_control" | "oversized_value"
        | "remote_origin" | "dynamic_code" => reason,
        _ => "active_content",
    }
}
