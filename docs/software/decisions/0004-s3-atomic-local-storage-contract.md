# Decision 0004: S3 atomic local storage contract

| Field | Value |
|---|---|
| Status | Accepted; authoritative only after the accountable actor manually merges protected PR [#29](https://github.com/smutti/codenoesis/pull/29) |
| Date | 2026-07-26 |
| Scope | `S3 — Atomic local storage` only |
| Product owner | Andrea Moretti — project governance persona represented by [`@smutti`](https://github.com/smutti), not a separate natural person |
| Technical approver | [`@smutti`](https://github.com/smutti) — sole human maintainer under the single-maintainer bootstrap model |
| Risk | High: SQLite schema, durable publication, filesystem CAS, crash consistency, corruption handling, filesystem safety, public errors, and protected oracles |
| Requirements | `FR-STO-001`, `FR-SNP-001`, `INV-SNP-001`, `NFR-REL-001` |
| Issue | [#28](https://github.com/smutti/codenoesis/issues/28) |
| Approval reference | PR [#29](https://github.com/smutti/codenoesis/pull/29); effective only on protected manual merge by `@smutti` |

This record proposes no production implementation by itself. It becomes
authoritative only through a protected manual merge by `@smutti`; the authoring
agent must not approve or merge it. A separate policy-binding change and an
agent-ready implementation issue are required after ratification.

## Context

S0 binds one immutable local Git commit. S1 inventories its committed tree
inside fixed safety limits. S2 parses one bounded Rust library and produces a
validated `RepositorySnapshotV3` with stable graph identities, claims, and
evidence. None of those slices retains the result after the process exits.

S3 must establish the first durable local authority without silently deciding:

1. what identifies a persisted semantic snapshot and a stored byte object;
2. which bytes live in SQLite and which live in the filesystem CAS;
3. where the atomic visibility point occurs across those two media;
4. what a reader validates before reporting the visible head;
5. how process termination, retry, contention, corruption, and cleanup behave;
6. how a user opts into writes without changing S0–S2 behavior;
7. which platform durability claims are actually evidenced;
8. the exact first Red and independently reviewable failpoint oracle.

The architecture already selects SQLite in WAL mode, a single application
writer, and content-addressed local artifacts. This decision makes that local
S3 subset executable. It does not define PostgreSQL, S3-compatible object
storage, jobs, general query behavior, migrations, deletion, or repair.

## Decision

### Explicit S3 operation and compatibility

S3 publication is selected only by:

```text
noesis scan \
  --repository <local-worktree-root> \
  --repository-id <canonical-logical-id> \
  --revision <full-oid-or-refs/heads/main> \
  --profile standard-local-s3 \
  --store <local-store-root> \
  --format json
```

A successful invocation writes the durable store and emits the same strict
`RepositorySnapshotV3` document and one LF that the approved S2 semantic path
would emit for the same repository identity and revision. It writes no stderr
and exits `0`.

`standard-local-s3` is an operational publication profile. The semantic
analysis configuration inside `RepositorySnapshotV3` remains
`standard-local-s2`: storage location, SQLite version, filesystem, and
publication timing do not change the repository knowledge meaning or semantic
hash. The explicit store path is operational input and never enters a semantic
identifier, snapshot semantic hash, graph hash, artifact bytes, or public error
context.

The approved S0 invocation without a profile, `standard-local-s1`, and
`standard-local-s2` retain their V1, V2, and V3 behavior and perform no new
write. Profile selection never depends on repository shape, environment,
existing store discovery, or successful parsing.

S3 introduces no supported `docs`, general `query`, or head-inspection CLI
command. Those public journeys remain S4 under `FR-CLI-001` and `FR-QRY-001`.
S3 head reads are exercised through an inward-owned application/storage
contract and an isolated test-only process probe.

### Project and snapshot identity

The exact canonical `repository_identity` is the local project key. S3 adds no
storage sequence number to semantic identity.

`RepositorySnapshotV3.semantic_hash.value` already commits to repository
identity, immutable commit, inventory, extraction chunks, graph, configuration,
pipeline, ontology, extractor, and evidence-lineage versions. The local
snapshot identifier therefore uses the RFC 8785 preimage:

```text
["codenoesis.snapshot-id/v1",
 repository_snapshot_v3_semantic_hash_value]
```

Its lowercase BLAKE3-256 result has the public form:

```text
urn:codenoesis:snapshot:blake3:<64 lowercase hexadecimal characters>
```

Envelope time, job, and correlation values are excluded. Re-running the same
semantic scan creates no second snapshot.

### Immutable artifact identity and bytes

S3 stages exactly these canonical artifacts:

1. RFC 8785 bytes of `RepositorySnapshotV3.semantic`;
2. RFC 8785 bytes of its `KnowledgeGraphV1`;
3. RFC 8785 bytes of every `ExtractionChunkV1`, in approved chunk order.

For exact artifact bytes `B`, the digest input is:

```text
UTF-8("codenoesis.artifact-id/v1") + 0x00 + B
```

The lowercase BLAKE3-256 result has form:

```text
urn:codenoesis:artifact:blake3:<digest>
```

The object path is:

```text
objects/blake3/<first two digest characters>/<remaining 62 characters>
```

The CAS artifact digest commits to exact bytes. The existing snapshot, graph,
and extraction semantic hashes continue to commit to their versioned semantic
domains. The SQLite artifact manifest stores both where applicable; neither is
substituted for the other.

### Store root and marker

The caller supplies one explicit store root. It must be:

- an absent leaf below a verified existing parent, an empty directory, or an
  exact previously initialized CodeNoesis v1 store;
- disjoint from the repository root in both ancestor directions;
- traversable without symlink or reparse-point components;
- writable without access outside the selected root.

S3 rejects a non-empty unmarked root, overlapping source/store roots, unsafe
path components, an unknown marker, schema drift, and any root whose safety
cannot be established. It performs no destructive repair.

The exact marker bytes are:

```json
{"database":"metadata.sqlite3","objects":"objects","schema_version":"codenoesis.local-store-marker/v1","temporary":"tmp"}
```

followed by one LF. Initialization durably creates the marker, database, object
directories, and temporary directory before publication. Public errors never
contain the store path or another absolute path.

### SQLite v1 authority

The exact fresh-store DDL is
[`local-store-v1.sql`](../../../tests/specifications/s3/local-store-v1.sql).
Its identity is:

- application ID `0x434e4f53` (`CNOS`);
- user version `1`;
- logical schema `codenoesis.local-store/v1`.

Every connection enables:

```text
journal_mode = WAL
synchronous = FULL
foreign_keys = ON
trusted_schema = OFF
busy_timeout = 0
```

There is one application writer using `BEGIN IMMEDIATE`. Writer contention is a
typed retryable result; the adapter does not spin or retry internally.

SQLite stores:

- immutable snapshot and artifact manifests;
- exact canonical extraction chunk bytes and metadata;
- exact canonical entity, relationship, claim, evidence, diagnostic, and
  coverage rows;
- one `project_heads` row per repository identity.

All tables except `project_heads` are append-only in S3 and have database
triggers rejecting update and delete. `project_heads` contains the current
snapshot ID and a monotonically increasing operational generation. Generation
increments only when the visible snapshot changes. Publishing the current head
again leaves it unchanged.

S3 reads and writes only schema v1. Migrations, downgrade, rebuild, deletion,
and automatic repair are deferred and an unknown version is rejected before
mutation.

### CAS staging protocol

For each artifact in canonical role and ordinal order, the filesystem adapter:

1. invokes the test-only `cas_before_temp_create` boundary callback;
2. creates a random temporary file exclusively below `tmp`;
3. writes every byte and rejects short writes;
4. performs the supported file durability operation;
5. invokes `cas_after_temp_sync`;
6. recomputes byte length and artifact digest;
7. atomically moves without replacing into the final digest path;
8. invokes `cas_after_object_move`;
9. performs the supported parent-directory durability operation;
10. invokes `cas_after_parent_sync`;
11. reports the object staged.

If the final object already exists, the adapter verifies exact length and
digest and reuses it. A mismatch is corruption or an implausible digest
collision and fails closed. The adapter never overwrites an existing final
object and never reports staged while a required durability operation remains
unsuccessful.

New shard and object directories are made durable as part of initialization or
first use. A partial temporary object is never addressable from the final
object tree.

### Atomic publication

Publication validates the complete V3 snapshot and stages every CAS artifact
before SQLite may reference one. It then:

1. starts one immediate SQLite transaction;
2. invokes `sqlite_after_begin`;
3. inserts or independently verifies all immutable snapshot, artifact, graph,
   extraction, and manifest rows;
4. invokes `sqlite_after_snapshot_rows`;
5. compares the current project head with the caller's expected head and
   inserts or updates `project_heads`;
6. invokes `sqlite_after_head_update`;
7. commits the transaction durably;
8. invokes `sqlite_after_commit`;
9. returns the unchanged reviewed V3 stdout bytes.

The SQLite commit is the only visibility point. CAS objects staged before a
failed transaction are safe orphans. Uncommitted SQLite rows and head changes
are invisible and roll back during recovery.

A process terminated before `sqlite_after_commit` exposes the previous head.
A process terminated after that boundary exposes the new complete head even if
the caller never received success. On first publication, “previous” means no
head.

The adapter must not convert a commit ambiguity into an old-head success. A new
process reads and validates the authoritative state before retry.

### Failpoint and crash oracle

The exact ordered boundaries and expected A/B outcomes are
[`publication-failpoints-v1.json`](../../../tests/specifications/s3/publication-failpoints-v1.json).
Every metadata boundary and every canonical artifact occurrence of each CAS
boundary is tested for:

- first publication into an empty store;
- replacement of reviewed head A with reviewed head B;
- external process termination without graceful cleanup;
- restart in a distinct process;
- complete head validation before retry;
- retry of the same intended publication;
- orphan sweep and another complete head validation.

No production environment variable, argument, feature, or global failpoint
switch exists. A test probe injects a boundary callback into the inward-owned
publication use case and an external controller terminates the probe only after
observing the exact boundary.

Panic before a boundary, timeout as a boundary signal, in-process rollback
without process termination, hidden retry, skipped cases, or a changed oracle
is not accepted fault evidence.

### Idempotency and contention

The publication idempotency key is:

```text
repository_identity + snapshot_id
```

Publishing the current snapshot verifies all immutable rows and referenced
objects, leaves the head generation unchanged, and returns the same domain
result. The property oracle repeats the operation 100 times and permits one
snapshot row set and one artifact-reference set only.

Publishing a different snapshot uses compare-and-set against the expected
head. Another writer that commits first causes
`publication.head_conflict`; the losing writer never overwrites that head.
`storage.writer_busy` and `publication.head_conflict` are the only retryable S3
errors. Caller retry is bounded and always observes state first.

### Head reads and corruption

A head read uses one stable SQLite read transaction and validates, in order:

1. the project head row;
2. its snapshot metadata;
3. ordered artifact references;
4. immutable graph rows and cross-references;
5. every referenced CAS object's existence;
6. exact byte length;
7. artifact digest;
8. public snapshot, graph, and extraction semantic hashes.

Success returns one strict
[`LocalSnapshotHeadV1`](../../../tests/specifications/s3/local-snapshot-head-v1.schema.json).
Missing, malformed, dangling, wrong-length, wrong-digest, or wrong-schema
reachable content returns one typed error. The reader never returns degraded
success, combines revisions, repairs content, or falls back to an older head.

### Orphan cleanup

S3 cleanup may remove:

- abandoned files below `tmp`;
- final CAS objects absent from every committed `snapshot_artifacts` row.

It obtains one stable committed reachability view and rechecks absence before
each final deletion. It never deletes an object referenced by any committed
snapshot, including a non-head snapshot. Corrupt reachable content is reported
and preserved for explicit human recovery.

S3 has no snapshot deletion, retention, automatic background garbage
collection, or database-row cleanup. Those require later policy and migration
decisions.

### Ports and dependency direction

Domain and application policy remain independent of SQLite, filesystem APIs,
async runtimes, and platform SDKs. Inward-owned contracts cover:

- immutable artifact stage/read/verify/sweep behavior;
- metadata transaction and head compare/set behavior;
- the atomic publication use case and complete head read.

The deterministic fake and real SQLite/filesystem adapters execute the same
behavioral vectors. Adapter errors are mapped to typed inward-owned errors;
interfaces contain no storage business logic. Concrete dependency versions and
the smallest required crate boundary are authorized only by the later Ready
implementation issue.

### Error contract and precedence

S3 failures emit one strict
[`CodeNoesisErrorV4`](../../../tests/specifications/s3/codenoesis-error-v4.schema.json)
and LF on stderr with empty stdout.

Exit classes are:

| Exit | Meaning |
|---:|---|
| `0` | V3 snapshot generated, atomically published, and emitted |
| `2` | Invalid invocation or store argument |
| `10` | Inherited acquisition or repository-policy failure |
| `11` | Inherited extraction, ontology, or graph failure |
| `12` | Store-open, integrity, CAS, transaction, head, or publication failure |
| `70` | Unexpected internal failure |

Stable S3 codes are:

```text
input.invalid_store_root
storage.unmarked_nonempty_root
storage.incompatible_schema
storage.writer_busy
storage.missing_object
storage.corrupt_object
storage.corrupt_metadata
storage.unsafe_path
publication.head_conflict
publication.failed
```

Failure precedence is input/profile, acquisition, extraction/graph validation,
store marker/schema/path validation, CAS staging in canonical artifact order,
SQLite transaction, visible-head validation, then stdout serialization. Within
one class, canonical byte order selects the first failure.

Errors contain bounded logical IDs, versions, digest values, counts, and
relative source paths only. They contain no store root, absolute path, object
bytes, source excerpt, SQL, environment data, or platform secret.

### Platform durability claims

Process-crash/restart behavior is required on Linux, macOS, and Windows. Every
evidence record names OS, filesystem, SQLite version and compile options,
file-sync primitive, atomic-move primitive, parent-directory durability
primitive, and process-termination primitive.

Power-loss durability is claimed only on a platform/filesystem combination for
which both the SQLite commit and CAS file/directory durability operations
succeed and are named. An unsupported parent durability primitive causes typed
publication failure before the head can reference that object. A process-crash
test is not mislabeled as power-loss evidence.

### Fixture, expected Red, and retained evidence

The reviewed fixture is
[`tests/fixtures/s3/atomic-local-storage-v1/`](../../../tests/fixtures/s3/atomic-local-storage-v1/).
It reuses the exact S2 Rust tree and creates two commits:

- A: the approved S2 fixture commit;
- B: the same tree with parent A and different deterministic commit metadata.

This isolates publication from parser changes while producing distinct V3
semantic hashes and snapshot IDs. The goldens bind exact semantic payloads,
heads, artifacts, crash recovery, idempotency, cleanup, corruption, schema, and
unsafe-path behavior.

The machine oracle is
[`e2e_fr_sto_001_atomic_local_storage.json`](../../../tests/specifications/s3/e2e_fr_sto_001_atomic_local_storage.json).
The first implementation test is `e2e_fr_sto_001_atomic_local_storage`, run by:

```text
cargo test --test e2e_fr_sto_001_atomic_local_storage
```

Before S3 production changes, merged S2 routes an unknown explicit profile to
the S1 parser and returns exit `2`, empty stdout, and
`CodeNoesisErrorV2/input.invalid_profile` for `standard-local-s3`. The S3
harness expects exit `0`, empty stderr, reviewed V3 stdout, and a complete
persisted head after restart; therefore it is Red for missing S3 behavior.

Compilation failure, missing or corrupt fixture, schema or DDL failure,
dependency or network outage, probe failure outside a named boundary, timeout,
race, panic, or modified oracle is not acceptable Red evidence.

The implementation change retains immutable Red before production edits, then
Green results for all S3 scenarios and inherited S0–S2 regressions. Evidence
includes exact SHAs, commands, first-run exits and logs, database and object
digests, SQLite settings, failpoint traces, platform primitives, write-set
audits, canaries, environment, agent identity, duration, cost, limitations, and
human approvals.

## Consequences

- SQLite commit is the only visible-head transition; cross-medium atomicity
  does not pretend to be a distributed transaction.
- Staging CAS first turns pre-commit crashes into safe orphans rather than
  dangling visible references.
- V3 semantic bytes remain independent of storage location and runtime.
- Strict fresh-store DDL and immutable-row triggers make accidental mutation
  observable before general query or migration work exists.
- Fail-closed reads make corruption visible instead of silently serving stale
  or mixed knowledge.
- Honest platform evidence separates process restart from power-loss claims.

## Deferred

- public `docs`, `query`, and head CLI commands;
- SQLite FTS5 and indexed graph-query projections;
- schema migration, downgrade, rebuild, and destructive repair;
- snapshot/project deletion, retention, legal hold, and automatic GC;
- backup, restore, and power-cut certification beyond retained named evidence;
- multiple concurrent application writers and distributed coordination;
- PostgreSQL, S3-compatible object storage, jobs, leases, outbox, REST, MCP,
  authentication, and tenant isolation;
- incremental refresh, documentation, federation, impact, plugins, and models.

## Ratification and delivery sequence

1. `@smutti` reviews the exact identities, DDL, schemas, fixture, goldens,
   failpoints, durability claims, errors, and deferrals.
2. `@smutti` manually squash-merges protected PR #29; the merge is the approval
   event under the disclosed single-maintainer bootstrap.
3. A separate critical-risk PR binds exactly the S3 requirement set to the
   byte-identical SRS revision in repository policy.
4. A separate Ready implementation issue fixes dependency versions, crate and
   adapter paths, expected Red, evidence, risk, and stop conditions.
5. Production work starts with retained expected Red and may not edit this
   contract bundle.

The authoring agent must not approve or merge any step.
