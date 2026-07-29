from __future__ import annotations

import hashlib
import json
import re
import unittest
from copy import deepcopy
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
SRS_PATH = ROOT / "docs/software/software-requirements-specification.md"
DECISION_PATH = (
    ROOT / "docs/software/decisions/0006-s1-packed-sha1-acquisition-contract.md"
)
ORACLE_PATH = ROOT / "tests/specifications/s1/e2e_fr_acq_004_packed_sha1.json"
ERROR_SCHEMA_PATH = ROOT / "tests/specifications/s1/codenoesis-error-v6.schema.json"
CORPUS_SCHEMA_PATH = (
    ROOT / "tests/specifications/s1/corpus-descriptor-v1.schema.json"
)
CORPUS_PATH = ROOT / "tests/corpora/real-world-rust-v1.json"
FIXTURE_PATH = ROOT / "tests/fixtures/s1/packed-sha1-v1/manifest.json"
BUNDLE_PATH = ROOT / "tests/specifications/s1/packed-sha1-contract-bundle.json"

LIMITS = {
    "pack_directory_entries": 512,
    "pack_pairs": 64,
    "single_pack_index_bytes": 134_217_728,
    "cumulative_pack_index_bytes": 268_435_456,
    "indexed_objects": 8_000_000,
    "single_pack_bytes": 4_294_967_296,
    "cumulative_verified_pack_bytes": 8_589_934_592,
    "compressed_entry_bytes": 67_108_864,
    "inflated_entry_bytes": 268_435_456,
    "cumulative_entry_inflate_bytes": 1_073_741_824,
    "delta_program_bytes": 33_554_432,
    "delta_depth": 50,
    "delta_instructions": 4_194_304,
    "delta_intermediate_bytes": 268_435_456,
    "cumulative_delta_work_bytes": 1_073_741_824,
    "object_locations": 8,
    "reconstructed_object_cache_bytes": 134_217_728,
}

LEGACY_S1_LIMITS = {
    "regular_files": 20_000,
    "tree_entries": 25_000,
    "cumulative_file_bytes": 268_435_456,
    "single_file_bytes": 4_194_304,
    "path_bytes": 1_024,
    "path_component_bytes": 255,
    "recursion_depth": 32,
    "canonical_output_bytes": 33_554_432,
    "scan_wall_milliseconds": 60_000,
}

ALL_LIMITS = {**LEGACY_S1_LIMITS, **LIMITS}

MATERIALIZATION_ORDER = (
    "loose-control",
    "packed-base-only",
    "packed-ofs-delta",
    "packed-ref-delta",
    "mixed-cross-pack-ref-delta",
)

OBSERVATION_STEP_ORDER = (
    "fresh_clone",
    "verify_commit",
    "verify_tree",
    "tree_statistics",
    "pack_headers_and_sizes",
    "verify_pack",
    "packed_baseline",
    "materialize_loose_control",
    "loose_baseline",
)

RAW_OUTPUT_ORDER = (
    "verify_commit_stdout",
    "verify_tree_stdout",
    "tree_statistics_stdout",
    "pack_file",
    "index_file",
    "verify_pack_stdout",
    "packed_baseline_stderr",
    "loose_materialization_stdout",
    "loose_baseline_stderr",
)

INVALID_CONTEXT_SHAPES = (
    ("catalog", ("catalog_entry",), ()),
    (
        "index",
        ("index_layout", "index_fanout", "index_checksum", "sha1_collision"),
        ("pack_id",),
    ),
    ("index", ("index_object_order", "index_offset"), ("pack_id", "object_oid")),
    (
        "pack",
        (
            "pack_header",
            "pack_checksum",
            "pack_index_mismatch",
            "object_count",
            "sha1_collision",
        ),
        ("pack_id",),
    ),
    ("entry", ("entry_header", "entry_crc", "zlib_stream"), ("pack_id", "object_oid")),
    (
        "object",
        ("object_size", "object_oid", "sha1_collision", "duplicate_object_conflict"),
        ("object_oid",),
    ),
    ("delta", ("delta_base", "delta_cycle", "delta_program"), ("pack_id", "object_oid")),
)

INVALID_CONTEXT_CASES = tuple(
    (reason, component, required_fields)
    for component, reasons, required_fields in INVALID_CONTEXT_SHAPES
    for reason in reasons
)

TEST_ORDER = (
    "e2e_fr_acq_004_packed_sha1_equivalence",
    "conf_fr_acq_004_pack_catalog_v1",
    "conf_fr_acq_004_index_v2",
    "sec_fr_acq_004_pack_integrity",
    "conf_fr_acq_004_entry_integrity",
    "conf_fr_acq_004_ofs_delta",
    "conf_fr_acq_004_ref_delta",
    "sec_fr_acq_004_delta_adversarial",
    "pt_fr_acq_004_limits_have_max_and_plus_one",
    "pt_fr_acq_004_location_and_order_invariant",
    "race_fr_acq_004_pack_replacement",
    "sec_fr_acq_004_scan_has_no_new_authority",
    "fz_fr_acq_004_pack_index_delta_seed_corpus",
    "diff_fr_acq_004_offline_reference_readers",
    "reg_fr_acq_004_legacy_profiles_unchanged",
)

