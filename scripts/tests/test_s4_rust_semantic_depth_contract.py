from __future__ import annotations

import hashlib
import json
import re
import unittest
from pathlib import Path
from typing import Any, Iterator
from urllib.parse import unquote

from scripts.tests.test_s1_contract import blake3_256


ROOT = Path(__file__).resolve().parents[2]
SRS_PATH = ROOT / "docs/software/software-requirements-specification.md"
ROADMAP_PATH = ROOT / "docs/software/roadmap.md"
DECISION_PATH = (
    ROOT / "docs/software/decisions/0015-s4-r5-rust-semantic-depth-contract.md"
)
SPEC_ROOT = ROOT / "tests/specifications/s4/r5"
SUBSET_PATH = SPEC_ROOT / "rust-semantic-depth-subset-v1.json"
CONFIGURATION_SCHEMA_PATH = SPEC_ROOT / "configuration-v5.schema.json"
SNAPSHOT_SCHEMA_PATH = SPEC_ROOT / "repository-snapshot-v8.schema.json"
CHUNK_SCHEMA_PATH = SPEC_ROOT / "extraction-chunk-v5.schema.json"
GRAPH_SCHEMA_PATH = SPEC_ROOT / "knowledge-graph-v5.schema.json"
ONTOLOGY_PATH = SPEC_ROOT / "rust-ontology-v5.json"
ERROR_SCHEMA_PATH = SPEC_ROOT / "codenoesis-error-v12.schema.json"
QUERY_SCHEMA_PATH = SPEC_ROOT / "local-query-result-v3.schema.json"
HASH_CONTRACT_PATH = SPEC_ROOT / "semantic-hash-contract-v4.json"
ORACLE_PATH = SPEC_ROOT / "e2e_fr_ext_010_rust_semantic_depth.json"
QUERY_ORACLE_PATH = SPEC_ROOT / "e2e_fr_qry_001_r5_exact_id_results.json"
INVALID_CASES_PATH = SPEC_ROOT / "invalid-cases-v1.json"
RED_OBSERVATION_PATH = SPEC_ROOT / "red-observation.json"
RED_LOG_PATH = SPEC_ROOT / "red/governance-red.log"
BUNDLE_PATH = SPEC_ROOT / "contract-bundle.json"
FIXTURE_ROOT = ROOT / "tests/fixtures/s4/rust-semantic-depth-v1"
FIXTURE_REPOSITORY = FIXTURE_ROOT / "repository"
FIXTURE_MANIFEST_PATH = FIXTURE_ROOT / "manifest.json"
EXPECTED_FACTS_PATH = FIXTURE_ROOT / "expected-rust-semantic-depth.json"

ISSUE_REFERENCE = "https://github.com/smutti/codenoesis/issues/111"
AUTHORIZATION_REFERENCE = (
    "https://github.com/smutti/codenoesis/issues/111#issuecomment-5179817871"
)
APPROVAL_REFERENCE = "__R5_GOVERNANCE_PR__"
REQUIRED_BASE = "e7cceb08b0aa4b7342cd2c6c1e267733130bd5f8"
REPOSITORY_IDENTITY = "urn:codenoesis:fixture:s4-rust-semantic-depth-v1"

LIMITS = {
    "fields_per_owner": 1_024,
    "variants_per_enum": 1_024,
    "tuple_fields_per_owner": 1_024,
    "associated_items_per_context": 1_024,
    "outer_attributes_per_declaration": 128,
    "attribute_token_bytes": 16_384,
    "declared_type_or_header_bytes": 4_096,
    "permutations": 50,
}

R5_ENTITY_KINDS = {
    "rust.field",
    "rust.enum_variant",
    "rust.constant",
    "rust.static",
    "rust.associated_type",
}

COMPILATION_PRESENCE_STATES = {
    "unconditional",
    "conditional_unknown",
    "attribute_transform_unknown",
}

CAPABILITY_STATES = {
    "rust.attribute_semantics_not_interpreted": "unsupported",
    "rust.cfg_presence_unresolved": "not_resolved",
    "rust.macro_generated_items_not_analyzed": "not_analyzed",
    "rust.type_resolution_not_performed": "not_resolved",
    "rust.value_not_evaluated": "not_evaluated",
    "rust.union_unsupported": "unsupported",
    "rust.foreign_block_unsupported": "unsupported",
    "rust.unsupported_impl_header": "unsupported",
}

