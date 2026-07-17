## Outcome

Describe the single demonstrable behavior delivered by this change. Link the
agent-ready issue with `Closes #...` only when this PR fully satisfies it.

## Traceability

| Field | Value |
|---|---|
| Issue | |
| Requirement IDs | |
| Requirement status | Approved / Proposed-only work |
| Slice | `S?` |
| Risk | low / medium / high / critical |
| Base SHA | |
| Head SHA | |

## Scope

Allowed paths from the issue:

- _Da compilare_

Actual changed paths:

- _Da compilare_

Protected paths touched, approval, and reason:

- None

Out of scope and intentionally unchanged:

- _Da compilare_

## TDD proof

### Red

| Field | Evidence |
|---|---|
| Acceptance/conformance test | |
| Command | |
| Expected failure | |
| Observed failure | |
| Pre-implementation SHA | |
| Immutable log or artifact | |

### Green and regression

| Command or check | Result | Evidence |
|---|---|---|
| Focused behavior | | |
| Unit/property/contract suites | | |
| Pull-request gate | | |
| Security/failure cases | | |
| Benchmark base versus head | | |

Use `not run — <reason>` for unavailable evidence. Do not describe an unrun
check as passing.

## Acceptance and evidence pack

- Fixture and oracle:
- Requirement-test-evidence link:
- Schema, artifact, or compatibility evidence:
- Boundary, failure, privacy, and security evidence:
- Determinism or reproducibility evidence:
- Benchmark manifest and regression decision:
- Environment: OS, architecture, toolchain, features, configuration:
- Known limitations, unknowns, and coverage gaps:

## Autonomous run record

Complete this section whenever AI authored, reviewed, or corrected the change.
Do not include hidden reasoning, secrets, or private source content.

| Field | Value |
|---|---|
| Actor/agent role | |
| Model and reasoning profile | |
| Policy/prompt version | |
| Run ID | |
| Correction rounds | |
| Duration and model cost/token usage | |
| Patch/evidence artifact digest | |
| Reviewer dissent or escalation | |

## Human decisions

List approvals for requirement state, risk, protected paths, oracle/golden
changes, compatibility, waivers, or open decisions. Write `None` only when none
is required.

- _Da compilare_

## Author checklist

- [ ] The linked requirement and slice are explicit; production code is tied to an Approved requirement.
- [ ] Red was observed for the expected reason before production implementation.
- [ ] The implementation is the smallest change that satisfies the approved behavior.
- [ ] Invalid input, boundaries, failure, security, and observability are covered in proportion to risk.
- [ ] No required failure was retried, skipped, quarantined, or weakened into success.
- [ ] Golden or benchmark baseline changes were reviewed semantically, not merely regenerated.
- [ ] Documentation and machine-readable traceability changed with the behavior.
- [ ] No secrets, credentials, private corpus, or confidential prompt content are present.
- [ ] The authoring agent has not approved or merged its own change.
- [ ] Unavailable checks and unresolved decisions are stated explicitly.
