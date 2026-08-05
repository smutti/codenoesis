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
DECISION_PATH = ROOT / "docs/software/decisions/0017-s4-r7-scip-import-contract.md"
SPEC_ROOT = ROOT / "tests/specifications/s4/r7"
SUBSET_PATH = SPEC_ROOT / "scip-rust-v0.9.0-subset-v1.json"
BINDING_SCHEMA_PATH = SPEC_ROOT / "compiler-index-binding-v1.schema.json"
CONFIGURATION_SCHEMA_PATH = SPEC_ROOT / "configuration-v7.schema.json"
SNAPSHOT_SCHEMA_PATH = SPEC_ROOT / "repository-snapshot-v10.schema.json"
CHUNK_SCHEMA_PATH = SPEC_ROOT / "extraction-chunk-v7.schema.json"
GRAPH_SCHEMA_PATH = SPEC_ROOT / "knowledge-graph-v7.schema.json"
ONTOLOGY_PATH = SPEC_ROOT / "rust-ontology-v7.json"
ERROR_SCHEMA_PATH = SPEC_ROOT / "codenoesis-error-v14.schema.json"
QUERY_SCHEMA_PATH = SPEC_ROOT / "local-query-result-v5.schema.json"
HASH_CONTRACT_PATH = SPEC_ROOT / "semantic-hash-contract-v6.json"
ORACLE_PATH = SPEC_ROOT / "e2e_fr_ext_005_scip_import.json"
QUERY_ORACLE_PATH = SPEC_ROOT / "e2e_fr_qry_001_r7_exact_id_results.json"
INVALID_CASES_PATH = SPEC_ROOT / "invalid-cases-v1.json"
SUPPLY_CHAIN_PATH = SPEC_ROOT / "supply-chain-v1.json"
RED_OBSERVATION_PATH = SPEC_ROOT / "red-observation.json"
RED_LOG_PATH = SPEC_ROOT / "red/governance-red.log"
RETAINED_GUARD_PATH = SPEC_ROOT / "red/test-first-guard.py"
BUNDLE_PATH = SPEC_ROOT / "contract-bundle.json"

FIXTURE_ROOT = ROOT / "tests/fixtures/s4/compiler-index-v1"
FIXTURE_REPOSITORY = FIXTURE_ROOT / "repository"
FIXTURE_MANIFEST_PATH = FIXTURE_ROOT / "manifest.json"
FIXTURE_SOURCE_PATH = FIXTURE_ROOT / "compiler-index-source.json"
FIXTURE_BINDING_PATH = FIXTURE_ROOT / "compiler-index-binding.json"
FIXTURE_INDEX_PATH = FIXTURE_ROOT / "index.scip"
EXPECTED_OVERLAY_PATH = FIXTURE_ROOT / "expected-compiler-overlay.json"

ISSUE_REFERENCE = "https://github.com/smutti/codenoesis/issues/123"
AUTHORIZATION_REFERENCE = (
    "https://github.com/smutti/codenoesis/issues/123#issuecomment-5193618752"
)
REQUIRED_BASE = "3f750201a1527c85ed2ce83f70ed0932213f3548"
R6_BUNDLE_SHA256 = "46f5e0fab0439979c456cb41ce7195efd5e02a342be4292402ef2cb44909bc47"
REPOSITORY_IDENTITY = "urn:codenoesis:fixture:s4-compiler-index-v1"
COMPILER_SYMBOL_DOMAIN = "codenoesis.entity-id/compiler-symbol/v1"
RELATIONSHIP_DOMAIN = "codenoesis.relationship-id/compiler-index/v1"
COMPILER_EVIDENCE_DOMAIN = "codenoesis.evidence-id/compiler-index/v1"
SCIP_TAG = "v0.9.0"
SCIP_COMMIT = "e8ee0ae6038f8298e2195812eea9d7b1196748ae"
SCIP_PROTO_SHA256 = "04cb20f2b8be73f6c0376b5b3e84c3ae20ebaff0ad3d23ba2d16f866b395ed7d"
RUST_ANALYZER_RELEASE = "2026-08-03"
RUST_ANALYZER_COMMIT = "b54a82b321c9617c5cf0b07ac0f12c08f7bc5902"

LIMITS = {
    "raw_index_bytes": 67_108_864,
    "binding_json_bytes": 1_048_576,
    "documents": 20_000,
    "occurrences_total": 1_000_000,
    "occurrences_per_document": 100_000,
    "symbol_information_total": 250_000,
    "relationships_total": 500_000,
    "symbol_or_display_bytes": 16_384,
    "unpromoted_value_bytes": 65_536,
    "tool_arguments": 128,
    "tool_argument_bytes": 4_096,
    "protobuf_recursion": 64,
    "permutations": 50,
}

