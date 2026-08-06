use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use codenoesis_domain::RepositoryInventory;
use codenoesis_domain::s4_r7::{
    CompilerIndexError, CompilerIndexMismatchSubject, CompilerProducer, CompilerToolchain,
    R7_COMPILER_INDEX_PROFILE,
};
use serde_json::{Map, Value};
use sha2::{Digest as _, Sha256};

const BINDING_VERSION: &str = "codenoesis.compiler-index-binding/v1";
const SCIP_TAG: &str = "v0.9.0";
const SCIP_COMMIT: &str = "e8ee0ae6038f8298e2195812eea9d7b1196748ae";
const SCIP_PROTO_SHA256: &str = "04cb20f2b8be73f6c0376b5b3e84c3ae20ebaff0ad3d23ba2d16f866b395ed7d";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BoundDocument {
    pub(crate) path: String,
    pub(crate) blob_oid: String,
    pub(crate) sha256: String,
    pub(crate) byte_length: u64,
    pub(crate) omission_reason: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CompilerBinding {
    pub(crate) binding_sha256: String,
    pub(crate) artifact_path: String,
    pub(crate) artifact_byte_length: u64,
    pub(crate) artifact_sha256: String,
    pub(crate) producer: CompilerProducer,
    pub(crate) toolchain: CompilerToolchain,
    pub(crate) coverage_mode: String,
    pub(crate) indexed: Vec<BoundDocument>,
    pub(crate) omitted: Vec<BoundDocument>,
    pub(crate) known_limitations: Vec<String>,
}

pub(crate) fn artifact_relative_path(
    binding_path: &str,
    bytes: &[u8],
) -> Result<String, CompilerIndexError> {
    reject_duplicate_members(bytes).map_err(|reason| invalid_binding(binding_path, reason))?;
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| invalid_binding(binding_path, "malformed_json"))?;
    let root = object(&value)?;
    let artifact = object_field(root, "artifact").and_then(object)?;
    let path = string_field(artifact, "path")?;
    validate_adjacent_artifact_path(path)?;
    Ok(Path::new(binding_path)
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .join(path)
        .to_string_lossy()
        .into_owned())
}

