//! Canonical Kotlin snapshot and portable projection contracts.
use crate::{LimitedVecWriter, SnapshotEnvelopeV1, publication_candidate, semantic_hash};
use codenoesis_domain::RepositoryInventory;
use codenoesis_domain::s7::SourceSpan;
use codenoesis_domain::s8_kotlin::{
    self as domain, DeclarationLocation, KotlinError, KotlinWorkspace,
};
use codenoesis_domain::storage::{LocalSnapshotHead, PublicationCandidate};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub const PORTABLE_VERSION: &str = "codenoesis.kotlin-portable-graph/v1";

#[derive(Clone, Debug)]
pub struct KotlinSnapshot {
    value: Value,
}

impl KotlinSnapshot {
    /// Builds one versioned, evidence-backed projection of committed syntax.
    /// # Errors
    /// Returns a capacity or invalid extraction contract failure.
    pub fn from_workspace(
        inventory: &RepositoryInventory,
        workspace: &KotlinWorkspace,
        envelope: &SnapshotEnvelopeV1,
    ) -> Result<Self, KotlinError> {
        let mut builder = Builder::new(inventory);
        builder.populate(workspace)?;
        let graph = builder.finish()?;
        let bound = inventory.bound_revision();
        let semantic = json!({
            "repository": {"identity":bound.repository_identity().as_str(), "commit_oid":bound.commit_oid().as_str(), "tree_oid":bound.tree_oid().as_str()},
            "profile":domain::PROFILE, "ontology_version":domain::ONTOLOGY_VERSION,
            "knowledge_graph":graph, "extraction_chunks":[],
            "configuration":{"gradle":"not_evaluated","parser":"tree-sitter-kotlin-ng/1.1.0","membership":"path_convention"}
        });
        validate_semantic(&semantic)?;
        let value = json!({"schema_version":domain::SNAPSHOT_VERSION,"semantic_hash":hash(domain::SNAPSHOT_HASH_DOMAIN,&semantic),"semantic":semantic,
            "envelope":{"created_at":envelope.created_at,"job_id":envelope.job_id,"correlation_id":envelope.correlation_id}});
        let snapshot = Self { value };
        snapshot.canonical_stdout()?;
        snapshot.publication_candidate()?;
        Ok(snapshot)
    }
    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }
    /// # Errors
    /// Returns a capacity failure before publication.
    pub fn canonical_stdout(&self) -> Result<Vec<u8>, KotlinError> {
        canonical(&self.value)
    }
    /// # Errors
    /// Returns an invalid publication contract failure.
    pub fn publication_candidate(&self) -> Result<PublicationCandidate, KotlinError> {
        publication_candidate(&self.value).map_err(|_| KotlinError::InvalidContract)
    }
}

