from __future__ import annotations

import hashlib
import json
import re
import unicodedata
import unittest
from pathlib import Path
from typing import Any, Iterator
from urllib.parse import unquote

from scripts.tests.test_s1_contract import blake3_256


ROOT = Path(__file__).resolve().parents[2]
SRS_PATH = ROOT / "docs/software/software-requirements-specification.md"
ROADMAP_PATH = ROOT / "docs/software/roadmap.md"
DECISION_PATH = (
    ROOT / "docs/software/decisions/0016-s4-r6-framework-declarations-contract.md"
)
SPEC_ROOT = ROOT / "tests/specifications/s4/r6"
SUBSET_PATH = SPEC_ROOT / "framework-declarations-subset-v1.json"
CONFIGURATION_SCHEMA_PATH = SPEC_ROOT / "configuration-v6.schema.json"
SNAPSHOT_SCHEMA_PATH = SPEC_ROOT / "repository-snapshot-v9.schema.json"
CHUNK_SCHEMA_PATH = SPEC_ROOT / "extraction-chunk-v6.schema.json"
GRAPH_SCHEMA_PATH = SPEC_ROOT / "knowledge-graph-v6.schema.json"
ONTOLOGY_PATH = SPEC_ROOT / "rust-ontology-v6.json"
ERROR_SCHEMA_PATH = SPEC_ROOT / "codenoesis-error-v13.schema.json"
QUERY_SCHEMA_PATH = SPEC_ROOT / "local-query-result-v4.schema.json"
HASH_CONTRACT_PATH = SPEC_ROOT / "semantic-hash-contract-v5.json"
ORACLE_PATH = SPEC_ROOT / "e2e_fr_ext_011_framework_declarations.json"
QUERY_ORACLE_PATH = SPEC_ROOT / "e2e_fr_qry_001_r6_exact_id_results.json"
INVALID_CASES_PATH = SPEC_ROOT / "invalid-cases-v1.json"
PILOT_OBSERVATIONS_PATH = SPEC_ROOT / "pilot-observations-v1.json"
RED_OBSERVATION_PATH = SPEC_ROOT / "red-observation.json"
RED_LOG_PATH = SPEC_ROOT / "red/governance-red.log"
BUNDLE_PATH = SPEC_ROOT / "contract-bundle.json"
FIXTURE_ROOT = ROOT / "tests/fixtures/s4/framework-declarations-v1"
FIXTURE_REPOSITORY = FIXTURE_ROOT / "repository"
FIXTURE_MANIFEST_PATH = FIXTURE_ROOT / "manifest.json"
EXPECTED_FACTS_PATH = FIXTURE_ROOT / "expected-framework-declarations.json"

ISSUE_REFERENCE = "https://github.com/smutti/codenoesis/issues/117"
AUTHORIZATION_REFERENCE = (
    "https://github.com/smutti/codenoesis/issues/117#issuecomment-5183312890"
)
REQUIRED_BASE = "6750f293c24ea501df6177a2f7c96c2c7f0a6390"
REPOSITORY_IDENTITY = "urn:codenoesis:fixture:s4-framework-declarations-v1"
IDENTITY_DOMAIN = "codenoesis.entity-id/framework-declaration/v1"
TEST_FIRST_GUARD_SHA256 = (
    "2dc7e2627165f6879733562b1459216365f6a0f0d9f6eceed7e37f65d1c3a48f"
)

LIMITS = {
    "framework_declarations_per_source": 4_096,
    "explicit_registration_chain_segments": 256,
    "registration_expression_depth": 64,
    "literal_route_path_bytes": 2_048,
    "literal_method_or_configuration_key_bytes": 1_024,
    "target_spelling_bytes": 1_024,
    "outer_attributes_per_declaration": 128,
    "attribute_token_bytes": 16_384,
    "permutations": 50,
}

ENTITY_KINDS = {
    "framework.component_declaration",
    "framework.service_declaration",
    "framework.configuration_declaration",
    "framework.endpoint_declaration",
    "framework.route_declaration",
    "framework.handler_declaration",
}

EPISTEMIC_STATES = {
    "declared_registration_syntax",
    "candidate_unresolved",
}

SOURCE_PROFILES = {
    "explicit-builder-registration-v1",
    "attribute-macro-candidate-v1",
}

COMPILATION_PRESENCE_STATES = {
    "unconditional",
    "conditional_unknown",
    "attribute_transform_unknown",
}

TARGET_BINDING_STATES = {
    "resolved_unique",
    "unresolved_external",
    "ambiguous_local",
    "not_applicable",
}

ERROR_CODES = (
    "input.invalid_rust_framework_profile",
    "extraction.invalid_framework_declaration",
    "extraction.framework_declaration_identity_conflict",
    "extraction.framework_declaration_limit_exceeded",
    "extraction.unsupported_framework_composition",
    "extraction.ambiguous_framework_target",
    "extraction.unresolvable_framework_evidence",
    "input.unsafe_framework_path",
    "internal.unexpected",
)

