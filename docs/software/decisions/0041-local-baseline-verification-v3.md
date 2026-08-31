# Decision 0041: Local baseline verification V3

- Status: Proposed verification candidate
- Date: 2026-08-31
- Issue: [#201](https://github.com/smutti/codenoesis/issues/201)
- Authorization: [maintainer decision](https://github.com/smutti/codenoesis/issues/201#issuecomment-5477071815)
- Exact base: `c783b612777a86e2f88620ece987723bb230c51c`
- Delivery slice: `S14`
- Risk: high

## Context

Protected PR #189 activated the immutable 32-profile
`LocalBaselineVerificationV2` baseline. Protected PR #191 subsequently made
R18 trusted local evidence-to-source retrieval Approved and Implemented, and
protected PR #197 made R19 Git-backed implementation-aware semantic impact
Approved and Implemented. Neither additive capability is independently
Verified on the exact current base.

The V2 plan, catalog, schema, validator, evidence, activation identities,
product-tree identity, and historical bytes remain accepted and immutable.
The product behavior, ontology, query and explorer contracts, fixtures,
goldens, workflows, release authority, support, and GA status are not reopened.

## Decision

Adopt `LocalBaselineVerificationV3` as one verification-only S14 package. Its
resolved catalog contains exactly 34 ordered profiles: the exact 32 V2 entries
bound by the accepted V2 catalog digest, followed only by
`r18-trusted-local-source` and `r19-git-backed-semantic-impact`. The two
additive entries bind their accepted requirements, issues, decisions,
implementation pull requests, contracts, schemas, retained Red and Green
evidence, correction history, traceability, platform runs, and protected merge
identities.

The public acceptance command is:

```text
python3 scripts/verify_local_baseline_v3.py --manifest tests/evidence/verification/local-baseline-v3/manifest.json
```

The validator must fail closed on an omission, substitution, duplicate,
ordering error, digest drift, unsafe or private path, missing Git identity,
missing or non-Green remote run, dangling evidence, product-tree change,
lifecycle disagreement, partial acceptance, V2 mutation, or unsupported G9,
release, support, publication, or GA implication. It recomputes repository
digests and Git identities instead of trusting prose. Its successful output is
canonical JSON plus LF and contains no secret or private path.

Before merge, the only valid status is
`candidate_verified_pending_merge`. The authoring agent cannot independently
verify its own package. Only independent review and protected manual merge of
the exact evidence-complete head make the 34 catalogued profiles Verified.

## Fixed identities

R18 is bound to issue #190, Decision 0038, review head
`16ef5ceaea6ad14d9838f84856f6ca3d445daa67`, merge
`fcdd6eddec8a4dd9b372cb88ff424c2004b5c88b`, CI run `32229313149`, benchmark
run `32229313162`, CodeQL run `32229311917`, and review-policy run
`32229313006`.

R19 is bound to issue #196, Decision 0040, review head
`c3cbced9ee2017b61ec8e0b10191553edc733004`, merge
`c783b612777a86e2f88620ece987723bb230c51c`, CI run `33203760261`, benchmark
run `33203760248`, CodeQL run `33203757887`, review-policy run `33203758766`,
Windows job `98959469058`, and final CI policy job `98965049693`.

## Required sequence and evidence

1. Commit this decision, the V3 plan, resolved-catalog contract, closed
   manifest schema, candidate lifecycle wording, and focused conformance test
   before the V3 validator or evidence manifest exists.
2. Run and retain the issue's exact expected Red. It is acceptable only when
   the V3 validator and canonical 34-profile manifest are absent.
3. Add the minimum standard-library validator, immutable evidence pack,
   normalized remote logs, focused Green evidence, and traceability.
4. Run the focused suite, public V3 command, unchanged V2 validation, complete
   repository gate, and unchanged base-controlled GitHub checks.
5. Obtain independent review and protected manual merge without rewriting the
   checkpoint or Red history.

The evidence pack must bind the issue, risk, allowed and actual paths, exact
base/checkpoint/Red/evidence/review heads, product-tree proof, all 34 profiles,
V2 inheritance, R18/R19 contracts and evidence, remote runs and normalized
logs, environment, toolchain, agent/policy metadata, limitations, corrections,
and independent review. Unavailable evidence is a failure, not a prose waiver.

## Authority boundaries

This decision grants no product Rust, dependency, ontology, schema, fixture,
golden, workflow, permission, secret, signing, tag, publication, deployment,
release, support, EOL, SLA, server, model, adapter, or GA change. It grants no
self-approval, self-merge, partial verification, or history rewrite. The only
allowed paths and five-round correction budget are those fixed by issue #201.
Rollback is a revert of this verification-only pull request.

## Stop conditions

Stop for a new maintainer decision if a profile, requirement, lifecycle
meaning, oracle, evidence class, remote identity, product-tree rule, risk,
dependency, path, authority, or release effect changes; any V2 or product byte
must change; required evidence is unavailable, contradictory, flaky, private,
or non-reproducible; the expected Red has another cause; a gate can pass only
by weakening completeness; or five correction rounds are exhausted.
