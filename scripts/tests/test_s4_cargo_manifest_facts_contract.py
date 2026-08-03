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
    ROOT / "docs/software/decisions/0012-s4-cargo-manifest-facts-contract.md"
)
SPEC_ROOT = ROOT / "tests/specifications/s4/r4"
ORACLE_PATH = SPEC_ROOT / "e2e_fr_ext_009_cargo_manifest_facts.json"
BUNDLE_PATH = SPEC_ROOT / "contract-bundle.json"
SUBSET_PATH = SPEC_ROOT / "cargo-manifest-subset-v1.json"
CONFIGURATION_SCHEMA_PATH = SPEC_ROOT / "configuration-v4.schema.json"
SNAPSHOT_SCHEMA_PATH = SPEC_ROOT / "repository-snapshot-v7.schema.json"
CHUNK_SCHEMA_PATH = SPEC_ROOT / "extraction-chunk-v4.schema.json"
GRAPH_SCHEMA_PATH = SPEC_ROOT / "knowledge-graph-v4.schema.json"
ONTOLOGY_PATH = SPEC_ROOT / "rust-ontology-v4.json"
ERROR_SCHEMA_PATH = SPEC_ROOT / "codenoesis-error-v11.schema.json"
HASH_CONTRACT_PATH = SPEC_ROOT / "semantic-hash-contract-v3.json"
RED_OBSERVATION_PATH = SPEC_ROOT / "red-observation.json"
FIXTURE_ROOT = ROOT / "tests/fixtures/s4/cargo-manifest-facts-v1"
FIXTURE_REPOSITORY = FIXTURE_ROOT / "repository"
FIXTURE_MANIFEST_PATH = FIXTURE_ROOT / "manifest.json"
EXPECTED_FACTS_PATH = FIXTURE_ROOT / "expected-manifest-facts.json"

ISSUE_REFERENCE = "https://github.com/smutti/codenoesis/issues/100"
AUTHORIZATION_REFERENCE = (
    "https://github.com/smutti/codenoesis/issues/100#issuecomment-5163551187"
)
APPROVAL_REFERENCE = "https://github.com/smutti/codenoesis/pull/PULL_REQUEST"
REQUIRED_BASE = "ea51a8151749fc65e75dd7a10e550adc0b67d422"
REPOSITORY_IDENTITY = "urn:codenoesis:fixture:s4-cargo-manifest-facts-v1"

LIMITS = {
    "manifest_fact_entities": 10_000,
    "dependencies_per_manifest": 256,
    "features_per_manifest": 256,
    "feature_members_per_feature": 128,
    "targets_per_package": 128,
    "patches_per_workspace": 256,
    "metadata_fields_per_owner": 32,
    "requested_features_per_declaration": 64,
    "target_predicates_per_manifest": 128,
    "declaration_string_bytes": 2_048,
    "external_locator_bytes": 4_096,
    "permutations": 50,
}

ERROR_CODES = (
    "input.invalid_manifest_profile",
    "extraction.invalid_cargo_manifest_fact",
    "extraction.cargo_manifest_fact_conflict",
    "extraction.cargo_manifest_fact_limit_exceeded",
    "internal.unexpected",
)

CAPABILITY_STATES = {
    "cargo.active_features_not_resolved": "not_resolved",
    "cargo.active_target_not_resolved": "not_resolved",
    "cargo.build_script_not_executed": "not_executed",
    "cargo.dependency_advanced_fields_unsupported": "unsupported",
    "cargo.dependency_graph_not_resolved": "not_resolved",
    "cargo.dependency_source_not_fetched": "not_fetched",
    "cargo.external_locator_redacted": "redacted",
    "cargo.generated_source_not_analyzed": "not_analyzed",
    "cargo.lint_configuration_unsupported": "unsupported",
    "cargo.package_file_selection_not_applied": "not_applied",
    "cargo.package_metadata_table_unsupported": "unsupported",
    "cargo.patch_not_applied": "not_applied",
    "cargo.proc_macro_not_executed": "not_executed",
    "cargo.profile_tables_unsupported": "unsupported",
    "cargo.replace_table_unsupported": "unsupported",
    "cargo.target_source_not_analyzed": "not_analyzed",
    "cargo.workspace_inheritance_not_materialized": "not_resolved",
}

