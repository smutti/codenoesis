# Decision 0029: R14/R15 real-repository fail-closed correction

| Field | Value |
|---|---|
| Status | Proposed branch-scoped candidate; becomes Accepted only after protected manual merge of the exact issue #170 pull request |
| Date | 2026-08-12 |
| Decider | `@smutti`, through the explicit issue #170 package authorization |
| Issue | [#170](https://github.com/smutti/codenoesis/issues/170) |
| Authorization | [accountable-maintainer authorization](https://github.com/smutti/codenoesis/issues/170#issuecomment-5266803744) |
| Base | `559fec3863830beef9fb4962d936c681a79c258e` |
| Requirements | Approved and Implemented, not Verified `FR-EXT-012`, `FR-EXT-016`, `FR-EXT-017`, `FR-EXP-006`, `FR-EXP-007`, `FR-CLI-001`; bounded amendments listed in issue #170 |
| Slice | `S4` |
| Risk | High: ontology extraction semantics, privacy, deterministic output, public CLI resource profile, and acquisition failure classification |
| Dependencies | No new dependency |

## Context

R14 and R15 are accepted source-only ontology layers, but ordinary supported
Rust repositories exposed four fail-open or over-strict boundaries. Complex
call target spelling could project arbitrary receiver source, including URL-
looking literals. R14 treated excluded arguments, receivers, initializers, and
callables intentionally absent from K1 as internal contract failures. R15 did
the same for source callables absent from its inherited R14/K1 authority.
Finally, an unselected repository gitlink could be collapsed into
`internal.unexpected` instead of the existing typed unsupported-composition
boundary.

Pinned diagnostic runs also showed that valid corrected Lekton V16/V17
snapshots exceed the existing optional 64 MiB final envelope while remaining
below 256 MiB. Decisions 0019, 0020, 0025, and 0027 are immutable historical
baselines and are not edited or reinterpreted by this correction.

## Decision

### Bounded call-target spelling

K1 preserves exact spelling only for direct identifier or scoped-identifier
targets, safe generic functions over those targets, and method targets whose
receiver is a recursively safe identifier, `self`, scoped identifier, or field
chain. Every call remains `candidate_unresolved` with the existing diagnostic
and `rust.call_target_resolution` gap unless the already accepted K1 rule
resolves it.

A complex field target publishes exactly
`<unsupported-receiver>.<field>`. Every other complex target publishes exactly
`<unsupported-call-target>`. Arbitrary receiver expressions, literals, raw
URLs, macro results, call chains, indexes, closures, bodies, and snippets do not
enter `name` or `target_spelling`. Existing call-site/evidence identity domains
and every historical simple K1 fixture byte remain unchanged.

### R14 partial-syntax boundary

R14 processes only callable declarations with one exact inherited K1
signature. A source callable intentionally absent from K1 is skipped without
R14 facts and without `CallSiteEvidenceMismatch`.

When any direct argument of one call has no selected R14 expression, R14 emits
zero `rust.call_argument`, `HAS_ARGUMENT`, and `ARGUMENT_VALUE` facts for that
whole call. It does not retain sparse source ordinals or renumber arguments.
When a method receiver has no selected R14 expression, `HAS_RECEIVER` is
omitted. In both cases the call expression, K1 call site, exact evidence, and
inherited typed uncertainty remain.

A let initializer outside the selected expression grammar preserves the
binding, emits `rust.pattern_input_unexpanded`, and emits no `BINDS_FROM`. It is
not an internal extraction failure.

### R15 inherited-callable authority

R15 compiles flow only for exact callable signatures present in the inherited
R14/K1 graph. A source callable absent from that graph is skipped. The existing
whole-callable rule is unchanged: an unsupported inherited callable emits zero
R15 facts and exactly the existing
`rust.syntax_normal_flow_not_analyzed` and
`rust.lexical_reaching_definitions_not_analyzed` capabilities.

### Repository-boundary rejection

R14 and R15 still do not compose with repository boundaries. Acquisition of
`UnsupportedRepositoryShape::SubmoduleOrGitlink` without a boundary selector
returns the existing expression or flow unsupported-composition error with
reason `repository_boundary_not_supported`, exit `2`, empty stdout, absent
store, and no nested read. Supplying `local-gitlinks-v1` remains an unsupported
pre-acquisition composition. No R11/R12/R14/R15 boundary composition is added.

### Explicit 256 MiB operational envelope

The only new selector is:

```text
--output-capacity-profile local-snapshot-256m-v1
```

It is accepted only by the exact complete R14 or R15 source-only scan matrix
and permits at most 268,435,456 canonical stdout bytes including the final LF.
Unknown, duplicate, incomplete, K1-only, R10-R13, boundary, cfg-alternative,
compiler/SCIP, nested/generated-source, and non-scan uses fail before
acquisition with empty stdout and no mutation.

The selector is operational and is not serialized into ConfigurationV13 or
ConfigurationV14: `output_capacity_profile` remains `null`. Therefore it cannot
change semantic payloads, hashes, graph counts, identities, documentation,
queries, portable projections, or explorer output for the same corrected
facts. Standard 32 MiB and historical `local-snapshot-64m-v1` bytes and
behavior remain unchanged. No schema or ontology family changes.

The new profile has a local acceptance ceiling of 4,294,967,296 peak RSS bytes
for scan and downstream processing. The inherited extraction deadline remains
60,000 ms. This package does not introduce streaming serialization, runtime OOM
prevention, a server SLO, or a general large-repository profile. Exceeding either
bound is a stop condition.

## Frozen fixture and oracle

The project-owned fixture
`urn:codenoesis:fixture:s4-rust-real-repository-shapes-v1` is commit
`accc966f2c2729dddc95fe7caf7036312a2a01e0` and tree
`389e22d33ed887e32da70c2d60ddd72893bf9c27`. It covers the simple and complex
targets, mixed closure argument, unsupported receiver, match initializer,
complete and unsupported inherited R15 callables, and direct `cfg(test)`
callable fixed by issue #170.

The machine oracle freezes every family through exact counts, canonical family
digests, complete sorted-ID digests, selected IDs and spans, placeholder
spellings, omitted edges, coverage records, one completed callable, five exact
blocks, and retained derivation count. It proves that `://` remains available
only in committed-source evidence and never in public K1/R14/R15 `name` or
`target_spelling`. Fifty option permutations and ten schedules must be byte-
identical.

On the corrected candidate the fixture has:

| Profile | Entities | Relationships | Claims | Evidence | Diagnostics | Coverage |
|---|---:|---:|---:|---:|---:|---:|
| R14 V16/V13 | 83 | 139 | 222 | 77 | 7 | 31 |
| R15 V17/V14 | 88 | 172 | 260 | 82 | 7 | 33 |

The complete oracle is
`tests/fixtures/s4/rust-real-repository-shapes-v1/expected-r14-r15-correction.json`
with SHA-256
`76286fc962b7ff0e2675d788761b3898f2a640ee6fc6d294553cb26ce16e7981`.

## Real-repository acceptance

The non-vendored positive pilot is `dghilardi/lekton` commit
`247b8f42fb045db41166d70a276a41c2e079b6eb`, tree
`55ba428493a4ffae86ba492422a049f46d567a30`. Under the 256 MiB selector, R14
must produce exactly 25,993 entities, 43,164 relationships, 69,157 claims,
24,365 evidence, 4,211 diagnostics, and 12,226 coverage records. R15 must
produce exactly 26,150 entities, 43,675 relationships, 69,825 claims, 24,522
evidence, 4,211 diagnostics, 13,172 coverage records, 147 completed callable
IDs, and 160 retained derivations. Both complete scan, docs, exact-ID query,
export, strict reimport, and explore twice with byte-identical non-envelope
results, clean source trees, no network, and no target execution.

The non-vendored typed-negative pilot is `rustdesk/rustdesk` commit
`d412d198720aa56f6cfed2dfad262e8fb1322fb7`, tree
`df8d4c292c9d256a445480eb878e507df3de1dc4`. Its one gitlink must produce the
typed `repository_boundary_not_supported` R14/R15 failures before store
creation and without reading nested source.

## Required delivery history

The branch preserves this order:

1. this decision, SRS/architecture/roadmap candidate, exact fixture and oracle,
   conformance guard, and executable CLI acceptance, with no production edit;
2. retained expected Red bound to that checkpoint;
3. minimum production correction, focused tests, and retained Green;
4. complete real-repository pilots and repository gate;
5. independent review and protected manual merge.

On the exact base the fixture R14 and R15 scans return the existing 196-byte
ErrorV21/ErrorV22 internal failures with SHA-256 respectively
`9b284f4bb7368bb0d11c5b33725c109ee469845aac00081e51175413adec4e3c` and
`01f7b883892dc357c177c072ce2cca62b3abbf3cbb22f40d173120a283181564`.
The unregistered 256 MiB selector fails before acquisition. Only retained
checkpoint-bound Red authorizes production edits.

## Consequences

R14/R15 gain a usable, bounded real-repository source-only journey while
remaining conservative about unsupported syntax and repository boundaries.
The correction improves privacy and error classification without adding facts,
resolution, compiler/runtime authority, schema versions, dependencies, or
release/control-plane effects. The behavior remains Proposed on the branch and
not Verified until the exact pull request is manually merged and independent
immutable evidence is accepted.
