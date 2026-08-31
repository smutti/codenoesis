#!/usr/bin/env python3
"""Verify the issue #201 LocalBaselineVerificationV3 evidence pack."""

from __future__ import annotations

import argparse
import base64
import gzip
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path
from typing import Any, Iterable, Optional


ISSUE = "https://github.com/smutti/codenoesis/issues/201"
BASE_SHA = "c783b612777a86e2f88620ece987723bb230c51c"
CHECKPOINT_SHA = "94618026a8c7ef34aa6ee0ae22b6e7b1aad9299b"
RED_COMMIT_SHA = "5345165af9f513a0737c40641246674f71e2485a"
EVIDENCE_PARENT_SHA = "332759213fb319f0b70d1820b568db5000cc8d47"
V2_ACTIVATION_MERGE = "1de6a420f25a1c7eb74d07a99f1800dde90eefa8"
R18_REVIEW_HEAD = "16ef5ceaea6ad14d9838f84856f6ca3d445daa67"
R18_REVIEW_TREE = "a1446f7621d5ce524792db74ce7b32640da80df4"
R18_MERGE = "fcdd6eddec8a4dd9b372cb88ff424c2004b5c88b"
R19_REVIEW_HEAD = "c3cbced9ee2017b61ec8e0b10191553edc733004"
R19_REVIEW_TREE = "f66ab5dfda426054d8591b246337293bbb85246a"
R19_MERGE = BASE_SHA
STATUS = "candidate_verified_pending_merge"
SCHEMA_VERSION = "codenoesis.local-baseline-verification/v3"
ERROR_SCHEMA_VERSION = "codenoesis.local-baseline-verification-error/v1"
RESULT_SCHEMA_VERSION = "codenoesis.local-baseline-verification-result/v1"
STATUS_MARKER = (
    "LocalBaselineVerificationV3 candidate Verified pending independent review "
    "and protected manual merge"
)

PLAN_PATH = Path("tests/specifications/verification/local-baseline-v3/plan.json")
CATALOG_PATH = Path(
    "tests/specifications/verification/local-baseline-v3/profile-catalog.json"
)
SCHEMA_PATH = Path(
    "tests/specifications/verification/local-baseline-v3/manifest.schema.json"
)
V2_CATALOG_PATH = Path(
    "tests/specifications/verification/local-baseline-v2/profile-catalog.json"
)
REMOTE_RUNS_PATH = Path(
    "tests/evidence/verification/local-baseline-v3/remote-runs.json"
)
REMOTE_LOG_INDEX_PATH = Path(
    "tests/evidence/verification/local-baseline-v3/remote-log-index.json"
)
RED_OBSERVATION_PATH = Path(
    "tests/evidence/verification/local-baseline-v3/red-observation.json"
)

SHA256_PATTERN = re.compile(r"^[0-9a-f]{64}$")
GIT_SHA_PATTERN = re.compile(r"^[0-9a-f]{40}$")

PROFILE_IDS = (
    "s0-walking-skeleton",
    "s1-safe-inventory",
    "r1-packed-sha1",
    "r2-gitlink-boundaries",
    "s2-rust-knowledge",
    "s3-atomic-storage",
    "s4-workspace-docs-query",
    "r3-root-package-workspace",
    "r4-cargo-manifest-facts",
    "r5-rust-semantic-depth",
    "r6-framework-declarations",
    "r7-scip-import",
    "r8-portable-explorer",
    "s5-incremental-refresh",
    "s6-openapi-federation",
    "k1-callable-value-semantics",
    "r9-output-capacity",
    "r10-cfg-declaration-alternatives",
    "r11-k1-repository-boundaries",
    "r12-k1-cfg-alternatives",
    "r13-k1-scip-composition",
    "r14-expression-bindings",
    "r15-local-flow",
    "r14-r15-real-repository-correction",
    "r16-safe-constant-evaluation",
    "r10-r16-versioned-explorer-correction",
    "r17-function-context",
    "s7-implementation-aware-api",
    "g0-release-profile",
    "g1a-local-distribution",
    "g2a-local-upgrade-safety",
    "g1b-g8-verifiable-distribution",
    "r18-trusted-local-source",
    "r19-git-backed-semantic-impact",
)

EVIDENCE_CLASSES = (
    "retained_red",
    "focused_green",
    "full_regression",
    "linux_native",
    "macos_native",
    "windows_native",
    "codeql",
    "benchmark_integrity",
    "policy_gate",
    "security",
    "browser",
    "real_repository_pilot",
    "supply_chain",
    "traceability",
    "independent_review",
)

