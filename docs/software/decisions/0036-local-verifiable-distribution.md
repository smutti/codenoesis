# Decision 0036: Local Verifiable Distribution

- Status: Proposed branch-scoped candidate
- Date: 2026-08-14
- Issue: [#186](https://github.com/smutti/codenoesis/issues/186)
- Authorization: accountable-maintainer authorization for the complete critical G1b/G8-local/S14 package
- Exact base: `c5d259d7689b8a49527f8322b606e58cc0e1e61d`
- Requirements: Proposed `FR-REL-003`, `FR-CLI-010`, and bounded amendments listed in issue #186
- Planning package: `G1b/G8-local`
- Slice: `S14`
- Risk: critical
- Owner and approver: `@smutti`

## Context

Protected PR #183 made the exact G1a fixed configuration and unsigned staged
directory Approved and Implemented but not Verified. Protected PR #185 then
made the exact G2a side-by-side transition and exact-plan rollback preflights
Approved and Implemented but not Verified. Neither package provides an archive,
SBOM, dependency policy, signature, release provenance, or consumer
verification journey.

The immutable G0 runtime profile still describes `local-experimental-r17` as
source-build-only and excludes signed distribution. G1b does not rewrite that
runtime contract. It adds an outer release-candidate carrier whose manifest
states that the embedded runtime profile remains experimental, unsupported,
not GA, and byte-identical.

## Decision

Add repository-maintenance commands that package one exact validated G1a
directory and five exact supply-chain evidence documents into one deterministic
stored-entry ZIP candidate and verify the complete candidate offline. The
candidate directory contains the ZIP, canonical release-candidate manifest,
canonical checksums, CycloneDX 1.6 SBOM, locked dependency graph, license
report, advisory report, and transitive-unsafe inventory.

The ZIP has six sorted regular-file entries under the exact G1a bundle name,
fixed DOS-epoch metadata, fixed data or executable modes, no compression,
extras, comments, descriptors, encryption, ZIP64, symlinks, devices, absolute
paths, traversal, backslashes, duplicates, or unspecified fields. Packaging
revalidates the G1a tree and all evidence before and after reads, validates the
written ZIP, and publishes one digest-named directory by same-filesystem rename
only after every byte is complete. Verification is read-only and emits one
canonical report only after the archive, manifest, checksums, evidence, source,
target, lockfile, CRC-32, SHA-256, length, mode, path, and tree bindings agree.

## Supply-chain policy

A standard-library generator consumes exact target-filtered locked Cargo
metadata, `Cargo.lock`, downloaded registry source, and `cargo audit 0.22.2`
JSON. It emits canonical privacy-safe evidence. License expressions and sources
must match the reviewed policy exactly. Any vulnerability, malformed or
unavailable advisory result, unknown package identity, first-party unsafe,
missing or expired third-party unsafe exception, limit, duplicate, or private
path fails the candidate.

The unsafe inventory is a conservative lexical surface inventory, not proof
that a dependency is memory safe. Every nonzero third-party package/version is
time-bound to owner, scope, rationale, review evidence, expiry, and supported
targets. First-party `unsafe` remains forbidden independently by workspace
lints. Independent semantic review remains required before Verified status.

The current advisory database is observational and time-varying. No byte-level
reproducibility claim applies to a later database observation. For fixed source,
target, binary, G1a tree, and normalized evidence inputs, the ZIP, manifest,
checksums, and offline verification report are byte-identical across fifty
constructions and ten schedules.

## Trusted attestation boundary

Add a `workflow_dispatch`-only workflow that accepts only `refs/heads/main` and
an expected SHA equal to `github.sha`. Supply and build jobs are GitHub-hosted
and have only `contents: read`. A separate attest job receives immutable
intermediate artifacts and has only `contents: read`, `id-token: write`, and
`attestations: write`. After those permissions are available it executes no
repository, dependency, or candidate program; it uses pinned GitHub actions and
fixed digest checks to produce SLSA build-provenance attestations and an offline
bundle.

The head-authored workflow is inert for its own pull request. It activates only
after protected manual merge and explicit maintainer dispatch. It has no secret,
key, tag, GitHub Release, package, OCI, deployment, environment, content-write,
merge, support, EOL, or GA authority. Workflow artifacts are retained for
thirty days and are not a release channel.

Consumer cryptographic verification remains the external boundary:

```text
gh attestation verify <archive> --bundle <bundle> \
  --repo smutti/codenoesis \
  --signer-workflow smutti/codenoesis/.github/workflows/local-release-candidate.yml \
  --source-ref refs/heads/main --deny-self-hosted-runners
```

The local verifier validates product and evidence bytes but never claims to
reimplement Sigstore trust.

## Bounds and failure behavior

The package accepts one inherited six-file G1a bundle with a binary no larger
than 268,435,456 bytes. The ZIP is bounded to 285,212,672 bytes, each evidence
document to 4,194,304 bytes, all evidence to 33,554,432 bytes, packages to 512,
dependency edges to 4,096, unsafe exceptions to 512, ZIP entries to 1,024,
relative paths to 256 bytes, and public JSON to 131,072 bytes including LF.

Invalid input emits only `CodeNoesisErrorV28` on stderr with exit 2 and empty
stdout. Completion failure uses exit 1. Failure leaves no final or partial
candidate. Error context never includes an input path, source root, user,
environment, credential, source content, registry root, URL, or secret.

## Red and lifecycle

The checkpoint contains this decision, Proposed governance, strict schemas,
policy, target oracle, invalid matrix, Python guards, and black-box acceptance
tests before contract or production Rust, workflow implementation, dependency
edge, or lockfile change. On the exact base, the public command is rejected as
G1a `distribution.invalid_arguments`, while the workflow contract fails because
the trusted workflow is absent. Those are the only acceptable Reds.

Before protected merge the requirements, implementation, workflow, signature,
and evidence remain Proposed candidates. Merge makes only the exact local
candidate mechanism Approved and Implemented. A manual post-merge dispatch and
independent acceptance of the resulting three-platform attestations are still
required before Verified. Publication, support, vulnerability-response SLA,
release channels, EOL, server images, and GA remain G9 or later work.

## Consequences

- Maintainers can create deterministic installable local candidate archives and
  inspect all evidence without trusting the archive producer.
- Consumers can bind candidate bytes to the exact protected-main signer
  workflow and source identity without a repository signing secret.
- Existing G0, G1a, G2a, R0-R17, K1, S7, ontology, and viewer bytes remain
  unchanged.
- Reverting this package removes only additive candidate tooling, policy, and
  the post-merge workflow; it publishes or migrates nothing.
