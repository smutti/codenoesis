# Decision 0020: S4 R9 K1 output-capacity contract

| Field | Value |
|---|---|
| Status | Proposed; effective only after protected manual merge |
| Issue | [#148](https://github.com/smutti/codenoesis/issues/148) |
| Exact base | `aadd065defba2d4f8d202583c7da9ff70e92ece8` |
| Requirements | Bounded amendments to `FR-EXT-012`, `FR-CLI-001`, `INV-BND-001`, `NFR-DET-001`, `NFR-TST-001`, and `NFR-TST-002`; K1 subset of `OD-LIM-001` |
| Slice | `S4` |
| Risk | High — public CLI/resource envelope, serialization, storage ordering, and real-repository evidence |
| Owner/approver | `@smutti` |
| Human authorization | Complete high-risk issue #148 package on the exact base above |
| Dependencies | None |
| Correction budget | Five rounds |
| Rollback | Revert the package; no migration or historical rewrite |

## Context

The immutable standard local profile caps canonical output at `33,554,432`
bytes including LF. K1 correctly reaches that bound on pinned Lekton revision
`247b8f42fb045db41166d70a276a41c2e079b6eb`: standard execution reports
`canonical_output_bytes`, maximum `33,554,432`, observed `33,554,433`, empty
stdout, and no visible head.

A diagnostic-only run with a temporary 256 MiB ceiling completed the same K1
snapshot at `53,031,841` bytes in 45.12 seconds. Raising the standard profile
would silently broaden every earlier slice. Inferring a larger limit from
repository shape or a failed first attempt would make resource authority
non-deterministic. Neither is acceptable.

## Decision

Add exactly one explicit selector:

```text
--output-capacity-profile local-snapshot-64m-v1
```

It is valid only for `scan --profile standard-local-s4` with the complete
K1 source composition ending in
`--rust-callable-profile rust-callable-semantics-v1`. It raises only the final
canonical RepositorySnapshotV11 serialization maximum from `33,554,432` to
`67,108,864` bytes including LF.

The selector is versioned, additive, and non-semantic. It is never inferred
from repository bytes, graph size, output size, failure, stored head, host,
memory, or timing. Selector absence is the immutable standard behavior.

## Closed capacity model

| Envelope | Selector | Maximum including LF |
|---|---|---:|
| Standard local | absent | `33,554,432` |
| K1 large local snapshot v1 | `local-snapshot-64m-v1` | `67,108,864` |

The serializer reserves one byte for LF, writes canonical JSON through a
bounded writer, and reports at most maximum plus one. An exact-maximum output
succeeds. An output requiring one or more additional bytes fails with typed
`canonical_output_bytes`, the selected maximum, and observed maximum plus one.
The complete bytes exist before any store root is created or publication is
attempted.

## Semantic invariance

The selector is an invocation resource envelope and does not enter:

- RepositorySnapshotV11 semantic or configuration bytes;
- snapshot, graph, extraction, entity, relationship, claim, evidence,
  diagnostic, coverage, document, portable, or explorer identities/hashes;
- extraction, repository, file, graph, query, docs, portable, explorer,
  process, network, memory, or wall-time limits;
- any schema, ontology catalog, error version, fixture, or golden.

For one already-built V11 value, standard and large envelopes produce exactly
the same bytes whenever the result fits the standard maximum. Existing K1 and
R0-R8 selector-absent commands remain failure-for-failure and byte-for-byte
unchanged.

## CLI and failure semantics

The selector is accepted once and only once in the complete K1 scan
composition. Unknown values, duplicates, a missing value, missing K1 lineage,
R7 compiler-index or repository-boundary composition, and use with docs,
query, export, or explore fail through existing ErrorV16
`input.unsupported_rust_callable_composition` behavior. They produce empty
stdout and cannot create or move a store head.

The standard 32 MiB overflow and selected 64 MiB overflow continue through the
existing typed acquisition-limit mapping. ErrorV16 and every historical error
schema remain byte-identical.

## Publication ordering

The current K1 journey builds the V11 snapshot in confined read-only analysis,
enforces the 60-second scan bound, serializes final stdout, and only then
prepares or opens the local store and invokes immutable publication. The new
envelope is passed only to that pre-publication serialization step. No
application or storage API change is required.

## Acceptance

The public oracle is
`tests/specifications/s4/r9-output-capacity/output-capacity-profile-v1.json`.
It requires:

1. exact 32 MiB and 64 MiB maximum/plus-one behavior;
2. identical small V11 bytes and semantic/configuration payloads;
3. strict invalid-composition ErrorV16 behavior with no store mutation;
4. standard K1 and R0-R8 selector-absent regressions unchanged;
5. serialization completion before publication;
6. two pinned Lekton runs with identical `53,031,841`-byte output, exit zero,
   and one complete visible head each;
7. docs, exact-ID query, PortableGraphV2 export, and LocalExplorerV2 generation
   from the resulting head;
8. no target, compiler, Cargo, build-script, process, network, or model
   execution and no external source vendoring;
9. retained Red, focused Green, and the complete repository gate.

## Expected Red and evidence

Governance and executable tests are committed before production Rust. On the
exact base, the contract test does not compile because V11 exposes only its
inherited standard serializer, and the CLI E2E fails because
`--output-capacity-profile` is rejected before K1 execution. Both failures are
retained with commands, logs, digests, environment, and checkpoint SHA.

Pinned Lekton remains external, replaceable public evidence. The repository is
not copied into this project. Evidence records repository/revision/tree,
commands, output/store digests, byte counts, timings, toolchain, platform,
failure state, and known limitations without source contents or secrets.

## Consequences and rollback

The explicit envelope makes a real K1 ontology observable without weakening
the standard profile or changing knowledge semantics. It does not solve K1/R7
composition, expression semantics, CFG/data flow, constant evaluation, trusted
source snippets, LLM projection, or broader R9 conference evaluation.

No dependency or migration is introduced. Reverting the package removes the
selector and restores the sole 32 MiB serializer. Historical heads and
artifacts remain valid. The authoring agent cannot approve or merge this
high-risk change.
