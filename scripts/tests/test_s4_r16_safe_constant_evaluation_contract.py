import hashlib
import json
import pathlib
import re
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]
SPEC = ROOT / "tests/specifications/s4/r16"
FIXTURE = ROOT / "tests/fixtures/s4/rust-safe-constant-evaluation-v1"
ENTITY_ID = re.compile(r"^urn:codenoesis:entity:blake3:[0-9a-f]{64}$")
RELATIONSHIP_ID = re.compile(
    r"^urn:codenoesis:relationship:blake3:[0-9a-f]{64}$"
)
CLAIM_ID = re.compile(r"^urn:codenoesis:claim:blake3:[0-9a-f]{64}$")
EVIDENCE_ID = re.compile(r"^urn:codenoesis:evidence:blake3:[0-9a-f]{64}$")
COVERAGE_ID = re.compile(
    r"^urn:codenoesis:coverage-gap:blake3:[0-9a-f]{64}$"
)


def load_json(path: pathlib.Path):
    return json.loads(path.read_text(encoding="utf-8"))


def sha256_bytes(value: bytes):
    return hashlib.sha256(value).hexdigest()


def sha256(path: pathlib.Path):
    return sha256_bytes(path.read_bytes())


def family_id_digest(identifiers):
    payload = ("\n".join(sorted(identifiers)) + "\n").encode("utf-8")
    return sha256_bytes(payload)


def canonical_sha256(value):
    payload = json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    ).encode("utf-8")
    return sha256_bytes(payload)


def git_object_id(kind: str, payload: bytes):
    header = f"{kind} {len(payload)}\0".encode("ascii")
    return hashlib.sha1(header + payload).hexdigest()


