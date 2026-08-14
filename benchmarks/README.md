# Benchmark scaffold

This directory defines the reproducibility contract for CodeNoesis performance
evidence. It does **not** contain an executable benchmark or make a performance
claim yet.

`manifest.json` remains in `scaffold` state until an implementation slice adds
at least one reviewed suite, a versioned corpus, and a deterministic runner. CI
validates the manifest and compiles every Rust benchmark target that exists; an
empty suite list is reported explicitly rather than treated as measured
performance.

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

Before changing `status` to `active`:

1. add a versioned, licensed corpus or an immutable corpus descriptor;
2. add an executable suite with a stable command and reviewed metrics;
3. add base-versus-head comparison with a ratified regression threshold;
4. retain raw samples rather than only aggregate values;
5. link the suite and CI evidence to `NFR-PER-001` or `NFR-PER-002`.
