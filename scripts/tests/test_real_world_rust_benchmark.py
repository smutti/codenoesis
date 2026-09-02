from __future__ import annotations

import copy
import hashlib
import json
import os
import subprocess
import sys
import tempfile
import unittest
from contextlib import contextmanager
from pathlib import Path
from typing import Iterator


ROOT = Path(__file__).resolve().parents[2]
CONTRACT = ROOT / "tests/specifications/benchmarks/real-world-rust-stability-v1/contract.json"
MANIFEST = ROOT / "benchmarks/manifest.json"
CORPUS = ROOT / "benchmarks/corpora/real-world-rust-stability-v1.json"
POLICY = ROOT / "benchmarks/policies/real-world-rust-stability-v1.json"
ORACLE = ROOT / "benchmarks/baselines/real-world-rust-stability-v1.json"
RUNNER = ROOT / "scripts/run_real_world_rust_benchmark.py"
sys.path.insert(0, str(ROOT / "scripts"))

from run_real_world_rust_benchmark import (  # noqa: E402
    BenchmarkError,
    BOOTSTRAP_BASELINE_COMMIT,
    EXPECTED_ORACLE_ENTRIES,
    EXPECTED_PROFILES,
    EXPECTED_SOURCE_IDENTITIES,
    REPORT_SCHEMA,
    RUNNER_VERSION,
    build_scan_command,
    canonical_json_bytes,
    compare_reports,
    ensure_privacy,
    load_json,
    nearest_rank,
    preflight_repository,
    publish_new_file,
    run_sample,
    validate_report,
)


class RealWorldRustBenchmarkContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.policy = json.loads(POLICY.read_text(encoding="utf-8"))
        cls.policy_sha256 = hashlib.sha256(POLICY.read_bytes()).hexdigest()

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
        self.assertTrue(
            all(
                entry["profiles"][-1] == "real-world-rust-benchmark-75s-v1"
                for entry in corpus["entries"]
            )
        )
        self.assertEqual(policy["suite_id"], suite["id"])
        self.assertEqual(policy["requirements"], ["NFR-PER-001"])
        self.assertFalse(policy["nfr_per_002_claimed"])
        self.assertFalse(policy["cross_host_comparison_allowed"])
        self.assertFalse(policy["failed_sample_retry_allowed"])
        self.assertEqual(oracle["suite_id"], suite["id"])
        self.assertEqual(oracle["baseline_product_commit"], BOOTSTRAP_BASELINE_COMMIT)
        self.assertEqual(
            oracle["entries"]["lekton"]["semantic_projection_sha256"],
            "7c800424b3176c96d4ea4164d4066adaf551134b3aea4b40a1e5647f74dc7fa9",
        )
        self.assertEqual(
            oracle["entries"]["rustdesk"]["error_code"],
            "input.unsupported_rust_constant_evaluation_composition",
        )
        self.assertTrue(RUNNER.is_file())

    def test_scan_command_selects_benchmark_only_75s_execution_limit(self) -> None:
        descriptor = {
            "repository_id": "urn:codenoesis:test:b1",
            "revision": "a" * 40,
            "profiles": copy.deepcopy(EXPECTED_PROFILES["lekton"]),
        }

        command = build_scan_command(
            Path("/tmp/noesis"),
            Path("/tmp/repository"),
            Path("/tmp/store"),
            descriptor,
        )

        self.assertIn("--execution-limit-profile", command)
        index = command.index("--execution-limit-profile")
        self.assertEqual(command[index + 1], "real-world-rust-benchmark-75s-v1")

    def test_bootstrap_baseline_and_repository_identity_oracle_are_exact(self) -> None:
        oracle = json.loads(ORACLE.read_text(encoding="utf-8"))
        lekton = oracle["entries"]["lekton"]

        self.assertEqual(
            oracle["baseline_product_commit"],
            "cce84869430ef129f55591998b30ea2ea728e1c3",
        )
        self.assertEqual(
            lekton["semantic_hash"],
            "22e32d20429d510d4674e0e6bdc5542f08dbc0e28874cd0098419e7512a334c1",
        )
        self.assertEqual(
            lekton["semantic_projection_sha256"],
            "7c800424b3176c96d4ea4164d4066adaf551134b3aea4b40a1e5647f74dc7fa9",
        )
        self.assertEqual(EXPECTED_ORACLE_ENTRIES["lekton"], lekton)
        self.assertIn(
            '"cce84869430ef129f55591998b30ea2ea728e1c3"',
            RUNNER.read_text(encoding="utf-8"),
        )

    @staticmethod
    def sample(entry_id: str, index: int, wall_time_ns: int) -> dict[str, object]:
        if entry_id == "lekton":
            return {
                "exit_code": 0,
                "index": index,
                "stderr_bytes": 0,
                "stdout_bytes": 100,
                "wall_time_ns": wall_time_ns,
                "snapshot_schema": EXPECTED_ORACLE_ENTRIES[entry_id]["snapshot_schema"],
                "semantic_hash": EXPECTED_ORACLE_ENTRIES[entry_id]["semantic_hash"],
                "semantic_projection_sha256": EXPECTED_ORACLE_ENTRIES[entry_id][
                    "semantic_projection_sha256"
                ],
                "counts": copy.deepcopy(EXPECTED_ORACLE_ENTRIES[entry_id]["counts"]),
            }
        return {
            "exit_code": 2,
            "index": index,
            "stderr_bytes": 366,
            "stdout_bytes": 0,
            "wall_time_ns": wall_time_ns,
            "error_schema": EXPECTED_ORACLE_ENTRIES[entry_id]["error_schema"],
            "error_code": EXPECTED_ORACLE_ENTRIES[entry_id]["error_code"],
            "error_stage": EXPECTED_ORACLE_ENTRIES[entry_id]["error_stage"],
            "error_reason": EXPECTED_ORACLE_ENTRIES[entry_id]["error_reason"],
            "store_created": False,
            "nested_source_read": False,
        }

    @classmethod
    def report(cls, product_commit: str = BOOTSTRAP_BASELINE_COMMIT) -> dict:
        entries = {}
        for entry_id in ("lekton", "rustdesk"):
            samples = [cls.sample(entry_id, index, index * 1_000_000_000) for index in (1, 2, 3)]
            entries[entry_id] = {
                "outcome": EXPECTED_ORACLE_ENTRIES[entry_id]["outcome"],
                "percentiles_ns": {"p50": 2_000_000_000, "p95": 3_000_000_000, "p99": 3_000_000_000},
                "profiles": copy.deepcopy(EXPECTED_PROFILES[entry_id]),
                "revision": EXPECTED_SOURCE_IDENTITIES[entry_id]["revision"],
                "samples": samples,
                "success_rate": 1.0,
                "tree": EXPECTED_SOURCE_IDENTITIES[entry_id]["tree"],
            }
        return {
            "cache_state": "mixed",
            "concurrency": 1,
            "corpus": {"descriptor_sha256": "a" * 64, "id": "real-world-rust-stability", "version": "1"},
            "corpus_version": "1",
            "enabled_extractors": ["rust-r16-source-only"],
            "entries": entries,
            "host": {
                "architecture": "test-arch",
                "logical_cpu_count": 8,
                "memory_class_bytes": 16_000_000_000,
                "operating_system": "test-os",
                "profile": "reviewed-test-host-v1",
                "rust_toolchain": "1.97.1",
            },
            "manifest_sha256": "b" * 64,
            "minimum_success_rate": 1.0,
            "oracle_sha256": "c" * 64,
            "percentile_method": "nearest-rank",
            "policy_sha256": cls.policy_sha256,
            "product": {
                "binary_sha256": "d" * 64,
                "commit": product_commit,
                "source_id": "codenoesis",
            },
            "repetitions": 3,
            "runner_version": RUNNER_VERSION,
            "schema_version": REPORT_SCHEMA,
            "success_rate": 1.0,
            "suite_id": "rust-real-world-stability-v1",
        }

    @staticmethod
    def set_wall_times(report: dict, entry_id: str, wall_times: list[int]) -> None:
        samples = report["entries"][entry_id]["samples"]
        for sample, wall_time in zip(samples, wall_times):
            sample["wall_time_ns"] = wall_time
        ordered = sorted(wall_times)
        report["entries"][entry_id]["percentiles_ns"] = {
            "p50": ordered[1],
            "p95": ordered[2],
            "p99": ordered[2],
        }

    def test_nearest_rank_uses_unrounded_raw_values(self) -> None:
        values = [31, 11, 21]

        self.assertEqual(nearest_rank(values, 50), 21)
        self.assertEqual(nearest_rank(values, 95), 31)
        self.assertEqual(nearest_rank(values, 99), 31)

    def test_valid_report_and_same_host_comparison_pass(self) -> None:
        baseline = self.report()
        candidate = self.report("e" * 40)

        validate_report(baseline, self.policy, expected_policy_sha256=self.policy_sha256)
        comparison = compare_reports(baseline, candidate, self.policy, self.policy_sha256)

        self.assertEqual(comparison["result"], "pass")
        self.assertTrue(comparison["results"]["lekton"]["semantic_and_outcome_identity"])

    def test_comparator_rejects_deleted_sample(self) -> None:
        candidate = self.report("e" * 40)
        candidate["entries"]["lekton"]["samples"].pop()

        with self.assertRaisesRegex(BenchmarkError, "samples are incomplete"):
            compare_reports(self.report(), candidate, self.policy, self.policy_sha256)

    def test_comparator_rejects_retried_sample_index(self) -> None:
        candidate = self.report("e" * 40)
        candidate["entries"]["lekton"]["samples"][2]["index"] = 2

        with self.assertRaisesRegex(BenchmarkError, "sample indexes"):
            compare_reports(self.report(), candidate, self.policy, self.policy_sha256)

    def test_comparator_rejects_host_mismatch(self) -> None:
        candidate = self.report("e" * 40)
        candidate["host"]["profile"] = "different-reviewed-host"

        with self.assertRaisesRegex(BenchmarkError, "report host differs"):
            compare_reports(self.report(), candidate, self.policy, self.policy_sha256)

    def test_comparator_rejects_semantic_drift(self) -> None:
        candidate = self.report("e" * 40)
        candidate["entries"]["lekton"]["samples"][1]["semantic_hash"] = "0" * 64

        with self.assertRaisesRegex(BenchmarkError, "fixed oracle"):
            compare_reports(self.report(), candidate, self.policy, self.policy_sha256)

    def test_comparator_rejects_typed_outcome_drift(self) -> None:
        candidate = self.report("e" * 40)
        candidate["entries"]["rustdesk"]["samples"][0]["error_code"] = "input.changed"

        with self.assertRaisesRegex(BenchmarkError, "fixed oracle"):
            compare_reports(self.report(), candidate, self.policy, self.policy_sha256)

    def test_comparator_rejects_ratio_threshold_plus_one(self) -> None:
        baseline = self.report()
        candidate = self.report("e" * 40)
        self.set_wall_times(baseline, "lekton", [28_000_000_000, 29_000_000_000, 30_000_000_000])
        self.set_wall_times(candidate, "lekton", [34_000_000_000, 35_000_000_000, 36_000_000_001])

        with self.assertRaisesRegex(BenchmarkError, "candidate p95 exceeds policy"):
            compare_reports(baseline, candidate, self.policy, self.policy_sha256)

    def test_comparator_rejects_absolute_threshold_plus_one(self) -> None:
        baseline = self.report()
        candidate = self.report("e" * 40)
        self.set_wall_times(baseline, "rustdesk", [5_000_000_000, 6_000_000_000, 7_000_000_000])
        self.set_wall_times(candidate, "rustdesk", [8_000_000_000, 9_000_000_000, 10_000_000_001])

        with self.assertRaisesRegex(BenchmarkError, "candidate p95 exceeds policy"):
            compare_reports(baseline, candidate, self.policy, self.policy_sha256)

    def test_report_size_plus_one_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            oversized = Path(temporary) / "report.json"
            oversized.write_bytes(b" " * (self.policy["report_bytes_max"] + 1))

            with self.assertRaisesRegex(BenchmarkError, "byte limit"):
                load_json(oversized, maximum_bytes=self.policy["report_bytes_max"])

    def test_malformed_report_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            malformed = Path(temporary) / "report.json"
            malformed.write_text("{not-json}\n", encoding="utf-8")

            with self.assertRaisesRegex(BenchmarkError, "invalid JSON input"):
                load_json(malformed, maximum_bytes=self.policy["report_bytes_max"])

    def test_privacy_canary_fails_closed(self) -> None:
        encoded = canonical_json_bytes({"host": "private-host-canary"})

        with self.assertRaisesRegex(BenchmarkError, "private host data"):
            ensure_privacy(encoded, {"private-host-canary"})

    def test_atomic_publication_refuses_existing_destination(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "report.json"
            publish_new_file(output, b"{}\n")

            with self.assertRaisesRegex(BenchmarkError, "already exists"):
                publish_new_file(output, b"{}\n")
            self.assertEqual(output.read_bytes(), b"{}\n")

    def test_invalid_cli_is_typed_with_empty_stdout(self) -> None:
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
        self.assertEqual(error["code"], "benchmark.invalid_arguments")

    @contextmanager
    def local_repository(self, *, shallow: bool = False) -> Iterator[tuple[Path, dict, Path]]:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source"
            source.mkdir()
            subprocess.run(["git", "init", "-q", str(source)], check=True)
            (source / "source.rs").write_text("pub fn sample() {}\n", encoding="utf-8")
            subprocess.run(["git", "-C", str(source), "add", "source.rs"], check=True)
            subprocess.run(
                [
                    "git",
                    "-C",
                    str(source),
                    "-c",
                    "user.name=B1 Test",
                    "-c",
                    "user.email=b1@example.invalid",
                    "commit",
                    "-q",
                    "-m",
                    "fixture",
                ],
                check=True,
            )
            repository = source
            repository_url = "https://example.invalid/b1-fixture.git"
            if shallow:
                repository = root / "shallow"
                repository_url = source.as_uri()
                subprocess.run(
                    ["git", "clone", "-q", "--depth", "1", repository_url, str(repository)],
                    check=True,
                )
            else:
                subprocess.run(
                    ["git", "-C", str(repository), "remote", "add", "origin", repository_url],
                    check=True,
                )
            revision = subprocess.check_output(
                ["git", "-C", str(repository), "rev-parse", "HEAD"], text=True
            ).strip()
            tree = subprocess.check_output(
                ["git", "-C", str(repository), "rev-parse", "HEAD^{tree}"], text=True
            ).strip()
            descriptor = {
                "id": "lekton",
                "repository_url": repository_url,
                "revision": revision,
                "tree": tree,
                "repository_id": "urn:codenoesis:test:b1",
                "outcome": "success",
                "profiles": copy.deepcopy(EXPECTED_PROFILES["lekton"]),
            }
            git_home = root / "git-home"
            git_home.mkdir()
            yield repository, descriptor, git_home

    def test_repository_preflight_accepts_exact_clean_full_clone(self) -> None:
        with self.local_repository() as (repository, descriptor, git_home):
            self.assertEqual(preflight_repository(repository, descriptor, git_home), b"")

    def test_repository_preflight_rejects_wrong_commit_and_tree(self) -> None:
        with self.local_repository() as (repository, descriptor, git_home):
            wrong_commit = copy.deepcopy(descriptor)
            wrong_commit["revision"] = "0" * 40
            with self.assertRaisesRegex(BenchmarkError, "HEAD does not match"):
                preflight_repository(repository, wrong_commit, git_home)

            wrong_tree = copy.deepcopy(descriptor)
            wrong_tree["tree"] = "0" * 40
            with self.assertRaisesRegex(BenchmarkError, "tree does not match"):
                preflight_repository(repository, wrong_tree, git_home)

    def test_repository_preflight_rejects_dirty_and_shallow_input(self) -> None:
        with self.local_repository() as (repository, descriptor, git_home):
            (repository / "untracked").write_text("dirty\n", encoding="utf-8")
            with self.assertRaisesRegex(BenchmarkError, "must be clean"):
                preflight_repository(repository, descriptor, git_home)
        with self.local_repository(shallow=True) as (repository, descriptor, git_home):
            with self.assertRaisesRegex(BenchmarkError, "full non-shallow"):
                preflight_repository(repository, descriptor, git_home)

    @unittest.skipUnless(hasattr(os, "symlink"), "symlinks unavailable")
    def test_repository_preflight_rejects_path_substitution(self) -> None:
        with self.local_repository() as (repository, descriptor, git_home):
            substituted = repository.parent / "substituted"
            substituted.symlink_to(repository, target_is_directory=True)
            with self.assertRaisesRegex(BenchmarkError, "not a symlink"):
                preflight_repository(substituted, descriptor, git_home)

    def test_mutating_product_is_rejected_even_when_output_is_invalid(self) -> None:
        with self.local_repository() as (repository, descriptor, _):
            binary = repository.parent / "mutating-product"
            binary.write_text(
                f"#!{sys.executable}\n"
                "import pathlib, sys\n"
                "root = pathlib.Path(sys.argv[sys.argv.index('--repository') + 1])\n"
                "(root / 'mutation-canary').write_text('changed')\n"
                "print('{}')\n",
                encoding="utf-8",
            )
            binary.chmod(0o755)

            with self.assertRaises(BenchmarkError) as raised:
                run_sample(binary, repository, descriptor, EXPECTED_ORACLE_ENTRIES["lekton"], 5, 1)

        self.assertEqual(raised.exception.code, "benchmark.mutable_input")

    def test_timed_out_product_is_not_retried(self) -> None:
        with self.local_repository() as (repository, descriptor, _):
            binary = repository.parent / "sleeping-product"
            binary.write_text(
                f"#!{sys.executable}\n"
                "import time\n"
                "time.sleep(2)\n",
                encoding="utf-8",
            )
            binary.chmod(0o755)

            with self.assertRaises(BenchmarkError) as raised:
                run_sample(binary, repository, descriptor, EXPECTED_ORACLE_ENTRIES["lekton"], 0.01, 1)

        self.assertEqual(raised.exception.code, "benchmark.timeout")


if __name__ == "__main__":
    unittest.main()
