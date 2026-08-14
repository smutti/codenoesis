import hashlib
import json
import pathlib
import re
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]
SPEC = ROOT / "tests/specifications/s4/r17"
FIXTURE = ROOT / "tests/fixtures/s4/rust-function-context-v1"
DECISION = ROOT / "docs/software/decisions/0032-s4-r17-function-context-navigation.md"
RELEASE_PROFILES = ROOT / "docs/software/release-profiles.md"
ENTITY_ID = re.compile(r"^urn:codenoesis:entity:blake3:[0-9a-f]{64}$")
RELATIONSHIP_ID = re.compile(
    r"^urn:codenoesis:relationship:blake3:[0-9a-f]{64}$"
)
CLAIM_ID = re.compile(r"^urn:codenoesis:claim:blake3:[0-9a-f]{64}$")
EVIDENCE_ID = re.compile(r"^urn:codenoesis:evidence:blake3:[0-9a-f]{64}$")
DIAGNOSTIC_ID = re.compile(
    r"^urn:codenoesis:diagnostic:blake3:[0-9a-f]{64}$"
)
COVERAGE_ID = re.compile(
    r"^urn:codenoesis:coverage-gap:blake3:[0-9a-f]{64}$"
)


def load_json(path: pathlib.Path):
    return json.loads(path.read_text(encoding="utf-8"))


def sha256_bytes(value: bytes):
    return hashlib.sha256(value).hexdigest()


def sha256(path: pathlib.Path):
    return sha256_bytes(path.read_bytes())


def git_object_id(kind: str, payload: bytes):
    header = f"{kind} {len(payload)}\0".encode("ascii")
    return hashlib.sha1(header + payload).hexdigest()


