# CodeNoesis Delivery Roadmap

> Status: **Proposed planning companion — not implementation authority**.
> Last updated: **2026-07-28**.

This roadmap sequences product and validation work without changing the
normative meaning or approval status of the
[Software Requirements Specification](software-requirements-specification.md)
or an accepted architecture decision. The SRS and decisions remain
authoritative. Every item below requires its own Ready issue, stable
requirement IDs, one approved delivery slice, acceptance oracle, expected Red
failure, risk classification, allowed paths, evidence, and human approvals
before production implementation starts.

Roadmap identifiers such as `R1` and `P1` are planning identifiers, not SRS
slice or requirement IDs. They must not be used to bypass the approved
`S0`–`S14` delivery and governance process.

## Current product baseline

The repository contains the local `S0`–`S4` implementation journey:

```text
immutable local Git revision
  -> bounded inventory
  -> Rust ontology
  -> atomic local snapshot
  -> evidence-backed Markdown and exact-ID query
```

The current compatibility profile deliberately remains narrow:

- acquisition accepts verified loose SHA-1 Git objects and rejects packed
  object databases;
- the S4 Cargo profile accepts a literal virtual workspace whose root manifest
  contains only `[workspace]`;
- Rust ontology v2 already covers crates, source files, modules, structs,
  enums, traits, type aliases, free functions, methods, imports, and named
  trait implementations;
- Cargo feature worlds, macro expansion, compiler-grade resolution, framework
  semantics, general graph traversal, and an interactive viewer remain
  unsupported or explicit coverage gaps.

## Real-world Rust compatibility target

This lane expands CodeNoesis from the deliberately narrow S4 fixture profile
to reusable classes of real-world Rust repositories:

- ordinary full clones backed by packed Git object databases;
- repositories with explicit Git submodule/gitlink boundaries that must remain
  safe when nested repositories are absent or supplied separately;
- virtual and non-virtual Cargo workspaces, including root packages, the `"."`
  member, exclusions, multiple targets, features, target-specific
  dependencies, patches, and build metadata;
- large modular codebases that use attributes, macros, conditional
  compilation, framework conventions, and generated-code boundaries.

### Initial public corpus candidates

