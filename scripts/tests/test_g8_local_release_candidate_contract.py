import hashlib
import json
import pathlib
import runpy
import tempfile
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]
SPEC = ROOT / "tests/specifications/g8/local-release-candidate-v1"
DECISION = ROOT / "docs/software/decisions/0036-local-verifiable-distribution.md"
POLICY = ROOT / "supply-chain/local-release-policy-v1.json"
WORKFLOW = ROOT / ".github/workflows/local-release-candidate.yml"
CI = ROOT / ".github/workflows/ci.yml"

TARGETS = [
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "x86_64-unknown-linux-gnu",
]
ERROR_CODES = [
    "release.internal",
    "release.invalid_archive",
    "release.invalid_arguments",
    "release.invalid_bundle",
    "release.invalid_evidence",
    "release.limit_exceeded",
    "release.policy_rejected",
    "release.unstable_input",
]
EXPECTED_INVALID_CASES = {
    "archive-central-local-mismatch",
    "archive-comment",
    "archive-compressed-entry",
    "archive-crc-mismatch",
    "archive-data-descriptor",
    "archive-duplicate-entry",
    "archive-encrypted",
    "archive-extra-field",
    "archive-maximum-plus-one",
    "archive-path-absolute",
    "archive-path-backslash",
    "archive-path-traversal",
    "archive-zip64",
    "bundle-extra-file",
    "bundle-manifest-tamper",
    "bundle-missing-file",
    "bundle-symlink",
    "candidate-extra-file",
    "candidate-name-mismatch",
    "checksum-substitution",
    "duplicate-argument",
    "evidence-cargo-lock-mismatch",
    "evidence-duplicate-package",
    "evidence-expired-unsafe-exception",
    "evidence-license-rejected",
    "evidence-missing-file",
    "evidence-private-canary",
    "evidence-sbom-target-mismatch",
    "evidence-unknown-schema",
    "evidence-vulnerability",
    "missing-argument",
    "output-not-empty",
    "output-race",
    "source-commit-invalid",
    "unstable-input",
    "unsupported-target",
}


def load_json(path: pathlib.Path):
    return json.loads(path.read_text(encoding="utf-8"))


def canonical_json(value):
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8") + b"\n"