struct Builder<'a> {
    inventory: &'a RepositoryInventory,
    entities: BTreeMap<String, Value>,
    relationships: BTreeMap<String, Value>,
    evidence: BTreeMap<String, Value>,
    claims: BTreeMap<String, Value>,
    coverage: Vec<Value>,
    declarations: BTreeMap<(usize, usize), (String, String)>,
}
impl<'a> Builder<'a> {
    fn new(inventory: &'a RepositoryInventory) -> Self {
        Self {
            inventory,
            entities: BTreeMap::new(),
            relationships: BTreeMap::new(),
            evidence: BTreeMap::new(),
            claims: BTreeMap::new(),
            coverage: Vec::new(),
            declarations: BTreeMap::new(),
        }
    }
    fn id(&self, kind: &str, key: &Value) -> String {
        let bound = self.inventory.bound_revision();
        format!(
            "urn:codenoesis:kotlin:{kind}:{}",
            semantic_hash(
                b"codenoesis.kotlin-id/v1",
                &json!([
                    bound.repository_identity().as_str(),
                    bound.commit_oid().as_str(),
                    kind,
                    key
                ])
            )
        )
    }
    fn entity(
        &mut self,
        kind: &str,
        key: &Value,
        name: &str,
        properties: Value,
        evidence: &str,
    ) -> String {
        let id = self.id("entity", &json!([kind, key]));
        if self.entities.contains_key(&id) {
            return id;
        }
        self.entities.entry(id.clone()).or_insert_with(|| {
            let mut row = json!({"id":id,"kind":kind,"name":name});
            row["properties"] = properties;
            row
        });
        self.claim("entity", &id, "Observed", &[evidence]);
        id
    }
    fn claim(&mut self, subject_kind: &str, subject: &str, state: &str, evidence: &[&str]) {
        let state = if state == "Unknown" {
            codenoesis_domain::knowledge::ClaimState::Candidate.as_str()
        } else {
            codenoesis_domain::knowledge::ClaimState::DeterministicFact.as_str()
        };
        let id = self.id("claim", &json!([subject_kind, subject]));
        let mut references: BTreeSet<String> = evidence.iter().map(|e| (*e).to_owned()).collect();
        if let Some(existing) = self.claims.get(&id)
            && let Some(previous) = existing["evidence_ids"].as_array()
        {
            references.extend(previous.iter().filter_map(Value::as_str).map(str::to_owned));
        }
        self.claims.insert(id.clone(),json!({"id":id,"subject_kind":subject_kind,"subject_id":subject,"state":state,"evidence_ids":references,"rule":domain::PROFILE}));
    }
    fn relationship(
        &mut self,
        kind: &str,
        source: &str,
        target: &str,
        state: &str,
        evidence: &[&str],
    ) {
        let id = self.id("relationship", &json!([kind, source, target]));
        self.relationships.insert(
            id.clone(),
            json!({"id":id,"kind":kind,"source":source,"target":target,"state":state}),
        );
        self.claim("relationship", &id, state, evidence);
    }
    fn source_evidence(
        &mut self,
        path: &str,
        span: Option<SourceSpan>,
    ) -> Result<String, KotlinError> {
        let file = self
            .inventory
            .files()
            .iter()
            .find(|f| f.path() == path)
            .ok_or(KotlinError::InvalidContract)?;
        let bytes = file.bytes();
        let (start, end, start_line, end_line) = span.map_or(
            (
                0,
                bytes.len(),
                1,
                bytes.split(|b| *b == b'\n').count() as u64,
            ),
            |s| (s.start_byte, s.end_byte, s.start_line, s.end_line),
        );
        let excerpt = bytes.get(start..end).ok_or(KotlinError::InvalidContract)?;
        let id = self.id("evidence", &json!([path, start, end]));
        let bound = self.inventory.bound_revision();
        self.evidence.insert(id.clone(),json!({"id":id,"repository_identity":bound.repository_identity().as_str(),"commit_oid":bound.commit_oid().as_str(),"path":path,"blob_oid":file.blob_oid().as_str(),"start_byte":start,"end_byte":end,"start_line":start_line,"end_line":end_line,"excerpt_blake3":blake3::hash(excerpt).to_hex().to_string()}));
        Ok(id)
    }
    fn gap(&mut self, reason: &str, path: &str, subject: Option<&str>) {
        self.coverage
            .push(json!({"reason":reason,"path":path,"subject_id":subject,"state":"Unknown"}));
    }
    fn populate(&mut self, workspace: &KotlinWorkspace) -> Result<(), KotlinError> {
        self.gap(
            "gradle_configuration_and_source_set_visibility_not_evaluated",
            "",
            None,
        );
        self.gap(
            "compiler_types_calls_inheritance_runtime_and_generated_sources_not_extracted",
            "",
            None,
        );
        for file in &workspace.gradle_files {
            let evidence = self.source_evidence(&file.path, None)?;
            self.entity(
                "GradleBuildFile",
                &json!(file.path),
                &file.path,
                json!({"path":file.path,"evaluation":"not_evaluated"}),
                &evidence,
            );
        }
        for path in &workspace.java_files {
            self.gap("java_source_boundary", path, None);
        }
        for (index, source) in workspace.sources.iter().enumerate() {
            self.source(index, source)?;
        }
        let mut matched_expects = BTreeSet::new();
        for (source_index, source) in workspace.sources.iter().enumerate() {
            let Some(extraction) = &source.extraction else {
                continue;
            };
            for (index, declaration) in extraction.declarations.iter().enumerate() {
                if !declaration.is_actual {
                    continue;
                }
                let (actual_id, actual_evidence) =
                    self.declarations[&(source_index, index)].clone();
                if let Some(candidate) = workspace.expect_candidate(DeclarationLocation {
                    source: source_index,
                    declaration: index,
                }) {
                    let (expect_id, expect_evidence) =
                        self.declarations[&(candidate.source, candidate.declaration)].clone();
                    self.relationship(
                        "EXPECT_ACTUAL_CANDIDATE",
                        &expect_id,
                        &actual_id,
                        "Unknown",
                        &[&expect_evidence, &actual_evidence],
                    );
                    matched_expects.insert(expect_id);
                } else {
                    self.gap(
                        "actual_candidate_missing_ambiguous_or_unsupported",
                        &source.path,
                        Some(&actual_id),
                    );
                }
            }
        }
        for (source_index, source) in workspace.sources.iter().enumerate() {
            if let Some(extraction) = &source.extraction {
                for (index, declaration) in extraction.declarations.iter().enumerate() {
                    let id = self.declarations[&(source_index, index)].0.clone();
                    if declaration.is_expect && !matched_expects.contains(&id) {
                        self.gap(
                            "expect_without_unique_syntactic_candidate",
                            &source.path,
                            Some(&id),
                        );
                    }
                }
            }
        }
        Ok(())
    }
    fn source(&mut self, index: usize, source: &domain::KotlinSource) -> Result<(), KotlinError> {
        let evidence = self.source_evidence(&source.path, None)?;
        let file_id = self.entity("KotlinSourceFile",&json!(source.path),&source.path,json!({"path":source.path,"blob_oid":source.blob_oid,"byte_length":source.byte_length,"module":source.module,"source_set":source.source_set,"membership":"path_convention"}),&evidence);
        if let (Some(module), Some(set)) = (&source.module, &source.source_set) {
            let prefix = if module.is_empty() {
                String::new()
            } else {
                format!("{module}/")
            };
            let build_path = [
                format!("{prefix}build.gradle.kts"),
                format!("{prefix}build.gradle"),
            ]
            .into_iter()
            .find(|path| {
                self.inventory
                    .files()
                    .iter()
                    .any(|file| file.path() == path)
            })
            .ok_or(KotlinError::InvalidContract)?;
            let module_evidence = self.source_evidence(&build_path, None)?;
            let module_id = self.entity(
                "GradleModule",
                &json!(module),
                if module.is_empty() { ":" } else { module },
                json!({"path":module,"membership":"path_convention","active_project":"Unknown"}),
                &module_evidence,
            );
            let set_id = self.entity("KotlinSourceSet",&json!([module,set]),set,json!({"module":module,"membership":"path_convention","effective_visibility":"Unknown"}),&evidence);
            self.relationship(
                "CONTAINS_BY_PATH",
                &module_id,
                &set_id,
                "Observed",
                &[&evidence],
            );
            self.relationship(
                "CONTAINS_BY_PATH",
                &set_id,
                &file_id,
                "Observed",
                &[&evidence],
            );
        } else {
            self.gap("unassigned_source_root", &source.path, Some(&file_id));
        }
        let Some(extraction) = &source.extraction else {
            self.gap(
                "kotlin_syntax_not_accepted_by_pinned_parser",
                &source.path,
                Some(&file_id),
            );
            return Ok(());
        };
        let package_id = self.entity(
            "KotlinPackage",
            &json!([source.module, source.source_set, extraction.package]),
            &extraction.package,
            json!({"module":source.module,"source_set":source.source_set}),
            &evidence,
        );
        self.relationship(
            "DECLARES_PACKAGE",
            &file_id,
            &package_id,
            "Observed",
            &[&evidence],
        );
        for import in &extraction.imports {
            let item_evidence = self.source_evidence(&source.path, Some(import.span))?;
            let id = self.entity(
                "KotlinImport",
                &json!([source.path, import.span.start_byte]),
                &import.text,
                json!({"resolution":"Unknown","path":source.path}),
                &item_evidence,
            );
            self.relationship("DECLARES", &file_id, &id, "Observed", &[&item_evidence]);
        }
        for (declaration_index, declaration) in extraction.declarations.iter().enumerate() {
            let item_evidence = self.source_evidence(&source.path, Some(declaration.span))?;
            let id = self.entity(declaration.kind.as_str(),&json!([source.path,declaration.span.start_byte,declaration.span.end_byte]),&declaration.name,json!({"path":source.path,"module":source.module,"source_set":source.source_set,"package":extraction.package,"owner":declaration.owner,"signature":declaration.signature,"expect":declaration.is_expect,"actual":declaration.is_actual,"types":"unresolved"}),&item_evidence);
            self.relationship("DECLARES", &file_id, &id, "Observed", &[&item_evidence]);
            self.declarations
                .insert((index, declaration_index), (id, item_evidence));
        }
        Ok(())
    }
    fn finish(mut self) -> Result<Value, KotlinError> {
        if self.entities.len() > domain::MAX_ENTITIES {
            return Err(KotlinError::LimitExceeded("entities"));
        }
        self.coverage.sort_by_key(Value::to_string);
        let mut graph = json!({"schema_version":domain::GRAPH_VERSION,"ontology_version":domain::ONTOLOGY_VERSION,
            "entities":self.entities.into_values().collect::<Vec<_>>(),"relationships":self.relationships.into_values().collect::<Vec<_>>(),"claims":self.claims.into_values().collect::<Vec<_>>(),"evidence":self.evidence.into_values().collect::<Vec<_>>(),"coverage":self.coverage,"diagnostics":[]});
        graph["semantic_hash"] = hash(domain::GRAPH_HASH_DOMAIN, &graph);
        Ok(graph)
    }
}

