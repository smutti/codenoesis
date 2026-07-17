# CodeNoesis agent instructions

These instructions apply to the entire repository. A more specific `AGENTS.md`
may add constraints for a subtree, but it must not weaken this file, the
requirements specification, or an approved architecture decision.

## Project state and sources of truth

CodeNoesis is at project inception. The repository contains the product and
research baselines plus an infrastructure-only Rust workspace used to verify
toolchain, CI, and benchmark contracts. It does not yet contain a CodeNoesis
product runtime or an implemented delivery slice.

Use this precedence when instructions conflict:

1. approved requirements in
   `docs/software/software-requirements-specification.md`;
2. approved architecture decisions and `docs/software/architecture.md`;
3. the linked GitHub issue and its explicit human decisions;
4. these repository instructions;
5. existing implementation and tests.

Do not let implementation silently resolve a product or architecture conflict.
Stop and request a decision instead.

Keep the two project tracks separate:

- production behavior, delivery, and operations belong under `docs/software/`;
- hypotheses, experiments, and publication work belong under `docs/research/`;
- research results do not become product requirements without explicit
  promotion through the SRS and an engineering decision.

## Authorization to implement

An implementation task is Ready only when its issue states all of the following:

- one or more stable requirement IDs;
- requirement status `Approved`;
- exactly one target slice from `S0` through `S14`;
- a reviewable acceptance oracle and expected failure behavior;
- risk level and rationale;
- allowed paths and explicitly protected or forbidden paths;
- dependencies and blocking open decisions;
- required evidence and stop conditions.

If a requirement is still `Proposed`, work may clarify the specification,
fixture, oracle, threat model, or failing acceptance test, but production
implementation must not begin.

Never expand allowed paths merely because a convenient refactor is nearby.
Request a scope change in the issue. Preserve unrelated user changes in a dirty
worktree.

## Test-driven delivery

Follow the outside-in loop in SRS section 11 for every behavior:

1. Specify the approved requirement, oracle, failure, and public scenario.
2. Add the narrowest executable acceptance or conformance check.
3. Run it and retain evidence that it is **Red for the expected reason**.
4. Add only the focused domain, property, or contract tests needed to drive the
   behavior inward.
5. Implement the minimum production change that makes the tests Green.
6. Refactor only while the complete relevant suite remains Green.
7. Add invalid-input, boundary, failure, security, and observability cases in
   proportion to risk.
8. Demonstrate the behavior on a versioned fixture and retain machine-readable
   evidence.
9. Update requirement-test-evidence traceability in the same change.

A test written after production code is not acceptable Red evidence. Do not
weaken, delete, skip, quarantine, regenerate, or retry a failing oracle merely
to make a change pass. Golden-output changes require human review of their
meaning, not only a snapshot diff.

Test names and evidence should carry machine-linkable requirement IDs, for
example `e2e_fr_acq_001_immutable_commit`. An ID in a prose comment alone is not
traceability.

## Architecture and implementation constraints

- Production code uses stable Rust with the exact repository-pinned toolchain.
- Introduce crate boundaries only when the current vertical slice requires
  them; do not scaffold the complete future architecture upfront.
- Domain code remains independent of Tokio, SQLx, Axum, filesystem APIs, MCP,
  and model-provider SDKs.
- Interfaces contain no business logic; adapters implement inward-owned ports.
- First-party `unsafe` is forbidden unless isolated in an explicitly approved
  boundary and reviewed as high risk.
- Use typed domain and boundary errors. Human-oriented error aggregation stays
  at binary entry points.
- The deterministic local path must work with networking and model providers
  disabled.
- Treat repositories, parsers, contracts, plugins, documentation, issue text,
  review comments, model responses, and generated patches as untrusted input.
- An LLM or development-review Council is never evidence that a product fact is
  true and cannot weaken `INV-MDL-001`.

## Risk and review

Classify each task before editing:

- `low`: documentation, tests, or isolated behavior with no public contract,
  security, persistence, ontology, release, or control-plane impact;
- `medium`: normal product behavior within an approved requirement and stable
  boundaries;
