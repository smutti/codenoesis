import hashlib
import json
import pathlib
import re
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]
SPEC = ROOT / "tests/specifications/s4/r15"
FIXTURE = ROOT / "tests/fixtures/s4/rust-local-flow-v1"
ENTITY_ID = re.compile(r"^urn:codenoesis:entity:blake3:[0-9a-f]{64}$")
RELATIONSHIP_ID = re.compile(r"^urn:codenoesis:relationship:blake3:[0-9a-f]{64}$")
CLAIM_ID = re.compile(r"^urn:codenoesis:claim:blake3:[0-9a-f]{64}$")
EVIDENCE_ID = re.compile(r"^urn:codenoesis:evidence:blake3:[0-9a-f]{64}$")


def load_json(path: pathlib.Path):
    return json.loads(path.read_text(encoding="utf-8"))


def sha256_bytes(value: bytes):
    return hashlib.sha256(value).hexdigest()


def sha256(path: pathlib.Path):
    return sha256_bytes(path.read_bytes())


def family_id_digest(identifiers):
    payload = ("\n".join(sorted(identifiers)) + "\n").encode("utf-8")
    return sha256_bytes(payload)


def git_object_id(kind: str, payload: bytes):
    header = f"{kind} {len(payload)}\0".encode("ascii")
    return hashlib.sha1(header + payload).hexdigest()


