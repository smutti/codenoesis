mod support;

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use codenoesis_contracts::{
    CodeNoesisErrorV15, LocalExplorerManifestV1, MAX_R8_JSON_NESTING,
    MAX_R8_NEIGHBORHOOD_RELATIONSHIPS, MAX_R8_NEIGHBORHOOD_SUBJECTS, MAX_R8_PORTABLE_GRAPH_BYTES,
    MAX_R8_TEXT_SEARCH_RESULTS, MAX_R8_TRAVERSAL_DEPTH, MAX_R8_VIEWER_NON_DATA_BYTES,
    PortableGraphV1, R8_DETERMINISM_PERMUTATIONS, R8_TRAVERSAL_DEPTH_DEFAULT, R8ContractError,
    validate_r8_view_limits, validate_r8_viewer_asset,
};
use serde_json::Value;

use support::s4_r7::{MaterializedCompilerIndexRepository, REPOSITORY_ID};
use support::s4_r8::{
    PORTABLE_CANONICAL_SHA256, PORTABLE_FILE_SHA256, VIEWER_SHA256, canonical_temp_root,
    corrupt_visible_snapshot_semantic, existing_test_path, explorer_manifest_bytes,
    family_digest_oracle, invalid_case_expectations, materialized_repository, portable_bytes,
    portable_value, reviewed_checkout_text, viewer_bytes, write_portable_input,
};

#[test]
fn e2e_fr_exp_001_export_and_explore_offline() {
    let repository = materialized_repository();
    let scan = repository.scan();
    assert_success(&scan, "R8 source V10 scan");

    let canonical_root = existing_test_path(&repository.root);
    let portable_root = canonical_root.join("portable-graph");
    let explorer_root = canonical_root.join("local-explorer");
    let export = export(&repository, &portable_root);

    assert_success(&export, "R8 portable graph export");
    assert!(export.stderr.is_empty());
    let portable_path = portable_root.join("portable-graph.json");
    let portable_bytes = fs::read(&portable_path).expect("read PortableGraphV1 output");
    assert_eq!(export.stdout, portable_bytes, "export stdout/file drift");
    let portable: Value =
        serde_json::from_slice(&portable_bytes).expect("parse PortableGraphV1 output");
    assert_eq!(portable["schema_version"], "codenoesis.portable-graph/v1");
    assert_eq!(
        portable["source_snapshot"]["schema_version"],
        "codenoesis.repository-snapshot/v10"
    );
    assert_eq!(portable["repository"]["identity"], REPOSITORY_ID);
    assert!(
        portable_root
            .join(".codenoesis-portable-graph-v1")
            .is_file()
    );

    let explore = explore(&portable_path, &explorer_root);
    assert_success(&explore, "R8 local explorer generation");
    assert!(explore.stderr.is_empty());
    let manifest: Value =
        serde_json::from_slice(&explore.stdout).expect("parse LocalExplorerV1 manifest");
    assert_eq!(manifest["schema_version"], "codenoesis.local-explorer/v1");
    assert_eq!(
        fs::read(explorer_root.join("explorer-manifest.json"))
            .expect("read LocalExplorerV1 manifest file"),
        explore.stdout
    );
    assert_eq!(
        fs::read(explorer_root.join("portable-graph.json")).expect("read explorer portable graph"),
        portable_bytes
    );
    assert!(explorer_root.join("index.html").is_file());
    assert!(
        explorer_root
            .join(".codenoesis-local-explorer-v1")
            .is_file()
    );
    assert!(!repository.build_sentinel().exists());
    assert!(!repository.indexer_sentinel().exists());
}

#[test]
fn conf_fr_cli_001_export_explore_are_explicit() {
    let root = canonical_temp_root();
    let missing_store = root.join("must-not-be-read");
    let output = root.join("must-not-be-created");
    let export = Command::new(env!("CARGO_BIN_EXE_noesis"))
        .args(["export", "--store"])
        .arg(&missing_store)
        .args(["--repository-id", REPOSITORY_ID, "--output"])
        .arg(&output)
        .args(["--output"])
        .arg(root.join("duplicate"))
        .args(["--format", "json"])
        .output()
        .expect("launch invalid explicit export");
    assert_r8_error(&export, 2, "input.invalid_export_profile");
    assert!(!missing_store.exists());
    assert!(!output.exists());

    let missing_value = Command::new(env!("CARGO_BIN_EXE_noesis"))
        .args(["export", "--repository-id", REPOSITORY_ID, "--output"])
        .arg(&output)
        .args(["--format", "json", "--store", "--unknown"])
        .output()
        .expect("launch missing-value export");
    assert_r8_error(&missing_value, 2, "input.invalid_export_profile");
    assert!(!missing_store.exists());
    assert!(!output.exists());

    let unsafe_output = root.join("selected").join("..").join("escaped");
    let unsafe_before_store = Command::new(env!("CARGO_BIN_EXE_noesis"))
        .args(["export", "--store"])
        .arg(&missing_store)
        .args(["--repository-id", REPOSITORY_ID, "--output"])
        .arg(&unsafe_output)
        .args(["--format", "json"])
        .output()
        .expect("launch unsafe-output export");
    assert_r8_error(&unsafe_before_store, 2, "input.unsafe_output_path");
    assert!(!missing_store.exists());
    assert!(!root.join("escaped").exists());

    let input = root.join("must-not-be-read.json");
    let explorer = Command::new(env!("CARGO_BIN_EXE_noesis"))
        .args(["explore", "--input"])
        .arg(&input)
        .args(["--output"])
        .arg(&output)
        .args(["--unknown", "value", "--format", "json"])
        .output()
        .expect("launch invalid explicit explorer");
    assert_r8_error(&explorer, 2, "input.invalid_explorer_profile");
    assert!(!input.exists());
    assert!(!output.exists());

    let implicit = Command::new(env!("CARGO_BIN_EXE_noesis"))
        .args(["scan", "--input", "portable-graph.json", "--format", "json"])
        .output()
        .expect("launch non-R8 command");
    let error: Value = serde_json::from_slice(&implicit.stderr).expect("parse legacy error");
    assert_ne!(error["schema_version"], "codenoesis.error/v15");
}

