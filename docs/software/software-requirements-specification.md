# CodeNoesis Software Requirements Specification

> Status: **0.9+r9-capacity — S0 through S6 and R0-R8 plus K1 are implemented
> but not Verified; one additive K1 output-capacity profile is proposed**.
> The implemented standard profile remains capped at 32 MiB. Decision 0020
> proposes one explicit non-semantic 64 MiB V11 serialization envelope needed
> for the pinned Lekton pilot without changing ontology, identities, schemas,
> hashes, extraction limits, or any selector-absent behavior.

## 1. Document control

| Field | Value |
|---|---|
| Scope | CodeNoesis software track, from the first local slice through version `1.0` |
| Version | `0.9` |
| Status | S0 through S6 and roadmap R0-R8 plus K1 are Approved and Implemented but not Verified. The bounded R9 output-capacity amendments are Proposed and become Approved and Implemented only after the accountable maintainer manually merges the exact protected pull request for issue #148. |
| Date | 2026-08-08 |
| Product owner | Andrea Moretti — explicitly a project governance persona represented by the accountable GitHub actor [`@smutti`](https://github.com/smutti), not a separate natural person |
| Technical approver | [`@smutti`](https://github.com/smutti) — sole human maintainer under the documented single-maintainer bootstrap model |
| Normative architecture | [Software architecture](architecture.md) after its decisions are ratified |
| Delivery method | Incremental outside-in test-driven development |

### 1.1 Change history

| Version | Date | Change |
|---|---|---|
| `0.1` | 2026-07-17 | Initial proposed requirements, TDD policy, vertical slices, gates, and open decisions. |
| `0.2` | 2026-07-18 | Ratified the exact S0 approval set under single-maintainer governance, adopted the repository-wide Apache-2.0 license, split atomic scan-CLI and execution-isolation requirements, and bound the S0 contract and Red oracle. |
| `0.3` | 2026-07-23 | Recorded the implemented-but-not-Verified S0 state and ratified the exact S1 safe-inventory requirement set, explicit compatibility profile, limits, evidence model, malicious fixture, filesystem-security oracle, and expected Red. |
| `0.4` | 2026-07-24 | Recorded the implemented-but-not-Verified S1 state and proposed the exact S2 Rust ontology, stable identities, extraction and graph contracts, claim states and deterministic rule, malformed/Unicode semantics, reviewed fixture, and expected Red. |
| `0.5` | 2026-07-26 | Recorded the implemented-but-not-Verified S2 state and proposed the exact S3 snapshot/artifact identities, SQLite/CAS contract, atomic head transition, crash/retry/corruption/cleanup semantics, reviewed fixture, and expected Red. |
| `0.6` | 2026-07-27 | Recorded the Approved S3 contract and proposed the exact S4 literal Rust-workspace profile, ontology v2 identities, V4 graph/snapshot contracts, evidence-backed Markdown bundle, exact-ID query, output-root safety, reviewed fixture, and expected Red. |
| `0.7` | 2026-07-27 | Corrected S4 snapshot, graph, and extraction semantic hashes to cover their complete RFC 8785 payloads; added the reviewed full fixture semantic payload and regenerated every transitive snapshot/docs/query binding. |
| `0.8` | 2026-07-28 | Recorded the protected S4 semantic-hash amendment, policy rebind, and production implementation merges from PRs #49, #51, and #52. S0 through S4 are Implemented but remain unverified pending complete immutable retention evidence; no approved behavior or oracle changed. |
| `0.9` | 2026-07-29 | Proposed `FR-ACQ-004` and Decision 0006 for explicit read-only local pack v2/index v2 SHA-1 acquisition; fixed the ErrorV6, limits, security/race oracle, synthetic fixture, and replaceable Lekton/RustDesk corpus descriptor while preserving every accepted S0–S4 invocation. |
| `0.9+s6` | 2026-07-30 | Amended the active `0.9` baseline to record the implemented S5 state and propose Decision 0009 for bounded output-only OpenAPI 3.1.0 HTTP/JSON federation; fixed workspace/client/report/ErrorV8 contracts, source-neutral identities, authority states, restricted YAML, limits, security and determinism oracles, hostile fixtures, downstream S7 identity conformance, and the reviewed `yaml-rust2` implementation candidate. The document-control version remains `0.9` until historical guards are version-aware. |
| `0.9+s6.1` | 2026-07-31 | Recorded protected PR #81 approval and corrected the S6 implementation contract through issue #83 and PR #84: zero-client workspaces, typed unsupported-OpenAPI gaps, reproducible provider evidence and source-neutral gap identities, exact Unicode heuristic selection, and public-command versus component-counter boundary observations. |
| `0.9+r2` | 2026-08-02 | Proposed `FR-ACQ-005` and Decision 0010 for the explicit `local-gitlinks-v1` boundary profile; fixed RepositorySnapshotV5, ErrorV9, strict committed `.gitmodules` metadata, digest-only URL projection, explicit depth-one nested binding, limits, synthetic fixture, exact Red, R0/R1 regressions, and the non-vendored RustDesk R0-R2 checkpoint. |
| `0.9+r3` | 2026-08-02 | Retained document-control version `0.9` for historical guard compatibility, recorded protected R2 implementation merge #95, and proposed `FR-EXT-008` plus Decision 0011 for the explicit `cargo-root-package-v1` workspace profile; fixed RepositorySnapshotV6, ErrorV10, Rust ontology v3, standalone/virtual/non-virtual root membership, literal exclusions, bounded target roots, R4-deferred coverage, R2 gitlink composition, exact Red, generic fixtures, and non-vendored Lekton/RustDesk R3 pilots. |
| `0.9+r4` | 2026-08-03 | Retained document-control version `0.9`, recorded protected R3 implementation merge #99, and proposed `FR-EXT-009` plus Decision 0012 for explicit declaration-only Cargo manifest facts; fixed RepositorySnapshotV7, ErrorV11, Rust/Cargo ontology v4, evidence-backed metadata/target/dependency/feature/patch/build facts, digest-only locators, typed unsupported coverage, exact Red, generic fixtures, and non-vendored Lekton/RustDesk R4 pilots. |
| `0.9+r4.1` | 2026-08-03 | Recorded protected R4 governance merge #103 and proposed Decision 0013 for additive V7-only `LocalQueryResultV2`, stable Cargo diagnostic identities, exact relationship/claim/evidence/diagnostic/coverage/document query examples after restart, and byte-identical V1 behavior for V4-V6 heads. |
| `0.9+r4.2` | 2026-08-04 | Recorded protected exact-ID governance merge #106 and proposed Decision 0014 after the pinned RustDesk pilot exposed legacy top-level `[badges]`; fixed one generic typed-unsupported mapping to `cargo.legacy_badges_unsupported` without interpreting badge values or weakening unknown-key fail-closed behavior. |
| `0.9+r5` | 2026-08-04 | Recorded R4 product merge #109 and proposed `FR-EXT-010` plus Decision 0015 for the explicit `rust-semantic-depth-v1` profile; fixed RepositorySnapshotV8, ExtractionChunkV5, KnowledgeGraphV5, Rust ontology v5, ErrorV12, LocalQueryResultV3 dispatch, declaration-level fields/variants/constants/statics/associated types/method contexts, attribute-preserving uncertainty, member identity, strict limits, project-owned fixture, exact governance Red, and non-vendored Lekton/RustDesk pilot descriptors while deferring framework meaning to R6 and compiler evidence to R7. |
| `0.9+r6` | 2026-08-04 | Recorded protected R5 governance, golden-correction, and product merges #112, #115, and #116; proposed `FR-EXT-011` plus Decision 0016 for explicit framework-neutral source declarations and unresolved attribute/macro candidates; fixed RepositorySnapshotV9, ExtractionChunkV6, KnowledgeGraphV6, Rust ontology v6, ErrorV13, LocalQueryResultV4 dispatch, disjoint identities, strict source/runtime epistemic boundaries, limits, project-owned two-style fixture, exact governance Red, and non-vendored motivation-only Lekton/RustDesk observations while deferring compiler evidence to R7 and export/explorer work to R8. |
| `0.9+r7` | 2026-08-05 | Recorded protected R6 governance, evidence-ID correction, and product merges #118, #121, and #122; proposed bounded `FR-EXT-005` plus Decision 0017 for explicit static import of one revision-, tree-, source-, producer-, toolchain-, and schema-bound Rust SCIP v0.9.0 artifact; fixed RepositorySnapshotV10, ExtractionChunkV7, KnowledgeGraphV7, Rust ontology v7, ErrorV14, LocalQueryResultV5 dispatch, compiler-symbol identity and SHA-256 evidence, strict protobuf/resource/privacy limits, project-owned binary fixture, exact governance Red, and immutable R6 compatibility while leaving index generation in S9 and export/explorer work in R8. |
| `0.9+r8` | 2026-08-06 | Recorded protected R7 governance/correction and product merge #130; proposed `FR-EXP-001` plus Decision 0018 for deterministic PortableGraphV1 export and a first-party static LocalExplorerV1; fixed exact R7 family reuse, lossless reimport, explicit export/explore CLI journeys, ErrorV15, offline search/filter/depth-1/2 traversal, CSP/XSS/privacy/path/resource limits, project-owned golden, retained governance Red, and immutable R7 compatibility while leaving production implementation to a separate Ready issue. |
| `0.9+delivery.2` | 2026-08-08 | Defined the issue #139 maintainer-supervised single-PR vertical package so one coherent capability may atomically combine product governance, Red-first evidence, implementation, and Green evidence while the delivery control plane remains separate. |
| `0.9+delivery.3` | 2026-08-08 | Recorded the explicit issue #139 scope expansion permitting product code and delivery control plane in one pull request under immutable base authority, base-controlled validation, inert head controls, manual merge, and post-merge-only privileged activation. |
| `0.9+k1` | 2026-08-08 | Proposed the issue #142 single-PR K1 candidate: `FR-EXT-012`, `FR-EXP-002`, and bounded S4 amendments for complete Rust callable signatures, ordered parameters, declared scalar/value states, local bindings, call candidates, unique-local `CALLS`, syntactic control structure, RepositorySnapshotV11, KnowledgeGraphV8, LocalQueryResultV6, PortableGraphV2, LocalExplorerV2, ErrorV16, exact evidence, explicit uncertainty, and immutable R0-R8 compatibility. |
| `0.9+r9-capacity` | 2026-08-08 | Recorded K1 product merge #143 and correction merge #146, then proposed issue #148 and Decision 0020 for an explicit `local-snapshot-64m-v1` K1 output-capacity selector. The selector raises only final canonical RepositorySnapshotV11 serialization from 32 MiB to 64 MiB, preserves all semantic/configuration bytes and standard behavior, and enables retained deterministic Lekton evidence without changing any ontology or historical contract. |

This document is the normative statement of **what** the software must do and
how conformance will be demonstrated. The architecture describes **how** the
system is intended to satisfy it. The [research track](../research/README.md)
may propose experiments, but a research result is not a product requirement
until this specification and an architecture decision explicitly adopt it.

If a requirement conflicts with an architecture choice, the conflict must be
resolved before implementation. Code must not silently decide the product
semantics.

## 2. Normative language and lifecycle

The key words **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are
to be interpreted as described by
[BCP 14](https://www.rfc-editor.org/info/bcp14),
[RFC 2119](https://www.rfc-editor.org/rfc/rfc2119), and
[RFC 8174](https://www.rfc-editor.org/rfc/rfc8174) when they appear in uppercase.

### 2.1 Requirement lifecycle

```text
Proposed -> Approved -> Implemented -> Verified
    |           |             |
    +--------> Deferred <------+----> Rejected
```

- **Proposed:** reviewable but not yet binding on `main`; it may receive only
  the branch-scoped implementation authority defined in section 2.1.1.
- **Approved:** accepted, testable, and assigned to a delivery slice.
- **Implemented:** production code exists, but all verification may not be
  complete.
- **Verified:** all linked acceptance and release evidence is green.
- **Deferred:** intentionally moved out of its target release with rationale.
- **Rejected:** retained for traceability but no longer part of the product.

Requirements remain **Proposed** unless an explicit register marks them
**Approved**. Approval requires accountable ownership, resolved blocking
decisions, and an accepted test oracle. In the current single-maintainer
bootstrap, the same disclosed GitHub actor may hold product and technical
accountability; this does not create a fictional separation of duties.

For autonomous delivery, `.github/codex/policy.json` is the machine-enforced
projection of those human approvals, not an independent source of approval. A
record may be added only after the requirement is Approved here and must cite a
`source_sha` whose SRS blob is byte-identical to the current SRS; the workflow
checks that binding before any model call and again before publication. Any SRS
change invalidates the binding until affected approvals are explicitly
re-ratified.

### 2.1.1 Maintainer-supervised accelerated delivery

A linked Ready issue MAY select a maintainer-supervised single-PR vertical
package through one explicit human authorization by the accountable maintainer.
That authorization MUST fix one delivery slice, one coherent vertical outcome,
exact stable requirement IDs and candidate semantics, risk owner and rollback
boundary, allowed and protected paths, exact dependencies, acceptance oracle
and expected Red, evidence, a bounded correction budget, and stop conditions.

The package MAY combine product governance and production implementation in one
pull request. Product governance includes the SRS, architecture decisions,
threat models, schemas or ontology contracts, fixtures or oracles,
traceability, and operational documentation. Multiple tightly related IDs or
sub-behaviors MAY share it only when they have the same public acceptance
journey, delivery slice, risk owner, rollback boundary, and versioned fixture or
oracle. An exact dependency and lockfile update MAY accompany only the behavior
that the issue names and reviews.

The package MAY also combine product code and delivery control plane in one
pull request. The delivery control plane includes `AGENTS.md`, `.github/**`, and
`.codex/**`, policy and prompts, workflows and required checks, permissions,
review, publication, signing, and release authority. The Ready issue MUST fix
the exact base SHA, complete changed paths, control and privilege effects,
post-merge activation, threats, rollback, and evidence.

The package MAY start from requirements already `Approved` on `main` or from a
complete candidate that remains `Proposed` until merge. For the latter, the
maintainer decision grants branch-scoped implementation authority only for the
exact package. Requirement approval and production behavior become effective
atomically only after the accountable maintainer manually merges the exact pull
request; before merge they MUST be described as Proposed and candidate, not as
an Approved, Implemented, or Verified fact on `main`.

The author MUST, before any production source edit, create a governance
checkpoint containing the complete candidate requirement and decisions, exact
contracts and oracle, traceability, and the executable acceptance or
conformance check. The check MUST run against that checkpoint and produce
retained expected Red evidence bound to its identity, command, exit status,
failure reason, log digest, and environment. Only subsequent commits may add
production code and Green evidence, and the pull request MUST preserve the
checkpoint and Red-before-code history for independent review.

Within unchanged scope and risk, that one explicit human authorization permits
the complete sequence, focused validation, documentation and evidence updates,
pull request publication, and bounded correction without repeated
authorization. A semantic requirement, oracle, scope, dependency, authority,
or risk change after the checkpoint invalidates its authority and Red evidence
and requires a new explicit maintainer decision. The default is three
correction rounds; the issue MAY set a value from one through five.

The exact base SHA establishes immutable base authority for the complete pull
request. Its required checks, branch protection, reviewer and merge authority,
workflow trust, permission boundaries, and signing and release restrictions
remain authoritative through manual merge. A head-authored control change is
inert as authority for that same pull request. Its result is advisory unless an
unchanged base-controlled gate independently evaluates the head tree.

No unmerged head receives privileged secrets, elevated tokens, ruleset bypass,
approval or merge authority, deployment credentials, signing keys, publication
credentials, tags, or release execution. Each declarative workflow, permission,
signing, or release-authority change activates only after manual merge and any
explicitly authorized post-merge application. Head controls cannot weaken the
oracle, remove manual merge, or approve, merge, publish, sign, or release their
own change.

This supervised interactive lane may proceed without waiting for a new
machine-policy projection. The base-bound `.github/codex/policy.json` remains
authoritative for the pull request, and the merged projection remains mandatory
before unattended autonomous execution. The lane never grants direct push,
self-approval, self-merge, secret handling, destructive-data, unmerged release,
or unattended authority.

The original accelerated control-plane amendment was authorized by
[#79](https://github.com/smutti/codenoesis/issues/79). The single-PR amendment
is authorized by [#139](https://github.com/smutti/codenoesis/issues/139). Each
becomes effective only after its protected manual merge by `@smutti`; the
authoring agent does not approve or merge either change.

### 2.2 S0 ratification register

The following is the exact Approved set for **S0 — Walking skeleton**. Andrea
Moretti is a transparent governance persona represented by `@smutti`; `@smutti`
is also the sole human maintainer and technical approver. The repository does
not claim independent human review that does not exist. Approval becomes
authoritative only when `@smutti` manually squash-merges the exact final head of
PR [#8](https://github.com/smutti/codenoesis/pull/8) after every mandatory gate
is green. Authorship alone is not approval: `@smutti`'s later manual protected
merge is the approval event. A model, authoring agent, or policy file cannot
perform that merge or independently grant approval.

| Requirement | Current state | Target state | Product owner | Technical approver | Approval reference | Slice | Ratification material |
|---|---|---|---|---|---|---|---|
| `DR-ART-001` | Approved | Approved | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #8 protected merge record](https://github.com/smutti/codenoesis/pull/8) | `S0` | [S0 contract decision](decisions/0001-s0-walking-skeleton-contract.md) |
| `DR-ART-002` | Approved | Approved | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #8 protected merge record](https://github.com/smutti/codenoesis/pull/8) | `S0` | [S0 contract decision](decisions/0001-s0-walking-skeleton-contract.md) |
| `FR-ACQ-001` | Approved | Approved | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #8 protected merge record](https://github.com/smutti/codenoesis/pull/8) | `S0` | [S0 contract decision](decisions/0001-s0-walking-skeleton-contract.md) |
| `FR-CLI-003` | Approved | Approved | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #8 protected merge record](https://github.com/smutti/codenoesis/pull/8) | `S0` | [S0 acceptance specification](../../tests/specifications/s0/e2e_fr_acq_001_immutable_commit.json) |
| `NFR-DET-001` | Approved | Approved | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #8 protected merge record](https://github.com/smutti/codenoesis/pull/8) | `S0` | [S0 contract decision](decisions/0001-s0-walking-skeleton-contract.md) |
| `NFR-MNT-001` | Approved | Approved | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #8 protected merge record](https://github.com/smutti/codenoesis/pull/8) | `S0` | [S0 acceptance specification](../../tests/specifications/s0/e2e_fr_acq_001_immutable_commit.json) |
| `NFR-SEC-005` | Approved | Approved | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #8 protected merge record](https://github.com/smutti/codenoesis/pull/8) | `S0` | [S0 acceptance specification](../../tests/specifications/s0/e2e_fr_acq_001_immutable_commit.json) |
| `NFR-TST-001` | Approved | Approved | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #8 protected merge record](https://github.com/smutti/codenoesis/pull/8) | `S0` | [S0 acceptance specification](../../tests/specifications/s0/e2e_fr_acq_001_immutable_commit.json) |
| `NFR-TST-002` | Approved | Approved | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #8 protected merge record](https://github.com/smutti/codenoesis/pull/8) | `S0` | [S0 acceptance specification](../../tests/specifications/s0/e2e_fr_acq_001_immutable_commit.json) |

`FR-CLI-003` and `NFR-SEC-005` close two traceability gaps in the original
seven-ID proposal: S0 exposes `noesis scan` and its exit gate promises zero
child-process execution and zero analysis-stage network access. New atomic IDs
avoid silently changing the broader Proposed meanings of `FR-CLI-001` and
`NFR-SEC-001`. `FR-EXT-006` remains Proposed for the extraction stage, and
`NFR-PRV-001` remains Proposed for broader data egress policy; neither
substitutes for the narrower S0 guarantee. Expansion from seven to nine IDs is
itself part of the protected human ratification decision.

`NFR-MNT-001` is also made atomic for the S0 first-party architecture gate.
The previously combined transitive dependency `unsafe` exception reporting is
retained as Proposed `NFR-MNT-002`; it is not silently discarded or treated as
S0 evidence.

The approved-requirement registry in `.github/codex/policy.json` MUST remain
empty in this pull request. After ratification is merged, a separate protected
pull request may bind these exact IDs to the full commit SHA on `main` that
contains the byte-identical SRS. Until that policy PR is reviewed and merged,
autonomous authorization must fail closed.

S0 contract bundle: `sha256:978a7128498d54a6c4a6b3fec11d195e37d2f67e179d2babb5320668c4e44811`.
The bundle manifest binds the decision, strict schemas, acceptance oracle,
fixture, reviewed goldens, and maintenance guard. A change to any bound file
requires a new bundle digest in this SRS and therefore invalidates an earlier
policy `source_sha`.

The `main-production` ruleset documents the single-maintainer bootstrap by
requiring zero external approvals and no Code Owner review while there is only
one write-capable human. Compensating controls remain mandatory: pull request,
strict up-to-date CI and benchmark policy gates, CodeQL and code-quality gates,
resolved review threads, linear history, squash-only merge, no bypass actor, and
a manual merge by `@smutti`. The authoring agent must leave PR #8 unmerged. When
a second write-capable maintainer joins, the approval count and Code Owner
review MUST be restored before the next protected governance ratification.

### 2.3 Priority and target

| Priority | Meaning |
|---|---|
| `P0` | Required for the reliable local product delivered by `0.1`. |
| `P1` | Required before production-grade `1.0`. |
| `P2` | Candidate for post-`1.0`; it must not complicate the baseline prematurely. |

### 2.3.1 S1 ratification register

The following is the exact Approved set for **S1 — Safe inventory**. Approval
becomes authoritative only when `@smutti` manually squash-merges the exact
protected head of PR [#17](https://github.com/smutti/codenoesis/pull/17).
The authoring agent does not approve or merge. This high-risk decision fixes
public artifact and error schemas, source-evidence semantics, untrusted-tree
policy, filesystem confinement, and numeric limits.

| Requirement | Current state | Target state | Product owner | Technical approver | Approval reference | Slice | Ratification material |
|---|---|---|---|---|---|---|---|
| `DR-EVD-001` | Approved | Approved | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #17 protected merge record](https://github.com/smutti/codenoesis/pull/17) | `S1` | [S1 contract decision](decisions/0002-s1-safe-inventory-contract.md) |
| `FR-ACQ-002` | Approved | Approved | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #17 protected merge record](https://github.com/smutti/codenoesis/pull/17) | `S1` | [S1 contract decision](decisions/0002-s1-safe-inventory-contract.md) |
| `FR-INV-001` | Approved | Approved | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #17 protected merge record](https://github.com/smutti/codenoesis/pull/17) | `S1` | [S1 acceptance specification](../../tests/specifications/s1/e2e_fr_inv_001_safe_inventory.json) |
| `NFR-SEC-001` | Approved | Approved | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #17 protected merge record](https://github.com/smutti/codenoesis/pull/17) | `S1` | [S1 acceptance specification](../../tests/specifications/s1/e2e_fr_inv_001_safe_inventory.json) |

S1 is an explicit compatibility profile:
`--profile standard-local-s1` emits `RepositorySnapshotV2` or
`CodeNoesisErrorV2`. The approved S0 invocation without that option retains its
V1 artifact and error behavior. Repository shape, extensions, environment, and
implicit configuration cannot select a contract version.

The fixed profile resolves `OD-LIM-001` only for S1. It rejects symlinks,
gitlinks, external Git directories, and packed object databases; the explicit
packed-object deferral made after S0 remains binding. Semantic parsing and
ontology construction begin in later slices.

The policy registry is intentionally unchanged in this ratification change. A
separate protected change may bind these exact four IDs to the full commit on
`main` containing the byte-identical SRS. Until that change is reviewed and
merged, autonomous S1 authorization fails closed.

S1 contract bundle: `sha256:1a0c6222699d683238e36aef0f77d40a02db469a6c394a812c0e8aa7a1398867`.
The bundle binds the decision, strict schemas, machine oracle, synthetic
fixture, reviewed goldens, inherited S0 security policy, and independent
maintenance guard. Any bound-byte change requires a new digest and renewed
human review.

### 2.4 Verification classes

| Code | Verification evidence |
|---|---|
| `UT` | Focused unit test of domain behaviour or an invariant. |
| `PT` | Property or model-based test over generated inputs and operation sequences. |
| `CT` | Reusable contract suite applied to every implementation of a port. |
| `GT` | Golden or differential test against a versioned, reviewed oracle. |
| `IT` | Integration test using real adapters in an isolated environment. |
| `E2E` | Black-box test through a supported public interface. |
| `SEC` | Security, privacy, authorization, or malicious-input test. |
| `FZ` | Fuzzing or parser robustness evidence. |
| `FT` | Failpoint, crash, retry, or chaos test. |
| `PERF` | Reproducible benchmark or load test against a versioned corpus. |
| `DR` | Migration, backup, restore, or rollback exercise. |
| `CONF` | Schema, protocol, platform, or compatibility conformance test. |

### 2.5 S2 ratification register

The following is the exact target Approved set for **S2 — Rust knowledge**.
Approval becomes authoritative only when `@smutti` manually squash-merges the
exact protected head of PR
[#23](https://github.com/smutti/codenoesis/pull/23). The authoring agent does
not approve or merge. This high-risk decision fixes public artifact and error
schemas, ontology properties and cardinalities, stable identity, claim states,
deterministic-rule provenance, parser recovery, and untrusted-source behavior.

| Requirement | Current state | Target state | Product owner | Technical approver | Approval reference | Slice | Ratification material |
|---|---|---|---|---|---|---|---|
| `DR-IDN-001` | Approved | Approved | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #23 protected merge record](https://github.com/smutti/codenoesis/pull/23) | `S2` | [S2 contract decision](decisions/0003-s2-rust-knowledge-contract.md) |
| `FR-EXT-001` | Approved | Approved | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #23 protected merge record](https://github.com/smutti/codenoesis/pull/23) | `S2` | [S2 acceptance specification](../../tests/specifications/s2/e2e_fr_ext_002_rust_knowledge.json) |
| `FR-EXT-002` | Approved | Approved | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #23 protected merge record](https://github.com/smutti/codenoesis/pull/23) | `S2` | [S2 acceptance specification](../../tests/specifications/s2/e2e_fr_ext_002_rust_knowledge.json) |
| `FR-KNW-001` | Approved | Approved | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #23 protected merge record](https://github.com/smutti/codenoesis/pull/23) | `S2` | [Rust ontology v1](../../tests/specifications/s2/rust-ontology-v1.json) |
| `FR-KNW-002` | Approved | Approved | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #23 protected merge record](https://github.com/smutti/codenoesis/pull/23) | `S2` | [Rust ontology v1](../../tests/specifications/s2/rust-ontology-v1.json) |
| `FR-KNW-003` | Approved | Approved | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #23 protected merge record](https://github.com/smutti/codenoesis/pull/23) | `S2` | [S2 contract decision](decisions/0003-s2-rust-knowledge-contract.md) |

S2 is selected only by `--profile standard-local-s2` and emits
`RepositorySnapshotV3` or `CodeNoesisErrorV3`. The approved S0 invocation
without a profile and the approved `standard-local-s1` invocation retain their
V1 and V2 behavior. Repository shape, source extension, parse success,
environment, and implicit configuration cannot select a contract version.

Ontology `codenoesis.ontology/rust/v1` contains exactly ten entity kinds and
four structural relationship kinds for the bounded fixture. It fixes required
properties, endpoint matrices, cardinalities, NFC identifier normalization,
domain-separated BLAKE3 entity/relationship/claim IDs, seven distinct claim
states, eleven allowed transitions, immutable ontology versioning, and one
versioned deterministic file-containment rule. Malformed and unsupported
syntax remains diagnostic or typed rather than inferred as truth.

S2 inherits the approved S1 repository, file, output, wall, CPU, RSS,
temporary-disk, filesystem, process, and network boundaries. `FR-EXT-006`
remains Proposed: its broader future standard-extraction policy is not silently
approved here. The narrower S2 no-execution behavior follows directly from
`FR-EXT-002` and preserved S0/S1 safety contracts.

The policy registry is intentionally unchanged in this ratification change. A
separate protected change may bind exactly these six IDs to the full commit on
`main` containing the byte-identical SRS. Until that change is reviewed and
merged, autonomous S2 authorization fails closed.

S2 contract bundle: `sha256:d105957b00335ece563ae2783543aa112916d71131cfb23a0ebcaa14d7f57c9f`.
The bundle binds the decision, strict schemas, ontology table, machine oracle,
synthetic fixture, reviewed extraction/graph/error goldens, inherited S1
bundle, and independent maintenance guard. Any bound-byte change requires a
new digest and renewed human review.

### 2.6 S3 ratification register

The following is the exact target Approved set for
**S3 — Atomic local storage**. Approval becomes authoritative only when
`@smutti` manually squash-merges the exact protected head of PR
[#29](https://github.com/smutti/codenoesis/pull/29). The authoring agent does
not approve or merge. This high-risk decision fixes local snapshot and artifact
identity, fresh SQLite schema, filesystem-CAS layout and durability protocol,
atomic visible-head publication, failpoint outcomes, retry, corruption,
cleanup, path safety, public storage errors, and platform evidence claims.

| Requirement | Current state | Target state | Product owner | Technical approver | Approval reference | Slice | Ratification material |
|---|---|---|---|---|---|---|---|
| `FR-SNP-001` | Proposed | Approved | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #29 protected merge record](https://github.com/smutti/codenoesis/pull/29) | `S3` | [S3 contract decision](decisions/0004-s3-atomic-local-storage-contract.md) |
| `FR-STO-001` | Proposed | Approved | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #29 protected merge record](https://github.com/smutti/codenoesis/pull/29) | `S3` | [S3 acceptance specification](../../tests/specifications/s3/e2e_fr_sto_001_atomic_local_storage.json) |
| `INV-SNP-001` | Proposed | Approved | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #29 protected merge record](https://github.com/smutti/codenoesis/pull/29) | `S3` | [S3 failpoint matrix](../../tests/specifications/s3/publication-failpoints-v1.json) |
| `NFR-REL-001` | Proposed | Approved | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #29 protected merge record](https://github.com/smutti/codenoesis/pull/29) | `S3` | [S3 failpoint matrix](../../tests/specifications/s3/publication-failpoints-v1.json) |

S3 is selected only by `--profile standard-local-s3` with one explicit
`--store` root. Successful stdout remains the approved
`RepositorySnapshotV3`; storage location and operational publication state do
not enter its semantic hash. S3 uses strict `CodeNoesisErrorV4` for its new
input, store, integrity, and publication failures. S0, S1, and S2 invocations
remain byte-compatible and perform no store write.

The local project key is canonical repository identity. Snapshot IDs derive
from the V3 semantic hash. Exact RFC 8785 snapshot-semantic, graph, and
extraction bytes receive domain-separated BLAKE3 artifact IDs and are staged in
the filesystem CAS before SQLite references them.

SQLite schema `codenoesis.local-store/v1` uses WAL, `synchronous=FULL`, foreign
keys, trusted schema off, and one immediate writer. Snapshot, artifact, graph,
claim, evidence, extraction, diagnostic, and coverage rows are immutable.
`project_heads` is the only mutable table. One SQLite commit is the visibility
point: readers observe the previous complete head or the new complete head,
never an uncommitted or cross-snapshot mixture.

The exact eight-boundary failpoint matrix covers every canonical CAS artifact
occurrence plus each metadata transition for first publication and A-to-B
replacement, external process termination, restart, complete head validation,
retry, and orphan sweep. Pre-commit termination retains no head or A;
post-commit termination exposes A or B. Duplicate publication is idempotent,
reachable corruption fails closed without fallback, and cleanup never deletes
an object referenced by a committed snapshot.

Process-crash/restart behavior is required on Linux, macOS, and Windows.
Power-loss durability is claimed only when evidence names successful SQLite,
file, atomic-move, and parent-directory durability primitives for the tested
platform and filesystem. Unsupported durability fails before publication.

The policy registry is intentionally unchanged in this ratification change. A
separate protected change may bind exactly these four IDs to the full commit on
`main` containing the byte-identical SRS. Until that change is reviewed and
merged, autonomous S3 implementation authorization fails closed.

S3 contract bundle: `sha256:6269996ebfa117f30bf5b7e8eee56fce5806acdd342ad9d7dfea109c25725605`.
The bundle binds decision 0004, the independent maintenance guard, inherited
S2 bundle, strict DDL and schemas, machine oracle, two-revision fixture,
reviewed semantic/head/error/recovery goldens, and failpoint matrix. Any bound
byte change requires a new digest and renewed human review.

### 2.7 S4 ratification register

The following is the exact target Approved set for
**S4 — Evidence-backed workspace docs**. Approval becomes authoritative only
when `@smutti` manually squash-merges the exact protected head of PR
[#42](https://github.com/smutti/codenoesis/pull/42). The authoring agent does
not approve or merge. This high-risk decision fixes a bounded literal-member
Rust workspace profile, ontology v2 multi-crate and out-of-line module
identity, V4 snapshot/graph contracts, deterministic evidence-backed Markdown,
marker-owned output publication, exact-ID local query, public errors, and
resource ceilings.

| Requirement | Current state | Target state | Product owner | Technical approver | Approval reference | Slice | Ratification material |
|---|---|---|---|---|---|---|---|
| `DR-IDN-002` | `Proposed` | `Approved` | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #42 protected merge record](https://github.com/smutti/codenoesis/pull/42) | `S4` | [S4 contract decision](decisions/0005-s4-workspace-docs-query-contract.md) |
| `FR-EXT-007` | `Proposed` | `Approved` | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #42 protected merge record](https://github.com/smutti/codenoesis/pull/42) | `S4` | [S4 acceptance specification](../../tests/specifications/s4/e2e_fr_cli_001_workspace_docs_query.json) |
| `FR-DOC-001` | `Proposed` | `Approved` | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #42 protected merge record](https://github.com/smutti/codenoesis/pull/42) | `S4` | [S4 docs output contract](../../tests/specifications/s4/docs-output-contract-v1.json) |
| `FR-DOC-002` | `Proposed` | `Approved` | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #42 protected merge record](https://github.com/smutti/codenoesis/pull/42) | `S4` | [S4 documentation manifest schema](../../tests/specifications/s4/documentation-manifest-v1.schema.json) |
| `FR-DOC-003` | `Proposed` | `Approved` | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #42 protected merge record](https://github.com/smutti/codenoesis/pull/42) | `S4` | [S4 docs output contract](../../tests/specifications/s4/docs-output-contract-v1.json) |
| `FR-QRY-001` | `Proposed` | `Approved` | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #42 protected merge record](https://github.com/smutti/codenoesis/pull/42) | `S4` | [S4 query result schema](../../tests/specifications/s4/local-query-result-v1.schema.json) |
| `FR-CLI-001` | `Proposed` | `Approved` | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #42 protected merge record](https://github.com/smutti/codenoesis/pull/42) | `S4` | [S4 acceptance specification](../../tests/specifications/s4/e2e_fr_cli_001_workspace_docs_query.json) |

The table preserves the lifecycle transition presented for protected
ratification. Following the protected merges of PRs #49, #51, and #52, all
seven listed requirements are Approved and Implemented but not Verified.
Implementation evidence does not substitute for the complete immutable
retention evidence required for verification.

S4 scan is selected only by `--profile standard-local-s4` with one explicit
store. It emits `RepositorySnapshotV4` with `KnowledgeGraphV2` and immutable
`codenoesis.ontology/rust/v2`. The profile accepts only committed UTF-8 root
manifests with literal workspace members, bounded conventional or explicit
library/binary roots, literal path dependencies, and unambiguous inline or
out-of-line modules. Cargo, rustc, build scripts, target code, network, macros,
feature worlds, and compiler-grade resolution remain forbidden or explicit
coverage gaps.

`noesis docs` reads one validated stored S4 head and atomically publishes a
marker-owned `DocumentationManifestV1`, `overview.md`, and one module page per
resolved module beneath an explicit output root. Every material statement
resolves through the manifest to source evidence or a visible unsupported
coverage state. The generator never adopts, overwrites, follows, or deletes
unowned content.

`noesis query` performs exact stable-ID lookup only. It returns a typed
`LocalQueryResultV1` for an entity, claim, evidence item, or generated document
bound to the same snapshot. Unknown IDs fail with `query.not_found`; traversal,
fuzzy search, and inferred answers remain deferred.

S0–S3 profiles and ontology v1 remain byte-compatible. S4 reuses the approved
fresh `codenoesis.local-store/v1` publication semantics without a migration or
new artifact role. If V4 cannot be represented without changing S3 meaning,
implementation stops for a separate storage decision.

S4 snapshot, graph, and extraction-chunk semantic hashes use BLAKE3-256 over
the exact versioned domain, one `0x00` separator byte, and the complete RFC
8785 canonical payload. The graph and chunk payloads omit only their own
`semantic_hash` member; the snapshot payload is the complete semantic object.
The machine-readable hash contract and full reviewed fixture semantic payload
are bound into the S4 contract bundle. Any material nested change must change
its enclosing digest and the snapshot identity.

The content-complete amendment was rebound through protected PR #51 before the
production implementation in PR #52. This status-only revision changes the SRS
blob without changing approved S4 meaning, so the machine policy source
binding must fail closed until a separate protected rebind is reviewed and
merged. This pull request does not authorize autonomous post-S4 production
implementation.

S4 contract bundle: `sha256:3efb380fb058a5831123a0f990676575da04e60998cada8987f034675b61f12e`.
The bundle binds decision 0005, the independent maintenance guard, inherited
S3 bundle, strict schemas, ontology v2, machine oracle, two-member workspace,
reviewed documentation/query/error goldens, and source/build sentinels. Any
bound-byte change requires a new digest and renewed human review.

### 2.8 S1 packed SHA-1 ratification register

The following is the exact target Approved set for an explicit
**S1 — Safe inventory compatibility extension** implementing roadmap `R0` and
`R1`. `R0` and `R1` remain planning identifiers, not delivery slices.
Approval becomes authoritative only when `@smutti` manually squash-merges the
exact protected head of PR
[#61](https://github.com/smutti/codenoesis/pull/61). The authoring agent does
not approve or merge. This high-risk decision fixes an operational selector,
the bounded local pack v2/index v2 SHA-1 subset, ErrorV6, corruption and race
behavior, numeric limits, a project-owned fixture, and a replaceable public
corpus descriptor.

| Requirement | Current state | Target state | Product owner | Technical approver | Approval reference | Slice | Ratification material |
|---|---|---|---|---|---|---|---|
| `FR-ACQ-004` | `Proposed` (pending protected merge) | `Approved` | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #61 protected merge record](https://github.com/smutti/codenoesis/pull/61) | `S1` | [Packed SHA-1 acquisition decision](decisions/0006-s1-packed-sha1-acquisition-contract.md) and [machine oracle](../../tests/specifications/s1/e2e_fr_acq_004_packed_sha1.json) |

The existing S1 contract remains immutable:
`standard-local-s1` without a new acquisition selector still accepts only
verified loose SHA-1 objects and rejects a packed object database. Packed
acquisition is selected only by:

```text
--acquisition-profile local-git-sha1-packed-v1
```

on an otherwise valid `standard-local-s1`, `standard-local-s2`,
`standard-local-s3`, or `standard-local-s4` scan. The selector is an explicit
operational input. It is excluded from semantic configuration, hashes, and
snapshot identity because loose and packed databases are physical
representations of the same verified Git objects. Repository shape,
environment, extensions, and implicit configuration cannot select it.

The selected boundary accepts repository-local loose objects and paired pack
v2/index v2 files, including bounded `OFS_DELTA` and local `REF_DELTA`
resolution. It validates catalog pairing, index structure and checksum, pack
header/count/trailer, CRC, zlib framing, delta programs, reconstructed
type/size, collision-detecting SHA-1 identity, stable file handles, and every
fixed limit. It invokes no Git process, network, hook, filter, credential
helper, target code, alternate object database, or automatic repair.

Input and acquisition failures under the selector use strict
`CodeNoesisErrorV6`; later stage errors retain their accepted lineages. All
legacy S0–S4 commands, artifacts, errors, semantic hashes, and the existing
packed-rejection test remain byte-compatible.

The project-owned packed fixture reuses the accepted S1 source bytes without
changing its bundle. The public corpus descriptor pins Lekton and RustDesk
revisions, trees, observed license evidence, immutable tree statistics,
contextual full-clone pack observations, current typed failures, and their next
generic roadmap blockers. External source is not vendored, neither repository
defines product semantics, and either entry may be replaced by another public
repository satisfying the same capability set.

The policy registry is intentionally unchanged in this ratification change. A
separate protected change may bind only `FR-ACQ-004` to the full commit on
`main` containing the byte-identical SRS. Until that policy PR is reviewed and
merged, autonomous implementation authorization fails closed.

S1 packed SHA-1 contract bundle: `sha256:08602c21b06e0e0cea754312fb8c9f5d28db36ae31e9e646df669b2c129826df`.
The separate bundle binds Decision 0006, the independent maintenance guard,
strict ErrorV6 and corpus schemas, machine oracle, synthetic fixture,
replaceable public corpus descriptor, and the unchanged S1/S4 contract
lineage. Any bound-byte change requires a new digest and renewed human review.

### 2.9 S7 implementation-aware compatibility ratification register

The following is the exact target Approved set for the bounded
**S7 — Change impact implementation-aware compatibility contract**. Approval
becomes authoritative only when `@smutti` manually squash-merges the exact
protected head of [PR #63](https://github.com/smutti/codenoesis/pull/63). The
authoring agent does not approve or merge.
This high-risk decision fixes the three-view semantic model, stable identities,
direction-aware classifier, strict public report, limits, project-owned
provider/client/decoy fixture, explicit unknown behavior, and future Red.

| Requirement | Current state | Target state | Product owner | Technical approver | Approval reference | Slice | Ratification material |
|---|---|---|---|---|---|---|---|
| `DR-SEM-001` | `Proposed` (pending protected merge) | `Approved` | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #63 protected merge record](https://github.com/smutti/codenoesis/pull/63) | `S7` | [S7 implementation-aware compatibility decision](decisions/0007-s7-implementation-aware-api-compatibility-contract.md) and [report schema](../../tests/specifications/s7/semantic-compatibility-report-v1.schema.json) |
| `FR-IMP-004` | `Proposed` (pending protected merge) | `Approved` | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #63 protected merge record](https://github.com/smutti/codenoesis/pull/63) | `S7` | [S7 machine oracle](../../tests/specifications/s7/e2e_fr_imp_004_implementation_aware_api_diff.json) and [reviewed fixture](../../tests/fixtures/s7/implementation-aware-api-v1/README.md) |
| `FR-IMP-005` | `Proposed` (pending protected merge) | `Approved` | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #63 protected merge record](https://github.com/smutti/codenoesis/pull/63) | `S7` | [S7 rule catalog](../../tests/specifications/s7/compatibility-rule-catalog-v1.json) and [reviewed report](../../tests/fixtures/s7/implementation-aware-api-v1/expected-semantic-compatibility-report.json) |

This additive contract does not silently approve or redefine the broader
`FR-IMP-001`, `FR-IMP-002`, or `FR-IMP-003` requirements. It partially resolves
`OD-CMP-001` only for the versioned
`implementation-aware-http-json/v1` projection and its seven declared
dimensions. Other contract formats, frameworks, dynamic behavior, observation
profiles, and causal claims remain open.

The contract compares immutable provider revisions even when their OpenAPI
bytes are identical. Declared contract, provider implementation, client
assumption, test observation, and runtime observation remain separate evidence
views. Presence, nullability, declared default, applied default, request versus
response direction, and deterministic versus candidate federation may not
substitute for one another. Missing traces, source types alone, custom mappings,
reflection, generated code, runtime configuration, or model output never
silently establish behavior.

The reviewed fixture proves an unchanged optional response field whose provider
implementation changes from guaranteed present to may be absent. A
deterministically linked strict client moves from latent
`potentially_breaking` risk to `breaking`; a safe linked client remains
`compatible`; a name-similar client for a different operation is rejected. A
custom provider mapping remains `unresolved` with an exact coverage gap.

No S7 runtime behavior is authorized by this register. The first production Red
is blocked until S5, S6, and the exact OpenAPI, provider, and client source
capability profiles are Approved and Implemented. The policy registry remains
unchanged in this ratification change; a separate protected policy PR may bind
only these three IDs to the exact merged SRS.

S7 implementation-aware compatibility contract bundle:
`sha256:08dabb5b4895adfa5f531a9364a8f02ba39a2548b05ac7b4af08e26b4d4d044a`.
The bundle binds Decision 0007, the independent governance guard,
strict report schema, rule catalog, machine oracle, project-owned source
fixture, manifest, and reviewed report. Any bound-byte change requires a new
digest and renewed human review.

### 2.10 S5 deterministic incremental refresh ratification register

The following is the exact target Approved set for the bounded
**S5 — Incremental refresh** contract. Approval becomes authoritative only when
`@smutti` manually merges the exact protected head of the pull request bound
before review. The authoring agent does not approve or merge. This high-risk
decision fixes the local refresh operation, revision-neutral analysis cache,
conservative rule catalog, commit-bound public rematerialization, strict
report/error artifacts, limits, project-owned two-revision fixture, exact
invalidation oracle, atomic publication behavior, and future Red.

| Requirement | Current state | Target state | Product owner | Technical approver | Approval reference | Slice | Ratification material |
|---|---|---|---|---|---|---|---|
| `INV-INC-001` | `Proposed` (pending protected merge) | `Approved` | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #67 protected merge record](https://github.com/smutti/codenoesis/pull/67) | `S5` | [S5 incremental refresh decision](decisions/0008-s5-incremental-refresh-contract.md) and [reviewed cold artifacts](../../tests/fixtures/s5/incremental-refresh-v1/expected-cold-artifacts.json) |
| `FR-INC-001` | `Proposed` (pending protected merge) | `Approved` | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #67 protected merge record](https://github.com/smutti/codenoesis/pull/67) | `S5` | [S5 rule catalog](../../tests/specifications/s5/incremental-rule-catalog-v1.json) and [reviewed report](../../tests/fixtures/s5/incremental-refresh-v1/expected-incremental-refresh-report.json) |
| `FR-INC-002` | `Proposed` (pending protected merge) | `Approved` | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #67 protected merge record](https://github.com/smutti/codenoesis/pull/67) | `S5` | [S5 rule catalog](../../tests/specifications/s5/incremental-rule-catalog-v1.json) and [machine oracle](../../tests/specifications/s5/e2e_fr_inc_001_incremental_refresh.json) |
| `FR-INC-003` | `Proposed` (pending protected merge) | `Approved` | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #67 protected merge record](https://github.com/smutti/codenoesis/pull/67) | `S5` | [Analysis cache schema](../../tests/specifications/s5/analysis-cache-entry-v1.schema.json) and [machine oracle](../../tests/specifications/s5/e2e_fr_inc_001_incremental_refresh.json) |
| `FR-CLI-004` | `Proposed` (pending protected merge) | `Approved` | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #67 protected merge record](https://github.com/smutti/codenoesis/pull/67) | `S5` | [Refresh report schema](../../tests/specifications/s5/incremental-refresh-report-v1.schema.json), [ErrorV7 schema](../../tests/specifications/s5/codenoesis-error-v7.schema.json), and [machine oracle](../../tests/specifications/s5/e2e_fr_inc_001_incremental_refresh.json) |

Bootstrap Red observation correction:
[PR #71 protected bootstrap correction](https://github.com/smutti/codenoesis/pull/71).
This correction records the unchanged pre-S5 command boundary
only; it does not alter final S5 success or `CodeNoesisErrorV7` behavior.

`standard-local-s5` selects a local refresh use case over one validated visible
S4 head and one independently bound immutable target commit. The target remains
the accepted `standard-local-s4` semantic profile and
`RepositorySnapshotV4`; no S4 public schema, identity, storage, documentation,
or query contract changes.

The internal `AnalysisCacheEntryV1` key binds repository identity, stable
source ownership and module mapping, canonical path, blob, cache schema,
extractors, normalization, ontology, extraction contract, semantic profile,
and dependency-rule version. It excludes commit and volatile envelope values.
Its payload contains only revision-neutral observations and source-relative
spans, with no public evidence, claim, relationship, chunk, snapshot,
document, statement, commit, or report identity. A cache entry is never
authoritative until target public artifacts are completely rematerialized,
validated, and atomically published.

Accepted S4 evidence identity includes the immutable commit. Therefore every
non-no-op target rematerializes target inventory evidence, source evidence,
all public chunks, graph, snapshot, documentation projection, and refresh
report even when source analysis is reused. An equal tree under a different
commit is not a no-op. `no_change` applies only when the exact requested commit
is already the validated visible head and every compatible version input
matches.

The versioned rule precedence is `error > full_rebuild >
full_workspace_analysis > partial_analysis > inventory_only > no_change`.
Only a modified already-mapped non-root Rust source with unchanged repository,
path, source, crate, module, and version inputs may select partial analysis.
Source add/delete, deterministic delete-plus-add rename, root/module,
manifest/target/workspace/ownership, unsupported, ambiguous, or fallback
changes select full-workspace analysis. Extractor, mapper, normalization,
ontology, extraction contract, semantic profile, dependency rule, cache
schema, snapshot schema, or public schema changes select a full rebuild.

The reviewed fixture proves two exact cache hits, one invalidated and
recomputed source analysis entry, one parser invocation, all three public
chunks rematerialized, four document manifest entries rematerialized, three
changed Markdown documents, exact public invalidation sets, and target
snapshot, graph, chunk, and document bytes equal to a reviewed clean target
result. It executes no Git process, Cargo, compiler, build script, target,
hook, plugin, network, model provider, or Council.

No S5 production behavior is authorized by this register. After protected
governance merge and a separate policy binding, the first production Red is
the exact `e2e_fr_inc_001_incremental_refresh` command in the machine oracle.
Before S5 exists, the exact `refresh` invocation is not recognized and must
exit `2` with ErrorV2 `input.invalid_revision` on stderr, empty stdout, no
created store, and no published head. This is the accepted bootstrap Red only;
the implemented S5 command retains strict `CodeNoesisErrorV7` failures.
Compilation, fixture, schema, panic, timeout, execution, fake reuse, stale
public chunk, or weakened cold-equivalence failures are rejected.

The policy registry is intentionally unchanged in this ratification change. A
separate protected policy pull request may bind only these five IDs to the
exact merged SRS. Until that binding is reviewed and merged, autonomous
implementation authorization fails closed.

S5 deterministic incremental refresh contract bundle:
`sha256:9e1725cfde7e20cd1f7c513b32f919eb7d2c3109a7a05427c515c89f360de6c8`.
The bundle binds Decision 0008, the independent maintenance guard, inherited
S4 bundle, strict cache/report/error schemas, rule catalog, machine oracle,
project-owned two-revision fixture, exact Git objects, reviewed cold summaries,
and reviewed refresh report. Any bound-byte change requires a new digest and
renewed human review.

### 2.11 S6 OpenAPI federation ratification register

Protected manual merge of
[PR #81](https://github.com/smutti/codenoesis/pull/81) made the following exact
set Approved for the bounded **S6 — Contract federation** capability. Issue
[#83](https://github.com/smutti/codenoesis/issues/83) corrects implementation
ambiguities found before production edits in issue
[#82](https://github.com/smutti/codenoesis/issues/82); its amendment becomes
effective only when `@smutti` manually merges the exact protected head of
[PR #84](https://github.com/smutti/codenoesis/pull/84). The authoring agent
does not approve or merge. This high-risk decision fixes one output-only local
CLI operation, OpenAPI 3.1.0 JSON/YAML normalization, stable cross-project
identities, explicit and heuristic authority states, strict public artifacts,
limits, a project-owned provider/client/decoy fixture, hostile inputs,
downstream S7 identity conformance, and the retained production Red.

| Requirement | Current state | Target state | Product owner | Technical approver | Approval reference | Slice | Ratification material |
|---|---|---|---|---|---|---|---|
| `FR-EXT-004` | `Approved` for `codenoesis.contract-capability/openapi-3.1-http-json/v1` only | `Implemented` | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #81 protected merge record](https://github.com/smutti/codenoesis/pull/81), corrected by [PR #84](https://github.com/smutti/codenoesis/pull/84) | `S6` | [S6 OpenAPI federation decision](decisions/0009-s6-openapi-federation-contract.md), [workspace schema](../../tests/specifications/s6/federation-workspace-v1.schema.json), and [reviewed provider fixture](../../tests/fixtures/s6/openapi-federation-v1/README.md) |
| `FR-FED-001` | `Approved` for `codenoesis.federation-rules/http-json/v1` | `Implemented` | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #81 protected merge record](https://github.com/smutti/codenoesis/pull/81), corrected by [PR #84](https://github.com/smutti/codenoesis/pull/84) | `S6` | [S6 federation rule catalog](../../tests/specifications/s6/openapi-federation-rule-catalog-v1.json) and [reviewed report](../../tests/fixtures/s6/openapi-federation-v1/expected-federation-report.json) |
| `FR-FED-002` | `Approved` for `codenoesis.federation-rules/http-json/v1` | `Implemented` | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #81 protected merge record](https://github.com/smutti/codenoesis/pull/81), corrected by [PR #84](https://github.com/smutti/codenoesis/pull/84) | `S6` | [S6 federation rule catalog](../../tests/specifications/s6/openapi-federation-rule-catalog-v1.json) and [machine oracle](../../tests/specifications/s6/e2e_fr_fed_001_openapi_federation.json) |
| `FR-CLI-005` | `Approved` for `standard-local-s6` | `Implemented` | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #81 protected merge record](https://github.com/smutti/codenoesis/pull/81), corrected by [PR #84](https://github.com/smutti/codenoesis/pull/84) | `S6` | [Federation report schema](../../tests/specifications/s6/federation-report-v1.schema.json), [ErrorV8 schema](../../tests/specifications/s6/codenoesis-error-v8.schema.json), and [machine oracle](../../tests/specifications/s6/e2e_fr_fed_001_openapi_federation.json) |

The capability-scoped approval of `FR-EXT-004` does not advertise or approve
AsyncAPI, GraphQL, Protocol Buffers, other OpenAPI versions, external
references, or broader HTTP behavior. The approved capability is exactly
`codenoesis.contract-capability/openapi-3.1-http-json/v1`, with exact OpenAPI
`3.1.0`, `application/json`, bounded local JSON Pointer references, and the
restricted single-document YAML subset in Decision 0009.

The only operation is:

```text
noesis federate \
  --workspace-manifest <path> \
  --profile standard-local-s6 \
  --format json
```

It reads only the explicit workspace manifest and its bound local roots.
Success exits `0` and writes exactly one LF-terminated canonical
`FederationReportV1` to stdout. Invalid invocation exits `2`; contract,
federation, and limit failures exit `10`; unexpected internal failure exits
`70`. Failure writes empty stdout and exactly one strict LF-terminated
`CodeNoesisErrorV8` to stderr. The complete report is buffered, validated,
size-checked, and written once. There is no `--store`, persistent mutation,
partial stdout, or S3 head transition.

The exact federation authority order is explicit workspace identity,
package/SCIP identity, canonical operation identity, event/schema identity,
then heuristic candidate. S6 exposes explicit workspace and canonical
operation authority only. Package/SCIP and event/schema authority remain
coverage gaps. A heuristic never becomes `confirmed` automatically, and
conflicting authoritative evidence fails closed.

Provider evidence uses reproducible format-specific selectors and exact
stable-ID preimages from Decision 0009. YAML binds normalized OpenAPI location
plus one-based inclusive source span; JSON and declarations bind canonical
JSON Pointers. Contract coverage-gap identities bind subject, reason, and
normalized OpenAPI location so equivalent JSON/YAML semantics remain
source-neutral.

The v1 heuristic applies exact Unicode scalar-sequence equality, without
normalization, case folding, trimming, fuzzy scoring, or model
interpretation, between client hints and OpenAPI `info.title` plus
`operationId`. One matching operation remains a candidate with
`heuristic_requires_confirmation`; zero and multiple matches produce
`heuristic_no_match` and `heuristic_ambiguous`.

The reviewed fixture proves a valid provider-only workspace, two explicit
confirmed clients, one explicit operation decoy rejection, one name-only
candidate with an unresolved coverage gap, and six exact typed gaps for
representable callbacks, webhooks, links, security semantics, server
variables, and non-JSON media. Equivalent reviewed JSON and YAML provider
contracts produce the same normalized service, operation, schema, field,
client, call-site, link, and contract-gap identities and the same
source-neutral semantic hash. Source paths, formats, byte digests, selectors,
and evidence identities remain format-specific and truthful.

The standard path executes no Git process, package manager, compiler, build
script, target, hook, plugin, network client, model provider, or Council. It
writes no filesystem content and uses no first-party `unsafe`. Duplicate YAML
keys, aliases, anchors, merge keys, custom tags, multiple documents, remote
references, local-reference cycles, malformed input, unsupported OpenAPI
versions, outside-root paths, and identity conflicts fail with typed errors.

Issue #78 reviewed and authorized only
`yaml-rust2 = { version = "=0.11.0", default-features = false }` as the future
implementation candidate. This governance package changes no manifest or
lockfile. The Ready implementation issue must retain the exact dependency,
checksum, transitive dependency, license, advisory, `unsafe`, marked-event,
duplicate-key, allocation, depth, fuzz, and maximum-plus-one evidence. A
second YAML or OpenAPI parser is a stop condition.

Every S6 resource uses inclusive charge before allocation or traversal.
Workspace bytes, repositories, contract bytes, YAML and reference depth, path
items, operations, schemas, fields, and report bytes require public command
max/max+1 evidence. Clients and other capacities not independently reachable
through the v1 workspace require component-counter conformance with
deterministic constructed state and injected resource observations; this
proves the shared counter boundary without claiming a public cardinality.

Issue #82 may resume no S6 production edit before protected merge of PR #84.
After that event, its retained public Red authorizes one coherent vertical
implementation pull request under the maintainer-supervised accelerated lane.
A separate machine-policy projection remains mandatory before unattended
autonomous execution but need not delay explicitly supervised interactive
implementation. Policy, workflow, agent instruction, and other control-plane
changes remain outside the product pull request.

Before S6 exists, the exact `federate` invocation reaches the accepted
unrecognized-command boundary and exits `2` with the exact 149-byte ErrorV2
`input.invalid_revision` document on stderr, SHA-256
`6441e0037f864d2fae4a60e6355e4a85b26b00d5e4e24c59ffeb5fe9c6f3859f`,
and empty stdout. The first implementation Red is the exact
`e2e_fr_fed_001_openapi_federation` command in the machine oracle.
Compilation, fixture, dependency, network, panic, timeout, target-execution,
or hand-authored-output failures are rejected reasons.

S6 bounded OpenAPI federation contract bundle:
`sha256:cc8bb6a42abc23f51068c94641ace7775f4979108aa153a050bde93580d6db13`.
The bundle binds Decision 0009, the independent governance guard, strict
workspace/client/report/error schemas, rule catalog, machine oracle,
project-owned fixture and hostile variants, reviewed outputs, and immutable S7
identity dependencies. Any bound-byte change requires a new digest and renewed
human review.

### 2.12 S1 gitlink boundary ratification register

Issue [#92](https://github.com/smutti/codenoesis/issues/92) records explicit
maintainer authorization for this high-risk governance package. The following
single requirement becomes Approved only when `@smutti` manually merges the
exact protected head of
[PR #93](https://github.com/smutti/codenoesis/pull/93).
The authoring agent does not approve or merge. This approval targets exactly
the **S1 — Safe inventory compatibility extension** and roadmap R2; it does
not reopen or reimplement the merged R0/R1 behavior in `FR-ACQ-004`.

| Requirement | Current state | Target state | Product owner | Technical approver | Approval reference | Slice | Ratification material |
|---|---|---|---|---|---|---|---|
| `FR-ACQ-005` | `Proposed` (authorized in issue #92; pending protected merge) | `Approved` | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #93 protected merge record](https://github.com/smutti/codenoesis/pull/93) | `S1` | [Gitlink boundary decision](decisions/0010-s1-gitlink-boundary-contract.md), [RepositorySnapshotV5 schema](../../tests/specifications/s1/repository-snapshot-v5.schema.json), [ErrorV9 schema](../../tests/specifications/s1/codenoesis-error-v9.schema.json), [machine oracle](../../tests/specifications/s1/e2e_fr_acq_005_gitlink_boundaries.json), and [project-owned fixture](../../tests/fixtures/s1/gitlink-boundary-v1/README.md) |

R2 is selected only by
`--repository-boundary-profile local-gitlinks-v1` on an otherwise valid
`standard-local-s4` scan. It may coexist with the implemented R1
`local-git-sha1-packed-v1` operational selector. Repository shape,
`.gitmodules`, a present nested worktree, Git configuration, or a legacy error
never selects it implicitly. Every invocation without the R2 selector remains
byte-for-byte unchanged, including the accepted selector-absent gitlink
rejection.

The selected scan emits `RepositorySnapshotV5`. Mode `160000` entries become
external repository boundaries with canonical path and committed nested commit
OID; they never become files, traversable directories, or root ontology facts.
The root committed `.gitmodules` blob is parsed only under the bounded grammar
in Decision 0010. URLs are inert metadata represented only by lexical kind and
SHA-256. Missing nested checkouts succeed with typed unbound states and gaps.

An optional strict boundary manifest may explicitly supply at most 32 nested
repositories below its confinement directory. Each is independently bound to
the exact parent gitlink commit at depth one. Loose/packed physical storage and
all input paths remain operational; only the verified nested identity, commit,
and tree enter semantic output. Nested source is not analyzed or merged in R2.

The primary preimplementation command fails before repository acquisition
because the selector is unknown: exit `2`, empty stdout, no store, and the
exact 149-byte LF-terminated ErrorV4 `input.invalid_revision` stderr with
SHA-256
`7f75f7a91f6af0328795f3fbd2729e69756beba2ebd642cc1f6401265662a2fe`.
That is the only accepted first product Red. Compilation, malformed fixture,
legacy gitlink acceptance, panic, timeout, dependency outage, or side effects
are rejected reasons.

The accepted R0/R1 corpus and bundles remain immutable. A future product PR
must retain those regressions, implement the complete R2 vertical behavior,
and demonstrate a non-committed pinned RustDesk run that advances beyond the
generic R2 blocker without fetching nested source or introducing
repository-specific semantics.

S1 safe gitlink boundary contract bundle:
`sha256:2f59bb311b64b0f4f9d506266f05e9e52f4c0bf5af8926276ed371967690969b`.
The bundle binds Decision 0010, the independent governance guard, strict V5,
boundary-input, boundary-report, and ErrorV9 schemas, machine oracle,
project-owned synthetic fixture and exact Git identities, plus immutable R0/R1
and S4 bundle dependencies. Any bound-byte change requires a new digest and
renewed human review.

### 2.13 S4 root-package workspace ratification register

Issue [#96](https://github.com/smutti/codenoesis/issues/96) and its explicit
maintainer authorization
[comment](https://github.com/smutti/codenoesis/issues/96#issuecomment-5158164180)
govern this high-risk package. The following single requirement became
Approved when `@smutti` manually merged the exact protected head of
[PR #97](https://github.com/smutti/codenoesis/pull/97), and its product
implementation was merged through
[PR #99](https://github.com/smutti/codenoesis/pull/99). It remains not
Verified. The authoring agent did not approve or merge. This approval targets exactly the
**S4 — Evidence-backed workspace docs compatibility extension** and roadmap
R3. It does not broaden the selector-absent `FR-EXT-007` contract.

For immutable historical-guard traceability, the pre-ratification row state was
`FR-EXT-008` | `Proposed`; the current normative row below supersedes that
state with the protected approval and implementation records.

| Requirement | Current state | Target state | Product owner | Technical approver | Approval reference | Slice | Ratification material |
|---|---|---|---|---|---|---|---|
| `FR-EXT-008` | `Implemented` (approved in PR #97; product merge #99; not Verified) | `Verified` | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #97 protected approval](https://github.com/smutti/codenoesis/pull/97), [PR #99 implementation](https://github.com/smutti/codenoesis/pull/99) | `S4` | [Root-package workspace decision](decisions/0011-s4-root-package-workspace-contract.md), [RepositorySnapshotV6 schema](../../tests/specifications/s4/r3/repository-snapshot-v6.schema.json), [ErrorV10 schema](../../tests/specifications/s4/r3/codenoesis-error-v10.schema.json), [machine oracle](../../tests/specifications/s4/r3/e2e_fr_ext_008_root_package_workspace.json), and [project-owned fixture](../../tests/fixtures/s4/root-package-workspace-v1/README.md) |

R3 is selected only by `--workspace-profile cargo-root-package-v1` on an
otherwise valid `standard-local-s4` scan. R1 packed acquisition and R2 gitlink
representation remain independent optional selectors. A repository shape,
root `[package]`, `workspace.members = ["."]`, or corpus identity never selects
R3 implicitly. Every invocation without the R3 selector remains byte-for-byte
unchanged, including V4/V5 snapshots, errors, publication, docs, query, and the
legacy root-package `extraction.unsupported_workspace` result.

The selected path accepts a bounded standalone root package, a virtual literal
workspace, or a non-virtual workspace whose root package is implicit or
explicitly named by `"."`. The root manifest identity is canonical
`Cargo.toml`; an explicit and implicit root never produce duplicate crates.
Literal normalized members and exclusions, multiple member manifests, and
bounded conventional or explicit library/binary roots are structural facts.
Cargo metadata, dependency, feature, patch, target-world, build-script, macro,
and generated behavior outside that subset remains typed R4-deferred coverage,
not active semantics.

The selected scan emits strict `RepositorySnapshotV6` with
`codenoesis.configuration/v3`, `codenoesis.extraction/v3`,
`codenoesis.knowledge-graph/v3`, and `codenoesis.ontology/rust/v3`. Ontology v3
retains the v2 entity and relationship kinds, identity domains, preimages, and
cardinalities for unchanged facts while adding closed workspace-member
provenance and R3 coverage semantics. V6 carries the canonical R2 boundary
projection only when the independent R2 selector is present. A gitlink
workspace member is never a root Rust crate and remains an external boundary
requiring that explicit R2 selector.

V6 reuses the existing immutable artifact roles, local-store marker, DDL,
single-writer transaction, documentation format, and exact-ID query contract.
No migration, repair, role, or destructive action is part of R3. Readers may
accept V6 only after validating every V6 semantic and artifact binding; V4/V5
behavior remains unchanged.

The first implementation command fails before repository acquisition because
the selector is unknown: exit `2`, empty stdout, no store, and the exact
149-byte LF-terminated ErrorV4 `input.invalid_revision` stderr with SHA-256
`7f75f7a91f6af0328795f3fbd2729e69756beba2ebd642cc1f6401265662a2fe`.
That is the only accepted first product Red. Compilation, malformed fixture,
acquisition failure, panic, timeout, target execution, or a different subject
error is rejected Red evidence.

The governance fixture, strict schemas, machine oracle, existing S4/R2 bundle
dependencies, and non-vendored pinned Lekton and RustDesk pilot expectations
are bound by the R3 contract bundle. Any bound-byte change requires a new
digest and renewed human review.

R3 root-package workspace contract bundle:
`sha256:0b99760da4e978fefa91468b5dbef1b59816e30b02d92c70c26a7df715ef509a`.

### 2.14 S4 Cargo manifest facts ratification register

Issue [#100](https://github.com/smutti/codenoesis/issues/100), its explicit
maintainer authorization
[comment](https://github.com/smutti/codenoesis/issues/100#issuecomment-5163551187),
and protected [PR #103](https://github.com/smutti/codenoesis/pull/103) Approved
`FR-EXT-009`. Issue [#105](https://github.com/smutti/codenoesis/issues/105)
and its explicit maintainer
[authorization](https://github.com/smutti/codenoesis/issues/105#issuecomment-5170981186)
govern the additive exact-ID correction required to make that approved R4
outcome implementation-complete; protected PR #106 merged that correction at
`557547285f9532772efceea900ba982b6a8e65a9`. Issue
[#107](https://github.com/smutti/codenoesis/issues/107) and its explicit
maintainer
[authorization](https://github.com/smutti/codenoesis/issues/107#issuecomment-5177741408)
govern the observed legacy `[badges]` typed-unsupported correction. Decision
0014 becomes effective only when `@smutti` manually merges its exact
protected head. The authoring agent does not approve or merge. All packages
target exactly the **S4 — Evidence-backed workspace docs compatibility
extension** and roadmap R4; they do not broaden R3 or any selector-absent
contract.

| Requirement | Current state | Target state | Product owner | Technical approver | Approval reference | Slice | Ratification material |
|---|---|---|---|---|---|---|---|
| `FR-EXT-009` | `Approved` (protected PR #103; issue #107 amendment authorized and pending protected merge) | `Approved` with exact legacy-family typed coverage | Andrea Moretti (`@smutti` persona) | `@smutti` | [PR #103 protected merge record](https://github.com/smutti/codenoesis/pull/103) and [issue #107 authorization](https://github.com/smutti/codenoesis/issues/107#issuecomment-5177741408) | `S4` | [Cargo manifest facts decision](decisions/0012-s4-cargo-manifest-facts-contract.md), [legacy badges decision](decisions/0014-s4-r4-legacy-badges-contract.md), [RepositorySnapshotV7 schema](../../tests/specifications/s4/r4/repository-snapshot-v7.schema.json), [Rust/Cargo ontology v4](../../tests/specifications/s4/r4/rust-ontology-v4.json), [ErrorV11 schema](../../tests/specifications/s4/r4/codenoesis-error-v11.schema.json), [machine oracle](../../tests/specifications/s4/r4/e2e_fr_ext_009_cargo_manifest_facts.json), [retained badges Red](../../tests/specifications/s4/r4/legacy-badges-red-observation.json), and [project-owned fixture](../../tests/fixtures/s4/cargo-manifest-facts-v1/README.md) |
| `FR-QRY-001` | `Approved` (R4 amendment authorized in issue #105; pending protected merge) | `Approved` with additive V7 query contract | Andrea Moretti (`@smutti` persona) | `@smutti` | [Issue #105 authorization](https://github.com/smutti/codenoesis/issues/105#issuecomment-5170981186) | `S4` | [Exact-ID query decision](decisions/0013-s4-r4-exact-id-query-contract.md), [LocalQueryResultV2 schema](../../tests/specifications/s4/r4/local-query-result-v2.schema.json), [query oracle](../../tests/specifications/s4/r4/e2e_fr_qry_001_r4_exact_id_results.json), and [retained governance Red](../../tests/specifications/s4/r4/query-v2-red-observation.json) |

R4 is selected only by `--manifest-profile cargo-manifest-facts-v1` on an
otherwise valid `standard-local-s4` scan that also explicitly selects
`--workspace-profile cargo-root-package-v1`. R1 packed acquisition and R2
gitlink representation remain independent optional selectors. Manifest
content, repository shape, a dependency table, corpus identity, or prior
profile never selects R4 implicitly. Every invocation without the R4 selector
remains byte-for-byte unchanged, including RepositorySnapshotV6 and every
earlier success, failure, publication, docs, and query byte.

The selected profile represents bounded committed Cargo declarations as
evidence-backed `cargo.manifest`, workspace-default, package, target,
dependency, feature, patch, and build-script entities. `DECLARES` and
`REFERENCES_DECLARATION` preserve literal ownership and workspace references;
`MATERIALIZES` links only an R3-analyzed lib/bin declaration to its unchanged
Rust crate ID. R4 emits no `DEPENDS_ON`, resolver, activation, target-selection,
patch-application, or execution relationship.

Package metadata values, direct/inherited provenance, target declarations,
registry/path/Git/workspace dependency source kinds, target predicates,
optional/default-feature flags, requested features, feature-member syntax,
`required-features`, patch declarations, and build-script presence retain
exact byte evidence. Path declarations normalize inside the bound repository
without traversal. Package/dependency/patch external locators and Git
branch/tag/rev values emit only SHA-256 plus evidence; plaintext is forbidden
in derived artifacts, errors, logs, documentation, and query output.

Dependency graphs, package versions/sources, active feature/target/cfg worlds,
effective workspace inheritance, patch application, generated source, and
Cargo validation remain explicit typed gaps. Cargo, rustc, build scripts,
procedural macros, targets, dependencies, Git, registries, path dependencies,
network clients, and model providers never execute or open under this profile.
Reviewed unsupported metadata/profile/lint/replace/advanced-dependency families
and the legacy top-level `[badges]` family emit exact diagnostics and coverage
rather than disappearing silently. Badge values remain uninterpreted and never
enter derived output; every other unknown key remains fail-closed.

The selected scan emits strict `RepositorySnapshotV7`
(`codenoesis.repository-snapshot/v7`) with
`codenoesis.configuration/v4`, `codenoesis.extraction/v4`,
`codenoesis.knowledge-graph/v4`, `codenoesis.ontology/rust/v4`, and
`codenoesis.error/v11`. Ontology v4
retains all v3 Rust identity domains and preimages and adds disjoint Cargo
declaration domains. V7 reuses the existing immutable artifact roles, local
store marker, DDL, single-writer transaction, crash behavior, documentation
format, and query command. A validated V7 head emits additive strict
`codenoesis.local-query-result/v2`; V4-V6 heads retain byte-identical
`codenoesis.local-query-result/v1`. No migration, repair, role, deletion, or
destructive action is part of R4.

The first implementation command fails before repository acquisition because
the selector is unknown: exit `2`, empty stdout, no store, and the exact
149-byte LF-terminated ErrorV4 `input.invalid_revision` stderr with SHA-256
`7f75f7a91f6af0328795f3fbd2729e69756beba2ebd642cc1f6401265662a2fe`.
That is the only accepted first product Red. Compilation, malformed fixture,
acquisition failure, panic, timeout, target/dependency execution, side effect,
or a different subject error is rejected Red evidence.

The governance fixture, strict schemas, machine subset and oracle, exact Red
observation, immutable R3 bundle dependency, and non-vendored pinned Lekton and
RustDesk pilot expectations are bound by the R4 contract bundle. Any bound-byte
change requires a new digest and renewed human review.

R4 Cargo manifest facts contract bundle:
`sha256:2588abf38d686cc6475e7662ad8e90d585d1cdbff77702231dcadb1626a0c249`.

### 2.15 S4 Rust semantic-depth ratification register

Issue [#111](https://github.com/smutti/codenoesis/issues/111), its explicit
accountable-maintainer
[authorization](https://github.com/smutti/codenoesis/issues/111#issuecomment-5179817871),
protected governance [PR #112](https://github.com/smutti/codenoesis/pull/112),
golden correction [PR #115](https://github.com/smutti/codenoesis/pull/115), and
product [PR #116](https://github.com/smutti/codenoesis/pull/116) record the R5
Rust semantic-depth contract as Approved and Implemented but not Verified.
Decision 0015 became effective through the protected manual governance merge;
the later product merge does not provide complete immutable verification
evidence. The package targets exactly the **S4 — Evidence-backed workspace docs
compatibility extension** and roadmap R5; it does not broaden S5, S6, S7, R6,
R7, or R8.

| Requirement | Current state | Target state | Product owner | Technical approver | Approval reference | Slice | Ratification material |
|---|---|---|---|---|---|---|---|
| `FR-EXT-010` | `Implemented` (`Approved`, not `Verified`) | `Verified` only after complete immutable retention evidence is independently accepted | Andrea Moretti (`@smutti` persona) | `@smutti` | [Issue #111 authorization](https://github.com/smutti/codenoesis/issues/111#issuecomment-5179817871), governance [PR #112](https://github.com/smutti/codenoesis/pull/112), correction [PR #115](https://github.com/smutti/codenoesis/pull/115), and product [PR #116](https://github.com/smutti/codenoesis/pull/116) | `S4` | [Rust semantic-depth decision](decisions/0015-s4-r5-rust-semantic-depth-contract.md), [RepositorySnapshotV8 schema](../../tests/specifications/s4/r5/repository-snapshot-v8.schema.json), [ExtractionChunkV5 schema](../../tests/specifications/s4/r5/extraction-chunk-v5.schema.json), [KnowledgeGraphV5 schema](../../tests/specifications/s4/r5/knowledge-graph-v5.schema.json), [Rust ontology v5](../../tests/specifications/s4/r5/rust-ontology-v5.json), [ErrorV12 schema](../../tests/specifications/s4/r5/codenoesis-error-v12.schema.json), [LocalQueryResultV3 schema](../../tests/specifications/s4/r5/local-query-result-v3.schema.json), [machine oracle](../../tests/specifications/s4/r5/e2e_fr_ext_010_rust_semantic_depth.json), [retained governance Red](../../tests/specifications/s4/r5/red-observation.json), and [project-owned fixture](../../tests/fixtures/s4/rust-semantic-depth-v1/README.md) |

R5 is selected only by
`--rust-semantic-profile rust-semantic-depth-v1` on an otherwise valid
`standard-local-s4` scan that also explicitly selects
`--workspace-profile cargo-root-package-v1` and
`--manifest-profile cargo-manifest-facts-v1`. R1 packed acquisition and R2
gitlink representation remain independent optional selectors. Repository
content, Rust attributes, macros, Cargo declarations, public-corpus identity,
and prior profiles never select R5 implicitly. Every invocation without the
R5 selector remains byte-for-byte unchanged, including every R0-R4 success,
error, storage, documentation, and query byte.

The selected profile represents only bounded committed Rust declaration
syntax. Ontology v5 adds `rust.field`, `rust.enum_variant`, `rust.constant`,
`rust.static`, and `rust.associated_type`; it extends V8 `rust.method` with
trait-declaration, inherent-implementation, or named-local-trait-implementation
context. Named and tuple fields, unit/tuple/struct variants, module and
associated constants, immutable and mutable statics, associated types, trait
required/default methods, inherent methods, and supported named local trait
methods retain exact committed-source evidence.

New member entities use the disjoint
`codenoesis.entity-id/rust-member/v1` domain over repository identity,
unchanged crate identity, lexical owner identity, member kind, NFC declared
name or zero-based tuple index, and resolved local trait identity or the empty
string. Trait context prevents same-name method collisions. Commit OID, byte
offset, file/output path, source order, scheduler order, inferred type,
evaluated value, active configuration, and macro output never enter identity.
Normalization collisions fail with a typed error; they are never repaired by
ordinal or offset.

Outer attributes no longer cause a committed declaration to disappear.
`#[cfg]` records `conditional_unknown` plus
`rust.cfg_presence_unresolved`; `#[cfg_attr]` records
`attribute_transform_unknown` plus the same unresolved capability. Every
other outer attribute emits
`rust.attribute_semantics_not_interpreted`. Attribute tokens remain bounded
syntax evidence and never imply a component, service, configuration,
endpoint, route, handler, active branch, generated item, or runtime role.

R5 does not evaluate `cfg`, derive, macros, types, values, initializers,
discriminants, calls, data flow, control flow, or runtime behavior. It does not
execute Cargo, rustc, build scripts, procedural macros, targets, dependencies,
network clients, or model providers. Unions, foreign blocks, macro-generated
items, unresolved implementation headers, type resolution, and value
evaluation remain exact typed diagnostics or coverage gaps. R5 emits no
`CALLS` or `EXECUTES` relationship.

The selected scan emits strict `codenoesis.repository-snapshot/v8` with
`codenoesis.configuration/v5`, `codenoesis.extraction-chunk/v5`,
`codenoesis.knowledge-graph/v5`, `codenoesis.ontology/rust/v5`,
`codenoesis.error/v12`, and pipeline `codenoesis.pipeline/s4-r5-v1`.
A validated V8 head emits additive
`codenoesis.local-query-result/v3`; V7 retains byte-identical
`codenoesis.local-query-result/v2`, and V4-V6 retain byte-identical
`codenoesis.local-query-result/v1`. The stored validated snapshot selects the
query result version; no public version flag is introduced.

Fixed maxima are 1,024 fields per owner, 1,024 variants per enum, 1,024 tuple
fields per owner, 1,024 associated items per context, 128 outer attributes per
declaration, 16,384 UTF-8 bytes per attribute token payload, 4,096 UTF-8 bytes
per declared type or implementation-header spelling, and 50 determinism
permutations. Existing S2 graph maxima remain authoritative. Maximum-plus-one
fails with `extraction.rust_semantic_limit_exceeded` and no partial
publication; no bound silently truncates.

The project-owned fixture fixes exact fields, variants, constants/statics,
associated types, method contexts, raw and Unicode identifiers, attributes,
hard negatives, and a build sentinel that must never execute. The pinned
Lekton commit `7a4d1a4a30468f4c18ce158a9b825680b00f4820` and RustDesk commit
`d412d198720aa56f6cfed2dfad262e8fb1322fb7` remain non-vendored,
replaceable pilot observations rather than ontology goldens.

The governance conformance test was committed before Decision 0015 and every
R5 schema, subset, fixture, and oracle. On test-first head
`b6d3ec20b69258fac76fc42b5b95c7ea8f436da0`, the command
`python3 -m unittest scripts.tests.test_s4_rust_semantic_depth_contract`
failed for the expected missing-Decision reason with exit `1`. The retained
683-byte log has SHA-256
`d565d942729dece7b3cd08b2a67b714962b4de6979e3c7aa309061b5c4a89dd4`.
No production or semantic-contract byte changed before Red. Product
implementation requires a separate Ready issue and a separate executable CLI
Red after this protected package is merged.

The strict schemas, machine subset and oracles, retained Red, project-owned
fixture and expected facts, immutable R4 dependency, and non-vendored pilot
descriptors are bound by the R5 contract bundle. SRS and roadmap bytes are
excluded from that digest. Any bound-byte change requires a new digest and
renewed semantic human review.

R5 Rust semantic-depth contract bundle:
`sha256:ed48512d8337d2dda2a3b5f752177f3988915bdfc98eda1ff2391e15039e7d45`.

### 2.16 S4 R6 framework-declarations ratification register

Issue [#117](https://github.com/smutti/codenoesis/issues/117), its explicit
accountable-maintainer
[authorization](https://github.com/smutti/codenoesis/issues/117#issuecomment-5183312890),
and protected [PR #118](https://github.com/smutti/codenoesis/pull/118) govern
the proposed R6 framework-declarations contract. Decision 0016 becomes
effective only when `@smutti` manually merges the exact independently reviewed
protected head. The authoring agent does not approve or merge. This package
targets exactly the **S4 — Evidence-backed workspace docs compatibility
extension** and roadmap R6; it does not broaden S5, S6, S7, R7, R8, compiler
enrichment, runtime observation, export, explorer, server, MCP, or release
behavior.

| Requirement | Current state | Target state | Product owner | Technical approver | Approval reference | Slice | Ratification material |
|---|---|---|---|---|---|---|---|
| `FR-EXT-011` | `Proposed` | `Approved` only after protected manual merge of PR #118 | Andrea Moretti (`@smutti` persona) | `@smutti` | [Issue #117 authorization](https://github.com/smutti/codenoesis/issues/117#issuecomment-5183312890) and [PR #118](https://github.com/smutti/codenoesis/pull/118) | `S4` | [Framework-declarations decision](decisions/0016-s4-r6-framework-declarations-contract.md), [RepositorySnapshotV9 schema](../../tests/specifications/s4/r6/repository-snapshot-v9.schema.json), [ExtractionChunkV6 schema](../../tests/specifications/s4/r6/extraction-chunk-v6.schema.json), [KnowledgeGraphV6 schema](../../tests/specifications/s4/r6/knowledge-graph-v6.schema.json), [Rust ontology v6](../../tests/specifications/s4/r6/rust-ontology-v6.json), [ErrorV13 schema](../../tests/specifications/s4/r6/codenoesis-error-v13.schema.json), [LocalQueryResultV4 schema](../../tests/specifications/s4/r6/local-query-result-v4.schema.json), [machine oracle](../../tests/specifications/s4/r6/e2e_fr_ext_011_framework_declarations.json), [retained governance Red](../../tests/specifications/s4/r6/red-observation.json), and [project-owned fixture](../../tests/fixtures/s4/framework-declarations-v1/README.md) |

R6 is selected only by
`--rust-framework-profile rust-framework-declarations-v1` on an otherwise
valid `standard-local-s4` scan that also explicitly selects
`--workspace-profile cargo-root-package-v1`,
`--manifest-profile cargo-manifest-facts-v1`, and
`--rust-semantic-profile rust-semantic-depth-v1`. R1 packed acquisition and R2
repository-boundary representation remain independent optional selectors.
Repository content, dependency names, imports, attributes, macros, file names,
public-corpus identity, and prior profiles never select R6 implicitly. Missing
or incomplete composition fails before acquisition. Every invocation without
the R6 selector remains byte-for-byte unchanged through R5.

Ontology v6 adds only framework-neutral source declaration entities:
`framework.component_declaration`, `framework.service_declaration`,
`framework.configuration_declaration`, `framework.endpoint_declaration`,
`framework.route_declaration`, and `framework.handler_declaration`. These
kinds never denote framework brands or runtime objects. Each entity has one
lexical R5 owner, one closed source-profile rule, inherited compilation
presence, exact committed byte evidence, and one of exactly two states:

- `declared_registration_syntax` means a reviewed direct builder source form
  exists; it does not prove the enclosing function runs or its result is used;
- `candidate_unresolved` means a reviewed attribute, derive, `cfg`,
  `cfg_attr`, declarative-macro, or proc-macro-looking form resembles a role
  while its meaning remains unsupported.

Resolved or observed runtime behavior has no R6 schema state. Candidate macro
arguments remain raw evidence and never become authoritative methods, paths,
keys, targets, roles, or generated declarations. Every declaration documents
that runtime behavior was not observed. Conditional, attribute, macro,
unresolved, and ambiguous forms additionally emit their exact reviewed
diagnostic and coverage gap.

The closed explicit-builder source profile accepts only reviewed direct
return-tail call chains rooted in the approved `RegistrationSet::new` or
`Router::new` constructor spelling. Its bounded rules cover direct
component/service/configuration/endpoint/handler registration, literal
method-path-target routes, literal path plus reviewed method-wrapper routes,
`group`/`nest`, `layer`/`route_layer`, and `with_state`. Constructor spelling
does not resolve type or framework meaning. Standalone or unused builders,
returned aliases requiring data flow, dynamic paths, closures, arbitrary
expressions, and forms outside the closed rules are not promoted.

The independent attribute/macro profile retains the underlying R5 declaration
and emits only an unresolved candidate for reviewed route-, component-,
service-, configuration-, command-, runtime-entry-, bridge-, endpoint-,
derive-, `cfg`-, `cfg_attr`-, and declarative-route-macro-looking syntax. It
never expands a macro, evaluates configuration, interprets arguments, infers a
generated declaration, or assigns runtime behavior. Comments, strings,
documentation, imports, return types, parameter wrappers, dependency names,
isolated identifiers, name-only conventions, generated directories, target
directories, and unused builder values remain hard negatives.

Framework declarations use the disjoint
`codenoesis.entity-id/framework-declaration/v1` domain. The NFC-normalized RFC
8785 JSON-array preimage contains repository identity, unchanged crate
identity, lexical owner identity, framework-neutral role, source profile,
source-form rule identity, and normalized declared key or target spelling.
The public ID retains the existing
`urn:codenoesis:entity:blake3:<digest>` shape. Commit OID, byte offset, source
or chunk order, schedule, active `cfg` world, macro output, inferred type,
evaluated value, runtime address, and runtime state never enter identity.
Duplicate or normalization-colliding preimages fail before publication and
cannot be repaired by ordinal, offset, ordering, scheduling, or retry.

R6 reuses only `DEFINES` for lexical ownership. A local target identity is a
nullable property and is retained only when existing committed R5 lexical
facts resolve exactly one declaration. An external target remains unresolved;
an ambiguous local spelling retains the reviewed ambiguity gap or fails where
the product oracle requires exact binding. Source order cannot select a
candidate. R6 emits no `CALLS`, `EXECUTES`, `SERVES`, `STARTS`, `REACHES`, or
`ACTIVATES` relationship, and no property may be reinterpreted as runtime
reachability, execution, service start, active configuration, endpoint
availability, or middleware order.

The selected scan emits strict `codenoesis.repository-snapshot/v9` with
`codenoesis.configuration/v6`, `codenoesis.extraction-chunk/v6`,
`codenoesis.knowledge-graph/v6`, `codenoesis.ontology/rust/v6`,
`codenoesis.error/v13`, and pipeline `codenoesis.pipeline/s4-r6-v1`.
A validated stored V9 head emits additive
`codenoesis.local-query-result/v4` for exact entity, relationship, claim,
evidence, diagnostic, coverage-gap, and document IDs. V8 retains byte-identical
LocalQueryResultV3, V7 retains V2, and V4-V6 retain V1. The stored validated
head selects the result version; no public query-version flag, traversal,
fuzzy search, migration, repair, or runtime inference is introduced.

Fixed maxima are 4,096 framework declarations per committed source file, 256
explicit registration chain segments, registration-expression depth 64, 2,048
UTF-8 bytes per literal route path, 1,024 UTF-8 bytes per literal method or
configuration key, 1,024 UTF-8 bytes per target spelling, inherited R5 maxima
of 128 outer attributes and 16,384 UTF-8 attribute-token bytes, and 50
determinism permutations plus isolated replay. Existing graph, snapshot,
repository, docs, query, memory, output, and wall-time limits remain
authoritative. Every maximum-plus-one fails before proportional allocation or
publication with no silent truncation, stdout, partial store, or documentation
mutation.

New failures are strict LF-terminated ErrorV13 with empty stdout and no partial
mutation: invalid profile, unsupported composition, malformed declaration,
identity or NFC collision, limit exceeded, required-target ambiguity,
unresolvable evidence, unsafe path, or internal contract failure. An
intentionally unsupported candidate remains an exact diagnostic and coverage
gap rather than an error, silent omission, or invented fact.

The Apache-2.0 project-owned `framework-declarations-v1` fixture contains two
independent modules. The explicit builder module covers every entity role,
nested groups, configuration, duplicate paths under distinct methods, unique,
external, and ambiguous targets, and an unused builder. The attribute/macro
module covers all reviewed candidate families and `cfg` uncertainty. Comments,
strings, docs, imports, names, macro-generated tokens, generated and target
directories, and a `build.rs` sentinel are hard negatives. Conformance must
never compile, execute, expand, fetch, link, or open external sources for this
fixture. Its manifest and expected facts bind every byte, identity, owner,
state, evidence span, diagnostic, gap, document statement, and query kind.

The governance conformance guard was committed before Decision 0016 and every
R6 schema, fixture, golden, and bundle. On test-first head
`be5ffb9a8380975ab8458adfb5ca55a70540d268`, the command
`python3 -m unittest scripts.tests.test_s4_r6_framework_declarations_contract`
failed for the expected missing-Decision reason with exit `1` and empty
stdout. The retained 704-byte stderr log has SHA-256
`aad6a707da779c1737c863aa693826933015031da7a989914276f105b7604b68`;
the test-first guard has SHA-256
`2dc7e2627165f6879733562b1459216365f6a0f0d9f6eceed7e37f65d1c3a48f`.
No production, dependency, R5 contract, fixture, golden, or unrelated
protected byte changed before Red. Product implementation requires a separate
Ready issue and a separate executable CLI Red after this package is merged.

Pinned non-vendored Lekton commit
`7a4d1a4a30468f4c18ce158a9b825680b00f4820` motivates the explicit-router
style; pinned non-vendored RustDesk commit
`d412d198720aa56f6cfed2dfad262e8fb1322fb7` motivates unresolved
attribute/macro candidates while its gitlink remains unopened. The retained
counts are lexical, motivation-only observations, not goldens, runtime facts,
completeness, performance evidence, or repository-specific product semantics.

The strict schemas, machine subset and oracles, retained Red, project-owned
fixture and expected facts, immutable R5 dependency, invalid/security matrix,
and non-vendored pilot descriptor are bound by the R6 contract bundle. SRS and
roadmap bytes are excluded from that digest. Any bound-byte change requires a
new digest and renewed semantic human review.

R6 framework-declarations contract bundle:
`sha256:46f5e0fab0439979c456cb41ce7195efd5e02a342be4292402ef2cb44909bc47`.

### 2.17 S4 R7 revision-bound SCIP import ratification register

Issue [#123](https://github.com/smutti/codenoesis/issues/123), its explicit
accountable-maintainer
[authorization](https://github.com/smutti/codenoesis/issues/123#issuecomment-5193618752),
and the independently reviewed protected pull request created from that issue
govern the proposed R7 static compiler-index contract. The factual
[supply-chain correction](https://github.com/smutti/codenoesis/issues/123#issuecomment-5193814091)
records that `protobuf 3.7.2` is MIT while `scip 0.9.0` is Apache-2.0; dependency
names, versions, scope, and risk are unchanged. Decision 0017 becomes effective
only when `@smutti` manually merges the exact reviewed head. The authoring agent
does not approve or merge.

This register targets exactly the **S4 — Evidence-backed workspace docs
compatibility extension** and roadmap R7 static import. It does not broaden S5,
S6, S7, S8, S9, R8, compiler/indexer generation, sandbox execution, runtime
observation, export, explorer, server, MCP, release, or control-plane behavior.

| Requirement | Current state | Target state | Product owner | Technical approver | Approval reference | Slice | Ratification material |
|---|---|---|---|---|---|---|---|
| Bounded `FR-EXT-005` for `codenoesis.compiler-index-profile/scip-rust-v0.9.0-import-v1` | `Proposed`; broader polyglot meaning remains `Proposed` | `Approved` only for this static Rust SCIP profile after protected manual merge | Andrea Moretti (`@smutti` persona) | `@smutti` | [Issue #123 authorization](https://github.com/smutti/codenoesis/issues/123#issuecomment-5193618752) | `S4` | [SCIP import decision](decisions/0017-s4-r7-scip-import-contract.md), [binding schema](../../tests/specifications/s4/r7/compiler-index-binding-v1.schema.json), [RepositorySnapshotV10 schema](../../tests/specifications/s4/r7/repository-snapshot-v10.schema.json), [ExtractionChunkV7 schema](../../tests/specifications/s4/r7/extraction-chunk-v7.schema.json), [KnowledgeGraphV7 schema](../../tests/specifications/s4/r7/knowledge-graph-v7.schema.json), [Rust ontology v7](../../tests/specifications/s4/r7/rust-ontology-v7.json), [ErrorV14 schema](../../tests/specifications/s4/r7/codenoesis-error-v14.schema.json), [LocalQueryResultV5 schema](../../tests/specifications/s4/r7/local-query-result-v5.schema.json), [machine oracle](../../tests/specifications/s4/r7/e2e_fr_ext_005_scip_import.json), [retained governance Red](../../tests/specifications/s4/r7/red-observation.json), and [project-owned fixture](../../tests/fixtures/s4/compiler-index-v1/README.md) |

R7 is selected only by
`--compiler-index-profile scip-rust-v0.9.0-import-v1` together with an explicit
`--compiler-index-binding <safe-relative-path>` on a valid
`standard-local-s4` scan that also selects `cargo-root-package-v1`,
`cargo-manifest-facts-v1`, `rust-semantic-depth-v1`, and
`rust-framework-declarations-v1`. Repository content, environment, dependency
names, a conventional artifact filename, prior selectors, or cached state never
enable R7 implicitly. Incomplete composition fails before acquisition. Every
invocation without both R7 inputs remains byte-identical through R6.

The only accepted wire schema is SCIP v0.9.0 at commit
`e8ee0ae6038f8298e2195812eea9d7b1196748ae`, with `scip.proto` SHA-256
`04cb20f2b8be73f6c0376b5b3e84c3ae20ebaff0ad3d23ba2d16f866b395ed7d`.
Metadata and every Rust document must use the approved UTF-8 encodings. The
accepted producer family is rust-analyzer SCIP, and the binding preserves exact
producer name, release, commit, digest-only arguments and project root,
toolchain release and commit, and target triple. These values establish declared
provenance and consistency, not executable attestation.

`CompilerIndexBindingV1` strictly binds repository identity, immutable commit
and tree OIDs, the R6 source-manifest digest, artifact path/length/SHA-256/schema,
producer and toolchain, and every indexed or omitted document. Indexed documents
bind canonical repository-relative path, committed blob OID, SHA-256, and byte
length. Omission reasons, coverage mode, generated exclusions, and known
producer limits are explicit. Neither `Metadata.project_root`, raw arguments,
embedded source text, documentation, signatures, diagnostics, external prose,
nor environment values grant filesystem authority or enter public bytes.

A bounded framing preflight runs before generated protobuf decode or
proportional allocation. It enforces raw bytes, legal wire types, metadata first
and exactly once, recursion, repeated counts, and length-delimited field bounds.
Recursive unknown fields, duplicate singular values, malformed or non-minimal
varints, invalid roles/ranges/encodings, and non-canonical re-encoding fail.
Every in-repository occurrence must resolve to exact committed UTF-8 bytes.
Traversal, absolute or escaping paths, symlinks, mutable-input races, stale
revisions, source mismatches, producer/toolchain mismatches, and changed
artifact bytes fail before graph construction or publication.

Ontology v7 adds only `compiler.symbol` assertions with binding state
`in_repository_bound`, `external_unbound`, or `generated_unbound`. Global SCIP
identities parse scheme, package manager/name/version, and descriptors; local
identities additionally bind repository and canonical document path. NFC
RFC 8785 preimages use the disjoint
`codenoesis.entity-id/compiler-symbol/v1` domain and BLAKE3-256. Commit, artifact
digest, ordering, offset, scheduling, and retry never enter symbol identity.
Duplicate, invalid, ambiguous, or normalization-colliding identities cannot be
repaired by ordinal or source order.

R7 adds only `RESOLVES_TO`, `REFERENCES`, `IMPLEMENTS`, and `TYPE_DEFINITION`.
The first two require unique source and lexical-owner bindings; the latter two
require their exact SCIP relationship flags and unique endpoints. A reference
is not a call. No `CALLS`, `EXECUTES`, `SERVES`, `STARTS`, `REACHES`, or
`ACTIVATES` relation, data flow, generated source declaration, macro expansion,
or runtime behavior is authorized. Unsupported call and generated-product
meaning remains an exact coverage gap.

Every promoted fact retains SHA-256 compiler-index evidence over the raw
artifact digest and canonical semantic locator plus exact committed-source
evidence for each local endpoint. A uniquely validated compiler relation may
outrank a syntax-only unresolved heuristic, but it never deletes syntax
evidence. Contradiction retains both evidence sets, emits a typed diagnostic and
gap, and blocks ambiguous promotion. External and generated symbols remain
compiler assertions and never become source or runtime declarations.

The selected scan emits strict `codenoesis.repository-snapshot/v10` with
`codenoesis.configuration/v7`, `codenoesis.extraction-chunk/v7`,
`codenoesis.knowledge-graph/v7`, `codenoesis.ontology/rust/v7`,
`codenoesis.error/v14`, and pipeline `codenoesis.pipeline/s4-r7-v1`.
A validated stored V10 head emits `codenoesis.local-query-result/v5` for exact
entity, relationship, claim, evidence, diagnostic, coverage-gap, and document
IDs, including compiler symbols and compiler evidence. V9 retains V4 query
bytes, V8 retains V3, V7 retains V2, and V4-V6 retain V1. No query-version flag,
traversal, fuzzy search, migration, repair, or runtime inference is introduced.

Fixed maxima are 67,108,864 raw artifact bytes, 1,048,576 binding JSON bytes,
20,000 documents, 1,000,000 total occurrences, 100,000 occurrences per
document, 250,000 symbol-information records, 500,000 relationships, 16,384
UTF-8 bytes per symbol/display value, 65,536 UTF-8 bytes per unpromoted value,
128 tool arguments, 4,096 UTF-8 bytes per tool argument, protobuf recursion 64,
and 50 deterministic permutations plus isolated replay. Inherited source/path,
60-second wall-time, and 512 MiB peak-RSS ceilings remain authoritative. Every
maximum-plus-one fails before proportional allocation or publication; silent
truncation is forbidden.

New selected failures are strict LF-terminated ErrorV14 with empty stdout and
no partial store/docs mutation: invalid profile or composition, unsafe path,
invalid binding, unsupported schema or producer, binding mismatch, malformed or
non-canonical artifact, identity collision, ambiguous endpoint, contradictory
relation, limit exceeded, unresolvable evidence, or internal failure. Reviewed
incomplete coverage remains a diagnostic/gap only where the oracle permits it.

The Apache-2.0 project-owned `compiler-index-v1` fixture contains a two-crate
Rust workspace, panic `build.rs` sentinel, reviewed source representation,
canonical binary artifact, strict binding, expected overlay, and manifest that
binds every byte. It covers cross-crate and Unicode references, aliases,
local/global/external/generated symbols, explicit implementation and
type-definition relations, one omitted document, exact source/compiler evidence,
privacy canaries, honest call/macro gaps, malformed inputs, stale/mismatched
bindings, path/race cases, and all maxima-plus-one. It launches no compiler,
indexer, build, proc macro, target, process, model, or network client.

The governance guard was committed first at
`bb9acb9ae7f6dbed4da2294cc85b12a7fb07b5ef`. The command
`python3 -m unittest scripts.tests.test_s4_r7_compiler_index_contract` failed
only for the expected missing Decision 0017, with exit `1`, empty stdout, and a
669-byte stderr log whose SHA-256 is
`6d8ef8c7850a87140028a25f49d920936338bc3191a5ed53d058c358434ef8cc`.
The retained test-first guard SHA-256 is
`61490532b478fc391bb383287503ecb8cd5f384aa1710aba4ffd93c4703c4ff9`.
No production, dependency, R6 contract, fixture, golden, or unrelated protected
byte changed before Red.

A future product issue may add exactly `scip = "=0.9.0"` and
`protobuf = "=3.7.2"`. This governance package changes no manifest or lockfile,
authorizes no generator or alternative parser, and performs no index generation.
Generation remains S9 work under a separately Approved trust/sandbox boundary.

The strict schemas, machine subset and oracles, supply-chain review, retained
Red, project-owned fixture and expected overlay, invalid/security matrix, and
immutable R6 dependency are bound by the R7 bundle. SRS and roadmap bytes are
excluded from that digest. Any bound-byte change requires a new digest and
renewed semantic human review.

R7 revision-bound SCIP import contract bundle:
`sha256:81ef2609c875af3d36a88f1fe97851f21368f90a60e2cc2706d6130ba95af882`.

### 2.18 S4 R8 portable export and offline explorer ratification register

Issue [#110](https://github.com/smutti/codenoesis/issues/110), its explicit
accountable-maintainer
[authorization](https://github.com/smutti/codenoesis/issues/110#issuecomment-5205875435),
and the independently reviewed protected pull request created from that issue
govern the proposed R8 package. The authorization supersedes the issue's stale
SnapshotV7/R4 baseline and binds the package to protected R7 product head
`d003a563830bdb5ff79197c8b92050b23eb92b27`. Decision 0018 becomes effective
only when `@smutti` manually merges the exact reviewed head. The authoring
agent does not approve or merge.

This register targets exactly the **S4 — Evidence-backed workspace docs
compatibility extension** and roadmap R8. It does not broaden S5 refresh, S6
federation, S7 impact, S8 polyglot extraction, S9 sandboxing or index
generation, server, REST, MCP, authentication, release, ontology facts, or
control-plane behavior.

The selected public lineage is exactly `codenoesis.repository-snapshot/v10`,
`codenoesis.ontology/rust/v7`, `codenoesis.local-query-result/v5`,
`codenoesis.portable-graph/v1`, `codenoesis.local-explorer/v1`, and
`codenoesis.error/v15`.

R8 does not alter historical query compatibility: V4-V6 retain
byte-identical LocalQueryResultV1, V7 retains LocalQueryResultV2, V8 retains
LocalQueryResultV3, V9 retains LocalQueryResultV4, and V10 retains
LocalQueryResultV5.

| Requirement | Current state | Target state | Product owner | Technical approver | Approval reference | Slice | Ratification material |
|---|---|---|---|---|---|---|---|
| `FR-EXP-001` | `Proposed` | `Approved` only for PortableGraphV1 export and LocalExplorerV1 generation after protected manual merge | Andrea Moretti (`@smutti` persona) | `@smutti` | [Issue #110 authorization](https://github.com/smutti/codenoesis/issues/110#issuecomment-5205875435) | `S4` | [Portable export decision](decisions/0018-s4-r8-portable-explorer-contract.md), [PortableGraphV1 schema](../../tests/specifications/s4/r8/portable-graph-v1.schema.json), [LocalExplorerV1 schema](../../tests/specifications/s4/r8/local-explorer-manifest-v1.schema.json), [machine oracle](../../tests/specifications/s4/r8/e2e_fr_exp_001_portable_explorer.json), and [golden fixture](../../tests/fixtures/s4/portable-explorer-v1/README.md) |
| Bounded R8 amendment to `FR-QRY-001` | `Approved` for exact-ID LocalQueryResultV1-V5 dispatch | `Approved` with additive read-only projection exposure; existing query contracts remain byte-identical | Andrea Moretti (`@smutti` persona) | `@smutti` | [Issue #110 authorization](https://github.com/smutti/codenoesis/issues/110#issuecomment-5205875435) | `S4` | [PortableGraphV1 schema](../../tests/specifications/s4/r8/portable-graph-v1.schema.json) and [lossless reimport matrix](../../tests/specifications/s4/r8/reimport-validation-v1.json) |
| Bounded R8 subset of `FR-QRY-002` | broader graph traversal remains `Proposed` | `Approved` only for deterministic read-only depth-1/2 traversal over one validated PortableGraphV1 | Andrea Moretti (`@smutti` persona) | `@smutti` | [Issue #110 authorization](https://github.com/smutti/codenoesis/issues/110#issuecomment-5205875435) | `S4` | [Portable export decision](decisions/0018-s4-r8-portable-explorer-contract.md) and [machine oracle](../../tests/specifications/s4/r8/e2e_fr_exp_001_portable_explorer.json) |
| Bounded R8 amendment to `FR-CLI-001` | `Approved` for `scan`, `docs`, and `query` | `Approved` with explicit additive `export` and `explore` journeys after protected manual merge | Andrea Moretti (`@smutti` persona) | `@smutti` | [Issue #110 authorization](https://github.com/smutti/codenoesis/issues/110#issuecomment-5205875435) | `S4` | [Portable export decision](decisions/0018-s4-r8-portable-explorer-contract.md) and [ErrorV15 schema](../../tests/specifications/s4/r8/codenoesis-error-v15.schema.json) |
| Bounded R8 amendment to `FR-DOC-003` | `Approved` for marker-owned generated documentation | `Approved` with the same fail-closed ownership rule for portable and explorer output roots | Andrea Moretti (`@smutti` persona) | `@smutti` | [Issue #110 authorization](https://github.com/smutti/codenoesis/issues/110#issuecomment-5205875435) | `S4` | [Portable export decision](decisions/0018-s4-r8-portable-explorer-contract.md) and [invalid/security corpus](../../tests/specifications/s4/r8/invalid-security-cases-v1.json) |
| Bounded R8 amendments to `NFR-DET-001`, `NFR-SEC-001`, and `NFR-SEC-005` | `Approved` | `Approved` with 50 export permutations, closed CSP/XSS/path limits, and zero network/process/target execution for both commands | Andrea Moretti (`@smutti` persona) | `@smutti` | [Issue #110 authorization](https://github.com/smutti/codenoesis/issues/110#issuecomment-5205875435) | `S4` | [CSP contract](../../tests/specifications/s4/r8/explorer-content-security-policy-v1.json), [invalid/security corpus](../../tests/specifications/s4/r8/invalid-security-cases-v1.json), and [reimport matrix](../../tests/specifications/s4/r8/reimport-validation-v1.json) |
| Bounded R8 subset of `NFR-PRV-002` | broader lifecycle policy remains `Proposed` | `Approved` only for explicit portable-v1 classification: redacted evidence metadata is exportable, source contents/snippets and ambient private values are excluded | Andrea Moretti (`@smutti` persona) | `@smutti` | [Issue #110 authorization](https://github.com/smutti/codenoesis/issues/110#issuecomment-5205875435) | `S4` | [PortableGraphV1 schema](../../tests/specifications/s4/r8/portable-graph-v1.schema.json), [CSP contract](../../tests/specifications/s4/r8/explorer-content-security-policy-v1.json), and [golden fixture](../../tests/fixtures/s4/portable-explorer-v1/README.md) |

`noesis export` accepts one explicit store, canonical repository identity,
marker-owned output root, and JSON format. It reads only a completely
validated visible RepositorySnapshotV10 head and atomically publishes one
`portable-graph.json` plus its exact ownership marker. Success stdout is the
same canonical PortableGraphV1 value plus LF; failure emits strict ErrorV15
on stderr with empty stdout and leaves the prior complete output unchanged.

`noesis explore` accepts one explicit canonical portable graph, marker-owned
output root, and JSON format. It validates before writing and atomically
publishes the graph, first-party `index.html`, strict
`explorer-manifest.json`, and exact ownership marker. Success stdout is
LocalExplorerV1 plus LF. Neither command mutates the repository or local
store, launches a child or browser, starts a server, opens a network channel,
executes target code, or consults ambient authority.

PortableGraphV1 binds repository identity, immutable commit/tree, V10 snapshot
ID and semantic hash, Rust ontology v7, and LocalQueryResultV5 compatibility.
It preserves exact R7 entities, relationships, claims, evidence, diagnostics,
coverage gaps, and documents plus exact document-statement bindings. Stable
IDs, reference closure, claim states, evidence lineage, diagnostics, gaps,
and redacted locator metadata are not translated or upgraded. Source contents
and snippets, absolute paths, raw compiler roots/arguments, environment
values, file URLs, and active content are absent.

The artifact is RFC8785 JSON plus one LF. Families are sorted by exact stable
ID, and repeated export plus 50 insertion-order permutations are byte
identical. Reimport preflights 268,435,456 bytes and nesting 64, rejects
duplicate members and unknown fields, validates identity/reference/evidence
closure and source binding, and compares all family counts, ordered IDs, and
canonical SHA-256 digests. It never repairs, deduplicates, drops, promotes, or
silently truncates.

LocalExplorerV1 is a non-canonical reconstructable view. The user opens the
static page manually and explicitly selects the graph. Exact-ID lookup,
case-sensitive NFC substring search, typed filters, and deterministic
breadth-first depth-1/2 traversal expose evidence, diagnostics, and gaps even
when the source repository is absent. Search displays at most 100 results;
neighborhoods display at most 256 subjects and 512 relationships. Display
truncation is explicit and never alters the projection.

The static entrypoint has no remote resource or runtime package manager. Its
reviewed CSP hashes the only inline style and script, defaults all other
capabilities to none, and denies connections, objects, frames, forms, base
navigation, manifests, media, fonts, workers, storage, cookies, dynamic code,
and telemetry. Untrusted values use `textContent` only. No browser auto-launch
or local server is permitted. Non-data viewer assets are bounded to 1,048,576
bytes.

Absent or empty destinations and exact matching marker-owned generations are
accepted. Non-empty unmarked roots, marker mismatch, absolute/parent escape,
parent or component symlink, input-output alias, race replacement, and any
write outside the selected root fail before publication. The retained corpus
covers corruption, loss, duplication, reordering, hash/reference mismatch,
unresolved evidence, script-close and quote injection, Unicode separators,
bidi/control values, oversized labels/metadata, remote origins, dynamic code,
path attacks, and every maximum-plus-one.

The governance guard was committed first at
`b8a3fde629417fb150275448f50ec9356b45ab76`. The command
`python3 -m unittest scripts.tests.test_s4_portable_explorer_contract` failed
only for the expected missing Decision 0018, with exit `1`, empty stdout, and
a 678-byte stderr log whose SHA-256 is
`784ac21ea5e0257136c3710616d463d00ee3117c0edb34dc40521aa73fe126e7`.
The retained test-first guard SHA-256 is
`42ec1167d9be5c935b50496f3703a9840601ce68cec3f669dad8ccc6aa6ff959`.
No R8 contract, production, dependency, workflow, policy, release, R7 golden,
or unrelated protected byte changed before Red.

This package adds no dependency and changes no manifest, lockfile, product
crate, workflow, policy, release asset, existing schema, fixture, or golden.
Product implementation requires a separate Ready issue with its own
executable CLI Red and retained evidence.

The strict schemas, machine oracle, CSP and invalid/security contracts,
lossless reimport matrix, retained Red, project-owned canonical fixture,
static entrypoint, and immutable R7 dependency are bound by the R8 contract
bundle. SRS and roadmap bytes are excluded from that digest. Any bound-byte
change requires a new digest and renewed semantic, privacy, and security
review.

R8 portable export and offline explorer contract bundle:
`sha256:f8bba5eda9e43825f2fe31e0c55a37641a4d9213a8d94c6854bdfa290c39ca42`.

### 2.19 S4 K1 Rust callable and value semantics candidate register

Issue [#142](https://github.com/smutti/codenoesis/issues/142), the accountable
maintainer's interactive “procediamo con K1” authorization recorded there, and
the explicit [minimal path expansion](https://github.com/smutti/codenoesis/issues/142#issuecomment-5225581916)
govern one maintainer-supervised single-PR package on exact base
`03ee09b172e84b5b7f5f423f9f65d63cf2953385`. Decision 0019, its schemas,
fixture, oracle, retained Red, implementation, and Green evidence become
effective together only when `@smutti` manually merges the exact reviewed
head. Before that merge, every K1 requirement and implementation statement is
a Proposed candidate rather than an Approved, Implemented, or Verified fact.

The package targets only **S4 — Evidence-backed workspace docs compatibility
extension**. It adds the explicit `rust-callable-semantics-v1` source profile
over the complete R6 lineage. K1 v1 does not compose with the R7 compiler-index
selector and does not broaden S5 refresh, S6 federation, S7 implementation
compatibility, S8 polyglot extraction, S9 generation/sandboxing, server,
release, model, or runtime-observation authority.

| Requirement | Current state | Target after protected merge | Owner/approver | Slice | Ratification material |
|---|---|---|---|---|---|
| `FR-EXT-012` | `Proposed` | `Approved` only for the closed K1 Rust callable/value/body-syntax profile | Andrea Moretti (`@smutti` persona) / `@smutti` | `S4` | [Decision 0019](decisions/0019-s4-k1-rust-callable-semantics-contract.md), [subset](../../tests/specifications/s4/k1/callable-semantics-subset-v1.json), [machine oracle](../../tests/specifications/s4/k1/e2e_fr_ext_012_rust_callable_semantics.json), and [fixture](../../tests/fixtures/s4/rust-callable-semantics-v1/README.md) |
| `FR-EXP-002` | `Proposed` | `Approved` only for lossless V11 `PortableGraphV2` and read-only `LocalExplorerV2` | Andrea Moretti (`@smutti` persona) / `@smutti` | `S4` | [PortableGraphV2 schema](../../tests/specifications/s4/k1/portable-graph-v2.schema.json), [LocalExplorerV2 schema](../../tests/specifications/s4/k1/local-explorer-manifest-v2.schema.json), and Decision 0019 |
| Bounded K1 amendment to `FR-DOC-001` | `Approved` for prior S4 graph versions | `Approved` with additive evidence-backed K1 sections and no raw body/expression text | Andrea Moretti (`@smutti` persona) / `@smutti` | `S4` | Decision 0019 and the K1 E2E oracle |
| Bounded K1 amendment to `FR-QRY-001` | `Approved` for LocalQueryResultV1-V5 | `Approved` with additive exact-ID LocalQueryResultV6 dispatch for every K1 family; V1-V5 remain byte-identical | Andrea Moretti (`@smutti` persona) / `@smutti` | `S4` | [LocalQueryResultV6 schema](../../tests/specifications/s4/k1/local-query-result-v6.schema.json) and the K1 E2E oracle |
| Bounded K1 amendment to `FR-CLI-001` | `Approved` for prior scan/docs/query/export/explore journeys | `Approved` with explicit K1 selectors completing the same local journey | Andrea Moretti (`@smutti` persona) / `@smutti` | `S4` | Decision 0019, ErrorV16, and the K1 E2E oracle |

K1 records complete source signatures, ordered parameters, explicit enum
discriminants and constant/static initializer metadata, normalized values only
for the closed boolean/integer/character/string literal subset, local `let`
bindings, direct and method-call syntax, and the reviewed control constructs.
Only a uniquely resolved already-known local free function creates `CALLS`.
Every method, associated, external, imported, ambiguous, generated, macro,
cfg-, compiler-, data-flow-, reachability-, side-effect-, or runtime-dependent
meaning remains schema-distinct uncertainty with exact evidence.

The additive lineage is exactly `codenoesis.configuration/v8`,
`codenoesis.ontology/rust/v8`, `codenoesis.extraction/v8`,
`codenoesis.extraction-chunk/v8`, `codenoesis.knowledge-graph/v8`,
`codenoesis.repository-snapshot/v11`, `codenoesis.local-query-result/v6`,
`codenoesis.portable-graph/v2`, `codenoesis.local-explorer/v2`, and
`codenoesis.error/v16`. Existing selectors, hash domains, schemas, stored
heads, golden fixtures, output assets, and command bytes remain unchanged.

The governance checkpoint and focused test are committed before production
Rust. Expected Red is only the absence of K1 production modules, V11 storage
registration, selector behavior, V2 export/explorer, and implementation
evidence. The project-owned fixture, exact identity formulas, family counts,
limits, invalid cases, 50 permutations, ten schedules, privacy/CSP checks,
no-execution sentinel, restart query, and R0-R8 regressions form the acceptance
oracle. No dependency is added.

The semantic checkpoint artifacts and immutable R8 dependency are bound by K1
contract bundle
`sha256:f98667ff5eb7b3aedfe83f9259c778641a843ca4bca879c9464d1f762812ea78`.
SRS, roadmap, retained Red, production source, and implementation evidence are
excluded from that semantic digest.

### 2.20 S4 R9 K1 output-capacity candidate register

Issue [#148](https://github.com/smutti/codenoesis/issues/148), the accountable
maintainer's explicit high-risk authorization, and [Decision 0020](decisions/0020-s4-r9-k1-output-capacity-contract.md)
govern one maintainer-supervised package on exact base
`aadd065defba2d4f8d202583c7da9ff70e92ece8`. The additive numeric decision and
selector remain Proposed until protected manual merge. Existing K1 behavior
and the requirement IDs amended below are already Approved and effective.

| Requirement | Current state | Target after protected merge | Slice | Acceptance material |
|---|---|---|---|---|
| Bounded R9 amendment to `FR-EXT-012` | K1 source semantics Approved and Implemented | Approved with an explicit non-semantic V11 output envelope; extraction and ontology unchanged | `S4` | Decision 0020 and the R9 output-capacity oracle |
| Bounded R9 amendment to `FR-CLI-001` | K1 scan journey Approved and Implemented | Approved with `--output-capacity-profile local-snapshot-64m-v1` only on the complete K1 scan composition | `S4` | CLI E2E and invalid-composition matrix |
| Bounded R9 amendment to `INV-BND-001` | Standard 32 MiB canonical output bound Approved | Approved with one explicit 64 MiB maximum/plus-one envelope that never changes the standard maximum | `S4` | Contract and serializer maximum/plus-one tests |
| Bounded R9 amendment to `NFR-DET-001` | K1 canonical semantics deterministic | Approved with repeated byte-identical pinned Lekton V11 output under the explicit envelope | `S4` | Two-run digest, bytes, timing, and visible-head evidence |
| Bounded R9 amendments to `NFR-TST-001/002` | Red-first deterministic evidence required | Approved with retained compile/runtime Red, focused Green, full gate, and real-repository replay | `S4` | Committed evidence pack |

The standard `canonical_output_bytes` maximum remains exactly `33,554,432`
bytes including LF. The explicit selector raises only final canonical
RepositorySnapshotV11 serialization to `67,108,864` bytes including LF. It is
valid only for `scan --profile standard-local-s4` with the complete
`rust-callable-semantics-v1` source composition. It is never inferred from
repository shape, output size, failure, or stored state.

The selector does not enter V11 semantic or configuration bytes and cannot
change extraction, identities, hashes, evidence, diagnostics, coverage,
query, documentation, PortableGraphV2, LocalExplorerV2, acquisition, file,
graph, process, network, memory, or wall-time authority. Outputs that fit the
standard maximum serialize byte-identically through both envelopes. Maximum
and maximum-plus-one are checked before local-store publication; a failure has
empty stdout and cannot create or move a visible head.

Unknown, duplicate, incomplete, non-K1, compiler-index, boundary, docs, query,
export, or explore composition uses existing ErrorV16 unsupported-composition
semantics. No schema, error version, dependency, historical fixture, golden,
identity domain, or prior command byte changes. The machine oracle is
`tests/specifications/s4/r9-output-capacity/output-capacity-profile-v1.json`.

## 3. Product intent and success definition

CodeNoesis will convert immutable software revisions into evidence-backed,
queryable knowledge snapshots. It will generate technical documentation and
explain cross-project change impact without presenting model output or graph
reachability as established fact.

The product succeeds when a user can:

1. scan a repository revision without executing untrusted project code;
2. inspect a deterministic graph whose facts resolve to source evidence;
3. generate documentation that exposes unknown or contradictory knowledge;
4. refresh that knowledge incrementally without changing the semantic result;
5. connect providers and consumers across repositories using reviewable
   identity evidence;
6. evaluate declared and implementation-derived change semantics and retrieve
   affected clients, call sites, owners, evidence paths, and coverage gaps;
7. operate the same use cases locally or through governed server interfaces;
8. keep LLM and Council use optional, bounded, and unable to manufacture facts.

Fluent prose, a high graph-edge count, or a passing unit test alone does not
demonstrate product success.

## 4. Stakeholders and actors

| Actor | Primary need |
|---|---|
| Developer | Understand one repository and the impact of a proposed change. |
| Maintainer or architect | Review evidence, architecture, contracts, and cross-project dependencies. |
| Automation agent | Consume stable JSON, REST, or MCP contracts without scraping prose. |
| Workspace administrator | Configure sources, providers, policies, roles, limits, retention, and audit. |
| Platform operator | Observe, recover, upgrade, back up, and restore the server safely. |
| Extractor author | Add a language or artifact extractor behind a versioned contract. |
| Authorized reviewer | Confirm, reject, or escalate non-deterministic candidate claims. |

## 5. Scope and release boundary

### 5.1 Local product `0.1`

The first usable release is intentionally narrow. It MUST provide a local CLI,
local Git repository acquisition, Rust inventory and extraction, a typed graph
with provenance, SQLite plus content-addressed artifacts, and deterministic
overview/module documentation. It MUST work without a model provider.

This is a sequencing decision, not a reduction of the polyglot `1.0` scope.

### 5.2 Production-grade `1.0`

Version `1.0` adds the approved language and contract adapters, incremental
refresh, cross-project federation and impact analysis, bounded queries,
PostgreSQL and object storage, durable jobs, REST and MCP, authentication and
tenant isolation, sandboxed extensions, governed Council operation, operations,
migration, restore, performance, and security gates.

### 5.3 Non-goals through `1.0`

- A graphical user interface.
- Automatic modification of source repositories or hand-written documents.
- Arbitrary build-script or target execution during a standard scan.
- LLM output, embeddings, generated prose, or RDF as the canonical store.
- Automatic ontology mutation or autonomous confirmation of inferred facts.
- Mandatory Neo4j, a vector database, NATS, or an external model provider.
- Full causal inference, private federation, or sheaf-based consistency; those
  remain research topics until separately promoted.

## 6. Product constraints

- Production code MUST use stable Rust with an exact toolchain pinned at
  workspace bootstrap. The current design target is Rust `1.97.x`, edition
  2024, and Cargo resolver 3.
- The domain MUST remain independent of Tokio, SQLx, Axum, filesystem APIs,
  MCP, and model SDKs.
- Standard analysis MUST treat repositories, archives, parser input, plugins,
  documentation, and model responses as untrusted.
- The deterministic local path MUST remain operational with networking and
  model providers disabled.
- Public contracts, ontology, configuration, extractors, evidence, and
  derivations MUST be versioned.
- Capability and resource limits MUST be explicit. The numeric defaults are a
  blocking decision for the slice that first enforces each limit.

## 7. Core invariants

| ID | Priority | Invariant | Minimum verification |
|---|---:|---|---|
| `INV-EVD-001` | P0 | Every published claim other than `unknown` MUST reference evidence that resolves in the same immutable snapshot. | `PT`, `GT`, `E2E` |
| `INV-SNP-001` | P0 | A reader MUST observe either the previous complete snapshot or the new complete snapshot, never a partial mixture. | `FT`, `IT` |
| `INV-IDN-001` | P0 | Equivalent repository content, configuration, ontology, and pipeline versions MUST produce the same semantic identifiers and hashes. | `PT`, `GT` |
| `INV-MDL-001` | P0 | An LLM or Council verdict MUST NOT create `deterministic_fact` or `confirmed` state directly. | `UT`, `PT`, `E2E` |
| `INV-INC-001` | P1 | Incremental refresh and a clean scan of the same immutable target under the same accepted versions MUST produce byte-identical canonical snapshot, graph, chunk, and generated-document semantic content. A revision-neutral cache entry MUST NOT become authoritative without complete target rematerialization and validation. | `PT`, `GT`, `E2E` |
| `INV-STO-001` | P1 | Storage adapters MUST expose the same domain-observable behaviour for the shared contract. | `CT`, `IT` |
| `INV-BND-001` | P1 | Repository processing, graph traversal, jobs, plugins, and model calls MUST terminate at configured bounds with typed outcomes. | `PT`, `SEC`, `FT` |
| `INV-TEN-001` | P1 | A workspace actor MUST NOT observe another workspace through data, search, cache, events, errors, logs, metrics, or timing-sensitive bulk operations. | `SEC`, `IT`, `PERF` |

These invariants are release blockers. Coverage exclusions or a manual waiver
cannot replace their verification.

## 8. Data and contract requirements

| ID | Pri. | Target | Normative requirement | Acceptance evidence |
|---|---:|---:|---|---|
| `DR-ART-001` | P0 | `0.1` | Public artifacts MUST contain schema, repository, configuration, pipeline, ontology, extractor, and evidence-lineage versions. | Schema tests reject missing or unknown mandatory fields. `CONF` |
| `DR-ART-002` | P0 | `0.1` | Canonical semantic content MUST be separated from volatile envelope metadata. Creation time, job ID, and correlation ID MUST NOT change the semantic hash. | Replays with different clocks and job IDs produce the same semantic hash and distinct envelopes. `PT` |
| `DR-IDN-001` | P0 | `0.1` | Stable IDs MUST derive from canonical project identity, language, symbol identity, and versioned normalization rules rather than storage sequence numbers. | Randomized insertion and file ordering do not change IDs. `PT` |
| `DR-IDN-002` | P0 | `0.1` | Rust workspace crate, source-file, module, symbol, relationship, document, and statement IDs MUST derive from canonical repository identity, manifest/target identity, module path, subject identity, and immutable versioned normalization rather than member order, output location, commit, or storage sequence. | Reordered members/files and changed envelopes/output roots retain reviewed IDs while canonical collisions fail closed. `PT`, `GT` |
| `DR-EVD-001` | P0 | `0.1` | Source evidence MUST identify repository, revision or blob, path, byte or line span, extractor, and derivation. | Invalid repository, blob, path, span, or derivation prevents publication. `GT`, `E2E` |
| `DR-SEM-001` | P1 | `0.2` | The implementation-aware HTTP/JSON compatibility projection MUST emit a closed, versioned, deterministic report that preserves contract, provider implementation, client assumption, observation, federation, rule, classification, evidence, and coverage-gap identities without conflating their claim states. | Schema, identity, reference, ordering, replay, and negative conformance tests validate `SemanticCompatibilityReportV1`; unknown fields, dangling evidence, duplicate IDs, unsorted sets, and invalid claim promotion fail closed. `CONF`, `PT`, `GT` |
| `DR-CMP-001` | P1 | `1.0` | Runtime readers MUST support the current and immediately previous public schema. The migrator MUST support a tested chain from the two most recent GA releases. | Versioned fixtures prove `N-1` reads and staged `N-2 -> N-1 -> N` migration. `CONF`, `DR` |
| `DR-DEL-001` | P1 | `1.0` | Project deletion MUST remove database references, search entries, unshared artifacts, and credentials according to a declared retention policy. | A purge verification finds no online data reachable by project or tenant identity; backup expiry is separately evidenced. `SEC`, `DR` |

## 9. Functional requirements

### 9.1 Repository acquisition and inventory

| ID | Pri. | Target | Normative requirement | Acceptance evidence |
|---|---:|---:|---|---|
| `FR-ACQ-001` | P0 | `0.1` | The system MUST bind an analysis to canonical repository identity and a verified immutable commit before extraction begins. | Moving a branch during a test does not change the bound commit; missing or inconsistent objects produce a typed failure. `IT` |
| `FR-ACQ-002` | P0 | `0.1` | Local acquisition MUST enforce allowed roots, repository size, file count, file size, path, recursion, symlink, and submodule policy. | Boundary and `limit + 1` fixtures terminate without access outside the allowed root or partial publication. `SEC`, `PT` |
| `FR-ACQ-003` | P1 | `1.0` | Remote acquisition MUST use short-lived read-only credentials confined to the acquisition process and MUST record source identity without persisting credentials. | Credential canaries do not appear in artifacts, logs, errors, or subsequent stages. `SEC`, `IT` |
| `FR-ACQ-004` | P0 | `0.1` | An explicitly selected local acquisition profile MUST read the approved repository-local SHA-1 loose and pack v2/index v2 subset in process, verify collision-aware object and container integrity, enforce fixed catalog/pack/delta/race limits, and produce storage-representation-invariant semantic input without changing any legacy invocation. | Equivalent loose/base/`OFS_DELTA`/`REF_DELTA` fixtures produce byte-identical semantic payloads; malformed, corrupt, changing, unsupported, collision, and maximum-plus-one cases produce strict typed errors with no process, network, outside-root access, automatic repair, or partial publication. `E2E`, `GT`, `SEC`, `PT`, `FZ`, `FT`, `CONF` |
| `FR-ACQ-005` | P0 | `0.1` | An explicitly selected local repository-boundary profile MUST represent each mode `160000` entry in the bound root commit as a deterministic external boundary, parse only the approved bounded committed `.gitmodules` subset without granting URL authority, continue when nested worktrees are absent, and bind only explicitly supplied depth-one nested repositories whose independently acquired commit exactly equals the gitlink OID. | Exact unbound/bound boundary projections, RepositorySnapshotV5 semantics, malformed/escaping/duplicate/orphan/credential/race cases, every maximum and maximum-plus-one, 50 permutations, parallel replay, no ambient authority, selector-absent rejection, R0/R1 and S0-S6 regressions, and a non-vendored pinned RustDesk progression pass. `E2E`, `GT`, `SEC`, `PT`, `FZ`, `FT`, `CONF` |
| `FR-INV-001` | P0 | `0.1` | Inventory MUST report supported languages, manifests, contracts, configuration, ownership, extraction capabilities, and unsupported content. | A reviewed fixture produces the exact inventory and explicit coverage gaps. `GT` |
| `FR-INV-002` | P1 | `1.0` | Inventory MUST record build target and configuration-world inputs that can affect extraction. | Two declared build profiles remain distinguishable and reproducible. `GT`, `PT` |

### 9.2 Extraction

| ID | Pri. | Target | Normative requirement | Acceptance evidence |
|---|---:|---:|---|---|
| `FR-EXT-001` | P0 | `0.1` | Every extractor MUST emit a versioned common contract containing entities, relationships, evidence spans, diagnostics, and coverage. | Invalid types, identities, relationships, spans, or evidence are rejected before graph ingestion. `CT`, `FZ` |
| `FR-EXT-002` | P0 | `0.1` | The first built-in language adapter MUST extract approved Rust modules, symbols, types, functions, imports, and relationships without executing the target. | Rust golden repositories match hand-reviewed oracles, including malformed and Unicode input. `GT`, `FZ` |
| `FR-EXT-003` | P1 | `0.3` | C, C++, Java, JavaScript, and TypeScript adapters MUST each pass the shared extractor contract and a language-specific capability matrix. | Each adapter passes golden, malformed-input, determinism, and coverage-gap tests. `CT`, `GT`, `FZ` |
| `FR-EXT-004` | P1 | `0.2` | Each separately Approved OpenAPI, AsyncAPI, GraphQL, or Protocol Buffers capability MUST extract only its declared subset into canonical service, operation, schema, field, and version identities while unsupported semantics remain typed failures or coverage gaps. | Capability-scoped contract fixtures produce exact identities and relationships; equivalent approved encodings retain source-neutral semantics and truthful source evidence. `GT`, `CONF`, `FZ` |
| `FR-EXT-005` | P1 | `0.3` | The system MUST accept optional validated SCIP artifacts and preserve their provenance and precedence over syntax-only heuristics. | Conflicting syntax and SCIP fixtures retain both evidence items and select the compiler-grade relation by policy. `GT`, `CT` |
| `FR-EXT-006` | P0 | `0.1` | A standard extraction stage MUST NOT spawn target processes or use network access. Trusted build/index execution MUST require a separate explicit profile. | Sentinel build scripts never execute in standard mode; process and network attempts are denied and audited. `SEC`, `E2E` |
| `FR-EXT-007` | P0 | `0.1` | The S4 standard profile MUST extract bounded literal-member Rust workspaces, conventional or explicit library/binary roots, and unambiguous inline/out-of-line modules from committed UTF-8 files without evaluating Cargo or target code. | A reviewed two-member workspace matches ontology v2 identities and graph counts; globs, ambiguous modules, build execution, and unsupported worlds fail or remain explicit coverage. `GT`, `SEC`, `E2E` |
| `FR-EXT-008` | P0 | `0.1` | An explicitly selected S4 workspace profile MUST extract bounded standalone, virtual, and non-virtual Cargo root-package workspaces, normalize implicit or explicit `"."` root membership exactly once, apply literal members/exclusions and conventional or explicit library/binary roots, preserve deferred Cargo meaning as coverage gaps, and keep gitlink members as external R2 boundaries without evaluating Cargo or target code. | Reviewed root-package fixtures match strict V6/ontology-v3 identities and provenance; maximum and maximum-plus-one, malformed/conflicting paths, deferred metadata, gitlink composition, permutations, docs/query, target/build sentinels, and pinned non-vendored Lekton/RustDesk pilots remain deterministic and side-effect free. `GT`, `SEC`, `E2E`, `PT`, `CONF` |
| `FR-EXT-009` | P0 | `0.1` | An explicitly selected S4 manifest profile MUST represent the approved bounded Cargo package metadata, target, dependency, target-predicate, feature, patch, workspace-reference, and build-script declarations as stable evidence-backed facts while preserving external locators as digest-only values and MUST NOT claim dependency resolution, active features/targets/cfg, patch application, generated source, path traversal, fetch, or execution. | The reviewed R4 fixture matches strict V7/ontology-v4 declaration identities, relationships, byte spans, locator digests, typed unsupported gaps, and unchanged R3 crate IDs; malformed/conflicting/escaping/Unicode inputs, every maximum and maximum-plus-one, permutations, selector-absent bytes, no-authority sentinels, docs/query, and pinned non-vendored Lekton/RustDesk pilots pass. `GT`, `SEC`, `E2E`, `PT`, `CONF` |
| `FR-EXT-010` | P0 | `0.1` | An explicitly selected S4 Rust semantic-depth profile MUST represent the approved bounded committed declaration subset for fields, enum variants, constants, statics, associated types, trait required/default methods, inherent methods, and named local trait-implementation methods with collision-safe owner/trait context, evidence-backed outer-attribute uncertainty, and no target execution or interpretation of cfg, macros, types, values, calls, framework roles, or runtime behavior. | The reviewed R5 fixture matches strict V8/ontology-v5 member identities, owner relationships, claims, byte evidence, compilation-presence states, diagnostics, coverage gaps, docs, and LocalQueryResultV3; malformed, Unicode, normalization-collision, ambiguous implementation, hard-negative, every maximum and maximum-plus-one, 50 permutations, selector-absent V1/V2 compatibility, no-authority sentinels, and pinned non-vendored Lekton/RustDesk pilots pass. `GT`, `SEC`, `E2E`, `PT`, `CONF` |
| `FR-EXT-011` | P0 | `0.1` | An explicitly selected S4 framework-declarations profile MUST represent only the approved bounded framework-neutral component, service, configuration, endpoint, route, and handler source declarations or unresolved candidates with exact committed evidence, disjoint NFC identities, inherited compilation presence, and unique-local-target binding, while keeping declaration syntax, macro/attribute candidate meaning, and unsupported runtime behavior schema-distinct and MUST NOT execute or infer cfg, macros, generated code, compiler facts, reachability, serving, startup, active configuration, handler execution, or any equivalent runtime relation. | The reviewed two-style R6 fixture matches strict V9/ontology-v6 entities, `DEFINES` ownership, claims, byte spans, candidate diagnostics/gaps, non-runtime docs, and LocalQueryResultV4; comments, strings, docs, imports, names, unused builders, generated/target/build sentinels, malformed/Unicode/NFC-collision/ambiguity/path/privacy cases, every maximum and maximum-plus-one, 50 permutations plus isolated replay, selector-absent V3/V2/V1 bytes, zero execution/network/model authority, and pinned non-vendored motivation-only Lekton/RustDesk pilots pass. `GT`, `SEC`, `E2E`, `PT`, `CONF` |
| `FR-EXT-012` | P0 | `0.1` | The explicit K1 `rust-callable-semantics-v1` profile MUST represent complete reviewed Rust callable signatures, ordered parameters, explicit declared-value metadata, the closed normalized scalar subset, local bindings, direct/method call syntax, syntactic control structure, lexical nesting, and exact committed evidence. It MUST emit `CALLS` only for one uniquely proven already-known local free-function target and MUST keep every compiler-, macro-, cfg-, dispatch-, type-, CFG-, reachability-, data-flow-, side-effect-, ownership-, and runtime-dependent meaning unresolved without executing target or toolchain code. | The project-owned K1 fixture matches RepositorySnapshotV11/KnowledgeGraphV8 identities, counts, values, body digests/spans, four unique-local calls, five unresolved candidates, all eleven control kinds, exact query/docs/export/explorer behavior, invalid/limit/privacy cases, 50 permutations, ten schedules, and immutable selector-absent R0-R8 bytes. `GT`, `E2E`, `CONF`, `PT`, `SEC` |

### 9.3 Knowledge graph, claims, and snapshots

| ID | Pri. | Target | Normative requirement | Acceptance evidence |
|---|---:|---:|---|---|
| `FR-KNW-001` | P0 | `0.1` | The canonical graph MUST validate approved entity types, relationship types, required properties, cardinalities, and claim-state transitions. | Invalid nodes, edges, cardinalities, and transitions fail with stable domain errors. `UT`, `PT` |
| `FR-KNW-002` | P0 | `0.1` | Deterministic parser facts, deterministic derived facts, heuristic candidates, reviewed inferences, confirmed facts, rejected claims, and superseded claims MUST remain distinguishable. | A state-machine model test rejects every forbidden transition. `PT` |
| `FR-KNW-003` | P0 | `0.1` | Deterministic inference MUST use versioned Rust rules and retain every input evidence and rule version. | Replaying a rule explains the same output and invalidation dependency set. `UT`, `PT` |
| `FR-SNP-001` | P0 | `0.1` | Snapshot publication MUST durably stage all referenced content before atomically advancing the visible project head. | Failpoints at every publication boundary expose either the old or one complete new snapshot; orphan cleanup cannot remove reachable content. `FT`, `IT` |
| `FR-STO-001` | P0 | `0.1` | The local profile MUST persist graph and metadata in SQLite and immutable artifacts in a content-addressed filesystem. | Restart preserves the head; missing or corrupted content is detected by its hash. `CT`, `IT` |
| `FR-STO-002` | P1 | `0.4` | The server profile MUST implement the same storage contract using PostgreSQL and S3-compatible object storage. | Fake, SQLite/PostgreSQL, and filesystem/S3 adapters pass the applicable shared contract suites. `CT`, `IT` |

### 9.4 Documentation

| ID | Pri. | Target | Normative requirement | Acceptance evidence |
|---|---:|---:|---|---|
| `FR-DOC-001` | P0 | `0.1` | The local product MUST generate deterministic overview and module documentation from validated claims and evidence. | Repeated generation is byte-identical apart from explicitly non-canonical envelope data. `GT`, `E2E` |
| `FR-DOC-002` | P0 | `0.1` | Every material generated statement MUST resolve to valid evidence or be rendered as unknown, contradictory, or unsupported. | Deleting or corrupting evidence blocks the claim or changes its explicit state; no unsupported prose remains. `PT`, `E2E` |
| `FR-DOC-003` | P0 | `0.1` | Generated output MUST be confined to a configured generated-document location and MUST NOT overwrite hand-written documentation. | Checksums of manual files remain unchanged after generation and failure recovery. `E2E`, `SEC` |
| `FR-DOC-004` | P1 | `1.0` | Approved views MUST cover architecture, modules, APIs/events, data, configuration, integrations, deployment, operations, onboarding, and change impact when sufficient evidence exists. | A view capability matrix maps every emitted section to evidence and reports missing views without fabrication. `GT`, `E2E` |

### 9.5 Incremental refresh, federation, and impact

| ID | Pri. | Target | Normative requirement | Acceptance evidence |
|---|---:|---:|---|---|
| `FR-INC-001` | P1 | `0.2` | Local incremental refresh MUST compare the validated visible S4 head with one independently bound immutable target, derive exact sorted invalidation and analysis-reuse sets through `codenoesis.incremental-rules/rust-workspace-v1`, enforce fixed bounds, and publish no partial target. | The reviewed one-file edit selects exact partial analysis, cache, public rematerialization, invalidation, failure, and atomic-head outcomes. `GT`, `PT`, `FT`, `E2E` |
| `FR-INC-002` | P1 | `0.2` | Extractor, workspace mapper, normalization, ontology, extraction contract, semantic profile, dependency rule, cache schema, snapshot schema, or public schema changes MUST trigger the exact classified rebuild scope and MUST NOT reuse incompatible bytes. | Maximum-compatible version matrices select the documented full rebuild reason; topology and ambiguous mapping cases conservatively select full-workspace analysis. `PT`, `E2E` |
| `FR-INC-003` | P1 | `0.2` | Incremental and cold results MUST satisfy `INV-INC-001`. Exact compatible revision-neutral analysis entries SHOULD retain cache identities and payload hashes, while every commit-bound public target artifact MUST be rematerialized and unaffected stable public identities retain IDs only where the accepted S4 identity contract permits. | Random edit/order/schedule sequences prove parser reuse separately from public rematerialization and compare canonical target bytes with a clean oracle. `PT`, `GT` |
| `FR-FED-001` | P1 | `0.2` | Cross-project identity MUST apply the approved authority order: explicit identity, package/SCIP identity, operation identity, event/schema identity, then heuristic candidate. | Provider, two clients, and plausible decoys produce the reviewed identity outcomes and evidence. `GT` |
| `FR-FED-002` | P1 | `0.2` | Heuristic or conflicting matches MUST remain candidates until deterministic evidence, authorized human review, or governed policy confirms them. | A high-scoring heuristic cannot silently become `confirmed`. `PT`, `E2E` |
| `FR-IMP-001` | P1 | `0.2` | Impact analysis MUST compare two provider revisions and classify approved contract changes as compatible, potentially breaking, breaking, or unresolved. | Versioned compatible and breaking fixtures match a ratified classifier oracle. `GT` |
| `FR-IMP-002` | P1 | `0.2` | An impact report MUST contain bounded affected repositories, call sites, owners, deployment units, evidence paths, claim states, and coverage gaps. | Known clients are returned, decoys are rejected, and every hop resolves to evidence. `GT`, `E2E` |
| `FR-IMP-003` | P1 | `0.2` | The system MUST distinguish declared dependency, graph reachability, inferred exposure, observed impact, and unresolved impact. | Test fixtures cannot promote a reachable-only client to observed or causally supported impact. `PT`, `GT` |
| `FR-IMP-004` | P1 | `0.2` | For each approved HTTP/JSON field capability, the system MUST reconcile declared contract semantics, provider implementation semantics, and deterministically linked client assumptions as separate evidence-backed views, including request/response direction and distinct presence, nullability, default, validation, value-set, status, and error dimensions. Unsupported control flow, framework behavior, codec behavior, configuration, or federation MUST remain `unresolved` with explicit coverage gaps. | A reviewed provider plus strict client, safe client, and operation decoy produces the exact three-view facts, latent mismatch, safe handling, decoy rejection, evidence spans, and custom-mapping gap without target, build, network, plugin, or model execution. `GT`, `E2E`, `SEC` |
| `FR-IMP-005` | P1 | `0.2` | The system MUST compare two immutable provider revisions and emit approved semantic implementation deltas even when declared contract bytes are unchanged. It MUST classify and propagate impact only through versioned rules to deterministically linked client paths whose proven assumptions are violated; safe clients MUST remain unaffected and insufficient evidence MUST remain `unresolved`. | Byte-identical OpenAPI revisions plus `guaranteed_present -> may_be_absent` provider source produce a `breaking` strict-client result, a `compatible` safe-client result, a rejected decoy, and an unresolved custom mapping matching the ratified report oracle. `GT`, `PT`, `E2E` |

### 9.6 Query and public interfaces

| ID | Pri. | Target | Normative requirement | Acceptance evidence |
|---|---:|---:|---|---|
| `FR-EXP-001` | P0 | `0.2` | The local CLI MUST export one validated visible RepositorySnapshotV10 as deterministic `PortableGraphV1` and MUST generate a read-only offline `LocalExplorerV1` without identity, reference, claim-state, evidence-lineage, diagnostic, coverage-gap, document, or statement loss. Reimport MUST fail closed on unsupported schema, non-canonical order, duplication, loss, ambiguity, hash/reference mismatch, unresolved evidence, unsafe path, or resource excess; source contents and snippets MUST be absent from portable v1. | Black-box export/explore, schema, 50-permutation, lossless reimport, CSP/XSS/privacy/path/symlink, corruption, maximum-plus-one, atomic publication, and R7 compatibility suites match Decision 0018 and the canonical project-owned fixture. `E2E`, `CONF`, `PT`, `SEC`, `FT` |
| `FR-EXP-002` | P0 | `0.2` | The explicit K1 export profile MUST project one validated visible RepositorySnapshotV11 and its deterministic documentation as canonical `PortableGraphV2`, then generate read-only offline `LocalExplorerV2`, preserving every K1 identity, relationship, claim state, evidence locator, diagnostic, coverage gap, document, and statement without raw body text, arbitrary initializer text, source contents, or snippets. Reimport MUST fail closed rather than repair, infer, deduplicate, or truncate. | The K1 black-box journey, exact family digests, lossless reimport, duplicate/reference/hash/order rejection, CSP/XSS/privacy/path/race/limit cases, 50 permutations, and selector-absent PortableGraphV1/LocalExplorerV1 regressions match Decision 0019. `E2E`, `CONF`, `PT`, `SEC`, `FT` |
| `FR-QRY-001` | P0 | `0.1` | Local queries MUST retrieve every approved entity, relationship, claim, evidence, diagnostic, coverage gap, and document by stable identity, expose unknown or contradictory states, and select a versioned result contract from the validated snapshot without changing prior result versions. R8 MAY expose the same exact subjects in a validated read-only portable projection. K1 adds LocalQueryResultV6 for V11 and directly linked callable/value/body records, while LocalQueryResultV1-V5 bytes and authority remain unchanged. | CLI black-box scenarios return the reviewed typed result and stable exit status; V11 exercises all K1 subject families after restart, V10 exercises all seven LocalQueryResultV5 kinds, V4-V9 retain approved dispatch/bytes, and portable family digests prove exact preservation. `E2E`, `CONF` |
| `FR-QRY-002` | P1 | `0.2` | Graph traversal MUST enforce configurable depth, result, time, and resource limits with cycle handling. The bounded R8 subset is deterministic breadth-first read-only traversal over one validated PortableGraphV1 with default depth 1, maximum depth 2, at most 256 subjects and 512 relationships, and explicit display truncation. Broader canonical/server traversal remains Proposed. | Cyclic and adversarial queries terminate within the configured bound without starving unrelated work; R8 permutation and maximum-plus-one cases return identical bounded neighborhoods or typed failures without mutating the graph. `PT`, `PERF`, `SEC` |
| `FR-CLI-001` | P0 | `0.1` | The CLI MUST provide local `scan`, `docs`, `query`, `export`, and `explore` journeys with human-readable and versioned JSON output. R8 and K1 behavior MUST be explicitly selected, preserve accepted command bytes, and use marker-owned atomic output roots. | The R8 and K1 black-box fixtures each complete scan -> docs -> query -> export -> explore without network, child process, browser auto-launch, target/toolchain execution, repository mutation, or unintended store mutation. `E2E`, `CONF`, `SEC`, `FT` |
| `FR-CLI-002` | P1 | `1.0` | Approved CLI commands MUST have stable exit codes, error codes, configuration precedence, and local/remote capability behaviour. | Golden compatibility tests cover output schema, error catalog, precedence, and server parity. `CONF`, `E2E` |
| `FR-CLI-003` | P0 | `0.1` | The S0 CLI MUST provide a local `noesis scan` JSON journey that accepts an explicit repository identity and revision and emits either `RepositorySnapshotV1` or `CodeNoesisErrorV1` with the ratified S0 stream and exit semantics. | S0 black-box tests validate success, non-Git input, missing and inconsistent objects, schema, stdout/stderr separation, and exit status. `E2E`, `CONF` |
| `FR-CLI-004` | P1 | `0.2` | The local CLI MUST provide `noesis refresh` with explicit repository, repository identity, revision, store, and `standard-local-s5` profile inputs; success emits strict canonical `IncrementalRefreshReportV1` on stdout and failure emits strict `CodeNoesisErrorV7` on stderr without a partial head. | Black-box and conformance tests validate exact command parsing, streams, exits, report/error schemas, no-change retry, concurrent-head failure, limits, and S4 cold equivalence. `E2E`, `CONF`, `FT` |
| `FR-CLI-005` | P1 | `0.2` | The local CLI MUST provide output-only `noesis federate` with one explicit workspace manifest, `standard-local-s6`, and JSON output; success MUST buffer and validate one canonical `FederationReportV1` before a single stdout write, while failure MUST emit one strict `CodeNoesisErrorV8` on stderr with no stdout, store, or partial artifact. | Black-box and conformance tests validate exact parsing, configuration authority, streams, exits, schemas, report size, no partial output, no persistent mutation, and unchanged S0–S5 commands. `E2E`, `CONF`, `SEC`, `FT` |
| `FR-API-001` | P1 | `0.4` | The REST API MUST expose approved `/api/v1` resources using versioned schemas, RFC 9457 Problem Details, correlation IDs, and idempotency keys. | Contract tests validate success, error, retry, and duplicate-submission cases. `CONF`, `E2E` |
| `FR-API-002` | P1 | `0.4` | Long-running REST operations MUST return a durable job identity and MUST expose bounded status and event retrieval. | Submit, disconnect, reconnect, retry, and event-resume scenarios retain one logical job. `E2E`, `FT` |
| `FR-MCP-001` | P1 | `0.4` | MCP tools and resources MUST invoke the same application use cases and authorization policies as CLI/REST. | Transport conformance proves semantic parity and absence of transport business logic. `CT`, `E2E` |
| `FR-MCP-002` | P1 | `0.4` | Long-running MCP tools MUST return `JobId`; stdio and Streamable HTTP capabilities MUST be explicitly declared. | Client conformance tests validate negotiation, schemas, errors, and job retrieval. `CONF` |

### 9.7 Jobs, extensions, authentication, and Council

| ID | Pri. | Target | Normative requirement | Acceptance evidence |
|---|---:|---:|---|---|
| `FR-JOB-001` | P1 | `0.4` | Server jobs MUST implement durable state, leases, heartbeats, classified retry, idempotent stages, cancellation semantics, and an outbox. | Crash, expiry, duplicate delivery, cancellation, and retry publish at most one complete snapshot and retain an observable history. `PT`, `FT` |
| `FR-PLG-001` | P1 | `0.3` | Portable extractors MUST use a versioned WIT contract and default-deny WebAssembly capabilities. | Network, write, path escape, clock, and randomness attempts fail unless individually granted. `SEC`, `CT` |
| `FR-PLG-002` | P1 | `0.3` | Plugin fuel, memory, wall time, output size, trap, and malformed output MUST be bounded and isolated from snapshot integrity. | Valid, timeout, trap, OOM, oversized, and invalid-output fixtures produce typed outcomes without corrupting the head. `SEC`, `FT` |
| `FR-AUT-001` | P1 | `0.4` | The server MUST validate OIDC issuer, audience, expiry, signature, and key rotation and MUST support short-lived service identities. | Token matrix covers invalid issuer/audience/expiry/signature, JWKS rotation, revocation, and service expiry. `SEC`, `CONF` |
| `FR-AUT-002` | P1 | `0.4` | Every protected action MUST apply a versioned role-action-resource policy with default deny and append an auditable outcome. | Generated tests cover every cell of the approved authorization matrix, including cross-workspace denial. `SEC`, `PT` |
| `FR-COU-001` | P1 | `0.5` | Council invocation MUST be selective, policy-driven, and bounded by seat, round, time, token, and monetary limits. | Limit boundaries and `limit + 1` terminate with explicit unresolved or human-review outcomes. `PT`, `E2E` |
| `FR-COU-002` | P1 | `0.5` | First-round seats MUST assess immutable evidence independently; every cited evidence identifier MUST be machine-validated. | Fabricated citations invalidate a verdict and correlated identical context is visible in evaluation output. `GT`, `PT` |
| `FR-COU-003` | P1 | `0.5` | Missing quorum, critical supported dissent, or policy uncertainty MUST produce `needs_human` or unresolved state. | Agreement, dissent, missing-seat, timeout, and outage fixtures match the decision table. `GT` |
| `FR-COU-004` | P1 | `0.5` | Council MUST run in shadow mode until a ratified calibration gate proves value over a deterministic baseline and a single strong verifier. | Promotion evidence includes calibration, selective risk, cost, dissent recall, and error-correlation evaluation. `PERF`, `GT` |

## 10. Non-functional requirements

### 10.1 Correctness, reliability, and performance

| ID | Pri. | Normative requirement | Acceptance evidence |
|---|---:|---|---|
| `NFR-DET-001` | P0 | Process scheduling, insertion order, clock, and job envelope MUST NOT alter canonical output. | At least 50 isolated randomized replays plus property tests produce identical canonical artifacts. `PT`, `GT` |
| `NFR-REL-001` | P0 | A failed or interrupted operation MUST NOT expose a partial snapshot or destroy the last valid head. | Failpoint coverage at every publication transition. `FT` |
| `NFR-REL-002` | P1 | At-least-once job execution MUST be safe under retry and duplicate delivery. | The same logical job delivered 100 times produces one visible semantic snapshot. `PT`, `FT` |
| `NFR-PER-001` | P1 | Every performance claim MUST name corpus version, host, concurrency, cache state, enabled extractors, repetitions, percentile method, and success rate. | Benchmark manifest validation rejects incomplete reports. `PERF`, `CONF` |
| `NFR-PER-002` | P1 | Version `1.0` MUST satisfy the ratified SLO table in [architecture.md](architecture.md#service-level-objectives) on the published reference corpus. | CI or release benchmark report meets every contractual threshold with no hidden failed samples. `PERF` |
| `NFR-DR-001` | P1 | Server releases MUST demonstrate PostgreSQL/object/index backup consistency, RPO, RTO, and restore correctness before GA and quarterly thereafter. | A restore drill validates snapshot heads, referenced objects, search reconstruction, and audit continuity. `DR` |

### 10.2 Security and privacy

| ID | Pri. | Normative requirement | Acceptance evidence |
|---|---:|---|---|
| `NFR-SEC-001` | P0 | Standard local analysis MUST have zero target process execution, zero analysis-stage network access, and zero filesystem access outside allowed roots. R8 export/explore additionally MUST NOT start a server, auto-launch a browser, load a remote resource, interpret graph data as active content, mutate repository/store state, or write outside an exact marker-owned destination. | A malicious-repository and portable-projection corpus includes traversal, symlink loops, archive/repository bombs, oversized input, parser attacks, XSS/CSP payloads, remote origins, path races, and sentinel scripts. `SEC`, `FZ` |
| `NFR-SEC-002` | P1 | Server authorization and storage MUST satisfy `INV-TEN-001` across database, objects, FTS, caches, jobs, events, logs, metrics, and model requests. | Randomized multi-tenant operations and explicit attack cases find no cross-tenant data. `SEC`, `PERF` |
| `NFR-SEC-003` | P1 | Secrets MUST use an external secret manager, MUST be redacted from observable output, and MUST be scoped to the stage that requires them. | Canary secrets never appear in persisted artifacts, logs, traces, metrics, errors, or model payloads. `SEC` |
| `NFR-SEC-004` | P1 | A release MUST have no known exploitable Critical vulnerability. High-risk exceptions require owner, expiry, rationale, and compensating control. | Dependency, container, binary, and configuration reports plus the exception register are release artifacts. `SEC`, `CONF` |
| `NFR-SEC-005` | P0 | From S0 `noesis` process start until exit, a standard local scan MUST launch no child process and MUST have no direct or brokered network channel. The same zero-child/zero-network boundary applies to R8 `export` and `explore`, including no browser auto-launch or local server. Fixture setup, manual opening of the generated static page after command exit, and the test harness are outside this monitored boundary. | A Linux black-box run combines an empty network namespace, non-socket-only inherited standard descriptors, and the ratified deny-and-audit seccomp policy for process, socket/network, and `io_uring` paths. Generated probes cover every policy syscall and conditional branch on the selected architecture; missing, unexpectedly allowed, or unproved `not_exposed` results fail. R8 repeats the command boundary for both additive journeys. `SEC`, `E2E` |
| `NFR-PRV-001` | P0 | Source, evidence, and derived knowledge MUST NOT leave the local system or configured workspace unless an authorized user explicitly enables an allowlisted destination. | Network capture in default/off mode records zero external content-bearing calls. `SEC`, `E2E` |
| `NFR-PRV-002` | P1 | Data classification, retention, export, deletion, legal hold, residency, and backup expiry MUST be explicit per deployment policy. The approved portable-v1 profile classifies already-redacted evidence metadata as exportable and excludes source contents, snippets, absolute paths, raw tool roots/arguments, environment values, telemetry, and ambient private data. Broader lifecycle policy remains Proposed. | Lifecycle conformance tests exercise creation through purge and backup expiration; R8 privacy goldens and canaries prove only the approved metadata crosses the explicit output boundary. `SEC`, `DR` |

### 10.3 Operability, compatibility, and maintainability

| ID | Pri. | Normative requirement | Acceptance evidence |
|---|---:|---|---|
| `NFR-OBS-001` | P1 | Every request and job MUST propagate a correlation ID and emit stage, duration, outcome, coverage, limit, queue, sandbox, Council, token, and cost signals without source leakage. | Trace and metric contract tests cover success, failure, retry, and cancellation. `CT`, `IT` |
| `NFR-OPS-001` | P1 | Server processes MUST expose liveness, readiness, startup, and graceful-drain behaviour with documented alerts and runbooks. | Dependency loss, shutdown, queue drain, and stuck-worker scenarios produce approved health transitions and alerts. `FT`, `E2E` |
| `NFR-CMP-001` | P1 | Public JSON, REST, MCP, WIT, configuration, ontology, and artifact changes MUST be classified for compatibility and versioned before merge. | Compatibility tests block an unapproved breaking fixture. `CONF` |
| `NFR-PORT-001` | P1 | The supported CLI and server platform matrix MUST state the guarantee available on each OS/architecture rather than implying identical sandbox capabilities. | Release tests run on every supported tier and verify its declared capability set. `CONF`, `SEC` |
| `NFR-MNT-001` | P0 | First-party crate dependencies MUST follow the approved inward dependency rules and first-party `unsafe` MUST remain forbidden. | An architecture fitness test rejects forbidden dependency edges, missing lint inheritance, and any first-party allowance of `unsafe`. `CONF` |
| `NFR-MNT-002` | P1 | Transitive dependency `unsafe` use MUST be inventoried; every accepted exception MUST record package identity, scope, rationale, owner, review evidence, and expiry. | The supply-chain gate rejects an unregistered or expired exception and publishes the reviewed inventory. `CONF`, `SEC` |
| `NFR-SUP-001` | P1 | Releases MUST produce a locked dependency graph, license/advisory evidence, SBOM, signed artifacts, and verifiable build provenance. | Consumers can verify artifact identity, signature, SBOM association, and provenance against source. `CONF`, `SEC` |
| `NFR-TST-001` | P0 | Every Approved behavioural requirement MUST have a failing acceptance test before production implementation; every other Approved requirement MUST have an appropriate failing executable check. All requirements MUST retain a requirement-test-evidence link. | CI rejects an Implemented or Verified requirement with missing or non-green required evidence. `CONF` |
| `NFR-TST-002` | P0 | Required tests MUST be deterministic and parallel-safe. A retry MUST NOT convert a flaky test into acceptable release evidence. | Repeated shuffled execution detects no order dependency; quarantine requires an issue and cannot cover a release-blocking requirement. `PT`, `CONF` |

## 11. Test-driven development policy

### 11.1 Outside-in loop

Every behavioural requirement is delivered through this outside-in loop.
Constraints and process requirements use the same test-first discipline with
the appropriate conformance, security, performance, or recovery check when a
public black-box scenario is not meaningful.

1. **Specify:** approve an atomic requirement, its oracle, failure behaviour,
   and one public acceptance scenario.
2. **Red:** implement the acceptance test through the narrowest public surface
   available and record that it fails for the expected reason.
3. **Drive inward:** add the smallest domain unit/property tests needed to
   implement the next behaviour.
4. **Green:** write the minimum production code that satisfies the behaviour.
5. **Refactor:** improve names and boundaries while all tests remain green.
6. **Contract:** apply the same port suite first to a controllable fake and then
   to the real adapter.
7. **Break it deliberately:** add invalid-input, limit, retry, crash, security,
   and observability cases proportional to the risk.
8. **Demonstrate:** run the black-box scenario on a versioned fixture and retain
   machine-readable evidence.
9. **Trace:** update requirement status and its test/evidence links in the same
   change.

Writing a unit test after the implementation does not satisfy this policy.

### 11.2 Test-double policy

- Fakes model port capabilities and domain outcomes, not internal call order.
- Clock, ID generation, scheduling, and fault injection are controllable ports.
- Mocks are reserved for true external boundaries such as remote Git, OIDC,
  model providers, and object-service protocols.
- Real adapters must pass the same contract suite as their fake where the
  behaviour is shared.
- Tests must not assert private implementation details that prevent safe
  refactoring.

### 11.3 Test layers

| Layer | Primary purpose |
|---|---|
| Domain unit/property | Invariants, identities, normalization, state transitions, graph rules. |
| Artifact/schema | Serialization, hashing, current/previous compatibility, invalid data. |
| Port contract | Behavioural parity across fake/real and local/server adapters. |
| Extractor golden/differential | Language semantics, diagnostics, coverage, malformed input, SCIP precedence. |
| Component | Real repository, parser, database, CAS, and plugin adapters in isolation. |
| Acceptance | Public CLI, REST, or MCP journey tied to a requirement. |
| Failure/security | Crash points, retry, duplicate jobs, malicious repositories, tenant isolation, provider outage. |
| Non-functional | Fuzz, mutation, load, performance, migration, restore, and platform conformance. |

Coverage is a diagnostic, not proof of correctness. The initial project
target is at least 80% line coverage across first-party workspace code and 90%
for domain/application crates, excluding generated code with an explicit rule.
Critical invariants additionally require property tests and mutation analysis;
no non-equivalent surviving mutant is acceptable in their decision logic.

## 12. Incremental delivery plan

Each slice MUST be independently demonstrable, reviewable, and reversible. A
slice expected to exceed ten working days for one small team MUST be split by
behaviour, not by horizontal technical layer. Planned crate boundaries are
introduced only when a slice needs their responsibility; the complete future
crate tree is not scaffolded upfront.

| Slice | Demonstrable outcome | Primary requirements | First black-box test | Exit gate |
|---|---|---|---|---|
| `S0` Walking skeleton | `noesis scan` of a one-file local Git fixture emits a versioned `RepositorySnapshotV1` JSON envelope. | `DR-ART-001/002`, `FR-ACQ-001`, `FR-CLI-003`, `NFR-DET-001`, `NFR-MNT-001`, `NFR-SEC-005`, `NFR-TST-*` | Assert commit, schema, semantic hash, typed acquisition failures, and CLI exit semantics. | Pinned toolchain and CI; the same semantic input is canonical-payload-byte-identical while volatile envelopes may differ; no analysis network/process execution. |
| `S1` Safe inventory | Snapshot contains supported files, language and manifest inventory, evidence, diagnostics, and coverage gaps. | `FR-ACQ-002`, `FR-INV-001`, `DR-EVD-001`, `NFR-SEC-001` | Scan a reviewed repository plus traversal, symlink, oversized, and sentinel-script fixtures. | All evidence resolves; every limit has a typed boundary case; nothing escapes the root. |
| `S2` Rust knowledge | A Rust fixture produces reviewed entities and relations in a validated graph. | `FR-EXT-001/002`, `FR-KNW-001/002/003`, `DR-IDN-001` | Compare a Rust repository to a hand-authored graph oracle. | Stable IDs; malformed/Unicode coverage; invariants property-tested and fuzz target seeded. |
| `S3` Atomic local storage | SQLite/CAS persists and publishes one immutable snapshot across restart and faults. | `FR-STO-001`, `FR-SNP-001`, `INV-SNP-001` | Scan, kill at each failpoint, restart, and query the visible head. | Fake/SQLite and fake/filesystem contracts green; no partial head; retry idempotent. |
| `S4` Evidence-backed docs | `noesis docs` creates deterministic overview/module views without touching manual files. | `FR-DOC-001/002/003`, `FR-QRY-001`, `FR-CLI-001` | Complete scan -> docs -> query through the CLI. | Every statement resolves or exposes uncertainty; output deterministic; manual checksums unchanged. |
| `S5` Incremental refresh | One mapped non-root Rust source edit reparses only that source, reuses exact revision-neutral analysis, rematerializes every commit-bound public target artifact, and equals a cold S4 target. | `FR-INC-*`, `FR-CLI-004`, `INV-INC-001` | Refresh the reviewed A→B fixture, observe parser/cache activity, and compare target snapshot plus documents with a clean target store. | Canonical target bytes equal; exact invalidation and cache sets emitted; all target evidence is commit-bound; atomic head and fixed failures proven; no target execution. |
| `S6` Contract federation | Output-only federation of one bounded OpenAPI 3.1.0 provider, two explicit clients, one operation decoy, and one heuristic candidate emits a deterministic source-neutral report. | `FR-EXT-004`, `FR-FED-*`, `FR-CLI-005` | Run `noesis federate` over the reviewed JSON/YAML provider and client catalog. | Two exact links confirmed; decoy rejected; heuristic remains candidate with a gap; JSON/YAML semantics agree; strict streams, limits, hostile-input failures, and no ambient authority proven. |
| `S7` Change impact | A semantic API diff compares declared contract and approved provider implementation facts, then returns bounded affected client paths, evidence, and gaps. | `DR-SEM-001`, `FR-IMP-*`, `FR-QRY-002` | Compare two provider revisions with unchanged contract bytes, one strict client, one safe client, and one operation decoy. | Ratified direction-aware classifier matches the three-view oracle; implementation-only change remains visible; safe client and decoy are not mislabeled; unknown behavior remains a gap. |
| `S8` Polyglot adapters | Add Java, JavaScript/TypeScript, and C/C++ one adapter at a time. | `FR-EXT-003/005` | Run the shared semantic capability fixture for each adapter. | Contract, golden, malformed, determinism, and differential suites green per language. |
| `S9` Sandboxed extensions | A WIT extractor runs with explicit capabilities and contained failure. | `FR-PLG-*`, `INV-BND-001` | Run valid, network, write, trap, timeout, OOM, and oversized plugins. | Every resource bound enforced; failure cannot change the published head. |
| `S10` Durable server path | REST -> job -> worker -> PostgreSQL/object storage produces the same semantic snapshot as local mode. | `FR-STO-002`, `FR-JOB-001`, `FR-API-*`, `INV-STO-001` | Submit, observe, duplicate, interrupt, and recover one scan. | Shared storage contracts; lease/outbox/failpoint tests; local/server semantic parity. |
| `S11` MCP parity | MCP invokes the approved application use cases over stdio and Streamable HTTP. | `FR-MCP-*`, `NFR-CMP-001` | Conformance journey for scan, query, and impact. | No transport business logic; schemas/errors stable; long jobs return `JobId`. |
| `S12` Tenant security | OIDC, RBAC, storage, search, events, logs, and metrics isolate workspaces. | `FR-AUT-*`, `INV-TEN-001`, `NFR-SEC-002/003` | Execute the role matrix and randomized cross-tenant attacks. | Default deny, no leakage, complete audit evidence, noisy-neighbour target ratified. |
| `S13` Council shadow | Council evaluates candidates without changing deterministic truth. | `FR-COU-*`, `INV-MDL-001`, `NFR-PRV-001` | Agreement, dissent, false citation, limit, and provider-outage scenarios. | Shadow only; citations checked; budgets enforced; baseline comparison published. |
| `S14` Hardening and pilot | Signed release candidate passes corpus, fault, upgrade, restore, load, and pilot gates. | All `P1`, especially `NFR-PER-*`, `NFR-DR-001`, `NFR-SUP-001` | Execute the release acceptance suite on the exact signed artifacts. | SLO/DR/security/compatibility gates green; no unaccepted Critical/High; pilot report approved. |

The `S1` row records the already Implemented base slice and is not reopened by
`FR-ACQ-004` or `FR-ACQ-005`. The implemented R0/R1 behavior is an additive
packed SHA-1 compatibility extension with its own accepted oracle. R2 gitlink
boundaries and R3 root-package workspaces are additive implemented profiles
with V5 and V6 lineages and remain not Verified. R4 is an additive implemented
S4 compatibility profile with a V7 declaration-only lineage and remains not
Verified. R5 is the separate implemented V8 declaration-depth profile and also
remains not Verified. R6 is the implemented V9 source-only framework profile,
and R7 is the implemented V10 revision-bound SCIP-import profile; both remain
not Verified. R8 is governance-only in this revision: it approves no product
code, dependency, migration, browser launch, server, source snippet, or
ontology change before a separate Ready implementation issue.

### 12.1 Release map

| Release | Included slices | User-visible outcome |
|---|---|---|
| `0.1` | `S0`–`S4` | Reliable local Rust repository documentation with evidence. |
| `0.2` | `S5`–`S7` | Incremental refresh and cross-project API impact. |
| `0.3` | `S8`–`S9` | Approved polyglot extraction and sandboxed extensions. |
| `0.4` | `S10`–`S12` | Production-candidate server, interfaces, jobs, and tenant isolation. |
| `0.5` | `S13` | Governed Council available in shadow mode. |
| `1.0` | `S14` | Production-readiness evidence and approved pilot. |

No release number is earned merely by completing code. Every included slice
must be Verified.

## 13. Continuous verification gates

The repository now has an infrastructure-only Cargo workspace and an executable
bootstrap gate for formatting, Clippy, unit/target tests, doctests, benchmark
metadata, and benchmark-target compilation. The fuller commands below remain
**planned contracts for `S0`** and become mandatory as their pinned tools and
product acceptance contracts are introduced.

### 13.1 Pull-request gate

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo nextest run --workspace
cargo test --workspace --doc
cargo llvm-cov nextest --workspace --fail-under-lines 80
cargo deny check
```

The pull-request gate MUST also check crate dependency rules, requirement-test
traceability, artifact/schema compatibility, documentation links, and reviewed
golden changes. A retry is diagnostic only; the original failure remains a
failure.

### 13.2 Nightly gate

- Linux, macOS, and Windows test matrix for declared tiers.
- PostgreSQL, object storage, and Wasmtime integration suites.
- Fuzzing, long property tests, differential extractors, and randomized edit
  sequences.
- Mutation testing for changed critical domain rules.
- Performance regression, feature-combination, and previous-schema tests.
- Repeated shuffled test execution to detect order dependency and flakiness.

### 13.3 Release gate

- Tests are rerun against the exact signed release artifacts.
- Migration, mixed-version compatibility where applicable, rollback, and full
  restore exercises pass.
- SBOM, provenance, signature, license, advisory, and vulnerability evidence is
  attached.
- Load, sandbox, tenant-isolation, failure-injection, and malicious-repository
  suites pass.
- Golden corpus and pilot reports are versioned and approved.

The selected Rust test runner is expected to be
[cargo-nextest](https://nexte.st/) for process-isolated CI execution, with
doctests run separately. Coverage is expected to use
[cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov), property testing
may use [proptest](https://github.com/proptest-rs/proptest), mutation testing
may use [cargo-mutants](https://github.com/sourcefrog/cargo-mutants), and fuzz
targets may use [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz). Exact
versions are pinned during `S0` and are implementation choices, not public
product contracts.

## 14. Traceability

### 14.1 Naming convention

The planned convention is:

```text
requirement: FR-ACQ-001
acceptance test: e2e_fr_acq_001_immutable_commit
property test: pt_dr_idn_001_insertion_order_is_irrelevant
evidence record: <CI run>/<artifact hash>/<test report>
```

For S0, the ratified contract fixes the following public evidence names:

```text
black-box test: e2e_fr_acq_001_immutable_commit
branch-move integration test: it_fr_acq_001_ref_move_after_binding_keeps_original_commit
artifact conformance test: conf_dr_art_001_repository_snapshot_v1
envelope property test: pt_dr_art_002_volatile_envelope_preserves_semantic_hash
isolation security test: sec_nfr_sec_005_scan_launches_no_child_and_opens_no_network
```

The machine-readable scenario set and execution budgets are in the
[S0 acceptance specification](../../tests/specifications/s0/e2e_fr_acq_001_immutable_commit.json).
That file is the ratified oracle, not a product test implementation and
not Green evidence.

For S1, the ratified public evidence names include:

```text
black-box test: e2e_fr_inv_001_safe_inventory
limit property test: pt_fr_acq_002_limits_have_max_and_plus_one
source-evidence conformance: conf_dr_evd_001_source_evidence_resolves
filesystem security test: sec_nfr_sec_001_scan_stays_inside_repository_root
sentinel security test: sec_nfr_sec_001_sentinel_scripts_never_execute
```

The complete ordered S1 scenario set, inherited S0 regressions, budgets, Red
condition, and evidence requirements are in the
[S1 acceptance specification](../../tests/specifications/s1/e2e_fr_inv_001_safe_inventory.json).
That file is a protected oracle, not Green or Verified evidence.

For S2, the proposed public evidence names include:

```text
black-box test: e2e_fr_ext_002_rust_knowledge
extractor conformance: conf_fr_ext_001_extraction_chunk_v1
reviewed graph golden: gt_fr_ext_002_reviewed_rust_graph
stable-ID property test: pt_dr_idn_001_stable_ids_ignore_order_and_revision
graph invariant property test: pt_fr_knw_001_graph_invariants
claim-state model test: pt_fr_knw_002_claim_state_machine
rule provenance property test: pt_fr_knw_003_rule_provenance_replays
parser fuzz seed: fz_fr_ext_002_rust_parser_seed_corpus
```

The complete ordered S2 scenario set, inherited S0/S1 regressions, ontology,
identity preimages, malformed/Unicode outcomes, Red condition, and evidence
requirements are in the
[S2 acceptance specification](../../tests/specifications/s2/e2e_fr_ext_002_rust_knowledge.json).
That protected oracle becomes binding only through the manual ratification
merge; it is not Red, Green, or Verified evidence by itself.

For S6, the proposed public evidence names include:

```text
black-box test: e2e_fr_fed_001_openapi_federation
encoding golden: gt_fr_ext_004_yaml_json_semantic_equivalence
explicit-link golden: gt_fr_fed_001_explicit_clients_confirmed
decoy hard negative: gt_fr_fed_001_operation_decoy_rejected
heuristic-state property: pt_fr_fed_002_heuristic_never_auto_confirms
authority conflict: conf_fr_fed_002_conflicting_authority_fails_closed
YAML security: sec_fr_ext_004_duplicate_yaml_key_rejected
limit property: pt_fr_fed_001_every_limit_has_max_and_plus_one
ambient-authority security: sec_fr_fed_001_standard_s6_has_no_ambient_authority
CLI conformance: conf_fr_cli_005_streams_exits_and_no_partial_output
compatibility regression: conf_fr_cli_005_s0_s5_regression
future production Red: red_e2e_fr_fed_001_pre_s6_command_boundary
```

The complete ordered S6 scenarios, fixed limits, identity domains,
dependency-review constraints, determinism repetitions, security observers,
accepted Red, required evidence, and stop conditions are in the
[S6 acceptance specification](../../tests/specifications/s6/e2e_fr_fed_001_openapi_federation.json).
That protected oracle becomes binding only through the manual ratification
merge; it is not Red, Green, or Verified evidence by itself.

Test source, fixture, oracle, CI evidence, and requirement status must be
machine-linkable. A comment containing an ID is not sufficient if CI cannot
detect a missing or stale link.

### 14.2 Initial requirement-to-slice matrix

| Capability | Requirement families | Slice | Principal evidence |
|---|---|---|---|
| Contracts and deterministic skeleton | `DR-ART`, `FR-ACQ-001`, `FR-CLI-003`, `NFR-DET`, `NFR-MNT-001`, `NFR-SEC-005`, `NFR-TST` | `S0` | Schema, replay, black-box CLI, process/network denial |
| Safe repository understanding | `FR-ACQ-002/003/004`, `FR-INV`, `FR-EXT`, `FR-KNW` | `S1`–`S2` | Malicious corpus, packed/loose equivalence, golden graph, property/fuzz |
| Atomic local knowledge | `FR-STO-001`, `FR-SNP`, `FR-DOC`, `FR-QRY-001`, `FR-CLI-001` | `S3`–`S4` | Contract, failpoint, CLI E2E |
| Change and ecosystem reasoning | `FR-INC`, `FR-FED`, `FR-IMP`, `FR-CLI-004/005` | `S5`–`S7` | Cold/incremental equivalence, provider/client/decoy, strict output-only federation |
| Polyglot and extension boundary | `FR-EXT-003/005`, `FR-PLG` | `S8`–`S9` | Adapter contract, golden/differential, sandbox |
| Server platform | `FR-STO-002`, `FR-JOB`, `FR-API`, `FR-MCP`, `FR-AUT` | `S10`–`S12` | Storage contracts, crash, conformance, tenant attacks |
| Governed intelligence | `FR-COU`, `INV-MDL`, `NFR-PRV` | `S13` | Council oracle, calibration, network capture |
| Production readiness | All `P1` NFRs | `S14` | Performance, DR, security, supply chain, pilot |

## 15. Definition of Ready and Definition of Done

### 15.1 Requirement ready for implementation

A requirement is Ready only when:

- its statement is atomic and has one stable ID;
- it is `Approved` on `main`, or its complete `Proposed` candidate and
  branch-scoped implementation authority satisfy section 2.1.1;
- priority, target slice, owner, rationale, and dependencies are known;
- success, failure, limit, security, and observability behaviour are testable;
- the fixture and oracle are reviewable and legally usable;
- blocking open decisions are resolved;
- the initial black-box test, or the appropriate executable non-behavioural
  check, can be written without assuming private design.

### 15.2 Requirement done

A requirement is Done only when:

- the acceptance test or other required executable check was observed red for
  the expected reason before the production implementation;
- the minimal implementation and relevant unit/property tests are green;
- every affected real adapter passes its contract suite;
- invalid input, boundaries, failures, security, and telemetry are covered in
  proportion to risk;
- the public demo is reproducible from a clean checkout;
- traceability and documentation are updated;
- CI evidence is attached and no required test is flaky, ignored, or silently
  retried into success;
- the requirement state is `Verified`, not merely `Implemented`.

## 16. Open decisions and approval gates

Open decisions remain visible rather than being filled by accidental
implementation choices.

| ID | Decision required | Blocks |
|---|---|---|
| `OD-LIM-001` | Numeric defaults and maximums for repository bytes/files, file size, depth, memory, CPU, wall time, output, graph query, jobs, and model cost. Decision 0002 resolves the fixed `standard-local-s1` subset. Decision 0010 resolves only the R2 gitlink, `.gitmodules`, explicit nested-root, depth, and boundary-output subset after protected merge. Decision 0011 resolves only the R3 member, exclusion, package-manifest, crate-target, and binary-root subset after protected merge. Decision 0012 resolves only the R4 manifest-fact, dependency, feature/member, target, patch, metadata, target-predicate, declaration-string, external-locator, and permutation subset after protected merge. Decision 0015 resolves only the R5 field, variant, tuple-field, associated-item, outer-attribute, attribute-token, declared-type/header, and permutation subset after protected merge. Decision 0016 resolves only the R6 per-source declaration, registration-chain, expression-depth, route-path, method/configuration-key, target-spelling, inherited attribute, and permutation subset after protected merge. Decision 0017 resolves only the R7 raw artifact, binding, document, occurrence, symbol, relationship, value, argument, recursion, and permutation subset after protected merge. Decision 0020 resolves only the explicit K1 final RepositorySnapshotV11 64 MiB output-capacity envelope while preserving the standard 32 MiB maximum. Decision 0008 resolves the fixed S5 changed-path, analysis-entry, dependency-edge, report-subject, report-byte, and wall-time subset. Decision 0009 resolves the fixed S6 manifest, repository, document, YAML/reference depth, semantic count, evidence, output, memory, and wall-time subset when its protected ratification revision is manually merged. Decision 0007 resolves only the fixed implementation-aware S7 report-count and output subset. | Approval of remaining `S7`, `S9`, `S10`, `S13` limits |
| `OD-ONT-001` | Decision 0003 resolves the bounded single-crate `codenoesis.ontology/rust/v1`. Decision 0005 resolves multi-crate cardinality and unambiguous out-of-line module identity for `codenoesis.ontology/rust/v2`. Decision 0011 resolves only root-package workspace provenance and R3 coverage semantics in `codenoesis.ontology/rust/v3` while preserving v2 identity domains for unchanged facts. Decision 0012 resolves only declaration-level Cargo entities, relationships, evidence, claim policy, and coverage states in `codenoesis.ontology/rust/v4` while preserving v3 Rust identities after protected merge. Decision 0015 resolves only the R5 committed Rust field, variant, constant/static, associated-type, implementation-context method, attribute-evidence, compilation-presence, and member-identity subset in `codenoesis.ontology/rust/v5` while preserving unchanged v4 identities after protected merge. Decision 0016 resolves only the R6 framework-neutral committed source declarations, unresolved attribute/macro candidates, disjoint identity, lexical `DEFINES` ownership, target-binding states, and explicit non-runtime semantics in `codenoesis.ontology/rust/v6` while preserving unchanged v5 identities after protected merge. Decision 0017 resolves only revision-bound Rust SCIP v0.9.0 compiler symbols, explicit compiler relationships, dual source/index evidence, precedence/conflict handling, and honest unsupported-call/generated states in `codenoesis.ontology/rust/v7` while preserving unchanged v6 identities after protected merge. Decision 0019 proposes only K1 committed-source callable signatures, parameters, bounded declared values, body syntax, lexical unique-local calls, explicit uncertainty, and portable/query views in `codenoesis.ontology/rust/v8`; it becomes effective only with protected merge of issue #142 and preserves all prior identities. Cross-language adapters, K1/R7 composition, index generation, compiler CFG/data flow, expanded/generated framework meaning, runtime observation, and later ontology evolution remain open. | `S8` and later ontology evolution |
| `OD-STO-001` | Decision 0004 resolves fresh single-writer local SQLite/CAS identity, publication, restart, corruption, and cleanup semantics for `codenoesis.local-store/v1` only when its protected S3 ratification revision is manually merged. Migration, repair, deletion, backup/restore, multi-writer, and server storage remain open. | Post-S3 storage evolution and `S10` |
| `OD-GIT-001` | Decision 0006 resolves the packed local SHA-1 subset only for the explicit `local-git-sha1-packed-v1` acquisition selector. After protected merge, Decision 0010 additionally resolves only the explicit `local-gitlinks-v1` representation of committed mode `160000` boundaries, bounded root `.gitmodules` metadata, and depth-one separately supplied local nested commit verification. Residual decisions cover remote protocols and identity resolution, SHA-256, LFS, shallow and bare repositories, alternates, promisor/partial clones, MIDX authority, symlinks, nested analysis/federation/recursion, complete Git configuration semantics, automatic repair, and history rewrite. Legacy invocations still reject packed objects without R1 and reject gitlinks without R2. | Remote and remaining post-S1 `FR-ACQ-*` |
| `OD-CMP-001` | Decision 0009 resolves only `codenoesis.contract-capability/openapi-3.1-http-json/v1`, deterministic provider/client operation federation, heuristic non-confirmation, and source-neutral S6 identities when its protected ratification revision is manually merged. Decision 0007 resolves only the `implementation-aware-http-json/v1` field-level projection, evidence separation, seven dimensions, classifier rules, and oracle. Residual decisions cover complete OpenAPI, AsyncAPI, GraphQL, Protobuf, events, package/SCIP and event/schema authority, framework-specific semantics, observation coverage, protocol behavior, and causal evidence. | Remaining `S6`–`S7` compatibility capabilities |
| `OD-API-001` | REST/MCP payload schemas, error catalog, cancellation, pagination, event resume, and deprecation window. | `S10`–`S11` |
| `OD-AUT-001` | Complete role-action-resource matrix and privileged break-glass policy. | `S12` |
| `OD-SBX-001` | Supported OS/platform sandbox tiers and which guarantees require Linux isolation. | `S9`, release matrix |
| `OD-DAT-001` | Data classes, default retention, legal hold, purge deadline, backup expiry, residency, and export policy. | `DR-DEL-001`, `NFR-PRV-002` |
| `OD-SLO-001` | Reference corpus, concurrency, cold/warm definitions, SLI queries, error budget, p99 targets, resource ceilings, and alignment of 99.9% availability with RTO. | `NFR-PER-002`, `NFR-DR-001`, `S14` |
| `OD-COU-001` | Ambiguity/high-impact selection thresholds, provider policy, budgets, calibration target, and shadow-to-gate promotion rule. | `S13` |

An open decision may be resolved by an approved ADR, schema, policy, benchmark
contract, or decision table. It must then be linked from the affected
requirements and represented in tests.

The [S0 contract decision](decisions/0001-s0-walking-skeleton-contract.md)
resolves the one-file local binding. The
[S1 contract decision](decisions/0002-s1-safe-inventory-contract.md) extends
only the verified loose-object tree subset and fixes rejection semantics for
symlinks, gitlinks, and external Git directories. The
[S1 gitlink boundary decision](decisions/0010-s1-gitlink-boundary-contract.md)
resolves only explicit representation and separate depth-one local verification
of gitlinks after its protected manual merge; it authorizes no traversal,
nested analysis, URL authority, or federation. `OD-GIT-001` remains open for
those semantics and every other listed advanced or remote case. The
[S2 contract decision](decisions/0003-s2-rust-knowledge-contract.md) resolves
`OD-ONT-001` only for the bounded Rust v1 ontology and only after its protected
manual merge; future language adapters or semantic expansion require another
versioned decision. The
[S3 contract decision](decisions/0004-s3-atomic-local-storage-contract.md)
resolves `OD-STO-001` only for a fresh local v1 store with one application
writer; every migration, repair, deletion, restore, multi-writer, and server
profile decision remains open. The
[S4 contract decision](decisions/0005-s4-workspace-docs-query-contract.md)
resolves the bounded Rust ontology v2, generated Markdown, and exact-ID local
query semantics only for the explicit S4 profile; Cargo evaluation, compiler
resolution, traversal/search, and later documentation formats remain open.
The
[S4 root-package decision](decisions/0011-s4-root-package-workspace-contract.md)
resolves only explicit R3 standalone/virtual/non-virtual package membership,
literal exclusions, target roots, gitlink composition, and ontology v3
provenance. The
[S4 Cargo manifest facts decision](decisions/0012-s4-cargo-manifest-facts-contract.md)
resolves only explicit R4 declaration entities, byte evidence, digest-only
locators, typed coverage, and ontology v4 identities after protected merge;
dependency/feature/target resolution, patch application, generated code, and
execution remain open.
The
[S4 R4 exact-ID query decision](decisions/0013-s4-r4-exact-id-query-contract.md)
resolves only additive V7 LocalQueryResultV2 dispatch, direct exact-ID
projection for approved R4 facts and uncertainty artifacts, and stable Cargo
diagnostic identities; V1 mutation, traversal, fuzzy search, repair, migration,
and new authority remain open.
The
[S4 R4 legacy badges decision](decisions/0014-s4-r4-legacy-badges-contract.md)
resolves only the exact typed-unsupported boundary for a literal top-level
`[badges]` table after the pinned public pilot exposed the missing mapping;
badge-value interpretation, provider semantics, network access, new
relationships, silent ignore, and any broader Cargo-family support remain
open.
The
[S4 R5 Rust semantic-depth decision](decisions/0015-s4-r5-rust-semantic-depth-contract.md)
resolves only the explicit `rust-semantic-depth-v1` committed declaration
subset, member and implementation-context method identities, outer-attribute
uncertainty, ontology v5 relationships, fixed R5 limits, ErrorV12, and V8-only
LocalQueryResultV3 dispatch after protected manual merge; framework roles,
macro expansion, active cfg worlds, compiler/type resolution, value
evaluation, calls, runtime behavior, migration, and explorer/export semantics
remain open.
The
[S4 R6 framework-declarations decision](decisions/0016-s4-r6-framework-declarations-contract.md)
resolves only the explicit `rust-framework-declarations-v1` source profile,
framework-neutral declaration entities, `declared_registration_syntax` and
`candidate_unresolved` states, disjoint identities, lexical ownership,
unique-local-target binding, fixed R6 limits, ErrorV13, and V9-only
LocalQueryResultV4 dispatch after protected manual merge. Macro expansion,
active cfg worlds, compiler/type/trait/value resolution, generated code,
calls, runtime behavior or observation, migration, and explorer/export
semantics remain open.
The
[S4 R7 revision-bound SCIP import decision](decisions/0017-s4-r7-scip-import-contract.md)
resolves only explicit static import of one bound Rust SCIP v0.9.0 artifact,
compiler-symbol identity, four explicit relationship kinds, dual source/index
evidence, fixed R7 limits, ErrorV14, and V10-only LocalQueryResultV5 dispatch
after protected manual merge. Index generation, compiler or build execution,
macro expansion, reliable call or data-flow meaning, runtime behavior,
migration, and explorer/export semantics remain open.
The
[S4 R8 portable export and offline explorer decision](decisions/0018-s4-r8-portable-explorer-contract.md)
resolves only deterministic lossless PortableGraphV1 export from one validated
V10 head and generation of one read-only first-party static LocalExplorerV1
with exact R7 identities/evidence, bounded search/traversal, fixed CSP/XSS,
privacy, path, atomic-output, and resource semantics after protected manual
merge. Product implementation, source snippets, remote resources, auto-launch,
servers, graph databases, ontology changes, S5/S6/S7 behavior, and external
repository bytes remain open or explicitly forbidden.
The
[S5 incremental refresh decision](decisions/0008-s5-incremental-refresh-contract.md)
resolves only the revision-neutral Rust-workspace analysis cache,
commit-bound S4 rematerialization, conservative invalidation catalog, local
refresh report/error contract, and fixed S5 bounds after protected manual
merge; cross-language invalidation, distributed cache, and S6/S7 incremental
reasoning remain open. The
[S6 OpenAPI federation decision](decisions/0009-s6-openapi-federation-contract.md)
resolves only the output-only `standard-local-s6` OpenAPI 3.1.0 HTTP/JSON
capability, restricted JSON/YAML normalization, explicit operation authority,
heuristic candidate state, source-neutral identities, strict artifacts, and
fixed S6 bounds after protected manual merge; other contract formats,
authority sources, language/framework extraction, persistence, and dynamic
behavior remain open.

## 17. Change control

- A requirement change and its tests MUST be reviewed together.
- One maintainer-supervised single-PR vertical package MAY atomically review and
  merge one capability's product governance, Red-first evidence, production
  implementation, delivery control plane, and Green evidence under section
  2.1.1.
- A branch-scoped package MUST preserve its governance checkpoint and expected
  Red evidence; merge is all-or-nothing and no candidate product contract or
  implementation becomes authoritative on `main` independently.
- A control-plane change in that package MUST remain inert for its own review;
  the exact base authority and an unchanged base-controlled gate judge the head,
  and privileged effects activate only after manual merge.
- Removing or weakening an Approved requirement requires rationale, impact
  analysis, and explicit approval.
- Breaking public-contract changes require compatibility classification and a
  migration/deprecation plan.
- Test oracle changes require the same scrutiny as production changes; golden
  output must never be accepted solely because a snapshot-update command
  produced it.
- Research evidence may create a Proposed requirement but cannot move it to
  Approved automatically.
- Production incidents and escaped defects MUST add or strengthen a regression
  scenario before the corrective implementation is accepted.

## 18. Reference standards and tools

- [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) and
  [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174) for requirement language.
- [RFC 9457](https://www.rfc-editor.org/rfc/rfc9457) for planned REST problem
  details.
- [SLSA 1.2](https://slsa.dev/spec/v1.2/) for incremental supply-chain
  provenance and verification goals.
- [cargo-nextest](https://nexte.st/),
  [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov),
  [proptest](https://github.com/proptest-rs/proptest),
  [cargo-mutants](https://github.com/sourcefrog/cargo-mutants), and
  [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) as candidate test
  infrastructure to pin during `S0`.

These references support the verification approach. Tool inclusion does not
make a specific tool version part of the product interface.
