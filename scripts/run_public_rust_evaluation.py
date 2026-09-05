#!/usr/bin/env python3
"""Evaluate CodeNoesis progressively over a pinned public Rust corpus."""

from __future__ import annotations

import argparse
import collections
import hashlib
import json
import math
import os
import platform
import re
import shutil
import signal
import stat
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any, BinaryIO, NoReturn, Sequence


RUNNER_VERSION = "codenoesis.public-rust-evaluation-runner/v1"
REPORT_SCHEMA = "codenoesis.public-rust-evaluation-report/v1"
ERROR_SCHEMA = "codenoesis.public-rust-evaluation-error/v1"
SUITE_ID = "rust-public-conference-v1"
STAGES = (
    "acquisition",
    "workspace",
    "manifest",
    "semantic",
    "framework",
    "flow",
    "constant",
)
HEX_40 = re.compile(r"^[0-9a-f]{40}$")
HOST_PROFILE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$")
ENTRY_ID = re.compile(r"^[a-z0-9][a-z0-9-]{0,63}$")
SNAPSHOT_BYTES_MAX = 2_147_483_648
ERROR_BYTES_MAX = 4_096
ENABLED_EXTRACTORS = ["rust-progressive-s1-r16-source-only"]

PROFILE_PAIRS = (
    ("workspace", "--workspace-profile", "cargo-root-package-v1"),
    ("manifest", "--manifest-profile", "cargo-manifest-facts-v1"),
    (
        "semantic",
        "--rust-semantic-profile",
        "rust-cfg-declaration-alternatives-v1",
    ),
    ("framework", "--rust-framework-profile", "rust-framework-declarations-v1"),
    ("flow", "--rust-callable-profile", "rust-callable-semantics-v1"),
    ("flow", "--rust-expression-profile", "rust-expression-bindings-v1"),
    ("flow", "--rust-flow-profile", "rust-local-flow-v1"),
    ("constant", "--rust-constant-profile", "rust-safe-constant-evaluation-v1"),
)


class EvaluationError(Exception):
    """Expected fail-closed evaluation error."""

    def __init__(
        self,
        code: str,
        message: str,
        *,
        stage: str = "input",
        exit_code: int = 2,
        context: dict[str, Any] | None = None,
    ):
        super().__init__(message)
        self.code = code
        self.message = message
        self.stage = stage
        self.exit_code = exit_code
        self.context = context or {}


class FailClosedParser(argparse.ArgumentParser):
    """Convert argparse output into the stable typed error protocol."""

    def error(self, message: str) -> NoReturn:
        del message
        raise EvaluationError("evaluation.invalid_arguments", "invalid command-line arguments")


def canonical_json_bytes(value: Any) -> bytes:
    try:
        encoded = json.dumps(
            value,
            ensure_ascii=False,
            allow_nan=False,
            separators=(",", ":"),
            sort_keys=True,
        ).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise EvaluationError(
            "evaluation.internal",
            "evaluation data cannot be serialized",
            stage="internal",
            exit_code=1,
        ) from error
    return encoded + b"\n"


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
    except OSError as error:
        raise EvaluationError("evaluation.invalid_input", "cannot read input file") from error
    return digest.hexdigest()


def load_json(path: Path, *, maximum_bytes: int = 2_097_152) -> Any:
    try:
        if path.stat().st_size > maximum_bytes:
            raise EvaluationError("evaluation.limit_exceeded", "JSON input exceeds byte limit")
        with path.open("r", encoding="utf-8") as handle:
            return json.load(handle)
    except EvaluationError:
        raise
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvaluationError("evaluation.invalid_input", "invalid JSON input") from error


def require_object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise EvaluationError("evaluation.invalid_contract", f"{label} must be an object")
    return value


def require_exact_keys(value: dict[str, Any], keys: set[str], label: str) -> None:
    if set(value) != keys:
        raise EvaluationError("evaluation.invalid_contract", f"{label} fields do not match")


