from __future__ import annotations

import copy
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
CORPUS = ROOT / "benchmarks/corpora/public-rust-conference-v1.json"
POLICY = ROOT / "benchmarks/policies/public-rust-conference-v1.json"
ORACLE = ROOT / "benchmarks/baselines/public-rust-conference-v1.json"
RUNNER = ROOT / "scripts/run_public_rust_evaluation.py"
sys.path.insert(0, str(ROOT / "scripts"))

from run_public_rust_evaluation import (  # noqa: E402
    CANDIDATE_GRAPH_FAMILIES,
    CANDIDATE_SEMANTIC_FAMILIES,
    CANDIDATE_STAGE_SCHEMAS,
    EvaluationError,
    REPORT_SCHEMA,
    RUNNER_VERSION,
    STAGES,
    aggregate_stage_coverage,
    aggregate_report,
    build_scan_command,
    canonical_json_bytes,
    graph_metrics,
    evaluate_entry,
    load_contracts,
    nearest_rank,
    parser,
    parse_rejection,
    run_evaluation,
    validate_candidate_sample,
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
        terminal_stages = {entry["terminal_stage"] for entry in entries.values()}
        highest_successful_stages = {
            entry["highest_successful_stage"]
            for entry in entries.values()
            if entry["highest_successful_stage"] is not None
        }
        self.assertEqual(terminal_stages | highest_successful_stages, set(STAGES))
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

    def test_candidate_commit_requires_explicit_observation_opt_in(self) -> None:
        validate_product_commit(
            "0" * 40, self.oracle["baseline_product_commit"], candidate_observation=True
        )
        with self.assertRaises(EvaluationError):
            validate_product_commit(
                "not-a-sha", self.oracle["baseline_product_commit"], candidate_observation=True
            )

    def candidate_sample(self, stage: str, index: int, outcome: str = "success") -> dict:
        sample = {
            "index": index,
            "stage": stage,
            "outcome": outcome,
            "exit_code": 0,
            "wall_time_ns": index * 100,
            "stdout_bytes": 100,
            "stderr_bytes": 0,
            "snapshot_schema": CANDIDATE_STAGE_SCHEMAS[stage],
            "semantic_families": {**CANDIDATE_SEMANTIC_FAMILIES, "extraction_chunks": "list", "knowledge_graph": "dict"},
            "graph_families": {key: "list" for key in CANDIDATE_GRAPH_FAMILIES},
            "semantic_hash": "a" * 64,
            "semantic_projection_sha256": "b" * 64,
            "counts": copy.deepcopy(self.oracle["entries"]["mio"]["counts"]),
            "information": {},
        }
        if outcome == "typed_rejection":
            sample = {key: sample[key] for key in (
                "index", "stage", "outcome", "wall_time_ns"
            )}
            sample.update({
                "exit_code": 11,
                "stdout_bytes": 0,
                "stderr_bytes": 100,
                "error_schema": "codenoesis.error/v24",
                "error_stage": "extraction",
                "error_code": "extraction.expression_contract_invalid",
                "error_context": {},
                "error_stderr_sha256": "c" * 64,
            })
        return sample

    def evaluate_candidate(self, entry_id: str, sample_factory) -> dict:
        descriptor = next(item for item in self.corpus["entries"] if item["id"] == entry_id)
        with mock.patch("run_public_rust_evaluation.run_sample", side_effect=sample_factory):
            return evaluate_entry(
                Path("binary"), Path("repository"), descriptor,
                self.oracle["entries"][entry_id], self.policy, Path("scratch"), Path("home"),
                candidate_observation=True,
            )

    def test_candidate_discovers_progress_beyond_historical_rejection(self) -> None:
        stages = []
        def sample_factory(binary, repository, descriptor, stage, index, *args):
            stages.append((stage, index))
            return self.candidate_sample(stage, index)

        entry = self.evaluate_candidate("dioxus", sample_factory)

        self.assertEqual(stages, [(stage, 1) for stage in STAGES] + [("constant", 2), ("constant", 3)])
        self.assertEqual(entry["terminal_stage"], "constant")
        self.assertEqual(entry["terminal_outcome"], "success")
        self.assertEqual(entry["historical_comparison"], "progressed_review_required")
        self.assertEqual(entry["oracle_match_rate"], 0.0)
        self.assertEqual(len(entry["terminal_samples"]), 3)
        aggregate = aggregate_report({"dioxus": entry}, candidate_observation=True)
        self.assertEqual(aggregate["extraction_success_rate"], 1.0)
        self.assertEqual(aggregate["oracle_match_rate"], 0.0)

    def test_candidate_typed_rejection_remains_failed_extraction_and_redacts_context(self) -> None:
        def sample_factory(binary, repository, descriptor, stage, index, *args):
            sample = self.candidate_sample(
                stage, index, "typed_rejection" if stage == "flow" else "success"
            )
            if sample["outcome"] == "typed_rejection":
                sample["error_context"] = {"path": "/private/secret-canary"}
            return sample

        entry = self.evaluate_candidate("wgpu", sample_factory)
        self.assertEqual(entry["terminal_outcome"], "typed_rejection")
        self.assertEqual(entry["highest_successful_stage"], "framework")
        self.assertNotIn("secret-canary", json.dumps(entry))
        self.assertIn("error_context_sha256", entry["terminal_samples"][0])
        aggregate = aggregate_report({"wgpu": entry}, candidate_observation=True)
        self.assertEqual(aggregate["typed_rejections"], 1)
        self.assertEqual(aggregate["extraction_success_rate"], 0.0)

    def test_candidate_rejects_early_regression(self) -> None:
        def sample_factory(binary, repository, descriptor, stage, index, *args):
            return self.candidate_sample(stage, index, "typed_rejection")
        with self.assertRaisesRegex(EvaluationError, "regressed"):
            self.evaluate_candidate("mio", sample_factory)

    def test_candidate_keeps_product_scan_limit_as_typed_failed_extraction(self) -> None:
        def sample_factory(binary, repository, descriptor, stage, index, *args):
            sample = self.candidate_sample(
                stage, index, "typed_rejection" if stage == "constant" else "success"
            )
            if stage == "constant":
                sample.update({
                    "error_code": "input.repository_limit_exceeded", "error_stage": "input",
                    "error_context": {"limit": "scan_wall_milliseconds", "maximum": 75000, "observed": 75001},
                    "exit_code": 10,
                })
            return sample
        entry = self.evaluate_candidate("dioxus", sample_factory)
        self.assertEqual(entry["terminal_outcome"], "typed_rejection")
        self.assertEqual(entry["highest_successful_stage"], "flow")
        self.assertEqual(aggregate_report({"dioxus": entry}, candidate_observation=True)["extraction_success_rate"], 0.0)

    def test_candidate_rejects_internal_unknown_and_signal_failures(self) -> None:
        for updates in (
            {"error_code": "internal.unexpected", "error_stage": "internal", "exit_code": 1},
            {"error_schema": "codenoesis.error/v999"},
            {"error_code": "arbitrary.failure"},
            {"exit_code": -9},
        ):
            def sample_factory(binary, repository, descriptor, stage, index, *args):
                return {**self.candidate_sample(stage, index, "typed_rejection"), **updates}
            with self.subTest(updates=updates), self.assertRaises(EvaluationError):
                self.evaluate_candidate("dioxus", sample_factory)

    def test_candidate_rejects_semantic_and_error_nondeterminism(self) -> None:
        for rejection in (False, True):
            def sample_factory(binary, repository, descriptor, stage, index, *args):
                sample = self.candidate_sample(
                    stage, index, "typed_rejection" if rejection else "success"
                )
                if index == 2:
                    sample["error_context" if rejection else "semantic_hash"] = (
                        {"changed": True} if rejection else "d" * 64
                    )
                return sample
            with self.subTest(rejection=rejection), self.assertRaisesRegex(EvaluationError, "deterministic"):
                self.evaluate_candidate("dioxus", sample_factory)

    def test_candidate_rejects_changed_error_message_digest(self) -> None:
        def sample_factory(binary, repository, descriptor, stage, index, *args):
            sample = self.candidate_sample(stage, index, "typed_rejection")
            if index == 2:
                sample["error_stderr_sha256"] = "d" * 64
            return sample
        with self.assertRaisesRegex(EvaluationError, "deterministic"):
            self.evaluate_candidate("dioxus", sample_factory)

    def test_candidate_rejects_missing_or_duplicate_terminal_sample(self) -> None:
        for missing in (False, True):
            def sample_factory(binary, repository, descriptor, stage, index, *args):
                if index == 3:
                    return None if missing else self.candidate_sample(stage, 2)
                return self.candidate_sample(stage, index)
            with self.subTest(missing=missing), self.assertRaises(EvaluationError):
                self.evaluate_candidate("dioxus", sample_factory)

    def test_candidate_does_not_count_partial_snapshot_as_r16_success(self) -> None:
        for updates in ({"counts": {}}, {"snapshot_schema": "codenoesis.repository-snapshot/v2"}):
            def sample_factory(binary, repository, descriptor, stage, index, *args):
                sample = self.candidate_sample(stage, index)
                return {**sample, **updates} if stage == "constant" else sample
            with self.subTest(updates=updates), self.assertRaises(EvaluationError):
                self.evaluate_candidate("dioxus", sample_factory)

    def test_candidate_requires_exact_schema_and_mandatory_families_at_every_stage(self) -> None:
        for stage in STAGES:
            mutations = [
                {"snapshot_schema": "codenoesis.repository-snapshot/v1"},
                {"semantic_families": {}},
            ]
            if stage != "acquisition":
                mutations += [{"counts": {}}, {"graph_families": {}}, {"information": None}]
            for mutation in mutations:
                sample = {**self.candidate_sample(stage, 1), **mutation}
                with self.subTest(stage=stage, mutation=mutation), self.assertRaises(EvaluationError):
                    validate_candidate_sample(sample, stage, 1)

    def test_candidate_accepts_exact_propagated_cfg_alternatives_errors(self) -> None:
        for schema in ("codenoesis.error/v21", "codenoesis.error/v22", "codenoesis.error/v24"):
            for code in (
                "extraction.callable_cfg_alternatives_contract_invalid",
                "extraction.callable_cfg_alternatives_unsupported",
            ):
                sample = {**self.candidate_sample("flow", 1, "typed_rejection"),
                          "error_schema": schema, "error_code": code}
                with self.subTest(schema=schema, code=code):
                    validate_candidate_sample(sample, "flow", 1)
                with self.assertRaises(EvaluationError):
                    validate_candidate_sample({**sample, "error_schema": "codenoesis.error/v6"}, "flow", 1)

    def test_rejection_parser_retains_exact_stderr_digest_and_rejects_opaque_output(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            stdout, stderr = root / "stdout", root / "stderr"
            stdout.write_bytes(b"")
            stderr.write_bytes(b"opaque product panic\n")
            with self.assertRaises(EvaluationError):
                parse_rejection(11, stdout, stderr)
            stderr.write_bytes(canonical_json_bytes({
                "schema_version": "codenoesis.error/v24", "code": "extraction.expression_contract_invalid",
                "stage": "extraction", "message": "failed", "retryable": False, "context": {},
            }))
            parsed = parse_rejection(11, stdout, stderr)
            self.assertEqual(len(parsed["error_stderr_sha256"]), 64)

    def test_cli_observation_reports_extraction_separately_and_preserves_oracle(self) -> None:
        original_oracle = ORACLE.read_bytes()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            binary = root / "noesis"
            binary.write_text("unused mock binary", encoding="utf-8")
            binary.chmod(0o755)
            base_arguments = [
                "run", "--manifest", str(ROOT / "benchmarks/manifest.json"),
                "--suite", "rust-public-conference-v1", "--corpus", str(CORPUS),
                "--policy", str(POLICY), "--oracle", str(ORACLE),
                "--binary", str(binary), "--repository-root", str(root),
                "--host-profile", "test-host", "--product-commit", "0" * 40,
                "--output", str(root / "report.json"),
            ]
            with self.assertRaisesRegex(EvaluationError, "frozen oracle baseline"):
                run_evaluation(parser().parse_args(base_arguments))
            self.assertFalse((root / "report.json").exists())
            for override in ("0", "601", "300"):
                with self.subTest(strict_timeout=override), self.assertRaisesRegex(
                    EvaluationError, "timeout override requires candidate observation"
                ):
                    run_evaluation(parser().parse_args(base_arguments + ["--timeout-seconds", override]))
            for override in ("0", "601"):
                with self.subTest(candidate_timeout=override), self.assertRaises(EvaluationError):
                    run_evaluation(parser().parse_args(
                        base_arguments + ["--candidate-observation", "--timeout-seconds", override]
                    ))

            def sample_factory(binary, repository, descriptor, stage, index, *args):
                self.assertEqual(args[1], 300)
                rejection = descriptor["id"] == "wgpu" and stage == "flow"
                return self.candidate_sample(stage, index, "typed_rejection" if rejection else "success")

            with mock.patch(
                "run_public_rust_evaluation.preflight_repository",
                side_effect=lambda repository, *args: (repository, b""),
            ), mock.patch("run_public_rust_evaluation.run_sample", side_effect=sample_factory):
                report = run_evaluation(parser().parse_args(
                    base_arguments + ["--candidate-observation", "--timeout-seconds", "300"]
                ))

            self.assertEqual(report, json.loads((root / "report.json").read_bytes()))
            self.assertEqual(report["schema_version"], "codenoesis.public-rust-candidate-observation-report/v1")
            self.assertEqual(report["result"], "candidate_review_required")
            self.assertEqual(report["product"]["commit"], "0" * 40)
            self.assertEqual(report["historical_baseline_product_commit"], self.oracle["baseline_product_commit"])
            self.assertEqual(report["success_rate"], 7 / 8)
            self.assertEqual(report["extraction_success_rate"], 7 / 8)
            self.assertEqual(report["oracle_match_rate"], 0.0)
            self.assertEqual(report["sample_timeout_seconds"], 300)
            self.assertEqual(report["historical_sample_timeout_seconds"], 150)
            self.assertEqual(self.policy["sample_timeout_seconds"], 150)
            self.assertEqual(report["aggregate"]["typed_rejections"], 1)
            self.assertTrue(all(len(entry["terminal_samples"]) == 3 for entry in report["entries"].values()))
        self.assertEqual(ORACLE.read_bytes(), original_oracle)

    def test_strict_mode_keeps_historical_terminal_and_exact_oracle_checks(self) -> None:
        calls = []
        def sample_factory(binary, repository, descriptor, stage, index, *args):
            calls.append((stage, index))
            expected = self.oracle["entries"][descriptor["id"]]
            return {**self.candidate_sample(stage, index, "typed_rejection"), **expected,
                    "stage": stage, "index": index}

        descriptor = next(item for item in self.corpus["entries"] if item["id"] == "dioxus")
        with mock.patch("run_public_rust_evaluation.run_sample", side_effect=sample_factory):
            entry = evaluate_entry(
                Path("binary"), Path("repository"), descriptor, self.oracle["entries"]["dioxus"],
                self.policy, Path("scratch"), Path("home"),
            )
        self.assertEqual(calls, [("acquisition", index) for index in (1, 2, 3)])
        self.assertEqual(entry["oracle_match_rate"], 1.0)
        self.assertEqual(entry["terminal_outcome"], "typed_rejection")
        with mock.patch("run_public_rust_evaluation.run_sample", side_effect=(
            lambda binary, repository, descriptor, stage, index, *args: self.candidate_sample(stage, index)
        )), self.assertRaisesRegex(EvaluationError, "differs from oracle"):
            evaluate_entry(
                Path("binary"), Path("repository"), descriptor, self.oracle["entries"]["dioxus"],
                self.policy, Path("scratch"), Path("home"),
            )

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
