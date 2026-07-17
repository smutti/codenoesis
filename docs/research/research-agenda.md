# CodeNoesis Research Agenda

## Document status

- **Programme state:** planned
- **Implementation state:** not implemented
- **Experimental results:** none
- **Benchmark releases:** none
- **Last interpretation:** this document contains hypotheses and proposed
  protocols, not validated scientific conclusions

## 1. Programme objective

The research objective is to determine whether an **Epistemic Software Twin**
can make repository documentation and cross-project impact analysis more
accurate, better calibrated, more explainable, and more privacy-preserving than
static code graphs or retrieval-augmented generation alone.

The proposed twin combines five views:

| View | Candidate evidence |
|---|---|
| **Intended** | requirements, ADRs, documentation, ownership policies |
| **Implemented** | source code, ASTs, compiler indexes, interface contracts |
| **Configured** | manifests, infrastructure as code, deployments, feature flags |
| **Observed** | tests, distributed traces, logs, metrics, eBPF observations |
| **Experienced** | incidents, rollbacks, corrections, human review decisions |

The research problem is not simply to merge those views. It is to preserve
where they agree, where they conflict, what was not observed, and which new
measurement would best reduce uncertainty.

## 2. Gap relative to current approaches

Static code graphs and compiler indexes provide strong evidence about declared
structure and symbol relationships. GraphRAG and repository-level retrieval can
select useful context for a model. Generated-wiki systems can turn that context
into readable documentation.

The proposed research focuses on capabilities that are often studied
separately and remain insufficiently evaluated as one cross-repository system:

1. **Time and worlds:** a fact may be valid on one branch, release, platform,
   tenant, or feature-flag assignment and false in another.
2. **Observation mismatch:** code structure does not prove that a path is
   deployed, reachable, exercised, or compatible at runtime.
3. **Causality:** reachability identifies possible exposure, while teams need to
   know whether an intervention is likely to cause a failure.
4. **Implicit behaviour:** schemas omit call ordering, retries, side effects,
   latency assumptions, state machines, and undocumented error semantics.
5. **Epistemic state:** a scalar confidence score cannot adequately distinguish
   false, unknown, contradicted, unobserved, and stale.
6. **Verifiability:** citations are traceable, but not necessarily replayable
   proofs of a documentation claim.
7. **Acquisition policy:** exhaustive parsing, execution, tracing, LLM review,
   and human review are too expensive to apply to every claim.
8. **Reviewer dependence:** a Council can manufacture apparent consensus when
   its seats share prompts, evidence, model families, or systematic errors.
9. **Federation boundaries:** cross-project reasoning is hardest when
   repositories belong to different teams or organizations and cannot be
   centralized.

## 3. Research questions

### RQ1 — Temporal and configuration semantics

Can a bitemporal graph with symbolic world conditions represent branch,
release, build-target, deployment, and feature-flag differences more compactly
and accurately than materializing one graph per configuration?

### RQ2 — Static-runtime reconciliation

Does reconciling intended, implemented, configured, observed, and experienced
evidence improve the detection of dark dependencies, dead paths,
architecture drift, and undocumented behaviour?

### RQ3 — Causal impact

Can intervention evidence from mutation, controlled replay, test execution, or
natural release experiments distinguish a causally affected client from one
that is only reachable in a dependency graph?

### RQ4 — Behavioural contracts

How accurately can symbolic automata, typestate, invariants, metamorphic
relations, and temporal properties recover client assumptions that are absent
from OpenAPI, protobuf, GraphQL, or language signatures?

### RQ5 — Epistemic representation

Do paraconsistent states, separate support and counter-evidence, and explicit
coverage produce better calibrated answers than one confidence value per node
or edge?

### RQ6 — Proof-carrying knowledge

Can generated documentation remain useful while every material claim carries a
replayable witness, invalidation condition, and evidence lineage?

### RQ7 — Context compilation

Can a query be compiled into a smaller evidence-complete subgraph than top-k
chunk retrieval or unconstrained graph expansion, without reducing answer
correctness?

### RQ8 — Adaptive acquisition

Can expected value of information select the next extractor, compiler index,
test, trace, falsifier, or human question with a better accuracy-cost frontier
than a fixed pipeline?

### RQ9 — Calibrated Council

When does an independent, sequential, evidence-diverse Council improve
calibration and falsification, and when does correlated review make it worse
than a single strong verifier?

