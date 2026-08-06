# S4 R8 portable explorer fixture

This project-owned fixture reviews the R8 interchange and offline-view contracts
without vendoring a source repository. `portable-graph.json` is one
RFC 8785 canonical JSON payload followed by one LF and preserves the exact R7
entity, relationship, claim, evidence, diagnostic, coverage-gap, document, and
document-statement shapes.

`source-family-digests.json` is the independent lossless-reimport oracle.
`explorer-manifest.json` binds the portable graph, static entrypoint, CSP
profile, capabilities, and limits. `index.html` is a non-canonical,
reconstructable materialized view.

To inspect the fixture manually:

1. open `index.html` directly as a local file;
2. choose `portable-graph.json` in the file picker;
3. search an exact stable ID or NFC text;
4. select a result and inspect its deterministic depth-1 or depth-2
   neighborhood.

The page never fetches data, starts a server, opens another process, persists
browser state, needs the analyzed repository, or interprets graph values as
active markup. Source contents and snippets are intentionally absent.
