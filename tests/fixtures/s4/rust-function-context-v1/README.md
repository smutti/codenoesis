# R17 function-context fixture

This project-owned Rust fixture exercises the closed
`rust-function-context-v1` projection over the complete R16 local lineage. It
contains one free function, one inherent method with a receiver and two typed
parameters, an explicit generic `Result` return, one uniquely resolvable local
call, one unresolved method call, one constant, and bounded body/control facts.

The fixture is materialized as an immutable local SHA-1 Git commit. CodeNoesis
must not execute Cargo, rustc, build scripts, macros, target code, plugins,
models, or network operations while scanning it. The function context and
viewer remain declared-source projections: they do not infer types, dispatch,
active configuration, ownership, side effects, returned values, or runtime
behavior.
