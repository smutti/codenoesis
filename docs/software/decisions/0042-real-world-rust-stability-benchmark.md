# Decision 0042: Real-world Rust stability benchmark

- Status: Proposed branch-scoped candidate
- Date: 2026-09-01
- Issue: [#205](https://github.com/smutti/codenoesis/issues/205)
- Authorization: [accountable-maintainer decision](https://github.com/smutti/codenoesis/issues/205#issuecomment-5498148191)
- Exact base: `3fb6504d1d6cb39f204eca032ff816266194e1ec`
- Requirements: Approved `NFR-PER-001` with the bounded evidence constraints in issue #205
- Slice: `S14`
- Risk: high
- Owner and approver: `@smutti`

## Context

LocalBaselineVerificationV3 independently accepted the exact 34-profile local
baseline, including R18 and R19. CodeNoesis already has retained real-repository
observations, but the global benchmark manifest remains a scaffold and no
executable same-host base-versus-candidate protocol exists. Performance claims
therefore cannot yet be reproduced through one stable command.

## Decision

Introduce one observational suite, `rust-real-world-stability-v1`, over two
replaceable pinned public inputs. Caller-supplied full local clones are outside
Git and are never vendored. The runner performs no network acquisition, source
mutation, build, package-manager action, target execution, model call, or
browser action.

Lekton at `247b8f42fb045db41166d70a276a41c2e079b6eb` is the positive complete R16
journey. Every sample must match the accepted RepositorySnapshotV18 semantic
hash, canonical semantic projection digest, and family counts. RustDesk at
`d412d198720aa56f6cfed2dfad262e8fb1322fb7` is the negative journey. Every
sample must preserve the exact ErrorV24 repository-boundary rejection, empty
stdout, absent publication, and unread nested source.

The suite records exactly three baseline and three candidate repetitions for
each entry, raw monotonic nanoseconds, nearest-rank p50/p95/p99, complete sample
outcomes, sanitized host/toolchain/binary/source metadata, corpus and profile
identity, concurrency, cache state, and success rate. Failed samples are never
retried or discarded.

Comparison is allowed only for matching host profile, corpus, configuration,
and policy. Candidate p95 must not exceed `max(base * 1.20, base + 5 seconds)`;
Lekton and RustDesk also retain observational ceilings of 75 and 10 seconds.
Semantic or typed-outcome drift always fails. These values are a B1 regression
policy, not a ratified SLO, release-artifact claim, cross-host claim, support
promise, conference result, or evidence for `NFR-PER-002`.

`benchmarks/manifest.json` becomes active with only `NFR-PER-001`. Its V1
schema remains immutable. The standard-library validator may accept active
status only after proving the exact runner, descriptor, policy, oracle,
required report fields, canonical suite, and observational scope.

## Failure and privacy model

Wrong repository, revision, tree, clone shape, mutable input, symlink or path
substitution, timeout, malformed or oversized report, missing or duplicated
sample, host/config mismatch, semantic/outcome drift, threshold excess, and
privacy canaries fail closed with no partial final report. Retained reports
exclude absolute roots, hostnames, usernames, environments, credentials,
tokens, source bytes, and private URLs.

## Compatibility and activation

No product Rust byte, ontology, schema, fixture, golden, identity, query,
viewer, workflow, dependency, release, or historical evidence changes. The
candidate becomes effective only after retained Red-before-code evidence,
focused and complete Green, real-repository evidence, independent review, and
protected manual merge. Reverting the complete pull request restores the
scaffold manifest and removes the runner without changing product state.
