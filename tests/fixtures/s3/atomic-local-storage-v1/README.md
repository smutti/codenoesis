# S3 atomic local storage fixture

This first-party fixture reuses the exact tree from the approved S2 Rust
knowledge fixture and gives it two immutable Git commits:

- revision A is the approved S2 fixture commit;
- revision B has the same tree, parent A, and different reviewed commit
  metadata.

The unchanged tree isolates S3 publication behavior from parser evolution while
still producing two distinct `RepositorySnapshotV3` semantic hashes and local
snapshot IDs. The logical repository identity is
`urn:codenoesis:fixture:s3-atomic-local-storage-v1`.

The fixture binds:

- exact semantic payloads for revisions A and B;
- complete `LocalSnapshotHeadV1` goldens;
- every publication failpoint outcome;
- idempotent retry and orphan-sweep outcomes;
- corruption, incompatible-schema, and unsafe-root errors.

The later implementation harness must materialize the loose-object Git history
without checkout, place the store outside the repository, and use a test-only
process probe for crash boundaries. No fixture content may execute.

All content is original CodeNoesis test material under the repository
Apache-2.0 license.
