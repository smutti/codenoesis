# K1 inherited uncertainty fixture

This project-owned parser-only Rust fixture verifies that K1 enriches only
callable subjects already present in the inherited R5/R6 graph. Attributed
free functions, inline test functions, and implementation headers that R5
classifies as unresolved remain covered by inherited uncertainty instead of
becoming dangling K1 subjects or internal failures.

The fixture is materialized as the exact Git commit declared by
`manifest.json`. A conforming scan does not execute Cargo, rustc, target code,
network clients, or model providers.

The sibling `imported-owner-repository` and `imported-owner-manifest.json`
verify that K1 retains methods whose uniquely named local owner is inherited
through the same unqualified cross-module resolution already accepted by R5.
