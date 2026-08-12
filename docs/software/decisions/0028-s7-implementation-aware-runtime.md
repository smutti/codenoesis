# Decision 0028: S7 implementation-aware runtime pilot

| Field | Value |
|---|---|
| Status | Proposed branch-scoped candidate; becomes Accepted only after protected manual merge of the exact issue #168 pull request |
| Date | 2026-08-12 |
| Decider | `@smutti`, through the explicit issue #168 package authorization |
| Issue | [#168](https://github.com/smutti/codenoesis/issues/168) |
| Base | `c212db5062ce28731f4aadc528a750d4ba33524f` |
| Requirements | Approved `DR-SEM-001`, `FR-IMP-004`, `FR-IMP-005`; Proposed `FR-EXT-018`, `FR-EXT-019`, `FR-CLI-006` |
| Slice | `S7 — Change impact` |
| Risk | High: public semantic classifier, source-capability contracts, parser supply chain, stable identities, cross-repository impact, privacy, and immutable oracle |
| Dependency | Exactly `tree-sitter-kotlin-ng = 1.1.0`; no other new dependency |

## Context

Decision 0007 fixes the meaning and exact public bytes of the first
implementation-aware HTTP/JSON compatibility report. S5 supplies immutable
revision comparison and S6 supplies source-neutral operation federation, but
neither proves provider emission behavior or client decoder assumptions. R15
now supplies a closed source-normal Rust flow layer that can support one narrow
provider proof without claiming compiler control flow.

Issue #168 selects one complete C0-C4 pilot rather than inventing an R16
ontology milestone. It combines this runtime governance, two retained Reds,
the minimum source adapters, one output-only command, tests, and evidence in a
single maintainer-supervised pull request. The existing Decision 0007, S7
schema, rule catalog, oracle, fixture, contract bundle, and golden are immutable
inputs to this package.

## Decision

### Public operation and authority

The only new invocation is:

```text
noesis impact \
  --workspace <impact-workspace-v1.json> \
  --profile implementation-aware-http-json-v1 \
  --format json
```

Successful execution exits `0`, writes exactly one canonical LF-terminated
`SemanticCompatibilityReportV1` to stdout, writes no stderr, and creates no
store or artifact. Invalid input or unsupported capability exits `2` with one
strict LF-terminated `CodeNoesisErrorV23` on stderr and no stdout. Unexpected
internal failure exits `1` under the same stream rule.

`ImpactWorkspaceV1` is the complete authority boundary. It binds explicit
provider baseline and target roots, revisions, contract and source paths,
SHA-256 digests, one callable symbol, explicit client roots, revisions, source
paths, source digests, decoder and call symbols, and one exact S6 federation
report path and digest. Environment, current directory discovery, repository
enumeration, Git, network, build tools, Gradle, Cargo target execution, plugins,
models, tests, and runtime traces are not authority.

The runtime registration is `codenoesis.pipeline/s7-v1`. The immutable public
golden continues to carry `codenoesis.pipeline/semantic-impact/v1`; this
package does not rewrite that accepted report field.

### Provider capability

`rust-direct-json-map/v1` is source-only and supports exactly one explicitly
selected committed Rust callable with these properties:

- one local `serde_json::Map` value;
- direct literal-key `map.insert("field", value)` writes;
- direct supported `if` control around those writes;
- one direct `serde_json::Value::Object(map)` publication;
- `guaranteed_present` only when each supported normal source path reaches a
  direct write before publication;
- `may_be_absent` only when a supported normal source path reaches publication
  without that write.

The implementation may reuse R15 source-normal flow facts but may not relabel
them as compiler CFG or actual runtime reachability. Helpers, aliases, loops,
unsupported early return, macros, generated code, custom codecs, reflection,
dynamic keys, framework registration, type or trait inference, panic or unwind,
runtime configuration, and target execution remain `unknown` with coverage.
The fixture call `custom_profile_fields(user)` is never traversed or guessed.

### Client capability

`kotlin-direct-json-access/v1` uses `tree-sitter-kotlin-ng 1.1.0` through the
already pinned Tree-sitter 0.26 language interface. It supports exactly one
selected committed Kotlin/KMP decoder and one direct selected call path:

- `JsonObject.getValue("field")` proves `requires_present`;
- guarded indexed access `payload["field"]?.…` proves `handles_absent`;
- a direct `httpGet` literal/interpolation path maps only to an already
  confirmed S6 operation identity;
- every fact retains exact source evidence and capability version.

DTO type spelling alone is not behavioral evidence. Generated serializers,
annotations with unresolved configuration, custom serializers or codecs,
reflection, aliases, extension indirection, arbitrary control flow, dynamic
paths, framework or runtime configuration, Gradle or compiler execution, and
unresolved calls remain `unknown`. A client for `/accounts/{id}` is rejected
even when its DTO and field names resemble the `/users/{id}` client.

### Reconciliation and output

The pipeline keeps `declared_contract`, `provider_implementation`, and
`client_assumption` as independent evidence views. It validates the exact S6
operation authority before applying the immutable Decision 0007 rule catalog.
Unsupported predicates produce `unresolved` plus a gap; they never default to
compatible or breaking. Byte-identical OpenAPI input does not suppress a
provider implementation delta.

The immutable acceptance result is the existing 14,991-byte golden with
SHA-256
`cfd9a8d4dcb2d04bcd9eaffd15f1ae947ffdaba80e07daee43375c9a67c15750`:

- two semantic diffs;
- two client assessments;
- one rejected decoy;
- nine evidence records;
- one coverage gap;
- `/nickname` changes from provider `guaranteed_present` to
  `may_be_absent`, breaks only the strict linked client, and leaves the safe
  linked client compatible;
- `/displayName` remains unresolved because the custom helper is unsupported.

### Closed failures

`CodeNoesisErrorV23` contains only:

- `impact.invalid_workspace`;
- `impact.invalid_federation_report`;
- `impact.unsupported_implementation_semantics`;
- `impact.limit_exceeded`;
- `impact.mutable_input`;
- `impact.internal`.

The command never emits a partial report. Unknown command or profile, malformed
source, panic, timeout, schema repair, network or target execution, and a
comparison of only the two OpenAPI documents are rejected outcomes.

### Limits

Decision 0007 maxima remain authoritative: 10,000 operations, 5,000 fields per
operation, 10,000 linked clients, 1,000,000 call sites, 200,000 semantic diffs,
1,000,000 evidence records, 200,000 gaps, and 67,108,864 report bytes.

This decision additionally fixes:

| Resource | Maximum |
|---|---:|
| Workspace bytes | 1,048,576 |
| Federation report bytes | 67,108,864 |
| Source files | 10,002 |
| Source bytes per file | 2,097,152 |
| Total source bytes | 268,435,456 |
| Syntax nodes per source | 1,000,000 |
| Syntax nesting depth | 256 |
| String literal bytes | 16,384 |
| Logical path bytes | 4,096 |
| Callable symbol bytes | 1,024 |

Maximum and maximum-plus-one behavior is typed and deterministic. Limits are
charged before allocating or traversing beyond the reviewed bound.

### Filesystem, privacy, and determinism

Every root is a non-symlink directory and every logical path is relative UTF-8
with `/`, without absolute form, traversal, empty segments, backslashes, NUL,
CR, or LF. Each file read is bound to its reviewed SHA-256 and a stable handle;
replacement during validation returns `impact.mutable_input` rather than
retrying into success.

The report exposes evidence metadata and excerpt digests, never raw source
bodies or snippets, absolute roots, credentials, environment, compiler
arguments, telemetry, or private URLs. Fifty input/order permutations and ten
shuffled parallel schedules must be byte-identical. The normal path opens no
network and launches no child, build, target, plugin, model, or browser.

## Required delivery history

The branch preserves this sequence:

1. this governance checkpoint, schemas, threat model, and conformance guard;
2. retained conformance Red because production capability registration is
   absent;
3. minimal CLI boundary and retained product Red with exact
   `impact.unsupported_implementation_semantics`;
4. minimum complete adapters, classifier, canonical writer, tests, and Green
   evidence;
5. independent review and protected manual merge.

Candidate requirements remain Proposed and behavior has no effect on `main`
before step 5. The authoring agent cannot approve or merge its own pull request.

## Consequences

The pilot can answer whether an optional contract field was actually guaranteed
by one provider revision and required by one client path, while preserving
explicit uncertainty. It does not advertise broad Rust, Kotlin, KMP,
serialization-framework, compiler-CFG, runtime, or cross-language support.
Additional source shapes, dimensions, formats, frameworks, and observations
require separate capability decisions.
