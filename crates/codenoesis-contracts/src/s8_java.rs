//! Canonical Java snapshot and portable projection contracts.
use crate::{LimitedVecWriter, SnapshotEnvelopeV1, publication_candidate, semantic_hash};
use codenoesis_domain::RepositoryInventory;
use codenoesis_domain::s7::SourceSpan;
use codenoesis_domain::s8_java::{self as domain, JavaError, JavaWorkspace};
use codenoesis_domain::storage::{LocalSnapshotHead, PublicationCandidate};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

const GLOBAL_GAPS: [&str; 3] = [
    "build_configuration_and_source_visibility_not_evaluated",
    "compiler_resolution_runtime_and_generated_members_not_extracted",
    "local_and_anonymous_declarations_not_extracted",
];

pub const PORTABLE_VERSION: &str = "codenoesis.java-portable-graph/v1";

#[derive(Clone, Debug)]
pub struct JavaSnapshot {
    value: Value,
}

impl JavaSnapshot {
    /// Builds one versioned, evidence-backed projection of committed syntax.
    /// # Errors
    /// Returns a capacity or invalid extraction contract failure.
    pub fn from_workspace(
        inventory: &RepositoryInventory,
        workspace: &JavaWorkspace,
        envelope: &SnapshotEnvelopeV1,
    ) -> Result<Self, JavaError> {
        let mut builder = Builder::new(inventory);
        builder.populate(workspace)?;
        let graph = builder.finish()?;
        let bound = inventory.bound_revision();
        let semantic = json!({
            "repository": {"identity":bound.repository_identity().as_str(), "commit_oid":bound.commit_oid().as_str(), "tree_oid":bound.tree_oid().as_str()},
            "profile":domain::PROFILE, "ontology_version":domain::ONTOLOGY_VERSION,
            "knowledge_graph":graph, "extraction_chunks":[],
            "configuration":{"maven":"not_evaluated","gradle":"not_evaluated","parser":"tree-sitter-java/0.23.5","membership":"path_convention"}
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
    pub fn canonical_stdout(&self) -> Result<Vec<u8>, JavaError> {
        canonical(&self.value)
    }
    /// # Errors
    /// Returns an invalid publication contract failure.
    pub fn publication_candidate(&self) -> Result<PublicationCandidate, JavaError> {
        publication_candidate(&self.value).map_err(|_| JavaError::InvalidContract)
    }
}

struct ClaimProjection {
    row: Value,
    evidence: BTreeSet<String>,
}

struct Builder<'a> {
    inventory: &'a RepositoryInventory,
    files: BTreeMap<&'a str, &'a codenoesis_domain::InventoryFile>,
    entities: BTreeMap<String, Value>,
    relationships: BTreeMap<String, Value>,
    evidence: BTreeMap<String, Value>,
    claims: BTreeMap<String, ClaimProjection>,
    coverage: Vec<Value>,
}
impl<'a> Builder<'a> {
    fn new(inventory: &'a RepositoryInventory) -> Self {
        Self {
            inventory,
            files: inventory
                .files()
                .iter()
                .map(|file| (file.path(), file))
                .collect(),
            entities: BTreeMap::new(),
            relationships: BTreeMap::new(),
            evidence: BTreeMap::new(),
            claims: BTreeMap::new(),
            coverage: Vec::new(),
        }
    }
    fn id(&self, kind: &str, key: &Value) -> String {
        let bound = self.inventory.bound_revision();
        format!(
            "urn:codenoesis:java:{kind}:{}",
            semantic_hash(
                b"codenoesis.java-id/v1",
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
        self.claims.entry(id.clone()).or_insert_with(||ClaimProjection {
            row:json!({"id":id,"subject_kind":subject_kind,"subject_id":subject,"state":state,"rule":domain::PROFILE}),evidence:BTreeSet::new()
        }).evidence.extend(evidence.iter().map(|e|(*e).to_owned()));
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
    ) -> Result<String, JavaError> {
        let file = self.files.get(path).ok_or(JavaError::InvalidContract)?;
        let bytes = file.bytes();
        let (start, end) = span.map_or((0, bytes.len()), |s| (s.start_byte, s.end_byte));
        let id = self.id("evidence", &json!([path, start, end]));
        if self.evidence.contains_key(&id) {
            return Ok(id);
        }
        let (start_line, end_line) = span.map_or_else(
            || (1, bytes.split(|b| *b == b'\n').count() as u64),
            |s| (s.start_line, s.end_line),
        );
        let excerpt = bytes.get(start..end).ok_or(JavaError::InvalidContract)?;
        let bound = self.inventory.bound_revision();
        self.evidence.insert(id.clone(),json!({"id":id,"repository_identity":bound.repository_identity().as_str(),"commit_oid":bound.commit_oid().as_str(),"path":path,"blob_oid":file.blob_oid().as_str(),"start_byte":start,"end_byte":end,"start_line":start_line,"end_line":end_line,"excerpt_blake3":blake3::hash(excerpt).to_hex().to_string()}));
        Ok(id)
    }
    fn gap(&mut self, reason: &str, path: &str, subject: Option<&str>) {
        self.coverage
            .push(json!({"reason":reason,"path":path,"subject_id":subject,"state":"Unknown"}));
    }
    fn populate(&mut self, workspace: &JavaWorkspace) -> Result<(), JavaError> {
        for reason in GLOBAL_GAPS {
            self.gap(reason, "", None);
        }
        for file in &workspace.build_files {
            let evidence = self.source_evidence(&file.path, None)?;
            self.entity(
                "JavaBuildFile",
                &json!(file.path),
                &file.path,
                json!({"path":file.path,"build_system":file.system,"evaluation":"not_evaluated"}),
                &evidence,
            );
        }
        for path in &workspace.kotlin_files {
            self.gap("kotlin_source_boundary", path, None);
        }
        for source in &workspace.sources {
            self.source(source)?;
        }
        Ok(())
    }
    fn membership(
        &mut self,
        source: &domain::JavaSource,
        file_id: &str,
        evidence: &str,
    ) -> Result<(), JavaError> {
        let (Some(module), Some(set)) = (&source.module, &source.source_set) else {
            self.gap("unassigned_source_root", &source.path, Some(file_id));
            return Ok(());
        };
        let prefix = if module.is_empty() {
            String::new()
        } else {
            format!("{module}/")
        };
        let build_paths: Vec<_> = ["pom.xml", "build.gradle", "build.gradle.kts"]
            .iter()
            .map(|name| format!("{prefix}{name}"))
            .filter(|path| self.files.contains_key(path.as_str()))
            .collect();
        if build_paths.is_empty() {
            return Err(JavaError::InvalidContract);
        }
        for build_path in build_paths {
            let module_evidence = self.source_evidence(&build_path, None)?;
            let module_id = self.entity(
                "JavaBuildModule",
                &json!(module),
                if module.is_empty() { ":" } else { module },
                json!({"path":module,"membership":"path_convention","active_project":"Unknown"}),
                &module_evidence,
            );
            let set_id = self.entity("JavaSourceSet", &json!([module,set]), set,
                json!({"module":module,"membership":"path_convention","effective_visibility":"Unknown"}), evidence);
            self.relationship(
                "CONTAINS_BY_PATH",
                &module_id,
                &set_id,
                "Observed",
                &[&module_evidence, evidence],
            );
            self.relationship(
                "CONTAINS_BY_PATH",
                &set_id,
                file_id,
                "Observed",
                &[evidence],
            );
        }
        Ok(())
    }
    fn source(&mut self, source: &domain::JavaSource) -> Result<(), JavaError> {
        let evidence = self.source_evidence(&source.path, None)?;
        let file_id = self.entity("JavaSourceFile", &json!(source.path), &source.path,
            json!({"path":source.path,"blob_oid":source.blob_oid,"byte_length":source.byte_length,"module":source.module,"source_set":source.source_set,"membership":"path_convention"}), &evidence);
        self.membership(source, &file_id, &evidence)?;
        let extraction = match &source.extraction {
            Ok(extraction) => extraction,
            Err(reason) => {
                self.gap(reason.as_str(), &source.path, Some(&file_id));
                return Ok(());
            }
        };
        let package_name = extraction.package.as_ref().map_or("", |p| p.name.as_str());
        let package_evidence =
            self.source_evidence(&source.path, extraction.package.as_ref().map(|p| p.span))?;
        let package_id = self.entity(
            "JavaPackage",
            &json!([source.module, source.source_set, package_name]),
            package_name,
            json!({"module":source.module,"source_set":source.source_set}),
            &package_evidence,
        );
        self.relationship(
            "DECLARES_PACKAGE",
            &file_id,
            &package_id,
            "Observed",
            &[&package_evidence],
        );
        for import in &extraction.imports {
            let item_evidence = self.source_evidence(&source.path, Some(import.span))?;
            let id = self.entity("JavaImport", &json!([source.path,import.span.start_byte]), &import.text,
                json!({"resolution":"Unknown","path":source.path,"static":import.is_static,"wildcard":import.is_wildcard}), &item_evidence);
            self.relationship("DECLARES", &file_id, &id, "Observed", &[&item_evidence]);
        }
        self.declarations(source, extraction, &file_id, package_name)
    }
    fn declarations(
        &mut self,
        source: &domain::JavaSource,
        extraction: &domain::SourceExtraction,
        file_id: &str,
        package_name: &str,
    ) -> Result<(), JavaError> {
        let mut projected: Vec<(String, String)> = Vec::new();
        for (index, declaration) in extraction.declarations.iter().enumerate() {
            let item_evidence = self.source_evidence(&source.path, Some(declaration.span))?;
            // Field declarators share their enclosing statement span; the occurrence ordinal
            // and name preserve distinct facts even when source evidence is shared.
            let id = self.entity(declaration.kind.as_str(),
                &json!([source.path,declaration.span.start_byte,declaration.span.end_byte,index,declaration.name]), &declaration.name,
                json!({"path":source.path,"module":source.module,"source_set":source.source_set,"package":package_name,"owner":declaration.owner,"signature":declaration.signature,"types":"unresolved"}), &item_evidence);
            self.relationship("DECLARES", file_id, &id, "Observed", &[&item_evidence]);
            if let Some(parent) = declaration.parent {
                let parent_decl = extraction
                    .declarations
                    .get(parent)
                    .ok_or(JavaError::InvalidContract)?;
                if parent >= index
                    || !parent_decl.kind.is_type()
                    || parent_decl.span.start_byte > declaration.span.start_byte
                    || parent_decl.span.end_byte < declaration.span.end_byte
                {
                    return Err(JavaError::InvalidContract);
                }
                let (parent_id, parent_evidence) = &projected[parent];
                self.relationship(
                    "CONTAINS_DECLARATION",
                    parent_id,
                    &id,
                    "Observed",
                    &[parent_evidence, &item_evidence],
                );
            }
            projected.push((id, item_evidence));
        }
        Ok(())
    }
    fn finish(mut self) -> Result<Value, JavaError> {
        if self.entities.len() > domain::MAX_ENTITIES {
            return Err(JavaError::LimitExceeded("entities"));
        }
        self.coverage.sort_by_key(Value::to_string);
        let mut graph = json!({"schema_version":domain::GRAPH_VERSION,"ontology_version":domain::ONTOLOGY_VERSION,
            "entities":self.entities.into_values().collect::<Vec<_>>(),"relationships":self.relationships.into_values().collect::<Vec<_>>(),"claims":self.claims.into_values().map(|mut claim| {claim.row["evidence_ids"]=json!(claim.evidence);claim.row}).collect::<Vec<_>>(),"evidence":self.evidence.into_values().collect::<Vec<_>>(),"coverage":self.coverage,"diagnostics":[]});
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
pub fn canonical(value: &Value) -> Result<Vec<u8>, JavaError> {
    let mut writer = LimitedVecWriter::new(domain::MAX_OUTPUT_BYTES - 1);
    serde_json::to_writer(&mut writer, value)
        .map_err(|_| JavaError::LimitExceeded("output_bytes"))?;
    let mut bytes = writer.bytes;
    bytes.push(b'\n');
    Ok(bytes)
}

/// Verifies the stored identity and complete canonical graph before projection.
/// # Errors
/// Returns invalid contract on hash, identity, reference or shape mismatch.
pub fn validate_stored(semantic: &Value, head: &LocalSnapshotHead) -> Result<(), JavaError> {
    if head.snapshot_schema_version != domain::SNAPSHOT_VERSION
        || semantic["repository"]["identity"] != head.repository_identity.as_str()
        || semantic["repository"]["commit_oid"] != head.commit_oid.as_str()
        || semantic_hash(domain::SNAPSHOT_HASH_DOMAIN.as_bytes(), semantic)
            != head.semantic_hash.value
        || semantic["knowledge_graph"]["semantic_hash"]["value"] != head.graph_semantic_hash.value
    {
        return Err(JavaError::InvalidContract);
    }
    validate_semantic(semantic)
}

fn validate_semantic(semantic: &Value) -> Result<(), JavaError> {
    if semantic["profile"] != domain::PROFILE
        || semantic["ontology_version"] != domain::ONTOLOGY_VERSION
    {
        return Err(JavaError::InvalidContract);
    }
    if semantic["configuration"]
        != json!({"maven":"not_evaluated","gradle":"not_evaluated","parser":"tree-sitter-java/0.23.5","membership":"path_convention"})
        || semantic["extraction_chunks"] != json!([])
    {
        return Err(JavaError::InvalidContract);
    }
    let graph = &semantic["knowledge_graph"];
    if graph["ontology_version"] != domain::ONTOLOGY_VERSION {
        return Err(JavaError::InvalidContract);
    }
    let mut unhashed = graph.clone();
    let supplied = unhashed
        .as_object_mut()
        .ok_or(JavaError::InvalidContract)?
        .remove("semantic_hash")
        .ok_or(JavaError::InvalidContract)?;
    if supplied != hash(domain::GRAPH_HASH_DOMAIN, &unhashed)
        || graph["schema_version"] != domain::GRAPH_VERSION
    {
        return Err(JavaError::InvalidContract);
    }
    validate_graph_rows(semantic)
}

fn validate_graph_rows(semantic: &Value) -> Result<(), JavaError> {
    let graph = &semantic["knowledge_graph"];
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
        .ok_or(JavaError::InvalidContract)?
    {
        let state = match edge["kind"].as_str() {
            Some("CONTAINS_BY_PATH" | "DECLARES" | "DECLARES_PACKAGE" | "CONTAINS_DECLARATION") => {
                "Observed"
            }
            _ => return Err(JavaError::InvalidContract),
        };
        if edge["state"] != state {
            return Err(JavaError::InvalidContract);
        }
        states.insert(
            (
                "relationship",
                edge["id"].as_str().ok_or(JavaError::InvalidContract)?,
            ),
            state,
        );
        if !entities.contains(edge["source"].as_str().ok_or(JavaError::InvalidContract)?)
            || !entities.contains(edge["target"].as_str().ok_or(JavaError::InvalidContract)?)
        {
            return Err(JavaError::InvalidContract);
        }
    }
    for claim in graph["claims"]
        .as_array()
        .ok_or(JavaError::InvalidContract)?
    {
        let subject_kind = claim["subject_kind"]
            .as_str()
            .ok_or(JavaError::InvalidContract)?;
        let subject_id = claim["subject_id"]
            .as_str()
            .ok_or(JavaError::InvalidContract)?;
        let state = states
            .remove(&(subject_kind, subject_id))
            .ok_or(JavaError::InvalidContract)?;
        let claim_state = if state == "Unknown" {
            "candidate"
        } else {
            "deterministic_fact"
        };
        if claim["state"] != claim_state || claim["rule"] != domain::PROFILE {
            return Err(JavaError::InvalidContract);
        }
        let refs = claim["evidence_ids"]
            .as_array()
            .ok_or(JavaError::InvalidContract)?;
        if refs.is_empty()
            || refs
                .windows(2)
                .any(|pair| pair[0].as_str() >= pair[1].as_str())
            || refs
                .iter()
                .any(|v| v.as_str().is_none_or(|id| !evidence.contains(id)))
        {
            return Err(JavaError::InvalidContract);
        }
    }
    if !states.is_empty() {
        return Err(JavaError::InvalidContract);
    }
    validate_evidence_rows(semantic)?;
    validate_coverage(graph, &entities)?;
    for family in ["coverage", "diagnostics"] {
        graph[family].as_array().ok_or(JavaError::InvalidContract)?;
    }
    Ok(())
}

fn validate_coverage(graph: &Value, entities: &BTreeSet<&str>) -> Result<(), JavaError> {
    let rows = graph["coverage"]
        .as_array()
        .ok_or(JavaError::InvalidContract)?;
    for required in GLOBAL_GAPS {
        if rows
            .iter()
            .filter(|r| r["reason"] == required && r["path"] == "" && r["subject_id"].is_null())
            .count()
            != 1
        {
            return Err(JavaError::InvalidContract);
        }
    }
    for row in rows {
        let reason = row["reason"].as_str().ok_or(JavaError::InvalidContract)?;
        if row["state"] != "Unknown"
            || row["path"].as_str().is_none()
            || (!row["subject_id"].is_null()
                && row["subject_id"]
                    .as_str()
                    .is_none_or(|id| !entities.contains(id)))
            || !(GLOBAL_GAPS.contains(&reason)
                || matches!(
                    reason,
                    "kotlin_source_boundary"
                        | "unassigned_source_root"
                        | "java_syntax_not_accepted_by_pinned_parser"
                        | "java_unicode_escape_preprocessing_not_supported"
                        | "java_module_descriptor_boundary"
                        | "java_compilation_unit_form_not_supported"
                ))
        {
            return Err(JavaError::InvalidContract);
        }
    }
    if graph["diagnostics"] != json!([]) {
        return Err(JavaError::InvalidContract);
    }
    Ok(())
}

fn validate_entity_rows(graph: &Value) -> Result<(), JavaError> {
    let rows = graph["entities"]
        .as_array()
        .ok_or(JavaError::InvalidContract)?;
    if rows.len() > domain::MAX_ENTITIES {
        return Err(JavaError::LimitExceeded("entities"));
    }
    for row in rows {
        if !matches!(
            row["kind"].as_str(),
            Some(
                "JavaBuildFile"
                    | "JavaBuildModule"
                    | "JavaSourceSet"
                    | "JavaSourceFile"
                    | "JavaPackage"
                    | "JavaImport"
                    | "JavaClass"
                    | "JavaInterface"
                    | "JavaEnum"
                    | "JavaRecord"
                    | "JavaAnnotationType"
                    | "JavaMethod"
                    | "JavaConstructor"
                    | "JavaField"
                    | "JavaEnumConstant"
                    | "JavaRecordComponent"
                    | "JavaAnnotationElement"
            )
        ) || row["name"].as_str().is_none()
            || row["properties"].as_object().is_none()
        {
            return Err(JavaError::InvalidContract);
        }
    }
    Ok(())
}

fn validate_evidence_rows(semantic: &Value) -> Result<(), JavaError> {
    let repository = &semantic["repository"];
    codenoesis_domain::RepositoryIdentity::parse(
        repository["identity"]
            .as_str()
            .ok_or(JavaError::InvalidContract)?,
    )
    .map_err(|_| JavaError::InvalidContract)?;
    for field in ["commit_oid", "tree_oid"] {
        codenoesis_domain::ObjectId::parse_sha1(
            repository[field]
                .as_str()
                .ok_or(JavaError::InvalidContract)?,
        )
        .ok_or(JavaError::InvalidContract)?;
    }
    for row in semantic["knowledge_graph"]["evidence"]
        .as_array()
        .ok_or(JavaError::InvalidContract)?
    {
        let start = row["start_byte"]
            .as_u64()
            .ok_or(JavaError::InvalidContract)?;
        let end = row["end_byte"].as_u64().ok_or(JavaError::InvalidContract)?;
        let start_line = row["start_line"]
            .as_u64()
            .ok_or(JavaError::InvalidContract)?;
        let end_line = row["end_line"].as_u64().ok_or(JavaError::InvalidContract)?;
        let path = row["path"].as_str().ok_or(JavaError::InvalidContract)?;
        let digest = row["excerpt_blake3"]
            .as_str()
            .ok_or(JavaError::InvalidContract)?;
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
            return Err(JavaError::InvalidContract);
        }
        codenoesis_domain::ObjectId::parse_sha1(
            row["blob_oid"].as_str().ok_or(JavaError::InvalidContract)?,
        )
        .ok_or(JavaError::InvalidContract)?;
    }
    Ok(())
}
fn ids<'a>(graph: &'a Value, family: &str) -> Result<BTreeSet<&'a str>, JavaError> {
    let rows = graph[family].as_array().ok_or(JavaError::InvalidContract)?;
    if rows.len() > domain::MAX_ENTITIES * 4 {
        return Err(JavaError::LimitExceeded("graph_rows"));
    }
    let mut ids = BTreeSet::new();
    let mut previous = None;
    for row in rows {
        let id = row["id"].as_str().ok_or(JavaError::InvalidContract)?;
        if previous.is_some_and(|p| p >= id) || !ids.insert(id) {
            return Err(JavaError::InvalidContract);
        }
        previous = Some(id);
    }
    Ok(ids)
}

/// Builds an offline package from a verified local snapshot.
/// # Errors
/// Returns a contract or output capacity failure.
pub fn portable(semantic: &Value, head: &LocalSnapshotHead) -> Result<Value, JavaError> {
    validate_stored(semantic, head)?;
    let payload = json!({"snapshot_id":head.snapshot_id.as_str(),"snapshot_semantic_hash":head.semantic_hash.value,"semantic":semantic});
    let value = json!({"schema_version":PORTABLE_VERSION,"integrity":hash("codenoesis.java-portable.semantic.v1",&payload),"payload":payload});
    canonical(&value)?;
    Ok(value)
}

/// Accepts only bounded canonical packages with intact lineage and references.
/// # Errors
/// Returns invalid contract for malformed or modified packages.
pub fn parse_portable(bytes: &[u8]) -> Result<Value, JavaError> {
    if bytes.len() > domain::MAX_OUTPUT_BYTES {
        return Err(JavaError::LimitExceeded("output_bytes"));
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|_| JavaError::InvalidContract)?;
    if canonical(&value)? != bytes
        || value["schema_version"] != PORTABLE_VERSION
        || value["integrity"] != hash("codenoesis.java-portable.semantic.v1", &value["payload"])
    {
        return Err(JavaError::InvalidContract);
    }
    let payload = &value["payload"];
    let digest = semantic_hash(
        domain::SNAPSHOT_HASH_DOMAIN.as_bytes(),
        &payload["semantic"],
    );
    let snapshot_id = codenoesis_domain::storage::SnapshotId::from_semantic_hash(&digest)
        .map_err(|_| JavaError::InvalidContract)?;
    if payload["snapshot_semantic_hash"] != digest || payload["snapshot_id"] != snapshot_id.as_str()
    {
        return Err(JavaError::InvalidContract);
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
    use domain::{Declaration, DeclarationKind, JavaSource, SourceExtraction};

    fn fixture() -> Value {
        let oid = ObjectId::parse_sha1("1111111111111111111111111111111111111111").unwrap();
        let bound = BoundRevision::new(
            RepositoryIdentity::parse("urn:codenoesis:repository:java-contract").unwrap(),
            oid.clone(),
            oid.clone(),
        );
        let text = "class A { int x, y; }";
        let paths = ["A.java", "B.java"];
        let acquired = paths
            .iter()
            .map(|path| {
                AcquiredFile::new(
                    (*path).to_owned(),
                    RegularFileMode::Regular,
                    oid.clone(),
                    text.as_bytes().to_vec(),
                )
            })
            .collect();
        let inventory = RepositoryInventory::classify(AcquiredRepository::new(bound, 7, acquired));
        let declarations = vec![
            Declaration {
                name: "A".into(),
                owner: String::new(),
                parent: None,
                kind: DeclarationKind::Class,
                signature: "class A".into(),
                span: SourceSpan {
                    start_byte: 0,
                    end_byte: text.len(),
                    start_line: 1,
                    end_line: 1,
                },
            },
            Declaration {
                name: "x".into(),
                owner: "A".into(),
                parent: Some(0),
                kind: DeclarationKind::Field,
                signature: "int x".into(),
                span: SourceSpan {
                    start_byte: 10,
                    end_byte: 19,
                    start_line: 1,
                    end_line: 1,
                },
            },
            Declaration {
                name: "y".into(),
                owner: "A".into(),
                parent: Some(0),
                kind: DeclarationKind::Field,
                signature: "int y".into(),
                span: SourceSpan {
                    start_byte: 10,
                    end_byte: 19,
                    start_line: 1,
                    end_line: 1,
                },
            },
        ];
        let workspace = JavaWorkspace {
            sources: paths
                .iter()
                .map(|path| JavaSource {
                    path: (*path).to_owned(),
                    blob_oid: oid.to_string(),
                    byte_length: text.len(),
                    module: None,
                    source_set: None,
                    extraction: Ok(SourceExtraction {
                        package: None,
                        imports: vec![],
                        declarations: declarations.clone(),
                    }),
                })
                .collect(),
            ..JavaWorkspace::default()
        };
        JavaSnapshot::from_workspace(
            &inventory,
            &workspace,
            &SnapshotEnvelopeV1::new("2026-09-07T00:00:00Z".into(), None, "test".into()),
        )
        .unwrap()
        .value()["semantic"]
            .clone()
    }
    fn rehash(semantic: &mut Value) {
        let graph = &mut semantic["knowledge_graph"];
        graph.as_object_mut().unwrap().remove("semantic_hash");
        graph["semantic_hash"] = hash(domain::GRAPH_HASH_DOMAIN, graph);
    }
    fn pack(semantic: &Value) -> Vec<u8> {
        let digest = semantic_hash(domain::SNAPSHOT_HASH_DOMAIN.as_bytes(), semantic);
        let id = codenoesis_domain::storage::SnapshotId::from_semantic_hash(&digest).unwrap();
        let payload =
            json!({"snapshot_id":id.as_str(),"snapshot_semantic_hash":digest,"semantic":semantic});
        canonical(&json!({"schema_version":PORTABLE_VERSION,"integrity":hash("codenoesis.java-portable.semantic.v1",&payload),"payload":payload})).unwrap()
    }
    #[test]
    fn ct_fr_ext_026_shared_spans_keep_distinct_fields_and_aggregate_package_evidence() {
        let semantic = fixture();
        let graph = &semantic["knowledge_graph"];
        assert_eq!(
            graph["entities"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|e| e["kind"] == "JavaField")
                .count(),
            4
        );
        let package = graph["entities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["kind"] == "JavaPackage")
            .unwrap();
        let claim = graph["claims"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["subject_id"] == package["id"])
            .unwrap();
        assert_eq!(claim["evidence_ids"].as_array().unwrap().len(), 2);
        assert!(parse_portable(&pack(&semantic)).is_ok());
    }
    #[test]
    fn ct_fr_ext_026_rehashed_graph_cannot_drop_boundaries_or_forge_claim_states() {
        for mutation in 0..4 {
            let mut semantic = fixture();
            let graph = &mut semantic["knowledge_graph"];
            match mutation {
                0 => {
                    graph["coverage"] = json!([]);
                }
                1 => {
                    graph["claims"][0]["state"] = json!("candidate");
                }
                2 => {
                    graph["claims"][0]["evidence_ids"] = json!(["missing"]);
                }
                _ => {
                    graph["relationships"][0]["state"] = json!("Unknown");
                }
            }
            rehash(&mut semantic);
            assert_eq!(
                parse_portable(&pack(&semantic)),
                Err(JavaError::InvalidContract)
            );
        }
    }
    #[test]
    fn ct_fr_ext_026_lineage_encoding_recursion_and_noncanonical_packages_fail() {
        assert_eq!(parse_portable(&[0xff]), Err(JavaError::InvalidContract));
        assert_eq!(
            parse_portable(format!("{}0{}", "[".repeat(200), "]".repeat(200)).as_bytes()),
            Err(JavaError::InvalidContract)
        );
        let valid = pack(&fixture());
        let mut pretty = valid.clone();
        pretty.insert(1, b' ');
        assert_eq!(parse_portable(&pretty), Err(JavaError::InvalidContract));
        let mut value: Value = serde_json::from_slice(&valid).unwrap();
        value["payload"]["snapshot_id"] = json!("wrong");
        value["integrity"] = hash("codenoesis.java-portable.semantic.v1", &value["payload"]);
        assert_eq!(
            parse_portable(&canonical(&value).unwrap()),
            Err(JavaError::InvalidContract)
        );
    }
    #[test]
    fn ct_fr_ext_026_evidence_cannot_escape_commit_or_source_boundary() {
        for (field, value) in [
            ("path", json!("../outside.java")),
            (
                "commit_oid",
                json!("2222222222222222222222222222222222222222"),
            ),
            ("end_byte", json!(domain::MAX_SOURCE_BYTES + 1)),
        ] {
            let mut semantic = fixture();
            semantic["knowledge_graph"]["evidence"][0][field] = value;
            rehash(&mut semantic);
            assert_eq!(
                parse_portable(&pack(&semantic)),
                Err(JavaError::InvalidContract)
            );
        }
    }
}