def load_contracts(
    corpus_path: Path, policy_path: Path, oracle_path: Path
) -> tuple[dict[str, Any], dict[str, Any], dict[str, Any]]:
    corpus = require_object(load_json(corpus_path), "corpus")
    policy = require_object(load_json(policy_path), "policy")
    oracle = require_object(load_json(oracle_path), "oracle")
    require_exact_keys(
        corpus,
        {
            "schema_version",
            "id",
            "version",
            "selection_method",
            "selection_frozen_at",
            "source_mode",
            "network_allowed",
            "source_vendored",
            "entries",
        },
        "corpus",
    )
    if (
        corpus["schema_version"] != "codenoesis.public-rust-evaluation-corpus/v1"
        or corpus["id"] != "public-rust-conference"
        or corpus["version"] != "1"
        or corpus["source_mode"] != "caller_supplied_full_non_shallow_local_clone"
        or corpus["network_allowed"] is not False
        or corpus["source_vendored"] is not False
    ):
        raise EvaluationError("evaluation.invalid_contract", "corpus identity is invalid")
    entries = corpus.get("entries")
    if not isinstance(entries, list) or len(entries) < 3:
        raise EvaluationError("evaluation.invalid_contract", "corpus entries are invalid")
    entry_ids = []
    for entry in entries:
        descriptor = require_object(entry, "corpus entry")
        require_exact_keys(
            descriptor,
            {
                "id",
                "repository_url",
                "revision",
                "tree",
                "observed_license",
                "archetype",
                "tree_entries",
                "rust_source_files",
                "rust_source_bytes",
                "repository_id",
            },
            "corpus entry",
        )
        entry_id = descriptor.get("id")
        if not isinstance(entry_id, str) or ENTRY_ID.fullmatch(entry_id) is None:
            raise EvaluationError("evaluation.invalid_contract", "corpus entry id is invalid")
        if entry_id in entry_ids:
            raise EvaluationError("evaluation.invalid_contract", "corpus entry id is duplicated")
        entry_ids.append(entry_id)
        if (
            not isinstance(descriptor.get("repository_url"), str)
            or not descriptor["repository_url"].startswith("https://github.com/")
            or not descriptor["repository_url"].endswith(".git")
            or not isinstance(descriptor.get("revision"), str)
            or HEX_40.fullmatch(descriptor["revision"]) is None
            or not isinstance(descriptor.get("tree"), str)
            or HEX_40.fullmatch(descriptor["tree"]) is None
            or descriptor.get("repository_id")
            != f"urn:codenoesis:benchmark:{entry_id}:conference-v1"
        ):
            raise EvaluationError("evaluation.invalid_contract", "corpus source identity is invalid")
        for field in ("tree_entries", "rust_source_files", "rust_source_bytes"):
            if not isinstance(descriptor.get(field), int) or descriptor[field] < 0:
                raise EvaluationError("evaluation.invalid_contract", "corpus source metrics are invalid")

    expected_policy = {
        "schema_version": "codenoesis.public-rust-evaluation-policy/v1",
        "suite_id": SUITE_ID,
        "claim_scope": "observational_pinned_corpus_same_host",
        "stage_order": list(STAGES),
        "repetitions": 3,
        "concurrency": 1,
        "cache_state": "cold",
        "percentile_method": "nearest-rank",
        "minimum_oracle_match_rate": 1.0,
        "minimum_constant_stage_successes": 3,
        "sample_timeout_seconds": 150,
        "report_bytes_max": 8_388_608,
        "network_allowed": False,
        "failed_sample_retry_allowed": False,
        "cross_host_comparison_allowed": False,
        "slo_claimed": False,
        "release_claimed": False,
        "ga_claimed": False,
    }
    if policy != expected_policy:
        raise EvaluationError("evaluation.invalid_contract", "evaluation policy is invalid")
    require_exact_keys(
        oracle,
        {"schema_version", "suite_id", "baseline_product_commit", "entries"},
        "oracle",
    )
    if (
        oracle["schema_version"] != "codenoesis.public-rust-evaluation-oracle/v1"
        or oracle["suite_id"] != SUITE_ID
        or not isinstance(oracle["baseline_product_commit"], str)
        or HEX_40.fullmatch(oracle["baseline_product_commit"]) is None
        or not isinstance(oracle.get("entries"), dict)
        or set(oracle["entries"]) != set(entry_ids)
    ):
        raise EvaluationError("evaluation.invalid_contract", "oracle identity is invalid")
    for entry_id, expected in oracle["entries"].items():
        expected = require_object(expected, f"oracle entry {entry_id}")
        terminal_stage = expected.get("terminal_stage")
        highest = expected.get("highest_successful_stage")
        outcome = expected.get("outcome")
        if terminal_stage not in STAGES or highest not in (*STAGES, None):
            raise EvaluationError("evaluation.invalid_contract", "oracle stage is invalid")
        terminal_index = STAGES.index(terminal_stage)
        if outcome == "success":
            if terminal_stage != STAGES[-1] or highest != terminal_stage:
                raise EvaluationError("evaluation.invalid_contract", "success oracle stage is invalid")
            required = {
                "highest_successful_stage",
                "terminal_stage",
                "outcome",
                "snapshot_schema",
                "semantic_hash",
                "semantic_projection_sha256",
                "counts",
            }
            require_exact_keys(expected, required, f"oracle entry {entry_id}")
        elif outcome == "typed_rejection":
            expected_highest = STAGES[terminal_index - 1] if terminal_index else None
            if highest != expected_highest:
                raise EvaluationError("evaluation.invalid_contract", "rejection oracle stage is invalid")
            required = {
                "highest_successful_stage",
                "terminal_stage",
                "outcome",
                "exit_code",
                "error_schema",
                "error_code",
                "error_stage",
                "error_context",
            }
            require_exact_keys(expected, required, f"oracle entry {entry_id}")
        else:
            raise EvaluationError("evaluation.invalid_contract", "oracle outcome is invalid")
    return corpus, policy, oracle


