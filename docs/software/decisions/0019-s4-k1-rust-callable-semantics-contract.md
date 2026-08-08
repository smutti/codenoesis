# Decision 0019: S4 K1 Rust callable and value semantics contract

| Field | Decision |
|---|---|
| Status | Proposed candidate; effective only after protected manual merge |
| Date | 2026-08-08 |
| Issue | [#142](https://github.com/smutti/codenoesis/issues/142) |
| Human authorization | Interactive “procediamo con K1” authorization recorded by issue #142, plus the [minimal storage/docs path expansion](https://github.com/smutti/codenoesis/issues/142#issuecomment-5225581916) |
| Exact base | `03ee09b172e84b5b7f5f423f9f65d63cf2953385` |
| Slice | `S4 — Evidence-backed workspace docs compatibility extension` |
| Risk | High: public ontology, identities, schemas, evidence, privacy, query, and export |
| Rollback | Revert the complete package; K1 is selector-bound and migrates no R0-R8 state |

## Context

R5 represents declaration-level members and methods but deliberately does not
evaluate declared values or inspect callable bodies. R6 adds framework-neutral
source declarations without call or runtime claims. R7 imports bounded static
SCIP relationships, while R8 exports and explores that graph. Those accepted
profiles remain useful, but an LLM cannot yet ask for a complete source
signature, distinguish a normalized literal from an unevaluated expression, or
inspect evidence-backed local call/control syntax.

K1 adds only deterministic facts visible in committed Rust syntax. It does not
turn syntax into compiler, data-flow, reachability, side-effect, or runtime
truth. Unsupported meaning remains a typed candidate, diagnostic, or coverage
gap.

## Decision

The explicit selector `--rust-callable-profile rust-callable-semantics-v1`
creates `RepositorySnapshotV11`, `ExtractionChunkV8`, `KnowledgeGraphV8`, and
Rust ontology v8 over the complete R6 source lineage. K1 requires the existing
workspace, Cargo-manifest, Rust-semantic, and framework profiles. K1 v1 rejects
composition with the R7 compiler-index selector; a future decision may define
an evidence-preserving source/compiler join. Selector absence preserves all
R0-R8 bytes.

The public journey is:

```text
noesis scan ... --profile standard-local-s4 \
  --workspace-profile cargo-root-package-v1 \
  --cargo-manifest-profile cargo-manifest-facts-v1 \
  --rust-semantic-profile rust-semantic-depth-v1 \
  --rust-framework-profile rust-framework-declarations-v1 \
  --rust-callable-profile rust-callable-semantics-v1
noesis docs ...
noesis query ... --id <exact K1 identity>
noesis export ... --portable-profile rust-callable-semantics-v1
noesis explore ... --explorer-profile rust-callable-semantics-v1
```

Every successful command is local, deterministic, bounded, and independent of
Cargo, rustc, Git subprocesses, build scripts, proc macros, target binaries,
network access, model providers, browser auto-launch, and ambient configuration.

## Public versions

| Contract | Version |
|---|---|
| configuration | `codenoesis.configuration/v8` |
| ontology | `codenoesis.ontology/rust/v8` |
| extraction | `codenoesis.extraction/v8` |
| extraction chunk | `codenoesis.extraction-chunk/v8` |
| knowledge graph | `codenoesis.knowledge-graph/v8` |
| repository snapshot | `codenoesis.repository-snapshot/v11` |
| local query | `codenoesis.local-query-result/v6` |
| portable graph | `codenoesis.portable-graph/v2` |
| local explorer | `codenoesis.local-explorer/v2` |
| error | `codenoesis.error/v16` |

V11 uses semantic hash domains
`codenoesis.repository-snapshot.semantic.v11`,
`codenoesis.knowledge-graph.semantic.v8`, and
`codenoesis.extraction-chunk.semantic.v8`. Existing V3-V10 domains are
unchanged.

## Callable declarations

K1 covers free functions plus R5 trait declarations, inherent implementation
methods, and named local trait-implementation methods. Each callable has one
`rust.callable_signature` entity linked by `HAS_SIGNATURE`. Its properties are:

- existing callable and lexical owner identities;
- visibility and the booleans `async`, `const`, and `unsafe`;
- literal ABI string or null when no `extern` qualifier exists;
- bounded NFC generic-parameter and where-clause spellings;
- declared return-type spelling or `unit_default`;
- body state `present` or `absent`, exact byte span, and BLAKE3-256 body digest
  when present, with no exported body text.

Each ordered parameter is a `rust.parameter` entity linked from the signature
by `HAS_PARAMETER`. It records zero-based ordinal, bounded NFC pattern and type
spellings, and receiver state `none`, `value`, `ref`, `ref_mut`, `typed_self`,
or `explicit`. Type inference is not performed.

Closures, generated or macro-expanded callables, active cfg-world selection,
trait solving, monomorphization, inferred types, and runtime dispatch are not
callables in K1 v1.

## Declared values

Every explicit enum discriminant and every constant/static initializer creates
one `rust.declared_value` entity linked from the existing declaration by
`DECLARES_VALUE`. An enum variant without a discriminant receives the same
entity with state `unresolved` and no expression digest.

For an explicit expression K1 records syntax kind, byte span, byte length, and
BLAKE3-256 digest, never arbitrary expression text. State is:

- `normalized_scalar` only for `true`/`false`, one integer literal with
  optional unary minus and explicit radix/suffix metadata, one character
  literal, or one string literal whose escapes are accepted by the closed
  subset;
- `expression_only` for a well-formed expression outside that subset;
- `unresolved` when no explicit expression exists or the literal cannot be
  decoded without interpretation.

Integers are represented as sign, radix, normalized digit string, and optional
suffix rather than a machine integer. K1 does not evaluate arithmetic, names,
constants, macros, cfg, casts, transitive expressions, or target-width values.

## Body syntax

For committed bodies K1 emits ordered entities for:

- `rust.local_binding` from `let` declarations;
- `rust.call_site` from direct and method-call syntax;
- `rust.control` for `if`, `if let`, `match`, `loop`, `while`, `while let`,
  `for`, `return`, `break`, `continue`, and postfix try/`?`.

Each record has an exact evidence span, lexical depth, source ordinal, and
optional parent body-fact identity. The callable links to every record through
`HAS_BODY_FACT`.

A `CALLS` relationship is emitted from the containing callable to a target
only when a direct unqualified or `self::`/`crate::` path identifies exactly
one already-known local free function. Its evidence is the call-site span.
Method calls, associated calls, imports, external paths, ambiguous names,
dynamic dispatch, and compiler-dependent resolution remain
`candidate_unresolved`; each has a diagnostic and
`rust.call_target_resolution` gap. K1 never emits `EXECUTES`, `REACHES`,
`READS`, `WRITES`, or an implicit CFG edge.

## Identity and ordering

Existing R0-R7 identities are reused unchanged. New identities are BLAKE3-256
over canonical JSON arrays:

```text
signature = ["codenoesis.entity-id/rust-callable-semantics/v1",
             repository_identity, callable_id, "rust.callable_signature"]
parameter = [domain, repository_identity, callable_id, "rust.parameter",
             decimal_ordinal, normalized_pattern]
value     = [domain, repository_identity, declaration_id,
             "rust.declared_value"]
body fact = [domain, repository_identity, callable_id, kind, evidence_id]
relation  = ["codenoesis.relationship-id/rust-callable-semantics/v1",
             kind, source_id, target_id]
diagnostic= ["codenoesis.diagnostic-id/rust-callable-semantics/v1",
             code, subject_id, ordered_evidence_ids]
gap       = ["codenoesis.coverage-gap-id/rust-callable-semantics/v1",
             capability, state, subject_id, ordered_evidence_ids]
```

IDs, evidence, entities, relationships, claims, diagnostics, and gaps are
strictly sorted by exact UTF-8 identity and unique. Parameters and body facts
also retain their explicit source ordinal. NFC collisions fail closed.

## Query, documentation, and portable projection

The V8 documentation renderer adds deterministic callable, declared-value,
call-site, control, and uncertainty sections while preserving the v1 manifest
format and exact evidence-linked statement model. It does not reproduce body
or arbitrary initializer text.

`LocalQueryResultV6` retrieves every V8 subject family by exact identity and,
for a callable or declaration, includes only its directly linked K1 records and
relationships in stable-ID order. V1-V5 dispatch and bytes are unchanged.

`PortableGraphV2` preserves the complete validated V11 graph and documentation
families, including K1 fields, without source contents or arbitrary snippets.
It adds no translated identity or inferred fact. Reimport requires canonical
JSON, strict versions, unique sorted identities, complete endpoint/evidence
closure, and exact family digests. `LocalExplorerV2` is an offline read-only
view with exact-ID/text search, typed filters, evidence inspection, and bounded
depth-1/2 traversal. Untrusted labels are rendered only with `textContent` under
a closed CSP; no network, server, browser launch, storage, dynamic evaluation,
or telemetry is permitted.

## Limits

| Limit | Maximum |
|---|---:|
| callables per source | 4,096 |
| parameters per callable | 256 |
| body facts per callable | 8,192 |
| body-fact lexical depth | 256 |
| signature component bytes | 4,096 |
| expression metadata bytes | 4,096 |
| K1 entities total | 200,000 |
| K1 relationships total | 400,000 |
| K1 diagnostics total | 50,000 |
| K1 coverage gaps total | 50,000 |
| portable graph bytes | 268,435,456 |
| JSON nesting | 64 |
| deterministic permutations | 50 |
| deterministic schedules | 10 |

Maximum and maximum-plus-one behavior is tested for every K1-specific bound.
Excess fails before publication with ErrorV16 and no partial head or output.

## Errors and uncertainty

ErrorV16 adds invalid K1 profile/composition, malformed callable syntax,
identity collision, limit, invalid V11 snapshot, invalid PortableGraphV2,
unsafe output, and asset-integrity codes. Human-readable messages are bounded
and contain no source body, expression, absolute path, environment, or secret.

Coverage explicitly includes call-target resolution, scalar evaluation,
closures, generated/macro callables, cfg selection, type inference, trait
solving, compiler CFG, reachability, data flow, aliasing/ownership flow, side
effects, and runtime behavior. No missing capability is silently upgraded.

## Fixture and acceptance

The project-owned fixture
`tests/fixtures/s4/rust-callable-semantics-v1` contains nine reviewed callable
signatures, fifteen ordered parameters, ten declared values, four local
bindings, nine call sites, all eleven control kinds, four unique-local `CALLS`
edges, and five unresolved call candidates. It includes literal and
expression-only values, Unicode identifiers, comments/string/macro decoys, and
a build-script execution sentinel.

The machine oracle and schemas under `tests/specifications/s4/k1` bind the
complete public journey, identity preimages, expected counts, limits, invalid
cases, compatibility, and retained Red/Green evidence. Fifty insertion-order
permutations and ten schedules must be byte-identical.

The governance checkpoint must precede every production Rust edit. Its focused
test is expected Red only because `s4_k1`, the K1 extractor/contracts, CLI
selector, V11 persistence, PortableGraphV2, LocalExplorerV2, and implementation
evidence are absent. The checkpoint commit and Red log remain visible in branch
history.

## Compatibility, consequences, and rollback

No dependency is added. K1 executes no target or compiler. It does not
modify R0-R8 schemas, fixtures, golden outputs, viewer assets, identities, hash
domains, selectors, stored heads, or command bytes. It intentionally offers
source-only lexical call semantics rather than pretending to be a compiler.

Before merge, the requirement and implementation remain Proposed candidates.
Protected manual merge atomically approves the bounded K1 contract and makes
the reviewed implementation effective. The authoring agent cannot approve or
merge. Reverting the package removes the selector and additive versions without
migrating or rewriting historical state.
