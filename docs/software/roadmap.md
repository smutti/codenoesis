# CodeNoesis Delivery Roadmap

> Status: **Proposed planning companion — not implementation authority**.
> Last updated: **2026-08-08**.

This roadmap sequences product and validation work without changing the
normative meaning or approval status of the
[Software Requirements Specification](software-requirements-specification.md)
or an accepted architecture decision. The SRS and decisions remain
authoritative. Every item below requires its own Ready issue, stable
requirement IDs, one delivery slice, acceptance oracle, expected Red failure,
risk classification, allowed paths, evidence, and human approvals. Production
implementation requires either Approved requirements or the branch-scoped
implementation authority defined by the single-PR vertical package below.

Roadmap identifiers such as `R1`, `P1`, and `G1` are planning identifiers, not
SRS slice or requirement IDs. They must not be used to bypass the approved
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
- the explicit R5 profile extends the accepted Rust ontology lineage through
  fields, variants, constants/statics, associated types, method contexts, and
  attribute-preserving uncertainty while remaining not Verified;
- Cargo feature worlds, macro expansion, compiler-grade resolution, framework
  semantics and general canonical graph traversal remain unsupported or
  explicit coverage gaps; R8 export/explorer governance is defined, but its
  product implementation is not yet present.

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

For immutable guard traceability, the R5 pre-merge roadmap said
"R0-R4 are implemented" and used the sequence "R5 → R6 → R7 → R8". The R6
pre-merge roadmap then said "R0-R5 are implemented". Its exact status marker
was "R6 governance is Proposed", and it retained "R5 → R6 → R7 → R8". These
quoted historical markers do not describe the current planning state. The R7
pre-merge roadmap said "R0-R6 are implemented" and used the exact marker
"R7 static import governance is Proposed"; those markers are also retained
only as history.

R0-R8 are implemented, but remain not Verified until their complete retained
evidence is independently accepted. R7 static import still grants no index
generation authority. K1 is the next Proposed branch-scoped candidate: it adds
source-callable/value/body semantics and V2 visualization without pretending
to provide compiler data flow or runtime truth. The bounded delivery order is
K1 → R9; protected review and manual merge remain authoritative.

