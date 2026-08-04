# R5 Rust semantic-depth fixture

This is a **project-owned** synthetic Rust repository for the exact
`rust-semantic-depth-v1` declaration contract. It is not copied from Lekton,
RustDesk, or any other external repository.

The fixture covers named and tuple fields, every enum variant form, module and
associated constants, immutable and mutable statics, associated types, trait
required/default methods, inherent methods, named local trait implementations,
Unicode and raw identifiers, and declarations carrying `cfg`, `cfg_attr`,
derive, built-in-looking, and custom attributes.

Comments, strings, and macro token trees contain declaration-shaped decoys.
`build.rs` is an execution sentinel. Scanners and tests must **never execute**
Cargo, rustc, build scripts, procedural macros, target code, or dependencies
while consuming these committed bytes.

The machine manifest binds only files below `repository/`. The reviewed facts
file binds declaration counts, member identity examples, uncertainty states,
and hard negatives without claiming an active configuration, expanded macro,
resolved type, evaluated value, call graph, or runtime behavior.
