# Decision 0043: Benchmark-only R16 execution limit

- Status: Proposed branch-scoped candidate
- Date: 2026-09-01
- Issue: [#206](https://github.com/smutti/codenoesis/issues/206)
- Authorization: [accountable-maintainer decision](https://github.com/smutti/codenoesis/issues/206#issuecomment-5499625925)
- Exact dependent base: `74bd959128472c3e95baeb5fd1a29cfc09b2c686`
- Parent package: issue #205 and Decision 0042
- Requirements: bounded amendments to Approved `NFR-PER-001`, `FR-CLI-001`,
  `NFR-DET-001`, `NFR-SEC-005`, `NFR-TST-001/002`, and `INV-BND-001`
- Slice: `S14`
- Risk: high
- Owner and approver: `@smutti`

## Context

The first exact Lekton execution of the B1 real-world Rust suite reached the
whole-scan wall boundary after R16 extraction. The product returned the typed
`scan_wall_milliseconds` limit at the unchanged standard maximum of 60,000
milliseconds. The same pinned input also completed below that boundary in a
diagnostic run, so retrying, discarding the failure, weakening the benchmark,
or globally changing the product limit would be incorrect.

## Decision

Add one explicit operational selector:

```text
--execution-limit-profile real-world-rust-benchmark-75s-v1
```

The selector is accepted only by `noesis scan` with the exact source-only R16
B1 composition: standard local S4, packed local SHA-1 acquisition, the complete
R3-R6/K1/R14-R16 lineage, `local-snapshot-256m-v1`, one explicit store, and JSON
format. For that composition only, the final whole-scan wall maximum is 75,000
milliseconds. Selector absence and every historical invocation retain exactly
60,000 milliseconds. Acquisition traversal retains its existing independent
60,000 millisecond maximum.

The selector is operational evidence configuration. It is excluded from the
semantic configuration, ontology, snapshot and graph hashes, entity and
relationship identities, counts, schemas, goldens, stores, and publication
contracts. Unknown, duplicate, incomplete, misplaced, boundary, cfg, compiler
index, non-packed, or non-256m composition fails before acquisition or
publication through the existing R16 typed input family.

The B1 runner supplies the selector explicitly and records it as sanitized
configuration identity. Its process timeout remains 90 seconds and the Lekton
observational p95 ceiling remains 75 seconds. This is neither an SLO nor a
release, support, availability, cross-host, conference, or GA claim.

## Compatibility and activation

The existing `STANDARD_LOCAL_S1_LIMITS.scan_wall_milliseconds` constant remains
60,000. The RustDesk boundary rejection keeps precedence and exact ErrorV24
bytes. No dependency, workflow, release authority, source-specific semantic
case, ambient override, or hidden bypass is introduced.

This dependent candidate preserves issue #205's checkpoint, Reds, implementation
history, and blocked observation. It becomes effective with B1 only after the
combined independently reviewed pull request is manually merged. Reverting the
B1a commits restores the blocked B1 observation without rewriting prior
evidence.