| Order | Planning item | Outcome | Governance dependency | Candidate acceptance gate |
|---|---|---|---|---|
| `R0` | Reproducible public corpus baseline | Record pinned public revisions, licenses, repository statistics, structural capabilities, current CodeNoesis failure sequences, and minimal synthetic shape fixtures. | Corpus, fixture, oracle, and licensing review. | Machine-readable descriptors reproduce each generic failure and capability without vendoring an external repository; corpus entries remain replaceable. |
| `R1` | Read-only packed Git acquisition | Read normal packed SHA-1 object databases directly and safely without invoking Git. | Resolve the packed-object subset of `OD-GIT-001`; approve acquisition errors, bounds, corruption behavior, and security oracle. | Packed and equivalent loose fixtures produce byte-identical semantic input; malformed indexes, packs, deltas, traversal attempts, alternates, and limit-plus-one cases fail with typed errors; the scan launches no process and opens no network channel. |
| `R2` | Safe gitlink and submodule boundary | Represent committed gitlinks and declared submodule metadata as external repository boundaries without fetching or traversing them implicitly. An explicitly supplied nested repository remains a separately acquired, revision-bound project. | Resolve the gitlink/submodule subset of `OD-GIT-001` and its later federation relationship; approve missing, malformed, mismatched, recursive, and limit behavior. | Root analysis remains deterministic with an absent submodule; a supplied nested repository must match the committed gitlink SHA; `.gitmodules` never grants network or filesystem authority; malformed or escaping declarations fail with typed evidence. |
| `R3` | Real Cargo root-package workspace | Accept virtual and non-virtual root manifests, including implicit root members, an explicit `"."` member, literal members/exclusions, conventional and explicit library/binary targets, and multiple member manifests. A gitlink member remains an external workspace boundary rather than an implicitly traversed crate. | Versioned extraction/profile decision under `FR-EXT-*` and the unresolved post-S4 ontology boundary. | Project-owned fixtures cover virtual roots, implicit and explicit root packages, exclusions, and external gitlink members while reaching the existing S4 graph/docs/query journey deterministically; Cargo, `rustc`, build scripts, proc macros, dependencies, and target code remain unexecuted. |
| `R4` | Manifest facts and feature coverage | Represent package metadata, target declarations, registry/path/Git dependencies, target-specific dependency tables, feature declarations, optional dependencies, `required-features`, patch declarations, and build-script presence without claiming an active Cargo resolution. | Approve entity/property identities, claim states, compatibility, limits, and ontology version. | Every supported manifest fact resolves to bytes; ignored or unsupported fields are explicit diagnostics or coverage gaps; no dependency is fetched, no patch is applied, and no feature or target world is guessed. |
| `R5` | Rust semantic depth at real-world scale | Implemented but not Verified: the explicit V8 declaration-only profile adds fields, variants, constants/statics, associated types, inherent methods, trait-context method identities, and attribute-preserving uncertainty states. | Decision 0015, protected governance/correction/product merges #112/#115/#116; complete immutable verification remains open. | Reviewed generic fixtures and sampled facts from multiple corpus entries cover every new entity/relation, malformed syntax, stable IDs, graph invariants, evidence resolution, selector-absent compatibility, no execution, and deterministic replay. |
| `R6` | Honest framework and macro handling | Implemented but not Verified: the explicit V9 source profile adds framework-neutral component, service, configuration, endpoint, route, and handler declarations for closed forms while preserving unresolved `cfg`, attribute-macro, declarative-macro, and proc-macro meaning as candidates and gaps. | Decision 0016 plus protected governance/evidence-ID/product merges #118/#121/#122; complete immutable verification remains open. | The project-owned two-style fixture finds reviewed declarations and unresolved candidates, rejects comments/strings/docs/imports/names/unused/generated/target decoys, preserves exact evidence and identities, never expands code, and never labels syntax as observed runtime behavior. |
| `R7` | Optional compiler-grade enrichment | Implemented but not Verified: one explicitly supplied Rust SCIP v0.9.0 artifact is accepted only after exact repository/revision/tree/source/producer/toolchain binding and bounded canonical wire validation; compiler symbols plus `RESOLVES_TO`, `REFERENCES`, `IMPLEMENTS`, and `TYPE_DEFINITION` add no calls or generated source. | Decision 0017, protected governance/correction/product merges through #130, and bounded `FR-EXT-005`; complete immutable verification remains open. Static import grants no build authority, and generation remains S9 work behind a separately approved trust/sandbox profile. | The project-owned binary fixture imports deterministically with dual source/index evidence, explicit external/generated states and call/macro gaps; stale, malformed, mismatched, ambiguous, over-limit, unsafe, or privacy-leaking inputs fail before publication. Standard local scans still execute nothing. |
| `R8` | Portable graph export and local explorer | Implemented but not Verified: export exact R7 families as canonical PortableGraphV1 and generate a read-only first-party LocalExplorerV1 with exact-ID/text search, typed filters, evidence inspection, and deterministic depth-1/2 neighborhoods. | Decision 0018 plus protected governance/product/correction merges through #139; complete immutable verification remains open. The V10 snapshot stays canonical and R8 adds no ontology fact. | Lossless reimport validates every identity/reference/evidence/family digest; 50 permutations are byte-identical; CSP/XSS/privacy/path/symlink and maximum-plus-one cases fail closed; the static viewer remains useful without the source repository and cannot mutate the graph. |
| `K1` | Deterministic Rust callable and value semantics | Proposed issue #142 single-PR candidate: add complete reviewed callable signatures, ordered parameters, normalized closed scalar values, expression-only states, local bindings, call sites, unique-local `CALLS`, syntactic control structure, exact evidence, LocalQueryResultV6, PortableGraphV2, and LocalExplorerV2. | Decision 0019, exact base `03ee09b172e84b5b7f5f423f9f65d63cf2953385`, maintainer authorization, governance checkpoint, retained Red, no new dependency, and protected manual merge. K1 v1 is source-only and does not compose with R7 SCIP. | The project-owned fixture completes scan → docs → exact-ID query → export → explore; exact counts/identities, five unresolved calls, all eleven control kinds, 50 permutations, ten schedules, privacy/limits/no-execution, and byte-identical R0-R8 regressions pass. |
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

## Production-readiness model

