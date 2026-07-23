# S1 safe-inventory fixture

This first-party synthetic repository is the reviewed source corpus for the
`S1 — Safe inventory` contract. The committed revision contains Rust and shell
source, a Cargo manifest, an OpenAPI contract, configuration, ownership data,
documentation, and deliberately unsupported content.

`build.rs` and `tools/sentinel.sh` are inert sentinel inputs. A conforming scan
classifies them but never compiles, launches, sources, or otherwise executes
them. The manifest also defines generated malicious variants for path
traversal, symlinks, gitlinks, resource boundaries, archive bombs, and parser
stress without storing large or unsafe generated artifacts in Git.

All fixture material is covered by the repository-wide Apache-2.0 license.
