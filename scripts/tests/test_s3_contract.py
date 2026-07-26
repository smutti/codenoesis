from __future__ import annotations

import hashlib
import re
import sqlite3
import tempfile
import unittest
from pathlib import Path
from typing import Any

from test_s1_contract import blake3_256, canonical_json, git_oid, load_json
from test_s2_contract import S2_TEST_ORDER


ROOT = Path(__file__).resolve().parents[2]
FIXTURE_ROOT = (
    ROOT / "tests" / "fixtures" / "s3" / "atomic-local-storage-v1"
)
SPEC_ROOT = ROOT / "tests" / "specifications" / "s3"
SPEC_PATH = SPEC_ROOT / "e2e_fr_sto_001_atomic_local_storage.json"
STORE_CONTRACT_PATH = SPEC_ROOT / "local-store-contract-v1.json"
DDL_PATH = SPEC_ROOT / "local-store-v1.sql"
FAILPOINT_PATH = SPEC_ROOT / "publication-failpoints-v1.json"
HEAD_SCHEMA_PATH = SPEC_ROOT / "local-snapshot-head-v1.schema.json"
ERROR_SCHEMA_PATH = SPEC_ROOT / "codenoesis-error-v4.schema.json"
BUNDLE_PATH = SPEC_ROOT / "contract-bundle.json"
SRS_PATH = ROOT / "docs" / "software" / "software-requirements-specification.md"
S2_FIXTURE_ROOT = ROOT / "tests" / "fixtures" / "s2" / "rust-knowledge-v1"

S3_REQUIREMENTS = {
    "FR-SNP-001",
    "FR-STO-001",
    "INV-SNP-001",
    "NFR-REL-001",
}

S3_TEST_ORDER = (
    "e2e_fr_sto_001_atomic_local_storage",
    "ct_fr_sto_001_metadata_store_parity",
    "ct_fr_sto_001_cas_parity",
    "ft_fr_snp_001_publication_failpoint_matrix",
    "pt_inv_snp_001_reader_visibility",
    "pt_fr_snp_001_idempotent_retry",
    "it_fr_sto_001_restart_preserves_head",
    "it_fr_sto_001_corruption_fails_closed",
    "ft_fr_snp_001_orphan_sweep_preserves_reachable",
    "sec_fr_sto_001_store_root_confinement",
    "conf_fr_sto_001_store_v1_and_error_v4",
    "reg_fr_sto_001_legacy_profiles_unchanged",
)

FAILPOINTS = (
    "cas_before_temp_create",
    "cas_after_temp_sync",
    "cas_after_object_move",
    "cas_after_parent_sync",
    "sqlite_after_begin",
    "sqlite_after_snapshot_rows",
    "sqlite_after_head_update",
    "sqlite_after_commit",
)

SQLITE_TABLES = {
    "artifacts",
    "claims",
    "coverage_gaps",
    "diagnostics",
    "entities",
    "evidence",
    "extraction_chunks",
    "project_heads",
    "relationships",
    "snapshot_artifacts",
    "snapshots",
    "store_metadata",
}

IMMUTABLE_TABLES = SQLITE_TABLES - {"project_heads"}

S3_BUNDLE_FILES = {
    "LICENSE",
    "docs/software/decisions/0004-s3-atomic-local-storage-contract.md",
    "docs/software/decisions/README.md",
    "scripts/tests/test_s3_contract.py",
    "tests/fixtures/s3/atomic-local-storage-v1/README.md",
    "tests/fixtures/s3/atomic-local-storage-v1/expected-error-corrupt-object.json",
    "tests/fixtures/s3/atomic-local-storage-v1/expected-error-incompatible-schema.json",
    "tests/fixtures/s3/atomic-local-storage-v1/expected-error-unsafe-path.json",
    "tests/fixtures/s3/atomic-local-storage-v1/expected-head-a.json",
    "tests/fixtures/s3/atomic-local-storage-v1/expected-head-b.json",
    "tests/fixtures/s3/atomic-local-storage-v1/expected-recovery.json",
    "tests/fixtures/s3/atomic-local-storage-v1/manifest.json",
    "tests/fixtures/s3/atomic-local-storage-v1/snapshot-semantic-a.json",
    "tests/fixtures/s3/atomic-local-storage-v1/snapshot-semantic-b.json",
    "tests/specifications/s2/contract-bundle.json",
    "tests/specifications/s3/codenoesis-error-v4.schema.json",
    "tests/specifications/s3/e2e_fr_sto_001_atomic_local_storage.json",
    "tests/specifications/s3/local-snapshot-head-v1.schema.json",
    "tests/specifications/s3/local-store-contract-v1.json",
    "tests/specifications/s3/local-store-v1.sql",
    "tests/specifications/s3/publication-failpoints-v1.json",
}