Production readiness is a continuous delivery constraint, not work deferred
until `S14`. Each earlier slice must preserve its applicable security,
compatibility, operability, evidence, and rollback properties even when the
complete server and release gates are not yet available.

CodeNoesis has two distinct candidate general-availability gates:

| Gate | User-visible outcome | Required scope | Candidate exit condition |
|---|---|---|---|
| Local GA | A supported, signed `noesis` distribution analyzes declared local repository profiles with deterministic evidence, bounded resource use, documented platform guarantees, recoverable local state, and no target execution or analysis network access. | Verified applicable `S0`–`S4` behavior, the advertised subset of `R0`–`R9` and `P1`–`P5`, plus applicable `G0`–`G9` controls. | Installation, upgrade, rollback, compatibility, backup/export, security, performance, support, and pilot evidence are Green for every advertised local capability and platform. |
| Server GA | A supported multi-user deployment provides durable jobs, REST/MCP parity, tenant isolation, governed intelligence, recovery, observability, and signed release operations. | Local GA plus Verified applicable `S5`–`S14` behavior and all server-applicable `G0`–`G9` controls. | SLO, tenant, privacy, migration, restore, chaos, supply-chain, incident, and multi-repository pilot gates pass on the exact release artifacts. |

An optional adapter, compiler index, framework capability, interface, or
deployment profile does not block a smaller GA scope when it is explicitly
excluded from the support matrix. Anything advertised as supported must pass
the same applicable readiness gates; experimental availability is not GA.

## Production-readiness lane

The SRS already defines many production requirements, especially across
`S9`–`S14`, but the following planning lane makes their delivery order and
evidence explicit. A row that identifies a missing requirement or decision
may update the SRS or architecture as product governance in the same authorized
single-PR vertical package as its implementation.

| Order | Planning item | Outcome | Existing SRS mapping | Candidate acceptance gate |
|---|---|---|---|---|
| `G0` | Release profiles and support matrix | Define Local GA and Server GA capabilities, supported operating systems/architectures, sandbox tiers, deployment profiles, excluded experimental features, owners, and support windows. | `NFR-PORT-001`, `OD-SBX-001`, `S9`, `S14` | Every release artifact and document names one profile and exact capability set; unsupported combinations fail before work starts. |
| `G1` | Distribution and configuration | Produce installable CLI and server artifacts with versioned configuration schemas, deterministic defaults, secret references, startup validation, installation, upgrade, uninstall, and rollback procedures. | `NFR-CMP-001`, `NFR-SEC-003`, `NFR-SUP-001`, `S10`, `S14`; installation and release-channel semantics require an approved requirement. | Clean install, invalid configuration, secret rotation, upgrade, rollback, and uninstall scenarios pass on every supported profile without hidden state or secret leakage. |
| `G2` | Compatibility and migration | Classify JSON, configuration, ontology, artifact, database, REST, MCP, WIT, and plugin changes; support the declared read/write window, explicit migrations, downgrade refusal, and rollback or rebuild paths. | `NFR-CMP-001`, `OD-STO-001`, `OD-API-001`, `S10`, `S11`, `S14` | Compatibility fixtures block unapproved breaks; migration from at least two releases and rollback/rebuild drills preserve or explicitly reject state without partial publication. |
| `G3` | Data lifecycle and disaster recovery | Define classification, retention, export, deletion, legal hold, residency, backup expiry, consistency, RPO, RTO, restore, and search/index reconstruction. | `NFR-DR-001`, `NFR-PRV-002`, `OD-DAT-001`, `S10`, `S12`, `S14` | Lifecycle conformance and complete restore exercises validate snapshot heads, objects, indexes, audit continuity, purge deadlines, and backup expiry. |
| `G4` | Runtime resilience and flow control | Bound queues and concurrency; implement idempotency, leases, retries, cancellation, backpressure, graceful drain, dependency-loss behavior, failover, and overload shedding. | `INV-BND-001`, `INV-STO-001`, `FR-JOB-001`, `NFR-REL-002`, `NFR-OPS-001`, `S10`, `S14` | Duplicate, crash, timeout, saturation, restart, dependency outage, and drain scenarios preserve the last valid state and recover within approved limits. |
| `G5` | Security, privacy, and tenancy | Maintain threat models, least privilege, secret isolation, authorization, tenant separation, sandboxing, network policy, rate limits, abuse controls, audit integrity, vulnerability exceptions, and privacy allowlists. | `INV-TEN-001`, `NFR-SEC-*`, `NFR-PRV-*`, `OD-AUT-001`, `OD-SBX-001`, `S9`, `S12`–`S14` | Malicious repository, cross-tenant, credential-canary, privilege, sandbox escape, denial-of-service, audit tamper, and external-transfer suites remain Green. |
| `G6` | Observability and operations | Define privacy-safe logs, metrics, traces, correlation, SLIs, SLOs, error budgets, health transitions, alerts, dashboards, runbooks, on-call ownership, and incident exercises. | `NFR-OBS-001`, `NFR-OPS-001`, `OD-SLO-001`, `S10`–`S14`; incident-response service levels require an approved policy or requirement. | Success, failure, retry, cancellation, dependency loss, stuck work, and incident drills produce actionable signals without source, secret, or tenant leakage. |
| `G7` | Performance and capacity | Publish reference corpora, cold/warm definitions, concurrency, cache state, ceilings, p50/p95/p99 methods, capacity models, load, soak, stress, and chaos evidence. | `NFR-PER-001/002`, `OD-LIM-001`, `OD-SLO-001`, `S14` | Exact release artifacts meet ratified latency, throughput, availability, resource, recovery, and success-rate thresholds with no discarded failures. |
| `G8` | Supply chain and release integrity | Lock dependencies, inventory transitive `unsafe`, enforce license/advisory policy, generate SBOMs, sign binaries and images, attach provenance, verify reproducibility where claimed, and time-bound exceptions. | `NFR-MNT-002`, `NFR-SEC-004`, `NFR-SUP-001`, `S14` | Consumers verify source-to-artifact identity, signatures, SBOM association, provenance, dependency policy, and absence of unaccepted exploitable Critical/High findings. |
| `G9` | Pilot, release, and support | Run staged internal/public pilots, canary and rollback exercises, operational handoff, known-limit review, support and vulnerability-response processes, deprecation/EOL policy, and final GA decision. | `S14`; support, vulnerability-response, release-channel, and EOL commitments require approved policy or requirements. | Independent reviewers approve the exact release evidence pack; pilot exit criteria, rollback, incident response, residual risk, ownership, and support commitments are explicit. |

