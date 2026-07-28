use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::knowledge::{ClaimState, ClaimSubjectKind, EntityKind, RelationshipKind};

pub const S4_ONTOLOGY_VERSION: &str = "codenoesis.ontology/rust/v2";
pub const S4_TREE_SITTER_EXTRACTOR_VERSION: &str = "codenoesis.rust-tree-sitter/s4-v1";
pub const S4_WORKSPACE_EXTRACTOR_VERSION: &str = "codenoesis.rust-workspace/s4-v1";
pub const MAX_S4_WORKSPACE_CRATES: usize = 200;

const ENTITY_ID_DOMAIN: &str = "codenoesis.entity-id/rust/v2";
const RELATIONSHIP_ID_DOMAIN: &str = "codenoesis.relationship-id/rust/v2";
const CLAIM_ID_DOMAIN: &str = "codenoesis.claim-id/v2";
const EVIDENCE_ID_DOMAIN: &str = "codenoesis.evidence-id/v2";
const COVERAGE_GAP_ID_DOMAIN: &str = "codenoesis.coverage-gap-id/v2";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum WorkspaceVisibility {
    Public,
    Private,
    InheritedTrait,
    NotApplicable,
}

impl WorkspaceVisibility {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
            Self::InheritedTrait => "inherited_trait",
            Self::NotApplicable => "not_applicable",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceEntity {
    pub id: String,
    pub kind: EntityKind,
    pub crate_id: Option<String>,
    pub module_path: Option<String>,
    pub name: String,
    pub visibility: WorkspaceVisibility,
    pub properties: BTreeMap<String, String>,
}

impl WorkspaceEntity {
    #[must_use]
    pub fn rust_crate(
        repository_identity: &str,
        manifest_path: &str,
        package_name: &str,
        target_kind: &str,
        target_name: &str,
    ) -> Self {
        let id = workspace_crate_id(
            repository_identity,
            manifest_path,
            package_name,
            target_kind,
            target_name,
        );
        Self {
            id,
            kind: EntityKind::RustCrate,
            crate_id: None,
            module_path: None,
            name: target_name.to_owned(),
            visibility: WorkspaceVisibility::NotApplicable,
            properties: BTreeMap::from([
                ("manifest_path".to_owned(), manifest_path.to_owned()),
                ("package_name".to_owned(), package_name.to_owned()),
                ("target_kind".to_owned(), target_kind.to_owned()),
                ("target_name".to_owned(), target_name.to_owned()),
            ]),
        }
    }

    #[must_use]
    pub fn source_file(
        repository_identity: &str,
        crate_id: &str,
        path: &str,
        blob_oid: &str,
    ) -> Self {
        Self {
            id: workspace_source_file_id(repository_identity, crate_id, path),
            kind: EntityKind::SourceFile,
            crate_id: Some(crate_id.to_owned()),
            module_path: None,
            name: path.to_owned(),
            visibility: WorkspaceVisibility::NotApplicable,
            properties: BTreeMap::from([
                ("blob_oid".to_owned(), blob_oid.to_owned()),
                ("path".to_owned(), path.to_owned()),
            ]),
        }
    }

    #[must_use]
    pub fn module(
        repository_identity: &str,
        crate_id: &str,
        module_path: &str,
        name: &str,
        visibility: WorkspaceVisibility,
        source_file_id: &str,
    ) -> Self {
        Self {
            id: workspace_module_id(repository_identity, crate_id, module_path),
            kind: EntityKind::RustModule,
            crate_id: Some(crate_id.to_owned()),
            module_path: Some(module_path.to_owned()),
            name: name.to_owned(),
            visibility,
            properties: BTreeMap::from([("source_file_id".to_owned(), source_file_id.to_owned())]),
        }
    }

