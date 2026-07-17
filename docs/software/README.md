# CodeNoesis Software

> Status: **design baseline — planned**. An infrastructure-only Rust workspace
> exists for repository verification, but no CodeNoesis product runtime, product
> crate, API, database schema, or deployment described here has been implemented.

CodeNoesis is planned as a production-grade repository intelligence platform. It will analyze polyglot source repositories, build an evidence-backed knowledge graph, generate maintainable technical documentation, federate knowledge across projects, and explain the likely impact of software changes.

This directory records the engineering track. Experimental hypotheses, novel inference techniques, and publication-oriented work belong in the separate [research track](../research/README.md).

The normative product requirements, verification model, TDD policy, and
incremental vertical-slice plan are defined in the
[Software Requirements Specification](software-requirements-specification.md).

## Product goals

The planned system will:

- analyze C, C++, Java, Rust, JavaScript, TypeScript, and mixed-language repositories;
- document architecture, modules, business behavior, APIs, events, data, configuration, integrations, deployment, and operational procedures;
- connect every generated claim to source evidence or mark it explicitly as unknown;
- maintain a typed, versioned graph within and across repositories;
- identify services, contracts, clients, call sites, owners, and deployment units affected by a change;
- support incremental refreshes when repositories evolve;
- expose the same application capabilities through a CLI, REST API, and Model Context Protocol server;
- remain useful without an LLM and keep external model use opt-in;
- isolate untrusted repository content, extractors, and plugins.

## Engineering principles

1. **Evidence before narrative.** Deterministic parsing, contracts, compiler indexes, and traceable rules produce facts. Language models may explain evidence but may not silently promote guesses to facts.
2. **One domain, replaceable adapters.** Storage, model providers, parsers, artifact stores, transports, and authentication depend on stable ports rather than leaking into the domain.
3. **Immutable analysis snapshots.** Readers see either the previous complete snapshot or the new complete snapshot, never a partial ingest.
4. **Local-first, server-ready.** The same domain and artifact contracts support an offline SQLite mode and a multi-user PostgreSQL deployment.
5. **Bounded execution.** Scans, graph traversal, model calls, plugins, and Council reviews have explicit resource and cost limits.
6. **Version everything.** Schemas, ontology, extractor versions, configuration, repository revision, and derivation history are recorded with every artifact.
7. **Unknown is a valid result.** Missing, contradictory, or weak evidence remains visible rather than being hidden by fluent prose.

## Planned product boundary

### Version 1 scope

- Git repository acquisition pinned to an immutable commit SHA.
- Language and contract extraction through built-in Rust adapters, Tree-sitter, and optional SCIP imports.
- Typed property graph, provenance, deterministic inference, documentation views, repository federation, and change-impact reports.
- SQLite/local filesystem profile and PostgreSQL/S3-compatible server profile.
- CLI, REST, asynchronous jobs, Server-Sent Events, and MCP interfaces.
- OIDC authentication, workspace authorization, audit events, sandboxing, observability, backup, and migration support.
- Selective Council review for ambiguous or high-impact inferences.
- A repository-intelligence skill pack that invokes stable CodeNoesis interfaces instead of duplicating analysis logic.

### Explicit non-goals for version 1

- A full graphical user interface.
- Executing arbitrary repository build scripts during a standard scan.
- Treating embeddings, RDF projections, LLM output, or generated Markdown as the canonical knowledge store.
- Requiring Neo4j, a vector database, NATS, or a cloud model for the baseline deployment.
- Loading third-party Rust dynamic libraries into the main process.
- Automatically changing the ontology or confirming an inferred fact without a governed review path.

## Delivery shape

CodeNoesis is planned as a modular Rust monolith with a small number of operational processes:

- `noesis`: local and remote CLI;
- `codenoesisd`: stateless HTTP and MCP service;
- `codenoesis-worker`: durable analysis worker;
- `codenoesis-sandbox`: isolated extractor/plugin runner;
- `codenoesis-migrator`: database migration utility.

The modular monolith keeps transactional behavior and deployment understandable while preserving crate-level boundaries that can later be moved behind process boundaries if measured scale requires it. See the complete [software architecture](architecture.md).

## Planned milestones

| Phase | Intended outcome | Indicative duration |
| --- | --- | ---: |
| Foundation | ADRs, threat model, ontology v1, artifact contracts, benchmark corpus, Cargo workspace | 2 weeks |
| Local core | Git acquisition, built-in extraction, SQLite, content-addressed artifacts, CLI, atomic snapshots | 5 weeks |
| Knowledge layer | Documentation, deterministic rules, federation, impact analysis, incremental refresh | 5 weeks |
| Server platform | PostgreSQL, durable jobs, REST/MCP, OIDC/RBAC, object storage, telemetry | 6 weeks |
| Governed AI | Model gateway, Council shadow mode, skill pack, agent orchestration | 4 weeks |
| Hardening and pilot | Fuzzing, fault injection, performance, restore exercises, 3–5 repository pilot, `1.0` gate | 5–7 weeks |

The estimate assumes four Rust engineers plus part-time platform and security support. It is a planning assumption, not a delivery commitment.

The phase table remains a macro planning view. Implementation order and release
eligibility are governed by the smaller, demonstrable slices in the
[Software Requirements Specification](software-requirements-specification.md#12-incremental-delivery-plan).

## Production-readiness exit criteria

Version `1.0` will not be considered production-ready until:

- every generated claim references valid evidence or is explicitly marked unknown;
- repeated analysis of the same revision and configuration produces stable identifiers and content hashes;
- interrupted or duplicated jobs cannot expose partial snapshots;
- cross-project fixtures identify all known clients without selecting designed decoys;
- Council or LLM output cannot autonomously create deterministic or confirmed facts;
- authorization, isolation, migration, restore, fault, compatibility, and load tests pass;
- the documented service-level objectives in [architecture.md](architecture.md#service-level-objectives) are met on the reference environment;
- there are no unaccepted Critical or High security findings;
- operational runbooks and rollback procedures have been exercised.

## Primary technical references

- [The Rust Programming Language release notes](https://doc.rust-lang.org/stable/releases.html)
- [Tree-sitter: using parsers](https://tree-sitter.github.io/tree-sitter/using-parsers/)
- [SCIP code intelligence protocol](https://github.com/sourcegraph/scip)
- [Model Context Protocol Rust SDK](https://github.com/modelcontextprotocol/rust-sdk)
- [Wasmtime Component Model](https://docs.wasmtime.dev/api/wasmtime/component/index.html)
- [PostgreSQL `SELECT`, including `SKIP LOCKED`](https://www.postgresql.org/docs/current/sql-select.html)
- [OpenTelemetry documentation](https://opentelemetry.io/docs/)
- [RustSec Advisory Database](https://rustsec.org/)
- [`cargo-deny`](https://github.com/EmbarkStudios/cargo-deny)