def build_scan_command(
    binary: Path,
    repository: Path,
    store: Path,
    descriptor: dict[str, Any],
    stage: str,
) -> list[str]:
    if stage not in STAGES:
        raise EvaluationError("evaluation.invalid_arguments", "unknown evaluation stage")
    command = [
        str(binary),
        "scan",
        "--repository",
        str(repository),
        "--repository-id",
        descriptor["repository_id"],
        "--revision",
        descriptor["revision"],
        "--profile",
        "standard-local-s1" if stage == "acquisition" else "standard-local-s4",
        "--acquisition-profile",
        "local-git-sha1-packed-rust-8m-v1",
    ]
    if stage == "acquisition":
        command.extend(("--format", "json"))
        return command
    stage_index = STAGES.index(stage)
    for profile_stage, flag, value in PROFILE_PAIRS:
        if STAGES.index(profile_stage) <= stage_index:
            if flag == "--rust-semantic-profile" and stage == "framework":
                value = "rust-semantic-depth-v1"
            command.extend((flag, value))
    if stage_index >= STAGES.index("flow"):
        command.extend(("--output-capacity-profile", "local-snapshot-2g-v1"))
    if stage == "constant":
        command.extend(("--execution-limit-profile", "real-world-rust-benchmark-75s-v1"))
    command.extend(("--store", str(store), "--format", "json"))
    return command


def nearest_rank(values: Sequence[int], percentile: int) -> int:
    if not values or percentile < 1 or percentile > 100:
        raise EvaluationError("evaluation.internal", "invalid percentile input", stage="internal", exit_code=1)
    ordered = sorted(values)
    rank = math.ceil(percentile * len(ordered) / 100)
    return ordered[rank - 1]


def semantic_projection(snapshot: dict[str, Any]) -> bytes:
    try:
        return json.dumps(
            snapshot["semantic"],
            ensure_ascii=False,
            allow_nan=False,
            separators=(",", ":"),
        ).encode("utf-8")
    except (KeyError, TypeError, ValueError) as error:
        raise EvaluationError("evaluation.oracle_mismatch", "semantic projection is invalid") from error


def basis_points(numerator: int, denominator: int) -> int:
    return 10_000 * numerator // max(1, denominator)


def graph_metrics(snapshot: dict[str, Any]) -> dict[str, Any]:
    try:
        graph = snapshot["semantic"]["knowledge_graph"]
        entities = graph["entities"]
        relationships = graph["relationships"]
        claims = graph["claims"]
        evidence = graph["evidence"]
        diagnostics = graph["diagnostics"]
        coverage = graph["coverage"]
        entity_kinds = collections.Counter(entity.get("kind") for entity in entities)
        relationship_kinds = collections.Counter(
            relationship.get("kind") for relationship in relationships
        )
        coverage_states = collections.Counter(item.get("state") for item in coverage)
        diagnostic_codes = collections.Counter(item.get("code") for item in diagnostics)
    except (KeyError, TypeError) as error:
        raise EvaluationError("evaluation.oracle_mismatch", "knowledge graph is invalid") from error
    callables = entity_kinds["rust.function"] + entity_kinds["rust.method"]
    signatures = entity_kinds["rust.callable_signature"]
    call_sites = entity_kinds["rust.call_site"]
    resolved_calls = relationship_kinds["CALLS"]
    counts = {
        "entities": len(entities),
        "relationships": len(relationships),
        "claims": len(claims),
        "evidence": len(evidence),
        "diagnostics": len(diagnostics),
        "coverage": len(coverage),
        "evaluated_values": entity_kinds["rust.evaluated_value"],
    }
    information = {
        "callables": callables,
        "signatures": signatures,
        "parameters": entity_kinds["rust.parameter"],
        "enum_types": entity_kinds["rust.enum"],
        "enum_variants": entity_kinds["rust.enum_variant"],
        "declared_values": entity_kinds["rust.declared_value"],
        "evaluated_values": entity_kinds["rust.evaluated_value"],
        "call_sites": call_sites,
        "resolved_calls": resolved_calls,
        "expressions": entity_kinds["rust.expression"],
        "local_bindings": entity_kinds["rust.local_binding"]
        + entity_kinds["rust.pattern_binding"],
        "syntax_basic_blocks": entity_kinds["rust.syntax_basic_block"],
        "signature_coverage_basis_points": basis_points(signatures, callables),
        "resolved_call_basis_points": basis_points(resolved_calls, call_sites),
        "claim_evidence_basis_points": basis_points(
            sum(bool(claim.get("evidence_ids")) for claim in claims), len(claims)
        ),
        "coverage_states": dict(sorted(coverage_states.items())),
        "diagnostic_codes": dict(sorted(diagnostic_codes.items())),
    }
    return {"counts": counts, "information": information}


