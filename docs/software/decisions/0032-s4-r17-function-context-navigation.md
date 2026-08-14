# Decision 0032: R17 function-centered context and navigation

- Status: Proposed branch-scoped candidate
- Date: 2026-08-14
- Issue: [#178](https://github.com/smutti/codenoesis/issues/178)
- Authorization: accountable-maintainer high-risk R17/S4 authorization recorded for the complete issue #178 package
- Exact base: `f0d0fc998a9158e7c8e96a5b70c8830a3150dd22`
- Requirements: Proposed `FR-CTX-001`, Proposed `FR-EXP-009`, and the bounded amendments listed in issue #178
- Slice: `S4`
- Risk: high
- Owner and approver: `@smutti`

## Context

The implemented R16 graph already contains evidence-backed Rust functions and
methods, one callable signature per supported callable, ordered parameters,
declared return spellings, body facts, uniquely proven local calls, claims,
evidence, derivations, diagnostics, and coverage gaps. Exact-ID queries and the
versioned explorer preserve those records, but a consumer must reconstruct a
function from raw identities and generic neighborhoods. This is technically
lossless but unnecessarily difficult for a human and unsuitable as a bounded
LLM context contract.

Protected PR #177 corrected LocalExplorerV3-V9 exact-schema loading and made
their generic graph inspection effective. It changed no ontology fact and did
not add a function-centered projection. R17 is additive to that protected
baseline and does not reopen Decision 0031.

## Decision

Add the explicit selector `rust-function-context-v1`. For one validated R16
head and one exact existing `rust.function` or `rust.method` identity, it emits
canonical `codenoesis.function-context/v1`. The projection groups only already
represented facts: root callable, lexical owner, one signature, ordered
parameters, direct body facts, proven incoming and outgoing `CALLS`, applicable
claims, evidence, diagnostics, coverage gaps, derivations, and stable navigation
roles.

The explicit explorer selector with the same value accepts exactly canonical
PortableGraphV9 and emits additive LocalExplorerV10. PortableGraphV9 remains
the interchange authority; R17 creates no PortableGraphV10 and no ontology,
snapshot, semantic-hash, identity, evidence, query, or portable family.
Selector absence preserves LocalQueryResultV13 and LocalExplorerV1-V9 bytes.

R17 starts the production-readiness `G0` lane by advertising the capability
only in `local-experimental-r17`. That profile is a source-build engineering
preview, not Local GA, a signed distribution, a support commitment, or release
authority.

## Function context contract

The context records:

- source repository identity, immutable commit and tree, snapshot identity,
  semantic hash, snapshot, graph, and ontology versions;
- root callable kind, identity, name, module, visibility, owner, exact claim,
  and evidence references;
- one `rust.callable_signature` linked by `HAS_SIGNATURE`, including visibility,
  qualifiers, ABI, generics, where clause, body state and digest, declared return
  state, and declared return spelling;
- contiguous zero-based parameters linked by `HAS_PARAMETER`, preserving
  pattern, declared type or explicit absence, receiver state, claims, evidence,
  and ordinal;
- direct `HAS_BODY_FACT`, outgoing and incoming `CALLS`, their endpoints,
  applicable derivations, and one normalized display signature;
- diagnostics, coverage gaps, and a fixed sorted limitation set that keeps
  unsupported meaning visible;
- a deterministic navigation table whose IDs are references to retained graph
  subjects rather than inferred hyperlinks.

The projection is `declared_source_only`. It never resolves aliases or types,
instantiates generics, chooses an active `cfg`, proves compiler validity,
resolves method dispatch, infers ownership or side effects, computes returned
values, or claims runtime behavior. It contains no raw source, body, expression,
initializer, condition, literal, URL, credential, absolute root, environment,
argument, telemetry, or model data.

Missing or unsupported meaning is not omitted silently. It remains a retained
diagnostic, coverage gap, candidate claim, or one of these fixed limitations:

```text
active_cfg_not_selected
compiler_validity_not_proven
declared_types_not_resolved
dispatch_not_resolved
ownership_and_aliasing_not_computed
returned_values_not_proven
runtime_behavior_not_observed
side_effects_not_computed
```

## Navigation and browser contract

LocalExplorerV10 validates the complete PortableGraphV9 boundary before
enabling controls. It provides a bounded function/method index, a compact
declared-signature card, clickable parameter/call/body/evidence/uncertainty
records, deterministic URL fragments, and at most 128 in-memory navigation
history entries. It retains exact-ID search and bounded SVG neighborhoods.

Browser rendering uses text nodes only. Graph content never enters HTML or
executable code. The viewer has no network, remote asset, storage, cookie,
telemetry, clipboard, dynamic-code, source-repository, process, plugin, model,
mutation, repair, inference, truncation, or active-world authority. Rejection
clears all prior context and disables navigation.

## Bounds and failures

FunctionContextV1 is limited to 4,194,304 bytes including LF, 256 parameters,
256 linked subjects, 512 linked relationships, 2,048 claims, 2,048 evidence
records, and 1,024 uncertainty records. The viewer returns at most 100 function
search results and retains at most 128 history entries. Every maximum and
maximum-plus-one is tested.

A non-callable root, missing or duplicate signature, non-contiguous or duplicate
parameter ordinal, invalid endpoint, dangling identity or evidence, inconsistent
claim or derivation, non-canonical order, privacy-denied field, unsupported
schema, or limit excess returns one typed failure, empty stdout, and no partial
publication. Repair, fallback, sampling, deduplication, truncation, and retry do
not turn invalid input into a context.

## Oracle

The project-owned fixture is
`urn:codenoesis:fixture:s4-rust-function-context-v1`, commit
`09093916dfc9b925fa22c7de660b67103a4def01`, tree
`aed06ed2ea7447c85a25b03fa48dd9d591b7d825`. The primary root is method `scale`,
entity `urn:codenoesis:entity:blake3:08acb2c94e0a5448751cac2901892967ce6489d47cbe88ffd1354dca8ad5a3ce`.
Its reviewed signature is:

```text
pub fn scale<T>(&self, value: i32, fallback: T) -> Result<i32, T> where T: Clone,
```

The context has three ordered parameters (`&self`, `value: i32`, `fallback: T`),
one proven outgoing local call to `clamp`, one unresolved method call
`fallback.clone`, 11 linked entities, 9 linked relationships, 20 claims,
11 evidence records, one diagnostic, seven coverage gaps, zero selected
derivations, and 11 navigation entries. The exact IDs and properties are frozen
in the machine oracle before production edits.

Fifty permutations and ten schedules must produce byte-identical contexts and
explorers. A pinned Lekton callable with parameters and an explicit return must
complete two fresh-store context and browser journeys without changing existing
R16 semantic or PortableGraphV9 bytes.

## Verification

The governance checkpoint contains this decision, candidate requirements,
schemas, fixture, machine oracle, Python guard, and a narrow public acceptance
test before production edits. On the exact base, governance is Green and the
public test is Red only because `noesis query` rejects the unknown
`--context-profile rust-function-context-v1` selector. Green requires exact
fixture output, selector-absence compatibility, CLI/browser parity, real-browser
navigation, invalid/security/limit/determinism cases, two Lekton replays, the
complete repository gate, independent review, and protected manual merge.

Merge makes this exact package Approved and Implemented but not Verified.
Independent acceptance of the complete immutable evidence pack remains a
separate lifecycle action.

## Consequences

- Humans can navigate a function as one evidence-backed card instead of joining
  generic records manually.
- LLM consumers receive a compact bounded declared-source context with explicit
  uncertainty and no model invocation.
- Existing ontology and interchange contracts remain byte-identical.
- Rollback is a complete revert of issue #178.
