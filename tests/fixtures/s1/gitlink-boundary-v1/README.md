# S1 gitlink boundary fixture

This project-owned fixture overlays the accepted S4 workspace source with one
committed `.gitmodules` blob and one synthetic Git tree entry whose mode is
`160000`. The nested commit uses Git's empty tree, so no external repository
source or nested worktree content is committed here.

The implementation harness constructs loose or approved packed Git objects
from `manifest.json` before the monitored `noesis` process starts. It also
creates absent, present-but-not-supplied, explicitly supplied, mismatched,
malformed, escaping, duplicate, orphan, recursive-input, and maximum-plus-one
variants from the bounded recipes in that manifest. Generated repositories,
bulk limit data, and third-party source are never committed.

The primary success proves that the root S4 analysis continues while the
nested checkout is absent. The gitlink remains a separate external repository
boundary; no nested source fact enters the root inventory, extraction chunks,
knowledge graph, documentation, or query results.

The `.gitmodules` URL is inert metadata. The product may classify its lexical
shape and retain only a SHA-256 digest. It must not resolve the URL, open a
network connection, invoke Git, inspect Git configuration, discover a
worktree, run a hook, or use credentials.

All material in this directory is covered by the repository-wide Apache-2.0
license.
