# Decision 0018: S4 R8 portable graph and offline explorer contract

| Field | Value |
|---|---|
| Status | Proposed; becomes Accepted only when `@smutti` manually merges the exact independently reviewed protected pull request governed by issue #110 |
| Date | 2026-08-06 |
| Owners | Andrea Moretti (`@smutti` governance persona), accountable maintainer `@smutti` |
| Scope | `S4 — Evidence-backed workspace docs compatibility extension` only; roadmap `R8` |
| Requirements | New `FR-EXP-001`; bounded additive amendments to `FR-QRY-001`, `FR-QRY-002`, `FR-CLI-001`, `FR-DOC-003`, `NFR-DET-001`, `NFR-SEC-001`, `NFR-SEC-005`, and `NFR-PRV-002` |
| Risk | High — public interchange contracts, deterministic graph identity, privacy, untrusted browser content, output confinement, and material resource limits |
| Governance issue | [#110](https://github.com/smutti/codenoesis/issues/110) |
| Authorization | [Accountable-maintainer authorization](https://github.com/smutti/codenoesis/issues/110#issuecomment-5205875435) |
| Required base | `d003a563830bdb5ff79197c8b92050b23eb92b27` |
| Immutable predecessor | R7 bundle `sha256:81ef2609c875af3d36a88f1fe97851f21368f90a60e2cc2706d6130ba95af882` |

## Context

Protected product merge #130 completed the R7 static SCIP-import path on the
required base. The current visible head can therefore contain
`RepositorySnapshotV10`, `KnowledgeGraphV7`, Rust ontology v7 compiler
assertions, exact SHA-256 or BLAKE3 evidence, diagnostics, coverage gaps, and
`LocalQueryResultV5` exact-ID results. R7 is Implemented but not Verified;
complete immutable retention and independent evidence acceptance remain open.

The graph is useful but is not yet portable or discoverable without issuing
exact-ID queries. Exporting an unversioned visualization graph would lose
CodeNoesis identity, claim-state, diagnostic, gap, document, or evidence
semantics. Embedding untrusted graph data into executable HTML or loading a
remote visualization library would also create privacy, CSP, XSS, supply-chain,
and offline-reproducibility failures.

Graphify motivated local search and neighborhood discoverability only. This
decision does not copy Graphify code and does not adopt label-based identity,
an open property graph as authority, CDN assets, whole-graph force layout,
LLM-generated facts, direct graph-database mutation, implicit cloning, or
community membership as product semantics.

## Decision

Ratify two additive public contracts:

1. `codenoesis.portable-graph/v1`, the sole R8 interchange artifact; and
2. `codenoesis.local-explorer/v1`, a non-canonical reconstructable manifest
   and first-party static viewer over a validated portable graph.

This protected package ratifies governance only. Product implementation
**requires a separate Ready product issue** after this decision and every
bounded R8 amendment are independently reviewed and manually merged.

## Explicit CLI journeys

Export is selected only by:

```text
noesis export \
  --store <local-store-root> \
  --repository-id <canonical-logical-id> \
  --output <portable-output-root> \
  --format json
```

It reads exactly one completely validated visible V10 head. Success atomically
publishes `portable-graph.json` plus the exact
`.codenoesis-portable-graph-v1` ownership marker. Stdout is the same canonical
`PortableGraphV1` JSON value plus one LF, byte-identical to the file; stderr is
empty and exit is `0`.

Explorer generation is selected only by:

```text
noesis explore \
  --input <portable-graph.json> \
  --output <explorer-output-root> \
  --format json
```

It validates one canonical portable graph before creating output. Success
atomically publishes the validated `portable-graph.json`, `index.html`,
`explorer-manifest.json`, and exact `.codenoesis-local-explorer-v1` ownership
marker. Stdout is strict `LocalExplorerManifestV1` plus one LF; stderr is empty
and exit is `0`.

Argument order is not semantic. Duplicate, missing, empty, unknown, or
incompatible arguments fail before store access, input decode, or destination
creation. Environment, current directory, repository content, browser state,
cached files, and ambient configuration never select either journey.

## Public versions and compatibility

| Role | Exact version |
|---|---|
| Source snapshot | `codenoesis.repository-snapshot/v10` |
| Source graph | `codenoesis.knowledge-graph/v7` |
| Source ontology | `codenoesis.ontology/rust/v7` |
| Existing exact query | `codenoesis.local-query-result/v5` |
| Interchange | `codenoesis.portable-graph/v1` (`PortableGraphV1`) |
| Explorer manifest | `codenoesis.local-explorer/v1` (`LocalExplorerV1`) |
| Selected failures | `codenoesis.error/v15` (`ErrorV15`) |

R8 changes no R7 entity, relationship, claim, evidence, diagnostic, coverage,
document, snapshot, ontology, query, or semantic-hash byte. `noesis scan`,
`docs`, `query`, `refresh`, and `federate` retain their accepted parsing,
streams, exits, artifacts, and mutation boundaries. Export and explorer are
explicit additive commands with no implicit migration or store write.

## PortableGraphV1

The top-level artifact binds:

- canonical repository identity, Git SHA-1 commit and tree OIDs;
- source schema `RepositorySnapshotV10`, stable snapshot ID, and semantic hash;
- Rust ontology v7 and LocalQueryResultV5 compatibility;
- the exact projection policy and ordered data families.

The artifact preserves all seven exact-query subject families: entities,
relationships, claims, evidence, diagnostics, coverage gaps, and documents.
Document statements are an eighth ordered adjunct family so every generated
statement keeps its document, subject, evidence, and gap bindings.

Every family reuses the immutable R7 or S4 schema shape by local `$ref`; R8
does not translate subjects into generic nodes and edges. Stable IDs, endpoint
IDs, claim states, evidence IDs and redacted locator metadata, diagnostics,
coverage gaps, document IDs, and statement links are copied exactly. Unknown
members, duplicate identities, unresolved references, and schema coercion are
forbidden.

The canonical payload is RFC8785 JSON using only the reviewed finite numeric
domain. The file and successful stdout contain that payload followed by one
LF. Top-level families are sorted by exact stable ID; nested source arrays keep
their source-contract canonical order. Export may buffer within the fixed byte
ceiling, but it must validate the complete artifact before the single atomic
publication. Repeated export and 50 shuffled insertion-order replays produce
byte-identical output.

No source contents or snippets are part of portable v1. Repository-relative
evidence paths, immutable object or artifact digests, byte/range locators,
extractor provenance already approved by R7, and explicit redaction gaps are
metadata, not authority to read a repository. Absolute paths, parent escapes,
file URLs, raw producer roots, raw arguments, environment values, and embedded
active content are not exported.

## Lossless validation and reimport

Validation is fail-closed in this exact order:

1. portable byte limit preflight;
2. JSON nesting preflight;
3. duplicate-member rejection;
4. exact schema and unknown-member rejection;
5. canonical family-order validation;
6. identity uniqueness;
7. endpoint, claim, document, statement, diagnostic, and gap reference closure;
8. evidence resolution;
9. repository, revision, snapshot, semantic-hash, ontology, and query binding;
10. exact family count, ordered ID list, and canonical family-digest equality.

Reimport reconstructs the same eight families and verifies every invariant.
It never guesses, repairs, drops, deduplicates, reorders semantically, upgrades
a claim, synthesizes evidence, or silently truncates. Loss, duplication,
ambiguity, unsupported version, unknown field, hash mismatch, reference
mismatch, unresolved evidence, or non-canonical order is a typed failure before
publication. Source-repository absence is valid because portable metadata is
self-contained.

## LocalExplorerV1

The explorer is a read-only materialized view. `portable-graph.json` remains
the only R8 data authority, and the V10 stored snapshot remains canonical
product truth. Browser state, layout, selection, filtering, and truncation
never alter either artifact.

The reviewed static entrypoint provides:

- exact-ID lookup across all subject families;
- case-sensitive NFC code-point substring search sorted by exact ID;
- filters for subject kind, relationship kind, claim state, diagnostic code,
  and coverage capability;
- deterministic breadth-first depth-1 or depth-2 neighborhood inspection;
- exact evidence metadata, diagnostic, gap, and document/statement inspection;
- explicit display truncation counters without projection truncation.

No locale collation, fuzzy score, graph community, model inference, claim-state
promotion, hidden ranking, or whole-graph layout enters the contract.

The page is opened manually. It requires the user to select
`portable-graph.json` through the browser file picker because a portable
`file:` page must not depend on network or local-server fetch behavior.
Untrusted values are assigned only with `textContent`; they are never parsed as
markup, CSS, URL, script, or event-handler source.

## Browser and output security

The viewer is one first-party static HTML asset with reviewed inline style and
script hashes. Its exact CSP defaults to none and separately denies connection,
object, frame, ancestor, form, base, manifest, media, font, and worker
capabilities. Only the reviewed style/script hashes plus self/data images are
permitted. There is no CDN, remote origin, telemetry, dynamic import, `eval`,
`Function`, worker, service worker, browser storage, cookie, frame, form,
plugin, or active untrusted markup.

No browser auto-launch, child process, local server, network service, target
execution, package-manager runtime, model provider, repository mutation, store
mutation, or write outside the selected marker-owned destination is allowed.
The explorer remains useful when the analyzed source repository is absent.

Both commands accept an absent or empty destination, or an existing directory
owned by the exact matching R8 marker and version. They reject non-empty
unmarked destinations, marker mismatch, every parent or component symlink,
absolute/parent escape, input-output aliasing, race-detected replacement, and
any write outside the destination. Failure leaves the prior complete owned
generation unchanged and exposes no partial generation.

The security corpus retains script-close, attribute-quote, Unicode line and
paragraph separators, bidi controls, disallowed controls, oversized labels and
metadata, remote origins, dynamic code, malicious evidence paths, symlink
escapes, corruption, and every maximum-plus-one case.

## Fixed limits

| Limit | Exact value |
|---|---:|
| Canonical portable graph bytes | 268,435,456 |
| Generated non-data viewer bytes | 1,048,576 |
| Text-search results displayed | 100 |
| Traversal depth default / maximum | 1 / 2 |
| Neighborhood subjects | 256 |
| Neighborhood relationships | 512 |
| JSON nesting | 64 |
| Deterministic permutations | 50 |

Every maximum-plus-one is rejected before proportional allocation or output
publication. Display result and neighborhood limits may truncate only the
non-canonical view and must report that fact. The canonical projection is never
silently truncated.

## ErrorV15

Selected R8 failures use strict LF-terminated ErrorV15 with empty stdout,
non-retryable status, redacted contexts, and no partial output. The closed
catalog covers invalid profiles, unsafe output, invalid or unsupported
snapshots and schemas, non-canonical projection, identity conflict, reference
or evidence failure, export limits, invalid or unsafe explorer projection,
asset-integrity mismatch, explorer limits, and internal failure.

The error context carries safe IDs or SHA-256 digests rather than raw untrusted
paths or payloads. Unsupported but reviewed source meaning remains an existing
R7 diagnostic or coverage gap; R8 does not turn uncertainty into an error or a
fact.

## Dependencies and implementation boundary

No new runtime, development, browser, JavaScript, CSS, Rust, or system
dependency is authorized. A later product issue must use existing locked Rust
dependencies and reviewed first-party static bytes. It may introduce only the
minimum inward-owned domain ports and CLI adapters required by this vertical
journey. Server, REST, MCP, graph databases, automatic browser opening, source
snippets, external assets, and ontology changes remain separate decisions.

## Governance Red and traceability

The conformance guard was committed before Decision 0018 and every R8 schema,
fixture, golden, security corpus, or bundle byte. On test-first head
`b8a3fde629417fb150275448f50ec9356b45ab76`, the command
`python3 -m unittest scripts.tests.test_s4_portable_explorer_contract` failed
only because this decision was absent, with exit `1`, empty stdout, and the
expected missing-artifact message. The retained 678-byte stderr log has
SHA-256 `784ac21ea5e0257136c3710616d463d00ee3117c0edb34dc40521aa73fe126e7`;
the retained test-first guard has SHA-256
`42ec1167d9be5c935b50496f3703a9840601ce68cec3f669dad8ccc6aa6ff959`.
No R8 contract, production, dependency, R7 golden, workflow, policy, release,
or unrelated protected byte changed before Red.

## Consequences and rollback

R8 makes the implemented Rust ontology portable and inspectable without
changing its authority or requiring the source repository. The cost is a new
strict public interchange boundary, complete reference validation, reviewed
static browser bytes, explicit output ownership, and security/resource tests.

The rollback boundary is the complete governance pull request. Reverting it
before product authorization restores the R7 baseline without migration,
runtime, dependency, release, destructive-data, or stored-format side effects.
The authoring agent does not approve or merge this decision.
