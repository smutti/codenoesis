# K1 Rust callable semantics fixture

This is a project-owned synthetic, parser-only Rust fixture. It reviews free
functions, existing method contexts, complete signatures, scalar and
non-scalar declared values, local bindings, calls, control syntax, lexical
nesting, uncertainty, Unicode, and comment/string/macro decoys.

`build.rs` writes `K1_BUILD_SENTINEL_EXECUTED` if executed. A conforming K1
scan never invokes Cargo, rustc, Git, the build script, target code, a network
client, or a model provider. The repository is materialized as the exact Git
commit declared by `manifest.json`; external source is not vendored.