### Production-readiness sequencing

- `G0` starts before the next public compatibility or interface contract.
- `G1`, `G2`, `G5`, and `G8` apply to Local GA rather than waiting for the
  server path.
- `G3`, `G4`, and `G6` grow with `S10` and must be exercised before Server GA.
- `G7` begins with every benchmarkable slice and becomes contractual only
  after `OD-LIM-001` and `OD-SLO-001` are approved.
- `G9` is the final release decision, not a substitute for any missing Green
  gate.

## Delivery package policy

The fastest safe unit is one review objective, not one file, requirement, or
commit. Related artifacts may be reviewed together when they establish one
coherent behavior and share the same risk boundary.

### Maintainer-supervised accelerated package

Use one single-PR vertical package for one coherent capability instead of
serial governance and implementation pull requests; one explicit human
authorization in its linked Ready issue fixes stable requirement IDs and
candidate semantics, one slice and public outcome, risk owner and rollback
boundary, exact paths and dependencies, oracle and expected Red, evidence,
correction budget, and stop conditions.

The package may combine product governance and production implementation in one
pull request. Product governance includes its SRS changes, architecture
decisions, threat model, schemas or ontology contracts, fixtures or oracles,
limits, failure behavior, traceability, and operational documentation. It may
also contain the minimum production code, focused domain, contract, and
security tests, exact reviewed dependency and lockfile changes, and Green and
regression evidence for the same outcome.

It may also combine product code and delivery control plane in one pull request.
The delivery control plane includes `AGENTS.md`, `.github/**`, and `.codex/**`,
policy and prompts, workflows and required checks, permissions, review,
publication, signing, and release authority. The Ready issue fixes every changed
control, privilege, post-merge effect, threat, and rollback path.

The package may start from requirements already Approved on `main` or from a
complete candidate that remains Proposed until merge. In the latter case, the
maintainer decision grants branch-scoped implementation authority only for the
exact package. Requirement approval and production behavior become effective
atomically only after the accountable maintainer manually merges the exact pull
request.

