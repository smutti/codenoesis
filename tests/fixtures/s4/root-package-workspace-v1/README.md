# R3 root-package workspace fixture

This project-owned fixture defines generic Cargo workspace shapes for
`FR-EXT-008`. It contains no external repository source.

The future implementation harness materializes one immutable Git revision by
combining exactly one file from `root-manifests/` as root `Cargo.toml` with the
files beneath `shared-tree/`. For the boundary scenario it also writes the
reviewed mode `160000` entry declared in `manifest.json`; the gitlink target is
not checked out.

The fixture covers:

- an implicit non-virtual root package;
- an explicit `"."` root package with identical crate IDs;
- a standalone root package;
- a virtual workspace control;
- a member/exclusion conflict;
- conventional and explicit library/binary roots;
- multiple conventional binary roots in one member package;
- deferred dependency, feature, patch, target-world, build, and
  `required-features` meaning;
- non-executed build-script, procedural-macro, and target sentinels;
- one workspace member that remains an external R2 gitlink boundary.

`expected-workspace-plan.json` is the reviewed structural oracle. It does not
claim Cargo resolution, active features, dependency selection, generated
source, build output, or nested-repository analysis.
