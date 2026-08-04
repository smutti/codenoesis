# Decision 0014: S4 R4 legacy Cargo badges contract

| Field | Value |
|---|---|
| Status | Proposed; effective only on protected manual merge of the pull request linked from issue #107 by `@smutti` |
| Date | 2026-08-04 |
| Deciders | Andrea Moretti governance persona, represented by accountable actor `@smutti` |
| Technical approver | `@smutti` |
| Issue | [#107](https://github.com/smutti/codenoesis/issues/107) |
| Authorization | [Maintainer comment](https://github.com/smutti/codenoesis/issues/107#issuecomment-5177741408) |
| Retained Red | [Evidence comment](https://github.com/smutti/codenoesis/issues/107#issuecomment-5177766905) |
| Product issue | [#104](https://github.com/smutti/codenoesis/issues/104) |
| Required base | `557547285f9532772efceea900ba982b6a8e65a9` |
| Slice | `S4 — Evidence-backed workspace docs compatibility extension` |
| Requirement | `FR-EXT-009` |
| Risk | High |

## Context

Decision 0012 requires the non-vendored RustDesk revision
`d412d198720aa56f6cfed2dfad262e8fb1322fb7` to complete R1, R2, R3, and R4
without opening its external gitlink. The exact pinned tree contains a legacy
top-level `[badges]` table in `libs/enigo/Cargo.toml`.

The ratified Cargo subset simultaneously requires unknown keys to fail closed
unless a family has an exact typed unsupported mapping. It lists package and
workspace metadata, profiles, lints, replace tables, and advanced dependency
fields, but not `badges`.

The product pilot therefore exits `11` with strict ErrorV11
`extraction.invalid_cargo_manifest_fact`, reason `unsupported_key`, empty
stdout, and no store. Ignoring the table would violate fail-closed parsing.
Treating it as package metadata would misclassify the committed syntax.
Adding a RustDesk-specific branch would violate repository-generic behavior.

The contradiction was retained by the governance Red before any subset,
schema, ontology, oracle, bundle, or product change.

## Decision

The R4 typed-unsupported catalog adds exactly:

```text
badges -> cargo.legacy_badges_unsupported
```

The mapping applies only to a literal top-level `[badges]` table in an
otherwise selected and authorized Cargo manifest. It emits:

- the existing diagnostic code `cargo.unsupported_manifest_family`;
- the existing diagnostic message for an unsupported Cargo manifest family;
- one coverage gap with capability `cargo.legacy_badges_unsupported`;
- coverage state `unsupported`;
- exact committed evidence for the table header.

No new diagnostic code or message is introduced. The existing Cargo
diagnostic identity domain and coverage-gap identity domain remain unchanged.
The new capability string participates in the existing coverage preimage.

## Uninterpreted values

R4 recognizes only the family boundary. It does not interpret, normalize, or
retain badge provider names, repository values, URLs, branches, service
configuration, or any nested value.

Badge values never enter an entity, relationship, claim, identifier, error,
log, document, query result, telemetry record, or model prompt. R4 emits no
entity or relationship for the table and makes no statement about whether a
badge service exists, is configured, is reachable, or reports a result.

The implementation must not walk badge values to derive semantics. The
committed header evidence is sufficient to support the typed unsupported
observation.

## Closed syntax

Only the single top-level table path `badges` receives this mapping.

- nested `[badges.<name>]` forms remain unsupported keys;
- duplicate table declarations retain deterministic TOML failure;
- malformed, Unicode-colliding, or non-table forms retain closed failures;
- all other unknown manifest keys remain subject to the unchanged unknown-key
  fail-closed rule.

This amendment does not make `badges` a supported Cargo declaration family.
It makes the unsupported boundary explicit and evidence-backed.

## Public contracts

The amendment changes only the reviewed R4 contract set:

- `cargo-manifest-subset-v1` gains the exact mapping;
- Rust/Cargo ontology v4 gains the coverage capability and state;
- ExtractionChunkV4 and KnowledgeGraphV4 accept that capability through the
  existing coverage shape;
- the R4 machine oracle records the generic RustDesk observation.

RepositorySnapshotV7, ConfigurationV4, ErrorV11, LocalQueryResultV2,
DocumentationManifestV1, identity domains, cardinality limits, artifact roles,
storage semantics, and all R0-R3/V1-V6 bytes remain unchanged.

## Security and authority

The selected path retains the complete R4 authority boundary:

- no Cargo, rustc, Git, hook, filter, build script, procedural macro, target,
  badge client, shell, or model process;
- no network, DNS, registry, cache, badge service, dependency, or credential
  access;
- no path traversal, nested gitlink read, environment expansion, repository
  write, retry, or fallback;
- no raw badge value in derived output.

The RustDesk gitlink remains the sole external boundary at
`libs/hbb_common@69cea8dafee147848ae88702029f4bf7df7224c3`; this decision grants
no authority over its nested source.

## Acceptance

The governance guard is:

```text
python3 -m unittest scripts.tests.test_s4_cargo_manifest_facts_contract.S4CargoManifestFactsGovernanceTests.test_legacy_badges_has_exact_typed_unsupported_mapping
```

The retained test-only head
`89f0455777cdf6fd344af2f8549e50462df98a1a` fails only with:

```text
R4 legacy badges family lacks an exact typed unsupported mapping
```

Green requires the mapping, ontology capability, schema enum, exact RustDesk
oracle record, unchanged diagnostic-code set, immutable inherited contracts,
and a valid complete R4 bundle.

After protected merge, product issue #104 may implement the generic table
boundary and must rerun the exact pinned RustDesk scan, docs, and seven-kind
query journey. Pilot counts and timings remain observations, not golden
requirements.

## Compatibility and rollback

This is an additive pre-release R4 governance correction. Selector-absent
behavior and every R0-R3 output remain byte-identical. The correction adds no
dependency, feature, migration, DDL, marker, artifact role, release action, or
destructive operation.

Rollback is one atomic revert of this governance package before product R4
merge. Product issue #104 remains a separate pull request and preserves its
retained Red history.

## Governance boundary

The SRS is excluded from the R4 bundle to avoid self-reference, but it binds
the recomputed bundle digest. Product implementation is forbidden in this
governance pull request. A separate product correction may begin only after
the protected merge SHA and new bundle digest are recorded on issue #104.

The authoring agent does not approve or merge its own change.

## Consequences

- The mandatory RustDesk pilot and the unknown-key fail-closed policy become
  simultaneously implementable.
- Users receive explicit uncertainty instead of silent omission or a false
  package-metadata interpretation.
- The ontology gains one narrowly scoped legacy capability.
- Any further unlisted Cargo family still requires a separate observed and
  reviewed governance decision.
