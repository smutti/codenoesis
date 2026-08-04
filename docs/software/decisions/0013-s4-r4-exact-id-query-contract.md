# Decision 0013: S4 R4 exact-ID query contract

| Field | Value |
|---|---|
| Status | Proposed; effective only on protected manual merge of the pull request linked from issue #105 by `@smutti` |
| Date | 2026-08-03 |
| Deciders | Andrea Moretti governance persona, represented by accountable actor `@smutti` |
| Technical approver | `@smutti` |
| Issue | [#105](https://github.com/smutti/codenoesis/issues/105) |
| Authorization | [Maintainer comment](https://github.com/smutti/codenoesis/issues/105#issuecomment-5170981186) |
| Retained Red | [Evidence comment](https://github.com/smutti/codenoesis/issues/105#issuecomment-5170981348) |
| Product issue | [#104](https://github.com/smutti/codenoesis/issues/104) |
| Required base | `94a8fe9b27b7e4fd9ae5c759cc23591c4fa12d00` |
| Slice | `S4 — Evidence-backed workspace docs compatibility extension` |
| Requirements | `FR-QRY-001`, `FR-EXT-009` |
| Risk | High |

## Context

Decision 0012 requires exact-ID query after restart for R4 Cargo entities,
relationships, claims, evidence, diagnostics, and coverage gaps. Its protected
schema set cannot express that outcome:

- `LocalQueryResultV1` accepts only entity, claim, evidence, and document IDs;
- its entity union contains only the inherited Rust V2 entity;
- it has no relationship, diagnostic, or coverage payload;
- Cargo relationships and coverage gaps have stable IDs, but the strict
  ExtractionChunkV4 diagnostic has no ID.

Changing V1 would break the explicit byte-compatibility requirement for every
V4 through V6 query. Emitting unversioned extra fields would create an
unreviewed public contract. Treating a diagnostic code as an ID would be
ambiguous whenever the same code occurs at multiple evidence spans.

The conflict was reproduced on issue #104 and retained by the independent
governance Red before any schema, ontology, fixture-plan, or product change.

## Decision

R4 adds strict `LocalQueryResultV2`. The local query command selects its
success contract from the already validated visible head:

| Stored head | Successful query contract |
|---|---|
| RepositorySnapshotV4 | LocalQueryResultV1 |
| RepositorySnapshotV5 | LocalQueryResultV1 |
| RepositorySnapshotV6 | LocalQueryResultV1 |
| RepositorySnapshotV7 | LocalQueryResultV2 |

There is no query-version CLI flag. Repository content, an ID prefix, document
content, environment state, or fallback parsing never selects the version.
Unknown and malformed IDs retain the existing ErrorV5 and exit semantics.

`LocalQueryResultV1`, DocumentationManifestV1, every V4-V6 query byte, and
all R0-R3 contracts remain immutable.

## LocalQueryResultV2

The schema version is exactly:

```text
codenoesis.local-query-result/v2
```

V2 retains the complete V1 envelope and adds three fields:

- `relationship`, containing one V4 graph relationship or `null`;
- `diagnostic`, containing one Cargo diagnostic or `null`;
- `coverage_gap`, containing one Cargo coverage gap or `null`.

Every field is required so a result has one canonical shape. The accepted
`requested_id` domains and `result_kind` values are exactly:

| ID domain | Result kind |
|---|---|
| `urn:codenoesis:entity:blake3:...` | `entity` |
| `urn:codenoesis:relationship:blake3:...` | `relationship` |
| `urn:codenoesis:claim:blake3:...` | `claim` |
| `urn:codenoesis:evidence:blake3:...` | `evidence` |
| `urn:codenoesis:diagnostic:blake3:...` | `diagnostic` |
| `urn:codenoesis:coverage-gap:blake3:...` | `coverage_gap` |
| `urn:codenoesis:document:blake3:...` | `document` |

No prefix alias, case folding, substring, fuzzy match, traversal, or empty
success is allowed.

## Projection and cardinality

The result is derived from a fully validated V7 snapshot and validated
DocumentationManifestV1:

- an entity result contains the exact entity, its unique claim, the claim
  evidence, and linked document statements;
- a relationship result contains the exact relationship, its unique claim,
  the claim evidence, and linked document statements;
- a claim result contains the exact claim, exactly one matching entity or
  relationship subject, the claim evidence, and linked document statements;
- an evidence result contains exactly one evidence record;
- a diagnostic result contains exactly one diagnostic and all of its one to
  64 referenced evidence records;
- a coverage-gap result contains exactly one gap and all of its one to 64
  referenced evidence records;
- a document result retains the V1 document record and ordered unlinked
  statements.

All unused singular payloads are `null`; all unused collection payloads are
empty. Claims remain at most one because the R4 ontology requires exactly one
claim per Cargo entity and relationship. Document statements use the existing
V1 statement contracts.

Missing, duplicate, contradictory, dangling, or kind-mismatched references
make the snapshot invalid. First-match resolution is forbidden. Output remains
bounded canonical JSON followed by exactly one LF.

## Diagnostic identity

ExtractionChunkV4 diagnostics now require:

```text
id = urn:codenoesis:diagnostic:blake3:<64 lowercase hex>
domain = codenoesis.diagnostic-id/cargo-manifest/v1
```

The preimage is the RFC 8785 canonical JSON string array:

```text
[
  "codenoesis.diagnostic-id/cargo-manifest/v1",
  repository_identity,
  diagnostic_code,
  evidence_id_1,
  ... evidence IDs sorted by byte value and deduplicated ...
]
```

The message is fixed by the closed diagnostic code and does not enter identity.
Commit, path, and span authority remain transitively bound through evidence
IDs. Locator plaintext, source bytes, message wording, scheduler order, and
storage sequence never enter the preimage.

Two diagnostics with the same code and same ordered evidence identity are
duplicates and invalidate the chunk. They are never collapsed by input order.

## Coverage-gap identity completion

Ontology v4 makes the already selected
`codenoesis.coverage-gap-id/v3` preimage explicit:

```text
[
  "codenoesis.coverage-gap-id/v3",
  repository_identity,
  commit_oid,
  capability,
  state,
  evidence_id_1,
  ... evidence IDs sorted by byte value and deduplicated ...
]
```

This clarification prevents a query implementation from inventing a different
identity recipe. It changes no inherited v2 coverage domain or preimage.

## Fixture oracle

The project-owned R4 fixture binds tree
`c99449f6f0651e4f6398521e316f3500d0e508e7` and commit
`7b1fc9073552b5967b1620d1e082a1d45e1b380e`.

The machine oracle fixes one reviewed exact ID for each result kind. Entity,
relationship, claim, evidence, diagnostic, and coverage examples all converge
on the explicit example-target declaration at bytes 714 through 739 of
`crates/app/Cargo.toml`. The document example uses the unchanged V1 overview
document identity recipe.

The acceptance journey publishes V7, terminates the scan process, starts a new
process, validates generated documentation, queries all seven IDs, and replays
each query byte-for-byte. An unknown valid ID returns `query.not_found`, exit
14, empty stdout, and no mutation.

## Security and authority

V2 adds no read, write, network, process, resolver, Cargo, rustc, build,
proc-macro, target, dependency, registry, path-traversal, model, or Council
authority. It projects only facts already present in the validated local head
and documents already validated under the existing root boundary.

Raw external locators, branch/tag/rev values, source bytes, absolute paths,
environment values, and secrets remain forbidden in IDs, diagnostics,
coverage, query output, errors, logs, documentation, and telemetry.

## Errors and failure precedence

This amendment adds no error schema or code. Existing input and query failures
remain strict CodeNoesisErrorV5 values:

- malformed ID: `input.invalid_query_id`, exit 2;
- unknown well-formed ID: `query.not_found`, exit 14;
- invalid or mismatched head: `query.snapshot_mismatch`, exit 14;
- invalid generated documents: `query.corrupt_documents`, exit 14;
- oversized result: `query.result_limit_exceeded`, exit 14.

Failures emit empty stdout and one LF-terminated stderr document. Query does
not repair, migrate, retry, truncate, partially succeed, or change the head.

## Test-first evidence

The test-only head
`4b8a38b2cbd9dcefb9dc5af0f3bd393ef6c95573` ran:

```text
python3 -m unittest scripts.tests.test_s4_cargo_manifest_facts_contract.S4CargoManifestFactsGovernanceTests.test_r4_exact_id_query_contract_is_complete
```

It exited 1 for the accepted assertion
`R4 diagnostics lack stable exact-query identity`. The retained 1,259-byte
log has SHA-256
`d4d489aa2afa625813da46660b5f3042fd5f38502d709f9fce4cffb5ffee8574`.
No product or protected semantic file changed before that Red.

The governance Green requires the focused guard, the complete R4 guard, the
inherited S4 guard, and the repository Python regression suite.

## Storage, compatibility, and rollback

V7 diagnostic IDs participate in the V7 semantic payload and therefore change
only the not-yet-released R4 semantic hashes and golden expectations under
human review. V1-V6 bytes are unchanged.

No DDL, local-store marker, artifact role, CAS rule, transaction, head
transition, migration, repair, sweep, deletion, or downgrade behavior changes.
The governance package can be reverted atomically before product R4 merges.
The product branch remains separate and consumes this contract only after the
protected governance merge is recorded in issue #104.

## Consequences

Positive:

- every R4 fact class named by the approved oracle has a discoverable stable
  identity and strict query result;
- V1 compatibility is preserved instead of weakened;
- relationship claims and uncertainty artifacts become directly inspectable;
- restart and replay behavior can be validated without adding authority.

Costs and limitations:

- V7 success queries use a new public result schema;
- each Cargo diagnostic now carries a stable semantic ID;
- the R4 bundle and fixture plan require renewed semantic review;
- traversal, search, resolver-grade facts, and implementation-aware reasoning
  remain deferred.

## Contract bundle

The SRS binds the amended R4 governance bundle. It includes this decision,
LocalQueryResultV2, the exact-ID oracle, diagnostic and coverage identity
contracts, retained Red, fixture query examples, and the independent guard.
The SRS is excluded to avoid a circular digest. Any bound-byte change requires
renewed human review.
