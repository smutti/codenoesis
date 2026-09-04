from __future__ import annotations

import copy
import json
import shutil
import sys
import tempfile
import unittest
from contextlib import contextmanager
from pathlib import Path
from typing import Iterator


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from validate_benchmark_assets import (  # noqa: E402
    validate_assets,
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

    @contextmanager
    def copied_assets(self) -> Iterator[Path]:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            shutil.copytree(ROOT / "benchmarks", root / "benchmarks")
            (root / "scripts").mkdir()
            shutil.copy2(
                ROOT / "scripts" / "run_real_world_rust_benchmark.py",
                root / "scripts" / "run_real_world_rust_benchmark.py",
            )
            shutil.copy2(
                ROOT / "scripts" / "run_public_rust_evaluation.py",
                root / "scripts" / "run_public_rust_evaluation.py",
            )
            yield root

    @staticmethod
    def write_json(path: Path, value: object) -> None:
        path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")

    def test_committed_active_assets_are_valid(self) -> None:
        errors, manifest = validate_assets(ROOT)

        self.assertEqual(errors, [])
        self.assertEqual(manifest["status"], "active")
        self.assertEqual(validate_schema_data(self.schema), [])

    def test_generic_scaffold_manifest_remains_valid(self) -> None:
        manifest = copy.deepcopy(self.manifest)
        manifest["status"] = "scaffold"
        manifest["suites"] = []

        self.assertEqual(validate_manifest_data(manifest), [])

    def test_active_manifest_requires_a_suite(self) -> None:
        manifest = copy.deepcopy(self.manifest)
        manifest["suites"] = []

        self.assertIn(
            "active status requires at least one benchmark suite",
            validate_manifest_data(manifest),
        )

    def test_suite_requires_reproducibility_fields(self) -> None:
        manifest = copy.deepcopy(self.manifest)
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
        suite = manifest["suites"][0]
        suite["concurrency"] = 0
        suite["cache_state"] = "sometimes"
        suite["runner"] = []

        errors = validate_manifest_data(manifest)

        self.assertTrue(any("concurrency" in error for error in errors))
        self.assertTrue(any("cache_state" in error for error in errors))
        self.assertTrue(any("runner" in error for error in errors))

    def test_active_validator_rejects_runner_path_substitution(self) -> None:
        with self.copied_assets() as root:
            manifest_path = root / "benchmarks" / "manifest.json"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest["suites"][0]["runner"][1] = "scripts/replacement.py"
            self.write_json(manifest_path, manifest)

            errors, _ = validate_assets(root)

        self.assertTrue(any("fixed observational contract" in error for error in errors))

    def test_active_validator_rejects_missing_runner(self) -> None:
        with self.copied_assets() as root:
            (root / "scripts" / "run_real_world_rust_benchmark.py").unlink()

            errors, _ = validate_assets(root)

        self.assertTrue(any("active runner is missing" in error for error in errors))

    def test_active_validator_rejects_missing_conference_runner(self) -> None:
        with self.copied_assets() as root:
            (root / "scripts" / "run_public_rust_evaluation.py").unlink()

            errors, _ = validate_assets(root)

        self.assertTrue(any("conference runner is missing" in error for error in errors))

    def test_active_validator_rejects_corpus_revision_drift(self) -> None:
        with self.copied_assets() as root:
            corpus_path = root / "benchmarks/corpora/real-world-rust-stability-v1.json"
            corpus = json.loads(corpus_path.read_text(encoding="utf-8"))
            corpus["entries"][0]["revision"] = "0" * 40
            self.write_json(corpus_path, corpus)

            errors, _ = validate_assets(root)

        self.assertTrue(any("corpus revisions" in error for error in errors))

    def test_active_validator_rejects_execution_limit_profile_matrix_drift(self) -> None:
        mutations = {
            "missing": lambda profiles: profiles.pop(),
            "duplicate": lambda profiles: profiles.append(profiles[-1]),
            "reordered": lambda profiles: profiles.__setitem__(
                slice(-2, None), reversed(profiles[-2:])
            ),
            "different": lambda profiles: profiles.__setitem__(-1, "unknown"),
        }
        for label, mutate in mutations.items():
            with self.subTest(label=label), self.copied_assets() as root:
                corpus_path = root / "benchmarks/corpora/real-world-rust-stability-v1.json"
                corpus = json.loads(corpus_path.read_text(encoding="utf-8"))
                mutate(corpus["entries"][0]["profiles"])
                self.write_json(corpus_path, corpus)

                errors, _ = validate_assets(root)

            self.assertTrue(any("profile matrix" in error for error in errors))

    def test_active_validator_rejects_threshold_weakening(self) -> None:
        with self.copied_assets() as root:
            policy_path = root / "benchmarks/policies/real-world-rust-stability-v1.json"
            policy = json.loads(policy_path.read_text(encoding="utf-8"))
            policy["candidate_p95_ratio_max"] = 1.21
            self.write_json(policy_path, policy)

            errors, _ = validate_assets(root)

        self.assertTrue(any("bounded observational contract" in error for error in errors))

    def test_active_validator_rejects_semantic_oracle_drift(self) -> None:
        with self.copied_assets() as root:
            oracle_path = root / "benchmarks/baselines/real-world-rust-stability-v1.json"
            oracle = json.loads(oracle_path.read_text(encoding="utf-8"))
            oracle["entries"]["lekton"]["semantic_hash"] = "0" * 64
            self.write_json(oracle_path, oracle)

            errors, _ = validate_assets(root)

        self.assertTrue(any("Lekton semantic oracle changed" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
