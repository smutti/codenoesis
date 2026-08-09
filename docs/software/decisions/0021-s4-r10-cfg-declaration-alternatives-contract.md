# Decision 0021: S4 R10 cfg declaration alternatives contract

| Field | Value |
|---|---|
| Status | Proposed; effective only after protected manual merge |
| Issue | [#152](https://github.com/smutti/codenoesis/issues/152) |
| Exact base | `dae3860ded12202bdf6bbb1134a08f28098f3c9a` |
| Requirements | Proposed `FR-EXT-013`, Proposed `FR-EXP-003`, and bounded amendments to `FR-QRY-001`, `FR-CLI-001`, `FR-DOC-001/002/003`, `NFR-DET-001`, and `NFR-TST-001/002` |
| Slice | `S4` |
| Risk | High — public ontology, schema, identity, evidence, query, documentation, export, and explorer behavior |
| Owner/approver | `@smutti` |
| Human authorization | Complete high-risk issue #152 package on the exact base above |
| Dependencies | Existing workspace only; no new dependency |
| Correction budget | Five rounds |
| Rollback | Revert the complete package; no migration, repair, rewrite, or release |

## Context

R5 preserves direct `cfg` uncertainty and one stable logical identity for
homogeneous repeated declarations. It must continue to do so byte-for-byte.
For heterogeneous direct-`cfg` methods, selecting one declaration shape would
invent an active configuration while rejecting the entire source loses useful
committed syntax. RustDesk exposes this boundary at
`try_start_clipboard`, but remains replaceable diagnostic evidence rather than
an ontology golden.

R10 therefore adds one explicit source-only profile. It represents every
accepted occurrence as an evidence-backed declaration alternative while
retaining the R5 logical method identity. It never interprets a `cfg`
predicate, proves exhaustiveness or mutual exclusion, or chooses an active
world.

## Selection and closed composition

The exact selector is:

```text
--profile standard-local-s4
--workspace-profile cargo-root-package-v1
--manifest-profile cargo-manifest-facts-v1
--rust-semantic-profile rust-cfg-declaration-alternatives-v1
```

`--repository-boundary-profile local-gitlinks-v1` is the only optional source
composition. R6, R7, K1, and `local-snapshot-64m-v1` do not compose. Invalid,
unknown, duplicate, incomplete, or forbidden compositions fail before
acquisition with ErrorV17, empty stdout, and no store mutation.

## Contract lineage

| Contract | Version |
|---|---|
| Configuration | `codenoesis.configuration/v9` |
| Repository snapshot | `codenoesis.repository-snapshot/v12` |
| Extraction contract/chunk | `codenoesis.extraction/v9` / `codenoesis.extraction-chunk/v9` |
| Knowledge graph | `codenoesis.knowledge-graph/v9` |
| Rust ontology | `codenoesis.ontology/rust/v9` |
| Semantic hash | `codenoesis.semantic-hash-contract/v8` |
| Error | `codenoesis.error/v17` |
| Query | `codenoesis.local-query-result/v7` |
| Portable graph | `codenoesis.portable-graph/v3` |
| Local explorer | `codenoesis.local-explorer/v3` |
| Pipeline | `codenoesis.pipeline/s4-r10-v1` |
| Extractor/index | `codenoesis.rust-cfg-alternatives/s4-r10-v1` / `codenoesis.rust-cfg-alternative-index/v1` |

R10 derives semantically from R5, not R6, R7, or K1. DocumentationManifestV1,
the local-store protocol, artifact roles, publication semantics, and immutable
K1 viewer bytes are reused. V12 introduces only its new snapshot, graph, and
extraction hash domains. All historical domains and bytes remain immutable.

## Logical method representation

The logical method retains its existing R5 ID and the exact preimage:

```text
codenoesis.entity-id/rust-member/v1
repository_identity
crate_id
owner_id
rust.method
NFC method name
trait_context_id or empty
```

When two or more accepted alternatives exist, the logical V9 entity retains
`id`, `kind`, `crate_id`, `module_path`, `name`, `visibility`, and `owner_id`.
Its properties are exactly the shared `implementation_context`, shared
`trait_context_id`, `declaration_state = alternatives`, and the sorted unique
`declaration_alternative_ids`. It carries no occurrence-specific
`receiver_present`, `declared_signature`, `compilation_presence`, or
`attributes`. A non-alternative method retains its R5 shape inside V9.

The existing logical entity claim and existing `DEFINES` relationship claim
aggregate every sorted unique declaration evidence ID. Their subject IDs do
not change.

## Declaration alternative representation

Each accepted occurrence becomes one `rust.declaration_alternative` entity
with `id`, `kind`, `crate_id`, `module_path`, `name`, `subject_id`, and
`source_file_id`. Its exact properties are:

```json
{
  "declaration_kind": "rust.method",
  "implementation_context": "inherent_implementation",
  "trait_context_id": null,
  "visibility": "public",
  "receiver_present": true,
  "declared_signature": "pub fn try_start_clipboard(&self, context: Option<Context>)",
  "compilation_presence": "conditional_unknown",
  "declaration_evidence_id": "urn:codenoesis:evidence:blake3:...",
  "attributes": []
}
```

Attributes retain the R5 `kind`, `token_text`, and `evidence_id` shape. Source
path, blob, and byte span resolve only through evidence. Each alternative has
one `deterministic_fact` entity claim backed by its declaration evidence.

One `HAS_DECLARATION_ALTERNATIVE` relationship connects the logical method to
each alternative. Its `deterministic_fact` relationship claim uses that
alternative's declaration evidence.

## Identities and ordering

Alternative entity IDs use BLAKE3-256 with the normal entity prefix over the
canonical array:

```text
[
  "codenoesis.entity-id/rust-declaration-alternative/v1",
  repository_identity,
  logical_method_id,
  declaration_evidence_id
]
```

Relationship IDs use BLAKE3-256 with the normal relationship prefix over:

```text
[
  "codenoesis.relationship-id/rust-declaration-alternative/v1",
  "HAS_DECLARATION_ALTERNATIVE",
  logical_method_id,
  alternative_entity_id
]
```

Source order and ordinal never enter identity; the exact committed evidence
locator does. Moving an occurrence may change evidence and alternative IDs but
not the logical method ID. Every family and ID array sorts by raw UTF-8 ID
bytes. Duplicate IDs fail rather than being repaired or silently deduplicated.

## Acceptance rule

R10 accepts repeated R5 `function_item` or `function_signature_item` methods
only when every occurrence:

1. has a direct `#[cfg(...)]` and `conditional_unknown`; `cfg_attr` alone is
   insufficient;
2. has equal logical ID, repository, crate, module, NFC name, owner,
   visibility, implementation context, and trait context;
3. resolves to the same source file and blob;
4. has distinct, in-bounds, pairwise non-overlapping declaration evidence;
5. resolves all declaration and attribute evidence to the same
   repository/commit/source closure; and
6. satisfies every inherited and R10 limit.

Both homogeneous and heterogeneous repeated direct-`cfg` methods become
alternatives in R10. The profile is methods-only. It does not broaden fields,
variants, constants, statics, associated types, owners, free functions,
macros, generated declarations, cross-file coalescing, R6/R7/K1 facts, or
compiler/runtime semantics.

## Epistemic and failure boundaries

`cfg` attributes retain exact source evidence and unresolved diagnostics. R10
does not parse or evaluate their predicates, select a target, claim mutual
exclusion or exhaustiveness, execute Cargo, `rustc`, Git, build scripts,
macros, target code, a browser, a model, or network I/O, or infer types,
values, bodies, calls, framework, compiler, or runtime meaning.

ErrorV17 distinguishes invalid profile/composition, inherited source failure,
logical identity mismatch, duplicate alternative, overlapping evidence,
cross-file/blob declarations, limit excess, invalid V12/query/portable data,
unsafe output, explorer integrity, and internal failure. Every failure is
atomic and publishes no partial head.

## Query, documentation, export, and explorer

LocalQueryResultV7 returns any requested R10 family and directly linked R10
entities, relationships, claims, and evidence. DocumentationManifestV1 adds
evidence-backed declaration-alternative sections only for GraphV9. Historical
documentation bytes remain unchanged.

PortableGraphV3 is a lossless canonical projection of one validated V12 head
and its deterministic documents. Reimport rejects unknown/private fields,
non-canonical order, duplicates, dangling references, invalid evidence,
family-hash mismatch, and limits; it never repairs, infers, truncates, or
deduplicates. LocalExplorerV3 reuses the immutable K1 viewer bytes and remains
read-only and offline, with no storage, telemetry, network, dynamic code,
browser-launch, or graph mutation authority.

## Limits

R10 adds exactly:

- 32 alternatives per logical method;
- 4,096 alternatives per source;
- 200,000 alternatives per snapshot;
- 50 insertion/permutation replays; and
- 10 parallel schedules.

It inherits 128 attributes per declaration, 16,384 attribute-token bytes,
4,096 signature bytes, 33,554,432 V12 stdout bytes including LF, 4,194,304
query bytes, 2,001 documents, 1,048,576 bytes per document, 33,554,432 total
document bytes, 200,000 statements, 268,435,456 PortableGraphV3 bytes, nesting
64, text result 100, traversal depth 1/2, and neighborhood 256 subjects/512
relationships. Every count and byte limit has exact maximum and maximum-plus-one
coverage. No limit permits truncation, sampling, retry, or silent repair.

## Fixture and evidence

The project-owned fixture is
`tests/fixtures/s4/rust-cfg-declaration-alternatives-v1/` with repository
identity `urn:codenoesis:fixture:s4-rust-cfg-declaration-alternatives-v1`, tree
`6aa31d889f4c87b2b7dfbff3fef3b32ee7fa0363`, and commit
`d5a44bb5bb12ddb6f71ea4dd0c88944dc41eefec`. Its manifest freezes all bytes,
blobs, spans, commit metadata, reviewed identities, two heterogeneous method
signatures, decoys, and a build sentinel.

The checkpoint test command is:

```text
cargo test -p noesis --test e2e_fr_ext_013_cfg_declaration_alternatives e2e_fr_ext_013_cfg_method_alternatives_publish_declarations -- --exact
```

On the exact base it must be Red only because the selector is absent: exit 2,
empty stdout, absent store, ErrorV12
`input.invalid_rust_semantic_profile`, 233 stderr bytes, and SHA-256
`dda30410fb0e9ea21d098ac38074c69d6316d777ee16340175ad7db00aa26be1`.
The same fixture under legacy R5 remains the frozen typed identity conflict:
exit 11, empty stdout, absent store, 366 stderr bytes, and SHA-256
`b5c9f9edd1c4d38220f5f20992a3cf2bc4e693ea6bcacab1b44d3b2dcbe62663`.

Green requires the exact V9/V12 fixture, both homogeneous and heterogeneous
R10 alternatives, scan → docs → exact-ID query → export → strict reimport →
explore, invalid and maximum-plus-one cases, 50 permutations, ten schedules,
legacy byte regressions, and two deterministic RustDesk runs pinned to commit
`d412d198720aa56f6cfed2dfad262e8fb1322fb7` and tree
`df8d4c292c9d256a445480eb878e507df3de1dc4`. RustDesk may publish V12 or stop
at the same next typed boundary; it is diagnostic and cannot weaken the
project-owned oracle.

## Compatibility and consequences

R5 ConfigurationV5, RepositorySnapshotV8, ExtractionChunkV5, GraphV5,
ontology v5, ErrorV12, QueryV3, docs, identities, hash domains, selectors,
fixtures, and goldens remain byte-identical. R6/V9, R7/V10, R8 V1 export and
explorer, K1/V11, QueryV1-V6, PortableGraphV2, LocalExplorerV2, R9 capacity,
and every R0-K1 golden/viewer byte remain unchanged.

This representation gives downstream tools and LLM-facing projections honest
access to all committed declaration shapes without claiming which one exists
in a build. It does not solve `cfg` evaluation, generated code, compiler
semantics, cross-file declarations, framework meaning, or runtime behavior.
Reverting the package removes only R10; no historical artifact requires
migration. The authoring agent cannot approve or merge this high-risk change.
