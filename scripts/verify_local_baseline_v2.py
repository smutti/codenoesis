#!/usr/bin/env python3
"""Verify the issue #188 local baseline evidence pack without dependencies."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable, Optional


ISSUE = "https://github.com/smutti/codenoesis/issues/188"
BASE_SHA = "9ecdc3acefd43495daf76b9f2ab69a7bbacff172"
CHECKPOINT_SHA = "1daa23ffbaa0ea13f5fc5910aa5798c7523f254c"
RED_COMMIT_SHA = "6fef76a4ba7fff8435d85a6df4c6efb3b1d1991b"
EVIDENCE_PARENT_SHA = "b754282d56dc65df8044d3e6d4acc672b5ae8cde"
FULL_GATE_HEAD_SHA = "5b3a4ea54dc9252a384d729e683f4cbb1ade8ce8"
LEGACY_REMOTE_HEAD_SHA = "b80c08935c32293f1315ae6dd1b4f1a7f52cd6c5"
REVIEW_HEAD_SHA = "a40a4cb0212e7b59b1eff81ab9818299c7ebc3b9"
REVIEW_TREE_SHA = "c13f40fa96017fa4407cb26052f8b5e3c7bb7009"
MERGE_COMMIT_SHA = "1de6a420f25a1c7eb74d07a99f1800dde90eefa8"
MERGE_TREE_SHA = REVIEW_TREE_SHA
ACTIVATION_SCHEMA_SHA256 = (
    "21b6747fa803848a25476832c474b27f420127c96e2c1d1755a9ec97f60918cf"
)
ACTIVATION_RECORD_SHA256 = (
    "f3d7b3ec3803f2216583e3440484e7924cff9c7a18eac1d16813b12c446e7138"
)
LEGACY_S0_BUNDLE_SHA256 = (
    "978a7128498d54a6c4a6b3fec11d195e37d2f67e179d2babb5320668c4e44811"
)
CURRENT_S0_BUNDLE_SHA256 = (
    "7f8c7b67651a9ff56431c14410e8b8a551f28e207eb0a882d887add74ccabf3a"
)
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
MANIFEST_SCHEMA_PATH = Path(
    "tests/specifications/verification/local-baseline-v2/manifest.schema.json"
)
ACTIVATION_SCHEMA_PATH = Path(
    "tests/specifications/verification/local-baseline-v2/"
    "post-merge-activation-v1.schema.json"
)
ACTIVATION_RECORD_PATH = Path(
    "tests/evidence/verification/local-baseline-v2/post-merge-activation.json"
)
REMOTE_RUNS_PATH = Path(
    "tests/evidence/verification/local-baseline-v2/remote-runs.json"
)
CODEQL_LOG_INDEX_PATH = Path(
    "tests/evidence/verification/local-baseline-v2/codeql-rust-log/index.json"
)
SHA256 = re.compile(r"^[0-9a-f]{64}$")
GIT_SHA1 = re.compile(r"^[0-9a-f]{40}$")

CHECKPOINT_CONTRACT_DIGESTS = {
    "docs/software/decisions/0037-local-baseline-verification-v2.md": (
        "c149645a7a5914e956ec41ce79578cb14f563daf7bb39b393194289cfe7d9072"
    ),
    "docs/software/verification.md": (
        "86569b0274aa5a7f088a3007b1d9237418c505fd34ec8a1724e99ebb8ccfb754"
    ),
    "tests/specifications/s0/evidence-manifest-v1.schema.json": (
        "d1a09942c260115282364bad8be5eada0103a48d13245af384d852b84cb72216"
    ),
    "tests/specifications/verification/local-baseline-v2/manifest.schema.json": (
        "72bb571a0d00b12543ccb4a3e4a42e13211942e1cdf78787f4b379252cb9a2bb"
    ),
    "tests/specifications/verification/local-baseline-v2/plan.json": (
        "2b3a6a5e71f35823faeb9a676a0bdd281ebac4095004bc0cd2f84f2f4264cc0f"
    ),
    "tests/specifications/verification/local-baseline-v2/profile-catalog.json": (
        "23d175dd5e73f0c67e2d8c9d8fdf36c921220f41fcfaaf6586514ed6d632172b"
    ),
}

VERSIONED_DESCENDANT_CONTRACT_PATHS = {
    "docs/software/verification.md",
}

PINNED_REPOSITORY_EVIDENCE = {
    "v2-red-log": (
        "tests/evidence/verification/local-baseline-v2/red.log",
        "ba91fcc0d23684a57f1356db70ce41f41db36c9c718851bbfbb6cd0603124ca4",
    ),
    "v2-red-observation": (
        "tests/evidence/verification/local-baseline-v2/red-observation.json",
        "6db8f580239db79e4a206fe1d3dc33a998382e127fee9d6aae5dcf4817e80fad",
    ),
    "v2-focused-green-log": (
        "tests/evidence/verification/local-baseline-v2/focused-green.log",
        "1019817a0f99e1b173035271646446752574f27abf66ad2820e2d7e68222e552",
    ),
    "v2-focused-green-observation": (
        "tests/evidence/verification/local-baseline-v2/focused-green-observation.json",
        "3200b184e470a9f19337af69318951a0a604848a7385d39fb7f8bd91ed150f8a",
    ),
    "v2-full-gate-log": (
        "tests/evidence/verification/local-baseline-v2/full-gate.log",
        "fb883c5ba638d9ab8f6c28629d49d11d1906b8b906a33de8794a2514f37d412e",
    ),
    "v2-full-gate-observation": (
        "tests/evidence/verification/local-baseline-v2/full-gate-observation.json",
        "8b4fe9b5f4a88571502127b224c3926891439e35868a45eabc60ece1a7a66351",
    ),
    "remote-runs": (
        "tests/evidence/verification/local-baseline-v2/remote-runs.json",
        "98926c041270ff1d7346f018efe8a2b25f6b912a30510813a7bcee18e8d60b34",
    ),
    "g8-post-merge": (
        "tests/evidence/verification/local-baseline-v2/g8-post-merge/post-merge-evidence.json",
        "7f27251aeabd013d8894c18561c885af1fe23dabbd8cfb40d32bb9451aa0e323",
    ),
    "g8-run-metadata": (
        "tests/evidence/verification/local-baseline-v2/g8-post-merge/run-metadata.json",
        "55feac9c04a9fce07814e5e6645033be7cf60f1f139a17664835346e7fbb1fcf",
    ),
    "g8-attestation-bundle": (
        "tests/evidence/verification/local-baseline-v2/g8-post-merge/attestation.jsonl",
        "3048d8078a79a00dc75015b36b0ebb00718ebf13cd71258060821d837270f28f",
    ),
    "g8-attestation-positive": (
        "tests/evidence/verification/local-baseline-v2/g8-post-merge/attestation-positive.log",
        "ffba6ad834f421c8cc79a96558d3ce02e7c9fd2276fae5c558d857852009802c",
    ),
    "g8-attestation-negative": (
        "tests/evidence/verification/local-baseline-v2/g8-post-merge/attestation-negative.log",
        "25ca3eb9285198c3ed81d7474aaec97dfa2158d5703b6fbea57f3fd293d77c2f",
    ),
    "g8-candidate-verification": (
        "tests/evidence/verification/local-baseline-v2/g8-post-merge/candidate-verification.log",
        "af8f9ff48f6b9e5363acb7b3b9ef7dcf024a42b7b72304cdf5bd1e2bc5c825e8",
    ),
}

EXPECTED_REMOTE_LOG_DIGESTS = {
    "base-benchmark-validation": "4e3dc0113f1373ca3ff3b203d248eaa760bd7907e75e571d213c43ecf0129f74",
    "base-ci-linux": "86069e86b74a07f9f3e688f7402f9c4a92d7c3573ee4262531b0b5b2dd31489d",
    "base-ci-macos": "0c60785f96e3dbb8c2156ddc1822e4cfc73e3565020e290577ab443a570259bc",
    "base-ci-quality": "baa3bb0c3de514eeb34184ac259a2ab97eb27129f5b5e1f9af969065a333a523",
    "base-ci-supply-chain": "57f4f4b0ba16ba63737c432bae516d4bd120347d9a374af85b0b690c1574f5d0",
    "base-ci-windows": "d5ef2c107eec5664fc60bf65471d08bcbe995497fe53ad44da19802eb0f15874",
    "base-release-attestation": "3c5151e403cc8dc4442eefc3fafaca3629371667647e3d9f8ead39d7a223b2bc",
    "base-release-supply": "47de713d883fa1a08d31206114acc636c206d6df542b48c6d733d5981fbdbf82",
    "s0-pr15-quality": "5887763bfc16ba9ddbc32590793b82fc567efb16ed17336bf07dfbf2ce6f16fa",
}

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
    "crates/noesis/tests/s0_conformance.rs",
    "tests/specifications/s0/contract-bundle.json",
    "tests/specifications/s0/evidence-manifest-v1.schema.json",
    "scripts/verify_local_baseline_v2.py",
    "scripts/tests/test_local_baseline_verification_v2.py",
}
ALLOWED_PREFIXES = (
    "tests/specifications/verification/local-baseline-v2/",
    "tests/evidence/verification/local-baseline-v2/",
)
AUTHORIZED_PRODUCT_TREE_EXCLUSIONS = (
    "crates/noesis/tests/s0_conformance.rs",
    "tests/specifications/s0/contract-bundle.json",
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
CANONICAL_ORIGIN_URLS = {
    "git@github.com:smutti/codenoesis.git",
    "https://github.com/smutti/codenoesis",
    "https://github.com/smutti/codenoesis.git",
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


def json_type_matches(value: Any, expected: str) -> bool:
    if expected == "object":
        return isinstance(value, dict)
    if expected == "array":
        return isinstance(value, list)
    if expected == "string":
        return isinstance(value, str)
    if expected == "integer":
        return isinstance(value, int) and not isinstance(value, bool)
    if expected == "boolean":
        return isinstance(value, bool)
    if expected == "null":
        return value is None
    return False


def resolve_schema_ref(schema_root: dict[str, Any], reference: str) -> dict[str, Any]:
    if not reference.startswith("#/"):
        raise ValueError(f"unsupported JSON Schema reference: {reference}")
    value: Any = schema_root
    for raw_part in reference[2:].split("/"):
        part = raw_part.replace("~1", "/").replace("~0", "~")
        if not isinstance(value, dict) or part not in value:
            raise ValueError(f"unresolved JSON Schema reference: {reference}")
        value = value[part]
    if not isinstance(value, dict):
        raise ValueError(f"JSON Schema reference is not an object: {reference}")
    return value


def validate_json_schema(
    value: Any,
    schema: dict[str, Any],
    schema_root: dict[str, Any],
    location: str,
    errors: list[str],
) -> None:
    reference = schema.get("$ref")
    if isinstance(reference, str):
        validate_json_schema(
            value,
            resolve_schema_ref(schema_root, reference),
            schema_root,
            location,
            errors,
        )
        return

    expected_types = schema.get("type")
    if isinstance(expected_types, str):
        expected_types = [expected_types]
    if isinstance(expected_types, list) and not any(
        isinstance(expected, str) and json_type_matches(value, expected)
        for expected in expected_types
    ):
        errors.append(
            f"{location} must have JSON type {' or '.join(expected_types)}"
        )
        return

    if "const" in schema and value != schema["const"]:
        errors.append(f"{location} differs from its schema constant")
    enum = schema.get("enum")
    if isinstance(enum, list) and value not in enum:
        errors.append(f"{location} is outside its schema enum")

    if isinstance(value, dict):
        required = schema.get("required", [])
        for field in required:
            if field not in value:
                errors.append(f"{location}.{field} is required by the schema")
        properties = schema.get("properties", {})
        if schema.get("additionalProperties") is False:
            for field in value:
                if field not in properties:
                    errors.append(f"{location}.{field} is not allowed by the schema")
        for field, child in value.items():
            child_schema = properties.get(field)
            if isinstance(child_schema, dict):
                validate_json_schema(
                    child,
                    child_schema,
                    schema_root,
                    f"{location}.{field}",
                    errors,
                )

    if isinstance(value, list):
        minimum_items = schema.get("minItems")
        maximum_items = schema.get("maxItems")
        if isinstance(minimum_items, int) and len(value) < minimum_items:
            errors.append(f"{location} has fewer than {minimum_items} items")
        if isinstance(maximum_items, int) and len(value) > maximum_items:
            errors.append(f"{location} has more than {maximum_items} items")
        item_schema = schema.get("items")
        if isinstance(item_schema, dict):
            for index, item in enumerate(value):
                validate_json_schema(
                    item,
                    item_schema,
                    schema_root,
                    f"{location}[{index}]",
                    errors,
                )

    if isinstance(value, str):
        minimum_length = schema.get("minLength")
        maximum_length = schema.get("maxLength")
        if isinstance(minimum_length, int) and len(value) < minimum_length:
            errors.append(f"{location} is shorter than {minimum_length} characters")
        if isinstance(maximum_length, int) and len(value) > maximum_length:
            errors.append(f"{location} is longer than {maximum_length} characters")
        pattern = schema.get("pattern")
        if isinstance(pattern, str) and re.fullmatch(pattern, value) is None:
            errors.append(f"{location} does not match its schema pattern")
        if schema.get("format") == "date-time":
            try:
                parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
            except ValueError:
                errors.append(f"{location} is not an RFC 3339 date-time")
            else:
                if parsed.tzinfo is None or parsed.utcoffset() != timezone.utc.utcoffset(None):
                    errors.append(f"{location} must be an explicit UTC date-time")

    minimum = schema.get("minimum")
    if isinstance(value, int) and not isinstance(value, bool):
        if isinstance(minimum, int) and value < minimum:
            errors.append(f"{location} is below schema minimum {minimum}")


def validate_checkpoint_contracts(root: Path, errors: list[str]) -> None:
    for relative_path, expected_digest in CHECKPOINT_CONTRACT_DIGESTS.items():
        if relative_path not in VERSIONED_DESCENDANT_CONTRACT_PATHS:
            current_path = root / relative_path
            if not current_path.is_file():
                errors.append(f"checkpoint contract is missing: {relative_path}")
                continue
            current_digest = sha256_file(current_path)
            if current_digest != expected_digest:
                errors.append(f"checkpoint contract changed after Red: {relative_path}")
        try:
            checkpoint_bytes = git(
                root,
                ["show", f"{CHECKPOINT_SHA}:{relative_path}"],
                binary=True,
            )
        except ValueError as error:
            errors.append(str(error))
            continue
        if sha256_bytes(checkpoint_bytes) != expected_digest:
            errors.append(f"checkpoint digest does not bind {relative_path}")


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


def git_commit_exists(root: Path, revision: str) -> bool:
    completed = subprocess.run(
        ["git", "cat-file", "-e", f"{revision}^{{commit}}"],
        cwd=root,
        check=False,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    return completed.returncode == 0


def ensure_required_git_history(root: Path, errors: list[str]) -> None:
    head = git(root, ["rev-parse", "HEAD"])
    required_revisions = (
        BASE_SHA,
        CHECKPOINT_SHA,
        RED_COMMIT_SHA,
        EVIDENCE_PARENT_SHA,
        FULL_GATE_HEAD_SHA,
        LEGACY_REMOTE_HEAD_SHA,
        REVIEW_HEAD_SHA,
        MERGE_COMMIT_SHA,
        head,
    )
    missing = [
        revision
        for revision in required_revisions
        if not git_commit_exists(root, revision)
    ]
    if not missing:
        return
    if git(root, ["rev-parse", "--is-shallow-repository"]) != "true":
        errors.append(
            "required Git history is missing from a non-shallow repository: "
            + ", ".join(missing)
        )
        return
    if os.environ.get("GITHUB_ACTIONS") != "true":
        errors.append(
            "required Git history is unavailable in an offline shallow checkout: "
            + ", ".join(missing)
        )
        return
    origin = git(root, ["remote", "get-url", "origin"])
    if origin not in CANONICAL_ORIGIN_URLS:
        errors.append("shallow history hydration requires the canonical origin")
        return
    fetch_revisions = sorted(set((*missing, head)))
    command = [
        "git",
        "fetch",
        "--no-tags",
        "--no-recurse-submodules",
        "--depth=512",
        "origin",
        *fetch_revisions,
    ]
    completed = subprocess.run(
        command,
        cwd=root,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if completed.returncode != 0:
        diagnostic = completed.stderr.decode("utf-8", errors="replace").strip()
        errors.append(f"exact shallow history hydration failed: {diagnostic}")
        return
    still_missing = [
        revision
        for revision in required_revisions
        if not git_commit_exists(root, revision)
    ]
    if still_missing:
        errors.append(
            "exact shallow history hydration is incomplete: "
            + ", ".join(still_missing)
        )


def validate_post_merge_activation(
    root: Path,
    activation: dict[str, Any],
    current_head: str,
    errors: list[str],
) -> None:
    schema_path = root / ACTIVATION_SCHEMA_PATH
    record_path = root / ACTIVATION_RECORD_PATH
    if not schema_path.is_file():
        errors.append("post-merge activation schema is missing")
        return
    if not record_path.is_file():
        errors.append("post-merge activation record is missing")
        return
    if sha256_file(schema_path) != ACTIVATION_SCHEMA_SHA256:
        errors.append("post-merge activation schema digest is invalid")
    if sha256_file(record_path) != ACTIVATION_RECORD_SHA256:
        errors.append("post-merge activation record digest is invalid")

    schema = load_json(schema_path)
    if not isinstance(schema, dict):
        errors.append("post-merge activation schema must be an object")
        return
    validate_json_schema(activation, schema, schema, "$activation", errors)

    if not git_is_ancestor(root, MERGE_COMMIT_SHA, current_head):
        errors.append("exact activation merge is not an ancestor of current head")

    review_tree = git(root, ["rev-parse", f"{REVIEW_HEAD_SHA}^{{tree}}"])
    merge_tree = git(root, ["rev-parse", f"{MERGE_COMMIT_SHA}^{{tree}}"])
    if review_tree != REVIEW_TREE_SHA:
        errors.append("review head tree differs from the authorized identity")
    if merge_tree != MERGE_TREE_SHA:
        errors.append("activation merge tree differs from the authorized identity")
    if review_tree != merge_tree:
        errors.append("review head and activation merge trees differ")

    merge_line = git(
        root,
        ["rev-list", "--parents", "-n", "1", MERGE_COMMIT_SHA],
    ).split()
    if merge_line != [MERGE_COMMIT_SHA, BASE_SHA]:
        errors.append("activation merge must have only the exact protected base parent")


def resolve_validation_subject(
    root: Path,
    current_head: str,
    activation: dict[str, Any],
    errors: list[str],
) -> Optional[str]:
    if git_is_ancestor(root, MERGE_COMMIT_SHA, current_head):
        error_count = len(errors)
        validate_post_merge_activation(root, activation, current_head, errors)
        if len(errors) != error_count:
            return None
        return REVIEW_HEAD_SHA
    if git_is_ancestor(root, EVIDENCE_PARENT_SHA, current_head):
        return current_head
    errors.append("current head is not on a recognized lineage for V2 verification")
    return None


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
    validation_subject: str,
    errors: list[str],
) -> None:
    constants = {
        "schema_version": SCHEMA_VERSION,
        "issue": ISSUE,
        "base_sha": BASE_SHA,
        "governance_checkpoint_sha": CHECKPOINT_SHA,
        "evidence_parent_sha": EVIDENCE_PARENT_SHA,
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

    for revision in (
        BASE_SHA,
        CHECKPOINT_SHA,
        RED_COMMIT_SHA,
        evidence_parent,
        validation_subject,
    ):
        try:
            git(root, ["cat-file", "-e", f"{revision}^{{commit}}"])
        except ValueError as error:
            errors.append(str(error))

    ancestry = (
        (BASE_SHA, CHECKPOINT_SHA, "base is not an ancestor of checkpoint"),
        (CHECKPOINT_SHA, RED_COMMIT_SHA, "checkpoint is not an ancestor of Red"),
        (RED_COMMIT_SHA, evidence_parent, "Red is not an ancestor of evidence parent"),
        (
            evidence_parent,
            validation_subject,
            "evidence parent is not an ancestor of validation subject",
        ),
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


def validate_changed_paths(
    root: Path,
    validation_subject: str,
    errors: list[str],
) -> None:
    changed = git(
        root,
        ["diff", "--name-only", f"{BASE_SHA}..{validation_subject}"],
    )
    changed_paths = [path for path in changed.splitlines() if path]
    for path in changed_paths:
        if path in ALLOWED_EXACT_PATHS or path.startswith(ALLOWED_PREFIXES):
            continue
        errors.append(f"changed path is outside issue #188 authority: {path}")


def validate_product_tree(
    root: Path,
    manifest: dict[str, Any],
    plan: dict[str, Any],
    validation_subject: str,
    errors: list[str],
) -> None:
    exclusions = plan.get("product_tree_exclusions")
    if not isinstance(exclusions, list) or not exclusions:
        errors.append("plan product_tree_exclusions must be non-empty")
        return
    if any(not safe_relative_path(path) for path in exclusions):
        errors.append("plan product_tree_exclusions contains an unsafe path")
        return
    effective_exclusions = [*exclusions, *AUTHORIZED_PRODUCT_TREE_EXCLUSIONS]
    base_digest = product_tree_digest(root, BASE_SHA, effective_exclusions)
    head_digest = product_tree_digest(
        root,
        validation_subject,
        effective_exclusions,
    )
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
    repository_path_to_id = {
        item.get("path"): item.get("id")
        for item in manifest.get("repository_evidence", [])
        if isinstance(item, dict)
        and isinstance(item.get("path"), str)
        and isinstance(item.get("id"), str)
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
        catalog_profile = catalog_by_id[profile_id]
        expected_catalog_evidence = {
            repository_path_to_id.get(relative_path)
            for field in ("oracle_paths", "evidence_paths")
            for relative_path in catalog_profile[field]
        }
        if None in expected_catalog_evidence:
            missing_paths = sorted(
                relative_path
                for field in ("oracle_paths", "evidence_paths")
                for relative_path in catalog_profile[field]
                if relative_path not in repository_path_to_id
            )
            errors.append(
                f"manifest profile {profile_id} has unbound catalog paths: "
                f"{', '.join(missing_paths)}"
            )
            expected_catalog_evidence.discard(None)
        if set(result) != {
            "id",
            "catalog_entry_sha256",
            "evidence_classes",
            "result",
        }:
            errors.append(f"manifest profile {profile_id} has unexpected fields")
            continue
        expected_catalog_digest = sha256_bytes(canonical_json(catalog_profile))
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
        referenced_evidence: set[str] = set()
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
            if len(evidence) != len(set(evidence)):
                errors.append(
                    f"manifest profile {profile_id} evidence references are duplicated"
                )
            referenced_evidence.update(evidence)
            if not isinstance(rationale, str) or not rationale:
                errors.append(
                    f"manifest profile {profile_id} evidence rationale is empty"
                )
            known_references = repository_ids | remote_log_ids | remote_run_ids
            for reference in evidence:
                if reference not in known_references:
                    errors.append(
                        f"manifest profile {profile_id} has dangling evidence reference: "
                        f"{reference}"
                    )
        missing_catalog_evidence = expected_catalog_evidence - referenced_evidence
        if missing_catalog_evidence:
            errors.append(
                f"manifest profile {profile_id} does not reference every catalog file: "
                f"{', '.join(sorted(missing_catalog_evidence))}"
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
    evidence_by_id: dict[str, dict[str, Any]] = {}
    for index, item in enumerate(evidence):
        label = f"repository_evidence[{index}]"
        if not isinstance(item, dict) or set(item) != {"id", "path", "sha256", "kind"}:
            errors.append(f"{label} has unexpected fields")
            continue
        evidence_id = item.get("id")
        ids.append(evidence_id)
        paths.append(item.get("path"))
        if isinstance(evidence_id, str):
            evidence_by_id[evidence_id] = item
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
    for evidence_id, (expected_path, expected_digest) in (
        PINNED_REPOSITORY_EVIDENCE.items()
    ):
        item = evidence_by_id.get(evidence_id)
        if item is None:
            errors.append(f"pinned repository evidence is missing: {evidence_id}")
            continue
        if item.get("path") != expected_path or item.get("sha256") != expected_digest:
            errors.append(f"pinned repository evidence changed: {evidence_id}")


def validate_full_gate_evidence(
    root: Path,
    manifest: dict[str, Any],
    validation_subject: str,
    errors: list[str],
) -> None:
    observation_path = root / (
        "tests/evidence/verification/local-baseline-v2/full-gate-observation.json"
    )
    observation = load_json(observation_path)
    expected_fields = {
        "schema_version",
        "issue",
        "base_sha",
        "governance_checkpoint_sha",
        "red_commit_sha",
        "head_sha",
        "started_at",
        "completed_at",
        "duration_seconds",
        "result",
        "commands",
        "log_path",
        "log_sha256",
        "log_bytes",
        "environment",
    }
    if not isinstance(observation, dict) or set(observation) != expected_fields:
        errors.append("full-gate observation has unexpected fields")
        return
    expected_identity = {
        "schema_version": "codenoesis.local-baseline-full-gate/v1",
        "issue": ISSUE,
        "base_sha": BASE_SHA,
        "governance_checkpoint_sha": CHECKPOINT_SHA,
        "red_commit_sha": RED_COMMIT_SHA,
        "head_sha": FULL_GATE_HEAD_SHA,
        "result": "green",
        "log_path": "tests/evidence/verification/local-baseline-v2/full-gate.log",
        "log_sha256": (
            "fb883c5ba638d9ab8f6c28629d49d11d1906b8b906a33de8794a2514f37d412e"
        ),
        "log_bytes": 78876,
    }
    for field, expected in expected_identity.items():
        if observation.get(field) != expected:
            errors.append(f"full-gate observation {field} is invalid")

    expected_commands = [
        "python3 scripts/verify_local_baseline_v2.py --manifest "
        "tests/evidence/verification/local-baseline-v2/manifest.json",
        "actionlint -no-color",
        "python3 .github/codex/scripts/codex_policy.py validate-policy --policy "
        ".github/codex/policy.json",
        "cargo fmt --all --check",
        "cargo clippy --workspace --all-targets --all-features --locked -- "
        "-D warnings",
        "cargo test --workspace --all-targets --all-features --locked",
        "cargo test --workspace --doc --all-features --locked",
        "python3 -m unittest discover -s scripts/tests -p 'test_*.py'",
        "python3 scripts/validate_benchmark_assets.py",
        "cargo bench --workspace --all-features --no-run --locked",
    ]
    commands = observation.get("commands")
    if commands != [
        {"command": command, "exit_status": 0} for command in expected_commands
    ]:
        errors.append("full-gate command sequence or result is invalid")

    try:
        started_at = datetime.fromisoformat(
            observation["started_at"].replace("Z", "+00:00")
        )
        completed_at = datetime.fromisoformat(
            observation["completed_at"].replace("Z", "+00:00")
        )
    except (AttributeError, TypeError, ValueError):
        errors.append("full-gate timestamps are invalid")
    else:
        duration_seconds = int((completed_at - started_at).total_seconds())
        if duration_seconds != 400 or observation.get("duration_seconds") != 400:
            errors.append("full-gate duration is invalid")

    expected_environment = {
        "os": "Darwin 25.5.0",
        "architecture": "arm64",
        "python": "Python 3.9.6",
        "rustc": "rustc 1.97.1 (8bab26f4f 2026-07-14)",
        "cargo": "cargo 1.97.1 (c980f4866 2026-06-30)",
        "actionlint": "1.7.12",
        "policy_sha256": (
            "98c1ad9e1897b3be091522100d401106e655f4a570458ba3d9c213c739775993"
        ),
    }
    if observation.get("environment") != expected_environment:
        errors.append("full-gate environment is invalid")

    log_path = root / expected_identity["log_path"]
    log_bytes = log_path.read_bytes()
    if (
        len(log_bytes) != observation["log_bytes"]
        or sha256_bytes(log_bytes) != observation["log_sha256"]
    ):
        errors.append("full-gate retained log identity is invalid")
    expected_prefix = (
        "LocalBaselineVerificationV2 full gate\n"
        f"head={FULL_GATE_HEAD_SHA}\n"
        f"started_at={observation['started_at']}\n"
    ).encode("utf-8")
    expected_suffix = f"completed_at={observation['completed_at']}\n".encode("utf-8")
    if not log_bytes.startswith(expected_prefix) or not log_bytes.endswith(
        expected_suffix
    ):
        errors.append("full-gate retained log boundary is invalid")

    if (
        git(root, ["rev-parse", f"{FULL_GATE_HEAD_SHA}^{{commit}}"])
        != FULL_GATE_HEAD_SHA
    ):
        errors.append("full-gate head commit is unavailable")
    if not git_is_ancestor(root, FULL_GATE_HEAD_SHA, validation_subject):
        errors.append("full-gate head is not an ancestor of the review head")

    required_references = {"v2-full-gate-log", "v2-full-gate-observation"}
    for profile in manifest.get("profiles", []):
        if not isinstance(profile, dict):
            continue
        classes = profile.get("evidence_classes", [])
        full_regression = next(
            (
                item
                for item in classes
                if isinstance(item, dict) and item.get("id") == "full_regression"
            ),
            None,
        )
        if full_regression is None or not required_references.issubset(
            set(full_regression.get("evidence", []))
        ):
            errors.append(
                f"manifest profile {profile.get('id')} lacks full-gate evidence"
            )
    full_gate = next(
        (
            gate
            for gate in manifest.get("required_gates", [])
            if isinstance(gate, dict) and gate.get("id") == "full-regression"
        ),
        None,
    )
    if full_gate is None or not required_references.issubset(
        set(full_gate.get("evidence", []))
    ):
        errors.append("full-regression gate lacks retained full-gate evidence")


def validate_remote_runs(
    root: Path,
    manifest: dict[str, Any],
    errors: list[str],
) -> dict[int, dict[str, Any]]:
    metadata = load_json(root / REMOTE_RUNS_PATH)
    if metadata.get("schema_version") != "codenoesis.github-actions-run-evidence/v1":
        errors.append("remote-runs.json schema version is invalid")
    if metadata.get("repository") != "smutti/codenoesis":
        errors.append("remote-runs.json repository is invalid")
    metadata_runs = metadata.get("runs")
    if not isinstance(metadata_runs, list):
        errors.append("remote-runs.json must contain runs")
        return {}
    metadata_by_id = {item["run_id"]: item for item in metadata_runs}
    if set(metadata_by_id) != EXPECTED_REMOTE_RUN_IDS:
        errors.append("remote-runs.json does not contain the exact required run set")
    for run_id, item in metadata_by_id.items():
        if item.get("run_attempt") != 1 or item.get("conclusion") != "success":
            errors.append(f"remote run {run_id} is not the successful first attempt")
        expected_url = f"https://github.com/smutti/codenoesis/actions/runs/{run_id}"
        if item.get("url") != expected_url:
            errors.append(f"remote run {run_id} URL is not canonical")
        head_sha = item.get("head_sha")
        if not isinstance(head_sha, str) or not GIT_SHA1.fullmatch(head_sha):
            errors.append(f"remote run {run_id} head is invalid")
        else:
            actual_tree = git(root, ["rev-parse", f"{head_sha}^{{tree}}"])
            if item.get("tree_sha") != actual_tree:
                errors.append(f"remote run {run_id} tree does not match its head")
        jobs = item.get("jobs")
        if not isinstance(jobs, list) or not jobs:
            errors.append(f"remote run {run_id} has no retained jobs")
        else:
            job_ids = [job.get("job_id") for job in jobs if isinstance(job, dict)]
            if len(job_ids) != len(jobs) or len(job_ids) != len(set(job_ids)):
                errors.append(f"remote run {run_id} job identities are invalid")
            if any(job.get("conclusion") != "success" for job in jobs):
                errors.append(f"remote run {run_id} contains a non-Green job")

    g8_run = metadata_by_id.get(31843845301, {})
    if (
        g8_run.get("artifact_id") != 9235430119
        or g8_run.get("artifact_digest")
        != "sha256:3c15eabb8fb3125dfe01e48c51c0322231f6e64a733eb72380bad8d86f8ecea8"
        or g8_run.get("attestation_id") != 40843198
        or g8_run.get("attestation_bundle_sha256")
        != "3048d8078a79a00dc75015b36b0ebb00718ebf13cd71258060821d837270f28f"
    ):
        errors.append("G1b/G8 retained run identities differ from issue #188")
    codeql_run = metadata_by_id.get(31842394592, {})
    if (
        codeql_run.get("downloaded_rust_job_log_sha256")
        != "e6f3ae953eff416ebdf4dd7c3d517f17e5d755ed200dedab0164bd06dfa32067"
        or codeql_run.get("downloaded_rust_job_log_bytes") != 1867883
    ):
        errors.append("CodeQL retained Rust log identity is invalid")

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
        expected_digest = EXPECTED_REMOTE_LOG_DIGESTS[log_id]
        if committed_digest != expected_digest:
            errors.append(f"remote log {log_id} differs from its retained source digest")
        if item.get("check_name") != item.get("job_name"):
            errors.append(f"remote log {log_id} check name mismatch")
        expected_workflow_ref = f"refs/heads/{run.get('head_branch')}"
        if item.get("workflow_ref") != expected_workflow_ref:
            errors.append(f"remote log {log_id} workflow ref mismatch")

        try:
            retained_bytes = git(
                root,
                ["show", f"{evidence_parent}:{relative_path}"],
                binary=True,
            )
        except ValueError as error:
            errors.append(str(error))
        else:
            if sha256_bytes(retained_bytes) != committed_digest:
                errors.append(f"remote log {log_id} evidence-head bytes mismatch")
        parent = git(root, ["rev-parse", f"{evidence_parent}^"])
        existed_before = subprocess.run(
            ["git", "cat-file", "-e", f"{parent}:{relative_path}"],
            cwd=root,
            check=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        if existed_before.returncode == 0:
            errors.append(f"remote log {log_id} predates its first-retention commit")

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


def validate_codeql_log(
    root: Path,
    manifest: dict[str, Any],
    metadata_by_id: dict[int, dict[str, Any]],
    errors: list[str],
) -> None:
    index = load_json(root / CODEQL_LOG_INDEX_PATH)
    expected_fields = {
        "schema_version",
        "repository",
        "run_id",
        "run_attempt",
        "job_id",
        "job_name",
        "command",
        "normalization",
        "source_log_sha256",
        "source_log_bytes",
        "chunks",
    }
    if not isinstance(index, dict) or set(index) != expected_fields:
        errors.append("CodeQL retained-log index has unexpected fields")
        return
    if index.get("schema_version") != "codenoesis.git-retained-command-log/v1":
        errors.append("CodeQL retained-log index schema is invalid")
    if index.get("repository") != "smutti/codenoesis":
        errors.append("CodeQL retained-log repository is invalid")
    if index.get("run_id") != 31842394592 or index.get("run_attempt") != 1:
        errors.append("CodeQL retained-log run identity is invalid")
    if index.get("job_id") != 94901863459 or index.get("job_name") != "Analyze (rust)":
        errors.append("CodeQL retained-log job identity is invalid")
    if index.get("command") != (
        "gh run view 31842394592 --repo smutti/codenoesis "
        "--job 94901863459 --log"
    ):
        errors.append("CodeQL retained-log command is invalid")
    if index.get("normalization") != "gh-run-view-job-log/v1":
        errors.append("CodeQL retained-log normalization is invalid")

    run = metadata_by_id.get(31842394592, {})
    jobs = {job.get("job_id"): job for job in run.get("jobs", [])}
    if jobs.get(94901863459, {}).get("conclusion") != "success":
        errors.append("CodeQL retained-log job is not Green")

    repository_by_path = {
        item.get("path"): item
        for item in manifest.get("repository_evidence", [])
        if isinstance(item, dict)
    }
    chunks = index.get("chunks")
    if not isinstance(chunks, list) or not chunks:
        errors.append("CodeQL retained-log chunks are missing")
        return
    reconstructed = bytearray()
    observed_paths: list[str] = []
    for chunk_index, chunk in enumerate(chunks):
        label = f"CodeQL retained-log chunk {chunk_index}"
        if not isinstance(chunk, dict) or set(chunk) != {"path", "sha256", "bytes"}:
            errors.append(f"{label} has unexpected fields")
            continue
        relative_path = chunk.get("path")
        if not safe_relative_path(relative_path):
            errors.append(f"{label} path is unsafe")
            continue
        observed_paths.append(relative_path)
        path = root / relative_path
        if not path.is_file():
            errors.append(f"{label} file is missing")
            continue
        content = path.read_bytes()
        digest = sha256_bytes(content)
        if chunk.get("sha256") != digest or chunk.get("bytes") != len(content):
            errors.append(f"{label} digest or length mismatch")
        repository_item = repository_by_path.get(relative_path)
        if (
            repository_item is None
            or repository_item.get("sha256") != digest
            or repository_item.get("kind") != "security"
        ):
            errors.append(f"{label} is not bound as repository security evidence")
        reconstructed.extend(content)
    if observed_paths != sorted(observed_paths) or len(observed_paths) != len(
        set(observed_paths)
    ):
        errors.append("CodeQL retained-log chunk order or identity is invalid")
    reconstructed_digest = sha256_bytes(bytes(reconstructed))
    if (
        index.get("source_log_sha256") != reconstructed_digest
        or index.get("source_log_bytes") != len(reconstructed)
    ):
        errors.append("CodeQL retained log does not reconstruct its source bytes")
    if (
        reconstructed_digest
        != "e6f3ae953eff416ebdf4dd7c3d517f17e5d755ed200dedab0164bd06dfa32067"
        or len(reconstructed) != 1867883
    ):
        errors.append("CodeQL retained log differs from the bound remote observation")


def validate_gates(manifest: dict[str, Any], errors: list[str]) -> None:
    gates = manifest.get("required_gates")
    if not isinstance(gates, list):
        errors.append("required_gates must be a list")
        return
    gate_ids = [gate.get("id") for gate in gates if isinstance(gate, dict)]
    if set(gate_ids) != EXPECTED_GATE_IDS or len(gate_ids) != len(set(gate_ids)):
        errors.append("required_gates does not contain the exact gate set")
        return
    known_references = {
        item.get("id")
        for item in manifest.get("repository_evidence", [])
        if isinstance(item, dict)
    }
    known_references.update(
        item.get("id")
        for item in manifest.get("remote_logs", [])
        if isinstance(item, dict)
    )
    known_references.update(
        f"run:{item.get('run_id')}"
        for item in manifest.get("remote_runs", [])
        if isinstance(item, dict)
    )
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
        elif isinstance(evidence, list):
            if len(evidence) != len(set(evidence)):
                errors.append(f"gate {gate_id} evidence is duplicated")
            for reference in evidence:
                if reference not in known_references:
                    errors.append(
                        f"gate {gate_id} has dangling evidence reference: {reference}"
                    )
        if gate_id == "independent-review-activation":
            if (
                gate.get("result") != "not_applicable"
                or gate.get("command") is not None
                or evidence != []
            ):
                errors.append("independent review must remain an external activation gate")
        elif gate.get("result") != "green":
            errors.append(f"mandatory gate {gate_id} is not green")


def validate_status_documents(
    root: Path,
    validation_subject: str,
    errors: list[str],
) -> None:
    for relative_path in (
        "README.md",
        "docs/software/software-requirements-specification.md",
        "docs/software/roadmap.md",
    ):
        try:
            content = git(
                root,
                ["show", f"{validation_subject}:{relative_path}"],
                binary=True,
            ).decode("utf-8")
        except (UnicodeError, ValueError) as error:
            errors.append(str(error))
            continue
        normalized = " ".join(content.split())
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

    bundle = load_json(root / "tests/specifications/s0/contract-bundle.json")
    files = bundle.get("files")
    if not isinstance(files, list):
        errors.append("S0 contract bundle files are invalid")
        return
    evidence_entries = [
        entry
        for entry in files
        if isinstance(entry, dict)
        and entry.get("path")
        == "tests/specifications/s0/evidence-manifest-v1.schema.json"
    ]
    if evidence_entries != [
        {
            "path": "tests/specifications/s0/evidence-manifest-v1.schema.json",
            "sha256": CHECKPOINT_CONTRACT_DIGESTS[
                "tests/specifications/s0/evidence-manifest-v1.schema.json"
            ],
        }
    ]:
        errors.append("S0 contract bundle does not bind the additive evidence schema")
    bundle_payload = {
        "schema_version": bundle.get("schema_version"),
        "files": files,
    }
    bundle_digest = sha256_bytes(canonical_json(bundle_payload))
    if bundle.get("bundle_sha256") != bundle_digest:
        errors.append("S0 contract bundle digest does not match its canonical payload")
    if bundle_digest != CURRENT_S0_BUNDLE_SHA256:
        errors.append("S0 contract bundle differs from the authorized correction")
    srs = (root / "docs/software/software-requirements-specification.md").read_text(
        encoding="utf-8"
    )
    if f"S0 contract bundle: `sha256:{CURRENT_S0_BUNDLE_SHA256}`" not in srs:
        errors.append("SRS does not bind the corrected S0 contract bundle")

    legacy_bundle_bytes = git(
        root,
        ["show", f"{BASE_SHA}:tests/specifications/s0/contract-bundle.json"],
        binary=True,
    )
    try:
        legacy_bundle = json.loads(legacy_bundle_bytes.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError):
        errors.append("legacy S0 contract bundle is not valid UTF-8 JSON")
        return
    legacy_payload = {
        "schema_version": legacy_bundle.get("schema_version"),
        "files": legacy_bundle.get("files"),
    }
    legacy_digest = sha256_bytes(canonical_json(legacy_payload))
    if (
        legacy_bundle.get("bundle_sha256") != LEGACY_S0_BUNDLE_SHA256
        or legacy_digest != LEGACY_S0_BUNDLE_SHA256
    ):
        errors.append("exact base does not retain the historical S0 bundle")
    legacy_schema_bytes = git(
        root,
        [
            "show",
            f"{BASE_SHA}:tests/specifications/s0/evidence-manifest-v1.schema.json",
        ],
        binary=True,
    )
    if sha256_bytes(legacy_schema_bytes) != (
        "5e3746d5d97a170959b87f24a1eeb9f3422d50b1d5f4ccdb0763cb30502f8743"
    ):
        errors.append("exact base does not retain the historical S0 evidence schema")

    historical_evidence = {
        "crates/noesis/tests/evidence/s0/red-observation-corrected-contract.json": (
            "1f12680a8f5127bb853ea289a50f6d9751063dc736b9d2ec74c77c42e0f439b4"
        ),
        "crates/noesis/tests/evidence/s0/green-observation-local.json": (
            "16260290deb395355041e3acfb4451deee21b46b76e209c0655f4aadbc2d3561"
        ),
    }
    for relative_path, expected_digest in historical_evidence.items():
        current_bytes = (root / relative_path).read_bytes()
        base_bytes = git(
            root,
            ["show", f"{BASE_SHA}:{relative_path}"],
            binary=True,
        )
        if current_bytes != base_bytes or sha256_bytes(current_bytes) != expected_digest:
            errors.append(f"historical S0 evidence changed: {relative_path}")
            continue
        evidence = json.loads(current_bytes.decode("utf-8"))
        if evidence.get("oracle_bundle_sha256") != LEGACY_S0_BUNDLE_SHA256:
            errors.append(f"historical S0 evidence lost its bundle identity: {relative_path}")


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


def validate_environment(root: Path, manifest: dict[str, Any], errors: list[str]) -> None:
    environment = manifest.get("environment")
    if not isinstance(environment, dict):
        errors.append("environment must be an object")
        return
    expected_file_digests = {
        "toolchain_file_sha256": sha256_file(root / "rust-toolchain.toml"),
        "policy_sha256": sha256_file(root / ".github/codex/policy.json"),
    }
    for field, expected in expected_file_digests.items():
        if environment.get(field) != expected:
            errors.append(f"environment {field} does not match repository bytes")
    if environment.get("run_identifier") != "issue-188-local-baseline-verification-v2":
        errors.append("environment run identifier is invalid")


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

    manifest_schema = load_json(root / MANIFEST_SCHEMA_PATH)
    if not isinstance(manifest_schema, dict):
        return ["manifest schema must be a JSON object"]
    validate_json_schema(manifest, manifest_schema, manifest_schema, "$", errors)

    plan = load_json(root / PLAN_PATH)
    catalog = load_json(root / CATALOG_PATH)
    if not isinstance(plan, dict) or not isinstance(catalog, dict):
        return ["plan and catalog must be JSON objects"]

    history_error_count = len(errors)
    ensure_required_git_history(root, errors)
    if len(errors) != history_error_count:
        return errors
    activation = load_json(root / ACTIVATION_RECORD_PATH)
    if not isinstance(activation, dict):
        return [*errors, "post-merge activation record must be an object"]
    current_head = git(root, ["rev-parse", "HEAD"])
    validation_subject = resolve_validation_subject(
        root,
        current_head,
        activation,
        errors,
    )
    if validation_subject is None:
        return errors
    validate_checkpoint_contracts(root, errors)
    validate_authority(root, manifest, plan, validation_subject, errors)
    validate_changed_paths(root, validation_subject, errors)
    validate_path_digest(root, manifest.get("plan"), "plan", errors)
    validate_path_digest(root, manifest.get("profile_catalog"), "profile_catalog", errors)
    validate_product_tree(root, manifest, plan, validation_subject, errors)
    validate_repository_evidence(root, manifest, errors)
    validate_full_gate_evidence(root, manifest, validation_subject, errors)
    metadata_by_id = validate_remote_runs(root, manifest, errors)
    validate_remote_logs(root, manifest, metadata_by_id, errors)
    validate_codeql_log(root, manifest, metadata_by_id, errors)
    validate_catalog(root, manifest, plan, catalog, errors)
    validate_gates(manifest, errors)
    validate_status_documents(root, validation_subject, errors)
    validate_s0_evidence_contract(root, errors)
    validate_review(manifest, errors)
    validate_environment(root, manifest, errors)
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
