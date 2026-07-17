# CodeNoesis

> An epistemic, temporal and causal digital twin for software systems.

CodeNoesis is an early-stage project exploring how to turn software repositories into evidence-backed, evolving knowledge systems. Its intended scope includes source-code analysis, architecture and API documentation, cross-project dependency reasoning, change-impact analysis, and explicit representation of uncertainty and provenance.

## Status

**Project inception.** The problem space, architecture, research hypotheses, and evaluation strategy are being defined. There is no working implementation or stable public interface yet.

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

The project is not yet ready for implementation contributions. For now, contributions should focus on clearly scoped proposals, references, testable hypotheses, architecture decisions, and review of the two track documents.
