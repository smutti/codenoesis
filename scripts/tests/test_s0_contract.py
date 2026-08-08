from __future__ import annotations

import hashlib
import json
import re
import unittest
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
FIXTURE_ROOT = ROOT / "tests" / "fixtures" / "s0" / "one-file-v1"
SPEC_PATH = (
    ROOT
    / "tests"
    / "specifications"
    / "s0"
    / "e2e_fr_acq_001_immutable_commit.json"
)
SRS_PATH = ROOT / "docs" / "software" / "software-requirements-specification.md"
POLICY_PATH = ROOT / ".github" / "codex" / "policy.json"
BUNDLE_PATH = ROOT / "tests" / "specifications" / "s0" / "contract-bundle.json"

S0_REQUIREMENTS = {
    "DR-ART-001",
    "DR-ART-002",
    "FR-ACQ-001",
    "FR-CLI-003",
    "NFR-DET-001",
    "NFR-MNT-001",
    "NFR-SEC-005",
    "NFR-TST-001",
    "NFR-TST-002",
}

S0_TEST_ORDER = (
    "e2e_fr_acq_001_immutable_commit",
    "it_fr_acq_001_ref_move_after_binding_keeps_original_commit",
    "e2e_fr_acq_001_non_git_returns_typed_error",
    "e2e_fr_acq_001_missing_object_returns_typed_error",
    "e2e_fr_acq_001_inconsistent_object_returns_typed_error",
    "conf_dr_art_001_repository_snapshot_v1",
    "pt_dr_art_002_volatile_envelope_preserves_semantic_hash",
    "pt_nfr_det_001_permutation_and_schedule_invariant",
    "conf_nfr_mnt_001_dependency_rules",
    "conf_nfr_tst_001_requires_fixture_oracle_evidence_links",
    "pt_nfr_tst_002_replays_are_parallel_and_order_independent",
    "sec_nfr_sec_005_scan_launches_no_child_and_opens_no_network",
)
S0_TESTS = set(S0_TEST_ORDER)

S0_BUNDLE_FILES = {
    "LICENSE",
    "docs/software/decisions/README.md",
    "docs/software/decisions/0001-s0-walking-skeleton-contract.md",
    "scripts/tests/test_s0_contract.py",
    "tests/fixtures/s0/one-file-v1/README.md",
    "tests/fixtures/s0/one-file-v1/commit-a/main.rs",
    "tests/fixtures/s0/one-file-v1/commit-b/main.rs",
    "tests/fixtures/s0/one-file-v1/expected-error-not-git.json",
    "tests/fixtures/s0/one-file-v1/expected-semantic-a.jcs",
    "tests/fixtures/s0/one-file-v1/expected-semantic-a.json",
    "tests/fixtures/s0/one-file-v1/expected-snapshot-a.jcs",
    "tests/fixtures/s0/one-file-v1/expected-snapshot-a.json",
    "tests/fixtures/s0/one-file-v1/manifest.json",
    "tests/specifications/s0/codenoesis-error-v1.schema.json",
    "tests/specifications/s0/e2e_fr_acq_001_immutable_commit.json",
    "tests/specifications/s0/evidence-manifest-v1.schema.json",
    "tests/specifications/s0/repository-snapshot-v1.schema.json",
    "tests/specifications/s0/seccomp-capability-deny-v1.json",
}

BLAKE3_IV = (
    0x6A09E667,
    0xBB67AE85,
    0x3C6EF372,
    0xA54FF53A,
    0x510E527F,
    0x9B05688C,
    0x1F83D9AB,
    0x5BE0CD19,
)
BLAKE3_MESSAGE_PERMUTATION = (2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8)
BLAKE3_CHUNK_START = 1
BLAKE3_CHUNK_END = 2
BLAKE3_ROOT = 8


def load_json(path: Path) -> dict[str, Any]:
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def git_oid(kind: str, content: bytes) -> str:
    header = f"{kind} {len(content)}\0".encode()
    return hashlib.sha1(header + content).hexdigest()  # noqa: S324 - Git SHA-1 fixture identity


