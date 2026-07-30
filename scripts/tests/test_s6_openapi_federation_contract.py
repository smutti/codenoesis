from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SRS_PATH = ROOT / "docs" / "software" / "software-requirements-specification.md"
DECISION_PATH = (
    ROOT
    / "docs"
    / "software"
    / "decisions"
    / "0009-s6-openapi-federation-contract.md"
)
BUNDLE_PATH = ROOT / "tests" / "specifications" / "s6" / "contract-bundle.json"


class S6OpenApiFederationContractTests(unittest.TestCase):
    def test_s6_governance_package_exists(self) -> None:
        srs = SRS_PATH.read_text(encoding="utf-8")

        self.assertIn("### 2.11 S6 OpenAPI federation ratification register", srs)
        self.assertTrue(DECISION_PATH.is_file(), "S6 decision is missing")
        self.assertTrue(BUNDLE_PATH.is_file(), "S6 contract bundle is missing")


if __name__ == "__main__":
    unittest.main()
