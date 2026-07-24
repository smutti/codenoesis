from __future__ import annotations

import copy
import hashlib
import re
import unicodedata
import unittest
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

from test_s1_contract import (
    INHERITED_S0_TESTS,
    S1_TEST_ORDER,
    blake3_256,
    canonical_json,
    commit_oid,
    git_oid,
    load_json,
    tree_oid,
)


ROOT = Path(__file__).resolve().parents[2]
FIXTURE_ROOT = ROOT / "tests" / "fixtures" / "s2" / "rust-knowledge-v1"
SOURCE_ROOT = FIXTURE_ROOT / "revision-a"
SPEC_PATH = (
    ROOT
    / "tests"
    / "specifications"
    / "s2"
    / "e2e_fr_ext_002_rust_knowledge.json"
)
ONTOLOGY_PATH = (
    ROOT / "tests" / "specifications" / "s2" / "rust-ontology-v1.json"
)
GRAPH_PATH = FIXTURE_ROOT / "expected-graph-a.json"
EXTRACTION_PATH = FIXTURE_ROOT / "expected-extraction-a.json"
SRS_PATH = ROOT / "docs" / "software" / "software-requirements-specification.md"
BUNDLE_PATH = ROOT / "tests" / "specifications" / "s2" / "contract-bundle.json"

S2_REQUIREMENTS = {
    "DR-IDN-001",
    "FR-EXT-001",
    "FR-EXT-002",
    "FR-KNW-001",
    "FR-KNW-002",
    "FR-KNW-003",
}

S2_TEST_ORDER = (
    "e2e_fr_ext_002_rust_knowledge",
    "conf_fr_ext_001_extraction_chunk_v1",
    "gt_fr_ext_002_reviewed_rust_graph",
    "gt_fr_ext_002_malformed_syntax_is_explicit",
    "gt_dr_idn_001_unicode_normalization_collision",
    "pt_dr_idn_001_stable_ids_ignore_order_and_revision",
    "pt_fr_knw_001_graph_invariants",
    "pt_fr_knw_002_claim_state_machine",
    "pt_fr_knw_003_rule_provenance_replays",
    "fz_fr_ext_001_extraction_contract_seed_corpus",
    "fz_fr_ext_002_rust_parser_seed_corpus",
    "sec_fr_ext_002_target_never_executes",
    "conf_fr_ext_001_snapshot_v3_and_error_v3",
)

ENTITY_KINDS = {
    "rust.crate",
    "rust.enum",
    "rust.function",
    "rust.method",
    "rust.module",
    "rust.struct",
    "rust.symbol_reference",
    "rust.trait",
    "rust.type_alias",
    "source.file",
}

RELATIONSHIP_KINDS = {"CONTAINS", "DEFINES", "IMPLEMENTS", "IMPORTS"}

CLAIM_STATES = {
    "candidate",
    "confirmed",
    "derived_fact",
    "deterministic_fact",
    "rejected",
    "reviewed_inference",
    "superseded",
}

CLAIM_TRANSITIONS = {
    ("candidate", "confirmed"),
    ("candidate", "rejected"),
    ("candidate", "reviewed_inference"),
    ("candidate", "superseded"),
    ("confirmed", "superseded"),
    ("derived_fact", "superseded"),
    ("deterministic_fact", "superseded"),
    ("rejected", "superseded"),
    ("reviewed_inference", "confirmed"),
    ("reviewed_inference", "rejected"),
    ("reviewed_inference", "superseded"),
}

