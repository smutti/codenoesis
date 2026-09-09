# Code and tooling obsolescence audit

Scope: main `399ee12b31a5d0c4fb1b3a59e7badb012e81a0d2` and the Local GA
benchmark package. This is a repository-local audit, not a proof that no public
API has an external caller. No product source or public format is changed.

## Checks and disposition

| Check | Observation | Disposition |
|---|---|---|
| Cargo metadata | 13 packages, 141 targets; every workspace dependency declaration is consumed by a package manifest | No unused workspace dependency declaration removed. This does not prove that every imported dependency is needed at runtime. |
| Tracked Rust module/target references | 347 tracked Rust files, including fixture sources; the only filename candidate is `tests/fixtures/s4/compiler-index-v1/repository/crates/client/src/omitted.rs` | Keep: the fixture README, manifest, binding, golden overlay and R7 contract intentionally test an omitted SCIP document. |
| Production lint suppression | No first-party production `allow(dead_code)` or `expect(dead_code)` found | Compiler/clippy checks remain enabled. Occurrences inside test source strings are parser inputs. |
| Rust function names occurring once | Mostly `#[test]` functions and the YAML parser's trait callback `on_event`; five public helper APIs listed below | Keep public APIs pending a separately reviewed API deprecation decision; an absent in-repository call is insufficient evidence of external disuse. |
| Python AST definitions versus tracked references | `profile_projection` in `scripts/verify_local_baseline_v3.py` has no caller, import, documented use or dynamic dispatch | Remove its unused projection body; retain existing V3 validation and its regression tests. |
| Historical snapshot/SCIP readers | Versioned readers and deprecated SCIP range conversion are referenced by compatibility and validation paths | Keep. Old formats are an active compatibility obligation. |
| Historical benchmark runners | B1/progressive runners are manifest-bound and tested; Java/Kotlin entry points reproduce their committed v1 report schemas | Keep and document as historical reproduction. The new complete-profile runner is the current cross-language observation entry point. |

The five public API candidates are:

- `RepositorySnapshotV16::from_inventory_and_expression_bindings` — convenience
  wrapper around the boundary-aware constructor with `None` boundaries.
- `RepositorySnapshotV17::from_inventory_and_local_flow` — equivalent R15 wrapper.
- `RepositorySnapshotV18::from_inventory_and_constant_evaluation` — equivalent R16 wrapper.
- `r14_read_relationships` in the R15 domain module.
- `SqliteMetadataStore::database_path` in the local store module.

These are candidate API simplifications, not confirmed dead product behavior.
Private dead code is additionally covered by the complete clippy gate with
warnings denied. Textual reference checks can miss generated names, trait
dispatch and consumers outside this repository; the audit explicitly reviewed
those categories rather than deleting every one-occurrence symbol.

The existing original checkout and its uncommitted R19 files were not cleaned,
stashed, reset or updated. Old worktrees and raw benchmark evidence were not
deleted: neither their age nor their absence from current source imports makes
them disposable user data.
