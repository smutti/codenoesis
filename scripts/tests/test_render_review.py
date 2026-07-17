from __future__ import annotations

import copy
import json
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / ".github" / "codex" / "scripts"))

from render_review import (  # noqa: E402
    MAX_COMMENT_CHARS,
    ReviewValidationError,
    main,
    parse_review_json,
    render_review,
    validate_review,
)


class ReviewRendererTests(unittest.TestCase):
    @staticmethod
    def valid_review() -> dict[str, object]:
        return {
            "verdict": "needs_changes",
            "summary": "The change has one actionable defect.",
            "findings": [
                {
                    "severity": "P1",
                    "confidence": "high",
                    "title": "Bounds check is missing",
                    "file": "src/parser.rs",
                    "line": 42,
                    "evidence": "An unchecked index can exceed the input length.",
                    "recommendation": "Validate the index before reading the byte.",
                }
            ],
            "limitations": ["The integration suite was not executed."],
        }

    def test_valid_review_is_readable(self) -> None:
        rendered = render_review(self.valid_review())

        self.assertIn("## Codex read-only review", rendered)
        self.assertIn("**Verdict:** `needs_changes`", rendered)
        self.assertIn("### Summary", rendered)
        self.assertIn("Finding 1 — P1 (high confidence)", rendered)
        self.assertIn("src&#47;parser&#46;rs:42", rendered)
        self.assertIn("### Limitations", rendered)
        self.assertNotIn("raw immutable review artifact", rendered)

    def test_revalidates_exact_shape_enums_types_and_limits(self) -> None:
        review = self.valid_review()
        review["unexpected"] = True
        with self.assertRaisesRegex(ReviewValidationError, "unexpected properties"):
            validate_review(review)

        review = self.valid_review()
        review["findings"][0]["severity"] = "critical"  # type: ignore[index]
        with self.assertRaisesRegex(ReviewValidationError, "severity"):
            validate_review(review)

        review = self.valid_review()
        review["findings"][0]["line"] = True  # type: ignore[index]
        with self.assertRaisesRegex(ReviewValidationError, "line"):
            validate_review(review)

        review = self.valid_review()
        review["summary"] = "x" * 4001
        with self.assertRaisesRegex(ReviewValidationError, "summary"):
            validate_review(review)

    def test_untrusted_text_is_inert_html(self) -> None:
        attack = (
            "@octocat https://evil.example/a <script>alert(1)</script> "
            "[click](https://evil.example) ![image](https://evil.example/x)\n"
            "```\n/close\n~~~"
        )
        review = self.valid_review()
        review["summary"] = attack
        review["findings"][0]["title"] = attack[:200]  # type: ignore[index]
        review["findings"][0]["file"] = attack  # type: ignore[index]
        review["findings"][0]["evidence"] = attack  # type: ignore[index]
        review["findings"][0]["recommendation"] = attack  # type: ignore[index]
        review["limitations"] = [attack]

        rendered = render_review(review)

        for active_source in (
            "@octocat",
            "https://",
            "<script>",
            "[click](",
            "![image]",
            "```",
            "/close",
            "~~~",
        ):
            self.assertNotIn(active_source, rendered)
        self.assertIn("&#64;octocat", rendered)
        self.assertIn("https:&#47;&#47;evil&#46;example", rendered)
        self.assertIn("&lt;script&gt;", rendered)
        self.assertIn("&#47;close", rendered)

    def test_extreme_valid_review_is_bounded_and_declares_authority(self) -> None:
        finding = self.valid_review()["findings"][0]  # type: ignore[index]
        finding["title"] = "[title](https://example.test)" * 6
        finding["file"] = "nested/path.with-punctuation/" * 30
        finding["evidence"] = ("@team https://example.test `evidence` " * 110)[:4000]
        finding["recommendation"] = (
            "[recommendation](https://example.test) " * 55
        )[:2000]

        review = {
            "verdict": "human_review",
            "summary": "https://example.test/@team " * 140,
            "findings": [copy.deepcopy(finding) for _ in range(50)],
            "limitations": ["https://example.test/@team " * 35 for _ in range(20)],
        }

        rendered = render_review(review)

        self.assertLessEqual(len(rendered), MAX_COMMENT_CHARS)
        self.assertLess(len(rendered), 60_000)
        self.assertIn("**Verdict:** `human_review`", rendered)
        self.assertIn("Validated findings: 50.", rendered)
        self.assertIn("Validated limitations: 20.", rendered)
        self.assertIn("raw immutable review artifact is authoritative", rendered)

    def test_parser_rejects_duplicate_keys_and_nonstandard_numbers(self) -> None:
        with self.assertRaisesRegex(ReviewValidationError, "duplicate"):
            parse_review_json(
                b'{"verdict":"pass","verdict":"pass","summary":"ok",'
                b'"findings":[],"limitations":[]}'
            )

        with self.assertRaisesRegex(ReviewValidationError, "non-standard"):
            parse_review_json(
                b'{"verdict":"pass","summary":"ok","findings":[],'
                b'"limitations":[],"ignored":NaN}'
            )

    def test_cli_writes_markdown_file(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            input_path = Path(directory) / "review.json"
            output_path = Path(directory) / "comment.md"
            input_path.write_text(json.dumps(self.valid_review()), encoding="utf-8")

            self.assertEqual(main([str(input_path), str(output_path)]), 0)
            rendered = output_path.read_text(encoding="utf-8")
            self.assertIn("## Codex read-only review", rendered)
            self.assertLess(len(rendered), 60_000)


if __name__ == "__main__":
    unittest.main()
