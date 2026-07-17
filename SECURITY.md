# Security policy

## Project status and supported versions

CodeNoesis is in project inception and has no released runtime or supported
version yet. Security reports about repository automation, specifications, test
fixtures, future implementation, or accidental disclosure are still welcome.
This section will be replaced by a supported-version matrix before the first
release.

## Reporting a vulnerability

Use [GitHub private vulnerability reporting](https://github.com/smutti/codenoesis/security/advisories/new).
Do not disclose the vulnerability, exploit, credentials, private repository
content, or proof-of-concept details in a public issue or discussion.

If private reporting is unavailable, open a public issue that asks the
maintainer for a private contact channel but contains no security details.

Include, when applicable:

- affected commit, component, configuration, and platform;
- impact and prerequisites;
- minimal reproduction using synthetic data;
- logs with credentials and source content removed;
- whether exploitation crosses a repository, filesystem, process, network,
  workspace, tenant, model-provider, or release boundary;
- suggested mitigation and any disclosure deadline constraints.

Do not test against systems or repositories you do not own or have permission
to assess. Do not retain, publish, or transmit source material encountered
during testing.

## AI and repository-automation concerns

Treat these as security reports when they can affect authority or confidential
data:

- prompt injection through source, documentation, issue text, comments,
  generated output, parser diagnostics, or model responses;
- an agent escaping allowed paths, sandbox, network, cost, or iteration limits;
- workflow-token, GitHub App, OIDC, model-key, signing-key, or secret exposure;
- a builder approving, merging, releasing, or changing the policy that judges
  its own patch;
- forged, missing, or mutable evidence used to pass a required check;
- execution of target build scripts or repository-controlled code with a model
  credential or write-capable token available;
- cross-project, cross-workspace, or cross-tenant knowledge disclosure;
- release, provenance, dependency, or artifact-integrity bypasses.

## Handling process

The maintainer will acknowledge and assess reports on a best-effort basis. No
response or remediation SLA is committed before the project defines its
maintainer rotation and release policy. Valid reports will be handled privately
until a fix or mitigation and a coordinated disclosure plan are ready.

Security fixes must add or strengthen an executable regression scenario before
the corrective production change is accepted. A security oracle or golden
fixture receives the same protected review as the implementation.
