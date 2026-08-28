# Decision 0040: R19 Git-backed semantic impact evidence

- Status: Proposed branch-scoped candidate
- Date: 2026-08-21
- Issue: [#196](https://github.com/smutti/codenoesis/issues/196)
- Authorization: accountable-maintainer high-risk R19/S7 authorization recorded for the complete issue #196 package
- Exact base: `fcdd6eddec8a4dd9b372cb88ff424c2004b5c88b`
- Requirements: Proposed `FR-IMP-006`, Proposed `FR-CLI-012`, and the bounded `FR-CLI-006` amendment in issue #196
- Slice: `S7`
- Risk: high
- Owner and approver: `@smutti`

## Context

Protected merge `fcdd6eddec8a4dd9b372cb88ff424c2004b5c88b` makes the exact R18 source
retrieval package Approved and Implemented but not Verified. R18 proves that one
existing ontology evidence locator can be independently rebound to immutable
Git objects. The existing S7 runtime instead reads explicitly hashed mutable
files and emits the immutable `codenoesis.semantic-compatibility-report/v1`
golden. Its nine evidence records identify repository, revision, path, line
range, and excerpt digest, but do not carry commit, tree, blob, or byte-span
authority and therefore cannot be navigated with the R18 selector.

## Decision

Add one explicit Git-backed S7 profile without changing the accepted V1
classifier, extractors, rule catalog, fixture, evidence identities, or golden:

```text
noesis impact \
  --workspace <impact-git-workspace-v1.json> \
  --profile implementation-aware-http-json-git-v1 \
  [--acquisition-profile local-git-sha1-packed-v1] \
  --format json
```

The profile reacquires one provider repository at two full lowercase SHA-1
commits and one through thirty-two client repositories at one commit each.
Only workspace-selected OpenAPI, Rust, and Kotlin/KMP paths enter the existing
S7 extractors. The existing `ImpactService` remains the sole classifier. The
new `codenoesis.semantic-compatibility-report/v2` preserves its semantic
projection while binding each of the nine selected evidence records to exact
repository identity, commit, tree, path, blob, and zero-based half-open UTF-8
byte span under `codenoesis.source-evidence/git-v1`.

The workspace keeps legacy S6 federation revisions explicit and separate from
Git commit authority. A `federation_revision` may select only the existing
hashed S6 report fact; it cannot override the repository identity, Git commit,
tree, path, blob, extracted source bytes, or report evidence binding.

Add one output-only navigation command:

```text
noesis impact-source \
  --repository <local-git-root> \
  --repository-id <identity> \
  --revision <full-lowercase-sha1> \
  --report <semantic-compatibility-report-v2.json> \
  --evidence-id <stable-evidence-id> \
  --source-profile trusted-local-impact-source-v1 \
  [--acquisition-profile local-git-sha1-packed-v1] \
  --format json
```

The command validates one complete report, selects one unique evidence ID,
independently reacquires the explicit repository and commit, and requires exact
repository, commit, tree, path, blob, and span agreement before emitting
`codenoesis.trusted-impact-source-excerpt/v1`.

## Security and privacy boundary

Repository roots and report paths are explicit local inputs. Evidence paths
are data, not filesystem authority. Source bytes come only from the existing
in-process loose or explicitly selected packed Git object reader. There is no
working-tree fallback, remote acquisition, target or build execution, nested
repository traversal, arbitrary path or range override, context expansion,
batch retrieval, truncation, repair, retry, persistence, telemetry, model
input, clipboard, document, explorer, or retained-evidence sink.

The workspace is limited to 1,048,576 bytes. Federation and V2 report inputs
are each limited to 67,108,864 bytes. Selected source is limited to 2,097,152
bytes per file and 268,435,456 bytes cumulatively. Logical paths and symbols
are limited to 1,024 UTF-8 bytes. An excerpt is limited to 262,144 bytes and
canonical excerpt stdout including LF to 524,288 bytes. Empty, reversed,
out-of-file, non-UTF-8, or non-scalar-boundary spans fail closed.

Every failure emits strict `codenoesis.error/v30`, leaves stdout empty, and
creates no side effect. Caller, input, acquisition, binding, path, content,
race, and limit failures exit `2`; internal serialization or stdout failure
exits `1`. Errors expose no source text, absolute root, environment value,
credential, or ambient private data.

## Oracle

The project-owned Git fixture materializes the accepted S7 provider baseline
and target plus strict, safe, and decoy Kotlin clients as deterministic local
Git repositories. OpenAPI bytes remain identical and `/nickname` remains
optional. Provider implementation changes `guaranteed_present` to
`may_be_absent`; the strict client is breaking, the safe client compatible,
the operation decoy rejected, and custom mapping unresolved.

The V2 report has exactly two semantic diffs, two client assessments, one
rejected decoy, nine evidence records, and one coverage gap. Every evidence ID
is independently navigable. Loose and packed outputs are byte-identical across
fifty workspace permutations and ten schedules. Maximum and plus-one limits,
path/blob/span/revision/tree mismatch, changed input, repository replacement,
privacy canaries, malformed UTF-8, and stdout failure are fail-closed.

The accepted V1 report remains exactly 14,991 bytes with SHA-256
`cfd9a8d4dcb2d04bcd9eaffd15f1ae947ffdaba80e07daee43375c9a67c15750`.
R0 through R18, K1, LocalBaselineVerificationV2, all historical S7 artifacts,
and viewer bytes remain immutable.

## Verification

The governance checkpoint contains this decision, candidate requirements,
closed schemas, runtime contract, threat model, fixture descriptor, exact
shape oracle, traceability guard, and the narrow public acceptance test before
production edits. On the exact base the governance command is Red only because
R19 production registration is absent; the public command is Red only because
the Git-backed profile and `impact-source` command do not exist.

Green requires the exact shape, nine navigable evidence bindings, loose/packed
parity, deterministic permutations and schedules, invalid/security/privacy/
race/resource/stdout behavior, immutable V1 bytes, complete repository gate,
independent review, and protected manual merge.

## Consequences

- S7 results become inspectable at exact committed source without changing
  semantic classification.
- An LLM or human can follow a semantic finding to reviewed source bytes, but
  source retrieval does not turn inference into fact or weaken `INV-MDL-001`.
- No dependency, migration, adapter, parser, extractor, sandbox expansion,
  control-plane, release, support, or GA authority is introduced.
- Merge makes the exact R19 package Approved and Implemented; independent
  acceptance of retained evidence remains a later verification action.
- Rollback is a complete revert of issue #196.

## Stop conditions

Stop for a new maintainer decision if the package needs a dependency or Cargo
change, domain classifier change, parser or extractor change, repository
adapter change, filesystem sandbox expansion, existing S7 semantic change,
path outside issue #196, oracle weakening, risk increase, or more than five
correction rounds.