#[allow(clippy::too_many_lines)]
pub(crate) fn parse_and_validate_binding(
    binding_path: &str,
    bytes: &[u8],
    binding_sha256: &str,
    inventory: &RepositoryInventory,
) -> Result<CompilerBinding, CompilerIndexError> {
    reject_duplicate_members(bytes).map_err(|reason| invalid_binding(binding_path, reason))?;
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| invalid_binding(binding_path, "malformed_json"))?;
    let root = object(&value)?;
    exact_keys(
        root,
        &[
            "schema_version",
            "profile",
            "repository",
            "artifact",
            "producer",
            "documents",
            "known_limitations",
        ],
    )?;
    require_eq(
        string_field(root, "schema_version")?,
        BINDING_VERSION,
        "schema_version",
    )?;
    require_eq(
        string_field(root, "profile")?,
        R7_COMPILER_INDEX_PROFILE,
        "profile",
    )?;

    let repository = object_field(root, "repository").and_then(object)?;
    exact_keys(
        repository,
        &[
            "identity",
            "commit_oid",
            "tree_oid",
            "source_manifest_sha256",
        ],
    )?;
    let identity = string_field(repository, "identity")?;
    let commit_oid = string_field(repository, "commit_oid")?;
    let tree_oid = string_field(repository, "tree_oid")?;
    let source_manifest_sha256 = string_field(repository, "source_manifest_sha256")?;
    validate_digest(commit_oid, 40)?;
    validate_digest(tree_oid, 40)?;
    validate_digest(source_manifest_sha256, 64)?;

    let bound = inventory.bound_revision();
    compare_bound(
        CompilerIndexMismatchSubject::Repository,
        identity,
        bound.repository_identity().as_str(),
    )?;
    compare_bound(
        CompilerIndexMismatchSubject::Revision,
        commit_oid,
        bound.commit_oid().as_str(),
    )?;
    compare_bound(
        CompilerIndexMismatchSubject::Tree,
        tree_oid,
        bound.tree_oid().as_str(),
    )?;

    let artifact = object_field(root, "artifact").and_then(object)?;
    exact_keys(
        artifact,
        &[
            "path",
            "byte_length",
            "sha256",
            "format",
            "scip_tag",
            "scip_commit",
            "scip_proto_sha256",
            "protocol_version",
            "canonical_encoding",
        ],
    )?;
    let artifact_path = string_field(artifact, "path")?;
    validate_adjacent_artifact_path(artifact_path)?;
    require_eq(
        string_field(artifact, "format")?,
        "scip-protobuf",
        "artifact.format",
    )?;
    let tag = string_field(artifact, "scip_tag")?;
    let scip_commit = string_field(artifact, "scip_commit")?;
    let proto_sha256 = string_field(artifact, "scip_proto_sha256")?;
    if tag != SCIP_TAG || scip_commit != SCIP_COMMIT || proto_sha256 != SCIP_PROTO_SHA256 {
        return Err(CompilerIndexError::UnsupportedSchema {
            commit: bounded_digest(scip_commit, 40),
            scip_proto_sha256: bounded_digest(proto_sha256, 64),
        });
    }
    if u64_field(artifact, "protocol_version")? != 0
        || bool_field(artifact, "canonical_encoding")? != Some(true)
    {
        return Err(CompilerIndexError::UnsupportedSchema {
            commit: scip_commit.to_owned(),
            scip_proto_sha256: proto_sha256.to_owned(),
        });
    }
    let artifact_sha256 = string_field(artifact, "sha256")?;
    validate_digest(artifact_sha256, 64)?;

    let producer_value = object_field(root, "producer").and_then(object)?;
    exact_keys(
        producer_value,
        &[
            "family",
            "name",
            "version",
            "commit",
            "arguments_sha256",
            "project_root_sha256",
            "toolchain",
        ],
    )?;
    let family = string_field(producer_value, "family")?;
    let name = string_field(producer_value, "name")?;
    let version = string_field(producer_value, "version")?;
    let producer_commit = string_field(producer_value, "commit")?;
    let arguments_sha256 = string_field(producer_value, "arguments_sha256")?;
    let project_root_sha256 = string_field(producer_value, "project_root_sha256")?;
    if family != "rust-analyzer-scip" || name != "rust-analyzer" {
        return Err(CompilerIndexError::UnsupportedProducer {
            name: name.chars().take(128).collect(),
            version_sha256: sha256(version.as_bytes()),
            commit_sha256: sha256(producer_commit.as_bytes()),
        });
    }
    validate_text(version, 128)?;
    validate_digest(producer_commit, 40)?;
    validate_digest(arguments_sha256, 64)?;
    validate_digest(project_root_sha256, 64)?;
    let toolchain_value = object_field(producer_value, "toolchain").and_then(object)?;
    exact_keys(
        toolchain_value,
        &["channel", "rustc_release", "rustc_commit", "target_triple"],
    )?;
    let toolchain = CompilerToolchain {
        channel: bounded_text(string_field(toolchain_value, "channel")?, 128)?,
        rustc_release: bounded_text(string_field(toolchain_value, "rustc_release")?, 128)?,
        rustc_commit: bounded_digest(string_field(toolchain_value, "rustc_commit")?, 40),
        target_triple: bounded_text(string_field(toolchain_value, "target_triple")?, 128)?,
    };
    validate_digest(&toolchain.rustc_commit, 40)?;
    if !toolchain
        .target_triple
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
    {
        return Err(CompilerIndexError::InvalidBinding {
            path: binding_path.to_owned(),
            reason: "invalid_target_triple".to_owned(),
        });
    }

    let documents = object_field(root, "documents").and_then(object)?;
    exact_keys(documents, &["coverage_mode", "indexed", "omitted"])?;
    let coverage_mode = string_field(documents, "coverage_mode")?;
    require_eq(coverage_mode, "declared_partial", "documents.coverage_mode")?;
    let indexed = parse_documents(array_field(documents, "indexed")?, false)?;
    let omitted = parse_documents(array_field(documents, "omitted")?, true)?;
    validate_documents(&indexed, inventory)?;
    validate_documents(&omitted, inventory)?;
    let observed_manifest = source_manifest_sha256_for(&indexed);
    compare_bound(
        CompilerIndexMismatchSubject::SourceManifest,
        source_manifest_sha256,
        &observed_manifest,
    )?;

    let known_limitations = string_array(root, "known_limitations")?;
    let required = BTreeSet::from([
        "absolute_project_root_redacted".to_owned(),
        "arguments_redacted".to_owned(),
        "call_kind_not_encoded".to_owned(),
        "documentation_not_imported".to_owned(),
        "generated_definition_not_source_bound".to_owned(),
    ]);
    if known_limitations.iter().cloned().collect::<BTreeSet<_>>() != required {
        return Err(invalid_binding(binding_path, "invalid_known_limitations"));
    }

    Ok(CompilerBinding {
        binding_sha256: binding_sha256.to_owned(),
        artifact_path: Path::new(binding_path)
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(artifact_path)
            .to_string_lossy()
            .into_owned(),
        artifact_byte_length: u64_field(artifact, "byte_length")?,
        artifact_sha256: artifact_sha256.to_owned(),
        producer: CompilerProducer {
            family: family.to_owned(),
            name: name.to_owned(),
            version: version.to_owned(),
            commit: producer_commit.to_owned(),
            arguments_sha256: arguments_sha256.to_owned(),
            project_root_sha256: project_root_sha256.to_owned(),
        },
        toolchain,
        coverage_mode: coverage_mode.to_owned(),
        indexed,
        omitted,
        known_limitations,
    })
}

