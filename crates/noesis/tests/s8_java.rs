mod support;

use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
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
        let root = canonical_temp_root();
        let repository = root.join("repository");
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/s8/java-declarations-v1/repository");
        copy_fixture(&source, &repository);
        let git = |args: &[&str]| {
            let output = Command::new("git")
                .current_dir(&repository)
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", root.join("absent-config"))
                .env("GIT_AUTHOR_DATE", "2026-09-07T00:00:00Z")
                .env("GIT_COMMITTER_DATE", "2026-09-07T00:00:00Z")
                .args([
                    "-c",
                    "user.name=Java Fixture",
                    "-c",
                    "user.email=fixture@example.invalid",
                    "-c",
                    "commit.gpgsign=false",
                    "-c",
                    "core.hooksPath=/dev/null",
                    "-c",
                    "core.autocrlf=false",
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
    fn run(&self, command: &str, extra: &[&str]) -> Output {
        let mut invocation = Command::new(env!("CARGO_BIN_EXE_noesis"));
        invocation
            .current_dir(self.root.as_os_str())
            .arg(command)
            .args(["--java-profile", "java-declarations-v1"]);
        if command != "explore" {
            invocation.arg("--store").arg(&self.store).args([
                "--repository-identity",
                "urn:codenoesis:repository:java-fixture",
            ]);
        }
        if command == "scan" {
            invocation
                .args(["--profile", "standard-local-s8", "--repository"])
                .arg(&self.repository)
                .args(["--revision", &self.commit]);
        }
        invocation.args(extra).output().unwrap()
    }
}
fn copy_fixture(source: &Path, target: &Path) {
    fs::create_dir(target).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let destination = target.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_fixture(&entry.path(), &destination);
        } else {
            // Preserve a canonical fixture commit even under Windows CRLF checkout.
            let source = fs::read_to_string(entry.path())
                .unwrap()
                .replace("\r\n", "\n");
            fs::write(destination, source).unwrap();
        }
    }
}
fn success(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    serde_json::from_slice(&output.stdout).unwrap()
}
fn assert_oracle(graph: &Value) {
    let oracle: Value = serde_json::from_str(include_str!(
        "../../../tests/specifications/s8/java-declarations-v1.json"
    ))
    .unwrap();
    let mut expected: Vec<_> = oracle["declarations"]
        .as_array()
        .unwrap()
        .iter()
        .map(Value::to_string)
        .collect();
    let mut observed: Vec<_> = graph["entities"].as_array().unwrap().iter().filter(|row| row["properties"]["owner"].is_string()).map(|row| serde_json::json!({"path":row["properties"]["path"],"kind":row["kind"],"name":row["name"],"owner":row["properties"]["owner"]}).to_string()).collect();
    expected.sort();
    observed.sort();
    assert_eq!(observed, expected);
    let edges = graph["relationships"].as_array().unwrap();
    assert_eq!(
        edges
            .iter()
            .filter(|e| e["kind"] == "CONTAINS_DECLARATION")
            .count(),
        usize::try_from(oracle["nested_declaration_edges"].as_u64().unwrap()).unwrap()
    );
    assert!(edges.iter().all(|e| e["state"] == "Observed"));
    assert!(
        graph["claims"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["state"] == "deterministic_fact")
    );
    for gap in oracle["gap_reasons"].as_array().unwrap() {
        assert!(
            graph["coverage"]
                .as_array()
                .unwrap()
                .iter()
                .any(|g| &g["reason"] == gap),
            "missing gap {gap}"
        );
    }
    for signature in oracle["overload_signatures"].as_array().unwrap() {
        assert!(
            graph["entities"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| &e["properties"]["signature"] == signature)
        );
    }
}

#[test]
fn e2e_fr_ext_026_scan_store_query_docs_export_and_offline_viewer() {
    let fixture = Fixture::new();
    let snapshot = success(&fixture.run("scan", &[]));
    assert_eq!(
        snapshot["schema_version"],
        "codenoesis.repository-snapshot/v20"
    );
    assert_oracle(&snapshot["semantic"]["knowledge_graph"]);
    fs::write(
        fixture
            .repository
            .join("src/main/java/example/Library.java"),
        "class Changed {}\n",
    )
    .unwrap();
    let repeat = success(&fixture.run("scan", &[]));
    assert_eq!(snapshot["semantic_hash"], repeat["semantic_hash"]);
    let query = success(&fixture.run("query", &["--search", "overloaded"]));
    assert_eq!(query["total_matches"], 2);
    let docs = success(&fixture.run("docs", &[]));
    assert_eq!(
        docs["entities"],
        snapshot["semantic"]["knowledge_graph"]["entities"]
    );
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
    assert!(html.contains("Content-Security-Policy") && html.contains("Library"));
    assert!(!html.contains('\r'));
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
    assert!(!result.status.success() && result.stdout.is_empty() && !rejected.exists());
}