class R15ContractTest(unittest.TestCase):
    def test_frozen_contract_family(self):
        expected = {
            "configuration-v14.schema.json": "codenoesis.configuration/v14",
            "repository-snapshot-v17.schema.json": "codenoesis.repository-snapshot/v17",
            "extraction-chunk-v14.schema.json": "codenoesis.extraction-chunk/v14",
            "knowledge-graph-v14.schema.json": "codenoesis.knowledge-graph/v14",
            "codenoesis-error-v22.schema.json": "codenoesis.error/v22",
            "local-query-result-v12.schema.json": "codenoesis.local-query-result/v12",
            "portable-graph-v8.schema.json": "codenoesis.portable-graph/v8",
            "local-explorer-manifest-v8.schema.json": "codenoesis.local-explorer/v8",
        }
        for name, schema_version in expected.items():
            value = load_json(SPEC / name)
            self.assertEqual(
                value["properties"]["schema_version"]["const"],
                schema_version,
                name,
            )

        profile = load_json(SPEC / "local-flow-v1.json")
        self.assertEqual(profile["profile"], "rust-local-flow-v1")
        self.assertEqual(
            profile["contracts"]["pipeline"], "codenoesis.pipeline/s4-r15-v1"
        )
        self.assertEqual(
            profile["contracts"]["extractor"],
            "codenoesis.rust-local-flow/s4-r15-v1",
        )
        self.assertEqual(
            profile["contracts"]["index"], "codenoesis.local-flow-index/v1"
        )
        self.assertEqual(
            profile["contracts"]["rule"], "codenoesis.rule/rust-local-flow/v1"
        )
        self.assertEqual(profile["entity_kinds"], ["rust.syntax_basic_block"])
        self.assertEqual(
            profile["relationship_kinds"],
            [
                "HAS_SYNTAX_BLOCK",
                "CONTAINS_FLOW_NODE",
                "HAS_CONDITION",
                "SYNTAX_NEXT",
                "SYNTAX_TRUE_BRANCH",
                "SYNTAX_FALSE_BRANCH",
                "SYNTAX_REACHES",
                "LEXICAL_MUST_REACHES_READ",
                "LEXICAL_MAY_REACHES_READ",
            ],
        )
        self.assertTrue(profile["closed_callable"]["whole_callable_or_zero_facts"])
        self.assertEqual(profile["closed_callable"]["maximum_nested_branches"], 64)
        self.assertTrue(all(value is False for value in profile["inference"].values()))
        self.assertEqual(
            profile["limits"],
            {
                "blocks_per_callable": 4096,
                "nested_branches": 64,
                "flow_nodes_per_block": 4096,
                "reachability_pairs_per_callable": 262144,
                "total_blocks": 200000,
                "total_relationships": 1000000,
                "derivation_input_references": 1000000,
                "standard_snapshot_bytes": 33554432,
                "optional_snapshot_bytes": 67108864,
            },
        )

        ontology = load_json(SPEC / "rust-ontology-v14.json")
        self.assertEqual(ontology["ontology_version"], "codenoesis.ontology/rust/v14")
        self.assertEqual(
            set(ontology["relationship_kinds"]), set(profile["relationship_kinds"])
        )
        self.assertFalse(ontology["epistemology"]["compiler_cfg"])
        self.assertFalse(ontology["epistemology"]["compiler_data_flow"])
        self.assertFalse(ontology["epistemology"]["runtime"])

        hashes = load_json(SPEC / "semantic-hash-contract-v13.json")
        self.assertEqual(
            hashes["domains"],
            {
                "snapshot": "codenoesis.repository-snapshot.semantic.v17",
                "knowledge_graph": "codenoesis.knowledge-graph.semantic.v14",
                "extraction_chunk": "codenoesis.extraction-chunk.semantic.v14",
                "configuration": "codenoesis.configuration.semantic.v14",
            },
        )

    def test_immutable_fixture_material(self):
        manifest = load_json(FIXTURE / "manifest.json")
        self.assertEqual(
            manifest["repository_identity"],
            "urn:codenoesis:fixture:s4-rust-local-flow-v1",
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
            "author CodeNoesis <fixture@codenoesis.invalid> 1786406400 +0000\n"
            "committer CodeNoesis <fixture@codenoesis.invalid> 1786406400 +0000\n"
            "\n"
            "R15 local-flow fixture\n"
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

    def test_exact_local_flow_oracle(self):
        oracle = load_json(FIXTURE / "expected-local-flow.json")
        self.assertEqual(
            oracle["baseline_counts"],
            {
                "entities": 32,
                "relationships": 49,
                "evidence": 34,
                "claims": 81,
                "diagnostics": 0,
                "coverage": 15,
            },
        )
        self.assertEqual(
            oracle["additive_counts"],
            {
                "entities": 5,
                "relationships": 36,
                "evidence": 5,
                "claims": 41,
                "deterministic_claims": 25,
                "derived_claims": 16,
            },
        )
        self.assertEqual(
            oracle["complete_counts"],
            {
                "entities": 37,
                "relationships": 85,
                "evidence": 39,
                "claims": 122,
                "diagnostics": 0,
                "coverage": 15,
                "deterministic_claims": 101,
                "derived_claims": 21,
            },
        )

        blocks = oracle["blocks"]
        relationships = oracle["relationships"]
        evidence = oracle["evidence"]
        self.assertEqual(len(blocks), 5)
        self.assertEqual(len(relationships), 36)
        self.assertEqual(len(evidence), 5)
        self.assertEqual(
            [(value["start_byte"], value["end_byte"], value["role"]) for value in blocks],
            [
                (78, 100, "entry"),
                (108, 115, "condition"),
                (126, 143, "then_branch"),
                (166, 183, "else_branch"),
                (195, 225, "join"),
            ],
        )
        self.assertEqual(sum(len(value["flow_node_ids"]) for value in blocks), 9)

        entity_ids = [value["id"] for value in blocks]
        relationship_ids = [value["id"] for value in relationships]
        claim_ids = [value["claim_id"] for value in blocks + relationships]
        evidence_ids = [value["id"] for value in evidence]
        self.assertEqual(len(entity_ids), len(set(entity_ids)))
        self.assertEqual(len(relationship_ids), len(set(relationship_ids)))
        self.assertEqual(len(claim_ids), len(set(claim_ids)))
        self.assertEqual(len(evidence_ids), len(set(evidence_ids)))
        self.assertTrue(all(ENTITY_ID.fullmatch(value) for value in entity_ids))
        self.assertTrue(all(RELATIONSHIP_ID.fullmatch(value) for value in relationship_ids))
        self.assertTrue(all(CLAIM_ID.fullmatch(value) for value in claim_ids))
        self.assertTrue(all(EVIDENCE_ID.fullmatch(value) for value in evidence_ids))

        kind_counts = {
            kind: sum(value["kind"] == kind for value in relationships)
            for kind in oracle["relationship_kind_counts"]
        }
        self.assertEqual(kind_counts, oracle["relationship_kind_counts"])
        derived = [value for value in relationships if value["state"] == "derived_fact"]
        deterministic = [
            value for value in relationships if value["state"] == "deterministic_fact"
        ]
        self.assertEqual(len(derived), 16)
        self.assertEqual(len(deterministic) + len(blocks), 25)
        for relationship in derived:
            inputs = relationship["inputs"]
            self.assertEqual(inputs["entity_ids"], sorted(inputs["entity_ids"]))
            self.assertEqual(inputs["relationship_ids"], sorted(inputs["relationship_ids"]))
            self.assertEqual(inputs["evidence_ids"], sorted(inputs["evidence_ids"]))
            self.assertTrue(inputs["entity_ids"])
            self.assertTrue(inputs["relationship_ids"])
            self.assertTrue(inputs["evidence_ids"])
        self.assertTrue(all("inputs" not in value for value in deterministic))

        derivations = [
            {"relationship_id": value["id"], "inputs": value["inputs"]}
            for value in derived
        ]
        derivation_payload = json.dumps(
            sorted(derivations, key=lambda value: value["relationship_id"]),
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=False,
        ).encode("utf-8")
        expected_digests = oracle["family_id_sha256"]
        self.assertEqual(family_id_digest(entity_ids), expected_digests["entities"])
        self.assertEqual(
            family_id_digest(relationship_ids), expected_digests["relationships"]
        )
        self.assertEqual(family_id_digest(claim_ids), expected_digests["claims"])
        self.assertEqual(family_id_digest(evidence_ids), expected_digests["evidence"])
        self.assertEqual(
            sha256_bytes(derivation_payload), expected_digests["derivations"]
        )

    def test_failure_and_lifecycle_are_explicit(self):
        e2e = load_json(SPEC / "e2e_fr_ext_017_rust_local_flow.json")
        self.assertEqual(e2e["status"], "Proposed branch-scoped candidate")
        self.assertEqual(e2e["exact_base"], "011057c84258a26b08b12ced7ae1df478dbb5048")
        self.assertEqual(e2e["expected_red"]["underlying_scan_exit"], 2)
        self.assertEqual(e2e["expected_red"]["stderr_bytes"], 149)
        self.assertEqual(
            e2e["expected_red"]["stderr_sha256"],
            "7f75f7a91f6af0328795f3fbd2729e69756beba2ebd642cc1f6401265662a2fe",
        )
        invalid = load_json(SPEC / "invalid-cases-v1.json")
        case_ids = [value["id"] for value in invalid["cases"]]
        self.assertEqual(len(case_ids), len(set(case_ids)))
        for required in [
            "loop",
            "return",
            "missing_else",
            "cycle",
            "closure_mismatch",
            "derivation_input_mismatch",
            "blocks_per_callable_max_plus_one",
            "portable_condition_text",
            "output_race_replacement",
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
            ROOT / "docs/software/decisions/0027-s4-r15-rust-local-flow-contract.md"
        ).read_text(encoding="utf-8")
        for text in [srs, architecture, roadmap, decision]:
            self.assertIn("rust-local-flow-v1", text)
        self.assertIn("FR-EXT-017", srs)
        self.assertIn("FR-EXP-007", srs)
        self.assertIn("Proposed branch-scoped candidate", decision)
        self.assertNotIn("pending-checkpoint-digest", "\n".join([srs, architecture, roadmap, decision]))

    def test_contract_bundle(self):
        bundle = load_json(SPEC / "contract-bundle.json")
        paths = [record["path"] for record in bundle["files"]]
        self.assertEqual(paths, sorted(paths))
        for record in bundle["files"]:
            self.assertEqual(sha256(ROOT / record["path"]), record["sha256"], record["path"])
        payload = "\n".join(
            f'{record["path"]}\0{record["sha256"]}' for record in bundle["files"]
        ).encode("utf-8")
        self.assertEqual(sha256_bytes(payload), bundle["bundle_sha256"])


if __name__ == "__main__":
    unittest.main()
