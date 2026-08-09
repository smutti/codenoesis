# Software decision records

Decision records fix product or engineering semantics that are too detailed for
the SRS but must not be left to accidental implementation choices.

| ID | Status | Decision |
|---|---|---|
| [0001](0001-s0-walking-skeleton-contract.md) | Accepted; effective on protected merge of PR #8 | S0 local Git binding, snapshot envelope, canonical hash, typed errors, fixture, and Red oracle |
| [0021](0021-s4-r10-cfg-declaration-alternatives-contract.md) | Proposed; effective only after protected manual merge | S4 R10 direct-cfg method declaration alternatives, V9/V12 lineage, evidence identities, query, export, explorer, limits, fixture, and Red oracle |

A record marked Proposed is reviewable input, not implementation authority. An
accepted record becomes binding only through the protected governance and merge
process described in the SRS. Under the disclosed single-maintainer bootstrap,
the manual merge by `@smutti` is the human approval event; agents cannot merge.
