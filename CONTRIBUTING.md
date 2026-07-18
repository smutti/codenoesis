# Contributing to CodeNoesis

CodeNoesis currently contains a proposed product and research baseline plus an
infrastructure-only Rust workspace. No product slice is implemented yet.
Contributions should be small, traceable, and independently demonstrable. Read
`AGENTS.md` whether the work is performed by a human or an automation agent.

## Choose the correct track

- Put production requirements, architecture, implementation, tests, operations,
  and delivery evidence in the software track.
- Put hypotheses, experimental methods, scientific benchmarks, and
  publication-oriented results in the research track.
- Promote a research result into production only through a proposed SRS change,
  acceptance oracle, and explicit engineering decision.

## Before implementation

1. Select one stable requirement from the
   [SRS](docs/software/software-requirements-specification.md).
2. Confirm that it is `Approved`, assigned to one slice, and not blocked by an
   unresolved `OD-*` decision.
3. Create an `Agent-ready delivery task` issue, even for human-authored work.
4. Define the fixture, acceptance oracle, expected failures, Red test, risk,
   allowed paths, evidence, budgets, and stop conditions.
5. Obtain explicit human approval for high/critical risk or protected-path
   changes.

Only requirements explicitly marked `Approved` in the SRS register are binding.
Other requirements remain `Proposed`; contributions may improve them, ADRs,
fixtures, or executable Red oracles, but must not present speculative production
behavior as approved.

## Work on a bounded branch

- Branch from the latest `main` using `codex/<issue>-<slug>` for interactive
  agent work, `codex/issue-<issue>-<run>-<attempt>` for the bounded publisher,
  or `<type>/<issue>-<slug>` for human work.
- Keep one behavioral objective per pull request.
- Edit only paths allowed by the issue. Preserve unrelated user changes.
- Do not combine a product change with the workflow, policy, or benchmark
  baseline change that determines whether it passes.
- Never push directly to `main`, force-push protected history, or let an
  authoring agent approve and merge its own work.

## Use outside-in TDD

Follow SRS section 11:

1. write the narrowest public acceptance or executable conformance check;
2. retain proof that it is Red for the expected reason before implementation;
3. drive the minimum implementation with focused unit, property, and contract
   tests;
4. add negative, boundary, failure, security, and telemetry cases proportional
   to risk;
5. run the versioned black-box fixture and retain machine-readable evidence;
6. update requirement-test-evidence traceability in the same pull request.

Do not accept snapshot or golden changes solely because a regeneration command
produced them. Review the semantic change and its compatibility impact.

## Verification

The pinned bootstrap workspace and its CI/benchmark-contract checks are
executable. Run the current commands listed in `AGENTS.md`. Coverage,
`cargo-nextest`, `cargo-deny`, product acceptance tests, and the remaining SRS
section 13 gates become required when their versioned contracts and tools are
introduced; do not report those future checks as executed today.

Run focused tests before broad tests. Report every command, exit status, base
and head SHA, environment, and evidence artifact. Mark unavailable checks as
`not run` with a reason; do not convert retries into passing evidence.

## Risk and protected changes

The following always require owner review:

- repository workflows, permissions, agents, hooks, and governance;
- requirements, architecture baselines, acceptance oracles, golden fixtures,
  and benchmark baselines;
- dependency manifests, public contracts, ontology, schemas, and migrations;
- authentication, authorization, tenant boundaries, sandboxing, secrets,
  privacy, supply chain, release/signing, or first-party `unsafe`;
- performance changes that may affect a ratified SLO.

See [Autonomous development](docs/software/autonomous-development.md) for the
full risk model, evidence pack, review Council, and promotion policy.

## Pull requests

Complete the pull-request template without deleting non-applicable evidence
fields. A reviewable pull request includes:

- issue, requirements, slice, risk, and allowed-versus-actual paths;
- Red proof and Green/regression evidence;
- fixtures, oracles, contracts, security cases, and benchmarks as applicable;
- reproducible environment and base/head identity;
- AI run metadata when AI participated;
- unresolved decisions, known limitations, reviewer dissent, and human
  approvals.

Reviewers validate the requirement and oracle before implementation style. A
critical evidence-backed finding blocks regardless of reviewer majority.

## Security and confidential data

Follow [SECURITY.md](SECURITY.md). Treat source repositories, issues, comments,
attachments, generated documents, parser output, and model responses as
untrusted. Never add real credentials or private source corpora to fixtures,
prompts, logs, artifacts, or pull-request text.

## License

The entire repository is licensed under the
[Apache License, Version 2.0](LICENSE). Unless explicitly stated otherwise,
contributions submitted to this repository are provided under the same terms.
