# Decision 0023: R12 K1 cfg-alternatives composition contract

- Status: Proposed branch-scoped candidate
- Date: 2026-08-10
- Issue: [#158](https://github.com/smutti/codenoesis/issues/158)
- Authorization: [maintainer comment](https://github.com/smutti/codenoesis/issues/158#issuecomment-5236032172)
- Exact base: `e4ab3faa609da32e0f1b72e3382209dddf5ed5fb`
- Slice: `S4`
- Risk: high
- Owner and approver: `@smutti`

## Context

R10 preserves one stable R5 logical method and represents heterogeneous
direct-`cfg` declaration occurrences as evidence-backed alternatives. K1
attaches complete source signatures, parameters, values, body-syntax facts,
and uniquely proven local free-function calls to callable subjects. R6 retains
framework-neutral declarations and explicit uncertainty over the same source
lineage. On the authorized base these profiles cannot be selected together,
so occurrence-dependent K1 facts cannot be represented without either losing
the R10 alternatives or choosing an arbitrary declaration shape.

Changing V12 or V11 would alter historical R10 or K1 bytes. Attaching one K1
signature directly to an alternative-bearing logical method would falsely
collapse multiple committed declarations. Compiler/SCIP composition, `cfg`
evaluation, target selection, and expression/data-flow meaning remain outside
this package.

## Decision

Introduce one additive R12 family selected only by the complete existing R10
semantic profile, R6 framework profile, and K1 callable profile. The existing
R2 `local-gitlinks-v1` boundary report and R9
`local-snapshot-64m-v1` output capacity are optional. Neither optional selector
changes callable or alternative semantics.

The contracts are fixed as follows:

| Contract | Value |
|---|---|
| configuration | `codenoesis.configuration/v11` |
| repository snapshot | `codenoesis.repository-snapshot/v14` |
| extraction/chunk | `codenoesis.extraction/v11` / `codenoesis.extraction-chunk/v11` |
| graph/ontology | `codenoesis.knowledge-graph/v11` / `codenoesis.ontology/rust/v11` |
| semantic hash | `codenoesis.semantic-hash-contract/v10` |
| error/query | `codenoesis.error/v19` / `codenoesis.local-query-result/v9` |
| portable/explorer | `codenoesis.portable-graph/v5` / `codenoesis.local-explorer/v5` |
| pipeline | `codenoesis.pipeline/s4-r12-v1` |
| composition | `codenoesis.rust-callable-cfg-alternatives-composition/s4-r12-v1` |
| join extractor/index | `codenoesis.rust-callable-cfg-alternatives/s4-r12-v1` / `codenoesis.callable-cfg-alternatives-index/v1` |

The R10 alternative and K1 callable extractor/index versions remain
unchanged. R12 adds a deterministic join and new hash domains; it does not
claim a new Rust parser, `cfg` evaluator, compiler, or dispatch engine.

## Canonical subject mapping

R10 remains authoritative for logical declaration grouping. One unchanged
logical `rust.method` links sorted `rust.declaration_alternative` entities with
`HAS_DECLARATION_ALTERNATIVE`. Occurrence shape remains only on each
alternative.

When a logical method has alternatives, R12 emits no direct K1 signature,
parameter, body fact, or `CALLS` relationship on that logical method. Each
declaration-alternative ID becomes the K1 callable subject for its occurrence.
The existing K1 v1 preimages are reused unchanged with that subject:

```text
signature = [domain, repository_identity, declaration_alternative_id,
             "rust.callable_signature"]
parameter = [domain, repository_identity, declaration_alternative_id,
             "rust.parameter", ordinal, normalized_pattern]
body_fact = [domain, repository_identity, declaration_alternative_id,
             callable_fact_kind, evidence_id]
```

No occurrence ordinal or source order enters identity. Declaration evidence
selects the R10 alternative; K1 evidence selects body-fact identity. Moving an
occurrence may change its evidence, alternative, and occurrence-dependent K1
IDs while preserving the logical R5 method ID.

The join index contains sorted logical alternative-method IDs, alternative
callable-subject IDs, and corresponding signature IDs. It must equal the graph
projection exactly. Every alternative callable subject has exactly one
signature. A logical alternative-bearing method has none. Ordinary
non-alternative K1 subjects and all declared-value subjects remain unchanged.

`CALLS` remains limited to one uniquely known local free-function target. R12
does not select an active alternative or infer method dispatch, types,
reachability, runtime behavior, or target-dependent meaning.

## Acquisition and extraction order

1. Validate the exact selector composition before acquisition.
2. Acquire the immutable root revision, optionally with the existing R2
   boundary-aware adapter.
3. Build the R10 semantic lineage and declaration alternatives.
4. Retain the R6 framework declaration projection over that committed source.
5. Extract K1 facts per accepted declaration occurrence and join every
   alternative occurrence to its exact R10 alternative subject.
6. Validate inherited indexes, the new join index, evidence containment,
   cardinalities, limits, references, and hashes.
7. Publish one V14 snapshot atomically.

Nested source never enters inventory or extraction. An optional boundary
report remains the exact validated R2 report and grants no cross-boundary code
relationship. An optional capacity selector changes only the final V14 stdout
ceiling.

## Read projections

`LocalQueryResultV9` retrieves every normal graph family and directly linked
R10/K1 subjects, including logical method, declaration alternative, callable
signature, parameters, body facts, claims, and evidence. Optional boundary
subjects retain the R11 representation.

Generated documentation states that alternatives are unresolved committed
declarations, exposes their occurrence-specific signatures and syntax facts,
and never chooses an active world. `PortableGraphV5` is a lossless strict V14
plus documents projection. It excludes source contents, snippets, body text,
initializer text, absolute roots, raw URLs, credentials, environment, and
telemetry. `LocalExplorerV5` reuses the immutable K1 viewer bytes without
network, storage, dynamic-code, mutation, execution, or browser-launch
authority.

## Errors, limits, and atomicity

`CodeNoesisErrorV19` composes R10, K1, R6, and optional R2 failures. Invalid
selector pairings fail before acquisition. Missing, duplicate, direct-logical,
or mismatched occurrence signatures; invalid evidence containment; malformed,
overlapping, or cross-source alternatives; ambiguous calls; reference, hash,
order, privacy, path, race, and resource failures publish no partial output or
visible head.

R12 inherits every R10, K1, R6, and applicable R2 limit. Standard V14 stdout
remains `33,554,432` bytes including LF; the explicit capacity selector permits
`67,108,864` bytes. Portable V5 remains `268,435,456` bytes. Every applicable
maximum and maximum-plus-one is tested. Fifty permutations and ten schedules
must produce byte-identical canonical semantics. Truncation, repair, sampling,
silent deduplication, and retry-as-evidence are forbidden.

The product launches no child process, opens no network channel, invokes no
Git, Cargo, rustc, build script, macro, target, model provider, or browser, and
does not resolve URLs or traverse nested repositories.

## Compatibility

Every R0-R11 and K1 selector, contract, schema, identity, hash domain, error,
fixture, golden, evidence pack, accepted command, document, portable
projection, explorer manifest, and viewer byte remains immutable. K1 alone
stays V11, R10 alone stays V12, and K1 plus boundary without R10 stays V13.
Only the complete R10 plus R6 plus K1 selector matrix emits V14. Boundary and
capacity selectors are optional. R7/SCIP does not compose in R12.

## Verification

The project-owned fixture reuses the exact K1 repository identity and exact
Cargo, build-script, and library bytes while replacing only `src/model.rs`
with two reviewed direct-`cfg` `Worker::run` declarations. The logical method
keeps its R5 identity; the two declaration alternatives become independent K1
callable subjects. The public journey is scan, docs, logical/alternative/
signature exact-ID query, export, strict reimport, and explore.

The first branch commit contains this decision, Proposed SRS/roadmap changes,
schemas, fixture, oracle, contract test, and executable E2E without production
source. On exact base `e4ab3faa609da32e0f1b72e3382209dddf5ed5fb`, the E2E
must fail before acquisition with ErrorV17
`input.unsupported_rust_cfg_alternatives_composition`, exit `2`, empty stdout,
absent store, and stderr SHA-256
`dbe134dbc101765a8ebdc2ffe917f4776fddb42d10e3dfe1957e2aa819adb70c`.
Only retained expected Red permits production implementation.

Pinned RustDesk is diagnostic only. R12 must advance deterministically beyond
the former `try_start_clipboard` identity conflict. It may complete V14 or stop
at the same next typed ErrorV19 boundary; it is never an ontology golden.

## Consequences

CodeNoesis can represent occurrence-specific callable details without lying
about one active `cfg` world. The additive family costs another strict contract
and dispatch path, but it preserves historical bytes and provides the first
honest composition of declaration alternatives with function-level syntax
facts suitable for later reasoning layers.
