# Development workflow

> Status: **simplified delivery active**. CodeNoesis uses deterministic CI and
> an optional read-only AI review. Automated proposal publication, risk tiers,
> path authorization, and evidence ceremonies are not part of the workflow.

## Objective

Move from an agreed objective to working, reviewable software with the minimum
process needed to preserve correctness, security, and reproducibility.

## One pull request delivery

Each coherent outcome follows this sequence:

1. **Plan** the objective, acceptance criteria, and non-goals.
2. **Tests** define the observable behavior and important failure cases.
3. **Benchmark** performance, scale, or real-repository behavior when relevant;
   otherwise record `not applicable`.
4. **Implementation** adds the minimum complete behavior.
5. **Technical validation** runs focused checks and then the relevant full CI
   gate.
6. **Pull request** presents the outcome, results, and limitations for manual
   merge.

The same pull request may update product code, requirements, architecture,
ontology, schemas, fixtures, documentation, dependencies, control plane,
workflows, permissions, signing configuration, and release configuration when
they belong to the same outcome. There is no mandatory governance checkpoint,
separate governance pull request, retained expected Red evidence, risk label,
exact path declaration, base-SHA authorization, or correction-round budget.

Tests should normally precede or accompany implementation, but the repository
does not require a preserved failing commit or log. Correctness is established
by reviewable tests, benchmarks where relevant, and green deterministic gates.

## Technical controls

The controls that remain are executable engineering checks:

- formatting and static analysis;
- unit, integration, contract, documentation, failure, and security tests;
- deterministic fixtures and semantic review of golden changes;
- benchmark metadata validation and regression checks;
- dependency locking and architecture boundary tests;
- read-only handling of untrusted repositories and pull-request content;
- branch protection, no direct push, and manual merge.

These controls validate the product. They do not require repeated permission to
edit a category of file.

## Human intervention

Ordinary scope refinements, governance edits, dependency updates, test fixes,
and CI corrections continue in the same pull request. Human confirmation is
needed only for a genuinely ambiguous product decision or before an operation
that uses secrets, publishes, signs, deploys, releases, destroys data, or has an
unresolved legal/licensing impact.

The authoring agent never pushes directly to `main`, approves its own pull
request, or merges it.

## Read-only AI review activation

`.github/workflows/codex-review.yml` can provide a static second pass over an
immutable pull-request head. It is optional and disabled unless:

- repository variable `CODEX_REVIEW_ENABLED` is exactly `true`; and
- environment `codex-model` contains `OPENAI_API_KEY`.

Without that environment secret, the review switch must remain `false`. The
model job receives read-only repository access, no write token, and no merge or
release authority. It does not execute repository code or network commands.
Its structured result is validated before a separate job posts the comment.

AI review is skipped when a change exceeds the workflow's technical file-count
capacity. It is not skipped merely because the pull request changes governance,
workflows, ontology, schemas, dependencies, benchmarks, or release files.

## Pull-request record

The pull-request description is intentionally small:

- objective and observable result;
- tests and technical gates run;
- benchmark result or `not applicable`;
- compatibility, security, migration, or operational impact where relevant;
- known limitations and checks not run.

CI logs and benchmark artifacts are the execution record. Separate evidence
packs, model-cost ledgers, pre-implementation log digests, and approval matrices
are not required.

## Failure handling

Fix ordinary implementation, test, benchmark, documentation, and CI findings in
the existing pull request. Stop only when behavior is ambiguous, the requested
operation is irreversible or privileged, user work conflicts, or a required
deterministic gate remains unavailable or repeatedly fails outside the change.
