from __future__ import annotations

import hashlib
import json
import re
import unittest
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
SRS_PATH = ROOT / "docs/software/software-requirements-specification.md"
ROADMAP_PATH = ROOT / "docs/software/roadmap.md"
DECISION_PATH = (
    ROOT / "docs/software/decisions/0019-s4-k1-rust-callable-semantics-contract.md"
)
SPEC_ROOT = ROOT / "tests/specifications/s4/k1"
FIXTURE_ROOT = ROOT / "tests/fixtures/s4/rust-callable-semantics-v1"

SUBSET_PATH = SPEC_ROOT / "callable-semantics-subset-v1.json"
ONTOLOGY_PATH = SPEC_ROOT / "rust-ontology-v8.json"
CONFIGURATION_SCHEMA_PATH = SPEC_ROOT / "configuration-v8.schema.json"
CHUNK_SCHEMA_PATH = SPEC_ROOT / "extraction-chunk-v8.schema.json"
GRAPH_SCHEMA_PATH = SPEC_ROOT / "knowledge-graph-v8.schema.json"
SNAPSHOT_SCHEMA_PATH = SPEC_ROOT / "repository-snapshot-v11.schema.json"
QUERY_SCHEMA_PATH = SPEC_ROOT / "local-query-result-v6.schema.json"
PORTABLE_SCHEMA_PATH = SPEC_ROOT / "portable-graph-v2.schema.json"
EXPLORER_SCHEMA_PATH = SPEC_ROOT / "local-explorer-manifest-v2.schema.json"
ERROR_SCHEMA_PATH = SPEC_ROOT / "codenoesis-error-v16.schema.json"
HASH_CONTRACT_PATH = SPEC_ROOT / "semantic-hash-contract-v7.json"
INVALID_CASES_PATH = SPEC_ROOT / "invalid-cases-v1.json"
ORACLE_PATH = SPEC_ROOT / "e2e_fr_ext_012_rust_callable_semantics.json"
BUNDLE_PATH = SPEC_ROOT / "contract-bundle.json"
RED_OBSERVATION_PATH = SPEC_ROOT / "red-observation.json"
RED_LOG_PATH = SPEC_ROOT / "red/governance-red.log"

FIXTURE_MANIFEST_PATH = FIXTURE_ROOT / "manifest.json"
EXPECTED_FACTS_PATH = FIXTURE_ROOT / "expected-callable-semantics.json"

ISSUE_REFERENCE = "https://github.com/smutti/codenoesis/issues/142"
SCOPE_AUTHORIZATION_REFERENCE = (
    "https://github.com/smutti/codenoesis/issues/142#issuecomment-5225581916"
)
REQUIRED_BASE = "03ee09b172e84b5b7f5f423f9f65d63cf2953385"
R8_BUNDLE_FILE_SHA256 = (
    "29ad08558c3ed2dd3a6d143382655323b7fa7474e296eec0c0beab4ce1f4721f"
)
R8_BUNDLE_SHA256 = (
    "f8bba5eda9e43825f2fe31e0c55a37641a4d9213a8d94c6854bdfa290c39ca42"
)

VERSIONS = {
    "configuration": "codenoesis.configuration/v8",
    "ontology": "codenoesis.ontology/rust/v8",
    "extraction": "codenoesis.extraction/v8",
    "extraction_chunk": "codenoesis.extraction-chunk/v8",
    "knowledge_graph": "codenoesis.knowledge-graph/v8",
    "snapshot": "codenoesis.repository-snapshot/v11",
    "query": "codenoesis.local-query-result/v6",
    "portable_graph": "codenoesis.portable-graph/v2",
    "local_explorer": "codenoesis.local-explorer/v2",
    "error": "codenoesis.error/v16",
}

EXPECTED_ENTITY_COUNTS = {
    "rust.callable_signature": 9,
    "rust.parameter": 15,
    "rust.declared_value": 10,
    "rust.local_binding": 4,
    "rust.call_site": 9,
    "rust.control": 11,
}

EXPECTED_RELATIONSHIP_COUNTS = {
    "HAS_SIGNATURE": 9,
    "HAS_PARAMETER": 15,
    "DECLARES_VALUE": 10,
    "HAS_BODY_FACT": 24,
    "CALLS": 4,
}

LIMITS = {
    "callables_per_source": 4_096,
    "parameters_per_callable": 256,
    "body_facts_per_callable": 8_192,
    "body_fact_lexical_depth": 256,
    "signature_component_bytes": 4_096,
    "expression_metadata_bytes": 4_096,
    "entities_total": 200_000,
    "relationships_total": 400_000,
    "diagnostics_total": 50_000,
    "coverage_gaps_total": 50_000,
    "portable_graph_bytes": 268_435_456,
    "json_nesting": 64,
    "permutations": 50,
    "schedules": 10,
}