fn hash(domain: &str, value: &Value) -> Value {
    json!({"algorithm":"blake3-256","domain":domain,"value":semantic_hash(domain.as_bytes(),value)})
}

/// Serializes a bounded, canonical JSON document with one trailing newline.
/// # Errors
/// Returns a capacity failure without a partial document.
pub fn canonical(value: &Value) -> Result<Vec<u8>, KotlinError> {
    let mut writer = LimitedVecWriter::new(domain::MAX_OUTPUT_BYTES - 1);
    serde_json::to_writer(&mut writer, value)
        .map_err(|_| KotlinError::LimitExceeded("output_bytes"))?;
    let mut bytes = writer.bytes;
    bytes.push(b'\n');
    Ok(bytes)
}

/// Verifies the stored identity and complete canonical graph before projection.
/// # Errors
/// Returns invalid contract on hash, identity, reference or shape mismatch.
pub fn validate_stored(semantic: &Value, head: &LocalSnapshotHead) -> Result<(), KotlinError> {
    if head.snapshot_schema_version != domain::SNAPSHOT_VERSION
        || semantic["repository"]["identity"] != head.repository_identity.as_str()
        || semantic["repository"]["commit_oid"] != head.commit_oid.as_str()
        || semantic_hash(domain::SNAPSHOT_HASH_DOMAIN.as_bytes(), semantic)
            != head.semantic_hash.value
        || semantic["knowledge_graph"]["semantic_hash"]["value"] != head.graph_semantic_hash.value
    {
        return Err(KotlinError::InvalidContract);
    }
    validate_semantic(semantic)
}

