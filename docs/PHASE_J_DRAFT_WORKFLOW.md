# Phase J — Experimental draft workflow adapter

The pure `crossfadeDraftWorkflow` adapter connects the existing in-memory crossfade editing history with the versioned source- and revision-bound snapshot codec.

## Draft flow

1. Open a snapshot only after strict identity validation; missing data (`null`) is a new draft, while malformed/stale data fails explicitly.
2. Each accepted cut-edit action creates one in-memory history entry with ordinary undo and redo. No-op edits do not create history.
3. At finalization, independently validate the **current** project revision, ordered clip/frame clock, saved SHA-256 metadata and exact trim marks against the captured baseline.
4. Reject any stale/changed context rather than blindly saving or rebasing.
5. If edits were undone to the original state, return the exact original JSON (or `null`) with `changed: false` to avoid an unnecessary write.
6. If state changed, emit a strictly validated JSON **candidate**, which a future transactional backend may accept or refuse.

This module has **no Tauri calls, filesystem writes, project schema v3 changes, editor UI, or production preview/export routing**. A candidate JSON string is not a durable save. Native source-byte verification, revision-safe atomic commit, crash recovery, migration and installed Windows real-media acceptance remain separate production gates.

Any later integration must preserve current hard-cut/dip functionality, no-overwrite exports and read-only original media.
