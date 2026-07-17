#!/usr/bin/env python3
"""Validate Codex review JSON and render a bounded, inert GitHub comment."""

from __future__ import annotations

import argparse
import html
import json
import sys
import unicodedata
from pathlib import Path
from typing import Any, NoReturn, Sequence


MAX_INPUT_BYTES = 512 * 1024
MAX_COMMENT_CHARS = 59_000

VERDICTS = frozenset({"pass", "needs_changes", "human_review"})
SEVERITIES = frozenset({"P0", "P1", "P2", "P3"})
CONFIDENCES = frozenset({"high", "medium", "low"})

REVIEW_KEYS = frozenset({"verdict", "summary", "findings", "limitations"})
FINDING_KEYS = frozenset(
    {
        "severity",
        "confidence",
        "title",
        "file",
        "line",
        "evidence",
        "recommendation",
    }
)

TRUNCATION_NOTICE = (
    "> This rendered comment was truncated for GitHub. The raw immutable review "
    "artifact is authoritative and contains the complete validated output."
)
POLICY_FOOTER = (
    "_Static AI review only; deterministic CI and human ownership rules remain "
    "authoritative._"
)

# These characters can activate GitHub mentions, autolinks, references, Markdown
# links/images, code fences, or slash commands. Numeric entities remain readable
# when GitHub renders the controlled <pre> element, while the Markdown parser
# never sees the active source spelling.
_INERT_HTML_ENTITIES = str.maketrans(
    {
        "@": "&#64;",
        "/": "&#47;",
        ".": "&#46;",
        "[": "&#91;",
        "]": "&#93;",
        "(": "&#40;",
        ")": "&#41;",
        "!": "&#33;",
        "`": "&#96;",
        "~": "&#126;",
        "#": "&#35;",
        "\\": "&#92;",
    }
)


class ReviewValidationError(ValueError):
    """Raised when input does not match the trusted review contract."""


def _fail(path: str, message: str) -> NoReturn:
    raise ReviewValidationError(f"{path}: {message}")


def _validate_exact_keys(
    value: dict[str, Any], expected: frozenset[str], path: str
) -> None:
    if any(not isinstance(key, str) for key in value):
        _fail(path, "property names must be strings")
    actual = frozenset(value)
    missing = sorted(expected - actual)
    extra = sorted(actual - expected)
    if missing:
        _fail(path, f"missing required properties: {', '.join(missing)}")
    if extra:
        _fail(path, f"unexpected properties: {', '.join(extra)}")


def _validate_string(
    value: object,
    path: str,
    *,
    minimum: int,
    maximum: int,
) -> None:
    if not isinstance(value, str):
        _fail(path, "must be a string")
    length = len(value)
    if length < minimum or length > maximum:
        _fail(path, f"length must be between {minimum} and {maximum} characters")


def validate_review(value: object) -> dict[str, Any]:
    """Validate the complete review schema without third-party dependencies."""

    if not isinstance(value, dict):
        _fail("$", "must be an object")
    _validate_exact_keys(value, REVIEW_KEYS, "$")

    verdict = value["verdict"]
    if not isinstance(verdict, str) or verdict not in VERDICTS:
        _fail("$.verdict", f"must be one of: {', '.join(sorted(VERDICTS))}")

    _validate_string(value["summary"], "$.summary", minimum=1, maximum=4000)

    findings = value["findings"]
    if not isinstance(findings, list):
        _fail("$.findings", "must be an array")
    if len(findings) > 50:
        _fail("$.findings", "must contain at most 50 items")

    for index, finding in enumerate(findings):
        path = f"$.findings[{index}]"
        if not isinstance(finding, dict):
            _fail(path, "must be an object")
        _validate_exact_keys(finding, FINDING_KEYS, path)

        severity = finding["severity"]
        if not isinstance(severity, str) or severity not in SEVERITIES:
            _fail(
                f"{path}.severity",
                f"must be one of: {', '.join(sorted(SEVERITIES))}",
            )

        confidence = finding["confidence"]
        if not isinstance(confidence, str) or confidence not in CONFIDENCES:
            _fail(
                f"{path}.confidence",
                f"must be one of: {', '.join(sorted(CONFIDENCES))}",
            )

        _validate_string(
            finding["title"], f"{path}.title", minimum=1, maximum=200
        )
        _validate_string(
            finding["file"], f"{path}.file", minimum=1, maximum=1000
        )

        line = finding["line"]
        if line is not None and (type(line) is not int or line < 1):
            _fail(f"{path}.line", "must be null or an integer greater than zero")

        _validate_string(
            finding["evidence"], f"{path}.evidence", minimum=1, maximum=4000
        )
        _validate_string(
            finding["recommendation"],
            f"{path}.recommendation",
            minimum=1,
            maximum=2000,
        )

    limitations = value["limitations"]
    if not isinstance(limitations, list):
        _fail("$.limitations", "must be an array")
    if len(limitations) > 20:
        _fail("$.limitations", "must contain at most 20 items")
    for index, limitation in enumerate(limitations):
        _validate_string(
            limitation,
            f"$.limitations[{index}]",
            minimum=1,
            maximum=1000,
        )

    return value