STORAGE_ERROR_CODES = {
    "input.invalid_store_root",
    "storage.unmarked_nonempty_root",
    "storage.incompatible_schema",
    "storage.writer_busy",
    "storage.missing_object",
    "storage.corrupt_object",
    "storage.corrupt_metadata",
    "storage.unsafe_path",
    "publication.head_conflict",
    "publication.failed",
}


def snapshot_id(semantic_hash: str) -> str:
    preimage = ["codenoesis.snapshot-id/v1", semantic_hash]
    return "urn:codenoesis:snapshot:blake3:" + blake3_256(
        canonical_json(preimage)
    )


def artifact_id(content: bytes) -> str:
    digest = blake3_256(b"codenoesis.artifact-id/v1\0" + content)
    return f"urn:codenoesis:artifact:blake3:{digest}"


def semantic_hash(document: dict[str, Any], domain: str) -> str:
    value = dict(document)
    value.pop("semantic_hash")
    return blake3_256(domain.encode() + b"\0" + canonical_json(value))


def commit_oid_with_parent(
    tree: str,
    parent: str | None,
    timestamp: int,
    message: str,
) -> tuple[str, bytes]:
    identity = (
        f"CodeNoesis Fixture <fixture@codenoesis.invalid> {timestamp} +0000"
    )
    headers = [f"tree {tree}"]
    if parent is not None:
        headers.append(f"parent {parent}")
    headers.extend([f"author {identity}", f"committer {identity}"])
    payload = ("\n".join(headers) + f"\n\n{message}").encode()
    return git_oid("commit", payload), payload