pub(crate) fn validate_artifact_binding(
    binding: &CompilerBinding,
    byte_length: usize,
    sha256_value: &str,
) -> Result<(), CompilerIndexError> {
    let observed_length = u64::try_from(byte_length).unwrap_or(u64::MAX);
    if observed_length != binding.artifact_byte_length {
        return Err(mismatch(
            CompilerIndexMismatchSubject::Artifact,
            &binding.artifact_byte_length.to_string(),
            &observed_length.to_string(),
        ));
    }
    compare_bound(
        CompilerIndexMismatchSubject::Artifact,
        &binding.artifact_sha256,
        sha256_value,
    )
}

fn parse_documents(
    values: &[Value],
    omitted: bool,
) -> Result<Vec<BoundDocument>, CompilerIndexError> {
    let mut documents = Vec::with_capacity(values.len());
    for value in values {
        let document = object(value)?;
        if omitted {
            exact_keys(
                document,
                &["path", "blob_oid", "sha256", "byte_length", "reason"],
            )?;
        } else {
            exact_keys(document, &["path", "blob_oid", "sha256", "byte_length"])?;
        }
        let path = string_field(document, "path")?;
        validate_safe_relative(path)?;
        let blob_oid = string_field(document, "blob_oid")?;
        let digest = string_field(document, "sha256")?;
        validate_digest(blob_oid, 40)?;
        validate_digest(digest, 64)?;
        let reason = omitted
            .then(|| string_field(document, "reason"))
            .transpose()?;
        if reason.is_some_and(|value| value.is_empty() || value.len() > 128) {
            return Err(CompilerIndexError::InvalidBinding {
                path: path.to_owned(),
                reason: "invalid_omission_reason".to_owned(),
            });
        }
        documents.push(BoundDocument {
            path: path.to_owned(),
            blob_oid: blob_oid.to_owned(),
            sha256: digest.to_owned(),
            byte_length: u64_field(document, "byte_length")?,
            omission_reason: reason.map(str::to_owned),
        });
    }
    if !ordered_unique(documents.iter().map(|value| value.path.as_str())) {
        return Err(CompilerIndexError::InvalidBinding {
            path: "documents".to_owned(),
            reason: "documents_not_ordered_unique".to_owned(),
        });
    }
    Ok(documents)
}