BUNDLE_FILES = {
    "LICENSE",
    "docs/software/decisions/0006-s1-packed-sha1-acquisition-contract.md",
    "scripts/tests/test_s1_packed_contract.py",
    "tests/corpora/real-world-rust-v1.json",
    "tests/fixtures/s1/packed-sha1-v1/README.md",
    "tests/fixtures/s1/packed-sha1-v1/manifest.json",
    "tests/specifications/s1/codenoesis-error-v6.schema.json",
    "tests/specifications/s1/contract-bundle.json",
    "tests/specifications/s1/corpus-descriptor-v1.schema.json",
    "tests/specifications/s1/e2e_fr_acq_004_packed_sha1.json",
    "tests/specifications/s4/contract-bundle.json",
}

EXPECTED_CORPUS = {
    "lekton": {
        "commit": "7a4d1a4a30468f4c18ce158a9b825680b00f4820",
        "tree": "fa81df35c6fa32068d1707d0eacdc459258877c5",
        "paths": 377,
        "blobs": 327,
        "trees": 50,
        "gitlinks": 0,
        "blob_bytes": 4_123_542,
        "pack_bytes": 2_973_530,
        "indexed_objects": 6_353,
        "delta_objects": 4_546,
        "next": "R3",
    },
    "rustdesk": {
        "commit": "d412d198720aa56f6cfed2dfad262e8fb1322fb7",
        "tree": "df8d4c292c9d256a445480eb878e507df3de1dc4",
        "paths": 1_118,
        "blobs": 940,
        "trees": 177,
        "gitlinks": 1,
        "blob_bytes": 15_808_673,
        "pack_bytes": 77_579_431,
        "indexed_objects": 86_521,
        "delta_objects": 64_793,
        "next": "R2",
    },
}


def load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def canonical_json(value: Any) -> bytes:
    return json.dumps(
        value, ensure_ascii=False, separators=(",", ":"), sort_keys=True
    ).encode()


def canonical_line_sha256(value: Any) -> str:
    return hashlib.sha256(canonical_json(value) + b"\n").hexdigest()


def corpus_normalized_log(entry: dict[str, Any]) -> list[str]:
    source = entry["source"]
    tree = entry["immutable_tree"]
    clone = entry["full_clone_observation"]
    packed = entry["baseline"]["current_packed_failure"]
    loose = entry["baseline"]["loose_control"]
    license_evidence = "|".join(
        f"{item['path']}:{item['sha256']}"
        for item in sorted(entry["license"]["evidence"], key=lambda item: item["path"])
    )
    packed_context = canonical_json(packed["stderr_context"]).decode()
    loose_context = canonical_json(loose["stderr_context"]).decode()
    normalized_log = [
        "procedure_version=codenoesis.public-corpus-observation/v1",
        f"repository_url={source['repository_url']}",
        f"pinned_commit={source['pinned_commit']}",
        f"tree_oid={source['tree_oid']}",
        f"commit_date={source['commit_date']}",
        f"object_format={source['object_format']}",
        f"paths={tree['paths']}",
        f"blobs={tree['blobs']}",
        f"trees={tree['trees']}",
        f"gitlinks={tree['gitlinks']}",
        f"symlinks={tree['symlinks']}",
        f"declared_blob_bytes={tree['declared_blob_bytes']}",
        f"cargo_manifests={tree['cargo_manifests']}",
        f"rust_files={tree['rust_files']}",
        f"license_evidence={license_evidence}",
        f"pack_count={clone['pack_count']}",
        f"pack_id={clone['pack_id']}",
        f"pack_bytes={clone['pack_bytes']}",
        f"index_bytes={clone['index_bytes']}",
        f"pack_version={clone['pack_version']}",
        f"index_version={clone['index_version']}",
        f"indexed_objects={clone['indexed_objects']}",
        f"delta_objects={clone['delta_objects']}",
        f"maximum_delta_depth={clone['maximum_delta_depth']}",
        f"packed_exit_code={packed['exit_code']}",
        f"packed_stderr_schema={packed['stderr_schema']}",
        f"packed_stderr_code={packed['stderr_code']}",
        f"packed_stderr_message={packed['stderr_message']}",
        f"packed_stderr_context={packed_context}",
        f"packed_stderr_sha256={packed['stderr_sha256']}",
        f"loose_exit_code={loose['exit_code']}",
        f"loose_stderr_schema={loose['stderr_schema']}",
        f"loose_stderr_code={loose['stderr_code']}",
        f"loose_stderr_message={loose['stderr_message']}",
        f"loose_stderr_context={loose_context}",
        f"loose_stderr_sha256={loose['stderr_sha256']}",
    ]
    raw_output = entry["reproduction"]["raw_output_sha256"]
    normalized_log.extend(
        f"raw_{name}_sha256={raw_output[name]}" for name in RAW_OUTPUT_ORDER
    )
    return normalized_log