MANIFEST_FIELDS = {
    "schema_version",
    "issue",
    "base_sha",
    "governance_checkpoint_sha",
    "red_commit_sha",
    "evidence_parent_sha",
    "verification_subject",
    "product_tree_sha256",
    "status",
    "plan",
    "profile_catalog",
    "v2_inheritance",
    "profiles",
    "repository_evidence",
    "remote_runs",
    "remote_logs",
    "required_gates",
    "environment",
    "limitations",
    "review",
}

CHECKPOINT_CONTRACT_DIGESTS = {
    "README.md": "2c045fdf288aa519e27945a74996c30ebdb991402a6120324edc92bdb3663ba9",
    "docs/software/architecture.md": "9440437ed6d75c200d93f261d54e2990f9e1c4571ab679de5cf3706fc6693f4d",
    "docs/software/roadmap.md": "862328559ce242fbf3a6fb51bc7207056ff6a10d86bcaf5295d0ab3068bc23e4",
    "docs/software/software-requirements-specification.md": "178ca85a14acd81b7e97979d3218c9dca6688f9b86bf955aef0fda209151516e",
    "docs/software/verification.md": "98d6808202e68787725470c1bee62f6ad74f48c114a5b32b936924e5cb68bad2",
    "docs/software/decisions/0041-local-baseline-verification-v3.md": "0c99a9bff50dae44d0926bfcc61f122ae5980df876f9a438d20ad6737ea95ce7",
    "scripts/tests/test_local_baseline_verification_v3.py": "937518c27b6636e77a0cc3c4b0c7b31cfcb42200475b3e93671ac88521b0af78",
    "tests/specifications/verification/local-baseline-v3/manifest.schema.json": "efebb43d3e794ba71735e83ad32ceed9b5f178b9ba1da0983749915273b7c2d7",
    "tests/specifications/verification/local-baseline-v3/plan.json": "c4ee51307e819675f71f659b64b4e168a6f9635e259e11e7c1b4a730ed61515a",
    "tests/specifications/verification/local-baseline-v3/profile-catalog.json": "f8a2186adea45ff052138b759b6aee4915b195d0802e31b41f4265a227c9c732",
}

V2_IMMUTABLE_DIGESTS = {
    "tests/specifications/verification/local-baseline-v2/profile-catalog.json": "23d175dd5e73f0c67e2d8c9d8fdf36c921220f41fcfaaf6586514ed6d632172b",
    "tests/specifications/verification/local-baseline-v2/plan.json": "2b3a6a5e71f35823faeb9a676a0bdd281ebac4095004bc0cd2f84f2f4264cc0f",
    "tests/specifications/verification/local-baseline-v2/manifest.schema.json": "72bb571a0d00b12543ccb4a3e4a42e13211942e1cdf78787f4b379252cb9a2bb",
    "tests/evidence/verification/local-baseline-v2/manifest.json": "123fb538e5f0566470d6f2c740b1e54f3fada3281e522179ed5e914f508e10e3",
    "scripts/verify_local_baseline_v2.py": "59a9eae29b5e756de6dd76895434cc244628b1ff793aa6b2858d6fa324a64499",
}

PINNED_EVIDENCE_DIGESTS = {
    "tests/evidence/verification/local-baseline-v3/red.log": "861923bd496522c1a9017983956f98d412a2efbf9e411717be729f4ad516a402",
    "tests/evidence/verification/local-baseline-v3/red-observation.json": "ad92c72bc9494a468e4ca30880bf270393b2a0b2dd9d1c9f70e6d13bf3ebceb3",
    "tests/evidence/verification/local-baseline-v3/remote-runs.json": "c81c55174f4a646110af8fddd79253bf74961131c5b990af1df3e55ab471c2e7",
    "tests/evidence/verification/local-baseline-v3/remote-log-index.json": "93cfbf7b149825a7c7e444ef27fe2f2ff0e4d7e065f48744493a86ef64d694d6",
}

EXPECTED_REMOTE_RUNS = {
    32229313149: ("r18-trusted-local-source", "ci", R18_REVIEW_HEAD),
    32229313162: ("r18-trusted-local-source", "benchmark", R18_REVIEW_HEAD),
    32229311917: ("r18-trusted-local-source", "codeql", R18_REVIEW_HEAD),
    32229313006: ("r18-trusted-local-source", "review-policy", R18_REVIEW_HEAD),
    33203760261: ("r19-git-backed-semantic-impact", "ci", R19_REVIEW_HEAD),
    33203760248: ("r19-git-backed-semantic-impact", "benchmark", R19_REVIEW_HEAD),
    33203757887: ("r19-git-backed-semantic-impact", "codeql", R19_REVIEW_HEAD),
    33203758766: ("r19-git-backed-semantic-impact", "review-policy", R19_REVIEW_HEAD),
}

EXPECTED_JOB_IDS = {
    95995562078,
    95995562220,
    95995562337,
    96001112520,
    98959469285,
    98959469170,
    98959469058,
    98965049693,
}

