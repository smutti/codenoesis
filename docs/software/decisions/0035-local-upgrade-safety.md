# Decision 0035: Local Upgrade Safety

- Status: Proposed branch-scoped candidate
- Date: 2026-08-14
- Issue: [#184](https://github.com/smutti/codenoesis/issues/184)
- Authorization: accountable-maintainer authorization for the complete Local Upgrade Safety G2/G5-local/G7/S14 package
- Exact base: `e7643d83965dca2f9342080264e7c6c58f3dd761`
- Requirements: Proposed `FR-CMP-001`, `FR-CLI-009`, and bounded amendments listed in issue #184
- Planning package: `G2a/G5-local/G7a`
- Slice: `S14`
- Risk: high
- Owner and approver: `@smutti`

## Context

Protected PR #183 made the exact G1a fixed configuration and unsigned staged
directory Approved and Implemented but not Verified. Its lifecycle describes
side-by-side installation and caller-owned rollback, but it intentionally makes
no compatibility promise and provides no executable transition preflight.

## Decision

Add output-only `preflight-local-upgrade` and `preflight-local-rollback`
commands to the repository maintenance adapter. Add inward-owned canonical
`LocalUpgradePlanV1`, `LocalRollbackReportV1`, and `CodeNoesisErrorV27`.

Upgrade accepts exactly two distinct complete G1a bundles. It validates their
stable non-symlink trees, canonical manifests, exact names, target, profile,
configuration schema, fixed payloads, modes, lengths and SHA-256 values. It
classifies only the identical V1 fixed-policy transition as compatible and as
requiring no migration. It emits a plan binding both manifest and binary
digests, caller-owned activation, exact-prior rollback and explicit absence of
automatic update, signing, publication, support and GA.

Rollback accepts only that exact canonical plan, the exact candidate bundle as
current and the exact prior bundle as target. Arbitrary downgrade, a third
bundle, substitution, tampering, unsupported schema/profile/target, path/race
failure or excess input is rejected rather than inferred or repaired.

## Security and authority

Both commands are read-only except for stdout/stderr. They execute no binary,
open no network, read no secret source, discover no ambient configuration,
mutate no invocation path or installation tree, create no hidden pointer, and
perform no migration, signing, publication or release action. Errors contain no
input path or private value.

## Bounds and determinism

Each transition has exactly two six-file G1a bundles and inherits every G1a
file and binary maximum. Plan input/output is at most 65,536 bytes. Fifty
argument constructions and ten schedules must be byte-identical. Maximum and
maximum-plus-one, malformed, privacy, tamper and race cases fail closed.

## Performance evidence

One dependency-free observational runner measures at least 30 warm
single-threaded fixture preflights and retains raw nanoseconds, nearest-rank
p50/p95/p99, success rate and all `NFR-PER-001` context. It establishes no SLO
or regression threshold and leaves `NFR-PER-002`, `OD-SLO-001` and the global
benchmark manifest unresolved.

## Red and lifecycle

The checkpoint includes governance, strict schemas, target goldens, invalid
oracle, Python guard and black-box tests before implementation. On the exact
base, upgrade exits 2 with empty stdout and canonical G1a ErrorV26
`distribution.invalid_arguments`; that is the sole acceptable Red.

Before protected merge the requirements and implementation remain Proposed.
Merge activates only the exact output-only preflight contracts. Independent
evidence acceptance is still required for Verified status. G2b, full G5, G8,
G9, support, signed distribution and GA remain separate.

## Consequences

- Users can prove that two exact experimental bundles form a supported-by-this-contract side-by-side pair without executing them.
- Rollback is bound to the exact prior plan instead of inferred ordering.
- Unsupported future contracts fail closed and remain future migration work.
- Existing G0/G1 and R0-R17/K1/S7 product bytes remain unchanged.
- Reverting the package removes only additive preflight behavior and evidence.
