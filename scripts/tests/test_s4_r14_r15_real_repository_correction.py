from __future__ import annotations

import hashlib
import json
import re
import unittest
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
SPEC = ROOT / "tests/specifications/s4/r14-r15-real-repository-correction"
FIXTURE = ROOT / "tests/fixtures/s4/rust-real-repository-shapes-v1"
DECISION = (
    ROOT
    / "docs/software/decisions/0029-s4-r14-r15-real-repository-fail-closed-correction.md"
)
ORACLE = FIXTURE / "expected-r14-r15-correction.json"
MANIFEST = FIXTURE / "manifest.json"
CORRECTION = SPEC / "correction-v1.json"
PILOTS = SPEC / "real-repository-pilots-v1.json"

EXACT_BASE = "559fec3863830beef9fb4962d936c681a79c258e"
ISSUE = "https://github.com/smutti/codenoesis/issues/170"
AUTHORIZATION = ISSUE + "#issuecomment-5266803744"
HEX_256 = re.compile(r"^[0-9a-f]{64}$")

IMMUTABLE_FILES = {
    "docs/software/decisions/0019-s4-k1-rust-callable-semantics-contract.md": (
        "bbaef8e1b723333e25aedde61191b100f16f07c481ec0bb30345df4ec20ffff2"
    ),
    "docs/software/decisions/0020-s4-r9-k1-output-capacity-contract.md": (
        "ac812749ee51560a88c1dc0545030ec1daf39ecf519601829c2e11bd4084eb86"
    ),
    "docs/software/decisions/0025-s4-r14-rust-expression-bindings-contract.md": (
        "da0da4a3d9ace0a0e58dee5d747e8c5557250f712040fd52f7c1e57f1fd699ad"
    ),
    "docs/software/decisions/0027-s4-r15-rust-local-flow-contract.md": (
        "95f336df90a00b7899bb180449f11e2397c428c36a839dbcbb96353fe2ce6709"
    ),
    "crates/noesis/assets/s4/k1/index.html": (
        "d0b633b29e6494d6494a35b5553d72c3dd04a747eeef219ca33a9f5fe2a1f4fa"
    ),
}

IMMUTABLE_DIRECTORIES = {
    "tests/fixtures/s4/rust-callable-semantics-v1": (
        7,
        "94f056193149adeccb92fe0a425b2a8f71e29bf986f32572529dc653b910e086",
    ),
    "tests/fixtures/s4/rust-expression-bindings-v1": (
        3,
        "1f5da260ae96bd173ed7d9e1477206d5a6816ac73af30c84c4bbf7077e68005c",
    ),
    "tests/fixtures/s4/rust-local-flow-v1": (
        5,
        "ca63c28533d467c28a87d31f4b3d7f811171828331684c62f0e6c71b10283acf",
    ),
    "tests/specifications/s4/k1": (
        16,
        "efbee8d0f10934c89c7749dbbbdab90475ae3e165b3e176ebb3904dd3ff47a38",
    ),
    "tests/specifications/s4/r14": (
        20,
        "aa922014a47442613a03ac92e0b86effa1360a92574fb8030776f5cb057093c6",
    ),
    "tests/specifications/s4/r15": (
        14,
        "7468cfd3bb1960fcffbce3481b10b9db4a2a4527876e18ddc989a8abd4c9d8dd",
    ),
}


