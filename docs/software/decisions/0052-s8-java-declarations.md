# Decision 0052: bounded Java declarations

- Status: branch candidate; effective after maintainer merge
- Requirement: FR-EXT-026, S8; first Java subset of FR-EXT-003
- Base: `e0af187cf1d3e2b64bb6e2d57feda4f39e7da1b2`
- Authorization: maintainer instruction to proceed after Kotlin/KMP PR #225 merge
- Dependency: exactly `tree-sitter-java = 0.23.5`, MIT

Deliver the [Java v0.1 contract](../java-v01.md) as one complete static
extraction/store/projection vertical. A new Java adapter implements an
inward-owned extraction port. Domain types contain no parser, filesystem,
runtime or persistence dependency. The existing tree-sitter runtime and compiler
build support are reused. Review the grammar's FFI binding and register its
transitive unsafe exception with the supply-chain policy; no first-party unsafe.

Snapshot v20 and Java v1 ontology, graph, query, docs, portable, error and viewer
namespaces are additive. Existing storage rows, schema and publication protocol
are reused without migration. Preserve Rust/Kotlin public contracts and use
separate Java declarations rather than translating Kotlin semantics.

The grammar provides syntax, not compiler truth. Record components and explicit
constructors are observed separately from generated members. Nested containment
is structural; signatures and imports carry unresolved types. Maven/Gradle
membership is a path convention only. JPMS and Unicode preprocessing remain
explicit file boundaries; malformed files supply no partial declaration graph.

The fixture/oracle, parser regressions and public journey precede or accompany
implementation. Two pinned public corpora have three retained observations each.
Run the full local gate and remote platform CI on the final review head, keeping
all failures visible. No workflow permissions, signing, publication, release,
network, compiler execution or automatic browser authority changes are needed.
