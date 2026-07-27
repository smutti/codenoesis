from __future__ import annotations

import hashlib
import copy
import re
import unittest
from pathlib import Path
from typing import Any

from test_s1_contract import (
    INHERITED_S0_TESTS,
    S1_TEST_ORDER,
    blake3_256,
    canonical_json,
    git_oid,
    load_json,
)
from test_s2_contract import S2_TEST_ORDER
from test_s3_contract import S3_TEST_ORDER


ROOT = Path(__file__).resolve().parents[2]
FIXTURE_ROOT = ROOT / "tests" / "fixtures" / "s4" / "workspace-docs-v1"
SPEC_ROOT = ROOT / "tests" / "specifications" / "s4"
SPEC_PATH = SPEC_ROOT / "e2e_fr_cli_001_workspace_docs_query.json"
BUNDLE_PATH = SPEC_ROOT / "contract-bundle.json"
ONTOLOGY_PATH = SPEC_ROOT / "rust-ontology-v2.json"
DOCS_CONTRACT_PATH = SPEC_ROOT / "docs-output-contract-v1.json"
DOCS_SCHEMA_PATH = SPEC_ROOT / "documentation-manifest-v1.schema.json"
QUERY_SCHEMA_PATH = SPEC_ROOT / "local-query-result-v1.schema.json"
SNAPSHOT_SCHEMA_PATH = SPEC_ROOT / "repository-snapshot-v4.schema.json"
CHUNK_SCHEMA_PATH = SPEC_ROOT / "extraction-chunk-v2.schema.json"
GRAPH_SCHEMA_PATH = SPEC_ROOT / "knowledge-graph-v2.schema.json"
ERROR_SCHEMA_PATH = SPEC_ROOT / "codenoesis-error-v5.schema.json"
HASH_CONTRACT_PATH = SPEC_ROOT / "semantic-hash-contract-v1.json"
SNAPSHOT_SEMANTIC_PATH = FIXTURE_ROOT / "expected-snapshot-semantic.json"
SRS_PATH = ROOT / "docs" / "software" / "software-requirements-specification.md"

S4_REQUIREMENTS = {
    "DR-IDN-002",
    "FR-CLI-001",
    "FR-DOC-001",
    "FR-DOC-002",
    "FR-DOC-003",
    "FR-EXT-007",
    "FR-QRY-001",
}

S4_TEST_ORDER = (
    "e2e_fr_cli_001_workspace_docs_query",
    "gt_fr_ext_007_workspace_graph_v2",
    "gt_fr_doc_001_deterministic_markdown",
    "pt_fr_doc_002_statement_evidence_integrity",
    "sec_fr_doc_003_generated_root_confinement",
    "e2e_fr_qry_001_exact_id_results",
    "conf_fr_cli_001_v4_manifest_query_error_v5",
    "pt_dr_idn_002_workspace_identity_stability",
    "sec_fr_ext_007_no_target_execution",
    "reg_fr_cli_001_legacy_profiles_unchanged",
)

