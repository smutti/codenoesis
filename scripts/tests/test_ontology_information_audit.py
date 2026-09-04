from __future__ import annotations

import copy
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from audit_ontology_information import (  # noqa: E402
    CORE_INFORMATION_CAPABILITIES,
    OntologyAuditError,
    audit_ontology_information,
)


def entity(identifier: str, kind: str, **fields: object) -> dict[str, object]:
    return {"id": identifier, "kind": kind, **fields}


def relationship(
    identifier: str, kind: str, source: str, target: str
) -> dict[str, object]:
    return {
        "evidence_ids": ["evidence-1"],
        "id": identifier,
        "kind": kind,
        "source": source,
        "target": target,
    }


def complete_graph() -> dict[str, object]:
    return {
        "schema_version": "codenoesis.portable-graph/v9",
        "ontology_version": "codenoesis.ontology/rust/v15",
        "query_contract_version": "codenoesis.local-query-result/v13",
        "entities": [
            entity("function-1", "rust.function", name="run"),
            entity(
                "signature-1",
                "rust.callable_signature",
                subject_id="function-1",
                properties={
                    "return_state": "declared",
                    "return_type": "Result<u8>",
                },
                evidence_ids=["evidence-1"],
            ),
            entity(
                "parameter-1",
                "rust.parameter",
                subject_id="function-1",
                ordinal=0,
                properties={
                    "declared_type": "&str",
                    "pattern": "input",
                    "receiver_state": "none",
                },
                evidence_ids=["evidence-1"],
            ),
            entity("enum-1", "rust.enum", name="Mode"),
            entity(
                "variant-1",
                "rust.enum_variant",
                owner_id="enum-1",
                properties={"form": "unit", "discriminant_present": True},
            ),
            entity(
                "declared-1",
                "rust.declared_value",
                subject_id="variant-1",
                properties={"state": "normalized_scalar"},
                evidence_ids=["evidence-1"],
            ),
            entity(
                "evaluated-1",
                "rust.evaluated_value",
                declared_value_id="declared-1",
                properties={"canonical_value": "1", "rust_type": "u8"},
            ),
            entity(
                "call-1",
                "rust.call_site",
                subject_id="function-1",
                properties={
                    "resolution_state": "resolved_unique_local",
                    "resolved_target_id": "function-1",
                },
                evidence_ids=["evidence-1"],
            ),
            entity(
                "argument-1",
                "rust.call_argument",
                properties={
                    "call_expression_id": "call-1",
                    "expression_id": "expression-1",
                    "ordinal": 0,
                },
                evidence_ids=["evidence-1"],
            ),
            entity("expression-1", "rust.expression", evidence_ids=["evidence-1"]),
            entity("block-1", "rust.syntax_basic_block", evidence_ids=["evidence-1"]),
        ],
        "relationships": [
            relationship("has-signature", "HAS_SIGNATURE", "function-1", "signature-1"),
            relationship("has-parameter", "HAS_PARAMETER", "signature-1", "parameter-1"),
            relationship("defines-variant", "DEFINES", "enum-1", "variant-1"),
            relationship("declares-value", "DECLARES_VALUE", "variant-1", "declared-1"),
            relationship("evaluates-to", "EVALUATES_TO", "declared-1", "evaluated-1"),
            relationship("has-argument", "HAS_ARGUMENT", "call-1", "argument-1"),
            relationship("argument-value", "ARGUMENT_VALUE", "argument-1", "expression-1"),
            relationship("has-expression", "HAS_EXPRESSION", "call-1", "expression-1"),
            relationship("calls", "CALLS", "function-1", "function-1"),
        ],
        "evidence": [
            {
                "blob_oid": "a" * 40,
                "end_byte": 24,
                "id": "evidence-1",
                "path": "src/lib.rs",
                "start_byte": 4,
            }
        ],
        "claims": [
            {
                "evidence_ids": ["evidence-1"],
                "id": "claim-1",
                "state": "deterministic_fact",
                "subject_id": "function-1",
                "subject_kind": "entity",
            }
        ],
        "coverage_gaps": [
            {
                "capability": capability,
                "evidence_ids": ["evidence-1"],
                "id": f"gap-{index}",
                "state": "not_analyzed",
                "subject_id": "signature-1",
            }
            for index, capability in enumerate(
                (
                    "rust.type_resolution_not_performed",
                    "rust.call_target_resolution",
                    "rust.data_flow_not_computed",
                    "rust.side_effects_not_computed",
                    "rust.runtime_behavior_not_observed",
                )
            )
        ],
        "local_flow_index": {
            "block_entity_ids": ["block-1"],
            "completed_callable_ids": ["function-1"],
        },
        "constant_evaluation_index": {
            "evaluated_entity_ids": ["evaluated-1"],
            "evaluation_relationship_ids": ["evaluates-to"],
        },
    }


