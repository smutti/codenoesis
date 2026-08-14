# CodeNoesis product threat model

## Local Upgrade Safety candidate

Issue #184 and Decision 0035 define the Proposed `FR-CMP-001` and `FR-CLI-009`
boundary. They treat bundle roots, directory entries, manifests, payload bytes,
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
