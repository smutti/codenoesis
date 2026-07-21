from __future__ import annotations

import copy
import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = REPOSITORY_ROOT / ".github/codex/scripts/codex_policy.py"
POLICY_PATH = REPOSITORY_ROOT / ".github/codex/policy.json"
SCHEMA_PATH = REPOSITORY_ROOT / ".github/codex/policy.schema.json"
APPROVED_S0_REQUIREMENTS = (
    "DR-ART-001",
    "DR-ART-002",
    "FR-ACQ-001",
    "FR-CLI-003",
    "NFR-DET-001",
    "NFR-MNT-001",
    "NFR-SEC-005",
    "NFR-TST-001",
    "NFR-TST-002",
)
S0_APPROVAL_SOURCE_SHA = "7dd9a0e0b97cad007dfc21f18c8f3c29b43140c1"
S0_APPROVAL_REFERENCE = "https://github.com/smutti/codenoesis/pull/8"

SPEC = importlib.util.spec_from_file_location("codex_policy", MODULE_PATH)
if SPEC is None or SPEC.loader is None:  # pragma: no cover - import contract
    raise RuntimeError(f"Cannot load {MODULE_PATH}")
codex_policy = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(codex_policy)


def approval(
    requirement_id: str = "FR-ACQ-001", delivery_slice: str = "S0"
) -> dict[str, str]:
    return {
        "id": requirement_id,
        "slice": delivery_slice,
        "approved_by": "github:smutti",
        "approved_at": "2026-07-17T20:15:00Z",
        "source_sha": "a" * 40,
        "approval_reference": "https://github.com/smutti/codenoesis/issues/1",
    }


class PolicyFixture(unittest.TestCase):
    def setUp(self) -> None:
        self.policy = copy.deepcopy(codex_policy.load_policy(POLICY_PATH))


class PolicyValidationTests(PolicyFixture):
    def test_repository_policy_and_schema_are_json(self) -> None:
        schema = json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))

        self.assertEqual(1, self.policy["version"])
        approvals = self.policy["approved_requirements"]
        self.assertEqual(
            list(APPROVED_S0_REQUIREMENTS),
            [approval_record["id"] for approval_record in approvals],
        )
        for approval_record in approvals:
            self.assertEqual("S0", approval_record["slice"])
            self.assertEqual("github:smutti", approval_record["approved_by"])
            self.assertEqual(S0_APPROVAL_SOURCE_SHA, approval_record["source_sha"])
            self.assertEqual(
                S0_APPROVAL_REFERENCE, approval_record["approval_reference"]
            )
        self.assertEqual(
            "https://json-schema.org/draft/2020-12/schema", schema["$schema"]
        )
        approval_reference = schema["properties"]["approved_requirements"]["items"]
        self.assertEqual("#/$defs/approval", approval_reference["$ref"])

    def test_all_canonical_patterns_are_present(self) -> None:
        protected = set(self.policy["hard_protected_patterns"])

        self.assertTrue(
            set(codex_policy.CANONICAL_HARD_PROTECTED_PATTERNS).issubset(protected)
        )

    def test_missing_canonical_pattern_is_rejected(self) -> None:
        self.policy["hard_protected_patterns"].remove(".gitattributes")

        with self.assertRaisesRegex(
            codex_policy.PolicyValidationError, "omits canonical"
        ):
            codex_policy.validate_policy(self.policy)

    def test_duplicate_protected_pattern_is_rejected(self) -> None:
        self.policy["hard_protected_patterns"].append(".github/**")

        with self.assertRaisesRegex(
            codex_policy.PolicyValidationError, "duplicate globs"
        ):
            codex_policy.validate_policy(self.policy)

    def test_duplicate_approval_id_is_rejected_even_if_records_differ(self) -> None:
        first = approval()
        second = approval()
        second["approval_reference"] = "https://example.invalid/other"
        self.policy["approved_requirements"] = [first, second]

        with self.assertRaisesRegex(
            codex_policy.PolicyValidationError, "duplicate IDs"
        ):
            codex_policy.validate_policy(self.policy)

    def test_approval_requires_exact_fields(self) -> None:
        record = approval()
        record["comment"] = "not part of the trusted schema"
        self.policy["approved_requirements"] = [record]

        with self.assertRaisesRegex(
            codex_policy.PolicyValidationError, "invalid field set"
        ):
            codex_policy.validate_policy(self.policy)

    def test_approval_requires_real_rfc3339_timestamp(self) -> None:
        record = approval()
        record["approved_at"] = "2026-02-30T20:15:00Z"
        self.policy["approved_requirements"] = [record]

        with self.assertRaisesRegex(
            codex_policy.PolicyValidationError, "real timestamp"
        ):
            codex_policy.validate_policy(self.policy)

    def test_approval_requires_lowercase_full_git_sha(self) -> None:
        record = approval()
        record["source_sha"] = "A" * 40
        self.policy["approved_requirements"] = [record]

        with self.assertRaisesRegex(
            codex_policy.PolicyValidationError, "lowercase 40- or 64-hex"
        ):
            codex_policy.validate_policy(self.policy)

    def test_boolean_limit_is_not_accepted_as_integer(self) -> None:
        self.policy["limits"]["review_max_files"] = True

        with self.assertRaisesRegex(
            codex_policy.PolicyValidationError, "must be an integer"
        ):
            codex_policy.validate_policy(self.policy)

    def test_duplicate_json_object_keys_are_rejected(self) -> None:
        raw = POLICY_PATH.read_text(encoding="utf-8")
        raw = raw.replace('"version": 1,', '"version": 1,\n  "version": 1,', 1)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "policy.json"
            path.write_text(raw, encoding="utf-8")

            with self.assertRaisesRegex(
                codex_policy.PolicyValidationError, "duplicate object keys"
            ):
                codex_policy.load_policy(path)