#[test]
fn sec_fr_ext_026_selector_conflicts_fail_before_store_creation() {
    let fixture = Fixture::new();
    for extra in [
        &["--kotlin-profile", "kotlin-kmp-declarations-v1"][..],
        &["--java-profile", "java-declarations-v1"][..],
        &["--rust-flow-profile", "rust-local-flow-v1"][..],
    ] {
        let output = fixture.run("scan", extra);
        assert!(!output.status.success() && output.stdout.is_empty() && !fixture.store.exists());
        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["schema_version"], "codenoesis.java-error/v1");
    }
}

#[test]
fn sec_fr_ext_026_export_preserves_unowned_output() {
    let fixture = Fixture::new();
    success(&fixture.run("scan", &[]));
    let output = fixture.root.join("unowned");
    fs::create_dir(&output).unwrap();
    fs::write(output.join("keep.txt"), "user content").unwrap();
    let result = fixture.run("export", &["--output", output.to_str().unwrap()]);
    assert!(!result.status.success() && result.stdout.is_empty());
    assert_eq!(
        fs::read_to_string(output.join("keep.txt")).unwrap(),
        "user content"
    );
    assert_eq!(fs::read_dir(&output).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn sec_fr_ext_026_export_rejects_symlink_output() {
    let fixture = Fixture::new();
    success(&fixture.run("scan", &[]));
    let outside = fixture.root.join("outside");
    fs::create_dir(&outside).unwrap();
    let output = fixture.root.join("linked");
    std::os::unix::fs::symlink(&outside, &output).unwrap();
    let result = fixture.run("export", &["--output", output.to_str().unwrap()]);
    assert!(!result.status.success() && result.stdout.is_empty());
    assert_eq!(fs::read_dir(outside).unwrap().count(), 0);
}

#[cfg(windows)]
#[test]
fn sec_fr_ext_026_export_rejects_windows_verbatim_output() {
    let fixture = Fixture::new();
    success(&fixture.run("scan", &[]));
    let output = fs::canonicalize(&*fixture.root)
        .unwrap()
        .join("verbatim-output");
    let result = fixture.run("export", &["--output", output.to_str().unwrap()]);
    assert!(!result.status.success() && result.stdout.is_empty() && !output.exists());
    let error: Value = serde_json::from_slice(&result.stderr).unwrap();
    assert_eq!(error["code"], "artifact.invalid_or_unsafe_java_projection");
}

#[test]
fn sec_fr_ext_026_invalid_utf8_and_missing_java_do_not_publish_a_store() {
    for invalid_encoding in [true, false] {
        let mut fixture = Fixture::new();
        let git = |args: &[&str]| {
            let output = Command::new("git")
                .current_dir(&fixture.repository)
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", fixture.root.join("absent-config"))
                .args([
                    "-c",
                    "user.name=Java Fixture",
                    "-c",
                    "user.email=fixture@example.invalid",
                    "-c",
                    "commit.gpgsign=false",
                    "-c",
                    "core.hooksPath=/dev/null",
                    "-c",
                    "core.autocrlf=false",
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
        if invalid_encoding {
            fs::write(
                fixture
                    .repository
                    .join("src/main/java/example/Library.java"),
                [0xff],
            )
            .unwrap();
            git(&["add", "."]);
        } else {
            git(&["rm", "--", "*.java"]);
        }
        git(&["commit", "-m", "invalid fixture"]);
        fixture.commit = String::from_utf8(git(&["rev-parse", "HEAD"]).stdout)
            .unwrap()
            .trim()
            .to_owned();
        let output = fixture.run("scan", &[]);
        assert!(!output.status.success() && output.stdout.is_empty() && !fixture.store.exists());
        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(
            error["code"],
            if invalid_encoding {
                "extraction.java_invalid_utf8"
            } else {
                "extraction.no_java_sources"
            }
        );
    }
}
