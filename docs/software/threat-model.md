# CodeNoesis product threat model

## Local Upgrade Safety

Issue #184, Decision 0035, and protected PR #185 made the bounded
`FR-CMP-001` and `FR-CLI-009` boundary Approved and Implemented but not
Verified. It treats bundle roots, directory entries, manifests, payload bytes,
file metadata, transition plans, command arguments and ambient process state as
untrusted. The protected assets and exact G1a contract define the only accepted
tree and compatibility authority.

Threats include traversal or absolute paths, symlinked roots/files/plans,
missing or extra entries, digest/mode/name substitution, manifest or plan
tampering, cross-target/profile/schema pairs, replacement during validation,
oversized inputs, private path disclosure, credential canaries, binary
execution, ambient discovery, and hidden activation mutation.

The candidate mitigates them through exact closed paths, bounded stable regular
file reads, pre/post-read identity checks, canonical manifest and plan bytes,
full SHA-256 verification, exact tree membership, caller-owned activation, one
typed privacy-safe error, empty stdout on failure, and output-only operation.
It never executes either bundle, follows network, reads a secret source, invokes
project tooling, mutates installation state, signs, publishes or grants support.

Residual risk remains explicit: no signature, provenance, package repository,
trusted release ordering, support window, cross-host SLO, native installer,
automatic updater, server migration or GA authority exists.

## G1b/G8-local verifiable distribution candidate

Issue #186 and Decision 0036 treat the G1a tree, supply metadata, dependency
source, advisory database output, policy, ZIP records, candidate paths,
checksums, workflow artifacts, attestations, command arguments, runner context,
and ambient process state as untrusted. The exact protected source commit,
closed contracts, reviewed policy, pinned tools/actions, and GitHub OIDC
certificate identity are the only accepted authorities.

Threats include archive traversal, duplicate or overlapping ZIP records,
absolute/backslash/private paths, symlink/device/hard-link substitution,
CRC/SHA/length/mode mismatch, malformed or oversized metadata, lock/SBOM/report
substitution, stale or unavailable advisory data, license drift, unregistered or
expired unsafe exceptions, mutable input, output-root races, credential/source
leakage, action substitution, pull-request OIDC, self-hosted runners, confused
signer identity, artifact replacement, and privilege expansion into release or
publication authority.

Mitigations include exact safe path grammar, stored entries with fixed metadata,
canonical JSON and checksums, stable regular-file identities, pre/post-read
validation, complete tree membership, fixed limits, failure-atomic publication,
privacy-safe errors, target-filtered locked metadata, current cargo-audit
evidence, time-bound exception policy, exact action SHAs, hosted runners,
main-only expected-SHA dispatch, and job separation. Supply/build jobs have only
contents-read. The OIDC/attestation job runs no repository, dependency, or
candidate executable and has no content, package, deployment, secret, release,
tag, or merge write authority.

Residual risk remains explicit: lexical unsafe inventory is conservative rather
than a semantic proof; GitHub and Sigstore remain external trust dependencies;
the advisory observation changes over time; a thirty-day workflow artifact is
not durable publication; macOS and Windows retain only functional portability;
and no support, response SLA, release channel, EOL, server image, or GA decision
exists. Independent post-merge attestation review is mandatory before Verified.
