# Benchmark contracts

This directory defines the reproducibility contract for CodeNoesis performance
evidence. Issue #205 proposes the first active observational suite over pinned
real-world Rust repositories. The candidate remains ineffective until the
exact independently reviewed pull request is manually merged.

`manifest.json` selects exactly one B1 suite with a versioned descriptor,
policy, semantic oracle, deterministic runner, raw samples, and same-host
comparison. Existing CI validates the committed contract and compiles Rust
benchmark targets; it does not clone or execute external repositories.

Every future benchmark report must identify:

- corpus version and host configuration;
- concurrency and cache state;
- enabled extractors and repetitions;
- percentile calculation method and success rate.

Generated reports belong under `benchmarks/results/` and are ignored by Git.
Release evidence must be stored as an immutable CI artifact with the exact
source revision and corpus identity.

Issue #184 adds one dependency-free observational G7a runner for the
project-owned two-generation G1a bundle fixture. It records raw warm
single-threaded samples, nearest-rank p50/p95/p99, success rate, corpus,
host/toolchain, cache and revision metadata. It does not activate this global
manifest, establish a regression threshold, resolve `NFR-PER-002` or
`OD-SLO-001`, or make an SLO, release-artifact, cross-host or GA claim.

An active observational suite must:

1. add a versioned, licensed corpus or an immutable corpus descriptor;
2. add an executable suite with a stable command and reviewed metrics;
3. add base-versus-head comparison with a reviewed regression threshold;
4. retain raw samples rather than only aggregate values;
5. link every claim to `NFR-PER-001`; `NFR-PER-002` remains separate until a
   reference corpus and SLO are ratified.

The `rust-real-world-stability-v1` candidate accepts caller-supplied local full
clones only. It performs no clone, fetch, checkout, target build or execution,
submodule initialization, LFS action, network access, model call, browser
launch, or source mutation. Generated reports stay under ignored
`benchmarks/results/`; retained review evidence contains only bounded public
identities, digests, samples, and sanitized host metadata.

Issue #206 and Decision 0043 add the explicit operational selector
`real-world-rust-benchmark-75s-v1` to this suite. It raises only the exact
packed, source-only R16 B1 whole-scan maximum from 60 to 75 seconds. The runner
timeout remains 90 seconds, the observational Lekton p95 ceiling remains 75
seconds, and every selector-absent or acquisition limit remains unchanged. The
selector is report configuration, not an extractor, ontology input, SLO, or
release/support claim.

Issue #207 and Decision 0044 define the honest B1 bootstrap baseline as the
first product commit supporting that selector, `cce8486`. They also bind the
Lekton semantic oracle to the corpus repository identity instead of the older
pilot identity. All counts, source revisions, policies, limits, timeouts, and
observational exclusions remain unchanged.