### RQ10 — Private federation and compositional consistency

Can contract capsules, privacy-preserving matching, or sheaf-inspired
consistency checks identify cross-project incompatibilities without exposing
source code or requiring one globally centralized graph?

## 4. Proposed technical investigations

Everything in this section is **planned experimental work**, not implemented
functionality.

### 4.1 Bitemporal, multi-world assertion graph

The candidate primitive is an immutable assertion:

```text
Assertion = {
  subject, predicate, object,
  valid_from, valid_to,
  recorded_from, recorded_to,
  world_condition,
  supporting_evidence[],
  counter_evidence[],
  derivation,
  epistemic_state,
  coverage,
  witness
}
```

`valid_time` describes when the assertion holds in the software system.
`recorded_time` describes when CodeNoesis learned, corrected, or superseded it.
`world_condition` is intended to encode revision, build target, operating
system, feature flags, deployment, or tenant conditions symbolically.

Candidate techniques include binary decision diagrams, SAT/SMT constraints,
and incremental truth maintenance. The experiment must compare their storage,
update, query, and explanation costs against per-world snapshot materialization.

### 4.2 Static-runtime reconciliation

Each evidence source contributes a view rather than overwriting another view.
The reconciliation layer should classify mismatches such as:

- `architecture_drift` — implementation contradicts an intended architecture;
- `dark_dependency` — runtime evidence reveals an undeclared interaction;
- `dead_path` — a structural path has adequate opportunity but no observation;
- `configuration_split` — deployments instantiate incompatible worlds;
- `undocumented_behaviour` — observed behaviour lacks intended or implemented
  documentation;
- `violated_assumption` — a client relies on behaviour the provider no longer
  exhibits.

Absence of a trace must not be interpreted as evidence of absence unless the
observation coverage and opportunity to observe are sufficient.

### 4.3 Causal change-impact analysis

The baseline is bounded graph reachability. The proposed causal layer will
study structural causal models and intervention evidence:

```text
change intervention
-> contract or behavioural delta
-> propagation mechanism
-> client assumption
-> observable outcome
```

Candidate interventions include controlled mutations, differential builds,
test replay, traffic replay in an isolated environment, and natural experiments
across releases. The output should separate:

- reachable;
- exposed under a world condition;
- assumption violated;
- failure observed after intervention;
- causally supported impact;
- unresolved impact.

The system must report alternative explanations and confounders instead of
presenting a causal score as proof.

### 4.4 Behavioural contract discovery

Candidate contract forms include:

- symbolic finite-state machines and protocol automata;
- typestate and call-order constraints;
- invariants over inputs, outputs, and state transitions;
- retry, idempotency, timeout, and error-handling assumptions;
- side-effect and resource-ownership constraints;
- latency or throughput envelopes;
- metamorphic relations that remain valid across transformations.

Contracts should be inferred independently from tests and runtime traces, then
checked against each other and against declared schemas. Differential and
metamorphic testing will provide falsification evidence.

### 4.5 Paraconsistent and uncertain knowledge

A candidate four-valued state distinguishes:

| Support present | Counter-evidence present | State |
|---:|---:|---|
| Yes | No | supported |
| No | Yes | refuted |
| Yes | Yes | contradictory |
| No | No | unknown |

This state is separate from observation coverage and probability. Uncertainty
should be decomposed into entity resolution, relation extraction, extractor
reliability, evidence dependence, temporal staleness, and world coverage.

Evaluation should include calibration under distribution shift and selective
prediction: the system may abstain and identify the missing evidence needed to
answer safely.

### 4.6 Proof-carrying documentation and context compiler

Every generated material claim should carry a witness that can be replayed:

- a source span or contract element;
- a graph query and snapshot hash;
- a deterministic rule derivation;
- a passing test, trace segment, or intervention result;
- a human review decision with provenance.

If the witness no longer succeeds, the claim becomes `stale` or `unsupported`;
it is not silently rewritten into a new fact.

The proposed Repository Context Compiler translates a question into evidence
obligations and a minimal sufficient evidence subgraph:

```text
question
-> typed query plan
-> evidence obligations
-> candidate subgraph
-> completeness and contradiction checks
-> answer or calibrated abstention
```

Candidate algorithms include program slicing, constrained Steiner trees,
information bottlenecks, flow diffusion, and constraint solving. Evaluation
must measure both evidence sufficiency and context reduction.

### 4.7 Adaptive evidence acquisition and ontology evolution