The exact base SHA establishes immutable base authority for the complete pull
request. Its required checks, branch protection, reviewer and merge authority,
workflow trust, permission boundaries, and signing and release restrictions
remain authoritative through manual merge. A head-authored control change is
inert as authority for that same pull request. Its output is advisory unless an
unchanged base-controlled gate independently evaluates the head tree.

No unmerged head receives privileged secrets, elevated tokens, ruleset bypass,
approval or merge authority, deployment credentials, signing keys, publication
credentials, tags, or release execution. Each declarative workflow, permission,
signing, or release-authority change activates only after manual merge and any
explicitly authorized post-merge application.

The package keeps one pull request but preserves this commit and evidence order:

1. The builder creates a governance checkpoint, before any production source
   edit, containing the complete candidate governance, exact base SHA, control
   changes, and executable acceptance or conformance check.
2. The builder runs that check against the checkpoint and records retained
   expected Red evidence bound to the checkpoint identity, command, exit,
   expected failure, log digest, and environment.
3. Subsequent commits add the minimum implementation, focused coverage,
   documentation, and Green evidence without rewriting away the checkpoint.
4. Review and manual merge accept or reject the governance and behavior
   atomically; no partial product authority reaches `main`.

Multiple tightly related requirement IDs or sub-behaviors may share one package
only when they have the same slice, public acceptance journey, risk owner,
rollback boundary, and versioned fixture or oracle. A semantic requirement,
oracle, scope, dependency, authority, or risk change invalidates the checkpoint
and requires a new explicit maintainer decision. Bounded implementation
corrections remain covered by the original authorization.

The machine-policy projection may proceed in parallel or be included in the
package, but the base-bound projection remains authoritative during review. The
merged projection remains mandatory before unattended autonomous execution; a
branch-scoped candidate is not eligible for unattended execution.

### Unattended autonomous package

Unattended autonomous execution retains the governance, machine-policy binding,
and implementation sequence. The policy-binding package remains the minimal
machine projection of merged governance and changes no requirement, oracle,
workflow, or production behavior.

When requirements are already Approved and no protected contract changes, a
low- or medium-risk behavior may use one implementation pull request containing
the complete Red-to-Green journey and evidence.

### Mandatory separation

Never bundle these merely to reduce pull-request count:

- a delivery-control change whose exact privilege, workflow, permission,
  signing, release, post-merge effect, threat, or rollback path is not explicitly
  authorized, or that is intended to judge or authorize its own head instead of
  remaining inert under immutable base authority;
- unrelated capabilities or more than one behavioral implementation
  objective;
- unrelated dependency upgrades, formatter churn, generated refreshes, or
  cleanup with a product behavior; an exact dependency named and reviewed in
  the Ready issue may accompany the objective that requires it;
- production code with a still-Proposed requirement outside the exact
  branch-authorized package, or with an oracle whose meaning has not received
  human review;
- ontology, schema, migration, authorization, sandbox, release, or secret
  changes that cross different risk owners or rollback boundaries.

Independent packages may proceed in parallel when their files, decisions, and
review evidence do not depend on one another. Stacked pull requests must name
their base and merge order; evidence from an unmerged dependency is not treated
as evidence on `main`.

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

## Implementation-aware API compatibility lane

The `S6` and `S7` outcomes are not limited to comparing two interface files.
The product must be able to keep declared API semantics, provider
implementation semantics, and client implementation assumptions as separate
evidence views, then explain a semantic change across immutable revisions.
Planning identifiers `C0`–`C5` below are not delivery slices and do not
authorize implementation.