#[test]
fn conf_fr_exp_001_invalid_security_matrix() {
    let expected = invalid_case_expectations();
    assert_eq!(expected.len(), 33);
    let observed = expected
        .keys()
        .map(|case_id| (case_id.clone(), exercise_invalid_case(case_id)))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(observed, expected);
}

#[test]
fn conf_fr_qry_001_r8_preserves_all_v5_subject_families() {
    let bytes = portable_bytes();
    let portable = PortableGraphV1::from_canonical_file(&bytes, noesis::portable_explorer::sha256)
        .expect("reimport reviewed R8 fixture");
    let oracle = family_digest_oracle();
    let observed = portable
        .family_digests()
        .expect("compute R8 family digests");

    assert_eq!(
        noesis::portable_explorer::sha256(&bytes),
        PORTABLE_FILE_SHA256
    );
    assert_eq!(portable.canonical_sha256(), PORTABLE_CANONICAL_SHA256);
    for family in portable_family_order() {
        assert_eq!(observed[family], oracle["families"][family], "{family}");
    }
}

#[test]
fn conf_fr_qry_002_bounded_deterministic_neighborhood() {
    let graph = portable_value();
    let root = graph["entities"][0]["id"]
        .as_str()
        .expect("reviewed root entity");
    let expected = bounded_neighborhood(&graph, root, 2);

    for _ in 0..R8_DETERMINISM_PERMUTATIONS {
        assert_eq!(bounded_neighborhood(&graph, root, 2), expected);
    }
    assert_eq!(expected.depth, 2);
    assert_eq!(expected.subject_ids.len(), 2);
    assert_eq!(expected.relationship_ids.len(), 1);
    assert!(!expected.subjects_truncated);
    assert!(!expected.relationships_truncated);

    let viewer = String::from_utf8(viewer_bytes()).expect("reviewed UTF-8 viewer");
    for required in [
        "const DEFAULT_DEPTH = 1;",
        "const MAX_DEPTH = 2;",
        "const MAX_SUBJECTS = 256;",
        "const MAX_RELATIONSHIPS = 512;",
        "const queue = [{ id: selectedId, depth: 0 }];",
        "relationships: selectedRelationships.length >= MAX_RELATIONSHIPS",
    ] {
        assert!(
            viewer.contains(required),
            "missing viewer behavior: {required}"
        );
    }
}

