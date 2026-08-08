# R6 framework declarations fixture

This is a **project-owned** Apache-2.0 fixture for the proposed S4/R6 contract.
It is generic and contains no source copied from Lekton, RustDesk, or any
framework dependency.

The fixture intentionally combines two independent source styles:

- a closed tail-expression `RegistrationSet::new()` builder profile whose
  direct calls are source registration declarations only;
- attribute, derive, `cfg`, `cfg_attr`, qualified proc-macro-looking, and
  declarative-macro forms that remain `candidate_unresolved`.

Conformance tools must **never compile, execute, expand, or fetch** this
repository. `build.rs`, `generated/`, and `target/` contain sentinels and hard
negatives. Comments, strings, documentation, imports, names, an unused builder,
and macro token trees must not become authoritative framework facts. The
expected file binds exact identities, spans, diagnostics, coverage gaps, docs,
and query kinds while explicitly denying runtime reachability or execution.
