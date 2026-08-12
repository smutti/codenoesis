# R14/R15 real-repository shape fixture

This project-owned source-only fixture reproduces the ordinary Rust syntax
that blocked the R14 and R15 journeys on pinned real repositories. It contains
one R15-complete callable, one inherited R15-unsupported callable, a direct
simple call target, a complex receiver containing `://`, a call with one
selected and one closure argument, a method call whose receiver is an excluded
macro invocation, a `match_expression` let initializer, and one direct
`cfg(test)` callable intentionally absent from K1.

Tests materialize the reviewed Git objects without invoking Cargo, rustc,
build scripts, proc macros, target code, network, models, or a browser. The
fixture intentionally need not compile: only committed source syntax is
authority. Exact repository objects and expected graph facts are frozen in
`manifest.json` and `expected-r14-r15-correction.json`.

The correction keeps unsupported syntax explicit. It does not infer target
resolution, compiler control or data flow, runtime behavior, types, ownership,
aliasing, side effects, macro expansion, or an active `cfg` world.
