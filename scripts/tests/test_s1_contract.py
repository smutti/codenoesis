from __future__ import annotations

import hashlib
import json
import re
import unittest
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
FIXTURE_ROOT = ROOT / "tests" / "fixtures" / "s1" / "safe-inventory-v1"
SOURCE_ROOT = FIXTURE_ROOT / "revision-a"
SPEC_PATH = (
    ROOT
    / "tests"
    / "specifications"
    / "s1"
    / "e2e_fr_inv_001_safe_inventory.json"
)
SRS_PATH = ROOT / "docs" / "software" / "software-requirements-specification.md"
BUNDLE_PATH = ROOT / "tests" / "specifications" / "s1" / "contract-bundle.json"

S1_REQUIREMENTS = {
    "DR-EVD-001",
    "FR-ACQ-002",
    "FR-INV-001",
    "NFR-SEC-001",
}

S1_TEST_ORDER = (
    "e2e_fr_inv_001_safe_inventory",
    "e2e_fr_acq_002_traversal_rejected",
    "e2e_fr_acq_002_symlink_rejected",
    "e2e_fr_acq_002_gitlink_rejected",
    "pt_fr_acq_002_limits_have_max_and_plus_one",
    "conf_dr_evd_001_source_evidence_resolves",
    "gt_fr_inv_001_exact_inventory_and_coverage_gaps",
    "pt_fr_inv_001_inventory_is_order_invariant",
    "sec_nfr_sec_001_scan_stays_inside_repository_root",
    "sec_nfr_sec_001_sentinel_scripts_never_execute",
    "sec_nfr_sec_001_bombs_and_parser_inputs_are_bounded",
    "conf_fr_inv_001_repository_snapshot_v2_and_error_v2",
)

INHERITED_S0_TESTS = {
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
}

LIMITS = {
    "regular_files": 20_000,
    "tree_entries": 25_000,
    "cumulative_file_bytes": 268_435_456,
    "single_file_bytes": 4_194_304,
    "path_bytes": 1_024,
    "path_component_bytes": 255,
    "recursion_depth": 32,
    "canonical_output_bytes": 33_554_432,
    "scan_wall_milliseconds": 60_000,
}

S1_BUNDLE_FILES = {
    "LICENSE",
    "docs/software/decisions/0002-s1-safe-inventory-contract.md",
    "scripts/tests/test_s1_contract.py",
    "tests/fixtures/s1/safe-inventory-v1/README.md",
    "tests/fixtures/s1/safe-inventory-v1/expected-error-file-limit.json",
    "tests/fixtures/s1/safe-inventory-v1/expected-error-symlink.json",
    "tests/fixtures/s1/safe-inventory-v1/expected-semantic-a.jcs",
    "tests/fixtures/s1/safe-inventory-v1/expected-semantic-a.json",
    "tests/fixtures/s1/safe-inventory-v1/expected-snapshot-a.jcs",
    "tests/fixtures/s1/safe-inventory-v1/expected-snapshot-a.json",
    "tests/fixtures/s1/safe-inventory-v1/manifest.json",
    "tests/fixtures/s1/safe-inventory-v1/revision-a/.github/CODEOWNERS",
    "tests/fixtures/s1/safe-inventory-v1/revision-a/Cargo.toml",
    "tests/fixtures/s1/safe-inventory-v1/revision-a/README.md",
    "tests/fixtures/s1/safe-inventory-v1/revision-a/api/openapi.yaml",
    "tests/fixtures/s1/safe-inventory-v1/revision-a/assets/payload.bin",
    "tests/fixtures/s1/safe-inventory-v1/revision-a/build.rs",
    "tests/fixtures/s1/safe-inventory-v1/revision-a/rustfmt.toml",
    "tests/fixtures/s1/safe-inventory-v1/revision-a/src/lib.rs",
    "tests/fixtures/s1/safe-inventory-v1/revision-a/tools/sentinel.sh",
    "tests/specifications/s0/seccomp-capability-deny-v1.json",
    "tests/specifications/s1/codenoesis-error-v2.schema.json",
    "tests/specifications/s1/e2e_fr_inv_001_safe_inventory.json",
    "tests/specifications/s1/repository-snapshot-v2.schema.json",
    "tests/specifications/s1/source-evidence-v1.schema.json",
}