S2_BUNDLE_FILES = {
    "LICENSE",
    "docs/software/decisions/0003-s2-rust-knowledge-contract.md",
    "scripts/tests/test_s2_contract.py",
    "tests/fixtures/s2/rust-knowledge-v1/README.md",
    "tests/fixtures/s2/rust-knowledge-v1/expected-error-nfc-collision.json",
    "tests/fixtures/s2/rust-knowledge-v1/expected-extraction-a.json",
    "tests/fixtures/s2/rust-knowledge-v1/expected-graph-a.json",
    "tests/fixtures/s2/rust-knowledge-v1/manifest.json",
    "tests/fixtures/s2/rust-knowledge-v1/revision-a/Cargo.toml",
    "tests/fixtures/s2/rust-knowledge-v1/revision-a/src/lib.rs",
    "tests/specifications/s1/contract-bundle.json",
    "tests/specifications/s2/codenoesis-error-v3.schema.json",
    "tests/specifications/s2/e2e_fr_ext_002_rust_knowledge.json",
    "tests/specifications/s2/extraction-chunk-v1.schema.json",
    "tests/specifications/s2/knowledge-graph-v1.schema.json",
    "tests/specifications/s2/repository-snapshot-v3.schema.json",
    "tests/specifications/s2/rust-ontology-v1.json",
    "tests/specifications/s2/source-evidence-v2.schema.json",
}


def stable_entity_id(
    repository_identity: str, kind: str, canonical_identity: str
) -> str:
    preimage = [
        "codenoesis.entity-id/v1",
        repository_identity,
        "rust",
        kind,
        canonical_identity,
    ]
    return "urn:codenoesis:entity:blake3:" + blake3_256(
        canonical_json(preimage)
    )


def stable_relationship_id(
    repository_identity: str,
    ontology_version: str,
    kind: str,
    source_entity_id: str,
    target_entity_id: str,
) -> str:
    preimage = [
        "codenoesis.relationship-id/v1",
        repository_identity,
        ontology_version,
        kind,
        source_entity_id,
        target_entity_id,
    ]
    return "urn:codenoesis:relationship:blake3:" + blake3_256(
        canonical_json(preimage)
    )


def stable_claim_id(
    repository_identity: str,
    ontology_version: str,
    subject_kind: str,
    subject_id: str,
) -> str:
    preimage = [
        "codenoesis.claim-id/v1",
        repository_identity,
        ontology_version,
        subject_kind,
        subject_id,
    ]
    return "urn:codenoesis:claim:blake3:" + blake3_256(
        canonical_json(preimage)
    )


def extraction_chunk_id(
    repository_identity: str,
    commit: str,
    blob: str,
    path: str,
    extractor_version: str,
) -> str:
    preimage = [
        "codenoesis.extraction-chunk-id/v1",
        repository_identity,
        commit,
        blob,
        path,
        extractor_version,
    ]
    return "urn:codenoesis:extraction-chunk:blake3:" + blake3_256(
        canonical_json(preimage)
    )


def semantic_hash(document: dict[str, Any], domain: str) -> str:
    semantic = copy.deepcopy(document)
    semantic.pop("semantic_hash")
    return blake3_256(domain.encode() + b"\0" + canonical_json(semantic))