#[test]
fn gt_fr_exp_001_exact_identity_reference_and_evidence_preservation() {
    let graph = portable_value();
    let oracle = family_digest_oracle();
    let mut subjects = BTreeSet::new();
    let mut evidence = BTreeSet::new();

    for family in portable_family_order() {
        let id_field = if family == "documents" {
            "document_id"
        } else if family == "document_statements" {
            "statement_id"
        } else {
            "id"
        };
        let ids = graph[family]
            .as_array()
            .expect("reviewed family")
            .iter()
            .map(|record| {
                record[id_field]
                    .as_str()
                    .expect("reviewed stable identity")
                    .to_owned()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            serde_json::to_value(&ids).expect("serialize preserved IDs"),
            oracle["families"][family]["ids"],
            "{family}"
        );
        subjects.extend(ids);
    }
    for record in graph["evidence"].as_array().expect("reviewed evidence") {
        evidence.insert(record["id"].as_str().expect("evidence identity").to_owned());
    }
    for relationship in graph["relationships"]
        .as_array()
        .expect("reviewed relationships")
    {
        assert!(subjects.contains(relationship["source"].as_str().expect("source")));
        assert!(subjects.contains(relationship["target"].as_str().expect("target")));
    }
    for claim in graph["claims"].as_array().expect("reviewed claims") {
        assert!(subjects.contains(claim["subject_id"].as_str().expect("claim subject")));
    }
    for statement in graph["document_statements"]
        .as_array()
        .expect("reviewed document statements")
    {
        assert!(subjects.contains(statement["document_id"].as_str().expect("document link")));
        for subject in statement["subject_ids"]
            .as_array()
            .expect("statement subjects")
        {
            assert!(subjects.contains(subject.as_str().expect("statement subject")));
        }
    }
    visit_evidence_ids(&graph, &mut |evidence_id| {
        assert!(evidence.contains(evidence_id), "unresolved {evidence_id}");
    });
}

#[test]
fn ft_fr_exp_001_corrupt_visible_snapshot_fails_before_publication() {
    let repository = materialized_repository();
    assert_success(&repository.scan(), "R8 corrupt-head source scan");
    corrupt_visible_snapshot_semantic(&repository.store, REPOSITORY_ID);
    let root = existing_test_path(&repository.root);
    let output = root.join("must-not-publish");
    let rejected = export(&repository, &output);
    assert_r8_error(&rejected, 16, "export.invalid_snapshot");
    assert!(!output.exists());
}

#[test]
fn pt_nfr_det_001_r8_fifty_permutation_replay() {
    let repository = materialized_repository();
    assert_success(&repository.scan(), "R8 permutation source scan");
    let canonical_root = existing_test_path(&repository.root);
    let store_before = directory_bytes(&repository.store);
    let mut expected = None;

    for permutation in 0..R8_DETERMINISM_PERMUTATIONS {
        let output = canonical_root.join(format!("portable-{permutation:02}"));
        let exported = export_permutation(&repository, &output, permutation);
        assert_success(&exported, "R8 permutation export");
        assert!(exported.stderr.is_empty());
        if let Some(expected) = &expected {
            assert_eq!(&exported.stdout, expected, "permutation {permutation}");
        } else {
            expected = Some(exported.stdout);
        }
    }
    assert_eq!(directory_bytes(&repository.store), store_before);
}

#[test]
fn sec_fr_exp_001_xss_payloads_render_as_text() {
    let fixture = portable_value();
    for payload in [
        "</script><img src=x onerror=alert(1)>",
        "\" autofocus onfocus=\"alert(1)",
        "line\u{2028}separator",
        "paragraph\u{2029}separator",
        "bidi\u{202e}override",
        "control\u{0007}character",
    ] {
        let mut mutation = fixture.clone();
        mutation["entities"][0]["display_name"] = Value::String(payload.to_owned());
        assert!(matches!(
            reimport_value(&mutation),
            Err(R8ContractError::UnsafePayload { .. })
        ));
    }
    let viewer = String::from_utf8(viewer_bytes()).expect("reviewed UTF-8 viewer");
    assert!(viewer.contains(".textContent"));
    assert!(!viewer.contains(".innerHTML"));
    assert!(!viewer.contains("insertAdjacentHTML"));
    assert!(!viewer.contains("document.write"));
}

#[test]
fn sec_nfr_sec_001_r8_csp_forbids_active_remote_content() {
    let viewer = viewer_bytes();
    assert_eq!(noesis::portable_explorer::sha256(&viewer), VIEWER_SHA256);
    let shipped = reviewed_checkout_text(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/s4/r8/index.html"),
    );
    assert_eq!(shipped, viewer);
    let html = String::from_utf8(shipped).expect("reviewed UTF-8 viewer");
    assert!(html.contains("default-src 'none'"));
    for directive in [
        "connect-src 'none'",
        "object-src 'none'",
        "frame-src 'none'",
        "frame-ancestors 'none'",
        "form-action 'none'",
        "base-uri 'none'",
        "worker-src 'none'",
    ] {
        assert!(
            html.contains(directive),
            "missing CSP directive {directive}"
        );
    }
    for forbidden in [
        "http://",
        "https://",
        "fetch(",
        "XMLHttpRequest",
        "WebSocket",
        "eval(",
        "new Function(",
        "localStorage",
        "sessionStorage",
    ] {
        assert!(!html.contains(forbidden), "active capability {forbidden}");
    }

    let portable_bytes = portable_bytes();
    let portable =
        PortableGraphV1::from_canonical_file(&portable_bytes, noesis::portable_explorer::sha256)
            .expect("validated R8 fixture");
    let manifest = LocalExplorerManifestV1::new(&portable, &portable_bytes, &viewer, VIEWER_SHA256)
        .expect("build reviewed explorer manifest")
        .canonical_file()
        .expect("serialize reviewed explorer manifest");
    assert_eq!(manifest, explorer_manifest_bytes());
}

#[test]
fn sec_nfr_sec_005_r8_path_symlink_and_destination_confinement() {
    let root = canonical_temp_root();
    let input = write_portable_input(&root);
    let parent_escape = root.join("selected").join("..").join("escaped");
    let escaped = explore(&input, &parent_escape);
    assert_r8_error(&escaped, 2, "input.unsafe_output_path");
    assert!(!root.join("escaped").exists());

    let unmarked = root.join("unmarked");
    fs::create_dir(&unmarked).expect("create unmarked destination");
    let sentinel = unmarked.join("sentinel.txt");
    fs::write(&sentinel, b"unchanged\n").expect("write destination sentinel");
    let rejected = explore(&input, &unmarked);
    assert_r8_error(&rejected, 2, "input.unsafe_output_path");
    assert_eq!(
        fs::read(&sentinel).expect("read destination sentinel"),
        b"unchanged\n"
    );

    let mismatched = root.join("mismatched");
    fs::create_dir(&mismatched).expect("create mismatched destination");
    fs::write(
        mismatched.join(".codenoesis-local-explorer-v1"),
        b"{\"schema_version\":\"wrong\"}\n",
    )
    .expect("write mismatched marker");
    let rejected = explore(&input, &mismatched);
    assert_r8_error(&rejected, 2, "input.unsafe_output_path");

    let alias = explore(&input, &input);
    assert_r8_error(&alias, 2, "input.unsafe_output_path");
    assert_eq!(
        fs::read(&input).expect("read aliased input"),
        portable_bytes()
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let outside = root.join("outside");
        fs::create_dir(&outside).expect("create outside destination");
        let outside_sentinel = outside.join("sentinel.txt");
        fs::write(&outside_sentinel, b"outside unchanged\n").expect("write outside sentinel");
        let linked_parent = root.join("linked-parent");
        symlink(&outside, &linked_parent).expect("create destination parent symlink");
        let rejected = explore(&input, &linked_parent.join("explorer"));
        assert_r8_error(&rejected, 2, "input.unsafe_output_path");
        assert_eq!(
            fs::read(outside_sentinel).expect("read outside sentinel"),
            b"outside unchanged\n"
        );
        assert!(!outside.join("explorer").exists());
    }
}

#[test]
fn sec_nfr_prv_002_r8_excludes_source_contents_and_snippets() {
    let repository = materialized_repository();
    assert_success(&repository.scan(), "R8 privacy source scan");
    let root = existing_test_path(&repository.root);
    let output = root.join("portable-privacy");
    let exported = export(&repository, &output);
    assert_success(&exported, "R8 privacy export");
    let value: Value = serde_json::from_slice(&exported.stdout).expect("parse privacy export");

    for forbidden in [
        "source_content",
        "source_contents",
        "source_snippet",
        "source_snippets",
        "raw_arguments",
        "raw_project_root",
        "absolute_path",
        "file_url",
    ] {
        assert!(
            !contains_object_key(&value, forbidden),
            "leaked key {forbidden}"
        );
    }
    assert_eq!(value["projection"]["source_contents_included"], false);
    assert_eq!(value["projection"]["source_snippets_included"], false);

    for path in string_fields_named(&value, &["path", "document_path"]) {
        assert!(
            !Path::new(path).is_absolute(),
            "absolute portable path {path}"
        );
        assert!(
            !Path::new(path)
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
        );
    }
    for private in [
        repository.root.as_os_str().as_encoded_bytes(),
        repository.worktree.as_os_str().as_encoded_bytes(),
        repository.store.as_os_str().as_encoded_bytes(),
    ] {
        assert!(!contains_bytes(&exported.stdout, private));
    }
    for source in regular_file_bytes(&repository.worktree) {
        if source.len() >= 32 {
            assert!(!contains_bytes(&exported.stdout, &source));
        }
    }
}

#[test]
fn pt_od_lim_001_r8_all_limits_have_maximum_plus_one() {
    let root = canonical_temp_root();
    let oversized = root.join("oversized.json");
    let file = fs::File::create(&oversized).expect("create oversized portable input");
    file.set_len(MAX_R8_PORTABLE_GRAPH_BYTES + 1)
        .expect("size sparse oversized portable input");
    let output = root.join("oversized-output");
    let rejected = explore(&oversized, &output);
    assert_r8_error(&rejected, 17, "export.limit_exceeded");
    assert!(!output.exists());

    let mut maximum_nesting = "null".to_owned();
    for _ in 0..MAX_R8_JSON_NESTING {
        maximum_nesting = format!("[{maximum_nesting}]");
    }
    assert!(!matches!(
        PortableGraphV1::from_canonical_file(
            format!("{maximum_nesting}\n").as_bytes(),
            noesis::portable_explorer::sha256,
        ),
        Err(R8ContractError::LimitExceeded {
            limit: "json_nesting",
            ..
        })
    ));
    let nested = format!("[{maximum_nesting}]");
    assert!(matches!(
        PortableGraphV1::from_canonical_file(
            format!("{nested}\n").as_bytes(),
            noesis::portable_explorer::sha256,
        ),
        Err(R8ContractError::LimitExceeded {
            limit: "json_nesting",
            maximum: MAX_R8_JSON_NESTING,
            ..
        })
    ));

    let bytes = portable_bytes();
    let portable = PortableGraphV1::from_canonical_file(&bytes, noesis::portable_explorer::sha256)
        .expect("validated limit fixture");
    let oversized_viewer =
        vec![b'x'; usize::try_from(MAX_R8_VIEWER_NON_DATA_BYTES + 1).expect("viewer limit usize")];
    assert!(matches!(
        LocalExplorerManifestV1::new(&portable, &bytes, &oversized_viewer, VIEWER_SHA256),
        Err(R8ContractError::LimitExceeded {
            limit: "viewer_non_data_bytes",
            maximum: MAX_R8_VIEWER_NON_DATA_BYTES,
            ..
        })
    ));

    let manifest: Value =
        serde_json::from_slice(&explorer_manifest_bytes()).expect("parse explorer limit manifest");
    assert_eq!(
        manifest["limits"]["text_search_results"],
        MAX_R8_TEXT_SEARCH_RESULTS
    );
    assert_eq!(
        manifest["limits"]["traversal_depth_default"],
        R8_TRAVERSAL_DEPTH_DEFAULT
    );
    assert_eq!(
        manifest["limits"]["traversal_depth_maximum"],
        MAX_R8_TRAVERSAL_DEPTH
    );
    assert_eq!(
        manifest["limits"]["neighborhood_subjects"],
        MAX_R8_NEIGHBORHOOD_SUBJECTS
    );
    assert_eq!(
        manifest["limits"]["neighborhood_relationships"],
        MAX_R8_NEIGHBORHOOD_RELATIONSHIPS
    );
    let viewer = String::from_utf8(viewer_bytes()).expect("reviewed UTF-8 viewer");
    assert!(viewer.contains("matches.slice(0, MAX_TEXT_RESULTS)"));
    assert!(viewer.contains("Math.min(Math.max(requestedDepth, DEFAULT_DEPTH), MAX_DEPTH)"));
    assert!(viewer.contains("visited.size < MAX_SUBJECTS"));
    assert!(viewer.contains("selectedRelationships.length >= MAX_RELATIONSHIPS"));
}

#[test]
fn reg_fr_qry_001_r7_exact_query_bytes_unchanged() {
    let repository = materialized_repository();
    let scan = repository.scan();
    assert_success(&scan, "R7 query-regression source scan");
    let snapshot: Value = serde_json::from_slice(&scan.stdout).expect("parse R7 snapshot");
    assert_success(&repository.docs(), "R7 query-regression docs");
    let requested_id = snapshot["semantic"]["knowledge_graph"]["entities"][0]["id"]
        .as_str()
        .expect("R7 query-regression entity ID");
    let before = repository.query(requested_id);
    assert_success(&before, "R7 query before R8");
    let store_before = directory_bytes(&repository.store);

    let root = existing_test_path(&repository.root);
    let portable_root = root.join("query-regression-portable");
    let explorer_root = root.join("query-regression-explorer");
    assert_success(&export(&repository, &portable_root), "R8 regression export");
    assert_success(
        &explore(&portable_root.join("portable-graph.json"), &explorer_root),
        "R8 regression explorer",
    );
    let after = repository.query(requested_id);

    assert_eq!(after.status.code(), before.status.code());
    assert_eq!(after.stdout, before.stdout);
    assert_eq!(after.stderr, before.stderr);
    assert_eq!(directory_bytes(&repository.store), store_before);
}

#[test]
fn reg_fr_cli_001_r7_commands_and_stored_head_unchanged() {
    let repository = materialized_repository();
    let before = repository.scan();
    assert_success(&before, "R7 CLI before R8");
    let store_before = directory_bytes(&repository.store);
    let root = existing_test_path(&repository.root);
    let portable_root = root.join("cli-regression-portable");
    let explorer_root = root.join("cli-regression-explorer");

    assert_success(
        &export(&repository, &portable_root),
        "R8 CLI-regression export",
    );
    assert_success(
        &explore(&portable_root.join("portable-graph.json"), &explorer_root),
        "R8 CLI-regression explorer",
    );
    assert_eq!(directory_bytes(&repository.store), store_before);
    assert!(!repository.build_sentinel().exists());
    assert!(!repository.indexer_sentinel().exists());

    let replay = repository.scan();
    assert_success(&replay, "R7 CLI after R8");
    assert_eq!(replay.stderr, before.stderr);
    let before: Value = serde_json::from_slice(&before.stdout).expect("parse R7 CLI before R8");
    let replay: Value = serde_json::from_slice(&replay.stdout).expect("parse R7 CLI after R8");
    assert_eq!(replay["schema_version"], before["schema_version"]);
    assert_eq!(replay["semantic"], before["semantic"]);
    assert_eq!(replay["semantic_hash"], before["semantic_hash"]);
}

fn export(repository: &MaterializedCompilerIndexRepository, output: &std::path::Path) -> Output {
    let store = existing_test_path(&repository.store);
    Command::new(env!("CARGO_BIN_EXE_noesis"))
        .args(["export", "--store"])
        .arg(store)
        .args(["--repository-id", REPOSITORY_ID, "--output"])
        .arg(output)
        .args(["--format", "json"])
        .output()
        .expect("launch R8 export subject")
}

fn explore(input: &std::path::Path, output: &std::path::Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_noesis"))
        .args(["explore", "--input"])
        .arg(input)
        .arg("--output")
        .arg(output)
        .args(["--format", "json"])
        .output()
        .expect("launch R8 explore subject")
}

fn assert_success(output: &Output, subject: &str) {
    assert!(
        output.status.success(),
        "{subject} failed: status={:?}, stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn export_permutation(
    repository: &MaterializedCompilerIndexRepository,
    output: &Path,
    permutation: u64,
) -> Output {
    let store = existing_test_path(&repository.store);
    let mut options = vec![
        ("--store", store.into_os_string()),
        ("--repository-id", REPOSITORY_ID.into()),
        ("--output", output.as_os_str().to_owned()),
        ("--format", "json".into()),
    ];
    let option_count = options.len();
    options.rotate_left(usize::try_from(permutation).expect("R8 permutation usize") % option_count);
    if permutation / u64::try_from(option_count).expect("R8 option count u64") % 2 == 1 {
        options.reverse();
    }
    let mut command = Command::new(env!("CARGO_BIN_EXE_noesis"));
    command.arg("export");
    for (flag, value) in options {
        command.arg(flag).arg(value);
    }
    command.output().expect("launch permuted R8 export")
}

fn assert_r8_error(output: &Output, exit_code: i32, code: &str) {
    assert_eq!(output.status.code(), Some(exit_code));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr.last(), Some(&b'\n'));
    let value: Value = serde_json::from_slice(&output.stderr).expect("parse ErrorV15");
    assert_eq!(value["schema_version"], "codenoesis.error/v15");
    assert_eq!(value["code"], code);
    assert_eq!(value["retryable"], false);
    let mut canonical = serde_json::to_vec(&value).expect("serialize ErrorV15");
    canonical.push(b'\n');
    assert_eq!(output.stderr, canonical);
}

fn portable_family_order() -> [&'static str; 8] {
    [
        "entities",
        "relationships",
        "claims",
        "evidence",
        "diagnostics",
        "coverage_gaps",
        "documents",
        "document_statements",
    ]
}

fn reimport_value(value: &Value) -> Result<PortableGraphV1, R8ContractError> {
    let mut bytes = serde_json::to_vec(&value).expect("serialize R8 mutation");
    bytes.push(b'\n');
    PortableGraphV1::from_canonical_file(&bytes, noesis::portable_explorer::sha256)
}

#[allow(clippy::too_many_lines)]
fn exercise_invalid_case(case_id: &str) -> String {
    let mut graph = portable_value();
    let result = match case_id {
        "connect_source_enabled" => {
            let viewer = String::from_utf8(viewer_bytes()).expect("reviewed R8 viewer");
            let mutated = viewer.replace("connect-src 'none'", "connect-src 'self'");
            validate_r8_viewer_asset(
                mutated.as_bytes(),
                &noesis::portable_explorer::sha256(mutated.as_bytes()),
                noesis::portable_explorer::sha256,
            )
        }
        "destination_component_dot_dot" => {
            let root = canonical_temp_root();
            let input = write_portable_input(&root);
            return strict_error_code(&explore(
                &input,
                &root.join("selected").join("..").join("escaped"),
            ));
        }
        "destination_marker_mismatch" => {
            let root = canonical_temp_root();
            let input = write_portable_input(&root);
            let output = root.join("marked");
            fs::create_dir(&output).expect("create mismatched R8 destination");
            fs::write(
                output.join(".codenoesis-local-explorer-v1"),
                b"{\"schema_version\":\"wrong\"}\n",
            )
            .expect("write mismatched R8 marker");
            return strict_error_code(&explore(&input, &output));
        }
        "destination_non_empty_unmarked" => {
            let root = canonical_temp_root();
            let input = write_portable_input(&root);
            let output = root.join("unmarked");
            fs::create_dir(&output).expect("create unmarked R8 destination");
            fs::write(output.join("manual.txt"), b"unchanged\n").expect("write unowned R8 byte");
            return strict_error_code(&explore(&input, &output));
        }
        "destination_parent_symlink_escape" => {
            let root = canonical_temp_root();
            let input = write_portable_input(&root);
            let unsafe_parent = root.join("unsafe-parent");
            create_unsafe_directory_alias(&root.join("outside"), &unsafe_parent);
            return strict_error_code(&explore(&input, &unsafe_parent.join("explorer")));
        }
        "duplicate_entity_id" => {
            let mut duplicate = graph["entities"][0].clone();
            duplicate["display_name"] = Value::String("different".to_owned());
            graph["entities"]
                .as_array_mut()
                .expect("R8 entities")
                .insert(1, duplicate);
            reimport_value(&graph).map(|_| ())
        }
        "duplicate_relationship_id" => {
            let duplicate = graph["relationships"][0].clone();
            graph["relationships"]
                .as_array_mut()
                .expect("R8 relationships")
                .push(duplicate);
            reimport_value(&graph).map(|_| ())
        }
        "dynamic_code_evaluation" => {
            let viewer = String::from_utf8(viewer_bytes()).expect("reviewed R8 viewer");
            let mutated = viewer.replace("</script>", "new Function('return 1');</script>");
            validate_r8_viewer_asset(
                mutated.as_bytes(),
                &noesis::portable_explorer::sha256(mutated.as_bytes()),
                noesis::portable_explorer::sha256,
            )
        }
        "evidence_absolute_path" => {
            source_evidence_mut(&mut graph)["path"] =
                Value::String("/private/source.rs".to_owned());
            reimport_value(&graph).map(|_| ())
        }
        "evidence_parent_escape" => {
            source_evidence_mut(&mut graph)["path"] = Value::String("../../source.rs".to_owned());
            reimport_value(&graph).map(|_| ())
        }
        "html_attribute_quotes" => {
            compiler_entity_mut(&mut graph)["display_name"] =
                Value::String("\" onfocus=\"alert(1)".to_owned());
            reimport_value(&graph).map(|_| ())
        }
        "html_script_close" => {
            compiler_entity_mut(&mut graph)["display_name"] =
                Value::String("</script><script>alert(1)</script>".to_owned());
            reimport_value(&graph).map(|_| ())
        }
        "json_nesting_max_plus_one" => {
            let mut nested = "null".to_owned();
            for _ in 0..=MAX_R8_JSON_NESTING {
                nested = format!("[{nested}]");
            }
            PortableGraphV1::from_canonical_file(
                format!("{nested}\n").as_bytes(),
                noesis::portable_explorer::sha256,
            )
            .map(|_| ())
        }
        "missing_claim_subject" => {
            graph["claims"][0]["subject_kind"] = Value::String("entity".to_owned());
            graph["claims"][0]["subject_id"] = Value::String(zero_entity_id());
            reimport_value(&graph).map(|_| ())
        }
        "missing_evidence_reference" => {
            graph["relationships"][0]["evidence_ids"] =
                Value::Array(vec![Value::String(zero_evidence_id())]);
            reimport_value(&graph).map(|_| ())
        }
        "missing_relationship_endpoint" => {
            graph["relationships"][0]["target"] = Value::String(zero_entity_id());
            reimport_value(&graph).map(|_| ())
        }
        "neighborhood_relationships_max_plus_one" => validate_r8_view_limits(
            MAX_R8_TEXT_SEARCH_RESULTS,
            MAX_R8_TRAVERSAL_DEPTH,
            MAX_R8_NEIGHBORHOOD_SUBJECTS,
            MAX_R8_NEIGHBORHOOD_RELATIONSHIPS + 1,
        ),
        "neighborhood_subjects_max_plus_one" => validate_r8_view_limits(
            MAX_R8_TEXT_SEARCH_RESULTS,
            MAX_R8_TRAVERSAL_DEPTH,
            MAX_R8_NEIGHBORHOOD_SUBJECTS + 1,
            MAX_R8_NEIGHBORHOOD_RELATIONSHIPS,
        ),
        "noncanonical_family_order" => {
            graph["entities"]
                .as_array_mut()
                .expect("R8 entities")
                .reverse();
            reimport_value(&graph).map(|_| ())
        }
        "oversized_display_label" => {
            compiler_entity_mut(&mut graph)["display_name"] = Value::String("x".repeat(16_385));
            reimport_value(&graph).map(|_| ())
        }
        "oversized_metadata" => {
            source_evidence_mut(&mut graph)["path"] = Value::String("x".repeat(65_537));
            reimport_value(&graph).map(|_| ())
        }
        "portable_graph_bytes_max_plus_one" => {
            let root = canonical_temp_root();
            let input = root.join("oversized.json");
            fs::File::create(&input)
                .expect("create oversized R8 matrix input")
                .set_len(MAX_R8_PORTABLE_GRAPH_BYTES + 1)
                .expect("size oversized R8 matrix input");
            return strict_error_code(&explore(&input, &root.join("output")));
        }
        "remote_script_origin" => {
            let viewer = String::from_utf8(viewer_bytes()).expect("reviewed R8 viewer");
            let mutated = viewer.replace(
                "<script>",
                "<script src=\"remote-origin/script.js\"></script><script>",
            );
            validate_r8_viewer_asset(
                mutated.as_bytes(),
                &noesis::portable_explorer::sha256(mutated.as_bytes()),
                noesis::portable_explorer::sha256,
            )
        }
        "snapshot_hash_mismatch" => {
            let repository = materialized_repository();
            assert_success(&repository.scan(), "R8 invalid-matrix source scan");
            corrupt_visible_snapshot_semantic(&repository.store, REPOSITORY_ID);
            let root = existing_test_path(&repository.root);
            return strict_error_code(&export(&repository, &root.join("output")));
        }
        "text_results_max_plus_one" => validate_r8_view_limits(
            MAX_R8_TEXT_SEARCH_RESULTS + 1,
            MAX_R8_TRAVERSAL_DEPTH,
            MAX_R8_NEIGHBORHOOD_SUBJECTS,
            MAX_R8_NEIGHBORHOOD_RELATIONSHIPS,
        ),
        "traversal_depth_max_plus_one" => validate_r8_view_limits(
            MAX_R8_TEXT_SEARCH_RESULTS,
            MAX_R8_TRAVERSAL_DEPTH + 1,
            MAX_R8_NEIGHBORHOOD_SUBJECTS,
            MAX_R8_NEIGHBORHOOD_RELATIONSHIPS,
        ),
        "unicode_bidi_override" => {
            compiler_entity_mut(&mut graph)["display_name"] =
                Value::String("safe\u{202e}txt.exe".to_owned());
            reimport_value(&graph).map(|_| ())
        }
        "unicode_control_character" => {
            compiler_entity_mut(&mut graph)["display_name"] =
                Value::String("safe\u{0001}label".to_owned());
            reimport_value(&graph).map(|_| ())
        }
        "unicode_line_separator" => {
            compiler_entity_mut(&mut graph)["display_name"] =
                Value::String("line\u{2028}break".to_owned());
            reimport_value(&graph).map(|_| ())
        }
        "unicode_paragraph_separator" => {
            compiler_entity_mut(&mut graph)["display_name"] =
                Value::String("line\u{2029}break".to_owned());
            reimport_value(&graph).map(|_| ())
        }
        "unknown_projection_field" => {
            graph
                .as_object_mut()
                .expect("R8 projection object")
                .insert("unknown".to_owned(), Value::Bool(true));
            reimport_value(&graph).map(|_| ())
        }
        "unsupported_projection_version" => {
            graph["schema_version"] = Value::String("codenoesis.portable-graph/v2".to_owned());
            reimport_value(&graph).map(|_| ())
        }
        "viewer_asset_bytes_max_plus_one" => {
            let bytes = portable_bytes();
            let portable =
                PortableGraphV1::from_canonical_file(&bytes, noesis::portable_explorer::sha256)
                    .expect("validated R8 limit fixture");
            let viewer = vec![
                b'x';
                usize::try_from(MAX_R8_VIEWER_NON_DATA_BYTES + 1)
                    .expect("R8 viewer maximum plus one")
            ];
            LocalExplorerManifestV1::new(
                &portable,
                &bytes,
                &viewer,
                &noesis::portable_explorer::sha256(&viewer),
            )
            .map(|_| ())
        }
        other => panic!("unimplemented R8 invalid case: {other}"),
    };
    strict_contract_error_code(&result.expect_err("R8 invalid case must fail"))
}

fn strict_contract_error_code(error: &R8ContractError) -> String {
    let bytes = CodeNoesisErrorV15::from_contract(error)
        .canonical_stderr()
        .expect("serialize strict R8 contract error");
    assert_eq!(bytes.last(), Some(&b'\n'));
    let value: Value = serde_json::from_slice(&bytes).expect("parse strict R8 contract error");
    let mut canonical = serde_json::to_vec(&value).expect("serialize strict R8 contract error");
    canonical.push(b'\n');
    assert_eq!(bytes, canonical);
    value["code"].as_str().expect("R8 error code").to_owned()
}

fn strict_error_code(output: &Output) -> String {
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr.last(), Some(&b'\n'));
    let value: Value = serde_json::from_slice(&output.stderr).expect("parse strict ErrorV15");
    assert_eq!(value["schema_version"], "codenoesis.error/v15");
    assert_eq!(value["retryable"], false);
    let mut canonical = serde_json::to_vec(&value).expect("serialize strict ErrorV15");
    canonical.push(b'\n');
    assert_eq!(output.stderr, canonical);
    value["code"].as_str().expect("R8 error code").to_owned()
}

