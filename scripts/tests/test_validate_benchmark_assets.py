from __future__ import annotations

import copy
import json
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from validate_benchmark_assets import (  # noqa: E402
    validate_manifest_data,
    validate_schema_data,
)


class BenchmarkAssetValidationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.manifest = json.loads(
            (ROOT / "benchmarks" / "manifest.json").read_text(encoding="utf-8")
        )
        cls.schema = json.loads(
            (ROOT / "benchmarks" / "manifest.schema.json").read_text(encoding="utf-8")
        )

    @staticmethod
    def valid_suite() -> dict[str, object]:
        return {
            "id": "scan-smoke",
            "description": "Bounded scan smoke benchmark",
            "corpus": {"id": "rust-smoke", "version": "1"},
            "host_profile": "controlled-linux-arm64-v1",
            "concurrency": 1,
            "cache_state": "cold",
            "enabled_extractors": ["rust"],
            "repetitions": 5,
            "percentile_method": "nearest-rank",
            "minimum_success_rate": 1.0,
            "metrics": ["wall_time_ms"],
            "runner": ["cargo", "bench", "--bench", "scan_smoke"],
        }

    def test_committed_scaffold_is_valid(self) -> None:
        self.assertEqual(validate_manifest_data(self.manifest), [])
        self.assertEqual(validate_schema_data(self.schema), [])

    def test_active_manifest_requires_a_suite(self) -> None:
        manifest = copy.deepcopy(self.manifest)
        manifest["status"] = "active"

        self.assertIn(
            "active status requires at least one benchmark suite",
            validate_manifest_data(manifest),
        )

    def test_active_manifest_remains_disabled_until_runner_exists(self) -> None:
        manifest = copy.deepcopy(self.manifest)
        manifest["status"] = "active"
        manifest["suites"] = [self.valid_suite()]

        self.assertIn(
            "active status is disabled until an executable runner validates corpus, samples, and base/head reports",
            validate_manifest_data(manifest),
        )

    def test_suite_requires_reproducibility_fields(self) -> None:
        manifest = copy.deepcopy(self.manifest)
        manifest["status"] = "active"
        manifest["suites"] = [{"id": "scan-smoke"}]

        errors = validate_manifest_data(manifest)

        self.assertTrue(any("host_profile" in error for error in errors))
        self.assertTrue(any("repetitions" in error for error in errors))

    def test_report_contract_cannot_drop_success_rate(self) -> None:
        manifest = copy.deepcopy(self.manifest)
        manifest["report_required_fields"].remove("success_rate")

        self.assertIn(
            "report_required_fields must contain the NFR-PER-001 evidence fields",
            validate_manifest_data(manifest),
        )

    def test_report_contract_rejects_duplicates(self) -> None:
        manifest = copy.deepcopy(self.manifest)
        manifest["report_required_fields"].append("host")

        self.assertIn(
            "report_required_fields must contain the NFR-PER-001 evidence fields",
            validate_manifest_data(manifest),
        )

    def test_schema_status_contract_is_checked(self) -> None:
        schema = copy.deepcopy(self.schema)
        schema["properties"]["status"]["type"] = "integer"

        self.assertIn("schema status contract is invalid", validate_schema_data(schema))

    def test_suite_rejects_invalid_execution_parameters(self) -> None:
        manifest = copy.deepcopy(self.manifest)
        manifest["status"] = "active"
        suite = self.valid_suite()
        suite["concurrency"] = 0
        suite["cache_state"] = "sometimes"
        suite["runner"] = []
        manifest["suites"] = [suite]

        errors = validate_manifest_data(manifest)

        self.assertTrue(any("concurrency" in error for error in errors))
        self.assertTrue(any("cache_state" in error for error in errors))
        self.assertTrue(any("runner" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
