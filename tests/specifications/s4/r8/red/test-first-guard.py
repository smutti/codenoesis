from __future__ import annotations

import base64
import copy
import hashlib
import json
import random
import re
import unittest
from pathlib import Path
from typing import Any, Iterator
from urllib.parse import unquote


ROOT = Path(__file__).resolve().parents[2]
SRS_PATH = ROOT / "docs/software/software-requirements-specification.md"
ROADMAP_PATH = ROOT / "docs/software/roadmap.md"
DECISION_PATH = ROOT / "docs/software/decisions/0018-s4-r8-portable-explorer-contract.md"

SPEC_ROOT = ROOT / "tests/specifications/s4/r8"
PORTABLE_GRAPH_SCHEMA_PATH = SPEC_ROOT / "portable-graph-v1.schema.json"
EXPLORER_MANIFEST_SCHEMA_PATH = SPEC_ROOT / "local-explorer-manifest-v1.schema.json"
ERROR_SCHEMA_PATH = SPEC_ROOT / "codenoesis-error-v15.schema.json"
ORACLE_PATH = SPEC_ROOT / "e2e_fr_exp_001_portable_explorer.json"
INVALID_CASES_PATH = SPEC_ROOT / "invalid-security-cases-v1.json"
CSP_PATH = SPEC_ROOT / "explorer-content-security-policy-v1.json"
REIMPORT_PATH = SPEC_ROOT / "reimport-validation-v1.json"
RED_OBSERVATION_PATH = SPEC_ROOT / "red-observation.json"
RED_LOG_PATH = SPEC_ROOT / "red/governance-red.log"
RETAINED_GUARD_PATH = SPEC_ROOT / "red/test-first-guard.py"
BUNDLE_PATH = SPEC_ROOT / "contract-bundle.json"

FIXTURE_ROOT = ROOT / "tests/fixtures/s4/portable-explorer-v1"
FIXTURE_README_PATH = FIXTURE_ROOT / "README.md"
FIXTURE_MANIFEST_PATH = FIXTURE_ROOT / "manifest.json"
PORTABLE_GRAPH_PATH = FIXTURE_ROOT / "portable-graph.json"
SOURCE_FAMILY_DIGESTS_PATH = FIXTURE_ROOT / "source-family-digests.json"
EXPLORER_MANIFEST_PATH = FIXTURE_ROOT / "explorer-manifest.json"
EXPLORER_HTML_PATH = FIXTURE_ROOT / "index.html"

ISSUE_REFERENCE = "https://github.com/smutti/codenoesis/issues/110"
AUTHORIZATION_REFERENCE = (
    "https://github.com/smutti/codenoesis/issues/110#issuecomment-5205875435"
)
REQUIRED_BASE = "d003a563830bdb5ff79197c8b92050b23eb92b27"
R7_BUNDLE_SHA256 = "81ef2609c875af3d36a88f1fe97851f21368f90a60e2cc2706d6130ba95af882"
R7_BUNDLE_FILE_SHA256 = (
    "aa3c8384f3ef48499d7ae51652774177739a720fc20c329f22d387ee4f040e8e"
)

REPOSITORY_IDENTITY = "urn:codenoesis:fixture:s4-portable-explorer-v1"
PORTABLE_GRAPH_VERSION = "codenoesis.portable-graph/v1"
EXPLORER_VERSION = "codenoesis.local-explorer/v1"
SNAPSHOT_VERSION = "codenoesis.repository-snapshot/v10"
ONTOLOGY_VERSION = "codenoesis.ontology/rust/v7"
QUERY_VERSION = "codenoesis.local-query-result/v5"
ERROR_VERSION = "codenoesis.error/v15"

FAMILY_KEYS = (
    "entities",
    "relationships",
    "claims",
    "evidence",
    "diagnostics",
    "coverage_gaps",
    "documents",
    "document_statements",
)

FAMILY_ID_KEYS = {
    "entities": "id",
    "relationships": "id",
    "claims": "id",
    "evidence": "id",
    "diagnostics": "id",
    "coverage_gaps": "id",
    "documents": "document_id",
    "document_statements": "statement_id",
}

LIMITS = {
    "portable_graph_bytes": 268_435_456,
    "viewer_non_data_bytes": 1_048_576,
    "text_search_results": 100,
    "traversal_depth_default": 1,
    "traversal_depth_maximum": 2,
    "neighborhood_subjects": 256,
    "neighborhood_relationships": 512,
    "json_nesting": 64,
    "permutations": 50,
}

