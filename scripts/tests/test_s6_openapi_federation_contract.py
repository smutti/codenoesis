from __future__ import annotations

import copy
import hashlib
import json
import re
import unittest
from pathlib import Path
from typing import Any

from test_s1_contract import blake3_256, canonical_json


ROOT = Path(__file__).resolve().parents[2]
FIXTURE_ROOT = ROOT / "tests" / "fixtures" / "s6" / "openapi-federation-v1"
SPEC_ROOT = ROOT / "tests" / "specifications" / "s6"
SRS_PATH = ROOT / "docs" / "software" / "software-requirements-specification.md"
DECISION_PATH = (
    ROOT
    / "docs"
    / "software"
    / "decisions"
    / "0009-s6-openapi-federation-contract.md"
)
BUNDLE_PATH = SPEC_ROOT / "contract-bundle.json"
WORKSPACE_SCHEMA_PATH = SPEC_ROOT / "federation-workspace-v1.schema.json"
CLIENT_SCHEMA_PATH = SPEC_ROOT / "federation-client-declaration-v1.schema.json"
REPORT_SCHEMA_PATH = SPEC_ROOT / "federation-report-v1.schema.json"
ERROR_SCHEMA_PATH = SPEC_ROOT / "codenoesis-error-v8.schema.json"
RULES_PATH = SPEC_ROOT / "openapi-federation-rule-catalog-v1.json"
ACCEPTANCE_PATH = SPEC_ROOT / "e2e_fr_fed_001_openapi_federation.json"
WORKSPACE_PATH = FIXTURE_ROOT / "workspace.json"
MANIFEST_PATH = FIXTURE_ROOT / "manifest.json"
REPORT_PATH = FIXTURE_ROOT / "expected-federation-report.json"
PROVIDER_ONLY_WORKSPACE_PATH = FIXTURE_ROOT / "workspace-provider-only.json"
PROVIDER_ONLY_REPORT_PATH = FIXTURE_ROOT / "expected-provider-only-report.json"
UNSUPPORTED_REPORT_PATH = (
    FIXTURE_ROOT / "expected-unsupported-semantics-report.json"
)
S7_MANIFEST_PATH = (
    ROOT
    / "tests"
    / "fixtures"
    / "s7"
    / "implementation-aware-api-v1"
    / "manifest.json"
)
S7_BUNDLE_PATH = ROOT / "tests" / "specifications" / "s7" / "contract-bundle.json"
S7_OPENAPI_PATH = (
    ROOT
    / "tests"
    / "fixtures"
    / "s7"
    / "implementation-aware-api-v1"
    / "provider"
    / "revision-a"
    / "openapi.yaml"
)

S6_REQUIREMENTS = [
    "FR-EXT-004",
    "FR-FED-001",
    "FR-FED-002",
    "FR-CLI-005",
]

EXPECTED_LIMITS = {
    "workspace_manifest_bytes": 8_388_608,
    "repositories": 128,
    "contract_documents": 256,
    "contract_bytes_per_document": 2_097_152,
    "yaml_nesting_depth": 32,
    "local_ref_depth": 16,
    "path_items": 10_000,
    "operations": 10_000,
    "schemas": 20_000,
    "fields_per_operation": 5_000,
    "clients": 10_000,
    "declarations": 100_000,
    "confirmed_links": 1_000_000,
    "candidates": 200_000,
    "rejections": 200_000,
    "evidence_items": 1_000_000,
    "coverage_gaps": 200_000,
    "report_bytes": 67_108_864,
    "memory_bytes": 536_870_912,
    "wall_milliseconds": 60_000,
}

EXPECTED_AUTHORITY_ORDER = [
    "explicit_workspace_identity",
    "package_or_scip_identity",
    "canonical_operation_identity",
    "event_or_schema_identity",
    "heuristic_candidate",
]

EXPECTED_FAILURE_PRECEDENCE = [
    "invocation",
    "workspace_manifest",
    "repository_binding",
    "contract_encoding",
    "yaml_structure",
    "openapi_profile",
    "local_reference",
    "client_declaration",
    "federation_identity",
    "resource_limit",
    "report_validation",
    "internal",
]

EXPECTED_COVERAGE_GAP_REASONS = [
    "heuristic_requires_confirmation",
    "heuristic_no_match",
    "heuristic_ambiguous",
    "unsupported_callbacks",
    "unsupported_webhooks",
    "unsupported_links",
    "unsupported_security_semantics",
    "unsupported_server_variables",
    "unsupported_media_type",
]

EXPECTED_PROVIDER_EVIDENCE_IDENTITY = {
    "domain": "codenoesis.federation-evidence-id/v1",
    "line_numbering": "one_based_inclusive",
    "yaml_selector_kind": "openapi_location_span",
    "yaml_preimage": [
        "repository_identity",
        "revision",
        "path",
        "openapi_location",
        "source_format",
        "start_line_decimal",
        "end_line_decimal",
        "file_sha256",
    ],
    "json_selector_kind": "json_pointer",
    "json_preimage": [
        "repository_identity",
        "revision",
        "path",
        "json_pointer",
        "file_sha256",
    ],
}

EXPECTED_HEURISTIC_MATCHING = {
    "comparison": "exact_unicode_scalar_sequence",
    "service_hint_source": "openapi_info_title",
    "operation_hint_source": "operationId",
    "unique_match": "candidate",
    "no_match": "coverage_gap:heuristic_no_match",
    "multiple_matches": "coverage_gap:heuristic_ambiguous",
    "automatic_confirmation": False,
}

PUBLIC_COMMAND_BOUNDARIES = {
    "workspace_manifest_bytes",
    "repositories",
    "contract_bytes_per_document",
    "yaml_nesting_depth",
    "local_ref_depth",
    "path_items",
    "operations",
    "schemas",
    "fields_per_operation",
    "clients",
    "report_bytes",
}

