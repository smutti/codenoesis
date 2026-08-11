mod support;

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::process::Output;

use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use support::parse_single_document;
use support::s4_r15::{MaterializedLocalFlowRepository, expected_local_flow};

const EXPECTED_RED_STDERR_SHA256: &str =
    "7f75f7a91f6af0328795f3fbd2729e69756beba2ebd642cc1f6401265662a2fe";

#[test]
#[allow(clippy::too_many_lines)]
fn e2e_fr_ext_017_rust_local_flow_complete_local_journey() {
    let repository = MaterializedLocalFlowRepository::fixture();
    assert!(!repository.build_sentinel().exists());
    let scan = repository.scan();

    if scan.status.code() == Some(2) {
        assert!(scan.stdout.is_empty(), "R15 expected-Red stdout changed");
        assert_eq!(scan.stderr.len(), 149, "R15 expected-Red length changed");
        assert_eq!(
            hex_sha256(&scan.stderr),
            EXPECTED_RED_STDERR_SHA256,
            "R15 expected-Red digest changed"
        );
        let error = parse_single_document(&scan.stderr);
        assert_eq!(error["schema_version"], "codenoesis.error/v4");
        assert_eq!(error["code"], "input.invalid_revision");
        assert_eq!(error["stage"], "input");
        assert!(!repository.store.exists(), "R15 expected Red mutated store");
        assert!(!repository.build_sentinel().exists());
        panic!("expected RepositorySnapshotV17 success; observed frozen selector Red");
    }

    assert_success(&scan, "R15 local-flow scan");
    assert!(scan.stderr.is_empty());
    let snapshot = parse_single_document(&scan.stdout);
    assert_eq!(
        snapshot["schema_version"],
        "codenoesis.repository-snapshot/v17"
    );
    assert_eq!(
        snapshot["semantic"]["configuration"]["schema_version"],
        "codenoesis.configuration/v14"
    );
    assert_eq!(
        snapshot["semantic"]["configuration"]["rust_flow_profile"],
        "rust-local-flow-v1"
    );
    assert_eq!(
        snapshot["semantic"]["pipeline_version"],
        "codenoesis.pipeline/s4-r15-v1"
    );

    let expected = expected_local_flow();
    let graph = &snapshot["semantic"]["knowledge_graph"];
    assert_eq!(graph["schema_version"], "codenoesis.knowledge-graph/v14");
    assert_eq!(graph["ontology_version"], "codenoesis.ontology/rust/v14");
    for (family, count) in expected["complete_counts"]
        .as_object()
        .expect("R15 complete counts")
    {
        if matches!(family.as_str(), "deterministic_claims" | "derived_claims") {
            continue;
        }
        assert_eq!(
            graph[family].as_array().map_or(0, Vec::len),
            usize::try_from(count.as_u64().expect("R15 family count"))
                .expect("R15 count fits usize"),
            "R15 graph family {family}"
        );
    }

    let entities = graph["entities"].as_array().expect("R15 entities");
    let blocks = entities
        .iter()
        .filter(|entity| entity["kind"] == "rust.syntax_basic_block")
        .collect::<Vec<_>>();
    assert_eq!(blocks.len(), 5);
    for reviewed in expected["blocks"].as_array().expect("reviewed R15 blocks") {
        let actual = blocks
            .iter()
            .copied()
            .find(|entity| entity["id"] == reviewed["id"])
            .expect("reviewed R15 block exists");
        assert_eq!(actual["callable_id"], expected["callable_id"]);
        assert_eq!(actual["source_file_id"], expected["source_file_id"]);
        assert_eq!(actual["evidence_id"], reviewed["evidence_id"]);
        assert_eq!(actual["locator"]["path"], "src/lib.rs");
        assert_eq!(actual["locator"]["start_byte"], reviewed["start_byte"]);
        assert_eq!(actual["locator"]["end_byte"], reviewed["end_byte"]);
        assert_eq!(actual["properties"]["ordinal"], reviewed["ordinal"]);
        assert_eq!(actual["properties"]["role"], reviewed["role"]);
        assert_eq!(actual["properties"]["flow_world"], reviewed["flow_world"]);
        assert_eq!(
            actual["properties"]["flow_node_ids"],
            reviewed["flow_node_ids"]
        );
    }

    let relationships = graph["relationships"]
        .as_array()
        .expect("R15 relationships");
    let reviewed_kinds = expected["relationship_kind_counts"]
        .as_object()
        .expect("R15 kind counts");
    let actual_additive = relationships
        .iter()
        .filter(|relationship| {
            relationship["kind"]
                .as_str()
                .is_some_and(|kind| reviewed_kinds.contains_key(kind))
        })
        .map(project_relationship)
        .collect::<BTreeMap<_, _>>();
    let reviewed_additive = expected["relationships"]
        .as_array()
        .expect("reviewed R15 relationships")
        .iter()
        .map(project_relationship)
        .collect::<BTreeMap<_, _>>();
    assert_eq!(actual_additive, reviewed_additive);

    let claims = graph["claims"].as_array().expect("R15 claims");
    assert_eq!(
        count_by(claims, "state").get("deterministic_fact"),
        Some(&101)
    );
    assert_eq!(count_by(claims, "state").get("derived_fact"), Some(&21));
    for reviewed in expected["blocks"]
        .as_array()
        .expect("reviewed blocks")
        .iter()
        .chain(
            expected["relationships"]
                .as_array()
                .expect("reviewed relationships"),
        )
    {
        let subject_id = reviewed["id"].as_str().expect("reviewed subject ID");
        let claim = claims
            .iter()
            .find(|claim| claim["subject_id"] == subject_id)
            .expect("R15 claim exists");
        assert_eq!(claim["id"], reviewed["claim_id"]);
        let expected_state = reviewed["state"].as_str().unwrap_or("deterministic_fact");
        assert_eq!(claim["state"], expected_state);
    }

    let additive_evidence = graph["evidence"]
        .as_array()
        .expect("R15 evidence")
        .iter()
        .filter(|evidence| {
            expected["evidence"]
                .as_array()
                .expect("reviewed evidence")
                .iter()
                .any(|reviewed| reviewed["id"] == evidence["id"])
        })
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(additive_evidence.len(), 5);

    let index = &graph["local_flow_index"];
    assert_eq!(index["schema_version"], "codenoesis.local-flow-index/v1");
    assert_eq!(index["rule_version"], "codenoesis.rule/rust-local-flow/v1");
    assert_eq!(
        index["completed_callable_ids"],
        json!([expected["callable_id"]])
    );
    assert_eq!(index["block_entity_ids"].as_array().map(Vec::len), Some(5));
    assert_eq!(
        index["flow_node_relationship_ids"].as_array().map(Vec::len),
        Some(9)
    );
    assert_eq!(
        index["condition_relationship_ids"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(
        index["direct_syntax_relationship_ids"]
            .as_array()
            .map(Vec::len),
        Some(5)
    );
    assert_eq!(
        index["reachability_relationship_ids"]
            .as_array()
            .map(Vec::len),
        Some(9)
    );
    assert_eq!(
        index["must_reach_relationship_ids"]
            .as_array()
            .map(Vec::len),
        Some(5)
    );
    assert_eq!(
        index["may_reach_relationship_ids"].as_array().map(Vec::len),
        Some(2)
    );
    let derivations = index["derivations"].as_array().expect("R15 derivations");
    assert_eq!(derivations.len(), 16);
    for reviewed in expected["relationships"]
        .as_array()
        .expect("reviewed relationships")
        .iter()
        .filter(|relationship| relationship["state"] == "derived_fact")
    {
        let derivation = derivations
            .iter()
            .find(|value| value["relationship_id"] == reviewed["id"])
            .expect("reviewed derivation exists");
        assert_eq!(
            derivation["rule_version"],
            "codenoesis.rule/rust-local-flow/v1"
        );
        assert_eq!(
            derivation["input_entity_ids"],
            reviewed["inputs"]["entity_ids"]
        );
        assert_eq!(
            derivation["input_relationship_ids"],
            reviewed["inputs"]["relationship_ids"]
        );
        assert_eq!(
            derivation["input_evidence_ids"],
            reviewed["inputs"]["evidence_ids"]
        );
    }
    let additive_subject_ids = expected["blocks"]
        .as_array()
        .expect("R15 blocks")
        .iter()
        .chain(
            expected["relationships"]
                .as_array()
                .expect("R15 relationships"),
        )
        .map(|value| value["id"].as_str().expect("R15 additive subject"))
        .collect::<BTreeSet<_>>();
    for (family, values) in [
        ("entities", blocks.clone()),
        (
            "relationships",
            relationships
                .iter()
                .filter(|relationship| {
                    relationship["kind"]
                        .as_str()
                        .is_some_and(|kind| reviewed_kinds.contains_key(kind))
                })
                .collect::<Vec<_>>(),
        ),
        (
            "claims",
            claims
                .iter()
                .filter(|claim| {
                    claim["subject_id"]
                        .as_str()
                        .is_some_and(|identifier| additive_subject_ids.contains(identifier))
                })
                .collect::<Vec<_>>(),
        ),
        ("evidence", additive_evidence.iter().collect::<Vec<_>>()),
    ] {
        assert_eq!(
            family_id_digest(&values),
            expected["family_id_sha256"][family]
                .as_str()
                .expect("reviewed R15 family digest"),
            "R15 additive family ID digest {family}"
        );
    }
    let derivation_projection = derivations
        .iter()
        .map(|derivation| {
            json!({
                "relationship_id": derivation["relationship_id"],
                "inputs": {
                    "entity_ids": derivation["input_entity_ids"],
                    "relationship_ids": derivation["input_relationship_ids"],
                    "evidence_ids": derivation["input_evidence_ids"]
                }
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        hex_sha256(
            &serde_json::to_vec(&derivation_projection)
                .expect("serialize R15 derivation projection")
        ),
        expected["family_id_sha256"]["derivations"]
            .as_str()
            .expect("reviewed R15 derivation digest")
    );

    let coverage = graph["coverage"].as_array().expect("R15 coverage");
    for capability in expected["required_inherited_coverage"]
        .as_array()
        .expect("required inherited coverage")
    {
        assert!(
            coverage.iter().any(|gap| gap["capability"] == *capability),
            "R15 removed inherited coverage {capability}"
        );
    }
    assert_connected(entities, relationships);

    assert_success(&repository.docs(), "R15 documentation generation");
    let documentation = generated_markdown(&repository.documents);
    assert!(documentation.contains("Syntax-normal local flow"));
    assert!(documentation.contains("not compiler or runtime control flow"));

    for requested_id in [
        expected["blocks"][0]["id"].as_str().expect("R15 block ID"),
        expected["relationships"]
            .as_array()
            .expect("R15 relationships")
            .iter()
            .find(|value| value["kind"] == "HAS_CONDITION")
            .and_then(|value| value["id"].as_str())
            .expect("R15 condition ID"),
        expected["relationships"]
            .as_array()
            .expect("R15 relationships")
            .iter()
            .find(|value| value["kind"] == "SYNTAX_REACHES")
            .and_then(|value| value["id"].as_str())
            .expect("R15 reachability ID"),
        expected["relationships"]
            .as_array()
            .expect("R15 relationships")
            .iter()
            .find(|value| value["kind"] == "LEXICAL_MAY_REACHES_READ")
            .and_then(|value| value["id"].as_str())
            .expect("R15 def-use ID"),
    ] {
        let query = repository.query(requested_id);
        assert_success(&query, "R15 exact-ID query");
        let result = parse_single_document(&query.stdout);
        assert_eq!(
            result["schema_version"],
            "codenoesis.local-query-result/v12"
        );
        assert_eq!(result["requested_id"], requested_id);
        assert!(!result["claims"].as_array().is_none_or(Vec::is_empty));
        assert!(!result["evidence"].as_array().is_none_or(Vec::is_empty));
        if requested_id
            == expected["relationships"]
                .as_array()
                .expect("R15 relationships")
                .iter()
                .find(|value| value["kind"] == "SYNTAX_REACHES")
                .and_then(|value| value["id"].as_str())
                .expect("R15 reachability ID")
            || requested_id
                == expected["relationships"]
                    .as_array()
                    .expect("R15 relationships")
                    .iter()
                    .find(|value| value["kind"] == "LEXICAL_MAY_REACHES_READ")
                    .and_then(|value| value["id"].as_str())
                    .expect("R15 def-use ID")
        {
            assert!(
                result["linked_derivations"]
                    .as_array()
                    .is_some_and(|values| values
                        .iter()
                        .any(|value| { value["relationship_id"].as_str() == Some(requested_id) }))
            );
        }
    }

    let export = repository.export();
    assert_success(&export, "R15 portable export");
    let portable_bytes =
        fs::read(repository.portable.join("portable-graph.json")).expect("read R15 portable graph");
    assert_eq!(export.stdout, portable_bytes);
    let portable = parse_single_document(&portable_bytes);
    assert_eq!(portable["schema_version"], "codenoesis.portable-graph/v8");
    assert_eq!(
        portable["source_snapshot"]["schema_version"],
        "codenoesis.repository-snapshot/v17"
    );
    assert_eq!(portable["entities"], graph["entities"]);
    assert_eq!(portable["relationships"], graph["relationships"]);
    assert_eq!(portable["local_flow_index"], graph["local_flow_index"]);
    assert_private(&portable);

    let explore = repository.explore();
    assert_success(&explore, "R15 local explorer");
    let manifest = parse_single_document(&explore.stdout);
    assert_eq!(manifest["schema_version"], "codenoesis.local-explorer/v8");
    assert_eq!(manifest["security"]["network"], false);
    assert_eq!(manifest["security"]["dynamic_code"], false);
    let viewer = fs::read(repository.explorer.join("index.html")).expect("read R15 viewer");
    let immutable = normalize_lf(
        fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/s4/k1/index.html"))
            .expect("read immutable K1 viewer")
            .as_slice(),
    );
    assert_eq!(viewer, immutable, "R15 viewer bytes changed");
    assert!(!repository.build_sentinel().exists());
}

#[test]
fn conf_fr_cli_001_r15_selector_absence_is_exact_r14() {
    let repository = MaterializedLocalFlowRepository::fixture();
    let output = repository.scan_r14();
    assert_success(&output, "R15 fixture through legacy R14 selectors");
    let snapshot = parse_single_document(&output.stdout);
    assert_eq!(
        snapshot["schema_version"],
        "codenoesis.repository-snapshot/v16"
    );
    assert_eq!(
        snapshot["semantic"]["knowledge_graph"]["semantic_hash"]["value"],
        "9234051e60d266305da77fa5a750c64f42a22d719282a9b0afd7b93461213003"
    );
    assert!(
        snapshot["semantic"]["configuration"]
            .get("rust_flow_profile")
            .is_none()
    );
    assert!(!repository.build_sentinel().exists());
}

#[test]
fn conf_fr_cli_001_r15_forbidden_composition_fails_before_acquisition() {
    let repository = MaterializedLocalFlowRepository::fixture();
    let output =
        repository.scan_with_options(&["--repository-boundary-profile", "root-gitlinks-v1"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error = parse_single_document(&output.stderr);
    assert_eq!(error["schema_version"], "codenoesis.error/v22");
    assert_eq!(error["code"], "input.unsupported_rust_flow_composition");
    assert_eq!(error["stage"], "input");
    assert!(!repository.store.exists());
    assert!(!repository.build_sentinel().exists());
}

#[test]
fn pt_nfr_det_001_r15_fifty_permutations_and_ten_schedules_are_identical() {
    let repository = MaterializedLocalFlowRepository::fixture();
    let expected_semantic = semantic_projection(
        &repository
            .permuted_scan_command(0)
            .output()
            .expect("run R15 argument permutation 0"),
    );
    for batch_start in (1_u64..49).step_by(10) {
        let batch_end = batch_start.saturating_add(10).min(49);
        let permutations = (batch_start..batch_end)
            .map(|seed| {
                let mut command = repository.permuted_scan_command(seed);
                (
                    seed,
                    std::thread::spawn(move || {
                        command.output().expect("run R15 argument permutation")
                    }),
                )
            })
            .collect::<Vec<_>>();
        for (seed, handle) in permutations {
            let output = handle.join().expect("join R15 argument permutation");
            assert_eq!(
                semantic_projection(&output),
                expected_semantic,
                "R15 semantic permutation {seed}"
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
                std::thread::spawn(move || command.output().expect("run R15 final batch")),
            )
        })
        .collect::<Vec<_>>();
    for (kind, seed, handle) in final_batch {
        let output = handle.join().expect("join R15 final batch");
        assert_eq!(
            semantic_projection(&output),
            expected_semantic,
            "R15 semantic {kind} {seed}"
        );
    }
    assert!(!repository.build_sentinel().exists());
}

fn project_relationship(value: &Value) -> (String, Value) {
    let id = value["id"]
        .as_str()
        .expect("R15 relationship ID")
        .to_owned();
    (
        id,
        json!({
            "id": value["id"],
            "kind": value["kind"],
            "source": value["source"],
            "target": value["target"],
            "evidence_ids": value["evidence_ids"],
        }),
    )
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
        .map(|entity| entity["id"].as_str().expect("entity ID"))
        .collect::<BTreeSet<_>>();
    let mut adjacency = BTreeMap::<&str, Vec<&str>>::new();
    for relationship in relationships {
        let source = relationship["source"]
            .as_str()
            .expect("relationship source");
        let target = relationship["target"]
            .as_str()
            .expect("relationship target");
        assert!(entity_ids.contains(source), "dangling source {source}");
        assert!(entity_ids.contains(target), "dangling target {target}");
        adjacency.entry(source).or_default().push(target);
        adjacency.entry(target).or_default().push(source);
    }
    let start = *entity_ids.iter().next().expect("R15 graph has entities");
    let mut seen = BTreeSet::from([start]);
    let mut pending = VecDeque::from([start]);
    while let Some(subject) = pending.pop_front() {
        for neighbor in adjacency.get(subject).into_iter().flatten() {
            if seen.insert(neighbor) {
                pending.push_back(neighbor);
            }
        }
    }
    assert_eq!(seen.len(), entity_ids.len(), "R15 graph is not connected");
}

fn generated_markdown(root: &std::path::Path) -> String {
    let mut paths = Vec::new();
    collect_markdown_paths(root, &mut paths);
    paths.sort();
    paths.into_iter().fold(String::new(), |mut output, path| {
        output.push_str(&fs::read_to_string(path).expect("read R15 documentation"));
        output
    })
}

fn collect_markdown_paths(root: &std::path::Path, paths: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(root).expect("read R15 documentation root") {
        let path = entry.expect("R15 documentation entry").path();
        if path.is_dir() {
            collect_markdown_paths(&path, paths);
        } else if path.extension().and_then(std::ffi::OsStr::to_str) == Some("md") {
            paths.push(path);
        }
    }
}

fn assert_private(value: &Value) {
    let text = serde_json::to_string(value).expect("serialize R15 privacy projection");
    for forbidden in [
        "body_text",
        "initializer_text",
        "expression_text",
        "condition_text",
        "literal_lexeme",
        "source_snippet",
        "file://",
        "http://",
        "https://",
        "BEGIN PRIVATE KEY",
    ] {
        assert!(
            !text.contains(forbidden),
            "R15 private field leaked: {forbidden}"
        );
    }
}

fn semantic_projection(output: &Output) -> Vec<u8> {
    assert_success(output, "R15 deterministic scan");
    serde_json::to_vec(&parse_single_document(&output.stdout)["semantic"])
        .expect("serialize R15 semantic projection")
}

fn assert_success(output: &Output, label: &str) {
    assert!(
        output.status.success(),
        "{label} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn normalize_lf(bytes: &[u8]) -> Vec<u8> {
    let mut normalized = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\r' && bytes.get(index + 1) == Some(&b'\n') {
            normalized.push(b'\n');
            index += 2;
        } else {
            normalized.push(bytes[index]);
            index += 1;
        }
    }
    normalized
}

fn hex_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut output, byte| {
            use std::fmt::Write as _;
            write!(&mut output, "{byte:02x}").expect("write R15 SHA-256 hex");
            output
        })
}

fn family_id_digest(values: &[&Value]) -> String {
    let mut identifiers = values
        .iter()
        .map(|value| value["id"].as_str().expect("R15 family ID"))
        .collect::<Vec<_>>();
    identifiers.sort_unstable();
    let mut payload = identifiers.join("\n").into_bytes();
    payload.push(b'\n');
    hex_sha256(&payload)
}
