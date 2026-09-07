# Java grammar dependency review

Reviewed 2026-09-07 for FR-EXT-026 / Decision 0052. Exact new registry input:
`tree-sitter-java = 0.23.5`, MIT, upstream
https://github.com/tree-sitter/tree-sitter-java. Cargo.lock checksum:
`0aa6cbcdc8c679b214e616fd3300da67da0e492e066df01bcf5a5921a71e90d6`.
No existing dependency version changes. The normal/build graph reuses
`tree-sitter-language 0.1.7` and `cc 1.3.0`; the runtime stays `tree-sitter 0.26.11`.

The package has two Rust files and one lexical unsafe token, verified with the
repository's conservative inventory. `bindings/rust/lib.rs` wraps the exported
`tree_sitter_java` C language function in `LanguageFn`; the unsafe boundary is
third party. `bindings/rust/build.rs` compiles the shipped `src/parser.c` as C11
through cc, with the existing MSVC UTF-8 option. It does not execute analyzed
Java, Maven, Gradle, wrappers, annotation processors or generated project code.
This is a bounded source/build-input review, not a proof of memory safety of the
generated C parser or its runtime. The scan retains existing confinement and
capacity boundaries. First-party unsafe remains forbidden.

The policy adds only this exact dependency exception for the existing three
supported targets, retains the existing expiry of 2026-11-14, and refreshes the
lock digest. The old policy-wide reviewed_on date is preserved because unrelated
exceptions were not re-reviewed. No workflow, permissions, release execution,
license allowlist, signing or publication authority changes.