EXPECTED_CLASSIFICATIONS = {
    ".github/CODEOWNERS": {
        "content_kind": "text_utf8",
        "roles": ["ownership"],
        "languages": [],
        "rules": ["git-tree-entry", "ownership:github-codeowners"],
    },
    "Cargo.toml": {
        "content_kind": "text_utf8",
        "roles": ["manifest"],
        "languages": [],
        "rules": ["git-tree-entry", "manifest:cargo"],
    },
    "README.md": {
        "content_kind": "text_utf8",
        "roles": ["documentation"],
        "languages": [],
        "rules": ["documentation:readme", "git-tree-entry"],
    },
    "api/openapi.yaml": {
        "content_kind": "text_utf8",
        "roles": ["contract"],
        "languages": [],
        "rules": ["contract:openapi", "git-tree-entry"],
    },
    "assets/payload.bin": {
        "content_kind": "binary_or_unknown",
        "roles": ["unsupported"],
        "languages": [],
        "rules": ["git-tree-entry", "unsupported:fallback"],
    },
    "build.rs": {
        "content_kind": "text_utf8",
        "roles": ["sentinel", "source"],
        "languages": ["rust"],
        "rules": ["git-tree-entry", "language:rust", "sentinel:build-script"],
    },
    "rustfmt.toml": {
        "content_kind": "text_utf8",
        "roles": ["configuration"],
        "languages": [],
        "rules": ["configuration:rustfmt", "git-tree-entry"],
    },
    "src/lib.rs": {
        "content_kind": "text_utf8",
        "roles": ["source"],
        "languages": ["rust"],
        "rules": ["git-tree-entry", "language:rust"],
    },
    "tools/sentinel.sh": {
        "content_kind": "text_utf8",
        "roles": ["sentinel", "source"],
        "languages": ["shell"],
        "rules": [
            "git-tree-entry",
            "language:shell",
            "sentinel:executable-script",
        ],
    },
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
BLAKE3_PARENT = 4
BLAKE3_ROOT = 8


def reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        if key in value:
            raise ValueError(f"duplicate JSON key {key!r}")
        value[key] = item
    return value


def load_json(path: Path) -> dict[str, Any]:
    with path.open(encoding="utf-8") as handle:
        return json.load(handle, object_pairs_hook=reject_duplicate_keys)


def canonical_json(value: Any) -> bytes:
    return json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    ).encode("utf-8")


def git_oid(kind: str, content: bytes) -> str:
    header = f"{kind} {len(content)}\0".encode()
    return hashlib.sha1(header + content).hexdigest()  # noqa: S324 - Git fixture


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


def block_words(block: bytes) -> list[int]:
    padded = block + b"\0" * (64 - len(block))
    return [
        int.from_bytes(padded[offset : offset + 4], "little")
        for offset in range(0, 64, 4)
    ]


def chunk_output(
    chunk: bytes, chunk_counter: int
) -> tuple[list[int], list[int], int, int, int]:
    blocks = [chunk[offset : offset + 64] for offset in range(0, len(chunk), 64)]
    if not blocks:
        blocks = [b""]
    chaining_value = list(BLAKE3_IV)
    for index, block in enumerate(blocks[:-1]):
        flags = BLAKE3_CHUNK_START if index == 0 else 0
        chaining_value = blake3_compress(
            chaining_value,
            block_words(block),
            chunk_counter,
            len(block),
            flags,
        )[:8]
    final_flags = BLAKE3_CHUNK_END
    if len(blocks) == 1:
        final_flags |= BLAKE3_CHUNK_START
    final_block = blocks[-1]
    return (
        chaining_value,
        block_words(final_block),
        chunk_counter,
        len(final_block),
        final_flags,
    )


def output_chaining_value(
    output: tuple[list[int], list[int], int, int, int]
) -> list[int]:
    chaining_value, words, counter, block_length, flags = output
    return blake3_compress(
        chaining_value, words, counter, block_length, flags
    )[:8]


def parent_output(
    left: list[int], right: list[int]
) -> tuple[list[int], list[int], int, int, int]:
    return (list(BLAKE3_IV), left + right, 0, 64, BLAKE3_PARENT)


