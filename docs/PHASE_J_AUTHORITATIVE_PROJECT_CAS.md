# Phase J — Authoritative video-project revision CAS (test-only)

The experimental `crossfade_project_cas` regression module exercises a two-table SQLite transaction: the authoritative `video_sequences` revision is read under `TransactionBehavior::Immediate` **before** any separate crossfade draft is inserted or updated. The draft revision must also match. A conflict aborts without overwriting previous state, and a recovery history record is written within the same transaction. Injected history failures roll back the draft change.

This is an **in-memory, test-only proof**, not a production persistence implementation. In the fixture, crossfade draft rows are independently bound to the saved project revision. If the real video project advances, a previous draft is rejected rather than silently rebased. The prototype does not attempt automatic migration, nor does it reopen the old draft against a changed video project.

## Remaining production gates

- Use the existing production project database and native `video_sequence` validation; do not trust revision metadata supplied solely by the frontend.
- Validate the complete versioned snapshot, exact source marks and pinned original-file fingerprints before writing.
- Ensure production transactions are revision-checked and use durable recovery and safe schema migrations, including old clients.
- Implement explicit user-visible resolution for stale drafts, source changes and interrupted writes.
- Keep current hard-cut/dip rendering unchanged until native preview/export parity and installed-Windows footage acceptance are complete.

No production table, migration, user file, Tauri command, editor UI, or export path is changed by this PR.
