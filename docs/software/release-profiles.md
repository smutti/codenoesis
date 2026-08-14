# CodeNoesis release profiles

> Status: Proposed G0 machine-contract candidate under issue #180 and Decision
> 0033. A profile is not a release and does not make any capability Verified or
> generally available.

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

- lifecycle: protected PR #179 made R17 Approved and Implemented but not
  Verified; issue #180 remains a Proposed G0 branch candidate;
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
- release status: not Local GA, not signed, not supported, and not Verified.

The issue #180 / Decision 0033 candidate adds the machine-readable
`codenoesis.release-profile/v1` registry and `noesis profile` preflight required
by Proposed `FR-REL-001` and `FR-CLI-007`. Signing and attestation are
`not-available`; build provenance is limited to protected Git and CI evidence
and is not release provenance; publication, deployment, and secret authority
are false.

Promotion still requires separate G1/G2/G5/G8 packages for installable
artifacts, compatibility windows, security policy, SBOMs, signatures,
attestations, release provenance, and distribution, followed by G9 support and
GA decisions.
