# CodeNoesis local distribution

> Status: G1a/S14 and bounded Local Upgrade Safety are Approved and Implemented
> but not Verified after protected PRs #183 and #185. The staged directory
> remains unsigned experimental engineering material. Issue #186 / Decision
> 0036 defines a Proposed G1b/G8-local verifiable carrier candidate.

## Boundary

G1a packages one already-built `noesis` executable for the current accepted G0
compile target. The packaging command performs no build, dependency resolution,
network access, signing, publication, installation outside its explicit output
root, privilege escalation, or operating-system configuration change.

The complete directory name binds the experimental profile, exact compile
target, and full binary SHA-256. Its canonical manifest binds the five payload
files by relative path, byte length, SHA-256, and executable/data mode. The
sixth file is the manifest itself.

## Configuration

The bundled configuration is the closed
`codenoesis.configuration/local-cli/v1` fixed-policy document. An invocation
may provide it only through the leading `--config <file>` pair. Without that
pair, the identical embedded default is authoritative. No ambient or merged
configuration source exists.

The policy fixes `local-experimental-r17`, JSON output, disabled network, model
providers, target execution and browser auto-open, and forbidden secret values
and references. Validation completes before command dispatch and any
repository, store, output, process, model, browser, signing, or publication
work.

## Lifecycle

Installation, upgrade, rollback, and uninstall use only complete digest-named
directories under an explicit caller-owned prefix:

1. **Clean install:** copy or move the complete staged directory into the
   prefix, validate its manifest, then run the bundled `config validate` and
   `profile` preflights with the bundled explicit configuration.
2. **Upgrade:** place the new digest directory beside the retained old one,
   validate and smoke-test it, then update only the caller-owned invocation
   path.
3. **Rollback:** restore the caller-owned invocation path to the retained prior
   digest directory and rerun both preflights.
4. **Uninstall:** stop running processes and remove exactly the selected digest
   directory.

No CodeNoesis command changes PATH, shell profiles, home or system
configuration, package-manager state, registry/plist values, services,
scheduled jobs, or a hidden `current` pointer. The caller owns activation and
retention. G2 must define a compatibility window before any supported upgrade
claim.

## Local Upgrade Safety

The G2a `FR-CMP-001` preflight compares two complete G1a directories without
executing either binary, while `FR-CLI-009` fixes the two public commands. It
validates exact canonical manifests, payload digests and
modes, target/profile/configuration identity, bundle names, tree membership,
and stable non-symlink file identities. A compatible pair emits
`LocalUpgradePlanV1` and classifies the fixed V1 configuration transition as
`identical-v1-no-migration`.

The branch-scoped candidate is exercised with:

```text
cargo run --locked -p xtask -- preflight-local-upgrade \
  --current <g1a-bundle-a> --candidate <g1a-bundle-b>

cargo run --locked -p xtask -- preflight-local-rollback \
  --plan <local-upgrade-plan-v1.json> \
  --current <g1a-bundle-b> --target <g1a-bundle-a>
```

The caller retains the first command's exact stdout as the rollback plan. A
failure emits only canonical `codenoesis.error/v27` on stderr and exits 2.

Rollback requires that exact plan, the exact candidate as current, and the
exact retained prior bundle as target. It emits `LocalRollbackReportV1` only
after all three inputs revalidate. A reversed pair without the plan, a third
bundle, a changed file, an unsupported contract, or arbitrary downgrade is
rejected. Preflight never changes the caller-owned invocation path or any file.

The package covers only two exact experimental G1a generations. It creates no
general compatibility window, data migration, package manager, updater,
support policy, signing, publication, release channel, SLO, or GA authority.

## Proposed verifiable local candidate

G1b wraps one exact validated G1a directory in a deterministic stored-entry ZIP
without changing any embedded byte or runtime-profile claim. The digest-named
candidate directory also contains canonical checksums, a CycloneDX 1.6 SBOM,
locked dependency graph, license report, current advisory report, transitive
unsafe inventory, and a manifest binding every subject to the exact source
commit and target.

```text
cargo run --locked -p xtask -- package-local-release-candidate \
  --bundle <g1a-directory> --source-commit <main-sha> \
  --supply-chain <evidence-directory> --output <empty-root>

cargo run --locked -p xtask -- verify-local-release-candidate \
  --candidate <candidate-directory>
```

Packaging is failure-atomic; verification is read-only. Neither command signs,
publishes, installs globally, mutates activation, invokes a package manager, or
opens network. A separate post-merge main-only workflow attests the exact
candidate subjects with GitHub/Sigstore SLSA provenance and retains an offline
bundle. Consumers verify repository, signer workflow, source ref, hosted runner,
predicate, and subject digest with the exact documented `gh attestation verify`
boundary.

The workflow creates no tag, GitHub Release, package or OCI artifact,
deployment, support commitment, release channel, EOL, or GA decision. Its
thirty-day Actions artifact is retained engineering evidence, not publication.

## Deferred authority

G1b provides only the bounded local candidate ZIP. It does not provide native
installers, server artifacts,
package repositories, automatic updates, secret managers, non-empty secret
references, release publication, release channels, support windows,
vulnerability-response commitments, EOL, or GA. Server-image signing and those
remaining authorities stay under G1, G2, G5, G8, and G9.