S4_BUNDLE_FILES = {
    "LICENSE",
    "docs/software/decisions/0005-s4-workspace-docs-query-contract.md",
    "scripts/tests/test_s4_contract.py",
    "tests/fixtures/s4/workspace-docs-v1/README.md",
    "tests/fixtures/s4/workspace-docs-v1/expected-docs/modules/app.md",
    "tests/fixtures/s4/workspace-docs-v1/expected-docs/modules/model-item.md",
    "tests/fixtures/s4/workspace-docs-v1/expected-docs/modules/model.md",
    "tests/fixtures/s4/workspace-docs-v1/expected-docs/overview.md",
    "tests/fixtures/s4/workspace-docs-v1/expected-documentation-manifest.json",
    "tests/fixtures/s4/workspace-docs-v1/expected-error-unknown-id.json",
    "tests/fixtures/s4/workspace-docs-v1/expected-error-unmarked-output.json",
    "tests/fixtures/s4/workspace-docs-v1/expected-error-unsupported-workspace.json",
    "tests/fixtures/s4/workspace-docs-v1/expected-graph-summary.json",
    "tests/fixtures/s4/workspace-docs-v1/expected-query-document.json",
    "tests/fixtures/s4/workspace-docs-v1/expected-query-entity.json",
    "tests/fixtures/s4/workspace-docs-v1/expected-snapshot-semantic.json",
    "tests/fixtures/s4/workspace-docs-v1/manifest.json",
    "tests/fixtures/s4/workspace-docs-v1/revision-a/Cargo.toml",
    "tests/fixtures/s4/workspace-docs-v1/revision-a/crates/app/Cargo.toml",
    "tests/fixtures/s4/workspace-docs-v1/revision-a/crates/app/build.rs",
    "tests/fixtures/s4/workspace-docs-v1/revision-a/crates/app/src/main.rs",
    "tests/fixtures/s4/workspace-docs-v1/revision-a/crates/model/Cargo.toml",
    "tests/fixtures/s4/workspace-docs-v1/revision-a/crates/model/src/item.rs",
    "tests/fixtures/s4/workspace-docs-v1/revision-a/crates/model/src/lib.rs",
    "tests/specifications/s3/contract-bundle.json",
    "tests/specifications/s4/codenoesis-error-v5.schema.json",
    "tests/specifications/s4/docs-output-contract-v1.json",
    "tests/specifications/s4/documentation-manifest-v1.schema.json",
    "tests/specifications/s4/e2e_fr_cli_001_workspace_docs_query.json",
    "tests/specifications/s4/extraction-chunk-v2.schema.json",
    "tests/specifications/s4/knowledge-graph-v2.schema.json",
    "tests/specifications/s4/local-query-result-v1.schema.json",
    "tests/specifications/s4/repository-snapshot-v4.schema.json",
    "tests/specifications/s4/rust-ontology-v2.json",
    "tests/specifications/s4/semantic-hash-contract-v1.json",
}

SCHEMA_PATHS = (
    SNAPSHOT_SCHEMA_PATH,
    CHUNK_SCHEMA_PATH,
    GRAPH_SCHEMA_PATH,
    DOCS_SCHEMA_PATH,
    QUERY_SCHEMA_PATH,
    ERROR_SCHEMA_PATH,
)


def stable_id(domain: str, preimage: list[str]) -> str:
    digest = blake3_256(canonical_json([domain, *preimage]))
    kind = domain.split(".", 1)[1].split("-id", 1)[0]
    return f"urn:codenoesis:{kind}:blake3:{digest}"


def relationship_id(kind: str, source: str, target: str) -> str:
    digest = blake3_256(
        canonical_json(
            ["codenoesis.relationship-id/rust/v2", kind, source, target]
        )
    )
    return f"urn:codenoesis:relationship:blake3:{digest}"


def semantic_hash(domain: str, payload: Any) -> str:
    return blake3_256(domain.encode() + b"\0" + canonical_json(payload))


def git_tree_oid(entries: list[dict[str, str]]) -> str:
    payload = b"".join(
        entry["mode"].encode()
        + b" "
        + entry["name"].encode()
        + b"\0"
        + bytes.fromhex(entry["oid"])
        for entry in entries
    )
    return git_oid("tree", payload)