REQUIRED_TEST_NAMES = (
    "e2e_fr_ext_011_framework_declarations",
    "gt_fr_ext_011_explicit_builder_declarations",
    "gt_fr_ext_011_attribute_macro_candidates_remain_unresolved",
    "gt_fr_ext_011_unique_local_targets_only",
    "conf_fr_ext_011_snapshot_v9_graph_v6_error_v13",
    "conf_fr_qry_001_v9_uses_local_query_result_v4",
    "pt_dr_idn_002_r6_framework_identity_nfc",
    "pt_fr_ext_011_limits_have_max_and_plus_one",
    "pt_nfr_det_001_r6_permutation_and_replay_invariant",
    "sec_fr_ext_011_never_executes_or_expands_target_worlds",
    "sec_fr_ext_011_hard_negative_source_forms",
    "e2e_fr_doc_001_r6_declaration_candidate_non_runtime_wording",
    "reg_fr_cli_001_r6_selector_absence_is_byte_identical",
)

IMMUTABLE_R5_FILES = {
    "tests/specifications/s4/r5/contract-bundle.json": (
        "6491386c0fc8ec21dd3bf5112c2460eed56e33b976bb3afb1ac55fa4a8308509"
    ),
    "tests/specifications/s4/r5/rust-ontology-v5.json": (
        "ed5460ba08308cc1d075fb92841943be3354ef33a46327a11f86e284a5cff348"
    ),
    "tests/specifications/s4/r5/repository-snapshot-v8.schema.json": (
        "6eebe6283143ab2c394fda69a422a1fb3fe5cc54303bd3e5f118cab412b80e66"
    ),
    "tests/specifications/s4/r5/local-query-result-v3.schema.json": (
        "c4cb94f96af9cc405e04c4f9a3c57969ddfddab886e34392a93fce38bda8ebab"
    ),
}

SCHEMA_PATHS = (
    CONFIGURATION_SCHEMA_PATH,
    SNAPSHOT_SCHEMA_PATH,
    CHUNK_SCHEMA_PATH,
    GRAPH_SCHEMA_PATH,
    ERROR_SCHEMA_PATH,
    QUERY_SCHEMA_PATH,
)

MATERIALIZED_PATHS = (
    DECISION_PATH,
    SUBSET_PATH,
    CONFIGURATION_SCHEMA_PATH,
    SNAPSHOT_SCHEMA_PATH,
    CHUNK_SCHEMA_PATH,
    GRAPH_SCHEMA_PATH,
    ONTOLOGY_PATH,
    ERROR_SCHEMA_PATH,
    QUERY_SCHEMA_PATH,
    HASH_CONTRACT_PATH,
    ORACLE_PATH,
    QUERY_ORACLE_PATH,
    INVALID_CASES_PATH,
    PILOT_OBSERVATIONS_PATH,
    RED_OBSERVATION_PATH,
    RED_LOG_PATH,
    BUNDLE_PATH,
    FIXTURE_ROOT / "README.md",
    FIXTURE_MANIFEST_PATH,
    EXPECTED_FACTS_PATH,
    FIXTURE_REPOSITORY / "Cargo.toml",
    FIXTURE_REPOSITORY / "build.rs",
    FIXTURE_REPOSITORY / "src/lib.rs",
    FIXTURE_REPOSITORY / "src/builder_style.rs",
    FIXTURE_REPOSITORY / "src/attribute_style.rs",
    FIXTURE_REPOSITORY / "generated/framework.rs",
    FIXTURE_REPOSITORY / "target/generated.rs",
)

