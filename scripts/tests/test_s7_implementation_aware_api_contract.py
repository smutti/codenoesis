from __future__ import annotations

import hashlib
import re
import unittest
from pathlib import Path
from typing import Any

from test_s1_contract import blake3_256, canonical_json, load_json


ROOT = Path(__file__).resolve().parents[2]
FIXTURE_ROOT = (
    ROOT / "tests" / "fixtures" / "s7" / "implementation-aware-api-v1"
)
SPEC_ROOT = ROOT / "tests" / "specifications" / "s7"
REPORT_PATH = FIXTURE_ROOT / "expected-semantic-compatibility-report.json"
MANIFEST_PATH = FIXTURE_ROOT / "manifest.json"
SCHEMA_PATH = SPEC_ROOT / "semantic-compatibility-report-v1.schema.json"
RULES_PATH = SPEC_ROOT / "compatibility-rule-catalog-v1.json"
ACCEPTANCE_PATH = SPEC_ROOT / "e2e_fr_imp_004_implementation_aware_api_diff.json"
BUNDLE_PATH = SPEC_ROOT / "contract-bundle.json"
SRS_PATH = ROOT / "docs" / "software" / "software-requirements-specification.md"
ROADMAP_PATH = ROOT / "docs" / "software" / "roadmap.md"
DECISION_PATH = (
    ROOT
    / "docs"
    / "software"
    / "decisions"
    / "0007-s7-implementation-aware-api-compatibility-contract.md"
)

S7_REQUIREMENTS = {"DR-SEM-001", "FR-IMP-004", "FR-IMP-005"}

S7_BUNDLE_FILES = {
    "LICENSE",
    "docs/software/decisions/0007-s7-implementation-aware-api-compatibility-contract.md",
    "scripts/tests/test_s7_implementation_aware_api_contract.py",
    "tests/fixtures/s7/implementation-aware-api-v1/README.md",
    "tests/fixtures/s7/implementation-aware-api-v1/clients/decoy/src/commonMain/kotlin/dev/codenoesis/fixture/DecoyAccountClient.kt",
    "tests/fixtures/s7/implementation-aware-api-v1/clients/safe/src/commonMain/kotlin/dev/codenoesis/fixture/SafeUsersClient.kt",
    "tests/fixtures/s7/implementation-aware-api-v1/clients/strict/src/commonMain/kotlin/dev/codenoesis/fixture/StrictUsersClient.kt",
    "tests/fixtures/s7/implementation-aware-api-v1/expected-semantic-compatibility-report.json",
    "tests/fixtures/s7/implementation-aware-api-v1/manifest.json",
    "tests/fixtures/s7/implementation-aware-api-v1/provider/revision-a/openapi.yaml",
    "tests/fixtures/s7/implementation-aware-api-v1/provider/revision-a/src/user_response.rs",
    "tests/fixtures/s7/implementation-aware-api-v1/provider/revision-b/openapi.yaml",
    "tests/fixtures/s7/implementation-aware-api-v1/provider/revision-b/src/user_response.rs",
    "tests/specifications/s7/compatibility-rule-catalog-v1.json",
    "tests/specifications/s7/e2e_fr_imp_004_implementation_aware_api_diff.json",
    "tests/specifications/s7/semantic-compatibility-report-v1.schema.json",
}

EXPECTED_RULE_IDS = {
    "cmp.error.code.changed/v1",
    "cmp.http.response-status.removed/v1",
    "cmp.request.presence.optional-to-required/v1",
    "cmp.request.validation.tightened/v1",
    "cmp.response.nullability.non-null-to-nullable/v1",
    "cmp.response.presence.client-stricter-than-contract/v1",
    "cmp.response.presence.safe-absence-handling/v1",
    "cmp.response.presence.undocumented-guarantee-removed/v1",
    "cmp.response.value-set.expanded/v1",
    "cmp.unresolved.insufficient-evidence/v1",
}

EXPECTED_LIMITS = {
    "operations": 10000,
    "fields_per_operation": 5000,
    "linked_clients": 10000,
    "call_sites": 1000000,
    "semantic_diffs": 200000,
    "evidence_items": 1000000,
    "coverage_gaps": 200000,
    "report_bytes": 67108864,
}


