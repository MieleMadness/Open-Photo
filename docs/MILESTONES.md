# Milestones and validation status

## Current branch — 0.54.3

The development branch contains a Windows Tauri photo library with workspaces, local indexing, previews, nondestructive photo-copy export, managed XMP keywords, maps, and opt-in local semantic/People/OCR features. Source-media originals remain read-only. CI success and installed-app acceptance are separate requirements.

### Integrity work implemented

- Namespace-aware, preservation-oriented XMP keyword updates and recovery copies.
- Incomplete-scan reconciliation protection and surfaced errors.
- Complete tag/GPS batching, tag refresh, and workspace isolation.
- Oriented, allocation-limited shared image decoding and serialized background editor jobs.
- Scoped local video streaming with operating-system decoder fallback.
- Durable organizational data with migration and previous-value history.
- Persistent People IDs; names protect reviewed groups from automatic additions.
- Locked dependencies and independent frontend/Rust validation, followed by installer packaging.

### Release acceptance still required

Use a copied media library to verify packaged WebView2 playback, all EXIF orientations, large-library metadata completeness, unavailable-drive recovery, existing-name migration, and settings recovery. GPU/model inference also requires real compatible model bundles and hardware validation. CI does not substitute for those checks.

## Phase 6 — Single-video editing

Phases 6.1/6.2 supply inspection, local streaming, playback controls, filmstrips, and compatible SDR previews. Version 0.54.3 adds versioned single-source projects, persistent edit settings, revision-checked autosave, recent drafts, undo/redo/reset, project-file copies, verified relinking, and rendered edit previews. Video export is not implemented. Packaged Windows acceptance with real media and hardware remains required. See [VIDEO_PROJECTS.md](VIDEO_PROJECTS.md), [VIDEO_ENGINE.md](VIDEO_ENGINE.md), and [VIDEO_ROADMAP.md](VIDEO_ROADMAP.md).

## Phase 7 — Basic multi-video editing

Multiple assets on one sequential timeline, split/trim/reorder/ripple-delete, linked audio, mixed-media normalization, save/relink/recovery, and continuous export. Start with hard cuts. See [VIDEO_ROADMAP.md](VIDEO_ROADMAP.md).

## Separately scoped future work

Reviewed People correction/merge/split tools, tag removal/rename and undo, cache management, and capability-tested Immich integration. Immich connectivity is not currently implemented.
