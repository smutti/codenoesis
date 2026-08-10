# R13 callable and SCIP composition fixture

This descriptor reuses the immutable project-owned
`tests/fixtures/s4/compiler-index-v1` repository, binding, and canonical SCIP
artifact. It copies no source and changes no R7 or K1 golden.

The repository is materialized at tree
`d117f2f0924cbef9e7396b97ee46c76bd5261e00` and commit
`2203600cce0f0904aefc66dcb49dd0dbc7fd5fd3`. R13 runs the existing K1 source
extractor and R7 importer over the same immutable inventory, then verifies the
five exact `HAS_COMPILER_SYMBOL` joins in `expected-composition.json`.

The two K1 call sites remain unresolved. The fixture authorizes no compiler,
index generator, build script, target, process, network, model, or browser.

