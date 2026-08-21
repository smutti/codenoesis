use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde_json::{Value, json};

use crate::support::{repository_root, unique_temp_root};

pub const PROVIDER_ID: &str = "urn:codenoesis:fixture:s7-provider";
pub const PROVIDER_BASELINE: &str = "73cc0752413bd337a6507ffcc422d7d5a4458523";
pub const PROVIDER_TARGET: &str = "fd6c8a3b1988e6a963a46824da09ec6132cf0290";
pub const DECOY_ID: &str = "urn:codenoesis:fixture:s7-client-decoy";
pub const DECOY_COMMIT: &str = "4c878122e919f211a953440732a2fed8f100df9f";
pub const SAFE_ID: &str = "urn:codenoesis:fixture:s7-client-safe";
pub const SAFE_COMMIT: &str = "d22bdc44624becac8459ccfa78874f245dd390b5";
pub const STRICT_ID: &str = "urn:codenoesis:fixture:s7-client-strict";
pub const STRICT_COMMIT: &str = "51a50dbf08ee9c90a222455a6a8fde1baa812b7d";
pub const ANALYSIS_PROFILE: &str = "implementation-aware-http-json-git-v1";
pub const SOURCE_PROFILE: &str = "trusted-local-impact-source-v1";

pub struct MaterializedGitImpactWorkspace {
    root: PathBuf,
    pub manifest: PathBuf,
}

impl MaterializedGitImpactWorkspace {
    pub fn reviewed() -> Self {
        let root = unique_temp_root();
        let global_config = root.join("global.gitconfig");
        let template = root.join("template");
        fs::write(&global_config, []).expect("create empty R19 Git configuration");
        fs::create_dir(&template).expect("create empty R19 Git template");

        let fixture = repository_root().join("tests/fixtures/s7/implementation-aware-api-v1");
        let provider = root.join("provider");
        init_repository(&provider, &global_config, &template);
        copy_file(
            &fixture.join("provider/revision-a/openapi.yaml"),
            &provider.join("openapi.yaml"),
        );
        copy_file(
            &fixture.join("provider/revision-a/src/user_response.rs"),
            &provider.join("src/user_response.rs"),
        );
        commit(
            &provider,
            &global_config,
            &["openapi.yaml", "src/user_response.rs"],
            "fixture: provider baseline",
            "946684800 +0000",
            PROVIDER_BASELINE,
        );
        copy_file(
            &fixture.join("provider/revision-b/src/user_response.rs"),
            &provider.join("src/user_response.rs"),
        );
        commit(
            &provider,
            &global_config,
            &["src/user_response.rs"],
            "fixture: provider target",
            "946684801 +0000",
            PROVIDER_TARGET,
        );

        materialize_client(
            &root,
            &fixture,
            &global_config,
            &template,
            "decoy",
            "DecoyAccountClient.kt",
            DECOY_COMMIT,
        );
        materialize_client(
            &root,
            &fixture,
            &global_config,
            &template,
            "safe",
            "SafeUsersClient.kt",
            SAFE_COMMIT,
        );
        materialize_client(
            &root,
            &fixture,
            &global_config,
            &template,
            "strict",
            "StrictUsersClient.kt",
            STRICT_COMMIT,
        );

        copy_file(
            &repository_root()
                .join("tests/fixtures/s6/openapi-federation-v1/expected-federation-report.json"),
            &root.join("federation-report.json"),
        );
        let manifest = root.join("impact-git-workspace.json");
        fs::write(
            &manifest,
            serde_json::to_vec(&workspace_value()).expect("serialize R19 workspace"),
        )
        .expect("write R19 workspace");

        Self { root, manifest }
    }

    pub fn impact(&self) -> Output {
        self.impact_command()
            .output()
            .expect("launch R19 Git-backed impact")
    }

    pub fn impact_command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_noesis"));
        command
            .current_dir(&self.root)
            .args(["impact", "--workspace"])
            .arg(&self.manifest)
            .args(["--profile", ANALYSIS_PROFILE, "--format", "json"]);
        command
    }

    pub fn write_report(&self, bytes: &[u8]) -> PathBuf {
        let report = self.root.join("semantic-compatibility-report-v2.json");
        fs::write(&report, bytes).expect("write transient R19 report");
        report
    }

    pub fn impact_source(&self, report: &Path, evidence: &Value) -> Output {
        let repository_id = evidence["repository_identity"]
            .as_str()
            .expect("R19 evidence repository identity");
        let revision = evidence["revision"]
            .as_str()
            .expect("R19 evidence revision");
        let evidence_id = evidence["id"].as_str().expect("R19 evidence ID");
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .current_dir(&self.root)
            .args(["impact-source", "--repository"])
            .arg(self.repository_for(repository_id))
            .args(["--repository-id", repository_id, "--revision", revision])
            .arg("--report")
            .arg(report)
            .args([
                "--evidence-id",
                evidence_id,
                "--source-profile",
                SOURCE_PROFILE,
                "--format",
                "json",
            ])
            .output()
            .expect("launch R19 impact source")
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn repository_for(&self, repository_id: &str) -> PathBuf {
        match repository_id {
            PROVIDER_ID => self.root.join("provider"),
            DECOY_ID => self.root.join("decoy"),
            SAFE_ID => self.root.join("safe"),
            STRICT_ID => self.root.join("strict"),
            _ => panic!("unknown R19 fixture repository identity"),
        }
    }
}

