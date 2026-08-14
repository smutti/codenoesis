# CodeNoesis

> An epistemic, temporal and causal digital twin for software systems.

CodeNoesis is an early-stage project exploring how to turn software repositories into evidence-backed, evolving knowledge systems. Its intended scope includes source-code analysis, architecture and API documentation, cross-project dependency reasoning, change-impact analysis, and explicit representation of uncertainty and provenance.

## Status

**Implemented local Rust analysis through R16 and the first bounded S7 runtime;
the product is not yet production-ready or Verified.** The pinned Rust
workspace now provides deterministic repository acquisition, evidence-backed
Rust ontology extraction, local persistence, documentation, exact-ID query,
portable export, and versioned offline-explorer artifacts. The protected
milestone **Implemented local Rust analysis through R14** remains an immutable
compatibility checkpoint; later protected merges extend it rather than
replacing it.
The latest protected merges add committed-source expression and lexical-binding
facts, closed source-normal local flow, the bounded implementation-aware
HTTP/JSON impact pilot, the Issue #170 / Decision 0029 accepted R14/R15
fail-closed correction with its explicit scan-only `local-snapshot-256m-v1`
output envelope, and bounded safe Rust constant evaluation. Protected PR #173,
merged as `c3d05994a56e747fbe3157173998f8ac76ef7333`, made the exact Issue #172 /
Decision 0030 `rust-safe-constant-evaluation-v1` package Approved and
Implemented, but not Verified. It derives only checked target-independent
primitive constants and fixed-repr enum discriminants while retaining explicit
gaps for every unsupported case.

Issue [#176](https://github.com/smutti/codenoesis/issues/176),
[Decision 0031](docs/software/decisions/0031-s4-versioned-local-explorer-browser.md),
and protected PR #177 corrected the LocalExplorerV3-V9 browser defect on
`main`. Each generated explorer now binds to its exact PortableGraphV3-V9
schema and exposes bounded graph inspection while preserving the immutable
V1/V2 viewer assets. The correction is Approved and Implemented but not
Verified.

Issue [#178](https://github.com/smutti/codenoesis/issues/178),
[Decision 0032](docs/software/decisions/0032-s4-r17-function-context-navigation.md),
and protected PR #179 made the high-risk R17/S4 function-context package
Approved and Implemented but not Verified. Its opt-in deterministic
`FunctionContextV1` projection and additive LocalExplorerV10 function view over
unchanged R16 facts let humans and LLM consumers inspect declared inputs,
output spelling, calls, evidence, and uncertainty without reconstructing raw
graph identifiers.

Issue [#180](https://github.com/smutti/codenoesis/issues/180) and
[Decision 0033](docs/software/decisions/0033-g0-release-profile-registry.md)
were protected by PR #181, merged as
`a525126228205901885038586e21d30db745b1ec`. The exact G0/S14 registry and
output-only profile preflight under `FR-REL-001` and `FR-CLI-007` are Approved
and Implemented but not Verified; they grant no signing, publication, support,
or GA authority.

Issue [#182](https://github.com/smutti/codenoesis/issues/182) and
[Decision 0034](docs/software/decisions/0034-g1a-local-cli-distribution-configuration.md)
define the Proposed high-risk G1a/S14 local-first candidate. It adds a closed
`FR-CFG-001` configuration contract, `FR-CLI-008` fail-closed startup
validation, and an unsigned `FR-REL-002` digest-named staged CLI bundle with
explicit install, upgrade, rollback, and uninstall procedures. Before protected
merge it remains a candidate and is not a supported or published release.
Complete independent evidence acceptance, broader language coverage,
operations, and the remaining production-readiness slices are still pending.

## Two development tracks

The project is organized into two related but independently managed tracks:

- **[Software engineering](docs/software/README.md):** the production-grade Rust platform, its modular architecture, data contracts, extraction pipeline, operational requirements, and delivery roadmap.
- **[Research](docs/research/README.md):** the scientific questions, experimental techniques, hypotheses, benchmarks, and publication-oriented work that may inform or extend the product.

Research results may graduate into the software track only after reproducible evaluation and an explicit engineering decision. Production requirements may also generate new research questions, without making experimental components mandatory runtime dependencies.

## Principles

- **Evidence before narrative:** every important claim should be traceable to source code, contracts, configuration, runtime observations, or another explicit form of evidence.
- **Uncertainty is data:** unknown, inferred, contradicted, stale, and confirmed knowledge must remain distinguishable.
- **Time and context matter:** knowledge should be tied to revisions, configurations, environments, and validity periods.
- **Deterministic core, optional intelligence:** parsers and rules establish the factual baseline; probabilistic and LLM-assisted techniques remain bounded and auditable.
- **Modularity by default:** extraction, storage, ontology, federation, documentation, impact analysis, and interfaces should evolve behind versioned contracts.
- **Privacy and safety by design:** repository analysis should be local-first, least-privileged, and explicit about any external data transfer or code execution.

## Proposed documentation map

```text
docs/
├── software/
│   ├── README.md
│   ├── architecture.md
│   ├── software-requirements-specification.md
│   ├── data-model.md
│   ├── interfaces.md
│   ├── operations.md
│   └── roadmap.md
├── research/
│   ├── README.md
│   ├── research-agenda.md
│   ├── hypotheses.md
│   ├── experiments.md
│   ├── benchmarks.md
│   └── literature.md
└── decisions/
    └── README.md
```

This map describes the intended structure and will be introduced incrementally as decisions become concrete.

## Contributing

Start with [CONTRIBUTING.md](CONTRIBUTING.md) and the repository
[agent instructions](AGENTS.md). Product implementation must be tied to an
approved requirement, one vertical slice, a reviewable acceptance oracle, and
captured Red evidence. Specification, fixture, benchmark, and research work can
proceed independently when its status is explicit.

## License

CodeNoesis is licensed under the [Apache License, Version 2.0](LICENSE).
