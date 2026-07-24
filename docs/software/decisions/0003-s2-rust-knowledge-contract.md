# Decision 0003: S2 Rust knowledge contract

| Field | Value |
|---|---|
| Status | Accepted; authoritative only after the accountable actor manually merges protected PR [#23](https://github.com/smutti/codenoesis/pull/23) |
| Date | 2026-07-24 |
| Scope | `S2 — Rust knowledge` only |
| Product owner | Andrea Moretti — project governance persona represented by [`@smutti`](https://github.com/smutti), not a separate natural person |
| Technical approver | [`@smutti`](https://github.com/smutti) — sole human maintainer under the single-maintainer bootstrap model |
| Risk | High: public artifacts, ontology, stable identity, claim semantics, parser recovery, and untrusted source input |
| Requirements | `DR-IDN-001`, `FR-EXT-001`, `FR-EXT-002`, `FR-KNW-001`, `FR-KNW-002`, `FR-KNW-003` |
| Issue | [#22](https://github.com/smutti/codenoesis/issues/22) |
| Approval reference | PR [#23](https://github.com/smutti/codenoesis/pull/23); effective only on protected manual merge by `@smutti` |

This record proposes no production implementation by itself. It becomes
authoritative only through a protected manual merge by `@smutti`; the authoring
agent must not approve or merge it. A separate policy-binding change and an
agent-ready implementation issue are required after ratification.

## Context

S0 binds one immutable local Git commit and emits a deterministic artifact. S1
safely inventories its committed tree with evidence, diagnostics, coverage
gaps, fixed limits, and filesystem confinement. S2 is the first semantic slice:
one reviewed Rust fixture must produce stable entities and relationships in a
validated canonical graph.

The architecture intentionally left `OD-ONT-001` open. Production extraction
cannot begin until human review fixes:

1. the smallest useful Rust entity and relationship vocabulary;
2. required properties, endpoint matrices, and cardinalities;
3. canonical symbol normalization and stable identifier preimages;
4. the claim-state model and deterministic-rule provenance;
5. malformed syntax, unresolved symbols, and Unicode collision behavior;
6. public extraction, graph, snapshot, and error contracts;
7. the exact first Red and reviewable graph oracle.

This decision resolves those questions for one local Rust library fixture only.
It does not claim compiler-grade Rust semantics or define a cross-language
ontology.

## Decision

### Explicit S2 profile and compatibility

S2 is selected only by:

```text
noesis scan \
  --repository <local-worktree-root> \
  --repository-id <canonical-logical-id> \
  --revision <full-oid-or-refs/heads/main> \
  --profile standard-local-s2 \
  --format json
```

Success emits one strict `RepositorySnapshotV3` and LF on stdout, no stderr,
and exit `0`. Invalid invocation uses exit `2`; inherited acquisition or
repository-policy failure uses exit `10`; extraction, ontology, or graph
validation failure uses exit `11`; unexpected internal failure uses exit `70`.
Every S2 error is one strict `CodeNoesisErrorV3` and LF on stderr with empty
stdout.

The approved S0 invocation without `--profile` retains V1 behavior. The
approved `standard-local-s1` invocation retains V2 behavior. Dispatch must not
depend on repository shape, source extensions, environment, implicit
configuration, or successful parsing.

### Versioned artifact boundary

`RepositorySnapshotV3` retains the complete S1 semantic inventory and adds:

- ordered `ExtractionChunkV1` values for eligible committed Rust blobs;
- one validated `KnowledgeGraphV1`;
- pipeline `codenoesis.pipeline/s2-v1`;
- ontology `codenoesis.ontology/rust/v1`;
- extractor contract `codenoesis.extraction-chunk/v1`;
- extractor versions
  `codenoesis.inventory-classifier/s1-v1` and
  `codenoesis.rust-tree-sitter/s2-v1`.

The V3 semantic hash uses RFC 8785 bytes and the domain
`codenoesis.repository-snapshot.semantic.v3`. Volatile envelope values remain
outside semantic content. `ExtractionChunkV1` and `KnowledgeGraphV1` each carry
their own BLAKE3 semantic hash so they can later become independently stored
artifacts without changing their meaning.

Publication is all-or-nothing. A malformed chunk contract, invalid identifier,
normalization collision, dangling reference, forbidden endpoint, cardinality
violation, invalid claim state, or invalid derivation publishes neither graph
nor partial snapshot.

### S2 source and crate boundary

S2 consumes only bounded UTF-8 `.rs` blobs already present in the approved S1
inventory and immutable commit. It never reads the dirty worktree. The
ratification fixture has exactly one library root, `src/lib.rs`, and uses
inline modules. The S2 adapter supports:

- crate and inline module scopes;
- structs, enums, traits, type aliases, free functions, trait methods, and
  methods in syntactically named trait implementations;
- simple `crate`, `self`, and `super` use trees, including grouped imports;
- syntactically named `impl Trait for Type` relationships;
- `private`, `public`, and inherited-trait visibility;
- Unicode Rust identifiers normalized as specified below.

Multiple crate roots, binaries, out-of-line module resolution, `include!`,
macro expansion, procedural macros, `cfg` worlds, glob imports, import aliases,
anonymous or unresolved implementation targets, and compiler name/type/call
resolution remain explicit coverage gaps or typed failures as fixed by the
oracle. No unsupported construct may be silently represented as resolved.

### Ontology v1 entities

The exact S2 entity kinds are:

| Kind | Meaning | Required kind-specific properties |
|---|---|---|
| `rust.crate` | The single lexical library-crate root. | `crate_root` |
| `source.file` | One parsed committed Rust blob. | `path` |
| `rust.module` | One named inline Rust module. | `visibility` |
| `rust.struct` | One named struct declaration. | `visibility` |
| `rust.enum` | One named enum declaration. | `visibility` |
| `rust.trait` | One named trait declaration. | `visibility` |
| `rust.type_alias` | One named type alias. | `visibility` |
| `rust.function` | One named free function. | `visibility` |
| `rust.method` | One named trait declaration or resolved trait-implementation method. | `visibility`, `owner_kind` |
| `rust.symbol_reference` | An explicitly unresolved imported or implementation symbol. | `symbol_path`, `resolution` |

Every entity also requires `entity_id`, `kind`, `canonical_identity`,
`display_name`, `language`, and one or more ordered `evidence_ids`. `language`
is `rust` except for `source.file`, where it is also `rust`.
`rust.symbol_reference.resolution` is exactly `unresolved` in S2. It is not a
claim that an external symbol exists.

Fields, enum variants, generic parameters, lifetimes, constants, statics,
macros, closures, expressions, calls, dependencies, services, and documents
are not S2 entities. Their absence is a declared capability limit.

### Ontology v1 relationships and cardinalities

The exact relationship kinds and endpoint matrix are:

| Kind | Allowed source | Allowed target |
|---|---|---|
| `CONTAINS` | `rust.crate` | `source.file` |
| `DEFINES` | `rust.crate`, `rust.module` | `rust.module`, `rust.struct`, `rust.enum`, `rust.trait`, `rust.type_alias`, `rust.function` |
| `DEFINES` | `rust.struct`, `rust.enum`, `rust.trait` | `rust.method` |
| `IMPORTS` | `rust.crate`, `rust.module` | `rust.module`, `rust.struct`, `rust.enum`, `rust.trait`, `rust.type_alias`, `rust.function`, `rust.symbol_reference` |
| `IMPLEMENTS` | `rust.struct`, `rust.enum` | `rust.trait`, `rust.symbol_reference` |

The graph has exactly one `rust.crate`. Every `source.file` has exactly one
incoming `CONTAINS`. Every module, type, function, and method has exactly one
incoming `DEFINES`. A `rust.symbol_reference` has no incoming `DEFINES`, has no
outgoing structural relationship, and must be targeted by at least one
`IMPORTS` or `IMPLEMENTS`.

Every relationship endpoint resolves in the same graph. `DEFINES` is acyclic,
and every declaration is reachable from the crate through `DEFINES`.
Duplicate `(kind, source_entity_id, target_entity_id)` tuples are forbidden.
Repeated source syntax contributes additional ordered evidence to one
relationship rather than creating duplicate relationships.

### Canonical identities and stable IDs

Rust identifier components are normalized to Unicode NFC before constructing a
canonical identity. The exact source spelling remains in `display_name` and
byte evidence. Paths retain the S1 rule: valid UTF-8 bytes are preserved
without Unicode normalization.

Canonical identities are:

- crate: `crate`;
- source file: `file:<canonical-relative-path>`;
- declarations: `crate::<normalized-scope>::<normalized-name>`;
- unresolved references:
  `unresolved:<normalized-lexical-owner>:<normalized-source-path>`.

Kind is a separate identity component, so Rust namespaces do not collide.
Two declarations with the same `(kind, canonical_identity)` are a typed
`extraction.normalization_collision`; the implementation must not choose one,
append an ordinal, use a byte offset, or merge evidence.

Identifier preimages are RFC 8785 canonical JSON arrays encoded as UTF-8:

```text
EntityId:
["codenoesis.entity-id/v1", repository_identity, "rust",
 entity_kind, canonical_identity]

RelationshipId:
["codenoesis.relationship-id/v1", repository_identity,
 "codenoesis.ontology/rust/v1", relationship_kind,
 source_entity_id, target_entity_id]

ClaimId:
["codenoesis.claim-id/v1", repository_identity,
 "codenoesis.ontology/rust/v1", subject_kind, subject_id]

ExtractionChunkId:
["codenoesis.extraction-chunk-id/v1", repository_identity,
 immutable_commit_oid, blob_oid, canonical_relative_path,
 "codenoesis.rust-tree-sitter/s2-v1"]
```

Each digest is BLAKE3-256 lowercase hexadecimal. Public forms are respectively
`urn:codenoesis:entity:blake3:<digest>`,
`urn:codenoesis:relationship:blake3:<digest>`, and
`urn:codenoesis:claim:blake3:<digest>`. Chunk IDs use
`urn:codenoesis:extraction-chunk:blake3:<digest>` and are intentionally
revision-specific, unlike stable graph-subject IDs.

Commit OIDs, blob OIDs, paths other than a `source.file` identity, source
offsets, parser node IDs, insertion order, scheduler order, storage sequence
numbers, and claim state are excluded from these stable IDs. A symbol keeps its
ID across commits only while its canonical identity remains unchanged.

### Extraction and source evidence

The built-in adapter is
`codenoesis.rust-tree-sitter/s2-v1` using the pinned Tree-sitter Rust grammar.
One `ExtractionChunkV1` corresponds to one committed `.rs` blob and declares
repository identity, immutable commit and blob OIDs, canonical path, ontology,
extractor, chunk hash, entities, relationships, claims, evidence, diagnostics,
and coverage.

`SourceEvidenceV2` identifies repository, commit, blob, path, half-open byte
span, extractor version, syntax kind, and deterministic derivation. Every span
must resolve within the exact committed blob and must cover the syntax that
supports the subject. Evidence ordering is `(path UTF-8 bytes, start, end,
evidence_id)`.

Parser chunks may emit only `deterministic_fact`. They may reference a stable
entity in another chunk by its independently computable ID, but graph
publication waits until every endpoint and evidence reference resolves.

### Claims, states, and deterministic rules

Every entity and relationship has exactly one `ClaimV1`; every claim has
exactly one subject. Parser claims use state `deterministic_fact` and a
`parser` derivation containing the extractor version and evidence IDs.

S2 has one graph rule:
`codenoesis.rule.rust-file-containment/s2-v1`. It deterministically derives the
single `CONTAINS` edge from the crate-root claim and source-file claim. Its
claim state is `derived_fact`, and its derivation records the exact rule
version, both ordered input claim IDs, and all supporting evidence IDs.
Derived claims with missing, duplicate, cyclic, non-existent, or
state-incompatible inputs are rejected.

The seven states remain distinct. Exact allowed transitions are:

```text
deterministic_fact -> superseded
derived_fact       -> superseded
candidate          -> reviewed_inference | confirmed | rejected | superseded
reviewed_inference -> confirmed | rejected | superseded
confirmed          -> superseded
rejected           -> superseded
```

`superseded` is terminal. A same-state write is not a transition. Every
unlisted transition is invalid. S2 emits only `deterministic_fact` and
`derived_fact`; the complete state machine is fixed now so later Council or
review code cannot reinterpret graph truth. Model output can create only
`candidate` and can never directly create `deterministic_fact` or `confirmed`.

### Malformed syntax and unresolved meaning

Tree-sitter recovery does not make malformed syntax true. A declaration or
relationship whose supporting span intersects an `ERROR` or `MISSING` node is
not emitted. Unaffected declarations outside those regions may be emitted only
when their complete supporting spans are disjoint from every malformed region.

Each malformed region produces one ordered
`extraction.malformed_syntax` diagnostic and one
`malformed_syntax_excluded` coverage gap with exact path and byte span.
Parser cancellation, invalid UTF-8, root ambiguity, a malformed region without
a bounded span, or inability to establish disjointness fails with no partial
snapshot.

Simple unresolved imports or trait targets become
`rust.symbol_reference` entities plus `unresolved_symbol` coverage gaps. The
graph never links them to a guessed internal declaration. NFC collisions fail
as `extraction.normalization_collision`.

### Ordering, validation, and failure precedence

Chunks are ordered by canonical path bytes. Within chunks and graphs, entities,
relationships, and claims are ordered by their IDs; evidence and malformed
records use their declared span keys. Duplicate IDs, keys, subjects,
relationship tuples, evidence, diagnostics, and gaps are forbidden.

After successful S1 acquisition, failures are selected in this order:

1. source eligibility and UTF-8;
2. parser bound or cancellation;
3. malformed-region safety;
4. Unicode normalization collision;
5. extraction-contract validation;
6. entity and evidence validation;
7. relationship endpoint and matrix validation;
8. cardinality and reachability validation;
9. claim state and derivation validation;
10. canonical-output limit.

Within one class, canonical path and then byte span choose the first failure.
Public error context contains bounded identifiers and relative paths only; it
never contains source excerpts, absolute paths, model output, or environment
data.

### Security and resource boundary

S2 inherits every approved S1 repository, file, path, byte, output, wall,
CPU, RSS, temporary-disk, process, network, and filesystem boundary. Parsing
uses only committed in-memory bytes already acquired by S1. It never invokes
Git, Cargo, rustc, rustfmt, a linker, build script, procedural macro, target
binary, shell, plugin, sidecar, model provider, or network client.

Malformed syntax, nesting, token count, and graph amplification must terminate
inside inherited S1 ceilings. Exceeding a subject limit is typed; exceeding an
external evidence ceiling fails the gate and cannot be rewritten as a subject
error.

### Fixture, Red, and retained evidence

The reviewed fixture is
`tests/fixtures/s2/rust-knowledge-v1/`. It contains a single library file with
an inline module, struct, enum, trait, alias, free functions, trait declaration
method, trait implementation method, grouped imports, one implementation edge,
and a composed Unicode identifier. Generated variants introduce malformed
syntax and canonically equivalent identifier collisions without changing the
reviewed source.

The machine oracle is
`tests/specifications/s2/e2e_fr_ext_002_rust_knowledge.json`. The first
implementation test is `e2e_fr_ext_002_rust_knowledge`, run by:

```text
cargo test --test e2e_fr_ext_002_rust_knowledge
```

Before S2 production changes, merged S1 returns exit `2`, empty stdout, and
`CodeNoesisErrorV2/input.invalid_profile` for `standard-local-s2`. The
acceptance harness expects exit `0`, empty stderr, and reviewed
`RepositorySnapshotV3`; therefore it is Red for the missing S2 profile
behavior. Compilation failure, missing or corrupt fixture, schema harness
failure, parser crash, dependency or network outage, timeout, race, or changed
oracle is not acceptable Red evidence.

The implementation change must retain immutable Red before production edits,
then Green results for every S2 scenario and inherited S0/S1 regression. The
evidence pack includes exact base/head SHAs, commands and exits, fixture and
bundle digests, seeds, shuffled schedules, parser and ontology versions,
process/network/filesystem observations, resource reports, environment, agent
identity, duration, cost, limitations, and human approvals.

## Consequences

- S2 becomes the first product ontology and parser contract, but remains small
  enough for a hand-reviewed graph.
- Stable IDs survive ordering and unrelated commit changes while honest
  normalization collisions fail closed.
- Claims preserve the distinction between parser facts and deterministic
  derivations before heuristic or Council states exist.
- Explicit unsupported and malformed coverage prevents syntax recovery from
  becoming fabricated semantic certainty.
- V3 is opt-in, so S0 and S1 public contracts remain independently testable.

## Deferred

- additional crate roots, binaries, workspaces, examples, tests, and benches;
- out-of-line module and Cargo feature/configuration-world resolution;
- fields, variants, constants, statics, generics, lifetimes, macros, closures,
  and expression entities;
- glob imports, import aliases, extern prelude, re-export semantics, and
  compiler-grade name/type resolution;
- inherent implementations, anonymous targets, call graphs, reads/writes, and
  data flow;
- macro expansion, procedural macros, Cargo/rustc execution, and trusted build
  profiles;
- SCIP, cross-language entities, contracts, dependencies, federation, and
  impact analysis;
- persistence, migrations, queries, documents, REST, MCP, plugins, and models;
- ontology evolution beyond immutable `codenoesis.ontology/rust/v1`.

## Ratification sequence

1. The human reviews entity properties, relationship matrix, cardinalities,
   identity preimages, Unicode behavior, states, malformed handling, schemas,
   fixture, graph golden, and Red meaning.
2. `@smutti` manually merges the protected ratification pull request.
3. A separate protected policy pull request binds exactly the six S2
   requirement IDs to the byte-identical SRS commit.
4. A separate Ready issue fixes one S2 implementation objective, allowed
   production paths, evidence, and stop conditions.
5. Implementation begins with retained expected Red and preserves all S0/S1
   regressions.

The authoring agent must not approve or merge any of those changes.