ALLOWED_EXACT_PATHS = {
    "README.md",
    "docs/software/software-requirements-specification.md",
    "docs/software/architecture.md",
    "docs/software/roadmap.md",
    "docs/software/verification.md",
    "docs/software/decisions/0041-local-baseline-verification-v3.md",
    "scripts/verify_local_baseline_v3.py",
    "scripts/tests/test_local_baseline_verification_v3.py",
}
ALLOWED_PREFIXES = (
    "tests/specifications/verification/local-baseline-v3/",
    "tests/evidence/verification/local-baseline-v3/",
)


def sha256_bytes(content: bytes) -> str:
    return hashlib.sha256(content).hexdigest()


def sha256_file(path: Path) -> str:
    hasher = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def safe_relative_path(value: Any) -> bool:
    if not isinstance(value, str) or not value or "\x00" in value:
        return False
    candidate = Path(value)
    return not candidate.is_absolute() and ".." not in candidate.parts


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


def validate_digest_record(
    root: Path,
    record: Any,
    label: str,
    errors: list[str],
) -> None:
    if not isinstance(record, dict) or set(record) != {"path", "sha256"}:
        errors.append(f"{label} must contain exactly path and sha256")
        return
    relative_path = record.get("path")
    expected_digest = record.get("sha256")
    if not safe_relative_path(relative_path):
        errors.append(f"{label}.path is unsafe")
        return
    if not isinstance(expected_digest, str) or not SHA256_PATTERN.fullmatch(
        expected_digest
    ):
        errors.append(f"{label}.sha256 is invalid")
        return
    absolute_path = root / relative_path
    if not absolute_path.is_file():
        errors.append(f"{label} path is missing: {relative_path}")
        return
    observed_digest = sha256_file(absolute_path)
    if observed_digest != expected_digest:
        errors.append(
            f"{label} digest mismatch for {relative_path}: "
            f"expected {expected_digest}, observed {observed_digest}"
        )


def validate_checkpoint(root: Path, errors: list[str]) -> None:
    for relative_path, expected_digest in CHECKPOINT_CONTRACT_DIGESTS.items():
        absolute_path = root / relative_path
        if not absolute_path.is_file():
            errors.append(f"checkpoint contract is missing: {relative_path}")
            continue
        if sha256_file(absolute_path) != expected_digest:
            errors.append(f"checkpoint contract changed after Red: {relative_path}")
        try:
            checkpoint_content = git(
                root,
                ["show", f"{CHECKPOINT_SHA}:{relative_path}"],
                binary=True,
            )
        except ValueError as error:
            errors.append(str(error))
            continue
        if sha256_bytes(checkpoint_content) != expected_digest:
            errors.append(f"checkpoint does not bind {relative_path}")


def validate_pinned_files(root: Path, errors: list[str]) -> None:
    for relative_path, expected_digest in {
        **V2_IMMUTABLE_DIGESTS,
        **PINNED_EVIDENCE_DIGESTS,
    }.items():
        absolute_path = root / relative_path
        if not absolute_path.is_file():
            errors.append(f"pinned file is missing: {relative_path}")
        elif sha256_file(absolute_path) != expected_digest:
            errors.append(f"pinned file digest changed: {relative_path}")


def validate_lineage(root: Path, current_head: str, errors: list[str]) -> None:
    for revision in (
        BASE_SHA,
        CHECKPOINT_SHA,
        RED_COMMIT_SHA,
        EVIDENCE_PARENT_SHA,
        V2_ACTIVATION_MERGE,
        R18_MERGE,
        R19_MERGE,
    ):
        try:
            git(root, ["cat-file", "-e", f"{revision}^{{commit}}"])
        except ValueError as error:
            errors.append(str(error))

    ancestry = (
        (V2_ACTIVATION_MERGE, R18_MERGE, "V2 activation is not an R18 ancestor"),
        (R18_MERGE, R19_MERGE, "R18 merge is not an R19 ancestor"),
        (BASE_SHA, CHECKPOINT_SHA, "base is not an ancestor of checkpoint"),
        (CHECKPOINT_SHA, RED_COMMIT_SHA, "checkpoint is not an ancestor of Red"),
        (
            RED_COMMIT_SHA,
            EVIDENCE_PARENT_SHA,
            "Red is not an ancestor of evidence parent",
        ),
        (
            EVIDENCE_PARENT_SHA,
            current_head,
            "evidence parent is not an ancestor of current head",
        ),
    )
    for ancestor, descendant, message in ancestry:
        if not git_is_ancestor(root, ancestor, descendant):
            errors.append(message)

    merge_identities = (
        (R18_MERGE, R18_REVIEW_TREE, "R18"),
        (R19_MERGE, R19_REVIEW_TREE, "R19"),
    )
    for merge_commit, expected_tree, label in merge_identities:
        try:
            observed_tree = git(root, ["rev-parse", f"{merge_commit}^{{tree}}"])
        except ValueError as error:
            errors.append(str(error))
            continue
        if observed_tree != expected_tree:
            errors.append(f"{label} merge tree differs from reviewed tree")


