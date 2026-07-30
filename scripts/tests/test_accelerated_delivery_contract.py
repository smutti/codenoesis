from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
AGENTS_PATH = ROOT / "AGENTS.md"


class AcceleratedDeliveryContractTests(unittest.TestCase):
    def test_maintainer_supervised_accelerated_delivery_exists(self) -> None:
        instructions = AGENTS_PATH.read_text(encoding="utf-8")

        self.assertIn(
            "## Maintainer-supervised accelerated delivery",
            instructions,
        )


if __name__ == "__main__":
    unittest.main()
