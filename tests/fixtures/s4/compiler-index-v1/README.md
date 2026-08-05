# R7 revision-bound compiler-index fixture

This Apache-2.0 project-owned fixture models a small two-crate Rust workspace,
one pre-generated canonical SCIP v0.9.0 artifact, and the strict sidecar that
binds it to committed source bytes, an immutable revision, producer metadata,
and a toolchain declaration.

The reviewed SCIP source JSON is the auditable input used to materialize the
binary `index.scip`; the governance test independently re-encodes it and
requires exact byte equality. Privacy canaries in raw producer metadata and
documentation must never appear in the expected public overlay.

The fixture is data only. CodeNoesis governance and product conformance must
never compile it, run Cargo or rust-analyzer, execute `build.rs`, expand the
macro, fetch a dependency, open the network, or follow a path outside the
explicit repository and binding roots.

`crates/client/src/omitted.rs` is intentionally absent from the SCIP document
set and is declared as a bounded coverage gap rather than silently treated as
indexed. Reference occurrences remain generic references: even a function
syntax kind does not authorize a `CALLS` relationship.