CONTROL_KINDS = {
    "if",
    "if_let",
    "match",
    "loop",
    "while",
    "while_let",
    "for",
    "return",
    "break",
    "continue",
    "try",
}

REQUIRED_TEST_NAMES = {
    "e2e_fr_ext_012_rust_callable_semantics",
    "gt_fr_ext_012_complete_signatures_and_parameters",
    "gt_fr_ext_012_declared_values_are_bounded_and_honest",
    "gt_fr_ext_012_calls_controls_and_lexical_nesting",
    "conf_fr_ext_012_snapshot_v11_graph_v8_error_v16",
    "conf_fr_qry_001_v11_uses_local_query_result_v6",
    "conf_fr_exp_002_portable_graph_v2_lossless_reimport",
    "conf_fr_exp_002_local_explorer_v2_is_offline",
    "pt_dr_idn_002_k1_identity_preimages_and_collisions",
    "pt_fr_ext_012_all_limits_have_maximum_plus_one",
    "pt_nfr_det_001_k1_fifty_permutations_ten_schedules",
    "sec_fr_ext_012_never_executes_target_or_toolchain",
    "sec_fr_exp_002_excludes_body_expression_and_source_text",
    "sec_fr_exp_002_xss_csp_path_and_race_fail_closed",
    "reg_fr_cli_001_selector_absence_preserves_r0_r8_bytes",
}

GOVERNANCE_PATHS = (
    DECISION_PATH,
    SUBSET_PATH,
    ONTOLOGY_PATH,
    CONFIGURATION_SCHEMA_PATH,
    CHUNK_SCHEMA_PATH,
    GRAPH_SCHEMA_PATH,
    SNAPSHOT_SCHEMA_PATH,
    QUERY_SCHEMA_PATH,
    PORTABLE_SCHEMA_PATH,
    EXPLORER_SCHEMA_PATH,
    ERROR_SCHEMA_PATH,
    HASH_CONTRACT_PATH,
    INVALID_CASES_PATH,
    ORACLE_PATH,
    BUNDLE_PATH,
    FIXTURE_ROOT / "README.md",
    FIXTURE_MANIFEST_PATH,
    EXPECTED_FACTS_PATH,
    FIXTURE_ROOT / "repository/Cargo.toml",
    FIXTURE_ROOT / "repository/build.rs",
    FIXTURE_ROOT / "repository/src/lib.rs",
    FIXTURE_ROOT / "repository/src/model.rs",
)

PRODUCTION_PATHS = (
    ROOT / "crates/codenoesis-domain/src/s4_k1.rs",
    ROOT / "crates/codenoesis-domain/tests/s4_k1_rust_callable_semantics.rs",
    ROOT / "crates/codenoesis-lang-rust/src/callable_semantics.rs",
    ROOT / "crates/codenoesis-lang-rust/tests/s4_k1_rust_callable_semantics.rs",
    ROOT / "crates/codenoesis-application/src/s4_k1.rs",
    ROOT / "crates/codenoesis-application/tests/s4_k1_rust_callable_semantics.rs",
    ROOT / "crates/codenoesis-contracts/src/s4_k1.rs",
    ROOT / "crates/codenoesis-contracts/tests/s4_k1_contracts.rs",
    ROOT / "crates/noesis/assets/s4/k1/index.html",
    ROOT / "crates/noesis/tests/e2e_fr_ext_012_rust_callable_semantics.rs",
    ROOT / "crates/noesis/tests/evidence/s4/k1/implementation-observation-local.json",
)

