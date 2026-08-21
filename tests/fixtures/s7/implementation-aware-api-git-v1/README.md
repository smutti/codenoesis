# S7 R19 Git-backed implementation-aware API fixture

This project-owned fixture reuses the immutable source bytes under
`tests/fixtures/s7/implementation-aware-api-v1` and materializes five local
Git repositories during tests: one provider with baseline and target commits,
plus strict, safe, and decoy Kotlin clients. No `.git` directory or external
repository is vendored.

The deterministic commits are built with the fixed CodeNoesis fixture actor,
UTC timestamps beginning at 2000-01-01, and the messages recorded in
`manifest.json`. The provider OpenAPI bytes are identical across revisions;
only the Rust provider source changes. The accepted S6 federation report is
reused byte-for-byte and its semantic revisions are mapped explicitly by the
R19 workspace rather than treated as Git authority.

The oracle requires 2 semantic diffs, 2 client assessments, 1 rejected decoy,
9 Git-bound evidence records, and 1 coverage gap. Every evidence ID must
navigate independently to exact committed UTF-8 bytes. Existing S7 V1 fixture
and 14,991-byte report remain byte-identical.
