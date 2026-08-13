import hashlib
import json
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CONTRACT = (
    ROOT / "tests/specifications/s4/versioned-local-explorer/contract-v1.json"
)
DECISION = ROOT / "docs/software/decisions/0031-s4-versioned-local-explorer-browser.md"


def load_json(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def sha256(path: Path):
    return hashlib.sha256(path.read_bytes().replace(b"\r\n", b"\n")).hexdigest()


class VersionedLocalExplorerContractTest(unittest.TestCase):
    def test_exact_authority_and_version_mapping(self):
        contract = load_json(CONTRACT)
        self.assertEqual(
            contract["schema_version"],
            "codenoesis.versioned-local-explorer-correction/v1",
        )
        self.assertEqual(contract["issue"], 176)
        self.assertEqual(
            contract["exact_base_sha"],
            "16252f59b2dd2302b3f660268843869a45f8ca87",
        )
        self.assertEqual(contract["status"], "proposed_branch_scoped_candidate")
        self.assertEqual(contract["slice"], "S4")
        self.assertEqual(contract["risk"], "high")
        self.assertFalse(contract["dependency_change"])
        self.assertEqual(contract["maximum_portable_graph_bytes"], 268_435_456)
        self.assertEqual(
            [record["portable_schema"] for record in contract["viewers"]],
            [f"codenoesis.portable-graph/v{version}" for version in range(3, 10)],
        )
        self.assertEqual(
            [record["explorer_schema"] for record in contract["viewers"]],
            [f"codenoesis.local-explorer/v{version}" for version in range(3, 10)],
        )

    def test_historical_viewer_assets_are_immutable(self):
        contract = load_json(CONTRACT)
        for record in contract["immutable_assets"]:
            self.assertEqual(sha256(ROOT / record["path"]), record["sha256"])

    def test_candidate_governance_is_complete_and_not_verified(self):
        decision = DECISION.read_text(encoding="utf-8")
        self.assertIn("Status: Proposed branch-scoped correction candidate", decision)
        self.assertIn("Issue: [#176]", decision)
        self.assertIn("Exact base: `16252f59b2dd2302b3f660268843869a45f8ca87`", decision)
        self.assertIn("Fifty materializations per", decision)
        self.assertIn("version must be byte-identical", decision)
        self.assertNotIn("Status: Accepted", decision)
        for relative in [
            "README.md",
            "docs/software/architecture.md",
            "docs/software/roadmap.md",
            "docs/software/software-requirements-specification.md",
        ]:
            text = (ROOT / relative).read_text(encoding="utf-8")
            self.assertIn("#176", text, relative)
            self.assertIn("Decision 0031", text, relative)
            self.assertNotIn("R0-R16 Verified", text, relative)

    def test_security_and_inspection_contract_is_closed(self):
        contract = load_json(CONTRACT)
        self.assertEqual(
            contract["common_array_families"],
            sorted(contract["common_array_families"]),
        )
        self.assertEqual(contract["capabilities"], sorted(contract["capabilities"]))
        self.assertEqual(
            contract["forbidden_authority"],
            sorted(contract["forbidden_authority"]),
        )
        self.assertEqual(
            contract["privacy_denied_fields"],
            sorted(contract["privacy_denied_fields"]),
        )
        self.assertEqual(
            contract["viewers"][-1]["required_sections"],
            ["constant_evaluation_index", "local_flow_index"],
        )


if __name__ == "__main__":
    unittest.main()
