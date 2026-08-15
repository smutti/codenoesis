# CodeNoesis release profiles

> Status: the bounded G0-G8-local evidence is Verified after protected PRs
> #181, #183, #185, #187, and LocalBaselineVerificationV2 PR #189. Existing
> runtime profile bytes remain experimental, unsigned, unsupported, and not
> generally available; verification grants no release or GA authority.

## Profile classes

| Class | Distribution | Support commitment | Intended use |
|---|---|---|---|
| Experimental | Source build from an exact reviewed revision | None | Bounded engineering evaluation with retained evidence |
| Local GA candidate | Signed installable local artifacts | Explicit platform and capability matrix required | Supported local analysis without target execution or analysis networking |
| Server GA candidate | Signed service artifacts | Explicit deployment, tenancy, SLO, recovery, and support matrix required | Supported multi-user operation |

Anything not named by a merged profile is unsupported. Experimental presence
does not imply compatibility, signing, vulnerability-response, support-window,
release-channel, deprecation, or availability commitments.

## `local-experimental-r17`

- lifecycle: protected PRs #179 and #181 made R17 and the exact G0 registry and
  preflight effective; the exact 32-profile LocalBaselineVerificationV2 pack
  subsequently Verified their bounded evidence through protected PR #189;
- delivery: source build only (`source-build-only`) from an exact reviewed
  revision;
- product boundary: local immutable Git acquisition, bounded Rust R16 analysis,
  deterministic docs/query/PortableGraphV9, and opt-in R17 FunctionContextV1
  plus LocalExplorerV10;
- input authority: explicitly supplied local repository, revision, store,
  documents, portable graph, and stable subject identity only;
- execution authority: no Cargo, rustc, Git subprocess during analysis, build
  script, proc macro, target binary, plugin, model, browser auto-launch, or
  analysis network access;
- data boundary: no raw source or snippets in portable/context/viewer output;
- candidate platform matrix: `x86_64-unknown-linux-gnu` with normative Linux
  seccomp/Landlock evidence, plus functional-only
  `aarch64-apple-darwin` and `x86_64-pc-windows-msvc`; these are experimental
  source-build observations, not support commitments;
- compatibility: existing R0-R16/K1/S7 and LocalExplorerV1-V9 contracts remain
  immutable; R17 is additive and explicitly selected;
- release status: evidence-Verified only for the bounded local baseline; not
  Local GA, signed, published, or supported.

The issue #180 / Decision 0033 package adds the machine-readable
`codenoesis.release-profile/v1` registry and `noesis profile` preflight required
by `FR-REL-001` and `FR-CLI-007`. Signing and attestation are
`not-available`; build provenance is limited to protected Git and CI evidence
and is not release provenance; publication, deployment, and secret authority
are false. The exact G0 package is Approved, Implemented, and Verified only
within the bounded LocalBaselineVerificationV2 catalog.

## G1a staged bundle

Issue #182, Decision 0034, and protected PR #183 made the local-only unsigned
staged-directory bundle under `FR-CFG-001`, `FR-REL-002`, and `FR-CLI-008`
effective; LocalBaselineVerificationV2 later Verified its bounded evidence. Its digest-named directory contains the current-target
binary, one fixed-policy configuration, its closed schema, installation
procedures, the protected license, and a canonical manifest.

The candidate configuration permits only embedded-default or explicit-file
authority and keeps network, model providers, target execution, browser
auto-open, values, and secret references disabled. The candidate lifecycle is
explicit side-by-side directory selection: install a complete digest path,
smoke-test it, select it externally, retain the prior path for rollback, and
remove only an explicitly selected path during uninstall. It mutates no PATH,
home/system state, package database, registry/plist, service, schedule, or
hidden activation pointer.

The bundle is evidence-Verified but remains unsigned, unpublished, unsupported,
and not GA. It is not an archive, installer, automatic updater, server artifact,
signature, attestation, SBOM, provenance statement, or release channel.

## G1b/G8-local carrier

Issue #186, Decision 0036, and protected PR #187 added an outer deterministic
ZIP carrier for the unchanged G1a directory. Metadata binds exact source and target,
CycloneDX SBOM, Cargo.lock graph, license/advisory policy, transitive unsafe
inventory, checksums, and an external GitHub/Sigstore build-provenance
attestation. Offline product verification and external signer verification are
separate, explicit steps.

The existing `local-experimental-r17` runtime profile remains byte-identical and
continues to report source-build-only, no signed-distribution capability, no
support, and no GA. The outer evidence does not rewrite that runtime authority.
LocalBaselineVerificationV2 Verified the bounded G1b/G8-local evidence through
PR #189; no publication or release authority followed.

## R18 candidate boundary

Issue #190 and Decision 0038 propose `trusted-local-source-v1` on exact base
`1de6a420f25a1c7eb74d07a99f1800dde90eefa8`. The selector is an explicit
source-build-only local command and is not added to the immutable
`local-experimental-r17` registry, staged bundle, release-candidate carrier, or
runtime profile JSON. It grants no signing, publication, support, release
channel, EOL, SLA, or GA authority.

Promotion still requires G9 pilot, release, support, channel, vulnerability
response, EOL, and GA decisions. Server artifacts and image signing remain
future G1/G8 work.
