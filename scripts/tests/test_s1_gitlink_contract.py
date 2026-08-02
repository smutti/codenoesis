from __future__ import annotations

import hashlib
import json
import re
import unittest
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
SRS_PATH = ROOT / "docs/software/software-requirements-specification.md"
DECISION_PATH = (
    ROOT / "docs/software/decisions/0010-s1-gitlink-boundary-contract.md"
)
ORACLE_PATH = ROOT / "tests/specifications/s1/e2e_fr_acq_005_gitlink_boundaries.json"
BUNDLE_PATH = (
    ROOT / "tests/specifications/s1/gitlink-boundary-contract-bundle.json"
)
SNAPSHOT_SCHEMA_PATH = (
    ROOT / "tests/specifications/s1/repository-snapshot-v5.schema.json"
)
BOUNDARY_SCHEMA_PATH = (
    ROOT / "tests/specifications/s1/external-repository-boundary-v1.schema.json"
)
INPUT_SCHEMA_PATH = (
    ROOT / "tests/specifications/s1/repository-boundary-input-v1.schema.json"
)
ERROR_SCHEMA_PATH = ROOT / "tests/specifications/s1/codenoesis-error-v9.schema.json"
FIXTURE_ROOT = ROOT / "tests/fixtures/s1/gitlink-boundary-v1"
FIXTURE_MANIFEST_PATH = FIXTURE_ROOT / "manifest.json"
GITMODULES_PATH = FIXTURE_ROOT / "revision-overlay/.gitmodules"
UNBOUND_GOLDEN_PATH = FIXTURE_ROOT / "expected-boundaries-unbound.json"
BOUND_GOLDEN_PATH = FIXTURE_ROOT / "expected-boundaries-bound.json"
MATCHING_INPUT_PATH = FIXTURE_ROOT / "boundary-input-matching.json"
MISMATCH_INPUT_PATH = FIXTURE_ROOT / "boundary-input-mismatch.json"

APPROVAL_REFERENCE = "https://github.com/smutti/codenoesis/pull/93"
ISSUE_REFERENCE = "https://github.com/smutti/codenoesis/issues/92"
REQUIREMENT = "FR-ACQ-005"
ROOT_IDENTITY = "urn:codenoesis:fixture:s1-gitlink-boundary-v1"
NESTED_IDENTITY = "urn:codenoesis:fixture:s1-gitlink-boundary-v1-nested-model"

LIMITS = {
    "boundary_manifest_bytes": 262_144,
    "gitlink_entries": 128,
    "gitmodules_bytes": 1_048_576,
    "gitmodules_sections": 256,
    "gitmodules_keys_per_section": 32,
    "explicit_nested_repositories": 32,
    "explicit_nesting_depth": 1,
    "repository_roots": 33,
    "boundary_report_bytes": 1_048_576,
    "canonical_path_bytes": 1_024,
    "canonical_path_component_bytes": 255,
    "canonical_snapshot_bytes": 33_554_432,
    "scan_wall_milliseconds": 60_000,
}

ERROR_CODES = (
    "input.invalid_repository_boundary_profile",
    "input.invalid_repository_boundary_manifest",
    "acquisition.repository_boundary_metadata_invalid",
    "acquisition.nested_repository_mismatch",
    "acquisition.nested_repository_unavailable",
    "acquisition.nested_repository_changed",
    "acquisition.repository_boundary_limit_exceeded",
    "internal.unexpected",
)

TEST_ORDER = (
    "e2e_fr_acq_005_gitlink_boundaries",
    "gt_fr_acq_005_exact_boundary_projection",
    "gt_fr_acq_005_explicit_nested_binding",
    "conf_fr_acq_005_boundary_input_v1",
    "conf_fr_acq_005_gitmodules_subset",
    "gt_fr_acq_005_boundary_states_and_gaps",
    "sec_fr_acq_005_url_redaction",
    "sec_fr_acq_005_no_ambient_nested_authority",
    "conf_fr_acq_005_error_v9",
    "race_fr_acq_005_nested_replacement",
    "pt_fr_acq_005_limits_have_max_and_plus_one",
    "pt_fr_acq_005_order_invariant",
    "pt_fr_acq_005_parallel_replay",
    "fz_fr_acq_005_gitmodules_seed_corpus",
    "conf_fr_acq_005_v5_store_docs_query",
    "reg_fr_acq_005_legacy_gitlink_rejection",
    "reg_fr_acq_005_r0_r1_unchanged",
    "reg_fr_acq_005_s0_s6_unchanged",
    "pilot_fr_acq_005_rustdesk_progression",
)

