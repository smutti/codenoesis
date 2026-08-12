# Decision 0030: R16 bounded safe Rust constant evaluation

- Status: Proposed branch-scoped candidate
- Date: 2026-08-12
- Issue: [#172](https://github.com/smutti/codenoesis/issues/172)
- Authorization: accountable-maintainer authorization in issue #172
- Exact base: `6043313789f6855770520ad5312672fdb081ef38`
- Requirements: candidate `FR-EXT-020` and `FR-EXP-008`, with the bounded amendments listed in issue #172
- Slice: `S4`
- Risk: high
- Owner and approver: `@smutti`

## Context

Protected merge #171 made the Decision 0029 R14/R15 real-repository correction
Approved and Implemented, but not Verified. R15 exposes source syntax and
declared-value metadata while deliberately retaining `rust.value_not_evaluated`
and attribute-semantics uncertainty. Consumers therefore cannot distinguish a
small, reviewed constant result from an unevaluated initializer without
reinterpreting untrusted source outside the ontology.

R16 adds one bounded, target-independent value layer over the exact corrected
R15 source-only lineage. It is not a Rust compiler, Cargo evaluator, abstract
interpreter, type checker, layout engine, active-`cfg` selector, or runtime
observation mechanism.

## Decision

Add the complete R15 selector matrix plus:

```text
--rust-constant-profile rust-safe-constant-evaluation-v1
```

The additive family is `codenoesis.configuration/v15`,
`codenoesis.repository-snapshot/v18`, `codenoesis.extraction/v15`,
`codenoesis.extraction-chunk/v15`, `codenoesis.knowledge-graph/v15`,
`codenoesis.ontology/rust/v15`, `codenoesis.semantic-hash-contract/v14`,
`codenoesis.error/v24`, `codenoesis.local-query-result/v13`,
`codenoesis.portable-graph/v9`, `codenoesis.local-explorer/v9`,
`codenoesis.pipeline/s4-r16-v1`,
`codenoesis.rust-constant-evaluation/s4-r16-v1`, and
`codenoesis.constant-evaluation-index/v1`. DocumentationManifestV1, local
store publication, artifact roles, marker ownership, and viewer bytes remain
unchanged.

R16 composes only with the complete corrected R15 source-only lineage. The
existing `local-snapshot-64m-v1` and scan-only `local-snapshot-256m-v1`
selectors may alter only final capacity. Repository boundaries, direct-`cfg`
alternatives, SCIP/compiler input, generated or nested source, and R10-R13
composition fail before acquisition. Omitting the selector preserves every
R0-R15/K1/S7 byte.

## Closed subjects and grammar

R16 evaluates only an existing K1 `rust.declared_value` owned by:

- a `rust.constant` with an exact primitive annotation;
- an immutable `rust.static` with an exact primitive annotation; or
- a unit `rust.enum_variant` in one closed enum with exactly one direct literal
  fixed-width integer `#[repr(T)]`.

Supported result types are exactly `bool`, `i8`, `i16`, `i32`, `i64`, `i128`,
`u8`, `u16`, `u32`, `u64`, and `u128`. `usize`, `isize`, floating point,
characters, strings, bytes, references, aliases, generics, aggregates,
pointers, and user-defined types are target-dependent or unsupported.

The expression grammar contains only boolean literals; integer literals in
binary, octal, decimal, or hexadecimal notation with separators and either no
suffix or the exact subject suffix; parentheses; unary signed `-`; unary `!`
for booleans and fixed-width integers; integer `+`, `-`, `*`, `/`, `%`, `&`,
`|`, and `^`; boolean `&&` and `||`; same-type comparisons; and one direct
unqualified identifier that uniquely resolves to another R16-complete `const`
in the same semantic owner. Forward source order is permitted.

All integer operations are checked in the exact subject width. Overflow,
underflow, zero division or remainder, signed minimum divided by minus one,
suffix mismatch, invalid literals, ambiguous or absent dependencies, cycles,
over-depth graphs, and unsupported syntax publish no evaluated value. Casts,
shifts, paths, imports, associated constants, calls, fields, indexing,
closures, macros, `cfg`, target properties, environment, intrinsics, runtime
values, implicit conversions, and inferred types are forbidden.

A supported enum is all-or-nothing. It contains only unit variants, has one
supported direct literal repr, has no enum/variant uncertainty, evaluates
explicit discriminants with the closed integer grammar, starts the first
implicit discriminant at zero, and computes each later implicit discriminant
as checked predecessor plus one. R16 makes no ABI, layout, niche, payload,
default-`isize`, or compiler-validity claim.

## Ontology and derivation

R16 adds exactly `rust.evaluated_value` and `EVALUATES_TO`, from the existing
declared-value entity to the evaluated entity. There is at most one evaluated
entity per declared-value identity.

Entity identity is BLAKE3-256 over the canonical JSON array:

```text
["codenoesis.entity-id/rust-safe-constant-evaluation/v1",
 "evaluated_value", repository-id, declared-value-id]
```

Relationship identity is BLAKE3-256 over:

```text
["codenoesis.relationship-id/rust-safe-constant-evaluation/v1",
 "EVALUATES_TO", declared-value-id, evaluated-value-id]
```

An evaluated entity stores only its declared-value reference and properties
`value_kind`, `canonical_value`, `rust_type`, `type_authority`, and
`rule_version`. Canonical integers use sign plus base-10 digits without
separators or leading zeroes; booleans use `true` or `false`. Raw initializer,
literal lexeme, expression, body, snippet, absolute path, URL, environment,
argument, and telemetry data are forbidden.

Both entity and relationship claims are `derived_fact`. The
`codenoesis.constant-evaluation-index/v1` record retains canonical evaluated
entity and relationship IDs plus, for every result, its rule, input claim IDs,
input evidence IDs, and evaluated dependency entity IDs. Duplicate or dangling
identity/evidence, inconsistent values, absent derivation inputs, represented
cycles, non-canonical order, index mismatch, privacy violations, or limit
excess fail before publication.

## Uncertainty and compatibility

Unsupported source remains a successful scan with zero guessed values and one
of the exact capabilities:

- `rust.constant_target_dependent`;
- `rust.constant_expression_not_evaluated`;
- `rust.constant_dependency_not_evaluated`;
- `rust.constant_arithmetic_not_defined`;
- `rust.enum_discriminant_not_evaluated`.

A successful subject discharges only the inherited
`rust.value_not_evaluated` gap bound to the same evidence. A complete fixed-
repr enum also discharges only the repr-specific
`rust.attribute_semantics_not_interpreted` diagnostic and gap. Every unrelated
compiler, runtime, type, data-flow, ownership, side-effect, and active-`cfg`
gap remains. Repair, fallback, truncation, retry-as-success, partial enum
publication, and best effort are forbidden.

## Limits and security

R16 inherits all corrected R15 limits and adds 4,096 candidate declared values
per source, 256 syntax nodes per expression, 256 direct dependencies per
subject, 64 dependency levels, 4,096 variants per enum, 200,000 evaluated
entities, 200,000 evaluation relationships, 400,000 dependency references,
and 1,000,000 derivation-input references. Every maximum and maximum-plus-one
is executable. Fifty input/argument permutations and ten schedules must be
byte-identical.

The 60,000 ms extraction deadline and 4,294,967,296-byte reference-host RSS
ceiling remain. Analysis is offline and read-only. It may not invoke Git,
Cargo, rustc, build scripts, proc macros, target code, processes, networking,
plugins, models, browsers, or mutation.

## Frozen oracle and Red

The project-owned fixture is
`urn:codenoesis:fixture:s4-rust-safe-constant-evaluation-v1`, commit
`d77f3b77aae0aeabb89c8833e4ab4d655075b837`, tree
`f46c4f56d5fd506ab5ce3f5fb338ee240065ad0b`. Its exact R15 baseline has 35
entities, 35 relationships, 70 claims, 33 evidence, one diagnostic, 35
coverage records, and semantic hash
`ab76dc72a7cc57cf25f4112a551c047ad091693b32d0f732b1a5005f6e504908`.

R16 evaluates `BASE = 14`, `OFFSET = 9`, `ENABLED = true`, `LIMIT = 256`, and
`Mode::{Off,Warm,Hot} = {0,3,4}`. It retains exactly dependencies
`BASE -> OFFSET` and `Mode::Warm -> Mode::Hot`; emits no value for
`TARGET_WIDTH`, `CALL_RESULT`, or `PlatformSized`; removes exactly five value
gaps plus the closed repr gap and diagnostic; and adds exactly three typed
gaps. The complete graph has 42 entities, 42 relationships, 84 claims, 33
evidence, zero diagnostics, 32 coverage records, 65 deterministic claims, and
19 derived claims. Canonical stdout is 214,974 bytes with semantic hash
`ad760d2ef7e5807140b1feabd071047494ed17545ffaccf02ebe7302e65a54df`.
The complete machine oracle is
`tests/fixtures/s4/rust-safe-constant-evaluation-v1/expected-safe-constant-evaluation.json`.

The first branch commit contains complete governance, contracts, fixture,
traceability, guard, and CLI E2E with no production source edit. On the exact
base the focused E2E must retain the selector-registration Red: exit 2, empty
stdout, absent store, 149-byte ErrorV4 `input.invalid_revision`, and stderr
SHA-256 `7f75f7a91f6af0328795f3fbd2729e69756beba2ebd642cc1f6401265662a2fe`.
Only checkpoint-bound Red permits production edits.

## Real-repository pilots

Pinned Lekton commit `247b8f42fb045db41166d70a276a41c2e079b6eb`
must complete scan, docs, exact-ID query, export, strict reimport, and explore
under the 256 MiB scan envelope. Exact reviewed results include
`SEARCH_DEBOUNCE_MS = 250`, `CHECKSUM_MASK = 77`, and
`DEFAULT_MAX_ATTACHMENT_SIZE = 26214400`. Two fresh stores must have identical
semantic payloads and downstream artifacts except the existing volatile
envelope.

Pinned RustDesk commit `d412d198720aa56f6cfed2dfad262e8fb1322fb7`
must fail before publication, with or without `local-gitlinks-v1`, using
`input.unsupported_rust_constant_evaluation_composition`, reason
`repository_boundary_not_supported`, exit 2, empty stdout, absent store, and no
nested read.

## Consequences

Deterministic consumers and LLM projections can query reviewed primitive
constant results and enumerative values without re-evaluating source. Coverage
is deliberately narrower than Rust constant evaluation, and explicit gaps are
preferred to plausible but ungrounded values. The candidate becomes Approved
and Implemented only after protected manual merge and remains not Verified
until its immutable evidence is independently accepted.
