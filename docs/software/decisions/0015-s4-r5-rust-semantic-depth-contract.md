# Decision 0015: S4 R5 Rust semantic-depth contract

| Field | Value |
|---|---|
| Status | Proposed; becomes Accepted only when `@smutti` manually merges the exact protected head of [PR #112](https://github.com/smutti/codenoesis/pull/112) |
| Date | 2026-08-04 |
| Owners | Andrea Moretti (`@smutti` governance persona), accountable maintainer `@smutti` |
| Scope | `S4 — Evidence-backed workspace docs compatibility extension` only; roadmap `R5` |
| Requirement | Proposed `FR-EXT-010`, with bounded compatibility amendments to `FR-EXT-001/002`, `FR-KNW-001/002/003`, `FR-DOC-001/002`, `FR-QRY-001`, and `FR-CLI-001` |
| Risk | High — public ontology, identity, snapshot, extraction, query, error, parser, evidence, and compatibility contracts |
| Governance issue | [#111](https://github.com/smutti/codenoesis/issues/111) |
| Authorization | [Accountable-maintainer authorization](https://github.com/smutti/codenoesis/issues/111#issuecomment-5179817871) |
| Protected review | [PR #112](https://github.com/smutti/codenoesis/pull/112) |
| Required base | `e7cceb08b0aa4b7342cd2c6c1e267733130bd5f8` |

## Context

The R4 product merge can acquire normal local Rust repositories, represent
bounded root-package Cargo workspaces, retain declaration-level manifest facts,
publish evidence-backed documentation, and answer exact-ID queries. Its Rust
ontology still has intentionally shallow declaration coverage. It can represent
structs, enums, traits, functions, and a subset of methods, but not the member
declarations needed to explain real source structure reliably:

- named and tuple fields;
- enum variants and variant fields;
- module and associated constants;
- immutable or mutable statics;
- associated types;
- inherent methods and collision-safe named local trait-implementation methods;
- declarations that carry outer attributes.

Dropping an attribute-decorated declaration hides committed syntax. Treating a
`cfg`, derive, custom attribute, macro token tree, type spelling, initializer,
or method body as resolved behavior invents knowledge that the standard local
profile does not possess. Both outcomes violate the evidence model and
`INV-MDL-001`.

The already pinned Lekton and RustDesk revisions contain enough structs,
enums, constants, statics, attributes, trait implementations, and inherent
implementations to motivate this generic subset. Their observed counts are
sampling input, not ontology truth or acceptance goldens. No external source
is vendored.

## Decision

Add one explicit Rust semantic-depth selector:

```text
--rust-semantic-profile rust-semantic-depth-v1
```

It is valid only for `scan --profile standard-local-s4` together with
`--workspace-profile cargo-root-package-v1` and
`--manifest-profile cargo-manifest-facts-v1`. R1 packed acquisition and R2
gitlink representation remain independent optional selectors.

Repository content, file names, attributes, macros, Cargo metadata, corpus
identity, and earlier profiles never select R5 implicitly. Missing or invalid
composition fails before repository acquisition with ErrorV12. Every invocation
without this selector retains its accepted success, error, storage,
documentation, and query bytes.

This protected change ratifies governance only. Product implementation
**requires a separate Ready issue** after this decision and `FR-EXT-010` are
independently reviewed and manually merged. It authorizes no production code,
dependency, workflow, release, migration, or control-plane change.

## Public versions

The selected path uses exactly:

| Contract | Version |
|---|---|
| Snapshot | `codenoesis.repository-snapshot/v8` (`RepositorySnapshotV8`) |
| Configuration | `codenoesis.configuration/v5` |
| Extraction chunk | `codenoesis.extraction-chunk/v5` (`ExtractionChunkV5`) |
| Extraction contract | `codenoesis.extraction/v5` |
| Knowledge graph | `codenoesis.knowledge-graph/v5` (`KnowledgeGraphV5`) |
| Rust ontology | `codenoesis.ontology/rust/v5` |
| Error | `codenoesis.error/v12` (`ErrorV12`) |
| Exact-ID query | `codenoesis.local-query-result/v3` (`LocalQueryResultV3`) |
| Pipeline | `codenoesis.pipeline/s4-r5-v1` |
| Rust semantic extractor | `codenoesis.rust-semantic/s4-r5-v1` |
| Semantic index | `codenoesis.rust-semantic-index/v1` |
| Semantic hash | `codenoesis.semantic-hash-contract/v4` |

The V8 lineage retains the immutable R4 Cargo manifest index, R3 workspace
projection, R2 optional boundary projection, S3 storage/publication semantics,
S4 document roles, and evidence-lineage v2.

## Declaration authority

R5 may claim only that one approved declaration form exists in a committed,
bounded UTF-8 Rust source blob selected by the immutable R4 workspace plan.
Each new entity and relationship has exactly one deterministic parser claim
and resolving committed-source evidence. Byte spans never enter identity.

R5 does not claim:

- an active `cfg` or feature world;
- attribute, derive, declarative-macro, or procedural-macro meaning;
- macro expansion or generated declarations;
- a framework component, service, configuration, route, endpoint, handler, or
  dependency-injection role;
- a resolved type, bound, lifetime, generic, where clause, trait selection, or
  external implementation;
- an evaluated constant, static, initializer, or discriminant value;
- a call, data-flow, control-flow, side effect, runtime behavior, or observed
  execution.

R5 therefore emits no `CALLS` or `EXECUTES` relationship. Build scripts,
procedural macros, rustc, Cargo, targets, dependencies, generated output,
network clients, model providers, and external repositories remain unopened
and unexecuted.

## Entity semantics

Ontology v5 adds exactly these entity kinds:

### `rust.field`

A field belongs to one supported `rust.struct` or `rust.enum_variant`. It
records named or tuple form, normalized declared name or zero-based tuple
index, declared visibility, bounded declared type spelling, and compilation
presence. The type spelling is syntax, not a resolved type.

### `rust.enum_variant`

A variant belongs to one `rust.enum`. It records unit, tuple, or struct form,
whether a discriminant is present, and compilation presence. The discriminant
is never evaluated. Tuple or struct fields are separate owned `rust.field`
entities.

### `rust.constant`

A constant belongs to one module, trait declaration, inherent implementation,
or supported named local trait implementation. It records bounded declared
type spelling, initializer presence, and compilation presence. The initializer
is not parsed as a value and no value is evaluated.

### `rust.static`

A static belongs to one module. It records visibility, declared mutability,
bounded declared type spelling, initializer presence, and compilation
presence. The scanner never reads or evaluates the runtime value, including a
`static mut` value.

### `rust.associated_type`

An associated type belongs to one trait declaration or supported named local
trait implementation. It records whether declared bounds or a default are
present and compilation presence. No type is resolved.

### V8 `rust.method`

Ontology v5 extends `rust.method` to distinguish:

- trait required and default method declarations;
- inherent implementation methods on one unambiguous local struct or enum;
- methods in one supported named local trait implementation on one
  unambiguous local struct or enum.

The method records implementation context, optional resolved local trait
context, receiver presence, bounded signature spelling, compilation presence,
and bounded outer-attribute evidence. Generic and where-clause text may be
retained but is not resolved.

## Relationships and cardinality

R5 adds no relationship kind. It extends only the reviewed owner/member matrix:

- a struct `DEFINES` its fields;
- an enum `DEFINES` its variants;
- a struct variant `DEFINES` its fields;
- a module, trait, or supported implementation context `DEFINES` its approved
  constants, statics, associated types, and methods;
- one resolved local struct or enum `IMPLEMENTS` one resolved local trait for a
  supported named trait implementation.

An entity has exactly one lexical owner. Every new member is reachable from its
unchanged crate through existing `CONTAINS`/`DEFINES` edges and the new reviewed
`DEFINES` edges. A named local trait implementation that is ambiguous after NFC
normalization fails; it is never repaired by source order or an ordinal.

## Identity

All unchanged R4 crate, source, module, struct, enum, trait, type-alias,
function, Cargo entity, relationship, claim, evidence, diagnostic, coverage,
document, and snapshot identity recipes remain unchanged. R5 uses one disjoint
member domain:

```text
codenoesis.entity-id/rust-member/v1
```

The RFC 8785 JSON-array preimage, hashed with BLAKE3-256, contains exactly:

1. repository identity;
2. unchanged crate identity;
3. lexical owner identity;
4. member kind;
5. NFC declared name or zero-based tuple index;
6. resolved local trait identity or the empty string.

The trait identity is mandatory for named local trait-implementation methods,
associated types, and constants. Two traits exposing `render` on one type
therefore produce distinct stable IDs.

Commit OID, source byte offset, file/output path, source order, scheduler order,
worker count, inferred type, evaluated value, active configuration, and macro
output never enter identity. Duplicate or normalization-colliding preimages
produce `extraction.rust_semantic_identity_conflict`; there is no ordinal,
offset, or retry repair.

## Attributes and compilation presence

An outer attribute is bounded evidence attached to its committed declaration.
The declaration remains visible; token text is not converted into framework or
runtime properties.

Compilation presence is exactly one of:

- `unconditional` for declarations without `cfg`/`cfg_attr` uncertainty;
- `conditional_unknown` for `#[cfg(...)]` with
  `rust.cfg_presence_unresolved`;
- `attribute_transform_unknown` for `#[cfg_attr(...)]` with
  `rust.cfg_presence_unresolved`.

Every other outer attribute, including built-in-looking, derive, and custom
paths, emits `rust.attribute_semantics_not_interpreted` while preserving the
declaration. Attribute names and token text never imply an active branch,
generated item, component, service, configuration, endpoint, route, handler,
or any other framework role.

Macro token trees may be retained as a bounded unsupported region but their
contents never become entities. `rust.macro_generated_items_not_analyzed`
makes that absence explicit.

## Unsupported syntax and epistemic gaps

The exact R5 capability/state pairs are:

| Capability | State |
|---|---|
| `rust.attribute_semantics_not_interpreted` | `unsupported` |
| `rust.cfg_presence_unresolved` | `not_resolved` |
| `rust.macro_generated_items_not_analyzed` | `not_analyzed` |
| `rust.type_resolution_not_performed` | `not_resolved` |
| `rust.value_not_evaluated` | `not_evaluated` |
| `rust.union_unsupported` | `unsupported` |
| `rust.foreign_block_unsupported` | `unsupported` |
| `rust.unsupported_impl_header` | `unsupported` |

Unions, foreign blocks, negative or blanket implementation headers, unresolved
or external implementation targets, macro-generated items, invalid UTF-8
boundaries, malformed declarations, and unsupported owner/member compositions
either fail with ErrorV12 or produce the exact reviewed diagnostic and coverage
gap. They never disappear silently and never grant traversal or execution
authority.

## Limits

The fixed initial maxima are:

| Limit | Maximum |
|---|---:|
| Fields per struct or struct variant | 1,024 |
| Variants per enum | 1,024 |
| Tuple fields per owner | 1,024 |
| Associated items per trait or supported implementation context | 1,024 |
| Outer attributes per declaration | 128 |
| One attribute token payload | 16,384 UTF-8 bytes |
| One declared type or implementation-header spelling | 4,096 UTF-8 bytes |
| Determinism permutations | 50 |

Existing S2 graph maxima remain authoritative for total entities,
relationships, claims, evidence, diagnostics, and coverage gaps. A
maximum-plus-one case produces `extraction.rust_semantic_limit_exceeded` with
the exact limit, maximum, and observed value. No limit silently truncates and
no partial snapshot is published.

## ErrorV12

The R5-selected path emits only these strict codes:

- `input.invalid_rust_semantic_profile`;
- `extraction.invalid_rust_semantic_declaration`;
- `extraction.rust_semantic_identity_conflict`;
- `extraction.rust_semantic_limit_exceeded`;
- `extraction.unsupported_rust_semantic_composition`;
- `internal.unexpected`.

Every expected input or extraction failure is non-retryable, carries only the
bounded context fixed by the schema, writes no partial semantic artifact, and
does not fall through to a legacy error version. Selector-absent invocations
retain their legacy error bytes.

## Snapshot, storage, docs, and query compatibility

RepositorySnapshotV8 binds ConfigurationV5, ExtractionChunkV5,
KnowledgeGraphV5, ontology v5, extraction contract v5, pipeline
`s4-r5-v1`, and semantic-hash contract v4. V8 reuses the immutable local store,
artifact roles, single-writer transaction, crash/restart behavior, and envelope
semantics. It introduces no migration, repair, deletion, multi-writer, or
release behavior.

Documentation must show every R5 declaration as declared syntax, link its
claim and committed evidence, and expose compilation presence, diagnostics,
and coverage gaps. It must never label a syntactic declaration as active or
observed behavior.

Exact-ID dispatch is automatic from the stored validated snapshot version:

| Snapshot | Query result |
|---|---|
| V4, V5, V6 | byte-identical `LocalQueryResultV1` |
| V7 | byte-identical `LocalQueryResultV2` |
| V8 | additive `LocalQueryResultV3` |

V3 retains the V2 outer shape and result kinds while accepting the v5 entity,
diagnostic, and coverage contracts. It performs exact-ID lookup only: no fuzzy
search, traversal, pagination, mutation, repair, or version-selection flag.

## Fixture and acceptance oracle

The project-owned `rust-semantic-depth-v1` fixture contains:

- named, tuple, and unit structs;
- unit, tuple, and struct enum variants;
- nested and duplicate-across-owner field names;
- module and associated constants, immutable and mutable statics, and
  associated types;
- trait required/default methods, multiple inherent implementation blocks, and
  two local traits implementing the same `render` name on one local type;
- raw and Unicode identifiers;
- `cfg`, `cfg_attr`, derive, built-in-looking, and custom attributes;
- comment, string, and macro token-tree decoys;
- a build-script sentinel that must never execute.

The invalid matrix separately fixes malformed, normalization-collision,
ambiguous-resolution, unsupported-header, hard-negative, forbidden-authority,
and every maximum-plus-one case. Fifty inventory/source/chunk/schedule
permutations and isolated replay must produce byte-identical semantic, docs,
and exact-ID query bytes.

The pinned public pilots are:

- Lekton at `7a4d1a4a30468f4c18ce158a9b825680b00f4820`;
- RustDesk at `d412d198720aa56f6cfed2dfad262e8fb1322fb7`.

Pilot source is not vendored. Pilot counts are non-authoritative observations
used to test generic coverage and reveal gaps; changing a public repository or
observation never rewrites the project-owned golden automatically.

## TDD and retained Red

The governance guard is
`scripts.tests.test_s4_rust_semantic_depth_contract`. It was committed and
bound to PR #112 before any Decision 0015, schema, subset, oracle, or fixture
byte existed.

Command:

```text
python3 -m unittest scripts.tests.test_s4_rust_semantic_depth_contract
```

On test-first head `b6d3ec20b69258fac76fc42b5b95c7ea8f436da0`, the command exited `1` because
`docs/software/decisions/0015-s4-r5-rust-semantic-depth-contract.md` did not
exist. The 683-byte raw log has SHA-256
`d565d942729dece7b3cd08b2a67b714962b4de6979e3c7aa309061b5c4a89dd4`.
No production or semantic-contract byte changed before that Red.

The later product issue must retain a separate executable public CLI Red before
changing production code. The governance Red is not product implementation
evidence.

## Contract bundle and review boundary

The R5 contract bundle includes Decision 0015, the conformance guard, retained
Red, strict schemas, machine subset and oracles, project-owned fixture, expected
facts, immutable R4 bundle dependency, and repository license. SRS is excluded
from the bundle; the roadmap and the bundle itself are also excluded so
editorial or recursive bytes cannot invalidate the semantic package implicitly.

Any bound-byte change requires a new digest and semantic human review. Golden
changes require review of meaning, not snapshot regeneration. Independent
ontology, identity, semantic, security, and V1/V2/V3 compatibility review is
required before manual merge. The authoring agent may not approve or merge.

## Deferred work

R5 intentionally defers:

- framework-neutral component, service, configuration, endpoint, route, and
  handler declarations plus honest macro handling to R6;
- compiler, rust-analyzer, or SCIP resolution and macro products to R7;
- portable export and the read-only local explorer to R8;
- incremental semantic invalidation changes to a later explicitly approved S5
  amendment;
- implementation-aware API behavior semantics to the separately governed S7
  capability.

## Consequences

Positive consequences:

- real Rust repositories gain a bounded, deterministic declaration vocabulary;
- same-name trait methods no longer collide in V8;
- outer attributes stop making declarations disappear while uncertainty stays
  explicit;
- every new fact remains evidence-backed and queryable after restart;
- R4 and every selector-absent contract stay stable.

Costs and limitations:

- V8/V5/V12/V3 contracts add compatibility and review surface;
- source syntax cannot prove active configuration, expanded macros, resolved
  types, values, calls, or runtime roles;
- public pilots can reveal missing generic cases but cannot silently broaden
  the ontology;
- production implementation remains blocked until a separate Ready issue fixes
  its own executable Red, paths, rollback boundary, evidence, and stop
  conditions.

## Alternatives rejected

- **Infer fields or roles from names and text matching.** Rejected because it
  invents semantic certainty and is unstable under decoys.
- **Evaluate `cfg`, macros, Cargo, or rustc in the standard profile.** Rejected
  because it adds execution and environment authority outside S4.
- **Use source offsets or ordinals for member identity.** Rejected because
  harmless reordering would churn IDs and hide collisions.
- **Fold framework roles into R5.** Rejected because syntax declarations and
  framework/runtime meaning require a separate R6 capability contract.
- **Jump directly to the explorer.** Rejected because R8 must visualize a
  reviewed ontology rather than compensate for missing semantic contracts.