COMPILER_STATES = {
    "in_repository_bound",
    "external_unbound",
    "generated_unbound",
}

RELATIONSHIP_KINDS = {
    "RESOLVES_TO",
    "REFERENCES",
    "IMPLEMENTS",
    "TYPE_DEFINITION",
}

COVERAGE_CAPABILITIES = {
    "compiler_index.call_semantics_unavailable",
    "compiler_index.generated_product_unbound",
    "compiler_index.document_not_indexed",
    "compiler_index.documentation_not_imported",
    "compiler_index.absolute_project_root_redacted",
    "compiler_index.arguments_redacted",
}

ERROR_CODES = (
    "input.invalid_compiler_index_profile",
    "input.unsafe_compiler_index_path",
    "extraction.unsupported_compiler_index_composition",
    "extraction.invalid_compiler_index_binding",
    "extraction.unsupported_compiler_index_schema",
    "extraction.unsupported_compiler_index_producer",
    "extraction.compiler_index_binding_mismatch",
    "extraction.malformed_compiler_index",
    "extraction.noncanonical_compiler_index",
    "extraction.compiler_index_identity_conflict",
    "extraction.ambiguous_compiler_index_endpoint",
    "extraction.compiler_index_relation_conflict",
    "extraction.compiler_index_limit_exceeded",
    "extraction.unresolvable_compiler_index_evidence",
    "internal.unexpected",
)

REQUIRED_TEST_NAMES = (
    "e2e_fr_ext_005_revision_bound_scip_import",
    "gt_fr_ext_005_cross_crate_symbol_resolution",
    "gt_fr_ext_005_explicit_implementation_and_type_relations",
    "gt_fr_ext_005_external_and_generated_symbols_remain_bounded",
    "conf_fr_ext_005_snapshot_v10_graph_v7_error_v14",
    "conf_fr_qry_001_v10_uses_local_query_result_v5",
    "pt_dr_idn_001_r7_global_local_symbol_identity_nfc",
    "pt_fr_ext_005_limits_have_max_and_plus_one",
    "pt_nfr_det_001_r7_permutation_and_replay_invariant",
    "sec_fr_ext_005_protobuf_preflight_precedes_decode",
    "sec_fr_ext_005_binding_path_race_and_privacy",
    "sec_fr_ext_005_never_executes_indexer_or_target",
    "e2e_fr_doc_001_r7_provenance_conflict_and_gap_wording",
    "reg_fr_cli_001_r7_selector_absence_is_byte_identical",
)

IMMUTABLE_R6_FILES = {
    "tests/specifications/s4/r6/contract-bundle.json": (
        "bab8814735d02a4f76ff94d4cf67036fefa19e5f81460fca99177e2c64823e70"
    ),
    "tests/specifications/s4/r6/rust-ontology-v6.json": (
        "d91c47426295b3fc633218cf386a79e69c7e19b69d575691e40be8b94a055818"
    ),
    "tests/specifications/s4/r6/repository-snapshot-v9.schema.json": (
        "3fed6f3f5b6ae51a6e5459ffd410584573bfca043febf9367c7b0abbd6f5fd65"
    ),
    "tests/specifications/s4/r6/local-query-result-v4.schema.json": (
        "83eb9cc246cda061feb9b924c2fbe3815f907013ec80a6f20423854770555598"
    ),
    "tests/specifications/s4/r6/extraction-chunk-v6.schema.json": (
        "c2efd6691ce402503cc9ce03da4d7cd138699def1050ea8fd63ca6f6c5f29c30"
    ),
}

SCHEMA_PATHS = (
    BINDING_SCHEMA_PATH,
    CONFIGURATION_SCHEMA_PATH,
    SNAPSHOT_SCHEMA_PATH,
    CHUNK_SCHEMA_PATH,
    GRAPH_SCHEMA_PATH,
    ERROR_SCHEMA_PATH,
    QUERY_SCHEMA_PATH,
)

FIXTURE_REPOSITORY_PATHS = (
    FIXTURE_REPOSITORY / "Cargo.toml",
    FIXTURE_REPOSITORY / "build.rs",
    FIXTURE_REPOSITORY / "crates/api/Cargo.toml",
    FIXTURE_REPOSITORY / "crates/api/src/lib.rs",
    FIXTURE_REPOSITORY / "crates/client/Cargo.toml",
    FIXTURE_REPOSITORY / "crates/client/src/lib.rs",
    FIXTURE_REPOSITORY / "crates/client/src/omitted.rs",
)