class GlobSemanticsTests(unittest.TestCase):
    def test_double_star_slash_matches_zero_or_more_directories(self) -> None:
        self.assertTrue(codex_policy.glob_matches("**/AGENTS.md", "AGENTS.md"))
        self.assertTrue(
            codex_policy.glob_matches("**/AGENTS.md", "crates/core/AGENTS.md")
        )
        self.assertFalse(
            codex_policy.glob_matches("**/AGENTS.md", "crates/core/AGENTS.md.bak")
        )

    def test_double_star_between_segments_can_match_zero_segments(self) -> None:
        self.assertTrue(codex_policy.glob_matches("src/**/mod.rs", "src/mod.rs"))
        self.assertTrue(
            codex_policy.glob_matches("src/**/mod.rs", "src/a/b/mod.rs")
        )

    def test_single_star_never_crosses_separator(self) -> None:
        self.assertTrue(codex_policy.glob_matches("src/*.rs", "src/lib.rs"))
        self.assertFalse(
            codex_policy.glob_matches("src/*.rs", "src/nested/lib.rs")
        )

    def test_question_mark_matches_exactly_one_non_separator(self) -> None:
        self.assertTrue(codex_policy.glob_matches("src/mod?.rs", "src/mod1.rs"))
        self.assertFalse(codex_policy.glob_matches("src/mod?.rs", "src/mod.rs"))
        self.assertFalse(
            codex_policy.glob_matches("src/?.rs", "src/nested/a.rs")
        )

    def test_globs_are_whole_path_matches(self) -> None:
        self.assertFalse(codex_policy.glob_matches("lib.rs", "src/lib.rs"))
        self.assertFalse(codex_policy.glob_matches("src", "src/lib.rs"))

    def test_character_classes_and_braces_are_literal(self) -> None:
        self.assertTrue(
            codex_policy.glob_matches("src/[ab].{rs}", "src/[ab].{rs}")
        )
        self.assertFalse(codex_policy.glob_matches("src/[ab].rs", "src/a.rs"))

    def test_double_star_must_be_a_complete_segment(self) -> None:
        for invalid in ("src/foo**bar.rs", "src/***.rs", "src/a**"):
            with self.subTest(invalid=invalid), self.assertRaises(
                codex_policy.InputValidationError
            ):
                codex_policy.compile_glob(invalid)

    def test_absolute_parent_and_backslash_globs_are_rejected(self) -> None:
        for invalid in ("/src/**", "src/../secret", "src\\**", "!src/**"):
            with self.subTest(invalid=invalid), self.assertRaises(
                codex_policy.InputValidationError
            ):
                codex_policy.compile_glob(invalid)