def sha256(path: pathlib.Path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def sha256_bytes(value: bytes):
    return hashlib.sha256(value).hexdigest()


class G8LocalReleaseCandidateContractTest(unittest.TestCase):
    def test_supply_generator_rejects_first_party_unsafe(self):
        generator = runpy.run_path(
            str(ROOT / "scripts/generate_local_supply_chain_evidence.py")
        )
        with tempfile.TemporaryDirectory() as temporary_directory:
            package_root = pathlib.Path(temporary_directory) / "workspace-package"
            (package_root / "src").mkdir(parents=True)
            (package_root / "Cargo.toml").write_text(
                '[package]\nname = "workspace-package"\nversion = "0.1.0"\n',
                encoding="utf-8",
            )
            source = package_root / "src/lib.rs"
            source.write_text(
                '// unsafe is rejected only as a Rust construct\n'
                'pub const POLICY_WORD: &str = "unsafe";\n',
                encoding="utf-8",
            )
            dependency = {
                "packages": [
                    {
                        "id": "workspace-package@0.1.0",
                        "name": "workspace-package",
                        "version": "0.1.0",
                        "source": "workspace",
                    }
                ]
            }
            package_details = {
                "workspace-package@0.1.0": {
                    "manifest_path": str(package_root / "Cargo.toml")
                }
            }
            accepted = generator["build_unsafe_inventory"](
                dependency,
                package_details,
                {},
                "x86_64-unknown-linux-gnu",
                "0" * 64,
            )
            self.assertEqual(accepted["packages"][0]["unsafe_tokens"], 0)

            source.write_text("unsafe fn rejected() {}\n", encoding="utf-8")
            with self.assertRaises(generator["EvidenceError"]):
                generator["build_unsafe_inventory"](
                    dependency,
                    package_details,
                    {},
                    "x86_64-unknown-linux-gnu",
                    "0" * 64,
                )

    def test_exact_branch_authority_dependencies_and_limits(self):
        contract = load_json(SPEC / "contract-v1.json")
        self.assertEqual(
            contract["schema_version"],
            "codenoesis.g1b-g8-local-release-contract/v1",
        )
        self.assertEqual(contract["issue"], 186)
        self.assertEqual(contract["requirements"], ["FR-REL-003", "FR-CLI-010"])
        self.assertEqual(contract["slice"], "S14")
        self.assertEqual(contract["risk"], "critical")
        self.assertEqual(
            contract["base_sha"],
            "c5d259d7689b8a49527f8322b606e58cc0e1e61d",
        )
        self.assertEqual(
            contract["cargo_lock_sha256"],
            "434cc5e8e38a4c57f35990431d4682974b6cae94893860e1948c8f7cc21ffbca",
        )
        self.assertEqual(
            contract["dependencies"],
            {
                "workspace_existing": "crc32fast = 1.5.0",
                "cargo_audit": "0.22.2",
                "checkout": "3d3c42e5aac5ba805825da76410c181273ba90b1",
                "upload_artifact": "043fb46d1a93c77aae656e7c1c64a875d1fc6a0a",
                "download_artifact": "3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c",
                "attest_build_provenance": (
                    "4d101475d8b20a2381f78447822ac1eab6504dd8"
                ),
            },
        )
        self.assertEqual(
            contract["limits"],
            {
                "bundles": 1,
                "files_per_bundle": 6,
                "archive_bytes": 285212672,
                "evidence_document_bytes": 4194304,
                "evidence_total_bytes": 33554432,
                "candidate_subject_files": 8,
                "packages": 512,
                "dependency_edges": 4096,
                "unsafe_exceptions": 512,
                "zip_entries": 1024,
                "relative_path_bytes": 256,
                "public_json_bytes": 131072,
                "constructions": 50,
                "schedules": 10,
                "artifact_retention_days": 30,
            },
        )
        for field in [
            "system_install",
            "automatic_update",
            "tag",
            "github_release",
            "package_publication",
            "deployment",
            "secrets",
            "ga",
        ]:
            self.assertFalse(contract["authority"][field], field)
        self.assertEqual(contract["authority"]["support"], "none")
        self.assertEqual(
            contract["authority"]["post_merge_attestation_dispatch"],
            "maintainer-only",
        )

    def test_closed_schemas_and_error_surface_are_exact(self):
        schema_names = [
            "codenoesis-error-v28.schema.json",
            "cyclonedx-1.6-profile.schema.json",
            "local-advisory-report-v1.schema.json",
            "local-dependency-lock-v1.schema.json",
            "local-license-report-v1.schema.json",
            "local-release-candidate-manifest-v1.schema.json",
            "local-release-candidate-verification-v1.schema.json",
            "local-unsafe-inventory-v1.schema.json",
        ]
        for name in schema_names:
            schema = load_json(SPEC / name)
            self.assertFalse(schema["additionalProperties"], name)

        error = load_json(SPEC / "codenoesis-error-v28.schema.json")
        self.assertEqual(error["properties"]["code"]["enum"], ERROR_CODES)
        self.assertEqual(
            error["properties"]["schema_version"]["const"],
            "codenoesis.error/v28",
        )
        self.assertFalse(error["properties"]["retryable"]["const"])

        manifest = load_json(
            SPEC / "local-release-candidate-manifest-v1.schema.json"
        )
        self.assertEqual(manifest["properties"]["target"]["enum"], TARGETS)
        self.assertEqual(
            manifest["properties"]["carrier"]["const"],
            "deterministic-zip-stored-v1",
        )
        self.assertFalse(manifest["properties"]["publication"]["const"])
        self.assertTrue(
            manifest["properties"]["runtime_profile_release_authority_unchanged"][
                "const"
            ]
        )
        self.assertEqual(
            manifest["properties"]["attestation"]["properties"]["provider"][
                "const"
            ],
            "github-artifact-attestations",
        )

    def test_target_goldens_are_canonical_and_match_frozen_oracles(self):
        contract = load_json(SPEC / "contract-v1.json")
        self.assertEqual(list(contract["target_oracles"]), TARGETS)
        for target in TARGETS:
            oracle = contract["target_oracles"][target]
            manifest_path = SPEC / f"expected-manifest-{target}.json"
            verification_path = SPEC / f"expected-verification-{target}.json"
            manifest = load_json(manifest_path)
            verification = load_json(verification_path)

            self.assertEqual(manifest_path.read_bytes(), canonical_json(manifest))
            self.assertEqual(
                verification_path.read_bytes(), canonical_json(verification)
            )
            self.assertEqual(manifest_path.stat().st_size, oracle["manifest_bytes"])
            self.assertEqual(sha256(manifest_path), oracle["manifest_sha256"])
            self.assertEqual(
                verification_path.stat().st_size,
                oracle["verification_bytes"],
            )
            self.assertEqual(
                sha256(verification_path), oracle["verification_sha256"]
            )
            self.assertEqual(manifest["target"], target)
            self.assertEqual(manifest["archive"]["sha256"], oracle["archive_sha256"])
            self.assertEqual(manifest["archive"]["length"], oracle["archive_bytes"])
            self.assertEqual(verification["candidate_name"], oracle["candidate_name"])
            self.assertEqual(
                verification["checksums_sha256"], oracle["checksums_sha256"]
            )
            self.assertEqual(
                [record["path"] for record in manifest["evidence"]],
                sorted(record["path"] for record in manifest["evidence"]),
            )

    def test_public_journey_red_invalid_matrix_and_errors_are_frozen(self):
        contract = load_json(SPEC / "contract-v1.json")
        self.assertEqual(
            contract["expected_red"],
            {
                "command_exit": 2,
                "stdout_bytes": 0,
                "stderr_bytes": 170,
                "stderr_sha256": (
                    "cd5f646ce966c60887ae0ed110142ba22c4a5f6a05a792c72bfbab5ba3a94311"
                ),
                "schema_version": "codenoesis.error/v26",
                "code": "distribution.invalid_arguments",
                "workflow": "absent",
            },
        )
        journey = load_json(SPEC / "e2e_fr_rel_003_local_release_candidate.json")
        self.assertEqual(journey["requirement_ids"], ["FR-REL-003", "FR-CLI-010"])
        self.assertEqual(journey["determinism"], {"constructions": 50, "schedules": 10})
        self.assertEqual(
            set(load_json(SPEC / "invalid-cases-v1.json")["cases"]),
            EXPECTED_INVALID_CASES,
        )
        for label, code in [
            ("invalid-arguments", "release.invalid_arguments"),
            ("invalid-evidence", "release.invalid_evidence"),
            ("invalid-archive", "release.invalid_archive"),
        ]:
            path = SPEC / f"expected-error-{label}.json"
            value = load_json(path)
            self.assertEqual(path.read_bytes(), canonical_json(value))
            self.assertEqual(value["code"], code)
            self.assertEqual(value["schema_version"], "codenoesis.error/v28")

    def test_reviewed_supply_policy_is_closed_and_time_bounded(self):
        policy = load_json(POLICY)
        self.assertEqual(
            policy["schema_version"], "codenoesis.local-release-policy/v1"
        )
        self.assertEqual(policy["supported_targets"], TARGETS)
        self.assertEqual(policy["cargo_audit"]["version"], "0.22.2")
        self.assertEqual(
            policy["cargo_audit"]["vulnerability_policy"], "deny-all"
        )
        self.assertEqual(
            policy["unsafe_inventory"]["method"],
            "conservative-rust-token-scan-v1",
        )
        self.assertGreater(len(policy["allowed_license_expressions"]), 0)
        exceptions = policy["unsafe_exceptions"]
        self.assertGreater(len(exceptions), 0)
        identities = [(entry["package"], entry["version"]) for entry in exceptions]
        self.assertEqual(len(identities), len(set(identities)))
        ids = [entry["id"] for entry in exceptions]
        self.assertEqual(len(ids), len(set(ids)))
        for entry in exceptions:
            self.assertEqual(entry["owner"], "@smutti")
            self.assertEqual(entry["expires_on"], "2026-11-14")
            self.assertGreater(entry["reviewed_rust_files"], 0)
            self.assertGreater(entry["reviewed_unsafe_tokens"], 0)
            self.assertEqual(entry["targets"], sorted(entry["targets"]))
            self.assertTrue(set(entry["targets"]).issubset(TARGETS))
            self.assertNotIn("/Users/", json.dumps(entry))

    def test_governance_records_proposed_critical_candidate(self):
        decision = DECISION.read_text(encoding="utf-8")
        normalized = " ".join(decision.split())
        for statement in [
            "Status: Proposed branch-scoped candidate",
            "Issue: [#186]",
            "Exact base: `c5d259d7689b8a49527f8322b606e58cc0e1e61d`",
            "FR-REL-003",
            "FR-CLI-010",
            "Planning package: `G1b/G8-local`",
            "Slice: `S14`",
            "Risk: critical",
            "fifty constructions and ten schedules",
        ]:
            self.assertIn(statement, normalized)
        self.assertNotIn("Status: Accepted", decision)
        self.assertNotIn("G1b/G8-local is Verified", decision)

        requirement_documents = {
            "README.md",
            "docs/software/architecture.md",
            "docs/software/roadmap.md",
            "docs/software/software-requirements-specification.md",
        }
        for relative in [
            "README.md",
            "docs/software/architecture.md",
            "docs/software/distribution.md",
            "docs/software/release-profiles.md",
            "docs/software/roadmap.md",
            "docs/software/software-requirements-specification.md",
            "docs/software/threat-model.md",
        ]:
            text = (ROOT / relative).read_text(encoding="utf-8")
            normalized_text = " ".join(text.replace(">", " ").split())
            self.assertIn("#186", normalized_text, relative)
            self.assertIn("Decision 0036", normalized_text, relative)
            if relative in requirement_documents:
                self.assertIn("FR-REL-003", normalized_text, relative)
                self.assertIn("FR-CLI-010", normalized_text, relative)
            self.assertNotIn("G1b/G8-local is Verified", normalized_text, relative)

    def test_trusted_workflow_and_ci_gate_are_exact(self):
        self.assertTrue(
            WORKFLOW.exists(),
            "expected Red: protected-main local release candidate workflow is absent",
        )
        workflow = WORKFLOW.read_text(encoding="utf-8")
        for required in [
            "workflow_dispatch:",
            "expected_sha:",
            "refs/heads/main",
            "contents: read",
            "id-token: write",
            "attestations: write",
            "runs-on: ubuntu-latest",
            "runs-on: macos-latest",
            "runs-on: windows-latest",
            "retention-days: 30",
            "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1",
            "actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a",
            "actions/download-artifact@3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c",
            "actions/attest-build-provenance@4d101475d8b20a2381f78447822ac1eab6504dd8",
            "cargo-audit --version 0.22.2",
        ]:
            self.assertIn(required, workflow)
        for forbidden in [
            "pull_request:",
            "push:",
            "contents: write",
            "packages: write",
            "security-events: write",
            "deployments: write",
            "secrets.",
            "self-hosted",
        ]:
            self.assertNotIn(forbidden, workflow)

        ci = CI.read_text(encoding="utf-8")
        self.assertIn("supply-chain:", ci)
        self.assertIn("needs: [quality, portability, supply-chain]", ci)
        self.assertNotIn("id-token: write", ci)
        self.assertNotIn("attestations: write", ci)

    def test_contract_bundle_is_complete_and_self_excluding(self):
        bundle_path = SPEC / "contract-bundle.json"
        bundle = load_json(bundle_path)
        self.assertEqual(bundle["schema_version"], "codenoesis.contract-bundle/v1")
        paths = [record["path"] for record in bundle["files"]]
        self.assertEqual(paths, sorted(paths))
        self.assertNotIn(
            "tests/specifications/g8/local-release-candidate-v1/contract-bundle.json",
            paths,
        )
        required = {
            "docs/software/decisions/0036-local-verifiable-distribution.md",
            "distribution/local-cli/VERIFY.md",
            "scripts/tests/test_g8_local_release_candidate_contract.py",
            "supply-chain/local-release-policy-v1.json",
            "xtask/tests/e2e_fr_rel_003_local_release_candidate.rs",
        }
        self.assertTrue(required.issubset(paths))
        for record in bundle["files"]:
            self.assertEqual(sha256(ROOT / record["path"]), record["sha256"])
        payload = "\n".join(
            f'{record["path"]}\0{record["sha256"]}' for record in bundle["files"]
        ).encode("utf-8")
        self.assertEqual(sha256_bytes(payload), bundle["bundle_sha256"])


if __name__ == "__main__":
    unittest.main()
