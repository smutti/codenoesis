# CodeNoesis Local Baseline Verification

> Status: **Proposed `LocalBaselineVerificationV3` contract for issue #201**. “LocalBaselineVerificationV3 candidate Verified pending independent review and protected manual merge” is the exact pre-activation marker. No new `Verified`, release, support, or GA claim is effective before independent review and protected manual merge of the exact evidence-complete head.

## Purpose

`Implemented` records that accepted behavior exists. `Verified` additionally
requires complete, immutable, independently accepted evidence for every linked
oracle. Protected PR #189 already activated the immutable 32-profile
`LocalBaselineVerificationV2` baseline. Protected PRs #191 and #197 made R18
trusted local source retrieval and R19 Git-backed semantic impact Approved and
Implemented, but not Verified.

Issue #201 consolidates only those two additive profiles with the exact V2
baseline. The resolved V3 catalog contains exactly 34 profiles. It changes no
product behavior, ontology, schema, fixture, golden, workflow, permission,
dependency, release authority, support, or GA status.

## Authority

- Issue: <https://github.com/smutti/codenoesis/issues/201>
- Authorization: <https://github.com/smutti/codenoesis/issues/201#issuecomment-5477071815>
- Exact base: `c783b612777a86e2f88620ece987723bb230c51c`
- Delivery slice: `S14`
- Risk: `high`
- Correction budget: five rounds
- Runtime, dependency, control-plane, and release changes: forbidden
- Activation: independent review plus protected manual merge

## Canonical contracts

- Decision: `docs/software/decisions/0041-local-baseline-verification-v3.md`
- Plan: `tests/specifications/verification/local-baseline-v3/plan.json`
- Resolved catalog contract:
  `tests/specifications/verification/local-baseline-v3/profile-catalog.json`
- Manifest schema:
  `tests/specifications/verification/local-baseline-v3/manifest.schema.json`
- Candidate manifest:
  `tests/evidence/verification/local-baseline-v3/manifest.json`

The public acceptance command is:

```text
python3 scripts/verify_local_baseline_v3.py --manifest tests/evidence/verification/local-baseline-v3/manifest.json
```

The validator must reject missing paths, digest mismatches, V2 drift,
duplicate, reordered or partial profiles, unbound Git or GitHub identities,
non-Green mandatory gates, unsafe or private evidence, product-tree changes,
unsupported lifecycle transitions, and any G9, release, support, or GA
implication.

## V2 inheritance and additive evidence

V3 binds the exact accepted V2 plan, catalog, schema, manifest, validator, and
activation merge by digest and identity. It does not copy, relabel, weaken, or
rewrite their evidence. The only additive catalog entries are:

- `r18-trusted-local-source`: issue #190, Decision 0038, protected PR #191,
  review head `16ef5ceaea6ad14d9838f84856f6ca3d445daa67`, merge
  `fcdd6eddec8a4dd9b372cb88ff424c2004b5c88b`;
- `r19-git-backed-semantic-impact`: issue #196, Decision 0040, protected PR
  #197, review head `c3cbced9ee2017b61ec8e0b10191553edc733004`, merge
  `c783b612777a86e2f88620ece987723bb230c51c`.

The evidence pack binds retained local Red/Green/correction observations,
contracts, schemas, traceability, Linux/macOS/Windows conclusions, security,
privacy, race, limit, benchmark, CodeQL, policy, and normalized remote logs.
The authoring agent is not an independent reviewer.

The issue #201 amendment permits one verification-only compatibility
correction: the V2 validator continues reading and digesting its historical
`docs/software/verification.md` checkpoint blob from Git, but does not require
this versioned descendant lifecycle document to remain byte-identical forever.
All other V2 contracts and current-file pins remain unchanged. The correction
has its own checkpoint, expected conflict Red, regression, and retained Green.

The second issue #201 amendment preserves every R18/R19 bundle and accepted
digest while correcting only their conformance tests. Lifecycle-document
records are read from the exact protected merge that accepted each bundle:
`fcdd6eddec8a4dd9b372cb88ff424c2004b5c88b` for R18 and
`c783b612777a86e2f88620ece987723bb230c51c` for R19. Non-lifecycle records
remain current-file checks. V3 alone validates the current 34-profile lifecycle
wording, so later versioned documentation cannot weaken historical product
contracts or make their accepted byte identities depend on the descendant
working tree.

The third amendment handles the two bundle self-records without weakening
coverage. Each corrected R18/R19 conformance test reads its own accepted
historical record from the same protected merge as the lifecycle documents;
every other non-lifecycle record remains current-tree-bound. V3 separately pins
the corrected test bytes, so both the immutable historical bundle and the
current verification correction remain reviewable.

## Promotion rule

The V3 manifest uses `candidate_verified_pending_merge` on the branch.
Protected manual merge of the exact independently accepted head makes only the
34 catalogued profiles Verified. Missing, unavailable, flaky, contradictory,
private, or non-reproducible evidence keeps the complete package open; subset
promotion is forbidden.

G9 pilot, release, support, deprecation, EOL, publication, Local GA, and Server
GA remain separate future governance.