def terminal_sample_matches(sample: dict[str, Any], expected: dict[str, Any]) -> bool:
    if sample.get("outcome") != expected.get("outcome"):
        return False
    if expected["outcome"] == "success":
        fields = (
            "snapshot_schema",
            "semantic_hash",
            "semantic_projection_sha256",
            "counts",
        )
    else:
        fields = (
            "exit_code",
            "error_schema",
            "error_code",
            "error_stage",
            "error_context",
        )
    return all(sample.get(field) == expected.get(field) for field in fields)


def aggregate_stage_coverage(
    entries: dict[str, dict[str, Any]], stage_order: Sequence[str]
) -> dict[str, dict[str, int]]:
    total = len(entries)
    coverage = {}
    for stage_index, stage in enumerate(stage_order):
        count = sum(
            entry.get("highest_successful_stage") in stage_order
            and stage_order.index(entry["highest_successful_stage"]) >= stage_index
            for entry in entries.values()
        )
        coverage[stage] = {"count": count, "basis_points": basis_points(count, total)}
    return coverage


def ensure_regular_file(path: Path, label: str, *, executable: bool = False) -> Path:
    try:
        metadata = path.lstat()
    except OSError as error:
        raise EvaluationError("evaluation.invalid_input", f"{label} is unavailable") from error
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
        raise EvaluationError("evaluation.invalid_input", f"{label} must be a regular file")
    if executable and not os.access(path, os.X_OK):
        raise EvaluationError("evaluation.invalid_input", f"{label} must be executable")
    return path.resolve(strict=True)


def ensure_directory(path: Path, label: str) -> Path:
    try:
        metadata = path.lstat()
    except OSError as error:
        raise EvaluationError("evaluation.invalid_input", f"{label} is unavailable") from error
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
        raise EvaluationError("evaluation.invalid_input", f"{label} must be a directory")
    return path.resolve(strict=True)


def git_environment(home: Path) -> dict[str, str]:
    environment = {
        "HOME": str(home),
        "XDG_CONFIG_HOME": str(home),
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CONFIG_GLOBAL": os.devnull,
        "GIT_TERMINAL_PROMPT": "0",
        "GIT_OPTIONAL_LOCKS": "0",
        "LC_ALL": "C",
        "LANG": "C",
    }
    for name in ("SystemRoot", "SystemDrive", "WINDIR", "COMSPEC", "PATHEXT", "TEMP", "TMP"):
        if name in os.environ:
            environment[name] = os.environ[name]
    return environment