def reject_duplicate_members(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON member: {key}")
        result[key] = value
    return result


def load_json(path: Path) -> Any:
    return json.loads(
        path.read_text(encoding="utf-8"),
        object_pairs_hook=reject_duplicate_members,
    )


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def git_object_id(kind: str, payload: bytes) -> str:
    header = f"{kind} {len(payload)}\0".encode("ascii")
    return hashlib.sha1(header + payload).hexdigest()


def directory_digest(relative: str) -> tuple[int, str]:
    digest = hashlib.sha256()
    paths = sorted(path for path in (ROOT / relative).rglob("*") if path.is_file())
    for path in paths:
        key = path.relative_to(ROOT).as_posix().encode("utf-8")
        digest.update(key + b"\0" + sha256(path).encode("ascii") + b"\n")
    return len(paths), digest.hexdigest()


class R14R15RealRepositoryCorrectionTest(unittest.TestCase):
    def test_governance_authority_and_lifecycle(self):
        correction = load_json(CORRECTION)
        self.assertEqual(correction["status"], "Proposed branch-scoped candidate")
        self.assertEqual(correction["issue"], ISSUE)
        self.assertEqual(correction["authorization"], AUTHORIZATION)
        self.assertEqual(correction["exact_base"], EXACT_BASE)
        self.assertEqual(correction["slice"], "S4")
        self.assertEqual(correction["risk"], "high")
        self.assertEqual(correction["correction_budget"], 5)

        required = {
            "FR-EXT-012",
            "FR-EXT-016",
            "FR-EXT-017",
            "FR-EXP-006",
            "FR-EXP-007",
            "FR-CLI-001",
            "FR-KNW-003",
            "NFR-DET-001",
            "NFR-SEC-001",
            "NFR-SEC-005",
            "NFR-PRV-002",
            "NFR-PER-001",
            "NFR-TST-001",
            "NFR-TST-002",
            "INV-BND-001",
        }
        self.assertEqual(set(correction["requirements"]), required)

        texts = [
            DECISION.read_text(encoding="utf-8"),
            (ROOT / "docs/software/software-requirements-specification.md").read_text(
                encoding="utf-8"
            ),
            (ROOT / "docs/software/architecture.md").read_text(encoding="utf-8"),
            (ROOT / "docs/software/roadmap.md").read_text(encoding="utf-8"),
            (ROOT / "README.md").read_text(encoding="utf-8"),
        ]
        for text in texts:
            self.assertIn("local-snapshot-256m-v1", text)
            self.assertIn("#170", text)
        decision = texts[0]
        self.assertIn(EXACT_BASE, decision)
        self.assertIn("Proposed branch-scoped candidate", decision)
        self.assertIn("No new dependency", decision)
        self.assertNotIn("Verified correction", decision)

    def test_fixture_materialization_is_exact(self):
        manifest = load_json(MANIFEST)
        self.assertEqual(
            manifest["repository_identity"],
            "urn:codenoesis:fixture:s4-rust-real-repository-shapes-v1",
        )
        self.assertFalse(manifest["external_source_vendored"])
        blobs: dict[str, bytes] = {}
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
            "author CodeNoesis <fixture@codenoesis.invalid> 1786492800 +0000\n"
            "committer CodeNoesis <fixture@codenoesis.invalid> 1786492800 +0000\n"
            "\n"
            "R14/R15 real-repository shape fixture\n"
        ).encode("utf-8")
        self.assertEqual(
            git_object_id("commit", commit_payload), materialization["commit_oid"]
        )

        expected = manifest["expected_oracle"]
        self.assertEqual(ORACLE.stat().st_size, expected["byte_length"])
        self.assertEqual(sha256(ORACLE), expected["sha256"])
        self.assertTrue(all(value is False for value in manifest["sentinels"].values()))

        source = (FIXTURE / "repository/src/lib.rs").read_text(encoding="utf-8")
        for shape in [
            "simple_target(value)",
            "consume(value, || value)",
            'factory("https://fixture.invalid").send(value)',
            "client_factory!().send(value)",
            "let chosen = match value",
            "#[cfg(test)]",
        ]:
            self.assertIn(shape, source)

    def test_machine_oracle_freezes_corrected_families(self):
        oracle = load_json(ORACLE)
        self.assertEqual(oracle["exact_base"], EXACT_BASE)
        self.assertEqual(oracle["status"], "Proposed branch-scoped candidate")
        self.assertIsNone(oracle["contracts"]["semantic_output_capacity_profile"])
        self.assertFalse(oracle["contracts"]["schema_change"])
        self.assertEqual(
            oracle["contracts"]["maximum_canonical_output_bytes"], 268435456
        )
        self.assertEqual(oracle["contracts"]["maximum_peak_rss_bytes"], 4294967296)

        self.assertEqual(
            oracle["r14"]["counts"],
            {
                "entities": 83,
                "relationships": 139,
                "claims": 222,
                "evidence": 77,
                "diagnostics": 7,
                "coverage": 31,
            },
        )
        self.assertEqual(
            oracle["r15"]["counts"],
            {
                "entities": 88,
                "relationships": 172,
                "claims": 260,
                "evidence": 82,
                "diagnostics": 7,
                "coverage": 33,
            },
        )
        for profile in ["r14", "r15"]:
            for digest_group in ["family_canonical_sha256", "family_id_sha256"]:
                digests = oracle[profile][digest_group]
                self.assertEqual(
                    set(digests),
                    {
                        "entities",
                        "relationships",
                        "claims",
                        "evidence",
                        "diagnostics",
                        "coverage",
                    },
                )
                self.assertTrue(all(HEX_256.fullmatch(value) for value in digests.values()))

        call_sites = oracle["r14"]["call_sites"]
        self.assertEqual(len(call_sites), 6)
        self.assertEqual(
            sum(value["target_spelling"] == "<unsupported-receiver>.send" for value in call_sites),
            2,
        )
        self.assertTrue(
            all("://" not in value["name"] for value in call_sites),
            "raw source escaped into public call-site name",
        )
        self.assertTrue(
            all("://" not in value["target_spelling"] for value in call_sites),
            "raw source escaped into target spelling",
        )
        self.assertEqual(oracle["r14"]["indexes"]["argument_count"], 5)
        self.assertEqual(oracle["r14"]["indexes"]["binding_count"], 9)
        self.assertEqual(oracle["r14"]["pattern_input_gap"]["capability"], "rust.pattern_input_unexpanded")
        self.assertEqual(len(oracle["r15"]["completed_callable_ids"]), 1)
        self.assertEqual(
            [(value["ordinal"], value["role"]) for value in oracle["r15"]["blocks"]],
            [
                (0, "entry"),
                (1, "condition"),
                (2, "then_branch"),
                (3, "else_branch"),
                (4, "join"),
            ],
        )
        self.assertEqual(oracle["r15"]["retained_derivation_count"], 15)
        self.assertEqual(oracle["determinism"], {
            "input_order_permutations": 50,
            "schedules": 10,
            "expected_relation": "byte_identical",
        })

    def test_selector_failure_and_red_are_exact(self):
        correction = load_json(CORRECTION)
        profile = correction["profiles"]["output_capacity"]
        self.assertEqual(profile["selector"], "local-snapshot-256m-v1")
        self.assertEqual(profile["maximum_bytes_including_lf"], 268435456)
        self.assertIsNone(profile["semantic_configuration_value"])
        self.assertEqual(
            profile["allowed_only_for"],
            ["complete-r14-source-only", "complete-r15-source-only"],
        )
        self.assertEqual(correction["repository_boundary"], {
            "composition": "unsupported",
            "shape": "SubmoduleOrGitlink",
            "r14_code": "input.unsupported_rust_expression_composition",
            "r15_code": "input.unsupported_rust_flow_composition",
            "reason": "repository_boundary_not_supported",
            "exit": 2,
            "stdout_bytes": 0,
            "store_created": False,
            "nested_source_read": False,
        })
        red = correction["expected_red"]
        self.assertEqual(red["r14"]["stderr_sha256"], "9b284f4bb7368bb0d11c5b33725c109ee469845aac00081e51175413adec4e3c")
        self.assertEqual(red["r15"]["stderr_sha256"], "01f7b883892dc357c177c072ce2cca62b3abbf3cbb22f40d173120a283181564")
        self.assertEqual(red["output_profile"]["stderr_sha256"], "e84e29861457502d4d5643259fbd0669ad8ad2dece27a7d8bdd6734a812e819c")
        self.assertEqual(red["r14"]["stdout_bytes"], 0)
        self.assertEqual(red["r15"]["stdout_bytes"], 0)
        self.assertEqual(red["output_profile"]["stdout_bytes"], 0)

    def test_pinned_pilots_are_non_vendored_and_exact(self):
        pilots = load_json(PILOTS)
        self.assertFalse(pilots["network_during_analysis"])
        self.assertFalse(pilots["target_execution"])
        self.assertEqual(len(pilots["pilots"]), 2)
        lekton, rustdesk = pilots["pilots"]
        self.assertEqual(lekton["repository"], "dghilardi/lekton")
        self.assertEqual(lekton["commit_oid"], "247b8f42fb045db41166d70a276a41c2e079b6eb")
        self.assertEqual(lekton["tree_oid"], "55ba428493a4ffae86ba492422a049f46d567a30")
        self.assertEqual(lekton["r14_counts"]["entities"], 25993)
        self.assertEqual(lekton["r15_counts"]["entities"], 26150)
        self.assertEqual(lekton["r15_counts"]["completed_callable_ids"], 147)
        self.assertEqual(lekton["r15_counts"]["retained_derivations"], 160)
        self.assertEqual(lekton["deterministic_fresh_store_runs"], 2)
        self.assertLessEqual(
            lekton["diagnostic_observations_not_equality_oracles"]["r15_scan_peak_rss_bytes"],
            4294967296,
        )
        self.assertLessEqual(
            lekton["diagnostic_observations_not_equality_oracles"]["r15_scan_elapsed_seconds"],
            60,
        )
        self.assertEqual(rustdesk["repository"], "rustdesk/rustdesk")
        self.assertEqual(rustdesk["commit_oid"], "d412d198720aa56f6cfed2dfad262e8fb1322fb7")
        self.assertEqual(rustdesk["tree_oid"], "df8d4c292c9d256a445480eb878e507df3de1dc4")
        self.assertEqual(rustdesk["gitlinks"], 1)
        self.assertEqual(rustdesk["reason"], "repository_boundary_not_supported")

    def test_historical_contracts_and_viewer_are_immutable(self):
        for relative, expected in IMMUTABLE_FILES.items():
            self.assertEqual(sha256(ROOT / relative), expected, relative)
        for relative, expected in IMMUTABLE_DIRECTORIES.items():
            self.assertEqual(directory_digest(relative), expected, relative)

    def test_executable_acceptance_is_present(self):
        support = ROOT / "crates/noesis/tests/support/s4_r14_r15_correction.rs"
        acceptance = (
            ROOT
            / "crates/noesis/tests/e2e_fr_ext_016_017_real_repository_fail_closed.rs"
        )
        self.assertTrue(support.is_file())
        self.assertTrue(acceptance.is_file())
        source = acceptance.read_text(encoding="utf-8")
        for name in [
            "e2e_fr_ext_016_real_repository_shapes_are_fail_closed",
            "e2e_fr_ext_017_real_repository_shapes_are_fail_closed",
            "conf_fr_cli_001_r14_r15_256m_profile_is_explicit",
            "sec_inv_bnd_001_r14_gitlink_is_typed_before_publication",
            "sec_inv_bnd_001_r15_gitlink_is_typed_before_publication",
        ]:
            self.assertIn(name, source)


if __name__ == "__main__":
    unittest.main()
