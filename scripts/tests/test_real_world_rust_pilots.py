from __future__ import annotations

import importlib
import json
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from generate_real_world_rust_pilots import (  # noqa: E402
    PILOTS,
    PilotError,
    build_docs_command,
    build_explore_command,
    build_export_command,
    build_llm_context_command,
    build_scan_command,
    prepare_output_root,
    select_representative_callable,
    snapshot_summary,
)


class RealWorldRustPilotTests(unittest.TestCase):
    def test_runner_is_importable_as_a_scripts_module(self) -> None:
        module = importlib.import_module("scripts.generate_real_world_rust_pilots")
        self.assertEqual(module.RUNNER_VERSION, "codenoesis.real-world-rust-pilot-runner/v3")

    def test_readme_documents_both_explorer_routes(self) -> None:
        readme = (ROOT / "README.md").read_text(encoding="utf-8")
        self.assertIn("scripts/generate_real_world_rust_pilots.py", readme)
        self.assertIn("information-audit.json", readme)
        self.assertIn("llm-context.json", readme)
        self.assertIn("rust-llm-context-v1", readme)
        self.assertIn("/lekton/explorer/", readme)
        self.assertIn("/rustdesk/explorer/", readme)

    def test_pilot_profiles_are_explicit_and_repository_generic(self) -> None:
        lekton, rustdesk = PILOTS
        self.assertEqual(lekton.portable_profile, "rust-safe-constant-evaluation-v1")
        self.assertEqual(lekton.explorer_profile, "rust-function-context-v1")
        self.assertEqual(lekton.llm_context_profile, "rust-llm-context-v1")
        self.assertTrue(lekton.export_documents)
        self.assertIn("rust-safe-constant-evaluation-v1", lekton.scan_options)
        self.assertIn("real-world-rust-benchmark-75s-v1", lekton.scan_options)
        self.assertEqual(rustdesk.portable_profile, "rust-safe-constant-evaluation-v1")
        self.assertEqual(rustdesk.explorer_profile, "rust-function-context-v1")
        self.assertEqual(rustdesk.llm_context_profile, "rust-llm-context-v1")
        self.assertTrue(rustdesk.export_documents)
        self.assertIn("rust-cfg-declaration-alternatives-v1", rustdesk.scan_options)
        self.assertIn("local-gitlinks-v1", rustdesk.scan_options)
        self.assertIn("rust-safe-constant-evaluation-v1", rustdesk.scan_options)
        self.assertIn("real-world-rust-benchmark-75s-v1", rustdesk.scan_options)
        for spec in PILOTS:
            self.assertNotIn(spec.name, " ".join(spec.scan_options))

    def test_commands_cover_scan_docs_export_and_explore(self) -> None:
        binary = Path("/opt/noesis")
        repository = Path("/src/repository")
        root = Path("/out/pilot")
        for spec in PILOTS:
            scan = build_scan_command(binary, repository, root / "store", spec)
            docs = build_docs_command(binary, root / "store", root / "documents", spec)
            export = build_export_command(
                binary,
                root / "store",
                root / "documents",
                root / "portable",
                spec,
            )
            explore = build_explore_command(
                binary,
                root / "portable/portable-graph.json",
                root / "explorer",
                spec,
            )
            llm_context = build_llm_context_command(
                binary,
                root / "store",
                root / "documents",
                "urn:codenoesis:entity:blake3:" + "a" * 64,
                spec,
            )
            self.assertEqual(scan[1], "scan")
            self.assertIn(spec.revision, scan)
            self.assertIn(spec.repository_id, scan)
            self.assertEqual(docs[1], "docs")
            self.assertEqual(export[1], "export")
            self.assertIn(spec.portable_profile, export)
            self.assertEqual("--documents" in export, spec.export_documents)
            self.assertEqual(explore[1], "explore")
            self.assertIn(spec.explorer_profile, explore)
            self.assertEqual(llm_context[1], "query")
            self.assertIn(spec.llm_context_profile, llm_context)

    def test_representative_callable_is_selected_by_stable_id(self) -> None:
        selected = select_representative_callable(
            {
                "entities": [
                    {"id": "z", "kind": "rust.function", "name": "later"},
                    {"id": "a", "kind": "rust.struct", "name": "ignored"},
                    {"id": "b", "kind": "rust.method", "name": "chosen"},
                ],
                "relationships": [],
            }
        )
        self.assertEqual(
            selected,
            {"id": "b", "kind": "rust.method", "name": "chosen"},
        )

    def test_representative_callable_prefers_bounded_body_and_calls(self) -> None:
        selected = select_representative_callable(
            {
                "entities": [
                    {"id": "a", "kind": "rust.method", "name": "empty"},
                    {"id": "z", "kind": "rust.function", "name": "useful"},
                ],
                "relationships": [
                    {"kind": "HAS_SIGNATURE", "source": "z", "target": "signature"},
                    {"kind": "HAS_PARAMETER", "source": "signature", "target": "parameter"},
                    {"kind": "HAS_BODY_FACT", "source": "z", "target": "body"},
                    {"kind": "CALLS", "source": "z", "target": "a"},
                ],
            }
        )
        self.assertEqual(
            selected,
            {"id": "z", "kind": "rust.function", "name": "useful"},
        )

    def test_snapshot_summary_reports_graph_families(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            snapshot = Path(directory) / "snapshot.json"
            snapshot.write_text(
                json.dumps(
                    {
                        "schema_version": "snapshot/v1",
                        "semantic_hash": {"value": "abc"},
                        "semantic": {
                            "ontology_version": "ontology/v1",
                            "knowledge_graph": {
                                "entities": [{"id": "a"}],
                                "relationships": [],
                                "claims": [{"id": "b"}],
                                "evidence": [],
                                "diagnostics": [],
                                "coverage": [{"id": "c"}],
                            },
                        },
                    }
                ),
                encoding="utf-8",
            )
            summary = snapshot_summary(snapshot)

        self.assertEqual(summary["snapshot_schema"], "snapshot/v1")
        self.assertEqual(summary["ontology_version"], "ontology/v1")
        self.assertEqual(
            summary["counts"],
            {
                "entities": 1,
                "relationships": 0,
                "claims": 1,
                "evidence": 0,
                "diagnostics": 0,
                "coverage": 1,
            },
        )

    def test_output_root_must_be_new_and_disjoint(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            repository = root / "repository"
            repository.mkdir()
            output = root / "output"
            prepared = prepare_output_root(output, (repository,))
            self.assertEqual(prepared, output)
            with self.assertRaisesRegex(PilotError, "already exists"):
                prepare_output_root(output, (repository,))
            with self.assertRaisesRegex(PilotError, "overlaps"):
                prepare_output_root(repository / "output", (repository,))


if __name__ == "__main__":
    unittest.main()