class AuthorizationTests(PolicyFixture):
    def test_exact_approved_ids_for_one_slice_are_authorized(self) -> None:
        self.policy["approved_requirements"] = [
            approval("FR-ACQ-001", "S0"),
            approval("DR-ART-001", "S0"),
        ]

        result = codex_policy.authorize_requirements(
            self.policy, ["FR-ACQ-001", "DR-ART-001"], "S0"
        )

        self.assertTrue(result["authorized"])
        self.assertEqual(
            ["FR-ACQ-001", "DR-ART-001"], result["requirement_ids"]
        )

    def test_missing_approval_is_denied(self) -> None:
        self.policy["approved_requirements"] = []

        with self.assertRaises(codex_policy.AuthorizationError) as raised:
            codex_policy.authorize_requirements(
                self.policy, ["FR-ACQ-001"], "S0"
            )

        self.assertEqual(["FR-ACQ-001"], raised.exception.details["missing"])

    def test_approval_for_another_slice_is_denied(self) -> None:
        self.policy["approved_requirements"] = [approval(delivery_slice="S1")]

        with self.assertRaises(codex_policy.AuthorizationError) as raised:
            codex_policy.authorize_requirements(
                self.policy, ["FR-ACQ-001"], "S0"
            )

        mismatch = raised.exception.details["slice_mismatches"][0]
        self.assertEqual("S1", mismatch["approved_slice"])
        self.assertEqual("S0", mismatch["requested_slice"])

    def test_authorization_is_case_sensitive_and_exact(self) -> None:
        self.policy["approved_requirements"] = [approval()]

        with self.assertRaises(codex_policy.InputValidationError):
            codex_policy.authorize_requirements(
                self.policy, ["fr-acq-001"], "S0"
            )

    def test_requirement_id_requires_numeric_suffix(self) -> None:
        for invalid in (
            "FR-ACQ",
            "FR-ACQ-ABC",
            "FR-ACQ-01",
            "FR-ACQ-0001",
            "REQ-001",
        ):
            with self.subTest(invalid=invalid), self.assertRaises(
                codex_policy.InputValidationError
            ):
                codex_policy.authorize_requirements(self.policy, [invalid], "S0")

    def test_duplicate_requested_id_is_rejected(self) -> None:
        self.policy["approved_requirements"] = [approval()]

        with self.assertRaisesRegex(
            codex_policy.InputValidationError, "contain duplicates"
        ):
            codex_policy.authorize_requirements(
                self.policy, ["FR-ACQ-001", "FR-ACQ-001"], "S0"
            )

    def test_slice_must_be_exactly_s0_through_s14(self) -> None:
        for invalid in ("S15", "S01", "S0 text", "s0"):
            with self.subTest(invalid=invalid), self.assertRaises(
                codex_policy.InputValidationError
            ):
                codex_policy.authorize_requirements(
                    self.policy, ["FR-ACQ-001"], invalid
                )


class ChangedPathInputTests(unittest.TestCase):
    def _read(self, content: bytes) -> tuple[str, ...]:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "paths"
            path.write_bytes(content)
            return codex_policy.read_nul_delimited_paths(path)

    def test_reads_strict_nul_delimited_utf8_paths(self) -> None:
        self.assertEqual(
            ("src/lib.rs", "tests/acceptance.rs"),
            self._read(b"src/lib.rs\0tests/acceptance.rs\0"),
        )

    def test_missing_terminal_nul_is_rejected(self) -> None:
        with self.assertRaisesRegex(
            codex_policy.InputValidationError, "must end with a NUL"
        ):
            self._read(b"src/lib.rs")

    def test_empty_interior_record_is_rejected(self) -> None:
        with self.assertRaisesRegex(
            codex_policy.InputValidationError, "empty record"
        ):
            self._read(b"src/lib.rs\0\0tests/lib.rs\0")

    def test_non_utf8_record_is_rejected(self) -> None:
        with self.assertRaisesRegex(
            codex_policy.InputValidationError, "not valid UTF-8"
        ):
            self._read(b"src/\xff.rs\0")

    def test_duplicate_path_is_rejected(self) -> None:
        with self.assertRaisesRegex(
            codex_policy.InputValidationError, "duplicate entries"
        ):
            self._read(b"src/lib.rs\0src/lib.rs\0")

    def test_unsafe_exact_paths_are_rejected(self) -> None:
        unsafe = (
            "/absolute",
            "src/../secret",
            "src\\lib.rs",
            "src/*.rs",
            ".git/config",
            "src//lib.rs",
            "src/visible\u202esecret.rs",
        )
        for path in unsafe:
            with self.subTest(path=path), self.assertRaises(
                codex_policy.InputValidationError
            ):
                codex_policy.validate_repository_path(path)


