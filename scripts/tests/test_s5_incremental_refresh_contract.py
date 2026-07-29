from __future__ import annotations

import copy
import hashlib
import re
import unittest
from pathlib import Path
from typing import Any

from test_s1_contract import (
    blake3_256,
    canonical_json,
    git_oid,
    load_json,
)


ROOT = Path(__file__).resolve().parents[2]
FIXTURE_ROOT = ROOT / "tests" / "fixtures" / "s5" / "incremental-refresh-v1"
SPEC_ROOT = ROOT / "tests" / "specifications" / "s5"
MANIFEST_PATH = FIXTURE_ROOT / "manifest.json"
COLD_PATH = FIXTURE_ROOT / "expected-cold-artifacts.json"
REPORT_PATH = FIXTURE_ROOT / "expected-incremental-refresh-report.json"
ACCEPTANCE_PATH = SPEC_ROOT / "e2e_fr_inc_001_incremental_refresh.json"
CACHE_SCHEMA_PATH = SPEC_ROOT / "analysis-cache-entry-v1.schema.json"
REPORT_SCHEMA_PATH = SPEC_ROOT / "incremental-refresh-report-v1.schema.json"
ERROR_SCHEMA_PATH = SPEC_ROOT / "codenoesis-error-v7.schema.json"
RULES_PATH = SPEC_ROOT / "incremental-rule-catalog-v1.json"
BUNDLE_PATH = SPEC_ROOT / "contract-bundle.json"
S4_BUNDLE_PATH = ROOT / "tests" / "specifications" / "s4" / "contract-bundle.json"
SRS_PATH = ROOT / "docs" / "software" / "software-requirements-specification.md"
DECISION_PATH = (
    ROOT
    / "docs"
    / "software"
    / "decisions"
    / "0008-s5-incremental-refresh-contract.md"
)

S5_REQUIREMENTS = [
    "INV-INC-001",
    "FR-INC-001",
    "FR-INC-002",
    "FR-INC-003",
    "FR-CLI-004",
]

S5_TEST_ORDER = [
    "e2e_fr_inc_001_incremental_refresh",
    "pt_inv_inc_001_cold_equivalence",
    "gt_fr_inc_001_exact_invalidation",
    "pt_fr_inc_002_version_rebuild_matrix",
    "pt_fr_inc_003_revision_neutral_cache",
    "conf_fr_cli_004_report_error_v7",
    "ft_fr_cli_004_atomic_head",
    "sec_fr_inc_001_no_target_execution",
    "pt_fr_inc_003_randomized_edit_sequences",
    "reg_fr_cli_004_s4_contract_unchanged",
]

EXPECTED_LIMITS = {
    "changed_paths": 100000,
    "analysis_entries": 100000,
    "dependency_edges": 1000000,
    "report_subject_ids": 1000000,
    "report_bytes": 16777216,
    "refresh_wall_milliseconds": 60000,
}

EXPECTED_VERSIONS = {
    "cache_schema": "codenoesis.analysis-cache-entry/v1",
    "language_extractor": "codenoesis.rust-tree-sitter/s4-v1",
    "workspace_mapper": "codenoesis.rust-workspace/s4-v1",
    "normalization": "codenoesis.normalization/rust-workspace/v1",
    "ontology": "codenoesis.ontology/rust/v2",
    "extraction_contract": "codenoesis.extraction/v2",
    "chunk_schema": "codenoesis.extraction-chunk/v2",
    "graph_schema": "codenoesis.knowledge-graph/v2",
    "snapshot_schema": "codenoesis.repository-snapshot/v4",
    "pipeline": "codenoesis.pipeline/s4-v1",
    "evidence_lineage": "codenoesis.evidence-lineage/v2",
    "renderer": "codenoesis.renderer/markdown-v1",
    "dependency_rules": "codenoesis.incremental-rules/rust-workspace-v1",
}

EXPECTED_INCREMENTAL_CODES = {
    "incremental.baseline_missing",
    "incremental.baseline_repository_mismatch",
    "incremental.baseline_incompatible",
    "incremental.cache_corrupt",
    "incremental.limit_exceeded",
    "incremental.cold_equivalence_failed",
}

