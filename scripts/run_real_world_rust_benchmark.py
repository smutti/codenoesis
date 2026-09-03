#!/usr/bin/env python3
"""Run and compare the bounded real-world Rust stability benchmark."""

from __future__ import annotations

import argparse
import getpass
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
from fractions import Fraction
from typing import Any, BinaryIO, NoReturn, Sequence


RUNNER_VERSION = "codenoesis.real-world-rust-benchmark-runner/v1"
REPORT_SCHEMA = "codenoesis.real-world-rust-benchmark-report/v1"
COMPARISON_SCHEMA = "codenoesis.real-world-rust-benchmark-comparison/v1"
ERROR_SCHEMA = "codenoesis.real-world-rust-benchmark-error/v1"
SUITE_ID = "rust-real-world-stability-v1"
BENCHMARK_EXECUTION_LIMIT_PROFILE = "real-world-rust-benchmark-75s-v1"
BOOTSTRAP_BASELINE_COMMIT = "cce84869430ef129f55591998b30ea2ea728e1c3"
FAILED_SAMPLE_MESSAGE_BYTES_MAX = 256
TYPED_PRODUCT_STDERR_BYTES_MAX = 2048
HEX_40 = re.compile(r"^[0-9a-f]{40}$")
HOST_PROFILE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$")
PUBLIC_PRODUCT_ERROR_CODE = re.compile(
    r"^[a-z][a-z0-9_]{0,15}(?:\.[a-z][a-z0-9_]{0,63}){1,2}$"
)
PUBLIC_PRODUCT_ERROR_STAGES = frozenset(
    {
        "acquisition",
        "contract",
        "explorer",
        "export",
        "extraction",
        "input",
        "internal",
        "publication",
        "query",
        "serialization",
        "store",
    }
)
PRIVATE_PRODUCT_ERROR_COMPONENTS = frozenset(
    {
        "canary",
        "context",
        "credential",
        "host",
        "message",
        "password",
        "path",
        "private",
        "secret",
        "source",
        "token",
        "url",
        "user",
    }
)
EXPECTED_ENTRY_IDS = ("lekton", "rustdesk")
EXPECTED_RUNNER_PREFIX = [
    "python3",
    "scripts/run_real_world_rust_benchmark.py",
    "run",
    "--manifest",
    "benchmarks/manifest.json",
    "--suite",
    SUITE_ID,
]
EXPECTED_REPORT_FIELDS = (
    "cache_state",
    "concurrency",
    "corpus_version",
    "enabled_extractors",
    "host",
    "percentile_method",
    "repetitions",
    "success_rate",
)
SCAN_PROFILE_ARGUMENTS = {
    "local-git-sha1-packed-v1": ("--acquisition-profile", "local-git-sha1-packed-v1"),
    "local-gitlinks-v1": ("--repository-boundary-profile", "local-gitlinks-v1"),
    "cargo-root-package-v1": ("--workspace-profile", "cargo-root-package-v1"),
    "cargo-manifest-facts-v1": ("--manifest-profile", "cargo-manifest-facts-v1"),
    "rust-semantic-depth-v1": ("--rust-semantic-profile", "rust-semantic-depth-v1"),
    "rust-framework-declarations-v1": (
        "--rust-framework-profile",
        "rust-framework-declarations-v1",
    ),
    "rust-callable-semantics-v1": (
        "--rust-callable-profile",
        "rust-callable-semantics-v1",
    ),
    "rust-expression-bindings-v1": (
        "--rust-expression-profile",
        "rust-expression-bindings-v1",
    ),
    "rust-local-flow-v1": ("--rust-flow-profile", "rust-local-flow-v1"),
    "rust-safe-constant-evaluation-v1": (
        "--rust-constant-profile",
        "rust-safe-constant-evaluation-v1",
    ),
    "local-snapshot-256m-v1": (
        "--output-capacity-profile",
        "local-snapshot-256m-v1",
    ),
    BENCHMARK_EXECUTION_LIMIT_PROFILE: (
        "--execution-limit-profile",
        BENCHMARK_EXECUTION_LIMIT_PROFILE,
    ),
}
EXPECTED_PROFILES = {
    "lekton": [
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
        BENCHMARK_EXECUTION_LIMIT_PROFILE,
    ],
    "rustdesk": [
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
        BENCHMARK_EXECUTION_LIMIT_PROFILE,
    ],
}
EXPECTED_ORACLE_ENTRIES = {
    "lekton": {
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
    },
    "rustdesk": {
        "outcome": "typed_rejection",
        "exit_code": 2,
        "stdout_bytes": 0,
        "error_schema": "codenoesis.error/v24",
        "error_code": "input.unsupported_rust_constant_evaluation_composition",
        "error_stage": "input",
        "error_reason": "repository_boundary_not_supported",
        "store_created": False,
        "nested_source_read": False,
    },
}
EXPECTED_SOURCE_IDENTITIES = {
    "lekton": {
        "revision": "247b8f42fb045db41166d70a276a41c2e079b6eb",
        "tree": "55ba428493a4ffae86ba492422a049f46d567a30",
    },
    "rustdesk": {
        "revision": "d412d198720aa56f6cfed2dfad262e8fb1322fb7",
        "tree": "df8d4c292c9d256a445480eb878e507df3de1dc4",
    },
}


class BenchmarkError(Exception):
    """Expected fail-closed benchmark error."""

    def __init__(self, code: str, message: str, *, stage: str = "input", exit_code: int = 2):
        super().__init__(message)
        self.code = code
        self.message = message
        self.stage = stage
        self.exit_code = exit_code


class FailClosedParser(argparse.ArgumentParser):
    """Convert argparse failures into the public typed error contract."""

    def error(self, message: str) -> NoReturn:
        del message
        raise BenchmarkError("benchmark.invalid_arguments", "invalid command-line arguments")


