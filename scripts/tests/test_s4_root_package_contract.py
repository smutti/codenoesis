from __future__ import annotations

import hashlib
import json
import re
import unittest
from pathlib import Path
from typing import Any, Iterator
from urllib.parse import unquote

from scripts.tests.test_s1_contract import blake3_256


ROOT = Path(__file__).resolve().parents[2]
SRS_PATH = ROOT / "docs/software/software-requirements-specification.md"
DECISION_PATH = (
    ROOT / "docs/software/decisions/0011-s4-root-package-workspace-contract.md"
)
SPEC_ROOT = ROOT / "tests/specifications/s4/r3"
ORACLE_PATH = SPEC_ROOT / "e2e_fr_ext_008_root_package_workspace.json"
BUNDLE_PATH = SPEC_ROOT / "contract-bundle.json"
CONFIGURATION_SCHEMA_PATH = SPEC_ROOT / "configuration-v3.schema.json"
SNAPSHOT_SCHEMA_PATH = SPEC_ROOT / "repository-snapshot-v6.schema.json"
CHUNK_SCHEMA_PATH = SPEC_ROOT / "extraction-chunk-v3.schema.json"
GRAPH_SCHEMA_PATH = SPEC_ROOT / "knowledge-graph-v3.schema.json"
ONTOLOGY_PATH = SPEC_ROOT / "rust-ontology-v3.json"
ERROR_SCHEMA_PATH = SPEC_ROOT / "codenoesis-error-v10.schema.json"
HASH_CONTRACT_PATH = SPEC_ROOT / "semantic-hash-contract-v2.json"
FIXTURE_ROOT = ROOT / "tests/fixtures/s4/root-package-workspace-v1"
FIXTURE_MANIFEST_PATH = FIXTURE_ROOT / "manifest.json"
EXPECTED_PLAN_PATH = FIXTURE_ROOT / "expected-workspace-plan.json"

ISSUE_REFERENCE = "https://github.com/smutti/codenoesis/issues/96"
AUTHORIZATION_REFERENCE = (
    "https://github.com/smutti/codenoesis/issues/96#issuecomment-5158164180"
)
CORRECTION_REFERENCES = (
    "https://github.com/smutti/codenoesis/issues/96#issuecomment-5158197452",
    "https://github.com/smutti/codenoesis/issues/96#issuecomment-5158328256",
)
APPROVAL_REFERENCE = "https://github.com/smutti/codenoesis/pull/97"
REQUIRED_BASE = "36e8fefe37e21ac936dedda9198465d655cebeba"
REPOSITORY_IDENTITY = (
    "urn:codenoesis:fixture:s4-root-package-workspace-v1"
)
GITLINK_OID = "6ecf94267842da776e35406a9ebcb85e058a3181"

LIMITS = {
    "single_manifest_bytes": 4_194_304,
    "workspace_members": 200,
    "projected_workspace_members": 201,
    "workspace_exclusions": 200,
    "package_manifests": 200,
    "workspace_crates": 200,
    "binary_roots_per_package": 64,
    "permutations": 50,
}

ERROR_CODES = (
    "input.invalid_workspace_profile",
    "extraction.invalid_workspace_manifest",
    "extraction.workspace_member_conflict",
    "extraction.workspace_target_conflict",
    "extraction.root_package_limit_exceeded",
    "internal.unexpected",
)

COVERAGE_CAPABILITIES = (
    "cargo.build_script_not_executed",
    "cargo.dependencies_deferred",
    "cargo.features_deferred",
    "cargo.package_metadata_deferred",
    "cargo.patch_deferred",
    "cargo.proc_macro_not_executed",
    "cargo.required_features_deferred",
    "cargo.target_world_deferred",
    "cargo.workspace_inheritance_deferred",
    "workspace.external_gitlink_member_not_analyzed",
)

REQUIRED_TEST_NAMES = (
    "e2e_fr_ext_008_root_package_workspace",
    "gt_fr_ext_008_root_membership_and_targets",
    "conf_fr_ext_008_snapshot_v6_graph_v3_error_v10",
    "pt_dr_idn_002_r3_preserves_v2_identity_domains",
    "pt_fr_ext_008_limits_have_max_and_plus_one",
    "pt_nfr_det_001_r3_permutation_and_schedule_invariant",
    "sec_fr_ext_008_deferred_cargo_meaning_never_executes",
    "sec_fr_ext_008_gitlink_member_stays_external",
    "reg_fr_cli_001_r3_selector_absence_is_byte_identical",
    "e2e_fr_doc_001_r3_coverage_is_documented",
    "e2e_fr_qry_001_r3_exact_id_results",
)