ERROR_CODES = (
    "input.invalid_export_profile",
    "input.invalid_explorer_profile",
    "input.unsafe_output_path",
    "export.invalid_snapshot",
    "export.unsupported_snapshot_schema",
    "export.unsupported_portable_graph_schema",
    "export.noncanonical_portable_graph",
    "export.identity_conflict",
    "export.reference_mismatch",
    "export.unresolved_evidence",
    "export.limit_exceeded",
    "explorer.invalid_projection",
    "explorer.unsafe_payload",
    "explorer.asset_integrity_mismatch",
    "explorer.limit_exceeded",
    "internal.unexpected",
)

REQUIRED_TEST_NAMES = (
    "e2e_fr_exp_001_export_and_explore_offline",
    "conf_fr_exp_001_portable_graph_v1_lossless_reimport",
    "conf_fr_cli_001_export_explore_are_explicit",
    "conf_fr_qry_001_r8_preserves_all_v5_subject_families",
    "conf_fr_qry_002_bounded_deterministic_neighborhood",
    "gt_fr_exp_001_exact_identity_reference_and_evidence_preservation",
    "gt_fr_exp_001_duplicate_loss_reorder_and_hash_rejection",
    "pt_nfr_det_001_r8_fifty_permutation_replay",
    "sec_fr_exp_001_xss_payloads_render_as_text",
    "sec_nfr_sec_001_r8_csp_forbids_active_remote_content",
    "sec_nfr_sec_005_r8_path_symlink_and_destination_confinement",
    "sec_nfr_prv_002_r8_excludes_source_contents_and_snippets",
    "pt_od_lim_001_r8_all_limits_have_maximum_plus_one",
    "reg_fr_qry_001_r7_exact_query_bytes_unchanged",
    "reg_fr_cli_001_r7_commands_and_stored_head_unchanged",
)

INVALID_CASE_IDS = {
    "duplicate_entity_id",
    "duplicate_relationship_id",
    "missing_relationship_endpoint",
    "missing_claim_subject",
    "missing_evidence_reference",
    "unknown_projection_field",
    "unsupported_projection_version",
    "snapshot_hash_mismatch",
    "noncanonical_family_order",
    "portable_graph_bytes_max_plus_one",
    "json_nesting_max_plus_one",
    "destination_non_empty_unmarked",
    "destination_marker_mismatch",
    "destination_parent_symlink_escape",
    "destination_component_dot_dot",
    "evidence_absolute_path",
    "evidence_parent_escape",
    "html_script_close",
    "html_attribute_quotes",
    "unicode_line_separator",
    "unicode_paragraph_separator",
    "unicode_bidi_override",
    "unicode_control_character",
    "oversized_display_label",
    "oversized_metadata",
    "remote_script_origin",
    "connect_source_enabled",
    "dynamic_code_evaluation",
    "viewer_asset_bytes_max_plus_one",
    "text_results_max_plus_one",
    "traversal_depth_max_plus_one",
    "neighborhood_subjects_max_plus_one",
    "neighborhood_relationships_max_plus_one",
}

IMMUTABLE_R7_FILES = {
    "tests/specifications/s4/r7/contract-bundle.json": R7_BUNDLE_FILE_SHA256,
    "tests/specifications/s4/r7/repository-snapshot-v10.schema.json": (
        "acc9631bd238535b3b9a7f1e16baf33dad90a857e69757eb2871d119c87056d9"
    ),
    "tests/specifications/s4/r7/knowledge-graph-v7.schema.json": (
        "82cede097309e10fd919568904607a41a130dfd17a4188e9c25d3b166cc12aba"
    ),
    "tests/specifications/s4/r7/local-query-result-v5.schema.json": (
        "8678aff01707d102f21d92f3c957b52cddccf21692acc324982ab5aea1b64d6f"
    ),
    "tests/specifications/s4/r7/extraction-chunk-v7.schema.json": (
        "c4775fbe91236e7f742af4bc523723359c30b1fa10bef93b5a8a33616246713f"
    ),
    "tests/specifications/s4/r7/rust-ontology-v7.json": (
        "3b5cda2d417b38ca58b24ed26282ac4c4336d87154257b1d208e066da9d80c35"
    ),
    "tests/specifications/s4/r7/codenoesis-error-v14.schema.json": (
        "f5570398cdebfa4d36e9694f32f4042a43e4d15a9215c58df62647cdc5a51358"
    ),
}

