use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use codenoesis_contracts::{
    DocumentationContractError, GeneratedDocumentationV1, validate_documentation_bundle_v1,
};
use serde_json::Value;

const MARKER_BYTES: &[u8] = b"{\"schema_version\":\"codenoesis.generated-docs-marker/v1\"}\n";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneratedDocsError {
    InvalidRoot,
    UnmarkedNonemptyRoot,
    UnsafePath,
    SnapshotMismatch,
    CorruptGeneration,
    Failed,
}

/// Creates only an absent safe generated-document root so a capability
/// boundary can reference it.
///
/// # Errors
///
/// Returns a typed input, overlap, symlink, or creation failure.
pub fn ensure_output_root_for_boundary(
    store_root: &Path,
    output_root: &Path,
) -> Result<(), GeneratedDocsError> {
    let (output, output_exists) = validate_disjoint_root(store_root, output_root)?;
    if !output_exists {
        let parent = output.parent().ok_or(GeneratedDocsError::InvalidRoot)?;
        fs::create_dir(&output).map_err(|_| GeneratedDocsError::Failed)?;
        sync_directory(parent)?;
    }
    Ok(())
}

/// Validates a generated-document root before rendering without creating or
/// modifying it.
///
/// # Errors
///
/// Returns a typed root, ownership, overlap, symlink, or corruption failure.
pub fn validate_output_root_for_generation(
    store_root: &Path,
    output_root: &Path,
) -> Result<(), GeneratedDocsError> {
    let (output, exists) = validate_disjoint_root(store_root, output_root)?;
    if !exists {
        return Ok(());
    }
    let root = fs::canonicalize(output).map_err(|_| GeneratedDocsError::InvalidRoot)?;
    match inspect_root(&root)? {
        RootState::Empty => Ok(()),
        RootState::MarkedWithoutManifest => validate_marked_root_structure(&root),
        RootState::Complete => {
            let manifest = load_manifest_value(&root)?;
            let (repository_identity, snapshot_id, snapshot_semantic_hash) =
                manifest_binding(&manifest)?;
            validate_generation(
                &root,
                &manifest,
                repository_identity,
                snapshot_id,
                snapshot_semantic_hash,
            )
        }
    }
}

/// Validates an existing generated-document root before a read-only query
/// boundary references it.
///
/// # Errors
///
/// Returns a typed input, overlap, or symlink-component failure.
pub fn validate_documents_root_for_boundary(
    store_root: &Path,
    documents_root: &Path,
) -> Result<(), GeneratedDocsError> {
    let (_, exists) = validate_disjoint_root(store_root, documents_root)?;
    if exists {
        Ok(())
    } else {
        Err(GeneratedDocsError::InvalidRoot)
    }
}

fn validate_disjoint_root(
    store_root: &Path,
    requested_root: &Path,
) -> Result<(PathBuf, bool), GeneratedDocsError> {
    let output = absolute_without_parent_components(requested_root)?;
    let store = absolute_without_parent_components(store_root)?;
    let trusted_ancestor = common_ancestor(&store, &output);
    if trusted_ancestor.as_os_str().is_empty() {
        return Err(GeneratedDocsError::InvalidRoot);
    }
    verify_directory(&trusted_ancestor)?;
    verify_directory(&store)?;
    verify_existing_components(&store, &trusted_ancestor)?;
    let parent = output.parent().ok_or(GeneratedDocsError::InvalidRoot)?;
    let output_exists = match fs::symlink_metadata(&output) {
        Ok(metadata) => {
            if !metadata.is_dir() || is_unsafe_metadata(&metadata) {
                return Err(GeneratedDocsError::UnsafePath);
            }
            verify_existing_components(&output, &trusted_ancestor)?;
            true
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            verify_directory(parent)?;
            verify_existing_components(parent, &trusted_ancestor)?;
            false
        }
        Err(_) => return Err(GeneratedDocsError::InvalidRoot),
    };
    let canonical_output = if output_exists {
        fs::canonicalize(&output).map_err(|_| GeneratedDocsError::InvalidRoot)?
    } else {
        fs::canonicalize(parent)
            .map_err(|_| GeneratedDocsError::InvalidRoot)?
            .join(output.file_name().ok_or(GeneratedDocsError::InvalidRoot)?)
    };
    let canonical_store = fs::canonicalize(&store).map_err(|_| GeneratedDocsError::InvalidRoot)?;
    if canonical_output.starts_with(&canonical_store)
        || canonical_store.starts_with(&canonical_output)
    {
        return Err(GeneratedDocsError::UnsafePath);
    }
    Ok((output, output_exists))
}

