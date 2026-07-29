# Decision 0006: S1 packed SHA-1 acquisition contract

| Field | Value |
|---|---|
| Status | Accepted; authoritative only after the accountable actor manually merges protected PR [#61](https://github.com/smutti/codenoesis/pull/61) |
| Date | 2026-07-29 |
| Scope | `S1 — Safe inventory` compatibility extension only; roadmap `R0` and `R1` |
| Product owner | Andrea Moretti — project governance persona represented by [`@smutti`](https://github.com/smutti), not a separate natural person |
| Technical approver | [`@smutti`](https://github.com/smutti) — sole human maintainer under the single-maintainer bootstrap model |
| Risk | High: public acquisition/error behavior, untrusted binary parsing, integrity, filesystem races, numeric limits, and security evidence |
| Requirement | `FR-ACQ-004` |
| Issue | [#60](https://github.com/smutti/codenoesis/issues/60) |
| Approval reference | PR [#61](https://github.com/smutti/codenoesis/pull/61); effective only on protected manual merge by `@smutti` |

This record approves no production implementation by itself. It becomes
authoritative only through the protected manual merge by `@smutti`; the
authoring agent must not approve or merge it. A separate policy-binding change
and an agent-ready implementation issue are required after ratification.

## Context

The accepted S1 contract deliberately supports only verified loose SHA-1 Git
objects and explicitly rejects a packed object database. That narrow boundary
made the first inventory implementation reviewable, but an ordinary full clone
normally stores most or all objects in `.pack` and `.idx` files. As a result,
the current S4 journey cannot reach extraction for either initial replaceable
public corpus entry:

| Repository | Pinned revision | Full-clone pack observation | Current first result |
|---|---|---|---|
| Lekton | `7a4d1a4a30468f4c18ce158a9b825680b00f4820` | one pack v2/index v2, 6,353 objects, 2,973,530 pack bytes, maximum observed delta depth 38 | exit `10`, `acquisition.unsupported_repository_shape`, feature `packed_object_database` |
| RustDesk | `d412d198720aa56f6cfed2dfad262e8fb1322fb7` | one pack v2/index v2, 86,521 objects, 77,579,431 pack bytes, maximum observed delta depth 38 | exit `10`, `acquisition.unsupported_repository_shape`, feature `packed_object_database` |

These repositories validate generic shapes; they are not sources of product
semantics and are never vendored. When their reachable objects are
materialized loose outside the monitored product process, Lekton reaches the
expected `R3` unsupported-workspace boundary and RustDesk reaches the expected
`R2` gitlink boundary. Packed acquisition is therefore the first generic
compatibility blocker rather than a repository-specific rule.

Changing `standard-local-s1` in place would contradict Decision 0002 and its
existing black-box rejection test. Selecting behavior from repository shape
would also violate the approved explicit-profile compatibility rule. This
decision therefore adds one orthogonal, explicit operational selector while
preserving every accepted S0–S4 command.

## Decision

### New operational acquisition selector

Packed acquisition is selected only by adding:

```text
--acquisition-profile local-git-sha1-packed-v1
```

to an otherwise valid `standard-local-s1`, `standard-local-s2`,
`standard-local-s3`, or `standard-local-s4` scan command. The selector is
invalid without one of those explicit standard profiles. It is never inferred
from `.pack` files, repository size, extensions, Git configuration, the
environment, or a failed loose lookup.

The selector describes how the same immutable Git objects may be read. It is
an operational input and does not enter configuration semantic value, semantic
hashes, snapshot identity, graph identity, documentation, or query results.
For a controlled envelope, equivalent loose and packed representations must
produce byte-identical RFC 8785 semantic payloads and all existing profile
artifacts.

Without the selector, every S0–S4 invocation and error remains byte-identical,
including the accepted S1 test that rejects a packed object database. The
default repository adapter remains loose-only. No constructor, environment
value, or repository shape may silently enable the new behavior.

### Public stream and error lineage

Success retains the snapshot version selected by the standard profile:

| Standard profile | Success artifact |
|---|---|
| `standard-local-s1` | `RepositorySnapshotV2` |
| `standard-local-s2` | `RepositorySnapshotV3` |
| `standard-local-s3` | `RepositorySnapshotV3` plus the accepted S3 publication |
| `standard-local-s4` | `RepositorySnapshotV4` plus the accepted S4 publication |

With the new selector, input and acquisition failures use strict
`CodeNoesisErrorV6`:

- invalid acquisition selector: exit `2`;
- acquisition or policy failure: exit `10`;
- unexpected internal failure: exit `70`.

Each error is one JSON document followed by one LF on stderr with empty stdout.
Post-acquisition extraction, graph, storage, publication, workspace,
documentation, and query failures retain their accepted error schemas and exit
semantics. The mixed lineage is explicit: V6 owns only the new input and
acquisition boundary.

### Supported local object-database subset

The selected profile supports:

- a non-bare local worktree with the accepted real direct `.git` child;
- SHA-1 Git object identity with collision detection;
- repository-local loose objects;
- pack format version 2 paired with index format version 2;
- commit, tree, blob, and tag base entries;
- `OFS_DELTA` and `REF_DELTA`;
- a `REF_DELTA` base in the same pack, another approved local pack, or an
  approved local loose object;
- multiple pack/index pairs and equivalent duplicate OID locations.

The following remain unsupported:

- pack versions other than 2 and index versions other than 2;
- MIDX-only lookup;
- `.promisor`, partial clones, or any missing-object fetch;
- SHA-256 object format;
- shallow or bare repositories;
- alternates, replace refs, or grafts;
- LFS materialization;
- linked worktrees or external Git directories;
- implicit gitlink or submodule traversal;
- remote repair, filters, hooks, credential helpers, and automatic retry.

Regular `.bitmap`, `.keep`, `.mtimes`, and `.rev` sidecars are
non-authoritative hints and may be ignored after safe entry validation. A
multi-pack index may be ignored only when every required pack has its complete
paired v2 index. It never overrides the paired indexes.

This resolves only the packed local SHA-1 subset of `OD-GIT-001`.

### Deterministic safe catalog

The authoritative pair names are
`pack-<40-lowercase-hex>.pack` and
`pack-<same-40-lowercase-hex>.idx`. The 40-hex value is the `pack_id`.

The reader:

1. applies the accepted root and Git-directory policy;
2. enumerates `.git/objects/pack` once;
3. rejects symlink or special entries without following them;
4. sorts authoritative entries by unsigned UTF-8 bytes of `pack_id`;
5. requires exactly one pack and one index per `pack_id`;
6. opens every authoritative regular file once and retains that handle;
7. validates metadata before and after use;
8. never memory maps an untrusted pack or index.

An unpaired, disappearing, replaced, truncated, or transient authoritative
entry yields retryable `acquisition.object_database_changed` with only the safe
component name. The product does not retry automatically. A retry may diagnose
a concurrent repack, but it cannot replace the original failure as acceptance
or release evidence.

Stable unreadable I/O yields non-retryable
`acquisition.object_database_unavailable`. Content that remains stable but
violates the format yields `acquisition.object_database_invalid`. The Linux
capability boundary remains required, so a check/open race cannot grant access
outside the accepted repository root.

### Index v2 validation

Every paired index is validated completely before revision resolution:

- magic bytes are `ff 74 4f 63`;
- version is exactly 2;
- the 256-entry fanout table is monotonic and exactly matches the OID table;
- OIDs are unique and strictly ascending unsigned bytes;
- there is exactly one CRC32 and one 32-bit offset row per OID;
- every high-bit 32-bit offset references an existing 64-bit offset slot;
- every 64-bit slot is referenced exactly once;
- all object offsets are unique candidate offsets inside the pack entry region;
- every candidate is matched one-to-one to the bounded sequential pack-entry
  map before revision resolution;
- the stored pack checksum equals the paired pack trailer;
- the final index checksum is collision-detecting SHA-1 over all preceding
  index bytes;
- no missing, surplus, truncated, or trailing row is accepted.

Index v1 or another version is
`acquisition.unsupported_repository_shape` with feature
`pack_index_version_unsupported`. Structural mutations use the exact safe V6
reason fixed by the machine oracle.

### Pack v2 validation

Every paired pack is checked before revision resolution:

- signature is `PACK`;
- version is exactly 2;
- object count equals the validated index count;
- the final 20 bytes equal collision-detecting SHA-1 over all preceding pack
  bytes;
- that checksum equals the index pack checksum and `pack_id`.

Pack version mismatch is
`acquisition.unsupported_repository_shape` with feature
`pack_version_unsupported`. A stable signature, count, checksum, filename, or
index mismatch is a typed invalid-object-database failure and publishes
nothing.

Whole-pack checksum validation detects raw corruption even in unreachable
entries. Before revision resolution, a bounded sequential pass starts at byte
12, consumes exactly the pack-declared entry count, and builds the authoritative
entry-start/end map. For every entry it validates:

- complete variable-length entry framing;
- one exact zlib stream with no truncation, concatenation, or trailing bytes;
- the index CRC32 over the complete packed entry;
- declared inflated size and the applicable per-entry and cumulative limits;
- one-to-one equality between sequential starts and candidate index offsets.

The sequential pass inflates every zlib stream only far enough to count and
discard its structural output. It does not reconstruct delta results or retain
unreachable object bodies. For every object needed by the once-bound reachable
closure, the reader then additionally validates:

- base or delta semantics and reconstructed type and size;
- collision-detecting Git object OID over `<kind> <size>\\0<body>`.

This makes entry boundaries structural facts derived from the pack stream,
rather than accepting an index pointer that merely lands on framing-looking
bytes inside another compressed stream. Semantic object reconstruction remains
lazy and reachable-only.

### Object locations and duplicate policy

The location order is:

1. the repository-local loose location;
2. ascending `pack_id`;
3. ascending offset within one pack.

All locations for a reachable OID are bounded. Every present location must
reconstruct the same Git type and bytes. The reader does not hide an invalid
earlier location by falling back to a later valid copy. A conflict is
`duplicate_object_conflict`; absence from every approved local source retains
the existing `acquisition.object_missing`; an incompatible kind after valid
reconstruction retains `acquisition.repository_inconsistent`.

Collision-detecting SHA-1 is mandatory at the selected boundary for object,
pack, and index verification. A detected collision yields reason
`sha1_collision`; it is never accepted merely because the 20-byte digest
matches. The exact implementation dependency is reviewable implementation
scope, but a broad Git client or object-database discovery library may not
expand runtime authority to satisfy this rule.

### Bounded delta machine

Delta resolution is iterative with an explicit stack, visited-location set,
and counters. Recursive call-stack growth is not used.

For `OFS_DELTA`, the decoded distance is nonzero, checked for overflow, names a
strictly earlier entry offset in the same pack, and resolves to an indexed
entry. For `REF_DELTA`, the 20-byte base OID resolves only through approved
local locations. A missing base never triggers network, repair, alternates, or
Git.

The delta header's base and result sizes are complete, bounded variable
integers. Copy offsets and lengths use checked arithmetic and remain wholly
inside the base. Literal inserts are complete and nonzero. Opcode zero is
invalid. The final byte count equals the declared result size exactly.

Cycles, missing or invalid bases, malformed programs, excessive depth, and
resource exhaustion terminate with one typed error before publication.

### Fixed limits

The new selector has no environment or user override:

| Limit | Maximum |
|---|---:|
| Entries observed in `.git/objects/pack` | 512 |
| Pack/index pairs | 64 |
| One index | 128 MiB |
| All indexes | 256 MiB |
| Indexed object rows | 8,000,000 |
| One pack | 4 GiB |
| Pack bytes checksum-verified per scan | 8 GiB |
| One compressed entry | 64 MiB |
| Inflated structural bytes for one entry | 256 MiB |
| Cumulative structural inflate bytes per scan | 1 GiB |
| One decompressed delta program | 32 MiB |
| Delta depth | 50 |
| Delta instructions per object | 4,194,304 |
| Intermediate reconstructed bytes per object | 256 MiB |
| Cumulative delta work per scan | 1 GiB |
| Locations for one OID | 8 |
| Reconstructed-object cache | 128 MiB |

Every existing S1 file, tree, path, output, and 60-second wall limit remains in
force and cannot be relaxed by a larger packed-parser ceiling. The effective
bound is the minimum of every applicable inherited type-aware S1 limit and the
packed-parser limit. In particular, a reachable regular-file blob remains
bounded by `single_file_bytes = 4,194,304`: a base entry is rejected from its
declared size before retained semantic body reconstruction, and a delta is
rejected from its bounded result-size header before result allocation or
reconstruction. The prerequisite discard-only structural inflate is charged
separately to the 256 MiB per-entry and 1 GiB cumulative ceilings. All counters
use checked arithmetic and charge before allocation, copy, or decompression.
Public `maximum` is the exact fixed value for its `limit` token and `observed`
is exactly `maximum + 1`.

Each limit requires a successful maximum model and typed maximum-plus-one
model. Large safety caps may use injected bounded readers or model tests rather
than committed bulk data, but representative real-byte integration cases,
including the two public-corpus observations, remain mandatory.

### Failure precedence

The selected acquisition order is:

1. complete invocation validation, including standard profile and acquisition
   selector, before any repository filesystem access;
2. repository root and accepted legacy shape policy;
3. pack-directory entry limit and safe catalog;
4. pairing and stable file handles;
5. index size, structure, order, candidate-offset bounds, and checksum;
6. pack size, header, count, trailer, and index binding;
7. bounded sequential entry map, framing, zlib, inflated size, CRC, and exact
   index-offset/start equality;
8. revision resolution;
9. location limit and deterministic duplicate validation;
10. lazy reachable delta, kind, reconstructed size, collision detection, and
    OID validation;
11. inherited S1 path, mode, file, tree, output, and wall limits;
12. all-or-nothing publication.

Within one check, sorted `pack_id`, then offset, then requested OID order
chooses the first failure. Construction, directory, scheduling, cache, and
hash-map order cannot affect public bytes.

### `CodeNoesisErrorV6`

V6 retains the S1 acquisition family and adds:

| Code | Retryable | Strict context |
|---|---:|---|
| `input.invalid_acquisition_profile` | no | empty |
| `acquisition.object_database_invalid` | no | one exact context shape from the reason table below |
| `acquisition.object_database_changed` | yes | `component` only |
| `acquisition.object_database_unavailable` | no | `component` only |

| Component | Reasons | Additional required safe fields |
|---|---|---|
| `catalog` | `catalog_entry` | none |
| `index` | `index_layout`, `index_fanout`, `index_checksum`, `sha1_collision` | `pack_id` |
| `index` | `index_object_order`, `index_offset` | `pack_id`, `object_oid` |
| `pack` | `pack_header`, `pack_checksum`, `pack_index_mismatch`, `object_count`, `sha1_collision` | `pack_id` |
| `entry` | `entry_header`, `entry_crc`, `zlib_stream` | `pack_id`, `object_oid` |
| `object` | `object_size`, `object_oid`, `sha1_collision`, `duplicate_object_conflict` | `object_oid` |
| `delta` | `delta_base`, `delta_cycle`, `delta_program` | `pack_id`, `object_oid` |

No listed field is optional within its row and no cross-row combination is
valid. `object_oid` always names the requested indexed object being validated,
not an untrusted byte sequence or path.

V6 extends the limit tokens and unsupported features only for an explicitly
selected scan. Its schema binds each limit token to its exact fixed `maximum`
and exact capped `observed = maximum + 1`. No error includes an absolute path,
actual pack filename, source byte, malformed byte, environment value,
credential, or secret.

### Production parser boundary

Production uses a bounded in-process safe-Rust reader owned by the repository
adapter:

- a deterministic `PackCatalog`;
- a positional streaming `PackReader` over retained `File` handles;
- a bounded sequential `PackEntryMap` proving every index offset;
- one unified loose/packed object resolver returning the existing internal Git
  object representation;
- an iterative bounded delta machine;
- one typed domain-error mapping.

The existing domain and `SafeRepositoryAcquirer` result remain independent of
physical Git storage. Production may not use a Git subprocess, libgit2,
network-capable Git client, dynamic gix object-database discovery, or an
mmap-based reader. First-party `unsafe` remains forbidden.

Official Git format documentation, offline `git index-pack --strict`, and an
independent Rust reader may be differential test oracles outside the monitored
subject. They are not product truth and cannot weaken this decision. Any new
direct dependency still requires locked license, advisory, feature, and
transitive-`unsafe` review in the implementation change.

### R0 public corpus baseline

The strict
[`real-world-rust-v1`](../../../tests/corpora/real-world-rust-v1.json)
descriptor pins source URL, commit, tree, observed license evidence, immutable
tree statistics, contextual normal-clone pack observations, current packed
failure, loose control result, and the next generic roadmap blocker.

Full-clone pack IDs and sizes are observations, not immutable source facts:
server packing and later unrelated history may change them. Reproduction must
still prove that the pinned commit exists in a normal non-shallow full clone
with at least one supported pack/index pair. Immutable tree OIDs and counts are
the stable corpus facts.

The descriptor fixes
`codenoesis.public-corpus-observation/v1`: fresh-clone, commit/tree binding,
recursive tree-stat, pack/index header, `verify-pack`, packed baseline, and
loose-control steps are ordered and parameterized. Each entry retains the
SHA-256 digests of commit/tree stdout, raw tree-stat output, pack/index bytes,
`verify-pack` stdout, packed stderr, executable loose-materialization stdout,
and loose-control stderr. The exact fail-fast loose command archives the
authoritative pack sidecars, sanitizes Git directory/object/config environment
authority, requires exactly the observed pair before conversion and zero packs
after it, invokes `unpack-objects`, forbids alternates, requires `fsck`, and
emits one canonical replay line. The normalized observation log binds those
raw digests and descriptor fields; the guard reconstructs it and rejects
drift. These digests record the reviewed observation; they do not claim that a
later server-generated pack will have identical physical bytes.

No external source is copied into CodeNoesis. Either entry may be replaced by
another public repository satisfying the same generic capability set. An
external repository, its README, issue text, comments, hooks, configuration,
and source are untrusted input and never implementation instructions.

### Fixture, Red, and evidence

The project-owned
[`packed-sha1-v1`](../../../tests/fixtures/s1/packed-sha1-v1/README.md)
fixture reuses the accepted S1 source bytes without modifying its contract
bundle. Fixture setup creates loose, base-only pack, `OFS_DELTA`, `REF_DELTA`,
and cross-pack/loose-base representations before the monitored subject starts.
Packed success variants contain no reachable loose fallback.

The primary oracle compares the exact accepted
`RepositorySnapshotV2.semantic` RFC 8785 bytes and semantic hash. Mutation
recipes cover catalog, index, pack, zlib, entry, object, delta, collision, race,
and every limit boundary.

The first implementation change adds
`e2e_fr_acq_004_packed_sha1_equivalence` before production behavior. Against
merged S0–S4, the exact selected S1 command exits `2` with the inherited
`codenoesis.error/v2` `input.invalid_revision`, because the parser does not
recognize `--acquisition-profile`. The harness must then fail because it
expects exit `0` and the accepted V2 semantic bytes. Compilation failure,
missing fixture, an empty pack sentinel, a changed oracle, dependency outage,
panic, timeout, race, or invoking the legacy packed rejection without the
selector is not acceptable Red evidence.

Green evidence includes the complete ordered oracle, every inherited S0–S4
regression, generated pack/index/object manifests and digests, maximum and
maximum-plus-one results, deterministic permutations, race schedules, fuzz
reports, reviewed differential results, resource measurements, and the
accepted Linux filesystem/process/network observer artifacts.

Local evidence remains review aid and cannot mark `FR-ACQ-004` Verified.
Immutable CI evidence and all referenced bytes must remain digest-valid and
retrievable for at least 90 days.

The historical decision index remains byte-identical because Decision 0001
bound it into the immutable S0 contract bundle. Decision 0006 is discoverable
through the SRS ratification register and this separate contract bundle.
Versioning or replacing the historical index requires separate governance and
cannot be smuggled into this high-risk acquisition package.

## Consequences

- Ordinary local SHA-1 packed clones become readable only through an explicit
  versioned operational selector.
- Existing standard profiles, schemas, semantic hashes, and legacy rejection
  behavior remain immutable.
- Physical loose/packed representation cannot alter product facts.
- Packed binary parsing is deterministic, bounded, race-aware, collision-aware,
  and all-or-nothing.
- The production reader remains smaller and less authoritative than a general
  Git client.
- Lekton can progress to the independently governed R3 workspace blocker, and
  RustDesk can progress to the independently governed R2 gitlink blocker.

## Deferred

R2 gitlink/submodule representation, R3 root-package workspaces, remote
acquisition, SHA-256, shallow and bare repositories, alternates, promisor and
partial clones, MIDX as an authoritative source, replace refs, grafts, LFS
materialization, linked worktrees, automatic repair/retry, user-configurable
packed limits, and any history-rewrite semantics remain outside this decision.

## Ratification sequence

1. Review this decision, ErrorV6 schema, corpus descriptor/schema, synthetic
   fixture, machine oracle, maintenance guard, and bundle digest together.
2. `@smutti` manually merges the protected ratification PR; the authoring agent
   never approves or merges it.
3. A separate protected policy PR binds only `FR-ACQ-004` to the byte-identical
   merged SRS.
4. A separate Ready implementation issue fixes allowed paths, dependencies,
   expected Red, evidence, risk, and stop conditions.
5. Implementation starts with the failing public test and retained Red, then
   makes the minimum production change while every legacy contract remains
   Green.
