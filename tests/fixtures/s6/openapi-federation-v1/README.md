# S6 bounded OpenAPI federation fixture

This project-owned fixture defines the smallest reviewed S6 federation
journey. It contains one immutable OpenAPI 3.1.0 HTTP/JSON provider contract,
two explicit client-operation declarations, one explicit operation decoy, and
one heuristic-only candidate.

`workspace-provider-only.json` fixes the valid zero-client boundary and its
golden report has empty client and federation collections. The client
declarations in the other workspaces are user-authorized catalog evidence.
They do not prove source-language, decoder, serializer, framework, or runtime
behavior. Their source locators are reserved for later independently approved
provider and client capability profiles. In particular, the Kotlin-shaped
paths align with the immutable S7 fixture without advertising Kotlin or KMP
support.

`provider/openapi.yaml` is byte-identical to the contract in the ratified S7
fixture. `provider/openapi.json` is a format-order variant with the same
approved normalized projection. The S6 oracle requires both inputs to produce
the same service, operation, schema, field, client, and link identities and the
same source-neutral federation semantic hash. Source path, source format, byte
digest, and evidence selectors remain truthful format-specific metadata.

The `variants` directory contains hard negatives for duplicate keys, anchors
and aliases, merge keys, custom tags, multiple YAML documents, remote
references, local reference cycles, malformed YAML, an unsupported OpenAPI
version, and conflicting client authority. `unsupported-semantics.yaml` is a
representable positive variant: callbacks, webhooks, links, security
semantics, server variables, and non-JSON media produce exact typed coverage
gaps while preserving supported provider facts.

YAML provider evidence binds a normalized OpenAPI location and a reviewed
one-based inclusive source span. JSON provider and workspace declaration
evidence bind canonical JSON Pointers. Heuristic matching is exact Unicode
scalar-sequence equality over OpenAPI `info.title` and `operationId`: one
match remains a candidate, while zero or multiple matches produce typed gaps
and never automatic confirmation.

The standard S6 path must not run a package manager, compiler, build script,
target process, hook, plugin, model provider, or network client. It must read
only explicitly authorized local roots, buffer and validate the complete
report, write exactly one LF-terminated JSON document to stdout on success,
and write nothing on failure except one typed stderr error.
