# Decision 0051: Kotlin/KMP static declarations

- Status: Branch candidate; effective after maintainer merge
- Date: 2026-09-07
- Requirement: FR-EXT-025; slice S8, first P1 adapter
- Base: `1eea130dfd3842bd1cdec5295fe43a0a0b41f788`
- Authorization: maintainer instruction to proceed with Kotlin/KMP after Rust
  validation PR #224 was merged

## Decision

Deliver the bounded [Kotlin/KMP v0.1 contract](../kotlin-kmp-v01.md) in one
vertical package: immutable local Git scan, syntax extraction, evidence-backed
ontology, existing local-store publication, query, documentation projection,
portable export and offline viewer. The dependency already pinned for S7,
`tree-sitter-kotlin-ng = 1.1.0`, is reused without changing its version or the
existing S7 client adapter.

The domain owns declaration kinds, capacities and conservative expect-candidate
selection. The Kotlin adapter owns parser traversal and path-convention module
and source-set discovery. The application invokes inward-owned acquisition and
extraction ports. Contracts serialize the domain result and validate graph
references, claim states, evidence locators and integrity. The CLI reuses the
confined scan worker, packed Git acquisition, immutable CAS/SQLite publication
and guarded marker-owned artifact output.

The new snapshot is `codenoesis.repository-snapshot/v19`. Ontology, graph,
portable, query, documentation, error and viewer contracts have separate Kotlin
v1 namespaces. Existing Rust snapshots and profiles keep their exact contracts.
Modern common storage rows are shared; no store migration or dependency change.
Entity identities include repository identity, commit and source occurrence;
this first profile does not offer cross-revision identity or incremental refresh.

Observed declarations use the common `deterministic_fact` claim state with exact source evidence.
`EXPECT_ACTUAL_CANDIDATE` relationships always have `Unknown` resolution and their claims use the common
`candidate` state. Overloaded or cross-module names cannot establish a candidate. This is
not compiler actualization, type checking or effective source-set visibility.
Each subject has one claim aggregating sorted evidence references, preserving
the existing immutable storage invariant.

Modules and source sets are explicitly observed by conventional committed paths
under directories with a build file; scripts are inventoried without evaluation.
Active Gradle projects, custom source roots and source-set dependency graphs are
not inferred. Nonconventional Kotlin files retain declarations with an unassigned
root gap. Java files remain explicit boundaries. Function bodies and local
declarations are outside this declaration profile.

## Validation and limitations

The hand-authored v1 oracle checks all 11 fixture declarations and two candidate
relationships, including overload and cross-module decoys. Source parser tests
check nested members, constructor properties, Unicode byte spans, annotations,
malformed input and the pinned parser's known single-line type-member gap.
Some valid Kotlin fails that parser; the complete file becomes a syntax gap and
contributes no partial declarations. This behavior is visible in measurements.

Two public Apache-2.0 samples are pinned in
`tests/specifications/s8/kotlin-kmp-public-v1.json`; the runner retains each of
three attempts, including errors, exact binary digest, counts and timings.
Public corpus determinism and counts are not semantic accuracy. Fixture
precision/recall is bounded to its source oracle. No general Kotlin support,
Gradle execution, Java adapter, GA, support or release claim is made.

Portable hashes establish integrity, not authorship. The viewer uses only a
CLI-validated embedded payload, escapes HTML delimiters and renders labels as
text. Static script/style hashes enforce CSP; no browser launch, server, model,
network, compiler, target wrapper or target child process is introduced.