def run_git(repository: Path, arguments: Sequence[str], home: Path) -> bytes:
    git = shutil.which("git")
    if git is None:
        raise EvaluationError("evaluation.invalid_input", "system Git is unavailable")
    try:
        result = subprocess.run(
            [git, "-c", "core.hooksPath=", *arguments],
            cwd=repository,
            env=git_environment(home),
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=30,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise EvaluationError("evaluation.invalid_input", "local Git inspection failed") from error
    if result.returncode != 0:
        raise EvaluationError("evaluation.invalid_input", "local Git inspection rejected repository")
    return result.stdout


def git_text(repository: Path, arguments: Sequence[str], home: Path) -> str:
    try:
        return run_git(repository, arguments, home).decode("utf-8").strip()
    except UnicodeError as error:
        raise EvaluationError("evaluation.invalid_input", "local Git output is not UTF-8") from error


def source_metrics(repository: Path, revision: str, home: Path) -> dict[str, int]:
    output = git_text(repository, ["ls-tree", "-r", "-t", "-l", revision], home)
    tree_entries = 0
    rust_source_files = 0
    rust_source_bytes = 0
    for line in output.splitlines():
        metadata, separator, path = line.partition("\t")
        fields = metadata.split()
        if not separator or len(fields) != 4:
            raise EvaluationError("evaluation.invalid_input", "Git tree listing is invalid")
        tree_entries += 1
        if fields[1] == "blob" and path.endswith(".rs"):
            if fields[3] == "-":
                raise EvaluationError("evaluation.invalid_input", "Rust blob size is unavailable")
            rust_source_files += 1
            rust_source_bytes += int(fields[3])
    return {
        "tree_entries": tree_entries,
        "rust_source_files": rust_source_files,
        "rust_source_bytes": rust_source_bytes,
    }


def preflight_repository(
    repository: Path, descriptor: dict[str, Any], home: Path
) -> tuple[Path, dict[str, int]]:
    repository = ensure_directory(repository, f"{descriptor['id']} repository")
    git_directory = repository / ".git"
    ensure_directory(git_directory, f"{descriptor['id']} Git directory")
    if git_text(repository, ["rev-parse", "--is-shallow-repository"], home) != "false":
        raise EvaluationError("evaluation.invalid_input", "repository must not be shallow")
    if git_text(repository, ["rev-parse", "HEAD"], home) != descriptor["revision"]:
        raise EvaluationError("evaluation.repository_mismatch", "repository HEAD does not match corpus")
    if (
        git_text(repository, ["rev-parse", f"{descriptor['revision']}^{{commit}}"], home)
        != descriptor["revision"]
        or git_text(repository, ["rev-parse", f"{descriptor['revision']}^{{tree}}"], home)
        != descriptor["tree"]
    ):
        raise EvaluationError("evaluation.repository_mismatch", "repository identity does not match corpus")
    observed = source_metrics(repository, descriptor["revision"], home)
    expected = {field: descriptor[field] for field in observed}
    if observed != expected:
        raise EvaluationError("evaluation.repository_mismatch", "repository source metrics do not match corpus")
    return repository, observed


def benchmark_environment(home: Path) -> dict[str, str]:
    environment = git_environment(home)
    environment.update(
        {
            "NO_COLOR": "1",
            "CLICOLOR": "0",
            "RUST_BACKTRACE": "0",
            "http_proxy": "",
            "https_proxy": "",
            "HTTP_PROXY": "",
            "HTTPS_PROXY": "",
            "ALL_PROXY": "",
            "NO_PROXY": "*",
        }
    )
    return environment


def wait_for_process(
    command: Sequence[str],
    stdout_handle: BinaryIO,
    stderr_handle: BinaryIO,
    timeout_seconds: int,
    home: Path,
) -> tuple[int, int]:
    options: dict[str, Any] = {}
    if os.name == "posix":
        options["start_new_session"] = True
    elif os.name == "nt":
        options["creationflags"] = subprocess.CREATE_NEW_PROCESS_GROUP
    started = time.monotonic_ns()
    try:
        process = subprocess.Popen(
            command,
            stdin=subprocess.DEVNULL,
            stdout=stdout_handle,
            stderr=stderr_handle,
            env=benchmark_environment(home),
            close_fds=True,
            **options,
        )
    except OSError as error:
        raise EvaluationError("evaluation.sample_failed", "cannot start product binary") from error
    try:
        return_code = process.wait(timeout=timeout_seconds)
    except subprocess.TimeoutExpired as error:
        if os.name == "posix":
            os.killpg(process.pid, signal.SIGKILL)
        else:
            process.kill()
        process.wait()
        raise EvaluationError("evaluation.timeout", "evaluation sample exceeded timeout") from error
    return return_code, time.monotonic_ns() - started


def parse_success(stdout_path: Path, stderr_path: Path) -> dict[str, Any]:
    if stderr_path.stat().st_size != 0:
        raise EvaluationError("evaluation.oracle_mismatch", "successful sample wrote stderr")
    if stdout_path.stat().st_size > SNAPSHOT_BYTES_MAX:
        raise EvaluationError("evaluation.limit_exceeded", "snapshot exceeds byte limit")
    snapshot = require_object(load_json(stdout_path, maximum_bytes=SNAPSHOT_BYTES_MAX), "snapshot")
    semantic_hash = snapshot.get("semantic_hash")
    if (
        not isinstance(semantic_hash, dict)
        or semantic_hash.get("algorithm") != "blake3-256"
        or not isinstance(semantic_hash.get("value"), str)
        or len(semantic_hash["value"]) != 64
    ):
        raise EvaluationError("evaluation.oracle_mismatch", "snapshot semantic hash is invalid")
    parsed = {
        "outcome": "success",
        "snapshot_schema": snapshot.get("schema_version"),
        "semantic_hash": semantic_hash["value"],
        "semantic_projection_sha256": sha256_bytes(semantic_projection(snapshot)),
    }
    semantic = snapshot.get("semantic")
    if isinstance(semantic, dict) and isinstance(semantic.get("knowledge_graph"), dict):
        parsed.update(graph_metrics(snapshot))
    return parsed


def parse_rejection(return_code: int, stdout_path: Path, stderr_path: Path) -> dict[str, Any]:
    if stdout_path.stat().st_size != 0:
        raise EvaluationError("evaluation.oracle_mismatch", "rejected sample wrote stdout")
    if stderr_path.stat().st_size == 0 or stderr_path.stat().st_size > ERROR_BYTES_MAX:
        raise EvaluationError("evaluation.oracle_mismatch", "rejected sample stderr is invalid")
    error = require_object(load_json(stderr_path, maximum_bytes=ERROR_BYTES_MAX), "product error")
    if canonical_json_bytes(error) != stderr_path.read_bytes():
        raise EvaluationError("evaluation.oracle_mismatch", "product error is not canonical")
    if set(error) != {"schema_version", "code", "stage", "message", "retryable", "context"}:
        raise EvaluationError("evaluation.oracle_mismatch", "product error fields are invalid")
    if error.get("retryable") is not False or not isinstance(error.get("context"), dict):
        raise EvaluationError("evaluation.oracle_mismatch", "product error contract is invalid")
    return {
        "outcome": "typed_rejection",
        "exit_code": return_code,
        "error_schema": error.get("schema_version"),
        "error_code": error.get("code"),
        "error_stage": error.get("stage"),
        "error_context": error.get("context"),
    }


def run_sample(
    binary: Path,
    repository: Path,
    descriptor: dict[str, Any],
    stage: str,
    index: int,
    scratch: Path,
    timeout_seconds: int,
    home: Path,
) -> dict[str, Any]:
    sample_root = scratch / f"{descriptor['id']}-{stage}-{index}"
    sample_root.mkdir()
    stdout_path = sample_root / "stdout.json"
    stderr_path = sample_root / "stderr.json"
    store = sample_root / "store"
    command = build_scan_command(binary, repository, store, descriptor, stage)
    with stdout_path.open("wb") as stdout_handle, stderr_path.open("wb") as stderr_handle:
        return_code, wall_time_ns = wait_for_process(
            command,
            stdout_handle,
            stderr_handle,
            timeout_seconds,
            home,
        )
    common = {
        "index": index,
        "stage": stage,
        "exit_code": return_code,
        "wall_time_ns": wall_time_ns,
        "stdout_bytes": stdout_path.stat().st_size,
        "stderr_bytes": stderr_path.stat().st_size,
    }
    parsed = (
        parse_success(stdout_path, stderr_path)
        if return_code == 0
        else parse_rejection(return_code, stdout_path, stderr_path)
    )
    return {**common, **parsed}


def sample_summary(sample: dict[str, Any]) -> dict[str, Any]:
    fields = [
        "index",
        "stage",
        "outcome",
        "exit_code",
        "wall_time_ns",
        "stdout_bytes",
        "stderr_bytes",
    ]
    if sample["outcome"] == "success":
        fields.extend(("snapshot_schema", "semantic_hash", "semantic_projection_sha256"))
        if "counts" in sample:
            fields.extend(("counts", "information"))
    else:
        fields.extend(("error_schema", "error_code", "error_stage", "error_context"))
    return {field: sample[field] for field in fields}


def evaluate_entry(
    binary: Path,
    repository: Path,
    descriptor: dict[str, Any],
    expected: dict[str, Any],
    policy: dict[str, Any],
    scratch: Path,
    home: Path,
) -> dict[str, Any]:
    stage_results = []
    terminal_sample = None
    for stage in STAGES:
        sample = run_sample(
            binary,
            repository,
            descriptor,
            stage,
            1,
            scratch,
            policy["sample_timeout_seconds"],
            home,
        )
        stage_results.append(sample_summary(sample))
        if stage == expected["terminal_stage"]:
            terminal_sample = sample
            break
        if sample["outcome"] != "success":
            raise EvaluationError(
                "evaluation.oracle_mismatch",
                "entry failed before terminal stage",
                context={
                    "entry": descriptor["id"],
                    "stage": stage,
                    "error_code": sample.get("error_code"),
                },
            )
    if terminal_sample is None or not terminal_sample_matches(terminal_sample, expected):
        raise EvaluationError(
            "evaluation.oracle_mismatch",
            "terminal sample differs from oracle",
            context={"entry": descriptor["id"], "stage": expected["terminal_stage"]},
        )
    terminal_samples = [sample_summary(terminal_sample)]
    for index in range(2, policy["repetitions"] + 1):
        sample = run_sample(
            binary,
            repository,
            descriptor,
            expected["terminal_stage"],
            index,
            scratch,
            policy["sample_timeout_seconds"],
            home,
        )
        if not terminal_sample_matches(sample, expected):
            raise EvaluationError(
                "evaluation.oracle_mismatch",
                "terminal replay differs from oracle",
                context={
                    "entry": descriptor["id"],
                    "stage": expected["terminal_stage"],
                    "sample": index,
                },
            )
        terminal_samples.append(sample_summary(sample))
    wall_times = [sample["wall_time_ns"] for sample in terminal_samples]
    return {
        "repository_id": descriptor["repository_id"],
        "revision": descriptor["revision"],
        "tree": descriptor["tree"],
        "archetype": descriptor["archetype"],
        "observed_license": descriptor["observed_license"],
        "source_metrics": {
            "tree_entries": descriptor["tree_entries"],
            "rust_source_files": descriptor["rust_source_files"],
            "rust_source_bytes": descriptor["rust_source_bytes"],
        },
        "highest_successful_stage": expected["highest_successful_stage"],
        "terminal_stage": expected["terminal_stage"],
        "terminal_outcome": expected["outcome"],
        "stage_results": stage_results,
        "terminal_samples": terminal_samples,
        "terminal_percentiles_ns": {
            "p50": nearest_rank(wall_times, 50),
            "p95": nearest_rank(wall_times, 95),
            "p99": nearest_rank(wall_times, 99),
        },
        "oracle_match_rate": 1.0,
    }


def aggregate_report(entries: dict[str, dict[str, Any]]) -> dict[str, Any]:
    stage_entries = {
        entry_id: {"highest_successful_stage": entry["highest_successful_stage"]}
        for entry_id, entry in entries.items()
    }
    constant_entries = [
        entry
        for entry in entries.values()
        if entry["highest_successful_stage"] == "constant"
    ]
    totals = collections.Counter()
    information = collections.Counter()
    for entry in constant_entries:
        first = entry["terminal_samples"][0]
        totals.update(first["counts"])
        for key, value in first["information"].items():
            if isinstance(value, int) and not key.endswith("basis_points"):
                information[key] += value
    return {
        "repositories": len(entries),
        "constant_stage_successes": len(constant_entries),
        "typed_rejections": sum(
            entry["terminal_outcome"] == "typed_rejection" for entry in entries.values()
        ),
        "oracle_matches": len(entries),
        "oracle_match_rate": 1.0,
        "stage_coverage": aggregate_stage_coverage(stage_entries, STAGES),
        "constant_stage_graph_totals": dict(sorted(totals.items())),
        "constant_stage_information_totals": dict(sorted(information.items())),
    }


def host_record(profile: str) -> dict[str, Any]:
    return {
        "profile": profile,
        "operating_system": platform.system().lower() or "unknown",
        "architecture": platform.machine().lower() or "unknown",
        "logical_cpu_count": os.cpu_count() or 0,
    }


def validate_manifest(manifest_path: Path) -> dict[str, Any]:
    manifest = require_object(load_json(manifest_path), "manifest")
    suites = manifest.get("suites")
    if not isinstance(suites, list):
        raise EvaluationError("evaluation.invalid_contract", "manifest suites are invalid")
    matches = [suite for suite in suites if isinstance(suite, dict) and suite.get("id") == SUITE_ID]
    if len(matches) != 1:
        raise EvaluationError("evaluation.invalid_contract", "evaluation suite is absent from manifest")
    return manifest


def validate_product_commit(product_commit: str, baseline_product_commit: str) -> None:
    if product_commit != baseline_product_commit:
        raise EvaluationError(
            "evaluation.product_mismatch",
            "product commit does not match the frozen oracle baseline",
        )


def publish_new_file(path: Path, content: bytes) -> None:
    if path.exists() or path.is_symlink():
        raise EvaluationError("evaluation.invalid_output", "output destination already exists")
    parent = ensure_directory(path.parent, "output parent")
    temporary = parent / f".{path.name}.tmp-{os.getpid()}"
    if temporary.exists() or temporary.is_symlink():
        raise EvaluationError("evaluation.invalid_output", "temporary output already exists")
    try:
        with temporary.open("xb") as handle:
            handle.write(content)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, parent / path.name)
    except OSError as error:
        try:
            temporary.unlink()
        except OSError:
            pass
        raise EvaluationError("evaluation.invalid_output", "cannot publish evaluation report") from error