def canonical_json_bytes(value: Any) -> bytes:
    """Serialize one compact, deterministic, LF-terminated JSON document."""
    try:
        encoded = json.dumps(
            value,
            ensure_ascii=False,
            allow_nan=False,
            separators=(",", ":"),
            sort_keys=True,
        ).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise BenchmarkError(
            "benchmark.internal",
            "benchmark data cannot be serialized",
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
        raise BenchmarkError("benchmark.invalid_input", "cannot read an input file") from error
    return digest.hexdigest()


def public_product_error_identity(stderr_path: Path) -> tuple[str, str]:
    try:
        stderr_size = stderr_path.stat().st_size
        if stderr_size == 0:
            return "unparseable", "empty"
        if stderr_size > TYPED_PRODUCT_STDERR_BYTES_MAX:
            return "unparseable", "oversized"
        stderr = stderr_path.read_bytes()
    except OSError:
        return "unparseable", "wrong_shape"
    if len(stderr) != stderr_size:
        return "unparseable", "wrong_shape"
    try:
        decoded = stderr.decode("utf-8")
    except UnicodeError:
        return "unparseable", "non_utf8"
    try:
        error = json.loads(decoded)
    except json.JSONDecodeError:
        return "unparseable", "invalid_json"
    if not isinstance(error, dict) or set(error) != {
        "code",
        "context",
        "message",
        "retryable",
        "schema_version",
        "stage",
    }:
        return "unparseable", "wrong_shape"
    try:
        if canonical_json_bytes(error) != stderr:
            return "unparseable", "noncanonical"
    except BenchmarkError:
        return "unparseable", "noncanonical"
    schema = error.get("schema_version")
    code = error.get("code")
    stage = error.get("stage")
    if (
        not isinstance(error.get("message"), str)
        or not isinstance(error.get("context"), dict)
        or error.get("retryable") is not False
    ):
        return "unparseable", "wrong_shape"
    if schema != "codenoesis.error/v24":
        return "unparseable", "unsupported_schema"
    if (
        not isinstance(code, str)
        or len(code) > 64
        or PUBLIC_PRODUCT_ERROR_CODE.fullmatch(code) is None
    ):
        return "unparseable", "unsafe_code"
    if not isinstance(stage, str) or stage not in PUBLIC_PRODUCT_ERROR_STAGES:
        return "unparseable", "unsafe_stage"
    if frozenset(re.split(r"[._]", code)) & PRIVATE_PRODUCT_ERROR_COMPONENTS:
        return "unparseable", "unsafe_code"
    if stage in PRIVATE_PRODUCT_ERROR_COMPONENTS:
        return "unparseable", "unsafe_stage"
    if code != "internal.failure" and not code.startswith(f"{stage}."):
        return "unparseable", "inconsistent_stage"
    return f"{schema}|{code}|{stage}", "accepted"


def failed_sample_message(
    entry_id: str,
    index: int,
    return_code: int,
    stdout_bytes: int,
    stderr_path: Path,
) -> str:
    try:
        stderr_bytes = stderr_path.stat().st_size
    except OSError as error:
        raise BenchmarkError(
            "benchmark.internal",
            "failed sample identity is unavailable",
            stage="internal",
            exit_code=1,
        ) from error
    exit_identity = str(return_code) if return_code >= 0 else "signal"
    product_identity, validation = public_product_error_identity(stderr_path)
    message = (
        f"sample_failed entry={entry_id} index={index} exit={exit_identity} "
        f"stdout={stdout_bytes} stderr={stderr_bytes} sha256={sha256_file(stderr_path)} "
        f"product={product_identity} validation={validation}"
    )
    if len(message.encode("utf-8")) > FAILED_SAMPLE_MESSAGE_BYTES_MAX:
        raise BenchmarkError(
            "benchmark.internal",
            "failed sample identity exceeds its byte limit",
            stage="internal",
            exit_code=1,
        )
    return message


def load_json(path: Path, *, maximum_bytes: int | None = None) -> Any:
    try:
        size = path.stat().st_size
        if maximum_bytes is not None and size > maximum_bytes:
            raise BenchmarkError("benchmark.limit_exceeded", "JSON input exceeds its byte limit")
        with path.open("r", encoding="utf-8") as handle:
            return json.load(handle)
    except BenchmarkError:
        raise
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise BenchmarkError("benchmark.invalid_input", "invalid JSON input") from error


def require_object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise BenchmarkError("benchmark.invalid_input", f"{label} must be an object")
    return value


def require_exact_keys(value: dict[str, Any], keys: set[str], label: str) -> None:
    if set(value) != keys:
        raise BenchmarkError("benchmark.invalid_input", f"{label} fields do not match the contract")


def ensure_regular_file(path: Path, label: str, *, executable: bool = False) -> Path:
    try:
        metadata = path.lstat()
    except OSError as error:
        raise BenchmarkError("benchmark.invalid_input", f"{label} is unavailable") from error
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
        raise BenchmarkError("benchmark.invalid_input", f"{label} must be a regular non-symlink file")
    if executable and not os.access(path, os.X_OK):
        raise BenchmarkError("benchmark.invalid_input", f"{label} must be executable")
    return path.resolve(strict=True)


def ensure_directory(path: Path, label: str) -> Path:
    try:
        metadata = path.lstat()
    except OSError as error:
        raise BenchmarkError("benchmark.invalid_input", f"{label} is unavailable") from error
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
        raise BenchmarkError("benchmark.invalid_input", f"{label} must be a directory, not a symlink")
    return path.resolve(strict=True)


def validate_output_destination(path: Path) -> Path:
    if path.exists() or path.is_symlink():
        raise BenchmarkError("benchmark.invalid_input", "output destination must not exist")
    parent = ensure_directory(path.parent, "output parent")
    return parent / path.name


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
        raise BenchmarkError("benchmark.invalid_input", "system Git is unavailable")
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
        raise BenchmarkError("benchmark.invalid_input", "local Git inspection failed") from error
    if result.returncode != 0:
        raise BenchmarkError("benchmark.invalid_input", "local Git inspection rejected the repository")
    return result.stdout


def git_text(repository: Path, arguments: Sequence[str], home: Path) -> str:
    try:
        return run_git(repository, arguments, home).decode("utf-8").strip()
    except UnicodeError as error:
        raise BenchmarkError("benchmark.invalid_input", "local Git output is not UTF-8") from error


def ensure_unmaterialized_gitlinks(repository: Path, home: Path) -> None:
    output = git_text(repository, ["ls-tree", "-r", "--full-tree", "HEAD"], home)
    for line in output.splitlines():
        metadata, separator, relative = line.partition("\t")
        if not separator or not metadata.startswith("160000 "):
            continue
        if not relative or relative.startswith("/") or ".." in Path(relative).parts:
            raise BenchmarkError("benchmark.invalid_input", "repository contains an unsafe gitlink path")
        gitlink = repository / relative
        if not gitlink.exists():
            continue
        if gitlink.is_symlink() or not gitlink.is_dir():
            raise BenchmarkError("benchmark.invalid_input", "gitlink content must not be materialized")
        try:
            if any(gitlink.iterdir()):
                raise BenchmarkError("benchmark.invalid_input", "gitlink content must not be materialized")
        except OSError as error:
            raise BenchmarkError("benchmark.invalid_input", "gitlink content cannot be inspected") from error


def preflight_repository(
    path: Path,
    descriptor: dict[str, Any],
    home: Path,
    *,
    require_clean: bool = True,
) -> bytes:
    repository = ensure_directory(path, f"{descriptor['id']} repository")
    git_directory = repository / ".git"
    try:
        git_metadata = git_directory.lstat()
    except OSError as error:
        raise BenchmarkError("benchmark.invalid_input", "repository must be a normal local clone") from error
    if stat.S_ISLNK(git_metadata.st_mode) or not stat.S_ISDIR(git_metadata.st_mode):
        raise BenchmarkError("benchmark.invalid_input", "repository must be a normal local clone")
    if git_text(repository, ["rev-parse", "--is-inside-work-tree"], home) != "true":
        raise BenchmarkError("benchmark.invalid_input", "repository work tree is invalid")
    if git_text(repository, ["rev-parse", "--is-shallow-repository"], home) != "false":
        raise BenchmarkError("benchmark.invalid_input", "repository must be a full non-shallow clone")
    try:
        observed_root = Path(
            git_text(repository, ["rev-parse", "--show-toplevel"], home)
        ).resolve(strict=True)
    except OSError as error:
        raise BenchmarkError("benchmark.invalid_input", "repository root cannot be resolved") from error
    if observed_root != repository:
        raise BenchmarkError("benchmark.invalid_input", "repository root does not match the supplied path")
    if git_text(repository, ["rev-parse", "--show-object-format"], home) != "sha1":
        raise BenchmarkError("benchmark.invalid_input", "repository object format must be sha1")
    if git_text(repository, ["remote", "get-url", "origin"], home) != descriptor["repository_url"]:
        raise BenchmarkError("benchmark.invalid_input", "repository origin does not match the corpus")
    if git_text(repository, ["rev-parse", "HEAD"], home) != descriptor["revision"]:
        raise BenchmarkError("benchmark.invalid_input", "repository HEAD does not match the corpus revision")
    tree_expression = f"{descriptor['revision']}^{{tree}}"
    if git_text(repository, ["rev-parse", tree_expression], home) != descriptor["tree"]:
        raise BenchmarkError("benchmark.invalid_input", "repository tree does not match the corpus")
    status_bytes = run_git(repository, ["status", "--porcelain=v1", "--untracked-files=all"], home)
    if require_clean and status_bytes:
        raise BenchmarkError("benchmark.mutable_input", "repository must be clean")
    ensure_unmaterialized_gitlinks(repository, home)
    return status_bytes


def load_contracts(manifest_path: Path, suite_id: str) -> tuple[dict[str, Any], ...]:
    manifest_path = ensure_regular_file(manifest_path, "manifest")
    root = manifest_path.parent.parent
    try:
        from validate_benchmark_assets import validate_assets
    except ImportError as error:
        raise BenchmarkError(
            "benchmark.internal",
            "benchmark asset validator is unavailable",
            stage="internal",
            exit_code=1,
        ) from error
    asset_errors, _ = validate_assets(root)
    if asset_errors:
        raise BenchmarkError("benchmark.invalid_input", "benchmark assets fail closed validation")
    manifest = require_object(load_json(manifest_path), "manifest")
    if suite_id != SUITE_ID or manifest.get("status") != "active":
        raise BenchmarkError("benchmark.invalid_input", "requested suite is not active")
    suites = manifest.get("suites")
    if not isinstance(suites, list):
        raise BenchmarkError("benchmark.invalid_input", "manifest suites are invalid")
    matching = [suite for suite in suites if isinstance(suite, dict) and suite.get("id") == suite_id]
    if len(matching) != 1:
        raise BenchmarkError("benchmark.invalid_input", "requested suite must be unique")
    suite = matching[0]
    if suite.get("runner") != EXPECTED_RUNNER_PREFIX:
        raise BenchmarkError("benchmark.invalid_input", "suite runner does not match the executable contract")
    if suite.get("repetitions") != 3 or suite.get("concurrency") != 1:
        raise BenchmarkError("benchmark.invalid_input", "suite execution parameters are invalid")
    if suite.get("cache_state") != "mixed" or suite.get("percentile_method") != "nearest-rank":
        raise BenchmarkError("benchmark.invalid_input", "suite sampling policy is invalid")
    if suite.get("minimum_success_rate") != 1.0:
        raise BenchmarkError("benchmark.invalid_input", "suite success rate is invalid")
    if tuple(manifest.get("report_required_fields", [])) != EXPECTED_REPORT_FIELDS:
        raise BenchmarkError("benchmark.invalid_input", "manifest report fields are invalid")
    if manifest.get("requirements") != ["NFR-PER-001"]:
        raise BenchmarkError("benchmark.invalid_input", "manifest claim scope is invalid")

    corpus_path = root / "benchmarks/corpora/real-world-rust-stability-v1.json"
    policy_path = root / "benchmarks/policies/real-world-rust-stability-v1.json"
    oracle_path = root / "benchmarks/baselines/real-world-rust-stability-v1.json"
    for contract_path, label in (
        (corpus_path, "corpus"),
        (policy_path, "policy"),
        (oracle_path, "oracle"),
    ):
        ensure_regular_file(contract_path, label)
    corpus = require_object(load_json(corpus_path), "corpus")
    policy = require_object(load_json(policy_path), "policy")
    oracle = require_object(load_json(oracle_path), "oracle")
    if corpus.get("id") != suite.get("corpus", {}).get("id") or corpus.get("version") != suite.get(
        "corpus", {}
    ).get("version"):
        raise BenchmarkError("benchmark.invalid_input", "corpus identity does not match the suite")
    entries = corpus.get("entries")
    if not isinstance(entries, list) or tuple(entry.get("id") for entry in entries) != EXPECTED_ENTRY_IDS:
        raise BenchmarkError("benchmark.invalid_input", "corpus entries do not match the suite")
    if any(
        not isinstance(entry, dict) or entry.get("profiles") != EXPECTED_PROFILES[entry_id]
        for entry_id, entry in zip(EXPECTED_ENTRY_IDS, entries)
    ):
        raise BenchmarkError("benchmark.invalid_input", "corpus profile matrix is invalid")
    if corpus.get("network_allowed") is not False or corpus.get("source_vendored") is not False:
        raise BenchmarkError("benchmark.invalid_input", "corpus source policy is invalid")
    if policy.get("suite_id") != suite_id or policy.get("requirements") != ["NFR-PER-001"]:
        raise BenchmarkError("benchmark.invalid_input", "benchmark policy scope is invalid")
    if policy.get("repetitions") != 3 or policy.get("failed_sample_retry_allowed") is not False:
        raise BenchmarkError("benchmark.invalid_input", "benchmark repetition policy is invalid")
    if policy.get("nfr_per_002_claimed") is not False or any(
        policy.get(field) is not False for field in ("slo_claimed", "release_claimed", "ga_claimed")
    ):
        raise BenchmarkError("benchmark.invalid_input", "benchmark policy exceeds observational scope")
    if (
        oracle.get("suite_id") != suite_id
        or oracle.get("baseline_product_commit") != BOOTSTRAP_BASELINE_COMMIT
    ):
        raise BenchmarkError("benchmark.invalid_input", "benchmark oracle identity is invalid")
    return manifest, suite, corpus, policy, oracle, {
        "manifest": sha256_file(manifest_path),
        "corpus": sha256_file(corpus_path),
        "policy": sha256_file(policy_path),
        "oracle": sha256_file(oracle_path),
    }


def rust_toolchain(manifest_path: Path) -> str:
    toolchain_path = manifest_path.parent.parent / "rust-toolchain.toml"
    ensure_regular_file(toolchain_path, "Rust toolchain contract")
    try:
        text = toolchain_path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        raise BenchmarkError("benchmark.invalid_input", "Rust toolchain contract cannot be read") from error
    match = re.search(r'^channel\s*=\s*"([^"]+)"\s*$', text, flags=re.MULTILINE)
    if match is None:
        raise BenchmarkError("benchmark.invalid_input", "Rust toolchain channel is missing")
    return match.group(1)


def build_scan_command(
    binary: Path,
    repository: Path,
    store: Path,
    descriptor: dict[str, Any],
) -> list[str]:
    arguments = [
        str(binary),
        "scan",
        "--repository",
        str(repository),
        "--repository-id",
        descriptor["repository_id"],
        "--revision",
        descriptor["revision"],
        "--profile",
        "standard-local-s4",
    ]
    profiles = descriptor.get("profiles")
    if not isinstance(profiles, list) or not profiles:
        raise BenchmarkError("benchmark.invalid_input", "corpus profiles are invalid")
    for profile in profiles:
        profile_arguments = SCAN_PROFILE_ARGUMENTS.get(profile)
        if profile_arguments is None:
            raise BenchmarkError("benchmark.invalid_input", "corpus contains an unknown profile")
        arguments.extend(profile_arguments)
    arguments.extend(["--store", str(store), "--format", "json"])
    return arguments


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
        raise BenchmarkError("benchmark.sample_failed", "cannot start the product binary") from error
    try:
        return_code = process.wait(timeout=timeout_seconds)
    except subprocess.TimeoutExpired as error:
        if os.name == "posix":
            os.killpg(process.pid, signal.SIGKILL)
        else:
            process.kill()
        process.wait()
        raise BenchmarkError("benchmark.timeout", "benchmark sample exceeded its timeout") from error
    return return_code, time.monotonic_ns() - started


def semantic_projection(snapshot: dict[str, Any]) -> bytes:
    if "semantic" not in snapshot:
        raise BenchmarkError("benchmark.oracle_mismatch", "snapshot semantic projection is missing")
    try:
        return json.dumps(
            snapshot["semantic"],
            ensure_ascii=False,
            allow_nan=False,
            separators=(",", ":"),
        ).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise BenchmarkError("benchmark.oracle_mismatch", "snapshot semantic projection is invalid") from error


def graph_counts(snapshot: dict[str, Any]) -> dict[str, int]:
    try:
        graph = snapshot["semantic"]["knowledge_graph"]
        entities = graph["entities"]
        counts = {
            "entities": len(entities),
            "relationships": len(graph["relationships"]),
            "claims": len(graph["claims"]),
            "evidence": len(graph["evidence"]),
            "diagnostics": len(graph["diagnostics"]),
            "coverage": len(graph["coverage"]),
            "evaluated_values": sum(
                1 for entity in entities if entity.get("kind") == "rust.evaluated_value"
            ),
        }
    except (KeyError, TypeError) as error:
        raise BenchmarkError("benchmark.oracle_mismatch", "snapshot graph families are invalid") from error
    return counts


def parse_success_sample(stdout_path: Path, sample: dict[str, Any], expected: dict[str, Any]) -> None:
    snapshot = require_object(load_json(stdout_path), "product snapshot")
    try:
        semantic_hash = snapshot["semantic_hash"]["value"]
    except (KeyError, TypeError) as error:
        raise BenchmarkError("benchmark.oracle_mismatch", "snapshot semantic hash is invalid") from error
    projection_sha256 = sha256_bytes(semantic_projection(snapshot))
    counts = graph_counts(snapshot)
    observed = {
        "snapshot_schema": snapshot.get("schema_version"),
        "semantic_hash": semantic_hash,
        "semantic_projection_sha256": projection_sha256,
        "counts": counts,
    }
    if observed != {
        "snapshot_schema": expected.get("snapshot_schema"),
        "semantic_hash": expected.get("semantic_hash"),
        "semantic_projection_sha256": expected.get("semantic_projection_sha256"),
        "counts": expected.get("counts"),
    }:
        raise BenchmarkError("benchmark.oracle_mismatch", "success sample does not match the semantic oracle")
    sample.update(observed)


def parse_rejection_sample(
    stderr_path: Path,
    sample: dict[str, Any],
    expected: dict[str, Any],
    store_created: bool,
) -> None:
    error_document = require_object(load_json(stderr_path), "product error")
    context = error_document.get("context")
    if not isinstance(context, dict):
        raise BenchmarkError("benchmark.oracle_mismatch", "typed rejection context is invalid")
    observed = {
        "error_schema": error_document.get("schema_version"),
        "error_code": error_document.get("code"),
        "error_stage": error_document.get("stage"),
        "error_reason": context.get("reason"),
        "store_created": store_created,
        "nested_source_read": False,
    }
    if observed != {
        "error_schema": expected.get("error_schema"),
        "error_code": expected.get("error_code"),
        "error_stage": expected.get("error_stage"),
        "error_reason": expected.get("error_reason"),
        "store_created": expected.get("store_created"),
        "nested_source_read": expected.get("nested_source_read"),
    }:
        raise BenchmarkError("benchmark.oracle_mismatch", "typed rejection does not match the oracle")
    sample.update(observed)


def owned_temporary_root() -> Path:
    root = Path(tempfile.mkdtemp(prefix="codenoesis-b1-"))
    (root / ".codenoesis-b1-owned").write_text(RUNNER_VERSION + "\n", encoding="utf-8")
    return root


def clean_owned_temporary_root(root: Path) -> None:
    marker = root / ".codenoesis-b1-owned"
    try:
        if marker.read_text(encoding="utf-8") != RUNNER_VERSION + "\n":
            raise BenchmarkError(
                "benchmark.internal",
                "temporary ownership marker is invalid",
                stage="cleanup",
                exit_code=1,
            )
        shutil.rmtree(root)
    except BenchmarkError:
        raise
    except OSError as error:
        raise BenchmarkError(
            "benchmark.internal",
            "temporary benchmark state could not be cleaned",
            stage="cleanup",
            exit_code=1,
        ) from error


def run_sample(
    binary: Path,
    repository: Path,
    descriptor: dict[str, Any],
    expected: dict[str, Any],
    timeout_seconds: int,
    index: int,
) -> dict[str, Any]:
    temporary_root = owned_temporary_root()
    primary_error: BenchmarkError | None = None
    sample: dict[str, Any] | None = None
    before: bytes | None = None
    repository_preflighted = False
    try:
        git_home = temporary_root / "git-home"
        git_home.mkdir()
        before = preflight_repository(repository, descriptor, git_home)
        repository_preflighted = True
        stdout_path = temporary_root / "stdout.json"
        stderr_path = temporary_root / "stderr.json"
        store_path = temporary_root / "store"
        with stdout_path.open("wb") as stdout_handle, stderr_path.open("wb") as stderr_handle:
            return_code, wall_time_ns = wait_for_process(
                build_scan_command(binary, repository, store_path, descriptor),
                stdout_handle,
                stderr_handle,
                timeout_seconds,
                git_home,
            )
        stdout_bytes = stdout_path.stat().st_size
        stderr_bytes = stderr_path.stat().st_size
        sample = {
            "exit_code": return_code,
            "index": index,
            "stderr_bytes": stderr_bytes,
            "stdout_bytes": stdout_bytes,
            "wall_time_ns": wall_time_ns,
        }
        if descriptor["outcome"] == "success":
            if return_code != 0 or stderr_bytes != 0:
                raise BenchmarkError(
                    "benchmark.sample_failed",
                    failed_sample_message(
                        descriptor["id"], index, return_code, stdout_bytes, stderr_path
                    ),
                )
            parse_success_sample(stdout_path, sample, expected)
        elif descriptor["outcome"] == "typed_rejection":
            if return_code != expected.get("exit_code") or stdout_bytes != expected.get("stdout_bytes"):
                raise BenchmarkError(
                    "benchmark.sample_failed",
                    failed_sample_message(
                        descriptor["id"], index, return_code, stdout_bytes, stderr_path
                    ),
                )
            parse_rejection_sample(stderr_path, sample, expected, store_path.exists())
        else:
            raise BenchmarkError("benchmark.invalid_input", "corpus outcome is unknown")
    except BenchmarkError as error:
        primary_error = error
    finally:
        if repository_preflighted:
            try:
                after = preflight_repository(repository, descriptor, git_home)
                if before != after or after:
                    raise BenchmarkError("benchmark.mutable_input", "repository changed during the sample")
            except BenchmarkError as mutation_error:
                primary_error = mutation_error
        try:
            clean_owned_temporary_root(temporary_root)
        except BenchmarkError as cleanup_error:
            if primary_error is None:
                primary_error = cleanup_error
    if primary_error is not None:
        raise primary_error
    if sample is None:
        raise BenchmarkError(
            "benchmark.internal",
            "sample completion state is missing",
            stage="internal",
            exit_code=1,
        )
    return sample


def nearest_rank(values: Sequence[int], percentile: int) -> int:
    if not values or percentile < 1 or percentile > 100 or any(
        type(value) is not int or value < 0 for value in values
    ):
        raise BenchmarkError("benchmark.invalid_input", "percentile input is invalid")
    ordered = sorted(values)
    rank = math.ceil(percentile * len(ordered) / 100)
    return ordered[rank - 1]


def percentile_summary(samples: Sequence[dict[str, Any]]) -> dict[str, int]:
    wall_times = [sample["wall_time_ns"] for sample in samples]
    return {
        "p50": nearest_rank(wall_times, 50),
        "p95": nearest_rank(wall_times, 95),
        "p99": nearest_rank(wall_times, 99),
    }


def host_metadata(profile: str, memory_class_bytes: int, toolchain: str) -> dict[str, Any]:
    return {
        "architecture": platform.machine().lower(),
        "logical_cpu_count": os.cpu_count() or 1,
        "memory_class_bytes": memory_class_bytes,
        "operating_system": platform.system().lower(),
        "profile": profile,
        "rust_toolchain": toolchain,
    }


def privacy_canaries(paths: Sequence[Path]) -> set[str]:
    canaries = {str(path) for path in paths if str(path)}
    for value in (platform.node(), getpass.getuser(), str(Path.home())):
        if value:
            canaries.add(value)
    return canaries


def ensure_privacy(encoded: bytes, canaries: set[str]) -> None:
    text = encoded.decode("utf-8")
    for canary in canaries:
        if canary and canary in text:
            raise BenchmarkError("benchmark.privacy_violation", "report contains private host data")


def publish_new_file(path: Path, encoded: bytes) -> None:
    staged: Path | None = None
    try:
        descriptor, staged_name = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
        staged = Path(staged_name)
        with os.fdopen(descriptor, "wb") as handle:
            handle.write(encoded)
            handle.flush()
            os.fsync(handle.fileno())
        os.link(staged, path)
        try:
            staged.unlink()
            staged = None
        except OSError:
            pass
    except FileExistsError as error:
        raise BenchmarkError("benchmark.invalid_input", "output destination already exists") from error
    except OSError as error:
        raise BenchmarkError(
            "benchmark.internal",
            "report publication failed",
            stage="output",
            exit_code=1,
        ) from error
    finally:
        if staged is not None:
            try:
                staged.unlink(missing_ok=True)
            except OSError:
                pass


def run_benchmark(arguments: argparse.Namespace) -> dict[str, Any]:
    manifest_path = Path(arguments.manifest)
    manifest, suite, corpus, policy, oracle, digests = load_contracts(
        manifest_path, arguments.suite
    )
    del manifest
    binary = ensure_regular_file(Path(arguments.binary), "product binary", executable=True)
    if not HEX_40.fullmatch(arguments.product_commit):
        raise BenchmarkError("benchmark.invalid_arguments", "product commit must be lowercase SHA-1")
    if not HOST_PROFILE.fullmatch(arguments.host_profile):
        raise BenchmarkError("benchmark.invalid_arguments", "host profile is invalid")
    if arguments.memory_class_bytes <= 0:
        raise BenchmarkError("benchmark.invalid_arguments", "memory class must be positive")
    output = validate_output_destination(Path(arguments.output))
    repository_paths = {
        "lekton": Path(arguments.lekton),
        "rustdesk": Path(arguments.rustdesk),
    }
    descriptors = {entry["id"]: entry for entry in corpus["entries"]}
    expected_entries = oracle.get("entries")
    if not isinstance(expected_entries, dict) or set(expected_entries) != set(EXPECTED_ENTRY_IDS):
        raise BenchmarkError("benchmark.invalid_input", "oracle entries are invalid")

    preflight_root = owned_temporary_root()
    try:
        preflight_home = preflight_root / "git-home"
        preflight_home.mkdir()
        for entry_id in EXPECTED_ENTRY_IDS:
            preflight_repository(repository_paths[entry_id], descriptors[entry_id], preflight_home)
    finally:
        clean_owned_temporary_root(preflight_root)

    report_entries: dict[str, Any] = {}
    all_samples = 0
    successful_samples = 0
    for entry_id in EXPECTED_ENTRY_IDS:
        descriptor = descriptors[entry_id]
        samples = []
        for index in range(1, suite["repetitions"] + 1):
            sample = run_sample(
                binary,
                repository_paths[entry_id],
                descriptor,
                expected_entries[entry_id],
                policy["timeout_seconds"][entry_id],
                index,
            )
            samples.append(sample)
            all_samples += 1
            successful_samples += 1
        report_entries[entry_id] = {
            "outcome": descriptor["outcome"],
            "percentiles_ns": percentile_summary(samples),
            "profiles": descriptor["profiles"],
            "revision": descriptor["revision"],
            "samples": samples,
            "success_rate": 1.0,
            "tree": descriptor["tree"],
        }

    report = {
        "cache_state": suite["cache_state"],
        "concurrency": suite["concurrency"],
        "corpus": {
            "descriptor_sha256": digests["corpus"],
            "id": corpus["id"],
            "version": corpus["version"],
        },
        "corpus_version": corpus["version"],
        "enabled_extractors": suite["enabled_extractors"],
        "entries": report_entries,
        "host": host_metadata(
            arguments.host_profile,
            arguments.memory_class_bytes,
            rust_toolchain(manifest_path.resolve(strict=True)),
        ),
        "manifest_sha256": digests["manifest"],
        "minimum_success_rate": suite["minimum_success_rate"],
        "oracle_sha256": digests["oracle"],
        "percentile_method": suite["percentile_method"],
        "policy_sha256": digests["policy"],
        "product": {
            "binary_sha256": sha256_file(binary),
            "commit": arguments.product_commit,
            "source_id": "codenoesis",
        },
        "repetitions": suite["repetitions"],
        "runner_version": RUNNER_VERSION,
        "schema_version": REPORT_SCHEMA,
        "success_rate": successful_samples / all_samples,
        "suite_id": suite["id"],
    }
    validate_report(report, policy, expected_policy_sha256=digests["policy"])
    encoded = canonical_json_bytes(report)
    if len(encoded) > policy["report_bytes_max"]:
        raise BenchmarkError("benchmark.limit_exceeded", "report exceeds its byte limit")
    ensure_privacy(
        encoded,
        privacy_canaries([binary, output, *repository_paths.values()]),
    )
    publish_new_file(output, encoded)
    return {
        "output_sha256": sha256_bytes(encoded),
        "report_bytes": len(encoded),
        "schema_version": REPORT_SCHEMA,
        "success_rate": report["success_rate"],
        "suite_id": SUITE_ID,
    }


def require_number(value: Any, label: str) -> float:
    if not isinstance(value, (int, float)) or isinstance(value, bool) or not math.isfinite(value):
        raise BenchmarkError("benchmark.invalid_report", f"{label} must be finite numeric data")
    return float(value)


def validate_report(
    report: Any,
    policy: dict[str, Any],
    *,
    expected_policy_sha256: str | None = None,
) -> dict[str, Any]:
    document = require_object(report, "benchmark report")
    required = {
        "cache_state",
        "concurrency",
        "corpus",
        "corpus_version",
        "enabled_extractors",
        "entries",
        "host",
        "manifest_sha256",
        "minimum_success_rate",
        "oracle_sha256",
        "percentile_method",
        "policy_sha256",
        "product",
        "repetitions",
        "runner_version",
        "schema_version",
        "success_rate",
        "suite_id",
    }
    require_exact_keys(document, required, "benchmark report")
    if document["schema_version"] != REPORT_SCHEMA or document["runner_version"] != RUNNER_VERSION:
        raise BenchmarkError("benchmark.invalid_report", "report version is invalid")
    if document["suite_id"] != SUITE_ID or policy.get("suite_id") != SUITE_ID:
        raise BenchmarkError("benchmark.invalid_report", "report suite is invalid")
    corpus = require_object(document["corpus"], "corpus")
    require_exact_keys(corpus, {"descriptor_sha256", "id", "version"}, "corpus")
    if (
        corpus["id"] != "real-world-rust-stability"
        or corpus["version"] != "1"
        or document["corpus_version"] != corpus["version"]
        or not re.fullmatch(r"[0-9a-f]{64}", str(corpus["descriptor_sha256"]))
    ):
        raise BenchmarkError("benchmark.invalid_report", "report corpus identity is invalid")
    if document["enabled_extractors"] != ["rust-r16-source-only"]:
        raise BenchmarkError("benchmark.invalid_report", "report extractor set is invalid")
    if document["minimum_success_rate"] != 1.0:
        raise BenchmarkError("benchmark.invalid_report", "report minimum success rate is invalid")
    for digest_field in ("manifest_sha256", "oracle_sha256", "policy_sha256"):
        if not re.fullmatch(r"[0-9a-f]{64}", str(document[digest_field])):
            raise BenchmarkError("benchmark.invalid_report", "report contract identity is invalid")
    if document["repetitions"] != 3 or document["repetitions"] != policy.get("repetitions"):
        raise BenchmarkError("benchmark.invalid_report", "report repetition count is invalid")
    if document["cache_state"] != policy.get("cache_state") or document["concurrency"] != policy.get(
        "concurrency"
    ):
        raise BenchmarkError("benchmark.invalid_report", "report execution policy is invalid")
    if document["percentile_method"] != "nearest-rank" or document["percentile_method"] != policy.get(
        "percentile_method"
    ):
        raise BenchmarkError("benchmark.invalid_report", "report percentile method is invalid")
    if require_number(document["success_rate"], "success rate") != 1.0:
        raise BenchmarkError("benchmark.sample_failed", "report contains a failed sample")
    if expected_policy_sha256 is not None and document["policy_sha256"] != expected_policy_sha256:
        raise BenchmarkError("benchmark.invalid_report", "report policy identity is invalid")
    product = require_object(document["product"], "product")
    require_exact_keys(product, {"binary_sha256", "commit", "source_id"}, "product")
    if product["source_id"] != "codenoesis" or not HEX_40.fullmatch(str(product["commit"])):
        raise BenchmarkError("benchmark.invalid_report", "product source identity is invalid")
    if not re.fullmatch(r"[0-9a-f]{64}", str(product["binary_sha256"])):
        raise BenchmarkError("benchmark.invalid_report", "product binary identity is invalid")
    host = require_object(document["host"], "host")
    require_exact_keys(
        host,
        {
            "architecture",
            "logical_cpu_count",
            "memory_class_bytes",
            "operating_system",
            "profile",
            "rust_toolchain",
        },
        "host",
    )
    if not HOST_PROFILE.fullmatch(str(host["profile"])):
        raise BenchmarkError("benchmark.invalid_report", "host profile is invalid")
    for field in ("architecture", "operating_system", "rust_toolchain"):
        if not isinstance(host[field], str) or not host[field] or len(host[field]) > 128:
            raise BenchmarkError("benchmark.invalid_report", "host metadata is invalid")
    for field in ("logical_cpu_count", "memory_class_bytes"):
        if type(host[field]) is not int or host[field] <= 0:
            raise BenchmarkError("benchmark.invalid_report", "host capacity is invalid")
    entries = require_object(document["entries"], "entries")
    if set(entries) != set(EXPECTED_ENTRY_IDS):
        raise BenchmarkError("benchmark.invalid_report", "report entries are invalid")
    for entry_id in EXPECTED_ENTRY_IDS:
        entry = require_object(entries[entry_id], f"{entry_id} entry")
        require_exact_keys(
            entry,
            {
                "outcome",
                "percentiles_ns",
                "profiles",
                "revision",
                "samples",
                "success_rate",
                "tree",
            },
            f"{entry_id} entry",
        )
        samples = entry["samples"]
        if not isinstance(samples, list) or len(samples) != 3:
            raise BenchmarkError("benchmark.invalid_report", "report samples are incomplete")
        if [sample.get("index") for sample in samples if isinstance(sample, dict)] != [1, 2, 3]:
            raise BenchmarkError("benchmark.invalid_report", "report sample indexes are invalid")
        if require_number(entry["success_rate"], "entry success rate") != 1.0:
            raise BenchmarkError("benchmark.sample_failed", "entry contains a failed sample")
        if entry["outcome"] != EXPECTED_ORACLE_ENTRIES[entry_id]["outcome"]:
            raise BenchmarkError("benchmark.oracle_mismatch", "entry outcome does not match the oracle")
        if entry["profiles"] != EXPECTED_PROFILES[entry_id]:
            raise BenchmarkError("benchmark.invalid_report", "entry profiles do not match the corpus")
        if {"revision": entry["revision"], "tree": entry["tree"]} != EXPECTED_SOURCE_IDENTITIES[
            entry_id
        ]:
            raise BenchmarkError("benchmark.invalid_report", "entry source identity is invalid")
        wall_times = []
        for sample in samples:
            sample_object = require_object(sample, "sample")
            common_keys = {"exit_code", "index", "stderr_bytes", "stdout_bytes", "wall_time_ns"}
            oracle_fields = (
                {"snapshot_schema", "semantic_hash", "semantic_projection_sha256", "counts"}
                if entry_id == "lekton"
                else {
                    "error_schema",
                    "error_code",
                    "error_stage",
                    "error_reason",
                    "store_created",
                    "nested_source_read",
                }
            )
            require_exact_keys(sample_object, common_keys | oracle_fields, "sample")
            wall_time = sample_object.get("wall_time_ns")
            if type(wall_time) is not int or wall_time < 0:
                raise BenchmarkError("benchmark.invalid_report", "sample wall time is invalid")
            for field in ("exit_code", "stdout_bytes", "stderr_bytes"):
                if type(sample_object[field]) is not int or sample_object[field] < 0:
                    raise BenchmarkError("benchmark.invalid_report", "sample process data is invalid")
            wall_times.append(wall_time)
            if entry_id == "lekton":
                observed_oracle = {
                    "outcome": "success",
                    "snapshot_schema": sample_object["snapshot_schema"],
                    "semantic_hash": sample_object["semantic_hash"],
                    "semantic_projection_sha256": sample_object["semantic_projection_sha256"],
                    "counts": sample_object["counts"],
                }
                if (
                    sample_object["exit_code"] != 0
                    or sample_object["stderr_bytes"] != 0
                    or sample_object["stdout_bytes"] <= 0
                ):
                    raise BenchmarkError("benchmark.sample_failed", "positive sample contains a failure")
            else:
                observed_oracle = {
                    "outcome": "typed_rejection",
                    "exit_code": sample_object["exit_code"],
                    "stdout_bytes": sample_object["stdout_bytes"],
                    "error_schema": sample_object["error_schema"],
                    "error_code": sample_object["error_code"],
                    "error_stage": sample_object["error_stage"],
                    "error_reason": sample_object["error_reason"],
                    "store_created": sample_object["store_created"],
                    "nested_source_read": sample_object["nested_source_read"],
                }
                if sample_object["stderr_bytes"] <= 0:
                    raise BenchmarkError("benchmark.sample_failed", "typed rejection is missing stderr")
            if observed_oracle != EXPECTED_ORACLE_ENTRIES[entry_id]:
                raise BenchmarkError("benchmark.oracle_mismatch", "sample does not match the fixed oracle")
        if entry["percentiles_ns"] != {
            "p50": nearest_rank(wall_times, 50),
            "p95": nearest_rank(wall_times, 95),
            "p99": nearest_rank(wall_times, 99),
        }:
            raise BenchmarkError("benchmark.invalid_report", "report percentiles are invalid")
    return document


def compare_reports(
    baseline: Any,
    candidate: Any,
    policy: dict[str, Any],
    policy_sha256: str,
) -> dict[str, Any]:
    baseline_report = validate_report(
        baseline, policy, expected_policy_sha256=policy_sha256
    )
    candidate_report = validate_report(
        candidate, policy, expected_policy_sha256=policy_sha256
    )
    if baseline_report["product"]["commit"] != BOOTSTRAP_BASELINE_COMMIT:
        raise BenchmarkError("benchmark.incomparable", "baseline product commit is not authoritative")
    for field in (
        "suite_id",
        "corpus",
        "corpus_version",
        "host",
        "repetitions",
        "cache_state",
        "concurrency",
        "enabled_extractors",
        "percentile_method",
        "minimum_success_rate",
        "manifest_sha256",
        "oracle_sha256",
        "policy_sha256",
        "runner_version",
    ):
        if baseline_report[field] != candidate_report[field]:
            raise BenchmarkError("benchmark.incomparable", f"report {field} differs")
    results: dict[str, Any] = {}
    for entry_id in EXPECTED_ENTRY_IDS:
        baseline_entry = baseline_report["entries"][entry_id]
        candidate_entry = candidate_report["entries"][entry_id]
        if baseline_entry["outcome"] != candidate_entry["outcome"]:
            raise BenchmarkError("benchmark.oracle_mismatch", "entry outcome changed")
        if baseline_entry["profiles"] != candidate_entry["profiles"]:
            raise BenchmarkError("benchmark.incomparable", "entry profiles differ")
        stable_fields = (
            ("semantic_hash", "semantic_projection_sha256", "counts")
            if entry_id == "lekton"
            else (
                "error_schema",
                "error_code",
                "error_stage",
                "error_reason",
                "store_created",
                "nested_source_read",
            )
        )
        for field in stable_fields:
            baseline_values = [sample.get(field) for sample in baseline_entry["samples"]]
            candidate_values = [sample.get(field) for sample in candidate_entry["samples"]]
            if len({json.dumps(value, sort_keys=True) for value in baseline_values + candidate_values}) != 1:
                raise BenchmarkError("benchmark.oracle_mismatch", f"{entry_id} semantic outcome drifted")
        baseline_p95 = baseline_entry["percentiles_ns"]["p95"]
        candidate_p95 = candidate_entry["percentiles_ns"]["p95"]
        ratio = Fraction(str(policy["candidate_p95_ratio_max"]))
        ratio_limit = baseline_p95 * ratio.numerator // ratio.denominator
        additive_limit = baseline_p95 + policy["candidate_p95_additive_ns_max"]
        allowed_p95 = max(ratio_limit, additive_limit)
        absolute_p95 = policy["absolute_p95_ns_max"][entry_id]
        if candidate_p95 > allowed_p95 or candidate_p95 > absolute_p95:
            raise BenchmarkError("benchmark.regression", f"{entry_id} candidate p95 exceeds policy")
        results[entry_id] = {
            "absolute_p95_ns_max": absolute_p95,
            "allowed_candidate_p95_ns": allowed_p95,
            "baseline_p95_ns": baseline_p95,
            "candidate_p95_ns": candidate_p95,
            "semantic_and_outcome_identity": True,
        }
    return {
        "baseline_product_commit": baseline_report["product"]["commit"],
        "candidate_product_commit": candidate_report["product"]["commit"],
        "result": "pass",
        "results": results,
        "schema_version": COMPARISON_SCHEMA,
        "suite_id": SUITE_ID,
    }


def compare_benchmark(arguments: argparse.Namespace) -> dict[str, Any]:
    policy_path = ensure_regular_file(Path(arguments.policy), "policy")
    root = policy_path.parent.parent.parent
    canonical_policy = root / "benchmarks/policies/real-world-rust-stability-v1.json"
    if policy_path != canonical_policy:
        raise BenchmarkError("benchmark.invalid_input", "policy path is not the active suite policy")
    _, _, _, policy, _, digests = load_contracts(
        root / "benchmarks/manifest.json", SUITE_ID
    )
    maximum_bytes = policy.get("report_bytes_max")
    if type(maximum_bytes) is not int or maximum_bytes <= 0:
        raise BenchmarkError("benchmark.invalid_input", "policy report byte limit is invalid")
    baseline_path = ensure_regular_file(Path(arguments.baseline), "baseline report")
    candidate_path = ensure_regular_file(Path(arguments.candidate), "candidate report")
    baseline = load_json(baseline_path, maximum_bytes=maximum_bytes)
    candidate = load_json(candidate_path, maximum_bytes=maximum_bytes)
    policy_sha256 = digests["policy"]
    for report in (baseline, candidate):
        report_object = require_object(report, "benchmark report")
        report_corpus = require_object(report_object.get("corpus"), "report corpus")
        if (
            report_object.get("manifest_sha256") != digests["manifest"]
            or report_object.get("oracle_sha256") != digests["oracle"]
            or report_object.get("policy_sha256") != digests["policy"]
            or report_corpus.get("descriptor_sha256") != digests["corpus"]
        ):
            raise BenchmarkError("benchmark.incomparable", "report contract identity is not current")
    comparison = compare_reports(baseline, candidate, policy, policy_sha256)
    encoded = canonical_json_bytes(comparison)
    ensure_privacy(encoded, privacy_canaries([baseline_path, candidate_path, policy_path]))
    if len(encoded) > maximum_bytes:
        raise BenchmarkError("benchmark.limit_exceeded", "comparison exceeds its byte limit")
    return comparison


def parser() -> FailClosedParser:
    command_parser = FailClosedParser(description=__doc__)
    subcommands = command_parser.add_subparsers(dest="command", required=True)
    run_parser = subcommands.add_parser("run")
    run_parser.add_argument("--manifest", required=True)
    run_parser.add_argument("--suite", required=True)
    run_parser.add_argument("--binary", required=True)
    run_parser.add_argument("--product-commit", required=True)
    run_parser.add_argument("--lekton", required=True)
    run_parser.add_argument("--rustdesk", required=True)
    run_parser.add_argument("--host-profile", required=True)
    run_parser.add_argument("--memory-class-bytes", required=True, type=int)
    run_parser.add_argument("--output", required=True)
    compare_parser = subcommands.add_parser("compare")
    compare_parser.add_argument("--baseline", required=True)
    compare_parser.add_argument("--candidate", required=True)
    compare_parser.add_argument("--policy", required=True)
    return command_parser


def emit_error(error: BenchmarkError) -> int:
    document = {
        "code": error.code,
        "message": error.message[:256],
        "retryable": False,
        "schema_version": ERROR_SCHEMA,
        "stage": error.stage,
    }
    encoded = canonical_json_bytes(document)
    if len(encoded) > 2048:
        encoded = canonical_json_bytes(
            {
                "code": "benchmark.internal",
                "message": "bounded error serialization failed",
                "retryable": False,
                "schema_version": ERROR_SCHEMA,
                "stage": "internal",
            }
        )
        return_code = 1
    else:
        return_code = error.exit_code
    sys.stderr.buffer.write(encoded)
    sys.stderr.buffer.flush()
    return return_code


def main(arguments: Sequence[str] | None = None) -> int:
    try:
        parsed = parser().parse_args(arguments)
        result = run_benchmark(parsed) if parsed.command == "run" else compare_benchmark(parsed)
        sys.stdout.buffer.write(canonical_json_bytes(result))
        sys.stdout.buffer.flush()
        return 0
    except BenchmarkError as error:
        return emit_error(error)
    except Exception:
        return emit_error(
            BenchmarkError(
                "benchmark.internal",
                "unexpected benchmark completion failure",
                stage="internal",
                exit_code=1,
            )
        )


if __name__ == "__main__":
    raise SystemExit(main())
