# CodeNoesis Software Requirements Specification

> Status: **0.9 — S0 through S5 implemented; S6 bounded OpenAPI federation
> Approved and implementation-completeness correction pending protected
> merge**.
> The S0–S5 runtime and product suites exist on `main`, but CodeNoesis claims
> no slice `Verified` without complete immutable retention evidence. This
> revision records PR #81 approval of the OpenAPI 3.1.0 capability-scoped
> portion of `FR-EXT-004`, plus `FR-FED-001`, `FR-FED-002`, `FR-CLI-005`, and
> corrects their implementation contract before issue #82 resumes. It changes
> no existing accepted profile, storage contract, ontology, or production
> runtime.

## 1. Document control

| Field | Value |
|---|---|
| Scope | CodeNoesis software track, from the first local slice through version `1.0` |
| Version | `0.9` |
| Status | S0 through S5 are Approved and Implemented but not Verified. The bounded OpenAPI 3.1.0 portion of `FR-EXT-004`, plus `FR-FED-001`, `FR-FED-002`, and `FR-CLI-005`, became Approved through protected PR #81. Production implementation issue #82 may resume only after protected merge of the implementation-completeness correction in PR #84. |
| Date | 2026-07-30 |
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

- **Proposed:** reviewable but not yet binding for implementation.
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

After requirements are `Approved` on `main`, one explicit human authorization
may select a maintainer-supervised accelerated package in a linked Ready issue.
The package MUST fix one delivery slice, one coherent vertical outcome, exact
requirement IDs, risk and rollback boundary, allowed and protected paths, exact
dependencies, acceptance oracle and expected Red, evidence, a bounded
correction budget, and stop conditions.

The package MAY include multiple tightly related requirement IDs or
sub-behaviors in one implementation pull request only when they share the same
public acceptance journey, risk owner, rollback boundary, and versioned fixture
or oracle. An exact dependency and its lockfile update MAY accompany the
behavior only when the issue names and reviews the dependency. Unrelated
capabilities, upgrades, cleanup, and generated churn remain separate.

Within unchanged scope and risk, that one explicit human authorization permits
implementation, focused validation, documentation and evidence updates, pull
request publication, and bounded correction without repeated authorization.
The default is three correction rounds; the issue MAY set a value from one
through five.

This supervised interactive lane may proceed without waiting for the separate
machine-policy projection. `.github/codex/policy.json` remains mandatory before
unattended autonomous execution and may be prepared in parallel. The lane does
not alter the requirement lifecycle, permit production work while a requirement
is Proposed, weaken an oracle, broaden paths or risk, authorize a control-plane
change in the product pull request it governs, or grant approval or merge
authority to the authoring agent.

