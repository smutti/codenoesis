from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
AGENTS_PATH = ROOT / "AGENTS.md"
SRS_PATH = ROOT / "docs" / "software" / "software-requirements-specification.md"
AUTONOMY_PATH = ROOT / "docs" / "software" / "autonomous-development.md"
ROADMAP_PATH = ROOT / "docs" / "software" / "roadmap.md"


class AcceleratedDeliveryContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.instructions = AGENTS_PATH.read_text(encoding="utf-8")
        cls.srs = SRS_PATH.read_text(encoding="utf-8")
        cls.autonomy = AUTONOMY_PATH.read_text(encoding="utf-8")
        cls.roadmap = ROADMAP_PATH.read_text(encoding="utf-8")

    def test_maintainer_supervised_accelerated_delivery_exists(self) -> None:
        self.assertIn(
            "## Maintainer-supervised accelerated delivery",
            self.instructions,
        )
        self.assertIn(
            "### 2.1.1 Maintainer-supervised accelerated delivery",
            self.srs,
        )
        self.assertIn(
            "## Maintainer-supervised accelerated lane",
            self.autonomy,
        )
        self.assertIn(
            "### Maintainer-supervised accelerated package",
            self.roadmap,
        )

    def test_one_authorization_covers_one_bounded_vertical_package(self) -> None:
        for document in (self.instructions, self.srs, self.autonomy):
            self.assertIn("one explicit human authorization", document)
            self.assertIn("requirements are `Approved` on `main`", document)

        self.assertIn("one coherent vertical outcome", self.instructions)
        self.assertIn(
            "same delivery slice, public acceptance journey, risk owner, "
            "rollback boundary, and versioned fixture or oracle",
            self.instructions,
        )
        self.assertIn("exact dependency name and version", self.instructions)

    def test_machine_policy_only_blocks_unattended_execution(self) -> None:
        for document in (self.instructions, self.srs, self.autonomy, self.roadmap):
            self.assertIn("unattended autonomous", document)

        self.assertIn(
            "without waiting for `.github/codex/policy.json`",
            self.instructions,
        )
        self.assertIn(
            "machine-policy projection may proceed in parallel",
            self.roadmap,
        )

    def test_corrections_and_final_gate_are_batched(self) -> None:
        for document in (self.instructions, self.autonomy):
            self.assertIn("three correction rounds", document)
            self.assertIn("one through five", document)

        self.assertIn(
            "complete repository gate once on the final review head",
            self.instructions,
        )
        self.assertIn(
            "again only after a correction changes that head or invalidates "
            "the evidence",
            self.instructions,
        )

    def test_non_negotiable_controls_remain_explicit(self) -> None:
        required_instructions = (
            "production implementation must not begin",
            "High- and critical-risk work always requires explicit human approval.",
            "The authoring agent must not approve or merge its own change.",
            "Never push directly to `main`",
            "Do not modify workflow or agent policy in the same pull request as the",
            "weaken, delete, skip, quarantine, regenerate, or retry a failing oracle",
            "Do not fabricate unavailable evidence.",
        )
        for statement in required_instructions:
            self.assertIn(statement, self.instructions)

        self.assertIn(
            "Requirements remain **Proposed** unless an explicit register marks "
            "them",
            self.srs,
        )
        self.assertIn(
            "High/critical changes and protected paths do not auto-merge.",
            self.autonomy,
        )


if __name__ == "__main__":
    unittest.main()
