#!/usr/bin/env python3
"""Generate explorable CodeNoesis ontologies for pinned Lekton and RustDesk clones."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any, NoReturn, Sequence

if __package__:
    from .audit_ontology_information import (
        OntologyAuditError,
        audit_ontology_information,
    )
else:
    from audit_ontology_information import (
        OntologyAuditError,
        audit_ontology_information,
    )

RUNNER_VERSION = "codenoesis.real-world-rust-pilot-runner/v3"
SUMMARY_SCHEMA = "codenoesis.real-world-rust-pilot-summary/v3"
ERROR_SCHEMA = "codenoesis.real-world-rust-pilot-error/v1"


class PilotError(Exception):
    """Expected fail-closed pilot error."""

    def __init__(self, code: str, message: str, *, stage: str = "input"):
        super().__init__(message)
        self.code = code
        self.message = message
        self.stage = stage


class FailClosedParser(argparse.ArgumentParser):
    """Convert argument failures into the public runner error."""

    def error(self, message: str) -> NoReturn:
        del message
        raise PilotError("pilot.invalid_arguments", "invalid command-line arguments")


@dataclass(frozen=True)
class PilotSpec:
    name: str
    repository_id: str
    revision: str
    tree: str
    scan_options: tuple[str, ...]
    portable_profile: str
    explorer_profile: str
    llm_context_profile: str
    export_documents: bool


PILOTS = (
    PilotSpec(
        name="lekton",
        repository_id="urn:codenoesis:pilot:lekton:r16",
        revision="247b8f42fb045db41166d70a276a41c2e079b6eb",
        tree="55ba428493a4ffae86ba492422a049f46d567a30",
        scan_options=(
            "--acquisition-profile",
            "local-git-sha1-packed-v1",
            "--workspace-profile",
            "cargo-root-package-v1",
            "--manifest-profile",
            "cargo-manifest-facts-v1",
            "--rust-semantic-profile",
            "rust-semantic-depth-v1",
            "--rust-framework-profile",
            "rust-framework-declarations-v1",
            "--rust-callable-profile",
            "rust-callable-semantics-v1",
            "--rust-expression-profile",
            "rust-expression-bindings-v1",
            "--rust-flow-profile",
            "rust-local-flow-v1",
            "--rust-constant-profile",
            "rust-safe-constant-evaluation-v1",
            "--output-capacity-profile",
            "local-snapshot-256m-v1",
            "--execution-limit-profile",
            "real-world-rust-benchmark-75s-v1",
        ),
        portable_profile="rust-safe-constant-evaluation-v1",
        explorer_profile="rust-function-context-v1",
        llm_context_profile="rust-llm-context-v1",
        export_documents=True,
    ),
    PilotSpec(
        name="rustdesk",
        repository_id="urn:codenoesis:pilot:rustdesk:r16",
        revision="d412d198720aa56f6cfed2dfad262e8fb1322fb7",
        tree="df8d4c292c9d256a445480eb878e507df3de1dc4",
        scan_options=(
            "--acquisition-profile",
            "local-git-sha1-packed-v1",
            "--repository-boundary-profile",
            "local-gitlinks-v1",
            "--workspace-profile",
            "cargo-root-package-v1",
            "--manifest-profile",
            "cargo-manifest-facts-v1",
            "--rust-semantic-profile",
            "rust-cfg-declaration-alternatives-v1",
            "--rust-framework-profile",
            "rust-framework-declarations-v1",
            "--rust-callable-profile",
            "rust-callable-semantics-v1",
            "--rust-expression-profile",
            "rust-expression-bindings-v1",
            "--rust-flow-profile",
            "rust-local-flow-v1",
            "--rust-constant-profile",
            "rust-safe-constant-evaluation-v1",
            "--output-capacity-profile",
            "local-snapshot-256m-v1",
            "--execution-limit-profile",
            "real-world-rust-benchmark-75s-v1",
        ),
        portable_profile="rust-safe-constant-evaluation-v1",
        explorer_profile="rust-function-context-v1",
        llm_context_profile="rust-llm-context-v1",
        export_documents=True,
    ),
)


def canonical_json_bytes(value: Any) -> bytes:
    return (
        json.dumps(
            value,
            ensure_ascii=False,
            allow_nan=False,
            separators=(",", ":"),
            sort_keys=True,
        ).encode("utf-8")
        + b"\n"
    )


def build_scan_command(
    binary: Path, repository: Path, store: Path, spec: PilotSpec
) -> list[str]:
    return [
        str(binary),
        "scan",
        "--repository",
        str(repository),
        "--repository-id",
        spec.repository_id,
        "--revision",
        spec.revision,
        "--profile",
        "standard-local-s4",
        *spec.scan_options,
        "--store",
        str(store),
        "--format",
        "json",
    ]


def build_docs_command(binary: Path, store: Path, output: Path, spec: PilotSpec) -> list[str]:
    return [
        str(binary),
        "docs",
        "--store",
        str(store),
        "--repository-id",
        spec.repository_id,
        "--output",
        str(output),
        "--format",
        "json",
    ]


def build_export_command(
    binary: Path, store: Path, documents: Path, output: Path, spec: PilotSpec
) -> list[str]:
    command = [
        str(binary),
        "export",
        "--store",
        str(store),
        "--repository-id",
        spec.repository_id,
    ]
    if spec.export_documents:
        command.extend(("--documents", str(documents)))
    command.extend(
        (
            "--output",
            str(output),
            "--portable-profile",
            spec.portable_profile,
            "--format",
            "json",
        )
    )
    return command


def build_explore_command(
    binary: Path, portable_graph: Path, output: Path, spec: PilotSpec
) -> list[str]:
    return [
        str(binary),
        "explore",
        "--input",
        str(portable_graph),
        "--output",
        str(output),
        "--explorer-profile",
        spec.explorer_profile,
        "--format",
        "json",
    ]


def build_llm_context_command(
    binary: Path,
    store: Path,
    documents: Path,
    callable_id: str,
    spec: PilotSpec,
) -> list[str]:
    return [
        str(binary),
        "query",
        "--store",
        str(store),
        "--repository-id",
        spec.repository_id,
        "--documents",
        str(documents),
        "--id",
        callable_id,
        "--context-profile",
        spec.llm_context_profile,
        "--format",
        "json",
    ]


def run_git(repository: Path, arguments: Sequence[str]) -> str:
    environment = os.environ.copy()
    environment.update(
        {
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_OPTIONAL_LOCKS": "0",
            "GIT_TERMINAL_PROMPT": "0",
        }
    )
    result = subprocess.run(
        ["git", "-C", str(repository), *arguments],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=15,
        env=environment,
    )
    if result.returncode != 0:
        raise PilotError("pilot.invalid_repository", "repository preflight failed")
    try:
        return result.stdout.decode("ascii").strip()
    except UnicodeDecodeError as error:
        raise PilotError(
            "pilot.invalid_repository", "repository identity is not ASCII"
        ) from error


def validate_repository(repository: Path, spec: PilotSpec) -> Path:
    try:
        resolved = repository.resolve(strict=True)
    except OSError as error:
        raise PilotError("pilot.invalid_repository", f"{spec.name} repository is unavailable") from error
    if not resolved.is_dir():
        raise PilotError("pilot.invalid_repository", f"{spec.name} repository is not a directory")
    revision = run_git(resolved, ("rev-parse", "--verify", f"{spec.revision}^{{commit}}"))
    tree = run_git(resolved, ("rev-parse", "--verify", f"{spec.revision}^{{tree}}"))
    if revision != spec.revision or tree != spec.tree:
        raise PilotError("pilot.repository_mismatch", f"{spec.name} pinned revision is unavailable")
    return resolved


def validate_binary(binary: Path) -> Path:
    try:
        resolved = binary.resolve(strict=True)
    except OSError as error:
        raise PilotError("pilot.invalid_binary", "CodeNoesis binary is unavailable") from error
    if not resolved.is_file() or not os.access(resolved, os.X_OK):
        raise PilotError("pilot.invalid_binary", "CodeNoesis binary is not executable")
    return resolved


def path_contains(parent: Path, child: Path) -> bool:
    try:
        child.relative_to(parent)
        return True
    except ValueError:
        return False


def prepare_output_root(output: Path, repositories: Sequence[Path]) -> Path:
    if output.exists():
        raise PilotError("pilot.output_exists", "output root already exists")
    try:
        parent = output.parent.resolve(strict=True)
    except OSError as error:
        raise PilotError("pilot.invalid_output", "output parent is unavailable") from error
    resolved = parent / output.name
    if any(
        path_contains(repository, resolved) or path_contains(resolved, repository)
        for repository in repositories
    ):
        raise PilotError("pilot.invalid_output", "output root overlaps an input repository")
    try:
        resolved.mkdir()
        (resolved / ".codenoesis-real-world-rust-pilots-v3").write_text(
            RUNNER_VERSION + "\n", encoding="utf-8"
        )
    except OSError as error:
        raise PilotError("pilot.invalid_output", "output root cannot be created") from error
    return resolved


def run_stage(
    command: Sequence[str],
    stage: str,
    stderr_path: Path,
    timeout_seconds: int,
    stdout_path: Path | None = None,
) -> None:
    try:
        with stderr_path.open("wb") as stderr_handle:
            if stdout_path is None:
                result = subprocess.run(
                    command,
                    check=False,
                    stdout=subprocess.DEVNULL,
                    stderr=stderr_handle,
                    timeout=timeout_seconds,
                )
            else:
                with stdout_path.open("wb") as stdout_handle:
                    result = subprocess.run(
                        command,
                        check=False,
                        stdout=stdout_handle,
                        stderr=stderr_handle,
                        timeout=timeout_seconds,
                    )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise PilotError("pilot.stage_failed", f"{stage} did not complete", stage=stage) from error
    if result.returncode != 0 or stderr_path.stat().st_size != 0:
        raise PilotError("pilot.stage_failed", f"{stage} failed", stage=stage)


def load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_bytes())
    except (OSError, json.JSONDecodeError) as error:
        raise PilotError("pilot.invalid_artifact", "generated JSON artifact is invalid") from error
    if not isinstance(value, dict):
        raise PilotError("pilot.invalid_artifact", "generated JSON artifact has the wrong shape")
    return value


def snapshot_summary(snapshot_path: Path) -> dict[str, Any]:
    snapshot = load_json(snapshot_path)
    try:
        semantic = snapshot["semantic"]
        graph = semantic["knowledge_graph"]
        entities = graph["entities"]
        return {
            "snapshot_schema": snapshot["schema_version"],
            "ontology_version": semantic["ontology_version"],
            "semantic_hash": snapshot["semantic_hash"]["value"],
            "counts": {
                "entities": len(entities),
                "relationships": len(graph["relationships"]),
                "claims": len(graph["claims"]),
                "evidence": len(graph["evidence"]),
                "diagnostics": len(graph["diagnostics"]),
                "coverage": len(graph["coverage"]),
            },
        }
    except (KeyError, TypeError) as error:
        raise PilotError("pilot.invalid_artifact", "snapshot graph is incomplete") from error


def select_representative_callable(portable_graph: dict[str, Any]) -> dict[str, str]:
    entities = portable_graph.get("entities")
    relationships = portable_graph.get("relationships")
    if not isinstance(entities, list) or not isinstance(relationships, list):
        raise PilotError("pilot.invalid_artifact", "portable graph is incomplete")
    degree: dict[str, int] = {}
    signature_ids: dict[str, list[str]] = {}
    parameter_counts: dict[str, int] = {}
    body_counts: dict[str, int] = {}
    outgoing_call_counts: dict[str, int] = {}
    incoming_call_counts: dict[str, int] = {}
    for relationship in relationships:
        if not isinstance(relationship, dict):
            continue
        source = relationship.get("source")
        target = relationship.get("target")
        kind = relationship.get("kind")
        if isinstance(source, str):
            degree[source] = degree.get(source, 0) + 1
        if isinstance(target, str):
            degree[target] = degree.get(target, 0) + 1
        if not all(isinstance(value, str) for value in (source, target, kind)):
            continue
        if kind == "HAS_SIGNATURE":
            signature_ids.setdefault(source, []).append(target)
        elif kind == "HAS_PARAMETER":
            parameter_counts[source] = parameter_counts.get(source, 0) + 1
        elif kind == "HAS_BODY_FACT":
            body_counts[source] = body_counts.get(source, 0) + 1
        elif kind == "CALLS":
            outgoing_call_counts[source] = outgoing_call_counts.get(source, 0) + 1
            incoming_call_counts[target] = incoming_call_counts.get(target, 0) + 1
    candidates = []
    bounded_candidates = []
    for entity in entities:
        if not isinstance(entity, dict) or entity.get("kind") not in {
            "rust.function",
            "rust.method",
        }:
            continue
        identifier = entity.get("id")
        kind = entity.get("kind")
        name = entity.get("name")
        if not all(isinstance(value, str) and value for value in (identifier, kind, name)):
            raise PilotError("pilot.invalid_artifact", "callable identity is invalid")
        candidates.append((degree.get(identifier, 0), identifier, kind, name))
        signatures = signature_ids.get(identifier, [])
        if len(signatures) != 1:
            continue
        parameters = parameter_counts.get(signatures[0], 0)
        body_facts = body_counts.get(identifier, 0)
        outgoing_calls = outgoing_call_counts.get(identifier, 0)
        incoming_calls = incoming_call_counts.get(identifier, 0)
        if parameters > 16 or body_facts > 64 or outgoing_calls + incoming_calls > 32:
            continue
        bounded_candidates.append(
            (
                0 if outgoing_calls else 1,
                0 if body_facts else 1,
                0 if parameters else 1,
                abs(body_facts - 8),
                degree.get(identifier, 0),
                identifier,
                kind,
                name,
            )
        )
    if not candidates:
        raise PilotError("pilot.invalid_artifact", "portable graph has no callable")
    if bounded_candidates:
        _, _, _, _, _, identifier, kind, name = min(bounded_candidates)
    else:
        _, identifier, kind, name = min(candidates)
    return {"id": identifier, "kind": kind, "name": name}


def validate_llm_context(
    path: Path, representative: dict[str, str], spec: PilotSpec
) -> dict[str, Any]:
    context = load_json(path)
    focus = context.get("focus")
    if (
        context.get("schema_version") != "codenoesis.llm-function-context/v1"
        or context.get("profile") != spec.llm_context_profile
        or context.get("authority") != "declared_source_only"
        or context.get("model_authority") is not False
        or not isinstance(focus, dict)
        or focus.get("id") != representative["id"]
        or focus.get("kind") != representative["kind"]
        or focus.get("name") != representative["name"]
    ):
        raise PilotError("pilot.invalid_artifact", "LLM context contract is invalid")
    return {
        "byte_length": path.stat().st_size,
        "callable": representative,
        "profile": context["profile"],
        "schema_version": context["schema_version"],
    }


def run_pilot(
    binary: Path,
    repository: Path,
    output: Path,
    spec: PilotSpec,
    timeout_seconds: int,
) -> dict[str, Any]:
    root = output / spec.name
    logs = root / "logs"
    logs.mkdir(parents=True)
    store = root / "store"
    documents = root / "documents"
    portable = root / "portable"
    explorer = root / "explorer"
    snapshot = root / "snapshot.json"
    information_audit = root / "information-audit.json"
    llm_context = root / "llm-context.json"
    run_stage(
        build_scan_command(binary, repository, store, spec),
        f"{spec.name}.scan",
        logs / "scan.stderr.json",
        timeout_seconds,
        snapshot,
    )
    run_stage(
        build_docs_command(binary, store, documents, spec),
        f"{spec.name}.docs",
        logs / "docs.stderr.json",
        timeout_seconds,
    )
    portable_graph = portable / "portable-graph.json"
    run_stage(
        build_export_command(binary, store, documents, portable, spec),
        f"{spec.name}.export",
        logs / "export.stderr.json",
        timeout_seconds,
    )
    portable_graph_value = load_json(portable_graph)
    try:
        audit = audit_ontology_information(portable_graph_value)
    except OntologyAuditError as error:
        raise PilotError(
            "pilot.invalid_artifact",
            f"{spec.name} ontology information audit failed",
            stage=f"{spec.name}.audit",
        ) from error
    information_audit.write_bytes(canonical_json_bytes(audit))
    if audit["verdict"] == "insufficient_information":
        raise PilotError(
            "pilot.insufficient_information",
            f"{spec.name} ontology is missing required reasoning information",
            stage=f"{spec.name}.audit",
        )
    representative = select_representative_callable(portable_graph_value)
    run_stage(
        build_llm_context_command(
            binary,
            store,
            documents,
            representative["id"],
            spec,
        ),
        f"{spec.name}.llm_context",
        logs / "llm-context.stderr.json",
        timeout_seconds,
        llm_context,
    )
    llm_context_summary = validate_llm_context(llm_context, representative, spec)
    run_stage(
        build_explore_command(binary, portable_graph, explorer, spec),
        f"{spec.name}.explore",
        logs / "explore.stderr.json",
        timeout_seconds,
    )
    index = explorer / "index.html"
    if not portable_graph.is_file() or not index.is_file():
        raise PilotError("pilot.invalid_artifact", f"{spec.name} explorer is incomplete")
    summary = snapshot_summary(snapshot)
    summary.update(
        {
            "name": spec.name,
            "repository_id": spec.repository_id,
            "revision": spec.revision,
            "tree": spec.tree,
            "portable_profile": spec.portable_profile,
            "explorer_profile": spec.explorer_profile,
            "llm_context": llm_context_summary,
            "information_audit": {
                "checks": {
                    check["capability"]: check["status"] for check in audit["checks"]
                },
                "reasoning_readiness": audit["reasoning_readiness"],
                "verdict": audit["verdict"],
            },
            "artifacts": {
                "snapshot": str(snapshot),
                "documents": str(documents),
                "portable_graph": str(portable_graph),
                "explorer": str(index),
                "information_audit": str(information_audit),
                "llm_context": str(llm_context),
            },
        }
    )
    return summary


def parser() -> FailClosedParser:
    command_parser = FailClosedParser(description=__doc__)
    command_parser.add_argument("--binary", required=True)
    command_parser.add_argument("--lekton", required=True)
    command_parser.add_argument("--rustdesk", required=True)
    command_parser.add_argument("--output", required=True)
    command_parser.add_argument("--timeout-seconds", type=int, default=180)
    return command_parser


def emit_error(error: PilotError) -> int:
    sys.stderr.buffer.write(
        canonical_json_bytes(
            {
                "code": error.code,
                "message": error.message,
                "retryable": False,
                "schema_version": ERROR_SCHEMA,
                "stage": error.stage,
            }
        )
    )
    sys.stderr.buffer.flush()
    return 2


def main(arguments: Sequence[str] | None = None) -> int:
    try:
        parsed = parser().parse_args(arguments)
        if parsed.timeout_seconds < 1 or parsed.timeout_seconds > 900:
            raise PilotError("pilot.invalid_arguments", "timeout must be between 1 and 900 seconds")
        binary = validate_binary(Path(parsed.binary))
        repositories = {
            PILOTS[0].name: validate_repository(Path(parsed.lekton), PILOTS[0]),
            PILOTS[1].name: validate_repository(Path(parsed.rustdesk), PILOTS[1]),
        }
        output = prepare_output_root(Path(parsed.output), tuple(repositories.values()))
        pilots = [
            run_pilot(
                binary,
                repositories[spec.name],
                output,
                spec,
                parsed.timeout_seconds,
            )
            for spec in PILOTS
        ]
        summary = {
            "schema_version": SUMMARY_SCHEMA,
            "runner_version": RUNNER_VERSION,
            "pilots": pilots,
        }
        (output / "summary.json").write_bytes(canonical_json_bytes(summary))
        sys.stdout.buffer.write(canonical_json_bytes(summary))
        sys.stdout.buffer.flush()
        return 0
    except PilotError as error:
        return emit_error(error)
    except Exception:
        return emit_error(PilotError("pilot.internal", "unexpected pilot failure", stage="internal"))


if __name__ == "__main__":
    raise SystemExit(main())
