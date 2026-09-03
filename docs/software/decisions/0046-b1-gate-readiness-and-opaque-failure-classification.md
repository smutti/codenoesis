# Decision 0046: B1 gate readiness and opaque-failure classification

- Status: Proposed branch-scoped candidate
- Date: 2026-09-03
- Issue: [#209](https://github.com/smutti/codenoesis/issues/209)
- Authorization: accountable-maintainer authorization in issue #209
- Exact dependent base: `09dd9df9c142ffbdea3b6e20c4cbeb0e7a9dd2d9`
- Parent packages: issues #205-#208 and Decisions 0042-0045
- Requirements: bounded corrections to Approved `NFR-PER-001`,
  `NFR-OBS-001`, `NFR-TST-001/002`, `NFR-DET-001`, `NFR-MNT-002`,
  `NFR-SEC-005`, and `INV-BND-001`
- Slice: `S14`
- Risk: high
- Dependencies: none
- Correction budget: five rounds
- Owner and approver: `@smutti`

## Context

B1c reproduced the Lekton failure on its first bounded diagnostic attempt and
retained product exit `12`, empty stdout, stderr length `239`, and stderr digest
without retaining source or stderr content. The strict parser could not select
public product schema/code/stage, so the identity remained `unparseable`.

The same review head exposed two unrelated gate-lifecycle defects. The B1a
selector extended `parse_r16_invocation` to 106 lines while the pinned clippy
limit is 100. The frozen LocalBaselineVerificationV2/V3 validators correctly
protect their historical packages but incorrectly compare those packages with
mutable descendant worktree documents, paths, product trees, and evidence.

## Decision

Extract only B1a execution-limit selector parsing from `parse_r16_invocation`
into a private helper. This is the only permitted Rust source difference after
the `cce8486` B1 baseline. Argument and error precedence, invalid UTF-8,
selector-absent behavior, exact accepted composition, 60,000/75,000 millisecond
limits, extraction, ontology, identities, hashes, counts, stores, publication,
and every non-B1 invocation remain unchanged. Baseline evidence retains exact
binary SHA-256 `d1dc7785cad652e674c50565f66ff0a23ec15e85af2842c9107abff84775b247`;
candidate evidence identifies its final review-head build honestly and proves
behavior rather than byte identity.

Keep every V2/V3 schema, plan, catalog, manifest, activation record, retained
log, remote-run identity, digest, and Verified conclusion byte immutable.
Correct only validator/test lifecycle resolution so a legitimate descendant
validates each frozen package against exact historical Git subjects and status
document bytes rather than current worktree bytes. Mutation, substitution,
non-descendant, missing-history, wrong-subject, wrong-digest, and
self-activation cases remain fail closed. Public validator output remains
canonical.

For B1c `product=unparseable`, append exactly one closed validation category:
`empty`, `oversized`, `non_utf8`, `invalid_json`, `wrong_shape`, `noncanonical`,
`unsupported_schema`, `unsafe_code`, `unsafe_stage`, or `inconsistent_stage`.
A selected public typed identity uses `accepted`. The ASCII message remains at
most 256 bytes and never emits stderr content or parsed values, product
message/context, source, path, host, user, environment, command, token, or URL.
Existing non-sample errors remain byte unchanged.

After Green, run exactly one additional independent Lekton diagnostic using
the same clean clone, exact baseline binary, fresh marker-owned state, and
90-second timeout. Do not retry. Retain only B1c fields plus the closed category
and stop; do not run RustDesk, candidate comparison, or B1 acceptance.

## Compatibility and activation

No dependency, Cargo, workflow, control-plane, benchmark corpus, policy,
oracle, baseline, report/comparison schema, ontology, schema, golden, product
fixture, threshold, timeout, retry, release, signing, publication, support,
SLO, cross-host, conference, or GA authority changes. This candidate becomes
effective only after independent review and protected manual merge.