SCHEMA_PATHS = (
    PORTABLE_GRAPH_SCHEMA_PATH,
    EXPLORER_MANIFEST_SCHEMA_PATH,
    ERROR_SCHEMA_PATH,
)

MATERIALIZED_PATHS = (
    DECISION_PATH,
    PORTABLE_GRAPH_SCHEMA_PATH,
    EXPLORER_MANIFEST_SCHEMA_PATH,
    ERROR_SCHEMA_PATH,
    ORACLE_PATH,
    INVALID_CASES_PATH,
    CSP_PATH,
    REIMPORT_PATH,
    RED_OBSERVATION_PATH,
    RED_LOG_PATH,
    RETAINED_GUARD_PATH,
    BUNDLE_PATH,
    FIXTURE_README_PATH,
    FIXTURE_MANIFEST_PATH,
    PORTABLE_GRAPH_PATH,
    SOURCE_FAMILY_DIGESTS_PATH,
    EXPLORER_MANIFEST_PATH,
    EXPLORER_HTML_PATH,
)

BUNDLE_FILES = {
    "LICENSE",
    "docs/software/decisions/0018-s4-r8-portable-explorer-contract.md",
    "scripts/tests/test_s4_portable_explorer_contract.py",
    "tests/fixtures/s4/portable-explorer-v1/README.md",
    "tests/fixtures/s4/portable-explorer-v1/explorer-manifest.json",
    "tests/fixtures/s4/portable-explorer-v1/index.html",
    "tests/fixtures/s4/portable-explorer-v1/manifest.json",
    "tests/fixtures/s4/portable-explorer-v1/portable-graph.json",
    "tests/fixtures/s4/portable-explorer-v1/source-family-digests.json",
    "tests/specifications/s4/r7/contract-bundle.json",
    "tests/specifications/s4/r8/codenoesis-error-v15.schema.json",
    "tests/specifications/s4/r8/e2e_fr_exp_001_portable_explorer.json",
    "tests/specifications/s4/r8/explorer-content-security-policy-v1.json",
    "tests/specifications/s4/r8/invalid-security-cases-v1.json",
    "tests/specifications/s4/r8/local-explorer-manifest-v1.schema.json",
    "tests/specifications/s4/r8/portable-graph-v1.schema.json",
    "tests/specifications/s4/r8/red-observation.json",
    "tests/specifications/s4/r8/red/governance-red.log",
    "tests/specifications/s4/r8/red/test-first-guard.py",
    "tests/specifications/s4/r8/reimport-validation-v1.json",
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


def csp_hash(value: str) -> str:
    digest = hashlib.sha256(value.encode("utf-8")).digest()
    return "sha256-" + base64.b64encode(digest).decode("ascii")


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


def family_ids(graph: dict[str, Any], family: str) -> list[str]:
    id_key = FAMILY_ID_KEYS[family]
    return [item[id_key] for item in graph[family]]


def normalized_projection(graph: dict[str, Any]) -> dict[str, Any]:
    normalized = copy.deepcopy(graph)
    for family in FAMILY_KEYS:
        id_key = FAMILY_ID_KEYS[family]
        normalized[family] = sorted(normalized[family], key=lambda item: item[id_key])
    return normalized


def portable_graph_failure(graph: dict[str, Any]) -> str | None:
    if graph.get("schema_version") != PORTABLE_GRAPH_VERSION:
        return "export.unsupported_portable_graph_schema"
    if canonical_json(graph) != canonical_json(normalized_projection(graph)):
        return "export.noncanonical_portable_graph"

    identifiers: dict[str, set[str]] = {}
    for family in FAMILY_KEYS:
        values = family_ids(graph, family)
        if len(values) != len(set(values)):
            return "export.identity_conflict"
        identifiers[family] = set(values)

    entity_ids = identifiers["entities"]
    relationship_ids = identifiers["relationships"]
    evidence_ids = identifiers["evidence"]
    coverage_ids = identifiers["coverage_gaps"]

    for relationship in graph["relationships"]:
        if relationship["source"] not in entity_ids:
            return "export.reference_mismatch"
        if relationship["target"] not in entity_ids:
            return "export.reference_mismatch"
        if not set(relationship["evidence_ids"]).issubset(evidence_ids):
            return "export.unresolved_evidence"

    for claim in graph["claims"]:
        subjects = entity_ids if claim["subject_kind"] == "entity" else relationship_ids
        if claim["subject_id"] not in subjects:
            return "export.reference_mismatch"
        if not set(claim["evidence_ids"]).issubset(evidence_ids):
            return "export.unresolved_evidence"

    for entity in graph["entities"]:
        referenced = entity.get("compiler_evidence_ids", [])
        referenced += entity.get("source_evidence_ids", [])
        if not set(referenced).issubset(evidence_ids):
            return "export.unresolved_evidence"
        source_entity_id = entity.get("source_entity_id")
        if source_entity_id is not None and source_entity_id not in entity_ids:
            return "export.reference_mismatch"

    for diagnostic in graph["diagnostics"]:
        if diagnostic["subject_id"] not in entity_ids:
            return "export.reference_mismatch"
        compiler_target_id = diagnostic.get("compiler_target_id")
        if compiler_target_id is not None and compiler_target_id not in entity_ids:
            return "export.reference_mismatch"
        if not set(diagnostic["evidence_ids"]).issubset(evidence_ids):
            return "export.unresolved_evidence"

    for coverage_gap in graph["coverage_gaps"]:
        if not set(coverage_gap["evidence_ids"]).issubset(evidence_ids):
            return "export.unresolved_evidence"

    for document in graph["documents"]:
        if document["subject_id"] not in entity_ids | relationship_ids | coverage_ids:
            return "export.reference_mismatch"

    for statement in graph["document_statements"]:
        subject_ids = entity_ids | relationship_ids | coverage_ids
        if not set(statement["subject_ids"]).issubset(subject_ids):
            return "export.reference_mismatch"
        if not set(statement["evidence_ids"]).issubset(evidence_ids):
            return "export.unresolved_evidence"
        if not set(statement["coverage_gap_ids"]).issubset(coverage_ids):
            return "export.reference_mismatch"

    return None


class S4R8PortableExplorerGovernanceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        for path in MATERIALIZED_PATHS:
            if not path.is_file():
                raise AssertionError(
                    "R8 governance artifact is not materialized: "
                    f"{path.relative_to(ROOT)}"
                )

    def test_ratification_decision_srs_and_roadmap_are_exact(self) -> None:
        srs = SRS_PATH.read_text(encoding="utf-8")
        decision = DECISION_PATH.read_text(encoding="utf-8")
        roadmap = ROADMAP_PATH.read_text(encoding="utf-8")
        for value in (
            "0.9+r8",
            "S4 R8 portable export and offline explorer ratification register",
            ISSUE_REFERENCE,
            AUTHORIZATION_REFERENCE,
            "FR-EXP-001",
            "FR-QRY-002",
            "codenoesis.portable-graph/v1",
            "codenoesis.local-explorer/v1",
            SNAPSHOT_VERSION,
            ONTOLOGY_VERSION,
            QUERY_VERSION,
            ERROR_VERSION,
        ):
            self.assertIn(value, srs)
        for value in (
            ISSUE_REFERENCE,
            AUTHORIZATION_REFERENCE,
            REQUIRED_BASE,
            "RepositorySnapshotV10",
            "KnowledgeGraphV7",
            "LocalQueryResultV5",
            "PortableGraphV1",
            "LocalExplorerV1",
            "ErrorV15",
            "RFC8785",
            "requires a separate Ready product issue",
            "No browser auto-launch",
            "No source contents or snippets",
        ):
            self.assertIn(value, decision)
        self.assertIn("R0-R7 are implemented", roadmap)
        self.assertIn("R7 is implemented but not yet Verified", roadmap)
        self.assertIn("R7 → R8 → R9", roadmap)
        self.assertIn("R8 governance is Approved", roadmap)

    def test_oracle_binds_complete_governance_scope(self) -> None:
        oracle = load_json(ORACLE_PATH)
        self.assertEqual(oracle["issue"], ISSUE_REFERENCE)
        self.assertEqual(oracle["authorization"], AUTHORIZATION_REFERENCE)
        self.assertEqual(oracle["required_base"], REQUIRED_BASE)
        self.assertEqual(oracle["slice"], "S4")
        self.assertEqual(oracle["roadmap_capability"], "R8")
        self.assertEqual(oracle["risk"], "high")
        self.assertEqual(oracle["correction_rounds"], 4)
        self.assertEqual(
            oracle["requirement_ids"],
            [
                "FR-EXP-001",
                "FR-QRY-001",
                "FR-QRY-002",
                "FR-CLI-001",
                "FR-DOC-003",
                "NFR-DET-001",
                "NFR-SEC-001",
                "NFR-SEC-005",
                "NFR-PRV-002",
            ],
        )
        self.assertEqual(
            oracle["requirement_status"],
            {
                "current": "Proposed",
                "target_after_protected_merge": "Approved for the bounded R8 profile",
            },
        )
        self.assertEqual(
            oracle["contracts"],
            {
                "source_snapshot": SNAPSHOT_VERSION,
                "source_ontology": ONTOLOGY_VERSION,
                "source_query": QUERY_VERSION,
                "portable_graph": PORTABLE_GRAPH_VERSION,
                "local_explorer": EXPLORER_VERSION,
                "errors": ERROR_VERSION,
            },
        )
        self.assertEqual(oracle["limits"], LIMITS)
        self.assertEqual(tuple(oracle["required_test_names"]), REQUIRED_TEST_NAMES)
        self.assertEqual(
            oracle["immutable_dependencies"],
            {
                "r7_contract_bundle_sha256": R7_BUNDLE_SHA256,
                "r7_contract_bundle_file_sha256": R7_BUNDLE_FILE_SHA256,
            },
        )
        self.assertEqual(oracle["future_product_dependencies"], [])
        self.assertTrue(oracle["production_implementation_forbidden"])
        self.assertFalse(oracle["source_contents_included"])
        self.assertFalse(oracle["source_snippets_included"])
        self.assertFalse(oracle["browser_auto_launch"])
        self.assertFalse(oracle["network_service"])
        self.assertFalse(oracle["child_process"])

    def test_schemas_are_strict_and_every_reference_resolves(self) -> None:
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

    def test_portable_graph_and_explorer_shapes_reuse_r7_exactly(self) -> None:
        portable = load_json(PORTABLE_GRAPH_SCHEMA_PATH)
        self.assertEqual(
            portable["properties"]["schema_version"]["const"],
            PORTABLE_GRAPH_VERSION,
        )
        self.assertEqual(
            portable["properties"]["ontology_version"]["const"],
            ONTOLOGY_VERSION,
        )
        self.assertEqual(
            portable["properties"]["query_contract_version"]["const"],
            QUERY_VERSION,
        )
        self.assertEqual(
            portable["properties"]["source_snapshot"]["properties"][
                "schema_version"
            ]["const"],
            SNAPSHOT_VERSION,
        )
        expected_references = {
            "entities": "../r7/extraction-chunk-v7.schema.json#/$defs/graph_entity",
            "relationships": (
                "../r7/extraction-chunk-v7.schema.json#/$defs/graph_relationship"
            ),
            "claims": "../r7/extraction-chunk-v7.schema.json#/$defs/claim",
            "evidence": "../r7/extraction-chunk-v7.schema.json#/$defs/evidence",
            "diagnostics": (
                "../r7/extraction-chunk-v7.schema.json#/$defs/graph_diagnostic"
            ),
            "coverage_gaps": (
                "../r7/extraction-chunk-v7.schema.json#/$defs/graph_coverage"
            ),
            "documents": "../local-query-result-v1.schema.json#/$defs/document",
        }
        for family, reference in expected_references.items():
            self.assertEqual(portable["properties"][family]["items"]["$ref"], reference)
        self.assertEqual(
            portable["properties"]["document_statements"]["items"]["oneOf"],
            [
                {"$ref": "../local-query-result-v1.schema.json#/$defs/statement"},
                {
                    "$ref": (
                        "../local-query-result-v1.schema.json#/$defs/linked_statement"
                    )
                },
            ],
        )

        explorer = load_json(EXPLORER_MANIFEST_SCHEMA_PATH)
        self.assertEqual(
            explorer["properties"]["schema_version"]["const"], EXPLORER_VERSION
        )
        self.assertEqual(
            explorer["properties"]["portable_graph"]["properties"]["path"]["const"],
            "portable-graph.json",
        )
        self.assertEqual(
            explorer["properties"]["entrypoint"]["properties"]["path"]["const"],
            "index.html",
        )

    def test_fixture_manifest_binds_every_byte_and_canonical_graph(self) -> None:
        manifest = load_json(FIXTURE_MANIFEST_PATH)
        self.assertEqual(
            manifest["schema_version"], "codenoesis.r8-fixture-manifest/v1"
        )
        self.assertEqual(manifest["repository_identity"], REPOSITORY_IDENTITY)
        self.assertFalse(manifest["external_source_vendored"])
        self.assertFalse(manifest["fixture_may_execute"])
        self.assertTrue(manifest["useful_without_source_repository"])
        files = manifest["files"]
        paths = [item["path"] for item in files]
        expected_paths = sorted(
            [
                "explorer-manifest.json",
                "index.html",
                "portable-graph.json",
                "source-family-digests.json",
            ]
        )
        self.assertEqual(paths, expected_paths)
        self.assertEqual(len(paths), len(set(paths)))
        for item in files:
            source = FIXTURE_ROOT / item["path"]
            value = source.read_bytes()
            self.assertEqual(item["mode"], "100644")
            self.assertEqual(item["byte_length"], len(value))
            self.assertEqual(item["sha256"], sha256_bytes(value))
            self.assertEqual(item["git_blob_oid"], git_blob_oid(value))

        graph = load_json(PORTABLE_GRAPH_PATH)
        self.assertEqual(PORTABLE_GRAPH_PATH.read_bytes(), canonical_json(graph))
        self.assertLessEqual(len(PORTABLE_GRAPH_PATH.read_bytes()), LIMITS["portable_graph_bytes"])
        self.assertIsNone(portable_graph_failure(graph))

    def test_portable_graph_is_lossless_sorted_and_private(self) -> None:
        graph = load_json(PORTABLE_GRAPH_PATH)
        self.assertEqual(graph["schema_version"], PORTABLE_GRAPH_VERSION)
        self.assertEqual(graph["repository"]["identity"], REPOSITORY_IDENTITY)
        self.assertEqual(graph["source_snapshot"]["schema_version"], SNAPSHOT_VERSION)
        self.assertEqual(graph["ontology_version"], ONTOLOGY_VERSION)
        self.assertEqual(graph["query_contract_version"], QUERY_VERSION)
        self.assertEqual(
            graph["projection"],
            {
                "canonicalization": "RFC8785",
                "family_order": list(FAMILY_KEYS),
                "identity_preservation": "exact",
                "reference_preservation": "exact",
                "evidence_preservation": "lossless_redacted_metadata",
                "claim_state_policy": "preserve_without_upgrade",
                "unknown_fields_allowed": False,
                "source_contents_included": False,
                "source_snippets_included": False,
            },
        )
        for family in FAMILY_KEYS:
            identifiers = family_ids(graph, family)
            self.assertGreater(len(identifiers), 0, family)
            self.assertEqual(identifiers, sorted(identifiers), family)
            self.assertEqual(len(identifiers), len(set(identifiers)), family)
        forbidden_keys = {
            "source_text",
            "source_content",
            "source_contents",
            "snippet",
            "snippets",
            "raw_source",
        }
        for item in walk_json(graph):
            self.assertTrue(forbidden_keys.isdisjoint(item))
        self.assertIsNone(portable_graph_failure(graph))

        digest_manifest = load_json(SOURCE_FAMILY_DIGESTS_PATH)
        self.assertEqual(
            digest_manifest["schema_version"],
            "codenoesis.source-family-digests/v1",
        )
        self.assertEqual(
            digest_manifest["portable_graph_canonical_sha256"],
            sha256_bytes(canonical_json(graph)),
        )
        self.assertEqual(list(digest_manifest["families"]), list(FAMILY_KEYS))
        for family in FAMILY_KEYS:
            expected = digest_manifest["families"][family]
            self.assertEqual(expected["count"], len(graph[family]))
            self.assertEqual(expected["ids"], family_ids(graph, family))
            self.assertEqual(
                expected["canonical_sha256"], sha256_bytes(canonical_json(graph[family]))
            )

    def test_reimport_and_fifty_permutations_preserve_exact_bytes(self) -> None:
        graph = load_json(PORTABLE_GRAPH_PATH)
        matrix = load_json(REIMPORT_PATH)
        canonical_sha256 = sha256_bytes(canonical_json(graph))
        self.assertEqual(
            matrix["schema_version"], "codenoesis.reimport-validation/v1"
        )
        self.assertEqual(matrix["fixture"], "portable-explorer-v1/portable-graph.json")
        self.assertEqual(matrix["canonical_sha256"], canonical_sha256)
        self.assertEqual(
            matrix["lossless_families"],
            list(FAMILY_KEYS),
        )
        self.assertEqual(
            matrix["valid_cases"],
            [
                "canonical_round_trip",
                "source_repository_absent",
                "viewer_not_generated",
                "all_claim_states_preserved",
                "all_gaps_and_diagnostics_preserved",
            ],
        )
        permutation_results = matrix["permutation_results"]
        self.assertEqual(len(permutation_results), LIMITS["permutations"])
        self.assertEqual(
            [item["seed"] for item in permutation_results],
            list(range(LIMITS["permutations"])),
        )
        for item in permutation_results:
            permuted = copy.deepcopy(graph)
            generator = random.Random(item["seed"])
            for family in FAMILY_KEYS:
                generator.shuffle(permuted[family])
            normalized = normalized_projection(permuted)
            self.assertEqual(sha256_bytes(canonical_json(normalized)), canonical_sha256)
            self.assertEqual(item["expected_canonical_sha256"], canonical_sha256)
            self.assertEqual(item["expected_result"], "accepted_byte_identical")

    def test_invalid_security_cases_cover_every_boundary(self) -> None:
        invalid = load_json(INVALID_CASES_PATH)
        self.assertEqual(
            invalid["schema_version"], "codenoesis.r8-invalid-security-cases/v1"
        )
        cases = invalid["cases"]
        identifiers = [item["id"] for item in cases]
        self.assertEqual(set(identifiers), INVALID_CASE_IDS)
        self.assertEqual(identifiers, sorted(identifiers))
        self.assertEqual(len(identifiers), len(set(identifiers)))
        for item in cases:
            self.assertIn(item["expected_code"], ERROR_CODES)
            self.assertFalse(item["repair_allowed"])
            self.assertFalse(item["silent_truncation_allowed"])
        payloads = {item["id"]: item.get("payload") for item in cases}
        self.assertEqual(payloads["html_script_close"], "</script><script>alert(1)</script>")
        self.assertEqual(payloads["unicode_line_separator"], "line\u2028break")
        self.assertEqual(payloads["unicode_paragraph_separator"], "line\u2029break")
        self.assertIn("\u202e", payloads["unicode_bidi_override"])
        self.assertIn("\u0001", payloads["unicode_control_character"])

    def test_static_explorer_csp_hashes_and_rendering_are_closed(self) -> None:
        contract = load_json(CSP_PATH)
        html = EXPLORER_HTML_PATH.read_text(encoding="utf-8")
        style_match = re.search(r"<style>(.*?)</style>", html, flags=re.DOTALL)
        script_match = re.search(r"<script>(.*?)</script>", html, flags=re.DOTALL)
        self.assertIsNotNone(style_match)
        self.assertIsNotNone(script_match)
        assert style_match is not None
        assert script_match is not None
        style_hash = csp_hash(style_match.group(1))
        script_hash = csp_hash(script_match.group(1))
        self.assertEqual(contract["inline_style_sha256"], style_hash)
        self.assertEqual(contract["inline_script_sha256"], script_hash)
        self.assertIn(contract["content_security_policy"], html)
        self.assertIn(style_hash, contract["content_security_policy"])
        self.assertIn(script_hash, contract["content_security_policy"])
        self.assertEqual(
            contract["directives"],
            {
                "default-src": ["'none'"],
                "script-src": [f"'{script_hash}'"],
                "style-src": [f"'{style_hash}'"],
                "img-src": ["'self'", "data:"],
                "font-src": ["'none'"],
                "connect-src": ["'none'"],
                "object-src": ["'none'"],
                "frame-src": ["'none'"],
                "frame-ancestors": ["'none'"],
                "form-action": ["'none'"],
                "base-uri": ["'none'"],
                "manifest-src": ["'none'"],
                "media-src": ["'none'"],
                "worker-src": ["'none'"],
            },
        )
        self.assertEqual(contract["untrusted_rendering"], "textContent_only")
        self.assertTrue(contract["explicit_file_selection_required"])
        self.assertFalse(contract["browser_auto_launch"])
        self.assertFalse(contract["network_allowed"])
        self.assertLessEqual(len(EXPLORER_HTML_PATH.read_bytes()), LIMITS["viewer_non_data_bytes"])
        for forbidden in (
            "innerHTML",
            "insertAdjacentHTML",
            "document.write",
            "eval(",
            "new Function",
            "fetch(",
            "XMLHttpRequest",
            "WebSocket",
            "EventSource",
            "localStorage",
            "sessionStorage",
            "indexedDB",
            "document.cookie",
            "window.open",
            "serviceWorker",
            "<script src=",
            "<iframe",
            "<form",
            "http://",
            "https://",
        ):
            self.assertNotIn(forbidden, html)
        for required in (
            "textContent",
            "file.text()",
            "normalize(\"NFC\")",
            "MAX_TEXT_RESULTS = 100",
            "MAX_DEPTH = 2",
            "MAX_SUBJECTS = 256",
            "MAX_RELATIONSHIPS = 512",
        ):
            self.assertIn(required, html)

        manifest = load_json(EXPLORER_MANIFEST_PATH)
        self.assertEqual(manifest["schema_version"], EXPLORER_VERSION)
        self.assertEqual(manifest["portable_graph"]["sha256"], sha256_path(PORTABLE_GRAPH_PATH))
        self.assertEqual(manifest["entrypoint"]["sha256"], sha256_path(EXPLORER_HTML_PATH))
        self.assertEqual(manifest["security"]["csp_sha256"], sha256_path(CSP_PATH))
        self.assertEqual(manifest["limits"], {key: LIMITS[key] for key in (
            "text_search_results",
            "traversal_depth_default",
            "traversal_depth_maximum",
            "neighborhood_subjects",
            "neighborhood_relationships",
        )})

    def test_error_v15_is_closed_and_limit_context_is_exact(self) -> None:
        error = load_json(ERROR_SCHEMA_PATH)
        self.assertEqual(error["properties"]["schema_version"]["const"], ERROR_VERSION)
        self.assertEqual(tuple(error["properties"]["code"]["enum"]), ERROR_CODES)
        self.assertEqual(
            set(error["properties"]["stage"]["enum"]),
            {"input", "export", "explorer", "internal"},
        )
        self.assertEqual(
            set(error["$defs"]["limit_context"]["properties"]["limit"]["enum"]),
            set(LIMITS) - {"permutations", "traversal_depth_default"},
        )
        conditional_codes = {
            item["if"]["properties"]["code"]["const"] for item in error["allOf"]
        }
        self.assertEqual(conditional_codes, set(ERROR_CODES))

    def test_red_evidence_is_retained_and_exact(self) -> None:
        observation = load_json(RED_OBSERVATION_PATH)
        self.assertEqual(observation["issue"], ISSUE_REFERENCE)
        self.assertEqual(observation["authorization"], AUTHORIZATION_REFERENCE)
        self.assertEqual(observation["required_base"], REQUIRED_BASE)
        self.assertEqual(observation["exit_code"], 1)
        self.assertTrue(observation["failed_for_expected_reason"])
        self.assertFalse(observation["r8_contract_files_changed_before_red"])
        self.assertFalse(observation["production_files_changed_before_red"])
        self.assertFalse(observation["dependency_files_changed_before_red"])
        self.assertEqual(
            observation["changed_paths_before_red"],
            ["scripts/tests/test_s4_portable_explorer_contract.py"],
        )
        self.assertRegex(observation["guard_commit_sha"], r"^[0-9a-f]{40}$")
        self.assertEqual(observation["guard_sha256"], sha256_path(RETAINED_GUARD_PATH))
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
        self.assertIn("R8 governance artifact is not materialized", log)
        self.assertIn(
            "docs/software/decisions/0018-s4-r8-portable-explorer-contract.md",
            log,
        )
        self.assertIn("FAILED (errors=1)", log)

    def test_complete_r7_lineage_is_immutable(self) -> None:
        for relative, expected in IMMUTABLE_R7_FILES.items():
            with self.subTest(relative=relative):
                self.assertEqual(sha256_path(ROOT / relative), expected)
        bundle = load_json(ROOT / "tests/specifications/s4/r7/contract-bundle.json")
        self.assertEqual(bundle["bundle_sha256"], R7_BUNDLE_SHA256)

    def test_contract_bundle_binds_every_r8_governance_artifact(self) -> None:
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
        self.assertNotIn("tests/specifications/s4/r8/contract-bundle.json", paths)
        for item in files:
            self.assertEqual(set(item), {"path", "sha256"})
            path = Path(item["path"])
            self.assertFalse(path.is_absolute())
            self.assertNotIn("..", path.parts)
            self.assertRegex(item["sha256"], r"^[0-9a-f]{64}$")
            self.assertEqual(sha256_path(ROOT / path), item["sha256"])
        payload = {"schema_version": bundle["schema_version"], "files": files}
        self.assertEqual(bundle["bundle_sha256"], sha256_bytes(canonical_json(payload)))
        srs = SRS_PATH.read_text(encoding="utf-8")
        self.assertIn(bundle["bundle_sha256"], srs)


if __name__ == "__main__":
    unittest.main()