def run_evaluation(arguments: argparse.Namespace) -> dict[str, Any]:
    if arguments.suite != SUITE_ID:
        raise EvaluationError("evaluation.invalid_arguments", "unknown evaluation suite")
    if HOST_PROFILE.fullmatch(arguments.host_profile) is None:
        raise EvaluationError("evaluation.invalid_arguments", "invalid host profile")
    root = Path(__file__).resolve().parents[1]
    manifest_path = ensure_regular_file(Path(arguments.manifest), "manifest")
    corpus_path = ensure_regular_file(Path(arguments.corpus), "corpus")
    policy_path = ensure_regular_file(Path(arguments.policy), "policy")
    oracle_path = ensure_regular_file(Path(arguments.oracle), "oracle")
    binary = ensure_regular_file(Path(arguments.binary), "product binary", executable=True)
    repository_root = ensure_directory(Path(arguments.repository_root), "repository root")
    output = Path(arguments.output)
    if output.exists() or output.is_symlink():
        raise EvaluationError("evaluation.invalid_output", "output destination already exists")
    ensure_directory(output.parent, "output parent")
    validate_manifest(manifest_path)
    corpus, policy, oracle = load_contracts(corpus_path, policy_path, oracle_path)
    validate_product_commit(arguments.product_commit, oracle["baseline_product_commit"])
    home_parent = ensure_directory(output.parent, "output parent")
    with tempfile.TemporaryDirectory(prefix=".codenoesis-evaluation-home-", dir=home_parent) as home_name:
        with tempfile.TemporaryDirectory(prefix=".codenoesis-evaluation-run-", dir=home_parent) as scratch_name:
            home = Path(home_name)
            scratch = Path(scratch_name)
            repositories = {}
            for descriptor in corpus["entries"]:
                repository, _ = preflight_repository(
                    repository_root / descriptor["id"], descriptor, home
                )
                repositories[descriptor["id"]] = repository
            entries = {}
            for descriptor in corpus["entries"]:
                entry_id = descriptor["id"]
                entries[entry_id] = evaluate_entry(
                    binary,
                    repositories[entry_id],
                    descriptor,
                    oracle["entries"][entry_id],
                    policy,
                    scratch,
                    home,
                )
    aggregate = aggregate_report(entries)
    if aggregate["constant_stage_successes"] < policy["minimum_constant_stage_successes"]:
        raise EvaluationError("evaluation.oracle_mismatch", "constant-stage coverage is below policy")
    report = {
        "schema_version": REPORT_SCHEMA,
        "runner_version": RUNNER_VERSION,
        "suite_id": SUITE_ID,
        "manifest_sha256": sha256_file(manifest_path),
        "corpus": {
            "id": corpus["id"],
            "version": corpus["version"],
            "descriptor_sha256": sha256_file(corpus_path),
        },
        "corpus_version": corpus["version"],
        "policy_sha256": sha256_file(policy_path),
        "oracle_sha256": sha256_file(oracle_path),
        "product": {
            "source_id": "codenoesis",
            "commit": arguments.product_commit,
            "binary_sha256": sha256_file(binary),
        },
        "host": host_record(arguments.host_profile),
        "enabled_extractors": ENABLED_EXTRACTORS,
        "repetitions": policy["repetitions"],
        "concurrency": policy["concurrency"],
        "cache_state": policy["cache_state"],
        "percentile_method": policy["percentile_method"],
        "success_rate": aggregate["oracle_match_rate"],
        "stage_order": list(STAGES),
        "entries": entries,
        "aggregate": aggregate,
        "limitations": {
            "cross_host_comparison_allowed": policy["cross_host_comparison_allowed"],
            "slo_claimed": policy["slo_claimed"],
            "release_claimed": policy["release_claimed"],
            "ga_claimed": policy["ga_claimed"],
            "runtime_behavior_observed": False,
            "model_authority": False,
        },
    }
    encoded = canonical_json_bytes(report)
    if len(encoded) > policy["report_bytes_max"]:
        raise EvaluationError("evaluation.limit_exceeded", "evaluation report exceeds byte limit")
    private_values = {str(root), str(repository_root), str(output.parent), os.environ.get("HOME", "")}
    if any(value and value.encode("utf-8") in encoded for value in private_values):
        raise EvaluationError("evaluation.privacy", "evaluation report contains a private path")
    publish_new_file(output, encoded)
    return report


