# Decision 0027: R15 closed Rust local-flow contract

- Status: Proposed branch-scoped candidate
- Date: 2026-08-11
- Issue: [#166](https://github.com/smutti/codenoesis/issues/166)
- Authorization: [accountable-maintainer authorization](https://github.com/smutti/codenoesis/issues/166#issuecomment-5252891670)
- Exact base: `011057c84258a26b08b12ced7ae1df478dbb5048`
- Requirements: candidate `FR-EXT-017` and `FR-EXP-007`, with the bounded amendments listed in issue #166
- Slice: `S4`
- Risk: high
- Owner and approver: `@smutti`

## Context

R14 exposes committed-source expressions, pattern bindings, and syntax-only
`READS`/`WRITES`, but deliberately does not connect those occurrences through
control or data flow. Consumers otherwise need to infer paths from source text,
which loses reviewed identity, evidence, limits, and epistemic boundaries.

Protected merges #163 and #165 make R14 and the empty additive R5 correction
Approved and Implemented, but not Verified. Their contracts and bytes remain
immutable. This decision adds one source-only layer over the exact R14 lineage;
it does not reinterpret or remove the existing compiler/runtime reachability or
data-flow gaps.

## Decision

Add the complete R14 selector matrix plus:

```text
--rust-flow-profile rust-local-flow-v1
```

The additive family is `codenoesis.configuration/v14`,
`codenoesis.repository-snapshot/v17`, `codenoesis.extraction/v14`,
`codenoesis.extraction-chunk/v14`, `codenoesis.knowledge-graph/v14`,
`codenoesis.ontology/rust/v14`, `codenoesis.semantic-hash-contract/v13`,
`codenoesis.error/v22`, `codenoesis.local-query-result/v12`,
`codenoesis.portable-graph/v8`, `codenoesis.local-explorer/v8`,
`codenoesis.pipeline/s4-r15-v1`,
`codenoesis.rust-local-flow/s4-r15-v1`, and
`codenoesis.local-flow-index/v1`. DocumentationManifestV1, storage
publication, artifact roles, marker ownership, and immutable K1 viewer bytes
remain unchanged.

R15 composes only with the complete R14 source-only lineage. The existing
`local-snapshot-64m-v1` selector may alter only final capacity. Repository
boundaries, cfg alternatives, compiler/SCIP input, generated or nested source,
and R10-R13 compositions fail before acquisition with ErrorV22 and no partial
publication. Omitting the R15 selector preserves exact R14 bytes.

## Closed callable grammar

A callable is complete only when its whole body is a finite acyclic tree made
from R14-supported parameters and lexical bindings, non-empty statement
sequences, initialized single-binding `let` statements, direct or compound
assignment to one uniquely resolved binding, R14 normal expression statements
or tails, and plain `if` expressions with one explicit non-empty `else`.
Nested branches are accepted to depth 64.

Loops, while/while-let, for, match, break, continue, return, try/`?`, await,
closures, nested functions, async/unsafe/const blocks, macros, labels,
destructuring or indirect writes, missing or empty branches, direct cfg
uncertainty, generated source, malformed syntax, unsupported R14 coverage, and
ambiguous scope reject the whole callable from R15 extraction. A rejected
callable emits zero R15 entities, relationships, claims, or derivations. It
retains all inherited facts and gaps and adds
`rust.syntax_normal_flow_not_analyzed` and
`rust.lexical_reaching_definitions_not_analyzed` tied to callable evidence.
Partial flow and guessed edges are forbidden.

## Blocks and syntax flow

R15 adds only `rust.syntax_basic_block`. Each block records repository,
callable, source file, exact committed-source locator and evidence, zero-based
ordinal, one role from `entry`, `condition`, `then_branch`, `else_branch`, or
`join`, ordered direct flow-node IDs, and
`flow_world = syntax_normal_completion`. It contains no source, condition,
statement, expression, or literal text.

The exact structural relationships are:

- `HAS_SYNTAX_BLOCK` from callable to block;
- `CONTAINS_FLOW_NODE` from block to one direct R14 pattern binding or root
  expression, or one K1 control fact;
- `HAS_CONDITION` from K1 `if` control to its exact R14 condition expression;
- `SYNTAX_NEXT`, `SYNTAX_TRUE_BRANCH`, and `SYNTAX_FALSE_BRANCH` for direct
  possible normal-completion successors; and
- `SYNTAX_REACHES` for strict transitive closure of those direct edges,
  excluding self.

`SYNTAX_*` is possible normal source progression under the closed grammar. It
is not executable validity, compiler CFG, panic/unwind behavior, actual
reachability, termination, side effects, or runtime execution.

## Lexical reaching definitions

Definitions are only exact R14 parameter/local pattern-binding entities at
activation and exact direct or compound assignment identifier occurrences
carrying R14 `WRITES`. Reads are only exact R14 occurrences already carrying
`READS`. Direct assignment evaluates right-hand reads before replacing the
target set; compound assignment reads the prior set before becoming the new
definition. Branch joins use exact path union and intersection.

R15 emits `LEXICAL_MUST_REACHES_READ` when one definition reaches a read on
every syntax-normal path to that read, and
`LEXICAL_MAY_REACHES_READ` for each definition that reaches it on some but not
all such paths. These edges do not claim values, equality, successful mutation,
alias-mediated writes, ownership, move/copy, borrow, type, validity, side
effects, compiler data flow, or runtime access.

Block, membership, condition, and direct syntax-edge claims are
`deterministic_fact`. Reachability and reaching-definition claims are
`derived_fact`. `codenoesis.local-flow-index/v1` retains completed callable
IDs, every R15 family ID, the exact rule version, and canonical sorted input
entity, relationship, and evidence IDs for every derived relationship.

## Identity and validation

Block identity is BLAKE3-256 over the canonical JSON string array:

```text
["codenoesis.entity-id/rust-local-flow/v1", "syntax_basic_block",
 repository-id, callable-id, source-file-id, start-byte, end-byte, role,
 ordinal]
```

Relationship identity is BLAKE3-256 over:

```text
["codenoesis.relationship-id/rust-local-flow/v1", kind, source-id, target-id]
```

R15 source evidence is disjoint and uses BLAKE3-256 over:

```text
["codenoesis.evidence-id/rust-local-flow/v1", repository-id, commit-oid,
 blob-oid, path, start-byte, end-byte]
```

Duplicate identities, spans, or ordinals; empty or invalid blocks; missing or
cross-callable references; invalid condition binding; missing or duplicate
branch edges; cycles; unreachable blocks; inconsistent closure or reaching
sets; forward definitions; R14 access disagreement; non-canonical order;
derivation, evidence, or index mismatch; and limit excess fail with ErrorV22
before publication. Repair, synthesis outside the rule, fallback, truncation,
best effort, and partial publication are forbidden.

## Limits, privacy, and compatibility

R15 inherits all R14 limits and adds 4,096 blocks per callable, 64 nested
branches, 4,096 direct flow nodes per block, 262,144 reach pairs per callable,
200,000 total blocks, 1,000,000 total R15 relationships, and 1,000,000 retained
derivation-input references. The final snapshot remains 32 MiB or the existing
explicit 64 MiB profile. Every maximum and maximum-plus-one is tested; excess
returns ErrorV22 with empty stdout, absent store, and no truncation. Fifty
argument permutations and ten schedules must be byte-identical.

PortableGraphV8 preserves the complete validated graph and derivation index but
contains no raw source/body/initializer/expression/condition/literal/snippet
text, absolute root, raw URL, credential, environment, command argument, or
telemetry. LocalExplorerV8 is static, read-only, offline, and reuses immutable
K1 viewer bytes. No Git, Cargo, rustc, build-script, proc-macro, target, process,
network, model, browser, repository mutation, cfg selection, macro expansion,
compiler-index generation/import, type/dispatch/borrow/ownership/alias,
constant evaluation, side-effect, runtime, API-diff, or semantic-diff authority
is granted.

## Frozen oracle and Red

The project-owned fixture is
`urn:codenoesis:fixture:s4-rust-local-flow-v1`, commit
`552a8cdc76b2dd80dc26ad1e3b381fc0de9eab24`, tree
`6a97e7b14d29db6aa50416fd9ac76b9022298104`. Its measured immutable R14
baseline has 32 entities, 49 relationships, 34 evidence, 81 claims, zero
diagnostics, 15 coverage records, and semantic hash
`9234051e60d266305da77fa5a750c64f42a22d719282a9b0afd7b93461213003`.

R15 adds exactly five blocks, 36 relationships, five disjoint evidence records,
25 deterministic claims, and 16 derived claims. The complete result is exactly
37 entities, 85 relationships, 39 evidence, 122 claims, zero diagnostics, and
15 coverage records. The machine descriptor freezes every span, endpoint,
identity, evidence locator, derivation input, order, and family digest.

The first branch commit contains this governance, contracts, fixture,
traceability, and executable E2E with no production source edit. On the exact
base the focused E2E is Red before acquisition because the selector is unknown:
exit 2, empty stdout, absent store, 149-byte ErrorV4
`input.invalid_revision`, and stderr SHA-256
`7f75f7a91f6af0328795f3fbd2729e69756beba2ebd642cc1f6401265662a2fe`.
Only retained checkpoint-bound Red permits production edits.

## Consequences

CodeNoesis gains evidence-backed local path and def-use context suitable for
deterministic consumers and LLM grounding while preserving explicit uncertainty
about compiler and runtime behavior. The strict whole-callable rule sacrifices
coverage instead of publishing partial or misleading flow.