VARIANT_ORDER = (
    "nested-absent",
    "present-but-ignored",
    "explicit-match",
    "explicit-mismatch",
    "missing-declaration",
    "orphan-declaration",
    "unsupported-key",
    "credential-canary",
    "malformed-metadata",
    "escaping-path",
    "duplicate-or-ambiguous",
    "recursive-input",
    "nested-change-race",
    "limits",
)

BUNDLE_FILES = {
    "LICENSE",
    "docs/software/decisions/0010-s1-gitlink-boundary-contract.md",
    "scripts/tests/test_s1_gitlink_contract.py",
    "tests/corpora/real-world-rust-v1.json",
    "tests/fixtures/s1/gitlink-boundary-v1/README.md",
    "tests/fixtures/s1/gitlink-boundary-v1/boundary-input-matching.json",
    "tests/fixtures/s1/gitlink-boundary-v1/boundary-input-mismatch.json",
    "tests/fixtures/s1/gitlink-boundary-v1/expected-boundaries-bound.json",
    "tests/fixtures/s1/gitlink-boundary-v1/expected-boundaries-unbound.json",
    "tests/fixtures/s1/gitlink-boundary-v1/manifest.json",
    "tests/fixtures/s1/gitlink-boundary-v1/revision-overlay/.gitmodules",
    "tests/specifications/s1/codenoesis-error-v9.schema.json",
    "tests/specifications/s1/e2e_fr_acq_005_gitlink_boundaries.json",
    "tests/specifications/s1/external-repository-boundary-v1.schema.json",
    "tests/specifications/s1/packed-sha1-contract-bundle.json",
    "tests/specifications/s1/repository-boundary-input-v1.schema.json",
    "tests/specifications/s1/repository-snapshot-v5.schema.json",
    "tests/specifications/s4/contract-bundle.json",
}

IMMUTABLE_FILES = {
    "docs/software/decisions/README.md": (
        "77ed3a1d795bc372f2780110c5a2651166b28b4a52c75f3b29917348bfbb583a"
    ),
    "tests/corpora/real-world-rust-v1.json": (
        "1d2edc9f858d612e76abb70e6dd255d28e88306a0e4874b0e8ea7351f4347f46"
    ),
    "tests/specifications/s1/packed-sha1-contract-bundle.json": (
        "38024dda756914484b38ded700a82373cff95b5ae549c815917c0f79649c0578"
    ),
    "tests/specifications/s4/contract-bundle.json": (
        "be199ebbeb9cb35c2e6a68c5b9d847f86fe131efd007b0d09d9fd28390c91437"
    ),
}


