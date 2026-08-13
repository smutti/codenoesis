mod support;

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::process::Output;

use serde_json::Value;
use sha2::{Digest as _, Sha256};

use support::parse_single_document;
use support::s4_r16::{
    MaterializedConstantEvaluationRepository, expected_safe_constant_evaluation,
};
use support::versioned_explorer::assert_matching_viewer_contract;

const EXPECTED_RED_STDERR_SHA256: &str =
    "7f75f7a91f6af0328795f3fbd2729e69756beba2ebd642cc1f6401265662a2fe";

#[test]
fn e2e_fr_exp_008_local_explorer_v9_is_bound_to_portable_v9() {
    let repository = MaterializedConstantEvaluationRepository::fixture();
    assert_success(&repository.scan(), "R16 viewer-contract scan");
    assert_success(&repository.docs(), "R16 viewer-contract docs");
    assert_success(&repository.export(), "R16 viewer-contract export");
    let explore = repository.explore();
    assert_success(&explore, "R16 viewer-contract explore");
    let manifest = parse_single_document(&explore.stdout);
    assert_matching_viewer_contract(&repository.explorer.join("index.html"), &manifest, 9);
}

#[test]
#[allow(clippy::too_many_lines)]
fn e2e_fr_ext_020_rust_safe_constant_evaluation_complete_local_journey() {
    let repository = MaterializedConstantEvaluationRepository::fixture();
    assert!(!repository.build_sentinel().exists());
    let scan = repository.scan();

    if scan.status.code() == Some(2) {
        assert!(scan.stdout.is_empty(), "R16 expected-Red stdout changed");
        assert_eq!(scan.stderr.len(), 149, "R16 expected-Red length changed");
        assert_eq!(
            hex_sha256(&scan.stderr),
            EXPECTED_RED_STDERR_SHA256,
            "R16 expected-Red digest changed"
        );
        let error = parse_single_document(&scan.stderr);
        assert_eq!(error["schema_version"], "codenoesis.error/v4");
        assert_eq!(error["code"], "input.invalid_revision");
        assert_eq!(error["stage"], "input");
        assert!(!repository.store.exists(), "R16 expected Red mutated store");
        assert!(!repository.build_sentinel().exists());
        panic!("expected RepositorySnapshotV18 success; observed frozen selector Red");
    }

    assert_success(&scan, "R16 safe constant-evaluation scan");
    assert!(scan.stderr.is_empty());
    let expected = expected_safe_constant_evaluation();
    let observed_local_bytes = usize::try_from(
        expected["canonical_stdout_bytes"]
            .as_u64()
            .expect("reviewed local R16 stdout length"),
    )
    .expect("reviewed local R16 stdout length fits usize");
    assert!(observed_local_bytes <= 268_435_456);
    assert!(scan.stdout.len() <= 268_435_456);
    assert_eq!(scan.stdout.last(), Some(&b'\n'));
    let snapshot = parse_single_document(&scan.stdout);
    assert_eq!(
        snapshot["schema_version"],
        "codenoesis.repository-snapshot/v18"
    );
    assert_eq!(
        snapshot["semantic_hash"]["value"],
        expected["expected_hashes"]["snapshot"]
    );
    assert_eq!(
        snapshot["semantic"]["configuration"]["schema_version"],
        "codenoesis.configuration/v15"
    );
    assert_eq!(
        snapshot["semantic"]["configuration"]["rust_constant_profile"],
        "rust-safe-constant-evaluation-v1"
    );
    assert_eq!(
        snapshot["semantic"]["configuration"]["semantic_hash"]["value"],
        expected["expected_hashes"]["configuration"]
    );
    assert_eq!(
        snapshot["semantic"]["pipeline_version"],
        "codenoesis.pipeline/s4-r16-v1"
    );

    let chunks = snapshot["semantic"]["extraction_chunks"]
        .as_array()
        .expect("R16 extraction chunks");
    assert_eq!(chunks.len(), 2);
    assert_eq!(
        chunks[0]["semantic_hash"]["value"],
        expected["expected_hashes"]["manifest_chunk"]
    );
    assert_eq!(
        chunks[1]["constant_evaluation_profile"],
        "rust-safe-constant-evaluation-v1"
    );
    assert_eq!(
        chunks[1]["semantic_hash"]["value"],
        expected["expected_hashes"]["source_chunk"]
    );

    let graph = &snapshot["semantic"]["knowledge_graph"];
    assert_eq!(graph["schema_version"], "codenoesis.knowledge-graph/v15");
    assert_eq!(graph["ontology_version"], "codenoesis.ontology/rust/v15");
    assert_eq!(
        graph["semantic_hash"]["value"],
        expected["expected_hashes"]["knowledge_graph"]
    );
    for (family, count) in expected["complete_counts"]
        .as_object()
        .expect("R16 complete counts")
    {
        if matches!(family.as_str(), "deterministic_claims" | "derived_claims") {
            continue;
        }
        assert_eq!(
            graph[family].as_array().map_or(0, Vec::len),
            usize::try_from(count.as_u64().expect("R16 family count"))
                .expect("R16 count fits usize"),
            "R16 graph family {family}"
        );
    }

    let entities = graph["entities"].as_array().expect("R16 entities");
    let relationships = graph["relationships"]
        .as_array()
        .expect("R16 relationships");
    let claims = graph["claims"].as_array().expect("R16 claims");
    let values = expected["evaluated_values"]
        .as_array()
        .expect("reviewed R16 values");
    for reviewed in values {
        let entity = entities
            .iter()
            .find(|value| value["id"] == reviewed["id"])
            .expect("reviewed evaluated value");
        assert_eq!(entity["kind"], "rust.evaluated_value");
        assert_eq!(entity["declared_value_id"], reviewed["declared_value_id"]);
        assert_eq!(entity["properties"]["canonical_value"], reviewed["value"]);
        assert_eq!(entity["properties"]["value_kind"], reviewed["value_kind"]);
        assert_eq!(entity["properties"]["rust_type"], reviewed["rust_type"]);
        assert_eq!(
            entity["properties"]["type_authority"],
            reviewed["type_authority"]
        );
        assert_eq!(
            entity["properties"]["rule_version"],
            "codenoesis.rule/rust-safe-constant-evaluation/v1"
        );

        let relationship = relationships
            .iter()
            .find(|value| value["id"] == reviewed["relationship_id"])
            .expect("reviewed EVALUATES_TO relationship");
        assert_eq!(relationship["kind"], "EVALUATES_TO");
        assert_eq!(relationship["source"], reviewed["declared_value_id"]);
        assert_eq!(relationship["target"], reviewed["id"]);
        assert_eq!(relationship["evidence_ids"], reviewed["evidence_ids"]);

        for (claim_id, subject_id, subject_kind) in [
            (&reviewed["claim_id"], &reviewed["id"], "entity"),
            (
                &reviewed["relationship_claim_id"],
                &reviewed["relationship_id"],
                "relationship",
            ),
        ] {
            let claim = claims
                .iter()
                .find(|value| value["id"] == *claim_id)
                .expect("reviewed R16 claim");
            assert_eq!(claim["subject_id"], *subject_id);
            assert_eq!(claim["subject_kind"], subject_kind);
            assert_eq!(claim["state"], "derived_fact");
            assert_eq!(claim["evidence_ids"], reviewed["evidence_ids"]);
        }
    }
    assert_eq!(
        count_by(claims, "state").get("deterministic_fact"),
        Some(&65)
    );
    assert_eq!(count_by(claims, "state").get("derived_fact"), Some(&19));

    let index = &graph["constant_evaluation_index"];
    assert_eq!(
        index["schema_version"],
        "codenoesis.constant-evaluation-index/v1"
    );
    assert_eq!(
        index["rule_version"],
        "codenoesis.rule/rust-safe-constant-evaluation/v1"
    );
    assert_eq!(
        index["evaluated_entity_ids"].as_array().map(Vec::len),
        Some(7)
    );
    assert_eq!(
        index["evaluation_relationship_ids"]
            .as_array()
            .map(Vec::len),
        Some(7)
    );
    assert_eq!(index["derivations"].as_array().map(Vec::len), Some(7));
    for reviewed in values {
        let derivation = index["derivations"]
            .as_array()
            .expect("R16 derivations")
            .iter()
            .find(|value| value["entity_id"] == reviewed["id"])
            .expect("reviewed R16 derivation");
        assert_eq!(derivation["relationship_id"], reviewed["relationship_id"]);
        assert_eq!(derivation["input_claim_ids"], reviewed["input_claim_ids"]);
        assert_eq!(derivation["input_evidence_ids"], reviewed["evidence_ids"]);
        assert_eq!(
            derivation["dependency_entity_ids"],
            reviewed["dependency_entity_ids"]
        );
    }

    let coverage = graph["coverage"].as_array().expect("R16 coverage");
    for removed in expected["removed_coverage_ids"]
        .as_array()
        .expect("R16 removed coverage")
    {
        assert!(coverage.iter().all(|value| value["id"] != *removed));
    }
    for reviewed in expected["new_coverage"]
        .as_array()
        .expect("R16 new coverage")
    {
        assert!(coverage.iter().any(|value| value == reviewed));
    }
    assert_connected(entities, relationships);

    assert_success(&repository.docs(), "R16 documentation generation");
    let documentation = generated_markdown(&repository.documents);
    assert!(documentation.contains("Safe constant evaluation"));
    assert!(documentation.contains("not compiler or runtime evaluation"));

    for reviewed in [values[0].clone(), values[1].clone()] {
        for requested_id in [
            reviewed["id"].as_str().expect("R16 entity ID"),
            reviewed["relationship_id"]
                .as_str()
                .expect("R16 relationship ID"),
        ] {
            let query = repository.query(requested_id);
            assert_success(&query, "R16 exact-ID query");
            let result = parse_single_document(&query.stdout);
            assert_eq!(
                result["schema_version"],
                "codenoesis.local-query-result/v13"
            );
            assert_eq!(result["requested_id"], requested_id);
            assert!(!result["claims"].as_array().is_none_or(Vec::is_empty));
            assert!(!result["evidence"].as_array().is_none_or(Vec::is_empty));
            assert!(
                result["linked_constant_derivations"]
                    .as_array()
                    .is_some_and(|items| !items.is_empty())
            );
        }
    }

    let export = repository.export();
    assert_success(&export, "R16 portable export");
    let portable_bytes =
        fs::read(repository.portable.join("portable-graph.json")).expect("read R16 graph");
    assert_eq!(export.stdout, portable_bytes);
    let portable = parse_single_document(&portable_bytes);
    assert_eq!(portable["schema_version"], "codenoesis.portable-graph/v9");
    assert_eq!(
        portable["source_snapshot"]["schema_version"],
        "codenoesis.repository-snapshot/v18"
    );
    assert_eq!(portable["entities"], graph["entities"]);
    assert_eq!(portable["relationships"], graph["relationships"]);
    assert_eq!(
        portable["constant_evaluation_index"],
        graph["constant_evaluation_index"]
    );
    assert_private(&portable);

    let explore = repository.explore();
    assert_success(&explore, "R16 local explorer");
    let manifest = parse_single_document(&explore.stdout);
    assert_eq!(manifest["schema_version"], "codenoesis.local-explorer/v9");
    assert_eq!(manifest["security"]["network"], false);
    assert_eq!(manifest["security"]["dynamic_code"], false);
    assert_matching_viewer_contract(&repository.explorer.join("index.html"), &manifest, 9);
    assert!(!repository.build_sentinel().exists());
}