def blake3_256(content: bytes) -> str:
    chunks = [
        content[offset : offset + 1024] for offset in range(0, len(content), 1024)
    ]
    if not chunks:
        chunks = [b""]
    stack: list[list[int]] = []
    for index, chunk in enumerate(chunks[:-1]):
        chaining_value = output_chaining_value(chunk_output(chunk, index))
        total_chunks = index + 1
        while total_chunks & 1 == 0:
            chaining_value = output_chaining_value(
                parent_output(stack.pop(), chaining_value)
            )
            total_chunks >>= 1
        stack.append(chaining_value)
    output = chunk_output(chunks[-1], len(chunks) - 1)
    while stack:
        output = parent_output(stack.pop(), output_chaining_value(output))
    chaining_value, words, _, block_length, flags = output
    root_words = blake3_compress(
        chaining_value, words, 0, block_length, flags | BLAKE3_ROOT
    )
    return b"".join(word.to_bytes(4, "little") for word in root_words)[:32].hex()


def tree_sort_key(mode: str, name: str) -> bytes:
    suffix = b"/" if mode == "40000" else b""
    return name.encode("utf-8") + suffix


def tree_oid(entries: list[tuple[str, str, str]]) -> tuple[str, int]:
    ordered = sorted(entries, key=lambda entry: tree_sort_key(entry[0], entry[1]))
    payload = b"".join(
        f"{mode} {name}\0".encode() + bytes.fromhex(object_id)
        for mode, name, object_id in ordered
    )
    return git_oid("tree", payload), len(payload)


def commit_oid(tree: str, timestamp: int, message: str) -> tuple[str, bytes]:
    identity = (
        f"CodeNoesis Fixture <fixture@codenoesis.invalid> {timestamp} +0000"
    )
    payload = (
        f"tree {tree}\nauthor {identity}\ncommitter {identity}\n\n{message}"
    ).encode()
    return git_oid("commit", payload), payload


