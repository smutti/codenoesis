# Java v0.1 — static declarations

FR-EXT-026 / S8 candidate, effective after maintainer merge. This is a bounded
syntax profile, not general Java or compiler support.

## Outcome and contract

`noesis scan --profile standard-local-s8 --java-profile java-declarations-v1`
acquires an immutable local Git commit, extracts Java declarations and publishes
a `codenoesis.repository-snapshot/v20` through the existing local store. The
same explicit Java selector supports query, docs, export and offline explore.
The existing Rust and Kotlin contracts retain their namespaces and behavior.

The Java v1 ontology includes source files, packages, imports, classes,
interfaces, enums, records, annotation types, methods, explicit constructors,
fields, enum constants, record components and annotation elements. Named nested
types and their members retain explicit structural containment. Overloads are
separate source occurrences. A record component is not an inferred field or
accessor; implicit constructors and generated members are not emitted.

All relationships describe observed syntax or path conventions. Claim state is
`deterministic_fact`, with repository identity, commit, blob OID, exact byte/line
spans and excerpt digest. Imports and signature types remain unresolved.
Callable/type signatures retain the written header; field signatures retain
declared type, name and array dimensions, without initializers. Supertypes in
headers do not establish inheritance resolution or subtype facts.

Directories containing `pom.xml`, `build.gradle` or `build.gradle.kts` are
observed build modules. `src/<name>/java` below those directories supplies
conventional source-set membership; the nearest matching module wins. All build
files remain unevaluated metadata. No effective Maven model, Gradle graph,
classpath, custom source-root discovery, annotation processor or target build
is executed. Nonconventional Java files still retain declarations with a gap.

## Boundaries and errors

Malformed syntax contributes a per-file gap and no partial declarations. Any
raw backslash followed by `u` conservatively makes the complete file an explicit
Unicode-preprocessing gap, including occurrences in literals/comments: Java's
pre-lexical escape eligibility is not evaluated. JPMS module descriptors and
unsupported compilation-unit forms are explicit file boundaries. Local types,
anonymous/enum-constant class bodies and initializer bodies are excluded.
Kotlin sources are recorded as boundaries; no Java/Kotlin symbol reconciliation.

Invalid UTF-8 and capacity violations are typed errors before publication.
Limits are 4 MiB/source, 250,000 syntax nodes/source, depth 128, 100,000 graph
entities, 32 MiB canonical output and the existing 60-second scan deadline.
Acquisition retains the standard packed SHA-1 profile and its bounds. Empty Java
input fails explicitly. Other language selectors, duplicate flags and unsupported
compositions are rejected. No refresh, compiler accuracy, GA or SLO claim.

Portable packages validate canonical encoding, lineage, hashes, references and
claim states before output. Export/explore reuse marker-owned publication and
the existing filesystem restrictions, including Windows path boundaries. The
viewer embeds validated escaped data, uses text rendering under exact CSP
hashes, and normalizes LF/CRLF checkout bytes. No network or automatic launch.

## Acceptance and measurement

The authored fixture checks exactly 26 declarations, 22 nested containment edges,
two distinct overload signatures, all explicit gap reasons, and false friends
in comments, strings, local classes and anonymous bodies. Tests cover immutable
Git scans, store/query/docs/export/viewer, corruption, unsafe output, malformed
source, Unicode preprocessing, UTF-8 and capacity failures. Fixture precision
and recall apply only to the declared oracle fields.

Two public license-identified repositories are pinned in
`tests/specifications/s8/java-public-v1.json` and scanned three times each. Retain every attempt, elapsed time, declaration/entity/edge/gap
counts, exact source and binary identity, and semantic-hash stability. Corpus
counts and repeatability do not measure public semantic accuracy or scalability.

## Commands

```sh
noesis scan --profile standard-local-s8 --java-profile java-declarations-v1 \
  --repository /absolute/repository --revision FULL_COMMIT_SHA \
  --repository-identity urn:codenoesis:repository:example --store /absolute/store
noesis query --java-profile java-declarations-v1 --store /absolute/store \
  --repository-identity urn:codenoesis:repository:example --search Library
noesis docs --java-profile java-declarations-v1 --store /absolute/store \
  --repository-identity urn:codenoesis:repository:example
noesis export --java-profile java-declarations-v1 --store /absolute/store \
  --repository-identity urn:codenoesis:repository:example --output /absolute/export
noesis explore --java-profile java-declarations-v1 \
  --input /absolute/export/portable-graph.json --output /absolute/viewer
```

Query reports total matches and returns at most 100. Docs is canonical JSON.
The standalone viewer exposes entity, source evidence, containment and gap
inspection; HTML, portable graph and manifest are published together.


```sh
cargo build -p noesis --release --locked
python3 scripts/run_java_benchmark.py --binary target/release/noesis \
  --corpus /absolute/java-corpus --output /absolute/new-observation
```

The corpus directory contains full local clones named `rest-service` and
`junit-examples` with the manifest commits available. The runner executes only
CodeNoesis, retains all three attempts and rejects incomplete or unstable runs.
No wrapper, target build, compiler or public accuracy oracle is invoked.
