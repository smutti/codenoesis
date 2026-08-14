# CodeNoesis release profiles

> Status: Proposed G0 planning baseline. A profile is not a release and does not
> make any capability Verified or generally available.

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

- lifecycle before protected merge: Proposed branch candidate;
- delivery: source build only from the exact reviewed revision;
- product boundary: local immutable Git acquisition, bounded Rust R16 analysis,
  deterministic docs/query/PortableGraphV9, and opt-in R17 FunctionContextV1
  plus LocalExplorerV10;
- input authority: explicitly supplied local repository, revision, store,
  documents, portable graph, and stable subject identity only;
- execution authority: no Cargo, rustc, Git subprocess during analysis, build
  script, proc macro, target binary, plugin, model, browser auto-launch, or
  analysis network access;
- data boundary: no raw source or snippets in portable/context/viewer output;
- platforms: repository CI observations are evidence only; no operating system,
  architecture, installer, or support commitment is made by this profile;
- compatibility: existing R0-R16/K1/S7 and LocalExplorerV1-V9 contracts remain
  immutable; R17 is additive and explicitly selected;
- release status: not Local GA, not signed, not supported, and not Verified.

Promotion requires a separate G0/G1/G2/G5/G8 package that fixes supported
platforms, artifacts, installation, compatibility windows, security policy,
signing, provenance, ownership, and support terms.
