# CodeNoesis Documentation

CodeNoesis is being developed along two related but independently governed
tracks.

## Software track

The [software documentation](software/README.md) describes the production
system to be built: the Rust architecture, runtime boundaries, data contracts,
repository-analysis pipeline, public interfaces, security model, operations,
and delivery roadmap.

The [Software Requirements Specification](software/software-requirements-specification.md)
turns that design into traceable requirements, acceptance evidence, TDD gates,
and incremental vertical slices.

Software decisions must optimize for correctness, maintainability, performance,
and safe operation on untrusted repositories. Experimental techniques enter
the production path only after they meet explicit evaluation and reliability
gates.

## Research track

The [research documentation](research/README.md) develops CodeNoesis as an
epistemic, temporal, and causal software digital twin. It defines the research
questions, experimental modules, benchmark strategy, evaluation metrics, and
reproducibility requirements.

Research components may explore techniques that are not yet appropriate for
production. Their outputs remain explicitly typed as hypotheses, predictions,
or reviewed inferences until deterministic evidence or human review confirms
them.

## Status vocabulary

Documentation in both tracks uses the following status vocabulary:

- **Planned**: accepted as part of the intended design but not implemented.
- **Experimental**: implemented or proposed for research evaluation, without a
  production reliability commitment.
- **Implemented**: present in the repository and verifiable through code or
  tests.

At project inception, all described capabilities are **planned** or
**experimental**. No production implementation exists yet.