class S4ContractTests(unittest.TestCase):
    def test_v4_semantic_hash_contract_is_content_complete(self) -> None:
        self.assertTrue(
            HASH_CONTRACT_PATH.is_file(),
            "S4 semantic-hash contract must be ratified before implementation",
        )
        self.assertTrue(
            SNAPSHOT_SEMANTIC_PATH.is_file(),
            "complete reviewed S4 snapshot semantic payload must exist",
        )

        contract = load_json(HASH_CONTRACT_PATH)
        self.assertEqual(
            contract,
            {
                "schema_version": "codenoesis.semantic-hash-contract/v1",
                "algorithm": "blake3-256",
                "canonicalization": "RFC8785",
                "domain_separator_hex": "00",
                "hashes": {
                    "snapshot": {
                        "domain": "codenoesis.repository-snapshot.semantic.v4",
                        "payload": "RepositorySnapshotV4.semantic",
                    },
                    "knowledge_graph": {
                        "domain": "codenoesis.knowledge-graph.semantic.v2",
                        "payload": (
                            "KnowledgeGraphV2 without semantic_hash"
                        ),
                    },
                    "extraction_chunk": {
                        "domain": "codenoesis.extraction-chunk.semantic.v2",
                        "payload": (
                            "ExtractionChunkV2 without semantic_hash"
                        ),
                    },
                },
            },
        )

        semantic = load_json(SNAPSHOT_SEMANTIC_PATH)
        graph = semantic["knowledge_graph"]
        graph_payload = {
            key: value
            for key, value in graph.items()
            if key != "semantic_hash"
        }
        graph_hash = semantic_hash(
            contract["hashes"]["knowledge_graph"]["domain"],
            graph_payload,
        )
        self.assertEqual(graph["semantic_hash"]["value"], graph_hash)

        chunk_domain = contract["hashes"]["extraction_chunk"]["domain"]
        for chunk in semantic["extraction_chunks"]:
            chunk_payload = {
                key: value
                for key, value in chunk.items()
                if key != "semantic_hash"
            }
            self.assertEqual(
                chunk["semantic_hash"]["value"],
                semantic_hash(chunk_domain, chunk_payload),
            )

        snapshot_hash = semantic_hash(
            contract["hashes"]["snapshot"]["domain"],
            semantic,
        )
        summary = load_json(FIXTURE_ROOT / "expected-graph-summary.json")
        self.assertEqual(summary["semantic_hash"], snapshot_hash)
        self.assertEqual(summary["graph_semantic_hash"], graph_hash)
        self.assertEqual(
            summary["extraction_chunk_semantic_hashes"],
            [
                chunk["semantic_hash"]["value"]
                for chunk in semantic["extraction_chunks"]
            ],
        )
        snapshot_id = stable_id(
            "codenoesis.snapshot-id/v1",
            [snapshot_hash],
        )
        self.assertEqual(summary["snapshot_id"], snapshot_id)

        manifest = load_json(
            FIXTURE_ROOT / "expected-documentation-manifest.json"
        )
        self.assertEqual(
            manifest["snapshot_semantic_hash"]["value"],
            snapshot_hash,
        )
        self.assertEqual(manifest["snapshot_id"], snapshot_id)
        for fixture in (
            "expected-query-entity.json",
            "expected-query-document.json",
        ):
            self.assertEqual(
                load_json(FIXTURE_ROOT / fixture)["snapshot_id"],
                snapshot_id,
            )

        changed_graph = copy.deepcopy(graph_payload)
        changed_graph["diagnostics"][0]["message"] += " changed"
        self.assertNotEqual(
            semantic_hash(
                contract["hashes"]["knowledge_graph"]["domain"],
                changed_graph,
            ),
            graph_hash,
        )
        changed_semantic = copy.deepcopy(semantic)
        changed_semantic["knowledge_graph"]["diagnostics"][0]["message"] += (
            " changed"
        )
        self.assertNotEqual(
            semantic_hash(
                contract["hashes"]["snapshot"]["domain"],
                changed_semantic,
            ),
            snapshot_hash,
        )

    def test_contract_bundle_binds_every_s4_ratification_artifact(self) -> None:
        manifest = load_json(BUNDLE_PATH)
        self.assertEqual(
            set(manifest), {"schema_version", "files", "bundle_sha256"}
        )
        self.assertEqual(
            manifest["schema_version"], "codenoesis.contract-bundle/v1"
        )
        files = manifest["files"]
        paths = [entry["path"] for entry in files]
        self.assertEqual(paths, sorted(paths))
        self.assertEqual(set(paths), S4_BUNDLE_FILES)
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
        match = re.search(r"S4 contract bundle: `sha256:([0-9a-f]{64})`", srs)
        self.assertIsNotNone(match, "SRS must bind the complete S4 bundle")
        self.assertEqual(match.group(1), bundle_sha256)  # type: ignore[union-attr]

    def test_s4_register_oracle_and_ratification_are_exact(self) -> None:
        spec = load_json(SPEC_PATH)
        self.assertEqual(spec["status"], "approved")
        self.assertEqual(set(spec["requirements"]), S4_REQUIREMENTS)
        self.assertEqual(len(spec["requirements"]), len(S4_REQUIREMENTS))
        self.assertEqual(
            spec["ratification"],
            {
                "governance_model": "single_maintainer_bootstrap",
                "product_owner_persona": "Andrea Moretti",
                "persona_is_natural_person": False,
                "accountable_github_actor": "smutti",
                "technical_approver": "smutti",
                "approval_reference": "https://github.com/smutti/codenoesis/pull/42",
                "effective_on": "protected_squash_merge_by_accountable_actor",
                "required_external_approvals": 0,
                "agent_merge_allowed": False,
            },
        )
        srs = SRS_PATH.read_text(encoding="utf-8")
        register = srs.split("### 2.7 S4 ratification register", 1)[1].split(
            "## 3. Product intent and success definition", 1
        )[0]
        registered = re.findall(
            r"^\| `([A-Z]+-[A-Z]+-\d{3})` \| "
            r"`(?:Proposed|Approved)` \| `Approved` \|",
            register,
            flags=re.MULTILINE,
        )
        self.assertEqual(set(registered), S4_REQUIREMENTS)
        self.assertEqual(len(registered), len(S4_REQUIREMENTS))
        decision = (ROOT / spec["decision"]).read_text(encoding="utf-8")
        self.assertIn("| Status | Accepted;", decision)
        self.assertIn("authoring agent must not approve or merge", decision)
        self.assertIn("separate policy-binding change", decision)

    def test_acceptance_spec_has_ordered_traceability_and_exact_red(self) -> None:
        spec = load_json(SPEC_PATH)
        self.assertEqual(
            [scenario["test_name"] for scenario in spec["scenarios"]],
            list(S4_TEST_ORDER),
        )
        traced = {
            requirement
            for scenario in spec["scenarios"]
            for requirement in scenario["requirements"]
        }
        self.assertEqual(traced, S4_REQUIREMENTS)
        self.assertEqual(
            set(spec["inherited_regressions"]),
            INHERITED_S0_TESTS
            | set(S1_TEST_ORDER)
            | set(S2_TEST_ORDER)
            | set(S3_TEST_ORDER),
        )
        self.assertEqual(
            spec["expected_red"],
            {
                "test_command": (
                    "cargo test --test e2e_fr_cli_001_workspace_docs_query"
                ),
                "required_base": (
                    "merged S3 implementation from pull request 35"
                ),
                "runner_expected_exit": (
                    "nonzero because the acceptance assertion fails"
                ),
                "subject_observed_exit_code": 2,
                "subject_observed_stderr_schema": "codenoesis.error/v4",
                "subject_observed_stderr_code": "input.invalid_profile",
                "subject_expected_exit_code": 0,
                "expected_snapshot": "codenoesis.repository-snapshot/v4",
                "expected_docs": "codenoesis.documentation-manifest/v1",
                "expected_query": "codenoesis.local-query-result/v1",
                "accepted_reason": (
                    "Merged S3 rejects standard-local-s4 before creating a "
                    "store or generated-document root."
                ),
                "rejected_reasons": [
                    "compilation failure",
                    "missing test target",
                    "missing or corrupt fixture",
                    "schema or guard failure",
                    "dependency or network outage",
                    "timeout",
                    "unexpected panic",
                    "partial store or docs output",
                    "a modified S4 oracle",
                ],
            },
        )
        self.assertEqual(
            spec["commands"]["scan"],
            [
                "noesis",
                "scan",
                "--repository",
                "{repository_path}",
                "--repository-id",
                "urn:codenoesis:fixture:s4-workspace-docs-v1",
                "--revision",
                "{revision}",
                "--profile",
                "standard-local-s4",
                "--store",
                "{store_path}",
                "--format",
                "json",
            ],
        )
        self.assertEqual(spec["commands"]["docs"][0:2], ["noesis", "docs"])
        self.assertEqual(spec["commands"]["query"][0:2], ["noesis", "query"])

    def test_fixture_manifest_binds_files_and_git_objects(self) -> None:
        manifest = load_json(FIXTURE_ROOT / "manifest.json")
        self.assertEqual(
            manifest["schema_version"], "codenoesis.s4-fixture-manifest/v1"
        )
        self.assertEqual(
            manifest["repository_identity"],
            "urn:codenoesis:fixture:s4-workspace-docs-v1",
        )
        file_entries = manifest["revision"]["files"]
        paths = [entry["path"] for entry in file_entries]
        self.assertEqual(paths, sorted(paths))
        for entry in file_entries:
            content = (
                FIXTURE_ROOT / "revision-a" / entry["path"]
            ).read_bytes()
            self.assertEqual(
                hashlib.sha256(content).hexdigest(), entry["sha256"]
            )
            self.assertEqual(git_oid("blob", content), entry["git_blob_oid"])
        for tree in manifest["revision"]["trees"]:
            self.assertEqual(git_tree_oid(tree["entries"]), tree["oid"])
        commit = manifest["revision"]["commit"]
        payload = commit["payload_utf8"].encode()
        self.assertEqual(git_oid("commit", payload), commit["oid"])

    def test_ontology_v2_fixes_workspace_identity_and_cardinality(self) -> None:
        ontology = load_json(ONTOLOGY_PATH)
        self.assertEqual(
            ontology["schema_version"], "codenoesis.rust-ontology/v2"
        )
        self.assertEqual(
            ontology["ontology_version"], "codenoesis.ontology/rust/v2"
        )
        self.assertEqual(ontology["normalization"], "NFC")
        self.assertEqual(
            ontology["workspace"]["member_source"],
            "literal_root_workspace_members",
        )
        self.assertEqual(ontology["workspace"]["minimum_crates"], 1)
        self.assertEqual(ontology["workspace"]["maximum_crates"], 200)
        self.assertEqual(
            ontology["module_resolution"]["supported"],
            ["inline", "name.rs", "name/mod.rs"],
        )
        self.assertEqual(
            set(ontology["module_resolution"]["rejected"]),
            {
                "ambiguous_files",
                "cfg_selected_module",
                "include_macro",
                "path_attribute",
            },
        )
        summary = load_json(FIXTURE_ROOT / "expected-graph-summary.json")
        for entity in summary["identity_examples"]["entities"]:
            self.assertEqual(
                stable_id("codenoesis.entity-id/rust/v2", entity["preimage"]),
                entity["id"],
            )
        for relationship in summary["identity_examples"]["relationships"]:
            self.assertEqual(
                relationship_id(
                    relationship["kind"],
                    relationship["source"],
                    relationship["target"],
                ),
                relationship["id"],
            )
        self.assertGreaterEqual(summary["counts"]["crate"], 2)
        self.assertGreaterEqual(summary["counts"]["module"], 3)
        self.assertEqual(summary["unresolved_cross_crate_uses"], 1)

    def test_document_goldens_are_exact_and_every_statement_is_grounded(
        self,
    ) -> None:
        manifest = load_json(
            FIXTURE_ROOT / "expected-documentation-manifest.json"
        )
        self.assertEqual(
            manifest["schema_version"],
            "codenoesis.documentation-manifest/v1",
        )
        documents = manifest["documents"]
        paths = [document["path"] for document in documents]
        self.assertEqual(paths, sorted(paths))
        self.assertEqual(
            set(paths),
            {
                "modules/app.md",
                "modules/model-item.md",
                "modules/model.md",
                "overview.md",
            },
        )
        all_statement_ids: set[str] = set()
        for document in documents:
            content = (
                FIXTURE_ROOT / "expected-docs" / document["path"]
            ).read_bytes()
            self.assertEqual(blake3_256(content), document["blake3"])
            text = content.decode()
            markdown_ids = set(
                re.findall(
                    r"<!-- statement:(urn:codenoesis:statement:blake3:"
                    r"[0-9a-f]{64}) -->",
                    text,
                )
            )
            manifest_ids = {
                statement["statement_id"]
                for statement in document["statements"]
            }
            self.assertEqual(markdown_ids, manifest_ids)
            self.assertFalse(all_statement_ids & manifest_ids)
            all_statement_ids |= manifest_ids
            for statement in document["statements"]:
                evidence = statement["evidence_ids"]
                coverage = statement["coverage_gap_ids"]
                self.assertTrue(evidence or coverage)
                self.assertFalse(evidence and coverage)
                if coverage:
                    self.assertEqual(statement["truth_state"], "unsupported")
                else:
                    self.assertIn(
                        statement["truth_state"],
                        {"deterministic_fact", "derived_fact"},
                    )

    def test_query_and_error_goldens_are_typed_and_linked(self) -> None:
        entity = load_json(FIXTURE_ROOT / "expected-query-entity.json")
        document = load_json(FIXTURE_ROOT / "expected-query-document.json")
        manifest = load_json(
            FIXTURE_ROOT / "expected-documentation-manifest.json"
        )
        documents_by_id = {
            item["document_id"]: item for item in manifest["documents"]
        }
        self.assertEqual(
            entity["schema_version"], "codenoesis.local-query-result/v1"
        )
        self.assertEqual(entity["result_kind"], "entity")
        self.assertEqual(entity["requested_id"], entity["entity"]["id"])
        entity_schema = load_json(CHUNK_SCHEMA_PATH)["$defs"]["entity"]
        self.assertEqual(
            set(entity["entity"]),
            set(entity_schema["required"]),
        )
        self.assertEqual(len(entity["claims"]), 1)
        self.assertTrue(entity["evidence"])
        self.assertEqual(
            entity["claims"][0]["subject_id"],
            entity["requested_id"],
        )
        self.assertEqual(
            [item["id"] for item in entity["evidence"]],
            entity["claims"][0]["evidence_ids"],
        )
        expected_links = [
            {"document_id": source["document_id"], **statement}
            for source in manifest["documents"]
            for statement in source["statements"]
            if entity["requested_id"] in statement["subject_ids"]
        ]
        self.assertEqual(entity["document_statements"], expected_links)
        self.assertEqual(
            document["schema_version"], "codenoesis.local-query-result/v1"
        )
        self.assertEqual(document["result_kind"], "document")
        self.assertEqual(document["requested_id"], document["document"]["document_id"])
        source_document = documents_by_id[document["requested_id"]]
        self.assertEqual(
            document["document"],
            {
                key: value
                for key, value in source_document.items()
                if key != "statements"
            },
        )
        self.assertEqual(
            document["document_statements"],
            source_document["statements"],
        )
        for name, code in [
            ("expected-error-unknown-id.json", "query.not_found"),
            (
                "expected-error-unmarked-output.json",
                "docs.unmarked_nonempty_root",
            ),
            (
                "expected-error-unsupported-workspace.json",
                "extraction.unsupported_workspace",
            ),
        ]:
            error = load_json(FIXTURE_ROOT / name)
            self.assertEqual(error["schema_version"], "codenoesis.error/v5")
            self.assertEqual(error["code"], code)
            self.assertFalse(error["retryable"])

    def test_public_schemas_and_docs_contract_are_strict(self) -> None:
        expected_ids = {
            SNAPSHOT_SCHEMA_PATH: "codenoesis.repository-snapshot/v4",
            CHUNK_SCHEMA_PATH: "codenoesis.extraction-chunk/v2",
            GRAPH_SCHEMA_PATH: "codenoesis.knowledge-graph/v2",
            DOCS_SCHEMA_PATH: "codenoesis.documentation-manifest/v1",
            QUERY_SCHEMA_PATH: "codenoesis.local-query-result/v1",
            ERROR_SCHEMA_PATH: "codenoesis.error/v5",
        }
        for path in SCHEMA_PATHS:
            schema = load_json(path)
            self.assertEqual(
                schema["$schema"], "https://json-schema.org/draft/2020-12/schema"
            )
            self.assertEqual(schema["type"], "object")
            self.assertFalse(schema["additionalProperties"])
            self.assertIn("schema_version", schema["required"])
            self.assertEqual(
                schema["properties"]["schema_version"]["const"],
                expected_ids[path],
            )
        query_schema = load_json(QUERY_SCHEMA_PATH)
        self.assertNotIn("record", query_schema["$defs"])
        self.assertTrue(
            all(
                definition["additionalProperties"] is False
                for definition in query_schema["$defs"].values()
            )
        )
        self.assertEqual(
            query_schema["properties"]["entity"]["oneOf"][1]["$ref"],
            "extraction-chunk-v2.schema.json#/$defs/entity",
        )
        self.assertEqual(
            query_schema["properties"]["claims"]["items"]["$ref"],
            "extraction-chunk-v2.schema.json#/$defs/claim",
        )
        self.assertEqual(
            query_schema["properties"]["evidence"]["items"]["$ref"],
            "extraction-chunk-v2.schema.json#/$defs/evidence",
        )
        self.assertEqual(
            {
                condition["if"]["properties"]["result_kind"]["const"]
                for condition in query_schema["allOf"]
            },
            {"entity", "claim", "evidence", "document"},
        )
        contract = load_json(DOCS_CONTRACT_PATH)
        self.assertEqual(
            contract["schema_version"], "codenoesis.docs-output-contract/v1"
        )
        self.assertEqual(
            contract["owned_paths"],
            ["manifest.json", "overview.md", "modules/*.md"],
        )
        self.assertEqual(contract["publication"]["commit_point"], "manifest")
        self.assertEqual(
            contract["publication"]["pre_commit_visible_generation"],
            "previous_or_none",
        )
        self.assertEqual(
            contract["publication"]["post_commit_visible_generation"],
            "complete_new",
        )
        self.assertEqual(
            contract["limits"],
            {
                "documents": 2001,
                "bytes_per_document": 1048576,
                "total_bytes": 33554432,
                "statements": 200000,
                "query_result_bytes": 4194304,
            },
        )
        self.assertFalse(contract["allow_unowned_file_deletion"])
        self.assertFalse(contract["allow_symlink_or_reparse_traversal"])


if __name__ == "__main__":
    unittest.main()