fn validate_semantic(semantic: &Value) -> Result<(), KotlinError> {
    if semantic["profile"] != domain::PROFILE
        || semantic["ontology_version"] != domain::ONTOLOGY_VERSION
    {
        return Err(KotlinError::InvalidContract);
    }
    let graph = &semantic["knowledge_graph"];
    let mut unhashed = graph.clone();
    let supplied = unhashed
        .as_object_mut()
        .ok_or(KotlinError::InvalidContract)?
        .remove("semantic_hash")
        .ok_or(KotlinError::InvalidContract)?;
    if supplied != hash(domain::GRAPH_HASH_DOMAIN, &unhashed)
        || graph["schema_version"] != domain::GRAPH_VERSION
    {
        return Err(KotlinError::InvalidContract);
    }
    let entities = ids(graph, "entities")?;
    ids(graph, "relationships")?;
    let evidence = ids(graph, "evidence")?;
    ids(graph, "claims")?;
    validate_entity_rows(graph)?;
    let mut states: BTreeMap<(&str, &str), &str> = entities
        .iter()
        .map(|id| (("entity", *id), "Observed"))
        .collect();
    for edge in graph["relationships"]
        .as_array()
        .ok_or(KotlinError::InvalidContract)?
    {
        let state = match edge["kind"].as_str() {
            Some("EXPECT_ACTUAL_CANDIDATE") => "Unknown",
            Some("CONTAINS_BY_PATH" | "DECLARES" | "DECLARES_PACKAGE") => "Observed",
            _ => return Err(KotlinError::InvalidContract),
        };
        if edge["state"] != state {
            return Err(KotlinError::InvalidContract);
        }
        states.insert(
            (
                "relationship",
                edge["id"].as_str().ok_or(KotlinError::InvalidContract)?,
            ),
            state,
        );
        if !entities.contains(
            edge["source"]
                .as_str()
                .ok_or(KotlinError::InvalidContract)?,
        ) || !entities.contains(
            edge["target"]
                .as_str()
                .ok_or(KotlinError::InvalidContract)?,
        ) {
            return Err(KotlinError::InvalidContract);
        }
    }
    for claim in graph["claims"]
        .as_array()
        .ok_or(KotlinError::InvalidContract)?
    {
        let subject_kind = claim["subject_kind"]
            .as_str()
            .ok_or(KotlinError::InvalidContract)?;
        let subject_id = claim["subject_id"]
            .as_str()
            .ok_or(KotlinError::InvalidContract)?;
        let state = states
            .remove(&(subject_kind, subject_id))
            .ok_or(KotlinError::InvalidContract)?;
        let claim_state = if state == "Unknown" {
            "candidate"
        } else {
            "deterministic_fact"
        };
        if claim["state"] != claim_state || claim["rule"] != domain::PROFILE {
            return Err(KotlinError::InvalidContract);
        }
        let refs = claim["evidence_ids"]
            .as_array()
            .ok_or(KotlinError::InvalidContract)?;
        if refs.is_empty()
            || refs
                .iter()
                .any(|v| v.as_str().is_none_or(|id| !evidence.contains(id)))
        {
            return Err(KotlinError::InvalidContract);
        }
    }
    if !states.is_empty() {
        return Err(KotlinError::InvalidContract);
    }
    validate_evidence_rows(semantic)?;
    for family in ["coverage", "diagnostics"] {
        graph[family]
            .as_array()
            .ok_or(KotlinError::InvalidContract)?;
    }
    Ok(())
}

