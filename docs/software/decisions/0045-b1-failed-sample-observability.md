# Decision 0045: B1 failed-sample observability

- Status: Proposed branch-scoped candidate
- Date: 2026-09-02
- Issue: [#208](https://github.com/smutti/codenoesis/issues/208)
- Authorization: accountable-maintainer authorization in issue #208
- Exact dependent base: `71d05cdb90c3cdd2af4824d5eeb712ec78e6b080`
- Parent packages: issues #205/#206/#207 and Decisions 0042/0043/0044
- Requirements: bounded corrections to Approved `NFR-PER-001`,
  `NFR-OBS-001`, `NFR-TST-001/002`, `NFR-SEC-005`, and `INV-BND-001`
- Slice: `S14`
- Risk: high
- Owner and approver: `@smutti`

## Context

The first B1b Lekton acceptance sample failed before report publication. The
runner correctly retained empty stdout, published no partial report, cleaned
its marker-owned temporary state, and did not retry or discard the failure.
Its public error retained only `benchmark.sample_failed` and a generic message,
so the product exit identity and exact stderr identity were deleted with the
temporary sample state. A separate exact diagnostic succeeded and therefore
did not explain the original nondeterministic observation.

## Decision

For a product sample that exits nonzero or has the wrong success shape, retain
one privacy-safe failure identity in the existing runner V1 error `message`:

- public corpus entry ID and 1-based sample index;
- numeric product exit status, or `signal` when no numeric exit is available;
- exact stdout and stderr byte lengths;
- SHA-256 of the exact stderr bytes;
- product error schema, code, and stage only when stderr is one canonical JSON
  object no larger than 2,048 bytes and every selected value satisfies the
  strict public product-error allowlist; otherwise `unparseable`.

The complete message is at most 256 characters. It never emits product message
or context, source bytes, repository or filesystem paths, host or user values,
environment, command lines, tokens, or URLs. Invalid, oversized, non-UTF-8,
multi-document, and private stderr is hashed but not echoed. Existing invalid
argument, input, timeout, cleanup, mutation, oracle, and internal errors remain
byte-for-byte unchanged.

After Green, run at most five separate sequential exact Lekton diagnostics
with the `cce8486` release binary, fresh stores, the existing 90-second timeout,
and no retry. Stop at the first failure and retain its bounded identity. If all
five complete, retain only that diagnostic fact. Neither outcome authorizes a
new B1 acceptance attempt, RustDesk execution, candidate comparison, policy or
oracle changes, or an SLO claim.

## Compatibility and activation

The runner error schema, code, stage, and exit status remain unchanged. No
report, comparison, corpus, policy, oracle, threshold, timeout, product source,
ontology, schema, golden, dependency, workflow, release, support, or GA
authority changes. This candidate becomes effective only after independent
review and protected manual merge.
