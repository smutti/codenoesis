import hashlib
import json
import pathlib
import subprocess
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]
SPEC = ROOT / "tests/specifications/s4/r18"
FIXTURE = ROOT / "tests/fixtures/s4/trusted-source-retrieval-v1"
DECISION = ROOT / "docs/software/decisions/0038-s4-trusted-local-source-retrieval.md"
R18_PROTECTED_MERGE = "fcdd6eddec8a4dd9b372cb88ff424c2004b5c88b"
R18_HISTORICAL_BUNDLE_PATHS = frozenset(
    {
        "README.md",
        "docs/software/architecture.md",
        "docs/software/decisions/0037-local-baseline-verification-v2.md",
        "docs/software/decisions/0038-s4-trusted-local-source-retrieval.md",
        "docs/software/roadmap.md",
        "docs/software/software-requirements-specification.md",
        "docs/software/verification.md",
        "scripts/tests/test_s4_r18_trusted_source_retrieval_contract.py",
    }
)


def load_json(path: pathlib.Path):
    return json.loads(path.read_text(encoding="utf-8"))


def sha256_bytes(value: bytes):
    return hashlib.sha256(value).hexdigest()


def sha256(path: pathlib.Path):
    return sha256_bytes(path.read_bytes())


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
    if relative_path in R18_HISTORICAL_BUNDLE_PATHS:
        return git_blob(R18_PROTECTED_MERGE, relative_path)
    return (ROOT / relative_path).read_bytes()


def historical_text(relative_path: str):
    return git_blob(R18_PROTECTED_MERGE, relative_path).decode("utf-8")