fn validate_entity_rows(graph: &Value) -> Result<(), KotlinError> {
    let rows = graph["entities"]
        .as_array()
        .ok_or(KotlinError::InvalidContract)?;
    if rows.len() > domain::MAX_ENTITIES {
        return Err(KotlinError::LimitExceeded("entities"));
    }
    for row in rows {
        if !matches!(
            row["kind"].as_str(),
            Some(
                "GradleBuildFile"
                    | "GradleModule"
                    | "KotlinSourceSet"
                    | "KotlinSourceFile"
                    | "KotlinPackage"
                    | "KotlinImport"
                    | "KotlinClass"
                    | "KotlinInterface"
                    | "KotlinObject"
                    | "KotlinFunction"
                    | "KotlinProperty"
                    | "KotlinTypeAlias"
            )
        ) || row["name"].as_str().is_none()
            || row["properties"].as_object().is_none()
        {
            return Err(KotlinError::InvalidContract);
        }
    }
    Ok(())
}

fn validate_evidence_rows(semantic: &Value) -> Result<(), KotlinError> {
    let repository = &semantic["repository"];
    codenoesis_domain::RepositoryIdentity::parse(
        repository["identity"]
            .as_str()
            .ok_or(KotlinError::InvalidContract)?,
    )
    .map_err(|_| KotlinError::InvalidContract)?;
    for field in ["commit_oid", "tree_oid"] {
        codenoesis_domain::ObjectId::parse_sha1(
            repository[field]
                .as_str()
                .ok_or(KotlinError::InvalidContract)?,
        )
        .ok_or(KotlinError::InvalidContract)?;
    }
    for row in semantic["knowledge_graph"]["evidence"]
        .as_array()
        .ok_or(KotlinError::InvalidContract)?
    {
        let start = row["start_byte"]
            .as_u64()
            .ok_or(KotlinError::InvalidContract)?;
        let end = row["end_byte"]
            .as_u64()
            .ok_or(KotlinError::InvalidContract)?;
        let start_line = row["start_line"]
            .as_u64()
            .ok_or(KotlinError::InvalidContract)?;
        let end_line = row["end_line"]
            .as_u64()
            .ok_or(KotlinError::InvalidContract)?;
        let path = row["path"].as_str().ok_or(KotlinError::InvalidContract)?;
        let digest = row["excerpt_blake3"]
            .as_str()
            .ok_or(KotlinError::InvalidContract)?;
        if row["repository_identity"] != repository["identity"]
            || row["commit_oid"] != repository["commit_oid"]
            || start > end
            || end > domain::MAX_SOURCE_BYTES as u64
            || start_line == 0
            || start_line > end_line
            || path.is_empty()
            || path.starts_with('/')
            || path.contains('\\')
            || path
                .split('/')
                .any(|c| c.is_empty() || c == "." || c == "..")
            || digest.len() != 64
            || !digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(KotlinError::InvalidContract);
        }
        codenoesis_domain::ObjectId::parse_sha1(
            row["blob_oid"]
                .as_str()
                .ok_or(KotlinError::InvalidContract)?,
        )
        .ok_or(KotlinError::InvalidContract)?;
    }
    Ok(())
}
fn ids<'a>(graph: &'a Value, family: &str) -> Result<BTreeSet<&'a str>, KotlinError> {
    let rows = graph[family]
        .as_array()
        .ok_or(KotlinError::InvalidContract)?;
    if rows.len() > domain::MAX_ENTITIES * 4 {
        return Err(KotlinError::LimitExceeded("graph_rows"));
    }
    let mut ids = BTreeSet::new();
    let mut previous = None;
    for row in rows {
        let id = row["id"].as_str().ok_or(KotlinError::InvalidContract)?;
        if previous.is_some_and(|p| p >= id) || !ids.insert(id) {
            return Err(KotlinError::InvalidContract);
        }
        previous = Some(id);
    }
    Ok(ids)
}