    #[must_use]
    pub fn declaration(
        repository_identity: &str,
        kind: EntityKind,
        crate_id: &str,
        module_path: &str,
        name: &str,
        visibility: WorkspaceVisibility,
    ) -> Self {
        Self {
            id: workspace_declaration_id(repository_identity, kind, crate_id, module_path, name),
            kind,
            crate_id: Some(crate_id.to_owned()),
            module_path: Some(module_path.to_owned()),
            name: name.to_owned(),
            visibility,
            properties: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn unresolved_symbol(
        repository_identity: &str,
        crate_id: &str,
        module_path: &str,
        spelling: &str,
    ) -> Self {
        Self {
            id: workspace_declaration_id(
                repository_identity,
                EntityKind::RustSymbolReference,
                crate_id,
                module_path,
                spelling,
            ),
            kind: EntityKind::RustSymbolReference,
            crate_id: Some(crate_id.to_owned()),
            module_path: Some(module_path.to_owned()),
            name: spelling.to_owned(),
            visibility: WorkspaceVisibility::NotApplicable,
            properties: BTreeMap::from([
                ("resolution_state".to_owned(), "unresolved".to_owned()),
                ("spelling".to_owned(), spelling.to_owned()),
            ]),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceRelationship {
    pub id: String,
    pub kind: RelationshipKind,
    pub source: String,
    pub target: String,
    pub evidence_ids: Vec<String>,
}

impl WorkspaceRelationship {
    #[must_use]
    pub fn new(
        kind: RelationshipKind,
        source: String,
        target: String,
        evidence_ids: Vec<String>,
    ) -> Self {
        Self {
            id: workspace_relationship_id(kind, &source, &target),
            kind,
            source,
            target,
            evidence_ids: stable_dedup(evidence_ids),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceClaim {
    pub id: String,
    pub subject_kind: ClaimSubjectKind,
    pub subject_id: String,
    pub state: ClaimState,
    pub evidence_ids: Vec<String>,
}

impl WorkspaceClaim {
    #[must_use]
    pub fn new(
        subject_kind: ClaimSubjectKind,
        subject_id: String,
        state: ClaimState,
        evidence_ids: Vec<String>,
    ) -> Self {
        Self {
            id: workspace_claim_id(subject_kind, &subject_id, state),
            subject_kind,
            subject_id,
            state,
            evidence_ids: stable_dedup(evidence_ids),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceEvidence {
    pub id: String,
    pub path: String,
    pub blob_oid: String,
    pub start_byte: u64,
    pub end_byte: u64,
}

impl WorkspaceEvidence {
    #[must_use]
    pub fn complete_file(
        repository_identity: &str,
        commit_oid: &str,
        path: &str,
        blob_oid: &str,
        byte_length: u64,
    ) -> Self {
        Self {
            id: workspace_evidence_id(
                repository_identity,
                commit_oid,
                blob_oid,
                path,
                0,
                byte_length,
            ),
            path: path.to_owned(),
            blob_oid: blob_oid.to_owned(),
            start_byte: 0,
            end_byte: byte_length,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceDiagnostic {
    pub code: String,
    pub message: String,
    pub evidence_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceCoverageGap {
    pub id: String,
    pub capability: String,
    pub evidence_ids: Vec<String>,
}

impl WorkspaceCoverageGap {
    #[must_use]
    pub fn unsupported(
        repository_identity: &str,
        commit_oid: &str,
        capability: &str,
        evidence_id: &str,
    ) -> Self {
        Self {
            id: workspace_coverage_gap_id(repository_identity, commit_oid, capability, evidence_id),
            capability: capability.to_owned(),
            evidence_ids: vec![evidence_id.to_owned()],
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceExtractionChunk {
    pub repository_identity: String,
    pub crate_id: String,
    pub source_file_id: String,
    pub entities: Vec<WorkspaceEntity>,
    pub relationships: Vec<WorkspaceRelationship>,
    pub claims: Vec<WorkspaceClaim>,
    pub evidence: Vec<WorkspaceEvidence>,
    pub diagnostics: Vec<WorkspaceDiagnostic>,
    pub coverage: Vec<WorkspaceCoverageGap>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceKnowledgeGraph {
    pub repository_identity: String,
    pub commit_oid: String,
    pub entities: Vec<WorkspaceEntity>,
    pub relationships: Vec<WorkspaceRelationship>,
    pub claims: Vec<WorkspaceClaim>,
    pub evidence: Vec<WorkspaceEvidence>,
    pub diagnostics: Vec<WorkspaceDiagnostic>,
    pub coverage: Vec<WorkspaceCoverageGap>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustWorkspaceKnowledge {
    pub extraction_chunks: Vec<WorkspaceExtractionChunk>,
    pub graph: WorkspaceKnowledgeGraph,
}

impl RustWorkspaceKnowledge {
    /// Validates stable identities, ordering, and reference integrity.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceError::ContractInvalid`] for malformed S4 knowledge.
    pub fn validate(&self) -> Result<(), WorkspaceError> {
        self.graph.validate()?;
        if self.extraction_chunks.is_empty()
            || self.extraction_chunks.iter().any(|chunk| {
                chunk.repository_identity != self.graph.repository_identity
                    || chunk.entities.is_empty()
                    || !ordered_unique(chunk.entities.iter().map(|entity| entity.id.as_str()))
                    || !ordered_unique(
                        chunk
                            .relationships
                            .iter()
                            .map(|relationship| relationship.id.as_str()),
                    )
                    || !ordered_unique(chunk.claims.iter().map(|claim| claim.id.as_str()))
                    || !ordered_unique(chunk.evidence.iter().map(|evidence| evidence.id.as_str()))
                    || !ordered_unique(chunk.coverage.iter().map(|gap| gap.id.as_str()))
            })
        {
            return Err(WorkspaceError::ContractInvalid);
        }
        Ok(())
    }
}

impl WorkspaceKnowledgeGraph {
    /// Validates stable graph identities and exact referential closure.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceError::ContractInvalid`] for the first invalid
    /// invariant.
    pub fn validate(&self) -> Result<(), WorkspaceError> {
        if self.entities.is_empty()
            || self.evidence.is_empty()
            || !ordered_unique(self.entities.iter().map(|entity| entity.id.as_str()))
            || !ordered_unique(
                self.relationships
                    .iter()
                    .map(|relationship| relationship.id.as_str()),
            )
            || !ordered_unique(self.claims.iter().map(|claim| claim.id.as_str()))
            || !ordered_unique(self.evidence.iter().map(|evidence| evidence.id.as_str()))
            || !ordered_unique(self.coverage.iter().map(|gap| gap.id.as_str()))
        {
            return Err(WorkspaceError::ContractInvalid);
        }
        let entity_ids = self
            .entities
            .iter()
            .map(|entity| entity.id.as_str())
            .collect::<BTreeSet<_>>();
        let relationship_ids = self
            .relationships
            .iter()
            .map(|relationship| relationship.id.as_str())
            .collect::<BTreeSet<_>>();
        let evidence_ids = self
            .evidence
            .iter()
            .map(|evidence| evidence.id.as_str())
            .collect::<BTreeSet<_>>();
        if self.relationships.iter().any(|relationship| {
            relationship.id
                != workspace_relationship_id(
                    relationship.kind,
                    &relationship.source,
                    &relationship.target,
                )
                || !entity_ids.contains(relationship.source.as_str())
                || !entity_ids.contains(relationship.target.as_str())
                || relationship.evidence_ids.is_empty()
                || relationship
                    .evidence_ids
                    .iter()
                    .any(|id| !evidence_ids.contains(id.as_str()))
        }) {
            return Err(WorkspaceError::ContractInvalid);
        }
        let subjects = entity_ids
            .iter()
            .map(|id| (ClaimSubjectKind::Entity, *id))
            .chain(
                relationship_ids
                    .iter()
                    .map(|id| (ClaimSubjectKind::Relationship, *id)),
            )
            .collect::<BTreeSet<_>>();
        let claimed_subjects = self
            .claims
            .iter()
            .map(|claim| (claim.subject_kind, claim.subject_id.as_str()))
            .collect::<BTreeSet<_>>();
        if subjects != claimed_subjects
            || self.claims.len() != subjects.len()
            || self.claims.iter().any(|claim| {
                claim.id != workspace_claim_id(claim.subject_kind, &claim.subject_id, claim.state)
                    || claim
                        .evidence_ids
                        .iter()
                        .any(|id| !evidence_ids.contains(id.as_str()))
            })
            || self.coverage.iter().any(|gap| {
                gap.evidence_ids.len() != 1
                    || !evidence_ids.contains(gap.evidence_ids[0].as_str())
                    || gap.id
                        != workspace_coverage_gap_id(
                            &self.repository_identity,
                            &self.commit_oid,
                            &gap.capability,
                            &gap.evidence_ids[0],
                        )
            })
        {
            return Err(WorkspaceError::ContractInvalid);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkspaceError {
    InvalidUtf8 {
        path: String,
    },
    UnsupportedWorkspace,
    AmbiguousModule,
    ParserCancelled {
        path: String,
    },
    MalformedSyntax {
        path: String,
    },
    ContractInvalid,
    LimitExceeded {
        limit: &'static str,
        maximum: u64,
        observed: u64,
    },
}

impl Display for WorkspaceError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidUtf8 { .. } => "invalid UTF-8 workspace input",
            Self::UnsupportedWorkspace => "unsupported workspace",
            Self::AmbiguousModule => "ambiguous module",
            Self::ParserCancelled { .. } => "Rust parser cancelled",
            Self::MalformedSyntax { .. } => "malformed Rust syntax",
            Self::ContractInvalid => "invalid workspace extraction contract",
            Self::LimitExceeded { .. } => "workspace extraction limit exceeded",
        })
    }
}

impl Error for WorkspaceError {}

#[must_use]
pub fn workspace_crate_id(
    repository_identity: &str,
    manifest_path: &str,
    package_name: &str,
    target_kind: &str,
    target_name: &str,
) -> String {
    stable_id(
        "urn:codenoesis:entity:blake3:",
        &[
            ENTITY_ID_DOMAIN,
            repository_identity,
            "crate",
            manifest_path,
            package_name,
            target_kind,
            target_name,
        ],
    )
}

#[must_use]
pub fn workspace_source_file_id(
    repository_identity: &str,
    crate_id: &str,
    source_path: &str,
) -> String {
    stable_id(
        "urn:codenoesis:entity:blake3:",
        &[
            ENTITY_ID_DOMAIN,
            repository_identity,
            "source_file",
            crate_id,
            source_path,
        ],
    )
}

#[must_use]
pub fn workspace_module_id(repository_identity: &str, crate_id: &str, module_path: &str) -> String {
    stable_id(
        "urn:codenoesis:entity:blake3:",
        &[
            ENTITY_ID_DOMAIN,
            repository_identity,
            "module",
            crate_id,
            module_path,
        ],
    )
}

#[must_use]
pub fn workspace_declaration_id(
    repository_identity: &str,
    kind: EntityKind,
    crate_id: &str,
    module_path: &str,
    name: &str,
) -> String {
    stable_id(
        "urn:codenoesis:entity:blake3:",
        &[
            ENTITY_ID_DOMAIN,
            repository_identity,
            identity_entity_kind(kind),
            crate_id,
            module_path,
            name,
        ],
    )
}

#[must_use]
pub fn workspace_relationship_id(kind: RelationshipKind, source: &str, target: &str) -> String {
    stable_id(
        "urn:codenoesis:relationship:blake3:",
        &[RELATIONSHIP_ID_DOMAIN, kind.as_str(), source, target],
    )
}

#[must_use]
pub fn workspace_claim_id(
    subject_kind: ClaimSubjectKind,
    subject_id: &str,
    state: ClaimState,
) -> String {
    stable_id(
        "urn:codenoesis:claim:blake3:",
        &[
            CLAIM_ID_DOMAIN,
            subject_kind.as_str(),
            subject_id,
            state.as_str(),
        ],
    )
}

#[must_use]
pub fn workspace_evidence_id(
    repository_identity: &str,
    commit_oid: &str,
    blob_oid: &str,
    path: &str,
    start_byte: u64,
    end_byte: u64,
) -> String {
    stable_id(
        "urn:codenoesis:evidence:blake3:",
        &[
            EVIDENCE_ID_DOMAIN,
            repository_identity,
            commit_oid,
            blob_oid,
            path,
            &start_byte.to_string(),
            &end_byte.to_string(),
        ],
    )
}

#[must_use]
pub fn workspace_coverage_gap_id(
    repository_identity: &str,
    commit_oid: &str,
    capability: &str,
    evidence_id: &str,
) -> String {
    stable_id(
        "urn:codenoesis:coverage-gap:blake3:",
        &[
            COVERAGE_GAP_ID_DOMAIN,
            repository_identity,
            commit_oid,
            capability,
            evidence_id,
        ],
    )
}

const fn identity_entity_kind(kind: EntityKind) -> &'static str {
    match kind {
        EntityKind::RustCrate => "crate",
        EntityKind::SourceFile => "source_file",
        EntityKind::RustModule => "module",
        EntityKind::RustStruct => "struct",
        EntityKind::RustEnum => "enum",
        EntityKind::RustTrait => "trait",
        EntityKind::RustTypeAlias => "type_alias",
        EntityKind::RustFunction => "function",
        EntityKind::RustMethod => "method",
        EntityKind::RustSymbolReference => "symbol_reference",
    }
}

fn ordered_unique<'a>(values: impl IntoIterator<Item = &'a str>) -> bool {
    let mut previous = None;
    for value in values {
        if previous.is_some_and(|previous_value| previous_value >= value) {
            return false;
        }
        previous = Some(value);
    }
    true
}

fn stable_dedup(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter(|value| seen.insert(value.clone()))
        .collect()
}

fn stable_id(prefix: &str, components: &[&str]) -> String {
    let preimage = canonical_string_array(components);
    format!("{prefix}{}", blake3::hash(&preimage).to_hex())
}

fn canonical_string_array(components: &[&str]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.push(b'[');
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
            '\u{0008}' => bytes.extend_from_slice(br"\b"),
            '\u{000c}' => bytes.extend_from_slice(br"\f"),
            '\n' => bytes.extend_from_slice(br"\n"),
            '\r' => bytes.extend_from_slice(br"\r"),
            '\t' => bytes.extend_from_slice(br"\t"),
            '\u{0000}'..='\u{001f}' => {
                use std::fmt::Write as _;
                write!(StringByteWriter(bytes), "\\u{:04x}", u32::from(character))
                    .expect("writing to a byte vector cannot fail");
            }
            _ => {
                let mut encoded = [0; 4];
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