EXPECTED_RULES = [
    (10, "fed.explicit-operation.confirm/v1", "confirmed"),
    (20, "fed.operation-identity.confirm/v1", "confirmed"),
    (30, "fed.operation-decoy.reject/v1", "rejected"),
    (40, "fed.heuristic-name.candidate/v1", "candidate"),
    (50, "fed.conflict.unresolved/v1", "unresolved"),
]

EXPECTED_SCENARIOS = [
    "e2e_fr_fed_001_openapi_federation",
    "gt_fr_ext_004_yaml_json_semantic_equivalence",
    "gt_fr_fed_001_explicit_clients_confirmed",
    "gt_fr_fed_001_operation_decoy_rejected",
    "pt_fr_fed_002_heuristic_never_auto_confirms",
    "conf_fr_fed_002_conflicting_authority_fails_closed",
    "sec_fr_ext_004_duplicate_yaml_key_rejected",
    "sec_fr_ext_004_yaml_alias_rejected",
    "sec_fr_ext_004_yaml_merge_key_rejected",
    "sec_fr_ext_004_yaml_custom_tag_rejected",
    "sec_fr_ext_004_multiple_yaml_documents_rejected",
    "sec_fr_ext_004_remote_ref_rejected",
    "sec_fr_ext_004_local_ref_cycle_rejected",
    "fz_fr_ext_004_malformed_yaml_seed",
    "conf_fr_ext_004_openapi_version_is_exact",
    "pt_fr_fed_001_every_limit_has_max_and_plus_one",
    "pt_fr_fed_001_authority_and_input_order_are_invariant",
    "pt_fr_fed_001_parallel_schedule_is_invariant",
    "sec_fr_fed_001_standard_s6_has_no_ambient_authority",
    "conf_fr_cli_005_streams_exits_and_no_partial_output",
    "conf_fr_cli_005_s0_s5_regression",
    "red_e2e_fr_fed_001_pre_s6_command_boundary",
]

EXPECTED_VARIANTS = {
    "alias": "variants/alias.yaml",
    "conflicting_authority": "variants/conflicting-client.json",
    "custom_tag": "variants/custom-tag.yaml",
    "duplicate_key": "variants/duplicate-key.yaml",
    "malformed_yaml": "variants/malformed.yaml",
    "merge_key": "variants/merge-key.yaml",
    "multiple_documents": "variants/multiple-documents.yaml",
    "reference_cycle": "variants/ref-cycle.yaml",
    "remote_ref": "variants/remote-ref.yaml",
    "unsupported_openapi": "variants/unsupported-openapi.yaml",
}

EXPECTED_ERROR_GOLDENS = {
    "expected-error-duplicate-key.json": (
        "contract.duplicate_key",
        "contract",
    ),
    "expected-error-identity-conflict.json": (
        "federation.identity_conflict",
        "federation",
    ),
    "expected-error-remote-ref.json": (
        "contract.remote_reference_forbidden",
        "contract",
    ),
}

EXPECTED_CORE_IDENTITIES = {
    "service": (
        "urn:codenoesis:service:blake3:"
        "509813cd6e049acb2c9de79cb9f0a1f385b355473512592a22d5f115ac9243cc"
    ),
    "operation": (
        "urn:codenoesis:operation:blake3:"
        "071cbb8fa33a959879d7d8a2bfbbac31e1fea4850c28fdb73227c605f5974923"
    ),
    "schema": (
        "urn:codenoesis:schema:blake3:"
        "d5ccc3bfeafd3668993d4a6f520231d62b47ca63dbac467b5ea1d23d13032c68"
    ),
    "field_id": (
        "urn:codenoesis:field:blake3:"
        "18ccd66ca65b4c53a46dfb46b7f376985a376e24e6d1dd1d5614dedeca73d88c"
    ),
    "field_nickname": (
        "urn:codenoesis:field:blake3:"
        "a2b6cafe0db73cb016ffd790941c18fbab768923a5b8b4b671eb22459ec66301"
    ),
    "field_display_name": (
        "urn:codenoesis:field:blake3:"
        "a4bef896d066ecb3441a6d9638cea00947075d62c66bad642455840a7ce4f0e7"
    ),
}

EXPECTED_PROVIDER_EVIDENCE = {
    (
        5,
        6,
    ): "urn:codenoesis:evidence:blake3:649dce60ffe264f21ba96bf3f95c0ac5a99c8936377ffc4ed102d1c0cae646e4",
    (
        8,
        17,
    ): "urn:codenoesis:evidence:blake3:36e6771d66d0c86f9dee560713c4dfab20e4930a4d6fa093fb5076539c372dcf",
    (
        18,
        30,
    ): "urn:codenoesis:evidence:blake3:7ad84b96363e0102c20212c0dbff3e8632232460df32744cbd8227a68d0de846",
}