ERROR_CODES = (
    "input.invalid_rust_semantic_profile",
    "extraction.invalid_rust_semantic_declaration",
    "extraction.rust_semantic_identity_conflict",
    "extraction.rust_semantic_limit_exceeded",
    "extraction.unsupported_rust_semantic_composition",
    "internal.unexpected",
)

REQUIRED_TEST_NAMES = (
    "e2e_fr_ext_010_rust_semantic_depth",
    "gt_fr_ext_010_fields_and_variants_are_owned",
    "gt_fr_ext_010_constants_statics_and_associated_types",
    "gt_fr_ext_010_method_context_prevents_trait_collisions",
    "gt_fr_ext_010_attributes_preserve_declarations_and_gaps",
    "conf_fr_ext_010_snapshot_v8_graph_v5_error_v12",
    "conf_fr_qry_001_v8_uses_local_query_result_v3",
    "pt_dr_idn_002_r5_member_identities_and_cardinalities",
    "pt_fr_ext_010_limits_have_max_and_plus_one",
    "pt_nfr_det_001_r5_permutation_and_schedule_invariant",
    "sec_fr_ext_010_never_executes_or_interprets_target_worlds",
    "e2e_fr_doc_001_r5_declared_and_unresolved_are_documented",
    "reg_fr_cli_001_r5_selector_absence_is_byte_identical",
)

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
    RED_OBSERVATION_PATH,
    RED_LOG_PATH,
    BUNDLE_PATH,
    FIXTURE_ROOT / "README.md",
    FIXTURE_MANIFEST_PATH,
    EXPECTED_FACTS_PATH,
    FIXTURE_REPOSITORY / "Cargo.toml",
    FIXTURE_REPOSITORY / "build.rs",
    FIXTURE_REPOSITORY / "src/lib.rs",
    FIXTURE_REPOSITORY / "src/model.rs",
)

BUNDLE_FILES = {
    "LICENSE",
    "docs/software/decisions/0015-s4-r5-rust-semantic-depth-contract.md",
    "scripts/tests/test_s4_rust_semantic_depth_contract.py",
    "tests/fixtures/s4/rust-semantic-depth-v1/README.md",
    "tests/fixtures/s4/rust-semantic-depth-v1/expected-rust-semantic-depth.json",
    "tests/fixtures/s4/rust-semantic-depth-v1/manifest.json",
    "tests/fixtures/s4/rust-semantic-depth-v1/repository/Cargo.toml",
    "tests/fixtures/s4/rust-semantic-depth-v1/repository/build.rs",
    "tests/fixtures/s4/rust-semantic-depth-v1/repository/src/lib.rs",
    "tests/fixtures/s4/rust-semantic-depth-v1/repository/src/model.rs",
    "tests/specifications/s4/r4/contract-bundle.json",
    "tests/specifications/s4/r5/codenoesis-error-v12.schema.json",
    "tests/specifications/s4/r5/configuration-v5.schema.json",
    "tests/specifications/s4/r5/e2e_fr_ext_010_rust_semantic_depth.json",
    "tests/specifications/s4/r5/e2e_fr_qry_001_r5_exact_id_results.json",
    "tests/specifications/s4/r5/extraction-chunk-v5.schema.json",
    "tests/specifications/s4/r5/invalid-cases-v1.json",
    "tests/specifications/s4/r5/knowledge-graph-v5.schema.json",
    "tests/specifications/s4/r5/local-query-result-v3.schema.json",
    "tests/specifications/s4/r5/red-observation.json",
    "tests/specifications/s4/r5/red/governance-red.log",
    "tests/specifications/s4/r5/repository-snapshot-v8.schema.json",
    "tests/specifications/s4/r5/rust-ontology-v5.json",
    "tests/specifications/s4/r5/rust-semantic-depth-subset-v1.json",
    "tests/specifications/s4/r5/semantic-hash-contract-v4.json",
}

