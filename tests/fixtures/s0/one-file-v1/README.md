# S0 one-file Git fixture

This is a synthetic, first-party fixture for the S0 acquisition contract. It
contains no copied third-party source or confidential data. The repository does
not yet have a project-wide license, so this fixture is used only as repository
test material until that governance decision is made.

The committed fixture contains source seeds and a manifest, not a nested
`.git` directory. The future acceptance harness materializes a fresh temporary
SHA-1 repository for every test, with global configuration and hooks disabled.
Commit A contains `commit-a/main.rs`; commit B replaces the same single path
with `commit-b/main.rs` and has A as its parent.

The manifest fixes all bytes that affect Git object identity:

- path and mode;
- source bytes and their SHA-256;
- author and committer identity;
- Unix timestamp and `+0000` timezone;
- commit message and parent;
- expected blob, tree, and commit OIDs.

The independent repository-maintenance test recomputes those object identities
without invoking product code. A mismatch is a fixture change requiring
semantic review, not a golden file to regenerate automatically.

The manifest also defines an `isolation` variant applied only after the base
repository is materialized. It installs an executable `post-checkout` sentinel
hook and a loopback HTTPS remote in `.git`; it does not alter the committed
one-file tree. The Linux process/network observer must prove those capabilities
are never exercised by `noesis scan`.