FIXTURE_CAPABILITY_STATES = {
    capability: state
    for capability, state in CAPABILITY_STATES.items()
    if capability
    not in {
        "cargo.dependency_advanced_fields_unsupported",
        "cargo.replace_table_unsupported",
    }
}

CARGO_ENTITY_KINDS = {
    "cargo.manifest",
    "cargo.workspace_package_defaults",
    "cargo.package",
    "cargo.target",
    "cargo.dependency",
    "cargo.feature",
    "cargo.patch",
    "cargo.build_script",
}

REQUIRED_TEST_NAMES = (
    "e2e_fr_ext_009_cargo_manifest_facts",
    "gt_fr_ext_009_manifest_declaration_entities",
    "gt_fr_ext_009_workspace_inheritance_references_declarations",
    "conf_fr_ext_009_snapshot_v7_graph_v4_error_v11",
    "pt_dr_idn_002_r4_preserves_rust_v3_identity_domains",
    "pt_fr_ext_009_limits_have_max_and_plus_one",
    "pt_nfr_det_001_r4_permutation_and_schedule_invariant",
    "sec_fr_ext_009_external_locators_are_digest_only",
    "sec_fr_ext_009_manifest_facts_never_resolve_fetch_or_execute",
    "reg_fr_cli_001_r4_selector_absence_is_byte_identical",
    "e2e_fr_doc_001_r4_declared_vs_resolved_is_documented",
    "e2e_fr_qry_001_r4_exact_id_results",
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
    "docs/software/decisions/0012-s4-cargo-manifest-facts-contract.md",
    "scripts/tests/test_s4_cargo_manifest_facts_contract.py",
    "tests/corpora/real-world-rust-v1.json",
    "tests/fixtures/s4/cargo-manifest-facts-v1/README.md",
    "tests/fixtures/s4/cargo-manifest-facts-v1/expected-manifest-facts.json",
    "tests/fixtures/s4/cargo-manifest-facts-v1/manifest.json",
    "tests/fixtures/s4/cargo-manifest-facts-v1/repository/Cargo.toml",
    "tests/fixtures/s4/cargo-manifest-facts-v1/repository/README.md",
    "tests/fixtures/s4/cargo-manifest-facts-v1/repository/crates/app/Cargo.toml",
    "tests/fixtures/s4/cargo-manifest-facts-v1/repository/crates/app/README.md",
    "tests/fixtures/s4/cargo-manifest-facts-v1/repository/crates/app/build.rs",
    "tests/fixtures/s4/cargo-manifest-facts-v1/repository/crates/app/examples/demo.rs",
    "tests/fixtures/s4/cargo-manifest-facts-v1/repository/crates/app/src/lib.rs",
    "tests/fixtures/s4/cargo-manifest-facts-v1/repository/crates/app/src/main.rs",
    "tests/specifications/s4/r3/contract-bundle.json",
    "tests/specifications/s4/r4/cargo-manifest-subset-v1.json",
    "tests/specifications/s4/r4/codenoesis-error-v11.schema.json",
    "tests/specifications/s4/r4/configuration-v4.schema.json",
    "tests/specifications/s4/r4/e2e_fr_ext_009_cargo_manifest_facts.json",
    "tests/specifications/s4/r4/extraction-chunk-v4.schema.json",
    "tests/specifications/s4/r4/knowledge-graph-v4.schema.json",
    "tests/specifications/s4/r4/red-observation.json",
    "tests/specifications/s4/r4/repository-snapshot-v7.schema.json",
    "tests/specifications/s4/r4/rust-ontology-v4.json",
    "tests/specifications/s4/r4/semantic-hash-contract-v3.json",
}

