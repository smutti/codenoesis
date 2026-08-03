# Decision 0012: S4 Cargo manifest facts contract

| Field | Value |
|---|---|
| Status | Proposed; becomes Accepted only when `@smutti` manually merges the exact protected head of PR [#PULL_REQUEST](https://github.com/smutti/codenoesis/pull/PULL_REQUEST) |
| Date | 2026-08-03 |
| Owners | Andrea Moretti (`@smutti` governance persona), accountable maintainer `@smutti` |
| Scope | `S4 — Evidence-backed workspace docs compatibility extension` only; roadmap `R4` |
| Requirement | `FR-EXT-009` |
| Governance issue | [#100](https://github.com/smutti/codenoesis/issues/100) |
| Authorization | [Maintainer comment](https://github.com/smutti/codenoesis/issues/100#issuecomment-5163551187) |
| Required base | `ea51a8151749fc65e75dd7a10e550adc0b67d422` |

## Context

Protected PR #99 implements R3. The explicit
`cargo-root-package-v1` workspace profile can now extract bounded standalone,
virtual, and non-virtual Cargo roots while preserving every broader Cargo
family as typed deferred coverage. That allows useful source ontology for
Lekton and RustDesk, but it cannot yet answer evidence-backed questions such
as:

- which dependency, feature, target, patch, or build declaration exists;
- whether a dependency was declared as registry, path, Git, or workspace
  inherited;
- which literal target predicate or `required-features` list was written;
- whether a package metadata value was direct or inherited;
- where the exact declaration occurs in committed bytes.

Reading a declaration is not equivalent to resolving Cargo. A dependency table
does not prove an active dependency edge, a feature member does not prove an
active feature, a target table does not prove the selected build world, a patch
does not prove application, and a build declaration does not prove execution.
Creating those stronger facts from syntax would violate the evidence model and
`INV-MDL-001`.

Cargo manifests may also contain credential-bearing locators, escaping paths,
hostile strings, unsupported tables, or sentinels intended to execute during a
build. R4 must remain deterministic with process, network, resolver, model, and
target authority disabled.

## Decision

Add one explicit manifest compatibility selector:

```text
--manifest-profile cargo-manifest-facts-v1
```

It is valid only for `scan --profile standard-local-s4` together with
`--workspace-profile cargo-root-package-v1`. Repository contents, manifest
tables, corpus identities, paths, and prior selectors never select it
implicitly. It may compose independently with the implemented R1
`local-git-sha1-packed-v1` acquisition selector and R2 `local-gitlinks-v1`
boundary selector.

The selected path emits a strict additive V7 lineage. Every invocation without
the R4 selector remains byte-for-byte compatible, including R3 V6 success and
failure bytes. R4 governance adds no production behavior; implementation
requires a separate Ready issue after protected merge.

## Public versions

The selected path uses exactly:

| Contract | Version |
|---|---|
| Snapshot | `codenoesis.repository-snapshot/v7` |
| Configuration | `codenoesis.configuration/v4` |
| Extraction chunk | `codenoesis.extraction-chunk/v4` |
| Extraction contract | `codenoesis.extraction/v4` |
| Knowledge graph | `codenoesis.knowledge-graph/v4` |
| Rust/Cargo ontology | `codenoesis.ontology/rust/v4` |
| Cargo manifest index | `codenoesis.cargo-manifest-index/v1` |
| Error | `codenoesis.error/v11` |
| Pipeline | `codenoesis.pipeline/s4-r4-v1` |
| Cargo manifest extractor | `codenoesis.cargo-manifest/s4-r4-v1` |
| Workspace extractor | `codenoesis.rust-workspace/s4-r3-v1` |
| Tree-sitter extractor | `codenoesis.rust-tree-sitter/s4-v1` |
| Boundary projection | `codenoesis.repository-boundaries/v1` |

The independent R2 repository-boundary projection remains present if and only
if its selector is present. All inherited R0-R3 schemas, fixtures, ontology
bytes, identity recipes, errors, and selector precedence remain immutable.

## Declaration authority

R4 claims only that a bounded literal declaration exists in one committed
UTF-8 `Cargo.toml` blob or that one bounded presence fact follows from the
already bound immutable inventory. Every supported value, list member,
relationship, diagnostic, and coverage item resolves to one or more exact
UTF-8 byte spans.

R4 never claims:

- an active feature, target, platform, compiler, or `cfg` world;
- a resolved dependency graph, package version, registry, or source;
- fetched Git, registry, patch, path-dependency, or generated content;
- an applied patch or effective workspace-inherited value;
- Cargo validation equivalence;
- executed Cargo, rustc, build script, procedural macro, test, example, bench,
  binary, library, hook, filter, or dependency code.

The graph therefore forbids `DEPENDS_ON`, `RESOLVES_TO`, `ACTIVATES`,
`SELECTS_TARGET`, `APPLIES_PATCH`, and `EXECUTES` for R4 facts. A later slice
may add stronger relationships only with separate authority and evidence.

## Input and normalization

R4 reads only committed manifest blobs already selected and bounded by R3. No
parent manifest, filesystem ancestor, Cargo configuration, environment value,
lockfile, registry cache, generated metadata, URL, or local checkout outside
the immutable repository participates.

- TOML strings are normalized to Unicode NFC for comparison and identity.
- Declaration names that collide after normalization fail closed.
- Manifest and repository-relative paths use the accepted slash-separated R3
  path model.
- A literal relative path is normalized lexically against its declaring
  package root and must remain inside the bound repository.
- Normalization grants no authority to stat, open, traverse, or analyze the
  referenced dependency or patch path.
- Duplicate normalized declaration identities fail rather than depending on
  table order.
- Unknown keys fail closed unless the machine subset assigns the exact field
  or family a typed unsupported diagnostic and coverage capability.
- Changing bound bytes fail before publication.

Manifest table order, field order, supported-array order where Cargo treats it
as a set, file order, scheduler order, and worker count never change semantic
bytes. Declaration evidence retains source offsets without entering entity or
relationship identity.

## Ontology and identities

Rust ontology v4 extends v3 without changing any v3 Rust entity,
relationship, claim, evidence, crate, workspace, or coverage identity domain
for unchanged facts. It adds disjoint Cargo declaration domains:

```text
codenoesis.entity-id/cargo-manifest/v1
codenoesis.relationship-id/cargo-manifest/v1
codenoesis.coverage-gap-id/v3
```

New entity kinds are exactly:

- `cargo.manifest`;
- `cargo.workspace_package_defaults`;
- `cargo.package`;
- `cargo.target`;
- `cargo.dependency`;
- `cargo.feature`;
- `cargo.patch`;
- `cargo.build_script`.

New relationship kinds are exactly:

- `DECLARES` from a manifest or package to an owned declaration;
- `REFERENCES_DECLARATION` from a package or member dependency to the exact
  root workspace declaration it names;
- `MATERIALIZES` from a declared lib/bin target to the unchanged R3
  `rust.crate` structurally extracted from that target.

`MATERIALIZES` is a derived structural correspondence, not an active Cargo
target claim. Example, test, and bench declarations have no Rust crate and no
`MATERIALIZES` edge in R4.

Cargo entity IDs use structural owner and declaration names, never declaration
values, source offsets, locator plaintext, commit IDs, feature order, or
scheduler order. This keeps an entity stable across a version requirement or
metadata value change and enables later semantic comparison. Patch source URLs
use only their SHA-256 digest where the selector itself is an identity input.

Every Cargo entity and relationship has exactly one claim. Literal declaration
and reference claims are `deterministic_fact`; R3 target correspondence and
conventional/absent build-script presence are `derived_fact`. Cargo subjects
may not use candidate, reviewed-inference, confirmation, rejection, or
supersession states in R4.

## Package metadata

The closed package metadata subset supports:

- string fields `version`, `edition`, `rust-version`, `description`, `license`,
  `default-run`, and `links`;
- locator fields `documentation`, `homepage`, and `repository` as SHA-256 only;
- normalized path fields `license-file` and `readme`;
- bounded string arrays `authors`, `keywords`, `categories`, `include`, and
  `exclude`;
- `publish` as false or a bounded registry-name array;
- literal auto-discovery booleans `autolib`, `autobins`, `autoexamples`,
  `autotests`, and `autobenches`.

The machine subset fixes which fields may use `{ workspace = true }`.
Inheritance emits the member declaration, the root default declaration, and a
`REFERENCES_DECLARATION` relationship. It does not copy or claim an effective
value. `include` and `exclude` are represented but never applied and therefore
emit `cargo.package_file_selection_not_applied`.

Literal auto-discovery switches are represented. If a switch contradicts the
already accepted R3 source-root plan, extraction fails with
`unsupported_structural_interaction`; R4 does not silently change R3 crate
identity or invent Cargo target discovery.

Arbitrary `[package.metadata]` and `[workspace.metadata]` content remains
uninterpreted. Each encountered table emits one exact unsupported diagnostic
and `cargo.package_metadata_table_unsupported` coverage with table evidence.

## Target declarations

R4 represents explicit `[lib]`, `[[bin]]`, `[[example]]`, `[[test]]`, and
`[[bench]]` declarations. The closed field set is `name`, `path`,
`required-features`, `crate-type`, `proc-macro`, `bench`, `doc`, `doctest`,
`test`, `harness`, and `edition`.

Names and paths use literal evidence or the already reviewed R3 lexical default
when omitted and unambiguous. Lib/bin targets structurally analyzed by R3 link
to their unchanged crate IDs. Example/test/bench declarations remain entities
with `source_analysis_state = not_analyzed`, no source parsing, and exact
`cargo.target_source_not_analyzed` coverage.

`required-features` names are declarations only. `proc-macro = true` emits
`cargo.proc_macro_not_executed`. No option activates, compiles, executes, or
selects a target.

## Dependency declarations

R4 accepts bounded literal string, inline-table, and standard-table entries in:

- package and workspace `dependencies`;
- package `dev-dependencies` and `build-dependencies`;
- target-specific normal, dev, and build dependency tables.

The target table key is retained as a normalized literal predicate and enters
dependency identity; it is never evaluated.

Supported fields are `version`, `registry`, `path`, `git`, `branch`, `tag`,
`rev`, `package`, `optional`, `default-features`, `features`, and `workspace`.
The closed source kinds are:

- `registry_default`;
- `registry_named`;
- `path`;
- `git`;
- `workspace_inherited`.

`workspace = true` names one exact root workspace dependency and emits a
reference relationship without materializing its effective fields. Git accepts
at most one of branch, tag, or rev. A version requirement may accompany path or
Git as a declaration but is not checked against source content. Invalid field
combinations fail closed.

Dependency keys, optional flags, default-feature flags, package aliases,
version requirements, registry names, and requested feature names remain
declarations with evidence. Optional dependencies do not cause implicit
feature entities. Requested features are never activated. Path dependencies
are never opened. Git and registries are never contacted. No dependency entity
targets an external package entity in R4.

Artifact/lib/target/public dependency fields are typed unsupported coverage;
unknown dependency keys outside that reviewed family fail closed.

## Feature declarations

Each `[features]` entry is one `cargo.feature` entity. Members retain exact
bounded syntax and evidence under four closed classes:

- bare `name`;
- explicit dependency `dep:name`;
- dependency feature `name/feature`;
- weak dependency feature `name?/feature`.

A bare member is intentionally not classified as either a local feature or an
implicit optional dependency. `default` is an ordinary declared feature, not
an active set. Duplicate or malformed normalized members fail closed. No
activation or dependency edge is emitted.

## Patch declarations

R4 represents root patch declarations for crates.io, a named registry, or a
source locator. Patch entries reuse the approved declaration-only source
fields. Source locators are digest-only. Every patch entity has
`applied = false` and `cargo.patch_not_applied` coverage. Patch target content
is never opened, selected, fetched, or compared.

`[replace]` remains typed unsupported coverage. Unknown patch shapes fail
closed.

## Build scripts and generated source

Every analyzed package has one bounded build-script presence projection with
one of:

- `explicit_path`;
- `explicit_disabled`;
- `conventional_present`;
- `absent`.

Explicit and conventional paths normalize inside the repository. Presence is
checked only against the immutable inventory. `executed` is always false.
Present scripts emit `cargo.build_script_not_executed` and
`cargo.generated_source_not_analyzed`. No generated file becomes repository or
ontology evidence.

## External locator handling

Package metadata URLs, dependency Git locators, Git branch/tag/rev values, and
patch source locators are untrusted and may contain credentials. R4 accepts at
most 4,096 input bytes and emits only lowercase SHA-256, `redacted = true`, and
the source evidence ID.

Locator plaintext is forbidden in snapshots, graph properties, IDs, errors,
logs, diagnostics, generated documentation, local query results, telemetry,
or pilot evidence. The committed manifest blob remains the sole source of the
original bytes. Registry names and normalized local paths are not external
locators and retain their bounded declared form.

## Diagnostics and coverage

Supported declarations produce entities even though their effective Cargo
meaning is unresolved. The exact capability-to-state map is closed in ontology
v4. It distinguishes `not_resolved`, `not_fetched`, `redacted`, `not_applied`,
`not_executed`, `not_analyzed`, and `unsupported`.

Typed unsupported families are exactly package/workspace metadata tables,
profile tables, package/workspace lint tables, replace tables, and the reviewed
advanced dependency fields. Every encountered unsupported field or family has
evidence and a matching diagnostic; it is never silently ignored.

## ExtractionChunkV4 and KnowledgeGraphV4

ExtractionChunkV4 has an explicit subject:

- `cargo_manifest` binds a manifest entity and canonical path;
- `rust_source` binds manifest, unchanged R3 crate/source IDs, and workspace
  provenance.

Its entity and relationship unions retain every inherited v2 Rust shape and add
strict Cargo shapes. KnowledgeGraphV4 contains one deterministic manifest index
ordered by path. Each entry binds a manifest ID, optional package ID, and
stable-ID-sorted Cargo fact IDs. Entity, relationship, claim, evidence,
diagnostic, and coverage collections are bounded and semantically validated.

## ErrorV11

New R4 failures emit one LF-terminated strict ErrorV11, empty stdout, and no
publication. Context is normalized and bounded and never contains declaration
values, source bytes, locator plaintext, absolute paths, environment data, or
secrets.

| Code | Stage | Exit | Context |
|---|---|---:|---|
| `input.invalid_manifest_profile` | `input` | 2 | empty |
| `extraction.invalid_cargo_manifest_fact` | `extraction` | 11 | reason, canonical manifest path, fact kind, optional field code |
| `extraction.cargo_manifest_fact_conflict` | `extraction` | 11 | canonical manifest path, fact kind, declaration-name SHA-256 |
| `extraction.cargo_manifest_fact_limit_exceeded` | `extraction` | 11 | limit, maximum, observed |
| `internal.unexpected` | `internal` | 70 | empty |

All ErrorV11 values are non-retryable. R0-R3 acquisition, boundary, workspace,
and target failures retain their accepted earlier error lineage and precedence
before R4 declaration projection.

## Fixed limits

Every new cardinality is checked at maximum and maximum-plus-one before
allocating proportional output:

| Limit | Maximum |
|---|---:|
| Cargo fact entities per graph | `10,000` |
| Dependencies per manifest | `256` |
| Features per manifest | `256` |
| Members per feature | `128` |
| Explicit targets per package | `128` |
| Patches per workspace | `256` |
| Metadata fields per package/default owner | `32` |
| Requested features per dependency or target | `64` |
| Target predicates per manifest | `128` |
| One declaration string | `2,048` bytes |
| One external locator input | `4,096` bytes |
| Determinism | `50` permutations plus shuffled parallel replay |

Manifest bytes, manifest count, path depth/bytes, source files, snapshot, graph,
documentation, query, wall time, and memory retain inherited R1-R3/S4 bounds.
There is no best-effort truncation, automatic retry, directory recursion for
dependency discovery, unbounded TOML walk, Cargo metadata invocation, or
ambient cache use.

## Snapshot, publication, docs, and query

RepositorySnapshotV7 contains the existing immutable repository binding and
inventory, ConfigurationV4, optional R2 boundary projection, ExtractionChunkV4
items, KnowledgeGraphV4, ontology/pipeline/extractor/evidence versions, and the
existing non-semantic envelope.

V7 semantic, graph, chunk, and configuration hashes use complete RFC 8785
payloads and new BLAKE3-256 domains. Atomic publication reuses the existing
three artifact roles, marker, DDL, CAS staging, metadata transaction, head
validation, failpoints, crash semantics, and sweep behavior. No migration,
repair, deletion, new role, or destructive action is authorized.

DocumentationManifestV1 and LocalQueryResultV1 retain their public schemas and
may bind a validated V7 head. Documentation must label R4 values as declared,
surface every gap, and never use “resolved”, “active”, “selected”, “applied”, or
“executed” for R4-only evidence. Exact-ID query resolves Cargo entities,
relationships, claims, evidence, diagnostics, and gaps without exposing locator
plaintext.

## Security boundary

Manifests, locators, paths, source, metadata, docs roots, pilots, and generated
text are untrusted. The selected path:

- launches no process and opens no network or DNS channel;
- executes no Cargo, rustc, build script, proc macro, target, hook, filter,
  dependency, or generated source;
- reads no path dependency, patch target, registry cache, Git locator, or path
  outside approved immutable roots;
- follows no symlink, reparse point, environment expansion, Cargo config, or
  URL;
- writes only through existing isolated store and generated-doc adapters;
- retains no locator plaintext in derived data;
- uses no first-party `unsafe`, model provider, automatic repair, or retry.

## Acceptance oracle

The project-owned fixture and machine oracle bind:

1. two manifest entities and exact byte evidence;
2. workspace defaults plus direct and inherited package metadata;
3. locator digest projection for package, dependency, Git reference, and patch
   source values;
4. lib/bin/example target declarations, required features, proc-macro flag,
   unchanged R3 crate IDs, and non-analyzed target coverage;
5. normal/dev/build/workspace/target-specific dependency declarations across
   every source kind;
6. normalized path declarations whose target paths are deliberately absent;
7. all four feature-member syntaxes without activation;
8. patch declaration identities with `applied = false`;
9. build, binary, example, and proc-macro no-execution sentinels;
10. exact unsupported metadata/profile/lint diagnostics and coverage;
11. malformed, duplicate, conflicting, escaping, unsupported, and Unicode
    variants;
12. every maximum and maximum-plus-one;
13. 50 permutations and shuffled parallel replay;
14. V7 publication, restart, docs, and exact-ID query;
15. byte-identical V6 and earlier selector-absent regressions;
16. non-vendored pinned Lekton and RustDesk observations.

Golden changes require human semantic review, not snapshot regeneration alone.

## First implementation Red

Against required base `ea51a8151749fc65e75dd7a10e550adc0b67d422`,
the unknown selector produces exactly:

- exit `2`;
- empty stdout, SHA-256
  `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`;
- stderr
  `{"code":"input.invalid_revision","context":{},"message":"invalid revision","retryable":false,"schema_version":"codenoesis.error/v4","stage":"input"}\n`;
- stderr length `149` and SHA-256
  `7f75f7a91f6af0328795f3fbd2729e69756beba2ebd642cc1f6401265662a2fe`;
- absent store and zero acquisition, process, network, resolver, target,
  dependency-read, and publication authority.

The later implementation issue must retain this Red in the named outside-in
test before product code. Compilation, fixture, acquisition, dependency,
panic, timeout, execution, side-effect, or different subject failures are
rejected reasons. The independent governance guard Red is retained in the R4
red-observation artifact.

## Public pilots

Pilots use existing reviewed descriptors and never vendor external source.

- Lekton commit `7a4d1a4a30468f4c18ce158a9b825680b00f4820`
  must complete R1+R3+R4 scan, docs, and exact-ID query with declaration-only
  manifest facts and explicit unresolved coverage.
- RustDesk commit `d412d198720aa56f6cfed2dfad262e8fb1322fb7`
  must complete R1+R2+R3+R4 scan, docs, and exact-ID query while
  `libs/hbb_common` remains the sole unbound boundary at
  `69cea8dafee147848ae88702029f4bf7df7224c3`; nested source remains unopened.

Pilot counts, diagnostic inventories, and timings are reproducible evidence,
not repository-specific requirements or synthetic goldens. Corpus revisions
are replaceable only through reviewed descriptor changes.

## Compatibility and rollback

R4 is additive and selector-bound. Governance adds no runtime behavior. The
later product package may be rolled back as one unit before release, restoring
manifest-selector rejection without changing R0-R3 bytes or migrating/deleting
data. Pilot and test V7 stores are disposable. A post-release downgrade policy
for a visible V7 head requires a separate decision.

## Consequences

Positive:

- users can query exact Cargo declarations and evidence before resolver-grade
  support exists;
- semantic comparison can later distinguish declaration changes using stable
  IDs;
- external locators remain useful through digest comparison without plaintext
  leakage;
- inherited metadata and dependencies are linked without semantic overclaim;
- R3 source ontology and all prior invocation bytes remain compatible.

Costs and limitations:

- R4 is not Cargo metadata or resolver equivalence;
- effective features, targets, dependency versions/sources, patches, generated
  code, compiler facts, and cross-crate semantic resolution remain deferred;
- typed entity/property schemas and V7 increase implementation and review
  surface;
- compiler-grade and implementation-aware relations still require R5/R7 or
  later approved capabilities.

## Contract bundle

The SRS ratification register binds the canonical R4 governance bundle digest.
The bundle covers this decision, independent Python guard, strict schemas,
machine subset and oracle, Red observation, project-owned fixture, exact
identity/evidence plan, immutable R3 bundle dependency, and reviewed pilot
pins. The SRS is excluded to avoid a circular digest. Any bound-byte change
requires a new digest and renewed human review.