def is_excluded(path: str, exclusions: list[str]) -> bool:
    return any(path == exclusion or path.startswith(f"{exclusion}/") for exclusion in exclusions)


def product_tree_digest(root: Path, revision: str, exclusions: list[str]) -> str:
    raw_tree = git(
        root,
        ["ls-tree", "-r", "-z", "--full-tree", revision],
        binary=True,
    )
    records: list[tuple[str, str, str]] = []
    for entry in raw_tree.split(b"\0"):
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


def validate_product_tree(
    root: Path,
    current_head: str,
    plan: dict[str, Any],
    manifest: dict[str, Any],
    errors: list[str],
) -> None:
    exclusions = plan.get("product_tree_exclusions")
    if not isinstance(exclusions, list) or not exclusions:
        errors.append("product-tree exclusions must be a non-empty list")
        return
    if any(not safe_relative_path(exclusion) for exclusion in exclusions):
        errors.append("product-tree exclusions contain an unsafe path")
        return
    base_digest = product_tree_digest(root, BASE_SHA, exclusions)
    head_digest = product_tree_digest(root, current_head, exclusions)
    if manifest.get("product_tree_sha256") != base_digest:
        errors.append(f"product_tree_sha256 must equal {base_digest}")
    if head_digest != base_digest:
        errors.append("product tree differs from the exact authorized base")


def validate_changed_paths(root: Path, current_head: str, errors: list[str]) -> None:
    changed_output = git(root, ["diff", "--name-only", f"{BASE_SHA}..{current_head}"])
    for path in changed_output.splitlines():
        if path in ALLOWED_EXACT_PATHS or path.startswith(ALLOWED_PREFIXES):
            continue
        errors.append(f"changed path is outside issue #201 authority: {path}")


def resolved_catalog(
    root: Path,
    catalog: dict[str, Any],
    errors: list[str],
) -> list[dict[str, Any]]:
    inherited = catalog.get("inherited_catalog")
    expected_inherited = {
        "path": str(V2_CATALOG_PATH),
        "sha256": V2_IMMUTABLE_DIGESTS[str(V2_CATALOG_PATH)],
        "profile_count": 32,
    }
    if inherited != expected_inherited:
        errors.append("V2 inherited catalog binding is invalid")
        return []
    try:
        v2_catalog = load_json(root / V2_CATALOG_PATH)
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        errors.append(f"cannot load V2 catalog: {error}")
        return []
    v2_profiles = v2_catalog.get("profiles")
    additive_profiles = catalog.get("additive_profiles")
    if not isinstance(v2_profiles, list) or not isinstance(additive_profiles, list):
        errors.append("resolved profile inputs must be arrays")
        return []
    resolved = [*v2_profiles, *additive_profiles]
    profile_ids = tuple(profile.get("id") for profile in resolved if isinstance(profile, dict))
    if profile_ids != PROFILE_IDS:
        errors.append("resolved catalog is not the exact ordered 34-profile set")
    if catalog.get("profile_count") != 34:
        errors.append("catalog profile_count must equal 34")
    if tuple(catalog.get("required_profile_ids", [])) != PROFILE_IDS:
        errors.append("catalog required_profile_ids differ from the exact set")

    if len(additive_profiles) == 2:
        expected_additive = (
            (
                "r18-trusted-local-source",
                ["FR-CLI-011", "FR-CTX-002"],
                [191],
                R18_REVIEW_HEAD,
                R18_MERGE,
            ),
            (
                "r19-git-backed-semantic-impact",
                ["FR-CLI-006", "FR-CLI-012", "FR-IMP-006"],
                [197],
                R19_REVIEW_HEAD,
                R19_MERGE,
            ),
        )
        for profile, expected in zip(additive_profiles, expected_additive):
            fields = (
                profile.get("id"),
                profile.get("requirements"),
                profile.get("implementation_prs"),
                profile.get("review_head"),
                profile.get("merge_commit"),
            )
            if fields != expected:
                errors.append(f"additive profile authority is invalid: {expected[0]}")
    else:
        errors.append("catalog must contain exactly two additive profiles")
    return resolved


def profile_projection(profile: dict[str, Any]) -> dict[str, Any]:
    return {
        "id": profile["id"],
        "slice": profile["slice"],
        "requirements": profile["requirements"],
        "implementation_prs": profile["implementation_prs"],
        "oracle_paths": profile["oracle_paths"],
        "evidence_paths": profile["evidence_paths"],
    }


