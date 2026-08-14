# Decision 0033: G0 bounded release-profile registry and preflight

- Status: Proposed branch-scoped candidate
- Date: 2026-08-14
- Issue: [#180](https://github.com/smutti/codenoesis/issues/180)
- Authorization: accountable-maintainer critical G0/S14 authorization recorded for the complete issue #180 package
- Exact base: `f0bdb5290566bb85bb103e24291e952d4c557156`
- Requirements: Proposed `FR-REL-001`, Proposed `FR-CLI-007`, and the bounded amendments listed in issue #180
- Planning item: `G0`
- Slice: `S14`
- Risk: critical
- Owner and approver: `@smutti`

## Context

Protected PR #179 made the exact R17 function-context package Approved and
Implemented but not Verified. Its `local-experimental-r17` label described a
source-build boundary in prose, but the product had no machine-readable
release-profile contract and no output-only command that could identify the
current target's exact experimental guarantee before repository work.

The roadmap requires G0 before another public compatibility or interface
contract. This decision closes only the bounded experimental registry and
preflight subset. It does not implement distribution, installation,
compatibility windows, SBOM generation, vulnerability policy, signatures,
attestations, release provenance, release channels, support, EOL, or GA.

## Decision

Add one embedded, deterministic `local-experimental-r17` definition and one
output-only command:

```text
noesis profile --id local-experimental-r17 --format json
```

The command installs the inherited S0 boundary first, derives its target only
from compile-time Rust `cfg`, validates the closed registry, and emits one
canonical LF-terminated `codenoesis.release-profile/v1` report. It accepts no
target override, path, repository, store, source, output destination,
environment-derived selector, credential, signing key, network endpoint, or
publication authority.

Unknown profiles, unsupported targets, invalid or duplicate arguments,
non-JSON format, malformed embedded state, non-canonical ordering, privacy
violations, and resource excess fail closed as `codenoesis.error/v25` with
empty stdout. Input and capability failures exit 2; internal registry failure
exits 1. Repair, fallback, inference, target spoofing, truncation, and retry do
not turn an invalid registry or target into an accepted profile.

## Exact profile

The report fixes these values:

- profile `local-experimental-r17`;
- classification `experimental`;
- distribution `source-build-only`;
- support `none` and owner `@smutti`;
- verification `not-verified` and release status `not-ga`;
- signing and artifact attestation `not-available`;
- build provenance `protected-git-and-ci-evidence-only`, explicitly not
  release provenance;
- no release publication, deployment, or secret authority.

The only advertised capability IDs, in order, are:

```text
local-acquisition-r2
local-analysis-r16
local-docs-query-r16
local-portable-graph-v9
local-function-context-v1
local-explorer-v10
```

The only excluded capability IDs, in order, are:

```text
incremental-refresh-s5
federation-s6
implementation-aware-impact-s7
trusted-source-retrieval
remote-acquisition
compiler-index-generation
model-provider
server-runtime
signed-distribution
release-publication
```

The only limitations, in order, are:

```text
experimental_source_build_only
not_verified
no_support_window
no_binary_distribution
no_signature_or_attestation
linux_only_normative_os_confinement
no_ga_compatibility_promise
```

## Platform matrix

The closed matrix contains exactly:

| Target | Classification | Sandbox tier | Normative OS confinement |
|---|---|---|---|
| `x86_64-unknown-linux-gnu` | `ci-observed-experimental` | `normative-linux-seccomp-landlock-v1` | yes |
| `aarch64-apple-darwin` | `ci-observed-experimental` | `functional-portability-only-v1` | no |
| `x86_64-pc-windows-msvc` | `ci-observed-experimental` | `functional-portability-only-v1` | no |

The matrix records exact-head CI observations, not support commitments. Linux
is the sole normative S0/S1 process, network, and filesystem confinement
evidence platform. macOS and Windows provide functional portability evidence
only and never imply an equivalent operating-system sandbox. Every other
target fails before profile publication.

## Bounds and privacy

The report maximum is 65,536 bytes including LF. A registry may contain at
most 16 platforms, 64 advertised capabilities, 64 exclusions, and 64
limitations; each identifier or text value is at most 128 UTF-8 bytes. Every
maximum and maximum-plus-one is tested even though the first-party registry is
smaller.

No timestamp, working-tree state, hostname, username, environment value,
absolute path, URL, token, credential, source text, telemetry, model data,
release secret, or ambient platform string enters the report. The compile-time
target is normalized to one exact reviewed identifier.

## Oracle and Red

The governance checkpoint freezes one registry fixture, one exact report
golden for each accepted target, the report and error schemas, a machine
acceptance contract, the Python governance guard, and the black-box Rust test
before production source changes.

On exact base `f0bdb5290566bb85bb103e24291e952d4c557156`, the command exits
2, writes zero stdout bytes, and emits the existing 149-byte
`codenoesis.error/v1` `input.invalid_revision` record. That is the sole
acceptable Red. Governance must already be Green.

Fifty argument or registry permutations and ten schedules per accepted target
must produce byte-identical target-specific report bytes. Invalid, duplicate,
non-canonical, private, unsupported, maximum, and maximum-plus-one cases must
produce the reviewed failure without work or publication.

## Lifecycle and authority

This package records protected PR #179 as Approved and Implemented but not
Verified without modifying Decision 0032 or any R17 artifact. Before this
package's protected merge, `FR-REL-001` and `FR-CLI-007` remain Proposed and
the implementation remains a candidate.

Protected merge activates only the embedded experimental registry and
preflight command. It grants no binary artifact, release, signing,
attestation, publication, deployment, support, compatibility, tag, secret, or
workflow authority. G1, G2, G5, G8, and G9 remain separate.

## Consequences

- A source build can state exactly which experimental capability and platform
  boundary it represents without reading a repository.
- Unsupported targets fail closed rather than inheriting a vague portability
  promise.
- Linux confinement and non-Linux functional evidence remain distinguishable.
- R0-R17, K1, S5-S7, query, portable, explorer, fixture, golden, and evidence
  bytes remain unchanged.
- Rollback is a complete revert of issue #180.
