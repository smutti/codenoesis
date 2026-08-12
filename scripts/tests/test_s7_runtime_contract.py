import hashlib
import json
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
RUNTIME_ROOT = ROOT / "tests" / "specifications" / "s7" / "runtime-v1"

FROZEN_SHA256 = {
    "docs/software/decisions/0007-s7-implementation-aware-api-compatibility-contract.md": "3858fcee11d6a5d0da39d832d2a6163c8aec237e362687207109658f4bfb8b9d",
    "tests/specifications/s7/semantic-compatibility-report-v1.schema.json": "3f8b6847ef688fc8ec5e3b8b5ec888efff1498bf7a6a2ef46890fa90238d3b79",
    "tests/specifications/s7/compatibility-rule-catalog-v1.json": "30cdddc1dbe986a8b2bf5dd76158a05a5beb959456f8500422af2a5bad6ae5c0",
    "tests/specifications/s7/e2e_fr_imp_004_implementation_aware_api_diff.json": "d17072db81a456d8ca94b64094f5f09c9fc99004d27458fc8237e911d7386af4",
    "tests/specifications/s7/contract-bundle.json": "8185cbdc0eb33dc96cb5c932a1343460b7d1794b953a64190ff4fcc6783b0208",
    "tests/fixtures/s7/implementation-aware-api-v1/manifest.json": "958948f2eb078ef53e1aebac5f9e7543411cb3ba7423e1ef2dc6c0e7ef2f69c0",
    "tests/fixtures/s7/implementation-aware-api-v1/expected-semantic-compatibility-report.json": "cfd9a8d4dcb2d04bcd9eaffd15f1ae947ffdaba80e07daee43375c9a67c15750",
}