An acquisition planner should estimate the expected value of a possible action:

```text
expected reduction in decision risk
-----------------------------------
cost + latency + privacy exposure + execution risk
```

Actions may include running a richer parser, requesting SCIP, executing a test
in a sandbox, collecting a trace, invoking a falsifier, or asking a human.

Ontology evolution follows a governed loop:

```text
failed or low-coverage query
-> competency question
-> candidate concept or relation
-> benchmark replay and compatibility check
-> reviewable ontology proposal
-> accept, revise, or reject
```

The research agent must never modify the production ontology autonomously.

### 4.8 Calibrated Council

The Council is studied as a sequential evidence-acquisition process rather than
as a fixed majority vote:

1. seats produce blind, independent assessments;
2. roles use meaningfully different evidence channels or verification methods;
3. the system estimates disagreement and error correlation;
4. the next action acquires evidence rather than merely another opinion;
5. a stopping rule returns supported, refuted, contradictory, or unknown;
6. critical dissent and missing quorum route to human review.

Candidate metrics include Brier score, expected calibration error, selective
risk, marginal information gain, seat error correlation, and effective panel
size. Baselines include a single verifier, self-consistency, fixed debate, and a
fixed-role Council.

### 4.9 Private federation and sheaf-based consistency

This is moonshot work and must first pass a feasibility stage.

Private federation may publish signed semantic dependency capsules containing
contract fingerprints, consumed operations, version constraints, and proof
commitments. Candidate protocols include private-set intersection,
confidential-computing attestations, and limited zero-knowledge proofs.

A sheaf-inspired model treats each repository or deployment as a local view and
interfaces as restriction maps. Failure to construct a coherent global section
could localize incompatibility to a contract, semantic interpretation,
configuration, or version boundary.

Evaluation must include confidentiality leakage, false matches, computation
cost, and the diagnostic value of detected obstructions.

## 5. TemporalCrossRepoImpactBench

### 5.1 Purpose

`TemporalCrossRepoImpactBench` is the proposed primary benchmark. It should
evaluate whether a method can identify and explain affected consumers across
time and repository boundaries, rather than merely retrieve related code.

No benchmark artifact currently exists.

### 5.2 Unit of evaluation

Each benchmark case should include:

- a provider revision before and after a change;
- one or more declared interface or behavioural deltas;
- real clients, hard negative clients, and semantically plausible decoys;
- build and runtime configuration worlds;
- available static, contract, test, and runtime evidence;
- a ground-truth impact label and evidence lineage;
- timestamps that support temporal evaluation without future leakage.

Ground truth should distinguish:

1. declared dependency;
2. structural reachability;
3. runtime exposure;
4. violated client assumption;
5. observed breakage;
6. causally supported breakage.

### 5.3 Candidate corpus

The first corpus should prioritize ecosystems with public history, polyglot
clients, machine-readable contracts, and reproducible builds. Candidate sources
include OpenTelemetry specifications and SDKs, protobuf/gRPC ecosystems,
Kubernetes OpenAPI clients, and selected real breaking changes augmented by
controlled mutations.

Dataset inclusion requires license review, provenance, reproducible revision
pinning, and a documented path from evidence to label.

### 5.4 Tasks

- temporal entity lineage and cross-repository identity resolution;
- provider-to-consumer retrieval and ranking;
- breaking-change classification;
- evidence-path generation and validation;
- causal-impact ranking;
- calibrated abstention under missing evidence;
- minimal evidence-complete context compilation;
- documentation claim support and stale-claim detection;
- Council action selection and stopping;
- privacy-preserving consumer matching.

### 5.5 Dataset splits and leakage controls

- split by time so that future fixes and documentation are unavailable;
- hold out complete repositories or ecosystems for transfer evaluation;
- prevent generated variants of the same mutation from crossing splits;
- track possible model-pretraining contamination separately;
- maintain a sealed test set for final comparisons;
- report results by language, contract type, change class, evidence availability,
  and repository size.

## 6. Evaluation protocol

### 6.1 Baselines

At minimum, experiments should compare with:

- lexical search and symbol search;
- vector retrieval over source chunks;
- static dependency and call-graph reachability;
- repository-level GraphRAG;
- contract-only breaking-change analysis;
- generated documentation without proof obligations;
- fixed full-analysis pipelines;
- one verifier, self-consistency, fixed debate, and fixed Council panels.

