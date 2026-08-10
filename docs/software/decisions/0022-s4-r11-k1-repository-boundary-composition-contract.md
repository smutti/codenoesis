# Decision 0022: R11 K1 repository-boundary composition contract

- Status: Proposed branch-scoped candidate
- Date: 2026-08-09
- Issue: [#155](https://github.com/smutti/codenoesis/issues/155)
- Exact base: `4fd0abb6d663d90ca35af4dee5eaf932f8f9ed94`
- Slice: `S4`
- Risk: high
- Owner and approver: `@smutti`

## Context

The implemented K1 profile produces evidence-backed Rust callable, value, and
body-syntax facts over the R6 source lineage. The implemented R2 profile
represents committed mode `160000` entries and bounded `.gitmodules` metadata
without fetching or traversing nested repositories. On the authorized base,
the CLI rejects selecting both profiles before acquisition, so a repository
such as RustDesk cannot retain its gitlink boundary while requesting K1.

Changing ConfigurationV8 or RepositorySnapshotV11 would alter historical K1
bytes and violate compatibility. Composing K1 with R10 or R7 in the same
package would change the source or compiler lineage and exceed issue #155.

## Decision

Introduce one additive R11 family selected only by the complete existing K1
source selectors plus `--repository-boundary-profile local-gitlinks-v1`.
The optional existing boundary manifest may bind depth-one nested revisions.
The optional existing 64 MiB output-capacity selector remains non-semantic.

The contracts are fixed as follows:

| Contract | Value |
|---|---|
| configuration | `codenoesis.configuration/v10` |
| repository snapshot | `codenoesis.repository-snapshot/v13` |
| extraction/chunk | `codenoesis.extraction/v10` / `codenoesis.extraction-chunk/v10` |
| graph/ontology | `codenoesis.knowledge-graph/v10` / `codenoesis.ontology/rust/v10` |
| semantic hash | `codenoesis.semantic-hash-contract/v9` |
| error/query | `codenoesis.error/v18` / `codenoesis.local-query-result/v8` |
| portable/explorer | `codenoesis.portable-graph/v4` / `codenoesis.local-explorer/v4` |
| pipeline | `codenoesis.pipeline/s4-r11-v1` |
| composition | `codenoesis.rust-callable-boundary-composition/s4-r11-v1` |

The K1 callable extractor/index and R2 boundary extractor/report retain their
existing versions. R11 adds a composition version; it does not claim a new
parser or boundary identity algorithm.

## Acquisition and extraction order

1. Validate selectors before acquisition.
2. Acquire the root revision with the R2 boundary-aware adapter.
3. Parse bounded committed `.gitmodules` metadata and build the exact R2
   report. Validate any explicit nested binding against the gitlink OID.
4. Convert only report path/boundary-ID pairs into external workspace
   boundaries.
5. Run the K1 callable extractor over the boundary-aware R6 root lineage.
6. Validate and publish one V13 snapshot atomically.

The product never reads nested source. An absent and a present-but-unsupplied
nested worktree have identical authority and canonical semantic output. An
explicit binding records only validated nested repository identity, commit,
tree, and the `boundary.nested_repository_not_analyzed` gap.

## Canonical representation

V13 contains the complete R2 `codenoesis.repository-boundaries/v1` report at
`semantic.repository_boundaries`, the boundary-aware R6 root workspace, and
the complete K1 callable projection. The graph does not turn a boundary into
a Rust code entity and creates no call, ownership, type, or dependency edge
across it.

For the same repository identity and source paths, existing K1 logical entity
and relationship preimages remain unchanged. Source evidence retains its
approved revision/blob/span rules. R2 boundary, declaration, evidence, and gap
IDs retain their SHA-256 preimages. New configuration, chunk, graph, and
snapshot hashes use only the V10/V13 domains fixed by the semantic-hash
contract.

All identity-bearing families are ordered by raw UTF-8 identity bytes.
Duplicate identities or dangling references fail rather than being repaired.

## Read projections

`LocalQueryResultV8` retrieves normal K1 subjects and the four boundary subject
kinds: boundary, declaration, evidence, and coverage gap. It includes only
directly linked boundary records and root workspace members.

Generated documentation adds deterministic boundary summary and detail
statements grounded in the report's evidence. It states `declared_unbound` or
`explicitly_bound` and the corresponding gap; it never describes nested code.

`PortableGraphV4` carries the validated V13 boundary report, graph families,
and generated documents losslessly. Reimport validates schema, order,
identities, references, evidence, hashes, privacy, and limits. It excludes
source contents, snippets, absolute roots, manifest filesystem roots, raw URL
text, credentials, environment, and telemetry. `LocalExplorerV4` reuses the
immutable K1 viewer bytes under a new manifest and security profile.

## Errors and atomicity

`CodeNoesisErrorV18` composes existing K1 and R2 failures. Invalid K1+R7,
K1+R10, missing/invalid boundary pairing, or invalid capacity pairing fails
before acquisition. Boundary metadata, binding, race, and limit failures keep
their established codes. K1 source failures keep their established semantic
codes. Any failure has no partial stdout, snapshot, document, portable,
explorer, or visible-head publication.

## Limits and security

R11 inherits every R2 and K1 limit without increasing semantic authority.
The standard V13 stdout maximum is `33,554,432` bytes including LF; the
explicit capacity profile permits `67,108,864` bytes including LF. Portable
V4 remains bounded at `268,435,456` bytes. Every applicable maximum and
maximum-plus-one is tested. Fifty permutations and ten schedules must produce
identical canonical semantics.

The monitored product launches no child process, opens no network channel,
invokes no Git/Cargo/rustc/build script/macro/target, resolves no URL, opens no
browser, and contacts no model provider. Existing confined filesystem and
marker-owned atomic output rules apply.

## Compatibility

K1 without a boundary selector remains exact V11. Existing R2 through R10
families, errors, hashes, fixtures, goldens, accepted commands, portable
projections, explorer manifests, and viewer bytes remain immutable. V13 is
never inferred. K1+R10 and K1+R7 require later decisions.

## Verification

The project-owned fixture reuses exact K1 source bytes and overlays one
reviewed `.gitmodules` blob and one empty-tree gitlink. Its unbound and bound
reports have exact reviewed identities and no external source. The public E2E
journey is scan, docs, callable and boundary queries, export, strict reimport,
and explore.

The first branch commit contains governance, schemas, fixture, oracle, and the
executable acceptance test without production edits. On the exact base the
test must fail with ErrorV16
`input.unsupported_rust_callable_composition`, exit `11`, empty stdout,
absent store, and stderr SHA-256
`2573e0f364350b300218c6d1940e6eb33f4f0bc70b7ba92dd9b2821f5bf97013`.
Only retained expected Red permits production implementation.

Pinned RustDesk is diagnostic only. R11 must advance it to the existing typed
R6 `rust.method` identity conflict for `try_start_clipboard`; full K1
completion requires the separately governed K1+R10 composition.

## Consequences

CodeNoesis can produce a complete K1 ontology for root-owned source while
honestly retaining external Git boundaries. The additive family costs another
set of dispatch and validation paths but preserves historical bytes and keeps
nested source authority explicit.