def rotate_right(value: int, count: int) -> int:
    return ((value >> count) | (value << (32 - count))) & 0xFFFFFFFF


def blake3_mix(
    state: list[int], a: int, b: int, c: int, d: int, first: int, second: int
) -> None:
    state[a] = (state[a] + state[b] + first) & 0xFFFFFFFF
    state[d] = rotate_right(state[d] ^ state[a], 16)
    state[c] = (state[c] + state[d]) & 0xFFFFFFFF
    state[b] = rotate_right(state[b] ^ state[c], 12)
    state[a] = (state[a] + state[b] + second) & 0xFFFFFFFF
    state[d] = rotate_right(state[d] ^ state[a], 8)
    state[c] = (state[c] + state[d]) & 0xFFFFFFFF
    state[b] = rotate_right(state[b] ^ state[c], 7)


def blake3_compress(
    chaining_value: list[int],
    block_words: list[int],
    counter: int,
    block_length: int,
    flags: int,
) -> list[int]:
    state = chaining_value + list(BLAKE3_IV[:4]) + [
        counter & 0xFFFFFFFF,
        counter >> 32,
        block_length,
        flags,
    ]
    message = block_words.copy()
    for _ in range(7):
        blake3_mix(state, 0, 4, 8, 12, message[0], message[1])
        blake3_mix(state, 1, 5, 9, 13, message[2], message[3])
        blake3_mix(state, 2, 6, 10, 14, message[4], message[5])
        blake3_mix(state, 3, 7, 11, 15, message[6], message[7])
        blake3_mix(state, 0, 5, 10, 15, message[8], message[9])
        blake3_mix(state, 1, 6, 11, 12, message[10], message[11])
        blake3_mix(state, 2, 7, 8, 13, message[12], message[13])
        blake3_mix(state, 3, 4, 9, 14, message[14], message[15])
        message = [message[index] for index in BLAKE3_MESSAGE_PERMUTATION]

    return [state[index] ^ state[index + 8] for index in range(8)] + [
        state[index + 8] ^ chaining_value[index] for index in range(8)
    ]


def blake3_256(content: bytes) -> str:
    """Independent single-chunk BLAKE3 oracle for S0 inputs below 1024 bytes."""
    if len(content) > 1024:
        raise ValueError("S0 oracle input exceeds the independent single-chunk limit")

    blocks = [content[offset : offset + 64] for offset in range(0, len(content), 64)]
    if not blocks:
        blocks = [b""]
    chaining_value = list(BLAKE3_IV)

    for index, block in enumerate(blocks[:-1]):
        words = list(
            int.from_bytes(block[offset : offset + 4], "little")
            for offset in range(0, 64, 4)
        )
        flags = BLAKE3_CHUNK_START if index == 0 else 0
        chaining_value = blake3_compress(
            chaining_value, words, 0, len(block), flags
        )[:8]

    final_block = blocks[-1]
    padded = final_block + b"\0" * (64 - len(final_block))
    final_words = [
        int.from_bytes(padded[offset : offset + 4], "little")
        for offset in range(0, 64, 4)
    ]
    final_flags = BLAKE3_CHUNK_END
    if len(blocks) == 1:
        final_flags |= BLAKE3_CHUNK_START
    root_words = blake3_compress(
        chaining_value,
        final_words,
        0,
        len(final_block),
        final_flags | BLAKE3_ROOT,
    )
    return b"".join(word.to_bytes(4, "little") for word in root_words)[:32].hex()


def canonical_json(value: Any) -> bytes:
    return json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    ).encode("utf-8")


