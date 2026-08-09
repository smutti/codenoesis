# R10 cfg declaration alternatives fixture

This project-owned Git fixture fixes the public `FR-EXT-013` method-alternative
oracle without copying an external repository. Its repository identity is
`urn:codenoesis:fixture:s4-rust-cfg-declaration-alternatives-v1`, tree is
`6aa31d889f4c87b2b7dfbff3fef3b32ee7fa0363`, and commit is
`d5a44bb5bb12ddb6f71ea4dd0c88944dc41eefec`.

The two direct-`cfg` methods deliberately share one R5 logical identity while
their signatures differ. Comments, strings, and a macro body contain decoys.
`build.rs` panics if any scan executes target or build code. The profile records
committed declaration alternatives and exact evidence only; it does not parse
or evaluate `cfg`, choose an active target, infer types, or claim runtime
behavior.

`manifest.json` freezes source bytes, Git object identities, commit metadata,
and reviewed spans. `expected-declaration-alternatives.json` freezes the logical
method, alternative, relationship, and evidence identities reviewed in issue
[#152](https://github.com/smutti/codenoesis/issues/152).