Baseline implementations and configurations must be pinned and reported. A
candidate method should not be promoted merely because it outperforms a weak or
under-tuned baseline.

### 6.2 Primary metrics

| Capability | Candidate metrics |
|---|---|
| Entity and lineage resolution | precision, recall, F1, temporal consistency |
| Impact retrieval | precision@k, recall@k, MAP, nDCG, decoy rejection |
| Impact explanation | valid evidence path rate, evidence coverage, unsupported-hop rate |
| Causal assessment | intervention-effect ranking, false causal attribution, alternative-explanation coverage |
| Calibration | Brier score, ECE, negative log-likelihood, risk-coverage, AURC |
| Documentation | unsupported claim rate, stale-claim detection, witness replay success |
| Context compilation | token reduction, evidence recall, contradiction retention, answer quality |
| Adaptive acquisition | accuracy-cost frontier, information gain, latency, privacy exposure |
| Council | calibration delta, critical-dissent recall, seat correlation, effective panel size |
| Privacy | membership/relationship leakage, matching utility, computation and communication cost |
| Operations | wall time, memory, incremental update cost, artifact size |

Accuracy, latency, token cost, compute cost, and privacy exposure must be
reported together rather than optimized in isolation.

### 6.3 Mandatory ablations

- remove valid time or recorded time;
- replace symbolic worlds with one flattened graph;
- remove runtime evidence;
- remove counter-evidence and contradiction states;
- replace decomposed uncertainty with one confidence score;
- remove proof obligations;
- replace compiled context with top-k retrieval;
- replace adaptive acquisition with a fixed pipeline;
- replace independent Council evidence channels with identical context;
- vary Council size while controlling the total inference budget;
- remove ontology governance and competency-question replay;
- compare centralized and private federation where feasible.

### 6.4 Statistical protocol

- pre-register primary hypotheses and metrics before opening the sealed test set;
- publish confidence intervals and effect sizes, not only point estimates;
- use paired tests where systems evaluate the same benchmark cases;
- repeat stochastic methods across declared seeds;
- report per-project distributions to avoid domination by large repositories;
- correct for multiple comparisons in broad ablation studies;
- retain negative and inconclusive findings.

## 7. Phased work packages

Dates and staffing are intentionally not committed yet. Advancement depends on
the exit criteria of the preceding package.

### WP0 — Foundations and benchmark governance

**Status:** planned

- formalize terminology, assertion semantics, evidence classes, and threat
  model;
- define dataset governance, license policy, annotation protocol, and leakage
  controls;
- create a small benchmark pilot and deterministic baselines.

**Exit criterion:** independently reviewable labels and reproducible baseline
runs on a pilot corpus.

### WP1 — Temporal epistemic ledger

**Status:** planned, experimental

- prototype bitemporal assertions, symbolic world conditions, provenance, and
  paraconsistent states;
- evaluate incremental maintenance and query performance;
- compare flattened, snapshot-per-world, and symbolic representations.

**Exit criterion:** semantics pass adversarial examples and outperform at least
one materialized baseline on a declared storage/query trade-off.

### WP2 — Static-runtime and behavioural twin

**Status:** planned, experimental

- ingest intended, implemented, configured, observed, and experienced evidence;
- measure mismatch detection;
- infer and falsify candidate behavioural contracts.

**Exit criterion:** improved detection of held-out mismatches without treating
missing observations as negative evidence.

### WP3 — Causal impact and TemporalCrossRepoImpactBench

**Status:** planned, experimental

- release the first benchmark version;
- implement reachability and contract baselines;
- evaluate interventions, replay, and causal models.

**Exit criterion:** demonstrate whether causal evidence adds measurable value
beyond reachability, including negative or inconclusive results.

### WP4 — Proof-carrying knowledge and context compilation

**Status:** planned, experimental

- define witness contracts and invalidation semantics;
- implement competing context-compilation strategies;
- evaluate unsupported claims, witness replay, and context efficiency.

**Exit criterion:** reduce context while preserving evidence completeness and
improve stale or unsupported claim detection.

### WP5 — Adaptive acquisition, ontology evolution, and Council

**Status:** planned, experimental

- compare expected-value policies with fixed pipelines;
- measure correlated reviewer errors and sequential stopping;
- run ontology proposals through benchmark replay and human governance.

**Exit criterion:** a calibrated accuracy-cost improvement that survives
ablation against a single strong verifier and fixed Council baselines.

