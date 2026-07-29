# S7 implementation-aware API compatibility fixture

This project-owned fixture defines the smallest reviewed three-view semantic
API diff for S7:

- one unchanged OpenAPI 3.1 response contract in two provider revisions;
- one Rust provider implementation that changes `nickname` from
  unconditionally present to conditionally absent;
- one Kotlin/KMP client whose decoder requires `nickname`;
- one Kotlin/KMP client whose decoder safely handles absence;
- one name-similar Kotlin/KMP decoy that calls a different operation;
- one `displayName` mapping through an unsupported custom helper, which must
  remain `unresolved` with an explicit coverage gap.

The source files are specification inputs, not advertised language or framework
support. Future adapters must separately approve and prove the exact source
capabilities they use. The standard analysis path must not compile, execute,
fetch dependencies for, or open a network connection from this fixture.

The reviewed report compares committed source semantics rather than only the
OpenAPI files. It distinguishes field presence from nullability and defaults,
keeps declared contract, provider implementation, and client assumptions as
separate evidence views, and never promotes a type name or missing trace into a
behavioral fact.