fn compiler_entity_mut(graph: &mut Value) -> &mut Value {
    graph["entities"]
        .as_array_mut()
        .expect("R8 entities")
        .iter_mut()
        .find(|entity| entity["kind"] == "compiler.symbol")
        .expect("R8 compiler entity")
}

fn source_evidence_mut(graph: &mut Value) -> &mut Value {
    graph["evidence"]
        .as_array_mut()
        .expect("R8 evidence")
        .iter_mut()
        .find(|evidence| evidence.get("path").is_some())
        .expect("R8 source evidence")
}

fn zero_entity_id() -> String {
    format!("urn:codenoesis:entity:blake3:{}", "0".repeat(64))
}

fn zero_evidence_id() -> String {
    format!("urn:codenoesis:evidence:sha256:{}", "0".repeat(64))
}

#[cfg(unix)]
fn create_unsafe_directory_alias(outside: &Path, alias: &Path) {
    use std::os::unix::fs::symlink;

    fs::create_dir(outside).expect("create R8 outside directory");
    symlink(outside, alias).expect("create R8 unsafe directory alias");
}

#[cfg(not(unix))]
fn create_unsafe_directory_alias(_outside: &Path, alias: &Path) {
    fs::write(alias, b"not a directory\n").expect("create R8 unsafe parent replacement");
}