IMMUTABLE_FILES = {
    "tests/specifications/s4/r4/contract-bundle.json": (
        "9153809d5108dbf395f4d25bcfbe582c80dd9394b97c091a39295c9a9e78908c"
    ),
    "tests/specifications/s4/r4/rust-ontology-v4.json": (
        "30138d850faac4a644be64796e1cf7934b51ae2ad8e24f922e0b2559f513d594"
    ),
    "tests/specifications/s4/r4/local-query-result-v2.schema.json": (
        "1235e23bd8e282d3f560dfad7c791c43b9d8d0dead120886d90dbf39c288deff"
    ),
    "tests/specifications/s4/documentation-manifest-v1.schema.json": (
        "13fc4500b8a63669f4e99e99e797f039421589c8b5caf9dcc7d4051792729943"
    ),
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


def stable_id(domain: str, preimage: list[str]) -> str:
    digest = blake3_256(canonical_json([domain, *preimage]))
    kind = domain.split(".", 1)[1].split("-id", 1)[0]
    return f"urn:codenoesis:{kind}:blake3:{digest}"


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


class S4RustSemanticDepthGovernanceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        for path in MATERIALIZED_PATHS:
            if not path.is_file():
                raise AssertionError(
                    f"R5 governance artifact is not materialized: "
                    f"{path.relative_to(ROOT)}"
                )

    def test_ratification_register_decision_and_roadmap_are_exact(self) -> None:
        srs = SRS_PATH.read_text(encoding="utf-8")
        decision = DECISION_PATH.read_text(encoding="utf-8")
        roadmap = ROADMAP_PATH.read_text(encoding="utf-8")
        for value in (
            "0.9+r5",
            "### 2.15 S4 Rust semantic-depth ratification register",
            ISSUE_REFERENCE,
            AUTHORIZATION_REFERENCE,
            APPROVAL_REFERENCE,
            "FR-EXT-010",
            "--rust-semantic-profile rust-semantic-depth-v1",
            "codenoesis.repository-snapshot/v8",
            "codenoesis.ontology/rust/v5",
            "codenoesis.local-query-result/v3",
            "codenoesis.error/v12",
        ):
            self.assertIn(value, srs)
        for value in (
            ISSUE_REFERENCE,
            AUTHORIZATION_REFERENCE,
            APPROVAL_REFERENCE,
            REQUIRED_BASE,
            "RepositorySnapshotV8",
            "ExtractionChunkV5",
            "KnowledgeGraphV5",
            "LocalQueryResultV3",
            "ErrorV12",
            "rust.attribute_semantics_not_interpreted",
            "rust.cfg_presence_unresolved",
            "SRS is excluded",
            "requires a separate Ready issue",
        ):
            self.assertIn(value, decision)
        self.assertIn("R0-R4 are implemented", roadmap)
        self.assertIn("R5 → R6 → R7 → R8", roadmap)

    def test_machine_oracles_bind_scope_limits_red_and_pilots(self) -> None:
        oracle = load_json(ORACLE_PATH)
        self.assertEqual(oracle["issue"], ISSUE_REFERENCE)
        self.assertEqual(oracle["authorization"], AUTHORIZATION_REFERENCE)
        self.assertEqual(oracle["approval"], APPROVAL_REFERENCE)
        self.assertEqual(oracle["requirement_ids"], ["FR-EXT-010"])
        self.assertEqual(
            oracle["requirement_status"],
            {"current": "Proposed", "target_after_protected_merge": "Approved"},
        )
        self.assertEqual(oracle["slice"], "S4")
        self.assertEqual(oracle["roadmap_capability"], "R5")
        self.assertEqual(oracle["risk"], "high")
        self.assertEqual(oracle["required_base"], REQUIRED_BASE)
        self.assertEqual(
            oracle["selector"],
            {
                "flag": "--rust-semantic-profile",
                "value": "rust-semantic-depth-v1",
                "required_profile": "standard-local-s4",
                "required_workspace_profile": "cargo-root-package-v1",
                "required_manifest_profile": "cargo-manifest-facts-v1",
                "implicit_selection": False,
            },
        )
        self.assertEqual(oracle["limits"], LIMITS)
        self.assertEqual(tuple(oracle["required_test_names"]), REQUIRED_TEST_NAMES)
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
        self.assertEqual(
            oracle["immutable_dependencies"],
            {"r4_contract_bundle_sha256": "2588abf38d686cc6475e7662ad8e90d585d1cdbff77702231dcadb1626a0c249"},
        )

        query = load_json(QUERY_ORACLE_PATH)
        self.assertEqual(query["issue"], ISSUE_REFERENCE)
        self.assertEqual(query["authorization"], AUTHORIZATION_REFERENCE)
        self.assertEqual(query["approval"], APPROVAL_REFERENCE)
        self.assertEqual(
            query["dispatch"],
            {
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

    def test_versioned_shapes_identity_coverage_and_dispatch_are_closed(self) -> None:
        configuration = load_json(CONFIGURATION_SCHEMA_PATH)
        self.assertEqual(
            configuration["properties"]["schema_version"]["const"],
            "codenoesis.configuration/v5",
        )
        self.assertEqual(
            configuration["properties"]["rust_semantic_profile"]["const"],
            "rust-semantic-depth-v1",
        )

        chunk = load_json(CHUNK_SCHEMA_PATH)
        self.assertEqual(
            chunk["properties"]["schema_version"]["const"],
            "codenoesis.extraction-chunk/v5",
        )
        self.assertEqual(
            chunk["properties"]["ontology_version"]["const"],
            "codenoesis.ontology/rust/v5",
        )
        self.assertEqual(
            set(chunk["$defs"]["r5_entity_kind"]["enum"]), R5_ENTITY_KINDS
        )
        self.assertEqual(
            set(chunk["$defs"]["compilation_presence"]["enum"]),
            COMPILATION_PRESENCE_STATES,
        )
        self.assertEqual(
            set(chunk["$defs"]["coverage"]["properties"]["capability"]["enum"]),
            set(CAPABILITY_STATES),
        )
        self.assertEqual(
            set(chunk["$defs"]["coverage"]["properties"]["state"]["enum"]),
            set(CAPABILITY_STATES.values()),
        )

        graph = load_json(GRAPH_SCHEMA_PATH)
        self.assertEqual(
            graph["properties"]["schema_version"]["const"],
            "codenoesis.knowledge-graph/v5",
        )
        snapshot = load_json(SNAPSHOT_SCHEMA_PATH)
        semantic = snapshot["$defs"]["semantic"]
        self.assertEqual(
            snapshot["properties"]["schema_version"]["const"],
            "codenoesis.repository-snapshot/v8",
        )
        self.assertEqual(
            semantic["properties"]["pipeline_version"]["const"],
            "codenoesis.pipeline/s4-r5-v1",
        )
        self.assertEqual(
            semantic["properties"]["extractor_contract_version"]["const"],
            "codenoesis.extraction/v5",
        )

        query = load_json(QUERY_SCHEMA_PATH)
        self.assertEqual(
            query["properties"]["schema_version"]["const"],
            "codenoesis.local-query-result/v3",
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

        ontology = load_json(ONTOLOGY_PATH)
        self.assertEqual(
            ontology["extends"],
            {
                "ontology_version": "codenoesis.ontology/rust/v4",
                "contract_path": "tests/specifications/s4/r4/rust-ontology-v4.json",
                "contract_sha256": IMMUTABLE_FILES[
                    "tests/specifications/s4/r4/rust-ontology-v4.json"
                ],
            },
        )
        self.assertEqual(set(ontology["r5_entity_kinds"]), R5_ENTITY_KINDS)
        self.assertEqual(
            ontology["identity"]["member_entity_domain"],
            "codenoesis.entity-id/rust-member/v1",
        )
        self.assertEqual(
            ontology["identity"]["legacy_rust_entity_domain"],
            "codenoesis.entity-id/rust/v2",
        )
        self.assertEqual(ontology["coverage_capability_states"], CAPABILITY_STATES)
        self.assertEqual(ontology["limits"], LIMITS)
        self.assertIn("active_cfg_world", ontology["forbidden_authority"])
        self.assertIn("macro_expansion", ontology["forbidden_authority"])
        self.assertIn("framework_role_inference", ontology["forbidden_authority"])

    def test_subset_is_declaration_only_and_defers_r6_r7_meaning(self) -> None:
        subset = load_json(SUBSET_PATH)
        self.assertEqual(
            subset["selection"],
            {
                "flag": "--rust-semantic-profile",
                "value": "rust-semantic-depth-v1",
                "required_scan_profile": "standard-local-s4",
                "required_workspace_profile": "cargo-root-package-v1",
                "required_manifest_profile": "cargo-manifest-facts-v1",
                "implicit_selection": False,
                "repository_content_selects_profile": False,
            },
        )
        self.assertEqual(set(subset["entity_kinds"]), R5_ENTITY_KINDS)
        self.assertEqual(subset["limits"], LIMITS)
        deferred = " ".join(subset["semantic_non_claims"])
        for phrase in (
            "active cfg",
            "macro expansion",
            "framework role",
            "resolved type",
            "evaluated value",
            "runtime behavior",
            "call graph",
        ):
            self.assertIn(phrase, deferred)
        self.assertEqual(
            subset["attribute_policy"]["cfg"],
            {
                "presence": "conditional_unknown",
                "coverage": "rust.cfg_presence_unresolved",
                "evaluated": False,
            },
        )
        self.assertFalse(subset["attribute_policy"]["custom"]["interpreted"])

    def test_fixture_manifest_binds_every_materialized_byte(self) -> None:
        manifest = load_json(FIXTURE_MANIFEST_PATH)
        self.assertEqual(
            manifest["schema_version"], "codenoesis.r5-fixture-manifest/v1"
        )
        self.assertEqual(manifest["repository_identity"], REPOSITORY_IDENTITY)
        self.assertFalse(manifest["external_source_vendored"])
        self.assertRegex(
            manifest["materialization"]["tree_oid"], r"^[0-9a-f]{40}$"
        )
        self.assertRegex(
            manifest["materialization"]["commit_oid"], r"^[0-9a-f]{40}$"
        )
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
        self.assertIn("never execute", readme)

    def test_expected_facts_have_exact_member_ids_and_hard_negatives(self) -> None:
        facts = load_json(EXPECTED_FACTS_PATH)
        self.assertEqual(facts["repository_identity"], REPOSITORY_IDENTITY)
        self.assertEqual(facts["ontology_version"], "codenoesis.ontology/rust/v5")
        self.assertEqual(set(facts["entity_counts"]), R5_ENTITY_KINDS | {"rust.method"})
        self.assertGreater(facts["entity_counts"]["rust.field"], 0)
        self.assertGreater(facts["entity_counts"]["rust.enum_variant"], 0)
        examples = facts["identity_examples"]
        self.assertGreaterEqual(len(examples), len(R5_ENTITY_KINDS))
        for example in examples:
            self.assertEqual(
                example["id"],
                stable_id(
                    "codenoesis.entity-id/rust-member/v1",
                    example["preimage"],
                ),
            )
        same_name_methods = facts["same_name_trait_method_examples"]
        self.assertEqual(len(same_name_methods), 2)
        self.assertEqual(
            {item["display_name"] for item in same_name_methods}, {"render"}
        )
        self.assertEqual(len({item["id"] for item in same_name_methods}), 2)
        self.assertEqual(len({item["trait_context_id"] for item in same_name_methods}), 2)
        self.assertEqual(
            set(facts["coverage_capability_states"]), set(CAPABILITY_STATES)
        )
        hard_negatives = set(facts["hard_negative_labels"])
        self.assertEqual(
            hard_negatives,
            {
                "comment_field_decoy",
                "string_variant_decoy",
                "macro_constant_decoy",
                "cfg_not_assumed_active",
                "build_sentinel_not_executed",
                "custom_attribute_not_interpreted",
            },
        )
        self.assertEqual(facts["forbidden_relationship_kinds"], ["CALLS", "EXECUTES"])

    def test_invalid_cases_cover_every_limit_and_security_boundary(self) -> None:
        cases = load_json(INVALID_CASES_PATH)
        self.assertEqual(cases["schema_version"], "codenoesis.r5-invalid-cases/v1")
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
                "unsupported_header",
                "limit_plus_one",
                "hard_negative",
                "forbidden_authority",
            }.issubset(classes)
        )
        self.assertTrue(all(not case["partial_publication"] for case in cases["cases"]))

    def test_error_v12_and_semantic_hash_contracts_are_exact(self) -> None:
        error = load_json(ERROR_SCHEMA_PATH)
        self.assertEqual(tuple(error["properties"]["code"]["enum"]), ERROR_CODES)
        self.assertEqual(
            tuple(
                branch["if"]["properties"]["code"]["const"]
                for branch in error["allOf"]
            ),
            ERROR_CODES,
        )
        self.assertEqual(
            set(error["$defs"]["limit_context"]["properties"]["limit"]["enum"]),
            set(LIMITS) - {"permutations"},
        )
        self.assertEqual(
            load_json(HASH_CONTRACT_PATH),
            {
                "schema_version": "codenoesis.semantic-hash-contract/v4",
                "algorithm": "blake3-256",
                "canonicalization": "RFC8785",
                "domain_separator_hex": "00",
                "hashes": {
                    "configuration": {
                        "domain": "codenoesis.configuration.semantic.v5",
                        "payload": "ConfigurationV5 without semantic_hash",
                    },
                    "extraction_chunk": {
                        "domain": "codenoesis.extraction-chunk.semantic.v5",
                        "payload": "ExtractionChunkV5 without semantic_hash",
                    },
                    "knowledge_graph": {
                        "domain": "codenoesis.knowledge-graph.semantic.v5",
                        "payload": "KnowledgeGraphV5 without semantic_hash",
                    },
                    "snapshot": {
                        "domain": "codenoesis.repository-snapshot.semantic.v8",
                        "payload": "RepositorySnapshotV8.semantic",
                    },
                },
            },
        )

    def test_red_observation_and_raw_log_are_immutable(self) -> None:
        observation = load_json(RED_OBSERVATION_PATH)
        self.assertEqual(observation["issue"], ISSUE_REFERENCE)
        self.assertEqual(observation["authorization"], AUTHORIZATION_REFERENCE)
        self.assertEqual(observation["required_base"], REQUIRED_BASE)
        self.assertEqual(observation["exit_code"], 1)
        self.assertTrue(observation["failed_for_expected_reason"])
        self.assertFalse(observation["semantic_contract_files_changed_before_red"])
        self.assertFalse(observation["production_files_changed_before_red"])
        self.assertEqual(observation["guard_sha256"], sha256_path(Path(__file__)))
        self.assertEqual(observation["raw_log_path"], RED_LOG_PATH.relative_to(ROOT).as_posix())
        self.assertEqual(observation["raw_log_bytes"], RED_LOG_PATH.stat().st_size)
        self.assertEqual(observation["raw_log_sha256"], sha256_path(RED_LOG_PATH))
        log = RED_LOG_PATH.read_text(encoding="utf-8")
        self.assertIn("Decision 0015", observation["expected_failure"])
        self.assertIn("R5 governance artifact is not materialized", log)
        self.assertIn(
            "docs/software/decisions/0015-s4-r5-rust-semantic-depth-contract.md",
            log,
        )

    def test_inherited_r4_bytes_are_immutable(self) -> None:
        for relative, expected in IMMUTABLE_FILES.items():
            with self.subTest(relative=relative):
                self.assertEqual(sha256_path(ROOT / relative), expected)
        bundle = load_json(ROOT / "tests/specifications/s4/r4/contract-bundle.json")
        self.assertEqual(
            bundle["bundle_sha256"],
            "2588abf38d686cc6475e7662ad8e90d585d1cdbff77702231dcadb1626a0c249",
        )

    def test_contract_bundle_binds_every_r5_governance_artifact(self) -> None:
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
        self.assertNotIn("tests/specifications/s4/r5/contract-bundle.json", paths)
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
            r"R5 Rust semantic-depth contract bundle:\s+"
            r"`sha256:([0-9a-f]{64})`",
            srs,
        )
        self.assertIsNotNone(match, "SRS must bind the complete R5 bundle")
        self.assertEqual(match.group(1), bundle_sha256)  # type: ignore[union-attr]


if __name__ == "__main__":
    unittest.main()
