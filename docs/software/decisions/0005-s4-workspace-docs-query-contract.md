# ADR 0005: S4 Rust workspace documentation and exact-ID query contract

| Field | Value |
|---|---|
| Status | Accepted; content-complete hash amendment effective only on protected squash merge of PR #49 by `@smutti` |
| Date | 2026-07-27 |
| Deciders | Andrea Moretti governance persona, represented by accountable actor `@smutti` |
| Technical approver | `@smutti` |
| Issue | [#41](https://github.com/smutti/codenoesis/issues/41) |
| Integrity amendment | [Issue #48](https://github.com/smutti/codenoesis/issues/48), [PR #49](https://github.com/smutti/codenoesis/pull/49) |
| Slice | `S4 — Evidence-backed workspace docs` |
| Requirements | `DR-IDN-002`, `FR-EXT-007`, `FR-DOC-001`, `FR-DOC-002`, `FR-DOC-003`, `FR-QRY-001`, `FR-CLI-001` |
| Risk | High |

## Context

S0 through S3 establish immutable Git binding, bounded inventory, a
syntax-derived Rust graph, and atomic local persistence. The first Rust
ontology deliberately accepts only one library root with inline modules. That
shape proves the graph contract but rejects ordinary multi-crate repositories
before a user can generate documentation.

S4 must create the first complete local product journey:

```text
immutable revision -> stored workspace graph -> generated docs -> exact query
```

The journey must remain deterministic, offline, evidence-backed, and honest
about unsupported Rust meaning. It must not execute Cargo or target code, and
it must not turn generated prose into an untraceable second source of truth.

This decision resolves the S4 subset only. It does not provide general Cargo
evaluation, compiler-grade resolution, arbitrary graph traversal, or a
production server.

## Decision

### Explicit S4 operations

S4 scan is selected only by:

```text
noesis scan \
  --repository <local-worktree-root> \
  --repository-id <canonical-logical-id> \
  --revision <full-oid-or-refs/heads/main> \
  --profile standard-local-s4 \
  --store <local-store-root> \
  --format json
```

Successful scan emits one strict `RepositorySnapshotV4` document plus LF,
writes no stderr, exits `0`, and atomically publishes the same semantic
snapshot through the approved S3 local-store boundary.

Documentation is generated only by:

```text
noesis docs \
  --store <local-store-root> \
  --repository-id <canonical-logical-id> \
  --output <generated-document-root> \
  --format json
```

Successful docs generation emits one strict `DocumentationManifestV1` plus LF,
writes no stderr, exits `0`, and publishes a complete marker-owned Markdown
generation beneath the explicit output root.

Exact local lookup is provided only by:

```text
noesis query \
  --store <local-store-root> \
  --repository-id <canonical-logical-id> \
  --documents <generated-document-root> \
  --id <stable-id> \
  --format json
```

Successful query emits one strict `LocalQueryResultV1` plus LF, writes no
stderr, and exits `0`.

Argument order is not semantic. Duplicate, missing, empty, unknown, or
incompatible arguments fail before acquisition, store access, or output-root
creation. Environment variables and ambient configuration do not select S4 or
its roots.

### Compatibility

The invocations approved for S0, S1, S2, and S3 retain their existing
snapshots, errors, stream behavior, semantic hashes, and write boundaries.
Rust ontology v1, `ExtractionChunkV1`, `KnowledgeGraphV1`, and
`RepositorySnapshotV3` remain immutable.

S4 introduces, rather than mutates:

- `codenoesis.ontology/rust/v2`;
- `codenoesis.extraction-chunk/v2`;
- `codenoesis.knowledge-graph/v2`;
- `codenoesis.repository-snapshot/v4`;
- `codenoesis.documentation-manifest/v1`;
- `codenoesis.local-query-result/v1`;
- S4-specific `codenoesis.error/v5` outcomes.

S4 may store V4 canonical snapshot, graph, and extraction artifacts in the
fresh `codenoesis.local-store/v1` schema because its rows and CAS roles are
version-carrying and ontology-neutral. This decision adds no table, role,
migration, repair, or alternate head rule. If implementation proves that the
v1 store cannot represent V4 without changing its approved meaning, work stops
for a new storage decision.

### Supported committed workspace shape

The S4 standard extractor accepts a root UTF-8 `Cargo.toml` whose `[workspace]`
table has a literal `members` string array. Member paths must already satisfy
the approved S1 normalized repository-path rules and must identify unique
committed UTF-8 manifests.

The supported manifest subset is:

- root `workspace.members` and `workspace.resolver`;
- member `package.name`, `package.version`, and `package.edition`;
- optional literal `package.build`, recorded but never executed;
- one conventional or explicit `[lib]` target;
- zero or more explicit `[[bin]]` targets;
- literal target `name` and `path`;
- literal path dependencies used as evidence-backed package dependency
  declarations.

One member may contribute one library and bounded binary targets. Target roots
default to `src/lib.rs` and `src/main.rs` only when the corresponding target
exists unambiguously. Package names, target names, manifest paths, and target
kinds must be unique under the ratified identity rules.

The first fixture fixes two literal members, one library, one binary, one path
dependency, one out-of-line module, and one sentinel build script.

The following are unsupported in S4 standard mode:

- workspace-member glob expansion or `exclude` interaction;
- generated, inherited, patched, or target-specific manifest evaluation;
- Cargo feature or `cfg` worlds;
- build scripts, Cargo, rustc, proc macros, or target execution;
- external registries, Git dependencies, network resolution, and lockfile
  solving;
- multiple files that could own the same module;
- `#[path]`, `include!`, macro-generated modules, and conditional modules;
- compiler-grade name, type, trait, call, data-flow, or macro resolution.

Unsupported workspace shape fails with a typed extraction result before
publication. Supported syntax with unresolved meaning produces stable
diagnostics and coverage gaps; it is never silently promoted to a resolved
fact.

### Rust ontology v2

Rust ontology v2 retains the v1 entity kinds:

- `rust.crate`;
- `source.file`;
- `rust.module`;
- `rust.struct`;
- `rust.enum`;
- `rust.trait`;
- `rust.type_alias`;
- `rust.function`;
- `rust.method`;
- `rust.symbol_reference`.

It retains `CONTAINS`, `DEFINES`, `IMPORTS`, and `IMPLEMENTS`, while changing
only the cardinality and ownership rules required for multiple crates and
out-of-line modules.

One graph contains between one and 200 crate roots. Every Rust source file is
contained by exactly one crate. Every crate has exactly one root module per
target. Every resolved module has exactly one lexical owner and exactly one
source-file definition. An out-of-line declaration resolves to exactly one of
`name.rs` or `name/mod.rs`; both or neither is a typed ambiguity or coverage
outcome according to the oracle.

Each entity and relationship has exactly one versioned claim. Parser facts and
deterministic derivations retain the v1 claim-state boundary. Cross-crate uses
without compiler-grade resolution remain `rust.symbol_reference` entities with
coverage explaining the missing capability. A literal path dependency is a
manifest fact and does not prove a Rust import target.

### Stable identity

Every identity uses BLAKE3-256 over RFC 8785 canonical JSON. UTF-8 strings use
NFC for Rust identifiers and the already-approved path normalization for
repository paths. The public prefix determines the identity kind.

Crate entity preimage:

```json
[
  "codenoesis.entity-id/rust/v2",
  "<repository-identity>",
  "crate",
  "<manifest-relative-path>",
  "<package-name>",
  "<target-kind>",
  "<target-name>"
]
```

Source-file entity preimage:

```json
[
  "codenoesis.entity-id/rust/v2",
  "<repository-identity>",
  "source_file",
  "<crate-id>",
  "<source-relative-path>"
]
```

Module and declaration preimages use repository identity, entity kind, crate
ID, canonical Rust module path, and declaration name. Relationship IDs use:

```json
[
  "codenoesis.relationship-id/rust/v2",
  "<relationship-kind>",
  "<source-entity-id>",
  "<target-entity-id>"
]
```

Public forms are:

- `urn:codenoesis:entity:blake3:<digest>`;
- `urn:codenoesis:relationship:blake3:<digest>`;
- `urn:codenoesis:claim:blake3:<digest>`;
- `urn:codenoesis:evidence:blake3:<digest>`;
- `urn:codenoesis:coverage-gap:blake3:<digest>`.

Claim, evidence, and coverage-gap preimages are respectively:

```json
[
  "codenoesis.claim-id/v2",
  "<subject-kind>",
  "<subject-id>",
  "<claim-state>"
]
```

```json
[
  "codenoesis.evidence-id/v2",
  "<repository-identity>",
  "<commit-oid>",
  "<blob-oid>",
  "<repository-relative-path>",
  "<start-byte-as-decimal-string>",
  "<end-byte-as-decimal-string>"
]
```

```json
[
  "codenoesis.coverage-gap-id/v2",
  "<repository-identity>",
  "<commit-oid>",
  "<capability>",
  "<evidence-id>"
]
```

Insertion order, member order, file order, commit OID, source offsets,
timestamps, storage IDs, and scheduler order never enter crate, module, or
declaration identity. Commit/blob/span remain part of evidence identity.
Canonical collisions fail closed.

### V4 snapshot and local publication

`RepositorySnapshotV4` retains the V3 envelope/semantic split and adds only the
versioned V2 extraction and graph contracts. Its semantic configuration names
`standard-local-s4`; operational store/docs paths never enter semantic
identity.

Every S4 semantic digest uses BLAKE3-256 over the UTF-8 domain bytes, one
`0x00` separator byte, and the RFC 8785 canonical JSON payload:

- snapshot domain `codenoesis.repository-snapshot.semantic.v4` covers the
  complete `RepositorySnapshotV4.semantic` object;
- graph domain `codenoesis.knowledge-graph.semantic.v2` covers the complete
  `KnowledgeGraphV2` object with only its `semantic_hash` member omitted;
- extraction domain `codenoesis.extraction-chunk.semantic.v2` covers the
  complete `ExtractionChunkV2` object with only its `semantic_hash` member
  omitted.

The machine-readable
`tests/specifications/s4/semantic-hash-contract-v1.json` and complete reviewed
`tests/fixtures/s4/workspace-docs-v1/expected-snapshot-semantic.json` payload
are normative. Graph entity, relationship, claim, evidence, and coverage
collections are stable-ID sorted. Diagnostics are code/evidence sorted.
Extraction chunks are source-path byte sorted; their entity, relationship,
claim, evidence, and coverage collections use the same stable-ID ordering.
Ordered evidence lists retain derivation order.

For the reviewed fixture the corrected graph hash is
`87052fa7112e5a0f7fc9ce075b5d78e1a051e666d2ca0a1446c0ca20f0a57df2`,
the source-path-ordered chunk hashes are
`2f02d8ea27481b49a2457016aeba06e7eccad9758387c534c0d66ec03ef11dce`,
`846a06f881313afc7402807ad1fd43912a55f3c86dad45b612242d8f3c03cc78`,
and `31b7394b72daa8e2dd5d4fcc11a43f34bc34abeb44e8f498d0eadbb0f7cbd915`.
The resulting snapshot semantic hash is
`6ed66dd0d5bf2451087fcb17c254048084d9d0f1bd2ea51062d46a38d1defe31`
and its existing v1 snapshot-ID derivation yields
`urn:codenoesis:snapshot:blake3:832888db94f6bf06da375a1cfc055a7c9b80d624b0596a1e45d1f9c646af9b8f`.
Changing any material nested field changes its enclosing digest and the
snapshot digest. Envelope fields remain excluded.

Canonical S4 artifact roles remain exactly those approved by S3:

1. one snapshot semantic payload;
2. one knowledge graph;
3. ordered extraction chunks.

The S3 CAS staging, SQLite transaction, head visibility, restart, corruption,
retry, cleanup, and path-safety semantics apply unchanged. A docs generation
reads one fully validated stored head and binds its output to that exact
snapshot ID and semantic hash.

### Generated-document root and publication

The explicit docs output root is either absent or already marked
`codenoesis.generated-docs/v1`. A non-empty unmarked root, a symlink or reparse
component, unsafe overlap with the store, or an unsafe path fails before any
owned output changes.

The generator owns only:

- `manifest.json`;
- `overview.md`;
- `modules/*.md`;
- private temporary paths named by the v1 marker contract.

It never overwrites, removes, follows, or adopts an unowned file. A marked root
containing an unrecognized path fails closed.

The generator stages every Markdown file and a candidate manifest, syncs and
hash-validates them, then atomically publishes `manifest.json` as the sole
generation commit point. Before that point, readers see the previous complete
generation or none. After it, every listed file must exist with the recorded
BLAKE3 digest and byte length. Abandoned temporary files are not documents.

`DocumentationManifestV1` is path-sorted and contains:

- repository identity;
- snapshot ID and semantic hash;
- renderer version;
- generation hash;
- each document ID, kind, subject ID, path, byte length, and content digest;
- every material statement ID, truth state, and ordered evidence or
  coverage-gap references.

Document IDs derive from repository identity, document kind, stable subject ID,
and renderer version. Output location, snapshot envelope, timestamp, and file
order do not enter the ID. Content digest and generation hash expose changed
meaning.

The fixed Markdown v1 bundle contains exactly:

- `overview.md`;
- one `modules/<stable-slug>.md` page per resolved module.

Every non-heading material line carries one HTML marker:

```text
<!-- statement:urn:codenoesis:statement:blake3:<digest> -->
```

The manifest resolves that statement to either one or more evidence IDs or one
or more coverage-gap IDs, never both. Unsupported statements are visibly
labelled `Unsupported`; absence is not interpreted as completeness.

### Exact-ID query

S4 query is not traversal or search. It performs one exact lookup against:

- an entity ID;
- a claim ID;
- an evidence ID;
- a generated document ID.

The stored snapshot and docs manifest must bind the same repository identity
and snapshot ID. A mismatch or corrupt referenced byte fails closed.

An entity result includes the exact entity, its single claim, ordered evidence,
and every generated statement that references it. Claim and evidence results
include their exact record and resolvable links. A document result includes its
manifest record and ordered statements. The result exposes truth and coverage
states without inference.

Unknown IDs return `query.not_found`; they do not return an empty successful
array. Results are canonical JSON, bounded, and versioned.

### Errors and exits

S4-specific failures use strict `CodeNoesisErrorV5`, empty stdout, one
LF-terminated stderr document, and these exit classes:

| Class | Exit |
|---|---:|
| input | `2` |
| acquisition | `10` |
| extraction/graph | `11` |
| storage/publication | `12` |
| docs | `13` |
| query | `14` |
| internal | `70` |

New stable codes are:

- `input.invalid_output_root`;
- `input.invalid_documents_root`;
- `input.invalid_query_id`;
- `extraction.unsupported_workspace`;
- `extraction.ambiguous_module`;
- `docs.unmarked_nonempty_root`;
- `docs.unsafe_path`;
- `docs.snapshot_mismatch`;
- `docs.corrupt_generation`;
- `docs.failed`;
- `query.not_found`;
- `query.snapshot_mismatch`;
- `query.corrupt_documents`;
- `query.result_limit_exceeded`.

Inherited S0–S3 failures preserve their already-approved schema and bytes.
Failure precedence is input, acquisition, workspace/extraction, graph
validation, store publication, docs-root validation, docs generation, query
validation, output serialization.

### Fixed limits

S4 inherits every S1 acquisition and process limit and fixes:

| Resource | Maximum |
|---|---:|
| workspace crates | `200` |
| generated documents | `2,001` |
| bytes per document | `1,048,576` |
| total generated bytes | `33,554,432` |
| material statements | `200,000` |
| query result bytes | `4,194,304` |

The document count is one overview plus at most 2,000 resolved module pages.
Limit checks occur before publication and return typed failures without
changing the visible store head or docs generation.

### Security and deterministic operation

Repositories, manifests, source, stored bytes, docs roots, and query IDs are
untrusted. S4 performs no network access, target execution, shell invocation,
model call, plugin execution, or write outside the explicit store and
generated-doc roots.

The sentinel build script in the fixture must never execute. Filesystem
boundaries reject traversal, symlinks/reparse points, unsafe overlap, and
unowned content. Rendering uses fixed templates and escapes source-derived text
so Markdown cannot create executable HTML or active links outside the reviewed
format.

No LLM or Council participates in S4 truth or prose generation.

## Acceptance oracle

The first black-box test is
`e2e_fr_cli_001_workspace_docs_query`.

The ordered journey:

1. materialize the reviewed immutable two-member workspace;
2. run S4 scan and compare V4 schema, semantic digests, graph counts, identity
   examples, diagnostics, and coverage;
3. restart the process and run docs into an absent output root;
4. compare every Markdown byte and `DocumentationManifestV1`;
5. rerun docs with changed envelope/order and prove byte identity;
6. query the reviewed entity and document IDs;
7. reject unknown IDs, unmarked roots, unsupported workspace forms,
   mismatched snapshots, corruption, traversal, and limit-plus-one cases;
8. prove build-script/network/process sentinels remain untouched;
9. rerun every inherited S0–S3 regression.

Before S4 production changes, merged S3 must reject
`--profile standard-local-s4` with exit `2` and the inherited strict
`input.invalid_profile` result before creating a store or docs root. A compile
failure, missing target, corrupt fixture, guard/schema failure, dependency
outage, timeout, panic, partial write, or changed oracle is not acceptable Red
evidence.

## Implementation constraints

The later production issue must name exact dependencies and paths. Under the
recorded fast-track authorization it may consider only safe MIT or Apache-2.0
dependencies, stable Rust, no first-party `unsafe`, and deterministic locked
resolution.

Domain workspace, document, statement, and query semantics remain independent
of filesystem, SQLite, Tokio, CLI, and parser SDKs. Application services own
ports. Adapters parse manifests/source, persist bytes, and publish generated
files without business rules.

Five bounded correction rounds are available only in the later implementation
issue. A changed oracle, ontology, public schema, identity, output ownership,
storage migration, dependency class, or risk remains human-required regardless
of that budget.

## Consequences

S4 creates the first human-readable local product and supports common literal
Rust workspaces without executing Cargo. It deliberately remains syntax-only
and exact-ID-only. Users receive explicit coverage gaps where compiler meaning
is unavailable.

The additional V4/V2 contracts preserve every earlier profile rather than
retroactively changing v1 identities. The generated docs root is independently
versioned and does not require a local-store schema migration.

## Deferred

- workspace globs, feature and target worlds, build output, and lock solving;
- `#[path]`, `include!`, macro expansion, proc macros, and generated modules;
- compiler-grade name/type/trait/call/data-flow resolution;
- arbitrary traversal, full-text/fuzzy search, ranking, and pagination;
- HTML/site generation and hand-written document updates;
- incremental regeneration;
- server/API/MCP paths;
- model-generated prose;
- store migrations and document artifacts stored as S3 CAS roles.

## Approval and separation

The original ADR became Accepted through the protected manual squash merge of
PR #42 by `@smutti`. Its separate policy-binding change bound the resulting
`main` SRS commit before autonomous production implementation became Ready.

The content-complete hash amendment was explicitly authorized by `@smutti`
after issue #47 demonstrated that the original fixture digest used an
undocumented summary preimage. The amendment becomes effective only through
protected manual squash merge of PR #49. A separate protected policy-binding
change must then bind the resulting `main` commit before S4 implementation
resumes.

The authoring agent must not approve or merge this decision. Independent human
review must inspect the fixture, identities, ontology, semantic hash
preimages, complete snapshot payload, document bytes, statement evidence,
query results, error behavior, limits, and compatibility without inheriting
the authoring agent's conclusions as facts.