class S2ContractTests(unittest.TestCase):
    def test_contract_bundle_binds_every_s2_ratification_artifact(self) -> None:
        manifest = load_json(BUNDLE_PATH)
        self.assertEqual(set(manifest), {"schema_version", "files", "bundle_sha256"})
        self.assertEqual(
            manifest["schema_version"], "codenoesis.contract-bundle/v1"
        )
        files = manifest["files"]
        paths = [entry["path"] for entry in files]
        self.assertEqual(paths, sorted(paths))
        self.assertEqual(set(paths), S2_BUNDLE_FILES)
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
        match = re.search(r"S2 contract bundle: `sha256:([0-9a-f]{64})`", srs)
        self.assertIsNotNone(match, "SRS must bind the complete S2 contract bundle")
        self.assertEqual(match.group(1), bundle_sha256)  # type: ignore[union-attr]

    def test_s2_register_oracle_and_ratification_are_exact(self) -> None:
        spec = load_json(SPEC_PATH)
        self.assertEqual(spec["status"], "approved")
        self.assertEqual(set(spec["requirements"]), S2_REQUIREMENTS)
        self.assertEqual(len(spec["requirements"]), len(S2_REQUIREMENTS))
        self.assertEqual(
            spec["ratification"],
            {
                "governance_model": "single_maintainer_bootstrap",
                "product_owner_persona": "Andrea Moretti",
                "persona_is_natural_person": False,
                "accountable_github_actor": "smutti",
                "technical_approver": "smutti",
                "approval_reference": (
                    "pending protected pull request for "
                    "https://github.com/smutti/codenoesis/issues/22"
                ),
                "effective_on": "protected_squash_merge_by_accountable_actor",
                "required_external_approvals": 0,
                "agent_merge_allowed": False,
            },
        )
        srs = SRS_PATH.read_text(encoding="utf-8")
        register = srs.split("### 2.5 S2 ratification register", 1)[1].split(
            "## 3. Product intent and success definition", 1
        )[0]
        registered = re.findall(
            r"^\| `([A-Z]+-[A-Z]+-\d{3})` \| Approved \| Approved \|",
            register,
            flags=re.MULTILINE,
        )
        self.assertEqual(set(registered), S2_REQUIREMENTS)
        self.assertEqual(len(registered), len(S2_REQUIREMENTS))
        self.assertNotIn("FR-EXT-006", registered)
        decision = (ROOT / spec["decision"]).read_text(encoding="utf-8")
        self.assertIn("| Status | Accepted;", decision)
        self.assertIn("authoring agent must not approve or merge", decision)
        self.assertIn("separate policy-binding change", decision)

    def test_acceptance_specification_has_complete_ordered_traceability(
        self,
    ) -> None:
        spec = load_json(SPEC_PATH)
        self.assertEqual(
            [scenario["test_name"] for scenario in spec["scenarios"]],
            list(S2_TEST_ORDER),
        )
        scenario_requirements = {
            requirement
            for scenario in spec["scenarios"]
            for requirement in scenario["requirements"]
        }
        self.assertEqual(scenario_requirements, S2_REQUIREMENTS)
        inherited = set(spec["inherited_regressions"])
        self.assertEqual(inherited, INHERITED_S0_TESTS | set(S1_TEST_ORDER))
        self.assertEqual(
            spec["public_command"],
            [
                "noesis",
                "scan",
                "--repository",
                "{repository_path}",
                "--repository-id",
                "urn:codenoesis:fixture:s2-rust-knowledge-v1",
                "--revision",
                "{revision}",
                "--profile",
                "standard-local-s2",
                "--format",
                "json",
            ],
        )
        self.assertEqual(
            spec["expected_red"],
            {
                "test_command": "cargo test --test e2e_fr_ext_002_rust_knowledge",
                "precondition": (
                    "The future TDD branch contains the black-box S2 harness "
                    "and reviewed fixture while production remains at merged "
                    "S1 behavior."
                ),
                "runner_expected_exit": (
                    "nonzero because the acceptance assertion fails"
                ),
                "subject_observed_exit_code": 2,
                "subject_observed_stderr_schema": "codenoesis.error/v2",
                "subject_observed_stderr_code": "input.invalid_profile",
                "subject_expected_exit_code": 0,
                "expected_artifact": "codenoesis.repository-snapshot/v3",
                "accepted_reason": (
                    "Merged S1 recognizes --profile but rejects "
                    "standard-local-s2, so the approved S2 public behavior is "
                    "absent."
                ),
                "rejected_reasons": [
                    "compilation failure",
                    "missing test target",
                    "missing or corrupt fixture",
                    "schema harness failure",
                    "dependency or network outage",
                    "parser crash",
                    "timing race",
                    "unexpected panic",
                    "a modified S2 oracle",
                ],
            },
        )
        for path in [
            spec["decision"],
            spec["fixture"],
            spec["ontology"],
            *spec["schemas"].values(),
            *spec["goldens"].values(),
        ]:
            self.assertTrue((ROOT / path).is_file(), path)
        self.assertEqual(
            spec["graph_contract"]["fixture_counts"],
            {
                "entities": 11,
                "relationships": 15,
                "claims": 26,
                "deterministic_fact": 25,
                "derived_fact": 1,
                "evidence": 13,
                "diagnostics": 0,
                "coverage_gaps": 3,
            },
        )

    def test_fixture_reproduces_git_objects_and_reviewed_variants(self) -> None:
        manifest = load_json(FIXTURE_ROOT / "manifest.json")
        provenance = manifest["provenance"]
        self.assertEqual(provenance["kind"], "synthetic_first_party")
        self.assertFalse(provenance["third_party_material"])
        self.assertEqual(provenance["license_spdx"], "Apache-2.0")
        self.assertEqual(
            (FIXTURE_ROOT / provenance["license_file"]).resolve(),
            (ROOT / "LICENSE").resolve(),
        )
        revision = manifest["revision"]
        file_entries = revision["files"]
        self.assertEqual(
            [entry["path"] for entry in file_entries], ["Cargo.toml", "src/lib.rs"]
        )
        object_ids: dict[str, str] = {}
        for entry in file_entries:
            source = (SOURCE_ROOT / entry["path"]).read_bytes()
            self.assertEqual(len(source), entry["byte_length"])
            self.assertEqual(hashlib.sha256(source).hexdigest(), entry["source_sha256"])
            object_id = git_oid("blob", source)
            self.assertEqual(object_id, entry["blob_oid"])
            object_ids[entry["path"]] = object_id
        source_tree, source_payload_bytes = tree_oid(
            [("100644", "lib.rs", object_ids["src/lib.rs"])]
        )
        root_tree, root_payload_bytes = tree_oid(
            [
                ("100644", "Cargo.toml", object_ids["Cargo.toml"]),
                ("40000", "src", source_tree),
            ]
        )
        self.assertEqual(
            revision["trees"],
            [
                {
                    "path": "",
                    "entry_count": 2,
                    "payload_bytes": root_payload_bytes,
                    "tree_oid": root_tree,
                },
                {
                    "path": "src",
                    "entry_count": 1,
                    "payload_bytes": source_payload_bytes,
                    "tree_oid": source_tree,
                },
            ],
        )
        self.assertEqual(root_tree, revision["tree_oid"])
        repository = manifest["repository"]
        commit, payload = commit_oid(
            root_tree, repository["timestamp"], repository["message"]
        )
        self.assertEqual(commit, revision["commit_oid"])
        self.assertEqual(
            hashlib.sha256(payload).hexdigest(),
            revision["commit_payload_sha256"],
        )
        source = (SOURCE_ROOT / "src/lib.rs").read_bytes()
        malformed = manifest["generated_variants"]["malformed_token"]
        anchor = malformed["anchor_utf8"].encode()
        insertion = malformed["insert_utf8"].encode()
        anchor_start = source.index(anchor)
        malformed_source = source[:anchor_start] + insertion + source[anchor_start:]
        self.assertEqual(len(malformed_source), malformed["result_byte_length"])
        self.assertEqual(
            hashlib.sha256(malformed_source).hexdigest(),
            malformed["result_sha256"],
        )
        collision = manifest["generated_variants"]["nfc_collision"]
        collision_source = source + collision["append_utf8"].encode()
        self.assertEqual(len(collision_source), collision["result_byte_length"])
        self.assertEqual(
            hashlib.sha256(collision_source).hexdigest(),
            collision["result_sha256"],
        )
        self.assertNotEqual(
            collision["first_source_spelling"],
            collision["second_source_spelling"],
        )
        self.assertEqual(
            unicodedata.normalize("NFC", collision["first_source_spelling"]),
            unicodedata.normalize("NFC", collision["second_source_spelling"]),
        )

    def test_ontology_v1_is_closed_and_versioned(self) -> None:
        ontology = load_json(ONTOLOGY_PATH)
        self.assertEqual(
            set(ontology),
            {
                "schema_version",
                "ontology_version",
                "status",
                "language",
                "entity_common_properties",
                "entity_kinds",
                "property_domains",
                "relationship_common_properties",
                "relationship_matrix",
                "cardinalities",
                "graph_invariants",
                "identity",
                "normalization",
                "claim_states",
                "claim_transitions",
                "claim_origins",
                "deterministic_rules",
                "ordering",
                "versioning",
                "deferred_entity_families",
                "deferred_resolution",
            },
        )
        self.assertEqual(ontology["status"], "approved")
        self.assertEqual(
            ontology["ontology_version"], "codenoesis.ontology/rust/v1"
        )
        kinds = [entry["kind"] for entry in ontology["entity_kinds"]]
        self.assertEqual(kinds, sorted(kinds))
        self.assertEqual(set(kinds), ENTITY_KINDS)
        self.assertEqual(set(ontology["claim_states"]), CLAIM_STATES)
        self.assertEqual(
            {tuple(transition) for transition in ontology["claim_transitions"]},
            CLAIM_TRANSITIONS,
        )
        self.assertEqual(
            len(ontology["claim_transitions"]), len(CLAIM_TRANSITIONS)
        )
        self.assertEqual(
            ontology["versioning"],
            {
                "write_policy": "write exactly codenoesis.ontology/rust/v1",
                "mutation_policy": "immutable semantic contract",
                "change_policy": (
                    "every semantic change requires a new ontology version"
                ),
                "migration_policy": (
                    "reviewed migration or deterministic rebuild evidence"
                ),
                "unsupported_version_policy": "reject before graph ingestion",
            },
        )
        self.assertEqual(
            ontology["claim_origins"]["model"], ["candidate"]
        )
        self.assertNotIn(
            "deterministic_fact", ontology["claim_origins"]["model"]
        )
        self.assertNotIn("confirmed", ontology["claim_origins"]["model"])
        relationship_kinds = {
            row["kind"] for row in ontology["relationship_matrix"]
        }
        self.assertEqual(relationship_kinds, RELATIONSHIP_KINDS)
        self.assertEqual(
            ontology["deterministic_rules"],
            [
                {
                    "version": "codenoesis.rule.rust-file-containment/s2-v1",
                    "output_relationship_kind": "CONTAINS",
                    "required_input_claims": ["rust.crate", "source.file"],
                    "output_state": "derived_fact",
                }
            ],
        )

    def test_public_schemas_are_strict_and_cross_versioned(self) -> None:
        specification = load_json(SPEC_PATH)
        schemas = {
            name: load_json(ROOT / path)
            for name, path in specification["schemas"].items()
        }
        self.assertEqual(
            {schema["$id"] for schema in schemas.values()},
            {
                "urn:codenoesis:schema:error:v3",
                "urn:codenoesis:schema:extraction-chunk:v1",
                "urn:codenoesis:schema:knowledge-graph:v1",
                "urn:codenoesis:schema:repository-snapshot:v3",
                "urn:codenoesis:schema:source-evidence:v2",
            },
        )
        for schema in schemas.values():
            self.assertEqual(
                schema["$schema"], "https://json-schema.org/draft/2020-12/schema"
            )
            self.assertFalse(schema["additionalProperties"])
            self.assertEqual(len(schema["required"]), len(set(schema["required"])))
        graph_schema = schemas["knowledge_graph"]
        self.assertEqual(
            set(graph_schema["$defs"]["entity_kind"]["enum"]), ENTITY_KINDS
        )
        self.assertEqual(
            set(graph_schema["$defs"]["relationship_kind"]["enum"]),
            RELATIONSHIP_KINDS,
        )
        self.assertEqual(
            set(graph_schema["$defs"]["claim"]["properties"]["state"]["enum"]),
            CLAIM_STATES,
        )
        snapshot_schema = schemas["snapshot"]
        semantic_properties = snapshot_schema["properties"]["semantic"][
            "properties"
        ]
        self.assertEqual(
            semantic_properties["configuration"]["$ref"],
            "#/$defs/configuration",
        )
        self.assertEqual(
            snapshot_schema["$defs"]["configuration"]["properties"]["profile"][
                "const"
            ],
            "standard-local-s2",
        )
        self.assertEqual(
            semantic_properties["inventory"]["$ref"],
            "urn:codenoesis:schema:repository-snapshot:v2#/$defs/inventory",
        )
        self.assertEqual(
            semantic_properties["knowledge_graph"]["$ref"],
            "urn:codenoesis:schema:knowledge-graph:v1",
        )
        error_codes = set(schemas["error"]["properties"]["code"]["enum"])
        self.assertEqual(error_codes, set(specification["error_codes"]))

    def test_reviewed_ids_hashes_and_order_are_independently_reproducible(
        self,
    ) -> None:
        graph = load_json(GRAPH_PATH)
        extraction = load_json(EXTRACTION_PATH)
        repository_identity = graph["repository"]["identity"]
        ontology_version = graph["ontology_version"]
        entities = graph["entities"]
        relationships = graph["relationships"]
        claims = graph["claims"]
        self.assertEqual(
            [entity["entity_id"] for entity in entities],
            sorted(entity["entity_id"] for entity in entities),
        )
        self.assertEqual(
            [relationship["relationship_id"] for relationship in relationships],
            sorted(
                relationship["relationship_id"]
                for relationship in relationships
            ),
        )
        self.assertEqual(
            [claim["claim_id"] for claim in claims],
            sorted(claim["claim_id"] for claim in claims),
        )
        for entity in entities:
            self.assertEqual(
                entity["entity_id"],
                stable_entity_id(
                    repository_identity,
                    entity["kind"],
                    entity["canonical_identity"],
                ),
            )
            self.assertEqual(
                entity["canonical_identity"],
                unicodedata.normalize("NFC", entity["canonical_identity"]),
            )
            self.assertEqual(
                entity["claim_id"],
                stable_claim_id(
                    repository_identity,
                    ontology_version,
                    "entity",
                    entity["entity_id"],
                ),
            )
        for relationship in relationships:
            self.assertEqual(
                relationship["relationship_id"],
                stable_relationship_id(
                    repository_identity,
                    ontology_version,
                    relationship["kind"],
                    relationship["source_entity_id"],
                    relationship["target_entity_id"],
                ),
            )
            self.assertEqual(
                relationship["claim_id"],
                stable_claim_id(
                    repository_identity,
                    ontology_version,
                    "relationship",
                    relationship["relationship_id"],
                ),
            )
        self.assertEqual(
            graph["semantic_hash"]["value"],
            semantic_hash(graph, "codenoesis.knowledge-graph.semantic.v1"),
        )
        self.assertEqual(
            extraction["semantic_hash"]["value"],
            semantic_hash(
                extraction, "codenoesis.extraction-chunk.semantic.v1"
            ),
        )
        source = extraction["source"]
        self.assertEqual(
            extraction["chunk_id"],
            extraction_chunk_id(
                repository_identity,
                extraction["repository"]["commit_oid"],
                source["blob_oid"],
                source["path"],
                extraction["extractor_version"],
            ),
        )
        self.assertEqual(
            graph["semantic_hash"]["value"],
            load_json(SPEC_PATH)["contract_constants"]["graph_hash"],
        )

    def test_graph_endpoint_cardinality_and_claim_invariants_are_exact(
        self,
    ) -> None:
        graph = load_json(GRAPH_PATH)
        ontology = load_json(ONTOLOGY_PATH)
        entities = {entity["entity_id"]: entity for entity in graph["entities"]}
        relationships = graph["relationships"]
        claims = {claim["claim_id"]: claim for claim in graph["claims"]}
        evidence = {
            item["evidence_id"]: item for item in graph["evidence"]
        }
        self.assertEqual(len(entities), 11)
        self.assertEqual(len(relationships), 15)
        self.assertEqual(len(claims), 26)
        self.assertEqual(len(evidence), 13)
        allowed_endpoints = {
            (row["kind"], source_kind, target_kind)
            for row in ontology["relationship_matrix"]
            for source_kind in row["sources"]
            for target_kind in row["targets"]
        }
        tuples = set()
        incoming: dict[tuple[str, str], int] = Counter()
        outgoing: dict[str, int] = Counter()
        defines_children: dict[str, list[str]] = defaultdict(list)
        for relationship in relationships:
            source_id = relationship["source_entity_id"]
            target_id = relationship["target_entity_id"]
            self.assertIn(source_id, entities)
            self.assertIn(target_id, entities)
            endpoint = (
                relationship["kind"],
                entities[source_id]["kind"],
                entities[target_id]["kind"],
            )
            self.assertIn(endpoint, allowed_endpoints)
            relation_tuple = (relationship["kind"], source_id, target_id)
            self.assertNotIn(relation_tuple, tuples)
            tuples.add(relation_tuple)
            incoming[(target_id, relationship["kind"])] += 1
            outgoing[source_id] += 1
            if relationship["kind"] == "DEFINES":
                defines_children[source_id].append(target_id)
            for evidence_id in relationship["evidence_ids"]:
                self.assertIn(evidence_id, evidence)
            claim = claims[relationship["claim_id"]]
            self.assertEqual(claim["subject_kind"], "relationship")
            self.assertEqual(claim["subject_id"], relationship["relationship_id"])
        crates = [
            entity for entity in entities.values() if entity["kind"] == "rust.crate"
        ]
        self.assertEqual(len(crates), 1)
        declarations = ENTITY_KINDS - {
            "rust.crate",
            "rust.symbol_reference",
            "source.file",
        }
        for entity_id, entity in entities.items():
            if entity["kind"] == "source.file":
                self.assertEqual(incoming[(entity_id, "CONTAINS")], 1)
            if entity["kind"] in declarations:
                self.assertEqual(incoming[(entity_id, "DEFINES")], 1)
            if entity["kind"] == "rust.symbol_reference":
                self.assertEqual(outgoing[entity_id], 0)
                self.assertGreaterEqual(
                    incoming[(entity_id, "IMPORTS")]
                    + incoming[(entity_id, "IMPLEMENTS")],
                    1,
                )
            for evidence_id in entity["evidence_ids"]:
                self.assertIn(evidence_id, evidence)
            claim = claims[entity["claim_id"]]
            self.assertEqual(claim["subject_kind"], "entity")
            self.assertEqual(claim["subject_id"], entity_id)
        reachable = {crates[0]["entity_id"]}
        frontier = [crates[0]["entity_id"]]
        while frontier:
            parent = frontier.pop()
            for child in defines_children[parent]:
                self.assertNotIn(child, reachable)
                reachable.add(child)
                frontier.append(child)
        declaration_ids = {
            entity_id
            for entity_id, entity in entities.items()
            if entity["kind"] in declarations
        }
        self.assertTrue(declaration_ids.issubset(reachable))

    def test_claim_state_rule_and_model_boundaries_are_closed(self) -> None:
        graph = load_json(GRAPH_PATH)
        ontology = load_json(ONTOLOGY_PATH)
        claims = {claim["claim_id"]: claim for claim in graph["claims"]}
        states = Counter(claim["state"] for claim in claims.values())
        self.assertEqual(states, {"deterministic_fact": 25, "derived_fact": 1})
        derived = [
            claim for claim in claims.values() if claim["state"] == "derived_fact"
        ]
        self.assertEqual(len(derived), 1)
        derivation = derived[0]["derivation"]
        self.assertEqual(
            derivation["rule_version"],
            "codenoesis.rule.rust-file-containment/s2-v1",
        )
        self.assertEqual(len(derivation["input_claim_ids"]), 2)
        for claim_id in derivation["input_claim_ids"]:
            self.assertIn(claim_id, claims)
            self.assertEqual(claims[claim_id]["state"], "deterministic_fact")
        for claim in claims.values():
            if claim["state"] == "deterministic_fact":
                self.assertEqual(claim["derivation"]["kind"], "parser")
                self.assertEqual(
                    claim["derivation"]["extractor_version"],
                    "codenoesis.rust-tree-sitter/s2-v1",
                )
            else:
                self.assertEqual(
                    claim["derivation"]["kind"], "deterministic_rule"
                )
        transitions = {
            tuple(transition) for transition in ontology["claim_transitions"]
        }
        self.assertEqual(transitions, CLAIM_TRANSITIONS)
        for state in CLAIM_STATES:
            self.assertNotIn((state, state), transitions)
        self.assertFalse(any(source == "superseded" for source, _ in transitions))
        self.assertEqual(ontology["claim_origins"]["model"], ["candidate"])

    def test_evidence_malformed_unicode_and_coverage_goldens_are_exact(
        self,
    ) -> None:
        graph = load_json(GRAPH_PATH)
        manifest = load_json(FIXTURE_ROOT / "manifest.json")
        source = (SOURCE_ROOT / "src/lib.rs").read_bytes()
        expected_spans = {
            "evidence-s2-0000": (0, 630, source),
            "evidence-s2-0001": (
                0,
                34,
                b"use crate::catalog::{Item, Store};",
            ),
            "evidence-s2-0002": (36, 58, b"pub type ItemId = u64;"),
            "evidence-s2-0003": (60, 83, b"pub trait Describable {"),
            "evidence-s2-0004": (
                88,
                117,
                b"fn describe(&self) -> String;",
            ),
            "evidence-s2-0005": (121, 138, b"pub mod catalog {"),
            "evidence-s2-0006": (
                143,
                176,
                b"use super::{Describable, ItemId};",
            ),
            "evidence-s2-0007": (182, 199, b"pub struct Item {"),
            "evidence-s2-0008": (257, 273, b"pub enum Store {"),
            "evidence-s2-0009": (
                315,
                342,
                b"impl Describable for Item {",
            ),
            "evidence-s2-0010": (
                351,
                381,
                b"fn describe(&self) -> String {",
            ),
            "evidence-s2-0011": (
                433,
                471,
                b"pub fn make_item(id: ItemId) -> Item {",
            ),
            "evidence-s2-0012": (
                555,
                607,
                "pub fn café_label(item: &catalog::Item) -> String {".encode(),
            ),
        }
        evidence_order = []
        for evidence in graph["evidence"]:
            evidence_id = evidence["evidence_id"]
            start, end, expected = expected_spans[evidence_id]
            self.assertEqual(evidence["span"], {"unit": "byte", "start": start, "end": end})
            self.assertEqual(source[start:end], expected)
            self.assertEqual(evidence["path"], "src/lib.rs")
            self.assertEqual(
                evidence["blob_oid"], manifest["revision"]["files"][1]["blob_oid"]
            )
            evidence_order.append(
                (evidence["path"].encode(), start, end, evidence_id)
            )
        self.assertEqual(evidence_order, sorted(evidence_order))
        gaps = graph["coverage"]["gaps"]
        gap_order = [
            (
                gap["path"].encode(),
                gap["span"]["start"],
                gap["span"]["end"],
                gap["code"],
            )
            for gap in gaps
        ]
        self.assertEqual(gap_order, sorted(gap_order))
        self.assertEqual(
            [gap["code"] for gap in gaps],
            [
                "calls_not_extracted",
                "fields_not_extracted",
                "variants_not_extracted",
            ],
        )
        malformed = manifest["generated_variants"]["malformed_token"]
        self.assertEqual(
            malformed["expected_diagnostic"]["span"],
            malformed["expected_gap"]["span"],
        )
        collision_error = load_json(
            FIXTURE_ROOT / "expected-error-nfc-collision.json"
        )
        collision = manifest["generated_variants"]["nfc_collision"]
        self.assertEqual(
            collision_error["context"]["canonical_identity"],
            collision["normalized_identity"],
        )
        self.assertEqual(
            collision_error["context"]["first_span"],
            {"unit": "byte", "start": 555, "end": 607},
        )
        self.assertEqual(
            collision_error["context"]["second_span"],
            {"unit": "byte", "start": 631, "end": 655},
        )
        self.assertNotIn(str(ROOT), canonical_json(collision_error).decode())


if __name__ == "__main__":
    unittest.main()
