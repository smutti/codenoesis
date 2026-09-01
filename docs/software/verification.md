# CodeNoesis Local Verification and Benchmark Evidence

> Status: **LocalBaselineVerificationV3 activated by protected PR #204**.
> The exact 34-profile catalog is Verified. Issue #205 adds only a Proposed B1
> observational benchmark candidate; no SLO, release, support, or GA claim is
> effective before independent review and protected manual merge.

## Purpose

`Implemented` records that accepted behavior exists. `Verified` additionally
requires complete, immutable, independently accepted evidence for every linked
oracle. Protected PR #189 already activated the immutable 32-profile
`LocalBaselineVerificationV2` baseline. Protected PRs #191 and #197 made R18
trusted local source retrieval and R19 Git-backed semantic impact Approved and
Implemented, but not Verified.

Issue #201 consolidated only those two additive profiles with the exact V2
baseline. The resolved V3 catalog contains exactly 34 profiles. Protected PR
#204 merged review head `75391c9061d691c1d6efdf8b726e120049389476` as
`3fb6504d1d6cb39f204eca032ff816266194e1ec`, activating only those Verified
profiles. It changed no product behavior, ontology, schema, fixture, golden,
workflow, permission, dependency, release authority, support, or GA status.

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

The retained V3 manifest uses `candidate_verified_pending_merge` as its
immutable branch marker. Protected manual merge of the exact independently
accepted head made only the 34 catalogued profiles Verified. Missing,
unavailable, flaky, contradictory, private, or non-reproducible evidence would
have kept the complete package open; subset promotion remains forbidden.

G9 pilot, release, support, deprecation, EOL, publication, Local GA, and Server
GA remain separate future governance.

## B1 real-world Rust stability candidate

Issue [#205](https://github.com/smutti/codenoesis/issues/205) and
[Decision 0042](decisions/0042-real-world-rust-stability-benchmark.md) define
one high-risk S14 benchmark package on exact V3 base
`3fb6504d1d6cb39f204eca032ff816266194e1ec`. Its checkpoint precedes runner and
active-validator implementation. The expected Red is the absent runner and the
base validator's unconditional rejection of active status.

Green requires three baseline and three candidate samples for each pinned
entry. Lekton must preserve exact R16 semantic identity and counts. RustDesk
must preserve the exact typed repository-boundary rejection with empty stdout
and no publication. Reports retain every raw sample and all `NFR-PER-001`
fields, compare only on the same sanitized host profile, and reject semantic,
outcome, completeness, privacy, limit, or reviewed p95 violations.

The B1 runner performs no external acquisition or source mutation. Existing
base-controlled CI validates the committed benchmark contract but does not run
the public repositories. Retained local evidence contains no external source
or private path. `NFR-PER-002`, `OD-SLO-001`, release-artifact performance,
availability, support, conference validity, and GA remain unresolved.

## B1a benchmark-only execution-limit candidate

Issue [#206](https://github.com/smutti/codenoesis/issues/206) and
[Decision 0043](decisions/0043-benchmark-only-r16-execution-limit.md) define a
dependent high-risk S14 checkpoint on exact B1 head
`74bd959128472c3e95baeb5fd1a29cfc09b2c686`. The expected Red is the current
R16 rejection of the explicit `real-world-rust-benchmark-75s-v1` selector plus
the runner command's missing selector.

Green requires exact 60,000 millisecond selector-absent behavior, exact 75,000
millisecond selected behavior, maximum-plus-one failure without a timed test,
fail-closed invalid compositions, and selector-present/absent R16 semantic
identity. The B1 real-repository matrix then runs again without retry or sample
discard. Acquisition, RustDesk error precedence, runner timeout, observational
ceiling, schemas, goldens, ontology, dependencies, workflows, release, support,
and GA remain unchanged.
