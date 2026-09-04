from __future__ import annotations

import copy
import json
import subprocess
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CORPUS = ROOT / "benchmarks/corpora/public-rust-conference-v1.json"
POLICY = ROOT / "benchmarks/policies/public-rust-conference-v1.json"
ORACLE = ROOT / "benchmarks/baselines/public-rust-conference-v1.json"
RUNNER = ROOT / "scripts/run_public_rust_evaluation.py"
sys.path.insert(0, str(ROOT / "scripts"))

from run_public_rust_evaluation import (  # noqa: E402
    EvaluationError,
    REPORT_SCHEMA,
    RUNNER_VERSION,
    STAGES,
    aggregate_stage_coverage,
    build_scan_command,
    canonical_json_bytes,
    graph_metrics,
    load_contracts,
    nearest_rank,
    terminal_sample_matches,
    validate_product_commit,
)


class PublicRustEvaluationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.corpus, cls.policy, cls.oracle = load_contracts(CORPUS, POLICY, ORACLE)

    def test_corpus_is_pinned_diverse_and_locally_supplied(self) -> None:
        entries = self.corpus["entries"]

        self.assertEqual(self.corpus["id"], "public-rust-conference")
        self.assertEqual(self.corpus["version"], "1")
        self.assertFalse(self.corpus["network_allowed"])
        self.assertFalse(self.corpus["source_vendored"])
        self.assertEqual(
            [entry["id"] for entry in entries],
            ["hyperfine", "tower", "mio", "fd", "delta", "rustfmt", "dioxus", "wgpu"],
        )
        self.assertEqual(len({entry["archetype"] for entry in entries}), len(entries))
        self.assertTrue(all(len(entry["revision"]) == 40 for entry in entries))
        self.assertTrue(all(len(entry["tree"]) == 40 for entry in entries))
        self.assertGreaterEqual(max(entry["rust_source_bytes"] for entry in entries), 10_000_000)
        self.assertLessEqual(min(entry["rust_source_bytes"] for entry in entries), 200_000)

    def test_policy_and_oracle_cover_every_progressive_stage(self) -> None:
        entries = self.oracle["entries"]

        self.assertEqual(tuple(self.policy["stage_order"]), STAGES)
        self.assertEqual(self.policy["repetitions"], 3)
        self.assertEqual(self.policy["minimum_constant_stage_successes"], 3)
        self.assertEqual(set(entries), {entry["id"] for entry in self.corpus["entries"]})
        self.assertEqual(
            {entry["terminal_stage"] for entry in entries.values()},
            {"acquisition", "workspace", "manifest", "semantic", "flow", "constant"},
        )
        self.assertEqual(
            sum(entry["outcome"] == "success" for entry in entries.values()),
            3,
        )

    def test_stage_commands_add_only_the_required_profiles(self) -> None:
        descriptor = self.corpus["entries"][0]
        binary = Path("/tmp/noesis")
        repository = Path("/tmp/repository")
        store = Path("/tmp/store")

        acquisition = build_scan_command(binary, repository, store, descriptor, "acquisition")
        constant = build_scan_command(binary, repository, store, descriptor, "constant")

        self.assertIn("standard-local-s1", acquisition)
        self.assertNotIn("--store", acquisition)
        self.assertIn("standard-local-s4", constant)
        self.assertIn("--rust-constant-profile", constant)
        self.assertIn("real-world-rust-benchmark-75s-v1", constant)
        self.assertEqual(constant[-4:], ["--store", str(store), "--format", "json"])

    def test_graph_metrics_capture_reasoning_information(self) -> None:
        snapshot = {
            "schema_version": "codenoesis.repository-snapshot/v18",
            "semantic_hash": {"algorithm": "blake3-256", "value": "a" * 64},
            "semantic": {
                "knowledge_graph": {
                    "entities": [
                        {"kind": "rust.function"},
                        {"kind": "rust.callable_signature"},
                        {"kind": "rust.parameter"},
                        {"kind": "rust.call_site"},
                        {"kind": "rust.enum"},
                        {"kind": "rust.enum_variant"},
                        {"kind": "rust.evaluated_value"},
                    ],
                    "relationships": [{"kind": "CALLS"}],
                    "claims": [{"evidence_ids": ["evidence"]}],
                    "evidence": [{}],
                    "diagnostics": [{"code": "rust.unresolved"}],
                    "coverage": [{"state": "not_resolved"}],
                }
            },
        }

        metrics = graph_metrics(snapshot)

        self.assertEqual(metrics["counts"]["entities"], 7)
        self.assertEqual(metrics["information"]["callables"], 1)
        self.assertEqual(metrics["information"]["parameters"], 1)
        self.assertEqual(metrics["information"]["enum_variants"], 1)
        self.assertEqual(metrics["information"]["evaluated_values"], 1)
        self.assertEqual(metrics["information"]["resolved_call_basis_points"], 10_000)
        self.assertEqual(metrics["information"]["claim_evidence_basis_points"], 10_000)

    def test_terminal_samples_match_exact_success_and_rejection_oracles(self) -> None:
        success = self.oracle["entries"]["mio"]
        rejection = self.oracle["entries"]["dioxus"]
        success_sample = {
            "outcome": "success",
            "snapshot_schema": success["snapshot_schema"],
            "semantic_hash": success["semantic_hash"],
            "semantic_projection_sha256": success["semantic_projection_sha256"],
            "counts": success["counts"],
        }
        rejection_sample = {
            "outcome": "typed_rejection",
            "exit_code": rejection["exit_code"],
            "error_schema": rejection["error_schema"],
            "error_code": rejection["error_code"],
            "error_stage": rejection["error_stage"],
            "error_context": copy.deepcopy(rejection["error_context"]),
        }

        self.assertTrue(terminal_sample_matches(success_sample, success))
        self.assertTrue(terminal_sample_matches(rejection_sample, rejection))
        rejection_sample["error_context"]["observed"] += 1
        self.assertFalse(terminal_sample_matches(rejection_sample, rejection))

    def test_stage_coverage_is_aggregate_and_honest(self) -> None:
        entries = {
            "one": {"highest_successful_stage": "constant"},
            "two": {"highest_successful_stage": "manifest"},
            "three": {"highest_successful_stage": None},
        }

        coverage = aggregate_stage_coverage(entries, STAGES)

        self.assertEqual(coverage["acquisition"], {"count": 2, "basis_points": 6666})
        self.assertEqual(coverage["manifest"], {"count": 2, "basis_points": 6666})
        self.assertEqual(coverage["constant"], {"count": 1, "basis_points": 3333})

    def test_nearest_rank_uses_raw_samples(self) -> None:
        self.assertEqual(nearest_rank([31, 11, 21], 50), 21)
        self.assertEqual(nearest_rank([31, 11, 21], 95), 31)

    def test_report_identity_constants_are_stable(self) -> None:
        self.assertEqual(RUNNER_VERSION, "codenoesis.public-rust-evaluation-runner/v1")
        self.assertEqual(REPORT_SCHEMA, "codenoesis.public-rust-evaluation-report/v1")
        self.assertEqual(canonical_json_bytes({"b": 2, "a": 1}), b'{"a":1,"b":2}\n')

    def test_product_commit_must_match_frozen_oracle_baseline(self) -> None:
        baseline = self.oracle["baseline_product_commit"]

        validate_product_commit(baseline, baseline)
        with self.assertRaises(EvaluationError) as raised:
            validate_product_commit("0" * 40, baseline)

        self.assertEqual(raised.exception.code, "evaluation.product_mismatch")

    def test_invalid_cli_is_typed_and_keeps_stdout_empty(self) -> None:
        result = subprocess.run(
            [sys.executable, str(RUNNER)],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

        self.assertEqual(result.returncode, 2)
        self.assertEqual(result.stdout, b"")
        error = json.loads(result.stderr)
        self.assertEqual(error["code"], "evaluation.invalid_arguments")


if __name__ == "__main__":
    unittest.main()
