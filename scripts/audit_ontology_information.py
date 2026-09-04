#!/usr/bin/env python3
"""Audit whether a portable ontology exposes a usable, honest reasoning core."""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any, Iterable, Sequence


AUDIT_SCHEMA = "codenoesis.ontology-information-audit/v1"
AUDITOR_VERSION = "codenoesis.ontology-information-auditor/v1"
CORE_INFORMATION_CAPABILITIES = (
    "callable_io_contracts",
    "enum_and_declared_values",
    "safe_constant_values",
    "call_arguments_and_targets",
    "expression_structure",
    "local_flow",
    "source_evidence_navigation",
    "explicit_uncertainty",
)


class OntologyAuditError(ValueError):
    """The input is not a structurally auditable portable ontology."""


def canonical_json_bytes(value: Any) -> bytes:
    return (
        json.dumps(
            value,
            ensure_ascii=False,
            allow_nan=False,
            separators=(",", ":"),
            sort_keys=True,
        ).encode("utf-8")
        + b"\n"
    )


def records(graph: dict[str, Any], family: str) -> list[dict[str, Any]]:
    values = graph.get(family)
    if not isinstance(values, list) or any(not isinstance(value, dict) for value in values):
        raise OntologyAuditError(f"{family} must be a list of objects")
    return values


def unique_index(values: Iterable[dict[str, Any]], family: str) -> dict[str, dict[str, Any]]:
    result: dict[str, dict[str, Any]] = {}
    for value in values:
        identifier = value.get("id")
        if not isinstance(identifier, str) or not identifier or identifier in result:
            raise OntologyAuditError(f"{family} contains an invalid or duplicate id")
        result[identifier] = value
    return result


def relation_pairs(
    relationships: Iterable[dict[str, Any]], kind: str
) -> set[tuple[str, str]]:
    return {
        (relationship["source"], relationship["target"])
        for relationship in relationships
        if relationship.get("kind") == kind
        and isinstance(relationship.get("source"), str)
        and isinstance(relationship.get("target"), str)
    }


def safe_relative_path(value: object) -> bool:
    if not isinstance(value, str) or not value or "\\" in value or "\x00" in value:
        return False
    return not value.startswith("/") and all(
        part not in {"", ".", ".."} for part in value.split("/")
    )


def check_result(
    capability: str, failures: Iterable[str], observed: dict[str, int]
) -> dict[str, Any]:
    unique_failures = sorted(set(failures))
    return {
        "capability": capability,
        "failures": unique_failures,
        "observed": observed,
        "status": "fail" if unique_failures else "pass",
    }


def check_callable_io(
    by_kind: dict[str, list[dict[str, Any]]],
    relationships: list[dict[str, Any]],
) -> dict[str, Any]:
    failures: list[str] = []
    callables = by_kind["rust.function"] + by_kind["rust.method"]
    signatures = by_kind["rust.callable_signature"]
    parameters = by_kind["rust.parameter"]
    has_signature = relation_pairs(relationships, "HAS_SIGNATURE")
    has_parameter = relation_pairs(relationships, "HAS_PARAMETER")
    alternatives = defaultdict(set)
    for owner, alternative in relation_pairs(relationships, "HAS_DECLARATION_ALTERNATIVE"):
        alternatives[owner].add(alternative)

    if not callables or not signatures:
        failures.append("callable_inventory:empty")
    for callable_entity in callables:
        identifier = callable_entity["id"]
        direct = any(source == identifier for source, _ in has_signature)
        indirect = alternatives[identifier] and all(
            any(source == alternative for source, _ in has_signature)
            for alternative in alternatives[identifier]
        )
        if not direct and not indirect:
            failures.append(f"{identifier}:missing_signature")

    for signature in signatures:
        identifier = signature["id"]
        subject = signature.get("subject_id")
        if not isinstance(subject, str) or (subject, identifier) not in has_signature:
            failures.append(f"{identifier}:invalid_signature_link")
        properties = signature.get("properties")
        if not isinstance(properties, dict):
            failures.append(f"{identifier}:missing_signature_properties")
            continue
        return_state = properties.get("return_state")
        return_type = properties.get("return_type")
        if return_state == "declared":
            if not isinstance(return_type, str) or not return_type:
                failures.append(f"{identifier}:missing_return_type")
        elif return_state == "unit_default":
            if return_type is not None:
                failures.append(f"{identifier}:unexpected_return_type")
        else:
            failures.append(f"{identifier}:invalid_return_state")

    for parameter in parameters:
        identifier = parameter["id"]
        properties = parameter.get("properties")
        sources = [source for source, target in has_parameter if target == identifier]
        if len(sources) != 1:
            failures.append(f"{identifier}:invalid_parameter_link")
        if (
            not isinstance(parameter.get("ordinal"), int)
            or parameter["ordinal"] < 0
            or not isinstance(properties, dict)
            or not isinstance(properties.get("pattern"), str)
            or "declared_type" not in properties
        ):
            failures.append(f"{identifier}:incomplete_parameter")
    return check_result(
        "callable_io_contracts",
        failures,
        {
            "callables": len(callables),
            "parameters": len(parameters),
            "signatures": len(signatures),
        },
    )


