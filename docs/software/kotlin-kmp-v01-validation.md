# Kotlin/KMP v0.1 validation

The bounded S8 candidate was measured on clean source commit
`ce473e0197f02e737f52533e4a1920fdd5712742`, with the pinned Rust toolchain and a
release build. Binary SHA-256:
`4f11a16b6e097645c5ef4b449899ad2194f657fd2716c2904865571ec14ff298`.
The [machine-readable report](evidence/kotlin-kmp-v01/benchmark.json) retains
all six attempts, exact repository pins, artifact hashes and environment.

| Public sample | Successful runs | Kotlin files / parser gaps | Declarations | Entities / relationships | Median whole-process time | Range |
|---|---:|---:|---:|---:|---:|---:|
| Kotlin/kmp-basic-sample | 3/3 | 10 / 0 | 23 | 94 / 93 | 0.101 s | 0.095–0.505 s |
| JetBrains/compose-multiplatform-template | 3/3 | 7 / 0 | 13 | 70 / 65 | 0.114 s | 0.113–0.115 s |

Both samples are Apache-2.0. The JetBrains template is archived. Semantic hashes
are identical across all three repetitions of each repository. Every sample
retains the two global gaps: effective Gradle/source-set configuration and
compiler/type/call/inheritance/runtime/generated-source semantics.

These are small public integration pilots, not a scalability or accuracy
benchmark. No cache clearing was attempted; the first basic-sample run is
retained. Whole-process time includes persistence and stdout. Peak memory,
compiler agreement, public-corpus precision/recall and production SLOs are
not measured. No target Gradle script, compiler or wrapper was executed.

## Ontology acceptance oracle

The [hand-authored fixture oracle](../../tests/specifications/s8/kotlin-kmp-declarations-v1.json)
checks the exact multiset of 11 source declarations. All expected declarations
are present, with no additional declarations: precision and recall are both
11/11 **for this fixture and these fields**. Two expected candidate links are
retained; cross-module decoys and overloaded names remain unresolved. This
does not estimate public-repository accuracy.

Focused checks cover nested type members, constructor properties, Unicode byte
spans, annotations, malformed and oversized input, unsupported parser syntax,
claim/evidence references, candidate-state promotion, immutable commits despite
dirty source files, typed selector errors, store publication, search, export,
offline viewer generation, modified portable input, unowned output preservation
and symlink rejection. The common S7 client adapter remains separate.

`noesis docs`, `export` and `explore` also completed on the stored basic sample.
The viewer's script/style CSP hashes and absence of dynamic HTML/network APIs
are checked automatically. Interactive browser verification was **not run**:
the browser security policy rejected the local `file://` URL. No alternate
browser surface or local server was used to bypass that restriction.

## Final performance correction check

Source commit `84745de4856e25502a4aa7da4d85076fb4990887` indexes candidate
names and source paths once, caches repeated evidence spans and accumulates
claim evidence before serialization. It removes repeated whole-workspace scans
and growing-array copies without changing the public ontology result.

The synthetic domain regression covers 18,000 declarations, 12,000 candidate
links and a duplicate actual that must affect only its own source set. It
completed in 0.04 s locally; this is a focused test observation, not an SLO.
The path-membership regression checks root, nested and misleading path segments.

All six final public scans succeeded with **exactly the same semantic hashes**
as the initial observations. The intermediate candidate-index measurements are
also retained; no attempt was discarded. See
[final observations](evidence/kotlin-kmp-v01/benchmark-84745de.json) and
[intermediate observations](evidence/kotlin-kmp-v01/benchmark-d3b30b3.json).

Final release binary SHA-256: `775737a81516083965c29584d2a8c0eb8bbcca3e5dbb10473c96e6e4ca2947cc`.
Final medians: basic `0.098` s; Compose template `0.118` s. Counts, source
coverage and limitations in the first table are unchanged.

## Windows fixture correction

The first [Windows CI execution](https://github.com/smutti/codenoesis/actions/runs/34102827743/job/101680960236)
on `7fabebf1260de7b32b97cc00b79969187ae1666d` failed at Kotlin export with
`artifact.invalid_or_unsafe_kotlin_projection`. The new fixture canonicalized
its temporary root to a Windows verbatim path, outside the inherited output
path profile. Existing Rust export fixtures already use a validated Windows
authority helper to avoid that unsupported spelling and temporary-root aliases.

The Kotlin fixture now reuses that helper. The ontology oracle and every
success assertion are unchanged; a Windows-only negative test also requires
verbatim output to fail with the typed error before destination creation.
The fixture correction does not change extraction or the ontology oracle. The
original failed run remains evidence; final-head CI must independently pass
instead of retrying the failed head as acceptable evidence.

The Kotlin HTML publisher also now uses the existing Rust viewer's checkout-text
normalizer. A regression renders the real asset from both LF and CRLF bytes and
requires identical output; the end-to-end viewer check requires LF output on
every platform. This preserves HTML and manifest byte identity across checkout
line endings without changing extracted ontology semantics.

## Reproduction

Use full Git clones at the two immutable pins in
`tests/specifications/s8/kotlin-kmp-public-v1.json`, in directories named
`basic` and `compose-template`. Shallow repositories remain outside the inherited
acquisition profile.

```sh
cargo build --release -p noesis --locked
python3 scripts/run_kotlin_kmp_benchmark.py \
  --binary target/release/noesis \
  --corpus /absolute/path/to/corpus \
  --output /absolute/path/to/new-evidence-directory
```

Run the complete repository gate from `AGENTS.md` on the final review head;
its results and logs belong to the PR evidence, independently of this source
commit's benchmark observations. Rust's frozen benchmark oracle is unchanged.