class R17FunctionContextContractTest(unittest.TestCase):
    def test_exact_authority_and_contract_family(self):
        contract = load_json(SPEC / "contract-v1.json")
        self.assertEqual(
            contract["schema_version"],
            "codenoesis.r17-function-context-contract/v1",
        )
        self.assertEqual(contract["issue"], 178)
        self.assertEqual(
            contract["exact_base_sha"],
            "f0d0fc998a9158e7c8e96a5b70c8830a3150dd22",
        )
        self.assertEqual(contract["status"], "proposed_branch_scoped_candidate")
        self.assertEqual(contract["slice"], "S4")
        self.assertEqual(contract["risk"], "high")
        self.assertEqual(contract["requirements"], ["FR-CTX-001", "FR-EXP-009"])
        self.assertEqual(contract["profile"], "rust-function-context-v1")
        self.assertEqual(contract["release_profile"], "local-experimental-r17")
        self.assertEqual(
            contract["contracts"],
            {
                "snapshot": "codenoesis.repository-snapshot/v18",
                "graph": "codenoesis.knowledge-graph/v15",
                "ontology": "codenoesis.ontology/rust/v15",
                "query_default": "codenoesis.local-query-result/v13",
                "context": "codenoesis.function-context/v1",
                "portable": "codenoesis.portable-graph/v9",
                "explorer": "codenoesis.local-explorer/v10",
            },
        )
        self.assertFalse(contract["new_dependency"])
        self.assertFalse(contract["migration"])
        self.assertFalse(contract["control_plane_effect"])
        self.assertFalse(contract["release_effect"])

        context_schema = load_json(SPEC / "function-context-v1.schema.json")
        self.assertEqual(
            context_schema["properties"]["schema_version"]["const"],
            "codenoesis.function-context/v1",
        )
        self.assertEqual(
            context_schema["properties"]["authority"]["const"],
            "declared_source_only",
        )
        self.assertEqual(
            context_schema["properties"]["parameters"]["maxItems"], 256
        )
        self.assertEqual(
            context_schema["properties"]["relationships"]["maxItems"], 512
        )
        self.assertEqual(context_schema["properties"]["claims"]["maxItems"], 2048)
        self.assertEqual(
            context_schema["properties"]["evidence"]["maxItems"], 2048
        )
        self.assertEqual(
            context_schema["properties"]["coverage_gaps"]["maxItems"], 1024
        )

        explorer_schema = load_json(
            SPEC / "local-explorer-manifest-v10.schema.json"
        )
        self.assertEqual(
            explorer_schema["properties"]["schema_version"]["const"],
            "codenoesis.local-explorer/v10",
        )
        self.assertEqual(
            explorer_schema["properties"]["portable_graph"]["properties"]
            ["schema_version"]["const"],
            "codenoesis.portable-graph/v9",
        )
        self.assertEqual(
            explorer_schema["properties"]["limits"]["properties"]
            ["navigation_history"]["const"],
            128,
        )

    def test_immutable_fixture_material(self):
        manifest = load_json(FIXTURE / "manifest.json")
        self.assertEqual(
            manifest["schema_version"], "codenoesis.r17-fixture-manifest/v1"
        )
        self.assertEqual(
            manifest["repository_identity"],
            "urn:codenoesis:fixture:s4-rust-function-context-v1",
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
        materialization = manifest["materialization"]
        self.assertEqual(tree_oid, materialization["tree_oid"])
        commit_payload = (
            f"tree {tree_oid}\n"
            "author CodeNoesis <fixture@codenoesis.invalid> 1786665600 +0000\n"
            "committer CodeNoesis <fixture@codenoesis.invalid> 1786665600 +0000\n"
            "\n"
            "R17 function context fixture\n"
        ).encode("utf-8")
        self.assertEqual(
            git_object_id("commit", commit_payload), materialization["commit_oid"]
        )

        expected = manifest["expected_oracle"]
        oracle_path = ROOT / expected["path"]
        self.assertEqual(oracle_path.stat().st_size, expected["byte_length"])
        self.assertEqual(sha256(oracle_path), expected["sha256"])
        self.assertTrue(all(value is False for value in manifest["sentinels"].values()))

    def test_exact_function_context_oracle(self):
        oracle = load_json(FIXTURE / "expected-function-context.json")
        self.assertEqual(
            oracle["schema_version"], "codenoesis.function-context-oracle/v1"
        )
        self.assertEqual(
            oracle["context_schema_version"], "codenoesis.function-context/v1"
        )
        self.assertEqual(oracle["selector"], "rust-function-context-v1")
        self.assertEqual(oracle["authority"], "declared_source_only")
        self.assertEqual(
            oracle["display_signature"],
            "pub fn scale<T>(&self, value: i32, fallback: T) -> Result<i32, T> where T: Clone,",
        )
        self.assertEqual(
            oracle["counts"],
            {
                "entities": 11,
                "relationships": 9,
                "claims": 20,
                "evidence": 11,
                "diagnostics": 1,
                "coverage_gaps": 7,
                "derivations": 0,
                "navigation": 11,
            },
        )
        self.assertEqual(
            [parameter["ordinal"] for parameter in oracle["parameters"]],
            [0, 1, 2],
        )
        self.assertEqual(
            [parameter["declared_type"] for parameter in oracle["parameters"]],
            [None, "i32", "T"],
        )
        self.assertEqual(oracle["parameters"][0]["receiver_state"], "ref")
        self.assertEqual(oracle["signature"]["return_type"], "Result<i32, T>")
        self.assertEqual(len(oracle["outgoing_calls"]), 1)
        self.assertEqual(oracle["outgoing_calls"][0]["target_name"], "clamp")
        self.assertEqual(oracle["incoming_calls"], [])
        self.assertEqual(
            [fact["ordinal"] for fact in oracle["body_facts"]], [0, 1, 2, 3]
        )
        self.assertEqual(
            oracle["body_facts"][-1]["resolution_state"], "candidate_unresolved"
        )

        id_families = [
            ("entity_ids", ENTITY_ID),
            ("relationship_ids", RELATIONSHIP_ID),
            ("claim_ids", CLAIM_ID),
            ("evidence_ids", EVIDENCE_ID),
            ("diagnostic_ids", DIAGNOSTIC_ID),
            ("coverage_gap_ids", COVERAGE_ID),
        ]
        for family, pattern in id_families:
            identifiers = oracle[family]
            self.assertEqual(identifiers, sorted(identifiers), family)
            self.assertEqual(len(identifiers), len(set(identifiers)), family)
            self.assertTrue(all(pattern.fullmatch(value) for value in identifiers))
        self.assertEqual(oracle["navigation_ids"], oracle["entity_ids"])
        self.assertEqual(oracle["derivation_ids"], [])

        contract = load_json(SPEC / "contract-v1.json")
        self.assertEqual(oracle["limitations"], contract["limitations"])
        self.assertEqual(oracle["limitations"], sorted(oracle["limitations"]))
        self.assertEqual(contract["privacy_denied_fields"], sorted(contract["privacy_denied_fields"]))
        self.assertEqual(contract["forbidden_authority"], sorted(contract["forbidden_authority"]))

    def test_candidate_governance_and_g0_are_complete(self):
        decision = DECISION.read_text(encoding="utf-8")
        self.assertIn("Status: Proposed branch-scoped candidate", decision)
        self.assertIn("Issue: [#178]", decision)
        self.assertIn(
            "Exact base: `f0d0fc998a9158e7c8e96a5b70c8830a3150dd22`",
            decision,
        )
        self.assertIn("FR-CTX-001", decision)
        self.assertIn("FR-EXP-009", decision)
        self.assertIn("rust-function-context-v1", decision)
        self.assertIn("local-experimental-r17", decision)
        self.assertIn("Fifty permutations and ten schedules", decision)
        self.assertNotIn("Status: Accepted", decision)

        release_profiles = RELEASE_PROFILES.read_text(encoding="utf-8")
        self.assertIn("## `local-experimental-r17`", release_profiles)
        self.assertIn("source build only", release_profiles)
        self.assertIn("not Local GA", release_profiles)

        for relative in [
            "README.md",
            "docs/software/architecture.md",
            "docs/software/roadmap.md",
            "docs/software/software-requirements-specification.md",
        ]:
            text = (ROOT / relative).read_text(encoding="utf-8")
            self.assertIn("#178", text, relative)
            self.assertIn("Decision 0032", text, relative)
            self.assertIn("FunctionContextV1", text, relative)
            self.assertNotIn("R17 is Verified", text, relative)

    def test_acceptance_contract_is_red_first_and_closed(self):
        e2e = load_json(SPEC / "e2e_fr_ctx_001_function_context.json")
        self.assertEqual(
            e2e["id"], "e2e_fr_ctx_001_function_context_and_navigation"
        )
        self.assertEqual(e2e["status"], "Proposed branch-scoped candidate")
        self.assertEqual(e2e["exact_base"], "f0d0fc998a9158e7c8e96a5b70c8830a3150dd22")
        self.assertEqual(e2e["expected_red"]["underlying_query_exit"], 2)
        self.assertEqual(e2e["expected_red"]["stdout_bytes"], 0)
        self.assertEqual(
            e2e["journey"],
            [
                "scan",
                "docs",
                "query_context",
                "export_v9",
                "explore_v10",
                "browser_navigation",
            ],
        )
        self.assertEqual(
            e2e["oracle"],
            "tests/fixtures/s4/rust-function-context-v1/expected-function-context.json",
        )
        self.assertTrue(e2e["expected_green"]["selector_absence_byte_identical"])
        self.assertTrue(e2e["expected_green"]["portable_v9_byte_identical"])
        self.assertEqual(
            e2e["failure"],
            {
                "stdout": "empty",
                "publication": "none",
                "repair": False,
                "truncation": False,
                "inference": False,
            },
        )

    def test_contract_bundle(self):
        bundle_path = SPEC / "contract-bundle.json"
        self.assertTrue(bundle_path.exists())
        bundle = load_json(bundle_path)
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