class OntologyInformationAuditTests(unittest.TestCase):
    def test_complete_reasoning_core_passes_with_honest_limitations(self) -> None:
        report = audit_ontology_information(complete_graph())

        self.assertEqual(
            report["schema_version"],
            "codenoesis.ontology-information-audit/v1",
        )
        self.assertEqual(report["verdict"], "usable_with_explicit_gaps")
        self.assertEqual(
            [check["capability"] for check in report["checks"]],
            list(CORE_INFORMATION_CAPABILITIES),
        )
        self.assertTrue(all(check["status"] == "pass" for check in report["checks"]))
        self.assertEqual(
            report["reasoning_readiness"],
            {
                "callable_and_data_structure": "available",
                "cross_callable_resolution": "partial",
                "intra_callable_flow": "partial",
                "runtime_behavior": "not_available",
                "side_effects": "not_available",
                "type_resolution": "not_available",
            },
        )
        self.assertEqual(
            report["adjacent_information"],
            {
                "implementation_aware_api": "separate_s7_report",
                "semantic_version_diff": "separate_s7_report",
                "source_text": "separate_r18_query",
            },
        )
        self.assertEqual(
            report["coverage_gap_counts"][0],
            {"capability": "rust.call_target_resolution", "count": 1},
        )

    def test_missing_callable_signature_fails_the_information_contract(self) -> None:
        graph = complete_graph()
        graph["relationships"] = [
            value
            for value in graph["relationships"]
            if value["kind"] != "HAS_SIGNATURE"
        ]

        report = audit_ontology_information(graph)

        callable_check = next(
            check
            for check in report["checks"]
            if check["capability"] == "callable_io_contracts"
        )
        self.assertEqual(callable_check["status"], "fail")
        self.assertEqual(
            callable_check["failures"],
            ["function-1:missing_signature", "signature-1:invalid_signature_link"],
        )
        self.assertEqual(report["verdict"], "insufficient_information")

    def test_unsafe_evidence_path_fails_navigation(self) -> None:
        graph = copy.deepcopy(complete_graph())
        graph["evidence"][0]["path"] = "../private.rs"

        report = audit_ontology_information(graph)

        evidence_check = next(
            check
            for check in report["checks"]
            if check["capability"] == "source_evidence_navigation"
        )
        self.assertEqual(evidence_check["status"], "fail")
        self.assertEqual(evidence_check["failures"], ["evidence-1:unsafe_path"])

    def test_dangling_claim_subject_fails_navigation(self) -> None:
        graph = copy.deepcopy(complete_graph())
        graph["claims"][0]["subject_id"] = "missing"

        report = audit_ontology_information(graph)

        evidence_check = next(
            check
            for check in report["checks"]
            if check["capability"] == "source_evidence_navigation"
        )
        self.assertEqual(
            evidence_check["failures"],
            ["claim-1:dangling_claim_subject"],
        )

    def test_audit_is_independent_of_portable_family_order(self) -> None:
        graph = complete_graph()
        permuted = copy.deepcopy(graph)
        for family in ("entities", "relationships", "claims", "evidence", "coverage_gaps"):
            permuted[family].reverse()

        self.assertEqual(
            audit_ontology_information(permuted),
            audit_ontology_information(graph),
        )

    def test_wrong_portable_contract_is_rejected(self) -> None:
        graph = complete_graph()
        graph["schema_version"] = "codenoesis.portable-graph/v8"

        with self.assertRaisesRegex(OntologyAuditError, "unsupported"):
            audit_ontology_information(graph)


if __name__ == "__main__":
    unittest.main()
