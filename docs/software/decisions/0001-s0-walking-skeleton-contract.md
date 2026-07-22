# Decision 0001: S0 walking-skeleton contract

| Field | Value |
|---|---|
| Status | Accepted; authoritative after the protected manual merge of PR [#8](https://github.com/smutti/codenoesis/pull/8) |
| Date | 2026-07-18 |
| Scope | `S0 — Walking skeleton` only |
| Product owner | Andrea Moretti — project governance persona represented by [`@smutti`](https://github.com/smutti), not a separate natural person |
| Technical approver | [`@smutti`](https://github.com/smutti) — sole human maintainer under the single-maintainer bootstrap model |
| Repository license | Apache License 2.0 under the repository-wide root [LICENSE](../../../LICENSE) file |
| Approval reference | PR [#8](https://github.com/smutti/codenoesis/pull/8); approval becomes authoritative when `@smutti` manually merges the final protected head |
| Requirements | `DR-ART-001/002`, `FR-ACQ-001`, `FR-CLI-003`, `NFR-DET-001`, `NFR-MNT-001`, `NFR-SEC-005`, `NFR-TST-001/002` |
| Issue | [#6](https://github.com/smutti/codenoesis/issues/6); strict internal-failure correction [#11](https://github.com/smutti/codenoesis/issues/11) |
| Amendment approval | `@smutti` explicitly approved `internal.unexpected` on 2026-07-22; the correction becomes authoritative only after its protected manual merge |

This record contains no production implementation authority. The disclosed
single-maintainer decision accepts the contract, but it becomes authoritative
on `main` only when `@smutti` manually squash-merges the final protected head of
PR #8 after all mandatory checks are green. The authoring agent never merges.
It cannot substitute an automated approval for the accountable human act.

## Context

S0 needs one public, deterministic path that is small enough to drive outside-in
TDD: bind a local Git repository to an immutable commit and emit a versioned
snapshot envelope. Four details were previously ambiguous:

1. local paths are acquisition locators, not stable repository identities;
2. an envelope containing time and job metadata cannot be wholly byte-identical
   across runs while `DR-ART-002` excludes those values from semantic identity;
3. branch movement cannot be tested reliably with timing races;
4. the original S0 requirement list did not trace the CLI or the zero
   target-process/network exit gate.

This decision resolves only the narrow local Git subset needed by S0. It does
not decide the remote and advanced Git questions tracked by `OD-GIT-001`.

## Decision

### Public command and stream contract

The S0 JSON journey is:

```text
noesis scan \
  --repository <local-worktree-root> \
  --repository-id <canonical-logical-id> \
  --revision <full-oid-or-refs/heads/main> \
  --format json
```

- Success returns exit `0`, exactly one `RepositorySnapshotV1` JSON document
  followed by one LF on stdout, and no stderr output.
- Invalid invocation returns exit `2` and a `CodeNoesisErrorV1` on stderr.
- A typed acquisition failure returns exit `10`, no stdout, and exactly one
  `CodeNoesisErrorV1` followed by one LF on stderr.
- An unexpected internal failure returns exit `70`, no stdout, and exactly one
  strict `CodeNoesisErrorV1` followed by one LF on stderr; it must not emit a
  partial snapshot.
- Human-readable output, remote sources, configuration precedence, and the full
  long-term CLI compatibility policy remain outside S0.

### Repository identity and supported Git shape

`--repository-id` is required. S0 accepts the deliberately narrow canonical
form `^urn:codenoesis:[a-z0-9][a-z0-9._:-]{0,254}$`; the fixture uses
`urn:codenoesis:fixture:s0-one-file-v1`. The validated ASCII bytes are the
logical identity and no normalization is performed. A broader URI identity
scheme requires a later version. The absolute path, hostname, user name,
temporary directory, and Git remote are excluded from semantic content.

S0 supports only:

- a local non-bare Git worktree whose root is passed explicitly;
- SHA-1 object format;
- a full 40-character lowercase commit OID or the literal ref
  `refs/heads/main`; every other revision spelling is invalid S0 input;
- exactly one root-level regular `100644` file whose UTF-8 path matches
  `^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$`;
- repository objects read directly through an in-process Rust adapter.

The committed tree is authoritative. Dirty and untracked worktree content is
ignored. Abbreviated OIDs, symbolic `HEAD`, every other ref spelling, tags,
SHA-256 repositories, bare or shallow repositories, alternates, replace
objects, grafts, LFS materialization, submodules/gitlinks, symlinks, remote
fetch, multiple files, subdirectories, and history-rewrite policy are
explicitly deferred. Encountering a deferred shape produces a typed error; it
must not trigger fallback execution.

### Immutable revision binding

The adapter resolves the supplied OID or ref exactly once. It then verifies:

1. the resolved full OID exists;
2. the object hashes to that OID and is a commit;
3. its referenced root tree exists, hashes correctly, and is a tree;
4. the root tree has exactly the supported one-file shape;
5. the referenced blob exists, hashes to its OID, and is a blob.

This is a complete traversal of every commit → root-tree → blob object reachable
in the supported S0 shape. An implementation may not stop after reading the
commit and tree or skip blob verification merely because file content is not
yet emitted in the snapshot.

The resulting repository identity, object format, commit OID, and tree OID form
an immutable `BoundRevision`. All subsequent S0 work consumes that value and
must not reread the ref. Moving `refs/heads/main` from commit A to B after the
binding therefore cannot change the in-flight result; a new scan resolves B.

### `RepositorySnapshotV1`

The minimal successful JSON shape is:

```json
{
  "schema_version": "codenoesis.repository-snapshot/v1",
  "semantic_hash": {
    "algorithm": "blake3-256",
    "value": "<64 lowercase hexadecimal characters>"
  },
  "semantic": {
    "repository": {
      "contract_version": "codenoesis.repository/v1",
      "identity_schema_version": "codenoesis.repository-identity/v1",
      "identity": "urn:codenoesis:fixture:s0-one-file-v1",
      "vcs": "git",
      "object_format": "sha1",
      "commit_oid": "<40 lowercase hexadecimal characters>",
      "tree_oid": "<40 lowercase hexadecimal characters>"
    },
    "configuration": {
      "schema_version": "codenoesis.configuration/v1",
      "profile": "standard-local-s0",
      "semantic_hash": {
        "algorithm": "blake3-256",
        "value": "<64 lowercase hexadecimal characters>"
      }
    },
    "pipeline_version": "codenoesis.pipeline/s0-v1",
    "ontology_version": "codenoesis.ontology/none-v1",
    "extractor_contract_version": "codenoesis.extraction/v1",
    "extractor_versions": [],
    "evidence_lineage_version": "codenoesis.evidence-lineage/v1"
  },
  "envelope": {
    "created_at": "<RFC 3339 UTC timestamp>",
    "job_id": null,
    "correlation_id": "<opaque non-secret identifier>"
  }
}
```

The exact strict Draft 2020-12 schema is
[`repository-snapshot-v1.schema.json`](../../../tests/specifications/s0/repository-snapshot-v1.schema.json).
Required properties may not be omitted and unknown properties are rejected. S0
represents absent ontology and extractor work with explicit version values and
an empty extractor list rather than omitting version lineage. The source locator
is operational input and does not appear in the artifact.

S0 has one fixed configuration semantic value:
`{"profile":"standard-local-s0"}`. Its `configuration.semantic_hash` uses
the same RFC 8785 and BLAKE3-256 rules with domain-separation prefix
`codenoesis.configuration.semantic.v1`, a `0x00` separator, and the canonical
bytes of that object. Its reviewed digest is
`4811a917bebed264f49382d65825686ad5ca506ce39bc51385e547b0c7ced1c0`.
No environment variable or user configuration changes this value in S0; adding
configurable analysis inputs requires a later contract version.

### Canonical bytes and semantic hash

`semantic` is canonicalized using RFC 8785 JSON Canonicalization Scheme. S0
semantic values contain no floating-point numbers. The hash input is:

```text
UTF-8("codenoesis.repository-snapshot.semantic.v1")
+ 0x00
+ RFC8785(semantic)
```

`semantic_hash.value` is the lowercase hexadecimal BLAKE3-256 digest of those
bytes. The domain-separation prefix binds the digest to this artifact contract.
There is no BOM or trailing LF in the hash input.

For fixture commit A, the reviewed RFC 8785 bytes are stored in
[`expected-semantic-a.jcs`](../../../tests/fixtures/s0/one-file-v1/expected-semantic-a.jcs)
with one repository-storage LF excluded from the hash input. Their required
snapshot digest is
`b673624a329f43fd84852bbdeefd66326a7fcb1c03fdb626e2de6bfedff11997`.
The complete fixed-envelope golden is
[`expected-snapshot-a.json`](../../../tests/fixtures/s0/one-file-v1/expected-snapshot-a.json),
and its exact RFC 8785 stdout bytes plus one LF are in
[`expected-snapshot-a.jcs`](../../../tests/fixtures/s0/one-file-v1/expected-snapshot-a.jcs).

The complete stdout document is also serialized with RFC 8785 and ends in one
LF. With all ports fixed, it is byte-identical across runs. When
`created_at`, `job_id`, or `correlation_id` changes, differences are permitted
only under `envelope`; the RFC 8785 bytes of `semantic` and `semantic_hash` must
remain identical. This is the meaning of the S0 canonical-byte exit gate.

### Typed failures

`CodeNoesisErrorV1` has the following minimal shape:

```json
{
  "schema_version": "codenoesis.error/v1",
  "code": "acquisition.object_missing",
  "stage": "acquisition",
  "message": "<human-readable non-secret summary>",
  "retryable": false,
  "context": {
    "object_oid": "<full OID when known>",
    "expected_kind": "<commit|tree|blob when known>",
    "referenced_by": "<full OID when known>"
  }
}
```

The stable S0 codes are:

| Code | Condition |
|---|---|
| `input.invalid_repository_identity` | Repository identity is missing or non-canonical; invocation exit `2` |
| `input.invalid_revision` | Revision syntax is unsupported or ambiguous; invocation exit `2` |
| `acquisition.not_git_repository` | The supplied root is not a supported Git worktree |
| `acquisition.revision_not_found` | The supplied ref or full OID cannot be resolved |
| `acquisition.revision_not_commit` | The resolved object is not a commit |
| `acquisition.object_missing` | A referenced commit, tree, or blob is absent |
| `acquisition.repository_inconsistent` | Decompression, object framing, type, or hash verification fails |
| `acquisition.unsupported_repository_shape` | A valid repository uses a Git feature outside the S0 subset |
| `internal.unexpected` | An unexpected failure prevents completion outside the typed input and acquisition catalog |

The exact strict error schema is
[`codenoesis-error-v1.schema.json`](../../../tests/specifications/s0/codenoesis-error-v1.schema.json),
with a reviewed non-Git golden in
[`expected-error-not-git.json`](../../../tests/fixtures/s0/one-file-v1/expected-error-not-git.json).
The machine contract is the code plus typed context, not the human message.
`context` is a strict code-specific object: fields that do not apply are
omitted, and unknown fields are rejected. It must not contain an absolute path,
hostname, environment value, secret, or raw untrusted content. All acquisition
failures are non-retryable in S0 and produce no artifact.

The strict `internal.unexpected` form uses stage `internal`, the generic message
`unexpected internal failure`, `retryable: false`, and an empty context object.
It must not expose the underlying internal error, an absolute path, raw
untrusted content, environment state, or a secret. Existing input and
acquisition failures retain their ratified exit codes and may not be relabeled
as internal failures.

### Zero target process and zero analysis network

Fixture creation occurs before the monitored scan boundary. From `noesis`
process start until exit:

- no target-controlled hook, executable, filter, helper, or Git command may
  execute;
- after the one initial `noesis` image load, `execve`, `execveat`, `fork`,
  `vfork`, and `clone3` are forbidden; `clone` is forbidden unless its decoded
  flags include `CLONE_THREAD` (classic seccomp cannot safely inspect the
  structure referenced by `clone3`, so S0 denies it entirely);
- the harness launches `noesis` in a fresh Linux network namespace with no
  interface up and no route, closes every inherited descriptor above `2`, and
  verifies that descriptors `0`, `1`, and `2` are not sockets;
- a deny-and-audit seccomp policy forbids every socket creation and network I/O
  syscall, including `socket`, `socketpair`, `socketcall`, `connect`, `bind`,
  `listen`, `accept`, `accept4`, `sendto`, `sendmsg`, `sendmmsg`, `recvfrom`,
  `recvmsg`, `recvmmsg`, and `shutdown`; this includes `AF_UNIX` so a resolver,
  credential, or remote-Git broker cannot be reached indirectly;
- `io_uring_setup`, `io_uring_enter`, and `io_uring_register` are also forbidden,
  preventing async socket creation or network I/O from bypassing the syscall
  gate;
- no DNS or remote Git operation may be attempted, and the network namespace
  remains without an interface or route for the full boundary;
- repository configuration, hooks, remotes, attributes, and source text are
  untrusted data and cannot grant a capability.

The normative S0 security evidence runs on Linux under both the isolated
network namespace and a process/network seccomp deny-and-audit observer. It
records syscall name, decoded arguments, PID/TID, and result for the complete
boundary, plus the namespace interface/route inventory, seccomp profile digest,
and inherited-descriptor audit. The exact normalized policy is
[`seccomp-capability-deny-v1.json`](../../../tests/specifications/s0/seccomp-capability-deny-v1.json)
(SHA-256 `5664635f8ad76dff5421f5eeb1f20ffdf0450203d8cb9c692606c026a39ee1ad`).
Before the product check, the harness generates one isolated probe for every
named syscall and conditional branch in that policy. Each applicable deny probe
must return `EPERM` and a matching observer event; `clone` with `CLONE_THREAD`
must not match the deny rule. A syscall may be marked `not_exposed` only when
libseccomp resolution and a direct `ENOSYS` probe agree for the selected
architecture. Missing policy coverage fails the gate.

A syscall observer without enforcement, a stripped `PATH`, invalid proxy, or
successful offline run alone is not sufficient evidence. The derived fixture
variant installs an executable
`post-checkout` sentinel hook and a loopback HTTPS remote after base fixture
materialization; neither may be used. Other supported platforms may run
functional smoke tests, but they cannot replace this evidence.

### Minimal modular boundary

The future implementation task may introduce only the crates needed by S0:

```text
codenoesis-domain <- codenoesis-contracts
codenoesis-domain <- codenoesis-ports
codenoesis-domain + codenoesis-contracts + codenoesis-ports
  <- codenoesis-application
codenoesis-domain + codenoesis-ports <- codenoesis-repository
all composition dependencies <- noesis
```

Domain and port crates do not depend on filesystem, async runtime, CLI, or Git
libraries. The repository adapter performs local Git I/O; the binary performs
composition and presentation only. Every first-party crate inherits
`unsafe_code = "forbid"`. A machine-readable architecture fitness check must
reject outward dependency edges and missing lint inheritance.

## Fixture and Red oracle

The versioned synthetic fixture is
[`tests/fixtures/s0/one-file-v1`](../../../tests/fixtures/s0/one-file-v1/README.md).
Its manifest fixes source bytes, file mode, author/committer identity, timestamps,
timezone, messages, parents, and expected blob/tree/commit OIDs for commits A
and B. Each commit contains exactly one regular file. The test materializes a
fresh repository per case with global Git configuration and hooks disabled. The
separate isolation variant then installs the manifest's exact hook and remote;
that mutation does not add a committed file or change commit A.

The machine-readable oracle is
[`e2e_fr_acq_001_immutable_commit.json`](../../../tests/specifications/s0/e2e_fr_acq_001_immutable_commit.json).
The first future TDD commit must add the black-box harness and a minimal `noesis`
surface that exits `70` with test-only stderr marker `S0_NOT_IMPLEMENTED`. That
marker is deliberately outside the public `CodeNoesisErrorV1` catalog and is
removed by the implementation; it cannot become a compatibility contract. The
Cargo test runner must exit nonzero because its assertion expected subject exit
`0` and `RepositorySnapshotV1`. The expected Red is that exact mismatch, not a
compile error, missing fixture, absent test target, network dependency, or
nondeterministic race. The evidence record keeps runner and subject exit codes
separate and retains the assertion and log digest before production behavior is
added.

The public black-box test covers commit A output and typed non-Git failure. A
controlled integration test uses the real local Git adapter and object database:
it binds A, the harness updates the real ref to B, and it then verifies that the
existing `BoundRevision(A)` still yields A. A fake port may pass the same
contract but cannot replace this adapter test. This seam avoids a timing race in
the CLI test while preserving the observable new-scan-on-B behavior.

Red/Green observations do not mutate the oracle. They are stored separately in
an instance of
[`evidence-manifest-v1.schema.json`](../../../tests/specifications/s0/evidence-manifest-v1.schema.json),
which binds requirement IDs, base/head/pre-implementation commits, the immutable
oracle-bundle digest, runner and subject exit codes, timestamps, and log/report
digests. Green and Verified evidence must contain one ordered passing result for
every test in the acceptance specification. Each result, the Red log, the
aggregate report, and the seccomp self-test report carry a resolvable immutable
GitHub Actions locator: repository, run ID, artifact ID, path, SHA-256, and
retention deadline. The evidence validator retrieves each artifact, recomputes
every digest, and rejects evidence that is already expired or expires before
the project's evidence-retention requirement. A stale oracle digest, missing
Red link, incomplete result set, duplicate result, unresolved locator, expired
artifact, or digest mismatch is rejected.

The ratified files are enumerated by
[`contract-bundle.json`](../../../tests/specifications/s0/contract-bundle.json).
Its `bundle_sha256` is SHA-256 over RFC 8785 canonical bytes of the manifest
payload containing `schema_version` and the path-sorted file/hash list, excluding
the digest field itself. The SRS carries that root digest, so changing an ADR,
schema, oracle, fixture, golden, or guard requires an SRS change and invalidates
the previous policy `source_sha`.

## Execution and evidence budgets

These are S0 implementation-task and acceptance-harness budgets, not general
repository limits; `OD-LIM-001` remains open for S1 and later slices.

| Budget | Limit |
|---|---:|
| Fixture files per materialized revision | 1 |
| Fixture repository bytes excluding `.git` | 4 KiB |
| One scan wall time in CI | 5 seconds |
| Full S0 acceptance suite wall time | 180 seconds |
| Scan stdout plus stderr | 1 MiB |
| Temporary disk per test process | 32 MiB |
| Peak resident memory target | 256 MiB |
| Determinism replays | 50 isolated runs with recorded seed |
| Parallel-safety repetitions | 10 shuffled runs with no retry-to-green |
| Raw retained Red/Green logs | 10 MiB per run after redaction |
| Machine-readable evidence manifest | 1 MiB |
| Resolvable evidence retention from `observed_at` | 90 days minimum |
| Autonomous correction rounds | 2 maximum; then `human-required` |
| Autonomous implementation wall time | 60 minutes maximum |

A budget violation is a failed check, not permission to increase the limit
silently. Any proposed change to the fixture, oracle, canonicalization, schema,
error code, or budget is protected and requires the same human scrutiny as the
contract.

## Consequences and deferred decisions

- Explicit repository identity makes temporary paths and clones semantically
  stable but requires callers to supply a canonical logical ID.
- Supporting the literal local `refs/heads/main` ref makes immutable-binding
  behavior testable; the adapter still converts it to a full verified OID before
  work begins.
- RFC 8785 plus domain-separated BLAKE3 gives an independently reproducible
  oracle while leaving volatile execution metadata outside semantic identity.
- The S0 Git subset is intentionally small. Unsupported features fail visibly
  rather than being handled partially or through an external command.
- The exact JSON schemas and goldens are versioned with this decision; the
  implementation PR consumes them and cannot replace or weaken its own oracle.
- Remote protocols, broader CLI compatibility, repository limits, filesystem
  confinement, privacy egress policy, extraction, and production sandbox tiers
  remain Proposed under their existing requirements and `OD-*` decisions.

## Ratification and policy binding sequence

1. The sole human maintainer `@smutti` reviews this record, the SRS register,
   fixture, and oracle while representing the disclosed Andrea Moretti product
   governance persona. No independent human reviewer is claimed.
2. This final ratification commit records the accountable actor, single-
   maintainer governance model, repository-wide Apache-2.0 license, SRS requirements
   as `Approved`, this record as `Accepted`, and the machine oracle as
   `approved`.
3. The active `main-production` ruleset requires the pull request, strict CI and
   benchmark gates, CodeQL/code quality, resolved threads, linear history, and
   squash-only merge, with no bypass actor. Its approval count is zero only for
   the documented single-maintainer bootstrap.
4. `@smutti` manually merges PR #8 after the exact final head is green. The
   resulting full commit SHA on `main` and protected merge event are the
   authoritative approval reference; a PR head SHA is not sufficient. The
   authoring agent never merges.
5. A separate critical control-plane PR makes autonomous authorization verify
   the SRS-declared contract-bundle digest before any model call. CI validation
   in this proposal is defense in depth but does not replace that preflight gate.
6. A separate protected PR updates only the approved-requirement policy binding
   for the exact S0 IDs, using that source SHA and immutable protected merge/PR
   references.
7. Policy tests prove authorization succeeds for every bound S0 ID and fails
   for unregistered IDs, stale SRS or bundle bytes, a non-ancestor SHA, or the
   wrong slice.
8. Only after all protected merges may an agent-ready implementation issue be
   created.
   `CODEX_AUTOMATION_ENABLED` remains off until its separate promotion gate.

Until step 4, this accepted content is not authoritative on `main`. Until step
6, the policy registry remains empty and autonomous execution remains denied.
