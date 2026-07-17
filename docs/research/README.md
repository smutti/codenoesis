# CodeNoesis Research Track

## Status

This directory defines the research programme behind CodeNoesis.

> **Current status:** research agenda only. No research prototype, dataset,
> experiment, benchmark result, or scientific claim described here has been
> implemented or validated yet.

Terms used in these documents have the following meaning:

| Label | Meaning |
|---|---|
| **Planned** | Accepted into the research agenda, but work has not started. |
| **Experimental** | Intended for a research prototype and empirical evaluation; not a production commitment. |
| **Moonshot** | High-risk direction that requires feasibility work before it can enter the main programme. |
| **Implemented** | Supported by code and tests in this repository. Nothing in this research track currently has this status. |

The detailed programme, research questions, evaluation protocol, and work
packages are in the [research agenda](research-agenda.md).

## Research thesis

CodeNoesis investigates an **Epistemic Software Twin**: a representation of a
software ecosystem that records not only what the system appears to contain,
but also:

- what is known, unknown, disputed, or contradicted;
- which evidence supports or challenges each assertion;
- when an assertion was valid and when it became known;
- under which revision, deployment, build target, or feature configuration it
  holds;
- which observation or experiment could falsify it;
- whether a change is merely connected to a component or is likely to cause an
  observable effect.

The central research object is therefore not a static graph edge. It is a
versioned, world-dependent, evidence-bearing assertion.

```text
Assertion
|- subject / predicate / object
|- valid time / recorded time
|- world condition
|- supporting evidence / counter-evidence
|- derivation and extractor lineage
|- epistemic state and calibrated uncertainty
|- observation coverage
`- reproducible witness or falsification obligation
```

## Why this is a research problem

Static code graphs, repository retrieval, GraphRAG, and generated wikis are
useful foundations, but they usually optimize structural extraction or context
retrieval. They are not, by themselves, an integrated account of:

- competing truths across branches, releases, deployments, and feature flags;
- disagreement between intended, implemented, configured, and observed
  behaviour;
- causal impact rather than graph reachability;
- behavioural contracts that are absent from interface schemas;
- negative evidence, contradictions, and incomplete observation coverage;
- verifiable documentation whose claims carry replayable witnesses;
- the value and cost of acquiring another piece of evidence;
- correlated failures in multi-agent review;
- cross-project reasoning when source code cannot be shared.

The programme does not claim that these topics are absent from prior research.
Its hypothesis is that they remain under-integrated and under-evaluated for
temporal, polyglot, cross-repository software ecosystems.

## Research pillars

| Pillar | Proposed contribution | Status |
|---|---|---|
| Bitemporal, multi-world knowledge | Represent valid time, observation time, and symbolic configuration conditions without enumerating every possible world. | Planned, experimental |
| Static-runtime software twin | Reconcile intended, implemented, configured, observed, and experienced views of the same system. | Planned, experimental |
| Causal change impact | Estimate whether a change causes a failure, not only whether a graph path exists. | Planned, experimental |
| Behavioural contract discovery | Infer protocol, ordering, invariant, side-effect, and performance assumptions from tests and traces. | Planned, experimental |
| Epistemic and paraconsistent graph | Preserve support, counter-evidence, contradiction, ignorance, coverage, and calibrated uncertainty. | Planned, experimental |
| Proof-carrying documentation | Attach a reproducible witness to every generated claim and invalidate stale claims when the witness no longer holds. | Planned, experimental |
| Repository context compiler | Compile a question into the smallest evidence-complete subgraph instead of retrieving arbitrary top-k chunks. | Planned, experimental |
| Adaptive evidence acquisition | Select the next parser, index, test, trace, reviewer, or human question by expected information value. | Planned, experimental |
| Governed ontology evolution | Turn failed queries into competency questions and reviewable ontology-change proposals. | Planned, experimental |
| Calibrated Council | Use independent, evidence-diverse reviewers as a sequential experiment rather than a majority-vote prompt pattern. | Planned, experimental |
| Private federation | Detect cross-project impact without centralizing all source code. | Moonshot |
| Sheaf-based consistency | Detect where locally valid project views fail to compose into a coherent global system view. | Moonshot |

## Separation from the software track

The production software and the research programme must evolve independently:

- production features require deterministic contracts, security, operations,
  compatibility, and acceptance tests;
- research components require explicit hypotheses, baselines, controlled
  experiments, ablations, uncertainty reporting, and reproducible artifacts;
- an experimental result does not become a product dependency automatically;
- promotion into production requires an architecture decision, an operational
  threat assessment, and measurable improvement over the deterministic
  baseline;
- production telemetry may be used for research only under an explicit data
  governance and privacy policy.

## Expected research outputs

The programme is intended to produce:

1. a formal model for evidence-bearing, bitemporal, multi-world software
   assertions;
2. `TemporalCrossRepoImpactBench`, a benchmark for temporal and cross-project
   change-impact reasoning;
3. reference experimental implementations isolated from the production core;
4. empirical comparisons with static code graph, lexical/vector retrieval,
   GraphRAG, and fixed-panel multi-agent baselines;
5. reusable datasets, manifests, evaluation scripts, and provenance records;
6. publications covering the individual work packages and their integration.

## Research principles

- **Evidence before narration:** an LLM may verbalize or propose a hypothesis,
  but it is not a source of truth.
- **Unknown is a valid result:** the system must abstain when the evidence is
  insufficient.
- **Contradictions are preserved:** conflicting observations are research data,
  not values to average away.
- **Temporal leakage is controlled:** evaluation splits follow time and project
  boundaries.
- **Negative results are recorded:** failed hypotheses, unsupported ontology
  extensions, and ineffective Council roles remain part of the experimental
  record.
- **Privacy is part of correctness:** a method that improves impact detection by
  exposing protected source code has not solved the target problem.
- **Claims remain provisional:** this documentation describes intended research,
  not demonstrated outcomes.

## Starting points

The programme builds on, but intends to go beyond, these research and
open-source directions:

- [RepoGraph](https://openreview.net/forum?id=dw9VUsSHGB) for repository-level
  code graph context;
- [RepoDoc](https://arxiv.org/abs/2604.26523) for repository documentation
  grounded in a semantic graph;
- [ReCUBE](https://arxiv.org/abs/2603.25770) for repository-level code
  understanding;
- [Code Digital Twin](https://arxiv.org/abs/2503.07967) for digital-twin views of
  source code;
- [Graphify](https://github.com/safishamsi/graphify) and
  [LLMWiki](https://github.com/lucasastorian/llmwiki) as practical code-graph and
  persistent-wiki references;
- [SCIP](https://github.com/sourcegraph/scip) for compiler-grade symbol and
  occurrence exchange;
- [W3C PROV-O](https://www.w3.org/TR/prov-o/) and
  [SHACL](https://www.w3.org/TR/shacl/) for provenance and graph validation;
- [Council of High Intelligence](https://github.com/0xNyk/council-of-high-intelligence)
  and [PR-AF](https://github.com/Agent-Field/pr-af) as patterns to investigate
  for structured, evidence-aware review.

These references are inspirations and candidate baselines. Their presence here
does not imply that CodeNoesis has implemented or reproduced them.
