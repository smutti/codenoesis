# S5 incremental refresh fixture

This project-owned fixture defines the smallest reviewed S5 partial-refresh
case:

- revision A is a two-crate Rust workspace with three mapped source chunks;
- revision B changes only the already-mapped non-root
  `crates/model/src/item.rs`;
- the target adds one free function while retaining the existing `Item`
  identity;
- the app source, model root source, manifests, target mapping, and workspace
  membership remain byte-identical;
- a build script sentinel must never execute.

The expected refresh plan must recompute only the changed revision-neutral
analysis entry and reuse the app and model-root analysis entries by exact cache
key and payload hash. It must then rematerialize all three public chunks,
because accepted S4 evidence identities include the immutable target commit.
The target snapshot and generated-document projection must be byte-identical to
a reviewed cold S4 target scan. The fixture does not authorize general Cargo,
module, rename, compiler, macro, or build behavior.

Analysis reuse and public artifact reuse are deliberately different facts.
`AnalysisCacheEntryV1` contains no commit-bound evidence, claim, relationship,
chunk, snapshot, or document identity. No baseline public chunk is copied into
the target snapshot.

The standard path must not invoke Git, Cargo, `rustc`, build scripts, hooks,
target code, plugins, a network, or a model provider.
