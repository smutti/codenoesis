# Local readiness benchmark package v1

This S14 evaluation package implements the user-authorized first Local GA
preparation package against main `399ee12b31a5d0c4fb1b3a59e7badb012e81a0d2`.
It maps to observational NFR-PER-001 and prepares evidence for NFR-PER-002,
FR-REL-001 and the existing G0–G9 Local GA gate. It does not ratify an SLO,
approve a new language capability, distribute a release, or claim Local GA.

Acceptance:

1. One recorded release binary analyzes all 20 pinned public entries: the
   existing 16 Rust cases and four Java/Kotlin cases. Three planned attempts
   use fresh stores, sequential execution and unchanged product limits.
   Every failure remains in the denominator and raw evidence. Historical
   progressive/B1 oracles remain intact; current complete profiles have a
   distinct versioned observation protocol.
2. A source-authored, reviewable ontology oracle and executable scorer show
   expected/extracted facts, missing/extra facts, relationship endpoints and
   source evidence. Oracle review status and corpus exposure are explicit.
   Agent-authored expectations cannot certify their own independent accuracy.
3. Local GA criteria identify measurable evidence and outstanding gates.
4. An obsolescence audit traces code, scripts, dependencies and compatibility
   readers. Only demonstrated unused or redundant code is removed.

No compiler execution, dependency resolution, new dependency, product ontology
change, network analysis, migration, signing, release or server implementation
is part of this package. Benchmark and oracle data require semantic review in
the pull request; passing infrastructure tests cannot supply that review.
