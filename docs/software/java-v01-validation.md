# Java v0.1 validation observation

Source head: `e6e690b51e222305b44fc7f7ee223d36b532faa8` (clean). Base: `e0af187cf1d3e2b64bb6e2d57feda4f39e7da1b2`.
Release binary SHA-256: `a2c0c90db688e88498d60257535a44ecc98b865956a4c5bb516e6e1a66d15982`.

Environment: macOS-26.5.2-arm64-arm-64bit, arm64, rustc 1.97.1 (8bab26f4f 2026-07-14); `cargo build -p noesis --release --locked`.

These are observations on two small public samples, not a performance baseline,
compiler accuracy estimate, cross-host comparison or SLO. All six attempts,
including the slower first Spring scan, are retained. No warm-up was discarded.

| Sample | Declarations | Entities | Edges | Gaps | Median s | Range s | Repeats |
|---|---:|---:|---:|---:|---:|---|---|
| spring-guides/gs-rest-service | 17 | 58 | 59 | 9 | 0.121 | 0.118–0.765 | 3/3, semantic hash stable |
| junit-team/junit-examples | 197 | 479 | 595 | 53 | 0.297 | 0.290–0.315 | 3/3, semantic hash stable |

Pins and licenses: [manifest](../../tests/specifications/s8/java-public-v1.json).
Complete [scan observations](evidence/java-v01-public-2026-09-07.json)
and [export/viewer observations](evidence/java-v01-projections-2026-09-07.json).
Raw stdout/stderr and local stores are retained outside Git in the output paths
recorded by these reports. Only metrics and hashes are committed; source repositories
and generated source projections are not redistributed in this change.

## Gap assessment

Spring has the three global boundaries plus six Kotlin files. JUnit has the
three global boundaries, seven Kotlin files, 34 files outside the module/root
convention, seven JPMS descriptors and two source-launcher files:

- `junit-source-launcher/lib/DownloadRequiredModules.java` uses top-level members
  and an implicit enclosing class, outside this named-declaration profile.
- `junit-source-launcher/src/HelloTests.java` includes `import module` and an
  implicit class; the pinned grammar rejects it, so it contributes no partial facts.

These are visible limits of the selected profile and grammar. Neither source
launcher nor any wrapper/download/build code was executed. No corpus entry was
excluded or retried to improve the result.

## Traceability and verification

| Contract | Executable evidence |
|---|---|
| 26 declarations, 22 nested edges, overload signatures and gaps | `e2e_fr_ext_026_scan_store_query_docs_export_and_offline_viewer` against the hand-authored [oracle](../../tests/specifications/s8/java-declarations-v1.json) |
| Named syntax, UTF-8 spans, record components, constants, no local/anonymous decoys | `fr_ext_026_*` in `codenoesis-lang-java/tests/s8_declarations.rs` |
| Source byte, node and depth caps | `fr_ext_026_source_failures_do_not_synthesize_declarations`, `fr_ext_026_syntax_node_and_depth_limits_are_typed` |
| Immutable Git input, no partial store, selector conflicts and unsafe output | `e2e_fr_ext_026_*` and `sec_fr_ext_026_*` in `noesis/tests/s8_java.rs` |
| Canonical integrity, lineage, claim/reference and boundary preservation | four `ct_fr_ext_026_*` contract tests |
| Offline executable CSP and LF/CRLF determinism | `test_fr_ext_026_script_and_style_have_exact_csp_hashes`, `pt_fr_ext_026_viewer_bytes_are_identical_for_lf_and_crlf_checkouts` |

The authored oracle exactly matches all 26 declarations: precision and recall
are both 1.0 for `(path, kind, name, owner)` on this fixture only. Two overload
signatures and 22 containment edges have separate exact assertions. This does
not establish precision/recall on the public corpus or on unrepresented syntax.

Focused parser, contract, CLI and CSP tests and workspace Clippy passed before
measurement. The final complete eight-command gate and platform CI are reported
on the PR review head; this source-head observation does not predeclare their
result. Local unsafe inventory accepted all 94 selected macOS packages, including
the exact Java grammar exception. A fresh vulnerability audit was not run locally
because cargo-audit is unavailable; the unchanged Linux supply-chain CI performs
that required check with version 0.22.2.

Both public stores exported and generated offline viewers successfully (4/4
projection commands). Interactive browser inspection was not run: the session’s
automatic browser approval review rejected local file navigation. No alternate
browser, local server or browser-policy workaround was used. Static/CSP and CLI
generation checks are distinct from interactive visual acceptance.

Agent: Codex / GPT-6; run `java-v01-20260907`. Cost/token telemetry unavailable.
Manual merge remains with the maintainer. The original dirty checkout is preserved.
