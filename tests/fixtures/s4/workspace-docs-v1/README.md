# S4 workspace documentation fixture

This reviewed fixture fixes the smallest user-tryable Rust workspace shape for
S4. The immutable revision contains:

- one virtual root workspace with two literal members;
- one library target and one binary target;
- one unambiguous out-of-line module;
- one literal path dependency;
- one cross-crate `use` that remains explicitly unresolved by syntax-only
  extraction;
- one build script sentinel that must never execute.

The expected documentation bundle contains one overview and one page for each
resolved module. Its machine manifest binds every material statement to source
evidence or an explicit coverage gap. Query goldens cover exact entity and
document lookup plus typed failure for an unknown ID.

The fixture is specification input, not an example of broad Cargo support.
Workspace globs, `cfg` worlds, generated manifests, `#[path]`, `include!`,
macro expansion, and compiler-grade resolution remain unsupported.