SCHEMA_KEYWORDS = {
    "$schema",
    "$id",
    "$ref",
    "$defs",
    "title",
    "type",
    "additionalProperties",
    "required",
    "properties",
    "const",
    "enum",
    "allOf",
    "oneOf",
    "if",
    "then",
    "else",
    "minProperties",
    "maxProperties",
    "minLength",
    "maxLength",
    "pattern",
    "minimum",
    "maximum",
    "minItems",
    "maxItems",
    "uniqueItems",
    "prefixItems",
    "items",
}


def resolve_local_ref(root_schema: dict[str, Any], reference: str) -> Any:
    if not reference.startswith("#/"):
        raise AssertionError(f"only local JSON Schema references are allowed: {reference}")
    value: Any = root_schema
    for token in reference[2:].split("/"):
        token = token.replace("~1", "/").replace("~0", "~")
        if not isinstance(value, dict) or token not in value:
            raise AssertionError(f"unresolved JSON Schema reference: {reference}")
        value = value[token]
    return value


def assert_supported_schema(
    root_schema: dict[str, Any], schema: Any, path: str = "$"
) -> None:
    if isinstance(schema, bool):
        return
    if not isinstance(schema, dict):
        raise AssertionError(f"{path}: schema must be an object or boolean")
    unknown = set(schema) - SCHEMA_KEYWORDS
    if unknown:
        raise AssertionError(f"{path}: unsupported schema keywords {sorted(unknown)}")
    if "$ref" in schema:
        resolve_local_ref(root_schema, schema["$ref"])
    if "type" in schema and schema["type"] not in {
        "object",
        "array",
        "string",
        "integer",
        "boolean",
    }:
        raise AssertionError(f"{path}: unsupported type {schema['type']!r}")
    if "required" in schema:
        required = schema["required"]
        if (
            not isinstance(required, list)
            or not all(isinstance(item, str) for item in required)
            or len(required) != len(set(required))
        ):
            raise AssertionError(f"{path}: required must contain unique strings")
    if "enum" in schema:
        enum = schema["enum"]
        if not isinstance(enum, list) or not enum:
            raise AssertionError(f"{path}: enum must be a non-empty array")
        if len({canonical_json(item) for item in enum}) != len(enum):
            raise AssertionError(f"{path}: enum values must be unique")
    if "pattern" in schema:
        re.compile(schema["pattern"])
    for minimum, maximum in (
        ("minProperties", "maxProperties"),
        ("minLength", "maxLength"),
        ("minimum", "maximum"),
        ("minItems", "maxItems"),
    ):
        if minimum in schema and maximum in schema:
            if schema[minimum] > schema[maximum]:
                raise AssertionError(f"{path}: {minimum} exceeds {maximum}")
    for container in ("properties", "$defs"):
        if container in schema:
            values = schema[container]
            if not isinstance(values, dict):
                raise AssertionError(f"{path}.{container}: expected object")
            for name, child in values.items():
                assert_supported_schema(root_schema, child, f"{path}.{container}.{name}")
    for container in ("allOf", "oneOf", "prefixItems"):
        if container in schema:
            values = schema[container]
            if not isinstance(values, list) or not values:
                raise AssertionError(f"{path}.{container}: expected non-empty array")
            for index, child in enumerate(values):
                assert_supported_schema(root_schema, child, f"{path}.{container}[{index}]")
    for container in ("if", "then", "else", "items", "additionalProperties"):
        if container in schema and isinstance(schema[container], (dict, bool)):
            assert_supported_schema(
                root_schema, schema[container], f"{path}.{container}"
            )


def schema_accepts(root_schema: dict[str, Any], schema: Any, value: Any) -> bool:
    try:
        validate_schema_instance(root_schema, schema, value)
    except AssertionError:
        return False
    return True


