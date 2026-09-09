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

## Reproduction

Build with the pinned toolchain from a clean worktree and retain the build log.
The JSON build record is an operator assertion bound to the binary digest, not
signed provenance. The following record is created after a successful build:

```sh
cargo build -p noesis --release --locked
python3 - <<'PY'
import hashlib, json, platform, subprocess
from pathlib import Path
git = lambda *a: subprocess.check_output(['git', *a], text=True).strip()
if git('status', '--porcelain'):
    raise SystemExit('build source must be clean')
record = {
    'schema_version': 'codenoesis.benchmark-build/v1',
    'product_commit': git('rev-parse', 'HEAD'),
    'product_tree': git('rev-parse', 'HEAD^{tree}'),
    'source_dirty': False,
    'binary_sha256': hashlib.sha256(Path('target/release/noesis').read_bytes()).hexdigest(),
    'command': ['cargo', 'build', '-p', 'noesis', '--release', '--locked'],
    'toolchain': subprocess.check_output(['rustc', '-Vv'], text=True),
    'os': platform.platform(), 'architecture': platform.machine(),
}
Path('benchmarks/results/build.json').write_text(json.dumps(record, indent=2) + '\n')
PY
python3 scripts/run_local_readiness_benchmark.py \
  --binary target/release/noesis --build-record benchmarks/results/build.json \
  --repositories /path/to/repository-map.json \
  --output benchmarks/results/local-readiness-new --host-profile my-host-profile
python3 scripts/compare_local_readiness.py \
  --report benchmarks/results/local-readiness-new/report.json \
  --output benchmarks/results/local-readiness-comparison.json
python3 scripts/score_ontology_quality.py \
  --observation benchmarks/results/local-readiness-new \
  --repositories /path/to/repository-map.json \
  --output benchmarks/results/ontology-quality-new
```

The repository map contains all 20 IDs from
`benchmarks/corpora/local-readiness-v1.json`, each mapped to an existing full,
clean local clone. Exact commit/tree, SHA-1 format and non-shallow state are
verified before any timed sample. No target checkout, build or execution runs.
The example uses the native Unix binary filename; on Windows use `noesis.exe`.

Every output path must be new. Raw stdout/stderr, hashes, exact commands, three
planned attempts, fresh product stores, duration and failures are retained.
The runner checks the binary digest before each sample and after the run.
Protocol failures/timeouts are never retried: remaining slots stay unattempted
in the fixed denominator. Typed rejections are failed extractions. JVM graph
counts are generic; the legacy `information` counters describe Rust capabilities
only. The host is non-exclusive and OS caches are uncontrolled.

Observation exit zero means complete deterministic observations requiring
candidate review. Comparison also requires all three identities to match
`benchmarks/baselines/local-readiness-v1.candidate.json`; changes exit 2 and do
not rewrite it. The exploratory extraction reference is 17/20 (85%), including
three explicit boundary cases. Advertised supported scope requires 100% success
under the [Local GA candidate criteria](local-ga-candidate-criteria.md).
No latency acceptance threshold is derived from these observations.

## Source oracle interpretation

`benchmarks/oracles/ontology-quality-v1.json` is source-authored but agent-
authored and development-exposed: `candidate_pending_independent_review`.
The scorer produces `report.json` and script-free `review.html`, with source
anchors, expected/extracted facts, missing/extra facts, relationship endpoints
and evidence intervals. Duplicate facts count as extras; wrong selected
properties/endpoints count as a missing and an extra fact. Source SHA-256 and
Git blob identities bind evidence to immutable bytes. Evidence covering an
anchor and exact-anchor evidence are separate metrics. Missing extractions
retain all expected facts as false negatives.

The scope is exhaustive for the selected kinds in named files, with a public
Rust surface filter. It covers direct declaration identity, selected syntax
properties and the induced relationships among selected facts. It does not
score unselected relation families or relationships to entities outside those
files. The Java seed has no overloaded names; overload accuracy remains covered
only by the adapter fixture until added to a larger quality corpus.

Precision/recall here are **candidate expectation agreement**, not independently
certified accuracy. Empty denominators stay null. Tests reject self-certifying
review labels. Independent source review, repository-disjoint holdout sampling,
compiler comparisons and task-quality evaluation remain open; a future reviewed
protocol must explicitly version its sampling, authority and thresholds.

The [obsolescence audit](code-obsolescence-audit-2026-09-09.md) records the unused
helper removal, retained public API candidates and historical reproduction paths.
