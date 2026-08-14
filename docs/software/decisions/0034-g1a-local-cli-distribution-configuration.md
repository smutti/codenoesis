# Decision 0034: G1a local CLI configuration and staged distribution

- Status: Proposed branch-scoped candidate
- Date: 2026-08-14
- Issue: [#182](https://github.com/smutti/codenoesis/issues/182)
- Authorization: accountable-maintainer high-risk G1a/S14 authorization recorded for the complete issue #182 package, plus the bounded G0 historical-guard correction recorded on the issue
- Exact base: `a525126228205901885038586e21d30db745b1ec`
- Requirements: Proposed `FR-CFG-001`, `FR-REL-002`, `FR-CLI-008`, and the bounded amendments listed in issue #182
- Planning item: local-first `G1a` subset of `G1`
- Slice: `S14`
- Risk: high
- Owner and approver: `@smutti`

## Context

Protected PR #181 made the exact G0 experimental release-profile registry and
preflight Approved and Implemented but not Verified, supported, signed,
published, or GA. The product still has no versioned startup configuration and
no deterministic installable material that a user can place under an explicit
local prefix without a source checkout.

This decision closes only a local unsigned precursor. It does not complete G1
and does not implement server artifacts, archives, native installers, package
repositories, automatic updates, secrets, release channels, compatibility
windows, SBOMs, signatures, attestations, provenance, publication, support, or
GA.

## Decision

Add one closed fixed-policy `LocalCliConfigurationV1`, one canonical
`LocalConfigurationReportV1`, one canonical `LocalDistributionManifestV1`, and
one strict `CodeNoesisErrorV26` family. Expose configuration validation through
`noesis config validate` and one optional leading `--config <file>` pair. Add
`xtask package-local-cli` for one already-built current-target binary and one
explicit empty output root.

Configuration precedence is only explicit file over embedded default. There is
no environment, home/system/current-directory, registry/plist, network,
interpolation, include, inheritance, merge, migration, fallback, or repair.
The exact fixed policy selects `local-experimental-r17`, JSON output, disabled
network, model providers, target execution and browser auto-open, and forbidden
secret values and references.

The package command derives the target from compile-time `cfg`, validates the
closed G0 target set, reads one stable regular non-symlink binary of at most
268,435,456 bytes, and stages a private sibling directory inside the explicit
output root. It publishes by same-filesystem rename only after validating every
payload path, length, SHA-256, mode, count, manifest byte, and limit.

## Bundle

The final name is
`codenoesis-local-experimental-r17-<target>-<full-binary-sha256>`. The payloads
are, in canonical order, the current-target binary, fixed configuration, closed
configuration schema, installation procedure, and protected root license. The
sixth file is `manifest.json`, whose bytes equal successful stdout.

The manifest classifies the material as `unsigned-staged-directory`, support
`none`, verification `not-verified`, release status `not-ga`, signing and
attestation `not-available`, and release provenance/publication false. It
contains no timestamp, host, username, absolute path, environment, working-tree
state, URL, credential, secret, source text, or ambient target string.

## Lifecycle

The complete digest-named directory is the installation unit. Upgrade installs
a second directory side by side and changes only a caller-owned invocation
path after validation. Rollback selects the retained prior path. Uninstall
removes exactly the selected path after processes stop. No product command
mutates PATH, shell profiles, home/system state, package databases,
registry/plist, services, schedules, or a hidden activation pointer.

## Bounds and failure

Configuration and report maximums are 65,536 bytes including final LF where
applicable. Binary maximum is 268,435,456 bytes. Manifest/output maximum is
65,536 bytes and payload count is exactly five. Every maximum and
maximum-plus-one is tested. Failure produces strict `CodeNoesisErrorV26`, empty
success output, and no downstream work or partial final directory.

Fifty argument constructions and ten schedules must reproduce byte-identical
configuration reports and bundle bytes for identical target and inputs.
Target-specific binary bytes are not claimed reproducible across targets or
builds.

## Oracle and Red

The governance checkpoint freezes strict schemas, embedded configuration,
reports, fixture binaries, target manifests, complete tree, lifecycle and
failure oracles, Python guard, and two black-box tests before production source,
manifest, lockfile, xtask, or distribution-asset edits.

On exact base `a525126228205901885038586e21d30db745b1ec`, the configuration
command exits 2 with empty stdout and the existing 149-byte
`codenoesis.error/v1` `input.invalid_revision`. The xtask command exits 0,
writes its historical 123-byte bootstrap sentence, empty stderr, and no bundle.
Those are the sole acceptable Reds. Governance must already be Green.

## Historical G0 guard correction

The immutable G0 contract bundle records SHA-256 observations for both product
artifacts and lifecycle documents at its checkpoint. Issue #182 authorizes the
G0 guard to continue enforcing current-tree equality for immutable G0 product
and oracle records while treating README, SRS, architecture, roadmap, release
profiles, and the historical guard record as checkpoint observations. The G0
bundle itself remains byte-identical with SHA-256
`e29d6484cc845c45ffd37f36b0a884dff8e7c92f15da9ca43c2e3dfb4f08c97e`.

## Lifecycle and authority

Before protected merge, all G1a requirements remain Proposed and the
implementation remains a candidate. Protected merge activates only the fixed
local configuration and unsigned staged-directory behavior. It grants no
server, secret, signing, provenance, publication, support, release-channel,
compatibility, installer, automatic-update, privilege, control-plane, or GA
authority.

## Consequences

- A user can validate deterministic startup policy before repository work.
- A reviewed local binary can be staged with exact digest-bound support files
  and explored without a source checkout.
- Explicit side-by-side paths make clean install, upgrade, rollback, and
  uninstall reviewable without hidden activation state.
- G0 and every R0-R17/K1/S7 product contract and golden remain unchanged.
- Rollback is a complete revert of issue #182 plus removal of explicitly copied
  staged directories.
