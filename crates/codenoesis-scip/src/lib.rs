//! Bounded static SCIP import for the approved R7 profile.

mod acquire;
mod binding;
mod normalize;
mod wire;

use std::path::{Path, PathBuf};

use codenoesis_domain::RepositoryInventory;
use codenoesis_domain::s4_r6::FrameworkKnowledge;
use codenoesis_domain::s4_r7::{CompilerIndexError, CompilerIndexOverlay};
use codenoesis_ports::CompilerIndexImporter;

/// Imports one explicitly selected, revision-bound SCIP sidecar pair.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticScipImporter {
    binding_path: PathBuf,
    artifact_path: PathBuf,
}

impl StaticScipImporter {
    #[must_use]
    pub fn new(binding_path: impl AsRef<Path>, artifact_path: impl AsRef<Path>) -> Self {
        Self {
            binding_path: binding_path.as_ref().to_path_buf(),
            artifact_path: artifact_path.as_ref().to_path_buf(),
        }
    }
}

impl CompilerIndexImporter for StaticScipImporter {
    fn import_compiler_index(
        &self,
        inventory: &RepositoryInventory,
        source: &FrameworkKnowledge,
    ) -> Result<CompilerIndexOverlay, CompilerIndexError> {
        let acquired = acquire::acquire_pair(&self.binding_path)?;
        if Path::new(&acquired.artifact.path) != self.artifact_path {
            return Err(CompilerIndexError::UnsafePath {
                path: acquired.artifact.path,
                reason: "artifact_path_changed".to_owned(),
            });
        }
        let binding = binding::parse_and_validate_binding(
            &acquired.binding.path,
            &acquired.binding.bytes,
            &acquired.binding.sha256,
            inventory,
        )?;
        if acquired.artifact.path != binding.artifact_path {
            return Err(CompilerIndexError::UnsafePath {
                path: acquired.artifact.path,
                reason: "artifact_path_changed".to_owned(),
            });
        }
        binding::validate_artifact_binding(
            &binding,
            acquired.artifact.bytes.len(),
            &acquired.artifact.sha256,
        )?;
        wire::preflight(&acquired.artifact.bytes, &acquired.artifact.sha256)?;
        let index =
            normalize::decode_canonical(&acquired.artifact.bytes, &acquired.artifact.sha256)?;
        normalize::normalize(&index, binding, inventory, source)
    }
}
