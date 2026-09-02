# Decision 0044: B1 bootstrap baseline and identity oracle

- Status: Proposed branch-scoped candidate
- Date: 2026-09-02
- Issue: [#207](https://github.com/smutti/codenoesis/issues/207)
- Authorization: [accountable-maintainer decision](https://github.com/smutti/codenoesis/issues/207#issuecomment-5508518302)
- Exact dependent base: `7a7c6601981f915496a4648867323bbcfc91daac`
- Parent packages: issues #205/#206 and Decisions 0042/0043
- Requirements: bounded corrections to Approved `NFR-PER-001`,
  `NFR-DET-001`, `NFR-TST-001/002`, `NFR-SEC-005`, and `INV-BND-001`
- Slice: `S14`
- Risk: high
- Owner and approver: `@smutti`

## Context

The B1a implementation proved that the exact benchmark-only 75-second selector
completes the pinned Lekton scan while preserving all reviewed graph-family
counts. It also exposed two independent contradictions in the inherited B1
contract.

The V3 project baseline `3fb6504d1d6cb39f204eca032ff816266194e1ec`
predates the selector and rejects it before acquisition. It cannot honestly
produce a report under the same execution configuration as a B1a candidate.
Separately, the frozen Lekton hashes came from a historical pilot using
`urn:codenoesis:pilot:lekton`, while the active corpus selects
`urn:codenoesis:benchmark:lekton:b1`. Repository identity is intentionally part
of semantic identity, so equal counts with different hashes are correct.

## Decision

Use `cce84869430ef129f55591998b30ea2ea728e1c3`, the first commit whose Rust
product supports the exact B1a selector, as the B1 V1 bootstrap baseline. A
baseline release must be built from that exact commit. A candidate release must
be rebuilt from the final review head while every Rust product source byte
remains identical to `cce8486`.

For the unchanged B1 corpus repository identity, revision, tree, and selector
matrix, correct only the two identity-bound Lekton values:

- semantic hash:
  `22e32d20429d510d4674e0e6bdc5542f08dbc0e28874cd0098419e7512a334c1`;
- canonical semantic projection SHA-256:
  `7c800424b3176c96d4ea4164d4066adaf551134b3aea4b40a1e5647f74dc7fa9`.

The snapshot remains V18 and the counts remain 26,158 entities, 43,683
relationships, 69,841 claims, 24,522 evidence, 4,211 diagnostics, 13,177
coverage records, and 8 evaluated values. RustDesk retains the exact ErrorV24
repository-boundary rejection.

Three baseline and three candidate repetitions per entry remain mandatory.
Reports stay separate and compare under the unchanged same-host, percentile,
ratio, additive, absolute-ceiling, privacy, timeout, no-retry, no-network, and
no-target-execution policy. This correction makes no SLO, release, support,
availability, cross-host, conference, or GA claim.

## Compatibility and activation

No Rust product byte, selector, limit, ontology family, identity algorithm,
schema, count, corpus source, policy, fixture, golden, dependency, workflow, or
release authority changes. The old base rejection and pilot-identity mismatch
remain retained correction evidence. B1/B1a/B1b become effective together only
after independent review and protected manual merge.