S5_BUNDLE_FILES = {
    "LICENSE",
    "docs/software/decisions/0008-s5-incremental-refresh-contract.md",
    "scripts/tests/test_s5_incremental_refresh_contract.py",
    "tests/fixtures/s5/incremental-refresh-v1/README.md",
    "tests/fixtures/s5/incremental-refresh-v1/expected-cold-artifacts.json",
    "tests/fixtures/s5/incremental-refresh-v1/expected-incremental-refresh-report.json",
    "tests/fixtures/s5/incremental-refresh-v1/manifest.json",
    "tests/fixtures/s5/incremental-refresh-v1/revision-a/Cargo.toml",
    "tests/fixtures/s5/incremental-refresh-v1/revision-a/crates/app/Cargo.toml",
    "tests/fixtures/s5/incremental-refresh-v1/revision-a/crates/app/build.rs",
    "tests/fixtures/s5/incremental-refresh-v1/revision-a/crates/app/src/main.rs",
    "tests/fixtures/s5/incremental-refresh-v1/revision-a/crates/model/Cargo.toml",
    "tests/fixtures/s5/incremental-refresh-v1/revision-a/crates/model/src/item.rs",
    "tests/fixtures/s5/incremental-refresh-v1/revision-a/crates/model/src/lib.rs",
    "tests/fixtures/s5/incremental-refresh-v1/revision-b/Cargo.toml",
    "tests/fixtures/s5/incremental-refresh-v1/revision-b/crates/app/Cargo.toml",
    "tests/fixtures/s5/incremental-refresh-v1/revision-b/crates/app/build.rs",
    "tests/fixtures/s5/incremental-refresh-v1/revision-b/crates/app/src/main.rs",
    "tests/fixtures/s5/incremental-refresh-v1/revision-b/crates/model/Cargo.toml",
    "tests/fixtures/s5/incremental-refresh-v1/revision-b/crates/model/src/item.rs",
    "tests/fixtures/s5/incremental-refresh-v1/revision-b/crates/model/src/lib.rs",
    "tests/specifications/s4/contract-bundle.json",
    "tests/specifications/s5/analysis-cache-entry-v1.schema.json",
    "tests/specifications/s5/codenoesis-error-v7.schema.json",
    "tests/specifications/s5/e2e_fr_inc_001_incremental_refresh.json",
    "tests/specifications/s5/incremental-refresh-report-v1.schema.json",
    "tests/specifications/s5/incremental-rule-catalog-v1.json",
}


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def assert_sorted_unique(
    case: unittest.TestCase, values: list[str], label: str
) -> None:
    case.assertEqual(values, sorted(values), f"{label} must be sorted")
    case.assertEqual(len(values), len(set(values)), f"{label} must be unique")


def object_schemas(value: Any) -> list[dict[str, Any]]:
    found: list[dict[str, Any]] = []
    if isinstance(value, dict):
        if value.get("type") == "object":
            found.append(value)
        for child in value.values():
            found.extend(object_schemas(child))
    elif isinstance(value, list):
        for child in value:
            found.extend(object_schemas(child))
    return found


def all_keys(value: Any) -> set[str]:
    keys: set[str] = set()
    if isinstance(value, dict):
        keys.update(value)
        for child in value.values():
            keys.update(all_keys(child))
    elif isinstance(value, list):
        for child in value:
            keys.update(all_keys(child))
    return keys


def git_tree_oid(entries: list[dict[str, str]]) -> str:
    payload = b""
    for entry in entries:
        mode = entry["mode"].lstrip("0")
        payload += (
            mode.encode()
            + b" "
            + entry["name"].encode()
            + b"\0"
            + bytes.fromhex(entry["oid"])
        )
    return git_oid("tree", payload)


def cache_entry_id(
    repository_identity: str,
    mapping: dict[str, str],
    blob_field: str,
) -> str:
    preimage = [
        "codenoesis.analysis-cache-entry-id/rust-workspace/v1",
        repository_identity,
        mapping["source_file_id"],
        mapping["path"],
        mapping[blob_field],
        mapping["crate_id"],
        mapping["module_path"],
        EXPECTED_VERSIONS["cache_schema"],
        EXPECTED_VERSIONS["language_extractor"],
        EXPECTED_VERSIONS["workspace_mapper"],
        EXPECTED_VERSIONS["normalization"],
        EXPECTED_VERSIONS["ontology"],
        EXPECTED_VERSIONS["extraction_contract"],
        "standard-local-s4",
        EXPECTED_VERSIONS["dependency_rules"],
    ]
    digest = blake3_256(canonical_json(preimage))
    return f"urn:codenoesis:analysis-cache-entry:blake3:{digest}"


def report_semantic_hash(report: dict[str, Any]) -> str:
    payload = copy.deepcopy(report)
    payload.pop("semantic_hash")
    return blake3_256(
        b"codenoesis.incremental-refresh-report.semantic.v1\0"
        + canonical_json(payload)
    )


