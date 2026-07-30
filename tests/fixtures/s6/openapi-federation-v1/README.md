# S6 bounded OpenAPI federation fixture

This project-owned fixture defines the smallest reviewed S6 federation
journey. It contains one immutable OpenAPI 3.1.0 HTTP/JSON provider contract,
two explicit client-operation declarations, one explicit operation decoy, and
one heuristic-only candidate.

The client declarations are user-authorized workspace catalog evidence. They
do not prove source-language, decoder, serializer, framework, or runtime
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
version, and conflicting client authority. They are specification inputs, not
accepted syntax.

The standard S6 path must not run a package manager, compiler, build script,
target process, hook, plugin, model provider, or network client. It must read
only explicitly authorized local roots, buffer and validate the complete
report, write exactly one LF-terminated JSON document to stdout on success,
and write nothing on failure except one typed stderr error.