### WP6 — Private and compositional federation

**Status:** moonshot feasibility study

- prototype semantic capsules and one privacy-preserving matching method;
- formalize a small sheaf-inspired consistency model;
- evaluate diagnostic utility and leakage.

**Exit criterion:** evidence that the method exposes less protected information
than centralized matching while retaining useful impact recall.

### WP7 — Integrated replication and product-transfer assessment

**Status:** planned only if earlier packages succeed

- replicate results across languages and ecosystems;
- run independent artifact evaluation;
- assess which experimental components merit a production architecture
  decision.

**Exit criterion:** reproducible integrated results and an explicit decision for
each component: reject, continue as research, or propose for productization.

## 8. Reproducibility and research operations

Every reported experiment should preserve:

- repository URLs, commit hashes, submodule state, and license metadata;
- dataset and annotation versions;
- extractor, compiler, ontology, rule, and schema versions;
- environment and hardware manifests;
- container or Nix/Guix-style reproducible environment definitions;
- commands, configuration, random seeds, and raw results;
- model provider, exact model identifier, parameters, prompt templates, and
  token usage where licensing permits;
- evidence packs and Council seat outputs before aggregation;
- statistical analysis notebooks or scripts;
- failure logs, exclusions, and deviations from the pre-registered protocol.

Artifacts should be content-addressed and accompanied by a machine-readable
manifest. Benchmark labels require reviewer provenance and disagreement
records. Publications should include an artifact-availability statement and a
replication guide.

LLM-dependent experiments must also report model drift risk. Results obtained
from an unavailable or mutable model endpoint are not considered independently
reproducible unless all inputs and outputs required for replay can legally be
released.

## 9. Threats, safety, and ethics

- Repository history may contain credentials or personal information; dataset
  construction requires secret scanning and minimization.
- Executing builds, tests, or traffic replay from untrusted repositories
  requires an isolated, network-denied sandbox.
- Runtime telemetry may reveal user or tenant data; collection and retention
  require purpose limitation and explicit governance.
- Open-source histories can appear public while still imposing license and
  attribution obligations.
- LLM benchmarks risk pretraining contamination and provider-specific drift.
- Causal language can create false operational confidence; unsupported causal
  conclusions must be distinguishable from reachability and correlation.
- Private federation must be attacked for membership and relationship leakage,
  not only evaluated for utility.

## 10. Key sources and candidate baselines

### Repository understanding and documentation

- [RepoGraph: Enhancing AI Software Engineering with Repository-level Code Graph](https://openreview.net/forum?id=dw9VUsSHGB)
- [RepoDoc](https://arxiv.org/abs/2604.26523)
- [ReCUBE](https://arxiv.org/abs/2603.25770)
- [Code Digital Twin](https://arxiv.org/abs/2503.07967)
- [Graphify](https://github.com/safishamsi/graphify)
- [LLMWiki](https://github.com/lucasastorian/llmwiki)
- [SCIP](https://github.com/sourcegraph/scip)

### Provenance, uncertainty, and knowledge representation

- [W3C PROV-O](https://www.w3.org/TR/prov-o/)
- [W3C SHACL](https://www.w3.org/TR/shacl/)
- [Provenance Semirings](https://arxiv.org/abs/2108.07758)
- [Relation-aware uncertainty for probabilistic knowledge graphs](https://arxiv.org/abs/2512.22318)
- [Knowledge Sheaves](https://arxiv.org/abs/2110.03789)

### Causal, runtime, retrieval, and ontology directions

- [Causal Software Engineering](https://arxiv.org/abs/2605.02454)
- [Bomfather: runtime software dependency observation with eBPF](https://arxiv.org/abs/2503.02097)
- [GFM-Retriever](https://arxiv.org/abs/2603.07179)
- [HyDRA](https://arxiv.org/abs/2507.15917)

### Multi-agent review and privacy

- [Can LLM Agents Really Debate?](https://arxiv.org/abs/2511.07784)
- [Council of High Intelligence](https://github.com/0xNyk/council-of-high-intelligence)
- [PR-AF](https://github.com/Agent-Field/pr-af)
- [Subgraph reconstruction attacks against GraphRAG](https://arxiv.org/abs/2602.06495)

These sources define candidate foundations and baselines. Inclusion does not
mean that their findings have been reproduced by CodeNoesis, and all sources
must be re-reviewed before an experiment or publication relies on them.
