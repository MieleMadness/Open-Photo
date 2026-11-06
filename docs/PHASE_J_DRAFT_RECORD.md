# Phase J — Versioned crossfade draft record (experimental)

`crossfadeDraftRecord` provides a bounded, strict JSON codec for a future opt-in crossfade draft. It is **not** wired into any production project save file, SQLite entry, Tauri command or preview/export path.

## Contract

- Record version is explicitly `1`, independent of the existing production project schema v3.
- Record binds `project_id`, 30/60 FPS and an ordered list of `clip.id` plus allocated whole project frames.
- Boundaries retain only explicit adjacent clip IDs and whole-frame overlap lengths. Unsupported versions, extra fields, invalid frame budgets, duplicate cuts, and stale clip identities are rejected rather than guessed.
- A **missing** draft record can be interpreted as no crossfades, but an invalid or stale existing record throws. Never overwrite invalid records with an empty state on load.
- JSON payloads are limited to 64 KiB and 200 clips. The existing two-hour duration ceiling remains enforced.
- Encode and decode make detached copies; no source media or existing project data is modified.

## Required before enabling persistence

1. Review the Rust and TypeScript persisted schema, migration and rollback strategy (including old clients opening newer drafts).
2. Decide where the record is durably stored, with atomic commits, revision checks, backup/history and recovery.
3. Replace frame-count-only identity with a source-aware native-verified binding to the project revision and saved source marks.
4. Integrate the editor's undo/redo, cut invalidation notices, and guarded save lifecycle.
5. Run exact-head CI, mixed-project native tests, real-footage installed-Windows acceptance and no-overwrite export verification.

Production hard cuts and dip-to-black remain the only active transition modes. This module intentionally does not claim live crossfade support.