#[test]
fn conf_fr_cli_001_r16_selector_absence_is_exact_r15() {
    let repository = MaterializedConstantEvaluationRepository::fixture();
    let output = repository.scan_r15();
    assert_success(&output, "R16 fixture through legacy R15 selectors");
    let expected = expected_safe_constant_evaluation();
    let observed_local_bytes = expected["baseline"]["canonical_stdout_bytes"]
        .as_u64()
        .expect("reviewed local R15 stdout length");
    assert!(observed_local_bytes <= 268_435_456);
    assert!(output.stdout.len() <= 268_435_456);
    assert_eq!(output.stdout.last(), Some(&b'\n'));
    let snapshot = parse_single_document(&output.stdout);
    assert_eq!(
        snapshot["schema_version"],
        "codenoesis.repository-snapshot/v17"
    );
    assert_eq!(
        snapshot["semantic_hash"]["value"],
        "ab76dc72a7cc57cf25f4112a551c047ad091693b32d0f732b1a5005f6e504908"
    );
    assert!(
        snapshot["semantic"]["configuration"]
            .get("rust_constant_profile")
            .is_none()
    );
    assert!(!repository.build_sentinel().exists());
}

#[test]
fn conf_fr_cli_001_r16_repository_boundary_fails_before_acquisition() {
    let repository = MaterializedConstantEvaluationRepository::fixture();
    let output =
        repository.scan_with_options(&["--repository-boundary-profile", "local-gitlinks-v1"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error = parse_single_document(&output.stderr);
    assert_eq!(error["schema_version"], "codenoesis.error/v24");
    assert_eq!(
        error["code"],
        "input.unsupported_rust_constant_evaluation_composition"
    );
    assert_eq!(
        error["context"]["reason"],
        "repository_boundary_not_supported"
    );
    assert!(!repository.store.exists());
    assert!(!repository.build_sentinel().exists());
}

#[test]
fn pt_nfr_det_001_r16_fifty_permutations_and_ten_schedules_are_identical() {
    let repository = MaterializedConstantEvaluationRepository::fixture();
    let expected_semantic = semantic_projection(
        &repository
            .permuted_scan_command(0)
            .output()
            .expect("run R16 argument permutation 0"),
    );
    for batch_start in (1_u64..49).step_by(10) {
        let batch_end = batch_start.saturating_add(10).min(49);
        let permutations = (batch_start..batch_end)
            .map(|seed| {
                let mut command = repository.permuted_scan_command(seed);
                (
                    seed,
                    std::thread::spawn(move || {
                        command.output().expect("run R16 argument permutation")
                    }),
                )
            })
            .collect::<Vec<_>>();
        for (seed, handle) in permutations {
            let output = handle.join().expect("join R16 argument permutation");
            assert_eq!(
                semantic_projection(&output),
                expected_semantic,
                "R16 semantic permutation {seed}"
            );
        }
    }
    let final_permutations = (49_u64..50).map(|seed| ("permutation", seed));
    let schedules = (100_u64..110).map(|seed| ("schedule", seed));
    let final_batch = final_permutations
        .chain(schedules)
        .map(|(kind, seed)| {
            let mut command = repository.permuted_scan_command(seed);
            (
                kind,
                seed,
                std::thread::spawn(move || command.output().expect("run R16 final batch")),
            )
        })
        .collect::<Vec<_>>();
    for (kind, seed, handle) in final_batch {
        let output = handle.join().expect("join R16 final batch");
        assert_eq!(
            semantic_projection(&output),
            expected_semantic,
            "R16 semantic {kind} {seed}"
        );
    }
    assert!(!repository.build_sentinel().exists());
}

fn count_by<'a>(values: &'a [Value], field: &str) -> BTreeMap<&'a str, u64> {
    let mut counts = BTreeMap::new();
    for value in values {
        if let Some(key) = value[field].as_str() {
            *counts.entry(key).or_insert(0) += 1;
        }
    }
    counts
}

