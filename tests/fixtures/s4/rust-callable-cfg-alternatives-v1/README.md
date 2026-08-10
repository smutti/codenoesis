# R12 callable cfg-alternatives fixture

This project-owned synthetic Git fixture composes the existing K1 callable
subset with two direct-`cfg` declarations of `Worker::run`. It reuses the exact
K1 repository identity and exact `Cargo.toml`, `build.rs`, and `src/lib.rs`
bytes. Only `src/model.rs` is a reviewed R12 overlay.

The logical method remains one stable R5/R10 subject. The Unix and Windows
declarations remain unresolved alternatives and become independent K1 callable
subjects. The fixture does not evaluate `cfg`, choose a target, infer method
dispatch or runtime behavior, or copy external source.

`build.rs` writes `K1_BUILD_SENTINEL_EXECUTED` if executed. A conforming scan
never invokes Git, Cargo, rustc, the build script, target code, a network
client, browser, or model provider. `manifest.json` freezes source bytes, Git
objects, commit metadata, and reviewed spans. `expected-composition.json`
freezes the reviewed cross-layer identities and counts authorized by issue
[#158](https://github.com/smutti/codenoesis/issues/158).
