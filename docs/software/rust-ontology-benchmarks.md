# Inspecting and benchmarking Rust ontology results

CodeNoesis has a versioned 34-profile verification pack, a frozen eight-repository
public evaluation, and pinned Lekton/RustDesk pilots. They measure regression,
robustness, emitted facts, repeatability, and performance. They do not establish
source-level precision or recall for the entire ontology.

For newer binaries, use the explicit candidate observation documented in
[benchmarks/README.md](../../benchmarks/README.md). Its report separates extraction
success from historical oracle agreement. A typed rejection is a failed
extraction with a diagnosable boundary. Three equal hashes demonstrate
repeatability, not semantic correctness. Whole-process duration includes
publication/output and is distinct from the 75-second confined scan deadline.

## External references

Primary sources checked on 2026-09-06:

| Reference | Useful comparison | Scope limitation |
|---|---|---|
| [rust-analyzer SCIP exporter and tests](https://github.com/rust-lang/rust-analyzer/blob/9074e9b4c6bc31d7986ccf4e22d18af70e5508da/crates/rust-analyzer/src/cli/scip.rs) | Definitions, references, parameters, locals, declared containers, and source positions. | The inspected exporter emits definition/reference roles and leaves symbol relationships empty. It does not provide an oracle for READS/WRITES, local flow, checked constants, ownership, or all declared cfg alternatives. |
| [SCIP snapshot, test, and lint](https://github.com/scip-code/scip/blob/1c2b6db7e560d5233c944f36e4ac1377cc6963fc/docs/CLI.md) | Human-readable source annotations, explicit expected assertions, and index consistency. | A generated snapshot accepted automatically can preserve an extractor defect. These tools do not supply independent truth by themselves. |
| [Joern rust2cpg tests](https://github.com/joernio/joern/tree/aa6928b46cf90e4fd9c299b8c7c52b751a40a2a1/joern-cli/frontends/rust2cpg) | External examples for parameters, binding/shadowing, loops, and source structure, including MethodTests, ForTests, and CfgAttributeTests. | CPG lowering and active-cfg behavior differ from the source-only ontology. Map individual common facts; do not compare raw graph counts. |
| [CodeQL language support](https://codeql.github.com/docs/codeql-overview/supported-languages-and-frameworks/) | A possible future query-based semantic/security comparator. The inspected documentation includes Rust 2021/2024. | Toolchain requirements, query semantics, versions, and configuration must be fixed first. Language support is not an ontology accuracy benchmark. |

No directly applicable whole-CodeNoesis-ontology accuracy benchmark was found
in these sources. The external tools were inspected, not executed as part of
this closure; no external agreement or accuracy score is claimed.

## Recommended evaluation

Keep the public corpus for scale and repeatability. Add a separately reviewed
source oracle for the common declaration/reference subset, using rust-analyzer
SCIP and readable SCIP assertions as a differential comparison. Pin tool and
source commits, target, features, and cfg configuration. Normalize paths, fact
kinds, and spans, including text encoding and name-versus-body spans; product
hash IDs cannot be compared directly.

For local binding and flow, use explicitly reviewed fixtures, including external
Joern examples where their semantics match, plus shadowing, unsupported iterator,
scope-boundary, and read/write cases. Differences are review candidates rather
than automatic proof that either tool is correct. Do not silently exclude
unsupported or difficult source cases.

The review view should connect **source → expected fact → extracted fact →
result**, with exact evidence and explicit gaps, alongside the offline graph
viewer. Report precision `TP/(TP+FP)` and recall `TP/(TP+FN)` only against a
reviewed expected set with a declared denominator. Without that oracle, call
the comparison differential agreement. Keep coverage, determinism, performance,
and source accuracy as separate metrics.
