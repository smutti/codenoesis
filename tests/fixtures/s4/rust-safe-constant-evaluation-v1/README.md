# R16 safe constant-evaluation fixture

This project-owned repository fixes the closed `rust-safe-constant-evaluation-v1`
oracle. It contains checked integer and boolean expressions, one same-owner
constant dependency, one fixed-representation enum with explicit and implicit
discriminants, and target-dependent or otherwise unsupported hard negatives.

The fixture is static input only. CodeNoesis must not run Cargo, rustc, the
`const fn`, build scripts, target code, plugins, models, browsers, or networking.
The repository material, expected identities, values, gaps, and derivations are
bound by `manifest.json` and `expected-safe-constant-evaluation.json`.
