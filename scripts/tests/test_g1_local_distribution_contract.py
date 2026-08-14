import hashlib
import json
import pathlib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]
SPEC = ROOT / "tests/specifications/g1/local-cli-distribution-v1"
DECISION = ROOT / "docs/software/decisions/0034-g1a-local-cli-distribution-configuration.md"
G0_BUNDLE = ROOT / "tests/specifications/g0/release-profile-v1/contract-bundle.json"

TARGETS = [
    ("x86_64-unknown-linux-gnu", "bin/noesis"),
    ("aarch64-apple-darwin", "bin/noesis"),
    ("x86_64-pc-windows-msvc", "bin/noesis.exe"),
]
CONFIGURATION = {
    "execution": {
        "browser_auto_open": "disabled",
        "model_providers": "disabled",
        "network": "disabled",
        "target_execution": "disabled",
    },
    "output": {"format": "json"},
    "release_profile": "local-experimental-r17",
    "schema_version": "codenoesis.configuration/local-cli/v1",
    "secrets": {"mode": "forbidden", "references": []},
}
CONFIGURATION_SEMANTIC_HASH = (
    "62bbfd0abce84b8eab9d970fc48d3c426925e41b2f583f09d5d7215c24f30b00"
)
FIXTURE_BINARY_SHA256 = (
    "f82e988be32ec8a3f077e49f4034d42847adc415cd05ec82a9affec2fe25fb6b"
)
PAYLOADS = {
    "etc/codenoesis/config.json": (
        301,
        "a923dcf8937410ed942f6c6f3ec7899f9a2fcccc52b91653bf8aaa3df6e4e327",
        "data",
    ),
    "share/codenoesis/schemas/local-cli-config-v1.schema.json": (
        1443,
        "e9a5b92168e2163533d20c974e6472fdda8fc43399cad7329d82d2d3eefc30c4",
        "data",
    ),
    "share/doc/codenoesis/INSTALL.md": (
        945,
        "7faf884c53a5c2595850636823227076ef7f71456a3856fef648e705053ee46c",
        "data",
    ),
    "share/doc/codenoesis/LICENSE": (
        11357,
        "c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4",
        "data",
    ),
}
ERROR_CODES = [
    "configuration.invalid_arguments",
    "configuration.invalid_file",
    "configuration.unstable_input",
    "configuration.unsupported_schema",
    "configuration.unsupported_value",
    "distribution.invalid_arguments",
    "distribution.invalid_binary",
    "distribution.output_exists",
    "distribution.limit_exceeded",
    "distribution.internal",
]
PRIVACY_CANARIES = [
    "absolute_path",
    "credential",
    "environment",
    "hostname",
    "model_data",
    "source_text",
    "telemetry",
    "token",
    "url",
    "username",
]


def load_json(path: pathlib.Path):
    return json.loads(path.read_text(encoding="utf-8"))