fn assert_connected(entities: &[Value], relationships: &[Value]) {
    let entity_ids = entities
        .iter()
        .map(|entity| entity["id"].as_str().expect("R16 entity ID"))
        .collect::<BTreeSet<_>>();
    let mut adjacency = BTreeMap::<&str, Vec<&str>>::new();
    for relationship in relationships {
        let source = relationship["source"]
            .as_str()
            .expect("R16 relationship source");
        let target = relationship["target"]
            .as_str()
            .expect("R16 relationship target");
        assert!(entity_ids.contains(source), "dangling R16 source {source}");
        assert!(entity_ids.contains(target), "dangling R16 target {target}");
        adjacency.entry(source).or_default().push(target);
        adjacency.entry(target).or_default().push(source);
    }
    let start = *entity_ids.iter().next().expect("R16 graph has entities");
    let mut seen = BTreeSet::from([start]);
    let mut pending = VecDeque::from([start]);
    while let Some(subject) = pending.pop_front() {
        for neighbor in adjacency.get(subject).into_iter().flatten() {
            if seen.insert(neighbor) {
                pending.push_back(neighbor);
            }
        }
    }
    assert_eq!(seen.len(), entity_ids.len(), "R16 graph is not connected");
}

fn generated_markdown(root: &std::path::Path) -> String {
    let mut paths = Vec::new();
    collect_markdown_paths(root, &mut paths);
    paths.sort();
    paths.into_iter().fold(String::new(), |mut output, path| {
        output.push_str(&fs::read_to_string(path).expect("read R16 documentation"));
        output
    })
}

