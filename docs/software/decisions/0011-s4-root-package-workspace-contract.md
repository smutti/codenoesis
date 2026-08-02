# Decision 0011: S4 root-package workspace compatibility contract

| Field | Value |
|---|---|
| Status | Proposed; becomes Accepted only when `@smutti` manually merges the exact protected head of PR #97 |
| Date | 2026-08-02 |
| Owners | Andrea Moretti (`@smutti` governance persona), accountable maintainer `@smutti` |
| Scope | `S4 — Evidence-backed workspace docs compatibility extension` only; roadmap `R3` |
| Requirement | `FR-EXT-008` |
| Governance issue | [#96](https://github.com/smutti/codenoesis/issues/96) |
| Authorization | [Maintainer comment](https://github.com/smutti/codenoesis/issues/96#issuecomment-5158164180) |
| Corrections | [Round 1](https://github.com/smutti/codenoesis/issues/96#issuecomment-5158197452), [round 2](https://github.com/smutti/codenoesis/issues/96#issuecomment-5158328256) |
| Required base | `36e8fefe37e21ac936dedda9198465d655cebeba` |

## Context

The implemented S4 profile intentionally recognizes only one strict virtual
Cargo workspace shape. Its root manifest must contain only `[workspace]`, its
members must be literal non-root paths, and each member manifest must fit the
approved structural subset. This was sufficient to ratify multi-crate Rust
ontology v2 without deciding Cargo's broader manifest semantics.

Roadmap R1 and R2 now remove the preceding packed-object and gitlink blockers.
The next ordinary-repository blocker is the Cargo root package:

- a standalone `[package]` is one package workspace;
- a manifest containing `[package]` and `[workspace]` has an implicit root
  member even when `"."` is absent;
- some repositories list `"."` explicitly;
- real roots contain exclusions, multiple targets, build declarations,
  features, dependencies, patches, and target-specific tables whose full
  meaning belongs to R4 or later;
- a workspace member may be a committed gitlink and must remain an external R2
  boundary rather than source owned by the root repository.

Running Cargo, rustc, build scripts, procedural macros, target code, Git, a
model provider, or a network client would violate the deterministic local
path. Silently ignoring unsupported manifest meaning would violate the
evidence model. Reusing the selector-absent S4 output version would silently
change an accepted public contract.

## Decision

Add one explicit workspace compatibility selector:

```text
--workspace-profile cargo-root-package-v1
```

It is valid only for `scan --profile standard-local-s4`. Repository contents
never select it implicitly. The selector is independent of, and may compose
with, the implemented R1 `local-git-sha1-packed-v1` acquisition selector and
R2 `local-gitlinks-v1` repository-boundary selector.

The selected profile emits a strict additive V6 lineage. Every invocation
without the selector remains byte-for-byte compatible, including the accepted
root-package `extraction.unsupported_workspace` failure.

## Public versions

The selected path uses exactly:

| Contract | Version |
|---|---|
| Snapshot | `codenoesis.repository-snapshot/v6` |
| Configuration | `codenoesis.configuration/v3` |
| Extraction chunk | `codenoesis.extraction-chunk/v3` |
| Extraction contract | `codenoesis.extraction/v3` |
| Knowledge graph | `codenoesis.knowledge-graph/v3` |
| Rust ontology | `codenoesis.ontology/rust/v3` |
| Error | `codenoesis.error/v10` |
| Pipeline | `codenoesis.pipeline/s4-r3-v1` |
| Workspace extractor | `codenoesis.rust-workspace/s4-r3-v1` |
| Tree-sitter extractor | `codenoesis.rust-tree-sitter/s4-v1` |
| Boundary projection | `codenoesis.repository-boundaries/v1` |

V6 contains one canonical repository-boundary projection if and only if the
independent R2 selector is present. Without R2 the member is absent. A root
containing a gitlink still requires the explicit R2 selector before extraction
begins.

## Workspace root classification

The committed root `Cargo.toml` is classified into exactly one shape:

1. `standalone_root_package`: `[package]` exists and `[workspace]` is absent;
2. `virtual_workspace`: `[workspace]` exists and `[package]` is absent;
3. `non_virtual_workspace`: both tables exist.

Any other root, duplicate table, non-table value, malformed TOML, non-UTF-8
content, unsupported structural key shape, or changing bound input fails
closed. No parent manifest, environment variable, filesystem ancestor, Cargo
configuration, lockfile, or generated metadata participates.

## Canonical member plan

Member planning is deterministic and precedes source parsing.

- A standalone root package contributes canonical member path `.`.
- A non-virtual workspace contributes canonical member path `.` implicitly.
- A literal workspace member `"."` confirms the same root member; it never
  creates a second member.
- Other members are normalized slash-separated relative paths.
- Empty segments, absolute paths, `.` or `..` segments outside the exact root
  token, backslashes, NUL, control bytes, normalization collisions, and glob
  metacharacters are rejected.
- Literal exclusions use the same path normalization, except `.` is forbidden.
- A non-root path present in both members and exclusions is a conflict and
  fails closed rather than guessing Cargo precedence.
- Member order, exclusion order, TOML table order, file order, and scheduler
  order never affect semantic bytes or stable identities.
- Every planned non-root package requires exactly one committed UTF-8
  `<member>/Cargo.toml`. Missing, duplicate, symlinked, gitlink-owned, or
  non-regular manifests do not grant traversal authority.

The input limit counts at most 200 literal `workspace.members` entries. A
non-virtual root that omits `"."` may therefore project at most 201 members:
one implicit root plus 200 literal entries. The independent package-manifest
limit remains 200, so any additional projected entries must be excluded or
external boundaries rather than silently analyzed packages.

The root package is represented once even when it is both implicit and
explicit. Its membership provenance is `implicit_root_package` when `"."` is
absent and `explicit_root_member` when present. This provenance is evidence,
not an identity input.

## Package and target subset

R3 reads only structural facts needed to select source roots:

- package `name` and `edition` as literal non-empty strings;
- optional literal `lib.name` and `lib.path`;
- optional literal `[[bin]].name` and `[[bin]].path`;
- conventional `src/lib.rs` and `src/main.rs`;
- conventional `src/bin/<name>.rs` and `src/bin/<name>/main.rs`;
- root source files that are committed regular UTF-8 files inside the package
  root.

Target names omitted from explicit declarations use the reviewed Cargo lexical
default only when unambiguous: package name for `src/lib.rs` and
`src/main.rs`, filename or directory name for conventional `src/bin` roots.
Name normalization never evaluates Cargo configuration. Exact matching
explicit and conventional declarations normalize to one target. Duplicate
target kind/name pairs with different roots, duplicate source roots with
different identities, any other explicit/conventional collision, escaping
paths, ambiguous conventional forms, unsupported target kinds, and missing
explicit paths fail closed. Targets are ordered by manifest path, then `lib`
before `bin`, then target name and source path, using unsigned UTF-8 bytes.

Each library or binary target remains one `rust.crate` entity under the v2
identity preimage:

```text
repository_identity
manifest_relative_path
package_name
target_kind
target_name
```

The root manifest path is always `Cargo.toml`, never `.`, an absolute path, or
an environment-derived value.

## Deferred Cargo meaning

R3 does not claim a Cargo resolution. The following reviewed families are
accepted only as bounded typed coverage gaps attached to manifest evidence:

- package metadata outside `name` and `edition`;
- `dependencies`, `dev-dependencies`, and `build-dependencies`;
- target-specific dependency tables;
- `features` and optional dependency activation;
- `patch`, `replace`, registry, Git, and path resolution;
- `required-features`, `default-run`, examples, tests, benches, and proc-macro
  meaning;
- build-script presence and generated source;
- workspace dependency/package inheritance;
- cfg, platform, compiler, and feature worlds.

The machine contract contains a closed capability code for each accepted
family, including `cargo.proc_macro_not_executed`. Automatic target switches
that alter source-root selection are structural rather than deferred and fail
closed. An unknown top-level structural family, malformed deferred value, or a
field that can change target-root selection outside the approved subset also
fails closed.
Nothing is silently treated as active, absent, resolved, fetched, or executed.

## Gitlink members

A planned member whose committed root entry is mode `160000` is not opened as
a package manifest and never becomes a root Rust crate.

- without R2, inherited acquisition rejection remains authoritative;
- with R2, the member must resolve to exactly one V1 external boundary;
- the workspace projection emits `external_gitlink_member_not_analyzed` with
  root-manifest evidence and the boundary ID;
- an optional depth-one nested R2 binding verifies identity/commit/tree only;
  it still grants no R3 source-analysis authority;
- nested docs, query, ontology merge, recursion, and federation are deferred.

## Ontology and identity compatibility

Rust ontology v3 keeps the v2 entity kinds, relationship kinds, cardinalities,
claim states, stable-ID preimages, and identity domains for unchanged facts.
It adds:

- closed crate property `workspace_member_source` with
  `literal_member`, `implicit_root_package`, or `explicit_root_member`;
- closed root-shape property with `standalone_root_package`,
  `virtual_workspace`, or `non_virtual_workspace`;
- reviewed R3 coverage capability codes;
- an external-boundary reference only on the gitlink-member coverage item.

Membership provenance and root shape are semantic properties but never stable
entity-ID inputs. Scanning the accepted S4 virtual fixture through R3 must
retain every v2 entity and relationship ID even though the enclosing V6
semantic hash differs. Selector-absent V4/V5 bytes do not change.
Workspace members are path sorted, while each member's `crate_ids` are stable-ID
sorted independently of target discovery order.

## ErrorV10

New R3 failures emit one LF-terminated strict ErrorV10, empty stdout, and no
publication. Context values are closed, normalized, bounded, and never contain
source bytes, absolute paths, environment data, dependency URLs, or secrets.

| Code | Stage | Exit | Context |
|---|---|---:|---|
| `input.invalid_workspace_profile` | `input` | 2 | empty |
| `extraction.invalid_workspace_manifest` | `extraction` | 11 | `reason`, optional canonical `path` |
| `extraction.workspace_member_conflict` | `extraction` | 11 | canonical `path` |
| `extraction.workspace_target_conflict` | `extraction` | 11 | canonical `path`, `target_kind`, `target_name` |
| `extraction.root_package_limit_exceeded` | `extraction` | 11 | `limit`, `maximum`, `observed` |
| `internal.unexpected` | `internal` | 70 | empty |

All ErrorV10 values are non-retryable. Earlier acquisition and R2 boundary
failures retain their accepted error lineage and retryability before R3.

## Fixed limits

The selected path enforces maximum and maximum-plus-one for every new
cardinality before allocating proportional output:

| Limit | Maximum |
|---|---:|
| Committed manifest bytes | inherited `4,194,304` per file |
| Literal workspace members | `200` |
| Projected members after implicit root insertion | `201` |
| Literal exclusions | `200` |
| Package manifests | `200` |
| Total crate targets | inherited `200` |
| Binary roots per package | `64` |
| Target/member path bounds | inherited S1 path bytes/depth |
| Snapshot, graph, docs, query, wall time, memory | inherited S1/S4 bounds |
| Determinism | `50` permutations plus shuffled parallel replay |

No glob expansion, directory recursion for member discovery, unbounded TOML
walk, Cargo metadata invocation, automatic retry, or best-effort truncation is
allowed. Conventional `src/bin` inspection is bounded to committed immediate
children already present in the immutable inventory.

## Snapshot, publication, docs, and query

RepositorySnapshotV6 contains:

- the existing immutable repository binding and inventory;
- configuration v3 with both `standard-local-s4` and the exact workspace
  selector;
- the canonical repository-boundary projection only when R2 is selected;
- extraction v3 chunks and knowledge graph v3;
- ontology, pipeline, extractor, and evidence-lineage versions;
- the existing non-semantic envelope.

V6 semantic, graph, and chunk hashes use the existing complete RFC 8785 and
BLAKE3-256 rules under new versioned domains. Atomic publication reuses the
existing three artifact roles, local-store marker, DDL, CAS staging, metadata
transaction, head validation, crash semantics, and sweep behavior. No schema
migration or new role is authorized.

DocumentationManifestV1 and LocalQueryResultV1 retain their public schemas.
Their snapshot bindings may name a validated V6 head. Generated prose must
surface R3 coverage gaps and may not state deferred Cargo behavior as fact.

## Security boundary

Cargo manifests, source, gitlinks, boundary metadata, docs roots, and corpus
repositories are untrusted input. The selected path:

- launches no process and opens no network channel;
- executes no Cargo, rustc, build script, proc macro, hook, filter, target,
  dependency, or generated source;
- reads no path outside the approved immutable root and explicit R2 roots;
- follows no symlink, reparse point, alternate object store, environment
  expansion, or URL;
- writes only through the existing isolated store and generated-doc adapters;
- retains no raw external URL, absolute nested path, secret, or environment
  value in output, errors, logs, evidence, or IDs;
- uses no first-party `unsafe`.

## Acceptance oracle

The project-owned fixture and machine oracle bind:

1. unchanged virtual-workspace control;
2. standalone root package;
3. implicit non-virtual root plus member;
4. explicit `"."` root with the same root crate IDs;
5. literal exclusions and member/exclude conflict;
6. conventional and explicit lib/bin roots, including multiple bins;
7. duplicate, escaping, missing, ambiguous, malformed, and Unicode cases;
8. exact R4-deferred coverage codes;
9. non-executed build/proc-macro/target sentinels;
10. R2 gitlink-member composition;
11. every maximum and maximum-plus-one;
12. 50 permutations and shuffled parallel replay;
13. V6 publication, docs, and exact-ID query;
14. byte-identical V4/V5 selector-absent regressions;
15. non-vendored pinned Lekton and RustDesk observations.

Golden changes require human semantic review, not snapshot regeneration alone.

## First implementation Red

Against required base `36e8fefe37e21ac936dedda9198465d655cebeba`,
the unknown selector produces exactly:

- exit `2`;
- empty stdout, SHA-256
  `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`;
- stderr
  `{"code":"input.invalid_revision","context":{},"message":"invalid revision","retryable":false,"schema_version":"codenoesis.error/v4","stage":"input"}\n`;
- stderr length `149` and SHA-256
  `7f75f7a91f6af0328795f3fbd2729e69756beba2ebd642cc1f6401265662a2fe`;
- absent store and zero acquisition, process, network, target, and publication
  authority.

The later implementation issue must retain this Red before production code.
Compilation, fixture, dependency, acquisition, panic, timeout, or side-effect
failures are rejected reasons.

## Public pilots

The pilots use existing reviewed descriptors and never vendor external source.

- Lekton commit `7a4d1a4a30468f4c18ce158a9b825680b00f4820`
  must complete R1+R3 scan, docs, and exact-ID query with one explicit root and
  one CLI member; build/features/dependencies remain gaps.
- RustDesk commit `d412d198720aa56f6cfed2dfad262e8fb1322fb7`
  must complete R1+R2+R3 scan, docs, and exact-ID query while
  `libs/hbb_common` remains the sole unbound boundary at
  `69cea8dafee147848ae88702029f4bf7df7224c3`; nested source remains unopened.

Pilot counts and timings are reproducible evidence, not repository-specific
requirements or synthetic goldens.

## Compatibility and rollback

This change is additive and selector-bound. Governance adds no production
behavior. The later product package may be rolled back as one unit before
release, restoring selector rejection without migrating or deleting data.
Pilot and test V6 stores are disposable. A post-release downgrade policy for a
visible V6 head requires a separate decision.

## Consequences

Positive:

- R1-R3 can analyze common real Cargo roots without target execution;
- implicit and explicit root membership is deterministic and evidence-backed;
- R4 semantics remain honest gaps instead of guessed Cargo state;
- gitlink members remain outside root ontology authority;
- old profiles and stored outputs remain compatible.

Costs and limitations:

- R3 is a strict structural subset, not Cargo metadata equivalence;
- globs, inherited workspace values, generated manifests, examples/tests/
  benches, active feature worlds, dependency resolution, and compiler facts
  remain deferred;
- ontology v3 and V6 add implementation and compatibility surface;
- full cross-crate semantic resolution still requires later R5/R7 work.

## Contract bundle

The SRS ratification register binds the canonical R3 governance bundle digest.
The bundle covers this decision, the independent Python guard, strict schemas,
machine oracle, project-owned fixture, exact Red, immutable S4 and R2 bundle
dependencies, and reviewed pilot pins. The SRS is excluded from the bundle to
avoid a circular digest. Any bound-byte change requires a new digest and
renewed human review.
