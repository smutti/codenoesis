use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

use super::s4_r17::MaterializedFunctionContextRepository;

pub const SOURCE_PROFILE: &str = "trusted-local-source-v1";
pub const SIGNATURE_EVIDENCE_ID: &str =
    "urn:codenoesis:evidence:blake3:3025c0ba243c210e5a923781cf1a57c420e36d393c24b358c1f965594c3002c8";

pub struct MaterializedTrustedSourceRepository {
    pub inherited: MaterializedFunctionContextRepository,
}

impl MaterializedTrustedSourceRepository {
    pub fn fixture() -> Self {
        Self {
            inherited: MaterializedFunctionContextRepository::fixture(),
        }
    }

    pub fn source(&self, evidence_id: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .current_dir(&self.inherited.root)
            .args(["source", "--repository"])
            .arg(&self.inherited.worktree)
            .args(["--revision", &self.inherited.commit_oid, "--repository-id"])
            .arg(super::s4_r17::REPOSITORY_ID)
            .arg("--store")
            .arg(&self.inherited.store)
            .args([
                "--evidence-id",
                evidence_id,
                "--source-profile",
                SOURCE_PROFILE,
                "--format",
                "json",
            ])
            .output()
            .expect("launch R18 trusted source retrieval")
    }
}

pub fn expected_source_excerpt() -> Value {
    serde_json::from_slice(
        &fs::read(fixture_root().join("expected-source-excerpt.json"))
            .expect("read R18 expected source excerpt"),
    )
    .expect("parse R18 expected source excerpt")
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/s4/trusted-source-retrieval-v1")
}