def load_json(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


class S7RuntimeContractTests(unittest.TestCase):
    def test_frozen_s7_artifacts_are_immutable(self):
        for relative_path, expected in FROZEN_SHA256.items():
            payload = (ROOT / relative_path).read_bytes()
            self.assertEqual(hashlib.sha256(payload).hexdigest(), expected, relative_path)

    def test_runtime_contract_fixes_exact_package(self):
        contract = load_json(RUNTIME_ROOT / "runtime-contract-v1.json")
        self.assertEqual(contract["base_commit_sha"], "c212db5062ce28731f4aadc528a750d4ba33524f")
        self.assertEqual(contract["slice"], "S7")
        self.assertEqual(contract["risk"], "high")
        self.assertEqual(
            contract["requirement_status"]["approved"],
            ["DR-SEM-001", "FR-IMP-004", "FR-IMP-005"],
        )
        self.assertEqual(
            contract["requirement_status"]["proposed_branch_scoped"],
            ["FR-CLI-006", "FR-EXT-018", "FR-EXT-019"],
        )
        self.assertEqual(contract["dependency"]["name"], "tree-sitter-kotlin-ng")
        self.assertEqual(contract["dependency"]["requirement"], "=1.1.0")
        self.assertFalse(contract["dependency"]["other_new_dependencies_allowed"])
        self.assertEqual(contract["immutable_oracle"]["bytes"], 14991)
        self.assertEqual(
            contract["immutable_oracle"]["sha256"],
            "cfd9a8d4dcb2d04bcd9eaffd15f1ae947ffdaba80e07daee43375c9a67c15750",
        )
        self.assertEqual(contract["correction_budget"], 5)

    def test_workspace_and_error_schemas_are_closed(self):
        workspace = load_json(RUNTIME_ROOT / "impact-workspace-v1.schema.json")
        error = load_json(RUNTIME_ROOT / "codenoesis-error-v23.schema.json")
        self.assertFalse(workspace["additionalProperties"])
        self.assertEqual(
            workspace["properties"]["schema_version"]["const"],
            "codenoesis.impact-workspace/v1",
        )
        self.assertEqual(
            workspace["properties"]["analysis_profile"]["const"],
            "implementation-aware-http-json/v1",
        )
        self.assertEqual(
            workspace["properties"]["pipeline"]["const"],
            "codenoesis.pipeline/s7-v1",
        )
        self.assertFalse(error["additionalProperties"])
        self.assertEqual(
            error["properties"]["schema_version"]["const"],
            "codenoesis.error/v23",
        )
        self.assertEqual(
            error["properties"]["code"]["enum"],
            sorted(error["properties"]["code"]["enum"]),
        )

    def test_capability_matrix_is_closed_and_honest(self):
        matrix = load_json(RUNTIME_ROOT / "source-capability-matrix-v1.json")
        self.assertEqual(matrix["provider"]["profile"], "rust-direct-json-map/v1")
        self.assertEqual(matrix["client"]["profile"], "kotlin-direct-json-access/v1")
        self.assertIn("custom_mapping_helper", matrix["provider"]["unsupported"])
        self.assertIn("compiler_cfg", matrix["provider"]["unsupported"])
        self.assertIn("type_only_behavior", matrix["client"]["unsupported"])
        self.assertIn("gradle_or_compiler_execution", matrix["client"]["unsupported"])

    def test_threat_model_covers_authority_privacy_and_resources(self):
        model = load_json(RUNTIME_ROOT / "threat-model-v1.json")
        ids = [item["id"] for item in model["threats"]]
        self.assertEqual(ids, sorted(ids))
        self.assertEqual(len(ids), len(set(ids)))
        controls = {item["control"] for item in model["threats"]}
        self.assertIn("stable_handle_and_sha256_revalidation", controls)
        self.assertIn("metadata_and_excerpt_digest_only_public_report", controls)
        self.assertIn("no_network_child_build_target_plugin_model_or_environment_authority", controls)

    def test_documentation_binds_candidate_without_approving_it(self):
        decision = (ROOT / "docs/software/decisions/0028-s7-implementation-aware-runtime.md").read_text(encoding="utf-8")
        srs = (ROOT / "docs/software/software-requirements-specification.md").read_text(encoding="utf-8")
        roadmap = (ROOT / "docs/software/roadmap.md").read_text(encoding="utf-8")
        architecture = (ROOT / "docs/software/architecture.md").read_text(encoding="utf-8")
        for identifier in ("FR-EXT-018", "FR-EXT-019", "FR-CLI-006"):
            self.assertIn(identifier, decision)
            self.assertIn(identifier, srs)
        self.assertIn("Issue #168", roadmap)
        self.assertIn("Decision 0028", architecture)
        self.assertIn("Proposed branch-scoped candidate", decision)

    def test_exact_runtime_capabilities_are_absent(self):
        registrations = {
            "workspace": (ROOT / "Cargo.toml", "crates/codenoesis-lang-kotlin"),
            "dependency": (ROOT / "Cargo.toml", 'tree-sitter-kotlin-ng = "=1.1.0"'),
            "provider": (
                ROOT / "crates/codenoesis-lang-rust/src/s7_provider.rs",
                'pub const PROVIDER_CAPABILITY: &str = "rust-direct-json-map/v1";',
            ),
            "client": (
                ROOT / "crates/codenoesis-lang-kotlin/src/lib.rs",
                'pub const CLIENT_CAPABILITY: &str = "kotlin-direct-json-access/v1";',
            ),
            "pipeline": (
                ROOT / "crates/noesis/src/impact.rs",
                'pub const PIPELINE_VERSION: &str = "codenoesis.pipeline/s7-v1";',
            ),
        }
        missing = []
        for name, (path, marker) in registrations.items():
            if not path.exists() or marker not in path.read_text(encoding="utf-8"):
                missing.append(name)
        self.assertEqual(
            missing,
            [],
            "S7 runtime capabilities are absent from production registration: "
            + ", ".join(missing),
        )

    def test_dependency_and_lock_projection_are_exact(self):
        workspace = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
        lock = (ROOT / "Cargo.lock").read_text(encoding="utf-8")
        self.assertIn('tree-sitter-kotlin-ng = "=1.1.0"', workspace)
        self.assertEqual(lock.count('name = "tree-sitter-kotlin-ng"'), 1)
        package = lock.split('name = "tree-sitter-kotlin-ng"', 1)[1].split("[[package]]", 1)[0]
        self.assertIn('version = "1.1.0"', package)
        self.assertIn('checksum = "e800ebbda938acfbf224f4d2c34947a31994b1295ee6e819b65226c7b51b4450"', package)
        self.assertIn(' "cc",', package)
        self.assertIn(' "tree-sitter-language",', package)
        self.assertEqual(package.count('\n "'), 2)

    def test_runtime_has_no_executable_or_network_authority(self):
        sources = [
            ROOT / "crates" / "noesis" / "src" / "impact.rs",
            ROOT / "crates" / "codenoesis-lang-rust" / "src" / "s7_provider.rs",
            ROOT / "crates" / "codenoesis-lang-kotlin" / "src" / "lib.rs",
        ]
        forbidden = [
            "Command::new",
            "TcpListener",
            "TcpStream",
            "UdpSocket",
            "reqwest",
            "model_provider",
            "std::process",
            "unsafe {",
        ]
        for path in sources:
            source = path.read_text(encoding="utf-8")
            for marker in forbidden:
                self.assertNotIn(marker, source, f"{path}: {marker}")


if __name__ == "__main__":
    unittest.main()