fn visit_evidence_ids(value: &Value, visit: &mut impl FnMut(&str)) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                if matches!(
                    key.as_str(),
                    "evidence_ids" | "compiler_evidence_ids" | "source_evidence_ids"
                ) {
                    for evidence_id in value.as_array().expect("reviewed evidence references") {
                        visit(evidence_id.as_str().expect("reviewed evidence ID"));
                    }
                } else {
                    visit_evidence_ids(value, visit);
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                visit_evidence_ids(value, visit);
            }
        }
        _ => {}
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Neighborhood {
    depth: u64,
    subject_ids: Vec<String>,
    relationship_ids: Vec<String>,
    subjects_truncated: bool,
    relationships_truncated: bool,
}

fn bounded_neighborhood(graph: &Value, root: &str, requested_depth: u64) -> Neighborhood {
    let depth = requested_depth.clamp(R8_TRAVERSAL_DEPTH_DEFAULT, MAX_R8_TRAVERSAL_DEPTH);
    let mut relationships = graph["relationships"]
        .as_array()
        .expect("reviewed relationships")
        .iter()
        .collect::<Vec<_>>();
    relationships.sort_by_key(|relationship| stable_id(relationship));
    let mut queue = VecDeque::from([(root.to_owned(), 0)]);
    let mut visited = BTreeSet::new();
    let mut selected = BTreeSet::new();

    while let Some((current, current_depth)) = queue.pop_front() {
        if visited.len()
            >= usize::try_from(MAX_R8_NEIGHBORHOOD_SUBJECTS).expect("subject limit usize")
            || !visited.insert(current.clone())
        {
            continue;
        }
        if current_depth >= depth {
            continue;
        }
        for relationship in &relationships {
            if selected.len()
                >= usize::try_from(MAX_R8_NEIGHBORHOOD_RELATIONSHIPS)
                    .expect("relationship limit usize")
            {
                break;
            }
            let source = relationship["source"].as_str().unwrap_or_default();
            let target = relationship["target"].as_str().unwrap_or_default();
            if source != current && target != current {
                continue;
            }
            selected.insert(stable_id(relationship).to_owned());
            let neighbor = if source == current { target } else { source };
            if !neighbor.is_empty()
                && !visited.contains(neighbor)
                && queue.len() + visited.len()
                    < usize::try_from(MAX_R8_NEIGHBORHOOD_SUBJECTS).expect("subject limit usize")
            {
                queue.push_back((neighbor.to_owned(), current_depth + 1));
            }
        }
    }
    let subjects_truncated = visited.len()
        >= usize::try_from(MAX_R8_NEIGHBORHOOD_SUBJECTS).expect("subject limit usize");
    let relationships_truncated = selected.len()
        >= usize::try_from(MAX_R8_NEIGHBORHOOD_RELATIONSHIPS).expect("relationship limit usize");
    Neighborhood {
        depth,
        subject_ids: visited.into_iter().collect(),
        relationship_ids: selected.into_iter().collect(),
        subjects_truncated,
        relationships_truncated,
    }
}