class R16ContractTest(unittest.TestCase):
    def test_frozen_contract_family(self):
        expected = {
            "configuration-v15.schema.json": "codenoesis.configuration/v15",
            "repository-snapshot-v18.schema.json": "codenoesis.repository-snapshot/v18",
            "extraction-chunk-v15.schema.json": "codenoesis.extraction-chunk/v15",
            "knowledge-graph-v15.schema.json": "codenoesis.knowledge-graph/v15",
            "codenoesis-error-v24.schema.json": "codenoesis.error/v24",
            "local-query-result-v13.schema.json": "codenoesis.local-query-result/v13",
            "portable-graph-v9.schema.json": "codenoesis.portable-graph/v9",
            "local-explorer-manifest-v9.schema.json": "codenoesis.local-explorer/v9",
        }
        for name, schema_version in expected.items():
            value = load_json(SPEC / name)
            self.assertEqual(
                value["properties"]["schema_version"]["const"],
                schema_version,
                name,
            )

        profile = load_json(SPEC / "safe-constant-evaluation-v1.json")
        self.assertEqual(profile["profile"], "rust-safe-constant-evaluation-v1")
        self.assertEqual(
            profile["requires_profiles"][-1], "rust-local-flow-v1"
        )
        self.assertEqual(
            profile["contracts"],
            {
                "configuration": "codenoesis.configuration/v15",
                "snapshot": "codenoesis.repository-snapshot/v18",
                "extraction": "codenoesis.extraction/v15",
                "chunk": "codenoesis.extraction-chunk/v15",
                "graph": "codenoesis.knowledge-graph/v15",
                "ontology": "codenoesis.ontology/rust/v15",
                "semantic_hash": "codenoesis.semantic-hash-contract/v14",
                "error": "codenoesis.error/v24",
                "query": "codenoesis.local-query-result/v13",
                "portable": "codenoesis.portable-graph/v9",
                "explorer": "codenoesis.local-explorer/v9",
                "pipeline": "codenoesis.pipeline/s4-r16-v1",
                "extractor": "codenoesis.rust-constant-evaluation/s4-r16-v1",
                "index": "codenoesis.constant-evaluation-index/v1",
                "rule": "codenoesis.rule/rust-safe-constant-evaluation/v1",
            },
        )
        self.assertEqual(profile["entity_kinds"], ["rust.evaluated_value"])
        self.assertEqual(profile["relationship_kinds"], ["EVALUATES_TO"])
        self.assertEqual(
            profile["result_types"],
            [
                "bool",
                "i8",
                "i16",
                "i32",
                "i64",
                "i128",
                "u8",
                "u16",
                "u32",
                "u64",
                "u128",
            ],
        )
        self.assertEqual(
            profile["limits"],
            {
                "candidate_declared_values_per_source": 4096,
                "syntax_nodes_per_expression": 256,
                "direct_dependencies_per_subject": 256,
                "dependency_levels": 64,
                "variants_per_enum": 4096,
                "total_evaluated_entities": 200000,
                "total_evaluation_relationships": 200000,
                "evaluation_dependency_references": 400000,
                "derivation_input_references": 1000000,
                "standard_snapshot_bytes": 33554432,
                "optional_snapshot_bytes": 67108864,
                "operational_snapshot_bytes": 268435456,
                "extraction_deadline_ms": 60000,
                "reference_peak_rss_bytes": 4294967296,
            },
        )
        self.assertTrue(all(value is False for value in profile["authority"].values()))

        ontology = load_json(SPEC / "rust-ontology-v15.json")
        self.assertEqual(ontology["ontology_version"], "codenoesis.ontology/rust/v15")
        self.assertEqual(list(ontology["entity_kinds"]), ["rust.evaluated_value"])
        self.assertEqual(list(ontology["relationship_kinds"]), ["EVALUATES_TO"])
        self.assertTrue(ontology["epistemology"]["bounded_constant_evaluation"])
        for forbidden in [
            "inferred_types",
            "target_properties",
            "compiler_validation",
            "compiler_layout",
            "active_cfg",
            "runtime",
            "ownership",
            "side_effects",
        ]:
            self.assertFalse(ontology["epistemology"][forbidden])

        hashes = load_json(SPEC / "semantic-hash-contract-v14.json")
        self.assertEqual(
            hashes["domains"],
            {
                "snapshot": "codenoesis.repository-snapshot.semantic.v18",
                "knowledge_graph": "codenoesis.knowledge-graph.semantic.v15",
                "extraction_chunk": "codenoesis.extraction-chunk.semantic.v15",
                "configuration": "codenoesis.configuration.semantic.v15",
            },
        )

    def test_immutable_fixture_material(self):
        manifest = load_json(FIXTURE / "manifest.json")
        self.assertEqual(
            manifest["repository_identity"],
            "urn:codenoesis:fixture:s4-rust-safe-constant-evaluation-v1",
        )
        self.assertFalse(manifest["external_source_vendored"])
        blobs = {}
        for record in manifest["files"]:
            path = FIXTURE / record["path"]
            payload = path.read_bytes()
            self.assertEqual(len(payload), record["byte_length"], record["path"])
            self.assertEqual(sha256_bytes(payload), record["sha256"], record["path"])
            blob_oid = git_object_id("blob", payload)
            self.assertEqual(blob_oid, record["blob_oid"], record["path"])
            blobs[record["path"]] = bytes.fromhex(blob_oid)

        src_tree_payload = b"100644 lib.rs\0" + blobs["repository/src/lib.rs"]
        src_tree_oid = bytes.fromhex(git_object_id("tree", src_tree_payload))
        root_tree_payload = (
            b"100644 Cargo.toml\0"
            + blobs["repository/Cargo.toml"]
            + b"40000 src\0"
            + src_tree_oid
        )
        tree_oid = git_object_id("tree", root_tree_payload)
        self.assertEqual(tree_oid, manifest["materialization"]["tree_oid"])
        commit_payload = (
            f"tree {tree_oid}\n"
            "author CodeNoesis <fixture@codenoesis.invalid> 1786492800 +0000\n"
            "committer CodeNoesis <fixture@codenoesis.invalid> 1786492800 +0000\n"
            "\n"
            "R16 safe constant evaluation fixture\n"
        ).encode("utf-8")
        self.assertEqual(
            git_object_id("commit", commit_payload),
            manifest["materialization"]["commit_oid"],
        )

        expected = manifest["expected_oracle"]
        oracle_path = ROOT / expected["path"]
        self.assertEqual(oracle_path.stat().st_size, expected["byte_length"])
        self.assertEqual(sha256(oracle_path), expected["sha256"])
        self.assertTrue(all(value is False for value in manifest["sentinels"].values()))

    def test_exact_safe_constant_oracle(self):
        oracle = load_json(FIXTURE / "expected-safe-constant-evaluation.json")
        self.assertEqual(
            oracle["complete_counts"],
            {
                "entities": 42,
                "relationships": 42,
                "claims": 84,
                "evidence": 33,
                "diagnostics": 0,
                "coverage": 32,
                "deterministic_claims": 65,
                "derived_claims": 19,
            },
        )
        self.assertEqual(oracle["canonical_stdout_bytes"], 214974)
        self.assertEqual(
            oracle["expected_hashes"],
            {
                "configuration": "969e2180203946f71aaa3bc9ec71878678595f56b076d67672e4a5a2c53961dd",
                "manifest_chunk": "7e8173ff6b6f63f1f2490d6ec9f6da28af5df61b065a84dc092ce56aa19221bb",
                "source_chunk": "c6ee48b6c3e46b1b9bdb345f7c002b0807f33e6c54b25267f9fba3030330899a",
                "knowledge_graph": "7e3c3dad5a4fda6f8317068c2e5567d861bff26c35dea94310cada98ceb1de13",
                "snapshot": "ad760d2ef7e5807140b1feabd071047494ed17545ffaccf02ebe7302e65a54df",
            },
        )

        values = oracle["evaluated_values"]
        self.assertEqual(len(values), 7)
        self.assertEqual(
            {value["name"]: (value["rust_type"], value["value"]) for value in values},
            {
                "BASE": ("i32", "14"),
                "OFFSET": ("i32", "9"),
                "ENABLED": ("bool", "true"),
                "LIMIT": ("u16", "256"),
                "Mode::Off": ("u8", "0"),
                "Mode::Warm": ("u8", "3"),
                "Mode::Hot": ("u8", "4"),
            },
        )
        for value in values:
            self.assertRegex(value["declared_value_id"], ENTITY_ID)
            self.assertRegex(value["id"], ENTITY_ID)
            self.assertRegex(value["relationship_id"], RELATIONSHIP_ID)
            self.assertRegex(value["declared_value_claim_id"], CLAIM_ID)
            self.assertRegex(value["claim_id"], CLAIM_ID)
            self.assertRegex(value["relationship_claim_id"], CLAIM_ID)
            self.assertTrue(all(EVIDENCE_ID.fullmatch(item) for item in value["evidence_ids"]))
            self.assertEqual(value["evidence_ids"], sorted(value["evidence_ids"]))
            self.assertEqual(value["input_claim_ids"], sorted(value["input_claim_ids"]))
            self.assertEqual(
                value["dependency_entity_ids"], sorted(value["dependency_entity_ids"])
            )
            self.assertIn(
                value["type_authority"],
                ["explicit_primitive_annotation", "fixed_repr_attribute"],
            )

        dependencies = {
            value["name"]: value["dependency_entity_ids"] for value in values
        }
        self.assertEqual(len(dependencies["OFFSET"]), 1)
        self.assertEqual(len(dependencies["Mode::Hot"]), 1)
        self.assertTrue(
            all(not dependency for name, dependency in dependencies.items() if name not in {"OFFSET", "Mode::Hot"})
        )

        entity_ids = [value["id"] for value in values]
        relationship_ids = [value["relationship_id"] for value in values]
        claim_ids = [
            identifier
            for value in values
            for identifier in [value["claim_id"], value["relationship_claim_id"]]
        ]
        expected_digests = oracle["family_id_sha256"]
        self.assertEqual(family_id_digest(entity_ids), expected_digests["entities"])
        self.assertEqual(
            family_id_digest(relationship_ids), expected_digests["relationships"]
        )
        self.assertEqual(family_id_digest(claim_ids), expected_digests["claims"])

        derivations = sorted(
            [
                {
                    "entity_id": value["id"],
                    "relationship_id": value["relationship_id"],
                    "rule_version": "codenoesis.rule/rust-safe-constant-evaluation/v1",
                    "input_claim_ids": value["input_claim_ids"],
                    "input_evidence_ids": value["evidence_ids"],
                    "dependency_entity_ids": value["dependency_entity_ids"],
                }
                for value in values
            ],
            key=lambda value: value["entity_id"],
        )
        self.assertEqual(
            canonical_sha256(derivations), expected_digests["derivations"]
        )
        index = {
            "schema_version": "codenoesis.constant-evaluation-index/v1",
            "rule_version": "codenoesis.rule/rust-safe-constant-evaluation/v1",
            "evaluated_entity_ids": sorted(entity_ids),
            "evaluation_relationship_ids": sorted(relationship_ids),
            "derivations": derivations,
        }
        self.assertEqual(canonical_sha256(index), expected_digests["index"])

        coverage = oracle["new_coverage"]
        self.assertEqual(len(coverage), 3)
        self.assertEqual(
            {value["capability"] for value in coverage},
            {
                "rust.constant_target_dependent",
                "rust.constant_expression_not_evaluated",
                "rust.enum_discriminant_not_evaluated",
            },
        )
        self.assertTrue(all(COVERAGE_ID.fullmatch(value["id"]) for value in coverage))
        self.assertEqual(len(oracle["removed_coverage_ids"]), 6)
        self.assertEqual(len(oracle["removed_diagnostic_ids"]), 1)

    def test_failure_lifecycle_and_pilots_are_explicit(self):
        e2e = load_json(SPEC / "e2e_fr_ext_020_rust_safe_constant_evaluation.json")
        self.assertEqual(e2e["status"], "Proposed branch-scoped candidate")
        self.assertEqual(e2e["exact_base"], "6043313789f6855770520ad5312672fdb081ef38")
        self.assertEqual(e2e["expected_red"]["underlying_scan_exit"], 2)
        self.assertEqual(e2e["expected_red"]["stderr_bytes"], 149)
        self.assertEqual(
            e2e["expected_red"]["stderr_sha256"],
            "7f75f7a91f6af0328795f3fbd2729e69756beba2ebd642cc1f6401265662a2fe",
        )
        self.assertEqual(e2e["pilots"]["lekton"]["runs"], 2)
        self.assertEqual(
            e2e["pilots"]["rustdesk"]["expected"],
            "input.unsupported_rust_constant_evaluation_composition",
        )

        invalid = load_json(SPEC / "invalid-cases-v1.json")
        case_ids = [value["id"] for value in invalid["cases"]]
        self.assertEqual(len(case_ids), len(set(case_ids)))
        for required in [
            "usize_subject",
            "call_expression",
            "dependency_cycle",
            "overflow",
            "enum_partial_failure",
            "missing_input_claim",
            "syntax_nodes_max_plus_one",
            "portable_raw_expression",
            "output_race_replacement",
            "mutable_input",
        ]:
            self.assertIn(required, case_ids)

        srs = (ROOT / "docs/software/software-requirements-specification.md").read_text(
            encoding="utf-8"
        )
        architecture = (ROOT / "docs/software/architecture.md").read_text(
            encoding="utf-8"
        )
        roadmap = (ROOT / "docs/software/roadmap.md").read_text(encoding="utf-8")
        decision = (
            ROOT / "docs/software/decisions/0030-s4-r16-safe-constant-evaluation.md"
        ).read_text(encoding="utf-8")
        readme = (ROOT / "README.md").read_text(encoding="utf-8")
        for text in [srs, architecture, roadmap, decision, readme]:
            self.assertIn("rust-safe-constant-evaluation-v1", text)
        self.assertIn("FR-EXT-020", srs)
        self.assertIn("FR-EXP-008", srs)
        self.assertIn("Proposed branch-scoped candidate", decision)
        self.assertNotIn(
            "pending-checkpoint-digest",
            "\n".join([srs, architecture, roadmap, decision, readme]),
        )

    def test_contract_bundle(self):
        bundle = load_json(SPEC / "contract-bundle.json")
        paths = [record["path"] for record in bundle["files"]]
        self.assertEqual(paths, sorted(paths))
        for record in bundle["files"]:
            self.assertEqual(
                sha256(ROOT / record["path"]), record["sha256"], record["path"]
            )
        payload = "\n".join(
            f'{record["path"]}\0{record["sha256"]}' for record in bundle["files"]
        ).encode("utf-8")
        self.assertEqual(sha256_bytes(payload), bundle["bundle_sha256"])


if __name__ == "__main__":
    unittest.main()
