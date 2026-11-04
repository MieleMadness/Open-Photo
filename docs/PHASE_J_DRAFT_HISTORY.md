# Phase J — Crossfade draft undo/redo foundation

The experimental `crossfadeDraftHistory` module wraps the validated frame-clock cut commands with the existing editor's bounded `editHistory` queue. It keeps one immutable snapshot per accepted edit, supports undo and redo, leaves the redo stack untouched on a no-op, rejects stale clip IDs or impossible overlap budgets without changing history, and clones caller-owned input.

This is an **in-memory engineering foundation**. It does not alter schema v3, production project persistence, native export, active previews, or the editor UI. The saved-project integration should only be attempted with a reviewed versioned migration, Rust/TypeScript validation parity, authenticated original-source tracking and a feature gate that safely refuses unsupported transitions on older application versions.

Before production integration:
1. Establish the persisted schema migration, old-draft compatibility and recovery behavior.
2. Connect editing actions to the production transactional save queue and reveal discarded cut transitions.
3. Make native preview and export use the same verified overlap clock and preserve no-overwrite source safety.
4. Obtain exact-head CI and installed Windows real-media acceptance evidence. Do not treat synthetic crossfade tests as interactive app acceptance.

Current shipped behavior remains hard cuts and supported dip-to-black transitions. The new experimental helpers are not user-facing.