S6_BUNDLE_FILES = {
    "LICENSE",
    "docs/software/decisions/0009-s6-openapi-federation-contract.md",
    "scripts/tests/test_s6_openapi_federation_contract.py",
    "tests/fixtures/s6/openapi-federation-v1/README.md",
    "tests/fixtures/s6/openapi-federation-v1/clients/candidate/federation.json",
    "tests/fixtures/s6/openapi-federation-v1/clients/decoy/federation.json",
    "tests/fixtures/s6/openapi-federation-v1/clients/safe/federation.json",
    "tests/fixtures/s6/openapi-federation-v1/clients/strict/federation.json",
    "tests/fixtures/s6/openapi-federation-v1/expected-error-duplicate-key.json",
    "tests/fixtures/s6/openapi-federation-v1/expected-error-identity-conflict.json",
    "tests/fixtures/s6/openapi-federation-v1/expected-error-remote-ref.json",
    "tests/fixtures/s6/openapi-federation-v1/expected-federation-report.json",
    "tests/fixtures/s6/openapi-federation-v1/manifest.json",
    "tests/fixtures/s6/openapi-federation-v1/provider/openapi.json",
    "tests/fixtures/s6/openapi-federation-v1/provider/openapi.yaml",
    "tests/fixtures/s6/openapi-federation-v1/sentinel/should-not-run.sh",
    "tests/fixtures/s6/openapi-federation-v1/variants/alias.yaml",
    "tests/fixtures/s6/openapi-federation-v1/variants/conflicting-client.json",
    "tests/fixtures/s6/openapi-federation-v1/variants/custom-tag.yaml",
    "tests/fixtures/s6/openapi-federation-v1/variants/duplicate-key.yaml",
    "tests/fixtures/s6/openapi-federation-v1/variants/malformed.yaml",
    "tests/fixtures/s6/openapi-federation-v1/variants/merge-key.yaml",
    "tests/fixtures/s6/openapi-federation-v1/variants/multiple-documents.yaml",
    "tests/fixtures/s6/openapi-federation-v1/variants/ref-cycle.yaml",
    "tests/fixtures/s6/openapi-federation-v1/variants/remote-ref.yaml",
    "tests/fixtures/s6/openapi-federation-v1/variants/unsupported-openapi.yaml",
    "tests/fixtures/s6/openapi-federation-v1/workspace.json",
    "tests/fixtures/s7/implementation-aware-api-v1/manifest.json",
    "tests/specifications/s6/codenoesis-error-v8.schema.json",
    "tests/specifications/s6/e2e_fr_fed_001_openapi_federation.json",
    "tests/specifications/s6/federation-client-declaration-v1.schema.json",
    "tests/specifications/s6/federation-report-v1.schema.json",
    "tests/specifications/s6/federation-workspace-v1.schema.json",
    "tests/specifications/s6/openapi-federation-rule-catalog-v1.json",
    "tests/specifications/s7/contract-bundle.json",
}


def reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise AssertionError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def load_json(path: Path) -> Any:
    return json.loads(
        path.read_text(encoding="utf-8"),
        object_pairs_hook=reject_duplicate_keys,
    )


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def stable_id(kind: str, domain: str, preimage: list[str]) -> str:
    digest = blake3_256(canonical_json([domain, *preimage]))
    return f"urn:codenoesis:{kind}:blake3:{digest}"


def object_schemas(value: Any) -> list[dict[str, Any]]:
    found: list[dict[str, Any]] = []
    if isinstance(value, dict):
        if value.get("type") == "object":
            found.append(value)
        for child in value.values():
            found.extend(object_schemas(child))
    elif isinstance(value, list):
        for child in value:
            found.extend(object_schemas(child))
    return found


def array_schemas(value: Any) -> list[dict[str, Any]]:
    found: list[dict[str, Any]] = []
    if isinstance(value, dict):
        if value.get("type") == "array":
            found.append(value)
        for child in value.values():
            found.extend(array_schemas(child))
    elif isinstance(value, list):
        for child in value:
            found.extend(array_schemas(child))
    return found


def assert_sorted_unique(
    case: unittest.TestCase, values: list[str], label: str
) -> None:
    case.assertEqual(values, sorted(values), f"{label} must be sorted")
    case.assertEqual(len(values), len(set(values)), f"{label} must be unique")


def remove_evidence_references(value: Any) -> None:
    if isinstance(value, dict):
        value.pop("evidence_ids", None)
        for child in value.values():
            remove_evidence_references(child)
    elif isinstance(value, list):
        for child in value:
            remove_evidence_references(child)


def report_semantic_hash(report: dict[str, Any]) -> str:
    projection = copy.deepcopy(report)
    projection.pop("semantic_hash")
    projection.pop("evidence")
    for key in ("contract_path", "contract_sha256", "source_format"):
        projection["provider"].pop(key)
    for client in projection["clients"]:
        client.pop("declaration_path")
    remove_evidence_references(projection)
    payload = (
        b"codenoesis.federation-report.semantic.v1\0"
        + canonical_json(projection)
    )
    return f"blake3:{blake3_256(payload)}"


