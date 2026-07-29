# Decision 0007: S7 implementation-aware API compatibility contract

| Field | Value |
|---|---|
| Status | Proposed; becomes Accepted only after protected manual merge of [PR #63](https://github.com/smutti/codenoesis/pull/63) |
| Date | 2026-07-29 |
| Deciders | Andrea Moretti (`@smutti` persona), with protected manual merge by `@smutti` as the approval event |
| Issue | [#62](https://github.com/smutti/codenoesis/issues/62) |
| Requirements | `DR-SEM-001`, `FR-IMP-004`, `FR-IMP-005` |
| Slice | `S7 — Change impact` |
| Risk | High: public semantic compatibility artifact, versioned classifier, ontology-facing identities, cross-repository client impact, and protected fixture/oracle |
| Scope | Governance only; production implementation, policy binding, workflow changes, dependencies, and target execution are excluded |

## Context

Comparing only two OpenAPI documents cannot answer whether a provider changed
behavior without editing its contract or whether a client relies on a guarantee
the contract never made. A response field can be:

- optional in the declared contract;
- unconditionally emitted by one provider revision;
- conditionally omitted by a later provider revision;
- required by one client decoder;
- safely handled by another client;
- present in an unrelated client with a similar type or field name.

Treating these views as one fact would create both false negatives and false
positives. In particular, an unchanged contract must not suppress an
implementation delta, a source-language nullable type must not automatically
prove decoder behavior, and a missing runtime trace must not prove that a path
or value is absent.

Arbitrary program equivalence and complete behavioral inference are
undecidable in the general case. The production contract therefore needs a
bounded capability model, exact evidence, deterministic rules, explicit
coverage gaps, and an `unresolved` result whenever the required proof is
outside an approved capability.

This decision partially resolves `OD-CMP-001` for one versioned HTTP/JSON
semantic compatibility projection. It does not approve S5, S6, S7 production
implementation, a broad Rust or Kotlin framework adapter, GraphQL, protobuf,
AsyncAPI, arbitrary control flow, runtime causality, or a universal
compatibility claim.

## Decision

### Profile and prerequisites

The versioned analysis profile is:

```text
implementation-aware-http-json/v1
```

It consumes immutable baseline and target provider snapshots plus immutable
client snapshots. The provider snapshots carry declared contract evidence and
provider implementation evidence. Client snapshots carry federation and
implementation evidence for actual decode, validation, request construction,
and use paths.

Before the first production Red may run, all of the following must be Approved
and Implemented:

1. S5 snapshot comparison;
2. S6 deterministic provider/client federation;
3. an OpenAPI 3.1 HTTP/JSON contract capability;
4. the exact provider source capability used by the implementation fixture;
5. the exact client source capability used by the implementation fixture.

The project-owned fixture uses a bounded direct Rust JSON-map shape and direct
Kotlin/KMP JSON access to define the reviewed meaning. Their presence in the
fixture does not advertise those adapters as supported. Each source capability
still requires its own language/framework matrix, malformed and negative
fixtures, limits, and approval.

### Three independent fact views

The classifier never overwrites one fact view with another:

| View | Meaning | Minimum authority |
|---|---|---|
| `declared_contract` | What the versioned API contract declares for an operation and message shape. | Authoritative validated contract bytes bound to the provider revision. |
| `provider_implementation` | What approved static analysis can prove about provider validation or emitted output on supported paths. | Exact source evidence plus a versioned deterministic capability/rule. |
| `client_assumption` | What an approved client decoder, validator, request builder, or use path requires or safely handles. | Exact source evidence, an actual call/decode path, and a versioned deterministic capability/rule. |
| `test_observation` | What one bound test execution observed. | Immutable test input, environment, result, coverage, and revision evidence. |
| `runtime_observation` | What one bound runtime observation recorded. | Immutable source, time/configuration world, coverage, and revision evidence. |

A test or runtime observation establishes only the observed event and bound
world. It does not by itself establish a universal invariant. Absence of an
observation is never evidence of absence unless a separately approved coverage
and opportunity rule proves that stronger statement.

Parser and authoritative-contract spans enter as `deterministic_fact`.
Versioned static semantic and compatibility rules emit `derived_fact`.
Heuristic federation, unsupported framework recognition, or model output
remains `candidate` or a coverage gap and cannot produce a `breaking` client
finding. Human or governed review cannot rewrite the originating evidence kind.

### Stable identities

The v1 fixture and future artifact use BLAKE3-256 over RFC 8785 canonical JSON
arrays. Each preimage begins with its exact domain:

```text
service:
["codenoesis.service-id/http/v1", canonical_service_authority]

operation:
["codenoesis.operation-id/http/v1", service_id, uppercase_method,
 canonical_path_template, explicit_operation_id]

field:
["codenoesis.field-id/http-json/v1", operation_id, direction,
 response_status_or_request, canonical_json_pointer]

client:
["codenoesis.client-id/v1", canonical_client_repository_identity]

call site:
["codenoesis.call-site-id/v1", client_id, client_revision,
 normalized_source_path, canonical_symbol_identity]

semantic diff:
["codenoesis.diff-id/v1", provider_repository_identity,
 baseline_revision, target_revision, field_id, dimension]

evidence:
["codenoesis.evidence-id/v1", repository_identity, revision,
 normalized_source_path, decimal_start_line, decimal_end_line,
 excerpt_sha256]

coverage gap:
["codenoesis.coverage-gap-id/v1", subject_id, reason_code,
 baseline_revision, target_revision]
```

The resulting URN is
`urn:codenoesis:<kind>:blake3:<lowercase-hex-digest>`. Source paths are
repository-relative UTF-8 paths with `/` separators. Operation matching follows
the S6 authority decision; this S7 package does not promote a name-only or
type-only match.

### Field semantics

The v1 compatibility projection keeps these dimensions distinct:

1. `presence`;
2. `nullability`;
3. `default`;
4. `validation`;
5. `value_set`;
6. `http_status`;
7. `error_code`.

For OpenAPI 3.1 object properties, membership in the containing schema's
`required` array determines declared presence. A property's inclusion of the
JSON Schema `null` type determines declared nullability. These are independent.
The OpenAPI/JSON Schema `default` keyword is an annotation and does not prove
that either provider or client applies the value.

Request and response direction are mandatory inputs to every rule:

- making a request field newly required can reject existing callers;
- making a response field newly optional can break clients that require it;
- accepting more request values is normally compatible for callers, while
  emitting more response values can break exhaustive consumers;
- provider validation, response status, and error identity changes are
  evaluated in the direction in which the client relies on them.

Read/write direction, media type, response status, containing schema, and field
JSON Pointer are part of the field context. A property name alone is never a
cross-operation identity.

### Provider implementation semantics

`guaranteed_present` requires a static proof that every supported normal output
path for the operation writes the field before publication. `may_be_absent`
requires a supported path that can publish the response without that write.
Equivalent rules apply to input requirements, null emission, defaults,
validation, value sets, status, and error identities.

The fixture's baseline direct unconditional
`body.insert("nickname", ...)` proves `guaranteed_present` within the approved
closed direct-map capability. The target's conditional insert proves
`may_be_absent`.

The following never silently produce a universal fact:

- an unresolved call, custom mapping helper, custom codec, reflection, dynamic
  registration, generated code, macro expansion, or runtime configuration;
- a return type or field type without the serializer/validator/output path;
- one test or trace without universal coverage evidence;
- model-generated explanations.

The fixture's `custom_profile_fields(user)` therefore produces
`unsupported_custom_provider_mapping` for `displayName`. Both implementation
states remain `unknown`, and the semantic diff remains `unresolved`.

### Client implementation assumptions

A client assumption requires the linked operation plus the actual applicable
decode, validation, request construction, or use path. A declaration such as
`String`, `String?`, `Optional<T>`, pointer, zero value, or generated DTO field
is evidence about source shape but is not sufficient by itself.

The v1 fixture proves:

- `getValue("nickname")` on the linked strict decoder path requires field
  presence;
- guarded indexed access on the linked safe decoder path handles absence;
- an otherwise similar decoder whose call path is `/accounts/{id}` is a decoy
  and is rejected by operation identity.

Framework adapters must account for constructor defaults, serializer and
decoder configuration, annotations, custom adapters, generated code, and
actual call paths. If any required piece is unsupported or ambiguous, the
assumption is `unknown` with a coverage gap.

### Semantic revision diff

The classifier compares baseline and target facts for each stable operation,
field, direction, and dimension. It reports contract and implementation deltas
separately. Byte-identical contract input does not imply an empty semantic
diff.

The fixture fixes the principal v1 outcome:

```text
field: /nickname
direction: response
dimension: presence
contract: optional -> optional
provider implementation: guaranteed_present -> may_be_absent
change kind: implementation_behavior_changed_without_contract_change
strict client: potentially_breaking -> breaking
safe client: compatible -> compatible
decoy: rejected
```

Before the provider change, the strict client's requirement is a latent
`potentially_breaking` contract/client mismatch. The provider's stronger
baseline implementation is an undocumented guarantee, not a contract
amendment. Removing that implementation guarantee in the target makes the
deterministically linked strict client `breaking`; it does not affect the safe
client.

### Rule catalog and classification

The authoritative v1 catalog is
[`compatibility-rule-catalog-v1.json`](../../../tests/specifications/s7/compatibility-rule-catalog-v1.json).
Each output records an exact rule ID and version.

For one subject/client pair:

1. reject non-authoritative or conflicting federation;
2. emit `unresolved` with coverage gaps when a required predicate fact is
   unknown, candidate-only, conflicting, or unsupported;
3. evaluate only rules matching direction and dimension;
4. select the highest numeric priority;
5. break equal-priority ties by classification severity and then rule ID.

`breaking` requires deterministic or confirmed operation identity and all facts
required by its predicate. A candidate client mapping remains `unresolved`.
The classifications are:

| Classification | Meaning |
|---|---|
| `compatible` | The approved rule proves compatibility for the represented client path and dimension. It is not a universal statement about uncovered paths. |
| `potentially_breaking` | Deterministic evidence exposes a latent mismatch or change whose concrete client failure is not proven. |
| `breaking` | A versioned deterministic rule proves that the represented change violates a deterministically linked client assumption. |
| `unresolved` | Required identity or semantic evidence is missing, conflicting, candidate-only, or outside an approved capability. |

Full causal impact, arbitrary behavioral equivalence, latency/SLO semantics,
business invariants, call-order protocols, and side-effect equivalence remain
outside v1.

### Public report

The strict public artifact is
[`SemanticCompatibilityReportV1`](../../../tests/specifications/s7/semantic-compatibility-report-v1.schema.json).
It contains:

- schema, profile, configuration, pipeline, ontology, extractor, evidence
  lineage, and rule-catalog versions;
- immutable provider baseline and target identities and contract digests;
- separate contract and implementation deltas;
- direction, dimension, before/after states, change kind, classification,
  claim state, rule, and affected clients;
- each linked client's operation/call-site identity, federation state,
  assumption, baseline risk, target impact, evidence, and gaps;
- rejected candidate clients;
- exact evidence spans and excerpt digests;
- explicit coverage gaps.

All objects reject unknown fields. Semantic diffs sort by `id`; client
assessments and rejected candidates sort by `client_identity`; evidence and
coverage gaps sort by `id`; every set-like ID array sorts by UTF-8 bytes and
rejects duplicates. Every reference must resolve exactly once in the same
report. `end_line` must be greater than or equal to `start_line`, and the
recomputed excerpt digest and evidence ID must match the bound source bytes.

### Fixed v1 limits

The profile fails with a typed bounded outcome before exceeding:

| Resource | Maximum |
|---|---:|
| Operations | 10,000 |
| Fields per operation | 5,000 |
| Linked clients | 10,000 |
| Call sites | 1,000,000 |
| Semantic diffs | 200,000 |
| Evidence items | 1,000,000 |
| Coverage gaps | 200,000 |
| Serialized report bytes | 67,108,864 |

These limits resolve only this S7 projection subset of `OD-LIM-001`. Each
future implementation issue must fix typed maximum and maximum-plus-one
behavior before production code.

### Security and deterministic operation

The standard profile:

- executes no provider or client build, compiler, package manager, target,
  plugin, hook, test, or generated binary;
- opens no analysis network channel and invokes no model provider;
- treats contracts, provider source, client source, configuration, generated
  code, observations, and model responses as untrusted input;
- never includes absolute paths, source bytes outside approved evidence spans,
  secrets, credentials, or runtime payload values in the report;
- applies explicit file, graph, rule, and output limits;
- produces byte-identical semantic output for the same immutable inputs,
  capability versions, and configuration.

Runtime and test observations, when separately enabled in a later sandboxed
profile, remain optional evidence sources and cannot weaken the deterministic
local path.

## Acceptance oracle

The project-owned fixture is
[`implementation-aware-api-v1`](../../../tests/fixtures/s7/implementation-aware-api-v1/README.md).
Its manifest binds exact source files, hashes, identities, sentinels, and the
reviewed expected report.

The machine oracle is
[`e2e_fr_imp_004_implementation_aware_api_diff.json`](../../../tests/specifications/s7/e2e_fr_imp_004_implementation_aware_api_diff.json).
It requires:

1. byte-identical baseline and target OpenAPI files;
2. a provider-only `nickname` presence delta;
3. a latent strict-client risk at baseline and `breaking` target impact;
4. a compatible safe client;
5. a rejected name-similar decoy;
6. an unresolved `displayName` diff with exact custom-mapping evidence;
7. closed schema, identities, references, ordering, limits, and evidence spans;
8. zero target, build, network, plugin, or model execution.

After every prerequisite exists, the first production acceptance test must be
observed Red because implementation-aware source semantics are explicitly
unsupported and no semantic report finding is emitted. Unknown command,
compilation failure, missing fixture, schema failure, panic, timeout, execution
attempt, or comparing only OpenAPI documents is not acceptable Red evidence.
No production Red is run in this governance package because the required S5,
S6, and source capability implementations do not yet exist.

## Implementation constraints

- The compatibility domain and rules remain independent of Tokio, SQLx, Axum,
  filesystem APIs, MCP, and model-provider SDKs.
- Contract, provider, client, observation, federation, and rule adapters
  implement inward-owned ports; interfaces contain no classifier logic.
- Capability adapters emit facts and gaps; they do not classify client impact.
- The deterministic classifier is a versioned Rust rule engine over validated
  facts and cannot inspect unvalidated raw model output.
- No first-party `unsafe`, target execution, network repair, dependency
  fetching, generated-client execution, or hidden framework fallback is
  authorized.
- New language, framework, media type, contract format, semantic dimension, or
  rule requires its own capability matrix and reviewed positive/negative
  fixture before being advertised.

## Consequences

- CodeNoesis can expose a provider implementation change that a contract-only
  diff would miss.
- A client that is stricter than the contract remains visible before it
  actually breaks.
- Safe linked clients and plausible decoys are distinguished from affected
  clients.
- Unsupported implementation semantics remain honest gaps instead of model or
  heuristic guesses.
- Initial delivery is narrower because provider and client capability profiles
  must be approved independently, but later languages can reuse the same report
  and classifier contract.

## Deferred

- S5 and S6 governance and implementation;
- complete OpenAPI, AsyncAPI, GraphQL, and protobuf compatibility;
- serializer/framework-specific semantics beyond approved direct-source
  capabilities;
- generated clients, reflection, macros, custom codecs, runtime configuration,
  and trusted-build indexes;
- request bodies, multipart, streaming, event ordering, protocol state
  machines, business invariants, side effects, performance envelopes, and
  causal inference beyond separately approved rules;
- observation ingestion, coverage proof, traffic replay, and natural-release
  experiments;
- public CLI/REST/MCP impact command shape and server operation.

## Approval and separation

This decision, the strict report schema, rule catalog, acceptance specification,
fixture, reviewed report, maintenance guard, and bundle form one protected
governance package. The policy registry remains unchanged. Production code,
dependencies, workflows, and existing accepted bundles remain unchanged.

Approval occurs only when `@smutti` manually merges the exact protected PR
head after every required gate is green. The authoring agent does not approve
or merge. A separate protected policy PR may then bind only `DR-SEM-001`,
`FR-IMP-004`, and `FR-IMP-005` to the exact merged SRS. Future production work
remains `human-required` until the S5, S6, and capability prerequisites are
Approved and Implemented and a separate S7 issue satisfies the repository
Ready criteria.