fn collect_markdown_paths(root: &std::path::Path, paths: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(root).expect("read R16 documentation root") {
        let path = entry.expect("R16 documentation entry").path();
        if path.is_dir() {
            collect_markdown_paths(&path, paths);
        } else if path.extension().and_then(std::ffi::OsStr::to_str) == Some("md") {
            paths.push(path);
        }
    }
}

fn assert_private(value: &Value) {
    let text = serde_json::to_string(value).expect("serialize R16 privacy projection");
    for forbidden in [
        "body_text",
        "initializer_text",
        "expression_text",
        "literal_lexeme",
        "source_snippet",
        "file://",
        "http://",
        "https://",
        "BEGIN PRIVATE KEY",
    ] {
        assert!(
            !text.contains(forbidden),
            "R16 private field leaked: {forbidden}"
        );
    }
}

fn semantic_projection(output: &Output) -> Vec<u8> {
    assert_success(output, "R16 deterministic scan");
    serde_json::to_vec(&parse_single_document(&output.stdout)["semantic"])
        .expect("serialize R16 semantic projection")
}

fn assert_success(output: &Output, label: &str) {
    assert!(
        output.status.success(),
        "{label} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn hex_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut output, byte| {
            use std::fmt::Write as _;
            write!(&mut output, "{byte:02x}").expect("write R16 SHA-256 hex");
            output
        })
}
