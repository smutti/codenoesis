# S2 Rust knowledge fixture

This first-party Apache-2.0 fixture is the reviewed source for the S2 ontology,
extraction, stable-ID, claim, malformed-input, and graph oracle. `revision-a`
contains one Rust library crate with one inline module; it is data only and
must never be compiled or executed by CodeNoesis.

`manifest.json` binds exact source bytes and deterministic Git identities.
Generated variants are recipes over those reviewed bytes and are not additional
authoritative source trees.
