# R18 trusted source retrieval fixture descriptor

R18 reuses the immutable project-owned R17 function-context Git fixture rather
than copying or changing it. The selected `scale` signature evidence already
exists in validated RepositorySnapshotV18 and FunctionContextV1. This descriptor
freezes the expected evidence-to-source output while preserving every R17
fixture, graph, query, portable, and explorer byte.

Only the reviewed excerpt in `expected-source-excerpt.json` may enter retained
test material. Real-repository pilots retain digests and relative locators but
never external source text or private roots.