impl Drop for MaterializedGitImpactWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn workspace_value() -> Value {
    json!({
        "schema_version": "codenoesis.impact-git-workspace/v1",
        "analysis_profile": ANALYSIS_PROFILE,
        "pipeline": "codenoesis.pipeline/s7-git-v1",
        "contract_capability": "codenoesis.contract-capability/openapi-3.1-http-json/v1",
        "provider_capability": "rust-direct-json-map/v1",
        "client_capability": "kotlin-direct-json-access/v1",
        "provider": {
            "repository_identity": PROVIDER_ID,
            "root": "provider",
            "baseline": {
                "revision": PROVIDER_BASELINE,
                "federation_revision": "fixture-provider-a",
                "contract_path": "openapi.yaml",
                "source_path": "src/user_response.rs",
                "callable_symbol": "user_response"
            },
            "target": {
                "revision": PROVIDER_TARGET,
                "federation_revision": "fixture-provider-a",
                "contract_path": "openapi.yaml",
                "source_path": "src/user_response.rs",
                "callable_symbol": "user_response"
            }
        },
        "clients": [
            client("decoy", DECOY_ID, DECOY_COMMIT, "DecoyAccountClient.kt", "decodeAccount", "getAccount"),
            client("safe", SAFE_ID, SAFE_COMMIT, "SafeUsersClient.kt", "decodeSafeUser", "getSafeUser"),
            client("strict", STRICT_ID, STRICT_COMMIT, "StrictUsersClient.kt", "decodeStrictUser", "getStrictUser")
        ],
        "federation_report": {
            "path": "federation-report.json",
            "sha256": "7a301b0eb5d0e5e702a647422e57f42e1611d9e1cac5aedb39be26c7f1064628"
        }
    })
}

fn client(
    role: &str,
    repository_identity: &str,
    revision: &str,
    file: &str,
    decoder_symbol: &str,
    call_symbol: &str,
) -> Value {
    json!({
        "role": role,
        "repository_identity": repository_identity,
        "root": role,
        "revision": revision,
        "federation_revision": "fixture-client-v1",
        "source_path": format!("src/commonMain/kotlin/dev/codenoesis/fixture/{file}"),
        "decoder_symbol": decoder_symbol,
        "call_symbol": call_symbol
    })
}

fn materialize_client(
    root: &Path,
    fixture: &Path,
    global_config: &Path,
    template: &Path,
    role: &str,
    file: &str,
    expected_commit: &str,
) {
    let repository = root.join(role);
    init_repository(&repository, global_config, template);
    let relative = format!("src/commonMain/kotlin/dev/codenoesis/fixture/{file}");
    copy_file(
        &fixture.join("clients").join(role).join(&relative),
        &repository.join(&relative),
    );
    commit(
        &repository,
        global_config,
        &[&relative],
        &format!("fixture: {role} client"),
        "946684802 +0000",
        expected_commit,
    );
}

fn init_repository(repository: &Path, global_config: &Path, template: &Path) {
    let mut init = git_command(global_config);
    init.args(["init", "--quiet", "--initial-branch=main"])
        .arg(format!("--template={}", template.display()))
        .arg(repository);
    successful_output(init, None);
}

fn commit(
    repository: &Path,
    global_config: &Path,
    paths: &[&str],
    message: &str,
    timestamp: &str,
    expected_commit: &str,
) {
    let mut add = git_command(global_config);
    add.arg("-C").arg(repository).arg("add").args(paths);
    successful_output(add, None);

    let mut commit = git_command(global_config);
    commit
        .arg("-C")
        .arg(repository)
        .args(["commit", "--quiet", "--message", message])
        .env("GIT_AUTHOR_NAME", "CodeNoesis Fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@codenoesis.invalid")
        .env("GIT_AUTHOR_DATE", timestamp)
        .env("GIT_COMMITTER_NAME", "CodeNoesis Fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@codenoesis.invalid")
        .env("GIT_COMMITTER_DATE", timestamp);
    successful_output(commit, None);

    let mut revision = git_command(global_config);
    revision
        .arg("-C")
        .arg(repository)
        .args(["rev-parse", "HEAD"]);
    let output = successful_output(revision, None);
    assert_eq!(
        String::from_utf8(output.stdout)
            .expect("R19 revision output")
            .trim(),
        expected_commit,
        "R19 fixture commit identity changed"
    );
}

fn copy_file(source: &Path, target: &Path) {
    fs::create_dir_all(target.parent().expect("R19 target parent"))
        .expect("create R19 target directory");
    fs::write(
        target,
        fs::read(source)
            .unwrap_or_else(|error| panic!("read R19 fixture {}: {error}", source.display())),
    )
    .unwrap_or_else(|error| panic!("write R19 fixture {}: {error}", target.display()));
}

fn git_command(global_config: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", global_config)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE");
    command
}

fn successful_output(mut command: Command, input: Option<&[u8]>) -> Output {
    let invocation = format!("{command:?}");
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    if input.is_some() {
        command.stdin(Stdio::piped());
    }
    let mut child = command.spawn().expect("launch R19 fixture Git command");
    if let Some(content) = input {
        child
            .stdin
            .take()
            .expect("R19 fixture Git command stdin")
            .write_all(content)
            .expect("write R19 fixture Git command stdin");
    }
    let output = child
        .wait_with_output()
        .expect("wait for R19 fixture Git command");
    assert!(
        output.status.success(),
        "R19 fixture command failed: {invocation}; stdout={:?}; stderr={:?}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}