def validate_profiles(
    root: Path,
    resolved: list[dict[str, Any]],
    manifest_profiles: Any,
    errors: list[str],
) -> None:
    if not isinstance(manifest_profiles, list) or len(manifest_profiles) != 34:
        errors.append("manifest must contain exactly 34 profiles")
        return
    observed_ids = tuple(
        profile.get("id") for profile in manifest_profiles if isinstance(profile, dict)
    )
    if observed_ids != PROFILE_IDS or len(set(observed_ids)) != 34:
        errors.append("manifest profiles are missing, duplicated, or reordered")
        return

    for expected_profile, manifest_profile in zip(resolved, manifest_profiles):
        if not isinstance(manifest_profile, dict):
            errors.append("manifest profile must be an object")
            continue
        expected_keys = {
            "id",
            "slice",
            "requirements",
            "implementation_prs",
            "oracle_paths",
            "evidence_paths",
        }
        if set(manifest_profile) != expected_keys:
            errors.append(f"profile fields are invalid: {expected_profile['id']}")
            continue
        for field in ("id", "slice", "requirements", "implementation_prs"):
            if manifest_profile.get(field) != expected_profile.get(field):
                errors.append(
                    f"profile {expected_profile['id']} differs in {field}"
                )
        for field in ("oracle_paths", "evidence_paths"):
            expected_paths = expected_profile.get(field)
            records = manifest_profile.get(field)
            if not isinstance(expected_paths, list) or not isinstance(records, list):
                errors.append(f"profile {expected_profile['id']} has invalid {field}")
                continue
            if [record.get("path") for record in records if isinstance(record, dict)] != expected_paths:
                errors.append(f"profile {expected_profile['id']} {field} differ")
            for index, record in enumerate(records):
                validate_digest_record(
                    root,
                    record,
                    f"profiles.{expected_profile['id']}.{field}[{index}]",
                    errors,
                )


