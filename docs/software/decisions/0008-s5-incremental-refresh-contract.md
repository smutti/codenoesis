# Decision 0008: S5 deterministic incremental refresh contract

| Field | Value |
|---|---|
| Status | Proposed; becomes Accepted only after protected manual merge of [PR #67](https://github.com/smutti/codenoesis/pull/67) |
| Date | 2026-07-29 |
| Deciders | Andrea Moretti (`@smutti` persona), with protected manual merge by `@smutti` as the approval event |
| Issue | [#66](https://github.com/smutti/codenoesis/issues/66) |
| Requirements | `INV-INC-001`, `FR-INC-001`, `FR-INC-002`, `FR-INC-003`, `FR-CLI-004` |
| Slice | `S5 — Incremental refresh` |
| Risk | High: cache authority, invalidation, public snapshot semantics, atomic publication, public report/error contracts, and protected oracle |
| Scope | Governance only; production implementation, policy binding, workflows, dependencies, and accepted S0–S4 artifacts are excluded |
| Bootstrap correction | protected bootstrap correction pull request `TBD` |

## Context

S4 binds every public source-evidence identity to an immutable commit. That
binding is required for provenance, but it means an unchanged source blob
cannot make its baseline public chunk valid for another commit. Copying the
chunk would retain stale evidence. Removing the commit from evidence identity
would reopen an accepted S4 contract.

Incremental work therefore has two different layers:

1. revision-neutral analysis that may be cached when every semantic input
   matches; and
2. revision-bound public materialization that must be rebuilt for the target
   commit.

The first materialization of the S5 fixture demonstrated this distinction.
All three target `ExtractionChunkV2` hashes changed even though only one source
blob changed. The maintainer approved preserving S4 and introducing an
internal revision-neutral cache in issue #66. The same approval fixes
`no_change` to an already-visible immutable commit: an equal tree under a
different commit still requires target-bound rematerialization.

Under-invalidation can publish stale facts, while over-claiming reuse can make
incorrect evidence appear authoritative. This decision fixes one conservative
Rust-workspace capability, exact rebuild triggers, strict artifacts, bounded
failure, and a reviewed cold-equivalence oracle before production work begins.

## Decision

### Operation and semantic target

The only S5 operation is:

```text
noesis refresh \
  --repository <local-worktree-root> \
  --repository-id <canonical-logical-id> \
  --revision <full-oid-or-refs/heads/main> \
  --store <local-store-root> \
  --profile standard-local-s5
```

Successful execution writes exactly one canonical
`IncrementalRefreshReportV1` JSON document plus LF to stdout, writes no
stderr, and exits `0`. Failure writes exactly one canonical
`CodeNoesisErrorV7` JSON document plus LF to stderr, writes no stdout, and uses
the inherited typed exit family.

`standard-local-s5` selects the refresh use case. It does not become target
semantic configuration. The target remains the accepted
`standard-local-s4` semantic profile with:

- `codenoesis.repository-snapshot/v4`;
- `codenoesis.knowledge-graph/v2`;
- `codenoesis.extraction-chunk/v2`;
- `codenoesis.pipeline/s4-v1`;
- `codenoesis.ontology/rust/v2`;
- `codenoesis.extraction/v2`;
- `codenoesis.evidence-lineage/v2`;
- `codenoesis.renderer/markdown-v1`.

This separation permits a byte-for-byte comparison with a clean S4 scan of the
same target commit. S5 does not migrate or amend any S4 public artifact.

### Baseline and target binding

The baseline is the validated visible S4 head for the exact repository
identity. Missing head, repository mismatch, corrupt metadata, missing or
corrupt reachable content, or a non-S4 head fails before cache inspection.

The target is independently bound to one verified immutable local Git commit
through the accepted in-process acquisition boundary. Revision names are
resolved once. Repository hooks, Git processes, alternates, filters,
credentials, target code, and network access are not used.

The implementation compares the baseline and target trees in process. Changed
paths are canonical relative repository paths, sorted by UTF-8 bytes, unique,
and classified as `added`, `modified`, or `deleted`. Rename inference is not
performed; a rename is one delete plus one add.

The visible head observed before planning is the publication precondition. It
must still be visible at commit time. Concurrent movement returns the inherited
retryable `publication.head_conflict`; it never rebases a plan or publishes
against a different baseline.

### Revision-neutral analysis cache

`AnalysisCacheEntryV1` is an internal optimization, not a public snapshot fact.
It is never authoritative until validated observations are rematerialized,
assembled, validated, and published through the S3 boundary.

Its stable identity is:

```text
urn:codenoesis:analysis-cache-entry:blake3:<digest>
```

where `<digest>` is BLAKE3-256 over the RFC 8785 canonical JSON array:

```text
[
  "codenoesis.analysis-cache-entry-id/rust-workspace/v1",
  repository_identity,
  source_file_id,
  canonical_source_path,
  source_blob_oid,
  crate_id,
  canonical_module_path,
  "codenoesis.analysis-cache-entry/v1",
  "codenoesis.rust-tree-sitter/s4-v1",
  "codenoesis.rust-workspace/s4-v1",
  "codenoesis.normalization/rust-workspace/v1",
  "codenoesis.ontology/rust/v2",
  "codenoesis.extraction/v2",
  "standard-local-s4",
  "codenoesis.incremental-rules/rust-workspace-v1"
]
```

The key includes repository identity, stable ownership and module mapping,
path, blob, both extractor versions, normalization, ontology, extraction
contract, semantic profile, cache schema, and dependency-rule version. It
excludes commit, tree, snapshot, job, clock, correlation, and envelope values.

The cache payload contains only closed revision-neutral parser observations,
local observation keys, deterministic properties, dependency observations,
coverage observations, and source-relative byte spans. It contains no public
evidence, claim, relationship, chunk, snapshot, document, statement, commit,
or report identity. Its `payload_hash` is BLAKE3-256 over:

```text
"codenoesis.analysis-cache-payload/rust-workspace/v1"
0x00
RFC8785(entry_without_analysis_cache_entry_id_and_payload_hash)
```

A cache hit requires exact identity, exact payload hash, complete schema
validation, and an exact key recomputation. A missing entry is a cache miss.
A corrupt entry is `incremental.cache_corrupt`; it is not silently ignored,
repaired, or presented as a cold-equivalent success. Incompatible versioned
bytes are excluded by a classified rebuild and are never deserialized as the
current schema.

Cache writes may be orphaned by a failed attempt and later collected, but they
cannot advance the visible head. Their presence is not evidence that a public
fact was published.

### Rule catalog and precedence

The exact catalog is
`codenoesis.incremental-rules/rust-workspace-v1`. If more than one rule
matches, the fail-closed outcome precedence is:

```text
error
full_rebuild
full_workspace_analysis
partial_analysis
inventory_only
no_change
```

The v1 outcomes are:

| Outcome | Exact condition | Analysis behavior |
|---|---|---|
| `error` | Missing, mismatched, corrupt, or concurrently moved baseline; corrupt current cache; exceeded bound; target acquisition or publication failure. | No target head. |
| `full_rebuild` | Extractor, workspace mapper, normalization, ontology, extraction contract, semantic profile, dependency rules, cache schema, snapshot schema, or another public schema differs. | Invalidate every prior analysis entry and recompute all supported target analysis. |
| `full_workspace_analysis` | Source add/delete; deterministic delete-plus-add rename; root or module-declaration change; manifest, target, membership, workspace, or ownership change; unsupported or ambiguous mapping; conservative fallback. | Recompute every supported Rust source in the target workspace. |
| `partial_analysis` | One or more modified already-mapped non-root Rust sources retain exact workspace, crate, source, and module mapping and no stronger rule matches. | Recompute only those source analysis entries and reuse every exact compatible hit outside that set. |
| `inventory_only` | Changed paths affect inventory but no approved extraction input, mapping, contract, or version. | Reuse all exact analysis entries and rebuild target inventory. |
| `no_change` | The requested immutable commit is already the validated visible head and every compatible version input matches. | No analysis, materialization, or head advance. |

Equal tree bytes under a different commit are not `no_change`. They select
public target rematerialization because S4 evidence is commit-bound. A
full-workspace outcome does not make an unsupported target valid: the complete
S4 target validation may still fail with its inherited typed error.

### Public rematerialization and invalidation

For every non-`no_change` target, the implementation rematerializes:

1. target inventory and revision-bound inventory evidence;
2. target source evidence;
3. every target `ExtractionChunkV2`;
4. the complete `KnowledgeGraphV2`;
5. the complete `RepositorySnapshotV4`;
6. the deterministic `DocumentationManifestV1` projection and Markdown byte
   hashes used by the equivalence oracle;
7. the `IncrementalRefreshReportV1`.

Revision-neutral analysis reuse never authorizes copying baseline public chunk
bytes. Every evidence reference in a target chunk resolves to the target
commit. Changed-source evidence also names the target blob and exact target
span.

The report distinguishes:

- reused, invalidated, and recomputed analysis-cache entry identities;
- unchanged and reclassified inventory paths;
- rematerialized public chunk hashes;
- invalidated, added, removed, and retained counts for entities,
  relationships, claims, evidence, coverage gaps, documents, and future
  federation links;
- document manifest rematerialization from changed Markdown content;
- deterministic invalidation metrics from volatile execution measurements.

Invalidated identifier sets are exact, sorted, unique, and globally bounded.
An invalidated public identity may be retained in the target when its stable
identity is unchanged but its target-bound payload changes. Added and removed
sets describe identity membership, not cache activity.

`noesis refresh` does not publish to a caller-selected documentation directory.
It computes the deterministic target documentation projection for comparison
and reporting. A subsequent accepted `noesis docs` operation over the target
head must emit the same manifest and Markdown bytes as a clean target store.

### Cold equivalence and publication

Before publication, the incrementally composed target semantic payload must be
RFC 8785 byte-identical to the reviewed clean S4 target payload. Its snapshot
semantic hash, graph semantic hash, chunk hashes, generated-document projection,
and documentation generation hash must match.

The implementation must not perform a second cold scan and label it
incremental. The acceptance harness observes parser invocations and proves that
only the changed fixture source is reparsed while exact unaffected analysis
entries are consumed.

Any equivalence mismatch returns `incremental.cold_equivalence_failed`, emits
no success report, and leaves the baseline head visible. The mismatch cannot
be waived, retried into success, or repaired by changing the golden oracle.

After complete validation, publication uses the accepted S3 durable staging
and compare-and-swap head transition. Readers observe the complete baseline or
the complete target, never a mixture. Repeating a completed request sees the
same commit already visible, returns a deterministic `no_change` report, and
does not create another head transition.

### Report identity and canonical bytes

`IncrementalRefreshReportV1` is a deterministic semantic artifact. Its
`semantic_hash.value` is BLAKE3-256 over:

```text
"codenoesis.incremental-refresh-report.semantic.v1"
0x00
RFC8785(report_without_semantic_hash)
```

The report includes all cache, rule, extractor, normalization, ontology,
extraction, semantic-profile, snapshot, renderer, and evidence-lineage versions
needed to interpret it. Clocks, durations, process IDs, job IDs, correlation
IDs, host paths, and retry counts are excluded. They may appear only in a
separate volatile envelope introduced by a later approved interface contract.

Set-like lists are sorted by UTF-8 bytes and reject duplicates. Path lists are
canonical, relative, sorted, and unique. Chunk and document records are sorted
by their stable source or document identity.

### Error contract and limits

`CodeNoesisErrorV7` preserves applicable acquisition, extraction, graph,
storage, and publication meanings and adds:

- `incremental.baseline_missing`;
- `incremental.baseline_repository_mismatch`;
- `incremental.baseline_incompatible`;
- `incremental.cache_corrupt`;
- `incremental.limit_exceeded`;
- `incremental.cold_equivalence_failed`.

Only inherited `publication.head_conflict` and `storage.writer_busy` are
retryable. Input, corruption, limit, and equivalence failures are not.

Fixed v1 maxima are:

| Resource | Maximum |
|---|---:|
| Changed paths | 100,000 |
| Analysis entries considered | 100,000 |
| Dependency edges traversed | 1,000,000 |
| Subject IDs across report sets | 1,000,000 |
| Serialized report bytes | 16,777,216 |
| Refresh wall time | 60,000 ms |

Maximum succeeds when all other inputs are valid. Maximum plus one returns the
typed limit outcome, emits no partial report, and does not advance the head.
Wall time is an operational bound and is not semantic report content.

### Security and deterministic operation

The standard refresh path invokes no Git command, shell, Cargo, `rustc`, build
script, procedural macro, target binary, hook, credential helper, plugin,
network, model provider, or Council. It does not follow symlinks or read outside
the accepted repository and store roots.

Repository bytes, cache bytes, stored artifacts, schemas, paths, and generated
content are untrusted. Every boundary validates before use. Model output cannot
create a cache hit, invalidation fact, equivalence result, or publication
decision.

## Acceptance oracle

The project-owned fixture is
[`incremental-refresh-v1`](../../../tests/fixtures/s5/incremental-refresh-v1/README.md).
It binds two deterministic Git commits where only
`crates/model/src/item.rs` changes and adds one public free function.

The reviewed oracle requires:

1. exactly one `modified` path and no rename inference;
2. two exact analysis-cache hits and one invalidated/recomputed entry;
3. exactly one parser invocation for the changed source;
4. all three target public chunks rematerialized;
5. no target chunk containing baseline-commit evidence;
6. exact entity, relationship, claim, evidence, coverage, document, and empty
   federation-link invalidation sets;
7. exact baseline and target snapshot, graph, chunk, document, and generation
   hashes;
8. target incremental semantic bytes equal reviewed cold target bytes;
9. baseline remains visible at every pre-publication failpoint;
10. no target, build, process, network, plugin, model, or Council execution.

After protected governance merge and separate policy binding, the first
production acceptance test is:

```text
cargo test -p noesis --test e2e_fr_inc_001_incremental_refresh --locked -- --exact e2e_fr_inc_001_incremental_refresh
```

It must first be observed Red because the current runtime can create the S4
baseline but does not yet recognize the `refresh` command. The exact invocation
therefore exits `2`, emits ErrorV2 `input.invalid_revision` on stderr, emits no
stdout, creates no store, and publishes no target head. This is a bootstrap-only
observation; every implemented S5 failure still uses the strict
`CodeNoesisErrorV7` contract. Compilation failure, missing fixture, schema
failure, panic, timeout, target execution, two cold scans mislabeled as reuse,
stale public chunk reuse, or a weakened equivalence oracle is not acceptable
Red evidence. No production Red runs in this governance package while the
requirements are Proposed and unbound.

## Implementation constraints

- Domain invalidation, cache-key, and equivalence logic remains independent of
  Tokio, SQLx, Axum, filesystem APIs, MCP, and model-provider SDKs.
- Repository, cache, storage, clock, parser, and publication adapters implement
  inward-owned ports; interfaces contain no invalidation policy.
- The cache is an optimization. Deleting valid cache bytes can reduce reuse but
  cannot change target semantic output.
- Missing cache is recomputed; corrupt current cache fails closed.
- No first-party `unsafe`, target execution, network repair, hidden compiler
  fallback, or automatic golden refresh is authorized.
- The first implementation issue must preserve every accepted S4 artifact byte
  and include observed Red, Green, regression, fixture, and traceability
  evidence.

## Consequences

- Parsing and source-level analysis can be reused without weakening
  commit-bound provenance.
- Public chunk hashes may all change for a new commit even when most analysis
  entries are cache hits; the report explains both facts.
- The first S5 implementation is intentionally conservative around workspace
  topology and version changes.
- Cold equivalence remains a release-blocking invariant rather than a
  performance assumption.
- Later languages can define their own cache keys and dependency catalogs
  without changing this Rust-workspace v1 meaning.

## Deferred

- Added, deleted, or renamed source partial analysis;
- fine-grained root/module/manifest/workspace invalidation;
- compiler, Cargo metadata, macro, generated-source, or trusted-build caches;
- cache sharing across repositories, tenants, machines, or semantic profiles;
- remote cache protocols, eviction policy, compression, encryption, and cache
  observability envelopes;
- non-Rust dependency catalogs;
- public documentation-directory publication during refresh;
- incremental S6 federation and S7 impact analysis;
- server, REST, MCP, job, tenant, and distributed publication behavior.

## Approval and separation

This decision, strict cache/report/error schemas, rule catalog, acceptance
specification, project-owned fixture, reviewed cold summaries and report,
maintenance guard, and contract bundle form one protected governance package.
The policy registry remains unchanged. Production code, dependencies,
workflows, architecture, roadmap, and every accepted S0–S4 artifact remain
unchanged.

Approval occurs only when `@smutti` manually merges the exact protected pull
request head after every required gate is green. The authoring agent does not
approve or merge. A separate protected policy pull request may then bind only
`INV-INC-001`, `FR-INC-001`, `FR-INC-002`, `FR-INC-003`, and `FR-CLI-004` to
the exact merged SRS. Production implementation requires a separate Ready
issue and fresh explicit approval for its high-risk scope.