fn validate_documents(
    documents: &[BoundDocument],
    inventory: &RepositoryInventory,
) -> Result<(), CompilerIndexError> {
    let files = inventory
        .files()
        .iter()
        .map(|file| (file.path(), file))
        .collect::<BTreeMap<_, _>>();
    for document in documents {
        let Some(file) = files.get(document.path.as_str()) else {
            return Err(mismatch(
                CompilerIndexMismatchSubject::Document,
                &document.sha256,
                &sha256(b"missing"),
            ));
        };
        let observed_sha256 = sha256(file.bytes());
        if file.blob_oid().as_str() != document.blob_oid
            || file.byte_length() != document.byte_length
            || observed_sha256 != document.sha256
        {
            return Err(mismatch(
                CompilerIndexMismatchSubject::Document,
                &document.sha256,
                &observed_sha256,
            ));
        }
    }
    Ok(())
}

fn source_manifest_sha256_for(documents: &[BoundDocument]) -> String {
    let value = Value::Array(
        documents
            .iter()
            .map(|document| {
                Value::Object(Map::from_iter([
                    (
                        "blob_oid".to_owned(),
                        Value::String(document.blob_oid.clone()),
                    ),
                    (
                        "byte_length".to_owned(),
                        Value::Number(document.byte_length.into()),
                    ),
                    ("path".to_owned(), Value::String(document.path.clone())),
                    ("sha256".to_owned(), Value::String(document.sha256.clone())),
                ]))
            })
            .collect(),
    );
    sha256(&serde_json::to_vec(&value).expect("JSON value serialization cannot fail"))
}

fn compare_bound(
    subject: CompilerIndexMismatchSubject,
    expected: &str,
    observed: &str,
) -> Result<(), CompilerIndexError> {
    if expected == observed {
        Ok(())
    } else {
        Err(mismatch(subject, expected, observed))
    }
}

fn mismatch(
    subject: CompilerIndexMismatchSubject,
    expected: &str,
    observed: &str,
) -> CompilerIndexError {
    CompilerIndexError::BindingMismatch {
        subject,
        expected_sha256: digest_or_hash(expected),
        observed_sha256: digest_or_hash(observed),
    }
}

fn digest_or_hash(value: &str) -> String {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        value.to_owned()
    } else {
        sha256(value.as_bytes())
    }
}

fn reject_duplicate_members(bytes: &[u8]) -> Result<(), &'static str> {
    let mut parser = JsonMemberParser { bytes, index: 0 };
    parser.value(0)?;
    parser.whitespace();
    (parser.index == bytes.len())
        .then_some(())
        .ok_or("trailing_json_bytes")
}

struct JsonMemberParser<'a> {
    bytes: &'a [u8],
    index: usize,
}

