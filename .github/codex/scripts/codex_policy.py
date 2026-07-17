#!/usr/bin/env python3
"""Deterministic authorization and path policy for Codex automation.

The module intentionally uses only the Python standard library.  Repository
globs are whole-path matches with these semantics:

* ``*`` matches zero or more characters other than ``/``;
* ``?`` matches exactly one character other than ``/``;
* ``**`` is allowed only as a complete path segment and matches zero or more
  characters, including ``/``;
* ``**/`` therefore matches zero or more complete directory segments.

Character classes and brace expansion are treated literally; negation and
backslash escapes are unsupported and rejected.  Input patterns are
repository-relative POSIX paths.  Changed-path files must be NUL-delimited
output that includes every affected path.  For a Git diff, generate them with
``git diff --name-only -z --no-renames`` so both the old and new side of a
rename are independently authorized.

Exit codes used by the CLI are stable: 0 for success, 2 for command-line
usage, 3 for invalid policy or input, 4 for authorization denial, and 5 for a
changed-path scope denial.  Every successful result is JSON on stdout; known
failures are JSON on stderr.
"""

from __future__ import annotations

import argparse
import datetime as dt
import functools
import json
import re
import sys
import unicodedata
from pathlib import Path
from typing import Any, Iterable, Mapping, Sequence


POLICY_VERSION = 1
DEFAULT_POLICY_PATH = Path(__file__).resolve().parents[1] / "policy.json"
MAX_POLICY_BYTES = 1024 * 1024
MAX_CHANGED_PATHS_BYTES = 8 * 1024 * 1024

CANONICAL_HARD_PROTECTED_PATTERNS = (
    ".github/**",
    ".codex/**",
    "AGENTS.md",
    "**/AGENTS.md",
    ".gitmodules",
    ".gitattributes",
    "docs/software/software-requirements-specification.md",
    "docs/software/architecture.md",
    "docs/software/autonomous-development.md",
    "benchmarks/manifest.json",
    "benchmarks/manifest.schema.json",
    "benchmarks/baselines/**",
    "benchmarks/results/baselines/**",
)

_POLICY_KEYS = {
    "$schema",
    "version",
    "limits",
    "hard_protected_patterns",
    "approved_requirements",
}
_LIMIT_KEYS = {"proposal_max_files", "review_max_files"}
_APPROVAL_KEYS = {
    "id",
    "slice",
    "approved_by",
    "approved_at",
    "source_sha",
    "approval_reference",
}
_REQUIREMENT_ID = re.compile(
    r"^[A-Z][A-Z0-9]*(?:-[A-Z0-9]+)+-[0-9]{3}$"
)
_DELIVERY_SLICE = re.compile(r"^S(?:[0-9]|1[0-4])$")
_SOURCE_SHA = re.compile(r"^(?:[0-9a-f]{40}|[0-9a-f]{64})$")
_RFC3339 = re.compile(
    r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}"
    r"(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$"
)


class CodexPolicyError(Exception):
    """Base class for expected, machine-readable policy failures."""

    exit_code = 3
    error_code = "invalid_input"

    def __init__(self, message: str, *, details: Any | None = None) -> None:
        super().__init__(message)
        self.details = details

    def as_dict(self) -> dict[str, Any]:
        error: dict[str, Any] = {
            "code": self.error_code,
            "message": str(self),
        }
        if self.details is not None:
            error["details"] = self.details
        return {"error": error, "status": "error"}


class PolicyValidationError(CodexPolicyError):
    """The trusted policy document is missing, malformed, or inconsistent."""

    error_code = "invalid_policy"


class InputValidationError(CodexPolicyError):
    """A CLI or changed-path input is malformed or unsafe."""

    error_code = "invalid_input"


class AuthorizationError(CodexPolicyError):
    """At least one requested requirement lacks the exact trusted approval."""

    exit_code = 4
    error_code = "authorization_denied"


