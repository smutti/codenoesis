import hashlib
import json
import pathlib
import re
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]
SPEC = ROOT / "tests/specifications/s4/r14"
FIXTURE = ROOT / "tests/fixtures/s4/rust-expression-bindings-v1"


def load_json(path: pathlib.Path):
    return json.loads(path.read_text(encoding="utf-8"))


def sha256(path: pathlib.Path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


class R14ContractTest(unittest.TestCase):
    def test_frozen_contract_family(self):
        expected = {
            "configuration-v13.schema.json": "codenoesis.configuration/v13",
            "repository-snapshot-v16.schema.json": "codenoesis.repository-snapshot/v16",
            "extraction-chunk-v13.schema.json": "codenoesis.extraction-chunk/v13",
            "knowledge-graph-v13.schema.json": "codenoesis.knowledge-graph/v13",
            "codenoesis-error-v21.schema.json": "codenoesis.error/v21",
            "local-query-result-v11.schema.json": "codenoesis.local-query-result/v11",
            "portable-graph-v7.schema.json": "codenoesis.portable-graph/v7",
            "local-explorer-manifest-v7.schema.json": "codenoesis.local-explorer/v7",
        }
        for name, schema_version in expected.items():
            value = load_json(SPEC / name)
            self.assertEqual(value["properties"]["schema_version"]["const"], schema_version, name)

        profile = load_json(SPEC / "expression-bindings-v1.json")
        self.assertEqual(profile["profile"], "rust-expression-bindings-v1")
        self.assertEqual(profile["contracts"]["pipeline"], "codenoesis.pipeline/s4-r14-v1")
        self.assertEqual(profile["contracts"]["extractor"], "codenoesis.rust-expression-bindings/s4-r14-v1")
        self.assertEqual(profile["contracts"]["index"], "codenoesis.expression-binding-index/v1")
        self.assertEqual(profile["entity_kinds"], ["rust.expression", "rust.call_argument", "rust.pattern_binding"])
        self.assertEqual(len(profile["selected_expression_kinds"]), 27)
        self.assertEqual(profile["relationship_kinds"], [
            "HAS_EXPRESSION", "CONTAINS_EXPRESSION", "HAS_ARGUMENT", "ARGUMENT_VALUE",
            "HAS_RECEIVER", "REPRESENTS_CALL_SITE", "DECLARES_BINDING", "BINDS_FROM",
            "READS", "WRITES",
        ])
        self.assertEqual(profile["access"]["compound_assignment_target"], ["READS", "WRITES"])
        self.assertFalse(profile["access"]["declaration_is_write"])
        self.assertEqual(profile["expression_depth"], {
            "root": 0,
            "definition": "direct_selected_contains_expression_ancestor_count",
            "maximum": 256,
        })
        self.assertTrue(all(value is False for value in profile["inference"].values()))
        self.assertEqual(profile["limits"], {
            "expressions_per_callable": 16384,
            "selected_expression_depth": 256,
            "arguments_per_call": 256,
            "bindings_per_callable": 4096,
            "total_expressions": 400000,
            "total_bindings_and_arguments": 200000,
            "total_relationships": 1000000,
            "normalized_spelling_utf8_bytes": 4096,
            "standard_snapshot_bytes": 33554432,
            "optional_snapshot_bytes": 67108864,
        })

        ontology = load_json(SPEC / "rust-ontology-v13.json")
        self.assertEqual(ontology["ontology_version"], "codenoesis.ontology/rust/v13")
        self.assertEqual(set(ontology["relationship_kinds"]), set(profile["relationship_kinds"]))
        self.assertFalse(ontology["access_epistemology"]["data_flow"])
        self.assertFalse(ontology["access_epistemology"]["runtime"])

        hashes = load_json(SPEC / "semantic-hash-contract-v12.json")
        self.assertEqual(hashes["domains"], {
            "snapshot": "codenoesis.repository-snapshot.semantic.v16",
            "knowledge_graph": "codenoesis.knowledge-graph.semantic.v13",
            "extraction_chunk": "codenoesis.extraction-chunk.semantic.v13",
            "configuration": "codenoesis.configuration.semantic.v13",
        })

    def test_immutable_fixture_descriptor(self):
        manifest = load_json(FIXTURE / "manifest.json")
        self.assertEqual(manifest["repository_identity"], "urn:codenoesis:fixture:s4-rust-callable-semantics-v1")
        self.assertEqual(manifest["materialization"]["tree_oid"], "ead855e0545cc26b351b305fcad39f2e491b285d")
        self.assertEqual(manifest["materialization"]["commit_oid"], "9a7bb3adaa5bf30eef3bc9bc656c81f42fbdb845")
        self.assertFalse(manifest["external_source_vendored"])
        for record in manifest["immutable_inputs"]:
            path = ROOT / record["path"]
            self.assertEqual(path.stat().st_size, record["byte_length"], record["path"])
            self.assertEqual(sha256(path), record["sha256"], record["path"])

        expected = manifest["expected_expression_bindings"]
        path = ROOT / expected["path"]
        self.assertEqual(path.stat().st_size, expected["byte_length"])
        self.assertEqual(sha256(path), expected["sha256"])
        self.assertFalse(manifest["sentinels"]["process_execution_permitted"])
        self.assertFalse(manifest["sentinels"]["network_access_permitted"])
        self.assertFalse(manifest["sentinels"]["macro_expansion_permitted"])

    def test_exact_expression_binding_oracle(self):
        expected = load_json(FIXTURE / "expected-expression-bindings.json")
        self.assertEqual(expected["baseline_counts"], {
            "entities": 91, "relationships": 96, "evidence": 99,
            "claims": 187, "diagnostics": 8, "coverage": 93,
        })
        self.assertEqual(expected["additive_counts"], {
            "entities": 104, "relationships": 207, "evidence": 86,
            "claims": 311, "diagnostics": 0, "coverage": 0,
        })
        self.assertEqual(expected["complete_counts"], {
            "entities": 195, "relationships": 303, "evidence": 185,
            "claims": 498, "diagnostics": 8, "coverage": 93,
        })
        self.assertEqual(expected["expression_kind_counts"], {
            "assignment_expression": 1, "binary_expression": 3, "call_expression": 12,
            "compound_assignment_expr": 6, "field_expression": 4, "identifier": 36,
            "integer_literal": 6, "scoped_identifier": 2, "string_literal": 1,
            "try_expression": 1, "type_cast_expression": 1,
        })
        self.assertEqual(expected["binding_origin_counts"], {
            "parameter": 15, "local_let": 4, "if_let": 1, "while_let": 1,
            "for": 1, "match_arm": 1,
        })
        self.assertEqual(expected["relationship_kind_counts"], {
            "HAS_EXPRESSION": 73, "CONTAINS_EXPRESSION": 38,
            "HAS_ARGUMENT": 8, "ARGUMENT_VALUE": 8, "HAS_RECEIVER": 4,
            "REPRESENTS_CALL_SITE": 9, "DECLARES_BINDING": 23,
            "BINDS_FROM": 8, "READS": 29, "WRITES": 7,
        })
        self.assertEqual(len(expected["expressions"]), 73)
        self.assertEqual(len(expected["arguments"]), 8)
        self.assertEqual(len(expected["bindings"]), 23)
        self.assertEqual(len(expected["relationships"]), 207)
        self.assertEqual(len(expected["new_evidence_ids"]), 86)
        self.assertEqual(sum(binding["properties"]["modifier"] == "explicit_mut" for binding in expected["bindings"]), 2)
        expressions_by_id = {expression["id"]: expression for expression in expected["expressions"]}
        depth_counts = {}
        for expression in expected["expressions"]:
            depth = expression["properties"]["lexical_depth"]
            depth_counts[depth] = depth_counts.get(depth, 0) + 1
            parent_id = expression["properties"]["parent_expression_id"]
            if parent_id is None:
                self.assertEqual(depth, 0)
            else:
                self.assertIn(parent_id, expressions_by_id)
                self.assertEqual(depth, expressions_by_id[parent_id]["properties"]["lexical_depth"] + 1)
        self.assertEqual(depth_counts, {0: 35, 1: 32, 2: 6})

        entity_pattern = re.compile(r"^urn:codenoesis:entity:blake3:[0-9a-f]{64}$")
        relationship_pattern = re.compile(r"^urn:codenoesis:relationship:blake3:[0-9a-f]{64}$")
        evidence_pattern = re.compile(r"^urn:codenoesis:evidence:blake3:[0-9a-f]{64}$")
        entities = expected["expressions"] + expected["arguments"] + expected["bindings"]
        self.assertEqual(len({entity["id"] for entity in entities}), 104)
        self.assertEqual(len({relationship["id"] for relationship in expected["relationships"]}), 207)
        for entity in entities:
            self.assertRegex(entity["id"], entity_pattern)
            self.assertRegex(entity["callable_id"], entity_pattern)
            self.assertRegex(entity["evidence_id"], evidence_pattern)
            self.assertNotIn("text", entity)
            self.assertNotIn("lexeme", entity)
            self.assertLess(entity["locator"]["start_byte"], entity["locator"]["end_byte"])
        for relationship in expected["relationships"]:
            self.assertRegex(relationship["id"], relationship_pattern)
            self.assertIn(relationship["kind"], load_json(SPEC / "expression-bindings-v1.json")["relationship_kinds"])
            self.assertTrue(relationship["evidence_ids"])
        for identifier in expected["new_evidence_ids"]:
            self.assertRegex(identifier, evidence_pattern)

    def test_oracle_traceability_and_invalid_matrix(self):
        oracle = load_json(SPEC / "e2e_fr_ext_016_rust_expression_bindings.json")
        self.assertEqual(oracle["issue"], 162)
        self.assertEqual(oracle["base_sha"], "e32428ecac33df384b2e8b6eed3d257da06e18fe")
        self.assertEqual(oracle["expected_red"]["exit_code"], 2)
        self.assertEqual(oracle["expected_red"]["stderr_bytes"], 149)
        self.assertEqual(oracle["expected_red"]["stderr_sha256"], "7f75f7a91f6af0328795f3fbd2729e69756beba2ebd642cc1f6401265662a2fe")
        self.assertEqual(oracle["acceptance"]["reads"], 29)
        self.assertEqual(oracle["acceptance"]["writes"], 7)

        cases = load_json(SPEC / "invalid-cases-v1.json")["cases"]
        identifiers = {case["id"] for case in cases}
        for required in (
            "unsupported_pattern", "ambiguous_shadowing", "invalid_parent",
            "argument_gap", "forward_access", "call_site_evidence_disagreement",
            "depth_max_plus_one", "portable_expression_text", "output_race_replacement",
        ):
            self.assertIn(required, identifiers)

        decision = (ROOT / "docs/software/decisions/0025-s4-r14-rust-expression-bindings-contract.md").read_text(encoding="utf-8")
        srs = (ROOT / "docs/software/software-requirements-specification.md").read_text(encoding="utf-8")
        architecture = (ROOT / "docs/software/architecture.md").read_text(encoding="utf-8")
        roadmap = (ROOT / "docs/software/roadmap.md").read_text(encoding="utf-8")
        for marker in (
            "codenoesis.configuration/v13", "codenoesis.repository-snapshot/v16",
            "codenoesis.local-query-result/v11", "codenoesis.portable-graph/v7",
            "codenoesis.local-explorer/v7", "rust-expression-bindings-v1",
            "29 `READS`", "7 `WRITES`", "93 coverage records",
        ):
            self.assertIn(marker, decision)
            self.assertIn(marker, srs)
        self.assertIn("R14 Rust expression and lexical bindings", architecture)
        self.assertIn("| `R14` | Rust expression and lexical bindings |", roadmap)

    def test_contract_bundle(self):
        bundle = load_json(SPEC / "contract-bundle.json")
        paths = [entry["path"] for entry in bundle["files"]]
        self.assertEqual(paths, sorted(paths, key=lambda value: value.encode("utf-8")))
        self.assertNotIn("docs/software/software-requirements-specification.md", paths)
        self.assertNotIn("tests/specifications/s4/r14/contract-bundle.json", paths)
        for entry in bundle["files"]:
            self.assertEqual(sha256(ROOT / entry["path"]), entry["sha256"])
        canonical = json.dumps(
            {"schema_version": bundle["schema_version"], "files": bundle["files"]},
            ensure_ascii=False, separators=(",", ":"), sort_keys=True,
        ).encode("utf-8")
        self.assertEqual(hashlib.sha256(canonical).hexdigest(), bundle["bundle_sha256"])
        srs = (ROOT / "docs/software/software-requirements-specification.md").read_text(encoding="utf-8")
        self.assertIn(bundle["bundle_sha256"], srs)


if __name__ == "__main__":
    unittest.main()
