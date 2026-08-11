# Decision 0026: Empty additive semantic-extension neutrality

- Status: Approved correction candidate; effective only after protected manual merge
- Date: 2026-08-11
- Issue: [#164](https://github.com/smutti/codenoesis/issues/164)
- Authorization: [accountable-maintainer authorization](https://github.com/smutti/codenoesis/issues/164#issuecomment-5250715942) and [immutable-R14 clarification](https://github.com/smutti/codenoesis/issues/164#issuecomment-5250970994)
- Exact base: `408e4021044cf6b3628b6a8873787148de719341`
- Requirement: approved `FR-EXT-010`, with bounded compatibility regressions through R14
- Slice: `S4`
- Risk: high
- Owner and approver: `@smutti`

## Context

Protected merge #163 made the complete R14 V13/V16 expression and lexical-
binding layer Approved and Implemented, but not Verified. Decision 0025 and its
R14 contract bundle remain byte-identical historical checkpoint artifacts; the
post-merge lifecycle is recorded here and in the SRS, architecture, roadmap,
README, and program tracker #145.

On the exact base, a valid Rust crate containing only a function reaches R4 but
fails at R5. `RustSemanticKnowledge::validate` requires at least one additive
R5 entity even though R5 is a declaration extension and the repository has no
field, variant, constant, static, associated type, or implementation-context
method. K1 and R14 consequently expose an internal failure instead of analyzing
the complete inherited graph.

## Decision

Treat the empty additive R5 member layer as a valid neutral element. A selected
`rust-semantic-depth-v1` result may contain zero additive R5 entities,
relationships, and claims when all of these conditions hold:

1. the inherited R4 `CargoManifestKnowledge` is valid under its unchanged
   contract;
2. at least one R5 extraction chunk exists and every chunk remains bound to an
   inherited crate and committed source-file identity;
3. the three additive collections are empty together;
4. both existing R5 index families are empty;
5. deterministic source-bound evidence, diagnostics, and all eight required R5
   capability records remain valid;
6. every supported R5 declaration is still emitted or the existing typed
   oracle fails; and
7. every non-empty R5 result retains all existing ordering, identity,
   reference, evidence, attribute, limit, and index validation.

This removes only the invalid additive-entity non-emptiness invariant. It does
not permit an empty inherited graph, missing chunks, inconsistent empty
collections, dangling references, invalid indexes, omitted supported
declarations, repair, fallback, truncation, or best effort. Downstream layers
may add their own evidence-backed facts but never synthesize an R5 placeholder.

## Compatibility

No public contract, schema, selector, identity domain, semantic hash domain,
fixture, golden, evidence pack, store rule, asset, or viewer byte changes. Every
previously accepted input preserves its exact serialization. The correction
accepts one previously rejected valid additive shape.

Decision 0025 and `tests/specifications/s4/r14/contract-bundle.json` remain
byte-identical under the maintainer's clarification. This decision changes no
R14 expression, binding, access, query, export, explorer, or security meaning.

## Fixture and oracle

The project-owned fixture
`urn:codenoesis:fixture:s4-rust-empty-semantic-extension-v1` is commit
`c780476957a29db6ede1cefb408140763990e829` and tree
`d13008ae7c7dbf9807b9599eb9c1b1213b4b94f4`. It contains one function, two
parameters, two locals, one plain `if`, one assignment, and a tail expression,
with no supported R5 member declaration.

R5 must produce one source chunk, zero additive entities/relationships/claims,
an empty two-family index, and the eight existing capability records. The
unchanged R14 journey must produce 27 entities, 38 relationships, 29 evidence,
65 claims, zero diagnostics, and 15 coverage records, then complete docs,
exact-ID query, export, strict reimport, and explore.

The governance checkpoint precedes every production edit. Its focused E2E must
first retain the reviewed ErrorV21 Red: underlying scan exit `11`, empty stdout,
absent store, 196-byte stderr, and SHA-256
`9b284f4bb7368bb0d11c5b33725c109ee469845aac00081e51175413adec4e3c`.

## Consequences

Ordinary function-only Rust crates can progress through R5, K1, and R14 without
fake declarations. The correction remains deliberately narrow: CFG/data-flow,
types, ownership, runtime behavior, semantic diff, compiler generation, and
cross-language semantics require separate governance.
