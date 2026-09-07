mod support;

use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
};
use support::s4_r8::{R8TestRoot, canonical_temp_root};

struct Fixture {
    root: R8TestRoot,
    repository: PathBuf,
    store: PathBuf,
    commit: String,
}
impl Fixture {
    fn new() -> Self {
        // Use the same validated authority as the inherited output contract.
        // Windows canonicalization introduces a verbatim path outside that profile.
        let root = canonical_temp_root();
        let repository = root.join("repository");
        fs::create_dir(&repository).unwrap();
        let files = [
            ("settings.gradle.kts", "include(\":shared\", \":decoy\")\n"),
            (
                "shared/build.gradle.kts",
                "plugins { kotlin(\"multiplatform\") }\n",
            ),
            ("decoy/build.gradle.kts", "plugins { kotlin(\"jvm\") }\n"),
            (
                "shared/src/commonMain/kotlin/demo/Common.kt",
                "package demo\nexpect class Platform\nexpect fun platformName(): String\nexpect fun overloaded(x: Int): String\nexpect fun overloaded(x: String): String\ndata class Greeting(val text: String)\n",
            ),
            (
                "shared/src/jvmMain/kotlin/demo/Jvm.kt",
                "package demo\nactual class Platform\nactual fun platformName(): String = \"JVM\"\nactual fun overloaded(x: Int): String = \"no candidate\"\n",
            ),
            (
                "decoy/src/jvmMain/kotlin/demo/Decoy.kt",
                "package demo\nactual fun platformName(): String = \"decoy\"\n",
            ),
            ("custom/Extra.kt", "package extra\nobject Extra\n"),
            ("shared/src/commonMain/kotlin/demo/Broken.kt", "fun (\n"),
        ];
        for (path, content) in files {
            let path = repository.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }
        let git = |args: &[&str]| {
            let output = Command::new("git")
                .current_dir(&repository)
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", root.join("absent-config"))
                .env("GIT_AUTHOR_DATE", "2026-09-07T00:00:00Z")
                .env("GIT_COMMITTER_DATE", "2026-09-07T00:00:00Z")
                .args([
                    "-c",
                    "user.name=Kotlin Fixture",
                    "-c",
                    "user.email=fixture@example.invalid",
                    "-c",
                    "commit.gpgsign=false",
                    "-c",
                    "core.hooksPath=/dev/null",
                ])
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            output
        };
        git(&["init", "--template=", "--object-format=sha1"]);
        git(&["add", "."]);
        git(&["commit", "-m", "fixture"]);
        let commit = String::from_utf8(git(&["rev-parse", "HEAD"]).stdout)
            .unwrap()
            .trim()
            .to_owned();
        Self {
            store: root.join("store"),
            root,
            repository,
            commit,
        }
    }
    fn run(&self, command: &str, args: &[&str]) -> Output {
        let mut invocation = Command::new(env!("CARGO_BIN_EXE_noesis"));
        invocation
            .current_dir(self.root.as_os_str())
            .arg(command)
            .args(["--kotlin-profile", "kotlin-kmp-declarations-v1"]);
        if command != "explore" {
            invocation.arg("--store").arg(&self.store).args([
                "--repository-identity",
                "urn:codenoesis:repository:kotlin-fixture",
            ]);
        }
        if command == "scan" {
            invocation
                .args(["--profile", "standard-local-s8", "--repository"])
                .arg(&self.repository)
                .args(["--revision", &self.commit]);
        }
        invocation.args(args).output().unwrap()
    }
}
fn success(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[cfg(windows)]
#[test]
fn sec_fr_ext_025_export_rejects_windows_verbatim_output() {
    let fixture = Fixture::new();
    success(&fixture.run("scan", &[]));
    let root = fs::canonicalize(&*fixture.root).unwrap();
    assert!(matches!(
        root.components().next(),
        Some(std::path::Component::Prefix(prefix))
            if matches!(prefix.kind(), std::path::Prefix::VerbatimDisk(_))
    ));
    let output = root.join("verbatim-output");
    let result = fixture.run("export", &["--output", output.to_str().unwrap()]);
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(!output.exists());
    let error: Value = serde_json::from_slice(&result.stderr).unwrap();
    assert_eq!(error["schema_version"], "codenoesis.kotlin-error/v1");
    assert_eq!(
        error["code"],
        "artifact.invalid_or_unsafe_kotlin_projection"
    );
}

#[test]
fn e2e_fr_ext_025_scan_store_query_export_offline_viewer() {
    let fixture = Fixture::new();
    let snapshot = success(&fixture.run("scan", &[]));
    assert_eq!(
        snapshot["schema_version"],
        "codenoesis.repository-snapshot/v19"
    );
    let graph = &snapshot["semantic"]["knowledge_graph"];
    assert_fixture_oracle(graph);
    let candidates: Vec<_> = graph["relationships"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["kind"] == "EXPECT_ACTUAL_CANDIDATE")
        .collect();
    assert_eq!(candidates.len(), 2);
    assert!(candidates.iter().all(|e| e["state"] == "Unknown"));
    let gaps = graph["coverage"].as_array().unwrap();
    for reason in [
        "unassigned_source_root",
        "kotlin_syntax_not_accepted_by_pinned_parser",
        "actual_candidate_missing_ambiguous_or_unsupported",
    ] {
        assert!(
            gaps.iter().any(|g| g["reason"] == reason),
            "missing {reason}"
        );
    }
    fs::write(
        fixture.repository.join("custom/Extra.kt"),
        "fun uncommitted() {}\n",
    )
    .unwrap();
    let repeated = success(&fixture.run("scan", &[]));
    assert_eq!(snapshot["semantic_hash"], repeated["semantic_hash"]);
    let query = success(&fixture.run("query", &["--search", "Greeting"]));
    assert_eq!(query["results"].as_array().unwrap().len(), 1);
    let output = fixture.root.join("export");
    let portable = success(&fixture.run("export", &["--output", output.to_str().unwrap()]));
    assert_eq!(portable["payload"]["semantic"], snapshot["semantic"]);
    let input = output.join("portable-graph.json");
    let viewer = fixture.root.join("viewer");
    success(&fixture.run(
        "explore",
        &[
            "--input",
            input.to_str().unwrap(),
            "--output",
            viewer.to_str().unwrap(),
        ],
    ));
    let html = fs::read_to_string(viewer.join("index.html")).unwrap();
    assert!(!html.contains('\r'));
    assert!(html.contains("Content-Security-Policy"));
    assert!(html.contains("Greeting"));
    let mut corrupt = fs::read(&input).unwrap();
    corrupt[0] = b'[';
    fs::write(&input, corrupt).unwrap();
    let rejected = fixture.root.join("rejected");
    let result = fixture.run(
        "explore",
        &[
            "--input",
            input.to_str().unwrap(),
            "--output",
            rejected.to_str().unwrap(),
        ],
    );
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(!rejected.exists());
}

#[test]
fn e2e_fr_ext_025_invalid_selector_has_typed_error_without_store() {
    let fixture = Fixture::new();
    let output = fixture.run("scan", &["--rust-flow-profile", "rust-local-flow-v1"]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!fixture.store.exists());
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["schema_version"], "codenoesis.kotlin-error/v1");
}