IMMUTABLE_FILES = {
    "LICENSE": "c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4",
    "tests/corpora/real-world-rust-v1.json": (
        "1d2edc9f858d612e76abb70e6dd255d28e88306a0e4874b0e8ea7351f4347f46"
    ),
    "tests/specifications/s4/r3/contract-bundle.json": (
        "e4707cb401b2e2c8cbbd0fd06e0c3ec0221ef2558bb4b860cada4f9a5dd674a3"
    ),
    "tests/specifications/s4/r3/rust-ontology-v3.json": (
        "ae86b71ac0a5940761b2799ba891f0f4de5cff06a6c87ff42dd0b30b69d1dc43"
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


def iter_strings(value: Any) -> Iterator[str]:
    if isinstance(value, str):
        yield value
    elif isinstance(value, dict):
        for child in value.values():
            yield from iter_strings(child)
    elif isinstance(value, list):
        for child in value:
            yield from iter_strings(child)


def table_body(text: str, header: str) -> str:
    match = re.search(
        rf"(?ms)^\[{re.escape(header)}\]\s*\n(?P<body>.*?)(?=^\[|\Z)",
        text,
    )
    if match is None:
        raise AssertionError(f"missing TOML table [{header}]")
    return match.group("body")


def assignment_keys(body: str) -> set[str]:
    return set(re.findall(r"(?m)^([A-Za-z0-9_-]+)(?:\.workspace)?\s*=", body))


class S4CargoManifestFactsGovernanceTests(unittest.TestCase):
    def test_ratification_register_and_decision_are_exact(self) -> None:
        srs = SRS_PATH.read_text(encoding="utf-8")
        decision = DECISION_PATH.read_text(encoding="utf-8")
        for value in (
            "### 2.14 S4 Cargo manifest facts ratification register",
            ISSUE_REFERENCE,
            AUTHORIZATION_REFERENCE,
            APPROVAL_REFERENCE,
            "FR-EXT-009",
            "--manifest-profile cargo-manifest-facts-v1",
            "codenoesis.repository-snapshot/v7",
            "codenoesis.ontology/rust/v4",
            "codenoesis.error/v11",
        ):
            self.assertIn(value, srs)
        for value in (
            ISSUE_REFERENCE,
            AUTHORIZATION_REFERENCE,
            APPROVAL_REFERENCE,
            REQUIRED_BASE,
            "--manifest-profile cargo-manifest-facts-v1",
            "--workspace-profile cargo-root-package-v1",
            "RepositorySnapshotV7",
            "KnowledgeGraphV4",
            "ErrorV11",
            "DEPENDS_ON",
            "SRS is excluded",
            "requires a separate Ready issue",
        ):
            self.assertIn(value, decision)
        self.assertIn("R3 implementation merge #99", srs)
        self.assertIn("R4 governance adds no production behavior", decision)

    def test_machine_oracle_binds_authorization_red_limits_and_pilots(self) -> None:
        oracle = load_json(ORACLE_PATH)
        self.assertEqual(oracle["issue"], ISSUE_REFERENCE)
        self.assertEqual(oracle["authorization"], AUTHORIZATION_REFERENCE)
        self.assertEqual(oracle["approval"], APPROVAL_REFERENCE)
        self.assertEqual(oracle["requirement_ids"], ["FR-EXT-009"])
        self.assertEqual(
            oracle["requirement_status"],
            {
                "current": "Proposed",
                "target_after_protected_merge": "Approved",
            },
        )
        self.assertEqual(oracle["slice"], "S4")
        self.assertEqual(oracle["roadmap_capability"], "R4")
        self.assertEqual(oracle["risk"], "high")
        self.assertEqual(oracle["required_base"], REQUIRED_BASE)
        self.assertEqual(
            oracle["selector"],
            {
                "flag": "--manifest-profile",
                "value": "cargo-manifest-facts-v1",
                "required_profile": "standard-local-s4",
                "required_workspace_profile": "cargo-root-package-v1",
                "implicit_selection": False,
                "composes_with": [
                    "local-git-sha1-packed-v1",
                    "local-gitlinks-v1",
                ],
            },
        )
        self.assertEqual(oracle["limits"], LIMITS)
        self.assertEqual(tuple(oracle["required_test_names"]), REQUIRED_TEST_NAMES)
        red = oracle["first_red"]
        self.assertEqual(red["subject_exit_code"], 2)
        self.assertEqual(red["stdout_bytes"], 0)
        self.assertEqual(
            red["stdout_sha256"],
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        )
        self.assertEqual(red["stderr_bytes"], 149)
        self.assertEqual(
            red["stderr_sha256"],
            "7f75f7a91f6af0328795f3fbd2729e69756beba2ebd642cc1f6401265662a2fe",
        )
        self.assertFalse(red["store_exists"])
        pilots = {pilot["id"]: pilot for pilot in oracle["public_pilots"]}
        self.assertEqual(set(pilots), {"lekton", "rustdesk"})
        self.assertEqual(
            pilots["lekton"]["commit"],
            "7a4d1a4a30468f4c18ce158a9b825680b00f4820",
        )
        self.assertEqual(
            pilots["rustdesk"]["commit"],
            "d412d198720aa56f6cfed2dfad262e8fb1322fb7",
        )
        self.assertTrue(all(not pilot["vendored_source"] for pilot in pilots.values()))

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

    def test_versioned_shapes_identity_and_coverage_are_closed(self) -> None:
        configuration = load_json(CONFIGURATION_SCHEMA_PATH)
        self.assertEqual(
            set(configuration["required"]),
            {
                "schema_version",
                "profile",
                "workspace_profile",
                "manifest_profile",
                "repository_boundary_profile",
                "semantic_hash",
            },
        )
        self.assertEqual(
            configuration["properties"]["schema_version"]["const"],
            "codenoesis.configuration/v4",
        )
        self.assertEqual(
            configuration["properties"]["manifest_profile"]["const"],
            "cargo-manifest-facts-v1",
        )

        snapshot = load_json(SNAPSHOT_SCHEMA_PATH)["$defs"]["semantic"]
        self.assertEqual(
            snapshot["properties"]["pipeline_version"]["const"],
            "codenoesis.pipeline/s4-r4-v1",
        )
        self.assertEqual(
            snapshot["properties"]["extractor_contract_version"]["const"],
            "codenoesis.extraction/v4",
        )
        self.assertNotIn("repository_boundaries", snapshot["required"])
        self.assertEqual(
            snapshot["allOf"][0]["then"]["required"],
            ["repository_boundaries"],
        )

        chunk = load_json(CHUNK_SCHEMA_PATH)
        cargo_kinds = set(
            chunk["$defs"]["cargo_entity"]["properties"]["kind"]["enum"]
        )
        self.assertEqual(cargo_kinds, CARGO_ENTITY_KINDS)
        self.assertEqual(
            set(
                chunk["$defs"]["cargo_relationship"]["properties"]["kind"][
                    "enum"
                ]
            ),
            {"DECLARES", "REFERENCES_DECLARATION", "MATERIALIZES"},
        )
        coverage = chunk["$defs"]["coverage"]["properties"]
        self.assertEqual(set(coverage["capability"]["enum"]), set(CAPABILITY_STATES))
        self.assertEqual(set(coverage["state"]["enum"]), set(CAPABILITY_STATES.values()))

        graph = load_json(GRAPH_SCHEMA_PATH)
        self.assertEqual(
            graph["$defs"]["manifest_index"]["properties"]["manifests"][
                "maxItems"
            ],
            200,
        )

        ontology = load_json(ONTOLOGY_PATH)
        self.assertEqual(
            ontology["extends"],
            {
                "ontology_version": "codenoesis.ontology/rust/v3",
                "contract_path": "tests/specifications/s4/r3/rust-ontology-v3.json",
                "contract_sha256": IMMUTABLE_FILES[
                    "tests/specifications/s4/r3/rust-ontology-v3.json"
                ],
            },
        )
        self.assertEqual(set(ontology["cargo_entity_kinds"]), CARGO_ENTITY_KINDS)
        self.assertEqual(ontology["coverage_capability_states"], CAPABILITY_STATES)
        self.assertEqual(ontology["limits"], LIMITS)
        self.assertEqual(
            ontology["identity"]["rust_entity_domain"],
            "codenoesis.entity-id/rust/v2",
        )
        self.assertEqual(
            ontology["identity"]["cargo_entity_domain"],
            "codenoesis.entity-id/cargo-manifest/v1",
        )
        self.assertIn("DEPENDS_ON", ontology["forbidden_relationship_kinds"])

    def test_manifest_subset_is_declaration_only_and_self_consistent(self) -> None:
        subset = load_json(SUBSET_PATH)
        self.assertEqual(
            subset["selection"],
            {
                "flag": "--manifest-profile",
                "required_scan_profile": "standard-local-s4",
                "required_workspace_profile": "cargo-root-package-v1",
                "implicit_selection": False,
                "repository_content_selects_profile": False,
            },
        )
        self.assertEqual(
            set(subset["dependencies"]["source_kinds"]),
            {
                "registry_default",
                "registry_named",
                "path",
                "git",
                "workspace_inherited",
            },
        )
        self.assertEqual(
            set(subset["features"]["member_syntax"]),
            {
                "bare",
                "explicit_dependency",
                "dependency_feature",
                "weak_dependency_feature",
            },
        )
        self.assertTrue(subset["external_locators"]["raw_output_forbidden"])
        self.assertEqual(
            subset["external_locators"]["maximum_input_bytes"],
            LIMITS["external_locator_bytes"],
        )
        self.assertIn("DEPENDS_ON", subset["dependencies"]["forbidden_graph_edges"])
        non_claims = " ".join(subset["semantic_non_claims"])
        for phrase in (
            "active feature",
            "resolved dependency graph",
            "applied patch",
            "executed build script",
            "Cargo validation equivalence",
        ):
            self.assertIn(phrase, non_claims)

    def test_fixture_manifest_binds_every_materialized_byte(self) -> None:
        manifest = load_json(FIXTURE_MANIFEST_PATH)
        self.assertEqual(
            manifest["schema_version"], "codenoesis.r4-fixture-manifest/v1"
        )
        self.assertEqual(manifest["repository_identity"], REPOSITORY_IDENTITY)
        self.assertFalse(manifest["external_source_vendored"])
        files = manifest["files"]
        paths = [item["path"] for item in files]
        expected_paths = sorted(
            path.relative_to(FIXTURE_ROOT).as_posix()
            for path in FIXTURE_REPOSITORY.rglob("*")
            if path.is_file()
        )
        self.assertEqual(paths, expected_paths)
        self.assertEqual(len(paths), len(set(paths)))
        for item in files:
            source = FIXTURE_ROOT / item["path"]
            data = source.read_bytes()
            self.assertEqual(item["mode"], "100644")
            self.assertEqual(item["byte_length"], len(data))
            self.assertEqual(item["sha256"], hashlib.sha256(data).hexdigest())
            self.assertEqual(item["git_blob_oid"], git_blob_oid(data))
        expected_plan = manifest["expected_plan"]
        self.assertEqual(expected_plan["path"], EXPECTED_FACTS_PATH.name)
        self.assertEqual(expected_plan["byte_length"], EXPECTED_FACTS_PATH.stat().st_size)
        self.assertEqual(expected_plan["sha256"], sha256_path(EXPECTED_FACTS_PATH))
        for relative in manifest["materialization"]["deliberately_absent_paths"]:
            self.assertFalse((FIXTURE_ROOT / relative).exists())
        readme = (FIXTURE_ROOT / "README.md").read_text(encoding="utf-8")
        self.assertIn("project-owned", readme)
        self.assertIn("never fetches", readme)

    def test_fixture_toml_covers_each_supported_family_and_sentinel(self) -> None:
        root_manifest = (FIXTURE_REPOSITORY / "Cargo.toml").read_text(
            encoding="utf-8"
        )
        app_manifest = (FIXTURE_REPOSITORY / "crates/app/Cargo.toml").read_text(
            encoding="utf-8"
        )
        workspace = table_body(root_manifest, "workspace")
        self.assertRegex(workspace, r'(?m)^members = \["crates/app"\]$')
        self.assertRegex(workspace, r'(?m)^resolver = "3"$')
        self.assertEqual(
            assignment_keys(table_body(root_manifest, "workspace.dependencies")),
            {"serde", "shared-core", "remote-api"},
        )
        self.assertEqual(
            len(assignment_keys(table_body(root_manifest, "workspace.package"))),
            15,
        )
        self.assertIn("[workspace.metadata.codenoesis-fixture]", root_manifest)
        self.assertIn("[workspace.lints.rust]", root_manifest)
        self.assertIn("[profile.release]", root_manifest)
        self.assertIn("[patch.crates-io]", root_manifest)
        self.assertIn(
            '[patch."https://fixture-token@example.invalid/custom-index"]',
            root_manifest,
        )

        package = table_body(app_manifest, "package")
        self.assertRegex(package, r'(?m)^name = "manifest-app"$')
        inherited = set(
            re.findall(r"(?m)^([A-Za-z0-9_-]+)\.workspace = true$", package)
        )
        self.assertEqual(
            inherited,
            {
                "version",
                "edition",
                "rust-version",
                "authors",
                "description",
                "documentation",
                "homepage",
                "repository",
                "license",
                "readme",
                "keywords",
                "categories",
                "publish",
                "include",
                "exclude",
            },
        )
        self.assertRegex(package, r"(?m)^autobins = false$")
        self.assertRegex(package, r"(?m)^autoexamples = false$")
        self.assertRegex(table_body(app_manifest, "lib"), r"(?m)^proc-macro = true$")
        self.assertEqual(app_manifest.count("[[bin]]"), 1)
        self.assertEqual(app_manifest.count("[[example]]"), 1)
        self.assertEqual(
            assignment_keys(table_body(app_manifest, "dependencies")),
            {"serde", "renamed", "local-util", "remote-api"},
        )
        self.assertEqual(
            assignment_keys(table_body(app_manifest, "dev-dependencies")),
            {"pretty_assertions"},
        )
        self.assertEqual(
            assignment_keys(table_body(app_manifest, "build-dependencies")),
            {"cc"},
        )
        self.assertEqual(
            assignment_keys(
                table_body(
                    app_manifest,
                    'target.\'cfg(target_os = "linux")\'.dependencies',
                )
            ),
            {"nix"},
        )
        self.assertRegex(
            table_body(app_manifest, "features"),
            r'(?m)^cli = \["dep:serde", "serde\?/std", "renamed/api", "local-feature"\]$',
        )
        self.assertIn("[package.metadata.codenoesis-fixture]", app_manifest)
        self.assertIn("[lints]", app_manifest)

        sentinel_text = "\n".join(
            (FIXTURE_REPOSITORY / path).read_text(encoding="utf-8")
            for path in (
                "crates/app/build.rs",
                "crates/app/src/main.rs",
                "crates/app/examples/demo.rs",
            )
        )
        self.assertIn("panic!", sentinel_text)
        self.assertIn("compile_error!", sentinel_text)

    def test_expected_ids_evidence_relationships_and_redaction_are_exact(self) -> None:
        plan = load_json(EXPECTED_FACTS_PATH)
        self.assertEqual(
            plan["schema_version"], "codenoesis.r4-manifest-fact-plan/v1"
        )
        self.assertEqual(plan["repository_identity"], REPOSITORY_IDENTITY)
        entity_domain = plan["identity_domains"]["cargo_entity"]
        relationship_domain = plan["identity_domains"]["cargo_relationship"]
        entities = plan["entities"]
        keys = [entity["key"] for entity in entities]
        self.assertEqual(len(keys), len(set(keys)))
        self.assertEqual({entity["kind"] for entity in entities}, CARGO_ENTITY_KINDS)
        ids: dict[str, str] = {}
        for entity in entities:
            expected_id = stable_id(entity_domain, entity["identity_inputs"])
            self.assertEqual(entity["id"], expected_id)
            ids[entity["key"]] = expected_id
            evidence = entity["evidence"]
            source = FIXTURE_REPOSITORY / evidence["path"]
            data = source.read_bytes()
            start = evidence["start_byte"]
            end = evidence["end_byte"]
            self.assertGreater(end, start)
            self.assertLessEqual(end, len(data))
            self.assertEqual(
                evidence["slice_sha256"],
                hashlib.sha256(data[start:end]).hexdigest(),
            )

        rust_crates = plan["rust_crates"]
        for target_key, kind, name in (
            ("target_lib", "lib", "manifest_app"),
            ("target_bin", "bin", "manifest-app"),
        ):
            self.assertEqual(
                rust_crates[target_key],
                stable_id(
                    plan["identity_domains"]["rust_entity"],
                    [
                        REPOSITORY_IDENTITY,
                        "crate",
                        "crates/app/Cargo.toml",
                        "manifest-app",
                        kind,
                        name,
                    ],
                ),
            )

        relationship_ids: list[str] = []
        for source_key, kind, target_key, relationship_id in plan["relationships"]:
            source = ids[source_key]
            if target_key.startswith("rust_crates."):
                target = rust_crates[target_key.split(".", 1)[1]]
            else:
                target = ids[target_key]
            self.assertEqual(
                relationship_id,
                stable_id(relationship_domain, [kind, source, target]),
            )
            relationship_ids.append(relationship_id)
        self.assertEqual(len(relationship_ids), len(set(relationship_ids)))
        self.assertEqual(plan["coverage_capability_states"], FIXTURE_CAPABILITY_STATES)

        manifest_text = "\n".join(
            path.read_text(encoding="utf-8")
            for path in (
            FIXTURE_REPOSITORY / "Cargo.toml",
            FIXTURE_REPOSITORY / "crates/app/Cargo.toml",
            )
        )
        locators = set(re.findall(r'https://[^"\s]+', manifest_text))
        reference_values = set(
            re.findall(r'\b(?:branch|tag|rev)\s*=\s*"([^"]+)"', manifest_text)
        )
        self.assertEqual(
            set(plan["redacted_locator_sha256"]),
            {hashlib.sha256(value.encode("utf-8")).hexdigest() for value in locators},
        )
        derived_bytes = b"\n".join(
            path.read_bytes()
            for path in sorted(SPEC_ROOT.glob("*.json"))
            if path.name != "contract-bundle.json"
        ) + EXPECTED_FACTS_PATH.read_bytes()
        for plaintext in locators | reference_values:
            needle = plaintext.encode("utf-8")
            if plaintext in reference_values:
                needle = b'"' + needle + b'"'
            self.assertNotIn(needle, derived_bytes)
        self.assertIn("DEPENDS_ON", plan["forbidden_relationship_kinds"])
        self.assertFalse((FIXTURE_REPOSITORY / "crates/shared").exists())
        self.assertFalse((FIXTURE_REPOSITORY / "vendor/custom").exists())

    def test_error_v11_and_semantic_hash_contracts_are_exact(self) -> None:
        schema = load_json(ERROR_SCHEMA_PATH)
        self.assertEqual(tuple(schema["properties"]["code"]["enum"]), ERROR_CODES)
        conditional_codes = tuple(
            condition["if"]["properties"]["code"]["const"]
            for condition in schema["allOf"]
        )
        self.assertEqual(conditional_codes, ERROR_CODES)
        self.assertEqual(len(set(conditional_codes)), len(ERROR_CODES))
        self.assertEqual(
            set(schema["$defs"]["limit_context"]["properties"]["limit"]["enum"]),
            set(LIMITS) - {"permutations"},
        )
        self.assertEqual(
            load_json(HASH_CONTRACT_PATH),
            {
                "schema_version": "codenoesis.semantic-hash-contract/v3",
                "algorithm": "blake3-256",
                "canonicalization": "RFC8785",
                "domain_separator_hex": "00",
                "hashes": {
                    "configuration": {
                        "domain": "codenoesis.configuration.semantic.v4",
                        "payload": "ConfigurationV4 without semantic_hash",
                    },
                    "extraction_chunk": {
                        "domain": "codenoesis.extraction-chunk.semantic.v4",
                        "payload": "ExtractionChunkV4 without semantic_hash",
                    },
                    "knowledge_graph": {
                        "domain": "codenoesis.knowledge-graph.semantic.v4",
                        "payload": "KnowledgeGraphV4 without semantic_hash",
                    },
                    "snapshot": {
                        "domain": "codenoesis.repository-snapshot.semantic.v7",
                        "payload": "RepositorySnapshotV7.semantic",
                    },
                },
            },
        )

    def test_red_observation_is_immutable_and_matches_the_oracle(self) -> None:
        observation = load_json(RED_OBSERVATION_PATH)
        self.assertEqual(observation["required_base"], REQUIRED_BASE)
        governance = observation["governance_guard_red"]
        self.assertEqual(governance["exit_code"], 1)
        self.assertTrue(governance["failed_for_expected_reason"])
        self.assertEqual(
            governance["guard_sha256"],
            "bd526f463aa3239b440c2746d6b1812f50803a86ecfbeefe856c1b2a33bd9a7e",
        )
        measured = observation["first_product_red_measurement"]
        oracle_red = load_json(ORACLE_PATH)["first_red"]
        for field in (
            "subject_exit_code",
            "stdout_bytes",
            "stdout_sha256",
            "stderr",
            "stderr_bytes",
            "stderr_sha256",
            "store_exists",
        ):
            self.assertEqual(measured[field], oracle_red[field])
        self.assertEqual(measured["forbidden_side_effects_observed"], 0)

    def test_inherited_r3_and_corpus_bytes_are_immutable(self) -> None:
        for relative, expected in IMMUTABLE_FILES.items():
            with self.subTest(relative=relative):
                self.assertEqual(sha256_path(ROOT / relative), expected)
        r3_bundle = load_json(ROOT / "tests/specifications/s4/r3/contract-bundle.json")
        self.assertEqual(
            r3_bundle["bundle_sha256"],
            "0b99760da4e978fefa91468b5dbef1b59816e30b02d92c70c26a7df715ef509a",
        )
        self.assertEqual(
            load_json(ORACLE_PATH)["immutable_dependencies"],
            {"r3_contract_bundle_sha256": r3_bundle["bundle_sha256"]},
        )

    def test_contract_bundle_binds_every_r4_governance_artifact(self) -> None:
        bundle = load_json(BUNDLE_PATH)
        self.assertEqual(set(bundle), {"schema_version", "files", "bundle_sha256"})
        self.assertEqual(bundle["schema_version"], "codenoesis.contract-bundle/v1")
        files = bundle["files"]
        paths = [item["path"] for item in files]
        self.assertEqual(paths, sorted(paths))
        self.assertEqual(set(paths), BUNDLE_FILES)
        self.assertEqual(len(paths), len(set(paths)))
        self.assertNotIn("docs/software/software-requirements-specification.md", paths)
        self.assertNotIn("tests/specifications/s4/r4/contract-bundle.json", paths)
        for item in files:
            self.assertEqual(set(item), {"path", "sha256"})
            path = Path(item["path"])
            self.assertFalse(path.is_absolute())
            self.assertNotIn("..", path.parts)
            self.assertRegex(item["sha256"], r"^[0-9a-f]{64}$")
            self.assertEqual(sha256_path(ROOT / path), item["sha256"])
        payload = {"schema_version": bundle["schema_version"], "files": files}
        bundle_sha256 = hashlib.sha256(canonical_json(payload)).hexdigest()
        self.assertEqual(bundle["bundle_sha256"], bundle_sha256)
        srs = SRS_PATH.read_text(encoding="utf-8")
        match = re.search(
            r"R4 Cargo manifest facts contract bundle:\s+"
            r"`sha256:([0-9a-f]{64})`",
            srs,
        )
        self.assertIsNotNone(match, "SRS must bind the complete R4 bundle")
        self.assertEqual(match.group(1), bundle_sha256)  # type: ignore[union-attr]


if __name__ == "__main__":
    unittest.main()
