import hashlib
import json
import pathlib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]
SPEC = ROOT / "tests/specifications/s4/r12"
FIXTURE = ROOT / "tests/fixtures/s4/rust-callable-cfg-alternatives-v1"


def load_json(path: pathlib.Path):
    return json.loads(path.read_text(encoding="utf-8"))


def sha256(path: pathlib.Path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def git_object_oid(kind: str, payload: bytes):
    header = f"{kind} {len(payload)}\0".encode("ascii")
    return hashlib.sha1(header + payload).hexdigest()


def git_tree_oid(entries):
    payload = b"".join(
        mode.encode("ascii")
        + b" "
        + name.encode("utf-8")
        + b"\0"
        + bytes.fromhex(oid)
        for mode, name, oid in entries
    )
    return git_object_oid("tree", payload)


class R12ContractTest(unittest.TestCase):
    def test_frozen_contract_family(self):
        expected = {
            "configuration-v11.schema.json": "codenoesis.configuration/v11",
            "repository-snapshot-v14.schema.json": "codenoesis.repository-snapshot/v14",
            "extraction-chunk-v11.schema.json": "codenoesis.extraction-chunk/v11",
            "knowledge-graph-v11.schema.json": "codenoesis.knowledge-graph/v11",
            "codenoesis-error-v19.schema.json": "codenoesis.error/v19",
            "local-query-result-v9.schema.json": "codenoesis.local-query-result/v9",
            "portable-graph-v5.schema.json": "codenoesis.portable-graph/v5",
            "local-explorer-manifest-v5.schema.json": "codenoesis.local-explorer/v5",
        }
        for name, schema_version in expected.items():
            value = load_json(SPEC / name)
            self.assertEqual(
                value["properties"]["schema_version"]["const"],
                schema_version,
                name,
            )

        ontology = load_json(SPEC / "rust-ontology-v11.json")
        self.assertEqual(ontology["ontology_version"], "codenoesis.ontology/rust/v11")
        self.assertFalse(ontology["composition"]["logical_method_direct_callable_shape"])
        self.assertTrue(ontology["composition"]["alternative_is_callable_subject"])
        self.assertFalse(ontology["composition"]["active_cfg_selection"])
        self.assertFalse(ontology["composition"]["nested_source_traversal"])

        hashes = load_json(SPEC / "semantic-hash-contract-v10.json")
        self.assertEqual(hashes["schema_version"], "codenoesis.semantic-hash-contract/v10")
        self.assertEqual(
            hashes["domains"],
            {
                "snapshot": "codenoesis.repository-snapshot.semantic.v14",
                "knowledge_graph": "codenoesis.knowledge-graph.semantic.v11",
                "extraction_chunk": "codenoesis.extraction-chunk.semantic.v11",
                "configuration": "codenoesis.configuration.semantic.v11",
            },
        )

        composition = load_json(SPEC / "callable-cfg-alternatives-composition-v1.json")
        self.assertTrue(composition["subject_mapping"]["alternative_is_callable_subject"])
        self.assertFalse(composition["subject_mapping"]["logical_method_direct_callable_shape"])
        self.assertFalse(composition["inference"]["cfg_evaluated"])
        self.assertFalse(composition["inference"]["method_dispatch_inferred"])

        invalid_cases = {
            case["id"]: case
            for case in load_json(SPEC / "invalid-cases-v1.json")["cases"]
        }
        for case_id in ("missing_framework_profile", "missing_callable_profile"):
            self.assertEqual(
                invalid_cases[case_id],
                {
                    "id": case_id,
                    "error_schema": "codenoesis.error/v17",
                    "expected": "input.unsupported_rust_cfg_alternatives_composition",
                    "dispatch": "legacy_r10",
                },
            )

        selector_dispatch = load_json(
            SPEC / "e2e_fr_ext_014_k1_cfg_alternatives.json"
        )["selector_error_dispatch"]
        self.assertEqual(
            selector_dispatch["complete_r10_r6_k1_intent"],
            "codenoesis.error/v19",
        )

    def test_exact_fixture_objects_and_spans(self):
        manifest = load_json(FIXTURE / "manifest.json")
        self.assertEqual(
            manifest["repository_identity"],
            "urn:codenoesis:fixture:s4-rust-callable-semantics-v1",
        )
        self.assertFalse(manifest["external_source_vendored"])

        blobs = {}
        for record in manifest["files"]:
            path = FIXTURE / record["path"]
            self.assertEqual(path.stat().st_size, record["byte_length"])
            self.assertEqual(sha256(path), record["sha256"])
            self.assertEqual(git_object_oid("blob", path.read_bytes()), record["git_blob_oid"])
            blobs[record["path"].removeprefix("repository/")] = record["git_blob_oid"]

        src_tree = git_tree_oid(
            [
                ("100644", "lib.rs", blobs["src/lib.rs"]),
                ("100644", "model.rs", blobs["src/model.rs"]),
            ]
        )
        self.assertEqual(src_tree, "c18f9f1d153699ccc04fff90dea61ac3bdd5837f")
        root_tree = git_tree_oid(
            [
                ("100644", "Cargo.toml", blobs["Cargo.toml"]),
                ("100644", "build.rs", blobs["build.rs"]),
                ("40000", "src", src_tree),
            ]
        )
        self.assertEqual(root_tree, "c43f0c6e91c8e3e27abbba94cdd666d6c3598414")
        commit = (
            "tree c43f0c6e91c8e3e27abbba94cdd666d6c3598414\n"
            "author CodeNoesis <fixture@codenoesis.invalid> 1786341600 +0000\n"
            "committer CodeNoesis <fixture@codenoesis.invalid> 1786341600 +0000\n"
            "\n"
            "R12 project-owned callable cfg alternatives fixture\n"
        ).encode("utf-8")
        self.assertEqual(
            git_object_oid("commit", commit),
            "637091858d6582fbe7f0c75b7c62d4fd9c2d87ca",
        )

        model = (FIXTURE / "repository/src/model.rs").read_bytes()
        spans = manifest["reviewed_spans"]
        self.assertEqual(model[slice(*spans["unix_cfg"])], b'#[cfg(target_family = "unix")]')
        self.assertEqual(model[slice(*spans["windows_cfg"])], b'#[cfg(target_family = "windows")]')
        self.assertTrue(model[slice(*spans["unix_method"])].startswith(b"pub fn run"))
        self.assertTrue(model[slice(*spans["windows_method"])].startswith(b"pub fn run"))

        expected_path = FIXTURE / manifest["expected_facts"]["path"]
        self.assertEqual(expected_path.stat().st_size, manifest["expected_facts"]["byte_length"])
        self.assertEqual(sha256(expected_path), manifest["expected_facts"]["sha256"])

    def test_exact_cross_layer_oracle(self):
        expected = load_json(FIXTURE / "expected-composition.json")
        logical = expected["logical_method"]
        alternatives = expected["alternatives"]
        self.assertFalse(logical["direct_callable_shape"])
        self.assertEqual(
            logical["declaration_alternative_ids"],
            [alternative["id"] for alternative in alternatives],
        )
        self.assertEqual(len(alternatives), 2)
        self.assertEqual(
            expected["callable_entity_counts"],
            {
                "rust.callable_signature": 10,
                "rust.parameter": 17,
                "rust.declared_value": 10,
                "rust.local_binding": 4,
                "rust.call_site": 10,
                "rust.control": 11,
            },
        )
        self.assertEqual(
            expected["callable_relationship_counts"],
            {
                "HAS_SIGNATURE": 10,
                "HAS_PARAMETER": 17,
                "DECLARES_VALUE": 10,
                "HAS_BODY_FACT": 25,
                "CALLS": 5,
            },
        )
        self.assertEqual(expected["unresolved_call_sites"], 5)
        for alternative in alternatives:
            self.assertEqual(len(alternative["parameter_ids"]), 2)
            self.assertRegex(alternative["callable_signature_id"], r"^urn:codenoesis:entity:blake3:[0-9a-f]{64}$")
            self.assertRegex(alternative["calls_relationship_id"], r"^urn:codenoesis:relationship:blake3:[0-9a-f]{64}$")

    def test_traceability_and_bundle(self):
        decision = (
            ROOT
            / "docs/software/decisions/0023-s4-r12-k1-cfg-alternatives-composition-contract.md"
        ).read_text(encoding="utf-8")
        srs = (ROOT / "docs/software/software-requirements-specification.md").read_text(
            encoding="utf-8"
        )
        architecture = (ROOT / "docs/software/architecture.md").read_text(encoding="utf-8")
        roadmap = (ROOT / "docs/software/roadmap.md").read_text(encoding="utf-8")
        for marker in (
            "codenoesis.configuration/v11",
            "codenoesis.repository-snapshot/v14",
            "codenoesis.local-query-result/v9",
            "codenoesis.portable-graph/v5",
            "codenoesis.local-explorer/v5",
            "input.unsupported_rust_cfg_alternatives_composition",
        ):
            self.assertIn(marker, decision)
            self.assertIn(marker, srs)
        self.assertIn("alternative ID as the K1 callable subject", architecture)
        self.assertIn("| `R12` | K1 cfg-alternatives composition |", roadmap)

        bundle = load_json(SPEC / "contract-bundle.json")
        paths = [entry["path"] for entry in bundle["files"]]
        self.assertEqual(paths, sorted(paths, key=lambda value: value.encode("utf-8")))
        for entry in bundle["files"]:
            self.assertEqual(sha256(ROOT / entry["path"]), entry["sha256"])
        canonical = json.dumps(
            {"schema_version": bundle["schema_version"], "files": bundle["files"]},
            ensure_ascii=False,
            separators=(",", ":"),
            sort_keys=True,
        ).encode("utf-8")
        self.assertEqual(hashlib.sha256(canonical).hexdigest(), bundle["bundle_sha256"])
        self.assertIn(bundle["bundle_sha256"], srs)


if __name__ == "__main__":
    unittest.main()
