# CodeNoesis

> An epistemic, temporal and causal digital twin for software systems.

CodeNoesis is an early-stage project exploring how to turn software repositories into evidence-backed, evolving knowledge systems. Its intended scope includes source-code analysis, architecture and API documentation, cross-project dependency reasoning, change-impact analysis, and explicit representation of uncertainty and provenance.

## Status

**Implemented local Rust analysis through R15 and the first bounded S7 runtime,
with R16 safe constant evaluation now a branch-scoped candidate; the product is
not yet production-ready or Verified.** The pinned Rust workspace now provides deterministic repository
acquisition, evidence-backed Rust ontology extraction, local persistence,
documentation, exact-ID query, portable export, and an offline explorer. The
protected milestone **Implemented local Rust analysis through R14** remains an
immutable compatibility checkpoint; later protected merges extend it rather
than replacing it.
The latest protected merges add committed-source expression and lexical-binding
facts, closed source-normal local flow, the bounded implementation-aware
HTTP/JSON impact pilot, and the accepted R14/R15 fail-closed correction with
its explicit scan-only `local-snapshot-256m-v1` output envelope. Issue #172 and
Decision 0030 define the Proposed `rust-safe-constant-evaluation-v1` candidate,
which derives only checked target-independent primitive constants and fixed-
repr enum discriminants while retaining explicit gaps for every unsupported
case.
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
