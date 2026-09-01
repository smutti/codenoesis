import hashlib
import json
import pathlib
import subprocess
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]
SPEC = ROOT / "tests/specifications/s7/r19"
FIXTURE = ROOT / "tests/fixtures/s7/implementation-aware-api-git-v1"
DECISION = ROOT / "docs/software/decisions/0040-s7-git-backed-semantic-impact-evidence.md"
R19_PROTECTED_MERGE = "c783b612777a86e2f88620ece987723bb230c51c"
R19_HISTORICAL_BUNDLE_PATHS = frozenset(
    {
        "README.md",
        "docs/software/architecture.md",
        "docs/software/decisions/0040-s7-git-backed-semantic-impact-evidence.md",
        "docs/software/roadmap.md",
        "docs/software/software-requirements-specification.md",
        "docs/software/verification.md",
        "scripts/tests/test_s7_r19_git_evidence_contract.py",
    }
)


def load_json(path: pathlib.Path):
    return json.loads(path.read_text(encoding="utf-8"))


def sha256(path: pathlib.Path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def sha256_bytes(value: bytes):
    return hashlib.sha256(value).hexdigest()


def git_blob(commit_sha: str, relative_path: str):
    completed = subprocess.run(
        ["git", "show", f"{commit_sha}:{relative_path}"],
        cwd=ROOT,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if completed.returncode != 0:
        diagnostic = completed.stderr.decode("utf-8", errors="replace").strip()
        raise RuntimeError(
            f"unable to read {relative_path} from {commit_sha}: {diagnostic}"
        )
    return completed.stdout


def contract_bytes(relative_path: str):
    if relative_path in R19_HISTORICAL_BUNDLE_PATHS:
        return git_blob(R19_PROTECTED_MERGE, relative_path)
    return (ROOT / relative_path).read_bytes()


def historical_text(relative_path: str):
    return git_blob(R19_PROTECTED_MERGE, relative_path).decode("utf-8")


class S7R19GitEvidenceContractTests(unittest.TestCase):
    def test_exact_authority_contracts_and_limits(self):
        contract = load_json(SPEC / "runtime-contract-v1.json")
        self.assertEqual(contract["issue"], 196)
        self.assertEqual(
            contract["exact_base_sha"],
            "fcdd6eddec8a4dd9b372cb88ff424c2004b5c88b",
        )
        self.assertEqual(contract["status"], "proposed_branch_scoped_candidate")
        self.assertEqual(contract["slice"], "S7")
        self.assertEqual(contract["risk"], "high")
        self.assertEqual(
            contract["requirements"],
            ["FR-IMP-006", "FR-CLI-006", "FR-CLI-012"],
        )
        self.assertEqual(
            contract["contracts"]["analysis_profile"],
            "implementation-aware-http-json-git-v1",
        )
        self.assertEqual(
            contract["contracts"]["source_profile"],
            "trusted-local-impact-source-v1",
        )
        self.assertEqual(contract["limits"]["client_repositories_maximum"], 32)
        self.assertEqual(contract["limits"]["workspace_bytes"], 1_048_576)
        self.assertEqual(contract["limits"]["semantic_report_bytes"], 67_108_864)
        self.assertEqual(contract["limits"]["excerpt_bytes"], 262_144)
        self.assertFalse(contract["new_dependency"])
        self.assertFalse(contract["migration"])
        self.assertFalse(contract["control_plane_effect"])
        self.assertFalse(contract["release_effect"])

    def test_schemas_are_closed_and_bind_git_objects(self):
        workspace = load_json(SPEC / "impact-git-workspace-v1.schema.json")
        self.assertFalse(workspace["additionalProperties"])
        self.assertEqual(
            workspace["properties"]["schema_version"]["const"],
            "codenoesis.impact-git-workspace/v1",
        )
        self.assertEqual(workspace["properties"]["clients"]["maxItems"], 32)
        self.assertEqual(workspace["$defs"]["sha1"]["pattern"], "^[0-9a-f]{40}$")

        report = load_json(SPEC / "semantic-compatibility-report-v2.schema.json")
        self.assertFalse(report["additionalProperties"])
        self.assertEqual(
            report["properties"]["schema_version"]["const"],
            "codenoesis.semantic-compatibility-report/v2",
        )
        self.assertEqual(
            report["properties"]["evidence_lineage_version"]["const"],
            "codenoesis.source-evidence/git-v1",
        )
        binding = report["$defs"]["git_evidence"]["properties"]["source_binding"]
        self.assertFalse(binding["additionalProperties"])
        self.assertEqual(
            binding["required"], ["commit_oid", "tree_oid", "blob_oid", "span"]
        )

        excerpt = load_json(SPEC / "trusted-impact-source-excerpt-v1.schema.json")
        self.assertEqual(
            excerpt["properties"]["schema_version"]["const"],
            "codenoesis.trusted-impact-source-excerpt/v1",
        )
        self.assertEqual(
            excerpt["properties"]["profile"]["const"],
            "trusted-local-impact-source-v1",
        )
        self.assertEqual(
            excerpt["properties"]["excerpt"]["properties"]["byte_length"]["maximum"],
            262_144,
        )

        error = load_json(SPEC / "codenoesis-error-v30.schema.json")
        self.assertEqual(
            error["properties"]["schema_version"]["const"],
            "codenoesis.error/v30",
        )
        self.assertEqual(error["properties"]["context"]["maxProperties"], 0)

    def test_project_owned_git_fixture_and_v1_oracle_are_immutable(self):
        manifest = load_json(FIXTURE / "manifest.json")
        self.assertEqual(
            manifest["schema_version"],
            "codenoesis.s7-r19-fixture-manifest/v1",
        )
        self.assertEqual(
            manifest["provider"]["baseline"]["commit_oid"],
            "73cc0752413bd337a6507ffcc422d7d5a4458523",
        )
        self.assertEqual(
            manifest["provider"]["target"]["commit_oid"],
            "fd6c8a3b1988e6a963a46824da09ec6132cf0290",
        )
        self.assertEqual(len(manifest["clients"]), 3)
        self.assertEqual(
            [client["role"] for client in manifest["clients"]],
            ["decoy", "safe", "strict"],
        )
        self.assertTrue(all(value is False for value in manifest["sentinels"].values()))

        federation = ROOT / manifest["inherited_federation_report"]["path"]
        self.assertEqual(
            federation.stat().st_size,
            manifest["inherited_federation_report"]["byte_length"],
        )
        self.assertEqual(
            sha256(federation), manifest["inherited_federation_report"]["sha256"]
        )
        v1 = ROOT / manifest["immutable_v1_report"]["path"]
        self.assertEqual(v1.stat().st_size, 14_991)
        self.assertEqual(
            sha256(v1),
            "cfd9a8d4dcb2d04bcd9eaffd15f1ae947ffdaba80e07daee43375c9a67c15750",
        )
        self.assertEqual(
            manifest["expected_report"],
            {
                "schema_version": "codenoesis.semantic-compatibility-report/v2",
                "semantic_diffs": 2,
                "client_assessments": 2,
                "rejected_candidates": 1,
                "evidence": 9,
                "coverage_gaps": 1,
                "all_evidence_navigable": True,
            },
        )

    def test_candidate_governance_and_red_contract_are_complete(self):
        decision = historical_text(DECISION.relative_to(ROOT).as_posix())
        self.assertIn("Status: Proposed branch-scoped candidate", decision)
        self.assertIn("Issue: [#196]", decision)
        self.assertIn("FR-IMP-006", decision)
        self.assertIn("FR-CLI-012", decision)
        self.assertIn("five", decision)
        self.assertIn("implementation-aware-http-json-git-v1", decision)
        self.assertIn(
            "fcdd6eddec8a4dd9b372cb88ff424c2004b5c88b", decision
        )
        self.assertNotIn("Status: Accepted", decision)

        e2e = load_json(SPEC / "e2e_fr_imp_006_git_evidence_navigation.json")
        self.assertEqual(
            e2e["id"], "e2e_fr_imp_006_git_bound_semantic_diff_is_navigable"
        )
        self.assertEqual(e2e["expected_green"]["navigable_evidence"], 9)
        self.assertEqual(e2e["expected_green"]["argument_permutations"], 50)
        self.assertEqual(e2e["expected_green"]["process_schedules"], 10)

    def test_contract_bundle(self):
        bundle = load_json(SPEC / "contract-bundle.json")
        paths = [record["path"] for record in bundle["files"]]
        self.assertEqual(paths, sorted(paths))
        self.assertTrue(R19_HISTORICAL_BUNDLE_PATHS.issubset(paths))
        for record in bundle["files"]:
            self.assertEqual(
                sha256_bytes(contract_bytes(record["path"])),
                record["sha256"],
                record["path"],
            )
        payload = "\n".join(
            f'{record["path"]}\0{record["sha256"]}' for record in bundle["files"]
        ).encode("utf-8")
        self.assertEqual(hashlib.sha256(payload).hexdigest(), bundle["bundle_sha256"])

    def test_production_registration_is_absent(self):
        registrations = {
            "crates/codenoesis-contracts/src/lib.rs": "mod s7_r19;",
            "crates/codenoesis-application/src/lib.rs": "mod s7_r19;",
            "crates/noesis/src/main.rs": "mod impact_git;",
            "crates/noesis/src/main.rs#source": "mod impact_source;",
        }
        for label, registration in registrations.items():
            relative = label.split("#", 1)[0]
            text = (ROOT / relative).read_text(encoding="utf-8")
            self.assertIn(
                registration,
                text,
                f"R19 production registration absent from {relative}",
            )


if __name__ == "__main__":
    unittest.main()
