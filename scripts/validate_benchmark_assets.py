#!/usr/bin/env python3
"""Validate benchmark metadata without third-party Python dependencies."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from typing import Any


EXPECTED_REPORT_FIELDS = {
    "cache_state",
    "concurrency",
    "corpus_version",
    "enabled_extractors",
    "host",
    "percentile_method",
    "repetitions",
    "success_rate",
}

EXPECTED_MANIFEST_FIELDS = {
    "$schema",
    "schema_version",
    "status",
    "requirements",
    "report_required_fields",
    "suites",
}

REQUIRED_SUITE_FIELDS = {
    "cache_state",
    "concurrency",
    "corpus",
    "description",
    "enabled_extractors",
    "host_profile",
    "id",
    "metrics",
    "minimum_success_rate",
    "percentile_method",
    "repetitions",
    "runner",
}

REQUIREMENT_ID = re.compile(r"^NFR-PER-[0-9]{3}$")
SUITE_ID = re.compile(r"^[a-z0-9][a-z0-9-]*$")


def load_json(path: Path) -> Any:
    """Load one JSON document and report its path on malformed input."""
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read {path}: {error}") from error


def validate_manifest_data(manifest: Any) -> list[str]:
    """Return all detected benchmark-manifest errors."""
    errors: list[str] = []
    if not isinstance(manifest, dict):
        return ["manifest must be a JSON object"]

    unexpected = manifest.keys() - EXPECTED_MANIFEST_FIELDS
    if unexpected:
        errors.append(f"manifest has unexpected fields: {', '.join(sorted(unexpected))}")

    if manifest.get("$schema") != "./manifest.schema.json":
        errors.append("$schema must reference ./manifest.schema.json")
    if manifest.get("schema_version") != 1:
        errors.append("schema_version must be 1")

    status = manifest.get("status")
    if status not in {"scaffold", "active"}:
        errors.append("status must be scaffold or active")

    requirements = manifest.get("requirements")
    if not isinstance(requirements, list) or not requirements:
        errors.append("requirements must be a non-empty list")
    else:
        if any(not isinstance(item, str) or not REQUIREMENT_ID.fullmatch(item) for item in requirements):
            errors.append("requirements must contain only NFR-PER requirement IDs")
        if len(set(requirements)) != len(requirements):
            errors.append("requirements must not contain duplicates")
        if not {"NFR-PER-001", "NFR-PER-002"}.issubset(set(requirements)):
            errors.append("requirements must include NFR-PER-001 and NFR-PER-002")

    report_fields = manifest.get("report_required_fields")
    if (
        not isinstance(report_fields, list)
        or len(report_fields) != len(EXPECTED_REPORT_FIELDS)
        or set(report_fields) != EXPECTED_REPORT_FIELDS
    ):
        errors.append("report_required_fields must contain the NFR-PER-001 evidence fields")

    suites = manifest.get("suites")
    if not isinstance(suites, list):
        errors.append("suites must be a list")
        return errors
    if status == "scaffold" and suites:
        errors.append("scaffold status cannot claim configured benchmark suites")
    if status == "active" and not suites:
        errors.append("active status requires at least one benchmark suite")
    if status == "active":
        errors.append(
            "active status is disabled until an executable runner validates corpus, samples, and base/head reports"
        )

    seen_ids: set[str] = set()
    for index, suite in enumerate(suites):
        prefix = f"suites[{index}]"
        if not isinstance(suite, dict):
            errors.append(f"{prefix} must be an object")
            continue
        missing = REQUIRED_SUITE_FIELDS - suite.keys()
        if missing:
            errors.append(f"{prefix} is missing: {', '.join(sorted(missing))}")
        unexpected = suite.keys() - REQUIRED_SUITE_FIELDS
        if unexpected:
            errors.append(f"{prefix} has unexpected fields: {', '.join(sorted(unexpected))}")
        suite_id = suite.get("id")
        if not isinstance(suite_id, str) or not SUITE_ID.fullmatch(suite_id):
            errors.append(f"{prefix}.id must be a lowercase kebab-case identifier")
        elif suite_id in seen_ids:
            errors.append(f"duplicate suite id: {suite_id}")
        else:
            seen_ids.add(suite_id)

        for field in ("description", "host_profile", "percentile_method"):
            if not isinstance(suite.get(field), str) or not suite[field].strip():
                errors.append(f"{prefix}.{field} must be a non-empty string")

        corpus = suite.get("corpus")
        if not isinstance(corpus, dict):
            errors.append(f"{prefix}.corpus must be an object")
        elif set(corpus) != {"id", "version"}:
            errors.append(f"{prefix}.corpus must contain exactly id and version")
        elif any(not isinstance(corpus[field], str) or not corpus[field].strip() for field in corpus):
            errors.append(f"{prefix}.corpus id and version must be non-empty strings")

        concurrency = suite.get("concurrency")
        if type(concurrency) is not int or concurrency < 1:
            errors.append(f"{prefix}.concurrency must be a positive integer")

        if suite.get("cache_state") not in {"cold", "warm", "mixed"}:
            errors.append(f"{prefix}.cache_state must be cold, warm, or mixed")

        for field in ("enabled_extractors", "metrics", "runner"):
            value = suite.get(field)
            if not isinstance(value, list) or not value:
                errors.append(f"{prefix}.{field} must be a non-empty list")
            elif any(not isinstance(item, str) or not item.strip() for item in value):
                errors.append(f"{prefix}.{field} must contain non-empty strings")
            elif field != "runner" and len(set(value)) != len(value):
                errors.append(f"{prefix}.{field} must not contain duplicates")

        repetitions = suite.get("repetitions")
        if type(repetitions) is not int or repetitions < 3:
            errors.append(f"{prefix}.repetitions must be an integer of at least 3")
        success_rate = suite.get("minimum_success_rate")
        if not isinstance(success_rate, (int, float)) or isinstance(success_rate, bool):
            errors.append(f"{prefix}.minimum_success_rate must be numeric")
        elif not 0 <= success_rate <= 1:
            errors.append(f"{prefix}.minimum_success_rate must be between 0 and 1")

    return errors


def validate_schema_data(schema: Any) -> list[str]:
    """Check that the committed schema preserves the mandatory contract."""
    if not isinstance(schema, dict):
        return ["schema must be a JSON object"]

    errors: list[str] = []
    if schema.get("type") != "object" or schema.get("additionalProperties") is not False:
        errors.append("schema root must be a closed object")
    top_level_required = set(schema.get("required", []))
    if not EXPECTED_MANIFEST_FIELDS.issubset(top_level_required):
        errors.append("schema does not require every top-level manifest field")

    suite_schema = schema.get("$defs", {}).get("suite", {})
    suite_required = set(suite_schema.get("required", []))
    if not REQUIRED_SUITE_FIELDS.issubset(suite_required):
        errors.append("schema does not require every suite reproducibility field")

    report_items = (
        schema.get("properties", {})
        .get("report_required_fields", {})
        .get("items", {})
        .get("enum", [])
    )
    if set(report_items) != EXPECTED_REPORT_FIELDS:
        errors.append("schema report fields do not match NFR-PER-001")
    status_schema = schema.get("properties", {}).get("status", {})
    if status_schema.get("type") != "string" or set(status_schema.get("enum", [])) != {
        "scaffold",
        "active",
    }:
        errors.append("schema status contract is invalid")
    return errors


def validate_assets(root: Path) -> tuple[list[str], dict[str, Any]]:
    """Validate the benchmark manifest and its local schema."""
    benchmark_dir = root / "benchmarks"
    manifest = load_json(benchmark_dir / "manifest.json")
    schema = load_json(benchmark_dir / "manifest.schema.json")
    errors = validate_manifest_data(manifest)
    errors.extend(validate_schema_data(schema))
    return errors, manifest


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    try:
        errors, manifest = validate_assets(root)
    except ValueError as error:
        print(error, file=sys.stderr)
        return 1

    if errors:
        print("benchmark assets are invalid:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print(
        "benchmark assets valid: "
        f"status={manifest['status']}, suites={len(manifest['suites'])}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