class S0ContractTests(unittest.TestCase):
    def test_contract_bundle_binds_every_ratification_artifact_to_the_srs(self) -> None:
        manifest = load_json(BUNDLE_PATH)
        self.assertEqual(set(manifest), {"schema_version", "files", "bundle_sha256"})
        self.assertEqual(
            manifest["schema_version"], "codenoesis.contract-bundle/v1"
        )
        self.assertRegex(manifest["bundle_sha256"], r"^[0-9a-f]{64}$")
        files = manifest["files"]
        paths = [entry["path"] for entry in files]
        self.assertEqual(paths, sorted(paths))
        self.assertEqual(set(paths), S0_BUNDLE_FILES)
        self.assertEqual(len(paths), len(set(paths)))

        for entry in files:
            self.assertEqual(set(entry), {"path", "sha256"})
            self.assertRegex(entry["sha256"], r"^[0-9a-f]{64}$")
            path = Path(entry["path"])
            self.assertFalse(path.is_absolute())
            self.assertNotIn("..", path.parts)
            content = (ROOT / path).read_bytes()
            self.assertEqual(hashlib.sha256(content).hexdigest(), entry["sha256"])

        payload = {
            "schema_version": manifest["schema_version"],
            "files": files,
        }
        bundle_sha256 = hashlib.sha256(canonical_json(payload)).hexdigest()
        self.assertEqual(bundle_sha256, manifest["bundle_sha256"])

        srs = SRS_PATH.read_text(encoding="utf-8")
        match = re.search(r"S0 contract bundle: `sha256:([0-9a-f]{64})`", srs)
        self.assertIsNotNone(match, "SRS must bind the complete S0 contract bundle")
        self.assertEqual(match.group(1), bundle_sha256)  # type: ignore[union-attr]

    def test_ratification_register_and_machine_spec_have_the_same_exact_set(
        self,
    ) -> None:
        spec = load_json(SPEC_PATH)
        self.assertEqual(set(spec["requirements"]), S0_REQUIREMENTS)
        self.assertEqual(len(spec["requirements"]), len(S0_REQUIREMENTS))

        srs = SRS_PATH.read_text(encoding="utf-8")
        register = srs.split("### 2.2 S0 ratification register", 1)[1].split(
            "### 2.3 Priority and target", 1
        )[0]
        registered_rows = re.findall(
            r"^\| `([A-Z]+-[A-Z]+-\d{3})` \| (Proposed|Approved) \| Approved \|",
            register,
            flags=re.MULTILINE,
        )
        registered = {requirement for requirement, _ in registered_rows}
        self.assertEqual(registered, S0_REQUIREMENTS)
        current_states = {state for _, state in registered_rows}
        if spec["status"] == "proposed_for_human_ratification":
            self.assertEqual(current_states, {"Proposed"})
        elif spec["status"] == "approved":
            self.assertEqual(current_states, {"Approved"})
        else:
            self.fail(f'unsupported S0 contract status: {spec["status"]}')

        ratification = spec["ratification"]
        self.assertEqual(
            ratification,
            {
                "governance_model": "single_maintainer_bootstrap",
                "product_owner_persona": "Andrea Moretti",
                "persona_is_natural_person": False,
                "accountable_github_actor": "smutti",
                "technical_approver": "smutti",
                "approval_reference": "https://github.com/smutti/codenoesis/pull/8",
                "effective_on": "protected_squash_merge_by_accountable_actor",
                "required_external_approvals": 0,
                "agent_merge_allowed": False,
            },
        )
        self.assertIn("Andrea Moretti", srs)
        self.assertIn("single-maintainer bootstrap", srs)
        self.assertNotIn("Pending protected review", register)
        decision = (ROOT / spec["decision"]).read_text(encoding="utf-8")
        self.assertIn("| Status | Accepted;", decision)
        self.assertIn("The authoring agent never merges.", decision)

        policy = load_json(POLICY_PATH)
        policy_ids = {
            requirement["id"] for requirement in policy["approved_requirements"]
        }
        if spec["status"] == "proposed_for_human_ratification":
            self.assertEqual(
                policy_ids,
                set(),
                "A Proposed contract cannot have an autonomous policy binding",
            )
        elif policy_ids:
            self.assertTrue(
                S0_REQUIREMENTS.issubset(policy_ids),
                "A non-empty policy after S0 ratification must bind the complete S0 set",
            )

    def test_acceptance_specification_is_complete_and_immutable(self) -> None:
        spec = load_json(SPEC_PATH)
        self.assertIn(spec["status"], {"proposed_for_human_ratification", "approved"})
        self.assertEqual(
            [scenario["test_name"] for scenario in spec["scenarios"]],
            list(S0_TEST_ORDER),
        )
        self.assertEqual(
            {scenario["test_name"] for scenario in spec["scenarios"]}, S0_TESTS
        )
        scenario_requirements = {
            requirement
            for scenario in spec["scenarios"]
            for requirement in scenario["requirements"]
        }
        self.assertEqual(scenario_requirements, S0_REQUIREMENTS)
        self.assertEqual(spec["budgets"]["determinism_replays"], 50)
        self.assertEqual(spec["budgets"]["autonomous_correction_rounds"], 2)
        self.assertEqual(spec["budgets"]["minimum_evidence_retention_days"], 90)
        self.assertEqual(spec["contract_constants"]["canonicalization"], "RFC8785")
        self.assertEqual(
            spec["contract_constants"]["snapshot_hash_algorithm"], "blake3-256"
        )

        observer = spec["observer_contract"]
        self.assertIn("socketcall", observer["forbidden_network_syscalls"])
        self.assertEqual(
            set(observer["forbidden_async_io_syscalls"]),
            {"io_uring_setup", "io_uring_enter", "io_uring_register"},
        )
        self.assertIn("no interface up", observer["network_namespace"])
        self.assertIn("non-socket", observer["inherited_descriptors"])

        security_policy = load_json(ROOT / spec["security_policy"])
        policy_bytes = (ROOT / spec["security_policy"]).read_bytes()
        policy_sha256 = hashlib.sha256(policy_bytes).hexdigest()
        self.assertEqual(observer["seccomp_policy_sha256"], policy_sha256)
        denied_syscalls = {
            syscall
            for rule in security_policy["rules"]
            for syscall in rule["syscalls"]
        }
        self.assertEqual(
            denied_syscalls,
            set(observer["forbidden_process_syscalls"])
            | set(observer["conditional_process_syscalls"])
            | set(observer["forbidden_network_syscalls"])
            | set(observer["forbidden_async_io_syscalls"]),
        )
        self.assertEqual(
            security_policy["self_test_contract"]["coverage"],
            "one isolated probe for every syscall named by every rule on the selected architecture",
        )

        for relative_path in (
            spec["decision"],
            spec["fixture"],
            spec["security_policy"],
        ):
            self.assertTrue((ROOT / relative_path).is_file(), relative_path)
        for group in (spec["schemas"], spec["goldens"]):
            for relative_path in group.values():
                self.assertTrue((ROOT / relative_path).is_file(), relative_path)

    def test_green_evidence_requires_every_ordered_s0_result(self) -> None:
        spec = load_json(SPEC_PATH)
        schema = load_json(ROOT / spec["schemas"]["evidence"])
        green = schema["properties"]["green"]["oneOf"][1]
        self.assertIn("test_results", green["required"])
        test_results = green["properties"]["test_results"]
        self.assertEqual(test_results["minItems"], len(S0_TEST_ORDER))
        self.assertEqual(test_results["maxItems"], len(S0_TEST_ORDER))
        self.assertFalse(test_results["items"])
        bound_names = [
            item["allOf"][1]["properties"]["test_name"]["const"]
            for item in test_results["prefixItems"]
        ]
        self.assertEqual(bound_names, list(S0_TEST_ORDER))
        passed = schema["$defs"]["passed_test_result"]
        self.assertEqual(passed["properties"]["status"]["const"], "passed")
        self.assertEqual(passed["properties"]["runner_exit_code"]["const"], 0)
        self.assertIn("result_artifact", passed["required"])

        artifact = schema["$defs"]["github_actions_artifact"]
        self.assertEqual(artifact["properties"]["provider"]["const"], "github_actions")
        self.assertEqual(
            artifact["properties"]["repository"]["const"], "smutti/codenoesis"
        )
        self.assertTrue(
            {"run_id", "artifact_id", "path", "sha256", "retention_until"}.issubset(
                artifact["required"]
            )
        )

        environment = schema["properties"]["environment"]
        self.assertEqual(
            environment["properties"]["inherited_file_descriptors"]["const"],
            [0, 1, 2],
        )
        self.assertEqual(
            environment["properties"]["inherited_socket_descriptors"]["maxItems"],
            0,
        )
        self.assertEqual(
            environment["properties"]["seccomp_profile_sha256"]["const"],
            hashlib.sha256((ROOT / spec["security_policy"]).read_bytes()).hexdigest(),
        )
        self.assertIn("seccomp_self_test_report", environment["required"])

    def test_internal_failure_has_strict_error_contract(self) -> None:
        spec = load_json(SPEC_PATH)
        error_schema = load_json(ROOT / spec["schemas"]["error"])

        self.assertEqual(
            spec["stream_contract"]["unexpected_internal_failure"],
            {
                "exit_code": 70,
                "stdout": "empty",
                "stderr": "one CodeNoesisErrorV1 JSON document followed by one LF",
            },
        )
        self.assertEqual(spec["error_codes"].get("internal.unexpected"), 70)
        self.assertEqual(
            set(error_schema["properties"]["code"]["enum"]),
            set(spec["error_codes"]),
        )
        self.assertEqual(
            set(error_schema["properties"]["stage"]["enum"]),
            {"input", "acquisition", "internal"},
        )

        internal_rules = [
            rule
            for rule in error_schema["allOf"]
            if rule["if"]["properties"]["code"].get("const")
            == "internal.unexpected"
        ]
        self.assertEqual(len(internal_rules), 1)
        internal_properties = internal_rules[0]["then"]["properties"]
        self.assertEqual(internal_properties["stage"], {"const": "internal"})
        self.assertEqual(internal_properties["context"], {"maxProperties": 0})

        decision = (ROOT / spec["decision"]).read_text(encoding="utf-8")
        self.assertIn("| `internal.unexpected` |", decision)
        self.assertIn("must not expose the underlying internal error", decision)

    def test_independent_blake3_oracle_matches_public_vectors(self) -> None:
        self.assertEqual(
            blake3_256(b""),
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262",
        )
        self.assertEqual(
            blake3_256(b"abc"),
            "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85",
        )

    def test_snapshot_and_error_goldens_are_independently_reproducible(self) -> None:
        spec = load_json(SPEC_PATH)
        semantic = load_json(ROOT / spec["goldens"]["semantic_a"])
        snapshot = load_json(ROOT / spec["goldens"]["snapshot_a"])
        not_git_error = load_json(ROOT / spec["goldens"]["not_git_error"])

        configuration_payload = {"profile": "standard-local-s0"}
        configuration_input = (
            b"codenoesis.configuration.semantic.v1\0"
            + canonical_json(configuration_payload)
        )
        self.assertEqual(
            blake3_256(configuration_input),
            semantic["configuration"]["semantic_hash"]["value"],
        )

        semantic_bytes = canonical_json(semantic)
        stored_jcs = (ROOT / spec["goldens"]["semantic_a_jcs"]).read_bytes()
        self.assertTrue(stored_jcs.endswith(b"\n"))
        self.assertNotEqual(stored_jcs, b"\n")
        self.assertEqual(stored_jcs[:-1], semantic_bytes)
        snapshot_input = (
            b"codenoesis.repository-snapshot.semantic.v1\0" + semantic_bytes
        )
        self.assertEqual(
            blake3_256(snapshot_input), snapshot["semantic_hash"]["value"]
        )
        self.assertEqual(snapshot["semantic"], semantic)
        snapshot_jcs = (ROOT / spec["goldens"]["snapshot_a_jcs"]).read_bytes()
        self.assertEqual(snapshot_jcs, canonical_json(snapshot) + b"\n")
        self.assertEqual(
            snapshot["schema_version"], "codenoesis.repository-snapshot/v1"
        )
        self.assertEqual(
            set(snapshot), {"schema_version", "semantic_hash", "semantic", "envelope"}
        )
        self.assertEqual(
            set(not_git_error),
            {"schema_version", "code", "stage", "message", "retryable", "context"},
        )
        self.assertEqual(not_git_error["code"], "acquisition.not_git_repository")
        self.assertEqual(not_git_error["context"], {})

        for relative_path in spec["schemas"].values():
            schema = load_json(ROOT / relative_path)
            self.assertEqual(
                schema["$schema"], "https://json-schema.org/draft/2020-12/schema"
            )
            self.assertFalse(schema["additionalProperties"])

    def test_fixture_sources_reproduce_reviewed_git_object_identities(self) -> None:
        manifest = load_json(FIXTURE_ROOT / "manifest.json")
        repository = manifest["repository"]
        provenance = manifest["provenance"]
        self.assertEqual(provenance["license_spdx"], "Apache-2.0")
        self.assertEqual(provenance["license_file"], "../../../../LICENSE")
        self.assertEqual(provenance["license_scope"], "entire repository")
        repository_license_path = (
            FIXTURE_ROOT / provenance["license_file"]
        ).resolve()
        self.assertEqual(repository_license_path, ROOT / "LICENSE")
        repository_license = repository_license_path.read_text(encoding="utf-8")
        self.assertIn("Apache License", repository_license)
        self.assertIn("Version 2.0, January 2004", repository_license)
        isolation = manifest["derived_variants"]["isolation"]
        self.assertFalse(isolation["committed_tree_changes"])
        self.assertEqual(
            [step["kind"] for step in isolation["post_materialization_steps"]],
            ["write_executable_file", "set_local_git_config"],
        )
        self.assertEqual(
            manifest["oracles"]["configuration_hash"],
            "4811a917bebed264f49382d65825686ad5ca506ce39bc51385e547b0c7ced1c0",
        )
        self.assertEqual(
            manifest["oracles"]["snapshot_a_hash"],
            "b673624a329f43fd84852bbdeefd66326a7fcb1c03fdb626e2de6bfedff11997",
        )
        commits_by_label: dict[str, str] = {}

        for commit in manifest["commits"]:
            source = (FIXTURE_ROOT / commit["source"]).read_bytes()
            self.assertEqual(hashlib.sha256(source).hexdigest(), commit["source_sha256"])

            blob_oid = git_oid("blob", source)
            self.assertEqual(blob_oid, commit["blob_oid"])

            tree = (
                f'{repository["file_mode"]} {repository["file_path"]}\0'.encode()
                + bytes.fromhex(blob_oid)
            )
            tree_oid = git_oid("tree", tree)
            self.assertEqual(tree_oid, commit["tree_oid"])

            headers = [f"tree {tree_oid}"]
            parent_label = commit["parent"]
            if parent_label is not None:
                headers.append(f"parent {commits_by_label[parent_label]}")
            identity = (
                f'{repository["author_name"]} <{repository["author_email"]}> '
                f'{commit["timestamp"]} {repository["timezone"]}'
            )
            headers.extend([f"author {identity}", f"committer {identity}"])
            commit_content = ("\n".join(headers) + "\n\n" + commit["message"]).encode()
            commit_oid = git_oid("commit", commit_content)
            self.assertEqual(commit_oid, commit["commit_oid"])
            commits_by_label[commit["label"]] = commit_oid

        self.assertEqual(set(commits_by_label), {"A", "B"})


if __name__ == "__main__":
    unittest.main()