class S3ContractTests(unittest.TestCase):
    def test_contract_bundle_binds_every_s3_ratification_artifact(self) -> None:
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
        self.assertEqual(set(paths), S3_BUNDLE_FILES)
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
        match = re.search(r"S3 contract bundle: `sha256:([0-9a-f]{64})`", srs)
        self.assertIsNotNone(match, "SRS must bind the complete S3 bundle")
        self.assertEqual(match.group(1), bundle_sha256)  # type: ignore[union-attr]

    def test_s3_register_oracle_and_ratification_are_exact(self) -> None:
        spec = load_json(SPEC_PATH)
        self.assertEqual(spec["status"], "approved")
        self.assertEqual(set(spec["requirements"]), S3_REQUIREMENTS)
        self.assertEqual(len(spec["requirements"]), len(S3_REQUIREMENTS))
        self.assertEqual(
            spec["ratification"],
            {
                "governance_model": "single_maintainer_bootstrap",
                "product_owner_persona": "Andrea Moretti",
                "persona_is_natural_person": False,
                "accountable_github_actor": "smutti",
                "technical_approver": "smutti",
                "approval_reference": "https://github.com/smutti/codenoesis/pull/29",
                "effective_on": "protected_squash_merge_by_accountable_actor",
                "required_external_approvals": 0,
                "agent_merge_allowed": False,
            },
        )
        srs = SRS_PATH.read_text(encoding="utf-8")
        register = srs.split("### 2.6 S3 ratification register", 1)[1].split(
            "## 3. Product intent and success definition", 1
        )[0]
        registered = re.findall(
            r"^\| `([A-Z]+-[A-Z]+-\d{3})` \| "
            r"(?:Proposed|Approved) \| Approved \|",
            register,
            flags=re.MULTILINE,
        )
        self.assertEqual(set(registered), S3_REQUIREMENTS)
        self.assertEqual(len(registered), len(S3_REQUIREMENTS))
        self.assertNotIn("FR-CLI-001", registered)
        self.assertNotIn("FR-STO-002", registered)
        decision = (ROOT / spec["decision"]).read_text(encoding="utf-8")
        self.assertIn("| Status | Accepted;", decision)
        self.assertIn("authoring agent must not approve or merge", decision)
        self.assertIn("separate policy-binding change", decision)

    def test_acceptance_spec_has_complete_ordered_traceability(self) -> None:
        spec = load_json(SPEC_PATH)
        self.assertEqual(
            [scenario["test_name"] for scenario in spec["scenarios"]],
            list(S3_TEST_ORDER),
        )
        traced = {
            requirement
            for scenario in spec["scenarios"]
            for requirement in scenario["requirements"]
        }
        self.assertEqual(traced, S3_REQUIREMENTS)
        self.assertEqual(
            set(spec["inherited_regressions"]), set(S2_TEST_ORDER)
        )
        self.assertEqual(
            spec["public_command"],
            [
                "noesis",
                "scan",
                "--repository",
                "{repository_path}",
                "--repository-id",
                "urn:codenoesis:fixture:s3-atomic-local-storage-v1",
                "--revision",
                "{revision}",
                "--profile",
                "standard-local-s3",
                "--store",
                "{store_path}",
                "--format",
                "json",
            ],
        )
        self.assertEqual(
            spec["head_probe"],
            {
                "kind": "test_only_process_probe",
                "production_command": False,
                "operation": "load_head",
                "key": "repository_identity",
                "result_contract": "codenoesis.local-snapshot-head/v1",
                "failpoint_control": "injected_boundary_callback",
                "termination": "external_process_termination",
            },
        )
        for path in [
            spec["decision"],
            spec["fixture"],
            spec["inherited_contract_bundle"],
            *spec["schemas"].values(),
            *spec["contracts"].values(),
            *spec["goldens"].values(),
        ]:
            self.assertTrue((ROOT / path).is_file(), path)

    def test_expected_red_is_exact_and_rejects_false_red(self) -> None:
        expected_red = load_json(SPEC_PATH)["expected_red"]
        self.assertEqual(
            expected_red,
            {
                "test_command": (
                    "cargo test --test e2e_fr_sto_001_atomic_local_storage"
                ),
                "precondition": (
                    "The future TDD branch contains only the black-box S3 "
                    "harness and reviewed storage fixture while production "
                    "remains at merged S2 behavior."
                ),
                "runner_expected_exit": (
                    "nonzero because the acceptance assertion fails"
                ),
                "subject_observed_exit_code": 2,
                "subject_observed_stderr_schema": "codenoesis.error/v2",
                "subject_observed_stderr_code": "input.invalid_profile",
                "subject_expected_exit_code": 0,
                "expected_artifact": "codenoesis.repository-snapshot/v3",
                "expected_head": "codenoesis.local-snapshot-head/v1",
                "accepted_reason": (
                    "Merged S2 routes an unknown explicit profile through the "
                    "S1 parser and rejects standard-local-s3, so no durable "
                    "head can exist."
                ),
                "rejected_reasons": [
                    "compilation failure",
                    "missing test target",
                    "missing or corrupt fixture",
                    "schema or DDL harness failure",
                    "dependency or network outage",
                    "probe failure outside a named boundary",
                    "timeout",
                    "timing race",
                    "unexpected panic",
                    "a modified S3 oracle",
                ],
            },
        )

    def test_store_contract_fixes_identity_durability_and_boundaries(self) -> None:
        contract = load_json(STORE_CONTRACT_PATH)
        self.assertEqual(
            set(contract),
            {
                "schema_version",
                "status",
                "project_key",
                "snapshot_identity",
                "artifact_identity",
                "sqlite",
                "cas",
                "publication",
                "head_read",
                "cleanup",
                "path_policy",
                "platform_durability",
                "versioning",
            },
        )
        self.assertEqual(
            contract["schema_version"], "codenoesis.local-store-contract/v1"
        )
        self.assertEqual(contract["status"], "approved")
        self.assertEqual(contract["project_key"], "repository_identity")
        self.assertEqual(
            contract["snapshot_identity"],
            {
                "algorithm": "blake3-256",
                "canonicalization": "RFC8785",
                "preimage": [
                    "codenoesis.snapshot-id/v1",
                    "repository_snapshot_v3_semantic_hash_value",
                ],
                "urn_prefix": "urn:codenoesis:snapshot:blake3:",
            },
        )
        self.assertEqual(
            contract["artifact_identity"],
            {
                "algorithm": "blake3-256",
                "preimage": (
                    "UTF-8(codenoesis.artifact-id/v1) + 0x00 + exact bytes"
                ),
                "urn_prefix": "urn:codenoesis:artifact:blake3:",
            },
        )
        sqlite_contract = contract["sqlite"]
        self.assertEqual(
            sqlite_contract["pragmas"],
            {
                "application_id": 1129205587,
                "busy_timeout": 0,
                "foreign_keys": True,
                "journal_mode": "WAL",
                "synchronous": "FULL",
                "trusted_schema": False,
                "user_version": 1,
            },
        )
        self.assertEqual(set(sqlite_contract["tables"]), SQLITE_TABLES)
        self.assertEqual(
            set(sqlite_contract["immutable_tables"]), IMMUTABLE_TABLES
        )
        self.assertEqual(sqlite_contract["mutable_tables"], ["project_heads"])
        self.assertEqual(
            contract["cas"]["required_roles"],
            ["snapshot_semantic", "knowledge_graph", "extraction_chunk"],
        )
        self.assertEqual(
            contract["publication"]["boundaries"], list(FAILPOINTS)
        )
        self.assertEqual(
            contract["publication"]["commit_point"], "sqlite_after_commit"
        )
        self.assertEqual(
            contract["publication"]["pre_commit_visible_head"], "previous"
        )
        self.assertEqual(
            contract["publication"]["post_commit_visible_head"], "new"
        )
        self.assertEqual(
            contract["head_read"]["corruption_policy"],
            "fail_closed_without_fallback",
        )
        self.assertEqual(
            contract["cleanup"]["reachable_object_policy"], "never_delete"
        )
        self.assertFalse(contract["publication"]["production_failpoint_switch"])

    def test_sqlite_ddl_is_executable_strict_and_immutable(self) -> None:
        ddl = DDL_PATH.read_text(encoding="utf-8")
        self.assertNotIn("DROP ", ddl.upper())
        self.assertNotIn("ALTER ", ddl.upper())
        with tempfile.TemporaryDirectory() as temporary:
            database = Path(temporary) / "metadata.sqlite3"
            connection = sqlite3.connect(database)
            connection.executescript(ddl)
            tables = {
                row[0]
                for row in connection.execute(
                    "SELECT name FROM sqlite_schema "
                    "WHERE type = 'table' AND name NOT LIKE 'sqlite_%'"
                )
            }
            self.assertEqual(tables, SQLITE_TABLES)
            self.assertEqual(
                connection.execute("PRAGMA application_id").fetchone()[0],
                1129205587,
            )
            self.assertEqual(
                connection.execute("PRAGMA user_version").fetchone()[0], 1
            )
            self.assertEqual(
                connection.execute("PRAGMA journal_mode").fetchone()[0].lower(),
                "wal",
            )
            self.assertEqual(
                connection.execute("PRAGMA foreign_keys").fetchone()[0], 1
            )
            triggers = {
                row[0]
                for row in connection.execute(
                    "SELECT name FROM sqlite_schema WHERE type = 'trigger'"
                )
            }
            expected_triggers = {
                f"{table}_forbid_{operation}"
                for table in IMMUTABLE_TABLES
                for operation in ("update", "delete")
            }
            self.assertEqual(triggers, expected_triggers)
            connection.execute(
                "INSERT INTO store_metadata(key, value) VALUES (?, ?)",
                ("schema_version", "codenoesis.local-store/v1"),
            )
            with self.assertRaises(sqlite3.IntegrityError):
                connection.execute(
                    "UPDATE store_metadata SET value = ? WHERE key = ?",
                    ("changed", "schema_version"),
                )
            connection.close()

    def test_failpoint_matrix_has_one_reviewed_outcome_per_boundary(self) -> None:
        matrix = load_json(FAILPOINT_PATH)
        self.assertEqual(
            matrix["schema_version"],
            "codenoesis.publication-failpoints/v1",
        )
        boundaries = matrix["boundaries"]
        self.assertEqual(
            [boundary["name"] for boundary in boundaries], list(FAILPOINTS)
        )
        self.assertEqual(
            [boundary["order"] for boundary in boundaries],
            list(range(1, len(FAILPOINTS) + 1)),
        )
        for boundary in boundaries[:-1]:
            self.assertEqual(boundary["restart_first_publication_head"], None)
            self.assertEqual(boundary["restart_replacement_head"], "A")
            self.assertEqual(boundary["retry_head"], "B")
        committed = boundaries[-1]
        self.assertEqual(committed["restart_first_publication_head"], "A")
        self.assertEqual(committed["restart_replacement_head"], "B")
        self.assertEqual(committed["retry_head"], "B")
        self.assertEqual(
            matrix["injection"],
            {
                "production_switch": False,
                "probe": "test_only_process",
                "control": "injected_boundary_callback",
                "termination": "external_process_termination",
            },
        )

    def test_public_schemas_are_strict_closed_and_versioned(self) -> None:
        head_schema = load_json(HEAD_SCHEMA_PATH)
        error_schema = load_json(ERROR_SCHEMA_PATH)
        self.assertEqual(
            head_schema["$id"],
            "urn:codenoesis:schema:local-snapshot-head:v1",
        )
        self.assertEqual(
            error_schema["$id"], "urn:codenoesis:schema:error:v4"
        )
        for schema in (head_schema, error_schema):
            self.assertEqual(
                schema["$schema"],
                "https://json-schema.org/draft/2020-12/schema",
            )
            self.assertFalse(schema["additionalProperties"])
            self.assertEqual(set(schema["required"]), set(schema["properties"]))
        self.assertEqual(
            head_schema["properties"]["schema_version"]["const"],
            "codenoesis.local-snapshot-head/v1",
        )
        error_codes = set(error_schema["properties"]["code"]["enum"])
        self.assertTrue(STORAGE_ERROR_CODES <= error_codes)
        self.assertIn("internal.unexpected", error_codes)
        self.assertEqual(
            error_schema["properties"]["schema_version"]["const"],
            "codenoesis.error/v4",
        )
        retryable_codes = set(error_schema["$defs"]["retryable_codes"]["enum"])
        self.assertEqual(
            retryable_codes,
            {"publication.head_conflict", "storage.writer_busy"},
        )

    def test_fixture_reproduces_revisions_and_canonical_head_artifacts(
        self,
    ) -> None:
        manifest = load_json(FIXTURE_ROOT / "manifest.json")
        s2_manifest = load_json(S2_FIXTURE_ROOT / "manifest.json")
        self.assertEqual(
            manifest["schema_version"], "codenoesis.fixture.git/v4"
        )
        self.assertEqual(manifest["provenance"]["kind"], "synthetic_first_party")
        self.assertFalse(manifest["provenance"]["third_party_material"])
        self.assertEqual(
            (FIXTURE_ROOT / manifest["provenance"]["license_file"]).resolve(),
            ROOT / "LICENSE",
        )
        self.assertEqual(
            manifest["repository"]["source_fixture"],
            "tests/fixtures/s2/rust-knowledge-v1",
        )
        self.assertEqual(
            manifest["repository"]["tree_oid"],
            s2_manifest["revision"]["tree_oid"],
        )
        revisions = manifest["revisions"]
        self.assertEqual([revision["label"] for revision in revisions], ["A", "B"])
        previous = None
        for revision in revisions:
            calculated, payload = commit_oid_with_parent(
                manifest["repository"]["tree_oid"],
                previous,
                revision["timestamp"],
                revision["message"],
            )
            self.assertEqual(calculated, revision["commit_oid"])
            self.assertEqual(
                hashlib.sha256(payload).hexdigest(),
                revision["commit_payload_sha256"],
            )
            self.assertEqual(revision["parent"], previous)
            previous = calculated

        for label, generation in (("a", 1), ("b", 2)):
            semantic = load_json(FIXTURE_ROOT / f"snapshot-semantic-{label}.json")
            head = load_json(FIXTURE_ROOT / f"expected-head-{label}.json")
            revision = revisions[generation - 1]
            self.assertEqual(
                semantic["repository"]["commit_oid"], revision["commit_oid"]
            )
            snapshot_digest = blake3_256(
                b"codenoesis.repository-snapshot.semantic.v3\0"
                + canonical_json(semantic)
            )
            self.assertEqual(
                head["semantic_hash"],
                {
                    "algorithm": "blake3-256",
                    "domain": "codenoesis.repository-snapshot.semantic.v3",
                    "value": snapshot_digest,
                },
            )
            self.assertEqual(head["snapshot_id"], snapshot_id(snapshot_digest))
            self.assertEqual(head["generation"], generation)
            expected_artifacts = [
                (
                    "snapshot_semantic",
                    0,
                    semantic,
                    head["semantic_hash"],
                ),
                (
                    "knowledge_graph",
                    0,
                    semantic["knowledge_graph"],
                    semantic["knowledge_graph"]["semantic_hash"],
                ),
                *[
                    (
                        "extraction_chunk",
                        index,
                        chunk,
                        chunk["semantic_hash"],
                    )
                    for index, chunk in enumerate(
                        semantic["extraction_chunks"]
                    )
                ],
            ]
            self.assertEqual(len(head["artifacts"]), len(expected_artifacts))
            for artifact, expected in zip(
                head["artifacts"], expected_artifacts
            ):
                role, ordinal, document, document_hash = expected
                content = canonical_json(document)
                self.assertEqual(artifact["role"], role)
                self.assertEqual(artifact["ordinal"], ordinal)
                self.assertEqual(artifact["artifact_id"], artifact_id(content))
                self.assertEqual(artifact["byte_length"], len(content))
                self.assertEqual(artifact["semantic_hash"], document_hash)
            graph = semantic["knowledge_graph"]
            self.assertEqual(
                graph["semantic_hash"]["value"],
                semantic_hash(
                    graph, "codenoesis.knowledge-graph.semantic.v1"
                ),
            )
            for chunk in semantic["extraction_chunks"]:
                self.assertEqual(
                    chunk["semantic_hash"]["value"],
                    semantic_hash(
                        chunk, "codenoesis.extraction-chunk.semantic.v1"
                    ),
                )

    def test_recovery_and_corruption_goldens_are_closed(self) -> None:
        recovery = load_json(FIXTURE_ROOT / "expected-recovery.json")
        matrix = load_json(FAILPOINT_PATH)
        self.assertEqual(
            recovery["schema_version"], "codenoesis.recovery-oracle/v1"
        )
        self.assertEqual(
            recovery["failpoints"],
            [
                {
                    "name": boundary["name"],
                    "first_publication_head": boundary[
                        "restart_first_publication_head"
                    ],
                    "replacement_head": boundary[
                        "restart_replacement_head"
                    ],
                    "retry_head": boundary["retry_head"],
                    "partial_head_visible": False,
                }
                for boundary in matrix["boundaries"]
            ],
        )
        self.assertEqual(
            recovery["idempotency"],
            {
                "same_snapshot_repetitions": 100,
                "snapshot_row_count": 1,
                "head_generation_delta": 0,
                "duplicate_artifact_rows": 0,
                "result": "same_head",
            },
        )
        self.assertEqual(
            recovery["cleanup"],
            {
                "reachable_objects_deleted": 0,
                "reachable_objects_reverified": True,
                "orphan_objects_deleted": "all_reviewed_orphans",
                "head_after_sweep": "B",
            },
        )
        errors = {
            "storage.corrupt_object": load_json(
                FIXTURE_ROOT / "expected-error-corrupt-object.json"
            ),
            "storage.incompatible_schema": load_json(
                FIXTURE_ROOT / "expected-error-incompatible-schema.json"
            ),
            "storage.unsafe_path": load_json(
                FIXTURE_ROOT / "expected-error-unsafe-path.json"
            ),
        }
        for code, error in errors.items():
            self.assertEqual(error["schema_version"], "codenoesis.error/v4")
            self.assertEqual(error["code"], code)
            self.assertFalse(error["retryable"])
            self.assertNotIn(str(ROOT), canonical_json(error).decode())
            self.assertNotIn("store_path", error["context"])


if __name__ == "__main__":
    unittest.main()
