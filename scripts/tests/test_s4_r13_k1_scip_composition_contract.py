import hashlib
import json
import pathlib
import re
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]
SPEC = ROOT / "tests/specifications/s4/r13"
FIXTURE = ROOT / "tests/fixtures/s4/rust-callable-scip-composition-v1"


def load_json(path: pathlib.Path):
    return json.loads(path.read_text(encoding="utf-8"))


def sha256(path: pathlib.Path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


class R13ContractTest(unittest.TestCase):
    def test_frozen_contract_family(self):
        expected = {
            "configuration-v12.schema.json": "codenoesis.configuration/v12",
            "repository-snapshot-v15.schema.json": "codenoesis.repository-snapshot/v15",
            "extraction-chunk-v12.schema.json": "codenoesis.extraction-chunk/v12",
            "knowledge-graph-v12.schema.json": "codenoesis.knowledge-graph/v12",
            "codenoesis-error-v20.schema.json": "codenoesis.error/v20",
            "local-query-result-v10.schema.json": "codenoesis.local-query-result/v10",
            "portable-graph-v6.schema.json": "codenoesis.portable-graph/v6",
            "local-explorer-manifest-v6.schema.json": "codenoesis.local-explorer/v6",
        }
        for name, schema_version in expected.items():
            value = load_json(SPEC / name)
            self.assertEqual(
                value["properties"]["schema_version"]["const"],
                schema_version,
                name,
            )

        ontology = load_json(SPEC / "rust-ontology-v12.json")
        self.assertEqual(ontology["ontology_version"], "codenoesis.ontology/rust/v12")
        self.assertIn("HAS_COMPILER_SYMBOL", ontology["relationship_kinds"])
        self.assertFalse(ontology["composition"]["call_site_resolution_promoted"])
        self.assertFalse(ontology["composition"]["cfg_selection"])
        self.assertFalse(ontology["composition"]["index_generation"])

        hashes = load_json(SPEC / "semantic-hash-contract-v11.json")
        self.assertEqual(hashes["schema_version"], "codenoesis.semantic-hash-contract/v11")
        self.assertEqual(
            hashes["domains"],
            {
                "snapshot": "codenoesis.repository-snapshot.semantic.v15",
                "knowledge_graph": "codenoesis.knowledge-graph.semantic.v12",
                "extraction_chunk": "codenoesis.extraction-chunk.semantic.v12",
                "configuration": "codenoesis.configuration.semantic.v12",
            },
        )

        composition = load_json(SPEC / "callable-scip-composition-v1.json")
        self.assertEqual(composition["join"]["relationship_kind"], "HAS_COMPILER_SYMBOL")
        self.assertEqual(composition["join"]["maximum_joins"], 200_000)
        self.assertEqual(composition["join"]["evidence_per_relationship"], 2)
        self.assertTrue(composition["join"]["identity_union_requires_equal_record"])
        self.assertFalse(composition["inference"]["call_site_promoted"])
        self.assertFalse(composition["inference"]["method_dispatch_inferred"])
        self.assertFalse(composition["inference"]["compiler_index_generated"])

    def test_immutable_fixture_descriptor(self):
        manifest = load_json(FIXTURE / "manifest.json")
        self.assertEqual(
            manifest["repository_identity"],
            "urn:codenoesis:fixture:s4-compiler-index-v1",
        )
        self.assertEqual(
            manifest["materialization"],
            {
                "object_format": "sha1",
                "tree_oid": "d117f2f0924cbef9e7396b97ee46c76bd5261e00",
                "commit_oid": "2203600cce0f0904aefc66dcb49dd0dbc7fd5fd3",
                "commit_recipe": "immutable compiler-index-v1 fixture",
            },
        )
        self.assertFalse(manifest["external_source_vendored"])
        for record in manifest["immutable_inputs"]:
            path = ROOT / record["path"]
            self.assertEqual(path.stat().st_size, record["byte_length"], record["path"])
            self.assertEqual(sha256(path), record["sha256"], record["path"])

        expected_path = ROOT / manifest["expected_composition"]["path"]
        self.assertEqual(
            expected_path.stat().st_size,
            manifest["expected_composition"]["byte_length"],
        )
        self.assertEqual(
            sha256(expected_path),
            manifest["expected_composition"]["sha256"],
        )
        self.assertFalse(manifest["sentinels"]["process_execution_permitted"])
        self.assertFalse(manifest["sentinels"]["index_generation_permitted"])

    def test_exact_cross_layer_oracle(self):
        expected = load_json(FIXTURE / "expected-composition.json")
        self.assertEqual(
            expected["family_counts"],
            {
                "entities": 61,
                "relationships": 53,
                "evidence": 80,
                "claims": 114,
                "diagnostics": 4,
                "coverage": 52,
            },
        )
        self.assertEqual(expected["callable_counts"]["rust.callable_signature"], 5)
        self.assertEqual(expected["callable_counts"]["rust.parameter"], 8)
        self.assertEqual(expected["callable_counts"]["rust.local_binding"], 2)
        self.assertEqual(expected["callable_counts"]["rust.call_site"], 2)
        self.assertEqual(expected["join_relationship_kind"], "HAS_COMPILER_SYMBOL")
        self.assertEqual(len(expected["joins"]), 5)
        self.assertEqual(len(expected["unresolved_call_sites"]), 2)
        self.assertEqual(expected["new_calls_relationships"], 0)

        entity_id = re.compile(r"^urn:codenoesis:entity:blake3:[0-9a-f]{64}$")
        relationship_id = re.compile(r"^urn:codenoesis:relationship:blake3:[0-9a-f]{64}$")
        evidence_id = re.compile(r"^urn:codenoesis:evidence:sha256:[0-9a-f]{64}$")
        callable_ids = set()
        compiler_ids = set()
        join_ids = set()
        for join in expected["joins"]:
            self.assertRegex(join["source_callable_id"], entity_id)
            self.assertRegex(join["signature_id"], entity_id)
            self.assertRegex(join["compiler_symbol_id"], entity_id)
            self.assertRegex(join["relationship_id"], relationship_id)
            self.assertIn(join["source_kind"], ("rust.function", "rust.method"))
            self.assertEqual(len(join["evidence_ids"]), 2)
            for identifier in join["evidence_ids"]:
                self.assertRegex(identifier, evidence_id)
            callable_ids.add(join["source_callable_id"])
            compiler_ids.add(join["compiler_symbol_id"])
            join_ids.add(join["relationship_id"])
        self.assertEqual(len(callable_ids), 5)
        self.assertEqual(len(compiler_ids), 5)
        self.assertEqual(len(join_ids), 5)
        for call_site in expected["unresolved_call_sites"]:
            self.assertEqual(call_site["resolution_state"], "candidate_unresolved")
            self.assertIsNone(call_site["resolved_target_id"])

    def test_oracle_and_traceability(self):
        oracle = load_json(SPEC / "e2e_fr_ext_015_k1_scip_composition.json")
        self.assertEqual(oracle["issue"], 160)
        self.assertEqual(oracle["base_sha"], "cc25dec49343c510f124585c91d459982e827c68")
        self.assertEqual(oracle["acceptance"]["callable_compiler_joins"], 5)
        self.assertEqual(oracle["acceptance"]["unresolved_call_sites"], 2)
        self.assertEqual(oracle["acceptance"]["new_calls"], 0)
        self.assertEqual(oracle["expected_red"]["exit_code"], 11)
        self.assertEqual(
            oracle["expected_red"]["stderr_sha256"],
            "2573e0f364350b300218c6d1940e6eb33f4f0bc70b7ba92dd9b2821f5bf97013",
        )

        decision = (
            ROOT / "docs/software/decisions/0024-s4-r13-k1-scip-composition-contract.md"
        ).read_text(encoding="utf-8")
        srs = (ROOT / "docs/software/software-requirements-specification.md").read_text(
            encoding="utf-8"
        )
        architecture = (ROOT / "docs/software/architecture.md").read_text(encoding="utf-8")
        roadmap = (ROOT / "docs/software/roadmap.md").read_text(encoding="utf-8")
        for marker in (
            "codenoesis.configuration/v12",
            "codenoesis.repository-snapshot/v15",
            "codenoesis.local-query-result/v10",
            "codenoesis.portable-graph/v6",
            "codenoesis.local-explorer/v6",
            "HAS_COMPILER_SYMBOL",
            "input.unsupported_rust_callable_composition",
            "52 coverage records",
        ):
            self.assertIn(marker, decision)
            self.assertIn(marker, srs)
        self.assertIn("R13 callable and revision-bound SCIP composition", architecture)
        self.assertIn("| `R13` | K1 and revision-bound SCIP composition |", roadmap)

    def test_contract_bundle(self):
        bundle = load_json(SPEC / "contract-bundle.json")
        paths = [entry["path"] for entry in bundle["files"]]
        self.assertEqual(paths, sorted(paths, key=lambda value: value.encode("utf-8")))
        self.assertNotIn("docs/software/software-requirements-specification.md", paths)
        self.assertNotIn("tests/specifications/s4/r13/contract-bundle.json", paths)
        for entry in bundle["files"]:
            self.assertEqual(sha256(ROOT / entry["path"]), entry["sha256"])
        canonical = json.dumps(
            {"schema_version": bundle["schema_version"], "files": bundle["files"]},
            ensure_ascii=False,
            separators=(",", ":"),
            sort_keys=True,
        ).encode("utf-8")
        self.assertEqual(hashlib.sha256(canonical).hexdigest(), bundle["bundle_sha256"])
        srs = (ROOT / "docs/software/software-requirements-specification.md").read_text(
            encoding="utf-8"
        )
        self.assertIn(bundle["bundle_sha256"], srs)


if __name__ == "__main__":
    unittest.main()