def sha256(path: pathlib.Path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def sha256_bytes(value: bytes):
    return hashlib.sha256(value).hexdigest()


def canonical_json(value):
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8") + b"\n"


class G1LocalDistributionContractTest(unittest.TestCase):
    def test_exact_authority_and_bounded_effects(self):
        contract = load_json(SPEC / "contract-v1.json")
        self.assertEqual(
            contract["schema_version"],
            "codenoesis.g1a-local-distribution-contract/v1",
        )
        self.assertEqual(contract["issue"], 182)
        self.assertEqual(
            contract["exact_base_sha"],
            "a525126228205901885038586e21d30db745b1ec",
        )
        self.assertEqual(contract["status"], "proposed_branch_scoped_candidate")
        self.assertEqual(contract["planning_item"], "G1a")
        self.assertEqual(contract["slice"], "S14")
        self.assertEqual(contract["risk"], "high")
        self.assertEqual(
            contract["requirements"],
            ["FR-CFG-001", "FR-REL-002", "FR-CLI-008"],
        )
        self.assertEqual(
            contract["dependencies"],
            {
                "new_packages": False,
                "xtask_runtime": [
                    "codenoesis-contracts",
                    "same-file",
                    "serde_json",
                    "sha2",
                ],
                "xtask_dev": ["tempfile"],
            },
        )
        for field in [
            "migration",
            "control_plane_effect",
            "workflow_effect",
            "permission_effect",
            "secret_effect",
            "signing_effect",
            "publication_effect",
            "release_effect",
            "support_commitment",
            "server_artifact",
        ]:
            self.assertFalse(contract[field], field)
        self.assertEqual(
            sha256(G0_BUNDLE),
            contract["protected_digests"]["g0_contract_bundle_file_sha256"],
        )
        self.assertEqual(
            sha256(ROOT / "docs/software/decisions/0033-g0-release-profile-registry.md"),
            contract["protected_digests"]["decision_0033_sha256"],
        )
        self.assertEqual(
            sha256(ROOT / "tests/specifications/g0/release-profile-v1/registry-v1.json"),
            contract["protected_digests"]["g0_registry_sha256"],
        )

    def test_configuration_and_reports_are_exact_canonical_bytes(self):
        configuration_path = SPEC / "default-config.json"
        configuration = load_json(configuration_path)
        self.assertEqual(configuration, CONFIGURATION)
        self.assertEqual(configuration_path.read_bytes(), canonical_json(CONFIGURATION))
        self.assertLessEqual(len(configuration_path.read_bytes()), 65536)

        for source, leaf in [
            ("embedded-default", "expected-config-embedded.json"),
            ("explicit-file", "expected-config-explicit.json"),
        ]:
            path = SPEC / leaf
            report = load_json(path)
            expected = {
                "configuration": CONFIGURATION,
                "configuration_schema": "codenoesis.configuration/local-cli/v1",
                "release_profile": "local-experimental-r17",
                "schema_version": "codenoesis.configuration-report/v1",
                "semantic_hash": {
                    "algorithm": "blake3-256",
                    "value": CONFIGURATION_SEMANTIC_HASH,
                },
                "source": source,
            }
            self.assertEqual(report, expected)
            self.assertEqual(path.read_bytes(), canonical_json(expected))
            self.assertLessEqual(len(path.read_bytes()), 65536)
            text = path.read_text(encoding="utf-8")
            for canary in PRIVACY_CANARIES:
                self.assertNotIn(canary, text, (leaf, canary))

    def test_schemas_freeze_closed_values_limits_and_errors(self):
        configuration = load_json(SPEC / "local-cli-config-v1.schema.json")
        self.assertFalse(configuration["additionalProperties"])
        self.assertEqual(
            configuration["properties"]["schema_version"]["const"],
            "codenoesis.configuration/local-cli/v1",
        )
        self.assertEqual(
            configuration["properties"]["release_profile"]["const"],
            "local-experimental-r17",
        )
        self.assertFalse(
            configuration["properties"]["execution"]["additionalProperties"]
        )
        self.assertEqual(
            configuration["properties"]["secrets"]["properties"]["references"]["maxItems"],
            0,
        )

        report = load_json(SPEC / "local-configuration-report-v1.schema.json")
        self.assertFalse(report["additionalProperties"])
        self.assertEqual(
            report["properties"]["schema_version"]["const"],
            "codenoesis.configuration-report/v1",
        )
        self.assertEqual(
            report["properties"]["semantic_hash"]["properties"]["value"]["const"],
            CONFIGURATION_SEMANTIC_HASH,
        )

        manifest = load_json(SPEC / "local-distribution-manifest-v1.schema.json")
        self.assertFalse(manifest["additionalProperties"])
        self.assertEqual(
            manifest["properties"]["schema_version"]["const"],
            "codenoesis.local-distribution/v1",
        )
        self.assertEqual(manifest["properties"]["files"]["minItems"], 5)
        self.assertEqual(manifest["properties"]["files"]["maxItems"], 5)
        self.assertEqual(
            manifest["properties"]["target"]["enum"],
            [target for target, _ in TARGETS],
        )

        error = load_json(SPEC / "codenoesis-error-v26.schema.json")
        self.assertFalse(error["additionalProperties"])
        self.assertEqual(
            error["properties"]["schema_version"]["const"],
            "codenoesis.error/v26",
        )
        self.assertEqual(error["properties"]["code"]["enum"], ERROR_CODES)
        self.assertEqual(error["properties"]["retryable"]["const"], False)

    def test_target_manifests_freeze_exact_fixture_tree(self):
        fixture = SPEC / "fixtures/noesis-v1.bin"
        self.assertEqual(fixture.stat().st_size, 43)
        self.assertEqual(sha256(fixture), FIXTURE_BINARY_SHA256)

        for target, binary_path in TARGETS:
            leaf = f"expected-manifest-{target}.json"
            path = SPEC / leaf
            manifest = load_json(path)
            expected_files = [
                {
                    "length": 43,
                    "mode": "executable",
                    "path": binary_path,
                    "sha256": FIXTURE_BINARY_SHA256,
                }
            ]
            expected_files.extend(
                {
                    "length": length,
                    "mode": mode,
                    "path": payload_path,
                    "sha256": digest,
                }
                for payload_path, (length, digest, mode) in PAYLOADS.items()
            )
            expected = {
                "artifact_attestation": "not-available",
                "binary_sha256": FIXTURE_BINARY_SHA256,
                "distribution": "unsigned-staged-directory",
                "files": expected_files,
                "profile_id": "local-experimental-r17",
                "publication": False,
                "release_provenance": False,
                "release_status": "not-ga",
                "schema_version": "codenoesis.local-distribution/v1",
                "signing": "not-available",
                "support": "none",
                "target": target,
                "verification": "not-verified",
            }
            self.assertEqual(manifest, expected, leaf)
            self.assertEqual(path.read_bytes(), canonical_json(expected), leaf)
            self.assertLessEqual(len(path.read_bytes()), 65536, leaf)
            text = path.read_text(encoding="utf-8")
            for canary in PRIVACY_CANARIES:
                self.assertNotIn(canary, text, (leaf, canary))

    def test_payload_sources_and_lifecycle_are_frozen(self):
        source_files = {
            "etc/codenoesis/config.json": SPEC / "default-config.json",
            "share/codenoesis/schemas/local-cli-config-v1.schema.json": SPEC
            / "local-cli-config-v1.schema.json",
            "share/doc/codenoesis/INSTALL.md": SPEC / "install-v1.md",
            "share/doc/codenoesis/LICENSE": ROOT / "LICENSE",
        }
        for payload_path, source in source_files.items():
            length, digest, _ = PAYLOADS[payload_path]
            self.assertEqual(source.stat().st_size, length, payload_path)
            self.assertEqual(sha256(source), digest, payload_path)

        tree = load_json(SPEC / "tree-v1.json")
        self.assertEqual(tree["logical_file_count"], 6)
        self.assertEqual(tree["payload_count"], 5)
        self.assertEqual(tree["paths"][-1], {"path": "manifest.json", "mode": "data"})
        self.assertFalse(tree["lifecycle"]["hidden_activation_state"])
        self.assertFalse(tree["lifecycle"]["path_or_system_mutation"])
        self.assertEqual(sha256(SPEC / "fixtures/noesis-v2.bin"), (
            "9d0005fbbae37436afe941a35dd6324b015be474371f808c6016b031f24d92ad"
        ))

    def test_invalid_matrix_and_expected_reds(self):
        invalid = load_json(SPEC / "invalid-cases-v1.json")
        observed_codes = {
            case["code"]
            for family in ["configuration", "distribution"]
            for case in invalid[family]
        }
        self.assertEqual(observed_codes, set(ERROR_CODES))

        configuration = load_json(SPEC / "e2e_fr_cfg_001_local_configuration.json")
        self.assertEqual(
            configuration["id"],
            "e2e_fr_cfg_001_validates_embedded_default",
        )
        self.assertEqual(configuration["planning_item"], "G1a")
        self.assertEqual(configuration["slice"], "S14")
        self.assertEqual(configuration["risk"], "high")
        self.assertEqual(
            configuration["expected_red"],
            {
                "exit": 2,
                "stdout_bytes": 0,
                "stderr_bytes": 149,
                "schema_version": "codenoesis.error/v1",
                "code": "input.invalid_revision",
            },
        )

        distribution = load_json(SPEC / "e2e_fr_rel_002_local_distribution.json")
        self.assertEqual(distribution["id"], "e2e_fr_rel_002_packages_local_cli")
        self.assertEqual(distribution["planning_item"], "G1a")
        self.assertEqual(distribution["slice"], "S14")
        self.assertEqual(distribution["risk"], "high")
        self.assertEqual(distribution["expected_red"]["exit"], 0)
        self.assertEqual(distribution["expected_red"]["stdout_bytes"], 123)
        self.assertEqual(distribution["expected_red"]["stderr_bytes"], 0)
        self.assertFalse(distribution["expected_red"]["bundle_created"])

    def test_governance_records_candidate_and_g0_lifecycle(self):
        decision = DECISION.read_text(encoding="utf-8")
        self.assertIn("Status: Proposed branch-scoped candidate", decision)
        self.assertIn("Issue: [#182]", decision)
        self.assertIn(
            "Exact base: `a525126228205901885038586e21d30db745b1ec`",
            decision,
        )
        self.assertIn("FR-CFG-001", decision)
        self.assertIn("FR-REL-002", decision)
        self.assertIn("FR-CLI-008", decision)
        self.assertIn("Risk: high", decision)
        self.assertIn("Fifty argument constructions and ten schedules", decision)
        self.assertNotIn("Status: Accepted", decision)

        for relative in [
            "README.md",
            "docs/software/architecture.md",
            "docs/software/release-profiles.md",
            "docs/software/roadmap.md",
            "docs/software/software-requirements-specification.md",
        ]:
            text = (ROOT / relative).read_text(encoding="utf-8")
            self.assertIn("#182", text, relative)
            self.assertIn("Decision 0034", text, relative)
            self.assertIn("FR-CFG-001", text, relative)
            self.assertIn("Approved and Implemented but not Verified", text, relative)
            self.assertNotIn("G1a is Verified", text, relative)

    def test_contract_bundle(self):
        bundle_path = SPEC / "contract-bundle.json"
        self.assertTrue(bundle_path.exists())
        bundle = load_json(bundle_path)
        paths = [record["path"] for record in bundle["files"]]
        self.assertEqual(paths, sorted(paths))
        for record in bundle["files"]:
            self.assertEqual(
                sha256(ROOT / record["path"]),
                record["sha256"],
                record["path"],
            )
        payload = "\n".join(
            f'{record["path"]}\0{record["sha256"]}' for record in bundle["files"]
        ).encode("utf-8")
        self.assertEqual(sha256_bytes(payload), bundle["bundle_sha256"])


if __name__ == "__main__":
    unittest.main()
