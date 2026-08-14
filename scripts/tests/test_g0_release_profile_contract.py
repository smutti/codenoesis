import hashlib
import json
import pathlib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]
SPEC = ROOT / "tests/specifications/g0/release-profile-v1"
DECISION = ROOT / "docs/software/decisions/0033-g0-release-profile-registry.md"
RELEASE_PROFILES = ROOT / "docs/software/release-profiles.md"

PLATFORMS = [
    {
        "target": "x86_64-unknown-linux-gnu",
        "classification": "ci-observed-experimental",
        "sandbox_tier": "normative-linux-seccomp-landlock-v1",
        "normative_os_confinement": True,
    },
    {
        "target": "aarch64-apple-darwin",
        "classification": "ci-observed-experimental",
        "sandbox_tier": "functional-portability-only-v1",
        "normative_os_confinement": False,
    },
    {
        "target": "x86_64-pc-windows-msvc",
        "classification": "ci-observed-experimental",
        "sandbox_tier": "functional-portability-only-v1",
        "normative_os_confinement": False,
    },
]
CAPABILITIES = [
    "local-acquisition-r2",
    "local-analysis-r16",
    "local-docs-query-r16",
    "local-portable-graph-v9",
    "local-function-context-v1",
    "local-explorer-v10",
]
EXCLUSIONS = [
    "incremental-refresh-s5",
    "federation-s6",
    "implementation-aware-impact-s7",
    "trusted-source-retrieval",
    "remote-acquisition",
    "compiler-index-generation",
    "model-provider",
    "server-runtime",
    "signed-distribution",
    "release-publication",
]
LIMITATIONS = [
    "experimental_source_build_only",
    "not_verified",
    "no_support_window",
    "no_binary_distribution",
    "no_signature_or_attestation",
    "linux_only_normative_os_confinement",
    "no_ga_compatibility_promise",
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


class G0ReleaseProfileContractTest(unittest.TestCase):
    def test_exact_authority_and_bounded_effects(self):
        contract = load_json(SPEC / "contract-v1.json")
        self.assertEqual(
            contract["schema_version"],
            "codenoesis.g0-release-profile-contract/v1",
        )
        self.assertEqual(contract["issue"], 180)
        self.assertEqual(
            contract["exact_base_sha"],
            "f0bdb5290566bb85bb103e24291e952d4c557156",
        )
        self.assertEqual(contract["status"], "proposed_branch_scoped_candidate")
        self.assertEqual(contract["planning_item"], "G0")
        self.assertEqual(contract["slice"], "S14")
        self.assertEqual(contract["risk"], "critical")
        self.assertEqual(contract["requirements"], ["FR-REL-001", "FR-CLI-007"])
        self.assertEqual(contract["profile_id"], "local-experimental-r17")
        self.assertEqual(
            contract["contracts"],
            {
                "report": "codenoesis.release-profile/v1",
                "error": "codenoesis.error/v25",
                "registry": "codenoesis.release-profile-registry/v1",
            },
        )
        for field in [
            "new_dependency",
            "migration",
            "control_plane_effect",
            "workflow_effect",
            "permission_effect",
            "secret_effect",
            "signing_effect",
            "publication_effect",
            "release_effect",
            "support_commitment",
        ]:
            self.assertFalse(contract[field], field)
        self.assertEqual(
            contract["protected_digests"]["decision_0032_sha256"],
            sha256(
                ROOT
                / "docs/software/decisions/0032-s4-r17-function-context-navigation.md"
            ),
        )
        self.assertEqual(
            contract["protected_digests"]["r17_contract_bundle_sha256"],
            sha256(ROOT / "tests/specifications/s4/r17/contract-bundle.json"),
        )

    def test_closed_registry_and_platform_matrix(self):
        registry = load_json(SPEC / "registry-v1.json")
        self.assertEqual(
            registry["schema_version"],
            "codenoesis.release-profile-registry/v1",
        )
        self.assertEqual(len(registry["profiles"]), 1)
        profile = registry["profiles"][0]
        self.assertEqual(profile["profile_id"], "local-experimental-r17")
        self.assertEqual(profile["classification"], "experimental")
        self.assertEqual(profile["distribution"], "source-build-only")
        self.assertEqual(profile["support"], "none")
        self.assertEqual(profile["owner"], "@smutti")
        self.assertEqual(profile["verification"], "not-verified")
        self.assertEqual(profile["release_status"], "not-ga")
        self.assertEqual(profile["platform_matrix"], PLATFORMS)
        self.assertEqual(profile["capabilities"], CAPABILITIES)
        self.assertEqual(profile["excluded_capabilities"], EXCLUSIONS)
        self.assertEqual(profile["limitations"], LIMITATIONS)
        self.assertEqual(
            profile["release_authority"],
            {
                "signing": "not-available",
                "artifact_attestation": "not-available",
                "build_provenance": "protected-git-and-ci-evidence-only",
                "release_provenance": False,
                "release_publication": False,
                "deployment": False,
                "secrets": False,
            },
        )

    def test_exact_target_goldens_are_canonical_and_private(self):
        registry_profile = load_json(SPEC / "registry-v1.json")["profiles"][0]
        for platform in PLATFORMS:
            path = SPEC / f'expected-{platform["target"]}.json'
            raw = path.read_bytes()
            self.assertTrue(raw.endswith(b"\n"), path.name)
            self.assertEqual(raw.count(b"\n"), 1, path.name)
            report = json.loads(raw)
            expected = {
                "schema_version": "codenoesis.release-profile/v1",
                "profile_id": registry_profile["profile_id"],
                "classification": registry_profile["classification"],
                "distribution": registry_profile["distribution"],
                "support": registry_profile["support"],
                "owner": registry_profile["owner"],
                "verification": registry_profile["verification"],
                "release_status": registry_profile["release_status"],
                "selected_platform": platform,
                "platform_matrix": registry_profile["platform_matrix"],
                "capabilities": registry_profile["capabilities"],
                "excluded_capabilities": registry_profile["excluded_capabilities"],
                "release_authority": registry_profile["release_authority"],
                "limitations": registry_profile["limitations"],
            }
            self.assertEqual(report, expected, path.name)
            canonical = json.dumps(
                expected,
                ensure_ascii=False,
                separators=(",", ":"),
            ).encode("utf-8") + b"\n"
            self.assertEqual(raw, canonical, path.name)
            self.assertLessEqual(len(raw), 65536, path.name)
            text = raw.decode("utf-8")
            for canary in PRIVACY_CANARIES:
                self.assertNotIn(canary, text, (path.name, canary))

    def test_schemas_freeze_limits_and_failures(self):
        report = load_json(SPEC / "release-profile-v1.schema.json")
        self.assertFalse(report["additionalProperties"])
        properties = report["properties"]
        self.assertEqual(
            properties["schema_version"]["const"],
            "codenoesis.release-profile/v1",
        )
        self.assertEqual(properties["profile_id"]["const"], "local-experimental-r17")
        self.assertEqual(properties["platform_matrix"]["maxItems"], 16)
        self.assertEqual(properties["capabilities"]["maxItems"], 64)
        self.assertEqual(properties["excluded_capabilities"]["maxItems"], 64)
        self.assertEqual(properties["limitations"]["maxItems"], 64)
        self.assertEqual(properties["platform_matrix"]["const"], PLATFORMS)
        self.assertEqual(properties["capabilities"]["const"], CAPABILITIES)
        self.assertEqual(properties["excluded_capabilities"]["const"], EXCLUSIONS)
        self.assertEqual(properties["limitations"]["const"], LIMITATIONS)
        self.assertEqual(report["$defs"]["platform"]["properties"]["target"]["maxLength"], 128)

        error = load_json(SPEC / "codenoesis-error-v25.schema.json")
        self.assertFalse(error["additionalProperties"])
        self.assertEqual(
            error["properties"]["schema_version"]["const"],
            "codenoesis.error/v25",
        )
        self.assertEqual(
            error["properties"]["code"]["enum"],
            [
                "input.invalid_profile_command",
                "input.invalid_format",
                "profile.unknown",
                "profile.unsupported_platform",
                "profile.contract_invalid",
            ],
        )
        self.assertEqual(error["properties"]["retryable"]["const"], False)

    def test_governance_reconciles_r17_without_claiming_release(self):
        decision = DECISION.read_text(encoding="utf-8")
        self.assertIn("Status: Proposed branch-scoped candidate", decision)
        self.assertIn("Issue: [#180]", decision)
        self.assertIn("Exact base: `f0bdb5290566bb85bb103e24291e952d4c557156`", decision)
        self.assertIn("FR-REL-001", decision)
        self.assertIn("FR-CLI-007", decision)
        self.assertIn("Risk: critical", decision)
        self.assertIn("Fifty argument or registry permutations", decision)
        self.assertNotIn("Status: Accepted", decision)

        release_profiles = RELEASE_PROFILES.read_text(encoding="utf-8")
        self.assertIn("## `local-experimental-r17`", release_profiles)
        self.assertIn("protected PR #179", release_profiles)
        self.assertIn("source-build-only", release_profiles)
        self.assertIn("not Local GA", release_profiles)
        self.assertIn("Decision 0033", release_profiles)

        for relative in [
            "README.md",
            "docs/software/architecture.md",
            "docs/software/roadmap.md",
            "docs/software/software-requirements-specification.md",
        ]:
            text = (ROOT / relative).read_text(encoding="utf-8")
            self.assertIn("#180", text, relative)
            self.assertIn("Decision 0033", text, relative)
            self.assertIn("FR-REL-001", text, relative)
            self.assertIn("Approved and Implemented but not Verified", text, relative)
            self.assertNotIn("R17 is Verified", text, relative)
            self.assertNotIn("G0 is Verified", text, relative)

    def test_acceptance_contract_records_exact_red(self):
        e2e = load_json(SPEC / "e2e_fr_rel_001_release_profile.json")
        self.assertEqual(e2e["id"], "e2e_fr_rel_001_reports_bound_profile")
        self.assertEqual(e2e["status"], "Proposed branch-scoped candidate")
        self.assertEqual(e2e["planning_item"], "G0")
        self.assertEqual(e2e["slice"], "S14")
        self.assertEqual(e2e["risk"], "critical")
        self.assertEqual(e2e["exact_base"], "f0bdb5290566bb85bb103e24291e952d4c557156")
        self.assertEqual(
            e2e["command"],
            ["profile", "--id", "local-experimental-r17", "--format", "json"],
        )
        self.assertEqual(
            e2e["expected_red"],
            {
                "exit": 2,
                "stdout_bytes": 0,
                "stderr_bytes": 149,
                "schema_version": "codenoesis.error/v1",
                "code": "input.invalid_revision",
            },
        )
        self.assertEqual(e2e["expected_green"]["accepted_targets"], [p["target"] for p in PLATFORMS])
        self.assertFalse(e2e["failure"]["repair"])
        self.assertFalse(e2e["failure"]["fallback"])
        self.assertFalse(e2e["failure"]["target_override"])

    def test_contract_bundle(self):
        bundle_path = SPEC / "contract-bundle.json"
        self.assertTrue(bundle_path.exists())
        bundle = load_json(bundle_path)
        paths = [record["path"] for record in bundle["files"]]
        self.assertEqual(paths, sorted(paths))
        for record in bundle["files"]:
            self.assertEqual(sha256(ROOT / record["path"]), record["sha256"], record["path"])
        payload = "\n".join(
            f'{record["path"]}\0{record["sha256"]}' for record in bundle["files"]
        ).encode("utf-8")
        self.assertEqual(sha256_bytes(payload), bundle["bundle_sha256"])


if __name__ == "__main__":
    unittest.main()
