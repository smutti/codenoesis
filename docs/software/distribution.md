# CodeNoesis local distribution

> Status: Proposed G1a/S14 branch-scoped candidate under issue #182 and
> Decision 0034. The staged directory described here is unsigned experimental
> engineering material, not a supported or published release.

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

## Deferred authority

This package does not provide archives, native installers, server artifacts,
package repositories, automatic updates, secret managers, non-empty secret
references, SBOMs, signatures, attestations, release provenance, publication,
release channels, support windows, vulnerability-response commitments, EOL,
or GA. Those remain under G1, G2, G5, G8, and G9.
