# CodeNoesis Local Baseline Verification

> Status: **Proposed `LocalBaselineVerificationV2` contract for issue #188**.
> No new `Verified`, release, support, or GA claim is effective before
> independent review and protected manual merge of the exact evidence-complete
> head.

## Purpose

`Implemented` records that accepted behavior exists. `Verified` additionally
requires complete, immutable, independently accepted evidence for every linked
oracle and release obligation. Issue #188 consolidates that assurance for S0
through S7, R0 through R17, K1, and the bounded G0 through G8 local controls.

The package is verification-only. It binds accepted contracts,
implementation merges, retained Red and Green observations, current regression
gates, platform and security outcomes, browser and real-repository pilots,
known limitations, and independent decisions. It changes no product behavior.

## Authority

- Issue: <https://github.com/smutti/codenoesis/issues/188>
- Exact base: `9ecdc3acefd43495daf76b9f2ab69a7bbacff172`
- Delivery slice: `S14`
- Risk: `high`
- Correction budget: five rounds
- Runtime and dependency changes: forbidden
- Control-plane and release changes: forbidden
- Activation: independent review plus protected manual merge

## Canonical contracts

- Decision: `docs/software/decisions/0037-local-baseline-verification-v2.md`
- Plan: `tests/specifications/verification/local-baseline-v2/plan.json`
- Profile catalog:
  `tests/specifications/verification/local-baseline-v2/profile-catalog.json`
- Manifest schema:
  `tests/specifications/verification/local-baseline-v2/manifest.schema.json`
- Candidate manifest:
  `tests/evidence/verification/local-baseline-v2/manifest.json`

The acceptance command is:

```text
python3 scripts/verify_local_baseline_v2.py --manifest tests/evidence/verification/local-baseline-v2/manifest.json
```

The validator must reject missing paths, digest mismatches, duplicate or
partial profiles, unbound Git or GitHub identities, non-Green mandatory gates,
unsupported lifecycle transitions, production/control-plane changes, and any
G9 or GA implication.

## Durable CI evidence

The original S0 artifact form remains accepted. The additive
`git-retained-github-actions-log/v1` form makes a downloaded GitHub Actions job
log durable through Git only when exact remote and source identities,
normalization, committed bytes, and consuming evidence head all agree. It does
not convert a local observation or prose claim into independent evidence.

## Promotion rule

The manifest uses `candidate_verified_pending_merge` on the branch. The target
lifecycle is effective on protected `main` only when the exact head is
independently accepted and manually merged. Missing, unavailable, flaky,
contradictory, or non-reproducible evidence keeps the entire package from
closing; subset promotion is forbidden.

G9 pilot, release, support, deprecation, EOL, publication, and final GA remain
separate future governance.