def validate_remote_evidence(
    root: Path,
    manifest: dict[str, Any],
    errors: list[str],
) -> None:
    try:
        remote_runs_document = load_json(root / REMOTE_RUNS_PATH)
        remote_log_document = load_json(root / REMOTE_LOG_INDEX_PATH)
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        errors.append(f"cannot load remote evidence: {error}")
        return

    detailed_runs = remote_runs_document.get("runs")
    if not isinstance(detailed_runs, list) or len(detailed_runs) != 8:
        errors.append("remote run metadata must contain exactly eight runs")
        return
    detailed_by_id = {
        run.get("run_id"): run for run in detailed_runs if isinstance(run, dict)
    }
    if set(detailed_by_id) != set(EXPECTED_REMOTE_RUNS):
        errors.append("remote run metadata has an unexpected run set")
    observed_required_jobs: set[int] = set()
    for run_id, expected in EXPECTED_REMOTE_RUNS.items():
        run = detailed_by_id.get(run_id)
        if not isinstance(run, dict):
            continue
        expected_profile, expected_kind, expected_head = expected
        if (
            run.get("profile") != expected_profile
            or run.get("kind") != expected_kind
            or run.get("head_sha") != expected_head
            or run.get("conclusion") != "success"
            or run.get("run_attempt") != 1
        ):
            errors.append(f"remote run metadata differs for {run_id}")
        expected_tree = R18_REVIEW_TREE if expected_head == R18_REVIEW_HEAD else R19_REVIEW_TREE
        if run.get("head_tree_sha") != expected_tree:
            errors.append(f"remote run tree differs for {run_id}")
        jobs = run.get("jobs")
        if not isinstance(jobs, list):
            errors.append(f"remote jobs are absent for {run_id}")
            continue
        for job in jobs:
            if isinstance(job, dict) and job.get("id") in EXPECTED_JOB_IDS:
                if job.get("conclusion") != "success":
                    errors.append(f"required remote job is not Green: {job.get('id')}")
                observed_required_jobs.add(job["id"])
    if observed_required_jobs != EXPECTED_JOB_IDS:
        errors.append("required Linux/macOS/Windows/policy job set is incomplete")

    manifest_runs = manifest.get("remote_runs")
    if not isinstance(manifest_runs, list) or len(manifest_runs) != 8:
        errors.append("manifest remote_runs must contain exactly eight runs")
    else:
        for run in manifest_runs:
            if not isinstance(run, dict):
                errors.append("manifest remote run must be an object")
                continue
            run_id = run.get("run_id")
            expected = EXPECTED_REMOTE_RUNS.get(run_id)
            if expected is None:
                errors.append(f"manifest references unexpected remote run {run_id}")
                continue
            expected_profile, expected_kind, expected_head = expected
            expected_run = {
                "profile": expected_profile,
                "kind": expected_kind,
                "run_id": run_id,
                "head_sha": expected_head,
                "conclusion": "success",
                "url": f"https://github.com/smutti/codenoesis/actions/runs/{run_id}",
            }
            if run != expected_run:
                errors.append(f"manifest remote run differs for {run_id}")

    index_entries = remote_log_document.get("entries")
    manifest_logs = manifest.get("remote_logs")
    if not isinstance(index_entries, list) or not isinstance(manifest_logs, list):
        errors.append("remote log index and manifest logs must be arrays")
        return
    if len(index_entries) != 9 or len(manifest_logs) != 9:
        errors.append("remote logs must contain exactly nine selected job logs")
        return
    index_by_id = {
        entry.get("id"): entry for entry in index_entries if isinstance(entry, dict)
    }
    selected_job_ids: set[int] = set()
    for manifest_log in manifest_logs:
        if not isinstance(manifest_log, dict):
            errors.append("manifest remote log must be an object")
            continue
        log_id = manifest_log.get("id")
        index_entry = index_by_id.get(log_id)
        if not isinstance(index_entry, dict):
            errors.append(f"remote log is absent from the index: {log_id}")
            continue
        expected_manifest_log = {
            "id": index_entry["id"],
            "run_id": index_entry["run_id"],
            "job_id": index_entry["job_id"],
            "head_sha": index_entry["head_sha"],
            "source_sha256": index_entry["source_sha256"],
            "normalization": remote_log_document.get("normalization"),
            "path": index_entry["path"],
            "sha256": index_entry["sha256"],
        }
        if manifest_log != expected_manifest_log:
            errors.append(f"manifest remote log differs for {log_id}")
        validate_digest_record(
            root,
            {"path": index_entry.get("path"), "sha256": index_entry.get("sha256")},
            f"remote_logs.{log_id}",
            errors,
        )
        carrier_path = index_entry.get("path")
        if not safe_relative_path(carrier_path) or not (root / carrier_path).is_file():
            continue
        carrier = (root / carrier_path).read_bytes()
        try:
            if index_entry.get("carrier") == "gzip-n-base64":
                normalized = gzip.decompress(
                    base64.b64decode(b"".join(carrier.split()), validate=True)
                )
            elif index_entry.get("carrier") == "raw-utf8-lf":
                normalized = carrier
            else:
                errors.append(f"unsupported remote log carrier for {log_id}")
                continue
        except (ValueError, OSError) as error:
            errors.append(f"cannot decode remote log {log_id}: {error}")
            continue
        if sha256_bytes(normalized) != index_entry.get("normalized_sha256"):
            errors.append(f"normalized remote log digest differs for {log_id}")
        try:
            normalized_text = normalized.decode("utf-8")
        except UnicodeDecodeError:
            errors.append(f"normalized remote log is not UTF-8: {log_id}")
        else:
            if "\r" in normalized_text or "\x1b[" in normalized_text:
                errors.append(f"remote log normalization is incomplete: {log_id}")
            if "/Users/" in normalized_text or "/private/" in normalized_text:
                errors.append(f"remote log contains a private local path: {log_id}")
        selected_job_ids.add(index_entry.get("job_id"))
    if not {98959469058, 98965049693}.issubset(selected_job_ids):
        errors.append("R19 Windows and final policy logs are not both retained")


def validate_repository_evidence(
    root: Path,
    manifest: dict[str, Any],
    errors: list[str],
) -> set[str]:
    repository_evidence = manifest.get("repository_evidence")
    if not isinstance(repository_evidence, list) or len(repository_evidence) < 15:
        errors.append("repository_evidence must contain at least 15 records")
        return set()
    evidence_ids: list[str] = []
    for index, record in enumerate(repository_evidence):
        if not isinstance(record, dict) or set(record) != {
            "id",
            "class",
            "path",
            "sha256",
        }:
            errors.append(f"repository_evidence[{index}] has invalid fields")
            continue
        evidence_id = record.get("id")
        evidence_class = record.get("class")
        if not isinstance(evidence_id, str) or not evidence_id:
            errors.append(f"repository_evidence[{index}].id is invalid")
        else:
            evidence_ids.append(evidence_id)
        if not isinstance(evidence_class, str) or not evidence_class:
            errors.append(f"repository_evidence[{index}].class is invalid")
        validate_digest_record(root, {"path": record.get("path"), "sha256": record.get("sha256")}, f"repository_evidence[{index}]", errors)
    if len(evidence_ids) != len(set(evidence_ids)):
        errors.append("repository evidence IDs must be unique")
    return set(evidence_ids)