def reject_duplicate_members(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON member: {key}")
        result[key] = value
    return result


def load_json(path: Path) -> Any:
    return json.loads(
        path.read_text(encoding="utf-8"),
        object_pairs_hook=reject_duplicate_members,
    )


def canonical_json(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_path(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def git_object_oid(kind: str, body: bytes) -> str:
    header = f"{kind} {len(body)}\0".encode("ascii")
    return hashlib.sha1(header + body).hexdigest()


def git_tree_oid(entries: list[dict[str, str]]) -> str:
    body = b"".join(
        entry["mode"].encode("ascii")
        + b" "
        + entry["name"].encode("utf-8")
        + b"\0"
        + bytes.fromhex(entry["oid"])
        for entry in entries
    )
    return git_object_oid("tree", body)


def identity_digest(domain: str, *fields: object) -> str:
    framed = domain.encode("utf-8") + b"\0"
    framed += b"\0".join(str(field).encode("utf-8") for field in fields)
    return sha256_bytes(framed)


def closed_object_schemas(value: Any, location: str = "$") -> list[str]:
    failures: list[str] = []
    if isinstance(value, dict):
        if value.get("type") == "object" and value.get("additionalProperties") is not False:
            failures.append(location)
        for key, child in value.items():
            failures.extend(closed_object_schemas(child, f"{location}/{key}"))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            failures.extend(closed_object_schemas(child, f"{location}/{index}"))
    return failures


def parse_primary_gitmodules(value: bytes) -> dict[str, str]:
    if b"\x00" in value or b"\r" in value.replace(b"\r\n", b""):
        raise ValueError("forbidden control or line ending")
    text = value.decode("utf-8")
    section: str | None = None
    keys: dict[str, str] = {}
    for raw_line in text.splitlines():
        line = raw_line.strip(" \t")
        if not line or line.startswith(("#", ";")):
            continue
        section_match = re.fullmatch(
            r'\[submodule "([A-Za-z0-9._/-]{1,255})"\]', line
        )
        if section_match:
            if section is not None:
                raise ValueError("fixture must contain one section")
            section = section_match.group(1)
            continue
        if section is None:
            raise ValueError("key outside section")
        assignment = re.fullmatch(r"([a-z][a-z0-9-]{0,63})[ \t]*=[ \t]*(.+)", line)
        if assignment is None:
            raise ValueError("invalid assignment")
        key, raw_value = assignment.groups()
        parsed_value = raw_value.rstrip(" \t")
        if key in keys or not parsed_value:
            raise ValueError("duplicate key or empty value")
        keys[key] = parsed_value
    if section is None or set(keys) != {"path", "url"}:
        raise ValueError("fixture requires one exact path/url section")
    return {"name": section, **keys}


class S1GitlinkContractTest(unittest.TestCase):
    def test_every_json_artifact_is_duplicate_free_and_valid_utf8(self) -> None:
        paths = (
            ORACLE_PATH,
            SNAPSHOT_SCHEMA_PATH,
            BOUNDARY_SCHEMA_PATH,
            INPUT_SCHEMA_PATH,
            ERROR_SCHEMA_PATH,
            FIXTURE_MANIFEST_PATH,
            UNBOUND_GOLDEN_PATH,
            BOUND_GOLDEN_PATH,
            MATCHING_INPUT_PATH,
            MISMATCH_INPUT_PATH,
        )
        for path in paths:
            with self.subTest(path=path.relative_to(ROOT)):
                self.assertIsNotNone(load_json(path))
                path.read_bytes().decode("utf-8")

    def test_new_json_schemas_are_closed_and_versioned(self) -> None:
        schemas = {
            SNAPSHOT_SCHEMA_PATH: "codenoesis.repository-snapshot/v5",
            BOUNDARY_SCHEMA_PATH: "codenoesis.repository-boundaries/v1",
            INPUT_SCHEMA_PATH: "codenoesis.repository-boundary-input/v1",
            ERROR_SCHEMA_PATH: "codenoesis.error/v9",
        }
        for path, version in schemas.items():
            with self.subTest(path=path.relative_to(ROOT)):
                schema = load_json(path)
                self.assertEqual(
                    closed_object_schemas(schema),
                    [],
                    "every object-valued public schema must reject unknown fields",
                )
                serialized = path.read_text(encoding="utf-8")
                self.assertIn(version, serialized)

        snapshot = load_json(SNAPSHOT_SCHEMA_PATH)
        semantic = snapshot["properties"]["semantic"]
        self.assertEqual(
            semantic["properties"]["pipeline_version"]["const"],
            "codenoesis.pipeline/s4-r2-v1",
        )
        self.assertEqual(
            semantic["properties"]["ontology_version"]["const"],
            "codenoesis.ontology/rust/v2",
        )
        self.assertEqual(
            semantic["properties"]["extractor_versions"]["const"],
            [
                "codenoesis.inventory-classifier/s1-v1",
                "codenoesis.rust-tree-sitter/s4-v1",
                "codenoesis.rust-workspace/s4-v1",
                "codenoesis.git-boundary/s1-v1",
            ],
        )
        configuration = semantic["properties"]["configuration"]["properties"]
        self.assertEqual(
            configuration["schema_version"]["const"],
            "codenoesis.configuration/v2",
        )
        self.assertEqual(
            configuration["repository_boundary_profile"]["const"],
            "local-gitlinks-v1",
        )
        self.assertEqual(
            semantic["properties"]["inventory"]["$ref"],
            "repository-snapshot-v2.schema.json#/$defs/inventory",
        )
        self.assertFalse(
            (ROOT / "tests/specifications/s1/repository-inventory-v2.schema.json").exists()
        )

    def test_canonical_path_grammar_accepts_only_confined_paths(self) -> None:
        for schema_path in (INPUT_SCHEMA_PATH, BOUNDARY_SCHEMA_PATH, ERROR_SCHEMA_PATH):
            pattern = load_json(schema_path)["$defs"]["canonical_relative_path"][
                "pattern"
            ]
            for valid in ("nested", "external/nested-model", ".hidden/repo"):
                self.assertIsNotNone(re.fullmatch(pattern, valid), valid)
            for invalid in (
                "",
                "/absolute",
                "../escape",
                "nested/../escape",
                "nested//repo",
                "nested\\repo",
                "nested\nrepo",
            ):
                self.assertIsNone(re.fullmatch(pattern, invalid), invalid)

    def test_fixture_git_objects_recompute_independently(self) -> None:
        manifest = load_json(FIXTURE_MANIFEST_PATH)
        objects = manifest["objects"]
        gitmodules = GITMODULES_PATH.read_bytes()
        gitmodules_record = objects["gitmodules"]
        self.assertEqual(len(gitmodules), gitmodules_record["byte_length"])
        self.assertEqual(sha256_bytes(gitmodules), gitmodules_record["sha256"])
        self.assertEqual(
            git_object_oid("blob", gitmodules), gitmodules_record["blob_oid"]
        )

        nested = objects["nested_repository"]
        self.assertEqual(
            git_object_oid("tree", b""),
            nested["tree_oid"],
        )
        self.assertEqual(
            git_object_oid("commit", nested["commit_payload_utf8"].encode("utf-8")),
            nested["commit_oid"],
        )

        external = objects["external_tree"]
        self.assertEqual(git_tree_oid(external["entries"]), external["oid"])
        root_tree = objects["root_tree"]
        self.assertEqual(git_tree_oid(root_tree["entries"]), root_tree["oid"])
        root_commit = objects["root_commit"]
        self.assertEqual(
            git_object_oid("commit", root_commit["payload_utf8"].encode("utf-8")),
            root_commit["oid"],
        )
        self.assertEqual(root_commit["repository_identity"], ROOT_IDENTITY)
        self.assertIn(f"tree {root_tree['oid']}\n", root_commit["payload_utf8"])
        self.assertEqual(
            external["entries"],
            [
                {
                    "mode": "160000",
                    "name": "nested-model",
                    "oid": nested["commit_oid"],
                }
            ],
        )

    def test_fixture_reuses_s4_source_without_copying_nested_source(self) -> None:
        manifest = load_json(FIXTURE_MANIFEST_PATH)
        inherited = manifest["inherited_contracts"]
        self.assertEqual(
            inherited["s4_source_fixture"],
            "tests/fixtures/s4/workspace-docs-v1",
        )
        self.assertEqual(
            inherited["s4_contract_bundle_sha256"],
            "3efb380fb058a5831123a0f990676575da04e60998cada8987f034675b61f12e",
        )
        self.assertEqual(
            inherited["r0_r1_contract_bundle_sha256"],
            "08602c21b06e0e0cea754312fb8c9f5d28db36ae31e9e646df669b2c129826df",
        )
        boundary = manifest["fixture_boundary"]
        self.assertFalse(boundary["nested_source_committed"])
        self.assertFalse(boundary["generated_repositories_committed"])
        self.assertFalse(boundary["product_git_process_allowed"])
        self.assertFalse(boundary["product_network_allowed"])
        self.assertFalse(boundary["product_url_resolution_allowed"])
        self.assertFalse(boundary["product_worktree_discovery_allowed"])
        self.assertFalse(boundary["product_first_party_unsafe_allowed"])
        committed_files = [path for path in FIXTURE_ROOT.rglob("*") if path.is_file()]
        self.assertFalse(any("generated" in path.parts for path in committed_files))
        self.assertEqual(
            [item["id"] for item in manifest["variants"]],
            list(VARIANT_ORDER),
        )
        self.assertEqual(manifest["limits"], LIMITS)

    def test_primary_gitmodules_projection_is_exact_and_digest_only(self) -> None:
        parsed = parse_primary_gitmodules(GITMODULES_PATH.read_bytes())
        self.assertEqual(
            parsed,
            {
                "name": "nested-model",
                "path": "external/nested-model",
                "url": "https://example.invalid/codenoesis/nested-model.git",
            },
        )
        unbound = load_json(UNBOUND_GOLDEN_PATH)
        declaration = unbound["declarations"][0]
        self.assertEqual(
            declaration["name_sha256"],
            sha256_bytes(parsed["name"].encode("utf-8")),
        )
        self.assertEqual(
            declaration["url_sha256"],
            sha256_bytes(parsed["url"].encode("utf-8")),
        )
        self.assertEqual(declaration["url_kind"], "https")
        raw_url = parsed["url"]
        for path in (UNBOUND_GOLDEN_PATH, BOUND_GOLDEN_PATH, ORACLE_PATH):
            self.assertNotIn(raw_url, path.read_text(encoding="utf-8"))
        self.assertNotIn("url", declaration.keys() - {"url_kind", "url_sha256"})

    def test_boundary_declaration_evidence_and_gap_ids_recompute(self) -> None:
        manifest = load_json(FIXTURE_MANIFEST_PATH)
        objects = manifest["objects"]
        root_commit = objects["root_commit"]["oid"]
        root_tree = objects["root_tree"]["oid"]
        external_tree = objects["external_tree"]["oid"]
        gitmodules_oid = objects["gitmodules"]["blob_oid"]
        nested_commit = objects["nested_repository"]["commit_oid"]
        parsed = parse_primary_gitmodules(GITMODULES_PATH.read_bytes())
        name_sha256 = sha256_bytes(parsed["name"].encode("utf-8"))

        boundary_id = "urn:codenoesis:repository-boundary:sha256:" + identity_digest(
            "codenoesis.repository-boundary/v1",
            ROOT_IDENTITY,
            root_commit,
            parsed["path"],
            nested_commit,
        )
        declaration_id = (
            "urn:codenoesis:gitmodules-declaration:sha256:"
            + identity_digest(
                "codenoesis.gitmodules-declaration/v1",
                ROOT_IDENTITY,
                root_commit,
                parsed["path"],
                name_sha256,
            )
        )
        tree_evidence_id = (
            "urn:codenoesis:boundary-evidence:sha256:"
            + identity_digest(
                "codenoesis.boundary-evidence.git-tree-entry/v1",
                ROOT_IDENTITY,
                root_commit,
                external_tree,
                parsed["path"],
                "160000",
                nested_commit,
            )
        )
        declaration_evidence_id = (
            "urn:codenoesis:boundary-evidence:sha256:"
            + identity_digest(
                "codenoesis.boundary-evidence.gitmodules/v1",
                ROOT_IDENTITY,
                root_commit,
                gitmodules_oid,
                0,
                len(GITMODULES_PATH.read_bytes()),
            )
        )
        unbound_gap_id = "urn:codenoesis:boundary-gap:sha256:" + identity_digest(
            "codenoesis.boundary-gap/v1",
            "boundary.nested_repository_unbound",
            boundary_id,
        )
        bound_gap_id = "urn:codenoesis:boundary-gap:sha256:" + identity_digest(
            "codenoesis.boundary-gap/v1",
            "boundary.nested_repository_not_analyzed",
            boundary_id,
        )

        unbound = load_json(UNBOUND_GOLDEN_PATH)
        bound = load_json(BOUND_GOLDEN_PATH)
        for golden in (unbound, bound):
            self.assertEqual(golden["root_repository"]["identity"], ROOT_IDENTITY)
            self.assertEqual(golden["root_repository"]["commit_oid"], root_commit)
            self.assertEqual(golden["root_repository"]["tree_oid"], root_tree)
            self.assertEqual(golden["boundaries"][0]["boundary_id"], boundary_id)
            self.assertEqual(golden["boundaries"][0]["gitlink_oid"], nested_commit)
            self.assertEqual(
                golden["boundaries"][0]["declaration_id"], declaration_id
            )
            self.assertEqual(golden["declarations"][0]["declaration_id"], declaration_id)
            self.assertEqual(
                golden["boundaries"][0]["evidence_ids"],
                [tree_evidence_id, declaration_evidence_id],
            )
            evidence_ids = [item["evidence_id"] for item in golden["evidence"]]
            self.assertEqual(evidence_ids, [tree_evidence_id, declaration_evidence_id])

        self.assertEqual(unbound["boundaries"][0]["state"], "declared_unbound")
        self.assertIsNone(unbound["boundaries"][0]["nested_repository"])
        self.assertEqual(
            unbound["boundaries"][0]["coverage_gap_ids"], [unbound_gap_id]
        )
        self.assertEqual(unbound["coverage_gaps"][0]["gap_id"], unbound_gap_id)

        self.assertEqual(bound["boundaries"][0]["state"], "explicitly_bound")
        self.assertEqual(
            bound["boundaries"][0]["nested_repository"],
            {
                "identity": NESTED_IDENTITY,
                "vcs": "git",
                "object_format": "sha1",
                "commit_oid": nested_commit,
                "tree_oid": objects["nested_repository"]["tree_oid"],
            },
        )
        self.assertEqual(bound["boundaries"][0]["coverage_gap_ids"], [bound_gap_id])
        self.assertEqual(bound["coverage_gaps"][0]["gap_id"], bound_gap_id)

    def test_boundary_goldens_have_closed_references_counts_and_order(self) -> None:
        for path in (UNBOUND_GOLDEN_PATH, BOUND_GOLDEN_PATH):
            golden = load_json(path)
            with self.subTest(path=path.name):
                self.assertEqual(
                    set(golden),
                    {
                        "schema_version",
                        "profile",
                        "root_repository",
                        "summary",
                        "boundaries",
                        "declarations",
                        "coverage_gaps",
                        "evidence",
                    },
                )
                self.assertEqual(golden["profile"], "local-gitlinks-v1")
                summary = golden["summary"]
                self.assertEqual(summary["boundary_count"], len(golden["boundaries"]))
                self.assertEqual(
                    summary["declaration_count"], len(golden["declarations"])
                )
                self.assertEqual(
                    summary["coverage_gap_count"], len(golden["coverage_gaps"])
                )
                self.assertEqual(
                    summary["bound_count"],
                    sum(item["state"] == "explicitly_bound" for item in golden["boundaries"]),
                )
                self.assertEqual(
                    summary["unbound_count"],
                    sum(item["state"] != "explicitly_bound" for item in golden["boundaries"]),
                )
                evidence_ids = {item["evidence_id"] for item in golden["evidence"]}
                gap_ids = {item["gap_id"] for item in golden["coverage_gaps"]}
                boundary_ids = {item["boundary_id"] for item in golden["boundaries"]}
                declaration_ids = {
                    item["declaration_id"] for item in golden["declarations"]
                }
                for boundary in golden["boundaries"]:
                    self.assertLessEqual(set(boundary["evidence_ids"]), evidence_ids)
                    self.assertLessEqual(set(boundary["coverage_gap_ids"]), gap_ids)
                    if boundary["declaration_id"] is not None:
                        self.assertIn(boundary["declaration_id"], declaration_ids)
                for declaration in golden["declarations"]:
                    self.assertIn(declaration["evidence_id"], evidence_ids)
                    if declaration["boundary_id"] is not None:
                        self.assertIn(declaration["boundary_id"], boundary_ids)
                for gap in golden["coverage_gaps"]:
                    self.assertIn(gap["subject_id"], boundary_ids | declaration_ids)
                    self.assertLessEqual(set(gap["evidence_ids"]), evidence_ids)

    def test_boundary_input_files_are_root_bound_flat_and_operational(self) -> None:
        matching = load_json(MATCHING_INPUT_PATH)
        mismatch = load_json(MISMATCH_INPUT_PATH)
        for document in (matching, mismatch):
            self.assertEqual(
                set(document), {"schema_version", "root", "nested_repositories"}
            )
            self.assertEqual(document["schema_version"], "codenoesis.repository-boundary-input/v1")
            self.assertEqual(document["root"]["repository_identity"], ROOT_IDENTITY)
            self.assertEqual(
                document["root"]["commit_oid"],
                load_json(FIXTURE_MANIFEST_PATH)["objects"]["root_commit"]["oid"],
            )
            self.assertEqual(len(document["nested_repositories"]), 1)
            nested = document["nested_repositories"][0]
            self.assertEqual(
                set(nested),
                {
                    "boundary_path",
                    "repository_identity",
                    "repository_root",
                    "revision",
                    "acquisition_profile",
                },
            )
            self.assertFalse(Path(nested["repository_root"]).is_absolute())
            self.assertNotIn("..", Path(nested["repository_root"]).parts)
        self.assertEqual(
            matching["nested_repositories"][0]["revision"],
            load_json(FIXTURE_MANIFEST_PATH)["objects"]["nested_repository"][
                "commit_oid"
            ],
        )
        self.assertNotEqual(
            mismatch["nested_repositories"][0]["revision"],
            matching["nested_repositories"][0]["revision"],
        )
        self.assertNotIn(
            "acquisition_profile",
            json.dumps(load_json(BOUND_GOLDEN_PATH), sort_keys=True),
        )

    def test_error_v9_is_boundary_only_closed_and_retryable_only_for_race(self) -> None:
        schema = load_json(ERROR_SCHEMA_PATH)
        self.assertEqual(tuple(schema["properties"]["code"]["enum"]), ERROR_CODES)
        retry_rule = schema["allOf"][0]
        self.assertEqual(
            retry_rule["if"]["properties"]["code"]["const"],
            "acquisition.nested_repository_changed",
        )
        self.assertTrue(
            retry_rule["then"]["properties"]["retryable"]["const"]
        )
        self.assertFalse(
            retry_rule["else"]["properties"]["retryable"]["const"]
        )
        context = schema["properties"]["context"]
        self.assertFalse(context["additionalProperties"])
        self.assertNotIn("url", context["properties"])
        self.assertNotIn("repository_root", context["properties"])
        self.assertEqual(
            set(context["properties"]["limit"]["enum"]),
            {
                "boundary_manifest_bytes",
                "gitlink_entries",
                "gitmodules_bytes",
                "gitmodules_sections",
                "gitmodules_keys_per_section",
                "explicit_nested_repositories",
                "explicit_nesting_depth",
                "boundary_report_bytes",
            },
        )
        self.assertEqual(len(ERROR_CODES), len(set(ERROR_CODES)))

    def test_oracle_fixes_selector_red_limits_security_and_test_order(self) -> None:
        oracle = load_json(ORACLE_PATH)
        self.assertEqual(oracle["status"], "approved")
        self.assertEqual(oracle["requirements"], [REQUIREMENT])
        self.assertEqual(oracle["slice"], "S1")
        self.assertEqual(oracle["roadmap_checkpoint"], "R0-R2")
        self.assertEqual(oracle["ratification"]["authorization_statement"], "Autorizzo issue #92")
        self.assertEqual(oracle["ratification"]["approval_reference"], APPROVAL_REFERENCE)
        selector = oracle["selector_contract"]
        self.assertEqual(selector["value"], "local-gitlinks-v1")
        self.assertEqual(selector["valid_with_profiles"], ["standard-local-s4"])
        self.assertTrue(selector["included_in_semantic_hash"])
        self.assertTrue(selector["legacy_absence_is_byte_identical"])
        self.assertFalse(
            oracle["boundary_input_contract"]["nested_acquisition_profile_is_semantic"]
        )
        self.assertEqual(oracle["limits"], LIMITS)
        self.assertEqual(tuple(oracle["test_order"]), TEST_ORDER)
        self.assertEqual(
            [scenario["test_name"] for scenario in oracle["scenarios"]],
            list(TEST_ORDER),
        )
        self.assertTrue(all("fr_acq_005" in name for name in TEST_ORDER))
        red = oracle["expected_red"]
        self.assertEqual(red["required_base_commit"], "728244e2d05087da20c0c722879cb05510f9a988")
        self.assertEqual(red["subject_observed_exit_code"], 2)
        self.assertEqual(red["subject_observed_stdout_bytes"], 0)
        self.assertEqual(red["subject_observed_stderr_schema"], "codenoesis.error/v4")
        self.assertEqual(red["subject_observed_stderr_code"], "input.invalid_revision")
        stderr = red["subject_observed_stderr_utf8"].encode("utf-8")
        self.assertEqual(len(stderr), red["subject_observed_stderr_bytes"])
        self.assertEqual(sha256_bytes(stderr), red["subject_observed_stderr_sha256"])
        self.assertTrue(stderr.endswith(b"\n"))
        self.assertFalse(red["store_exists_after_subject"])
        self.assertTrue(all(value in (False, 0) for value in red["authority_observation"].values()))
        self.assertIn("implicit nested discovery or traversal", oracle["security_contract"]["forbidden"])
        self.assertIn("raw credential-bearing URL retention", oracle["security_contract"]["forbidden"])

    def test_rustdesk_pilot_is_pinned_generic_and_matches_public_tree(self) -> None:
        oracle = load_json(ORACLE_PATH)
        pilot = oracle["rustdesk_pilot"]
        self.assertEqual(pilot["commit_oid"], "d412d198720aa56f6cfed2dfad262e8fb1322fb7")
        self.assertEqual(pilot["tree_oid"], "df8d4c292c9d256a445480eb878e507df3de1dc4")
        self.assertEqual(pilot["gitlink_count"], 1)
        self.assertEqual(pilot["gitlink_path"], "libs/hbb_common")
        self.assertEqual(pilot["gitlink_oid"], "69cea8dafee147848ae88702029f4bf7df7224c3")
        self.assertEqual(pilot["gitmodules_blob_oid"], "d80e69aa84a4c7f764eea191622a386878cf852d")
        self.assertFalse(pilot["vendored_source"])
        self.assertFalse(pilot["raw_url_retained"])
        self.assertFalse(pilot["repository_specific_semantics"])
        fixture_pilot = load_json(FIXTURE_MANIFEST_PATH)["public_pilot_observation"]
        self.assertEqual(fixture_pilot["commit_oid"], pilot["commit_oid"])
        self.assertEqual(fixture_pilot["tree_oid"], pilot["tree_oid"])
        self.assertEqual(fixture_pilot["gitlink_path"], pilot["gitlink_path"])
        self.assertEqual(fixture_pilot["gitlink_oid"], pilot["gitlink_oid"])
        self.assertEqual(fixture_pilot["gitmodules_blob_oid"], pilot["gitmodules_blob_oid"])

    def test_srs_decision_and_oracle_are_machine_linked(self) -> None:
        srs = SRS_PATH.read_text(encoding="utf-8")
        decision = DECISION_PATH.read_text(encoding="utf-8")
        oracle = load_json(ORACLE_PATH)
        heading = "### 2.12 S1 gitlink boundary ratification register"
        self.assertIn(heading, srs)
        register = srs.split(heading, 1)[1].split(
            "## 3. Product intent and success definition", 1
        )[0]
        rows = re.findall(
            r"^\| `(FR-ACQ-\d{3})` \| `Proposed` "
            r"\(authorized in issue #92; pending protected merge\) \| `Approved` \|",
            register,
            flags=re.MULTILINE,
        )
        self.assertEqual(rows, [REQUIREMENT])
        self.assertIsNotNone(
            re.search(
                r"^\| `FR-ACQ-005` \| P0 \| `0\.1` \|",
                srs,
                flags=re.MULTILINE,
            ),
        )
        self.assertIn(APPROVAL_REFERENCE, register)
        self.assertIn("Decision 0010 additionally resolves only", srs)
        self.assertIn("implemented R0/R1 behavior", srs)
        self.assertIn("| Status | Accepted;", decision)
        self.assertIn("[#92](https://github.com/smutti/codenoesis/issues/92)", decision)
        self.assertIn(APPROVAL_REFERENCE, decision)
        self.assertIn("Autorizzo issue #92", decision)
        self.assertIn("authoring agent must not approve or merge", decision)
        self.assertIn("separate", decision)
        self.assertIn(oracle["decision"], str(DECISION_PATH.relative_to(ROOT)))
        self.assertIn("no `RepositoryInventoryV2`", decision)
        self.assertIn("does not modify `tests/corpora/real-world-rust-v1.json`", decision)

    def test_accepted_r0_r1_s4_and_decision_index_files_are_unchanged(self) -> None:
        for path, expected in IMMUTABLE_FILES.items():
            with self.subTest(path=path):
                self.assertEqual(sha256_path(ROOT / path), expected)
        oracle = load_json(ORACLE_PATH)
        self.assertEqual(
            oracle["inherited_contracts"]["r0_r1_bundle"]["bundle_sha256"],
            "08602c21b06e0e0cea754312fb8c9f5d28db36ae31e9e646df669b2c129826df",
        )
        self.assertEqual(
            oracle["inherited_contracts"]["s4_bundle"]["bundle_sha256"],
            "3efb380fb058a5831123a0f990676575da04e60998cada8987f034675b61f12e",
        )
        self.assertFalse(
            oracle["inherited_contracts"]["corpus"]["mutable_in_r2_governance"]
        )

    def test_contract_bundle_binds_every_r2_artifact(self) -> None:
        bundle = load_json(BUNDLE_PATH)
        self.assertEqual(set(bundle), {"schema_version", "files", "bundle_sha256"})
        self.assertEqual(bundle["schema_version"], "codenoesis.contract-bundle/v1")
        files = bundle["files"]
        paths = [item["path"] for item in files]
        self.assertEqual(paths, sorted(paths))
        self.assertEqual(set(paths), BUNDLE_FILES)
        self.assertEqual(len(paths), len(set(paths)))
        for item in files:
            self.assertEqual(set(item), {"path", "sha256"})
            path = Path(item["path"])
            self.assertFalse(path.is_absolute())
            self.assertNotIn("..", path.parts)
            self.assertRegex(item["sha256"], r"^[0-9a-f]{64}$")
            self.assertEqual(sha256_path(ROOT / path), item["sha256"])
        payload = {
            "schema_version": bundle["schema_version"],
            "files": files,
        }
        bundle_sha256 = sha256_bytes(canonical_json(payload))
        self.assertEqual(bundle["bundle_sha256"], bundle_sha256)
        srs = SRS_PATH.read_text(encoding="utf-8")
        match = re.search(
            r"S1 safe gitlink boundary contract bundle:\s+"
            r"`sha256:([0-9a-f]{64})`",
            srs,
        )
        self.assertIsNotNone(match, "SRS must bind the complete R2 bundle")
        self.assertEqual(match.group(1), bundle_sha256)  # type: ignore[union-attr]


if __name__ == "__main__":
    unittest.main()
