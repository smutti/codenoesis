import hashlib
import json
import pathlib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]
SPEC = ROOT / "tests/specifications/g2/local-upgrade-safety-v1"
DECISION = ROOT / "docs/software/decisions/0035-local-upgrade-safety.md"

CURRENT_BINARY_SHA256 = (
    "f82e988be32ec8a3f077e49f4034d42847adc415cd05ec82a9affec2fe25fb6b"
)
CANDIDATE_BINARY_SHA256 = (
    "9d0005fbbae37436afe941a35dd6324b015be474371f808c6016b031f24d92ad"
)
TARGETS = {
    "aarch64-apple-darwin": {
        "current_manifest": (
            "757a9253736f2712dbe60a480af4ffa6b7d3b06a39458bc0e8714be288caf7b1"
        ),
        "candidate_manifest": (
            "fb6e8f203768cc7bbdce3deaece09b46ead5d519d5ce7bed308edb31332c6b59"
        ),
        "plan_bytes": 1313,
        "plan_sha256": (
            "514d1eacf9dc61410e797c6fcf6424dd3fd5e5325d1f25db602335f9869fe23f"
        ),
        "report_bytes": 1155,
        "report_sha256": (
            "c860fc110a7bc5647ac17b7018e39917eb7d66f8cfa823e831847a675efcdbbe"
        ),
    },
    "x86_64-pc-windows-msvc": {
        "current_manifest": (
            "409e6050f606b5067ed63896ca54b9a1fbe3577fe20e253152ab79aec45486e5"
        ),
        "candidate_manifest": (
            "f234b15b8d4326005d40642e1a3cb23a5b02c5d5c9478a08146f2492945c6d64"
        ),
        "plan_bytes": 1319,
        "plan_sha256": (
            "94b95ba5627e77503459d65cb01b59cb1282a65de3b52eedcd0100256ce05bf4"
        ),
        "report_bytes": 1161,
        "report_sha256": (
            "755b5b04ee6cff6443f99d4beaec0a9b387b874498aa2bc2a99ab8f1a225b4a7"
        ),
    },
    "x86_64-unknown-linux-gnu": {
        "current_manifest": (
            "d332001a85f9ae7608cf6ec8dd4f0aad03e6c77478052bf526bca63693eb6faf"
        ),
        "candidate_manifest": (
            "5ae7ec9b0a0c3196b73ac44d25f54b5877186fbea1be2373dda668f4a9ece4a1"
        ),
        "plan_bytes": 1325,
        "plan_sha256": (
            "49a200ec38108c668be6d7147708904e0aacc6d2e4924d5d537dac92ca2b705c"
        ),
        "report_bytes": 1167,
        "report_sha256": (
            "d1ffd355343eb65abff79214e9410d707068140d814baf23a40372017981fe23"
        ),
    },
}
ERROR_CODES = [
    "compatibility.incompatible",
    "compatibility.internal",
    "compatibility.invalid_arguments",
    "compatibility.invalid_bundle",
    "compatibility.invalid_plan",
    "compatibility.limit_exceeded",
    "compatibility.unstable_input",
]
INVALID_CASES = {
    "duplicate-argument",
    "missing-argument",
    "same-bundle",
    "third-bundle-rollback",
    "rollback-without-plan",
    "plan-substitution",
    "unsupported-target",
    "unsupported-profile",
    "unsupported-manifest-schema",
    "changed-bundle-name",
    "changed-manifest",
    "changed-binary",
    "changed-fixed-payload",
    "missing-file",
    "extra-file",
    "symlink-root",
    "symlink-file",
    "symlink-plan",
    "unstable-input",
    "private-canary",
    "plan-maximum-plus-one",
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


def bundle_record(target: str, binary_sha256: str, manifest_sha256: str):
    return {
        "binary_sha256": binary_sha256,
        "bundle_name": (
            f"codenoesis-local-experimental-r17-{target}-{binary_sha256}"
        ),
        "manifest_sha256": manifest_sha256,
    }


class G2LocalUpgradeSafetyContractTest(unittest.TestCase):
    def test_exact_branch_authority_and_limits(self):
        contract = load_json(SPEC / "contract-v1.json")
        self.assertEqual(
            contract["schema_version"],
            "codenoesis.g2-local-upgrade-safety-contract/v1",
        )
        self.assertEqual(contract["issue"], 184)
        self.assertEqual(
            contract["base_sha"],
            "e7643d83965dca2f9342080264e7c6c58f3dd761",
        )
        self.assertEqual(contract["requirements"], ["FR-CMP-001", "FR-CLI-009"])
        self.assertEqual(contract["slice"], "S14")
        self.assertEqual(contract["risk"], "high")
        self.assertEqual(
            contract["contracts"],
            [
                "codenoesis.local-upgrade-plan/v1",
                "codenoesis.local-rollback-report/v1",
                "codenoesis.error/v27",
            ],
        )
        self.assertEqual(
            contract["limits"],
            {
                "bundles": 2,
                "files_per_bundle": 6,
                "plan_bytes": 65536,
                "constructions": 50,
                "schedules": 10,
                "benchmark_repetitions": 30,
            },
        )
        for field in [
            "activation",
            "automatic_update",
            "execution",
            "migration",
            "publication",
            "signing",
            "ga",
        ]:
            self.assertFalse(contract["authority"][field], field)
        self.assertEqual(contract["authority"]["support"], "none")

    def test_closed_schemas_freeze_values_and_error_surface(self):
        plan = load_json(SPEC / "local-upgrade-plan-v1.schema.json")
        rollback = load_json(SPEC / "local-rollback-report-v1.schema.json")
        error = load_json(SPEC / "codenoesis-error-v27.schema.json")
        for schema in [plan, rollback, error]:
            self.assertFalse(schema["additionalProperties"])
        for schema in [plan, rollback]:
            self.assertFalse(schema["$defs"]["bundle"]["additionalProperties"])
            self.assertEqual(
                schema["properties"]["platform_target"]["enum"],
                list(TARGETS),
            )
            self.assertEqual(schema["properties"]["profile_id"]["const"], (
                "local-experimental-r17"
            ))
            self.assertEqual(schema["properties"]["release_status"]["const"], "not-ga")
            self.assertEqual(schema["properties"]["verification"]["const"], "not-verified")
        self.assertFalse(plan["properties"]["automatic_update"]["const"])
        self.assertFalse(plan["properties"]["publication"]["const"])
        self.assertFalse(rollback["properties"]["publication"]["const"])
        self.assertEqual(error["properties"]["code"]["enum"], ERROR_CODES)
        self.assertEqual(error["properties"]["schema_version"]["const"], (
            "codenoesis.error/v27"
        ))
        self.assertFalse(error["properties"]["retryable"]["const"])

    def test_target_goldens_are_exact_canonical_transitions(self):
        for target, oracle in TARGETS.items():
            plan_path = SPEC / f"expected-upgrade-plan-{target}.json"
            report_path = SPEC / f"expected-rollback-report-{target}.json"
            plan = load_json(plan_path)
            report = load_json(report_path)
            current = bundle_record(
                target,
                CURRENT_BINARY_SHA256,
                oracle["current_manifest"],
            )
            candidate = bundle_record(
                target,
                CANDIDATE_BINARY_SHA256,
                oracle["candidate_manifest"],
            )
            expected_plan = {
                "activation": "caller-owned",
                "automatic_update": False,
                "candidate": candidate,
                "compatibility": "exact-g1a-side-by-side",
                "configuration_transition": "identical-v1-no-migration",
                "current": current,
                "downgrade": "forbidden-without-exact-plan",
                "platform_target": target,
                "profile_id": "local-experimental-r17",
                "publication": False,
                "release_status": "not-ga",
                "rollback": {
                    "mode": "exact-prior-bundle",
                    "required_current_manifest_sha256": oracle[
                        "candidate_manifest"
                    ],
                    "target_manifest_sha256": oracle["current_manifest"],
                },
                "schema_version": "codenoesis.local-upgrade-plan/v1",
                "signing": "not-available",
                "support": "none",
                "verification": "not-verified",
            }
            expected_report = {
                "activation": "caller-owned",
                "compatibility": "exact-plan-match",
                "configuration_transition": "identical-v1-no-migration",
                "current": candidate,
                "downgrade": "exact-plan-only",
                "operation": "rollback-preflight",
                "plan_sha256": oracle["plan_sha256"],
                "platform_target": target,
                "profile_id": "local-experimental-r17",
                "publication": False,
                "release_status": "not-ga",
                "schema_version": "codenoesis.local-rollback-report/v1",
                "signing": "not-available",
                "support": "none",
                "target_bundle": current,
                "verification": "not-verified",
            }
            self.assertEqual(plan, expected_plan, target)
            self.assertEqual(report, expected_report, target)
            self.assertEqual(plan_path.read_bytes(), canonical_json(expected_plan), target)
            self.assertEqual(report_path.read_bytes(), canonical_json(expected_report), target)
            self.assertEqual(plan_path.stat().st_size, oracle["plan_bytes"], target)
            self.assertEqual(report_path.stat().st_size, oracle["report_bytes"], target)
            self.assertEqual(sha256(plan_path), oracle["plan_sha256"], target)
            self.assertEqual(sha256(report_path), oracle["report_sha256"], target)

    def test_public_journey_red_and_invalid_matrix_are_frozen(self):
        journey = load_json(SPEC / "e2e_fr_cmp_001_local_upgrade.json")
        self.assertEqual(journey["requirement_ids"], ["FR-CMP-001", "FR-CLI-009"])
        self.assertEqual(
            journey["upgrade_command"],
            [
                "xtask",
                "preflight-local-upgrade",
                "--current",
                "<bundle-a>",
                "--candidate",
                "<bundle-b>",
            ],
        )
        self.assertEqual(
            journey["rollback_command"],
            [
                "xtask",
                "preflight-local-rollback",
                "--plan",
                "<plan>",
                "--current",
                "<bundle-b>",
                "--target",
                "<bundle-a>",
            ],
        )
        self.assertEqual(journey["determinism"], {
            "constructions": 50,
            "schedules": 10,
        })
        self.assertEqual(set(load_json(SPEC / "invalid-cases-v1.json")["cases"]), (
            INVALID_CASES
        ))
        expected_red = load_json(SPEC / "contract-v1.json")["expected_red"]
        self.assertEqual(
            expected_red,
            {
                "exit": 2,
                "stdout_bytes": 0,
                "stderr_bytes": 170,
                "stderr_sha256": (
                    "cd5f646ce966c60887ae0ed110142ba22c4a5f6a05a792c72bfbab5ba3a94311"
                ),
                "schema_version": "codenoesis.error/v26",
                "code": "distribution.invalid_arguments",
            },
        )

    def test_governance_records_proposed_output_only_candidate(self):
        decision = DECISION.read_text(encoding="utf-8")
        normalized_decision = " ".join(decision.split())
        for statement in [
            "Status: Proposed branch-scoped candidate",
            "Issue: [#184]",
            "Exact base: `e7643d83965dca2f9342080264e7c6c58f3dd761`",
            "FR-CMP-001",
            "FR-CLI-009",
            "Slice: `S14`",
            "Risk: high",
            "Fifty argument constructions and ten schedules",
        ]:
            self.assertIn(statement, normalized_decision)
        self.assertNotIn("Status: Accepted", decision)
        self.assertNotIn("G2a is Verified", decision)

        for relative in [
            "README.md",
            "docs/software/architecture.md",
            "docs/software/distribution.md",
            "docs/software/roadmap.md",
            "docs/software/software-requirements-specification.md",
            "docs/software/threat-model.md",
        ]:
            text = (ROOT / relative).read_text(encoding="utf-8")
            self.assertIn("#184", text, relative)
            self.assertIn("Decision 0035", text, relative)
            self.assertIn("FR-CMP-001", text, relative)
            self.assertIn("FR-CLI-009", text, relative)
            self.assertNotIn("G2a is Verified", text, relative)

    def test_contract_bundle_is_complete_and_self_excluding(self):
        bundle_path = SPEC / "contract-bundle.json"
        self.assertTrue(bundle_path.exists())
        bundle = load_json(bundle_path)
        self.assertEqual(bundle["schema_version"], "codenoesis.contract-bundle/v1")
        paths = [record["path"] for record in bundle["files"]]
        self.assertEqual(paths, sorted(paths))
        self.assertNotIn(
            "tests/specifications/g2/local-upgrade-safety-v1/contract-bundle.json",
            paths,
        )
        for record in bundle["files"]:
            self.assertEqual(sha256(ROOT / record["path"]), record["sha256"])
        payload = "\n".join(
            f'{record["path"]}\0{record["sha256"]}' for record in bundle["files"]
        ).encode("utf-8")
        self.assertEqual(sha256_bytes(payload), bundle["bundle_sha256"])


if __name__ == "__main__":
    unittest.main()
