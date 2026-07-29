# S1 packed SHA-1 fixture

This project-owned fixture extends the accepted
[`safe-inventory-v1`](../safe-inventory-v1/README.md) source repository with
multiple physical Git object-database materializations of the same immutable
commit. It adds no third-party source and does not change the accepted S1
fixture, goldens, or contract bundle.

The implementation harness materializes all Git objects before the monitored
`noesis` process starts. It may use a project-owned pack builder and independent
offline Git tooling during fixture setup, but the product runtime may not
invoke Git, a child process, a network service, a hook, a filter, or a
credential helper.

The primary oracle compares the RFC 8785 bytes of
`RepositorySnapshotV2.semantic` for loose, base-only packed, `OFS_DELTA`, and
`REF_DELTA` representations. The packed variants contain no reachable loose
fallback. Additional recipes mutate index, pack, zlib, object, and delta
structures and exercise each fixed resource limit.

The conformance harness builds a bounded sequential entry map for every pack
before revision resolution. It structurally inflates and discards each entry
under separate per-entry and cumulative ceilings, proves exact index-offset
alignment, and keeps delta/object reconstruction lazy for the reachable
closure.

Large limit cases are deterministic builder/model recipes rather than committed
bulk data. Generated pack bytes, index bytes, logs, and their digests belong in
the implementation evidence pack and are not source fixtures.

All material in this directory is covered by the repository-wide Apache-2.0
license.
