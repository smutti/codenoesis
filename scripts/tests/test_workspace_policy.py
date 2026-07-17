from __future__ import annotations

import json
import re
import subprocess
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


class WorkspacePolicyTests(unittest.TestCase):
    def test_every_workspace_package_inherits_lints_and_rust_version(self) -> None:
        completed = subprocess.run(
            [
                "cargo",
                "metadata",
                "--locked",
                "--no-deps",
                "--format-version",
                "1",
            ],
            cwd=ROOT,
            check=True,
            capture_output=True,
            text=True,
        )
        metadata = json.loads(completed.stdout)

        self.assertGreater(len(metadata["packages"]), 0)
        for package in metadata["packages"]:
            manifest_path = Path(package["manifest_path"])
            manifest = manifest_path.read_text(encoding="utf-8")
            with self.subTest(package=package["name"]):
                lints_section = re.search(
                    r"(?ms)^\[lints\]\s*$\n(.*?)(?=^\[|\Z)", manifest
                )
                self.assertIsNotNone(lints_section, f"{manifest_path} needs [lints]")
                self.assertRegex(
                    lints_section.group(1),  # type: ignore[union-attr]
                    r"(?m)^workspace\s*=\s*true\s*(?:#.*)?$",
                    f"{manifest_path} must inherit workspace lints",
                )
                package_section = re.search(
                    r"(?ms)^\[package\]\s*$\n(.*?)(?=^\[|\Z)", manifest
                )
                self.assertIsNotNone(package_section, f"{manifest_path} needs [package]")
                self.assertRegex(
                    package_section.group(1),  # type: ignore[union-attr]
                    r"(?m)^rust-version\.workspace\s*=\s*true\s*(?:#.*)?$",
                    f"{manifest_path} must inherit the pinned workspace rust-version",
                )
                self.assertEqual(package["rust_version"], "1.97.1")


if __name__ == "__main__":
    unittest.main()