BUNDLE_FILES = {
    "LICENSE",
    "docs/software/decisions/0016-s4-r6-framework-declarations-contract.md",
    "scripts/tests/test_s4_r6_framework_declarations_contract.py",
    "tests/fixtures/s4/framework-declarations-v1/README.md",
    "tests/fixtures/s4/framework-declarations-v1/expected-framework-declarations.json",
    "tests/fixtures/s4/framework-declarations-v1/manifest.json",
    "tests/fixtures/s4/framework-declarations-v1/repository/Cargo.toml",
    "tests/fixtures/s4/framework-declarations-v1/repository/build.rs",
    "tests/fixtures/s4/framework-declarations-v1/repository/generated/framework.rs",
    "tests/fixtures/s4/framework-declarations-v1/repository/src/attribute_style.rs",
    "tests/fixtures/s4/framework-declarations-v1/repository/src/builder_style.rs",
    "tests/fixtures/s4/framework-declarations-v1/repository/src/lib.rs",
    "tests/fixtures/s4/framework-declarations-v1/repository/target/generated.rs",
    "tests/specifications/s4/r5/contract-bundle.json",
    "tests/specifications/s4/r6/codenoesis-error-v13.schema.json",
    "tests/specifications/s4/r6/configuration-v6.schema.json",
    "tests/specifications/s4/r6/e2e_fr_ext_011_framework_declarations.json",
    "tests/specifications/s4/r6/e2e_fr_qry_001_r6_exact_id_results.json",
    "tests/specifications/s4/r6/extraction-chunk-v6.schema.json",
    "tests/specifications/s4/r6/framework-declarations-subset-v1.json",
    "tests/specifications/s4/r6/invalid-cases-v1.json",
    "tests/specifications/s4/r6/knowledge-graph-v6.schema.json",
    "tests/specifications/s4/r6/local-query-result-v4.schema.json",
    "tests/specifications/s4/r6/pilot-observations-v1.json",
    "tests/specifications/s4/r6/red-observation.json",
    "tests/specifications/s4/r6/red/governance-red.log",
    "tests/specifications/s4/r6/repository-snapshot-v9.schema.json",
    "tests/specifications/s4/r6/rust-ontology-v6.json",
    "tests/specifications/s4/r6/semantic-hash-contract-v5.json",
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


def sha256_path(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def git_blob_oid(value: bytes) -> str:
    header = f"blob {len(value)}\0".encode("ascii")
    return hashlib.sha1(header + value).hexdigest()


def stable_framework_id(preimage: list[str]) -> str:
    normalized = [unicodedata.normalize("NFC", value) for value in preimage]
    digest = blake3_256(canonical_json([IDENTITY_DOMAIN, *normalized]))
    return f"urn:codenoesis:entity:blake3:{digest}"


def evidence_id(path: str, start: int, end: int, source_sha256: str) -> str:
    payload = ["codenoesis.evidence-id/source-span/v1", path, start, end, source_sha256]
    digest = hashlib.sha256(canonical_json(payload)).hexdigest()
    return f"urn:codenoesis:evidence:sha256:{digest}"


def walk_json(value: Any) -> Iterator[dict[str, Any]]:
    if isinstance(value, dict):
        yield value
        for child in value.values():
            yield from walk_json(child)
    elif isinstance(value, list):
        for child in value:
            yield from walk_json(child)


def resolve_pointer(document: Any, fragment: str) -> Any:
    if not fragment:
        return document
    if not fragment.startswith("/"):
        raise ValueError(f"unsupported JSON pointer: {fragment}")
    current = document
    for raw_token in fragment[1:].split("/"):
        token = unquote(raw_token).replace("~1", "/").replace("~0", "~")
        if isinstance(current, list):
            current = current[int(token)]
        else:
            current = current[token]
    return current


class S4R6FrameworkDeclarationsGovernanceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        for path in MATERIALIZED_PATHS:
            if not path.is_file():
                raise AssertionError(
                    "R6 governance artifact is not materialized: "
                    f"{path.relative_to(ROOT)}"
                )

    def test_ratification_decision_and_roadmap_are_exact(self) -> None:
        srs = SRS_PATH.read_text(encoding="utf-8")
        decision = DECISION_PATH.read_text(encoding="utf-8")
        roadmap = ROADMAP_PATH.read_text(encoding="utf-8")
        for value in (
            "0.9+r6",
            "S4 R6 framework-declarations ratification register",
            ISSUE_REFERENCE,
            AUTHORIZATION_REFERENCE,
            "FR-EXT-011",
            "--rust-framework-profile rust-framework-declarations-v1",
            "codenoesis.repository-snapshot/v9",
            "codenoesis.ontology/rust/v6",
            "codenoesis.local-query-result/v4",
            "codenoesis.error/v13",
        ):
            self.assertIn(value, srs)
        for value in (
            ISSUE_REFERENCE,
            AUTHORIZATION_REFERENCE,
            REQUIRED_BASE,
            "RepositorySnapshotV9",
            "ExtractionChunkV6",
            "KnowledgeGraphV6",
            "LocalQueryResultV4",
            "ErrorV13",
            "declared_registration_syntax",
            "candidate_unresolved",
            "No `CALLS`, `EXECUTES`, or `SERVES`",
            "requires a separate Ready product issue",
        ):
            self.assertIn(value, decision)
        self.assertIn("R0-R5 are implemented", roadmap)
        self.assertIn("R5 → R6 → R7 → R8", roadmap)
        self.assertIn("R6 governance is Proposed", roadmap)

    def test_oracle_binds_scope_selector_limits_and_pilots(self) -> None:
        oracle = load_json(ORACLE_PATH)
        self.assertEqual(oracle["issue"], ISSUE_REFERENCE)
        self.assertEqual(oracle["authorization"], AUTHORIZATION_REFERENCE)
        self.assertEqual(oracle["required_base"], REQUIRED_BASE)
        self.assertEqual(oracle["requirement_ids"][0], "FR-EXT-011")
        self.assertEqual(
            oracle["requirement_status"],
            {"current": "Proposed", "target_after_protected_merge": "Approved"},
        )
        self.assertEqual(oracle["slice"], "S4")
        self.assertEqual(oracle["roadmap_capability"], "R6")
        self.assertEqual(oracle["risk"], "high")
        self.assertEqual(oracle["correction_rounds"], 4)
        self.assertEqual(
            oracle["selector"],
            {
                "flag": "--rust-framework-profile",
                "value": "rust-framework-declarations-v1",
                "required_scan_profile": "standard-local-s4",
                "required_workspace_profile": "cargo-root-package-v1",
                "required_manifest_profile": "cargo-manifest-facts-v1",
                "required_semantic_profile": "rust-semantic-depth-v1",
                "implicit_selection": False,
            },
        )
        self.assertEqual(oracle["limits"], LIMITS)
        self.assertEqual(tuple(oracle["required_test_names"]), REQUIRED_TEST_NAMES)
        self.assertEqual(
            oracle["immutable_dependencies"],
            {
                "r5_contract_bundle_sha256": (
                    "ed48512d8337d2dda2a3b5f752177f3988915bdfc98eda1ff2391e15039e7d45"
                )
            },
        )
        pilots = {pilot["id"]: pilot for pilot in oracle["public_pilots"]}
        self.assertEqual(set(pilots), {"lekton", "rustdesk"})
        self.assertEqual(
            pilots["lekton"]["commit"],
            "7a4d1a4a30468f4c18ce158a9b825680b00f4820",
        )
        self.assertEqual(
            pilots["rustdesk"]["commit"],
            "d412d198720aa56f6cfed2dfad262e8fb1322fb7",
        )
        self.assertTrue(all(not pilot["vendored_source"] for pilot in pilots.values()))
        self.assertTrue(oracle["production_implementation_forbidden"])
        self.assertEqual(oracle["new_dependencies"], [])

    def test_new_schemas_are_strict_and_every_reference_resolves(self) -> None:
        for path in SCHEMA_PATHS:
            with self.subTest(path=path):
                document = load_json(path)
                self.assertEqual(
                    document["$schema"],
                    "https://json-schema.org/draft/2020-12/schema",
                )
                self.assertEqual(document["type"], "object")
                self.assertFalse(document["additionalProperties"])
                for schema in walk_json(document):
                    if schema.get("type") == "object":
                        self.assertIn("additionalProperties", schema)
                        self.assertFalse(schema["additionalProperties"])
                    reference = schema.get("$ref")
                    if reference is None:
                        continue
                    self.assertFalse(reference.startswith(("http:", "https:")))
                    relative, separator, fragment = reference.partition("#")
                    target_path = path if not relative else path.parent / relative
                    target_path = target_path.resolve()
                    try:
                        target_path.relative_to(ROOT)
                    except ValueError as error:
                        self.fail(f"schema ref escapes repository: {reference}: {error}")
                    self.assertTrue(target_path.is_file(), reference)
                    target = load_json(target_path)
                    if separator:
                        resolve_pointer(target, fragment)

    def test_versioned_shapes_identity_states_and_dispatch_are_closed(self) -> None:
        configuration = load_json(CONFIGURATION_SCHEMA_PATH)
        self.assertEqual(
            configuration["properties"]["schema_version"]["const"],
            "codenoesis.configuration/v6",
        )
        self.assertEqual(
            configuration["properties"]["rust_framework_profile"]["const"],
            "rust-framework-declarations-v1",
        )

        chunk = load_json(CHUNK_SCHEMA_PATH)
        self.assertEqual(
            chunk["properties"]["schema_version"]["const"],
            "codenoesis.extraction-chunk/v6",
        )
        self.assertEqual(
            chunk["properties"]["ontology_version"]["const"],
            "codenoesis.ontology/rust/v6",
        )
        framework = chunk["$defs"]["framework_declaration"]
        self.assertEqual(
            set(framework["properties"]["kind"]["enum"]), ENTITY_KINDS
        )
        self.assertEqual(
            set(framework["properties"]["epistemic_state"]["enum"]),
            EPISTEMIC_STATES,
        )
        self.assertEqual(
            set(framework["properties"]["source_profile"]["enum"]),
            SOURCE_PROFILES,
        )
        self.assertEqual(
            set(framework["properties"]["compilation_presence"]["enum"]),
            COMPILATION_PRESENCE_STATES,
        )
        self.assertEqual(
            set(framework["properties"]["target_binding"]["enum"]),
            TARGET_BINDING_STATES,
        )

        graph = load_json(GRAPH_SCHEMA_PATH)
        self.assertEqual(
            graph["properties"]["schema_version"]["const"],
            "codenoesis.knowledge-graph/v6",
        )
        snapshot = load_json(SNAPSHOT_SCHEMA_PATH)
        semantic = snapshot["$defs"]["semantic"]
        self.assertEqual(
            snapshot["properties"]["schema_version"]["const"],
            "codenoesis.repository-snapshot/v9",
        )
        self.assertEqual(
            semantic["properties"]["pipeline_version"]["const"],
            "codenoesis.pipeline/s4-r6-v1",
        )
        self.assertEqual(
            semantic["properties"]["extractor_contract_version"]["const"],
            "codenoesis.extraction/v6",
        )

        query = load_json(QUERY_SCHEMA_PATH)
        self.assertEqual(
            query["properties"]["schema_version"]["const"],
            "codenoesis.local-query-result/v4",
        )
        self.assertEqual(
            set(query["properties"]["result_kind"]["enum"]),
            {
                "entity",
                "relationship",
                "claim",
                "evidence",
                "diagnostic",
                "coverage_gap",
                "document",
            },
        )

    def test_golden_evidence_ids_are_schema_representable_and_queryable(self) -> None:
        facts = load_json(EXPECTED_FACTS_PATH)
        evidence_ids = [
            declaration["evidence"]["id"]
            for declaration in facts["declarations"]
        ]
        self.assertEqual(len(evidence_ids), 24)
        self.assertEqual(len(set(evidence_ids)), len(evidence_ids))

        chunk = load_json(CHUNK_SCHEMA_PATH)
        evidence_pattern = chunk["$defs"]["evidence_id"]["pattern"]
        self.assertTrue(
            all(re.fullmatch(evidence_pattern, identifier) for identifier in evidence_ids),
            f"reviewed SHA-256 evidence IDs are rejected by {evidence_pattern}",
        )
        self.assertEqual(
            chunk["properties"]["evidence"]["items"]["$ref"],
            "#/$defs/evidence",
        )
        self.assertEqual(
            chunk["$defs"]["evidence"]["properties"]["id"]["$ref"],
            "#/$defs/evidence_id",
        )

        graph = load_json(GRAPH_SCHEMA_PATH)
        self.assertEqual(
            graph["properties"]["evidence"]["items"]["$ref"],
            "extraction-chunk-v6.schema.json#/$defs/evidence",
        )

        query = load_json(QUERY_SCHEMA_PATH)
        self.assertEqual(
            query["properties"]["evidence"]["items"]["$ref"],
            "extraction-chunk-v6.schema.json#/$defs/evidence",
        )
        evidence_rule = next(
            rule
            for rule in query["allOf"]
            if rule["if"]["properties"]["result_kind"].get("const") == "evidence"
        )
        query_pattern = evidence_rule["then"]["properties"]["requested_id"][
            "pattern"
        ]
        for identifier in evidence_ids:
            with self.subTest(query_identifier=identifier):
                self.assertIsNotNone(re.fullmatch(query_pattern, identifier))

        for result_kind in (
            "entity",
            "relationship",
            "claim",
            "diagnostic",
            "coverage_gap",
            "document",
        ):
            rule = next(
                rule
                for rule in query["allOf"]
                if rule["if"]["properties"]["result_kind"].get("const")
                == result_kind
            )
            requested_pattern = rule["then"]["properties"]["requested_id"][
                "pattern"
            ]
            self.assertIn(":blake3:", requested_pattern)
            self.assertIsNone(re.fullmatch(requested_pattern, evidence_ids[0]))

    def test_subset_and_ontology_keep_source_syntax_non_runtime(self) -> None:
        subset = load_json(SUBSET_PATH)
        self.assertEqual(set(subset["entity_kinds"]), ENTITY_KINDS)
        self.assertEqual(set(subset["epistemic_states"]), EPISTEMIC_STATES)
        self.assertEqual(subset["limits"], LIMITS)
        profiles = {profile["id"]: profile for profile in subset["source_profiles"]}
        self.assertEqual(set(profiles), SOURCE_PROFILES)
        self.assertEqual(
            profiles["explicit-builder-registration-v1"]["epistemic_state"],
            "declared_registration_syntax",
        )
        self.assertEqual(
            profiles["attribute-macro-candidate-v1"]["epistemic_state"],
            "candidate_unresolved",
        )
        self.assertFalse(
            profiles["attribute-macro-candidate-v1"]["macro_arguments_authoritative"]
        )
        self.assertEqual(subset["relationships"], ["DEFINES"])
        hard_negatives = set(subset["hard_negatives"])
        self.assertTrue(
            {
                "comments",
                "strings",
                "documentation",
                "imports",
                "dependency_names",
                "name_only_conventions",
                "generated_directories",
                "target_directories",
            }.issubset(hard_negatives)
        )
        non_claims = " ".join(subset["semantic_non_claims"]).lower()
        for phrase in (
            "runtime behavior",
            "route reachability",
            "handler execution",
            "service start",
            "active configuration",
            "macro expansion",
            "call graph",
        ):
            self.assertIn(phrase, non_claims)

        ontology = load_json(ONTOLOGY_PATH)
        self.assertEqual(
            ontology["extends"],
            {
                "ontology_version": "codenoesis.ontology/rust/v5",
                "contract_path": "tests/specifications/s4/r5/rust-ontology-v5.json",
                "contract_sha256": IMMUTABLE_R5_FILES[
                    "tests/specifications/s4/r5/rust-ontology-v5.json"
                ],
            },
        )
        self.assertEqual(set(ontology["framework_entity_kinds"]), ENTITY_KINDS)
        self.assertEqual(set(ontology["epistemic_states"]), EPISTEMIC_STATES)
        self.assertEqual(ontology["identity"]["domain"], IDENTITY_DOMAIN)
        self.assertEqual(
            ontology["identity"]["preimage_fields"],
            [
                "repository_identity",
                "crate_identity",
                "lexical_owner_identity",
                "role",
                "source_profile",
                "source_form_identity",
                "normalized_declared_key_or_target_spelling",
            ],
        )
        self.assertEqual(ontology["relationship_kinds"], ["DEFINES"])
        self.assertTrue(
            {"CALLS", "EXECUTES", "SERVES"}.issubset(
                set(ontology["forbidden_relationship_kinds"])
            )
        )

    def test_fixture_manifest_binds_every_project_owned_byte(self) -> None:
        manifest = load_json(FIXTURE_MANIFEST_PATH)
        self.assertEqual(
            manifest["schema_version"], "codenoesis.r6-fixture-manifest/v1"
        )
        self.assertEqual(manifest["repository_identity"], REPOSITORY_IDENTITY)
        self.assertFalse(manifest["external_source_vendored"])
        self.assertFalse(manifest["fixture_may_execute"])
        self.assertRegex(manifest["materialization"]["tree_oid"], r"^[0-9a-f]{40}$")
        self.assertRegex(manifest["materialization"]["commit_oid"], r"^[0-9a-f]{40}$")
        files = manifest["files"]
        paths = [item["path"] for item in files]
        expected_paths = sorted(
            path.relative_to(FIXTURE_ROOT).as_posix()
            for path in FIXTURE_REPOSITORY.rglob("*")
            if path.is_file()
        )
        self.assertEqual(paths, expected_paths)
        self.assertEqual(len(paths), len(set(paths)))
        for item in files:
            source = FIXTURE_ROOT / item["path"]
            value = source.read_bytes()
            self.assertEqual(item["mode"], "100644")
            self.assertEqual(item["byte_length"], len(value))
            self.assertEqual(item["sha256"], hashlib.sha256(value).hexdigest())
            self.assertEqual(item["git_blob_oid"], git_blob_oid(value))
        expected = manifest["expected_facts"]
        self.assertEqual(expected["path"], EXPECTED_FACTS_PATH.name)
        self.assertEqual(expected["byte_length"], EXPECTED_FACTS_PATH.stat().st_size)
        self.assertEqual(expected["sha256"], sha256_path(EXPECTED_FACTS_PATH))
        readme = (FIXTURE_ROOT / "README.md").read_text(encoding="utf-8")
        self.assertIn("project-owned", readme)
        self.assertIn("never compile, execute, expand, or fetch", readme)

    def test_expected_facts_have_derived_identities_evidence_and_states(self) -> None:
        facts = load_json(EXPECTED_FACTS_PATH)
        self.assertEqual(facts["repository_identity"], REPOSITORY_IDENTITY)
        self.assertEqual(facts["ontology_version"], "codenoesis.ontology/rust/v6")
        declarations = facts["declarations"]
        self.assertGreater(len(declarations), len(ENTITY_KINDS))
        self.assertEqual(len({item["id"] for item in declarations}), len(declarations))
        self.assertEqual(
            set(facts["entity_counts"]),
            ENTITY_KINDS,
        )
        self.assertTrue(all(count > 0 for count in facts["entity_counts"].values()))
        computed_entity_counts = {
            kind: sum(item["entity_kind"] == kind for item in declarations)
            for kind in ENTITY_KINDS
        }
        self.assertEqual(facts["entity_counts"], computed_entity_counts)
        computed_state_counts = {
            state: sum(item["epistemic_state"] == state for item in declarations)
            for state in EPISTEMIC_STATES
        }
        self.assertEqual(facts["epistemic_state_counts"], computed_state_counts)
        self.assertEqual(set(facts["target_binding_counts"]), TARGET_BINDING_STATES)

        for declaration in declarations:
            normalized_key = unicodedata.normalize(
                "NFC", declaration["declared_key_or_target"]
            )
            preimage = [
                REPOSITORY_IDENTITY,
                declaration["crate_identity"],
                declaration["lexical_owner_id"],
                declaration["role"],
                declaration["source_profile"],
                declaration["source_form_identity"],
                normalized_key,
            ]
            self.assertEqual(declaration["identity_preimage"], preimage)
            self.assertEqual(declaration["id"], stable_framework_id(preimage))
            self.assertIn(declaration["entity_kind"], ENTITY_KINDS)
            self.assertIn(declaration["source_profile"], SOURCE_PROFILES)
            self.assertIn(declaration["epistemic_state"], EPISTEMIC_STATES)
            self.assertIn(
                declaration["compilation_presence"], COMPILATION_PRESENCE_STATES
            )
            self.assertIn(declaration["target_binding"], TARGET_BINDING_STATES)
            evidence = declaration["evidence"]
            source = FIXTURE_REPOSITORY / evidence["path"]
            value = source.read_bytes()
            self.assertLess(evidence["start_byte"], evidence["end_byte"])
            self.assertLessEqual(evidence["end_byte"], len(value))
            source_sha256 = hashlib.sha256(value).hexdigest()
            self.assertEqual(evidence["source_sha256"], source_sha256)
            self.assertEqual(
                evidence["id"],
                evidence_id(
                    evidence["path"],
                    evidence["start_byte"],
                    evidence["end_byte"],
                    source_sha256,
                ),
            )
            self.assertEqual(
                value[evidence["start_byte"] : evidence["end_byte"]].decode("utf-8"),
                evidence["source_form"],
            )

        self.assertEqual(facts["relationship_counts"], {"DEFINES": len(declarations)})
        self.assertEqual(facts["relationship_kinds"], ["DEFINES"])
        self.assertTrue(
            {"CALLS", "EXECUTES", "SERVES"}.issubset(
                set(facts["forbidden_relationship_kinds"])
            )
        )
        candidate_ids = {
            item["id"]
            for item in declarations
            if item["epistemic_state"] == "candidate_unresolved"
        }
        gap_ids = {gap["declaration_id"] for gap in facts["coverage_gaps"]}
        self.assertTrue(candidate_ids.issubset(gap_ids))
        ambiguous_ids = {
            item["id"]
            for item in declarations
            if item["target_binding"] == "ambiguous_local"
        }
        diagnostic_ids = {
            diagnostic["declaration_id"] for diagnostic in facts["diagnostics"]
        }
        self.assertTrue(ambiguous_ids.issubset(diagnostic_ids))

    def test_fixture_contains_two_styles_and_security_decoys(self) -> None:
        builder = (FIXTURE_REPOSITORY / "src/builder_style.rs").read_text(
            encoding="utf-8"
        )
        attributes = (FIXTURE_REPOSITORY / "src/attribute_style.rs").read_text(
            encoding="utf-8"
        )
        library = (FIXTURE_REPOSITORY / "src/lib.rs").read_text(encoding="utf-8")
        for value in (
            'RegistrationSet::new()',
            '.route("GET", "/items", handlers::list_items)',
            '.route("POST", "/items", handlers::create_item)',
            '.group("/api", nested_group())',
            '.configuration("mode", config::mode)',
            '.service(services::worker)',
            '.component(components::shell)',
            '.endpoint("/metrics", handlers::metrics)',
            '.handler(handlers::fallback)',
            'external::missing',
            '.handler(duplicate)',
            'let _unused_builder = RegistrationSet::new()',
        ):
            self.assertIn(value, builder)
        for value in (
            "#[route(",
            "#[component]",
            "#[service]",
            "#[command]",
            "#[runtime::entry]",
            "#[bridge]",
            "#[cfg(",
            "#[cfg_attr(",
            "#[derive(",
            "declare_routes!",
        ):
            self.assertIn(value, attributes)
        self.assertIn("COMMENT_ROUTE_DECOY", library)
        self.assertIn("STRING_ROUTE_DECOY", library)
        self.assertIn("DOC_ROUTE_DECOY", library)
        self.assertIn("IMPORT_ROUTE_DECOY", library)
        self.assertIn("NAME_ONLY_ROUTE_DECOY", library)
        build_source = (FIXTURE_REPOSITORY / "build.rs").read_text(encoding="utf-8")
        self.assertIn("must never execute", build_source)
        expected = load_json(EXPECTED_FACTS_PATH)
        self.assertEqual(
            set(expected["hard_negative_labels"]),
            {
                "comment_route_decoy",
                "string_route_decoy",
                "documentation_route_decoy",
                "import_route_decoy",
                "name_only_route_decoy",
                "unused_builder_decoy",
                "declarative_macro_generated_decoy",
                "generated_directory_decoy",
                "target_directory_decoy",
                "build_sentinel_not_executed",
            },
        )

    def test_invalid_cases_errors_and_semantic_hash_are_exact(self) -> None:
        cases = load_json(INVALID_CASES_PATH)
        self.assertEqual(cases["schema_version"], "codenoesis.r6-invalid-cases/v1")
        ids = [case["id"] for case in cases["cases"]]
        self.assertEqual(ids, sorted(ids))
        self.assertEqual(len(ids), len(set(ids)))
        limit_cases = {
            case["limit"]
            for case in cases["cases"]
            if case["class"] == "limit_plus_one"
        }
        self.assertEqual(limit_cases, set(LIMITS) - {"permutations"})
        classes = {case["class"] for case in cases["cases"]}
        self.assertTrue(
            {
                "malformed",
                "normalization_collision",
                "ambiguous_resolution",
                "unsupported_source_form",
                "unsupported_composition",
                "unresolvable_evidence",
                "unsafe_path",
                "symlink_escape",
                "privacy_boundary",
                "limit_plus_one",
                "hard_negative",
                "forbidden_authority",
            }.issubset(classes)
        )
        self.assertTrue(all(not case["partial_publication"] for case in cases["cases"]))

        error = load_json(ERROR_SCHEMA_PATH)
        self.assertEqual(tuple(error["properties"]["code"]["enum"]), ERROR_CODES)
        self.assertEqual(
            set(error["$defs"]["limit_context"]["properties"]["limit"]["enum"]),
            set(LIMITS) - {"permutations"},
        )
        self.assertEqual(
            load_json(HASH_CONTRACT_PATH),
            {
                "schema_version": "codenoesis.semantic-hash-contract/v5",
                "algorithm": "blake3-256",
                "canonicalization": "RFC8785",
                "domain_separator_hex": "00",
                "hashes": {
                    "configuration": {
                        "domain": "codenoesis.configuration.semantic.v6",
                        "payload": "ConfigurationV6 without semantic_hash",
                    },
                    "extraction_chunk": {
                        "domain": "codenoesis.extraction-chunk.semantic.v6",
                        "payload": "ExtractionChunkV6 without semantic_hash",
                    },
                    "knowledge_graph": {
                        "domain": "codenoesis.knowledge-graph.semantic.v6",
                        "payload": "KnowledgeGraphV6 without semantic_hash",
                    },
                    "snapshot": {
                        "domain": "codenoesis.repository-snapshot.semantic.v9",
                        "payload": "RepositorySnapshotV9.semantic",
                    },
                },
            },
        )

    def test_query_dispatch_and_pilot_observations_are_bounded(self) -> None:
        query = load_json(QUERY_ORACLE_PATH)
        self.assertEqual(query["issue"], ISSUE_REFERENCE)
        self.assertEqual(query["authorization"], AUTHORIZATION_REFERENCE)
        self.assertEqual(
            query["dispatch"],
            {
                "codenoesis.repository-snapshot/v9": "codenoesis.local-query-result/v4",
                "codenoesis.repository-snapshot/v8": "codenoesis.local-query-result/v3",
                "codenoesis.repository-snapshot/v7": "codenoesis.local-query-result/v2",
                "codenoesis.repository-snapshot/v6": "codenoesis.local-query-result/v1",
                "codenoesis.repository-snapshot/v5": "codenoesis.local-query-result/v1",
                "codenoesis.repository-snapshot/v4": "codenoesis.local-query-result/v1",
                "explicit_query_version_flag": False,
            },
        )
        self.assertEqual(
            set(query["result_kinds"]),
            {
                "entity",
                "relationship",
                "claim",
                "evidence",
                "diagnostic",
                "coverage_gap",
                "document",
            },
        )
        self.assertTrue(query["stored_head_validation_required"])
        self.assertFalse(query["runtime_semantics_authorized"])

        observations = load_json(PILOT_OBSERVATIONS_PATH)
        self.assertEqual(
            observations["schema_version"],
            "codenoesis.r6-pilot-observations/v1",
        )
        self.assertFalse(observations["external_source_vendored"])
        pilots = {pilot["id"]: pilot for pilot in observations["pilots"]}
        self.assertEqual(set(pilots), {"lekton", "rustdesk"})
        self.assertEqual(
            pilots["lekton"]["observed_counts"],
            {"route_like_builder_source": 88, "layer_or_service_builder_source": 7},
        )
        self.assertEqual(
            pilots["rustdesk"]["observed_counts"],
            {
                "route_like_builder_source": 0,
                "attribute_macro_role_looking_source": 75,
            },
        )
        for pilot in pilots.values():
            self.assertFalse(pilot["ontology_truth"])
            self.assertFalse(pilot["golden_fixture"])
            self.assertTrue(pilot["git_commit_pinned"])
            self.assertTrue(pilot["command"])
            self.assertTrue(pilot["limitations"])

    def test_red_observation_and_raw_log_are_immutable(self) -> None:
        observation = load_json(RED_OBSERVATION_PATH)
        self.assertEqual(observation["issue"], ISSUE_REFERENCE)
        self.assertEqual(observation["authorization"], AUTHORIZATION_REFERENCE)
        self.assertEqual(observation["required_base"], REQUIRED_BASE)
        self.assertEqual(observation["exit_code"], 1)
        self.assertTrue(observation["failed_for_expected_reason"])
        self.assertFalse(observation["r6_contract_files_changed_before_red"])
        self.assertFalse(observation["production_files_changed_before_red"])
        self.assertEqual(
            observation["changed_paths_before_red"],
            ["scripts/tests/test_s4_r6_framework_declarations_contract.py"],
        )
        self.assertEqual(observation["guard_sha256"], TEST_FIRST_GUARD_SHA256)
        self.assertEqual(
            observation["raw_log_path"], RED_LOG_PATH.relative_to(ROOT).as_posix()
        )
        self.assertEqual(observation["raw_log_bytes"], RED_LOG_PATH.stat().st_size)
        self.assertEqual(observation["raw_log_sha256"], sha256_path(RED_LOG_PATH))
        self.assertEqual(observation["stdout_bytes"], 0)
        self.assertEqual(
            observation["stdout_sha256"],
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        )
        log = RED_LOG_PATH.read_text(encoding="utf-8")
        self.assertIn("R6 governance artifact is not materialized", log)
        self.assertIn(
            "docs/software/decisions/0016-s4-r6-framework-declarations-contract.md",
            log,
        )
        self.assertIn("FAILED (errors=1)", log)

    def test_complete_r5_lineage_is_immutable(self) -> None:
        for relative, expected in IMMUTABLE_R5_FILES.items():
            with self.subTest(relative=relative):
                self.assertEqual(sha256_path(ROOT / relative), expected)
        bundle = load_json(ROOT / "tests/specifications/s4/r5/contract-bundle.json")
        self.assertEqual(
            bundle["bundle_sha256"],
            "ed48512d8337d2dda2a3b5f752177f3988915bdfc98eda1ff2391e15039e7d45",
        )

    def test_contract_bundle_binds_every_r6_governance_artifact(self) -> None:
        bundle = load_json(BUNDLE_PATH)
        self.assertEqual(set(bundle), {"schema_version", "files", "bundle_sha256"})
        self.assertEqual(bundle["schema_version"], "codenoesis.contract-bundle/v1")
        files = bundle["files"]
        paths = [item["path"] for item in files]
        self.assertEqual(paths, sorted(paths))
        self.assertEqual(set(paths), BUNDLE_FILES)
        self.assertEqual(len(paths), len(set(paths)))
        self.assertNotIn("docs/software/software-requirements-specification.md", paths)
        self.assertNotIn("docs/software/roadmap.md", paths)
        self.assertNotIn("tests/specifications/s4/r6/contract-bundle.json", paths)
        for item in files:
            self.assertEqual(set(item), {"path", "sha256"})
            path = Path(item["path"])
            self.assertFalse(path.is_absolute())
            self.assertNotIn("..", path.parts)
            self.assertRegex(item["sha256"], r"^[0-9a-f]{64}$")
            self.assertEqual(sha256_path(ROOT / path), item["sha256"])
        payload = {"schema_version": bundle["schema_version"], "files": files}
        bundle_sha256 = hashlib.sha256(canonical_json(payload)).hexdigest()
        self.assertEqual(bundle["bundle_sha256"], bundle_sha256)
        srs = SRS_PATH.read_text(encoding="utf-8")
        match = re.search(
            r"R6 framework-declarations contract bundle:\s+"
            r"`sha256:([0-9a-f]{64})`",
            srs,
        )
        self.assertIsNotNone(match, "SRS must bind the complete R6 bundle")
        self.assertEqual(match.group(1), bundle_sha256)  # type: ignore[union-attr]


if __name__ == "__main__":
    unittest.main()
