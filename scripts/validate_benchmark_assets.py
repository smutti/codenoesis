#!/usr/bin/env python3
"""Validate benchmark metadata without third-party Python dependencies."""

from __future__ import annotations

import json
import re
import stat
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
ACTIVE_SUITE_ID = "rust-real-world-stability-v1"
ACTIVE_RUNNER = [
    "python3",
    "scripts/run_real_world_rust_benchmark.py",
    "run",
    "--manifest",
    "benchmarks/manifest.json",
    "--suite",
    ACTIVE_SUITE_ID,
]
ACTIVE_PATHS = {
    "corpus": "benchmarks/corpora/real-world-rust-stability-v1.json",
    "policy": "benchmarks/policies/real-world-rust-stability-v1.json",
    "oracle": "benchmarks/baselines/real-world-rust-stability-v1.json",
    "runner": "scripts/run_real_world_rust_benchmark.py",
}
CONFERENCE_SUITE_ID = "rust-public-conference-v1"
CONFERENCE_RUNNER = [
    "python3",
    "scripts/run_public_rust_evaluation.py",
    "run",
    "--manifest",
    "benchmarks/manifest.json",
    "--suite",
    CONFERENCE_SUITE_ID,
]
CONFERENCE_PATHS = {
    "corpus": "benchmarks/corpora/public-rust-conference-v1.json",
    "policy": "benchmarks/policies/public-rust-conference-v1.json",
    "oracle": "benchmarks/baselines/public-rust-conference-v1.json",
    "runner": "scripts/run_public_rust_evaluation.py",
}


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
        if "NFR-PER-001" not in requirements:
            errors.append("requirements must include NFR-PER-001")

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


def validate_regular_asset(root: Path, relative_path: str, label: str) -> tuple[list[str], Path]:
    """Reject missing, substituted, or non-regular active-suite assets."""
    path = root / relative_path
    try:
        metadata = path.lstat()
    except OSError:
        return [f"active {label} is missing: {relative_path}"], path
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
        return [f"active {label} must be a regular non-symlink file: {relative_path}"], path
    return [], path


