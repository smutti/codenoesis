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