EXPECTED_TARGETS = (
    (
        "Cargo.toml",
        "root-app",
        "lib",
        "root_app",
        "src/lib.rs",
        "urn:codenoesis:entity:blake3:"
        "6f20a1ab8dc60551d001178172368742238429b975c2740e41cce9975f04a4dd",
    ),
    (
        "Cargo.toml",
        "root-app",
        "bin",
        "root-admin",
        "src/bin/admin.rs",
        "urn:codenoesis:entity:blake3:"
        "7619b82a3299db9236b4a5fb8955f38c8ce8bb388391448ddc2df5dbbc8e7249",
    ),
    (
        "Cargo.toml",
        "root-app",
        "bin",
        "root-app",
        "src/main.rs",
        "urn:codenoesis:entity:blake3:"
        "819b0a4083e3c4553c559735f1d0f2a07a1ad01fdeeaf180669d4e178564e0d5",
    ),
    (
        "crates/cli/Cargo.toml",
        "root-cli",
        "bin",
        "inspect",
        "crates/cli/src/bin/inspect/main.rs",
        "urn:codenoesis:entity:blake3:"
        "f6f2630c8a4b7f71c01be72c910bdbca56bae8c43bc8ff9e51489257801ba092",
    ),
    (
        "crates/cli/Cargo.toml",
        "root-cli",
        "bin",
        "root-cli",
        "crates/cli/src/main.rs",
        "urn:codenoesis:entity:blake3:"
        "ae8f827c9b5e76a5cdf198c8ca6dd2e7b480068c75b0251972158d9f5f85b253",
    ),
    (
        "crates/macro-sentinel/Cargo.toml",
        "macro-sentinel",
        "lib",
        "macro_sentinel",
        "crates/macro-sentinel/src/lib.rs",
        "urn:codenoesis:entity:blake3:"
        "e3ff23bf52b9bc47ecb50ffd231109e9f51c891444a3234c7678b8a0c37f2edd",
    ),
)

SCHEMA_PATHS = (
    CONFIGURATION_SCHEMA_PATH,
    SNAPSHOT_SCHEMA_PATH,
    CHUNK_SCHEMA_PATH,
    GRAPH_SCHEMA_PATH,
    ERROR_SCHEMA_PATH,
)

BUNDLE_FILES = {
    "LICENSE",
    "docs/software/decisions/0011-s4-root-package-workspace-contract.md",
    "scripts/tests/test_s4_root_package_contract.py",
    "tests/corpora/real-world-rust-v1.json",
    "tests/fixtures/s4/root-package-workspace-v1/README.md",
    "tests/fixtures/s4/root-package-workspace-v1/expected-workspace-plan.json",
    "tests/fixtures/s4/root-package-workspace-v1/manifest.json",
    "tests/fixtures/s4/root-package-workspace-v1/root-manifests/explicit-dot.toml",
    "tests/fixtures/s4/root-package-workspace-v1/root-manifests/implicit.toml",
    "tests/fixtures/s4/root-package-workspace-v1/root-manifests/member-exclude-conflict.toml",
    "tests/fixtures/s4/root-package-workspace-v1/root-manifests/standalone.toml",
    "tests/fixtures/s4/root-package-workspace-v1/root-manifests/virtual.toml",
    "tests/fixtures/s4/root-package-workspace-v1/shared-tree/.gitmodules",
    "tests/fixtures/s4/root-package-workspace-v1/shared-tree/build.rs",
    "tests/fixtures/s4/root-package-workspace-v1/shared-tree/crates/cli/Cargo.toml",
    "tests/fixtures/s4/root-package-workspace-v1/shared-tree/crates/cli/src/bin/inspect/main.rs",
    "tests/fixtures/s4/root-package-workspace-v1/shared-tree/crates/cli/src/main.rs",
    "tests/fixtures/s4/root-package-workspace-v1/shared-tree/crates/macro-sentinel/Cargo.toml",
    "tests/fixtures/s4/root-package-workspace-v1/shared-tree/crates/macro-sentinel/src/lib.rs",
    "tests/fixtures/s4/root-package-workspace-v1/shared-tree/src/bin/admin.rs",
    "tests/fixtures/s4/root-package-workspace-v1/shared-tree/src/lib.rs",
    "tests/fixtures/s4/root-package-workspace-v1/shared-tree/src/main.rs",
    "tests/fixtures/s4/root-package-workspace-v1/shared-tree/src/model.rs",
    "tests/specifications/s1/gitlink-boundary-contract-bundle.json",
    "tests/specifications/s4/contract-bundle.json",
    "tests/specifications/s4/rust-ontology-v2.json",
    "tests/specifications/s4/r3/codenoesis-error-v10.schema.json",
    "tests/specifications/s4/r3/configuration-v3.schema.json",
    "tests/specifications/s4/r3/e2e_fr_ext_008_root_package_workspace.json",
    "tests/specifications/s4/r3/extraction-chunk-v3.schema.json",
    "tests/specifications/s4/r3/knowledge-graph-v3.schema.json",
    "tests/specifications/s4/r3/repository-snapshot-v6.schema.json",
    "tests/specifications/s4/r3/rust-ontology-v3.json",
    "tests/specifications/s4/r3/semantic-hash-contract-v2.json",
}

