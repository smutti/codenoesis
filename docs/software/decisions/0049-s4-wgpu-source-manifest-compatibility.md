# Decision 0049: wgpu source and manifest compatibility

- Status: Proposed branch candidate
- Date: 2026-09-04
- Issue: [#221](https://github.com/smutti/codenoesis/issues/221)
- Exact dependent base: `ddb0acafeaa18ea1e01ba9bc48502974c68aa97a`
- Requirements: `FR-EXT-023`, `FR-EXT-024`
- Slice: `S4`
- Dependencies: none
- Owner and approver: `@smutti`

## Context

Merged PR #220 lets the pinned wgpu revision complete Cargo workspace package
planning. The next R3 failures are not invalid Rust: tree-sitter-rust 0.24.2
reports a bare `$` token inside an opaque `macro_rules!` definition and valid
`#[cfg(...)]` attributes on struct-pattern fields as parser errors. Once R3
accepts those closed forms, R4 reaches legal Cargo dotted dependency fields
such as `tracy-client.workspace = true` in `benches/Cargo.toml`.

Rejecting these forms prevents CodeNoesis from publishing otherwise supported
source and manifest facts. Treating arbitrary parser recovery as valid or
flattening arbitrary dotted TOML keys would instead manufacture unsupported
meaning.

## Decision

### Source parser compatibility

The tolerant R3 source planner and its downstream semantic parser may reparse
a same-length whitespace projection only for these exact tree-sitter gaps:

1. an error node whose source is exactly `$`, nested in a token tree under a
   `macro_definition`; and
2. a balanced `#[cfg(...)]` attribute followed by a named or renamed
   struct-pattern field, when the attribute is contained in an error node or
   is the exact field attribute bracketed by the parser's struct-pattern
   recovery nodes.

The projection preserves every byte offset and newline. Reparsing may proceed
iteratively because one corrected recovery site can expose another, but each
iteration must remove at least one newly recognized site. The result is
accepted only when the final tree has no error or missing node. R3 records the
existing `rust_unsupported_construct` diagnostic and coverage gap. Downstream
profiles may use the clean positional tree but may not assign meaning to the
projected bytes.

The legacy strict workspace extractor remains unchanged. Any unrecognized
error, missing node, malformed `cfg`, nested syntax ambiguity, or unrelated
malformed source still fails closed.

### Cargo dotted dependency declarations

Within an approved dependency section, R4 groups legal two-segment dotted
fields with the same first segment into one dependency declaration. Thus
`name.workspace`, `name.optional`, and the other already approved dependency
fields are parsed exactly like their equivalent inline table. The declaration
retains one evidence span covering its committed fields and then passes through
the existing field, source-selector, workspace-reference, limit, privacy, and
identity validation.

A direct declaration mixed with dotted fields, duplicate field, path deeper
than two segments, unknown field, missing workspace declaration, or invalid
value retains a typed fail-closed result. No effective dependency graph is
resolved.

## Safety and compatibility

Both corrections inspect only committed UTF-8 source already present in the
immutable inventory. They perform no macro expansion, Cargo or rustc
invocation, build script, target execution, dependency fetch, active-cfg
selection, filesystem discovery, network request, plugin call, or model call.

The package changes no public schema, ontology family, identity function,
dependency version, selector, release authority, or historical golden. Clean
parser inputs and non-dotted dependency declarations preserve their existing
paths. A supported `cfg` attribute elsewhere in a source file is not projected
merely because another recognized parser gap exists.

Direct `cfg` alternatives for inline modules remain outside this decision.
Pinned wgpu therefore stops honestly at R5 on the two `trace` module
declarations in `wgpu-core/src/snatch.rs`; their inherited heterogeneous
members require a separate semantic model rather than a parser exception.

## Acceptance and evidence

Focused tests cover bare-dollar macro bodies, named and renamed cfg pattern
fields, iterative recovery, unrelated malformed syntax, retained supported cfg
semantics, dotted workspace dependencies, conflicts, and nested-key rejection.
The complete Rust adapter suite and the R3 CLI journey pass.

Product commit `8d6ab53406f543356b22c614be64104e4b32c6d7` produces the release
binary with SHA-256
`0ae27fc648addbe8f51782a22fb529e2efc4a1bdf4bd3935ce4dcdd993885c5e`.
Pinned wgpu now emits:

- R3 RepositorySnapshotV6: 2,604 entities, 2,944 relationships, semantic hash
  `3fffccaa9edf9db5da036100af1ca5ecaea4ea087c70d135f4dbbc76613dc28a`;
- R4 RepositorySnapshotV7: 3,400 entities, 4,124 relationships, semantic hash
  `607c1eaa3e00bc52a36b33b2b605143ab62ed9b50a48dc9db84a8d8c3f3bb2f`;
- R5 typed `codenoesis.error/v12`
  `extraction.rust_semantic_identity_conflict` for `rust.module` `trace`.

All three R5 samples return exit `11`, empty stdout, 352 stderr bytes, and the
same error context. The complete eight-repository evaluation matches 8/8
reviewed oracles with report SHA-256
`d703fc7ec7bd5478eaf9674ba814cc01aa8169a065c4f0344c27f6f5e4c114cb`.
Aggregate stage coverage is acquisition 7/8, workspace 7/8, manifest 6/8,
semantic 4/8, framework 4/8, flow 3/8, and constant 3/8.

The candidate becomes Approved and Implemented only after independent review
and protected manual merge.