class S6OpenApiFederationContractTests(unittest.TestCase):
    def test_implementation_contract_is_complete(self) -> None:
        workspace_schema = load_json(WORKSPACE_SCHEMA_PATH)
        report_schema = load_json(REPORT_SCHEMA_PATH)
        rules = load_json(RULES_PATH)
        acceptance = load_json(ACCEPTANCE_PATH)
        decision = DECISION_PATH.read_text(encoding="utf-8")

        self.assertEqual(
            workspace_schema["properties"]["clients"]["minItems"],
            0,
            "Decision 0009 authorizes a provider-only workspace",
        )
        self.assertEqual(
            report_schema["$defs"]["coverage_gap"]["properties"][
                "reason_code"
            ]["enum"],
            EXPECTED_COVERAGE_GAP_REASONS,
        )
        self.assertEqual(
            set(
                report_schema["$defs"]["evidence"]["properties"]["kind"][
                    "enum"
                ]
            ),
            {
                "openapi_yaml_span",
                "openapi_json_pointer",
                "workspace_json_pointer",
            },
        )
        self.assertEqual(
            rules["provider_evidence_identity"],
            EXPECTED_PROVIDER_EVIDENCE_IDENTITY,
        )
        self.assertEqual(
            rules["heuristic_matching"],
            EXPECTED_HEURISTIC_MATCHING,
        )
        self.assertEqual(
            acceptance["provider_evidence_identity"],
            EXPECTED_PROVIDER_EVIDENCE_IDENTITY,
        )
        self.assertEqual(
            acceptance["heuristic_matching"],
            EXPECTED_HEURISTIC_MATCHING,
        )
        boundary_by_limit = {
            item["limit"]: item for item in acceptance["boundary_matrix"]
        }
        for limit in EXPECTED_LIMITS:
            expected_level = (
                "public_command"
                if limit in PUBLIC_COMMAND_BOUNDARIES
                else "component_counter"
            )
            self.assertEqual(
                boundary_by_limit[limit]["observation_level"],
                expected_level,
            )
            self.assertEqual(
                boundary_by_limit[limit]["counter_contract"],
                "inclusive_charge_before_allocation_or_traversal",
            )
        self.assertTrue(PROVIDER_ONLY_WORKSPACE_PATH.is_file())
        self.assertTrue(PROVIDER_ONLY_REPORT_PATH.is_file())
        self.assertTrue(UNSUPPORTED_REPORT_PATH.is_file())
        self.assertIn(
            "exact Unicode scalar-sequence equality",
            decision,
        )
        self.assertIn(
            "component-counter conformance",
            decision,
        )

    def test_ratification_register_and_scope_are_exact(self) -> None:
        srs = SRS_PATH.read_text(encoding="utf-8")
        decision = DECISION_PATH.read_text(encoding="utf-8")
        acceptance = load_json(ACCEPTANCE_PATH)

        heading = "### 2.11 S6 OpenAPI federation ratification register"
        self.assertIn(heading, srs)
        register = srs.split(heading, 1)[1].split(
            "## 3. Product intent and success definition",
            1,
        )[0]
        rows = re.findall(
            r"^\| `(FR-[A-Z]+-\d{3})` \| "
            r"`Proposed` \(pending protected merge\) \|",
            register,
            flags=re.MULTILINE,
        )
        self.assertEqual(rows, S6_REQUIREMENTS)
        for requirement in S6_REQUIREMENTS:
            self.assertEqual(register.count(f"| `{requirement}` |"), 1)
            self.assertIn(f"`{requirement}`", decision)

        self.assertEqual(
            acceptance["issue"],
            "https://github.com/smutti/codenoesis/issues/78",
        )
        self.assertEqual(
            acceptance["approval_pull_request"],
            "https://github.com/smutti/codenoesis/pull/81",
        )
        self.assertEqual(acceptance["correction_budget"], 3)
        self.assertEqual(
            [item["id"] for item in acceptance["requirements"]],
            S6_REQUIREMENTS,
        )
        self.assertTrue(
            all(
                item["current_state"] == "Proposed"
                and item["target_state"] == "Approved"
                for item in acceptance["requirements"]
            )
        )
        self.assertEqual(acceptance["slice"], "S6")
        self.assertEqual(acceptance["risk"]["level"], "high")
        self.assertFalse(acceptance["runtime_implementation_authorized"])
        self.assertIn("Issue | [#78]", decision)
        self.assertIn("PR #81", decision)
        self.assertIn("Scope | Governance only", decision)
        self.assertIn("output-only", decision)
        self.assertIn("There is no `--store`", register)
        self.assertIn(
            "| `FR-CLI-005` | P1 | `0.2` | The local CLI MUST provide "
            "output-only `noesis federate`",
            srs,
        )
        self.assertIn(
            "| `S6` Contract federation | Output-only federation",
            srs,
        )

    def test_schemas_are_strict_closed_and_bounded(self) -> None:
        schemas = {
            WORKSPACE_SCHEMA_PATH: load_json(WORKSPACE_SCHEMA_PATH),
            CLIENT_SCHEMA_PATH: load_json(CLIENT_SCHEMA_PATH),
            REPORT_SCHEMA_PATH: load_json(REPORT_SCHEMA_PATH),
            ERROR_SCHEMA_PATH: load_json(ERROR_SCHEMA_PATH),
        }
        for path, schema in schemas.items():
            self.assertEqual(
                schema["$schema"],
                "https://json-schema.org/draft/2020-12/schema",
            )
            for item in object_schemas(schema):
                self.assertIs(
                    item.get("additionalProperties"),
                    False,
                    f"{path} contains an open object schema",
                )
            for item in array_schemas(schema):
                self.assertIn(
                    "maxItems",
                    item,
                    f"{path} contains an unbounded array schema",
                )

        workspace_schema = schemas[WORKSPACE_SCHEMA_PATH]
        client_schema = schemas[CLIENT_SCHEMA_PATH]
        report_schema = schemas[REPORT_SCHEMA_PATH]
        error_schema = schemas[ERROR_SCHEMA_PATH]
        workspace = load_json(WORKSPACE_PATH)
        report = load_json(REPORT_PATH)

        self.assertEqual(set(workspace), set(workspace_schema["required"]))
        self.assertEqual(set(report), set(report_schema["required"]))
        self.assertEqual(
            workspace_schema["properties"]["analysis_profile"]["const"],
            "standard-local-s6",
        )
        self.assertEqual(
            report_schema["properties"]["contract_capability"]["const"],
            "codenoesis.contract-capability/openapi-3.1-http-json/v1",
        )
        self.assertEqual(
            report_schema["$defs"]["limits"]["required"],
            list(EXPECTED_LIMITS),
        )
        self.assertEqual(
            {
                key: value["const"]
                for key, value in report_schema["$defs"]["limits"][
                    "properties"
                ].items()
            },
            EXPECTED_LIMITS,
        )
        self.assertEqual(
            report_schema["properties"]["operations"]["maxItems"],
            EXPECTED_LIMITS["operations"],
        )
        self.assertEqual(
            report_schema["$defs"]["operation"]["properties"]["fields"][
                "maxItems"
            ],
            EXPECTED_LIMITS["fields_per_operation"],
        )
        self.assertEqual(
            client_schema["properties"]["schema_version"]["const"],
            "codenoesis.federation-client-declaration/v1",
        )
        self.assertEqual(
            error_schema["properties"]["schema_version"]["const"],
            "codenoesis.error/v8",
        )
        self.assertEqual(error_schema["properties"]["retryable"]["const"], False)
        self.assertIn(
            "contract.duplicate_key",
            error_schema["properties"]["code"]["enum"],
        )
        self.assertIn(
            "federation.identity_conflict",
            error_schema["properties"]["code"]["enum"],
        )

        for path in sorted(FIXTURE_ROOT.rglob("*.json")) + sorted(
            SPEC_ROOT.glob("*.json")
        ):
            if path != BUNDLE_PATH:
                load_json(path)

    def test_rule_catalog_and_oracle_are_exact(self) -> None:
        rules = load_json(RULES_PATH)
        acceptance = load_json(ACCEPTANCE_PATH)

        self.assertEqual(
            rules["catalog_version"],
            "codenoesis.federation-rules/http-json/v1",
        )
        self.assertEqual(rules["analysis_profile"], "standard-local-s6")
        self.assertEqual(rules["authority_order"], EXPECTED_AUTHORITY_ORDER)
        self.assertEqual(
            rules["states"],
            ["candidate", "confirmed", "rejected", "unresolved"],
        )
        self.assertEqual(
            [
                (item["precedence"], item["id"], item["outcome"])
                for item in rules["rules"]
            ],
            EXPECTED_RULES,
        )
        self.assertTrue(
            all(item["on_conflict"] == "unresolved" for item in rules["rules"])
        )
        self.assertEqual(
            rules["failure_precedence"],
            EXPECTED_FAILURE_PRECEDENCE,
        )
        self.assertEqual(rules["limits"], EXPECTED_LIMITS)
        capabilities = {
            item["authority"]: (item["status"], item["confirmation"])
            for item in rules["capabilities"]
        }
        self.assertEqual(
            capabilities["heuristic_candidate"],
            ("supported", "never_automatic"),
        )
        self.assertEqual(
            capabilities["package_or_scip_identity"],
            ("not_exposed", "coverage_gap"),
        )

        self.assertEqual(
            acceptance["public_command"]["argv"],
            [
                "noesis",
                "federate",
                "--workspace-manifest",
                "<path>",
                "--profile",
                "standard-local-s6",
                "--format",
                "json",
            ],
        )
        self.assertNotIn("--store", acceptance["public_command"]["argv"])
        self.assertEqual(
            acceptance["public_command"]["persistent_store"],
            "none",
        )
        self.assertEqual(acceptance["limits"], EXPECTED_LIMITS)
        self.assertEqual(
            [item["id"] for item in acceptance["scenarios"]],
            EXPECTED_SCENARIOS,
        )
        self.assertEqual(
            [item["order"] for item in acceptance["scenarios"]],
            list(range(1, len(EXPECTED_SCENARIOS) + 1)),
        )

        boundary_by_limit = {
            item["limit"]: item for item in acceptance["boundary_matrix"]
        }
        self.assertEqual(set(boundary_by_limit), set(EXPECTED_LIMITS))
        self.assertEqual(
            len(boundary_by_limit),
            len(acceptance["boundary_matrix"]),
        )
        for name, maximum in EXPECTED_LIMITS.items():
            boundary = boundary_by_limit[name]
            self.assertEqual(boundary["maximum"], maximum)
            self.assertEqual(boundary["maximum_plus_one"], maximum + 1)
            self.assertIn(
                boundary["failure_code"],
                {"contract.limit_exceeded", "federation.limit_exceeded"},
            )

        subset = acceptance["supported_openapi_subset"]
        self.assertEqual(subset["versions"], ["3.1.0"])
        self.assertEqual(subset["document_formats"], ["json", "yaml"])
        self.assertEqual(
            subset["references"],
            "local JSON Pointer under #/components/schemas only",
        )
        for forbidden in (
            "duplicate_key",
            "external_reference",
            "multiple_documents",
            "reference_cycle",
            "yaml_alias",
            "yaml_anchor",
            "yaml_custom_tag",
            "yaml_merge_key",
        ):
            self.assertIn(forbidden, subset["unsupported_as_failure"])

        candidate = acceptance["yaml_parser_candidate"]
        self.assertEqual(candidate["crate"], "yaml-rust2")
        self.assertEqual(candidate["version"], "0.11.0")
        self.assertEqual(candidate["requirement"], "=0.11.0")
        self.assertFalse(candidate["default_features"])
        self.assertEqual(
            candidate["checksum"],
            "631a50d867fafb7093e709d75aaee9e0e0d5deb934021fcea25ac2fe09edc51e",
        )
        self.assertEqual(candidate["direct_unsafe_in_src"], 0)
        self.assertEqual(candidate["osv_vulnerabilities_observed_at_review"], 0)
        manifests = "\n".join(
            path.read_text(encoding="utf-8")
            for path in sorted(ROOT.glob("**/Cargo.toml"))
        )
        self.assertNotIn("yaml-rust2", manifests)

    def test_fixture_manifest_binds_every_input_and_hostile_variant(self) -> None:
        manifest = load_json(MANIFEST_PATH)
        workspace = load_json(WORKSPACE_PATH)

        self.assertEqual(
            manifest["fixture_identity"],
            "urn:codenoesis:fixture:s6-openapi-federation-v1",
        )
        self.assertEqual(manifest["analysis_profile"], "standard-local-s6")
        self.assertEqual(
            sha256(FIXTURE_ROOT / manifest["workspace"]["path"]),
            manifest["workspace"]["sha256"],
        )
        self.assertEqual(
            workspace["workspace_identity"],
            manifest["fixture_identity"],
        )
        self.assertEqual(
            workspace["contract_capability"],
            manifest["contract_capability"],
        )

        provider = manifest["provider"]
        for representation in ("yaml", "json"):
            item = provider[representation]
            self.assertEqual(
                sha256(FIXTURE_ROOT / item["path"]),
                item["sha256"],
            )
        self.assertEqual(
            (FIXTURE_ROOT / provider["yaml"]["path"]).read_bytes(),
            S7_OPENAPI_PATH.read_bytes(),
        )
        provider_json = load_json(FIXTURE_ROOT / provider["json"]["path"])
        self.assertEqual(provider_json["openapi"], "3.1.0")
        self.assertEqual(
            provider_json["servers"],
            [{"url": "https://api.example.invalid"}],
        )
        self.assertEqual(
            provider_json["paths"]["/users/{id}"]["get"]["operationId"],
            "getUser",
        )
        self.assertEqual(
            provider_json["components"]["schemas"]["User"]["required"],
            ["id"],
        )
        self.assertEqual(
            workspace["provider"]["contract_sha256"],
            provider["yaml"]["sha256"],
        )

        manifest_clients = {
            item["role"]: item for item in manifest["clients"]
        }
        workspace_clients = {
            item["role"]: item for item in workspace["clients"]
        }
        self.assertEqual(
            set(manifest_clients),
            {"candidate", "decoy", "safe", "strict"},
        )
        self.assertEqual(set(workspace_clients), set(manifest_clients))
        for role, item in manifest_clients.items():
            self.assertEqual(sha256(FIXTURE_ROOT / item["path"]), item["sha256"])
            declaration = load_json(FIXTURE_ROOT / item["path"])
            workspace_item = workspace_clients[role]
            self.assertEqual(
                workspace_item["declaration_sha256"],
                item["sha256"],
            )
            self.assertEqual(declaration["role"], role)
            self.assertEqual(
                item["expected_state"],
                {
                    "candidate": "candidate",
                    "decoy": "rejected",
                    "safe": "confirmed",
                    "strict": "confirmed",
                }[role],
            )

        variants = {
            item["scenario"]: item for item in manifest["variants"]
        }
        self.assertEqual(
            {key: item["path"] for key, item in variants.items()},
            EXPECTED_VARIANTS,
        )
        for item in variants.values():
            self.assertEqual(sha256(FIXTURE_ROOT / item["path"]), item["sha256"])

        for item in manifest["expected"]["errors"]:
            self.assertEqual(sha256(FIXTURE_ROOT / item["path"]), item["sha256"])
        self.assertEqual(
            sha256(FIXTURE_ROOT / manifest["expected"]["report"]["path"]),
            manifest["expected"]["report"]["sha256"],
        )
        sentinel = manifest["sentinels"]
        self.assertEqual(sha256(FIXTURE_ROOT / sentinel["path"]), sentinel["sha256"])
        self.assertTrue(
            all(
                value is False
                for key, value in sentinel.items()
                if key not in {"path", "sha256"}
            )
        )
        self.assertEqual(
            (FIXTURE_ROOT / sentinel["path"]).stat().st_mode & 0o111,
            0,
        )

    def test_reviewed_report_identities_references_and_hash_are_exact(
        self,
    ) -> None:
        report = load_json(REPORT_PATH)
        manifest = load_json(MANIFEST_PATH)
        workspace = load_json(WORKSPACE_PATH)
        report_schema = load_json(REPORT_SCHEMA_PATH)

        self.assertEqual(set(report), set(report_schema["required"]))
        self.assertEqual(report["analysis_profile"], "standard-local-s6")
        self.assertEqual(report["limits"], EXPECTED_LIMITS)
        self.assertEqual(
            report["workspace_identity"],
            workspace["workspace_identity"],
        )
        provider = report["provider"]
        service_id = stable_id(
            "service",
            "codenoesis.service-id/http/v1",
            [provider["service_authority"]],
        )
        self.assertEqual(service_id, EXPECTED_CORE_IDENTITIES["service"])
        self.assertEqual(provider["service_id"], service_id)

        self.assertEqual(len(report["operations"]), 1)
        operation = report["operations"][0]
        operation_id = stable_id(
            "operation",
            "codenoesis.operation-id/http/v1",
            [
                service_id,
                operation["method"],
                operation["path_template"],
                operation["explicit_operation_id"],
            ],
        )
        self.assertEqual(operation_id, EXPECTED_CORE_IDENTITIES["operation"])
        self.assertEqual(operation["operation_id"], operation_id)
        schema_id = stable_id(
            "schema",
            "codenoesis.schema-id/http-json/v1",
            [
                operation_id,
                "response",
                operation["response_status"],
                "#/components/schemas/User",
            ],
        )
        self.assertEqual(schema_id, EXPECTED_CORE_IDENTITIES["schema"])
        self.assertEqual(operation["schema_id"], schema_id)

        expected_fields = {
            "/id": EXPECTED_CORE_IDENTITIES["field_id"],
            "/nickname": EXPECTED_CORE_IDENTITIES["field_nickname"],
            "/displayName": EXPECTED_CORE_IDENTITIES["field_display_name"],
        }
        for field in operation["fields"]:
            self.assertEqual(
                field["field_id"],
                stable_id(
                    "field",
                    "codenoesis.field-id/http-json/v1",
                    [
                        operation_id,
                        "response",
                        operation["response_status"],
                        field["json_pointer"],
                    ],
                ),
            )
            self.assertEqual(
                field["field_id"],
                expected_fields[field["json_pointer"]],
            )

        clients_by_id = {item["client_id"]: item for item in report["clients"]}
        clients_by_role = {item["role"]: item for item in report["clients"]}
        for role, client in clients_by_role.items():
            declaration = load_json(FIXTURE_ROOT / client["declaration_path"])
            client_id = stable_id(
                "client",
                "codenoesis.client-id/v1",
                [declaration["repository_identity"]],
            )
            self.assertEqual(client["client_id"], client_id)
            self.assertEqual(
                client["call_site_id"],
                stable_id(
                    "call-site",
                    "codenoesis.call-site-id/v1",
                    [
                        client_id,
                        declaration["revision"],
                        declaration["source_path"],
                        declaration["symbol_identity"],
                    ],
                ),
            )
            if declaration["binding"]["kind"] == "explicit_operation_identity":
                binding = declaration["binding"]
                expected_operation = stable_id(
                    "operation",
                    "codenoesis.operation-id/http/v1",
                    [
                        service_id,
                        binding["method"],
                        binding["path_template"],
                        binding["operation_id"],
                    ],
                )
                self.assertEqual(
                    client["operation_candidate_id"],
                    expected_operation,
                )
            else:
                self.assertEqual(role, "candidate")
                self.assertIsNone(client["operation_candidate_id"])

        self.assertEqual(len(report["confirmed_links"]), 2)
        for link in report["confirmed_links"]:
            client = clients_by_id[link["client_id"]]
            self.assertEqual(
                link["link_id"],
                stable_id(
                    "federation-link",
                    "codenoesis.federation-link-id/v1",
                    [
                        operation_id,
                        link["client_id"],
                        link["call_site_id"],
                        client["binding_kind"],
                    ],
                ),
            )
            self.assertEqual(link["state"], "confirmed")

        self.assertEqual(len(report["candidates"]), 1)
        candidate = report["candidates"][0]
        candidate_client = clients_by_id[candidate["client_id"]]
        self.assertEqual(
            candidate["candidate_id"],
            stable_id(
                "federation-candidate",
                "codenoesis.federation-candidate-id/v1",
                [
                    operation_id,
                    candidate["client_id"],
                    candidate["call_site_id"],
                    candidate_client["binding_kind"],
                    candidate["service_hint"],
                    candidate["operation_hint"],
                ],
            ),
        )
        self.assertEqual(candidate["state"], "candidate")

        self.assertEqual(len(report["rejections"]), 1)
        rejection = report["rejections"][0]
        self.assertEqual(
            rejection["rejection_id"],
            stable_id(
                "federation-rejection",
                "codenoesis.federation-rejection-id/v1",
                [
                    rejection["operation_candidate_id"],
                    rejection["client_id"],
                    rejection["call_site_id"],
                    rejection["reason_code"],
                ],
            ),
        )
        self.assertEqual(rejection["state"], "rejected")

        self.assertEqual(len(report["coverage_gaps"]), 1)
        gap = report["coverage_gaps"][0]
        self.assertEqual(gap["subject_id"], candidate["candidate_id"])
        self.assertEqual(candidate["coverage_gap_id"], gap["coverage_gap_id"])
        self.assertEqual(
            gap["coverage_gap_id"],
            stable_id(
                "coverage-gap",
                "codenoesis.federation-gap-id/v1",
                [gap["subject_id"], gap["reason_code"], gap["evidence_ids"][0]],
            ),
        )

        evidence_by_id = {
            item["evidence_id"]: item for item in report["evidence"]
        }
        referenced_evidence: set[str] = set()
        for item in report["evidence"]:
            path = FIXTURE_ROOT / item["path"]
            self.assertEqual(sha256(path), item["file_sha256"])
            selector = item["selector"]
            if selector["kind"] == "line_span":
                lines = path.read_text(encoding="utf-8").splitlines()
                self.assertLessEqual(selector["start_line"], selector["end_line"])
                self.assertLessEqual(selector["end_line"], len(lines))
                self.assertEqual(
                    item["evidence_id"],
                    EXPECTED_PROVIDER_EVIDENCE[
                        (selector["start_line"], selector["end_line"])
                    ],
                )
            else:
                self.assertEqual(selector["pointer"], "/binding")
                self.assertEqual(
                    item["evidence_id"],
                    stable_id(
                        "evidence",
                        "codenoesis.federation-evidence-id/v1",
                        [
                            item["repository_identity"],
                            item["revision"],
                            item["path"],
                            selector["pointer"],
                            item["file_sha256"],
                        ],
                    ),
                )

        def inspect_references(value: Any) -> None:
            if isinstance(value, dict):
                if "evidence_ids" in value:
                    identifiers = value["evidence_ids"]
                    assert_sorted_unique(self, identifiers, "evidence references")
                    self.assertTrue(set(identifiers).issubset(evidence_by_id))
                    referenced_evidence.update(identifiers)
                for child in value.values():
                    inspect_references(child)
            elif isinstance(value, list):
                for child in value:
                    inspect_references(child)

        semantic_records = {
            key: value
            for key, value in report.items()
            if key != "evidence"
        }
        inspect_references(semantic_records)
        self.assertEqual(referenced_evidence, set(evidence_by_id))

        sort_keys = {
            "operations": "operation_id",
            "clients": "client_id",
            "confirmed_links": "link_id",
            "candidates": "candidate_id",
            "rejections": "rejection_id",
            "evidence": "evidence_id",
            "coverage_gaps": "coverage_gap_id",
        }
        for collection, identity_key in sort_keys.items():
            assert_sorted_unique(
                self,
                [item[identity_key] for item in report[collection]],
                collection,
            )
        assert_sorted_unique(
            self,
            [field["field_id"] for field in operation["fields"]],
            "fields",
        )

        self.assertEqual(
            report["semantic_hash"],
            "blake3:f747384b61804372d75ff2c6e77e2a21664149f7d95c118c036e1086c6a7d6db",
        )
        self.assertEqual(report["semantic_hash"], report_semantic_hash(report))
        self.assertEqual(
            sha256(REPORT_PATH),
            manifest["expected"]["report"]["sha256"],
        )

        for filename, (code, stage) in EXPECTED_ERROR_GOLDENS.items():
            error = load_json(FIXTURE_ROOT / filename)
            self.assertEqual(
                set(error),
                set(load_json(ERROR_SCHEMA_PATH)["required"]),
            )
            self.assertEqual(error["schema_version"], "codenoesis.error/v8")
            self.assertEqual((error["code"], error["stage"]), (code, stage))
            self.assertFalse(error["retryable"])

    def test_s7_identity_alignment_and_acceptance_red_are_bound(self) -> None:
        manifest = load_json(MANIFEST_PATH)
        report = load_json(REPORT_PATH)
        s7_manifest = load_json(S7_MANIFEST_PATH)
        acceptance = load_json(ACCEPTANCE_PATH)
        downstream = manifest["downstream_s7_conformance"]

        self.assertEqual(sha256(S7_MANIFEST_PATH), downstream["manifest_sha256"])
        self.assertEqual(sha256(S7_BUNDLE_PATH), downstream["bundle_file_sha256"])
        self.assertEqual(
            downstream["manifest_sha256"],
            "958948f2eb078ef53e1aebac5f9e7543411cb3ba7423e1ef2dc6c0e7ef2f69c0",
        )
        self.assertEqual(
            downstream["bundle_file_sha256"],
            "8185cbdc0eb33dc96cb5c932a1343460b7d1794b953a64190ff4fcc6783b0208",
        )
        self.assertEqual(
            report["provider"]["service_id"],
            s7_manifest["identities"]["service"],
        )
        self.assertEqual(
            report["operations"][0]["operation_id"],
            s7_manifest["identities"]["operation"],
        )
        fields = {
            field["json_pointer"].removeprefix("/"): field["field_id"]
            for field in report["operations"][0]["fields"]
        }
        self.assertEqual(fields["nickname"], s7_manifest["identities"]["fields"]["nickname"])
        self.assertEqual(
            fields["displayName"],
            s7_manifest["identities"]["fields"]["displayName"],
        )
        s6_clients = {
            item["role"]: item
            for item in report["clients"]
            if item["role"] in {"decoy", "safe", "strict"}
        }
        s7_clients = {item["role"]: item for item in s7_manifest["clients"]}
        self.assertEqual(set(s6_clients), set(s7_clients))
        for role, client in s6_clients.items():
            self.assertEqual(
                client["client_id"],
                s7_clients[role]["client_identity"],
            )
            self.assertEqual(
                client["call_site_id"],
                s7_clients[role]["call_site_id"],
            )
            self.assertEqual(
                client["operation_candidate_id"],
                s7_clients[role]["operation_candidate_id"],
            )

        red = acceptance["expected_red"]
        self.assertEqual(
            red["command"],
            "cargo test --locked -p noesis "
            "--test e2e_fr_fed_001_openapi_federation -- "
            "--exact e2e_fr_fed_001_openapi_federation",
        )
        self.assertEqual(red["subject_exit"], 2)
        self.assertEqual(red["stdout_bytes"], 0)
        self.assertEqual(red["stderr_schema"], "codenoesis.error/v2")
        self.assertEqual(red["stderr_code"], "input.invalid_revision")
        self.assertEqual(red["stderr_bytes"], 149)
        self.assertEqual(
            red["stderr_sha256"],
            "6441e0037f864d2fae4a60e6355e4a85b26b00d5e4e24c59ffeb5fe9c6f3859f",
        )
        self.assertFalse(red["store_or_artifact_created"])
        self.assertEqual(
            acceptance["allowed_paths"],
            [
                "docs/software/software-requirements-specification.md",
                "docs/software/decisions/0009-s6-openapi-federation-contract.md",
                "tests/specifications/s6",
                "tests/fixtures/s6/openapi-federation-v1",
                "scripts/tests/test_s6_openapi_federation_contract.py",
            ],
        )
        self.assertIn(
            "automatic heuristic confirmation",
            acceptance["forbidden"],
        )
        self.assertIn(
            "partial stdout or persistent publication",
            acceptance["forbidden"],
        )
        self.assertEqual(acceptance["determinism"]["replays"], 50)
        self.assertEqual(
            acceptance["determinism"]["parallel_repetitions"],
            10,
        )
        self.assertEqual(acceptance["security"]["writes"], "none")
        self.assertEqual(acceptance["security"]["target_processes"], 0)
        self.assertEqual(acceptance["security"]["network_channels"], 0)
        self.assertEqual(acceptance["security"]["first_party_unsafe"], 0)
        self.assertFalse(
            acceptance["compatibility"]["persistent_store_change"]
        )
        self.assertFalse(acceptance["compatibility"]["ontology_change"])

    def test_contract_bundle_binds_every_s6_artifact(self) -> None:
        bundle = load_json(BUNDLE_PATH)
        self.assertEqual(
            set(bundle),
            {"schema_version", "files", "bundle_sha256"},
        )
        self.assertEqual(
            bundle["schema_version"],
            "codenoesis.contract-bundle/v1",
        )
        files = bundle["files"]
        paths = [item["path"] for item in files]
        assert_sorted_unique(self, paths, "bundle paths")
        self.assertEqual(set(paths), S6_BUNDLE_FILES)
        for item in files:
            self.assertEqual(set(item), {"path", "sha256"})
            path = Path(item["path"])
            self.assertFalse(path.is_absolute())
            self.assertNotIn("..", path.parts)
            self.assertRegex(item["sha256"], r"^[0-9a-f]{64}$")
            self.assertEqual(sha256(ROOT / path), item["sha256"])

        payload = {
            "schema_version": bundle["schema_version"],
            "files": files,
        }
        bundle_sha256 = hashlib.sha256(canonical_json(payload)).hexdigest()
        self.assertEqual(bundle["bundle_sha256"], bundle_sha256)
        srs = SRS_PATH.read_text(encoding="utf-8")
        match = re.search(
            r"S6 bounded OpenAPI federation contract bundle:\s+"
            r"`sha256:([0-9a-f]{64})`",
            srs,
        )
        self.assertIsNotNone(match, "SRS must bind the complete S6 bundle")
        self.assertEqual(match.group(1), bundle_sha256)  # type: ignore[union-attr]


if __name__ == "__main__":
    unittest.main()
