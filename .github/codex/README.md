# Codex GitHub review

CodeNoesis keeps one optional AI workflow: static read-only pull-request review.

To enable it, set repository variable `CODEX_REVIEW_ENABLED=true` and add
`OPENAI_API_KEY` to the protected `codex-model` environment. Leave the switch
disabled when the secret is unavailable.

The workflow:

- reviews an immutable in-repository pull-request head against its base;
- gives the model read-only access and no repository write token;
- does not execute repository code, builds, tests, package hooks, or network
  commands inside the model job;
- validates structured output before a separate job posts a comment;
- blocks on validated P0/P1 findings when enabled;
- skips only drafts, unsupported targets/authors, or diffs above its technical
  file-count capacity;
- never approves, merges, publishes, signs, deploys, or releases.

Deterministic CI remains independent and authoritative. Governance,
control-plane, ontology, schema, dependency, benchmark, and product changes may
be reviewed together in one coherent pull request.

`policy.json` and `policy.schema.json` are retained only because historical S0
conformance evidence binds the registry. No current workflow reads them and
they grant no delivery authority.