/// Builds an offline package from a verified local snapshot.
/// # Errors
/// Returns a contract or output capacity failure.
pub fn portable(semantic: &Value, head: &LocalSnapshotHead) -> Result<Value, KotlinError> {
    validate_stored(semantic, head)?;
    let payload = json!({"snapshot_id":head.snapshot_id.as_str(),"snapshot_semantic_hash":head.semantic_hash.value,"semantic":semantic});
    let value = json!({"schema_version":PORTABLE_VERSION,"integrity":hash("codenoesis.kotlin-portable.semantic.v1",&payload),"payload":payload});
    canonical(&value)?;
    Ok(value)
}

/// Accepts only bounded canonical packages with intact lineage and references.
/// # Errors
/// Returns invalid contract for malformed or modified packages.
pub fn parse_portable(bytes: &[u8]) -> Result<Value, KotlinError> {
    if bytes.len() > domain::MAX_OUTPUT_BYTES {
        return Err(KotlinError::LimitExceeded("output_bytes"));
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|_| KotlinError::InvalidContract)?;
    if canonical(&value)? != bytes
        || value["schema_version"] != PORTABLE_VERSION
        || value["integrity"] != hash("codenoesis.kotlin-portable.semantic.v1", &value["payload"])
    {
        return Err(KotlinError::InvalidContract);
    }
    let payload = &value["payload"];
    let digest = semantic_hash(
        domain::SNAPSHOT_HASH_DOMAIN.as_bytes(),
        &payload["semantic"],
    );
    let snapshot_id = codenoesis_domain::storage::SnapshotId::from_semantic_hash(&digest)
        .map_err(|_| KotlinError::InvalidContract)?;
    if payload["snapshot_semantic_hash"] != digest || payload["snapshot_id"] != snapshot_id.as_str()
    {
        return Err(KotlinError::InvalidContract);
    }
    validate_semantic(&payload["semantic"])?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use codenoesis_domain::{
        AcquiredFile, AcquiredRepository, BoundRevision, ObjectId, RegularFileMode,
        RepositoryIdentity,
    };
    use domain::{Declaration, DeclarationKind, GradleFile, KotlinSource, SourceExtraction};

    fn fixture() -> KotlinSnapshot {
        let oid = ObjectId::parse_sha1("1111111111111111111111111111111111111111").unwrap();
        let identity =
            RepositoryIdentity::parse("urn:codenoesis:repository:kotlin-contract").unwrap();
        let bound = BoundRevision::new(identity, oid.clone(), oid.clone());
        let common = "expect fun name(): String\n";
        let actual = "actual fun name(): String = \"JVM\"\n";
        let files = [
            ("shared/build.gradle.kts", ""),
            ("shared/src/commonMain/kotlin/Common.kt", common),
            ("shared/src/jvmMain/kotlin/Jvm.kt", actual),
        ];
        let acquired = files
            .iter()
            .map(|(path, source)| {
                AcquiredFile::new(
                    (*path).to_owned(),
                    RegularFileMode::Regular,
                    oid.clone(),
                    source.as_bytes().to_vec(),
                )
            })
            .collect();
        let inventory = RepositoryInventory::classify(AcquiredRepository::new(bound, 7, acquired));
        let sources = files[1..]
            .iter()
            .enumerate()
            .map(|(index, (path, source))| KotlinSource {
                path: (*path).to_owned(),
                blob_oid: oid.to_string(),
                byte_length: source.len(),
                module: Some("shared".to_owned()),
                source_set: Some(if index == 0 { "commonMain" } else { "jvmMain" }.to_owned()),
                extraction: Some(SourceExtraction {
                    package: String::new(),
                    imports: vec![],
                    declarations: vec![Declaration {
                        name: "name".to_owned(),
                        owner: String::new(),
                        kind: DeclarationKind::Function,
                        signature: "fun name (): String".to_owned(),
                        is_expect: index == 0,
                        is_actual: index == 1,
                        span: SourceSpan {
                            start_byte: 0,
                            end_byte: source.len() - 1,
                            start_line: 1,
                            end_line: 1,
                        },
                    }],
                }),
            })
            .collect();
        let workspace = KotlinWorkspace {
            sources,
            gradle_files: vec![GradleFile {
                path: files[0].0.to_owned(),
                blob_oid: oid.to_string(),
                byte_length: 0,
            }],
            java_files: vec![],
        };
        KotlinSnapshot::from_workspace(
            &inventory,
            &workspace,
            &SnapshotEnvelopeV1::new("2026-09-07T00:00:00Z".to_owned(), None, "test".to_owned()),
        )
        .unwrap()
    }

    #[test]
    fn ct_fr_ext_025_candidate_cannot_be_promoted_even_with_recomputed_hash() {
        let snapshot = fixture();
        let mut semantic = snapshot.value()["semantic"].clone();
        let graph = &mut semantic["knowledge_graph"];
        let relationship = graph["relationships"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|r| r["kind"] == "EXPECT_ACTUAL_CANDIDATE")
            .unwrap();
        relationship["state"] = json!("Observed");
        graph.as_object_mut().unwrap().remove("semantic_hash");
        graph["semantic_hash"] = hash(domain::GRAPH_HASH_DOMAIN, graph);
        assert_eq!(
            validate_semantic(&semantic),
            Err(KotlinError::InvalidContract)
        );
    }

    #[test]
    fn ct_fr_ext_025_claims_preserve_common_state_and_evidence_invariants() {
        let snapshot = fixture();
        let semantic = &snapshot.value()["semantic"];
        for claim in semantic["knowledge_graph"]["claims"].as_array().unwrap() {
            assert!(matches!(
                claim["state"].as_str(),
                Some("candidate" | "deterministic_fact")
            ));
        }
        let mut malformed = semantic.clone();
        let graph = &mut malformed["knowledge_graph"];
        graph["claims"][0]["evidence_ids"] = json!(["missing"]);
        graph.as_object_mut().unwrap().remove("semantic_hash");
        graph["semantic_hash"] = hash(domain::GRAPH_HASH_DOMAIN, graph);
        assert_eq!(
            validate_semantic(&malformed),
            Err(KotlinError::InvalidContract)
        );
    }

    #[test]
    fn ct_fr_ext_025_invalid_utf8_json_and_deep_json_are_typed_failures() {
        assert_eq!(parse_portable(&[0xff]), Err(KotlinError::InvalidContract));
        let deep = format!("{}0{}", "[".repeat(200), "]".repeat(200));
        assert_eq!(
            parse_portable(deep.as_bytes()),
            Err(KotlinError::InvalidContract)
        );
    }
}