class PathScopeError(CodexPolicyError):
    """Changed paths exceed the issue or repository policy scope."""

    exit_code = 5
    error_code = "path_scope_denied"


def _contains_control(value: str) -> bool:
    return any(
        unicodedata.category(character) in {"Cc", "Cf"} for character in value
    )


def _find_duplicates(values: Iterable[str]) -> list[str]:
    seen: set[str] = set()
    duplicates: set[str] = set()
    for value in values:
        if value in seen:
            duplicates.add(value)
        seen.add(value)
    return sorted(duplicates)


def _require_exact_keys(
    value: Mapping[str, Any], expected: set[str], context: str
) -> None:
    actual = set(value)
    missing = sorted(expected - actual)
    unexpected = sorted(repr(key) for key in actual - expected)
    if missing or unexpected:
        raise PolicyValidationError(
            f"{context} has an invalid field set",
            details={"missing": missing, "unexpected": unexpected},
        )


def _require_positive_integer(value: Any, context: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise PolicyValidationError(f"{context} must be an integer")
    if not 1 <= value <= 1000:
        raise PolicyValidationError(f"{context} must be between 1 and 1000")
    return value


def _require_identity_text(value: Any, context: str, maximum: int) -> str:
    if not isinstance(value, str):
        raise PolicyValidationError(f"{context} must be a string")
    if value == "" or value != value.strip():
        raise PolicyValidationError(
            f"{context} must be non-empty and have no surrounding whitespace"
        )
    if len(value) > maximum or _contains_control(value):
        raise PolicyValidationError(
            f"{context} is too long or contains control characters"
        )
    try:
        value.encode("utf-8", errors="strict")
    except UnicodeEncodeError as error:
        raise PolicyValidationError(f"{context} is not valid UTF-8 text") from error
    return value


def validate_glob_pattern(pattern: Any, *, context: str = "glob pattern") -> str:
    """Validate and return one repository-relative policy glob.

    ``**`` must occupy an entire path segment.  This removes ambiguous cases
    such as ``src**test`` while retaining conventional ``src/**/test?.rs``.
    """

    if not isinstance(pattern, str):
        raise InputValidationError(f"{context} must be a string")
    if pattern == "" or pattern != pattern.strip():
        raise InputValidationError(
            f"{context} must be non-empty and have no surrounding whitespace"
        )
    if len(pattern) > 4096:
        raise InputValidationError(f"{context} exceeds 4096 characters")
    if pattern.startswith("/") or pattern.startswith("!"):
        raise InputValidationError(
            f"{context} must be relative and cannot use negation: {pattern}"
        )
    if "\\" in pattern or _contains_control(pattern):
        raise InputValidationError(
            f"{context} cannot contain backslashes or control characters: {pattern}"
        )
    try:
        pattern.encode("utf-8", errors="strict")
    except UnicodeEncodeError as error:
        raise InputValidationError(f"{context} is not valid UTF-8 text") from error

    segments = pattern.split("/")
    if any(segment in {"", ".", ".."} for segment in segments):
        raise InputValidationError(
            f"{context} contains an empty, current, or parent segment: {pattern}"
        )
    for segment in segments:
        if "**" in segment and segment != "**":
            raise InputValidationError(
                f"{context} must use '**' as a complete segment: {pattern}"
            )
    return pattern


@functools.lru_cache(maxsize=512)
def compile_glob(pattern: str) -> re.Pattern[str]:
    """Compile a validated repository glob into a whole-path regular expression."""

    validate_glob_pattern(pattern)
    expression: list[str] = []
    index = 0
    while index < len(pattern):
        if pattern[index : index + 3] == "**/":
            expression.append("(?:.*/)?")
            index += 3
        elif pattern[index : index + 2] == "**":
            expression.append(".*")
            index += 2
        elif pattern[index] == "*":
            expression.append("[^/]*")
            index += 1
        elif pattern[index] == "?":
            expression.append("[^/]")
            index += 1
        else:
            expression.append(re.escape(pattern[index]))
            index += 1
    return re.compile("^" + "".join(expression) + "$")


def validate_repository_path(path: Any, *, context: str = "changed path") -> str:
    """Validate one exact, repository-relative POSIX path."""

    if not isinstance(path, str):
        raise InputValidationError(f"{context} must be a string")
    if path == "" or path != path.strip():
        raise InputValidationError(
            f"{context} must be non-empty and have no surrounding whitespace"
        )
    if len(path) > 4096:
        raise InputValidationError(f"{context} exceeds 4096 characters")
    if path.startswith("/") or "\\" in path:
        raise InputValidationError(
            f"{context} must be a repository-relative POSIX path: {path}"
        )
    if _contains_control(path) or "*" in path or "?" in path:
        raise InputValidationError(
            f"{context} contains control or wildcard characters: {path}"
        )
    try:
        path.encode("utf-8", errors="strict")
    except UnicodeEncodeError as error:
        raise InputValidationError(f"{context} is not valid UTF-8 text") from error

    segments = path.split("/")
    if any(segment in {"", ".", ".."} for segment in segments):
        raise InputValidationError(
            f"{context} contains an empty, current, or parent segment: {path}"
        )
    if segments[0] == ".git":
        raise InputValidationError(f"{context} targets repository metadata: {path}")
    return path


def glob_matches(pattern: str, path: str) -> bool:
    """Return whether a policy glob matches an entire validated path."""

    validate_repository_path(path)
    return compile_glob(pattern).fullmatch(path) is not None


def _normalize_patterns(
    patterns: Iterable[Any], *, context: str, require_nonempty: bool
) -> tuple[str, ...]:
    if isinstance(patterns, (str, bytes)):
        raise InputValidationError(f"{context} must be an array of glob strings")
    try:
        values = tuple(patterns)
    except TypeError as error:
        raise InputValidationError(f"{context} must be an array of glob strings") from error
    if require_nonempty and not values:
        raise InputValidationError(f"{context} must contain at least one glob")
    normalized = tuple(
        validate_glob_pattern(value, context=f"{context}[{index}]")
        for index, value in enumerate(values)
    )
    duplicates = _find_duplicates(normalized)
    if duplicates:
        raise InputValidationError(
            f"{context} contains duplicate globs", details=duplicates
        )
    return normalized


def _normalize_changed_paths(paths: Iterable[Any]) -> tuple[str, ...]:
    if isinstance(paths, (str, bytes)):
        raise InputValidationError("changed paths must be an array")
    try:
        values = tuple(paths)
    except TypeError as error:
        raise InputValidationError("changed paths must be an array") from error
    if not values:
        raise InputValidationError("changed paths must contain at least one path")
    normalized = tuple(
        validate_repository_path(value, context=f"changed path[{index}]")
        for index, value in enumerate(values)
    )
    duplicates = _find_duplicates(normalized)
    if duplicates:
        raise InputValidationError(
            "changed paths contain duplicate entries", details=duplicates
        )
    return normalized


def _duplicate_object_hook(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    duplicates: list[str] = []
    for key, item in pairs:
        if key in value:
            duplicates.append(key)
        value[key] = item
    if duplicates:
        raise PolicyValidationError(
            "policy JSON contains duplicate object keys",
            details=sorted(set(duplicates)),
        )
    return value


def validate_policy(policy: Any) -> Mapping[str, Any]:
    """Validate the complete trusted policy without third-party libraries."""

    if not isinstance(policy, Mapping):
        raise PolicyValidationError("policy root must be an object")
    _require_exact_keys(policy, _POLICY_KEYS, "policy")
    if policy["$schema"] != "./policy.schema.json":
        raise PolicyValidationError("policy $schema must be './policy.schema.json'")
    if isinstance(policy["version"], bool) or policy["version"] != POLICY_VERSION:
        raise PolicyValidationError(f"policy version must be {POLICY_VERSION}")

    limits = policy["limits"]
    if not isinstance(limits, Mapping):
        raise PolicyValidationError("policy limits must be an object")
    _require_exact_keys(limits, _LIMIT_KEYS, "policy limits")
    _require_positive_integer(limits["proposal_max_files"], "proposal_max_files")
    _require_positive_integer(limits["review_max_files"], "review_max_files")

    try:
        protected = _normalize_patterns(
            policy["hard_protected_patterns"],
            context="hard_protected_patterns",
            require_nonempty=True,
        )
    except InputValidationError as error:
        raise PolicyValidationError(str(error), details=error.details) from error
    missing_protected = [
        pattern
        for pattern in CANONICAL_HARD_PROTECTED_PATTERNS
        if pattern not in protected
    ]
    if missing_protected:
        raise PolicyValidationError(
            "policy omits canonical hard-protected patterns",
            details=missing_protected,
        )

    approvals = policy["approved_requirements"]
    if not isinstance(approvals, list):
        raise PolicyValidationError("approved_requirements must be an array")
    seen_ids: set[str] = set()
    for index, approval in enumerate(approvals):
        context = f"approved_requirements[{index}]"
        if not isinstance(approval, Mapping):
            raise PolicyValidationError(f"{context} must be an object")
        _require_exact_keys(approval, _APPROVAL_KEYS, context)

        requirement_id = approval["id"]
        if not isinstance(requirement_id, str) or not _REQUIREMENT_ID.fullmatch(
            requirement_id
        ):
            raise PolicyValidationError(f"{context}.id is not a valid requirement ID")
        if requirement_id in seen_ids:
            raise PolicyValidationError(
                "approved_requirements contains duplicate IDs",
                details=[requirement_id],
            )
        seen_ids.add(requirement_id)

        delivery_slice = approval["slice"]
        if not isinstance(delivery_slice, str) or not _DELIVERY_SLICE.fullmatch(
            delivery_slice
        ):
            raise PolicyValidationError(f"{context}.slice must be S0 through S14")
        _require_identity_text(approval["approved_by"], f"{context}.approved_by", 256)
        _require_identity_text(
            approval["approval_reference"], f"{context}.approval_reference", 2048
        )

        approved_at = approval["approved_at"]
        if not isinstance(approved_at, str) or not _RFC3339.fullmatch(approved_at):
            raise PolicyValidationError(f"{context}.approved_at must be RFC 3339")
        try:
            parsed = dt.datetime.fromisoformat(approved_at.replace("Z", "+00:00"))
        except ValueError as error:
            raise PolicyValidationError(
                f"{context}.approved_at is not a real timestamp"
            ) from error
        if parsed.tzinfo is None or parsed.utcoffset() is None:
            raise PolicyValidationError(f"{context}.approved_at must include an offset")

        source_sha = approval["source_sha"]
        if not isinstance(source_sha, str) or not _SOURCE_SHA.fullmatch(source_sha):
            raise PolicyValidationError(
                f"{context}.source_sha must be a lowercase 40- or 64-hex Git SHA"
            )
    return policy


def load_policy(path: str | Path = DEFAULT_POLICY_PATH) -> Mapping[str, Any]:
    """Load UTF-8 JSON, reject duplicate keys, and validate the policy."""

    policy_path = Path(path)
    try:
        raw = policy_path.read_bytes()
    except OSError as error:
        raise PolicyValidationError(f"cannot read policy {policy_path}: {error}") from error
    if len(raw) > MAX_POLICY_BYTES:
        raise PolicyValidationError(
            f"policy exceeds the {MAX_POLICY_BYTES}-byte limit"
        )
    try:
        text = raw.decode("utf-8", errors="strict")
    except UnicodeDecodeError as error:
        raise PolicyValidationError("policy is not valid UTF-8") from error
    try:
        policy = json.loads(text, object_pairs_hook=_duplicate_object_hook)
    except PolicyValidationError:
        raise
    except json.JSONDecodeError as error:
        raise PolicyValidationError(
            f"policy is not valid JSON at line {error.lineno}, column {error.colno}"
        ) from error
    return validate_policy(policy)


def authorize_requirements(
    policy: Mapping[str, Any], requirement_ids: Sequence[str], delivery_slice: str
) -> dict[str, Any]:
    """Authorize exact, case-sensitive requirement IDs for one delivery slice."""

    validate_policy(policy)
    if not isinstance(delivery_slice, str) or not _DELIVERY_SLICE.fullmatch(
        delivery_slice
    ):
        raise InputValidationError("delivery slice must be exactly S0 through S14")
    if isinstance(requirement_ids, (str, bytes)) or not requirement_ids:
        raise InputValidationError("at least one exact requirement ID is required")

    requested: list[str] = []
    for index, requirement_id in enumerate(requirement_ids):
        if not isinstance(requirement_id, str) or not _REQUIREMENT_ID.fullmatch(
            requirement_id
        ):
            raise InputValidationError(
                f"requirement_ids[{index}] is not a valid exact requirement ID"
            )
        requested.append(requirement_id)
    duplicates = _find_duplicates(requested)
    if duplicates:
        raise InputValidationError(
            "requested requirement IDs contain duplicates", details=duplicates
        )

    approvals = {
        approval["id"]: approval for approval in policy["approved_requirements"]
    }
    missing = [requirement_id for requirement_id in requested if requirement_id not in approvals]
    mismatched = [
        {
            "approved_slice": approvals[requirement_id]["slice"],
            "id": requirement_id,
            "requested_slice": delivery_slice,
        }
        for requirement_id in requested
        if requirement_id in approvals
        and approvals[requirement_id]["slice"] != delivery_slice
    ]
    if missing or mismatched:
        raise AuthorizationError(
            "requirements are not approved for the requested delivery slice",
            details={"missing": missing, "slice_mismatches": mismatched},
        )

    records = [approvals[requirement_id] for requirement_id in requested]
    return {
        "approval_records": records,
        "authorized": True,
        "delivery_slice": delivery_slice,
        "requirement_ids": requested,
        "status": "authorized",
    }


def read_nul_delimited_paths(path: str | Path) -> tuple[str, ...]:
    """Read a strictly terminated NUL path list and reject unsafe duplicates."""

    input_path = Path(path)
    try:
        raw = input_path.read_bytes()
    except OSError as error:
        raise InputValidationError(
            f"cannot read changed-path file {input_path}: {error}"
        ) from error
    if len(raw) > MAX_CHANGED_PATHS_BYTES:
        raise InputValidationError(
            f"changed-path file exceeds the {MAX_CHANGED_PATHS_BYTES}-byte limit"
        )
    if not raw:
        raise InputValidationError("changed-path file is empty")
    if not raw.endswith(b"\0"):
        raise InputValidationError("changed-path file must end with a NUL byte")

    records = raw[:-1].split(b"\0")
    if any(record == b"" for record in records):
        raise InputValidationError("changed-path file contains an empty record")
    decoded: list[str] = []
    for index, record in enumerate(records):
        try:
            decoded.append(record.decode("utf-8", errors="strict"))
        except UnicodeDecodeError as error:
            raise InputValidationError(
                f"changed path[{index}] is not valid UTF-8"
            ) from error
    return _normalize_changed_paths(decoded)


def _effective_max_files(configured: int, requested: int | None) -> int:
    if requested is None:
        return configured
    if isinstance(requested, bool) or not isinstance(requested, int) or requested < 1:
        raise InputValidationError("max_files must be a positive integer")
    return min(configured, requested)


def _matching_patterns(patterns: Sequence[str], path: str) -> list[str]:
    return [pattern for pattern in patterns if compile_glob(pattern).fullmatch(path)]


def validate_changed_paths(
    policy: Mapping[str, Any],
    changed_paths: Iterable[str],
    allowed_patterns: Iterable[str],
    issue_protected_patterns: Iterable[str] = (),
    *,
    max_files: int | None = None,
) -> dict[str, Any]:
    """Enforce allowlist, issue denylist, and canonical hard protections.

    All entries are evaluated independently.  Callers must supply both sides of
    renames (for Git, use ``--no-renames`` while producing the NUL path list).
    """

    validate_policy(policy)
    paths = _normalize_changed_paths(changed_paths)
    allowed = _normalize_patterns(
        allowed_patterns, context="allowed_patterns", require_nonempty=True
    )
    issue_protected = _normalize_patterns(
        issue_protected_patterns,
        context="issue_protected_patterns",
        require_nonempty=False,
    )
    hard_protected = tuple(policy["hard_protected_patterns"])
    configured_max = policy["limits"]["proposal_max_files"]
    effective_max = _effective_max_files(configured_max, max_files)

    violations: list[dict[str, Any]] = []
    if len(paths) > effective_max:
        violations.append(
            {
                "actual": len(paths),
                "kind": "max_files_exceeded",
                "maximum": effective_max,
            }
        )
    for changed_path in paths:
        allowed_matches = _matching_patterns(allowed, changed_path)
        issue_matches = _matching_patterns(issue_protected, changed_path)
        hard_matches = _matching_patterns(hard_protected, changed_path)
        if not allowed_matches:
            violations.append(
                {"kind": "outside_allowed_patterns", "path": changed_path}
            )
        if issue_matches:
            violations.append(
                {
                    "kind": "issue_protected_path",
                    "matched_patterns": issue_matches,
                    "path": changed_path,
                }
            )
        if hard_matches:
            violations.append(
                {
                    "kind": "hard_protected_path",
                    "matched_patterns": hard_matches,
                    "path": changed_path,
                }
            )
    if violations:
        raise PathScopeError(
            "changed paths violate the trusted repository or issue scope",
            details={
                "file_count": len(paths),
                "max_files": effective_max,
                "violations": violations,
            },
        )

    return {
        "allowed": True,
        "file_count": len(paths),
        "max_files": effective_max,
        "paths": list(paths),
        "status": "allowed",
    }


def classify_review_scope(
    policy: Mapping[str, Any],
    changed_paths: Iterable[str],
    *,
    max_files: int | None = None,
) -> dict[str, Any]:
    """Classify a PR as AI-review eligible or requiring human-only review."""

    validate_policy(policy)
    paths = _normalize_changed_paths(changed_paths)
    hard_protected = tuple(policy["hard_protected_patterns"])
    configured_max = policy["limits"]["review_max_files"]
    effective_max = _effective_max_files(configured_max, max_files)
    hard_matches = [
        {
            "matched_patterns": _matching_patterns(hard_protected, changed_path),
            "path": changed_path,
        }
        for changed_path in paths
    ]
    hard_matches = [item for item in hard_matches if item["matched_patterns"]]

    reasons: list[dict[str, Any]] = []
    if len(paths) > effective_max:
        reasons.append(
            {
                "actual": len(paths),
                "kind": "max_files_exceeded",
                "maximum": effective_max,
            }
        )
    if hard_matches:
        reasons.append(
            {"kind": "hard_protected_paths", "paths": hard_matches}
        )

    eligible = not reasons
    return {
        "classification": "ai_review_eligible"
        if eligible
        else "human_review_required",
        "eligible": eligible,
        "file_count": len(paths),
        "max_files": effective_max,
        "reasons": reasons,
        "status": "classified",
    }


def _parse_pattern_json(value: str, context: str) -> tuple[str, ...]:
    try:
        parsed = json.loads(value)
    except json.JSONDecodeError as error:
        raise InputValidationError(f"{context} is not valid JSON") from error
    if not isinstance(parsed, list):
        raise InputValidationError(f"{context} must be a JSON array")
    return tuple(parsed)


def _write_json(stream: Any, value: Mapping[str, Any]) -> None:
    json.dump(value, stream, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    stream.write("\n")


def _add_policy_argument(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "--policy",
        type=Path,
        default=DEFAULT_POLICY_PATH,
        help=f"trusted policy JSON (default: {DEFAULT_POLICY_PATH})",
    )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Validate CodeNoesis trusted Codex automation policy"
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    validate_parser = subparsers.add_parser(
        "validate-policy", help="validate the complete trusted policy"
    )
    _add_policy_argument(validate_parser)

    authorize_parser = subparsers.add_parser(
        "authorize", help="authorize exact requirement IDs for one slice"
    )
    _add_policy_argument(authorize_parser)
    authorize_parser.add_argument("--slice", required=True, dest="delivery_slice")
    authorize_parser.add_argument(
        "--requirement-id", action="append", required=True, dest="requirement_ids"
    )

    paths_parser = subparsers.add_parser(
        "validate-paths", help="validate a NUL-delimited changed-path list"
    )
    _add_policy_argument(paths_parser)
    paths_parser.add_argument(
        "--changed-paths-file", required=True, type=Path, dest="changed_paths_file"
    )
    paths_parser.add_argument(
        "--allowed-patterns-json",
        required=True,
        help="JSON array of issue allowlist globs",
    )
    paths_parser.add_argument(
        "--protected-patterns-json",
        default="[]",
        help="JSON array of issue-protected globs (default: [])",
    )
    paths_parser.add_argument(
        "--max-files",
        type=int,
        default=None,
        help="optional stricter cap; cannot raise the trusted policy limit",
    )

    review_parser = subparsers.add_parser(
        "classify-review", help="classify deterministic PR review scope"
    )
    _add_policy_argument(review_parser)
    review_parser.add_argument(
        "--changed-paths-file", required=True, type=Path, dest="changed_paths_file"
    )
    review_parser.add_argument(
        "--max-files",
        type=int,
        default=None,
        help="optional stricter cap; cannot raise the trusted policy limit",
    )
    return parser


def run_command(arguments: argparse.Namespace) -> dict[str, Any]:
    policy = load_policy(arguments.policy)
    if arguments.command == "validate-policy":
        return {
            "approved_requirement_count": len(policy["approved_requirements"]),
            "hard_protected_pattern_count": len(
                policy["hard_protected_patterns"]
            ),
            "status": "valid",
            "version": policy["version"],
        }
    if arguments.command == "authorize":
        return authorize_requirements(
            policy, arguments.requirement_ids, arguments.delivery_slice
        )
    if arguments.command == "validate-paths":
        changed_paths = read_nul_delimited_paths(arguments.changed_paths_file)
        allowed = _parse_pattern_json(
            arguments.allowed_patterns_json, "allowed_patterns_json"
        )
        protected = _parse_pattern_json(
            arguments.protected_patterns_json, "protected_patterns_json"
        )
        return validate_changed_paths(
            policy,
            changed_paths,
            allowed,
            protected,
            max_files=arguments.max_files,
        )
    if arguments.command == "classify-review":
        changed_paths = read_nul_delimited_paths(arguments.changed_paths_file)
        return classify_review_scope(
            policy, changed_paths, max_files=arguments.max_files
        )
    raise InputValidationError(f"unknown command: {arguments.command}")


def main(argv: Sequence[str] | None = None) -> int:
    parser = build_parser()
    arguments = parser.parse_args(argv)
    try:
        result = run_command(arguments)
    except CodexPolicyError as error:
        _write_json(sys.stderr, error.as_dict())
        return error.exit_code
    _write_json(sys.stdout, result)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
