# Decision 0016: S4 R6 framework-declarations contract

| Field | Value |
|---|---|
| Status | Proposed; becomes Accepted only when `@smutti` manually merges the exact independently reviewed protected head of [PR #118](https://github.com/smutti/codenoesis/pull/118) |
| Date | 2026-08-04 |
| Owners | Andrea Moretti (`@smutti` governance persona), accountable maintainer `@smutti` |
| Scope | `S4 — Evidence-backed workspace docs compatibility extension` only; roadmap `R6` |
| Requirement | Proposed `FR-EXT-011`, with bounded compatibility amendments to `FR-EXT-001/002/010`, `FR-KNW-001/002/003`, `FR-DOC-001/002`, `FR-QRY-001`, and `FR-CLI-001` |
| Risk | High — public ontology, identity, snapshot, extraction, query, error, evidence, framework-role, macro-uncertainty, and compatibility contracts |
| Governance issue | [#117](https://github.com/smutti/codenoesis/issues/117) |
| Authorization | [Accountable-maintainer authorization](https://github.com/smutti/codenoesis/issues/117#issuecomment-5183312890) |
| Protected review | [PR #118](https://github.com/smutti/codenoesis/pull/118) |
| Required base | `6750f293c24ea501df6177a2f7c96c2c7f0a6390` |

## Context

The protected R5 governance, golden-correction, and product merges in PRs
#112, #115, and #116 make the explicit `rust-semantic-depth-v1` profile
available on `main`. R5 preserves committed fields, variants, constants,
statics, associated types, methods, and outer attributes, but deliberately
does not assign framework meaning to builder calls, attributes, derives, or
macro token trees.

That boundary avoids invented facts but leaves two materially different source
signals indistinguishable:

- a direct, closed registration expression whose committed syntax contains a
  literal method, path, key, and direct target spelling;
- an attribute or macro form whose apparent role depends on expansion,
  conditional compilation, type or trait resolution, generated code, or
  runtime framework behavior.

Treating both as absent hides useful committed syntax. Treating either as an
observed route, running service, active configuration, reachable handler, or
generated endpoint violates `INV-MDL-001`. R6 therefore introduces a
framework-neutral source-declaration family with explicit epistemic states.
The pinned Lekton and RustDesk observations motivate the two generic source
styles only. They are not ontology truth, product branches, framework
dependencies, or vendored acceptance fixtures.

## Decision

Add one explicit selector:

```text
--rust-framework-profile rust-framework-declarations-v1
```

It is valid only with all of:

```text
scan --profile standard-local-s4
--workspace-profile cargo-root-package-v1
--manifest-profile cargo-manifest-facts-v1
--rust-semantic-profile rust-semantic-depth-v1
```

Missing, invalid, or incomplete composition fails before repository
acquisition with ErrorV13. Repository content, dependency names, imports,
attributes, macros, public-corpus identity, file names, and earlier profiles
never select R6 implicitly. R1 packed acquisition and R2 repository-boundary
representation remain independent optional selectors.

Every invocation without the R6 selector retains its accepted R0-R5 success,
error, store, documentation, and query bytes. This protected change ratifies
governance only. Product implementation **requires a separate Ready product issue**
after this decision and `FR-EXT-011` are independently reviewed and manually
merged. It authorizes no production code, dependency, workflow, migration,
release, browser, server, explorer, compiler-index, or control-plane change.

## Public versions

The selected path uses exactly:

| Contract | Version |
|---|---|
| Snapshot | `codenoesis.repository-snapshot/v9` (`RepositorySnapshotV9`) |
| Configuration | `codenoesis.configuration/v6` |
| Extraction chunk | `codenoesis.extraction-chunk/v6` (`ExtractionChunkV6`) |
| Extraction contract | `codenoesis.extraction/v6` |
| Knowledge graph | `codenoesis.knowledge-graph/v6` (`KnowledgeGraphV6`) |
| Rust ontology | `codenoesis.ontology/rust/v6` |
| Error | `codenoesis.error/v13` (`ErrorV13`) |
| Exact-ID query | `codenoesis.local-query-result/v4` (`LocalQueryResultV4`) |
| Pipeline | `codenoesis.pipeline/s4-r6-v1` |
| Framework declaration extractor | `codenoesis.rust-framework/s4-r6-v1` |
| Framework declaration index | `codenoesis.framework-declaration-index/v1` |
| Semantic hash | `codenoesis.semantic-hash-contract/v5` |

V9 extends the complete immutable V8 lineage. It retains the R4 Cargo manifest
index, R3 workspace projection, R2 optional boundary projection, R5 semantic
index and identities, S3 publication semantics, S4 documentation roles, and
evidence-lineage v2.

## Epistemic states

R6 emits exactly two framework-declaration states:

### `declared_registration_syntax`

A closed, versioned source-profile rule matched one committed registration
form. The entity may retain only literal method, path, configuration key, and
target spelling present in that syntax. It says that registration syntax was
declared. It does not say the enclosing function is called, its result is
used, a service starts, configuration is active, an endpoint is available, a
route is reachable, or a handler executes.

### `candidate_unresolved`

A reviewed attribute, derive, declarative-macro, proc-macro-looking, `cfg`, or
`cfg_attr` form resembles one framework-neutral role, while its meaning
depends on unsupported expansion, configuration, type, trait, value,
generated-code, or runtime evidence. Raw tokens and their byte span remain
evidence. Candidate arguments never become authoritative method, path, key,
handler, component, service, configuration, endpoint, or runtime properties.

Resolved or observed runtime behavior is unsupported and has no R6 schema
state. R6 can neither emit nor silently encode `observed`, `active`,
`reachable`, `serving`, `running`, or `executed` framework meaning.

## Framework-neutral entities

Ontology v6 adds exactly:

- `framework.component_declaration`;
- `framework.service_declaration`;
- `framework.configuration_declaration`;
- `framework.endpoint_declaration`;
- `framework.route_declaration`;
- `framework.handler_declaration`.

Names describe roles in committed source declarations, not framework brands
or runtime objects. Every entity contains one lexical owner, one source
profile, one source-form rule identity, one epistemic state, inherited R5
compilation presence, exact evidence IDs, and the reviewed nullable literal or
target fields. Absence stays `null`; it is never repaired from comments,
imports, dependency names, naming conventions, types, wrappers, or macro
arguments.

## Closed explicit-builder profile

`explicit-builder-registration-v1` recognizes only a direct call-expression
chain in an approved return-tail context. The reviewed constructor terminal
spellings are `RegistrationSet::new` and `Router::new`; matching a spelling
does not resolve its type. The closed rule set covers:

- direct component, service, configuration, endpoint, and handler
  registrations;
- direct three-argument route registration containing literal method, literal
  path, and direct target spelling;
- direct two-argument route registration containing literal path and one
  reviewed method-wrapper call around a direct target spelling;
- direct `group` or `nest` registration containing literal path and direct
  target spelling;
- direct `layer`, `route_layer`, and `with_state` source registrations, which
  remain service or configuration declarations rather than runtime order or
  active state.

The chain is bounded and parsed without macro expansion. A standalone chain,
an unused local builder value, a returned alias requiring data-flow analysis,
a dynamically constructed path, a closure target, an arbitrary expression,
or a source form outside the closed rules is not promoted. It either remains
a reviewed hard negative or produces the exact unsupported-form gap. The
chain is registration syntax, not a general call graph and not proof that its
result reaches a runtime.

## Closed attribute and macro candidate profile

`attribute-macro-candidate-v1` recognizes only the reviewed route-,
component-, service-, configuration-, command-, runtime-entry-, bridge-,
endpoint-, derive-, `cfg`-, `cfg_attr`-, and declarative-route-macro-looking
patterns. It preserves the underlying R5 declaration when one exists and adds
one bounded `candidate_unresolved` entity.

Every attribute or derive candidate binds
`rust.attribute_semantics_not_interpreted`. Conditional forms additionally
bind `rust.cfg_presence_unresolved`. Declarative macro invocations bind
`rust.macro_generated_items_not_analyzed`. No macro argument is decoded into
an authoritative route or target, and no generated declaration is invented.
Comments, strings, documentation, imports, return types, parameter wrappers,
dependency names, isolated identifiers, naming-only conventions, generated
directories, target directories, and non-tail builders remain hard negatives.

## Lexical ownership and relationships

R6 reuses only `DEFINES`: the existing R5 lexical owner defines the framework
declaration entity. A framework entity has exactly one lexical owner and one
or more resolving committed-source evidence IDs.

No `CALLS`, `EXECUTES`, or `SERVES` relationship is introduced. R6 also
forbids equivalent `STARTS`, `REACHES`, or `ACTIVATES` edges. Local target
identity is a nullable entity property, not a call edge. Graph traversal must
not reinterpret `DEFINES` or that property as reachability, execution,
serving, activation, startup, or middleware order.

## Identity

All accepted R5 and earlier identities remain byte-identical. Framework
declarations use the disjoint domain:

```text
codenoesis.entity-id/framework-declaration/v1
```

The RFC 8785 JSON-array preimage, hashed with BLAKE3-256 after NFC
normalization, contains exactly:

1. repository identity;
2. unchanged crate identity;
3. lexical owner identity;
4. framework-neutral role;
5. source profile;
6. source-form rule identity;
7. normalized declared key or target spelling.

The resulting public ID retains the existing
`urn:codenoesis:entity:blake3:<digest>` shape. Commit OID, byte offset, source
order, chunk order, scheduler order, worker count, active `cfg` world, macro
output, inferred type, evaluated value, runtime address, and framework runtime
state never enter identity. Duplicate or NFC-colliding preimages fail before
publication; source order, ordinal, offset, scheduling, and retry may not
repair them.

For a route, the reviewed normalized declared key contains its literal method
when present, literal path, and target spelling. Distinct methods on the same
path therefore remain distinct. Candidate keys use only the reviewed attribute
or macro rule and underlying committed declaration spelling; uninterpreted
arguments cannot change authority.

## Local target binding

A builder declaration may retain a local target identity only when the
already committed R5 lexical facts resolve the exact target spelling to one
declaration. No compiler, trait, type, value, dependency, import expansion, or
external repository is consulted.

- `resolved_unique` retains the one local R5 entity ID;
- `unresolved_external` retains spelling and an exact unresolved gap;
- `ambiguous_local` retains spelling and an exact ambiguity diagnostic and
  gap, or fails where the future product oracle requires exact binding;
- `not_applicable` makes no target claim.

Source order or an ordinal cannot select an ambiguous target. Attribute-bound
candidates may retain the underlying local declaration identity while their
framework role remains unresolved. A declarative macro candidate has no
generated target identity.

## Evidence, claims, diagnostics, and documentation

Each framework declaration has exactly one deterministic parser claim and at
least one exact committed UTF-8 byte span. Every declaration also emits
`rust.framework_runtime_not_observed`. Candidate, `cfg`, macro, unresolved,
and ambiguous cases emit their additional reviewed diagnostic and coverage
capability. Evidence outside the bound repository, inside excluded generated
or target directories, through an escaping symlink, or failing byte-boundary
validation is rejected before graph ingestion.

Generated documentation and LocalQueryResultV4 preserve the same distinction.
Statements say “source registration declaration” or “unresolved candidate”
and explicitly deny observed runtime behavior. They never present a route as
reachable, a handler as executed, a service as started, configuration as
active, middleware as ordered, or generated code as present.

LocalQueryResultV4 is selected only by a validated stored V9 head and supports
exact entity, relationship, claim, evidence, diagnostic, coverage-gap, and
document IDs. V8 continues to emit LocalQueryResultV3, V7 continues to emit
V2, and V4-V6 continue to emit V1. There is no public query-version flag,
traversal, fuzzy search, repair, migration, or runtime inference.

## Fixed limits

R6 fixes:

| Limit | Maximum |
|---|---:|
| Framework declarations per committed source file | 4,096 |
| Explicit registration chain segments | 256 |
| Nested registration-expression depth | 64 |
| One literal route path | 2,048 UTF-8 bytes |
| One literal method or configuration key | 1,024 UTF-8 bytes |
| One target spelling | 1,024 UTF-8 bytes |
| Outer attributes per declaration | inherited R5 maximum 128 |
| Attribute token payload | inherited R5 maximum 16,384 UTF-8 bytes |
| Deterministic permutations | 50 plus isolated replay |

Existing graph, snapshot, repository, documentation, query, memory, output,
and wall-time limits remain authoritative. Every maximum is accepted and
maximum-plus-one fails before proportional allocation or publication with no
silent truncation, stdout, partial store, or documentation mutation. A new or
raised bound requires renewed approval.

## ErrorV13

New R6 failures are strict LF-terminated ErrorV13 with empty stdout,
`retryable: false`, no partial publication, and no store or documentation
mutation:

- `input.invalid_rust_framework_profile`;
- `extraction.invalid_framework_declaration`;
- `extraction.framework_declaration_identity_conflict`;
- `extraction.framework_declaration_limit_exceeded`;
- `extraction.unsupported_framework_composition`;
- `extraction.ambiguous_framework_target`;
- `extraction.unresolvable_framework_evidence`;
- `input.unsafe_framework_path`;
- `internal.unexpected`.

An intentionally unresolved candidate remains an exact diagnostic and
coverage gap rather than an error, silent omission, or invented fact. Earlier
selectors retain their accepted error schemas and bytes.

## Project-owned fixture and oracle

`framework-declarations-v1` is Apache-2.0 project-owned and contains no source
from an external framework or pilot repository. Its builder module covers
every entity role, nested groups, configuration, duplicate paths under
different methods, direct local targets, an unresolved external target, an
ambiguous local target, and an unused-builder decoy. Its independent
attribute/macro module covers route, component, service, configuration,
command, runtime-entry, bridge, derive, qualified endpoint, `cfg`, `cfg_attr`,
and declarative-macro-looking forms.

Comments, strings, docs, imports, names, generated and target directories,
macro-generated tokens, and `build.rs` are reviewed hard negatives. The
fixture must never be compiled, fetched, linked, expanded, or executed. Its
manifest binds every byte, Git blob, deterministic tree, synthetic commit,
expected identity, state, ownership, evidence, diagnostic, gap, document
statement, and exact-ID query kind.

## Determinism, security, and compatibility

The complete semantic payload is invariant across 50 source/chunk/schedule
permutations and isolated replay. IDs do not use offsets or ordering. Invalid
UTF-8 boundaries, malformed forms, NFC collisions, unsafe paths, symlink
escapes, privacy canaries, evidence mismatches, ambiguous required bindings,
and every maximum-plus-one terminate before publication.

The selected standard local path launches zero Cargo, rustc, Git, build
script, procedural macro, target, dependency, generated-source, network,
model-provider, or external-repository authority. It does not read `target`,
generated directories, dependency sources, gitlinks, nested repositories, or
paths outside the immutable R4 plan. First-party `unsafe`, new dependencies,
manifest changes, and lockfile changes are outside this decision.

## Public pilot observations

The non-vendored lexical sample records:

- Lekton at `7a4d1a4a30468f4c18ce158a9b825680b00f4820`: 88
  route-like builder/source occurrences and 7 layer/service-builder-like
  occurrences;
- RustDesk at `d412d198720aa56f6cfed2dfad262e8fb1322fb7`: zero
  route-builder occurrences and 75 command/runtime/bridge/component/service/
  handler-looking outer attributes, with its gitlink unopened.

The retained pilot descriptor records commands, environment, unavailable
pre-issue timing, gaps, and limitations honestly. These counts motivate the
two source styles only. They are not goldens, completeness claims, runtime
facts, performance evidence, framework branches, or vendored corpus. Future
product acceptance reruns the pinned repositories and retains complete timing
and resource evidence.

## Test-first governance evidence

The independent conformance guard was committed at
`be5ffb9a8380975ab8458adfb5ca55a70540d268` before this decision and every R6
schema, fixture, golden, or bundle byte. On the required base plus only that
guard:

```text
python3 -m unittest scripts.tests.test_s4_r6_framework_declarations_contract
```

failed with exit `1` only because this Decision 0016 path was absent. Stdout
was empty. The retained 704-byte stderr log has SHA-256
`aad6a707da779c1737c863aa693826933015031da7a989914276f105b7604b68`;
the test-first guard has SHA-256
`2dc7e2627165f6879733562b1459216365f6a0f0d9f6eceed7e37f65d1c3a48f`.
No production, dependency, R5 contract, fixture, golden, or unrelated
protected byte changed before Red.

The retained Red proves governance was absent, not that product behavior is
implemented. A separate post-merge Ready issue must define the executable CLI
Red and production evidence.

## Consequences

Positive consequences:

- committed registration syntax becomes queryable without being mislabeled as
  runtime behavior;
- macro- and attribute-heavy code remains visible through explicit candidates
  and gaps;
- ontology roles stay framework-neutral and evidence-backed;
- exact identities, strict schemas, limits, errors, docs, and query dispatch
  are reviewable before implementation;
- R5 and selector-absent behavior remain immutable.

Costs and limitations:

- R6 intentionally misses source forms outside the closed profiles;
- attribute and macro candidates remain semantically unresolved;
- compiler-grade cross-crate resolution, macro products, types, traits, calls,
  and generated code remain R7 work;
- reachability, execution, active configuration, and observed behavior remain
  future implementation-aware or runtime-evidence work;
- the pilot timing was not retained before issue creation and cannot support a
  performance claim.

## Rollback and implementation boundary

Revert PR #118 as one unit before authorizing any R6 product issue. The package
contains only specification, schemas, project-owned fixture, immutable
goldens, retained Red, and traceability. It adds no runtime, dependency,
migration, release, or destructive action.

After protected merge, a new Ready issue must fix Approved requirement IDs,
one S4/R6 vertical public journey, executable product Red, exact allowed and
protected paths, risk owner, rollback, evidence, and correction budget. This
decision does not authorize self-approval, self-merge, direct push to `main`,
or production implementation.