/// Publishes or validates one deterministic generated-document generation.
///
/// # Errors
///
/// Returns a typed root, ownership, binding, integrity, or publication failure.
pub fn publish(
    store_root: &Path,
    output_root: &Path,
    generated: &GeneratedDocumentationV1,
) -> Result<(), GeneratedDocsError> {
    ensure_output_root_for_boundary(store_root, output_root)?;
    let root = fs::canonicalize(output_root).map_err(|_| GeneratedDocsError::InvalidRoot)?;
    let state = inspect_root(&root)?;
    match state {
        RootState::Empty => {
            write_exclusive(&root.join(".codenoesis-generated.json"), MARKER_BYTES)?;
            sync_directory(&root)?;
        }
        RootState::MarkedWithoutManifest => {}
        RootState::Complete => {
            let existing = load_manifest_value(&root)?;
            let (repository_identity, snapshot_id, snapshot_semantic_hash) =
                manifest_binding(&existing)?;
            validate_generation(
                &root,
                &existing,
                repository_identity,
                snapshot_id,
                snapshot_semantic_hash,
            )?;
            if existing != *generated.manifest() {
                return Err(GeneratedDocsError::SnapshotMismatch);
            }
            return Ok(());
        }
    }
    validate_precommit_root(&root, generated)?;

    let generation = generated
        .manifest()
        .pointer("/generation_hash/value")
        .and_then(Value::as_str)
        .ok_or(GeneratedDocsError::CorruptGeneration)?;
    let temporary_root = root.join(".tmp");
    create_or_verify_directory(&temporary_root)?;
    let staging = temporary_root.join(generation);
    create_or_verify_directory(&staging)?;
    let staging_modules = staging.join("modules");
    create_or_verify_directory(&staging_modules)?;

    for document in generated.documents() {
        let relative = safe_document_path(&document.path)?;
        let destination = staging.join(relative);
        write_or_verify(&destination, &document.bytes)?;
    }
    let manifest_bytes = generated
        .canonical_manifest_file()
        .map_err(|_| GeneratedDocsError::Failed)?;
    write_or_verify(&staging.join("manifest.json"), &manifest_bytes)?;
    validate_staged_generation(&staging, generated, &manifest_bytes)?;
    sync_directory(&staging_modules)?;
    sync_directory(&staging)?;

    let modules = root.join("modules");
    create_or_verify_directory(&modules)?;
    for document in generated.documents() {
        let relative = safe_document_path(&document.path)?;
        let staged = staging.join(relative);
        let destination = root.join(relative);
        publish_noclobber(&staged, &destination, &document.bytes)?;
    }
    sync_directory(&modules)?;
    sync_directory(&root)?;
    publish_noclobber(
        &staging.join("manifest.json"),
        &root.join("manifest.json"),
        &manifest_bytes,
    )?;
    sync_directory(&root)?;
    let (repository_identity, snapshot_id, snapshot_semantic_hash) =
        manifest_binding(generated.manifest())?;
    validate_generation(
        &root,
        generated.manifest(),
        repository_identity,
        snapshot_id,
        snapshot_semantic_hash,
    )?;
    let _ = cleanup_staging(&staging, generated);
    Ok(())
}

/// Loads and verifies one complete generated-document generation.
///
/// # Errors
///
/// Returns a typed root, ownership, binding, or content-integrity failure.
pub fn load_validated_manifest(
    documents_root: &Path,
    repository_identity: &str,
    snapshot_id: &str,
    snapshot_semantic_hash: &str,
) -> Result<Value, GeneratedDocsError> {
    let root = absolute_without_parent_components(documents_root)?;
    verify_directory(&root)?;
    if inspect_root(&root)? != RootState::Complete {
        return Err(GeneratedDocsError::CorruptGeneration);
    }
    let manifest = load_manifest_value(&root)?;
    validate_generation(
        &root,
        &manifest,
        repository_identity,
        snapshot_id,
        snapshot_semantic_hash,
    )?;
    Ok(manifest)
}

