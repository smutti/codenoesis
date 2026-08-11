# Decision 0025: R14 Rust expression and lexical-binding contract

- Status: Proposed branch-scoped candidate
- Date: 2026-08-10
- Issue: [#162](https://github.com/smutti/codenoesis/issues/162)
- Authorization: [accountable-maintainer authorization](https://github.com/smutti/codenoesis/issues/162#issuecomment-5239994397) and [lexical-depth oracle correction](https://github.com/smutti/codenoesis/issues/162#issuecomment-5240550261)
- Exact base: `e32428ecac33df384b2e8b6eed3d257da06e18fe`
- Slice: `S4`
- Risk: high
- Owner and approver: `@smutti`

## Context

K1 records callable declarations, signatures, parameters, bounded values,
source-only call candidates, and syntactic control facts. It deliberately does
not expose the expression tree or lexical binding occurrences needed for an LLM
or deterministic consumer to inspect arguments, receivers, pattern-bound names,
or assignment/read syntax. Inferring these facts from source text outside the
validated graph would lose stable identity and evidence and could overstate
compiler or runtime meaning.

R13 is Approved and Implemented after protected merge #161 but remains not
Verified. R14 changes no R13 contract or byte and composes only with the exact
K1 source-only lineage.

## Decision

Add one explicit profile selected by the complete K1 matrix plus:

```text
--rust-expression-profile rust-expression-bindings-v1
```

The existing `local-snapshot-64m-v1` selector may additionally change only final
snapshot capacity. Repository-boundary, cfg-alternative, compiler/SCIP,
nested-source, generated-source, and R11-R13 compositions fail before
acquisition. K1 without the new selector remains byte-identical V8/V11.

The additive family is `codenoesis.configuration/v13`,
`codenoesis.repository-snapshot/v16`, `codenoesis.extraction/v13`,
`codenoesis.extraction-chunk/v13`, `codenoesis.knowledge-graph/v13`,
`codenoesis.ontology/rust/v13`, `codenoesis.semantic-hash-contract/v12`,
`codenoesis.error/v21`, `codenoesis.local-query-result/v11`,
`codenoesis.portable-graph/v7`, `codenoesis.local-explorer/v7`,
`codenoesis.pipeline/s4-r14-v1`,
`codenoesis.rust-expression-bindings/s4-r14-v1`, and
`codenoesis.expression-binding-index/v1`. DocumentationManifestV1, storage
publication, artifact roles, marker ownership, and K1 viewer bytes remain
unchanged.

## Closed expression facts

R14 adds only `rust.expression`, `rust.call_argument`, and
`rust.pattern_binding`. Selected expressions are exactly the 27 tree-sitter
Rust kinds frozen in `expression-bindings-v1.json`. Each expression stores its
exact committed-source locator, callable owner, selected direct parent, lexical
depth, sorted parse-position roles, normalized identifier/scoped spelling when
applicable, fixed operator when applicable, BLAKE3 source-byte digest, and byte
length. It stores no raw expression text or literal lexeme.

Lexical depth is exactly the number of direct selected
`CONTAINS_EXPRESSION` ancestors: a selected root has depth `0` and every
selected child has its selected parent's depth plus one. This is expression-tree
nesting only, not control-flow, scope, evaluation, or runtime depth.

Every expression has one `HAS_EXPRESSION`. `CONTAINS_EXPRESSION` exists only
between direct selected AST parent and child. Calls own zero-based contiguous
arguments through `HAS_ARGUMENT` and `ARGUMENT_VALUE`; field calls expose only
the syntax receiver through `HAS_RECEIVER`. An exact non-constructor K1
call-site evidence match adds `REPRESENTS_CALL_SITE`. Constructor-shaped calls
remain syntax-only.

Macros, closures, nested functions, async/unsafe/const blocks, generated syntax,
and malformed trees are not expanded. Identifier/self nodes count only in
expression grammar position, never declarations, path components, fields,
patterns, types, labels, attributes, imports, comments, strings, or docs.

## Bindings, scope, and access

The closed binding subset accepts identifier/self leaves in K1 parameters and
lets, one non-chained if-let or while-let, one for pattern, and one unguarded
match-arm pattern, including tuple, tuple-struct, mut, ref, and reference
wrappers. Constructor path tokens are not bindings. Every unsupported,
ambiguous, guarded, chained, captured, macro, or-pattern, range, slice, struct,
remaining-field, or malformed pattern emits typed coverage and no guessed
binding.

Origins are `parameter`, `local_let`, `if_let`, `while_let`, `for`, and
`match_arm`. Explicit mut/ref/ref-mut syntax is recorded without inferring
borrow, mutability, ownership, move/copy, alias, lifetime, type, or validity.
Parameter scope is the callable body; let scope starts after its initializer in
the containing block; control bindings are confined to their body/arm value.
Nearest in-scope prior binding wins; unsupported or ambiguous shadowing fails
closed as coverage.

`DECLARES_BINDING` joins the exact K1 parameter/local/control owner.
`BINDS_FROM` means only syntactic pattern input. Exact identifier/self uses emit
`READS`; a direct assignment target emits `WRITES`; a direct compound target
emits both. Declarations are not writes. Fields, indexes, dereferences,
destructuring assignment, captures, macros, and ambiguity emit coverage rather
than access edges.

`READS` and `WRITES` are syntax-occurrence facts, not def-use, reaching
definitions, data flow, mutation success, side effects, ownership transfer,
runtime access, or execution. Existing K1 ownership/data-flow/reachability/
side-effect coverage remains truthful.

## Identity and validation

Entity IDs are BLAKE3-256 over canonical JSON string arrays using
`codenoesis.entity-id/rust-expression-bindings/v1`:

```text
[domain,"expression",repository,callable,source-file,start,end,syntax-kind]
[domain,"argument",call-expression,ordinal]
[domain,"binding",repository,callable,scope-owner,source-file,start,end,name]
```

Relationship IDs use
`codenoesis.relationship-id/rust-expression-bindings/v1` and
`[domain,kind,source,target]`. Allowed kinds are exactly `HAS_EXPRESSION`,
`CONTAINS_EXPRESSION`, `HAS_ARGUMENT`, `ARGUMENT_VALUE`, `HAS_RECEIVER`,
`REPRESENTS_CALL_SITE`, `DECLARES_BINDING`, `BINDS_FROM`, `READS`, and
`WRITES`.

Duplicate identities/spans, invalid parents, non-contiguous arguments, dangling
or cross-callable endpoints, invalid scope, forward/out-of-scope access,
call-site evidence disagreement, invalid roles/operators, collisions, or index
mismatch fail with ErrorV21 before publication. Repair, fallback, truncation,
best effort, and inferred correspondence are forbidden.

## Security, privacy, limits, and compatibility

No process, network, Git, Cargo, rustc, build script, proc macro, target, index,
model, browser, repository mutation, nested traversal, cfg selection, macro
expansion, type/trait/method dispatch, borrow checking, CFG/data-flow,
constant evaluation, side-effect, runtime, API-diff, or semantic-diff authority
is granted. PortableGraphV7 excludes raw source/expression/literal text,
snippets, absolute roots, raw URLs, credentials, environment, command
arguments, and telemetry. LocalExplorerV7 reuses immutable K1 viewer bytes.

Limits are 16,384 expressions per callable, depth 256, 256 arguments per call,
4,096 bindings per callable, 400,000 total expressions, 200,000 total
bindings/arguments, 1,000,000 R14 relationships, and 4,096 UTF-8 bytes per
normalized spelling. Final snapshot capacity is 32 MiB or the existing explicit
64 MiB selector. Maximum and maximum-plus-one, 50 permutations, and ten
schedules are required. All R0-R13/K1 contracts, identities, fixtures, goldens,
evidence, queries, exports, explorers, and viewer bytes remain immutable.

## Frozen oracle and Red

The unchanged K1 fixture has tree
`ead855e0545cc26b351b305fcad39f2e491b285d`, commit
`9a7bb3adaa5bf30eef3bc9bc656c81f42fbdb845`, and baseline 91 entities,
96 relationships, 99 evidence, 187 claims, 8 diagnostics, and 93 coverage records.
R14 adds 73 expressions, 8 arguments, 23 bindings, 207 relationships,
86 evidence, and 311 claims, yielding exactly **195 entities, 303
relationships, 185 evidence, 498 claims, 8 diagnostics, and 93 coverage records**.
The additive relationships include 29 `READS`, 7 `WRITES`, 9
`REPRESENTS_CALL_SITE`, and no new K1 `CALLS`.

The first branch commit contains governance, schemas, exact fixture descriptor,
contract test, and executable E2E only. On the exact base, the focused E2E exits
2 before acquisition with 149-byte ErrorV4 `input.invalid_revision`, empty
stdout, absent store, and stderr SHA-256
`7f75f7a91f6af0328795f3fbd2729e69756beba2ebd642cc1f6401265662a2fe`.
Only retained expected Red permits production edits.

## Consequences

CodeNoesis gains a deterministic, evidence-backed expression and lexical access
layer useful to exact queries and LLM grounding without claiming compiler or
runtime semantics. The larger graph and strict scope validation increase cost,
but create a sound base for separately governed type, CFG/data-flow, semantic
diff, and LLM-oriented projections.
