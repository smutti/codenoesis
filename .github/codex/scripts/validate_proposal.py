#!/usr/bin/env python3
"""Revalidate Codex proposal evidence before granting publisher credentials."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any, NoReturn, Sequence

from codex_policy import CodexPolicyError, read_nul_delimited_paths


MAX_INPUT_BYTES = 512 * 1024
ROOT_KEYS = {
    "status",
    "merge_readiness",
    "summary",
    "changed_files",
    "validation",
    "risks",
    "blockers",
}
VALIDATION_KEYS = {"phase", "command", "outcome", "exit_code", "evidence"}


class ProposalValidationError(ValueError):
    """Proposal evidence is malformed or inconsistent with the patch."""


def _fail(message: str) -> NoReturn:
    raise ProposalValidationError(message)


def _strict_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            _fail(f"duplicate JSON property: {key}")
        result[key] = value
    return result


def _string(value: Any, name: str, minimum: int, maximum: int) -> str:
    if not isinstance(value, str) or not minimum <= len(value) <= maximum:
        _fail(f"{name} must be a string of {minimum}..{maximum} characters")
    return value


def _string_array(value: Any, name: str, maximum_items: int, maximum_length: int) -> list[str]:
    if not isinstance(value, list) or len(value) > maximum_items:
        _fail(f"{name} must be an array with at most {maximum_items} items")
    result = [
        _string(item, f"{name}[{index}]", 1, maximum_length)
        for index, item in enumerate(value)
    ]
    if len(set(result)) != len(result):
        _fail(f"{name} must not contain duplicates")
    return result


def parse_and_validate(raw: bytes, actual_paths: Sequence[str]) -> dict[str, Any]:
    """Validate strict proposal JSON and bind it to the actual staged paths."""

    if len(raw) > MAX_INPUT_BYTES:
        _fail("proposal JSON exceeds 512 KiB")
    try:
        value = json.loads(
            raw.decode("utf-8"),
            object_pairs_hook=_strict_object,
            parse_constant=lambda token: _fail(f"non-standard JSON number: {token}"),
        )
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ProposalValidationError(f"invalid UTF-8 JSON: {error}") from error
    if not isinstance(value, dict) or set(value) != ROOT_KEYS:
        _fail("proposal must contain exactly the trusted root fields")

    status = value["status"]
    if status not in {"proposed", "no_change", "blocked"}:
        _fail("status is invalid")
    if value["merge_readiness"] != "proposal_only":
        _fail("merge_readiness must be proposal_only")
    _string(value["summary"], "summary", 1, 4000)
    changed_files = _string_array(value["changed_files"], "changed_files", 100, 1000)
    risks = _string_array(value["risks"], "risks", 30, 1000)
    blockers = _string_array(value["blockers"], "blockers", 30, 1000)

    validation = value["validation"]
    if not isinstance(validation, list) or len(validation) > 50:
        _fail("validation must be an array with at most 50 items")
    saw_red = False
    saw_green = False
    for index, record in enumerate(validation):
        if not isinstance(record, dict) or set(record) != VALIDATION_KEYS:
            _fail(f"validation[{index}] has an invalid field set")
        if record["phase"] not in {"red", "green", "regression", "other"}:
            _fail(f"validation[{index}].phase is invalid")
        if record["outcome"] not in {"passed", "failed", "not_run"}:
            _fail(f"validation[{index}].outcome is invalid")
        exit_code = record["exit_code"]
        if exit_code is not None and type(exit_code) is not int:
            _fail(f"validation[{index}].exit_code must be an integer or null")
        _string(record["command"], f"validation[{index}].command", 1, 2000)
        _string(record["evidence"], f"validation[{index}].evidence", 1, 4000)
        saw_red |= (
            record["phase"] == "red"
            and record["outcome"] == "failed"
            and type(exit_code) is int
            and exit_code >= 1
        )
        saw_green |= (
            record["phase"] in {"green", "regression"}
            and record["outcome"] == "passed"
            and exit_code == 0
        )

    actual = list(actual_paths)
    if status != "proposed":
        _fail("a non-empty patch requires status proposed")
    if blockers:
        _fail("a proposed patch cannot contain blockers")
    if not saw_red or not saw_green:
        _fail("a proposed patch requires one failed Red and one passing Green/regression record")
    if set(changed_files) != set(actual) or len(changed_files) != len(actual):
        _fail("declared changed_files do not exactly match the staged patch paths")

    return {
        "actual_file_count": len(actual),
        "declared_file_count": len(changed_files),
        "merge_readiness": "proposal_only",
        "risk_count": len(risks),
        "status": "valid",
        "validation_record_count": len(validation),
    }


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("proposal", type=Path)
    parser.add_argument("changed_paths", type=Path)
    arguments = parser.parse_args(argv)
    try:
        paths = read_nul_delimited_paths(arguments.changed_paths)
        result = parse_and_validate(arguments.proposal.read_bytes(), paths)
    except (OSError, ProposalValidationError, CodexPolicyError) as error:
        print(f"validate_proposal: {error}", file=sys.stderr)
        return 2
    json.dump(result, sys.stdout, sort_keys=True, separators=(",", ":"))
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
