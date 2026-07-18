# Codex GitHub automation

The workflows in this directory keep model access and repository write access
in separate jobs.

Required protected environment secrets:

- environment `codex-model`: `OPENAI_API_KEY`, available only to the static
  review job and the patch-producing job;
- environment `codex-publisher`: `CODEX_PUBLISHER_PRIVATE_KEY`, used only by
  the patch publisher and never by a model job.

Both environments must allow deployments only from `main`. Keep these as
environment secrets, not repository or organization secrets: a workflow
selected from another ref must be unable to request them. Initially require an
owner approval on `codex-publisher`; it can be relaxed only through the
documented autonomy-promotion process.

Required repository variable:

- `CODEX_AUTOMATION_ENABLED=true`: explicit kill switch. Key-bearing jobs stay
  skipped until this value is present.
- `CODEX_PUBLISHER_APP_ID`: numeric ID of the dedicated publisher GitHub App.
- `CODEX_PUBLISHER_BOT_LOGIN`: App slug used to authorize its bot-triggered
  review runs, without the trailing `[bot]` suffix.

Install the App only on this repository and grant it repository permissions
`Contents: Read and write` and `Pull requests: Read and write`. Do not grant
workflow, administration, secrets, Actions, or checks permissions. The
workflow requests a short-lived token narrowed to the current repository and
those two permissions; the token is revoked when the job finishes. Using the
App instead of `GITHUB_TOKEN` ensures that its branch push and draft PR trigger
the normal CI and review events.

Do not set `CODEX_AUTOMATION_ENABLED=true` and do not install the publisher App
until the `main` ruleset is active, direct pushes are prohibited, and the App
has no ruleset or branch-protection bypass. `Contents: write` is required to
publish proposal branches, so the protected default branch is the final
technical barrier against an unintended update to `main`.

Third-party Actions are pinned to reviewed immutable commit SHAs. Codex Action
also pins Codex CLI and its Responses API proxy to version `0.144.5`; update the
Action and CLI pins together through a separately reviewed control-plane PR.
The workflows use built-in `:read-only` and `:workspace` permission profiles;
the repository config intentionally does not override their sandbox mode.

Initial autonomy policy:

- proposal runs are manual and start from the default branch;
- proposal issues must satisfy the complete agent-ready form with an Approved
  requirement present in the owner-reviewed policy registry, and both sides of
  changed paths are checked against issue scope and canonical hard denials;
- initial A1 proposal automation accepts only low- and medium-risk work;
  high/critical work remains human-supervised and cannot mint a publisher token;
- review runs are read-only and ignore forks unless a trusted maintainer copies
  the change to an in-repository branch;
- model jobs cannot push or comment;
- publisher jobs never receive the OpenAI key and their built-in
  `GITHUB_TOKEN` is read-only;
- automation never merges a pull request;
- autonomous repair iterations are set to zero;
- runs are bounded by time, patch size, file count, and concurrency limits;
- generated pull requests stay draft and proposal-only until a maintainer
  completes the Red/Green evidence pack; the initial workflow does not claim
  merge-ready TDD proof.

## Remote activation order

1. Push the disabled workflows and let `CI` and `Benchmark integrity` complete
   on `main`; exercise `Codex review policy gate` on a synthetic test PR.
2. Protect `main` with a ruleset that requires a pull request, conversation
   resolution, linear history, squash-only merge, CodeQL/code-quality gates,
   and the exact stable checks `CI policy gate` and `Benchmark policy gate`,
   with strict up-to-date-before-merge checking. Block force pushes and
   deletions. The current single-maintainer bootstrap sets required approvals to
   zero and disables Code Owner review because `@smutti` cannot approve a pull
   request created by the same account; the manual merge remains exclusively
   human and the AI review remains advisory. When a second write-capable human
   joins, restore one approval and Code Owner review before the next protected
   governance change. Do not enable merge queue until these workflows
   explicitly support `merge_group`.
3. Keep repository administrators and the publisher App out of every bypass
   list. Protect release tags matching `v*` as well.
4. Create the two branch-restricted environments, then create and install the
   dedicated publisher App with only the permissions documented above. Add its
   two variables and environment private-key secret.
5. Add the model environment secret. Temporarily enable the kill switch inside
   this protected setup and prove that a `Proposed` or unregistered requirement
   is rejected before any model call; clear it immediately after the negative
   test. Then add one formally approved S0
   requirement to `policy.json` through an owner-reviewed PR, exercise proposal
   and review against synthetic non-sensitive input, and inspect every artifact.
6. Set `CODEX_AUTOMATION_ENABLED=true` only after the controlled positive smoke test and owner
   review. Clearing or changing this variable away from `true` is the immediate
   kill switch.

Emergency shutdown is: clear the kill switch, cancel active workflow runs,
uninstall the publisher App from this repository, revoke/delete its private
key, and rotate the model key if disclosure is suspected. Retain the affected
run IDs and artifacts for incident review.

Do not treat AI review as human approval. In the disclosed single-maintainer
bootstrap, `@smutti` retains the manual merge decision and deterministic checks
remain independent gates even when automation is enabled. Automation never
merges. Restore independent human approval as soon as a second write-capable
maintainer is available.
