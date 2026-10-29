# Milestones and validation status

## Current branch — 0.56.6

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

Use a copied media library to verify packaged WebView2 playback, all EXIF orientations, large-library metadata completeness, unavailable-drive recovery, existing-name migration, and settings recovery. The installer now includes the pinned AI assets and native runtimes, with synthetic installed CPU inference and uninstall lifecycle gates. Recognition accuracy and GPU behavior still require hardware validation. CI does not substitute for those checks.

## Phase 6 — Single-video editing

Phases 6.1/6.2 supply inspection, local streaming, playback controls, filmstrips, and compatible SDR previews. Version 0.54.3 adds versioned single-source projects, persistent edit settings, revision-checked autosave, recent drafts, undo/redo/reset, project-file copies, verified relinking, and rendered edit previews. Version 0.54.4 adds staged, validated MP4 export with shared edit rendering, progress/cancel, source revalidation, full output decode, explicit audio policy, and no-overwrite publication. Packaged Windows acceptance with real media and hardware remains required. See [VIDEO_EXPORT.md](VIDEO_EXPORT.md), [VIDEO_PROJECTS.md](VIDEO_PROJECTS.md), [VIDEO_ENGINE.md](VIDEO_ENGINE.md), and [VIDEO_ROADMAP.md](VIDEO_ROADMAP.md).

Version 0.56.2 adds protected updater installation and a separate full-recording native acceptance mode with continuous-export decode and distant timestamp comparisons. Real execution evidence, packaged playback, perceptual A/V synchronization, drive failures, and whole-app performance remain separate gates. The newer interface and major-only signed updater foundation are retained.

## Pre-v1.0 release checklist

Physical source/destination drive removal, actual disk-full/quota/write failure, installed-package GUI playback, and whole-app performance measurements remain required before v1.0. The owner deferred physical drive-failure testing on 2026-10-29. Further long-form development/testing is deferred to 1.x; the successful 0.56.2 long-recording evidence is retained, and current development uses short clips.

## Phase 7 — Basic multi-video editing

0.56.6 adds pure, validated timeline commands and an accessible timeline surface: trim, split, reorder, ripple deletion, repeat instances, durations/zoom, undo/redo, and revision-safe autosave. The Library opens the editor; the workspace selector remains at top left. Empty timelines preserve the media bin. This does not complete the assembled playback/export or shared editor/ownership migrations. See [DESIGN_SYSTEM_AUDIT.md](DESIGN_SYSTEM_AUDIT.md) and [DEVELOPMENT_PHASES.md](DEVELOPMENT_PHASES.md).

0.56.4 begins the feedback-driven UI cleanup alongside Phase 7: navigation/folder management, viewer navigation, video transport/trim wording, map wrapping, and a documented roadmap. These presentation changes do not replace the Phase 7 timeline work below.

0.56.3 delivers the project/media-bin foundation: bounded schema-v2 multi-asset documents, explicit v1 project-file migration into new drafts, multi-file import and content deduplication, selected-source previews, revision-checked saving/reopening, portable copies, and per-asset relinking. Phase 6 editing/export remains unchanged. Sources currently belong to one selected library folder and its subfolders. Timeline assembly and combined export remain next, not completed.

Multiple assets on one sequential timeline, split/trim/reorder/ripple-delete, linked audio, mixed-media normalization, save/relink/recovery, and continuous export. Start with hard cuts. See [VIDEO_ROADMAP.md](VIDEO_ROADMAP.md).

## Separately scoped future work

Reviewed People correction/merge/split tools, tag removal/rename and undo, cache management, and capability-tested Immich integration. Immich connectivity is not currently implemented.
