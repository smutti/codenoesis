# Decision 0048: Cargo workspace package edition inheritance

- Status: Approved and Implemented
- Date: 2026-09-04
- Issue: [#219](https://github.com/smutti/codenoesis/issues/219)
- Pull request: [#220](https://github.com/smutti/codenoesis/pull/220)
- Merge commit: `ddb0acafeaa18ea1e01ba9bc48502974c68aa97a`
- Exact dependent base: `5a3671d590d7e4ad864fe4fbe277add636ddf843`
- Requirement: `FR-EXT-022`
- Slice: `S4`
- Dependencies: none
- Owner and approver: `@smutti`

## Context

After deterministic one-level member expansion, the pinned wgpu workspace
reaches `benches/Cargo.toml` and stops because the R3 root-package planner
requires a direct package edition string. Cargo manifests commonly declare the
required edition through `edition.workspace = true`, with its value owned by
the committed root `[workspace.package]` table.

R4 already represents package workspace references as declarations without
resolving them. The planner needs only enough information to validate that a
package has a usable edition before selecting source targets; it does not need
to infer or publish an effective metadata value.

## Decision

The `cargo-root-package-v1` planner accepts the required package `edition` in
either of two exact forms:

1. an existing non-empty direct string; or
2. a table containing only `workspace = true`, when the committed root
   `[workspace.package]` table contains a non-empty string `edition`.

The rule applies equally to virtual-workspace members and a non-virtual root
package that inherits from its own workspace declaration. A missing, empty,
non-string, false, or structurally ambiguous declaration fails closed as the
existing `invalid_package_manifest` reason.

This validation does not copy the root value into package facts and does not
change public identities, schemas, ontology families, or serialization. R4
continues to emit the existing workspace-reference declaration for edition and
the other supported metadata fields. Other inherited fields remain outside R3
planning because they are not required to select package source targets.

## Safety and compatibility

Resolution reads only TOML values from root and member manifests already in the
immutable repository inventory. It performs no filesystem discovery, Cargo or
rustc invocation, dependency resolution, build-script or target execution,
network request, plugin call, model call, or best-effort repair.

Direct editions preserve their existing behavior and identities. Invalid
references retain the existing typed package-manifest failure. Historical R3
and R4 fixtures, manifest workspace-reference facts, schemas, ontology
contracts, and goldens remain unchanged.

## Acceptance and evidence

Synthetic tests cover virtual and non-virtual workspaces, direct compatibility,
missing and malformed root defaults, false and extra-key references, existing
R3 journeys, and unchanged R4 manifest facts. The complete public Rust corpus
must match all eight reviewed oracles.

Product commit `095518ec49ede920af90b5c408c58a7ab99fc754` moves pinned wgpu
beyond package-manifest validation. Its next deterministic boundary is a typed
`codenoesis.error/v5` `extraction.unsupported_workspace` failure during source
extraction. The public error contract exposes no internal subtype, so this
decision makes no stronger claim about the unsupported source form.

The release binary has SHA-256
`320111cedd71e5a5feb9faa6af999babbefb1ffef0d938d907f21b2ff805e724`.
The complete eight-repository evaluation matches 8/8 reviewed oracles with
report SHA-256
`e2c130e408964495ecad3f8dfb9470d3c29a34d6c9a29c639f72adacbeb085f1`.
All three wgpu samples return exit `11`, empty stdout, 169 stderr bytes, and the
same typed error. Aggregate stage coverage is acquisition 7/8, workspace 6/8,
manifest 5/8, semantic 4/8, framework 4/8, flow 3/8, and constant 3/8.

Protected PR #220 was independently reviewed and manually merged. The decision
is therefore Approved and Implemented. It changes no dependency, release,
signing, workflow, permission, support, SLO, or GA authority.
