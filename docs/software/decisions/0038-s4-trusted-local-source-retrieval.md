# Decision 0038: R18 trusted local evidence-to-source retrieval

- Status: Proposed branch-scoped candidate
- Date: 2026-08-15
- Issue: [#190](https://github.com/smutti/codenoesis/issues/190)
- Authorization: accountable-maintainer high-risk R18/S4 authorization recorded for the complete issue #190 package
- Exact base: `1de6a420f25a1c7eb74d07a99f1800dde90eefa8`
- Requirements: Proposed `FR-CTX-002`, Proposed `FR-CLI-011`, and the bounded amendments listed in issue #190
- Slice: `S4`
- Risk: high
- Owner and approver: `@smutti`

## Context

Protected PR #189 merged the evidence-complete LocalBaselineVerificationV2
package. Its exact 32-profile evidence pack activates the bounded S0-S7,
R0-R17, K1, and G0-G8-local baseline without changing product or release
bytes. The retained branch manifest remains immutable pre-activation evidence
with status `candidate_verified_pending_merge`; protected manual merge is the
external lifecycle event. Issue #141 is closed as superseded.

R17 groups one exact function or method into `FunctionContextV1`, including
its stable evidence identities and relative Git locators. It deliberately
excludes source text. A user can therefore identify the evidence for a
signature or body fact but cannot ask CodeNoesis to resolve that evidence to
the exact committed bytes and human-readable line and column.

## Decision

Add one output-only command and one explicit selector:

```text
noesis source \
  --repository <local-git-root> \
  --revision <full-lowercase-sha1> \
  --repository-id <identity> \
  --store <local-store-root> \
  --evidence-id <stable-evidence-id> \
  --source-profile trusted-local-source-v1 \
  [--acquisition-profile local-git-sha1-packed-v1] \
  [--repository-boundary-profile local-gitlinks-v1] \
  --format json
```

The command loads one validated visible `RepositorySnapshotV18`, finds one
exact evidence record, and independently reacquires the explicit local Git
repository at the snapshot commit. It reads the immutable Git object selected
by the evidence path and blob OID, never mutable working-tree bytes. Repository
identity, commit, tree, path, blob, and byte span must all agree before source
content enters the result.

Success emits canonical `codenoesis.trusted-source-excerpt/v1` with authority
`explicit_local_git_object_only` and disclosure
`explicit_transient_stdout`. It includes the immutable source binding, the
existing locator, one-based line and Unicode-scalar columns, exact UTF-8 text,
byte length, and SHA-256. It does not change or enrich the ontology, snapshot,
query, function-context, portable graph, explorer, documents, or release
profile.

## Security and privacy boundary

The evidence locator is data, not filesystem authority. The application asks
the existing bounded repository adapter to reacquire the reviewed commit and
then matches the locator against its validated inventory. It never joins an
untrusted evidence path to the worktree. Existing loose and explicitly
selected packed-object validation, path policy, symlink and reparse rejection,
Git-object integrity, and changed-input checks remain authoritative.

Only one non-empty UTF-8 span whose boundaries are UTF-8 scalar boundaries is
eligible. The maximum excerpt is 262,144 bytes and the maximum canonical
stdout, including LF, is 524,288 bytes. There is no path override, context-line
expansion, range override, glob, batch, nested-source traversal, working-tree
fallback, remote acquisition, repair, retry, truncation, or partial result.

Source bytes are classified only as explicit transient local stdout for this
selector. They never enter the store, documents, FunctionContextV1,
LocalQueryResultV1-V13, PortableGraphV1-V9, LocalExplorerV1-V10, retained
operational evidence, telemetry, model input, clipboard, or release artifact.
Errors contain no source text, absolute root, credential, environment value,
or ambient private data.

## Contract and failures

`TrustedSourceExcerptV1` binds:

- repository identity, commit OID, tree OID, snapshot ID and semantic hashes;
- exact evidence ID, repository-relative path, blob OID, and zero-based
  half-open byte span;
- one-based line and Unicode-scalar column positions with a half-open end;
- exact UTF-8 excerpt, byte length, SHA-256, authority, disclosure, and fixed
  limitations.

Invalid arguments, missing evidence, snapshot or repository mismatch, invalid
locator, rejected acquisition or path, non-UTF-8 content, invalid scalar
boundary, empty/reversed/out-of-file span, limit excess, or changed input emits
one strict `CodeNoesisErrorV29`, leaves stdout empty, and creates no side
effect. Caller/input/boundary failures exit `2`; internal contract failure exits
`1`.

## Oracle

The project-owned R17 fixture remains byte-identical and is reused rather than
copied. Its method `scale` exposes signature evidence
`urn:codenoesis:evidence:blake3:3025c0ba243c210e5a923781cf1a57c420e36d393c24b358c1f965594c3002c8`.
The locator selects `src/lib.rs`, blob
`3beb259d89c879b3b16f8d7de20b5f7d23ffcf88`, bytes `[218, 316)`, lines 14:5
through 17:5, and the exact 98-byte reviewed signature excerpt with SHA-256
`2beedeaf7f4333bd21ec5b33de802f1b2006377ad6435ebc983b16029fd19f83`.

The same command must produce byte-identical output for equivalent loose and
packed object databases. Fifty argument permutations and ten process schedules
must be identical. ASCII, multibyte UTF-8, LF, CRLF, maximum and plus-one
boundaries are reviewed. One pinned Lekton evidence locator is replayed twice
without committing external source.

## Verification

The governance checkpoint contains this decision, candidate requirements,
schemas, inherited-fixture descriptor, exact oracle, traceability guard, and
the narrow public acceptance test before production edits. On the exact base,
governance is Green and the public test is Red only because `noesis source` and
`trusted-local-source-v1` do not exist.

Green requires the exact fixture output, loose and packed parity, full binding,
UTF-8 positions, invalid/security/privacy/race/limit/stdout-failure behavior,
50 permutations, ten schedules, one pinned Lekton diagnostic, immutable
R0-R17/K1/S7/G0-G8 bytes, the complete repository gate, independent review,
and protected manual merge.

## Consequences

- Humans and tools can navigate from a FunctionContext evidence identity to
  exact committed source without treating a path as authority.
- Portable and browser artifacts remain source-private and byte-identical.
- No new dependency, migration, control-plane, release, support, or GA
  authority is introduced.
- Merge makes the exact R18 package Approved and Implemented; independent
  acceptance of its retained evidence remains a later verification action.
- Rollback is a complete revert of issue #190.
