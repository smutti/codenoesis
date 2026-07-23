# Decision 0002: S1 safe-inventory contract

| Field | Value |
|---|---|
| Status | Accepted; authoritative only after the accountable actor manually merges protected PR [#17](https://github.com/smutti/codenoesis/pull/17) |
| Date | 2026-07-23 |
| Scope | `S1 — Safe inventory` only |
| Product owner | Andrea Moretti — project governance persona represented by [`@smutti`](https://github.com/smutti), not a separate natural person |
| Technical approver | [`@smutti`](https://github.com/smutti) — sole human maintainer under the single-maintainer bootstrap model |
| Risk | High: public artifacts, evidence semantics, untrusted repository traversal, filesystem confinement, and numeric limits |
| Requirements | `DR-EVD-001`, `FR-ACQ-002`, `FR-INV-001`, `NFR-SEC-001` |
| Issue | [#16](https://github.com/smutti/codenoesis/issues/16) |
| Approval reference | PR [#17](https://github.com/smutti/codenoesis/pull/17); effective only on protected manual merge by `@smutti` |

This record approves no production implementation by itself. It becomes
authoritative only through the protected manual merge by `@smutti`; the
authoring agent must not approve or merge it. A separate policy-binding change
and an agent-ready implementation issue are required after ratification.

## Context

S0 proves immutable local Git binding and a deterministic artifact envelope for
one root-level file. S1 must safely walk a useful committed tree and report
what CodeNoesis can and cannot understand before semantic extraction begins.
The previous requirements left five implementation-critical choices open:

1. how S1 coexists with the already approved S0 public contract;
2. the exact repository, path, symlink, gitlink, and limit policy;
3. the minimum inventory and source-evidence structures;
4. deterministic classifier and failure precedence;
5. evidence sufficient to show that untrusted input cannot execute code, use
   network, or escape the repository root.

This decision resolves those choices only for one fixed local profile. It does
not introduce ontology entities, parse source semantics, or revive packed Git
object support deferred after S0.

## Decision

### Explicit S1 profile and S0 compatibility

S1 is selected only by:

```text
noesis scan \
  --repository <local-worktree-root> \
  --repository-id <canonical-logical-id> \
  --revision <full-oid-or-refs/heads/main> \
  --profile standard-local-s1 \
  --format json
```

Success emits one strict `RepositorySnapshotV2` and LF on stdout, no stderr,
and exit `0`. Invalid S1 invocation uses exit `2`; acquisition and policy
failure uses exit `10`; unexpected internal failure uses exit `70`. Every S1
error is one strict `CodeNoesisErrorV2` and LF on stderr with empty stdout.

The approved S0 invocation has no `--profile`. It continues to emit
`RepositorySnapshotV1` and `CodeNoesisErrorV1`. Dispatch may not depend on
repository shape, extensions, environment, or implicit configuration. This
explicit opt-in avoids silently breaking the S0 oracle while S1 is developed.

### Repository and filesystem boundary

The canonical directory named by `--repository` is the sole allowed read root.
Its Git directory must be the real, non-symlink direct child `<root>/.git`.
Gitfiles, linked worktrees, external Git directories, alternate object stores,
and symlinked root or Git-directory components are rejected. Absolute paths are
operational inputs only and never enter semantic output or public errors.

The verified committed tree of the once-bound immutable commit is
authoritative. Dirty and untracked worktree files are ignored and never read as
source. S1 accepts verified loose SHA-1 objects and regular tree modes `100644`
and `100755`; directory mode `040000` is traversed. It rejects:

- mode `120000` as `symlink` without interpreting or following the target;
- mode `160000` as `gitlink` without opening a nested repository;
- any other mode as `special_file_mode`;
- LFS pointer materialization;
- packed object databases and the advanced Git shapes already deferred.

Each tree component must be valid UTF-8, preserved byte-for-byte without
normalization, non-empty, neither `.` nor `..`, and contain no backslash, C0
control, or `U+007F`. The canonical relative path joins components with `/`.
Traversal order is ascending unsigned UTF-8 bytes of the complete path.

Root policy is checked before object traversal. After immutable object
integrity, each entry is evaluated in this order: path validity, path/depth
limits, entry-mode policy, file-count/byte limits, static classification, and
canonical-output limit. Within the same check, the canonical path order chooses
the first failure. No failure publishes a partial artifact.

### Fixed `standard-local-s1` limits

The fixed profile has no environment or user override:

| Limit | Maximum |
|---|---:|
| Regular-file paths | 20,000 |
| Reachable tree entries | 25,000 |
| Cumulative uncompressed regular-file bytes | 256 MiB |
| One uncompressed regular file | 4 MiB |
| Canonical relative path | 1,024 UTF-8 bytes |
| One path component | 255 UTF-8 bytes |
| Recursion depth | 32 components |
| Canonical stdout document | 32 MiB including final LF |
| One scan wall time | 60,000 ms |

Counts are per reachable canonical path, so two paths to one blob count twice.
The implicit root is not a tree entry. A root-level entry has depth one.
Declared object sizes and streaming decompression are bounded before full
allocation; decompression stops at the lowest applicable framing, entry, path,
or byte limit. Public limit context reports the first value over the maximum,
capped at `maximum + 1`, so an attacker cannot amplify error output.

The conformance evidence ceilings are 30 seconds CPU per scan, 512 MiB peak
RSS, 64 MiB temporary disk, and 180 seconds for the S1 suite. Determinism uses
50 replays and 10 shuffled parallel repetitions. Required evidence is retained
for at least 90 days. Input limits have successful maximum and typed
maximum-plus-one cases. Exceeding an external evidence ceiling fails the gate;
it is not rewritten as a fabricated subject error.

This resolves `OD-LIM-001` only for S1. Limits for later slices remain open.

### Deterministic static inventory

S1 inspects paths, modes, and bounded committed blob bytes in process. It does
not invoke Git, a build tool, a parser, a shell, a plugin, or a model. Rules are
ASCII case-sensitive:

| Rule | Classification |
|---|---|
| basename `build.rs` or suffix `.rs` | Rust source |
| suffix `.sh` | shell source |
| basename `Cargo.toml` | Cargo manifest |
| basename `openapi.yaml` or `openapi.yml` and bytes start `openapi:` | OpenAPI contract |
| basename `rustfmt.toml` | rustfmt configuration |
| exact `.github/CODEOWNERS` | GitHub CODEOWNERS ownership source |
| basename `README.md` | documentation |
| exact `build.rs` | target-execution sentinel diagnostic |
| executable `.sh` | target-execution sentinel diagnostic |
| no preceding rule | unsupported content |

`text_utf8` means the complete bounded blob is valid UTF-8 and did not reach the
unsupported fallback. Every other blob is `binary_or_unknown`. Recognition is
not parsing: manifests, contracts, configuration, ownership, source syntax,
and archives remain uninterpreted. Their unavailable capabilities and coverage
gaps are explicit.

The exact arrays, stable keys, roles, capabilities, diagnostics, and gaps are
fixed by the machine oracle. Arrays are sorted, duplicates are forbidden, and
construction order cannot affect canonical bytes or hashes.

### `RepositorySnapshotV2`

V2 preserves the S0 top-level envelope and version lineage and adds one strict
semantic `inventory`. It uses:

- schema `codenoesis.repository-snapshot/v2`;
- pipeline `codenoesis.pipeline/s1-v1`;
- fixed profile `standard-local-s1`;
- inventory `codenoesis.inventory/v1`;
- classifier `codenoesis.inventory-classifier/s1-v1`;
- source evidence `codenoesis.source-evidence/v1`;
- snapshot hash domain `codenoesis.repository-snapshot.semantic.v2`.

The fixed configuration semantic value is
`{"profile":"standard-local-s1"}`. Its hash uses the existing
`codenoesis.configuration.semantic.v1` domain, a zero separator, RFC 8785
bytes, and BLAKE3-256. The snapshot hash similarly uses the V2 domain, a zero
separator, and RFC 8785 bytes of the complete `semantic` value. Stored `.jcs`
goldens have one trailing repository LF excluded from the semantic hash.

The strict schema is
[`repository-snapshot-v2.schema.json`](../../../tests/specifications/s1/repository-snapshot-v2.schema.json).
The reviewed fixed-envelope artifact and canonical bytes live with the S1
fixture.

### `SourceEvidenceV1`

Each regular file has exactly one evidence record. An evidence record contains:

- a deterministic evidence ID;
- canonical repository identity, VCS, object format, and commit OID;
- exact blob OID and canonical relative path;
- a half-open byte span `[0, blob_length)`;
- classifier identity and version;
- deterministic derivation rules.

All inventory references resolve to these records. Evidence IDs and paths are
unique; spans are non-negative and within the verified blob; the referenced
blob hashes to the recorded OID. A missing, dangling, conflicting, or invalid
record prevents publication. Evidence is a provenance claim, not proof that an
unparsed source statement is true.

The strict schema is
[`source-evidence-v1.schema.json`](../../../tests/specifications/s1/source-evidence-v1.schema.json).

### Typed S1 failures

`CodeNoesisErrorV2` retains S0 conditions and adds:

| Code | Strict context |
|---|---|
| `input.invalid_profile` | empty |
| `acquisition.path_invalid` | `reason` only; raw invalid bytes are not emitted |
| `acquisition.root_policy_violation` | `policy` only; no absolute path |
| `acquisition.entry_policy_violation` | safe canonical `path` and `entry` |
| `acquisition.limit_exceeded` | `limit`, `maximum`, and capped `observed` |

Every context is code-specific, closed to unknown fields, non-retryable, and
free of absolute paths, raw invalid bytes, source content, environment values,
and secrets. The strict schema is
[`codenoesis-error-v2.schema.json`](../../../tests/specifications/s1/codenoesis-error-v2.schema.json).

### No execution, network, or root escape

The complete S0 process/network boundary remains mandatory. S1 additionally
requires a Linux security gate that combines:

1. a read-only Landlock or equivalently strong path-capability allowlist for
   only the canonical repository root after initial image load;
2. PID-scoped observation of open, openat, openat2, stat, access, readlink, and
   directory-enumeration operations with directory-descriptor resolution;
3. self-tests for direct, relative, parent, symlink, proc-fd, and alternate
   object-store escapes before product evidence;
4. an outside-root canary and the committed target-execution sentinels.

An attempted denied access, unresolved event, missing self-test, outside-root
path, filesystem write, process execution, or network event fails evidence even
if another control blocks it. Other platforms may run functional smoke tests
but do not replace the Linux security evidence.

### Fixture, Red, and retained evidence

The first-party fixture is
[`safe-inventory-v1`](../../../tests/fixtures/s1/safe-inventory-v1/README.md).
Its manifest fixes all source bytes, modes, Git identities, tree and commit
objects, reviewed goldens, and generated malicious variants. Large boundary
and attack inputs are deterministic recipes rather than committed bulk data.

The immutable machine oracle is
[`e2e_fr_inv_001_safe_inventory.json`](../../../tests/specifications/s1/e2e_fr_inv_001_safe_inventory.json).
The first implementation change adds its black-box harness before production
behavior. Against merged S0, the exact command exits `2` with
`codenoesis.error/v1` and `input.invalid_revision` because
`--profile standard-local-s1` is absent. The runner must fail because it expects
exit `0` and `RepositorySnapshotV2`. Compilation failure, missing fixture,
modified oracle, outage, panic, or race is not acceptable Red evidence.

Green evidence contains every ordered S1 result and every inherited S0
regression, immutable logs and reports, security observer and self-test
artifacts, exact base/head/pre-implementation commits, deterministic
environment, agent identity, policy version, duration, cost, and all required
digests. Local output is review aid, not `Verified` evidence; immutable CI
artifacts must remain retrievable and digest-valid for at least 90 days.

## Consequences

- S1 can be implemented outside-in without silently changing S0.
- Inventory facts are deterministic, bounded, evidence-backed, and explicit
  about unsupported or unparsed content.
- Secure repository traversal is intentionally narrower than general Git.
- The profile limits and classifier map are public versioned behavior; changing
  either requires a new reviewed contract.
- Linux is the normative filesystem-confinement evidence platform for S1.

## Deferred

Remote acquisition, packed objects, SHA-256, LFS materialization, shallow and
bare repositories, alternates, replace/grafts, linked worktrees, symlink or
submodule traversal, archive expansion, semantic parsers, ontology entities,
user-configurable limits, and non-Linux security certification remain outside
S1. The explicit packed-object deferral recorded after issue #9 remains in
force.

## Ratification sequence

1. Review this decision, schemas, fixture, goldens, machine oracle, maintenance
   guard, and bundle digest together.
2. `@smutti` manually merges the protected ratification PR; the authoring agent
   never merges.
3. A separate protected policy PR binds the exact Approved S1 IDs to the
   byte-identical SRS commit.
4. A separate issue declares one implementation objective, allowed paths,
   expected Red, evidence, risk, dependencies, and stop conditions.
5. Implementation starts with the failing public S1 test and retained Red,
   then makes the minimum production change while all S0 regressions remain
   Green.
