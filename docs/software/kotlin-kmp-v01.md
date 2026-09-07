# Kotlin/KMP v0.1 — static declaration profile

Status: implementation candidate for the first P1/S8 vertical; effective after
maintainer merge. Requirement: FR-EXT-025. Rust profiles remain frozen.

## Observable outcome

`noesis scan --profile standard-local-s8 --kotlin-profile kotlin-kmp-declarations-v1`
acquires one immutable local Git commit, extracts a bounded Kotlin declaration
graph, and publishes a v19 snapshot through the existing local store. The same
explicit Kotlin selector supports query, export and offline exploration.

The ontology describes committed syntax, never effective Gradle configuration
or compiler-confirmed types. It includes modules with committed Gradle build
files, source sets observed under `src/<name>/kotlin`, source files, packages,
classes, interfaces, objects, functions, properties, type aliases and imports.
Module and source-set membership is explicitly `path_convention`; a module is
not asserted to be an active Gradle project. Build and settings files are
inventoried as Gradle declarations and carry a configuration-not-evaluated gap.
This deliberately supports conventional roots even when settings use plugins
or dynamically compute includes. Custom roots remain unassigned with a gap.

Every source declaration retains repository identity, immutable revision, blob
OID, byte span, line span and an excerpt digest. Nested type members are retained;
function-local declarations and initializer bodies are outside this profile.
Primary constructor `val`/`var` parameters are properties. Signature text is
declared syntax, not a resolved type. No calls, inheritance resolution, overload
resolution, Java extraction, generated source discovery or runtime claims.

`EXPECT_ACTUAL_CANDIDATE` relates syntactically compatible, uniquely named
top-level expect and actual declarations in the same observed module and package,
from distinct source sets. It is an Unknown candidate with both source evidence
items, not an actualization fact. Function matching additionally requires exact
extracted signature text (including whitespace inside parameter nodes). Overloads, actual type aliases, nested scopes,
missing or ambiguous candidates produce explicit gaps. Source-set visibility and
Gradle dependsOn graphs are not inferred.

## Failures and bounds

Malformed Kotlin produces a per-file syntax gap and no declarations from that
file. Invalid UTF-8, parser resource limits and graph/output capacity violations
are typed failures before publication. No Kotlin files is a typed unsupported
input. Existing packed-SHA1 acquisition and filesystem/security restrictions
apply; no Gradle, compiler, wrapper, child process or network operation is run.
Limits: 4 MiB/file, 256 MiB acquired bytes, 20,000 acquired files, 250,000 syntax
nodes/file, depth 128, 100,000 graph entities, 32 MiB canonical snapshot, 60 s
scan deadline. Viewer input is bounded and rendered as text under a restrictive
CSP; no server, external resources or automatic browser launch.

## Acceptance and benchmark

Hand-authored fixtures check cross-source-set declarations, constructor
properties, Unicode evidence, false friends in comments/strings, overloads,
cross-module decoys, malformed files and unsupported custom roots. The public
journey checks scan/store/query/export/viewer, deterministic semantic hashes,
integrity failures, bounded errors and preservation of historical Rust profiles.

Two public, license-identified Kotlin/KMP repositories will be pinned before
measurement and scanned three times each. Report extraction success, elapsed
time, source/declaration/relationship/gap counts and semantic-hash stability.
Fixture precision and recall use a hand-authored source oracle; public corpus
counts measure coverage and repeatability, not semantic accuracy. No baseline is
regenerated to hide a failure. Java is the next separate vertical.
