from __future__ import annotations

import json
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
ISSUE = "https://github.com/smutti/codenoesis/issues/188"
BASE_SHA = "9ecdc3acefd43495daf76b9f2ab69a7bbacff172"
SPEC_ROOT = ROOT / "tests/specifications/verification/local-baseline-v2"
SCHEMA_PATH = SPEC_ROOT / "manifest.schema.json"
PLAN_PATH = SPEC_ROOT / "plan.json"
CATALOG_PATH = SPEC_ROOT / "profile-catalog.json"
MANIFEST_PATH = ROOT / "tests/evidence/verification/local-baseline-v2/manifest.json"
VALIDATOR_PATH = ROOT / "scripts/verify_local_baseline_v2.py"
S0_SCHEMA_PATH = ROOT / "tests/specifications/s0/evidence-manifest-v1.schema.json"
SRS_PATH = ROOT / "docs/software/software-requirements-specification.md"
ROADMAP_PATH = ROOT / "docs/software/roadmap.md"
README_PATH = ROOT / "README.md"

PROFILE_IDS = (
    "s0-walking-skeleton",
    "s1-safe-inventory",
    "r1-packed-sha1",
    "r2-gitlink-boundaries",
    "s2-rust-knowledge",
    "s3-atomic-storage",
    "s4-workspace-docs-query",
    "r3-root-package-workspace",
    "r4-cargo-manifest-facts",
    "r5-rust-semantic-depth",
    "r6-framework-declarations",
    "r7-scip-import",
    "r8-portable-explorer",
    "s5-incremental-refresh",
    "s6-openapi-federation",
    "k1-callable-value-semantics",
    "r9-output-capacity",
    "r10-cfg-declaration-alternatives",
    "r11-k1-repository-boundaries",
    "r12-k1-cfg-alternatives",
    "r13-k1-scip-composition",
    "r14-expression-bindings",
    "r15-local-flow",
    "r14-r15-real-repository-correction",
    "r16-safe-constant-evaluation",
    "r10-r16-versioned-explorer-correction",
    "r17-function-context",
    "s7-implementation-aware-api",
    "g0-release-profile",
    "g1a-local-distribution",
    "g2a-local-upgrade-safety",
    "g1b-g8-verifiable-distribution",
)

REQUIRED_EVIDENCE_CLASSES = (
    "retained_red",
    "focused_green",
    "full_regression",
    "linux_native",
    "macos_native",
    "windows_native",
    "codeql",
    "benchmark_integrity",
    "policy_gate",
    "security",
    "browser",
    "real_repository_pilot",
    "supply_chain",
    "traceability",
    "independent_review",
)

STATUS_MARKER = (
    "LocalBaselineVerificationV2 candidate Verified pending independent review "
    "and protected manual merge"
)


class LocalBaselineVerificationV2ContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.schema = json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))
        cls.plan = json.loads(PLAN_PATH.read_text(encoding="utf-8"))
        cls.catalog = json.loads(CATALOG_PATH.read_text(encoding="utf-8"))
        cls.s0_schema = json.loads(S0_SCHEMA_PATH.read_text(encoding="utf-8"))

    def test_plan_fixes_exact_authority_and_scope(self) -> None:
        self.assertEqual(self.plan["issue"], ISSUE)
        self.assertEqual(self.plan["base_sha"], BASE_SHA)
        self.assertEqual(self.plan["delivery_slice"], "S14")
        self.assertEqual(self.plan["risk"], "high")
        self.assertFalse(self.plan["runtime_changes_allowed"])
        self.assertFalse(self.plan["control_plane_changes_allowed"])
        self.assertFalse(self.plan["release_authority_allowed"])
        self.assertFalse(self.plan["new_dependencies_allowed"])
        self.assertFalse(self.plan["partial_verification_allowed"])
        self.assertEqual(self.plan["correction_budget"], 5)
        self.assertEqual(tuple(self.plan["required_profile_ids"]), PROFILE_IDS)
        self.assertEqual(
            tuple(self.plan["required_evidence_classes"]),
            REQUIRED_EVIDENCE_CLASSES,
        )

    def test_catalog_is_complete_and_references_existing_inputs(self) -> None:
        profiles = self.catalog["profiles"]
        self.assertEqual(tuple(profile["id"] for profile in profiles), PROFILE_IDS)
        self.assertEqual(len({profile["id"] for profile in profiles}), len(profiles))
        for profile in profiles:
            self.assertEqual(
                profile["requirements"],
                sorted(set(profile["requirements"])),
                profile["id"],
            )
            self.assertEqual(
                profile["implementation_prs"],
                sorted(set(profile["implementation_prs"])),
                profile["id"],
            )
            for field in ("oracle_paths", "evidence_paths"):
                self.assertTrue(profile[field], f"{profile['id']} has no {field}")
                for relative_path in profile[field]:
                    self.assertTrue(
                        (ROOT / relative_path).is_file(),
                        f"missing {field}: {relative_path}",
                    )

    def test_manifest_schema_is_closed_and_base_bound(self) -> None:
        self.assertFalse(self.schema["additionalProperties"])
        properties = self.schema["properties"]
        self.assertEqual(properties["issue"]["const"], ISSUE)
        self.assertEqual(properties["base_sha"]["const"], BASE_SHA)
        self.assertEqual(properties["verification_subject"]["const"], BASE_SHA)
        self.assertEqual(
            properties["status"]["const"],
            "candidate_verified_pending_merge",
        )
        self.assertEqual(properties["profiles"]["minItems"], len(PROFILE_IDS))
        self.assertEqual(properties["profiles"]["maxItems"], len(PROFILE_IDS))

    def test_s0_schema_keeps_artifact_and_adds_strict_git_retention(self) -> None:
        definitions = self.s0_schema["$defs"]
        retained = definitions["retained_ci_evidence"]["oneOf"]
        self.assertEqual(
            retained,
            [
                {"$ref": "#/$defs/github_actions_artifact"},
                {"$ref": "#/$defs/git_retained_github_actions_log"},
            ],
        )
        git_retained = definitions["git_retained_github_actions_log"]
        self.assertFalse(git_retained["additionalProperties"])
        required = set(git_retained["required"])
        self.assertTrue(
            {
                "workflow_sha256",
                "run_id",
                "run_attempt",
                "job_id",
                "head_sha",
                "tree_sha",
                "source_log_sha256",
                "committed_log_sha256",
                "first_retained_commit",
                "evidence_head",
            }.issubset(required)
        )
        self.assertTrue(git_retained["properties"]["base_controlled"]["const"])

    def test_validator_exists(self) -> None:
        self.assertTrue(VALIDATOR_PATH.is_file(), "verification validator is absent")

    def test_candidate_manifest_exists(self) -> None:
        self.assertTrue(MANIFEST_PATH.is_file(), "verification manifest is absent")

    def test_status_documents_are_aligned(self) -> None:
        for document_path in (SRS_PATH, ROADMAP_PATH, README_PATH):
            normalized = " ".join(document_path.read_text(encoding="utf-8").split())
            self.assertIn(
                STATUS_MARKER,
                normalized,
                f"verification status marker is absent from {document_path}",
            )


if __name__ == "__main__":
    unittest.main()