fn validate_generation(
    root: &Path,
    manifest: &Value,
    repository_identity: &str,
    snapshot_id: &str,
    snapshot_semantic_hash: &str,
) -> Result<(), GeneratedDocsError> {
    if manifest.get("repository_identity").and_then(Value::as_str) != Some(repository_identity)
        || manifest.get("snapshot_id").and_then(Value::as_str) != Some(snapshot_id)
        || manifest
            .pointer("/snapshot_semantic_hash/value")
            .and_then(Value::as_str)
            != Some(snapshot_semantic_hash)
    {
        return Err(GeneratedDocsError::SnapshotMismatch);
    }
    let documents = manifest
        .get("documents")
        .and_then(Value::as_array)
        .ok_or(GeneratedDocsError::CorruptGeneration)?;
    let mut bytes = BTreeMap::new();
    let mut expected_paths = BTreeSet::new();
    for document in documents {
        let path = document
            .get("path")
            .and_then(Value::as_str)
            .ok_or(GeneratedDocsError::CorruptGeneration)?;
        let relative = safe_document_path(path)?;
        let destination = root.join(relative);
        verify_regular_file(&destination)?;
        if !expected_paths.insert(path.to_owned()) {
            return Err(GeneratedDocsError::CorruptGeneration);
        }
        bytes.insert(
            path.to_owned(),
            fs::read(destination).map_err(|_| GeneratedDocsError::CorruptGeneration)?,
        );
    }
    validate_documentation_bundle_v1(
        manifest,
        &bytes,
        repository_identity,
        snapshot_id,
        snapshot_semantic_hash,
    )
    .map_err(map_contract_error)?;
    validate_owned_entries(root, &expected_paths)
}

fn manifest_binding(manifest: &Value) -> Result<(&str, &str, &str), GeneratedDocsError> {
    Ok((
        manifest
            .get("repository_identity")
            .and_then(Value::as_str)
            .ok_or(GeneratedDocsError::CorruptGeneration)?,
        manifest
            .get("snapshot_id")
            .and_then(Value::as_str)
            .ok_or(GeneratedDocsError::CorruptGeneration)?,
        manifest
            .pointer("/snapshot_semantic_hash/value")
            .and_then(Value::as_str)
            .ok_or(GeneratedDocsError::CorruptGeneration)?,
    ))
}

fn validate_precommit_root(
    root: &Path,
    generated: &GeneratedDocumentationV1,
) -> Result<(), GeneratedDocsError> {
    for entry in fs::read_dir(root).map_err(|_| GeneratedDocsError::CorruptGeneration)? {
        let entry = entry.map_err(|_| GeneratedDocsError::CorruptGeneration)?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| GeneratedDocsError::UnsafePath)?;
        match name.as_str() {
            ".codenoesis-generated.json" => verify_exact_file(&entry.path(), MARKER_BYTES)?,
            "overview.md" => verify_generated_document(&entry.path(), "overview.md", generated)?,
            "modules" => validate_precommit_modules(&entry.path(), generated)?,
            ".tmp" => validate_temporary_root(&entry.path())?,
            "manifest.json" => return Err(GeneratedDocsError::CorruptGeneration),
            _ => return Err(GeneratedDocsError::UnsafePath),
        }
    }
    Ok(())
}

fn validate_marked_root_structure(root: &Path) -> Result<(), GeneratedDocsError> {
    for entry in fs::read_dir(root).map_err(|_| GeneratedDocsError::CorruptGeneration)? {
        let entry = entry.map_err(|_| GeneratedDocsError::CorruptGeneration)?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| GeneratedDocsError::UnsafePath)?;
        match name.as_str() {
            ".codenoesis-generated.json" => verify_exact_file(&entry.path(), MARKER_BYTES)?,
            "overview.md" => verify_regular_file(&entry.path())?,
            "modules" => validate_temporary_modules(&entry.path())?,
            ".tmp" => validate_temporary_root(&entry.path())?,
            "manifest.json" => return Err(GeneratedDocsError::CorruptGeneration),
            _ => return Err(GeneratedDocsError::UnsafePath),
        }
    }
    Ok(())
}

fn validate_precommit_modules(
    modules: &Path,
    generated: &GeneratedDocumentationV1,
) -> Result<(), GeneratedDocsError> {
    verify_directory(modules)?;
    for entry in fs::read_dir(modules).map_err(|_| GeneratedDocsError::CorruptGeneration)? {
        let entry = entry.map_err(|_| GeneratedDocsError::CorruptGeneration)?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| GeneratedDocsError::UnsafePath)?;
        verify_generated_document(&entry.path(), &format!("modules/{name}"), generated)?;
    }
    Ok(())
}

fn verify_generated_document(
    path: &Path,
    relative: &str,
    generated: &GeneratedDocumentationV1,
) -> Result<(), GeneratedDocsError> {
    let document = generated
        .documents()
        .iter()
        .find(|document| document.path == relative)
        .ok_or(GeneratedDocsError::UnsafePath)?;
    verify_exact_file(path, &document.bytes)
}