def validate_active_suite_assets(root: Path, manifest: dict[str, Any]) -> list[str]:
    """Validate the exact bounded B1 observational suite and its known assets."""
    if manifest.get("status") != "active":
        return []

    errors: list[str] = []
    if manifest.get("requirements") != ["NFR-PER-001"]:
        errors.append("active B1 requirements must be exactly NFR-PER-001")
    if manifest.get("report_required_fields") != [
        "cache_state",
        "concurrency",
        "corpus_version",
        "enabled_extractors",
        "host",
        "percentile_method",
        "repetitions",
        "success_rate",
    ]:
        errors.append("active B1 report fields must retain canonical ordering")
    suites = manifest.get("suites")
    if (
        not isinstance(suites, list)
        or len(suites) != 2
        or any(not isinstance(suite, dict) for suite in suites)
    ):
        errors.append("active benchmark manifest must contain exactly B1 and conference suites")
        return errors
    suite = suites[0]
    expected_suite = {
        "id": ACTIVE_SUITE_ID,
        "description": "Same-host semantic and latency stability over pinned Lekton and RustDesk",
        "corpus": {"id": "real-world-rust-stability", "version": "1"},
        "host_profile": "declared-same-host-local-v1",
        "concurrency": 1,
        "cache_state": "mixed",
        "enabled_extractors": ["rust-r16-source-only"],
        "repetitions": 3,
        "percentile_method": "nearest-rank",
        "minimum_success_rate": 1.0,
        "metrics": [
            "wall_time_ns",
            "exit_code",
            "stdout_bytes",
            "stderr_bytes",
            "semantic_hash",
            "semantic_projection_sha256",
        ],
        "runner": ACTIVE_RUNNER,
    }
    if suite != expected_suite:
        errors.append("active B1 suite does not match the fixed observational contract")
    conference_suite = suites[1]
    expected_conference_suite = {
        "id": CONFERENCE_SUITE_ID,
        "description": "Progressive compatibility and ontology-information evaluation over eight pinned public Rust repositories",
        "corpus": {"id": "public-rust-conference", "version": "1"},
        "host_profile": "declared-same-host-local-v1",
        "concurrency": 1,
        "cache_state": "cold",
        "enabled_extractors": ["rust-progressive-s1-r16-source-only"],
        "repetitions": 3,
        "percentile_method": "nearest-rank",
        "minimum_success_rate": 1.0,
        "metrics": [
            "stage_coverage_basis_points",
            "wall_time_ns",
            "exit_code",
            "stdout_bytes",
            "stderr_bytes",
            "semantic_hash",
            "semantic_projection_sha256",
            "graph_counts",
            "ontology_information",
        ],
        "runner": CONFERENCE_RUNNER,
    }
    if conference_suite != expected_conference_suite:
        errors.append("conference suite does not match the fixed evaluation contract")

    loaded: dict[str, Any] = {}
    for label, relative_path in ACTIVE_PATHS.items():
        asset_errors, path = validate_regular_asset(root, relative_path, label)
        errors.extend(asset_errors)
        if not asset_errors and label != "runner":
            try:
                loaded[label] = load_json(path)
            except ValueError as error:
                errors.append(str(error))
    if errors:
        return errors

    corpus = loaded["corpus"]
    if not isinstance(corpus, dict):
        errors.append("active B1 corpus must be an object")
    else:
        if set(corpus) != {
            "schema_version",
            "id",
            "version",
            "source_mode",
            "network_allowed",
            "source_vendored",
            "entries",
        }:
            errors.append("active B1 corpus fields do not match the contract")
        if (
            corpus.get("schema_version") != "codenoesis.real-world-rust-benchmark-corpus/v1"
            or corpus.get("id") != "real-world-rust-stability"
            or corpus.get("version") != "1"
            or corpus.get("source_mode")
            != "caller_supplied_full_non_shallow_local_clone"
            or corpus.get("network_allowed") is not False
            or corpus.get("source_vendored") is not False
        ):
            errors.append("active B1 corpus identity or source policy is invalid")
        entries = corpus.get("entries")
        expected_entry_identities = [
            (
                "lekton",
                "https://github.com/dghilardi/lekton.git",
                "247b8f42fb045db41166d70a276a41c2e079b6eb",
                "55ba428493a4ffae86ba492422a049f46d567a30",
                "success",
            ),
            (
                "rustdesk",
                "https://github.com/rustdesk/rustdesk.git",
                "d412d198720aa56f6cfed2dfad262e8fb1322fb7",
                "df8d4c292c9d256a445480eb878e507df3de1dc4",
                "typed_rejection",
            ),
        ]
        expected_profiles = [
            [
                "local-git-sha1-packed-v1",
                "cargo-root-package-v1",
                "cargo-manifest-facts-v1",
                "rust-semantic-depth-v1",
                "rust-framework-declarations-v1",
                "rust-callable-semantics-v1",
                "rust-expression-bindings-v1",
                "rust-local-flow-v1",
                "rust-safe-constant-evaluation-v1",
                "local-snapshot-256m-v1",
                "real-world-rust-benchmark-75s-v1",
            ],
            [
                "local-git-sha1-packed-v1",
                "local-gitlinks-v1",
                "cargo-root-package-v1",
                "cargo-manifest-facts-v1",
                "rust-semantic-depth-v1",
                "rust-framework-declarations-v1",
                "rust-callable-semantics-v1",
                "rust-expression-bindings-v1",
                "rust-local-flow-v1",
                "rust-safe-constant-evaluation-v1",
                "local-snapshot-256m-v1",
                "real-world-rust-benchmark-75s-v1",
            ],
        ]
        if not isinstance(entries, list) or len(entries) != 2:
            errors.append("active B1 corpus must contain exactly two entries")
        else:
            observed_identities = [
                (
                    entry.get("id"),
                    entry.get("repository_url"),
                    entry.get("revision"),
                    entry.get("tree"),
                    entry.get("outcome"),
                )
                for entry in entries
                if isinstance(entry, dict)
            ]
            if observed_identities != expected_entry_identities:
                errors.append("active B1 corpus revisions, trees, ordering, or outcomes changed")
            if [entry.get("profiles") for entry in entries if isinstance(entry, dict)] != (
                expected_profiles
            ):
                errors.append("active B1 corpus profile matrix changed")
            if any(
                not isinstance(entry, dict)
                or set(entry)
                != {
                    "id",
                    "repository_url",
                    "revision",
                    "tree",
                    "observed_license",
                    "repository_id",
                    "outcome",
                    "profiles",
                }
                for entry in entries
            ):
                errors.append("active B1 corpus entry fields do not match the contract")

    policy = loaded["policy"]
    expected_policy = {
        "schema_version": "codenoesis.real-world-rust-benchmark-policy/v1",
        "suite_id": ACTIVE_SUITE_ID,
        "claim_scope": "observational_same_host_regression_only",
        "requirements": ["NFR-PER-001"],
        "repetitions": 3,
        "concurrency": 1,
        "cache_state": "mixed",
        "percentile_method": "nearest-rank",
        "minimum_success_rate": 1.0,
        "candidate_p95_ratio_max": 1.2,
        "candidate_p95_additive_ns_max": 5_000_000_000,
        "absolute_p95_ns_max": {"lekton": 75_000_000_000, "rustdesk": 10_000_000_000},
        "timeout_seconds": {"lekton": 90, "rustdesk": 15},
        "report_bytes_max": 1_048_576,
        "semantic_and_outcome_identity_required": True,
        "cross_host_comparison_allowed": False,
        "failed_sample_retry_allowed": False,
        "nfr_per_002_claimed": False,
        "slo_claimed": False,
        "release_claimed": False,
        "ga_claimed": False,
    }
    if policy != expected_policy:
        errors.append("active B1 policy does not match the bounded observational contract")

    oracle = loaded["oracle"]
    if not isinstance(oracle, dict):
        errors.append("active B1 oracle must be an object")
    else:
        if set(oracle) != {
            "schema_version",
            "suite_id",
            "baseline_product_commit",
            "entries",
            "historical_observation",
        }:
            errors.append("active B1 oracle fields do not match the contract")
        if (
            oracle.get("schema_version") != "codenoesis.real-world-rust-benchmark-oracle/v1"
            or oracle.get("suite_id") != ACTIVE_SUITE_ID
            or oracle.get("baseline_product_commit")
            != "cce84869430ef129f55591998b30ea2ea728e1c3"
        ):
            errors.append("active B1 oracle identity is invalid")
        entries = oracle.get("entries")
        expected_lekton = {
            "outcome": "success",
            "snapshot_schema": "codenoesis.repository-snapshot/v18",
            "semantic_hash": "22e32d20429d510d4674e0e6bdc5542f08dbc0e28874cd0098419e7512a334c1",
            "semantic_projection_sha256": "7c800424b3176c96d4ea4164d4066adaf551134b3aea4b40a1e5647f74dc7fa9",
            "counts": {
                "entities": 26_158,
                "relationships": 43_683,
                "claims": 69_841,
                "evidence": 24_522,
                "diagnostics": 4_211,
                "coverage": 13_177,
                "evaluated_values": 8,
            },
        }
        expected_rustdesk = {
            "outcome": "typed_rejection",
            "exit_code": 2,
            "stdout_bytes": 0,
            "error_schema": "codenoesis.error/v24",
            "error_code": "input.unsupported_rust_constant_evaluation_composition",
            "error_stage": "input",
            "error_reason": "repository_boundary_not_supported",
            "store_created": False,
            "nested_source_read": False,
        }
        if not isinstance(entries, dict) or entries.get("lekton") != expected_lekton:
            errors.append("active B1 Lekton semantic oracle changed")
        if not isinstance(entries, dict) or entries.get("rustdesk") != expected_rustdesk:
            errors.append("active B1 RustDesk typed oracle changed")

    conference_loaded: dict[str, Any] = {}
    for label, relative_path in CONFERENCE_PATHS.items():
        asset_errors, path = validate_regular_asset(root, relative_path, f"conference {label}")
        errors.extend(asset_errors)
        if not asset_errors and label != "runner":
            conference_loaded[label] = path
    if not errors:
        try:
            from run_public_rust_evaluation import load_contracts

            load_contracts(
                conference_loaded["corpus"],
                conference_loaded["policy"],
                conference_loaded["oracle"],
            )
        except Exception as error:
            errors.append(f"conference evaluation contract is invalid: {error}")
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
    if isinstance(manifest, dict):
        errors.extend(validate_active_suite_assets(root, manifest))
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