| Order | Planning item | Generic outcome | Governance dependency | Candidate acceptance gate |
|---|---|---|---|---|
| `C0` | Canonical contract semantics | Normalize approved request/response operation and field semantics, including presence, nullability, defaults, validation, value sets, status, and error identity, without treating one dimension as another. | Contract-format capability and stable operation/field identity; OpenAPI 3.1 HTTP/JSON is the first bounded candidate, not a universal API model. | Directional positive and negative fixtures resolve every normalized fact to exact contract bytes; unknown format features remain explicit gaps. |
| `C1` | Provider implementation facts | Prove only supported validation and emission behavior from real provider source paths, preserving `guaranteed`, `may`, and `unknown` rather than extrapolating from signatures or one execution. | One approved language/framework/source capability at a time, with closed control-flow meaning and unsupported custom mapping behavior. | A direct unconditional output, a conditional output, and a custom/dynamic output produce reviewed `guaranteed_present`, `may_be_absent`, and `unknown` outcomes without build or target execution. |
| `C2` | Client implementation assumptions | Recover what an actual linked decoder, validator, request builder, and use path requires or safely handles; a DTO type alone is insufficient. | One approved client language/framework capability at a time plus exact call-site and S6 operation federation. | Strict and safe clients with superficially similar models are distinguished by their real decode/use paths; custom codecs and runtime configuration remain gaps. |
| `C3` | Three-view reconciliation | Compare declared contract, provider implementation, and client assumption facts without overwriting provenance or claim state. | `DR-SEM-001`, approved `C0`–`C2` capabilities, and deterministic S6 federation. | Contract/client mismatch, provider/contract contradiction, undocumented provider guarantee, safe client, and operation decoy match the reviewed oracle. |
| `C4` | Semantic revision diff | Compare provider revisions even when contract bytes are unchanged and classify approved implementation deltas as compatible, potentially breaking, breaking, or unresolved for each linked client path. | `FR-IMP-004/005`, versioned classifier catalog, S5 revision comparison, and `C3`. | Removing an undocumented field-presence guarantee breaks only the strict linked client; safe client remains compatible, decoy is rejected, and unsupported behavior remains unresolved. |
| `C5` | Extended behavioral evidence | Add separately governed contract formats, framework semantics, tests, traces, protocol/state behavior, side effects, and causal evidence without weakening the deterministic static baseline. | Per-capability ontology, sandbox, privacy, coverage, observation-world, and rule decisions. | Each advertised dimension has independent positive, hard-negative, decoy, insufficient-evidence, determinism, resource, and evidence-lineage gates. |

The first implementation-aware checkpoint is `C0`–`C4` for one approved
provider capability and one approved client capability. It is production-useful
only when the support matrix names the exact format, language, framework,
serializer/decoder behavior, and unsupported cases. The project-owned S7
fixture may use representative Rust and Kotlin/KMP source, but neither language
defines the generic contract and neither becomes broadly supported merely by
appearing in an oracle.

A semantic finding must expose:

- baseline and target provider revision;
- stable service, operation, field, client, and call-site identities;
- request/response direction and semantic dimension;
- separate declared and implementation before/after states;
- client assumption and deterministic federation state;
- exact classifier rule and claim state;
- source evidence and explicit coverage gaps;
- `compatible`, `potentially_breaking`, `breaking`, or `unresolved` outcome.

Static analysis cannot prove arbitrary program equivalence. Reflection, custom
codecs, generated behavior, unresolved calls, macro expansion, runtime
configuration, or incomplete federation must fail open epistemically as
`unresolved`, not fail open operationally by guessing. Tests and runtime traces
may add bounded evidence later, but a missing observation never proves absence
and an LLM never upgrades a candidate to a deterministic fact.

## Sequencing and evidence

The recommended execution order is:

1. approve and retain `R0` baseline evidence;
2. specify, Red-test, implement, and independently review `R1`;
3. specify, Red-test, implement, and independently review `R2`;
4. specify, Red-test, implement, and independently review `R3`;
5. run the first partial S4 journey on one replaceable corpus entry;
6. deliver `R5` and `R6` as separate ontology/framework objectives after the
   implemented R4 baseline;
7. retain and independently accept the implemented R7 importer evidence;
   index generation remains S9 work under a distinct sandbox decision;
8. implement the separately governed `R8` package, then execute `R9` on at
   least two independent repositories;
9. continue the polyglot lane one approved adapter at a time;
10. after S5/S6 prerequisites, deliver `C0`–`C4` as one capability and one
    behavioral implementation objective at a time, then extend through `C5`.

Each implementation pull request keeps one coherent vertical outcome under the
delivery package policy and includes the required issue, requirement status,
slice, risk, paths, base/head SHAs, expected Red, Green/regression commands,
deterministic environment, fixture, oracle, traceability, limitations, and
human approvals. No pilot or conference claim upgrades a product requirement
or marks a slice Verified by itself. The delivery package policy above reduces
administrative pull requests without weakening this vertical boundary.