def validate_schema_instance(
    root_schema: dict[str, Any], schema: Any, value: Any, path: str = "$"
) -> None:
    if schema is False:
        raise AssertionError(f"{path}: rejected by false schema")
    if schema is True:
        return
    if "$ref" in schema:
        validate_schema_instance(
            root_schema, resolve_local_ref(root_schema, schema["$ref"]), value, path
        )
    if "const" in schema and value != schema["const"]:
        raise AssertionError(f"{path}: value does not match const")
    if "enum" in schema and value not in schema["enum"]:
        raise AssertionError(f"{path}: value is outside enum")
    for child in schema.get("allOf", []):
        validate_schema_instance(root_schema, child, value, path)
    if "oneOf" in schema:
        matches = sum(
            schema_accepts(root_schema, child, value)
            for child in schema["oneOf"]
        )
        if matches != 1:
            raise AssertionError(f"{path}: expected one oneOf match, got {matches}")
    if "if" in schema:
        branch = "then" if schema_accepts(root_schema, schema["if"], value) else "else"
        if branch in schema:
            validate_schema_instance(root_schema, schema[branch], value, path)

    expected_type = schema.get("type")
    type_matches = {
        "object": isinstance(value, dict),
        "array": isinstance(value, list),
        "string": isinstance(value, str),
        "integer": isinstance(value, int) and not isinstance(value, bool),
        "boolean": isinstance(value, bool),
        None: True,
    }
    if not type_matches[expected_type]:
        raise AssertionError(f"{path}: expected {expected_type}")

    if isinstance(value, dict):
        required = schema.get("required", [])
        missing = set(required) - set(value)
        if missing:
            raise AssertionError(f"{path}: missing required properties {sorted(missing)}")
        properties = schema.get("properties", {})
        for name, child in properties.items():
            if name in value:
                validate_schema_instance(
                    root_schema, child, value[name], f"{path}.{name}"
                )
        additional = set(value) - set(properties)
        if schema.get("additionalProperties") is False and additional:
            raise AssertionError(
                f"{path}: additional properties are forbidden: {sorted(additional)}"
            )
        if isinstance(schema.get("additionalProperties"), dict):
            for name in additional:
                validate_schema_instance(
                    root_schema,
                    schema["additionalProperties"],
                    value[name],
                    f"{path}.{name}",
                )
        if len(value) < schema.get("minProperties", 0):
            raise AssertionError(f"{path}: too few properties")
        if len(value) > schema.get("maxProperties", len(value)):
            raise AssertionError(f"{path}: too many properties")

    if isinstance(value, list):
        if len(value) < schema.get("minItems", 0):
            raise AssertionError(f"{path}: too few items")
        if len(value) > schema.get("maxItems", len(value)):
            raise AssertionError(f"{path}: too many items")
        if schema.get("uniqueItems"):
            serialized = [canonical_json(item) for item in value]
            if len(serialized) != len(set(serialized)):
                raise AssertionError(f"{path}: items are not unique")
        prefix_items = schema.get("prefixItems", [])
        for index, child in enumerate(prefix_items[: len(value)]):
            validate_schema_instance(root_schema, child, value[index], f"{path}[{index}]")
        if "items" in schema:
            item_schema = schema["items"]
            start = len(prefix_items)
            for index in range(start, len(value)):
                validate_schema_instance(
                    root_schema, item_schema, value[index], f"{path}[{index}]"
                )

    if isinstance(value, str):
        if len(value) < schema.get("minLength", 0):
            raise AssertionError(f"{path}: string is too short")
        if len(value) > schema.get("maxLength", len(value)):
            raise AssertionError(f"{path}: string is too long")
        if "pattern" in schema and re.search(schema["pattern"], value) is None:
            raise AssertionError(f"{path}: string does not match pattern")

    if isinstance(value, (int, float)) and not isinstance(value, bool):
        if value < schema.get("minimum", value):
            raise AssertionError(f"{path}: number is below minimum")
        if value > schema.get("maximum", value):
            raise AssertionError(f"{path}: number exceeds maximum")


def strict_error(
    code: str,
    stage: str,
    context: dict[str, Any],
    *,
    retryable: bool = False,
) -> dict[str, Any]:
    return {
        "schema_version": "codenoesis.error/v6",
        "code": code,
        "stage": stage,
        "message": "bounded packed acquisition result",
        "retryable": retryable,
        "context": context,
    }