BUNDLE_FILES = {
    "LICENSE",
    "docs/software/decisions/0019-s4-k1-rust-callable-semantics-contract.md",
    "scripts/tests/test_s4_k1_rust_callable_semantics_contract.py",
    "tests/fixtures/s4/rust-callable-semantics-v1/README.md",
    "tests/fixtures/s4/rust-callable-semantics-v1/expected-callable-semantics.json",
    "tests/fixtures/s4/rust-callable-semantics-v1/manifest.json",
    "tests/fixtures/s4/rust-callable-semantics-v1/repository/Cargo.toml",
    "tests/fixtures/s4/rust-callable-semantics-v1/repository/build.rs",
    "tests/fixtures/s4/rust-callable-semantics-v1/repository/src/lib.rs",
    "tests/fixtures/s4/rust-callable-semantics-v1/repository/src/model.rs",
    "tests/specifications/s4/k1/callable-semantics-subset-v1.json",
    "tests/specifications/s4/k1/codenoesis-error-v16.schema.json",
    "tests/specifications/s4/k1/configuration-v8.schema.json",
    "tests/specifications/s4/k1/e2e_fr_ext_012_rust_callable_semantics.json",
    "tests/specifications/s4/k1/extraction-chunk-v8.schema.json",
    "tests/specifications/s4/k1/invalid-cases-v1.json",
    "tests/specifications/s4/k1/knowledge-graph-v8.schema.json",
    "tests/specifications/s4/k1/local-explorer-manifest-v2.schema.json",
    "tests/specifications/s4/k1/local-query-result-v6.schema.json",
    "tests/specifications/s4/k1/portable-graph-v2.schema.json",
    "tests/specifications/s4/k1/repository-snapshot-v11.schema.json",
    "tests/specifications/s4/k1/rust-ontology-v8.json",
    "tests/specifications/s4/k1/semantic-hash-contract-v7.json",
    "tests/specifications/s4/r8/contract-bundle.json",
}