fn validate_staged_generation(
    staging: &Path,
    generated: &GeneratedDocumentationV1,
    manifest_bytes: &[u8],
) -> Result<(), GeneratedDocsError> {
    verify_directory(staging)?;
    let mut expected_paths = BTreeSet::new();
    let mut document_bytes = BTreeMap::new();
    for document in generated.documents() {
        safe_document_path(&document.path)?;
        if !expected_paths.insert(document.path.clone()) {
            return Err(GeneratedDocsError::CorruptGeneration);
        }
        let path = staging.join(&document.path);
        verify_exact_file(&path, &document.bytes)?;
        document_bytes.insert(
            document.path.clone(),
            fs::read(path).map_err(|_| GeneratedDocsError::CorruptGeneration)?,
        );
    }
    verify_exact_file(&staging.join("manifest.json"), manifest_bytes)?;
    validate_staged_entries(staging, &expected_paths)?;
    let (repository_identity, snapshot_id, snapshot_semantic_hash) =
        manifest_binding(generated.manifest())?;
    validate_documentation_bundle_v1(
        generated.manifest(),
        &document_bytes,
        repository_identity,
        snapshot_id,
        snapshot_semantic_hash,
    )
    .map_err(map_contract_error)
}

fn validate_staged_entries(
    staging: &Path,
    expected_documents: &BTreeSet<String>,
) -> Result<(), GeneratedDocsError> {
    for entry in fs::read_dir(staging).map_err(|_| GeneratedDocsError::CorruptGeneration)? {
        let entry = entry.map_err(|_| GeneratedDocsError::CorruptGeneration)?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| GeneratedDocsError::UnsafePath)?;
        match name.as_str() {
            "manifest.json" => verify_regular_file(&entry.path())?,
            "overview.md" if expected_documents.contains("overview.md") => {
                verify_regular_file(&entry.path())?;
            }
            "modules" => validate_module_entries(&entry.path(), expected_documents)?,
            _ => return Err(GeneratedDocsError::UnsafePath),
        }
    }
    Ok(())
}

fn validate_module_entries(
    modules: &Path,
    expected_documents: &BTreeSet<String>,
) -> Result<(), GeneratedDocsError> {
    verify_directory(modules)?;
    for entry in fs::read_dir(modules).map_err(|_| GeneratedDocsError::CorruptGeneration)? {
        let entry = entry.map_err(|_| GeneratedDocsError::CorruptGeneration)?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| GeneratedDocsError::UnsafePath)?;
        if !expected_documents.contains(&format!("modules/{name}")) {
            return Err(GeneratedDocsError::UnsafePath);
        }
        verify_regular_file(&entry.path())?;
    }
    Ok(())
}

fn inspect_root(root: &Path) -> Result<RootState, GeneratedDocsError> {
    verify_directory(root)?;
    let entries = fs::read_dir(root)
        .map_err(|_| GeneratedDocsError::Failed)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| GeneratedDocsError::Failed)?;
    if entries.is_empty() {
        return Ok(RootState::Empty);
    }
    let names = entries
        .into_iter()
        .map(|entry| {
            entry
                .file_name()
                .into_string()
                .map_err(|_| GeneratedDocsError::UnsafePath)
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let marker = root.join(".codenoesis-generated.json");
    if !names.contains(".codenoesis-generated.json") {
        return Err(GeneratedDocsError::UnmarkedNonemptyRoot);
    }
    verify_regular_file(&marker)?;
    if fs::read(marker).map_err(|_| GeneratedDocsError::CorruptGeneration)? != MARKER_BYTES {
        return Err(GeneratedDocsError::CorruptGeneration);
    }
    if names.iter().any(|name| {
        !matches!(
            name.as_str(),
            ".codenoesis-generated.json" | "manifest.json" | "overview.md" | "modules" | ".tmp"
        )
    }) {
        return Err(GeneratedDocsError::UnsafePath);
    }
    if names.contains("manifest.json") {
        verify_regular_file(&root.join("manifest.json"))?;
        Ok(RootState::Complete)
    } else {
        Ok(RootState::MarkedWithoutManifest)
    }
}

fn validate_owned_entries(
    root: &Path,
    expected_documents: &BTreeSet<String>,
) -> Result<(), GeneratedDocsError> {
    for entry in fs::read_dir(root).map_err(|_| GeneratedDocsError::CorruptGeneration)? {
        let entry = entry.map_err(|_| GeneratedDocsError::CorruptGeneration)?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| GeneratedDocsError::UnsafePath)?;
        match name.as_str() {
            ".codenoesis-generated.json" => verify_exact_file(&entry.path(), MARKER_BYTES)?,
            "manifest.json" => verify_regular_file(&entry.path())?,
            "overview.md" if expected_documents.contains("overview.md") => {
                verify_regular_file(&entry.path())?;
            }
            "modules" => validate_module_entries(&entry.path(), expected_documents)?,
            ".tmp" => validate_temporary_root(&entry.path())?,
            _ => return Err(GeneratedDocsError::UnsafePath),
        }
    }
    Ok(())
}

