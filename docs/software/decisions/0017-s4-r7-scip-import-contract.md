# Decision 0017: S4 R7 revision-bound SCIP import contract

| Field | Value |
|---|---|
| Status | Proposed; becomes Accepted only when `@smutti` manually merges the exact independently reviewed protected pull request governed by issue #123 |
| Date | 2026-08-05 |
| Owners | Andrea Moretti (`@smutti` governance persona), accountable maintainer `@smutti` |
| Scope | `S4 — Evidence-backed workspace docs compatibility extension` only; roadmap `R7` static import |
| Requirement | Bounded approval of `FR-EXT-005` for `codenoesis.compiler-index-profile/scip-rust-v0.9.0-import-v1`; the broader polyglot requirement remains Proposed |
| Risk | High — public contracts, ontology, untrusted binary parsing, evidence precedence, revision binding, resource safety, privacy, and supply chain |
| Governance issue | [#123](https://github.com/smutti/codenoesis/issues/123) |
| Authorization | [Accountable-maintainer authorization](https://github.com/smutti/codenoesis/issues/123#issuecomment-5193618752) |
| Supply-chain correction | [Correction 1/4](https://github.com/smutti/codenoesis/issues/123#issuecomment-5193814091) |
| Required base | `3f750201a1527c85ed2ce83f70ed0932213f3548` |
| Immutable predecessor | R6 bundle `sha256:46f5e0fab0439979c456cb41ce7195efd5e02a342be4292402ef2cb44909bc47` |

## Context

R6 represents committed Rust declarations and bounded framework-neutral source
forms without running repository code or claiming runtime behavior. Syntax-only
analysis intentionally cannot resolve every cross-crate symbol, trait
implementation, type-definition relation, generated symbol, or ambiguous
reference. A compiler-grade index can add useful evidence, but accepting one
without binding it to the acquired revision would let stale, mismatched, or
host-controlled bytes become product facts.

This decision ratifies only static import of one explicitly supplied,
pre-generated Rust SCIP artifact. It does not authorize CodeNoesis to install,
launch, or orchestrate a compiler, Cargo, rust-analyzer, build script, proc
macro, target, plugin, sidecar, model provider, or network client. Artifact
generation remains future S9-compatible trusted-build or sandbox work.

SCIP v0.9.0 also does not provide a reliable Rust call-kind relationship.
An identifier classified as function syntax may be referenced without being
called. Treating every such reference as a call would violate `INV-MDL-001`.

## Decision

Add the explicit selector pair:

```text
--compiler-index-profile scip-rust-v0.9.0-import-v1
--compiler-index-binding <safe-relative-path>
```

It is valid only with the complete explicit composition:

```text
scan --profile standard-local-s4
--workspace-profile cargo-root-package-v1
--manifest-profile cargo-manifest-facts-v1
--rust-semantic-profile rust-semantic-depth-v1
--rust-framework-profile rust-framework-declarations-v1
```

The binding names the adjacent raw artifact by a safe relative path. Repository
content, environment, dependency names, a conventional `index.scip` filename,
prior selectors, or a previously imported artifact never select R7 implicitly.
Missing or incomplete composition fails before repository acquisition.

This protected package ratifies governance only. Product implementation
**requires a separate Ready product issue** after this decision and the bounded
`FR-EXT-005` profile are independently reviewed and manually merged.

## Pinned wire contract

The only accepted binary wire contract is `scip-code/scip` v0.9.0:

| Item | Pinned value |
|---|---|
| Tag | `v0.9.0` |
| Commit | `e8ee0ae6038f8298e2195812eea9d7b1196748ae` |
| Schema path | `scip.proto` |
| Schema SHA-256 | `04cb20f2b8be73f6c0376b5b3e84c3ae20ebaff0ad3d23ba2d16f866b395ed7d` |
| Protocol enum | `UnspecifiedProtocolVersion = 0` |
| Metadata encoding | UTF-8 |
| Rust document position encoding | UTF-8 code-unit offset from line start |
| Document language | `rust` |
| Accepted producer family | `rust-analyzer-scip`, preserving exact name, release, commit, arguments digest, project-root digest, toolchain, and target triple |

The project-owned golden declares rust-analyzer release `2026-08-03` at commit
`b54a82b321c9617c5cf0b07ac0f12c08f7bc5902`. Validation proves declared
provenance and byte consistency only. It is not cryptographic attestation that
the declared executable or toolchain produced the artifact.

## Public versions

The selected path uses exactly:

| Contract | Version |
|---|---|
| Binding | `codenoesis.compiler-index-binding/v1` |
| Configuration | `codenoesis.configuration/v7` |
| Snapshot | `codenoesis.repository-snapshot/v10` (`RepositorySnapshotV10`) |
| Extraction chunk | `codenoesis.extraction-chunk/v7` (`ExtractionChunkV7`) |
| Extraction contract | `codenoesis.extraction/v7` |
| Knowledge graph | `codenoesis.knowledge-graph/v7` (`KnowledgeGraphV7`) |
| Rust ontology | `codenoesis.ontology/rust/v7` |
| Error | `codenoesis.error/v14` (`ErrorV14`) |
| Exact-ID query | `codenoesis.local-query-result/v5` (`LocalQueryResultV5`) |
| Pipeline | `codenoesis.pipeline/s4-r7-v1` |
| Compiler evidence locator | `codenoesis.compiler-index-evidence/v1` |
| Semantic hash | `codenoesis.semantic-hash-contract/v6` |

V10 extends the complete immutable V9 lineage. V9 remains byte-identical on
LocalQueryResultV4, V8 remains byte-identical on V3, V7 remains byte-identical
on V2, and V4 through V6 remain byte-identical on V1. Every invocation without
both R7 inputs remains byte-for-byte compatible through R6.

## Binding and trust boundary

`CompilerIndexBindingV1` is closed and binds all authority-relevant inputs:

1. repository identity, immutable commit OID, root tree OID, and R6 source
   manifest SHA-256;
2. safe artifact-relative path, byte length, SHA-256, SCIP tag, commit, schema
   digest, protocol, and canonical-encoding requirement;
3. producer family, name, version, commit, digest-only arguments and project
   root, Rust toolchain release and commit, and target triple;
4. each indexed document's canonical repository-relative path, committed blob
   OID, SHA-256, and byte length;
5. explicit indexed and omitted document sets, bounded omission reasons,
   coverage mode, generated exclusions, and known producer limitations.

The JSON binding and raw artifact are untrusted local inputs. Their path pair is
opened through the established safe-path and immutable-read boundary. No path
derived from SCIP `Metadata.project_root`, arguments, embedded text, diagnostic,
documentation, signature, external symbol, environment value, or repository
content grants filesystem authority. Mutable-input races, escaping symlinks,
absolute paths, traversal, changed bytes, and mismatched lengths fail before
decode or publication.

## Bounded wire validation

A framing preflight runs before generated protobuf decode or proportional
allocation. It enforces the raw-byte ceiling, legal wire types and field
numbers, metadata first and exactly once, recursion depth, repeated counts, and
length-delimited field bounds. The decoded representation then must:

- reject every unknown field recursively;
- reject duplicate singular values and non-minimal varints;
- reject malformed, truncated, over-recursive, or unsupported encodings;
- deterministically re-encode with the pinned library and require byte equality;
- require deprecated and typed ranges to be semantically equal when both occur;
- require UTF-8 metadata and Rust document position encodings;
- validate range order and exact UTF-8 byte boundaries;
- validate all document hashes against committed bytes already acquired by R6;
- reject invalid SCIP symbol grammar, role bits, duplicate document paths,
  unsupported protocol/language, and unresolved authoritative evidence.

Malformed, duplicate, unknown, non-canonical, or over-limit protobuf is never
normalized into acceptance.

## Ontology and identity

Ontology v7 adds `compiler.symbol` as a compiler assertion. It never rewrites
an existing syntax entity and has exactly one binding state:

- `in_repository_bound`: a definition or occurrence resolves to exact committed
  source bytes in the bound repository;
- `external_unbound`: a valid external symbol has no committed definition in
  the bound repository;
- `generated_unbound`: a producer-generated symbol lacks an exact committed
  definition and therefore cannot become a source declaration.

A non-local SCIP identity is parsed into scheme, package manager, package name,
package version, and normalized descriptors. It is independent of repository,
commit, artifact digest, document order, occurrence order, byte offset, and
scheduler order. A local symbol additionally includes repository identity and
canonical document path. NFC-normalized RFC 8785 array preimages use the
disjoint domain:

```text
codenoesis.entity-id/compiler-symbol/v1
```

The BLAKE3-256 result retains the public
`urn:codenoesis:entity:blake3:<digest>` shape. Duplicate definitions, invalid
grammar, and NFC collisions fail or remain the exact reviewed ambiguity gap.
Ordinal, source order, byte offset, scheduling, and retry cannot repair an
identity collision.

When one uniquely mapped committed occurrence has no selector-absent R6
`rust.symbol_reference`, R7 materializes that syntax-reference entity with the
unchanged R6 declaration preimage rather than inventing an R7 identity domain:

```json
[
  "codenoesis.entity-id/rust/v2",
  "<repository-identity>",
  "symbol_reference",
  "<crate-id>",
  "<canonical-module-path>",
  "<NFC-spelling>"
]
```

This entity is derived from the uniquely mapped committed occurrence and is
not claimed to have existed in the selector-absent R6 graph. The occurrence
path and range remain evidence locators and do not alter the immutable R6
identity recipe.

## Relationships and honest limitations

R7 adds only these compiler-backed relationship kinds:

- `RESOLVES_TO` links an existing R6 syntax reference, or the occurrence-derived
  R6-identity syntax reference defined above, to one uniquely bound committed
  declaration or compiler symbol;
- `REFERENCES` links one uniquely identified lexical owner to a resolved target
  and means symbol reference only;
- `IMPLEMENTS` requires an explicit SCIP `is_implementation` relationship and
  unique endpoints;
- `TYPE_DEFINITION` requires an explicit SCIP `is_type_definition` relationship
  and unique endpoints.

No `CALLS`, `EXECUTES`, or `SERVES` relationship is authorized. `STARTS`,
`REACHES`, `ACTIVATES`, inferred data flow, runtime behavior, generated source
authority, and macro expansion are also forbidden. A function-like SCIP
occurrence remains a reference. The mandatory
`compiler_index.call_semantics_unavailable` and
`compiler_index.generated_product_unbound` gaps make these limits explicit.

## Evidence, precedence, and conflicts

Every promoted assertion retains both:

- compiler evidence identified by raw artifact SHA-256 plus a canonical
  semantic locator under `codenoesis.evidence-id/compiler-index/v1`; and
- exact committed source evidence for each in-repository occurrence or endpoint.

Compiler evidence IDs use SHA-256 so the raw artifact binding is visible in the
identity. Existing source-span evidence remains byte-identical. A uniquely
validated compiler relation may outrank a syntax-only unresolved heuristic,
but it never deletes syntax evidence. Contradiction retains both evidence sets,
emits a typed diagnostic and coverage gap, and blocks promotion where an
endpoint is ambiguous. Missing definitions, omitted documents, unimported
prose, redacted roots, and redacted arguments remain explicit bounded gaps.

Raw absolute roots, command arguments, environment values, source snippets,
embedded `Document.text`, documentation, signatures, diagnostics, and external
symbol prose never enter public graph identities, docs, query results, errors,
logs, or semantic hashes. Only approved digests and bounded coverage statements
may be retained.

## Query and documentation behavior

A validated stored V10 head dispatches to LocalQueryResultV5 for the existing
exact-ID result kinds: entity, relationship, claim, evidence, diagnostic,
coverage gap, and document. Compiler symbols and SHA-256 compiler evidence use
those same kinds. The validated stored head selects the result version; no
query-version flag, fuzzy search, traversal, repair, or migration is added.

Evidence-backed documentation distinguishes committed syntax, compiler
assertion, unique binding, conflict, external or generated state, and coverage
gap. It never upgrades an index assertion to observed execution, availability,
reachability, serving, or runtime truth.

## Errors and limits

New selected-path failures are strict LF-terminated ErrorV14 on stderr with
empty stdout and no partial store or documentation mutation. Codes cover
invalid profile or composition, unsafe paths, invalid binding, unsupported
schema or producer, binding mismatch, malformed or non-canonical artifact,
identity conflict, ambiguous endpoint, contradictory relationship, limit
exceeded, unresolvable evidence, and internal contract failure.

The initial fixed maxima are:

| Limit | Maximum |
|---|---:|
| Raw `index.scip` bytes | 67,108,864 |
| Binding JSON bytes | 1,048,576 |
| Documents | 20,000 |
| Total occurrences | 1,000,000 |
| Occurrences per document | 100,000 |
| Symbol-information records | 250,000 |
| Relationships | 500,000 |
| One parsed symbol or display value | 16,384 UTF-8 bytes |
| One unpromoted text, diagnostic, signature, or argument value | 65,536 UTF-8 bytes |
| Tool arguments | 128 |
| One tool argument | 4,096 UTF-8 bytes |
| Protobuf/message recursion | 64 |
| Determinism permutations | 50 plus isolated replay |

Inherited path, source-file, output, wall-time, and 512 MiB peak-RSS ceilings
remain authoritative. Every maximum-plus-one fails before proportional
allocation or publication. Silent truncation is forbidden.

## Fixture and oracle

The Apache-2.0 project-owned `compiler-index-v1` fixture contains a two-crate
Rust workspace, a reviewed source representation, canonical `index.scip`,
strict binding, expected overlay, and manifest binding every input byte. Its
`build.rs` is a panic sentinel and is never run. The fixture covers cross-crate
definitions and references, aliases, local and global symbols, Unicode/NFC,
one explicit implementation relation, one type-definition relation, an
external symbol, a generated unbound symbol, one omitted source file, all
mandatory privacy and semantic gaps, and exact IDs and evidence locators.

The invalid matrix covers stale revision, artifact/source/toolchain mismatch,
malformed and non-canonical protobuf, unknown fields, duplicate metadata,
illegal roles and encodings, invalid ranges, ambiguous and conflicting
endpoints, normalization collision, missing evidence, traversal, symlink,
race, privacy canaries, every maximum-plus-one, and forbidden process/network
authority. Linux, macOS, Windows, 50 permutations, and isolated replay remain
required product evidence; this governance fixture itself executes no indexer.

Lekton and RustDesk remain replaceable, non-vendored future pilot repositories.
They are not executed, fetched, copied, or used as ontology truth by this
package. A pilot may import them only after a separately generated and reviewed
revision-bound artifact is supplied under an authorized product issue.

## Supply-chain decision

A future product issue may add exactly:

- `scip = "=0.9.0"`, Apache-2.0;
- `protobuf = "=3.7.2"`, MIT.

The governance package changes no manifest or lockfile. It authorizes no
`prost`, generator, `protoc`, build dependency, runtime downloader, alternative
parser, feature expansion, or unlisted package. The product issue must retain
the exact resolved lockfile, advisories, licenses, transitive graph, pinned
toolchain compile, and first-party/transitive `unsafe` review. The recorded
lexical `unsafe` counts are supply-chain observations, not semantic audits.

## Governance Red and traceability

The conformance guard was committed before Decision 0017 and every R7 schema,
fixture, golden, bundle, or dependency byte. On test-first head
`bb9acb9ae7f6dbed4da2294cc85b12a7fb07b5ef`, the command
`python3 -m unittest scripts.tests.test_s4_r7_compiler_index_contract` failed
only because this decision was absent, with exit `1`, empty stdout, and the
expected missing-artifact message. The retained 669-byte stderr log has
SHA-256 `6d8ef8c7850a87140028a25f49d920936338bc3191a5ed53d058c358434ef8cc`;
the retained test-first guard has SHA-256
`61490532b478fc391bb383287503ecb8cd5f384aa1710aba4ffd93c4703c4ff9`.
No production, dependency, R6 contract, fixture, golden, or unrelated protected
byte changed before Red.

## Consequences and rollback

R7 can add revision-bound compiler assertions without granting build authority
or weakening syntax evidence. The cost is a larger strict parser boundary,
explicit artifact lifecycle, and product work for bounded framing, canonical
protobuf validation, identity reconciliation, privacy, and supply-chain review.

The rollback boundary is the complete governance pull request. Reverting it
before product authorization restores the R6 baseline without migration,
runtime, dependency, release, destructive-data, or stored-format side effects.
The authoring agent does not approve or merge this decision.