def reject_duplicate_members(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON member: {key}")
        result[key] = value
    return result


def load_json(path: Path) -> Any:
    return json.loads(
        path.read_text(encoding="utf-8"),
        object_pairs_hook=reject_duplicate_members,
    )


def canonical_json(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_path(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def git_blob_oid(value: bytes) -> str:
    header = f"blob {len(value)}\0".encode("ascii")
    return hashlib.sha1(header + value).hexdigest()


class S4K1RustCallableSemanticsContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        missing = [path.relative_to(ROOT).as_posix() for path in GOVERNANCE_PATHS if not path.is_file()]
        if missing:
            raise AssertionError(f"K1 governance artifact is not materialized: {missing}")

    def test_srs_decision_and_roadmap_bind_the_candidate(self) -> None:
        srs = SRS_PATH.read_text(encoding="utf-8")
        decision = DECISION_PATH.read_text(encoding="utf-8")
        roadmap = ROADMAP_PATH.read_text(encoding="utf-8")
        for value in (
            "0.9+k1",
            "### 2.19 S4 K1 Rust callable and value semantics candidate register",
            ISSUE_REFERENCE,
            SCOPE_AUTHORIZATION_REFERENCE,
            REQUIRED_BASE,
            "FR-EXT-012",
            "FR-EXP-002",
            "codenoesis.repository-snapshot/v11",
            "codenoesis.portable-graph/v2",
            "codenoesis.local-explorer/v2",
        ):
            self.assertIn(value, srs)
        for value in (
            ISSUE_REFERENCE,
            SCOPE_AUTHORIZATION_REFERENCE,
            REQUIRED_BASE,
            "source-only lexical call semantics",
            "candidate_unresolved",
            "No dependency is added",
            "protected manual merge",
        ):
            self.assertIn(value, decision)
        self.assertIn("| `K1` | Deterministic Rust callable and value semantics |", roadmap)
        self.assertIn("R0-R8 are implemented", roadmap)

    def test_machine_oracle_fixes_scope_versions_counts_and_limits(self) -> None:
        oracle = load_json(ORACLE_PATH)
        self.assertEqual(oracle["issue"], ISSUE_REFERENCE)
        self.assertEqual(oracle["scope_expansion_authorization"], SCOPE_AUTHORIZATION_REFERENCE)
        self.assertEqual(oracle["required_base"], REQUIRED_BASE)
        self.assertEqual(oracle["slice"], "S4")
        self.assertEqual(oracle["roadmap_capability"], "K1")
        self.assertEqual(oracle["risk"], "high")
        self.assertEqual(oracle["correction_rounds"], 5)
        self.assertEqual(oracle["dependencies"], [])
        self.assertEqual(oracle["contracts"], VERSIONS)
        self.assertEqual(oracle["limits"], LIMITS)
        self.assertEqual(oracle["expected_fixture"]["new_entity_counts"], EXPECTED_ENTITY_COUNTS)
        self.assertEqual(
            oracle["expected_fixture"]["new_relationship_counts"],
            EXPECTED_RELATIONSHIP_COUNTS,
        )
        self.assertEqual(set(oracle["required_test_names"]), REQUIRED_TEST_NAMES)
        self.assertIn("crates/codenoesis-domain/src/storage.rs", oracle["allowed_paths"])
        self.assertIn("crates/codenoesis-contracts/src/s4.rs", oracle["allowed_paths"])
        self.assertIn("Cargo.lock", oracle["protected_paths"])
        self.assertFalse(oracle["selector"]["implicit_selection"])
        self.assertFalse(oracle["selector"]["compiler_index_composition"])
        self.assertTrue(all(value is False for value in oracle["privacy"].values()))

    def test_subset_and_ontology_keep_syntax_distinct_from_runtime(self) -> None:
        subset = load_json(SUBSET_PATH)
        ontology = load_json(ONTOLOGY_PATH)
        self.assertEqual(set(subset["control_kinds"]), CONTROL_KINDS)
        self.assertEqual(
            set(subset["relationship_kinds"]),
            set(EXPECTED_RELATIONSHIP_COUNTS),
        )
        self.assertFalse(subset["source_body_exported"])
        self.assertFalse(subset["arbitrary_expression_exported"])
        self.assertFalse(subset["compiler_cfg_claimed"])
        self.assertFalse(subset["reachability_claimed"])
        self.assertFalse(subset["data_flow_claimed"])
        self.assertFalse(subset["runtime_behavior_claimed"])
        self.assertEqual(ontology["ontology_version"], VERSIONS["ontology"])
        self.assertEqual(set(ontology["entity_kinds"]), set(EXPECTED_ENTITY_COUNTS))
        self.assertEqual(set(ontology["relationship_kinds"]), set(EXPECTED_RELATIONSHIP_COUNTS))
        self.assertEqual(
            set(ontology["forbidden_relationship_kinds"]),
            {"EXECUTES", "REACHES", "READS", "WRITES"},
        )
        self.assertTrue(ontology["preserves_existing_ids"])

    def test_schemas_fix_every_additive_public_version(self) -> None:
        expected = {
            CONFIGURATION_SCHEMA_PATH: ("schema_version", VERSIONS["configuration"]),
            CHUNK_SCHEMA_PATH: ("schema_version", VERSIONS["extraction_chunk"]),
            GRAPH_SCHEMA_PATH: ("schema_version", VERSIONS["knowledge_graph"]),
            SNAPSHOT_SCHEMA_PATH: ("schema_version", VERSIONS["snapshot"]),
            QUERY_SCHEMA_PATH: ("schema_version", VERSIONS["query"]),
            PORTABLE_SCHEMA_PATH: ("schema_version", VERSIONS["portable_graph"]),
            EXPLORER_SCHEMA_PATH: ("schema_version", VERSIONS["local_explorer"]),
            ERROR_SCHEMA_PATH: ("schema_version", VERSIONS["error"]),
        }
        for schema_path, (field, version) in expected.items():
            with self.subTest(schema=schema_path.name):
                schema = load_json(schema_path)
                self.assertEqual(schema["$schema"], "https://json-schema.org/draft/2020-12/schema")
                self.assertEqual(schema["type"], "object")
                properties = schema["properties"]
                self.assertEqual(properties[field]["const"], version)
        portable_text = PORTABLE_SCHEMA_PATH.read_text(encoding="utf-8")
        for forbidden in ("body_text", "expression_text", "source_contents", "source_snippet"):
            self.assertNotIn(forbidden, portable_text)

    def test_fixture_manifest_binds_exact_owned_bytes(self) -> None:
        manifest = load_json(FIXTURE_MANIFEST_PATH)
        expected = load_json(EXPECTED_FACTS_PATH)
        self.assertEqual(manifest["repository_identity"], expected["repository_identity"])
        self.assertEqual(manifest["materialization"]["commit_oid"], "9a7bb3adaa5bf30eef3bc9bc656c81f42fbdb845")
        self.assertEqual(manifest["materialization"]["tree_oid"], "ead855e0545cc26b351b305fcad39f2e491b285d")
        self.assertFalse(manifest["external_source_vendored"])
        for entry in manifest["files"]:
            path = FIXTURE_ROOT / entry["path"]
            value = path.read_bytes()
            self.assertEqual(len(value), entry["byte_length"])
            self.assertEqual(sha256_bytes(value), entry["sha256"])
            self.assertEqual(git_blob_oid(value), entry["git_blob_oid"])
        expected_entry = manifest["expected_facts"]
        value = EXPECTED_FACTS_PATH.read_bytes()
        self.assertEqual(len(value), expected_entry["byte_length"])
        self.assertEqual(sha256_bytes(value), expected_entry["sha256"])
        self.assertTrue(all(value is False for key, value in manifest["sentinels"].items() if key.endswith("_permitted")))
        source = (FIXTURE_ROOT / "repository/src/lib.rs").read_text(encoding="utf-8")
        for token in ("if let", "while let", "match", "loop", "for item", "fallible(total)?"):
            self.assertIn(token, source)

    def test_expected_golden_fixes_values_calls_controls_and_negatives(self) -> None:
        expected = load_json(EXPECTED_FACTS_PATH)
        self.assertEqual(expected["new_entity_counts"], EXPECTED_ENTITY_COUNTS)
        self.assertEqual(expected["new_relationship_counts"], EXPECTED_RELATIONSHIP_COUNTS)
        self.assertEqual(expected["value_states"], {"normalized_scalar": 7, "expression_only": 2, "unresolved": 1})
        self.assertEqual(set(expected["control_kinds"]), CONTROL_KINDS)
        self.assertEqual(len(expected["resolved_call_targets"]), 4)
        self.assertEqual(len(expected["unresolved_call_spellings"]), 5)
        self.assertEqual(
            set(expected["expression_only_declarations"]),
            {"COMPUTED", "Computed"},
        )
        self.assertEqual(expected["unresolved_declarations"], ["Pending"])
        self.assertIn("no_body_text_export", expected["hard_negative_labels"])

    def test_invalid_corpus_covers_limits_integrity_privacy_and_viewer_security(self) -> None:
        cases = load_json(INVALID_CASES_PATH)["cases"]
        identifiers = {case["id"] for case in cases}
        self.assertEqual(len(identifiers), len(cases))
        for required in (
            "compiler_profile_composition",
            "unicode_nfc_identity_collision",
            "parameters_max_plus_one",
            "body_facts_max_plus_one",
            "dangling_calls_target",
            "portable_body_text",
            "portable_expression_text",
            "output_race_replacement",
            "html_script_close",
            "remote_resource",
            "dynamic_code",
        ):
            self.assertIn(required, identifiers)

    def test_r8_contract_bundle_is_immutable(self) -> None:
        path = ROOT / "tests/specifications/s4/r8/contract-bundle.json"
        self.assertEqual(sha256_path(path), R8_BUNDLE_FILE_SHA256)
        self.assertEqual(load_json(path)["bundle_sha256"], R8_BUNDLE_SHA256)

    def test_contract_bundle_binds_every_semantic_checkpoint_artifact(self) -> None:
        bundle = load_json(BUNDLE_PATH)
        self.assertEqual(bundle["schema_version"], "codenoesis.contract-bundle/v1")
        files = bundle["files"]
        paths = [entry["path"] for entry in files]
        self.assertEqual(paths, sorted(paths))
        self.assertEqual(set(paths), BUNDLE_FILES)
        self.assertEqual(len(paths), len(set(paths)))
        for entry in files:
            self.assertEqual(entry["sha256"], sha256_path(ROOT / entry["path"]))
        payload = {"schema_version": bundle["schema_version"], "files": files}
        self.assertEqual(bundle["bundle_sha256"], sha256_bytes(canonical_json(payload)))
        self.assertIn(bundle["bundle_sha256"], SRS_PATH.read_text(encoding="utf-8"))

    def test_production_candidate_is_materialized_after_expected_red(self) -> None:
        missing = [path.relative_to(ROOT).as_posix() for path in PRODUCTION_PATHS if not path.is_file()]
        self.assertEqual(
            missing,
            [],
            "expected K1 Red: production modules, CLI/viewer behavior, and implementation evidence are not materialized",
        )
        combined = "\n".join(path.read_text(encoding="utf-8") for path in PRODUCTION_PATHS if path.suffix in {".rs", ".json"})
        for version in VERSIONS.values():
            self.assertIn(version, combined)
        for test_name in REQUIRED_TEST_NAMES:
            self.assertIn(test_name, combined)

    def test_retained_red_is_required_once_product_exists(self) -> None:
        if any(not path.is_file() for path in PRODUCTION_PATHS):
            return
        self.assertTrue(RED_OBSERVATION_PATH.is_file())
        self.assertTrue(RED_LOG_PATH.is_file())
        observation = load_json(RED_OBSERVATION_PATH)
        self.assertEqual(observation["command"], "python3 -m unittest scripts.tests.test_s4_k1_rust_callable_semantics_contract")
        self.assertEqual(observation["exit_status"], 1)
        self.assertRegex(observation["checkpoint_commit_sha"], r"^[0-9a-f]{40}$")
        self.assertEqual(observation["raw_log_sha256"], sha256_path(RED_LOG_PATH))
        self.assertIn("expected K1 Red", RED_LOG_PATH.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