fn validate_temporary_root(path: &Path) -> Result<(), GeneratedDocsError> {
    verify_directory(path)?;
    for generation in fs::read_dir(path).map_err(|_| GeneratedDocsError::CorruptGeneration)? {
        let generation = generation.map_err(|_| GeneratedDocsError::CorruptGeneration)?;
        let name = generation
            .file_name()
            .into_string()
            .map_err(|_| GeneratedDocsError::UnsafePath)?;
        if name.len() != 64
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(GeneratedDocsError::UnsafePath);
        }
        validate_temporary_generation(&generation.path())?;
    }
    Ok(())
}

fn validate_temporary_generation(path: &Path) -> Result<(), GeneratedDocsError> {
    verify_directory(path)?;
    for entry in fs::read_dir(path).map_err(|_| GeneratedDocsError::CorruptGeneration)? {
        let entry = entry.map_err(|_| GeneratedDocsError::CorruptGeneration)?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| GeneratedDocsError::UnsafePath)?;
        match name.as_str() {
            "manifest.json" | "overview.md" => verify_regular_file(&entry.path())?,
            "modules" => validate_temporary_modules(&entry.path())?,
            _ => return Err(GeneratedDocsError::UnsafePath),
        }
    }
    Ok(())
}

fn validate_temporary_modules(path: &Path) -> Result<(), GeneratedDocsError> {
    verify_directory(path)?;
    for module in fs::read_dir(path).map_err(|_| GeneratedDocsError::CorruptGeneration)? {
        let module = module.map_err(|_| GeneratedDocsError::CorruptGeneration)?;
        let module_name = module
            .file_name()
            .into_string()
            .map_err(|_| GeneratedDocsError::UnsafePath)?;
        safe_document_path(&format!("modules/{module_name}"))?;
        verify_regular_file(&module.path())?;
    }
    Ok(())
}

fn load_manifest_value(root: &Path) -> Result<Value, GeneratedDocsError> {
    let path = root.join("manifest.json");
    verify_regular_file(&path)?;
    let bytes = fs::read(path).map_err(|_| GeneratedDocsError::CorruptGeneration)?;
    let value = serde_json::from_slice::<Value>(&bytes)
        .map_err(|_| GeneratedDocsError::CorruptGeneration)?;
    if serde_json::to_vec(&value).map_err(|_| GeneratedDocsError::CorruptGeneration)? != bytes {
        return Err(GeneratedDocsError::CorruptGeneration);
    }
    Ok(value)
}

fn cleanup_staging(
    staging: &Path,
    generated: &GeneratedDocumentationV1,
) -> Result<(), GeneratedDocsError> {
    for document in generated.documents() {
        let relative = safe_document_path(&document.path)?;
        fs::remove_file(staging.join(relative)).map_err(|_| GeneratedDocsError::Failed)?;
    }
    fs::remove_file(staging.join("manifest.json")).map_err(|_| GeneratedDocsError::Failed)?;
    fs::remove_dir(staging.join("modules")).map_err(|_| GeneratedDocsError::Failed)?;
    fs::remove_dir(staging).map_err(|_| GeneratedDocsError::Failed)?;
    sync_directory(
        staging
            .parent()
            .ok_or(GeneratedDocsError::CorruptGeneration)?,
    )
}

fn create_or_verify_directory(path: &Path) -> Result<(), GeneratedDocsError> {
    match fs::symlink_metadata(path) {
        Ok(_) => verify_directory(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(path).map_err(|_| GeneratedDocsError::Failed)?;
            sync_directory(path.parent().ok_or(GeneratedDocsError::InvalidRoot)?)
        }
        Err(_) => Err(GeneratedDocsError::Failed),
    }
}

fn write_or_verify(path: &Path, bytes: &[u8]) -> Result<(), GeneratedDocsError> {
    match fs::symlink_metadata(path) {
        Ok(_) => verify_exact_file(path, bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => write_exclusive(path, bytes),
        Err(_) => Err(GeneratedDocsError::Failed),
    }
}

fn publish_noclobber(
    source: &Path,
    destination: &Path,
    expected: &[u8],
) -> Result<(), GeneratedDocsError> {
    verify_exact_file(source, expected)?;
    match fs::symlink_metadata(destination) {
        Ok(_) => verify_exact_file(destination, expected),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::hard_link(source, destination).map_err(|_| GeneratedDocsError::Failed)?;
            verify_exact_file(destination, expected)
        }
        Err(_) => Err(GeneratedDocsError::Failed),
    }
}

