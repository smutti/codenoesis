#!/usr/bin/env python3
"""Verify the issue #188 local baseline evidence pack without dependencies."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path
from typing import Any, Iterable


ISSUE = "https://github.com/smutti/codenoesis/issues/188"
BASE_SHA = "9ecdc3acefd43495daf76b9f2ab69a7bbacff172"
CHECKPOINT_SHA = "1daa23ffbaa0ea13f5fc5910aa5798c7523f254c"
RED_COMMIT_SHA = "6fef76a4ba7fff8435d85a6df4c6efb3b1d1991b"
STATUS = "candidate_verified_pending_merge"
STATUS_MARKER = (
    "LocalBaselineVerificationV2 candidate Verified pending independent review "
    "and protected manual merge"
)
SCHEMA_VERSION = "codenoesis.local-baseline-verification/v2"
PLAN_PATH = Path("tests/specifications/verification/local-baseline-v2/plan.json")
CATALOG_PATH = Path(
    "tests/specifications/verification/local-baseline-v2/profile-catalog.json"
)
REMOTE_RUNS_PATH = Path(
    "tests/evidence/verification/local-baseline-v2/remote-runs.json"
)
SHA256 = re.compile(r"^[0-9a-f]{64}$")
GIT_SHA1 = re.compile(r"^[0-9a-f]{40}$")

MANIFEST_FIELDS = {
    "schema_version",
    "issue",
    "base_sha",
    "governance_checkpoint_sha",
    "evidence_parent_sha",
    "verification_subject",
    "product_tree_sha256",
    "status",
    "plan",
    "profile_catalog",
    "profiles",
    "repository_evidence",
    "remote_logs",
    "remote_runs",
    "required_gates",
    "environment",
    "limitations",
    "review",
}

ALLOWED_EXACT_PATHS = {
    "README.md",
    "docs/software/software-requirements-specification.md",
    "docs/software/roadmap.md",
    "docs/software/verification.md",
    "docs/software/decisions/0037-local-baseline-verification-v2.md",
    "tests/specifications/s0/evidence-manifest-v1.schema.json",
    "scripts/verify_local_baseline_v2.py",
    "scripts/tests/test_local_baseline_verification_v2.py",
}
ALLOWED_PREFIXES = (
    "tests/specifications/verification/local-baseline-v2/",
    "tests/evidence/verification/local-baseline-v2/",
)

EXPECTED_REMOTE_LOG_IDS = {
    "base-benchmark-validation",
    "base-ci-linux",
    "base-ci-macos",
    "base-ci-quality",
    "base-ci-supply-chain",
    "base-ci-windows",
    "base-release-attestation",
    "base-release-supply",
    "s0-pr15-quality",
}
EXPECTED_REMOTE_RUN_IDS = {
    29955409362,
    31842394592,
    31842394863,
    31842394871,
    31843845301,
}
EXPECTED_GATE_IDS = {
    "benchmark-integrity",
    "browser",
    "codeql",
    "expected-red",
    "focused-green",
    "full-regression",
    "g8-attestation",
    "governance-checkpoint",
    "independent-review-activation",
    "linux-native",
    "macos-native",
    "policy-gate",
    "product-tree",
    "profile-catalog",
    "real-repository-pilot",
    "security",
    "status-parity",
    "supply-chain",
    "traceability",
    "windows-native",
}


def load_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read JSON {path}: {error}") from error


def canonical_json(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def git(root: Path, arguments: Iterable[str], *, binary: bool = False) -> Any:
    command = ["git", *arguments]
    completed = subprocess.run(
        command,
        cwd=root,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if completed.returncode != 0:
        diagnostic = completed.stderr.decode("utf-8", errors="replace").strip()
        raise ValueError(f"{' '.join(command)} failed: {diagnostic}")
    if binary:
        return completed.stdout
    return completed.stdout.decode("utf-8").strip()


def git_is_ancestor(root: Path, ancestor: str, descendant: str) -> bool:
    completed = subprocess.run(
        ["git", "merge-base", "--is-ancestor", ancestor, descendant],
        cwd=root,
        check=False,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    return completed.returncode == 0


def safe_relative_path(value: Any) -> bool:
    if not isinstance(value, str) or not value or "\x00" in value:
        return False
    path = Path(value)
    return not path.is_absolute() and ".." not in path.parts


def is_excluded(path: str, exclusions: list[str]) -> bool:
    return any(path == exclusion or path.startswith(f"{exclusion}/") for exclusion in exclusions)


def product_tree_digest(root: Path, revision: str, exclusions: list[str]) -> str:
    raw = git(root, ["ls-tree", "-r", "-z", "--full-tree", revision], binary=True)
    records: list[tuple[str, str, str]] = []
    for entry in raw.split(b"\0"):
        if not entry:
            continue
        metadata, raw_path = entry.split(b"\t", 1)
        mode, object_type, object_id = metadata.decode("ascii").split(" ", 2)
        path = raw_path.decode("utf-8")
        if object_type != "blob" or is_excluded(path, exclusions):
            continue
        records.append((path, mode, object_id))
    payload = b"".join(
        f"{mode}\0{object_id}\0{path}\n".encode("utf-8")
        for path, mode, object_id in sorted(records)
    )
    return sha256_bytes(payload)


def validate_path_digest(
    root: Path,
    value: Any,
    label: str,
    errors: list[str],
) -> None:
    if not isinstance(value, dict) or set(value) != {"path", "sha256"}:
        errors.append(f"{label} must contain exactly path and sha256")
        return
    relative_path = value.get("path")
    expected_sha256 = value.get("sha256")
    if not safe_relative_path(relative_path):
        errors.append(f"{label}.path is unsafe")
        return
    if not isinstance(expected_sha256, str) or not SHA256.fullmatch(expected_sha256):
        errors.append(f"{label}.sha256 is invalid")
        return
    path = root / relative_path
    if not path.is_file():
        errors.append(f"{label} path is missing: {relative_path}")
        return
    actual_sha256 = sha256_file(path)
    if actual_sha256 != expected_sha256:
        errors.append(
            f"{label} digest mismatch for {relative_path}: "
            f"expected {expected_sha256}, observed {actual_sha256}"
        )


def validate_authority(
    root: Path,
    manifest: dict[str, Any],
    plan: dict[str, Any],
    errors: list[str],
) -> None:
    constants = {
        "schema_version": SCHEMA_VERSION,
        "issue": ISSUE,
        "base_sha": BASE_SHA,
        "governance_checkpoint_sha": CHECKPOINT_SHA,
        "verification_subject": BASE_SHA,
        "status": STATUS,
    }
    for field, expected in constants.items():
        if manifest.get(field) != expected:
            errors.append(f"{field} must equal {expected}")

    evidence_parent = manifest.get("evidence_parent_sha")
    if not isinstance(evidence_parent, str) or not GIT_SHA1.fullmatch(evidence_parent):
        errors.append("evidence_parent_sha is invalid")
        return

    head = git(root, ["rev-parse", "HEAD"])
    for revision in (BASE_SHA, CHECKPOINT_SHA, RED_COMMIT_SHA, evidence_parent, head):
        try:
            git(root, ["cat-file", "-e", f"{revision}^{{commit}}"])
        except ValueError as error:
            errors.append(str(error))

    ancestry = (
        (BASE_SHA, CHECKPOINT_SHA, "base is not an ancestor of checkpoint"),
        (CHECKPOINT_SHA, RED_COMMIT_SHA, "checkpoint is not an ancestor of Red"),
        (RED_COMMIT_SHA, evidence_parent, "Red is not an ancestor of evidence parent"),
        (evidence_parent, head, "evidence parent is not an ancestor of current head"),
    )
    for ancestor, descendant, message in ancestry:
        if not git_is_ancestor(root, ancestor, descendant):
            errors.append(message)

    if plan.get("base_sha") != BASE_SHA or plan.get("issue") != ISSUE:
        errors.append("plan authority does not match issue #188")
    if plan.get("delivery_slice") != "S14" or plan.get("risk") != "high":
        errors.append("plan must remain high-risk S14")
    for field in (
        "runtime_changes_allowed",
        "control_plane_changes_allowed",
        "release_authority_allowed",
        "new_dependencies_allowed",
        "partial_verification_allowed",
    ):
        if plan.get(field) is not False:
            errors.append(f"plan {field} must remain false")


def validate_changed_paths(root: Path, errors: list[str]) -> None:
    changed = git(root, ["diff", "--name-only", f"{BASE_SHA}..HEAD"])
    changed_paths = [path for path in changed.splitlines() if path]
    for path in changed_paths:
        if path in ALLOWED_EXACT_PATHS or path.startswith(ALLOWED_PREFIXES):
            continue
        errors.append(f"changed path is outside issue #188 authority: {path}")


def validate_product_tree(
    root: Path,
    manifest: dict[str, Any],
    plan: dict[str, Any],
    errors: list[str],
) -> None:
    exclusions = plan.get("product_tree_exclusions")
    if not isinstance(exclusions, list) or not exclusions:
        errors.append("plan product_tree_exclusions must be non-empty")
        return
    if any(not safe_relative_path(path) for path in exclusions):
        errors.append("plan product_tree_exclusions contains an unsafe path")
        return
    base_digest = product_tree_digest(root, BASE_SHA, exclusions)
    head_digest = product_tree_digest(root, "HEAD", exclusions)
    expected = manifest.get("product_tree_sha256")
    if expected != base_digest:
        errors.append(
            f"product_tree_sha256 must equal base product tree {base_digest}"
        )
    if head_digest != base_digest:
        errors.append(
            f"verification package changed product tree: {base_digest} != {head_digest}"
        )


def validate_catalog(
    root: Path,
    manifest: dict[str, Any],
    plan: dict[str, Any],
    catalog: dict[str, Any],
    errors: list[str],
) -> None:
    required_ids = plan.get("required_profile_ids")
    required_classes = plan.get("required_evidence_classes")
    catalog_profiles = catalog.get("profiles")
    if not isinstance(required_ids, list) or len(required_ids) != len(set(required_ids)):
        errors.append("plan required_profile_ids must be unique")
        return
    if not isinstance(required_classes, list) or len(required_classes) != len(set(required_classes)):
        errors.append("plan required_evidence_classes must be unique")
        return
    if not isinstance(catalog_profiles, list):
        errors.append("profile catalog profiles must be a list")
        return
    catalog_ids = [profile.get("id") for profile in catalog_profiles if isinstance(profile, dict)]
    if catalog_ids != required_ids:
        errors.append("profile catalog order or scope differs from plan")
        return

    for profile in catalog_profiles:
        profile_id = profile["id"]
        requirements = profile.get("requirements")
        implementation_prs = profile.get("implementation_prs")
        if not isinstance(requirements, list) or requirements != sorted(set(requirements)):
            errors.append(f"catalog {profile_id} requirements are not sorted and unique")
        if not isinstance(implementation_prs, list) or implementation_prs != sorted(
            set(implementation_prs)
        ):
            errors.append(f"catalog {profile_id} implementation PRs are not sorted and unique")
        for field in ("oracle_paths", "evidence_paths"):
            paths = profile.get(field)
            if not isinstance(paths, list) or not paths:
                errors.append(f"catalog {profile_id} has no {field}")
                continue
            for relative_path in paths:
                if not safe_relative_path(relative_path):
                    errors.append(f"catalog {profile_id} has unsafe {field} path")
                elif not (root / relative_path).is_file():
                    errors.append(f"catalog {profile_id} is missing {field}: {relative_path}")

    results = manifest.get("profiles")
    if not isinstance(results, list):
        errors.append("manifest profiles must be a list")
        return
    result_ids = [profile.get("id") for profile in results if isinstance(profile, dict)]
    if result_ids != required_ids:
        errors.append("manifest profile order or scope differs from plan")
        return

    catalog_by_id = {profile["id"]: profile for profile in catalog_profiles}
    repository_ids = {
        item.get("id")
        for item in manifest.get("repository_evidence", [])
        if isinstance(item, dict)
    }
    remote_log_ids = {
        item.get("id")
        for item in manifest.get("remote_logs", [])
        if isinstance(item, dict)
    }
    remote_run_ids = {
        f"run:{item.get('run_id')}"
        for item in manifest.get("remote_runs", [])
        if isinstance(item, dict)
    }

    for result in results:
        profile_id = result["id"]
        if set(result) != {
            "id",
            "catalog_entry_sha256",
            "evidence_classes",
            "result",
        }:
            errors.append(f"manifest profile {profile_id} has unexpected fields")
            continue
        expected_catalog_digest = sha256_bytes(canonical_json(catalog_by_id[profile_id]))
        if result.get("catalog_entry_sha256") != expected_catalog_digest:
            errors.append(f"manifest profile {profile_id} catalog digest mismatch")
        if result.get("result") != "green":
            errors.append(f"manifest profile {profile_id} result must be green")
        evidence_classes = result.get("evidence_classes")
        if not isinstance(evidence_classes, list):
            errors.append(f"manifest profile {profile_id} evidence classes must be a list")
            continue
        class_ids = [item.get("id") for item in evidence_classes if isinstance(item, dict)]
        if class_ids != required_classes:
            errors.append(f"manifest profile {profile_id} evidence classes differ from plan")
            continue
        for evidence_class in evidence_classes:
            if set(evidence_class) != {"id", "result", "evidence", "rationale"}:
                errors.append(
                    f"manifest profile {profile_id} evidence class has unexpected fields"
                )
                continue
            result_value = evidence_class.get("result")
            evidence = evidence_class.get("evidence")
            rationale = evidence_class.get("rationale")
            if result_value not in {"green", "not_applicable"}:
                errors.append(
                    f"manifest profile {profile_id} evidence class result is invalid"
                )
            if not isinstance(evidence, list) or any(
                not isinstance(reference, str) or not reference for reference in evidence
            ):
                errors.append(
                    f"manifest profile {profile_id} evidence references are invalid"
                )
                continue
            if result_value == "green" and not evidence:
                errors.append(
                    f"manifest profile {profile_id} green evidence class is empty"
                )
            if not isinstance(rationale, str) or not rationale:
                errors.append(
                    f"manifest profile {profile_id} evidence rationale is empty"
                )
            known_references = repository_ids | remote_log_ids | remote_run_ids | {
                f"catalog:{profile_id}:oracle",
                f"catalog:{profile_id}:evidence",
            }
            for reference in evidence:
                if reference not in known_references:
                    errors.append(
                        f"manifest profile {profile_id} has dangling evidence reference: "
                        f"{reference}"
                    )


def validate_repository_evidence(
    root: Path,
    manifest: dict[str, Any],
    errors: list[str],
) -> None:
    evidence = manifest.get("repository_evidence")
    if not isinstance(evidence, list) or not evidence:
        errors.append("repository_evidence must be a non-empty list")
        return
    ids: list[Any] = []
    paths: list[Any] = []
    for index, item in enumerate(evidence):
        label = f"repository_evidence[{index}]"
        if not isinstance(item, dict) or set(item) != {"id", "path", "sha256", "kind"}:
            errors.append(f"{label} has unexpected fields")
            continue
        ids.append(item.get("id"))
        paths.append(item.get("path"))
        validate_path_digest(
            root,
            {"path": item.get("path"), "sha256": item.get("sha256")},
            label,
            errors,
        )
    if len(ids) != len(set(ids)):
        errors.append("repository_evidence IDs must be unique")
    if len(paths) != len(set(paths)):
        errors.append("repository_evidence paths must be unique")


def validate_remote_runs(
    root: Path,
    manifest: dict[str, Any],
    errors: list[str],
) -> dict[int, dict[str, Any]]:
    metadata = load_json(root / REMOTE_RUNS_PATH)
    metadata_runs = metadata.get("runs")
    if not isinstance(metadata_runs, list):
        errors.append("remote-runs.json must contain runs")
        return {}
    metadata_by_id = {item["run_id"]: item for item in metadata_runs}
    if set(metadata_by_id) != EXPECTED_REMOTE_RUN_IDS:
        errors.append("remote-runs.json does not contain the exact required run set")

    runs = manifest.get("remote_runs")
    if not isinstance(runs, list):
        errors.append("manifest remote_runs must be a list")
        return metadata_by_id
    run_ids = [item.get("run_id") for item in runs if isinstance(item, dict)]
    if set(run_ids) != EXPECTED_REMOTE_RUN_IDS or len(run_ids) != len(set(run_ids)):
        errors.append("manifest remote_runs does not contain the exact required run set")
        return metadata_by_id
    for item in runs:
        run_id = item["run_id"]
        expected = metadata_by_id[run_id]
        compared = {
            "run_id": expected["run_id"],
            "run_attempt": expected["run_attempt"],
            "head_sha": expected["head_sha"],
            "workflow": expected["workflow"],
            "workflow_path": expected["workflow_path"],
            "event": expected["event"],
            "url": expected["url"],
            "conclusion": expected["conclusion"],
        }
        if item != compared:
            errors.append(f"manifest remote run {run_id} differs from retained metadata")
    return metadata_by_id


def validate_remote_logs(
    root: Path,
    manifest: dict[str, Any],
    metadata_by_id: dict[int, dict[str, Any]],
    errors: list[str],
) -> None:
    logs = manifest.get("remote_logs")
    if not isinstance(logs, list):
        errors.append("manifest remote_logs must be a list")
        return
    log_ids = [item.get("id") for item in logs if isinstance(item, dict)]
    if set(log_ids) != EXPECTED_REMOTE_LOG_IDS or len(log_ids) != len(set(log_ids)):
        errors.append("manifest remote_logs does not contain the exact retained log set")
        return

    evidence_parent = manifest.get("evidence_parent_sha")
    required_fields = {
        "id",
        "provider",
        "format",
        "repository",
        "workflow_path",
        "workflow_sha256",
        "workflow_ref",
        "run_id",
        "run_attempt",
        "job_id",
        "job_name",
        "check_name",
        "head_sha",
        "tree_sha",
        "conclusion",
        "base_controlled",
        "source_log_sha256",
        "normalization",
        "committed_log_path",
        "committed_log_sha256",
        "first_retained_commit",
        "evidence_head",
    }
    for item in logs:
        log_id = item["id"]
        if set(item) != required_fields:
            errors.append(f"remote log {log_id} has unexpected fields")
            continue
        if item.get("provider") != "github_actions":
            errors.append(f"remote log {log_id} provider is invalid")
        if item.get("format") != "git-retained-github-actions-log/v1":
            errors.append(f"remote log {log_id} format is invalid")
        if item.get("repository") != "smutti/codenoesis":
            errors.append(f"remote log {log_id} repository is invalid")
        if item.get("conclusion") != "success" or item.get("base_controlled") is not True:
            errors.append(f"remote log {log_id} is not successful base-controlled evidence")
        if item.get("normalization") != "codenoesis.github-actions-log-normalization/v1":
            errors.append(f"remote log {log_id} normalization is invalid")
        if item.get("first_retained_commit") != evidence_parent:
            errors.append(f"remote log {log_id} first retention commit is invalid")
        if item.get("evidence_head") != evidence_parent:
            errors.append(f"remote log {log_id} evidence head is invalid")

        run_id = item.get("run_id")
        run = metadata_by_id.get(run_id)
        if run is None:
            errors.append(f"remote log {log_id} references unknown run {run_id}")
            continue
        if item.get("run_attempt") != run.get("run_attempt"):
            errors.append(f"remote log {log_id} run attempt mismatch")
        if item.get("head_sha") != run.get("head_sha"):
            errors.append(f"remote log {log_id} head mismatch")
        if item.get("tree_sha") != run.get("tree_sha"):
            errors.append(f"remote log {log_id} tree mismatch")
        jobs = {job["job_id"]: job for job in run.get("jobs", [])}
        job = jobs.get(item.get("job_id"))
        if job is None:
            errors.append(f"remote log {log_id} references unknown job")
        elif item.get("job_name") != job.get("name") or job.get("conclusion") != "success":
            errors.append(f"remote log {log_id} job metadata mismatch")

        relative_path = item.get("committed_log_path")
        if not safe_relative_path(relative_path):
            errors.append(f"remote log {log_id} committed path is unsafe")
            continue
        committed_path = root / relative_path
        if not committed_path.is_file():
            errors.append(f"remote log {log_id} committed file is missing")
            continue
        committed_digest = sha256_file(committed_path)
        if item.get("committed_log_sha256") != committed_digest:
            errors.append(f"remote log {log_id} committed digest mismatch")
        if item.get("source_log_sha256") != committed_digest:
            errors.append(f"remote log {log_id} source and identity normalization differ")

        workflow_path = item.get("workflow_path")
        if workflow_path != run.get("workflow_path"):
            errors.append(f"remote log {log_id} workflow path mismatch")
        if isinstance(workflow_path, str) and workflow_path.startswith(".github/workflows/"):
            try:
                workflow_bytes = git(
                    root,
                    ["show", f"{item['head_sha']}:{workflow_path}"],
                    binary=True,
                )
            except ValueError as error:
                errors.append(str(error))
            else:
                workflow_digest = sha256_bytes(workflow_bytes)
                if item.get("workflow_sha256") != workflow_digest:
                    errors.append(f"remote log {log_id} workflow digest mismatch")
        actual_tree = git(root, ["rev-parse", f"{item['head_sha']}^{{tree}}"])
        if item.get("tree_sha") != actual_tree:
            errors.append(f"remote log {log_id} Git tree mismatch")


def validate_gates(manifest: dict[str, Any], errors: list[str]) -> None:
    gates = manifest.get("required_gates")
    if not isinstance(gates, list):
        errors.append("required_gates must be a list")
        return
    gate_ids = [gate.get("id") for gate in gates if isinstance(gate, dict)]
    if set(gate_ids) != EXPECTED_GATE_IDS or len(gate_ids) != len(set(gate_ids)):
        errors.append("required_gates does not contain the exact gate set")
        return
    for gate in gates:
        gate_id = gate["id"]
        if set(gate) != {"id", "result", "evidence", "command"}:
            errors.append(f"gate {gate_id} has unexpected fields")
            continue
        if gate.get("result") not in {"green", "not_applicable"}:
            errors.append(f"gate {gate_id} result is invalid")
        evidence = gate.get("evidence")
        if not isinstance(evidence, list):
            errors.append(f"gate {gate_id} evidence must be a list")
        elif gate.get("result") == "green" and not evidence:
            errors.append(f"gate {gate_id} green evidence is empty")
        if gate_id == "independent-review-activation":
            if gate.get("result") != "not_applicable" or gate.get("command") is not None:
                errors.append("independent review must remain an external activation gate")
        elif gate.get("result") != "green":
            errors.append(f"mandatory gate {gate_id} is not green")


def validate_status_documents(root: Path, errors: list[str]) -> None:
    for relative_path in (
        "README.md",
        "docs/software/software-requirements-specification.md",
        "docs/software/roadmap.md",
    ):
        normalized = " ".join((root / relative_path).read_text(encoding="utf-8").split())
        if STATUS_MARKER not in normalized:
            errors.append(f"verification status marker is absent from {relative_path}")
        if "G9 remains a separate governed package" not in normalized:
            errors.append(f"G9 separation marker is absent from {relative_path}")


def validate_s0_evidence_contract(root: Path, errors: list[str]) -> None:
    schema = load_json(root / "tests/specifications/s0/evidence-manifest-v1.schema.json")
    definitions = schema.get("$defs", {})
    retained = definitions.get("retained_ci_evidence", {}).get("oneOf")
    if retained != [
        {"$ref": "#/$defs/github_actions_artifact"},
        {"$ref": "#/$defs/git_retained_github_actions_log"},
    ]:
        errors.append("S0 retained evidence union is invalid")
    git_retained = definitions.get("git_retained_github_actions_log", {})
    if git_retained.get("additionalProperties") is not False:
        errors.append("S0 Git-retained evidence must remain closed")
    properties = git_retained.get("properties", {})
    if properties.get("base_controlled", {}).get("const") is not True:
        errors.append("S0 Git-retained evidence must require base control")
    if properties.get("normalization", {}).get("const") != (
        "codenoesis.github-actions-log-normalization/v1"
    ):
        errors.append("S0 Git-retained normalization is invalid")


def validate_review(manifest: dict[str, Any], errors: list[str]) -> None:
    review = manifest.get("review")
    expected = {
        "state": "pending_independent_review",
        "required_actor": "github:smutti",
        "activation": "independent_review_then_protected_manual_merge_of_exact_head",
        "decision_url": None,
    }
    if review != expected:
        errors.append("review must remain pending independent activation")
    limitations = manifest.get("limitations")
    if not isinstance(limitations, list) or not limitations:
        errors.append("limitations must be a non-empty list")
    elif not any("G9" in limitation for limitation in limitations):
        errors.append("limitations must preserve the G9 boundary")


def validate_manifest(root: Path, manifest_path: Path) -> list[str]:
    errors: list[str] = []
    manifest = load_json(manifest_path)
    if not isinstance(manifest, dict):
        return ["manifest must be a JSON object"]
    if set(manifest) != MANIFEST_FIELDS:
        missing = sorted(MANIFEST_FIELDS - set(manifest))
        unexpected = sorted(set(manifest) - MANIFEST_FIELDS)
        if missing:
            errors.append(f"manifest is missing fields: {', '.join(missing)}")
        if unexpected:
            errors.append(f"manifest has unexpected fields: {', '.join(unexpected)}")

    plan = load_json(root / PLAN_PATH)
    catalog = load_json(root / CATALOG_PATH)
    if not isinstance(plan, dict) or not isinstance(catalog, dict):
        return ["plan and catalog must be JSON objects"]

    validate_authority(root, manifest, plan, errors)
    validate_changed_paths(root, errors)
    validate_path_digest(root, manifest.get("plan"), "plan", errors)
    validate_path_digest(root, manifest.get("profile_catalog"), "profile_catalog", errors)
    validate_product_tree(root, manifest, plan, errors)
    validate_repository_evidence(root, manifest, errors)
    metadata_by_id = validate_remote_runs(root, manifest, errors)
    validate_remote_logs(root, manifest, metadata_by_id, errors)
    validate_catalog(root, manifest, plan, catalog, errors)
    validate_gates(manifest, errors)
    validate_status_documents(root, errors)
    validate_s0_evidence_contract(root, errors)
    validate_review(manifest, errors)
    return errors


def parse_arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", required=True, type=Path)
    return parser.parse_args()


def main() -> int:
    arguments = parse_arguments()
    root = Path(__file__).resolve().parents[1]
    manifest_path = arguments.manifest
    if not manifest_path.is_absolute():
        manifest_path = root / manifest_path
    try:
        manifest_path.resolve().relative_to(root.resolve())
    except ValueError:
        print("manifest must stay inside the repository", file=sys.stderr)
        return 2
    try:
        errors = validate_manifest(root, manifest_path)
    except ValueError as error:
        print(f"local baseline verification failed: {error}", file=sys.stderr)
        return 1
    if errors:
        print("local baseline verification failed:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1
    manifest = load_json(manifest_path)
    print(
        "local baseline verification valid: "
        f"status={manifest['status']}, profiles={len(manifest['profiles'])}, "
        f"remote_runs={len(manifest['remote_runs'])}, "
        f"remote_logs={len(manifest['remote_logs'])}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
