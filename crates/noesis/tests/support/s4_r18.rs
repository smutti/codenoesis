use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

use crate::support::s4_r17::MaterializedFunctionContextRepository;

pub const SOURCE_PROFILE: &str = "trusted-local-source-v1";
pub const SIGNATURE_EVIDENCE_ID: &str = "urn:codenoesis:evidence:blake3:3025c0ba243c210e5a923781cf1a57c420e36d393c24b358c1f965594c3002c8";

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
        self.source_command(evidence_id)
            .output()
            .expect("launch R18 trusted source retrieval")
    }

    pub fn source_packed(&self, evidence_id: &str) -> Output {
        self.source_command(evidence_id)
            .args([
                "--acquisition-profile",
                codenoesis_domain::s1_packed::LOCAL_GIT_SHA1_PACKED_V1,
            ])
            .output()
            .expect("launch packed R18 trusted source retrieval")
    }

    pub fn source_with_boundaries(&self, evidence_id: &str) -> Output {
        self.source_command(evidence_id)
            .args([
                "--repository-boundary-profile",
                codenoesis_domain::s1_boundaries::LOCAL_GITLINKS_V1,
            ])
            .output()
            .expect("launch boundary-aware R18 trusted source retrieval")
    }

    pub fn source_command(&self, evidence_id: &str) -> Command {
        self.source_command_at(
            &self.inherited.worktree,
            &self.inherited.commit_oid,
            crate::support::s4_r17::REPOSITORY_ID,
            &self.inherited.store,
            evidence_id,
        )
    }

    pub fn source_command_at(
        &self,
        repository: &Path,
        revision: &str,
        repository_id: &str,
        store: &Path,
        evidence_id: &str,
    ) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_noesis"));
        command
            .current_dir(&self.inherited.root)
            .args(["source", "--repository"])
            .arg(repository)
            .args(["--revision", revision, "--repository-id", repository_id])
            .arg("--store")
            .arg(store)
            .args([
                "--evidence-id",
                evidence_id,
                "--source-profile",
                SOURCE_PROFILE,
                "--format",
                "json",
            ]);
        command
    }
}

pub fn expected_source_excerpt() -> Value {
    serde_json::from_slice(
        &fs::read(fixture_root().join("expected-source-excerpt.json"))
            .expect("read R18 expected source excerpt"),
    )
    .expect("parse R18 expected source excerpt")
}

pub fn expected_source_stdout() -> Vec<u8> {
    let mut bytes =
        serde_json::to_vec(&expected_source_excerpt()).expect("canonical R18 source excerpt");
    bytes.push(b'\n');
    bytes
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/s4/trusted-source-retrieval-v1")
}
