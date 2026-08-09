import hashlib
import json
import pathlib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]
SPEC = ROOT / "tests/specifications/s4/r11"
FIXTURE = ROOT / "tests/fixtures/s4/rust-callable-boundary-composition-v1"
K1_FIXTURE = ROOT / "tests/fixtures/s4/rust-callable-semantics-v1"


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


class R11ContractTest(unittest.TestCase):
    def test_frozen_contract_family(self):
        expected = {
            "configuration-v10.schema.json": (
                "properties",
                "schema_version",
                "const",
                "codenoesis.configuration/v10",
            ),
            "repository-snapshot-v13.schema.json": (
                "properties",
                "schema_version",
                "const",
                "codenoesis.repository-snapshot/v13",
            ),
            "extraction-chunk-v10.schema.json": (
                "properties",
                "schema_version",
                "const",
                "codenoesis.extraction-chunk/v10",
            ),
            "knowledge-graph-v10.schema.json": (
                "properties",
                "schema_version",
                "const",
                "codenoesis.knowledge-graph/v10",
            ),
            "codenoesis-error-v18.schema.json": (
                "properties",
                "schema_version",
                "const",
                "codenoesis.error/v18",
            ),
            "local-query-result-v8.schema.json": (
                "properties",
                "schema_version",
                "const",
                "codenoesis.local-query-result/v8",
            ),
            "portable-graph-v4.schema.json": (
                "properties",
                "schema_version",
                "const",
                "codenoesis.portable-graph/v4",
            ),
            "local-explorer-manifest-v4.schema.json": (
                "properties",
                "schema_version",
                "const",
                "codenoesis.local-explorer/v4",
            ),
        }
        for name, path in expected.items():
            value = load_json(SPEC / name)
            for key in path[:-1]:
                value = value[key]
            self.assertEqual(value, path[-1], name)

        ontology = load_json(SPEC / "rust-ontology-v10.json")
        self.assertEqual(ontology["ontology_version"], "codenoesis.ontology/rust/v10")
        self.assertFalse(ontology["composition"]["nested_source_traversal"])
        self.assertFalse(ontology["composition"]["boundary_as_code_entity"])

        hashes = load_json(SPEC / "semantic-hash-contract-v9.json")
        self.assertEqual(hashes["schema_version"], "codenoesis.semantic-hash-contract/v9")
        self.assertEqual(
            hashes["domains"],
            {
                "snapshot": "codenoesis.repository-snapshot.semantic.v13",
                "knowledge_graph": "codenoesis.knowledge-graph.semantic.v10",
                "extraction_chunk": "codenoesis.extraction-chunk.semantic.v10",
                "configuration": "codenoesis.configuration.semantic.v10",
            },
        )

    def test_exact_fixture_objects(self):
        manifest = load_json(FIXTURE / "manifest.json")
        self.assertEqual(
            manifest["repository_identity"],
            "urn:codenoesis:fixture:s4-rust-callable-semantics-v1",
        )
        self.assertFalse(manifest["external_source_vendored"])

        source = load_json(K1_FIXTURE / "manifest.json")
        source_blobs = {}
        for record in source["files"]:
            path = K1_FIXTURE / record["path"]
            self.assertEqual(path.stat().st_size, record["byte_length"])
            self.assertEqual(sha256(path), record["sha256"])
            self.assertEqual(git_object_oid("blob", path.read_bytes()), record["git_blob_oid"])
            source_blobs[record["path"].removeprefix("repository/")] = record[
                "git_blob_oid"
            ]

        gitmodules = FIXTURE / "revision-overlay/.gitmodules"
        self.assertEqual(gitmodules.stat().st_size, 116)
        self.assertEqual(
            sha256(gitmodules),
            "1f5b8be2b1398183bb02369231938556f9f02aba714cf1aa134b96bd0cdabab8",
        )
        self.assertEqual(
            git_object_oid("blob", gitmodules.read_bytes()),
            "204d65a4ed6a58dd265349f8a1579a4522dc4f7d",
        )

        src_tree = git_tree_oid(
            [
                ("100644", "lib.rs", source_blobs["src/lib.rs"]),
                ("100644", "model.rs", source_blobs["src/model.rs"]),
            ]
        )
        self.assertEqual(src_tree, "b73aad53ec66942370fbf4a7b4ee4e6161e5ad15")
        external_tree = git_tree_oid(
            [
                (
                    "160000",
                    "nested-model",
                    "6ecf94267842da776e35406a9ebcb85e058a3181",
                )
            ]
        )
        self.assertEqual(external_tree, "3ee7026beba4578478d73788368adc793e975ca0")
        root_tree = git_tree_oid(
            [
                (
                    "100644",
                    ".gitmodules",
                    "204d65a4ed6a58dd265349f8a1579a4522dc4f7d",
                ),
                ("100644", "Cargo.toml", source_blobs["Cargo.toml"]),
                ("100644", "build.rs", source_blobs["build.rs"]),
                ("40000", "external", external_tree),
                ("40000", "src", src_tree),
            ]
        )
        self.assertEqual(root_tree, "289bc8a5abcc2f45fa7e2aa9d787a97305975b71")
        commit = (
            "tree 289bc8a5abcc2f45fa7e2aa9d787a97305975b71\n"
            "author CodeNoesis <fixture@codenoesis.invalid> 1786298400 +0000\n"
            "committer CodeNoesis <fixture@codenoesis.invalid> 1786298400 +0000\n"
            "\n"
            "R11 project-owned callable boundary composition fixture\n"
        ).encode("utf-8")
        self.assertEqual(
            git_object_oid("commit", commit),
            "1c8921919b50f565db49acdbf344cc7e1e864dd1",
        )

    def test_boundary_oracles_and_manifest(self):
        boundary_input = FIXTURE / "boundary-input-matching.json"
        self.assertEqual(boundary_input.stat().st_size, 584)
        self.assertEqual(
            sha256(boundary_input),
            "26a21e2a3847d709d185f49deb86aa05b17a0fa7e954b35cc6172c3b8aa4fee1",
        )
        unbound = load_json(FIXTURE / "expected-boundaries-unbound.json")
        bound = load_json(FIXTURE / "expected-boundaries-bound.json")
        self.assertEqual(unbound["boundaries"][0]["state"], "declared_unbound")
        self.assertIsNone(unbound["boundaries"][0]["nested_repository"])
        self.assertEqual(bound["boundaries"][0]["state"], "explicitly_bound")
        self.assertEqual(
            bound["boundaries"][0]["nested_repository"]["commit_oid"],
            "6ecf94267842da776e35406a9ebcb85e058a3181",
        )
        for family in ("declarations", "evidence"):
            self.assertEqual(unbound[family], bound[family])
        self.assertEqual(
            unbound["coverage_gaps"][0]["code"],
            "boundary.nested_repository_unbound",
        )
        self.assertEqual(
            bound["coverage_gaps"][0]["code"],
            "boundary.nested_repository_not_analyzed",
        )

    def test_traceability_and_bundle(self):
        decision = (
            ROOT
            / "docs/software/decisions/0022-s4-r11-k1-repository-boundary-composition-contract.md"
        ).read_text(encoding="utf-8")
        srs = (ROOT / "docs/software/software-requirements-specification.md").read_text(
            encoding="utf-8"
        )
        roadmap = (ROOT / "docs/software/roadmap.md").read_text(encoding="utf-8")
        for marker in (
            "codenoesis.configuration/v10",
            "codenoesis.repository-snapshot/v13",
            "codenoesis.local-query-result/v8",
            "codenoesis.portable-graph/v4",
            "codenoesis.local-explorer/v4",
            "input.unsupported_rust_callable_composition",
        ):
            self.assertIn(marker, decision)
            self.assertIn(marker, srs)
        self.assertIn("| `R11` | K1 repository-boundary composition |", roadmap)

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