fn verify_exact_file(path: &Path, expected: &[u8]) -> Result<(), GeneratedDocsError> {
    verify_regular_file(path)?;
    if fs::read(path).map_err(|_| GeneratedDocsError::CorruptGeneration)? == expected {
        Ok(())
    } else {
        Err(GeneratedDocsError::CorruptGeneration)
    }
}

fn write_exclusive(path: &Path, bytes: &[u8]) -> Result<(), GeneratedDocsError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| GeneratedDocsError::Failed)?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| GeneratedDocsError::Failed)
}

fn safe_document_path(path: &str) -> Result<&Path, GeneratedDocsError> {
    let path = Path::new(path);
    let valid = path == Path::new("overview.md")
        || path
            .components()
            .collect::<Vec<_>>()
            .as_slice()
            .split_first()
            .is_some_and(|(first, rest)| {
                *first == Component::Normal("modules".as_ref())
                    && rest.len() == 1
                    && rest[0].as_os_str().to_str().is_some_and(|name| {
                        name.strip_suffix(".md").is_some_and(|slug| {
                            !slug.is_empty()
                                && slug.bytes().all(|byte| {
                                    byte.is_ascii_lowercase()
                                        || byte.is_ascii_digit()
                                        || byte == b'-'
                                })
                        })
                    })
            });
    valid.then_some(path).ok_or(GeneratedDocsError::UnsafePath)
}

fn absolute_without_parent_components(path: &Path) -> Result<PathBuf, GeneratedDocsError> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(GeneratedDocsError::InvalidRoot);
    }
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::env::current_dir()
            .map_err(|_| GeneratedDocsError::InvalidRoot)
            .map(|current| current.join(path))
    }
}

fn verify_existing_components(
    path: &Path,
    trusted_ancestor: &Path,
) -> Result<(), GeneratedDocsError> {
    let relative = path
        .strip_prefix(trusted_ancestor)
        .map_err(|_| GeneratedDocsError::UnsafePath)?;
    let mut current = trusted_ancestor.to_path_buf();
    for component in relative.components() {
        current.push(component.as_os_str());
        let metadata =
            fs::symlink_metadata(&current).map_err(|_| GeneratedDocsError::UnsafePath)?;
        if is_unsafe_metadata(&metadata) {
            return Err(GeneratedDocsError::UnsafePath);
        }
    }
    Ok(())
}

fn common_ancestor(left: &Path, right: &Path) -> PathBuf {
    let mut common = PathBuf::new();
    for (left, right) in left.components().zip(right.components()) {
        if left != right {
            break;
        }
        common.push(left.as_os_str());
    }
    common
}

fn verify_directory(path: &Path) -> Result<(), GeneratedDocsError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| GeneratedDocsError::InvalidRoot)?;
    if !metadata.is_dir() || is_unsafe_metadata(&metadata) {
        return Err(GeneratedDocsError::UnsafePath);
    }
    Ok(())
}

fn verify_regular_file(path: &Path) -> Result<(), GeneratedDocsError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| GeneratedDocsError::CorruptGeneration)?;
    if !metadata.is_file() || is_unsafe_metadata(&metadata) {
        return Err(GeneratedDocsError::UnsafePath);
    }
    Ok(())
}

#[cfg(unix)]
fn is_unsafe_metadata(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(windows)]
fn is_unsafe_metadata(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(any(unix, windows)))]
fn is_unsafe_metadata(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(windows)]
fn sync_directory(_directory: &Path) -> Result<(), GeneratedDocsError> {
    Ok(())
}

#[cfg(not(windows))]
fn sync_directory(directory: &Path) -> Result<(), GeneratedDocsError> {
    File::open(directory)
        .and_then(|file| file.sync_all())
        .map_err(|_| GeneratedDocsError::Failed)
}