def check_declared_values(
    entities: dict[str, dict[str, Any]],
    by_kind: dict[str, list[dict[str, Any]]],
    relationships: list[dict[str, Any]],
) -> dict[str, Any]:
    failures: list[str] = []
    enums = by_kind["rust.enum"]
    variants = by_kind["rust.enum_variant"]
    declared = by_kind["rust.declared_value"]
    defines = relation_pairs(relationships, "DEFINES")
    declares = relation_pairs(relationships, "DECLARES_VALUE")
    value_subjects = (
        variants + by_kind["rust.constant"] + by_kind["rust.static"]
    )
    if not enums or not variants or not declared:
        failures.append("declared_value_inventory:empty")
    for variant in variants:
        identifier = variant["id"]
        owner = variant.get("owner_id")
        properties = variant.get("properties")
        if (
            not isinstance(owner, str)
            or entities.get(owner, {}).get("kind") != "rust.enum"
            or (owner, identifier) not in defines
        ):
            failures.append(f"{identifier}:invalid_enum_owner")
        if not isinstance(properties, dict) or properties.get("form") not in {
            "unit",
            "tuple",
            "struct",
        }:
            failures.append(f"{identifier}:missing_variant_shape")
    for subject in value_subjects:
        identifier = subject["id"]
        targets = [target for source, target in declares if source == identifier]
        if len(targets) != 1 or entities.get(targets[0], {}).get("kind") != "rust.declared_value":
            failures.append(f"{identifier}:missing_declared_value")
        elif entities[targets[0]].get("subject_id") != identifier:
            failures.append(f"{identifier}:declared_value_subject_mismatch")
    return check_result(
        "enum_and_declared_values",
        failures,
        {
            "declared_values": len(declared),
            "enums": len(enums),
            "variants": len(variants),
        },
    )


def check_safe_constants(
    entities: dict[str, dict[str, Any]],
    by_kind: dict[str, list[dict[str, Any]]],
    relationships: list[dict[str, Any]],
    graph: dict[str, Any],
) -> dict[str, Any]:
    failures: list[str] = []
    evaluated = by_kind["rust.evaluated_value"]
    evaluates = relation_pairs(relationships, "EVALUATES_TO")
    index = graph.get("constant_evaluation_index")
    if not evaluated:
        failures.append("safe_constant_inventory:empty")
    if not isinstance(index, dict):
        failures.append("constant_evaluation_index:missing")
        indexed_entities: set[str] = set()
        indexed_relationships: set[str] = set()
    else:
        indexed_entities = set(index.get("evaluated_entity_ids", []))
        indexed_relationships = set(index.get("evaluation_relationship_ids", []))
    evaluation_relationships = {
        relationship["id"]: relationship
        for relationship in relationships
        if relationship.get("kind") == "EVALUATES_TO"
    }
    for value in evaluated:
        identifier = value["id"]
        declared = value.get("declared_value_id")
        if (
            not isinstance(declared, str)
            or entities.get(declared, {}).get("kind") != "rust.declared_value"
            or (declared, identifier) not in evaluates
        ):
            failures.append(f"{identifier}:invalid_evaluation_link")
        if identifier not in indexed_entities:
            failures.append(f"{identifier}:missing_evaluation_index")
    for identifier in indexed_relationships:
        if identifier not in evaluation_relationships:
            failures.append(f"{identifier}:missing_evaluation_relationship")
    if set(evaluation_relationships) != indexed_relationships:
        failures.append("constant_evaluation_index:relationship_mismatch")
    return check_result(
        "safe_constant_values",
        failures,
        {
            "evaluated_values": len(evaluated),
            "evaluation_relationships": len(evaluation_relationships),
        },
    )