#[test]
fn e2e_fr_ext_025_export_preserves_unowned_output() {
    let fixture = Fixture::new();
    success(&fixture.run("scan", &[]));
    let output = fixture.root.join("unowned");
    fs::create_dir(&output).unwrap();
    fs::write(output.join("keep.txt"), "user content").unwrap();
    let result = fixture.run("export", &["--output", output.to_str().unwrap()]);
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert_eq!(
        fs::read_to_string(output.join("keep.txt")).unwrap(),
        "user content"
    );
    assert_eq!(fs::read_dir(output).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn e2e_fr_ext_025_export_rejects_symlink_output() {
    let fixture = Fixture::new();
    success(&fixture.run("scan", &[]));
    let destination = fixture.root.join("destination");
    fs::create_dir(&destination).unwrap();
    let link = fixture.root.join("link");
    std::os::unix::fs::symlink(&destination, &link).unwrap();
    let result = fixture.run("export", &["--output", link.to_str().unwrap()]);
    assert!(!result.status.success());
    assert_eq!(fs::read_dir(destination).unwrap().count(), 0);
}

fn assert_fixture_oracle(graph: &Value) {
    let oracle: Value = serde_json::from_str(include_str!(
        "../../../tests/specifications/s8/kotlin-kmp-declarations-v1.json"
    ))
    .unwrap();
    let mut actual: Vec<_> = graph["entities"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| {
            matches!(
                e["kind"].as_str(),
                Some(
                    "KotlinClass"
                        | "KotlinInterface"
                        | "KotlinObject"
                        | "KotlinFunction"
                        | "KotlinProperty"
                        | "KotlinTypeAlias"
                )
            )
        })
        .map(|e| {
            serde_json::json!([
                e["properties"]["path"],
                e["kind"],
                e["name"],
                e["properties"]["owner"]
            ])
        })
        .collect();
    actual.sort_by_key(Value::to_string);
    let mut expected = oracle["declarations"].as_array().unwrap().clone();
    expected.sort_by_key(Value::to_string);
    assert_eq!(
        actual, expected,
        "hand-authored declaration precision and recall"
    );
}
