# Contributing to CodeNoesis

Read `AGENTS.md` before changing the repository. CodeNoesis uses one simple
delivery loop for human and AI contributors:

1. plan the objective, observable acceptance criteria, and non-goals;
2. add or update focused automated tests;
3. add or run a benchmark when performance, scale, or real repositories are
   affected, otherwise mark it `not applicable`;
4. implement the smallest complete change;
5. run focused checks and the relevant full technical gate;
6. open one pull request for the coherent outcome.

An issue, requirement ID, delivery slice, risk classification, path allowlist,
retained Red proof, correction budget, or separate governance pull request is
not required. Product code may share a pull request with required SRS,
architecture, ontology, schema, fixture, dependency, documentation, workflow,
permission, or release-configuration changes.

## Tracks

- Put product behavior, architecture, implementation, tests, operations, and
  delivery documentation under `docs/software/` and the relevant source tree.
- Put hypotheses, experimental methods, scientific benchmarks, and publication
  material under `docs/research/`.
- Promote research findings into product behavior through the same reviewed
  pull request as the implementation they justify.

## Tests and benchmarks

- Prefer public acceptance or contract tests over private implementation
  details.
- Add boundary, invalid-input, security, and failure cases where they are
  relevant to the behavior.
- Keep required tests deterministic and parallel-safe.
- Do not weaken or regenerate an oracle merely to make a change pass.
- Review golden and benchmark baseline changes semantically.
- Use the commands listed in `AGENTS.md`; report any check not run.

## Pull requests

- Keep one coherent objective per pull request.
- Describe the outcome, tests, benchmark status, compatibility/security impact,
  known limitations, and checks not run.
- Never include secrets, credentials, private corpora, or confidential prompts.
- Never push directly to `main`, rewrite protected history, self-approve, or
  self-merge.
- Deterministic CI remains the merge gate. Optional AI review is read-only and
  does not replace maintainer judgment.

## Security and license

Follow `SECURITY.md` and treat repository inputs as untrusted. Contributions are
provided under the [Apache License, Version 2.0](LICENSE).
