from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
AGENTS_PATH = ROOT / "AGENTS.md"
SRS_PATH = ROOT / "docs" / "software" / "software-requirements-specification.md"
AUTONOMY_PATH = ROOT / "docs" / "software" / "autonomous-development.md"
ROADMAP_PATH = ROOT / "docs" / "software" / "roadmap.md"


def normalized_prose(document: str) -> str:
    return " ".join(document.split())


class AcceleratedDeliveryContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.instructions = AGENTS_PATH.read_text(encoding="utf-8")
        cls.srs = SRS_PATH.read_text(encoding="utf-8")
        cls.autonomy = AUTONOMY_PATH.read_text(encoding="utf-8")
        cls.roadmap = ROADMAP_PATH.read_text(encoding="utf-8")
        cls.normalized_instructions = normalized_prose(cls.instructions)
        cls.normalized_srs = normalized_prose(cls.srs)
        cls.normalized_autonomy = normalized_prose(cls.autonomy)
        cls.normalized_roadmap = normalized_prose(cls.roadmap)

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
        for document in (
            self.normalized_instructions,
            self.normalized_srs,
            self.normalized_autonomy,
            self.normalized_roadmap,
        ):
            self.assertIn("one explicit human authorization", document)
            self.assertIn("single-PR vertical package", document)

        self.assertIn("one coherent vertical outcome", self.normalized_instructions)
        self.assertIn(
            "same delivery slice, public acceptance journey, risk owner, "
            "rollback boundary, and versioned fixture or oracle",
            self.normalized_instructions,
        )
        self.assertIn(
            "exact dependency name and version",
            self.normalized_instructions,
        )

    def test_product_governance_and_implementation_share_one_pr(self) -> None:
        for document in (
            self.normalized_instructions,
            self.normalized_srs,
            self.normalized_autonomy,
            self.normalized_roadmap,
        ):
            self.assertIn(
                "product governance and production implementation in one pull "
                "request",
                document,
            )
            self.assertIn("branch-scoped implementation authority", document)
            self.assertIn("effective atomically only after", document)

        self.assertIn(
            "SRS, architecture decisions, threat models, schemas or ontology "
            "contracts, fixtures or oracles, traceability, and operational "
            "documentation",
            self.normalized_instructions,
        )
        self.assertNotIn(
            "Use two pull requests for one high-risk capability",
            self.normalized_roadmap,
        )

    def test_delivery_control_plane_can_share_the_vertical_pr(self) -> None:
        for document in (
            self.normalized_instructions,
            self.normalized_srs,
            self.normalized_autonomy,
            self.normalized_roadmap,
        ):
            self.assertIn(
                "product code and delivery control plane in one pull request",
                document,
            )
            self.assertIn("`AGENTS.md`, `.github/**`, and `.codex/**`", document)
            self.assertIn("immutable base authority", document)
            self.assertIn("base-controlled gate", document)
            self.assertIn(
                "inert as authority for that same pull request",
                document,
            )
            self.assertIn("activates only after manual merge", document)

        self.assertNotIn(
            "Do not modify workflow or agent policy in the same pull request as "
            "the",
            self.normalized_instructions,
        )
        self.assertNotIn(
            "must remain in a separate pull request from the product change",
            self.normalized_instructions,
        )

    def test_governance_checkpoint_preserves_red_first(self) -> None:
        for document in (
            self.normalized_instructions,
            self.normalized_srs,
            self.normalized_autonomy,
            self.normalized_roadmap,
        ):
            self.assertIn("governance checkpoint", document)
            self.assertIn("before any production source edit", document)
            self.assertIn("retained expected Red evidence", document)

        self.assertIn(
            "semantic requirement, oracle, scope, dependency, authority, or risk "
            "change",
            self.normalized_instructions,
        )

    def test_machine_policy_only_blocks_unattended_execution(self) -> None:
        for document in (
            self.normalized_instructions,
            self.normalized_srs,
            self.normalized_autonomy,
            self.normalized_roadmap,
        ):
            self.assertIn("unattended autonomous", document)

        self.assertIn(
            "without waiting for `.github/codex/policy.json`",
            self.normalized_instructions,
        )
        self.assertIn(
            "machine-policy projection may proceed in parallel",
            self.normalized_roadmap,
        )
        self.assertNotIn(
            "Use three pull requests instead",
            self.normalized_roadmap,
        )

    def test_corrections_and_final_gate_are_batched(self) -> None:
        for document in (
            self.normalized_instructions,
            self.normalized_autonomy,
        ):
            self.assertIn("three correction rounds", document)
            self.assertIn("one through five", document)

        self.assertIn(
            "complete repository gate once on the final review head",
            self.normalized_instructions,
        )
        self.assertIn(
            "again only after a correction changes that head or invalidates "
            "the evidence",
            self.normalized_instructions,
        )

    def test_non_negotiable_controls_remain_explicit(self) -> None:
        required_instructions = (
            "Outside this branch-scoped exception, production implementation "
            "must not begin",
            "High- and critical-risk work always requires explicit human approval.",
            "The authoring agent must not approve or merge its own change.",
            "Never push directly to `main`",
            "A head-authored control change never judges or authorizes its own "
            "pull request.",
            "No unmerged head receives privileged secrets",
            "weaken, delete, skip, quarantine, regenerate, or retry a failing oracle",
            "Do not fabricate unavailable evidence.",
        )
        for statement in required_instructions:
            self.assertIn(statement, self.normalized_instructions)

        self.assertIn(
            "Requirements remain **Proposed** unless an explicit register marks "
            "them",
            self.normalized_srs,
        )
        self.assertIn(
            "High/critical changes and protected paths do not auto-merge.",
            self.normalized_autonomy,
        )

    def test_base_authority_controls_privileged_effects(self) -> None:
        for document in (
            self.normalized_instructions,
            self.normalized_srs,
            self.normalized_autonomy,
            self.normalized_roadmap,
        ):
            self.assertIn("delivery control plane", document)
            self.assertIn("required checks", document)
            self.assertIn("manual merge", document)
            self.assertIn("privileged secrets", document)
            self.assertIn("signing", document)
            self.assertIn("release", document)


if __name__ == "__main__":
    unittest.main()