impl JsonMemberParser<'_> {
    fn value(&mut self, depth: u8) -> Result<(), &'static str> {
        if depth > 64 {
            return Err("json_recursion_limit");
        }
        self.whitespace();
        match self.bytes.get(self.index).copied() {
            Some(b'{') => self.object(depth + 1),
            Some(b'[') => self.array(depth + 1),
            Some(b'"') => self.string().map(|_| ()),
            Some(b't') => self.literal(b"true"),
            Some(b'f') => self.literal(b"false"),
            Some(b'n') => self.literal(b"null"),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err("malformed_json"),
        }
    }

    fn object(&mut self, depth: u8) -> Result<(), &'static str> {
        self.index += 1;
        self.whitespace();
        let mut keys = BTreeSet::new();
        if self.take(b'}') {
            return Ok(());
        }
        loop {
            self.whitespace();
            let key = self.string()?;
            if !keys.insert(key) {
                return Err("duplicate_json_member");
            }
            self.whitespace();
            if !self.take(b':') {
                return Err("malformed_json");
            }
            self.value(depth)?;
            self.whitespace();
            if self.take(b'}') {
                return Ok(());
            }
            if !self.take(b',') {
                return Err("malformed_json");
            }
        }
    }

    fn array(&mut self, depth: u8) -> Result<(), &'static str> {
        self.index += 1;
        self.whitespace();
        if self.take(b']') {
            return Ok(());
        }
        loop {
            self.value(depth)?;
            self.whitespace();
            if self.take(b']') {
                return Ok(());
            }
            if !self.take(b',') {
                return Err("malformed_json");
            }
        }
    }

    fn string(&mut self) -> Result<String, &'static str> {
        let start = self.index;
        if !self.take(b'"') {
            return Err("malformed_json");
        }
        let mut escaped = false;
        while let Some(byte) = self.bytes.get(self.index).copied() {
            self.index += 1;
            if escaped {
                escaped = false;
                continue;
            }
            match byte {
                b'\\' => escaped = true,
                b'"' => {
                    return serde_json::from_slice(&self.bytes[start..self.index])
                        .map_err(|_| "malformed_json_string");
                }
                0x00..=0x1f => return Err("malformed_json_string"),
                _ => {}
            }
        }
        Err("malformed_json_string")
    }

    fn literal(&mut self, literal: &[u8]) -> Result<(), &'static str> {
        if self.bytes.get(self.index..self.index + literal.len()) == Some(literal) {
            self.index += literal.len();
            Ok(())
        } else {
            Err("malformed_json")
        }
    }

    fn number(&mut self) -> Result<(), &'static str> {
        let start = self.index;
        while self
            .bytes
            .get(self.index)
            .is_some_and(|byte| matches!(byte, b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9'))
        {
            self.index += 1;
        }
        serde_json::from_slice::<Value>(&self.bytes[start..self.index])
            .map(|_| ())
            .map_err(|_| "malformed_json_number")
    }

    fn whitespace(&mut self) {
        while self
            .bytes
            .get(self.index)
            .is_some_and(|byte| matches!(byte, b' ' | b'\n' | b'\r' | b'\t'))
        {
            self.index += 1;
        }
    }

    fn take(&mut self, byte: u8) -> bool {
        if self.bytes.get(self.index) == Some(&byte) {
            self.index += 1;
            true
        } else {
            false
        }
    }
}

fn object(value: &Value) -> Result<&Map<String, Value>, CompilerIndexError> {
    value
        .as_object()
        .ok_or_else(|| CompilerIndexError::InvalidBinding {
            path: "binding".to_owned(),
            reason: "object_required".to_owned(),
        })
}

fn object_field<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a Value, CompilerIndexError> {
    object
        .get(key)
        .ok_or_else(|| CompilerIndexError::InvalidBinding {
            path: "binding".to_owned(),
            reason: format!("missing_{key}"),
        })
}

fn string_field<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a str, CompilerIndexError> {
    object_field(object, key)?
        .as_str()
        .ok_or_else(|| CompilerIndexError::InvalidBinding {
            path: "binding".to_owned(),
            reason: format!("invalid_{key}"),
        })
}

fn u64_field(object: &Map<String, Value>, key: &str) -> Result<u64, CompilerIndexError> {
    object_field(object, key)?
        .as_u64()
        .ok_or_else(|| CompilerIndexError::InvalidBinding {
            path: "binding".to_owned(),
            reason: format!("invalid_{key}"),
        })
}

fn bool_field(object: &Map<String, Value>, key: &str) -> Result<Option<bool>, CompilerIndexError> {
    object_field(object, key)?
        .as_bool()
        .map(Some)
        .ok_or_else(|| CompilerIndexError::InvalidBinding {
            path: "binding".to_owned(),
            reason: format!("invalid_{key}"),
        })
}