def validate_gates(
    manifest: dict[str, Any],
    repository_ids: set[str],
    errors: list[str],
) -> None:
    gates = manifest.get("required_gates")
    if not isinstance(gates, list) or len(gates) != len(EVIDENCE_CLASSES):
        errors.append("required_gates must contain exactly 15 evidence classes")
        return
    gate_ids = tuple(gate.get("id") for gate in gates if isinstance(gate, dict))
    if gate_ids != EVIDENCE_CLASSES:
        errors.append("required gate order or identity differs from the plan")
    remote_log_ids = {
        remote_log.get("id")
        for remote_log in manifest.get("remote_logs", [])
        if isinstance(remote_log, dict)
    }
    allowed_evidence_ids = repository_ids | remote_log_ids | {
        f"run:{run_id}" for run_id in EXPECTED_REMOTE_RUNS
    }
    for gate in gates:
        if not isinstance(gate, dict) or set(gate) != {"id", "status", "evidence_ids"}:
            errors.append("required gate has invalid fields")
            continue
        if gate.get("status") != "green":
            errors.append(f"required gate is not Green: {gate.get('id')}")
        evidence_ids = gate.get("evidence_ids")
        if not isinstance(evidence_ids, list) or not evidence_ids:
            errors.append(f"required gate has no evidence: {gate.get('id')}")
            continue
        dangling = set(evidence_ids) - allowed_evidence_ids
        if dangling:
            errors.append(
                f"required gate {gate.get('id')} has dangling evidence: "
                + ", ".join(sorted(dangling))
            )


def validate_status_documents(root: Path, errors: list[str]) -> None:
    documents = (
        "README.md",
        "docs/software/software-requirements-specification.md",
        "docs/software/architecture.md",
        "docs/software/roadmap.md",
        "docs/software/verification.md",
    )
    for relative_path in documents:
        normalized = " ".join((root / relative_path).read_text(encoding="utf-8").split())
        if STATUS_MARKER not in normalized:
            errors.append(f"V3 status marker is absent from {relative_path}")
    prohibited_claims = (
        "LocalBaselineVerificationV3 is generally available",
        "LocalBaselineVerificationV3 is released",
        "LocalBaselineVerificationV3 is supported",
    )
    combined = "\n".join(
        (root / relative_path).read_text(encoding="utf-8") for relative_path in documents
    )
    for claim in prohibited_claims:
        if claim in combined:
            errors.append(f"unsupported lifecycle claim is present: {claim}")


def validate_authority(
    manifest: dict[str, Any],
    plan: dict[str, Any],
    errors: list[str],
) -> None:
    constants = {
        "schema_version": SCHEMA_VERSION,
        "issue": ISSUE,
        "base_sha": BASE_SHA,
        "governance_checkpoint_sha": CHECKPOINT_SHA,
        "red_commit_sha": RED_COMMIT_SHA,
        "evidence_parent_sha": EVIDENCE_PARENT_SHA,
        "verification_subject": BASE_SHA,
        "status": STATUS,
    }
    for field, expected in constants.items():
        if manifest.get(field) != expected:
            errors.append(f"{field} must equal {expected}")
    if set(manifest) != MANIFEST_FIELDS:
        errors.append("manifest top-level fields differ from the closed contract")

    if plan.get("issue") != ISSUE or plan.get("base_sha") != BASE_SHA:
        errors.append("plan authority differs from issue #201")
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
    if plan.get("correction_budget") != 5:
        errors.append("plan correction budget must remain five")
    if tuple(plan.get("required_profile_ids", [])) != PROFILE_IDS:
        errors.append("plan profile set differs from the exact 34 profiles")
    if tuple(plan.get("required_evidence_classes", [])) != EVIDENCE_CLASSES:
        errors.append("plan evidence classes differ from the exact set")

    review = manifest.get("review")
    expected_review = {
        "authoring_agent_is_independent_reviewer": False,
        "required": True,
        "activation": "independent_review_then_protected_manual_merge_of_exact_head",
        "release_authority": False,
    }
    if review != expected_review:
        errors.append("review and activation authority is invalid")
    limitations = manifest.get("limitations")
    if not isinstance(limitations, list) or not limitations:
        errors.append("limitations must be explicit")
    elif not any("G9" in limitation and "GA" in limitation for limitation in limitations):
        errors.append("limitations must retain explicit G9 and GA exclusion")


def validate_red(root: Path, errors: list[str]) -> None:
    try:
        observation = load_json(root / RED_OBSERVATION_PATH)
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        errors.append(f"cannot load retained Red observation: {error}")
        return
    expected = {
        "base_sha": BASE_SHA,
        "governance_checkpoint_sha": CHECKPOINT_SHA,
        "exit_status": 1,
        "actual_failure": "LocalBaselineVerificationV3 validator and canonical 34-profile manifest are absent",
    }
    for field, value in expected.items():
        if observation.get(field) != value:
            errors.append(f"retained Red differs in {field}")
    log_record = observation.get("log")
    validate_digest_record(root, log_record, "retained Red log", errors)