def check_calls(
    entities: dict[str, dict[str, Any]],
    by_kind: dict[str, list[dict[str, Any]]],
    relationships: list[dict[str, Any]],
    coverage: list[dict[str, Any]],
) -> dict[str, Any]:
    failures: list[str] = []
    call_sites = by_kind["rust.call_site"]
    arguments = by_kind["rust.call_argument"]
    has_argument = relation_pairs(relationships, "HAS_ARGUMENT")
    argument_value = relation_pairs(relationships, "ARGUMENT_VALUE")
    calls = relation_pairs(relationships, "CALLS")
    unresolved = {
        gap.get("subject_id")
        for gap in coverage
        if gap.get("capability") == "rust.call_target_resolution"
    }
    if not call_sites or not arguments:
        failures.append("call_inventory:empty")
    for argument in arguments:
        identifier = argument["id"]
        properties = argument.get("properties")
        subject = properties.get("call_expression_id") if isinstance(properties, dict) else None
        if not isinstance(subject, str) or (subject, identifier) not in has_argument:
            failures.append(f"{identifier}:invalid_argument_owner")
        values = [target for source, target in argument_value if source == identifier]
        if (
            len(values) != 1
            or entities.get(values[0], {}).get("kind") != "rust.expression"
            or not isinstance(properties, dict)
            or properties.get("expression_id") != values[0]
            or not isinstance(properties.get("ordinal"), int)
        ):
            failures.append(f"{identifier}:missing_argument_value")
    for call_site in call_sites:
        identifier = call_site["id"]
        properties = call_site.get("properties")
        if not isinstance(properties, dict):
            failures.append(f"{identifier}:missing_call_properties")
            continue
        state = properties.get("resolution_state")
        if state == "resolved_unique_local":
            target = properties.get("resolved_target_id")
            subject = call_site.get("subject_id")
            if (
                not isinstance(target, str)
                or target not in entities
                or not isinstance(subject, str)
                or (subject, target) not in calls
            ):
                failures.append(f"{identifier}:missing_resolved_call")
        elif state == "candidate_unresolved":
            if identifier not in unresolved:
                failures.append(f"{identifier}:missing_resolution_gap")
        else:
            failures.append(f"{identifier}:invalid_resolution_state")
    return check_result(
        "call_arguments_and_targets",
        failures,
        {
            "arguments": len(arguments),
            "call_sites": len(call_sites),
            "resolved_call_relationships": len(calls),
        },
    )


def check_expressions(
    by_kind: dict[str, list[dict[str, Any]]],
    relationships: list[dict[str, Any]],
) -> dict[str, Any]:
    expressions = by_kind["rust.expression"]
    linked = {target for _, target in relation_pairs(relationships, "HAS_EXPRESSION")}
    failures = [
        f"{expression['id']}:missing_expression_owner"
        for expression in expressions
        if expression["id"] not in linked
    ]
    if not expressions:
        failures.append("expression_inventory:empty")
    return check_result(
        "expression_structure",
        failures,
        {"expressions": len(expressions), "owned_expressions": len(linked)},
    )


def check_local_flow(
    entities: dict[str, dict[str, Any]], graph: dict[str, Any]
) -> dict[str, Any]:
    failures: list[str] = []
    index = graph.get("local_flow_index")
    if not isinstance(index, dict):
        failures.append("local_flow_index:missing")
        block_ids: list[object] = []
        callable_ids: list[object] = []
    else:
        block_ids = index.get("block_entity_ids", [])
        callable_ids = index.get("completed_callable_ids", [])
    if not isinstance(block_ids, list) or not block_ids:
        failures.append("local_flow_index:missing_blocks")
        block_ids = []
    if not isinstance(callable_ids, list) or not callable_ids:
        failures.append("local_flow_index:missing_callables")
        callable_ids = []
    for identifier in block_ids:
        if not isinstance(identifier, str) or entities.get(identifier, {}).get("kind") != "rust.syntax_basic_block":
            failures.append(f"{identifier}:invalid_flow_block")
    for identifier in callable_ids:
        if not isinstance(identifier, str) or entities.get(identifier, {}).get("kind") not in {
            "rust.function",
            "rust.method",
            "rust.declaration_alternative",
        }:
            failures.append(f"{identifier}:invalid_flow_callable")
    return check_result(
        "local_flow",
        failures,
        {"completed_callables": len(callable_ids), "syntax_blocks": len(block_ids)},
    )