def stable_id(kind: str, domain: str, preimage: list[str]) -> str:
    digest = blake3_256(canonical_json([domain, *preimage]))
    return f"urn:codenoesis:{kind}:blake3:{digest}"


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def assert_sorted_unique(
    case: unittest.TestCase, values: list[str], label: str
) -> None:
    case.assertEqual(values, sorted(values), f"{label} must be sorted")
    case.assertEqual(len(values), len(set(values)), f"{label} must be unique")


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


class S7ImplementationAwareApiContractTests(unittest.TestCase):
    def test_ratification_register_and_scope_are_exact(self) -> None:
        srs = SRS_PATH.read_text(encoding="utf-8")
        decision = DECISION_PATH.read_text(encoding="utf-8")
        roadmap = ROADMAP_PATH.read_text(encoding="utf-8")
        acceptance = load_json(ACCEPTANCE_PATH)

        self.assertIn(
            "### 2.9 S7 implementation-aware compatibility ratification register",
            srs,
        )
        self.assertIn("implementation-aware-http-json/v1", srs)
        self.assertIn("Decision 0007 resolves only", srs)
        self.assertIn("Issue | [#62]", decision)
        self.assertIn("Governance only", decision)
        self.assertIn("Production code", decision)
        self.assertIn("## Implementation-aware API compatibility lane", roadmap)
        for planning_id in ("C0", "C1", "C2", "C3", "C4", "C5"):
            self.assertIn(f"`{planning_id}`", roadmap)

        requirements = {
            item["id"]: (item["current_state"], item["target_state"])
            for item in acceptance["requirements"]
        }
        self.assertEqual(
            requirements,
            {
                requirement: ("Proposed", "Approved")
                for requirement in S7_REQUIREMENTS
            },
        )
        self.assertEqual(acceptance["slice"], "S7")
        self.assertEqual(acceptance["risk"]["level"], "high")
        self.assertFalse(acceptance["runtime_implementation_authorized"])
        for requirement in S7_REQUIREMENTS:
            self.assertRegex(
                srs,
                rf"\| `{re.escape(requirement)}` \| "
                r"`Proposed` \(pending protected merge\) \| `Approved` ",
            )
            self.assertGreaterEqual(srs.count(f"`{requirement}`"), 2)
            self.assertIn(f"`{requirement}`", decision)

        self.assertEqual(
            srs.count("| `DR-CMP-001` | P1 | `1.0` |"),
            1,
            "existing schema-compatibility requirement must remain distinct",
        )
        self.assertIn(
            "does not silently approve or redefine the broader",
            srs,
        )
        self.assertIn(
            "`FR-IMP-001`, `FR-IMP-002`, or `FR-IMP-003`",
            srs,
        )

    def test_report_schema_is_closed_and_bounded(self) -> None:
        schema = load_json(SCHEMA_PATH)
        report = load_json(REPORT_PATH)

        self.assertEqual(
            schema["$schema"], "https://json-schema.org/draft/2020-12/schema"
        )
        self.assertEqual(
            schema["$id"],
            "https://codenoesis.dev/schemas/"
            "semantic-compatibility-report-v1.schema.json",
        )
        self.assertEqual(schema["type"], "object")
        self.assertFalse(schema["additionalProperties"])
        self.assertEqual(set(schema["required"]), set(report))
        for item in object_schemas(schema):
            self.assertIn("additionalProperties", item)
            self.assertFalse(item["additionalProperties"])

        properties = schema["properties"]
        self.assertEqual(
            properties["schema_version"]["const"],
            "codenoesis.semantic-compatibility-report/v1",
        )
        self.assertEqual(
            properties["analysis_profile"]["const"],
            "implementation-aware-http-json/v1",
        )
        self.assertEqual(
            properties["rule_catalog_version"]["const"],
            "codenoesis.compatibility-rules/http-json/v1",
        )
        self.assertEqual(
            properties["semantic_diffs"]["maxItems"],
            EXPECTED_LIMITS["semantic_diffs"],
        )
        self.assertEqual(
            properties["client_assessments"]["maxItems"],
            EXPECTED_LIMITS["linked_clients"],
        )
        self.assertEqual(
            properties["evidence"]["maxItems"],
            EXPECTED_LIMITS["evidence_items"],
        )
        self.assertEqual(
            properties["coverage_gaps"]["maxItems"],
            EXPECTED_LIMITS["coverage_gaps"],
        )

        definitions = schema["$defs"]
        self.assertIn("claim_state", definitions["declared_view_delta"]["required"])
        self.assertIn(
            "claim_state", definitions["implementation_view_delta"]["required"]
        )
        self.assertIn(
            "assumption_claim_state",
            definitions["client_assessment"]["required"],
        )
        self.assertEqual(
            definitions["semantic_diff"]["properties"]["claim_state"]["const"],
            "derived_fact",
        )
        self.assertEqual(
            definitions["dimension"]["enum"],
            [
                "presence",
                "nullability",
                "default",
                "validation",
                "value_set",
                "http_status",
                "error_code",
            ],
        )

    def test_rule_catalog_fixes_direction_and_epistemic_precedence(self) -> None:
        catalog = load_json(RULES_PATH)
        self.assertEqual(
            set(catalog),
            {
                "schema_version",
                "catalog_version",
                "analysis_profile",
                "fact_views",
                "dimensions",
                "classifications",
                "rules",
                "precedence",
                "semantic_constraints",
            },
        )
        self.assertEqual(
            catalog["schema_version"],
            "codenoesis.compatibility-rule-catalog/v1",
        )
        self.assertEqual(
            catalog["catalog_version"],
            "codenoesis.compatibility-rules/http-json/v1",
        )
        self.assertEqual(
            catalog["fact_views"],
            [
                "declared_contract",
                "provider_implementation",
                "client_assumption",
                "test_observation",
                "runtime_observation",
            ],
        )
        self.assertEqual(
            set(catalog["dimensions"]),
            {
                "presence",
                "nullability",
                "default",
                "validation",
                "value_set",
                "http_status",
                "error_code",
            },
        )
        rules = catalog["rules"]
        rule_ids = [item["id"] for item in rules]
        assert_sorted_unique(self, rule_ids, "rule ids")
        self.assertEqual(set(rule_ids), EXPECTED_RULE_IDS)
        for rule in rules:
            self.assertEqual(
                set(rule),
                {
                    "id",
                    "priority",
                    "direction",
                    "dimension",
                    "predicate",
                    "classification",
                },
            )
            self.assertGreater(rule["priority"], 0)
            self.assertIn(
                rule["classification"],
                catalog["classifications"],
            )
        unresolved = next(
            item
            for item in rules
            if item["id"] == "cmp.unresolved.insufficient-evidence/v1"
        )
        self.assertEqual(unresolved["priority"], max(item["priority"] for item in rules))
        self.assertEqual(
            catalog["precedence"]["breaking_fact_states"],
            ["deterministic_fact", "confirmed"],
        )
        self.assertEqual(
            catalog["precedence"]["candidate_client_impact"], "unresolved"
        )
        self.assertEqual(
            catalog["semantic_constraints"],
            {
                "presence_is_distinct_from_nullability": True,
                "presence_is_distinct_from_default": True,
                "declared_default_proves_runtime_application": False,
                "source_type_alone_proves_client_requiredness": False,
                "absence_of_trace_proves_absence_of_behavior": False,
                "unchanged_contract_suppresses_implementation_diff": False,
                "model_output_may_create_breaking_fact": False,
            },
        )

    def test_fixture_manifest_binds_portable_repository_paths(self) -> None:
        manifest = load_json(MANIFEST_PATH)
        self.assertEqual(
            set(manifest),
            {
                "schema_version",
                "fixture_identity",
                "analysis_profile",
                "identities",
                "provider",
                "clients",
                "expected_report",
                "sentinels",
            },
        )
        self.assertEqual(
            manifest["schema_version"], "codenoesis.s7-fixture-manifest/v1"
        )
        self.assertTrue(manifest["provider"]["contract_bytes_identical"])

        roots: dict[tuple[str, str], Path] = {}
        provider_identity = manifest["provider"]["repository_identity"]
        provider_revisions = manifest["provider"]["revisions"]
        self.assertEqual(
            [item["name"] for item in provider_revisions],
            ["fixture-provider-a", "fixture-provider-b"],
        )
        for revision in provider_revisions:
            root = Path(revision["root"])
            self.assertFalse(root.is_absolute())
            self.assertNotIn("..", root.parts)
            roots[(provider_identity, revision["name"])] = root
            paths = [item["path"] for item in revision["files"]]
            assert_sorted_unique(self, paths, f"{revision['name']} files")
            for item in revision["files"]:
                path = Path(item["path"])
                self.assertFalse(path.is_absolute())
                self.assertNotIn("..", path.parts)
                self.assertEqual(
                    sha256(FIXTURE_ROOT / root / path),
                    item["sha256"],
                )

        contract_a = (
            FIXTURE_ROOT / provider_revisions[0]["root"] / "openapi.yaml"
        )
        contract_b = (
            FIXTURE_ROOT / provider_revisions[1]["root"] / "openapi.yaml"
        )
        self.assertEqual(contract_a.read_bytes(), contract_b.read_bytes())

        roles = [item["role"] for item in manifest["clients"]]
        assert_sorted_unique(self, roles, "client roles")
        client_shapes = {
            "decoy": (
                "urn:codenoesis:fixture:s7-client-decoy",
                "src/commonMain/kotlin/dev/codenoesis/fixture/"
                "DecoyAccountClient.kt",
                "getAccount",
                "/accounts/{id}",
                "getAccount",
                "rejected_decoy",
            ),
            "safe": (
                "urn:codenoesis:fixture:s7-client-safe",
                "src/commonMain/kotlin/dev/codenoesis/fixture/SafeUsersClient.kt",
                "getSafeUser",
                "/users/{id}",
                "getUser",
                "compatible",
            ),
            "strict": (
                "urn:codenoesis:fixture:s7-client-strict",
                "src/commonMain/kotlin/dev/codenoesis/fixture/"
                "StrictUsersClient.kt",
                "getStrictUser",
                "/users/{id}",
                "getUser",
                "breaking",
            ),
        }
        for client in manifest["clients"]:
            (
                repository_identity,
                source_path,
                symbol,
                operation_path,
                operation_name,
                impact,
            ) = client_shapes[client["role"]]
            self.assertEqual(client["repository_identity"], repository_identity)
            self.assertEqual(client["file"]["path"], source_path)
            self.assertEqual(client["expected_target_impact"], impact)
            root = Path(client["root"])
            roots[(repository_identity, client["revision"])] = root
            self.assertEqual(
                sha256(FIXTURE_ROOT / root / source_path),
                client["file"]["sha256"],
            )
            client_id = stable_id(
                "client",
                "codenoesis.client-id/v1",
                [repository_identity],
            )
            self.assertEqual(client["client_identity"], client_id)
            self.assertEqual(
                client["call_site_id"],
                stable_id(
                    "call-site",
                    "codenoesis.call-site-id/v1",
                    [
                        client_id,
                        client["revision"],
                        source_path,
                        symbol,
                    ],
                ),
            )
            self.assertEqual(
                client["operation_candidate_id"],
                stable_id(
                    "operation",
                    "codenoesis.operation-id/http/v1",
                    [
                        manifest["identities"]["service"],
                        "GET",
                        operation_path,
                        operation_name,
                    ],
                ),
            )

        service_id = stable_id(
            "service",
            "codenoesis.service-id/http/v1",
            ["https://api.example.invalid"],
        )
        self.assertEqual(manifest["identities"]["service"], service_id)
        operation_id = stable_id(
            "operation",
            "codenoesis.operation-id/http/v1",
            [service_id, "GET", "/users/{id}", "getUser"],
        )
        self.assertEqual(manifest["identities"]["operation"], operation_id)
        for field_name in ("displayName", "nickname"):
            self.assertEqual(
                manifest["identities"]["fields"][field_name],
                stable_id(
                    "field",
                    "codenoesis.field-id/http-json/v1",
                    [
                        operation_id,
                        "response",
                        "200",
                        f"/{field_name}",
                    ],
                ),
            )

        report_path = FIXTURE_ROOT / manifest["expected_report"]["path"]
        self.assertEqual(
            sha256(report_path), manifest["expected_report"]["sha256"]
        )
        self.assertTrue(
            all(value is False for value in manifest["sentinels"].values())
        )
        self.assertEqual(
            roots,
            {
                (
                    "urn:codenoesis:fixture:s7-provider",
                    "fixture-provider-a",
                ): Path("provider/revision-a"),
                (
                    "urn:codenoesis:fixture:s7-provider",
                    "fixture-provider-b",
                ): Path("provider/revision-b"),
                (
                    "urn:codenoesis:fixture:s7-client-decoy",
                    "fixture-client-v1",
                ): Path("clients/decoy"),
                (
                    "urn:codenoesis:fixture:s7-client-safe",
                    "fixture-client-v1",
                ): Path("clients/safe"),
                (
                    "urn:codenoesis:fixture:s7-client-strict",
                    "fixture-client-v1",
                ): Path("clients/strict"),
            },
        )

    def test_reviewed_report_proves_diff_clients_decoy_and_unknown(self) -> None:
        report = load_json(REPORT_PATH)
        manifest = load_json(MANIFEST_PATH)
        schema = load_json(SCHEMA_PATH)
        catalog = load_json(RULES_PATH)

        self.assertEqual(set(report), set(schema["required"]))
        self.assertEqual(
            report["schema_version"],
            "codenoesis.semantic-compatibility-report/v1",
        )
        self.assertEqual(
            report["analysis_profile"], "implementation-aware-http-json/v1"
        )
        self.assertEqual(
            report["rule_catalog_version"],
            "codenoesis.compatibility-rules/http-json/v1",
        )
        configuration = {
            "analysis_profile": report["analysis_profile"],
            "dimensions": catalog["dimensions"],
            "rule_catalog_version": report["rule_catalog_version"],
        }
        self.assertEqual(
            report["configuration_hash"],
            f"blake3:{blake3_256(canonical_json(configuration))}",
        )
        assert_sorted_unique(
            self, report["ontology_versions"], "ontology versions"
        )
        extractor_names = [
            item["name"] for item in report["extractor_versions"]
        ]
        assert_sorted_unique(self, extractor_names, "extractor names")
        self.assertEqual(
            len(report["extractor_versions"]),
            len(
                {
                    (item["name"], item["version"])
                    for item in report["extractor_versions"]
                }
            ),
        )
        self.assertLessEqual(
            len(canonical_json(report)), EXPECTED_LIMITS["report_bytes"]
        )
        self.assertEqual(
            report["provider"]["service_identity"],
            manifest["identities"]["service"],
        )
        self.assertEqual(
            report["provider"]["repository_identity"],
            manifest["provider"]["repository_identity"],
        )
        self.assertEqual(
            [
                report["provider"]["baseline"]["revision"],
                report["provider"]["target"]["revision"],
            ],
            [
                item["name"]
                for item in manifest["provider"]["revisions"]
            ],
        )
        self.assertEqual(
            report["provider"]["baseline"]["contract_sha256"],
            report["provider"]["target"]["contract_sha256"],
        )
        for label, revision in zip(
            ("baseline", "target"),
            manifest["provider"]["revisions"],
        ):
            contract = next(
                item
                for item in revision["files"]
                if item["path"] == "openapi.yaml"
            )
            self.assertEqual(
                report["provider"][label]["contract_sha256"],
                contract["sha256"],
            )

        assert_sorted_unique(
            self,
            [item["id"] for item in report["semantic_diffs"]],
            "semantic diffs",
        )
        assert_sorted_unique(
            self,
            [
                item["client_identity"]
                for item in report["client_assessments"]
            ],
            "client assessments",
        )
        assert_sorted_unique(
            self,
            [
                item["client_identity"]
                for item in report["rejected_candidates"]
            ],
            "rejected candidates",
        )
        assert_sorted_unique(
            self,
            [item["id"] for item in report["evidence"]],
            "evidence",
        )
        assert_sorted_unique(
            self,
            [item["id"] for item in report["coverage_gaps"]],
            "coverage gaps",
        )

        roots: dict[tuple[str, str], Path] = {}
        provider_identity = manifest["provider"]["repository_identity"]
        for revision in manifest["provider"]["revisions"]:
            roots[(provider_identity, revision["name"])] = Path(
                revision["root"]
            )
        for client in manifest["clients"]:
            roots[(client["repository_identity"], client["revision"])] = Path(
                client["root"]
            )

        evidence_by_id = {item["id"]: item for item in report["evidence"]}
        for evidence in report["evidence"]:
            path = Path(evidence["path"])
            self.assertFalse(path.is_absolute())
            self.assertNotIn("..", path.parts)
            self.assertGreaterEqual(
                evidence["end_line"], evidence["start_line"]
            )
            fixture_path = (
                FIXTURE_ROOT
                / roots[
                    (
                        evidence["repository_identity"],
                        evidence["revision"],
                    )
                ]
                / path
            )
            lines = fixture_path.read_text(encoding="utf-8").splitlines()
            excerpt = (
                "\n".join(
                    lines[
                        evidence["start_line"] - 1 : evidence["end_line"]
                    ]
                )
                + "\n"
            )
            excerpt_sha256 = hashlib.sha256(excerpt.encode()).hexdigest()
            self.assertEqual(
                evidence["excerpt_sha256"], excerpt_sha256
            )
            self.assertEqual(
                evidence["id"],
                stable_id(
                    "evidence",
                    "codenoesis.evidence-id/v1",
                    [
                        evidence["repository_identity"],
                        evidence["revision"],
                        evidence["path"],
                        str(evidence["start_line"]),
                        str(evidence["end_line"]),
                        excerpt_sha256,
                    ],
                ),
            )
            self.assertEqual(evidence["claim_state"], "deterministic_fact")

        client_by_id = {
            item["client_identity"]: item
            for item in report["client_assessments"]
        }
        manifest_clients = {
            item["client_identity"]: item for item in manifest["clients"]
        }
        self.assertEqual(
            set(client_by_id),
            {
                item["client_identity"]
                for item in manifest["clients"]
                if item["role"] in {"safe", "strict"}
            },
        )
        for client_id, client in client_by_id.items():
            expected = manifest_clients[client_id]
            self.assertEqual(
                client["repository_identity"],
                expected["repository_identity"],
            )
            self.assertEqual(client["call_site_id"], expected["call_site_id"])
            self.assertEqual(
                client["operation_id"], manifest["identities"]["operation"]
            )
        gap_by_id = {item["id"]: item for item in report["coverage_gaps"]}
        referenced_evidence: set[str] = set()
        referenced_gaps: set[str] = set()
        for diff in report["semantic_diffs"]:
            for field in ("evidence_ids", "affected_client_ids", "coverage_gap_ids"):
                assert_sorted_unique(self, diff[field], f"{diff['id']} {field}")
            for view in ("contract", "implementation"):
                assert_sorted_unique(
                    self,
                    diff[view]["evidence_ids"],
                    f"{diff['id']} {view} evidence",
                )
                referenced_evidence.update(diff[view]["evidence_ids"])
            referenced_evidence.update(diff["evidence_ids"])
            referenced_gaps.update(diff["coverage_gap_ids"])
            self.assertTrue(
                set(diff["affected_client_ids"]).issubset(client_by_id)
            )
            self.assertTrue(set(diff["evidence_ids"]).issubset(evidence_by_id))
            self.assertTrue(set(diff["coverage_gap_ids"]).issubset(gap_by_id))
            self.assertEqual(diff["claim_state"], "derived_fact")
            self.assertEqual(diff["contract"]["claim_state"], "deterministic_fact")
            self.assertEqual(
                diff["implementation"]["claim_state"], "derived_fact"
            )

        for client in report["client_assessments"]:
            for field in ("rule_ids", "evidence_ids", "coverage_gap_ids"):
                assert_sorted_unique(
                    self, client[field], f"{client['client_identity']} {field}"
                )
            referenced_evidence.update(client["evidence_ids"])
            referenced_gaps.update(client["coverage_gap_ids"])
            self.assertTrue(set(client["evidence_ids"]).issubset(evidence_by_id))
            self.assertTrue(
                set(client["coverage_gap_ids"]).issubset(gap_by_id)
            )
            self.assertEqual(client["link_state"], "deterministic_fact")
            self.assertEqual(client["assumption_claim_state"], "derived_fact")
            self.assertEqual(
                client["affected"], client["target_impact"] == "breaking"
            )

        for candidate in report["rejected_candidates"]:
            assert_sorted_unique(
                self,
                candidate["evidence_ids"],
                f"{candidate['client_identity']} rejected evidence",
            )
            referenced_evidence.update(candidate["evidence_ids"])
            self.assertTrue(
                set(candidate["evidence_ids"]).issubset(evidence_by_id)
            )
        for gap in report["coverage_gaps"]:
            assert_sorted_unique(
                self, gap["revisions"], f"{gap['id']} revisions"
            )
            assert_sorted_unique(
                self, gap["evidence_ids"], f"{gap['id']} evidence"
            )
            referenced_evidence.update(gap["evidence_ids"])
            self.assertTrue(set(gap["evidence_ids"]).issubset(evidence_by_id))

        self.assertEqual(referenced_evidence, set(evidence_by_id))
        self.assertEqual(referenced_gaps, set(gap_by_id))

        diffs = {
            item["field_pointer"]: item for item in report["semantic_diffs"]
        }
        self.assertEqual(set(diffs), {"/displayName", "/nickname"})
        for field_pointer, diff in diffs.items():
            field_name = field_pointer.removeprefix("/")
            self.assertEqual(
                diff["operation_id"], manifest["identities"]["operation"]
            )
            self.assertEqual(
                diff["field_id"],
                manifest["identities"]["fields"][field_name],
            )
            self.assertEqual(
                diff["id"],
                stable_id(
                    "diff",
                    "codenoesis.diff-id/v1",
                    [
                        report["provider"]["repository_identity"],
                        report["provider"]["baseline"]["revision"],
                        report["provider"]["target"]["revision"],
                        diff["field_id"],
                        diff["dimension"],
                    ],
                ),
            )
        nickname = diffs["/nickname"]
        self.assertEqual(
            nickname["contract"],
            {
                "before": "optional",
                "after": "optional",
                "delta": "unchanged",
                "claim_state": "deterministic_fact",
                "evidence_ids": nickname["contract"]["evidence_ids"],
            },
        )
        self.assertEqual(
            (
                nickname["implementation"]["before"],
                nickname["implementation"]["after"],
                nickname["implementation"]["delta"],
                nickname["change_kind"],
                nickname["classification"],
                nickname["rule_id"],
            ),
            (
                "guaranteed_present",
                "may_be_absent",
                "weakened",
                "implementation_behavior_changed_without_contract_change",
                "breaking",
                "cmp.response.presence.undocumented-guarantee-removed/v1",
            ),
        )
        display_name = diffs["/displayName"]
        self.assertEqual(
            (
                display_name["implementation"]["before"],
                display_name["implementation"]["after"],
                display_name["implementation"]["delta"],
                display_name["classification"],
                display_name["rule_id"],
            ),
            (
                "unknown",
                "unknown",
                "unresolved",
                "unresolved",
                "cmp.unresolved.insufficient-evidence/v1",
            ),
        )
        self.assertEqual(len(display_name["coverage_gap_ids"]), 1)
        gap = gap_by_id[display_name["coverage_gap_ids"][0]]
        self.assertEqual(
            gap["reason_code"], "unsupported_custom_provider_mapping"
        )
        self.assertTrue(gap["blocks_classification"])

        safe = next(
            item
            for item in report["client_assessments"]
            if item["repository_identity"]
            == "urn:codenoesis:fixture:s7-client-safe"
        )
        strict = next(
            item
            for item in report["client_assessments"]
            if item["repository_identity"]
            == "urn:codenoesis:fixture:s7-client-strict"
        )
        self.assertEqual(
            (
                safe["presence_assumption"],
                safe["baseline_risk"],
                safe["target_impact"],
                safe["affected"],
            ),
            ("handles_absent", "compatible", "compatible", False),
        )
        self.assertEqual(
            (
                strict["presence_assumption"],
                strict["baseline_risk"],
                strict["target_impact"],
                strict["affected"],
            ),
            ("requires_present", "potentially_breaking", "breaking", True),
        )
        self.assertEqual(
            nickname["affected_client_ids"], [strict["client_identity"]]
        )
        rejected = report["rejected_candidates"]
        self.assertEqual(len(rejected), 1)
        expected_decoy = next(
            item for item in manifest["clients"] if item["role"] == "decoy"
        )
        self.assertEqual(
            rejected[0]["client_identity"],
            expected_decoy["client_identity"],
        )
        self.assertEqual(
            rejected[0]["call_site_id"], expected_decoy["call_site_id"]
        )
        self.assertEqual(
            rejected[0]["reason_code"], "operation_identity_mismatch"
        )
        self.assertNotIn(rejected[0]["client_identity"], client_by_id)
        self.assertNotIn(
            rejected[0]["client_identity"],
            nickname["affected_client_ids"],
        )

        used_rule_ids = {
            item["rule_id"] for item in report["semantic_diffs"]
        } | {
            rule_id
            for client in report["client_assessments"]
            for rule_id in client["rule_ids"]
        }
        self.assertTrue(used_rule_ids.issubset(EXPECTED_RULE_IDS))
        self.assertEqual(
            EXPECTED_RULE_IDS,
            {item["id"] for item in catalog["rules"]},
        )
        self.assertFalse(
            {
                item["source_kind"] for item in report["evidence"]
            }
            & {"test_observation", "runtime_observation"}
        )

    def test_acceptance_oracle_is_ready_for_future_red_only(self) -> None:
        acceptance = load_json(ACCEPTANCE_PATH)
        self.assertEqual(
            acceptance["schema_version"],
            "codenoesis.acceptance-specification/v1",
        )
        self.assertEqual(
            acceptance["issue"],
            "https://github.com/smutti/codenoesis/issues/62",
        )
        approval = acceptance["approval_pull_request"]
        self.assertTrue(
            approval == "TBD"
            or re.fullmatch(
                r"https://github\.com/smutti/codenoesis/pull/[1-9][0-9]*",
                approval,
            )
        )
        self.assertEqual(
            acceptance["analysis_profile"],
            "implementation-aware-http-json/v1",
        )
        self.assertEqual(len(acceptance["prerequisites"]["required_before_red"]), 5)
        expected_red = acceptance["expected_red"]
        self.assertFalse(expected_red["run_in_governance_change"])
        self.assertEqual(
            expected_red["accepted_error_code"],
            "impact.unsupported_implementation_semantics",
        )
        self.assertIn(
            "comparison of only the two byte-identical OpenAPI files",
            expected_red["rejected_failures"],
        )
        self.assertEqual(acceptance["limits"], EXPECTED_LIMITS)
        self.assertEqual(
            acceptance["semantic_contract"]["dimensions"],
            [
                "presence",
                "nullability",
                "default",
                "validation",
                "value_set",
                "http_status",
                "error_code",
            ],
        )
        self.assertTrue(
            acceptance["oracle"]["provider_contract_sha256_equal"]
        )
        self.assertEqual(
            acceptance["oracle"]["nickname"],
            {
                "contract_baseline": "optional",
                "contract_target": "optional",
                "implementation_baseline": "guaranteed_present",
                "implementation_target": "may_be_absent",
                "change_kind": (
                    "implementation_behavior_changed_without_contract_change"
                ),
                "strict_client_baseline": "potentially_breaking",
                "strict_client_target": "breaking",
                "safe_client_target": "compatible",
            },
        )
        tests_by_requirement = {
            requirement: {
                test["id"]
                for test in acceptance["tests"]
                if requirement in test["requirements"]
            }
            for requirement in S7_REQUIREMENTS
        }
        self.assertTrue(
            tests_by_requirement["DR-SEM-001"]
            >= {
                "conf_dr_sem_001_report_v1",
                "pt_dr_sem_001_semantic_dimensions_are_distinct",
            }
        )
        self.assertTrue(
            tests_by_requirement["FR-IMP-004"]
            >= {
                "e2e_fr_imp_004_implementation_aware_api_diff",
                "gt_fr_imp_004_client_stricter_than_contract",
                "gt_fr_imp_004_safe_client_and_decoy",
                "sec_fr_imp_004_no_target_execution",
            }
        )
        self.assertTrue(
            tests_by_requirement["FR-IMP-005"]
            >= {
                "gt_fr_imp_005_unchanged_contract_implementation_diff",
                "pt_fr_imp_005_rule_precedence_and_unknowns",
            }
        )
        self.assertEqual(
            set(acceptance["allowed_paths"]),
            {
                "docs/software/software-requirements-specification.md",
                "docs/software/roadmap.md",
                "docs/software/decisions/"
                "0007-s7-implementation-aware-api-compatibility-contract.md",
                "tests/specifications/s7",
                "tests/fixtures/s7/implementation-aware-api-v1",
                "scripts/tests/"
                "test_s7_implementation_aware_api_contract.py",
            },
        )
        self.assertIn("production Rust code", acceptance["forbidden"])
        self.assertIn(
            "automatic promotion of type names, heuristic links, model output, "
            "or missing traces into behavioral facts",
            acceptance["forbidden"],
        )

    def test_contract_bundle_binds_every_s7_ratification_artifact(self) -> None:
        bundle = load_json(BUNDLE_PATH)
        self.assertEqual(
            set(bundle), {"schema_version", "files", "bundle_sha256"}
        )
        self.assertEqual(
            bundle["schema_version"], "codenoesis.contract-bundle/v1"
        )
        files = bundle["files"]
        paths = [item["path"] for item in files]
        assert_sorted_unique(self, paths, "bundle paths")
        self.assertEqual(set(paths), S7_BUNDLE_FILES)
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
            r"S7 implementation-aware compatibility contract bundle:\s+"
            r"`sha256:([0-9a-f]{64})`",
            srs,
        )
        self.assertIsNotNone(match, "SRS must bind the complete S7 bundle")
        self.assertEqual(match.group(1), bundle_sha256)  # type: ignore[union-attr]


if __name__ == "__main__":
    unittest.main()
