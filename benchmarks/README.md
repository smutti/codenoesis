# Benchmark contracts

This directory defines the reproducibility contract for CodeNoesis performance
and real-repository compatibility evidence. `manifest.json` selects two
observational Rust suites with versioned descriptors, policies, semantic
oracles, deterministic runners, raw samples, and same-host constraints:

- `rust-real-world-stability-v1` measures the pinned Lekton/RustDesk pilot;
- `rust-public-conference-v1` evaluates progressive extraction and ontology
  information over eight additional pinned public repositories.

Existing CI validates the committed contracts and compiles Rust benchmark
targets; it does not clone, fetch, or execute external repositories.

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

Issue #208 and Decision 0045 retain bounded failed-sample identity before owned
temporary cleanup: public entry/index/exit/stream lengths, exact stderr SHA-256,
and only allowlisted schema/code/stage from one canonical product error up to
2,048 bytes. Product message/context and source or private values are never
echoed. The runner V1 error protocol, report schema, no-retry policy, product,
corpus, oracle, threshold, timeout, and observational exclusions stay unchanged.

Issue #209 and Decision 0046 add one closed validation category when the B1c
typed product identity is opaque, correct historical V2/V3 descendant
verification, and permit one semantics-neutral private R16 parser extraction
to restore the complete gate. The exact baseline binary stays frozen; candidate
behavior, not binary identity, must match. One further Lekton diagnostic is
allowed without retry, acceptance, comparison, or raw stderr retention.

## Public Rust conference corpus

`public-rust-conference-v1` uses purposeful maximum-variation sampling rather
than popularity as a proxy for compatibility. It covers command-line tools,
libraries, virtual workspaces, cross-platform systems code, terminal rendering,
compiler-adjacent code, a UI framework, and a graphics stack:

| Repository | Archetype | Rust files | Rust bytes | Current terminal result |
|---|---:|---:|---:|---|
| [hyperfine](https://github.com/sharkdp/hyperfine) | CLI | 41 | 198,410 | R16 success |
| [tower](https://github.com/tower-rs/tower) | Async middleware workspace | 133 | 487,346 | R16 success |
| [mio](https://github.com/tokio-rs/mio) | Systems I/O library | 82 | 681,387 | R16 success |
| [fd](https://github.com/sharkdp/fd) | Filesystem CLI | 24 | 261,335 | Typed manifest rejection |
| [delta](https://github.com/dandavison/delta) | Terminal renderer | 82 | 1,042,590 | Typed semantic rejection |
| [rustfmt](https://github.com/rust-lang/rustfmt) | Compiler-adjacent formatter | 2,008 | 2,799,610 | Typed flow rejection |
| [dioxus](https://github.com/DioxusLabs/dioxus) | UI framework workspace | 878 | 6,724,384 | Typed acquisition rejection |
| [wgpu](https://github.com/gfx-rs/wgpu) | Graphics workspace | 849 | 14,349,544 | Typed source-extraction rejection after workspace package inheritance |

The exact commit, tree, observed license, source metrics, and repository ID are
frozen in `corpora/public-rust-conference-v1.json`. The suite accepts only full,
non-shallow local clones whose `HEAD`, tree, and aggregate source metrics match
that descriptor. It performs no network operation, checkout, build, macro
expansion, target execution, dependency resolution, or model call.

Place the eight clones in directories named after their corpus IDs, build the
release CLI, and run:

```sh
cargo build -p noesis --release
python3 scripts/run_public_rust_evaluation.py run \
  --manifest benchmarks/manifest.json \
  --suite rust-public-conference-v1 \
  --corpus benchmarks/corpora/public-rust-conference-v1.json \
  --policy benchmarks/policies/public-rust-conference-v1.json \
  --oracle benchmarks/baselines/public-rust-conference-v1.json \
  --binary target/release/noesis \
  --repository-root /path/to/public-rust-corpus-v1 \
  --output benchmarks/results/public-rust-conference-v1-local.json \
  --host-profile local-machine-v1 \
  --product-commit 095518ec49ede920af90b5c408c58a7ab99fc754
```

Each repository advances through acquisition, workspace, manifest, semantic,
framework, flow, and constant stages until its frozen success or typed-rejection
oracle. Three terminal samples with a fresh product store are retained without
retry; operating-system filesystem caches are not reset. The report
contains raw wall time and stream sizes, exact semantic identities, graph
counts, callable/signature/parameter coverage, enum and value coverage, calls,
expressions, bindings, basic blocks, evidence coverage, diagnostics, and
aggregate stage coverage.

The initial same-host macOS arm64 observation matched all eight oracles. Three
repositories completed R16 and five failed closed at distinct compatibility
boundaries. The three complete graphs contain 5,228 entities, 8,862
relationships, 14,090 claims, 4,458 evidence records, 102 callable signatures,
176 parameters, 60 enum variants, 74 declared values, 5 safely evaluated
values, 561 call sites, and 19 uniquely resolved local calls. These are
observational corpus results, not an SLO, supported-repository list, release,
cross-host comparison, or conference-validity claim.
