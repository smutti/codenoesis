# Decision 0050: Rust v0.1 validation corrections

- Status: Branch candidate; effective after maintainer merge
- Date: 2026-09-06
- Base: `166177ab175abeef70a2eb7e4ee74a925d06dff8`
- Requirements: `FR-ACQ-002`, `FR-INV-001`, `DR-EVD-001`, `NFR-SEC-001`,
  `FR-EXT-016`, `FR-EXT-017`, `FR-EXT-020`, `NFR-PER-001`

## Observable outcome

Close reproducible implementation defects found while validating the bounded
Rust v0.1 profile. Preserve the 34-profile historical verification pack and
the frozen public benchmark oracle. Candidate measurements describe the
candidate binary, including failures, independently of historical acceptance.
This decision grants no general Rust, release, support, SLO, or GA claim.

## Expression and validation corrections

R4 manifest source mapping retains multiline nested arrays and strings inside
uninterpreted `package.metadata` values. A continuation line beginning with `[`
does not become a new table while the validated TOML value remains incomplete.
The metadata still emits its existing diagnostic and coverage gap; dependency
tables following it retain their exact source evidence.

An R14 `for` iterator whose node kind is outside the selected expression profile
uses the existing `rust.pattern_input_unexpanded` diagnostic and coverage gap.
Its exact evidence and loop owner remain visible. Supported inner expressions
and calls are retained; no missing iterator identity or pattern binding is
invented. This matches the existing handling of unsupported inputs in other
control constructs and removes a reproducible WGPU contract failure.
An omitted binding also forms a lexical resolution barrier in its scope:
references must not fall through to an outer namesake. Supported inner bindings
and references outside the affected scope retain their ordinary resolution.

R15 validation constructs borrowed relationship and access indexes once per
validation rather than once per derivation. The same provenance, endpoint,
ownership, scope, and access checks apply. No limit, hash, ordering, or ontology
semantics change. The product scan deadline remains 75,000 ms under the explicit
benchmark execution profile. Whole-process time also includes persistence and
output after the confined scan; it is reported separately.

## Internal Git symlink profile

The explicit acquisition selector
`local-git-sha1-packed-internal-symlinks-v1` is accepted only with the complete
R16 scan selector matrix. It inherits packed SHA-1 acquisition and the 8 MiB
regular-file cap. Existing acquisition selectors continue to reject symlinks.
Older snapshots, refresh, and trusted-source commands do not accept the new
selector.

Committed mode `120000` entries are separate inventory metadata, never regular
source files. Resolution uses only the acquired immutable Git tree and verified
objects. It never reads a host symlink or materializes a target, copies a
directory, invokes Git/Cargo, or fetches a dependency. Regular targets are
inventoried once at their physical committed paths.

Each link retains its path, link blob OID, exact target byte length, resolved
physical target path, target object OID, and file/directory kind. Targets must
be 1–1,024 bytes of relative UTF-8, without absolute/drive paths, backslashes,
control characters, or empty components. A single trailing slash requires a
directory. `.` and `..` are interpreted during traversal, after resolving
preceding directory aliases; they may not escape the committed root. The
resolved target must be a non-root regular file or directory in that same tree.
Resolution is bounded to 32 link expansions, 32 resolved path components,
1,024 path bytes, and 255 bytes per component. Tree-entry, cumulative-byte, and
acquisition-time limits still apply; link blobs contribute to the byte budget.
Packed-object size rejection reports the selected limit (including the 8 MiB
regular-file profile and 1,024-byte link target cap), with bounded observed
size. The admission predicate and cumulative-byte limit are unchanged.

Cycles, dangling links, root aliases, Gitlink crossings, unsafe targets, and
unsupported forms fail closed with a typed acquisition error. Oversized target
blobs fail before capture. No partial snapshot is published.

R16 projects link-blob evidence under the versioned
`codenoesis.git-internal-symlink/v1` metadata contract, together with a diagnostic
and explicit unsupported coverage for alias-based extraction. These records
survive portable export and validation. The profile does **not** implement Rust
module alias resolution or Cargo source-directory aliases. An affected workspace
may therefore still return a typed workspace/extraction rejection. Accepting a
link as repository metadata is not evidence that declarations reachable through
that alias have been extracted.

## Benchmark interpretation

Strict public evaluation retains its exact historical product SHA, stage
commands, oracle, and timeout. Explicit `--candidate-observation` uses a distinct
report contract and labels progress or semantic/error changes for review. It
does not rebaseline, certify the candidate, or count a typed rejection as an
extraction success. Three terminal samples must have deterministic identities;
internal failures, malformed output, and regressions before historical success
stop the observation. An explicit candidate outer timeout is recorded and does
not alter the product's 75-second scan deadline.

B1 creates its temporary store below a canonical path, avoiding the macOS
`/var` alias. Its redacted error classifier recognizes only the exact existing
V4 storage unsafe-path diagnostic in addition to its established identities.

Structural counts, stable hashes, evidence resolution, and emitted-fact ratios
are useful regression metrics. They are not source-level precision or recall.
A reviewed, independently annotated source sample is required before reporting
an accuracy score; observations do not become an acceptance oracle implicitly.

## Verification

Focused regressions exercise unsupported loop inputs; R15 provenance/access
rejection; immutable link evidence and unchanged regular-file counts; host
canaries; directory aliases and parent traversal; cycles, dangling and unsafe
targets; exact target/expansion boundaries; Gitlink crossing; graph metadata
tampering; CLI composition; and benchmark protocol failures. The final PR must
link the complete technical gate and three-repeat candidate corpus results.