class R18TrustedSourceRetrievalContractTest(unittest.TestCase):
    def test_exact_authority_and_contract_family(self):
        contract = load_json(SPEC / "contract-v1.json")
        self.assertEqual(
            contract["schema_version"],
            "codenoesis.r18-trusted-source-contract/v1",
        )
        self.assertEqual(contract["issue"], 190)
        self.assertEqual(
            contract["exact_base_sha"],
            "1de6a420f25a1c7eb74d07a99f1800dde90eefa8",
        )
        self.assertEqual(contract["status"], "proposed_branch_scoped_candidate")
        self.assertEqual(contract["slice"], "S4")
        self.assertEqual(contract["risk"], "high")
        self.assertEqual(contract["requirements"], ["FR-CTX-002", "FR-CLI-011"])
        self.assertEqual(contract["profile"], "trusted-local-source-v1")
        self.assertEqual(contract["limits"]["evidence_records"], 1)
        self.assertEqual(contract["limits"]["excerpt_bytes"], 262144)
        self.assertEqual(
            contract["limits"]["canonical_stdout_bytes_including_lf"], 524288
        )
        self.assertFalse(contract["new_dependency"])
        self.assertFalse(contract["migration"])
        self.assertFalse(contract["control_plane_effect"])
        self.assertFalse(contract["release_effect"])

        source_schema = load_json(SPEC / "trusted-source-excerpt-v1.schema.json")
        self.assertEqual(
            source_schema["properties"]["schema_version"]["const"],
            "codenoesis.trusted-source-excerpt/v1",
        )
        self.assertEqual(
            source_schema["properties"]["authority"]["const"],
            "explicit_local_git_object_only",
        )
        self.assertEqual(
            source_schema["properties"]["disclosure"]["const"],
            "explicit_transient_stdout",
        )
        self.assertEqual(
            source_schema["properties"]["excerpt"]["properties"]["byte_length"]["maximum"],
            262144,
        )

        error_schema = load_json(SPEC / "error-v29.schema.json")
        self.assertEqual(
            error_schema["properties"]["schema_version"]["const"],
            "codenoesis.error/v29",
        )
        self.assertEqual(
            error_schema["properties"]["context"]["maxProperties"], 0
        )

    def test_inherited_fixture_and_exact_oracle(self):
        descriptor = load_json(FIXTURE / "manifest.json")
        self.assertEqual(
            descriptor["schema_version"],
            "codenoesis.r18-fixture-descriptor/v1",
        )
        self.assertFalse(descriptor["external_source_vendored"])
        inherited = ROOT / descriptor["inherited_fixture"]
        self.assertTrue(inherited.is_file())
        source = ROOT / descriptor["source"]["path"]
        source_bytes = source.read_bytes()
        self.assertEqual(len(source_bytes), descriptor["source"]["byte_length"])
        self.assertEqual(sha256_bytes(source_bytes), descriptor["source"]["sha256"])
        self.assertTrue(all(value is False for value in descriptor["sentinels"].values()))

        oracle = load_json(FIXTURE / "expected-source-excerpt.json")
        self.assertEqual(
            oracle["schema_version"],
            "codenoesis.trusted-source-excerpt/v1",
        )
        self.assertEqual(oracle["profile"], "trusted-local-source-v1")
        self.assertEqual(oracle["authority"], "explicit_local_git_object_only")
        self.assertEqual(oracle["disclosure"], "explicit_transient_stdout")
        self.assertEqual(oracle["evidence"]["id"], descriptor["selected_evidence_id"])
        self.assertEqual(oracle["evidence"]["path"], "src/lib.rs")
        self.assertEqual(oracle["evidence"]["blob_oid"], descriptor["source"]["blob_oid"])
        self.assertEqual(oracle["evidence"]["span"]["start"], 218)
        self.assertEqual(oracle["evidence"]["span"]["end"], 316)
        self.assertEqual(oracle["evidence"]["span"]["start_position"], {
            "line": 14,
            "column": 5,
            "unit": "unicode_scalar",
        })
        self.assertEqual(oracle["evidence"]["span"]["end_position"], {
            "line": 17,
            "column": 5,
            "unit": "unicode_scalar",
        })
        excerpt = oracle["excerpt"]["text"].encode("utf-8")
        self.assertEqual(len(excerpt), 98)
        self.assertEqual(
            sha256_bytes(excerpt),
            "2beedeaf7f4333bd21ec5b33de802f1b2006377ad6435ebc983b16029fd19f83",
        )

    def test_candidate_governance_and_status_reconciliation_are_complete(self):
        decision = historical_text(DECISION.relative_to(ROOT).as_posix())
        self.assertIn("Status: Proposed branch-scoped candidate", decision)
        self.assertIn("Issue: [#190]", decision)
        self.assertIn("FR-CTX-002", decision)
        self.assertIn("FR-CLI-011", decision)
        self.assertIn("trusted-local-source-v1", decision)
        self.assertIn("Fifty argument permutations and ten process schedules", decision)
        self.assertNotIn("Status: Accepted", decision)

        for relative in [
            "README.md",
            "docs/software/architecture.md",
            "docs/software/roadmap.md",
            "docs/software/software-requirements-specification.md",
        ]:
            text = historical_text(relative)
            self.assertIn("#190", text, relative)
            self.assertIn("Decision 0038", text, relative)
            self.assertIn("trusted-local-source-v1", text, relative)
            self.assertIn("1de6a420f25a1c7eb74d07a99f1800dde90eefa8", text, relative)
            self.assertIn("G9 remains a separate governed package", text, relative)

        readme = historical_text("README.md")
        self.assertIn("32-profile", readme)
        self.assertIn("#141", readme)
        self.assertIn("closed as", readme)
        self.assertIn("superseded", readme)
        self.assertIn("candidate_verified_pending_merge", readme)

        immutable_v2_documents = {
            "docs/software/verification.md": (
                "86569b0274aa5a7f088a3007b1d9237418c505fd34ec8a1724e99ebb8ccfb754"
            ),
            "docs/software/decisions/0037-local-baseline-verification-v2.md": (
                "c149645a7a5914e956ec41ce79578cb14f563daf7bb39b393194289cfe7d9072"
            ),
        }
        for relative, expected_digest in immutable_v2_documents.items():
            self.assertEqual(
                sha256_bytes(contract_bytes(relative)), expected_digest, relative
            )

    def test_acceptance_contract_is_red_first_and_closed(self):
        e2e = load_json(SPEC / "e2e_fr_ctx_002_trusted_source_retrieval.json")
        self.assertEqual(
            e2e["id"],
            "e2e_fr_ctx_002_retrieves_exact_committed_excerpt",
        )
        self.assertEqual(e2e["status"], "Proposed branch-scoped candidate")
        self.assertEqual(e2e["exact_base"], "1de6a420f25a1c7eb74d07a99f1800dde90eefa8")
        self.assertEqual(e2e["expected_red"]["exit"], 2)
        self.assertEqual(e2e["expected_red"]["stdout_bytes"], 0)
        self.assertEqual(e2e["expected_green"]["excerpt_bytes"], 98)
        self.assertTrue(e2e["expected_green"]["loose_packed_byte_identical"])
        self.assertEqual(
            e2e["oracle"],
            "tests/fixtures/s4/trusted-source-retrieval-v1/expected-source-excerpt.json",
        )

        rust_test = (
            ROOT / "crates/noesis/tests/e2e_fr_ctx_002_trusted_source_retrieval.rs"
        ).read_text(encoding="utf-8")
        self.assertIn(
            "fn e2e_fr_ctx_002_retrieves_exact_committed_excerpt", rust_test
        )
        self.assertIn("SIGNATURE_EVIDENCE_ID", rust_test)

    def test_contract_bundle(self):
        bundle_path = SPEC / "contract-bundle.json"
        self.assertTrue(bundle_path.exists())
        bundle = load_json(bundle_path)
        paths = [record["path"] for record in bundle["files"]]
        self.assertEqual(paths, sorted(paths))
        self.assertTrue(R18_HISTORICAL_BUNDLE_PATHS.issubset(paths))
        for record in bundle["files"]:
            self.assertEqual(
                sha256_bytes(contract_bytes(record["path"])),
                record["sha256"],
                record["path"],
            )
        payload = "\n".join(
            f'{record["path"]}\0{record["sha256"]}' for record in bundle["files"]
        ).encode("utf-8")
        self.assertEqual(sha256_bytes(payload), bundle["bundle_sha256"])


if __name__ == "__main__":
    unittest.main()
