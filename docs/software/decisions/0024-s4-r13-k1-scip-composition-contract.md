# Decision 0024: R13 K1 and revision-bound SCIP composition

- Status: Proposed branch-scoped candidate
- Date: 2026-08-10
- Issue: [#160](https://github.com/smutti/codenoesis/issues/160)
- Authorization: [maintainer authorization](https://github.com/smutti/codenoesis/issues/160#issuecomment-5237555163) and [oracle correction authorization](https://github.com/smutti/codenoesis/issues/160#issuecomment-5237601867)
- Exact base: `cc25dec49343c510f124585c91d459982e827c68`
- Slice: `S4`
- Risk: high
- Owner and approver: `@smutti`

## Context

K1 adds deterministic committed-source callable signatures, parameters, values,
body-syntax facts, and a deliberately narrow source-only call-resolution rule.
R7 imports one explicitly supplied, revision-bound Rust SCIP artifact and adds
compiler symbols plus only explicit `RESOLVES_TO`, `REFERENCES`, `IMPLEMENTS`,
and `TYPE_DEFINITION` assertions. Both lineages preserve the same R6 source
identities, but the authorized base rejects selecting them together.

Keeping the lineages disconnected forces a consumer to rediscover whether a
K1 callable and an R7 compiler symbol refer to the same committed declaration.
Joining by spelling, source order, ranges, or an LLM would be ambiguous and
would discard the existing R7 binding proof. Rewriting K1 or R7 would alter
historical bytes. R13 therefore adds only an evidence-backed correspondence
over identities that R7 already bound exactly.

## Decision

Add one explicit R13 composition selected only by the complete existing R7 and
K1 selector matrix:

```text
--profile standard-local-s4
--workspace-profile cargo-root-package-v1
--manifest-profile cargo-manifest-facts-v1
--rust-semantic-profile rust-semantic-depth-v1
--rust-framework-profile rust-framework-declarations-v1
--compiler-index-profile scip-rust-v0.9.0-import-v1
--compiler-index-binding <safe-relative-path>
--rust-callable-profile rust-callable-semantics-v1
```

R13 rejects R10/R12 declaration alternatives, R2/R11 boundary composition,
nested source, and the R9 output-capacity selector. Selector absence preserves
all prior bytes. The additive contracts are:

| Contract | Value |
|---|---|
| configuration | `codenoesis.configuration/v12` |
| repository snapshot | `codenoesis.repository-snapshot/v15` |
| extraction/chunk | `codenoesis.extraction/v12` / `codenoesis.extraction-chunk/v12` |
| graph/ontology | `codenoesis.knowledge-graph/v12` / `codenoesis.ontology/rust/v12` |
| semantic hash | `codenoesis.semantic-hash-contract/v11` |
| error/query | `codenoesis.error/v20` / `codenoesis.local-query-result/v10` |
| portable/explorer | `codenoesis.portable-graph/v6` / `codenoesis.local-explorer/v6` |
| pipeline | `codenoesis.pipeline/s4-r13-v1` |
| composition | `codenoesis.rust-callable-scip-composition/s4-r13-v1` |
| join index | `codenoesis.callable-compiler-join-index/v1` |

`DocumentationManifestV1`, local-store publication, artifact roles, generated
root ownership, and the K1 viewer bytes remain unchanged.

## Exact correspondence

The composition starts from independently valid K1 knowledge and R7 compiler
overlay over the same repository identity, immutable commit, root tree,
inventory, and R6 source manifest. A join is eligible only when:

1. one R7 `compiler.symbol` has `in_repository_bound` state;
2. its existing non-null `source_entity_id` is one K1 `rust.function` or
   `rust.method` identity;
3. that callable has exactly one existing K1 `HAS_SIGNATURE` relationship;
4. the symbol has exactly one committed definition source locator and one
   canonical compiler definition locator; and
5. no second compiler symbol claims the same callable identity.

The graph then adds exactly:

```text
source callable
  HAS_SIGNATURE       -> K1 callable signature
  HAS_COMPILER_SYMBOL -> R7 compiler.symbol
```

`HAS_COMPILER_SYMBOL` means only that the validated revision-bound artifact
binds the compiler symbol definition to the same exact committed source
declaration identity. It does not assert type equivalence, call resolution,
dispatch, trait selection, generated-source identity, active configuration, or
runtime behavior.

The relationship ID is BLAKE3-256 over the canonical JSON array:

```json
[
  "codenoesis.relationship-id/rust-callable-scip-composition/v1",
  "HAS_COMPILER_SYMBOL",
  "<source-callable-id>",
  "<compiler-symbol-id>"
]
```

The relationship carries exactly the committed definition source evidence and
compiler definition evidence. The unchanged claim model creates one
evidence-backed claim. A sorted join index records source callable, K1
signature, compiler symbol, and join relationship IDs and must equal the graph
projection exactly.

Compiler symbols bound to non-callable source entities remain valid and
unjoined. External, generated, omitted, source-unbound, or ambiguous compiler
symbols remain unchanged and unjoined. K1 call sites retain their existing
resolution state. R7 compiler relationships retain their exact identities and
meanings.

## Extraction and publication

1. Validate the complete selector intent before repository acquisition.
2. Acquire one immutable repository inventory through the existing safe local
   adapter and one immutable R7 binding/artifact pair through the existing
   race-safe sidecar boundary.
3. Extract and validate the K1 R6/callable lineage.
4. Import and validate the R7 overlay against the exact same R6 knowledge.
5. Build only exact eligible joins and validate their cardinality, evidence,
   identity, endpoint closure, and index projection.
6. Form canonical V12 extraction chunks and one V12 graph by identity-union of
   unchanged K1 and R7 families plus the additive joins.
7. Publish V15 atomically through the existing local-store protocol.

Identity-union accepts a duplicate only when the complete JSON record is
byte-equivalent after canonical serialization. A same-ID disagreement is a
typed composition failure; no side silently wins.

## Read projections

`LocalQueryResultV10` returns the existing exact result kinds and adds stable
linked callable/compiler entities and relationships. Querying a joined
callable, its signature, its compiler symbol, or the join relationship exposes
the same bounded neighborhood and exact evidence without inferred call edges.

Generated documentation labels the compiler correspondence as a validated
artifact assertion and distinguishes it from source syntax and runtime facts.
`PortableGraphV6` is a lossless strict V15 plus deterministic documents
projection. It excludes source contents, snippets, body or initializer text,
absolute roots, raw URLs, credentials, environment, arguments, and telemetry.
`LocalExplorerV6` reuses immutable K1 viewer bytes and grants no network,
storage, mutation, dynamic-code, execution, or browser-launch authority.

## Errors, limits, and determinism

`CodeNoesisErrorV20` composes existing R7/K1 failures and adds typed invalid
composition, duplicate compiler ownership, missing/duplicate signature,
invalid join evidence, identity conflict, limit, snapshot, query, portable,
and explorer failures. Every failure has empty stdout and no partial visible
head or generated root.

R13 inherits every R7 and K1 limit and adds a maximum of 200,000 joins/index
entries, one compiler symbol and one signature per joined callable, and exactly
two evidence IDs per join. Maximum and maximum-plus-one are required. Fifty
input permutations and ten schedules must produce byte-identical canonical
semantics. Repair, fallback, truncation, sampling, silent deduplication, and
retry-as-evidence are forbidden.

The product launches no process, opens no network channel, invokes no Git,
Cargo, rustc, build script, macro, target, index generator, model provider, or
browser, and does not mutate repository source or traverse nested repositories.

## Compatibility

Every R0-R12 and K1 selector, contract, schema, identity/hash domain, fixture,
golden, evidence pack, accepted command, query, portable projection, explorer,
and viewer byte remains immutable. R7 alone remains V10 and K1 alone remains
V11. Only the exact complete R7 plus K1 matrix emits V15.

R13 does not compose with R10/R12 alternatives because a target-specific SCIP
definition would require a separately reviewed alternative-to-compiler binding.
It does not add SCIP generation authority. External repository diagnostics need
an independently reviewed binding/artifact pair and are not acceptance oracles.

## Verification

The immutable `compiler-index-v1` fixture provides both lineages over one
reviewed two-crate repository. Independent replay establishes the exact union:

- 61 entities;
- 53 relationships, including five `HAS_COMPILER_SYMBOL` joins;
- 80 evidence records;
- 114 claims;
- four diagnostics;
- 52 coverage records;
- five K1 signatures, eight parameters, two local bindings, and two unresolved
  call sites.

The `store.get` and `load` call sites remain `candidate_unresolved`; R13 emits
no new `CALLS`. The public journey is scan, docs, exact callable/signature/
compiler/join query, export, strict reimport, and explore.

The first branch commit contains this decision, Proposed SRS/architecture/
roadmap changes, strict schemas, immutable fixture descriptor, oracle, contract
test, and executable E2E without production source edits. On exact base
`cc25dec49343c510f124585c91d459982e827c68`, the E2E fails before acquisition
with ErrorV16 `input.unsupported_rust_callable_composition`, exit 11, empty
stdout, absent store, and stderr SHA-256
`2573e0f364350b300218c6d1940e6eb33f4f0bc70b7ba92dd9b2821f5bf97013`.
Only retained expected Red permits implementation.

## Consequences

CodeNoesis can expose a machine-queryable, evidence-backed bridge between
source callable shape and compiler-grade symbol identity without pretending
that either layer proves calls or runtime behavior. The additive contract and
strict union increase implementation and validation cost, but preserve all
historical lineages and create a sound base for later expression, binding,
data-flow, LLM projection, and semantic-diff work.
