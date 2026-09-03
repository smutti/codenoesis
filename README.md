# CodeNoesis

> An epistemic, temporal and causal digital twin for software systems.

CodeNoesis is an early-stage project exploring how to turn software repositories into evidence-backed, evolving knowledge systems. Its intended scope includes source-code analysis, architecture and API documentation, cross-project dependency reasoning, change-impact analysis, and explicit representation of uncertainty and provenance.

## Status

**The bounded 34-profile local baseline through S0-S7, R0-R19, R0-R17/K1,
and G0-G8-local is Verified; the product is not yet production-ready or
generally available.** Issue
[#201](https://github.com/smutti/codenoesis/issues/201) and protected PR #204
independently accepted the exact 32-profile V2 baseline plus R18 trusted local
source retrieval and R19 Git-backed semantic impact. Review head
`75391c9061d691c1d6efdf8b726e120049389476` merged as
`3fb6504d1d6cb39f204eca032ff816266194e1ec`. The retained V2/V3 candidate
markers remain immutable pre-activation evidence; protected manual merge is
the external lifecycle event. Issue #141 is closed as superseded. G9 remains a
separate governed package.

That Verified baseline provides deterministic loose/packed local Git
acquisition, safe gitlink boundaries, evidence-backed Rust ontology extraction
through R16, atomic local persistence, documentation, exact-ID query, portable
export, versioned offline exploration, R17 `FunctionContextV1`, the bounded S7
implementation-aware HTTP/JSON pilot, R18 immutable evidence-to-source
retrieval, R19 Git-backed semantic impact, and local configuration,
distribution, upgrade, and supply-chain preflights. Verification does not grant
signing, publication, support, a release channel, EOL, SLA, or GA authority.
The historical milestone **Implemented local Rust analysis through R14**
remains an immutable compatibility checkpoint within that verified baseline.

Immutable historical conformance metadata is retained for issue #170 and
`local-snapshot-256m-v1`; issue #176 and Decision 0031; issue #178 and Decision 0032
with `FunctionContextV1`; issue #180, Decision 0033, and `FR-REL-001`; issue
#182, Decision 0034, and `FR-CFG-001`; issue #184, Decision 0035, `FR-CMP-001`,
and `FR-CLI-009`; issue #186, Decision 0036, `FR-REL-003`, and `FR-CLI-010`;
and `rust-safe-constant-evaluation-v1`. Their historical lifecycle label was
**Approved and Implemented but not Verified**; the current bounded status is
the independently verified baseline described above.

Issue [#205](https://github.com/smutti/codenoesis/issues/205) and
[Decision 0042](docs/software/decisions/0042-real-world-rust-stability-benchmark.md)
define the Proposed high-risk B1/G7 observational benchmark candidate over
pinned Lekton and RustDesk. It changes no product or ontology byte and makes no
SLO, release, support, cross-host, conference-validity, or GA claim before
independent review and protected merge.

Broader language coverage, operations, semantic comparison, and the remaining
production-readiness slices are still pending.

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