def check_evidence_navigation(
    evidence: list[dict[str, Any]],
    evidence_ids: set[str],
    families: Iterable[list[dict[str, Any]]],
    claims: list[dict[str, Any]],
    entity_ids: set[str],
    relationship_ids: set[str],
) -> dict[str, Any]:
    failures: list[str] = []
    if not evidence:
        failures.append("evidence_inventory:empty")
    for item in evidence:
        identifier = item["id"]
        if not safe_relative_path(item.get("path")):
            failures.append(f"{identifier}:unsafe_path")
        start = item.get("start_byte")
        end = item.get("end_byte")
        if (
            not isinstance(start, int)
            or isinstance(start, bool)
            or not isinstance(end, int)
            or isinstance(end, bool)
            or start < 0
            or end < start
        ):
            failures.append(f"{identifier}:invalid_span")
    referenced = 0
    for family in families:
        for record in family:
            references = record.get("evidence_ids", [])
            if not isinstance(references, list):
                failures.append(f"{record.get('id', 'unknown')}:invalid_evidence_links")
                continue
            referenced += len(references)
            for identifier in references:
                if identifier not in evidence_ids:
                    failures.append(f"{record.get('id', 'unknown')}:{identifier}:missing_evidence")
    for claim in claims:
        identifier = claim["id"]
        subject = claim.get("subject_id")
        subject_kind = claim.get("subject_kind")
        valid_subject = (
            subject_kind == "entity" and subject in entity_ids
        ) or (
            subject_kind == "relationship" and subject in relationship_ids
        )
        if not valid_subject:
            failures.append(f"{identifier}:dangling_claim_subject")
    return check_result(
        "source_evidence_navigation",
        failures,
        {
            "claims": len(claims),
            "evidence": len(evidence),
            "evidence_references": referenced,
        },
    )


def check_uncertainty(
    coverage: list[dict[str, Any]], evidence_ids: set[str]
) -> dict[str, Any]:
    failures: list[str] = []
    if not coverage:
        failures.append("coverage_gap_inventory:empty")
    for gap in coverage:
        identifier = gap["id"]
        if not isinstance(gap.get("capability"), str) or not isinstance(gap.get("state"), str):
            failures.append(f"{identifier}:incomplete_gap")
        references = gap.get("evidence_ids")
        if not isinstance(references, list) or not references:
            failures.append(f"{identifier}:missing_gap_evidence")
        elif any(reference not in evidence_ids for reference in references):
            failures.append(f"{identifier}:invalid_gap_evidence")
    return check_result(
        "explicit_uncertainty",
        failures,
        {"coverage_gaps": len(coverage)},
    )


def readiness_status(gap_counts: Counter[str], capabilities: Sequence[str]) -> str:
    return "partial" if any(gap_counts[capability] for capability in capabilities) else "available"


