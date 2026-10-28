# Milestones and validation status

## Current branch — 0.54.2

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

Phase 6.1 now includes the bundled pinned engine, bounded/cancellable video inspection, video/audio metadata, HDR and timing warnings, and real-media tests. Phase 6.2 adds seeking/playback controls, a filmstrip, temporary in/out preview marks, and cancellable compatible SDR previews. Packaged-app acceptance, saved editing projects, undo/redo, and video export remain incomplete. See [VIDEO_ENGINE.md](VIDEO_ENGINE.md).

Probe media, scoped playback/proxies, one-source trim/crop/rotate/audio edits, saved projects/undo, and verified new-file export. See [VIDEO_ROADMAP.md](VIDEO_ROADMAP.md) for implementation order and acceptance criteria.

## Phase 7 — Basic multi-video editing

Multiple assets on one sequential timeline, split/trim/reorder/ripple-delete, linked audio, mixed-media normalization, save/relink/recovery, and continuous export. Start with hard cuts. See [VIDEO_ROADMAP.md](VIDEO_ROADMAP.md).

## Separately scoped future work

Reviewed People correction/merge/split tools, tag removal/rename and undo, cache management, and capability-tested Immich integration. Immich connectivity is not currently implemented.