def _reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ReviewValidationError(f"duplicate JSON property: {key}")
        result[key] = value
    return result


def _reject_nonstandard_number(value: str) -> NoReturn:
    raise ReviewValidationError(f"non-standard JSON number: {value}")


def parse_review_json(raw: bytes) -> dict[str, Any]:
    """Parse strict UTF-8 JSON, rejecting duplicate keys and NaN/Infinity."""

    if len(raw) > MAX_INPUT_BYTES:
        raise ReviewValidationError(
            f"input exceeds the {MAX_INPUT_BYTES}-byte evidence limit"
        )
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ReviewValidationError("input must be valid UTF-8") from error
    try:
        value = json.loads(
            text,
            object_pairs_hook=_reject_duplicate_keys,
            parse_constant=_reject_nonstandard_number,
        )
    except json.JSONDecodeError as error:
        raise ReviewValidationError(
            f"invalid JSON at line {error.lineno}, column {error.colno}: {error.msg}"
        ) from error
    return validate_review(value)


def _visible_text(value: str) -> str:
    """Expose invisible/control characters as deterministic printable escapes."""

    rendered: list[str] = []
    for character in value:
        if character in {"\n", "\t"}:
            rendered.append(character)
            continue
        category = unicodedata.category(character)
        if category in {"Cc", "Cf", "Cs", "Zl", "Zp"}:
            codepoint = ord(character)
            if codepoint <= 0xFFFF:
                rendered.append(f"\\u{codepoint:04X}")
            else:
                rendered.append(f"\\U{codepoint:08X}")
        else:
            rendered.append(character)
    return "".join(rendered)


def inert_html(value: str) -> str:
    """Return readable HTML text that cannot activate GitHub Markdown features."""

    escaped = html.escape(_visible_text(value), quote=True)
    return escaped.translate(_INERT_HTML_ENTITIES)


def _pre(value: str) -> str:
    return f"<pre>{inert_html(value)}</pre>"


def _fit_pre(value: str, maximum: int) -> tuple[str, bool]:
    """Render a complete, balanced pre block no longer than ``maximum``."""

    complete = _pre(value)
    if len(complete) <= maximum:
        return complete, False

    marker = "\n[... field truncated ...]"
    smallest = _pre(marker.lstrip("\n"))
    if len(smallest) > maximum:
        # Callers provide substantially larger budgets. Keep this branch safe and
        # deterministic if the helper is reused with an unexpectedly tiny one.
        return "", True

    low = 0
    high = len(value)
    while low < high:
        middle = (low + high + 1) // 2
        candidate = _pre(value[:middle] + marker)
        if len(candidate) <= maximum:
            low = middle
        else:
            high = middle - 1
    return _pre(value[:low] + marker), True


def _render_finding(
    index: int,
    finding: dict[str, Any],
    *,
    bounded: bool,
) -> tuple[str, bool]:
    location = finding["file"]
    if finding["line"] is not None:
        location = f"{location}:{finding['line']}"

    if bounded:
        title, title_cut = _fit_pre(finding["title"], 1_100)
        location_block, location_cut = _fit_pre(location, 1_900)
        evidence, evidence_cut = _fit_pre(finding["evidence"], 3_300)
        recommendation, recommendation_cut = _fit_pre(
            finding["recommendation"], 2_000
        )
    else:
        title, location_block = _pre(finding["title"]), _pre(location)
        evidence, recommendation = (
            _pre(finding["evidence"]),
            _pre(finding["recommendation"]),
        )
        title_cut = location_cut = evidence_cut = recommendation_cut = False

    block = "\n\n".join(
        (
            f"#### Finding {index} — {finding['severity']} "
            f"({finding['confidence']} confidence)",
            f"**Title**\n\n{title}",
            f"**Location**\n\n{location_block}",
            f"**Evidence**\n\n{evidence}",
            f"**Recommendation**\n\n{recommendation}",
        )
    )
    return block, any(
        (title_cut, location_cut, evidence_cut, recommendation_cut)
    )