class S1ContractTests(unittest.TestCase):
    def test_contract_bundle_binds_every_s1_ratification_artifact(self) -> None:
        manifest = load_json(BUNDLE_PATH)
        self.assertEqual(set(manifest), {"schema_version", "files", "bundle_sha256"})
        self.assertEqual(
            manifest["schema_version"], "codenoesis.contract-bundle/v1"
        )
        files = manifest["files"]
        paths = [entry["path"] for entry in files]
        self.assertEqual(paths, sorted(paths))
        self.assertEqual(set(paths), S1_BUNDLE_FILES)
        self.assertEqual(len(paths), len(set(paths)))
        for entry in files:
            self.assertEqual(set(entry), {"path", "sha256"})
            self.assertRegex(entry["sha256"], r"^[0-9a-f]{64}$")
            path = Path(entry["path"])
            self.assertFalse(path.is_absolute())
            self.assertNotIn("..", path.parts)
            self.assertEqual(
                hashlib.sha256((ROOT / path).read_bytes()).hexdigest(),
                entry["sha256"],
            )
        payload = {
            "schema_version": manifest["schema_version"],
            "files": files,
        }
        bundle_sha256 = hashlib.sha256(canonical_json(payload)).hexdigest()
        self.assertEqual(manifest["bundle_sha256"], bundle_sha256)
        srs = SRS_PATH.read_text(encoding="utf-8")
        match = re.search(r"S1 contract bundle: `sha256:([0-9a-f]{64})`", srs)
        self.assertIsNotNone(match, "SRS must bind the complete S1 contract bundle")
        self.assertEqual(match.group(1), bundle_sha256)  # type: ignore[union-attr]

    def test_s1_register_oracle_and_ratification_are_exact(self) -> None:
        spec = load_json(SPEC_PATH)
        self.assertEqual(spec["status"], "approved")
        self.assertEqual(set(spec["requirements"]), S1_REQUIREMENTS)
        self.assertEqual(len(spec["requirements"]), len(S1_REQUIREMENTS))
        self.assertEqual(
            spec["ratification"],
            {
                "governance_model": "single_maintainer_bootstrap",
                "product_owner_persona": "Andrea Moretti",
                "persona_is_natural_person": False,
                "accountable_github_actor": "smutti",
                "technical_approver": "smutti",
                "approval_reference": "https://github.com/smutti/codenoesis/issues/16",
                "effective_on": "protected_squash_merge_by_accountable_actor",
                "required_external_approvals": 0,
                "agent_merge_allowed": False,
            },
        )
        srs = SRS_PATH.read_text(encoding="utf-8")
        register = srs.split("### 2.3.1 S1 ratification register", 1)[1].split(
            "### 2.4 Verification classes", 1
        )[0]
        registered = re.findall(
            r"^\| `([A-Z]+-[A-Z]+-\d{3})` \| Approved \| Approved \|",
            register,
            flags=re.MULTILINE,
        )
        self.assertEqual(set(registered), S1_REQUIREMENTS)
        self.assertEqual(len(registered), len(S1_REQUIREMENTS))
        decision = (ROOT / spec["decision"]).read_text(encoding="utf-8")
        self.assertIn("| Status | Accepted;", decision)
        self.assertIn("authoring agent must not approve or merge", decision)
        self.assertIn("separate policy-binding change", decision)

    def test_acceptance_specification_has_complete_ordered_traceability(self) -> None:
        spec = load_json(SPEC_PATH)
        self.assertEqual(
            [scenario["test_name"] for scenario in spec["scenarios"]],
            list(S1_TEST_ORDER),
        )
        scenario_requirements = {
            requirement
            for scenario in spec["scenarios"]
            for requirement in scenario["requirements"]
        }
        self.assertEqual(scenario_requirements, S1_REQUIREMENTS)
        self.assertEqual(
            set(spec["inherited_s0_regressions"]), INHERITED_S0_TESTS
        )
        self.assertEqual(
            spec["contract_constants"]["snapshot_hash_domain"],
            "codenoesis.repository-snapshot.semantic.v2",
        )
        self.assertEqual(
            spec["compatibility_contract"]["forbidden_dispatch"],
            [
                "repository shape",
                "file count",
                "file extension",
                "environment variable",
                "implicit configuration",
            ],
        )
        self.assertEqual(
            spec["expected_red"],
            {
                "test_command": "cargo test --test e2e_fr_inv_001_safe_inventory",
                "precondition": "The future TDD branch contains the black-box S1 harness and the reviewed fixture, while production remains at the merged S0 behavior.",
                "runner_expected_exit": "nonzero because the acceptance assertion fails",
                "subject_observed_exit_code": 2,
                "subject_observed_stderr_schema": "codenoesis.error/v1",
                "subject_observed_stderr_code": "input.invalid_revision",
                "subject_expected_exit_code": 0,
                "expected_artifact": "codenoesis.repository-snapshot/v2",
                "accepted_reason": "The S0 CLI does not recognize --profile standard-local-s1, so the approved S1 public behavior is absent.",
                "rejected_reasons": [
                    "compilation failure",
                    "missing test target",
                    "missing or corrupt fixture",
                    "network or dependency outage",
                    "timing race",
                    "unexpected panic",
                    "a modified S1 oracle",
                ],
            },
        )
        for relative_path in (spec["decision"], spec["fixture"]):
            self.assertTrue((ROOT / relative_path).is_file(), relative_path)
        for group in (spec["schemas"], spec["goldens"]):
            for relative_path in group.values():
                self.assertTrue((ROOT / relative_path).is_file(), relative_path)

    def test_limits_error_catalog_and_security_contract_are_closed(self) -> None:
        spec = load_json(SPEC_PATH)
        self.assertEqual(
            spec["limit_profile"]["enforced_input_limits"], LIMITS
        )
        evidence_budgets = spec["limit_profile"]["evidence_budgets"]
        self.assertEqual(evidence_budgets["single_scan_cpu_milliseconds"], 30_000)
        self.assertEqual(evidence_budgets["peak_resident_memory_bytes"], 536_870_912)
        self.assertEqual(evidence_budgets["temporary_disk_bytes"], 67_108_864)
        self.assertEqual(evidence_budgets["acceptance_suite_seconds"], 180)
        self.assertEqual(evidence_budgets["determinism_replays"], 50)
        self.assertEqual(evidence_budgets["parallel_shuffled_repetitions"], 10)
        self.assertEqual(evidence_budgets["minimum_evidence_retention_days"], 90)
        self.assertEqual(evidence_budgets["autonomous_correction_rounds"], 2)

        fixture = load_json(FIXTURE_ROOT / "manifest.json")
        limit_cases = fixture["limit_cases"]
        self.assertEqual(
            {case["limit"]: case["maximum"] for case in limit_cases}, LIMITS
        )
        for case in limit_cases:
            self.assertEqual(set(case), {"limit", "maximum", "max_recipe", "plus_one_recipe"})
            self.assertTrue(case["max_recipe"])
            self.assertTrue(case["plus_one_recipe"])

        error_schema = load_json(
            ROOT / spec["schemas"]["error"]
        )
        self.assertEqual(
            set(error_schema["properties"]["code"]["enum"]),
            set(spec["error_codes"]),
        )
        self.assertEqual(
            set(error_schema["properties"]["context"]["properties"]["limit"]["enum"]),
            set(LIMITS),
        )
        conditioned_codes: set[str] = set()
        for rule in error_schema["allOf"]:
            code_rule = rule["if"]["properties"]["code"]
            if "const" in code_rule:
                conditioned_codes.add(code_rule["const"])
            else:
                conditioned_codes.update(code_rule["enum"])
        self.assertEqual(conditioned_codes, set(spec["error_codes"]))

        filesystem = spec["security_contract"]["filesystem"]
        self.assertIn("Landlock", filesystem["linux_enforcement"])
        self.assertIn("openat2", filesystem["linux_observation"])
        self.assertIn("proc-fd", filesystem["self_test"])
        self.assertIn("unresolved path event", filesystem["failure_rule"])
        self.assertEqual(
            spec["security_contract"]["inherits_s0_observer"],
            "tests/specifications/s0/seccomp-capability-deny-v1.json",
        )

    def test_schemas_are_strict_versioned_and_link_source_evidence(self) -> None:
        spec = load_json(SPEC_PATH)
        schemas = {
            name: load_json(ROOT / path) for name, path in spec["schemas"].items()
        }
        for schema in schemas.values():
            self.assertEqual(
                schema["$schema"], "https://json-schema.org/draft/2020-12/schema"
            )
            self.assertFalse(schema["additionalProperties"])
        snapshot = schemas["snapshot"]
        self.assertEqual(
            snapshot["properties"]["schema_version"]["const"],
            "codenoesis.repository-snapshot/v2",
        )
        inventory = snapshot["$defs"]["inventory"]
        self.assertEqual(
            inventory["properties"]["evidence"]["items"]["$ref"],
            "urn:codenoesis:schema:source-evidence:v1",
        )
        self.assertEqual(
            inventory["properties"]["files"]["maxItems"], LIMITS["regular_files"]
        )
        source_evidence = schemas["source_evidence"]
        self.assertEqual(
            source_evidence["properties"]["schema_version"]["const"],
            "codenoesis.source-evidence/v1",
        )
        self.assertEqual(
            source_evidence["properties"]["span"]["properties"]["end"]["maximum"],
            LIMITS["single_file_bytes"],
        )
        self.assertEqual(
            schemas["error"]["properties"]["schema_version"]["const"],
            "codenoesis.error/v2",
        )

    def test_independent_blake3_handles_multi_chunk_inputs(self) -> None:
        self.assertEqual(
            blake3_256(b""),
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262",
        )
        self.assertEqual(
            blake3_256(b"abc"),
            "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85",
        )
        vector = bytes(index % 251 for index in range(2_049))
        self.assertEqual(
            blake3_256(vector),
            "5f4d72f40d7a5f82b15ca2b2e44b1de3c2ef86c426c95c1af0b6879522563030",
        )

    def test_snapshot_goldens_are_canonical_and_reproducible(self) -> None:
        spec = load_json(SPEC_PATH)
        fixture = load_json(FIXTURE_ROOT / "manifest.json")
        semantic = load_json(ROOT / spec["goldens"]["semantic_a"])
        snapshot = load_json(ROOT / spec["goldens"]["snapshot_a"])

        configuration_payload = {"profile": "standard-local-s1"}
        configuration_hash = blake3_256(
            b"codenoesis.configuration.semantic.v1\0"
            + canonical_json(configuration_payload)
        )
        self.assertEqual(
            configuration_hash,
            semantic["configuration"]["semantic_hash"]["value"],
        )
        self.assertEqual(configuration_hash, fixture["oracles"]["configuration_hash"])

        semantic_bytes = canonical_json(semantic)
        semantic_jcs = (ROOT / spec["goldens"]["semantic_a_jcs"]).read_bytes()
        self.assertEqual(semantic_jcs, semantic_bytes + b"\n")
        snapshot_hash = blake3_256(
            b"codenoesis.repository-snapshot.semantic.v2\0" + semantic_bytes
        )
        self.assertEqual(snapshot_hash, snapshot["semantic_hash"]["value"])
        self.assertEqual(snapshot_hash, fixture["oracles"]["snapshot_a_hash"])
        self.assertEqual(snapshot["semantic"], semantic)
        snapshot_jcs = (ROOT / spec["goldens"]["snapshot_a_jcs"]).read_bytes()
        self.assertEqual(snapshot_jcs, canonical_json(snapshot) + b"\n")
        self.assertLessEqual(
            len(snapshot_jcs), LIMITS["canonical_output_bytes"]
        )

    def test_fixture_sources_reproduce_git_revision_and_tree_closure(self) -> None:
        manifest = load_json(FIXTURE_ROOT / "manifest.json")
        provenance = manifest["provenance"]
        self.assertEqual(provenance["license_spdx"], "Apache-2.0")
        self.assertFalse(provenance["third_party_material"])
        self.assertEqual(
            (FIXTURE_ROOT / provenance["license_file"]).resolve(),
            ROOT / "LICENSE",
        )

        revision = manifest["revision"]
        files = revision["files"]
        self.assertEqual(
            [entry["path"] for entry in files],
            sorted(entry["path"] for entry in files),
        )
        modes = {entry["path"]: entry["mode"] for entry in files}
        expected_files = {entry["path"]: entry for entry in files}
        for path, entry in expected_files.items():
            content = (SOURCE_ROOT / path).read_bytes()
            self.assertEqual(len(content), entry["byte_length"])
            self.assertEqual(hashlib.sha256(content).hexdigest(), entry["source_sha256"])
            self.assertEqual(git_oid("blob", content), entry["blob_oid"])

        calculated_trees: dict[str, dict[str, Any]] = {}

        def calculate_tree(directory: Path, relative: str = "") -> str:
            entries: list[tuple[str, str, str]] = []
            for child in directory.iterdir():
                child_relative = (
                    f"{relative}/{child.name}" if relative else child.name
                )
                if child.is_dir():
                    object_id = calculate_tree(child, child_relative)
                    mode = "40000"
                else:
                    object_id = expected_files[child_relative]["blob_oid"]
                    mode = modes[child_relative]
                entries.append((mode, child.name, object_id))
            object_id, payload_bytes = tree_oid(entries)
            calculated_trees[relative] = {
                "path": relative,
                "entry_count": len(entries),
                "payload_bytes": payload_bytes,
                "tree_oid": object_id,
            }
            return object_id

        root_tree = calculate_tree(SOURCE_ROOT)
        self.assertEqual(root_tree, revision["tree_oid"])
        reviewed_trees = {entry["path"]: entry for entry in revision["trees"]}
        self.assertEqual(calculated_trees, reviewed_trees)

        repository = manifest["repository"]
        calculated_commit, payload = commit_oid(
            root_tree, repository["timestamp"], repository["message"]
        )
        self.assertEqual(calculated_commit, revision["commit_oid"])
        self.assertEqual(
            hashlib.sha256(payload).hexdigest(),
            revision["commit_payload_sha256"],
        )

    def test_generated_variant_object_identities_are_reproducible(self) -> None:
        manifest = load_json(FIXTURE_ROOT / "manifest.json")
        revision = manifest["revision"]
        root_tree = next(tree for tree in revision["trees"] if tree["path"] == "")
        directory_oids = {
            tree["path"]: tree["tree_oid"]
            for tree in revision["trees"]
            if tree["path"]
        }
        root_files = [
            (entry["mode"], entry["path"], entry["blob_oid"])
            for entry in revision["files"]
            if "/" not in entry["path"]
        ]
        base_entries = root_files + [
            ("40000", path, object_id)
            for path, object_id in directory_oids.items()
        ]
        self.assertEqual(tree_oid(base_entries)[0], root_tree["tree_oid"])

        variants = manifest["derived_variants"]
        for label in ("traversal", "symlink_escape", "symlink_loop"):
            variant = variants[label]
            entry = variant["entry"]
            content = (
                entry.get("content_utf8") or entry.get("target_utf8")
            ).encode()
            object_id = git_oid("blob", content)
            self.assertEqual(object_id, entry["blob_oid"])
            calculated_tree, _ = tree_oid(
                base_entries
                + [(entry["mode"], entry["name_utf8"], entry["blob_oid"])]
            )
            self.assertEqual(calculated_tree, variant["tree_oid"])
            calculated_commit, _ = commit_oid(
                calculated_tree, variant["timestamp"], variant["message"]
            )
            self.assertEqual(calculated_commit, variant["commit_oid"])

        gitlink = variants["gitlink"]
        entry = gitlink["entry"]
        calculated_tree, _ = tree_oid(
            base_entries
            + [(entry["mode"], entry["name_utf8"], entry["commit_oid"])]
        )
        self.assertEqual(calculated_tree, gitlink["tree_oid"])
        calculated_commit, _ = commit_oid(
            calculated_tree, gitlink["timestamp"], gitlink["message"]
        )
        self.assertEqual(calculated_commit, gitlink["commit_oid"])

        for label in ("single_file_at_limit", "single_file_over_limit"):
            variant = variants[label]
            generator = variant["generator"]
            content = bytes.fromhex(generator["repeat_byte_hex"]) * generator[
                "byte_length"
            ]
            object_id = git_oid("blob", content)
            self.assertEqual(object_id, variant["blob_oid"])
            calculated_tree, _ = tree_oid(
                [(generator["mode"], generator["path"], object_id)]
            )
            self.assertEqual(calculated_tree, variant["tree_oid"])
            calculated_commit, _ = commit_oid(
                calculated_tree, variant["timestamp"], variant["message"]
            )
            self.assertEqual(calculated_commit, variant["commit_oid"])

        isolation = variants["isolation"]
        outside = isolation["outside_canary"]["content_utf8"].encode()
        self.assertEqual(
            hashlib.sha256(outside).hexdigest(),
            isolation["expected_outside_canary_sha256"],
        )

    def test_inventory_evidence_counts_order_and_references_are_exact(self) -> None:
        manifest = load_json(FIXTURE_ROOT / "manifest.json")
        semantic = load_json(FIXTURE_ROOT / "expected-semantic-a.json")
        inventory = semantic["inventory"]
        files = inventory["files"]
        evidence = inventory["evidence"]
        manifest_files = {
            entry["path"]: entry for entry in manifest["revision"]["files"]
        }
        paths = [entry["path"] for entry in files]
        self.assertEqual(paths, sorted(paths, key=lambda value: value.encode()))
        self.assertEqual(set(paths), set(EXPECTED_CLASSIFICATIONS))
        self.assertEqual(len(evidence), len(files))
        self.assertEqual(
            [entry["evidence_id"] for entry in evidence],
            [f"evidence-{index:05d}" for index in range(1, len(files) + 1)],
        )
        evidence_by_id = {entry["evidence_id"]: entry for entry in evidence}
        self.assertEqual(len(evidence_by_id), len(evidence))

        self.assertEqual(len(files), len(evidence))
        for file_entry, evidence_entry in zip(files, evidence):
            path = file_entry["path"]
            source = manifest_files[path]
            classification = EXPECTED_CLASSIFICATIONS[path]
            self.assertEqual(file_entry["mode"], source["mode"])
            self.assertEqual(file_entry["blob_oid"], source["blob_oid"])
            self.assertEqual(file_entry["byte_length"], source["byte_length"])
            self.assertEqual(
                file_entry["content_kind"], classification["content_kind"]
            )
            self.assertEqual(file_entry["roles"], classification["roles"])
            self.assertEqual(file_entry["languages"], classification["languages"])
            self.assertEqual(file_entry["evidence_id"], evidence_entry["evidence_id"])
            self.assertEqual(evidence_entry["path"], path)
            self.assertEqual(evidence_entry["blob_oid"], source["blob_oid"])
            self.assertEqual(
                evidence_entry["repository"]["identity"],
                manifest["repository"]["identity"],
            )
            self.assertEqual(
                evidence_entry["repository"]["commit_oid"],
                manifest["revision"]["commit_oid"],
            )
            self.assertEqual(
                evidence_entry["span"],
                {"unit": "byte", "start": 0, "end": source["byte_length"]},
            )
            self.assertEqual(
                evidence_entry["derivation"]["rules"], classification["rules"]
            )

        for collection in ("manifests", "contracts", "configurations", "ownership"):
            for finding in inventory[collection]:
                evidence_entry = evidence_by_id[finding["evidence_id"]]
                self.assertEqual(evidence_entry["path"], finding["path"])
        for finding in inventory["unsupported_content"] + inventory["diagnostics"]:
            evidence_entry = evidence_by_id[finding["evidence_id"]]
            self.assertEqual(evidence_entry["path"], finding["path"])
        for language in inventory["languages"]:
            resolved_paths = [
                evidence_by_id[evidence_id]["path"]
                for evidence_id in language["evidence_ids"]
            ]
            self.assertEqual(resolved_paths, language["paths"])
        for gap in inventory["coverage_gaps"]:
            resolved_paths = [
                evidence_by_id[evidence_id]["path"]
                for evidence_id in gap["evidence_ids"]
            ]
            self.assertEqual(resolved_paths, gap["paths"])

        summary = inventory["summary"]
        expected = manifest["expected_inventory"]
        self.assertEqual(summary["directory_count"], expected["directory_count"])
        self.assertEqual(
            summary["regular_file_count"], expected["regular_file_count"]
        )
        self.assertEqual(
            summary["total_file_bytes"], expected["total_file_bytes"]
        )
        self.assertEqual(
            summary["supported_file_count"], expected["supported_file_count"]
        )
        self.assertEqual(
            summary["unsupported_file_count"], expected["unsupported_file_count"]
        )
        self.assertEqual(summary["language_count"], len(inventory["languages"]))
        self.assertEqual(summary["manifest_count"], len(inventory["manifests"]))
        self.assertEqual(summary["contract_count"], len(inventory["contracts"]))
        self.assertEqual(
            summary["configuration_count"], len(inventory["configurations"])
        )
        self.assertEqual(summary["ownership_count"], len(inventory["ownership"]))
        self.assertEqual(
            summary["diagnostic_count"], len(inventory["diagnostics"])
        )
        self.assertEqual(
            summary["coverage_gap_count"], len(inventory["coverage_gaps"])
        )
        self.assertEqual(
            summary["coverage_gap_count"], expected["coverage_gap_count"]
        )

        self.assertEqual(
            [entry["id"] for entry in inventory["languages"]],
            sorted(entry["id"] for entry in inventory["languages"]),
        )
        self.assertEqual(
            [
                (entry["capability"], entry["subject"])
                for entry in inventory["extraction_capabilities"]
            ],
            sorted(
                (entry["capability"], entry["subject"])
                for entry in inventory["extraction_capabilities"]
            ),
        )
        self.assertEqual(
            [(entry["code"], entry["path"]) for entry in inventory["diagnostics"]],
            sorted(
                (entry["code"], entry["path"]) for entry in inventory["diagnostics"]
            ),
        )
        self.assertEqual(
            [(entry["code"], entry["scope"]) for entry in inventory["coverage_gaps"]],
            sorted(
                (entry["code"], entry["scope"])
                for entry in inventory["coverage_gaps"]
            ),
        )
        for entry in files:
            self.assertEqual(entry["roles"], sorted(entry["roles"]))
            self.assertEqual(entry["languages"], sorted(entry["languages"]))
        for entry in evidence:
            self.assertEqual(
                entry["derivation"]["rules"],
                sorted(entry["derivation"]["rules"]),
            )

    def test_reviewed_error_goldens_are_strict_and_non_leaking(self) -> None:
        spec = load_json(SPEC_PATH)
        symlink = load_json(ROOT / spec["goldens"]["symlink_error"])
        file_limit = load_json(ROOT / spec["goldens"]["file_limit_error"])
        self.assertEqual(
            set(symlink),
            {"schema_version", "code", "stage", "message", "retryable", "context"},
        )
        self.assertEqual(
            symlink["context"], {"entry": "symlink", "path": "escape"}
        )
        self.assertEqual(
            file_limit["context"],
            {
                "limit": "single_file_bytes",
                "maximum": 4_194_304,
                "observed": 4_194_305,
            },
        )
        for value in (symlink, file_limit):
            serialized = canonical_json(value).decode()
            self.assertNotIn(str(ROOT), serialized)
            self.assertNotIn("../outside", serialized)
            self.assertFalse(value["retryable"])
            self.assertEqual(value["stage"], "acquisition")


if __name__ == "__main__":
    unittest.main()
