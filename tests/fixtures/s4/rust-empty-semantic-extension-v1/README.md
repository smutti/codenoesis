# R5 empty semantic-extension fixture

This is a project-owned synthetic Rust repository for issue #164. Its exact
bytes materialize Git commit `c780476957a29db6ede1cefb408140763990e829`
and tree `d13008ae7c7dbf9807b9599eb9c1b1213b4b94f4`.

The crate intentionally contains only one ordinary function. It has no field,
enum variant, constant, static, associated type, or implementation-context
method, so the correct additive R5 member graph and both R5 index families are
empty. The inherited R4 graph and committed source chunk remain complete.

The acceptance journey may read these bytes and materialize their reviewed Git
objects. CodeNoesis must never execute Cargo, rustc, a build script, target
code, network access, a model, or a browser, and must never mutate this source.
