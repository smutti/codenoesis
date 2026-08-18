# Decision 0039: Local baseline V2 post-merge activation

- Status: Proposed branch-scoped correction
- Date: 2026-08-18
- Issue: [#194](https://github.com/smutti/codenoesis/issues/194)
- Exact base: `1de6a420f25a1c7eb74d07a99f1800dde90eefa8`
- Requirements: `NFR-TST-001`, `NFR-TST-002`, `NFR-MNT-001`,
  `INV-BND-001`
- Delivery slice: `S14`
- Risk: high

## Context

Protected PR #189 reviewed
`a40a4cb0212e7b59b1eff81ab9818299c7ebc3b9` and squash-merged it as
`1de6a420f25a1c7eb74d07a99f1800dde90eefa8`. Both commits have exact tree
`c13f40fa96017fa4407cb26052f8b5e3c7bb7009`, and the squash commit has exact
parent `9ecdc3acefd43495daf76b9f2ab69a7bbacff172`.

The LocalBaselineVerificationV2 validator nevertheless requires the original
branch evidence and full-gate commits to be ancestors of current `HEAD`.
Squash merging intentionally does not preserve that ancestry, so the validator
fails on the protected merge and every later descendant even though the
reviewed and merged trees are byte-identical. It also attributes unrelated
later product paths to issue #188.

The retained V2 manifest is immutable pre-activation evidence and correctly
remains `candidate_verified_pending_merge`. Rewriting it after merge would
destroy the reviewed artifact rather than prove activation.

## Decision

Add one strict external activation record for the exact PR #189 squash. The
record binds the issue, pull request, target branch, protected base, review
head and tree, merge commit, parent and tree, merge method, time, accountable
maintainer, candidate status, activated status, and decision URL.

The validator supports exactly two modes:

1. candidate lineage preserves the original issue #188 ancestry and evaluates
   the candidate at current `HEAD`;
2. activated descendant requires the exact merge commit to be an ancestor of
   current `HEAD`, validates the activation record and exact Git identities,
   and evaluates issue #188 scope, product-tree identity, and full-gate
   ancestry at the exact review head.

Review-tree and merge-tree identity is mandatory. The squash parent must equal
the original protected base. An unknown head, merge, parent, tree, actor,
status, record, or schema fails closed. Later descendant changes are not
reattributed to issue #188.

The original manifest, schema, plan, catalog, Decision 0037, retained evidence,
S0 historical evidence, and pending-review marker remain byte-identical.

## Public oracle

```text
python3 scripts/verify_local_baseline_v2.py --manifest tests/evidence/verification/local-baseline-v2/manifest.json
```

The exact base is Red only because the evidence-parent and full-gate branch
commits are not ancestors of the squash commit. Green requires exact activation
identity on the base and on later descendants, plus fail-closed mutation tests.

## Authority boundaries

This correction changes no product behavior, ontology, product schema,
fixture/golden, dependency, workflow, permission, secret, signing,
publication, deployment, release, tag, support, SLA, or GA authority. Only the
paths authorized by issue #194 may change. Rollback is a revert of the complete
correction pull request.

## Required sequence

1. Commit this decision, the closed activation schema and record, and the
   narrow acceptance tests before changing the validator.
2. Retain the expected Red on that checkpoint.
3. Implement the minimum fail-closed mode selection and historical-subject
   validation.
4. Run focused tests, the public oracle on the exact squash and a descendant,
   and the complete repository gate.
5. Obtain independent review and protected manual merge.

## Stop conditions

Stop for a new maintainer decision if an exact identity, semantic oracle, risk,
path, dependency, authority, status transition, product behavior,
control-plane effect, or release effect changes; if tree equality cannot be
proven; if historical evidence must be weakened; or if three correction rounds
are exhausted.