class ChangedPathScopeTests(PolicyFixture):
    def test_allowed_paths_pass(self) -> None:
        result = codex_policy.validate_changed_paths(
            self.policy,
            ["xtask/src/lib.rs", "xtask/src/main.rs"],
            ["xtask/src/**"],
            ["xtask/src/generated/**"],
        )

        self.assertTrue(result["allowed"])
        self.assertEqual(2, result["file_count"])

    def test_outside_allowlist_is_denied(self) -> None:
        with self.assertRaises(codex_policy.PathScopeError) as raised:
            codex_policy.validate_changed_paths(
                self.policy, ["README.md"], ["xtask/**"]
            )

        self.assertEqual(
            "outside_allowed_patterns",
            raised.exception.details["violations"][0]["kind"],
        )

    def test_issue_protected_path_is_denied(self) -> None:
        with self.assertRaises(codex_policy.PathScopeError) as raised:
            codex_policy.validate_changed_paths(
                self.policy,
                ["xtask/src/generated/model.rs"],
                ["xtask/**"],
                ["xtask/src/generated/**"],
            )

        kinds = [item["kind"] for item in raised.exception.details["violations"]]
        self.assertIn("issue_protected_path", kinds)

    def test_hard_protection_overrides_broad_allowlist(self) -> None:
        with self.assertRaises(codex_policy.PathScopeError) as raised:
            codex_policy.validate_changed_paths(
                self.policy,
                ["docs/software/architecture.md"],
                ["**"],
            )

        kinds = [item["kind"] for item in raised.exception.details["violations"]]
        self.assertIn("hard_protected_path", kinds)

    def test_nested_agents_file_is_hard_protected(self) -> None:
        with self.assertRaises(codex_policy.PathScopeError) as raised:
            codex_policy.validate_changed_paths(
                self.policy, ["crates/parser/AGENTS.md"], ["crates/**"]
            )

        self.assertEqual(
            ["**/AGENTS.md"],
            raised.exception.details["violations"][0]["matched_patterns"],
        )

    def test_issue_pattern_may_repeat_a_canonical_pattern(self) -> None:
        with self.assertRaises(codex_policy.PathScopeError) as raised:
            codex_policy.validate_changed_paths(
                self.policy,
                [".github/workflows/ci.yml"],
                ["**"],
                [".github/**"],
            )

        kinds = [item["kind"] for item in raised.exception.details["violations"]]
        self.assertEqual(["issue_protected_path", "hard_protected_path"], kinds)

    def test_rename_old_path_is_independently_denied(self) -> None:
        with self.assertRaises(codex_policy.PathScopeError) as raised:
            codex_policy.validate_changed_paths(
                self.policy,
                ["docs/software/architecture.md", "src/architecture.rs"],
                ["**"],
            )

        violations = raised.exception.details["violations"]
        self.assertTrue(
            any(
                item.get("kind") == "hard_protected_path"
                and item.get("path") == "docs/software/architecture.md"
                for item in violations
            )
        )

    def test_rename_new_path_is_independently_denied(self) -> None:
        with self.assertRaises(codex_policy.PathScopeError) as raised:
            codex_policy.validate_changed_paths(
                self.policy,
                ["src/old.rs", ".github/workflows/new.yml"],
                ["**"],
            )

        violations = raised.exception.details["violations"]
        self.assertTrue(
            any(
                item.get("kind") == "hard_protected_path"
                and item.get("path") == ".github/workflows/new.yml"
                for item in violations
            )
        )

    def test_proposal_max_files_cannot_be_relaxed_by_argument(self) -> None:
        self.policy["limits"]["proposal_max_files"] = 1

        with self.assertRaises(codex_policy.PathScopeError) as raised:
            codex_policy.validate_changed_paths(
                self.policy,
                ["src/a.rs", "src/b.rs"],
                ["src/**"],
                max_files=100,
            )

        self.assertEqual(1, raised.exception.details["max_files"])


