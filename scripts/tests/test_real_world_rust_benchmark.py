from __future__ import annotations

import json
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CONTRACT = ROOT / "tests/specifications/benchmarks/real-world-rust-stability-v1/contract.json"
MANIFEST = ROOT / "benchmarks/manifest.json"
CORPUS = ROOT / "benchmarks/corpora/real-world-rust-stability-v1.json"
POLICY = ROOT / "benchmarks/policies/real-world-rust-stability-v1.json"
ORACLE = ROOT / "benchmarks/baselines/real-world-rust-stability-v1.json"
RUNNER = ROOT / "scripts/run_real_world_rust_benchmark.py"


class RealWorldRustBenchmarkContractTests(unittest.TestCase):
    def test_active_suite_is_complete(self) -> None:
        contract = json.loads(CONTRACT.read_text(encoding="utf-8"))
        manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
        corpus = json.loads(CORPUS.read_text(encoding="utf-8"))
        policy = json.loads(POLICY.read_text(encoding="utf-8"))
        oracle = json.loads(ORACLE.read_text(encoding="utf-8"))

        self.assertEqual(contract["exact_base"], "3fb6504d1d6cb39f204eca032ff816266194e1ec")
        self.assertEqual(contract["risk"], "high")
        self.assertEqual(manifest["status"], "active")
        self.assertEqual(manifest["requirements"], ["NFR-PER-001"])
        self.assertEqual(len(manifest["suites"]), 1)
        suite = manifest["suites"][0]
        self.assertEqual(suite["id"], contract["suite_id"])
        self.assertEqual(suite["repetitions"], 3)
        self.assertEqual(suite["cache_state"], "mixed")
        self.assertEqual(suite["minimum_success_rate"], 1.0)
        self.assertEqual(corpus["id"], suite["corpus"]["id"])
        self.assertEqual(corpus["version"], suite["corpus"]["version"])
        self.assertEqual([entry["id"] for entry in corpus["entries"]], ["lekton", "rustdesk"])
        self.assertFalse(corpus["network_allowed"])
        self.assertFalse(corpus["source_vendored"])
        self.assertEqual(policy["suite_id"], suite["id"])
        self.assertEqual(policy["requirements"], ["NFR-PER-001"])
        self.assertFalse(policy["nfr_per_002_claimed"])
        self.assertFalse(policy["cross_host_comparison_allowed"])
        self.assertFalse(policy["failed_sample_retry_allowed"])
        self.assertEqual(oracle["suite_id"], suite["id"])
        self.assertEqual(oracle["baseline_product_commit"], contract["exact_base"])
        self.assertEqual(
            oracle["entries"]["lekton"]["semantic_projection_sha256"],
            "ba87c5551fe630bfe9b6d9fcdf9e1ee9b8ca48e3add0b4dae849bf844b8c7700",
        )
        self.assertEqual(
            oracle["entries"]["rustdesk"]["error_code"],
            "input.unsupported_rust_constant_evaluation_composition",
        )
        self.assertTrue(RUNNER.exists(), "expected Red: executable B1 runner is absent")


if __name__ == "__main__":
    unittest.main()
