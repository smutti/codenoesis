from __future__ import annotations

import copy
import hashlib
import importlib.util
import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from types import ModuleType
from typing import Callable


ROOT = Path(__file__).resolve().parents[2]
ISSUE = "https://github.com/smutti/codenoesis/issues/201"
BASE_SHA = "c783b612777a86e2f88620ece987723bb230c51c"
SPEC_ROOT = ROOT / "tests/specifications/verification/local-baseline-v3"
SCHEMA_PATH = SPEC_ROOT / "manifest.schema.json"
PLAN_PATH = SPEC_ROOT / "plan.json"
CATALOG_PATH = SPEC_ROOT / "profile-catalog.json"
V2_CATALOG_PATH = (
    ROOT / "tests/specifications/verification/local-baseline-v2/profile-catalog.json"
)
MANIFEST_PATH = ROOT / "tests/evidence/verification/local-baseline-v3/manifest.json"
VALIDATOR_PATH = ROOT / "scripts/verify_local_baseline_v3.py"
STATUS_DOCUMENTS = (
    ROOT / "README.md",
    ROOT / "docs/software/software-requirements-specification.md",
    ROOT / "docs/software/architecture.md",
    ROOT / "docs/software/roadmap.md",
    ROOT / "docs/software/verification.md",
)
V2_IMMUTABLE_DIGESTS = {
    "tests/specifications/verification/local-baseline-v2/profile-catalog.json": "23d175dd5e73f0c67e2d8c9d8fdf36c921220f41fcfaaf6586514ed6d632172b",
    "tests/specifications/verification/local-baseline-v2/plan.json": "2b3a6a5e71f35823faeb9a676a0bdd281ebac4095004bc0cd2f84f2f4264cc0f",
    "tests/specifications/verification/local-baseline-v2/manifest.schema.json": "72bb571a0d00b12543ccb4a3e4a42e13211942e1cdf78787f4b379252cb9a2bb",
    "tests/evidence/verification/local-baseline-v2/manifest.json": "123fb538e5f0566470d6f2c740b1e54f3fada3281e522179ed5e914f508e10e3",
    "scripts/verify_local_baseline_v2.py": "59a9eae29b5e756de6dd76895434cc244628b1ff793aa6b2858d6fa324a64499",
}
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
    "r18-trusted-local-source",
    "r19-git-backed-semantic-impact",
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
    "LocalBaselineVerificationV3 candidate Verified pending independent review "
    "and protected manual merge"
)


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