def audit_ontology_information(graph: dict[str, Any]) -> dict[str, Any]:
    if not isinstance(graph, dict):
        raise OntologyAuditError("portable graph must be an object")
    if (
        graph.get("schema_version") != "codenoesis.portable-graph/v9"
        or graph.get("ontology_version") != "codenoesis.ontology/rust/v15"
        or graph.get("query_contract_version") != "codenoesis.local-query-result/v13"
    ):
        raise OntologyAuditError("unsupported ontology information contract")
    entities_list = records(graph, "entities")
    relationships = records(graph, "relationships")
    evidence = records(graph, "evidence")
    claims = records(graph, "claims")
    coverage = records(graph, "coverage_gaps")
    entities = unique_index(entities_list, "entities")
    relationship_index = unique_index(relationships, "relationships")
    evidence_index = unique_index(evidence, "evidence")
    unique_index(claims, "claims")
    unique_index(coverage, "coverage_gaps")
    by_kind: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for value in entities_list:
        kind = value.get("kind")
        if not isinstance(kind, str):
            raise OntologyAuditError("entity kind must be a string")
        by_kind[kind].append(value)

    checks = [
        check_callable_io(by_kind, relationships),
        check_declared_values(entities, by_kind, relationships),
        check_safe_constants(entities, by_kind, relationships, graph),
        check_calls(entities, by_kind, relationships, coverage),
        check_expressions(by_kind, relationships),
        check_local_flow(entities, graph),
        check_evidence_navigation(
            evidence,
            set(evidence_index),
            (entities_list, relationships, claims, coverage),
            claims,
            set(entities),
            set(relationship_index),
        ),
        check_uncertainty(coverage, set(evidence_index)),
    ]
    gap_counts = Counter(
        gap["capability"]
        for gap in coverage
        if isinstance(gap.get("capability"), str)
    )
    all_pass = all(check["status"] == "pass" for check in checks)
    structure_available = "available" if all_pass else "not_available"
    readiness = {
        "callable_and_data_structure": structure_available,
        "cross_callable_resolution": readiness_status(
            gap_counts, ("rust.call_target_resolution",)
        ),
        "intra_callable_flow": readiness_status(
            gap_counts,
            (
                "rust.data_flow_not_computed",
                "rust.lexical_reaching_definitions_not_analyzed",
                "rust.ownership_flow_not_computed",
                "rust.reachability_not_computed",
                "rust.syntax_normal_flow_not_analyzed",
            ),
        ),
        "runtime_behavior": (
            "not_available"
            if gap_counts["rust.runtime_behavior_not_observed"]
            else "available"
        ),
        "side_effects": (
            "not_available" if gap_counts["rust.side_effects_not_computed"] else "available"
        ),
        "type_resolution": (
            "not_available"
            if gap_counts["rust.type_resolution_not_performed"]
            else "available"
        ),
    }
    if not all_pass:
        verdict = "insufficient_information"
    elif gap_counts:
        verdict = "usable_with_explicit_gaps"
    else:
        verdict = "complete_for_declared_profile"
    return {
        "adjacent_information": {
            "implementation_aware_api": "separate_s7_report",
            "semantic_version_diff": "separate_s7_report",
            "source_text": "separate_r18_query",
        },
        "auditor_version": AUDITOR_VERSION,
        "checks": checks,
        "coverage_gap_counts": [
            {"capability": capability, "count": count}
            for capability, count in sorted(gap_counts.items())
        ],
        "reasoning_readiness": readiness,
        "schema_version": AUDIT_SCHEMA,
        "source": {
            "ontology_version": graph.get("ontology_version"),
            "portable_graph_schema": graph.get("schema_version"),
        },
        "verdict": verdict,
    }


def load_graph(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_bytes())
    except (OSError, json.JSONDecodeError) as error:
        raise OntologyAuditError("portable graph is not valid JSON") from error
    if not isinstance(value, dict):
        raise OntologyAuditError("portable graph must be an object")
    return value


def parser() -> argparse.ArgumentParser:
    command_parser = argparse.ArgumentParser(description=__doc__)
    command_parser.add_argument("--input", required=True)
    command_parser.add_argument("--output")
    return command_parser


def main(arguments: Sequence[str] | None = None) -> int:
    try:
        parsed = parser().parse_args(arguments)
        report = audit_ontology_information(load_graph(Path(parsed.input)))
        output = canonical_json_bytes(report)
        if parsed.output:
            Path(parsed.output).write_bytes(output)
        sys.stdout.buffer.write(output)
        sys.stdout.buffer.flush()
        return 0 if report["verdict"] != "insufficient_information" else 2
    except (OntologyAuditError, OSError):
        sys.stderr.buffer.write(
            canonical_json_bytes(
                {
                    "code": "ontology_audit.invalid_input",
                    "message": "portable ontology cannot be audited",
                    "schema_version": "codenoesis.ontology-information-audit-error/v1",
                }
            )
        )
        sys.stderr.buffer.flush()
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
