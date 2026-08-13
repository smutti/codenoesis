# Decision 0031: exact-schema LocalExplorerV3-V9 browser

- Status: Proposed branch-scoped correction candidate
- Date: 2026-08-13
- Issue: [#176](https://github.com/smutti/codenoesis/issues/176)
- Authorization: accountable-maintainer high-risk S4 authorization recorded in issue #176
- Exact base: `16252f59b2dd2302b3f660268843869a45f8ca87`
- Requirements: corrections to `FR-EXP-003`, the R11 amendment to `FR-EXP-002`, and `FR-EXP-004` through `FR-EXP-008`, with the bounded amendments in issue #176
- Slice: `S4`
- Risk: high
- Owner and approver: `@smutti`

## Context

LocalExplorerV3 through LocalExplorerV9 publish valid versioned manifests and
matching PortableGraphV3 through PortableGraphV9 files. Their browser entrypoint
nevertheless reuses the immutable K1 LocalExplorerV2 asset, whose loader accepts
only `codenoesis.portable-graph/v2`. Every later explorer therefore rejects its
own generated graph before search or inspection. Static artifact generation is
deterministic, but the advertised browser capability is not usable.

## Decision

Retain the existing LocalExplorerV3-V9 and PortableGraphV3-V9 schemas, marker
ownership, selectors, graph bytes, identities, and manifest structure. Replace
only the V3-V9 publisher asset with one reviewed offline template materialized
for an exact expected portable schema and display version. Each materialized
asset has an independently pinned SHA-256 and is bound by its existing manifest
entrypoint digest and length.

The exact mapping is V3 to PortableGraphV3 through V9 to PortableGraphV9. A
materialized viewer accepts only its matching schema. Accepting a range, falling
back to V2, repairing an input, or selecting a compatible version is forbidden.
The R8/V1 and K1/V2 assets and publishers remain byte-identical.

## Browser contract

The browser reads only one file explicitly selected by the user. Before it
enables inspection it validates:

- byte length at or below 268,435,456;
- exact matching `schema_version`;
- the eight common array families plus `document_statements`;
- version-specific `repository_boundaries`, `local_flow_index`, and
  `constant_evaluation_index` sections where required;
- unique non-empty indexed identities;
- absence of the reviewed privacy-denied fields.

On rejection the previous graph, indexes, selection, results, and visualization
are cleared and controls remain disabled. The browser never repairs, infers,
deduplicates, truncates, persists, uploads, or executes graph content.

For a valid graph it provides counts, exact-ID and bounded NFC text search,
family and exact-kind filters, direct relationship inspection, linked claims and
evidence, derivation references, diagnostics, coverage gaps, and deterministic
depth-one or depth-two neighborhoods. The neighborhood is rendered as a bounded
SVG node-link view and as bounded JSON. Labels and inspector content use DOM
text nodes; graph data never enters HTML markup or executable code.

## Security and determinism

The existing offline security boundary remains: no network, remote asset,
source-repository access, browser auto-launch, storage, telemetry, dynamic code,
`unsafe-inline`, `unsafe-eval`, process, plugin, model, or mutation authority.
The CSP pins the reviewed inline style and script. Fifty materializations per
version must be byte-identical, and each manifest must match the final viewer
digest and length exactly.

Malformed JSON, mismatched schemas, incomplete families, duplicate identities,
privacy-denied fields, and maximum-plus-one files fail closed. The existing Rust
strict reimport remains the authority for the complete PortableGraph contract;
the browser checks are an additional untrusted-file boundary, not a replacement
for canonical contract validation.

## Compatibility

No ontology, graph, query, portable graph, local explorer manifest, CLI envelope,
identity, evidence, derivation, fixture, source golden, or historical decision
changes. Historical evidence remains an immutable record that V3-V9 previously
reused the K1 bytes. Current acceptance tests are corrected to require exact
version-bound viewer behavior while continuing to prove V1/V2 byte stability.

## Verification

The governance checkpoint adds the exact contract and an R16 acceptance check
before product edits. On the authorized base that check is Red because the
generated V9 viewer contains only the V2 schema guard. Green requires focused
R10-R16 journeys, exact mismatch and malformed-input rejection, deterministic
asset/manifests, privacy and CSP checks, a real-browser fixture observation, and
the complete repository gate. Merge does not make any requirement Verified;
independent evidence acceptance remains separate.

## Consequences

- Generated V3-V9 explorer directories become directly usable offline.
- Viewer bytes and manifest entrypoint digests change only for V3-V9.
- V1/V2 compatibility bytes stay immutable.
- The correction introduces no dependency and no new product authority.
- Rollback is a complete revert of issue #176.
