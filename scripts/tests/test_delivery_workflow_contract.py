from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


class DeliveryWorkflowContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.agents = (ROOT / "AGENTS.md").read_text(encoding="utf-8")
        cls.contributing = (ROOT / "CONTRIBUTING.md").read_text(encoding="utf-8")
        cls.workflow = (
            ROOT / "docs" / "software" / "autonomous-development.md"
        ).read_text(encoding="utf-8")
        cls.roadmap = (ROOT / "docs" / "software" / "roadmap.md").read_text(
            encoding="utf-8"
        )
        cls.srs = (
            ROOT / "docs" / "software" / "software-requirements-specification.md"
        ).read_text(encoding="utf-8")

    def test_default_flow_is_plan_test_benchmark_implementation(self) -> None:
        for document in (
            self.agents,
            self.contributing,
            self.workflow,
            self.roadmap,
            self.srs,
        ):
            normalized = document.lower()
            for term in ("plan", "test", "benchmark", "implementation"):
                self.assertIn(term, normalized)
            self.assertIn("one pull request", normalized)

    def test_governance_and_code_can_share_the_pull_request(self) -> None:
        for document in (self.agents, self.workflow, self.roadmap, self.srs):
            normalized = " ".join(document.lower().split())
            for term in (
                "requirements",
                "architecture",
                "ontology",
                "schemas",
                "workflows",
                "permissions",
                "release configuration",
            ):
                self.assertIn(term, normalized)

    def test_old_delivery_rituals_are_not_active_agent_rules(self) -> None:
        forbidden = (
            "authorization to implement",
            "maintainer-supervised accelerated delivery",
            "classify each task before editing",
            "high- and critical-risk work always requires explicit human approval",
            "required evidence pack",
            "the default budget is three correction rounds",
        )
        normalized = " ".join(self.agents.lower().split())
        for phrase in forbidden:
            self.assertNotIn(phrase, normalized)

        self.assertIn("an issue, requirement id, slice, risk label", normalized)
        self.assertIn("is not required", normalized)
        self.assertIn("a retained red-before-code ceremony is not required", normalized)

    def test_only_technical_and_irreversible_controls_remain(self) -> None:
        for command in (
            "actionlint -no-color",
            "cargo fmt --all --check",
            "cargo clippy --workspace",
            "cargo test --workspace",
            "python3 -m unittest discover",
            "python3 scripts/validate_benchmark_assets.py",
            "cargo bench --workspace",
        ):
            self.assertIn(command, self.agents)

        for condition in (
            "secret",
            "publish",
            "sign",
            "deploy",
            "release",
            "delete data",
            "licensing",
        ):
            self.assertIn(condition, self.agents.lower())

    def test_automated_proposal_lane_is_removed(self) -> None:
        removed_paths = (
            ".github/workflows/codex-propose.yml",
            ".github/workflows/codex-watchdog.yml",
            ".github/codex/prompts/propose-from-issue.md",
            ".github/codex/schemas/proposal-output.schema.json",
            ".github/codex/scripts/validate_proposal.py",
            ".github/codex/scripts/codex_policy.py",
        )
        for relative_path in removed_paths:
            self.assertFalse((ROOT / relative_path).exists(), relative_path)

    def test_code_ownership_has_no_special_file_categories(self) -> None:
        codeowners = (ROOT / ".github" / "CODEOWNERS").read_text(encoding="utf-8")
        active_lines = [
            line for line in codeowners.splitlines() if line and not line.startswith("#")
        ]
        self.assertEqual(active_lines, ["* @smutti"])


if __name__ == "__main__":
    unittest.main()
