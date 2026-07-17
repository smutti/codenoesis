from __future__ import annotations

import copy
import json
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / ".github" / "codex" / "scripts"))

from validate_proposal import ProposalValidationError, parse_and_validate  # noqa: E402


class ProposalValidationTests(unittest.TestCase):
    @staticmethod
    def proposal() -> dict[str, object]:
        return {
            "status": "proposed",
            "merge_readiness": "proposal_only",
            "summary": "Bounded proposal",
            "changed_files": ["src/lib.rs", "tests/acceptance.rs"],
            "validation": [
                {
                    "phase": "red",
                    "command": "cargo test acceptance",
                    "outcome": "failed",
                    "exit_code": 101,
                    "evidence": "Expected assertion failed before implementation.",
                },
                {
                    "phase": "green",
                    "command": "cargo test acceptance",
                    "outcome": "passed",
                    "exit_code": 0,
                    "evidence": "Focused acceptance test passed.",
                },
            ],
            "risks": [],
            "blockers": [],
        }

    def test_valid_proposal_is_bound_to_actual_paths(self) -> None:
        result = parse_and_validate(
            json.dumps(self.proposal()).encode(),
            ["tests/acceptance.rs", "src/lib.rs"],
        )
        self.assertEqual(result["status"], "valid")
        self.assertEqual(result["actual_file_count"], 2)

    def test_blocked_or_no_change_status_cannot_publish_a_patch(self) -> None:
        for status in ("blocked", "no_change"):
            proposal = self.proposal()
            proposal["status"] = status
            with self.subTest(status=status), self.assertRaisesRegex(
                ProposalValidationError, "requires status proposed"
            ):
                parse_and_validate(json.dumps(proposal).encode(), ["src/lib.rs"])

    def test_declared_paths_must_match_both_rename_sides(self) -> None:
        proposal = self.proposal()
        proposal["changed_files"] = ["src/new.rs"]
        with self.assertRaisesRegex(ProposalValidationError, "exactly match"):
            parse_and_validate(
                json.dumps(proposal).encode(), ["src/old.rs", "src/new.rs"]
            )

    def test_proposed_requires_red_green_and_no_blocker(self) -> None:
        no_red = self.proposal()
        no_red["validation"] = [copy.deepcopy(no_red["validation"][1])]  # type: ignore[index]
        with self.assertRaisesRegex(ProposalValidationError, "failed Red"):
            parse_and_validate(json.dumps(no_red).encode(), no_red["changed_files"])  # type: ignore[arg-type]

        blocked = self.proposal()
        blocked["blockers"] = ["Decision missing"]
        with self.assertRaisesRegex(ProposalValidationError, "cannot contain blockers"):
            parse_and_validate(json.dumps(blocked).encode(), blocked["changed_files"])  # type: ignore[arg-type]

    def test_duplicate_json_property_is_rejected(self) -> None:
        raw = json.dumps(self.proposal()).replace(
            '"status": "proposed"',
            '"status": "proposed", "status": "proposed"',
            1,
        )
        with self.assertRaisesRegex(ProposalValidationError, "duplicate"):
            parse_and_validate(raw.encode(), ["src/lib.rs", "tests/acceptance.rs"])


if __name__ == "__main__":
    unittest.main()