IMMUTABLE_FILES = {
    "LICENSE": (
        "c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4"
    ),
    "tests/corpora/real-world-rust-v1.json": (
        "1d2edc9f858d612e76abb70e6dd255d28e88306a0e4874b0e8ea7351f4347f46"
    ),
    "tests/specifications/s1/gitlink-boundary-contract-bundle.json": (
        "710a640fd6bf43ee87cc4a4c2eb159093dbb6c6654ba4baf5fcbc3adf8e5970e"
    ),
    "tests/specifications/s4/contract-bundle.json": (
        "be199ebbeb9cb35c2e6a68c5b9d847f86fe131efd007b0d09d9fd28390c91437"
    ),
    "tests/specifications/s4/rust-ontology-v2.json": (
        "10d5a5ba797c1226f5b46377cfed9f61bbd1723a75944c0e87caa4b5877a9342"
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


def sha256_path(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def git_blob_oid(value: bytes) -> str:
    header = f"blob {len(value)}\0".encode("ascii")
    return hashlib.sha1(header + value).hexdigest()


def stable_id(domain: str, preimage: list[str]) -> str:
    digest = blake3_256(canonical_json([domain, *preimage]))
    kind = domain.split(".", 1)[1].split("-id", 1)[0]
    return f"urn:codenoesis:{kind}:blake3:{digest}"


def walk_json(value: Any) -> Iterator[dict[str, Any]]:
    if isinstance(value, dict):
        yield value
        for child in value.values():
            yield from walk_json(child)
    elif isinstance(value, list):
        for child in value:
            yield from walk_json(child)


def resolve_pointer(document: Any, fragment: str) -> Any:
    if not fragment:
        return document
    if not fragment.startswith("/"):
        raise ValueError(f"unsupported JSON pointer: {fragment}")
    current = document
    for raw_token in fragment[1:].split("/"):
        token = unquote(raw_token).replace("~1", "/").replace("~0", "~")
        if isinstance(current, list):
            current = current[int(token)]
        else:
            current = current[token]
    return current


def manifest_array(text: str, key: str) -> tuple[str, ...]:
    match = re.search(
        rf"(?m)^{re.escape(key)}\s*=\s*\[(?P<values>[^\]]*)\]\s*$",
        text,
    )
    if match is None:
        raise AssertionError(f"missing literal array {key}")
    values = re.findall(r'"([^"\\]*)"', match.group("values"))
    return tuple(values)


class S4RootPackageGovernanceTests(unittest.TestCase):
    def test_ratification_register_and_decision_are_exact(self) -> None:
        srs = SRS_PATH.read_text(encoding="utf-8")
        decision = DECISION_PATH.read_text(encoding="utf-8")
        for required in (
            "FR-EXT-008",
            ISSUE_REFERENCE,
            AUTHORIZATION_REFERENCE,
            APPROVAL_REFERENCE,
            "--workspace-profile cargo-root-package-v1",
            "codenoesis.repository-snapshot/v6",
            "codenoesis.configuration/v3",
            "codenoesis.extraction/v3",
            "codenoesis.knowledge-graph/v3",
            "codenoesis.ontology/rust/v3",
            "codenoesis.error/v10",
            REQUIRED_BASE,
        ):
            with self.subTest(required=required):
                self.assertIn(required, srs + decision)
        self.assertIn("`FR-EXT-008` | `Proposed`", srs)
        self.assertIn("manually merges the exact protected head", decision)
        self.assertIn("The authoring agent", srs)
        self.assertIn("does not approve or merge", srs)
        self.assertIn("if and only if", decision)
        self.assertIn("at most 201 members", decision)
        self.assertIn("SRS is excluded from the bundle", decision)
        self.assertNotIn("BUNDLE_SHA256_PENDING", decision)
        self.assertNotIn(
            "extraction.external_workspace_member_requires_boundary",
            decision,
        )

    def test_machine_oracle_binds_authorization_red_and_limits(self) -> None:
        oracle = load_json(ORACLE_PATH)
        self.assertEqual(oracle["issue"], ISSUE_REFERENCE)
        self.assertEqual(oracle["requirement_ids"], ["FR-EXT-008"])
        self.assertEqual(
            oracle["requirement_status"],
            {
                "current": "Proposed",
                "target_after_protected_merge": "Approved",
            },
        )
        self.assertEqual(oracle["slice"], "S4")
        self.assertEqual(oracle["roadmap_capability"], "R3")
        self.assertEqual(oracle["risk"], "high")
        self.assertEqual(oracle["required_base"], REQUIRED_BASE)
        self.assertEqual(oracle["authorization"], AUTHORIZATION_REFERENCE)
        self.assertEqual(
            oracle["governance_corrections"], list(CORRECTION_REFERENCES)
        )
        self.assertEqual(
            oracle["selector"],
            {
                "flag": "--workspace-profile",
                "value": "cargo-root-package-v1",
                "required_profile": "standard-local-s4",
                "implicit_selection": False,
                "composes_with": [
                    "local-git-sha1-packed-v1",
                    "local-gitlinks-v1",
                ],
            },
        )
        red = oracle["first_red"]
        self.assertEqual(red["subject_exit_code"], 2)
        self.assertEqual(red["stdout_bytes"], 0)
        self.assertEqual(
            red["stdout_sha256"],
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        )
        expected_stderr = (
            '{"code":"input.invalid_revision","context":{},'
            '"message":"invalid revision","retryable":false,'
            '"schema_version":"codenoesis.error/v4","stage":"input"}\n'
        )
        self.assertEqual(red["stderr"], expected_stderr)
        self.assertEqual(red["stderr_bytes"], 149)
        self.assertEqual(len(expected_stderr.encode("utf-8")), 149)
        self.assertEqual(
            hashlib.sha256(expected_stderr.encode("utf-8")).hexdigest(),
            red["stderr_sha256"],
        )
        self.assertFalse(red["store_exists"])
        self.assertEqual(oracle["limits"], LIMITS)
        self.assertEqual(tuple(oracle["error_v10"]), ERROR_CODES)
        self.assertEqual(
            oracle["error_v10"],
            {
                "input.invalid_workspace_profile": 2,
                "extraction.invalid_workspace_manifest": 11,
                "extraction.workspace_member_conflict": 11,
                "extraction.workspace_target_conflict": 11,
                "extraction.root_package_limit_exceeded": 11,
                "internal.unexpected": 70,
            },
        )
        self.assertEqual(
            tuple(oracle["required_test_names"]), REQUIRED_TEST_NAMES
        )

    def test_public_pilots_are_pinned_and_non_vendored(self) -> None:
        pilots = load_json(ORACLE_PATH)["public_pilots"]
        self.assertEqual([pilot["id"] for pilot in pilots], ["lekton", "rustdesk"])
        lekton, rustdesk = pilots
        self.assertEqual(lekton["repository"], "dghilardi/lekton")
        self.assertEqual(
            lekton["commit"],
            "7a4d1a4a30468f4c18ce158a9b825680b00f4820",
        )
        self.assertEqual(rustdesk["repository"], "rustdesk/rustdesk")
        self.assertEqual(
            rustdesk["commit"],
            "d412d198720aa56f6cfed2dfad262e8fb1322fb7",
        )
        self.assertEqual(rustdesk["boundary_path"], "libs/hbb_common")
        self.assertEqual(
            rustdesk["boundary_commit"],
            "69cea8dafee147848ae88702029f4bf7df7224c3",
        )
        self.assertFalse(rustdesk["nested_source_opened"])
        self.assertTrue(all(not pilot["vendored_source"] for pilot in pilots))

    def test_fixture_manifest_binds_every_materialized_file(self) -> None:
        manifest = load_json(FIXTURE_MANIFEST_PATH)
        self.assertEqual(
            set(manifest),
            {
                "schema_version",
                "repository_identity",
                "materialization",
                "files",
                "expected_plan",
                "external_source_vendored",
            },
        )
        self.assertEqual(
            manifest["schema_version"],
            "codenoesis.r3-fixture-manifest/v1",
        )
        self.assertEqual(manifest["repository_identity"], REPOSITORY_IDENTITY)
        self.assertFalse(manifest["external_source_vendored"])

        files = manifest["files"]
        paths = [item["path"] for item in files]
        actual_paths = sorted(
            path.relative_to(FIXTURE_ROOT).as_posix()
            for directory in ("root-manifests", "shared-tree")
            for path in (FIXTURE_ROOT / directory).rglob("*")
            if path.is_file()
        )
        self.assertEqual(paths, sorted(paths))
        self.assertEqual(paths, actual_paths)
        self.assertEqual(len(paths), len(set(paths)))
        allowed_roles = {
            "root_manifest_variant",
            "boundary_metadata",
            "package_manifest",
            "execution_sentinel",
            "rust_source",
        }
        for item in files:
            self.assertEqual(
                set(item),
                {
                    "path",
                    "role",
                    "mode",
                    "byte_length",
                    "sha256",
                    "git_blob_oid",
                },
            )
            self.assertIn(item["role"], allowed_roles)
            self.assertEqual(item["mode"], "100644")
            path = Path(item["path"])
            self.assertFalse(path.is_absolute())
            self.assertNotIn("..", path.parts)
            source = FIXTURE_ROOT / path
            self.assertFalse(source.is_symlink())
            value = source.read_bytes()
            self.assertEqual(item["byte_length"], len(value))
            self.assertEqual(
                item["sha256"], hashlib.sha256(value).hexdigest()
            )
            self.assertEqual(item["git_blob_oid"], git_blob_oid(value))

        expected = manifest["expected_plan"]
        self.assertEqual(
            set(expected), {"path", "byte_length", "sha256"}
        )
        self.assertEqual(expected["path"], "expected-workspace-plan.json")
        value = EXPECTED_PLAN_PATH.read_bytes()
        self.assertEqual(expected["byte_length"], len(value))
        self.assertEqual(
            expected["sha256"], hashlib.sha256(value).hexdigest()
        )

    def test_fixture_materialization_variants_are_closed(self) -> None:
        materialization = load_json(FIXTURE_MANIFEST_PATH)["materialization"]
        self.assertEqual(
            set(materialization),
            {
                "root_manifest_directory",
                "root_manifest_destination",
                "shared_tree_directory",
                "variants",
                "gitlink",
            },
        )
        self.assertEqual(materialization["root_manifest_destination"], "Cargo.toml")
        self.assertEqual(
            materialization["variants"],
            {
                "explicit_dot": {
                    "root_manifest": "root-manifests/explicit-dot.toml",
                    "includes_boundary": True,
                },
                "implicit": {
                    "root_manifest": "root-manifests/implicit.toml",
                    "includes_boundary": True,
                },
                "member_exclude_conflict": {
                    "root_manifest": (
                        "root-manifests/member-exclude-conflict.toml"
                    ),
                    "includes_boundary": False,
                },
                "standalone": {
                    "root_manifest": "root-manifests/standalone.toml",
                    "includes_boundary": False,
                },
                "virtual": {
                    "root_manifest": "root-manifests/virtual.toml",
                    "includes_boundary": True,
                },
            },
        )
        self.assertEqual(
            materialization["gitlink"],
            {
                "path": "external/model",
                "mode": "160000",
                "commit_oid": GITLINK_OID,
            },
        )
        self.assertFalse((FIXTURE_ROOT / "shared-tree/external/model").exists())

    def test_fixture_toml_shapes_and_sentinels_are_reviewable(self) -> None:
        root = FIXTURE_ROOT / "root-manifests"
        implicit = (root / "implicit.toml").read_text(encoding="utf-8")
        explicit = (root / "explicit-dot.toml").read_text(encoding="utf-8")
        standalone = (root / "standalone.toml").read_text(encoding="utf-8")
        virtual = (root / "virtual.toml").read_text(encoding="utf-8")
        conflict = (root / "member-exclude-conflict.toml").read_text(
            encoding="utf-8"
        )
        expected_non_root = {
            "crates/cli",
            "crates/macro-sentinel",
            "external/model",
        }
        self.assertIn("[package]", implicit)
        self.assertIn("[workspace]", implicit)
        self.assertEqual(set(manifest_array(implicit, "members")), expected_non_root)
        self.assertNotIn(".", manifest_array(implicit, "members"))
        self.assertEqual(
            set(manifest_array(explicit, "members")),
            expected_non_root | {"."},
        )
        self.assertIn("[package]", standalone)
        self.assertNotIn("[workspace]", standalone)
        self.assertNotIn("[package]", virtual)
        self.assertIn("[workspace]", virtual)
        self.assertEqual(set(manifest_array(virtual, "members")), expected_non_root)
        self.assertEqual(manifest_array(conflict, "members"), ("crates/cli",))
        self.assertEqual(manifest_array(conflict, "exclude"), ("crates/cli",))
        for deferred in (
            "[features]",
            "[dependencies]",
            "[build-dependencies]",
            "[patch.crates-io]",
            "[target.'cfg(windows)'.dependencies]",
            "required-features",
        ):
            self.assertIn(deferred, implicit)

        macro_manifest = (
            FIXTURE_ROOT
            / "shared-tree/crates/macro-sentinel/Cargo.toml"
        ).read_text(encoding="utf-8")
        self.assertIn("proc-macro = true", macro_manifest)
        sentinel_paths = {
            item["path"]
            for item in load_json(FIXTURE_MANIFEST_PATH)["files"]
            if item["role"] == "execution_sentinel"
        }
        self.assertEqual(
            sentinel_paths,
            {
                "shared-tree/build.rs",
                "shared-tree/crates/cli/src/bin/inspect/main.rs",
                "shared-tree/crates/cli/src/main.rs",
                "shared-tree/crates/macro-sentinel/src/lib.rs",
                "shared-tree/src/bin/admin.rs",
                "shared-tree/src/main.rs",
            },
        )
        for relative in sentinel_paths:
            source = (FIXTURE_ROOT / relative).read_text(encoding="utf-8")
            self.assertRegex(source, r"(?:panic!|compile_error!)")

    def test_workspace_plan_ids_ordering_and_boundary_recipe(self) -> None:
        plan = load_json(EXPECTED_PLAN_PATH)
        self.assertEqual(
            set(plan),
            {
                "schema_version",
                "repository_identity",
                "variants",
                "targets",
                "coverage_capabilities",
                "gitlink",
            },
        )
        self.assertEqual(
            plan["schema_version"], "codenoesis.r3-workspace-plan/v1"
        )
        self.assertEqual(plan["repository_identity"], REPOSITORY_IDENTITY)
        implicit = plan["variants"]["implicit"]
        members = implicit["members"]
        self.assertEqual(
            [member["path"] for member in members],
            [".", "crates/cli", "crates/macro-sentinel", "external/model"],
        )
        external = members[-1]
        self.assertEqual(external["manifest_path"], None)
        self.assertEqual(external["crate_ids"], [])
        self.assertEqual(
            external["external_boundary"],
            {
                "profile": "local-gitlinks-v1",
                "identity_domain": "codenoesis.repository-boundary/v1",
                "identity_inputs": [
                    "repository_identity",
                    "materialized_commit_oid",
                    "path",
                    "gitlink_oid",
                ],
                "gitlink_oid": GITLINK_OID,
                "analyzed": False,
            },
        )
        self.assertTrue(
            all(member["external_boundary"] is None for member in members[:-1])
        )
        self.assertEqual(
            plan["variants"]["explicit_dot"],
            {
                "root_shape": "non_virtual_workspace",
                "root_member_source": "explicit_root_member",
                "root_crate_ids_equal_variant": "implicit",
            },
        )
        self.assertEqual(
            plan["variants"]["virtual"]["member_paths"],
            ["crates/cli", "crates/macro-sentinel", "external/model"],
        )
        self.assertEqual(
            tuple(plan["coverage_capabilities"]), COVERAGE_CAPABILITIES
        )
        self.assertEqual(
            plan["gitlink"],
            {
                "path": "external/model",
                "commit_oid": GITLINK_OID,
                "analyzed": False,
            },
        )

        targets = plan["targets"]
        observed = tuple(
            (
                target["manifest_path"],
                target["package_name"],
                target["target_kind"],
                target["target_name"],
                target["source_path"],
                target["crate_id"],
            )
            for target in targets
        )
        self.assertEqual(observed, EXPECTED_TARGETS)
        for target in targets:
            expected_id = stable_id(
                "codenoesis.entity-id/rust/v2",
                [
                    REPOSITORY_IDENTITY,
                    "crate",
                    target["manifest_path"],
                    target["package_name"],
                    target["target_kind"],
                    target["target_name"],
                ],
            )
            self.assertEqual(target["crate_id"], expected_id)
            self.assertTrue(
                (FIXTURE_ROOT / "shared-tree" / target["source_path"]).is_file()
            )
        self.assertEqual(
            len({target["crate_id"] for target in targets}), len(targets)
        )
        by_manifest: dict[str, list[str]] = {}
        for target in targets:
            by_manifest.setdefault(target["manifest_path"], []).append(
                target["crate_id"]
            )
        member_ids = {
            member["manifest_path"]: member["crate_ids"]
            for member in members
            if member["manifest_path"] is not None
        }
        self.assertEqual(
            member_ids,
            {
                manifest: sorted(crate_ids)
                for manifest, crate_ids in by_manifest.items()
            },
        )

    def test_new_schemas_are_strict_and_every_ref_resolves(self) -> None:
        for path in SCHEMA_PATHS:
            with self.subTest(path=path):
                document = load_json(path)
                self.assertEqual(
                    document["$schema"],
                    "https://json-schema.org/draft/2020-12/schema",
                )
                self.assertEqual(document["type"], "object")
                self.assertFalse(document["additionalProperties"])
                for schema in walk_json(document):
                    if schema.get("type") == "object":
                        self.assertIn("additionalProperties", schema)
                        self.assertFalse(schema["additionalProperties"])
                    reference = schema.get("$ref")
                    if reference is None:
                        continue
                    self.assertFalse(reference.startswith(("http:", "https:")))
                    relative, separator, fragment = reference.partition("#")
                    target_path = path if not relative else path.parent / relative
                    target_path = target_path.resolve()
                    try:
                        target_path.relative_to(ROOT)
                    except ValueError as error:
                        self.fail(f"schema ref escapes repository: {reference}: {error}")
                    self.assertTrue(target_path.is_file(), reference)
                    target = load_json(target_path)
                    if separator:
                        resolve_pointer(target, fragment)

    def test_versioned_contract_shapes_are_additive_and_closed(self) -> None:
        configuration = load_json(CONFIGURATION_SCHEMA_PATH)
        self.assertEqual(
            set(configuration["required"]),
            {
                "schema_version",
                "profile",
                "workspace_profile",
                "repository_boundary_profile",
                "semantic_hash",
            },
        )
        self.assertEqual(
            configuration["properties"]["schema_version"]["const"],
            "codenoesis.configuration/v3",
        )
        self.assertEqual(
            configuration["properties"]["workspace_profile"]["const"],
            "cargo-root-package-v1",
        )

        snapshot = load_json(SNAPSHOT_SCHEMA_PATH)
        semantic = snapshot["$defs"]["semantic"]
        self.assertNotIn("repository_boundaries", semantic["required"])
        self.assertEqual(len(semantic["allOf"]), 1)
        conditional = semantic["allOf"][0]
        self.assertEqual(
            conditional["then"]["required"], ["repository_boundaries"]
        )
        self.assertEqual(
            semantic["properties"]["pipeline_version"]["const"],
            "codenoesis.pipeline/s4-r3-v1",
        )

        graph = load_json(GRAPH_SCHEMA_PATH)
        self.assertEqual(
            graph["$defs"]["workspace"]["properties"]["members"]["maxItems"],
            201,
        )
        member = graph["$defs"]["member"]
        self.assertIn("external_boundary_id", member["required"])
        self.assertRegex(
            member["properties"]["external_boundary_id"]["oneOf"][1]["pattern"],
            r"repository-boundary",
        )

        ontology = load_json(ONTOLOGY_PATH)
        self.assertEqual(
            ontology["extends"],
            {
                "ontology_version": "codenoesis.ontology/rust/v2",
                "contract_path": "tests/specifications/s4/rust-ontology-v2.json",
                "contract_sha256": IMMUTABLE_FILES[
                    "tests/specifications/s4/rust-ontology-v2.json"
                ],
            },
        )
        self.assertEqual(
            tuple(ontology["coverage_capabilities"]), COVERAGE_CAPABILITIES
        )
        self.assertEqual(
            ontology["limits"],
            {
                "workspace_members": 200,
                "projected_workspace_members": 201,
                "workspace_exclusions": 200,
                "package_manifests": 200,
                "crate_targets": 200,
                "binary_roots_per_package": 64,
                "single_manifest_bytes": 4_194_304,
            },
        )
        self.assertEqual(
            ontology["identity"]["entity_domain"],
            "codenoesis.entity-id/rust/v2",
        )
        self.assertFalse(
            ontology["identity"]["workspace_member_source_is_identity_input"]
        )
        self.assertFalse(
            ontology["identity"]["workspace_root_shape_is_identity_input"]
        )

    def test_error_v10_has_one_exact_mapping_per_code(self) -> None:
        schema = load_json(ERROR_SCHEMA_PATH)
        self.assertEqual(
            tuple(schema["properties"]["code"]["enum"]), ERROR_CODES
        )
        conditional_codes = tuple(
            condition["if"]["properties"]["code"]["const"]
            for condition in schema["allOf"]
        )
        self.assertEqual(conditional_codes, ERROR_CODES)
        self.assertEqual(len(set(conditional_codes)), len(ERROR_CODES))
        self.assertEqual(
            schema["$defs"]["limit_context"]["properties"]["limit"]["enum"],
            [
                "workspace_members",
                "workspace_exclusions",
                "package_manifests",
                "workspace_crates",
                "binary_roots_per_package",
                "single_manifest_bytes",
            ],
        )
        self.assertNotIn(
            "external_workspace_member_requires_boundary",
            ERROR_SCHEMA_PATH.read_text(encoding="utf-8"),
        )

    def test_semantic_hash_domains_are_new_and_content_complete(self) -> None:
        self.assertEqual(
            load_json(HASH_CONTRACT_PATH),
            {
                "schema_version": "codenoesis.semantic-hash-contract/v2",
                "algorithm": "blake3-256",
                "canonicalization": "RFC8785",
                "domain_separator_hex": "00",
                "hashes": {
                    "configuration": {
                        "domain": "codenoesis.configuration.semantic.v3",
                        "payload": "ConfigurationV3 without semantic_hash",
                    },
                    "extraction_chunk": {
                        "domain": "codenoesis.extraction-chunk.semantic.v3",
                        "payload": "ExtractionChunkV3 without semantic_hash",
                    },
                    "knowledge_graph": {
                        "domain": "codenoesis.knowledge-graph.semantic.v3",
                        "payload": "KnowledgeGraphV3 without semantic_hash",
                    },
                    "snapshot": {
                        "domain": "codenoesis.repository-snapshot.semantic.v6",
                        "payload": "RepositorySnapshotV6.semantic",
                    },
                },
            },
        )

    def test_inherited_s4_r2_and_corpus_bytes_are_immutable(self) -> None:
        for relative, expected in IMMUTABLE_FILES.items():
            with self.subTest(relative=relative):
                self.assertEqual(sha256_path(ROOT / relative), expected)
        s4_bundle = load_json(ROOT / "tests/specifications/s4/contract-bundle.json")
        r2_bundle = load_json(
            ROOT
            / "tests/specifications/s1/gitlink-boundary-contract-bundle.json"
        )
        self.assertEqual(
            s4_bundle["bundle_sha256"],
            "3efb380fb058a5831123a0f990676575da04e60998cada8987f034675b61f12e",
        )
        self.assertEqual(
            r2_bundle["bundle_sha256"],
            "2f59bb311b64b0f4f9d506266f05e9e52f4c0bf5af8926276ed371967690969b",
        )
        oracle = load_json(ORACLE_PATH)
        self.assertEqual(
            oracle["immutable_dependencies"],
            {
                "s4_contract_bundle_sha256": s4_bundle["bundle_sha256"],
                "r2_contract_bundle_sha256": r2_bundle["bundle_sha256"],
            },
        )
        r2_paths = {item["path"] for item in r2_bundle["files"]}
        self.assertIn("tests/corpora/real-world-rust-v1.json", r2_paths)

    def test_contract_bundle_binds_every_r3_governance_artifact(self) -> None:
        bundle = load_json(BUNDLE_PATH)
        self.assertEqual(
            set(bundle), {"schema_version", "files", "bundle_sha256"}
        )
        self.assertEqual(
            bundle["schema_version"], "codenoesis.contract-bundle/v1"
        )
        files = bundle["files"]
        paths = [item["path"] for item in files]
        self.assertEqual(paths, sorted(paths))
        self.assertEqual(set(paths), BUNDLE_FILES)
        self.assertEqual(len(paths), len(set(paths)))
        self.assertNotIn(
            "docs/software/software-requirements-specification.md", paths
        )
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
        bundle_sha256 = hashlib.sha256(canonical_json(payload)).hexdigest()
        self.assertEqual(bundle["bundle_sha256"], bundle_sha256)
        srs = SRS_PATH.read_text(encoding="utf-8")
        match = re.search(
            r"R3 root-package workspace contract bundle:\s+"
            r"`sha256:([0-9a-f]{64})`",
            srs,
        )
        self.assertIsNotNone(match, "SRS must bind the complete R3 bundle")
        self.assertEqual(match.group(1), bundle_sha256)  # type: ignore[union-attr]


if __name__ == "__main__":
    unittest.main()
