# S4 R4 Cargo manifest facts fixture

This project-owned fixture defines the canonical `cargo-manifest-facts-v1`
declaration corpus. It is intentionally generic and is not copied from Lekton,
RustDesk, or another external repository.

The root is a virtual workspace and `crates/app` is its only analyzed member.
The manifests cover package defaults and inheritance, explicit targets,
normal/dev/build and target-specific dependencies, registry/path/Git source
declarations, optional dependencies, requested features, feature-member syntax,
patch declarations, build-script presence, unsupported metadata/profile/lint
families, and credential-shaped locator sentinels.

The fixture is declaration evidence only. A conforming scan never fetches a
dependency, resolves a feature or target world, applies a patch, follows a path
dependency, executes Cargo/rustc/build/target/proc-macro code, or emits a raw
external locator. Missing `crates/shared` and `vendor/custom` paths are
deliberate and must not be opened or treated as resolution failures.

`manifest.json` binds every materialized byte. `expected-manifest-facts.json`
binds canonical identities, ordering, redaction, normalized paths, coverage,
and relationship recipes used by the implementation acceptance test.
