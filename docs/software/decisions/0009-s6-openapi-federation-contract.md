# Decision 0009: S6 bounded OpenAPI federation contract

| Field | Value |
|---|---|
| Status | Proposed; becomes Accepted only after protected manual merge of [PR #81](https://github.com/smutti/codenoesis/pull/81) |
| Date | 2026-07-30 |
| Deciders | Andrea Moretti (`@smutti` persona), with protected manual merge by `@smutti` as the approval event |
| Issue | [#78](https://github.com/smutti/codenoesis/issues/78) |
| Requirements | `FR-EXT-004`, `FR-FED-001`, `FR-FED-002`, `FR-CLI-005` |
| Slice | `S6 — Contract federation` |
| Risk | High: untrusted OpenAPI and YAML input, public CLI and artifact contracts, cross-project authority, stable identities, heuristic epistemic state, bounded output, and downstream S7 correctness |
| Scope | Governance only; production implementation, policy binding, workflows, architecture, dependency manifests, lockfiles, ontology, storage, and accepted S0–S5/S7 artifacts are excluded |
| Human authorizations | [S6 package](https://github.com/smutti/codenoesis/issues/78#issuecomment-5131609031), [reviewed `yaml-rust2` replacement](https://github.com/smutti/codenoesis/issues/78#issuecomment-5133952315), and [output-only accelerated package](https://github.com/smutti/codenoesis/issues/78#issuecomment-5135312628) |

## Context

S7 already fixes an implementation-aware compatibility model, but its first
production Red is intentionally blocked on deterministic provider/client
federation. A field-level compatibility conclusion is unsafe when a
name-similar client is silently attached to the wrong service or operation.
Conversely, demanding source-language support before any federation exists
would conflate contract identity with framework-specific implementation facts.

S6 therefore establishes one narrow capability:

- one explicitly authorized local workspace manifest;
- one immutable OpenAPI 3.1.0 HTTP/JSON provider contract;
- explicit client-operation declarations as workspace catalog evidence;
- deterministic confirmed links, rejected decoys, heuristic candidates, and
  unresolved coverage gaps;
- one strict, bounded, output-only report.

Client declarations identify intended operation bindings. They do not prove
decoder, serializer, validation, framework, generated-code, source-language,
or runtime behavior. Those facts require independently approved adapters and
remain the concern of S7 and later slices.

The S6 operation does not need persistent publication. Extending the accepted
S3 store would require a new schema, migration, authority, and recovery
decision outside issue #78. The maintainer therefore selected an output-only
command under the accelerated delivery lane. The complete report is buffered,
validated, size-checked, and written once; no partial report or persistent
mutation is permitted.

## Decision

### Public operation

The only S6 invocation is:

```text
noesis federate \
  --workspace-manifest <path> \
  --profile standard-local-s6 \
  --format json
```

Configuration comes only from these explicit arguments and the exact local
paths bound by the workspace manifest. Environment variables, current-user
configuration, repository discovery, network discovery, model output, and
plugins are not configuration sources.

Successful execution:

- exits `0`;
- writes exactly one RFC 8785 canonical `FederationReportV1` JSON document
  followed by LF to stdout;
- writes no stderr;
- creates or mutates no persistent store or artifact.

Invalid invocation exits `2`. Contract, federation, or limit failure exits
`10`. An unexpected internal failure exits `70`. Every failure writes no
stdout and exactly one LF-terminated strict `CodeNoesisErrorV8` document to
stderr. The implementation must not write stdout bytes until the complete
report passes schema, identity, reference, ordering, limit, canonicalization,
and semantic-hash validation.

The command does not add `--store`, does not advance an S3 head, and does not
change any accepted S0–S5 command, profile, exit, stream, schema, or byte.

### Workspace authority

`FederationWorkspaceV1` names:

- one workspace identity and exact `standard-local-s6` profile;
- one exact contract capability and federation rule catalog;
- one provider repository identity, immutable revision, explicit local root,
  contract path, SHA-256 digest, and canonical service authority;
- zero or more client roles, explicit local roots, declaration paths, and
  SHA-256 digests.

Paths are normalized UTF-8 relative paths with `/` separators. Absolute paths,
empty segments, `.` and `..`, NUL, backslash aliases, symlink escapes, and
outside-root resolution fail before reading escaped content. Only manifest
entries are authoritative. Directory enumeration cannot introduce a provider,
client, contract, or declaration.

Repository identities and revisions are caller-authorized catalog facts. S6
does not execute Git, resolve moving references, infer repository ownership,
or claim that declaration paths exist in a supported source-language model.

### Approved contract capability

The exact capability is:

```text
codenoesis.contract-capability/openapi-3.1-http-json/v1
```

It accepts exactly OpenAPI `3.1.0` documents encoded as UTF-8 JSON or the
restricted YAML subset below. The normalized projection supports:

- literal absolute HTTPS server authority without variables, userinfo, query,
  or fragment;
- `GET`, `POST`, `PUT`, `PATCH`, and `DELETE`;
- literal path templates and mandatory `operationId`;
- responses with `application/json`;
- JSON Schema object, array, boolean, integer, number, and string types;
- object-property presence from the containing schema's `required` array;
- local JSON Pointer references under `#/components/schemas`.

Callbacks, webhooks, links, security semantics, server variables, and content
outside `application/json` produce typed coverage gaps when the containing
document remains otherwise representable. Ambiguous service authority,
unsupported OpenAPI version, external reference, local reference cycle, or
structurally invalid required data fails the complete operation.

This decision approves only the OpenAPI capability-scoped portion of
`FR-EXT-004`. AsyncAPI, GraphQL, Protocol Buffers, other OpenAPI versions,
external references, and broader HTTP behavior remain Proposed and
unadvertised.

### Restricted YAML

JSON and YAML must enter the same normalized OpenAPI model. YAML is not a
second semantic profile. The accepted YAML subset has:

- one document;
- UTF-8 text;
- plain mappings, sequences, and scalar values needed by the approved
  projection;
- unique mapping keys;
- maximum nesting depth `32`.

Anchors, aliases, merge keys, custom tags, directives that alter semantics,
multiple documents, duplicate keys, malformed input, and unsupported scalar
interpretation fail before normalized model construction. Remote references
fail without opening a network channel.

Issue #78 authorizes this exact implementation candidate:

```toml
yaml-rust2 = { version = "=0.11.0", default-features = false }
```

The reviewed crate checksum is
`631a50d867fafb7093e709d75aaee9e0e0d5deb934021fcea25ac2fe09edc51e`;
the declared license is `MIT OR Apache-2.0`; the reviewed minimum Rust version
is `1.65.0`; no direct `unsafe` was observed in the crate's `src` at review;
and no OSV vulnerability was observed on 2026-07-30. This is candidate review,
not implementation evidence.

The Ready implementation issue must still bind the exact manifest and lockfile
change and retain:

- complete transitive dependency, license, advisory, and `unsafe` inventory;
- marked-event rejection before normalized construction;
- duplicate-key detection;
- depth and allocation charging;
- malformed and fuzz seeds;
- every exact maximum and maximum-plus-one result.

A second YAML or OpenAPI parser dependency is a stop condition.

### Canonical identities

Stable identities are BLAKE3-256 over RFC 8785 canonical JSON arrays. Each
array begins with the exact domain string. The result is:

```text
urn:codenoesis:<kind>:blake3:<lowercase-hex-digest>
```

The core preimages are:

```text
service:
["codenoesis.service-id/http/v1", canonical_service_authority]

operation:
["codenoesis.operation-id/http/v1", service_id, uppercase_method,
 canonical_path_template, explicit_operation_id]

schema:
["codenoesis.schema-id/http-json/v1", operation_id, direction,
 response_status_or_request, canonical_component_pointer]

field:
["codenoesis.field-id/http-json/v1", operation_id, direction,
 response_status_or_request, canonical_json_pointer]

client:
["codenoesis.client-id/v1", canonical_client_repository_identity]

call site:
["codenoesis.call-site-id/v1", client_id, client_revision,
 normalized_source_path, canonical_symbol_identity]

confirmed link:
["codenoesis.federation-link-id/v1", operation_id, client_id,
 call_site_id, binding_kind]

heuristic candidate:
["codenoesis.federation-candidate-id/v1", target_operation_id, client_id,
 call_site_id, "heuristic_name", service_hint, operation_hint]

rejection:
["codenoesis.federation-rejection-id/v1", operation_candidate_id, client_id,
 call_site_id, reason_code]

coverage gap:
["codenoesis.federation-gap-id/v1", subject_id, reason_code,
 primary_evidence_id]
```

Evidence identities bind repository identity, immutable revision, normalized
path, exact source selector, and file SHA-256. A JSON declaration selector is
its canonical JSON Pointer. A contract selector binds its normalized OpenAPI
location and the truthful format-specific source span. Evidence is
format-specific even when the represented contract fact is source-neutral.

No clock, machine path, traversal order, scheduling order, process identifier,
workspace checkout location, or volatile run identifier enters a stable
identity.

### Federation authority and epistemic state

The exact catalog is:

```text
codenoesis.federation-rules/http-json/v1
```

Its authority order is:

1. explicit workspace identity;
2. package or SCIP identity;
3. canonical operation identity;
4. event or schema identity;
5. heuristic candidate.

The v1 capability exposes explicit workspace and canonical operation
authority. Package/SCIP and event/schema authority remain coverage gaps. A
heuristic is never automatic confirmation.

The public states are `candidate`, `confirmed`, `rejected`, and `unresolved`.
The ordered rules are:

| Precedence | Rule | Result |
|---:|---|---|
| 10 | `fed.explicit-operation.confirm/v1` | Confirm an exact explicit service, operation, revision, client, and call-site binding; conflict is unresolved. |
| 20 | `fed.operation-identity.confirm/v1` | Confirm an exact canonical service and operation identity with exact revision binding; conflict is unresolved. |
| 30 | `fed.operation-decoy.reject/v1` | Reject an explicit operation identity absent from the provider. |
| 40 | `fed.heuristic-name.candidate/v1` | Emit a candidate and coverage gap for bounded name similarity. |
| 50 | `fed.conflict.unresolved/v1` | Preserve conflicting authoritative evidence as unresolved. |

Explicit conflicting authority fails closed with
`federation.identity_conflict`; it is not resolved by rule order, heuristic
score, model output, or arbitrary input order. Human review may authorize a
future governed fact, but it cannot rewrite the provenance or epistemic state
of the originating evidence.

### Public artifacts and canonical report

The strict public artifacts are:

- `FederationWorkspaceV1`;
- `FederationClientDeclarationV1`;
- `FederationReportV1`;
- the S6 subset of `CodeNoesisErrorV8`;
- `codenoesis.federation-rules/http-json/v1`.

Every object schema is closed. Every collection is bounded. Identity-bearing
collections and every `evidence_ids` list are sorted by ascending stable
identity and unique. Rule, authority, scenario, and failure precedence lists
retain their declared order.

`FederationReportV1.semantic_hash` is BLAKE3-256 over:

```text
"codenoesis.federation-report.semantic.v1"
0x00
RFC8785(source_neutral_projection)
```

The projection removes:

- the `semantic_hash` member;
- the top-level evidence collection and every nested `evidence_ids`;
- provider contract path, SHA-256, source format, and evidence references;
- client declaration paths.

It retains normalized provider/client identities, operation and field facts,
confirmed links, candidates, rejections, coverage gaps, profile, capability,
catalog, workspace identity, and limits. Consequently equivalent reviewed JSON
and YAML contracts produce the same semantic hash and semantic identities,
while paths, byte digests, formats, selectors, and evidence identities remain
truthful to each source.

### Failure model

The fail-closed precedence is:

```text
invocation
workspace_manifest
repository_binding
contract_encoding
yaml_structure
openapi_profile
local_reference
client_declaration
federation_identity
resource_limit
report_validation
internal
```

`CodeNoesisErrorV8` is additive and applies only to `federate`. Earlier
commands retain their accepted error schemas. Error context contains only
bounded safe paths, identities, digests, numeric limits, and classified
details. It must not contain source bytes, secrets, host paths, parser debug
output, credentials, or unbounded third-party messages.

Malformed input, duplicate key, unsupported YAML feature, unsupported OpenAPI
version, remote reference, reference cycle, identity conflict, and every
resource maximum-plus-one outcome have exact typed codes in the reviewed
schema and oracle. A panic, partial stdout, untyped error, retry into success,
or fallback to a less authoritative parser is non-conforming.

### Fixed limits

`standard-local-s6` fixes these inclusive maxima:

| Resource | Maximum |
|---|---:|
| Workspace manifest bytes | 8,388,608 |
| Repositories | 128 |
| Contract documents | 256 |
| Bytes per contract document | 2,097,152 |
| YAML nesting depth | 32 |
| Local reference depth | 16 |
| Path items | 10,000 |
| Operations | 10,000 |
| Schemas | 20,000 |
| Fields per operation | 5,000 |
| Clients | 10,000 |
| Client declarations | 100,000 |
| Confirmed links | 1,000,000 |
| Candidates | 200,000 |
| Rejections | 200,000 |
| Evidence items | 1,000,000 |
| Coverage gaps | 200,000 |
| Report bytes | 67,108,864 |
| Accounted memory bytes | 536,870,912 |
| Wall milliseconds | 60,000 |

Every maximum must be accepted when the input is otherwise valid. Every
maximum plus one must return the exact `contract.limit_exceeded` or
`federation.limit_exceeded` outcome named by the machine oracle, with empty
stdout and no artifact.

Accounting starts before allocation or traversal. The report byte limit covers
the final canonical document including LF. The wall limit includes input,
parsing, normalization, federation, validation, hashing, and output
preparation. Parallel scheduling cannot weaken a shared bound.

### Security and determinism

The standard path performs no:

- target, package manager, compiler, build script, hook, or Git process;
- network, DNS, remote reference fetch, plugin, model-provider, or Council
  request;
- filesystem write or persistent publication;
- ambient repository or client discovery;
- first-party `unsafe`.

It reads only the manifest and explicitly bound local provider/client roots.
The acceptance harness observes processes, network, writes, descriptor bounds,
outside-root reads, memory, CPU, wall time, and the fixture sentinel.

Fifty input-order permutations and ten recorded shuffled parallel repetitions
must produce byte-identical canonical output. A diagnostic retry cannot replace
the first result as evidence.

### Reviewed fixture and future Red

The project-owned
[`s6-openapi-federation-v1`](../../../tests/fixtures/s6/openapi-federation-v1/README.md)
fixture contains:

- one OpenAPI provider in equivalent reviewed YAML and JSON forms;
- strict and safe explicit clients that produce exactly two confirmed links;
- one explicit operation decoy that is rejected;
- one name-only client that remains a candidate with an unresolved gap;
- hostile duplicate-key, alias, merge, custom-tag, multiple-document,
  remote-reference, reference-cycle, malformed, unsupported-version, and
  conflicting-authority variants;
- strict reviewed success and error artifacts;
- a non-executable sentinel.

The exact machine oracle is
[`e2e_fr_fed_001_openapi_federation.json`](../../../tests/specifications/s6/e2e_fr_fed_001_openapi_federation.json).
It binds every scenario, limit maximum and maximum plus one, dependency review,
determinism repetition, security observer, compatibility regression, required
evidence, and stop condition.

Before S6 production code, the merged binary interprets the exact `federate`
invocation through the existing unrecognized-command boundary. The accepted
future production Red is:

```text
cargo test --locked -p noesis \
  --test e2e_fr_fed_001_openapi_federation \
  -- --exact e2e_fr_fed_001_openapi_federation
```

The subject command exits `2`, writes empty stdout, and emits the exact
149-byte ErrorV2 `input.invalid_revision` stderr document with SHA-256
`6441e0037f864d2fae4a60e6355e4a85b26b00d5e4e24c59ffeb5fe9c6f3859f`.
Compilation failure, missing fixture or test target, network failure, panic,
timeout, target execution, or validation of hand-authored JSON without the
public command is not the expected Red.

### Downstream S7 conformance

The S6 fixture intentionally reuses the immutable S7 provider OpenAPI bytes and
the S7 provider, strict-client, safe-client, and decoy repository identities.
Service, operation, selected field, client, call-site, and decoy operation
identities must equal the S7 fixture.

This alignment proves identity compatibility only. Kotlin-shaped paths are
opaque declaration locators and do not advertise Kotlin or KMP extraction.
S6 does not prove S7 provider implementation facts, client assumptions,
framework behavior, compatibility classification, or causal impact.

## Delivery and separation

This governance pull request contains no product runtime. After its protected
manual merge, all four requirement IDs become Approved for this exact S6
capability. Under the maintainer-supervised accelerated lane, one separately
linked Ready implementation issue may authorize one coherent vertical
implementation pull request containing:

- the public black-box Red retained before production edits;
- the exact reviewed `yaml-rust2` manifest and lockfile change;
- the minimum domain, application, adapter, contract, and CLI behavior;
- focused tests, the full final-head gate, and immutable evidence.

The machine-policy projection may be prepared in parallel but remains required
before unattended autonomous execution. It is not required to delay the
already authorized supervised interactive implementation. Policy, workflow,
agent instruction, and control-plane changes remain outside the product pull
request.

The authoring agent cannot approve or merge either change.

## Consequences

### Positive

- S6 obtains an executable, source-neutral OpenAPI federation boundary before
  implementation.
- Explicit links, decoys, candidates, conflicts, and gaps remain distinct.
- YAML support is bounded and reviewed without authorizing a dependency change
  in governance.
- S7 identities become reproducible without claiming unsupported source
  semantics.
- Output-only delivery avoids an unauthorized storage migration.

### Costs

- Client declarations are an explicit catalog until language/framework
  adapters can derive equivalent facts.
- The restricted YAML subset rejects common YAML conveniences.
- Complete output buffering uses bounded memory to guarantee no partial
  stdout.
- OpenAPI coverage is intentionally smaller than the full specification.

### Deferred

- AsyncAPI, GraphQL, Protocol Buffers, other OpenAPI versions, and external
  references;
- package/SCIP and event/schema federation authority;
- governed human promotion records and candidate persistence;
- source-language and framework extraction;
- persistent federation storage, query, server, and MCP exposure;
- dynamic and runtime behavior;
- incremental federation across S5 refreshes;
- distributed execution and remote repositories.
