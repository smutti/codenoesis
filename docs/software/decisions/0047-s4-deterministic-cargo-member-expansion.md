# Decision 0047: deterministic Cargo workspace member expansion

- Status: Proposed branch-scoped candidate
- Date: 2026-09-04
- Issue: [#217](https://github.com/smutti/codenoesis/issues/217)
- Exact dependent base: `b365d3069b8a1507d4a90dfd686a5c729d6d0747`
- Requirement: `FR-EXT-021`
- Slice: `S4`
- Risk: high
- Dependencies: none
- Correction budget: five rounds
- Owner and approver: `@smutti`

## Context

The RW5 public corpus showed that the pinned wgpu workspace stops at R3 with
`extraction.invalid_workspace_manifest/invalid_member_path`. Its committed root
manifest uses three declarations ending in `/*`. The existing R3 contract
accepts only literal members, although every candidate package manifest is
already present in the immutable acquired inventory.

This is a generic Cargo compatibility boundary. It must not be solved with a
wgpu-specific member list, ambient directory enumeration, Cargo metadata, or a
general-purpose glob engine whose semantics exceed the reviewed contract.

## Decision

The existing `cargo-root-package-v1` planner accepts one additive declaration
shape: a non-empty canonical relative prefix followed by the final component
`*`. The star matches exactly one non-empty path component. A match exists only
when the immutable repository inventory contains the committed regular file
`<prefix>/<child>/Cargo.toml`. Nested descendants do not match.

The planner sorts concrete paths by UTF-8 bytes and records each expanded
member as `glob_expanded_member`. An exact literal declaration for the same
concrete member takes provenance precedence. Multiple patterns may converge on
one concrete member without duplicating it, while duplicate raw declarations
remain invalid. A literal exclusion removes a pattern-derived match; the
existing literal-member/exclusion conflict remains a typed failure. A valid
pattern with zero committed matches fails as `invalid_member_path`.

The declaration count and expanded non-root member count are each bounded by
200. Existing projected member, package manifest, crate target, manifest byte,
path, and output limits still apply. Capacity is checked before package parsing
and no partial projection is returned.

Every other glob form is rejected: a root `*`, a star before the final
component, `**`, `?`, character classes, braces, empty or absolute prefixes,
backslashes, dot segments, escaping paths, and normalization collisions. A
pattern never matches a gitlink boundary because expansion requires a committed
regular package manifest in the root inventory.

## Safety and compatibility

Expansion is a pure operation over `RepositoryInventory`. It performs no
filesystem read, directory walk, symlink traversal, Git operation, submodule
fetch, Cargo or rustc invocation, build-script or target execution, network
request, model call, retry, or best-effort truncation.

The new provenance value is additive and is not an entity-identity input.
Literal workspaces preserve their existing plans, graph identities, semantic
bytes, schemas, fixtures, and goldens. Historical Decision 0011 remains the
literal-only R3 baseline; this decision narrowly extends only the reviewed
member-planning boundary.

## Acceptance and activation

The focused checkpoint test is Red on the exact base because `crates/*` is
rejected before planning. Green requires deterministic synthetic expansion,
literal exclusion and precedence, unsupported/empty-pattern rejection,
capacity boundaries, existing R3 and contract regressions, and a pinned wgpu
run that advances to its next honest typed stage. The complete eight-repository
RW5 oracle and repository gate must pass.

The candidate becomes Approved and Implemented only after independent review
and protected manual merge. It changes no release, signing, workflow,
permission, dependency, support, SLO, or GA authority.
