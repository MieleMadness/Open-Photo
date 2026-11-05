# Phase J — In-memory atomic draft transaction regression

This test-only Rust module uses an in-memory SQLite database to prototype the transaction semantics needed for **future** crossfade persistence. It is compiled with `#[cfg(test)]` and cannot change user projects.

The regression tests verify that a draft update is accepted only when both the expected saved-project revision and the draft revision match; each changed record increments its own draft revision. A successful changed update writes a recovery copy in the *same* transaction, retains at most 20 prior drafts, and never overwrites data on a stale comparison. If the recovery insertion fails, the entire update rolls back. No-op updates avoid unnecessary history.

**Security boundary:** This fixture does **not** read or lock real video projects or prove that its expected project revision matches the authoritative video project revision. It does not validate production source files, install a persistent table, migrate schema v3, implement a Tauri save command, or enable UI/preview/export. A production design must atomically compare against the authoritative project row, add recovery/repair for crashes and old versions, validate the entire snapshot contract, enforce disk ownership and source-manifest verification, and pass installed Windows acceptance. Existing hard-cut/dip editing remains unchanged.
