# R11 K1 repository-boundary composition fixture

This project-owned fixture overlays the immutable K1 callable-semantics source
bytes with one committed `.gitmodules` declaration and one mode `160000`
entry at `external/nested-model`. The nested revision uses Git's empty tree.

The test harness materializes all Git objects before the monitored `noesis`
process starts. The primary unbound run materializes no nested worktree. The
explicitly-bound run supplies the separately materialized empty repository
through `boundary-input-matching.json`. Neither run authorizes nested source
inventory, parsing, documentation, query-as-source, export, Git execution,
URL resolution, network, target execution, or model-provider access.

The Rust, Cargo, and build-script bytes are read from the existing
`tests/fixtures/s4/rust-callable-semantics-v1` fixture and must retain their
reviewed byte lengths, SHA-256 digests, and Git blob OIDs. This directory does
not duplicate or modify those source bytes. `build.rs` remains the execution
sentinel.