class LocalBaselineVerificationV3ContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.schema = json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))
        cls.plan = json.loads(PLAN_PATH.read_text(encoding="utf-8"))
        cls.catalog = json.loads(CATALOG_PATH.read_text(encoding="utf-8"))
        cls.v2_catalog = json.loads(V2_CATALOG_PATH.read_text(encoding="utf-8"))

    def load_validator(self) -> ModuleType:
        self.assertTrue(VALIDATOR_PATH.is_file(), "V3 validator is absent")
        module_spec = importlib.util.spec_from_file_location(
            "verify_local_baseline_v3",
            VALIDATOR_PATH,
        )
        if module_spec is None or module_spec.loader is None:
            self.fail("cannot load the LocalBaselineVerificationV3 validator")
        validator = importlib.util.module_from_spec(module_spec)
        module_spec.loader.exec_module(validator)
        return validator

    def validate_mutation(
        self,
        mutation: Callable[[dict[str, object]], None],
    ) -> list[str]:
        validator = self.load_validator()
        manifest = json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))
        mutated = copy.deepcopy(manifest)
        mutation(mutated)
        with tempfile.TemporaryDirectory() as temporary_directory:
            manifest_path = Path(temporary_directory) / "manifest.json"
            manifest_path.write_text(
                json.dumps(mutated, ensure_ascii=False) + "\n",
                encoding="utf-8",
            )
            return validator.validate_manifest(ROOT, manifest_path)

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

    def test_catalog_resolves_exact_v2_plus_r18_r19(self) -> None:
        inherited = self.catalog["inherited_catalog"]
        self.assertEqual(inherited["profile_count"], 32)
        self.assertEqual(inherited["sha256"], sha256_file(V2_CATALOG_PATH))
        resolved = [*self.v2_catalog["profiles"], *self.catalog["additive_profiles"]]
        self.assertEqual(tuple(profile["id"] for profile in resolved), PROFILE_IDS)
        self.assertEqual(self.catalog["profile_count"], len(resolved))
        self.assertEqual(
            tuple(self.catalog["required_profile_ids"]),
            PROFILE_IDS,
        )
        for profile in self.catalog["additive_profiles"]:
            self.assertEqual(profile["requirements"], sorted(set(profile["requirements"])))
            self.assertEqual(
                profile["implementation_prs"],
                sorted(set(profile["implementation_prs"])),
            )
            for field in ("decision",):
                self.assertTrue((ROOT / profile[field]).is_file(), profile[field])
            for field in ("oracle_paths", "evidence_paths"):
                for relative_path in profile[field]:
                    self.assertTrue((ROOT / relative_path).is_file(), relative_path)

    def test_v2_contracts_are_byte_immutable(self) -> None:
        for relative_path, expected_digest in V2_IMMUTABLE_DIGESTS.items():
            self.assertEqual(
                sha256_file(ROOT / relative_path),
                expected_digest,
                relative_path,
            )

    def test_manifest_schema_is_closed_and_exact(self) -> None:
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
        self.assertEqual(properties["remote_runs"]["minItems"], 8)
        self.assertEqual(properties["remote_runs"]["maxItems"], 8)

    def test_status_documents_are_aligned(self) -> None:
        for document_path in STATUS_DOCUMENTS:
            normalized = " ".join(document_path.read_text(encoding="utf-8").split())
            self.assertIn(STATUS_MARKER, normalized, str(document_path))

    def test_validator_and_manifest_are_complete(self) -> None:
        self.assertTrue(
            VALIDATOR_PATH.is_file() and MANIFEST_PATH.is_file(),
            "LocalBaselineVerificationV3 validator and canonical 34-profile manifest are absent",
        )

    def test_candidate_manifest_is_complete_and_valid(self) -> None:
        validator = self.load_validator()
        self.assertTrue(MANIFEST_PATH.is_file(), "V3 manifest is absent")
        self.assertEqual(validator.validate_manifest(ROOT, MANIFEST_PATH), [])

    def test_profile_omission_duplication_and_order_fail_closed(self) -> None:
        mutations = {
            "omission": lambda manifest: manifest["profiles"].pop(),
            "duplication": lambda manifest: manifest["profiles"].__setitem__(
                33, copy.deepcopy(manifest["profiles"][32])
            ),
            "order": lambda manifest: manifest["profiles"].__setitem__(
                slice(32, 34), list(reversed(manifest["profiles"][32:34]))
            ),
        }
        for name, mutation in mutations.items():
            with self.subTest(name=name):
                self.assertTrue(self.validate_mutation(mutation))

    def test_identity_digest_and_lifecycle_mutations_fail_closed(self) -> None:
        mutations = {
            "base": lambda manifest: manifest.__setitem__("base_sha", "0" * 40),
            "product_tree": lambda manifest: manifest.__setitem__(
                "product_tree_sha256", "0" * 64
            ),
            "catalog_digest": lambda manifest: manifest["profile_catalog"].__setitem__(
                "sha256", "0" * 64
            ),
            "partial_status": lambda manifest: manifest.__setitem__(
                "status", "partially_verified"
            ),
            "self_review": lambda manifest: manifest["review"].__setitem__(
                "authoring_agent_is_independent_reviewer", True
            ),
        }
        for name, mutation in mutations.items():
            with self.subTest(name=name):
                self.assertTrue(self.validate_mutation(mutation))

    def test_remote_runs_are_exact_and_green(self) -> None:
        manifest = json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))
        self.assertEqual(
            {run["run_id"] for run in manifest["remote_runs"]},
            {
                32229313149,
                32229313162,
                32229311917,
                32229313006,
                33203760261,
                33203760248,
                33203757887,
                33203758766,
            },
        )
        self.assertEqual({run["conclusion"] for run in manifest["remote_runs"]}, {"success"})

    def test_public_cli_is_canonical_and_fail_closed(self) -> None:
        valid = subprocess.run(
            ["python3", str(VALIDATOR_PATH), "--manifest", str(MANIFEST_PATH)],
            cwd=ROOT,
            check=False,
            capture_output=True,
        )
        self.assertEqual(valid.returncode, 0, valid.stderr.decode("utf-8"))
        parsed = json.loads(valid.stdout)
        self.assertEqual(valid.stdout, json.dumps(parsed, sort_keys=True, separators=(",", ":")).encode("utf-8") + b"\n")
        with tempfile.TemporaryDirectory() as temporary_directory:
            invalid_path = Path(temporary_directory) / "invalid.json"
            invalid_path.write_text("{}\n", encoding="utf-8")
            invalid = subprocess.run(
                ["python3", str(VALIDATOR_PATH), "--manifest", str(invalid_path)],
                cwd=ROOT,
                check=False,
                capture_output=True,
            )
        self.assertNotEqual(invalid.returncode, 0)
        self.assertEqual(invalid.stdout, b"")
        failure = json.loads(invalid.stderr)
        self.assertEqual(failure["schema_version"], "codenoesis.local-baseline-verification-error/v1")


if __name__ == "__main__":
    unittest.main()