| Repository | Pinned revision | Observed license | Independent structural role |
|---|---|---|---|
| [Lekton](https://github.com/dghilardi/lekton) | [`7a4d1a4a30468f4c18ce158a9b825680b00f4820`](https://github.com/dghilardi/lekton/commit/7a4d1a4a30468f4c18ce158a9b825680b00f4820) | `AGPL-3.0-or-later` | Root package explicitly listed as `"."`, a separate CLI member, library and multiple binaries, features, build dependencies, and a build script in a modular web/server application. |
| [RustDesk](https://github.com/rustdesk/rustdesk) | [`d412d198720aa56f6cfed2dfad262e8fb1322fb7`](https://github.com/rustdesk/rustdesk/commit/d412d198720aa56f6cfed2dfad262e8fb1322fb7) | `AGPL-3.0` | Implicit root package plus workspace members and exclusions, library and multiple binaries, a build script, target-specific and Git/path dependencies, patches, one gitlink member, and substantial Rust/Dart/native/mobile source. |

Both repositories are validation cases, not sources of product semantics. No
ontology kind, extractor rule, error, or interface may contain
repository-specific meaning. A corpus entry may be replaced by another
repository that exercises the same approved capability contract.

External corpus source MUST NOT be copied into this repository merely for
convenience. Each corpus descriptor must pin repository URL, commit, observed
license, provenance, relevant structural features, and expected outcomes.
Small project-owned synthetic fixtures reproduce each generic repository shape
for reviewed Red/Green acceptance tests. Complete external repositories are
used only by separately reproducible pilot runs.

## Real-world Rust compatibility lane

| Order | Planning item | Outcome | Governance dependency | Candidate acceptance gate |
|---|---|---|---|---|
| `R0` | Reproducible public corpus baseline | Record pinned public revisions, licenses, repository statistics, structural capabilities, current CodeNoesis failure sequences, and minimal synthetic shape fixtures. | Corpus, fixture, oracle, and licensing review. | Machine-readable descriptors reproduce each generic failure and capability without vendoring an external repository; corpus entries remain replaceable. |
| `R1` | Read-only packed Git acquisition | Read normal packed SHA-1 object databases directly and safely without invoking Git. | Resolve the packed-object subset of `OD-GIT-001`; approve acquisition errors, bounds, corruption behavior, and security oracle. | Packed and equivalent loose fixtures produce byte-identical semantic input; malformed indexes, packs, deltas, traversal attempts, alternates, and limit-plus-one cases fail with typed errors; the scan launches no process and opens no network channel. |
| `R2` | Safe gitlink and submodule boundary | Represent committed gitlinks and declared submodule metadata as external repository boundaries without fetching or traversing them implicitly. An explicitly supplied nested repository remains a separately acquired, revision-bound project. | Resolve the gitlink/submodule subset of `OD-GIT-001` and its later federation relationship; approve missing, malformed, mismatched, recursive, and limit behavior. | Root analysis remains deterministic with an absent submodule; a supplied nested repository must match the committed gitlink SHA; `.gitmodules` never grants network or filesystem authority; malformed or escaping declarations fail with typed evidence. |
| `R3` | Real Cargo root-package workspace | Accept virtual and non-virtual root manifests, including implicit root members, an explicit `"."` member, literal members/exclusions, conventional and explicit library/binary targets, and multiple member manifests. A gitlink member remains an external workspace boundary rather than an implicitly traversed crate. | Versioned extraction/profile decision under `FR-EXT-*` and the unresolved post-S4 ontology boundary. | Project-owned fixtures cover virtual roots, implicit and explicit root packages, exclusions, and external gitlink members while reaching the existing S4 graph/docs/query journey deterministically; Cargo, `rustc`, build scripts, proc macros, dependencies, and target code remain unexecuted. |
| `R4` | Manifest facts and feature coverage | Represent package metadata, target declarations, registry/path/Git dependencies, target-specific dependency tables, feature declarations, optional dependencies, `required-features`, patch declarations, and build-script presence without claiming an active Cargo resolution. | Approve entity/property identities, claim states, compatibility, limits, and ontology version. | Every supported manifest fact resolves to bytes; ignored or unsupported fields are explicit diagnostics or coverage gaps; no dependency is fetched, no patch is applied, and no feature or target world is guessed. |
| `R5` | Rust semantic depth at real-world scale | Exercise existing enum, trait, named implementation, and method extraction on independent repositories; add only approved missing concepts such as fields, variants, constants/statics, attributes, components, services, configuration, and endpoint declarations. | New versioned Rust ontology decision resolving the relevant part of `OD-ONT-001`; high-risk golden review. | Reviewed generic fixtures and sampled facts from multiple corpus entries cover every new entity/relation, malformed syntax, stable IDs, graph invariants, evidence resolution, and deterministic replay. |
| `R6` | Honest framework and macro handling | Extract deterministic syntax-level declarations through framework-neutral capability contracts where source form is sufficient, while preserving unresolved `cfg`, attribute-macro, declarative-macro, and proc-macro meaning as coverage gaps. | Framework capability contract and explicit distinction between declaration, candidate, and resolved runtime behavior. | Fixtures from at least two framework styles find reviewed declarations, reject designed decoys, never infer expanded code, and never label a syntactic declaration as observed runtime behavior. |
| `R7` | Optional compiler-grade enrichment | Import version-bound SCIP or rust-analyzer evidence for cross-crate names, types, traits, calls, and macro products when deterministic syntax is insufficient. | Compiler-index schema and trust decision; generation belongs behind an explicit sandboxed profile compatible with `S9`. | Imported indexes are source/revision/toolchain bound and validated before facts are promoted; stale, malformed, mismatched, or incomplete indexes fail or remain explicit gaps. Standard local scans still execute nothing. |
| `R8` | Portable graph export and local explorer | Export a versioned, evidence-preserving projection and open a read-only local graph explorer for entities, relations, claims, gaps, and source evidence. | Public export compatibility decision; viewer security and size limits. The canonical snapshot remains authoritative. | Reimport validates identity and evidence without loss; filters and bounded traversal cannot mutate the snapshot; unsupported projections remain non-canonical. This is not a full product authoring GUI. |
| `R9` | Multi-repository pilot and publication evidence | Run scan, docs, query, export, and explorer against structurally independent pinned public repositories and publish a reproducible evaluation package. | Product evidence remains under `docs/software/`; conference hypotheses and analysis remain under `docs/research/`. | Repeated runs are deterministic; per-repository and aggregate graph/coverage counts, unresolved constructs, timings, resource usage, environment, tool versions, failure cases, and known limitations are retained in machine-readable form. |

### Earliest useful real-world checkpoint

`R1`–`R3` are the minimum path to analyzing ordinary packed clones with safe
gitlink boundaries and common virtual or non-virtual Cargo workspace layouts.
At that checkpoint CodeNoesis should create and query a partial but honest
ontology and generate evidence-backed documentation. `R4`–`R8` increase
semantic coverage and make the graph easier to inspect; they are not permitted
to hide unsupported meaning.

The existing `S5` slice is incremental refresh, so none of `R1`–`R8` may be
silently folded into `S5`. Governance must either amend the delivery plan with
bounded post-S4 compatibility slices or assign each behavior to an existing
future slice without changing that slice's approved meaning.

### Real-world compatibility completion definition

The compatibility profile is considered supported only when all applicable
gates are approved and at least two structurally independent pinned public
repositories demonstrate:

- acquisition from a normal full clone without repacking or manually
  materializing loose objects;
- safe representation of gitlinks as external boundaries, with no implicit
  submodule fetch and exact revision matching when a nested repository is
  explicitly supplied;
- recognition of virtual and non-virtual workspaces, root/member packages,
  library and binary targets, source modules, and declared feature/build
  metadata across the advertised profile;
- zero execution of Cargo, `rustc`, Git, build scripts, proc macros, project
  binaries, or downloaded dependencies during the standard local scan;
- a schema-valid graph in which every fact resolves to committed source
  evidence and every unsupported construct remains a diagnostic or coverage
  gap;
- deterministic snapshot, generated-document, query, and export behavior for
  the same revision and configuration;
- successful bounded exploration of architecture-relevant entities and
  relations;
- a reproducible per-repository and aggregate evidence pack suitable for
  independent engineering review and conference evaluation.

Each target's own build, CI, quality, or dependency-audit state is contextual
pilot metadata, not proof that an ontology fact is true and not a prerequisite
for a static CodeNoesis scan. CodeNoesis must report what it can establish from
approved inputs without executing or repairing a target repository.

## Polyglot adapter lane

The normative SRS currently assigns Java, JavaScript/TypeScript, and C/C++ to
`S8`. The following requested adapters are proposed additions to the roadmap.
They do not become S8 requirements until the SRS and versioned cross-language
ontology explicitly approve them.

| Proposed order | Adapter | Initial semantic capability |
|---|---|---|
| `P1` | Kotlin and Kotlin Multiplatform | Gradle modules, source sets, packages, classes, interfaces, objects, functions, properties, imports, `expect`/`actual`, and Java interoperability boundaries. |
| `P2` | Python | Packages, modules, classes, functions, methods, decorators, imports, constants, and explicit dynamic-resolution gaps. |
| `P3` | Go | Modules, packages, files, structs, interfaces, functions, methods, imports, embedding declarations, and generated-code boundaries. |
| `P4` | Swift | Swift packages, modules, protocols, types, extensions, functions, properties, actors, imports, and Objective-C interoperability boundaries. |
| `P5` | C# | Solutions/projects, namespaces, classes, records, structs, interfaces, methods, properties, attributes, references, and generated-code boundaries. |

Every language is delivered one adapter at a time through the same capability
contract:

- a project-owned reviewed fixture and hand-authored graph oracle;
- versioned entity, relationship, identity, evidence, and coverage semantics;
- malformed, Unicode, limit, determinism, and differential tests;
- explicit build-system and generated-code behavior;
- no compiler, package-manager, plugin, or target execution in the standard
  local profile;
- independent approval before the adapter is advertised as supported.

Kotlin/KMP is first in this proposed extension because it exercises JVM,
mobile, and shared multiplatform source-set boundaries in one adapter family.
The final order remains a governance decision and may change without altering
already-approved adapter contracts.

## Sequencing and evidence

The recommended execution order is:

1. approve and retain `R0` baseline evidence;
2. specify, Red-test, implement, and independently review `R1`;
3. specify, Red-test, implement, and independently review `R2`;
4. specify, Red-test, implement, and independently review `R3`;
5. run the first partial S4 journey on one replaceable corpus entry;
6. deliver `R4`–`R6` as separate manifest/ontology/framework objectives;
7. add `R7` only after the sandbox and index trust boundary is approved;
8. deliver `R8`, then execute `R9` on at least two independent repositories;
9. continue the polyglot lane one approved adapter at a time.

Each implementation pull request keeps one behavioral objective and includes
the required issue, requirement status, slice, risk, paths, base/head SHAs,
expected Red, Green/regression commands, deterministic environment, fixture,
oracle, traceability, limitations, and human approvals. No pilot or conference
claim upgrades a product requirement or marks a slice Verified by itself.
