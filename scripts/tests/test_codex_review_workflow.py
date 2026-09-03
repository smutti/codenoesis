from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
REVIEW_WORKFLOW_PATH = ROOT / ".github" / "workflows" / "codex-review.yml"
AUTONOMY_PATH = ROOT / "docs" / "software" / "autonomous-development.md"


def job(workflow: str, name: str, following_name: str | None = None) -> str:
    start = workflow.index(f"  {name}:\n")
    if following_name is None:
        return workflow[start:]
    end = workflow.index(f"  {following_name}:\n", start)
    return workflow[start:end]


class CodexReviewWorkflowTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.review_workflow = REVIEW_WORKFLOW_PATH.read_text(encoding="utf-8")
        cls.autonomy = AUTONOMY_PATH.read_text(encoding="utf-8")
        cls.normalized_autonomy = " ".join(cls.autonomy.split())
        cls.inspect_job = job(cls.review_workflow, "inspect", "review")
        cls.review_job = job(cls.review_workflow, "review", "codex-review-gate")
        cls.review_gate_job = job(cls.review_workflow, "codex-review-gate")

    def test_only_read_only_review_automation_remains(self) -> None:
        self.assertTrue(REVIEW_WORKFLOW_PATH.exists())
        self.assertFalse(
            (ROOT / ".github" / "workflows" / "codex-propose.yml").exists()
        )
        self.assertFalse(
            (ROOT / ".github" / "workflows" / "codex-watchdog.yml").exists()
        )
        self.assertIn("vars.CODEX_REVIEW_ENABLED", self.review_workflow)
        self.assertNotIn("CODEX_AUTOMATION_ENABLED", self.review_workflow)
        self.assertNotIn("CODEX_PUBLISHER", self.review_workflow)

    def test_model_job_remains_read_only_and_secret_scoped(self) -> None:
        required_controls = (
            "environment: codex-model",
            "contents: read",
            "persist-credentials: false",
            "openai-api-key: ${{ secrets.OPENAI_API_KEY }}",
            'permission-profile: ":read-only"',
            "safety-strategy: drop-sudo",
            "codex-args: '[\"--ephemeral\"]'",
            "retention-days: 14",
        )
        for control in required_controls:
            self.assertIn(control, self.review_job)

        self.assertNotIn("contents: write", self.review_job)
        self.assertNotIn("pull-requests: write", self.review_job)

    def test_review_scope_has_only_a_technical_capacity_limit(self) -> None:
        self.assertIn('if [ "$changed_files" -le 256 ]; then', self.inspect_job)
        self.assertIn('"kind":"file_count_limit"', self.inspect_job)
        self.assertNotIn("hard_protected", self.review_workflow)
        self.assertNotIn("codex-policy", self.review_workflow)
        self.assertIn(
            'reason="Diff exceeds AI review file-count capacity"',
            self.review_gate_job,
        )

    def test_policy_gate_is_optional_and_fail_closed_when_enabled(self) -> None:
        self.assertIn(
            "REVIEW_ENABLED: ${{ vars.CODEX_REVIEW_ENABLED }}",
            self.review_gate_job,
        )
        self.assertIn(
            'elif [ "$REVIEW_ENABLED" != "true" ]; then', self.review_gate_job
        )
        self.assertIn(
            'reason="AI review disabled by dedicated repository review switch"',
            self.review_gate_job,
        )
        self.assertIn(
            'select(.severity == "P0" or .severity == "P1")',
            self.review_gate_job,
        )
        self.assertIn('if [ "$verdict" != "pass" ]', self.review_gate_job)

    def test_docs_keep_review_disabled_without_the_secret(self) -> None:
        self.assertIn("## Read-only AI review activation", self.autonomy)
        self.assertIn("`CODEX_REVIEW_ENABLED`", self.autonomy)
        self.assertIn("`OPENAI_API_KEY`", self.autonomy)
        self.assertIn(
            "Without that environment secret, the review switch must remain "
            "`false`.",
            self.normalized_autonomy,
        )


if __name__ == "__main__":
    unittest.main()