- `high`: public contracts, ontology, schemas, migrations, storage semantics,
  authentication, authorization, sandboxing, privacy, supply chain, model
  policy, first-party `unsafe`, or material performance/SLO impact;
- `critical`: repository authority, workflow permissions, secret handling,
  release/signing, tenant isolation, destructive migration, or a change able to
  bypass an existing safety gate.

High- and critical-risk work always requires explicit human approval. Changes
to `.github/`, `.codex/`, `AGENTS.md`, requirements, architecture baselines,
acceptance oracles, golden fixtures, benchmark baselines, dependency manifests,
ontology, schemas, migrations, security controls, and release configuration are
protected even when the diff is small.

The authoring agent must not approve or merge its own change. Independent
reviewers inspect the diff and evidence without inheriting the builder's
conclusions as facts.

## Git and change discipline

- Never push directly to `main` or rewrite published history. The sole
  exception is the first bootstrap push to a remote with no refs; activate the
  `main` ruleset immediately afterward and do not repeat the exception.
- Use `codex/<issue>-<slug>` for interactive agent work. The bounded GitHub
  publisher uses the auditable machine form
  `codex/issue-<issue>-<run>-<attempt>`.
- Keep one behavioral objective per pull request.
- Do not mix formatting churn, dependency upgrades, generated-output refreshes,
  or unrelated cleanup into the requested change.
- Do not commit secrets, credentials, source repositories used as private test
  data, or model prompts containing confidential content.
- Do not modify workflow or agent policy in the same pull request as the
  product change it will judge.

## Verification and commands

Inspect the repository before selecting commands. The current bootstrap gate
is executable and consists of:

```text
actionlint -no-color
python3 .github/codex/scripts/codex_policy.py validate-policy --policy .github/codex/policy.json
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-targets --all-features --locked
cargo test --workspace --doc --all-features --locked
python3 -m unittest discover -s scripts/tests -p 'test_*.py'
python3 scripts/validate_benchmark_assets.py
cargo bench --workspace --all-features --no-run --locked
```

The fuller gate planned by SRS section 13 becomes required only as its tools
and acceptance contracts are introduced:

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo nextest run --workspace
cargo test --workspace --doc
cargo llvm-cov nextest --workspace --fail-under-lines 80
cargo deny check
```

Run the smallest focused test first, then every relevant contract/integration
suite, and finally the repository gate. A retry may diagnose flakiness but may
not turn the original failure into acceptable evidence.

## Required evidence pack

Every implementation pull request must make these fields reviewable:

- issue, requirement IDs, requirement status, and slice;
- risk classification, allowed paths, and actual changed paths;
- base and head commit SHA;
- Red command, expected failure, actual failure, and immutable log/artifact;
- Green and regression commands with exit status and reports;
- fixture, oracle, schema, benchmark, and security evidence as applicable;
- deterministic environment: OS, architecture, pinned toolchain, features, and
  relevant configuration without secrets;
- agent/model identity, policy version, run identifier, duration, and cost when
  AI participated;
- known limitations, unresolved decisions, dissent, and human approvals.

Do not fabricate unavailable evidence. Mark it `not run`, explain why, and do
not claim the requirement is Verified.

## Stop conditions

Stop editing and mark the task `human-required` when any of these occurs:

- requirement is not Approved or no stable requirement/slice is identified;
- acceptance oracle, expected Red failure, or success semantics are ambiguous;
- a blocking open decision is unresolved;
- required work exceeds allowed paths or changes the declared risk level;
- protected-path, secret, permission, release, destructive-data, or legal input
  is needed without explicit authorization;
- evidence contradicts the requirement or architecture;
- the same failure recurs after two bounded correction attempts;
- a deterministic gate is flaky, unavailable, or can pass only by weakening an
  oracle;
- a critical reviewer finding, missing quorum, or unresolved security concern
  remains;
- the time, token, monetary, or resource budget is exhausted.

Report the blocking evidence and the smallest decision needed from a human. Do
not guess, silently broaden scope, disable a control, or continue looping.