fn map_contract_error(error: DocumentationContractError) -> GeneratedDocsError {
    match error {
        DocumentationContractError::InvalidSnapshot => GeneratedDocsError::CorruptGeneration,
        DocumentationContractError::LimitExceeded => GeneratedDocsError::Failed,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RootState {
    Empty,
    MarkedWithoutManifest,
    Complete,
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn sec_fr_doc_003_overlap_fails_without_creating_output() {
        let root = test_root("overlap");
        let store = root.join("store");
        fs::create_dir(&store).expect("create store");
        let output = store.join("documents");

        assert_eq!(
            ensure_output_root_for_boundary(&store, &output),
            Err(GeneratedDocsError::UnsafePath)
        );
        assert!(!output.exists(), "unsafe overlap created the output root");

        fs::remove_dir_all(root).expect("remove test root");
    }

    #[test]
    fn sec_fr_doc_003_unrecognized_marked_entry_fails_closed() {
        let root = test_root("unrecognized");
        fs::write(root.join(".codenoesis-generated.json"), MARKER_BYTES).expect("write marker");
        fs::write(root.join("manual.md"), b"manual\n").expect("write manual document");

        assert_eq!(inspect_root(&root), Err(GeneratedDocsError::UnsafePath));
        assert_eq!(
            fs::read(root.join("manual.md")).expect("read manual document"),
            b"manual\n"
        );

        fs::remove_dir_all(root).expect("remove test root");
    }

    #[test]
    fn sec_fr_doc_003_unlisted_overview_is_not_owned() {
        let root = test_root("overview");
        fs::write(root.join("overview.md"), b"manual\n").expect("write overview");

        assert_eq!(
            validate_owned_entries(&root, &BTreeSet::new()),
            Err(GeneratedDocsError::UnsafePath)
        );
        assert_eq!(
            fs::read(root.join("overview.md")).expect("read overview"),
            b"manual\n"
        );

        fs::remove_dir_all(root).expect("remove test root");
    }

    #[test]
    fn sec_fr_doc_003_unmarked_nonempty_root_is_unchanged() {
        let root = test_root("unmarked");
        let store = root.join("store");
        let output = root.join("documents");
        fs::create_dir(&store).expect("create store");
        fs::create_dir(&output).expect("create output");
        fs::write(output.join("manual.md"), b"manual\n").expect("write manual document");

        assert_eq!(
            validate_output_root_for_generation(&store, &output),
            Err(GeneratedDocsError::UnmarkedNonemptyRoot)
        );
        assert_eq!(
            fs::read(output.join("manual.md")).expect("read manual document"),
            b"manual\n"
        );

        fs::remove_dir_all(root).expect("remove test root");
    }

    #[test]
    fn sec_fr_doc_003_complete_generation_rejects_snapshot_replacement() {
        let root = test_root("snapshot-replacement");
        let store = root.join("store");
        let output = root.join("documents");
        fs::create_dir(&store).expect("create store");
        let generated = fixture_generation();
        assert_eq!(publish(&store, &output, &generated), Ok(()));
        let original = fs::read(output.join("manifest.json")).expect("read original manifest");
        let replacement = fixture_generation_for_snapshot(
            "urn:codenoesis:snapshot:blake3:0000000000000000000000000000000000000000000000000000000000000000",
        );

        assert_eq!(
            publish(&store, &output, &replacement),
            Err(GeneratedDocsError::SnapshotMismatch)
        );
        assert_eq!(
            fs::read(output.join("manifest.json")).expect("read retained manifest"),
            original
        );

        fs::remove_dir_all(root).expect("remove test root");
    }

    #[test]
    fn sec_fr_doc_003_corrupt_complete_generation_fails_closed() {
        let root = test_root("corrupt-complete");
        let store = root.join("store");
        let output = root.join("documents");
        fs::create_dir(&store).expect("create store");
        let generated = fixture_generation();
        assert_eq!(publish(&store, &output, &generated), Ok(()));
        fs::write(output.join("overview.md"), b"corrupt\n").expect("corrupt overview");
        let (repository_identity, snapshot_id, snapshot_hash) =
            manifest_binding(generated.manifest()).expect("fixture binding");

        assert_eq!(
            load_validated_manifest(&output, repository_identity, snapshot_id, snapshot_hash,),
            Err(GeneratedDocsError::CorruptGeneration)
        );

        fs::remove_dir_all(root).expect("remove test root");
    }

    #[test]
    fn ft_fr_doc_003_abandoned_staging_is_resumed() {
        let root = test_root("abandoned-staging");
        let store = root.join("store");
        let output = root.join("documents");
        fs::create_dir(&store).expect("create store");
        fs::create_dir(&output).expect("create output");
        fs::write(output.join(".codenoesis-generated.json"), MARKER_BYTES).expect("write marker");
        let generated = fixture_generation();
        let generation = generated
            .manifest()
            .pointer("/generation_hash/value")
            .and_then(Value::as_str)
            .expect("fixture generation hash");
        let staging = output.join(".tmp").join(generation);
        fs::create_dir_all(staging.join("modules")).expect("create abandoned staging");
        let overview = generated
            .documents()
            .iter()
            .find(|document| document.path == "overview.md")
            .expect("fixture overview");
        fs::write(staging.join("overview.md"), &overview.bytes).expect("write staged overview");

        assert_eq!(publish(&store, &output, &generated), Ok(()));
        assert_generation_bytes(&output, &generated);

        fs::remove_dir_all(root).expect("remove test root");
    }

    #[test]
    fn ft_fr_doc_003_precommit_files_are_resumed_without_overwrite() {
        let root = test_root("precommit-files");
        let store = root.join("store");
        let output = root.join("documents");
        fs::create_dir(&store).expect("create store");
        fs::create_dir(&output).expect("create output");
        fs::write(output.join(".codenoesis-generated.json"), MARKER_BYTES).expect("write marker");
        let generated = fixture_generation();
        fs::create_dir(output.join("modules")).expect("create modules");
        for document in generated.documents().iter().take(2) {
            let relative = safe_document_path(&document.path).expect("safe fixture document");
            fs::write(output.join(relative), &document.bytes).expect("write precommit document");
        }

        assert_eq!(publish(&store, &output, &generated), Ok(()));
        assert_generation_bytes(&output, &generated);

        fs::remove_dir_all(root).expect("remove test root");
    }

    #[cfg(unix)]
    #[test]
    fn sec_fr_doc_003_symlink_component_is_rejected() {
        use std::os::unix::fs::symlink;

        let root = test_root("component-symlink");
        let real = root.join("real");
        fs::create_dir(&real).expect("create real root");
        let store = real.join("store");
        fs::create_dir(&store).expect("create store");
        let alias = root.join("alias");
        symlink(&real, &alias).expect("create root alias");
        let output = alias.join("documents");

        assert_eq!(
            ensure_output_root_for_boundary(&store, &output),
            Err(GeneratedDocsError::UnsafePath)
        );
        assert!(!real.join("documents").exists());
        fs::create_dir(real.join("documents")).expect("create documents");
        assert_eq!(
            validate_documents_root_for_boundary(&store, &output),
            Err(GeneratedDocsError::UnsafePath)
        );

        fs::remove_file(alias).expect("remove root alias");
        fs::remove_dir_all(root).expect("remove test root");
    }

    #[cfg(unix)]
    #[test]
    fn sec_fr_doc_003_temporary_symlink_is_rejected() {
        use std::os::unix::fs::symlink;

        let root = test_root("temporary-symlink");
        symlink(std::env::temp_dir(), root.join(".tmp")).expect("create temporary symlink");

        assert_eq!(
            validate_owned_entries(&root, &BTreeSet::new()),
            Err(GeneratedDocsError::UnsafePath)
        );

        fs::remove_file(root.join(".tmp")).expect("remove temporary symlink");
        fs::remove_dir_all(root).expect("remove test root");
    }

    fn test_root(label: &str) -> PathBuf {
        let sequence = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "codenoesis-generated-docs-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("create test root");
        root
    }

    fn fixture_generation() -> GeneratedDocumentationV1 {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/s4/workspace-docs-v1");
        let semantic = serde_json::from_slice::<Value>(
            &fs::read(fixture.join("expected-snapshot-semantic.json"))
                .expect("read fixture semantic"),
        )
        .expect("parse fixture semantic");
        let summary = serde_json::from_slice::<Value>(
            &fs::read(fixture.join("expected-graph-summary.json")).expect("read fixture summary"),
        )
        .expect("parse fixture summary");
        codenoesis_contracts::generate_documentation_v1(
            &semantic,
            summary["snapshot_id"]
                .as_str()
                .expect("fixture snapshot ID"),
            summary["semantic_hash"]
                .as_str()
                .expect("fixture semantic hash"),
        )
        .expect("generate fixture documentation")
    }

    fn fixture_generation_for_snapshot(snapshot_id: &str) -> GeneratedDocumentationV1 {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/s4/workspace-docs-v1");
        let semantic = serde_json::from_slice::<Value>(
            &fs::read(fixture.join("expected-snapshot-semantic.json"))
                .expect("read fixture semantic"),
        )
        .expect("parse fixture semantic");
        codenoesis_contracts::generate_documentation_v1(
            &semantic,
            snapshot_id,
            "6ed66dd0d5bf2451087fcb17c254048084d9d0f1bd2ea51062d46a38d1defe31",
        )
        .expect("generate fixture documentation")
    }

    fn assert_generation_bytes(root: &Path, generated: &GeneratedDocumentationV1) {
        for document in generated.documents() {
            assert_eq!(
                fs::read(root.join(&document.path)).expect("read published document"),
                document.bytes
            );
        }
        assert_eq!(
            fs::read(root.join("manifest.json")).expect("read published manifest"),
            generated
                .canonical_manifest_file()
                .expect("serialize fixture manifest")
        );
    }
}