def parser() -> FailClosedParser:
    value = FailClosedParser(add_help=False)
    subparsers = value.add_subparsers(dest="command", required=True, parser_class=FailClosedParser)
    run = subparsers.add_parser("run", add_help=False)
    run.add_argument("--manifest", required=True)
    run.add_argument("--suite", required=True)
    run.add_argument("--corpus", required=True)
    run.add_argument("--policy", required=True)
    run.add_argument("--oracle", required=True)
    run.add_argument("--binary", required=True)
    run.add_argument("--repository-root", required=True)
    run.add_argument("--output", required=True)
    run.add_argument("--host-profile", required=True)
    run.add_argument("--product-commit", required=True)
    return value


def emit_error(error: EvaluationError) -> None:
    payload = {
        "schema_version": ERROR_SCHEMA,
        "code": error.code,
        "stage": error.stage,
        "message": error.message,
        "retryable": False,
        "context": error.context,
    }
    sys.stderr.buffer.write(canonical_json_bytes(payload))


def main() -> int:
    try:
        arguments = parser().parse_args()
        if arguments.command != "run" or HEX_40.fullmatch(arguments.product_commit) is None:
            raise EvaluationError("evaluation.invalid_arguments", "invalid command-line arguments")
        run_evaluation(arguments)
        return 0
    except EvaluationError as error:
        emit_error(error)
        return error.exit_code
    except Exception:
        emit_error(
            EvaluationError(
                "evaluation.internal",
                "unexpected evaluation failure",
                stage="internal",
                exit_code=1,
            )
        )
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