This control-plane amendment is authorized by
[#79](https://github.com/smutti/codenoesis/issues/79) and becomes effective only
after its protected manual merge by `@smutti`. The authoring agent does not
approve or merge that change.

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
`sha256:64587eb20880f0beb17320abf6dd301d811fabdc248eb3f59268aebf7ca47081`.
The bundle binds Decision 0009, the independent governance guard, strict
workspace/client/report/error schemas, rule catalog, machine oracle,
project-owned fixture and hostile variants, reviewed outputs, and immutable S7
identity dependencies. Any bound-byte change requires a new digest and renewed
human review.

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
| `FR-QRY-001` | P0 | `0.1` | Local queries MUST retrieve entities, claims, evidence, and documents by stable identity and expose unknown or contradictory states. | CLI black-box scenarios return the reviewed typed result and stable exit status. `E2E` |
| `FR-QRY-002` | P1 | `0.2` | Graph traversal MUST enforce configurable depth, result, time, and resource limits with cycle handling. | Cyclic and adversarial queries terminate within the configured bound without starving unrelated work. `PT`, `PERF`, `SEC` |
| `FR-CLI-001` | P0 | `0.1` | The CLI MUST provide local `scan`, `docs`, and `query` journeys with human-readable and versioned JSON output. | One black-box fixture completes scan -> docs -> query without network access. `E2E`, `CONF` |
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
| `NFR-SEC-001` | P0 | Standard local analysis MUST have zero target process execution, zero analysis-stage network access, and zero filesystem access outside allowed roots. | A malicious-repository corpus includes traversal, symlink loops, archive/repository bombs, oversized input, parser attacks, and sentinel scripts. `SEC`, `FZ` |
| `NFR-SEC-002` | P1 | Server authorization and storage MUST satisfy `INV-TEN-001` across database, objects, FTS, caches, jobs, events, logs, metrics, and model requests. | Randomized multi-tenant operations and explicit attack cases find no cross-tenant data. `SEC`, `PERF` |
| `NFR-SEC-003` | P1 | Secrets MUST use an external secret manager, MUST be redacted from observable output, and MUST be scoped to the stage that requires them. | Canary secrets never appear in persisted artifacts, logs, traces, metrics, errors, or model payloads. `SEC` |
| `NFR-SEC-004` | P1 | A release MUST have no known exploitable Critical vulnerability. High-risk exceptions require owner, expiry, rationale, and compensating control. | Dependency, container, binary, and configuration reports plus the exception register are release artifacts. `SEC`, `CONF` |
| `NFR-SEC-005` | P0 | From S0 `noesis` process start until exit, a standard local scan MUST launch no child process and MUST have no direct or brokered network channel. Fixture setup and the test harness are outside this monitored boundary. | A Linux black-box run combines an empty network namespace, non-socket-only inherited standard descriptors, and the ratified deny-and-audit seccomp policy for process, socket/network, and `io_uring` paths. Generated probes cover every policy syscall and conditional branch on the selected architecture; missing, unexpectedly allowed, or unproved `not_exposed` results fail. `SEC`, `E2E` |
| `NFR-PRV-001` | P0 | Source, evidence, and derived knowledge MUST NOT leave the local system or configured workspace unless an authorized user explicitly enables an allowlisted destination. | Network capture in default/off mode records zero external content-bearing calls. `SEC`, `E2E` |
| `NFR-PRV-002` | P1 | Data classification, retention, export, deletion, legal hold, residency, and backup expiry MUST be explicit per deployment policy. | Lifecycle conformance tests exercise creation through purge and backup expiration. `SEC`, `DR` |

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
`FR-ACQ-004`. The packed SHA-1 behavior is an additive S1 compatibility
extension with its own lifecycle and oracle: it becomes Implemented only when
the separate delivery change proves loose/packed semantic-byte equivalence,
every fixed maximum and maximum-plus-one case, corruption and race behavior,
and all inherited S0–S4 regressions. Until then S1 base remains Implemented
while `FR-ACQ-004` remains independently Proposed or Approved.

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
| `OD-LIM-001` | Numeric defaults and maximums for repository bytes/files, file size, depth, memory, CPU, wall time, output, graph query, jobs, and model cost. Decision 0002 resolves the fixed `standard-local-s1` subset. Decision 0008 resolves the fixed S5 changed-path, analysis-entry, dependency-edge, report-subject, report-byte, and wall-time subset. Decision 0009 resolves the fixed S6 manifest, repository, document, YAML/reference depth, semantic count, evidence, output, memory, and wall-time subset when its protected ratification revision is manually merged. Decision 0007 resolves only the fixed implementation-aware S7 report-count and output subset. | Approval of remaining `S7`, `S9`, `S10`, `S13` limits |
| `OD-ONT-001` | Decision 0003 resolves the bounded single-crate `codenoesis.ontology/rust/v1`. Decision 0005 resolves multi-crate cardinality and unambiguous out-of-line module identity for `codenoesis.ontology/rust/v2` only when its protected S4 ratification revision is manually merged. Cross-language, compiler-grade, and later ontology evolution remain open. | `S8` and later ontology evolution |
| `OD-STO-001` | Decision 0004 resolves fresh single-writer local SQLite/CAS identity, publication, restart, corruption, and cleanup semantics for `codenoesis.local-store/v1` only when its protected S3 ratification revision is manually merged. Migration, repair, deletion, backup/restore, multi-writer, and server storage remain open. | Post-S3 storage evolution and `S10` |
| `OD-GIT-001` | Decision 0006 resolves the packed local SHA-1 subset only for the explicit `local-git-sha1-packed-v1` acquisition selector. Residual decisions cover remote protocols and identity resolution, SHA-256, LFS, shallow and bare repositories, alternates, promisor/partial clones, MIDX authority, supported submodule/symlink semantics, automatic repair, and history rewrite. Legacy S1 still rejects packed objects without the selector and rejects rather than traverses symlinks and gitlinks. | Remote and remaining post-S1 `FR-ACQ-*` |
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
symlinks, gitlinks, and external Git directories. `OD-GIT-001` remains open for
supported traversal or materialization of those features and for every other
listed advanced or remote case. The
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