fn array_field<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a [Value], CompilerIndexError> {
    object_field(object, key)?
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| CompilerIndexError::InvalidBinding {
            path: "binding".to_owned(),
            reason: format!("invalid_{key}"),
        })
}

fn string_array(object: &Map<String, Value>, key: &str) -> Result<Vec<String>, CompilerIndexError> {
    array_field(object, key)?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| CompilerIndexError::InvalidBinding {
                    path: "binding".to_owned(),
                    reason: format!("invalid_{key}"),
                })
        })
        .collect()
}

fn exact_keys(object: &Map<String, Value>, expected: &[&str]) -> Result<(), CompilerIndexError> {
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    (actual == expected)
        .then_some(())
        .ok_or_else(|| CompilerIndexError::InvalidBinding {
            path: "binding".to_owned(),
            reason: "unknown_or_missing_member".to_owned(),
        })
}

fn require_eq(actual: &str, expected: &str, field: &str) -> Result<(), CompilerIndexError> {
    (actual == expected)
        .then_some(())
        .ok_or_else(|| CompilerIndexError::InvalidBinding {
            path: "binding".to_owned(),
            reason: format!("invalid_{field}"),
        })
}

fn validate_safe_relative(path: &str) -> Result<(), CompilerIndexError> {
    let path_value = Path::new(path);
    if path.is_empty()
        || path.len() > 4_096
        || path.contains('\\')
        || path_value.is_absolute()
        || path_value.components().any(|component| {
            matches!(
                component,
                Component::CurDir
                    | Component::ParentDir
                    | Component::RootDir
                    | Component::Prefix(_)
            )
        })
    {
        return Err(CompilerIndexError::UnsafePath {
            path: path.chars().take(4_096).collect(),
            reason: "path_must_be_safe_relative".to_owned(),
        });
    }
    Ok(())
}

fn validate_adjacent_artifact_path(path: &str) -> Result<(), CompilerIndexError> {
    validate_safe_relative(path)?;
    if Path::new(path).components().count() == 1 {
        Ok(())
    } else {
        Err(CompilerIndexError::UnsafePath {
            path: path.chars().take(4_096).collect(),
            reason: "artifact_must_be_adjacent".to_owned(),
        })
    }
}

fn validate_digest(value: &str, length: usize) -> Result<(), CompilerIndexError> {
    if value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        Ok(())
    } else {
        Err(CompilerIndexError::InvalidBinding {
            path: "binding".to_owned(),
            reason: "invalid_digest".to_owned(),
        })
    }
}

fn validate_text(value: &str, maximum: usize) -> Result<(), CompilerIndexError> {
    if value.is_empty() || value.len() > maximum {
        Err(CompilerIndexError::InvalidBinding {
            path: "binding".to_owned(),
            reason: "invalid_text".to_owned(),
        })
    } else {
        Ok(())
    }
}

fn bounded_text(value: &str, maximum: usize) -> Result<String, CompilerIndexError> {
    validate_text(value, maximum)?;
    Ok(value.to_owned())
}

fn bounded_digest(value: &str, length: usize) -> String {
    if value.len() == length {
        value.to_owned()
    } else {
        "0".repeat(length)
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

fn invalid_binding(path: &str, reason: &str) -> CompilerIndexError {
    CompilerIndexError::InvalidBinding {
        path: path.chars().take(4_096).collect(),
        reason: reason.chars().take(256).collect(),
    }
}

pub(crate) fn sha256(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use codenoesis_domain::s4_r7::CompilerIndexError;

    use super::validate_adjacent_artifact_path;

    #[test]
    fn sec_fr_ext_005_artifact_must_be_adjacent_to_binding() {
        validate_adjacent_artifact_path("index.scip").expect("adjacent artifact path");
        for path in ["nested/index.scip", "../index.scip", "/tmp/index.scip"] {
            assert!(matches!(
                validate_adjacent_artifact_path(path),
                Err(CompilerIndexError::UnsafePath { .. })
            ));
        }
    }
}