def validate_manifest(root: Path, manifest_path: Path) -> list[str]:
    errors: list[str] = []
    try:
        manifest = load_json(manifest_path)
        plan = load_json(root / PLAN_PATH)
        catalog = load_json(root / CATALOG_PATH)
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        return [f"cannot load verification input: {error}"]
    if not isinstance(manifest, dict):
        return ["manifest must be a JSON object"]
    if not isinstance(plan, dict) or not isinstance(catalog, dict):
        return ["plan and catalog must be JSON objects"]

    try:
        current_head = git(root, ["rev-parse", "HEAD"])
    except ValueError as error:
        return [str(error)]

    validate_authority(manifest, plan, errors)
    validate_checkpoint(root, errors)
    validate_pinned_files(root, errors)
    validate_lineage(root, current_head, errors)
    validate_changed_paths(root, current_head, errors)
    validate_product_tree(root, current_head, plan, manifest, errors)
    validate_red(root, errors)

    validate_digest_record(root, manifest.get("plan"), "manifest.plan", errors)
    validate_digest_record(
        root,
        manifest.get("profile_catalog"),
        "manifest.profile_catalog",
        errors,
    )
    v2_inheritance = manifest.get("v2_inheritance")
    expected_v2 = {
        "catalog": {
            "path": "tests/specifications/verification/local-baseline-v2/profile-catalog.json",
            "sha256": V2_IMMUTABLE_DIGESTS["tests/specifications/verification/local-baseline-v2/profile-catalog.json"],
        },
        "plan": {
            "path": "tests/specifications/verification/local-baseline-v2/plan.json",
            "sha256": V2_IMMUTABLE_DIGESTS["tests/specifications/verification/local-baseline-v2/plan.json"],
        },
        "manifest_schema": {
            "path": "tests/specifications/verification/local-baseline-v2/manifest.schema.json",
            "sha256": V2_IMMUTABLE_DIGESTS["tests/specifications/verification/local-baseline-v2/manifest.schema.json"],
        },
        "manifest": {
            "path": "tests/evidence/verification/local-baseline-v2/manifest.json",
            "sha256": V2_IMMUTABLE_DIGESTS["tests/evidence/verification/local-baseline-v2/manifest.json"],
        },
        "validator": {
            "path": "scripts/verify_local_baseline_v2.py",
            "sha256": V2_IMMUTABLE_DIGESTS["scripts/verify_local_baseline_v2.py"],
        },
        "profile_count": 32,
        "activation_merge": V2_ACTIVATION_MERGE,
    }
    if v2_inheritance != expected_v2:
        errors.append("V2 inheritance differs from the immutable accepted baseline")
    else:
        for field in ("catalog", "plan", "manifest_schema", "manifest", "validator"):
            validate_digest_record(root, v2_inheritance[field], f"v2_inheritance.{field}", errors)

    resolved = resolved_catalog(root, catalog, errors)
    validate_profiles(root, resolved, manifest.get("profiles"), errors)
    validate_remote_evidence(root, manifest, errors)
    repository_ids = validate_repository_evidence(root, manifest, errors)
    validate_gates(manifest, repository_ids, errors)
    validate_status_documents(root, errors)

    environment = manifest.get("environment")
    if not isinstance(environment, dict) or environment.get("network_product_path") != "disabled" or environment.get("model_provider_path") != "disabled":
        errors.append("environment must disable product network and model-provider paths")
    return sorted(set(errors))


def canonical_json(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8") + b"\n"


def repository_root() -> Optional[Path]:
    try:
        completed = subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except (OSError, subprocess.CalledProcessError):
        return None
    return Path(completed.stdout.decode("utf-8").strip())


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Verify LocalBaselineVerificationV3 evidence",
    )
    parser.add_argument("--manifest", required=True, type=Path)
    arguments = parser.parse_args()
    root = repository_root()
    if root is None:
        failure = {
            "schema_version": ERROR_SCHEMA_VERSION,
            "status": "invalid",
            "errors": ["cannot resolve repository root"],
        }
        sys.stderr.buffer.write(canonical_json(failure))
        return 2
    manifest_path = arguments.manifest
    if not manifest_path.is_absolute():
        manifest_path = root / manifest_path
    errors = validate_manifest(root, manifest_path)
    if errors:
        failure = {
            "schema_version": ERROR_SCHEMA_VERSION,
            "status": "invalid",
            "errors": errors,
        }
        sys.stderr.buffer.write(canonical_json(failure))
        return 2
    success = {
        "base_sha": BASE_SHA,
        "profile_count": len(PROFILE_IDS),
        "schema_version": RESULT_SCHEMA_VERSION,
        "status": STATUS,
        "verification_subject": BASE_SHA,
    }
    sys.stdout.buffer.write(canonical_json(success))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
