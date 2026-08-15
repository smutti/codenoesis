# Decision 0037: Local baseline verification V2

- Status: Proposed branch-scoped candidate
- Date: 2026-08-15
- Issue: [#188](https://github.com/smutti/codenoesis/issues/188)
- Exact base: `9ecdc3acefd43495daf76b9f2ab69a7bbacff172`
- Delivery slice: `S14`
- Risk: high

## Context

CodeNoesis has implemented S0 through S7, roadmap R0 through R17 and K1, and
the bounded G0 through G8 local controls. Their accepted contracts require
independent immutable evidence before `Verified`. Issue #141 attempted a
smaller S0-S6/R0-R8 closure on an earlier base, but stopped because the S0
evidence schema accepted only expiring GitHub Actions artifacts that the
base-controlled workflow had not uploaded.

The product behavior, ontology, schemas, fixtures, goldens, historical Reds,
and implementation evidence are not reopened. The remaining decision is how
to bind the complete implemented local baseline to durable, independently
reviewable evidence without granting release or GA authority.

## Decision

Adopt `LocalBaselineVerificationV2` as one verification-only S14 package. Its
canonical plan and profile catalog enumerate the only capabilities eligible
for promotion. Every declared profile must be complete; partial or optimistic
promotion is invalid.

The package adds one strict S0 evidence alternative,
`git-retained-github-actions-log/v1`. The existing expiring artifact form
remains unchanged. A Git-retained log is admissible only when it binds the
exact repository, workflow bytes and ref, run attempt, job and check, head and
tree, successful base-controlled conclusion, source-log digest, deterministic
normalization, committed-log digest, first retention commit, and consuming
evidence head. Prose, local-only logs, missing remote identities, mutable URLs,
head-authored self-authority, and unbound reruns are rejected.

The public acceptance command is:

```text
python3 scripts/verify_local_baseline_v2.py --manifest tests/evidence/verification/local-baseline-v2/manifest.json
```

The validator independently recomputes repository-owned digests, Git
identities, product-tree identity, profile completeness, evidence classes,
remote-log bindings, status parity, and the absence of production or control
plane changes. The candidate manifest remains
`candidate_verified_pending_merge` while the pull request is open. A
`Verified` transition is effective only after independent review and protected
manual merge of the exact evidence-complete head.

## Authority boundaries

This decision grants no production Rust, dependency, ontology, fixture,
golden, workflow, permission, secret, signing, publication, deployment, tag,
release, support, EOL, SLA, or GA change. It grants no self-approval or
self-merge. G9 remains a separate governed decision.

The only allowed paths are those in issue #188. No new dependency is allowed.
Rollback is a revert of the verification pull request; no product state or
stored artifact migration exists.

## Required sequence and evidence

1. Commit this decision, the V2 contracts, catalog, traceability plan, strict
   S0 evidence form, and focused test before the validator, manifest, retained
   V2 evidence, or lifecycle transitions.
2. Retain the focused expected Red on that checkpoint.
3. Add only the validator, immutable evidence pack, lifecycle reconciliation,
   and verification documentation needed for Green.
4. Run the focused command, public validator, complete repository gate, and
   unchanged base-controlled GitHub checks.
5. Obtain independent review and protected manual merge.

Issue #141 checkpoint `5f27f0c50899e513930ef7793a863477dc680dcd`
and Red `27632f842b6b5fc5b798656183f45db0fb4931a4` remain historical
blocker evidence. They are not reused as the V2 checkpoint or Red.

## Stop conditions

Stop for a new maintainer decision if any profile lacks reproducible evidence,
an accepted product oracle would need semantic change, production or control
plane work becomes necessary, remote run/job/log lineage cannot be proven, a
gate is flaky or contradictory, scope or risk changes, or five correction
rounds are exhausted.