def _render_full(review: dict[str, Any]) -> str:
    parts = [
        "## Codex read-only review",
        f"**Verdict:** `{review['verdict']}`",
        "### Summary",
        _pre(review["summary"]),
        "### Findings",
    ]

    if review["findings"]:
        parts.append(f"Validated findings: {len(review['findings'])}.")
        for index, finding in enumerate(review["findings"], start=1):
            block, _ = _render_finding(index, finding, bounded=False)
            parts.append(block)
    else:
        parts.append("No actionable findings.")

    parts.append("### Limitations")
    if review["limitations"]:
        parts.append(f"Validated limitations: {len(review['limitations'])}.")
        for index, limitation in enumerate(review["limitations"], start=1):
            parts.append(f"**Limitation {index}**\n\n{_pre(limitation)}")
    else:
        parts.append("No reported limitations.")

    parts.append(POLICY_FOOTER)
    return "\n\n".join(parts) + "\n"


def _render_bounded(review: dict[str, Any]) -> str:
    """Render a semantically complete summary while reserving space per section."""

    summary, _ = _fit_pre(review["summary"], 9_000)
    parts = [
        "## Codex read-only review",
        f"**Verdict:** `{review['verdict']}`",
        "### Summary",
        summary,
        "### Findings",
    ]

    findings = review["findings"]
    if not findings:
        parts.append("No actionable findings.")
    else:
        parts.append(f"Validated findings: {len(findings)}.")

    # Keep a fixed reserve for the limitations section and authoritative notice.
    fixed_tail = "\n\n".join(
        (
            "### Limitations",
            f"Validated limitations: {len(review['limitations'])}.",
            TRUNCATION_NOTICE,
            POLICY_FOOTER,
        )
    )
    included_findings = 0
    for index, finding in enumerate(findings, start=1):
        block, _ = _render_finding(index, finding, bounded=True)
        candidate_length = len("\n\n".join((*parts, block, fixed_tail))) + 1
        if candidate_length > MAX_COMMENT_CHARS - 9_000:
            break
        parts.append(block)
        included_findings += 1

    if included_findings < len(findings):
        omitted = len(findings) - included_findings
        parts.append(
            f"_{omitted} validated finding(s) omitted from this bounded rendering._"
        )

    parts.append("### Limitations")
    limitations = review["limitations"]
    if not limitations:
        parts.append("No reported limitations.")
    else:
        parts.append(f"Validated limitations: {len(limitations)}.")

    included_limitations = 0
    for index, limitation in enumerate(limitations, start=1):
        remaining = MAX_COMMENT_CHARS - len("\n\n".join(parts))
        # Leave ample room for an omission line, notice, footer, and separators.
        block_budget = min(1_800, remaining - len(TRUNCATION_NOTICE) - 500)
        if block_budget < 100:
            break
        body, _ = _fit_pre(limitation, block_budget - 30)
        block = f"**Limitation {index}**\n\n{body}"
        candidate = "\n\n".join(
            (*parts, block, TRUNCATION_NOTICE, POLICY_FOOTER)
        ) + "\n"
        if len(candidate) > MAX_COMMENT_CHARS:
            break
        parts.append(block)
        included_limitations += 1

    if included_limitations < len(limitations):
        omitted = len(limitations) - included_limitations
        parts.append(
            f"_{omitted} validated limitation(s) omitted from this bounded rendering._"
        )

    parts.extend((TRUNCATION_NOTICE, POLICY_FOOTER))
    rendered = "\n\n".join(parts) + "\n"
    if len(rendered) > MAX_COMMENT_CHARS:
        raise AssertionError("bounded renderer exceeded its hard comment limit")
    return rendered


def render_review(value: object) -> str:
    """Validate and render a review as a safe GitHub Markdown comment."""

    review = validate_review(value)
    complete = _render_full(review)
    if len(complete) <= MAX_COMMENT_CHARS:
        return complete
    return _render_bounded(review)


def _read_input(path: str) -> bytes:
    if path == "-":
        return sys.stdin.buffer.read(MAX_INPUT_BYTES + 1)
    with Path(path).open("rb") as source:
        return source.read(MAX_INPUT_BYTES + 1)


def _write_output(path: str, content: str) -> None:
    if path == "-":
        sys.stdout.write(content)
        return
    with Path(path).open("w", encoding="utf-8", newline="\n") as output:
        output.write(content)


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Validate Codex review JSON and render an inert GitHub comment."
    )
    parser.add_argument("input", help="review JSON path, or - for stdin")
    parser.add_argument("output", help="Markdown output path, or - for stdout")
    arguments = parser.parse_args(argv)

    try:
        review = parse_review_json(_read_input(arguments.input))
        _write_output(arguments.output, render_review(review))
    except (OSError, ReviewValidationError) as error:
        print(f"render_review: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