fn stable_id(value: &Value) -> &str {
    [
        "id",
        "entity_id",
        "relationship_id",
        "claim_id",
        "evidence_id",
        "document_id",
        "statement_id",
    ]
    .into_iter()
    .find_map(|field| value.get(field).and_then(Value::as_str))
    .unwrap_or_default()
}

fn directory_bytes(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    collect_directory_bytes(root, root, &mut files);
    files
}

fn collect_directory_bytes(root: &Path, path: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
    let mut entries = fs::read_dir(path)
        .expect("read retained directory")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect retained directory");
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let file_type = entry.file_type().expect("retained entry type");
        if file_type.is_dir() {
            collect_directory_bytes(root, &entry.path(), files);
        } else if file_type.is_file() {
            files.insert(
                entry
                    .path()
                    .strip_prefix(root)
                    .expect("retained relative path")
                    .to_path_buf(),
                fs::read(entry.path()).expect("read retained file"),
            );
        } else {
            panic!("unexpected retained filesystem entry");
        }
    }
}

fn contains_object_key(value: &Value, expected: &str) -> bool {
    match value {
        Value::Object(object) => {
            object.contains_key(expected)
                || object
                    .values()
                    .any(|value| contains_object_key(value, expected))
        }
        Value::Array(values) => values
            .iter()
            .any(|value| contains_object_key(value, expected)),
        _ => false,
    }
}

fn string_fields_named<'a>(value: &'a Value, fields: &[&str]) -> Vec<&'a str> {
    fn collect<'a>(value: &'a Value, fields: &[&str], output: &mut Vec<&'a str>) {
        match value {
            Value::Object(object) => {
                for (key, value) in object {
                    if fields.contains(&key.as_str()) {
                        if let Some(value) = value.as_str() {
                            output.push(value);
                        }
                    } else {
                        collect(value, fields, output);
                    }
                }
            }
            Value::Array(values) => {
                for value in values {
                    collect(value, fields, output);
                }
            }
            _ => {}
        }
    }
    let mut output = Vec::new();
    collect(value, fields, &mut output);
    output
}

fn regular_file_bytes(root: &Path) -> Vec<Vec<u8>> {
    directory_bytes(root).into_values().collect()
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}