class S5IncrementalRefreshContractTests(unittest.TestCase):
    def test_ratification_register_and_srs_are_exact(self) -> None:
        srs = SRS_PATH.read_text(encoding="utf-8")
        decision = DECISION_PATH.read_text(encoding="utf-8")
        acceptance = load_json(ACCEPTANCE_PATH)

        self.assertIn(
            "### 2.10 S5 deterministic incremental refresh "
            "ratification register",
            srs,
        )
        register = srs.split(
            "### 2.10 S5 deterministic incremental refresh "
            "ratification register",
            1,
        )[1].split("## 3. Product intent and success definition", 1)[0]
        rows = re.findall(
            r"^\| `([A-Z]+-[A-Z]+-\d{3})` \| "
            r"`Proposed` \(pending protected merge\) \| `Approved` \|",
            register,
            flags=re.MULTILINE,
        )
        self.assertEqual(rows, S5_REQUIREMENTS)
        for requirement in S5_REQUIREMENTS:
            self.assertEqual(register.count(f"| `{requirement}` |"), 1)

        approval = acceptance["approval_pull_request"]
        if approval == "TBD":
            self.assertEqual(register.count("protected pull request `TBD`"), 5)
        else:
            match = re.fullmatch(
                r"https://github\.com/smutti/codenoesis/pull/([1-9][0-9]*)",
                approval,
            )
            self.assertIsNotNone(match)
            pull_number = match.group(1)  # type: ignore[union-attr]
            reference = (
                f"[PR #{pull_number} protected merge record]({approval})"
            )
            self.assertEqual(register.count(reference), 5)
            self.assertIn(approval, decision)

        self.assertIn("Issue | [#66]", decision)
        self.assertIn(
            "Requirements | `INV-INC-001`, `FR-INC-001`, `FR-INC-002`, "
            "`FR-INC-003`, `FR-CLI-004`",
            decision,
        )
        self.assertIn("Scope | Governance only", decision)
        self.assertIn("standard-local-s5", srs)
        self.assertIn("AnalysisCacheEntryV1", srs)
        self.assertIn("IncrementalRefreshReportV1", srs)
        self.assertIn("CodeNoesisErrorV7", srs)
        self.assertIn("An equal tree under a different", srs)
        self.assertIn("commit is not a no-op", srs)
        self.assertIn(
            "| `FR-CLI-004` | P1 | `0.2` | The local CLI MUST provide "
            "`noesis refresh`",
            srs,
        )
        self.assertIn(
            "| `S5` Incremental refresh | One mapped non-root Rust source "
            "edit reparses only that source",
            srs,
        )
        self.assertNotIn(
            "reuse the app and model-root chunks byte-identically",
            (FIXTURE_ROOT / "README.md").read_text(encoding="utf-8"),
        )

    def test_schemas_are_closed_bounded_and_revision_neutral(self) -> None:
        cache_schema = load_json(CACHE_SCHEMA_PATH)
        report_schema = load_json(REPORT_SCHEMA_PATH)
        error_schema = load_json(ERROR_SCHEMA_PATH)

        for path, schema in (
            (CACHE_SCHEMA_PATH, cache_schema),
            (REPORT_SCHEMA_PATH, report_schema),
            (ERROR_SCHEMA_PATH, error_schema),
        ):
            for object_schema in object_schemas(schema):
                self.assertIs(
                    object_schema.get("additionalProperties"),
                    False,
                    f"{path} contains an open object schema",
                )

        self.assertEqual(
            cache_schema["properties"]["schema_version"]["const"],
            EXPECTED_VERSIONS["cache_schema"],
        )
        cache_text = CACHE_SCHEMA_PATH.read_text(encoding="utf-8")
        for forbidden in (
            '"commit_oid"',
            '"tree_oid"',
            '"snapshot_id"',
            '"evidence_id"',
            '"claim_id"',
            '"relationship_id"',
            '"chunk_id"',
            '"document_id"',
            '"statement_id"',
            '"report_id"',
        ):
            self.assertNotIn(forbidden, cache_text)
        self.assertIn('"source_file_id"', cache_text)
        self.assertIn('"blob_oid"', cache_text)
        self.assertIn('"payload_hash"', cache_text)
        self.assertEqual(
            cache_schema["$defs"]["observations"]["properties"]["entities"][
                "maxItems"
            ],
            EXPECTED_LIMITS["analysis_entries"],
        )
        self.assertEqual(
            cache_schema["$defs"]["observations"]["properties"][
                "relationships"
            ]["maxItems"],
            EXPECTED_LIMITS["dependency_edges"],
        )

        self.assertEqual(
            report_schema["properties"]["schema_version"]["const"],
            "codenoesis.incremental-refresh-report/v1",
        )
        self.assertEqual(
            report_schema["properties"]["changed_paths"]["maxItems"],
            EXPECTED_LIMITS["changed_paths"],
        )
        self.assertEqual(
            report_schema["$defs"]["cache_entry_id_list"]["maxItems"],
            EXPECTED_LIMITS["analysis_entries"],
        )
        self.assertEqual(
            report_schema["$defs"]["identity_list"]["maxItems"],
            EXPECTED_LIMITS["report_subject_ids"],
        )
        self.assertNotIn(
            "duration",
            report_schema["properties"],
        )
        self.assertNotIn(
            "created_at",
            report_schema["properties"],
        )

        error_codes = set(error_schema["properties"]["code"]["enum"])
        self.assertTrue(EXPECTED_INCREMENTAL_CODES.issubset(error_codes))
        retryable = set(
            error_schema["allOf"][0]["if"]["properties"]["code"]["enum"]
        )
        self.assertEqual(
            retryable, {"publication.head_conflict", "storage.writer_busy"}
        )
        limit_values = set(
            error_schema["properties"]["context"]["properties"]["limit"][
                "enum"
            ]
        )
        self.assertEqual(limit_values, set(EXPECTED_LIMITS))

    def test_rule_catalog_is_conservative_and_exact(self) -> None:
        rules = load_json(RULES_PATH)
        self.assertEqual(
            rules["catalog_version"],
            EXPECTED_VERSIONS["dependency_rules"],
        )
        self.assertEqual(
            rules["outcome_precedence"],
            [
                "error",
                "full_rebuild",
                "full_workspace_analysis",
                "partial_analysis",
                "inventory_only",
                "no_change",
            ],
        )
        self.assertEqual(
            [rule["id"] for rule in rules["rules"]],
            [
                "INC-RULE-001",
                "INC-RULE-002",
                "INC-RULE-003",
                "INC-RULE-004",
                "INC-RULE-005",
                "INC-RULE-006",
                "INC-RULE-007",
            ],
        )
        priorities = [rule["priority"] for rule in rules["rules"]]
        self.assertEqual(priorities, sorted(priorities, reverse=True))
        self.assertEqual(len(priorities), len(set(priorities)))

        by_id = {rule["id"]: rule for rule in rules["rules"]}
        self.assertEqual(by_id["INC-RULE-004"]["outcome"], "partial_analysis")
        self.assertEqual(
            set(by_id["INC-RULE-004"]["when_all"]),
            {
                "mapped_non_root_rust_source_modified",
                "repository_identity_unchanged",
                "source_identity_unchanged",
                "source_path_unchanged",
                "crate_ownership_unchanged",
                "module_mapping_unchanged",
                "all_cache_key_versions_match",
            },
        )
        self.assertIn(
            "public_schema_version_changed",
            by_id["INC-RULE-002"]["when_any"],
        )
        self.assertIn(
            "source_mapping_ambiguous",
            by_id["INC-RULE-003"]["when_any"],
        )
        self.assertEqual(
            by_id["INC-RULE-006"]["when_all"][0],
            "requested_commit_equals_visible_commit",
        )
        self.assertEqual(
            rules["equal_tree_different_commit"],
            "public_rematerialization_required",
        )
        self.assertFalse(
            rules["public_materialization"][
                "baseline_public_chunk_copy_permitted"
            ]
        )
        self.assertTrue(
            rules["public_materialization"][
                "target_evidence_commit_must_equal_target"
            ]
        )
        self.assertEqual(rules["rename_semantics"], "delete_plus_add")

    def test_fixture_manifest_binds_git_objects_and_cache_keys(self) -> None:
        manifest = load_json(MANIFEST_PATH)
        self.assertEqual(
            manifest["schema_version"],
            "codenoesis.s5-fixture-manifest/v1",
        )
        self.assertEqual(manifest["versions"], EXPECTED_VERSIONS)
        self.assertEqual(
            [revision["name"] for revision in manifest["revisions"]],
            ["revision-a", "revision-b"],
        )

        revisions: dict[str, dict[str, Any]] = {}
        for revision in manifest["revisions"]:
            revisions[revision["name"]] = revision
            fixture_revision = FIXTURE_ROOT / revision["name"]
            files = revision["files"]
            paths = [item["path"] for item in files]
            assert_sorted_unique(self, paths, f"{revision['name']} files")
            actual_paths = sorted(
                path.relative_to(fixture_revision).as_posix()
                for path in fixture_revision.rglob("*")
                if path.is_file()
            )
            self.assertEqual(paths, actual_paths)
            for item in files:
                relative = Path(item["path"])
                self.assertFalse(relative.is_absolute())
                self.assertNotIn("..", relative.parts)
                source = fixture_revision / relative
                self.assertEqual(sha256(source), item["sha256"])
                self.assertEqual(
                    git_oid("blob", source.read_bytes()),
                    item["git_blob_oid"],
                )

            trees = revision["trees"]
            tree_paths = [item["path"] for item in trees]
            assert_sorted_unique(
                self, tree_paths, f"{revision['name']} tree paths"
            )
            for tree in trees:
                entry_names = [item["name"] for item in tree["entries"]]
                assert_sorted_unique(
                    self,
                    entry_names,
                    f"{revision['name']} tree {tree['path']}",
                )
                self.assertEqual(git_tree_oid(tree["entries"]), tree["oid"])
            self.assertEqual(trees[0]["path"], "")
            self.assertEqual(trees[0]["oid"], revision["root_tree_oid"])

            commit = revision["commit"]
            self.assertEqual(
                git_oid("commit", commit["payload_utf8"].encode()),
                commit["oid"],
            )
            self.assertTrue(
                commit["payload_utf8"].startswith(
                    f"tree {revision['root_tree_oid']}\n"
                )
            )

        baseline = revisions["revision-a"]
        target = revisions["revision-b"]
        self.assertIn(
            f"parent {baseline['commit']['oid']}\n",
            target["commit"]["payload_utf8"],
        )
        self.assertNotIn("parent ", baseline["commit"]["payload_utf8"])

        baseline_files = {
            item["path"]: (FIXTURE_ROOT / "revision-a" / item["path"]).read_bytes()
            for item in baseline["files"]
        }
        target_files = {
            item["path"]: (FIXTURE_ROOT / "revision-b" / item["path"]).read_bytes()
            for item in target["files"]
        }
        changed = sorted(
            path
            for path in baseline_files.keys() | target_files.keys()
            if baseline_files.get(path) != target_files.get(path)
        )
        self.assertEqual(changed, ["crates/model/src/item.rs"])
        self.assertEqual(
            manifest["diff"],
            {
                "baseline_commit_oid": baseline["commit"]["oid"],
                "target_commit_oid": target["commit"]["oid"],
                "paths": [
                    {
                        "path": "crates/model/src/item.rs",
                        "change_kind": "modified",
                        "baseline_blob_oid": (
                            "885b4746097a67e5c4fb997a2082597dc23699e6"
                        ),
                        "target_blob_oid": (
                            "ab58a9d6417ab6852d5311994e480fba6185002f"
                        ),
                    }
                ],
                "rename_inference": False,
            },
        )

        mappings = manifest["source_mappings"]
        mapping_paths = [mapping["path"] for mapping in mappings]
        assert_sorted_unique(self, mapping_paths, "source mappings")
        self.assertEqual(
            mapping_paths,
            [
                "crates/app/src/main.rs",
                "crates/model/src/item.rs",
                "crates/model/src/lib.rs",
            ],
        )
        for mapping in mappings:
            self.assertEqual(
                cache_entry_id(
                    manifest["repository_identity"],
                    mapping,
                    "baseline_blob_oid",
                ),
                mapping["baseline_cache_entry_id"],
            )
            self.assertEqual(
                cache_entry_id(
                    manifest["repository_identity"],
                    mapping,
                    "target_blob_oid",
                ),
                mapping["target_cache_entry_id"],
            )
            if mapping["baseline_blob_oid"] == mapping["target_blob_oid"]:
                self.assertEqual(
                    mapping["baseline_cache_entry_id"],
                    mapping["target_cache_entry_id"],
                )
            else:
                self.assertNotEqual(
                    mapping["baseline_cache_entry_id"],
                    mapping["target_cache_entry_id"],
                )

        changed_mapping = next(
            item
            for item in mappings
            if item["path"] == "crates/model/src/item.rs"
        )
        self.assertEqual(changed_mapping["module_path"], "crate::item")
        sentinel = manifest["sentinels"]
        self.assertEqual(sentinel["build_script"], "crates/app/build.rs")
        self.assertFalse(sentinel["must_execute"])
        for key, value in sentinel.items():
            if key != "build_script":
                self.assertFalse(value, f"{key} must remain disabled")
        sentinel_a = (
            FIXTURE_ROOT / "revision-a" / sentinel["build_script"]
        ).read_bytes()
        sentinel_b = (
            FIXTURE_ROOT / "revision-b" / sentinel["build_script"]
        ).read_bytes()
        self.assertEqual(sentinel_a, sentinel_b)
        self.assertIn(b"must never execute", sentinel_a)

    def test_reviewed_cold_artifacts_and_report_are_exact(self) -> None:
        manifest = load_json(MANIFEST_PATH)
        cold = load_json(COLD_PATH)
        report = load_json(REPORT_PATH)
        s4_bundle = load_json(S4_BUNDLE_PATH)

        self.assertEqual(
            cold["inherited_s4_contract_bundle_sha256"],
            s4_bundle["bundle_sha256"],
        )
        self.assertEqual(
            s4_bundle["bundle_sha256"],
            "3efb380fb058a5831123a0f990676575da04e60998cada8987f034675b61f12e",
        )
        self.assertEqual(cold["repository_identity"], manifest["repository_identity"])
        self.assertEqual(
            cold["target_configuration_hash"],
            report["target_configuration_hash"],
        )

        cold_by_name = {
            revision["name"]: revision for revision in cold["revisions"]
        }
        manifest_by_name = {
            revision["name"]: revision
            for revision in manifest["revisions"]
        }
        self.assertEqual(set(cold_by_name), {"revision-a", "revision-b"})
        for name in ("revision-a", "revision-b"):
            self.assertEqual(
                cold_by_name[name]["commit_oid"],
                manifest_by_name[name]["commit"]["oid"],
            )
            self.assertEqual(
                cold_by_name[name]["tree_oid"],
                manifest_by_name[name]["root_tree_oid"],
            )
            self.assertEqual(cold_by_name[name]["graph_counts"]["chunks"], 3)
            chunk_ids = [
                chunk["source_file_id"]
                for chunk in cold_by_name[name]["chunks"]
            ]
            self.assertEqual(
                set(chunk_ids),
                {
                    mapping["source_file_id"]
                    for mapping in manifest["source_mappings"]
                },
            )
            document_paths = [
                document["path"]
                for document in cold_by_name[name]["documents"]
            ]
            self.assertEqual(
                document_paths,
                [
                    "modules/app.md",
                    "modules/model-item.md",
                    "modules/model.md",
                    "overview.md",
                ],
            )

        baseline = cold_by_name["revision-a"]
        target = cold_by_name["revision-b"]
        self.assertEqual(
            target["snapshot_semantic_hash"]["value"],
            "5526646a790a72eb6efd0824a28d727ecfdc9d9a409519f6f8a550ddeee34131",
        )
        self.assertEqual(
            target["graph_semantic_hash"]["value"],
            "2dfc8c43e3f950a9ea526b51c7cacafff58179242d105c24de9521906d6f7f59",
        )
        self.assertEqual(
            target["documentation_generation_hash"]["value"],
            "3aa97344bc07e32378f62fa5b24f3469f6d919d69093fd0575b371d9e92be7b2",
        )

        self.assertEqual(set(report), set(load_json(REPORT_SCHEMA_PATH)["required"]))
        self.assertEqual(report["versions"], EXPECTED_VERSIONS)
        self.assertEqual(report["repository_identity"], manifest["repository_identity"])
        self.assertEqual(report["rule"]["outcome"], "partial_analysis")
        self.assertEqual(report["rule"]["rule_ids"], ["INC-RULE-004"])
        self.assertEqual(report["baseline"]["commit_oid"], baseline["commit_oid"])
        self.assertEqual(report["target"]["commit_oid"], target["commit_oid"])
        self.assertEqual(
            report["changed_paths"],
            manifest["diff"]["paths"],
        )

        mappings = manifest["source_mappings"]
        analysis = report["analysis"]
        self.assertEqual(
            analysis["baseline_entry_ids"],
            sorted(mapping["baseline_cache_entry_id"] for mapping in mappings),
        )
        self.assertEqual(
            analysis["target_entry_ids"],
            sorted(mapping["target_cache_entry_id"] for mapping in mappings),
        )
        unchanged = [
            mapping
            for mapping in mappings
            if mapping["baseline_blob_oid"] == mapping["target_blob_oid"]
        ]
        changed = [
            mapping
            for mapping in mappings
            if mapping["baseline_blob_oid"] != mapping["target_blob_oid"]
        ]
        self.assertEqual(
            analysis["reused_entry_ids"],
            sorted(mapping["target_cache_entry_id"] for mapping in unchanged),
        )
        self.assertEqual(
            analysis["invalidated_entry_ids"],
            sorted(mapping["baseline_cache_entry_id"] for mapping in changed),
        )
        self.assertEqual(
            analysis["recomputed_entry_ids"],
            sorted(mapping["target_cache_entry_id"] for mapping in changed),
        )
        for key, values in analysis.items():
            assert_sorted_unique(self, values, f"analysis {key}")

        inventory = report["inventory"]
        for key, paths in inventory.items():
            assert_sorted_unique(self, paths, f"inventory {key}")
        self.assertEqual(
            set(inventory["reused_classification_paths"])
            | set(inventory["reclassified_paths"]),
            {
                item["path"]
                for item in manifest_by_name["revision-a"]["files"]
            },
        )
        self.assertEqual(
            inventory["reclassified_paths"], ["crates/model/src/item.rs"]
        )

        baseline_chunks = {
            chunk["source_file_id"]: chunk for chunk in baseline["chunks"]
        }
        target_chunks = {
            chunk["source_file_id"]: chunk for chunk in target["chunks"]
        }
        report_chunks = report["public_rematerialization"]["chunks"]
        chunk_ids = [chunk["source_file_id"] for chunk in report_chunks]
        assert_sorted_unique(self, chunk_ids, "report chunks")
        self.assertEqual(set(chunk_ids), set(baseline_chunks))
        for chunk in report_chunks:
            source_id = chunk["source_file_id"]
            self.assertEqual(
                chunk["baseline_semantic_hash"]["value"],
                baseline_chunks[source_id]["semantic_hash"],
            )
            self.assertEqual(
                chunk["target_semantic_hash"]["value"],
                target_chunks[source_id]["semantic_hash"],
            )
            self.assertNotEqual(
                chunk["baseline_semantic_hash"],
                chunk["target_semantic_hash"],
            )
        self.assertFalse(
            report["public_rematerialization"][
                "baseline_public_chunk_copy_permitted"
            ]
        )
        self.assertTrue(
            report["public_rematerialization"][
                "all_target_evidence_uses_target_commit"
            ]
        )

        baseline_documents = {
            document["document_id"]: document
            for document in baseline["documents"]
        }
        target_documents = {
            document["document_id"]: document
            for document in target["documents"]
        }
        report_documents = report["public_rematerialization"]["documents"]
        document_ids = [
            document["document_id"] for document in report_documents
        ]
        assert_sorted_unique(self, document_ids, "report documents")
        self.assertEqual(set(document_ids), set(baseline_documents))
        for document in report_documents:
            document_id = document["document_id"]
            self.assertEqual(
                document["baseline_blake3"],
                baseline_documents[document_id]["blake3"],
            )
            self.assertEqual(
                document["target_blake3"],
                target_documents[document_id]["blake3"],
            )
            self.assertTrue(document["manifest_rematerialized"])
            self.assertEqual(
                document["content_changed"],
                document["baseline_blake3"] != document["target_blake3"],
            )

        invalidation = report["invalidation"]
        expected_lengths = {
            "entities": (12, 13, 12, 1, 1, 0),
            "relationships": (12, 13, 12, 12, 1, 0),
            "claims": (24, 26, 24, 24, 2, 0),
            "evidence": (6, 6, 0, 6, 6, 6),
            "coverage_gaps": (2, 2, 0, 2, 2, 2),
            "documents": (4, 4, 4, 4, 0, 0),
            "federation_links": (0, 0, 0, 0, 0, 0),
        }
        for category, delta in invalidation.items():
            for key in ("invalidated_ids", "added_ids", "removed_ids"):
                assert_sorted_unique(
                    self, delta[key], f"{category} {key}"
                )
            self.assertEqual(
                (
                    delta["baseline_count"],
                    delta["target_count"],
                    delta["retained_count"],
                    len(delta["invalidated_ids"]),
                    len(delta["added_ids"]),
                    len(delta["removed_ids"]),
                ),
                expected_lengths[category],
            )
        self.assertEqual(
            invalidation["evidence"]["invalidated_ids"],
            invalidation["evidence"]["removed_ids"],
        )
        self.assertEqual(
            invalidation["coverage_gaps"]["invalidated_ids"],
            invalidation["coverage_gaps"]["removed_ids"],
        )

        cold_equivalence = report["cold_equivalence"]
        self.assertTrue(cold_equivalence["semantic_bytes_equal"])
        for key, expected in (
            ("snapshot", target["snapshot_semantic_hash"]),
            ("graph", target["graph_semantic_hash"]),
            ("documentation", target["documentation_generation_hash"]),
        ):
            self.assertTrue(cold_equivalence[key]["equal"])
            self.assertEqual(cold_equivalence[key]["incremental"], expected)
            self.assertEqual(cold_equivalence[key]["cold"], expected)

        metrics = report["metrics"]
        self.assertEqual(
            metrics,
            {
                "changed_path_count": 1,
                "analysis_entry_count": 3,
                "cache_hit_count": 2,
                "cache_miss_count": 1,
                "cache_invalidated_count": 1,
                "parser_invocation_count": 1,
                "dependency_edge_count": 0,
                "rematerialized_chunk_count": 3,
                "rematerialized_document_manifest_count": 4,
                "changed_document_content_count": 3,
                "report_subject_id_count": 79,
            },
        )
        subject_occurrences = sum(
            len(values) for values in analysis.values()
        ) + sum(
            len(delta[key])
            for delta in invalidation.values()
            for key in ("invalidated_ids", "added_ids", "removed_ids")
        )
        self.assertEqual(metrics["report_subject_id_count"], subject_occurrences)
        self.assertLessEqual(
            len(canonical_json(report)), EXPECTED_LIMITS["report_bytes"]
        )

        self.assertEqual(
            report["semantic_hash"],
            {
                "algorithm": "blake3-256",
                "value": report_semantic_hash(report),
            },
        )
        self.assertEqual(
            report["semantic_hash"]["value"],
            "698ae3084d8d6e586080039f017476b357c473759b65423a73a4525e2cc94e40",
        )
        forbidden_volatile_keys = {
            "created_at",
            "duration",
            "duration_ms",
            "wall_time",
            "job_id",
            "correlation_id",
            "process_id",
            "host_path",
            "retry_count",
        }
        self.assertTrue(forbidden_volatile_keys.isdisjoint(all_keys(report)))

    def test_acceptance_oracle_is_ready_for_future_red_only(self) -> None:
        acceptance = load_json(ACCEPTANCE_PATH)
        report = load_json(REPORT_PATH)
        self.assertEqual(
            acceptance["schema_version"],
            "codenoesis.acceptance-specification/v1",
        )
        self.assertEqual(
            acceptance["issue"],
            "https://github.com/smutti/codenoesis/issues/66",
        )
        self.assertEqual(
            [item["id"] for item in acceptance["requirements"]],
            S5_REQUIREMENTS,
        )
        for item in acceptance["requirements"]:
            self.assertEqual(item["current_state"], "Proposed")
            self.assertEqual(item["target_state"], "Approved")
        self.assertEqual(acceptance["slice"], "S5")
        self.assertEqual(acceptance["risk"]["level"], "high")
        self.assertFalse(acceptance["runtime_implementation_authorized"])
        self.assertEqual(
            acceptance["operation"]["command"],
            "noesis refresh --repository <path> --repository-id <id> "
            "--revision <rev> --store <path> --profile standard-local-s5",
        )

        expected_red = acceptance["expected_red"]
        self.assertEqual(
            expected_red["test_name"],
            "e2e_fr_inc_001_incremental_refresh",
        )
        self.assertEqual(
            expected_red["command"],
            "cargo test -p noesis --test "
            "e2e_fr_inc_001_incremental_refresh --locked -- --exact "
            "e2e_fr_inc_001_incremental_refresh",
        )
        self.assertEqual(
            expected_red["accepted_error_code"], "input.invalid_profile"
        )
        self.assertFalse(expected_red["run_in_governance_change"])
        self.assertIn(
            "two cold scans mislabeled as incremental reuse",
            expected_red["rejected_failures"],
        )
        self.assertIn(
            "baseline public chunk bytes copied with stale commit evidence",
            expected_red["rejected_failures"],
        )

        self.assertEqual(acceptance["limits"], EXPECTED_LIMITS)
        self.assertEqual(
            [item["id"] for item in acceptance["tests"]],
            S5_TEST_ORDER,
        )
        traced = {
            requirement
            for test in acceptance["tests"]
            for requirement in test["requirements"]
        }
        self.assertEqual(traced, set(S5_REQUIREMENTS))
        self.assertEqual(
            acceptance["oracle"]["report"]["semantic_hash"],
            report["semantic_hash"]["value"],
        )
        self.assertEqual(
            acceptance["oracle"]["report"][
                "subject_id_occurrences_across_set_fields"
            ],
            report["metrics"]["report_subject_id_count"],
        )
        self.assertFalse(
            acceptance["oracle"]["no_change"][
                "equal_tree_under_different_commit_is_no_change"
            ]
        )
        self.assertEqual(
            acceptance["failure_oracles"]["concurrent_head_movement"],
            "publication.head_conflict",
        )
        self.assertEqual(
            acceptance["allowed_paths"],
            [
                "docs/software/software-requirements-specification.md",
                "docs/software/decisions/"
                "0008-s5-incremental-refresh-contract.md",
                "tests/specifications/s5",
                "tests/fixtures/s5/incremental-refresh-v1",
                "scripts/tests/test_s5_incremental_refresh_contract.py",
            ],
        )
        self.assertIn(
            "baseline public chunk bytes reused for another target commit",
            acceptance["forbidden"],
        )
        self.assertIn(
            "commit-bound public identity inside AnalysisCacheEntryV1",
            acceptance["forbidden"],
        )

    def test_contract_bundle_binds_every_s5_artifact(self) -> None:
        bundle = load_json(BUNDLE_PATH)
        self.assertEqual(
            set(bundle), {"schema_version", "files", "bundle_sha256"}
        )
        self.assertEqual(
            bundle["schema_version"], "codenoesis.contract-bundle/v1"
        )
        files = bundle["files"]
        paths = [item["path"] for item in files]
        assert_sorted_unique(self, paths, "bundle paths")
        self.assertEqual(set(paths), S5_BUNDLE_FILES)
        for item in files:
            self.assertEqual(set(item), {"path", "sha256"})
            path = Path(item["path"])
            self.assertFalse(path.is_absolute())
            self.assertNotIn("..", path.parts)
            self.assertRegex(item["sha256"], r"^[0-9a-f]{64}$")
            self.assertEqual(sha256(ROOT / path), item["sha256"])

        payload = {
            "schema_version": bundle["schema_version"],
            "files": files,
        }
        bundle_sha256 = hashlib.sha256(canonical_json(payload)).hexdigest()
        self.assertEqual(bundle["bundle_sha256"], bundle_sha256)
        srs = SRS_PATH.read_text(encoding="utf-8")
        match = re.search(
            r"S5 deterministic incremental refresh contract bundle:\s+"
            r"`sha256:([0-9a-f]{64})`",
            srs,
        )
        self.assertIsNotNone(match, "SRS must bind the complete S5 bundle")
        self.assertEqual(match.group(1), bundle_sha256)  # type: ignore[union-attr]


if __name__ == "__main__":
    unittest.main()
