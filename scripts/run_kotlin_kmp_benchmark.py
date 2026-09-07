#!/usr/bin/env python3
"""Measure pinned Kotlin repositories; retain every attempt, including failures."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import platform
import statistics
import subprocess
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--corpus", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    manifest = json.loads((root / "tests/specifications/s8/kotlin-kmp-public-v1.json").read_text())
    binary = args.binary.resolve(strict=True)
    corpus = args.corpus.resolve(strict=True)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    report = {"schema_version": "codenoesis.kotlin-benchmark-observation/v1",
              "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
              "product_head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip(),
              "product_dirty": bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=root)),
              "environment": {"os": platform.platform(), "architecture": platform.machine()},
              "accuracy": "not measured on public repositories; hand-authored fixture oracle is separate",
              "samples": [], "summary": []}
    for repository in manifest["repositories"]:
        samples = []
        for repeat in range(1, manifest["repetitions"] + 1):
            prefix = f"{repository['name']}-{repeat}"
            command = [str(binary), "scan", "--profile", "standard-local-s8", "--kotlin-profile",
                       "kotlin-kmp-declarations-v1", "--repository", str(corpus / repository["name"]),
                       "--revision", repository["revision"], "--repository-identity",
                       f"urn:codenoesis:repository:kotlin-{repository['name']}", "--store", str(output / f"{prefix}-store")]
            started = time.perf_counter()
            try:
                process = subprocess.run(command, capture_output=True, timeout=90, cwd=output, check=False)
                stdout, stderr, exit_code = process.stdout, process.stderr, process.returncode
            except subprocess.TimeoutExpired as error:
                stdout, stderr, exit_code = error.stdout or b"", error.stderr or b"", "timeout"
            elapsed = time.perf_counter() - started
            (output / f"{prefix}.stdout.json").write_bytes(stdout)
            (output / f"{prefix}.stderr.json").write_bytes(stderr)
            sample = {"repository": repository["repository"], "revision": repository["revision"],
                      "repeat": repeat, "exit_code": exit_code, "seconds": elapsed,
                      "stdout_bytes": len(stdout), "stdout_sha256": hashlib.sha256(stdout).hexdigest(),
                      "stderr_sha256": hashlib.sha256(stderr).hexdigest(), "command": command}
            if exit_code == 0:
                snapshot = json.loads(stdout)
                graph = snapshot["semantic"]["knowledge_graph"]
                kinds = Counter(entity["kind"] for entity in graph["entities"])
                sample.update(semantic_hash=snapshot["semantic_hash"]["value"], entities=len(graph["entities"]),
                              entity_kinds=dict(sorted(kinds.items())), relationships=len(graph["relationships"]),
                              claims=len(graph["claims"]), evidence=len(graph["evidence"]), gaps=len(graph["coverage"]),
                              gap_reasons=dict(sorted(Counter(gap["reason"] for gap in graph["coverage"]).items())),
                              syntax_gap_paths=[gap["path"] for gap in graph["coverage"] if gap["reason"] == "kotlin_syntax_not_accepted_by_pinned_parser"])
            else:
                try:
                    sample["error"] = json.loads(stderr)
                except (ValueError, UnicodeDecodeError):
                    sample["error"] = "non-JSON error or timeout; inspect retained stderr"
            samples.append(sample)
            report["samples"].append(sample)
            (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        hashes = {sample.get("semantic_hash") for sample in samples}
        report["summary"].append({**repository, "successes": sum(s["exit_code"] == 0 for s in samples),
                                  "repetitions": len(samples), "semantic_hash_stable": len(hashes) == 1 and None not in hashes,
                                  "median_seconds": statistics.median(s["seconds"] for s in samples),
                                  "min_seconds": min(s["seconds"] for s in samples), "max_seconds": max(s["seconds"] for s in samples)})
    (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report["summary"], indent=2))
    return 0 if all(s["successes"] == 3 and s["semantic_hash_stable"] for s in report["summary"]) else 2


if __name__ == "__main__":
    raise SystemExit(main())