class ReviewScopeTests(PolicyFixture):
    def test_small_product_diff_is_ai_review_eligible(self) -> None:
        result = codex_policy.classify_review_scope(
            self.policy, ["xtask/src/lib.rs"]
        )

        self.assertTrue(result["eligible"])
        self.assertEqual("ai_review_eligible", result["classification"])

    def test_hard_protected_diff_requires_human_review(self) -> None:
        result = codex_policy.classify_review_scope(
            self.policy, ["benchmarks/manifest.json"]
        )

        self.assertFalse(result["eligible"])
        self.assertEqual("human_review_required", result["classification"])
        self.assertEqual("hard_protected_paths", result["reasons"][0]["kind"])

    def test_file_count_equal_to_maximum_is_eligible(self) -> None:
        result = codex_policy.classify_review_scope(
            self.policy, ["src/a.rs", "src/b.rs"], max_files=2
        )

        self.assertTrue(result["eligible"])

    def test_file_count_above_maximum_requires_human_review(self) -> None:
        result = codex_policy.classify_review_scope(
            self.policy, ["src/a.rs", "src/b.rs", "src/c.rs"], max_files=2
        )

        self.assertFalse(result["eligible"])
        self.assertEqual("max_files_exceeded", result["reasons"][0]["kind"])

    def test_requested_review_max_cannot_relax_trusted_limit(self) -> None:
        self.policy["limits"]["review_max_files"] = 1

        result = codex_policy.classify_review_scope(
            self.policy, ["src/a.rs", "src/b.rs"], max_files=100
        )

        self.assertFalse(result["eligible"])
        self.assertEqual(1, result["max_files"])


class CommandLineTests(PolicyFixture):
    def run_cli(self, *arguments: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, str(MODULE_PATH), *arguments],
            cwd=REPOSITORY_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )

    def test_validate_policy_command_outputs_machine_readable_json(self) -> None:
        result = self.run_cli("validate-policy", "--policy", str(POLICY_PATH))

        self.assertEqual(0, result.returncode, result.stderr)
        payload = json.loads(result.stdout)
        self.assertEqual("valid", payload["status"])
        self.assertEqual(
            len(APPROVED_S0_REQUIREMENTS), payload["approved_requirement_count"]
        )

    def test_authorization_denial_has_stable_exit_code_and_json_error(self) -> None:
        result = self.run_cli(
            "authorize",
            "--policy",
            str(POLICY_PATH),
            "--slice",
            "S0",
            "--requirement-id",
            "FR-INV-001",
        )

        self.assertEqual(4, result.returncode)
        payload = json.loads(result.stderr)
        self.assertEqual("authorization_denied", payload["error"]["code"])

    def test_path_scope_denial_has_stable_exit_code(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path_file = Path(directory) / "paths"
            path_file.write_bytes(b".github/workflows/ci.yml\0")
            result = self.run_cli(
                "validate-paths",
                "--policy",
                str(POLICY_PATH),
                "--changed-paths-file",
                str(path_file),
                "--allowed-patterns-json",
                '["**"]',
            )

        self.assertEqual(5, result.returncode)
        payload = json.loads(result.stderr)
        self.assertEqual("path_scope_denied", payload["error"]["code"])

    def test_review_classification_is_non_error_json(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path_file = Path(directory) / "paths"
            path_file.write_bytes(b".gitattributes\0")
            result = self.run_cli(
                "classify-review",
                "--policy",
                str(POLICY_PATH),
                "--changed-paths-file",
                str(path_file),
            )

        self.assertEqual(0, result.returncode, result.stderr)
        payload = json.loads(result.stdout)
        self.assertFalse(payload["eligible"])
        self.assertEqual("human_review_required", payload["classification"])


if __name__ == "__main__":
    unittest.main()