class PackedSha1GovernanceTests(unittest.TestCase):
    def test_oracle_selector_limits_red_and_traceability_are_exact(self) -> None:
        oracle = load_json(ORACLE_PATH)
        fixture = load_json(FIXTURE_PATH)

        self.assertEqual(oracle["status"], "approved")
        self.assertEqual(oracle["slice"], "S1")
        self.assertEqual(oracle["requirements"], ["FR-ACQ-004"])
        self.assertEqual(
            oracle["selector_contract"],
            {
                "flag": "--acquisition-profile",
                "value": "local-git-sha1-packed-v1",
                "kind": "explicit_operational_input",
                "valid_with_profiles": [
                    "standard-local-s1",
                    "standard-local-s2",
                    "standard-local-s3",
                    "standard-local-s4",
                ],
                "invalid_without_standard_profile": True,
                "implicit_repository_shape_dispatch": False,
                "included_in_configuration_semantic": False,
                "included_in_semantic_hash": False,
                "included_in_snapshot_identity": False,
                "reason": (
                    "loose and packed storage are physical representations "
                    "of the same verified Git objects"
                ),
            },
        )
        self.assertEqual(oracle["limits"], LIMITS)
        self.assertEqual(fixture["limits"], LIMITS)
        self.assertEqual(
            oracle["error_contract"]["new_codes"][
                "acquisition.object_database_invalid"
            ]["context_shapes"],
            [
                {
                    "component": component,
                    "reasons": list(reasons),
                    "required_fields": [
                        "component",
                        "reason",
                        *required_fields,
                    ],
                }
                for component, reasons, required_fields in INVALID_CONTEXT_SHAPES
            ],
        )
        self.assertEqual(tuple(oracle["test_order"]), TEST_ORDER)
        self.assertEqual(
            [test["test_name"] for test in oracle["acceptance_tests"]],
            list(TEST_ORDER),
        )
        self.assertEqual(
            oracle["expected_red"],
            {
                "test_name": "e2e_fr_acq_004_packed_sha1_equivalence",
                "command": (
                    "cargo test --locked -p noesis --test "
                    "e2e_fr_acq_004_packed_sha1 "
                    "e2e_fr_acq_004_packed_sha1_equivalence -- --exact"
                ),
                "fixture_state": (
                    "packed v2/index v2 source fixture with no reachable loose "
                    "fallback, materialized before subject launch"
                ),
                "subject_expected_success": (
                    "exit 0 and RepositorySnapshotV2 semantic bytes equal the "
                    "accepted loose golden"
                ),
                "accepted_preimplementation_exit": 2,
                "accepted_preimplementation_schema": "codenoesis.error/v2",
                "accepted_preimplementation_code": "input.invalid_revision",
                "accepted_reason": (
                    "Merged S0 through S4 do not recognize "
                    "--acquisition-profile and reject the unknown flag before "
                    "acquisition."
                ),
                "rejected_reasons": [
                    "compilation failure",
                    "missing test target",
                    "missing or corrupt fixture",
                    "empty pack sentinel without a valid packed closure",
                    "modified oracle",
                    "dependency outage",
                    "panic",
                    "timeout",
                    "race",
                    (
                        "legacy packed_object_database rejection without the "
                        "new explicit selector"
                    ),
                ],
            },
        )
        self.assertTrue(
            oracle["compatibility_contract"]["legacy_invocations"].startswith(
                "Every S0 through S4"
            )
        )
        self.assertTrue(
            fixture["equivalence_oracle"][
                "acquisition_selector_excluded_from_semantic_identity"
            ]
        )
        materializations = fixture["materializations"]
        self.assertEqual(
            [materialization["id"] for materialization in materializations],
            list(MATERIALIZATION_ORDER),
        )
        self.assertTrue(materializations[0]["reachable_loose_fallback"])
        self.assertEqual(
            [
                materialization["reachable_loose_fallback"]
                for materialization in materializations[1:]
            ],
            [False, False, False, False],
        )
        covered_invalid_contexts = {
            (recipe["component"], recipe["reason"])
            for recipe in fixture["mutation_recipes"]
        }
        covered_invalid_contexts.update(
            (case["expected_error"]["component"], case["expected_error"]["reason"])
            for case in fixture["catalog_cases"]
            if case.get("expected_error", {}).get("code")
            == "acquisition.object_database_invalid"
        )
        self.assertEqual(
            covered_invalid_contexts,
            {
                (component, reason)
                for reason, component, _ in INVALID_CONTEXT_CASES
            },
        )
        self.assertEqual(
            oracle["failure_precedence"][0],
            (
                "complete invocation validation including standard profile and "
                "acquisition selector before repository filesystem access"
            ),
        )
        self.assertIn(
            "bounded sequential entry map",
            oracle["failure_precedence"][6],
        )
        self.assertTrue(
            oracle["limit_contract"]["packed_limits_never_relax_inherited_limits"]
        )
        self.assertEqual(
            oracle["limit_contract"]["regular_file_blob"],
            (
                "single_file_bytes 4194304 is checked before retained "
                "semantic body reconstruction or allocation; the prerequisite "
                "discard-only structural inflate is separately charged to "
                "inflated_entry_bytes and cumulative_entry_inflate_bytes"
            ),
        )

    def test_schemas_validate_instances_and_reject_impossible_combinations(
        self,
    ) -> None:
        error_schema = load_json(ERROR_SCHEMA_PATH)
        corpus_schema = load_json(CORPUS_SCHEMA_PATH)
        corpus = load_json(CORPUS_PATH)
        for schema in (error_schema, corpus_schema):
            self.assertEqual(
                schema["$schema"],
                "https://json-schema.org/draft/2020-12/schema",
            )
            assert_supported_schema(schema, schema)
        validate_schema_instance(corpus_schema, corpus_schema, corpus)

        oid = "1" * 40
        pack_id = "2" * 40
        validate_schema_instance(
            error_schema,
            error_schema,
            strict_error("input.invalid_acquisition_profile", "input", {}),
        )
        for reason, component, required_fields in INVALID_CONTEXT_CASES:
            context: dict[str, Any] = {
                "component": component,
                "reason": reason,
            }
            for field in required_fields:
                context[field] = pack_id if field == "pack_id" else oid
            validate_schema_instance(
                error_schema,
                error_schema,
                strict_error(
                    "acquisition.object_database_invalid",
                    "acquisition",
                    context,
                ),
            )
        for limit, maximum in ALL_LIMITS.items():
            validate_schema_instance(
                error_schema,
                error_schema,
                strict_error(
                    "acquisition.limit_exceeded",
                    "acquisition",
                    {
                        "limit": limit,
                        "maximum": maximum,
                        "observed": maximum + 1,
                    },
                ),
            )
        validate_schema_instance(
            error_schema,
            error_schema,
            strict_error(
                "acquisition.object_database_changed",
                "acquisition",
                {"component": "catalog"},
                retryable=True,
            ),
        )
        validate_schema_instance(
            error_schema,
            error_schema,
            strict_error(
                "acquisition.object_database_unavailable",
                "acquisition",
                {"component": "pack"},
            ),
        )

        invalid_samples = [
            strict_error(
                "acquisition.object_database_invalid",
                "acquisition",
                {"component": "catalog", "reason": "delta_program"},
            ),
            strict_error(
                "acquisition.object_database_invalid",
                "acquisition",
                {"component": "index", "reason": "index_layout"},
            ),
            strict_error(
                "acquisition.object_database_invalid",
                "acquisition",
                {
                    "component": "object",
                    "reason": "object_size",
                    "object_oid": oid,
                    "pack_id": pack_id,
                },
            ),
            strict_error(
                "acquisition.object_database_invalid",
                "acquisition",
                {
                    "component": "delta",
                    "reason": "delta_program",
                    "pack_id": pack_id,
                },
            ),
            strict_error(
                "acquisition.limit_exceeded",
                "acquisition",
                {
                    "limit": "single_file_bytes",
                    "maximum": LEGACY_S1_LIMITS["single_file_bytes"] + 1,
                    "observed": LEGACY_S1_LIMITS["single_file_bytes"] + 2,
                },
            ),
            strict_error(
                "acquisition.limit_exceeded",
                "acquisition",
                {
                    "limit": "pack_pairs",
                    "maximum": LIMITS["pack_pairs"],
                    "observed": LIMITS["pack_pairs"] + 2,
                },
            ),
            strict_error(
                "acquisition.object_database_changed",
                "acquisition",
                {"component": "catalog", "pack_id": pack_id},
                retryable=True,
            ),
            strict_error(
                "acquisition.object_database_changed",
                "acquisition",
                {"component": "catalog"},
            ),
        ]
        leaked = deepcopy(invalid_samples[0])
        leaked["context"]["path"] = "objects/pack/untrusted"
        invalid_samples.append(leaked)
        for sample in invalid_samples:
            self.assertFalse(schema_accepts(error_schema, error_schema, sample))

        duplicate_corpus = deepcopy(corpus)
        duplicate_corpus["entries"][1] = deepcopy(duplicate_corpus["entries"][0])
        self.assertFalse(
            schema_accepts(corpus_schema, corpus_schema, duplicate_corpus)
        )

    def test_error_v6_is_closed_and_additive(self) -> None:
        schema = load_json(ERROR_SCHEMA_PATH)
        self.assertEqual(
            schema["properties"]["schema_version"]["const"],
            "codenoesis.error/v6",
        )
        codes = set(schema["properties"]["code"]["enum"])
        self.assertEqual(
            {
                "input.invalid_acquisition_profile",
                "acquisition.object_database_invalid",
                "acquisition.object_database_changed",
                "acquisition.object_database_unavailable",
            }
            - codes,
            set(),
        )
        context = schema["properties"]["context"]["properties"]
        self.assertEqual(
            set(context["component"]["enum"]),
            {"catalog", "index", "pack", "entry", "delta", "object"},
        )
        self.assertEqual(set(context["limit"]["enum"]), set(ALL_LIMITS))
        self.assertIn("sha1_collision", context["reason"]["enum"])
        self.assertIn("duplicate_object_conflict", context["reason"]["enum"])
        self.assertIn("promisor_object_database", context["feature"]["enum"])
        self.assertIn("multi_pack_index_only", context["feature"]["enum"])
        serialized = json.dumps(schema, sort_keys=True)
        self.assertIn('"const": "acquisition.object_database_changed"', serialized)
        self.assertIn('"const": true', serialized)
        self.assertEqual(context["pack_id"]["$ref"], "#/$defs/oid")
        invalid_rule = next(
            rule
            for rule in schema["allOf"]
            if rule["if"]["properties"]["code"].get("const")
            == "acquisition.object_database_invalid"
        )
        self.assertEqual(
            invalid_rule["then"]["properties"]["context"]["$ref"],
            "#/$defs/object_database_invalid_context",
        )
        invalid_context = schema["$defs"]["object_database_invalid_context"]
        self.assertFalse(invalid_context["additionalProperties"])
        self.assertEqual(
            set(invalid_context["properties"]),
            {"component", "reason", "pack_id", "object_oid"},
        )
        self.assertEqual(
            set(invalid_context["properties"]["reason"]["enum"]),
            {reason for reason, _, _ in INVALID_CONTEXT_CASES},
        )
        self.assertEqual(len(invalid_context["oneOf"]), 7)
        limit_context = schema["$defs"]["limit_context"]
        self.assertEqual(
            set(limit_context["properties"]["limit"]["enum"]),
            set(ALL_LIMITS),
        )
        self.assertEqual(len(limit_context["oneOf"]), len(ALL_LIMITS))

    def test_public_corpus_is_pinned_observed_and_replaceable(self) -> None:
        schema = load_json(CORPUS_SCHEMA_PATH)
        corpus = load_json(CORPUS_PATH)
        self.assertEqual(
            schema["properties"]["schema_version"]["const"],
            corpus["schema_version"],
        )
        self.assertEqual(corpus["status"], "validation_only")
        self.assertEqual(
            corpus["baseline_codenoesis_commit"],
            "5c56f22d5cd39fa869b338ae78b8b6bee64b92f6",
        )
        self.assertEqual(
            corpus["observation_environment"]["network_purpose"],
            "fixture_setup_and_public_corpus_observation_only",
        )
        self.assertEqual(
            [
                step["id"]
                for step in corpus["observation_procedure"]["ordered_steps"]
            ],
            list(OBSERVATION_STEP_ORDER),
        )
        self.assertEqual(
            corpus["observation_procedure"]["sanitized_environment"],
            (
                "unset Git directory, object, alternate, index, worktree, and "
                "injected-config variables; disable system and global Git "
                "config for every observation step"
            ),
        )
        materialize_step = corpus["observation_procedure"]["ordered_steps"][7]
        self.assertEqual(materialize_step["commands"][0][:2], ["sh", "-ceu"])
        materialize_script = materialize_step["commands"][0][2]
        for required_operation in (
            "unpack-objects",
            "objects/info/alternates",
            "fsck --full --no-reflogs",
            "loose_objects",
            "unset GIT_DIR",
            "GIT_ALTERNATE_OBJECT_DIRECTORIES",
            "GIT_CONFIG_NOSYSTEM=1",
            "find \"$pack_dir\"",
            "in_pack",
            "packs",
        ):
            self.assertIn(required_operation, materialize_script)
        self.assertEqual(
            [entry["id"] for entry in corpus["entries"]],
            list(EXPECTED_CORPUS),
        )
        entries = {entry["id"]: entry for entry in corpus["entries"]}
        self.assertEqual(set(entries), set(EXPECTED_CORPUS))
        for entry_id, expected in EXPECTED_CORPUS.items():
            entry = entries[entry_id]
            source = entry["source"]
            tree = entry["immutable_tree"]
            clone = entry["full_clone_observation"]
            baseline = entry["baseline"]
            self.assertEqual(source["pinned_commit"], expected["commit"])
            self.assertEqual(source["tree_oid"], expected["tree"])
            self.assertEqual(tree["paths"], expected["paths"])
            self.assertEqual(tree["blobs"], expected["blobs"])
            self.assertEqual(tree["trees"], expected["trees"])
            self.assertEqual(tree["gitlinks"], expected["gitlinks"])
            self.assertEqual(tree["declared_blob_bytes"], expected["blob_bytes"])
            self.assertTrue(clone["contextual_not_immutable"])
            self.assertEqual(clone["pack_version"], 2)
            self.assertEqual(clone["index_version"], 2)
            self.assertEqual(clone["pack_bytes"], expected["pack_bytes"])
            self.assertEqual(clone["indexed_objects"], expected["indexed_objects"])
            self.assertEqual(clone["delta_objects"], expected["delta_objects"])
            self.assertLessEqual(
                clone["pack_bytes"], LIMITS["single_pack_bytes"]
            )
            self.assertLessEqual(
                clone["indexed_objects"], LIMITS["indexed_objects"]
            )
            self.assertEqual(
                baseline["expected_next_roadmap_blocker"], expected["next"]
            )
            current = baseline["current_packed_failure"]
            self.assertEqual(current["exit_code"], 10)
            self.assertEqual(current["stdout_bytes"], 0)
            self.assertEqual(current["stderr_schema"], "codenoesis.error/v4")
            self.assertEqual(
                current["stderr_code"],
                "acquisition.unsupported_repository_shape",
            )
            self.assertEqual(
                current["stderr_context"], {"feature": "packed_object_database"}
            )
            for outcome in (current, baseline["loose_control"]):
                outcome_error = {
                    "code": outcome["stderr_code"],
                    "context": outcome["stderr_context"],
                    "message": outcome["stderr_message"],
                    "retryable": False,
                    "schema_version": outcome["stderr_schema"],
                    "stage": outcome["stderr_code"].split(".", 1)[0],
                }
                self.assertEqual(
                    outcome["stderr_sha256"],
                    canonical_line_sha256(outcome_error),
                )
            reproduction = entry["reproduction"]
            self.assertEqual(
                list(reproduction["raw_output_sha256"]),
                list(RAW_OUTPUT_ORDER),
            )
            self.assertEqual(
                reproduction["raw_output_sha256"]["packed_baseline_stderr"],
                current["stderr_sha256"],
            )
            self.assertEqual(
                reproduction["raw_output_sha256"]["loose_baseline_stderr"],
                baseline["loose_control"]["stderr_sha256"],
            )
            self.assertEqual(
                reproduction["raw_output_sha256"]["verify_commit_stdout"],
                hashlib.sha256(
                    f"{source['pinned_commit']}\n".encode()
                ).hexdigest(),
            )
            self.assertEqual(
                reproduction["raw_output_sha256"]["verify_tree_stdout"],
                hashlib.sha256(f"{source['tree_oid']}\n".encode()).hexdigest(),
            )
            self.assertEqual(
                reproduction["raw_output_sha256"][
                    "loose_materialization_stdout"
                ],
                canonical_line_sha256(
                    {
                        "commit": source["pinned_commit"],
                        "loose_objects": clone["indexed_objects"],
                        "procedure_version": (
                            "codenoesis.public-corpus-observation/v1"
                        ),
                        "tree": source["tree_oid"],
                    }
                ),
            )
            normalized_log = corpus_normalized_log(entry)
            self.assertEqual(reproduction["normalized_log"], normalized_log)
            self.assertEqual(
                reproduction["normalized_output_sha256"],
                canonical_line_sha256(normalized_log),
            )
            self.assertFalse(entry["replaceability"]["repository_specific_semantics"])
            self.assertEqual(
                entry["license"]["redistribution"],
                "descriptor_only_no_external_source_vendored",
            )
            for evidence in entry["license"]["evidence"]:
                self.assertRegex(evidence["sha256"], r"^[0-9a-f]{64}$")

        authority = corpus["authority"]
        self.assertFalse(authority["external_source_vendored"])
        self.assertFalse(authority["repository_specific_product_semantics"])
        self.assertFalse(authority["product_truth_source"])

    def test_srs_decision_and_ratification_are_machine_linked(self) -> None:
        oracle = load_json(ORACLE_PATH)
        approval_reference = oracle["ratification"]["approval_reference"]
        srs = SRS_PATH.read_text(encoding="utf-8")
        decision = DECISION_PATH.read_text(encoding="utf-8")

        self.assertIn("| Version | `0.9` |", srs)
        self.assertIn("### 2.8 S1 packed SHA-1 ratification register", srs)
        register = srs.split(
            "### 2.8 S1 packed SHA-1 ratification register", 1
        )[1].split("## 3. Product intent and success definition", 1)[0]
        rows = re.findall(
            r"^\| `(FR-ACQ-\d{3})` \| "
            r"`Proposed` \(pending protected merge\) \| `Approved` \|",
            register,
            flags=re.MULTILINE,
        )
        self.assertEqual(rows, ["FR-ACQ-004"])
        self.assertIsNotNone(
            re.search(
                r"^\| `FR-ACQ-004` \| P0 \| `0\.1` \|",
                srs,
                flags=re.MULTILINE,
            ),
            "FR-ACQ-004 must be a stable normative requirement",
        )
        self.assertIn("Decision 0006 resolves the packed local SHA-1 subset", srs)
        self.assertIn(
            "| `S1` Safe inventory | Snapshot contains supported files, language "
            "and manifest inventory, evidence, diagnostics, and coverage gaps. | "
            "`FR-ACQ-002`,",
            srs,
        )
        self.assertIn(
            "The `S1` row records the already Implemented base slice and is not "
            "reopened by",
            srs,
        )
        self.assertIn(approval_reference, register)
        self.assertIn("| Status | Accepted;", decision)
        self.assertIn("[#60](https://github.com/smutti/codenoesis/issues/60)", decision)
        self.assertIn(approval_reference, decision)
        self.assertIn("authoring agent must not approve or merge", decision)
        self.assertIn("separate policy-binding change", decision)
        self.assertIn(
            "historical decision index remains byte-identical", decision
        )

    def test_contract_bundle_binds_every_ratification_artifact(self) -> None:
        manifest = load_json(BUNDLE_PATH)
        self.assertEqual(
            set(manifest), {"schema_version", "files", "bundle_sha256"}
        )
        self.assertEqual(
            manifest["schema_version"], "codenoesis.contract-bundle/v1"
        )
        files = manifest["files"]
        paths = [entry["path"] for entry in files]
        self.assertEqual(paths, sorted(paths))
        self.assertEqual(set(paths), BUNDLE_FILES)
        self.assertEqual(len(paths), len(set(paths)))
        for entry in files:
            self.assertEqual(set(entry), {"path", "sha256"})
            self.assertRegex(entry["sha256"], r"^[0-9a-f]{64}$")
            path = Path(entry["path"])
            self.assertFalse(path.is_absolute())
            self.assertNotIn("..", path.parts)
            self.assertEqual(
                hashlib.sha256((ROOT / path).read_bytes()).hexdigest(),
                entry["sha256"],
            )
        payload = {
            "schema_version": manifest["schema_version"],
            "files": files,
        }
        bundle_sha256 = hashlib.sha256(canonical_json(payload)).hexdigest()
        self.assertEqual(manifest["bundle_sha256"], bundle_sha256)
        srs = SRS_PATH.read_text(encoding="utf-8")
        match = re.search(
            r"S1 packed SHA-1 contract bundle: `sha256:([0-9a-f]{64})`", srs
        )
        self.assertIsNotNone(match, "SRS must bind the complete new bundle")
        self.assertEqual(match.group(1), bundle_sha256)  # type: ignore[union-attr]

    def test_accepted_contract_lineage_files_are_unchanged(self) -> None:
        self.assertEqual(
            hashlib.sha256(
                (
                    ROOT / "tests/specifications/s1/contract-bundle.json"
                ).read_bytes()
            ).hexdigest(),
            "5e626841c94d7fb5c01a812ebbca62f583b85d2365546ce94ba9ed2b479d74a7",
        )
        self.assertEqual(
            hashlib.sha256(
                (
                    ROOT / "tests/specifications/s4/contract-bundle.json"
                ).read_bytes()
            ).hexdigest(),
            "be199ebbeb9cb35c2e6a68c5b9d847f86fe131efd007b0d09d9fd28390c91437",
        )


if __name__ == "__main__":
    unittest.main()
