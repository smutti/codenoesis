# Decision 0010: S1 gitlink and submodule boundary contract

| Field | Value |
|---|---|
| Status | Accepted; authoritative only after the accountable actor manually merges protected PR [#93](https://github.com/smutti/codenoesis/pull/93) |
| Date | 2026-08-02 |
| Scope | `S1 — Safe inventory` compatibility extension only; roadmap `R2` and the inherited `R0-R2` checkpoint |
| Product owner | Andrea Moretti — project governance persona represented by [`@smutti`](https://github.com/smutti), not a separate natural person |
| Technical approver | [`@smutti`](https://github.com/smutti) — sole human maintainer under the single-maintainer bootstrap model |
| Risk | High: public snapshot/error contracts, ontology boundary semantics, untrusted Git metadata, explicit multi-root filesystem authority, privacy, races, and numeric limits |
| Requirement | `FR-ACQ-005` |
| Issue | [#92](https://github.com/smutti/codenoesis/issues/92) |
| Authorization | `@smutti`: “Autorizzo issue #92” in the maintainer-supervised task on 2026-08-02 |
| Approval reference | PR [#93](https://github.com/smutti/codenoesis/pull/93); effective only on protected manual merge by `@smutti` |

This record approves no production implementation by itself. It becomes
authoritative only through protected manual merge by `@smutti`; the
authoring agent must not approve or merge it. Production Rust, policy binding, workflow,
dependency, and release changes remain separate. After this requirement is
Approved on `main`, one separately authorized product change may implement the
complete R2 vertical outcome while retaining R0 and R1 regressions.

## Context

Decision 0006 and the merged `FR-ACQ-004` implementation already provide:

- `R0`, the replaceable pinned public Rust corpus baseline;
- `R1`, explicit read-only local SHA-1 pack v2/index v2 acquisition;
- storage-representation-invariant semantic output for loose and packed Git
  objects;
- byte-identical behavior for every invocation without the R1 selector.

The pinned RustDesk corpus observation now reaches one mode `160000` entry at
`libs/hbb_common`, whose tree-entry OID is
`69cea8dafee147848ae88702029f4bf7df7224c3`. The accepted legacy S1 adapter
correctly rejects that entry. Gitlinks are neither regular files nor
directories, and a submodule worktree, URL, branch, or local Git configuration
must not silently become analysis authority.

R2 therefore introduces an explicit semantic boundary profile. It represents
the committed superproject fact and may verify an independently supplied
nested repository, but it never discovers, fetches, enters, executes, or
federates nested source implicitly. The contract is generic and includes no
RustDesk- or Lekton-specific rule.

## Decision

### Explicit selector and profile matrix

R2 is selected only by adding:

```text
--repository-boundary-profile local-gitlinks-v1
```

to an otherwise valid `noesis scan --profile standard-local-s4` invocation.
The selector is invalid with S0, S1, S2, S3, S5, S6, an absent standard
profile, or a non-scan command. This deliberately creates one new public
snapshot contract rather than versioning every earlier snapshot.

The selector may coexist with:

```text
--acquisition-profile local-git-sha1-packed-v1
```

for the root repository. R1 remains an operational storage selector and is
excluded from semantic identity. Repository shape, a `.gitmodules` file, a
present worktree directory, environment state, an extension, Git
configuration, or a failed legacy scan never selects R2 implicitly.

An optional explicit nested-input document is supplied as:

```text
--repository-boundary-manifest <path>
```

The manifest is valid only with `local-gitlinks-v1`. The path is operational
and excluded from semantic identity. A semantically equivalent manifest,
entry order, loose/packed nested representation, or containing directory
produces identical semantic bytes. The independently verified nested
repository identity, commit OID, and tree OID do enter the boundary report and
therefore the snapshot semantic hash.

Without the R2 selector, every accepted S0-S6 invocation, output, error,
snapshot, storage artifact, documentation file, query result, bundle, and hash
remains byte-for-byte unchanged. In particular, the accepted
`e2e_fr_acq_002_gitlink_rejected` behavior remains mandatory.

### Public journey

The primary unbound journey is:

```text
noesis scan \
  --repository {repository_path} \
  --repository-id urn:codenoesis:fixture:s1-gitlink-boundary-v1 \
  --revision e3a117e190a92585bcae4c49c775d310243107e7 \
  --profile standard-local-s4 \
  --repository-boundary-profile local-gitlinks-v1 \
  --store {store_path} \
  --format json
```

The bound journey adds:

```text
--repository-boundary-manifest \
  tests/fixtures/s1/gitlink-boundary-v1/boundary-input-matching.json
```

Success emits one strict `RepositorySnapshotV5` JSON document plus one LF on
stdout, emits no stderr, publishes atomically through the existing local-store
v1 role, and exits `0`. S4 documentation and exact-ID query operations must
read both V4 and V5 snapshots. For V5 they use the unchanged root inventory,
extraction chunks, and knowledge graph; they do not interpret or merge nested
facts. Existing V4 publication and reading remain unchanged and require no
migration.

### Gitlink tree semantics

During the already bounded recursive walk of the root commit tree, an entry
with exact mode `160000` is:

- an external repository boundary;
- identified by its canonical root-relative path;
- bound to the 20-byte tree-entry OID interpreted as the required nested
  commit OID;
- excluded from regular-file count, file bytes, language/manifests/contracts,
  extraction input, root knowledge graph, and target execution;
- never opened as a worktree path and never counted as a directory.

The parent directory trees still follow the accepted S1 counting rules. The
root inventory remains `codenoesis.inventory/v1`; no inventory v2 is needed.
The committed `.gitmodules` blob remains an ordinary root file for that
unchanged classifier and may therefore retain its existing unsupported-content
classification independently of the R2 metadata projection.

The root commit is resolved and bound exactly once before tree traversal. A
ref move cannot change the selected commit. Every gitlink fact is taken only
from that immutable tree closure.

### Restricted committed `.gitmodules` grammar

Only the root commit's exact `.gitmodules` tree entry is inspected. Absence is
valid. If present, it must be one regular `100644` blob; a symlink, gitlink,
tree, executable file, or special shape is
`acquisition.repository_boundary_metadata_invalid` with reason
`unsafe_entry_kind`.

The parser accepts at most 1 MiB of UTF-8 and applies this complete grammar:

- LF and CRLF terminate lines; a bare CR, NUL, C0 control other than tab, or
  DEL is rejected;
- empty lines and lines whose first non-horizontal-whitespace byte is `#` or
  `;` are comments;
- a section is exactly `[submodule "<name>"]` after trimming leading and
  trailing horizontal whitespace;
- `<name>` is 1 through 255 ASCII bytes from `[A-Za-z0-9._/-]` and is never
  emitted raw;
- an assignment is `<key> = <value>` inside a section, with optional horizontal
  whitespace around key, equals, and value;
- a key is 1 through 64 lowercase ASCII bytes matching
  `[a-z][a-z0-9-]*`;
- values are nonempty UTF-8 byte strings after horizontal-whitespace trimming;
  quoting, escapes, continuation, interpolation, includes, subsections other
  than the exact form above, and inline comments are unsupported syntax;
- each section contains exactly one `path` and one `url` and no duplicate key;
- each section name is unique;
- `path` is an accepted S1 canonical relative path, at most 1,024 UTF-8 bytes
  and 255 bytes per component, with no absolute root, empty component, dot,
  dot-dot, backslash, control, or normalization ambiguity;
- two declarations may not map to the same canonical path.

Invalid encoding and syntax use the exact V9 reasons fixed by its schema. The
first failure is selected by byte offset, then the decision's reason order.
The byte limit is charged before decode and allocation. Section and key limits
are charged before insertion.

Only `path` and `url` have R2 meaning. Any other syntactically valid key is
retained as its safe key name plus SHA-256 of the trimmed value and creates a
`boundary.gitmodules_key_unsupported` coverage gap. Its value is never emitted
raw or acted upon.

The URL is inert metadata. The product performs only this byte-prefix lexical
classification, in order:

- `relative` for exact ASCII prefixes `./` or `../`;
- `absolute_path` for `/`;
- `file`, `ssh`, `https`, `http`, or `git` for the corresponding exact
  lowercase ASCII `<scheme>:` prefix;
- `scp_like` for
  `^[^/@:\\x00-\\x20\\x7f]+@[^/:\\x00-\\x20\\x7f]+:.+$` over UTF-8 bytes;
- `other` otherwise, including uppercase or mixed-case schemes.

It emits only the kind and SHA-256 of the exact trimmed UTF-8 value. It never
normalizes, resolves, connects to, logs, displays, uses as identity, passes to
Git, or consults a credential helper for that value. Credentials, query data,
and fragments must not occur in output, errors, logs, traces, retained
evidence, or identities.

### Declarations, states, and coverage

Gitlinks and declarations are joined only by exact canonical path.

Each gitlink has exactly one state:

- `declared_unbound`: one declaration matches and no nested repository was
  explicitly supplied;
- `undeclared_unbound`: no declaration matches;
- `explicitly_bound`: one supplied nested repository independently resolves
  to the exact gitlink commit.

Missing nested checkouts always succeed deterministically. An unbound boundary
has `boundary.nested_repository_unbound`. An undeclared boundary additionally
has `boundary.gitmodules_declaration_missing`. A valid declaration without a
gitlink remains in `declarations`, has no boundary ID, and creates
`boundary.gitmodules_declaration_orphan`; it is not an error and grants no
authority.

An explicitly bound nested repository has
`boundary.nested_repository_not_analyzed`. R2 verifies only repository
identity, exact commit, and root tree. It does not inventory, parse, query,
document, recurse into, or merge that repository in the root invocation.
Cross-project ontology federation remains separately governed.

### Explicit nested input and authority

`RepositoryBoundaryInputV1` is at most 262,144 bytes of strict UTF-8 JSON with
duplicate object members rejected before schema validation. Its
`root.repository_identity` and
`root.commit_oid` must equal the scan invocation. Entries are sorted by
unsigned UTF-8 bytes of `boundary_path`; duplicate paths fail before any nested
root is opened.

Each entry supplies:

- one boundary path that must name an existing root gitlink;
- a distinct stable nested repository identity;
- one canonical repository root relative to the manifest's containing
  directory;
- one exact 40-lowercase-hex SHA-1 revision;
- either `verified-loose-sha1-v1` or
  `local-git-sha1-packed-v1` as an operational acquisition profile.

The declared revision is compared to the parent gitlink OID before opening the
nested root. A difference is non-retryable
`acquisition.nested_repository_mismatch`, with only boundary path and expected
and observed OIDs. A match authorizes one root-confined read of that exact
nested repository under the selected existing acquisition subset. The reader
binds the commit and tree, performs pre/post stability checks, and closes the
root before proceeding.

The manifest directory is the explicit confinement base. Absolute roots,
parent components, backslashes, symlinks/reparse points in the authorized
chain, duplicate canonical roots, and escaping resolution are rejected. The
product does not search parents, inspect a superproject worktree path, follow a
`.git` indirection outside the accepted direct-directory contract, or use any
ambient Git configuration.

The schema is intentionally flat. Unknown recursive members are
`input.invalid_repository_boundary_manifest` with reason `recursive_input`.
Explicit nesting depth is exactly one. A single invocation may open at most
the root plus 32 explicitly supplied nested roots. No nested source or nested
gitlink is traversed.

The nested acquisition profile and all manifest/store paths are operational
and absent from semantic output. Equivalent loose and packed nested object
databases that bind the same identity, commit, and tree produce byte-identical
boundary and snapshot semantics.

### Identities, hashes, and deterministic ordering

All SHA-256 identities hash the UTF-8 domain, one `0x00` separator, then the
listed UTF-8 fields separated by one `0x00`. Fields are never Unicode- or
URL-normalized after they pass their fixed grammar.

Boundary identity:

```text
sha256(
  "codenoesis.repository-boundary/v1" 0x00
  root_repository_identity 0x00 root_commit_oid 0x00
  canonical_path 0x00 gitlink_oid
)
```

and public form:

```text
urn:codenoesis:repository-boundary:sha256:<digest>
```

Declaration identity uses domain
`codenoesis.gitmodules-declaration/v1` and fields root identity, root commit,
canonical path, and SHA-256 of the exact section name. Tree-entry evidence uses
domain `codenoesis.boundary-evidence.git-tree-entry/v1` and fields root
identity, root commit, containing tree OID, path, mode, and object OID.
Declaration evidence uses domain
`codenoesis.boundary-evidence.gitmodules/v1` and fields root identity, root
commit, blob OID, start byte, and end byte. Coverage-gap identity uses domain
`codenoesis.boundary-gap/v1`, gap code, and subject ID.

Public forms are respectively:

```text
urn:codenoesis:gitmodules-declaration:sha256:<digest>
urn:codenoesis:boundary-evidence:sha256:<digest>
urn:codenoesis:boundary-gap:sha256:<digest>
```

Boundaries sort by canonical path bytes. Declarations sort by path then
declaration ID. Unsupported keys sort by key then value digest. Coverage gaps
sort by code, path, then subject ID. Tree evidence sorts by path and precedes
declaration evidence, which sorts by source start byte. Referenced ID arrays
retain this derivation order and contain no duplicate.

The complete boundary report is semantic. Envelope time, job/correlation IDs,
filesystem locations, manifest path/order, worktree presence, acquisition
profile, scheduler, and observation timing are excluded.

### RepositorySnapshotV5

The selected profile emits:

- schema `codenoesis.repository-snapshot/v5`;
- semantic configuration `codenoesis.configuration/v2` with exact value
  `{"profile":"standard-local-s4","repository_boundary_profile":"local-gitlinks-v1"}`;
- configuration hash domain `codenoesis.configuration.semantic.v2`;
- snapshot hash domain `codenoesis.repository-snapshot.semantic.v5`;
- pipeline `codenoesis.pipeline/s4-r2-v1`;
- unchanged ontology `codenoesis.ontology/rust/v2`;
- unchanged extractor contract `codenoesis.extraction/v2`;
- existing S4 extractor version order followed by
  `codenoesis.git-boundary/s1-v1`;
- unchanged inventory v1, extraction chunk v2, knowledge graph v2, evidence
  lineage v2, and envelope;
- one new semantic `repository_boundaries` field conforming to
  `codenoesis.repository-boundaries/v1`.

Configuration and snapshot hashes use BLAKE3-256 over the UTF-8 domain, one
`0x00`, and RFC 8785 canonical JSON. The configuration payload excludes its
schema version and hash exactly as v1 does. The snapshot payload is the
complete V5 `semantic` value.

For the synthetic fixture, the V5 inventory is the existing S4 workspace plus
the committed `.gitmodules` regular file; the gitlink itself is absent from
inventory. Root Rust extraction and graph topology remain the same source
shape as the inherited S4 fixture, while commit-bound evidence and snapshot
identity correctly bind the new root commit. Bound and unbound variants differ
only in the boundary semantic projection and derived snapshot hash.

### Error and stream lineage

Only new selector, boundary-manifest, metadata, nested-binding, and boundary
limit failures use strict `CodeNoesisErrorV9`:

- invalid selector/profile matrix or manifest-without-selector: exit `2`;
- unavailable, malformed, root-mismatched, duplicate, or recursive manifest:
  exit `2`;
- malformed committed metadata, nested mismatch/unavailability/change, or R2
  limit: exit `10`;
- unexpected R2 internal failure: exit `70`.

Every failure emits one RFC 8785 JSON document plus one LF on stderr, empty
stdout, no snapshot/store mutation, and no raw absolute path, URL, source byte,
environment value, credential, or secret. Only
`acquisition.nested_repository_changed` is retryable. The product never
retries automatically.

Root acquisition retains its accepted lineage: ErrorV6 when the R1 packed
selector is present, otherwise the existing selected S4 lineage. Extraction,
graph, storage, publication, documentation, and query failures retain their
accepted S4 schemas and exits. Nested physical acquisition failures are mapped
to V9 `acquisition.nested_repository_unavailable` with one safe reason; no
nested absolute root is exposed.

Failure precedence is:

1. CLI shape, selector matrix, and manifest-without-selector;
2. boundary-manifest open, bytes, duplicate-member/schema, root binding,
   duplicate path/root, and count/depth limits;
3. inherited root acquisition and immutable commit binding;
4. root gitlink count and canonical path policy;
5. `.gitmodules` entry kind, byte limit, decode, syntax, section/key limits,
   and mapping ambiguity;
6. explicit entries in boundary-path order: revision mismatch, nested open,
   nested acquisition, and stability race;
7. canonical boundary-report byte limit;
8. inherited S4 extraction, graph, storage, and publication.

Within one category the lowest canonical path wins; metadata syntax uses the
lowest source byte, then the reason order in ErrorV9. No partial boundary
report or store head is published.

### Fixed limits

Every count is charged inclusively before allocation, open, traversal, or
append. Exact maximum succeeds; maximum plus one emits one V9 limit error whose
`observed` is capped at `maximum + 1`.

| Limit | Maximum |
|---|---:|
| Boundary manifest bytes | 262,144 |
| Root gitlink entries | 128 |
| Committed `.gitmodules` bytes | 1,048,576 |
| Submodule sections | 256 |
| Keys per section, including `path` and `url` | 32 |
| Explicit nested repositories | 32 |
| Explicit nesting depth | 1 |
| Total repository roots opened | derived maximum 33: one root plus 32 supplied nested roots |
| Canonical boundary report bytes | 1,048,576 |
| Canonical path bytes | inherited 1,024 |
| Canonical path component bytes | inherited 255 |
| Canonical complete snapshot bytes | inherited 33,554,432 |
| Scan wall time | inherited 60,000 ms |

The 33-root bound is an invariant derived from the root plus the separately
charged 32-entry nested limit, not a competing public error selector. The
32-entry maximum observes 33 opened roots; its plus-one case fails before a
34th root can open. The 1 MiB boundary-output limit explicitly replaces the
8 MiB candidate in issue #92 so both exact maximum and maximum-plus-one remain
constructible under the fixed item and input limits.

The accepted root S1 file/tree/byte/depth limits and R1 pack/index/delta limits
remain independently enforced. R2 adds no hidden retry, unbounded URL parser,
history traversal, worktree search, or nested source allocation. The future
implementation must record a bounded peak-memory observation; this decision
adds no separate memory maximum because all new retained collections and bytes
have explicit limits inside the already bounded local scan.

### Security and privacy boundary

Repositories, object databases, trees, `.gitmodules`, nested manifests,
worktrees, URLs, and generated fixture variants are untrusted input. The
selected profile grants only these additional capabilities:

- inspect mode `160000` entries already reached in the immutable root tree;
- read one bounded committed root `.gitmodules` blob;
- read one explicitly named strict manifest;
- independently open at most 32 canonical roots confined below that manifest's
  directory.

It grants no network, DNS, process, Git executable, shell, hook, filter,
credential helper, configuration include, environment expansion, file URL,
device, symlink/reparse traversal, write, checkout, repair, submodule update,
target execution, model provider, Council, or implicit root authority. No
first-party `unsafe` or new dependency is approved by this decision.

Present-but-unsupplied nested directories are never opened, even when they
contain canaries, `.git` data, valid source, or a matching commit. A supplied
root cannot authorize siblings or parents. Pre/post metadata changes produce
one retryable race result, never mixed facts or automatic retry.

### Public corpus checkpoint

The accepted `tests/corpora/real-world-rust-v1.json` and R1 contract bundle are
immutable inherited R0/R1 evidence and are not rewritten by this decision.
The new bundle binds their existing digests.

After product implementation, a reproducible, non-committed RustDesk pilot at
commit `d412d198720aa56f6cfed2dfad262e8fb1322fb7` must:

- retain the existing pack acquisition observation;
- emit exactly one external boundary at `libs/hbb_common` for nested commit
  `69cea8dafee147848ae88702029f4bf7df7224c3`;
- continue root analysis without a nested checkout;
- advance from the generic R2 blocker to the next generic independently
  governed blocker;
- retain only repository ID, pinned OIDs, counts, digests, timings, gaps,
  environment, and commands, with no vendored source or raw URL.

The pilot is evidence, not a special case and not proof of broad Git support.

## Acceptance and first Red

The future product change must first add
`e2e_fr_acq_005_gitlink_boundaries` and run it against the merged pre-R2
runtime. The exact public command is the primary journey above.

Before implementation, the current binary rejects the unknown R2 selector
before any repository or store read:

- subject exit: `2`;
- stdout: empty, SHA-256
  `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`;
- stderr: exactly
  `{"code":"input.invalid_revision","context":{},"message":"invalid revision","retryable":false,"schema_version":"codenoesis.error/v4","stage":"input"}\n`;
- stderr bytes: `149`;
- stderr SHA-256:
  `7f75f7a91f6af0328795f3fbd2729e69756beba2ebd642cc1f6401265662a2fe`;
- store path absent;
- no root/nested open, process, network, target execution, write, or
  publication.

The test runner must be nonzero only because it expected successful V5 output.
Compilation failure, missing target, malformed fixture objects, changed
oracle, legacy gitlink acceptance, panic, timeout, dependency outage, or any
side effect is rejected Red evidence.

Green evidence must cover, in order:

1. exact unbound snapshot and boundary projection;
2. present-but-unsupplied worktree equivalence;
3. exact explicitly bound projection and loose/packed equivalence;
4. invocation/manifest root binding and flat-input validation;
5. declared, undeclared, orphan, unsupported-key, and not-analyzed gaps;
6. malformed, escaping, duplicate, ambiguous, and credential-canary metadata;
7. nested mismatch, unavailable, object failure, and replacement race;
8. exact maximum and maximum-plus-one for every R2 limit;
9. 50 declaration/input permutations and parallel replay;
10. no ambient authority, target execution, raw URL, or outside-root access;
11. V5 publication followed by unchanged root docs and query;
12. selector-absent gitlink rejection and every S0-S6 regression;
13. R0 corpus and R1 packed guards;
14. the reproducible RustDesk progression observation.

The machine oracle fixes complete test names, errors, limits, fixture paths,
inherited regressions, and evidence requirements.

## Consequences

Positive consequences:

- ordinary packed Rust repositories containing submodules can be represented
  without requiring network or nested checkout availability;
- ontology truth remains separated by repository boundary;
- explicit nested verification is deterministic and storage-representation
  invariant;
- URL metadata cannot silently become authority or leak raw credentials;
- R0/R1 and every legacy profile remain stable.

Costs and limitations:

- V5 readers require a small explicit compatibility addition;
- R2 does not analyze nested code or federate graphs;
- the supported `.gitmodules` grammar is intentionally narrower than Git's
  complete configuration grammar;
- SHA-256 Git repositories, remote acquisition, linked worktrees, alternates,
  LFS, partial/promisor clones, automatic repair, nested recursion, and
  symlink support remain open;
- a public corpus pilot can establish compatibility progression but not
  production verification by itself.

## Alternatives rejected

- **Treat gitlinks as directories or files:** false ontology and unsafe
  traversal semantics.
- **Infer support from `.gitmodules` or mode `160000`:** breaks legacy
  compatibility and explicit-profile policy.
- **Run `git submodule`, libgit2, or a new Git dependency:** grants unapproved
  process/network/configuration authority and is unnecessary for this slice.
- **Automatically inspect a present nested worktree:** ambient filesystem
  state would change semantic output and create confused-deputy risk.
- **Resolve or redact URLs heuristically after logging:** secrets may already
  have escaped; digest-only projection is simpler and safer.
- **Merge nested facts into the root graph:** cross-project federation and
  identity semantics are not approved by R2.
- **Change InventoryV1 or SnapshotV4 in place:** would invalidate accepted
  bundles and legacy hashes.
- **Rewrite the accepted R0/R1 corpus descriptor:** would mutate inherited
  evidence; the R2 bundle instead binds it unchanged.

## Scope and sequence

This governance change is intentionally narrowed from issue #92:

- it adds no `RepositoryInventoryV2`; boundaries are a separate V5 semantic
  section;
- it does not modify `tests/corpora/real-world-rust-v1.json`;
- it does not modify the incomplete historical decisions index;
- it changes no accepted schema, fixture, golden, decision, bundle, source,
  policy, workflow, architecture, dependency, benchmark, or release file.

After protected merge makes `FR-ACQ-005` Approved, one separate
maintainer-supervised implementation issue may authorize production paths,
exactly one S1 compatibility extension, this oracle, three correction rounds,
and no new dependency. The implementation change must retain immutable Red
evidence, make the complete R2 package Green, publish a PR for independent
human review, and must not self-approve or self-merge.

This resolves only the safe local SHA-1 gitlink/submodule-boundary subset of
`OD-GIT-001`. All other advanced and remote Git behavior remains open.