MATERIALIZED_PATHS = (
    DECISION_PATH,
    SUBSET_PATH,
    BINDING_SCHEMA_PATH,
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
    SUPPLY_CHAIN_PATH,
    RED_OBSERVATION_PATH,
    RED_LOG_PATH,
    RETAINED_GUARD_PATH,
    BUNDLE_PATH,
    FIXTURE_ROOT / "README.md",
    FIXTURE_MANIFEST_PATH,
    FIXTURE_SOURCE_PATH,
    FIXTURE_BINDING_PATH,
    FIXTURE_INDEX_PATH,
    EXPECTED_OVERLAY_PATH,
    *FIXTURE_REPOSITORY_PATHS,
)

BUNDLE_FILES = {
    "LICENSE",
    "docs/software/decisions/0017-s4-r7-scip-import-contract.md",
    "scripts/tests/test_s4_r7_compiler_index_contract.py",
    "tests/fixtures/s4/compiler-index-v1/README.md",
    "tests/fixtures/s4/compiler-index-v1/compiler-index-binding.json",
    "tests/fixtures/s4/compiler-index-v1/compiler-index-source.json",
    "tests/fixtures/s4/compiler-index-v1/expected-compiler-overlay.json",
    "tests/fixtures/s4/compiler-index-v1/index.scip",
    "tests/fixtures/s4/compiler-index-v1/manifest.json",
    "tests/fixtures/s4/compiler-index-v1/repository/Cargo.toml",
    "tests/fixtures/s4/compiler-index-v1/repository/build.rs",
    "tests/fixtures/s4/compiler-index-v1/repository/crates/api/Cargo.toml",
    "tests/fixtures/s4/compiler-index-v1/repository/crates/api/src/lib.rs",
    "tests/fixtures/s4/compiler-index-v1/repository/crates/client/Cargo.toml",
    "tests/fixtures/s4/compiler-index-v1/repository/crates/client/src/lib.rs",
    "tests/fixtures/s4/compiler-index-v1/repository/crates/client/src/omitted.rs",
    "tests/specifications/s4/r6/contract-bundle.json",
    "tests/specifications/s4/r7/codenoesis-error-v14.schema.json",
    "tests/specifications/s4/r7/compiler-index-binding-v1.schema.json",
    "tests/specifications/s4/r7/configuration-v7.schema.json",
    "tests/specifications/s4/r7/e2e_fr_ext_005_scip_import.json",
    "tests/specifications/s4/r7/e2e_fr_qry_001_r7_exact_id_results.json",
    "tests/specifications/s4/r7/extraction-chunk-v7.schema.json",
    "tests/specifications/s4/r7/invalid-cases-v1.json",
    "tests/specifications/s4/r7/knowledge-graph-v7.schema.json",
    "tests/specifications/s4/r7/local-query-result-v5.schema.json",
    "tests/specifications/s4/r7/red-observation.json",
    "tests/specifications/s4/r7/red/governance-red.log",
    "tests/specifications/s4/r7/red/test-first-guard.py",
    "tests/specifications/s4/r7/repository-snapshot-v10.schema.json",
    "tests/specifications/s4/r7/rust-ontology-v7.json",
    "tests/specifications/s4/r7/scip-rust-v0.9.0-subset-v1.json",
    "tests/specifications/s4/r7/semantic-hash-contract-v6.json",
    "tests/specifications/s4/r7/supply-chain-v1.json",
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


def stable_compiler_symbol_id(preimage: list[str]) -> str:
    normalized = [unicodedata.normalize("NFC", item) for item in preimage]
    digest = blake3_256(canonical_json([COMPILER_SYMBOL_DOMAIN, *normalized]))
    return f"urn:codenoesis:entity:blake3:{digest}"


def stable_relationship_id(kind: str, source: str, target: str) -> str:
    payload = [RELATIONSHIP_DOMAIN, kind, source, target]
    digest = blake3_256(canonical_json(payload))
    return f"urn:codenoesis:relationship:blake3:{digest}"


def compiler_evidence_id(artifact_sha256: str, locator: dict[str, Any]) -> str:
    payload = [COMPILER_EVIDENCE_DOMAIN, artifact_sha256, locator]
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


def encode_varint(value: int) -> bytes:
    if value < 0:
        value &= (1 << 64) - 1
    encoded = bytearray()
    while value > 0x7F:
        encoded.append((value & 0x7F) | 0x80)
        value >>= 7
    encoded.append(value)
    return bytes(encoded)


def encode_key(field_number: int, wire_type: int) -> bytes:
    return encode_varint((field_number << 3) | wire_type)


def encode_scalar(field_number: int, value: int) -> bytes:
    if value == 0:
        return b""
    return encode_key(field_number, 0) + encode_varint(value)


def encode_bytes(field_number: int, value: bytes) -> bytes:
    return encode_key(field_number, 2) + encode_varint(len(value)) + value


def encode_string(field_number: int, value: str) -> bytes:
    if not value:
        return b""
    return encode_bytes(field_number, value.encode("utf-8"))


def encode_packed_int32(field_number: int, values: list[int]) -> bytes:
    if not values:
        return b""
    return encode_bytes(field_number, b"".join(encode_varint(value) for value in values))


def encode_relationship(value: dict[str, Any]) -> bytes:
    result = encode_string(1, value["symbol"])
    result += encode_scalar(2, int(value.get("is_reference", False)))
    result += encode_scalar(3, int(value.get("is_implementation", False)))
    result += encode_scalar(4, int(value.get("is_type_definition", False)))
    result += encode_scalar(5, int(value.get("is_definition", False)))
    return result


def encode_symbol_information(value: dict[str, Any]) -> bytes:
    result = encode_string(1, value["symbol"])
    for documentation in value.get("documentation", []):
        result += encode_string(3, documentation)
    for relationship in value.get("relationships", []):
        result += encode_bytes(4, encode_relationship(relationship))
    result += encode_scalar(5, value.get("kind", 0))
    result += encode_string(6, value.get("display_name", ""))
    result += encode_string(8, value.get("enclosing_symbol", ""))
    return result


def encode_occurrence(value: dict[str, Any]) -> bytes:
    result = encode_packed_int32(1, value["range"])
    result += encode_string(2, value["symbol"])
    result += encode_scalar(3, value.get("symbol_roles", 0))
    for documentation in value.get("override_documentation", []):
        result += encode_string(4, documentation)
    result += encode_scalar(5, value.get("syntax_kind", 0))
    result += encode_packed_int32(7, value.get("enclosing_range", []))
    return result


def encode_document(value: dict[str, Any]) -> bytes:
    result = encode_string(1, value["relative_path"])
    for occurrence in value["occurrences"]:
        result += encode_bytes(2, encode_occurrence(occurrence))
    for symbol in value["symbols"]:
        result += encode_bytes(3, encode_symbol_information(symbol))
    result += encode_string(4, value["language"])
    result += encode_string(5, value.get("text", ""))
    result += encode_scalar(6, value["position_encoding"])
    return result


def encode_metadata(value: dict[str, Any]) -> bytes:
    tool = value["tool_info"]
    encoded_tool = encode_string(1, tool["name"])
    encoded_tool += encode_string(2, tool["version"])
    for argument in tool["arguments"]:
        encoded_tool += encode_string(3, argument)
    result = encode_scalar(1, value["version"])
    result += encode_bytes(2, encoded_tool)
    result += encode_string(3, value["project_root"])
    result += encode_scalar(4, value["text_document_encoding"])
    return result


def encode_index(value: dict[str, Any]) -> bytes:
    result = encode_bytes(1, encode_metadata(value["metadata"]))
    for document in value["documents"]:
        result += encode_bytes(2, encode_document(document))
    for symbol in value["external_symbols"]:
        result += encode_bytes(3, encode_symbol_information(symbol))
    return result


def decode_varint(value: bytes, offset: int) -> tuple[int, int]:
    result = 0
    shift = 0
    while offset < len(value) and shift < 70:
        byte = value[offset]
        offset += 1
        result |= (byte & 0x7F) << shift
        if byte < 0x80:
            return result, offset
        shift += 7
    raise ValueError("truncated or overlong protobuf varint")


def wire_fields(value: bytes) -> list[tuple[int, int, int | bytes]]:
    fields: list[tuple[int, int, int | bytes]] = []
    offset = 0
    while offset < len(value):
        key, offset = decode_varint(value, offset)
        field_number = key >> 3
        wire_type = key & 7
        if field_number == 0:
            raise ValueError("zero protobuf field number")
        if wire_type == 0:
            scalar, offset = decode_varint(value, offset)
            fields.append((field_number, wire_type, scalar))
        elif wire_type == 2:
            length, offset = decode_varint(value, offset)
            end = offset + length
            if end > len(value):
                raise ValueError("truncated protobuf length-delimited field")
            fields.append((field_number, wire_type, value[offset:end]))
            offset = end
        else:
            raise ValueError(f"unsupported fixture wire type: {wire_type}")
    return fields


class S4R7CompilerIndexGovernanceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        for path in MATERIALIZED_PATHS:
            if not path.is_file():
                raise AssertionError(
                    "R7 governance artifact is not materialized: "
                    f"{path.relative_to(ROOT)}"
                )

    def test_ratification_decision_and_roadmap_are_exact(self) -> None:
        srs = SRS_PATH.read_text(encoding="utf-8")
        decision = DECISION_PATH.read_text(encoding="utf-8")
        roadmap = ROADMAP_PATH.read_text(encoding="utf-8")
        for value in (
            "0.9+r7",
            "S4 R7 revision-bound SCIP import ratification register",
            ISSUE_REFERENCE,
            AUTHORIZATION_REFERENCE,
            "FR-EXT-005",
            "--compiler-index-profile scip-rust-v0.9.0-import-v1",
            "codenoesis.repository-snapshot/v10",
            "codenoesis.ontology/rust/v7",
            "codenoesis.local-query-result/v5",
            "codenoesis.error/v14",
        ):
            self.assertIn(value, srs)
        for value in (
            ISSUE_REFERENCE,
            AUTHORIZATION_REFERENCE,
            REQUIRED_BASE,
            SCIP_COMMIT,
            SCIP_PROTO_SHA256,
            "RepositorySnapshotV10",
            "ExtractionChunkV7",
            "KnowledgeGraphV7",
            "LocalQueryResultV5",
            "ErrorV14",
            "No `CALLS`, `EXECUTES`, or `SERVES`",
            "requires a separate Ready product issue",
        ):
            self.assertIn(value, decision)
        self.assertIn("R0-R6 are implemented", roadmap)
        self.assertIn("R6 → R7 → R8", roadmap)
        self.assertIn("R7 static import governance is Proposed", roadmap)
        self.assertIn("generation remains S9 work", roadmap)

    def test_oracle_binds_scope_selector_limits_and_dependencies(self) -> None:
        oracle = load_json(ORACLE_PATH)
        self.assertEqual(oracle["issue"], ISSUE_REFERENCE)
        self.assertEqual(oracle["authorization"], AUTHORIZATION_REFERENCE)
        self.assertEqual(oracle["required_base"], REQUIRED_BASE)
        self.assertEqual(oracle["requirement_ids"][0], "FR-EXT-005")
        self.assertEqual(
            oracle["requirement_status"],
            {
                "current": "Proposed",
                "target_after_protected_merge": "Approved for the bounded R7 profile",
            },
        )
        self.assertEqual(oracle["slice"], "S4")
        self.assertEqual(oracle["roadmap_capability"], "R7")
        self.assertEqual(oracle["risk"], "high")
        self.assertEqual(oracle["correction_rounds"], 4)
        self.assertEqual(
            oracle["selector"],
            {
                "flag": "--compiler-index-profile",
                "value": "scip-rust-v0.9.0-import-v1",
                "binding_flag": "--compiler-index-binding",
                "required_scan_profile": "standard-local-s4",
                "required_workspace_profile": "cargo-root-package-v1",
                "required_manifest_profile": "cargo-manifest-facts-v1",
                "required_semantic_profile": "rust-semantic-depth-v1",
                "required_framework_profile": "rust-framework-declarations-v1",
                "implicit_selection": False,
            },
        )
        self.assertEqual(oracle["limits"], LIMITS)
        self.assertEqual(tuple(oracle["required_test_names"]), REQUIRED_TEST_NAMES)
        self.assertEqual(
            oracle["immutable_dependencies"],
            {"r6_contract_bundle_sha256": R6_BUNDLE_SHA256},
        )
        self.assertEqual(
            oracle["future_product_dependencies"],
            [
                {"name": "protobuf", "version": "=3.7.2"},
                {"name": "scip", "version": "=0.9.0"},
            ],
        )
        self.assertTrue(oracle["production_implementation_forbidden"])
        self.assertTrue(oracle["index_generation_forbidden"])

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

    def test_versioned_shapes_overlay_and_query_dispatch_are_closed(self) -> None:
        configuration = load_json(CONFIGURATION_SCHEMA_PATH)
        self.assertEqual(
            configuration["properties"]["schema_version"]["const"],
            "codenoesis.configuration/v7",
        )
        self.assertEqual(
            configuration["properties"]["compiler_index_profile"]["const"],
            "scip-rust-v0.9.0-import-v1",
        )

        chunk = load_json(CHUNK_SCHEMA_PATH)
        self.assertEqual(
            chunk["properties"]["schema_version"]["const"],
            "codenoesis.extraction-chunk/v7",
        )
        self.assertEqual(
            chunk["properties"]["ontology_version"]["const"],
            "codenoesis.ontology/rust/v7",
        )
        compiler = chunk["$defs"]["compiler_symbol"]
        self.assertEqual(
            set(compiler["properties"]["binding_state"]["enum"]),
            COMPILER_STATES,
        )
        relation = chunk["$defs"]["compiler_relationship"]
        self.assertEqual(
            set(relation["properties"]["kind"]["enum"]),
            RELATIONSHIP_KINDS,
        )
        evidence = chunk["$defs"]["compiler_evidence"]
        self.assertEqual(
            evidence["properties"]["id"]["pattern"],
            "^urn:codenoesis:evidence:sha256:[0-9a-f]{64}$",
        )

        graph = load_json(GRAPH_SCHEMA_PATH)
        self.assertEqual(
            graph["properties"]["schema_version"]["const"],
            "codenoesis.knowledge-graph/v7",
        )
        snapshot = load_json(SNAPSHOT_SCHEMA_PATH)
        semantic = snapshot["$defs"]["semantic"]
        self.assertEqual(
            snapshot["properties"]["schema_version"]["const"],
            "codenoesis.repository-snapshot/v10",
        )
        self.assertEqual(
            semantic["properties"]["pipeline_version"]["const"],
            "codenoesis.pipeline/s4-r7-v1",
        )
        self.assertEqual(
            semantic["properties"]["extractor_contract_version"]["const"],
            "codenoesis.extraction/v7",
        )

        query = load_json(QUERY_SCHEMA_PATH)
        self.assertEqual(
            query["properties"]["schema_version"]["const"],
            "codenoesis.local-query-result/v5",
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

    def test_binding_and_fixture_manifest_bind_every_input_byte(self) -> None:
        manifest = load_json(FIXTURE_MANIFEST_PATH)
        self.assertEqual(manifest["schema_version"], "codenoesis.r7-fixture-manifest/v1")
        self.assertEqual(manifest["repository_identity"], REPOSITORY_IDENTITY)
        self.assertFalse(manifest["external_source_vendored"])
        self.assertFalse(manifest["fixture_may_execute"])
        files = manifest["files"]
        paths = [item["path"] for item in files]
        expected_paths = sorted(
            [
                "compiler-index-binding.json",
                "compiler-index-source.json",
                "index.scip",
                *[
                    path.relative_to(FIXTURE_ROOT).as_posix()
                    for path in FIXTURE_REPOSITORY_PATHS
                ],
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
        expected = manifest["expected_overlay"]
        self.assertEqual(expected["path"], EXPECTED_OVERLAY_PATH.name)
        self.assertEqual(expected["byte_length"], EXPECTED_OVERLAY_PATH.stat().st_size)
        self.assertEqual(expected["sha256"], sha256_path(EXPECTED_OVERLAY_PATH))

        binding = load_json(FIXTURE_BINDING_PATH)
        source = load_json(FIXTURE_SOURCE_PATH)
        raw_index = FIXTURE_INDEX_PATH.read_bytes()
        self.assertEqual(binding["schema_version"], "codenoesis.compiler-index-binding/v1")
        self.assertEqual(binding["repository"]["identity"], REPOSITORY_IDENTITY)
        self.assertEqual(binding["artifact"]["path"], "index.scip")
        self.assertEqual(binding["artifact"]["byte_length"], len(raw_index))
        self.assertEqual(binding["artifact"]["sha256"], sha256_bytes(raw_index))
        self.assertEqual(binding["artifact"]["scip_tag"], SCIP_TAG)
        self.assertEqual(binding["artifact"]["scip_commit"], SCIP_COMMIT)
        self.assertEqual(binding["artifact"]["scip_proto_sha256"], SCIP_PROTO_SHA256)
        self.assertEqual(binding["producer"]["name"], "rust-analyzer")
        self.assertEqual(binding["producer"]["version"], RUST_ANALYZER_RELEASE)
        self.assertEqual(binding["producer"]["commit"], RUST_ANALYZER_COMMIT)
        self.assertEqual(
            binding["producer"]["arguments_sha256"],
            sha256_bytes(canonical_json(source["metadata"]["tool_info"]["arguments"])),
        )
        self.assertEqual(
            binding["producer"]["project_root_sha256"],
            sha256_bytes(source["metadata"]["project_root"].encode("utf-8")),
        )
        expected_documents = []
        for document in source["documents"]:
            path = document["relative_path"]
            value = (FIXTURE_REPOSITORY / path).read_bytes()
            expected_documents.append(
                {
                    "path": path,
                    "blob_oid": git_blob_oid(value),
                    "sha256": sha256_bytes(value),
                    "byte_length": len(value),
                }
            )
        self.assertEqual(binding["documents"]["indexed"], expected_documents)
        self.assertEqual(
            binding["repository"]["source_manifest_sha256"],
            sha256_bytes(canonical_json(expected_documents)),
        )

    def test_binary_scip_is_canonical_and_matches_reviewed_source(self) -> None:
        source = load_json(FIXTURE_SOURCE_PATH)
        raw_index = FIXTURE_INDEX_PATH.read_bytes()
        self.assertEqual(raw_index, encode_index(source))
        top_level = wire_fields(raw_index)
        self.assertEqual([field[0] for field in top_level], [1, 2, 2, 3])
        self.assertTrue(all(field[1] == 2 for field in top_level))
        metadata = wire_fields(top_level[0][2])  # type: ignore[arg-type]
        self.assertEqual([field[0] for field in metadata], [2, 3, 4])
        documents = [field for field in top_level if field[0] == 2]
        self.assertEqual(len(documents), len(source["documents"]))
        self.assertLessEqual(len(raw_index), LIMITS["raw_index_bytes"])
        self.assertLessEqual(len(FIXTURE_BINDING_PATH.read_bytes()), LIMITS["binding_json_bytes"])

    def test_overlay_has_derived_identities_evidence_conflicts_and_gaps(self) -> None:
        overlay = load_json(EXPECTED_OVERLAY_PATH)
        raw_index_sha256 = sha256_path(FIXTURE_INDEX_PATH)
        self.assertEqual(overlay["repository_identity"], REPOSITORY_IDENTITY)
        self.assertEqual(overlay["ontology_version"], "codenoesis.ontology/rust/v7")
        self.assertEqual(overlay["artifact_sha256"], raw_index_sha256)
        symbols = overlay["compiler_symbols"]
        self.assertEqual(len({item["id"] for item in symbols}), len(symbols))
        self.assertEqual(set(overlay["binding_state_counts"]), COMPILER_STATES)
        for symbol in symbols:
            self.assertEqual(
                symbol["id"], stable_compiler_symbol_id(symbol["identity_preimage"])
            )
            self.assertIn(symbol["binding_state"], COMPILER_STATES)
            evidence = symbol["compiler_evidence"]
            self.assertEqual(
                evidence["id"],
                compiler_evidence_id(raw_index_sha256, evidence["locator"]),
            )
        relationships = overlay["relationships"]
        self.assertEqual(
            set(item["kind"] for item in relationships), RELATIONSHIP_KINDS
        )
        for relationship in relationships:
            self.assertEqual(
                relationship["id"],
                stable_relationship_id(
                    relationship["kind"],
                    relationship["source"],
                    relationship["target"],
                ),
            )
            self.assertGreaterEqual(len(relationship["evidence_ids"]), 1)
        self.assertEqual(
            set(item["capability"] for item in overlay["coverage_gaps"]),
            COVERAGE_CAPABILITIES,
        )
        self.assertNotIn("CALLS", overlay["relationship_counts"])
        self.assertTrue(overlay["syntax_evidence_retained_on_conflict"])
        public_bytes = canonical_json(overlay)
        for canary in (
            b"R7_SECRET_ARGUMENT_CANARY",
            b"R7_DOCUMENTATION_CANARY",
            b"/private/compiler-index-fixture",
        ):
            self.assertNotIn(canary, public_bytes)

    def test_subset_ontology_invalid_cases_and_errors_are_exact(self) -> None:
        subset = load_json(SUBSET_PATH)
        self.assertEqual(subset["limits"], LIMITS)
        self.assertEqual(set(subset["binding_states"]), COMPILER_STATES)
        self.assertEqual(set(subset["relationship_kinds"]), RELATIONSHIP_KINDS)
        self.assertEqual(
            set(subset["coverage_capabilities"]), COVERAGE_CAPABILITIES
        )
        self.assertFalse(subset["call_relationship_authorized"])
        self.assertFalse(subset["index_generation_authorized"])

        ontology = load_json(ONTOLOGY_PATH)
        self.assertEqual(
            ontology["extends"],
            {
                "ontology_version": "codenoesis.ontology/rust/v6",
                "contract_path": "tests/specifications/s4/r6/rust-ontology-v6.json",
                "contract_sha256": IMMUTABLE_R6_FILES[
                    "tests/specifications/s4/r6/rust-ontology-v6.json"
                ],
            },
        )
        self.assertEqual(ontology["compiler_entity_kind"], "compiler.symbol")
        self.assertEqual(set(ontology["binding_states"]), COMPILER_STATES)
        self.assertEqual(ontology["identity"]["domain"], COMPILER_SYMBOL_DOMAIN)
        self.assertEqual(set(ontology["relationship_kinds"]), RELATIONSHIP_KINDS)
        self.assertTrue(
            {"CALLS", "EXECUTES", "SERVES"}.issubset(
                set(ontology["forbidden_relationship_kinds"])
            )
        )

        cases = load_json(INVALID_CASES_PATH)
        self.assertEqual(cases["schema_version"], "codenoesis.r7-invalid-cases/v1")
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
                "artifact_mismatch",
                "binding_mismatch",
                "malformed_protobuf",
                "noncanonical_protobuf",
                "unknown_field",
                "duplicate_metadata",
                "unsupported_encoding",
                "invalid_symbol",
                "normalization_collision",
                "ambiguous_endpoint",
                "relation_conflict",
                "incomplete_coverage",
                "unsafe_path",
                "symlink_escape",
                "mutable_input_race",
                "privacy_boundary",
                "limit_plus_one",
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
                "schema_version": "codenoesis.semantic-hash-contract/v6",
                "algorithm": "blake3-256",
                "canonicalization": "RFC8785",
                "domain_separator_hex": "00",
                "hashes": {
                    "configuration": {
                        "domain": "codenoesis.configuration.semantic.v7",
                        "payload": "ConfigurationV7 without semantic_hash",
                    },
                    "extraction_chunk": {
                        "domain": "codenoesis.extraction-chunk.semantic.v7",
                        "payload": "ExtractionChunkV7 without semantic_hash",
                    },
                    "knowledge_graph": {
                        "domain": "codenoesis.knowledge-graph.semantic.v7",
                        "payload": "KnowledgeGraphV7 without semantic_hash",
                    },
                    "snapshot": {
                        "domain": "codenoesis.repository-snapshot.semantic.v10",
                        "payload": "RepositorySnapshotV10.semantic",
                    },
                },
            },
        )

    def test_query_dispatch_supply_chain_and_red_evidence_are_exact(self) -> None:
        query = load_json(QUERY_ORACLE_PATH)
        self.assertEqual(query["issue"], ISSUE_REFERENCE)
        self.assertEqual(query["authorization"], AUTHORIZATION_REFERENCE)
        self.assertEqual(
            query["dispatch"],
            {
                "codenoesis.repository-snapshot/v10": "codenoesis.local-query-result/v5",
                "codenoesis.repository-snapshot/v9": "codenoesis.local-query-result/v4",
                "codenoesis.repository-snapshot/v8": "codenoesis.local-query-result/v3",
                "codenoesis.repository-snapshot/v7": "codenoesis.local-query-result/v2",
                "codenoesis.repository-snapshot/v6": "codenoesis.local-query-result/v1",
                "codenoesis.repository-snapshot/v5": "codenoesis.local-query-result/v1",
                "codenoesis.repository-snapshot/v4": "codenoesis.local-query-result/v1",
                "explicit_query_version_flag": False,
            },
        )
        self.assertTrue(query["stored_head_validation_required"])
        self.assertFalse(query["call_semantics_authorized"])

        supply_chain = load_json(SUPPLY_CHAIN_PATH)
        self.assertEqual(
            [(item["name"], item["version"]) for item in supply_chain["dependencies"]],
            [("protobuf", "3.7.2"), ("scip", "0.9.0")],
        )
        self.assertEqual(
            {item["name"]: item["license"] for item in supply_chain["dependencies"]},
            {"protobuf": "MIT", "scip": "Apache-2.0"},
        )
        self.assertFalse(supply_chain["governance_manifest_changed"])
        self.assertFalse(supply_chain["governance_lockfile_changed"])
        self.assertFalse(supply_chain["build_time_codegen_authorized"])

        observation = load_json(RED_OBSERVATION_PATH)
        self.assertEqual(observation["issue"], ISSUE_REFERENCE)
        self.assertEqual(observation["authorization"], AUTHORIZATION_REFERENCE)
        self.assertEqual(observation["required_base"], REQUIRED_BASE)
        self.assertEqual(observation["exit_code"], 1)
        self.assertTrue(observation["failed_for_expected_reason"])
        self.assertFalse(observation["r7_contract_files_changed_before_red"])
        self.assertFalse(observation["production_files_changed_before_red"])
        self.assertEqual(
            observation["changed_paths_before_red"],
            ["scripts/tests/test_s4_r7_compiler_index_contract.py"],
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
        self.assertIn("R7 governance artifact is not materialized", log)
        self.assertIn(
            "docs/software/decisions/0017-s4-r7-scip-import-contract.md", log
        )
        self.assertIn("FAILED (errors=1)", log)

    def test_complete_r6_lineage_is_immutable(self) -> None:
        for relative, expected in IMMUTABLE_R6_FILES.items():
            with self.subTest(relative=relative):
                self.assertEqual(sha256_path(ROOT / relative), expected)
        bundle = load_json(ROOT / "tests/specifications/s4/r6/contract-bundle.json")
        self.assertEqual(bundle["bundle_sha256"], R6_BUNDLE_SHA256)

    def test_contract_bundle_binds_every_r7_governance_artifact(self) -> None:
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
        self.assertNotIn("tests/specifications/s4/r7/contract-bundle.json", paths)
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
            r"R7 revision-bound SCIP import contract bundle:\s+"
            r"`sha256:([0-9a-f]{64})`",
            srs,
        )
        self.assertIsNotNone(match, "SRS must bind the complete R7 bundle")
        self.assertEqual(match.group(1), bundle_sha256)  # type: ignore[union-attr]


if __name__ == "__main__":
    unittest.main()
