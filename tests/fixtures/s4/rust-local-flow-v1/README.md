# R15 Rust local-flow fixture

This project-owned fixture freezes the smallest complete R15 closed-flow
journey. It contains one acyclic function with an initialized binding, one
explicit non-empty `if`/`else`, one assignment in each arm, one join binding,
and one tail read.

The fixture is source-only. Tests materialize the reviewed Git objects without
executing Cargo, rustc, build scripts, proc macros, target code, network,
models, or a browser. The exact repository, object, file, graph, evidence, and
derivation identities are frozen in `manifest.json` and
`expected-local-flow.json`.

The inherited `rust.reachability_not_computed` and
`rust.data_flow_not_computed` gaps intentionally remain. R15 facts describe
only possible normal source progression and the closed lexical
reaching-definition rule in Decision 0027.
