//! Bounded resolution in an immutable Git tree; no host path is dereferenced.

use std::collections::{BTreeMap, VecDeque};

use codenoesis_domain::{
    AcquiredSymlink, EntryPolicy, ObjectId, RepositoryError, SymlinkTargetKind,
};

use super::{S1TraversalState, check_path_limits, entry_policy_error, validate_s1_path_component};

pub(super) const MAX_TARGET_BYTES: u64 = 1_024;
const MAX_EXPANSIONS: usize = 32;

pub(super) enum Entry {
    File(ObjectId),
    Directory(ObjectId),
    Link { blob_oid: ObjectId, bytes: Vec<u8> },
    Gitlink,
}

pub(super) fn resolve_all(
    state: &S1TraversalState,
) -> Result<Vec<AcquiredSymlink>, RepositoryError> {
    let mut result = Vec::new();
    for (path, entry) in &state.link_entries {
        state.check_time()?;
        if let Entry::Link { blob_oid, bytes } = entry {
            let (resolved_target, target_oid, target_kind) = resolve(&state.link_entries, path)
                .ok_or_else(|| entry_policy_error(path.clone(), EntryPolicy::Symlink))?;
            result.push(AcquiredSymlink {
                path: path.clone(),
                blob_oid: blob_oid.clone(),
                bytes: bytes.clone(),
                resolved_target,
                target_oid,
                target_kind,
            });
        }
    }
    Ok(result)
}

fn target_components(bytes: &[u8]) -> Option<(VecDeque<String>, bool)> {
    if bytes.is_empty() || bytes.len() > usize::try_from(MAX_TARGET_BYTES).ok()? {
        return None;
    }
    let target = std::str::from_utf8(bytes).ok()?;
    if target.starts_with('/')
        || target.contains('\\')
        || target.chars().any(char::is_control)
        || target.split('/').next()?.ends_with(':')
    {
        return None;
    }
    let directory_required = target.ends_with('/');
    let mut components = VecDeque::new();
    let body = target.strip_suffix('/').unwrap_or(target);
    for component in body.split('/') {
        if component.is_empty() {
            return None;
        }
        if component != "." && component != ".." {
            validate_s1_path_component(component.as_bytes()).ok()?;
            if component.len() > 255 {
                return None;
            }
        }
        components.push_back(component.to_owned());
    }
    Some((components, directory_required))
}

fn resolve(
    entries: &BTreeMap<String, Entry>,
    path: &str,
) -> Option<(String, ObjectId, SymlinkTargetKind)> {
    let mut pending: VecDeque<String> = path.split('/').map(str::to_owned).collect();
    let mut resolved = Vec::<String>::new();
    let mut expansions = 0;
    let mut directory_required = false;
    while let Some(component) = pending.pop_front() {
        match component.as_str() {
            "." => continue,
            ".." => {
                resolved.pop()?;
                continue;
            }
            _ => (),
        }
        resolved.push(component.clone());
        let current = resolved.join("/");
        check_path_limits(
            &current,
            component.as_bytes(),
            u64::try_from(resolved.len()).ok()?,
        )
        .ok()?;
        match entries.get(&current)? {
            Entry::Link { bytes, .. } => {
                expansions += 1;
                if expansions > MAX_EXPANSIONS {
                    return None;
                }
                resolved.pop();
                let (mut target, needs_directory) = target_components(bytes)?;
                if pending.is_empty() {
                    directory_required |= needs_directory;
                }
                target.append(&mut pending);
                pending = target;
            }
            Entry::File(_) if !pending.is_empty() => return None,
            Entry::Directory(_) | Entry::File(_) => (),
            Entry::Gitlink => return None,
        }
    }
    let target = resolved.join("/");
    match entries.get(&target)? {
        Entry::File(oid) if !directory_required => {
            Some((target, oid.clone(), SymlinkTargetKind::File))
        }
        Entry::Directory(oid) => Some((target, oid.clone(), SymlinkTargetKind::Directory)),
        _ => None,
    }
}
