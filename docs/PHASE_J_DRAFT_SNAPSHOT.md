# Phase J — Revision- and source-bound draft snapshot

This experimental `crossfadeDraftSnapshot` adapter combines the existing versioned frame-clock draft record with the exact ordered source identity contract and the saved project's expected revision.

It is **not** production persistence. It has no filesystem, Tauri, database or output side effects and is not called by the active editor.

## Invariants

- Encode/decode require matching project ID, 30/60 FPS, ordered clip IDs and allocated frames through the existing draft-record validator.
- Source clips must match that ordered frame identity; SHA-256, source asset ID, byte count and exact rational trim marks remain bound through the existing source-identity validator.
- A nonnegative safe-integer `project_revision` must match exactly. Stale revisions never silently rebase to the current project.
- Missing draft (`null`) is empty. Any existing malformed, future-version, unexpected-field or stale draft fails closed rather than being silently erased.
- Validation copies inputs and does not alter project/source metadata, original media or output files.

## Unresolved production work

1. Define an actual durable storage schema and migration, Rust/TypeScript parity, old-client compatibility, atomic revision-checked commit and failure recovery.
2. Establish that source SHA-256 and byte counts are verified against pinned native original handles, not merely trusted serialized metadata.
3. Wire history and transition reconciliation into one user-visible edit transaction; give users clear notices if an edge can no longer be retained.
4. Gate active preview/export by matching the native project clock and verified source manifests; preserve existing hard-cut/dip fallback.
5. Obtain installed-Windows SDR/VFR real-footage acceptance, cancellation/no-overwrite checks, and exact-head CI before user-facing enabling.

A green test for this pure contract does not imply that crossfade persistence, interactive preview or export is shipped.
